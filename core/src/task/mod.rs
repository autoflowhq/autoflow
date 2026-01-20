mod context;
mod datatype;
mod definition;
mod error;
mod result;
mod schema;
mod status;
mod task;

pub use context::TaskContext;
pub use datatype::DataType;
pub use definition::TaskDefinition;
pub use error::{Result, TaskError};
pub use result::TaskResult;
pub use schema::{InputSpec, OutputSpec, TaskSchema};
pub use status::TaskStatus;
pub use task::Task;

use uuid::Uuid;

use crate::Value;

/// Represents how a task input is bound to a value or another task's output
/// It can be a literal value, a reference to another task's output, or a reference to a trigger's output.
#[derive(Clone, Debug)]
pub enum InputBinding<'a> {
    /// A literal value
    Literal(Value<'a>),

    /// Reference to another task's output
    TaskReference { task_id: Uuid, output: &'a str },

    /// Reference to a trigger's output
    TriggerReference { trigger_id: Uuid, output: &'a str },
}

/// Convenience constructors for InputBinding enum variants
impl<'a> InputBinding<'a> {
    /// Creates a TriggerReference variant
    pub fn trigger(trigger_id: Uuid, output: &'a str) -> Self {
        InputBinding::TriggerReference { trigger_id, output }
    }

    /// Creates a TaskReference variant
    pub fn task(task_id: Uuid, output: &'a str) -> Self {
        InputBinding::TaskReference { task_id, output }
    }

    /// Creates a Literal variant
    pub fn literal(value: Value<'a>) -> Self {
        InputBinding::Literal(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::datatype::DataType;
    use crate::task::schema::{InputSpec, TaskSchema};
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn test_literal_type_mismatch_returns_error() {
        // Build a schema expecting an Integer input named "foo"
        let mut input_specs = HashMap::new();
        input_specs.insert("foo", InputSpec::new(DataType::Integer));
        let schema = TaskSchema::builder().inputs(input_specs).build().unwrap();

        // Dummy TaskDefinition with the schema
        let def = TaskDefinition::builder()
            .name("test")
            .schema(schema)
            .execute(|_| TaskResult::default())
            .build()
            .unwrap();

        // Build a Task with a String value for "foo" (should be Integer)
        let result = Task::builder()
            .id(Uuid::new_v4())
            .name("bad_task")
            .definition(&def)
            .input(("foo", InputBinding::literal(Value::String("not an int"))))
            .build();

        assert!(
            result.is_err(),
            "Expected error for type mismatch, got: {:?}",
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("incompatible type"),
            "Error message should mention incompatible type, got: {}",
            err
        );
    }
}
