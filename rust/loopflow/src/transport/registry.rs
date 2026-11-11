//! The Machine server's held terminals. One thread hosts every terminal a
//! holder keeps: it reads their output from the holders, keeps their screens,
//! serves their viewers and settles their exits. Nothing here is a process;
//! what it knows is rebuilt from the holders, the per-terminal files and the
//! store whenever a server starts.
//!
//! Every connection to a held terminal, a view's or its own child's, enters
//! through one socket and names its terminal on the first line.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::holder::client::Client;
use crate::holder::wire::{self, Held};
use crate::holder::Place;
use crate::id::{LfSessionId, TerminalId};

use super::held::Source;
use super::hosted::{Child, Hosted, Pass};
use super::{
    dimensions, drain, wait_for_activity, Host, CONTROL_ENV, HELD_ENV, OWNER_ENV, PARENT_ENV,
    STREAM_ENV, TERMINAL_ENV,
};

/// How long a connection has to name its terminal.
const ARRIVAL: Duration = Duration::from_secs(5);
/// How long the narrowed size stands before a rebuilt screen's size returns.
const NUDGE: Duration = Duration::from_millis(400);
/// How often an exit the store could not record is offered again.
const SETTLE_RETRY: Duration = Duration::from_secs(2);

/// What the server writes beside a terminal before asking its holder to start
/// it, so that another server instance can host the same child.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Meta {
    #[serde(deserialize_with = "Option::deserialize")]
    session: Option<LfSessionId>,
    /// Identifies the start; repeating it answers this terminal.
    request: String,
    /// What the child's own control requests present.
    control_key: String,
}

impl Meta {
    fn path(place: &Place, terminal: &TerminalId) -> PathBuf {
        place.terminal(terminal.as_str()).join("meta.json")
    }

    fn read(place: &Place, terminal: &TerminalId) -> Option<Self> {
        let bytes = std::fs::read(Self::path(place, terminal)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn write(&self, place: &Place, terminal: &TerminalId) -> Result<()> {
        let path = Self::path(place, terminal);
        std::fs::create_dir_all(path.parent().expect("meta has a directory"))?;
        let draft = path.with_extension("json.draft");
        std::fs::write(&draft, serde_json::to_vec(self)?)?;
        std::fs::rename(draft, path)?;
        Ok(())
    }
}

/// A managed launch: the command to run in a holder's PTY.
#[derive(Debug, Clone)]
pub(crate) struct Start {
    pub session: Option<LfSessionId>,
    pub request: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// The caller's whole environment.
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

/// Where connections to held terminals enter the server.
pub(crate) fn entrance() -> Result<PathBuf> {
    Ok(super::private_directory()?.join("held.sock"))
}

/// A connection speaking the viewer protocol with `terminal`. It fails when no
/// server is running, which says nothing about the terminal's agent; a server
/// that does not host the terminal answers `not_found`.
pub(crate) fn connect(terminal: &TerminalId) -> Result<UnixStream> {
    let mut socket = UnixStream::connect(entrance()?)
        .context("Loopflow server is not running; run lf server status")?;
    socket.set_read_timeout(Some(ARRIVAL))?;
    socket.write_all(&named(terminal))?;
    let mut line = Vec::new();
    loop {
        let mut byte = [0];
        match socket.read(&mut byte) {
            Ok(0) => anyhow::bail!("Loopflow server closed the connection; run lf server status"),
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) if line.len() < 128 => line.push(byte[0]),
            Ok(_) => anyhow::bail!("Loopflow server answered a terminal connection with noise"),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => {
                return Err(e).context("Loopflow server did not answer; run lf server status")
            }
        }
    }
    let answer: Answered = serde_json::from_slice(&line)?;
    if !answer.hosted {
        return Err(ended(terminal).into());
    }
    socket.set_read_timeout(None)?;
    Ok(socket)
}

/// The refusal for a terminal no server hosts: it ended, or never started.
/// Attaching never starts one.
pub(crate) fn ended(terminal: &TerminalId) -> crate::engine::Error {
    crate::engine::Error::new(
        crate::engine::ErrorCode::NotFound,
        format!("terminal {terminal} is not running; nothing was started and no input replayed"),
    )
    .with(serde_json::json!({ "terminal_id": terminal }))
}

/// What the entrance answers a named connection before the viewer protocol.
#[derive(Serialize, Deserialize)]
struct Answered {
    hosted: bool,
}

/// The first line of a connection through the entrance.
pub(crate) fn named(terminal: &TerminalId) -> Vec<u8> {
    wire::line(&Named {
        terminal: terminal.clone(),
    })
}

#[derive(Serialize, Deserialize)]
struct Named {
    terminal: TerminalId,
}

struct Entry {
    hosted: Hosted,
    child: Child,
    place: Place,
    /// A screen rebuilt from the ring asks its program to repaint once the
    /// replay reaches this position.
    nudge: Option<Nudge>,
}

enum Nudge {
    Replaying {
        until: u64,
        rows: u16,
        cols: u16,
    },
    Narrowed {
        since: Instant,
        rows: u16,
        cols: u16,
    },
}

enum Command {
    Add(TerminalId, Box<Entry>),
    /// The server is going; its terminals stay with their holders. A real
    /// server's exit is enough; fixtures replace one within a process.
    #[cfg(test)]
    Stop,
}

/// An ended terminal whose exit the store has not recorded yet. Its
/// directory is the receipt and stays until it has.
struct Unsettled {
    terminal: TerminalId,
    place: Place,
    code: Option<u8>,
    retry_at: Instant,
}

#[derive(Debug)]
pub(crate) struct Registry {
    build: String,
    place: Place,
    commands: Mutex<mpsc::Sender<Command>>,
    waker: UnixStream,
    /// Starts by request identity, so a repeated request starts nothing.
    started: Mutex<HashMap<String, TerminalId>>,
    /// The terminals hosted now and the Session each was started for.
    live: Live,
    #[cfg(test)]
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

type Live = Arc<Mutex<HashMap<TerminalId, Option<LfSessionId>>>>;

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add(terminal, _) => write!(f, "Add({terminal})"),
            #[cfg(test)]
            Self::Stop => f.write_str("Stop"),
        }
    }
}

impl Registry {
    /// Bind the entrance, take up every terminal the holders already keep, and
    /// host them on one thread. `build` is this executable's SHA-256.
    ///
    /// The caller holds the server's lock: only one registry serves a Machine.
    pub(crate) fn open(build: &str) -> Result<Arc<Self>> {
        let path = entrance()?;
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
        let listener = UnixListener::bind(&path)
            .with_context(|| format!("Bind held terminal entrance {}", path.display()))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        let (wake, waker) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        waker.set_nonblocking(true)?;
        let (commands, received) = mpsc::channel();
        let registry = Arc::new(Self {
            build: build.to_owned(),
            place: Place::of(build)?,
            commands: Mutex::new(commands),
            waker: waker.try_clone()?,
            started: Mutex::new(HashMap::new()),
            live: Live::default(),
            #[cfg(test)]
            thread: Mutex::new(None),
        });
        let mut unsettled = Vec::new();
        let entries = registry.adopt(&waker, &mut unsettled);
        let live = registry.live.clone();
        #[cfg(test)]
        let ledger = crate::journal::test_ledger_path();
        let _thread = std::thread::Builder::new()
            .name("held-terminals".into())
            .spawn(move || {
                let run = || run(listener, wake, received, entries, unsettled, live);
                #[cfg(test)]
                let run = || match ledger {
                    Some(ledger) => crate::journal::with_test_ledger(ledger, run),
                    None => run(),
                };
                run()
            })?;
        #[cfg(test)]
        {
            *registry.thread.lock().expect("registry lock poisoned") = Some(_thread);
        }
        Ok(registry)
    }

    /// Stop hosting. Every terminal stays with its holder for the next server.
    #[cfg(test)]
    pub(crate) fn close(&self) {
        let _ = self.send(Command::Stop);
        if let Some(thread) = self.thread.lock().expect("registry lock poisoned").take() {
            let _ = thread.join();
        }
    }

    /// The running terminal started for `session`.
    pub(crate) fn session_terminal(&self, session: &LfSessionId) -> Option<TerminalId> {
        self.live
            .lock()
            .expect("registry lock poisoned")
            .iter()
            .find(|(_, held)| held.as_ref() == Some(session))
            .map(|(terminal, _)| terminal.clone())
    }

    /// Start `start`'s command in this build's holder and host it. Repeating a
    /// request answers the terminal it already started.
    pub(crate) fn start(&self, start: Start) -> Result<TerminalId> {
        let mut started = self.started.lock().expect("registry lock poisoned");
        if let Some(terminal) = started.get(&start.request) {
            return Ok(terminal.clone());
        }
        let terminal = TerminalId::new();
        let meta = Meta {
            session: start.session.clone(),
            request: start.request.clone(),
            control_key: uuid::Uuid::new_v4().to_string(),
        };
        meta.write(&self.place, &terminal)?;
        crate::journal::process_ledger()?.create_terminal(&terminal, &self.build)?;
        let mut client = Client::ensure(&self.place)?;
        let holder = client.hello()?;
        let (rows, cols) = dimensions(start.rows, start.cols);
        let mut env: Vec<(String, String)> = start
            .env
            .into_iter()
            // A pane's relay variables describe that pane's viewer, and an
            // owner's handoff the owner; neither is this child's.
            .filter(|(name, _)| {
                ![
                    OWNER_ENV,
                    HELD_ENV,
                    PARENT_ENV,
                    CONTROL_ENV,
                    STREAM_ENV,
                    TERMINAL_ENV,
                    "LF_TERMINAL_ID",
                    "LF_TERMINAL_TTY",
                    "LF_SERVER",
                    "LF_SERVER_STATUS",
                ]
                .contains(&name.as_str())
            })
            .collect();
        env.push((HELD_ENV.into(), terminal.to_string()));
        env.push((PARENT_ENV.into(), holder.pid.to_string()));
        env.push((CONTROL_ENV.into(), meta.control_key.clone()));
        // An unanswered start may have started the child. The holder's
        // inventory says which before anything is tried again.
        let request = wire::Start {
            terminal: terminal.to_string(),
            request: start.request.clone(),
            argv: start.argv,
            cwd: start.cwd,
            env,
            rows,
            cols,
        };
        let pid = match client.start(request) {
            Ok(pid) => pid,
            Err(error) => {
                let listed = Client::connect(&self.place)
                    .map_err(anyhow::Error::from)
                    .and_then(|mut client| client.list());
                match listed {
                    Ok(listed) => match listed
                        .iter()
                        .find(|held| held.terminal == terminal.as_str())
                    {
                        Some(held) => held.pid,
                        None => {
                            // The holder answers and never started it.
                            let _ = std::fs::remove_dir_all(self.place.terminal(terminal.as_str()));
                            let _ = crate::journal::process_ledger()
                                .and_then(|store| store.end_terminal(&terminal, None));
                            return Err(error);
                        }
                    },
                    Err(_) => {
                        return Err(error.context(format!(
                            "terminal {terminal} may have started; its holder is not answering"
                        )))
                    }
                }
            }
        };
        let entry = Entry::new(
            &self.place,
            &terminal,
            pid,
            0,
            meta.control_key,
            &self.waker,
        )?;
        self.live
            .lock()
            .expect("registry lock poisoned")
            .insert(terminal.clone(), start.session);
        self.send(Command::Add(terminal.clone(), Box::new(entry)))?;
        started.insert(start.request, terminal.clone());
        Ok(terminal)
    }

    fn send(&self, command: Command) -> Result<()> {
        self.commands
            .lock()
            .expect("registry lock poisoned")
            .send(command)
            .ok()
            .context("held terminal loop has stopped")?;
        let _ = (&self.waker).write_all(&[1]);
        Ok(())
    }

    /// Take up what the holders keep: running terminals are hosted again under
    /// their ids, and exits that happened with no server running are settled.
    fn adopt(
        &self,
        waker: &UnixStream,
        unsettled: &mut Vec<Unsettled>,
    ) -> Vec<(TerminalId, Entry)> {
        let mut entries = Vec::new();
        let mut known = Vec::new();
        for place in holders() {
            let listed = Client::connect(&place)
                .map_err(anyhow::Error::from)
                .and_then(|mut client| {
                    client.hello()?;
                    client.list()
                });
            let listed = match listed {
                Ok(listed) => listed,
                Err(error) => {
                    // A holder that is alive and slow keeps its terminals;
                    // their views say so until it answers.
                    tracing::warn!(%error, socket = %place.socket.display(), "holder did not answer at startup");
                    continue;
                }
            };
            for held in listed {
                let Ok(terminal) = TerminalId::parse(&held.terminal) else {
                    continue;
                };
                known.push(terminal.clone());
                if let Some(exit) = &held.exit {
                    unsettled.push(Unsettled {
                        terminal,
                        place: place.clone(),
                        code: Some(exit.status()),
                        retry_at: Instant::now(),
                    });
                    continue;
                }
                match self.readopt(&place, &terminal, &held, waker) {
                    Ok(entry) => entries.push((terminal, entry)),
                    Err(error) => {
                        tracing::warn!(%error, %terminal, "held terminal could not be hosted again")
                    }
                }
            }
        }
        // A terminal the store still calls running whose holder is gone.
        let recorded = crate::journal::process_ledger()
            .and_then(|store| store.running_terminals())
            .unwrap_or_default();
        for (terminal, build) in recorded {
            if known.contains(&terminal) {
                continue;
            }
            let Ok(place) = Place::of(&build) else {
                continue;
            };
            if Client::connect(&place).is_ok() {
                // Its holder is there and did not list it in time; leave it.
                continue;
            }
            let code = crate::holder::recorded_exit(&place, terminal.as_str())
                .ok()
                .flatten()
                .map(|exit| exit.status());
            unsettled.push(Unsettled {
                terminal,
                place,
                code,
                retry_at: Instant::now(),
            });
        }
        entries
    }

    fn readopt(
        &self,
        place: &Place,
        terminal: &TerminalId,
        held: &Held,
        waker: &UnixStream,
    ) -> Result<Entry> {
        let meta = Meta::read(place, terminal)
            .with_context(|| format!("terminal {terminal} has no readable meta.json"))?;
        self.started
            .lock()
            .expect("registry lock poisoned")
            .insert(meta.request.clone(), terminal.clone());
        self.live
            .lock()
            .expect("registry lock poisoned")
            .insert(terminal.clone(), meta.session.clone());
        let mut entry = Entry::new(
            place,
            terminal,
            held.pid,
            held.ring_start,
            meta.control_key,
            waker,
        )?;
        entry.hosted.restore_size(held.rows, held.cols)?;
        // The screen is rebuilt from the ring, then the program repaints what
        // the ring no longer holds of the visible screen.
        entry.nudge = Some(Nudge::Replaying {
            until: held.written,
            rows: held.rows,
            cols: held.cols,
        });
        Ok(entry)
    }
}

/// Every holder socket in this Machine's private directory, by its place.
fn holders() -> Vec<Place> {
    let Ok(private) = super::private_directory() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&private) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let short = name.strip_prefix("holder-")?.strip_suffix(".sock")?;
            Place::of(short).ok()
        })
        .collect()
}

/// A connection that has not named its terminal yet.
struct Arriving {
    socket: UnixStream,
    line: Vec<u8>,
    since: Instant,
}

impl Arriving {
    /// The terminal named, once the whole line is here. Read a byte at a
    /// time: what follows the line belongs to the terminal's host.
    fn named(&mut self) -> io::Result<Option<TerminalId>> {
        loop {
            let mut byte = [0];
            match self.socket.read(&mut byte) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(_) if byte[0] == b'\n' => {
                    let named: Named = serde_json::from_slice(&self.line)?;
                    return Ok(Some(named.terminal));
                }
                Ok(_) if self.line.len() >= 128 => return Err(io::ErrorKind::InvalidData.into()),
                Ok(_) => self.line.push(byte[0]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}

fn run(
    listener: UnixListener,
    wake: UnixStream,
    commands: mpsc::Receiver<Command>,
    adopted: Vec<(TerminalId, Entry)>,
    mut unsettled: Vec<Unsettled>,
    live: Live,
) {
    let mut terminals: HashMap<TerminalId, Entry> = adopted.into_iter().collect();
    let mut arriving: Vec<Arriving> = Vec::new();
    loop {
        while let Ok((socket, _)) = listener.accept() {
            if socket.set_nonblocking(true).is_ok() {
                arriving.push(Arriving {
                    socket,
                    line: Vec::new(),
                    since: Instant::now(),
                });
            }
        }
        drain(&wake);
        for command in commands.try_iter() {
            match command {
                Command::Add(terminal, entry) => {
                    terminals.insert(terminal, *entry);
                }
                #[cfg(test)]
                Command::Stop => return,
            }
        }
        let mut waiting = Vec::new();
        for mut connection in arriving.drain(..) {
            match connection.named() {
                Ok(Some(terminal)) => {
                    // A terminal that is not hosted here has ended or was
                    // never started: closing says so. Attach never creates.
                    let entry = terminals.get_mut(&terminal);
                    let answer = wire::line(&Answered {
                        hosted: entry.is_some(),
                    });
                    // Sixteen bytes into an empty socket buffer.
                    if (&connection.socket).write_all(&answer).is_err() {
                        continue;
                    }
                    if let Some(entry) = entry {
                        entry.hosted.admit(connection.socket);
                    }
                }
                Ok(None) if connection.since.elapsed() < ARRIVAL => waiting.push(connection),
                Ok(None) | Err(_) => {}
            }
        }
        arriving = waiting;
        let mut ended = Vec::new();
        for (terminal, entry) in &mut terminals {
            // One terminal's failure is never another's.
            let pass = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                entry.nudge();
                entry.hosted.pass(&mut entry.child)
            }));
            match pass {
                Ok(Ok(Pass::Continue)) => {}
                Ok(Ok(Pass::Ended(code, settling))) => {
                    // Exit records still being written finish off this loop.
                    if let Some(settling) = settling {
                        std::thread::spawn(move || settling.linger());
                    }
                    let code = match &entry.child {
                        Child::Held(source) if source.outcome_unknown() => None,
                        _ => Some(code),
                    };
                    ended.push((terminal.clone(), code));
                }
                Ok(Err(error)) => {
                    tracing::warn!(%error, %terminal, "held terminal pass failed; continuing")
                }
                Err(_) => {
                    tracing::error!(%terminal, "held terminal host panicked; its agent keeps running in the holder");
                    ended.push((terminal.clone(), None));
                }
            }
        }
        for (terminal, code) in ended {
            let Some(entry) = terminals.remove(&terminal) else {
                continue;
            };
            live.lock()
                .expect("registry lock poisoned")
                .remove(&terminal);
            let running = matches!(&entry.child, Child::Held(source) if !source.drained());
            if running {
                // The host is gone, not the child; a later server takes it up.
                continue;
            }
            unsettled.push(Unsettled {
                terminal,
                place: entry.place,
                code,
                retry_at: Instant::now(),
            });
        }
        unsettled.retain_mut(|exit| exit.retry_at > Instant::now() || !settle(exit));
        let mut fds = vec![
            libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        for connection in &arriving {
            fds.push(libc::pollfd {
                fd: connection.socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            });
        }
        let mut deadline = unsettled.iter().map(|exit| exit.retry_at).min();
        for entry in terminals.values() {
            entry.hosted.poll(&entry.child, &mut fds);
            deadline = deadline
                .into_iter()
                .chain(entry.hosted.deadline(&entry.child))
                .chain(entry.nudge_at())
                .min();
        }
        if !arriving.is_empty() {
            deadline = deadline
                .into_iter()
                .chain(Some(Instant::now() + ARRIVAL))
                .min();
        }
        wait_for_activity(
            &mut fds,
            deadline.map(|at| at.saturating_duration_since(Instant::now())),
        );
    }
}

impl Entry {
    /// Host `terminal`, whose child is `pid`, reading its output from byte `at`.
    fn new(
        place: &Place,
        terminal: &TerminalId,
        pid: u32,
        at: u64,
        control_key: String,
        waker: &UnixStream,
    ) -> Result<Self> {
        Ok(Self {
            hosted: Hosted::new(
                Host::Terminal(terminal.clone()),
                true,
                None,
                control_key,
                waker.try_clone()?,
            )?,
            child: Child::Held(Source::new(place.clone(), terminal.clone(), pid, at)),
            place: place.clone(),
            nudge: None,
        })
    }

    /// Ask a program whose screen was rebuilt from the ring to repaint: one
    /// column narrower, then back.
    fn nudge(&mut self) {
        let Child::Held(source) = &mut self.child else {
            return;
        };
        // Output the ring dropped before it was read is missing from the
        // screen; the program is asked to draw it again.
        if source.take_gap().is_some() && self.nudge.is_none() {
            let (rows, cols) = self.hosted.size();
            self.nudge = Some(Nudge::Replaying {
                until: source.position(),
                rows,
                cols,
            });
        }
        match self.nudge {
            Some(Nudge::Replaying { until, rows, cols }) if source.position() >= until => {
                source.resize(rows, cols.saturating_sub(1).max(2));
                self.nudge = Some(Nudge::Narrowed {
                    since: Instant::now(),
                    rows,
                    cols,
                });
            }
            Some(Nudge::Narrowed { since, rows, cols }) if since.elapsed() >= NUDGE => {
                source.resize(rows, cols);
                self.nudge = None;
            }
            _ => {}
        }
    }

    fn nudge_at(&self) -> Option<Instant> {
        match self.nudge {
            Some(Nudge::Narrowed { since, .. }) => Some(since + NUDGE),
            _ => None,
        }
    }
}

/// Record an ended terminal, then drop its ring and its directory. The
/// directory is the receipt: it goes only once the store has the exit, so a
/// store outage leaves it for the next attempt. Whether it is settled.
fn settle(exit: &mut Unsettled) -> bool {
    let recorded = crate::journal::process_ledger()
        .and_then(|store| store.end_terminal(&exit.terminal, exit.code));
    if let Err(error) = recorded {
        tracing::warn!(%error, terminal = %exit.terminal, "terminal exit not recorded; keeping its receipt");
        exit.retry_at = Instant::now() + SETTLE_RETRY;
        return false;
    }
    if let Ok(mut client) = Client::connect(&exit.place) {
        let _ = client.forget(exit.terminal.as_str());
    }
    remove_receipt(&exit.place.terminal(exit.terminal.as_str()));
    true
}

fn remove_receipt(directory: &Path) {
    if let Err(error) = std::fs::remove_dir_all(directory) {
        if error.kind() != io::ErrorKind::NotFound {
            tracing::warn!(%error, directory = %directory.display(), "terminal receipt not removed");
        }
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
