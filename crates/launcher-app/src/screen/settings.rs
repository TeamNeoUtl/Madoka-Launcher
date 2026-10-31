use iced::alignment::Vertical;
use iced::widget::{column, row};
use iced::{Element, Length};
use iced_m3::theme::{Accent, Theme};
use iced_m3::widget::text_input::TextInput;
use iced_m3::widget::{self as m3, button, card};

use crate::strings;
use crate::style;

const INPUT_PADDING: f32 = 12.0;

#[derive(Debug, Clone)]
pub enum Message {
    TokenChanged(String),
    SaveToken,
    OpenDataDirectory,
}

pub struct Form {
    pub token: String,
}

pub fn view<'a>(form: &'a Form, theme: &'a Theme, data_dir: String) -> Element<'a, Message> {
    let background = card::Style::filled(theme).container_color;
    let token = TextInput::new(strings::GITHUB_TOKEN_HINT, &form.token, theme)
        .with_label_text(strings::GITHUB_TOKEN, background)
        .on_input(Message::TokenChanged)
        .padding(INPUT_PADDING);
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

    let body = column![
        row![
            style::label(theme, format!("{}: {data_dir}", strings::DATA_DIRECTORY))
                .size(14)
                .width(Length::Fill),
            open
        ]
        .spacing(8)
        .align_y(Vertical::Center),
        row![token, save].spacing(8).align_y(Vertical::Center),
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
