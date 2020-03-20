pub use nix::unistd::Pid;
pub use nix::pty::Winsize;
pub use nix::Error as NixError;
pub use nix::errno::Errno as NixErrno;
pub use nix::sys::wait::WaitStatus;

mod common;
mod pty_master;
mod child;
#[cfg(feature = "async")]
mod async_pty_master;
#[cfg(feature = "async")]
mod async_child;
mod utils;

mod forkpty_common;
use forkpty_common::forkpty_common;
pub use common::{ PtyResize, ForkPtyErr, ForkPtyValue };
#[cfg(feature = "async")]
pub use common::AsyncForkPtyValue;
pub use pty_master::PtyMaster;
pub use child::Child;
#[cfg(feature = "async")]
pub use async_pty_master::AsyncPtyMaster;
#[cfg(feature = "async")]
pub use async_child::AsyncChild;

pub fn forkpty() -> Result<ForkPtyValue, ForkPtyErr> {
    match forkpty_common() {
        Ok(value) => Ok(value.into()),
        Err(error) => Err(error),
    }
}

#[cfg(feature = "async")]
pub fn forkpty_async() -> Result<AsyncForkPtyValue, ForkPtyErr> {
    match forkpty_common() {
        Ok(value) => Ok(value.into()),
        Err(error) => Err(error),
    }
}
