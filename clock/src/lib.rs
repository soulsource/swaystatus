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

    /// Simple, non-synchronized loop. Just sleeps the configured duration between sending the
    /// current time to the main module. Because thread sleeping is inaccurrate, this will alias
    /// sooner or later. Probably sooner.
    fn simple_loop(&self, timeout : std::time::Duration) {
        loop {
            self.to_main.send_update(Ok(self.print_current_time_with_format())).expect("Clock plugin tried to send the current time to the main program, but the main program doesn't listen any more.");
            match self.from_main.recv_timeout(timeout) {
                Ok(MessagesFromMain::Refresh) | Err(RecvTimeoutError::Timeout) => {},
                Ok(MessagesFromMain::Quit) | Err(RecvTimeoutError::Disconnected) => { break; },
            }
        }
    }

    /// Helper for `synchronized_loop(fraction_of_thirty_mins : u64)`. Due to rounding, if one is
    /// extremely unlucky and refreshes/starts the loop in the last millisecond before a
    /// synchronization point, one synchronization will be skipped. This function checks for such
    /// rounding issues and sleeps for the problematic ms.
    fn fix_rounding_error_if_bad_refresh(fraction_of_thirty_mins : u64) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("System time before beginning of UNIX epoch?!?");
        let now_millis = now.as_millis() as u64;
        let now_millis_up = now_millis + 1;
        let now_fraction_millis = now_millis * fraction_of_thirty_mins;
        let now_fraction_millis_up = now_millis_up * fraction_of_thirty_mins;
        if now_fraction_millis_up / 1800000 != now_fraction_millis / 1800000 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }


    /// Loop that's synchronized to UTC. It hardcoded operates on a basis of 30 minutes and uses
    /// millisecond accurracy (meaning: calculations are rounded up to an error of 1 ms, and thread
    /// sleeping inacurracies can then cause even higher errors). For this reason, it's not advised
    /// to update more often than every 0.1 seconds or so.
    /// The calcluations are done in u64 because there's little to gain by using u128, given that
    /// with those constraints 64 bits will last for nearly 9000 years...
    fn synchronized_loop(&self, fraction_of_thirty_mins : u64) {
        Self::fix_rounding_error_if_bad_refresh(fraction_of_thirty_mins);
        loop {
             self.to_main.send_update(Ok(self.print_current_time_with_format())).expect("Clock plugin tried to send the current time to the main program, but the main program doesn't listen any more.");
             let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("System time before beginning of UNIX epoch?!?");
             
             let now_millis = now.as_millis() as u64 + 1; //+1 for rounding up. 
             let now_fraction_millis = now_millis * fraction_of_thirty_mins;
             let now_fraction = now_fraction_millis / (1800000);
             let target_fraction = now_fraction + 1; //Adds one fraction_of_thirty_mins
             let target_rounded_fraction_millis = target_fraction * 1800000;
             let target_millis = target_rounded_fraction_millis / fraction_of_thirty_mins;
             let timeout_millis = target_millis - now_millis +1; //the 1 from above again, this time to ensure timeout_millis is actually rounded _up_
             let timeout = std::time::Duration::from_millis(timeout_millis);
             match self.from_main.recv_timeout(timeout) {
                 Ok(MessagesFromMain::Refresh) => {
                     Self::fix_rounding_error_if_bad_refresh(fraction_of_thirty_mins);
                 },
                 Err(RecvTimeoutError::Timeout) => {},
                 Ok(MessagesFromMain::Quit) | Err(RecvTimeoutError::Disconnected) => { break; },
             }
        }
    }
}

impl<'c> SwayStatusModuleRunnable for ClockRunnable<'c> {
    fn run(&self) {
        match self.config.refresh_rate {
            ClockRefreshRate::NotSynchronized { seconds } => {
                self.simple_loop(std::time::Duration::from_secs_f32(seconds.abs()));
            },
            ClockRefreshRate::UtcSynchronized { updates_per_thirty_minutes }=> {
                self.synchronized_loop(std::cmp::max(updates_per_thirty_minutes,1) as u64);
            }
        }
    }
}

/// How the clock should refresh. There are two modes of operation. NotSynchronizedSeconds just
/// sleeps the given number of seconds  (float) between updates. As the name implies, it's NOT
/// synchronized to UTC, not even at startup. This means, that if you set a refresh rate of 3600
/// seconds and start the program at 2:45, the clock will remain at 2:45 until it's actually
/// 3:45... This setting takes a floating point amount of seconds as parameter.
/// The other mode of operation is synchronized to UTC. It can be set to update every 30/x minutes,
/// where x is a number between 1 and 65535. This range limitation was chosen to ensure meaningful
/// input. The 30 minute maximum for time between updates was chosen because of time zones.
/// Synchronizing to UTC days is not useful, because UTC midnight will in general not correspond to
/// the local midnight time. Synchronizing to UTC hours is questionable for the same reasons,
/// given that there are some time zones offset by 30 minutes. This brings us to the largest
/// duration that actually makes sense to synchronize with UTC: 30 minutes. The maximum update
/// rate has its current value because of accuracy limitations. Thread sleep is inherently
/// imprecise. Depending on the hardware/software, the error can be up to milliseconds. For a
/// simple wall clock there is little point in investing CPU cycles for higher accuracy, so the
/// maximum update rate was chosen to be way below 1/ms. By coincidence a 16 bit integer fits the
/// range of reasonable values nicely.
#[derive(Serialize, Deserialize)]
#[serde(tag = "Synchronization")]
enum ClockRefreshRate {
    NotSynchronized {
        #[serde(rename = "Seconds")]
        seconds : f32
    },
    UtcSynchronized {
        #[serde(rename = "PerThirtyMinutes")]
        updates_per_thirty_minutes : u16
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase",default)]
struct ClockConfig {
    format : String,
    refresh_rate : ClockRefreshRate 
}

impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            format : String::from("%R"), 
            refresh_rate : ClockRefreshRate::UtcSynchronized { updates_per_thirty_minutes: 1800 }
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
    fn print_help(&self) {
        println!(
r#"Swaystatus Clock plugin.

This plugin is a simple wall clock, implemented as a thin wrapper around the chrono crate (https://github.com/chronotope/chrono). This plugin just has two options: The format of the to-be-printed time, and the update rate.

The general format for a Clock configuration is:
[Element.Config]
Format = "<strftime format string>"

[Element.Config.RefreshRate]
Synchronization = "UtcSynchronized"
PerThirtyMinutes = <integer>

or alternatively
[Element.Config]
Format = "<strftime format string>"

[Element.Config.RefreshRate]
Synchronization = "NotSynchronized"
Seconds = <float>

The format is directly passed on to chrono and uses the strftime format. For available formatting options please see https://docs.rs/chrono/0.4.19/chrono/format/strftime/index.html or the strftime(3) man page.

For the RefreshRate you can choose between two options. Unless you have a very special use case, you'll likely want to use the UtcSynchronized option. As the name implies this mode aims to update in sync with your computer's system clock. For instance, if you set it to update every second, the text of the clock will update within a few milliseconds after a full second of the system clock passed.
Since it synchronizes with UTC, and time zones are in general offset by multiples of 30 minutes, 30 minutes has been chosen as maximum time between updates. Following the "make invalid states unrepresentable" paradigm, the actual update rate is set as a fraction of 30 minutes. For example, if you want to update every second, the "PerThirtyMinutes" field needs to be set to 1800. If you need updates every minute, you'll want toset "PerThirtyMinutes" to 30. Beware that setting PerThirtyMinutes above 36000 is not supported.

The other option, "NotSynchronized" is for cases where 30/n minutes as an update rate is not desired. This simply waits approximately Seconds seconds between updates, but does not care about any synchronization to UTC. In other words, if you set this to 24 hours update rate, but launch the program at noon, that's when your date will update instead of midnight. Long story short, this mode only exists because it was easy to implement, and will probably never be useful to anybody."#
);
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
    fn send_quit(&self) -> Result<(),PluginCommunicationError> {
        self.0.send(MessagesFromMain::Quit).map_err(|_| PluginCommunicationError)
    }
    fn send_refresh(&self) -> Result<(),PluginCommunicationError> {
        self.0.send(MessagesFromMain::Refresh).map_err(|_| PluginCommunicationError)
    }
}

declare_swaystatus_module!(ClockPlugin, ClockPlugin::new);
