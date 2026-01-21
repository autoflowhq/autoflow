use crate::task::{TaskContext, TaskResult};

/// Trait defining the behavior of a Task Handler
pub trait TaskHandler: Send + Sync {
    /// Execute the task with the given context
    fn execute<'a>(&self, ctx: &TaskContext<'a>) -> TaskResult<'a>;
}

/// A No-Operation Task Handler that does nothing and returns an empty result
pub struct NoOpTaskHandler;

impl TaskHandler for NoOpTaskHandler {
    fn execute<'a>(&self, _ctx: &TaskContext<'a>) -> TaskResult<'a> {
        TaskResult::default()
    }
}
