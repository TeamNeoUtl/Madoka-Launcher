use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::Duration;

use iced::widget::{column, container, row};
use iced::{Element, Length, Subscription, Task};
use iced_m3::theme::{ColorScheme, Mode, Theme};
use iced_m3::widget::card;
use iced_m3::widget::hybrid_icon::Icon;
use iced_m3::widget::{self as m3, Badge, navrail};
use launcher_core::{AppSelection, InstallPlan, Instance, InstanceId, Kind, Package, TaskInfo};
use rust_i18n::t;
use url::Url;

use launcher_providers::Versions;
use launcher_providers::au2pkg::is_au2pkg_filename;

use crate::deeplink::{self, DeepLinkAction};
use crate::ops::{AppUpdates, Browsed, ConfigEntry, Loaded, RestoreReport, Services};
use crate::screen::edit::{self, Catalog, Pending, Tab};
use crate::screen::new_instance::Submission;
use crate::screen::{main_screen, new_instance, settings, tasks};
use crate::{strings, style};

const REDRAW_INTERVAL: Duration = Duration::from_millis(250);
const ICON_INSTANCES: &str = "\u{e5c3}";
const ICON_NEW: &str = "\u{e145}";
const ICON_TASKS: &str = "\u{e6b1}";
const ICON_SETTINGS: &str = "\u{e8b8}";
const RAIL_VERTICAL_PADDING: f32 = 16.0;
const CONFIG_FILE_NAME: &str = "madoka-packages.json";

#[derive(Debug, Clone)]
pub enum Message {
    Main(main_screen::Message),
    New(new_instance::Message),
    Edit(edit::Message),
    Settings(settings::Message),
    TabSelected(Screen),
    ThemeToggled,
    Saved(Result<(), String>),
    PortableSwitched(bool, Result<(), String>),
    ImageLoaded(InstanceId, String, Result<Vec<u8>, String>),
    FontsLoaded(Result<Vec<String>, String>),
    Loaded(Result<Loaded, String>),
    VersionsLoaded(Kind, Result<Versions, String>),
    CatalogLoaded(InstanceId, Result<Vec<Package>, String>),
    Planned(InstanceId, Result<Box<(Package, InstallPlan)>, String>),
    Updated(Result<(), String>),
    /// App and addon versions that the update of the instance installs.
    AppUpdatesLoaded(InstanceId, Result<AppUpdates, String>),
    /// Packages that the update of the instance leaves to the browser window.
    UpdatedAll(InstanceId, Vec<Package>),
    Created(Option<InstanceId>),
    InstallRecommended(InstanceId),
    Finished,
    Opened(Result<(), String>),
    Browsed(InstanceId, Box<Package>, Result<Option<Browsed>, String>),
    UpdateChecked(bool, Result<Option<String>, String>),
    SelfUpdated(Result<(), String>),
    ManualPluginsScanned(Result<Vec<String>, String>),
    ConfigImported(InstanceId, Option<String>),
    ConfigRestored(RestoreReport),
    ConfigExported(Result<Option<String>, String>),
    DeepLinkUrl(String),
    Redraw,
    FileDropped(std::path::PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    NewInstance,
    Tasks,
    Settings,
    Edit,
}

impl Screen {
    /// Position in the navigation rail. The edit screen belongs to the instance list.
    const fn rail_index(self) -> usize {
        match self {
            Self::Main | Self::Edit => 0,
            Self::NewInstance => 1,
            Self::Tasks => 2,
            Self::Settings => 3,
        }
    }
}

pub struct App {
    services: Services,
    theme: Theme,
    screen: Screen,
    instances: Vec<Instance>,
    selected: Option<InstanceId>,
    pending_delete: Option<InstanceId>,
    context: Option<InstanceId>,
    notice: Option<String>,
    form: new_instance::Form,
    edit: Option<edit::State>,
    /// Edit screens that the user left, by instance. Reopening restores tab, search and detail view.
    saved_edits: HashMap<InstanceId, edit::State>,
    settings: settings::Form,
    browse_queue: VecDeque<(InstanceId, Package, Url)>,
    browsing: bool,
    pending_recommended: bool,
    /// The user chose to keep the dialog of installed deprecated packages closed.
    hide_deprecated_notice: bool,
    /// Start-up URL. It runs after the instance list has loaded.
    pending_deeplink: Option<String>,
}

impl App {
    pub fn new(services: Services) -> (Self, Task<Message>) {
        let mode = services.theme_name().map_or(Mode::Dark, style::mode_of);
        strings::set_locale(services.locale());
        let form = new_instance::Form::new(services.host);
        let settings = settings::Form {
            token: services.token().unwrap_or_default().to_owned(),
            font: services.font_name().map(str::to_owned),
            fonts: Vec::new(),
            locale: services.locale(),
            portable: services.is_portable(),
            update: settings::UpdateState::Checking,
            dns: services
                .dns_servers()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        };
        let hide_deprecated_notice = services.hides_deprecated_notice();
        let app = Self {
            services,
            theme: Theme::default(mode),
            screen: Screen::Main,
            instances: Vec::new(),
            selected: None,
            pending_delete: None,
            context: None,
            notice: None,
            form,
            edit: None,
            saved_edits: HashMap::new(),
            settings,
            browse_queue: VecDeque::new(),
            browsing: false,
            pending_recommended: false,
            hide_deprecated_notice,
            pending_deeplink: deeplink::url_from_args(),
        };
        let load = Task::batch([
            app.reload(),
            Task::perform(crate::ops::installed_fonts(), |result| {
                Message::FontsLoaded(result.map_err(|error| error.to_string()))
            }),
            app.check_update(false),
        ]);
        (app, load)
    }

    pub const fn theme(&self) -> iced::Theme {
        style::iced_theme(self.theme.mode)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Main(message) => self.update_main(message),
            Message::New(message) => self.update_new(message),
            Message::Edit(message) => self.update_edit(message),
            Message::Settings(message) => self.update_settings(message),
            Message::TabSelected(screen) => self.select_screen(screen),
            Message::ThemeToggled => {
                self.theme.mode = match self.theme.mode {
                    Mode::Light => Mode::Dark,
                    Mode::Dark => Mode::Light,
                };
                let name = style::mode_name(self.theme.mode).to_owned();
                Task::perform(self.services.clone().save_theme(name), |result| {
                    Message::Saved(result.map_err(|error| error.to_string()))
                })
            }
            Message::ImageLoaded(id, url, result) => {
                if let Some(state) = self.edit.as_mut().filter(|state| state.id == id) {
                    state.images.insert(
                        url,
                        match result {
                            Ok(bytes) => edit::ImageState::Ready(
                                iced::widget::image::Handle::from_bytes(bytes),
                            ),
                            Err(reason) => edit::ImageState::Failed(reason),
                        },
                    );
                }
                Task::none()
            }
            Message::PortableSwitched(enabled, result) => {
                self.notice = Some(match result {
                    Ok(()) => {
                        self.settings.portable = enabled;
                        t!("portable_restart").into_owned()
                    }
                    Err(reason) => reason,
                });
                Task::none()
            }
            Message::FontsLoaded(result) => {
                match result {
                    Ok(fonts) => self.settings.fonts = fonts,
                    Err(reason) => self.notice = Some(reason),
                }
                Task::none()
            }
            Message::Saved(result) | Message::Opened(result) => {
                if let Err(reason) = result {
                    self.notice = Some(reason);
                }
                Task::none()
            }
            Message::Loaded(result) => {
                self.loaded(result);
                self.pending_deeplink
                    .take()
                    .map_or_else(Task::none, |url| Task::done(Message::DeepLinkUrl(url)))
            }
            Message::VersionsLoaded(kind, result) => {
                if kind == self.form.kind {
                    self.form.finish_load(result);
                    self.refilter();
                }
                Task::none()
            }
            Message::CatalogLoaded(id, result) => {
                let Some(state) = self.edit.as_mut().filter(|state| state.id == id) else {
                    return Task::none();
                };
                state.catalog = match result {
                    Ok(packages) => Catalog::Ready(state.with_registered(packages)),
                    Err(reason) => Catalog::Failed(reason),
                };
                if !self.hide_deprecated_notice
                    && let Some(instance) = self.instances.iter().find(|i| i.config.id == id)
                {
                    state.check_deprecated(instance);
                }
                let follow = if state.tab == Tab::Browse && state.viewing_package_id.is_none() {
                    restore_browse_scroll(state)
                } else {
                    load_screenshots(&self.services, state)
                };
                Task::batch([follow, pending_follow_up(state)])
            }
            Message::AppUpdatesLoaded(id, result) => {
                self.preview_updates(id, result.unwrap_or_default())
            }
            Message::Planned(id, result) => self.planned(id, result),
            Message::Updated(result) => match result {
                Ok(()) => self.reload(),
                Err(reason) => {
                    self.notice = Some(reason);
                    Task::none()
                }
            },
            Message::UpdatedAll(id, packages) => {
                self.browse_queue
                    .extend(packages.into_iter().map(|package| {
                        let url = package.page_url.clone();
                        (id, package, url)
                    }));
                Task::batch([self.reload(), self.next_browse()])
            }
            Message::Created(id) => {
                if id.is_some() {
                    self.selected = id;
                }
                let reload = self.reload();
                let recommended = std::mem::take(&mut self.pending_recommended);
                if let Some(id) = id.filter(|_| recommended) {
                    return Task::batch([reload, Task::done(Message::InstallRecommended(id))]);
                }
                reload
            }
            Message::InstallRecommended(id) => self.install_recommended_plugins(id),
            Message::Finished => self.reload(),
            Message::Browsed(id, package, result) => {
                self.browsing = false;
                let download = match result {
                    Ok(Some(_)) if self.instance_running(id) => {
                        self.notice = Some(t!("instance_running_error").into_owned());
                        Task::none()
                    }
                    Ok(Some(browsed)) => self.start_download(id, *package, browsed),
                    Ok(None) => Task::none(),
                    Err(reason) => {
                        self.notice = Some(reason);
                        Task::none()
                    }
                };
                Task::batch([download, self.next_browse()])
            }
            Message::UpdateChecked(manual, result) => {
                self.settings.update = match result {
                    Ok(Some(version)) => settings::UpdateState::Available(version),
                    Ok(None) => settings::UpdateState::UpToDate,
                    Err(reason) => {
                        if manual {
                            self.notice = Some(reason);
                        }
                        settings::UpdateState::Idle
                    }
                };
                Task::none()
            }
            Message::SelfUpdated(result) => {
                self.settings.update = match (result, &self.settings.update) {
                    (Ok(()), _) => settings::UpdateState::Applied,
                    (Err(reason), state) => {
                        self.notice = Some(reason);
                        match state {
                            settings::UpdateState::Installing(version) => {
                                settings::UpdateState::Available(version.clone())
                            }
                            other => other.clone(),
                        }
                    }
                };
                Task::none()
            }
            Message::ManualPluginsScanned(result) => match result {
                Ok(names) if names.is_empty() => {
                    self.notice = Some(t!("manual_scan_none").into_owned());
                    Task::none()
                }
                Ok(names) => {
                    self.notice = Some(
                        t!(
                            "manual_scan_registered",
                            count = names.len(),
                            names = names.join(", ")
                        )
                        .into_owned(),
                    );
                    self.reload()
                }
                Err(err) => {
                    self.notice = Some(t!("manual_scan_error", error = err).into_owned());
                    Task::none()
                }
            },
            Message::ConfigExported(result) => {
                self.notice = match result {
                    Ok(Some(path)) => Some(t!("config_exported", path = path).into_owned()),
                    Ok(None) => None,
                    Err(error) => Some(t!("config_export_error", error = error).into_owned()),
                };
                Task::none()
            }
            Message::ConfigImported(id, content) => {
                if self.instance_running(id) {
                    self.notice = Some(t!("instance_running_error").into_owned());
                    return Task::none();
                }
                let Some(content) = content.filter(|s| !s.trim().is_empty()) else {
                    self.notice = Some(t!("config_empty").into_owned());
                    return Task::none();
                };
                let parsed: Result<serde_json::Value, _> = serde_json::from_str(&content);
                let Ok(value) = parsed else {
                    self.notice = Some(t!("config_invalid").into_owned());
                    return Task::none();
                };
                let entries: Vec<ConfigEntry> = value
                    .get("plugins")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|plugin| {
                        Some(ConfigEntry {
                            id: plugin.get("id")?.as_str()?.to_owned(),
                            version: plugin
                                .get("version")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                            paused: plugin
                                .get("paused")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false),
                        })
                    })
                    .collect();
                if entries.is_empty() {
                    self.notice = Some(t!("config_no_plugins").into_owned());
                    return Task::none();
                }
                let Some(inst) = self.instances.iter().find(|i| i.config.id == id) else {
                    return Task::none();
                };
                let kind = inst.config.kind;
                self.notice = Some(t!("config_import_start", count = entries.len()).into_owned());
                Task::perform(
                    self.services.clone().restore_config(
                        kind,
                        id,
                        t!("task_install").into_owned(),
                        entries,
                    ),
                    Message::ConfigRestored,
                )
            }
            Message::ConfigRestored(report) => {
                let mut lines = vec![
                    t!(
                        "config_restored",
                        installed = report.installed,
                        paused = report.paused
                    )
                    .into_owned(),
                ];
                if !report.version_differs.is_empty() {
                    lines.push(
                        t!(
                            "config_version_differs",
                            list = report.version_differs.join(", ")
                        )
                        .into_owned(),
                    );
                }
                if !report.unavailable.is_empty() {
                    lines.push(
                        t!("config_unavailable", list = report.unavailable.join(", ")).into_owned(),
                    );
                }
                self.notice = Some(lines.join("\n"));
                self.reload()
            }
            Message::DeepLinkUrl(url) => {
                if let Some((instance, action)) = deeplink::parse_targeted(&url) {
                    self.handle_deeplink_action(instance, action)
                } else {
                    self.notice = Some(t!("deeplink_unknown", url = url).into_owned());
                    Task::none()
                }
            }
            Message::FileDropped(path) => self.import_dropped(path),
            Message::Redraw => {
                if let Some(url) = deeplink::poll_incoming() {
                    return Task::batch([Task::done(Message::DeepLinkUrl(url)), bring_to_front()]);
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let board = self.services.board.snapshot();
        let content: Element<'_, Message> = match self.screen {
            Screen::Main => main_screen::view(&main_screen::Props {
                theme: &self.theme,
                instances: &self.instances,
                selected: self.selected,
                pending_delete: self.pending_delete,
                context: self.context,
                notice: self.notice.as_deref(),
                tasks: &board,
            })
            .map(Message::Main),
            Screen::NewInstance => {
                new_instance::view(&self.form, &self.theme, self.services.host).map(Message::New)
            }
            Screen::Tasks => tasks::view(&board, &self.theme),
            Screen::Settings => self.with_notice(
                settings::view(
                    &self.settings,
                    &self.theme,
                    &self.services.root().display().to_string(),
                )
                .map(Message::Settings),
            ),
            Screen::Edit => self.edit_view(),
        };
        let content = container(content)
            .padding(style::PADDING)
            .width(Length::Fill)
            .height(Length::Fill);
        let body = row![self.rail(&board), content].height(Length::Fill);
        container(body)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::page(&self.theme))
            .into()
    }

    fn edit_view(&self) -> Element<'_, Message> {
        let instance = self.edit.as_ref().and_then(|state| {
            self.instances
                .iter()
                .find(|instance| instance.config.id == state.id)
                .map(|instance| (state, instance))
        });
        match instance {
            Some((state, instance)) => {
                self.with_notice(edit::view(state, instance, &self.theme).map(Message::Edit))
            }
            None => style::label(&self.theme, t!("no_selection")).into(),
        }
    }

    /// Places the notice above `content`. Pages other than the instance list show it here.
    fn with_notice<'a>(&'a self, content: Element<'a, Message>) -> Element<'a, Message> {
        let Some(notice) = self.notice.as_deref() else {
            return content;
        };
        column![
            m3::card(
                card::Style::filled(&self.theme),
                row![
                    style::icon(style::icons::INFO)
                        .size(16)
                        .color(self.theme.on_surface()),
                    style::label(&self.theme, notice.to_owned()).size(12),
                ]
                .spacing(8)
                .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill),
            content
        ]
        .spacing(style::GAP)
        .into()
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(REDRAW_INTERVAL).map(|_| Message::Redraw),
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Window(iced::window::Event::FileDropped(path)) => {
                    Some(Message::FileDropped(path))
                }
                _ => None,
            }),
        ])
    }

    fn rail(&self, board: &[TaskInfo]) -> Element<'_, Message> {
        let active = board.iter().filter(|task| task.state.is_active()).count();
        let items = vec![
            nav_item(ICON_INSTANCES, t!("tab_instances"), None, Screen::Main),
            nav_item(ICON_NEW, t!("tab_new"), None, Screen::NewInstance),
            nav_item(
                ICON_TASKS,
                t!("tab_tasks"),
                (active > 0).then(|| Badge {
                    label: Some(active.to_string().into()),
                }),
                Screen::Tasks,
            ),
            nav_item(
                ICON_SETTINGS,
                t!("tab_settings"),
                matches!(self.settings.update, settings::UpdateState::Available(_))
                    .then_some(Badge { label: None }),
                Screen::Settings,
            ),
        ];
        let rail = m3::navrail(navrail::Style::new(&self.theme), items)
            .item_alignment(navrail::ItemAlignment::Top)
            .container_vertical_padding(RAIL_VERTICAL_PADDING)
            .active(self.screen.rail_index());
        container(rail)
            .height(Length::Fill)
            .style(style::rail_background(&self.theme))
            .into()
    }

    fn select_screen(&mut self, screen: Screen) -> Task<Message> {
        self.screen = screen;
        if screen != Screen::Edit {
            self.stash_edit();
        }
        if screen == Screen::NewInstance && self.form.needs_load() {
            self.form.begin_load();
            return self.load_versions();
        }
        Task::none()
    }

    fn update_main(&mut self, message: main_screen::Message) -> Task<Message> {
        use main_screen::Message as M;
        if !matches!(message, M::ContextOpen(_)) {
            self.context = None;
        }
        match message {
            M::ContextOpen(id) => {
                self.pending_delete = None;
                self.selected = Some(id);
                self.context = Some(id);
                Task::none()
            }
            M::ContextClose => Task::none(),
            M::Edit => {
                let Some(instance) = self.selected_instance() else {
                    return Task::none();
                };
                let id = instance.config.id;
                self.notice = None;
                self.open_edit(id, |_| {})
            }
            M::Duplicate => {
                let Some(instance) = self.selected_instance() else {
                    return Task::none();
                };
                let id = instance.config.id;
                if instance.is_running() {
                    self.notice = Some(t!("instance_running_error").into_owned());
                    return Task::none();
                }
                let name = format!("{}{}", instance.config.name, t!("copy_suffix"));
                let handle = self.services.board.add(format!(
                    "{}: {}",
                    t!("task_duplicate"),
                    instance.config.name
                ));
                Task::perform(
                    self.services.clone().duplicate(handle, id, name),
                    Message::Created,
                )
            }
            M::Select(id) => {
                if self.selected != Some(id) {
                    self.pending_delete = None;
                }
                self.selected = Some(id);
                Task::none()
            }
            M::ShowNew => self.select_screen(Screen::NewInstance),
            M::Refresh => {
                self.notice = None;
                self.reload()
            }
            M::Launch => {
                let Some(instance) = self.selected_instance() else {
                    return Task::none();
                };
                let id = instance.config.id;
                let title = format!("{}: {}", t!("task_launch"), instance.config.name);
                let handle = self.services.board.add(title);
                Task::perform(self.services.clone().launch(handle, id), |()| {
                    Message::Redraw
                })
            }
            M::LaunchInstance(id) => {
                self.selected = Some(id);
                let Some(instance) = self.selected_instance() else {
                    return Task::none();
                };
                let id = instance.config.id;
                let title = format!("{}: {}", t!("task_launch"), instance.config.name);
                let handle = self.services.board.add(title);
                Task::perform(self.services.clone().launch(handle, id), |()| {
                    Message::Redraw
                })
            }
            M::OpenFolder => {
                let Some(instance) = self.selected_instance() else {
                    return Task::none();
                };
                let dir = instance.dir.clone();
                Task::perform(self.services.clone().open_folder(dir), |result| {
                    Message::Opened(result.map_err(|error| error.to_string()))
                })
            }
            M::Delete => {
                self.pending_delete = self.selected;
                Task::none()
            }
            M::CancelDelete => {
                self.pending_delete = None;
                Task::none()
            }
            M::ConfirmDelete => {
                let Some(instance) = self.selected_instance() else {
                    return Task::none();
                };
                if instance.is_running() {
                    self.notice = Some(t!("instance_running_error").into_owned());
                    self.pending_delete = None;
                    return Task::none();
                }
                let id = instance.config.id;
                if self.pending_delete != Some(id) {
                    return Task::none();
                }
                let title = format!("{}: {}", t!("task_delete"), instance.config.name);
                let handle = self.services.board.add(title);
                self.pending_delete = None;
                Task::perform(self.services.clone().delete(handle, id), |()| {
                    Message::Finished
                })
            }
        }
    }

    fn update_new(&mut self, message: new_instance::Message) -> Task<Message> {
        use new_instance::Message as M;
        match message {
            M::KindSelected(kind) => {
                let host = self.services.host;
                if new_instance::availability(kind, host) != new_instance::Availability::Available {
                    return Task::none();
                }
                self.form.select_kind(kind);
                self.load_versions()
            }
            M::NameChanged(name) => {
                self.form.name = name;
                Task::none()
            }
            M::GroupChanged(group) => {
                self.form.group = group;
                Task::none()
            }
            M::EditionSelected(edition) => {
                self.form.edition = edition;
                self.refilter();
                Task::none()
            }
            M::AppImageToggled(checked) => {
                self.form.app_image = checked;
                self.refilter();
                Task::none()
            }
            M::VersionSelected(choice) => {
                self.form.selected = Some(choice);
                Task::none()
            }
            M::AddonSelected(choice) => {
                self.form.addon_selected = Some(choice);
                Task::none()
            }
            M::ToggleRecommended(checked) => {
                self.form.install_recommended = checked;
                Task::none()
            }
            M::Reload => {
                self.form.begin_load();
                self.load_versions()
            }
            M::PickSource => Task::perform(pick_folder(), |folder| {
                Message::New(new_instance::Message::SourcePicked(folder))
            }),
            M::SourcePicked(folder) => {
                if let Some(folder) = folder {
                    self.form.set_source(folder);
                }
                Task::none()
            }
            M::ClearSource => {
                self.form.source = None;
                Task::none()
            }
            M::Submit => {
                let Some(submission) = self.form.submission() else {
                    return Task::none();
                };
                self.pending_recommended = matches!(submission, Submission::Create(_))
                    && self.form.kind == Kind::Aviutl2
                    && self.form.install_recommended;
                self.form = new_instance::Form::new(self.services.host);
                self.screen = Screen::Main;
                let services = self.services.clone();
                match submission {
                    Submission::Create(request) => {
                        let title = format!("{}: {}", t!("task_create"), request.name);
                        let handle = services.board.add(title);
                        Task::perform(services.create(handle, request), Message::Created)
                    }
                    Submission::Import(request) => {
                        let title = format!("{}: {}", t!("task_import"), request.name);
                        let handle = services.board.add(title);
                        Task::perform(services.import(handle, request), Message::Created)
                    }
                }
            }
            M::Cancel => {
                self.screen = Screen::Main;
                Task::none()
            }
        }
    }

    /// Scans the processes, so call it only for operations that the guard covers.
    fn instance_running(&self, id: InstanceId) -> bool {
        self.instances
            .iter()
            .find(|inst| inst.config.id == id)
            .is_some_and(launcher_core::Instance::is_running)
    }

    fn update_edit(&mut self, message: edit::Message) -> Task<Message> {
        use edit::Message as M;
        let Some(state) = self.edit.as_mut() else {
            return Task::none();
        };
        let id = state.id;
        let guarded = matches!(
            message,
            M::Install(_)
                | M::ConfirmPending
                | M::Remove(_)
                | M::RemoveDeprecated(_)
                | M::RequestUpdateAll
                | M::ConfirmUpdateAll
                | M::SaveSettings
        );
        if guarded
            && self
                .instances
                .iter()
                .find(|inst| inst.config.id == id)
                .is_some_and(launcher_core::Instance::is_running)
        {
            self.notice = Some(t!("instance_running_error").into_owned());
            return Task::none();
        }
        match message {
            M::Back => {
                self.stash_edit();
                self.screen = Screen::Main;
                Task::none()
            }
            M::TabSelected(tab) => {
                state.tab = tab;
                if state.needs_catalog() {
                    state.catalog = Catalog::Loading;
                    return self.load_catalog(false);
                }
                if tab != Tab::Browse {
                    return Task::none();
                }
                restore_browse_scroll(state)
            }
            M::AuthorSelected(author) => {
                state.search = author;
                state.viewing_package_id = None;
                state.active_screenshot_idx = 0;
                state.preview_open = false;
                restore_browse_scroll(state)
            }
            M::SearchChanged(search) => {
                state.search = search;
                Task::none()
            }
            M::CategorySelected(category) => {
                state.category = category;
                Task::none()
            }
            M::Scrolled(offset, height) => {
                state.viewport_height = height;
                match state.pending_scroll {
                    Some(target) if offset <= 0.0 && target > 0.0 => {}
                    _ => {
                        state.pending_scroll = None;
                        state.scroll_y = offset;
                    }
                }
                Task::none()
            }
            M::ReloadCatalog => {
                state.catalog = Catalog::Loading;
                self.load_catalog(true)
            }
            M::Install(package_id) => {
                let kind = state.kind;
                let Some(package) = state.package(&package_id).cloned() else {
                    return Task::none();
                };
                Task::perform(self.services.clone().plan(kind, package), move |result| {
                    Message::Planned(id, result.map(Box::new).map_err(|error| error.to_string()))
                })
            }
            M::Open(package_id) => {
                let Some(package) = state.package(&package_id).cloned() else {
                    return Task::none();
                };
                let plan = InstallPlan::OpenInBrowser(package.page_url.clone());
                state.pending = Some(Pending { package, plan });
                Task::none()
            }
            M::ConfirmPending => {
                let Some(Pending { package, plan }) = state.pending.take() else {
                    return Task::none();
                };
                match plan {
                    InstallPlan::OpenInBrowser(url) => self.enqueue_browse(id, package, url),
                    plan => self.start_install(id, package, plan),
                }
            }
            M::CancelPending => {
                state.pending = None;
                Task::none()
            }
            M::Remove(plugin_id) => remove_task(&self.services, id, plugin_id),
            M::RemoveDeprecated(plugin_id) => {
                if let Some(plugins) = &mut state.deprecated_dialog {
                    plugins.retain(|plugin| plugin.id != plugin_id);
                    if plugins.is_empty() {
                        state.deprecated_dialog = None;
                    }
                }
                remove_task(&self.services, id, plugin_id)
            }
            M::CloseDeprecatedDialog => {
                state.deprecated_dialog = None;
                Task::none()
            }
            M::SuppressDeprecatedDialog => {
                state.deprecated_dialog = None;
                self.hide_deprecated_notice = true;
                Task::perform(
                    self.services.clone().save_hide_deprecated_notice(),
                    |result| Message::Saved(result.map_err(|error| error.to_string())),
                )
            }
            M::RequestUpdateAll => {
                if matches!(state.catalog, Catalog::Loading) {
                    state.pending_updates = true;
                    return Task::none();
                }
                Task::perform(self.services.clone().app_updates(id), move |result| {
                    Message::AppUpdatesLoaded(id, result.map_err(|error| error.to_string()))
                })
            }
            M::ConfirmUpdateAll => {
                state.update_preview = None;
                let name = self
                    .instances
                    .iter()
                    .find(|instance| instance.config.id == id)
                    .map_or_else(String::new, |instance| instance.config.name.clone());
                update_all_task(&self.services, id, &name)
            }
            M::CancelUpdateAll => {
                state.update_preview = None;
                Task::none()
            }
            M::SortOrderChanged(order) => {
                state.sort_order = order;
                Task::none()
            }
            M::StatusFilterChanged(filter) => {
                state.status_filter = filter;
                Task::none()
            }
            M::TagToggled(tag) => {
                if !state.tags.remove(&tag) {
                    state.tags.insert(tag);
                }
                Task::none()
            }
            M::TagsCleared => {
                state.tags.clear();
                Task::none()
            }
            M::ToggleHideDeprecated => {
                state.hide_deprecated = !state.hide_deprecated;
                Task::none()
            }
            M::TogglePausePlugin(plugin_id, paused) => Task::perform(
                self.services
                    .clone()
                    .set_plugin_paused(id, plugin_id, paused),
                |result| Message::Updated(result.map_err(|error| error.to_string())),
            ),
            M::ScanManualPlugins => {
                let packages = match &state.catalog {
                    Catalog::Ready(pkgs) => pkgs.clone(),
                    _ => Vec::new(),
                };
                let services = self.services.clone();
                Task::perform(
                    services.scan_and_register_manual_plugins(id, packages),
                    |result| Message::ManualPluginsScanned(result.map_err(|e| e.to_string())),
                )
            }
            M::ToggleCommonsSelect(commons_id) => {
                if state.selected_commons.contains(&commons_id) {
                    state.selected_commons.remove(&commons_id);
                } else {
                    state.selected_commons.insert(commons_id);
                }
                Task::none()
            }
            M::SelectAllCommons => {
                if let Some(inst) = self.instances.iter().find(|i| i.config.id == id) {
                    for record in &inst.config.plugins {
                        let Some(pkg) = state.package(&record.id) else {
                            continue;
                        };
                        if let Some(commons_id) = &pkg.niconi_commons_id {
                            state.selected_commons.insert(commons_id.clone());
                        }
                    }
                }
                Task::none()
            }
            M::DeselectAllCommons => {
                state.selected_commons.clear();
                Task::none()
            }
            M::CopySelectedCommons => {
                let text = state
                    .selected_commons
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                self.notice = Some(format!(
                    "{} {}",
                    t!("commons_copied"),
                    t!("items_count", count = state.selected_commons.len())
                ));
                iced::clipboard::write(text)
            }
            M::CopySingleCommons(commons_id) => {
                self.notice = Some(format!("{}: {commons_id}", t!("commons_copied")));
                iced::clipboard::write(commons_id)
            }
            M::OpenCommonsUrl(commons_id) => {
                if let Some(url) = launcher_core::commons_url(&commons_id) {
                    let _ = launcher_core::open_url(&url, launcher_core::Host::current());
                }
                Task::none()
            }
            M::OpenLink(link) => {
                // Only web links open; other schemes in catalog text are ignored.
                if let Ok(url) = url::Url::parse(&link).and_then(|url| {
                    if matches!(url.scheme(), "http" | "https") {
                        Ok(url)
                    } else {
                        Err(url::ParseError::RelativeUrlWithoutBase)
                    }
                }) {
                    let _ = launcher_core::open_url(&url, launcher_core::Host::current());
                }
                Task::none()
            }
            M::OpenCommonsGuide => {
                if let Ok(url) = url::Url::parse(strings::COMMONS_GUIDE_URL) {
                    let _ = launcher_core::open_url(&url, launcher_core::Host::current());
                }
                Task::none()
            }
            M::JsonInputChanged(input) => {
                state.json_input = input;
                Task::none()
            }
            M::LoadSampleJson => {
                r#"{
  "id": "example.myplugin",
  "name": "My Custom Plugin",
  "type": "plugin",
  "summary": "自作のAviUtl2用プラグインです。",
  "author": "Author",
  "latest-version": "1.0.0",
  "niconiCommonsId": "nc123456",
  "tags": ["フィルター", "自作"],
  "installer": {
    "source": { "direct": "https://example.com/myplugin.zip" },
    "install": [
      { "action": "download" },
      { "action": "extract" },
      { "action": "copy", "from": "{tmp}/myplugin.auf2", "to": "{pluginsDir}" }
    ]
  },
  "version": [
    {
      "version": "1.0.0",
      "release_date": "2026-01-01",
      "file": [{ "path": "{pluginsDir}/myplugin.auf2", "XXH3_128": "00112233445566778899aabbccddeeff" }]
    }
  ]
}"#
                .clone_into(&mut state.json_input);
                Task::none()
            }
            M::RegisterJsonPackage => {
                state.json_status = Some(state.register_json_package());
                Task::none()
            }
            M::ViewPackageDetails(pkg_id) => {
                let closed = pkg_id.is_none();
                state.viewing_package_id = pkg_id;
                state.active_screenshot_idx = 0;
                state.preview_open = false;
                if closed && state.tab == Tab::Browse {
                    return restore_browse_scroll(state);
                }
                load_screenshots(&self.services, state)
            }
            M::TogglePreview => {
                state.preview_open = !state.preview_open;
                Task::none()
            }
            M::NextScreenshot(len) => {
                if len > 0 {
                    state.active_screenshot_idx = (state.active_screenshot_idx + 1) % len;
                }
                Task::none()
            }
            M::PrevScreenshot(len) => {
                if len > 0 {
                    state.active_screenshot_idx = (state.active_screenshot_idx + len - 1) % len;
                }
                Task::none()
            }
            M::ShowLicense(content) => {
                state.license_dialog_content = content;
                Task::none()
            }
            M::ExportConfig => {
                let Some(inst) = self.instances.iter().find(|i| i.config.id == id) else {
                    return Task::none();
                };
                let export_data = serde_json::json!({
                    "name": inst.config.name,
                    "kind": inst.config.kind,
                    "app_version": inst.config.app_version,
                    "plugins": inst.config.plugins.iter().map(|p| serde_json::json!({
                        "id": p.id,
                        "version": p.version,
                        "paused": p.paused,
                    })).collect::<Vec<_>>()
                });
                let text = serde_json::to_string_pretty(&export_data).unwrap_or_default();
                Task::perform(save_text(CONFIG_FILE_NAME, text), Message::ConfigExported)
            }
            M::ImportConfig => {
                Task::perform(pick_text(), move |text| Message::ConfigImported(id, text))
            }
            M::NameChanged(name) => {
                state.name = name;
                Task::none()
            }
            M::GroupChanged(group) => {
                state.group = group;
                Task::none()
            }
            M::SaveSettings => {
                if !state.can_save() {
                    return Task::none();
                }
                let name = state.name.trim().to_owned();
                let group = Some(state.group.trim().to_owned()).filter(|group| !group.is_empty());
                Task::perform(
                    self.services.clone().update_instance(id, name, group),
                    |result| Message::Updated(result.map_err(|error| error.to_string())),
                )
            }
        }
    }

    fn update_settings(&mut self, message: settings::Message) -> Task<Message> {
        use settings::Message as M;
        match message {
            M::TokenChanged(token) => {
                self.settings.token = token;
                Task::none()
            }
            M::SaveToken => Task::perform(
                self.services
                    .clone()
                    .save_token(self.settings.token.clone()),
                |result| Message::Saved(result.map_err(|error| error.to_string())),
            ),
            M::DnsChanged(dns) => {
                self.settings.dns = dns;
                Task::none()
            }
            M::SaveDns => {
                let parsed: Result<Vec<IpAddr>, _> = self
                    .settings
                    .dns
                    .split([',', ' ', '\t'])
                    .filter(|entry| !entry.is_empty())
                    .map(str::parse)
                    .collect();
                let Ok(servers) = parsed else {
                    self.notice = Some(t!("dns_invalid").into_owned());
                    return Task::none();
                };
                self.notice = Some(t!("dns_restart").into_owned());
                Task::perform(self.services.clone().save_dns(servers), |result| {
                    Message::Saved(result.map_err(|error| error.to_string()))
                })
            }
            M::ThemeToggled => self.update(Message::ThemeToggled),
            M::FontSelected(name) => {
                self.settings.font.clone_from(&name);
                self.notice = Some(t!("font_restart").into_owned());
                Task::perform(self.services.clone().save_font(name), |result| {
                    Message::Saved(result.map_err(|error| error.to_string()))
                })
            }
            M::LocaleSelected(locale) => {
                self.settings.locale = locale;
                strings::set_locale(locale);
                self.notice = Some(t!("locale_saved").into_owned());
                Task::perform(self.services.clone().save_locale(locale), |result| {
                    Message::Saved(result.map_err(|error| error.to_string()))
                })
            }
            M::PortableToggled(enabled) => {
                let handle = self.services.board.add(t!("task_portable").into_owned());
                Task::perform(
                    self.services.clone().switch_portable(handle, enabled),
                    move |result| {
                        Message::PortableSwitched(
                            enabled,
                            result.map_err(|error| error.to_string()),
                        )
                    },
                )
            }
            M::CheckUpdate => {
                self.settings.update = settings::UpdateState::Checking;
                self.notice = None;
                self.check_update(true)
            }
            M::ApplyUpdate => {
                let settings::UpdateState::Available(version) = &self.settings.update else {
                    return Task::none();
                };
                let version = version.clone();
                let title = format!("{}: v{version}", t!("task_self_update"));
                let handle = self.services.board.add(title);
                self.settings.update = settings::UpdateState::Installing(version.clone());
                Task::perform(
                    self.services.clone().self_update(handle, version),
                    |result| Message::SelfUpdated(result.map_err(|error| error.to_string())),
                )
            }
            M::OpenDataDirectory => {
                let dir = self.services.root().to_path_buf();
                Task::perform(self.services.clone().open_folder(dir), |result| {
                    Message::Opened(result.map_err(|error| error.to_string()))
                })
            }
        }
    }

    fn planned(
        &mut self,
        id: InstanceId,
        result: Result<Box<(Package, InstallPlan)>, String>,
    ) -> Task<Message> {
        let (package, plan) = match result {
            Ok(planned) => *planned,
            Err(reason) => {
                self.notice = Some(reason);
                return Task::none();
            }
        };
        match plan {
            plan @ InstallPlan::OpenInBrowser(_) => {
                if let Some(state) = self.edit.as_mut().filter(|state| state.id == id) {
                    state.pending = Some(Pending { package, plan });
                    Task::none()
                } else if let InstallPlan::OpenInBrowser(url) = plan {
                    self.enqueue_browse(id, package, url)
                } else {
                    Task::none()
                }
            }
            plan if plan.is_verified() && !package.deprecated && package.notice.is_none() => {
                self.start_install(id, package, plan)
            }
            plan => {
                if let Some(state) = self.edit.as_mut().filter(|state| state.id == id) {
                    state.pending = Some(Pending { package, plan });
                }
                Task::none()
            }
        }
    }

    /// Installs the recommended plugins. The report lists the ones that need the browser window.
    fn install_recommended_plugins(&self, id: InstanceId) -> Task<Message> {
        let entries = strings::RECOMMENDED_PLUGIN_IDS
            .iter()
            .map(|plugin_id| ConfigEntry {
                id: (*plugin_id).to_owned(),
                version: None,
                paused: false,
            })
            .collect();
        Task::perform(
            self.services.clone().restore_config(
                Kind::Aviutl2,
                id,
                t!("task_install").into_owned(),
                entries,
            ),
            Message::ConfigRestored,
        )
    }

    fn start_install(&self, id: InstanceId, package: Package, plan: InstallPlan) -> Task<Message> {
        let title = format!("{}: {}", t!("task_install"), package.name);
        let handle = self.services.board.add(title);
        Task::perform(
            self.services
                .clone()
                .install_plugin(handle, id, package, plan),
            |()| Message::Finished,
        )
    }

    /// Opens the browser window for `url` once no other browser window is open.
    fn enqueue_browse(&mut self, id: InstanceId, package: Package, url: Url) -> Task<Message> {
        self.browse_queue.push_back((id, package, url));
        self.next_browse()
    }

    fn next_browse(&mut self) -> Task<Message> {
        if self.browsing {
            return Task::none();
        }
        let Some((id, package, url)) = self.browse_queue.pop_front() else {
            return Task::none();
        };
        self.browsing = true;
        self.browse(id, package, url)
    }

    /// Shows `url` in the browser window. The download that starts there installs `package` into `id`.
    fn browse(&self, id: InstanceId, package: Package, url: Url) -> Task<Message> {
        Task::perform(self.services.clone().browse(url), move |result| {
            Message::Browsed(
                id,
                Box::new(package),
                result.map_err(|error| error.to_string()),
            )
        })
    }

    fn start_download(&self, id: InstanceId, package: Package, browsed: Browsed) -> Task<Message> {
        let title = format!("{}: {}", t!("task_download"), browsed.label());
        let handle = self.services.board.add(title);
        Task::perform(
            self.services
                .clone()
                .download_browsed(handle, id, package, browsed),
            |()| Message::Finished,
        )
    }

    fn load_catalog(&self, refresh: bool) -> Task<Message> {
        let Some(state) = &self.edit else {
            return Task::none();
        };
        let (id, kind) = (state.id, state.kind);
        Task::perform(
            self.services.clone().catalog(kind, refresh),
            move |result| Message::CatalogLoaded(id, result.map_err(|error| error.to_string())),
        )
    }

    fn check_update(&self, manual: bool) -> Task<Message> {
        Task::perform(self.services.clone().check_update(), move |result| {
            Message::UpdateChecked(manual, result.map_err(|error| error.to_string()))
        })
    }

    fn reload(&self) -> Task<Message> {
        Task::perform(self.services.clone().list(), |result| {
            Message::Loaded(result.map_err(|error| error.to_string()))
        })
    }

    fn load_versions(&self) -> Task<Message> {
        let kind = self.form.kind;
        Task::perform(self.services.clone().app_versions(kind), move |result| {
            Message::VersionsLoaded(kind, result.map_err(|error| error.to_string()))
        })
    }

    fn loaded(&mut self, result: Result<Loaded, String>) {
        match result {
            Ok(loaded) => {
                self.notice = (!loaded.failures.is_empty()).then(|| loaded.failures.join("\n"));
                self.instances = loaded.instances;
                if self.selected.is_some_and(|id| !self.contains(id)) {
                    self.selected = None;
                }
                let ids: Vec<InstanceId> = self.instances.iter().map(|i| i.config.id).collect();
                self.saved_edits.retain(|id, _| ids.contains(id));
                if self.pending_delete.is_some_and(|id| !self.contains(id)) {
                    self.pending_delete = None;
                }
                if self
                    .edit
                    .as_ref()
                    .is_some_and(|state| !self.contains(state.id))
                {
                    self.edit = None;
                    self.screen = Screen::Main;
                }
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    fn refilter(&mut self) {
        let selection = AppSelection {
            host: self.services.host,
            edition: self.form.edition,
            bundle: self.form.bundle(),
        };
        let kind = self.form.kind;
        let installable = self
            .services
            .installable(kind, selection, self.form.versions());
        let addons = self
            .services
            .installable(kind, selection, self.form.addon_versions());
        self.form.set_choices(&installable, &addons);
    }

    fn contains(&self, id: InstanceId) -> bool {
        self.instances
            .iter()
            .any(|instance| instance.config.id == id)
    }

    fn selected_instance(&self) -> Option<&Instance> {
        let id = self.selected?;
        self.instances
            .iter()
            .find(|instance| instance.config.id == id)
    }

    /// Moves the edit screen state to `saved_edits`.
    fn stash_edit(&mut self) {
        if let Some(state) = self.edit.take() {
            self.saved_edits.insert(state.id, state);
        }
    }

    /// Opens the edit screen of `id`, with the state the user left, and loads the catalog if needed.
    fn open_edit(
        &mut self,
        id: InstanceId,
        configure: impl FnOnce(&mut edit::State),
    ) -> Task<Message> {
        let Some(instance) = self.instances.iter().find(|i| i.config.id == id) else {
            return Task::none();
        };
        self.selected = Some(id);
        let mut state = self.saved_edits.remove(&id).map_or_else(
            || edit::State::new(instance),
            |mut saved| {
                saved.reset_transient(instance);
                saved
            },
        );
        configure(&mut state);
        let load = state.needs_catalog();
        if load {
            state.catalog = Catalog::Loading;
        }
        let follow = if load {
            Task::none()
        } else {
            let scroll = if state.tab == Tab::Browse && state.viewing_package_id.is_none() {
                restore_browse_scroll(&mut state)
            } else {
                Task::none()
            };
            Task::batch([scroll, pending_follow_up(&mut state)])
        };
        self.edit = Some(state);
        self.screen = Screen::Edit;
        if load {
            self.load_catalog(false)
        } else {
            follow
        }
    }

    /// Shows the update preview of the open edit screen of `id`: the app, the addon, then the plugins.
    /// Runs the update at once when nothing is outdated.
    fn preview_updates(&mut self, id: InstanceId, app: AppUpdates) -> Task<Message> {
        let Some(state) = self.edit.as_mut().filter(|state| state.id == id) else {
            return Task::none();
        };
        let Some(inst) = self.instances.iter().find(|i| i.config.id == id) else {
            return Task::none();
        };
        let mut items: Vec<edit::UpdatePreviewItem> = [
            (t!("version").into_owned(), app.app),
            (t!("addon_version").into_owned(), app.addon),
        ]
        .into_iter()
        .filter_map(|(name, versions)| {
            let (current, latest) = versions?;
            Some((name, current, latest, None))
        })
        .collect();
        for record in inst.config.plugins.iter().filter(|record| !record.paused) {
            let Some(pkg) = state.package(&record.id) else {
                continue;
            };
            let Some(latest) = pkg.latest_version().filter(|v| *v != record.version) else {
                continue;
            };
            items.push((
                pkg.name.clone(),
                record.version.clone(),
                latest.to_owned(),
                pkg.changelog
                    .as_deref()
                    .map(|log| crate::screen::changelog::range_text(log, &record.version)),
            ));
        }
        if items.is_empty() {
            state.update_preview = None;
            update_all_task(&self.services, id, &inst.config.name)
        } else {
            state.update_preview = Some(items);
            Task::none()
        }
    }

    fn handle_deeplink_action(
        &mut self,
        instance: Option<InstanceId>,
        action: DeepLinkAction,
    ) -> Task<Message> {
        if action == DeepLinkAction::Focus {
            return bring_to_front();
        }
        let id = instance
            .filter(|id| self.contains(*id))
            .or(self.selected)
            .or_else(|| self.instances.first().map(|i| i.config.id));
        let Some(id) = id else {
            self.notice = Some(t!("no_instance_exists").into_owned());
            return Task::none();
        };
        match action {
            DeepLinkAction::Focus => Task::none(),
            DeepLinkAction::ShowUpdates => self.open_edit(id, |state| {
                state.tab = Tab::Plugins;
                state.viewing_package_id = None;
                state.pending_updates = true;
            }),
            DeepLinkAction::ViewPackage(pkg_id) => self.open_edit(id, |state| {
                state.tab = Tab::Browse;
                state.viewing_package_id = Some(pkg_id);
            }),
            DeepLinkAction::InstallPackage(pkg_id) => self.open_edit(id, |state| {
                state.tab = Tab::Browse;
                state.viewing_package_id = Some(pkg_id.clone());
                state.pending_install = Some(pkg_id);
            }),
        }
    }
}

fn nav_item(
    glyph: &'static str,
    label: Cow<'static, str>,
    badge: Option<Badge<'static>>,
    screen: Screen,
) -> navrail::Item<'static, Message> {
    let icon = Icon::Text {
        text: glyph.into(),
        font: Some(style::ICON_FONT),
    };
    navrail::Item {
        icon_active: icon.clone(),
        icon_inactive: icon,
        badge,
        label,
        on_press: Message::TabSelected(screen),
    }
}

/// Writes `text` to a file that the user picks. `Ok(None)` means the user cancelled.
async fn save_text(file_name: &'static str, text: String) -> Result<Option<String>, String> {
    let Some(handle) = rfd::AsyncFileDialog::new()
        .set_file_name(file_name)
        .add_filter("JSON", &["json"])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    handle
        .write(text.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    Ok(Some(handle.path().display().to_string()))
}

/// Folder that the user picks. `None` means cancelled.
async fn pick_folder() -> Option<std::path::PathBuf> {
    let handle = rfd::AsyncFileDialog::new().pick_folder().await?;
    Some(handle.path().to_path_buf())
}

/// Reads a UTF-8 text file that the user picks. `None` means cancelled or unreadable.
async fn pick_text() -> Option<String> {
    let handle = rfd::AsyncFileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
        .await?;
    String::from_utf8(handle.read().await).ok()
}

/// Runs the update and the install that a deep link left on `state`.
/// The update runs unless the catalog is loading; it needs the catalog only for plugin rows.
/// The install runs only with a ready catalog, because the package is looked up there.
fn pending_follow_up(state: &mut edit::State) -> Task<Message> {
    let install = state.pending_install.take();
    let updates = std::mem::take(&mut state.pending_updates);
    let mut tasks = Vec::new();
    if updates && !matches!(state.catalog, Catalog::Loading) {
        tasks.push(Task::done(Message::Edit(edit::Message::RequestUpdateAll)));
    }
    if let Some(package_id) = install.filter(|_| matches!(state.catalog, Catalog::Ready(_))) {
        tasks.push(Task::done(Message::Edit(edit::Message::Install(
            package_id,
        ))));
    }
    Task::batch(tasks)
}

/// Returns the task that scrolls the browse list back to the offset of `state`.
fn restore_browse_scroll(state: &mut edit::State) -> Task<Message> {
    state.pending_scroll = Some(state.scroll_y).filter(|offset| *offset > 0.0);
    state
        .pending_scroll
        .map_or_else(Task::none, edit::restore_scroll)
}

/// Restores the window from the minimized state and focuses it.
fn bring_to_front() -> Task<Message> {
    iced::window::latest().and_then(|id| {
        Task::batch([
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
        ])
    })
}

impl App {
    /// Imports a dropped `.au2pkg.zip` into the instance of the open edit screen.
    fn import_dropped(&mut self, path: std::path::PathBuf) -> Task<Message> {
        let is_zip = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
        if !(is_zip || is_au2pkg_filename(&path.to_string_lossy())) {
            self.notice = Some(t!("au2pkg_unsupported_file").into_owned());
            return Task::none();
        }
        let Some(state) = self.edit.as_ref() else {
            self.notice = Some(t!("au2pkg_open_instance").into_owned());
            return Task::none();
        };
        if state.kind != Kind::Aviutl2 {
            self.notice = Some(t!("au2pkg_unsupported_kind").into_owned());
            return Task::none();
        }
        if self.instance_running(state.id) {
            self.notice = Some(t!("instance_running_error").into_owned());
            return Task::none();
        }
        let name = path
            .file_name()
            .map_or_default(|name| name.to_string_lossy().into_owned());
        let handle = self
            .services
            .board
            .add(format!("{}: {name}", t!("task_au2pkg")));
        Task::perform(
            self.services.clone().import_au2pkg(handle, state.id, path),
            |()| Message::Finished,
        )
    }
}

/// Starts the download of the screenshots of the open package that no request covers yet.
fn load_screenshots(services: &Services, state: &mut edit::State) -> Task<Message> {
    let urls: Vec<String> = state
        .viewing_package_id
        .as_deref()
        .and_then(|package_id| state.package(package_id))
        .map_or_default(|package| {
            package
                .screenshots
                .iter()
                .filter(|url| !state.images.contains_key(*url))
                .cloned()
                .collect()
        });
    let id = state.id;
    let tasks: Vec<Task<Message>> = urls
        .into_iter()
        .map(|url| {
            state.images.insert(url.clone(), edit::ImageState::Loading);
            Task::perform(services.clone().fetch_image(url.clone()), move |result| {
                Message::ImageLoaded(id, url, result)
            })
        })
        .collect();
    Task::batch(tasks)
}

fn remove_task(services: &Services, id: InstanceId, plugin_id: String) -> Task<Message> {
    let title = format!("{}: {plugin_id}", t!("task_remove"));
    let handle = services.board.add(title);
    Task::perform(
        services.clone().remove_plugin(handle, id, plugin_id),
        |()| Message::Finished,
    )
}

fn update_all_task(services: &Services, id: InstanceId, name: &str) -> Task<Message> {
    let handle = services
        .board
        .add(format!("{}: {name}", t!("task_update_all")));
    Task::perform(services.clone().update_all(handle, id), move |packages| {
        Message::UpdatedAll(id, packages)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Edition, Host, InstanceStore, NewInstance, PackageSource};

    fn app_with_instance(kind: Kind) -> (tempfile::TempDir, App, InstanceId) {
        let root = tempfile::tempdir().unwrap();
        let instance = InstanceStore::new(root.path())
            .create(
                NewInstance {
                    name: "t".to_owned(),
                    kind,
                    edition: Edition::Normal,
                    bundle: launcher_core::Bundle::Archive,
                    app_version: "1".to_owned(),
                    group: None,
                },
                Host::Windows,
            )
            .unwrap();
        let id = instance.config.id;
        let (mut app, _) = App::new(Services::new(root.path().to_path_buf()).unwrap());
        app.pending_deeplink = None;
        app.instances = vec![instance];
        (root, app, id)
    }

    fn open_edit_screen(app: &mut App) {
        app.edit = Some(edit::State::new(&app.instances[0]));
        app.screen = Screen::Edit;
    }

    fn deprecated_catalog(app: &mut App) -> Vec<Package> {
        let url = Url::parse("https://example.com").unwrap();
        let mut package = Package::simple("a.b", "a", url.clone(), PackageSource::Web(url));
        package.deprecated = true;
        app.instances[0]
            .config
            .plugins
            .push(launcher_core::PluginRecord {
                id: package.id.clone(),
                version: "1".to_owned(),
                source: String::new(),
                files: Vec::new(),
                paused: false,
                uninstall_steps: Vec::new(),
            });
        vec![package]
    }

    #[test]
    fn deprecated_dialog_lists_installed_deprecated_plugins_once() {
        let (_root, mut app, id) = app_with_instance(Kind::Aviutl2);
        open_edit_screen(&mut app);
        let catalog = deprecated_catalog(&mut app);
        let _ = app.update(Message::CatalogLoaded(id, Ok(catalog.clone())));
        let state = app.edit.as_mut().unwrap();
        assert_eq!(state.deprecated_dialog.as_ref().map(Vec::len), Some(1));
        state.deprecated_dialog = None;
        let _ = app.update(Message::CatalogLoaded(id, Ok(catalog)));
        assert!(app.edit.as_ref().unwrap().deprecated_dialog.is_none());
    }

    #[test]
    fn deprecated_dialog_stays_closed_after_suppression() {
        let (_root, mut app, id) = app_with_instance(Kind::Aviutl2);
        open_edit_screen(&mut app);
        let catalog = deprecated_catalog(&mut app);
        app.hide_deprecated_notice = true;
        let _ = app.update(Message::CatalogLoaded(id, Ok(catalog)));
        assert!(app.edit.as_ref().unwrap().deprecated_dialog.is_none());
    }

    #[test]
    fn updates_link_waits_for_the_catalog_then_runs() {
        let (_root, mut app, id) = app_with_instance(Kind::Aviutl2);
        let _ = app.update(Message::DeepLinkUrl("madoka://updates".to_owned()));
        let state = app.edit.as_ref().unwrap();
        assert!(matches!(state.catalog, Catalog::Loading));
        assert!(state.pending_updates);
        let _ = app.update(Message::CatalogLoaded(id, Ok(Vec::new())));
        let state = app.edit.as_ref().unwrap();
        assert!(matches!(state.catalog, Catalog::Ready(_)));
        assert!(!state.pending_updates);
    }

    #[test]
    fn updates_link_runs_after_catalog_failure() {
        let (_root, mut app, id) = app_with_instance(Kind::Aviutl2);
        let _ = app.update(Message::DeepLinkUrl("madoka://updates".to_owned()));
        let _ = app.update(Message::CatalogLoaded(id, Err("offline".to_owned())));
        let state = app.edit.as_ref().unwrap();
        assert!(matches!(state.catalog, Catalog::Failed(_)));
        assert!(!state.pending_updates);
    }

    #[test]
    fn install_link_is_dropped_after_catalog_failure() {
        let (_root, mut app, id) = app_with_instance(Kind::Aviutl2);
        let _ = app.update(Message::DeepLinkUrl(
            "madoka://package/a.b?install=true".to_owned(),
        ));
        assert_eq!(
            app.edit.as_ref().unwrap().pending_install.as_deref(),
            Some("a.b")
        );
        let _ = app.update(Message::CatalogLoaded(id, Err("offline".to_owned())));
        assert_eq!(app.edit.as_ref().unwrap().pending_install, None);
    }

    #[test]
    fn updates_link_on_kind_without_catalog_runs_at_once() {
        let (_root, mut app, _id) = app_with_instance(Kind::Neoutl);
        let _ = app.update(Message::DeepLinkUrl("madoka://updates".to_owned()));
        assert_eq!(app.screen, Screen::Edit);
        assert!(!app.edit.as_ref().unwrap().pending_updates);
    }

    #[test]
    fn link_without_instance_reports_notice() {
        let root = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new(Services::new(root.path().to_path_buf()).unwrap());
        app.pending_deeplink = None;
        let _ = app.update(Message::DeepLinkUrl("madoka://updates".to_owned()));
        assert_eq!(app.notice, Some(t!("no_instance_exists").into_owned()));
        assert!(app.edit.is_none());
    }

    #[test]
    fn idle_instance_accepts_removal() {
        let (_root, mut app, _id) = app_with_instance(Kind::Aviutl2);
        open_edit_screen(&mut app);
        let _ = app.update(Message::Edit(edit::Message::Remove("x".to_owned())));
        assert_eq!(app.notice, None);
    }

    #[cfg(target_os = "linux")]
    fn start_sleeper(app: &App) -> std::process::Child {
        let copy = app.instances[0].app_dir().join("sleeper");
        std::fs::copy("/bin/sleep", &copy).unwrap();
        std::process::Command::new(&copy).arg("30").spawn().unwrap()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn running_instance_blocks_guarded_edit_messages() {
        let (_root, mut app, _id) = app_with_instance(Kind::Aviutl2);
        open_edit_screen(&mut app);
        let mut child = start_sleeper(&app);
        let mut notices = Vec::new();
        for message in [
            edit::Message::Remove("x".to_owned()),
            edit::Message::RequestUpdateAll,
            edit::Message::Install("x".to_owned()),
        ] {
            app.notice = None;
            let _ = app.update(Message::Edit(message));
            notices.push(app.notice.clone());
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let expected = Some(t!("instance_running_error").into_owned());
        assert!(
            notices.iter().all(|notice| *notice == expected),
            "{notices:?}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn running_instance_blocks_browsed_install() {
        let (_root, mut app, id) = app_with_instance(Kind::Aviutl2);
        let mut child = start_sleeper(&app);
        let url = Url::parse("https://example.com/a.zip").unwrap();
        let package = Package::simple("a.b", "a", url.clone(), PackageSource::Web(url.clone()));
        let browsed = Browsed {
            url,
            name: None,
            saved: None,
        };
        let _ = app.update(Message::Browsed(id, Box::new(package), Ok(Some(browsed))));
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(app.notice, Some(t!("instance_running_error").into_owned()));
    }
}
