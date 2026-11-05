//! Blocking client for the server protocol: the readiness probe, one
//! connection with requests by id, and the owner bridge.
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read as _, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use super::wire::{
    ClientIdentity, ClientMessage, ConnectedClient, Hello, Operation, ServerError, ServerMessage,
    ServerOperation, Welcome, MAX_MESSAGE, MIN_REVISION, REVISION,
};
use crate::id::LfProcessId;
use crate::lf::commands::read::Read;

/// How long the server has to answer `status`, `hello` or `attach`. A session's
/// later waits are unbounded unless the caller sets its own limit.
const HANDSHAKE: Duration = Duration::from_secs(10);

/// `lf server status`: readiness observed without opening Work.
#[derive(Debug, serde::Serialize)]
pub struct Status {
    pub socket: PathBuf,
    pub ready: bool,
    /// A server accepted the connection, whatever it answered.
    pub listening: bool,
    pub instance: Option<String>,
    /// The newest protocol revision the server speaks.
    pub revision: Option<u32>,
    /// Every welcomed connection still open; empty when no server answered.
    pub clients: Vec<ConnectedClient>,
    pub error: Option<String>,
}

pub fn status() -> Result<Status> {
    let mut status = Status {
        socket: super::socket_path()?,
        ready: false,
        listening: false,
        instance: None,
        revision: None,
        clients: Vec::new(),
        error: None,
    };
    let stream = match connect(&status.socket) {
        Ok(stream) => stream,
        Err(error) => {
            status.error = Some(error.to_string());
            return Ok(status);
        }
    };
    status.listening = true;
    stream.set_read_timeout(Some(HANDSHAKE))?;
    let mut socket = Socket::new(stream, 64 * 1024)?;
    status.error = match socket
        .send(&ClientMessage::Status)
        .and_then(|()| socket.receive())
    {
        Ok(None) => Some(OLDER_PROTOCOL.into()),
        Ok(Some(ServerMessage::Status(answer))) => {
            status.instance = Some(answer.instance);
            status.revision = Some(answer.revision);
            status.clients = answer.clients;
            let expected = crate::installation::ArtifactIdentity::capture(
                crate::installation::ArtifactRole::Cli,
                &std::env::current_exe()?,
            )?
            .sha256;
            if answer.executable_sha256 != expected {
                Some(
                    "Loopflow server uses another executable; obtain the selected installation"
                        .into(),
                )
            } else if answer.min_revision > REVISION || answer.revision < MIN_REVISION {
                Some(format!(
                    "Loopflow server speaks protocol revisions {}–{}; this build speaks {MIN_REVISION}–{REVISION}; update the selected installation",
                    answer.min_revision, answer.revision
                ))
            } else {
                None
            }
        }
        Ok(Some(ServerMessage::Error { code, message, .. })) => Some(format!("{code}: {message}")),
        Ok(Some(_)) => Some("Server did not answer the readiness request".into()),
        Err(error) => Some(stalled(error, "answer the readiness request").to_string()),
    };
    status.ready = status.error.is_none();
    Ok(status)
}

/// A revision-1 server reads `{"ty` as an absurd frame length and closes.
const OLDER_PROTOCOL: &str = "Loopflow server closed the connection without answering; it may speak an older protocol; update the selected installation";

/// What a request returned, and the trace that correlates its diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub trace_id: crate::id::TraceId,
    pub result: serde_json::Value,
}

/// One connection: hello and welcome once, then requests, subscriptions and
/// cancels by id for its lifetime.
#[derive(Debug)]
pub struct Connection {
    socket: Socket,
    welcome: Welcome,
    /// Messages received while waiting for another id's answer.
    pending: VecDeque<ServerMessage>,
}

impl Connection {
    /// The selected Machine's server.
    pub fn open() -> Result<Self> {
        Self::open_at(&super::socket_path()?)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        Self::start(connect(path)?, 64 * 1024)
    }

    /// Hello and welcome within [`HANDSHAKE`]; the session's reads are then unbounded.
    fn start(stream: UnixStream, capacity: usize) -> Result<Self> {
        stream.set_read_timeout(Some(HANDSHAKE))?;
        let mut socket = Socket::new(stream, capacity)?;
        socket.send(&ClientMessage::Hello(Hello {
            revision: REVISION,
            min_revision: MIN_REVISION,
            client: ClientIdentity {
                kind: "cli".into(),
                platform: std::env::consts::OS.into(),
                name: "lf".into(),
                version: crate::build_info::BUILD_VERSION.into(),
            },
        }))?;
        let answer = socket
            .receive()
            .map_err(|error| stalled(error, "welcome the connection"))?;
        socket.writer.set_read_timeout(None)?;
        match answer {
            Some(ServerMessage::Welcome(welcome)) => Ok(Self {
                socket,
                welcome,
                pending: VecDeque::new(),
            }),
            Some(ServerMessage::Error {
                code,
                message,
                data,
                ..
            }) => Err(ServerError {
                code,
                message,
                data,
            }
            .into()),
            Some(other) => anyhow::bail!("Server did not welcome the connection: {other:?}"),
            None => anyhow::bail!(OLDER_PROTOCOL),
        }
    }

    pub fn welcome(&self) -> &Welcome {
        &self.welcome
    }

    /// How long a wait for the server may last; none by default.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        Ok(self.socket.writer.set_read_timeout(timeout)?)
    }

    /// Send a request without waiting; its answer arrives through [`Self::receive`].
    pub fn send(&mut self, operation: Operation) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.socket.send(&ClientMessage::Request {
            id: id.clone(),
            operation,
        })?;
        Ok(id)
    }

    /// One request and its answer; other ids' messages wait in `pending`.
    pub fn request(&mut self, operation: Operation) -> Result<Response> {
        let id = self.send(operation)?;
        match self.wait(|message| answers(message, &id))? {
            ServerMessage::Response {
                trace_id, result, ..
            } => Ok(Response { trace_id, result }),
            ServerMessage::Error {
                code,
                message,
                data,
                ..
            } => Err(ServerError {
                code,
                message,
                data,
            }
            .into()),
            other => anyhow::bail!("unexpected server message: {other:?}"),
        }
    }

    /// Start a subscription and return its id once the server confirms it.
    pub fn subscribe(&mut self, operation: Read) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.socket.send(&ClientMessage::Subscribe {
            id: id.clone(),
            operation,
        })?;
        match self.wait(|message| answers(message, &id))? {
            ServerMessage::Response { .. } => Ok(id),
            ServerMessage::Error {
                code,
                message,
                data,
                ..
            } => Err(ServerError {
                code,
                message,
                data,
            }
            .into()),
            other => anyhow::bail!("unexpected server message: {other:?}"),
        }
    }

    /// The next reading of subscription `id`; an error for it ends the subscription.
    pub fn reading(&mut self, id: &str) -> Result<serde_json::Value> {
        match self.wait(|message| {
            matches!(message, ServerMessage::Reading { id: of, .. } if of == id)
                || answers(message, id)
        })? {
            ServerMessage::Reading { frame, .. } => Ok(frame),
            ServerMessage::Error {
                code,
                message,
                data,
                ..
            } => Err(ServerError {
                code,
                message,
                data,
            }
            .into()),
            other => anyhow::bail!("unexpected server message: {other:?}"),
        }
    }

    /// Ask subscription `id` for a reading whose `answers` is `checkpoint`.
    pub fn checkpoint(&mut self, id: &str, checkpoint: u64) -> Result<()> {
        self.socket.send(&ClientMessage::Checkpoint {
            id: id.into(),
            checkpoint,
        })
    }

    fn wait(&mut self, mut matches: impl FnMut(&ServerMessage) -> bool) -> Result<ServerMessage> {
        if let Some(index) = self.pending.iter().position(&mut matches) {
            return Ok(self
                .pending
                .remove(index)
                .expect("position names a pending message"));
        }
        loop {
            let message = self
                .socket
                .receive()?
                .context("Loopflow server closed the connection; the outcome is unknown")?;
            if matches(&message) {
                return Ok(message);
            }
            self.pending.push_back(message);
        }
    }
}

/// A response or error for `id`, or an error for the whole connection.
fn answers(message: &ServerMessage, id: &str) -> bool {
    match message {
        ServerMessage::Response { id: of, .. } => of == id,
        ServerMessage::Error { id: of, .. } => of.as_deref().is_none_or(|of| of == id),
        _ => false,
    }
}

pub fn bridge(owner: &LfProcessId) -> Result<UnixStream> {
    // Restored terminals can open before Desktop's first Work reading obtains
    // the helper. Setup delegates through the same selected installation.
    let setup = crate::installation::server::readiness_command()?.output()?;
    anyhow::ensure!(
        setup.status.success(),
        "Loopflow server unavailable: {}",
        String::from_utf8_lossy(&setup.stderr)
    );
    bridge_existing(owner)
}

/// Recovery never registers a service or changes installation policy.
pub(crate) fn bridge_existing(owner: &LfProcessId) -> Result<UnixStream> {
    attach(&super::socket_path()?, owner)
}

/// Hello, attach, `bridged`: the stream then speaks the owner protocol. Until
/// then it reads one byte at a time, so no owner byte is left in a buffer.
pub fn attach(path: &Path, owner: &LfProcessId) -> Result<UnixStream> {
    let mut connection = Connection::start(connect(path)?, 1)?;
    connection.set_read_timeout(Some(HANDSHAKE))?;
    connection
        .request(Operation::Server(ServerOperation::Attach {
            owner: owner.clone(),
        }))
        .map_err(|error| stalled(error, "open the owner bridge"))?;
    let stream = connection.socket.reader.into_inner();
    stream.set_read_timeout(None)?;
    Ok(stream)
}

/// A wait that [`HANDSHAKE`] cut short, named for the answer it waited for.
fn stalled(error: anyhow::Error, answer: &str) -> anyhow::Error {
    match error
        .root_cause()
        .downcast_ref::<std::io::Error>()
        .map(std::io::Error::kind)
    {
        Some(std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => anyhow::anyhow!(
            "Loopflow server did not {answer} within {}s; check `lf server status`",
            HANDSHAKE.as_secs()
        ),
        _ => error,
    }
}

fn connect(path: &Path) -> Result<UnixStream> {
    let stream = UnixStream::connect(path).context(
        "Loopflow server unavailable; check `lf server status` and the selected installation",
    )?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

/// Newline-delimited JSON messages over one stream.
#[derive(Debug)]
struct Socket {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Socket {
    fn new(stream: UnixStream, capacity: usize) -> Result<Self> {
        Ok(Self {
            writer: stream.try_clone()?,
            reader: BufReader::with_capacity(capacity, stream),
        })
    }

    fn send(&mut self, message: &ClientMessage) -> Result<()> {
        let mut line = serde_json::to_string(message)?;
        anyhow::ensure!(
            line.len() <= MAX_MESSAGE,
            "Server request exceeds {MAX_MESSAGE} bytes"
        );
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;
        Ok(())
    }

    /// The next message; `None` once the server closed after a whole message.
    fn receive(&mut self) -> Result<Option<ServerMessage>> {
        let mut line = Vec::new();
        (&mut self.reader)
            .take(MAX_MESSAGE as u64 + 1)
            .read_until(b'\n', &mut line)?;
        if line.is_empty() {
            return Ok(None);
        }
        anyhow::ensure!(
            line.len() <= MAX_MESSAGE,
            "Server message exceeds {MAX_MESSAGE} bytes"
        );
        anyhow::ensure!(
            line.ends_with(b"\n"),
            "Loopflow server closed the connection mid-message"
        );
        Ok(Some(serde_json::from_slice(&line)?))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::time::{Duration, Instant};

    use super::{attach, Connection, HANDSHAKE};
    use crate::id::LfProcessId;
    use crate::server::wire::{
        Limits, ServerMessage, Welcome, MAX_CONNECTIONS, MAX_MESSAGE, MIN_REVISION, REVISION,
    };

    /// A server that accepts one connection and answers nothing until the
    /// client gives up; `welcome` first when `after_hello`.
    fn silent_server(after_hello: bool) -> (tempfile::TempDir, std::path::PathBuf) {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("server.sock");
        let listener = UnixListener::bind(&path).unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if after_hello {
                reader.read_line(&mut line).unwrap();
                let welcome = ServerMessage::Welcome(Welcome {
                    client_id: "fixture-client".into(),
                    revision: REVISION,
                    server_revision: REVISION,
                    min_revision: MIN_REVISION,
                    instance: "fixture".into(),
                    build: "test".into(),
                    executable_sha256: "f".repeat(64),
                    machine: None,
                    capabilities: Vec::new(),
                    limits: Limits {
                        max_message: MAX_MESSAGE,
                        max_connections: MAX_CONNECTIONS,
                    },
                });
                let mut text = serde_json::to_string(&welcome).unwrap();
                text.push('\n');
                stream.write_all(text.as_bytes()).unwrap();
            }
            // Hold the connection open until the client closes it.
            while reader.read_line(&mut line).unwrap() > 0 {}
        });
        (home, path)
    }

    fn within_bound(started: Instant) {
        let elapsed = started.elapsed();
        assert!(elapsed >= HANDSHAKE, "gave up early after {elapsed:?}");
        assert!(elapsed < HANDSHAKE * 2, "waited {elapsed:?}");
    }

    #[test]
    fn open_gives_up_on_a_server_that_never_welcomes() {
        let (_home, path) = silent_server(false);
        let started = Instant::now();
        let error = Connection::open_at(&path).expect_err("open stalled");
        within_bound(started);
        assert_eq!(
            error.to_string(),
            "Loopflow server did not welcome the connection within 10s; check `lf server status`"
        );
    }

    #[test]
    fn attach_gives_up_on_a_server_that_never_bridges() {
        let (_home, path) = silent_server(true);
        let started = Instant::now();
        let error = attach(&path, &LfProcessId::new()).expect_err("attach stalled");
        within_bound(started);
        assert_eq!(
            error.to_string(),
            "Loopflow server did not open the owner bridge within 10s; check `lf server status`"
        );
    }

    #[test]
    fn a_welcomed_session_reads_without_a_bound() {
        let (_home, path) = silent_server(true);
        let connection = Connection::open_at(&path).unwrap();
        let stream: &UnixStream = &connection.socket.writer;
        assert_eq!(stream.read_timeout().unwrap(), None);
        connection
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        assert_eq!(
            stream.read_timeout().unwrap(),
            Some(Duration::from_millis(50))
        );
    }
}
