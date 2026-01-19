use derive_builder::Builder;
use getset::{CopyGetters, Getters, Setters};

/// Context provided to a trigger when it is evaluated in the workflow
#[derive(Getters, Setters, CopyGetters, Builder)]
#[builder(pattern = "owned")]
pub struct TriggerContext<'a> {
    /// Name of the trigger being evaluated
    #[getset(get_copy = "pub")]
    trigger_name: &'a str,
}
