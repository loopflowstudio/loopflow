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
    transition: Option<crate::store::sqlite::provider_transitions::ProviderTransition>,
}

#[derive(Debug, Serialize)]
struct Reading<'a> {
    generation: u64,
    state: &'a str,
    input: bool,
    message: Option<&'a str>,
    transition: Option<&'a crate::store::sqlite::provider_transitions::ProviderTransition>,
}

impl Status {
    pub(super) fn from_env() -> Option<Self> {
        std::env::var_os("LF_SERVER_STATUS").map(|path| Self {
            path: path.into(),
            attempts: 0,
            generation: 0,
            transition: None,
        })
    }

    fn publish(&self, state: &str, input: bool, message: Option<&str>) -> Result<()> {
        let pending = self.path.with_extension("pending");
        let bytes = serde_json::to_vec(&Reading {
            generation: self.generation,
            state,
            input,
            message,
            transition: self.transition.as_ref(),
        })?;
        match std::fs::write(&pending, bytes).and_then(|()| std::fs::rename(pending, &self.path)) {
            // The pane atomically removes its mailbox while a publish can be in flight.
            Err(_) if self.closed() => Ok(()),
            result => result.map_err(Into::into),
        }
    }

    pub(super) fn connected(&mut self, input: bool) -> Result<()> {
        self.attempts = 0;
        self.publish("connected", input, None)
    }

    pub(super) fn ended(&self) -> Result<()> {
        self.publish("ended", false, None)
    }

    pub(super) fn closed(&self) -> bool {
        !self.path.parent().is_some_and(|path| path.is_dir())
    }

    /// Consume before sending: a lost acknowledgement never retries a takeover.
    pub(super) fn take_move(&mut self, ready: bool) -> Result<bool> {
        let path = self.path.with_extension("move");
        let requested = match std::fs::read(&path) {
            Ok(bytes) => {
                std::fs::remove_file(path)?;
                serde_json::from_slice::<u64>(&bytes).ok()
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        if !ready || self.closed() || requested != Some(self.generation) {
            return Ok(false);
        }
        self.generation += 1;
        self.publish("moving", false, None)?;
        Ok(true)
    }

    /// Returns the bridge and whether connection failure consumed the initial
    /// intent to replace claims. Recovery may only ask for free claims.
    pub(super) fn connect(
        &mut self,
        owner: &LfProcessId,
        session: &crate::id::LfSessionId,
        initial: bool,
    ) -> Result<Option<(UnixStream, bool)>> {
        self.generation += 1;
        self.publish("reconnecting", false, None)?;
        super::discard_reconnect_input()?;
        let mut recovered = !initial;
        // Readiness belongs to this first call, never to a Retry cycle. A failed
        // setup consumes it too; installation must repair availability separately.
        let setup_error = if initial {
            match self.prepare() {
                Ok(false) => return Ok(None),
                Ok(true) => None,
                Err(error) => {
                    recovered = true;
                    self.attempts = MAX_ATTEMPTS;
                    Some(error.to_string())
                }
            }
        } else {
            None
        };
        // Each cycle is bounded; a native Retry action starts another cycle.
        loop {
            self.publish("reconnecting", false, setup_error.as_deref())?;
            super::discard_reconnect_input()?;
            let mut reason = "The exact owner did not acknowledge its connection".to_owned();
            while self.attempts < MAX_ATTEMPTS {
                let attempt = self.attempts;
                self.attempts += 1;
                let until = Instant::now() + Duration::from_millis(100 << attempt.min(5));
                while Instant::now() < until {
                    if !self.wait(Duration::from_millis(50))? {
                        return Ok(None);
                    }
                }
                let (owner, session) = (owner.clone(), session.clone());
                let (send, receive) = mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let result = crate::server::client::bridge_existing(&session, &owner);
                    // Presentation cannot prevent the terminal from reconnecting.
                    let transition = crate::store::read_existing_registry()
                        .ok()
                        .flatten()
                        .and_then(|store| {
                            let current = store
                                .sqlite
                                .session_attachment(session.as_str())
                                .ok()
                                .flatten()?;
                            store
                                .sqlite
                                .provider_transition(session.as_str())
                                .ok()
                                .flatten()
                                .filter(|transition| {
                                    transition.to_agent_process == current.agent_process_id
                                })
                        });
                    let _ = send.send((result, transition));
                });
                loop {
                    if !self.wait(Duration::from_millis(50))? {
                        return Ok(None);
                    }
                    match receive.try_recv() {
                        Ok((Ok(socket), mut transition)) => {
                            if let Some(next) = transition.as_mut() {
                                if self.transition.as_ref().is_some_and(|old| {
                                    old.to_agent_process == next.to_agent_process
                                        && old.to_provider_session == next.to_provider_session
                                }) {
                                    next.kind = crate::store::sqlite::provider_transitions::TransitionKind::Retained;
                                }
                            }
                            self.transition = transition;
                            return Ok(Some((socket, recovered)));
                        }
                        Ok((Err(error), _)) => {
                            recovered = true;
                            reason = error.to_string();
                            break;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            recovered = true;
                            reason = "Connection worker stopped".into();
                            break;
                        }
                    }
                }
            }
            let setup = setup_error
                .as_ref()
                .map(|error| format!("Initial setup failed: {error}. "))
                .unwrap_or_default();
            let message = format!("Session unavailable. Last screen is stale. {setup}{reason}. Check the selected Loopflow installation and Background Items, then Retry. Retry only reconnects. No provider was restarted.");
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
        let mut child = crate::installation::server::readiness_command()?
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
                anyhow::ensure!(Instant::now() < deadline, "Loopflow server setup timed out; check the selected installation and Background Items");
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
        if self.closed() {
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
