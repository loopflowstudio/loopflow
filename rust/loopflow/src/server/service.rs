//! One session per connection: hello, welcome, then requests multiplexed by id
//! until the client closes. Admitted work finishes even when its answer is lost.
//! Both carriers dispatch the same operations after their own admission; the
//! remote carrier never connects to the private socket as a proxy Unix user.
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::Arc;
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
    ClientMessage, ErrorCode, Hello, Limits, Operation, Reply, ServerMessage, Status, Welcome,
    MAX_BULK, MAX_CONNECTIONS, MAX_MESSAGE, MIN_REVISION, REVISION,
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
        tickets.clone(),
    );
    tracing::info!(%instance, socket = %path.display(), "Loopflow server ready");
    tokio::select! {
        result = remote => result,
        result = local(listener, instance, executable_sha256, slots, remote_control, tickets) => result,
    }
}

async fn local(
    listener: UnixListener,
    instance: Arc<str>,
    executable_sha256: Arc<String>,
    slots: Arc<Semaphore>,
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
        let remote_control = remote_control.clone();
        let tickets = tickets.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = connection(
                socket,
                &instance,
                &executable_sha256,
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
    let line = line(&error(
        None,
        ErrorCode::Busy,
        format!("Server has {MAX_CONNECTIONS} connections; retry shortly"),
    ));
    let _ = tokio::time::timeout(SEND, socket.write_all(line.as_bytes())).await;
}

/// Local connections may configure mobile access; `remote_control` is absent
/// on the remote carrier.
pub(super) async fn connection(
    mut socket: UnixStream,
    instance: &str,
    executable_sha256: &str,
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
    match welcome(&mut lines, &line, instance, executable_sha256).await? {
        Greeting::Ended => Ok(()),
        Greeting::Session => {
            session(
                &mut lines,
                instance.into(),
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
#[derive(Debug)]
pub(super) enum Greeting {
    /// `welcome` was sent; the session follows.
    Session,
    /// A ticket to redeem; `admitted` or an error follows.
    Ticket(String),
    /// The connection's last message was sent.
    Ended,
}

/// Answer a connection's first message. `Session` once `welcome` was sent and the
/// session may begin; `false` after a `status` probe or a refusal, which close.
pub(super) async fn welcome<C: Carrier>(
    carrier: &mut C,
    line: &str,
    instance: &str,
    executable_sha256: &str,
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
            &error(
                None,
                ErrorCode::Incompatible,
                format!(
                    "Server speaks protocol revisions {MIN_REVISION}–{REVISION}; the client speaks {}–{}; update the {side}",
                    hello.min_revision, hello.revision
                ),
            ),
        )
        .await?;
        return Ok(Greeting::Ended);
    };
    tracing::debug!(%instance, client = ?hello.client, revision, "Server connection welcomed");
    // A Machine registry that is unavailable or uninitialized leaves `machine`
    // null; file settings stay reachable through the same connection.
    let machine =
        tokio::task::spawn_blocking(|| crate::journal::request::isolate(|| Read::MachineId.read()))
            .await?
            .ok();
    send(
        carrier,
        &ServerMessage::Welcome(Welcome {
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
    Ok(Greeting::Session)
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
    checkpoints: Option<mpsc::Sender<serde_json::Value>>,
}

/// Requests until the client closes. Terminal bytes and messages over the
/// control limit never travel here: `attach` and `bulk` answer with tickets
/// for connections of their own.
pub(super) async fn session<C: Carrier>(
    lines: &mut C,
    instance: Arc<str>,
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
                    send(lines, &error(None, ErrorCode::Invalid, format!(
                        "message exceeds {MAX_MESSAGE} bytes; request a bulk ticket for it"
                    ))).await?;
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
        match message {
            ClientMessage::Request { id, operation } => {
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
                        &error(
                            Some(id),
                            ErrorCode::Invalid,
                            "request ID is still unanswered",
                        ),
                    )
                    .await?;
                    continue;
                }
                let entry = match operation {
                    Operation::Attach { owner } => {
                        let ticket = tickets.issue(admission.clone(), Purpose::Terminal(owner));
                        send(
                            lines,
                            &ServerMessage::Response {
                                id,
                                result: Reply::Ticket { ticket },
                            },
                        )
                        .await?;
                        continue;
                    }
                    Operation::Bulk { bytes } => {
                        if bytes > MAX_BULK {
                            send(
                                lines,
                                &error(
                                    Some(id),
                                    ErrorCode::Invalid,
                                    format!("a bulk request carries at most {MAX_BULK} bytes"),
                                ),
                            )
                            .await?;
                            continue;
                        }
                        let ticket =
                            tickets.issue(admission.clone(), Purpose::Upload(uploads.clone()));
                        send(
                            lines,
                            &ServerMessage::Response {
                                id,
                                result: Reply::Ticket { ticket },
                            },
                        )
                        .await?;
                        continue;
                    }
                    Operation::Subscribe { query, generation } => {
                        let (checkpoints, inbox) = mpsc::channel(CHECKPOINTS);
                        let task = tasks.spawn(subscription(
                            id.clone(),
                            query,
                            generation,
                            outgoing.clone(),
                            inbox,
                        ));
                        Inflight {
                            task,
                            checkpoints: Some(checkpoints),
                        }
                    }
                    operation => {
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
            ClientMessage::Checkpoint { id, frame } => {
                match inflight
                    .get(&id)
                    .and_then(|entry| entry.checkpoints.as_ref())
                {
                    Some(checkpoints) => {
                        if checkpoints.try_send(frame).is_err() {
                            // Ending the subscription tells the client to reopen it
                            // for a complete reading rather than drop a checkpoint.
                            if let Some(entry) = inflight.remove(&id) {
                                entry.task.abort();
                            }
                            send(
                                lines,
                                &error(
                                    Some(id),
                                    ErrorCode::Unavailable,
                                    "subscription fell behind its checkpoints; subscribe again",
                                ),
                            )
                            .await?;
                        }
                    }
                    None => {
                        send(
                            lines,
                            &error(
                                Some(id),
                                ErrorCode::Invalid,
                                "no live subscription has this ID",
                            ),
                        )
                        .await?
                    }
                }
            }
            ClientMessage::Cancel { id } => {
                // An answered request has nothing left to forget.
                if let Some(entry) = inflight.remove(&id) {
                    entry.task.abort();
                }
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
        }
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
            &error(
                None,
                ErrorCode::Invalid,
                "ticket is unknown, expired or was issued to another connection",
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
                    send(carrier, &error(None, ErrorCode::Unavailable, format!(
                            "Session owner is unavailable; no provider restarted and no input replayed: {failure}"
                        ))).await?;
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
                .context("no bulk request in time")?
                .context("closed before its bulk request")?
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
                            "a bulk connection carries one request",
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
            &error(
                Some(id),
                ErrorCode::Unavailable,
                format!("answer exceeds {MAX_BULK} bytes"),
            ),
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
    // Owner-protocol bytes the client pipelined behind its attach request.
    if !parts.read_buf.is_empty() {
        destination.write_all(&parts.read_buf).await?;
    }
    if let Err(error) = tokio::io::copy_bidirectional(&mut socket, &mut destination).await {
        // Once bridged, a JSON error would corrupt the owner stream: just close.
        tracing::debug!(%error, "Server owner bridge disconnected");
    }
    Ok(())
}

/// Why a request was not answered with a `Reply`.
struct Refusal {
    code: ErrorCode,
    message: String,
}

impl From<anyhow::Error> for Refusal {
    fn from(error: anyhow::Error) -> Self {
        Self {
            code: ErrorCode::Unavailable,
            message: error.to_string(),
        }
    }
}

async fn request(
    id: String,
    operation: Operation,
    instance: Arc<str>,
    outgoing: mpsc::Sender<ServerMessage>,
    remote_control: Option<mpsc::Sender<Control>>,
) -> String {
    let started = Instant::now();
    let result = operate(operation, &id, &instance, remote_control).await;
    tracing::debug!(request = %id, %instance, elapsed_ms = started.elapsed().as_millis(), ok = result.is_ok(), "Server request ended");
    let message = match result {
        Ok(result) => ServerMessage::Response {
            id: id.clone(),
            result,
        },
        Err(refusal) => {
            tracing::warn!(request = %id, %instance, error = %refusal.message, "Server request failed");
            error(Some(id.clone()), refusal.code, refusal.message)
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
    remote_control: Option<mpsc::Sender<Control>>,
) -> Result<Reply, Refusal> {
    Ok(match operation {
        Operation::MobileStatus => {
            let (reply, result) = tokio::sync::oneshot::channel();
            mobile_control(remote_control, Control::Status { reply }).await?;
            Reply::MobileStatus {
                status: result.await.map_err(anyhow::Error::from)?,
            }
        }
        Operation::ConfigureRemote { configuration } => {
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
                .map_err(anyhow::Error::from)?
                .map_err(anyhow::Error::msg)?;
            Reply::Remote { configuration }
        }
        Operation::SessionOwner { session } => {
            let owner = tokio::task::spawn_blocking(move || -> Result<_> {
                let registry = crate::store::read_existing_registry()?
                    .context("Machine registry is unavailable")?;
                Ok(registry.sqlite.session_transport(session.as_str())?)
            })
            .await
            .map_err(anyhow::Error::from)??;
            Reply::SessionOwner { owner }
        }
        Operation::Read { query } => Reply::Read {
            value: tokio::task::spawn_blocking(move || {
                crate::journal::request::isolate(|| query.read())
            })
            .await
            .map_err(anyhow::Error::from)??,
        },
        Operation::Configuration { request } => {
            let mut target = serde_json::to_value(&request).map_err(anyhow::Error::from)?;
            target
                .as_object_mut()
                .expect("configuration request is an object")
                .remove("value");
            execute_command(target, id, instance, true, move |output, _errors| {
                use std::io::Write;
                writeln!(output, "{}", crate::config::api::run(request)?)?;
                Ok(())
            })
            .await
            .map_err(outcome_unknown)?
        }
        Operation::Command { change } if change.delegates() => {
            let recovery = change_recovery(&change);
            delegate(change, id, instance)
                .await
                .map_err(|error| Refusal {
                    code: ErrorCode::OutcomeUnknown,
                    message: format!("{error}; {recovery}"),
                })?
        }
        Operation::Command { change } => command(change, id, instance)
            .await
            .map_err(outcome_unknown)?,
        Operation::Subscribe { .. } | Operation::Attach { .. } | Operation::Bulk { .. } => {
            unreachable!("the session dispatches subscriptions and tickets")
        }
    })
}

/// Mobile setup belongs to the Machine's own user: the private socket only.
async fn mobile_control(
    remote_control: Option<mpsc::Sender<Control>>,
    request: Control,
) -> Result<(), Refusal> {
    let Some(control) = remote_control else {
        return Err(Refusal {
            code: ErrorCode::Invalid,
            message: "Mobile access is configured on the Machine itself".into(),
        });
    };
    control
        .send(request)
        .await
        .map_err(|_| anyhow::anyhow!("mobile listener is not running"))?;
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
async fn delegate(change: Write, id: &str, instance: &str) -> Result<Reply> {
    let executable = crate::os_process::resolve_lf_binary()?;
    let mut command = tokio::process::Command::new(&executable);
    // A service has no caller Session/Task. Never lend its launcher's identity
    // to requests, or change process-global environment while serving clients.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            command.env_remove(key);
        }
    }
    let trace_id = crate::id::TraceId::default();
    command.env("LF_TRACE_ID", trace_id.as_str());
    command.env("LF_HOME", crate::store::lf_home_dir());
    command.env("LF_BIN", executable);
    command
        .args(change.argv())
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
    let code = status?.code().unwrap_or(1);
    let output = output?;
    Ok(Reply::Command {
        trace_id,
        code,
        value: serde_json::from_str(&output).ok(),
        output,
        stderr: errors?,
    })
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

fn outcome_unknown(error: anyhow::Error) -> Refusal {
    Refusal {
        code: ErrorCode::OutcomeUnknown,
        message: format!("{error}; inspect the affected work before retrying"),
    }
}

async fn subscription(
    id: String,
    query: Read,
    generation: u64,
    outgoing: mpsc::Sender<ServerMessage>,
    checkpoints: mpsc::Receiver<serde_json::Value>,
) -> String {
    if let Err(failure) = watch(&id, query, generation, &outgoing, checkpoints).await {
        // The error ends the subscription for the client; its last reading
        // already carried the watch's failure.
        let _ = outgoing
            .send(error(
                Some(id.clone()),
                ErrorCode::Unavailable,
                failure.to_string(),
            ))
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
    generation: u64,
    outgoing: &mpsc::Sender<ServerMessage>,
    mut checkpoints: mpsc::Receiver<serde_json::Value>,
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
            result: Reply::Subscribed,
        },
    )
    .await?;
    loop {
        tokio::select! {
            frame = checkpoints.recv() => {
                let Some(frame) = frame else { return Ok(()) };
                // The shared watch owns checkpoint parsing and its final error
                // reading, including argument validation and request limits.
                watch.request(&serde_json::to_vec(&frame)?);
            }
            () = notify.notified() => {
                let pull = Arc::clone(&watch);
                let frame = tokio::task::spawn_blocking(move || crate::journal::request::isolate(|| pull.pull())).await?;
                let mut reason = None;
                if let Some(frame) = frame {
                    let frame: serde_json::Value = serde_json::from_slice(&frame)?;
                    reason = frame["unavailable"].as_str().map(str::to_owned);
                    deliver(outgoing, ServerMessage::Reading { id: id.into(), generation, frame }).await?;
                }
                if watch.ended() {
                    anyhow::bail!("{}", reason.unwrap_or_else(|| "the watch ended".into()));
                }
            }
        }
    }
}

async fn command(change: Write, id: &str, instance: &str) -> Result<Reply> {
    use clap::Parser;
    let mut target = serde_json::to_value(&change)?;
    if let Some(fields) = target.as_object_mut() {
        for field in ["content", "notes", "plan", "layout", "slots", "expression"] {
            fields.remove(field);
        }
    }
    let args: Vec<_> = std::iter::once("lf".to_owned())
        .chain(change.argv())
        .collect();
    let json = args.iter().any(|arg| arg == "--json");
    execute_command(target, id, instance, json, move |output, errors| {
        let cli = match crate::lf::Cli::try_parse_from(&args).and_then(crate::lf::Cli::checked) {
            Ok(cli) => cli,
            Err(error) => {
                use std::io::Write;
                let stream = if error.use_stderr() {
                    &mut *errors
                } else {
                    &mut *output
                };
                write!(stream, "{error}")?;
                return Err(crate::process::CommandExit(
                    u8::try_from(error.exit_code()).expect("Clap exit code fits a byte"),
                )
                .into());
            }
        };
        anyhow::ensure!(
            crate::lf::commands::mutation::accepts(&cli),
            "unsupported mutation"
        );
        let mut input = change.input().unwrap_or_default().as_bytes();
        crate::lf::commands::mutation::run(
            &cli,
            &args,
            change.directory(),
            &mut crate::lf::commands::mutation::CommandIo {
                input: &mut input,
                output,
                error: errors,
            },
        )
    })
    .await
}

/// `json`: the command's spelling prints JSON, so stdout that parses becomes `value`.
async fn execute_command(
    target: serde_json::Value,
    id: &str,
    instance: &str,
    json: bool,
    execute: impl FnOnce(&mut Vec<u8>, &mut Vec<u8>) -> Result<()> + Send + 'static,
) -> Result<Reply> {
    let id = id.to_owned();
    let instance = instance.to_owned();
    // Once admitted, disconnection releases the viewer; effects finish without replay.
    tokio::task::spawn_blocking(move || {
        let trace_id = crate::id::TraceId::default();
        let mut output = Vec::new();
        let mut errors = Vec::new();
        let result =
            crate::journal::request::run(&trace_id, &target, || execute(&mut output, &mut errors));
        let code = i32::from(crate::journal::command_exit_code(&result));
        if let Err(error) = result {
            use std::io::Write;
            if !error.is::<crate::process::CommandExit>() {
                writeln!(errors, "Error: {error:?}")?;
            }
        }
        tracing::info!(request = %id, %instance, code, %trace_id, "Server command finished");
        anyhow::ensure!(
            output.len() < MAX_BULK / 2 && errors.len() < MAX_BULK / 2,
            "command output exceeded its limit"
        );
        let value = json.then(|| serde_json::from_slice(&output).ok()).flatten();
        Ok(Reply::Command {
            trace_id,
            code,
            value,
            output: String::from_utf8(output)?,
            stderr: String::from_utf8(errors)?,
        })
    })
    .await?
}

fn error(id: Option<String>, code: ErrorCode, message: impl Into<String>) -> ServerMessage {
    ServerMessage::Error {
        id,
        code,
        message: message.into(),
    }
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

    use super::{connection, deliver_or_spill};
    use crate::id::LfProcessId;
    use crate::lf::commands::read::Read;
    use crate::server::ticket::{Admission, Tickets};
    use crate::server::wire::{
        ClientIdentity, ClientMessage, ErrorCode, Hello, Operation, Reply, ServerMessage, MAX_BULK,
        MAX_MESSAGE, REVISION,
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
            Self::connect_with(Default::default())
        }

        /// A connection whose tickets another connection can redeem.
        fn connect_with(tickets: Arc<Tickets>) -> Self {
            let (client, server) = UnixStream::pair().unwrap();
            let server = tokio::spawn(async move {
                connection(server, "incarnation", "fixture", None, tickets).await
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
            let mut connection = Self::connect_with(tickets);
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
        let ServerMessage::Response {
            id: of,
            result: Reply::Ticket { ticket },
        } = client.receive().await.unwrap()
        else {
            panic!("expected a ticket for {id}")
        };
        assert_eq!(of, id);
        ticket
    }

    #[test]
    fn tickets_carry_bulk_requests_and_large_answers_off_the_control_connection() {
        with_empty_home(async {
            let tickets = Arc::new(Tickets::default());
            let mut client = Client::connect_with(tickets.clone());
            client.hello(REVISION, REVISION).await;
            // A request over the control limit travels on its own connection and
            // is answered on the control connection under its own id.
            client
                .send(&ClientMessage::Request {
                    id: "u".into(),
                    operation: Operation::Bulk { bytes: 200 },
                })
                .await;
            let ticket = ticket_reply(&mut client, "u").await;
            let mut bulk = Client::redeem(tickets.clone(), &ticket).await;
            bulk.send(&ClientMessage::Request {
                id: "r".into(),
                operation: catalog(),
            })
            .await;
            let ServerMessage::Response {
                id,
                result: Reply::Command { code, .. },
            } = client.receive().await.unwrap()
            else {
                panic!("expected the uploaded request's answer")
            };
            assert_eq!((id.as_str(), code), ("r", 0));
            bulk.ended().await;
            // A ticket is single use.
            let mut again = Client::connect_with(tickets.clone());
            again.send(&ClientMessage::Ticket { ticket }).await;
            assert!(matches!(
                again.receive().await.unwrap(),
                ServerMessage::Error {
                    id: None,
                    code: ErrorCode::Invalid,
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
                    result: Reply::Read {
                        value: value.clone(),
                    },
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
                result: Reply::Read { value: fetched },
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
            let owner = LfProcessId::new();
            let listener =
                tokio::net::UnixListener::bind(crate::transport::endpoint(&owner).unwrap())
                    .unwrap();
            let mut client = Client::connect_with(tickets.clone());
            client.hello(REVISION, REVISION).await;
            client
                .send(&ClientMessage::Request {
                    id: "t".into(),
                    operation: Operation::Attach {
                        owner: owner.clone(),
                    },
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
            // An owner that ended between ticket and redemption fails explicitly.
            drop(listener);
            client
                .send(&ClientMessage::Request {
                    id: "gone".into(),
                    operation: Operation::Attach { owner },
                })
                .await;
            let ticket = ticket_reply(&mut client, "gone").await;
            let mut late = Client::connect_with(tickets.clone());
            late.send(&ClientMessage::Ticket { ticket }).await;
            assert!(matches!(
                late.receive().await.unwrap(),
                ServerMessage::Error {
                    id: None,
                    code: ErrorCode::Unavailable,
                    ..
                }
            ));
            late.ended().await;
            drop(client.writer);
            client.server.await.unwrap().unwrap();
        });
    }

    fn catalog() -> Operation {
        Operation::Configuration {
            request: crate::config::api::Request::Catalog,
        }
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
                let ServerMessage::Response {
                    id,
                    result: Reply::Command { code, value, .. },
                } = client.receive().await.unwrap()
                else {
                    panic!("expected a command result")
                };
                assert_eq!(code, 0);
                assert!(value.unwrap().is_array(), "catalog parses as JSON");
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
            let ServerMessage::Error { id, code, message } =
                client.hello(revision, min_revision).await
            else {
                panic!("expected an error")
            };
            assert_eq!(id, None);
            assert_eq!(code, ErrorCode::Incompatible);
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
            connection(server, "incarnation", "fixture", None, Default::default()).await
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
                .send(&ClientMessage::Request {
                    id: "ps".into(),
                    operation: Operation::Subscribe {
                        query: Read::MonitorPs,
                        generation: 4,
                    },
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, result: Reply::Subscribed } if id == "ps"
            ));
            let ServerMessage::Reading {
                id,
                generation,
                frame,
            } = client.receive().await.unwrap()
            else {
                panic!("expected the first reading")
            };
            assert_eq!((id.as_str(), generation), ("ps", 4));
            assert!(frame["unavailable"].is_null(), "{frame}");
            client
                .send(&ClientMessage::Checkpoint {
                    id: "ps".into(),
                    frame: serde_json::json!({"id": 3}),
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
                ServerMessage::Response { id, result: Reply::Command { .. } } if id == "after"
            ));
            // A cancelled subscription takes no more checkpoints.
            client
                .send(&ClientMessage::Checkpoint {
                    id: "ps".into(),
                    frame: serde_json::json!({"id": 5}),
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Error { id: Some(id), code: ErrorCode::Invalid, .. } if id == "ps"
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
    async fn an_overlong_line_ends_the_connection() {
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
        } = client.receive().await.unwrap()
        else {
            panic!("expected the control limit")
        };
        assert!(message.contains("bulk ticket"), "{message}");
        client.ended().await;
    }
}
