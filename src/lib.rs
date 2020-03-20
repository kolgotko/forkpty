pub use nix::unistd::Pid;
pub use nix::pty::Winsize;
pub use nix::Error as NixError;
pub use nix::errno::Errno as NixErrno;
pub use nix::sys::wait::WaitStatus;

mod common;
mod pty_master;
mod child;
mod async_pty_master;
mod async_child;
mod utils;

mod forkpty_common;
use forkpty_common::forkpty_common;
pub use common::{ PtyResize, ForkPtyErr, ForkPtyValue, AsyncForkPtyValue };
pub use pty_master::PtyMaster;
pub use async_pty_master::AsyncPtyMaster;
pub use child::Child;
pub use async_child::AsyncChild;

pub fn forkpty() -> Result<ForkPtyValue, ForkPtyErr> {
    match forkpty_common() {
        Ok(value) => Ok(value.into()),
        Err(error) => Err(error),
    }
}

pub fn forkpty_async() -> Result<AsyncForkPtyValue, ForkPtyErr> {
    match forkpty_common() {
        Ok(value) => Ok(value.into()),
        Err(error) => Err(error),
    }
}
