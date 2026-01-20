use thiserror::Error;

use crate::{task::TaskError, workflow::WorkflowError};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("workflow error: {0}")]
    Workflow(#[from] WorkflowError),

    #[error("task error: {0}")]
    Task(#[from] TaskError),
}

pub type Result<T> = std::result::Result<T, CoreError>;
