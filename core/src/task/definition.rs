use derive_builder::Builder;
use getset::{Getters, Setters};

use crate::task::{TaskContext, TaskHandler, TaskResult, TaskSchema};

/// Definition of a kind of  Task within the workflow system.
/// This is independent of any specific instance of a Task.
#[derive(Builder, Getters, Setters)]
#[builder(pattern = "owned")]
pub struct TaskDefinition<'a> {
    /// Name of the task
    #[getset(get = "pub")]
    name: &'a str,

    /// Schema defining inputs and outputs of the task
    #[builder(default)]
    #[getset(get = "pub")]
    schema: TaskSchema<'a>,

    /// Function that is called to execute the task
    #[getset(get = "pub")]
    handler: Box<dyn TaskHandler>,
}

impl<'a> TaskDefinition<'a> {
    /// Creates a new builder for TaskDefinition
    pub fn builder() -> TaskDefinitionBuilder<'a> {
        TaskDefinitionBuilder::default()
    }

    /// Execute the task with the given context
    pub fn execute(&self, ctx: &TaskContext<'a>) -> TaskResult<'a> {
        self.handler.execute(ctx)
    }
}
