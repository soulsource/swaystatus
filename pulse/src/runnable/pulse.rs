//! This module encapsulates all interaction with pulseaudio.
//! It is the only place in this plugin that explicitly includes unsafe code.

use std::sync::{Arc, Weak};
use crate::config::Sink;
use std::marker::PhantomData;

pub(super) struct Pulse {
    main_loop : Arc<PulseMainLoop>, //Beware: Never ever clone this except for PulseWakeUp!

    //Pulse is _definitely_ not sync. It is supposed to be Send though.
    marker : PhantomData<std::cell::RefCell<i8>>
}

//In general it's a really stupid idea to Send Arcs of non-Sync types around.
//However this Arc is only cloned under one very specific condition: creating a PulseWakeUp.
//Apart from that one (thread-safe) exception it's actually single-ownership.
unsafe impl Send for Pulse {}

impl Pulse {
    pub fn init(config : &Sink) -> Self {
        //TODO:Use the sink parameter.
        Pulse {
            //TODO: Initialize main_loop on the pulse-side.
            main_loop : Arc::new(PulseMainLoop {}),
            marker :PhantomData
        }
    }
    pub fn get_wake_up(&self) -> PulseWakeUp {
        PulseWakeUp { main_loop : Arc::downgrade(&self.main_loop) }
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
    //TODO
}
impl PulseMainLoop {
    //this is intentionally all private. Nobody outside this module should call anything on this.
    fn awaken(&self) {
        //TODO!
    }
}
impl Drop for PulseMainLoop {
    fn drop(&mut self) {
        //TODO: if this gets dropped, free the pulse main loop.
    }
}
