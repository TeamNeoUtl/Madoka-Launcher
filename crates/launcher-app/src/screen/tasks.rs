use iced::alignment::Vertical;
use iced::widget::{column, container, progress_bar, row, scrollable, text};
use iced::{Element, Length};
use launcher_core::{Progress, TaskInfo, TaskState};

use crate::strings;
use crate::style::{self, Tone};

const BYTES_PER_MIB: f64 = 1_048_576.0;

pub fn view<M: Clone + 'static>(tasks: &[TaskInfo]) -> Element<'static, M> {
    let list: Element<'static, M> = if tasks.is_empty() {
        container(text(strings::NO_TASKS))
            .padding(style::PADDING)
            .width(Length::Fill)
            .style(style::card)
            .into()
    } else {
        column(tasks.iter().rev().map(task_card))
            .spacing(style::GAP)
            .into()
    };
    column![
        text(strings::TAB_TASKS).size(24),
        scrollable(list).height(Length::Fill),
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
        TaskState::Succeeded => Tone::Success,
        TaskState::Failed { .. } => Tone::Danger,
    }
}

fn task_card<M: Clone + 'static>(task: &TaskInfo) -> Element<'static, M> {
    let head = row![
        text(task.title.clone()).size(16).width(Length::Fill),
        style::badge_label::<M>(state_label(&task.state), tone(&task.state)),
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
                Some(progress) => body.push(transfer_bar(*progress)),
                None => body.push(
                    progress_bar(0.0..=f32::from(*total), f32::from(*done)).style(style::bar),
                ),
            };
        }
        TaskState::Failed { reason } => {
            body = body.push(text(reason.trim_end().to_owned()).size(12));
        }
        TaskState::Queued | TaskState::Succeeded => {}
    }
    container(body)
        .padding(style::PADDING)
        .width(Length::Fill)
        .style(style::card)
        .into()
}

fn transfer_bar<M: 'static>(progress: Progress) -> Element<'static, M> {
    let bar = match progress.total {
        Some(total) if total > 0 => {
            progress_bar(0.0..=total as f32, progress.done as f32).style(style::bar)
        }
        _ => progress_bar(0.0..=1.0, 0.0).style(style::bar),
    };
    column![bar, text(transfer_label(progress)).size(12)]
        .spacing(4)
        .into()
}
