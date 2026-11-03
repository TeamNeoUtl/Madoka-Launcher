use iced::alignment::Vertical;
use iced::widget::{column, row, scrollable};
use iced::{Element, Length, Padding};
use iced_m3::theme::{Accent, Mode, Theme};
use iced_m3::widget::text_input::TextInput;
use iced_m3::widget::{self as m3, Action, button, card, switch};

use crate::strings;
use crate::style;

const INPUT_PADDING: f32 = 12.0;
const ENTRY_PADDING: [f32; 2] = [8.0, 16.0];
const FONT_LIST_HEIGHT: f32 = 220.0;

#[derive(Debug, Clone)]
pub enum Message {
    TokenChanged(String),
    SaveToken,
    DnsChanged(String),
    SaveDns,
    ThemeToggled,
    FontSelected(Option<String>),
    OpenDataDirectory,
}

pub struct Form {
    pub token: String,
    pub dns: String,
    pub font: Option<String>,
    pub fonts: Vec<String>,
}

pub fn view<'a>(form: &'a Form, theme: &'a Theme, data_dir: &str) -> Element<'a, Message> {
    let background = card::Style::filled(theme).container_color;
    let token = TextInput::new(strings::GITHUB_TOKEN_HINT, &form.token, theme)
        .with_label_text(strings::GITHUB_TOKEN, background)
        .on_input(Message::TokenChanged)
        .padding(INPUT_PADDING);
    let dns = TextInput::new(strings::DNS_HINT, &form.dns, theme)
        .with_label_text(strings::DNS, background)
        .on_input(Message::DnsChanged)
        .padding(INPUT_PADDING);
    let save_dns = m3::button(
        button::Style::filled(theme, Accent::Primary),
        button::Content::Label(strings::SAVE.into()),
    )
    .on_press(Message::SaveDns);
    let save = m3::button(
        button::Style::filled(theme, Accent::Primary),
        button::Content::Label(strings::SAVE.into()),
    )
    .on_press(Message::SaveToken);
    let open = m3::button(
        button::Style::tonal(theme, Accent::Primary),
        button::Content::Label(strings::OPEN_FOLDER.into()),
    )
    .on_press(Message::OpenDataDirectory);

    let dark_mode = row![
        style::label(theme, strings::DARK_MODE).width(Length::Fill),
        switch(theme, theme.mode == Mode::Dark).on_toggle(Message::ThemeToggled),
    ]
    .spacing(8)
    .align_y(Vertical::Center);

    let body = column![
        dark_mode,
        font_picker(form, theme),
        row![
            style::label(theme, format!("{}: {data_dir}", strings::DATA_DIRECTORY))
                .size(14)
                .width(Length::Fill),
            open
        ]
        .spacing(8)
        .align_y(Vertical::Center),
        row![token, save].spacing(8).align_y(Vertical::Center),
        row![dns, save_dns].spacing(8).align_y(Vertical::Center),
        style::label(
            theme,
            format!("{}: {}", strings::LANGUAGE, strings::LANGUAGE_VALUE)
        )
        .size(14),
    ]
    .spacing(style::GAP);
    column![
        style::label(theme, strings::TAB_SETTINGS).size(24),
        m3::card(card::Style::filled(theme), body).width(Length::Fill),
    ]
    .spacing(style::GAP)
    .into()
}

fn font_picker<'a>(form: &'a Form, theme: &'a Theme) -> Element<'a, Message> {
    let entry = |label: &'a str, value: Option<&'a str>| -> Element<'a, Message> {
        let selected = form.font.as_deref() == value;
        m3::card(
            style::tile_card(theme, selected),
            style::label(theme, label).color(style::tile_content(theme, selected)),
        )
        .interaction(card::Interaction::Press(Action::Direct(
            Message::FontSelected(value.map(str::to_owned)),
        )))
        .padding(Padding::from(ENTRY_PADDING))
        .width(Length::Fill)
        .into()
    };
    let entries = std::iter::once(entry(strings::FONT_DEFAULT, None)).chain(
        form.fonts
            .iter()
            .map(|name| entry(name.as_str(), Some(name.as_str()))),
    );
    column![
        style::label(theme, strings::FONT).size(14),
        scrollable(column(entries).spacing(4))
            .height(Length::Fixed(FONT_LIST_HEIGHT))
            .style(style::scroll(theme)),
    ]
    .spacing(4)
    .into()
}
