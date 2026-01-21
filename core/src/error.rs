use thiserror::Error;

use crate::{
    task::TaskError,
    workflow::{WorkflowError, execution::ExecutionError},
};

/// Core errors that can occur in the Autoflow system
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("workflow error: {0}")]
    Workflow(#[from] WorkflowError),

    #[error("execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("task error: {0}")]
    Task(#[from] TaskError),
}

pub type Result<T> = std::result::Result<T, CoreError>;
