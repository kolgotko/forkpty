use nix::pty::PtyMaster as NixPtyMaster;
use nix::errno::Errno as NixErrno;
use nix::Error as NixError;
use nix::unistd::{ read, write, dup, close };
use std::io;
use std::mem;
use std::os::unix::io::{ AsRawFd, IntoRawFd };
use crate::common::PtyResize;

#[derive(Debug)]
pub struct PtyMaster(pub(crate) i32);

impl PtyMaster {
    pub fn try_clone(&self) -> Result<PtyMaster, NixError> {
        let new_fd = dup(self.0)?;

        Ok(PtyMaster(new_fd))
    }
}

impl PtyResize for PtyMaster { }

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

impl io::Read for PtyMaster {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        read(self.0, buf).map_err(|_| {
            io::Error::last_os_error()
        })
    }
}

impl io::Write for PtyMaster {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(self.0, buf).map_err(|_| {
            io::Error::last_os_error()
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PtyMaster {
    fn drop(&mut self) {
        let err = close(self.0);

        if err == Err(NixError::Sys(NixErrno::EBADF)) {
            panic!("Closing an invalid file descriptor!");
        };
    }
}
