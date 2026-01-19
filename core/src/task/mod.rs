pub mod context;
pub mod datatype;
pub mod definition;
pub mod result;
pub mod schema;
pub mod status;

pub use context::TaskContext;
pub use datatype::DataType;
pub use definition::TaskDefinition;
use derive_builder::Builder;
pub use result::TaskResult;
pub use schema::TaskSchema;
pub use status::TaskStatus;

use getset::{CopyGetters, Getters, Setters};
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

/// Represents an instance of a Task within a workflow
#[derive(Getters, Setters, CopyGetters, Builder, Debug, Clone)]
#[builder(pattern = "owned", setter(into), build_fn(validate = "Self::validate"))]
pub struct Task<'a> {
    /// Unique identifier for the task
    #[getset(get_copy = "pub")]
    #[builder(default = "Uuid::new_v4()")]
    id: Uuid,

    /// Name of the task
    #[getset(get = "pub", set = "pub")]
    name: &'a str,

    /// Optional description of the task
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    description: Option<&'a str>,

    /// Definition of the task
    #[getset(get = "pub")]
    definition: &'a TaskDefinition<'a>,

    /// Input bindings for the task
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default, setter(each = "input"))]
    inputs: HashMap<&'a str, InputBinding<'a>>,

    /// Dependencies on other tasks by their UUIDs. These are tasks that must complete before this
    /// task can run. As opposed to input bindings, dependencies do not pass data, they only enforce
    /// execution order.
    #[getset(get = "pub", get_mut = "pub")]
    #[builder(default, setter(each = "dependency"))]
    dependencies: Vec<Uuid>,

    /// Current status of the task
    #[getset(get = "pub")]
    #[builder(default)]
    status: TaskStatus,

    /// Number of times to retry the task on failure before marking it as failed
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    retry: u32,

    /// Whether the task can be executed in parallel with other tasks that are ready to run
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    parallel: bool,
}

impl<'a> Task<'a> {
    /// Creates a new builder for Task
    pub fn builder() -> TaskBuilder<'a> {
        TaskBuilder::default()
    }

    /// Adds a dependency on another task by its UUID
    pub fn add_dependency(&mut self, task_id: Uuid) {
        self.dependencies.push(task_id);
    }

    /// Removes a dependency on another task by its UUID
    pub fn remove_dependency(&mut self, task_id: Uuid) {
        self.dependencies.retain(|&id| id != task_id);
    }

    /// Clears all dependencies
    pub fn clear_dependencies(&mut self) {
        self.dependencies.clear();
    }

    /// Sets the status of the task
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
    }

    /// Retrieves an input binding by key
    pub fn get_input(&'a self, key: &str) -> Option<&'a InputBinding<'a>> {
        self.inputs.get(key)
    }

    /// Adds or updates an input binding
    pub fn add_input(&mut self, key: &'a str, binding: InputBinding<'a>) {
        self.inputs.insert(key, binding);
    }
    /// Removes an input binding by key
    pub fn remove_input(&mut self, key: &str) {
        self.inputs.remove(key);
    }
}

impl<'a> TaskBuilder<'a> {
    /// Validates the Task before building
    fn validate(&self) -> Result<(), String> {
        self.validate_schema()
    }

    /// Validates input bindings against the task definition schema
    /// In particular, checks that literal values match expected data types
    /// as defined in the TaskSchema.
    fn validate_schema(&self) -> Result<(), String> {
        let definition = self
            .definition
            .ok_or_else(|| "Task definition is required".to_string())?;
        let schema = definition.schema();
        if let Some(inputs) = &self.inputs {
            for (key, binding) in inputs {
                if let InputBinding::Literal(value) = binding {
                    let expected_type = schema
                        .inputs()
                        .iter()
                        .find(|(name, _)| **name == *key)
                        .map(|(_, spec)| spec.data_type());
                    if let Some(expected) = expected_type {
                        if value.get_type() != *expected {
                            return Err(format!(
                                "Input '{}' has incompatible type. Expected {:?}, got {:?}",
                                key, expected, value
                            ));
                        }
                    } else {
                        return Err(format!("Input '{}' not defined in schema", key));
                    }
                }
            }
        }
        Ok(())
    }
}

/// Represents a value used in task inputs and outputs
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    String(&'a str),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
    FilePath(PathBuf),
}

impl<'a> Value<'a> {
    /// Gets the DataType of the Value
    pub fn get_type(&self) -> DataType {
        match self {
            Value::String(_) => DataType::String,
            Value::Integer(_) => DataType::Integer,
            Value::Float(_) => DataType::Float,
            Value::Boolean(_) => DataType::Boolean,
            Value::Json(_) => DataType::Json,
            Value::FilePath(_) => DataType::File,
        }
    }
}

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
