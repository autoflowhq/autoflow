use thiserror::Error;
use uuid::Uuid;

use crate::workflow::DependencyRef;

/// Errors that can occur during workflow execution
#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("workflow execution encountered a cycle in dependencies")]
    DependencyCycle,

    #[error("missing output '{output}' from dependency {dependency:?}")]
    MissingDependencyOutput {
        dependency: DependencyRef,
        output: String,
    },

    #[error("dependency {dependency:?} failed to produce outputs.")]
    FailedDependencyOutput { dependency: DependencyRef },

    #[error("task with ID {id} not found")]
    TaskNotFound { id: Uuid },

    #[error("trigger with ID {id} not found")]
    TriggerNotFound { id: Uuid },

    #[error("failed to build task context")]
    FailedToBuildTaskContext,
}

pub type Result<T> = std::result::Result<T, ExecutionError>;
