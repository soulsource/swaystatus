use std::sync::mpsc::*;
use crate::config::*;
use swaystatus_plugin::*;
use crate::communication::*;
use std::convert::TryFrom;

pub mod pulse;
use pulse::{Pulse,MainLoopCreationError, PulseContext, PaContextState, SinkHandle};

pub struct PulseVolumeRunnable<'p> {
    config : &'p PulseVolumeConfig,
    to_main : Box<dyn MsgModuleToMain + 'p>,
    from_main : Receiver<MessagesFromMain>,
    pulse : Result<Pulse,MainLoopCreationError>
}

impl<'p : 's, 's> PulseVolumeRunnable<'p> {
    pub fn new(config : &'p PulseVolumeConfig, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Self, SenderForMain) {
        let (s, r) = channel();
        let result = PulseVolumeRunnable {
            config,
            to_main,
            from_main : r,
            pulse: Pulse::create()
        };
        let sender = SenderForMain::new(s, result.pulse.as_ref().map_or(None,|x| Some(x.get_wake_up())));
        (result, sender)
    }
    fn send_error_to_main<E>(&self, err : E) where E : std::error::Error {
        self.to_main.send_update(Err(PluginError::ShowInsteadOfText(String::from("Error")))).expect("Tried to tell main thread that an error occured. Main thread isn't listening any more.");
        self.to_main.send_update(Err(PluginError::PrintToStdErr(err.to_string()))).expect("Tried to tell main thread that an error occured. Main thread isn't listening any more.");
    }

}

impl<'p> SwayStatusModuleRunnable for PulseVolumeRunnable<'p> {
    fn run(&self) {
        let pulse = match &self.pulse {
            Err(x) => {
                self.send_error_to_main(x);
                return;
            }
            Ok(x) => x
        };
        let context = match PulseContext::create(pulse) {
            Err(x) => {
                self.send_error_to_main(x);
                return;
            }
            Ok(x) => x
        };
        let mut context_state = PaContextState::Unconnected;
        let mut curr_default_sink = None;
        let mut curr_volume = None;
        let mut sink_we_care_about = match self.config.sink {
            crate::config::Sink::Default => { None }
            crate::config::Sink::Specific { sink_name } => { 
                Some(match SinkHandle::try_from(&*sink_name) {
                    Ok(x) => {x}
                    Err(e) => {
                        self.send_error_to_main(e);
                        return;
                    }
                }) 
            }
        };
        loop {
            match context_state {
                PaContextState::Unconnected => { 
                    if let crate::config::Sink::Default = self.config.sink {
                        sink_we_care_about = None; 
                    }
                    context.connect(); 
                }
                PaContextState::Failed | PaContextState::Terminated => { 
                    match self.from_main.recv_timeout(std::time::Duration::from_secs(1)) {
                        Ok(x) => {
                            if let MessagesFromMain::Quit = x {
                                break;
                            }
                        }
                        Err(e) => {
                            if let RecvTimeoutError::Disconnected = e {
                                break;
                            }
                        }
                    }
                    if let crate::config::Sink::Default = self.config.sink {
                        sink_we_care_about = None;
                    }
                    context.connect();
                }
                PaContextState::Ready => {
                    if sink_we_care_about.is_none() {
                        //this may trigger several redundant refreshes, but it _should_ only happen
                        //during startup, so we don't really care.
                        context.refresh_default_sink();
                    }
                }
                _ => {}
            }
            
            let Pulse::IterationResult { default_sink, volume, state } = context.iterate(sink_we_care_about);
            context_state = state.unwrap_or(context_state);
            if default_sink.is_some() && default_sink != curr_default_sink {
                curr_default_sink = default_sink;
                if let crate::config::Sink::Default = self.config.sink {
                    sink_we_care_about = curr_default_sink;
                    context.refresh_volume();
                }
            }
            if volume.is_some() && volume != curr_volume {
                curr_volume = volume;
                self.send_updated_volume_to_main(curr_volume);
            }
            match self.from_main.try_recv() {
                Ok(x) => match x {
                    MessagesFromMain::Quit => {
                        break;
                    }
                    MessagesFromMain::Refresh => {
                        context.refresh_volume();
                        if let crate::config::Sink::Default = self.config.sink {
                            context.refresh_default_sink();
                        }
                    }
                }
                Err(e) => {
                    if let TryRecvError::Disconnected = e {
                        break;
                    }
                }
            }
        }
    }
}
