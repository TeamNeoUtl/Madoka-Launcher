use iced::widget::text::IntoFragment;
use iced::widget::{Text, container, scrollable, text};
use iced::{Color, Element, Font, Padding, Theme as IcedTheme};
use iced_m3::style::StateLayer;
use iced_m3::theme::{Accent, ColorScheme, Mode, Theme};
use iced_m3::widget::hybrid_icon::Icon;
use iced_m3::widget::{self, button, card};
use launcher_core::Kind;

pub const GAP: f32 = 12.0;
pub const PADDING: f32 = 16.0;
/// Font of the family `name`, or the iced default.
/// The name is leaked once per process: `Font` holds `&'static str` and the app sets one font at start.
pub fn font_of(name: Option<&str>) -> Font {
    name.map_or(Font::DEFAULT, |name| {
        Font::with_name(Box::leak(name.to_owned().into_boxed_str()))
    })
}
pub const ICON_FONT: Font = Font::with_name("Material Symbols Rounded");
pub const ICON_FONT_BYTES: &[u8] =
    include_bytes!("../assets/fonts/MaterialSymbolsRounded-Regular.ttf");

pub mod icons {
    pub const INSTANCES: &str = "\u{e5c3}"; // apps / grid
    pub const ADD: &str = "\u{e145}"; // add
    pub const TASKS: &str = "\u{e6b1}"; // task
    pub const SETTINGS: &str = "\u{e8b8}"; // settings
    pub const LAUNCH: &str = "\u{e037}"; // play_arrow
    pub const FOLDER_OPEN: &str = "\u{e2c8}"; // folder_open
    pub const FOLDER: &str = "\u{e2c7}"; // folder
    pub const EDIT: &str = "\u{e3c9}"; // edit
    pub const DUPLICATE: &str = "\u{e14d}"; // content_copy
    pub const DELETE: &str = "\u{e872}"; // delete
    pub const REFRESH: &str = "\u{e5d5}"; // refresh
    pub const CLOSE: &str = "\u{e5cd}"; // close
    pub const BACK: &str = "\u{e5c4}"; // arrow_back
    pub const CHECK: &str = "\u{e5ca}"; // check
    pub const CHECK_CIRCLE: &str = "\u{e86c}"; // check_circle
    pub const CANCEL: &str = "\u{e5c9}"; // cancel
    pub const DOWNLOAD: &str = "\u{e2c4}"; // file_download
    pub const SEARCH: &str = "\u{e8b6}"; // search
    pub const DARK_MODE: &str = "\u{e51c}"; // dark_mode
    pub const LIGHT_MODE: &str = "\u{e518}"; // light_mode
    pub const FONT: &str = "\u{e167}"; // font_download
    pub const KEY: &str = "\u{e73c}"; // vpn_key
    pub const NETWORK: &str = "\u{eb2f}"; // lan / network
    pub const INFO: &str = "\u{e88e}"; // info
    pub const WARNING: &str = "\u{e002}"; // warning
    pub const ERROR: &str = "\u{e000}"; // error
    pub const EXTENSION: &str = "\u{e87b}"; // extension (plugin)
    pub const CATEGORY: &str = "\u{e574}"; // category
    pub const SAVE: &str = "\u{e161}"; // save
    pub const UPDATE: &str = "\u{e923}"; // update
    pub const OPEN_IN_NEW: &str = "\u{e89e}"; // open_in_new
    pub const SYNC: &str = "\u{e627}"; // sync
    pub const HOURGLASS: &str = "\u{e85d}"; // hourglass_top
    pub const LANGUAGE: &str = "\u{e894}"; // language
    pub const MOVIE: &str = "\u{e02c}"; // movie
    pub const VIDEOCAM: &str = "\u{e30a}"; // videocam
    pub const LAYERS: &str = "\u{e865}"; // layers
    pub const PLAY_CIRCLE: &str = "\u{e038}"; // play_circle
    pub const GROUP: &str = "\u{e7ef}"; // group
    pub const HISTORY: &str = "\u{e897}"; // history / version
    pub const TUNE: &str = "\u{e1c4}"; // tune / settings
    pub const SHARE: &str = "\u{e80d}"; // share
    pub const COPY: &str = "\u{e14d}"; // content_copy
    pub const LINK: &str = "\u{e157}"; // link
    pub const TAG: &str = "\u{e54e}"; // label / tag
    pub const SORT: &str = "\u{e164}"; // sort
    pub const FILTER: &str = "\u{ef4f}"; // filter_list
    pub const CODE: &str = "\u{e86f}"; // code / json
    pub const PAUSE: &str = "\u{e034}"; // pause
    pub const PLAY: &str = "\u{e037}"; // play
    pub const EXPORT: &str = "\u{e2c6}"; // file_upload / export
    pub const IMPORT: &str = "\u{e2c4}"; // file_download / import
    pub const IMAGE: &str = "\u{e3f4}"; // image
    pub const DESCRIPTION: &str = "\u{e873}"; // description
    pub const PERSON: &str = "\u{e7fd}"; // person / author
    pub const POLICY: &str = "\u{e899}"; // policy / license
}

pub fn button_icon(glyph: &'static str) -> Icon<'static> {
    Icon::Text {
        text: glyph.into(),
        font: Some(ICON_FONT),
    }
}

pub fn icon<'a>(glyph: &'static str) -> Text<'a> {
    text(glyph).font(ICON_FONT)
}

pub fn btn_content<'a>(
    glyph: &'static str,
    label: impl Into<std::borrow::Cow<'a, str>>,
) -> button::Content<'a> {
    button::Content::Full {
        icon: button_icon(glyph),
        label: label.into(),
    }
}

pub const fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Aviutl | Kind::Aviutl2 => icons::MOVIE,
        Kind::Ymm4 => icons::VIDEOCAM,
        Kind::Neoutl => icons::LAYERS,
        Kind::Freya => icons::PLAY_CIRCLE,
    }
}

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

/// Theme of iced itself. It colors the widgets that `iced_m3` does not replace.
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

/// Non-interactive card that shows an icon and short status text.
pub fn icon_chip<M: Clone + 'static>(
    theme: &Theme,
    glyph: &'static str,
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
        iced::widget::row![
            icon(glyph).size(14).color(content_color),
            text(content.into())
                .size(CHIP_TEXT_SIZE)
                .color(content_color),
        ]
        .spacing(4)
        .align_y(iced::alignment::Vertical::Center),
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
    theme: &Theme,
) -> impl Fn(&IcedTheme, scrollable::Status) -> scrollable::Style + use<> {
    let theme = *theme;
    move |_, status| iced_m3::style::scrollable(status, &theme)
}
