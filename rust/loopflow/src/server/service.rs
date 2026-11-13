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
use tokio::sync::{mpsc, Semaphore};
use tokio::task::{AbortHandle, JoinSet};
use tokio_util::codec::{Framed, LinesCodec, LinesCodecError};

use super::outbox::{Outbox, Reservation, MAX_OPEN, MAX_RETAINED, MAX_WRITES};
use super::remote::Control;
use super::subscriptions::{Member, Subscriptions};
use super::ticket::{Admission, Purpose, Tickets};
use super::wire::{
    ClientIdentity, ClientMessage, ConnectedClient, Hello, Limits, Operation, ServerMessage,
    ServerOperation, SessionAttached, SessionStarted, Status, Welcome, MAX_BULK, MAX_CONNECTIONS,
    MAX_MESSAGE, MIN_REVISION, REVISION, TICKET_LIFETIME,
};
use crate::engine::{Error, ErrorCode};
use crate::lf::commands::read::Read;
use crate::transport::registry::Registry;
use crate::transport::Host;

/// How long a connection has to send its first message.
pub(super) const FIRST_MESSAGE: Duration = Duration::from_secs(5);
/// How long a client has to take a message before its connection ends.
const SEND: Duration = Duration::from_secs(5);
/// How long a ticketed connection has to carry its one large message.
const BULK: Duration = Duration::from_secs(120);
/// Answers waiting for one connection's socket.
const QUEUE: usize = 64;
/// How long the listener rests after `accept` fails, as it does out of descriptors.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

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
pub(super) type Clients = Arc<Mutex<HashMap<String, ClientState>>>;

pub(super) struct ClientState {
    row: ConnectedClient,
    outbox: Outbox,
    open: Arc<Semaphore>,
}

/// A welcomed connection's row, removed when the connection ends.
pub(super) struct Listed {
    clients: Clients,
    id: String,
    _permit: tokio::sync::OwnedSemaphorePermit,
    outbox: Outbox,
    open: Arc<Semaphore>,
}

impl Listed {
    fn new(
        clients: &Clients,
        client: ClientIdentity,
        permit: tokio::sync::OwnedSemaphorePermit,
        outbox: Outbox,
        open: Arc<Semaphore>,
    ) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        let connected_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("the current time formats as RFC 3339");
        lock(clients).insert(
            id.clone(),
            ClientState {
                row: ConnectedClient {
                    id: id.clone(),
                    client,
                    connected_at,
                    requests: 0,
                    open: 0,
                    retained_bytes: 0,
                },
                outbox: outbox.clone(),
                open: open.clone(),
            },
        );
        Self {
            clients: clients.clone(),
            id,
            _permit: permit,
            outbox,
            open,
        }
    }

    fn requested(&self) {
        if let Some(row) = lock(&self.clients).get_mut(&self.id) {
            row.row.requests += 1;
        }
    }
}

impl Drop for Listed {
    fn drop(&mut self) {
        lock(&self.clients).remove(&self.id);
    }
}

pub(super) fn lock<T>(shared: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Oldest connection first.
fn connected(clients: &Clients) -> Vec<ConnectedClient> {
    let mut rows: Vec<_> = lock(clients)
        .values()
        .map(|state| {
            let mut row = state.row.clone();
            row.open = MAX_OPEN - state.open.available_permits();
            row.retained_bytes = state.outbox.retained();
            row
        })
        .collect();
    rows.sort_by(|a, b| (&a.connected_at, &a.id).cmp(&(&b.connected_at, &b.id)));
    rows
}

pub(super) async fn run() -> Result<()> {
    let observations = crate::observe::Cache::shared();
    let _transcripts = crate::transcript::watch::watch(&observations);
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
    let subscriptions = Arc::new(Subscriptions::default());
    super::wake::start();
    super::statistics::start();
    // Held terminals keep running in their holders whether or not this server
    // can host them; without a registry their views wait for the next server.
    let terminals = match Registry::open(&executable_sha256) {
        Ok(terminals) => Some(terminals),
        Err(error) => {
            tracing::error!(%instance, %error, "Held terminals are not hosted by this server");
            None
        }
    };
    let (remote_control, remote_requests) = mpsc::channel(8);
    let remote = super::remote::manage(
        super::mobile::configured(),
        remote_requests,
        instance.clone(),
        executable_sha256.clone(),
        slots.clone(),
        clients.clone(),
        tickets.clone(),
        subscriptions.clone(),
        terminals.clone(),
    );
    tracing::info!(%instance, socket = %path.display(), "Loopflow server ready");
    tokio::select! {
        result = remote => result,
        _ = tickets.expiring() => unreachable!("ticket expiry runs until shutdown"),
        result = local(listener, instance, executable_sha256, slots, clients, remote_control, tickets.clone(), subscriptions, terminals) => result,
    }
}

#[allow(clippy::too_many_arguments)]
async fn local(
    listener: UnixListener,
    instance: Arc<str>,
    executable_sha256: Arc<String>,
    slots: Arc<Semaphore>,
    clients: Clients,
    remote_control: mpsc::Sender<Control>,
    tickets: Arc<Tickets>,
    subscriptions: Arc<Subscriptions>,
    terminals: Option<Arc<Registry>>,
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
        let subscriptions = subscriptions.clone();
        let terminals = terminals.clone();
        tokio::spawn(async move {
            if let Err(error) = connection(
                socket,
                &instance,
                &executable_sha256,
                &clients,
                Some(remote_control),
                terminals.clone(),
                tickets,
                &slots,
                &subscriptions,
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
#[allow(clippy::too_many_arguments)]
pub(super) async fn connection(
    mut socket: UnixStream,
    instance: &str,
    executable_sha256: &str,
    clients: &Clients,
    remote_control: Option<mpsc::Sender<Control>>,
    terminals: Option<Arc<Registry>>,
    tickets: Arc<Tickets>,
    slots: &Arc<Semaphore>,
    subscriptions: &Arc<Subscriptions>,
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
        Greeting::Session(listed, queue) => {
            session(
                &mut lines,
                instance.into(),
                &listed,
                remote_control,
                Admission::Local,
                tickets,
                terminals,
                subscriptions,
                queue,
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
    Session(Listed, mpsc::Receiver<super::outbox::Frame>),
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
    let (outbox, queue) = Outbox::new();
    let listed = Listed::new(
        clients,
        hello.client,
        permit,
        outbox,
        Arc::new(Semaphore::new(MAX_OPEN)),
    );
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
                max_open: MAX_OPEN,
                max_retained_bytes: MAX_RETAINED,
            },
        }),
    )
    .await?;
    Ok(Greeting::Session(listed, queue))
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
    checkpoints: Option<Member>,
}

/// What a request or subscription asks the session to start.
enum Start {
    Request(Operation, Option<Vec<String>>),
    Subscribe(Read, Option<Vec<String>>, Option<String>),
}

/// Requests until the client closes. Terminal bytes and messages over the
/// control limit never travel here: `session.attach` and `request.upload`
/// answer with tickets for connections of their own.
#[allow(clippy::too_many_arguments)]
pub(super) async fn session<C: Carrier>(
    lines: &mut C,
    instance: Arc<str>,
    listed: &Listed,
    remote_control: Option<mpsc::Sender<Control>>,
    admission: Admission,
    tickets: Arc<Tickets>,
    terminals: Option<Arc<Registry>>,
    subscriptions: &Arc<Subscriptions>,
    mut queue: mpsc::Receiver<super::outbox::Frame>,
) -> Result<()> {
    let outgoing = listed.outbox.clone();
    let open = listed.open.clone();
    let writes = Arc::new(Semaphore::new(MAX_WRITES));
    let mut event_seq = 0u64;
    // Requests that arrived on a ticketed connection join this session's.
    let (uploads, mut uploaded) = mpsc::channel::<(ClientMessage, Arc<Reservation>)>(QUEUE);
    // Dropping the set aborts every answer still pending; admitted blocking
    // work finishes on its worker. IDs stay reserved until final delivery.
    let mut tasks: JoinSet<()> = JoinSet::new();
    let mut inflight: HashMap<String, Inflight> = HashMap::new();
    loop {
        let (message, mut uploaded_reservation) = tokio::select! {
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
                    Ok(message) => (message, None),
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
            Some((message, reservation)) = uploaded.recv() => (message, Some(reservation)),
            Some(frame) = queue.recv() => {
                let Some(entry) = inflight.get(&frame.id).filter(|entry| Some(entry.task.id()) == frame.task) else { continue; };
                let complete = frame.complete || (frame.response && entry.checkpoints.is_none());
                let line = frame.payload.into_line(&mut event_seq)?;
                deliver_or_spill(lines, &frame.id, line, &admission, &tickets, frame.reservation).await?;
                let _ = frame.delivered.send(());
                if complete { inflight.remove(&frame.id); }
                continue;
            },
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
                        checkpoints.checkpoint(checkpoint);
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
        if inflight.len() >= MAX_OPEN {
            send(lines, &busy(&id, "open", MAX_OPEN)).await?;
            continue;
        }
        let Ok(slot) = open.clone().try_acquire_owned() else {
            send(lines, &busy(&id, "open", MAX_OPEN)).await?;
            continue;
        };
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
                Operation::Server(ServerOperation::SessionStart {
                    session,
                    request,
                    argv,
                    cwd,
                    env,
                    rows,
                    cols,
                    placement,
                }),
                None,
            ) => {
                let start = crate::transport::registry::Start {
                    session,
                    request,
                    argv,
                    cwd,
                    env,
                    rows,
                    cols,
                    placement,
                };
                send(lines, &start_held(id, terminals.clone(), Ok(start)).await).await?;
                continue;
            }
            Start::Request(
                Operation::Server(ServerOperation::SessionContinue {
                    session,
                    request,
                    rows,
                    cols,
                }),
                None,
            ) => {
                let start =
                    tokio::task::spawn_blocking(move || continuing(session, request, rows, cols))
                        .await
                        .map_err(unavailable)
                        .and_then(|start| start);
                send(lines, &start_held(id, terminals.clone(), start).await).await?;
                continue;
            }
            Start::Request(
                Operation::Server(ServerOperation::SessionAttach {
                    session,
                    owner_lf_process_id: exact,
                    terminal_id,
                }),
                None,
            ) => {
                let held = terminals
                    .as_ref()
                    .and_then(|terminals| terminals.session_terminal(&session));
                // A terminal whose holder has not answered this server yet.
                let pending = terminals.as_ref().and_then(|terminals| {
                    terminals.pending_terminal(&session, terminal_id.as_ref())
                });
                if exact.is_none() && (terminal_id.is_some() || held.is_some() || pending.is_some())
                {
                    let message = match (terminal_id, held, pending) {
                        (wanted, Some(held), _) if wanted.as_ref().is_none_or(|wanted| *wanted == held) => {
                            let ticket = tickets.issue(admission.clone(), Purpose::Terminal(Host::Terminal(held.clone())));
                            ServerMessage::Response {
                                id,
                                trace_id: crate::id::TraceId::default(),
                                result: serde_json::to_value(SessionAttached {
                                    ticket,
                                    owner_lf_process_id: None,
                                    terminal_id: Some(held),
                                    expires_in_ms: TICKET_LIFETIME.as_millis() as u64,
                                })
                                .expect("a ticket serializes"),
                            }
                        }
                        // Nothing ended; asking again later attaches.
                        (_, _, Some(pending)) => ServerMessage::error(Some(id), crate::transport::registry::waiting(&pending)),
                        (wanted, current, None) => ServerMessage::error(Some(id), Error::new(ErrorCode::NotFound,
                            "terminal has ended; nothing was started and no input replayed",
                        ).with(serde_json::json!({ "terminal_id": wanted, "current_terminal_id": current }))),
                    };
                    send(lines, &message).await?;
                    continue;
                }
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
                let ticket = tickets.issue(
                    admission.clone(),
                    Purpose::Terminal(Host::Owner(owner.clone())),
                );
                let result = serde_json::to_value(SessionAttached {
                    ticket,
                    owner_lf_process_id: Some(owner),
                    terminal_id: None,
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
                let Some(reservation) = outgoing.reserve() else {
                    send(lines, &busy(&id, "retained_bytes", MAX_RETAINED)).await?;
                    continue;
                };
                let ticket = tickets.issue(
                    admission.clone(),
                    Purpose::Upload {
                        requests: uploads.clone(),
                        bytes,
                        reservation,
                    },
                );
                send(lines, &ticket_response(id, ticket)).await?;
                continue;
            }
            Start::Subscribe(query, select, resume) => {
                let (member, delivery) =
                    match subscriptions.join(&id, query.clone(), select.clone()) {
                        Ok(joined) => joined,
                        Err(failure) => {
                            send(lines, &ServerMessage::error(Some(id), unavailable(failure)))
                                .await?;
                            continue;
                        }
                    };
                let scope = serde_json::to_vec(&(&*instance, &query, &select))?;
                let (delivery_id, output) = (id.clone(), outgoing.clone());
                let task = tasks.spawn(async move {
                    let _slot = slot;
                    delivery.run(delivery_id, output, scope, resume).await;
                });
                Inflight {
                    task,
                    checkpoints: Some(member),
                }
            }
            Start::Request(operation, select) => {
                let write = if matches!(operation, Operation::Write(_)) {
                    let Ok(write) = writes.clone().try_acquire_owned() else {
                        send(lines, &busy(&id, "writes", MAX_WRITES)).await?;
                        continue;
                    };
                    Some(write)
                } else {
                    None
                };
                let (request_id, instance, outgoing, remote_control) = (
                    id.clone(),
                    instance.clone(),
                    outgoing.clone(),
                    remote_control.clone(),
                );
                let uploaded_reservation = uploaded_reservation.take();
                let task = tasks.spawn(async move {
                    // Keep admission bounded while waiting for producer headroom.
                    // Cancellation before capacity is available performs no work.
                    let slots = (slot, write);
                    let reservation = match uploaded_reservation {
                        Some(reservation) => reservation,
                        None => match outgoing.wait().await {
                            Ok(reservation) => reservation,
                            Err(_) => return,
                        },
                    };
                    reservation.hold(slots);
                    request(
                        request_id,
                        operation,
                        select,
                        instance,
                        outgoing,
                        reservation,
                        remote_control,
                    )
                    .await;
                });
                Inflight {
                    task,
                    checkpoints: None,
                }
            }
        };
        inflight.insert(id, entry);
    }
}

/// Start a held terminal and answer `{terminal_id}`. Starting a holder can
/// take seconds; nothing else on this connection waits on it but its own
/// requests.
async fn start_held(
    id: String,
    terminals: Option<Arc<Registry>>,
    start: Result<crate::transport::registry::Start, Error>,
) -> ServerMessage {
    let Some(terminals) = terminals else {
        return ServerMessage::error(Some(id), Error::new(ErrorCode::Unavailable,
            "this server hosts no held terminals; see logs/lf.log and restart it with lf server restart",
        ).with(serde_json::json!({ "resource": "held_terminals" })));
    };
    let start = match start {
        Ok(start) => start,
        Err(refusal) => return ServerMessage::error(Some(id), refusal),
    };
    match tokio::task::spawn_blocking(move || terminals.start(start)).await {
        Ok(Ok(terminal_id)) => ServerMessage::Response {
            id,
            trace_id: crate::id::TraceId::default(),
            result: serde_json::to_value(SessionStarted { terminal_id })
                .expect("a terminal ID serializes"),
        },
        // The child may be running: the caller repeats the same request to
        // learn which.
        Ok(Err(failure)) => ServerMessage::error(
            Some(id),
            Error::new(ErrorCode::OutcomeUnknown, format!("{failure:#}")),
        ),
        Err(join) => ServerMessage::error(
            Some(id),
            Error::new(ErrorCode::OutcomeUnknown, join.to_string()),
        ),
    }
}

/// What `session.continue` starts: `lf session resume` of exactly this
/// Session, in its directory, with this server's environment and its own
/// `LF_HOME` and `LF_BIN`.
fn continuing(
    session: crate::id::LfSessionId,
    request: String,
    rows: u16,
    cols: u16,
) -> Result<crate::transport::registry::Start, Error> {
    let registry = crate::store::read_existing_registry()
        .map_err(unavailable)?
        .ok_or_else(|| {
            Error::new(ErrorCode::Unavailable, "Machine registry is unavailable")
                .with(serde_json::json!({ "resource": "machine_registry" }))
        })?;
    let record = registry
        .sqlite
        .session(session.as_str())
        .map_err(unavailable)?
        .ok_or_else(|| {
            Error::new(ErrorCode::NotFound, "Session is unavailable")
                .with(serde_json::json!({ "session": session }))
        })?;
    if let Some(reason) =
        crate::ops::human_session::run_refusal(&registry.sqlite, &record).map_err(unavailable)?
    {
        return Err(Error::new(ErrorCode::Conflict, reason));
    }
    let context = crate::os_process::execution_context().map_err(unavailable)?;
    // This exact Session owns its child, not the service's launcher.
    let mut env: Vec<(String, String)> = std::env::vars()
        .filter(|(name, _)| !name.starts_with("LF_"))
        .collect();
    env.push((
        "LF_HOME".into(),
        context.lf_home.to_string_lossy().into_owned(),
    ));
    env.push((
        "LF_BIN".into(),
        context.lf_bin.to_string_lossy().into_owned(),
    ));
    Ok(crate::transport::registry::Start {
        argv: vec![
            context.lf_bin.to_string_lossy().into_owned(),
            "session".into(),
            "resume".into(),
            "--".into(),
            session.to_string(),
        ],
        cwd: record.cwd.clone(),
        session: Some(session),
        request,
        env,
        rows,
        cols,
        placement: None,
    })
}

/// `{ticket}`: single use, bound to this connection's admission, expiring in
/// [`TICKET_LIFETIME`].
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
        Purpose::Terminal(Host::Terminal(terminal)) => {
            let mut destination =
                UnixStream::connect(crate::transport::registry::entrance()?).await?;
            destination
                .write_all(&crate::transport::registry::named(&terminal))
                .await?;
            // The entrance says whether the viewer protocol follows; a
            // terminal that ended or whose holder has not answered since the
            // ticket is refused by name.
            let mut answer = Vec::new();
            tokio::time::timeout(FIRST_MESSAGE, async {
                loop {
                    match destination.read_u8().await? {
                        b'\n' => return std::io::Result::Ok(()),
                        byte => answer.push(byte),
                    }
                }
            })
            .await
            .context("held terminal entrance did not answer")??;
            if let Some(refusal) = crate::transport::registry::refusal(&answer, &terminal)? {
                send(carrier, &ServerMessage::error(None, refusal)).await?;
                return Ok(None);
            }
            send(
                carrier,
                &ServerMessage::Bridged {
                    owner_lf_process_id: None,
                    terminal_id: Some(terminal),
                },
            )
            .await?;
            Ok(Some(destination))
        }
        Purpose::Terminal(Host::Owner(owner)) => {
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
                    owner_lf_process_id: Some(owner),
                    terminal_id: None,
                },
            )
            .await?;
            Ok(Some(destination))
        }
        Purpose::Download {
            message,
            reservation,
        } => {
            send(carrier, &ServerMessage::Admitted).await?;
            tokio::time::timeout(BULK, carrier.send(message))
                .await
                .context("client did not take the bulk message in time")?
                .map_err(Into::into)?;
            drop(reservation);
            Ok(None)
        }
        Purpose::Upload {
            requests,
            bytes,
            reservation,
        } => {
            send(carrier, &ServerMessage::Admitted).await?;
            let line = tokio::time::timeout(BULK, carrier.next())
                .await
                .context("no uploaded request in time")?
                .context("closed before its uploaded request")?
                .map_err(Into::into)?;
            anyhow::ensure!(
                line.len() <= bytes,
                "uploaded request exceeds its reserved size"
            );
            match serde_json::from_str::<ClientMessage>(&line) {
                Ok(message @ ClientMessage::Request { .. }) => {
                    // The issuing session may have ended; its answer then has no reader.
                    let _ = requests.send((message, reservation)).await;
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

/// Large snapshots use a separate connection; keep their memory charged until delivered.
async fn deliver_or_spill<C: Carrier>(
    carrier: &mut C,
    id: &str,
    text: String,
    admission: &Admission,
    tickets: &Tickets,
    reservation: Arc<Reservation>,
) -> Result<()> {
    if text.len() <= MAX_MESSAGE {
        return send_line(carrier, text).await;
    }
    anyhow::ensure!(text.len() <= MAX_BULK, "answer exceeds max_bulk");
    let bytes = text.len();
    let ticket = tickets.issue(
        admission.clone(),
        Purpose::Download {
            message: text,
            reservation,
        },
    );
    send(
        carrier,
        &ServerMessage::Bulk {
            id: id.into(),
            ticket,
            bytes,
        },
    )
    .await
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
            Ok(registry
                .sqlite
                .session_transport(session.as_str())?
                .and_then(|host| host.owner().cloned()))
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
pub(super) fn unavailable(error: impl Into<anyhow::Error>) -> Error {
    Error::refused(&error.into(), ErrorCode::Unavailable)
}

async fn request(
    id: String,
    operation: Operation,
    select: Option<Vec<String>>,
    instance: Arc<str>,
    outgoing: Outbox,
    reservation: Arc<Reservation>,
    remote_control: Option<mpsc::Sender<Control>>,
) {
    #[cfg(test)]
    let completion = tests::take_request_completion(&id);
    let write = matches!(operation, Operation::Write(_));
    let started = Instant::now();
    let trace_id = crate::id::TraceId::default();
    let result = match operation {
        Operation::Server(ServerOperation::SessionBoundary(boundary)) => {
            session_boundary(boundary, reservation.clone()).await
        }
        operation => {
            operate(
                operation,
                &id,
                &instance,
                &trace_id,
                reservation.clone(),
                remote_control,
            )
            .await
        }
    };
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
    if let Err(failure) = outgoing.deliver(message, reservation).await {
        let _ = deliver(
            &outgoing,
            ServerMessage::error(
                Some(id.clone()),
                Error::new(
                    if write {
                        ErrorCode::OutcomeUnknown
                    } else {
                        ErrorCode::Busy
                    },
                    failure.to_string(),
                )
                .with(serde_json::json!({"limit":"retained_bytes"})),
            ),
        )
        .await;
    }
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
    reservation: Arc<Reservation>,
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

        Operation::Read(query) => tokio::task::spawn_blocking(move || {
            let _held = reservation;
            crate::journal::request::isolate(|| {
                crate::observe::Cache::shared().capture(|| query.read()).0
            })
        })
        .await
        .map_err(unavailable)?
        .map_err(unavailable),
        Operation::Write(change) => {
            let mut target = serde_json::to_value(&change).map_err(unavailable)?;
            if let Some(fields) = target.as_object_mut() {
                // Setting values and authored text stay out of request telemetry.
                for field in [
                    "content",
                    "request",
                    "title",
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
                let _held = reservation;
                crate::lf::commands::account::served(|| Ok(serde_json::to_value(change.run()?)?))
            })
            .await
        }
        Operation::Server(
            ServerOperation::SessionAttach { .. }
            | ServerOperation::SessionStart { .. }
            | ServerOperation::SessionContinue { .. }
            | ServerOperation::RequestUpload { .. },
        ) => {
            unreachable!("the session dispatches attachment, starts and uploads")
        }
        Operation::Server(ServerOperation::SessionBoundary(_)) => {
            unreachable!("request dispatches boundaries")
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

/// Record a hook's boundary and answer its steers. A `stop`'s planning refresh
/// and pending move start before the answer and never delay it; a closed
/// connection does not cancel them.
async fn session_boundary(
    boundary: super::wire::SessionBoundary,
    reservation: Arc<Reservation>,
) -> Result<serde_json::Value, Error> {
    let (answer, after) = tokio::task::spawn_blocking(move || {
        let _held = reservation;
        crate::journal::request::isolate(|| {
            let path = crate::store::default_db_path();
            anyhow::ensure!(path.exists(), "Machine registry is absent");
            let store = crate::store::sqlite::SqliteStore::new(&path)?;
            super::boundary::answer(&store, &boundary)
        })
    })
    .await
    .map_err(unavailable)?
    .map_err(unavailable)?;
    if let Some(after) = after {
        tokio::task::spawn_blocking(move || {
            crate::journal::request::isolate(|| super::boundary::after_stop(after))
        });
    }
    serde_json::to_value(answer).map_err(unavailable)
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

fn busy(id: &str, limit: &str, max: usize) -> ServerMessage {
    ServerMessage::error(
        Some(id.into()),
        Error::new(ErrorCode::Busy, "server capacity reached; retry shortly")
            .with(serde_json::json!({ "limit": limit, "max": max })),
    )
}

/// A checkpoint identifies the selected reading in this server incarnation.
/// Recomputing it needs no retained payloads or disconnected subscriptions.
pub(super) fn reading_checkpoint(scope: &[u8], frame: &serde_json::Value) -> Result<String> {
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

pub(super) async fn deliver(outgoing: &Outbox, message: ServerMessage) -> Result<()> {
    outgoing.deliver(message, outgoing.wait().await?).await
}

async fn send<C: Carrier>(carrier: &mut C, message: &ServerMessage) -> Result<()> {
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
    use crate::server::subscriptions::Subscriptions;
    use crate::server::ticket::{Admission, Purpose, Tickets};
    use crate::server::wire::MAX_CONNECTIONS;
    use crate::server::wire::{
        ClientIdentity, ClientMessage, Hello, Operation, ServerMessage, ServerOperation, MAX_BULK,
        MAX_MESSAGE, REVISION,
    };
    use crate::transport::Host;
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
            Self::connect_shared(clients, tickets, &Arc::new(Subscriptions::default()))
        }

        fn connect_to(clients: &Clients) -> Self {
            Self::connect_with(clients, Default::default())
        }

        fn connect_shared(
            clients: &Clients,
            tickets: Arc<Tickets>,
            subscriptions: &Arc<Subscriptions>,
        ) -> Self {
            Self::connect_counted(
                clients,
                tickets,
                subscriptions,
                &Arc::new(Semaphore::new(MAX_CONNECTIONS)),
            )
        }

        /// A connection whose client slot comes from `slots`, which the
        /// server's connections share.
        fn connect_counted(
            clients: &Clients,
            tickets: Arc<Tickets>,
            subscriptions: &Arc<Subscriptions>,
            slots: &Arc<Semaphore>,
        ) -> Self {
            let (subscriptions, slots) = (subscriptions.clone(), slots.clone());
            let (client, server) = UnixStream::pair().unwrap();
            let clients = clients.clone();
            let server = tokio::spawn(async move {
                connection(
                    server,
                    "incarnation",
                    "fixture",
                    &clients,
                    None,
                    None,
                    tickets,
                    &slots,
                    &subscriptions,
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
                "big",
                serde_json::to_string(&ServerMessage::Response {
                    id: "big".into(),
                    trace_id: crate::id::TraceId::default(),
                    result: value.clone(),
                })
                .unwrap(),
                &Admission::Local,
                &tickets,
                super::Outbox::new().0.reserve().unwrap(),
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
            let ticket = tickets.issue(
                Admission::Local,
                Purpose::Terminal(Host::Owner(owner.clone())),
            );
            let terminal = Client::redeem(tickets.clone(), &ticket).await;
            let (mut owned, _) = listener.accept().await.unwrap();
            // Replaying the ticket, or presenting a phone's here, opens nothing
            // and spends it.
            let phone = Admission::Remote {
                node: "phone".into(),
                user: 1,
            };
            let remote =
                tickets.issue(phone.clone(), Purpose::Terminal(Host::Owner(owner.clone())));
            for presented in [ticket, remote.clone()] {
                let mut refused = Client::connect_with(&clients, tickets.clone());
                refused
                    .send(&ClientMessage::Attach { ticket: presented })
                    .await;
                assert!(matches!(
                    refused.receive().await.unwrap(),
                    ServerMessage::Error {
                        id: None,
                        code: ErrorCode::NotFound,
                        ..
                    }
                ));
                refused.ended().await;
            }
            assert!(tickets.redeem(&remote, &phone).is_none());
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
            // A bridge takes no client slot: more bridges than the server
            // admits clients, and a client still gets in.
            let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
            let subscriptions = Arc::new(Subscriptions::default());
            let mut bridges = Vec::new();
            for _ in 0..MAX_CONNECTIONS + 8 {
                let ticket = tickets.issue(
                    Admission::Local,
                    Purpose::Terminal(Host::Owner(owner.clone())),
                );
                let mut bridge =
                    Client::connect_counted(&clients, tickets.clone(), &subscriptions, &slots);
                bridge.send(&ClientMessage::Attach { ticket }).await;
                assert!(matches!(
                    bridge.receive().await.unwrap(),
                    ServerMessage::Bridged { owner_lf_process_id: Some(bridged), terminal_id: None }
                        if bridged == owner
                ));
                bridges.push((bridge, listener.accept().await.unwrap().0));
            }
            let mut desktop =
                Client::connect_counted(&clients, tickets.clone(), &subscriptions, &slots);
            assert!(matches!(
                desktop.hello(REVISION, REVISION).await,
                ServerMessage::Welcome(_)
            ));
            drop(bridges);
            // An attach without a ticket is refused before any bridge.
            let mut malformed = Client::connect_with(&clients, tickets.clone());
            malformed
                .writer
                .write_all(b"{\"type\":\"attach\"}\n")
                .await
                .unwrap();
            assert!(matches!(
                malformed.receive().await.unwrap(),
                ServerMessage::Error {
                    id: None,
                    code: ErrorCode::Invalid,
                    ..
                }
            ));
            malformed.ended().await;
            // An owner that ended between ticket and redemption fails by name.
            drop(listener);
            let ticket = tickets.issue(Admission::Local, Purpose::Terminal(Host::Owner(owner)));
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
                None,
                Default::default(),
                &Arc::new(Semaphore::new(MAX_CONNECTIONS)),
                &Arc::new(Subscriptions::default()),
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

    #[test]
    fn typed_events_share_connection_order_survive_selection_and_stop_on_cancel() {
        with_empty_home(async {
            use crate::durable::{Author, WorkRef};
            use crate::work::task::TaskId;
            let path = crate::store::database_path_from_env().unwrap();
            let store = crate::store::sqlite::SqliteStore::open_ephemeral(&path).unwrap();
            rusqlite::Connection::open(&path)
                .unwrap()
                .execute_batch(include_str!(
                    "../../tests/fixtures/saved_selection_work.sql"
                ))
                .unwrap();
            rusqlite::Connection::open(&path)
                .unwrap()
                .execute_batch(
                    "UPDATE tasks SET project_id=NULL;
                 INSERT OR IGNORE INTO machines(id,is_local,created_at,observed_at)
                 VALUES('home_11111111111111111111111111111111',1,1,1);",
                )
                .unwrap();
            let task = TaskId::parse("task_11111111111111111111111111111111").unwrap();
            let work = WorkRef::Task(task);
            store
                .save_steer(&work, &Author::User, "historical")
                .unwrap();
            let mut client = Client::connect();
            client.hello(REVISION, REVISION).await;
            for id in ["a", "b"] {
                client
                    .send(&ClientMessage::Subscribe {
                        id: id.into(),
                        operation: Read::TaskWait {
                            issue: "FIX-1".into(),
                            until: crate::ops::task::TaskWaitUntil::Terminal,
                        },
                        select: Some(vec!["reached".into()]),
                        resume: None,
                    })
                    .await;
                assert!(matches!(
                    client.receive().await.unwrap(),
                    ServerMessage::Response { .. }
                ));
                let ServerMessage::Reading { frame, .. } = client.receive().await.unwrap() else {
                    panic!("missing baseline")
                };
                assert_eq!(
                    frame["body"],
                    serde_json::json!({"reached":false}),
                    "{frame}"
                );
            }
            store
                .save_steer(&work, &Author::User, "first occurrence")
                .unwrap();
            for id in ["a", "b"] {
                client
                    .send(&ClientMessage::Checkpoint {
                        id: id.into(),
                        checkpoint: 1,
                    })
                    .await;
            }
            let mut ids = Vec::new();
            while ids.len() < 2 {
                let message = tokio::time::timeout(Duration::from_secs(10), client.receive())
                    .await
                    .unwrap()
                    .unwrap();
                match message {
                    ServerMessage::Event {
                        id,
                        seq,
                        event: crate::server::wire::SubscriptionEvent::TaskSteer { steer, .. },
                    } => {
                        assert_eq!(seq, ids.len() as u64 + 1);
                        assert_eq!(steer.text, "first occurrence");
                        ids.push(id);
                    }
                    ServerMessage::Reading { .. } => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            ids.sort();
            assert_eq!(ids, ["a", "b"]);
            client.send(&ClientMessage::Cancel { id: "a".into() }).await;
            client
                .send(&ClientMessage::Checkpoint {
                    id: "a".into(),
                    checkpoint: 2,
                })
                .await;
            loop {
                match client.receive().await.unwrap() {
                    ServerMessage::Error {
                        id: Some(id),
                        code: ErrorCode::NotFound,
                        ..
                    } if id == "a" => break,
                    ServerMessage::Reading { .. } => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            store
                .save_steer(&work, &Author::User, "after cancellation")
                .unwrap();
            client
                .send(&ClientMessage::Checkpoint {
                    id: "b".into(),
                    checkpoint: 2,
                })
                .await;
            loop {
                match tokio::time::timeout(Duration::from_secs(10), client.receive())
                    .await
                    .unwrap()
                    .unwrap()
                {
                    ServerMessage::Event { id, seq, .. } => {
                        assert_eq!(id, "b");
                        assert_eq!(seq, 3);
                        break;
                    }
                    ServerMessage::Reading { .. } => {}
                    other => panic!("unexpected {other:?}"),
                }
            }
            client.send(&ClientMessage::Cancel { id: "b".into() }).await;
            client
                .send(&ClientMessage::Request {
                    id: "sibling".into(),
                    operation: catalog(),
                    select: None,
                })
                .await;
            assert!(
                matches!(client.receive().await.unwrap(), ServerMessage::Response { id, .. } if id == "sibling")
            );
            drop(client.writer);
            client.server.await.unwrap().unwrap();
        });
    }

    #[test]
    fn flooded_requests_and_subscriptions_refuse_by_id_and_recover_checkpoints() {
        with_empty_home(async {
            let clients = Clients::default();
            let mut client = Client::connect_to(&clients);
            assert!(matches!(
                client.hello(REVISION, REVISION).await,
                ServerMessage::Welcome(_)
            ));
            let writer = tokio::spawn(async move {
                for n in 0..1300 {
                    let message = if n < 1000 {
                        ClientMessage::Request {
                            id: n.to_string(),
                            operation: catalog(),
                            select: None,
                        }
                    } else {
                        ClientMessage::Subscribe {
                            id: n.to_string(),
                            operation: Read::ProcessList,
                            select: None,
                            resume: None,
                        }
                    };
                    let mut line = serde_json::to_vec(&message).unwrap();
                    line.push(b'\n');
                    client.writer.write_all(&line).await.unwrap();
                }
                client.writer
            });
            let mut answered = std::collections::HashSet::new();
            let mut subscription = None;
            let mut refused = 0;
            while answered.len() < 1300 {
                for reading in super::connected(&clients) {
                    assert!(reading.open <= super::MAX_OPEN);
                    assert!(reading.retained_bytes <= super::MAX_RETAINED);
                }
                let mut line = String::new();
                tokio::time::timeout(Duration::from_secs(10), client.reader.read_line(&mut line))
                    .await
                    .unwrap()
                    .unwrap();
                match serde_json::from_str::<ServerMessage>(&line).unwrap() {
                    ServerMessage::Response { id, result, .. } => {
                        if result.is_null() {
                            subscription = Some(id.clone());
                        }
                        assert!(answered.insert(id));
                    }
                    ServerMessage::Error {
                        id: Some(id),
                        code,
                        data,
                        ..
                    } => {
                        assert_eq!(code, ErrorCode::Busy);
                        assert!(matches!(
                            data.unwrap()["limit"].as_str(),
                            Some("open" | "retained_bytes")
                        ));
                        refused += 1;
                        assert!(answered.insert(id));
                    }
                    ServerMessage::Reading { .. } => {}
                    message => panic!("unexpected {message:?}"),
                }
            }
            assert!(refused > 0);
            let mut writer = writer.await.unwrap();
            let id = subscription.expect("at least one subscription was admitted");
            let line = serde_json::to_string(&ClientMessage::Checkpoint {
                id: id.clone(),
                checkpoint: 42,
            })
            .unwrap()
                + "\n";
            writer.write_all(line.as_bytes()).await.unwrap();
            loop {
                let mut line = String::new();
                tokio::time::timeout(Duration::from_secs(10), client.reader.read_line(&mut line))
                    .await
                    .unwrap()
                    .unwrap();
                if let ServerMessage::Reading {
                    id: actual, frame, ..
                } = serde_json::from_str(&line).unwrap()
                {
                    if actual == id && frame["answers"] == 42 {
                        break;
                    }
                }
            }
            drop(writer);
            client.server.await.unwrap().unwrap();
        });
    }

    #[test]
    fn late_checkpoint_preserves_terminal_reading_and_end() {
        with_empty_home(async {
            let path =
                std::path::PathBuf::from(std::env::var_os("LF_HOME").unwrap()).join("loopflow.db");
            let store = crate::store::sqlite::SqliteStore::open_ephemeral(&path).unwrap();
            let repo = loopflow_test_support::TestRepo::new();
            let task = crate::durable::TaskId::new();
            store
                .create_task(&crate::work::task::NewTask {
                    repository_id: store
                        .ensure_repository(repo.path().to_str().unwrap())
                        .unwrap(),
                    id: task.clone(),
                    project_id: None,
                    title: "Completed wait".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE tasks SET planning_completed=1 WHERE id=?1",
                    [task.as_str()],
                )
                .unwrap();
            let mut client = Client::connect();
            client.hello(REVISION, REVISION).await;
            let pause = super::subscription_measurement::pause("terminal", "terminal-delivery");
            client
                .send(&ClientMessage::Subscribe {
                    resume: None,
                    id: "terminal".into(),
                    operation: Read::TaskWait {
                        issue: task.to_string(),
                        until: crate::ops::task::TaskWaitUntil::Terminal,
                    },
                    select: None,
                })
                .await;
            pause.entered.await.unwrap();
            client
                .send(&ClientMessage::Checkpoint {
                    id: "terminal".into(),
                    checkpoint: 9,
                })
                .await;
            client
                .send(&ClientMessage::Checkpoint {
                    id: "barrier".into(),
                    checkpoint: 1,
                })
                .await;
            let barrier = client.receive().await.unwrap();
            pause.release.send(()).unwrap();
            assert!(
                matches!(barrier, ServerMessage::Error { id: Some(id), code: ErrorCode::NotFound, .. } if id == "barrier")
            );
            assert!(
                matches!(client.receive().await.unwrap(), ServerMessage::Response { id, .. } if id == "terminal")
            );
            let ServerMessage::Reading { id, frame, .. } = client.receive().await.unwrap() else {
                panic!("final reading was lost")
            };
            assert_eq!(id, "terminal");
            assert_eq!(frame["body"]["reached"], true);
            assert!(frame["answers"].is_null());
            assert!(
                matches!(client.receive().await.unwrap(), ServerMessage::Ended { id } if id == "terminal")
            );
            client
                .send(&ClientMessage::Checkpoint {
                    id: "terminal".into(),
                    checkpoint: 10,
                })
                .await;
            assert!(
                matches!(client.receive().await.unwrap(), ServerMessage::Error { id: Some(id), code: ErrorCode::NotFound, .. } if id == "terminal")
            );
            drop(client.writer);
            drop(client.reader);
            client.server.await.unwrap().unwrap();
        });
    }

    #[test]
    fn cancellation_during_setup_and_pull_preserves_sibling_and_reused_id() {
        for stage in ["setup", "pull"] {
            with_empty_home(async {
                let mut client = Client::connect();
                client.hello(REVISION, REVISION).await;
                let pause = super::subscription_measurement::pause("slow", stage);
                client
                    .send(&ClientMessage::Subscribe {
                        resume: None,
                        id: "slow".into(),
                        operation: Read::ProcessList,
                        select: None,
                    })
                    .await;
                pause.queued.await.unwrap();
                pause.entered.await.unwrap();
                if stage == "pull" {
                    assert!(
                        matches!(client.receive().await.unwrap(), ServerMessage::Response { id, .. } if id == "slow")
                    );
                }
                client
                    .send(&ClientMessage::Subscribe {
                        resume: None,
                        id: "peer".into(),
                        operation: Read::ProcessList,
                        select: Some(vec!["id".into()]),
                    })
                    .await;
                assert!(
                    matches!(client.receive().await.unwrap(), ServerMessage::Response { id, .. } if id == "peer")
                );
                assert!(
                    matches!(client.receive().await.unwrap(), ServerMessage::Reading { id, .. } if id == "peer")
                );
                client
                    .send(&ClientMessage::Cancel { id: "slow".into() })
                    .await;
                client
                    .send(&ClientMessage::Checkpoint {
                        id: "peer".into(),
                        checkpoint: 7,
                    })
                    .await;
                let ServerMessage::Reading { id, frame, .. } = client.receive().await.unwrap()
                else {
                    panic!("sibling did not answer")
                };
                assert_eq!(id, "peer");
                assert_eq!(frame["answers"], 7);
                client
                    .send(&ClientMessage::Request {
                        id: "slow".into(),
                        operation: catalog(),
                        select: None,
                    })
                    .await;
                assert!(
                    matches!(client.receive().await.unwrap(), ServerMessage::Response { id, result: serde_json::Value::Array(_), .. } if id == "slow")
                );
                pause.release.send(()).unwrap();
                pause.finished.await.unwrap();
                client
                    .send(&ClientMessage::Checkpoint {
                        id: "peer".into(),
                        checkpoint: 8,
                    })
                    .await;
                let ServerMessage::Reading { id, frame, .. } = client.receive().await.unwrap()
                else {
                    panic!("old work escaped cancellation")
                };
                assert_eq!(id, "peer");
                assert_eq!(frame["answers"], 8);
                drop(client.writer);
                drop(client.reader);
                client.server.await.unwrap().unwrap();
            });
        }
    }

    #[test]
    fn identical_subscriptions_share_one_reader_across_connections() {
        use crate::performance::subscription_measurement as meter;
        with_empty_home(async {
            let registry = Arc::new(Subscriptions::default());
            let clients = Clients::default();
            let mut first = Client::connect_shared(&clients, Default::default(), &registry);
            let mut second = Client::connect_shared(&clients, Default::default(), &registry);
            first.hello(REVISION, REVISION).await;
            second.hello(REVISION, REVISION).await;
            meter::ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
            let before = meter::snapshot();
            let setup = super::subscription_measurement::pause("shared", "setup");
            first
                .send(&ClientMessage::Subscribe {
                    resume: None,
                    id: "shared".into(),
                    operation: Read::ProcessList,
                    select: None,
                })
                .await;
            setup.entered.await.unwrap();
            first
                .send(&ClientMessage::Subscribe {
                    resume: None,
                    id: "same-socket".into(),
                    operation: Read::ProcessList,
                    select: None,
                })
                .await;
            second
                .send(&ClientMessage::Subscribe {
                    resume: None,
                    id: "shared".into(),
                    operation: Read::ProcessList,
                    select: None,
                })
                .await;
            for client in [&mut first, &mut second] {
                client
                    .send(&ClientMessage::Checkpoint {
                        id: "admission-barrier".into(),
                        checkpoint: 0,
                    })
                    .await;
                assert!(matches!(
                    client.receive().await.unwrap(),
                    ServerMessage::Error {
                        code: ErrorCode::NotFound,
                        ..
                    }
                ));
            }
            setup.release.send(()).unwrap();
            let mut bodies = Vec::new();
            for (client, count) in [(&mut first, 4), (&mut second, 2)] {
                for _ in 0..count {
                    match client.receive().await.unwrap() {
                        ServerMessage::Response { .. } => {}
                        ServerMessage::Reading { frame, .. } => bodies.push(frame),
                        message => panic!("unexpected {message:?}"),
                    }
                }
            }
            assert_eq!(bodies.len(), 3);
            assert!(bodies.iter().all(|body| *body == bodies[0]));
            let work = meter::snapshot().since(before);
            meter::ENABLED.store(false, std::sync::atomic::Ordering::Relaxed);
            assert_eq!(work.stages["runtime_build"].count, 1);
            assert_eq!(work.stages["projection"].count, 1);
            drop(first.writer);
            first.server.await.unwrap().unwrap();
            second
                .send(&ClientMessage::Checkpoint {
                    id: "shared".into(),
                    checkpoint: 3,
                })
                .await;
            let ServerMessage::Reading { frame, .. } = second.receive().await.unwrap() else {
                panic!("sibling lost its reader")
            };
            assert_eq!(frame["answers"], 3);
            drop(second.writer);
            second.server.await.unwrap().unwrap();
        });
    }

    #[test]
    fn queued_subscription_cancellation_skips_construction() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .unwrap();
        use crate::performance::subscription_measurement as meter;
        meter::ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
        let before = meter::snapshot();
        runtime.block_on(async {
            let mut client = Client::connect();
            client.hello(REVISION, REVISION).await;
            let mut workers = Vec::new();
            let mut releases = Vec::new();
            for _ in 0..2 {
                let (entered, began) = tokio::sync::oneshot::channel();
                let (release, wait) = std::sync::mpsc::channel();
                releases.push(release);
                workers.push(tokio::task::spawn_blocking(move || {
                    entered.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(30)).unwrap();
                }));
                began.await.unwrap();
            }
            let pause = super::subscription_measurement::pause("queued", "setup");
            client
                .send(&ClientMessage::Subscribe {
                    resume: None,
                    id: "queued".into(),
                    operation: Read::ProcessList,
                    select: None,
                })
                .await;
            pause.queued.await.unwrap();
            client
                .send(&ClientMessage::Cancel {
                    id: "queued".into(),
                })
                .await;
            client
                .send(&ClientMessage::Checkpoint {
                    id: "queued".into(),
                    checkpoint: 1,
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::NotFound,
                    ..
                }
            ));
            // Wake the canceled job if it runs, then drain everything before the assertion.
            let _ = pause.release.send(());
            for release in releases {
                release.send(()).unwrap();
            }
            for worker in workers {
                worker.await.unwrap();
            }
            let _entered = pause.entered.await;
            pause.finished.await.unwrap();
            drop(client.writer);
            drop(client.reader);
            client.server.await.unwrap().unwrap();
        });
        drop(runtime);
        let work = meter::snapshot().since(before);
        meter::ENABLED.store(false, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            work.stages
                .get("runtime_build")
                .map_or(0, |count| count.count),
            0,
            "canceled queued subscription constructed a runtime"
        );
    }

    #[tokio::test]
    async fn an_overlong_line_ends_the_connection() {
        let mut client = Client::connect();
        assert!(matches!(
            client.hello(REVISION, REVISION).await,
            ServerMessage::Welcome(_)
        ));
        let chunk = vec![b'x'; 1024 * 1024];
        let writer = tokio::spawn(async move {
            let mut writer = client.writer;
            for _ in 0..=MAX_BULK / chunk.len() {
                if writer.write_all(&chunk).await.is_err() {
                    return;
                }
            }
        });
        let mut rest = Vec::new();
        client.reader.read_to_end(&mut rest).await.unwrap();
        assert!(rest.is_empty());
        writer.await.unwrap();
        assert!(client.server.await.unwrap().is_err());
    }

    // ---- Work subscriptions over the test's Machine store ----

    /// The Machine store under the test's `LF_HOME`.
    fn home_store() -> crate::store::sqlite::SqliteStore {
        crate::store::sqlite::SqliteStore::open_ephemeral(
            &crate::store::lf_home_dir().join("loopflow.db"),
        )
        .unwrap()
    }

    /// A started (owned) Task in `repo`, so `state:open` selects it.
    fn create_task(
        store: &crate::store::sqlite::SqliteStore,
        repo: &loopflow_test_support::TestRepo,
        title: &str,
    ) -> crate::work::task::Task {
        let path = repo.path().canonicalize().unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository(path.to_str().unwrap()).unwrap(),
                id: crate::durable::TaskId::new(),
                project_id: None,
                title: title.into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,written_here)
                 VALUES(?1,?2,'recorded',NULL,NULL,1,1000,1)",
                [
                    format!("own_{}", uuid::Uuid::new_v4().simple()),
                    task.id.to_string(),
                ],
            )
            .unwrap();
        task
    }

    /// Subscribe as `id` and answer its first reading.
    async fn subscribe(client: &mut Client, id: &str, operation: Read) -> serde_json::Value {
        client
            .send(&ClientMessage::Subscribe {
                id: id.into(),
                operation,
                select: None,
                resume: None,
            })
            .await;
        assert!(matches!(
            client.receive().await.unwrap(),
            ServerMessage::Response { id: of, .. } if of == id
        ));
        reading(client, id).await
    }

    /// The next reading of `id`, within ten seconds.
    async fn reading(client: &mut Client, id: &str) -> serde_json::Value {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(10), client.receive())
                .await
                .expect("a reading within ten seconds")
                .unwrap();
            if let ServerMessage::Reading { id: of, frame, .. } = message {
                if of == id {
                    return frame;
                }
            }
        }
    }

    /// The reading answering checkpoint `answers`.
    async fn confirmed(client: &mut Client, id: &str, answers: u64) -> serde_json::Value {
        client
            .send(&ClientMessage::Checkpoint {
                id: id.into(),
                checkpoint: answers,
            })
            .await;
        loop {
            let frame = reading(client, id).await;
            if frame["answers"] == answers {
                return frame;
            }
        }
    }

    async fn request(client: &mut Client, id: &str, operation: Read) -> serde_json::Value {
        client
            .send(&ClientMessage::Request {
                id: id.into(),
                operation: Operation::Read(operation),
                select: None,
            })
            .await;
        loop {
            match client.receive().await.unwrap() {
                ServerMessage::Response { id: of, result, .. } if of == id => return result,
                ServerMessage::Error {
                    id: Some(of),
                    message,
                    ..
                } if of == id => {
                    panic!("{id} failed: {message}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn session_attach_tickets_the_current_owner_and_refuses_a_replaced_one() {
        with_empty_home(async {
            let store = home_store();
            let session =
                store.test_session("attached", &crate::session_record::new_artifact_key());
            let launcher = store.test_lf_process();
            let attachment = store
                .claim_session_attachment(session.id.as_str(), None, &launcher, true)
                .unwrap();
            let (replaced, current) = (store.test_lf_process(), store.test_lf_process());
            store
                .bind_session_transport(session.id.as_str(), &attachment, &current)
                .unwrap();
            let listener =
                tokio::net::UnixListener::bind(crate::transport::endpoint(&current).unwrap())
                    .unwrap();
            let processes = || -> i64 {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
                    .unwrap()
            };
            let before = processes();
            let (clients, tickets) = (Clients::default(), Arc::new(Tickets::default()));
            let mut client = Client::connect_with(&clients, tickets.clone());
            client.hello(REVISION, REVISION).await;
            let attach = |session: crate::id::LfSessionId, exact: Option<LfProcessId>| {
                ClientMessage::Request {
                    id: "attach".into(),
                    select: None,
                    operation: Operation::Server(ServerOperation::SessionAttach {
                        session,
                        owner_lf_process_id: exact,
                        terminal_id: None,
                    }),
                }
            };

            // An exact owner that was replaced is refused, naming its successor.
            client
                .send(&attach(session.id.clone(), Some(replaced.clone())))
                .await;
            let ServerMessage::Error {
                code: ErrorCode::NotFound,
                data: Some(data),
                ..
            } = client.receive().await.unwrap()
            else {
                panic!("expected the replaced owner to be refused")
            };
            assert_eq!(data["owner_lf_process_id"], replaced.as_str());
            assert_eq!(data["current_lf_process_id"], current.as_str());

            // Without an owner ID, the Session's current owner is ticketed.
            client.send(&attach(session.id.clone(), None)).await;
            let ServerMessage::Response { result, .. } = client.receive().await.unwrap() else {
                panic!("expected a ticket")
            };
            assert_eq!(result["owner_lf_process_id"], current.as_str());
            assert_eq!(
                result["expires_in_ms"],
                super::TICKET_LIFETIME.as_millis() as u64
            );
            let ticket = result["ticket"].as_str().unwrap().to_owned();
            assert_eq!(ticket.len(), 22, "{ticket}");
            // The connection that asked stays a client.
            client
                .send(&ClientMessage::Request {
                    id: "after".into(),
                    operation: catalog(),
                    select: None,
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, .. } if id == "after"
            ));

            // Another connection of the same user redeems the ticket, once.
            let mut bridge = Client::connect_with(&clients, tickets.clone());
            bridge
                .send(&ClientMessage::Attach {
                    ticket: ticket.clone(),
                })
                .await;
            assert!(matches!(
                bridge.receive().await.unwrap(),
                ServerMessage::Bridged { owner_lf_process_id: Some(owner), terminal_id: None }
                    if owner == current
            ));
            listener.accept().await.unwrap();
            let mut spent = Client::connect_with(&clients, tickets.clone());
            spent.send(&ClientMessage::Attach { ticket }).await;
            assert!(matches!(
                spent.receive().await.unwrap(),
                ServerMessage::Error {
                    id: None,
                    code: ErrorCode::NotFound,
                    ..
                }
            ));

            // A Session with no owner gets nothing started.
            client
                .send(&attach(
                    crate::id::LfSessionId::from("session_missing"),
                    None,
                ))
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Error {
                    code: ErrorCode::NotFound,
                    ..
                }
            ));
            assert_eq!(processes(), before);
        });
    }

    /// `session.continue` resumes exactly its Session in the Session's
    /// directory; the registry starts it once per request
    /// (`registry_tests::a_held_terminal_outlives_the_server_that_started_it`).
    #[test]
    fn continuing_a_session_resumes_exactly_it_and_a_missing_one_is_refused() {
        with_empty_home(async {
            let store = home_store();
            let session =
                store.test_session("continued", &crate::session_record::new_artifact_key());
            let id = session.id.clone();
            let start = tokio::task::spawn_blocking(move || {
                super::continuing(id, "phone-continue".into(), 40, 60)
            })
            .await
            .unwrap()
            .unwrap();
            assert_eq!(
                start.argv[1..],
                ["session", "resume", "--", session.id.as_str()]
            );
            assert_eq!(start.cwd, session.cwd);
            assert_eq!(start.session.as_ref(), Some(&session.id));
            assert_eq!(
                (start.request.as_str(), start.rows, start.cols),
                ("phone-continue", 40, 60)
            );
            assert!(start.env.iter().any(|(name, value)| name == "LF_HOME"
                && std::path::Path::new(value) == crate::store::lf_home_dir()));

            let missing = tokio::task::spawn_blocking(|| {
                super::continuing(
                    crate::id::LfSessionId::from("session_00000000000040008000000000000099"),
                    "phone-continue".into(),
                    40,
                    60,
                )
            })
            .await
            .unwrap()
            .unwrap_err();
            assert_eq!(missing.code, ErrorCode::NotFound);
            assert_eq!(missing.message, "Session is unavailable");
        });
    }

    #[test]
    fn a_plan_watch_refreshes_and_a_task_view_resolves_its_primary_session() {
        with_empty_home(async {
            let store = home_store();
            let repo = loopflow_test_support::TestRepo::new();
            let task = create_task(&store, &repo, "Subscribed");
            let session = store.test_session("primary", &crate::session_record::new_artifact_key());
            let mut client = Client::connect();
            client.hello(REVISION, REVISION).await;
            let plans = Read::PlanList {
                directory: "/".into(),
                wave: None,
                task: None,
                all: true,
            };
            let initial = subscribe(&mut client, "plans", plans).await;
            assert!(initial["unavailable"].is_null(), "{initial}");
            assert!(
                initial["body"].to_string().contains("Subscribed"),
                "{initial}"
            );
            assert_eq!(confirmed(&mut client, "plans", 17).await["answers"], 17);

            // A changed Task reaches the watch through the shared invalidation
            // path, and a read on the same connection resolves its primary Session.
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE tasks SET primary_session_id=?2,issue_title='Updated subscribed Task' WHERE id=?1",
                    [task.id.as_str(), session.id.as_str()],
                )
                .unwrap();
            let view = request(
                &mut client,
                "view",
                Read::DesktopViewGet {
                    directory: repo.path().to_path_buf(),
                    expression: Some(format!("task:{}", task.identifier)),
                },
            )
            .await;
            assert_eq!(
                view["contents"][0]["primary_session"],
                session.id.as_str(),
                "{view}"
            );
            let changed = confirmed(&mut client, "plans", 18).await;
            assert!(
                changed["body"]
                    .to_string()
                    .contains("Updated subscribed Task"),
                "{changed}"
            );
        });
    }

    #[test]
    fn a_task_watch_follows_renames_and_the_portfolio_across_repositories() {
        with_empty_home(async {
            use crate::portfolio::PortfolioCommand;
            // Portfolio changes run their own runtime.
            let change = |command: PortfolioCommand| async move {
                tokio::task::spawn_blocking(move || {
                    crate::portfolio::change(&command, &crate::store::lf_home_dir())
                })
                .await
                .unwrap()
                .unwrap();
            };
            let store = home_store();
            let (first, second) = (
                loopflow_test_support::TestRepo::new(),
                loopflow_test_support::TestRepo::new(),
            );
            change(PortfolioCommand::Manual).await;
            change(PortfolioCommand::Add {
                paths: vec![first.path().to_path_buf()],
            })
            .await;
            let task = create_task(&store, &first, "Watched");
            let query = Read::ViewSelect {
                expression: Some("kind:task state:open".into()),
            };
            let contents = |frame: &serde_json::Value| -> Vec<serde_json::Value> {
                frame["body"]
                    .as_array()
                    .unwrap_or_else(|| panic!("expected a complete Task reading: {frame}"))
                    .iter()
                    .flat_map(|plan| plan["contents"].as_array().unwrap().clone())
                    .collect()
            };
            let mut watch = Client::connect();
            watch.hello(REVISION, REVISION).await;
            let initial = subscribe(&mut watch, "tasks", query.clone()).await;
            assert_eq!(contents(&initial).len(), 1, "{initial}");

            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE tasks SET issue_title='Renamed on Desktop' WHERE id=?1",
                    [task.id.as_str()],
                )
                .unwrap();
            assert_eq!(
                contents(&reading(&mut watch, "tasks").await)[0]["title"],
                "Renamed on Desktop"
            );

            // A Task outside the portfolio is absent, as from Desktop's sidebar.
            create_task(&store, &second, "Elsewhere");
            assert_eq!(contents(&confirmed(&mut watch, "tasks", 11).await).len(), 1);
            change(PortfolioCommand::Add {
                paths: vec![second.path().to_path_buf()],
            })
            .await;
            while contents(&reading(&mut watch, "tasks").await).len() != 2 {}
            assert_eq!(contents(&confirmed(&mut watch, "tasks", 12).await).len(), 2);

            // The one-shot read shares the subscription's query and projection.
            let mut once = Client::connect();
            once.hello(REVISION, REVISION).await;
            let plans = request(&mut once, "once", query).await;
            assert_eq!(plans.as_array().unwrap().len(), 2, "{plans}");
        });
    }
}

#[cfg(test)]
#[path = "subscription_measurement.rs"]
pub(super) mod subscription_measurement;
