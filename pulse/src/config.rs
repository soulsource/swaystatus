use serde::{Serialize,Deserialize};
use std::collections::BTreeMap;
use swaystatus_plugin::*;


#[derive(Serialize, Deserialize)]
#[serde(tag = "Sink")]
enum Sink {
    Default,
    Specific {
        sink_name : String
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "Format")]
enum Volume {
    Off,
    Numeric {
        label : String
    },
    Binned {
        label: String,
        bin_symbol_map : BTreeMap<u8,String>
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "Format")]
enum Balance {
    Off,
    Numeric {
        label : String
    },
    Binned {
        label :String,
        bin_symbol_map : BTreeMap<i8,String>
    }
}


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct PulseVolumeConfig {
    sink : Sink,
    volume : Volume,
    balance : Balance
}

impl Default for PulseVolumeConfig {
    fn default() -> Self {
        PulseVolumeConfig {
            sink : Sink::Default,
            volume : Volume::Numeric { label : String::new()},
            balance : Balance::Off
        }
    }
}

impl SwayStatusModuleInstance for PulseVolumeConfig {
    fn make_runnable<'p>(&'p self,to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>) {
        let (runnable, sender_for_main) = crate::runnable::PulseVolumeRunnable::new(&self, to_main);
        (Box::new(runnable), Box::new(sender_for_main))
    }
}

