use std::collections::BTreeMap;

use iced::widget::{button, container, pick_list, progress_bar, text, text_input};
use iced::{Background, Border, Color, Element, Theme};
use iced_aw::Badge;
use iced_aw::style::status::Status;
use iced_aw::style::{badge, sidebar};
use launcher_core::Kind;

pub const RADIUS: f32 = 16.0;
pub const MENU_RADIUS: f32 = 8.0;
pub const GAP: f32 = 12.0;
pub const PADDING: f32 = 16.0;

const THEME_FAMILIES: [&str; 7] = [
    "標準",
    "Solarized",
    "Gruvbox",
    "Catppuccin",
    "Tokyo Night",
    "Kanagawa",
    "その他",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Primary,
    Secondary,
    Success,
    Warning,
    Danger,
}

pub const fn kind_tone(kind: Kind) -> Tone {
    match kind {
        Kind::Ymm4 => Tone::Warning,
        Kind::Neoutl => Tone::Primary,
        Kind::Freya => Tone::Success,
        Kind::Aviutl | Kind::Aviutl2 => Tone::Secondary,
    }
}

pub fn badge_label<M: Clone + 'static>(
    label: impl Into<String>,
    tone: Tone,
) -> Element<'static, M> {
    let content = text(label.into()).size(12);
    match tone {
        Tone::Primary => Badge::new(content).style(badge::primary).into(),
        Tone::Secondary => Badge::new(content).style(badge::secondary).into(),
        Tone::Success => Badge::new(content).style(badge::success).into(),
        Tone::Warning => Badge::new(content).style(badge::warning).into(),
        Tone::Danger => Badge::new(content).style(badge::danger).into(),
    }
}

pub fn card(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(Background::Color(palette.background.weak.color)),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: RADIUS.into(),
        },
        ..container::Style::default()
    }
}

fn rounded(mut style: button::Style, radius: f32) -> button::Style {
    style.border = style.border.rounded(radius);
    style
}

pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    rounded(button::primary(theme, status), RADIUS)
}

pub fn secondary_button(theme: &Theme, status: button::Status) -> button::Style {
    rounded(button::secondary(theme, status), RADIUS)
}

pub fn danger_button(theme: &Theme, status: button::Status) -> button::Style {
    rounded(button::danger(theme, status), RADIUS)
}

/// A card-shaped button. The selected one uses the primary color.
pub fn tile(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        if selected {
            primary_button(theme, status)
        } else {
            secondary_button(theme, status)
        }
    }
}

pub fn menu_item(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let base = if selected {
            button::primary(theme, status)
        } else {
            button::text(theme, status)
        };
        rounded(base, MENU_RADIUS)
    }
}

/// Style of the tabs of `SidebarWithContent`. The selected tab uses the primary color and the
/// others follow the card color. `Disabled` is the status that a tab which is not selected gets.
pub fn sidebar_tab(theme: &Theme, status: Status) -> sidebar::Style {
    let palette = theme.extended_palette();
    let pair = match status {
        Status::Active => palette.primary.base,
        Status::Hovered => palette.background.strong,
        _ => palette.background.weak,
    };
    sidebar::Style {
        tab_label_background: Background::Color(pair.color),
        tab_label_border_color: Color::TRANSPARENT,
        tab_label_border_width: 0.0,
        icon_color: pair.text,
        text_color: pair.text,
        ..sidebar::Style::default()
    }
}

pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.border = style.border.rounded(RADIUS);
    style
}

pub fn dropdown(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let mut style = pick_list::default(theme, status);
    style.border = style.border.rounded(RADIUS);
    style
}

pub fn bar(theme: &Theme) -> progress_bar::Style {
    let mut style = progress_bar::primary(theme);
    style.border = style.border.rounded(RADIUS);
    style
}

/// Names of every built-in theme, grouped by family. Unknown themes fall into the last group.
pub fn theme_groups() -> Vec<(&'static str, Vec<Theme>)> {
    let mut groups: BTreeMap<usize, Vec<Theme>> = BTreeMap::new();
    for theme in Theme::ALL {
        groups
            .entry(family(&theme.to_string()))
            .or_default()
            .push(theme.clone());
    }
    groups
        .into_iter()
        .map(|(index, themes)| (THEME_FAMILIES[index], themes))
        .collect()
}

pub fn theme_by_name(name: &str) -> Option<Theme> {
    Theme::ALL
        .iter()
        .find(|theme| theme.to_string() == name)
        .cloned()
}

fn family(name: &str) -> usize {
    const PREFIXES: [&str; 5] = [
        "Solarized",
        "Gruvbox",
        "Catppuccin",
        "Tokyo Night",
        "Kanagawa",
    ];
    if name == "Light" || name == "Dark" {
        return 0;
    }
    PREFIXES
        .iter()
        .position(|prefix| name.starts_with(prefix))
        .map_or(THEME_FAMILIES.len() - 1, |index| index + 1)
}
