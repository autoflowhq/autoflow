// core/src/workflow/error.rs
use thiserror::Error;
use uuid::Uuid;

use crate::{task::DataType, workflow::ReferenceKind};

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("task {id} already exists in workflow")]
    DuplicateTask { id: Uuid },

    #[error("trigger {id} already exists in workflow")]
    DuplicateTrigger { id: Uuid },

    #[error("cannot remove task {id}: dependent tasks exist: {dependents:?}")]
    TaskHasDependents { id: Uuid, dependents: Vec<Uuid> },

    #[error("cannot remove trigger {id}: dependent tasks exist: {dependents:?}")]
    TriggerHasDependents { id: Uuid, dependents: Vec<Uuid> },

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

    #[error(
        "task '{task_id}' defines input '{input}', but that input is not present in the task schema"
    )]
    InputNotInSchema { task_id: Uuid, input: String },

    #[error(
        "type mismatch for input '{input}' in task '{task_id}': expected {expected}, but referenced output '{output}' from {reference_kind} ID {reference_id} is {found}"
    )]
    InputTypeMismatch {
        task_id: Uuid,
        input: String,
        expected: DataType,
        found: DataType,
        reference_id: Uuid,
        reference_kind: ReferenceKind,
        output: String,
    },
}

pub type Result<T> = std::result::Result<T, WorkflowError>;
