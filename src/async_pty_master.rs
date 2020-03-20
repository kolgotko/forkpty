use nix::pty::PtyMaster as NixPtyMaster;
use nix::errno::Errno as NixErrno;
use nix::Error as NixError;
use nix::unistd::{ read, write, dup, close };
use tokio::io::{
    PollEvented,
    AsyncRead,
    AsyncWrite,
    Result as TokioIoResult,
};
use mio::unix::EventedFd;
use mio::event::Evented;
use futures::ready;
use std::io;
use std::{
    pin::Pin,
    os::unix::io::{ AsRawFd, IntoRawFd, RawFd },
    task::{ Poll, Context },
};
use crate::common::PtyResize;
use crate::utils::set_nonblocking;

#[derive(Debug)]
struct PtyEvented(RawFd);

impl Evented for PtyEvented {
    fn register(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.0).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.0).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        EventedFd(&self.0).deregister(poll)
    }
}

#[derive(Debug)]
pub struct AsyncPtyMaster {
    evented: PollEvented<PtyEvented>,
    fd: i32,
}

impl AsyncPtyMaster {
    fn new(fd: i32) -> Self {
        let result = AsyncPtyMaster {
            evented: PollEvented::new(PtyEvented(fd)).unwrap(),
            fd,
        };

        set_nonblocking(fd, true).unwrap();
        result
    }

    pub fn try_clone(&self) -> Result<AsyncPtyMaster, NixError> {
        let new_fd = dup(self.fd)?;

        Ok(AsyncPtyMaster::new(new_fd))
    }
}

impl PtyResize for AsyncPtyMaster { }

impl AsRawFd for AsyncPtyMaster {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl From<NixPtyMaster> for AsyncPtyMaster {
    fn from(pty_master: NixPtyMaster) -> AsyncPtyMaster {
        AsyncPtyMaster::new(pty_master.into_raw_fd())
    }
}

impl Drop for AsyncPtyMaster {
    fn drop(&mut self) {
        let err = close(self.fd);

        if err == Err(NixError::Sys(NixErrno::EBADF)) {
            panic!("Closing an invalid file descriptor!");
        };
    }
}

impl AsyncWrite for AsyncPtyMaster {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<TokioIoResult<usize>> {
        use tokio::io::Error;
        use tokio::io::ErrorKind;

        ready!(self.evented.poll_write_ready(cx))?;

        match write(self.fd, buf) {
            Ok(length) => Poll::Ready(Ok(length)),
            Err(NixError::Sys(NixErrno::EAGAIN)) => {
                self.evented.clear_write_ready(cx)?;
                Poll::Pending
            },
            Err(error) => {
                let kind = ErrorKind::Other;

                Poll::Ready(Err(Error::new(kind, error)))
            },
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncRead for AsyncPtyMaster {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<TokioIoResult<usize>> {
        use tokio::io::Error;
        use tokio::io::ErrorKind;

        ready!(self.evented.poll_read_ready(cx, mio::Ready::readable()))?;

        match read(self.fd, buf) {
            Ok(length) => Poll::Ready(Ok(length)),
            Err(NixError::Sys(NixErrno::EAGAIN)) => {
                self.evented.clear_read_ready(cx, mio::Ready::readable())?;
                Poll::Pending
            },
            Err(error) => {
                let kind = ErrorKind::Other;

                Poll::Ready(Err(Error::new(kind, error)))
            },
        }
    }
}
