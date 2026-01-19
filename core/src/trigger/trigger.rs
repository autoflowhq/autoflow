use std::collections::HashMap;

use derive_builder::Builder;
use getset::{CopyGetters, Getters, Setters};
use uuid::Uuid;

use crate::{Value, trigger::TriggerDefinition};

/// Represents a Trigger in the workflow
#[derive(Getters, Setters, CopyGetters, Builder, Clone)]
#[builder(pattern = "owned", setter(into))]
pub struct Trigger<'a> {
    /// Unique identifier for the trigger
    #[getset(get_copy = "pub")]
    id: Uuid,

    /// Name of the trigger
    #[getset(get = "pub", set = "pub")]
    name: &'a str,

    /// Optional description of the trigger
    #[getset(get = "pub", set = "pub")]
    #[builder(default)]
    description: Option<&'a str>,

    /// Definition of the trigger
    #[getset(get = "pub")]
    definition: &'a TriggerDefinition<'a>,

    /// Input bindings for the trigger
    #[getset(get = "pub")]
    #[builder(default, setter(each = "input"))]
    inputs: HashMap<&'a str, Value<'a>>,
}

impl<'a> Trigger<'a> {
    /// Creates a new builder for Trigger
    pub fn builder() -> TriggerBuilder<'a> {
        TriggerBuilder::default()
    }
}
