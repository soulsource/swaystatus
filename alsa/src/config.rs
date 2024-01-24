use std::sync::mpsc::channel;

use serde::{Serialize, Deserialize};
use swaystatus_plugin::*;

use crate::{runnable::AlsaVolumeRunnable, communication::SenderForMain};

#[derive(Debug, Serialize, Deserialize)]
pub struct AlsaVolumeConfig{

}


impl SwayStatusModuleInstance for AlsaVolumeConfig {
    fn make_runnable<'p>(&'p self, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>) {
        let (s,r) = channel();
        (Box::new(AlsaVolumeRunnable::new(to_main, r, self)), Box::new(SenderForMain::new(s)))
    }
}
