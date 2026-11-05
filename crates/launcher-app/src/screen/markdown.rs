//! Lightweight Markdown renderer for Iced with GitHub alert, table, and task list support.

use iced::widget::{column, container, row, text};
use iced::{Element, Length, Padding};
use iced_m3::theme::{ColorScheme, Theme};
use iced_m3::widget::{self as m3, card};
use pulldown_cmark::{BlockQuoteKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use rust_i18n::t;

use crate::style::{self, Tone};

/// Renders a markdown string into an Iced Element.
/// Images show their alt text, because the renderer loads no pictures.
pub fn view<'a, M: Clone + 'static>(theme: &'a Theme, markdown: &'a str) -> Element<'a, M> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let parser = Parser::new_ext(markdown, options);
    let mut elements: Vec<Element<'a, M>> = Vec::new();

    let mut current_text = String::new();
    let mut in_code_block = false;
    let mut code_content = String::new();
    let mut in_blockquote = false;
    let mut quote_kind: Option<BlockQuoteKind> = None;
    let mut quote_content = String::new();
    let mut current_heading_level: Option<HeadingLevel> = None;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_row: Vec<String> = Vec::new();
    let mut in_table = false;

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
            Event::End(TagEnd::Paragraph) => {
                if in_blockquote {
                    quote_content.push('\n');
                } else if lists.is_empty() && !in_table {
                    flush_text(&mut current_text, &mut elements, theme);
                } else {
                    current_text.push('\n');
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
            Event::Start(Tag::BlockQuote(kind)) => {
                flush_text(&mut current_text, &mut elements, theme);
                in_blockquote = true;
                quote_kind = kind;
                quote_content.clear();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_blockquote = false;
                let quote = std::mem::take(&mut quote_content);
                elements.push(render_quote_or_alert(theme, quote_kind.take(), &quote));
            }
            Event::Start(Tag::List(start)) => {
                flush_text(&mut current_text, &mut elements, theme);
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                flush_text(&mut current_text, &mut elements, theme);
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                flush_text(&mut current_text, &mut elements, theme);
            }
            Event::End(TagEnd::Item) => {
                let item = std::mem::take(&mut current_text);
                if !item.trim().is_empty() {
                    let indent = (lists.len().saturating_sub(1) * 16) as f32;
                    let marker = match lists.last_mut() {
                        Some(Some(number)) => {
                            let marker = format!("{number}.");
                            *number += 1;
                            marker
                        }
                        _ => "•".to_owned(),
                    };
                    let line = row![
                        container(text(marker).size(14).color(theme.primary()))
                            .padding(Padding::default().left(indent)),
                        text(item.trim().to_owned())
                            .size(14)
                            .color(theme.on_surface()),
                    ]
                    .spacing(8);
                    elements.push(line.into());
                }
            }
            Event::TaskListMarker(checked) => {
                current_text.push_str(if checked { "☑ " } else { "☐ " });
            }
            Event::Start(Tag::Table(_)) => {
                flush_text(&mut current_text, &mut elements, theme);
                in_table = true;
                table_rows.clear();
                table_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                table_rows.push(std::mem::take(&mut table_row));
            }
            Event::Start(Tag::TableRow) => table_row.clear(),
            Event::End(TagEnd::TableRow) => {
                table_rows.push(std::mem::take(&mut table_row));
            }
            Event::Start(Tag::TableCell) => current_text.clear(),
            Event::End(TagEnd::TableCell) => {
                table_row.push(std::mem::take(&mut current_text).trim().to_owned());
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                elements.push(render_table(theme, std::mem::take(&mut table_rows)));
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

/// The first row is the header row.
fn render_table<'a, M: Clone + 'static>(theme: &Theme, rows: Vec<Vec<String>>) -> Element<'a, M> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let lines: Vec<Element<'a, M>> = rows
        .into_iter()
        .enumerate()
        .map(|(index, cells)| {
            let header = index == 0;
            let color = if header {
                theme.primary()
            } else {
                theme.on_surface()
            };
            let cells: Vec<Element<'a, M>> = (0..columns)
                .map(|column_index| {
                    let content = cells.get(column_index).cloned().unwrap_or_default();
                    container(text(content).size(13).color(color))
                        .width(Length::FillPortion(1))
                        .padding(Padding::from([4.0, 8.0]))
                        .into()
                })
                .collect();
            m3::card(style::tile_card(theme, header), row(cells))
                .padding(Padding::ZERO)
                .width(Length::Fill)
                .into()
        })
        .collect();
    column(lines).spacing(2).into()
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

type Alert = (&'static str, &'static str, Tone);

/// The alert that a blockquote is, and its text without the `[!NAME]` marker.
/// `kind` comes from the parser. The marker in `quote` serves a parser that reports none.
fn alert_of(kind: Option<BlockQuoteKind>, quote: &str) -> (Option<Alert>, &str) {
    let note: Alert = ("NOTE", style::icons::INFO, Tone::Primary);
    let tip: Alert = ("TIP", style::icons::CHECK_CIRCLE, Tone::Tertiary);
    let important: Alert = ("IMPORTANT", style::icons::INFO, Tone::Primary);
    let warning: Alert = ("WARNING", style::icons::WARNING, Tone::Error);
    let caution: Alert = ("CAUTION", style::icons::ERROR, Tone::Error);
    let trimmed = quote.trim();
    match kind {
        Some(BlockQuoteKind::Note) => return (Some(note), trimmed),
        Some(BlockQuoteKind::Tip) => return (Some(tip), trimmed),
        Some(BlockQuoteKind::Important) => return (Some(important), trimmed),
        Some(BlockQuoteKind::Warning) => return (Some(warning), trimmed),
        Some(BlockQuoteKind::Caution) => return (Some(caution), trimmed),
        None => {}
    }
    for (marker, alert) in [
        ("[!NOTE]", note),
        ("[!TIP]", tip),
        ("[!IMPORTANT]", important),
        ("[!WARNING]", warning),
        ("[!CAUTION]", caution),
    ] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return (Some(alert), rest);
        }
    }
    (None, trimmed)
}

/// Renders GitHub alerts (`[!NOTE]`, `[!TIP]`, `[!WARNING]`, `[!IMPORTANT]`, `[!CAUTION]`)
/// or standard blockquotes.
fn render_quote_or_alert<'a, M: Clone + 'static>(
    theme: &Theme,
    kind: Option<BlockQuoteKind>,
    quote: &str,
) -> Element<'a, M> {
    let (alert_type, content) = alert_of(kind, quote);
    let trimmed = content.trim();
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

#[cfg(test)]
mod tests {
    use pulldown_cmark::BlockQuoteKind;

    use super::alert_of;

    #[test]
    fn alert_comes_from_the_parser_kind() {
        let (alert, text) = alert_of(Some(BlockQuoteKind::Warning), " body \n");
        assert_eq!(alert.map(|alert| alert.0), Some("WARNING"));
        assert_eq!(text, "body");
    }

    #[test]
    fn alert_comes_from_the_marker_when_the_parser_reports_none() {
        for (marker, label) in [
            ("[!NOTE]", "NOTE"),
            ("[!TIP]", "TIP"),
            ("[!IMPORTANT]", "IMPORTANT"),
            ("[!WARNING]", "WARNING"),
            ("[!CAUTION]", "CAUTION"),
        ] {
            let quote = format!("{marker}\nbody");
            let (alert, text) = alert_of(None, &quote);
            assert_eq!(alert.map(|alert| alert.0), Some(label));
            assert_eq!(text.trim(), "body");
        }
    }

    #[test]
    fn plain_blockquote_is_no_alert() {
        let (alert, text) = alert_of(None, "just a quote");
        assert!(alert.is_none());
        assert_eq!(text, "just a quote");
    }
}
