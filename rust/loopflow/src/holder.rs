//! The holder: one process per build that keeps the PTYs and agent children of
//! managed Sessions while Machine servers come and go. It starts a child on a
//! PTY, keeps the newest output of each in a ring, takes input and resizes, and
//! reports exits. It opens no store and knows nothing about Sessions; the
//! server keeps the emulator, the views and every record.

pub mod client;
pub mod wire;

use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs2::FileExt;

use wire::{Answer, Exit, Gap, Held, Request, Start, Tag};

fn json(tag: Tag, value: &impl serde::Serialize, into: &mut VecDeque<u8>) {
    let payload = serde_json::to_vec(value).expect("holder frames serialize");
    wire::encode(tag, &payload, into);
}

/// Raw output kept per PTY.
const RING: usize = 2 * 1024 * 1024;
/// Input accepted ahead of a child that is not reading.
const INPUT: usize = 1024 * 1024;
/// Frames queued for one reader before the ring is read again.
const READER_QUEUE: usize = 4 * wire::CHUNK;
/// How long a holder with no running child waits for a start.
#[cfg(not(test))]
const IDLE: Duration = Duration::from_secs(60);
#[cfg(test)]
thread_local! {
    /// A fixture's holder thread chooses how long it idles; two seconds suits
    /// the tests of leaving, and is too short for a slow store beside it.
    pub(crate) static TEST_IDLE: std::cell::Cell<Duration> =
        const { std::cell::Cell::new(Duration::from_secs(2)) };
}

/// Where one build's holder lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub socket: PathBuf,
    /// Held exclusively for the holder's life; only its holder touches `socket`.
    pub lock: PathBuf,
    /// `<terminal>/` directories: the holder writes `exit.json`, the server the rest.
    pub state: PathBuf,
}

impl Place {
    /// The place of the holder built from the executable hashing to `build`.
    pub fn of(build: &str) -> Result<Self> {
        let short = build.get(..12).context("holder build is a SHA-256")?;
        let private = crate::transport::private_directory()?;
        Ok(Self {
            socket: private.join(format!("holder-{short}.sock")),
            lock: private.join(format!("holder-{short}.lock")),
            state: crate::store::lf_home_dir().join("holders").join(short),
        })
    }

    pub fn terminal(&self, terminal: &str) -> PathBuf {
        self.state.join(terminal)
    }
}

/// SHA-256 of the running executable: what a holder is keyed by.
pub fn build() -> Result<String> {
    crate::installation::file_sha256(&std::env::current_exe()?.canonicalize()?)
}

/// What became of a `serve` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Served {
    /// Another holder of this build is alive; nothing was touched.
    AlreadyHeld,
    /// This holder ran until its last child was gone.
    Finished,
}

/// `lf holder run`: hold this build's PTYs until the last child is gone.
pub fn run() -> Result<()> {
    let build = build()?;
    serve(&Place::of(&build)?, &build, || {
        // The one line the starting server waits for.
        let _ = writeln!(io::stdout(), "{}", std::process::id());
        let _ = io::stdout().flush();
    })?;
    Ok(())
}

struct Terminal {
    request: String,
    pid: u32,
    rows: u16,
    cols: u16,
    /// Closed when the child exits.
    master: Option<File>,
    /// The slave side reported end of file before the child's wait returned.
    drained: bool,
    ring: VecDeque<u8>,
    written: u64,
    /// Positions where the PTY turned raw or cooked; the first entry is the
    /// state at or before the ring's start.
    modes: VecDeque<(u64, bool)>,
    /// The PTY's modes before the child ran.
    cooked: Option<libc::termios>,
    input: VecDeque<u8>,
    exit: Option<Exit>,
}

/// The PTY's attributes, when it still answers.
fn attributes(fd: RawFd) -> Option<libc::termios> {
    // SAFETY: tcgetattr writes into a live termios; the descriptor is borrowed.
    unsafe {
        let mut termios = std::mem::zeroed::<libc::termios>();
        (libc::tcgetattr(fd, &mut termios) == 0).then_some(termios)
    }
}

impl Terminal {
    /// Whether the PTY was raw for the byte at `at`, and where that changes.
    fn mode(&self, at: u64) -> (bool, Option<u64>) {
        let index = self
            .modes
            .iter()
            .rposition(|(from, _)| *from <= at)
            .unwrap_or(0);
        (
            self.modes[index].1,
            self.modes.get(index + 1).map(|(from, _)| *from),
        )
    }

    fn ring_start(&self) -> u64 {
        self.written - self.ring.len() as u64
    }

    fn push(&mut self, bytes: &[u8], raw: bool) {
        if self.modes.back().is_some_and(|(_, last)| *last != raw) {
            self.modes.push_back((self.written, raw));
        }
        self.written += bytes.len() as u64;
        self.ring.extend(bytes);
        let over = self.ring.len().saturating_sub(RING);
        self.ring.drain(..over);
        let start = self.ring_start();
        while self.modes.get(1).is_some_and(|(from, _)| *from <= start) {
            self.modes.pop_front();
        }
    }

    /// Up to `limit` bytes from position `at`, which the ring holds.
    fn slice(&self, at: u64, limit: usize) -> Vec<u8> {
        let start = (at - self.ring_start()) as usize;
        let end = self.ring.len().min(start + limit);
        self.ring.range(start..end).copied().collect()
    }

    /// Read what the child has written, at most `passes` buffers of it.
    fn drain(&mut self, passes: usize) {
        let mut bytes = [0; 16384];
        for _ in 0..passes {
            let Some(master) = self.master.as_mut() else {
                return;
            };
            match master.read(&mut bytes) {
                Ok(0) => {
                    self.drained = true;
                    return;
                }
                Ok(n) => {
                    let raw = attributes(master.as_raw_fd())
                        .is_some_and(|termios| termios.c_lflag & libc::ICANON == 0);
                    self.push(&bytes[..n], raw);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                // Linux reports a closed slave as EIO.
                Err(_) => {
                    self.drained = true;
                    return;
                }
            }
        }
    }

    fn forward_input(&mut self) {
        let Some(master) = self.master.as_mut() else {
            self.input.clear();
            return;
        };
        while !self.input.is_empty() {
            match master.write(self.input.as_slices().0) {
                Ok(0) => return,
                Ok(n) => {
                    self.input.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return,
            }
        }
    }

    fn held(&self, terminal: &str) -> Held {
        Held {
            terminal: terminal.to_owned(),
            request: self.request.clone(),
            pid: self.pid,
            rows: self.rows,
            cols: self.cols,
            written: self.written,
            ring_start: self.ring_start(),
            exit: self.exit.clone(),
        }
    }
}

enum Mode {
    Requests,
    Reader {
        terminal: String,
        at: u64,
        /// The mode last sent on this stream.
        raw: Option<bool>,
    },
    Writer {
        terminal: String,
    },
}

struct Connection {
    socket: UnixStream,
    received: Vec<u8>,
    output: VecDeque<u8>,
    mode: Mode,
    /// Close once `output` is written.
    closing: bool,
    closed: bool,
}

impl Connection {
    fn read(&mut self) {
        let mut bytes = [0; 16384];
        match self.socket.read(&mut bytes) {
            Ok(0) => self.closed = true,
            Ok(n) => self.received.extend_from_slice(&bytes[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => self.closed = true,
        }
    }

    fn flush(&mut self) {
        while !self.output.is_empty() {
            match self.socket.write(self.output.as_slices().0) {
                Ok(0) => {
                    self.closed = true;
                    return;
                }
                Ok(n) => {
                    self.output.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.closed = true;
                    return;
                }
            }
        }
        if self.closing {
            self.closed = true;
        }
    }
}

struct Holder<'a> {
    place: &'a Place,
    build: &'a str,
    terminals: BTreeMap<String, Terminal>,
    exits: mpsc::Sender<(String, ExitStatus)>,
    waker: UnixStream,
}

/// Claim this build's socket and hold PTYs until the last child is gone and no
/// start arrives for `IDLE`. `ready` runs once the socket accepts connections.
pub fn serve(place: &Place, build: &str, ready: impl FnOnce()) -> Result<Served> {
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(&place.lock)
        .with_context(|| format!("Open holder lock {}", place.lock.display()))?;
    // The kernel releases this lock when the process dies, however it dies. A
    // holder too starved to answer still holds it, so nothing takes its socket.
    if lock.try_lock_exclusive().is_err() {
        return Ok(Served::AlreadyHeld);
    }
    // Only the lock's holder unlinks: whatever is at this path is a dead holder's.
    match std::fs::remove_file(&place.socket) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    let listener = UnixListener::bind(&place.socket)
        .with_context(|| format!("Bind holder socket {}", place.socket.display()))?;
    std::fs::set_permissions(&place.socket, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let (wake, waker) = UnixStream::pair()?;
    wake.set_nonblocking(true)?;
    waker.set_nonblocking(true)?;
    let (exits, exited) = mpsc::channel();
    let mut holder = Holder {
        place,
        build,
        terminals: BTreeMap::new(),
        exits,
        waker,
    };
    ready();
    #[cfg(test)]
    #[allow(non_snake_case)]
    let IDLE = TEST_IDLE.get();
    let mut connections: Vec<Connection> = Vec::new();
    let mut idle_since = Some(Instant::now());
    loop {
        let timeout =
            idle_since.map(|since| (since + IDLE).saturating_duration_since(Instant::now()));
        let ready = poll(&listener, &wake, &connections, &holder.terminals, timeout);
        if ready.listener {
            while let Ok((socket, _)) = listener.accept() {
                if socket.set_nonblocking(true).is_ok() {
                    connections.push(Connection {
                        socket,
                        received: Vec::new(),
                        output: VecDeque::new(),
                        mode: Mode::Requests,
                        closing: false,
                        closed: false,
                    });
                }
            }
        }
        // Output first: a child never waits on a server, a reader or a request.
        for terminal in holder.terminals.values_mut() {
            if ready.readable.contains(&terminal_fd(terminal)) {
                terminal.drain(16);
            }
        }
        if ready.wake {
            let mut bytes = [0; 64];
            while matches!((&wake).read(&mut bytes), Ok(1..)) {}
        }
        while let Ok((terminal, status)) = exited.try_recv() {
            holder.finish(&terminal, status);
        }
        for connection in &mut connections {
            if ready.readable.contains(&connection.socket.as_raw_fd()) {
                connection.read();
            }
            holder.advance(connection);
        }
        for terminal in holder.terminals.values_mut() {
            terminal.forward_input();
        }
        for connection in &mut connections {
            holder.feed(connection);
            connection.flush();
        }
        connections.retain(|connection| !connection.closed);
        let running = holder
            .terminals
            .values()
            .any(|terminal| terminal.exit.is_none());
        idle_since = match (running, idle_since) {
            (true, _) => None,
            (false, None) => Some(Instant::now()),
            (false, since) => since,
        };
        if idle_since.is_some_and(|since| since.elapsed() >= IDLE) {
            let _ = std::fs::remove_file(&place.socket);
            return Ok(Served::Finished);
        }
    }
}

fn terminal_fd(terminal: &Terminal) -> RawFd {
    terminal.master.as_ref().map_or(-1, AsRawFd::as_raw_fd)
}

impl Holder<'_> {
    /// Act on what a connection has sent.
    fn advance(&mut self, connection: &mut Connection) {
        loop {
            match &connection.mode {
                // A reader sends nothing; whatever arrives is dropped.
                Mode::Reader { .. } => {
                    connection.received.clear();
                    return;
                }
                Mode::Writer { terminal } => {
                    let Some(held) = self
                        .terminals
                        .get_mut(terminal)
                        .filter(|held| held.master.is_some())
                    else {
                        connection.closed = true;
                        return;
                    };
                    held.input.extend(connection.received.drain(..));
                    return;
                }
                Mode::Requests => {}
            }
            let Some(end) = connection.received.iter().position(|byte| *byte == b'\n') else {
                if connection.received.len() > wire::MAX_LINE {
                    connection.closed = true;
                }
                return;
            };
            let line: Vec<u8> = connection.received.drain(..=end).collect();
            let answer = match serde_json::from_slice::<Request>(&line[..end]) {
                Ok(request) => self.answer(request, &mut connection.mode),
                Err(error) => Answer::Error {
                    message: format!("unreadable request: {error}"),
                },
            };
            connection.output.extend(wire::line(&answer));
        }
    }

    fn answer(&mut self, request: Request, mode: &mut Mode) -> Answer {
        let unknown = |terminal: &str| Answer::Error {
            message: format!("holder does not hold terminal {terminal}"),
        };
        match request {
            Request::Hello { revision } if revision < wire::REVISION => Answer::Incompatible {
                revision: wire::REVISION,
            },
            Request::Hello { .. } => Answer::Welcome {
                revision: wire::REVISION,
                build: self.build.to_owned(),
                pid: std::process::id(),
            },
            Request::List => Answer::Terminals {
                terminals: self
                    .terminals
                    .iter()
                    .map(|(name, terminal)| terminal.held(name))
                    .collect(),
            },
            Request::Start(start) => match self.start(start) {
                Ok(pid) => Answer::Started { pid },
                Err(error) => Answer::Error {
                    message: format!("{error:#}"),
                },
            },
            Request::Read { terminal, from } => match self.terminals.get(&terminal) {
                None => unknown(&terminal),
                // A position this PTY never reached belongs to another terminal.
                Some(held) if from > held.written => Answer::Error {
                    message: format!(
                        "terminal {terminal} has written {} bytes; cannot read from {from}",
                        held.written
                    ),
                },
                Some(_) => {
                    *mode = Mode::Reader {
                        terminal,
                        at: from,
                        raw: None,
                    };
                    Answer::Reading
                }
            },
            Request::Write { terminal } => match self.terminals.get(&terminal) {
                None => unknown(&terminal),
                Some(_) => {
                    *mode = Mode::Writer { terminal };
                    Answer::Writing
                }
            },
            Request::Resize {
                terminal,
                rows,
                cols,
            } => match self.terminals.get_mut(&terminal) {
                None => unknown(&terminal),
                Some(held) => {
                    held.rows = rows;
                    held.cols = cols;
                    if let Some(master) = &held.master {
                        let fd = master.as_raw_fd();
                        let _ = crate::transport::resize(fd, rows, cols);
                        // Resizing to the same dimensions sends no signal; ask
                        // the foreground program to repaint either way.
                        // SAFETY: the descriptor is this holder's PTY; only its
                        // foreground group receives the nonterminating signal.
                        unsafe {
                            let foreground = libc::tcgetpgrp(fd);
                            if foreground > 1 {
                                libc::kill(-foreground, libc::SIGWINCH);
                            }
                        }
                    }
                    Answer::Done
                }
            },
            Request::Restore { terminal } => match self.terminals.get(&terminal) {
                None => unknown(&terminal),
                Some(held) => {
                    if let (Some(master), Some(cooked)) = (&held.master, &held.cooked) {
                        // SAFETY: restores attributes read from this PTY before its child ran.
                        unsafe {
                            libc::tcsetattr(master.as_raw_fd(), libc::TCSANOW, cooked);
                        }
                    }
                    Answer::Done
                }
            },
            Request::Forget { terminal } => match self.terminals.get(&terminal) {
                None => Answer::Done,
                Some(held) if held.exit.is_none() => Answer::Error {
                    message: format!("terminal {terminal} is still running"),
                },
                Some(_) => {
                    self.terminals.remove(&terminal);
                    Answer::Done
                }
            },
        }
    }

    fn start(&mut self, start: Start) -> Result<u32> {
        anyhow::ensure!(
            !start.terminal.is_empty()
                && start.terminal.len() <= 64
                && start
                    .terminal
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'),
            "terminal id {:?} is not a plain name",
            start.terminal
        );
        if let Some(held) = self.terminals.get(&start.terminal) {
            anyhow::ensure!(
                held.request == start.request,
                "terminal {} was started by another request",
                start.terminal
            );
            return Ok(held.pid);
        }
        let program = start.argv.first().context("start has no command")?;
        let (master, slave) = crate::transport::terminal_pair()?;
        crate::transport::resize(master.as_raw_fd(), start.rows, start.cols)?;
        let cooked = attributes(master.as_raw_fd());
        let mut command = Command::new(program);
        command
            .args(&start.argv[1..])
            .env_clear()
            .envs(start.env.iter().map(|(name, value)| (name, value)))
            .current_dir(&start.cwd)
            .stdin(slave.try_clone()?)
            .stdout(slave.try_clone()?)
            .stderr(slave.try_clone()?);
        // SAFETY: these syscalls only affect the child; fd 0 is the slave PTY after dup2.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .with_context(|| format!("start {program}"))?;
        // Close every slave descriptor here so the master sees the child's end.
        drop(command);
        drop(slave);
        crate::transport::nonblocking(master.as_raw_fd())?;
        let pid = child.id();
        let exits = self.exits.clone();
        let waker = self.waker.try_clone()?;
        let name = start.terminal.clone();
        std::thread::spawn(move || {
            if let Ok(status) = child.wait() {
                let _ = exits.send((name, status));
                let _ = (&waker).write_all(&[1]);
            }
        });
        self.terminals.insert(
            start.terminal,
            Terminal {
                request: start.request,
                pid,
                rows: start.rows,
                cols: start.cols,
                master: Some(master),
                drained: false,
                ring: VecDeque::new(),
                written: 0,
                modes: VecDeque::from([(0, false)]),
                cooked,
                input: VecDeque::new(),
                exit: None,
            },
        );
        Ok(pid)
    }

    /// Record a child's exit: its last output, then `exit.json`, then readers.
    fn finish(&mut self, name: &str, status: ExitStatus) {
        let Some(terminal) = self.terminals.get_mut(name) else {
            return;
        };
        terminal.drain(RING / 16384 + 1);
        // Closing the master hangs up whatever the child left on the PTY.
        terminal.master = None;
        terminal.input.clear();
        let exit = Exit {
            code: status.code(),
            signal: status.signal(),
            at: time::OffsetDateTime::now_utc().unix_timestamp(),
            written: terminal.written,
        };
        if let Err(error) = record_exit(&self.place.terminal(name), &exit) {
            eprintln!("lf holder: cannot write exit of terminal {name}: {error:#}");
        }
        terminal.exit = Some(exit);
    }

    /// Queue a reader's next frames from its terminal's ring.
    fn feed(&mut self, connection: &mut Connection) {
        let Mode::Reader { terminal, at, raw } = &mut connection.mode else {
            return;
        };
        if connection.closing {
            return;
        }
        let Some(held) = self.terminals.get(terminal.as_str()) else {
            connection.closed = true;
            return;
        };
        if *at < held.ring_start() {
            let gap = Gap {
                ring_start: held.ring_start(),
            };
            json(Tag::Gap, &gap, &mut connection.output);
            *at = gap.ring_start;
        }
        while *at < held.written && connection.output.len() < READER_QUEUE {
            let (mode, until) = held.mode(*at);
            if *raw != Some(mode) {
                json(Tag::Mode, &wire::Mode { raw: mode }, &mut connection.output);
                *raw = Some(mode);
            }
            let limit = until.map_or(wire::CHUNK, |until| wire::CHUNK.min((until - *at) as usize));
            let chunk = held.slice(*at, limit);
            wire::encode(Tag::Out, &chunk, &mut connection.output);
            *at += chunk.len() as u64;
        }
        if let Some(exit) = held.exit.as_ref().filter(|_| *at == held.written) {
            json(Tag::Exit, exit, &mut connection.output);
            connection.closing = true;
        }
    }
}

/// Write `exit.json` whole or not at all.
fn record_exit(directory: &Path, exit: &Exit) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    let partial = directory.join("exit.json.partial");
    std::fs::write(&partial, serde_json::to_vec(exit)?)?;
    std::fs::rename(partial, directory.join("exit.json"))?;
    Ok(())
}

/// The exit a holder recorded for `terminal`, if its child has ended.
pub fn recorded_exit(place: &Place, terminal: &str) -> Result<Option<Exit>> {
    match std::fs::read(place.terminal(terminal).join("exit.json")) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

struct Ready {
    listener: bool,
    wake: bool,
    readable: Vec<RawFd>,
}

fn poll(
    listener: &UnixListener,
    wake: &UnixStream,
    connections: &[Connection],
    terminals: &BTreeMap<String, Terminal>,
    timeout: Option<Duration>,
) -> Ready {
    let pollfd = |fd: RawFd, events: libc::c_short| libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let mut fds = vec![
        pollfd(listener.as_raw_fd(), libc::POLLIN),
        pollfd(wake.as_raw_fd(), libc::POLLIN),
    ];
    for connection in connections {
        let mut events = 0;
        // A writer ahead of its child waits; everything else is read at once.
        let full = matches!(&connection.mode, Mode::Writer { terminal }
            if terminals.get(terminal).is_some_and(|held| held.input.len() >= INPUT));
        if !full {
            events |= libc::POLLIN;
        }
        if !connection.output.is_empty() {
            events |= libc::POLLOUT;
        }
        fds.push(pollfd(connection.socket.as_raw_fd(), events));
    }
    for terminal in terminals.values() {
        let Some(master) = &terminal.master else {
            continue;
        };
        let mut events = 0;
        if !terminal.drained {
            events |= libc::POLLIN;
        }
        if !terminal.input.is_empty() {
            events |= libc::POLLOUT;
        }
        if events != 0 {
            fds.push(pollfd(master.as_raw_fd(), events));
        }
    }
    let timeout = timeout.map_or(-1, |timeout| {
        libc::c_int::try_from(timeout.as_nanos().div_ceil(1_000_000)).unwrap_or(libc::c_int::MAX)
    });
    // SAFETY: poll reads a live array of exactly fds.len() descriptors.
    unsafe {
        libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout);
    }
    Ready {
        listener: fds[0].revents != 0,
        wake: fds[1].revents != 0,
        readable: fds[2..]
            .iter()
            .filter(|fd| fd.revents & !libc::POLLOUT != 0)
            .map(|fd| fd.fd)
            .collect(),
    }
}

#[cfg(test)]
mod tests;
