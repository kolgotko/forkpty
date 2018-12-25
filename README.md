# forkpty
Create PTY for forked process 

## Example:

```rust
extern crate forkpty;
use forkpty::*;

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

        // parent process logic

    },
    Ok(ForkPtyResult::Child(_)) => {

        // child process logic

    },
    _ => {
        // error handling
    },

}
```
