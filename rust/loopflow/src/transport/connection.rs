//! Ephemeral Desktop presentation, outside the provider's terminal stream.
//! Desktop creates a private directory per surface and removes it on closure.
use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;

use crate::id::LfProcessId;

const MAX_ATTEMPTS: u32 = 8;

#[derive(Debug)]
pub(super) struct Status {
    path: PathBuf,
    attempts: u32,
    generation: u64,
}

#[derive(Debug, Serialize)]
struct Reading<'a> {
    generation: u64,
    state: &'a str,
    input: bool,
    message: Option<&'a str>,
}

impl Status {
    pub(super) fn from_env() -> Option<Self> {
        std::env::var_os("LF_MACHINE_STATUS").map(|path| Self {
            path: path.into(),
            attempts: 0,
            generation: 0,
        })
    }

    fn publish(&self, state: &str, input: bool, message: Option<&str>) -> Result<()> {
        let pending = self.path.with_extension("pending");
        std::fs::write(
            &pending,
            serde_json::to_vec(&Reading {
                generation: self.generation,
                state,
                input,
                message,
            })?,
        )?;
        std::fs::rename(pending, &self.path)?;
        Ok(())
    }

    pub(super) fn connected(&mut self, input: bool) -> Result<()> {
        self.attempts = 0;
        self.publish("connected", input, None)
    }

    pub(super) fn ended(&self) -> Result<()> {
        self.publish("ended", false, None)
    }

    pub(super) fn connect(
        &mut self,
        owner: &LfProcessId,
        mut initial: bool,
    ) -> Result<Option<UnixStream>> {
        self.generation += 1;
        // Each cycle is bounded; a native Retry action starts another cycle.
        loop {
            self.publish("reconnecting", false, None)?;
            super::discard_reconnect_input()?;
            let mut reason = "The exact owner did not acknowledge its connection".to_owned();
            if initial {
                match self.prepare() {
                    Ok(false) => return Ok(None),
                    Ok(true) => initial = false,
                    Err(error) => {
                        reason = error.to_string();
                        self.attempts = MAX_ATTEMPTS;
                    }
                }
            }
            while self.attempts < MAX_ATTEMPTS {
                let attempt = self.attempts;
                self.attempts += 1;
                let until = Instant::now() + Duration::from_millis(100 << attempt.min(5));
                while Instant::now() < until {
                    if !self.wait(Duration::from_millis(50))? {
                        return Ok(None);
                    }
                }
                let owner = owner.clone();
                let (send, receive) = mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let result = crate::machine_service::client::bridge_existing(&owner);
                    let _ = send.send(result);
                });
                loop {
                    if !self.wait(Duration::from_millis(50))? {
                        return Ok(None);
                    }
                    match receive.try_recv() {
                        Ok(Ok(socket)) => return Ok(Some(socket)),
                        Ok(Err(error)) => {
                            reason = error.to_string();
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            reason = "Connection worker stopped".into();
                            break;
                        }
                    }
                }
            }
            let message = format!("Session unavailable. Last screen is stale. {reason}. Check the selected Loopflow installation and Background Items, then Retry. No provider was restarted.");
            self.publish("unavailable", false, Some(&message))?;
            loop {
                if !self.wait(Duration::from_millis(100))? {
                    return Ok(None);
                }
                if std::fs::remove_file(self.path.with_extension("retry")).is_ok() {
                    self.attempts = 0;
                    break;
                }
            }
        }
    }

    // Hold the setup child here, so closing a pane cannot leave an installer
    // waiting on the registration lock after its viewer has gone away.
    fn prepare(&self) -> Result<bool> {
        let diagnostic = self.path.with_extension("setup");
        let mut child = crate::machine_service::client::setup_command()?
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&diagnostic)?)
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(30);
        let result = (|| -> Result<bool> {
            loop {
                if !self.wait(Duration::from_millis(50))? {
                    return Ok(false);
                }
                if let Some(status) = child.try_wait()? {
                    anyhow::ensure!(
                        status.success(),
                        "{}",
                        std::fs::read_to_string(&diagnostic)?
                    );
                    return Ok(true);
                }
                anyhow::ensure!(Instant::now() < deadline, "Machine service setup timed out; check the selected installation and Background Items");
            }
        })();
        // Also reap a failed/timed-out setup; no subprocess outlives this call.
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(diagnostic);
        result
    }

    // Raw keys are discarded, never treated as Retry or queued across a
    // connection boundary. Closure cancels even a blocked handshake.
    fn wait(&self, delay: Duration) -> io::Result<bool> {
        if !self.path.parent().is_some_and(|path| path.is_dir()) {
            return Ok(false);
        }
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll borrows one valid descriptor entry.
        let ready = unsafe { libc::poll(&mut fd, 1, delay.as_millis() as i32) };
        if ready <= 0 {
            return Ok(true);
        }
        let mut bytes = [0; 8192];
        let n = io::stdin().read(&mut bytes)?;
        Ok(n != 0 && !bytes[..n].contains(&0x1d))
    }
}
