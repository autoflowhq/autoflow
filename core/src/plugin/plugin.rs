use derive_builder::Builder;
use getset::{Getters, Setters};
use uuid::Uuid;

use crate::{task::TaskDefinition, trigger::TriggerDefinition};
use std::collections::HashMap;

/// A Plugin encapsulates a set of tasks and triggers that can be registered and used within
/// the workflow system.
#[derive(Getters, Setters, Builder)]
#[builder(pattern = "owned")]
pub struct Plugin<'a> {
    /// Unique identifier for the plugin
    #[getset(get = "pub")]
    #[builder(default = "Uuid::new_v4()")]
    id: Uuid,

    /// Name of the plugin
    #[getset(get = "pub", set = "pub")]
    name: &'a str,

    /// Tasks provided by the plugin
    #[getset(get = "pub")]
    #[builder(default)]
    tasks: HashMap<&'a str, TaskDefinition<'a>>,

    /// Triggers provided by the plugin
    #[getset(get = "pub")]
    #[builder(default)]
    triggers: HashMap<&'a str, TriggerDefinition<'a>>,
}

impl<'a> Plugin<'a> {
    /// Create a new Plugin builder
    pub fn builder() -> PluginBuilder<'a> {
        PluginBuilder::default()
    }

    /// Add a task to this plugin
    pub fn add_task(mut self, task: TaskDefinition<'a>) -> Self {
        self.tasks.insert(task.name(), task);
        self
    }

    /// Add a trigger to this plugin
    pub fn add_trigger(mut self, trigger: TriggerDefinition<'a>) -> Self {
        self.triggers.insert(trigger.name(), trigger);
        self
    }

    /// Get a task by name
    pub fn get_task(&self, name: &str) -> Option<&TaskDefinition<'a>> {
        self.tasks.get(name)
    }

    /// Get a trigger by name
    pub fn get_trigger(&self, name: &str) -> Option<&TriggerDefinition<'a>> {
        self.triggers.get(name)
    }

    /// List all task names
    pub fn list_tasks(&self) -> Vec<&'a str> {
        self.tasks.keys().cloned().collect()
    }

    /// List all trigger names
    pub fn list_triggers(&self) -> Vec<&'a str> {
        self.triggers.keys().cloned().collect()
    }
}

impl<'a> PluginBuilder<'a> {
    /// Add a task using its name as the key
    pub fn task(mut self, task_def: TaskDefinition<'a>) -> Self {
        let name = task_def.name();
        self.tasks
            .get_or_insert_with(HashMap::new)
            .insert(name, task_def);
        self
    }

    /// Add a trigger using its name as the key
    pub fn trigger(mut self, trigger_def: TriggerDefinition<'a>) -> Self {
        let name = trigger_def.name();
        self.triggers
            .get_or_insert_with(HashMap::new)
            .insert(name, trigger_def);
        self
    }
}
