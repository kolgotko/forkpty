use std::io;
use std::os::unix::io::AsRawFd;
use nix::unistd::Pid;
use nix::pty::Winsize;
use nix::Error as NixError;
use thiserror::Error as ThisError;

use crate::Child;
use crate::AsyncChild;
use crate::pty_master::PtyMaster;
use crate::async_pty_master::AsyncPtyMaster;
use crate::forkpty_common::CommonForkPtyValue;

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

impl From<CommonForkPtyValue> for ForkPtyValue {
    fn from(value: CommonForkPtyValue) -> Self {
        match value {
            CommonForkPtyValue::Parent(pid, pty_master) => {
                Self::Parent(pid.into(), pty_master.into())
            },
            CommonForkPtyValue::Child(pid) => Self::Child(pid),
        }
    }
}

pub enum AsyncForkPtyValue {
    Parent(AsyncChild, AsyncPtyMaster),
    Child(Pid),
}

impl From<CommonForkPtyValue> for AsyncForkPtyValue {
    fn from(value: CommonForkPtyValue) -> Self {
        match value {
            CommonForkPtyValue::Parent(pid, pty_master) => {
                Self::Parent(pid.into(), pty_master.into())
            },
            CommonForkPtyValue::Child(pid) => Self::Child(pid),
        }
    }
}

pub trait PtyResize: AsRawFd {
    fn resize(&self, winsize: Winsize) -> Result<(), io::Error> {
        let result = unsafe { libc::ioctl(self.as_raw_fd(), libc::TIOCSWINSZ, &winsize) };

        if result != -1 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
