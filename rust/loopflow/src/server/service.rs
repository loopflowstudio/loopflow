//! One session per connection: hello, welcome, then requests multiplexed by id
//! until the client closes. Admitted work finishes even when its answer is lost.
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use fs2::FileExt;
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::{AbortHandle, JoinSet};
use tokio_util::codec::{Framed, LinesCodec};

use super::outbox::{Outbox, Reservation, MAX_OPEN, MAX_RETAINED, MAX_WRITES};
use super::subscriptions::{Member, Subscriptions};
use super::wire::{
    ClientIdentity, ClientMessage, ConnectedClient, Hello, Limits, Operation, ServerMessage,
    ServerOperation, SessionAttached, Status, Welcome, MAX_CONNECTIONS, MAX_MESSAGE, MIN_REVISION,
    REVISION, TICKET_LIFETIME,
};
use crate::engine::{Error, ErrorCode};
use crate::id::LfProcessId;
use crate::lf::commands::read::Read;

/// How long a connection has to send its first message.
const FIRST_MESSAGE: Duration = Duration::from_secs(5);
/// How long a client has to take a message before its connection ends.
const SEND: Duration = Duration::from_secs(5);
/// How long the listener rests after `accept` fails, as it does out of descriptors.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);

type Lines = Framed<UnixStream, LinesCodec>;

/// Welcomed connections by client ID, as `lf server status` lists them.
type Clients = Arc<Mutex<HashMap<String, ClientState>>>;

struct ClientState {
    row: ConnectedClient,
    outbox: Outbox,
    open: Arc<Semaphore>,
}

/// Bridges `session.attach` has granted and no connection has opened yet.
#[derive(Debug, Default)]
struct Tickets(Mutex<HashMap<String, Ticket>>);

#[derive(Debug)]
struct Ticket {
    owner: LfProcessId,
    /// The user who requested it; only that user's connection may present it.
    uid: u32,
    issued: Instant,
}

impl Tickets {
    /// Grant `uid` one bridge to `owner`, open until `TICKET_LIFETIME` passes.
    fn issue(&self, owner: LfProcessId, uid: u32, now: Instant) -> String {
        use base64::prelude::{Engine, BASE64_URL_SAFE_NO_PAD};
        let ticket = BASE64_URL_SAFE_NO_PAD.encode(uuid::Uuid::new_v4().as_bytes());
        let mut tickets = lock(&self.0);
        tickets.retain(|_, held| now.duration_since(held.issued) < TICKET_LIFETIME);
        tickets.insert(
            ticket.clone(),
            Ticket {
                owner,
                uid,
                issued: now,
            },
        );
        ticket
    }

    /// The owner `ticket` opens for `uid`; the ticket is spent either way.
    /// Unknown, spent, expired and another user's tickets are alike: there is
    /// nothing to tell a presenter who was never granted one.
    fn redeem(&self, ticket: &str, uid: u32, now: Instant) -> Result<LfProcessId, Error> {
        let granted = lock(&self.0).remove(ticket);
        match granted {
            Some(held) if held.uid == uid && now.duration_since(held.issued) < TICKET_LIFETIME => {
                Ok(held.owner)
            }
            _ => Err(Error::new(
                ErrorCode::NotFound,
                "attach ticket is unknown, spent or expired; request session.attach again",
            )),
        }
    }
}

/// A welcomed connection's row, removed when the connection ends.
struct Admitted {
    clients: Clients,
    id: String,
}

impl Admitted {
    fn new(
        clients: &Clients,
        client: ClientIdentity,
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
                outbox,
                open,
            },
        );
        Self {
            clients: clients.clone(),
            id,
        }
    }

    fn requested(&self) {
        if let Some(row) = lock(&self.clients).get_mut(&self.id) {
            row.row.requests += 1;
        }
    }
}

impl Drop for Admitted {
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
    let _observations = crate::observe::Cache::shared();
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
    let tickets = Arc::new(Tickets::default());
    let subscriptions = Arc::new(Subscriptions::default());
    tracing::info!(%instance, socket = %path.display(), "Loopflow server ready");
    loop {
        // One connection's failure is never the listener's: every client on
        // this Machine reaches its Sessions through this loop.
        let socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(error) => {
                tracing::warn!(%instance, %error, "Server could not accept a connection");
                tokio::time::sleep(ACCEPT_RETRY).await;
                continue;
            }
        };
        // Unix admission is deliberately unavailable to remote proxies. macOS
        // has no peer to name once a client has closed, so that client is gone.
        // SAFETY: geteuid has no preconditions.
        let uid = match socket.peer_cred() {
            Ok(peer) if peer.uid() == unsafe { libc::geteuid() } => peer.uid(),
            _ => continue,
        };
        let instance = instance.clone();
        let executable_sha256 = executable_sha256.clone();
        let clients = clients.clone();
        let slots = slots.clone();
        let tickets = tickets.clone();
        let subscriptions = subscriptions.clone();
        tokio::spawn(async move {
            let connection = Connection {
                instance: &instance,
                executable_sha256: &executable_sha256,
                clients: &clients,
                slots: &slots,
                tickets: &tickets,
                subscriptions: &subscriptions,
                uid,
            };
            if let Err(error) = connection.serve(socket).await {
                tracing::warn!(%instance, %error, "Server connection ended");
            }
        });
    }
}

/// Capacity answers before closing, so the client knows nothing ran.
async fn refuse_busy(mut lines: Lines) {
    let _ = send(
        &mut lines,
        &ServerMessage::error(
            None,
            Error::new(
                ErrorCode::Busy,
                format!("Server has {MAX_CONNECTIONS} connections; retry shortly"),
            )
            .with(serde_json::json!({ "max_connections": MAX_CONNECTIONS })),
        ),
    )
    .await;
    let mut socket = lines.into_inner();
    let _ = socket.shutdown().await;
    drain(&mut socket).await;
}

/// Read what the client already sent before closing: Linux resets a Unix socket
/// closed with unread input, and the client would lose its answer. A client
/// that keeps writing is not waited for.
async fn drain(socket: &mut UnixStream) {
    let mut rest = [0; 4096];
    let _ = tokio::time::timeout(Duration::from_millis(250), async {
        while socket.read(&mut rest).await? > 0 {}
        std::io::Result::Ok(())
    })
    .await;
}

/// What one accepted socket is served with. Its first message decides what
/// it is: a client (`hello`, which takes one of the `MAX_CONNECTIONS` slots),
/// a probe (`status`) or a terminal bridge (`attach`, bounded by its ticket,
/// never by the client cap).
struct Connection<'a> {
    instance: &'a str,
    executable_sha256: &'a str,
    clients: &'a Clients,
    slots: &'a Arc<Semaphore>,
    tickets: &'a Tickets,
    subscriptions: &'a Arc<Subscriptions>,
    uid: u32,
}

impl Connection<'_> {
    async fn serve(self, mut socket: UnixStream) -> Result<()> {
        let first = tokio::time::timeout(FIRST_MESSAGE, socket.read_u8())
            .await
            .context("no message within five seconds")??;
        // A revision-1 request begins with its four-byte length; JSON begins with
        // `{`. Delete this branch with revision 1.
        if first < 0x20 {
            return refuse_revision_one(socket, self.instance).await;
        }
        let mut lines = Framed::new(socket, LinesCodec::new_with_max_length(MAX_MESSAGE));
        lines.read_buffer_mut().extend_from_slice(&[first]);
        let line = tokio::time::timeout(FIRST_MESSAGE, lines.next())
            .await
            .context("no message within five seconds")?
            .context("closed before its first message")??;
        match serde_json::from_str::<ClientMessage>(&line) {
            Ok(ClientMessage::Hello(hello)) => {
                let Ok(_slot) = self.slots.clone().try_acquire_owned() else {
                    tracing::warn!(instance = %self.instance, "Server connection capacity reached");
                    refuse_busy(lines).await;
                    return Ok(());
                };
                tracing::debug!(
                    active = MAX_CONNECTIONS - self.slots.available_permits(),
                    "Server connection admitted"
                );
                self.client(lines, hello).await
            }
            Ok(ClientMessage::Attach { ticket }) => {
                let owner = match self.tickets.redeem(&ticket, self.uid, Instant::now()) {
                    Ok(owner) => owner,
                    Err(refusal) => {
                        return send(&mut lines, &ServerMessage::error(None, refusal)).await
                    }
                };
                let destination = match UnixStream::connect(crate::transport::endpoint(&owner)?)
                    .await
                {
                    Ok(destination) => destination,
                    Err(failure) => {
                        // A missing owner endpoint means the owner ended.
                        return send(&mut lines, &ServerMessage::error(None, Error::new(ErrorCode::NotFound, format!(
                            "Session owner is not running; no provider restarted and no input replayed: {failure}"
                        )).with(serde_json::json!({ "owner_lf_process_id": owner })))).await;
                    }
                };
                send(
                    &mut lines,
                    &ServerMessage::Bridged {
                        owner_lf_process_id: owner,
                    },
                )
                .await?;
                bridge(lines, destination).await
            }
            Ok(ClientMessage::Status) => {
                send(
                    &mut lines,
                    &ServerMessage::Status(Status {
                        revision: REVISION,
                        min_revision: MIN_REVISION,
                        instance: self.instance.into(),
                        executable_sha256: self.executable_sha256.into(),
                        clients: connected(self.clients),
                    }),
                )
                .await
            }
            Ok(_) => {
                send(
                    &mut lines,
                    &error(
                        None,
                        ErrorCode::Invalid,
                        "the first message must be hello, status or attach",
                    ),
                )
                .await
            }
            Err(parse) => {
                send(
                    &mut lines,
                    &error(None, ErrorCode::Invalid, parse.to_string()),
                )
                .await
            }
        }
    }

    /// Welcome a client and serve its requests until it closes.
    async fn client(self, mut lines: Lines, hello: Hello) -> Result<()> {
        let instance = self.instance;
        let executable_sha256 = self.executable_sha256;
        let Some(revision) = negotiate(&hello) else {
            let side = if hello.min_revision > REVISION {
                "selected installation"
            } else {
                "client"
            };
            return send(
            &mut lines,
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
        .await;
        };
        let (outgoing, queue) = Outbox::new();
        let open = Arc::new(Semaphore::new(MAX_OPEN));
        let admitted = Admitted::new(self.clients, hello.client, outgoing.clone(), open.clone());
        tracing::debug!(%instance, client = %admitted.id, revision, "Server connection welcomed");
        // A Machine registry that is unavailable or uninitialized leaves `machine`
        // null; file settings stay reachable through the same connection.
        let machine = tokio::task::spawn_blocking(|| {
            crate::journal::request::isolate(|| Read::MachineSelf.read())
        })
        .await?
        .ok();
        send(
            &mut lines,
            &ServerMessage::Welcome(Welcome {
                client_id: admitted.id.clone(),
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
        session(
            lines,
            instance.into(),
            &admitted,
            self.tickets,
            self.uid,
            self.subscriptions,
            outgoing,
            queue,
            open,
        )
        .await
    }
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

#[allow(clippy::too_many_arguments)]
async fn session(
    mut lines: Lines,
    instance: Arc<str>,
    admitted: &Admitted,
    tickets: &Tickets,
    uid: u32,
    subscriptions: &Arc<Subscriptions>,
    outgoing: Outbox,
    mut queue: mpsc::Receiver<super::outbox::Frame>,
    open: Arc<Semaphore>,
) -> Result<()> {
    let writes = Arc::new(Semaphore::new(MAX_WRITES));
    // Dropping the set aborts every answer still pending; admitted blocking
    // work finishes on its worker. Final frame delivery releases the request ID.
    let mut tasks: JoinSet<()> = JoinSet::new();
    let mut inflight: HashMap<String, Inflight> = HashMap::new();
    let mut event_seq = 0u64;
    loop {
        tokio::select! {
            incoming = lines.next() => {
                let Some(line) = incoming else { return Ok(()) };
                let line = line?;
                let message = match serde_json::from_str::<ClientMessage>(&line) {
                    Ok(message) => message,
                    Err(parse) => match refused_operation(&line) {
                        // A well-formed request this build cannot run fails alone,
                        // so a newer client keeps its other requests and subscriptions.
                        Some((id, refusal)) => {
                            send(&mut lines, &ServerMessage::error(Some(id), refusal)).await?;
                            continue;
                        }
                        None => {
                            // A null-id error is the connection's last message.
                            send(&mut lines, &error(None, ErrorCode::Invalid, parse.to_string())).await?;
                            return Ok(());
                        }
                    },
                };
                let (id, admission) = match message {
                    ClientMessage::Request { id, operation, select } => (id, Admission::Request(operation, select)),
                    ClientMessage::Subscribe { id, operation, select, resume } => (id, Admission::Subscribe(operation, select, resume)),
                    ClientMessage::Checkpoint { id, checkpoint } => {
                        match inflight.get(&id).and_then(|entry| entry.checkpoints.as_ref()) {
                            Some(checkpoints) => {
                                // A closed producer may still have its final reading and
                                // Ended queued. Let delivery drain without acknowledging
                                // a checkpoint that arrived after the final read.
                                checkpoints.checkpoint(checkpoint);
                            }
                            None => send(&mut lines, &ServerMessage::error(Some(id.clone()), Error::new(ErrorCode::NotFound, "no live subscription has this ID").with(serde_json::json!({ "id": id })))).await?,
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
                        send(&mut lines, &error(None, ErrorCode::Invalid, "hello was already exchanged")).await?;
                        return Ok(());
                    }
                };
                admitted.requested();
                if id.is_empty() || id.len() > 128 {
                    send(&mut lines, &error(Some(id), ErrorCode::Invalid, "request ID must contain 1–128 bytes")).await?;
                    continue;
                }
                if inflight.contains_key(&id) {
                    send(&mut lines, &ServerMessage::error(Some(id.clone()), Error::new(ErrorCode::Conflict, "request ID is still unanswered").with(serde_json::json!({ "id": id })))).await?;
                    continue;
                }
                if inflight.len() >= MAX_OPEN {
                    send(&mut lines, &busy(&id, "open", MAX_OPEN)).await?;
                    continue;
                }
                let Ok(slot) = open.clone().try_acquire_owned() else {
                    send(&mut lines, &busy(&id, "open", MAX_OPEN)).await?;
                    continue;
                };
                let entry = match admission {
                    Admission::Request(ref operation, Some(_)) if !matches!(operation, Operation::Read(_)) => {
                        send(&mut lines, &error(Some(id), ErrorCode::Invalid, "select applies to reads")).await?;
                        continue;
                    }
                    Admission::Request(Operation::Server(ServerOperation::SessionAttach { session, owner_lf_process_id: exact }), None) => {
                        let owner = match attached_owner(session, exact).await {
                            Ok(owner) => owner,
                            Err(refusal) => {
                                send(&mut lines, &ServerMessage::error(Some(id), refusal)).await?;
                                continue;
                            }
                        };
                        // An owner whose endpoint is gone has ended; the bridge
                        // itself connects when the ticket is presented.
                        if !crate::transport::endpoint(&owner)?.exists() {
                            send(&mut lines, &ServerMessage::error(Some(id), Error::new(ErrorCode::NotFound,
                                "Session owner is not running; no provider restarted and no input replayed",
                            ).with(serde_json::json!({ "owner_lf_process_id": owner })))).await?;
                            continue;
                        }
                        let ticket = tickets.issue(owner.clone(), uid, Instant::now());
                        let result = serde_json::to_value(SessionAttached {
                            ticket,
                            owner_lf_process_id: owner,
                            expires_in_ms: TICKET_LIFETIME.as_millis() as u64,
                        }).expect("a ticket serializes");
                        send(&mut lines, &ServerMessage::Response { id, trace_id: crate::id::TraceId::default(), result }).await?;
                        continue;
                    }
                    Admission::Subscribe(query, select, resume) => {
                        let (member, delivery) = match subscriptions.join(&id, query.clone(), select.clone()) {
                            Ok(joined) => joined,
                            Err(failure) => {
                                send(&mut lines, &ServerMessage::error(Some(id), unavailable(failure))).await?;
                                continue;
                            }
                        };
                        let scope = serde_json::to_vec(&(&*instance, &query, &select))?;
                        let (delivery_id, output) = (id.clone(), outgoing.clone());
                        let task = tasks.spawn(async move {
                            let _slot = slot;
                            delivery.run(delivery_id, output, scope, resume).await;
                        });
                        Inflight { task, checkpoints: Some(member) }
                    }
                    Admission::Request(operation, select) => {
                        let write = if matches!(operation, Operation::Write(_)) {
                            let Ok(write) = writes.clone().try_acquire_owned() else {
                                send(&mut lines, &busy(&id, "writes", MAX_WRITES)).await?;
                                continue;
                            };
                            Some(write)
                        } else { None };
                        let Some(reservation) = outgoing.reserve() else {
                            send(&mut lines, &busy(&id, "retained_bytes", MAX_RETAINED)).await?;
                            continue;
                        };
                        reservation.hold((slot, write));
                        let task = tasks.spawn(request(id.clone(), operation, select, instance.clone(), outgoing.clone(), reservation));
                        Inflight { task, checkpoints: None }
                    }
                };
                inflight.insert(id, entry);
            }
            Some(frame) = queue.recv() => {
                let Some(entry) = inflight.get(&frame.id).filter(|entry| Some(entry.task.id()) == frame.task) else { continue; };
                let complete = frame.complete || (frame.response && entry.checkpoints.is_none());
                let line = frame.payload.into_line(&mut event_seq)?;
                tokio::time::timeout(SEND, lines.send(line)).await.context("client did not take a message within five seconds")??;
                drop(frame.reservation);
                let _ = frame.delivered.send(());
                if complete { inflight.remove(&frame.id); }
            },
            Some(finished) = tasks.join_next() => {
                match finished {
                    Ok(()) => {}
                    Err(join) if join.is_cancelled() => {}
                    Err(join) => tracing::warn!(%instance, %join, "Server request task failed"),
                }
            }
        }
    }
}

/// What a request or subscription asks the session to start.
enum Admission {
    Request(Operation, Option<Vec<String>>),
    Subscribe(Read, Option<Vec<String>>, Option<String>),
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
) {
    #[cfg(test)]
    let completion = tests::take_request_completion(&id);
    let write = matches!(operation, Operation::Write(_));
    let started = Instant::now();
    let trace_id = crate::id::TraceId::default();
    let result = operate(operation, &id, &instance, &trace_id, reservation.clone()).await;
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
) -> Result<serde_json::Value, Error> {
    match operation {
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
                crate::lf::commands::account::served(|| change.run())
            })
            .await
        }
        Operation::Server(ServerOperation::SessionAttach { .. }) => {
            unreachable!("the session dispatches attachment")
        }
    }
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

async fn send(lines: &mut Lines, message: &ServerMessage) -> Result<()> {
    let line = line(message);
    anyhow::ensure!(
        line.len() <= MAX_MESSAGE,
        "Server message exceeds {MAX_MESSAGE} bytes"
    );
    tokio::time::timeout(SEND, lines.send(line))
        .await
        .context("client did not take a message within five seconds")??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock, Mutex};
    use std::time::{Duration, Instant};

    use super::{Clients, Connection, Tickets};
    use crate::engine::ErrorCode;
    use crate::id::LfProcessId;
    use crate::lf::commands::read::Read;
    use crate::server::subscriptions::Subscriptions;
    use crate::server::wire::{
        ClientIdentity, ClientMessage, Hello, Operation, ServerMessage, MAX_MESSAGE, REVISION,
    };
    use crate::server::wire::{MAX_CONNECTIONS, TICKET_LIFETIME};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    use tokio::sync::Semaphore;

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
            Self::connect_to(&Clients::default())
        }

        fn connect_to(clients: &Clients) -> Self {
            Self::connect_with(clients, &Arc::new(Subscriptions::default()))
        }

        fn connect_with(clients: &Clients, subscriptions: &Arc<Subscriptions>) -> Self {
            let (client, server) = UnixStream::pair().unwrap();
            let clients = clients.clone();
            let subscriptions = subscriptions.clone();
            let server = tokio::spawn(async move {
                let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
                Connection {
                    instance: "incarnation",
                    executable_sha256: "fixture",
                    clients: &clients,
                    slots: &slots,
                    tickets: &Tickets::default(),
                    subscriptions: &subscriptions,
                    uid: 501,
                }
                .serve(server)
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
            let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
            Connection {
                instance: "incarnation",
                executable_sha256: "fixture",
                clients: &Clients::default(),
                slots: &slots,
                tickets: &Tickets::default(),
                subscriptions: &Arc::new(Subscriptions::default()),
                uid: 501,
            }
            .serve(server)
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
        let mut probe = Client::connect_to(clients);
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
            let mut desktop = Client::connect_to(&clients);
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

    #[test]
    fn a_ticket_opens_one_bridge_for_its_user_within_its_lifetime() {
        let tickets = Tickets::default();
        let owner = LfProcessId::new();
        let issued = Instant::now();
        let ticket = tickets.issue(owner.clone(), 501, issued);
        assert_eq!(ticket.len(), 22, "16 bytes as base64url: {ticket}");
        // Another user's connection cannot present it, and trying spends it.
        assert_eq!(
            tickets.redeem(&ticket, 502, issued).unwrap_err().code,
            ErrorCode::NotFound
        );
        assert_eq!(
            tickets.redeem(&ticket, 501, issued).unwrap_err().code,
            ErrorCode::NotFound
        );
        let ticket = tickets.issue(owner.clone(), 501, issued);
        assert_eq!(
            tickets
                .redeem(
                    &ticket,
                    501,
                    issued + TICKET_LIFETIME - Duration::from_millis(1)
                )
                .unwrap(),
            owner
        );
        // Single use.
        assert!(tickets.redeem(&ticket, 501, issued).is_err());
        // Expired at exactly its lifetime.
        let ticket = tickets.issue(owner.clone(), 501, issued);
        assert!(tickets
            .redeem(&ticket, 501, issued + TICKET_LIFETIME)
            .is_err());
        // Issuing sweeps the expired.
        let stale = tickets.issue(owner.clone(), 501, issued);
        tickets.issue(owner, 501, issued + TICKET_LIFETIME);
        assert!(!super::lock(&tickets.0).contains_key(&stale));
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
            let mut first = Client::connect_with(&clients, &registry);
            let mut second = Client::connect_with(&clients, &registry);
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
            for _ in 0..=MAX_MESSAGE / chunk.len() {
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
}

#[cfg(test)]
#[path = "subscription_measurement.rs"]
pub(super) mod subscription_measurement;
