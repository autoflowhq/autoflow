use derive_builder::Builder;
use getset::{Getters, Setters};

/// Represents the result of evaluating a trigger in the workflow
#[derive(Builder, Getters, Setters, Default)]
#[builder(pattern = "owned", setter(into))]
pub struct TriggerResult {}
