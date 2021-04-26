#![warn(missing_docs)]
//! Interface definitions for swaystatus plugins.
//!
//! A plugin is a shared library that provides the following types:
//! - `SwayStatusModule`, the main entry point of the module, used to create SwayStatusModuleInstance 
//! - `SwayStatusModuleInstance`, one instance of the module. Gets initialized with settings and creates SwayStatusModuleRunnable.
//! - `SwayStatusModuleRunnable`, created by SwayStatusModuleInstance, the plugin's main loop
//!
//! In addition the plugin must have a constructor function that returns a valid SwayStatusModule.
//! This can then be exported using the `declare_swaystatus_module()` macro.
//!
//! The swaystatus main program will call methods on your `SwayStatusModule` during initialization.
//! All calls to this interface will come from the main thread.
//!
//! The `make_runnable()` method should prepare a runnable object, which will then be run in a
//! separate worker thread. You have full control over that thread, meaning you can just sleep
//! until an update is required. The make_runnable() is still called from the main thread.
//! However the runnable's run() method is called from a worker thread.
//!
//! Communication between plugin and main module is routed through special trait objects instead
//! of directly moving channel endpoints. This is done for two reasons. The first is that it allows
//! to abstract away the need to use a given synchronization framework. That way for instance the
//! communication from main to plugin could use crossbeam, while the answers from the plugin are
//! sent using the standard library's mpsc channels.
//! The second, and more important reason is that (at least crossbeam's) channels use thread-local
//! storage, which would only work properly between different dynamic libraries, if they also
//! dynamically link against the library that supplies the channel implementation. Since linkage
//! with Cargo is defined by the dependency, not by the user, that's not really an option...
//!
//! ## Note on lifetimes:
//! There are relatively strict lifetimes imposed. No created trait object may outlive its creator.
//! This is because loading plugins is by definition unsafe, and we need to make sure that nothing
//! can exist beyond the symbols from the dynamic library.
//!
//! ## Note on linkage:
//! While you can't easily change how dependencies are linked into your plugin, you can choose to
//! dynamically link against the Rust standard library. For memory reasons I'd strongly recommend
//! to do so. The easiest way is to set rustc compiler flags using .cargo/config in the project.

use erased_serde::serialize_trait_object;

#[doc(hidden)]
pub static RUSTC_VERSION : &str = env!("RUSTC_VERSION");
#[doc(hidden)]
pub static MODULE_VERSION : &str = env!("CARGO_PKG_VERSION");

/// Declares a public export C function that creates your plugin's main object.
/// parameters are: The plugin's concrete type, and the constructor function for it.
/// This is blatantly stolen from
/// https://michael-f-bryan.github.io/rust-ffi-guide/dynamic_loading.html
#[macro_export]
macro_rules! declare_swaystatus_module {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn _swaystatus_module_create() -> *mut dyn $crate::SwayStatusModule {
            // make sure the constructor is the correct type.
            let constructor: fn() -> $plugin_type = $constructor;
            let object = constructor();
            let boxed: Box<$crate::SwayStatusModule> = Box::new(object);
            Box::into_raw(boxed)
        }
        #[no_mangle]
        pub extern "C" fn _swaystatus_module_version() -> *const str {
            $crate::MODULE_VERSION
        }
        #[no_mangle]
        pub extern "C" fn _swaystatus_rustc_version() -> *const str {
            $crate::RUSTC_VERSION
        }
    };
}

/// You need to implement this trait, as creating a runnable needs to return this type as well.
/// A typical implementation would be a thin wrapper around a channel's sender end.
/// Please don't make this blocking to prevent deadlocks.
pub trait MsgMainToModule {
    /// Implement this in such a way, that when it's called from the main thread, your module will
    /// soon-ish return from it's main function, after cleaning up it's resources and after joining
    /// all threads it spawns.
    fn send_quit(&self) -> Result<(),PluginCommunicationError>;

    /// Implement this in such a way, that when it's called from the main thread, your module will
    /// soon-ish send an updated text. The main module does not really wait for updates, so
    /// ignoring this or implementing it empty is perfectly fine if you know that your module's
    /// output cannot possibly change between updates it sends anyhow.
    fn send_refresh(&self) -> Result<(),PluginCommunicationError>;
}

/// When communicating an error to the main program, this allows to choose an appropriate handling
/// method. If the error does not prevent text updates, you likely just want to print it to stderr.
/// If it makes further processing impossible but doesn't cause an outright crash, consider
/// showing it instead of the usual text instead.
#[derive(Debug)]
pub enum PluginError {
    /// Use this variant if your error is not critical for the plugin's operation, but should still
    /// be communicated to the main program. The main program currently just calls eprintln! with
    /// it, but this might change if it gets more features (like logging). If you want a
    /// verbose/short type of error, send two errors, first one with this variant that holds the
    /// verbose error, afterwards one with ShowInsteadOfText that just replaces the text with a
    /// short error.
    PrintToStdErr(String),
    /// This notifies the main program that an error was encountered and no future text updates
    /// from this plugin are expected. The main program will replace the last text output by this
    /// error message. If you want to send both, a verbose error for the terminal/log and a short
    /// one to be displayed in the status bar, first send a PrintToStdErr variant with a verbose
    /// error, and then the short text as this variant.
    ShowInsteadOfText(String)
}
impl std::error::Error for PluginError {}
impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            PluginError::PrintToStdErr(x) => x,
            PluginError::ShowInsteadOfText(x) => x
        })
    }
}

/// This is implemented by the main program and passed to your module when creating the runnable.
/// Please note that the functions on this might eventually block until the main module finished
/// processing them. 
pub trait MsgModuleToMain : Send {
    /// Invoke this to update your text. This triggers the main program to build a new line of
    /// stdout output. While this can in theory return an error, that should practically never
    /// happen. If this errors, you should probably clean up your resources and return from the
    /// run() function. In other words, act as if main had sent you a quit command.
    fn send_update(&self, text : Result<String, PluginError>) -> Result<(),PluginCommunicationError>;
}

/// Interface your module should implement. All functions of this will be called in the main thread.
pub trait SwayStatusModule {
    ///The plugin's name. Must be globally unique. Shown to users in the config file.
    fn get_name(&self) -> &str;

    ///Called by the main program with a deserializer that holds the config for an instance of this
    ///module.
    ///In almost all cases it's enough to just call `erased_serde::deserialize(deserializer)?` in
    ///this function.
    fn deserialize_config<'de, 'p>(&'p self, deserializer : &mut (dyn erased_serde::Deserializer + 'de)) -> Result<Box<dyn SwayStatusModuleInstance + 'p>,erased_serde::Error>;

    ///This is used for the command line option to print a default configuration.
    ///Let it return your config with all defaults (including optional fields).
    fn get_default_config<'p>(&'p self) -> Box<dyn SwayStatusModuleInstance + 'p>;
}

///This is what `SwayStatusModuleInstance::make_runnable()` returns. The main function of your module.
///Will be called in a worker thread.
pub trait SwayStatusModuleRunnable : Send {
    ///Starts executing this module.
    fn run(&self); 
}

///Implement this trait on a struct that holds the configuration for a single instance of your
///plugin. The make_runnable then creates a runnable that gets moved to a different thread.
///In addition to making the runnable, the `make_runnable()` method also needs to return a
///MsgMainToModule trait object.
pub trait SwayStatusModuleInstance : erased_serde::Serialize { 
    ///The main initialization function. Takes the 2 communication channels and a configuration.
    ///The config is a trait object of the same type you provide in `get_default_config()` and 
    ///`deserialize_config()`.
    fn make_runnable<'p>(&'p self, to_main : Box<dyn MsgModuleToMain + 'p>) -> (Box<dyn SwayStatusModuleRunnable + 'p>, Box<dyn MsgMainToModule + 'p>);
}
serialize_trait_object!(SwayStatusModuleInstance);

///Error type used by send functions.
#[derive(Debug)]
pub struct PluginCommunicationError;

impl std::fmt::Display for PluginCommunicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,"Communication between plugin and main program terminated unexpectedly")
    }
}

impl std::error::Error for PluginCommunicationError {}
