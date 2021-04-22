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
    fn simple_loop(&self, timeout : std::time::Duration) {
        loop {
            self.to_main.send_update(Ok(self.print_current_time_with_format())).expect("Clock plugin tried to send the current time to the main program, but the main program doesn't listen any more.");
            match self.from_main.recv_timeout(timeout) {
                Ok(MessagesFromMain::Refresh) | Err(RecvTimeoutError::Timeout) => {},
                Ok(MessagesFromMain::Quit) | Err(RecvTimeoutError::Disconnected) => { break; },
            }
        }
    }


    fn synchronized_loop(&self, full_seconds : u128, second_fraction : u128) {
        loop {
             self.to_main.send_update(Ok(self.print_current_time_with_format())).expect("Clock plugin tried to send the current time to the main program, but the main program doesn't listen any more.");
             let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("System time before beginning of UNIX epoch?!?");
             
             let now_millis = now.as_millis() +1; //+1 for rounding up. TODO: Add a conditional millisecond of sleep after startup or refresh to compensate if now_fraction != now_plus_1_fraction
             let now_fraction_millis = now_millis * second_fraction;
             let now_fraction = now_fraction_millis / 1000;
             let target_fraction = now_fraction + full_seconds; //TODO: this is horrible. Make full seconds a separate loop function and just hardcode 1 here.
             let target_rounded_fraction_millis = target_fraction * 1000;
             let target_millis = target_rounded_fraction_millis / second_fraction;
             let timeout_millis = target_millis - now_millis +1; //the 1 from above again, this time to ensure timeout_millis is actually rounded _up_
             let timeout = std::time::Duration::from_millis(timeout_millis as u64);
             match self.from_main.recv_timeout(timeout) {
                 Ok(MessagesFromMain::Refresh) | Err(RecvTimeoutError::Timeout) => {},
                 Ok(MessagesFromMain::Quit) | Err(RecvTimeoutError::Disconnected) => { break; },
             }
        }
    }
}

impl<'c> SwayStatusModuleRunnable for ClockRunnable<'c> {
    fn run(&self) {
        //there are two modes of operation for this module.
        //Which one is used depends entirely on the interval
        //If the interval is a full multiple of a second, 
        //we use the "synchronized" variant, which
        //aims at ticking approximately at the full second.
        //
        //Similarly if the interval can be written as 1/x seconds.
        //
        //Otherwise we just loop.
        let abs_frac_part = (self.config.refresh_rate.round() - self.config.refresh_rate).abs();
        let inverse = self.config.refresh_rate.recip();
        let abs_inverse_frac_part = (inverse.round() - inverse).abs();
        if self.config.refresh_rate < 1e-3_f32 || ((abs_frac_part >= 1e-3_f32) && abs_inverse_frac_part >= 1e-3_f32) {
            self.simple_loop(std::time::Duration::from_secs_f32(self.config.refresh_rate.abs()));
        }
        else if abs_frac_part <= 1e-3 {
            self.synchronized_loop(self.config.refresh_rate.round() as u128, 1);
        }
        else {
            self.synchronized_loop(1, inverse.round() as u128);
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
