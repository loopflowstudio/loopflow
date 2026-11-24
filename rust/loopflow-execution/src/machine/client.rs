//! Blocking client for the server protocol: the readiness probe, one
//! connection with requests by id, and the ticketed connections that carry a
//! terminal's bytes or one message over the control limit. A connection
//! reaches this Machine's server on its socket and another Machine's over SSH.
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::wire::{
    ClientIdentity, ConnectedClient, Hello, Sender, ServerMessage, ServerOperation,
    SessionAttached, Welcome, MAX_BULK, MAX_MESSAGE, MIN_REVISION, REVISION,
};
use crate::engine::{Error, ErrorCode};
use loopflow_model::durable::Machine;
use loopflow_model::id::LfSessionId;

type ClientMessage = super::wire::ClientMessage;

/// How long the server has to answer `status`, `hello`, `session.attach` or
/// a ticket. A session's later waits are unbounded unless the caller sets its
/// own limit.
pub const HANDSHAKE: Duration = Duration::from_secs(10);
/// How long another Machine's server has to answer a request over SSH. A
/// subscription's quiet stream is not bounded.
pub const ANSWER: Duration = Duration::from_secs(120);

/// `lf server status`: readiness observed without opening Work.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Status {
    pub socket: PathBuf,
    pub ready: bool,
    /// A server accepted the connection, whatever it answered.
    pub listening: bool,
    pub instance: Option<String>,
    /// The newest protocol revision the server speaks.
    pub revision: Option<u32>,
    /// The server's own executable; `lf install server` replaces a server
    /// whose executable is not the selected installation's.
    pub executable_sha256: Option<String>,
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
        executable_sha256: None,
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
    let mut socket = Socket::local(stream, 64 * 1024)?;
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
            status.executable_sha256 = Some(answer.executable_sha256);
            (answer.min_revision > REVISION || answer.revision < MIN_REVISION).then(|| {
                format!(
                    "Loopflow server speaks protocol revisions {}–{}; this build speaks {MIN_REVISION}–{REVISION}; update the selected installation",
                    answer.min_revision, answer.revision
                )
            })
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
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
            )
        })
}

/// A revision-1 server reads `{"ty` as an absurd frame length and closes.
const OLDER_PROTOCOL: &str = "Loopflow server closed the connection without answering; it may speak an older protocol; update the selected installation";

/// What a request returned, and the trace that correlates its diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub trace_id: loopflow_model::id::TraceId,
    pub result: serde_json::Value,
}

/// A peer's answer can be a refusal on a healthy connection. Losing the
/// connection before an answer is a separate decision for a caller that retries.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error(transparent)]
    Refused(Error),
    #[error(transparent)]
    Disconnected(anyhow::Error),
    #[error(transparent)]
    Failed(anyhow::Error),
}

impl RequestError {
    pub fn into_error(self) -> anyhow::Error {
        match self {
            Self::Refused(error) => error.into(),
            Self::Disconnected(error) | Self::Failed(error) => error,
        }
    }
}

impl From<anyhow::Error> for RequestError {
    fn from(error: anyhow::Error) -> Self {
        if resets(&error) {
            Self::Disconnected(error)
        } else {
            Self::Failed(error)
        }
    }
}

/// What carries a connection's bytes to a server.
#[derive(Debug, Clone)]
enum Carrier {
    /// This Machine's server, at its socket.
    Socket(PathBuf),
    /// Another Machine's server through its SSH entry.
    Ssh(Machine),
}

impl Carrier {
    /// A fresh connection that reads at most `capacity` bytes ahead.
    fn open(&self, capacity: usize) -> Result<Socket> {
        match self {
            Self::Socket(path) => Socket::local(connect(path)?, capacity),
            Self::Ssh(machine) => {
                let (pipe, output, input) = super::ssh::Pipe::open(machine)?;
                let output = Timed::new(output);
                Ok(Socket {
                    end: End::Pipe(pipe, output.timeout.clone()),
                    reader: BufReader::with_capacity(capacity, Box::new(output)),
                    writer: Box::new(input),
                })
            }
        }
    }
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
    carrier: Carrier,
    /// CLI connections may use the same sender for their lifetime.
    sender: Option<Sender>,
    /// When the caller stops waiting for another Machine's answers.
    until: Option<Instant>,
}

impl Connection {
    /// A ticket for a running Session, addressed by Task or Machine when
    /// its record lives on a peer.
    pub fn session_ticket(
        &mut self,
        session: &LfSessionId,
        terminal_id: Option<loopflow_model::id::TerminalId>,
        task: Option<&str>,
        machine: Option<&loopflow_model::durable::MachineId>,
    ) -> Result<SessionAttached> {
        let answer = self.query(ServerOperation::SessionAttach {
            session: session.clone(),
            terminal_id,
            task: task.map(str::to_owned),
            machine: machine.cloned(),
        })?;
        serde_json::from_value(answer.result)
            .context("Loopflow server answered session.attach without a ticket")
    }

    /// The selected Machine's server.
    pub fn open() -> Result<Self> {
        Self::open_at(&super::socket_path()?)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        let carrier = Carrier::Socket(path.to_path_buf());
        let connection = Self::start(carrier, identity("cli", "lf"), HANDSHAKE)?;
        connection.set_read_timeout(None)?;
        Ok(connection)
    }

    /// `machine`'s server, over SSH. Its `lf server connect` starts the
    /// server when none is running. Every operation sent here runs there,
    /// with what `sender` says this `lf` runs with.
    ///
    /// With `until`, the welcome and every answer have what is left of it,
    /// and [`ANSWER`] at most.
    pub fn open_on(machine: &Machine, sender: Sender, until: Option<Instant>) -> Result<Self> {
        let mut connection = Self::open_peer(machine, until)?;
        connection.sender = Some(sender);
        Ok(connection)
    }

    /// An SSH connection shared by requests that each name their sender.
    pub fn open_peer(machine: &Machine, until: Option<Instant>) -> Result<Self> {
        let mut connection = Self::start(
            Carrier::Ssh(machine.clone()),
            identity(super::wire::MACHINE_CLIENT, "lf"),
            left(until).min(HANDSHAKE),
        )
        .map_err(|error| unavailable_on(machine, error))?;
        connection.until = until;
        connection.set_read_timeout(None)?;
        Ok(connection)
    }

    /// Hello and welcome within `within`, which stays set until the caller
    /// has what it opened the connection for.
    fn start(carrier: Carrier, client: ClientIdentity, within: Duration) -> Result<Self> {
        let mut socket = carrier.open(64 * 1024)?;
        socket.set_read_timeout(Some(within))?;
        let hello = socket.send(&ClientMessage::Hello(Hello {
            revision: REVISION,
            min_revision: MIN_REVISION,
            client,
        }));
        // A destination that exits before reading, as SSH does when it cannot
        // connect, closes the pipe under the hello: its exit says why.
        if let Err(error) = hello {
            return Err(match resets(&error) {
                true => socket.ended(OLDER_PROTOCOL, ErrorCode::Unavailable),
                false => error,
            });
        }
        let answer = socket.receive().map_err(|error| match &carrier {
            Carrier::Ssh(machine) => unanswered(error, machine, within, ErrorCode::Unavailable),
            Carrier::Socket(_) => stalled(error, "welcome the connection", within),
        })?;
        match answer.map(accepted).transpose()? {
            Some(ServerMessage::Welcome(welcome)) => Ok(Self {
                socket,
                welcome,
                pending: VecDeque::new(),
                carrier,
                sender: None,
                until: None,
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
            None => Err(socket.ended(OLDER_PROTOCOL, ErrorCode::Unavailable)),
        }
    }

    pub fn welcome(&self) -> &Welcome {
        &self.welcome
    }

    /// How long a wait for the server may last; none by default.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Send a request without waiting; its answer arrives through
    /// [`Self::request`] or [`Self::reading`]. A request over the control limit
    /// travels on a ticketed connection and is answered here like any other.
    pub fn send(&mut self, operation: impl serde::Serialize) -> Result<String> {
        self.send_as(operation, self.sender.clone(), None)
            .map_err(RequestError::into_error)
    }

    fn send_as(
        &mut self,
        operation: impl serde::Serialize,
        sender: Option<Sender>,
        request_id: Option<String>,
    ) -> Result<String, RequestError> {
        let id = uuid::Uuid::new_v4().to_string();
        let line = Socket::line(&ClientMessage::Request {
            id: id.clone(),
            operation: serde_json::to_value(operation).map_err(anyhow::Error::from)?,
            select: None,
            sender: sender.clone(),
            request_id,
        })?;
        if line.len() <= MAX_MESSAGE {
            self.socket.write(&line)?;
        } else {
            self.upload(line, sender)?;
        }
        Ok(id)
    }

    fn upload(&mut self, line: String, sender: Option<Sender>) -> Result<(), RequestError> {
        if line.len() > MAX_BULK {
            return Err(RequestError::Failed(anyhow::anyhow!(
                "Server request exceeds {MAX_BULK} bytes"
            )));
        }
        let ticket = self
            .request_as(
                ServerOperation::RequestUpload { bytes: line.len() },
                sender,
                None,
                ErrorCode::Unavailable,
            )?
            .result;
        let ticket = ticket["ticket"]
            .as_str()
            .context("Server did not issue an upload ticket")?;
        let mut socket = redeem(&self.carrier, ticket)?;
        socket.write(&line)?;
        if matches!(self.carrier, Carrier::Ssh(_)) {
            // The SSH child may still be forwarding the final bytes in its
            // stdin pipe. The peer closes the ticket once it has the request.
            if let Some(message) = socket.receive()? {
                accepted(message)?;
                return Err(RequestError::Failed(anyhow::anyhow!(
                    "Unexpected reply on upload connection"
                )));
            }
        }
        Ok(())
    }

    /// Read one bulk message using a ticket from the named Machine.
    pub fn fetch_peer(machine: Machine, ticket: &str) -> Result<ServerMessage> {
        Self::fetch_from(&Carrier::Ssh(machine), ticket)
    }

    fn fetch_from(carrier: &Carrier, ticket: &str) -> Result<ServerMessage> {
        let mut socket = redeem(carrier, ticket)?;
        let mut line = Vec::new();
        (&mut socket.reader)
            .take(MAX_BULK as u64 + 1)
            .read_until(b'\n', &mut line)?;
        anyhow::ensure!(
            line.len() <= MAX_BULK && line.ends_with(b"\n"),
            "Loopflow server did not deliver the bulk message"
        );
        Ok(serde_json::from_slice(&line)?)
    }

    /// One request and its answer; other ids' messages wait in `pending`.
    pub fn request(&mut self, operation: impl serde::Serialize) -> Result<Response> {
        self.request_as(
            operation,
            self.sender.clone(),
            None,
            ErrorCode::OutcomeUnknown,
        )
        .map_err(RequestError::into_error)
    }

    /// A read has no effect to recover if the Machine loses the connection.
    pub fn query(&mut self, operation: impl serde::Serialize) -> Result<Response> {
        self.request_as(operation, self.sender.clone(), None, ErrorCode::Unavailable)
            .map_err(RequestError::into_error)
    }

    /// Send one operation in the sender's repository. A request ID names a
    /// repeatable write. `disconnected` is the outcome if no reply arrives.
    pub fn request_as(
        &mut self,
        operation: impl serde::Serialize,
        sender: Option<Sender>,
        request_id: Option<String>,
        disconnected: ErrorCode,
    ) -> Result<Response, RequestError> {
        let sent = self.send_as(operation, sender, request_id);
        let within = left(self.until);
        if matches!(self.carrier, Carrier::Ssh(_)) {
            self.set_read_timeout(Some(within))?;
        }
        let exchange = sent.and_then(|id| self.wait(disconnected, |message| answers(message, &id)));
        let answer = match &self.carrier {
            Carrier::Socket(_) => exchange?,
            Carrier::Ssh(machine) => {
                self.set_read_timeout(None)?;
                let name_failure = |error| match disconnected {
                    ErrorCode::Unavailable => unavailable_on(machine, error),
                    code => unanswered(error, machine, within, code),
                };
                exchange.map_err(|failure| match failure {
                    RequestError::Disconnected(error) => {
                        let error = if disconnected == ErrorCode::OutcomeUnknown {
                            Error::refused(&error, ErrorCode::OutcomeUnknown).into()
                        } else {
                            name_failure(error)
                        };
                        RequestError::Disconnected(error)
                    }
                    RequestError::Failed(error) => RequestError::Failed(name_failure(error)),
                    refusal => refusal,
                })?
            }
        };
        match answer {
            ServerMessage::Response {
                trace_id, result, ..
            } => Ok(Response { trace_id, result }),
            ServerMessage::Error {
                id,
                code,
                message,
                data,
                ..
            } => {
                let error = Error {
                    code,
                    message,
                    data,
                };
                Err(if id.is_some() {
                    RequestError::Refused(error)
                } else {
                    RequestError::Failed(error.into())
                })
            }
            other => Err(RequestError::Failed(anyhow::anyhow!(
                "unexpected server message: {other:?}"
            ))),
        }
    }

    /// Start a subscription keeping only `select` of each reading, and return
    /// its id once the server confirms it.
    pub fn subscribe(
        &mut self,
        operation: impl serde::Serialize,
        select: Option<Vec<String>>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.socket.send(&ClientMessage::Subscribe {
            id: id.clone(),
            operation: serde_json::to_value(operation)?,
            select,
            resume: None,
            sender: self.sender.clone(),
        })?;
        match self
            .wait(ErrorCode::Unavailable, |message| answers(message, &id))
            .map_err(RequestError::into_error)?
        {
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
            let message = self
                .wait(ErrorCode::Unavailable, |message| {
                    matches!(message,
                        ServerMessage::Event { id: of, .. }
                        | ServerMessage::Reading { id: of, .. }
                        | ServerMessage::Ended { id: of } if of == id
                    ) || answers(message, id)
                })
                .map_err(RequestError::into_error)?;
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

    /// The first matching frame, including a refusal from the peer.
    fn wait(
        &mut self,
        disconnected: ErrorCode,
        mut matches: impl FnMut(&ServerMessage) -> bool,
    ) -> Result<ServerMessage, RequestError> {
        if let Some(index) = self.pending.iter().position(&mut matches) {
            return Ok(self
                .pending
                .remove(index)
                .expect("position names a pending message"));
        }
        loop {
            let Some(mut message) = self.socket.receive()? else {
                return Err(RequestError::Disconnected(self.socket.ended(
                    "Loopflow server closed the connection before answering",
                    disconnected,
                )));
            };
            if let ServerMessage::Bulk { ticket, .. } = &message {
                message = Self::fetch_from(&self.carrier, ticket)?;
            }
            if matches(&message) {
                return Ok(message);
            }
            self.pending.push_back(message);
        }
    }
}

pub fn identity(kind: &str, name: &str) -> ClientIdentity {
    ClientIdentity {
        kind: kind.into(),
        platform: std::env::consts::OS.into(),
        name: name.into(),
        version: crate::build_identity::version().into(),
    }
}

/// How long a hook waits for the server before its turn goes on without it.
pub const HOOK_DEADLINE: Duration = Duration::from_secs(2);

/// A provider hook's one request: connecting, `hello` and the answer share
/// `within`, so a stopped, restarting or slow server costs the turn at most that.
pub fn hook_request(
    path: &Path,
    operation: impl serde::Serialize,
    within: Duration,
) -> Result<Response> {
    let deadline = std::time::Instant::now() + within;
    let client = identity("hook", "lf __context-block");
    let mut connection = Connection::start(Carrier::Socket(path.to_path_buf()), client, within)?;
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
    let carrier = Carrier::Socket(path.to_path_buf());
    let mut connection = Connection::start(carrier, identity("cli", "lf"), HANDSHAKE)?;
    let issued = connection
        .session_ticket(session, None, None, None)
        .map_err(|error| stalled(error, "issue the attach ticket", HANDSHAKE))?;
    redeem_terminal(path, issued)
}

/// Open a single-use terminal ticket at the server that issued it.
pub fn redeem_terminal(path: &Path, issued: SessionAttached) -> Result<UnixStream> {
    let carrier = Carrier::Socket(path.to_path_buf());
    let mut socket = carrier.open(1)?;
    socket.set_read_timeout(Some(HANDSHAKE))?;
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
    // Reading one byte at a time left nothing of the terminal's in a buffer.
    let End::Socket(stream) = socket.end else {
        unreachable!("a local carrier ends in a socket")
    };
    stream.set_read_timeout(None)?;
    Ok(stream)
}

/// How long another Machine's server has to answer for a caller that stops
/// waiting at `until`. Never zero, which a read takes as no limit.
fn left(until: Option<Instant>) -> Duration {
    until.map_or(ANSWER, |until| {
        until
            .saturating_duration_since(Instant::now())
            .clamp(Duration::from_millis(1), ANSWER)
    })
}

/// Name the unreachable Machine while retaining a peer's typed refusal.
pub fn unavailable_on(machine: &Machine, error: anyhow::Error) -> anyhow::Error {
    let refusal = Error::refused(&error, ErrorCode::Unavailable);
    if refusal.code != ErrorCode::Unavailable || refusal.data.is_some() {
        return error;
    }
    // A message that already names an `ssh` check (SSH's own failure,
    // `unanswered`, an exited `lf` there) names the Machine too.
    let message = if refusal.message.contains("`ssh ") {
        refusal.message
    } else {
        format!(
            "{} is unavailable: {}; check `ssh {} lf server status`",
            machine.name(),
            refusal.message,
            machine.route,
        )
    };
    Error::new(ErrorCode::Unavailable, message)
        .with(serde_json::json!({
            "resource": "machine",
            "machine_id": machine.id,
            "machine_label": machine.label,
            "machine_name": machine.name(),
        }))
        .into()
}

fn unanswered(
    error: anyhow::Error,
    machine: &Machine,
    within: Duration,
    code: ErrorCode,
) -> anyhow::Error {
    if !timed_out(&error) {
        return error;
    }
    let message = anyhow::anyhow!(
        "{} did not answer within {}s; check `ssh {} lf server status`",
        machine.name(),
        within.as_secs_f64().ceil(),
        machine.route
    );
    match code {
        ErrorCode::OutcomeUnknown => Error::outcome_unknown(&message),
        code => Error::new(code, message.to_string()),
    }
    .into()
}

fn timed_out(error: &anyhow::Error) -> bool {
    matches!(
        error
            .root_cause()
            .downcast_ref::<std::io::Error>()
            .map(std::io::Error::kind),
        Some(std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)
    )
}

/// A wait that `within` cut short, named for the answer it waited for.
fn stalled(error: anyhow::Error, answer: &str, within: Duration) -> anyhow::Error {
    if !timed_out(&error) {
        return error;
    }
    anyhow::anyhow!(
        "Loopflow server did not {answer} within {}s; check `lf server status`",
        within.as_secs()
    )
}

fn connect(path: &Path) -> Result<UnixStream> {
    let stream = UnixStream::connect(path).context(
        "Loopflow server unavailable; check `lf server status` and the selected installation",
    )?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

/// Newline-delimited JSON messages over one carrier's byte streams.
struct Socket {
    reader: BufReader<Box<dyn Read + Send>>,
    writer: Box<dyn Write + Send>,
    end: End,
}

/// The far end of a [`Socket`]'s byte streams.
#[derive(Debug)]
enum End {
    /// This Machine's socket, which bounds reads.
    Socket(UnixStream),
    /// SSH to another Machine, which says why it closed, and the bound on a
    /// read from it.
    Pipe(super::ssh::Pipe, Timeout),
}

type Timeout = std::sync::Arc<std::sync::Mutex<Option<Duration>>>;

/// A pipe's reads, bounded like a socket's: a thread reads, and a read that
/// waits past the timeout fails as `TimedOut`.
struct Timed {
    chunks: std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
    unread: Vec<u8>,
    timeout: Timeout,
}

impl Timed {
    fn new(mut source: impl Read + Send + 'static) -> Self {
        let (sent, chunks) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut buffer = vec![0; 64 * 1024];
            loop {
                let chunk = match source.read(&mut buffer) {
                    Ok(read) => Ok(buffer[..read].to_vec()),
                    Err(error) => Err(error),
                };
                let ended = !matches!(&chunk, Ok(chunk) if !chunk.is_empty());
                if sent.send(chunk).is_err() || ended {
                    return;
                }
            }
        });
        Self {
            chunks,
            unread: Vec::new(),
            timeout: Timeout::default(),
        }
    }
}

impl Read for Timed {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use std::sync::mpsc::RecvTimeoutError;
        if self.unread.is_empty() {
            let timeout = *self.timeout.lock().expect("pipe timeout lock poisoned");
            let chunk = match timeout {
                Some(timeout) => self
                    .chunks
                    .recv_timeout(timeout)
                    .map_err(|error| match error {
                        RecvTimeoutError::Timeout => std::io::ErrorKind::TimedOut,
                        RecvTimeoutError::Disconnected => std::io::ErrorKind::UnexpectedEof,
                    }),
                None => self
                    .chunks
                    .recv()
                    .map_err(|_| std::io::ErrorKind::UnexpectedEof),
            };
            match chunk {
                Ok(chunk) => self.unread = chunk?,
                // The reader thread ended after an end of file it sent.
                Err(std::io::ErrorKind::UnexpectedEof) => return Ok(0),
                Err(kind) => return Err(kind.into()),
            }
        }
        let read = buffer.len().min(self.unread.len());
        buffer[..read].copy_from_slice(&self.unread[..read]);
        self.unread.drain(..read);
        Ok(read)
    }
}

impl std::fmt::Debug for Socket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socket")
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

impl Socket {
    fn local(stream: UnixStream, capacity: usize) -> Result<Self> {
        Ok(Self {
            writer: Box::new(stream.try_clone()?),
            reader: BufReader::with_capacity(capacity, Box::new(stream.try_clone()?)),
            end: End::Socket(stream),
        })
    }

    fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<()> {
        match &self.end {
            End::Socket(stream) => stream.set_read_timeout(timeout)?,
            End::Pipe(_, bound) => *bound.lock().expect("pipe timeout lock poisoned") = timeout,
        }
        Ok(())
    }

    /// Why the server closed before answering: SSH says, the socket cannot.
    fn ended(&mut self, local: &str, code: ErrorCode) -> anyhow::Error {
        match &mut self.end {
            End::Pipe(pipe, _) => pipe.ended(code),
            End::Socket(_) => anyhow::anyhow!("{local}"),
        }
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
        self.writer.write_all(line.as_bytes())?;
        Ok(self.writer.flush()?)
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
        if !line.ends_with(b"\n") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Loopflow server closed the connection mid-message",
            )
            .into());
        }
        Ok(Some(serde_json::from_slice(&line)?))
    }
}

/// An admitted bulk connection. Its reader keeps any prefetched body bytes.
fn redeem(carrier: &Carrier, ticket: &str) -> Result<Socket> {
    let mut socket = carrier.open(64 * 1024)?;
    socket.set_read_timeout(Some(HANDSHAKE))?;
    socket.send(&ClientMessage::Attach {
        ticket: ticket.into(),
    })?;
    let answer = socket
        .receive()
        .map_err(|error| stalled(error, "admit the ticket", HANDSHAKE))?;
    match answer.map(accepted).transpose()? {
        Some(ServerMessage::Admitted) => {}
        Some(other) => anyhow::bail!("Server did not admit the ticket: {other:?}"),
        None => {
            return Err(socket.ended(
                "Loopflow server closed the connection before admitting the ticket",
                ErrorCode::Unavailable,
            ))
        }
    }
    socket.set_read_timeout(Some(ANSWER))?;
    Ok(socket)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::time::Instant;

    use super::{attach, hook_request, Connection, End, HANDSHAKE};
    use crate::machine::wire::{
        Limits, ServerMessage, Welcome, MAX_CONNECTIONS, MAX_MESSAGE, MIN_REVISION, REVISION,
    };
    use loopflow_model::id::LfSessionId;

    #[test]
    fn a_pipe_that_stays_silent_past_its_bound_times_out_and_keeps_what_arrived() {
        use std::io::Read;
        let (reader, mut writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut timed = super::Timed::new(reader);
        *timed.timeout.lock().unwrap() = Some(std::time::Duration::from_millis(50));
        writer.write_all(b"answer").unwrap();
        let mut buffer = [0; 3];
        timed.read_exact(&mut buffer).unwrap();
        assert_eq!(&buffer, b"ans");
        timed.read_exact(&mut buffer).unwrap();
        assert_eq!(&buffer, b"wer");
        let started = Instant::now();
        let error = timed.read(&mut buffer).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(super::timed_out(&error.into()));
        assert!(started.elapsed() < HANDSHAKE);
        drop(writer);
        assert_eq!(timed.read(&mut buffer).unwrap(), 0);
    }

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
                    // A peer advertises its own admission limits.
                    limits: Limits {
                        max_message: MAX_MESSAGE,
                        max_connections: MAX_CONNECTIONS,
                        max_open: 256,
                        max_retained_bytes: 64 * 1024 * 1024,
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

    fn boundary() -> crate::machine::wire::ServerOperation {
        crate::machine::wire::ServerOperation::SessionBoundary(
            crate::machine::wire::SessionBoundary {
                moment: crate::machine::wire::BoundaryMoment::Tool,
                delivery: "/nonexistent/delivery.json".into(),
                adapter: None,
                callback: serde_json::json!({}),
                gap: None,
                printed: Vec::new(),
                inbox: None,
            },
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
        let End::Socket(stream) = &connection.socket.end else {
            panic!("a local connection ends in a socket")
        };
        assert_eq!(stream.read_timeout().unwrap(), None);
    }
}
