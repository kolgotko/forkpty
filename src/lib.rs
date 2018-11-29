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
pub use nix::sys::wait::WaitStatus;

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

#[derive(Debug, Clone)]
pub struct PtyReader(i32);

impl io::Read for PtyReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {

        read(self.0, buf).map_err(|error| {
            let kind = io::ErrorKind::Other;
            io::Error::new(kind, error)
        })

    }
}

impl AsRawFd for PtyReader {
    fn as_raw_fd(&self) -> i32 {
        self.0
    }
}

impl IsAlive for PtyReader {}

#[derive(Debug, Clone)]
pub struct PtyWriter(i32);

impl io::Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(self.0, buf).map_err(|error| {
            let kind = io::ErrorKind::Other;
            io::Error::new(kind, error)
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRawFd for PtyWriter {
    fn as_raw_fd(&self) -> i32 {
        self.0
    }
}

impl IsAlive for PtyWriter {}

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
            Some(PtyReader(self.as_raw_fd()))
        } else {
            None
        }

    }

    pub fn get_writer(&self) -> Option<PtyWriter> {

        if self.is_alive() {
            Some(PtyWriter(self.as_raw_fd()))
        } else {
            None
        }

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

impl Clone for PtyMaster {
    fn clone(&self) -> PtyMaster {
        let new_fd = dup(self.0).unwrap();
        PtyMaster(new_fd)
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

