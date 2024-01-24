use std::collections::BTreeMap;

use serde::{Serialize,Deserialize};
use swaystatus_plugin::*;
use formatable_float::{FormatableFloatValue, FormattingError, KeyBackingTypeMetadata, FormatableFloatKey};

#[derive(Serialize, Deserialize)]
#[serde(tag = "Sink")]
pub(crate) enum Sink {
    Default,
    Specific {
        #[serde(rename = "SinkName")]
        sink_name : String
    }
}

#[derive(Serialize, Deserialize)]
enum FieldSorting {
    MuteVolumeBalance,
    MuteBalanceVolume,
    VolumeMuteBalance,
    VolumeBalanceMute,
    BalanceMuteVolume,
    BalanceVolumeMute,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "Format")]
enum FormatableMute {
    Off,
    Symbol {
        #[serde(rename = "Label")]
        label : String,
        #[serde(rename = "MuteSymbol")]
        mute_symbol : String,
        #[serde(rename = "UnmuteSymbol")]
        unmute_symbol : String
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PulseVolumeConfig {
    sorting : FieldSorting,
    pub(crate) sink : Sink,
    volume : FormatableFloatValue<VolumeKeyVolume>,
    balance : FormatableFloatValue<VolumeKeyBalance>,
    mute : FormatableMute,
}

impl PulseVolumeConfig {
    pub(crate) fn format_volume(&self, volume : f32, balance : f32, mute : bool) -> Result<String,FormattingError> {
        let formatted_volume = self.volume.format_float(volume);
        let formatted_balance = self.balance.format_float(balance);
        let have_errors_occured = formatted_volume.is_err() || formatted_balance.is_err();
        let formatted_mute_option = self.mute.format_mute(mute);
        let formatted_mute = formatted_mute_option.as_deref().unwrap_or("");
        let get_numeric_fallback = |x| -> Option<String> { 
            match x {
                FormattingError::EmptyMap{ numeric_fallback } => { Some(numeric_fallback) } 
            }
        };
        let formatted_volume = formatted_volume.unwrap_or_else(get_numeric_fallback);
        let formatted_volume = formatted_volume.as_deref().unwrap_or("");
        let formatted_balance = formatted_balance.unwrap_or_else(get_numeric_fallback);
        let formatted_balance = formatted_balance.as_deref().unwrap_or("");

        let sorted_values = match self.sorting {
            FieldSorting::BalanceMuteVolume => {[formatted_balance, formatted_mute, formatted_volume]}
            FieldSorting::BalanceVolumeMute => {[formatted_balance, formatted_volume, formatted_mute]}
            FieldSorting::MuteBalanceVolume => {[formatted_mute, formatted_balance, formatted_volume]}
            FieldSorting::MuteVolumeBalance => {[formatted_mute, formatted_volume, formatted_balance]}
            FieldSorting::VolumeBalanceMute => {[formatted_volume, formatted_balance, formatted_mute]}
            FieldSorting::VolumeMuteBalance => {[formatted_volume, formatted_mute, formatted_balance]}
        };
        let formatted_string = format!("{}{}{}",sorted_values[0], sorted_values[1], sorted_values[2]);
        if have_errors_occured {
            Err(FormattingError::EmptyMap{ numeric_fallback : formatted_string })
        }
        else {
            Ok(formatted_string)
        }
    }
}

impl Default for PulseVolumeConfig {
    fn default() -> Self {
        PulseVolumeConfig {
            sink : Sink::Default,
            volume : FormatableFloatValue::Numeric { label : String::from(""), digits : 0 },
            balance : FormatableFloatValue::Binned { 
                label : String::from(" "), 
                bin_symbol_map : {
                    let mut a = BTreeMap::new(); 
                    a.insert(FormatableFloatKey(-100),String::from("|.."));
                    a.insert(FormatableFloatKey(-10), String::from(".|."));
                    a.insert(FormatableFloatKey(10), String::from("..|"));
                    a
                }
            },
            mute : FormatableMute::Symbol { label : String::new(), mute_symbol : String::from("🔇"), unmute_symbol : String::from("🔊") },
            sorting : FieldSorting::MuteVolumeBalance,
        }
    }
}

impl SwayStatusModuleInstance for PulseVolumeConfig {
    fn make_runnable<'p>(&'p self,to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>) {
        let (runnable, sender_for_main) = crate::runnable::PulseVolumeRunnable::new(self, to_main);
        (Box::new(runnable), Box::new(sender_for_main))
    }
}

impl FormatableMute {
    fn format_mute(&self, mute : bool) -> Option<String> {
        match self {
            FormatableMute::Off => { None }
            FormatableMute::Symbol{ label, mute_symbol, unmute_symbol}  => { Some(format!("{}{}", label, { if mute { mute_symbol } else { unmute_symbol }}))}
        }
    }
}
#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct VolumeKeyVolume;
#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct VolumeKeyBalance;

impl KeyBackingTypeMetadata for VolumeKeyVolume{
    type BackingType = u8;
    const MIN : Self::BackingType = 0;
    const MAX : Self::BackingType = 100;
    const FLOAT_MIN : f32 = 0.0;
    const FLOAT_MAX : f32 = 1.0;
}

impl KeyBackingTypeMetadata for VolumeKeyBalance{
    type BackingType = i8;
    const MIN : Self::BackingType = -100;
    const MAX : Self::BackingType = 100;
    const FLOAT_MIN : f32 = -1.0;
    const FLOAT_MAX : f32 = 1.0;
}
