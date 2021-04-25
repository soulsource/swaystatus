use crate::communication;
use std::sync::mpsc;
use signal_hook::iterator::Signals;
use signal_hook::consts::*;
use crossbeam_utils::thread::Scope;

/// This function starts an endless loop, waiting for signals. The only ones that we explicitly
/// handle are USR1 (immediate update), SIGPIPE (because that indicates nobody is listening to us
/// any more), SIGHUP to trigger a reload, and the usual term signals. 
pub fn handle_signals(scope : &Scope, sender : mpsc::Sender<communication::Message>) {
    //we mustn't forget that upon any terminating signals (including PIPE) and HUP we need to exit.
    let mut signals = Signals::new(&[
        signal::SIGTERM, //quit
        signal::SIGINT,  //quit as well
        //signal::SIGQUIT, //we don't do anything special here. Users _expect_ QUIT to make a dump.
        signal::SIGPIPE, //quit, because nobody's listening
        signal::SIGHUP,  //quit, but send the Reload message instead of the Quit one.
        signal::SIGUSR1, //trigger a refresh. The ONLY one upon which we dont break the loop!
    ]).unwrap_or_else(|_| {panic!("{}",gettextrs::gettext("Failed to register signal handler. Since without signal handler there's no proper way to cleanly exit any plugins, we bail now."))});

    scope.spawn(move |_| {
        for signal in &mut signals {
            match signal {
                signal::SIGUSR1 => send(&sender, communication::InternalMessage::Refresh),
                signal::SIGHUP => { send(&sender, communication::InternalMessage::Reload); break; }
                _=> { send(&sender, communication::InternalMessage::Quit); break;}

            }
        }
    });
}

fn send(sender : &mpsc::Sender<communication::Message>, message : communication::InternalMessage) {
    sender.send(communication::Message::Internal(message)).unwrap_or_else(|_| {panic!("{}",gettextrs::gettext("Message handler failed to send a message to main thread. This is supposed to be impossible. In any case it's a critical error."))});
}
