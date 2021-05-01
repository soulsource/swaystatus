use std::fmt;
use serde::{Serialize, Deserialize, Deserializer};
use serde::de::{self, Visitor, DeserializeSeed, MapAccess, SeqAccess, Error};
use super::plugin_database::PluginDatabase;
use super::plugin;

mod custom_deserializers;

#[cfg(test)]
mod tests;

/**
 * Struct that holds configuration options specific to the main program.
 * This is where new config options should go, because it inherits Serialize/Deserialize.
 */
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct SwaystatusMainConfig {
    pub separator : String
}
/**
 * Helper struct for global configuration. Holds a list of element configurations.
 * This is what goes into the config file or is read from it. Needs a manual deserialize
 * implementation, to allow routing a seed through.
 */
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub struct SwaystatusConfig<'p> {
    ///Settings for the main part of the program.
    #[serde(rename = "Settings")]
    pub settings : Option<SwaystatusMainConfig>,
    ///Settings for each part of the output sting.
    #[serde(rename = "Element")]
    pub elements : Option<Vec<SwaystatusPluginConfig<'p>>>,
}

/**
 * Helper struct with custom deserializer. Holds config for a single element.
 * This is its own struct to make serialization/deserialization easier to maintain.
 */
#[derive(Serialize)]
#[serde(deny_unknown_fields)]
pub struct SwaystatusPluginConfig<'p> {
    #[serde(rename = "Plugin")]
    plugin : String,
    #[serde(rename = "Config")]
    config : Box<dyn plugin::SwayStatusModuleInstance + 'p>,
    #[serde(rename = "General")]
    general : SwaystatusElementNonPluginOptions
}

/**
 * Struct containing the per-element options that are not plugin-specific.
 */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, default, rename_all="PascalCase")]
pub struct SwaystatusElementNonPluginOptions {
    pub before_text : String,
    pub after_text : String
}

impl Default for SwaystatusElementNonPluginOptions {
    fn default() -> Self {
        SwaystatusElementNonPluginOptions{
            before_text : String::new(),
            after_text : String::new() 
        }
    }
}

impl<'p> SwaystatusConfig<'p> {
    fn serialize(&self) -> Result<String, toml::ser::Error> {
        toml::to_string(self)
    }
    fn deserialize(serialized : &str, plugins : &'p PluginDatabase) -> Result<SwaystatusConfig<'p>, toml::de::Error> {
        let seed = custom_deserializers::SwaystatusConfigDeserializeSeed(plugins);
        let mut deserializer = toml::Deserializer::new(serialized);
        seed.deserialize(&mut deserializer)
    }

    fn create_default(plugins : &'p PluginDatabase) -> SwaystatusConfig<'p> {
        SwaystatusConfig {
            settings : Some(SwaystatusMainConfig::default()),
            elements : {
                let v : Vec<SwaystatusPluginConfig> = 
                    plugins.get_name_and_plugin_iterator().map(|(name, object)| {
                        SwaystatusPluginConfig{ 
                            general : SwaystatusElementNonPluginOptions::default(),
                            plugin : name.clone(), 
                            config : object.get_default_config()
                        }
                    }).collect();
                if v.is_empty() { None } else { Some(v) }
            }
        }
    }

    pub fn print_sample_config(plugins : &PluginDatabase) {
        let output = SwaystatusConfig::create_default(plugins).serialize().unwrap();
        print!("{}", output);
    }
    pub fn read_config<'d>(path : &'d std::path::Path, plugins : &'p PluginDatabase) -> Result<SwaystatusConfig<'p>,SwaystatusConfigErrors> {
        let config_file = match std::fs::read_to_string(path) {
            Ok(x) => x,
            Err (_) => return Err(SwaystatusConfigErrors::FileNotFound)
        };
        let result = match SwaystatusConfig::deserialize(&config_file, plugins) {
            Ok(x) => x,
            Err(e) => return Err(SwaystatusConfigErrors::ParsingError{message: e.to_string()})
        };

        Ok(result)
    }
}

impl<'p> SwaystatusPluginConfig<'p> {
    pub fn get_instance(&'p self) -> &(dyn plugin::SwayStatusModuleInstance + 'p) {
        &*self.config
    }
    pub fn get_name(&'p self) -> &'p str {
        &self.plugin
    }
    pub fn get_non_plugin_settings(&self) -> &SwaystatusElementNonPluginOptions {
        &self.general

    }
}

pub enum SwaystatusConfigErrors
{
    FileNotFound,
    ParsingError {
        message : String
    }
}

impl Default for SwaystatusMainConfig {
    fn default() -> Self {
        SwaystatusMainConfig { separator : String::from(", ")}
    }
}
