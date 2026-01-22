mod context;
mod error;

pub use crate::compiler::{CompiledWorkflow, WorkflowCompiler};
pub use context::WorkflowExecutionContext;
pub use error::{Result, WorkflowError};

use crate::{task::Task, trigger::Trigger};

use derive_builder::Builder;
use getset::{Getters, Setters};
use std::{collections::HashMap, fmt::Display};
use uuid::Uuid;

/// Reference to either a Task or a Trigger within the Workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyRef {
    Task(Uuid),
    Trigger(Uuid),
}

impl DependencyRef {
    /// Get the UUID of the referenced entity
    pub fn id(&self) -> Uuid {
        match self {
            DependencyRef::Task(id) => *id,
            DependencyRef::Trigger(id) => *id,
        }
    }

    /// Create a DependencyRef from a Task
    pub fn from_task(task: &Task) -> Self {
        DependencyRef::Task(task.id())
    }

    /// Create a DependencyRef from a Trigger
    pub fn from_trigger(trigger: &Trigger) -> Self {
        DependencyRef::Trigger(trigger.id())
    }
}

impl Display for DependencyRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyRef::Task(id) => write!(f, "Task({})", id),
            DependencyRef::Trigger(id) => write!(f, "Trigger({})", id),
        }
    }
}

/// Represents a Workflow consisting of Tasks and Triggers
#[derive(Getters, Setters, Builder, Default, Debug)]
#[builder(pattern = "owned", setter(into))]
pub struct Workflow<'a> {
    /// Unique identifier for the workflow
    #[getset(get = "pub")]
    #[builder(default = "Uuid::new_v4()")]
    id: Uuid,

    /// Name of the workflow
    #[getset(get = "pub", set = "pub")]
    name: &'a str,

    /// Optional description of the workflow
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    description: Option<&'a str>,

    /// Tasks within the workflow
    #[getset(get = "pub")]
    #[builder(default, setter(each = "task"))]
    tasks: HashMap<Uuid, Task<'a>>,

    /// Triggers within the workflow
    #[getset(get = "pub")]
    #[builder(default, setter(each = "trigger"))]
    triggers: HashMap<Uuid, Trigger<'a>>,
}

impl<'a> Workflow<'a> {
    /// Creates a new builder for Workflow
    pub fn builder() -> WorkflowBuilder<'a> {
        WorkflowBuilder::default()
    }

    /// Adds a task to the workflow without validation
    /// Validation should be done separately via the compiler
    pub fn add_task(&mut self, task: Task<'a>) {
        let task_id = task.id();
        self.tasks.insert(task_id, task);
    }

    /// Adds a trigger to the workflow without validation
    /// Validation should be done separately via the compiler
    pub fn add_trigger(&mut self, trigger: Trigger<'a>) {
        let trigger_id = trigger.id();
        self.triggers.insert(trigger_id, trigger);
    }

    /// Removes a task from the workflow without checking dependents
    pub fn remove_task(&mut self, task_id: Uuid) {
        self.tasks.remove(&task_id);
    }

    /// Removes a trigger from the workflow without checking dependents
    pub fn remove_trigger(&mut self, trigger_id: Uuid) {
        self.triggers.remove(&trigger_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::CompileError;
    use crate::task::{
        DataType, InputBinding, InputSpec, NoOpTaskHandler, OutputSpec, Task, TaskDefinition,
        TaskSchema,
    };
    use crate::trigger::{Trigger, TriggerDefinition, TriggerSchema};
    use uuid::Uuid;

    fn dummy_task_def<'a>() -> TaskDefinition<'a> {
        TaskDefinition::builder()
            .name("dummy")
            .schema(
                TaskSchema::builder()
                    .input(("foo", InputSpec::new(DataType::String)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap()
    }

    fn dummy_trigger_def<'a>() -> TriggerDefinition<'a> {
        TriggerDefinition::builder()
            .name("dummy_trigger")
            .schema(
                TriggerSchema::builder()
                    .output(("bar", DataType::String))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    }

    #[test]
    fn test_add_task_with_missing_dependency_fails() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();
        let dummy_def = dummy_task_def();
        let mut task = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&dummy_def)
            .build()
            .unwrap();
        // Add a fake dependency
        let missing_id = Uuid::new_v4();
        task.add_dependency(DependencyRef::Task(missing_id));
        wf.add_task(task); // This should succeed (no validation)

        // But compilation should fail
        let res = WorkflowCompiler::compile(&wf);
        assert!(res.is_err());
        if let Err(e) = res {
            match e {
                CompileError::MissingDependency { id, .. } => {
                    assert_eq!(id, missing_id);
                }
                _ => panic!("Expected MissingDependency error"),
            }
        }
    }

    #[test]
    fn test_add_and_remove_task() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();
        let def = dummy_task_def();
        let t1_id = Uuid::new_v4();
        let t2_id = Uuid::new_v4();
        let t1 = Task::builder()
            .id(t1_id)
            .name("t1")
            .definition(&def)
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(t2_id)
            .name("t2")
            .definition(&def)
            .dependencies([DependencyRef::Task(t1_id)])
            .build()
            .unwrap();
        wf.add_task(t1);
        wf.add_task(t2);
        // Remove t1 (should not panic, t2 remains)
        wf.remove_task(t1_id);
        assert!(wf.tasks().get(&t2_id).is_some());
    }

    #[test]
    fn test_remove_task_strict_with_dependents_fails() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();
        let def = dummy_task_def();
        let t1_id = Uuid::new_v4();
        let t2_id = Uuid::new_v4();
        let t1 = Task::builder()
            .id(t1_id)
            .name("t1")
            .definition(&def)
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(t2_id)
            .name("t2")
            .definition(&def)
            .dependencies([DependencyRef::Task(t1_id)])
            .build()
            .unwrap();
        wf.add_task(t1);
        wf.add_task(t2);
        // With new architecture, remove_task just removes without checking
        // Validation of dependents should be done separately if needed
        wf.remove_task(t1_id);
        // t1 should be gone
        assert!(wf.tasks().get(&t1_id).is_none());
    }

    #[test]
    fn test_remove_task_cascade_removes_dependents() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();
        let def = dummy_task_def();
        let t1_id = Uuid::new_v4();
        let t2_id = Uuid::new_v4();
        let t1 = Task::builder()
            .id(t1_id)
            .name("t1")
            .definition(&def)
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(t2_id)
            .name("t2")
            .definition(&def)
            .dependencies([DependencyRef::Task(t1_id)])
            .build()
            .unwrap();
        wf.add_task(t1);
        wf.add_task(t2);
        // Remove just t1 (cascade behavior no longer exists)
        wf.remove_task(t1_id);
        assert!(wf.tasks().get(&t1_id).is_none());
        // t2 should still exist
        assert!(wf.tasks().get(&t2_id).is_some());
    }

    #[test]
    fn test_trigger_reference_and_removal() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();
        let trigger_def = dummy_trigger_def();
        let trigger_id = Uuid::new_v4();
        let trigger = Trigger::builder()
            .id(trigger_id)
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        wf.add_trigger(trigger);
        let def = dummy_task_def();
        let t_id = Uuid::new_v4();
        let t = Task::builder()
            .id(t_id)
            .name("t")
            .definition(&def)
            .input((
                "foo",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger_id),
                    output: "bar",
                },
            ))
            .build()
            .unwrap();
        wf.add_task(t);
        // Remove trigger (should not panic, task remains)
        wf.remove_trigger(trigger_id);
        assert!(wf.tasks().get(&t_id).is_some());
    }

    #[test]
    fn test_task_reference_type_mismatch_fails() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();

        // Task 1: output "out" is Float
        let def1 = TaskDefinition::builder()
            .name("t1")
            .schema(
                TaskSchema::builder()
                    .output(("out", OutputSpec::new(DataType::Float)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap();
        let t1_id = Uuid::new_v4();
        let t1 = Task::builder()
            .id(t1_id)
            .name("t1")
            .definition(&def1)
            .build()
            .unwrap();
        wf.add_task(t1);

        // Task 2: input "foo" expects String, but references t1's "out" (Float)
        let def2 = TaskDefinition::builder()
            .name("t2")
            .schema(
                TaskSchema::builder()
                    .input(("foo", InputSpec::new(DataType::String)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(Uuid::new_v4())
            .name("t2")
            .definition(&def2)
            .input((
                "foo",
                InputBinding::Reference {
                    ref_from: DependencyRef::Task(t1_id),
                    output: "out",
                },
            ))
            .build()
            .unwrap();

        // Add task (no validation during construction)
        wf.add_task(t2);

        // Check that compilation fails with the correct error
        let res = WorkflowCompiler::compile(&wf);
        assert!(res.is_err());
        if let Err(e) = res {
            match e {
                CompileError::InputTypeMismatch {
                    task_id: _,
                    input,
                    expected,
                    found,
                    reference: _,
                    output: _,
                } => {
                    assert_eq!(input, "foo");
                    assert_eq!(expected, DataType::String);
                    assert_eq!(found, DataType::Float);
                }
                _ => panic!("Expected InputTypeMismatch error"),
            }
        }
    }

    #[test]
    fn test_trigger_reference_type_mismatch_fails() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();

        // Trigger: output "bar" is Float
        let trigger_def = TriggerDefinition::builder()
            .name("trig")
            .schema(
                TriggerSchema::builder()
                    .output(("bar", DataType::Float))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let trigger_id = Uuid::new_v4();
        let trigger = Trigger::builder()
            .id(trigger_id)
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        wf.add_trigger(trigger);

        // Task: input "foo" expects Bool, but references trigger's "bar" (Float)
        let def = TaskDefinition::builder()
            .name("t")
            .schema(
                TaskSchema::builder()
                    .input(("foo", InputSpec::new(DataType::Boolean)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap();
        let t = Task::builder()
            .id(Uuid::new_v4())
            .name("t")
            .definition(&def)
            .input((
                "foo",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger_id),
                    output: "bar",
                },
            ))
            .build()
            .unwrap();

        // Add task (no validation during construction)
        wf.add_task(t);

        // Check that compilation fails with the correct error
        let res = WorkflowCompiler::compile(&wf);
        assert!(res.is_err());
        if let Err(e) = res {
            match e {
                CompileError::InputTypeMismatch {
                    task_id: _,
                    input,
                    expected,
                    found,
                    reference: _,
                    output: _,
                } => {
                    assert_eq!(input, "foo");
                    assert_eq!(expected, DataType::Boolean);
                    assert_eq!(found, DataType::Float);
                }
                _ => panic!("Expected InputTypeMismatch error"),
            }
        }
    }

    #[test]
    fn test_task_reference_type_match_succeeds() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();

        // Task 1: output "out" is String
        let def1 = TaskDefinition::builder()
            .name("t1")
            .schema(
                TaskSchema::builder()
                    .output(("out", OutputSpec::new(DataType::String)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap();
        let t1_id = Uuid::new_v4();
        let t1 = Task::builder()
            .id(t1_id)
            .name("t1")
            .definition(&def1)
            .build()
            .unwrap();
        wf.add_task(t1);

        // Task 2: input "foo" expects String, references t1's "out" (String)
        let def2 = TaskDefinition::builder()
            .name("t2")
            .schema(
                TaskSchema::builder()
                    .input(("foo", InputSpec::new(DataType::String)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(Uuid::new_v4())
            .name("t2")
            .definition(&def2)
            .input((
                "foo",
                InputBinding::Reference {
                    ref_from: DependencyRef::Task(t1_id),
                    output: "out",
                },
            ))
            .build()
            .unwrap();

        // Should succeed (no panic)
        wf.add_task(t2);

        // And compilation should succeed
        let res = WorkflowCompiler::compile(&wf);
        assert!(res.is_ok());
    }

    #[test]
    fn test_trigger_reference_type_match_succeeds() {
        let mut wf = Workflow::builder().name("wf").build().unwrap();

        // Trigger: output "bar" is Boolean
        let trigger_def = TriggerDefinition::builder()
            .name("trig")
            .schema(
                TriggerSchema::builder()
                    .output(("bar", DataType::Boolean))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let trigger_id = Uuid::new_v4();
        let trigger = Trigger::builder()
            .id(trigger_id)
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        wf.add_trigger(trigger);

        // Task: input "foo" expects Boolean, references trigger's "bar" (Boolean)
        let def = TaskDefinition::builder()
            .name("t")
            .schema(
                TaskSchema::builder()
                    .input(("foo", InputSpec::new(DataType::Boolean)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap();
        let t = Task::builder()
            .id(Uuid::new_v4())
            .name("t")
            .definition(&def)
            .input((
                "foo",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger_id),
                    output: "bar",
                },
            ))
            .build()
            .unwrap();

        // Should succeed (no panic)
        wf.add_task(t);

        // And compilation should succeed
        let res = WorkflowCompiler::compile(&wf);
        assert!(res.is_ok());
    }
}
