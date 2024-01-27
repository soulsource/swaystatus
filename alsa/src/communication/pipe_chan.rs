use std::{ffi::{c_int, c_void}, fmt::Display, error::Error};
use libc::{read, close, pipe2, O_NONBLOCK, EINTR };
use errno::{errno, set_errno, Errno}; //Why isn't this in libc?!?

/// Sends byte data to the corresponding receiver.
pub(crate) struct Sender {
    handle : FileHandle,
}
/// Receives byte data from the corresponding sender.
pub(crate) struct Receiver {
    handle : FileHandle,
}

impl Receiver {
    pub(crate) fn read_byte(&self) -> Result<Option<u8>, ReceiveError> {
        set_errno(Errno(0));
        let mut buf : u8 = 0;
        let status = unsafe {read(self.handle.get_raw(),&mut buf as *mut u8 as *mut c_void, 1)};
        if status == 0 {
            //need to check errno. if a signal interrupted, then there is no error and we just
            //retry.
            //If no error happened, then the sender has hung up and we return err.
            let e = errno();
            if e == Errno(0) {
                Err(ReceiveError::SenderHasHungUp)
            } else if e.0 == EINTR {
                self.read_byte()
            } else {
                //Not sure what to do
                todo!()
            }
        }
        else {
            todo!()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ReceiveError {
    SenderHasHungUp
}

impl Display for ReceiveError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Read failed, Sender has closed their end of the pipe.")
    }
}

impl Error for ReceiveError {}

pub(crate) fn create_pipe_chan() -> Result<(Sender, Receiver),()> {
    let mut handles : [c_int;2] = [0;2];
    let result = unsafe { pipe2(handles.as_mut_ptr(), O_NONBLOCK) };
    if result == -1 { 
        Err(())
    } else {
        Ok((
            Sender{handle : FileHandle{raw : handles[1]}},
            Receiver{handle : FileHandle{ raw : handles[0]}}
        ))
    }
}

struct FileHandle {
    raw : c_int,
}

impl FileHandle {
    pub(crate) fn get_raw(&self) -> c_int {
        self.raw
    }
}

impl Drop for FileHandle {
    fn drop(&mut self) {
        unsafe {
            close(self.raw);
        }
    }
}
