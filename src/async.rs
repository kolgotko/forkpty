use std::io;
use std::os::unix::io::{ AsRawFd, IntoRawFd };
use std::mem;
use std::{task::{Waker, Context}, fs::OpenOptions};
use nix::unistd::*;
use nix::pty::*;
pub use nix::pty::Winsize;
use nix::pty::PtyMaster as NixPtyMaster;
use nix::fcntl::{ OFlag, fcntl, FcntlArg };
use nix::sys::wait::*;
use nix::poll::*;
pub use nix::Error as NixError;
pub use nix::errno::Errno as NixErrno;
pub use nix::sys::wait::WaitStatus;
pub use nix::sys::wait::WaitPidFlag;
use thiserror::Error as ThisError;

use tokio::io::{ AsyncRead, Result as IoResult };
use core::pin::Pin;
use core::task::Poll;

#[derive(ThisError, Debug)]
pub enum ForkPtyErr {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    NixError(#[from] NixError),
}

pub enum ForkPtyValue {
    Parent(Child, PtyMaster),
    Child(Pid),
}

pub trait PtyResize {
    fn resize(&self, winsize: Winsize) -> Result<(), io::Error>;
}

pub trait IsAlive: AsRawFd {
    fn is_alive(&self) -> bool {
        let fd = self.as_raw_fd();
        let result = fcntl(fd, FcntlArg::F_GETFD);

        match result {
            Ok(_) => true,
            _ => false,
        }
    }
}

pub trait SetNonblocking: AsRawFd {
    fn set_nonblocking(&mut self, value: bool) -> io::Result<()> {
        let fd = self.as_raw_fd();
        let saved = fcntl(fd, FcntlArg::F_GETFL)
            .map_err(|error| {
                let kind = io::ErrorKind::Other;

                io::Error::new(kind, error)
            })?;

        let mut o_flag = unsafe { OFlag::from_bits_unchecked(saved) };
        o_flag.set(OFlag::O_NONBLOCK, value);

        fcntl(fd, FcntlArg::F_SETFL(o_flag))
            .map_err(|error| {
                let kind = io::ErrorKind::Other;

                io::Error::new(kind, error)
            })?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct PtyMaster(i32);

impl PtyResize for PtyMaster {
    fn resize(&self, winsize: Winsize) -> Result<(), io::Error> {
        let result = unsafe { libc::ioctl(self.0, libc::TIOCSWINSZ, &winsize) };

        if result != -1 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl IsAlive for PtyMaster {}

use tokio::io::PollEvented;
use mio::unix::EventedFd;

impl PtyMaster {
    pub fn get_reader(&self) -> Option<PtyReader> {
        if self.is_alive() {
            Some(PtyReader{ fd: self.as_raw_fd(), timeout: -1 })
        } else {
            None
        }
    }

    pub fn get_writer(&self) -> Option<PtyWriter> {
        if self.is_alive() {
            Some(PtyWriter{ fd: self.as_raw_fd(), timeout: -1 })
        } else {
            None
        }
    }

    pub fn try_clone(&self) -> Result<PtyMaster, NixError> {
        let new_fd = dup(self.0)?;

        Ok(PtyMaster(new_fd))
    }

    pub fn get_async_reader(&self) -> Option<AsyncReader> {
        if self.is_alive() {
            Some(AsyncReader::new(&self.0))
        } else {
            None
        }
    }

    pub fn get_async_writer(&self) -> Option<AsyncWriter> {
        if self.is_alive() {
            Some(AsyncWriter::new(&self.0))
        } else {
            None
        }
    }
}

pub struct AsyncWriter<'a> {
    evented: PollEvented<EventedFd<'a>>,
    fd: i32,
}

impl <'a>AsRawFd for AsyncWriter<'a> {
    fn as_raw_fd(&self) -> i32 {
        self.fd
    }
}

impl <'a>SetNonblocking for AsyncWriter<'a> {}

impl <'a>AsyncWriter<'a> {
    fn new(fd: &'a i32) -> Self {
        let mut result = AsyncWriter {
            evented: PollEvented::new(EventedFd(fd)).unwrap(),
            fd: *fd,
        };
        result.set_nonblocking(true).unwrap();
        result
    }
}

impl <'a>tokio::io::AsyncWrite for AsyncWriter<'a> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8]
    ) -> Poll<Result<usize, tokio::io::Error>> {
        use futures::ready;

        ready!(self.evented.poll_write_ready(cx))?;

        match write(self.fd, buf) {
            Ok(length) => Poll::Ready(Ok(length)),
            Err(NixError::Sys(NixErrno::EAGAIN)) => {
                self.evented.clear_write_ready(cx)?;
                Poll::Pending
            },
            Err(error) => {
                let kind = tokio::io::ErrorKind::Other;

                Poll::Ready(Err(tokio::io::Error::new(kind, error)))
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


pub struct AsyncReader<'a> {
    evented: PollEvented<EventedFd<'a>>,
    fd: i32,
}

impl <'a>AsRawFd for AsyncReader<'a> {
    fn as_raw_fd(&self) -> i32 {
        self.fd
    }
}

impl <'a>SetNonblocking for AsyncReader<'a> {}

impl <'a>AsyncReader<'a> {
    fn new(fd: &'a i32) -> Self {
        let mut result = AsyncReader {
            evented: PollEvented::new(EventedFd(fd)).unwrap(),
            fd: *fd,
        };
        result.set_nonblocking(true).unwrap();
        result
    }
}

impl <'a>tokio::io::AsyncRead for AsyncReader<'a> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8]
    ) -> Poll<IoResult<usize>> {
        use mio::Ready;
        use futures::ready;

        ready!(self.evented.poll_read_ready(cx, Ready::readable()))?;

        match read(self.fd, buf) {
            Ok(length) => Poll::Ready(Ok(length)),
            Err(NixError::Sys(NixErrno::EAGAIN)) => {
                self.evented.clear_read_ready(cx, Ready::readable())?;
                Poll::Pending
            },
            Err(error) => {
                let kind = tokio::io::ErrorKind::Other;

                Poll::Ready(Err(tokio::io::Error::new(kind, error)))
            },
        }
    }
}

impl Drop for PtyMaster {
    fn drop(&mut self) {
        let err = close(self.0);

        if err == Err(NixError::Sys(nix::errno::Errno::EBADF)) {
            panic!("Closing an invalid file descriptor!");
        };
    }
}

impl From<NixPtyMaster> for PtyMaster {
    fn from(pty_master: NixPtyMaster) -> PtyMaster {
        PtyMaster(pty_master.into_raw_fd())
    }
}

impl AsRawFd for PtyMaster {
    fn as_raw_fd(&self) -> i32 {
        self.0
    }
}

impl IntoRawFd for PtyMaster {
    fn into_raw_fd(self) -> i32 {
        let fd = self.0;
        mem::forget(self);

        fd
    }
}


#[derive(Debug, Copy, Clone)]
pub struct Child(Pid);

impl Child {
    pub async fn status(&self) -> nix::Result<WaitStatus> {
        WaitPidFuture::new(self.0).await
    }
}

pub fn forkpty() -> Result<ForkPtyValue, ForkPtyErr> {
    let pty_master = posix_openpt(OFlag::O_RDWR)?;

    grantpt(&pty_master)?;
    unlockpt(&pty_master)?;

    let slave_name = unsafe { ptsname(&pty_master) }?;
    let slave_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&slave_name)?;
    let slave_fd = slave_file.as_raw_fd();

    match fork() {
        Ok(ForkResult::Parent { child, .. }) => {
            close(slave_fd)?;

            let child = Child(child);
            let fork_pty_master: PtyMaster = pty_master.into();

            Ok(ForkPtyValue::Parent(child, fork_pty_master))
        },
        Ok(ForkResult::Child) => {
            let stdin = io::stdin();
            let stdout = io::stdout();
            let stderr = io::stderr();

            close(stdin.as_raw_fd())?;
            close(stdout.as_raw_fd())?;
            close(stderr.as_raw_fd())?;
            dup(slave_fd)?;
            dup(slave_fd)?;
            dup(slave_fd)?;

            let pid = setsid()?;

            unsafe {
                libc::ioctl(0, libc::TIOCSCTTY.into(), 1);
            }

            Ok(ForkPtyValue::Child(pid))
        },
        Err(error) => Err(error.into()),
    }
}
