use std::sync::mpsc::*;
use crate::config::*;
use swaystatus_plugin::*;
use crate::communication::*;

pub mod pulse;
use pulse::Pulse;

pub struct PulseVolumeRunnable<'p> {
    config : &'p PulseVolumeConfig,
    to_main : Box<dyn MsgModuleToMain + 'p>,
    from_main : Receiver<MessagesFromMain>,
    pulse : Pulse
}

impl<'p : 's, 's> PulseVolumeRunnable<'p> {
    pub fn new(config : &'p PulseVolumeConfig, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Self, SenderForMain) {
        let (s, r) = channel();
        let result = PulseVolumeRunnable {
            config,
            to_main,
            from_main : r,
            pulse: Pulse::init(&config.sink), 
        };
        let sender = SenderForMain::new(s, result.pulse.get_wake_up());
        (result, sender)
    }
}

impl<'p> SwayStatusModuleRunnable for PulseVolumeRunnable<'p> {
    fn run(&self) {
        //TODO
    }
}
