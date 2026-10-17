//! A detached terminal owns an execution, including its capture and native client.
//! Viewers hold revocable input tokens, never provider attachment capabilities.

mod output;
mod terminal;

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::id::LfProcessId;

const OWNER_ENV: &str = "LF_TRANSPORT_OWNER";
const PARENT_ENV: &str = "LF_TRANSPORT_PARENT";
const LIMIT: usize = 4 * 1024 * 1024;
const ATTACH_TIMEOUT: Duration = Duration::from_secs(5);
static HANDOFF: OnceLock<Handoff> = OnceLock::new();

#[derive(Debug)]
struct Handoff {
    owner: LfProcessId,
    key: String,
}

#[derive(Debug, clap::Subcommand)]
pub enum TransportCommand {
    /// Start an independently owned command and print its Process identity
    Start {
        #[arg(long)]
        terminal: bool,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    /// Own a command's transport until it exits (normally launched by start)
    Serve {
        #[arg(long)]
        terminal: bool,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        argv: Vec<String>,
    },
    /// Attach to an existing execution; Ctrl+] detaches without stopping it
    Attach {
        process: String,
        #[arg(long)]
        observe: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Request {
    Attach {
        control: bool,
        rows: u16,
        cols: u16,
    },
    Input {
        token: String,
        bytes: Vec<u8>,
    },
    Resize {
        token: String,
        rows: u16,
        cols: u16,
    },
    DetachViewers {
        key: String,
    },
    Status {
        key: String,
        id: String,
        state: crate::program_status::State,
        progress: Option<u8>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Response {
    Attached {
        token: Option<String>,
        terminal: bool,
        bytes: Vec<u8>,
    },
    Output {
        stream: u8,
        bytes: Vec<u8>,
    },
    Exit {
        code: u8,
    },
    Detached,
}

/// Consume the direct execution handoff. Provider tools and nested lf calls do
/// not inherit the right to run inside this execution's owner.
pub fn initialize() {
    let parent = std::env::var(PARENT_ENV)
        .ok()
        .and_then(|p| p.parse::<u32>().ok());
    // SAFETY: getppid has no preconditions.
    let direct = parent == Some(unsafe { libc::getppid() } as u32);
    if direct {
        if let (Some(owner), Ok(key)) = (
            std::env::var(OWNER_ENV)
                .ok()
                .and_then(|id| LfProcessId::parse(&id).ok()),
            std::env::var("LF_TRANSPORT_CONTROL"),
        ) {
            let _ = HANDOFF.set(Handoff { owner, key });
        }
    }
    std::env::remove_var("LF_TRANSPORT_CONTROL");
    std::env::remove_var(PARENT_ENV);
    std::env::remove_var(OWNER_ENV);
}

pub(crate) fn current() -> Option<&'static LfProcessId> {
    HANDOFF.get().map(|handoff| &handoff.owner)
}

pub fn run(command: &TransportCommand) -> Result<()> {
    match command {
        TransportCommand::Start { terminal, argv } => {
            println!(
                "{}",
                start(argv, *terminal, &std::env::current_dir()?, &|_| {})?
            );
            Ok(())
        }
        TransportCommand::Serve { terminal, argv } => serve(argv, *terminal),
        TransportCommand::Attach { process, observe } => {
            attach(&LfProcessId::parse(process)?, *observe)
        }
    }
}

/// Delegate before capture admission or provider effects. A missing startup
/// acknowledgement is uncertainty, never permission to launch again.
pub fn delegate_current() -> Result<bool> {
    if current().is_some() {
        return Ok(false);
    }
    // Library tests do not execute the CLI entry point.
    #[cfg(test)]
    {
        Ok(false)
    }
    #[cfg(not(test))]
    {
        let mut argv: Vec<String> = std::env::args().collect();
        argv[0] = std::env::current_exe()?.to_string_lossy().into_owned();
        let owner = start(
            &argv,
            io::stdin().is_terminal() && io::stdout().is_terminal(),
            &std::env::current_dir()?,
            &|_| {},
        )?;
        attach(&owner, false)?;
        Ok(true)
    }
}

pub(crate) fn start(
    argv: &[String],
    terminal: bool,
    cwd: &Path,
    inherit: &dyn Fn(&mut Command),
) -> Result<LfProcessId> {
    let logs = crate::store::lf_home_dir().join("logs");
    std::fs::create_dir_all(&logs)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("transport.log"))?;
    let mut command = Command::new(std::env::current_exe()?);
    command.args(["transport", "serve"]);
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
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(output).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
        let _ = child.wait();
    });
    let line = rx
        .recv_timeout(Duration::from_secs(20))
        .context("Execution handoff was not acknowledged; delivery is unknown, do not replay")??;
    LfProcessId::parse(line.trim())
        .context("Owner exited before acknowledging execution; inspect transport.log")
}

fn endpoint(id: &LfProcessId) -> Result<PathBuf> {
    // Keep sockaddr_un short even with a long LF_HOME or macOS temporary path.
    let home = crate::store::lf_home_dir().canonicalize()?;
    let digest = hex::encode(Sha256::digest(home.as_os_str().as_encoded_bytes()));
    // SAFETY: geteuid has no preconditions.
    let dir = PathBuf::from(format!(
        "/tmp/lf-{}-{}",
        unsafe { libc::geteuid() },
        &digest[..16]
    ));
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
    Ok(dir.join(format!("{id}.sock")))
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

fn terminal_pair() -> io::Result<(File, File)> {
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
            &mut size,
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
    // An unopened terminal may report zero. vt100 needs room for a wide glyph
    // and a previous row when wrapping, even in a temporarily collapsed pane.
    (
        if rows == 0 { 24 } else { rows.clamp(2, 500) },
        if cols == 0 { 80 } else { cols.clamp(2, 500) },
    )
}

fn resize(fd: RawFd, rows: u16, cols: u16) -> io::Result<()> {
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

#[derive(Debug)]
struct Viewer {
    socket: UnixStream,
    input: Vec<u8>,
    output: VecDeque<u8>,
    token: Option<String>,
    attached: bool,
}

impl Viewer {
    fn queue(&mut self, response: &Response) -> Result<()> {
        let mut bytes = serde_json::to_vec(response)?;
        bytes.push(b'\n');
        anyhow::ensure!(
            self.output.len() + bytes.len() <= LIMIT,
            "Viewer fell behind"
        );
        self.output.extend(bytes);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
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

    fn requests(&mut self) -> Result<Vec<Request>> {
        let mut bytes = [0; 8192];
        match self.socket.read(&mut bytes) {
            Ok(0) => bail!("Viewer disconnected"),
            Ok(n) => self.input.extend_from_slice(&bytes[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.into()),
        }
        anyhow::ensure!(self.input.len() <= LIMIT, "Viewer request too large");
        let mut requests = Vec::new();
        while let Some(end) = self.input.iter().position(|b| *b == b'\n') {
            requests.push(serde_json::from_slice(&self.input[..end])?);
            self.input.drain(..=end);
        }
        Ok(requests)
    }
}

fn serve(argv: &[String], terminal: bool) -> Result<()> {
    let id =
        crate::journal::current_lf_process_id().context("Transport needs an admitted Process")?;
    let path = endpoint(&id)?;
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    let control_key = uuid::Uuid::new_v4().to_string();
    let mut command = Command::new(&argv[0]);
    command.env("LF_TRANSPORT_CONTROL", &control_key);
    command
        .args(&argv[1..])
        .env(OWNER_ENV, id.as_str())
        .env(PARENT_ENV, std::process::id().to_string());
    let pair = terminal.then(terminal_pair).transpose()?;
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
    let mut child = command.spawn()?;
    let group = crate::os_process::ProcessGroupGuard::new(child.id());
    let mut readers: Vec<(u8, File)> = Vec::new();
    let mut input: File;
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
    // The child and its capture now belong to this owner, even if acknowledgement is lost.
    let _ = writeln!(io::stdout(), "{id}");
    let _ = io::stdout().flush();
    let mut viewers: Vec<Viewer> = Vec::new();
    let mut control: Option<String> = None;
    let mut pending_input = VecDeque::new();
    let mut output = output::Output::new(terminal);
    let mut exit: Option<(u8, Instant)> = None;
    let mut detach = false;
    let mut announced = false;
    loop {
        while let Ok((socket, _)) = listener.accept() {
            socket.set_nonblocking(true)?;
            viewers.push(Viewer {
                socket,
                input: Vec::new(),
                output: VecDeque::new(),
                token: None,
                attached: false,
            });
        }
        viewers.retain_mut(|viewer| {
            let result = (|| -> Result<()> {
                for request in viewer.requests()? {
                    match request {
                        Request::Status {
                            key,
                            id,
                            state,
                            progress,
                        } if key == control_key => {
                            output.report(id, state, progress);
                        }
                        Request::DetachViewers { key } if key == control_key => {
                            detach = true;
                        }
                        Request::Attach {
                            control: wants_control,
                            rows,
                            cols,
                        } => {
                            let (rows, cols) = dimensions(rows, cols);
                            if wants_control {
                                let token = uuid::Uuid::new_v4().to_string();
                                control = Some(token.clone());
                                viewer.token = Some(token);
                                if terminal {
                                    resize(input.as_raw_fd(), rows, cols)?;
                                    output.resize(rows, cols);
                                }
                            } else {
                                if viewer.token.is_some() && viewer.token == control {
                                    control = None;
                                }
                                viewer.token = None;
                            }
                            viewer.attached = true;
                            output.attach(viewer)?;
                            if terminal && wants_control {
                                // Resizing to the same dimensions sends no signal.
                                // Ask the actual foreground program to repaint too.
                                // SAFETY: the descriptor is our PTY; only its current
                                // foreground group receives the nonterminating signal.
                                unsafe {
                                    let foreground = libc::tcgetpgrp(input.as_raw_fd());
                                    if foreground > 1 {
                                        libc::kill(-foreground, libc::SIGWINCH);
                                    }
                                }
                            }
                            if announced {
                                if let Some((code, _)) = exit {
                                    viewer.queue(&Response::Exit { code })?;
                                }
                            }
                        }
                        Request::Input { token, bytes }
                            if control.as_ref() == Some(&token)
                                && viewer.token.as_ref() == Some(&token) =>
                        {
                            anyhow::ensure!(
                                pending_input.len() + bytes.len() <= LIMIT,
                                "Provider input is busy"
                            );
                            pending_input.extend(bytes);
                        }
                        Request::Resize { token, rows, cols }
                            if terminal
                                && control.as_ref() == Some(&token)
                                && viewer.token.as_ref() == Some(&token) =>
                        {
                            let (rows, cols) = dimensions(rows, cols);
                            resize(input.as_raw_fd(), rows, cols)?;
                            output.resize(rows, cols);
                        }
                        _ => {} // stale and passive viewers cannot write or release another claim
                    }
                }
                viewer.flush()?;
                Ok(())
            })();
            result.is_ok()
        });
        if detach {
            control = None;
            for viewer in &mut viewers {
                let _ = viewer.queue(&Response::Detached);
                let _ = viewer.flush();
            }
            viewers.clear();
            detach = false;
        }
        let detached = !viewers
            .iter()
            .any(|v| v.token.is_some() && v.token == control);
        if let Some(response) = output.take_reports() {
            viewers.retain_mut(|v| !v.attached || v.queue(&response).is_ok());
        }
        if !pending_input.is_empty() {
            match input.write(pending_input.as_slices().0) {
                Ok(n) => {
                    pending_input.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => pending_input.clear(),
            }
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
                let replies = output.process(*stream, &bytes[..n], detached);
                if pending_input.len() + replies.len() <= LIMIT {
                    pending_input.extend(replies);
                }
                let response = Response::Output {
                    stream: *stream,
                    bytes: bytes[..n].to_vec(),
                };
                viewers.retain_mut(|viewer| {
                    !viewer.attached
                        || viewer
                            .queue(&response)
                            .and_then(|_| Ok(viewer.flush()?))
                            .is_ok()
                });
            }
        }
        readers.retain(|(stream, _)| !drained.contains(stream));
        if exit.is_none() {
            if let Some(status) = child.try_wait()? {
                use std::os::unix::process::ExitStatusExt;
                let code = status
                    .code()
                    .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
                    as u8;
                exit = Some((code, Instant::now()));
            }
        }
        if let Some((code, at)) = exit {
            if !announced && readers.is_empty() && at.elapsed() > Duration::from_millis(50) {
                for viewer in &mut viewers {
                    let _ = viewer.queue(&Response::Exit { code });
                }
                announced = true;
            }
            if announced && at.elapsed() > Duration::from_secs(2) {
                group.disarm();
                let _ = std::fs::remove_file(path);
                return if code == 0 {
                    Ok(())
                } else {
                    Err(crate::process::CommandExit(code).into())
                };
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
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

struct RawTerminal(Option<libc::termios>);
impl RawTerminal {
    fn enter() -> io::Result<Self> {
        if !io::stdin().is_terminal() {
            return Ok(Self(None));
        }
        // SAFETY: termios is initialized by tcgetattr before it is read.
        let original = unsafe {
            let mut value = std::mem::zeroed();
            if libc::tcgetattr(0, &mut value) < 0 {
                return Err(io::Error::last_os_error());
            }
            value
        };
        let restore = original;
        crate::agent::register_interrupt_cleanup(move || {
            // SAFETY: restores attributes previously read from stdin.
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, &restore);
            }
        });
        let mut raw = original;
        // SAFETY: raw is initialized; these calls only change terminal attributes.
        unsafe {
            libc::cfmakeraw(&mut raw);
            libc::tcsetattr(0, libc::TCSANOW, &raw);
        }
        Ok(Self(Some(original)))
    }
}
impl Drop for RawTerminal {
    fn drop(&mut self) {
        if let Some(original) = self.0 {
            // SAFETY: restores attributes previously read from stdin.
            unsafe {
                libc::tcsetattr(0, libc::TCSANOW, &original);
            }
            let _ = io::stdout().write_all(b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?2004l\x1b[?1049l\x1b[?25h\x1b[0m");
        }
    }
}

fn send(socket: &mut UnixStream, request: &Request) -> Result<()> {
    let mut bytes = serde_json::to_vec(request)?;
    bytes.push(b'\n');
    socket.write_all(&bytes)?;
    Ok(())
}

pub fn attach(id: &LfProcessId, observe: bool) -> Result<()> {
    let mut socket = UnixStream::connect(endpoint(id)?).context(
        "Execution owner is unavailable; no provider was restarted and no input was replayed",
    )?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut acknowledgement_deadline = Some(Instant::now() + ATTACH_TIMEOUT);
    let (rows, cols) = size();
    send(
        &mut socket,
        &Request::Attach {
            control: !observe,
            rows,
            cols,
        },
    )?;
    let mut terminal = None;
    let mut token = None;
    let mut buffer = Vec::new();
    let mut stdin_open = true;
    let mut previous_size = (rows, cols);
    loop {
        anyhow::ensure!(
            acknowledgement_deadline.is_none_or(|deadline| Instant::now() < deadline),
            "Execution owner did not acknowledge attachment; outcome is unknown, no provider was restarted and no input was replayed"
        );
        let mut fds = [
            libc::pollfd {
                fd: socket.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if stdin_open && (token.is_some() || terminal.is_some()) {
                    0
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: poll receives a live array of exactly two descriptors.
        if unsafe { libc::poll(fds.as_mut_ptr(), 2, 100) } < 0 {
            return Err(io::Error::last_os_error().into());
        }
        if fds[0].revents != 0 {
            let mut bytes = [0; 16384];
            let n = socket.read(&mut bytes)?;
            anyhow::ensure!(n != 0, "Execution owner disconnected; outcome is unknown");
            buffer.extend_from_slice(&bytes[..n]);
            anyhow::ensure!(buffer.len() <= LIMIT, "Transport response too large");
            while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
                let response: Response = serde_json::from_slice(&buffer[..end])?;
                buffer.drain(..=end);
                match response {
                    Response::Attached {
                        token: claim,
                        terminal: is_terminal,
                        bytes,
                    } => {
                        acknowledgement_deadline = None;
                        token = claim;
                        if is_terminal {
                            terminal = Some(RawTerminal::enter()?);
                        }
                        io::stdout().write_all(&bytes)?;
                        io::stdout().flush()?;
                    }
                    Response::Output { stream, bytes } => {
                        if stream == 2 {
                            io::stderr().write_all(&bytes)?;
                            io::stderr().flush()?;
                        } else {
                            io::stdout().write_all(&bytes)?;
                            io::stdout().flush()?;
                        }
                    }
                    Response::Exit { code } => {
                        return if code == 0 {
                            Ok(())
                        } else {
                            Err(crate::process::CommandExit(code).into())
                        }
                    }
                    Response::Detached => return Ok(()),
                }
            }
        }
        if fds[1].revents != 0 {
            let mut bytes = [0; 8192];
            let n = io::stdin().read(&mut bytes)?;
            if n == 0 {
                stdin_open = false;
            } else if terminal.is_some()
                && (bytes[..n].contains(&0x1d) || (observe && bytes[..n].contains(&0x03)))
            {
                return Ok(());
            } else if let Some(token) = &token {
                send(
                    &mut socket,
                    &Request::Input {
                        token: token.clone(),
                        bytes: bytes[..n].to_vec(),
                    },
                )?;
            }
        }
        let current_size = size();
        if terminal.is_some() && current_size != previous_size {
            previous_size = current_size;
            if let Some(token) = &token {
                send(
                    &mut socket,
                    &Request::Resize {
                        token: token.clone(),
                        rows: current_size.0,
                        cols: current_size.1,
                    },
                )?;
            }
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
        &Request::DetachViewers {
            key: handoff.key.clone(),
        },
    )
}

/// A Flow and its steps share one terminal owner. This is an explicit execution
/// handoff, not authority inferred from process ancestry.
pub(crate) fn configure_child(command: &mut Command) {
    if let Some(handoff) = HANDOFF.get() {
        command
            .env(OWNER_ENV, handoff.owner.as_str())
            .env(PARENT_ENV, std::process::id().to_string())
            .env("LF_TRANSPORT_CONTROL", &handoff.key);
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
    let id = format!(
        "lf/{}{}",
        process.as_str().replace('-', ""),
        if step { "/step" } else { "" }
    );
    let result = (|| -> Result<()> {
        let mut socket = UnixStream::connect(endpoint(&handoff.owner)?)?;
        socket.set_write_timeout(Some(Duration::from_secs(1)))?;
        send(
            &mut socket,
            &Request::Status {
                key: handoff.key.clone(),
                id,
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
