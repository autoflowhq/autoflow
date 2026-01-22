use std::collections::HashMap;

use petgraph::graph::{DiGraph, NodeIndex};

use crate::{
    compiler::CompileError,
    compiler::Result,
    task::{DataType, InputBinding, Task},
    workflow::{DependencyRef, Workflow},
};

/// Compiled and validated workflow ready for execution
#[derive(Debug)]
pub struct CompiledWorkflow<'a> {
    workflow: &'a Workflow<'a>,
    /// Dependency graph showing which tasks depend on which (directed edges)
    dependency_graph: DiGraph<DependencyRef, ()>,
    /// Mapping from DependencyRef to their corresponding NodeIndex in the graph
    index_map: HashMap<DependencyRef, NodeIndex>,
}

impl<'a> CompiledWorkflow<'a> {
    /// Get the underlying workflow
    pub fn workflow(&self) -> &'a Workflow<'a> {
        self.workflow
    }

    /// Get the dependency graph
    pub fn dependency_graph(&self) -> &DiGraph<DependencyRef, ()> {
        &self.dependency_graph
    }

    /// Get the index map
    pub fn index_map(&self) -> &HashMap<DependencyRef, NodeIndex> {
        &self.index_map
    }
}

/// Workflow compiler responsible for validation, type checking, and cycle detection
pub struct WorkflowCompiler;

impl WorkflowCompiler {
    /// Compile and validate a workflow
    ///
    /// This performs:
    /// - Dependency existence validation
    /// - Type checking for all input bindings
    /// - Cycle detection in task dependencies
    /// - Building the dependency graph
    pub fn compile<'a>(workflow: &'a Workflow<'a>) -> Result<CompiledWorkflow<'a>> {
        let compiler = Self;

        // Validate all tasks
        for task in workflow.tasks().values() {
            compiler.verify_task_dependencies(workflow, task)?;
            compiler.validate_task_input_types(workflow, task)?;
        }

        // Build dependency graph (also detects cycles during construction)
        let (dependency_graph, index_map) = compiler.build_dependency_graph(workflow)?;

        Ok(CompiledWorkflow {
            workflow,
            dependency_graph,
            index_map,
        })
    }

    /// Verify that all dependencies of a task exist within the workflow
    fn verify_task_dependencies(&self, workflow: &Workflow, task: &Task) -> Result<()> {
        // Check explicit dependencies
        for dep in task.dependencies() {
            match dep {
                DependencyRef::Task(dep_id) => {
                    if !workflow.tasks().contains_key(dep_id) {
                        return Err(CompileError::MissingDependency { id: *dep_id });
                    }
                }
                DependencyRef::Trigger(dep_id) => {
                    if !workflow.triggers().contains_key(dep_id) {
                        return Err(CompileError::MissingDependency { id: *dep_id });
                    }
                }
            }
        }

        // Check input binding dependencies
        for (input, binding) in task.inputs() {
            match binding {
                InputBinding::Reference {
                    ref_from: DependencyRef::Task(task_id),
                    output,
                } => {
                    if !workflow.tasks().contains_key(task_id) {
                        return Err(CompileError::MissingTaskDependency {
                            input: input.to_string(),
                            id: *task_id,
                        });
                    }

                    let referenced_task = workflow.tasks().get(task_id).unwrap();
                    if !referenced_task
                        .definition()
                        .schema()
                        .outputs()
                        .contains_key(output)
                    {
                        return Err(CompileError::MissingTaskOutput {
                            input: input.to_string(),
                            output: output.to_string(),
                            id: *task_id,
                        });
                    }
                }
                InputBinding::Reference {
                    ref_from: DependencyRef::Trigger(trigger_id),
                    output,
                } => {
                    if !workflow.triggers().contains_key(trigger_id) {
                        return Err(CompileError::MissingTriggerDependency {
                            input: input.to_string(),
                            id: *trigger_id,
                        });
                    }

                    let trigger = workflow.triggers().get(trigger_id).unwrap();
                    if !trigger.definition().schema().outputs().contains_key(output) {
                        return Err(CompileError::MissingTriggerOutput {
                            input: input.to_string(),
                            output: output.to_string(),
                            id: *trigger_id,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Validate that all input types for a task match the types of the referenced outputs
    fn validate_task_input_types(&self, workflow: &Workflow, task: &Task) -> Result<()> {
        for (input_key, binding) in task.inputs() {
            if let InputBinding::Reference { ref_from, output } = binding {
                self.validate_reference(workflow, task, input_key, *ref_from, output)?;
            }
        }
        Ok(())
    }

    /// Validate a single reference input against its source entity (task or trigger)
    fn validate_reference(
        &self,
        workflow: &Workflow,
        task: &Task,
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
            .ok_or_else(|| CompileError::InputNotInSchema {
                task_id: task.id(),
                input: input_key.to_string(),
            })?;

        // Get the referenced output spec
        let output_type: &DataType = match ref_from {
            DependencyRef::Task(id) => {
                let referenced_task = workflow.tasks().get(&id).ok_or_else(|| {
                    CompileError::MissingTaskDependency {
                        input: input_key.to_string(),
                        id,
                    }
                })?;
                referenced_task
                    .definition()
                    .schema()
                    .outputs()
                    .get(output_key)
                    .ok_or_else(|| CompileError::MissingTaskOutput {
                        input: input_key.to_string(),
                        output: output_key.to_string(),
                        id,
                    })?
                    .data_type()
            }
            DependencyRef::Trigger(id) => {
                let referenced_trigger = workflow.triggers().get(&id).ok_or_else(|| {
                    CompileError::MissingTriggerDependency {
                        input: input_key.to_string(),
                        id,
                    }
                })?;
                referenced_trigger
                    .definition()
                    .schema()
                    .outputs()
                    .get(output_key)
                    .ok_or_else(|| CompileError::MissingTriggerOutput {
                        input: input_key.to_string(),
                        output: output_key.to_string(),
                        id,
                    })?
            }
        };

        // Compare data types
        if input_spec.data_type() != output_type {
            return Err(CompileError::InputTypeMismatch {
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

    /// Build the dependency graph for the workflow using petgraph
    /// This creates a directed graph where edges point from dependencies to dependents
    /// Also performs cycle detection using petgraph's toposort
    fn build_dependency_graph(
        &self,
        workflow: &Workflow,
    ) -> Result<(
        DiGraph<DependencyRef, ()>,
        HashMap<DependencyRef, NodeIndex>,
    )> {
        let mut graph = DiGraph::new();
        let mut index_map = HashMap::new();

        // Add all triggers as nodes
        for trigger_id in workflow.triggers().keys() {
            let trigger_ref = DependencyRef::Trigger(*trigger_id);
            let node_index = graph.add_node(trigger_ref);
            index_map.insert(trigger_ref, node_index);
        }

        // Add all tasks as nodes
        for task_id in workflow.tasks().keys() {
            let task_ref = DependencyRef::Task(*task_id);
            let node_index = graph.add_node(task_ref);
            index_map.insert(task_ref, node_index);
        }

        // Add edges: from dependency to dependent (source -> target)
        for task in workflow.tasks().values() {
            let task_ref = DependencyRef::Task(task.id());
            let task_index = *index_map.get(&task_ref).unwrap();

            // Get all dependencies for this task
            let dependencies = task.get_all_dependencies();

            for dep_ref in dependencies {
                if let Some(&dep_index) = index_map.get(&dep_ref) {
                    // Add edge from dependency to task (dep -> task)
                    graph.add_edge(dep_index, task_index, ());
                }
            }
        }

        // Detect cycles using topological sort
        use petgraph::algo::toposort;
        if let Err(cycle) = toposort(&graph, None) {
            // Extract the node involved in the cycle
            let node_ref = graph[cycle.node_id()];
            return Err(CompileError::CyclicDependency {
                from: node_ref.id(),
                to: node_ref.id(),
            });
        }

        Ok((graph, index_map))
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::task::{
        DataType, InputSpec, NoOpTaskHandler, OutputSpec, Task, TaskDefinition, TaskSchema,
    };
    use crate::workflow::Workflow;

    fn dummy_task_def<'a>() -> TaskDefinition<'a> {
        TaskDefinition::builder()
            .name("dummy")
            .schema(
                TaskSchema::builder()
                    .input(("in", InputSpec::new(DataType::String)))
                    .output(("out", OutputSpec::new(DataType::String)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(NoOpTaskHandler))
            .build()
            .unwrap()
    }

    #[test]
    fn test_compile_valid_workflow() {
        let def = dummy_task_def();
        let t1 = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&def)
            .build()
            .unwrap();

        let mut wf = Workflow::builder().name("wf").build().unwrap();
        wf.add_task(t1);

        let result = WorkflowCompiler::compile(&wf);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_missing_dependency() {
        let def = dummy_task_def();
        let mut task = Task::builder()
            .id(Uuid::new_v4())
            .name("t1")
            .definition(&def)
            .build()
            .unwrap();

        let missing_id = Uuid::new_v4();
        task.add_dependency(DependencyRef::Task(missing_id));

        let mut wf = Workflow::builder().name("wf").build().unwrap();
        wf.add_task(task);

        let result = WorkflowCompiler::compile(&wf);
        assert!(result.is_err());
    }
}
