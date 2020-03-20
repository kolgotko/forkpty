use std::io;
use std::os::unix::io::{ AsRawFd, IntoRawFd };
use std::mem;
use std::fs::OpenOptions;
use nix::unistd::*;
use nix::pty::*;
pub use nix::pty::Winsize;
use nix::pty::PtyMaster as NixPtyMaster;
use nix::fcntl::{ OFlag, fcntl, FcntlArg };
use nix::sys::wait::*;
use nix::poll::*;
pub use nix::Error as NixError;
pub use nix::sys::wait::WaitStatus;
pub use nix::sys::wait::WaitPidFlag;
use thiserror::Error as ThisError;

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
        let poll_fd = PollFd::new(self.fd, PollFlags::POLLIN);

        match poll(&mut [poll_fd], self.timeout) {
            Ok(0) => {
                let kind = io::ErrorKind::TimedOut;

                Err(io::Error::from(kind))
            },
            Ok(-1) => {
                Err(io::Error::last_os_error())
            },
            Ok(_) => {
                read(self.fd, buf).map_err(|error| {
                    let kind = io::ErrorKind::Other;

                    io::Error::new(kind, error)
                })
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
        let poll_fd = PollFd::new(self.fd, PollFlags::POLLOUT);

        match poll(&mut [poll_fd], self.timeout) {
            Ok(0) => {
                let kind = io::ErrorKind::TimedOut;

                Err(io::Error::from(kind))
            },
            Ok(-1) => {
                Err(io::Error::last_os_error())
            },
            Ok(_) => {
                write(self.fd, buf).map_err(|error| {
                    let kind = io::ErrorKind::Other;

                    io::Error::new(kind, error)
                })
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

#[derive(Debug, Copy, Clone)]
pub struct Child(Pid);

impl Child {
    pub fn wait(&self, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus> {
        waitpid(self.0, options)
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
