//! The server's side of a holder connection, and how a holder is started.

use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use super::wire::{self, Answer, Decoder, Frame, Held, Request, Start};
use super::Place;

/// How long one request waits for its answer line.
const ANSWER: Duration = Duration::from_secs(10);
/// How long a started holder has to accept its first connection.
const STARTUP: Duration = Duration::from_secs(20);

/// A holder as its `hello` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Welcome {
    pub revision: u32,
    pub build: String,
    pub pid: u32,
}

/// One request connection to a holder.
#[derive(Debug)]
pub struct Client {
    socket: BufReader<UnixStream>,
}

impl Client {
    /// Connect to the holder at `place`. A refusal or a missing socket says no
    /// holder is listening there; it says nothing about agents.
    pub fn connect(place: &Place) -> io::Result<Self> {
        Self::connect_within(place, ANSWER)
    }

    /// Connect, giving each request `answer` to be answered: an event loop
    /// serving many terminals cannot wait long on one holder.
    pub fn connect_within(place: &Place, answer: Duration) -> io::Result<Self> {
        let socket = UnixStream::connect(&place.socket)?;
        socket.set_read_timeout(Some(answer))?;
        socket.set_write_timeout(Some(answer))?;
        Ok(Self {
            socket: BufReader::new(socket),
        })
    }

    /// The holder at `place`, started first when none is listening. Starting is
    /// idempotent: a holder that is alive keeps its lock and its socket.
    pub fn ensure(place: &Place) -> Result<Self> {
        if let Ok(client) = Self::connect(place) {
            return Ok(client);
        }
        spawn()?;
        let deadline = Instant::now() + STARTUP;
        loop {
            match Self::connect(place) {
                Ok(client) => return Ok(client),
                Err(error) if Instant::now() >= deadline => {
                    return Err(error).with_context(|| {
                        format!("Holder at {} is not answering", place.socket.display())
                    })
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    pub fn request(&mut self, request: &Request) -> Result<Answer> {
        self.socket.get_mut().write_all(&wire::line(request))?;
        let mut line = String::new();
        self.socket
            .read_line(&mut line)
            .context("Holder did not answer; the request's outcome is unknown")?;
        anyhow::ensure!(
            !line.is_empty(),
            "Holder closed the connection; the request's outcome is unknown"
        );
        Ok(serde_json::from_str(&line)?)
    }

    pub fn hello(&mut self) -> Result<Welcome> {
        match self.request(&Request::Hello {
            revision: wire::REVISION,
        })? {
            Answer::Welcome {
                revision,
                build,
                pid,
            } => Ok(Welcome {
                revision,
                build,
                pid,
            }),
            Answer::Incompatible { revision } => {
                bail!("Holder speaks revision {revision}, newer than this lf; run lf install")
            }
            other => bail!("Holder answered hello with {other:?}"),
        }
    }

    pub fn list(&mut self) -> Result<Vec<Held>> {
        match self.request(&Request::List)? {
            Answer::Terminals { terminals } => Ok(terminals),
            other => bail!("Holder answered list with {other:?}"),
        }
    }

    /// Start a child, or learn the pid of the one this request already started.
    pub fn start(&mut self, start: Start) -> Result<u32> {
        match self.request(&Request::Start(start))? {
            Answer::Started { pid } => Ok(pid),
            Answer::Error { message } => bail!("Holder could not start: {message}"),
            other => bail!("Holder answered start with {other:?}"),
        }
    }

    pub fn resize(&mut self, terminal: &str, rows: u16, cols: u16) -> Result<()> {
        self.done(Request::Resize {
            terminal: terminal.to_owned(),
            rows,
            cols,
        })
    }

    pub fn forget(&mut self, terminal: &str) -> Result<()> {
        self.done(Request::Forget {
            terminal: terminal.to_owned(),
        })
    }

    fn done(&mut self, request: Request) -> Result<()> {
        match self.request(&request)? {
            Answer::Done => Ok(()),
            Answer::Error { message } => bail!("{message}"),
            other => bail!("Holder answered with {other:?}"),
        }
    }

    /// Turn this connection into `terminal`'s output from byte `from`.
    pub fn read(mut self, terminal: &str, from: u64) -> Result<Output> {
        match self.request(&Request::Read {
            terminal: terminal.to_owned(),
            from,
        })? {
            Answer::Reading => {}
            Answer::Error { message } => bail!("{message}"),
            other => bail!("Holder answered read with {other:?}"),
        }
        let mut decoder = Decoder::default();
        decoder.extend(self.socket.buffer());
        let socket = self.socket.into_inner();
        // macOS refuses socket options once the holder has sent an ended
        // terminal's last frames and closed; those frames are still readable.
        let _ = socket.set_read_timeout(None);
        Ok(Output { socket, decoder })
    }

    /// Turn this connection into `terminal`'s input.
    pub fn write(mut self, terminal: &str) -> Result<UnixStream> {
        match self.request(&Request::Write {
            terminal: terminal.to_owned(),
        })? {
            Answer::Writing => {}
            Answer::Error { message } => bail!("{message}"),
            other => bail!("Holder answered write with {other:?}"),
        }
        let socket = self.socket.into_inner();
        let _ = socket.set_write_timeout(None);
        Ok(socket)
    }
}

/// One terminal's output stream.
#[derive(Debug)]
pub struct Output {
    pub socket: UnixStream,
    pub decoder: Decoder,
}

impl Output {
    /// The next frame, waiting for it. End of stream without an exit frame is
    /// a lost connection, not an exit.
    pub fn frame(&mut self) -> io::Result<Frame> {
        loop {
            if let Some(frame) = self.decoder.frame()? {
                return Ok(frame);
            }
            let mut bytes = [0; 16384];
            match self.socket.read(&mut bytes)? {
                0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                n => self.decoder.extend(&bytes[..n]),
            }
        }
    }
}

/// Start this build's holder in its own session, so that stopping or replacing
/// the server's service leaves it running, and wait until it is listening or
/// has found another holder of this build already there.
fn spawn() -> Result<()> {
    let logs = crate::store::lf_home_dir().join("logs");
    std::fs::create_dir_all(&logs)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("holder.log"))
        .with_context(|| format!("Cannot open holder log in {}", logs.display()))?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["holder", "run"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log);
    // SAFETY: only an async-signal-safe syscall is used between fork and exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("Start lf holder run")?;
    let output = child.stdout.take().expect("ready pipe configured");
    let started = crate::journal::process_started_at(child.id())
        .ok()
        .flatten();
    std::thread::spawn(move || {
        let status = child.wait();
        if let (Some(started), Ok(status)) = (started, status) {
            settle(child.id(), started, status);
        }
    });
    // A ready line, or end of file from a holder that found the lock taken.
    let mut line = String::new();
    BufReader::new(output).read_line(&mut line)?;
    Ok(())
}

// ---- The holder's LfProcess ----

/// The LfProcess a holder is recorded as. Its id comes from the holder's pid
/// and OS start time, so every server that meets the same holder names the
/// same row, however many terminals it holds.
fn holder_process(pid: u32, os_started_at: i64) -> crate::process::LfProcess {
    let name = format!("holder:{pid}:{os_started_at}");
    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, name.as_bytes());
    crate::process::LfProcess {
        id: crate::id::LfProcessId::parse(&id.to_string()).expect("a UUID is an LfProcessId"),
        pid: Some(pid),
        os_started_at: Some(os_started_at),
        trace_id: crate::id::TraceId::new(),
        parent_lf_process_id: None,
        via_agent: Some(false),
        caller_session_id: None,
        caller_agent_process_id: None,
        command: Some(r#"["lf","holder","run"]"#.to_owned()),
        repo: None,
        cwd: None,
        started_at: os_started_at,
        completed_at: None,
        outcome: None,
        exit_code: None,
        signal: None,
        error: None,
    }
}

/// Record the holder `pid` as running. Recording again writes nothing; a
/// store that cannot be written leaves the holder unlisted, never stopped.
pub fn record(pid: u32) {
    let Ok(Some(started)) = crate::journal::process_started_at(pid) else {
        return;
    };
    let recorded = crate::journal::process_ledger()
        .and_then(|store| store.record_process(&holder_process(pid, started)));
    if let Err(error) = recorded {
        tracing::warn!(%error, pid, "holder process not recorded");
    }
}

/// Record that the holder this server started has exited.
fn settle(pid: u32, os_started_at: i64, status: std::process::ExitStatus) {
    let mut process = holder_process(pid, os_started_at);
    process.completed_at = Some(time::OffsetDateTime::now_utc().unix_timestamp());
    process.outcome = Some(
        if status.success() {
            "succeeded"
        } else {
            "failed"
        }
        .into(),
    );
    process.exit_code = status.code();
    let recorded =
        crate::journal::process_ledger().and_then(|store| store.record_process(&process));
    if let Err(error) = recorded {
        tracing::warn!(%error, pid, "holder exit not recorded");
    }
}
