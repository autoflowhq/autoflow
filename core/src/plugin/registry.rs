use derive_new::new;
use getset::Getters;

use crate::plugin::Plugin;
use std::collections::HashMap;

#[derive(Default, Getters, new)]
pub struct Registry<'a> {
    #[get = "pub"]
    #[new(default)]
    plugins: HashMap<String, Plugin<'a>>,
}

impl<'a> Registry<'a> {
    pub fn register_plugin(&mut self, plugin: Plugin<'a>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    pub fn get_plugin(&self, name: &str) -> Option<&Plugin<'a>> {
        self.plugins.get(name)
    }

    pub fn remove_plugin(&mut self, name: &str) -> Option<Plugin<'a>> {
        self.plugins.remove(name)
    }

    pub fn list_plugins(&self) -> Vec<&str> {
        self.plugins.keys().map(|k| k.as_str()).collect()
    }
}
