use std::sync::mpsc::*;
use crate::runnable::*;
use swaystatus_plugin::*;

pub enum MessagesFromMain {
    Quit,
    Refresh
}

pub struct SenderForMain {
    sender : Sender<MessagesFromMain>,
    pulse_loop : std::sync::Arc<PulseMainLoop>
}

impl<'p> SenderForMain {
    pub fn new(sender : Sender<MessagesFromMain>, pulse_loop : std::sync::Arc<PulseMainLoop>) -> Self {
        SenderForMain{
            sender,
            pulse_loop
        }
    }

    fn send(&self, message : MessagesFromMain) -> Result<(), PluginCommunicationError> 
    {
        //TODO!!!!
        Ok(())
    }
}

impl MsgMainToModule for SenderForMain {
    fn send_quit(&self) -> Result<(), PluginCommunicationError> {
        self.send(MessagesFromMain::Quit)
    }
    fn send_refresh(&self) -> Result<(), PluginCommunicationError> {
        self.send(MessagesFromMain::Refresh)
    }
}
