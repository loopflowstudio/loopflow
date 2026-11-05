//! One session per connection: hello, welcome, then requests multiplexed by id
//! until the client closes. Admitted work finishes even when its answer is lost.
//! Both carriers dispatch the same operations after their own admission; the
//! remote carrier never connects to the private socket as a proxy Unix user.
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs2::FileExt;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Notify, Semaphore};
use tokio::task::{AbortHandle, JoinSet};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use super::remote::Control;
use super::ticket::{Admission, Purpose, Tickets};
use super::wire::{
    ClientIdentity, ClientMessage, ConnectedClient, ErrorCode, Hello, Limits, Operation,
    ServerError, ServerMessage, ServerOperation, Status, Welcome, MAX_BULK, MAX_CONNECTIONS,
    MAX_MESSAGE, MIN_REVISION, REVISION,
};
use crate::lf::commands::{read::Read, watch::Subscription, write::Write};

/// How long a connection has to send its first message.
pub(super) const FIRST_MESSAGE: Duration = Duration::from_secs(5);
/// How long a client has to take a message before its connection ends.
const SEND: Duration = Duration::from_secs(5);
/// How long a ticketed connection has to carry its one large message.
const BULK: Duration = Duration::from_secs(120);
/// Answers waiting for one connection's socket.
const QUEUE: usize = 64;
/// Checkpoints one subscription may have waiting.
const CHECKPOINTS: usize = 64;

const CAPABILITIES: &[&str] = &[
    "reads",
    "subscriptions",
    "commands",
    "configuration",
    "roadmap",
    "machine_id",
    "portfolio",
    "repository_path",
    "session_rename",
    "session_start",
    "task_create",
    "owner_bridge",
    "session_owner",
    "desktop_sessions",
    "session_page",
    "view_plan",
    "all_view_plans",
];

type Lines = Framed<UnixStream, LinesCodec>;

/// A connection as its carrier delimits it: one protocol message per item.
pub(super) trait Carrier:
    Stream<Item = Result<String, <Self as Carrier>::Failure>>
    + Sink<String, Error = <Self as Carrier>::Failure>
    + Unpin
    + Send
{
    type Failure: Into<anyhow::Error> + Send + 'static;
}

impl Carrier for Lines {
    type Failure = LinesCodecError;
}

/// Welcomed connections by client ID, as `lf server status` lists them.
pub(super) type Clients = Arc<Mutex<HashMap<String, ConnectedClient>>>;

/// A welcomed connection's row, removed when the connection ends.
pub(super) struct Admitted {
    clients: Clients,
    id: String,
}

impl Admitted {
    fn new(clients: &Clients, client: ClientIdentity) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let connected_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("the current time formats as RFC 3339");
        lock(clients).insert(
            id.clone(),
            ConnectedClient {
                id: id.clone(),
                client,
                connected_at,
                requests: 0,
            },
        );
        Self {
            clients: clients.clone(),
            id,
        }
    }

    fn requested(&self) {
        if let Some(row) = lock(&self.clients).get_mut(&self.id) {
            row.requests += 1;
        }
    }
}

impl Drop for Admitted {
    fn drop(&mut self) {
        lock(&self.clients).remove(&self.id);
    }
}

fn lock(clients: &Clients) -> std::sync::MutexGuard<'_, HashMap<String, ConnectedClient>> {
    clients.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Oldest connection first.
fn connected(clients: &Clients) -> Vec<ConnectedClient> {
    let mut rows: Vec<_> = lock(clients).values().cloned().collect();
    rows.sort_by(|a, b| (&a.connected_at, &a.id).cmp(&(&b.connected_at, &b.id)));
    rows
}

pub(super) async fn run() -> Result<()> {
    let path = super::socket_path()?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path.with_extension("lock"))?;
    lock.try_lock_exclusive()
        .context("Loopflow server is already running")?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    let executable_sha256 = Arc::new(
        crate::installation::ArtifactIdentity::capture(
            crate::installation::ArtifactRole::Cli,
            &std::env::current_exe()?,
        )?
        .sha256,
    );
    let instance: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    let clients = Clients::default();
    // One ticket store: a terminal or bulk connection on either carrier
    // redeems what a control connection on the same carrier requested.
    let tickets = Arc::new(Tickets::default());
    let (remote_control, remote_requests) = mpsc::channel(8);
    let remote = super::remote::manage(
        super::mobile::configured(),
        remote_requests,
        instance.clone(),
        executable_sha256.clone(),
        slots.clone(),
        clients.clone(),
        tickets.clone(),
    );
    tracing::info!(%instance, socket = %path.display(), "Loopflow server ready");
    tokio::select! {
        result = remote => result,
        result = local(listener, instance, executable_sha256, slots, clients, remote_control, tickets) => result,
    }
}

async fn local(
    listener: UnixListener,
    instance: Arc<str>,
    executable_sha256: Arc<String>,
    slots: Arc<Semaphore>,
    clients: Clients,
    remote_control: mpsc::Sender<Control>,
    tickets: Arc<Tickets>,
) -> Result<()> {
    loop {
        let (socket, _) = listener.accept().await?;
        // Unix admission is deliberately unavailable to remote proxies.
        let credentials = match socket.peer_cred() {
            Ok(credentials) => credentials,
            Err(error) => {
                // A caller can disappear between accept and credential lookup
                // (ENOTCONN on macOS). Only its connection has ended.
                tracing::debug!(%error, "Server caller disconnected before admission");
                continue;
            }
        };
        // SAFETY: geteuid has no preconditions.
        if credentials.uid() != unsafe { libc::geteuid() } {
            continue;
        }
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            tracing::warn!(%instance, "Server connection capacity reached");
            tokio::spawn(refuse_busy(socket));
            continue;
        };
        tracing::debug!(
            active = MAX_CONNECTIONS - slots.available_permits(),
            "Server connection admitted"
        );
        let instance = instance.clone();
        let executable_sha256 = executable_sha256.clone();
        let clients = clients.clone();
        let remote_control = remote_control.clone();
        let tickets = tickets.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = connection(
                socket,
                &instance,
                &executable_sha256,
                &clients,
                Some(remote_control),
                tickets,
            )
            .await
            {
                tracing::warn!(%instance, %error, "Server connection ended");
            }
        });
    }
}

/// Capacity answers before closing, so the client knows nothing ran.
async fn refuse_busy(mut socket: UnixStream) {
    let line = line(
        &ServerError::new(
            ErrorCode::Busy,
            format!("Server has {MAX_CONNECTIONS} connections; retry shortly"),
        )
        .with(serde_json::json!({ "max_connections": MAX_CONNECTIONS }))
        .into_message(None),
    );
    let _ = tokio::time::timeout(SEND, socket.write_all(line.as_bytes())).await;
}

/// Local connections may configure mobile access; `remote_control` is absent
/// on the remote carrier.
pub(super) async fn connection(
    mut socket: UnixStream,
    instance: &str,
    executable_sha256: &str,
    clients: &Clients,
    remote_control: Option<mpsc::Sender<Control>>,
    tickets: Arc<Tickets>,
) -> Result<()> {
    let first = tokio::time::timeout(FIRST_MESSAGE, socket.read_u8())
        .await
        .context("no message within five seconds")??;
    // A revision-1 request begins with its four-byte length; JSON begins with
    // `{`. Delete this branch with revision 1.
    if first < 0x20 {
        return refuse_revision_one(socket, instance).await;
    }
    // A ticketed connection may carry one bulk message; the control limit is
    // enforced per message in `session`.
    let mut lines = Framed::new(socket, LinesCodec::new_with_max_length(MAX_BULK));
    lines.read_buffer_mut().extend_from_slice(&[first]);
    let line = tokio::time::timeout(FIRST_MESSAGE, lines.next())
        .await
        .context("no message within five seconds")?
        .context("closed before its first message")??;
    match welcome(&mut lines, &line, instance, executable_sha256, clients).await? {
        Greeting::Ended => Ok(()),
        Greeting::Session(admitted) => {
            session(
                &mut lines,
                instance.into(),
                &admitted,
                remote_control,
                Admission::Local,
                tickets,
            )
            .await
        }
        Greeting::Ticket(ticket) => {
            match ticketed(&mut lines, &ticket, &Admission::Local, &tickets).await? {
                None => Ok(()),
                Some(destination) => bridge(lines, destination).await,
            }
        }
    }
}

/// What a connection's first message made it.
pub(super) enum Greeting {
    /// `welcome` was sent; the session follows for this listed client.
    Session(Admitted),
    /// A ticket to redeem; `admitted` or an error follows.
    Ticket(String),
    /// The connection's last message was sent.
    Ended,
}

/// Answer a connection's first message: `welcome` and the session's client row,
/// a ticket to redeem, or the last message after a `status` probe or a refusal.
pub(super) async fn welcome<C: Carrier>(
    carrier: &mut C,
    line: &str,
    instance: &str,
    executable_sha256: &str,
    clients: &Clients,
) -> Result<Greeting> {
    let hello = match serde_json::from_str::<ClientMessage>(line) {
        Ok(ClientMessage::Hello(hello)) => hello,
        Ok(ClientMessage::Ticket { ticket }) => return Ok(Greeting::Ticket(ticket)),
        Ok(ClientMessage::Status) => {
            send(
                carrier,
                &ServerMessage::Status(Status {
                    revision: REVISION,
                    min_revision: MIN_REVISION,
                    instance: instance.into(),
                    executable_sha256: executable_sha256.into(),
                    clients: connected(clients),
                }),
            )
            .await?;
            return Ok(Greeting::Ended);
        }
        Ok(_) => {
            send(
                carrier,
                &error(None, ErrorCode::Invalid, "the first message must be hello"),
            )
            .await?;
            return Ok(Greeting::Ended);
        }
        Err(parse) => {
            send(carrier, &error(None, ErrorCode::Invalid, parse.to_string())).await?;
            return Ok(Greeting::Ended);
        }
    };
    let Some(revision) = negotiate(&hello) else {
        let side = if hello.min_revision > REVISION {
            "selected installation"
        } else {
            "client"
        };
        send(
            carrier,
            &ServerError::new(
                ErrorCode::Incompatible,
                format!(
                    "Server speaks protocol revisions {MIN_REVISION}–{REVISION}; the client speaks {}–{}; update the {side}",
                    hello.min_revision, hello.revision
                ),
            )
            .with(serde_json::json!({
                "server": { "revision": REVISION, "min_revision": MIN_REVISION },
                "update": if hello.min_revision > REVISION { "server" } else { "client" },
            }))
            .into_message(None),
        )
        .await?;
        return Ok(Greeting::Ended);
    };
    let admitted = Admitted::new(clients, hello.client);
    tracing::debug!(%instance, client = %admitted.id, revision, "Server connection welcomed");
    // A Machine registry that is unavailable or uninitialized leaves `machine`
    // null; file settings stay reachable through the same connection.
    let machine =
        tokio::task::spawn_blocking(|| crate::journal::request::isolate(|| Read::MachineId.read()))
            .await?
            .ok();
    send(
        carrier,
        &ServerMessage::Welcome(Welcome {
            client_id: admitted.id.clone(),
            revision,
            server_revision: REVISION,
            min_revision: MIN_REVISION,
            instance: instance.into(),
            build: crate::build_info::BUILD_VERSION.to_owned(),
            executable_sha256: executable_sha256.into(),
            machine,
            capabilities: CAPABILITIES.iter().map(|name| (*name).to_owned()).collect(),
            limits: Limits {
                max_message: MAX_MESSAGE,
                max_connections: MAX_CONNECTIONS,
            },
        }),
    )
    .await?;
    Ok(Greeting::Session(admitted))
}

/// The newest revision both sides speak.
fn negotiate(hello: &Hello) -> Option<u32> {
    let revision = REVISION.min(hello.revision);
    (revision >= MIN_REVISION && revision >= hello.min_revision).then_some(revision)
}

/// A revision-1 client sent a four-byte length. Answer in its framing, with the
/// incarnation its readiness check reports, so installation replaces it.
/// Delete with revision 1.
async fn refuse_revision_one(mut socket: UnixStream, instance: &str) -> Result<()> {
    let body = serde_json::to_vec(&serde_json::json!({
        "id": "",
        "instance": instance,
        "result": {
            "kind": "error",
            "code": "incompatible",
            "message": format!("Server protocol revision {REVISION} required; update the client or service"),
        },
    }))?;
    tokio::time::timeout(SEND, async {
        socket.write_u32(body.len() as u32).await?;
        socket.write_all(&body).await
    })
    .await??;
    Ok(())
}

/// A request's task and, for a subscription, where its checkpoints go.
struct Inflight {
    task: AbortHandle,
    checkpoints: Option<mpsc::Sender<u64>>,
}

/// What a request or subscription asks the session to start.
enum Start {
    Request(Operation),
    Subscribe(Read),
}

/// Requests until the client closes. Terminal bytes and messages over the
/// control limit never travel here: `terminal.attach` and `request.upload`
/// answer with tickets for connections of their own.
pub(super) async fn session<C: Carrier>(
    lines: &mut C,
    instance: Arc<str>,
    admitted: &Admitted,
    remote_control: Option<mpsc::Sender<Control>>,
    admission: Admission,
    tickets: Arc<Tickets>,
) -> Result<()> {
    let (outgoing, mut queue) = mpsc::channel::<ServerMessage>(QUEUE);
    // Requests that arrived on a ticketed connection join this session's.
    let (uploads, mut uploaded) = mpsc::channel::<ClientMessage>(QUEUE);
    // Dropping the set aborts every answer still pending; admitted blocking
    // work finishes on its worker. A task returns its request ID.
    let mut tasks: JoinSet<String> = JoinSet::new();
    let mut inflight: HashMap<String, Inflight> = HashMap::new();
    loop {
        let message = tokio::select! {
            incoming = lines.next() => {
                let Some(line) = incoming else { return Ok(()) };
                let line = line.map_err(Into::into)?;
                if line.len() > MAX_MESSAGE {
                    // A null-id error is the connection's last message.
                    send(lines, &ServerError::new(ErrorCode::Invalid, format!(
                        "message exceeds {MAX_MESSAGE} bytes; request an upload ticket for it"
                    )).with(serde_json::json!({ "max_message": MAX_MESSAGE })).into_message(None)).await?;
                    return Ok(());
                }
                match serde_json::from_str::<ClientMessage>(&line) {
                    Ok(message) => message,
                    Err(parse) => {
                        send(lines, &error(None, ErrorCode::Invalid, parse.to_string())).await?;
                        return Ok(());
                    }
                }
            }
            Some(message) = uploaded.recv() => message,
            Some(message) = queue.recv() => {
                deliver_or_spill(lines, message, &admission, &tickets).await?;
                continue;
            }
            Some(finished) = tasks.join_next_with_id() => {
                match finished {
                    // A cancelled ID may already name a newer request.
                    Ok((task, id)) => {
                        if inflight.get(&id).is_some_and(|entry| entry.task.id() == task) {
                            inflight.remove(&id);
                        }
                    }
                    Err(join) if join.is_cancelled() => {}
                    Err(join) => tracing::warn!(%instance, %join, "Server request task failed"),
                }
                continue;
            }
        };
        let (id, start) = match message {
            ClientMessage::Request { id, operation } => (id, Start::Request(operation)),
            ClientMessage::Subscribe { id, operation } => (id, Start::Subscribe(operation)),
            ClientMessage::Checkpoint { id, checkpoint } => {
                match inflight
                    .get(&id)
                    .and_then(|entry| entry.checkpoints.as_ref())
                {
                    Some(checkpoints) => {
                        if checkpoints.try_send(checkpoint).is_err() {
                            // Ending the subscription tells the client to reopen it
                            // for a complete reading rather than drop a checkpoint.
                            if let Some(entry) = inflight.remove(&id) {
                                entry.task.abort();
                            }
                            send(
                                lines,
                                &error(
                                    Some(id),
                                    ErrorCode::Busy,
                                    "subscription fell behind its checkpoints; subscribe again",
                                ),
                            )
                            .await?;
                        }
                    }
                    None => {
                        send(
                            lines,
                            &ServerError::new(
                                ErrorCode::NotFound,
                                "no live subscription has this ID",
                            )
                            .with(serde_json::json!({ "id": id }))
                            .into_message(Some(id)),
                        )
                        .await?
                    }
                }
                continue;
            }
            ClientMessage::Cancel { id } => {
                // An answered request has nothing left to forget.
                if let Some(entry) = inflight.remove(&id) {
                    entry.task.abort();
                }
                continue;
            }
            ClientMessage::Hello(_) | ClientMessage::Status => {
                send(
                    lines,
                    &error(None, ErrorCode::Invalid, "hello was already exchanged"),
                )
                .await?;
                return Ok(());
            }
            ClientMessage::Ticket { .. } => {
                send(
                    lines,
                    &error(
                        None,
                        ErrorCode::Invalid,
                        "a ticket is a connection's first message",
                    ),
                )
                .await?;
                return Ok(());
            }
        };
        admitted.requested();
        if id.is_empty() || id.len() > 128 {
            send(
                lines,
                &error(
                    Some(id),
                    ErrorCode::Invalid,
                    "request ID must contain 1–128 bytes",
                ),
            )
            .await?;
            continue;
        }
        if inflight.contains_key(&id) {
            send(
                lines,
                &ServerError::new(ErrorCode::Conflict, "request ID is still unanswered")
                    .with(serde_json::json!({ "id": id }))
                    .into_message(Some(id)),
            )
            .await?;
            continue;
        }
        let entry = match start {
            Start::Request(Operation::Server(ServerOperation::Attach { owner })) => {
                let ticket = tickets.issue(admission.clone(), Purpose::Terminal(owner));
                send(lines, &ticket_response(id, ticket)).await?;
                continue;
            }
            Start::Request(Operation::Server(ServerOperation::Upload { bytes })) => {
                if bytes > MAX_BULK {
                    send(
                        lines,
                        &ServerError::new(
                            ErrorCode::Invalid,
                            format!("an uploaded request carries at most {MAX_BULK} bytes"),
                        )
                        .with(serde_json::json!({ "max_bulk": MAX_BULK }))
                        .into_message(Some(id)),
                    )
                    .await?;
                    continue;
                }
                let ticket = tickets.issue(admission.clone(), Purpose::Upload(uploads.clone()));
                send(lines, &ticket_response(id, ticket)).await?;
                continue;
            }
            Start::Subscribe(query) => {
                let (checkpoints, inbox) = mpsc::channel(CHECKPOINTS);
                let task = tasks.spawn(subscription(id.clone(), query, outgoing.clone(), inbox));
                Inflight {
                    task,
                    checkpoints: Some(checkpoints),
                }
            }
            Start::Request(operation) => {
                let task = tasks.spawn(request(
                    id.clone(),
                    operation,
                    instance.clone(),
                    outgoing.clone(),
                    remote_control.clone(),
                ));
                Inflight {
                    task,
                    checkpoints: None,
                }
            }
        };
        inflight.insert(id, entry);
    }
}

/// `{ticket}`: single use, bound to this connection's admission, expiring in
/// [`TICKET_TTL`](super::ticket::TICKET_TTL).
fn ticket_response(id: String, ticket: String) -> ServerMessage {
    ServerMessage::Response {
        id,
        trace_id: crate::id::TraceId::default(),
        result: serde_json::json!({ "ticket": ticket }),
    }
}

/// Redeem a ticketed connection's ticket. A terminal ticket returns the owner
/// to bridge to once `admitted` was sent; the other purposes complete here.
pub(super) async fn ticketed<C: Carrier>(
    carrier: &mut C,
    ticket: &str,
    admission: &Admission,
    tickets: &Tickets,
) -> Result<Option<UnixStream>> {
    let Some(purpose) = tickets.redeem(ticket, admission) else {
        send(
            carrier,
            &ServerError::new(
                ErrorCode::NotFound,
                "ticket is unknown, expired or was issued to another connection",
            )
            .with(serde_json::json!({ "ticket": ticket }))
            .into_message(None),
        )
        .await?;
        return Ok(None);
    };
    match purpose {
        Purpose::Terminal(owner) => {
            let destination = match UnixStream::connect(crate::transport::endpoint(&owner)?).await {
                Ok(destination) => destination,
                Err(failure) => {
                    // A missing owner endpoint means the owner ended.
                    send(carrier, &ServerError::new(ErrorCode::NotFound, format!(
                        "Session owner is not running; no provider restarted and no input replayed: {failure}"
                    )).with(serde_json::json!({ "owner": owner })).into_message(None)).await?;
                    return Ok(None);
                }
            };
            send(carrier, &ServerMessage::Admitted).await?;
            Ok(Some(destination))
        }
        Purpose::Download(message) => {
            send(carrier, &ServerMessage::Admitted).await?;
            tokio::time::timeout(BULK, carrier.send(message))
                .await
                .context("client did not take the bulk message in time")?
                .map_err(Into::into)?;
            Ok(None)
        }
        Purpose::Upload(requests) => {
            send(carrier, &ServerMessage::Admitted).await?;
            let line = tokio::time::timeout(BULK, carrier.next())
                .await
                .context("no uploaded request in time")?
                .context("closed before its uploaded request")?
                .map_err(Into::into)?;
            match serde_json::from_str::<ClientMessage>(&line) {
                Ok(message @ ClientMessage::Request { .. }) => {
                    // The issuing session may have ended; its answer then has no reader.
                    let _ = requests.send(message).await;
                }
                Ok(_) => {
                    send(
                        carrier,
                        &error(
                            None,
                            ErrorCode::Invalid,
                            "an upload connection carries one request",
                        ),
                    )
                    .await?;
                }
                Err(parse) => {
                    send(carrier, &error(None, ErrorCode::Invalid, parse.to_string())).await?
                }
            }
            Ok(None)
        }
    }
}

/// Send an answer on the control connection, or a ticket for it when it
/// exceeds the control limit, so other readings never wait behind it.
async fn deliver_or_spill<C: Carrier>(
    carrier: &mut C,
    message: ServerMessage,
    admission: &Admission,
    tickets: &Tickets,
) -> Result<()> {
    let text = line(&message);
    if text.len() <= MAX_MESSAGE {
        return send_line(carrier, text).await;
    }
    let id = match message {
        ServerMessage::Response { id, .. } | ServerMessage::Reading { id, .. } => id,
        other => anyhow::bail!("connection message exceeds {MAX_MESSAGE} bytes: {other:?}"),
    };
    if text.len() > MAX_BULK {
        return send(
            carrier,
            &ServerError::new(
                ErrorCode::Unavailable,
                format!("answer exceeds {MAX_BULK} bytes"),
            )
            .with(serde_json::json!({ "max_bulk": MAX_BULK }))
            .into_message(Some(id)),
        )
        .await;
    }
    let bytes = text.len();
    let ticket = tickets.issue(admission.clone(), Purpose::Download(text));
    send(carrier, &ServerMessage::Bulk { id, ticket, bytes }).await
}

/// Hand the connection to the owner protocol. Tokio uses bounded copy buffers;
/// closing either side releases the owner claim, and no service state can
/// restore it on another connection.
async fn bridge(lines: Lines, mut destination: UnixStream) -> Result<()> {
    let parts = lines.into_parts();
    let mut socket = parts.io;
    // Owner-protocol bytes the client pipelined behind its ticket.
    if !parts.read_buf.is_empty() {
        destination.write_all(&parts.read_buf).await?;
    }
    if let Err(error) = tokio::io::copy_bidirectional(&mut socket, &mut destination).await {
        // Once bridged, a JSON error would corrupt the owner stream: just close.
        tracing::debug!(%error, "Server owner bridge disconnected");
    }
    Ok(())
}

/// The [`ServerError`] a shared operation named anywhere in `error`'s chain,
/// else `otherwise` with the error's text.
fn refused(error: anyhow::Error, otherwise: ErrorCode) -> ServerError {
    match error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ServerError>())
    {
        Some(named) => named.clone(),
        None => ServerError::new(otherwise, error.to_string()),
    }
}

/// A read that failed without naming a code depends on something unavailable.
fn unavailable(error: impl Into<anyhow::Error>) -> ServerError {
    refused(error.into(), ErrorCode::Unavailable)
}

async fn request(
    id: String,
    operation: Operation,
    instance: Arc<str>,
    outgoing: mpsc::Sender<ServerMessage>,
    remote_control: Option<mpsc::Sender<Control>>,
) -> String {
    let started = Instant::now();
    let trace_id = crate::id::TraceId::default();
    let result = operate(operation, &id, &instance, &trace_id, remote_control).await;
    tracing::debug!(request = %id, %instance, %trace_id, elapsed_ms = started.elapsed().as_millis(), ok = result.is_ok(), "Server request ended");
    let message = match result {
        Ok(result) => ServerMessage::Response {
            id: id.clone(),
            trace_id,
            result,
        },
        Err(refusal) => {
            tracing::warn!(request = %id, %instance, %trace_id, code = %refusal.code, error = %refusal.message, "Server request failed");
            refusal.into_message(Some(id.clone()))
        }
    };
    // A closed connection drops the answer; the effect stands.
    let _ = outgoing.send(message).await;
    id
}

async fn operate(
    operation: Operation,
    id: &str,
    instance: &str,
    trace_id: &crate::id::TraceId,
    remote_control: Option<mpsc::Sender<Control>>,
) -> Result<serde_json::Value, ServerError> {
    match operation {
        Operation::Server(ServerOperation::MobileStatus) => {
            let (reply, result) = tokio::sync::oneshot::channel();
            mobile_control(remote_control, Control::Status { reply }).await?;
            serde_json::to_value(result.await.map_err(unavailable)?).map_err(unavailable)
        }
        Operation::Server(ServerOperation::ConfigureRemote { configuration }) => {
            let (reply, result) = tokio::sync::oneshot::channel();
            mobile_control(
                remote_control,
                Control::Configure {
                    configuration,
                    reply,
                },
            )
            .await?;
            let configuration = result
                .await
                .map_err(unavailable)?
                .map_err(|message| ServerError::new(ErrorCode::Unavailable, message))?;
            Ok(serde_json::json!({ "configuration": configuration }))
        }
        Operation::Server(ServerOperation::SessionOwner { session }) => {
            let owner = tokio::task::spawn_blocking(move || -> Result<_> {
                let registry = crate::store::read_existing_registry()?.ok_or_else(|| {
                    ServerError::new(ErrorCode::Unavailable, "Machine registry is unavailable")
                        .with(serde_json::json!({ "resource": "machine_registry" }))
                })?;
                Ok(registry.sqlite.session_transport(session.as_str())?)
            })
            .await
            .map_err(unavailable)?
            .map_err(unavailable)?;
            Ok(serde_json::json!({ "owner": owner }))
        }
        Operation::Read(query) => {
            tokio::task::spawn_blocking(move || crate::journal::request::isolate(|| query.read()))
                .await
                .map_err(unavailable)?
                .map_err(unavailable)
        }
        Operation::Server(ServerOperation::Configuration { request }) => {
            let mut target = serde_json::to_value(&request).map_err(unavailable)?;
            target
                .as_object_mut()
                .expect("configuration request is an object")
                .remove("value");
            effect(target, id, instance, trace_id, move || {
                crate::config::api::run(request)
            })
            .await
        }
        Operation::Write(change) => {
            if let Some(argv) = change.delegated() {
                let recovery = change_recovery(&change);
                return delegate(argv, &change, id, instance, trace_id)
                    .await
                    .map_err(|error| {
                        ServerError::new(
                            ErrorCode::OutcomeUnknown,
                            format!("{error:#}; {recovery}"),
                        )
                    });
            }
            let mut target = serde_json::to_value(&change).map_err(unavailable)?;
            if let Some(fields) = target.as_object_mut() {
                for field in ["content", "notes", "plan", "layout", "slots", "expression"] {
                    fields.remove(field);
                }
            }
            effect(target, id, instance, trace_id, move || change.run()).await
        }
        Operation::Server(ServerOperation::Attach { .. } | ServerOperation::Upload { .. }) => {
            unreachable!("the session answers tickets")
        }
    }
}

/// Mobile setup belongs to the Machine's own user: the private socket only.
async fn mobile_control(
    remote_control: Option<mpsc::Sender<Control>>,
    request: Control,
) -> Result<(), ServerError> {
    let Some(control) = remote_control else {
        return Err(ServerError::new(
            ErrorCode::Invalid,
            "Mobile access is configured on the Machine itself",
        ));
    };
    control
        .send(request)
        .await
        .map_err(|_| ServerError::new(ErrorCode::Unavailable, "mobile listener is not running"))?;
    Ok(())
}

fn change_recovery(change: &Write) -> &'static str {
    if matches!(change, Write::TaskCreate { .. }) {
        "retry with the same creation request ID to recover its result"
    } else {
        "inspect Session state before retrying"
    }
}

/// The whole CLI operation in an `lf` child: Graph launch and Session owner
/// startup stay with their existing owners, never with the server process.
/// Its `--json` output is the value; a failed exit is its diagnostics.
async fn delegate(
    argv: Vec<String>,
    change: &Write,
    id: &str,
    instance: &str,
    trace_id: &crate::id::TraceId,
) -> Result<serde_json::Value> {
    let executable = crate::os_process::resolve_lf_binary()?;
    let mut command = tokio::process::Command::new(&executable);
    // A service has no caller Session/Task. Never lend its launcher's identity
    // to requests, or change process-global environment while serving clients.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            command.env_remove(key);
        }
    }
    command.env("LF_TRACE_ID", trace_id.as_str());
    command.env("LF_HOME", crate::store::lf_home_dir());
    command.env("LF_BIN", executable);
    command
        .args(argv)
        .current_dir(change.directory())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn()?;
    tracing::info!(request = %id, %instance, pid = child.id(), %trace_id, "Server command started");
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");
    // The child is supervised on its own task: a connection that ends drops
    // the answer, never the command's pipes or its exit record.
    let (request_id, instance_name, trace) = (id.to_owned(), instance.to_owned(), trace_id.clone());
    let (output, errors, status) = tokio::spawn(async move {
        let (output, errors, status) = tokio::join!(
            read_command_output(stdout),
            read_command_output(stderr),
            child.wait()
        );
        let code = status.as_ref().map(|status| status.code().unwrap_or(1));
        tracing::info!(request = %request_id, instance = %instance_name, ?code, trace_id = %trace, "Server command exited");
        (output, errors, status)
    })
    .await?;
    let status = status?;
    let errors = errors?;
    for line in errors.lines() {
        tracing::info!(request = %id, %trace_id, diagnostic = line, "Server command diagnostic");
    }
    anyhow::ensure!(
        status.success(),
        "{}",
        match errors.trim() {
            "" => format!("lf exited with {status}"),
            said => said.to_owned(),
        }
    );
    Ok(serde_json::from_str(&output?)?)
}

async fn read_command_output(stream: impl tokio::io::AsyncRead + Unpin) -> Result<String> {
    let mut bytes = Vec::new();
    stream
        .take((MAX_BULK / 2) as u64)
        .read_to_end(&mut bytes)
        .await?;
    anyhow::ensure!(
        bytes.len() < MAX_BULK / 2,
        "command output exceeded its limit"
    );
    Ok(String::from_utf8(bytes)?)
}

/// Run a write on a request worker under `trace_id`. Once admitted,
/// disconnection releases the client; the effect finishes without replay.
/// Diagnostics the shared functions print go to the request's log.
async fn effect(
    target: serde_json::Value,
    id: &str,
    instance: &str,
    trace_id: &crate::id::TraceId,
    run: impl FnOnce() -> Result<serde_json::Value> + Send + 'static,
) -> Result<serde_json::Value, ServerError> {
    let (id, instance, trace_id) = (id.to_owned(), instance.to_owned(), trace_id.clone());
    tokio::task::spawn_blocking(move || {
        let mut diagnostics = Vec::new();
        let result = crate::journal::request::run(&trace_id, &target, || {
            crate::journal::request::diagnostics(&mut diagnostics, run)
        });
        for line in String::from_utf8_lossy(&diagnostics).lines() {
            tracing::info!(request = %id, %trace_id, diagnostic = line, "Server command diagnostic");
        }
        tracing::info!(request = %id, %instance, %trace_id, ok = result.is_ok(), "Server command finished");
        result.map_err(written)
    })
    .await
    .map_err(|join| outcome_unknown(join.into()))?
}

/// The code a failed write names, else the one its error's type implies.
/// A write that failed for another reason may have taken partial effect.
fn written(error: anyhow::Error) -> ServerError {
    use crate::store::StoreError;
    use crate::work::selection::SelectionError;
    if let Some(named) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ServerError>())
    {
        return named.clone();
    }
    let code = error.chain().find_map(|cause| {
        if let Some(store) = cause.downcast_ref::<StoreError>() {
            return match store {
                StoreError::NotFound => Some(ErrorCode::NotFound),
                StoreError::Stale(_) => Some(ErrorCode::Conflict),
                StoreError::InvalidData(_) | StoreError::Serde(_) => Some(ErrorCode::Invalid),
                _ => None,
            };
        }
        if cause.is::<SelectionError>() || cause.is::<serde_json::Error>() {
            return Some(ErrorCode::Invalid);
        }
        None
    });
    match code {
        Some(code) => ServerError::new(code, format!("{error:#}")),
        None => outcome_unknown(error),
    }
}

fn outcome_unknown(error: anyhow::Error) -> ServerError {
    ServerError::new(
        ErrorCode::OutcomeUnknown,
        format!("{error:#}; inspect the affected work before retrying"),
    )
}

async fn subscription(
    id: String,
    query: Read,
    outgoing: mpsc::Sender<ServerMessage>,
    checkpoints: mpsc::Receiver<u64>,
) -> String {
    if let Err(failure) = watch(&id, query, &outgoing, checkpoints).await {
        // The error ends the subscription for the client; a watch that
        // started sent its failure as its last reading.
        let _ = outgoing
            .send(unavailable(failure).into_message(Some(id.clone())))
            .await;
    }
    id
}

/// The shared watch owns a blocking runtime, which cannot be dropped on an
/// async worker. However its task ends, the watch leaves through a blocking one.
struct Watch(Option<Arc<Subscription>>);

impl std::ops::Deref for Watch {
    type Target = Arc<Subscription>;

    fn deref(&self) -> &Arc<Subscription> {
        self.0.as_ref().expect("the watch is held until drop")
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let subscription = self.0.take();
        tokio::task::spawn_blocking(move || drop(subscription));
    }
}

async fn watch(
    id: &str,
    query: Read,
    outgoing: &mpsc::Sender<ServerMessage>,
    mut checkpoints: mpsc::Receiver<u64>,
) -> Result<()> {
    let notify = Arc::new(Notify::new());
    let wake = notify.clone();
    let watch = Watch(Some(Arc::new(
        tokio::task::spawn_blocking(move || {
            crate::journal::request::isolate(|| query.subscribe(move || wake.notify_one()))
        })
        .await??,
    )));
    deliver(
        outgoing,
        ServerMessage::Response {
            id: id.into(),
            trace_id: crate::id::TraceId::default(),
            result: serde_json::Value::Null,
        },
    )
    .await?;
    loop {
        tokio::select! {
            checkpoint = checkpoints.recv() => {
                let Some(checkpoint) = checkpoint else { return Ok(()) };
                watch.checkpoint(checkpoint);
            }
            () = notify.notified() => {
                let pull = Arc::clone(&watch);
                let frame = tokio::task::spawn_blocking(move || crate::journal::request::isolate(|| pull.pull())).await?;
                let mut reason = None;
                if let Some(frame) = frame {
                    let frame: serde_json::Value = serde_json::from_slice(&frame)?;
                    reason = frame["unavailable"].as_str().map(str::to_owned);
                    deliver(outgoing, ServerMessage::Reading { id: id.into(), frame }).await?;
                }
                if watch.ended() {
                    anyhow::bail!("{}", reason.unwrap_or_else(|| "the watch ended".into()));
                }
            }
        }
    }
}

fn error(id: Option<String>, code: ErrorCode, message: impl Into<String>) -> ServerMessage {
    ServerError::new(code, message).into_message(id)
}

fn line(message: &ServerMessage) -> String {
    // `serde_json` never emits a raw newline inside a message.
    serde_json::to_string(message).expect("server messages serialize")
}

async fn deliver(outgoing: &mpsc::Sender<ServerMessage>, message: ServerMessage) -> Result<()> {
    outgoing
        .send(message)
        .await
        .map_err(|_| anyhow::anyhow!("connection closed"))
}

pub(super) async fn send<C: Carrier>(carrier: &mut C, message: &ServerMessage) -> Result<()> {
    let line = line(message);
    anyhow::ensure!(
        line.len() <= MAX_MESSAGE,
        "Server message exceeds {MAX_MESSAGE} bytes"
    );
    send_line(carrier, line).await
}

async fn send_line<C: Carrier>(carrier: &mut C, line: String) -> Result<()> {
    tokio::time::timeout(SEND, carrier.send(line))
        .await
        .context("client did not take a message within five seconds")?
        .map_err(Into::into)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{connection, deliver_or_spill, Clients};
    use crate::id::LfProcessId;
    use crate::lf::commands::read::Read;
    use crate::server::ticket::{Admission, Tickets};
    use crate::server::wire::{
        ClientIdentity, ClientMessage, ErrorCode, Hello, Operation, ServerMessage, ServerOperation,
        MAX_BULK, MAX_MESSAGE, REVISION,
    };
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    use tokio_util::codec::{Framed, LinesCodec};

    struct Client {
        reader: BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: tokio::net::unix::OwnedWriteHalf,
        server: tokio::task::JoinHandle<anyhow::Result<()>>,
    }

    impl Client {
        /// A connection to a session with a private, uninitialized Machine.
        fn connect() -> Self {
            Self::connect_with(&Clients::default(), Default::default())
        }

        /// A connection listed in `clients` whose tickets another connection can redeem.
        fn connect_with(clients: &Clients, tickets: Arc<Tickets>) -> Self {
            let (client, server) = UnixStream::pair().unwrap();
            let clients = clients.clone();
            let server = tokio::spawn(async move {
                connection(server, "incarnation", "fixture", &clients, None, tickets).await
            });
            let (read, writer) = client.into_split();
            Self {
                reader: BufReader::new(read),
                writer,
                server,
            }
        }

        async fn send(&mut self, message: &ClientMessage) {
            let mut line = serde_json::to_string(message).unwrap();
            line.push('\n');
            self.writer.write_all(line.as_bytes()).await.unwrap();
        }

        async fn hello(&mut self, revision: u32, min_revision: u32) -> ServerMessage {
            self.send(&ClientMessage::Hello(Hello {
                revision,
                min_revision,
                client: ClientIdentity {
                    kind: "test".into(),
                    platform: "test".into(),
                    name: "fixture".into(),
                    version: "0".into(),
                },
            }))
            .await;
            self.receive().await.unwrap()
        }

        async fn receive(&mut self) -> Option<ServerMessage> {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line).await.unwrap();
            (n > 0).then(|| serde_json::from_str(&line).unwrap())
        }

        async fn ended(mut self) {
            assert!(self.receive().await.is_none(), "connection stayed open");
            self.server.await.unwrap().unwrap();
        }

        /// Present a ticket on a fresh connection and read `admitted`.
        async fn redeem(tickets: Arc<Tickets>, ticket: &str) -> Self {
            let mut connection = Self::connect_with(&Clients::default(), tickets);
            connection
                .send(&ClientMessage::Ticket {
                    ticket: ticket.into(),
                })
                .await;
            assert!(
                matches!(connection.receive().await, Some(ServerMessage::Admitted)),
                "ticket was not admitted"
            );
            connection
        }
    }

    async fn ticket_reply(client: &mut Client, id: &str) -> String {
        let ServerMessage::Response { id: of, result, .. } = client.receive().await.unwrap() else {
            panic!("expected a ticket for {id}")
        };
        assert_eq!(of, id);
        result["ticket"]
            .as_str()
            .expect("a ticket string")
            .to_owned()
    }

    fn catalog() -> Operation {
        Operation::Server(ServerOperation::Configuration {
            request: crate::config::api::Request::Catalog,
        })
    }

    /// Serialize the tests that read an empty Machine registry through env.
    fn with_empty_home(body: impl std::future::Future<Output = ()>) {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        // block_on is synchronous, so the env guard never spans an await point.
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(body);
    }

    #[test]
    fn tickets_carry_uploaded_requests_and_large_answers_off_the_control_connection() {
        with_empty_home(async {
            let tickets = Arc::new(Tickets::default());
            let clients = Clients::default();
            let mut client = Client::connect_with(&clients, tickets.clone());
            client.hello(REVISION, REVISION).await;
            // A request over the control limit travels on its own connection and
            // is answered on the control connection under its own id.
            client
                .send(&ClientMessage::Request {
                    id: "u".into(),
                    operation: Operation::Server(ServerOperation::Upload { bytes: 200 }),
                })
                .await;
            let ticket = ticket_reply(&mut client, "u").await;
            let mut upload = Client::redeem(tickets.clone(), &ticket).await;
            upload
                .send(&ClientMessage::Request {
                    id: "r".into(),
                    operation: catalog(),
                })
                .await;
            let ServerMessage::Response { id, result, .. } = client.receive().await.unwrap() else {
                panic!("expected the uploaded request's answer")
            };
            assert_eq!(id, "r");
            assert!(result.is_array(), "the catalog is its value");
            upload.ended().await;
            // A ticket is single use.
            let mut again = Client::connect_with(&clients, tickets.clone());
            again.send(&ClientMessage::Ticket { ticket }).await;
            assert!(matches!(
                again.receive().await.unwrap(),
                ServerMessage::Error {
                    id: None,
                    code: ErrorCode::NotFound,
                    ..
                }
            ));
            again.ended().await;
            // An answer over the control limit arrives as a ticket for one message.
            let (reader, server) = UnixStream::pair().unwrap();
            let mut carrier = Framed::new(server, LinesCodec::new_with_max_length(MAX_BULK));
            let value = serde_json::Value::String("x".repeat(2 * MAX_MESSAGE));
            deliver_or_spill(
                &mut carrier,
                ServerMessage::Response {
                    id: "big".into(),
                    trace_id: crate::id::TraceId::default(),
                    result: value.clone(),
                },
                &Admission::Local,
                &tickets,
            )
            .await
            .unwrap();
            let mut line = String::new();
            BufReader::new(reader).read_line(&mut line).await.unwrap();
            assert!(line.len() <= MAX_MESSAGE);
            let ServerMessage::Bulk { id, ticket, bytes } = serde_json::from_str(&line).unwrap()
            else {
                panic!("expected a bulk ticket: {line}")
            };
            assert_eq!(id, "big");
            assert!(bytes > MAX_MESSAGE);
            let mut fetch = Client::redeem(tickets.clone(), &ticket).await;
            let ServerMessage::Response {
                id,
                result: fetched,
                ..
            } = fetch.receive().await.unwrap()
            else {
                panic!("expected the spilled answer")
            };
            assert_eq!(id, "big");
            assert_eq!(fetched, value);
            fetch.ended().await;
            // A ticket is a connection's first message, not a session request.
            client
                .send(&ClientMessage::Ticket {
                    ticket: "late".into(),
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Error { id: None, .. }
            ));
            client.ended().await;
        });
    }

    #[test]
    fn attach_ticket_bridges_a_second_connection_to_the_owner() {
        with_empty_home(async {
            let tickets = Arc::new(Tickets::default());
            let clients = Clients::default();
            let owner = LfProcessId::new();
            let listener =
                tokio::net::UnixListener::bind(crate::transport::endpoint(&owner).unwrap())
                    .unwrap();
            let mut client = Client::connect_with(&clients, tickets.clone());
            client.hello(REVISION, REVISION).await;
            client
                .send(&ClientMessage::Request {
                    id: "t".into(),
                    operation: Operation::Server(ServerOperation::Attach {
                        owner: owner.clone(),
                    }),
                })
                .await;
            let ticket = ticket_reply(&mut client, "t").await;
            let terminal = Client::redeem(tickets.clone(), &ticket).await;
            let (mut owned, _) = listener.accept().await.unwrap();
            let Client {
                mut reader,
                mut writer,
                server,
            } = terminal;
            writer.write_all(b"\x12\x00\x00\x00\x02hi").await.unwrap();
            let mut input = [0u8; 7];
            owned.read_exact(&mut input).await.unwrap();
            assert_eq!(&input, b"\x12\x00\x00\x00\x02hi");
            owned.write_all(b"screen").await.unwrap();
            let mut screen = [0u8; 6];
            reader.read_exact(&mut screen).await.unwrap();
            assert_eq!(&screen, b"screen");
            drop(owned);
            drop(writer);
            let mut rest = Vec::new();
            reader.read_to_end(&mut rest).await.unwrap();
            assert!(rest.is_empty());
            server.await.unwrap().unwrap();
            // The control connection is unchanged by the terminal it ticketed.
            client
                .send(&ClientMessage::Request {
                    id: "a".into(),
                    operation: catalog(),
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, .. } if id == "a"
            ));
            // An owner that ended between ticket and redemption fails by name.
            drop(listener);
            client
                .send(&ClientMessage::Request {
                    id: "gone".into(),
                    operation: Operation::Server(ServerOperation::Attach { owner }),
                })
                .await;
            let ticket = ticket_reply(&mut client, "gone").await;
            let mut late = Client::connect_with(&clients, tickets.clone());
            late.send(&ClientMessage::Ticket { ticket }).await;
            let ServerMessage::Error {
                id: None,
                code: ErrorCode::NotFound,
                data,
                ..
            } = late.receive().await.unwrap()
            else {
                panic!("expected the owner to be missing")
            };
            assert!(data.unwrap()["owner"].is_string());
            late.ended().await;
            drop(client.writer);
            client.server.await.unwrap().unwrap();
        });
    }

    #[test]
    fn hello_negotiates_and_requests_answer_by_id_in_either_order() {
        with_empty_home(async {
            let mut client = Client::connect();
            let ServerMessage::Welcome(welcome) = client.hello(REVISION + 1, REVISION).await else {
                panic!("expected welcome");
            };
            assert_eq!(welcome.revision, REVISION);
            assert_eq!(welcome.instance, "incarnation");
            assert_eq!(welcome.executable_sha256, "fixture");
            assert_eq!(welcome.limits.max_message, MAX_MESSAGE);
            assert!(welcome.machine.is_none(), "no Machine registry exists");
            for id in ["a", "b"] {
                client
                    .send(&ClientMessage::Request {
                        id: id.into(),
                        operation: catalog(),
                    })
                    .await;
            }
            let mut answered = Vec::new();
            for _ in 0..2 {
                let ServerMessage::Response { id, result, .. } = client.receive().await.unwrap()
                else {
                    panic!("expected a result")
                };
                assert!(result.is_array(), "the catalog is its value");
                answered.push(id);
            }
            answered.sort();
            assert_eq!(answered, ["a", "b"]);
            // A duplicate unanswered ID is refused; an answered one may be reused.
            client
                .send(&ClientMessage::Request {
                    id: "a".into(),
                    operation: catalog(),
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, .. } if id == "a"
            ));
            drop(client.writer);
            client.server.await.unwrap().unwrap();
        });
    }

    #[tokio::test]
    async fn no_common_revision_names_the_side_to_update_then_closes() {
        for (revision, min_revision, side) in [
            (REVISION + 1, REVISION + 1, "selected installation"),
            (REVISION - 1, REVISION - 1, "client"),
        ] {
            let mut client = Client::connect();
            let ServerMessage::Error {
                id,
                code,
                message,
                data,
            } = client.hello(revision, min_revision).await
            else {
                panic!("expected an error")
            };
            assert_eq!(id, None);
            assert_eq!(code, ErrorCode::Incompatible);
            let update = if side == "client" { "client" } else { "server" };
            assert_eq!(data.unwrap()["update"], update);
            assert!(
                message.ends_with(&format!("update the {side}")),
                "{message}"
            );
            client.ended().await;
        }
    }

    #[tokio::test]
    async fn revision_one_frame_gets_a_revision_one_refusal() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let server = tokio::spawn(async move {
            connection(
                server,
                "incarnation",
                "fixture",
                &Clients::default(),
                None,
                Default::default(),
            )
            .await
        });
        let request = br#"{"version":1,"id":"old","operation":{"kind":"status"}}"#;
        client.write_u32(request.len() as u32).await.unwrap();
        client.write_all(request).await.unwrap();
        let length = client.read_u32().await.unwrap() as usize;
        let mut body = vec![0; length];
        client.read_exact(&mut body).await.unwrap();
        let reply: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(reply["instance"], "incarnation");
        assert_eq!(reply["result"]["kind"], "error");
        assert_eq!(reply["result"]["code"], "incompatible");
        assert_eq!(client.read(&mut [0]).await.unwrap(), 0);
        server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn status_probe_answers_without_hello_then_closes() {
        let mut client = Client::connect();
        client.send(&ClientMessage::Status).await;
        let ServerMessage::Status(status) = client.receive().await.unwrap() else {
            panic!("expected status")
        };
        assert_eq!(status.revision, REVISION);
        assert_eq!(status.instance, "incarnation");
        assert_eq!(status.executable_sha256, "fixture");
        client.ended().await;
    }

    async fn status(clients: &Clients) -> Vec<crate::server::wire::ConnectedClient> {
        let mut probe = Client::connect_with(clients, Default::default());
        probe.send(&ClientMessage::Status).await;
        let Some(ServerMessage::Status(status)) = probe.receive().await else {
            panic!("expected status")
        };
        probe.ended().await;
        status.clients
    }

    #[test]
    fn status_lists_welcomed_clients_until_they_close() {
        with_empty_home(async {
            let clients = Clients::default();
            let mut desktop = Client::connect_with(&clients, Default::default());
            let ServerMessage::Welcome(welcome) = desktop.hello(REVISION, REVISION).await else {
                panic!("expected welcome");
            };
            desktop
                .send(&ClientMessage::Request {
                    id: "a".into(),
                    operation: catalog(),
                })
                .await;
            desktop.receive().await.unwrap();
            let listed = status(&clients).await;
            assert_eq!(listed.len(), 1, "{listed:?}");
            assert_eq!(listed[0].id, welcome.client_id);
            assert_eq!(listed[0].client.name, "fixture");
            assert_eq!(listed[0].requests, 1);
            drop(desktop.writer);
            desktop.server.await.unwrap().unwrap();
            assert!(status(&clients).await.is_empty());
        });
    }

    #[test]
    fn cancel_ends_a_subscription_while_a_sibling_request_answers() {
        with_empty_home(async {
            let mut client = Client::connect();
            assert!(matches!(
                client.hello(REVISION, REVISION).await,
                ServerMessage::Welcome(_)
            ));
            // Live processes need no Machine registry: an empty activity reading.
            client
                .send(&ClientMessage::Subscribe {
                    id: "ps".into(),
                    operation: Read::MonitorPs,
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, result: serde_json::Value::Null, .. } if id == "ps"
            ));
            let ServerMessage::Reading { id, frame } = client.receive().await.unwrap() else {
                panic!("expected the first reading")
            };
            assert_eq!(id, "ps");
            assert!(frame["unavailable"].is_null(), "{frame}");
            client
                .send(&ClientMessage::Checkpoint {
                    id: "ps".into(),
                    checkpoint: 3,
                })
                .await;
            let ServerMessage::Reading { frame, .. } = client.receive().await.unwrap() else {
                panic!("expected the checkpoint's reading")
            };
            assert_eq!(frame["answers"], 3);
            client
                .send(&ClientMessage::Cancel { id: "ps".into() })
                .await;
            client
                .send(&ClientMessage::Request {
                    id: "after".into(),
                    operation: catalog(),
                })
                .await;
            // Nothing follows the cancel for `ps`; the sibling still answers.
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, result: serde_json::Value::Array(_), .. } if id == "after"
            ));
            // A cancelled subscription takes no more checkpoints.
            client
                .send(&ClientMessage::Checkpoint {
                    id: "ps".into(),
                    checkpoint: 5,
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Error { id: Some(id), code: ErrorCode::NotFound, .. } if id == "ps"
            ));
            drop(client.writer);
            client.server.await.unwrap().unwrap();
        });
    }

    #[tokio::test]
    async fn a_malformed_line_after_hello_gets_a_null_id_error_then_eof() {
        let mut client = Client::connect();
        assert!(matches!(
            client.hello(REVISION, REVISION).await,
            ServerMessage::Welcome(_)
        ));
        client.writer.write_all(b"not json\n").await.unwrap();
        let ServerMessage::Error { id, code, .. } = client.receive().await.unwrap() else {
            panic!("expected an error")
        };
        assert_eq!(id, None);
        assert_eq!(code, ErrorCode::Invalid);
        client.ended().await;
    }

    #[tokio::test]
    async fn an_overlong_control_line_names_the_upload_ticket_then_closes() {
        let mut client = Client::connect();
        assert!(matches!(
            client.hello(REVISION, REVISION).await,
            ServerMessage::Welcome(_)
        ));
        let mut line = vec![b'x'; MAX_MESSAGE + 1];
        line.push(b'\n');
        client.writer.write_all(&line).await.unwrap();
        let ServerMessage::Error {
            id: None,
            code: ErrorCode::Invalid,
            message,
            data,
        } = client.receive().await.unwrap()
        else {
            panic!("expected the control limit")
        };
        assert!(message.contains("upload ticket"), "{message}");
        assert_eq!(data.unwrap()["max_message"], MAX_MESSAGE);
        client.ended().await;
    }
}
