use std::collections::HashMap;

use derive_builder::Builder;
use derive_new::new;
use getset::Getters;

use crate::task::DataType;

/// Schema defining the inputs and outputs of a Task in the workflow
#[derive(Getters, Builder, Debug, Default)]
#[builder(pattern = "owned")]
pub struct TaskSchema<'a> {
    /// Input specifications for the task
    #[getset(get = "pub")]
    #[builder(default, setter(each = "input"))]
    inputs: HashMap<&'a str, InputSpec>,

    /// Output specifications for the task
    #[getset(get = "pub")]
    #[builder(default, setter(each = "output"))]
    outputs: HashMap<&'a str, OutputSpec>,
}

impl<'a> TaskSchema<'a> {
    /// Creates a new builder for TaskSchema
    pub fn builder() -> TaskSchemaBuilder<'a> {
        TaskSchemaBuilder::default()
    }
}

/// Specification for a task input
#[derive(Getters, Debug, new)]
pub struct InputSpec {
    /// Data type of the input
    #[getset(get = "pub")]
    data_type: DataType,

    /// Whether the input is required
    #[getset(get = "pub")]
    #[new(default)]
    required: bool,
}

/// Specification for a task output
#[derive(Getters, Debug, new)]
pub struct OutputSpec {
    /// Data type of the output
    #[getset(get = "pub")]
    data_type: DataType,
}
