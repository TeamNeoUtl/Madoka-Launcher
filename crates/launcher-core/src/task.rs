use std::fmt::Display;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(u64);

/// `Running` counts steps of one task: `done` of `total`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Running { done: u16, total: u16 },
    Succeeded,
    Failed { reason: String },
}

impl TaskState {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Queued | Self::Running { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskInfo {
    pub id: TaskId,
    pub title: String,
    pub state: TaskState,
}

#[derive(Debug, Default)]
struct Entries {
    next_id: u64,
    tasks: Vec<TaskInfo>,
}

/// Task list shared between task futures and the UI. Entries stay in insertion order.
#[derive(Debug, Clone, Default)]
pub struct TaskBoard {
    entries: Arc<Mutex<Entries>>,
}

impl TaskBoard {
    pub fn add(&self, title: impl Into<String>) -> TaskHandle {
        let id = {
            let mut entries = self.lock();
            let id = TaskId(entries.next_id);
            entries.next_id += 1;
            entries.tasks.push(TaskInfo {
                id,
                title: title.into(),
                state: TaskState::Queued,
            });
            id
        };
        TaskHandle {
            board: self.clone(),
            id,
        }
    }

    pub fn snapshot(&self) -> Vec<TaskInfo> {
        self.lock().tasks.clone()
    }

    pub fn has_active(&self) -> bool {
        self.lock().tasks.iter().any(|task| task.state.is_active())
    }

    fn set(&self, id: TaskId, state: TaskState) {
        if let Some(task) = self.lock().tasks.iter_mut().find(|task| task.id == id) {
            debug!(task = %task.title, ?state, "task state");
            task.state = state;
        }
    }

    fn lock(&self) -> MutexGuard<'_, Entries> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Debug)]
pub struct TaskHandle {
    board: TaskBoard,
    id: TaskId,
}

impl TaskHandle {
    pub fn running(&self, done: u16, total: u16) {
        self.board.set(self.id, TaskState::Running { done, total });
    }

    pub fn finish<T, E: Display>(self, result: &Result<T, E>) {
        let state = match result {
            Ok(_) => TaskState::Succeeded,
            Err(error) => TaskState::Failed {
                reason: error.to_string(),
            },
        };
        self.board.set(self.id, state);
    }
}
