use serde::{Serialize,Deserialize,Serializer,Deserializer};
use serde::de::Error as DeError;
use serde::de::Unexpected as DeUnexpect;
use std::collections::BTreeMap;
use swaystatus_plugin::*;
use std::ops::{Add, Sub};
use std::str::FromStr;
use std::num::{ParseIntError,IntErrorKind};


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
#[serde(tag = "Format")]
enum FormatableVolume<KeyTypeMetadata : VolumeKeyBackingTypeMetadata> {
    Off,
    Numeric {
        #[serde(rename = "Label")]
        label : String,
        #[serde(rename = "DecimalDigits")]
        digits : u8
    },
    Binned {
        #[serde(rename = "Label")]
        label: String,
        #[serde(rename = "PercentToSymbolMap")]
        bin_symbol_map : BTreeMap<VolumeKey<KeyTypeMetadata>,String>
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
    volume : FormatableVolume<VolumeKeyVolume>,
    balance : FormatableVolume<VolumeKeyBalance>,
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
            volume : FormatableVolume::Numeric { label : String::from(""), digits : 0 },
            balance : FormatableVolume::Binned { 
                label : String::from(" "), 
                bin_symbol_map : {
                    let mut a = BTreeMap::new(); 
                    a.insert(VolumeKey(-100),String::from("|.."));
                    a.insert(VolumeKey(-10), String::from(".|."));
                    a.insert(VolumeKey(10), String::from("..|"));
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
        let (runnable, sender_for_main) = crate::runnable::PulseVolumeRunnable::new(&self, to_main);
        (Box::new(runnable), Box::new(sender_for_main))
    }
}

#[derive(Debug)]
pub(crate) enum FormattingError {
    EmptyMap {
        numeric_fallback : String
    }
}
impl std::fmt::Display for FormattingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormattingError::EmptyMap{numeric_fallback} => { write!(f, "Formatting failed. Empty PercentToSymbolMap. Numeric value: {}", numeric_fallback) }
        }
    }
}
impl std::error::Error for FormattingError {}

impl<KeyTypeMetadata : VolumeKeyBackingTypeMetadata> FormatableVolume<KeyTypeMetadata> {
    fn format_float(&self, float : f32) -> Result<Option<String>, FormattingError> {
        match self {
            FormatableVolume::Numeric{ label, digits } => { Ok(Some(Self::format_float_numeric(float, label, *digits))) }
            FormatableVolume::Binned{ label, bin_symbol_map } => { Some(Self::format_float_binned(float, label, bin_symbol_map)).transpose()}
            FormatableVolume::Off => {Ok(None)}
        }
    }
    fn format_float_binned(float : f32, label : &str, bin_symbol_map : &BTreeMap<VolumeKey<KeyTypeMetadata>, String>) -> Result<String,FormattingError> {
        let value_to_match = VolumeKey::<KeyTypeMetadata>::match_float(float);
        //first try to find the next lower value.
        if let Some((_,msg)) = bin_symbol_map.range(..=value_to_match).next_back() {
            Ok(format!("{}{}",label,msg))
        }
        else {
            if let Some((_,msg)) = bin_symbol_map.iter().next() {
                Ok(format!("{}{}",label,msg))
            }
            else {
                Err(FormattingError::EmptyMap{numeric_fallback : Self::format_float_numeric(float, label, 0) })
            }
        }
    }
    fn format_float_numeric(float : f32, label : &str, digits : u8) -> String {
        let percentage = 100.0*float;
        format!("{}{:.*}%", label, digits as usize, percentage)
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
///Helper trait for conversion from float to integer backing type for volume binning keys.
///Needed because Rust seems not to offer a trait that indicates "can be rounded from float"
///in the standard library. There are thir-party crates that do this, but using a full crate
///for a few lines of code sounds a bit excessive...
trait VolumeKeyBackingTypeFromFloat {
    fn round_from_float(float : f32) -> Self;
}
macro_rules! impl_volume_key_backing_type_from_float_for {
    ($( $t:ty ), *) => {
        $( impl VolumeKeyBackingTypeFromFloat for $t {
            fn round_from_float(float : f32) -> $t { float.round() as $t }
        } )*
    }
}

///Metadata description for VolumeKeys. Basically a workaround for Rust's lack of
///constant generics. Having Ord as supertrait is because of the BTreeMap's trait
///bounds.
trait VolumeKeyBackingTypeMetadata : Ord {
    type BackingType 
        : Ord
        + Add<Output = Self::BackingType>
        + Sub<Output = Self::BackingType>
        + Into<f32>
        + VolumeKeyBackingTypeFromFloat
        + ToString //TOML needs map keys to be strings...
        + FromStr<Err = ParseIntError>
        + std::fmt::Display;
    const MIN : Self::BackingType;
    const MAX : Self::BackingType;
    const FLOAT_MIN : f32;
    const FLOAT_MAX : f32;
}
#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct VolumeKeyVolume;
#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct VolumeKeyBalance;

impl VolumeKeyBackingTypeMetadata for VolumeKeyVolume{
    type BackingType = u8;
    const MIN : Self::BackingType = 0;
    const MAX : Self::BackingType = 100;
    const FLOAT_MIN : f32 = 0.0;
    const FLOAT_MAX : f32 = 1.0;
}

impl VolumeKeyBackingTypeMetadata for VolumeKeyBalance{
    type BackingType = i8;
    const MIN : Self::BackingType = -100;
    const MAX : Self::BackingType = 100;
    const FLOAT_MIN : f32 = -1.0;
    const FLOAT_MAX : f32 = 1.0;
}

impl_volume_key_backing_type_from_float_for!(i8, u8);

#[derive(Debug,PartialEq,Eq,PartialOrd,Ord)]
struct VolumeKey<BackingType : VolumeKeyBackingTypeMetadata>(BackingType::BackingType);

impl<BackingType : VolumeKeyBackingTypeMetadata> VolumeKey<BackingType> {
    fn match_float(float : f32) -> Self {
        let x = (float - BackingType::FLOAT_MIN) / (BackingType::FLOAT_MAX - BackingType::FLOAT_MIN);
        let cx = x.clamp(0.0,1.0);
        let interval = BackingType::MAX.into() - BackingType::MIN.into();
        let offset = interval * cx;
        let result = BackingType::BackingType::round_from_float(offset) + BackingType::MIN;
        Self(result)
    }

}
/// Custom serializer, as TOML only supports string map keys.
impl<Metadata : VolumeKeyBackingTypeMetadata> Serialize for VolumeKey<Metadata> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S : Serializer 
    {
        let string = self.0.to_string();
        serializer.serialize_str(&string)
    }
}
impl<'de, Metadata> Deserialize<'de> for VolumeKey<Metadata>
    where Metadata : VolumeKeyBackingTypeMetadata,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        let a = String::deserialize(deserializer)?;
        match a.parse() {
            Ok(x) => { 
                if x >= Metadata::MIN && x <= Metadata::MAX {
                    Ok(Self(x)) 
                }
                else {
                    Err(DeError::invalid_value(DeUnexpect::Str(&a), &&*format!("an integer equal or larger {} and equal or smaller {}", Metadata::MIN, Metadata::MAX)))
                }
            }
            Err(e) => {
                match e.kind() {
                    IntErrorKind::Empty => { Err(DeError::missing_field("Bin Map Key")) }
                    IntErrorKind::InvalidDigit => { Err(DeError::invalid_type(DeUnexpect::Str(&a), &"an integer value")) }
                    IntErrorKind::NegOverflow | IntErrorKind::PosOverflow => { Err(DeError::invalid_value(DeUnexpect::Str(&a), &&*format!("an integer equal or larger {} and equal or smaller {}", Metadata::MIN, Metadata::MAX))) }
                    IntErrorKind::Zero => { Err(DeError::invalid_value(DeUnexpect::Str(&a), &&*format!("a nonzero integer between {} and {}", Metadata::MIN, Metadata::MAX)))}
                    _ => { Err(DeError::custom("Value could not be parsed")) }
                }
            }
        }
    }
}
