use thiserror::Error;

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
}

pub type Result<T> = std::result::Result<T, ExecutionError>;
