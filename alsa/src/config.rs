use std::ffi::CString;

use formatable_float::{FormatableFloatValue, KeyBackingTypeMetadata, FormattingError};
use serde::{Serialize, Deserialize};
use swaystatus_plugin::*;

use crate::{runnable::AlsaVolumeRunnable, communication::{SenderForMain, make_sender_for_main}};

#[derive(Serialize, Deserialize)]
pub struct AlsaVolumeConfig{
    pub(crate) device : CString,
    pub(crate) element : CString,
    pub(crate) abstraction : SElemAbstraction,
    sorting: FieldSorting,
    mute: FormatableMute,
    volume: FormatableFloatValue<VolumeKeyVolume>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub(crate) enum SElemAbstraction{
    None,
    Basic,
}


#[derive(Serialize, Deserialize)]
enum FieldSorting {
    MuteVolume,
    VolumeMute,
}

impl AlsaVolumeConfig {
    pub(crate) fn format_volume(&self, volume : f32, mute : bool) -> Result<String,FormattingError> {
        let formatted_mute = self.mute.format_mute(mute).unwrap_or(String::new());
        let join_strings = |v : String,m : String| match self.sorting {
            FieldSorting::MuteVolume => m + &v,
            FieldSorting::VolumeMute => v + &m,
        };
        match self.volume.format_float(volume)
        {
            Ok(v) => Ok(join_strings(v.unwrap_or_default(), formatted_mute)),
            Err(FormattingError::EmptyMap { numeric_fallback }) => Err(FormattingError::EmptyMap { numeric_fallback: join_strings(numeric_fallback, formatted_mute) }),
        }
    }
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
impl KeyBackingTypeMetadata for VolumeKeyVolume{
    type BackingType = u8;
    const MIN : Self::BackingType = 0;
    const MAX : Self::BackingType = 100;
    const FLOAT_MIN : f32 = 0.0;
    const FLOAT_MAX : f32 = 1.0;
}


impl SwayStatusModuleInstance for AlsaVolumeConfig {
    fn make_runnable<'p>(&'p self, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>) {
        if let Ok((s,r)) = make_sender_for_main() {
            (Box::new(AlsaVolumeRunnable::new(to_main, r, self)), Box::new(SenderForMain::new(s)))
        } else {
            to_main.send_update(Err(PluginError::ShowInsteadOfText("Pipe creation failed. Call your plumber.".to_owned())))
                .expect("Tried to send an error to main, but main is not listening any more.");
            panic!("Pipe creation failed. Call your plumber.")
        }
    }
}

impl Default for AlsaVolumeConfig {
    fn default() -> Self {
        Self {
            device: CString::new("default").unwrap(),
            element: CString::new("Master").unwrap(),
            abstraction : SElemAbstraction::None,
            volume: FormatableFloatValue::Numeric { label: " ".into(), digits: 0 },
            mute: FormatableMute::Symbol { label : String::new(), mute_symbol : String::from("🔇"), unmute_symbol : String::from("🔊") },
            sorting: FieldSorting::MuteVolume,
        }
    }
}