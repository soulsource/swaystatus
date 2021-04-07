/**
 * Plugin implementation for testing purposes of general plugin handling.
 *
 * Has a few extra functions that assist in testing.
 * Does not actually do anything.
 */

use serde::{Serialize, Deserialize};
use crate::plugin::*;

pub struct TestPlugin;
pub struct TestRunnable;
pub struct DeadEndSend;

impl MsgMainToModule for DeadEndSend {
    fn send_quit(&self) -> Result<(),()> {
        Err(())
    }
    fn send_refresh(&self) -> Result<(),()> {
        Err(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct TestConfig {
    lines : u32,
    skull : String,
}

impl SwayStatusModuleRunnable for TestRunnable {
    fn run(&self) {
        println!("Running!");
    }
}

impl SwayStatusModuleInstance for TestConfig {
    fn make_runnable<'p>(&'p self, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>) {
       return (Box::new(TestRunnable), Box::new(DeadEndSend)); 
    }
}

impl SwayStatusModule for TestPlugin {
    fn get_name(&self) -> &str {
        return "TestPlugin";
    }
    fn deserialize_config<'de>(&self, deserializer : &mut (dyn erased_serde::Deserializer + 'de)) -> Result<Box<dyn SwayStatusModuleInstance>, erased_serde::Error> {
       let result : TestConfig = erased_serde::deserialize(deserializer)?;
       return Ok(Box::new(result));
    }
    fn get_default_config(&self) -> Box<dyn SwayStatusModuleInstance> {
        let config = TestConfig{
            lines : 3,
            skull : String::from("☠"),
        };
        return Box::new(config);
    }
}

