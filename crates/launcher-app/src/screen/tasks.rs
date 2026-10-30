use iced::alignment::Vertical;
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Length};
use iced_m3::theme::{ColorScheme, Theme};
use iced_m3::widget::{self, card, progress_bar};
use launcher_core::{Progress, TaskInfo, TaskState};

use crate::strings;
use crate::style::{self, Tone};

const BYTES_PER_MIB: f64 = 1_048_576.0;

pub fn view<M: Clone + 'static>(tasks: &[TaskInfo], theme: &Theme) -> Element<'static, M> {
    let list: Element<'static, M> = if tasks.is_empty() {
        widget::card(
            card::Style::filled(theme),
            style::label(theme, strings::NO_TASKS),
        )
        .width(Length::Fill)
        .into()
    } else {
        column(tasks.iter().rev().map(|task| task_card(task, theme)))
            .spacing(style::GAP)
            .into()
    };
    column![
        style::label(theme, strings::TAB_TASKS).size(24),
        scrollable(list)
            .height(Length::Fill)
            .style(style::scroll(*theme)),
    ]
    .spacing(style::GAP)
    .into()
}

pub fn state_label(state: &TaskState) -> String {
    match state {
        TaskState::Queued => strings::QUEUED.to_owned(),
        TaskState::Running { done, total, .. } => {
            format!("{} {done}/{total}", strings::RUNNING)
        }
        TaskState::Succeeded => strings::SUCCEEDED.to_owned(),
        TaskState::Failed { .. } => strings::FAILED.to_owned(),
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
    format!("{:.1} MiB", bytes as f64 / BYTES_PER_MIB)
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
        style::label(theme, task.title.clone())
            .size(16)
            .width(Length::Fill),
        style::chip::<M>(theme, state_label(&task.state), tone(&task.state)),
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
            body = match transfer {
                Some(progress) => body.push(transfer_bar(theme, *progress)),
                None => body.push(bar(
                    theme,
                    Some(f32::from(*done) / f32::from(*total).max(1.0)),
                )),
            };
        }
        TaskState::Failed { reason } => {
            body = body.push(
                text(reason.trim_end().to_owned())
                    .size(12)
                    .color(theme.error()),
            );
        }
        TaskState::Queued | TaskState::Succeeded => {}
    }
    widget::card(card::Style::filled(theme), body)
        .width(Length::Fill)
        .into()
}

fn bar<M: 'static>(theme: &Theme, progress: Option<f32>) -> Element<'static, M> {
    widget::progress_bar(progress_bar::Style::new(theme))
        .progress_maybe(progress)
        .into()
}

fn transfer_bar<M: 'static>(theme: &Theme, progress: Progress) -> Element<'static, M> {
    let fraction = progress
        .total
        .filter(|total| *total > 0)
        .map(|total| progress.done.min(total) as f32 / total as f32);
    column![
        bar(theme, fraction),
        style::label(theme, transfer_label(progress)).size(12)
    ]
    .spacing(4)
    .into()
}
