use std::net::IpAddr;
use std::time::Duration;

use iced::widget::{column, container, row};
use iced::{Element, Length, Subscription, Task};
use iced_m3::theme::{Mode, Theme};
use iced_m3::widget::card;
use iced_m3::widget::hybrid_icon::Icon;
use iced_m3::widget::{self as m3, Badge, navrail};
use launcher_core::{AppSelection, InstallPlan, Instance, InstanceId, Kind, Package, TaskInfo};
use url::Url;

use launcher_providers::Versions;

use crate::ops::{Browsed, Loaded, Services};
use crate::screen::edit::{self, Catalog, Pending, Tab};
use crate::screen::{main_screen, new_instance, settings, tasks};
use crate::{strings, style};

const REDRAW_INTERVAL: Duration = Duration::from_millis(250);
const ICON_INSTANCES: &str = "\u{e5c3}";
const ICON_NEW: &str = "\u{e145}";
const ICON_TASKS: &str = "\u{e6b1}";
const ICON_SETTINGS: &str = "\u{e8b8}";
const RAIL_VERTICAL_PADDING: f32 = 16.0;

#[derive(Debug, Clone)]
pub enum Message {
    Main(main_screen::Message),
    New(new_instance::Message),
    Edit(edit::Message),
    Settings(settings::Message),
    TabSelected(Screen),
    ThemeToggled,
    Saved(Result<(), String>),
    FontsLoaded(Result<Vec<String>, String>),
    Loaded(Result<Loaded, String>),
    VersionsLoaded(Kind, Result<Versions, String>),
    CatalogLoaded(InstanceId, Result<Vec<Package>, String>),
    Planned(InstanceId, Result<Box<(Package, InstallPlan)>, String>),
    Updated(Result<(), String>),
    Created(Option<InstanceId>),
    Finished,
    Opened(Result<(), String>),
    Browsed(InstanceId, Box<Package>, Result<Option<Browsed>, String>),
    UpdateChecked(bool, Result<Option<String>, String>),
    SelfUpdated(Result<(), String>),
    Redraw,
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
    settings: settings::Form,
}

impl App {
    pub fn new(services: Services) -> (Self, Task<Message>) {
        let mode = services.theme_name().map_or(Mode::Dark, style::mode_of);
        let form = new_instance::Form::new(services.host);
        let settings = settings::Form {
            token: services.token().unwrap_or_default().to_owned(),
            font: services.font_name().map(str::to_owned),
            fonts: Vec::new(),
            update: settings::UpdateState::Checking,
            dns: services
                .dns_servers()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        };
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
            settings,
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
                Task::none()
            }
            Message::VersionsLoaded(kind, result) => {
                if kind == self.form.kind {
                    self.form.finish_load(result);
                    self.refilter();
                }
                Task::none()
            }
            Message::CatalogLoaded(id, result) => {
                if let Some(state) = self.edit.as_mut().filter(|state| state.id == id) {
                    state.catalog = match result {
                        Ok(packages) => Catalog::Ready(packages),
                        Err(reason) => Catalog::Failed(reason),
                    };
                }
                Task::none()
            }
            Message::Planned(id, result) => self.planned(id, result),
            Message::Updated(result) => match result {
                Ok(()) => self.reload(),
                Err(reason) => {
                    self.notice = Some(reason);
                    Task::none()
                }
            },
            Message::Created(id) => {
                if id.is_some() {
                    self.selected = id;
                }
                self.reload()
            }
            Message::Finished => self.reload(),
            Message::Browsed(id, package, result) => match result {
                Ok(Some(browsed)) => self.start_download(id, *package, browsed),
                Ok(None) => Task::none(),
                Err(reason) => {
                    self.notice = Some(reason);
                    Task::none()
                }
            },
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
            Message::Redraw => Task::none(),
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
            None => style::label(&self.theme, strings::NO_SELECTION).into(),
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
                style::label(&self.theme, notice.to_owned()).size(12),
            )
            .width(Length::Fill),
            content
        ]
        .spacing(style::GAP)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.services.board.has_active() {
            iced::time::every(REDRAW_INTERVAL).map(|_| Message::Redraw)
        } else {
            Subscription::none()
        }
    }

    fn rail(&self, board: &[TaskInfo]) -> Element<'_, Message> {
        let active = board.iter().filter(|task| task.state.is_active()).count();
        let items = vec![
            nav_item(ICON_INSTANCES, strings::TAB_INSTANCES, None, Screen::Main),
            nav_item(ICON_NEW, strings::TAB_NEW, None, Screen::NewInstance),
            nav_item(
                ICON_TASKS,
                strings::TAB_TASKS,
                (active > 0).then(|| Badge {
                    label: Some(active.to_string().into()),
                }),
                Screen::Tasks,
            ),
            nav_item(
                ICON_SETTINGS,
                strings::TAB_SETTINGS,
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
            self.edit = None;
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
                self.edit = Some(edit::State::new(instance));
                self.screen = Screen::Edit;
                self.notice = None;
                Task::none()
            }
            M::Duplicate => {
                let Some(instance) = self.selected_instance() else {
                    return Task::none();
                };
                let id = instance.config.id;
                let name = format!("{}{}", instance.config.name, strings::COPY_SUFFIX);
                let handle = self.services.board.add(format!(
                    "{}: {}",
                    strings::TASK_DUPLICATE,
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
                let title = format!("{}: {}", strings::TASK_LAUNCH, instance.config.name);
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
                let id = instance.config.id;
                if self.pending_delete != Some(id) {
                    return Task::none();
                }
                let title = format!("{}: {}", strings::TASK_DELETE, instance.config.name);
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
            M::VersionSelected(choice) => {
                self.form.selected = Some(choice);
                Task::none()
            }
            M::AddonSelected(choice) => {
                self.form.addon_selected = Some(choice);
                Task::none()
            }
            M::Reload => {
                self.form.begin_load();
                self.load_versions()
            }
            M::Submit => {
                let Some(request) = self.form.request() else {
                    return Task::none();
                };
                let title = format!("{}: {}", strings::TASK_CREATE, request.name);
                let handle = self.services.board.add(title);
                self.form = new_instance::Form::new(self.services.host);
                self.screen = Screen::Main;
                Task::perform(
                    self.services.clone().create(handle, request),
                    Message::Created,
                )
            }
            M::Cancel => {
                self.screen = Screen::Main;
                Task::none()
            }
        }
    }

    fn update_edit(&mut self, message: edit::Message) -> Task<Message> {
        use edit::Message as M;
        let Some(state) = self.edit.as_mut() else {
            return Task::none();
        };
        let id = state.id;
        match message {
            M::Back => {
                self.edit = None;
                self.screen = Screen::Main;
                Task::none()
            }
            M::TabSelected(tab) => {
                state.tab = tab;
                if tab == Tab::Browse && state.needs_catalog() {
                    state.catalog = Catalog::Loading;
                    return self.load_catalog(false);
                }
                Task::none()
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
                state.scroll_y = offset;
                state.viewport_height = height;
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
                let url = package.page_url.clone();
                self.browse(id, package, url)
            }
            M::ConfirmPending => {
                let Some(Pending { package, plan }) = state.pending.take() else {
                    return Task::none();
                };
                self.start_install(id, package, plan)
            }
            M::CancelPending => {
                state.pending = None;
                Task::none()
            }
            M::Remove(plugin_id) => {
                let title = format!("{}: {plugin_id}", strings::TASK_REMOVE);
                let handle = self.services.board.add(title);
                Task::perform(
                    self.services.clone().remove_plugin(handle, id, plugin_id),
                    |()| Message::Finished,
                )
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
                    self.notice = Some(strings::DNS_INVALID.to_owned());
                    return Task::none();
                };
                self.notice = Some(strings::DNS_RESTART.to_owned());
                Task::perform(self.services.clone().save_dns(servers), |result| {
                    Message::Saved(result.map_err(|error| error.to_string()))
                })
            }
            M::ThemeToggled => self.update(Message::ThemeToggled),
            M::FontSelected(name) => {
                self.settings.font.clone_from(&name);
                self.notice = Some(strings::FONT_RESTART.to_owned());
                Task::perform(self.services.clone().save_font(name), |result| {
                    Message::Saved(result.map_err(|error| error.to_string()))
                })
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
                let title = format!("{}: v{version}", strings::TASK_SELF_UPDATE);
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
            InstallPlan::OpenInBrowser(url) => self.browse(id, package, url),
            plan if plan.is_verified() => self.start_install(id, package, plan),
            plan => {
                if let Some(state) = self.edit.as_mut().filter(|state| state.id == id) {
                    state.pending = Some(Pending { package, plan });
                }
                Task::none()
            }
        }
    }

    fn start_install(&self, id: InstanceId, package: Package, plan: InstallPlan) -> Task<Message> {
        let title = format!("{}: {}", strings::TASK_INSTALL, package.name);
        let handle = self.services.board.add(title);
        Task::perform(
            self.services
                .clone()
                .install_plugin(handle, id, package, plan),
            |()| Message::Finished,
        )
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
        let title = format!("{}: {}", strings::TASK_DOWNLOAD, browsed.label());
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
}

fn nav_item(
    glyph: &'static str,
    label: &'static str,
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
        label: label.into(),
        on_press: Message::TabSelected(screen),
    }
}
