//! Lightweight Markdown renderer for Iced with GitHub alert support.

use iced::widget::{column, container, row, text};
use iced::{Element, Length, Padding};
use iced_m3::theme::{ColorScheme, Theme};
use iced_m3::widget::{self as m3, card};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use rust_i18n::t;

use crate::style::{self, Tone};

/// Renders a markdown string into an Iced Element.
pub fn view<'a, M: Clone + 'static>(theme: &'a Theme, markdown: &'a str) -> Element<'a, M> {
    let parser = Parser::new(markdown);
    let mut elements: Vec<Element<'a, M>> = Vec::new();

    let mut current_text = String::new();
    let mut in_code_block = false;
    let mut code_content = String::new();
    let mut in_blockquote = false;
    let mut quote_content = String::new();
    let mut current_heading_level: Option<HeadingLevel> = None;
    let mut list_depth: usize = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_text(&mut current_text, &mut elements, theme);
                current_heading_level = Some(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                let size = match current_heading_level.take() {
                    Some(HeadingLevel::H1) => 22,
                    Some(HeadingLevel::H2) => 18,
                    Some(HeadingLevel::H3) => 16,
                    _ => 14,
                };
                let content = std::mem::take(&mut current_text);
                if !content.trim().is_empty() {
                    elements.push(text(content).size(size).color(theme.primary()).into());
                }
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_text(&mut current_text, &mut elements, theme);
                in_code_block = true;
                code_content.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                let code = std::mem::take(&mut code_content);
                let code_widget = m3::card(
                    style::tile_card(theme, false),
                    text(code)
                        .size(13)
                        .font(iced::Font::MONOSPACE)
                        .color(theme.on_surface()),
                )
                .padding(Padding::from([8.0, 12.0]))
                .width(Length::Fill);
                elements.push(code_widget.into());
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush_text(&mut current_text, &mut elements, theme);
                in_blockquote = true;
                quote_content.clear();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_blockquote = false;
                let quote = std::mem::take(&mut quote_content);
                elements.push(render_quote_or_alert(theme, &quote));
            }
            Event::Start(Tag::List(_)) => {
                flush_text(&mut current_text, &mut elements, theme);
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                flush_text(&mut current_text, &mut elements, theme);
                list_depth = list_depth.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                flush_text(&mut current_text, &mut elements, theme);
            }
            Event::End(TagEnd::Item) => {
                let item = std::mem::take(&mut current_text);
                if !item.trim().is_empty() {
                    let indent = (list_depth.saturating_sub(1) * 16) as f32;
                    let line = row![
                        container(text("•").size(14).color(theme.primary()))
                            .padding(Padding::default().left(indent)),
                        text(item).size(14).color(theme.on_surface()),
                    ]
                    .spacing(8);
                    elements.push(line.into());
                }
            }
            Event::Text(t) => {
                if in_code_block {
                    code_content.push_str(&t);
                } else if in_blockquote {
                    quote_content.push_str(&t);
                } else {
                    current_text.push_str(&t);
                }
            }
            Event::Code(c) => {
                if in_code_block {
                    code_content.push_str(&c);
                } else if in_blockquote {
                    quote_content.push_str(&format!("`{c}`"));
                } else {
                    current_text.push_str(&format!(" `{c}` "));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_code_block {
                    code_content.push('\n');
                } else if in_blockquote {
                    quote_content.push('\n');
                } else {
                    current_text.push('\n');
                }
            }
            _ => {}
        }
    }

    flush_text(&mut current_text, &mut elements, theme);

    if elements.is_empty() {
        text(t!("no_description").into_owned())
            .size(13)
            .color(theme.outline())
            .into()
    } else {
        column(elements).spacing(8).into()
    }
}

fn flush_text<'a, M: Clone + 'static>(
    current_text: &mut String,
    elements: &mut Vec<Element<'a, M>>,
    theme: &Theme,
) {
    let t = std::mem::take(current_text);
    let trimmed = t.trim();
    if !trimmed.is_empty() {
        elements.push(
            text(trimmed.to_owned())
                .size(14)
                .color(theme.on_surface())
                .into(),
        );
    }
}

/// Renders GitHub alerts (`[!NOTE]`, `[!TIP]`, `[!WARNING]`, `[!IMPORTANT]`, `[!CAUTION]`)
/// or standard blockquotes.
fn render_quote_or_alert<'a, M: Clone + 'static>(theme: &Theme, quote: &str) -> Element<'a, M> {
    let trimmed = quote.trim();
    let (alert_type, content) = if let Some(rest) = trimmed.strip_prefix("[!NOTE]") {
        (Some(("NOTE", style::icons::INFO, Tone::Primary)), rest)
    } else if let Some(rest) = trimmed.strip_prefix("[!TIP]") {
        (
            Some(("TIP", style::icons::CHECK_CIRCLE, Tone::Tertiary)),
            rest,
        )
    } else if let Some(rest) = trimmed.strip_prefix("[!IMPORTANT]") {
        (Some(("IMPORTANT", style::icons::INFO, Tone::Primary)), rest)
    } else if let Some(rest) = trimmed.strip_prefix("[!WARNING]") {
        (Some(("WARNING", style::icons::WARNING, Tone::Error)), rest)
    } else if let Some(rest) = trimmed.strip_prefix("[!CAUTION]") {
        (Some(("CAUTION", style::icons::ERROR, Tone::Error)), rest)
    } else {
        (None, trimmed)
    };

    if let Some((label, glyph, tone)) = alert_type {
        let (border_color, bg_color) = match tone {
            Tone::Primary => (theme.primary(), theme.primary_container()),
            Tone::Tertiary => (theme.tertiary(), theme.tertiary_container()),
            Tone::Secondary => (theme.secondary(), theme.secondary_container()),
            Tone::Error => (theme.error(), theme.error_container()),
        };
        m3::card(
            card::Style {
                container_color: bg_color,
                ..card::Style::filled(theme)
            },
            column![
                row![
                    style::icon(glyph).size(16).color(border_color),
                    text(label).size(13).color(border_color),
                ]
                .spacing(6),
                text(content.trim().to_owned())
                    .size(13)
                    .color(theme.on_surface()),
            ]
            .spacing(6),
        )
        .padding(Padding::from([8.0, 12.0]))
        .width(Length::Fill)
        .into()
    } else {
        m3::card(
            style::tile_card(theme, false),
            row![
                container(column![])
                    .width(Length::Fixed(4.0))
                    .height(Length::Fill),
                text(trimmed.to_owned())
                    .size(13)
                    .color(theme.on_surface_variant()),
            ]
            .spacing(8),
        )
        .padding(Padding::from([6.0, 10.0]))
        .width(Length::Fill)
        .into()
    }
}
