use std::sync::mpsc::*;
use crate::config::*;
use swaystatus_plugin::*;
use crate::communication::*;

pub struct PulseVolumeRunnable<'p> {
    config : &'p PulseVolumeConfig,
    to_main : Box<dyn MsgModuleToMain + 'p>,
    from_main : Receiver<MessagesFromMain>,
    pulse : std::sync::Arc<PulseMainLoop>
}

impl<'p : 's, 's> PulseVolumeRunnable<'p> {
    pub fn new(config : &'p PulseVolumeConfig, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Self, SenderForMain) {
        let (s, r) = channel();
        let pulse = std::sync::Arc::new(PulseMainLoop {});//TODO: initialize this properly
        let result = PulseVolumeRunnable {
            config,
            to_main,
            from_main : r,
            pulse: pulse.clone()
        };
        let sender = SenderForMain::new(s, pulse);
        (result, sender)
    }
}

impl<'p> SwayStatusModuleRunnable for PulseVolumeRunnable<'p> {
    fn run(&self) {
        //TODO
    }
}

pub struct PulseMainLoop {
    //TODO!
}
