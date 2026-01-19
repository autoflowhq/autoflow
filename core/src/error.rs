use thiserror::Error;

use crate::workflow::WorkflowError;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("workflow error: {0}")]
    Workflow(#[from] WorkflowError),
}

pub type Result<T> = std::result::Result<T, CoreError>;
