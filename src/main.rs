extern crate nix;
extern crate libc;

use nix::unistd::*;
use nix::pty::*;
use nix::fcntl::{OFlag, open};
use std::process::Command;
use std::ffi::CString;
use nix::sys::wait::*;
use std::io;
use std::io::{Read, Write};
use std::os::unix::io::AsRawFd;
use std::fs::OpenOptions;
use std::thread;
use std::os::unix::net::{UnixStream, UnixListener};
use std::error::Error;
use std::fs::create_dir_all;

// #[derive(Copy, Clone)]
struct Pty {
    pty_master: PtyMaster,
}

impl Pty {

    fn read(&self, buffer: &mut Vec<u8>) -> nix::Result<usize> {

        read(self.pty_master.as_raw_fd(), buffer)

    }

    fn write(&mut self, bytes: &[u8]) -> nix::Result<usize> {

        write(self.pty_master.as_raw_fd(), bytes)

    }

    fn resize(&self, winsize: Winsize) {

        unsafe {
            libc::ioctl(self.pty_master.as_raw_fd(), libc::TIOCSWINSZ, &winsize);
        }

    }
}

#[derive(Copy, Clone)]
struct Child {
    pid: Pid,
}

impl Child {
    fn wait(&self) -> nix::Result<WaitStatus> {

        waitpid(self.pid, None)

    }
}

enum ForkPtyResult {

    Parent(Child, Pty),
    Child(Pid),

}

fn forkpty() -> Result<ForkPtyResult, Box<Error>> {

    let pty_master = posix_openpt(OFlag::O_RDWR)?;

    grantpt(&pty_master)?;
    unlockpt(&pty_master)?;

    let slave_name = unsafe { ptsname(&pty_master) }?;
    let master_fd = pty_master.as_raw_fd();

    println!("{:?}", slave_name);

    let slave_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&slave_name)?;

    let slave_fd = slave_file.as_raw_fd();

    match fork() {
        Ok(ForkResult::Parent { child, .. }) => {

            close(slave_fd);

            let child = Child { pid: child };
            let pty_master = Pty { pty_master: pty_master };

            Ok(ForkPtyResult::Parent(child, pty_master))

        },
        Ok(ForkResult::Child) => {

            let stdin = io::stdin();
            let stdout = io::stdout();
            let stderr = io::stderr();

            close(master_fd);
            close(stdin.as_raw_fd());
            close(stdout.as_raw_fd());
            close(stderr.as_raw_fd());

            dup(slave_fd);
            dup(slave_fd);
            dup(slave_fd);

            close(slave_fd);

            let pid = setsid()?;

            unsafe {
                libc::ioctl(0, libc::TIOCSCTTY.into(), 1);
            }

            let command = CString::new("sh").unwrap();
            execvp(&command, &[
                   CString::new("").unwrap(),
            ]);
            Ok(ForkPtyResult::Child(pid))
        },
        Err(_) => { Err("Fork failed")? },
    }

}

fn main() -> Result<(), Box<Error>> {

    let fork_result = forkpty();

    match fork_result {

        Ok(ForkPtyResult::Parent(child, mut pty_master)) => {

            // let winsize: Winsize = Winsize {
            //     ws_row: 34,
            //     ws_col: 125,
            //     ws_xpixel: 0,
            //     ws_ypixel: 0,
            // };

            // pty_master.resize(winsize);

            // let out_listener = UnixListener::bind("/tmp/new_process_out.sock").unwrap();
            // let in_listener = UnixListener::bind("/tmp/new_process_in.sock").unwrap();

            // let (mut out_stream, _) = out_listener.accept().unwrap();

            // thread::spawn(move || {

            //     let mut buffer: Vec<u8> = vec![0; libc::BUFSIZ as usize];

            //     loop {

            //         let count = pty_master.read(&mut buffer).unwrap();
            //         // let count = read(master_fd, &mut buffer).unwrap();
            //         out_stream.write(&buffer[0..count]).unwrap();
            //         out_stream.flush().unwrap();

            //     }

            // });

            // let (mut in_stream, _) = in_listener.accept().unwrap();

            // thread::spawn(move || {

            //     for byte in in_stream.bytes() {
            //         pty_master.write(&[byte.unwrap()]);
            //         // write(master_fd, &[byte.unwrap()]);
            //     }

            // });

            let result = waitpid(child.pid, None);
            println!("{:?}", result);

        },
        Ok(ForkPtyResult::Child(_)) => {

            create_dir_all("/tmp/i_am_a_live");

            let command = CString::new("sh").unwrap();
            execvp(&command, &[
                   CString::new("").unwrap(),
            ]);

        },

        _ => {

        },

    }

    Ok(())

}

fn _main() {

    let master_fd = posix_openpt(OFlag::O_RDWR).unwrap();
    grantpt(&master_fd).unwrap();
    unlockpt(&master_fd).unwrap();
    let slave_name = unsafe { ptsname(&master_fd) }.unwrap();
    let master_fd = master_fd.as_raw_fd();

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&slave_name)
        .unwrap();

    let slave_fd = file.as_raw_fd();

    println!("dev: {}", slave_name);

    unsafe {
        let winsize: Winsize = Winsize {
            ws_row: 34,
            ws_col: 125,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        libc::ioctl(master_fd, libc::TIOCSWINSZ, &winsize);
    }

    let out_listener = UnixListener::bind("/tmp/new_process_out.sock").unwrap();
    let in_listener = UnixListener::bind("/tmp/new_process_in.sock").unwrap();

    let (mut out_stream, _) = out_listener.accept().unwrap();

    thread::spawn(move || {

        let mut buffer: Vec<u8> = vec![0; libc::BUFSIZ as usize];

        loop {

            let count = read(master_fd, &mut buffer).unwrap();
            out_stream.write(&buffer[0..count]).unwrap();
            out_stream.flush().unwrap();

        }

    });

    let (mut in_stream, _) = in_listener.accept().unwrap();

    thread::spawn(move || {

        for byte in in_stream.bytes() {
            write(master_fd, &[byte.unwrap()]);
        }

    });

    match fork() {
        Ok(ForkResult::Parent { child, .. }) => {

            println!("child: {:?}", child);
            close(slave_fd);
            waitpid(child, None);

        },
        Ok(ForkResult::Child) => {

            let stdin = io::stdin();
            let stdout = io::stdout();
            let stderr = io::stderr();

            close(master_fd);
            close(stdin.as_raw_fd());
            close(stdout.as_raw_fd());
            close(stderr.as_raw_fd());

            dup(slave_fd);
            dup(slave_fd);
            dup(slave_fd);

            close(slave_fd);

            setsid().unwrap();

            unsafe {
                libc::ioctl(0, libc::TIOCSCTTY.into(), 1);
            }

            let command = CString::new("sh").unwrap();
            execvp(&command, &[
               CString::new("").unwrap(),
            ]);

        },
        Err(_) => println!("Fork failed"),
    }

}
