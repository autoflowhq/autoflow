use thiserror::Error;

use crate::{
    compiler::CompileError, executor::ExecutionError, task::TaskError, workflow::WorkflowError,
};

/// Core errors that can occur in the Autoflow system
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("workflow error: {0}")]
    Workflow(#[from] WorkflowError),

    #[error("execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("compilation error: {0}")]
    Compilation(#[from] CompileError),

    #[error("task error: {0}")]
    Task(#[from] TaskError),
}

pub type Result<T> = std::result::Result<T, CoreError>;
