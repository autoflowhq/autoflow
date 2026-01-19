use std::{collections::HashMap, path::PathBuf};

use derive_builder::Builder;
use getset::{CopyGetters, Getters, Setters};
use uuid::Uuid;

use crate::{
    Logger, ProgressReporter,
    task::{TaskStatus, Value},
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
    logger: &'a dyn Logger,
    #[getset(get = "pub")]
    progress: &'a dyn ProgressReporter,

    /// Workflow-level parameters
    #[getset(get = "pub")]
    workflow_params: &'a HashMap<String, Value<'a>>,
}
