use swaystatus_plugin::*;

use self::pipe_chan::{Sender, Receiver, SendError, ReceiveError, create_pipe_chan};

pub(crate) mod pipe_chan;

#[repr(C)]
pub enum MessagesFromMain {
    Quit,
    Refresh
}

pub(crate) struct MessagesFromMainReceiver{
    receiver : Receiver
}

pub(crate) struct MessagesFromMainSender{
    sender: Sender
}

pub(crate) fn make_sender_for_main() -> Result<(MessagesFromMainSender, MessagesFromMainReceiver),()>{
    create_pipe_chan().map(|(sender,receiver)| (MessagesFromMainSender{sender}, MessagesFromMainReceiver{receiver}))
}

impl MessagesFromMainSender {
    pub(crate) fn send(&self, message : MessagesFromMain) -> Result<(), SendError>{
        self.sender.send_byte(match message {
            MessagesFromMain::Quit => 0,
            MessagesFromMain::Refresh => 1,
        })
    }
}

impl MessagesFromMainReceiver {
    pub(crate) fn receive(&self) -> Result<Option<MessagesFromMain>, ReceiveError> {
        self.receiver.read_byte().map(|o| o.map(|b| match b {
            0 => MessagesFromMain::Quit,
            1 => MessagesFromMain::Refresh,
            _ => unreachable!()
        }))
    }
    pub(crate) fn file_handle(&self) -> &pipe_chan::FileHandle{
        self.receiver.file_handle()
    }
}

pub struct SenderForMain {
    sender : MessagesFromMainSender,
}

impl<'p> SenderForMain {
    pub fn new(sender : MessagesFromMainSender) -> Self {
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
