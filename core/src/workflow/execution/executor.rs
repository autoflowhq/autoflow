use std::collections::{HashMap, HashSet};

use derive_builder::Builder;
use getset::{Getters, Setters};
use petgraph::Graph;
use uuid::Uuid;

use crate::{
    Value,
    plugin::Registry,
    task::{InputBinding, TaskContext, TaskResult, TaskStatus},
    trigger::{Trigger, TriggerContext, TriggerResult},
    workflow::{
        self, DependencyRef, Workflow, WorkflowExecutionContext,
        execution::{ExecutionError, ExecutionGraph},
    },
};

/// Executor for managing the execution of a workflow
#[derive(Getters, Setters, Builder)]
#[builder(pattern = "owned", build_fn(name = "finish", private))]
pub struct WorkflowExecutor<'a> {
    // Immutable
    /// The workflow to be executed
    #[getset(get = "pub")]
    workflow: &'a Workflow<'a>,

    /// The plugin registry used during execution
    #[getset(get = "pub")]
    plugins: &'a Registry<'a>,

    // Execution state
    /// States of tasks in the workflow
    #[getset(get = "pub")]
    #[builder(default, setter(each = "task_state"))]
    task_states: HashMap<Uuid, TaskStatus>,

    /// Results of tasks in the workflow
    #[getset(get = "pub")]
    #[builder(default, setter(each = "task_result"))]
    task_results: HashMap<Uuid, TaskResult<'a>>,

    /// Results of triggers in the workflow
    #[getset(get = "pub")]
    #[builder(default, setter(each = "trigger_result"))]
    trigger_results: HashMap<Uuid, TriggerResult<'a>>,

    // Runtime context
    /// Execution context for the workflow
    #[getset(get = "pub")]
    #[builder(default)]
    context: Option<WorkflowExecutionContext<'a>>,

    // Policy
    /// Maximum parallelism for task execution
    #[getset(get = "pub")]
    #[builder(default = "1")]
    max_parallelism: usize,
}

impl<'a> WorkflowExecutor<'a> {
    /// Creates a new builder for WorkflowExecutor
    pub fn builder() -> WorkflowExecutorBuilder<'a> {
        WorkflowExecutorBuilder::default()
    }

    /// Adds a task result to the executor
    #[allow(dead_code)]
    fn add_task_result(&mut self, task_id: Uuid, result: TaskResult<'a>) {
        self.task_results.insert(task_id, result);
    }

    /// Adds a trigger result to the executor
    #[allow(dead_code)]
    fn add_trigger_result(&mut self, trigger_id: Uuid, result: TriggerResult<'a>) {
        self.trigger_results.insert(trigger_id, result);
    }

    /// Adds a task state to the executor
    #[allow(dead_code)]
    fn add_task_state(&mut self, task_id: Uuid, status: TaskStatus) {
        self.task_states.insert(task_id, status);
    }

    /// Retrieves a task result by task ID
    pub fn get_task_result(&self, task_id: &Uuid) -> Option<&TaskResult<'a>> {
        self.task_results.get(task_id)
    }

    /// Retrieves a trigger result by trigger ID
    pub fn get_trigger_result(&'a self, trigger_id: &Uuid) -> Option<&'a TriggerResult<'a>> {
        self.trigger_results.get(trigger_id)
    }

    /// Retrieves a task state by task ID
    pub fn get_task_state(&self, task_id: &Uuid) -> Option<&TaskStatus> {
        self.task_states.get(task_id)
    }

    /// Updates the state of a task
    pub fn update_task_state(&mut self, task_id: &Uuid, status: TaskStatus) {
        if let Some(state) = self.task_states.get_mut(task_id) {
            *state = status;
        }
    }

    /// Executes the workflow
    pub fn execute_from_trigger(
        &mut self,
        trigger_id: Uuid,
        result: TriggerResult<'a>,
    ) -> workflow::Result<()> {
        // Execution logic to be implemented

        let trigger = self
            .workflow
            .triggers()
            .get(&trigger_id)
            .ok_or_else(|| panic!("Trigger with ID {} not found", trigger_id))
            .unwrap();

        let _trigger_context = TriggerContext::builder()
            .trigger_name(trigger.name())
            .build();

        let graph = self.build_execution_graph(trigger)?;
        let sorted = graph.topological_sort()?;

        // Map to hold outputs for each DependencyRef::Task or DependencyRef::Trigger
        let mut outputs_map: HashMap<DependencyRef, HashMap<&'a str, Value>> = HashMap::new();

        // Insert the trigger's outputs using the correct DependencyRef::Trigger key
        outputs_map.insert(DependencyRef::Trigger(trigger_id), result.outputs().clone());

        for dep in sorted {
            println!("Executing dependency: {:?}", dep);
            match dep {
                DependencyRef::Trigger(_tid) => {}
                DependencyRef::Task(task_id) => {
                    let task = self
                        .workflow
                        .tasks()
                        .get(&task_id)
                        .ok_or_else(|| panic!("Task with ID {} not found", task_id))
                        .unwrap();

                    // Gather inputs from dependencies
                    let mut input_values: HashMap<&'a str, Value> = HashMap::new();

                    for (input_key, binding) in task.inputs() {
                        let value = match binding {
                            InputBinding::Literal(v) => v.clone(),

                            InputBinding::Reference { ref_from, output } => {
                                let dep_outputs = outputs_map.get(ref_from).ok_or_else(|| {
                                    ExecutionError::FailedDependencyOutput {
                                        dependency: *ref_from,
                                    }
                                })?;

                                dep_outputs.get(output).cloned().ok_or_else(|| {
                                    ExecutionError::MissingDependencyOutput {
                                        dependency: *ref_from,
                                        output: output.to_string(),
                                    }
                                })?
                            }
                        };

                        input_values.insert(*input_key, value);
                    }

                    let task_context = TaskContext::builder()
                        .task_id(task_id)
                        .task_name(task.name())
                        .task_definition(task.definition())
                        .workflow_id(*self.workflow.id())
                        .description(*task.description())
                        .resolved_inputs(input_values)
                        .build()
                        .unwrap();
                    let task_result = task.definition().execute(&task_context);
                    let outputs = task_result.outputs();
                    println!("Task Result: {:?}", task_result);

                    self.task_results.insert(task_id, task_result.clone());

                    // If the task failed to produce expected outputs, raise error
                    outputs_map.insert(dep, outputs.clone());
                }
            }
        }

        Ok(())
    }

    fn reachable_tasks(&self, trigger: &'a Trigger<'a>) -> HashSet<DependencyRef> {
        let mut visited = HashSet::new();
        let mut stack = vec![DependencyRef::from_trigger(trigger)];

        while let Some(curr_ref) = stack.pop() {
            for task_ref in self.workflow.find_tasks_depending_on(&curr_ref) {
                if visited.insert(task_ref) {
                    stack.push(task_ref);
                }
            }
        }

        visited
    }

    fn build_execution_graph(&self, trigger: &'a Trigger<'a>) -> workflow::Result<ExecutionGraph> {
        let mut graph = Graph::new();
        let mut index_map = HashMap::new();

        // Find all reachable tasks from this trigger
        let reachable_tasks = self.reachable_tasks(trigger);

        // Add reachable tasks as nodes
        for &task_dep in &reachable_tasks {
            let task_node = task_dep;
            let node_index = graph.add_node(task_node.clone());
            index_map.insert(task_node, node_index);
        }

        // Add edges for dependencies
        for &task_dep in &reachable_tasks {
            let task_node = task_dep;
            let task_index = index_map[&task_node];

            // Get the corresponding Task
            let task = match task_node {
                DependencyRef::Task(id) => self.workflow.tasks().get(&id),
                _ => None,
            };
            if let Some(task) = task {
                // Add edges for explicit dependencies
                for dep in task.dependencies() {
                    if let Some(&dep_index) = index_map.get(dep) {
                        graph.add_edge(dep_index, task_index, ());
                    }
                }
                // Add edges for input bindings
                for binding in task.inputs().values() {
                    if let crate::task::InputBinding::Reference { ref_from, .. } = binding {
                        if let Some(&dep_index) = index_map.get(ref_from) {
                            graph.add_edge(dep_index, task_index, ());
                        }
                    }
                }
            }
        }

        Ok(ExecutionGraph::new(graph, index_map))
    }
}

impl<'a> WorkflowExecutorBuilder<'a> {
    pub fn build(&mut self) -> Result<WorkflowExecutor<'a>, String> {
        // Use the private finish() to build the struct with default/empty context
        let builder = std::mem::take(self);
        let mut executor = builder.finish().map_err(|e| e.to_string())?;

        // Ensure workflow is set
        let workflow = executor.workflow;

        // Construct execution context automatically
        let context = WorkflowExecutionContext::builder()
            .run_id(Uuid::new_v4())
            .workflow_id(*workflow.id())
            .build()
            .map_err(|e| e.to_string())?;

        executor.context = Some(context);
        Ok(executor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Registry;
    use crate::task::{
        DataType, InputBinding, InputSpec, NoOpTaskHandler, OutputSpec, Task, TaskDefinition,
        TaskHandler, TaskSchema,
    };
    use crate::trigger::{Trigger, TriggerDefinition, TriggerSchema};

    struct InOutTaskHandler;

    impl TaskHandler for InOutTaskHandler {
        fn execute<'a>(&self, context: &TaskContext<'a>) -> TaskResult<'a> {
            // Get the first input value (or Null if no inputs)
            let input_value = context
                .resolved_inputs()
                .values()
                .next()
                .cloned()
                .unwrap_or(Value::Null);

            // Create outputs for all expected output keys from the schema
            let mut outputs = HashMap::new();
            for output_key in context.task_definition().schema().outputs().keys() {
                outputs.insert(*output_key, input_value.clone());
            }

            TaskResult::builder().outputs(outputs).build().unwrap()
        }
    }

    fn dummy_task_def<'a>(
        name: &'a str,
        input: Option<(&'a str, DataType)>,
        output: Option<(&'a str, DataType)>,
    ) -> TaskDefinition<'a> {
        let mut schema_builder = TaskSchema::builder();
        if let Some((input_name, input_type)) = input {
            schema_builder = schema_builder.input((input_name, InputSpec::new(input_type)));
        }
        if let Some((output_name, output_type)) = output {
            schema_builder = schema_builder.output((output_name, OutputSpec::new(output_type)));
        }
        if input.is_some() && output.is_some() {
            TaskDefinition::builder()
                .name(name)
                .schema(schema_builder.build().unwrap())
                .handler(Box::new(InOutTaskHandler))
                .build()
                .unwrap()
        } else {
            TaskDefinition::builder()
                .name(name)
                .schema(schema_builder.build().unwrap())
                .handler(Box::new(NoOpTaskHandler))
                .build()
                .unwrap()
        }
    }

    fn dummy_trigger_def<'a>(
        name: &'a str,
        output: Option<(&'a str, DataType)>,
    ) -> TriggerDefinition<'a> {
        let mut schema_builder = TriggerSchema::builder();
        if let Some((output_name, output_type)) = output {
            schema_builder = schema_builder.output((output_name, output_type));
        }
        TriggerDefinition::builder()
            .name(name)
            .schema(schema_builder.build().unwrap())
            .build()
            .unwrap()
    }

    fn setup_registry<'a>() -> Registry<'a> {
        Registry::new()
    }

    #[test]
    fn test_executor_runs_only_relevant_tasks_for_trigger() {
        let reg = setup_registry();
        let trigger_def1 = dummy_trigger_def("trig1", Some(("out", DataType::String)));
        let trigger_def2 = dummy_trigger_def("trig2", Some(("out", DataType::String)));
        let trigger1 = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig1")
            .definition(&trigger_def1)
            .build()
            .unwrap();
        let trigger2 = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig2")
            .definition(&trigger_def2)
            .build()
            .unwrap();

        let t1_def = dummy_task_def("t1", None, Some(("out", DataType::String)));
        let t2_def = dummy_task_def("t2", Some(("in", DataType::String)), None);
        let t3_def = dummy_task_def("t3", Some(("in", DataType::String)), None);

        let t1 = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&t1_def)
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(Uuid::new_v4())
            .name("t2")
            .definition(&t2_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger1.id()),
                    output: "out",
                },
            ))
            .build()
            .unwrap();
        let t3 = Task::builder()
            .id(Uuid::new_v4())
            .name("t3")
            .definition(&t3_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger2.id()),
                    output: "out",
                },
            ))
            .build()
            .unwrap();

        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger1.id(), trigger1.clone()))
            .trigger((trigger2.id(), trigger2.clone()))
            .task((t1.id(), t1.clone()))
            .task((t2.id(), t2.clone()))
            .task((t3.id(), t3.clone()))
            .build()
            .unwrap();

        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();

        // Trigger 1 should only execute t2 (and not t3)
        let trigger1_result = TriggerResult::builder()
            .outputs([("out", Value::String("trigger1_output"))])
            .build()
            .unwrap();
        let result = exec.execute_from_trigger(trigger1.id(), trigger1_result);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        // The execution order should include t2 but not t3
        let reachable = exec.reachable_tasks(&trigger1);
        let task_ids: Vec<_> = reachable
            .iter()
            .filter_map(|d| {
                if let DependencyRef::Task(id) = d {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        assert!(task_ids.contains(&t2.id()));
        assert!(!task_ids.contains(&t3.id()));

        // Trigger 2 should only execute t3 (and not t2)
        let mut exec2 = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        let trigger2_result = TriggerResult::builder()
            .outputs([("out", Value::String("trigger2_output"))])
            .build()
            .unwrap();
        let result2 = exec2.execute_from_trigger(trigger2.id(), trigger2_result);
        assert!(result2.is_ok());
        let reachable2 = exec2.reachable_tasks(&trigger2);
        let task_ids2: Vec<_> = reachable2
            .iter()
            .filter_map(|d| {
                if let DependencyRef::Task(id) = d {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        assert!(task_ids2.contains(&t3.id()));
        assert!(!task_ids2.contains(&t2.id()));
    }

    #[test]
    fn test_executor_handles_no_tasks_for_trigger() {
        let reg = setup_registry();
        let trigger_def = dummy_trigger_def("trig", Some(("out", DataType::String)));
        let trigger = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger.id(), trigger.clone()))
            .build()
            .unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        let result = exec.execute_from_trigger(trigger.id(), TriggerResult::default());
        assert!(result.is_ok());
        let reachable = exec.reachable_tasks(&trigger);
        assert!(reachable.is_empty());
    }

    #[test]
    fn test_executor_handles_task_with_multiple_dependencies() {
        let reg = setup_registry();
        let trigger_def = dummy_trigger_def("trig", Some(("out", DataType::String)));
        let trigger = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        let t1_def = dummy_task_def("t1", None, Some(("out", DataType::String)));
        let t2_def = dummy_task_def(
            "t2",
            Some(("in", DataType::String)),
            Some(("out2", DataType::String)),
        );
        let t3_def = dummy_task_def("t3", Some(("in", DataType::String)), None);
        let t1 = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&t1_def)
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(Uuid::new_v4())
            .name("t2")
            .definition(&t2_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger.id()),
                    output: "out",
                },
            ))
            .dependency(DependencyRef::Task(t1.id()))
            .build()
            .unwrap();
        let t3 = Task::builder()
            .id(Uuid::new_v4())
            .name("t3")
            .definition(&t3_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Task(t2.id()),
                    output: "out2",
                },
            ))
            .build()
            .unwrap();
        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger.id(), trigger.clone()))
            .task((t1.id(), t1.clone()))
            .task((t2.id(), t2.clone()))
            .task((t3.id(), t3.clone()))
            .build()
            .unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        let result = exec.execute_from_trigger(
            trigger.id(),
            TriggerResult::builder()
                .outputs([("out", Value::String("trigger_output"))])
                .build()
                .unwrap(),
        );
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        let reachable = exec.reachable_tasks(&trigger);
        let task_ids: Vec<_> = reachable
            .iter()
            .filter_map(|d| {
                if let DependencyRef::Task(id) = d {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        assert!(task_ids.contains(&t2.id()));
        assert!(task_ids.contains(&t3.id()));
    }

    #[test]
    fn test_executor_handles_circular_dependencies() {
        let reg = setup_registry();
        let trigger_def = dummy_trigger_def("trig", Some(("out", DataType::String)));
        let trigger = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        let def = dummy_task_def(
            "t1",
            Some(("in", DataType::String)),
            Some(("out", DataType::String)),
        );
        let t1_id = Uuid::new_v4();
        let t2_id = Uuid::new_v4();
        let t1 = Task::builder()
            .id(t1_id)
            .name("t1")
            .definition(&def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Task(t2_id),
                    output: "out",
                },
            ))
            .dependency(DependencyRef::from_trigger(&trigger))
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(t2_id)
            .name("t2")
            .definition(&def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Task(t1_id),
                    output: "out",
                },
            ))
            .build()
            .unwrap();
        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger.id(), trigger.clone()))
            .task((t1_id, t1.clone()))
            .task((t2_id, t2.clone()))
            .build()
            .unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        // Should return an error due to circular dependency
        let result = exec.execute_from_trigger(trigger.id(), TriggerResult::default());
        assert!(
            result.is_err(),
            "Expected error due to circular dependency, but got Ok"
        );
        let err_str = format!("{:?}", result.unwrap_err());
        assert!(
            err_str.contains("cycle") || err_str.contains("Cycle"),
            "Error should mention cycle, got: {}",
            err_str
        );
    }

    #[test]
    fn test_executor_handles_multiple_triggers_and_tasks() {
        let reg = setup_registry();
        let trigger_def1 = dummy_trigger_def("trig1", Some(("out", DataType::String)));
        let trigger_def2 = dummy_trigger_def("trig2", Some(("out", DataType::String)));
        let trigger1 = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig1")
            .definition(&trigger_def1)
            .build()
            .unwrap();
        let trigger2 = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig2")
            .definition(&trigger_def2)
            .build()
            .unwrap();
        let t1_def = dummy_task_def("t1", Some(("in", DataType::String)), None);
        let t2_def = dummy_task_def("t2", Some(("in", DataType::String)), None);
        let t1 = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&t1_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger1.id()),
                    output: "out",
                },
            ))
            .build()
            .unwrap();
        let t2 = Task::builder()
            .id(Uuid::new_v4())
            .name("t2")
            .definition(&t2_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger2.id()),
                    output: "out",
                },
            ))
            .build()
            .unwrap();
        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger1.id(), trigger1.clone()))
            .trigger((trigger2.id(), trigger2.clone()))
            .task((t1.id(), t1.clone()))
            .task((t2.id(), t2.clone()))
            .build()
            .unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        let trigger1_result = TriggerResult::builder()
            .outputs([("out", Value::String("trigger1_output"))])
            .build()
            .unwrap();
        let result = exec.execute_from_trigger(trigger1.id(), trigger1_result);
        assert!(result.is_ok());
        let reachable = exec.reachable_tasks(&trigger1);
        let task_ids: Vec<_> = reachable
            .iter()
            .filter_map(|d| {
                if let DependencyRef::Task(id) = d {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        assert!(task_ids.contains(&t1.id()));
        assert!(!task_ids.contains(&t2.id()));
    }

    #[test]
    fn test_missing_dependency_output_error() {
        let reg = setup_registry();
        let trigger_def = dummy_trigger_def("trig", Some(("out", DataType::String)));
        let trigger = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        let t1_def = dummy_task_def("t1", Some(("in", DataType::String)), None);
        let t1 = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&t1_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger.id()),
                    output: "missing_output", // output key does not exist
                },
            ))
            .build()
            .unwrap();
        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger.id(), trigger.clone()))
            .task((t1.id(), t1.clone()))
            .build()
            .unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        let result = exec.execute_from_trigger(trigger.id(), TriggerResult::default());
        match result {
            Err(e) => {
                let err_str = format!("{:?}", e);
                assert!(
                    err_str.contains("MissingDependencyOutput"),
                    "Expected MissingDependencyOutput error, got: {}",
                    err_str
                );
            }
            Ok(_) => panic!("Expected error due to missing dependency output, but got Ok"),
        }
    }

    #[test]
    fn test_failed_dependency_output_error() {
        let reg = setup_registry();
        let trigger_def = dummy_trigger_def("trig", None); // No outputs
        let trigger = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        let t1_def = dummy_task_def("t1", Some(("in", DataType::String)), None);
        let t1 = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&t1_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger.id()),
                    output: "out", // output key does not exist
                },
            ))
            .build()
            .unwrap();
        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger.id(), trigger.clone()))
            .task((t1.id(), t1.clone()))
            .build()
            .unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        let result = exec.execute_from_trigger(trigger.id(), TriggerResult::default());
        match result {
            Err(e) => {
                let err_str = format!("{:?}", e);
                assert!(
                    err_str.contains("FailedDependencyOutput")
                        || err_str.contains("MissingDependencyOutput"),
                    "Expected FailedDependencyOutput or MissingDependencyOutput error, got: {}",
                    err_str
                );
            }
            Ok(_) => panic!("Expected error due to failed dependency output, but got Ok"),
        }
    }

    #[test]
    fn test_trigger_output_propagation_to_task() {
        let reg = setup_registry();
        let trigger_def = dummy_trigger_def("trig", Some(("out", DataType::String)));
        let trigger = Trigger::builder()
            .id(Uuid::new_v4())
            .name("trig")
            .definition(&trigger_def)
            .build()
            .unwrap();
        let t1_def = dummy_task_def(
            "t1",
            Some(("in", DataType::String)),
            Some(("out", DataType::String)),
        );
        let t1 = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&t1_def)
            .input((
                "in",
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger.id()),
                    output: "out",
                },
            ))
            .build()
            .unwrap();
        let workflow = Workflow::builder()
            .name("wf")
            .trigger((trigger.id(), trigger.clone()))
            .task((t1.id(), t1.clone()))
            .build()
            .unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&workflow)
            .plugins(&reg)
            .build()
            .unwrap();
        // Provide a trigger result with output
        let mut outputs = HashMap::new();
        outputs.insert("out", Value::String("hello"));
        let trigger_result = TriggerResult::builder().outputs(outputs).build().unwrap();
        let result = exec.execute_from_trigger(trigger.id(), trigger_result);
        assert!(
            result.is_ok(),
            "Expected Ok for successful output propagation"
        );
        // Check that t1 received the correct input
        let t1_result = exec.get_task_result(&t1.id());
        assert!(t1_result.is_some(), "Task result should be present");
        let t1_outputs = t1_result.unwrap().outputs();
        assert!(
            t1_outputs.contains_key("out"),
            "Task output should be present"
        );
    }
}
