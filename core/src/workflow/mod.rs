mod context;
mod error;
pub mod execution;

pub use context::WorkflowExecutionContext;
pub use error::{Result, WorkflowError};

use crate::{
    task::{DataType, InputBinding, Task},
    trigger::Trigger,
};

use derive_builder::Builder;
use getset::{Getters, Setters};
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};
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
#[derive(Getters, Setters, Builder, Default)]
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

    /// Adds a task to the workflow after verifying dependencies and input types
    pub fn add_task(&mut self, task: Task<'a>) -> Result<()> {
        self.verify_task_dependencies(&task)?;
        self.validate_task_input_types(&task)?;
        let task_id = task.id();
        self.tasks.insert(task_id, task);
        Ok(())
    }

    /// Validate that all input types for a task match the types of the referenced outputs
    fn validate_task_input_types(&self, task: &Task<'a>) -> Result<()> {
        for (input_key, binding) in task.inputs() {
            if let InputBinding::Reference { ref_from, output } = binding {
                self.validate_reference(task, input_key, *ref_from, output)?;
            }
        }
        Ok(())
    }

    /// Validate a single reference input against its source entity (task or trigger)
    fn validate_reference(
        &self,
        task: &Task<'a>,
        input_key: &str,
        ref_from: DependencyRef,
        output_key: &str,
    ) -> Result<()> {
        // Get the input spec from the task's schema
        let input_spec = task
            .definition()
            .schema()
            .inputs()
            .get(input_key)
            .ok_or_else(|| WorkflowError::InputNotInSchema {
                task_id: task.id(),
                input: input_key.to_string(),
            })?;

        // Get the referenced output spec
        let output_type: &DataType = match ref_from {
            DependencyRef::Task(id) => {
                let referenced_task =
                    self.tasks
                        .get(&id)
                        .ok_or_else(|| WorkflowError::MissingTaskDependency {
                            input: input_key.to_string(),
                            id,
                        })?;
                referenced_task
                    .definition()
                    .schema()
                    .outputs()
                    .get(output_key)
                    .ok_or_else(|| WorkflowError::MissingTaskOutput {
                        input: input_key.to_string(),
                        output: output_key.to_string(),
                        id,
                    })?
                    .data_type()
            }
            DependencyRef::Trigger(id) => {
                let referenced_trigger = self.triggers.get(&id).ok_or_else(|| {
                    WorkflowError::MissingTriggerDependency {
                        input: input_key.to_string(),
                        id,
                    }
                })?;
                referenced_trigger
                    .definition()
                    .schema()
                    .outputs()
                    .get(output_key)
                    .ok_or_else(|| WorkflowError::MissingTriggerOutput {
                        input: input_key.to_string(),
                        output: output_key.to_string(),
                        id,
                    })?
            }
        };

        // Compare data types
        if input_spec.data_type() != output_type {
            return Err(WorkflowError::InputTypeMismatch {
                task_id: task.id(),
                input: input_key.to_string(),
                expected: input_spec.data_type().clone(),
                found: output_type.clone(),
                reference: ref_from,
                output: output_key.to_string(),
            });
        }
        Ok(())
    }

    /// Adds a trigger to the workflow
    pub fn add_trigger(&mut self, trigger: Trigger<'a>) {
        let trigger_id = trigger.id();
        self.triggers.insert(trigger_id, trigger);
    }

    /// Generalized method to find all tasks that depend on a given predicate over InputBinding
    #[allow(dead_code)]
    fn find_tasks_with_input_binding<F>(&self, mut pred: F) -> Vec<DependencyRef>
    where
        F: FnMut(&InputBinding<'a>) -> Option<DependencyRef>,
    {
        let mut refs = Vec::new();
        for (&_, t) in &self.tasks {
            for b in t.inputs().values() {
                if let Some(dep_ref) = pred(b) {
                    refs.push(dep_ref);
                }
            }
        }
        refs
    }

    /// Find all tasks that depend on a given DependencyRef (directly or via inputs)
    fn find_tasks_depending_on(&self, dep_ref: &DependencyRef) -> HashSet<DependencyRef> {
        self.tasks
            .values()
            .filter(|t| {
                let input_ref_found = t.inputs().values().any(|b| {
                matches!(b, InputBinding::Reference { ref_from, .. } if ref_from == dep_ref)
            });
                let depends_on_found = t.dependencies().contains(dep_ref);
                input_ref_found || depends_on_found
            })
            .map(|t| DependencyRef::Task(t.id()))
            .collect()
    }

    /// Helper to detach references from dependents for both tasks and triggers
    fn detach_references(&mut self, dependents: Vec<Uuid>, dep_ref: &DependencyRef) {
        for dep_id in dependents {
            if let Some(dep_task) = self.tasks.get_mut(&dep_id) {
                // Remove from depends_on
                match dep_ref {
                    DependencyRef::Task(_) => dep_task.remove_dependency(*dep_ref),
                    DependencyRef::Trigger(_) => dep_task.remove_dependency(*dep_ref),
                }

                // Collect keys to remove first
                let keys_to_remove: Vec<_> = dep_task
                    .inputs()
                    .iter()
                    .filter_map(|(&key, binding)| match (dep_ref, binding) {
                        (
                            DependencyRef::Task(task_id),
                            InputBinding::Reference {
                                ref_from: DependencyRef::Task(tid),
                                ..
                            },
                        ) if tid == task_id => Some(key),
                        (
                            DependencyRef::Trigger(trigger_id),
                            InputBinding::Reference {
                                ref_from: DependencyRef::Trigger(tid),
                                ..
                            },
                        ) if tid == trigger_id => Some(key),
                        _ => None,
                    })
                    .collect();

                // Remove them one by one
                for key in keys_to_remove {
                    dep_task.remove_input(key);
                }
            }
        }
    }

    /// Removes a task from the workflow, handling dependents based on the specified DeleteMode.
    /// There are three modes:
    /// - Strict: Returns an error if dependents exist.
    /// - Cascade: Removes dependents recursively.
    /// - Detach: Keeps dependents but removes references to the deleted task.
    /// # Errors
    /// - `WorkflowError::TaskHasDependents` if in Strict mode and dependents exist.
    /// - Errors from removing dependent tasks in Cascade mode.
    /// - Errors from detaching references in Detach mode.  
    pub fn remove_task(&mut self, task_id: Uuid, mode: DeleteMode) -> Result<()> {
        let dep_ref = DependencyRef::Task(task_id);
        let dependents: Vec<Uuid> = self
            .find_tasks_depending_on(&dep_ref)
            .iter()
            .map(|t| t.id())
            .collect();

        match mode {
            DeleteMode::Strict => {
                if !dependents.is_empty() {
                    return Err(WorkflowError::TaskHasDependents {
                        id: task_id,
                        dependents,
                    }
                    .into());
                }
            }
            DeleteMode::Cascade => {
                for dep_id in &dependents {
                    self.remove_task(*dep_id, DeleteMode::Cascade)?;
                }
            }
            DeleteMode::Detach => {
                self.detach_references(dependents, &dep_ref);
            }
        }

        self.tasks.remove(&task_id);
        Ok(())
    }

    /// Removes a trigger from the workflow, handling dependents based on the specified DeleteMode.
    /// There are three modes:
    /// - Strict: Returns an error if dependents exist.
    /// - Cascade: Removes dependents recursively.
    /// - Detach: Keeps dependents but removes references to the deleted trigger.
    /// # Errors
    /// - `WorkflowError::TriggerHasDependents` if in Strict mode and dependents exist.
    /// - Errors from removing dependent tasks in Cascade mode.
    /// - Errors from detaching references in Detach mode.
    pub fn remove_trigger(&mut self, trigger_id: Uuid, mode: DeleteMode) -> Result<()> {
        let dep_ref = DependencyRef::Trigger(trigger_id);
        let dependents: Vec<Uuid> = self
            .find_tasks_depending_on(&dep_ref)
            .iter()
            .map(|t| t.id())
            .collect();

        match mode {
            DeleteMode::Strict => {
                if !dependents.is_empty() {
                    return Err(WorkflowError::TriggerHasDependents {
                        id: trigger_id,
                        dependents,
                    });
                }
            }
            DeleteMode::Cascade => {
                for dep_id in &dependents {
                    self.remove_task(*dep_id, DeleteMode::Cascade)?;
                }
            }
            DeleteMode::Detach => {
                self.detach_references(dependents, &dep_ref);
            }
        }

        self.triggers.remove(&trigger_id);
        Ok(())
    }

    /// Verifies that all dependencies of a task exist within the workflow
    fn verify_task_dependencies(&self, task: &Task<'a>) -> Result<()> {
        for dep in task.dependencies() {
            match dep {
                DependencyRef::Task(dep_id) => {
                    if !self.tasks.contains_key(dep_id) {
                        return Err(WorkflowError::MissingDependency { id: *dep_id });
                    }
                }
                DependencyRef::Trigger(dep_id) => {
                    if !self.triggers.contains_key(dep_id) {
                        return Err(WorkflowError::MissingDependency { id: *dep_id });
                    }
                }
            }
        }

        for (input, binding) in task.inputs() {
            match binding {
                InputBinding::Reference {
                    ref_from: DependencyRef::Task(task_id),
                    output,
                } => {
                    if !self.tasks.contains_key(task_id) {
                        Err(WorkflowError::MissingTaskDependency {
                            input: input.to_string(),
                            id: *task_id,
                        })?;
                    }
                    let referenced_task = &self.tasks[task_id];
                    if !referenced_task
                        .definition()
                        .schema()
                        .outputs()
                        .contains_key(output)
                    {
                        Err(WorkflowError::MissingTaskOutput {
                            input: input.to_string(),
                            output: output.to_string(),
                            id: *task_id,
                        })?;
                    }
                }
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger_id),
                    output,
                } => {
                    if !self.triggers.contains_key(trigger_id) {
                        Err(WorkflowError::MissingTriggerDependency {
                            input: input.to_string(),
                            id: *trigger_id,
                        })?;
                    }
                    let referenced_trigger = &self.triggers[trigger_id];
                    if !referenced_trigger
                        .definition()
                        .schema()
                        .outputs()
                        .contains_key(output)
                    {
                        Err(WorkflowError::MissingTriggerOutput {
                            input: input.to_string(),
                            output: output.to_string(),
                            id: *trigger_id,
                        })?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Modes for deleting tasks or triggers from a workflow
#[derive(Debug, Clone, Copy)]
pub enum DeleteMode {
    /// Strict mode: Returns an error if dependents exist.
    Strict,

    /// Cascade mode: Removes dependents recursively.
    Cascade,

    /// Detach mode: Keeps dependents but removes references to the deleted entity.
    Detach,
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let res = wf.add_task(task);
        assert!(res.is_err());
        if let Err(e) = res {
            match e {
                WorkflowError::MissingDependency { id, .. } => {
                    assert_eq!(id, missing_id);
                }
                _ => panic!("Expected MissingDependency error"),
            }
        }
    }

    #[test]
    fn test_add_and_remove_task_detach() {
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
        wf.add_task(t1).unwrap();
        wf.add_task(t2).unwrap();
        // Remove t1 in Detach mode (should not panic, t2 remains)
        assert!(wf.remove_task(t1_id, DeleteMode::Detach).is_ok());
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
        wf.add_task(t1).unwrap();
        wf.add_task(t2).unwrap();
        // Remove t1 in Strict mode (should fail)
        let res = wf.remove_task(t1_id, DeleteMode::Strict);
        assert!(res.is_err());
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
        wf.add_task(t1).unwrap();
        wf.add_task(t2).unwrap();
        // Remove t1 in Cascade mode (should remove both t1 and t2)
        assert!(wf.remove_task(t1_id, DeleteMode::Cascade).is_ok());
        assert!(wf.tasks().get(&t1_id).is_none());
        assert!(wf.tasks().get(&t2_id).is_none());
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
        wf.add_task(t).unwrap();
        // Remove trigger in Detach mode (should not panic, task remains)
        assert!(wf.remove_trigger(trigger_id, DeleteMode::Detach).is_ok());
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
        wf.add_task(t1).unwrap();

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

        // Check that the correct error is returned
        let res = wf.add_task(t2);
        assert!(res.is_err());
        if let Err(e) = res {
            match e {
                WorkflowError::InputTypeMismatch {
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

        // Check that the correct error is returned
        let res = wf.add_task(t);
        assert!(res.is_err());
        if let Err(e) = res {
            match e {
                WorkflowError::InputTypeMismatch {
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
        wf.add_task(t1).unwrap();

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
        wf.add_task(t2).unwrap();
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
        wf.add_task(t).unwrap();
    }
}
