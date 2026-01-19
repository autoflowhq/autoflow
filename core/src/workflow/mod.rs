pub mod context;
pub mod definition;
pub mod result;
pub mod schema;
pub mod trigger;

use std::collections::HashMap;

use derive_builder::Builder;
use getset::{Getters, Setters};

pub use context::TriggerContext;
pub use definition::TriggerDefinition;
pub use schema::TriggerSchema;
pub use trigger::Trigger;

use uuid::Uuid;

use crate::task::{InputBinding, Task};

/// Enum to distinguish between Task and Trigger references
enum ReferenceKind {
    Task,
    Trigger,
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
    name: String,

    /// Optional description of the workflow
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    description: Option<String>,

    /// Tasks within the workflow
    #[getset(get = "pub")]
    #[builder(default)]
    tasks: HashMap<Uuid, Task<'a>>,

    /// Triggers within the workflow
    #[getset(get = "pub")]
    #[builder(default)]
    triggers: HashMap<Uuid, Trigger<'a>>,
}

impl<'a> Workflow<'a> {
    /// Creates a new builder for Workflow
    pub fn builder() -> WorkflowBuilder<'a> {
        WorkflowBuilder::default()
    }

    /// Adds a task to the workflow after verifying dependencies and input types
    pub fn add_task(&mut self, task: Task<'a>) {
        self.verify_task_dependencies(&task);
        self.validate_task_input_types(&task);
        let task_id = task.id();
        self.tasks.insert(task_id, task);
    }

    /// Validate that all input types for a task match the types of the referenced outputs
    fn validate_task_input_types(&self, task: &Task<'a>) {
        for (input_key, binding) in task.inputs() {
            match binding {
                InputBinding::TaskReference { task_id, output } => {
                    self.validate_reference(task, input_key, *task_id, output, ReferenceKind::Task);
                }
                InputBinding::TriggerReference { trigger_id, output } => {
                    self.validate_reference(
                        task,
                        input_key,
                        *trigger_id,
                        output,
                        ReferenceKind::Trigger,
                    );
                }
                _ => {}
            }
        }
    }

    /// Validate a single reference input against its source entity (task or trigger)
    fn validate_reference(
        &self,
        task: &Task<'a>,
        input_key: &str,
        ref_from: Uuid,
        output_key: &str,
        kind: ReferenceKind,
    ) {
        // Get the input spec from the task's schema
        let input_spec = task
            .definition()
            .schema()
            .inputs()
            .get(input_key)
            .unwrap_or_else(|| {
                panic!(
                    "Input '{}' not defined in schema for task {}.",
                    input_key,
                    task.id()
                )
            });

        // Get the referenced output spec
        let output_type = match kind {
            ReferenceKind::Task => {
                let referenced_task = self.tasks.get(&ref_from).unwrap_or_else(|| {
                    panic!(
                        "Input '{}' references non-existent task ID {}.",
                        input_key, ref_from
                    )
                });
                referenced_task
                    .definition()
                    .schema()
                    .outputs()
                    .get(output_key)
                    .unwrap_or_else(|| {
                        panic!(
                            "Input '{}' references non-existent output '{}' from task ID {}.",
                            input_key, output_key, ref_from
                        )
                    })
                    .data_type()
            }
            ReferenceKind::Trigger => {
                let referenced_trigger = self.triggers.get(&ref_from).unwrap_or_else(|| {
                    panic!(
                        "Input '{}' references non-existent trigger ID {}.",
                        input_key, ref_from
                    )
                });
                referenced_trigger
                    .definition()
                    .schema()
                    .outputs()
                    .get(output_key)
                    .unwrap_or_else(|| {
                        panic!(
                            "Input '{}' references non-existent output '{}' from trigger ID {}.",
                            input_key, output_key, ref_from
                        )
                    })
            }
        };

        // Compare data types
        if input_spec.data_type() != output_type {
            panic!(
                "Type mismatch for input '{}' in task '{}': expected {:?}, but referenced output '{}' from {} ID {} is {:?}",
                input_key,
                task.id(),
                input_spec.data_type(),
                output_key,
                match kind {
                    ReferenceKind::Task => "task",
                    ReferenceKind::Trigger => "trigger",
                },
                ref_from,
                output_type
            );
        }
    }

    /// Adds a trigger to the workflow
    pub fn add_trigger(&mut self, trigger: Trigger<'a>) {
        let trigger_id = trigger.id();
        self.triggers.insert(trigger_id, trigger);
    }

    /// Generalized method to find all tasks that depend on a given predicate over InputBinding
    fn find_tasks_with_input_binding<F>(&self, mut pred: F) -> Vec<Uuid>
    where
        F: FnMut(&InputBinding<'a>) -> bool,
    {
        self.tasks
            .iter()
            .filter_map(|(&id, t)| {
                if t.inputs().values().any(|b| pred(b)) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find all tasks that depend on a given task ID (directly or via inputs)
    fn find_tasks_depending_on_task(&self, task_id: Uuid) -> Vec<Uuid> {
        let mut ids = self.find_tasks_with_input_binding(
            |b| matches!(b, InputBinding::TaskReference { task_id: tid, .. } if *tid == task_id),
        );
        // Also include tasks that have this task_id in their depends_on
        ids.extend(self.tasks.iter().filter_map(|(&id, t)| {
            if t.dependencies().contains(&task_id) {
                Some(id)
            } else {
                None
            }
        }));
        ids.sort();
        ids.dedup();
        ids
    }

    /// Find all tasks that depend on a given trigger ID (via inputs)
    fn find_tasks_depending_on_trigger(&self, trigger_id: Uuid) -> Vec<Uuid> {
        self.find_tasks_with_input_binding(|b| {
            matches!(b, InputBinding::TriggerReference { trigger_id: tid, .. } if *tid == trigger_id)
        })
    }

    /// Helper to detach references from dependents for both tasks and triggers
    fn detach_references<F>(&mut self, dependents: Vec<Uuid>, mut should_remove: F)
    where
        F: FnMut(&InputBinding<'a>, Uuid) -> bool,
    {
        for dep_id in dependents {
            if let Some(dep_task) = self.tasks.get_mut(&dep_id) {
                // Remove from depends_on
                dep_task.remove_dependency(dep_id);

                // Collect keys to remove first
                let keys_to_remove: Vec<_> = dep_task
                    .inputs()
                    .iter()
                    .filter_map(|(&key, binding)| {
                        if should_remove(binding, dep_id) {
                            Some(key)
                        } else {
                            None
                        }
                    })
                    .collect();

                // Remove them one by one
                for key in keys_to_remove {
                    dep_task.remove_input(key);
                }
            }
        }
    }

    pub fn remove_task(&mut self, task_id: Uuid, mode: DeleteMode) -> Result<(), String> {
        let dependents = self.find_tasks_depending_on_task(task_id);

        match mode {
            DeleteMode::Strict => {
                if !dependents.is_empty() {
                    return Err(format!(
                        "Cannot remove task {}: dependent tasks {:?} exist.",
                        task_id, dependents
                    ));
                }
            }
            DeleteMode::Cascade => {
                for dep_id in dependents {
                    self.remove_task(dep_id, DeleteMode::Cascade)?;
                }
            }
            DeleteMode::Detach => {
                self.detach_references(dependents, |binding, _id| {
                    matches!(binding, InputBinding::TaskReference { task_id: tid, .. } if *tid == task_id)
                });
            }
        }

        self.tasks.remove(&task_id);
        Ok(())
    }

    pub fn remove_trigger(&mut self, trigger_id: Uuid, mode: DeleteMode) -> Result<(), String> {
        let dependents = self.find_tasks_depending_on_trigger(trigger_id);

        match mode {
            DeleteMode::Strict => {
                if !dependents.is_empty() {
                    return Err(format!(
                        "Cannot remove trigger {}: dependent tasks {:?} exist.",
                        trigger_id, dependents
                    ));
                }
            }
            DeleteMode::Cascade => {
                for dep_id in dependents {
                    self.remove_task(dep_id, DeleteMode::Cascade)?;
                }
            }
            DeleteMode::Detach => {
                self.detach_references(dependents, |binding, _id| {
                    matches!(binding, InputBinding::TriggerReference { trigger_id: tid, .. } if *tid == trigger_id)
                });
            }
        }

        self.triggers.remove(&trigger_id);
        Ok(())
    }

    fn verify_task_dependencies(&self, task: &Task<'a>) {
        for dep_id in task.dependencies() {
            if !self.tasks.contains_key(dep_id) && !self.triggers.contains_key(dep_id) {
                panic!(
                    "Task dependency with ID {} does not exist in the workflow.",
                    dep_id
                );
            }
        }

        for (input, binding) in task.inputs() {
            match binding {
                InputBinding::TaskReference { task_id, output } => {
                    if !self.tasks.contains_key(task_id) {
                        panic!(
                            "Input '{}' references non-existent task ID {}.",
                            input, task_id
                        );
                    }
                    let referenced_task = &self.tasks[task_id];
                    if !referenced_task
                        .definition()
                        .schema()
                        .outputs()
                        .contains_key(output)
                    {
                        panic!(
                            "Input '{}' references non-existent output '{}' from task ID {}.",
                            input, output, task_id
                        );
                    }
                }
                InputBinding::TriggerReference { trigger_id, output } => {
                    if !self.triggers.contains_key(trigger_id) {
                        panic!(
                            "Input '{}' references non-existent trigger ID {}.",
                            input, trigger_id
                        );
                    }
                    let referenced_trigger = &self.triggers[trigger_id];
                    if !referenced_trigger
                        .definition()
                        .schema()
                        .outputs()
                        .contains_key(output)
                    {
                        panic!(
                            "Input '{}' references non-existent output '{}' from trigger ID {}.",
                            input, output, trigger_id
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

pub enum DeleteMode {
    Strict,  // panic / return error if dependents exist
    Cascade, // remove dependents recursively
    Detach,  // keep dependents, just remove references to the deleted entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::schema::{InputSpec, OutputSpec};
    use crate::task::{DataType, InputBinding, Task, TaskDefinition, TaskSchema};
    use crate::workflow::{Trigger, TriggerDefinition, TriggerSchema};
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
            .execute(|_| crate::task::TaskResult::default())
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
            .triggered(|_| crate::workflow::result::TriggerResult::default())
            .build()
            .unwrap()
    }

    #[test]
    #[should_panic(expected = "Task dependency with ID")]
    fn test_add_task_with_missing_dependency_panics() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();
        let dummy_def = dummy_task_def();
        let mut task = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&dummy_def)
            .build()
            .unwrap();
        // Add a fake dependency
        let missing_id = Uuid::new_v4();
        task.add_dependency(missing_id);
        wf.add_task(task); // should panic
    }

    #[test]
    fn test_add_and_remove_task_detach() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();
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
            .dependencies(vec![t1_id])
            .build()
            .unwrap();
        wf.add_task(t1);
        wf.add_task(t2);
        // Remove t1 in Detach mode (should not panic, t2 remains)
        assert!(wf.remove_task(t1_id, DeleteMode::Detach).is_ok());
        assert!(wf.tasks().get(&t2_id).is_some());
    }

    #[test]
    fn test_remove_task_strict_with_dependents_fails() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();
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
            .dependencies(vec![t1_id])
            .build()
            .unwrap();
        wf.add_task(t1);
        wf.add_task(t2);
        // Remove t1 in Strict mode (should fail)
        let res = wf.remove_task(t1_id, DeleteMode::Strict);
        assert!(res.is_err());
    }

    #[test]
    fn test_remove_task_cascade_removes_dependents() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();
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
            .dependencies(vec![t1_id])
            .build()
            .unwrap();
        wf.add_task(t1);
        wf.add_task(t2);
        // Remove t1 in Cascade mode (should remove both t1 and t2)
        assert!(wf.remove_task(t1_id, DeleteMode::Cascade).is_ok());
        assert!(wf.tasks().get(&t1_id).is_none());
        assert!(wf.tasks().get(&t2_id).is_none());
    }

    #[test]
    fn test_trigger_reference_and_removal() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();
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
                InputBinding::TriggerReference {
                    trigger_id,
                    output: "bar",
                },
            ))
            .build()
            .unwrap();
        wf.add_task(t);
        // Remove trigger in Detach mode (should not panic, task remains)
        assert!(wf.remove_trigger(trigger_id, DeleteMode::Detach).is_ok());
        assert!(wf.tasks().get(&t_id).is_some());
    }

    #[test]
    #[should_panic(expected = "Type mismatch for input")]
    fn test_task_reference_type_mismatch_panics() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();

        // Task 1: output "out" is Float
        let def1 = TaskDefinition::builder()
            .name("t1")
            .schema(
                TaskSchema::builder()
                    .output(("out", OutputSpec::new(DataType::Float)))
                    .build()
                    .unwrap(),
            )
            .execute(|_| crate::task::TaskResult::default())
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
            .execute(|_| crate::task::TaskResult::default())
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(Uuid::new_v4())
            .name("t2")
            .definition(&def2)
            .input((
                "foo",
                InputBinding::TaskReference {
                    task_id: t1_id,
                    output: "out",
                },
            ))
            .build()
            .unwrap();

        // Should panic due to type mismatch
        wf.add_task(t2);
    }

    #[test]
    #[should_panic(expected = "Type mismatch for input")]
    fn test_trigger_reference_type_mismatch_panics() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();

        // Trigger: output "bar" is Float
        let trigger_def = TriggerDefinition::builder()
            .name("trig")
            .schema(
                TriggerSchema::builder()
                    .output(("bar", DataType::Float))
                    .build()
                    .unwrap(),
            )
            .triggered(|_| crate::workflow::result::TriggerResult::default())
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
            .execute(|_| crate::task::TaskResult::default())
            .build()
            .unwrap();
        let t = Task::builder()
            .id(Uuid::new_v4())
            .name("t")
            .definition(&def)
            .input((
                "foo",
                InputBinding::TriggerReference {
                    trigger_id,
                    output: "bar",
                },
            ))
            .build()
            .unwrap();

        // Should panic due to type mismatch
        wf.add_task(t);
    }

    #[test]
    fn test_task_reference_type_match_succeeds() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();

        // Task 1: output "out" is String
        let def1 = TaskDefinition::builder()
            .name("t1")
            .schema(
                TaskSchema::builder()
                    .output(("out", OutputSpec::new(DataType::String)))
                    .build()
                    .unwrap(),
            )
            .execute(|_| crate::task::TaskResult::default())
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
            .execute(|_| crate::task::TaskResult::default())
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(Uuid::new_v4())
            .name("t2")
            .definition(&def2)
            .input((
                "foo",
                InputBinding::TaskReference {
                    task_id: t1_id,
                    output: "out",
                },
            ))
            .build()
            .unwrap();

        // Should succeed (no panic)
        wf.add_task(t2);
    }

    #[test]
    fn test_trigger_reference_type_match_succeeds() {
        let mut wf = Workflow::builder().name("wf".to_string()).build().unwrap();

        // Trigger: output "bar" is Boolean
        let trigger_def = TriggerDefinition::builder()
            .name("trig")
            .schema(
                TriggerSchema::builder()
                    .output(("bar", DataType::Boolean))
                    .build()
                    .unwrap(),
            )
            .triggered(|_| crate::workflow::result::TriggerResult::default())
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
            .execute(|_| crate::task::TaskResult::default())
            .build()
            .unwrap();
        let t = Task::builder()
            .id(Uuid::new_v4())
            .name("t")
            .definition(&def)
            .input((
                "foo",
                InputBinding::TriggerReference {
                    trigger_id,
                    output: "bar",
                },
            ))
            .build()
            .unwrap();

        // Should succeed (no panic)
        wf.add_task(t);
    }
}
