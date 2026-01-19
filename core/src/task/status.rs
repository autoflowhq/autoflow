/// Represents the current status of a task in the workflow
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,

    /// Task is currently in progress
    InProgress,

    /// Task has been completed successfully
    Completed,

    /// Task execution has failed
    Failed(&'static str),
}

impl Default for TaskStatus {
    /// Returns the default TaskStatus as Pending
    fn default() -> Self {
        TaskStatus::Pending
    }
}
