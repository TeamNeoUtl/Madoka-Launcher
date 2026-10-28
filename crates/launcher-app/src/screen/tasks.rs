use iced::widget::{button, column, progress_bar, row, scrollable, text};
use iced::{Element, Length};
use launcher_core::{TaskInfo, TaskState};

use crate::strings;

type Ui = Element<'static, Message>;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
}

pub fn view(tasks: &[TaskInfo]) -> Ui {
    let rows: Vec<Ui> = tasks.iter().rev().map(task_row).collect();
    let list: Ui = if rows.is_empty() {
        text(strings::NO_TASKS).into()
    } else {
        column(rows).spacing(10).into()
    };
    column![
        button(text(strings::BACK)).on_press(Message::Back),
        text(strings::TASKS).size(22),
        scrollable(list).height(Length::Fill),
    ]
    .spacing(10)
    .padding(12)
    .into()
}

pub fn state_label(state: &TaskState) -> String {
    match state {
        TaskState::Queued => strings::QUEUED.to_owned(),
        TaskState::Running { done, total } => format!("{} {done}/{total}", strings::RUNNING),
        TaskState::Succeeded => strings::SUCCEEDED.to_owned(),
        TaskState::Failed { .. } => strings::FAILED.to_owned(),
    }
}

fn task_row(task: &TaskInfo) -> Ui {
    let head = row![
        text(task.title.clone()).width(Length::Fill),
        text(state_label(&task.state)),
    ]
    .spacing(8);
    let mut item = column![head].spacing(4);
    match &task.state {
        TaskState::Running { done, total } => {
            item = item.push(progress_bar(0.0..=f32::from(*total), f32::from(*done)));
        }
        TaskState::Failed { reason } => {
            item = item.push(text(reason.trim_end().to_owned()).size(12));
        }
        TaskState::Queued | TaskState::Succeeded => {}
    }
    item.into()
}
