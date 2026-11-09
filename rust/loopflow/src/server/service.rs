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
    ClientIdentity, ClientMessage, ConnectedClient, Hello, Limits, Operation, ServerMessage,
    ServerOperation, SessionAttached, Status, Welcome, MAX_BULK, MAX_CONNECTIONS, MAX_MESSAGE,
    MIN_REVISION, REVISION, TICKET_LIFETIME,
};
use crate::engine::{Error, ErrorCode};
use crate::lf::commands::{read::Read, subscribe::Subscription, write::Write};

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
/// How long the listener rests after `accept` fails, as it does out of descriptors.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

type Lines = Framed<UnixStream, LinesCodec>;
type Outgoing = mpsc::Sender<(tokio::task::Id, ServerMessage)>;

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

/// A welcomed connection's row in `lf server status`, removed when the
/// connection ends.
pub(super) struct Listed {
    clients: Clients,
    id: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl Listed {
    fn new(
        clients: &Clients,
        client: ClientIdentity,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Self {
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
            _permit: permit,
        }
    }

    fn requested(&self) {
        if let Some(row) = lock(&self.clients).get_mut(&self.id) {
            row.requests += 1;
        }
    }
}

impl Drop for Listed {
    fn drop(&mut self) {
        lock(&self.clients).remove(&self.id);
    }
}

fn lock<T>(shared: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(|poison| poison.into_inner())
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
        let (socket, _) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(error) => {
                tracing::warn!(%error, "Server accept failed; retrying");
                tokio::time::sleep(ACCEPT_RETRY).await;
                continue;
            }
        };
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
        let slots = slots.clone();
        let instance = instance.clone();
        let executable_sha256 = executable_sha256.clone();
        let clients = clients.clone();
        let remote_control = remote_control.clone();
        let tickets = tickets.clone();
        tokio::spawn(async move {
            if let Err(error) = connection(
                socket,
                &instance,
                &executable_sha256,
                &clients,
                Some(remote_control),
                tickets,
                &slots,
            )
            .await
            {
                tracing::warn!(%instance, %error, "Server connection ended");
            }
        });
    }
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
    slots: &Arc<Semaphore>,
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
    match welcome(
        &mut lines,
        &line,
        instance,
        executable_sha256,
        clients,
        slots,
    )
    .await?
    {
        Greeting::Ended => {
            let mut socket = lines.into_inner();
            let _ = socket.shutdown().await;
            drain(&mut socket).await;
            Ok(())
        }
        Greeting::Session(listed) => {
            session(
                &mut lines,
                instance.into(),
                &listed,
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
    Session(Listed),
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
    slots: &Arc<Semaphore>,
) -> Result<Greeting> {
    let hello = match serde_json::from_str::<ClientMessage>(line) {
        Ok(ClientMessage::Hello(hello)) => hello,
        Ok(ClientMessage::Attach { ticket }) => return Ok(Greeting::Ticket(ticket)),
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
            &ServerMessage::error(None, Error::new(
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
            ),
        )
        .await?;
        return Ok(Greeting::Ended);
    };
    let Ok(permit) = slots.clone().try_acquire_owned() else {
        send(
            carrier,
            &ServerMessage::error(
                None,
                Error::new(
                    ErrorCode::Busy,
                    format!("Server has {MAX_CONNECTIONS} connections; retry shortly"),
                )
                .with(serde_json::json!({ "max_connections": MAX_CONNECTIONS })),
            ),
        )
        .await?;
        return Ok(Greeting::Ended);
    };
    let listed = Listed::new(clients, hello.client, permit);
    tracing::debug!(%instance, client = %listed.id, revision, "Server connection welcomed");
    // A Machine registry that is unavailable or uninitialized leaves `machine`
    // null; file settings stay reachable through the same connection.
    let machine = tokio::task::spawn_blocking(|| {
        crate::journal::request::isolate(|| Read::MachineSelf.read())
    })
    .await?;
    let machine = match machine {
        Ok(machine) => Some(machine),
        Err(error) => {
            tracing::warn!(client = %listed.id, %error, "Machine identity unavailable during server welcome");
            None
        }
    };
    send(
        carrier,
        &ServerMessage::Welcome(Welcome {
            client_id: listed.id.clone(),
            revision,
            server_revision: REVISION,
            min_revision: MIN_REVISION,
            instance: instance.into(),
            build: crate::build_info::BUILD_VERSION.to_owned(),
            executable_sha256: executable_sha256.into(),
            machine,
            limits: Limits {
                max_message: MAX_MESSAGE,
                max_connections: MAX_CONNECTIONS,
            },
        }),
    )
    .await?;
    Ok(Greeting::Session(listed))
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
        socket.write_all(&body).await?;
        socket.shutdown().await
    })
    .await??;
    drain(&mut socket).await;
    Ok(())
}

/// A request's task and, for a subscription, where its checkpoints go.
struct Inflight {
    task: AbortHandle,
    checkpoints: Option<mpsc::Sender<u64>>,
}

/// What a request or subscription asks the session to start.
enum Start {
    Request(Operation, Option<Vec<String>>),
    Subscribe(Read, Option<Vec<String>>, Option<String>),
}

/// Requests until the client closes. Terminal bytes and messages over the
/// control limit never travel here: `session.attach` and `request.upload`
/// answer with tickets for connections of their own.
pub(super) async fn session<C: Carrier>(
    lines: &mut C,
    instance: Arc<str>,
    listed: &Listed,
    remote_control: Option<mpsc::Sender<Control>>,
    admission: Admission,
    tickets: Arc<Tickets>,
) -> Result<()> {
    let (outgoing, mut queue) = mpsc::channel(QUEUE);
    // Requests that arrived on a ticketed connection join this session's.
    let (uploads, mut uploaded) = mpsc::channel::<ClientMessage>(QUEUE);
    // Dropping the set aborts every answer still pending; admitted blocking
    // work finishes on its worker. IDs stay reserved until final delivery.
    let mut tasks: JoinSet<()> = JoinSet::new();
    let mut inflight: HashMap<String, Inflight> = HashMap::new();
    loop {
        let message = tokio::select! {
            incoming = lines.next() => {
                let Some(line) = incoming else { return Ok(()) };
                let line = line.map_err(Into::into)?;
                if line.len() > MAX_MESSAGE {
                    // A null-id error is the connection's last message.
                    send(lines, &ServerMessage::error(None, Error::new(ErrorCode::Invalid, format!(
                        "message exceeds {MAX_MESSAGE} bytes; request an upload ticket for it"
                    )).with(serde_json::json!({ "max_message": MAX_MESSAGE })))).await?;
                    return Ok(());
                }
                match serde_json::from_str::<ClientMessage>(&line) {
                    Ok(message) => message,
                    Err(parse) => match refused_operation(&line) {
                        // A well-formed request this build cannot run fails alone,
                        // so a newer client keeps its other requests and subscriptions.
                        Some((id, refusal)) => {
                            send(lines, &ServerMessage::error(Some(id), refusal)).await?;
                            continue;
                        }
                        None => {
                            // A null-id error is the connection's last message.
                            send(lines, &error(None, ErrorCode::Invalid, parse.to_string())).await?;
                            return Ok(());
                        }
                    },
                }
            }
            Some(message) = uploaded.recv() => message,
            Some((task, message)) = queue.recv() => {
                let id = match &message {
                    ServerMessage::Response { id, .. }
                    | ServerMessage::Reading { id, .. }
                    | ServerMessage::Ended { id }
                    | ServerMessage::Error { id: Some(id), .. } => id,
                    _ => unreachable!("workers send only request or subscription messages"),
                };
                // Cancellation may have discarded this answer or reused its ID.
                let Some(entry) = inflight.get(id).filter(|entry| entry.task.id() == task) else {
                    continue;
                };
                let ends = matches!(message, ServerMessage::Error { .. } | ServerMessage::Ended { .. })
                    || (matches!(message, ServerMessage::Response { .. }) && entry.checkpoints.is_none());
                if ends {
                    // The client may reuse this ID as soon as it sees the answer.
                    inflight.remove(id);
                }
                deliver_or_spill(lines, message, &admission, &tickets).await?;
                continue;
            }
            Some(finished) = tasks.join_next() => {
                if let Err(join) = finished {
                    if !join.is_cancelled() {
                        tracing::warn!(%instance, %join, "Server request task failed");
                    }
                }
                continue;
            }
        };
        let (id, start) = match message {
            ClientMessage::Request {
                id,
                operation,
                select,
            } => (id, Start::Request(operation, select)),
            ClientMessage::Subscribe {
                id,
                operation,
                select,
                resume,
            } => (id, Start::Subscribe(operation, select, resume)),
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
                            &ServerMessage::error(
                                Some(id.clone()),
                                Error::new(ErrorCode::NotFound, "no live subscription has this ID")
                                    .with(serde_json::json!({ "id": id })),
                            ),
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
            ClientMessage::Hello(_) | ClientMessage::Status | ClientMessage::Attach { .. } => {
                send(
                    lines,
                    &error(None, ErrorCode::Invalid, "hello was already exchanged"),
                )
                .await?;
                return Ok(());
            }
        };
        listed.requested();
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
                &ServerMessage::error(
                    Some(id.clone()),
                    Error::new(ErrorCode::Conflict, "request ID is still unanswered")
                        .with(serde_json::json!({ "id": id })),
                ),
            )
            .await?;
            continue;
        }
        let entry = match start {
            Start::Request(ref operation, Some(_)) if !matches!(operation, Operation::Read(_)) => {
                send(
                    lines,
                    &error(Some(id), ErrorCode::Invalid, "select applies to reads"),
                )
                .await?;
                continue;
            }
            Start::Request(
                Operation::Server(ServerOperation::SessionAttach {
                    session,
                    owner_lf_process_id: exact,
                }),
                None,
            ) => {
                let owner = match attached_owner(session, exact).await {
                    Ok(owner) => owner,
                    Err(refusal) => {
                        send(lines, &ServerMessage::error(Some(id), refusal)).await?;
                        continue;
                    }
                };
                // An owner whose endpoint is gone has ended; the bridge
                // itself connects when the ticket is presented.
                if !crate::transport::endpoint(&owner)?.exists() {
                    send(lines, &ServerMessage::error(Some(id), Error::new(ErrorCode::NotFound,
                        "Session owner is not running; no provider restarted and no input replayed",
                    ).with(serde_json::json!({ "owner_lf_process_id": owner })))).await?;
                    continue;
                }
                let ticket = tickets.issue(admission.clone(), Purpose::Terminal(owner.clone()));
                let result = serde_json::to_value(SessionAttached {
                    ticket,
                    owner_lf_process_id: owner,
                    expires_in_ms: TICKET_LIFETIME.as_millis() as u64,
                })
                .expect("a ticket serializes");
                send(
                    lines,
                    &ServerMessage::Response {
                        id,
                        trace_id: crate::id::TraceId::default(),
                        result,
                    },
                )
                .await?;
                continue;
            }
            Start::Request(Operation::Server(ServerOperation::RequestUpload { bytes }), None) => {
                if bytes > MAX_BULK {
                    send(
                        lines,
                        &ServerMessage::error(
                            Some(id),
                            Error::new(
                                ErrorCode::Invalid,
                                format!("an uploaded request carries at most {MAX_BULK} bytes"),
                            )
                            .with(serde_json::json!({ "max_bulk": MAX_BULK })),
                        ),
                    )
                    .await?;
                    continue;
                }
                let ticket = tickets.issue(admission.clone(), Purpose::Upload(uploads.clone()));
                send(lines, &ticket_response(id, ticket)).await?;
                continue;
            }
            Start::Subscribe(query, select, resume) => {
                let (checkpoints, inbox) = mpsc::channel(CHECKPOINTS);
                let task = tasks.spawn(subscribing(
                    id.clone(),
                    query,
                    select,
                    resume,
                    instance.clone(),
                    outgoing.clone(),
                    inbox,
                ));
                Inflight {
                    task,
                    checkpoints: Some(checkpoints),
                }
            }
            Start::Request(operation, select) => {
                let task = tasks.spawn(request(
                    id.clone(),
                    operation,
                    select,
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
            &ServerMessage::error(
                None,
                Error::new(
                    ErrorCode::NotFound,
                    "ticket is unknown, expired or was issued to another connection",
                )
                .with(serde_json::json!({ "ticket": ticket })),
            ),
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
                    send(carrier, &ServerMessage::error(None, Error::new(ErrorCode::NotFound, format!(
                        "Session owner is not running; no provider restarted and no input replayed: {failure}"
                    )).with(serde_json::json!({ "owner_lf_process_id": owner })))).await?;
                    return Ok(None);
                }
            };
            send(
                carrier,
                &ServerMessage::Bridged {
                    owner_lf_process_id: owner,
                },
            )
            .await?;
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
            &ServerMessage::error(
                Some(id),
                Error::new(
                    ErrorCode::Unavailable,
                    format!("answer exceeds {MAX_BULK} bytes"),
                )
                .with(serde_json::json!({ "max_bulk": MAX_BULK })),
            ),
        )
        .await;
    }
    let bytes = text.len();
    let ticket = tickets.issue(admission.clone(), Purpose::Download(text));
    send(carrier, &ServerMessage::Bulk { id, ticket, bytes }).await
}

/// The owner `session.attach` reaches: the Session's current owner Process,
/// which must be `exact` when the client names one. Resolution never starts a
/// provider.
async fn attached_owner(
    session: crate::id::LfSessionId,
    exact: Option<crate::id::LfProcessId>,
) -> Result<crate::id::LfProcessId, Error> {
    let current = tokio::task::spawn_blocking({
        let session = session.clone();
        move || -> Result<_> {
            let registry = crate::store::read_existing_registry()?.ok_or_else(|| {
                Error::new(ErrorCode::Unavailable, "Machine registry is unavailable")
                    .with(serde_json::json!({ "resource": "machine_registry" }))
            })?;
            Ok(registry.sqlite.session_transport(session.as_str())?)
        }
    })
    .await
    .map_err(unavailable)?
    .map_err(unavailable)?;
    let Some(current) = current else {
        return Err(Error::new(
            ErrorCode::NotFound,
            format!(
                "Session {} has no running owner; nothing was started",
                session.as_str()
            ),
        )
        .with(serde_json::json!({ "session": session })));
    };
    match exact {
        Some(exact) if exact != current => Err(Error::new(
            ErrorCode::NotFound,
            format!(
                "owner {exact} no longer holds Session {}; its owner is now {current}",
                session.as_str()
            ),
        )
        .with(
            serde_json::json!({ "owner_lf_process_id": exact, "current_lf_process_id": current }),
        )),
        _ => Ok(current),
    }
}

/// A `request` or `subscription` whose envelope parses but whose operation this
/// build does not accept: its id and why. An unknown kind is `invalid` with
/// `data.kind`; a known kind names the argument it refused.
fn refused_operation(line: &str) -> Option<(String, Error)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let kind_of = value.get("type")?.as_str()?;
    if !matches!(kind_of, "request" | "subscribe") {
        return None;
    }
    let id = value.get("id")?.as_str()?.to_owned();
    if id.is_empty() || id.len() > 128 {
        return None;
    }
    let operation = value.get("operation")?;
    let kind = operation.get("kind")?.as_str()?.to_owned();
    // A `subscription` reaches here for any kind that is not a read.
    if kind_of == "subscribe"
        && (serde_json::from_value::<crate::lf::commands::write::Write>(operation.clone()).is_ok()
            || serde_json::from_value::<ServerOperation>(operation.clone()).is_ok())
    {
        let refusal = Error::new(
            ErrorCode::Invalid,
            format!("`{kind}` is not a read, so it has no subscription; send it as a request"),
        );
        return Some((id, refusal.with(serde_json::json!({ "kind": kind }))));
    }
    let refusals = [
        serde_json::from_value::<crate::lf::commands::read::Read>(operation.clone()).err(),
        serde_json::from_value::<crate::lf::commands::write::Write>(operation.clone()).err(),
        serde_json::from_value::<ServerOperation>(operation.clone()).err(),
    ];
    let known = refusals
        .iter()
        .flatten()
        .find(|refusal| !refusal.to_string().starts_with("unknown variant"));
    let refusal = match known {
        Some(refusal) => Error::new(ErrorCode::Invalid, format!("{kind}: {refusal}")),
        None => Error::new(
            ErrorCode::Invalid,
            format!("this server has no operation `{kind}`; update the selected installation"),
        ),
    };
    Some((id, refusal.with(serde_json::json!({ "kind": kind }))))
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

/// A read that failed without naming a code depends on something unavailable.
fn unavailable(error: impl Into<anyhow::Error>) -> Error {
    Error::refused(&error.into(), ErrorCode::Unavailable)
}

async fn request(
    id: String,
    operation: Operation,
    select: Option<Vec<String>>,
    instance: Arc<str>,
    outgoing: Outgoing,
    remote_control: Option<mpsc::Sender<Control>>,
) {
    #[cfg(test)]
    let completion = tests::take_request_completion(&id);
    let started = Instant::now();
    let trace_id = crate::id::TraceId::default();
    let result = operate(operation, &id, &instance, &trace_id, remote_control).await;
    tracing::debug!(request = %id, %instance, %trace_id, elapsed_ms = started.elapsed().as_millis(), ok = result.is_ok(), "Server request ended");
    let message = match result {
        Ok(result) => ServerMessage::Response {
            id: id.clone(),
            trace_id,
            result: match &select {
                Some(paths) => crate::engine::select::select(&result, paths),
                None => result,
            },
        },
        Err(refusal) => {
            tracing::warn!(request = %id, %instance, %trace_id, code = %refusal.code, error = %refusal.message, "Server request failed");
            ServerMessage::error(Some(id.clone()), refusal)
        }
    };
    // A closed connection drops the answer; the effect stands.
    let _ = deliver(&outgoing, message).await;
    #[cfg(test)]
    if let Some(completion) = completion {
        let _ = completion.await;
    }
}

async fn operate(
    operation: Operation,
    id: &str,
    instance: &str,
    trace_id: &crate::id::TraceId,
    remote_control: Option<mpsc::Sender<Control>>,
) -> Result<serde_json::Value, Error> {
    match operation {
        Operation::Server(ServerOperation::MobileStatus) => {
            let (reply, result) = tokio::sync::oneshot::channel();
            mobile_control(remote_control, Control::Status { reply }).await?;
            serde_json::to_value(result.await.map_err(unavailable)?).map_err(unavailable)
        }
        Operation::Server(ServerOperation::MobileConfigure { configuration }) => {
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
                .map_err(|message| Error::new(ErrorCode::Unavailable, message))?;
            Ok(serde_json::json!({ "configuration": configuration }))
        }
        Operation::Read(query) => {
            tokio::task::spawn_blocking(move || crate::journal::request::isolate(|| query.read()))
                .await
                .map_err(unavailable)?
                .map_err(unavailable)
        }
        Operation::Write(change) => {
            if let Some(argv) = change.delegated() {
                let recovery = change_recovery(&change);
                return delegate(argv, &change, id, instance, trace_id)
                    .await
                    .map_err(|error| {
                        Error::new(ErrorCode::OutcomeUnknown, format!("{error:#}; {recovery}"))
                    });
            }
            let mut target = serde_json::to_value(&change).map_err(unavailable)?;
            if let Some(fields) = target.as_object_mut() {
                // Setting values and authored text stay out of request telemetry.
                for field in [
                    "content",
                    "notes",
                    "plan",
                    "layout",
                    "slots",
                    "expression",
                    "value",
                ] {
                    fields.remove(field);
                }
            }
            effect(target, id, instance, trace_id, move || {
                crate::lf::commands::account::served(|| change.run())
            })
            .await
        }
        Operation::Server(
            ServerOperation::SessionAttach { .. } | ServerOperation::RequestUpload { .. },
        ) => {
            unreachable!("the session dispatches attachment")
        }
    }
}

/// Mobile setup belongs to the Machine's own user: the private socket only.
async fn mobile_control(
    remote_control: Option<mpsc::Sender<Control>>,
    request: Control,
) -> Result<(), Error> {
    let Some(control) = remote_control else {
        return Err(Error::new(
            ErrorCode::Invalid,
            "Mobile access is configured on the Machine itself",
        ));
    };
    control
        .send(request)
        .await
        .map_err(|_| Error::new(ErrorCode::Unavailable, "mobile listener is not running"))?;
    Ok(())
}

fn change_recovery(change: &Write) -> &'static str {
    if matches!(change, Write::TaskStart { .. }) {
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
) -> Result<serde_json::Value, Error> {
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
        result.map_err(|error| Error::written(&error))
    })
    .await
    .map_err(|join| Error::outcome_unknown(&join.into()))?
}

async fn subscribing(
    id: String,
    query: Read,
    select: Option<Vec<String>>,
    resume: Option<String>,
    instance: Arc<str>,
    outgoing: Outgoing,
    checkpoints: mpsc::Receiver<u64>,
) {
    if let Err(failure) = subscribe(
        &id,
        query,
        select,
        resume,
        &instance,
        &outgoing,
        checkpoints,
    )
    .await
    {
        // The error ends the subscription for the client; a subscription that
        // started sent its failure as its last reading.
        let _ = deliver(
            &outgoing,
            ServerMessage::error(Some(id), unavailable(failure)),
        )
        .await;
    }
}

/// The shared subscription owns a blocking runtime, which cannot be dropped on an
/// async worker. However its task ends, the subscription leaves through a blocking one.
struct Held(Option<Arc<Subscription>>);

impl std::ops::Deref for Held {
    type Target = Arc<Subscription>;

    fn deref(&self) -> &Arc<Subscription> {
        self.0
            .as_ref()
            .expect("the subscription is held until drop")
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let subscription = self.0.take();
        tokio::task::spawn_blocking(move || drop(subscription));
    }
}

async fn subscribe(
    id: &str,
    query: Read,
    select: Option<Vec<String>>,
    mut resume: Option<String>,
    instance: &str,
    outgoing: &Outgoing,
    mut checkpoints: mpsc::Receiver<u64>,
) -> Result<()> {
    let notify = Arc::new(Notify::new());
    let wake = notify.clone();
    let ends = query.clone();
    let scope = serde_json::to_vec(&(instance, &query, &select))?;
    let subscription = Held(Some(Arc::new(
        tokio::task::spawn_blocking(move || {
            crate::journal::request::isolate(|| query.subscribe(select, move || wake.notify_one()))
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
                subscription.checkpoint(checkpoint);
            }
            () = notify.notified() => {
                let pull = Arc::clone(&subscription);
                let frame = tokio::task::spawn_blocking(move || crate::journal::request::isolate(|| pull.pull())).await?;
                let mut reason = None;
                if let Some(frame) = frame {
                    let frame: serde_json::Value = serde_json::from_slice(&frame)?;
                    reason = frame["unavailable"].as_str().map(str::to_owned);
                    let last = ends.ends_after(&frame["body"]);
                    let checkpoint = reading_checkpoint(&scope, &frame)?;
                    let unchanged = resume.take().as_ref() == Some(&checkpoint)
                        && frame["answers"].is_null() && reason.is_none();
                    if !unchanged {
                        deliver(outgoing, ServerMessage::Reading { id: id.into(), checkpoint, frame }).await?;
                    }
                    if last {
                        return deliver(outgoing, ServerMessage::Ended { id: id.into() }).await;
                    }
                }
                if subscription.ended() {
                    anyhow::bail!("{}", reason.unwrap_or_else(|| "the subscription ended".into()));
                }
            }
        }
    }
}

/// A checkpoint identifies the selected reading in this server incarnation.
/// Recomputing it needs no retained payloads or disconnected subscriptions.
fn reading_checkpoint(scope: &[u8], frame: &serde_json::Value) -> Result<String> {
    use sha2::{Digest, Sha256};
    let reading =
        crate::lf::commands::subscribe::identity(frame["unavailable"].as_str(), frame.get("body"))?;
    let mut hash = Sha256::new();
    hash.update(scope);
    hash.update(reading);
    Ok(format!("{:x}", hash.finalize()))
}

fn error(id: Option<String>, code: ErrorCode, message: impl Into<String>) -> ServerMessage {
    ServerMessage::error(id, Error::new(code, message))
}

fn line(message: &ServerMessage) -> String {
    // `serde_json` never emits a raw newline inside a message.
    serde_json::to_string(message).expect("server messages serialize")
}

async fn deliver(outgoing: &Outgoing, message: ServerMessage) -> Result<()> {
    outgoing
        .send((tokio::task::id(), message))
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

async fn drain(socket: &mut UnixStream) {
    let mut rest = [0; 4096];
    let _ = tokio::time::timeout(Duration::from_millis(250), async {
        while socket.read(&mut rest).await? > 0 {}
        std::io::Result::Ok(())
    })
    .await;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock, Mutex};
    use std::time::Duration;

    use super::{connection, deliver_or_spill, Clients};
    use crate::engine::ErrorCode;
    use crate::id::LfProcessId;
    use crate::lf::commands::read::Read;
    use crate::server::ticket::{Admission, Purpose, Tickets};
    use crate::server::wire::MAX_CONNECTIONS;
    use crate::server::wire::{
        ClientIdentity, ClientMessage, Hello, Operation, ServerMessage, ServerOperation, MAX_BULK,
        MAX_MESSAGE, REVISION,
    };
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    use tokio::sync::Semaphore;
    use tokio_util::codec::{Framed, LinesCodec};

    // Hold a worker after it queues its response, before the JoinSet can reap it.
    static REQUEST_COMPLETIONS: LazyLock<
        Mutex<HashMap<String, tokio::sync::oneshot::Receiver<()>>>,
    > = LazyLock::new(|| Mutex::new(HashMap::new()));

    pub(super) fn take_request_completion(id: &str) -> Option<tokio::sync::oneshot::Receiver<()>> {
        REQUEST_COMPLETIONS.lock().unwrap().remove(id)
    }

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
                connection(
                    server,
                    "incarnation",
                    "fixture",
                    &clients,
                    None,
                    tickets,
                    &Arc::new(Semaphore::new(MAX_CONNECTIONS)),
                )
                .await
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
                .send(&ClientMessage::Attach {
                    ticket: ticket.into(),
                })
                .await;
            assert!(
                matches!(
                    connection.receive().await,
                    Some(ServerMessage::Admitted | ServerMessage::Bridged { .. })
                ),
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
        Operation::Read(crate::lf::commands::read::Read::ConfigCatalog)
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
                    select: None,
                    id: "u".into(),
                    operation: Operation::Server(ServerOperation::RequestUpload { bytes: 200 }),
                })
                .await;
            let ticket = ticket_reply(&mut client, "u").await;
            let mut upload = Client::redeem(tickets.clone(), &ticket).await;
            upload
                .send(&ClientMessage::Request {
                    select: None,
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
            again.send(&ClientMessage::Attach { ticket }).await;
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
                .send(&ClientMessage::Attach {
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
            let ticket = tickets.issue(Admission::Local, Purpose::Terminal(owner.clone()));
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
                    select: None,
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
            let ticket = tickets.issue(Admission::Local, Purpose::Terminal(owner));
            let mut late = Client::connect_with(&clients, tickets.clone());
            late.send(&ClientMessage::Attach { ticket }).await;
            let ServerMessage::Error {
                id: None,
                code: ErrorCode::NotFound,
                data,
                ..
            } = late.receive().await.unwrap()
            else {
                panic!("expected the owner to be missing")
            };
            assert!(data.unwrap()["owner_lf_process_id"].is_string());
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
                        select: None,
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
                    select: None,
                })
                .await;
            let response = client.receive().await.unwrap();
            assert!(
                matches!(&response, ServerMessage::Response { id, .. } if id == "a"),
                "{response:?}"
            );
            drop(client.writer);
            client.server.await.unwrap().unwrap();
        });
    }

    #[test]
    fn answered_request_id_can_be_reused_before_its_worker_returns() {
        with_empty_home(async {
            let mut client = Client::connect();
            assert!(matches!(
                client.hello(REVISION, REVISION).await,
                ServerMessage::Welcome(_)
            ));
            let (release, completion) = tokio::sync::oneshot::channel();
            REQUEST_COMPLETIONS
                .lock()
                .unwrap()
                .insert("held-completion".into(), completion);
            for _ in 0..2 {
                client
                    .send(&ClientMessage::Request {
                        id: "held-completion".into(),
                        operation: catalog(),
                        select: None,
                    })
                    .await;
                let response = client.receive().await.unwrap();
                assert!(
                    matches!(&response, ServerMessage::Response {
                        id, result: serde_json::Value::Array(_), ..
                    } if id == "held-completion"),
                    "{response:?}"
                );
            }
            drop(release);
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
                &Arc::new(Semaphore::new(MAX_CONNECTIONS)),
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
                    select: None,
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
    fn reconnect_resumes_an_unchanged_watch_without_a_complete_reading() {
        with_empty_home(async {
            let mut first = Client::connect();
            first.hello(REVISION, REVISION).await;
            first
                .send(&ClientMessage::Subscribe {
                    id: "first".into(),
                    operation: Read::ProcessList,
                    select: None,
                    resume: None,
                })
                .await;
            assert!(matches!(
                first.receive().await.unwrap(),
                ServerMessage::Response { .. }
            ));
            let ServerMessage::Reading {
                checkpoint, frame, ..
            } = first.receive().await.unwrap()
            else {
                panic!("expected the baseline");
            };
            assert!(frame["unavailable"].is_null());
            // Lose the actual socket while its watch is live.
            drop(first.writer);
            drop(first.reader);
            first.server.await.unwrap().unwrap();

            let mut next = Client::connect();
            next.hello(REVISION, REVISION).await;
            next.send(&ClientMessage::Subscribe {
                id: "new-id".into(),
                operation: Read::ProcessList,
                select: None,
                resume: Some(checkpoint.clone()),
            })
            .await;
            assert!(matches!(
                next.receive().await.unwrap(),
                ServerMessage::Response { .. }
            ));
            assert!(
                tokio::time::timeout(Duration::from_millis(300), next.receive())
                    .await
                    .is_err(),
                "an unchanged watch must not resend its baseline"
            );
            // It is still live and answers an explicit confirmation, unchanged or not.
            next.send(&ClientMessage::Checkpoint {
                id: "new-id".into(),
                checkpoint: 7,
            })
            .await;
            let ServerMessage::Reading {
                checkpoint: confirmed,
                frame,
                ..
            } = next.receive().await.unwrap()
            else {
                panic!("expected the confirmation");
            };
            assert_eq!(frame["answers"], 7);
            assert_eq!(confirmed, checkpoint);

            for (id, resume, select) in [
                ("unknown", Some("unknown".to_owned()), None),
                ("fresh", None, None),
                (
                    "changed-selection",
                    Some(checkpoint),
                    Some(vec!["absent".into()]),
                ),
            ] {
                next.send(&ClientMessage::Subscribe {
                    id: id.into(),
                    operation: Read::ProcessList,
                    select,
                    resume,
                })
                .await;
                assert!(matches!(
                    next.receive().await.unwrap(),
                    ServerMessage::Response { .. }
                ));
                assert!(
                    matches!(next.receive().await.unwrap(), ServerMessage::Reading { id: of, .. } if of == id)
                );
            }
        });
    }

    #[test]
    fn resume_checkpoint_tracks_the_read_selection_and_server_incarnation() {
        let query = Read::SessionList {
            directory: "/repo".into(),
            headless: false,
            after: None,
            limit: None,
            task: None,
        };
        let scope = |instance, query: &Read, select: Option<Vec<&str>>| {
            serde_json::to_vec(&(instance, query, select)).unwrap()
        };
        let original = scope("first-server", &query, None);
        let frame = |title, at, answers| {
            serde_json::json!({
                "body": [{"title": title, "condition": {"observed_at": at}}],
                "unavailable": null, "answers": answers,
            })
        };
        let baseline =
            super::reading_checkpoint(&original, &frame("first", 1, None::<u64>)).unwrap();
        assert_eq!(
            baseline,
            super::reading_checkpoint(&original, &frame("first", 2, Some(7))).unwrap()
        );
        assert_ne!(
            baseline,
            super::reading_checkpoint(&original, &frame("changed", 1, None)).unwrap()
        );
        for changed in [
            scope("restarted-server", &query, None),
            scope("first-server", &Read::ProcessList, None),
            scope(
                "first-server",
                &Read::SessionList {
                    directory: "/another".into(),
                    headless: false,
                    after: None,
                    limit: None,
                    task: None,
                },
                None,
            ),
            scope("first-server", &query, Some(vec!["title"])),
        ] {
            assert_ne!(
                baseline,
                super::reading_checkpoint(&changed, &frame("first", 1, None)).unwrap()
            );
        }
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
                    operation: Read::ProcessList,
                    resume: None,
                    select: None,
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, result: serde_json::Value::Null, .. } if id == "ps"
            ));
            let ServerMessage::Reading { id, frame, .. } = client.receive().await.unwrap() else {
                panic!("expected the first reading")
            };
            assert_eq!(id, "ps");
            assert!(frame["unavailable"].is_null(), "{frame}");
            // Its acknowledgement does not release a subscription's ID.
            client
                .send(&ClientMessage::Request {
                    id: "ps".into(),
                    operation: catalog(),
                    select: None,
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Error { id: Some(id), code: ErrorCode::Conflict, .. } if id == "ps"
            ));
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
                    select: None,
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
            client
                .send(&ClientMessage::Request {
                    id: "ps".into(),
                    operation: catalog(),
                    select: None,
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, result: serde_json::Value::Array(_), .. } if id == "ps"
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

    #[test]
    fn an_unknown_operation_fails_alone_and_the_connection_stays() {
        with_empty_home(async {
            let mut client = Client::connect();
            assert!(matches!(
                client.hello(REVISION, REVISION).await,
                ServerMessage::Welcome(_)
            ));
            client
            .writer
            .write_all(b"{\"type\":\"request\",\"id\":\"new\",\"operation\":{\"kind\":\"task.teleport\"}}\n")
            .await
            .unwrap();
            let ServerMessage::Error { id, code, data, .. } = client.receive().await.unwrap()
            else {
                panic!("expected an error")
            };
            assert_eq!(id.as_deref(), Some("new"));
            assert_eq!(code, ErrorCode::Invalid);
            assert_eq!(data.unwrap()["kind"], "task.teleport");
            client
                .send(&ClientMessage::Request {
                    id: "next".into(),
                    operation: catalog(),
                    select: None,
                })
                .await;
            let ServerMessage::Response { id, .. } = client.receive().await.unwrap() else {
                panic!("the connection should still answer")
            };
            assert_eq!(id, "next");
        });
    }

    #[test]
    fn select_keeps_only_the_named_paths_of_a_reads_answer() {
        with_empty_home(async {
            let mut client = Client::connect();
            assert!(matches!(
                client.hello(REVISION, REVISION).await,
                ServerMessage::Welcome(_)
            ));
            client
                .send(&ClientMessage::Request {
                    id: "keys".into(),
                    operation: catalog(),
                    select: Some(vec!["key".into()]),
                })
                .await;
            let ServerMessage::Response { result, .. } = client.receive().await.unwrap() else {
                panic!("expected the catalog")
            };
            let entries = result.as_array().expect("the catalog is a list");
            assert!(!entries.is_empty());
            for entry in entries {
                let fields: Vec<&String> = entry.as_object().unwrap().keys().collect();
                assert_eq!(fields, ["key"], "{entry}");
            }
            client
                .send(&ClientMessage::Subscribe {
                    id: "ps".into(),
                    operation: Read::ProcessList,
                    resume: None,
                    select: Some(vec!["absent".into()]),
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, .. } if id == "ps"
            ));
            let ServerMessage::Reading { frame, .. } = client.receive().await.unwrap() else {
                panic!("expected the first reading")
            };
            assert_eq!(frame["body"], serde_json::json!({}), "{frame}");
            // A write has no answer to select from.
            client
                .writer
                .write_all(b"{\"type\":\"request\",\"id\":\"w\",\"operation\":{\"kind\":\"task.complete\",\"directory\":\"/r\",\"issue\":\"A-1\"},\"select\":[\"x\"]}\n")
                .await
                .unwrap();
            let ServerMessage::Error {
                id, code, message, ..
            } = client.receive().await.unwrap()
            else {
                panic!("expected a refusal")
            };
            assert_eq!(id.as_deref(), Some("w"));
            assert_eq!(code, ErrorCode::Invalid);
            assert_eq!(message, "select applies to reads");
        });
    }

    #[test]
    fn subscribing_to_a_write_says_it_has_no_subscription() {
        with_empty_home(async {
            let mut client = Client::connect();
            assert!(matches!(
                client.hello(REVISION, REVISION).await,
                ServerMessage::Welcome(_)
            ));
            client
                .writer
                .write_all(b"{\"type\":\"subscribe\",\"id\":\"w\",\"operation\":{\"kind\":\"task.complete\",\"directory\":\"/r\",\"issue\":\"A-1\"}}\n")
                .await
                .unwrap();
            let ServerMessage::Error {
                id, code, message, ..
            } = client.receive().await.unwrap()
            else {
                panic!("expected an error")
            };
            assert_eq!(id.as_deref(), Some("w"));
            assert_eq!(code, ErrorCode::Invalid);
            assert!(message.contains("no subscription"), "{message}");
        });
    }

    #[tokio::test]
    async fn an_unknown_ticket_is_refused_before_any_bridge() {
        let mut client = Client::connect();
        client
            .send(&ClientMessage::Attach {
                ticket: "Qk9nM3hLcjJwVnc5YVptRQ".into(),
            })
            .await;
        let ServerMessage::Error { id, code, .. } = client.receive().await.unwrap() else {
            panic!("expected a refusal");
        };
        assert_eq!(id, None);
        assert_eq!(code, ErrorCode::NotFound);
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
