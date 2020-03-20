use std::future::Future;
use std::pin::Pin;
use std::task::{ Context, Poll };
use nix::unistd::Pid;
use nix::sys::wait::{ waitpid, WaitStatus, WaitPidFlag };
use tokio::signal::unix::{ signal, Signal, SignalKind };
use derive_more::From;

struct WaitPidFuture {
    pid: Pid,
    signal: Signal,
}

impl WaitPidFuture {
    pub fn new(pid: Pid) -> Self {
        let signal = signal(SignalKind::child()).unwrap();
        Self { pid, signal }
    }
}

impl Future for WaitPidFuture {
    type Output = nix::Result<WaitStatus>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let _ = self.signal.poll_recv(cx);

        match waitpid(self.pid, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) => {
                Poll::Pending
            },
            result => Poll::Ready(result),
        }
    }
}

#[derive(Debug, Clone, From)]
pub struct AsyncChild(pub Pid);

impl AsyncChild {
    pub async fn status(&self) -> nix::Result<WaitStatus> {
        WaitPidFuture::new(self.0).await
    }
}
