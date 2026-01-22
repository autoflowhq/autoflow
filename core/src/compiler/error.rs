use thiserror::Error;
use uuid::Uuid;

use crate::{task::DataType, workflow::DependencyRef};

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("input '{input}' references non-existent task id {id}.")]
    MissingTaskDependency { input: String, id: Uuid },

    #[error(
        "task depends on the completion of a task or trigger with id {id} that does not exist in the workflow."
    )]
    MissingDependency { id: Uuid },

    #[error("input '{input}' references non-existent output '{output}' from task id {id}.")]
    MissingTaskOutput {
        input: String,
        output: String,
        id: Uuid,
    },

    #[error("input '{input}' references non-existent trigger id {id}.")]
    MissingTriggerDependency { input: String, id: Uuid },

    #[error("input '{input}' references non-existent output '{output}' from trigger id {id}.")]
    MissingTriggerOutput {
        input: String,
        output: String,
        id: Uuid,
    },

    #[error("cannot remove task {id}: dependent tasks exist: {dependents:?}")]
    TaskHasDependents { id: Uuid, dependents: Vec<Uuid> },

    #[error("cannot remove trigger {id}: dependent tasks exist: {dependents:?}")]
    TriggerHasDependents { id: Uuid, dependents: Vec<Uuid> },

    #[error(
        "task '{task_id}' defines input '{input}', but that input is not present in the task schema"
    )]
    InputNotInSchema { task_id: Uuid, input: String },

    #[error(
        "type mismatch for input '{input}' in task '{task_id}': expected {expected}, but referenced output '{output}' from {reference} is {found}"
    )]
    InputTypeMismatch {
        task_id: Uuid,
        input: String,
        expected: DataType,
        found: DataType,
        reference: DependencyRef,
        output: String,
    },

    #[error("cyclic dependency detected from task {from} to task {to}")]
    CyclicDependency { from: Uuid, to: Uuid },
}

pub type Result<T> = std::result::Result<T, CompileError>;
