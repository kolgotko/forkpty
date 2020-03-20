use nix::unistd::Pid;
use nix::sys::wait::{ waitpid, WaitStatus };
use derive_more::From;

#[derive(Debug, Clone, From)]
pub struct Child(pub Pid);

impl Child {
    pub fn status(&self) -> nix::Result<WaitStatus> {
        waitpid(self.0, None)
    }
}
