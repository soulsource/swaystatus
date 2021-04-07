use swaystatus_plugin as plugin;
use std::sync::mpsc::Sender;

/// Used for internal communication. At the moment only from the signal handler to the main thread.
pub enum InternalMessage {
    ///Exit gracefully.
    Quit,
    ///Refresh all text.
    Refresh,
    ///Reload everything. Plugins, config, basically exit and restart.
    Reload
}

pub enum Message {
    Internal(InternalMessage),
    External{
        text :Result<String,plugin::PluginError>,
        element_number : usize
    },
    ThreadCrash{
        element_number : usize
    }
}

/// Sender we give to our plugins. The same message type is used by our message handler internally,
/// but the plugins needn't know. That's why we hide it behind a trait object. Also, vtables rock
/// for going across dynlib boundaries, because that way we can make sure we're actually calling
/// our main-application's symbols, not those of the plugins. Yes, that's an issue. Initially we
/// sent crossbeam channels to plugins directly, without any trait object wrapping them. That
/// didn't work well, because crossbeam uses thread-local storage, which was a different object in
/// the main program and the plugins, as both linked statically against crossbeam...
pub struct SenderToMain {
    pub sender : Sender<Message>,
    pub element_number : usize,
}

impl Drop for SenderToMain {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let message = Message::ThreadCrash { element_number : self.element_number};
            if let Err(_e) = self.sender.send(message) {
                eprintln!("{}", super::gettext!("I, element {}, tried to inform the main thread that I crashed. However the main thread isn't listening any more. This should be impossible, but well... Also, it's not critical enough to halt the whole program...", self.element_number));
            }
        }
    }
}

impl plugin::MsgModuleToMain for SenderToMain {
    fn send_update(&self, text : Result<String, plugin::PluginError>) -> Result<(),()> {
        let message = Message::External { text , element_number : self.element_number };
        self.sender.send(message).map_err(|_| {})
    }
}
