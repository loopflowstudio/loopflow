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
use tokio::sync::{mpsc, Notify, Semaphore};
use tokio::task::{AbortHandle, JoinSet};
use tokio_util::codec::{Framed, LinesCodec};

use super::wire::{
    ClientIdentity, ClientMessage, ConnectedClient, ErrorCode, Hello, Limits, Operation, Reply,
    ServerMessage, Status, Welcome, MAX_CONNECTIONS, MAX_MESSAGE, MIN_REVISION, REVISION,
};
use crate::lf::commands::{read::Read, watch::Subscription, write::Write};

/// How long a connection has to send its first message.
const FIRST_MESSAGE: Duration = Duration::from_secs(5);
/// How long a client has to take a message before its connection ends.
const SEND: Duration = Duration::from_secs(5);
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
    "owner_bridge",
    "session_owner",
    "desktop_sessions",
    "view_plan",
];

type Lines = Framed<UnixStream, LinesCodec>;

/// Welcomed connections by client ID, as `lf server status` lists them.
type Clients = Arc<Mutex<HashMap<String, ConnectedClient>>>;

/// A welcomed connection's row, removed when the connection ends.
struct Admitted {
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
    tracing::info!(%instance, socket = %path.display(), "Loopflow server ready");
    loop {
        let (socket, _) = listener.accept().await?;
        // Unix admission is deliberately unavailable to remote proxies.
        // SAFETY: geteuid has no preconditions.
        if socket.peer_cred()?.uid() != unsafe { libc::geteuid() } {
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
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(error) = connection(socket, &instance, &executable_sha256, &clients).await {
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

async fn connection(
    mut socket: UnixStream,
    instance: &str,
    executable_sha256: &str,
    clients: &Clients,
) -> Result<()> {
    let first = tokio::time::timeout(FIRST_MESSAGE, socket.read_u8())
        .await
        .context("no message within five seconds")??;
    // A revision-1 request begins with its four-byte length; JSON begins with
    // `{`. Delete this branch with revision 1.
    if first < 0x20 {
        return refuse_revision_one(socket, instance).await;
    }
    let mut lines = Framed::new(socket, LinesCodec::new_with_max_length(MAX_MESSAGE));
    lines.read_buffer_mut().extend_from_slice(&[first]);
    let line = tokio::time::timeout(FIRST_MESSAGE, lines.next())
        .await
        .context("no message within five seconds")?
        .context("closed before its first message")??;
    let hello = match serde_json::from_str::<ClientMessage>(&line) {
        Ok(ClientMessage::Hello(hello)) => hello,
        Ok(ClientMessage::Status) => {
            return send(
                &mut lines,
                &ServerMessage::Status(Status {
                    revision: REVISION,
                    min_revision: MIN_REVISION,
                    instance: instance.into(),
                    executable_sha256: executable_sha256.into(),
                    clients: connected(clients),
                }),
            )
            .await;
        }
        Ok(_) => {
            return send(
                &mut lines,
                &error(None, ErrorCode::Invalid, "the first message must be hello"),
            )
            .await;
        }
        Err(parse) => {
            return send(
                &mut lines,
                &error(None, ErrorCode::Invalid, parse.to_string()),
            )
            .await;
        }
    };
    let Some(revision) = negotiate(&hello) else {
        let side = if hello.min_revision > REVISION {
            "selected installation"
        } else {
            "client"
        };
        return send(
            &mut lines,
            &error(
                None,
                ErrorCode::Incompatible,
                format!(
                    "Server speaks protocol revisions {MIN_REVISION}–{REVISION}; the client speaks {}–{}; update the {side}",
                    hello.min_revision, hello.revision
                ),
            ),
        )
        .await;
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
            capabilities: CAPABILITIES.iter().map(|name| (*name).to_owned()).collect(),
            limits: Limits {
                max_message: MAX_MESSAGE,
                max_connections: MAX_CONNECTIONS,
            },
        }),
    )
    .await?;
    session(lines, instance.into(), &admitted).await
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

async fn session(mut lines: Lines, instance: Arc<str>, admitted: &Admitted) -> Result<()> {
    let (outgoing, mut queue) = mpsc::channel::<ServerMessage>(QUEUE);
    // Dropping the set aborts every answer still pending; admitted blocking
    // work finishes on its worker. A task returns its request ID.
    let mut tasks: JoinSet<String> = JoinSet::new();
    let mut inflight: HashMap<String, Inflight> = HashMap::new();
    loop {
        tokio::select! {
            incoming = lines.next() => {
                let Some(line) = incoming else { return Ok(()) };
                let message = match serde_json::from_str::<ClientMessage>(&line?) {
                    Ok(message) => message,
                    Err(parse) => {
                        // A null-id error is the connection's last message.
                        send(&mut lines, &error(None, ErrorCode::Invalid, parse.to_string())).await?;
                        return Ok(());
                    }
                };
                match message {
                    ClientMessage::Request { id, operation } => {
                        admitted.requested();
                        if id.is_empty() || id.len() > 128 {
                            send(&mut lines, &error(Some(id), ErrorCode::Invalid, "request ID must contain 1–128 bytes")).await?;
                            continue;
                        }
                        if inflight.contains_key(&id) {
                            send(&mut lines, &error(Some(id), ErrorCode::Invalid, "request ID is still unanswered")).await?;
                            continue;
                        }
                        let entry = match operation {
                            Operation::Attach { owner } => {
                                let destination = match UnixStream::connect(crate::transport::endpoint(&owner)?).await {
                                    Ok(destination) => destination,
                                    Err(failure) => {
                                        send(&mut lines, &error(Some(id), ErrorCode::Unavailable, format!(
                                            "Session owner is unavailable; no provider restarted and no input replayed: {failure}"
                                        ))).await?;
                                        continue;
                                    }
                                };
                                send(&mut lines, &ServerMessage::Response { id, result: Reply::Bridged }).await?;
                                // The connection now speaks only the owner protocol; other
                                // answers on it are dropped with the task set.
                                return bridge(lines, destination).await;
                            }
                            Operation::Subscribe { query } => {
                                let (checkpoints, inbox) = mpsc::channel(CHECKPOINTS);
                                let task = tasks.spawn(subscription(id.clone(), query, outgoing.clone(), inbox));
                                Inflight { task, checkpoints: Some(checkpoints) }
                            }
                            operation => {
                                let task = tasks.spawn(request(id.clone(), operation, instance.clone(), outgoing.clone()));
                                Inflight { task, checkpoints: None }
                            }
                        };
                        inflight.insert(id, entry);
                    }
                    ClientMessage::Checkpoint { id, checkpoint } => {
                        match inflight.get(&id).and_then(|entry| entry.checkpoints.as_ref()) {
                            Some(checkpoints) => {
                                if checkpoints.try_send(checkpoint).is_err() {
                                    // Ending the subscription tells the client to reopen it
                                    // for a complete reading rather than drop a checkpoint.
                                    if let Some(entry) = inflight.remove(&id) { entry.task.abort(); }
                                    send(&mut lines, &error(Some(id), ErrorCode::Unavailable, "subscription fell behind its checkpoints; subscribe again")).await?;
                                }
                            }
                            None => send(&mut lines, &error(Some(id), ErrorCode::Invalid, "no live subscription has this ID")).await?,
                        }
                    }
                    ClientMessage::Cancel { id } => {
                        // An answered request has nothing left to forget.
                        if let Some(entry) = inflight.remove(&id) {
                            entry.task.abort();
                        }
                    }
                    ClientMessage::Hello(_) | ClientMessage::Status => {
                        send(&mut lines, &error(None, ErrorCode::Invalid, "hello was already exchanged")).await?;
                        return Ok(());
                    }
                }
            }
            Some(message) = queue.recv() => send(&mut lines, &message).await?,
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
            }
        }
    }
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
) -> String {
    let started = Instant::now();
    let result = operate(operation, &id, &instance).await;
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

async fn operate(operation: Operation, id: &str, instance: &str) -> Result<Reply, Refusal> {
    Ok(match operation {
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
        Operation::Command { change } => command(change, id, instance)
            .await
            .map_err(outcome_unknown)?,
        Operation::Subscribe { .. } | Operation::Attach { .. } => {
            unreachable!("the session dispatches subscriptions and attachment")
        }
    })
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
    outgoing: mpsc::Sender<ServerMessage>,
    checkpoints: mpsc::Receiver<u64>,
) -> String {
    if let Err(failure) = watch(&id, query, &outgoing, checkpoints).await {
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
            result: Reply::Subscribed,
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
            output.len() < MAX_MESSAGE / 2 && errors.len() < MAX_MESSAGE / 2,
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
    use super::{connection, Clients};
    use crate::lf::commands::read::Read;
    use crate::server::wire::{
        ClientIdentity, ClientMessage, ErrorCode, Hello, Operation, Reply, ServerMessage,
        MAX_MESSAGE, REVISION,
    };
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

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
            let (client, server) = UnixStream::pair().unwrap();
            let clients = clients.clone();
            let server = tokio::spawn(async move {
                connection(server, "incarnation", "fixture", &clients).await
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
            connection(server, "incarnation", "fixture", &Clients::default()).await
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
                .send(&ClientMessage::Request {
                    id: "ps".into(),
                    operation: Operation::Subscribe {
                        query: Read::MonitorPs,
                    },
                })
                .await;
            assert!(matches!(
                client.receive().await.unwrap(),
                ServerMessage::Response { id, result: Reply::Subscribed } if id == "ps"
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
                ServerMessage::Response { id, result: Reply::Command { .. } } if id == "after"
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
