use std::borrow::Cow;

use iced::alignment::Vertical;
use iced::widget::{column, row, scrollable};
use iced::{Element, Length, Padding};
use iced_m3::theme::{Accent, ColorScheme, Mode, Theme};
use iced_m3::widget::text_input::TextInput;
use iced_m3::widget::{self as m3, Action, button, card, switch};
use rust_i18n::t;

use crate::i18n::Locale;
use crate::strings;
use crate::style;

const INPUT_PADDING: f32 = 12.0;
const ENTRY_PADDING: [f32; 2] = [8.0, 16.0];
const FONT_LIST_HEIGHT: f32 = 180.0;

#[derive(Debug, Clone)]
pub enum Message {
    TokenChanged(String),
    SaveToken,
    DnsChanged(String),
    SaveDns,
    ThemeToggled,
    FontSelected(Option<String>),
    LocaleSelected(Locale),
    OpenDataDirectory,
    CheckUpdate,
    ApplyUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Installing(String),
    Applied,
}

pub struct Form {
    pub token: String,
    pub dns: String,
    pub font: Option<String>,
    pub fonts: Vec<String>,
    pub locale: Locale,
    pub update: UpdateState,
}

pub fn view<'a>(form: &'a Form, theme: &'a Theme, data_dir: &str) -> Element<'a, Message> {
    let background = card::Style::filled(theme).container_color;
    let token = TextInput::new(&t!("github_token_hint"), &form.token, theme)
        .with_label_text(strings::label("github_token"), background)
        .on_input(Message::TokenChanged)
        .padding(INPUT_PADDING);
    let dns = TextInput::new(&t!("dns_hint"), &form.dns, theme)
        .with_label_text(strings::label("dns"), background)
        .on_input(Message::DnsChanged)
        .padding(INPUT_PADDING);
    let save_dns = m3::button(
        button::Style::filled(theme, Accent::Primary),
        style::btn_content(style::icons::SAVE, t!("save")),
    )
    .on_press(Message::SaveDns);
    let save_token = m3::button(
        button::Style::filled(theme, Accent::Primary),
        style::btn_content(style::icons::SAVE, t!("save")),
    )
    .on_press(Message::SaveToken);
    let open = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        style::btn_content(style::icons::FOLDER_OPEN, t!("open_folder")),
    )
    .on_press(Message::OpenDataDirectory);

    let mode_glyph = if theme.mode == Mode::Dark {
        style::icons::DARK_MODE
    } else {
        style::icons::LIGHT_MODE
    };
    let dark_mode = row![
        row![
            style::icon(mode_glyph).size(20).color(theme.primary()),
            style::label(theme, t!("dark_mode")).size(15),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .width(Length::Fill),
        switch(theme, theme.mode == Mode::Dark).on_toggle(Message::ThemeToggled),
    ]
    .spacing(8)
    .align_y(Vertical::Center);

    // Appearance Section
    let appearance_section = m3::card(
        card::Style::filled(theme),
        column![dark_mode, font_picker(form, theme),].spacing(style::PADDING),
    )
    .padding(style::PADDING)
    .width(Length::Fill);

    // System & Storage Section
    let system_section = m3::card(
        card::Style::filled(theme),
        column![
            version_row(&form.update, theme),
            row![
                row![
                    style::icon(style::icons::FOLDER_OPEN)
                        .size(18)
                        .color(theme.primary()),
                    style::label(theme, format!("{}: {data_dir}", t!("data_directory"))).size(14),
                ]
                .spacing(8)
                .align_y(Vertical::Center)
                .width(Length::Fill),
                open
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        ]
        .spacing(style::PADDING),
    )
    .padding(style::PADDING)
    .width(Length::Fill);

    // Network & API Section
    let network_section = m3::card(
        card::Style::filled(theme),
        column![
            row![
                style::icon(style::icons::KEY)
                    .size(18)
                    .color(theme.primary()),
                token,
                save_token
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            row![
                style::icon(style::icons::NETWORK)
                    .size(18)
                    .color(theme.primary()),
                dns,
                save_dns
            ]
            .spacing(8)
            .align_y(Vertical::Center),
            row![
                style::icon(style::icons::LANGUAGE)
                    .size(18)
                    .color(theme.primary()),
                style::label(theme, t!("language")).size(14),
                locale_selector(form, theme),
            ]
            .spacing(12)
            .align_y(Vertical::Center),
        ]
        .spacing(style::PADDING),
    )
    .padding(style::PADDING)
    .width(Length::Fill);

    let body = column![appearance_section, system_section, network_section,].spacing(style::GAP);

    column![
        row![
            style::icon(style::icons::SETTINGS)
                .size(26)
                .color(theme.primary()),
            style::label(theme, t!("tab_settings").into_owned()).size(24),
        ]
        .spacing(8)
        .align_y(Vertical::Center),
        scrollable(body)
            .height(Length::Fill)
            .style(style::scroll(theme)),
    ]
    .spacing(style::GAP)
    .into()
}

fn font_picker<'a>(form: &'a Form, theme: &'a Theme) -> Element<'a, Message> {
    let entry = |label: Cow<'a, str>, value: Option<&'a str>| -> Element<'a, Message> {
        let selected = form.font.as_deref() == value;
        let glyph = if selected {
            style::icons::CHECK_CIRCLE
        } else {
            style::icons::FONT
        };
        let color = style::tile_content(theme, selected);
        let icon_color = if selected {
            theme.on_primary_container()
        } else {
            theme.primary()
        };
        m3::card(
            style::tile_card(theme, selected),
            row![
                style::icon(glyph).size(16).color(icon_color),
                style::label(theme, label).color(color),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        )
        .interaction(card::Interaction::Press(Action::Direct(
            Message::FontSelected(value.map(str::to_owned)),
        )))
        .padding(Padding::from(ENTRY_PADDING))
        .width(Length::Fill)
        .into()
    };
    let entries = std::iter::once(entry(t!("font_default"), None)).chain(
        form.fonts
            .iter()
            .map(|name| entry(Cow::Borrowed(name.as_str()), Some(name.as_str()))),
    );
    column![
        row![
            style::icon(style::icons::FONT)
                .size(16)
                .color(theme.primary()),
            style::label(theme, t!("font")).size(14),
        ]
        .spacing(6)
        .align_y(Vertical::Center),
        scrollable(column(entries).spacing(4))
            .height(Length::Fixed(FONT_LIST_HEIGHT))
            .style(style::scroll(theme)),
    ]
    .spacing(6)
    .into()
}

fn version_row<'a>(state: &'a UpdateState, theme: &'a Theme) -> Element<'a, Message> {
    let (status, status_icon) = match state {
        UpdateState::Idle => (None, None),
        UpdateState::Checking => (
            Some(t!("checking_update").into_owned()),
            Some(style::icons::SYNC),
        ),
        UpdateState::UpToDate => (
            Some(t!("launcher_up_to_date").into_owned()),
            Some(style::icons::CHECK_CIRCLE),
        ),
        UpdateState::Available(version) => (
            Some(format!("{}: v{version}", t!("update_available"))),
            Some(style::icons::UPDATE),
        ),
        UpdateState::Installing(_) => (
            Some(t!("launcher_updating").into_owned()),
            Some(style::icons::DOWNLOAD),
        ),
        UpdateState::Applied => (
            Some(t!("launcher_updated").into_owned()),
            Some(style::icons::CHECK_CIRCLE),
        ),
    };
    let action = match state {
        UpdateState::Idle | UpdateState::UpToDate => Some(
            m3::button(
                button::Style::tonal(theme, Accent::Primary),
                style::btn_content(style::icons::REFRESH, t!("check_update")),
            )
            .on_press(Message::CheckUpdate),
        ),
        UpdateState::Available(_) => Some(
            m3::button(
                button::Style::filled(theme, Accent::Primary),
                style::btn_content(style::icons::DOWNLOAD, t!("update")),
            )
            .on_press(Message::ApplyUpdate),
        ),
        UpdateState::Checking | UpdateState::Installing(_) | UpdateState::Applied => None,
    };
    let mut line = row![
        row![
            style::icon(style::icons::UPDATE)
                .size(18)
                .color(theme.primary()),
            style::label(
                theme,
                format!("{}: v{}", t!("launcher_version"), strings::APP_VERSION)
            )
            .size(14),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .width(Length::Fill)
    ]
    .spacing(8)
    .align_y(Vertical::Center);
    if let (Some(status), Some(icon_glyph)) = (status, status_icon) {
        line = line.push(
            row![
                style::icon(icon_glyph).size(15).color(theme.primary()),
                style::label(theme, status).size(14),
            ]
            .spacing(4)
            .align_y(Vertical::Center),
        );
    }
    if let Some(action) = action {
        line = line.push(action);
    }
    line.into()
}

fn locale_selector<'a>(form: &'a Form, theme: &'a Theme) -> Element<'a, Message> {
    let chips: Vec<Element<'a, Message>> = Locale::ALL
        .iter()
        .map(|&locale| {
            let selected = locale == form.locale;
            let color = style::tile_content(theme, selected);
            m3::card(
                style::tile_card(theme, selected),
                iced::widget::text(locale.label()).size(12).color(color),
            )
            .interaction(card::Interaction::Press(Action::Direct(
                Message::LocaleSelected(locale),
            )))
            .padding(Padding::from([4.0, 10.0]))
            .into()
        })
        .collect();

    row(chips).spacing(6).align_y(Vertical::Center).into()
}
