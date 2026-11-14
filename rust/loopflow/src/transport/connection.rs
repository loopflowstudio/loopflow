//! Ephemeral Desktop presentation, outside the provider's terminal stream.
//! Desktop creates a private directory per surface and removes it on closure.
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};

const MAX_ATTEMPTS: u32 = 8;

#[derive(Debug)]
pub(super) struct Status {
    path: PathBuf,
    attempts: u32,
    generation: u64,
    connected_once: bool,
    reading: Reading,
    stream: Option<UnixStream>,
    received: Vec<u8>,
    pending_move: Option<u64>,
    retry: bool,
    gap: crate::store::RecordingGap,
}

#[derive(Debug, Serialize)]
struct Reading {
    generation: u64,
    state: &'static str,
    input: bool,
    native_ready: bool,
    message: Option<String>,
    transition: Option<crate::store::sqlite::provider_transitions::ProviderTransition>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Action {
    Retry,
    Move { generation: u64 },
}

impl Status {
    pub(super) fn from_env() -> Option<Self> {
        std::env::var_os("LF_SERVER_STATUS").map(|path| Self {
            path: path.into(),
            attempts: 0,
            generation: 0,
            connected_once: false,
            reading: Reading {
                generation: 0,
                state: "connecting",
                input: false,
                native_ready: false,
                message: None,
                transition: None,
            },
            stream: None,
            received: Vec::new(),
            pending_move: None,
            retry: false,
            gap: crate::store::RecordingGap::default(),
        })
    }

    fn publish(&mut self, state: &'static str, input: bool, message: Option<&str>) {
        self.reading.generation = self.generation;
        self.reading.state = state;
        self.reading.input = input;
        self.reading.message = message.map(str::to_owned);
        if self.stream.is_none() {
            self.refresh(); // A new connection receives the latest reading below.
        } else if let Err(error) = self.write_reading() {
            self.disconnect(&error);
        }
    }

    fn write_reading(&mut self) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&self.reading)?;
        bytes.push(b'\n');
        if let Some(stream) = &mut self.stream {
            stream.write_all(&bytes)?;
        }
        Ok(())
    }

    fn disconnect(&mut self, error: &io::Error) {
        // A broken/slow status consumer cannot block the provider's byte path.
        // Closing also makes Desktop discard its previous input permission.
        self.stream = None;
        self.received.clear();
        self.pending_move = None;
        self.retry = false;
        if !self.closed() {
            self.gap.note("publish terminal connection status", error);
        }
    }

    fn open_stream(&self) -> io::Result<UnixStream> {
        // A suspended Desktop may not accept. Even connect must not block the relay.
        // SAFETY: socket creates an unowned descriptor, checked before adoption.
        let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd is a new socket owned only by this stream.
        let stream = unsafe { UnixStream::from_raw_fd(fd) };
        stream.set_nonblocking(true)?;
        // SAFETY: fcntl operates on the live owned descriptor.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: zero is a valid empty sockaddr_un, populated before connect.
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        address.sun_family = libc::AF_UNIX as _;
        let path = self.path.as_os_str().as_encoded_bytes();
        if path.len() >= address.sun_path.len() {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        for (to, from) in address.sun_path.iter_mut().zip(path) {
            *to = *from as _;
        }
        // SAFETY: address points to an initialized Unix address of the given size.
        if unsafe {
            libc::connect(
                stream.as_raw_fd(),
                (&raw const address).cast(),
                std::mem::size_of_val(&address) as _,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(stream)
    }

    fn refresh(&mut self) {
        if self.closed() {
            return;
        }
        if self.stream.is_none() {
            let result = self.open_stream();
            match result {
                Ok(stream) => {
                    self.stream = Some(stream);
                    if let Err(error) = self.write_reading() {
                        self.disconnect(&error);
                        return;
                    }
                    self.gap.recovered();
                }
                Err(error) => {
                    self.disconnect(&error);
                    return;
                }
            }
        }
        let mut bytes = [0; 1024];
        loop {
            match self
                .stream
                .as_mut()
                .expect("status stream connected")
                .read(&mut bytes)
            {
                Ok(0) => {
                    self.disconnect(&io::Error::from(io::ErrorKind::UnexpectedEof));
                    return;
                }
                Ok(n) => self.received.extend_from_slice(&bytes[..n]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    self.disconnect(&error);
                    return;
                }
            }
            if self.received.len() > 8192 {
                self.disconnect(&io::Error::from(io::ErrorKind::InvalidData));
                return;
            }
        }
        while let Some(end) = self.received.iter().position(|byte| *byte == b'\n') {
            let line: Vec<_> = self.received.drain(..=end).collect();
            match serde_json::from_slice::<Action>(&line) {
                Ok(Action::Retry) if self.reading.state == "unavailable" => self.retry = true,
                Ok(Action::Move { generation }) if self.reading.state == "connected" => {
                    self.pending_move = Some(generation)
                }
                _ => {}
            }
        }
    }

    pub(super) fn connected(&mut self, input: bool) {
        self.connected_once = true;
        self.attempts = 0;
        self.publish("connected", input, None);
    }

    pub(super) fn native_ready(&mut self) {
        self.reading.native_ready = true;
        self.publish(self.reading.state, self.reading.input, None);
    }

    pub(super) fn ended(&mut self) {
        self.publish("ended", false, None);
    }

    pub(super) fn closed(&self) -> bool {
        self.path.parent().is_some_and(|path| {
            matches!(std::fs::metadata(path), Err(error) if error.kind() == io::ErrorKind::NotFound)
        })
    }

    /// Consume before sending: a lost acknowledgement never retries a takeover.
    pub(super) fn take_move(&mut self, ready: bool) -> bool {
        self.refresh();
        let requested = self.pending_move.take();
        if !ready || self.closed() || requested != Some(self.generation) {
            return false;
        }
        self.generation += 1;
        self.publish("moving", false, None);
        true
    }

    /// Returns the bridge and whether connection failure consumed the initial
    /// intent to replace claims. Recovery may only ask for free claims.
    pub(super) fn connect(
        &mut self,
        owner: &super::Host,
        session: &crate::id::LfSessionId,
        initial: bool,
    ) -> Result<Option<(UnixStream, bool)>> {
        self.generation += 1;
        self.reading.native_ready = false;
        let waiting = if self.connected_once {
            "reconnecting"
        } else {
            "connecting"
        };
        self.publish(waiting, false, None);
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
            self.publish(waiting, false, setup_error.as_deref());
            super::discard_reconnect_input()?;
            let mut reason = "The exact owner did not acknowledge its connection".to_owned();
            while self.attempts < MAX_ATTEMPTS {
                let attempt = self.attempts;
                self.attempts += 1;
                let delay = if initial && attempt == 0 {
                    Duration::ZERO
                } else {
                    super::pace().backoff * (1 << attempt.min(5))
                };
                let until = Instant::now() + delay;
                while Instant::now() < until {
                    if !self.wait(Duration::from_millis(50))? {
                        return Ok(None);
                    }
                }
                let (owner, session) = (owner.clone(), session.clone());
                let (send, receive) = mpsc::sync_channel(1);
                std::thread::spawn(move || {
                    let result = super::open_bridge(&owner, &session);
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
                                if self.reading.transition.as_ref().is_some_and(|old| {
                                    old.to_agent_process == next.to_agent_process
                                        && old.to_provider_session == next.to_provider_session
                                }) {
                                    next.kind = crate::store::sqlite::provider_transitions::TransitionKind::Retained;
                                }
                            }
                            self.reading.transition = transition;
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
            self.publish("unavailable", false, Some(&message));
            loop {
                if !self.wait(Duration::from_millis(100))? {
                    return Ok(None);
                }
                if std::mem::take(&mut self.retry) {
                    self.attempts = 0;
                    break;
                }
            }
        }
    }

    // Hold the setup child here, so closing a pane cannot leave an installer
    // waiting on the registration lock after its viewer has gone away.
    fn prepare(&mut self) -> Result<bool> {
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
    fn wait(&mut self, delay: Duration) -> io::Result<bool> {
        if self.closed() {
            return Ok(false);
        }
        self.refresh();
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
