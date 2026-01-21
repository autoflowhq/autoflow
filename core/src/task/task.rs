use std::collections::{HashMap, HashSet};

use derive_builder::Builder;
use getset::{CopyGetters, Getters, Setters};
use uuid::Uuid;

use crate::{
    task::{InputBinding, TaskDefinition, TaskError, TaskStatus},
    workflow::DependencyRef,
};

/// Represents an instance of a Task within a workflow
#[derive(Getters, Setters, CopyGetters, Builder, Clone)]
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
    dependencies: HashSet<DependencyRef>,

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
    pub fn add_dependency(&mut self, dependency: DependencyRef) {
        self.dependencies.insert(dependency);
    }

    /// Removes a dependency on another task by its UUID
    pub fn remove_dependency(&mut self, dependency: DependencyRef) {
        self.dependencies.remove(&dependency);
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

    /// Retrieves all dependencies of the task, including dependencies via input bindings, as well
    /// as explicit dependencies. Returns a tuple: (task dependencies, trigger dependencies).
    pub fn get_all_dependencies(&self) -> Vec<DependencyRef> {
        let mut deps = Vec::new();
        // Collect from input bindings
        for binding in self.inputs.values() {
            if let InputBinding::Reference { ref_from, .. } = binding {
                if !deps.contains(ref_from) {
                    deps.push(ref_from.clone());
                }
            }
        }
        // Collect explicit dependencies
        for dep in &self.dependencies {
            if !deps.contains(dep) {
                deps.push(dep.clone());
            }
        }
        deps
    }
}

impl<'a> PartialEq for Task<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<'a> Eq for Task<'a> {}

use std::fmt;

impl<'a> fmt::Debug for Task<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("id", &self.id)
            .field("name", &self.name)
            // Add other fields as appropriate
            .finish()
    }
}

impl<'a> TaskBuilder<'a> {
    /// Validates the Task before building
    fn validate(&self) -> Result<(), TaskBuilderError> {
        self.validate_schema().map_err(|e| match e {
            TaskError::MissingTaskDefinition => TaskBuilderError::UninitializedField("definition"),
            _ => TaskBuilderError::ValidationError(e.to_string()),
        })
    }

    /// Validates input bindings against the task definition schema
    /// In particular, checks that literal values match expected data types
    /// as defined in the TaskSchema.
    fn validate_schema(&self) -> crate::task::Result<()> {
        let definition = self
            .definition
            .ok_or_else(|| TaskError::MissingTaskDefinition)?;
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
                            return Err(TaskError::InputTypeMismatch {
                                input: key.to_string(),
                                expected: format!("{:?}", expected),
                                found: format!("{:?}", value),
                            });
                        }
                    } else {
                        return Err(TaskError::InputNotInSchema {
                            input: key.to_string(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
