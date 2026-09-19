use crate::{config::AppSettings, domain::{TaskProgress, TaskState}};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page { #[default] Home, Instances, Downloads, Accounts, Logs, Settings }

#[derive(Debug)]
pub struct AppState {
    pub page: Page,
    pub settings: AppSettings,
    pub recovery_notice: Option<String>,
    pub task: Option<TaskProgress>,
}

impl AppState {
    #[must_use] pub fn new(settings: AppSettings, recovery_notice: Option<String>) -> Self {
        Self { page: Page::Home, settings, recovery_notice, task: None }
    }
    pub fn start_demo(&mut self) {
        self.task = Some(TaskProgress { state: TaskState::Running, completed: 0, total: 100, phase: "Preparing installation plan".into() });
    }
    pub fn tick(&mut self) {
        let Some(task) = &mut self.task else { return };
        if task.state != TaskState::Running { return; }
        task.completed = (task.completed + 2).min(task.total);
        task.phase = match task.completed { 0..=24 => "Preparing installation plan", 25..=69 => "Downloading libraries", 70..=94 => "Verifying assets", _ => "Finalizing" }.into();
        if task.completed == task.total { task.state = TaskState::Completed; }
    }
    pub fn cancel_demo(&mut self) {
        if let Some(task) = &mut self.task { if task.state == TaskState::Running { task.state = TaskState::Cancelled; task.phase = "Cancelled safely".into(); } }
    }
}
