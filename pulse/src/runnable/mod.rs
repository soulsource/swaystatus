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

    fn format_and_send_updated_volume_to_main(&self, volume : &pulse::Volume) -> Result<(),PluginCommunicationError> {
        let formatted_volume = self.config.format_volume(volume.volume, volume.balance, volume.muted);
        match formatted_volume {
            Ok(msg) => { self.to_main.send_update(Ok(msg)) }
            Err(e) => {
                let full_message = e.to_string();
                match e {
                    FormattingError::EmptyMap{ numeric_fallback } => {
                        self.to_main.send_update(Err(PluginError::ShowInsteadOfText(numeric_fallback)))?;
                        self.to_main.send_update(Err(PluginError::PrintToStdErr(full_message)))
                    }
                }
            }
        }
    }
}

impl<'p> SwayStatusModuleRunnable for PulseVolumeRunnable<'p> {
    fn run(&self) {
        'outer : loop {
            let pulse = match &self.pulse {
                Err(x) => {
                    self.send_error_to_main(x);
                    return;
                }
                Ok(x) => x
            };
            let mut scratch = pulse::ContextScratch::default();
            let mut context = match PulseContext::create(pulse, &mut scratch) {
                Err(x) => {
                    self.send_error_to_main(x);
                    return;
                }
                Ok(x) => x
            };
            let mut curr_default_sink = None;
            let mut curr_volume = None;
            let mut sink_we_care_about = match &self.config.sink {
                crate::config::Sink::Default => { None }
                crate::config::Sink::Specific { sink_name } => { 
                    Some(match SinkHandle::try_from(sink_name as &str) {
                        Ok(x) => {x}
                        Err(e) => {
                            self.send_error_to_main(e);
                            return;
                        }
                    }) 
                }
            };
            loop {
                match context.get_state() {
                    PaContextState::Unconnected => { 
                        if let crate::config::Sink::Default = &self.config.sink {
                            sink_we_care_about = None; 
                        }
                        if let Err(e) = context.connect_and_set_callbacks() {
                            self.send_error_to_main(e);
                            match self.from_main.recv_timeout(std::time::Duration::from_secs(1)) {
                                Ok(x) => {
                                    if let MessagesFromMain::Quit = x {
                                        break 'outer;
                                    }
                                }
                                Err(e) => {
                                    if let RecvTimeoutError::Disconnected = e {
                                        break 'outer;
                                    }
                                }
                            }
                        }
                    }
                    PaContextState::Failed | PaContextState::Terminated => { 
                        //context is dead. Wait a second, and start over.
                        self.to_main.send_update(Err(PluginError::ShowInsteadOfText(String::from("Context died")))).expect("Tried to tell main thread that pulse context died. Main thread isn't listening.");
                        self.to_main.send_update(Err(PluginError::PrintToStdErr(String::from("Pulseaudio context entered either the Terminated or Failed state. Creating a new context and retrying")))).expect("Tried to tell main thread that pulse context died. Main thread isn't listening.");
                        match self.from_main.recv_timeout(std::time::Duration::from_secs(1)) {
                            Ok(x) => {
                                if let MessagesFromMain::Quit = x {
                                    break 'outer;
                                }
                            }
                            Err(e) => {
                                if let RecvTimeoutError::Disconnected = e {
                                    break 'outer;
                                }
                            }
                        }
                        continue 'outer;
                    }
                    PaContextState::Ready => {
                        if curr_default_sink.is_none() {
                            //this may trigger several redundant refreshes, but it _should_ only happen
                            //during startup, so we don't really care.
                            context.refresh_default_sink();
                        }
                    }
                    _ => {}
                }

                let iteration_result = context.iterate(&sink_we_care_about);
                if let Err(e) = iteration_result {
                   self.send_error_to_main(e);
                   break 'outer;
                }
                
                let pulse::IterationResult { default_sink, volume } = iteration_result.unwrap();
                if default_sink.is_some() && default_sink != curr_default_sink {
                    curr_default_sink = default_sink;
                    if let crate::config::Sink::Default = self.config.sink {
                        sink_we_care_about = curr_default_sink.clone();
                    }
                    if let Some(s) = &sink_we_care_about {
                        context.refresh_volume(s);
                    }
                }
                if volume.is_some() && volume != curr_volume {
                    curr_volume = volume;
                    self.format_and_send_updated_volume_to_main(curr_volume.as_ref().unwrap()).expect("Tried to inform main thread about volume update. Main thread isn't listening.");
                }
                match self.from_main.try_recv() {
                    Ok(x) => match x {
                        MessagesFromMain::Quit => {
                            break 'outer;
                        }
                        MessagesFromMain::Refresh => {
                            if let Some(s) = &sink_we_care_about {
                                context.refresh_volume(s);
                            }
                            if let crate::config::Sink::Default = self.config.sink {
                                context.refresh_default_sink();
                            }
                        }
                    }
                    Err(e) => {
                        if let TryRecvError::Disconnected = e {
                            break 'outer;
                        }
                    }
                }
            }
        }
    }
}
