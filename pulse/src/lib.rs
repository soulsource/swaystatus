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
        println!(
r#"Swaystatus Pulseaudio Volume plugin.

This is a volume display for pulseaudio (or compatibles). It can either monitor the default sink and switch whenever the default changes, or it can monitor a specific sink given by a configured name. The volume information can either be printed numerically or as a symbol that can be set based on a percentage range. In addition, the mute, volume and balance display can be arranged in any way wanted.

The configuration for this plugin looks like this:
[Element.Config]
Sorting = ["MuteVolumeBalance", "MuteBalanceVolume", "VolumeMuteBalance", "VolumeBalanceMute", "BalanceMuteVolume", "BalanceVolumeMute"]

[Element.Config.Sink]
Sink = ["Default", "Specific"]
SinkName = <if Sink = "Specific": Sink name to observe. Omit if Sink = "Default".>

[Element.Config.Volume]
Format = ["Off", "Numeric", "Binned"]
Label = <if Format != "Off" string to print in front of actual value. Omit if Format = "Off">
DecimalDigits = <if Format = "Numeric" the number of digits after the comma. Omit otherwise.>

[Element.Config.Volume.PercentToSymbolMap]
<if Format = "Binned" this map has to be filled in. It's a set of key-value-pairs where the key denotes the lower limit of a range. For instance writing 20 = "AAA" makes volumes above 20% print AAA. See sample config -s for an example.>

[Element.Config.Balance]
Format = ["Off", "Numeric", "Binned"]
Label = <if Format != "Off" string to print in front of actual value. Omit if Format = "Off">
DecimalDigits = <if Format = "Numeric" the number of digits after the comma. Omit otherwise.>

[Element.Config.Balance.PercentToSymbolMap]
<see Element.Config.Volume.PercentToSymbolMap for details. It's the same thing, just allows negative values>

[Element.Config.Mute]
Format = ["Off", "Symbol"]
Label = <if Format = "Symbol" string to print in front of symbol. Omit if Format = "Off">
MuteSymbol = <if Format = "Symbol" the string to display if muted. Omit if Format = "Off">
UnmuteSymbol = <if Format = "Symbol" the string to display if unmuted. Omit if Format = "Off">

Sorry if this is overly complicated. Using the -s command line switch to get a sample configuration should clarify things.


Thanks to Jason White, whose gist https://gist.github.com/jasonwhite/1df6ee4b5039358701d2 was immensely helpful when it comes to interaction with the pulseaudio daemon."#);
    }
}

impl PulseVolumePlugin {
    fn new() -> Self {
        Self
    }
}

declare_swaystatus_module!(PulseVolumePlugin, PulseVolumePlugin::new);
