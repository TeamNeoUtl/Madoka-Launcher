use iced::widget::text::IntoFragment;
use iced::widget::{Text, container, scrollable, text};
use iced::{Color, Element, Font, Padding, Theme as IcedTheme};
use iced_m3::style::StateLayer;
use iced_m3::theme::{Accent, ColorScheme, Mode, Theme};
use iced_m3::widget::{self, button, card};
use launcher_core::Kind;

pub const GAP: f32 = 12.0;
pub const PADDING: f32 = 16.0;
pub const ICON_FONT: Font = Font::with_name("Material Symbols Rounded");
pub const ICON_FONT_BYTES: &[u8] =
    include_bytes!("../assets/fonts/MaterialSymbolsRounded-Regular.ttf");

const CHIP_PADDING: [f32; 2] = [4.0, 12.0];
const CHIP_TEXT_SIZE: f32 = 12.0;
const MODE_LIGHT: &str = "Light";
const MODE_DARK: &str = "Dark";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Primary,
    Secondary,
    Tertiary,
    Error,
}

pub const fn kind_tone(kind: Kind) -> Tone {
    match kind {
        Kind::Ymm4 => Tone::Tertiary,
        Kind::Neoutl => Tone::Primary,
        Kind::Freya | Kind::Aviutl | Kind::Aviutl2 => Tone::Secondary,
    }
}

/// Reads the value that `mode_name` writes to `config.toml`. Unknown names yield `Mode::Dark`.
pub fn mode_of(name: &str) -> Mode {
    if name == MODE_LIGHT {
        Mode::Light
    } else {
        Mode::Dark
    }
}

pub const fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Light => MODE_LIGHT,
        Mode::Dark => MODE_DARK,
    }
}

/// Theme of iced itself. It colors the widgets that iced_m3 does not replace.
pub const fn iced_theme(mode: Mode) -> IcedTheme {
    match mode {
        Mode::Light => IcedTheme::Light,
        Mode::Dark => IcedTheme::Dark,
    }
}

pub fn label<'a>(theme: &Theme, content: impl IntoFragment<'a>) -> Text<'a> {
    text(content).color(theme.on_surface())
}

/// Non-interactive card that shows a short status text.
pub fn chip<M: Clone + 'static>(
    theme: &Theme,
    content: impl Into<String>,
    tone: Tone,
) -> Element<'static, M> {
    let (container_color, content_color) = match tone {
        Tone::Primary => (theme.primary_container(), theme.on_primary_container()),
        Tone::Secondary => (theme.secondary_container(), theme.on_secondary_container()),
        Tone::Tertiary => (theme.tertiary_container(), theme.on_tertiary_container()),
        Tone::Error => (theme.error_container(), theme.on_error_container()),
    };
    let style = card::Style {
        container_color,
        ..card::Style::filled(theme)
    };
    widget::card(
        style,
        text(content.into())
            .size(CHIP_TEXT_SIZE)
            .color(content_color),
    )
    .padding(Padding::from(CHIP_PADDING))
    .into()
}

/// Card that is selectable: the selected one uses the primary container color.
pub fn tile_card(theme: &Theme, selected: bool) -> card::Style {
    if !selected {
        return card::Style::outlined(theme);
    }
    let base = card::Style::filled(theme);
    card::Style {
        container_color: theme.primary_container(),
        state_layer: StateLayer::new(theme.on_primary_container()),
        ..base
    }
}

pub fn tile_content(theme: &Theme, selected: bool) -> Color {
    if selected {
        theme.on_primary_container()
    } else {
        theme.on_surface()
    }
}

pub fn danger_button(theme: &Theme) -> button::Style {
    let state = button::StateStyle {
        container: theme.error(),
        label: theme.on_error(),
        icon: theme.on_error(),
        outline: button::Outline::default(),
    };
    button::Style {
        regular: state,
        unselected: state,
        selected: state,
        state_layer: StateLayer::new(theme.on_error()),
        state_layer_unselected: StateLayer::new(theme.on_error()),
        state_layer_selected: StateLayer::new(theme.on_error()),
        ..button::Style::filled(theme, Accent::Primary)
    }
}

pub fn page(theme: &Theme) -> impl Fn(&IcedTheme) -> container::Style + use<> {
    let background = theme.background();
    move |_| container::Style::default().background(background)
}

pub fn rail_background(theme: &Theme) -> impl Fn(&IcedTheme) -> container::Style + use<> {
    let background = theme.surface_container();
    move |_| container::Style::default().background(background)
}

pub fn scroll(
    theme: Theme,
) -> impl Fn(&IcedTheme, scrollable::Status) -> scrollable::Style + use<> {
    move |_, status| iced_m3::style::scrollable(status, &theme)
}
