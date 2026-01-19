use std::collections::HashMap;

use derive_builder::Builder;
use getset::Getters;

use crate::task::DataType;

/// Schema defining the outputs of a Trigger in the workflow
#[derive(Default, Getters, Builder)]
#[builder(pattern = "owned")]
pub struct TriggerSchema<'a> {
    /// Output specifications for the trigger
    #[getset(get = "pub")]
    #[builder(default, setter(each = "output"))]
    outputs: HashMap<&'a str, DataType>,
}

impl<'a> TriggerSchema<'a> {
    /// Creates a new builder for TriggerSchema
    pub fn builder() -> TriggerSchemaBuilder<'a> {
        TriggerSchemaBuilder::default()
    }
}
