use derive_builder::Builder;
use getset::{Getters, Setters};

use crate::trigger::TriggerSchema;

/// Definition of a kind of Trigger within the workflow system.
/// This is independent of any specific instance of a Trigger.
/// A TriggerDefinition defines the name, schema, and the function
/// that is called when the trigger is evaluated.
#[derive(Builder, Getters, Setters, Debug)]
#[builder(pattern = "owned")]
pub struct TriggerDefinition<'a> {
    /// Name of the trigger
    #[getset(get = "pub")]
    name: &'a str,

    /// Schema defining inputs and outputs of the trigger
    #[builder(default)]
    #[getset(get = "pub")]
    schema: TriggerSchema<'a>,
}

impl<'a> TriggerDefinition<'a> {
    /// Creates a new builder for TriggerDefinition
    pub fn builder() -> TriggerDefinitionBuilder<'a> {
        TriggerDefinitionBuilder::default()
    }
}
