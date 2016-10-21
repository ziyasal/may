use std::{self, io};
use std::time::Duration;
use std::net::ToSocketAddrs;
use std::os::unix::io::AsRawFd;
use net::UdpSocket;
use yield_now::yield_with;
use scheduler::get_scheduler;
use coroutine::{CoroutineImpl, EventSource};
use super::super::{EventData, FLAG_WRITE, co_io_result};

pub struct UdpSendTo<'a, A: ToSocketAddrs> {
    io_data: EventData,
    buf: &'a [u8],
    socket: &'a std::net::UdpSocket,
    addr: A,
    timeout: Option<Duration>,
}

impl<'a, A: ToSocketAddrs> UdpSendTo<'a, A> {
    pub fn new(socket: &'a UdpSocket, buf: &'a [u8], addr: A) -> io::Result<Self> {
        Ok(UdpSendTo {
            io_data: EventData::new(socket.as_raw_fd(), FLAG_WRITE),
            buf: buf,
            socket: socket.inner(),
            addr: addr,
            timeout: socket.write_timeout().unwrap(),
        })
    }

    #[inline]
    pub fn done(self) -> io::Result<usize> {
        let s = get_scheduler().get_selector();
        loop {
            s.del_fd(self.io_data.fd);
            try!(co_io_result());

            match self.socket.send_to(self.buf, &self.addr) {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                ret @ _ => return ret,
            }

            // the result is still WouldBlock, need to try again
            yield_with(&self);
        }
    }
}

impl<'a, A: ToSocketAddrs> EventSource for UdpSendTo<'a, A> {
    fn subscribe(&mut self, co: CoroutineImpl) {
        let s = get_scheduler();
        let selector = s.get_selector();
        selector.add_io_timer(&mut self.io_data, self.timeout);
        self.io_data.co = Some(co);

        // register the io operaton
        co_try!(s,
                self.io_data.co.take().expect("can't get co"),
                selector.add_io(&self.io_data));
    }
}
