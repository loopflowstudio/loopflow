//! A terminal the Machine server's holder keeps hosts an execution, including
//! its capture and native client. Viewers hold revocable input tokens, never
//! provider attachment capabilities.

mod connection;
mod held;
mod hosted;
pub mod input;
mod lifetime;
mod native;
mod output;
pub mod registry;
pub use output::local_directory_report;
mod snapshot;
mod stream;
use loopflow_terminal::repaint as terminal;
#[cfg(test)]
mod viewer_tests;
pub mod wire;

use std::collections::VecDeque;
use std::ffi::CStr;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::harness::native_exit::{NativeExit, Settled};
use input::{Paste, PendingInput};
use loopflow_model::id::{LfSessionId, TerminalId};
use loopflow_storage::sqlite::ViewerHold;
use wire::{Close, CloseReason, Decoder, Event, Incoming, Outgoing, Reply, Request, Scanner, Tag};

/// The held terminal a direct child runs in.
const HELD_ENV: &str = "LF_TRANSPORT_HELD";
/// The held terminal a program runs in, inherited by everything it starts.
pub(crate) const WITHIN_ENV: &str = "LF_HELD_TERMINAL";
const PARENT_ENV: &str = "LF_TRANSPORT_PARENT";
const CONTROL_ENV: &str = "LF_TRANSPORT_CONTROL";
const LIMIT: usize = 4 * 1024 * 1024;
// Short liveness timers use uptime: laptop sleep is not a silent connection.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

pub const STREAM_ENV: &str = "LF_VIEWER_STREAM";

const ATTACH_TIMEOUT: Duration = Duration::from_secs(5);
// SSH connection setup and destination Session resolution precede attachment.
const COMMAND_ATTACH_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a person's terminal waits for exit records before it gets the
/// provider's own exit back. A locked or refused store never holds a quit.
const SETTLE_WAIT: Duration = Duration::from_millis(800);

/// How long a server that hosted an exit stays to finish its records.
const SETTLE_LINGER: Duration = Duration::from_secs(20);

/// The exit code a provider's status reports on its own.
pub(crate) fn provider_exit_code(status: std::process::ExitStatus) -> u8 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)) as u8
}

/// A handed-off exit being recorded off the host's loop. Its records wait on
/// the store; the person's terminal waits only `SETTLE_WAIT` for them, then
/// gets the provider's own exit back, or a move already judged by the host.
struct Settling {
    settled: mpsc::Receiver<Settled>,
    since: Instant,
    fallback: Settled,
    process: String,
    /// The terminal has its exit; only the records are still being written.
    delivered: bool,
}

impl Settling {
    fn spawn(
        record: Box<NativeExit>,
        status: std::process::ExitStatus,
        waker: UnixStream,
        departure: Option<Settled>,
    ) -> Self {
        let process = format!(
            "lf/{}",
            record
                .attachment
                .launcher_lf_process_id
                .as_str()
                .replace('-', "")
        );
        let (sender, settled) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(crate::harness::native_exit::settle(&record, status));
            let _ = (&waker).write_all(&[1]);
        });
        Self {
            settled,
            since: Instant::now(),
            fallback: departure.unwrap_or_else(|| Settled {
                code: provider_exit_code(status),
                moved: false,
                lines: Vec::new(),
            }),
            process,
            delivered: false,
        }
    }

    /// When the terminal stops waiting for records, while it still waits.
    fn deadline(&self) -> Option<Instant> {
        (!self.delivered).then(|| self.since + SETTLE_WAIT)
    }

    /// The settlement once its records are written, or the provider's own exit
    /// once the terminal has waited long enough for them. Delivered once.
    fn poll(&mut self) -> Option<Settled> {
        let deadline = self.deadline()?;
        let settled = match self.settled.try_recv() {
            Ok(settled) => settled,
            Err(mpsc::TryRecvError::Empty) if Instant::now() < deadline => return None,
            Err(_) => {
                tracing::warn!(process = self.process, "exit records are still being written; returning the terminal with its known outcome");
                Settled {
                    code: self.fallback.code,
                    moved: self.fallback.moved,
                    lines: std::mem::take(&mut self.fallback.lines),
                }
            }
        };
        self.delivered = true;
        Some(settled)
    }

    /// Give records still being written time to land before the host lets go.
    fn linger(self) {
        let _ = self.settled.recv_timeout(SETTLE_LINGER);
    }
}
static HANDOFF: OnceLock<Handoff> = OnceLock::new();

#[derive(Debug)]
struct Handoff {
    terminal: TerminalId,
    key: String,
    parent: u32,
}

impl Handoff {
    /// Read and remove the environment handoff; only a direct child adopts it.
    fn take() -> Option<Self> {
        let parent = std::env::var(PARENT_ENV)
            .ok()
            .and_then(|p| p.parse::<u32>().ok());
        let terminal = std::env::var(HELD_ENV)
            .ok()
            .and_then(|id| TerminalId::parse(&id).ok());
        let key = std::env::var(CONTROL_ENV).ok();
        std::env::remove_var(CONTROL_ENV);
        std::env::remove_var(PARENT_ENV);
        std::env::remove_var(HELD_ENV);
        // SAFETY: getppid has no preconditions.
        let parent = parent.filter(|p| *p == unsafe { libc::getppid() } as u32)?;
        Some(Self {
            terminal: terminal?,
            key: key?,
            parent,
        })
    }

    /// A control connection to this execution's terminal. A server of another
    /// build is a recording gap: its reports go unrecorded and the agent goes on.
    fn connect(&self) -> Result<UnixStream> {
        static GAP: LazyLock<Mutex<crate::store::RecordingGap>> = LazyLock::new(Default::default);
        let connected = registry::connect(&self.terminal);
        let mut gap = GAP.lock().expect("control gap lock poisoned");
        match &connected {
            Err(error) if error.is::<registry::OtherRevision>() => {
                gap.note("held terminal control", error)
            }
            Ok(_) => gap.recovered(),
            Err(_) => {}
        }
        connected
    }

    fn pass(&self, command: &mut Command, parent: u32) {
        command
            .env(HELD_ENV, self.terminal.as_str())
            .env(PARENT_ENV, parent.to_string())
            .env(CONTROL_ENV, &self.key);
    }
}

/// Consume the direct execution handoff. Provider tools and nested lf calls do
/// not inherit the right to run inside this execution's held terminal.
pub fn initialize() {
    crate::os_process::detached::initialize();
    if let Some(handoff) = Handoff::take() {
        let _ = HANDOFF.set(handoff);
    }
    take_open_receipt();
}

/// The held terminal this process runs in, when it took up a held handoff.
pub fn current() -> Option<&'static TerminalId> {
    HANDOFF.get().map(|handoff| &handoff.terminal)
}

/// The held terminal this process's work runs in. The holder sets it on
/// every start and descendants inherit it. Work started there holds it open.
pub fn held_in() -> Option<TerminalId> {
    std::env::var(WITHIN_ENV)
        .ok()
        .and_then(|id| TerminalId::parse(&id).ok())
}

/// Inputs that select the native client or stream driver before delegation.
#[derive(Debug, Clone, Copy)]
pub struct LaunchInput {
    pub batch: bool,
    pub interactive: bool,
    pub has_prompt: bool,
}

impl LaunchInput {
    /// Select the native client or stream driver from the invocation and terminal.
    pub fn mode(self) -> crate::harness::Mode {
        self.mode_with_tty(io::stdin().is_terminal() || io::stdout().is_terminal())
    }

    /// A direct command is a chat or batch in its caller's terminal or pipe,
    /// whatever its Task attribution; Task delivery's step is a worker and a
    /// held child takes up a work session.
    pub fn kind(self) -> crate::harness::LaunchKind {
        use crate::harness::{LaunchKind, Mode};

        if crate::journal::worker_step() {
            LaunchKind::Worker
        } else if current().is_some() {
            LaunchKind::WorkSession
        } else {
            match self.mode() {
                Mode::Interactive => LaunchKind::Chat,
                Mode::Headless => LaunchKind::Batch,
            }
        }
    }

    fn mode_with_tty(self, tty: bool) -> crate::harness::Mode {
        if !self.batch && (self.interactive || tty || !self.has_prompt) {
            crate::harness::Mode::Interactive
        } else {
            crate::harness::Mode::Headless
        }
    }
}

/// `lf task run` and `lf task auto on` leave their terminal: the Graph they start outlives the caller,
/// and `lf -b task run` waits for it. Returns whether a detached child ran the
/// command; in a held terminal, an already detached process or a Graph step it
/// runs here.
pub fn detach_current() -> Result<bool> {
    if cfg!(test)
        || current().is_some()
        || crate::os_process::detached::launched()
        || crate::journal::graph_step()
    {
        return Ok(false);
    }
    let mut argv: Vec<String> = std::env::args().collect();
    let cwd = admit_current(&mut argv)?;
    let code = crate::os_process::detached::run(&argv, &cwd, false, &|command| {
        // No terminal shows detached work; it claims none.
        command
            .env_remove(STREAM_ENV)
            .env_remove("LF_TERMINAL_ID")
            .env_remove("LF_TERMINAL_TTY");
    })?;
    if code != 0 {
        return Err(crate::process::CommandExit(code).into());
    }
    Ok(true)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ViewerOptions {
    pub json: bool,
    pub unfocused: bool,
}

/// Reopen a recorded Session as a chat or a work session. Only an interactive
/// Session has a last screen to preview. A held terminal decides again under
/// its own launch lock, so this process releases `launch_lock` before it
/// starts one. Returns the kind and the lock when the launch runs here, None
/// when a held terminal took it.
pub(crate) fn delegate_session(
    id: &str,
    viewer: ViewerOptions,
    preview: bool,
    launch_lock: File,
) -> Result<Option<(crate::harness::LaunchKind, File)>> {
    if let Some(kind) = local_session_kind(id) {
        return Ok(Some((kind, launch_lock)));
    }
    drop(launch_lock);
    let mut argv: Vec<String> = std::env::args().collect();
    if viewer.json {
        // The viewer owns this continuation, including an early detach.
        argv.retain(|arg| arg != "--json");
    }
    delegate(id, viewer, preview, argv)?;
    Ok(None)
}

/// Open the resulting Session after a fork or thread replacement, without
/// repeating the command that prepared it.
pub(crate) fn delegate_prepared_session(
    id: &str,
    viewer: ViewerOptions,
) -> Result<Option<crate::harness::LaunchKind>> {
    if let Some(kind) = local_session_kind(id) {
        return Ok(Some(kind));
    }
    delegate(
        id,
        viewer,
        true,
        vec![String::new(), "session".into(), "resume".into(), id.into()],
    )?;
    Ok(None)
}

/// Chats reopen as chats; bound and primary Sessions as work sessions. A held
/// child keeps its parent's choice. A relayed viewer (a Desktop pane or a
/// framed `lf --machine` stream) is no terminal a chat can own, so the holder
/// keeps what it opens.
fn session_kind(id: &str) -> crate::harness::LaunchKind {
    use crate::harness::LaunchKind;

    if current().is_some()
        || std::env::var_os("LF_SERVER_STATUS").is_some()
        || std::env::var_os(STREAM_ENV).is_some()
    {
        return LaunchKind::WorkSession;
    }
    static GAP: LazyLock<Mutex<crate::store::RecordingGap>> = LazyLock::new(Default::default);
    let bare = crate::journal::process_ledger().and_then(|store| {
        store
            .session(id)?
            .map(|session| store.is_bare(&session))
            .transpose()
    });
    let mut gap = GAP.lock().expect("resume gap lock poisoned");
    match &bare {
        Ok(_) => gap.recovered(),
        Err(error) => gap.note("resume lifetime", error),
    }
    if matches!(bare, Ok(Some(true))) {
        LaunchKind::Chat
    } else {
        LaunchKind::WorkSession
    }
}

/// Launch the Session in a held terminal and relay its view here.
fn delegate(id: &str, viewer: ViewerOptions, preview: bool, mut argv: Vec<String>) -> Result<()> {
    let preview_session = preview.then_some(id);
    let cwd = admit_current(&mut argv)?;
    let preview = if std::env::var_os(STREAM_ENV).is_some() {
        if let Some(screen) = preview_session.and_then(snapshot::last_screen) {
            io::stdout().write_all(&wire::frame(Tag::Snapshot, &screen))?;
        }
        None
    } else {
        ViewerTerminal::preview(preview_session)?
    };
    let session = preview_session.map(LfSessionId::from);
    let terminal = start_held(&argv, &cwd, session.as_ref(), &|_| {})?;
    relay_with_preview(
        &terminal,
        session.as_ref(),
        viewer.json.then_some(id),
        preview,
        viewer.unfocused,
    )
}

/// Only a work Session outside a holder or detached process needs delegation.
fn local_session_kind(id: &str) -> Option<crate::harness::LaunchKind> {
    let kind = session_kind(id);
    (cfg!(test)
        || current().is_some()
        || crate::os_process::detached::launched()
        || kind != crate::harness::LaunchKind::WorkSession)
        .then_some(kind)
}

/// Admit this invocation before another process takes over its work, and point
/// `argv` at this build. Returns the working directory the child starts in.
fn admit_current(argv: &mut [String]) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    crate::journal::admit_process(&cwd, &std::env::args().collect::<Vec<_>>());
    crate::journal::current_lf_process_id().context(
        "Execution requires an admitted invocation; check the process ledger and caller provenance",
    )?;
    argv[0] = std::env::current_exe()?.to_string_lossy().into_owned();
    Ok(cwd)
}

/// Start `argv` in a terminal this build's holder keeps, through the Machine
/// server. `inherit` edits the caller's environment for it. With no server it
/// fails.
pub fn start_held(
    argv: &[String],
    cwd: &Path,
    session: Option<&LfSessionId>,
    inherit: &dyn Fn(&mut Command),
) -> Result<TerminalId> {
    use crate::machine::wire::{ServerOperation, SessionStarted};
    let mut edits = Command::new(&argv[0]);
    inherit(&mut edits);
    let mut env: std::collections::BTreeMap<_, _> = std::env::vars_os().collect();
    for (name, value) in edits.get_envs() {
        match value {
            Some(value) => env.insert(name.to_owned(), value.to_owned()),
            None => env.remove(name),
        };
    }
    if let Some(receipt) = open_receipt() {
        env.insert(OPEN_RECEIPT_ENV.into(), receipt.into());
    }
    env.remove(std::ffi::OsStr::new(RELAY_FOCUSED_ENV));
    let env = env
        .into_iter()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect();
    let (rows, cols) = size();
    crate::harness::agent_process::require_server()?;
    let mut server = crate::machine::client::Connection::open()?;
    let answer = server.request(ServerOperation::SessionStart {
        session: session.cloned(),
        request: uuid::Uuid::new_v4().to_string(),
        argv: argv.to_vec(),
        cwd: cwd.to_path_buf(),
        env,
        rows,
        cols,
        placement: crate::session_record::current_terminal_id(),
    })?;
    let started: SessionStarted = serde_json::from_value(answer.result)
        .context("Loopflow server answered session.start without a terminal")?;
    Ok(started.terminal_id)
}

pub fn directory(home: &Path) -> Result<PathBuf> {
    // Keep sockaddr_un short even with a long LF_HOME or macOS temporary path.
    let home = home.canonicalize()?;
    let digest = hex::encode(Sha256::digest(home.as_os_str().as_encoded_bytes()));
    // SAFETY: geteuid has no preconditions.
    Ok(Path::new("/tmp").canonicalize()?.join(format!(
        "lf-{}-{}",
        unsafe { libc::geteuid() },
        &digest[..16]
    )))
}

pub(crate) fn private_directory() -> Result<PathBuf> {
    let dir = directory(&crate::store::lf_home_dir())?;
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::symlink_metadata(&dir)?;
            // SAFETY: geteuid has no preconditions.
            anyhow::ensure!(
                metadata.is_dir()
                    && metadata.uid() == unsafe { libc::geteuid() }
                    && metadata.permissions().mode() & 0o077 == 0,
                "Transport directory is not private"
            );
        }
        Err(e) => return Err(e.into()),
    }
    Ok(dir)
}

pub fn nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl operates on a borrowed live descriptor without taking ownership.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// PTYs allocated on this Machine against its limit. Every held terminal
/// takes one, as does every terminal tab; they run out before anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtyUsage {
    pub in_use: u64,
    pub max: u64,
    /// The setting that holds `max`.
    pub limit: &'static str,
}

#[cfg(target_os = "macos")]
pub fn pty_usage() -> Option<PtyUsage> {
    let mut max: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    // SAFETY: the name is NUL-terminated, and `max` and `size` are live
    // locals describing one writable `c_int`.
    let read = unsafe {
        libc::sysctlbyname(
            c"kern.tty.ptmx_max".as_ptr(),
            std::ptr::from_mut(&mut max).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    let max = u64::try_from(max).ok().filter(|_| read == 0)?;
    // devfs lists a `ttysNNN` node for each PTY while it is allocated.
    let in_use = std::fs::read_dir("/dev")
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name.strip_prefix("ttys")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            })
        })
        .count() as u64;
    Some(PtyUsage {
        in_use,
        max,
        limit: "kern.tty.ptmx_max",
    })
}

#[cfg(not(target_os = "macos"))]
pub fn pty_usage() -> Option<PtyUsage> {
    let read = |path: &str| std::fs::read_to_string(path).ok()?.trim().parse().ok();
    Some(PtyUsage {
        in_use: read("/proc/sys/kernel/pty/nr")?,
        max: read("/proc/sys/kernel/pty/max")?,
        limit: "kernel.pty.max",
    })
}

/// Why a held terminal got no PTY, as its conversation's start failure
/// reads: the Machine has none left, or the OS error beside what is in use.
pub(crate) fn pty_failure(error: &io::Error, usage: Option<PtyUsage>) -> String {
    match usage {
        Some(PtyUsage { in_use, max, limit }) if in_use >= max => {
            format!("no PTY left: {in_use} in use ({limit})")
        }
        Some(PtyUsage { in_use, max, limit }) => {
            format!("cannot open a PTY: {error} ({in_use} of {max} in use, {limit})")
        }
        None => format!("cannot open a PTY: {error}"),
    }
}

pub(crate) fn terminal_pair() -> io::Result<(File, File)> {
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: openpty initializes both descriptors; pointers reference live storage.
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut size,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: these are the descriptors just returned by openpty. Prevent the
    // executed command from inheriting the master or spare slave.
    unsafe {
        libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(slave, libc::F_SETFD, libc::FD_CLOEXEC);
    }
    // SAFETY: successful openpty returned two newly owned descriptors.
    Ok(unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) })
}

fn dimensions(rows: u16, cols: u16) -> (u16, u16) {
    // An unopened terminal may report zero. A held terminal needs room for
    // a wide glyph and a previous row when wrapping, even in a collapsed pane.
    (
        if rows == 0 { 24 } else { rows.clamp(2, 500) },
        if cols == 0 { 80 } else { cols.clamp(2, 500) },
    )
}

pub fn resize(fd: RawFd, rows: u16, cols: u16) -> io::Result<()> {
    let size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: ioctl borrows a valid descriptor and a correctly sized winsize.
    if unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &size) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Resize the PTY at `fd` and ask its foreground program to repaint: resizing
/// to the same dimensions sends no signal.
pub(crate) fn resize_foreground(fd: RawFd, rows: u16, cols: u16) -> io::Result<()> {
    resize(fd, rows, cols)?;
    // SAFETY: the descriptor is a PTY this process holds; only its current
    // foreground group receives the nonterminating signal.
    unsafe {
        let foreground = libc::tcgetpgrp(fd);
        if foreground > 1 {
            libc::kill(-foreground, libc::SIGWINCH);
        }
    }
    Ok(())
}

/// Whether the terminal at `fd` is raw (no line editing).
pub(crate) fn is_raw(fd: RawFd) -> bool {
    // SAFETY: tcgetattr writes into a live termios; the descriptor is borrowed.
    unsafe {
        let mut termios = std::mem::zeroed::<libc::termios>();
        libc::tcgetattr(fd, &mut termios) == 0 && termios.c_lflag & libc::ICANON == 0
    }
}

#[derive(Debug)]
struct Viewer {
    socket: UnixStream,
    /// The peer stopped reading; its frames still queued are read out until
    /// end of stream, and nothing more is written.
    draining: bool,
    decoder: Decoder,
    /// Encoded frames not yet written.
    output: VecDeque<u8>,
    /// Public identity, named to others when it sizes the terminal.
    id: String,
    /// Tags this viewer's events; a program printing the same bytes lacks it.
    nonce: String,
    attached: bool,
    /// When this view last received input or focus, on its terminal's
    /// clock; the most recent live view sizes the terminal.
    active: u64,
    /// The controls are still to be shown to this view.
    wants_hint: bool,
    ready: bool,
    heartbeat_at: Instant,
    seq: u64,
    /// Events awaiting a sequence boundary in the stream.
    events: Vec<Vec<u8>>,
    size: (u16, u16),
    theme: Option<serde_json::Value>,
    name: Option<String>,
    leaving: bool,
    /// The `native_ended` request awaiting its acknowledgement.
    ending: Option<u64>,
    input_hint: Option<String>,
    /// The PTY position this viewer's stream has reached.
    position: u64,
    /// The position an attachment asked to continue from.
    from: Option<u64>,
    /// Behind or ahead of the live stream: it receives only the PTY bytes
    /// that bring it to the live position, then the current state again.
    catching_up: bool,
    catch_up: Option<held::CatchUp>,
}

impl Viewer {
    fn new(socket: UnixStream) -> Self {
        let token = || uuid::Uuid::new_v4().simple().to_string();
        Self {
            socket,
            draining: false,
            decoder: Decoder::default(),
            output: VecDeque::new(),
            id: token()[..12].to_owned(),
            nonce: token()[..16].to_owned(),
            attached: false,
            active: 0,
            wants_hint: false,
            ready: false,
            heartbeat_at: Instant::now(),
            seq: 0,
            events: Vec::new(),
            size: (24, 80),
            theme: None,
            name: None,
            leaving: false,
            ending: None,
            input_hint: None,
            position: 0,
            from: None,
            catching_up: false,
            catch_up: None,
        }
    }

    fn live(&self) -> bool {
        self.attached && !self.leaving
    }

    /// An attached viewer that stopped answering for `HEARTBEAT_TIMEOUT` of
    /// uptime is gone.
    fn heartbeat_expired(&self, now: Instant) -> bool {
        self.live() && now.saturating_duration_since(self.heartbeat_at) >= HEARTBEAT_TIMEOUT
    }

    /// Append `bytes` to this viewer's stream. A view that reconnects
    /// continues from the holder's ring instead.
    fn stream(&mut self, tag: Tag, bytes: &[u8]) -> io::Result<()> {
        for chunk in bytes.chunks(wire::CHUNK) {
            if !self.draining {
                wire::encode(tag, chunk, &mut self.output);
            }
        }
        if self.output.len() > LIMIT {
            return Err(io::Error::other("Viewer fell behind"));
        }
        Ok(())
    }

    fn event(&mut self, event: Event) {
        self.seq += 1;
        self.events.push(wire::event(&self.nonce, self.seq, event));
    }

    /// Place queued events on the stream.
    /// lf's own reports and events travel outside the PTY's positions.
    fn flush_events(&mut self) -> io::Result<()> {
        for bytes in std::mem::take(&mut self.events) {
            self.stream(Tag::Aside, &bytes)?;
        }
        Ok(())
    }

    /// Tell this viewer its stream now stands at PTY byte `at`.
    fn place(&mut self, at: u64) {
        self.position = at;
        if !self.draining {
            wire::encode(Tag::Position, &wire::position(at), &mut self.output);
        }
    }

    /// Whether lf's reports and events may reach this viewer now. One
    /// catching up receives them once it reaches the live stream.
    fn current(&self) -> bool {
        self.attached && !self.catching_up
    }

    fn reply(&mut self, answers: u64, reply: Reply) {
        let json = serde_json::to_vec(&Outgoing {
            answers,
            seq: self.seq,
            reply,
        })
        .expect("replies serialize");
        wire::encode(Tag::Reply, &json, &mut self.output);
    }

    fn attached_reply(&self, owner: Option<&str>) -> Reply {
        Reply::Attached {
            viewer: self.id.clone(),
            nonce: self.nonce.clone(),
            terminal: true,
            owner: owner.map(str::to_owned),
        }
    }

    /// Write queued frames. A peer that stopped reading may still have sent
    /// input, so its connection drains instead of failing.
    fn flush(&mut self) -> io::Result<()> {
        match self.write_output() {
            Err(error) if lost(&error) => {
                tracing::debug!(%error, viewer = %self.id, "viewer stopped reading; draining its input");
                self.output.clear();
                self.draining = true;
                Ok(())
            }
            written => written,
        }
    }

    fn write_output(&mut self) -> io::Result<()> {
        if self.draining {
            self.output.clear();
            return Ok(());
        }
        while !self.output.is_empty() {
            match self.socket.write(self.output.as_slices().0) {
                Ok(0) => return Err(io::ErrorKind::BrokenPipe.into()),
                Ok(n) => {
                    self.output.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Read into the frame decoder; a closed connection is `UnexpectedEof`.
    fn read(&mut self) -> io::Result<()> {
        let mut bytes = [0; 8192];
        match self.socket.read(&mut bytes) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => self.decoder.extend(&bytes[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }
}

fn lost(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::UnexpectedEof
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
    )
}

/// Whose size and colors the terminal takes: the live view that most recently
/// received input or focus. When it leaves, the next most recent does.
#[derive(Debug, Default)]
struct Sizing {
    owner: Option<String>,
    clock: u64,
}

impl Sizing {
    fn id(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    fn held_by(&self, viewer: &Viewer) -> bool {
        self.id() == Some(&viewer.id)
    }

    fn attach(&mut self, viewer: &mut Viewer, focused: bool) {
        if focused {
            self.touch(viewer);
        } else if self.owner.is_none() {
            self.owner = Some(viewer.id.clone());
        }
    }

    /// Input or focus makes this view the owner immediately, including for the next frame.
    fn touch(&mut self, viewer: &mut Viewer) {
        self.clock += 1;
        viewer.active = self.clock;
        if !self.held_by(viewer) {
            self.owner = Some(viewer.id.clone());
        }
    }

    /// A live owner keeps the terminal until another view is used or this one leaves.
    fn settle(&mut self, viewers: &[Viewer]) {
        if viewers
            .iter()
            .any(|viewer| viewer.live() && self.held_by(viewer))
        {
            return;
        }
        self.owner = viewers
            .iter()
            .rev()
            .filter(|viewer| viewer.live())
            .max_by_key(|viewer| viewer.active)
            .map(|viewer| viewer.id.clone());
    }
}

fn drain(wake: &UnixStream) {
    let mut bytes = [0; 64];
    while matches!((&*wake).read(&mut bytes), Ok(1..)) {}
}

/// Block until one of `fds` has work or `timeout` passes.
fn wait_for_activity(fds: &mut [libc::pollfd], timeout: Option<Duration>) {
    let timeout = timeout.map_or(-1, |timeout| {
        // Round up so the last fraction of a millisecond does not spin.
        let millis = timeout.as_nanos().div_ceil(1_000_000);
        libc::c_int::try_from(millis).unwrap_or(libc::c_int::MAX)
    });
    // SAFETY: poll reads a live array of exactly fds.len() descriptors.
    unsafe {
        libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout);
    }
}

/// Only connected viewers count. A lost stream enters the ordinary absence
/// window immediately.
fn viewer_hold(viewers: &[Viewer], previous: Option<ViewerHold>, now: i64) -> Option<ViewerHold> {
    if viewers.iter().any(Viewer::live) {
        match previous {
            Some(ViewerHold::Viewed(at)) if now < at + 10 => previous,
            _ => Some(ViewerHold::Viewed(now)),
        }
    } else {
        match previous {
            Some(ViewerHold::Viewed(_)) => Some(ViewerHold::Absent(now)),
            _ => previous,
        }
    }
}

/// Ending execution releases its remaining viewers without restarting an
/// absence clock that already ran while requested work finished.
fn release_viewer_hold(terminal: &TerminalId, previous: Option<ViewerHold>) {
    let hold = viewer_hold(
        &[],
        previous,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    );
    let recorded =
        crate::journal::process_ledger().and_then(|store| store.record_viewer_hold(terminal, hold));
    if let Err(error) = recorded {
        tracing::warn!(%error, %terminal, "cannot record Session viewer release; last observation expires");
    }
}

/// Send the ending event even when no more output can complete a sequence.
/// A failed viewer must not prevent the others from receiving it.
fn end_viewers(viewers: &mut [Viewer], event: Event) {
    for viewer in viewers.iter_mut().filter(|viewer| viewer.attached) {
        viewer.event(event.clone());
        let _ = viewer.flush_events();
        let _ = viewer.flush();
    }
}

/// End the protocol outside the terminal stream, then give queued output a
/// bounded chance to drain. Slow or lost connections still end with unknown EOF.
fn close_viewers(viewers: &mut [Viewer], reason: CloseReason) {
    let payload = serde_json::to_vec(&Close { reason }).expect("close reason serializes");
    for viewer in viewers.iter_mut().filter(|v| v.attached && !v.draining) {
        let _ = viewer.flush_events();
        wire::encode(Tag::Close, &payload, &mut viewer.output);
    }
    let deadline = Instant::now() + Duration::from_millis(200);
    loop {
        let mut pending = Vec::new();
        for viewer in viewers.iter_mut().filter(|v| v.attached) {
            if viewer.flush().is_err() {
                viewer.output.clear();
                viewer.draining = true;
            }
            if !viewer.output.is_empty() {
                pending.push(libc::pollfd {
                    fd: viewer.socket.as_raw_fd(),
                    events: libc::POLLOUT,
                    revents: 0,
                });
            }
        }
        if pending.is_empty() || Instant::now() >= deadline {
            break;
        }
        let timeout = deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as i32;
        // SAFETY: poll borrows the live sockets for this call.
        unsafe {
            libc::poll(pending.as_mut_ptr(), pending.len() as _, timeout);
        }
    }
    for viewer in viewers.iter_mut().filter(|v| v.attached) {
        let _ = viewer.socket.shutdown(std::net::Shutdown::Both);
    }
}

/// Send reports the stream can carry now, and any directory they changed.
fn publish_reports(output: &mut output::Output, viewers: &mut Vec<Viewer>) {
    if let Some(reports) = output.take_reports() {
        viewers
            .retain_mut(|viewer| !viewer.current() || viewer.stream(Tag::Aside, &reports).is_ok());
    }
    if let Some(cwd) = output.take_changed_directory() {
        for viewer in viewers.iter_mut().filter(|viewer| viewer.attached) {
            viewer.event(Event::Directory { cwd: cwd.clone() });
        }
    }
}

/// Events enter a stream only between complete terminal sequences, so a viewer
/// feeding the raw stream to a terminal never sees one split. After the
/// execution has ended nothing more completes, so they go at once.
fn place_events(output: &output::Output, viewers: &mut Vec<Viewer>, announced: bool) {
    let ready = output.ready() || announced;
    viewers.retain_mut(|viewer| {
        // Pending events and replies consume the same bounded queue as output,
        // including while the program leaves a terminal sequence unfinished.
        if viewer.output.len() + viewer.events.iter().map(Vec::len).sum::<usize>() > LIMIT {
            return false;
        }
        !ready
            || viewer.catching_up
            || viewer.events.is_empty()
            || viewer.flush_events().and_then(|()| viewer.flush()).is_ok()
    });
}

/// Where delivered bytes stand in the PTY stream.
#[derive(Debug, Clone, Copy)]
enum Span {
    /// Exactly the PTY's output from `start` to `end`.
    Pty { start: u64, end: u64 },
    /// lf's own bytes, after which viewers stand at `through` when it is set:
    /// a native client's replacement for output it withheld, settled lines.
    Aside { through: Option<u64> },
}

/// Feed `bytes` to the retained screen and every current viewer. `answering`
/// says the screen answers the program's queries: no viewer's terminal
/// receiving this output will.
fn deliver(
    output: &mut output::Output,
    viewers: &mut Vec<Viewer>,
    pending_input: &mut PendingInput,
    stream: u8,
    bytes: &[u8],
    answering: bool,
    span: Span,
) {
    if bytes.is_empty() {
        return;
    }
    let replies = output.process(stream, bytes, answering);
    pending_input.push(&replies);
    let reports = output.take_reports();
    viewers.retain_mut(|viewer| {
        !viewer.current() || {
            let written = (|| {
                match span {
                    Span::Pty { start, end } => {
                        // Output the ring dropped before this server read it
                        // never reaches the viewer; its program is asked to
                        // repaint.
                        if viewer.position != start {
                            viewer.place(start);
                        }
                        viewer.stream(Tag::Out, bytes)?;
                        viewer.position = end;
                    }
                    Span::Aside { through } => {
                        viewer.stream(Tag::Aside, bytes)?;
                        if let Some(through) = through.filter(|through| *through != viewer.position)
                        {
                            viewer.place(through);
                        }
                    }
                }
                if let Some(reports) = &reports {
                    viewer.stream(Tag::Aside, reports)?;
                }
                viewer.flush()
            })();
            written.is_ok()
        }
    });
    place_events(output, viewers, false);
}

fn size() -> (u16, u16) {
    let mut size = libc::winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: ioctl writes into a live winsize; failure leaves the defaults.
    unsafe {
        libc::ioctl(0, libc::TIOCGWINSZ, &mut size);
    }
    dimensions(size.ws_row, size.ws_col)
}

struct ViewerTerminal {
    state: Arc<Mutex<ViewerState>>,
}

struct ViewerState {
    // ViewerTerminal restores modes before this announces any diagnostics.
    output: crate::terminal_output::TerminalOutput,
    original: Option<libc::termios>,
    alternate: terminal::AlternateScreen,
    restored: bool,
    /// This viewer took over a caller's terminal and reported a directory.
    retire_directory: bool,
    /// The terminal reports focus to this view (DEC mode 1004), even after
    /// the agent turns its own focus reports off.
    reports_focus: bool,
}

impl ViewerState {
    /// The held terminal's output stream `stream` here: stdout's goes to
    /// stderr while stdout holds a JSON record.
    fn screen(&mut self, stream: u8) -> &mut std::fs::File {
        if stream == 2 || STDOUT_HOLDS_RECORD.load(Ordering::Relaxed) {
            &mut self.output.stderr
        } else {
            &mut self.output.stdout
        }
    }

    fn reset_display(&mut self) -> io::Result<()> {
        // Kitty keyboard state belongs to each screen; reset both when leaving.
        self.output.stdout.write_all(terminal::RESET_MODES)?;
        if self.alternate.entered {
            self.output.stdout.write_all(b"\x1b[?1049l")?;
            self.alternate.entered = false;
            self.output.stdout.write_all(terminal::RESET_MODES)?;
        }
        self.output.stdout.write_all(terminal::RESET_DRAWING)
    }

    fn restore(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        if self.output.stdout.is_terminal() {
            let _ = self.reset_display();
            if self.retire_directory {
                // The caller's shell owns this terminal again. An empty report
                // retires ours, so an unreported `cd` is not hidden behind it.
                let _ = self.output.stdout.write_all(b"\x1b]7;\x1b\\");
            }
        }
        if let Some(original) = self.original {
            // Discard input the viewer never forwarded, including terminal reports.
            // SAFETY: restores attributes previously read from stdin.
            unsafe { libc::tcsetattr(0, libc::TCSAFLUSH, &original) };
        }
    }
}

impl ViewerTerminal {
    fn enter() -> Result<Self> {
        let original = if io::stdin().is_terminal() {
            // SAFETY: termios is initialized by tcgetattr before it is read.
            Some(unsafe {
                let mut value = std::mem::zeroed();
                if libc::tcgetattr(0, &mut value) < 0 {
                    return Err(io::Error::last_os_error().into());
                }
                value
            })
        } else {
            None
        };
        let output = crate::terminal_output::TerminalOutput::enter()?;
        let state = Arc::new(Mutex::new(ViewerState {
            output,
            original,
            alternate: Default::default(),
            restored: false,
            retire_directory: false,
            reports_focus: false,
        }));
        let cleanup = Arc::downgrade(&state);
        crate::agent::register_interrupt_cleanup(move || {
            if let Some(state) = cleanup.upgrade() {
                state
                    .lock()
                    .expect("viewer terminal lock poisoned")
                    .restore();
            }
        });
        Ok(Self { state })
    }

    fn preview(session: Option<&str>) -> Result<Option<Self>> {
        if !io::stdout().is_terminal() {
            return Ok(None);
        }
        let Some(bytes) = session.and_then(snapshot::last_screen) else {
            return Ok(None);
        };
        let mut terminal = Self::enter()?;
        terminal.write(1, &bytes)?;
        // Keep cooked input until acknowledgement, so Ctrl+C still cancels startup.
        Ok(Some(terminal))
    }

    fn make_raw(&self) -> Result<()> {
        let state = self.state.lock().expect("viewer terminal lock poisoned");
        if let Some(mut raw) = state.original {
            // SAFETY: raw was read from stdin; only terminal attributes change.
            unsafe {
                libc::cfmakeraw(&mut raw);
                if libc::tcsetattr(0, libc::TCSANOW, &raw) < 0 {
                    return Err(io::Error::last_os_error().into());
                }
            }
        }
        Ok(())
    }

    fn retire_directory_on_restore(&self) {
        self.state
            .lock()
            .expect("viewer terminal lock poisoned")
            .retire_directory = true;
    }

    /// Ask the terminal to report focus in and out; restoring it turns that off.
    fn report_focus(&mut self) -> io::Result<()> {
        let mut state = self.state.lock().expect("viewer terminal lock poisoned");
        state.reports_focus = true;
        state.screen(1).write_all(FOCUS_ON)
    }

    fn handoff(&mut self) -> io::Result<()> {
        let mut state = self.state.lock().expect("viewer terminal lock poisoned");
        state.reset_display()?;
        // The live terminal is authoritative even when its first screen is blank.
        state.screen(1).write_all(b"\x1b[H\x1b[2J")
    }

    fn write(&mut self, stream: u8, bytes: &[u8]) -> io::Result<()> {
        // An interrupt cannot restore the terminal midway through an output write.
        let mut state = self.state.lock().expect("viewer terminal lock poisoned");
        state.alternate.observe(bytes);
        state.screen(stream).write_all(bytes)?;
        // An agent that stops its own focus reports leaves this view's on.
        if stream != 2 && state.reports_focus && contains(bytes, b"\x1b[?1004l") {
            state.screen(stream).write_all(FOCUS_ON)?;
        }
        Ok(())
    }
}
const FOCUS_ON: &[u8] = b"\x1b[?1004h";

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle)
}

/// Input on either side of a focus report must reach the holder in that order.
fn focus_reports(mut bytes: &[u8]) -> impl Iterator<Item = (&[u8], Option<bool>)> {
    std::iter::from_fn(move || {
        if bytes.is_empty() {
            return None;
        }
        for (at, sequence) in bytes.windows(3).enumerate() {
            if sequence == b"\x1b[I" || sequence == b"\x1b[O" {
                let input = &bytes[..at];
                bytes = &bytes[at + 3..];
                return Some((input, Some(sequence[2] == b'I')));
            }
        }
        Some((std::mem::take(&mut bytes), None))
    })
}

impl Drop for ViewerTerminal {
    fn drop(&mut self) {
        self.state
            .lock()
            .expect("viewer terminal lock poisoned")
            .restore();
    }
}

fn send(socket: &mut impl Write, request: Request) -> Result<()> {
    let bytes = serde_json::to_vec(&Incoming { id: 1, request })?;
    send_frame(socket, Tag::Request, &bytes)
}

fn send_frame(socket: &mut impl Write, tag: Tag, payload: &[u8]) -> Result<()> {
    // A write timeout reports EAGAIN on macOS, which names no cause.
    socket
        .write_all(&wire::frame(tag, payload))
        .map_err(|error| match error.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => anyhow::anyhow!(
                "Held terminal stopped reading requests; outcome is unknown and later input was not delivered"
            ),
            _ => error.into(),
        })
}

/// This viewer's terminal device, named to others when it takes ownership.
pub(crate) fn tty_name(fd: RawFd) -> Option<String> {
    // SAFETY: ttyname returns null or a pointer to a static NUL-terminated name.
    unsafe {
        let name = libc::ttyname(fd);
        (!name.is_null()).then(|| CStr::from_ptr(name).to_string_lossy().into_owned())
    }
}

/// Attach this terminal as one more view: every view types, and other views
/// stay.
///
/// The process that resolved the connection built it with a store, a command
/// tree and often a runtime; none of that serves a relay. It carries its
/// Process receipt into the relay, which opens no store until it returns.
pub fn attach(id: &TerminalId, session: Option<&LfSessionId>, viewer: ViewerOptions) -> Result<()> {
    relay_with_preview(
        id,
        session,
        session.filter(|_| viewer.json).map(LfSessionId::as_str),
        None,
        viewer.unfocused,
    )
}

fn relay_with_preview(
    id: &TerminalId,
    session: Option<&LfSessionId>,
    json_session: Option<&str>,
    terminal: Option<ViewerTerminal>,
    unfocused: bool,
) -> Result<()> {
    let focused = initial_focus(unfocused);
    let mut command = relay_command(std::env::current_exe()?, id, session, focused);
    if let Some(terminal) = &terminal {
        let state = terminal
            .state
            .lock()
            .expect("viewer terminal lock poisoned");
        // Preview input is still cooked. Carry its display and original output
        // descriptors into the relay without resetting the screen across exec.
        state.output.configure_child(&mut command)?;
        command.env(
            RELAY_PREVIEW_ENV,
            if state.alternate.entered {
                "alternate"
            } else {
                "primary"
            },
        );
    }
    if let Some(session) = json_session {
        command.args([RELAY_JSON_FLAG, session]);
    }
    crate::journal::carry_across_exec(&mut command)?;
    pass_open(&mut command);
    let error = command.exec();
    tracing::warn!(%error, "cannot exec the terminal relay; relaying here");
    attach_with_terminal(id, session, terminal, &mut RelayInput::new(focused))
}

const RELAY_FOCUSED_ENV: &str = "LF_RELAY_FOCUSED";

fn initial_focus(unfocused: bool) -> bool {
    !unfocused
        && std::env::var(RELAY_FOCUSED_ENV).as_deref() != Ok("0")
        && io::stdin().is_terminal()
}

const RELAY_PREVIEW_ENV: &str = "LF_RELAY_PREVIEW";
const RELAY_COMMAND: [&str; 2] = ["session", "attach"];
const RELAY_TERMINAL_FLAG: &str = "--terminal";
const RELAY_SESSION_FLAG: &str = "--session";
const RELAY_JSON_FLAG: &str = "--session-json";

// Keep the argv readable by older installed relays during a build handoff.
fn relay_command(
    lf: PathBuf,
    terminal: &TerminalId,
    session: Option<&LfSessionId>,
    focused: bool,
) -> Command {
    let mut command = Command::new(lf);
    command
        .env(RELAY_FOCUSED_ENV, if focused { "1" } else { "0" })
        .args(RELAY_COMMAND)
        .args([RELAY_TERMINAL_FLAG, terminal.as_str()]);
    if let Some(session) = session {
        command.args([RELAY_SESSION_FLAG, session.as_str()]);
    }
    command
}

/// What a relay invocation names: `lf session attach --terminal TERMINAL
/// [--session SESSION] [--session-json SESSION]`, the exact held terminal and
/// the Session a server bridge attaches by. Recognized before command parsing.
#[derive(Debug)]
pub struct Relay {
    pub terminal: TerminalId,
    pub session: Option<LfSessionId>,
    pub json_session: Option<String>,
}

pub fn relay_args(args: &[String]) -> Option<Relay> {
    let [flag, terminal, rest @ ..] = args else {
        return None;
    };
    if flag != RELAY_TERMINAL_FLAG {
        return None;
    }
    let terminal = TerminalId::parse(terminal).ok()?;
    let mut session = None;
    let mut json_session = None;
    for pair in rest.chunks(2) {
        match pair {
            [flag, value] if flag == RELAY_SESSION_FLAG && session.is_none() => {
                session = Some(LfSessionId::from(value.as_str()));
            }
            [flag, value] if flag == RELAY_JSON_FLAG && json_session.is_none() => {
                json_session = Some(value.clone());
            }
            _ => return None,
        }
    }
    Some(Relay {
        terminal,
        session,
        json_session,
    })
}

/// Paint an ended Desktop shell's retained screen without starting execution.
pub(crate) fn shell_history(terminal: &TerminalId) -> Result<()> {
    if let Some(screen) = snapshot::last_screen(terminal.as_str()) {
        if std::env::var_os(STREAM_ENV).is_some() {
            io::stdout().write_all(&wire::frame(Tag::Snapshot, &screen))?;
        } else {
            io::stdout().write_all(&screen)?;
        }
    }
    Ok(())
}

/// A `--json` relay whose stdout is no terminal keeps stdout for the Session
/// record printed after relaying; the held terminal's bytes go to stderr.
static STDOUT_HOLDS_RECORD: AtomicBool = AtomicBool::new(false);

/// Relay this terminal to a held terminal: socket in, terminal out.
pub fn relay(id: &TerminalId, session: Option<&LfSessionId>, json: bool) -> Result<()> {
    if std::env::var_os(STREAM_ENV).is_some() {
        return stream::bridge(id);
    }
    if json && !io::stdout().is_terminal() {
        STDOUT_HOLDS_RECORD.store(true, Ordering::Relaxed);
    }
    let preview = std::env::var(RELAY_PREVIEW_ENV).ok();
    std::env::remove_var(RELAY_PREVIEW_ENV);
    let terminal = if let Some(screen) = preview {
        let terminal = ViewerTerminal::enter()?;
        terminal
            .state
            .lock()
            .expect("viewer terminal lock poisoned")
            .alternate
            .entered = screen == "alternate";
        Some(terminal)
    } else {
        None
    };
    let mut input = RelayInput::new(initial_focus(false));
    std::env::remove_var(RELAY_FOCUSED_ENV);
    attach_with_terminal(id, session, terminal, &mut input)?;
    // A delegated launch that lost to a concurrent opening ends without a
    // provider; its view joins the terminal running the Session instead.
    // Desktop's relays report the end and reopen by Session themselves.
    let Some(session) = session.filter(|_| std::env::var_os("LF_SERVER_STATUS").is_none()) else {
        return Ok(());
    };
    // An unreadable store leaves this view ended, as it was before the read.
    let running = crate::journal::process_ledger()
        .ok()
        .and_then(|store| store.session_terminal(session.as_str()).ok().flatten());
    match running.filter(|running| running != id) {
        Some(running) => attach_with_terminal(
            &running,
            Some(session),
            None,
            &mut RelayInput::new(input.focused),
        ),
        None => Ok(()),
    }
}

/// The held terminal's PTY position the screen shows and the partial event
/// scanner describe one view. A fresh attachment replaces both together.
#[derive(Default)]
struct RelayOutput {
    /// The PTY position shown on screen, until the relay draws over it.
    position: Option<u64>,
    scanner: Option<Scanner>,
    /// The attachment in flight requested a continuation.
    continuing: bool,
}

impl RelayOutput {
    /// The screen continues from the PTY position it shows, or repaints.
    fn attachment(&mut self, focused: bool) -> Request {
        self.continuing = self.position.is_some();
        if !self.continuing {
            self.scanner = None;
        }
        let (rows, cols) = size();
        Request::Attach {
            rows,
            cols,
            focused: Some(focused),
            name: tty_name(libc::STDIN_FILENO),
            from: self.position,
        }
    }

    /// Whether this acknowledgement continues the screen from a position.
    fn acknowledged(&mut self, nonce: &str) -> bool {
        let continued = std::mem::take(&mut self.continuing);
        match &mut self.scanner {
            // A continuation is a new viewer with its own nonce.
            Some(scanner) if continued => scanner.renonce(nonce),
            Some(_) => {}
            None => self.scanner = Some(Scanner::new(nonce)),
        }
        continued
    }

    /// The relay drew into the screen itself: it no longer shows a PTY
    /// position, so the next recovery repaints it.
    fn drew(&mut self) {
        self.position = None;
    }
}

/// Terminals answer queries in order, so once a terminal answers this
/// status report every answer it owed replayed output has arrived.
const FENCE: &[u8] = b"\x1b[5n";
const FENCE_ANSWER: &[u8] = b"\x1b[0n";
/// How long a terminal that never answers `FENCE` holds input back.
const FENCE_WAIT: Duration = Duration::from_secs(1);

/// A view catching up from a position replays output whose queries were
/// answered long ago; its terminal answers them again. Nothing it reads
/// reaches the agent until that is over.
#[derive(Debug)]
enum Replay {
    Live,
    /// Replayed output is still arriving.
    Replaying,
    /// The stream is live; the terminal's answers end with `FENCE_ANSWER`.
    Fenced {
        matched: usize,
        until: Instant,
    },
}

impl Replay {
    fn fenced() -> Self {
        Self::Fenced {
            matched: 0,
            until: Instant::now() + FENCE_WAIT,
        }
    }

    /// Only input after the replay fence reaches the agent.
    fn pass<'a>(&mut self, bytes: &'a [u8]) -> &'a [u8] {
        let matched = match self {
            Self::Live => return bytes,
            Self::Replaying => return &[],
            Self::Fenced { matched, .. } => matched,
        };
        for (i, byte) in bytes.iter().enumerate() {
            if *byte == FENCE_ANSWER[*matched] {
                *matched += 1;
                if *matched == FENCE_ANSWER.len() {
                    *self = Self::Live;
                    return &bytes[i + 1..];
                }
            } else {
                *matched = usize::from(*byte == FENCE_ANSWER[0]);
            }
        }
        &[]
    }

    fn expired(&self) -> bool {
        matches!(self, Self::Fenced { until, .. } if Instant::now() >= *until)
    }
}

struct RelayInput {
    focused: bool,
    paste: Paste,
}

impl RelayInput {
    fn new(focused: bool) -> Self {
        Self {
            focused,
            paste: Paste::default(),
        }
    }

    fn observe_keys(&mut self, bytes: &[u8]) {
        if !self.focused && input::keys(bytes).next().is_some() {
            self.focused = true;
        }
    }

    fn discard(&mut self, bytes: &[u8]) {
        self.paste.observe(bytes);
        for (input, report) in focus_reports(bytes) {
            self.observe_keys(input);
            if let Some(focused) = report {
                self.focused = focused;
            }
        }
    }

    fn forward(
        &mut self,
        bytes: &[u8],
        socket: &mut impl Write,
        replay: &mut Replay,
        resume_input: &mut bool,
    ) -> Result<()> {
        // Keep observing focus and paste framing after a write fails, for reconnect.
        let mut sent = Ok(());
        for (input, report) in focus_reports(bytes) {
            let typed = replay.pass(input);
            self.paste.observe(&input[..input.len() - typed.len()]);
            if !typed.is_empty() {
                self.observe_keys(typed);
                if std::mem::take(resume_input) {
                    let framing = self.paste.resume();
                    if !framing.is_empty() {
                        sent = sent.and_then(|()| send_frame(socket, Tag::Input, &framing));
                    }
                }
                self.paste.observe(typed);
                sent = sent.and_then(|()| send_frame(socket, Tag::Input, typed));
            }
            if let Some(focused) = report {
                self.focused = focused;
                sent = sent.and_then(|()| send(socket, Request::Focus { focused }));
            }
        }
        sent
    }
}

/// A view of a held terminal always outlives its server connection.
fn attach_with_terminal(
    id: &TerminalId,
    session: Option<&LfSessionId>,
    terminal: Option<ViewerTerminal>,
    input: &mut RelayInput,
) -> Result<()> {
    let mut status = connection::Status::from_env();
    let mut recovering = false;
    let socket = if let Some(status) = &mut status {
        let Some((socket, recovered)) =
            status.connect(id, bridged_session(session)?, true, input)?
        else {
            return Ok(());
        };
        recovering = recovered;
        socket
    } else {
        match registry::connect(id) {
            Ok(socket) => socket,
            Err(error) => {
                return replace_view(
                    id,
                    session,
                    input.focused,
                    &error.downcast::<registry::OtherRevision>()?,
                )
            }
        }
    };
    view(
        stream::Stream::socket(socket)?,
        Some(id),
        Some(RelayRoute {
            session,
            status,
            recovering,
        }),
        terminal,
        ATTACH_TIMEOUT,
        input,
    )
}

/// View the existing protocol over a command's stdin/stdout, such as SSH.
/// The destination forwards bytes to its held terminal and creates no
/// terminal here.
pub fn relay_over(argv: &[String], unfocused: bool) -> Result<()> {
    let (program, args) = argv
        .split_first()
        .context("Viewer transport command is missing")?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .process_group(0)
        .spawn()
        .context("start viewer transport")?;
    let group = crate::os_process::ProcessGroupGuard::new(child.id());
    let result = (|| {
        let stream = stream::Stream::new(
            OwnedFd::from(child.stdout.take().expect("piped transport stdout")).into(),
            OwnedFd::from(child.stdin.take().expect("piped transport stdin")).into(),
        )?;
        view(
            stream,
            None,
            None,
            None,
            COMMAND_ATTACH_TIMEOUT,
            &mut RelayInput::new(initial_focus(unfocused)),
        )
    })();
    // Leave is already written, but SSH must forward it before we stop its
    // process. A failed/unresponsive connection instead uses ordinary absence.
    if result.is_ok() {
        let _ = crate::os_process::wait_for_exit(&mut child, Some(Duration::from_secs(2)), || {});
    }
    group.terminate();
    let _ = child.wait();
    result
}

/// A viewer of one known held terminal, which reconnects through the server.
/// Command pipes (including SSH) have no local route to reconnect.
struct RelayRoute<'a> {
    session: Option<&'a LfSessionId>,
    status: Option<connection::Status>,
    /// The first connection already failed once; attach as a recovery.
    recovering: bool,
}

/// The server attaches by Session; only Desktop's Session panes bridge through
/// it, and they always name theirs.
fn bridged_session(session: Option<&LfSessionId>) -> Result<&LfSessionId> {
    session.context("A server-bridged terminal relay names its Session (`--session`)")
}

fn view(
    mut socket: stream::Stream,
    id: Option<&TerminalId>,
    route: Option<RelayRoute<'_>>,
    mut terminal: Option<ViewerTerminal>,
    attach_timeout: Duration,
    input: &mut RelayInput,
) -> Result<()> {
    let server = route.is_some();
    let (session, mut status, mut recovering) = match route {
        Some(route) => (route.session, route.status, route.recovering),
        None => (None, None, false),
    };
    let mut resume_input = true;
    let mut acknowledgement_deadline = Some(Instant::now() + attach_timeout);
    let (rows, cols) = size();
    let mut output = RelayOutput::default();
    let initial = send(&mut socket, output.attachment(input.focused));
    let mut interrupted = match initial {
        Ok(()) => None,
        Err(error) if server => Some(error.to_string()),
        Err(error) => return Err(error),
    };
    let mut decoder = Decoder::default();
    let mut attached = false;
    let mut returning = None;
    let mut stdin_open = true;
    let mut previous_size = (rows, cols);
    let mut title = id.map(crate::terminal_title::ViewerTitle::new);
    let mut heartbeat = Instant::now() + HEARTBEAT_INTERVAL;
    let mut child_code = 0;
    let mut last_response = Instant::now();
    let mut replay = Replay::Live;
    loop {
        if replay.expired() {
            replay = Replay::Live;
        }
        let mut can_reconnect = server;
        if status.as_ref().is_some_and(connection::Status::closed) {
            return Ok(());
        }
        if let Some(reason) = interrupted.take() {
            // Close the old bridge before asking the terminal for its free claims.
            socket.shutdown();
            let held = id.context("only a known terminal reconnects")?;
            let next = if let Some(status) = &mut status {
                status
                    .connect(held, bridged_session(session)?, false, input)?
                    .map(|(socket, _)| socket)
            } else {
                tracing::debug!(%reason, terminal = %held, "view reconnecting");
                let next = reconnect_held(held, input);
                if let Some(skew) = next
                    .as_ref()
                    .err()
                    .and_then(|error| error.downcast_ref::<registry::OtherRevision>())
                {
                    drop(terminal.take());
                    return replace_view(held, session, input.focused, skew);
                }
                // An agent that exited while no server ran ends the view as
                // its exit would have.
                if let Some((code, provider)) = next.as_ref().err().and_then(ended_exit) {
                    drop(terminal.take());
                    if let (Some(provider), 1..) = (provider, code) {
                        eprintln!(
                            "\n{}",
                            crate::harness::native_exit::exit_line(&provider, code)
                        );
                    }
                    return if code == 0 {
                        Ok(())
                    } else {
                        Err(crate::process::CommandExit(code).into())
                    };
                }
                next?
            };
            let Some(next) = next else {
                return Ok(());
            };
            socket = stream::Stream::socket(next)?;
            last_response = Instant::now();
            heartbeat = last_response + HEARTBEAT_INTERVAL;
            decoder = Decoder::default();
            attached = false;
            recovering = true;
            resume_input = true;
            if let Err(error) = send(&mut socket, output.attachment(input.focused)) {
                interrupted = Some(error.to_string());
                continue;
            }
            acknowledgement_deadline = Some(Instant::now() + attach_timeout);
        }
        if let Some(title) = &mut title {
            title.tick();
        }
        if Instant::now() >= heartbeat {
            heartbeat = Instant::now() + HEARTBEAT_INTERVAL;
            if let Err(error) = send(&mut socket, Request::Controlled { ready: None }) {
                if !can_reconnect {
                    return Err(error);
                }
                interrupted = Some(error.to_string());
                continue;
            }
        }
        if acknowledgement_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            if can_reconnect {
                interrupted =
                    Some("Attachment was not acknowledged; its outcome is unknown".to_owned());
                continue;
            }
            bail!("Held terminal did not acknowledge attachment; outcome is unknown, no provider was restarted and no input was replayed");
        }
        let mut fds = [
            libc::pollfd {
                fd: socket.input.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if stdin_open
                    && (attached || (terminal.is_some() && acknowledgement_deadline.is_none()))
                {
                    0
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: poll receives a live array of exactly two descriptors.
        if unsafe { libc::poll(fds.as_mut_ptr(), 2, 250) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error.into());
            }
        }
        if fds[0].revents != 0 {
            let mut bytes = [0; 16384];
            let n = match socket.read(&mut bytes) {
                Ok(n) if n != 0 => n,
                result if can_reconnect => {
                    interrupted = Some(match result {
                        Err(error) => error.to_string(),
                        _ => "The bridge closed".to_owned(),
                    });
                    continue;
                }
                Ok(_) => bail!("Held terminal disconnected; outcome is unknown"),
                Err(error) => return Err(error.into()),
            };
            last_response = Instant::now();
            decoder.extend(&bytes[..n]);
            while let Some((tag, payload)) = decoder.next()? {
                match tag {
                    Tag::Reply | Tag::Aside | Tag::Close => {}
                    Tag::Position => {
                        output.position = Some(wire::parse_position(&payload)?);
                        continue;
                    }
                    Tag::Snapshot => output.drew(),
                    _ => {
                        if let Some(position) = &mut output.position {
                            *position += payload.len() as u64;
                        }
                    }
                }
                let pieces = match tag {
                    Tag::Close => {
                        let close: Close = serde_json::from_slice(&payload)?;
                        let message = match close.reason {
                            CloseReason::Finished => None,
                            CloseReason::Replaced => {
                                Some("Session terminal replaced; reattach by Session.")
                            }
                            CloseReason::Retired => {
                                Some("Session terminal retired; resume when needed.")
                            }
                        };
                        if let Some(status) = &mut status {
                            status.ended(message);
                        }
                        if let Some(message) = message {
                            write_output(
                                &mut terminal,
                                2,
                                format!("\r\n{message}\r\n").as_bytes(),
                            )?;
                        }
                        return if child_code == 0 {
                            Ok(())
                        } else {
                            Err(crate::process::CommandExit(child_code).into())
                        };
                    }
                    Tag::Reply => {
                        let reply: Outgoing = serde_json::from_slice(&payload)?;
                        match reply.reply {
                            Reply::Attached {
                                nonce,
                                terminal: is_terminal,
                                ..
                            } => {
                                acknowledgement_deadline = None;
                                can_reconnect |= id.is_some() && is_terminal;
                                attached = true;
                                let recovered = std::mem::take(&mut recovering);
                                let continued = output.acknowledged(&nonce);
                                replay = if continued {
                                    Replay::Replaying
                                } else {
                                    Replay::Live
                                };
                                if recovered || status.is_some() {
                                    discard_reconnect_input(input)?;
                                    fds[1].revents = 0;
                                }
                                // A continued screen shows the agent's output and nothing else.
                                if recovered && status.is_none() && !continued {
                                    output.drew();
                                    write_output(
                                        &mut terminal,
                                        2,
                                        b"\r\nReconnected to the same Session.\r\n",
                                    )?;
                                }
                                // A continued screen already shows the agent's modes
                                // and output up to its position; clearing it repaints.
                                if !continued {
                                    if let Some(terminal) = &mut terminal {
                                        terminal.handoff()?;
                                    }
                                }
                                if is_terminal {
                                    if terminal.is_none() {
                                        terminal = Some(ViewerTerminal::enter()?);
                                    }
                                    let terminal = terminal.as_mut().expect("terminal entered");
                                    terminal.make_raw()?;
                                    // Focus retakes the size; the agent hears it only
                                    // when it asked for focus reports itself.
                                    terminal.report_focus()?;
                                }
                                if let Some(status) = &mut status {
                                    status.connected();
                                }
                            }
                            Reply::Busy
                            | Reply::Controlled { .. }
                            | Reply::NativeEnded
                            | Reply::Accepted => {}
                        }
                        continue;
                    }
                    Tag::Snapshot if output.scanner.is_none() => {
                        // A destination may show retained history before a terminal
                        // is available. It grants no input or retention claim.
                        if terminal.is_none() {
                            terminal = Some(ViewerTerminal::enter()?);
                        }
                        terminal
                            .as_mut()
                            .expect("preview owns a terminal")
                            .write(1, &payload)?;
                        continue;
                    }
                    Tag::Snapshot | Tag::Out | Tag::Aside => output
                        .scanner
                        .as_mut()
                        .context("Held terminal streamed before acknowledging attachment")?
                        .scan(&payload),
                    Tag::Err => vec![wire::Piece::Bytes(payload)],
                    Tag::Request | Tag::Input | Tag::Position => {
                        anyhow::bail!("unexpected transport frame")
                    }
                };
                let stream = if tag == Tag::Err { 2 } else { 1 };
                for piece in pieces {
                    let event = match piece {
                        wire::Piece::Bytes(bytes) => {
                            write_output(&mut terminal, stream, &bytes)?;
                            continue;
                        }
                        wire::Piece::Event(_, event) => event,
                    };
                    match event {
                        Event::Resize { .. }
                        | Event::Theme { .. }
                        | Event::Owner { .. }
                        | Event::ProgramStatus { .. } => {}
                        Event::Directory { cwd } => {
                            if let Err(error) = std::env::set_current_dir(&cwd) {
                                tracing::debug!(%error, "viewer cannot enter execution directory");
                            }
                            if let Some(terminal) = &terminal {
                                terminal.retire_directory_on_restore();
                            }
                        }
                        Event::ChildExited { code } => {
                            child_code = code;
                        }
                        Event::Detached => {
                            // Detached on purpose: an SSH viewer must not read as lost.
                            let _ = send(&mut socket, Request::Leave);
                            return Ok(());
                        }
                        Event::Return { detach } => {
                            // The agent may have exited with its final output and
                            // status still buffered. A readiness write cannot discard them.
                            let acknowledged = send(
                                &mut socket,
                                Request::Controlled {
                                    ready: Some(detach.clone()),
                                },
                            )
                            .inspect_err(|error| tracing::debug!(%error, "viewer readiness was not delivered"))
                            .is_ok();
                            returning = Some(detach);
                            // A tab launch succeeds only when this view is attached
                            // and its Session's native client is ready.
                            if attached && acknowledged {
                                answer_open("attached");
                            }
                            if let Some(status) = &mut status {
                                status.native_ready();
                            }
                            // This marker comes from the acknowledged live relay, never
                            // the terminal's saved screen. Desktop consumes it as readiness.
                            if attached && crate::session_record::current_terminal_id().is_some() {
                                write_output(&mut terminal, 1, b"\x1b]2;loopflow:input-ready\x07")?;
                            }
                        }
                        Event::Live => {
                            // The terminal's answer to the fence comes after
                            // every answer it owes the replayed output.
                            if matches!(replay, Replay::Replaying) {
                                replay = if terminal.is_some() {
                                    write_output(&mut terminal, 1, FENCE)?;
                                    Replay::fenced()
                                } else {
                                    Replay::Live
                                };
                            }
                        }
                        Event::Hint { text } => {
                            if let Some(terminal) = &mut terminal {
                                output.drew();
                                terminal.write(2, format!("\r\n{text}\r\n").as_bytes())?;
                            } else {
                                eprintln!("{text}");
                            }
                        }
                    }
                }
            }
        }
        // Drain available replies before judging an unanswered heartbeat.
        if acknowledgement_deadline.is_none() && last_response.elapsed() >= HEARTBEAT_TIMEOUT {
            if can_reconnect {
                interrupted = Some("The bridge stopped answering".to_owned());
                continue;
            }
            bail!("Held terminal stopped answering; outcome is unknown, no provider was restarted and no input was replayed");
        }
        if let Some(status) = &mut status {
            if status.closed() {
                return Ok(());
            }
        }
        if fds[1].revents != 0 {
            let mut bytes = [0; 8192];
            let n = io::stdin().read(&mut bytes)?;
            if n == 0 {
                stdin_open = false;
            } else if terminal.is_some() && input.paste.detaches(&bytes[..n]) {
                // Restore the terminal before saying how to return.
                drop(terminal.take());
                let _ = send(&mut socket, Request::Leave);
                if let Some(detach) = &returning {
                    // An inline client can leave the cursor mid-line.
                    eprintln!("\n{detach}");
                }
                return Ok(());
            } else if attached {
                if let Err(error) =
                    input.forward(&bytes[..n], &mut socket, &mut replay, &mut resume_input)
                {
                    if !can_reconnect {
                        return Err(error);
                    }
                    // The write may have reached the terminal. Never retain or resend it.
                    interrupted = Some(error.to_string());
                    continue;
                }
            } else {
                input.discard(&bytes[..n]);
            }
        }
        let current_size = size();
        if terminal.is_some() && current_size != previous_size {
            previous_size = current_size;
            let resized = send(
                &mut socket,
                Request::Resize {
                    rows: current_size.0,
                    cols: current_size.1,
                },
            );
            if let Err(error) = resized {
                if !can_reconnect {
                    return Err(error);
                }
                interrupted = Some(error.to_string());
            }
        }
    }
}

/// Wait for a server to host the terminal again. Nothing is drawn: the
/// screen stays as the agent left it, and the server that takes the terminal
/// up paints its own. What is typed meanwhile is discarded; Ctrl+] closes.
fn reconnect_held(terminal: &TerminalId, input: &mut RelayInput) -> Result<Option<UnixStream>> {
    let mut wait = Duration::from_millis(100);
    loop {
        match registry::connect(terminal) {
            Ok(socket) => {
                discard_reconnect_input(input)?;
                return Ok(Some(socket));
            }
            // A server answered: the terminal has ended, or another build hosts it.
            Err(error)
                if error.is::<crate::engine::Error>() || error.is::<registry::OtherRevision>() =>
            {
                return Err(error)
            }
            Err(_) => {}
        }
        let until = Instant::now() + wait;
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            let mut fd = libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            };
            wait_for_activity(std::slice::from_mut(&mut fd), Some(left));
            if fd.revents == 0 {
                continue;
            }
            let mut bytes = [0; 8192];
            match io::stdin().read(&mut bytes) {
                Ok(0) => return Ok(None),
                Ok(n) => {
                    if input.paste.detaches(&bytes[..n]) {
                        return Ok(None);
                    }
                    input.discard(&bytes[..n]);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
        wait = (wait * 2).min(Duration::from_secs(2));
    }
}

/// Set on a view that replaced one of another build, so that meeting another
/// revision again ends it instead of starting a third.
const REPLACED_ENV: &str = "LF_VIEW_REPLACED";

/// A server of another build hosts `host`: become the installed lf as a fresh
/// view of the same terminal. A fresh view repaints from that server's screen,
/// so no position crosses builds. The agent keeps running either way; a view
/// that cannot be replaced ends with one line.
fn replace_view(
    terminal: &TerminalId,
    session: Option<&LfSessionId>,
    focused: bool,
    skew: &registry::OtherRevision,
) -> Result<()> {
    let failure = if std::env::var_os(REPLACED_ENV).is_some() {
        "the installed lf is this build".to_owned()
    } else {
        match crate::os_process::resolve_lf_binary() {
            Ok(lf) => {
                let mut command = relay_command(lf, terminal, session, focused);
                command.env(REPLACED_ENV, "1");
                crate::journal::carry_across_exec(&mut command)?;
                pass_open(&mut command);
                command.exec().to_string()
            }
            Err(error) => error.to_string(),
        }
    };
    bail!("{skew} and {failure}; the agent keeps running. Run lf install, then lf session attach --terminal {terminal}")
}

/// The code and Session provider of a held terminal's agent, when `error` is a
/// server's answer that the terminal ended and the store recorded its exit.
fn ended_exit(error: &anyhow::Error) -> Option<(u8, Option<String>)> {
    let data = error
        .downcast_ref::<crate::engine::Error>()?
        .data
        .as_ref()?;
    let code = u8::try_from(data["exit_code"].as_u64()?).ok()?;
    Some((code, data["provider"].as_str().map(str::to_owned)))
}

fn discard_reconnect_input(input: &mut RelayInput) -> io::Result<()> {
    let mut fd = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    // Drain queued keys and terminal reports, including keys typed during the
    // handshake. This is never an input queue to replay on the next connection.
    loop {
        // SAFETY: poll borrows one live descriptor entry for the duration of the call.
        if unsafe { libc::poll(&mut fd, 1, 0) } <= 0 || fd.revents == 0 {
            return Ok(());
        }
        let mut bytes = [0; 8192];
        let n = io::stdin().read(&mut bytes)?;
        input.discard(&bytes[..n]);
        if n == 0 {
            return Ok(());
        }
    }
}

pub const OPEN_RECEIPT_ENV: &str = "LF_OPEN_RECEIPT";

/// Where this process answers the `lf open` that started it. Only the viewer
/// and the launcher it starts hold one: it is handed on by name, so nothing
/// else lf runs can answer for the opening.
static OPEN_RECEIPT: Mutex<Option<PathBuf>> = Mutex::new(None);

fn take_open_receipt() {
    let receipt = std::env::var_os(OPEN_RECEIPT_ENV).map(PathBuf::from);
    std::env::remove_var(OPEN_RECEIPT_ENV);
    *OPEN_RECEIPT.lock().expect("open receipt lock poisoned") = receipt;
}

fn open_receipt() -> Option<PathBuf> {
    OPEN_RECEIPT
        .lock()
        .expect("open receipt lock poisoned")
        .clone()
}

/// Hand the opening to the lf this process becomes.
fn pass_open(command: &mut Command) {
    if let Some(receipt) = open_receipt() {
        command.env(OPEN_RECEIPT_ENV, receipt);
    }
}

/// Answer `lf open`: "attached", or why the opening failed. The first answer
/// stands: the launcher's own error beats its viewer's.
fn answer_open(answer: &str) {
    let Some(path) = OPEN_RECEIPT
        .lock()
        .expect("open receipt lock poisoned")
        .take()
    else {
        return;
    };
    let temporary = path.with_extension(std::process::id().to_string());
    if std::fs::write(&temporary, answer).is_ok() {
        // Fails when another process answered first.
        let _ = std::fs::hard_link(&temporary, &path);
        let _ = std::fs::remove_file(&temporary);
    }
}

/// Answer an opening this command failed without attaching, with its own
/// error. The pane waits; the receipt says why. A viewer whose conversation
/// exited leaves the answer to the launcher, or to the pane's status line.
pub fn end_open(result: &Result<()>) {
    let (Err(error), Some(_)) = (result, open_receipt()) else {
        return;
    };
    let reason = format!("{error:#}");
    tracing::warn!(
        command = std::env::args().skip(1).collect::<Vec<_>>().join(" "),
        error = %reason,
        "conversation opened by lf open failed before attaching"
    );
    if error
        .downcast_ref::<crate::process::CommandExit>()
        .is_none()
    {
        answer_open(&reason);
    }
}

fn write_output(terminal: &mut Option<ViewerTerminal>, stream: u8, bytes: &[u8]) -> io::Result<()> {
    if let Some(terminal) = terminal {
        terminal.write(stream, bytes)
    } else if stream == 2 || STDOUT_HOLDS_RECORD.load(Ordering::Relaxed) {
        io::stderr().write_all(bytes)?;
        io::stderr().flush()
    } else {
        io::stdout().write_all(bytes)?;
        io::stdout().flush()
    }
}

/// Hand the terminal's host what to record when this process, about to exec
/// into its provider, exits. A nested launcher keeps its own wait and returns
/// false.
pub(crate) fn hand_exit_record(record: &NativeExit) -> Result<bool> {
    let handoff = HANDOFF
        .get()
        .context("this launch has no held terminal to hand its wait to")?;
    let mut socket = handoff.connect()?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    // A delayed reply does not undo the host's acceptance. Wait for its
    // decision or a closed connection before choosing who records the exit.
    send(
        &mut socket,
        Request::ExitRecord {
            key: handoff.key.clone(),
            pid: std::process::id(),
            record: Box::new(record.clone()),
        },
    )?;
    match receive_reply(&mut socket)? {
        Reply::Accepted => Ok(true),
        Reply::Busy => Ok(false),
        other => bail!("held terminal refused the exit record: {other:?}"),
    }
}

/// The exec failed; the launcher records its own failure.
pub(crate) fn drop_exit_record() {
    let dropped = (|| -> Result<()> {
        let handoff = HANDOFF.get().context("no held terminal")?;
        let mut socket = handoff.connect()?;
        socket.set_write_timeout(Some(Duration::from_secs(1)))?;
        send(
            &mut socket,
            Request::DropExitRecord {
                key: handoff.key.clone(),
            },
        )?;
        // Do not let launcher exit overtake cancellation in the host's loop.
        match receive_reply(&mut socket)? {
            Reply::Accepted => Ok(()),
            other => bail!("held terminal refused exit record cancellation: {other:?}"),
        }
    })();
    if let Err(error) = dropped {
        tracing::warn!(%error, "could not cancel the exit record for a failed exec");
    }
}

/// Whether a view of this execution is attached. A closed terminal leaves its
/// execution running with none, ready for the next view.
pub fn viewed(id: &TerminalId) -> Result<bool> {
    attachment(id).map(|(viewed, _)| viewed)
}

/// End a held terminal with `text` for its viewers. No decision is stored.
pub(crate) fn close_notice(id: &TerminalId, text: &str) -> Result<()> {
    let mut socket = registry::connect(id)?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    send(&mut socket, Request::CloseNotice { text: text.into() })?;
    match receive_reply(&mut socket)? {
        Reply::Accepted => Ok(()),
        other => bail!("unexpected close notice reply {other:?}"),
    }
}

/// Whether a view is attached, and whether the native client is ready for one.
pub fn attachment(id: &TerminalId) -> Result<(bool, bool)> {
    let mut socket = registry::connect(id)?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    send(&mut socket, Request::Controlled { ready: None })?;
    match receive_reply(&mut socket)? {
        Reply::Controlled { viewed, ready } => Ok((viewed, ready == Some(true))),
        other => bail!("unexpected transport reply {other:?}"),
    }
}

/// Read the reply on a control connection that carries no output stream.
fn receive_reply(socket: &mut UnixStream) -> Result<Reply> {
    let mut decoder = Decoder::default();
    let mut bytes = [0; 4096];
    loop {
        let n = socket.read(&mut bytes)?;
        anyhow::ensure!(n != 0, "Held terminal closed before answering");
        decoder.extend(&bytes[..n]);
        if let Some((tag, payload)) = decoder.next()? {
            anyhow::ensure!(tag == Tag::Reply, "unexpected transport frame");
            let reply: Outgoing = serde_json::from_slice(&payload)?;
            return Ok(reply.reply);
        }
    }
}

/// Detach the execution's viewers; only its direct child holds this capability.
pub(crate) fn detach_viewers() -> Result<()> {
    let handoff = HANDOFF
        .get()
        .context("Execution has no retained transport; use Ctrl+] in an attached viewer")?;
    let mut socket = handoff.connect()?;
    send(
        &mut socket,
        Request::DetachViewers {
            key: handoff.key.clone(),
        },
    )
}

/// The Session's selected directory, which `lf session bind` can move after launch.
fn selected_directory(record: &NativeExit) -> Option<PathBuf> {
    crate::journal::process_ledger()
        .and_then(|store| store.session(&record.session))
        .ok()
        .flatten()
        .map(|session| session.cwd)
}

/// The native client has connected in this directory. The host serializes
/// its OSC report with provider bytes and retains it for later viewers.
pub(crate) fn report_directory(cwd: &Path) {
    let Some(handoff) = HANDOFF.get() else { return };
    let result = (|| -> Result<()> {
        let mut socket = handoff.connect()?;
        socket.set_write_timeout(Some(Duration::from_secs(1)))?;
        send(
            &mut socket,
            Request::Directory {
                key: handoff.key.clone(),
                cwd: cwd.to_path_buf(),
            },
        )
    })();
    if let Err(error) = result {
        tracing::debug!(%error, "terminal directory report unavailable");
    }
}

/// A native client running in this execution's held terminal. Its own exit text is
/// replaced by `exit` when it ends; dropping the guard reports that end.
#[derive(Debug)]
pub(crate) struct NativeClient {
    socket: UnixStream,
    key: String,
}

pub(crate) fn native_client(exit: String, detach: String) -> Option<NativeClient> {
    let handoff = HANDOFF.get()?;
    let started = (|| -> Result<UnixStream> {
        let mut socket = handoff.connect()?;
        socket.set_write_timeout(Some(Duration::from_secs(1)))?;
        send(
            &mut socket,
            Request::NativeStarted {
                key: handoff.key.clone(),
                exit,
                detach,
            },
        )?;
        Ok(socket)
    })();
    match started {
        Ok(socket) => Some(NativeClient {
            socket,
            key: handoff.key.clone(),
        }),
        Err(error) => {
            tracing::debug!(%error, "native client exit text unavailable");
            None
        }
    }
}

impl Drop for NativeClient {
    fn drop(&mut self) {
        let ended = (|| -> Result<()> {
            send(
                &mut self.socket,
                Request::NativeEnded {
                    key: self.key.clone(),
                },
            )?;
            // Hold further output until the host has replaced the exit text.
            self.socket.set_read_timeout(Some(Duration::from_secs(2)))?;
            let mut reply = [0; 64];
            let _ = self.socket.read(&mut reply)?;
            Ok(())
        })();
        if let Err(error) = ended {
            tracing::debug!(%error, "native client end was not acknowledged");
        }
    }
}

/// Live input a controlling viewer can type. The host shows `hint` to each
/// controlling viewer as it attaches until this guard drops, so headless output
/// never contains it. `None` when no viewer can attach.
#[derive(Debug)]
pub(crate) struct InputHint {
    _socket: UnixStream,
}

pub(crate) fn input_hint(hint: String) -> Option<InputHint> {
    let handoff = HANDOFF.get()?;
    let registered = (|| -> Result<UnixStream> {
        let mut socket = handoff.connect()?;
        socket.set_write_timeout(Some(Duration::from_secs(1)))?;
        send(
            &mut socket,
            Request::InputHint {
                key: handoff.key.clone(),
                hint,
            },
        )?;
        Ok(socket)
    })();
    match registered {
        Ok(socket) => Some(InputHint { _socket: socket }),
        Err(error) => {
            tracing::debug!(%error, "input hint unavailable to viewers");
            None
        }
    }
}

/// A Graph and its steps share one held terminal. This is an explicit execution
/// handoff, not authority inferred from process ancestry.
pub fn configure_child(command: &mut Command) {
    if let Some(handoff) = HANDOFF.get() {
        handoff.pass(command, std::process::id());
    }
}

/// An exec keeps this PID and its parent, so the replacing lf binary is still
/// the direct child the handoff names, and still answers its opening.
pub(crate) fn carry_across_exec(command: &mut Command) {
    if let Some(handoff) = HANDOFF.get() {
        handoff.pass(command, handoff.parent);
    }
    pass_open(command);
}

/// Status travels beside output to the sole writer, never through capture pipes.
pub fn report(state: loopflow_model::program_status::State, step: bool, progress: Option<u8>) {
    if !io::stdout().is_terminal() || std::env::args().any(|arg| arg == "--json") {
        return;
    }
    let (Some(handoff), Some(process)) = (HANDOFF.get(), crate::journal::current_lf_process_id())
    else {
        return;
    };
    let program = format!(
        "lf/{}{}",
        process.as_str().replace('-', ""),
        if step { "/step" } else { "" }
    );
    let result = (|| -> Result<()> {
        let mut socket = handoff.connect()?;
        socket.set_write_timeout(Some(Duration::from_secs(1)))?;
        send(
            &mut socket,
            Request::Status {
                key: handoff.key.clone(),
                program,
                state,
                progress,
            },
        )
    })();
    if let Err(error) = result {
        tracing::debug!(%error, "terminal status unavailable");
    }
}

/// Report the execution's actual return, never a viewer's disappearance.
pub fn report_exit(code: u8) {
    report(loopflow_model::program_status::State::Clear, true, None);
    report(
        if code == 0 {
            loopflow_model::program_status::State::Done
        } else if code == 130 {
            loopflow_model::program_status::State::Idle
        } else {
            loopflow_model::program_status::State::Error
        },
        false,
        None,
    );
}

#[cfg(test)]
mod open_receipt_tests {
    use std::ffi::OsStr;
    use std::process::Command;

    use super::{
        answer_open, carry_across_exec, open_receipt, take_open_receipt, OPEN_RECEIPT_ENV,
    };

    #[test]
    fn the_first_answer_to_an_opening_stands() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let receipt = dir.path().join("receipt");
        for answer in ["'claude' is not installed", "a later error", "attached"] {
            std::env::set_var(OPEN_RECEIPT_ENV, &receipt);
            take_open_receipt();
            answer_open(answer);
            assert!(open_receipt().is_none());
        }
        assert_eq!(
            std::fs::read_to_string(&receipt).unwrap(),
            "'claude' is not installed"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    /// `lf install server`, a provider and its tools inherit the environment;
    /// none of them may answer for the opening.
    #[test]
    fn only_the_lf_this_one_becomes_inherits_the_opening() {
        let _env = crate::journal::test_env_lock();
        std::env::set_var(OPEN_RECEIPT_ENV, "/tmp/receipt");
        take_open_receipt();
        assert!(std::env::var_os(OPEN_RECEIPT_ENV).is_none());
        let mut next = Command::new("lf");
        carry_across_exec(&mut next);
        assert_eq!(
            next.get_envs()
                .find(|(name, _)| *name == OPEN_RECEIPT_ENV)
                .and_then(|(_, value)| value),
            Some(OsStr::new("/tmp/receipt"))
        );
        take_open_receipt();
        assert!(open_receipt().is_none());
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::net::UnixStream;
    use std::process::Command;
    use std::time::Duration;

    use super::{
        carry_across_exec, focus_reports, output, place_events, wire, Event, Handoff, RelayOutput,
        Replay, Sizing, Viewer, CONTROL_ENV, HELD_ENV, LIMIT, PARENT_ENV,
    };
    use loopflow_model::id::TerminalId;

    /// A start that gets no PTY names the shortage, not the OS error alone.
    #[test]
    fn a_pty_failure_says_when_the_machine_has_none_left() {
        let error = std::io::Error::from_raw_os_error(libc::EAGAIN);
        let usage = |in_use| {
            Some(super::PtyUsage {
                in_use,
                max: 511,
                limit: "kern.tty.ptmx_max",
            })
        };
        assert_eq!(
            super::pty_failure(&error, usage(511)),
            "no PTY left: 511 in use (kern.tty.ptmx_max)"
        );
        let other = super::pty_failure(&error, usage(144));
        assert!(other.starts_with("cannot open a PTY: "), "{other}");
        assert!(
            other.ends_with("(144 of 511 in use, kern.tty.ptmx_max)"),
            "{other}"
        );
    }

    #[test]
    fn a_catching_up_views_terminal_answers_never_reach_the_agent() {
        // Replayed output is still arriving: whatever the terminal says waits.
        let mut catching = Replay::Replaying;
        assert_eq!(catching.pass(b"\x1b[12;1R"), b"");
        // Late answers to replayed queries, then the fence's answer split
        // across reads, then the first key typed after it.
        let mut catching = Replay::fenced();
        assert_eq!(catching.pass(b"\x1b[3;1R\x1b[?62;22c\x1b"), b"");
        assert_eq!(catching.pass(b"[0"), b"");
        assert_eq!(catching.pass(b"nls\r"), b"ls\r");
        let mut catching = Replay::fenced();
        assert_eq!(catching.pass(b"\x1b\x1b[0n"), b"");
        assert_eq!(catching.pass(b"x"), b"x");
    }

    #[test]
    fn a_held_screen_the_relay_drew_into_is_refreshed_not_continued() {
        let attachment = |request: wire::Request| match request {
            wire::Request::Attach { from, focused, .. } => (from, focused),
            other => panic!("not an attachment: {other:?}"),
        };
        for focused in [false, true] {
            let mut output = RelayOutput {
                position: Some(42),
                ..RelayOutput::default()
            };
            assert_eq!(
                attachment(output.attachment(focused)),
                (Some(42), Some(focused))
            );
            output.acknowledged("nonce");
            output.drew();
            assert_eq!(
                attachment(output.attachment(focused)),
                (None, Some(focused))
            );
        }
    }

    #[test]
    fn slow_exit_recording_keeps_an_already_observed_move() {
        let (_sender, settled) = std::sync::mpsc::channel();
        let mut settling = super::Settling {
            settled,
            since: std::time::Instant::now() - super::SETTLE_WAIT,
            fallback: crate::harness::native_exit::Settled {
                moved: true,
                code: 0,
                lines: vec!["This conversation moved to tmux %9".into()],
            },
            process: "test".into(),
            delivered: false,
        };
        let outcome = settling.poll().unwrap();
        assert_eq!(outcome.code, 0);
        assert!(outcome.moved);
        assert_eq!(outcome.lines, ["This conversation moved to tmux %9"]);
        assert!(settling.poll().is_none());
    }

    #[test]
    fn heartbeat_requires_thirty_seconds_of_uptime() {
        let (socket, _peer) = UnixStream::pair().unwrap();
        let mut viewer = Viewer::new(socket);
        viewer.attached = true;
        // Only uptime advances this deadline; wall time spent asleep is absent.
        assert!(!viewer.heartbeat_expired(viewer.heartbeat_at + Duration::from_secs(29)));
        assert!(viewer.heartbeat_expired(viewer.heartbeat_at + Duration::from_secs(30)));
    }

    #[test]
    fn close_is_one_final_frame_after_output_for_every_reason() {
        use super::{close_viewers, Close, CloseReason};
        use std::io::Read;
        for reason in [
            CloseReason::Replaced,
            CloseReason::Finished,
            CloseReason::Retired,
        ] {
            let (socket, mut peer) = UnixStream::pair().unwrap();
            socket.set_nonblocking(true).unwrap();
            let mut viewer = Viewer::new(socket);
            viewer.attached = true;
            // An incomplete terminal escape cannot swallow the close frame.
            viewer.stream(wire::Tag::Out, b"last\x1b]").unwrap();
            close_viewers(std::slice::from_mut(&mut viewer), reason);
            let mut bytes = Vec::new();
            peer.read_to_end(&mut bytes).unwrap();
            let mut decoder = wire::Decoder::default();
            decoder.extend(&bytes);
            assert_eq!(
                decoder.next().unwrap(),
                Some((wire::Tag::Out, b"last\x1b]".to_vec()))
            );
            let (tag, payload) = decoder.next().unwrap().unwrap();
            assert_eq!(tag, wire::Tag::Close);
            assert_eq!(
                serde_json::from_slice::<Close>(&payload).unwrap().reason,
                reason
            );
            assert!(decoder.next().unwrap().is_none());
        }
    }

    #[test]
    fn equally_inactive_views_keep_the_live_owner() {
        let (first, _first_peer) = UnixStream::pair().unwrap();
        let (second, _second_peer) = UnixStream::pair().unwrap();
        let mut viewers = vec![Viewer::new(first), Viewer::new(second)];
        let mut sizing = Sizing::default();
        viewers[0].attached = true;
        sizing.attach(&mut viewers[0], false);
        viewers[1].attached = true;
        sizing.attach(&mut viewers[1], false);
        sizing.settle(&viewers);
        assert!(sizing.held_by(&viewers[0]));
        sizing.touch(&mut viewers[1]);
        assert!(sizing.held_by(&viewers[1]));
        viewers[1].leaving = true;
        sizing.settle(&viewers);
        assert!(sizing.held_by(&viewers[0]));
        viewers[0].leaving = true;
        sizing.settle(&viewers);
        assert_eq!(sizing.id(), None);
    }

    #[test]
    fn focus_reports_split_input_in_arrival_order() {
        let parts = focus_reports(b"a\x1b[I\x1b[Ob\x1b[A").collect::<Vec<_>>();
        assert_eq!(
            parts,
            [
                (b"a".as_slice(), Some(true)),
                (b"", Some(false)),
                (b"b\x1b[A", None)
            ]
        );
    }

    #[test]
    fn pending_events_are_bounded_even_inside_an_unfinished_terminal_sequence() {
        let mut output = output::Output::new(true).unwrap();
        output.process(1, b"\x1b]unfinished", false);
        assert!(!output.ready());
        let (first, _first_peer) = UnixStream::pair().unwrap();
        let (second, _second_peer) = UnixStream::pair().unwrap();
        let mut viewers = vec![Viewer::new(first), Viewer::new(second)];
        let survivor = viewers[1].id.clone();
        for _ in 0..=LIMIT / wire::CHUNK {
            viewers[0].event(Event::Hint {
                text: "x".repeat(wire::CHUNK),
            });
        }
        place_events(&output, &mut viewers, false);
        assert_eq!(
            viewers.len(),
            1,
            "pending controls must share the viewer's queue limit"
        );
        assert_eq!(viewers[0].id, survivor);
    }

    fn env<'a>(command: &'a Command, key: &str) -> Option<&'a OsStr> {
        command
            .get_envs()
            .find(|(name, _)| *name == key)
            .and_then(|(_, value)| value)
    }

    #[test]
    fn exec_keeps_the_handoff_with_its_original_parent() {
        let _env = crate::journal::test_env_lock();
        let terminal = TerminalId::new();
        // SAFETY: getppid has no preconditions.
        let parent = unsafe { libc::getppid() } as u32;
        std::env::set_var(HELD_ENV, terminal.as_str());
        std::env::set_var(PARENT_ENV, parent.to_string());
        std::env::set_var(CONTROL_ENV, "key");
        let handoff = Handoff::take().expect("direct child adopts the handoff");
        assert!(std::env::var_os(CONTROL_ENV).is_none());

        let mut exec = Command::new("lf");
        handoff.pass(&mut exec, handoff.parent);
        assert_eq!(env(&exec, HELD_ENV), Some(OsStr::new(terminal.as_str())));
        assert_eq!(
            env(&exec, PARENT_ENV),
            Some(OsStr::new(&parent.to_string()))
        );
        assert_eq!(env(&exec, CONTROL_ENV), Some(OsStr::new("key")));
    }

    #[test]
    fn a_non_child_neither_adopts_nor_carries_a_handoff() {
        let _env = crate::journal::test_env_lock();
        std::env::set_var(HELD_ENV, TerminalId::new().as_str());
        std::env::set_var(PARENT_ENV, "0");
        std::env::set_var(CONTROL_ENV, "key");
        assert!(Handoff::take().is_none());
        assert!(std::env::var_os(CONTROL_ENV).is_none());

        let mut exec = Command::new("lf");
        carry_across_exec(&mut exec);
        assert_eq!(exec.get_envs().count(), 0);
    }
}

#[cfg(test)]
mod launch_mode_tests {
    use super::LaunchInput;
    use crate::harness::{LaunchKind, Mode};

    #[test]
    fn mode_keeps_batch_interactive_tty_and_empty_prompt_semantics() {
        for (batch, interactive, has_prompt, tty, expected) in [
            (true, false, false, false, Mode::Headless),
            (true, true, false, true, Mode::Headless),
            (false, true, true, false, Mode::Interactive),
            (false, false, true, true, Mode::Interactive),
            (false, false, true, false, Mode::Headless),
            (false, false, false, false, Mode::Interactive),
        ] {
            let input = LaunchInput {
                batch,
                interactive,
                has_prompt,
            };
            assert_eq!(input.mode_with_tty(tty), expected, "{input:?}, tty={tty}");
        }
    }

    #[test]
    fn direct_commands_keep_their_mode_and_caller() {
        for (batch, kind) in [(true, LaunchKind::Batch), (false, LaunchKind::Chat)] {
            let input = LaunchInput {
                batch,
                interactive: true,
                has_prompt: true,
            };
            assert_eq!(input.kind(), kind);
        }
    }

    #[test]
    fn inherited_task_attribution_keeps_a_direct_command_bare() {
        let _lock = crate::journal::test_env_lock();
        let _env = crate::test_ambient::EnvGuard::new();
        std::env::set_var(crate::ops::WORK_DECLARATION_ENV, "task:inherited");
        for (batch, kind) in [(false, LaunchKind::Chat), (true, LaunchKind::Batch)] {
            let input = LaunchInput {
                batch,
                interactive: true,
                has_prompt: true,
            };
            assert_eq!(input.kind(), kind);
        }
    }

    #[test]
    fn reopening_keeps_bare_attribution_and_managed_conversations() {
        use crate::session::WorkSource;
        use crate::session_record::{SessionCaptureSpec, SessionGraphMembership, SessionRecorder};

        let ledger = crate::journal::TestLedgerGuard::new();
        crate::journal::with_runtime(ledger.home(), &["lf".into(), "resume".into()], || {
            let store = crate::journal::process_ledger().unwrap();
            // Attribution selects a launch kind independently of the stored
            // surface. Resume eligibility is checked before delegation.
            for (surface, source, bound, primary, expected) in [
                ("headless", None, false, false, LaunchKind::Chat),
                ("interactive", None, false, false, LaunchKind::Chat),
                (
                    "interactive",
                    Some(WorkSource::Checkout),
                    false,
                    false,
                    LaunchKind::Chat,
                ),
                (
                    "interactive",
                    Some(WorkSource::Declared),
                    false,
                    false,
                    LaunchKind::Chat,
                ),
                (
                    "interactive",
                    Some(WorkSource::Inherited),
                    false,
                    false,
                    LaunchKind::Chat,
                ),
                (
                    "interactive",
                    Some(WorkSource::Bound),
                    true,
                    false,
                    LaunchKind::WorkSession,
                ),
                (
                    "interactive",
                    Some(WorkSource::Declared),
                    false,
                    true,
                    LaunchKind::WorkSession,
                ),
            ] {
                let capture = SessionRecorder::begin_at(
                    ledger.home(),
                    SessionCaptureSpec {
                        harness: "claude".into(),
                        model: None,
                        surface: surface.into(),
                        cwd: ledger.home().into(),
                        repo: None,
                        worktree: None,
                        skill: None,
                        subjects: vec![],
                        graph: SessionGraphMembership::Independent,
                        work: None,
                    },
                )
                .unwrap();
                let session = store
                    .session_for_artifact(&capture.artifact_key())
                    .unwrap()
                    .unwrap();
                rusqlite::Connection::open(ledger.home().join("loopflow.db"))
                    .unwrap()
                    .execute(
                        "UPDATE agent_sessions SET work_source=?2,bound_at=?3,
                         primary_scope=CASE WHEN ?4 THEN 'repository' END,repo=?5 WHERE id=?1",
                        rusqlite::params![
                            session.id,
                            source.map(|value| serde_json::to_value(value)
                                .unwrap()
                                .as_str()
                                .unwrap()
                                .to_owned()),
                            bound.then_some(1),
                            primary,
                            ledger.home().to_str().unwrap(),
                        ],
                    )
                    .unwrap();
                let lock = tempfile::tempfile().unwrap();
                let (kind, _) = super::delegate_session(
                    &session.id,
                    Default::default(),
                    surface == "interactive",
                    lock,
                )
                .unwrap()
                .unwrap();
                assert_eq!(
                    kind, expected,
                    "surface={surface}, source={source:?}, bound={bound}, primary={primary}",
                );
                assert_eq!(
                    super::delegate_prepared_session(session.id.as_str(), Default::default())
                        .unwrap(),
                    Some(expected),
                    "prepared: source={source:?}, bound={bound}, primary={primary}",
                );
                // A Desktop pane or framed remote viewer has no terminal for
                // a chat to own; the holder keeps every Session it opens.
                for relay in ["LF_SERVER_STATUS", super::STREAM_ENV] {
                    std::env::set_var(relay, "1");
                    let relayed = super::delegate_session(
                        &session.id,
                        Default::default(),
                        true,
                        tempfile::tempfile().unwrap(),
                    );
                    std::env::remove_var(relay);
                    let (kind, _) = relayed.unwrap().unwrap();
                    assert_eq!(kind, LaunchKind::WorkSession, "{relay}: source={source:?}");
                }
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn reopening_with_an_unreadable_store_stays_managed() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let database = ledger.home().join("unreadable.db");
        std::fs::write(&database, "not a database").unwrap();
        ledger.set_db_path(database);
        assert_eq!(
            super::delegate_prepared_session("unreadable-session", Default::default()).unwrap(),
            Some(crate::harness::LaunchKind::WorkSession)
        );
    }
}
