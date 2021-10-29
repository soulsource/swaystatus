//! This module encapsulates all interaction with pulseaudio.
//! It is the only place in this plugin that explicitly includes unsafe code.

use std::sync::{Arc, Weak};
use std::ffi::{CString, CStr, c_void};
use std::os::raw::{c_int, c_char};
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
    scratch : &'c mut  ContextScratch, //This scratch space is needed, because user data pointers in pulse need to survive between iterations. Yes, that's insane. No, I can't do anything about it...
    main : &'c Pulse 
}

impl<'c> PulseContext<'c> {
    pub(super) fn create<'m>(pulse : &'m Pulse, scratch : &'m mut ContextScratch) -> Result<PulseContext<'m>, PulseContextCreationError> {
        let api = pulse.main_loop.get_api();
        if api.is_null() {
            Err(PulseContextCreationError::FailedToGetPulseApi)
        }
        else {
            if let Ok(plugin_name) = CString::new("Swaystatus Pulse Plugin") {
                let context = unsafe { pa_context_new(api, plugin_name.as_ptr()) };
                if context.is_null() {
                    Err(PulseContextCreationError::ContextNewFailed)
                }
                else {
                    Ok(PulseContext { context, scratch, main : pulse})
                }
            }
            else {
                Err(PulseContextCreationError::SettingNameFailed)
            }
        }
    }
    pub(super) fn iterate(&mut self, relevant_sink : &Option<SinkHandle>) -> Result<IterationResult,MainLoopIterationError> {
        self.scratch.sink_we_care_about = relevant_sink.clone();
        self.scratch.volume = None;
        self.scratch.default_sink = None;
        self.main.main_loop.iterate()?;
        return Ok(IterationResult {
            default_sink : self.scratch.default_sink.clone(),
            volume : self.scratch.volume.clone()
        });
    }
    pub(super) fn get_state(&self) -> PaContextState {
        unsafe { pa_context_get_state(self.context) }
    }
    pub(super) fn connect_and_set_callbacks(&mut self) -> Result<(), PulseContextConnectError> {
        unsafe {
            pa_context_set_state_callback(self.context,Some(Self::on_context_state_change),self.scratch as *mut ContextScratch as *mut c_void);
            let connect_result = pa_context_connect(self.context,std::ptr::null(),PaContextFlags::NoAutoSpawnNoFail,std::ptr::null());
            if connect_result == 0 {
                Ok(())
            }
            else {
                Err(PulseContextConnectError(connect_result))
            }
        }
    }

    pub(super) fn refresh_default_sink(&mut self) {
        unsafe {pa_operation_unref(pa_context_get_server_info(self.context,Some(Self::on_server_info_received),self.scratch as *mut ContextScratch as *mut c_void)); }
    }

    pub(super) fn refresh_volume(&mut self, sink : &SinkHandle) {
        unsafe {pa_operation_unref(pa_context_get_sink_info_by_name(self.context,sink.sink.as_ptr(),Some(Self::on_sink_info_received),self.scratch as *mut ContextScratch as *mut c_void));}
    }

    extern fn on_context_state_change(context : *mut PaContext, scratch : *mut c_void) {
        unsafe {
            match pa_context_get_state(context) {
                PaContextState::Ready => {
                    pa_context_set_subscribe_callback(context,Some(Self::on_context_event),scratch);
                    pa_operation_unref(pa_context_subscribe(context,0x80 /* SERVER */ | 0x01 /* SINK */, None, std::ptr::null_mut()));
                }
                _ => {}
            }
        }
    }

    extern fn on_context_event(context : *mut PaContext,event_type : c_int,index: u32,scratch : *mut c_void) {
        assert!(!context.is_null());
        assert!(!scratch.is_null());
        unsafe {
            let facility = 0x000f & event_type;
            match facility {
                0x0 /* SINK */ => {
                    pa_operation_unref(pa_context_get_sink_info_by_index(context,index,Some(Self::on_sink_info_received),scratch as *mut c_void));
                }
                0x7 /* SERVER */ => {
                    pa_operation_unref(pa_context_get_server_info(context,Some(Self::on_server_info_received),scratch as *mut c_void));
                }
                _ /* should not happen */ => {
                    assert!(false);
                }
            }
        }
    }

    extern fn on_sink_info_received(_context : *mut PaContext, sink_info : *const PaSinkInfo, _eol : c_int, scratch_void : *mut c_void) {
        if sink_info.is_null() {
            return;
        }
        unsafe {
            let scratch = scratch_void as *mut ContextScratch;
            if let Some(s) = &(*scratch).sink_we_care_about {
                if s.sink.as_c_str() == CStr::from_ptr((*sink_info).sink_name) {
                    let avg_volume = pa_cvolume_avg(&(*sink_info).volume);
                    const NORM : u32 = 0x10000;
                    let volume = (avg_volume as f32) / (NORM as f32);
                    let balance = pa_cvolume_get_balance(&(*sink_info).volume, &(*sink_info).channel_map);
                    let muted = (*sink_info).mute != 0;

                    (*scratch).volume = Some(Volume {volume, balance, muted});
                }
            }
        }
    }

    extern fn on_server_info_received(_context : *mut PaContext, server_info : *const PaServerInfo, scratch : *mut c_void) {
        if server_info.is_null() {
            return;
        }
        unsafe {
            (*(scratch as *mut ContextScratch)).default_sink= Some(SinkHandle::from(CStr::from_ptr((*server_info).default_sink_name)));
        }
    }
}

impl<'c> Drop for PulseContext<'c> {
    fn drop(&mut self) {
        unsafe {pa_context_unref(self.context)}
    }
}

#[derive(Debug)]
pub struct PulseContextConnectError(i32);
impl std::fmt::Display for PulseContextConnectError {
    fn fmt(&self, f : &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pulse Context connect error: ")?;
        match self.0 {
            0 => { write!(f,"No error occured.") }
            1 => { write!(f,"Access failure") }
            2 => { write!(f,"Unknown command") }
            3 => { write!(f,"Invalid argument") }
            4 => { write!(f,"Entity exists") }
            5 => { write!(f,"No such entity") }
            6 => { write!(f,"Connection refused") }
            7 => { write!(f,"Protocol error") }
            8 => { write!(f,"Timeout") }
            9 => { write!(f,"No authentication key") }
            10 => { write!(f,"Internal error") }
            11 => { write!(f,"Connection terminated") }
            12 => { write!(f,"Entity killed") }
            13 => { write!(f,"Invalid server") }
            14 => { write!(f,"Module initialization failed") }
            15 => { write!(f,"Bad state") }
            16 => { write!(f,"No data") }
            17 => { write!(f,"Incompatible protocol version") }
            18 => { write!(f,"Data too large") }
            19 => { write!(f,"Operation not supported") }
            20 => { write!(f,"The error code was unknown to the client") }
            21 => { write!(f,"Extension does not exist.") }
            22 => { write!(f,"Obsolete functionality.") }
            23 => { write!(f,"Missing implementation.") }
            24 => { write!(f,"The caller forked without calling execve() and tried to reuse the context.") }
            25 => { write!(f,"An IO error happened.") }
            26 => { write!(f,"Device or resource busy.") }
            _ => { write!(f,"Unknown error. Not documented at time of writing.") }
        }
    }
}
impl std::error::Error for PulseContextConnectError {}


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
    fn iterate(&self) -> Result<(),MainLoopIterationError> {
        let i = unsafe { pa_mainloop_iterate(self.main_loop, 1, std::ptr::null_mut()) };
        if i >= 0 {
            Ok(())
        }
        else {
            Err(MainLoopIterationError(i))
        }
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
pub struct MainLoopIterationError(i32);
impl std::fmt::Display for MainLoopIterationError {
    fn fmt(&self, f : &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Failed to iterate pulseaudio main loop. Error {}", self.0)
    }
}
impl std::error::Error for MainLoopIterationError {}

#[derive(Debug)]
pub struct MainLoopCreationError {}
impl std::fmt::Display for MainLoopCreationError {
    fn fmt(&self, f : &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Failed to create Pulse Main loop, PulseAudio returned a null pointer.")
    }
}
impl std::error::Error for MainLoopCreationError {}

#[derive(Clone, PartialEq, Eq, Debug)]
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

impl From<&CStr> for SinkHandle {
    fn from(s : &CStr) -> SinkHandle {
        SinkHandle { sink : CString::from(s) }
    }
}

pub(super) struct IterationResult {
    pub default_sink : Option<SinkHandle>,
    pub volume : Option<Volume>,
}

#[derive(Clone, PartialEq)]
pub(super) struct Volume {
    pub volume : f32,
    pub balance : f32,
    pub muted : bool
}

pub(super) struct ContextScratch {
    /* output */
    volume : Option<Volume>,
    default_sink : Option<SinkHandle>,

    /* input */
    sink_we_care_about : Option<SinkHandle>
}

impl Default for ContextScratch {
    fn default() -> Self {
        ContextScratch {
            volume : None,
            default_sink : None,
            sink_we_care_about : None
        }
    }
}

#[allow(dead_code)] //this is not dead code, but it seems the compiler doesn't understand the usage in FFI...
#[repr(C)] pub(super) enum PaContextState {
    Unconnected,
    Connecting,
    Authorizing,
    SettingName,
    Ready,
    Failed,
    Terminated
}

#[allow(dead_code)] //this is partially dead code, but the unused enum values are kept for readability reasons.
#[repr(C)] enum PaContextFlags {
    NoFlags = 0,
    NoAutoSpawn = 1,
    NoFail = 2,
    NoAutoSpawnNoFail = 3
}

#[repr(C)] struct PaSinkInfo {
    sink_name : *const c_char,
    index : u32,
    description : *const c_char,
    sample_spec : PaSampleSpec,
    channel_map : PaChannelMap,
    owner_module : u32,
    volume : PaCVolume,
    mute : c_int,
    monitor_source : u32,
    monitor_source_name : *const c_char,
    latency : u64,
    driver : *const c_char,
    flags : c_int,
    proplist : *mut PaPropList,
    configured_latency : u64,
    base_volume : u32,
    state : c_int,
    n_volume_steps : u32,
    card : u32,
    n_ports : u32,
    ports : *mut *mut PaSinkPortInfo,
    active_port : *mut PaSinkPortInfo,
    n_formats : u8,
    formats : *mut *mut PaFormatInfo
}

#[repr(C)] struct PaSampleSpec {
    format : c_int,
    rate : u32,
    channels : u8
}

#[repr(C)] struct PaChannelMap {
    channels : u8,
    map : [c_int; 32]
}

#[repr(C)] struct PaCVolume {
    channels : u8,
    values : [u32; 32]
}

#[repr(C)] struct PaServerInfo {
    user_name : *const c_char,
    host_name : *const c_char,
    server_version : *const c_char,
    server_name : *const c_char,
    sample_spec : PaSampleSpec,
    default_sink_name : *const c_char,
    default_source_name : *const c_char,
    cookie : u32,
    channel_map : PaChannelMap
}

#[repr(C)] struct PaPropList { _private: [u8; 0]}

///This actually holds meaningful data, but we don't care about it for now.
#[repr(C)] struct PaSinkPortInfo { _private: [u8; 0]}

///Similar to PaSinkPortInfo, we could interpret this, but won't need to.
#[repr(C)] struct PaFormatInfo { _private: [u8;0]}

#[repr(C)] struct PaMainloop { _private: [u8; 0] }
#[repr(C)] struct PaContext { _private: [u8; 0] }
#[repr(C)] struct PaOperation { _private: [u8; 0] }

///While we could in theory wrap the whole API, there's no need for it. We can treat it as an
///opaque type, because we never call any functions on it.
#[repr(C)] struct PaMainloopApi { _private: [u8; 0] }

///If we were to allow auto-spawning, we would need to actually implement this...
#[repr(C)] struct PaSpawnApi { _private: [u8; 0] }

type PaContextSuccessCb = extern fn(*mut PaContext, c_int, *mut c_void);
type PaContextSubscribeCb = extern fn(*mut PaContext,c_int,u32,*mut c_void);
type PaContextStateCb = extern fn(*mut PaContext, *mut c_void);
type PaSinkInfoCb = extern fn(*mut PaContext, *const PaSinkInfo, c_int, *mut c_void);
type PaServerInfoCb = extern fn(*mut PaContext, *const PaServerInfo, *mut c_void);

#[link(name = "pulse")]
extern {
    #[must_use]
    fn pa_mainloop_new() -> *mut PaMainloop;
    fn pa_mainloop_wakeup(_ : *mut PaMainloop);
    fn pa_mainloop_quit(_ : *mut PaMainloop, retval : c_int);
    fn pa_mainloop_free(_ : *mut PaMainloop);
    fn pa_mainloop_iterate(_ : *mut PaMainloop, block : c_int, return_value : *mut c_int) -> c_int;

    fn pa_mainloop_get_api(_ : *mut PaMainloop) -> *mut PaMainloopApi;
    #[must_use]
    fn pa_context_new(_ : *mut PaMainloopApi, name :*const c_char) -> *mut PaContext;
    fn pa_context_unref(_ : *mut PaContext);
    fn pa_context_get_state(_ : *mut PaContext) -> PaContextState;
    fn pa_context_connect(_: *mut PaContext, server : *const c_char, flags : PaContextFlags, api : *const PaSpawnApi) -> c_int;
    fn pa_context_set_state_callback(_: *mut PaContext, callback: Option<PaContextStateCb>, scratch : *mut c_void);
    fn pa_context_set_subscribe_callback(_: *mut PaContext,callback : Option<PaContextSubscribeCb>,scratch : *mut c_void);
    #[must_use]
    fn pa_context_subscribe(_: *mut PaContext, subscription_mask : c_int, callback : Option<PaContextSuccessCb>, scratch : *mut c_void) -> *mut PaOperation;
    fn pa_operation_unref(operation : *mut PaOperation);

    #[must_use]
    fn pa_context_get_sink_info_by_index(_: *mut PaContext, sink_index : u32, callback : Option<PaSinkInfoCb>, scratch : *mut c_void) -> *mut PaOperation;
    fn pa_context_get_sink_info_by_name(_: *mut PaContext, sink : *const c_char, callback : Option<PaSinkInfoCb>, scratch : *mut c_void) -> *mut PaOperation;
    #[must_use]
    fn pa_context_get_server_info(_: *mut PaContext, callback : Option<PaServerInfoCb>, scratch : *mut c_void) -> *mut PaOperation;

    fn pa_cvolume_avg(volume : *const PaCVolume) -> u32;
    fn pa_cvolume_get_balance(volume : *const PaCVolume, channel_map : *const PaChannelMap) -> f32;
}
