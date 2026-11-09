//! A detached terminal owns an execution, including its capture and native client.
//! Viewers hold revocable input tokens, never provider attachment capabilities.

mod connection;
mod input;
#[cfg(test)]
mod input_retry_tests;
mod lifetime;
#[cfg(test)]
mod lifetime_tests;
mod native;
mod output;
pub(crate) use output::local_directory_report;
mod snapshot;
mod stream;
mod terminal;
#[cfg(test)]
mod viewer_tests;
mod wire;

use std::collections::VecDeque;
use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::harness::native_exit::{NativeExit, Settled};
use crate::id::{LfProcessId, LfSessionId};
use crate::store::sqlite::ViewerHold;
use input::PendingInput;
use wire::{Decoder, Event, Incoming, Outgoing, Own, Reply, Request, Resume, Scanner, Tag};

const OWNER_ENV: &str = "LF_TRANSPORT_OWNER";
const PARENT_ENV: &str = "LF_TRANSPORT_PARENT";
const CONTROL_ENV: &str = "LF_TRANSPORT_CONTROL";
const TERMINAL_ENV: &str = "LF_TRANSPORT_TERMINAL";
const LIMIT: usize = 4 * 1024 * 1024;
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) const STREAM_ENV: &str = "LF_VIEWER_STREAM";

const ATTACH_TIMEOUT: Duration = Duration::from_secs(5);
// SSH connection setup and destination Session resolution precede attachment.
const COMMAND_ATTACH_TIMEOUT: Duration = Duration::from_secs(60);
/// Continuous output tells the lifetime policy once per pause, not per read.
const OUTPUT_ACTIVITY_PAUSE: Duration = Duration::from_secs(1);

/// How long a dropped viewer's stream waits for a byte-counted resume.
const GRACE: Duration = Duration::from_secs(60);

/// How long a person's terminal waits for exit records before it gets the
/// provider's own exit back. A locked or refused store never holds a quit.
const SETTLE_WAIT: Duration = Duration::from_millis(800);

/// How long the owner process stays to finish exit records after its viewers
/// have returned. A resume waits this long for it before continuing.
pub(crate) const SETTLE_LINGER: Duration = Duration::from_secs(20);

/// The exit code a provider's status reports on its own.
fn provider_exit_code(status: std::process::ExitStatus) -> u8 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)) as u8
}

/// A handed-off exit being recorded off the serve loop. Its records wait on
/// the store; the person's terminal waits only `SETTLE_WAIT` for them, then
/// gets the provider's own exit back.
struct Settling {
    settled: mpsc::Receiver<Settled>,
    since: Instant,
    code: u8,
    process: String,
    /// The terminal has its exit; only the records are still being written.
    delivered: bool,
}

impl Settling {
    fn spawn(record: Box<NativeExit>, status: std::process::ExitStatus, waker: UnixStream) -> Self {
        let process = format!(
            "lf/{}",
            record
                .attachment
                .provider_lf_process_id
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
            code: provider_exit_code(status),
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
                tracing::warn!(process = self.process, "exit records are still being written; returning the terminal with the provider's own exit");
                Settled {
                    code: self.code,
                    lines: Vec::new(),
                }
            }
        };
        self.delivered = true;
        Some(settled)
    }

    /// Give records still being written time to land before the owner exits.
    fn linger(self) {
        let _ = self.settled.recv_timeout(SETTLE_LINGER);
    }
}
/// Stream bytes retained per viewer for resume.
const SPOOL: u64 = 1024 * 1024;
static HANDOFF: OnceLock<Handoff> = OnceLock::new();

#[derive(Debug)]
struct Handoff {
    owner: LfProcessId,
    key: String,
    parent: u32,
}

impl Handoff {
    /// Read and remove the environment handoff; only a direct child adopts it.
    fn take() -> Option<Self> {
        let parent = std::env::var(PARENT_ENV)
            .ok()
            .and_then(|p| p.parse::<u32>().ok());
        let owner = std::env::var(OWNER_ENV)
            .ok()
            .and_then(|id| LfProcessId::parse(&id).ok());
        let key = std::env::var(CONTROL_ENV).ok();
        std::env::remove_var(CONTROL_ENV);
        std::env::remove_var(PARENT_ENV);
        std::env::remove_var(OWNER_ENV);
        // SAFETY: getppid has no preconditions.
        let parent = parent.filter(|p| *p == unsafe { libc::getppid() } as u32)?;
        Some(Self {
            owner: owner?,
            key: key?,
            parent,
        })
    }

    fn pass(&self, command: &mut Command, parent: u32) {
        command
            .env(OWNER_ENV, self.owner.as_str())
            .env(PARENT_ENV, parent.to_string())
            .env(CONTROL_ENV, &self.key);
    }
}

/// Consume the direct execution handoff. Provider tools and nested lf calls do
/// not inherit the right to run inside this execution's owner.
pub fn initialize() {
    if let Some(handoff) = Handoff::take() {
        let _ = HANDOFF.set(handoff);
    }
}

pub(crate) fn current() -> Option<&'static LfProcessId> {
    HANDOFF.get().map(|handoff| &handoff.owner)
}

/// Delegate before capture admission or provider effects. A missing startup
/// acknowledgement is uncertainty, never permission to launch again.
pub fn delegate_current(batch: bool) -> Result<bool> {
    delegate_session(batch, None, None, true)
}

pub(crate) fn delegate_session(
    batch: bool,
    json_session: Option<&str>,
    preview_session: Option<&str>,
    replace: bool,
) -> Result<bool> {
    let mut argv: Vec<String> = std::env::args().collect();
    if json_session.is_some() {
        // The viewer owns this continuation, including an early detach.
        argv.retain(|arg| arg != "--json");
    }
    delegate(batch, json_session, preview_session, argv, replace)
}

/// Open the resulting Session after a fork or thread replacement, without
/// repeating the command that prepared it.
pub(crate) fn delegate_prepared_session(id: &str, json: bool) -> Result<bool> {
    delegate(
        false,
        json.then_some(id),
        Some(id),
        vec![String::new(), "session".into(), "resume".into(), id.into()],
        true,
    )
}

fn delegate(
    batch: bool,
    json_session: Option<&str>,
    preview_session: Option<&str>,
    mut argv: Vec<String>,
    replace: bool,
) -> Result<bool> {
    if current().is_some() {
        return Ok(false);
    }
    // Library tests do not execute the CLI entry point.
    #[cfg(test)]
    {
        let _ = (batch, json_session, preview_session, &mut argv, replace);
        Ok(false)
    }
    #[cfg(not(test))]
    {
        crate::journal::admit_process(
            &std::env::current_dir()?,
            &std::env::args().collect::<Vec<_>>(),
        );
        crate::journal::current_lf_process_id()
            .context("Execution requires an admitted invocation; check the process ledger and caller provenance")?;
        argv[0] = std::env::current_exe()?.to_string_lossy().into_owned();
        // `lf -b … > file` typed at a terminal writes only the answer, never a
        // relayed screen; an interactive run keeps the terminal it reads from.
        let terminal = std::env::var_os(STREAM_ENV).is_some()
            || io::stdout().is_terminal()
            || (!batch && io::stdin().is_terminal());
        let preview = if std::env::var_os(STREAM_ENV).is_some() {
            if let Some(screen) = preview_session.and_then(snapshot::last_screen) {
                io::stdout().write_all(&wire::frame(Tag::Snapshot, &screen))?;
            }
            None
        } else {
            ViewerTerminal::preview(preview_session)?
        };
        let cwd = std::env::current_dir()?;
        let owner = match preview_session {
            Some(session) => start_session(session, &argv, terminal, &cwd)?,
            None => start(&argv, terminal, &cwd, &|_| {})?,
        };
        let session = preview_session.map(LfSessionId::from);
        relay_with_preview(
            &owner,
            session.as_ref(),
            None,
            json_session,
            preview,
            replace,
        )?;
        Ok(true)
    }
}

/// Start or obtain the Session's existing terminal owner without becoming a
/// viewer. Ordinary Desktop delegation uses this same startup boundary. This
/// lock differs from the provider launch lock the child must acquire itself.
pub(crate) fn start_session(
    session: &str,
    argv: &[String],
    terminal: bool,
    cwd: &Path,
) -> Result<LfProcessId> {
    let _startup = crate::ops::human_session::lock_session_process(&format!("owner:{session}"))?;
    let registry =
        crate::store::read_existing_registry()?.context("Session registry is unavailable")?;
    let record = registry
        .sqlite
        .session(session)?
        .context("Session is unavailable")?;
    if let Some(owner) = registry.sqlite.session_transport(session)? {
        if crate::ops::human_session::attaches(&registry.sqlite, &record, &owner)? {
            return Ok(owner);
        }
    }
    let owner = start(argv, terminal, cwd, &|_| {})?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if registry.sqlite.session_transport(session)?.as_ref() == Some(&owner) {
            return Ok(owner);
        }
        anyhow::ensure!(crate::ops::human_session::owner_running(&registry.sqlite, &owner)?,
            "Session owner exited before opening the provider; inspect the Session before another start");
        anyhow::ensure!(
            Instant::now() < deadline,
            "Session startup has not been confirmed; delivery is unknown, do not repeat the launch"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn start(
    argv: &[String],
    terminal: bool,
    cwd: &Path,
    inherit: &dyn Fn(&mut Command),
) -> Result<LfProcessId> {
    let logs = crate::store::lf_home_dir().join("logs");
    std::fs::create_dir_all(&logs)
        .with_context(|| format!("Cannot open execution transport log in {}", logs.display()))?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("transport.log"))
        .with_context(|| format!("Cannot open execution transport log in {}", logs.display()))?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["session", "serve"])
        .env_remove("LF_SERVER_STATUS")
        .env_remove(STREAM_ENV);
    if terminal {
        command.arg("--terminal");
    }
    command
        .arg("--")
        .args(argv)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log);
    inherit(&mut command);
    // Validate placement on the viewer's PTY before handing it to the owner,
    // whose stdin is deliberately detached.
    command.env_remove(TERMINAL_ENV);
    if let Some(terminal) = crate::session_record::current_terminal_id() {
        command.env(TERMINAL_ENV, terminal);
    }
    // SAFETY: only an async-signal-safe syscall is used between fork and exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let output = child.stdout.take().expect("startup pipe configured");
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    let mut line = String::new();
    BufReader::new(output)
        .read_line(&mut line)
        .context("Execution handoff was not acknowledged; delivery is unknown, do not replay")?;
    LfProcessId::parse(line.trim())
        .context("Owner exited before acknowledging execution; inspect transport.log")
}

pub(crate) fn directory(home: &Path) -> Result<PathBuf> {
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

pub(crate) fn endpoint(id: &LfProcessId) -> Result<PathBuf> {
    Ok(private_directory()?.join(format!("{id}.sock")))
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

fn nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl operates on a borrowed live descriptor without taking ownership.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
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
    // executed command from inheriting the owner's master or spare slave.
    unsafe {
        libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(slave, libc::F_SETFD, libc::FD_CLOEXEC);
    }
    // SAFETY: successful openpty returned two newly owned descriptors.
    Ok(unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) })
}

fn dimensions(rows: u16, cols: u16) -> (u16, u16) {
    // An unopened terminal may report zero. The owner's terminal needs room for
    // a wide glyph and a previous row when wrapping, even in a collapsed pane.
    (
        if rows == 0 { 24 } else { rows.clamp(2, 500) },
        if cols == 0 { 80 } else { cols.clamp(2, 500) },
    )
}

pub(crate) fn resize(fd: RawFd, rows: u16, cols: u16) -> io::Result<()> {
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

/// The last `SPOOL` bytes of one viewer's stream, so a resume continues from
/// the exact byte without a snapshot.
#[derive(Debug, Default)]
struct Spool {
    chunks: VecDeque<(Tag, Vec<u8>)>,
    /// Stream offset of the first retained byte.
    base: u64,
    /// Stream offset after the last byte.
    total: u64,
}

impl Spool {
    fn push(&mut self, tag: Tag, bytes: &[u8]) {
        self.total += bytes.len() as u64;
        self.chunks.push_back((tag, bytes.to_vec()));
        while self.total - self.base > SPOOL {
            let (_, front) = self.chunks.pop_front().expect("spool exceeds its limit");
            self.base += front.len() as u64;
        }
    }

    fn holds(&self, byte: u64) -> bool {
        (self.base..=self.total).contains(&byte)
    }

    /// The stream from byte `from` on; `from` must be held.
    fn replay(&self, from: u64) -> impl Iterator<Item = (Tag, &[u8])> {
        let mut at = self.base;
        self.chunks.iter().filter_map(move |(tag, bytes)| {
            let start = at;
            at += bytes.len() as u64;
            (at > from).then(|| (*tag, &bytes[from.saturating_sub(start) as usize..]))
        })
    }
}

#[derive(Debug)]
enum Connection {
    Open(UnixStream),
    /// The peer stopped reading; its frames still queued are read out until
    /// end of stream, and nothing more is written.
    Draining(UnixStream),
    /// Lost at this instant; the stream waits for a resume until `GRACE` passes.
    Dropped(Instant),
}

impl Connection {
    fn writable(&mut self) -> Option<&mut UnixStream> {
        match self {
            Self::Open(socket) => Some(socket),
            Self::Draining(_) | Self::Dropped(_) => None,
        }
    }

    fn readable(&mut self) -> Option<&mut UnixStream> {
        match self {
            Self::Open(socket) | Self::Draining(socket) => Some(socket),
            Self::Dropped(_) => None,
        }
    }

    fn is_open(&self) -> bool {
        matches!(self, Self::Open(_))
    }

    fn is_dropped(&self) -> bool {
        matches!(self, Self::Dropped(_))
    }

    fn expired(&self) -> bool {
        matches!(self, Self::Dropped(at) if at.elapsed() >= GRACE)
    }
}

#[derive(Debug)]
struct Viewer {
    connection: Connection,
    decoder: Decoder,
    /// Encoded frames not yet written.
    output: VecDeque<u8>,
    spool: Spool,
    /// Public identity, named to others when it takes ownership.
    id: String,
    /// Secret a resume presents.
    reconnect: String,
    /// Tags this viewer's events; a program printing the same bytes lacks it.
    nonce: String,
    /// The input claim, while this connection may type.
    token: Option<String>,
    attached: bool,
    heartbeat_at: SystemTime,
    seq: u64,
    /// Events awaiting a sequence boundary in the stream.
    events: Vec<Vec<u8>>,
    size: (u16, u16),
    theme: Option<serde_json::Value>,
    name: Option<String>,
    leaving: bool,
    /// A resume this connection sent, moved onto the dropped viewer's stream.
    resuming: Option<(u64, Resume)>,
    /// The `native_ended` request awaiting its acknowledgement.
    ending: Option<u64>,
    input_hint: Option<String>,
}

impl Viewer {
    fn new(socket: UnixStream) -> Self {
        let token = || uuid::Uuid::new_v4().simple().to_string();
        Self {
            connection: Connection::Open(socket),
            decoder: Decoder::default(),
            output: VecDeque::new(),
            spool: Spool::default(),
            id: token()[..12].to_owned(),
            reconnect: token(),
            nonce: token()[..16].to_owned(),
            token: None,
            attached: false,
            heartbeat_at: SystemTime::now(),
            seq: 0,
            events: Vec::new(),
            size: (24, 80),
            theme: None,
            name: None,
            leaving: false,
            resuming: None,
            ending: None,
            input_hint: None,
        }
    }

    fn live(&self) -> bool {
        self.attached && !self.connection.is_dropped() && !self.leaving
    }

    fn expire_heartbeat(&mut self, now: SystemTime) {
        if self.live()
            && now.duration_since(self.heartbeat_at).unwrap_or_default() >= HEARTBEAT_TIMEOUT
        {
            self.drop_connection();
        }
    }

    /// Append `bytes` to this viewer's stream; a dropped viewer only spools them.
    fn stream(&mut self, tag: Tag, bytes: &[u8]) -> io::Result<()> {
        for chunk in bytes.chunks(wire::CHUNK) {
            self.spool.push(tag, chunk);
            if self.connection.is_open() {
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
    fn flush_events(&mut self) -> io::Result<()> {
        for bytes in std::mem::take(&mut self.events) {
            self.stream(Tag::Out, &bytes)?;
        }
        Ok(())
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

    fn attached_reply(&self, owner: Option<&str>, terminal: bool) -> Reply {
        Reply::Attached {
            viewer: self.id.clone(),
            reconnect: self.reconnect.clone(),
            nonce: self.nonce.clone(),
            token: self.token.clone(),
            terminal,
            grace_ms: GRACE.as_millis() as u64,
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
                self.connection = match std::mem::replace(
                    &mut self.connection,
                    Connection::Dropped(Instant::now()),
                ) {
                    Connection::Open(socket) => Connection::Draining(socket),
                    other => other,
                };
                Ok(())
            }
            written => written,
        }
    }

    fn write_output(&mut self) -> io::Result<()> {
        let Some(socket) = self.connection.writable() else {
            self.output.clear();
            return Ok(());
        };
        while !self.output.is_empty() {
            match socket.write(self.output.as_slices().0) {
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
        let Some(socket) = self.connection.readable() else {
            return Ok(());
        };
        let mut bytes = [0; 8192];
        match socket.read(&mut bytes) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => self.decoder.extend(&bytes[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Lose the connection and every claim; keep the stream for a resume.
    fn drop_connection(&mut self) {
        self.connection = Connection::Dropped(Instant::now());
        self.output.clear();
        self.decoder = Decoder::default();
        self.token = None;
    }

    /// Continue on `socket` from byte `received`, replying to request `answers`.
    fn resume(
        &mut self,
        socket: UnixStream,
        answers: u64,
        received: u64,
        owner: Option<&str>,
        terminal: bool,
    ) {
        self.connection = Connection::Open(socket);
        self.heartbeat_at = SystemTime::now();
        self.decoder = Decoder::default();
        self.output.clear();
        self.reply(answers, self.attached_reply(owner, terminal));
        for (tag, bytes) in self.spool.replay(received) {
            wire::encode(tag, bytes, &mut self.output);
        }
    }
}

/// Whether a viewer whose connection failed stays for a resume: only a lost
/// connection qualifies, never a protocol error or a viewer that fell behind.
fn survive(viewer: &mut Viewer, error: &io::Error) -> bool {
    if lost(error) && viewer.live() {
        viewer.drop_connection();
        return true;
    }
    false
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

/// Whether a viewer stays after a stream write.
fn keep(viewer: &mut Viewer, written: io::Result<()>) -> bool {
    match written {
        Ok(()) => true,
        Err(error) => survive(viewer, &error),
    }
}

/// Whose size and colors the terminal takes, and who waits for that.
#[derive(Debug, Default)]
struct Ownership {
    owner: Option<Owner>,
    requests: VecDeque<String>,
}

#[derive(Debug, Clone)]
struct Owner {
    id: String,
    name: Option<String>,
}

impl Owner {
    fn of(viewer: &Viewer) -> Self {
        Self {
            id: viewer.id.clone(),
            name: viewer.name.clone(),
        }
    }
}

impl Ownership {
    fn id(&self) -> Option<&str> {
        self.owner.as_ref().map(|owner| owner.id.as_str())
    }

    fn held_by(&self, viewer: &Viewer) -> bool {
        self.id() == Some(&viewer.id)
    }

    /// Apply a viewer's claim; whether the owner changed. `take` replaces the
    /// owner; `request` becomes owner when there is none, else waits in line.
    fn claim(&mut self, mode: Option<Own>, viewer: &Viewer) -> bool {
        match mode {
            Some(Own::Take) if !self.held_by(viewer) => {}
            Some(Own::Request) if self.owner.is_none() => {}
            Some(Own::Request) if !self.held_by(viewer) && !self.requests.contains(&viewer.id) => {
                self.requests.push_back(viewer.id.clone());
                return false;
            }
            _ => return false,
        }
        self.requests.retain(|id| id != &viewer.id);
        self.owner = Some(Owner::of(viewer));
        true
    }

    /// Ownership follows live viewers: a dropped or departed owner passes to
    /// the first waiting request. Whether the owner changed.
    fn settle(&mut self, viewers: &[Viewer]) -> bool {
        let find = |id: &str| {
            viewers
                .iter()
                .find(|viewer| viewer.id == id && viewer.live())
        };
        self.requests.retain(|id| find(id).is_some());
        let mut changed = false;
        if self
            .owner
            .as_ref()
            .is_some_and(|owner| find(&owner.id).is_none())
        {
            self.owner = None;
            changed = true;
        }
        if self.owner.is_none() {
            if let Some(next) = self.requests.pop_front().and_then(|id| find(&id)) {
                self.owner = Some(Owner::of(next));
                changed = true;
            }
        }
        changed
    }

    /// What `viewer` is told about who owns.
    fn event(&self, viewer: &Viewer) -> Event {
        Event::Owner {
            viewer: self.owner.as_ref().map(|owner| owner.id.clone()),
            name: self.owner.as_ref().and_then(|owner| owner.name.clone()),
            held: self.held_by(viewer),
        }
    }
}

struct OwnerSocket(PathBuf);
impl Drop for OwnerSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Own a command's transport until it exits; only `start` launches this.
pub fn serve(argv: &[String], terminal: bool) -> Result<()> {
    let placement = std::env::var(TERMINAL_ENV).ok();
    std::env::remove_var(TERMINAL_ENV);
    let id =
        crate::journal::current_lf_process_id().context("Transport needs an admitted Process")?;
    let shell = std::env::var("LF_DESKTOP_SHELL").ok();
    std::env::remove_var("LF_DESKTOP_SHELL");
    if let Some(shell) = &shell {
        crate::journal::process_ledger()?.bind_desktop_shell(shell, &id)?;
    }
    let path = endpoint(&id).context("Prepare transport socket")?;
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("Bind transport socket {}", path.display()))?;
    listener.set_nonblocking(true)?;
    let _socket_file = OwnerSocket(path.clone());
    let socket = path.clone();
    crate::agent::register_interrupt_cleanup(move || {
        let _ = std::fs::remove_file(&socket);
    });
    let control_key = uuid::Uuid::new_v4().to_string();
    let mut command = Command::new(&argv[0]);
    command.env(CONTROL_ENV, &control_key);
    command
        .args(&argv[1..])
        .env(OWNER_ENV, id.as_str())
        .env(PARENT_ENV, std::process::id().to_string());
    let pair = terminal.then(terminal_pair).transpose()?;
    command
        .env_remove("LF_TERMINAL_ID")
        .env_remove("LF_TERMINAL_TTY");
    if let (Some(placement), Some((_, slave))) = (placement, &pair) {
        if let Some(tty) = tty_name(slave.as_raw_fd()) {
            // This owned PTY continues the validated viewer placement. Native
            // admission can now associate its first Session with Desktop.
            command
                .env("LF_TERMINAL_ID", placement)
                .env("LF_TERMINAL_TTY", tty);
        }
    }
    // A native client that dies raw is returned to these modes.
    let cooked = pair.as_ref().and_then(|(master, _)| {
        // SAFETY: tcgetattr writes into a live termios; the descriptor is borrowed.
        unsafe {
            let mut termios = std::mem::zeroed::<libc::termios>();
            (libc::tcgetattr(master.as_raw_fd(), &mut termios) == 0).then_some(termios)
        }
    });
    if let Some((_, slave)) = &pair {
        command
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
    } else {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.process_group(0);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            if shell.is_some() {
                crate::journal::process_ledger()?.settle_shell_jobs(&id)?;
            }
            return Err(error.into());
        }
    };
    // Command retains its configured slave descriptors after spawn. Close them
    // so Linux reports PTY EOF when the execution closes its own descriptors.
    drop(command);
    let group = crate::os_process::ProcessGroupGuard::new(child.id());
    let shell_jobs = shell
        .as_ref()
        .map(|_| crate::os_process::TerminalJobs::new(child.id()));
    if let Some(jobs) = &shell_jobs {
        let jobs = jobs.clone();
        let owner = id.clone();
        crate::agent::register_interrupt_cleanup(move || {
            let settled = jobs
                .terminate()
                .map_err(anyhow::Error::from)
                .and_then(|()| Ok(crate::journal::process_ledger()?.settle_shell_jobs(&owner)?));
            if let Err(error) = settled {
                tracing::error!(%owner,%error,"Shell job cleanup remains pending");
            }
        });
    }
    let mut readers: Vec<(u8, File)> = Vec::new();
    let input: File;
    if let Some((master, slave)) = pair {
        drop(slave);
        input = master.try_clone()?;
        readers.push((1, master));
    } else {
        input = OwnedFd::from(child.stdin.take().expect("piped stdin")).into();
        readers.push((
            1,
            OwnedFd::from(child.stdout.take().expect("piped stdout")).into(),
        ));
        readers.push((
            2,
            OwnedFd::from(child.stderr.take().expect("piped stderr")).into(),
        ));
    }
    for (_, reader) in &readers {
        nonblocking(reader.as_raw_fd())?;
    }
    nonblocking(input.as_raw_fd())?;
    let mut input = Some(input);
    let mut end_input = false;
    // The child and its capture now belong to this owner, even if acknowledgement is lost.
    let _ = writeln!(io::stdout(), "{id}");
    let _ = io::stdout().flush();
    let mut viewers: Vec<Viewer> = Vec::new();
    // Who may type.
    let mut control: Option<String> = None;
    let mut ownership = Ownership::default();
    let mut size = (24, 80);
    let mut pending_input = PendingInput::default();
    let mut ending_failure = crate::store::RecordingGap::default();
    let mut output = output::Output::new(terminal)?;
    let mut exit: Option<(u8, Instant)> = None;
    let mut detach = false;
    let mut client: Option<native::NativeClient> = None;
    let mut returning = None;
    let mut hint_due = false;
    let mut announced = false;
    let mut attached = false;
    // The lifetime policy and a blocking child wait wake the descriptor loop.
    // Keep SIGCHLD available to runtimes used by the owner's background work.
    let (wake, waker) = UnixStream::pair()?;
    wake.set_nonblocking(true)?;
    waker.set_nonblocking(true)?;
    let (activity, ending) = lifetime::spawn(id.clone(), terminal, waker.try_clone()?);
    let child_pid = child.id();
    let settle_waker = waker.try_clone()?;
    let (finished, child_exit) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = finished.send(child.wait());
        let _ = (&waker).write_all(&[1]);
    });
    let mut settling: Option<Settling> = None;
    let mut observed = lifetime::Activity::default();
    let mut output_told: Option<Instant> = None;
    let mut exit_record: Option<Box<NativeExit>> = None;
    let mut execution_directory_reported = false;
    let mut planning = None;
    let mut reported_status = None;
    let mut title = terminal.then(|| crate::terminal_title::OutputTitle::new(&id));
    loop {
        #[cfg(test)]
        input_retry_tests::poll();
        #[cfg(test)]
        lifetime_tests::before_read(&id, &mut readers, &mut output_told)?;
        let mut replaced = Vec::new();
        let mut owner_change = false;
        let mut size_change = false;
        let mut theme_change = false;
        let mut attaching = Vec::new();
        while let Ok((socket, _)) = listener.accept() {
            socket.set_nonblocking(true)?;
            viewers.push(Viewer::new(socket));
        }
        // Observe every connection before admitting claims. A viewer earlier
        // in the list must not see Busy for a controller whose EOF is ready.
        viewers.retain_mut(|viewer| {
            viewer.expire_heartbeat(SystemTime::now());
            match viewer.read() {
                Ok(()) => !viewer.connection.expired(),
                Err(error) => survive(viewer, &error),
            }
        });
        owner_change |= ownership.settle(&viewers);
        if !viewers
            .iter()
            .any(|viewer| viewer.token.is_some() && viewer.token == control)
        {
            control = None;
        }
        viewers.retain_mut(|viewer| {
            let result = (|| -> Result<()> {
                while let Some((tag, payload)) = viewer.decoder.next()? {
                    let request = match tag {
                        Tag::Input => {
                            // Stale and passive viewers cannot write.
                            if viewer.token.is_some() && viewer.token == control && !end_input {
                                anyhow::ensure!(
                                    pending_input.push(&payload),
                                    "Provider input is busy"
                                );
                            }
                            continue;
                        }
                        Tag::Request => serde_json::from_slice::<Incoming>(&payload)?,
                        _ => bail!("unexpected frame from viewer"),
                    };
                    let Incoming {
                        id: answers,
                        request,
                    } = request;
                    match request {
                        Request::Status {
                            key,
                            program,
                            state,
                            progress,
                        } if key == control_key => {
                            output.report(program, state, progress);
                        }
                        Request::DetachViewers { key } if key == control_key => {
                            detach = true;
                        }
                        Request::Directory { key, cwd } if key == control_key => {
                            output.directory(&cwd);
                        }
                        Request::NativeStarted { key, exit, detach }
                            if key == control_key && terminal =>
                        {
                            returning = Some(Event::Return {
                                detach: detach.clone(),
                            });
                            client = Some(native::NativeClient::new(exit, detach, cooked));
                        }
                        Request::NativeEnded { key } if key == control_key => {
                            viewer.ending = Some(answers);
                        }
                        Request::InputHint { key, hint } if key == control_key => {
                            viewer.input_hint = Some(hint);
                            hint_due = true;
                        }
                        Request::Controlled => {
                            if viewer.attached {
                                viewer.heartbeat_at = SystemTime::now();
                            }
                            viewer.reply(
                                answers,
                                Reply::Controlled {
                                    held: control.is_some(),
                                },
                            );
                        }
                        Request::ExitRecord { key, pid, record } if key == control_key => {
                            // Graph steps share this terminal, but their Graph
                            // still owns their wait. Only our child can hand it off.
                            if pid != child_pid {
                                viewer.reply(answers, Reply::Busy);
                                continue;
                            }
                            // The launcher kept its repository's planning in
                            // sync while it waited; that wait is now ours.
                            planning = crate::ops::planning_sync::PlanningSync::start_for_directory(
                                &record.cwd,
                            )
                            .inspect_err(|error| {
                                tracing::warn!(%error, "planning sync unavailable to this owner")
                            })
                            .ok();
                            exit_record = Some(record);
                            viewer.reply(answers, Reply::Accepted);
                        }
                        Request::DropExitRecord { key } if key == control_key => {
                            exit_record = None;
                            planning = None;
                            viewer.reply(answers, Reply::Accepted);
                        }
                        Request::Attach {
                            control: wants_control,
                            replace,
                            own,
                            rows,
                            cols,
                            name,
                            moved,
                        } => {
                            if wants_control
                                && !replace
                                && control.is_some()
                                && viewer.token != control
                            {
                                viewer.reply(answers, Reply::Busy);
                                continue;
                            }
                            attached = true;
                            viewer.size = dimensions(rows, cols);
                            viewer.name = name;
                            if wants_control {
                                hint_due = true;
                                if viewer.token != control {
                                    if let Some(token) = &control {
                                        replaced.push((token.clone(), moved));
                                    }
                                }
                                let token = uuid::Uuid::new_v4().to_string();
                                control = Some(token.clone());
                                viewer.token = Some(token);
                            } else {
                                if viewer.token.is_some() && viewer.token == control {
                                    control = None;
                                }
                                viewer.token = None;
                            }
                            viewer.attached = true;
                            viewer.heartbeat_at = SystemTime::now();
                            attaching.push(viewer.id.clone());
                            owner_change |= ownership.claim(own, viewer);
                            viewer.reply(answers, viewer.attached_reply(ownership.id(), terminal));
                            // Binding selects a directory for later connections;
                            // the running provider keeps its own.
                            if let Some(cwd) = exit_record
                                .as_deref()
                                .filter(|_| execution_directory_reported)
                                .and_then(selected_directory)
                            {
                                output.directory(&cwd);
                            }
                            if let Some(cwd) = output.current_directory() {
                                viewer.event(Event::Directory {
                                    cwd: cwd.to_path_buf(),
                                });
                            }
                            if let Some(snapshot) = output.snapshot()? {
                                viewer.stream(Tag::Snapshot, &snapshot)?;
                            }
                            for (stream, bytes) in output.tail() {
                                viewer.stream(Tag::stream(stream), bytes)?;
                            }
                        }
                        Request::Resume(resume) => {
                            viewer.resuming = Some((answers, resume));
                        }
                        Request::EndInput
                            if !terminal && viewer.token.is_some() && viewer.token == control =>
                        {
                            end_input = true;
                        }
                        Request::Resize { rows, cols } => {
                            viewer.size = dimensions(rows, cols);
                            size_change |= ownership.held_by(viewer);
                        }
                        Request::Own { mode } if viewer.attached => {
                            owner_change |= ownership.claim(Some(mode), viewer);
                        }
                        Request::Theme { theme } if viewer.attached => {
                            viewer.theme = Some(theme);
                            theme_change |= ownership.held_by(viewer);
                        }
                        Request::Leave => {
                            viewer.leaving = true;
                        }
                        _ => {} // stale and passive viewers cannot release another claim
                    }
                }
                viewer.flush()?;
                Ok(())
            })();
            match result {
                Ok(()) => !viewer.leaving,
                Err(error) => error
                    .downcast_ref()
                    .is_some_and(|error| survive(viewer, error)),
            }
        });
        // Release dropped connections' claims before a same-pass resume asks
        // for them again; reconnecting restores the stream, not its old claims.
        viewers.retain(|viewer| !viewer.connection.expired());
        owner_change |= ownership.settle(&viewers);
        if !viewers
            .iter()
            .any(|viewer| viewer.token.is_some() && viewer.token == control)
        {
            control = None;
        }
        // A resume moves its connection onto the dropped viewer's stream.
        for index in 0..viewers.len() {
            let Some((answers, resume)) = viewers[index].resuming.take() else {
                continue;
            };
            let target = viewers.iter().position(|viewer| {
                viewer.connection.is_dropped()
                    && viewer.reconnect == resume.reconnect
                    && viewer.spool.holds(resume.received)
            });
            let Some(target) = target else {
                viewers[index].reply(answers, Reply::ReconnectRequired);
                let _ = viewers[index].flush();
                continue;
            };
            viewers[index].leaving = true;
            // A resuming peer that already closed has nothing to continue on.
            let Connection::Open(socket) = std::mem::replace(
                &mut viewers[index].connection,
                Connection::Dropped(Instant::now()),
            ) else {
                continue;
            };
            let viewer = &mut viewers[target];
            viewer.size = dimensions(resume.rows, resume.cols);
            if resume.control && control.is_none() {
                let token = uuid::Uuid::new_v4().to_string();
                control = Some(token.clone());
                viewer.token = Some(token);
                hint_due = true;
            }
            owner_change |= ownership.claim(resume.own, viewer);
            viewer.resume(socket, answers, resume.received, ownership.id(), terminal);
        }
        viewers.retain(|viewer| !viewer.leaving);
        owner_change |= ownership.settle(&viewers);
        let owner = viewers.iter().find(|viewer| ownership.held_by(viewer));
        let new_size = owner
            .filter(|_| owner_change || size_change)
            .map(|viewer| viewer.size);
        let theme = owner
            .filter(|_| owner_change || theme_change || !attaching.is_empty())
            .and_then(|viewer| viewer.theme.clone());
        if let Some((rows, cols)) = new_size {
            size = (rows, cols);
            if terminal {
                let fd = input
                    .as_ref()
                    .expect("terminal input stays open")
                    .as_raw_fd();
                resize(fd, rows, cols)?;
                output.resize(rows, cols)?;
                // Resizing to the same dimensions sends no signal. Ask the actual
                // foreground program to repaint too.
                // SAFETY: the descriptor is our PTY; only its current foreground
                // group receives the nonterminating signal.
                unsafe {
                    let foreground = libc::tcgetpgrp(fd);
                    if foreground > 1 {
                        libc::kill(-foreground, libc::SIGWINCH);
                    }
                }
            }
        }
        for viewer in viewers.iter_mut().filter(|viewer| viewer.attached) {
            // Fresh streams and ownership changes share the same appearance.
            let fresh = attaching.contains(&viewer.id);
            if new_size.is_some() || fresh {
                viewer.event(Event::Resize {
                    rows: size.0,
                    cols: size.1,
                });
            }
            if owner_change || theme_change || fresh {
                if let Some(theme) = &theme {
                    viewer.event(Event::Theme {
                        theme: theme.clone(),
                    });
                }
            }
            if owner_change || fresh {
                let event = ownership.event(viewer);
                viewer.event(event);
            }
            if fresh {
                if let Some(status) = output.program_status() {
                    viewer.event(Event::ProgramStatus {
                        status: status.clone(),
                    });
                }
                if let Some(client) = &client {
                    viewer.event(Event::Return {
                        detach: client.detach.clone(),
                    });
                }
                if announced {
                    if let Some((code, _)) = exit {
                        viewer.event(Event::ChildExited { code });
                    }
                }
            }
        }
        // The viewer that held control learns where it went; LOO-618's viewer
        // leaves on that, a shared viewer may stay.
        for (token, notice) in replaced {
            for viewer in viewers
                .iter_mut()
                .filter(|viewer| viewer.token.as_ref() == Some(&token))
            {
                viewer.token = None;
                viewer.event(Event::Control {
                    held: false,
                    notice: notice.clone(),
                });
            }
        }
        if let Some(event) = returning.take() {
            for viewer in viewers.iter_mut().filter(|viewer| viewer.attached) {
                viewer.event(event.clone());
            }
        }
        // The controls reach the one controlling viewer when it attaches or
        // when the execution registers them, whichever comes later.
        if std::mem::take(&mut hint_due) {
            let hint = viewers.iter().rev().find_map(|v| v.input_hint.clone());
            if let Some(hint) = hint {
                for viewer in viewers
                    .iter_mut()
                    .filter(|v| v.attached && v.token.is_some() && v.token == control)
                {
                    viewer.event(Event::Hint { text: hint.clone() });
                }
            }
        }
        drain(&wake);
        let detached = !viewers
            .iter()
            .any(|v| v.token.is_some() && v.token == control);
        if detached {
            control = None;
        }
        if let Some(name) = title.as_mut().and_then(|title| title.tick()) {
            output.title(&name);
        }
        if let Some(input) = input.as_mut() {
            // Typing at an idle shell prompt is not an agent draft: retain only
            // input that work running inside the shell could receive.
            let record = terminal
                && (shell.is_none()
                    || pending_input.is_empty()
                    || crate::desktop::shells::holds_input(&id));
            pending_input.forward(input, &id, record);
        }
        if end_input && pending_input.is_empty() {
            input.take();
        }
        let mut drained = Vec::new();
        for (stream, reader) in &mut readers {
            let mut bytes = [0; 16384];
            // Bounded work per stream keeps control responsive under continuous output.
            for _ in 0..16 {
                let n = match reader.read(&mut bytes) {
                    Ok(0) => {
                        drained.push(*stream);
                        break;
                    }
                    Ok(n) => n,
                    Err(e) if e.raw_os_error() == Some(libc::EIO) => {
                        drained.push(*stream);
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e.into()),
                };
                observed.output += 1;
                // An exec handoff announces before exec. The provider's first
                // output establishes that it reached its configured directory.
                if !execution_directory_reported {
                    if let Some(record) = &exit_record {
                        let connected = crate::journal::process_ledger()
                            .and_then(|store| {
                                store.agent_process(&record.attachment.agent_process_id)
                            })
                            .ok()
                            .flatten()
                            .is_some_and(|process| {
                                process.pid.is_some() && process.completed_at.is_none()
                            });
                        if connected {
                            output.directory(&record.cwd);
                            execution_directory_reported = true;
                        }
                    }
                }
                let ready = match client.as_mut().filter(|_| terminal) {
                    Some(client) => client.accept(reader.as_raw_fd(), &bytes[..n]),
                    None => bytes[..n].to_vec(),
                };
                deliver(
                    &mut output,
                    &mut viewers,
                    &mut pending_input,
                    *stream,
                    &ready,
                    detached,
                );
                if output_told.is_none_or(|told| told.elapsed() >= OUTPUT_ACTIVITY_PAUSE) {
                    output_told = Some(Instant::now());
                    let _ = activity.send(observed);
                }
            }
        }
        readers.retain(|(stream, _)| !drained.contains(stream));
        // After the reads: a provider that prints once and waits wakes nothing further.
        publish_reports(&mut output, &mut viewers);
        place_events(&output, &mut viewers, announced);
        // Output written before the execution detached reaches its viewers first.
        if detach {
            control = None;
            end_viewers(&mut viewers, Event::Detached);
            // The execution's own control connections outlive its viewers.
            viewers.retain(|v| !v.attached);
            detach = false;
        }
        if let Some(held) = client.as_mut().map(native::NativeClient::expire) {
            deliver(
                &mut output,
                &mut viewers,
                &mut pending_input,
                1,
                &held,
                detached,
            );
        }
        // The child wait reports once. A returning launcher announces its
        // client's end; an exec'd launcher leaves that announcement to our wait.
        let status = child_exit.try_recv().ok().transpose()?;
        if viewers.iter().any(|viewer| viewer.ending.is_some())
            || (status.is_some() && exit_record.is_some())
        {
            pending_input.native_ended(&id);
            if let Some(ended) = client.take() {
                finish_client(
                    ended,
                    &mut readers,
                    &mut output,
                    &mut viewers,
                    &mut pending_input,
                    detached,
                );
            }
            for viewer in viewers.iter_mut() {
                let Some(answers) = viewer.ending.take() else {
                    continue;
                };
                viewer.reply(answers, Reply::NativeEnded);
                let _ = viewer.flush();
            }
        }
        if let Some(status) = status {
            if let Some(jobs) = &shell_jobs {
                jobs.terminate()?;
                crate::journal::process_ledger()?.settle_shell_jobs(&id)?;
            }
            if let Some(record) = exit_record.take() {
                settling = Some(Settling::spawn(record, status, settle_waker.try_clone()?));
            } else {
                // A launch that died before reporting its client's end.
                if let Some(held) = client.take().map(native::NativeClient::release) {
                    deliver(
                        &mut output,
                        &mut viewers,
                        &mut pending_input,
                        1,
                        &held,
                        detached,
                    );
                }
                exit = Some((provider_exit_code(status), Instant::now()));
            }
        }
        let delivered = settling.as_mut().and_then(|settling| {
            let settled = settling.poll()?;
            Some((settled, settling.process.clone()))
        });
        if let Some((settled, process)) = delivered {
            for line in &settled.lines {
                deliver(
                    &mut output,
                    &mut viewers,
                    &mut pending_input,
                    1,
                    format!("\r\n{line}\r\n").as_bytes(),
                    detached,
                );
            }
            output.report(
                format!("{process}/step"),
                crate::program_status::State::Clear,
                None,
            );
            output.report(
                process,
                match settled.code {
                    0 => crate::program_status::State::Done,
                    130 => crate::program_status::State::Idle,
                    _ => crate::program_status::State::Error,
                },
                None,
            );
            planning = None;
            exit = Some((settled.code, Instant::now()));
        }
        // Include held native output and exit reports before announcing the end.
        if output.ready() && output.program_status() != reported_status.as_ref() {
            reported_status = output.program_status().cloned();
            if let Some(status) = &reported_status {
                for viewer in viewers.iter_mut().filter(|viewer| viewer.attached) {
                    viewer.event(Event::ProgramStatus {
                        status: status.clone(),
                    });
                }
                place_events(&output, &mut viewers, announced);
            }
        }
        if let Some((code, at)) = exit {
            if !announced && readers.is_empty() && at.elapsed() > Duration::from_millis(50) {
                end_viewers(&mut viewers, Event::ChildExited { code });
                announced = true;
            }
            // Once viewers have received the final output, release execution
            // locks with the owner. Only never-viewed starts need a replay window.
            if announced
                && (at.elapsed() > Duration::from_secs(2)
                    || (attached && viewers.iter().all(|viewer| viewer.output.is_empty())))
            {
                group.disarm();
                let _ = std::fs::remove_file(path);
                release_viewer_hold(&id, observed.hold);
                snapshot::leave(&id, &output);
                // Records still being written finish before this process exits.
                if let Some(settling) = settling.take() {
                    settling.linger();
                }
                return if code == 0 {
                    Ok(())
                } else {
                    Err(crate::process::CommandExit(code).into())
                };
            }
        }
        // Ready output and viewer changes precede the final ending judgment.
        // A throttled/unconsumed activity report also invalidates the candidate.
        let viewed = observed.view(viewers.iter().any(|v| v.live()));
        if observed.hold(viewer_hold(
            &viewers,
            observed.hold,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )) | viewed
        {
            let _ = activity.send(observed);
        }
        #[cfg(test)]
        lifetime_tests::before_judgment();
        if let Ok(ending) = ending.try_recv() {
            let event = lifetime::report_result(
                &mut ending_failure,
                "Session ending",
                ending.confirm(&id, observed, pending_input.holds_owner()),
            )
            .flatten();
            if let Some(event) = event {
                if let Some(jobs) = &shell_jobs {
                    jobs.terminate()?;
                    crate::journal::process_ledger()?.settle_shell_jobs(&id)?;
                }
                end_viewers(&mut viewers, event);
                release_viewer_hold(&id, observed.hold);
                snapshot::leave(&id, &output);
                crate::platform::kill_process(std::process::id());
            }
            // Flush even throttled activity so the applicable quiet interval
            // starts again and a settled owner can eventually end.
            if ending.activity != observed {
                let _ = activity.send(observed);
            }
        }
        #[cfg(test)]
        lifetime_tests::after_judgment(&output, observed)?;
        // Sleep until a descriptor has work or held output, exit announcement,
        // input retention or disconnected-viewer grace needs another attempt.
        let deadline = match exit {
            Some((_, at)) if announced => Some(at + Duration::from_secs(2)),
            Some((_, at)) if readers.is_empty() => Some(at + Duration::from_millis(50)),
            _ => client.as_ref().and_then(native::NativeClient::expires_at),
        };
        let deadline = viewers
            .iter()
            .filter_map(|viewer| match viewer.connection {
                Connection::Dropped(at) => Some(at + GRACE),
                Connection::Open(_) | Connection::Draining(_) if viewer.live() => Some(
                    Instant::now()
                        + HEARTBEAT_TIMEOUT
                            .saturating_sub(viewer.heartbeat_at.elapsed().unwrap_or_default()),
                ),
                Connection::Open(_) | Connection::Draining(_) => None,
            })
            .chain(deadline)
            .chain(Some(Instant::now() + Duration::from_secs(10)))
            .chain(pending_input.retry_at())
            .chain(title.as_ref().map(|title| title.deadline()))
            .chain(settling.as_ref().and_then(Settling::deadline))
            .min();
        wait_for_activity(
            &listener,
            &wake,
            &viewers,
            &readers,
            input.as_ref().filter(|_| pending_input.needs_write()),
            deadline.map(|at| at.saturating_duration_since(Instant::now())),
        );
    }
}

/// Drain and deliver what a finished native client still held, then its exit line.
fn finish_client(
    mut ended: native::NativeClient,
    readers: &mut [(u8, File)],
    output: &mut output::Output,
    viewers: &mut Vec<Viewer>,
    pending_input: &mut PendingInput,
    detached: bool,
) {
    for (stream, reader) in readers.iter_mut() {
        let mut bytes = [0; 16384];
        for _ in 0..LIMIT / bytes.len() {
            let Ok(n @ 1..) = reader.read(&mut bytes) else {
                break;
            };
            let ready = ended.accept(reader.as_raw_fd(), &bytes[..n]);
            deliver(output, viewers, pending_input, *stream, &ready, detached);
        }
    }
    let exit = ended.finish();
    deliver(output, viewers, pending_input, 1, &exit, detached);
}

fn drain(wake: &UnixStream) {
    let mut bytes = [0; 64];
    while matches!((&*wake).read(&mut bytes), Ok(1..)) {}
}

/// Block until a viewer, the execution's output, the wake pipe or a writable
/// input descriptor has work, or `timeout` passes.
fn wait_for_activity(
    listener: &UnixListener,
    wake: &UnixStream,
    viewers: &[Viewer],
    readers: &[(u8, File)],
    input: Option<&File>,
    timeout: Option<Duration>,
) {
    let pollfd = |fd: RawFd, events: libc::c_short| libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let mut fds = vec![
        pollfd(listener.as_raw_fd(), libc::POLLIN),
        pollfd(wake.as_raw_fd(), libc::POLLIN),
    ];
    for viewer in viewers {
        let (socket, events) = match &viewer.connection {
            Connection::Open(socket) if !viewer.output.is_empty() => {
                (socket, libc::POLLIN | libc::POLLOUT)
            }
            Connection::Open(socket) | Connection::Draining(socket) => (socket, libc::POLLIN),
            Connection::Dropped(_) => continue,
        };
        fds.push(pollfd(socket.as_raw_fd(), events));
    }
    for (_, reader) in readers {
        fds.push(pollfd(reader.as_raw_fd(), libc::POLLIN));
    }
    if let Some(input) = input {
        fds.push(pollfd(input.as_raw_fd(), libc::POLLOUT));
    }
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
/// window immediately; its sixty-second replay grace is not a second hold.
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
fn release_viewer_hold(owner: &LfProcessId, previous: Option<ViewerHold>) {
    let hold = viewer_hold(
        &[],
        previous,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    );
    let recorded = crate::journal::process_ledger()
        .and_then(|store| store.record_owner_viewer_hold(owner, hold));
    if let Err(error) = recorded {
        tracing::warn!(%error, %owner, "cannot record Session viewer release; last observation expires");
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

/// Send reports the stream can carry now, and any directory they changed.
fn publish_reports(output: &mut output::Output, viewers: &mut Vec<Viewer>) {
    if let Some(reports) = output.take_reports() {
        viewers.retain_mut(|viewer| {
            !viewer.attached || {
                let written = viewer.stream(Tag::Out, &reports);
                keep(viewer, written)
            }
        });
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
        !ready || viewer.events.is_empty() || {
            let written = viewer.flush_events().and_then(|()| viewer.flush());
            keep(viewer, written)
        }
    });
}

fn deliver(
    output: &mut output::Output,
    viewers: &mut Vec<Viewer>,
    pending_input: &mut PendingInput,
    stream: u8,
    bytes: &[u8],
    detached: bool,
) {
    if bytes.is_empty() {
        return;
    }
    let replies = output.process(stream, bytes, detached);
    pending_input.push(&replies);
    let reports = output.take_reports();
    viewers.retain_mut(|viewer| {
        !viewer.attached || {
            let written = viewer
                .stream(Tag::stream(stream), bytes)
                .and_then(|()| match &reports {
                    Some(reports) => viewer.stream(Tag::Out, reports),
                    None => Ok(()),
                })
                .and_then(|()| viewer.flush());
            keep(viewer, written)
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
}

impl ViewerState {
    fn reset_display(&mut self) -> io::Result<()> {
        // Kitty keyboard state belongs to each screen; reset both when leaving.
        self.output.stdout.write_all(terminal::RESET_MODES)?;
        if self.alternate.entered {
            self.output.stdout.write_all(b"\x1b[?1049l")?;
            self.alternate.entered = false;
            self.output.stdout.write_all(terminal::RESET_MODES)?;
        }
        // Restore the drawing state too; soft reset support varies by terminal.
        self.output
            .stdout
            .write_all(b"\x1b[?6l\x1b[?7h\x1b[r\x1b(B\x1b)B\x0f\x1b]8;;\x1b\\")
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

    fn handoff(&mut self) -> io::Result<()> {
        let mut state = self.state.lock().expect("viewer terminal lock poisoned");
        state.reset_display()?;
        // The live owner is authoritative even when its first screen is blank.
        state.output.stdout.write_all(b"\x1b[H\x1b[2J")
    }

    fn write(&mut self, stream: u8, bytes: &[u8]) -> io::Result<()> {
        // An interrupt cannot restore the terminal midway through an output write.
        let mut state = self.state.lock().expect("viewer terminal lock poisoned");
        state.alternate.observe(bytes);
        if stream == 2 {
            state.output.stderr.write_all(bytes)
        } else {
            state.output.stdout.write_all(bytes)
        }
    }
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
                "Execution owner stopped reading requests; outcome is unknown and later input was not delivered"
            ),
            _ => error.into(),
        })
}

/// This viewer's terminal device, named to others when it takes ownership.
fn tty_name(fd: RawFd) -> Option<String> {
    // SAFETY: ttyname returns null or a pointer to a static NUL-terminated name.
    unsafe {
        let name = libc::ttyname(fd);
        (!name.is_null()).then(|| CStr::from_ptr(name).to_string_lossy().into_owned())
    }
}

/// Attach this terminal with input control, taking it from any other viewer;
/// that viewer prints `moved` as it leaves.
///
/// The process that resolved the connection built it with a store, a command
/// tree and often a runtime; none of that serves a relay. It carries its
/// Process receipt into the relay, which opens no store until it returns.
pub(crate) fn attach(
    id: &LfProcessId,
    session: Option<&LfSessionId>,
    moved: Option<String>,
    json_session: Option<&str>,
) -> Result<()> {
    relay_with_preview(id, session, moved, json_session, None, true)
}

pub(crate) fn attach_existing(id: &LfProcessId, session: &str, json: bool) -> Result<()> {
    std::env::set_var("LF_RELAY_SESSION", session);
    relay_with_preview(
        id,
        Some(&LfSessionId::from(session)),
        None,
        json.then_some(session),
        None,
        false,
    )
}

fn relay_with_preview(
    id: &LfProcessId,
    session: Option<&LfSessionId>,
    moved: Option<String>,
    json_session: Option<&str>,
    terminal: Option<ViewerTerminal>,
    replace: bool,
) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    if !replace {
        command.env("LF_RELAY_IF_FREE", "1");
    }
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
    command.args([RELAY_COMMAND[0], RELAY_COMMAND[1], id.as_str()]);
    if let Some(session) = session {
        command.args([RELAY_SESSION_FLAG, session.as_str()]);
    }
    if let Some(moved) = &moved {
        command.args([RELAY_MOVED_FLAG, moved]);
    }
    if let Some(session) = json_session {
        command.args([RELAY_JSON_FLAG, session]);
    }
    crate::journal::carry_across_exec(&mut command)?;
    let error = command.exec();
    tracing::warn!(%error, "cannot exec the terminal relay; relaying here");
    attach_with_terminal(id, session, moved, terminal, replace)
}

const RELAY_PREVIEW_ENV: &str = "LF_RELAY_PREVIEW";
const RELAY_COMMAND: [&str; 2] = ["session", "attach"];
const RELAY_SESSION_FLAG: &str = "--session";
const RELAY_MOVED_FLAG: &str = "--moved";
const RELAY_JSON_FLAG: &str = "--session-json";

/// What a relay invocation names: `lf session attach OWNER [--session SESSION]
/// [--moved TEXT] [--session-json SESSION]`, the exact owner, the Session a
/// server bridge attaches by, and the notice it prints. Recognized before
/// command parsing.
#[derive(Debug)]
pub struct Relay {
    pub owner: LfProcessId,
    pub session: Option<LfSessionId>,
    pub moved: Option<String>,
    pub json_session: Option<String>,
}

pub fn relay_args(args: &[String]) -> Option<Relay> {
    let [_, command, subcommand, owner, rest @ ..] = args else {
        return None;
    };
    if command != RELAY_COMMAND[0] || subcommand != RELAY_COMMAND[1] {
        return None;
    }
    let owner = LfProcessId::parse(owner).ok()?;
    let mut session = None;
    let mut moved = None;
    let mut json_session = None;
    for pair in rest.chunks(2) {
        match pair {
            [flag, value] if flag == RELAY_SESSION_FLAG && session.is_none() => {
                session = Some(LfSessionId::from(value.as_str()));
            }
            [flag, value] if flag == RELAY_MOVED_FLAG && moved.is_none() => {
                moved = Some(value.clone());
            }
            [flag, value] if flag == RELAY_JSON_FLAG && json_session.is_none() => {
                json_session = Some(value.clone());
            }
            _ => return None,
        }
    }
    Some(Relay {
        owner,
        session,
        moved,
        json_session,
    })
}

/// Paint an ended Desktop shell's retained screen without starting execution.
pub(crate) fn shell_history(owner: &LfProcessId) -> Result<()> {
    if let Some(screen) = snapshot::last_screen(owner.as_str()) {
        if std::env::var_os(STREAM_ENV).is_some() {
            io::stdout().write_all(&wire::frame(Tag::Snapshot, &screen))?;
        } else {
            io::stdout().write_all(&screen)?;
        }
    }
    Ok(())
}

/// Relay this terminal to the execution owner: socket in, terminal out.
pub fn relay(id: &LfProcessId, session: Option<&LfSessionId>, moved: Option<String>) -> Result<()> {
    if std::env::var_os(STREAM_ENV).is_some() {
        return stream::bridge(id);
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
    let replace = std::env::var_os("LF_RELAY_IF_FREE").is_none();
    std::env::remove_var("LF_RELAY_IF_FREE");
    attach_with_terminal(id, session, moved, terminal, replace)
}

/// A reconnect token, byte position and partial event scanner describe one
/// retained output stream. A fresh connection replaces all three together.
#[derive(Default)]
struct RelayOutput {
    reconnect: Option<String>,
    received: u64,
    scanner: Option<Scanner>,
    replayed_claims_through: Option<u64>,
}

impl RelayOutput {
    fn recover(&mut self, terminal: bool) -> Request {
        // A fresh terminal screen restores parser state even if the bridge
        // interrupted an escape sequence; byte streams continue at their offset.
        if !terminal {
            if let Some(reconnect) = &self.reconnect {
                let (rows, cols) = size();
                return Request::Resume(Resume {
                    reconnect: reconnect.clone(),
                    received: self.received,
                    control: true,
                    own: Some(Own::Request),
                    rows,
                    cols,
                });
            }
        }
        self.fresh_attachment(true)
    }

    fn fresh_attachment(&mut self, control: bool) -> Request {
        *self = Self::default();
        let (rows, cols) = size();
        Request::Attach {
            control,
            replace: false,
            own: control.then_some(Own::Request),
            rows,
            cols,
            name: tty_name(libc::STDIN_FILENO),
            moved: None,
        }
    }

    fn attached(&mut self, reconnect: String, nonce: &str, sequence: u64) {
        self.replayed_claims_through = self.reconnect.as_ref().map(|_| sequence);
        if self.reconnect.is_none() {
            self.scanner = Some(Scanner::new(nonce));
        }
        self.reconnect = Some(reconnect);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Recovery {
    Reconnect,
    Takeover,
}

fn attach_with_terminal(
    id: &LfProcessId,
    session: Option<&LfSessionId>,
    moved: Option<String>,
    terminal: Option<ViewerTerminal>,
    replace: bool,
) -> Result<()> {
    let server = std::env::var_os("LF_SERVER").is_some();
    let mut status = connection::Status::from_env();
    let bridged = || bridged_session(session);
    let mut recovery = None;
    let socket = if let Some(status) = &mut status {
        let Some((socket, recovered)) = status.connect(id, bridged()?, true)? else {
            return Ok(());
        };
        recovery = recovered.then_some(Recovery::Reconnect);
        socket
    } else if server {
        crate::server::client::bridge(bridged()?, id)?
    } else {
        UnixStream::connect(endpoint(id)?).context(
            "Execution owner is unavailable; no provider was restarted and no input was replayed",
        )?
    };
    view(
        stream::Stream::socket(socket)?,
        Some(id),
        Some(ServerRoute {
            session,
            status,
            recovery,
            server,
        }),
        moved,
        terminal,
        ATTACH_TIMEOUT,
        replace,
    )
}

/// View the existing protocol over a command's stdin/stdout, such as SSH.
/// The destination forwards bytes to its owner and creates no terminal here.
pub fn relay_over(argv: &[String], replace: bool) -> Result<()> {
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
            None,
            COMMAND_ATTACH_TIMEOUT,
            replace,
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

/// A viewer of one owner through the server, which reconnects when the
/// bridge drops. SSH and direct sockets have none: their loss ends the viewer.
struct ServerRoute<'a> {
    session: Option<&'a LfSessionId>,
    status: Option<connection::Status>,
    recovery: Option<Recovery>,
    server: bool,
}

/// The server attaches by Session; only Desktop's Session panes bridge through
/// it, and they always name theirs.
fn bridged_session(session: Option<&LfSessionId>) -> Result<&LfSessionId> {
    session.context("A server-bridged terminal relay names its Session (`--session`)")
}

fn view(
    mut socket: stream::Stream,
    id: Option<&LfProcessId>,
    route: Option<ServerRoute<'_>>,
    moved: Option<String>,
    mut terminal: Option<ViewerTerminal>,
    attach_timeout: Duration,
    replace: bool,
) -> Result<()> {
    let (session, mut status, mut recovery, server) = match route {
        Some(route) => (route.session, route.status, route.recovery, route.server),
        None => (None, None, None, false),
    };
    let mut acknowledgement_deadline = Some(Instant::now() + attach_timeout);
    let (rows, cols) = size();
    let mut output = RelayOutput::default();
    let attachment = if recovery.is_some() {
        output.recover(terminal.is_some())
    } else {
        Request::Attach {
            control: true,
            replace,
            own: Some(if replace { Own::Take } else { Own::Request }),
            rows,
            cols,
            name: tty_name(libc::STDIN_FILENO),
            moved,
        }
    };
    let initial = send(&mut socket, attachment);
    let mut interrupted = match initial {
        Ok(()) => None,
        Err(error) if server => Some(error.to_string()),
        Err(error) => return Err(error),
    };
    let mut decoder = Decoder::default();
    let mut controlled = false;
    let mut owned = false;
    let mut returning = None;
    let mut stdin_open = true;
    let mut previous_size = (rows, cols);
    let mut title = id.map(crate::terminal_title::ViewerTitle::new);
    let mut heartbeat = SystemTime::now() + HEARTBEAT_INTERVAL;
    'relay: loop {
        if status.as_ref().is_some_and(connection::Status::closed) {
            return Ok(());
        }
        if let Some(reason) = interrupted.take() {
            // Close the old bridge before asking the owner for its free claims.
            socket.shutdown();
            let owner = id.context("only a known owner reconnects")?;
            let session = bridged_session(session)?;
            let next = if let Some(status) = &mut status {
                status
                    .connect(owner, session, false)?
                    .map(|(socket, _)| socket)
            } else {
                reconnect_server(owner, session, &mut terminal, &reason)?
            };
            let Some(next) = next else {
                return Ok(());
            };
            socket = stream::Stream::socket(next)?;
            decoder = Decoder::default();
            controlled = false;
            owned = false;
            recovery = Some(Recovery::Reconnect);
            if let Err(error) = send(&mut socket, output.recover(terminal.is_some())) {
                interrupted = Some(error.to_string());
                continue;
            }
            acknowledgement_deadline = Some(Instant::now() + attach_timeout);
        }
        if let Some(title) = &mut title {
            title.tick();
        }
        if SystemTime::now() >= heartbeat {
            heartbeat = SystemTime::now() + HEARTBEAT_INTERVAL;
            if let Err(error) = send(&mut socket, Request::Controlled) {
                if !server {
                    return Err(error);
                }
                interrupted = Some(error.to_string());
                continue;
            }
        }
        if acknowledgement_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            if server {
                interrupted =
                    Some("Attachment was not acknowledged; its outcome is unknown".to_owned());
                continue;
            }
            bail!("Execution owner did not acknowledge attachment; outcome is unknown, no provider was restarted and no input was replayed");
        }
        let mut fds = [
            libc::pollfd {
                fd: socket.input.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if stdin_open && (controlled || terminal.is_some()) {
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
                result if server => {
                    interrupted = Some(match result {
                        Err(error) => error.to_string(),
                        _ => "The bridge closed".to_owned(),
                    });
                    continue;
                }
                Ok(_) => bail!("Execution owner disconnected; outcome is unknown"),
                Err(error) => return Err(error.into()),
            };
            decoder.extend(&bytes[..n]);
            while let Some((tag, payload)) = decoder.next()? {
                if tag != Tag::Reply {
                    output.received += payload.len() as u64;
                }
                let pieces = match tag {
                    Tag::Reply => {
                        let reply: Outgoing = serde_json::from_slice(&payload)?;
                        match reply.reply {
                            Reply::Attached {
                                viewer,
                                reconnect: token,
                                nonce,
                                token: claim,
                                terminal: is_terminal,
                                owner,
                                ..
                            } => {
                                acknowledgement_deadline = None;
                                controlled = claim.is_some();
                                if controlled {
                                    if let Some(id) = id {
                                        if let Err(error) = record_relay_terminal(id) {
                                            crate::store::RecordingGap::default()
                                                .note("record viewer terminal", &error);
                                        }
                                    }
                                }
                                owned = owner.as_ref() == Some(&viewer);
                                let recovered = recovery.take();
                                if recovered == Some(Recovery::Takeover) {
                                    // Reset the parser at the fresh snapshot, keeping
                                    // this connection's cumulative resume offset.
                                    output.scanner = Some(Scanner::new(&nonce));
                                }
                                output.attached(token, &nonce, reply.seq);
                                if recovered.is_some() || status.is_some() {
                                    discard_reconnect_input()?;
                                    fds[1].revents = 0;
                                }
                                if recovered.is_some() && status.is_none() {
                                    write_output(&mut terminal, 2, if controlled {
                                        b"\r\nReconnected to the same Session.\r\n"
                                    } else {
                                        b"\r\nReconnected as a viewer; input remains elsewhere. Use Move here to take control.\r\n"
                                    })?;
                                }
                                if let Some(terminal) = &mut terminal {
                                    terminal.handoff()?;
                                }
                                if is_terminal {
                                    if terminal.is_none() {
                                        terminal = Some(ViewerTerminal::enter()?);
                                    }
                                    terminal.as_ref().expect("terminal entered").make_raw()?;
                                }
                                if let Some(status) = &mut status {
                                    status.connected(controlled)?;
                                }
                            }
                            Reply::Busy if recovery.is_some() => {
                                // A fresh screen cannot atomically ask for optional
                                // input. On Busy, attach passively to the same owner.
                                if let Err(error) = send(&mut socket, output.fresh_attachment(false)) {
                                    interrupted = Some(error.to_string());
                                    continue 'relay;
                                }
                            }
                            Reply::Busy => anyhow::bail!("Session is attached in another viewer; use an explicit attachment to move input control here"),
                            Reply::ReconnectRequired if recovery.is_some() => {
                                if status.is_none() {
                                    write_output(&mut terminal, 2, b"\r\nRetained output expired; reading the same owner's current screen.\r\n")?;
                                }
                                if let Err(error) = send(&mut socket, output.fresh_attachment(true)) {
                                    interrupted = Some(error.to_string());
                                    continue 'relay;
                                }
                            }
                            Reply::ReconnectRequired => {
                                anyhow::bail!("Execution owner no longer holds this viewer's stream")
                            }
                            Reply::Controlled { .. } | Reply::NativeEnded | Reply::Accepted => {}
                        }
                        continue;
                    }
                    Tag::Snapshot if output.scanner.is_none() => {
                        // A destination may show retained history before an owner
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
                    Tag::Snapshot | Tag::Out => output
                        .scanner
                        .as_mut()
                        .context("Execution owner streamed before acknowledging attachment")?
                        .scan(&payload),
                    Tag::Err => vec![wire::Piece::Bytes(payload)],
                    Tag::Request | Tag::Input => anyhow::bail!("unexpected transport frame"),
                };
                let stream = if tag == Tag::Err { 2 } else { 1 };
                for piece in pieces {
                    let event = match piece {
                        wire::Piece::Bytes(bytes) => {
                            write_output(&mut terminal, stream, &bytes)?;
                            continue;
                        }
                        // The acknowledgement describes current claims. Replayed
                        // old notices cannot revoke them or resume a provider.
                        wire::Piece::Event(seq, Event::Control { .. } | Event::Owner { .. })
                            if output
                                .replayed_claims_through
                                .is_some_and(|last| seq <= last) =>
                        {
                            continue
                        }
                        wire::Piece::Event(_, event) => event,
                    };
                    match event {
                        Event::Resize { .. }
                        | Event::Theme { .. }
                        | Event::ProgramStatus { .. } => {}
                        Event::Directory { cwd } => {
                            if let Err(error) = std::env::set_current_dir(&cwd) {
                                tracing::debug!(%error, "viewer cannot enter execution directory");
                            }
                            if let Some(terminal) = &terminal {
                                terminal.retire_directory_on_restore();
                            }
                        }
                        Event::Owner { name, viewer, held } => {
                            if status.is_none() && owned && !held {
                                let who =
                                    name.or(viewer).unwrap_or_else(|| "another viewer".into());
                                write_output(
                                    &mut terminal,
                                    2,
                                    format!("\r\nSize and colors now follow {who}.\r\n").as_bytes(),
                                )?;
                            }
                            owned = held;
                        }
                        Event::Control { held: true, .. } => {}
                        Event::Control {
                            held: false,
                            notice,
                        } => {
                            drop(terminal.take());
                            if let Some(notice) = notice {
                                eprintln!("\n{notice}");
                            }
                            let _ = send(&mut socket, Request::Leave);
                            return Ok(());
                        }
                        Event::ChildExited { code } => {
                            if let Some(status) = &status {
                                status.ended()?;
                            }
                            return if code == 0 {
                                Ok(())
                            } else {
                                Err(crate::process::CommandExit(code).into())
                            };
                        }
                        Event::Detached => {
                            // Detached on purpose: an SSH viewer must not read as lost.
                            let _ = send(&mut socket, Request::Leave);
                            return Ok(());
                        }
                        Event::Return { detach } => {
                            returning = Some(detach);
                            // This marker comes from the acknowledged live relay, never
                            // the owner's saved screen. Desktop consumes it as readiness.
                            if controlled && crate::session_record::current_terminal_id().is_some()
                            {
                                write_output(&mut terminal, 1, b"\x1b]2;loopflow:input-ready\x07")?;
                            }
                        }
                        Event::Hint { text } => {
                            if let Some(terminal) = &mut terminal {
                                terminal.write(2, format!("\r\n{text}\r\n").as_bytes())?;
                            } else {
                                eprintln!("{text}");
                            }
                        }
                    }
                }
            }
        }
        if let Some(status) = &mut status {
            if status.closed() {
                return Ok(());
            }
            if status.take_move(acknowledgement_deadline.is_none() && !controlled)? {
                controlled = false;
                owned = false;
                recovery = Some(Recovery::Takeover);
                discard_reconnect_input()?;
                fds[1].revents = 0;
                let (rows, cols) = size();
                let request = Request::Attach {
                    control: true,
                    replace: true,
                    own: Some(Own::Take),
                    rows,
                    cols,
                    name: tty_name(libc::STDIN_FILENO),
                    moved: None,
                };
                if let Err(error) = send(&mut socket, request) {
                    interrupted = Some(error.to_string());
                    continue;
                }
                acknowledgement_deadline = Some(Instant::now() + attach_timeout);
            }
        }
        if fds[1].revents != 0 {
            let mut bytes = [0; 8192];
            let n = io::stdin().read(&mut bytes)?;
            if n == 0 {
                stdin_open = false;
                if terminal.is_none() && controlled {
                    if let Err(error) = send(&mut socket, Request::EndInput) {
                        if !server {
                            return Err(error);
                        }
                        interrupted = Some(error.to_string());
                        continue;
                    }
                }
            } else if terminal.is_some() && bytes[..n].contains(&0x1d) {
                // Restore the terminal before saying how to return.
                drop(terminal.take());
                let _ = send(&mut socket, Request::Leave);
                if let Some(detach) = &returning {
                    // An inline client can leave the cursor mid-line.
                    eprintln!("\n{detach}");
                }
                return Ok(());
            } else if controlled {
                if let Err(error) = send_frame(&mut socket, Tag::Input, &bytes[..n]) {
                    if !server {
                        return Err(error);
                    }
                    // The write may have reached the owner. Never retain or resend it.
                    interrupted = Some(error.to_string());
                    continue;
                }
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
                if !server {
                    return Err(error);
                }
                interrupted = Some(error.to_string());
            }
        }
    }
}

/// Deliberate connection recovery, retaining the exact owner in this relay.
/// No Session preparation, provider command, or uncertain input is repeated.
fn reconnect_server(
    id: &LfProcessId,
    session: &LfSessionId,
    terminal: &mut Option<ViewerTerminal>,
    reason: &str,
) -> Result<Option<UnixStream>> {
    if terminal.is_none() && io::stdout().is_terminal() {
        *terminal = Some(ViewerTerminal::enter()?);
    }
    if let Some(terminal) = terminal {
        terminal.handoff()?;
        terminal.make_raw()?;
    }
    discard_reconnect_input()?;
    write_output(terminal, 2, format!("\r\nServer connection lost: {reason}.\r\nCtrl+R to reconnect; Ctrl+] to close. Typing while disconnected is discarded.\r\n").as_bytes())?;
    loop {
        let mut bytes = [0; 8192];
        let n = io::stdin().read(&mut bytes)?;
        if n == 0 || bytes[..n].contains(&0x1d) {
            return Ok(None);
        }
        if !bytes[..n].contains(&0x12) {
            continue;
        }
        match crate::server::client::bridge_existing(session, id) {
            Ok(socket) => return Ok(Some(socket)),
            Err(error) if error.is::<crate::engine::Error>() => {
                return Err(error);
            }
            Err(error) => write_output(
                terminal,
                2,
                format!("\r\nCould not reconnect: {error}. Ctrl+R to retry; Ctrl+] to close.\r\n")
                    .as_bytes(),
            )?,
        }
    }
}

fn discard_reconnect_input() -> io::Result<()> {
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
        if io::stdin().read(&mut bytes)? == 0 {
            return Ok(());
        }
    }
}

/// A passive reveal records its terminal only after acquiring input control.
fn record_relay_terminal(owner: &LfProcessId) -> Result<()> {
    let Some(session) = std::env::var_os("LF_RELAY_SESSION") else {
        return Ok(());
    };
    std::env::remove_var("LF_RELAY_SESSION");
    let store = crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let session = session.to_string_lossy();
    if store.session_transport(&session)?.as_ref() != Some(owner) {
        return Ok(());
    }
    if let Some(attachment) = store.session_attachment(&session)? {
        let terminal = crate::session_record::current_terminal_id()
            .or_else(crate::lf::commands::open::host_placement);
        store.record_session_terminal(&session, &attachment, terminal.as_deref())?;
    }
    Ok(())
}

fn write_output(terminal: &mut Option<ViewerTerminal>, stream: u8, bytes: &[u8]) -> io::Result<()> {
    if let Some(terminal) = terminal {
        terminal.write(stream, bytes)
    } else if stream == 2 {
        io::stderr().write_all(bytes)?;
        io::stderr().flush()
    } else {
        io::stdout().write_all(bytes)?;
        io::stdout().flush()
    }
}

/// Hand the owner what to record when this process, about to exec into its
/// provider, exits. A nested launcher keeps its own wait and returns false.
pub(crate) fn hand_exit_record(record: &NativeExit) -> Result<bool> {
    let handoff = HANDOFF
        .get()
        .context("this launch has no execution owner to hand its wait to")?;
    let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    // A delayed reply does not undo the owner's acceptance. Wait for its
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
        other => bail!("owner refused the exit record: {other:?}"),
    }
}

/// The exec failed; the launcher records its own failure.
pub(crate) fn drop_exit_record() {
    let dropped = (|| -> Result<()> {
        let handoff = HANDOFF.get().context("no execution owner")?;
        let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
        socket.set_write_timeout(Some(Duration::from_secs(1)))?;
        send(
            &mut socket,
            Request::DropExitRecord {
                key: handoff.key.clone(),
            },
        )?;
        // Do not let launcher exit overtake cancellation in the owner's loop.
        match receive_reply(&mut socket)? {
            Reply::Accepted => Ok(()),
            other => bail!("owner refused exit record cancellation: {other:?}"),
        }
    })();
    if let Err(error) = dropped {
        tracing::warn!(%error, "could not cancel the exit record for a failed exec");
    }
}

/// Whether a running owner has returned its viewers and closed its socket, and
/// stays only to finish the exit records its terminal stopped waiting for.
pub(crate) fn leaving(id: &LfProcessId) -> bool {
    endpoint(id).is_ok_and(|path| !path.exists())
}

/// Whether a viewer of this execution holds input control. A closed terminal
/// leaves its execution running with none, ready for the next viewer.
pub(crate) fn controlled(id: &LfProcessId) -> Result<bool> {
    let mut socket = UnixStream::connect(endpoint(id)?)?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    send(&mut socket, Request::Controlled)?;
    match receive_reply(&mut socket)? {
        Reply::Controlled { held } => Ok(held),
        other => bail!("unexpected transport reply {other:?}"),
    }
}

/// Read the reply on a control connection that carries no output stream.
fn receive_reply(socket: &mut UnixStream) -> Result<Reply> {
    let mut decoder = Decoder::default();
    let mut bytes = [0; 4096];
    loop {
        let n = socket.read(&mut bytes)?;
        anyhow::ensure!(n != 0, "Execution owner closed before answering");
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
    let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
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

/// The native client has connected in this directory. The owner serializes
/// its OSC report with provider bytes and retains it for later viewers.
pub(crate) fn report_directory(cwd: &Path) {
    let Some(handoff) = HANDOFF.get() else { return };
    let result = (|| -> Result<()> {
        let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
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

/// A native client running under this execution's owner. Its own exit text is
/// replaced by `exit` when it ends; dropping the guard reports that end.
#[derive(Debug)]
pub(crate) struct NativeClient {
    socket: UnixStream,
    key: String,
}

pub(crate) fn native_client(exit: String, detach: String) -> Option<NativeClient> {
    let handoff = HANDOFF.get()?;
    let started = (|| -> Result<UnixStream> {
        let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
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
            // Hold further output until the owner has replaced the exit text.
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

/// Live input a controlling viewer can type. The owner shows `hint` to each
/// controlling viewer as it attaches until this guard drops, so headless output
/// never contains it. `None` when no viewer can attach.
#[derive(Debug)]
pub(crate) struct InputHint {
    _socket: UnixStream,
}

pub(crate) fn input_hint(hint: String) -> Option<InputHint> {
    let handoff = HANDOFF.get()?;
    let registered = (|| -> Result<UnixStream> {
        let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
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

/// A Graph and its steps share one terminal owner. This is an explicit execution
/// handoff, not authority inferred from process ancestry.
pub(crate) fn configure_child(command: &mut Command) {
    if let Some(handoff) = HANDOFF.get() {
        handoff.pass(command, std::process::id());
    }
}

/// An exec keeps this PID and its parent, so the replacing lf binary is still
/// the direct child the handoff names.
pub(crate) fn carry_across_exec(command: &mut Command) {
    if let Some(handoff) = HANDOFF.get() {
        handoff.pass(command, handoff.parent);
    }
}

/// Status travels beside output to the sole writer, never through capture pipes.
pub(crate) fn report(state: crate::program_status::State, step: bool, progress: Option<u8>) {
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
        let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
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
    report(crate::program_status::State::Clear, true, None);
    report(
        if code == 0 {
            crate::program_status::State::Done
        } else if code == 130 {
            crate::program_status::State::Idle
        } else {
            crate::program_status::State::Error
        },
        false,
        None,
    );
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::net::UnixStream;
    use std::process::Command;

    use super::{
        carry_across_exec, output, place_events, wire, Event, Handoff, Own, Ownership, Viewer,
        CONTROL_ENV, LIMIT, OWNER_ENV, PARENT_ENV,
    };
    use crate::id::LfProcessId;

    #[test]
    fn taking_ownership_fulfills_a_viewers_queued_request() {
        let (first, _first_peer) = UnixStream::pair().unwrap();
        let (second, _second_peer) = UnixStream::pair().unwrap();
        let mut viewers = vec![Viewer::new(first), Viewer::new(second)];
        for viewer in &mut viewers {
            viewer.attached = true;
        }
        let mut ownership = Ownership::default();
        ownership.claim(Some(Own::Take), &viewers[0]);
        ownership.claim(Some(Own::Request), &viewers[1]);
        ownership.claim(Some(Own::Take), &viewers[1]);
        ownership.claim(Some(Own::Take), &viewers[0]);
        viewers[0].drop_connection();
        ownership.settle(&viewers);
        assert_eq!(
            ownership.id(),
            None,
            "a fulfilled request cannot reclaim ownership"
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
        let owner = LfProcessId::new();
        // SAFETY: getppid has no preconditions.
        let parent = unsafe { libc::getppid() } as u32;
        std::env::set_var(OWNER_ENV, owner.as_str());
        std::env::set_var(PARENT_ENV, parent.to_string());
        std::env::set_var(CONTROL_ENV, "key");
        let handoff = Handoff::take().expect("direct child adopts the handoff");
        assert!(std::env::var_os(CONTROL_ENV).is_none());

        let mut exec = Command::new("lf");
        handoff.pass(&mut exec, handoff.parent);
        assert_eq!(env(&exec, OWNER_ENV), Some(OsStr::new(owner.as_str())));
        assert_eq!(
            env(&exec, PARENT_ENV),
            Some(OsStr::new(&parent.to_string()))
        );
        assert_eq!(env(&exec, CONTROL_ENV), Some(OsStr::new("key")));
    }

    #[test]
    fn a_non_child_neither_adopts_nor_carries_a_handoff() {
        let _env = crate::journal::test_env_lock();
        std::env::set_var(OWNER_ENV, LfProcessId::new().as_str());
        std::env::set_var(PARENT_ENV, "0");
        std::env::set_var(CONTROL_ENV, "key");
        assert!(Handoff::take().is_none());
        assert!(std::env::var_os(CONTROL_ENV).is_none());

        let mut exec = Command::new("lf");
        carry_across_exec(&mut exec);
        assert_eq!(exec.get_envs().count(), 0);
    }
}
