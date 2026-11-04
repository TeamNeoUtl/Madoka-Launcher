use iced::alignment::Vertical;
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Length};
use iced_m3::theme::{ColorScheme, Theme};
use iced_m3::widget::{self, card, progress_bar};
use launcher_core::{Progress, TaskInfo, TaskState};
use rust_i18n::t;

use crate::style::{self, Tone};

const BYTES_PER_MIB: u128 = 1_048_576;

pub fn view<M: Clone + 'static>(tasks: &[TaskInfo], theme: &Theme) -> Element<'static, M> {
    let list: Element<'static, M> = if tasks.is_empty() {
        widget::card(
            card::Style::filled(theme),
            row![
                style::icon(style::icons::CHECK_CIRCLE)
                    .size(22)
                    .color(theme.tertiary()),
                style::label(theme, t!("no_tasks")).size(14),
            ]
            .spacing(8)
            .align_y(Vertical::Center),
        )
        .width(Length::Fill)
        .into()
    } else {
        column(tasks.iter().rev().map(|task| task_card(task, theme)))
            .spacing(style::GAP)
            .into()
    };
    column![
        row![
            style::icon(style::icons::TASKS)
                .size(26)
                .color(theme.primary()),
            style::label(theme, t!("tab_tasks")).size(24),
            style::label(theme, format!("({})", tasks.len())).size(16),
        ]
        .spacing(8)
        .align_y(Vertical::Center),
        scrollable(list)
            .height(Length::Fill)
            .style(style::scroll(theme)),
    ]
    .spacing(style::GAP)
    .into()
}

pub fn state_label(state: &TaskState) -> String {
    match state {
        TaskState::Queued => t!("queued").into_owned(),
        TaskState::Running { done, total, .. } => {
            format!("{} {done}/{total}", t!("running"))
        }
        TaskState::Succeeded => t!("succeeded").into_owned(),
        TaskState::Failed { .. } => t!("failed").into_owned(),
    }
}

pub fn state_icon(state: &TaskState) -> &'static str {
    match state {
        TaskState::Queued => style::icons::HOURGLASS,
        TaskState::Running { .. } => style::icons::SYNC,
        TaskState::Succeeded => style::icons::CHECK_CIRCLE,
        TaskState::Failed { .. } => style::icons::ERROR,
    }
}

/// `state_label`, or the byte progress while a download is running.
pub fn detail_label(state: &TaskState) -> String {
    match state {
        TaskState::Running {
            transfer: Some(progress),
            ..
        } => transfer_label(*progress),
        _ => state_label(state),
    }
}

fn transfer_label(progress: Progress) -> String {
    match progress.total {
        Some(total) if total > 0 => format!(
            "{} / {} ({}%)",
            mib(progress.done),
            mib(total),
            progress.done.min(total) * 100 / total
        ),
        _ => mib(progress.done),
    }
}

fn mib(bytes: u64) -> String {
    let tenths = (u128::from(bytes) * 10 + BYTES_PER_MIB / 2) / BYTES_PER_MIB;
    format!("{}.{} MiB", tenths / 10, tenths % 10)
}

const fn tone(state: &TaskState) -> Tone {
    match state {
        TaskState::Queued => Tone::Secondary,
        TaskState::Running { .. } => Tone::Primary,
        TaskState::Succeeded => Tone::Tertiary,
        TaskState::Failed { .. } => Tone::Error,
    }
}

fn task_card<M: Clone + 'static>(task: &TaskInfo, theme: &Theme) -> Element<'static, M> {
    let head = row![
        row![
            style::icon(state_icon(&task.state))
                .size(18)
                .color(theme.primary()),
            style::label(theme, task.title.clone())
                .size(16)
                .width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
        .width(Length::Fill),
        style::icon_chip::<M>(
            theme,
            state_icon(&task.state),
            state_label(&task.state),
            tone(&task.state)
        ),
    ]
    .spacing(style::GAP)
    .align_y(Vertical::Center);

    let mut body = column![head].spacing(8);
    match &task.state {
        TaskState::Running {
            done,
            total,
            transfer,
        } => {
            let overall_fraction = f32::from(*done) / f32::from(*total).max(1.0);
            let overall_bar = column![
                row![
                    style::icon(style::icons::SYNC)
                        .size(13)
                        .color(theme.primary()),
                    style::label(
                        theme,
                        t!(
                            "overall_progress",
                            done = done,
                            total = total,
                            percent = *done * 100 / total.max(&1)
                        )
                        .into_owned()
                    )
                    .size(12),
                ]
                .spacing(4)
                .align_y(Vertical::Center),
                bar(theme, Some(overall_fraction)),
            ]
            .spacing(4);

            body = body.push(overall_bar);

            if let Some(progress) = transfer {
                body = body.push(transfer_bar(theme, *progress));
            }
        }
        TaskState::Failed { reason } => {
            body = body.push(
                row![
                    style::icon(style::icons::ERROR)
                        .size(14)
                        .color(theme.error()),
                    text(reason.trim_end().to_owned())
                        .size(12)
                        .color(theme.error()),
                ]
                .spacing(6)
                .align_y(Vertical::Center),
            );
        }
        TaskState::Queued | TaskState::Succeeded => {}
    }
    widget::card(card::Style::filled(theme), body)
        .padding(14.0)
        .width(Length::Fill)
        .into()
}

fn bar<M: 'static>(theme: &Theme, progress: Option<f32>) -> Element<'static, M> {
    widget::progress_bar(progress_bar::Style::new(theme))
        .progress_maybe(progress)
        .into()
}

/// `done / total` in steps of 0.001. `total` is above zero.
fn fraction(done: u64, total: u64) -> f32 {
    let permille = u128::from(done.min(total)) * 1000 / u128::from(total);
    f32::from(u16::try_from(permille).unwrap_or(1000)) / 1000.0
}

fn transfer_bar<M: 'static>(theme: &Theme, progress: Progress) -> Element<'static, M> {
    let fraction = progress
        .total
        .filter(|total| *total > 0)
        .map(|total| fraction(progress.done, total));
    column![
        bar(theme, fraction),
        row![
            style::icon(style::icons::DOWNLOAD)
                .size(13)
                .color(theme.on_surface_variant()),
            style::label(theme, transfer_label(progress)).size(12),
        ]
        .spacing(4)
        .align_y(Vertical::Center)
    ]
    .spacing(4)
    .into()
}
