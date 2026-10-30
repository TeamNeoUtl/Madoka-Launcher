use std::fmt;

use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, radio, row, text, text_input};
use iced::{Element, Length};
use launcher_core::{AppVersion, Edition, Host, Kind};
use launcher_providers::has_provider;

use crate::ops::CreateRequest;
use crate::strings;
use crate::style::{self, Tone};

type Ui = Element<'static, Message>;

const KIND_LIST_WIDTH: f32 = 300.0;
const BUTTON_PADDING: [u16; 2] = [8, 16];
const INPUT_PADDING: u16 = 10;

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

pub fn availability(kind: Kind, host: Host) -> Availability {
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
    pub load: Load,
    versions: Vec<AppVersion>,
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
            load: Load::Idle,
            versions: Vec::new(),
        }
    }

    pub fn needs_load(&self) -> bool {
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
        self.choices.clear();
        self.selected = None;
    }

    pub fn finish_load(&mut self, result: Result<Vec<AppVersion>, String>) {
        match result {
            Ok(versions) => {
                self.versions = versions;
                self.load = Load::Ready;
            }
            Err(reason) => {
                self.versions.clear();
                self.load = Load::Failed(reason);
            }
        }
    }

    pub fn versions(&self) -> &[AppVersion] {
        &self.versions
    }

    /// Replaces the choices with `installable` and selects the first entry.
    pub fn set_choices(&mut self, installable: &[AppVersion]) {
        self.choices = installable.iter().map(VersionChoice::from).collect();
        self.selected = self.choices.first().cloned();
    }

    pub fn can_submit(&self) -> bool {
        !self.name.trim().is_empty() && self.selected.is_some()
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
        let group = self.group.trim();
        Some(CreateRequest {
            name: self.name.trim().to_owned(),
            kind: self.kind,
            edition: self.edition,
            group: (!group.is_empty()).then(|| group.to_owned()),
            version,
        })
    }
}

pub fn view(form: &Form, host: Host) -> Ui {
    let kinds: Vec<Ui> = KINDS
        .iter()
        .map(|kind| kind_entry(*kind, form.kind, host))
        .collect();
    let kind_card = container(
        column![
            text(strings::KIND).size(18),
            column(kinds).spacing(style::GAP)
        ]
        .spacing(style::GAP),
    )
    .width(Length::Fixed(KIND_LIST_WIDTH))
    .padding(style::PADDING)
    .style(style::card);

    let mut fields = column![
        text(strings::kind_label(form.kind)).size(22),
        labeled(
            strings::NAME,
            text_input(strings::NAME_HINT, &form.name)
                .on_input(Message::NameChanged)
                .padding(INPUT_PADDING)
                .style(style::input),
        ),
        labeled(
            strings::GROUP,
            text_input(strings::GROUP_HINT, &form.group)
                .on_input(Message::GroupChanged)
                .padding(INPUT_PADDING)
                .style(style::input),
        ),
    ]
    .spacing(style::GAP);
    if form.kind == Kind::Ymm4 {
        fields = fields.push(edition_picker(form.edition));
    }
    fields = fields.push(labeled(strings::VERSION, version_picker(form)));

    let actions = row![
        button(text(strings::CREATE))
            .padding(BUTTON_PADDING)
            .style(style::primary_button)
            .on_press_maybe(form.can_submit().then_some(Message::Submit)),
        button(text(strings::CANCEL))
            .padding(BUTTON_PADDING)
            .style(style::secondary_button)
            .on_press(Message::Cancel),
    ]
    .spacing(8);

    let form_card = container(column![fields, actions].spacing(style::PADDING))
        .width(Length::Fill)
        .padding(style::PADDING)
        .style(style::card);

    column![
        text(strings::TAB_NEW).size(24),
        row![kind_card, form_card].spacing(style::GAP),
    ]
    .spacing(style::GAP)
    .into()
}

fn kind_entry(kind: Kind, current: Kind, host: Host) -> Ui {
    let label = strings::kind_label(kind);
    match availability(kind, host) {
        Availability::Available => entry_button(label, None)
            .style(style::tile(kind == current))
            .on_press(Message::KindSelected(kind))
            .into(),
        Availability::UnsupportedHost => {
            disabled_entry(label, strings::unsupported_host_badge(host), Tone::Danger)
        }
        Availability::NoProvider => {
            disabled_entry(label, strings::PLANNED.to_owned(), Tone::Warning)
        }
    }
}

fn entry_button(label: &'static str, badge: Option<Ui>) -> iced::widget::Button<'static, Message> {
    let mut content = row![text(label).width(Length::Fill)]
        .spacing(8)
        .align_y(Vertical::Center);
    if let Some(badge) = badge {
        content = content.push(badge);
    }
    button(content).width(Length::Fill).padding(12)
}

fn disabled_entry(label: &'static str, badge: String, tone: Tone) -> Ui {
    entry_button(label, Some(style::badge_label::<Message>(badge, tone)))
        .style(style::secondary_button)
        .into()
}

fn labeled(label: &'static str, input: impl Into<Ui>) -> Ui {
    column![text(label).size(14), input.into()]
        .spacing(4)
        .into()
}

fn edition_picker(current: Edition) -> Ui {
    let option = |edition: Edition| -> Ui {
        radio(
            strings::edition_label(edition),
            edition,
            Some(current),
            Message::EditionSelected,
        )
        .into()
    };
    labeled(
        strings::EDITION,
        row![option(Edition::Normal), option(Edition::Lite)].spacing(16),
    )
}

fn version_picker(form: &Form) -> Ui {
    match &form.load {
        Load::Idle | Load::Loading => text(strings::LOADING).into(),
        Load::Failed(reason) => column![
            text(format!("{}: {reason}", strings::LOAD_FAILED)).size(12),
            button(text(strings::RELOAD))
                .padding(BUTTON_PADDING)
                .style(style::secondary_button)
                .on_press(Message::Reload),
        ]
        .spacing(6)
        .into(),
        Load::Ready if form.choices.is_empty() => text(strings::NO_VERSION).into(),
        Load::Ready => pick_list(
            form.choices.clone(),
            form.selected.clone(),
            Message::VersionSelected,
        )
        .padding(INPUT_PADDING)
        .width(Length::Fill)
        .style(style::dropdown)
        .into(),
    }
}
