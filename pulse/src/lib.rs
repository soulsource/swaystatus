use swaystatus_plugin::*;

mod runnable;
mod config;
mod communication;

use config::*;

pub struct PulseVolumePlugin;
impl SwayStatusModule for PulseVolumePlugin {
    fn get_name(&self) -> &str {
        "PulseVolume"
    }
    fn deserialize_config<'de>(&self, deserializer : &mut (dyn erased_serde::Deserializer + 'de)) -> Result<Box<dyn SwayStatusModuleInstance>, erased_serde::Error> {
        let result : PulseVolumeConfig = erased_serde::deserialize(deserializer)?;
        Ok(Box::new(result))
    }
    fn get_default_config(&self) -> Box<dyn SwayStatusModuleInstance> {
        let config = PulseVolumeConfig::default();
        Box::new(config)
    }
    fn print_help(&self) {
        //TODO!
        println!(
r#"Swaystatus Pulseaudio Volume plugin.

Sorry, this help has not been finalized. If this ever gets public, slap Andi."#);
    }
}

impl PulseVolumePlugin {
    fn new() -> Self {
        Self
    }
}

declare_swaystatus_module!(PulseVolumePlugin, PulseVolumePlugin::new);
