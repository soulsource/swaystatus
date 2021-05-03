//! This module encapsulates all interaction with pulseaudio.
//! It is the only place in this plugin that explicitly includes unsafe code.

use std::sync::{Arc, Weak};
use std::ffi::{c_void, CString};
use libc::{c_int, size_t};
use std::os::raw::c_char;

use crate::config::Sink;
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
    pub fn init(config : &Sink) -> Self {
        let main_loop = Arc::new(PulseMainLoop::new());
        //TODO:Use the sink parameter.
        Pulse {
            main_loop, 
            _marker :PhantomData
        }
    }
    pub fn get_wake_up(&self) -> PulseWakeUp {
        PulseWakeUp { main_loop : Arc::downgrade(&self.main_loop) }
    }
    pub fn is_valid(&self) -> bool {
        self.main_loop.is_valid()
    }
    pub fn create_context<'c>(&'c self) -> PulseContext<'c> {
        let context = if self.main_loop.is_valid() {
            let api = self.main_loop.get_api();
            if !api.is_null() {
                let plugin_name = CString::new("Swaystatus Pulse Plugin").expect("Pulse context name couldn't be set");
                unsafe { pa_context_new(api, plugin_name.as_ptr()) }
            }
            else {
                std::ptr::null_mut()
            }
        }
        else {
            std::ptr::null_mut()
        };
        PulseContext { context, _marker : PhantomData }
    }
}

pub struct PulseContext<'c> {
    context : *mut PaContext,
    _marker : PhantomData<&'c PulseMainLoop>
}

impl<'c> PulseContext<'c> {
    pub fn is_valid(&self) -> bool {
        !self.context.is_null()
    }
}

impl<'c> Drop for PulseContext<'c> {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe {pa_context_unref(self.context)}
        }
    }
}

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
        if !self.main_loop.is_null() {
            unsafe { pa_mainloop_wakeup(self.main_loop); }
        }
    }
    fn new() -> Self {
        unsafe {
            let pointer = pa_mainloop_new();
            Self { main_loop : pointer }
        }
    }
    fn is_valid(&self) -> bool {
        !self.main_loop.is_null()
    }
    fn get_api(&self) -> *mut PaMainloopApi {
        assert!(self.is_valid());
        unsafe { pa_mainloop_get_api(self.main_loop) }
    }
}
impl Drop for PulseMainLoop {
    fn drop(&mut self) {
        if !self.main_loop.is_null() {
            unsafe { 
                pa_mainloop_quit(self.main_loop, 0);
                pa_mainloop_free(self.main_loop); 
            }
        }
        self.main_loop = std::ptr::null_mut();
    }
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
