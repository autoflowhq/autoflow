use std::{collections::HashMap, path::PathBuf};

use derive_builder::Builder;
use getset::Getters;
use uuid::Uuid;

use crate::{
    Value,
    logger::{Logger, ProgressReporter},
};

/// Contextual information available during workflow execution
#[derive(Getters, Clone, Builder)]
pub struct WorkflowExecutionContext<'a> {
    // Identity
    /// Unique run identifier
    #[getset(get_copy = "pub")]
    #[allow(dead_code)]
    #[builder(default = "Uuid::new_v4()")]
    run_id: Uuid,

    /// Unique identifier of the workflow
    #[getset(get_copy = "pub")]
    #[allow(dead_code)]
    workflow_id: Uuid,

    // Runtime configuration
    /// Environment & runtime info
    #[getset(get = "pub")]
    #[builder(default)]
    env: HashMap<&'a str, &'a str>,

    /// Current working directory for the workflow execution
    #[getset(get = "pub")]
    #[builder(default)]
    cwd: Option<PathBuf>,

    /// Parameters provided to the workflow at execution time
    #[getset(get = "pub")]
    #[builder(default)]
    workflow_params: HashMap<&'a str, Value<'a>>,

    // Observability
    /// Logger for the workflow execution
    #[getset(get = "pub")]
    #[builder(default)]
    logger: Option<&'a dyn Logger>,

    /// Progress reporter for the workflow execution
    #[getset(get = "pub")]
    #[builder(default)]
    progress: Option<&'a dyn ProgressReporter>,
}

impl<'a> WorkflowExecutionContext<'a> {
    /// Creates a new builder for WorkflowExecutionContext
    pub fn builder() -> WorkflowExecutionContextBuilder<'a> {
        WorkflowExecutionContextBuilder::default()
    }
}
