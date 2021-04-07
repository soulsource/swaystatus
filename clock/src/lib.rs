use serde::{Serialize, Deserialize};
use swaystatus_plugin::*;
use std::sync::mpsc::*;

pub struct ClockPlugin;
pub struct ClockRunnable<'c> {
    config : &'c ClockConfig,
    from_main : Receiver<MessagesFromMain>, 
    to_main : Box<dyn MsgModuleToMain +'c>
}

impl<'c> SwayStatusModuleRunnable for ClockRunnable<'c> {
    fn run(&self) {
        for i in 0..4 {
            println!("Sending Error {}",i);
            self.to_main.send_update(Err(PluginError::PrintToStdErr(format!("Hello {}", i)))).unwrap();
            std::thread::sleep(std::time::Duration::from_secs(2));
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
