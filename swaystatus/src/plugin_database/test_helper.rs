use std::collections::HashMap;
use crate::test_plugin::TestPlugin;
use swaystatus_plugin::SwayStatusModule;
use super::PluginDatabase;

pub fn get_plugin_database_with_test_plugin() -> PluginDatabase<'static> {
    PluginDatabase { plugins : {
    let mut m = HashMap::new();
    m.insert(String::from(TestPlugin.get_name()),
        Box::new(TestPlugin) as Box<dyn SwayStatusModule>);
        m
    }}
}
pub fn get_plugin_database_empty() -> PluginDatabase<'static> {
    PluginDatabase { plugins : HashMap::new() }
}
