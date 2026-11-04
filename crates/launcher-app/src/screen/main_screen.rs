use std::borrow::Cow;
use std::collections::BTreeMap;

use iced::alignment::Vertical;
use iced::widget::{Space, column, mouse_area, row, scrollable, text};
use iced::{Element, Length};
use iced_m3::theme::{Accent, ColorScheme, Theme};
use iced_m3::widget::{self as m3, Action, button, card};
use launcher_core::{Instance, InstanceId, Kind, TaskInfo};
use rust_i18n::t;

use super::tasks::detail_label;
use crate::strings;
use crate::style;

type Ui = Element<'static, Message>;

const COLUMNS: usize = 3;
const TILE_WIDTH: f32 = 230.0;
const SIDEBAR_WIDTH: f32 = 280.0;
const TILE_PADDING: f32 = 14.0;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Select(InstanceId),
    ShowNew,
    Refresh,
    Launch,
    LaunchInstance(InstanceId),
    OpenFolder,
    Edit,
    Duplicate,
    ContextOpen(InstanceId),
    ContextClose,
    Delete,
    ConfirmDelete,
    CancelDelete,
}

pub struct Props<'a> {
    pub theme: &'a Theme,
    pub instances: &'a [Instance],
    pub selected: Option<InstanceId>,
    pub pending_delete: Option<InstanceId>,
    pub context: Option<InstanceId>,
    pub notice: Option<&'a str>,
    pub tasks: &'a [TaskInfo],
}

pub fn view(props: &Props<'_>) -> Ui {
    let theme = props.theme;
    let header = row![
        row![
            style::icon(style::icons::INSTANCES)
                .size(26)
                .color(theme.primary()),
            style::label(theme, t!("tab_instances")).size(24),
            style::label(theme, format!("({})", props.instances.len())).size(16),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .width(Length::Fill),
        compact(
            style::icons::REFRESH,
            t!("refresh"),
            &button::Style::tonal(theme, Accent::Primary),
            Message::Refresh,
        ),
        compact(
            style::icons::ADD,
            t!("add_instance"),
            &button::Style::filled(theme, Accent::Primary),
            Message::ShowNew,
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
        scrollable(grid(theme, props.instances, props.selected, props.context))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::scroll(theme)),
        sidebar(theme, selected, confirming),
    ]
    .spacing(style::GAP)
    .height(Length::Fill);

    let mut page = column![header].spacing(style::GAP);
    if let Some(notice) = props.notice {
        page = page.push(
            m3::card(
                card::Style::filled(theme),
                row![
                    style::icon(style::icons::INFO)
                        .size(16)
                        .color(theme.on_surface()),
                    style::label(theme, notice.to_owned()).size(12),
                ]
                .spacing(8)
                .align_y(Vertical::Center),
            )
            .width(Length::Fill),
        );
    }
    page.push(body).push(status_bar(theme, props.tasks)).into()
}

fn grid(
    theme: &Theme,
    instances: &[Instance],
    selected: Option<InstanceId>,
    context: Option<InstanceId>,
) -> Ui {
    if instances.is_empty() {
        return m3::card(
            card::Style::filled(theme),
            row![
                style::icon(style::icons::INFO)
                    .size(20)
                    .color(theme.on_surface_variant()),
                style::label(theme, t!("no_instances")),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
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
        .map(|(name, list)| section(theme, name, &list, selected, context, false))
        .collect();
    if !ungrouped.is_empty() {
        sections.push(section(
            theme,
            &t!("ungrouped"),
            &ungrouped,
            selected,
            context,
            true,
        ));
    }
    column(sections).spacing(style::GAP).into()
}

fn section(
    theme: &Theme,
    title: &str,
    list: &[&Instance],
    selected: Option<InstanceId>,
    context: Option<InstanceId>,
    is_ungrouped: bool,
) -> Ui {
    let icon_glyph = if is_ungrouped {
        style::icons::FOLDER
    } else {
        style::icons::FOLDER_OPEN
    };
    let rows: Vec<Ui> = list
        .chunks(COLUMNS)
        .map(|chunk| {
            let tiles: Vec<Ui> = chunk
                .iter()
                .map(|instance| {
                    let id = instance.config.id;
                    if context == Some(id) {
                        context_menu(theme)
                    } else {
                        tile(theme, instance, selected == Some(id))
                    }
                })
                .collect();
            row(tiles).spacing(style::GAP).into()
        })
        .collect();
    m3::card(
        card::Style::filled(theme),
        column![
            row![
                style::icon(icon_glyph).size(18).color(theme.primary()),
                style::label(theme, title.to_owned()).size(18),
                style::label(theme, format!("({})", list.len())).size(14),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
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

    let top_row = row![
        style::icon_chip::<Message>(
            theme,
            style::kind_icon(config.kind),
            strings::kind_label(config.kind),
            style::kind_tone(config.kind),
        ),
        Space::new().width(Length::Fill),
        if !config.plugins.is_empty() {
            row![
                style::icon(style::icons::EXTENSION).size(14).color(color),
                text(config.plugins.len().to_string()).size(12).color(color),
            ]
            .spacing(3)
            .align_y(Vertical::Center)
        } else {
            row![]
        }
    ]
    .align_y(Vertical::Center);

    let info = column![
        text(config.name.clone()).size(17).color(color),
        row![
            style::icon(style::icons::HISTORY).size(13).color(color),
            text(config.version_label()).size(12).color(color),
        ]
        .spacing(4)
        .align_y(Vertical::Center),
    ]
    .spacing(4);

    let launch_btn = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        style::btn_content(style::icons::LAUNCH, t!("launch")),
    )
    .size(button::Size::default().width(Length::Fill))
    .on_press(Message::LaunchInstance(config.id));

    let content = column![top_row, info, launch_btn].spacing(10);
    let tile = m3::card(style::tile_card(theme, selected), content)
        .interaction(card::Interaction::Press(Action::Direct(Message::Select(
            config.id,
        ))))
        .padding(TILE_PADDING)
        .width(Length::Fixed(TILE_WIDTH));
    mouse_area(tile)
        .on_right_press(Message::ContextOpen(config.id))
        .into()
}

/// Replaces the tile of the selected instance. The entries act on the selected instance.
fn context_menu(theme: &Theme) -> Ui {
    let entries: Vec<Ui> = vec![
        action(
            style::icons::LAUNCH,
            t!("launch"),
            &button::Style::filled(theme, Accent::Primary),
            Message::Launch,
        ),
        action(
            style::icons::EDIT,
            t!("edit"),
            &button::Style::tonal(theme, Accent::Primary),
            Message::Edit,
        ),
        action(
            style::icons::FOLDER_OPEN,
            t!("open_folder"),
            &button::Style::tonal(theme, Accent::Primary),
            Message::OpenFolder,
        ),
        action(
            style::icons::DUPLICATE,
            t!("duplicate"),
            &button::Style::tonal(theme, Accent::Primary),
            Message::Duplicate,
        ),
        action(
            style::icons::DELETE,
            t!("delete"),
            &style::danger_button(theme),
            Message::Delete,
        ),
        action(
            style::icons::CLOSE,
            t!("close"),
            &button::Style::tonal(theme, Accent::Primary),
            Message::ContextClose,
        ),
    ];
    m3::card(card::Style::filled(theme), column(entries).spacing(4))
        .padding(TILE_PADDING)
        .width(Length::Fixed(TILE_WIDTH))
        .into()
}

fn sidebar(theme: &Theme, instance: Option<&Instance>, confirming: bool) -> Ui {
    let content: Ui = instance.map_or_else(
        || {
            column![
                style::icon(style::icons::INSTANCES)
                    .size(32)
                    .color(theme.on_surface_variant()),
                style::label(theme, t!("no_selection")).size(14),
            ]
            .spacing(8)
            .align_x(iced::alignment::Horizontal::Center)
            .into()
        },
        |instance| details(theme, instance, confirming),
    );
    m3::card(card::Style::filled(theme), content)
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Length::Fill)
        .into()
}

fn details(theme: &Theme, instance: &Instance, confirming: bool) -> Ui {
    let config = &instance.config;
    let mut lines = column![
        style::label(theme, config.name.clone()).size(20),
        style::icon_chip::<Message>(
            theme,
            style::kind_icon(config.kind),
            strings::kind_label(config.kind),
            style::kind_tone(config.kind),
        ),
    ]
    .spacing(8);

    if config.kind == Kind::Ymm4 {
        lines = lines.push(field_with_icon(
            theme,
            style::icons::LAYERS,
            &t!("edition"),
            &strings::edition_label(config.edition),
        ));
    }
    lines = lines
        .push(field_with_icon(
            theme,
            style::icons::HISTORY,
            &t!("version"),
            &config.version_label(),
        ))
        .push(field_with_icon(
            theme,
            style::icons::GROUP,
            &t!("group"),
            config.group.as_deref().unwrap_or(&t!("ungrouped")),
        ))
        .push(field_with_icon(
            theme,
            style::icons::EXTENSION,
            &t!("plugins"),
            &config.plugins.len().to_string(),
        ));

    let actions: Ui = if confirming {
        column![
            action(
                style::icons::DELETE,
                t!("delete_confirm"),
                &style::danger_button(theme),
                Message::ConfirmDelete,
            ),
            action(
                style::icons::CANCEL,
                t!("cancel"),
                &button::Style::tonal(theme, Accent::Primary),
                Message::CancelDelete,
            ),
        ]
        .spacing(8)
        .into()
    } else {
        column![
            // Primary prominent launch button at top
            action(
                style::icons::LAUNCH,
                t!("launch"),
                &button::Style::filled(theme, Accent::Primary),
                Message::Launch,
            ),
            Space::new().height(Length::Fixed(4.0)),
            // Management actions
            action(
                style::icons::EDIT,
                t!("edit"),
                &button::Style::tonal(theme, Accent::Primary),
                Message::Edit,
            ),
            action(
                style::icons::FOLDER_OPEN,
                t!("open_folder"),
                &button::Style::tonal(theme, Accent::Primary),
                Message::OpenFolder,
            ),
            action(
                style::icons::DUPLICATE,
                t!("duplicate"),
                &button::Style::tonal(theme, Accent::Primary),
                Message::Duplicate,
            ),
            Space::new().height(Length::Fixed(8.0)),
            // Dangerous action separated at bottom
            action(
                style::icons::DELETE,
                t!("delete"),
                &style::danger_button(theme),
                Message::Delete,
            ),
        ]
        .spacing(8)
        .into()
    };

    column![lines, Space::new().height(Length::Fill), actions]
        .spacing(style::PADDING)
        .height(Length::Fill)
        .into()
}

fn field_with_icon(theme: &Theme, glyph: &'static str, label: &str, value: &str) -> Ui {
    row![
        style::icon(glyph).size(16).color(theme.primary()),
        style::label(theme, format!("{label}: {value}")).size(14),
    ]
    .spacing(6)
    .align_y(Vertical::Center)
    .into()
}

fn action(
    glyph: &'static str,
    label: Cow<'static, str>,
    style: &button::Style,
    message: Message,
) -> Ui {
    m3::button(*style, style::btn_content(glyph, label))
        .size(button::Size::default().width(Length::Fill))
        .on_press(message)
        .into()
}

fn compact(
    glyph: &'static str,
    label: Cow<'static, str>,
    style: &button::Style,
    message: Message,
) -> Ui {
    m3::button(*style, style::btn_content(glyph, label))
        .on_press(message)
        .into()
}

fn status_bar(theme: &Theme, tasks: &[TaskInfo]) -> Ui {
    let active: Vec<&TaskInfo> = tasks.iter().filter(|task| task.state.is_active()).collect();
    if let Some(latest) = active.last() {
        row![
            style::icon(style::icons::SYNC)
                .size(15)
                .color(theme.primary()),
            style::label(
                theme,
                format!(
                    "{}: {} | {}: {}",
                    t!("active_tasks"),
                    active.len(),
                    latest.title,
                    detail_label(&latest.state)
                )
            )
            .size(12),
        ]
        .spacing(6)
        .align_y(Vertical::Center)
        .into()
    } else if let Some(last) = tasks.last() {
        row![
            style::icon(style::icons::TASKS)
                .size(15)
                .color(theme.on_surface_variant()),
            style::label(
                theme,
                format!("{}: {}", last.title, detail_label(&last.state))
            )
            .size(12),
        ]
        .spacing(6)
        .align_y(Vertical::Center)
        .into()
    } else {
        row![
            style::icon(style::icons::CHECK_CIRCLE)
                .size(15)
                .color(theme.tertiary()),
            style::label(theme, t!("no_tasks")).size(12),
        ]
        .spacing(6)
        .align_y(Vertical::Center)
        .into()
    }
}
