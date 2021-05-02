use std::sync::mpsc::*;
use crate::runnable::pulse::*;
use swaystatus_plugin::*;

pub enum MessagesFromMain {
    Quit,
    Refresh
}

pub struct SenderForMain {
    sender : Sender<MessagesFromMain>,
    pulse_waker : PulseWakeUp
}

impl<'p> SenderForMain {
    pub fn new(sender : Sender<MessagesFromMain>, pulse_waker : PulseWakeUp) -> Self {
        SenderForMain{
            sender,
            pulse_waker
        }
    }

    fn send(&self, message : MessagesFromMain) -> Result<(), PluginCommunicationError> {
        if let Ok(_) = self.sender.send(message) {
            //The cool thing about pulse using poll() is that poll() also wakes up if started after
            //the actual wake up call. So no need to worry about races, this is inherently sane!
            self.pulse_waker.wake_up()?;
            Ok(())
        }
        else {
            Err(PluginCommunicationError{})
        }
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

impl From<PulseWakeUpError> for PluginCommunicationError {
    fn from(error : PulseWakeUpError) -> Self {
        PluginCommunicationError {}
    }
}
