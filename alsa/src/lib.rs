use swaystatus_plugin::*;

mod config;
mod communication;
mod runnable;

use config::AlsaVolumeConfig;

pub struct AlsaVolumePlugin;
impl SwayStatusModule for AlsaVolumePlugin {
    fn get_name(&self) -> &str {
        "AlsaVolume"
    }
    fn deserialize_config<'de, 'p>(&'p self, deserializer : &mut (dyn erased_serde::Deserializer + 'de)) -> Result<Box<dyn SwayStatusModuleInstance + 'p>,erased_serde::Error> {
        erased_serde::deserialize::<AlsaVolumeConfig>(deserializer)
            .map(|c| Box::new(c) as Box<dyn SwayStatusModuleInstance>)
    }
    fn get_default_config<'p>(&'p self) -> Box<dyn SwayStatusModuleInstance + 'p> {
        todo!();
    }
    fn print_help(&self) {
        println!(
r#"Swaystatus Alsa Volume plugin.

This is a volume display for ALSA. You can either choose a specific device, mixer and element to display, or just show the default output's volume."#
        );
    }
}

impl AlsaVolumePlugin {
    fn new() -> Self {
        Self
    }
}

declare_swaystatus_module!(AlsaVolumePlugin, AlsaVolumePlugin::new);
