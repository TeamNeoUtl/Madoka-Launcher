use std::fmt;

use iced::alignment::Vertical;
use iced::widget::{checkbox, column, row, scrollable};
use iced::{Element, Length, Padding};
use iced_m3::theme::{Accent, ColorScheme, Theme};
use iced_m3::widget::button_group::connected;
use iced_m3::widget::text_input::TextInput;
use iced_m3::widget::{self as m3, Action, button, card};
use launcher_core::{AppVersion, Edition, Host, Kind};
use launcher_providers::{Versions, has_provider};

use crate::ops::CreateRequest;
use crate::strings;
use crate::style::{self, Tone};

const KIND_LIST_WIDTH: f32 = 300.0;
const VERSION_LIST_HEIGHT: f32 = 220.0;
const ENTRY_PADDING: [f32; 2] = [8.0, 16.0];
const INPUT_PADDING: f32 = 12.0;

pub const KINDS: [Kind; 5] = [
    Kind::Ymm4,
    Kind::Aviutl,
    Kind::Aviutl2,
    Kind::Neoutl,
    Kind::Freya,
];

#[derive(Debug, Clone)]
pub enum Message {
    KindSelected(Kind),
    NameChanged(String),
    GroupChanged(String),
    EditionSelected(Edition),
    VersionSelected(VersionChoice),
    AddonSelected(VersionChoice),
    ToggleRecommended(bool),
    Reload,
    Submit,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Available,
    UnsupportedHost,
    NoProvider,
}

pub const fn availability(kind: Kind, host: Host) -> Availability {
    if !kind.supported_on(host) {
        Availability::UnsupportedHost
    } else if !has_provider(kind) {
        Availability::NoProvider
    } else {
        Availability::Available
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionChoice {
    pub tag: String,
    label: String,
}

impl From<&AppVersion> for VersionChoice {
    fn from(version: &AppVersion) -> Self {
        let label = if version.prerelease {
            format!("{} ({})", version.version, strings::PRERELEASE)
        } else {
            version.version.clone()
        };
        Self {
            tag: version.version.clone(),
            label,
        }
    }
}

impl fmt::Display for VersionChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

pub enum Load {
    /// No request has been made for the selected kind.
    Idle,
    Loading,
    Ready,
    Failed(String),
}

pub struct Form {
    pub kind: Kind,
    pub name: String,
    pub group: String,
    pub edition: Edition,
    pub selected: Option<VersionChoice>,
    pub choices: Vec<VersionChoice>,
    pub addon_selected: Option<VersionChoice>,
    pub addon_choices: Vec<VersionChoice>,
    pub install_recommended: bool,
    pub load: Load,
    versions: Vec<AppVersion>,
    addon_versions: Vec<AppVersion>,
}

impl Form {
    pub fn new(host: Host) -> Self {
        let kind = KINDS
            .into_iter()
            .find(|kind| availability(*kind, host) == Availability::Available)
            .unwrap_or(Kind::Neoutl);
        Self {
            kind,
            name: String::new(),
            group: String::new(),
            edition: Edition::Normal,
            selected: None,
            choices: Vec::new(),
            addon_selected: None,
            addon_choices: Vec::new(),
            install_recommended: true,
            load: Load::Idle,
            versions: Vec::new(),
            addon_versions: Vec::new(),
        }
    }

    pub const fn needs_load(&self) -> bool {
        matches!(self.load, Load::Idle)
    }

    pub fn select_kind(&mut self, kind: Kind) {
        self.kind = kind;
        self.edition = Edition::Normal;
        self.begin_load();
    }

    pub fn begin_load(&mut self) {
        self.load = Load::Loading;
        self.versions.clear();
        self.addon_versions.clear();
        self.choices.clear();
        self.addon_choices.clear();
        self.selected = None;
        self.addon_selected = None;
    }

    pub fn finish_load(&mut self, result: Result<Versions, String>) {
        match result {
            Ok(versions) => {
                self.versions = versions.app;
                self.addon_versions = versions.addon;
                self.load = Load::Ready;
            }
            Err(reason) => {
                self.versions.clear();
                self.addon_versions.clear();
                self.load = Load::Failed(reason);
            }
        }
    }

    pub fn versions(&self) -> &[AppVersion] {
        &self.versions
    }

    pub fn addon_versions(&self) -> &[AppVersion] {
        &self.addon_versions
    }

    pub fn has_addon(&self) -> bool {
        !self.addon_versions.is_empty()
    }

    /// Replaces the choices with the installable versions and selects the first entry of each.
    pub fn set_choices(&mut self, installable: &[AppVersion], addons: &[AppVersion]) {
        self.choices = installable.iter().map(VersionChoice::from).collect();
        self.selected = self.choices.first().cloned();
        self.addon_choices = addons.iter().map(VersionChoice::from).collect();
        self.addon_selected = self.addon_choices.first().cloned();
    }

    pub fn can_submit(&self) -> bool {
        !self.name.trim().is_empty()
            && self.selected.is_some()
            && (!self.has_addon() || self.addon_selected.is_some())
    }

    pub fn request(&self) -> Option<CreateRequest> {
        if !self.can_submit() {
            return None;
        }
        let tag = &self.selected.as_ref()?.tag;
        let version = self
            .versions
            .iter()
            .find(|version| &version.version == tag)?
            .clone();
        let addon = match &self.addon_selected {
            Some(choice) => Some(
                self.addon_versions
                    .iter()
                    .find(|version| version.version == choice.tag)?
                    .clone(),
            ),
            None => None,
        };
        let group = self.group.trim();
        Some(CreateRequest {
            name: self.name.trim().to_owned(),
            kind: self.kind,
            edition: self.edition,
            group: (!group.is_empty()).then(|| group.to_owned()),
            version,
            addon,
        })
    }
}

pub fn view<'a>(form: &'a Form, theme: &'a Theme, host: Host) -> Element<'a, Message> {
    let kinds: Vec<Element<'a, Message>> = KINDS
        .iter()
        .map(|kind| kind_entry(theme, *kind, form.kind, host))
        .collect();
    let kind_card = m3::card(
        card::Style::filled(theme),
        column![
            row![
                style::icon(style::icons::CATEGORY)
                    .size(20)
                    .color(theme.primary()),
                style::label(theme, strings::KIND).size(18),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            column(kinds).spacing(style::GAP)
        ]
        .spacing(style::GAP),
    )
    .width(Length::Fixed(KIND_LIST_WIDTH));

    let form_style = card::Style::filled(theme);
    let name_input = TextInput::new("", &form.name, theme)
        .with_label_text(strings::NAME, form_style.container_color)
        .on_input(Message::NameChanged)
        .padding(INPUT_PADDING);
    let group_input = TextInput::new(strings::GROUP_HINT, &form.group, theme)
        .with_label_text(strings::GROUP, form_style.container_color)
        .on_input(Message::GroupChanged)
        .padding(INPUT_PADDING);

    let form_header = row![
        style::icon(style::kind_icon(form.kind))
            .size(26)
            .color(theme.primary()),
        style::label(theme, strings::kind_label(form.kind)).size(22),
        style::icon_chip::<Message>(
            theme,
            style::kind_icon(form.kind),
            strings::kind_label(form.kind),
            style::kind_tone(form.kind),
        ),
    ]
    .spacing(8)
    .align_y(Vertical::Center);

    let mut fields = column![
        form_header,
        labeled_with_icon(theme, style::icons::EDIT, strings::NAME, name_input),
        labeled_with_icon(theme, style::icons::GROUP, strings::GROUP, group_input),
    ]
    .spacing(style::GAP);
    if form.kind == Kind::Ymm4 {
        fields = fields.push(edition_picker(theme, form.edition));
    }
    fields = fields.push(labeled_with_icon(
        theme,
        style::icons::HISTORY,
        strings::VERSION,
        version_picker(
            form,
            theme,
            &form.choices,
            form.selected.as_ref(),
            Message::VersionSelected,
        ),
    ));
    if form.has_addon() {
        fields = fields.push(labeled_with_icon(
            theme,
            style::icons::EXTENSION,
            strings::ADDON_VERSION,
            version_picker(
                form,
                theme,
                &form.addon_choices,
                form.addon_selected.as_ref(),
                Message::AddonSelected,
            ),
        ));
    }
    if form.kind == Kind::Aviutl2 {
        fields = fields.push(
            checkbox(form.install_recommended)
                .label(strings::RECOMMENDED_PLUGINS)
                .on_toggle(Message::ToggleRecommended),
        );
    }

    let submit = m3::button(
        button::Style::filled(theme, Accent::Primary),
        style::btn_content(style::icons::ADD, strings::CREATE),
    )
    .on_press_maybe(form.can_submit().then_some(Message::Submit));
    let cancel = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        style::btn_content(style::icons::CLOSE, strings::CANCEL),
    )
    .on_press(Message::Cancel);
    let actions = row![submit, cancel].spacing(8);

    let form_card =
        m3::card(form_style, column![fields, actions].spacing(style::PADDING)).width(Length::Fill);

    column![
        row![
            style::icon(style::icons::ADD)
                .size(26)
                .color(theme.primary()),
            style::label(theme, strings::TAB_NEW).size(24),
        ]
        .spacing(8)
        .align_y(Vertical::Center),
        row![kind_card, form_card].spacing(style::GAP),
    ]
    .spacing(style::GAP)
    .into()
}

fn kind_entry<'a>(theme: &Theme, kind: Kind, current: Kind, host: Host) -> Element<'a, Message> {
    let label = strings::kind_label(kind);
    let (interaction, badge) = match availability(kind, host) {
        Availability::Available => (
            card::Interaction::Press(Action::Direct(Message::KindSelected(kind))),
            None,
        ),
        Availability::UnsupportedHost => (
            card::Interaction::Disabled,
            Some((strings::unsupported_host_badge(host), Tone::Error)),
        ),
        Availability::NoProvider => (
            card::Interaction::Disabled,
            Some((strings::PLANNED.to_owned(), Tone::Tertiary)),
        ),
    };
    let selected = badge.is_none() && kind == current;
    let color = style::tile_content(theme, selected);
    let icon_color = if selected {
        theme.on_primary_container()
    } else {
        theme.primary()
    };
    let mut content = row![
        style::icon(style::kind_icon(kind))
            .size(20)
            .color(icon_color),
        iced::widget::text(label).color(color).width(Length::Fill)
    ]
    .spacing(8)
    .align_y(Vertical::Center);
    if let Some((text, tone)) = badge {
        content = content.push(style::chip::<Message>(theme, text, tone));
    }
    m3::card(style::tile_card(theme, selected), content)
        .interaction(interaction)
        .padding(Padding::from(ENTRY_PADDING))
        .width(Length::Fill)
        .into()
}

fn labeled_with_icon<'a>(
    theme: &Theme,
    glyph: &'static str,
    label: &'static str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        row![
            style::icon(glyph).size(15).color(theme.primary()),
            style::label(theme, label).size(14),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
        input.into()
    ]
    .spacing(4)
    .into()
}

fn edition_picker<'a>(theme: &Theme, current: Edition) -> Element<'a, Message> {
    let entry = |edition: Edition| connected::Button {
        content: style::btn_content(style::icons::LAYERS, strings::edition_label(edition)),
        on_press: Some(Message::EditionSelected(edition)),
        selected: current == edition,
    };
    let group = m3::connected_button_group(
        button::Style::tonal(theme, Accent::Primary),
        vec![entry(Edition::Normal), entry(Edition::Lite)],
    );
    labeled_with_icon(theme, style::icons::LAYERS, strings::EDITION, group)
}

fn version_picker<'a>(
    form: &'a Form,
    theme: &Theme,
    choices: &'a [VersionChoice],
    current: Option<&VersionChoice>,
    select: impl Fn(VersionChoice) -> Message + 'a,
) -> Element<'a, Message> {
    match &form.load {
        Load::Idle | Load::Loading => row![
            style::icon(style::icons::SYNC)
                .size(16)
                .color(theme.primary()),
            style::label(theme, strings::LOADING).size(14),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .into(),
        Load::Failed(reason) => column![
            row![
                style::icon(style::icons::ERROR)
                    .size(16)
                    .color(theme.error()),
                style::label(theme, format!("{}: {reason}", strings::LOAD_FAILED)).size(12),
            ]
            .spacing(6)
            .align_y(Vertical::Center),
            m3::button(
                button::Style::tonal(theme, Accent::Primary),
                style::btn_content(style::icons::REFRESH, strings::RELOAD),
            )
            .on_press(Message::Reload),
        ]
        .spacing(6)
        .into(),
        Load::Ready if choices.is_empty() => row![
            style::icon(style::icons::INFO)
                .size(16)
                .color(theme.on_surface_variant()),
            style::label(theme, strings::NO_VERSION).size(14),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .into(),
        Load::Ready => {
            let entries = choices.iter().map(|choice| {
                let selected = current == Some(choice);
                let glyph = if selected {
                    style::icons::CHECK_CIRCLE
                } else {
                    style::icons::HISTORY
                };
                let content_color = style::tile_content(theme, selected);
                m3::card(
                    style::tile_card(theme, selected),
                    row![
                        style::icon(glyph).size(16).color(content_color),
                        style::label(theme, choice.to_string()).color(content_color),
                    ]
                    .spacing(8)
                    .align_y(Vertical::Center),
                )
                .interaction(card::Interaction::Press(Action::Direct(select(
                    choice.clone(),
                ))))
                .padding(Padding::from(ENTRY_PADDING))
                .width(Length::Fill)
                .into()
            });
            scrollable(column(entries).spacing(4))
                .height(Length::Fixed(VERSION_LIST_HEIGHT))
                .style(style::scroll(theme))
                .into()
        }
    }
}
