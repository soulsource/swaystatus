//! This module encapsulates all interaction with pulseaudio.
//! It is the only place in this plugin that explicitly includes unsafe code.

use std::sync::{Arc, Weak};
use std::ffi::{c_void, CString};
use libc::{c_int, size_t};
use std::os::raw::c_char;
use std::convert::TryFrom;

use std::marker::PhantomData;

pub(super) struct Pulse {
    main_loop : Arc<PulseMainLoop>, //Beware: Never ever clone this except for PulseWakeUp!
    
    //Make sure that Pulse never accidentally gets Sync. That's more of a reminder for myself,
    //given that Arc<PulseMainLoop> already is not Sync...
    _marker : PhantomData<std::cell::RefCell<i8>>
}

//In general it's a really stupid idea to Send Arcs of non-Sync types around.
//However this Arc is only cloned under one very specific condition: creating a PulseWakeUp.
//Apart from that one (thread-safe) exception it's actually single-ownership.
unsafe impl Send for Pulse {}

impl Pulse {
    pub fn create() -> Result<Self,MainLoopCreationError> {
        let main_loop = Arc::new(PulseMainLoop::new()?);
        Ok(Pulse {
            main_loop, 
            _marker :PhantomData
        })
    }
    pub fn get_wake_up(&self) -> PulseWakeUp {
        PulseWakeUp { main_loop : Arc::downgrade(&self.main_loop) }
    }
    
}

pub(super) struct PulseContext<'c> {
    context : *mut PaContext, //Can actually never be null, the nullptr case is handled by create, which returns an Err instead of a PulseContext then.
    main : &'c Pulse 
}

impl<'c> PulseContext<'c> {
    pub(super) fn create<'m>(pulse : &'m Pulse) -> Result<PulseContext<'m>, PulseContextCreationError> {
        let api = pulse.main_loop.get_api();
        if api.is_null() {
            Err(PulseContextCreationError::FailedToGetPulseApi)
        }
        else {
            let plugin_name = CString::new("Swaystatus Pulse Plugin").expect("Pulse context name couldn't be set");
            let context = unsafe { pa_context_new(api, plugin_name.as_ptr()) };
            if context.is_null() {
                Err(PulseContextCreationError::ContextNewFailed)
            }
            else {
                Ok(PulseContext { context, main : pulse})
            }
        }
    }
}

impl<'c> Drop for PulseContext<'c> {
    fn drop(&mut self) {
        unsafe {pa_context_unref(self.context)}
    }
}

#[derive(Debug)]
pub enum PulseContextCreationError {
    SettingNameFailed,
    FailedToGetPulseApi,
    ContextNewFailed
}
impl std::fmt::Display for PulseContextCreationError {
    fn fmt(&self, f : &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PulseContextCreationError::SettingNameFailed => {
                write!(f, "Pulse Context creation failed, error converting context name to C chars")
            }
            PulseContextCreationError::FailedToGetPulseApi => {
                write!(f, "Pulse Context creation failed, Pulse Main Loop didn't return a valid API")
            }
            PulseContextCreationError::ContextNewFailed => {
                write!(f, "Pulse Context creation failed, Pulse API didn't return a valid context")
            }
        }
    }
}
impl std::error::Error for PulseContextCreationError {}

/// Helper to wake up the Pulseaudio main loop.
/// This implements Send and Sync, because it's explicitly meant to allow cross-thread
/// communication. It only exposes the wake up function of pulse, and that function is in itself
/// send and sync (https://github.com/pulseaudio/pulseaudio/blob/master/src/pulse/mainloop.c).
pub struct PulseWakeUp {
    main_loop : Weak<PulseMainLoop>
}
unsafe impl Send for PulseWakeUp {}
unsafe impl Sync for PulseWakeUp {}

impl PulseWakeUp {
    pub fn wake_up(&self) -> Result<(), PulseWakeUpError> {
        match self.main_loop.upgrade() {
            Some(main_loop) => {
                main_loop.awaken();
                Ok(())
            }
            None => {
                Err(PulseWakeUpError {} )
            }
        }
    }
}

#[derive(Debug)]
pub struct PulseWakeUpError;
impl std::error::Error for PulseWakeUpError {}
impl std::fmt::Display for PulseWakeUpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f,"Failed to wake up Pulse main loop. It has been dropped already.")
    }
}

struct PulseMainLoop {
    main_loop : *mut PaMainloop,
}
impl PulseMainLoop {
    //this is intentionally all private. Nobody outside this module should call anything on this.
    fn awaken(&self) {
        unsafe { pa_mainloop_wakeup(self.main_loop); }
    }
    fn new() -> Result<Self,MainLoopCreationError> {
        unsafe {
            let pointer = pa_mainloop_new();
            if pointer.is_null() {
                Err(MainLoopCreationError{})
            }
            else {
                Ok(Self { main_loop : pointer })
            }
        }
    }
    fn get_api(&self) -> *mut PaMainloopApi {
        unsafe { pa_mainloop_get_api(self.main_loop) }
    }
}
impl Drop for PulseMainLoop {
    fn drop(&mut self) {
        unsafe { 
            pa_mainloop_quit(self.main_loop, 0);
            pa_mainloop_free(self.main_loop); 
        }
        self.main_loop = std::ptr::null_mut();
    }
}

#[derive(Debug)]
pub struct MainLoopCreationError {}
impl std::fmt::Display for MainLoopCreationError {
    fn fmt(&self, f : &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Failed to create Pulse Main loop, PulseAudio returned a null pointer.")
    }
}
impl std::error::Error for MainLoopCreationError {}

pub(super) struct SinkHandle {
    sink: CString
}

impl TryFrom<&str> for SinkHandle {
    type Error = std::ffi::NulError;
    fn try_from(s : &str) -> Result<Self, Self::Error> {
        let converted = std::ffi::CString::new(s)?;
        Ok(SinkHandle {
            sink : converted
        })
    }
}

pub(super) struct IterationResult {
    pub default_sink : Option<SinkHandle>,
    pub volume : Option<Volume>,
    pub state : Option<PaContextState>
}

pub(super) struct Volume {
    pub volume : f32,
    pub balance : f32
}

#[repr(C)] pub(super) enum PaContextState {
    Unconnected,
    Connecting,
    Authorizing,
    SettingName,
    Ready,
    Failed,
    Terminated
}

#[repr(C)] struct PaMainloop { _private: [u8; 0] }
#[repr(C)] struct PaContext { _private: [u8; 0] }

///While we could in theory wrap the whole API, there's no need for it. We can treat it as an
///opaque type, because we never call any functions on it.
#[repr(C)] struct PaMainloopApi { _private: [u8; 0] }

#[link(name = "pulse")]
extern {
    fn pa_mainloop_new() -> *mut PaMainloop;
    fn pa_mainloop_wakeup(_ : *mut PaMainloop);
    fn pa_mainloop_quit(_ : *mut PaMainloop, retval : c_int);
    fn pa_mainloop_free(_ : *mut PaMainloop);

    fn pa_mainloop_get_api(_ : *mut PaMainloop) -> *mut PaMainloopApi;
    fn pa_context_new(_ : *mut PaMainloopApi, name :*const c_char) -> *mut PaContext;
    fn pa_context_unref(_ : *mut PaContext);
}
