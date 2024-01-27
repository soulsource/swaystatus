use std::sync::mpsc::*;
use swaystatus_plugin::*;

pub(crate) mod pipe_chan;

#[repr(C)]
pub enum MessagesFromMain {
    Quit,
    Refresh
}

pub struct SenderForMain {
    sender : Sender<MessagesFromMain>,
}

impl<'p> SenderForMain {
    pub fn new(sender : Sender<MessagesFromMain>) -> Self {
        Self { sender }
    }
    
    fn send(&self, message : MessagesFromMain) -> Result<(), PluginCommunicationError> {
        self.sender.send(message).map_err(|_| PluginCommunicationError)
    }
}

impl MsgMainToModule for SenderForMain {
    fn send_quit(&self) -> Result<(),PluginCommunicationError> {
        self.send(MessagesFromMain::Quit)
    }
    fn send_refresh(&self) -> Result<(),PluginCommunicationError> {
        self.send(MessagesFromMain::Refresh)
    }
}
