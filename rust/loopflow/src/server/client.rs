//! Blocking client for the server protocol: the readiness probe, one
//! connection with requests by id, and the ticketed connections that carry a
//! terminal's bytes or one message over the control limit.
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read as _, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use super::wire::{
    ClientIdentity, ClientMessage, ConnectedClient, Hello, Operation, ServerMessage,
    ServerOperation, SessionAttached, Welcome, MAX_BULK, MAX_MESSAGE, MIN_REVISION, REVISION,
};
use crate::engine::Error;
use crate::id::LfSessionId;
use crate::lf::commands::read::Read;

/// How long the server has to answer `status`, `hello`, `session.attach` or
/// a ticket. A session's later waits are unbounded unless the caller sets its
/// own limit.
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
    let mut socket = Socket::new(stream)?;
    status.error = match socket
        .send(&ClientMessage::Status)
        .and_then(|()| socket.receive())
    {
        Ok(None) => Some(OLDER_PROTOCOL.into()),
        // Linux resets a Unix socket closed with the probe's bytes unread,
        // where macOS reports a clean close; both mean the same older server.
        Err(error) if resets(&error) => Some(OLDER_PROTOCOL.into()),
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
        Err(error) => Some(stalled(error, "answer the readiness request", HANDSHAKE).to_string()),
    };
    status.ready = status.error.is_none();
    Ok(status)
}

fn resets(error: &anyhow::Error) -> bool {
    error
        .root_cause()
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
            )
        })
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
    /// Where ticketed connections for bulk messages open.
    path: PathBuf,
}

impl Connection {
    /// The selected Machine's server.
    pub fn open() -> Result<Self> {
        Self::open_at(&super::socket_path()?)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        let connection = Self::start(connect(path)?, path, identity("cli", "lf"), HANDSHAKE)?;
        connection.set_read_timeout(None)?;
        Ok(connection)
    }

    /// Hello and welcome within `within`, which stays set until the caller
    /// has what it opened the connection for.
    fn start(
        stream: UnixStream,
        path: &Path,
        client: ClientIdentity,
        within: Duration,
    ) -> Result<Self> {
        stream.set_read_timeout(Some(within))?;
        let mut socket = Socket::new(stream)?;
        socket.send(&ClientMessage::Hello(Hello {
            revision: REVISION,
            min_revision: MIN_REVISION,
            client,
        }))?;
        let answer = socket
            .receive()
            .map_err(|error| stalled(error, "welcome the connection", within))?;
        match answer.map(accepted).transpose()? {
            Some(ServerMessage::Welcome(welcome)) => Ok(Self {
                socket,
                welcome,
                pending: VecDeque::new(),
                path: path.to_path_buf(),
            }),
            Some(ServerMessage::Error {
                code,
                message,
                data,
                ..
            }) => Err(Error {
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

    /// Send a request without waiting; its answer arrives through
    /// [`Self::request`] or [`Self::reading`]. A request over the control limit
    /// travels on a ticketed connection and is answered here like any other.
    pub fn send(&mut self, operation: Operation) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let line = Socket::line(&ClientMessage::Request {
            id: id.clone(),
            operation,
            select: None,
        })?;
        if line.len() <= MAX_MESSAGE {
            self.socket.write(&line)?;
        } else {
            self.upload(line)?;
        }
        Ok(id)
    }

    fn upload(&mut self, line: String) -> Result<()> {
        anyhow::ensure!(
            line.len() <= MAX_BULK,
            "Server request exceeds {MAX_BULK} bytes"
        );
        let ticket = self
            .request(Operation::Server(ServerOperation::RequestUpload {
                bytes: line.len(),
            }))?
            .result;
        let ticket = ticket["ticket"]
            .as_str()
            .context("Server did not issue an upload ticket")?;
        let mut stream = redeem(&self.path, ticket)?;
        stream.write_all(line.as_bytes())?;
        Ok(())
    }

    /// One message the server moved to a ticketed connection.
    fn fetch(&self, ticket: &str) -> Result<ServerMessage> {
        let stream = redeem(&self.path, ticket)?;
        let mut line = Vec::new();
        BufReader::new(stream)
            .take(MAX_BULK as u64 + 1)
            .read_until(b'\n', &mut line)?;
        anyhow::ensure!(
            line.len() <= MAX_BULK && line.ends_with(b"\n"),
            "Loopflow server did not deliver the bulk message"
        );
        Ok(serde_json::from_slice(&line)?)
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
            } => Err(Error {
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
            select: None,
            resume: None,
        })?;
        match self.wait(|message| answers(message, &id))? {
            ServerMessage::Response { .. } => Ok(id),
            ServerMessage::Error {
                code,
                message,
                data,
                ..
            } => Err(Error {
                code,
                message,
                data,
            }
            .into()),
            other => anyhow::bail!("unexpected server message: {other:?}"),
        }
    }

    /// The next frame in connection order, including typed events. Track event
    /// sequences before routing by ID; reconnect with fresh subscriptions on a gap.
    pub fn receive(&mut self) -> Result<Option<ServerMessage>> {
        match self.pending.pop_front() {
            Some(message) => Ok(Some(message)),
            None => self.socket.receive(),
        }
    }

    /// The next reading of subscription `id`, or `None` once it ended after
    /// its last; an error for it ends the subscription.
    pub fn reading(&mut self, id: &str) -> Result<Option<serde_json::Value>> {
        loop {
            let message = self.wait(|message| {
                matches!(message,
                    ServerMessage::Event { id: of, .. }
                    | ServerMessage::Reading { id: of, .. }
                    | ServerMessage::Ended { id: of } if of == id
                ) || answers(message, id)
            })?;
            match message {
                // Reading-only consumers must not accumulate an unused event queue.
                ServerMessage::Event { .. } => continue,
                ServerMessage::Reading { frame, .. } => return Ok(Some(frame)),
                ServerMessage::Ended { .. } => return Ok(None),
                ServerMessage::Error {
                    code,
                    message,
                    data,
                    ..
                } => {
                    return Err(Error {
                        code,
                        message,
                        data,
                    }
                    .into());
                }
                other => anyhow::bail!("unexpected server message: {other:?}"),
            }
        }
    }

    /// Ask subscription `id` for a reading whose `answers` is `checkpoint`.
    pub fn checkpoint(&mut self, id: &str, checkpoint: u64) -> Result<()> {
        self.socket.send(&ClientMessage::Checkpoint {
            id: id.into(),
            checkpoint,
        })
    }

    /// The first message `matches`; a refusal among them is the error.
    fn wait(&mut self, mut matches: impl FnMut(&ServerMessage) -> bool) -> Result<ServerMessage> {
        if let Some(index) = self.pending.iter().position(&mut matches) {
            return accepted(
                self.pending
                    .remove(index)
                    .expect("position names a pending message"),
            );
        }
        loop {
            let mut message = self
                .socket
                .receive()?
                .context("Loopflow server closed the connection; the outcome is unknown")?;
            if let ServerMessage::Bulk { ticket, .. } = &message {
                message = self.fetch(ticket)?;
            }
            if matches(&message) {
                return accepted(message);
            }
            self.pending.push_back(message);
        }
    }
}

fn identity(kind: &str, name: &str) -> ClientIdentity {
    ClientIdentity {
        kind: kind.into(),
        platform: std::env::consts::OS.into(),
        name: name.into(),
        version: crate::build_info::BUILD_VERSION.into(),
    }
}

/// A provider hook's one request: connecting, `hello` and the answer share
/// `within`, so a stopped, restarting or slow server costs the turn at most that.
pub fn hook_request(path: &Path, operation: Operation, within: Duration) -> Result<Response> {
    let deadline = std::time::Instant::now() + within;
    let stream = connect(path)?;
    stream.set_write_timeout(Some(within))?;
    let client = identity("hook", "lf __context-block");
    let mut connection = Connection::start(stream, path, client, within)?;
    let left = deadline.saturating_duration_since(std::time::Instant::now());
    anyhow::ensure!(
        !left.is_zero(),
        "Loopflow server did not answer within {}s",
        within.as_secs()
    );
    connection.set_read_timeout(Some(left))?;
    connection
        .request(operation)
        .map_err(|error| stalled(error, "answer", within))
}

/// A response or error for `id`, or an error for the whole connection.
fn answers(message: &ServerMessage, id: &str) -> bool {
    match message {
        ServerMessage::Response { id: of, .. } => of == id,
        ServerMessage::Error { id: of, .. } => of.as_deref().is_none_or(|of| of == id),
        _ => false,
    }
}

/// The message, unless the server refused: then its [`Error`].
fn accepted(message: ServerMessage) -> Result<ServerMessage> {
    match message {
        ServerMessage::Error {
            code,
            message,
            data,
            ..
        } => Err(Error {
            code,
            message,
            data,
        }
        .into()),
        other => Ok(other),
    }
}

/// Two connections: a client asks `session.attach` for a ticket, and a fresh
/// one presents it. After `bridged`, that stream speaks the viewer protocol
/// with the Session's held terminal; until then it reads one byte at a time,
/// so no terminal byte is left in a buffer.
pub fn attach(path: &Path, session: &LfSessionId) -> Result<UnixStream> {
    let mut connection = Connection::start(connect(path)?, path, identity("cli", "lf"), HANDSHAKE)?;
    let issued = connection
        .request(Operation::Server(ServerOperation::SessionAttach {
            session: session.clone(),
            terminal_id: None,
        }))
        .map_err(|error| stalled(error, "issue the attach ticket", HANDSHAKE))?;
    let issued: SessionAttached = serde_json::from_value(issued.result)
        .context("Loopflow server answered session.attach without a ticket")?;
    drop(connection);
    let stream = connect(path)?;
    stream.set_read_timeout(Some(HANDSHAKE))?;
    let mut socket = Socket::with_capacity(stream, 1)?;
    socket.send(&ClientMessage::Attach {
        ticket: issued.ticket,
    })?;
    match socket
        .receive()
        .map_err(|error| stalled(error, "open the terminal bridge", HANDSHAKE))?
    {
        Some(ServerMessage::Bridged { .. }) => {}
        Some(ServerMessage::Error {
            code,
            message,
            data,
            ..
        }) => {
            return Err(Error {
                code,
                message,
                data,
            }
            .into())
        }
        Some(other) => anyhow::bail!("unexpected server message: {other:?}"),
        None => anyhow::bail!("Loopflow server closed the connection before bridging"),
    }
    let stream = socket.reader.into_inner();
    stream.set_read_timeout(None)?;
    Ok(stream)
}

/// A wait that `within` cut short, named for the answer it waited for.
fn stalled(error: anyhow::Error, answer: &str, within: Duration) -> anyhow::Error {
    match error
        .root_cause()
        .downcast_ref::<std::io::Error>()
        .map(std::io::Error::kind)
    {
        Some(std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => anyhow::anyhow!(
            "Loopflow server did not {answer} within {}s; check `lf server status`",
            within.as_secs()
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
    fn new(stream: UnixStream) -> Result<Self> {
        Self::with_capacity(stream, 64 * 1024)
    }

    fn with_capacity(stream: UnixStream, capacity: usize) -> Result<Self> {
        Ok(Self {
            writer: stream.try_clone()?,
            reader: BufReader::with_capacity(capacity, stream),
        })
    }

    fn send(&mut self, message: &ClientMessage) -> Result<()> {
        let line = Self::line(message)?;
        anyhow::ensure!(
            line.len() <= MAX_MESSAGE,
            "Server request exceeds {MAX_MESSAGE} bytes"
        );
        self.write(&line)
    }

    fn line(message: &ClientMessage) -> Result<String> {
        let mut line = serde_json::to_string(message)?;
        line.push('\n');
        Ok(line)
    }

    fn write(&mut self, line: &str) -> Result<()> {
        Ok(self.writer.write_all(line.as_bytes())?)
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

fn redeem(path: &Path, ticket: &str) -> Result<UnixStream> {
    let mut stream = connect(path)?;
    stream.set_read_timeout(Some(HANDSHAKE))?;
    stream.write_all(
        Socket::line(&ClientMessage::Attach {
            ticket: ticket.into(),
        })?
        .as_bytes(),
    )?;
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while !line.ends_with(b"\n") {
        anyhow::ensure!(
            line.len() <= MAX_MESSAGE,
            "Server answer to a ticket exceeds {MAX_MESSAGE} bytes"
        );
        stream
            .read_exact(&mut byte)
            .map_err(|error| stalled(error.into(), "admit the ticket", HANDSHAKE))?;
        line.push(byte[0]);
    }
    match accepted(serde_json::from_slice(&line)?)? {
        ServerMessage::Admitted => {}
        other => anyhow::bail!("Server did not admit the ticket: {other:?}"),
    }
    stream.set_read_timeout(None)?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::time::Instant;

    use super::{attach, hook_request, Connection, HANDSHAKE};
    use crate::id::LfSessionId;
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
                    limits: Limits {
                        max_message: MAX_MESSAGE,
                        max_connections: MAX_CONNECTIONS,
                        max_open: super::super::outbox::MAX_OPEN,
                        max_retained_bytes: super::super::outbox::MAX_RETAINED,
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
    fn attach_gives_up_on_a_server_that_never_issues_a_ticket() {
        let (_home, path) = silent_server(true);
        let started = Instant::now();
        let error = attach(&path, &LfSessionId::from("session")).expect_err("attach stalled");
        within_bound(started);
        assert_eq!(
            error.to_string(),
            "Loopflow server did not issue the attach ticket within 10s; check `lf server status`"
        );
    }

    fn boundary() -> crate::server::wire::Operation {
        crate::server::wire::Operation::Server(
            crate::server::wire::ServerOperation::SessionBoundary(
                crate::server::wire::SessionBoundary {
                    moment: crate::server::wire::BoundaryMoment::Tool,
                    delivery: "/nonexistent/delivery.json".into(),
                    adapter: None,
                    callback: serde_json::json!({}),
                    gap: None,
                    printed: Vec::new(),
                    inbox: None,
                },
            ),
        )
    }

    #[test]
    fn a_hook_gives_up_on_a_slow_server_within_its_one_deadline() {
        let within = std::time::Duration::from_millis(300);
        for welcomes in [false, true] {
            let (_home, path) = silent_server(welcomes);
            let started = Instant::now();
            let error = hook_request(&path, boundary(), within).expect_err("server stalled");
            let elapsed = started.elapsed();
            assert!(elapsed >= within, "gave up early after {elapsed:?}");
            assert!(elapsed < within * 3, "waited {elapsed:?}");
            assert!(error.to_string().contains("did not"), "{error:#}");
        }
    }

    #[test]
    fn a_hook_with_no_server_fails_at_once() {
        let home = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let error = hook_request(
            &home.path().join("machine.sock"),
            boundary(),
            std::time::Duration::from_secs(2),
        )
        .expect_err("no server");
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        assert!(error.to_string().contains("Loopflow server unavailable"));
    }

    #[test]
    fn a_welcomed_session_reads_without_a_bound() {
        let (_home, path) = silent_server(true);
        let connection = Connection::open_at(&path).unwrap();
        assert_eq!(connection.socket.writer.read_timeout().unwrap(), None);
    }
}
