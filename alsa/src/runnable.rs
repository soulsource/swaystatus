use std::{cell::RefCell, fmt::Display, error::Error, ffi::{CStr, CString}};

use formatable_float::FormattingError;
use libc::{c_int, c_char, c_uint, c_void, c_long, c_ushort, c_short};
use swaystatus_plugin::*;

use crate::{communication::MessagesFromMainReceiver, config::SElemAbstraction};

use super::config::AlsaVolumeConfig;

pub struct AlsaVolumeRunnable<'r>{
    to_main : Box<dyn MsgModuleToMain + 'r>,
    from_main : MessagesFromMainReceiver,
    config : &'r AlsaVolumeConfig,
}

impl<'r> AlsaVolumeRunnable<'r> {
    pub fn new(to_main : Box<dyn MsgModuleToMain + 'r>, from_main : MessagesFromMainReceiver, config : &'r AlsaVolumeConfig) -> Self {
        Self { to_main, from_main, config }
    }
    fn send_error_to_main<E>(&self, err : E) where E : std::error::Error {
        self.to_main.send_update(Err(PluginError::ShowInsteadOfText(String::from("Error")))).expect("Tried to tell main thread that an error occured. Main thread isn't listening any more.");
        self.to_main.send_update(Err(PluginError::PrintToStdErr(err.to_string()))).expect("Tried to tell main thread that an error occured. Main thread isn't listening any more.");
    }
    fn run_internal(&self) -> Result<(), AlsaVolumeError>{
        //Using C callbacks in Rust is a minefield.
        //However, we can take the easy way out here, namely we only care about a single element, so we can just make a single data field ;-)
        //It still needs to be in a Cell though.
        let elem_name = match CString::new(&*self.config.element){
            Ok(s) => s,
            Err(_) => return Err(AlsaVolumeError::ConfigError),
        };
        let device = match CString::new(&*self.config.device){
            Ok(s) => s,
            Err(_) => return Err(AlsaVolumeError::ConfigError),
        };

        let elem_scratch_space = RefCell::new(None);
        let mixer_scratch_space = MixerScratchSpace{
            elem_name: &elem_name,
            elem_scratch: &elem_scratch_space,
        };
        let mixer = open_mixer(0)?;
        register_selem(mixer.handle, &device, self.config.abstraction)?;
        unsafe { snd_mixer_set_callback(mixer.handle, Some(Self::mixer_callback)) };
        unsafe { snd_mixer_set_callback_private(mixer.handle, &mixer_scratch_space as *const MixerScratchSpace as *const c_void)};
        load_mixer(mixer.handle)?;

        let mut should_update_main_even_if_unchanged = true;
        loop {

            let descriptor_count = unsafe{snd_mixer_poll_descriptors_count(mixer.handle)};
            if descriptor_count < 0 {
                return Err(AlsaVolumeError::FailedToGetPollDescriptors);
            }
            let mut descriptors : Vec<libc::pollfd> = std::iter::once(libc::pollfd{ 
                    fd: self.from_main.file_handle().get_raw(),
                    events: libc::POLLIN,
                    revents: 0 
                })
                .chain(std::iter::repeat(
                    libc::pollfd{
                        fd: 0,
                        events: 0,
                        revents: 0,
                    }
                ))
                .take(descriptor_count as usize + 1)
                .collect();
            let descriptor_count = if descriptor_count > 0 { unsafe {snd_mixer_poll_descriptors(mixer.handle, &mut descriptors[1] , descriptor_count as c_uint)} } else { 0 };
            if descriptor_count < 0 {
                return Err(AlsaVolumeError::FailedToGetPollDescriptors);
            }
            let n = unsafe {libc::poll(descriptors.as_mut_ptr(), descriptors.len() as u64, -1)};
            if n < 0 && n != libc::EINTR {
                return Err(AlsaVolumeError::UnexpectedPollError);
            }
            //first check if there's any data on our pipe from main.
            loop {
                match self.from_main.receive(){
                    Ok(Some(message)) => match message{
                        crate::communication::MessagesFromMain::Quit => { return Ok(())},
                        crate::communication::MessagesFromMain::Refresh => { should_update_main_even_if_unchanged = true; },
                    },
                    Ok(None) => break, //main has nothing more to say.
                    Err(e) => match e {
                        crate::communication::pipe_chan::ReceiveError::SenderHasHungUp => { return Err(AlsaVolumeError::MainHungUpWithoutQuit) },
                        crate::communication::pipe_chan::ReceiveError::UnknownError => { return Err(AlsaVolumeError::ErrorInPluginCommunication) },
                    },
                }
            }
            let old_values = mixer_scratch_space.elem_scratch.borrow().clone();
            let anything_new_from_alsa = n > (if descriptors[0].revents != 0 { 1 } else { 0 });
            if anything_new_from_alsa {
                let mut revents = 0;
                let worked = unsafe {snd_mixer_poll_descriptors_revents(mixer.handle, &descriptors[1], descriptor_count as c_uint,&mut revents) };
                if worked < 0{
                    return Err(AlsaVolumeError::UnexpectedPollError);
                }
                if (revents as c_short) & (libc::POLLERR | libc::POLLNVAL) != 0 {
                    return Err(AlsaVolumeError::DeviceRemoved);
                }
                if (revents as c_short) & libc::POLLIN != 0 {
                    let handling_worked = unsafe {snd_mixer_handle_events(mixer.handle)};
                    if handling_worked < 0 {
                        return Err(AlsaVolumeError::EventHandlingError);
                    }
                }
            }
            let new_values = mixer_scratch_space.elem_scratch.borrow().clone();
            if new_values != old_values || should_update_main_even_if_unchanged {
                self.send_updated_values_to_main(new_values).expect("Tried to update main thread, but it seems to be gone?");
            }
            should_update_main_even_if_unchanged = false;
        }
    }

    fn send_updated_values_to_main(&self, volume : Option<ElemVolumeInfo>) -> Result<(),PluginCommunicationError> {
        match volume{
            Some(volume) => {
                let formatted_volume = self.config.format_volume(volume.volume, volume.mute);
                match formatted_volume {
                    Ok(msg) => { self.to_main.send_update(Ok(msg)) }
                    Err(e) => {
                        let full_message = e.to_string();
                        match e {
                            FormattingError::EmptyMap{ numeric_fallback } => {
                                self.to_main.send_update(Err(PluginError::ShowInsteadOfText(numeric_fallback)))?;
                                self.to_main.send_update(Err(PluginError::PrintToStdErr(full_message)))
                            }
                        }
                    }
                }
            },
            None => {
                self.to_main.send_update(Err(PluginError::ShowInsteadOfText("Unknown".into())))
            },
        }
    }

    extern "C" fn mixer_callback(mixer : SndMixerHandle, flags : c_uint, element :  SndMixerElemHandle) -> c_int {
        if flags & (1<<2) != 0 { //SND_CTL_EVENT_MASK_ADD
            //check if the newly added element is the one we are looking for.
            let scratch : &MixerScratchSpace = unsafe{&*(snd_mixer_get_callback_private(mixer) as *const MixerScratchSpace)};            
            let elem_name = unsafe { CStr::from_ptr(snd_mixer_selem_get_name(element)) };
            if elem_name == scratch.elem_name {
                unsafe {snd_mixer_elem_set_callback(element, Some(Self::element_callback))};
                unsafe {snd_mixer_elem_set_callback_private(element,scratch.elem_scratch as *const ElemScratchSpace as *const c_void)};
                0
            } else {
                0
            }
        } else {
            0
        }
    }

    extern "C" fn element_callback(element : SndMixerElemHandle, flags : c_uint) -> c_int {
        //okay, we hit the right element, sooo
        if flags == (!0) { //SND_CTL_EVENT_MASK_REMOVE
            0
        } else { //could check further to exclude more spurious wake-ups, but for now...
            //we don't do any magic here. Just sum up all channel's values and call it a day.
            let (count, volume_sum) = ALL_CHANNELS.iter()
                .filter_map(|channel| get_volume_for_channel(element, *channel))
                .fold((0,0), |(c, ov), v| (c+1, ov + v));
            let average = if count == 0 { None } else { Some(volume_sum / count)};
            let range = get_volume_range(element);
            let normalized = average.zip(range).map(|(average, range)| ((average - range.0)*100000) / (range.1 - range.0));
            let scratch = unsafe{&*(snd_mixer_elem_get_callback_private(element) as *const ElemScratchSpace)};
            let has_mute = unsafe{ snd_mixer_selem_has_playback_switch(element) != 0};
            let is_mute = if has_mute {
                !ALL_CHANNELS.iter().any(|channel| get_switch_for_channel(element, *channel))
            } else { false };
            *scratch.borrow_mut() = normalized.map(|volume| ElemVolumeInfo{volume : volume as f32 / 100000.0, mute : is_mute});
            0
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ElemVolumeInfo{
    volume : f32,
    mute : bool
}

type ElemScratchSpace = RefCell<Option<ElemVolumeInfo>>;

struct MixerScratchSpace<'a>{
    elem_name : &'a CStr,
    elem_scratch : &'a ElemScratchSpace,
}

impl<'r> SwayStatusModuleRunnable for AlsaVolumeRunnable<'r> {
    fn run(&self) {
        match self.run_internal(){
            Ok(()) => {},
            Err(e) => self.send_error_to_main(e),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum AlsaVolumeError{
    FailedToOpenMixer,
    FailedToLoadElements,
    FailedToGetPollDescriptors,
    UnexpectedPollError,
    ErrorInPluginCommunication,
    MainHungUpWithoutQuit,
    DeviceRemoved,
    EventHandlingError,
    ConfigError,
}

impl Display for AlsaVolumeError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlsaVolumeError::FailedToOpenMixer => write!(f, "Failed to open Mixer"),
            AlsaVolumeError::FailedToLoadElements => write!(f, "Failed to load Mixer elements"),
            AlsaVolumeError::FailedToGetPollDescriptors => write!(f, "Failed to get poll descriptors"),
            AlsaVolumeError::UnexpectedPollError => write!(f, "Polling for updates failed for an unhandled reason. Debug."),
            AlsaVolumeError::ErrorInPluginCommunication => write!(f, "Something went wrong with the pipe from main thread. Debug."),
            AlsaVolumeError::MainHungUpWithoutQuit => write!(f, "Main thread ended communication without saying goodbye. Debug."),
            AlsaVolumeError::DeviceRemoved => write!(f, "Device removed. Unsupported for now."),
            AlsaVolumeError::EventHandlingError => write!(f, "Failure while handling mixer events. Debug."),
            AlsaVolumeError::ConfigError => write!(f, "Configuration contains non-ASCI values for device or element."),
        }
    }
}
impl Error for AlsaVolumeError{}

struct MixerHandleScopeGuard{
    handle : SndMixerHandle
}
impl Drop for MixerHandleScopeGuard{
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {snd_mixer_close(self.handle)};
        }
    }
}

fn open_mixer(mode : c_int) -> Result<MixerHandleScopeGuard,AlsaVolumeError>{
    let mut handle : SndMixerHandle = std::ptr::null();
    let error_code = unsafe {snd_mixer_open(&mut handle, mode)};
    if error_code == 0 {
        Ok(MixerHandleScopeGuard { handle })
    } else {
        Err(AlsaVolumeError::FailedToOpenMixer)
    }
}

fn load_mixer(mixer : SndMixerHandle) -> Result<(), AlsaVolumeError>{
    let error_code = unsafe { snd_mixer_load(mixer)};
    if error_code == 0 {
        Ok(())
    } else {
        Err(AlsaVolumeError::FailedToLoadElements)
    }
}

fn register_selem(mixer : SndMixerHandle, device : &CStr, abstraction : SElemAbstraction) -> Result<(), AlsaVolumeError>{
    let options = SndMixerSelemRegopt{
        ver: 1,
        abstraction: match abstraction {
            SElemAbstraction::None => SndMixerSelemRegoptAbstract::None,
            SElemAbstraction::Basic => SndMixerSelemRegoptAbstract::Basic,
        },
        device: device.as_ptr(),
        playback_pcm: std::ptr::null(),
        capture_pcm: std::ptr::null(),
    };
    let error_code = unsafe {snd_mixer_selem_register(mixer, &options, std::ptr::null_mut())};
    if error_code == 0 {
        Ok(())
    } else {
        Err(AlsaVolumeError::FailedToOpenMixer)
    }
}

fn get_volume_for_channel(element : SndMixerElemHandle, channel : SndMixerSelemChannelIdT) -> Option<c_long>{
    if unsafe {snd_mixer_selem_has_playback_channel(element, channel) > 0} {
        let mut value : c_long = 0;
        if unsafe { snd_mixer_selem_get_playback_volume(element, channel, &mut value) == 0}{
            Some(value)
        } else {
            None
        }
    } else {
        None
    }
}

fn get_switch_for_channel(element : SndMixerElemHandle, channel : SndMixerSelemChannelIdT) -> bool {
    let mut switch = 0;
    let worked = unsafe { snd_mixer_selem_get_playback_switch(element, channel, &mut switch)};
    worked == 0 && switch != 0
}

fn get_volume_range(element : SndMixerElemHandle) -> Option<(c_long, c_long)>{
    let mut min = 0;
    let mut max = 0;
    if unsafe { snd_mixer_selem_get_playback_volume_range(element, &mut min, &mut max) == 0 } {
        Some((min, max))
    } else {
        None
    }
}

#[repr(C)] struct SndMixerT { _private: [u8; 0]}
#[repr(C)] struct SndPcmT { _private: [u8; 0]}
#[repr(C)] struct SndMixerClassT { _private: [u8; 0]}
#[repr(C)] struct SndMixerElemT { _private: [u8; 0]}

type SndMixerHandle = *const SndMixerT;
type SndMixerElemHandle = *const SndMixerElemT;

#[repr(C)] enum SndMixerSelemRegoptAbstract {
    None,
    Basic,
}
#[repr(C)] struct SndMixerSelemRegopt {
    ver : c_int,
    abstraction : SndMixerSelemRegoptAbstract,
    device : *const c_char,
    playback_pcm : *const SndPcmT,
    capture_pcm : *const SndPcmT,
}

#[derive(Clone,Copy)]
#[repr(C)] enum SndMixerSelemChannelIdT {
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
    FrontCenter,
    Woofer,
    SideLeft,
    SideRight,
    RearCenter,
}

static ALL_CHANNELS : [SndMixerSelemChannelIdT;9] = [
    SndMixerSelemChannelIdT::FrontLeft,
    SndMixerSelemChannelIdT::FrontRight,
    SndMixerSelemChannelIdT::RearLeft,
    SndMixerSelemChannelIdT::RearRight,
    SndMixerSelemChannelIdT::FrontCenter,
    SndMixerSelemChannelIdT::Woofer,
    SndMixerSelemChannelIdT::SideLeft,
    SndMixerSelemChannelIdT::SideRight,
    SndMixerSelemChannelIdT::RearCenter,
];


type SndMixerCallbackT = extern "C" fn(SndMixerHandle, c_uint, SndMixerElemHandle) -> c_int;
type SndMixerElemCallbackT = extern "C" fn(SndMixerElemHandle, c_uint) -> c_int;
#[link(name = "asound")]
extern "C" {
    //int snd_mixer_open 	( 	snd_mixer_t **  	mixerp,	int  	mode ) 	
    fn snd_mixer_open(mixer : *mut SndMixerHandle, mode : c_int) -> c_int;
    //int snd_mixer_close 	( 	snd_mixer_t *  	mixer	) 	
    fn snd_mixer_close(mixer : SndMixerHandle) -> c_int;
    //int snd_mixer_selem_register(snd_mixer_t *mixer, struct snd_mixer_selem_regopt *options, snd_mixer_class_t **classp);
    fn snd_mixer_selem_register(mixer : SndMixerHandle, options: *const SndMixerSelemRegopt, class: *mut *const SndMixerClassT) -> c_int;
    //void snd_mixer_set_callback 	( 	snd_mixer_t *  	obj, snd_mixer_callback_t  	val 	) 	
    fn snd_mixer_set_callback(mixer : SndMixerHandle, callback : Option<SndMixerCallbackT>);
    //void snd_mixer_set_callback_private 	( 	snd_mixer_t *  	mixer, void *  	val 	) 		
    fn snd_mixer_set_callback_private(mixer : SndMixerHandle, value : *const c_void); //the *const is a lie, but one that we need for Stacked Borrows sanity.
    //void* snd_mixer_get_callback_private 	( 	const snd_mixer_t *  	mixer	) 	
    fn snd_mixer_get_callback_private(mixer : SndMixerHandle) -> *const c_void; //the *const is a lie, but one that we need for Stacked Borrows sanity.

    //int snd_mixer_load 	( 	snd_mixer_t *  	mixer	) 	
    fn snd_mixer_load(mixer : SndMixerHandle) -> c_int;

    //const char* snd_mixer_selem_get_name 	( 	snd_mixer_elem_t *  	elem	) 
    fn snd_mixer_selem_get_name(element: SndMixerElemHandle) -> *const c_char;
    //void snd_mixer_elem_set_callback 	( 	snd_mixer_elem_t *  	mixer,		snd_mixer_elem_callback_t  	val 	) 	
    fn snd_mixer_elem_set_callback(element : SndMixerElemHandle, callback : Option<SndMixerElemCallbackT>);
    //void snd_mixer_elem_set_callback_private 	( 	snd_mixer_elem_t *  	mixer,		void *  	val 	) 	
    fn snd_mixer_elem_set_callback_private(element : SndMixerElemHandle, value : *const c_void);
    //void* snd_mixer_elem_get_callback_private 	( 	const snd_mixer_elem_t *  	mixer	) 	
    fn snd_mixer_elem_get_callback_private(element : SndMixerElemHandle) -> *const c_void;

    //int snd_mixer_selem_has_playback_channel 	( 	snd_mixer_elem_t *  	elem,		snd_mixer_selem_channel_id_t  	channel 	) 	
    fn snd_mixer_selem_has_playback_channel(element : SndMixerElemHandle, channel : SndMixerSelemChannelIdT) -> c_int;
    //int snd_mixer_selem_get_playback_volume 	( 	snd_mixer_elem_t *  	elem,		snd_mixer_selem_channel_id_t  	channel,		long *  	value 	) 	
    fn snd_mixer_selem_get_playback_volume(element : SndMixerElemHandle, channel : SndMixerSelemChannelIdT, value : *mut c_long) -> c_int;
    //int snd_mixer_selem_get_playback_volume_range 	( 	snd_mixer_elem_t *  	elem,		long *  	min,		long *  	max 	) 	
    fn snd_mixer_selem_get_playback_volume_range(element : SndMixerElemHandle, min: *mut c_long, max : *mut c_long) -> c_int;

    //int snd_mixer_selem_has_playback_switch 	( 	snd_mixer_elem_t *  	elem	) 	
    fn snd_mixer_selem_has_playback_switch(element : SndMixerElemHandle) -> c_int;
    //int snd_mixer_selem_get_playback_switch 	( 	snd_mixer_elem_t *  	elem,		snd_mixer_selem_channel_id_t  	channel,		int *  	value 	) 	
    fn snd_mixer_selem_get_playback_switch(element : SndMixerElemHandle, channel : SndMixerSelemChannelIdT, value : *mut c_int) -> c_int;

    //int snd_mixer_poll_descriptors_count 	( 	snd_mixer_t *  	mixer	) 	
    fn snd_mixer_poll_descriptors_count(mixer : SndMixerHandle) -> c_int;
    //int snd_mixer_poll_descriptors 	( 	snd_mixer_t *  	mixer,		struct pollfd *  	pfds,		unsigned int  	space 	) 	
    fn snd_mixer_poll_descriptors(mixer : SndMixerHandle, fds : *mut libc::pollfd, space : c_uint) -> c_int;

    //int snd_mixer_poll_descriptors_revents 	( 	snd_mixer_t *  	mixer,	struct pollfd *  	pfds,	unsigned int  	nfds, unsigned short *  	revents ) 	
    fn snd_mixer_poll_descriptors_revents(mixer : SndMixerHandle, fds : *const libc::pollfd, nfds : c_uint, revents : *mut c_ushort) -> c_int;

    //int snd_mixer_handle_events 	( 	snd_mixer_t *  	mixer	) 	
    fn snd_mixer_handle_events(mixer : SndMixerHandle) -> c_int;
}