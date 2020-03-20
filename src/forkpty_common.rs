use std::io;
use std::os::unix::io::AsRawFd;
use std::fs::OpenOptions;
use nix::unistd::{ fork, ForkResult, close, dup, setsid };
use nix::pty::{ posix_openpt, grantpt, unlockpt, ptsname };
use nix::fcntl::OFlag;
use nix::unistd::Pid;
use nix::pty::PtyMaster as NixPtyMaster;
use crate::ForkPtyErr;

pub(crate) enum CommonForkPtyValue {
    Parent(Pid, NixPtyMaster),
    Child(Pid)
}

pub(crate) fn forkpty_common() -> Result<CommonForkPtyValue, ForkPtyErr> {
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

            Ok(CommonForkPtyValue::Parent(child, pty_master))
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
                libc::ioctl(0, libc::TIOCSCTTY, 1);
            }

            Ok(CommonForkPtyValue::Child(pid))
        },
        Err(error) => Err(error.into()),
    }
}
