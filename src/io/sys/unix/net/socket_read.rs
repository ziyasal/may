use std::{self, io};
use std::io::Read;
use std::time::Duration;
use std::os::unix::io::RawFd;
use super::co_io_result;
use super::super::from_nix_error;
use super::super::nix::unistd::read;
use super::super::{EventData, FLAG_READ};
use scheduler::get_scheduler;
use coroutine::{CoroutineImpl, EventSource};


pub struct SocketRead<'a> {
    io_data: EventData,
    buf: &'a mut [u8],
    timeout: Option<Duration>,
}

impl<'a> SocketRead<'a> {
    pub fn new(socket: RawFd, buf: &'a mut [u8], timeout: Option<Duration>) -> Self {
        SocketRead {
            io_data: EventData::new(socket, FLAG_READ),
            buf: buf,
            timeout: timeout,
        }
    }

    #[inline]
    pub fn done(self) -> io::Result<usize> {
        try!(co_io_result());
        // finish the read operaion
        read(self.io_data.fd, self.buf).map_err(from_nix_error);
    }
}

impl<'a> EventSource for SocketRead<'a> {
    fn subscribe(&mut self, co: CoroutineImpl) {
        let s = get_scheduler();
        // prepare the co first
        self.io_data.co = Some(co);

        // register the io operaton
        co_try!(s,
                self.io_data.co.take().unwrap(),
                s.add_io(&mut self.io_data, self.timeout));
    }
}