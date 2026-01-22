use std::collections::HashMap;

use derive_new::new;
use getset::Getters;
use petgraph::{
    algo::toposort,
    graph::{DiGraph, NodeIndex},
};

use crate::{
    executor::{ExecutionError, Result},
    workflow::DependencyRef,
};

/// Represents the execution graph of dependencies in a workflow
#[derive(new, Getters)]
pub struct ExecutionGraph {
    /// Directed graph representing dependencies
    #[getset(get = "pub")]
    graph: DiGraph<DependencyRef, ()>,

    /// Mapping from DependencyRef to their corresponding NodeIndex in the graph
    #[getset(get = "pub")]
    index_map: HashMap<DependencyRef, NodeIndex>,
}

impl ExecutionGraph {
    /// Performs a topological sort of the execution graph
    pub fn topological_sort(&self) -> Result<Vec<DependencyRef>> {
        let sorted_indices =
            toposort(&self.graph, None).map_err(|_| ExecutionError::DependencyCycle)?;

        let sorted_refs = sorted_indices
            .into_iter()
            .map(|index| {
                self.graph
                    .node_weight(index)
                    .cloned()
                    .expect("Node index should exist in the graph")
            })
            .collect();

        Ok(sorted_refs)
    }
}
