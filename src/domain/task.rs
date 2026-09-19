use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    Demonstration,
    Install,
    Repair,
    Launch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Queued,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskProgress {
    pub state: TaskState,
    pub completed: u64,
    pub total: u64,
    pub phase: String,
}

impl TaskProgress {
    #[must_use]
    pub fn percentage(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.completed as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }
}
