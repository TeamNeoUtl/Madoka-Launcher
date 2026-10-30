use std::collections::BTreeMap;

use iced::alignment::Vertical;
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Length};
use iced_m3::theme::{Accent, Theme};
use iced_m3::widget::{self as m3, OnPress, button, card};
use launcher_core::{Instance, InstanceId, Kind, TaskInfo};

use super::tasks::detail_label;
use crate::strings;
use crate::style;

type Ui = Element<'static, Message>;

const COLUMNS: usize = 3;
const TILE_WIDTH: f32 = 210.0;
const SIDEBAR_WIDTH: f32 = 280.0;
const TILE_PADDING: f32 = 14.0;

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
    pub theme: &'a Theme,
    pub instances: &'a [Instance],
    pub selected: Option<InstanceId>,
    pub pending_delete: Option<InstanceId>,
    pub notice: Option<&'a str>,
    pub tasks: &'a [TaskInfo],
}

pub fn view(props: &Props<'_>) -> Ui {
    let theme = props.theme;
    let header = row![
        style::label(theme, strings::TAB_INSTANCES)
            .size(24)
            .width(Length::Fill),
        compact(
            strings::REFRESH,
            button::Style::tonal(theme, Accent::Primary),
            Message::Refresh
        ),
        compact(
            strings::ADD_INSTANCE,
            button::Style::filled(theme, Accent::Primary),
            Message::ShowNew
        ),
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
        scrollable(grid(theme, props.instances, props.selected))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::scroll(*theme)),
        sidebar(theme, selected, confirming),
    ]
    .spacing(style::GAP)
    .height(Length::Fill);

    let mut page = column![header].spacing(style::GAP);
    if let Some(notice) = props.notice {
        page = page.push(
            m3::card(
                card::Style::filled(theme),
                style::label(theme, notice.to_owned()).size(12),
            )
            .width(Length::Fill),
        );
    }
    page.push(body)
        .push(style::label(theme, status_line(props.tasks)).size(12))
        .into()
}

fn grid(theme: &Theme, instances: &[Instance], selected: Option<InstanceId>) -> Ui {
    if instances.is_empty() {
        return m3::card(
            card::Style::filled(theme),
            style::label(theme, strings::NO_INSTANCES),
        )
        .width(Length::Fill)
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
        .map(|(name, list)| section(theme, name, &list, selected))
        .collect();
    if !ungrouped.is_empty() {
        sections.push(section(theme, strings::UNGROUPED, &ungrouped, selected));
    }
    column(sections).spacing(style::GAP).into()
}

fn section(theme: &Theme, title: &str, list: &[&Instance], selected: Option<InstanceId>) -> Ui {
    let rows: Vec<Ui> = list
        .chunks(COLUMNS)
        .map(|chunk| {
            let tiles: Vec<Ui> = chunk
                .iter()
                .map(|instance| tile(theme, instance, selected == Some(instance.config.id)))
                .collect();
            row(tiles).spacing(style::GAP).into()
        })
        .collect();
    m3::card(
        card::Style::filled(theme),
        column![
            style::label(theme, title.to_owned()).size(18),
            column(rows).spacing(style::GAP)
        ]
        .spacing(style::GAP),
    )
    .width(Length::Fill)
    .into()
}

fn tile(theme: &Theme, instance: &Instance, selected: bool) -> Ui {
    let config = &instance.config;
    let color = style::tile_content(theme, selected);
    let content = column![
        style::chip::<Message>(
            theme,
            strings::kind_label(config.kind),
            style::kind_tone(config.kind)
        ),
        text(config.name.clone()).size(17).color(color),
        text(config.app_version.clone()).size(12).color(color),
    ]
    .spacing(6);
    m3::card(style::tile_card(theme, selected), content)
        .interaction(card::Interaction::Press(OnPress::Direct(Message::Select(
            config.id,
        ))))
        .padding(TILE_PADDING.into())
        .width(Length::Fixed(TILE_WIDTH))
        .into()
}

fn sidebar(theme: &Theme, instance: Option<&Instance>, confirming: bool) -> Ui {
    let content: Ui = match instance {
        None => style::label(theme, strings::NO_SELECTION).into(),
        Some(instance) => details(theme, instance, confirming),
    };
    m3::card(card::Style::filled(theme), content)
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Length::Fill)
        .into()
}

fn details(theme: &Theme, instance: &Instance, confirming: bool) -> Ui {
    let config = &instance.config;
    let mut lines = column![
        style::label(theme, config.name.clone()).size(20),
        style::chip::<Message>(
            theme,
            strings::kind_label(config.kind),
            style::kind_tone(config.kind)
        ),
    ]
    .spacing(8);
    if config.kind == Kind::Ymm4 {
        lines = lines.push(field(
            theme,
            strings::EDITION,
            strings::edition_label(config.edition),
        ));
    }
    lines = lines
        .push(field(theme, strings::VERSION, &config.app_version))
        .push(field(
            theme,
            strings::GROUP,
            config.group.as_deref().unwrap_or(strings::UNGROUPED),
        ))
        .push(field(
            theme,
            strings::PLUGINS,
            &config.plugins.len().to_string(),
        ));

    let actions: Vec<Ui> = if confirming {
        vec![
            action(
                strings::DELETE_CONFIRM,
                style::danger_button(theme),
                Message::ConfirmDelete,
            ),
            action(
                strings::CANCEL,
                button::Style::tonal(theme, Accent::Primary),
                Message::CancelDelete,
            ),
        ]
    } else {
        vec![
            action(
                strings::LAUNCH,
                button::Style::filled(theme, Accent::Primary),
                Message::Launch,
            ),
            action(
                strings::OPEN_FOLDER,
                button::Style::tonal(theme, Accent::Primary),
                Message::OpenFolder,
            ),
            action(
                strings::DELETE,
                style::danger_button(theme),
                Message::Delete,
            ),
        ]
    };
    column![lines, column(actions).spacing(8)]
        .spacing(style::PADDING)
        .into()
}

fn field(theme: &Theme, label: &str, value: &str) -> Ui {
    style::label(theme, format!("{label}: {value}"))
        .size(14)
        .into()
}

fn action(label: &'static str, style: button::Style, message: Message) -> Ui {
    m3::button(style, button::Content::Label(label.into()))
        .size(button::Size::default().width(Length::Fill))
        .on_press(message)
        .into()
}

fn compact(label: &'static str, style: button::Style, message: Message) -> Ui {
    m3::button(style, button::Content::Label(label.into()))
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
