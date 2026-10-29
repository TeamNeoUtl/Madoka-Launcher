use std::collections::BTreeMap;

use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Theme};
use launcher_core::{Instance, InstanceId, Kind, TaskInfo};

use super::tasks::detail_label;
use crate::strings;
use crate::style;

type Ui = Element<'static, Message>;
type ButtonStyle = fn(&Theme, button::Status) -> button::Style;

const COLUMNS: usize = 3;
const TILE_WIDTH: f32 = 210.0;
const SIDEBAR_WIDTH: f32 = 280.0;
const BUTTON_PADDING: [u16; 2] = [8, 16];

#[derive(Debug, Clone)]
pub enum Message {
    Select(InstanceId),
    ShowNew,
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
    let header = row![
        text(strings::TAB_INSTANCES).size(24).width(Length::Fill),
        button(text(strings::REFRESH))
            .padding(BUTTON_PADDING)
            .style(style::secondary_button)
            .on_press(Message::Refresh),
        button(text(strings::ADD_INSTANCE))
            .padding(BUTTON_PADDING)
            .style(style::primary_button)
            .on_press(Message::ShowNew),
    ]
    .spacing(8)
    .align_y(Vertical::Center);

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
    .spacing(style::GAP)
    .height(Length::Fill);

    let mut page = column![header].spacing(style::GAP);
    if let Some(notice) = props.notice {
        page = page.push(
            container(text(notice.to_owned()).size(12))
                .padding(style::GAP)
                .width(Length::Fill)
                .style(style::card),
        );
    }
    page.push(body)
        .push(text(status_line(props.tasks)).size(12))
        .into()
}

fn grid(instances: &[Instance], selected: Option<InstanceId>) -> Ui {
    if instances.is_empty() {
        return container(text(strings::NO_INSTANCES))
            .padding(style::PADDING)
            .width(Length::Fill)
            .style(style::card)
            .into();
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
    column(sections).spacing(style::GAP).into()
}

fn section(title: &str, list: &[&Instance], selected: Option<InstanceId>) -> Ui {
    let rows: Vec<Ui> = list
        .chunks(COLUMNS)
        .map(|chunk| {
            let tiles: Vec<Ui> = chunk
                .iter()
                .map(|instance| tile(instance, selected == Some(instance.config.id)))
                .collect();
            row(tiles).spacing(style::GAP).into()
        })
        .collect();
    container(
        column![
            text(title.to_owned()).size(18),
            column(rows).spacing(style::GAP)
        ]
        .spacing(style::GAP),
    )
    .padding(style::PADDING)
    .width(Length::Fill)
    .style(style::card)
    .into()
}

fn tile(instance: &Instance, selected: bool) -> Ui {
    let config = &instance.config;
    let content = column![
        style::badge_label::<Message>(
            strings::kind_label(config.kind),
            style::kind_tone(config.kind)
        ),
        text(config.name.clone()).size(17),
        text(config.app_version.clone()).size(12),
    ]
    .spacing(6);
    button(content)
        .width(Length::Fixed(TILE_WIDTH))
        .padding(14)
        .style(style::tile(selected))
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
        .height(Length::Fill)
        .padding(style::PADDING)
        .style(style::card)
        .into()
}

fn details(instance: &Instance, confirming: bool) -> Ui {
    let config = &instance.config;
    let mut lines = column![
        text(config.name.clone()).size(20),
        style::badge_label::<Message>(
            strings::kind_label(config.kind),
            style::kind_tone(config.kind)
        ),
    ]
    .spacing(8);
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
                style::danger_button,
                Message::ConfirmDelete,
            ),
            action(
                strings::CANCEL,
                style::secondary_button,
                Message::CancelDelete,
            ),
        ]
    } else {
        vec![
            action(strings::LAUNCH, style::primary_button, Message::Launch),
            action(
                strings::OPEN_FOLDER,
                style::secondary_button,
                Message::OpenFolder,
            ),
            action(strings::DELETE, style::danger_button, Message::Delete),
        ]
    };
    column![lines, column(actions).spacing(8)]
        .spacing(style::PADDING)
        .into()
}

fn field(label: &str, value: &str) -> Ui {
    text(format!("{label}: {value}")).size(14).into()
}

fn action(label: &'static str, style: ButtonStyle, message: Message) -> Ui {
    button(text(label))
        .width(Length::Fill)
        .padding(BUTTON_PADDING)
        .style(style)
        .on_press(message)
        .into()
}

fn status_line(tasks: &[TaskInfo]) -> String {
    let active: Vec<&TaskInfo> = tasks.iter().filter(|task| task.state.is_active()).collect();
    if let Some(latest) = active.last() {
        return format!(
            "{}: {} | {}: {}",
            strings::ACTIVE_TASKS,
            active.len(),
            latest.title,
            detail_label(&latest.state)
        );
    }
    tasks.last().map_or_else(String::new, |task| {
        format!("{}: {}", task.title, detail_label(&task.state))
    })
}
