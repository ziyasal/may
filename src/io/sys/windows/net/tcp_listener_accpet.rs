use std;
use std::io;
use std::net::SocketAddr;
use std::os::windows::io::AsRawSocket;
use super::super::winapi::*;
use super::super::EventData;
use super::super::co_io_result;
use super::super::miow::net::{TcpListenerExt, AcceptAddrsBuf};
use cancel::Cancel;
use net2::TcpBuilder;
use scheduler::get_scheduler;
use net::{TcpStream, TcpListener};
use io::cancel::{CancelIoData, CancelIoImpl};
use coroutine::{CoroutineImpl, EventSource, get_cancel_data};

pub struct TcpListenerAccept<'a> {
    io_data: EventData,
    socket: &'a std::net::TcpListener,
    builder: TcpBuilder,
    ret: Option<std::net::TcpStream>,
    addr: AcceptAddrsBuf,
    io_cancel: &'static Cancel<CancelIoImpl>,
}

impl<'a> TcpListenerAccept<'a> {
    pub fn new(socket: &'a TcpListener) -> io::Result<Self> {
        let addr = try!(socket.local_addr());
        let builder = match addr {
            SocketAddr::V4(..) => try!(TcpBuilder::new_v4()),
            SocketAddr::V6(..) => try!(TcpBuilder::new_v6()),
        };

        Ok(TcpListenerAccept {
            io_data: EventData::new(socket.as_raw_socket() as HANDLE),
            socket: socket.inner(),
            builder: builder,
            ret: None,
            addr: AcceptAddrsBuf::new(),
            io_cancel: get_cancel_data(),
        })
    }

    #[inline]
    pub fn done(self) -> io::Result<(TcpStream, SocketAddr)> {
        try!(co_io_result(&self.io_data));
        let socket = &self.socket;
        let s = try!(self.ret
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "tcp listener ret is not set"))
            .and_then(|s| socket.accept_complete(&s).and_then(|_| TcpStream::new(s))));

        let addr = try!(self.addr.parse(&self.socket).and_then(|a| {
            a.remote().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "could not obtain remote address")
            })
        }));

        Ok((s, addr))
    }
}

impl<'a> EventSource for TcpListenerAccept<'a> {
    fn get_cancel_data(&self) -> Option<&Cancel<CancelIoImpl>> {
        Some(self.io_cancel)
    }

    fn subscribe(&mut self, co: CoroutineImpl) {
        let s = get_scheduler();
        // we don't need to register the timeout here,
        // prepare the co first
        self.io_data.co = Some(co);

        // call the overlapped read API
        let (s, _) = co_try!(s, self.io_data.co.take().expect("can't get co"), unsafe {
            self.socket
                .accept_overlapped(&self.builder, &mut self.addr, self.io_data.get_overlapped())
        });

        self.ret = Some(s);

        // deal with the cancel
        self.get_cancel_data().map(|cancel| {
            // register the cancel io data
            cancel.set_io(CancelIoData::new(&self.io_data));
            // re-check the cancel status
            if cancel.is_canceled() {
                unsafe { cancel.cancel() };
            }
        });
    }
}
