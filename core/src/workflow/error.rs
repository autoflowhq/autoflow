// core/src/workflow/error.rs
use thiserror::Error;
use uuid::Uuid;

/// Errors related to workflows that can occur during workflow definition or validation
#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("task {id} already exists in workflow")]
    DuplicateTask { id: Uuid },

    #[error("trigger {id} already exists in workflow")]
    DuplicateTrigger { id: Uuid },
}

pub type Result<T> = std::result::Result<T, WorkflowError>;
