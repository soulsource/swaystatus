use serde::{Serialize,Deserialize,Serializer,Deserializer};
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
        sink_name : String
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "Format")]
enum FormatableVolume<KeyTypeMetadata : VolumeKeyBackingTypeMetadata> {
    Off,
    Numeric {
        label : String
    },
    Binned {
        label: String,
        bin_symbol_map : BTreeMap<VolumeKey<KeyTypeMetadata>,String>
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PulseVolumeConfig {
    pub(crate) sink : Sink,
    volume : FormatableVolume<VolumeKeyVolume>,
    balance : FormatableVolume<VolumeKeyBalance>
}

impl Default for PulseVolumeConfig {
    fn default() -> Self {
        PulseVolumeConfig {
            sink : Sink::Default,
            volume : FormatableVolume::Numeric { label : String::new() },
            //volume : FormatableVolume::Binned { label : String::new(), bin_symbol_map : {let mut a = BTreeMap::new(); a.insert(4,String::from("Blah")); a}},
            balance : FormatableVolume::Off
        }
    }
}

impl SwayStatusModuleInstance for PulseVolumeConfig {
    fn make_runnable<'p>(&'p self,to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>) {
        let (runnable, sender_for_main) = crate::runnable::PulseVolumeRunnable::new(&self, to_main);
        (Box::new(runnable), Box::new(sender_for_main))
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
        + VolumeKeyBackingTypeFromFloat;
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
        let interval = BackingType::MAX - BackingType::MIN;
        let offset = interval.into() * cx;
        let result = BackingType::BackingType::round_from_float(offset) + BackingType::MIN;
        Self(result)
    }
}

impl<Metadata : VolumeKeyBackingTypeMetadata> Serialize for VolumeKey<Metadata> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S : Serializer 
    {
        Err(serde::ser::Error::custom("Unimplemented"))
    }
}
impl<'de, Metadata : VolumeKeyBackingTypeMetadata> Deserialize<'de> for VolumeKey<Metadata> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        Err(serde::de::Error::custom("Unimplemented"))
    }
}
