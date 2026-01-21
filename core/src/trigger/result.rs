use std::collections::HashMap;

use derive_builder::Builder;
use getset::{Getters, Setters};

use crate::Value;

/// Represents the result of evaluating a trigger in the workflow
#[derive(Builder, Getters, Setters, Default, Clone)]
#[builder(pattern = "owned", setter(into))]
pub struct TriggerResult<'a> {
    /// outputs produced by the trigger
    #[getset(get = "pub")]
    #[builder(default, setter(each = "output"))]
    pub outputs: HashMap<&'a str, Value<'a>>,
}

impl<'a> TriggerResult<'a> {
    /// Creates a new builder for TriggerResult
    pub fn builder() -> TriggerResultBuilder<'a> {
        TriggerResultBuilder::default()
    }
}
