use std::{ffi::{c_int, c_void}, fmt::Display, error::Error};
use libc::{read, close, pipe2, O_NONBLOCK, EINTR, EAGAIN, EWOULDBLOCK, EPIPE };
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
        let e = errno();
        if status > 0 {
            Ok(Some(buf))
        } else if status == 0 && e == Errno(0) {
            Err(ReceiveError::SenderHasHungUp)
        } else if e.0 == EINTR {
            self.read_byte() //got interrupted by a signal, try again.
        } else if e.0 == EAGAIN || e.0 == EWOULDBLOCK {
            Ok(None) //nothing to receive
        } else {
            Err(ReceiveError::UnknownError)
        }
    }
}

impl Sender {
    pub(crate) fn send_byte(&self, byte : u8) -> Result<(), SendError> {
        set_errno(Errno(0));
        let status = unsafe {libc::write(self.handle.get_raw(), &byte as *const u8 as *const c_void, 1)};
        let e = errno();
        if status > 0 {
            Ok(())
        } else if e.0 == EINTR {
            self.send_byte(byte) //interrupted, retry
        } else if e.0 == EPIPE {
            Err(SendError::ReceiverHasHungUp)
        } else if e.0 == EAGAIN {
            Err(SendError::ChannelFullWouldBlock)
        } else {
            Err(SendError::UnknownError)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendError{
    ReceiverHasHungUp,
    ChannelFullWouldBlock,
    UnknownError
}

impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::ReceiverHasHungUp => write!(f, "Write failed, Receiver has closed their end of the pipe."),
            SendError::ChannelFullWouldBlock => write!(f, "Write failed, the pipe is clogged."),
            SendError::UnknownError => write!(f, "Write failed for unknown reasons. Probably a bug."),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiveError {
    SenderHasHungUp,
    UnknownError,
}

impl Display for ReceiveError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReceiveError::SenderHasHungUp => write!(f, "Read failed, Sender has closed their end of the pipe."),
            ReceiveError::UnknownError => write!(f, "Read failed for unknown reasons. Probably a bug."),
        }
        
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

#[cfg(test)]
mod pipe_chan_test {
    use libc::fcntl;

    use super::*;

    #[test]
    fn simple_send_read(){
        let (send, recv) = create_pipe_chan().unwrap();
        assert_eq!(recv.read_byte(), Ok(None));
        send.send_byte(5).unwrap();
        send.send_byte(27).unwrap();
        assert_eq!(recv.read_byte(), Ok(Some(5)));
        assert_eq!(recv.read_byte(), Ok(Some(27)));
        assert_eq!(recv.read_byte(), Ok(None));
    }

    #[test]
    fn simple_drop_sender(){
        let (send, recv) = create_pipe_chan().unwrap();
        assert_eq!(recv.read_byte(), Ok(None));
        send.send_byte(5).unwrap();
        send.send_byte(27).unwrap();
        drop(send);
        assert_eq!(recv.read_byte(), Ok(Some(5)));
        assert_eq!(recv.read_byte(), Ok(Some(27)));
        assert_eq!(recv.read_byte(), Err(ReceiveError::SenderHasHungUp));
    }

    #[test]
    fn simple_drop_receiver(){
        let (send, recv) = create_pipe_chan().unwrap();
        assert_eq!(recv.read_byte(), Ok(None));
        send.send_byte(5).unwrap();
        drop(recv);
        assert_eq!(send.send_byte(27), Err(SendError::ReceiverHasHungUp));
    }

    #[test]
    fn overfill_sender(){
        let (send, recv) = create_pipe_chan().unwrap();
        assert_eq!(recv.read_byte(), Ok(None));

        let capacity = unsafe {fcntl(send.handle.get_raw(), libc::F_GETPIPE_SZ)};
        for _ in 0..capacity {
            assert!(send.send_byte(3).is_ok());
        }
        assert_eq!(send.send_byte(3), Err(SendError::ChannelFullWouldBlock));

    }
}