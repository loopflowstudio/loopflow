//! A detached terminal owns an execution, including its capture and native client.
//! Viewers hold revocable input tokens, never provider attachment capabilities.

mod lifetime;
mod native;
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
const CONTROL_ENV: &str = "LF_TRANSPORT_CONTROL";
const LIMIT: usize = 4 * 1024 * 1024;
const ATTACH_TIMEOUT: Duration = Duration::from_secs(5);
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Request {
    Attach {
        control: bool,
        replace: bool,
        rows: u16,
        cols: u16,
    },
    Input {
        token: String,
        bytes: Vec<u8>,
    },
    EndInput {
        token: String,
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
    /// The direct child's native client began. `exit` replaces the client's own
    /// exit text; a detaching viewer prints `detach`.
    NativeStarted {
        key: String,
        exit: String,
        detach: String,
    },
    /// The native client exited; acknowledged once its exit line is delivered.
    NativeEnded {
        key: String,
    },
    /// What a controlling viewer can type, shown when it attaches and while the
    /// registering connection stays open.
    InputHint {
        key: String,
        hint: String,
    },
    /// Whether some viewer holds input control, asked before opening another.
    Controlled,
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
    Busy,
    /// How a viewer that detaches can return.
    Return {
        detach: String,
    },
    NativeEnded,
    InputHint {
        hint: String,
    },
    Controlled {
        held: bool,
    },
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
    if current().is_some() {
        return Ok(false);
    }
    // Library tests do not execute the CLI entry point.
    #[cfg(test)]
    {
        let _ = batch;
        Ok(false)
    }
    #[cfg(not(test))]
    {
        let mut argv: Vec<String> = std::env::args().collect();
        crate::journal::admit_process(&std::env::current_dir()?, &argv);
        crate::journal::current_lf_process_id()
            .context("Execution requires an admitted invocation; check the process ledger and caller provenance")?;
        argv[0] = std::env::current_exe()?.to_string_lossy().into_owned();
        // `lf -b … > file` typed at a terminal writes only the answer, never a
        // relayed screen; an interactive run keeps the terminal it reads from.
        let terminal = io::stdout().is_terminal() || (!batch && io::stdin().is_terminal());
        let owner = start(&argv, terminal, &std::env::current_dir()?, &|_| {})?;
        attach(&owner, false, true)?;
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
    std::fs::create_dir_all(&logs)
        .with_context(|| format!("Cannot open execution transport log in {}", logs.display()))?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("transport.log"))
        .with_context(|| format!("Cannot open execution transport log in {}", logs.display()))?;
    let mut command = Command::new(std::env::current_exe()?);
    command.args(["session", "serve"]);
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

fn endpoint(id: &LfProcessId) -> Result<PathBuf> {
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
    // An unopened terminal may report zero. vt100 needs room for a wide glyph
    // and a previous row when wrapping, even in a temporarily collapsed pane.
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

#[derive(Debug)]
struct Viewer {
    socket: UnixStream,
    input: Vec<u8>,
    output: VecDeque<u8>,
    token: Option<String>,
    attached: bool,
    ending: bool,
    input_hint: Option<String>,
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

/// Own a command's transport until it exits; only `start` launches this.
pub fn serve(argv: &[String], terminal: bool) -> Result<()> {
    let id =
        crate::journal::current_lf_process_id().context("Transport needs an admitted Process")?;
    let path = endpoint(&id).context("Prepare transport socket")?;
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("Bind transport socket {}", path.display()))?;
    listener.set_nonblocking(true)?;
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
    let mut child = command.spawn()?;
    // Command retains its configured slave descriptors after spawn. Close them
    // so Linux reports PTY EOF when the execution closes its own descriptors.
    drop(command);
    let group = crate::os_process::ProcessGroupGuard::new(child.id());
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
    let mut control: Option<String> = None;
    let mut pending_input = VecDeque::new();
    let mut output = output::Output::new(terminal);
    let mut exit: Option<(u8, Instant)> = None;
    let mut detach = false;
    let mut client: Option<native::NativeClient> = None;
    let mut returning = None;
    let mut hint_due = false;
    let mut announced = false;
    let mut attached = false;
    let (viewing, ending) = lifetime::spawn(id.clone());
    let mut viewed = None;
    loop {
        while let Ok((socket, _)) = listener.accept() {
            socket.set_nonblocking(true)?;
            viewers.push(Viewer {
                socket,
                input: Vec::new(),
                output: VecDeque::new(),
                token: None,
                attached: false,
                ending: false,
                input_hint: None,
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
                        Request::NativeStarted { key, exit, detach }
                            if key == control_key && terminal =>
                        {
                            returning = Some(Response::Return {
                                detach: detach.clone(),
                            });
                            client = Some(native::NativeClient::new(exit, detach, cooked));
                        }
                        Request::NativeEnded { key } if key == control_key => {
                            viewer.ending = true;
                        }
                        Request::InputHint { key, hint } if key == control_key => {
                            viewer.input_hint = Some(hint);
                            hint_due = true;
                        }
                        Request::Controlled => {
                            viewer.queue(&Response::Controlled {
                                held: control.is_some(),
                            })?;
                        }
                        Request::Attach {
                            control: wants_control,
                            replace,
                            rows,
                            cols,
                        } => {
                            if wants_control
                                && !replace
                                && control.is_some()
                                && viewer.token != control
                            {
                                viewer.queue(&Response::Busy)?;
                                continue;
                            }
                            attached = true;
                            let (rows, cols) = dimensions(rows, cols);
                            if wants_control {
                                hint_due = true;
                                let token = uuid::Uuid::new_v4().to_string();
                                control = Some(token.clone());
                                viewer.token = Some(token);
                                if terminal {
                                    resize(
                                        input
                                            .as_ref()
                                            .expect("terminal input stays open")
                                            .as_raw_fd(),
                                        rows,
                                        cols,
                                    )?;
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
                            if let Some(client) = &client {
                                viewer.queue(&Response::Return {
                                    detach: client.detach.clone(),
                                })?;
                            }
                            if terminal && wants_control {
                                // Resizing to the same dimensions sends no signal.
                                // Ask the actual foreground program to repaint too.
                                // SAFETY: the descriptor is our PTY; only its current
                                // foreground group receives the nonterminating signal.
                                unsafe {
                                    let foreground = libc::tcgetpgrp(
                                        input
                                            .as_ref()
                                            .expect("terminal input stays open")
                                            .as_raw_fd(),
                                    );
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
                                && viewer.token.as_ref() == Some(&token)
                                && !end_input =>
                        {
                            anyhow::ensure!(
                                pending_input.len() + bytes.len() <= LIMIT,
                                "Provider input is busy"
                            );
                            pending_input.extend(bytes);
                        }
                        Request::EndInput { token }
                            if !terminal
                                && control.as_ref() == Some(&token)
                                && viewer.token.as_ref() == Some(&token) =>
                        {
                            end_input = true;
                        }
                        Request::Resize { token, rows, cols }
                            if terminal
                                && control.as_ref() == Some(&token)
                                && viewer.token.as_ref() == Some(&token) =>
                        {
                            let (rows, cols) = dimensions(rows, cols);
                            resize(
                                input
                                    .as_ref()
                                    .expect("terminal input stays open")
                                    .as_raw_fd(),
                                rows,
                                cols,
                            )?;
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
        if let Some(response) = returning.take() {
            viewers.retain_mut(|v| !v.attached || v.queue(&response).is_ok());
        }
        // The controls reach the one controlling viewer when it attaches or
        // when the execution registers them, whichever comes later.
        if std::mem::take(&mut hint_due) {
            let hint = viewers.iter().rev().find_map(|v| v.input_hint.clone());
            if let Some(hint) = hint {
                viewers.retain_mut(|v| {
                    !(v.attached && v.token.is_some() && v.token == control)
                        || v.queue(&Response::InputHint { hint: hint.clone() }).is_ok()
                });
            }
        }
        if let Ok(hint) = ending.try_recv() {
            for viewer in viewers.iter_mut().filter(|v| v.attached) {
                if let Some(hint) = &hint {
                    let _ = viewer.queue(&Response::InputHint { hint: hint.clone() });
                }
                let _ = viewer.flush();
            }
            crate::platform::kill_process(std::process::id());
        }
        let now_viewed = viewers.iter().any(|v| v.attached);
        if viewed != Some(now_viewed) {
            viewed = Some(now_viewed);
            let _ = viewing.send(now_viewed);
        }
        let detached = !viewers
            .iter()
            .any(|v| v.token.is_some() && v.token == control);
        if detached {
            control = None;
        }
        if let Some(response) = output.take_reports() {
            viewers.retain_mut(|v| !v.attached || v.queue(&response).is_ok());
        }
        if let Some(input) = input.as_mut().filter(|_| !pending_input.is_empty()) {
            match input.write(pending_input.as_slices().0) {
                Ok(n) => {
                    pending_input.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => pending_input.clear(),
            }
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
            }
        }
        readers.retain(|(stream, _)| !drained.contains(stream));
        // Output written before the execution detached reaches its viewers first.
        if detach {
            control = None;
            for viewer in viewers.iter_mut().filter(|v| v.attached) {
                let _ = viewer.queue(&Response::Detached);
                let _ = viewer.flush();
            }
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
        // The client has exited and its parent waits for acknowledgement, so
        // everything it printed is already readable.
        if viewers.iter().any(|viewer| viewer.ending) {
            if let Some(mut ended) = client.take() {
                for (stream, reader) in &mut readers {
                    let mut bytes = [0; 16384];
                    for _ in 0..LIMIT / bytes.len() {
                        let Ok(n @ 1..) = reader.read(&mut bytes) else {
                            break;
                        };
                        let ready = ended.accept(reader.as_raw_fd(), &bytes[..n]);
                        deliver(
                            &mut output,
                            &mut viewers,
                            &mut pending_input,
                            *stream,
                            &ready,
                            detached,
                        );
                    }
                }
                let exit = ended.finish();
                deliver(
                    &mut output,
                    &mut viewers,
                    &mut pending_input,
                    1,
                    &exit,
                    detached,
                );
            }
            for viewer in viewers.iter_mut().filter(|viewer| viewer.ending) {
                viewer.ending = false;
                let _ = viewer.queue(&Response::NativeEnded);
                let _ = viewer.flush();
            }
        }
        if exit.is_none() {
            if let Some(status) = child.try_wait()? {
                use std::os::unix::process::ExitStatusExt;
                let code = status
                    .code()
                    .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
                    as u8;
                exit = Some((code, Instant::now()));
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
            }
        }
        if let Some((code, at)) = exit {
            if !announced && readers.is_empty() && at.elapsed() > Duration::from_millis(50) {
                for viewer in &mut viewers {
                    let _ = viewer.queue(&Response::Exit { code });
                }
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

fn deliver(
    output: &mut output::Output,
    viewers: &mut Vec<Viewer>,
    pending_input: &mut VecDeque<u8>,
    stream: u8,
    bytes: &[u8],
    detached: bool,
) {
    if bytes.is_empty() {
        return;
    }
    let replies = output.process(stream, bytes, detached);
    if pending_input.len() + replies.len() <= LIMIT {
        pending_input.extend(replies);
    }
    let response = Response::Output {
        stream,
        bytes: bytes.to_vec(),
    };
    viewers.retain_mut(|viewer| {
        !viewer.attached
            || viewer
                .queue(&response)
                .and_then(|_| Ok(viewer.flush()?))
                .is_ok()
    });
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

struct RawTerminal {
    original: Option<libc::termios>,
    // Drop restores terminal modes before output announces any diagnostics.
    output: crate::terminal_output::TerminalOutput,
    alternate: terminal::AlternateScreen,
}
impl RawTerminal {
    fn enter() -> Result<Self> {
        let output = crate::terminal_output::TerminalOutput::enter()?;
        if !io::stdin().is_terminal() {
            return Ok(Self {
                original: None,
                output,
                alternate: Default::default(),
            });
        }
        // SAFETY: termios is initialized by tcgetattr before it is read.
        let original = unsafe {
            let mut value = std::mem::zeroed();
            if libc::tcgetattr(0, &mut value) < 0 {
                return Err(io::Error::last_os_error().into());
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
        Ok(Self {
            original: Some(original),
            output,
            alternate: Default::default(),
        })
    }

    fn write(&mut self, stream: u8, bytes: &[u8]) -> io::Result<()> {
        self.alternate.observe(bytes);
        if stream == 2 {
            self.output.stderr.write_all(bytes)
        } else {
            self.output.stdout.write_all(bytes)
        }
    }
}
impl Drop for RawTerminal {
    fn drop(&mut self) {
        if let Some(original) = self.original {
            // Discard input the viewer never forwarded, such as mouse reports
            // queued while it waited on the owner; the shell would run them.
            // SAFETY: restores attributes previously read from stdin.
            unsafe {
                libc::tcsetattr(0, libc::TCSAFLUSH, &original);
            }
            let _ = self.output.stdout.write_all(
                b"\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?2004l\x1b[?25h\x1b[0m",
            );
            if self.alternate.entered {
                let _ = self.output.stdout.write_all(b"\x1b[?1049l");
            }
        }
    }
}

fn send(socket: &mut UnixStream, request: &Request) -> Result<()> {
    let mut bytes = serde_json::to_vec(request)?;
    bytes.push(b'\n');
    // A write timeout reports EAGAIN on macOS, which names no cause.
    socket.write_all(&bytes).map_err(|error| match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => anyhow::anyhow!(
            "Execution owner stopped reading requests; outcome is unknown and later input was not delivered"
        ),
        _ => error.into(),
    })
}

pub(crate) fn attach(id: &LfProcessId, observe: bool, replace: bool) -> Result<()> {
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
            replace,
            rows,
            cols,
        },
    )?;
    let mut terminal = None;
    let mut token = None;
    let mut returning = None;
    let mut buffer = Vec::new();
    let mut stdin_open = true;
    let mut previous_size = (rows, cols);
    let mut title = crate::terminal_title::ViewerTitle::new(id);
    loop {
        title.tick();
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
                        if let Some(terminal) = &mut terminal {
                            terminal.write(1, &bytes)?;
                        } else {
                            io::stdout().write_all(&bytes)?;
                            io::stdout().flush()?;
                        }
                    }
                    Response::Output { stream, bytes } => {
                        if let Some(terminal) = &mut terminal {
                            terminal.write(stream, &bytes)?;
                        } else if stream == 2 {
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
                    Response::Busy => anyhow::bail!("Session is attached in another viewer; use an explicit attachment to move input control here"),
                    Response::Return { detach } => returning = Some(detach),
                    Response::NativeEnded | Response::Controlled { .. } => {}
                    Response::InputHint { hint } => {
                        if let Some(terminal) = &mut terminal {
                            write!(terminal.output.stderr, "\r\n{hint}\r\n")?;
                        } else {
                            eprintln!("{hint}");
                        }
                    }
                }
            }
        }
        if fds[1].revents != 0 {
            let mut bytes = [0; 8192];
            let n = io::stdin().read(&mut bytes)?;
            if n == 0 {
                stdin_open = false;
                if terminal.is_none() {
                    if let Some(token) = &token {
                        send(
                            &mut socket,
                            &Request::EndInput {
                                token: token.clone(),
                            },
                        )?;
                    }
                }
            } else if terminal.is_some()
                && (bytes[..n].contains(&0x1d) || (observe && bytes[..n].contains(&0x03)))
            {
                // Restore the terminal before saying how to return.
                drop(terminal.take());
                if let Some(detach) = &returning {
                    // An inline client can leave the cursor mid-line.
                    eprintln!("\n{detach}");
                }
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

/// Whether a viewer of this execution holds input control. A closed terminal
/// leaves its execution running with none, ready for the next viewer.
pub(crate) fn controlled(id: &LfProcessId) -> Result<bool> {
    let mut socket = UnixStream::connect(endpoint(id)?)?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    send(&mut socket, &Request::Controlled)?;
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line)?;
    match serde_json::from_str(&line)? {
        Response::Controlled { held } => Ok(held),
        other => bail!("unexpected transport reply {other:?}"),
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
            &Request::NativeStarted {
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
                &Request::NativeEnded {
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
            &Request::InputHint {
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

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::process::Command;

    use super::{carry_across_exec, Handoff, CONTROL_ENV, OWNER_ENV, PARENT_ENV};
    use crate::id::LfProcessId;

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
