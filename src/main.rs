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
use std::os::unix::io::IntoRawFd;
use std::fs::OpenOptions;
use std::thread;
use std::os::unix::net::{UnixStream, UnixListener};
use std::error::Error;
use std::fs::create_dir_all;
use std::mem;

extern crate forkpty;

use forkpty::*;


fn main() -> Result<(), Box<Error>> {

    let fork_result = forkpty();

    match fork_result {

        Ok(ForkPtyResult::Parent(child, mut pty_master)) => {

            let winsize: Winsize = Winsize {
                ws_row: 34,
                ws_col: 125,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };

            pty_master.resize(winsize);
            let mut pty_master_reader = pty_master.get_reader().unwrap();
            let mut pty_master_writer = pty_master.get_writer().unwrap();

            let out_listener = UnixListener::bind("/tmp/new_process_out.sock").unwrap();
            let in_listener = UnixListener::bind("/tmp/new_process_in.sock").unwrap();

            let (mut out_stream, _) = out_listener.accept().unwrap();

            thread::spawn(move || {

                let mut buffer: Vec<u8> = vec![0; libc::BUFSIZ as usize];

                loop {

                    let count = pty_master_reader.read(&mut buffer).unwrap();
                    // let count = read(master_fd, &mut buffer).unwrap();
                    out_stream.write(&buffer[0..count]).unwrap();
                    out_stream.flush().unwrap();

                }

            });

            let (mut in_stream, _) = in_listener.accept().unwrap();

            thread::spawn(move || {

                for byte in in_stream.bytes() {
                    pty_master_writer.write(&[byte.unwrap()]);
                    // write(master_fd, &[byte.unwrap()]);
                }

            });

            let result = child.wait();
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
