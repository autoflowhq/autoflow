use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("input '{input}' has incompatible type. expected '{expected}' but found '{found}'")]
    InputTypeMismatch {
        input: String,
        expected: String,
        found: String,
    },

    #[error("input '{input}' not defined in schema")]
    InputNotInSchema { input: String },

    #[error("task definition is required")]
    MissingTaskDefinition,
}

pub type Result<T> = std::result::Result<T, TaskError>;
