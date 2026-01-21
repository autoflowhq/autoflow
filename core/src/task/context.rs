use std::{collections::HashMap, path::PathBuf};

use derive_builder::Builder;
use getset::{CopyGetters, Getters, Setters};
use uuid::Uuid;

use crate::{
    logger::{Logger, ProgressReporter},
    task::{TaskDefinition, TaskStatus, Value},
};

/// Contextual information available during task execution
#[derive(Getters, Setters, CopyGetters, Builder)]
pub struct TaskContext<'a> {
    /// Unique identifier of the task
    #[getset(get_copy = "pub")]
    task_id: Uuid,

    /// Name of the task
    #[getset(get = "pub")]
    task_name: &'a str,

    /// Definition of the task
    #[getset(get = "pub")]
    task_definition: &'a TaskDefinition<'a>,

    /// Identifier of the workflow this task belongs to
    #[getset(get_copy = "pub")]
    workflow_id: Uuid,

    /// Optional description of the task
    #[getset(get = "pub")]
    #[builder(default)]
    description: Option<&'a str>,

    /// Current status of the task
    #[getset(get_copy = "pub")]
    #[builder(default)]
    status: TaskStatus,

    /// Inputs already resolved
    #[getset(get = "pub")]
    #[builder(default)]
    resolved_inputs: HashMap<&'a str, Value<'a>>,

    /// Collector for outputs
    #[getset(get = "pub")]
    #[builder(default)]
    output_collector: HashMap<String, Value<'a>>,

    /// Environment & runtime info
    #[getset(get = "pub")]
    #[builder(default)]
    env: HashMap<String, String>,
    #[getset(get = "pub")]
    #[builder(default)]
    cwd: Option<PathBuf>,

    /// Logging & progress
    #[getset(get = "pub")]
    #[builder(default)]
    logger: Option<&'a dyn Logger>,

    /// Progress reporter for the task execution
    #[getset(get = "pub")]
    #[builder(default)]
    progress: Option<&'a dyn ProgressReporter>,
}

impl<'a> TaskContext<'a> {
    /// Creates a new builder for TaskContext
    pub fn builder() -> TaskContextBuilder<'a> {
        TaskContextBuilder::default()
    }
}
