use std::sync::mpsc::Receiver;
use swaystatus_plugin::*;
use super::config::AlsaVolumeConfig;
use super::communication::MessagesFromMain;

pub struct AlsaVolumeRunnable<'r>{
    to_main : Box<dyn MsgModuleToMain + 'r>,
    from_main : Receiver<MessagesFromMain>,
    config : &'r AlsaVolumeConfig,
}

impl<'r> AlsaVolumeRunnable<'r> {
    pub fn new(to_main : Box<dyn MsgModuleToMain + 'r>, from_main : Receiver<MessagesFromMain>, config : &'r AlsaVolumeConfig) -> Self {
        Self { to_main, from_main, config }
    }
}

impl<'r> SwayStatusModuleRunnable for AlsaVolumeRunnable<'r> {
    fn run(&self) {
        todo!()
    }
}
