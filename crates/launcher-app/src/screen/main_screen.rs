use std::collections::BTreeMap;

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Theme};
use launcher_core::{Instance, InstanceId, Kind, TaskInfo};

use super::tasks::state_label;
use crate::strings;

type Ui = Element<'static, Message>;
type ButtonStyle = fn(&Theme, button::Status) -> button::Style;

const COLUMNS: usize = 3;
const TILE_WIDTH: f32 = 200.0;
const SIDEBAR_WIDTH: f32 = 240.0;

#[derive(Debug, Clone)]
pub enum Message {
    Select(InstanceId),
    ShowNew,
    ShowTasks,
    Refresh,
    Launch,
    OpenFolder,
    Delete,
    ConfirmDelete,
    CancelDelete,
}

pub struct Props<'a> {
    pub instances: &'a [Instance],
    pub selected: Option<InstanceId>,
    pub pending_delete: Option<InstanceId>,
    pub notice: Option<&'a str>,
    pub tasks: &'a [TaskInfo],
}

pub fn view(props: &Props<'_>) -> Ui {
    let active = props
        .tasks
        .iter()
        .filter(|task| task.state.is_active())
        .count();
    let toolbar = row![
        button(text(strings::ADD_INSTANCE)).on_press(Message::ShowNew),
        button(text(strings::REFRESH)).on_press(Message::Refresh),
        button(text(format!("{} ({active})", strings::TASKS))).on_press(Message::ShowTasks),
    ]
    .spacing(8);

    let selected = props.selected.and_then(|id| {
        props
            .instances
            .iter()
            .find(|instance| instance.config.id == id)
    });
    let confirming = props.pending_delete.is_some() && props.pending_delete == props.selected;
    let body = row![
        scrollable(grid(props.instances, props.selected))
            .width(Length::Fill)
            .height(Length::Fill),
        sidebar(selected, confirming),
    ]
    .spacing(12)
    .height(Length::Fill);

    let mut page = column![toolbar].spacing(12).padding(12);
    if let Some(notice) = props.notice {
        page = page.push(text(notice.to_owned()).size(12));
    }
    page.push(body)
        .push(text(status_line(props.tasks)).size(12))
        .into()
}

fn grid(instances: &[Instance], selected: Option<InstanceId>) -> Ui {
    if instances.is_empty() {
        return text(strings::NO_INSTANCES).into();
    }
    let mut named: BTreeMap<&str, Vec<&Instance>> = BTreeMap::new();
    let mut ungrouped: Vec<&Instance> = Vec::new();
    for instance in instances {
        match instance.config.group.as_deref() {
            Some(group) => named.entry(group).or_default().push(instance),
            None => ungrouped.push(instance),
        }
    }
    let mut sections: Vec<Ui> = named
        .into_iter()
        .map(|(name, list)| section(name, &list, selected))
        .collect();
    if !ungrouped.is_empty() {
        sections.push(section(strings::UNGROUPED, &ungrouped, selected));
    }
    column(sections).spacing(20).into()
}

fn section(title: &str, list: &[&Instance], selected: Option<InstanceId>) -> Ui {
    let rows: Vec<Ui> = list
        .chunks(COLUMNS)
        .map(|chunk| {
            let tiles: Vec<Ui> = chunk
                .iter()
                .map(|instance| tile(instance, selected == Some(instance.config.id)))
                .collect();
            row(tiles).spacing(8).into()
        })
        .collect();
    column![text(title.to_owned()).size(18), column(rows).spacing(8)]
        .spacing(8)
        .into()
}

fn tile(instance: &Instance, selected: bool) -> Ui {
    let config = &instance.config;
    let label = column![
        text(config.name.clone()).size(16),
        text(format!(
            "{} {}",
            strings::kind_label(config.kind),
            config.app_version
        ))
        .size(12),
    ]
    .spacing(4);
    button(label)
        .width(Length::Fixed(TILE_WIDTH))
        .style(move |theme, status| {
            if selected {
                button::primary(theme, status)
            } else {
                button::secondary(theme, status)
            }
        })
        .on_press(Message::Select(config.id))
        .into()
}

fn sidebar(instance: Option<&Instance>, confirming: bool) -> Ui {
    let content: Ui = match instance {
        None => text(strings::NO_SELECTION).into(),
        Some(instance) => details(instance, confirming),
    };
    container(content)
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .padding(12)
        .into()
}

fn details(instance: &Instance, confirming: bool) -> Ui {
    let config = &instance.config;
    let mut lines = column![
        text(config.name.clone()).size(20),
        field(strings::KIND, strings::kind_label(config.kind)),
    ]
    .spacing(6);
    if config.kind == Kind::Ymm4 {
        lines = lines.push(field(
            strings::EDITION,
            strings::edition_label(config.edition),
        ));
    }
    lines = lines
        .push(field(strings::VERSION, &config.app_version))
        .push(field(
            strings::GROUP,
            config.group.as_deref().unwrap_or(strings::UNGROUPED),
        ))
        .push(field(strings::PLUGINS, &config.plugins.len().to_string()));

    let actions: Vec<Ui> = if confirming {
        vec![
            action(
                strings::DELETE_CONFIRM,
                button::danger,
                Message::ConfirmDelete,
            ),
            action(strings::CANCEL, button::secondary, Message::CancelDelete),
        ]
    } else {
        vec![
            action(strings::LAUNCH, button::primary, Message::Launch),
            action(strings::OPEN_FOLDER, button::secondary, Message::OpenFolder),
            action(strings::DELETE, button::danger, Message::Delete),
        ]
    };
    column![lines, column(actions).spacing(6)]
        .spacing(16)
        .into()
}

fn field(label: &str, value: &str) -> Ui {
    text(format!("{label}: {value}")).size(14).into()
}

fn action(label: &'static str, style: ButtonStyle, message: Message) -> Ui {
    button(text(label))
        .width(Length::Fill)
        .style(style)
        .on_press(message)
        .into()
}

fn status_line(tasks: &[TaskInfo]) -> String {
    let active = tasks.iter().filter(|task| task.state.is_active()).count();
    if active > 0 {
        return format!("{}: {active}", strings::ACTIVE_TASKS);
    }
    tasks.last().map_or_else(String::new, |task| {
        format!("{}: {}", task.title, state_label(&task.state))
    })
}
