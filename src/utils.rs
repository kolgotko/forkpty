use std::os::unix::io::RawFd;
use std::io;
use nix::fcntl::{ OFlag, fcntl, FcntlArg };

#[allow(dead_code)]
pub(crate) fn set_nonblocking(fd: RawFd, value: bool) -> io::Result<()> {
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

#[allow(dead_code)]
pub(crate) fn is_alive(fd: RawFd) -> bool {
    let result = fcntl(fd, FcntlArg::F_GETFD);

    match result {
        Ok(_) => true,
        _ => false,
    }
}
