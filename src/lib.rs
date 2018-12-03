extern crate nix;
extern crate libc;

use std::io;
use std::fmt;
use std::os::unix::io::{ AsRawFd, IntoRawFd };
use std::error::Error;
use std::mem;
use std::fs::OpenOptions;
use nix::unistd::*;
use nix::pty::*;
pub use nix::pty::Winsize;
use nix::pty::PtyMaster as NixPtyMaster;
use nix::fcntl::{OFlag, open, fcntl, FcntlArg};
use nix::sys::wait::*;
use nix::poll::*;
use nix::Error as NixError;
use nix::errno::Errno as NixErrno;
pub use nix::sys::wait::WaitStatus;


#[derive(Debug)]
pub enum CloneError {
    EBADF,
    EMFILE,
    Unsupported(NixError),
}

impl fmt::Display for CloneError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        match self {
            CloneError::EBADF => {
                write!(f, "[EBADF] The oldd argument is not a valid active descriptor")
            },
            CloneError::EMFILE => {
                write!(f, "[EMFILE] Too many descriptors are active")
            },
            CloneError::Unsupported(error) => error.fmt(f)
        }

    }
}

impl Error for CloneError {}

impl From<NixError> for CloneError {
    fn from(value: NixError) -> CloneError {
        match value {
            NixError::Sys(NixErrno::EBADF) => CloneError::EBADF,
            NixError::Sys(NixErrno::EMFILE) => CloneError::EMFILE,
            nix_error @ _ => CloneError::Unsupported(nix_error),
        }
    }
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

        let mut o_flag = OFlag::from_bits(saved)
            .ok_or_else(|| {
                let kind = io::ErrorKind::Other;
                io::Error::new(kind, "incorrect bits for OFlag")
            })?;

        o_flag.set(OFlag::O_NONBLOCK, value);

        fcntl(fd, FcntlArg::F_SETFL(o_flag))
            .map_err(|error| {
                let kind = io::ErrorKind::Other;
                io::Error::new(kind, error)
            })?;

        Ok(())

    }

}

#[derive(Debug, Clone)]
pub struct PtyReader { fd: i32, timeout: i32 }

impl PtyReader {

    pub fn set_timeout(&mut self, value: i32) -> io::Result<()> {
        self.timeout = value;
        Ok(())
    }

    pub fn get_timeout(&self) -> i32 {
        self.timeout
    }

}

impl io::Read for PtyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {

        let poll_fd = PollFd::new(self.fd, EventFlags::POLLIN);

        match poll(&mut [poll_fd], self.timeout) {

            Ok(0) => {

                let kind = io::ErrorKind::TimedOut;
                Err(io::Error::from(kind))

            },
            Ok(_) => {

                read(self.fd, buf).map_err(|error| {
                    let kind = io::ErrorKind::Other;
                    io::Error::new(kind, error)
                })

            },
            Ok(-1) => {

                Err(io::Error::last_os_error())

            },
            Err(error) => {

                let kind = io::ErrorKind::Other;
                Err(io::Error::new(kind, error))

            }

        }

    }
}

impl AsRawFd for PtyReader {
    fn as_raw_fd(&self) -> i32 {
        self.fd
    }
}

impl IsAlive for PtyReader {}
impl SetNonblocking for PtyReader {}

#[derive(Debug, Clone)]
pub struct PtyWriter{ fd: i32, timeout: i32 }

impl PtyWriter {

    pub fn set_timeout(&mut self, value: i32) -> io::Result<()> {
        self.timeout = value;
        Ok(())
    }

    pub fn get_timeout(&self) -> i32 {
        self.timeout
    }

}

impl io::Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {

        let poll_fd = PollFd::new(self.fd, EventFlags::POLLOUT);

        match poll(&mut [poll_fd], self.timeout) {

            Ok(0) => {

                let kind = io::ErrorKind::TimedOut;
                Err(io::Error::from(kind))

            },
            Ok(_) => {

                write(self.fd, buf).map_err(|error| {
                    let kind = io::ErrorKind::Other;
                    io::Error::new(kind, error)
                })

            },
            Ok(-1) => {

                Err(io::Error::last_os_error())

            },
            Err(error) => {

                let kind = io::ErrorKind::Other;
                Err(io::Error::new(kind, error))

            }

        }

    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRawFd for PtyWriter {
    fn as_raw_fd(&self) -> i32 {
        self.fd
    }
}

impl IsAlive for PtyWriter {}
impl SetNonblocking for PtyWriter {}

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

    pub fn try_clone(&self) -> Result<PtyMaster, CloneError> {
        let new_fd = dup(self.0)?;
        Ok(PtyMaster(new_fd))
    }

}

impl Drop for PtyMaster {
    fn drop(&mut self) {

        let err = close(self.0);

        if err == Err(nix::Error::Sys(nix::errno::Errno::EBADF)) {
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

#[derive(Copy, Clone)]
pub struct Child(Pid);

impl Child {
    pub fn wait(&self) -> nix::Result<WaitStatus> {

        waitpid(self.0, None)

    }
}

pub enum ForkPtyResult {

    Parent(Child, PtyMaster),
    Child(Pid),

}

pub fn forkpty() -> Result<ForkPtyResult, Box<Error>> {

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

            close(slave_fd);

            let child = Child(child);
            let fork_pty_master: PtyMaster = pty_master.into();

            Ok(ForkPtyResult::Parent(child, fork_pty_master))

        },
        Ok(ForkResult::Child) => {

            let stdin = io::stdin();
            let stdout = io::stdout();
            let stderr = io::stderr();

            close(stdin.as_raw_fd());
            close(stdout.as_raw_fd());
            close(stderr.as_raw_fd());

            dup(slave_fd)?;
            dup(slave_fd)?;
            dup(slave_fd)?;

            let pid = setsid()?;

            unsafe {
                libc::ioctl(0, libc::TIOCSCTTY.into(), 1);
            }

            Ok(ForkPtyResult::Child(pid))
        },
        Err(_) => { Err("Fork failed")? },
    }

}

