use std::time::Duration;

use iced::widget::{column, container};
use iced::{Element, Length, Subscription, Task, Theme};
use iced_aw::{TabBar, TabLabel};
use launcher_core::{AppSelection, AppVersion, Instance, InstanceId, Kind, TaskInfo};

use crate::ops::{Loaded, Services};
use crate::screen::{main_screen, menu_bar, new_instance, tasks};
use crate::{strings, style};

const REDRAW_INTERVAL: Duration = Duration::from_millis(250);
const TAB_WIDTH: f32 = 160.0;

#[derive(Debug, Clone)]
pub enum Message {
    Main(main_screen::Message),
    New(new_instance::Message),
    TabSelected(Screen),
    ThemeSelected(Theme),
    Saved(Result<(), String>),
    Loaded(Result<Loaded, String>),
    VersionsLoaded(Kind, Result<Vec<AppVersion>, String>),
    Created(Option<InstanceId>),
    Finished,
    Opened(Result<(), String>),
    Redraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
    NewInstance,
    Tasks,
}

pub struct App {
    services: Services,
    theme: Theme,
    screen: Screen,
    instances: Vec<Instance>,
    selected: Option<InstanceId>,
    pending_delete: Option<InstanceId>,
    notice: Option<String>,
    form: new_instance::Form,
}

impl App {
    pub fn new(services: Services) -> (Self, Task<Message>) {
        let theme = services
            .theme_name()
            .and_then(style::theme_by_name)
            .unwrap_or(Theme::Dark);
        let form = new_instance::Form::new(services.host);
        let app = Self {
            services,
            theme,
            screen: Screen::Main,
            instances: Vec::new(),
            selected: None,
            pending_delete: None,
            notice: None,
            form,
        };
        let load = app.reload();
        (app, load)
    }

    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Main(message) => self.update_main(message),
            Message::New(message) => self.update_new(message),
            Message::TabSelected(screen) => self.select_screen(screen),
            Message::ThemeSelected(theme) => {
                let name = theme.to_string();
                self.theme = theme;
                Task::perform(self.services.clone().save_theme(name), |result| {
                    Message::Saved(result.map_err(|error| error.to_string()))
                })
            }
            Message::Saved(result) => {
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
            Message::Created(id) => {
                if id.is_some() {
                    self.selected = id;
                }
                self.reload()
            }
            Message::Finished => self.reload(),
            Message::Opened(result) => {
                if let Err(reason) = result {
                    self.notice = Some(reason);
                }
                Task::none()
            }
            Message::Redraw => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let board = self.services.board.snapshot();
        let content: Element<'_, Message> = match self.screen {
            Screen::Main => main_screen::view(&main_screen::Props {
                instances: &self.instances,
                selected: self.selected,
                pending_delete: self.pending_delete,
                notice: self.notice.as_deref(),
                tasks: &board,
            })
            .map(Message::Main),
            Screen::NewInstance => {
                new_instance::view(&self.form, self.services.host).map(Message::New)
            }
            Screen::Tasks => tasks::view(&board),
        };
        column![
            menu_bar::view(&self.theme).map(Message::ThemeSelected),
            self.tab_bar(&board),
            container(content)
                .padding(style::PADDING)
                .width(Length::Fill)
                .height(Length::Fill),
        ]
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.services.board.has_active() {
            iced::time::every(REDRAW_INTERVAL).map(|_| Message::Redraw)
        } else {
            Subscription::none()
        }
    }

    fn tab_bar(&self, board: &[TaskInfo]) -> Element<'static, Message> {
        let active = board.iter().filter(|task| task.state.is_active()).count();
        let tasks_label = if active > 0 {
            format!("{} ({active})", strings::TAB_TASKS)
        } else {
            strings::TAB_TASKS.to_owned()
        };
        TabBar::new(Message::TabSelected)
            .push(
                Screen::Main,
                TabLabel::Text(strings::TAB_INSTANCES.to_owned()),
            )
            .push(
                Screen::NewInstance,
                TabLabel::Text(strings::TAB_NEW.to_owned()),
            )
            .push(Screen::Tasks, TabLabel::Text(tasks_label))
            .set_active_tab(&self.screen)
            .tab_width(Length::Fixed(TAB_WIDTH))
            .into()
    }

    fn select_screen(&mut self, screen: Screen) -> Task<Message> {
        self.screen = screen;
        if screen == Screen::NewInstance && self.form.needs_load() {
            self.form.begin_load();
            return self.load_versions();
        }
        Task::none()
    }

    fn update_main(&mut self, message: main_screen::Message) -> Task<Message> {
        use main_screen::Message as M;
        match message {
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
            }
            Err(reason) => self.notice = Some(reason),
        }
    }

    fn refilter(&mut self) {
        let selection = AppSelection {
            host: self.services.host,
            edition: self.form.edition,
        };
        let installable =
            self.services
                .installable(self.form.kind, selection, self.form.versions());
        self.form.set_choices(&installable);
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
