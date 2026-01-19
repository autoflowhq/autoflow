use std::collections::HashMap;

use derive_builder::Builder;
use getset::{Getters, Setters};

use crate::task::{TaskStatus, Value};

/// Represents the result of a task execution including status, outputs, logs, and metrics
#[derive(Builder, Getters, Setters)]
#[builder(pattern = "owned", setter(into))]
pub struct TaskResult<'a> {
    /// Status of the task execution
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    status: TaskStatus,

    /// Outputs produced by the task
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    outputs: HashMap<&'a str, Value<'a>>,

    /// Optional message providing additional information about the task execution
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    message: Option<String>,

    /// Logs generated during task execution
    #[getset(get = "pub")]
    #[builder(default)]
    logs: Vec<String>,

    /// Metrics collected during task execution
    #[getset(get = "pub")]
    #[builder(default)]
    metrics: HashMap<String, f64>,
}

impl<'a> Default for TaskResult<'a> {
    fn default() -> Self {
        Self {
            status: TaskStatus::Completed,
            outputs: HashMap::new(),
            message: None,
            logs: Vec::new(),
            metrics: HashMap::new(),
        }
    }
}

impl<'a> TaskResult<'a> {
    /// Creates a new builder for TaskResult
    pub fn builder() -> TaskResultBuilder<'a> {
        TaskResultBuilder::default()
    }
}
