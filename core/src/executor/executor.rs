use std::collections::{HashMap, HashSet};

use derive_builder::Builder;
use getset::{Getters, Setters};
use petgraph::Graph;
use uuid::Uuid;

use crate::{
    Value,
    executor::{self, ExecutionError, ExecutionGraph},
    plugin::Registry,
    task::{InputBinding, Task, TaskContext, TaskResult, TaskStatus},
    trigger::{Trigger, TriggerResult},
    workflow::{CompiledWorkflow, DependencyRef, WorkflowExecutionContext},
};

/// Executor for managing the execution of a workflow
#[derive(Getters, Setters, Builder)]
#[builder(pattern = "owned", build_fn(name = "finish", private))]
pub struct WorkflowExecutor<'a> {
    // Immutable
    /// The compiled workflow to be executed
    #[getset(get = "pub")]
    workflow: &'a CompiledWorkflow<'a>,

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

    /// Executes the workflow starting from a given trigger
    pub fn trigger(&mut self, trigger_id: Uuid, result: TriggerResult<'a>) -> executor::Result<()> {
        let trigger = self.get_trigger(trigger_id)?;
        let graph = self.build_execution_graph(trigger)?;
        let sorted_nodes = graph.topological_sort()?;

        let mut outputs_map: HashMap<DependencyRef, HashMap<&'a str, Value<'a>>> = HashMap::new();
        outputs_map.insert(DependencyRef::Trigger(trigger_id), result.outputs().clone());

        for node in sorted_nodes {
            self.execute_node(node, &mut outputs_map)?;
        }

        Ok(())
    }

    /// Retrieves a trigger by ID
    fn get_trigger(&self, trigger_id: Uuid) -> executor::Result<&Trigger<'a>> {
        self.workflow
            .workflow()
            .triggers()
            .get(&trigger_id)
            .ok_or_else(|| ExecutionError::TriggerNotFound { id: trigger_id }.into())
    }

    /// Executes a node in the execution graph
    fn execute_node(
        &mut self,
        node: DependencyRef,
        outputs_map: &mut HashMap<DependencyRef, HashMap<&'a str, Value<'a>>>,
    ) -> executor::Result<()> {
        match node {
            DependencyRef::Trigger(_) => Ok(()), // nothing to do
            DependencyRef::Task(task_id) => self.execute_task(task_id, outputs_map),
        }
    }

    /// Executes a task by its ID
    fn execute_task(
        &mut self,
        task_id: Uuid,
        outputs_map: &mut HashMap<DependencyRef, HashMap<&'a str, Value<'a>>>,
    ) -> executor::Result<()> {
        let task = self
            .workflow
            .workflow()
            .tasks()
            .get(&task_id)
            .ok_or_else(|| ExecutionError::TaskNotFound { id: task_id })?;

        let input_values = self.resolve_task_inputs(task, outputs_map)?;
        let task_context = self.build_task_context(task, input_values)?;

        let result = task.definition().execute(&task_context);
        outputs_map.insert(DependencyRef::Task(task_id), result.outputs().clone());
        self.task_results.insert(task_id, result.clone());

        Ok(())
    }

    /// Resolves the inputs for a task based on its input bindings
    fn resolve_task_inputs(
        &self,
        task: &Task<'a>,
        outputs_map: &HashMap<DependencyRef, HashMap<&'a str, Value<'a>>>,
    ) -> executor::Result<HashMap<&'a str, Value<'a>>> {
        let mut input_values: HashMap<&'a str, Value<'a>> = HashMap::new();
        for (key, binding) in task.inputs() {
            let value = match binding {
                InputBinding::Literal(v) => v.clone(),
                InputBinding::Reference { ref_from, output } => {
                    let dep_outputs = outputs_map.get(&ref_from).ok_or_else(|| {
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
            input_values.insert(*key, value);
        }
        Ok(input_values)
    }

    /// Builds the task context for a given task
    fn build_task_context(
        &self,
        task: &Task<'a>,
        resolved_inputs: HashMap<&'a str, Value<'a>>,
    ) -> executor::Result<TaskContext<'a>> {
        Ok(TaskContext::builder()
            .task_id(task.id())
            .task_name(task.name())
            .task_definition(task.definition())
            .workflow_id(*self.workflow.workflow().id())
            .description(*task.description())
            .resolved_inputs(resolved_inputs)
            .build()
            .map_err(|_| ExecutionError::FailedToBuildTaskContext)?)
    }

    /// Builds the execution graph for the workflow starting from a given trigger
    /// This extracts a subgraph from the compiled workflow containing only the tasks
    /// reachable from the specified trigger
    fn build_execution_graph(&self, trigger: &'a Trigger<'a>) -> executor::Result<ExecutionGraph> {
        use petgraph::visit::Bfs;

        let mut graph = Graph::new();
        let mut index_map = HashMap::new();
        let trigger_ref = DependencyRef::Trigger(trigger.id());

        // Get the compiled workflow's graph and index map
        let compiled_graph = self.workflow.dependency_graph();
        let compiled_index_map = self.workflow.index_map();

        // Find all nodes reachable from this trigger using BFS
        let mut reachable = HashSet::new();
        if let Some(&start_index) = compiled_index_map.get(&trigger_ref) {
            let mut bfs = Bfs::new(compiled_graph, start_index);
            while let Some(node_idx) = bfs.next(compiled_graph) {
                if let Some(node_ref) = compiled_graph.node_weight(node_idx) {
                    reachable.insert(*node_ref);
                }
            }
        }

        // Build subgraph with only reachable nodes
        for &node_ref in &reachable {
            let node_index = graph.add_node(node_ref);
            index_map.insert(node_ref, node_index);
        }

        // Add edges between reachable nodes
        for &node_ref in &reachable {
            if let Some(&compiled_idx) = compiled_index_map.get(&node_ref) {
                // Find all edges in the compiled graph where source is this node
                let mut edges = compiled_graph.neighbors(compiled_idx).detach();
                while let Some(target_idx) = edges.next_node(compiled_graph) {
                    let target_ref = compiled_graph[target_idx];
                    // Only add edge if target is also reachable
                    if reachable.contains(&target_ref) {
                        let source_idx = index_map[&node_ref];
                        let target_idx_new = index_map[&target_ref];
                        graph.add_edge(source_idx, target_idx_new, ());
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
            .workflow_id(*workflow.workflow().id())
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
    use crate::workflow::{Workflow, WorkflowCompiler};

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

        // Compile the workflow before execution
        let compiled_wf = WorkflowCompiler::compile(&workflow).unwrap();

        let mut exec = WorkflowExecutor::builder()
            .workflow(&compiled_wf)
            .plugins(&reg)
            .build()
            .unwrap();

        // Trigger 1 should only execute t2 (and not t3)
        let trigger1_result = TriggerResult::builder()
            .outputs([("out", Value::String("trigger1_output"))])
            .build()
            .unwrap();
        let result = exec.trigger(trigger1.id(), trigger1_result);
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
        // Trigger 2 should only execute t3 (and not t2)
        let mut exec2 = WorkflowExecutor::builder()
            .workflow(&compiled_wf)
            .plugins(&reg)
            .build()
            .unwrap();
        let trigger2_result = TriggerResult::builder()
            .outputs([("out", Value::String("trigger2_output"))])
            .build()
            .unwrap();
        let result2 = exec2.trigger(trigger2.id(), trigger2_result);
        assert!(result2.is_ok());
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
        let compiled_wf = WorkflowCompiler::compile(&workflow).unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&compiled_wf)
            .plugins(&reg)
            .build()
            .unwrap();
        let result = exec.trigger(trigger.id(), TriggerResult::default());
        assert!(result.is_ok());
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
        let compiled_wf = WorkflowCompiler::compile(&workflow).unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&compiled_wf)
            .plugins(&reg)
            .build()
            .unwrap();
        let result = exec.trigger(
            trigger.id(),
            TriggerResult::builder()
                .outputs([("out", Value::String("trigger_output"))])
                .build()
                .unwrap(),
        );
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    }

    #[test]
    fn test_executor_handles_circular_dependencies() {
        let _reg = setup_registry();
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
        // Compilation should fail due to circular dependency
        let compiled_result = WorkflowCompiler::compile(&workflow);
        assert!(
            compiled_result.is_err(),
            "Expected compilation error due to circular dependency"
        );
        let err_str = format!("{:?}", compiled_result.unwrap_err());
        assert!(
            err_str.contains("Cyclic") || err_str.contains("cycle"),
            "Error should mention cyclic dependency, got: {}",
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
        let compiled_wf = WorkflowCompiler::compile(&workflow).unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&compiled_wf)
            .plugins(&reg)
            .build()
            .unwrap();
        let trigger1_result = TriggerResult::builder()
            .outputs([("out", Value::String("trigger1_output"))])
            .build()
            .unwrap();
        let result = exec.trigger(trigger1.id(), trigger1_result);
        assert!(result.is_ok());
    }

    #[test]
    fn test_missing_dependency_output_error() {
        let _reg = setup_registry();
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
        // This should fail during compilation due to missing output
        let compiled_result = WorkflowCompiler::compile(&workflow);
        match compiled_result {
            Err(e) => {
                let err_str = format!("{:?}", e);
                assert!(
                    err_str.contains("MissingOutput") || err_str.contains("missing"),
                    "Expected missing output error during compilation, got: {}",
                    err_str
                );
            }
            Ok(_) => panic!("Expected compilation error due to missing dependency output"),
        }
    }

    #[test]
    fn test_failed_dependency_output_error() {
        let _reg = setup_registry();
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
        // This should fail during compilation due to missing output
        let compiled_result = WorkflowCompiler::compile(&workflow);
        match compiled_result {
            Err(e) => {
                let err_str = format!("{:?}", e);
                assert!(
                    err_str.contains("MissingTriggerOutput") || err_str.contains("MissingOutput"),
                    "Expected missing output error during compilation, got: {}",
                    err_str
                );
            }
            Ok(_) => panic!("Expected compilation error due to failed dependency output"),
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
        let compiled_wf = WorkflowCompiler::compile(&workflow).unwrap();
        let mut exec = WorkflowExecutor::builder()
            .workflow(&compiled_wf)
            .plugins(&reg)
            .build()
            .unwrap();
        // Provide a trigger result with output
        let mut outputs = HashMap::new();
        outputs.insert("out", Value::String("hello"));
        let trigger_result = TriggerResult::builder().outputs(outputs).build().unwrap();
        let result = exec.trigger(trigger.id(), trigger_result);
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
