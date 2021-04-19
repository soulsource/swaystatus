use serde::{Serialize, Deserialize};
use swaystatus_plugin::*;
use std::sync::mpsc::*;

pub struct ClockPlugin;
pub struct ClockRunnable<'c> {
    config : &'c ClockConfig,
    from_main : Receiver<MessagesFromMain>, 
    to_main : Box<dyn MsgModuleToMain +'c>
}

impl<'c> ClockRunnable<'c> {
    fn print_current_time_with_format(&self) -> String {
        let now = chrono::offset::Local::now();
        now.format(&self.config.format).to_string()
    }
    fn simple_loop(&self) {
        loop {
            self.to_main.send_update(Ok(self.print_current_time_with_format())).expect("Clock plugin tried to send the current time to the main program, but the main program doesn't listen any more.");
            match self.from_main.recv_timeout(std::time::Duration::from_secs_f32(self.config.refresh_rate)) {
                Ok(MessagesFromMain::Refresh) | Err(RecvTimeoutError::Timeout) => {},
                Ok(MessagesFromMain::Quit) | Err(RecvTimeoutError::Disconnected) => { break; },
            }
        }
    }

    fn synchronized_loop(&self, second_fraction : u32) {
        //TODO: implement...
        self.simple_loop();
    }
}

impl<'c> SwayStatusModuleRunnable for ClockRunnable<'c> {
    fn run(&self) {
        //there are two modes of operation for this module.
        //Which one is used depends entirely on the interval
        //If the interval or its inverse is a full multiple of
        //a second, we use the "synchronized" variant, which
        //aims at ticking approximately at the full second.
        //Otherwise we just loop.
        let frac_part = self.config.refresh_rate.fract();
        let inverse_frac_part = self.config.refresh_rate.recip().fract();
        if frac_part.abs() > 1e-3 && inverse_frac_part.abs() > 1e-3 {
            self.simple_loop();
        }
        else if frac_part.abs() <= 1e-3 {
            self.synchronized_loop(1);
        }
        else {
            self.synchronized_loop(self.config.refresh_rate.recip().trunc().abs() as u32);
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase",default)]
struct ClockConfig {
    format : String,
    refresh_rate : f32
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            format : String::from("%R"), 
            refresh_rate : 1.0
        }
    }
}

impl SwayStatusModuleInstance for ClockConfig {
     fn make_runnable<'p>(&'p self, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>) {
         let (sender_from_main, from_main) = channel();
         let runnable = ClockRunnable {
             config : &self,
             from_main,
             to_main
         };
         let s = SenderForMain(sender_from_main);
         (Box::new(runnable), Box::new(s))
     }
}

impl SwayStatusModule for ClockPlugin {
    fn get_name(&self) -> &str {
        "ClockPlugin"
    }
    fn deserialize_config<'de>(&self, deserializer : &mut (dyn erased_serde::Deserializer + 'de)) -> Result<Box<dyn SwayStatusModuleInstance>, erased_serde::Error> {
       let result : ClockConfig = erased_serde::deserialize(deserializer)?;
       Ok(Box::new(result))
    }
    fn get_default_config(&self) -> Box<dyn SwayStatusModuleInstance> {
        let config = ClockConfig::default();
        Box::new(config)
    }
}

impl ClockPlugin {
    fn new() -> ClockPlugin {
        ClockPlugin
    }
}

enum MessagesFromMain {
    Quit,
    Refresh
}

struct SenderForMain(Sender<MessagesFromMain>);

impl MsgMainToModule for SenderForMain {
    fn send_quit(&self) -> Result<(),()> {
        self.0.send(MessagesFromMain::Quit).map_err(|_| ())
    }
    fn send_refresh(&self) -> Result<(),()> {
        self.0.send(MessagesFromMain::Refresh).map_err(|_| ())
    }
}

declare_swaystatus_module!(ClockPlugin, ClockPlugin::new);
