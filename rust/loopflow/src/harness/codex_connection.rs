//! lf's side connections to a Codex app-server. The native client talks to the
//! app-server directly: lf opens the conversation, then watches it as one more
//! subscriber. Nothing here carries the person's bytes, so a store failure is a
//! recording gap and never reaches the conversation.

use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio_tungstenite::{
    client_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Error, Message},
    WebSocketStream,
};

use super::codex::CodexStream;
use super::fact_writer::FactWriter;
use super::task_updates::{TaskUpdate, TaskUpdates};
use crate::id::AgentSessionId;
use crate::journal::{process_identity_evidence, ProcessIdentityEvidence};
use crate::process::SessionAttachment;
use crate::provider_auth::Provider;
use crate::store::sqlite::SqliteStore;
use crate::store::{ProviderAccountId, StoreError};

type Upstream = WebSocketStream<UnixStream>;

const REPLY_LIMIT: Duration = Duration::from_secs(15);
/// Starting a thread loads MCP servers and plugins.
const STARTUP_LIMIT: Duration = Duration::from_secs(60);
/// How often a broken store is tried again.
const STORE_RETRY: Duration = Duration::from_secs(5);
/// How often Task updates and the attachment are read.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
const CLIENT_STOP_GRACE: Duration = Duration::from_secs(2);
const REJOIN_INTERVAL: Duration = Duration::from_millis(250);

// Codex sends single frames larger than tungstenite's defaults: plugin/list
// carries the whole curated marketplace (7,000+ plugins, over 16 MiB).
fn connection_config() -> Option<WebSocketConfig> {
    Some(
        WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None),
    )
}

pub(super) async fn connect_upstream(socket: UnixStream) -> Result<Upstream, Error> {
    Ok(
        client_async_with_config("ws://localhost", socket, connection_config())
            .await?
            .0,
    )
}

/// Refuse shutdown if the saved endpoint serves an unrelated conversation.
/// The caller holds the Session attachment lock through inspection and shutdown;
/// this check neither signals the provider nor changes its saved history.
pub(crate) fn validate_agent_process_close(
    endpoint: &str,
    thread: Option<&AgentSessionId>,
) -> Result<()> {
    // Use a separate runtime: exit is also reached from synchronous capture
    // settlement and signal cleanup, sometimes inside an existing runtime.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?
                    .block_on(async {
                        tokio::time::timeout(
                            Duration::from_secs(3),
                            inspect_agent_threads(endpoint, thread),
                        )
                        .await
                        .map_err(|_| anyhow!("AgentProcess inspection timed out"))?
                    })
            })
            .join()
            .map_err(|_| anyhow!("AgentProcess close worker panicked"))?
    })
}

async fn inspect_agent_threads(endpoint: &str, thread: Option<&AgentSessionId>) -> Result<()> {
    let socket = match UnixStream::connect(endpoint).await {
        Ok(socket) => socket,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(())
        }
        Err(error) => return Err(error.into()),
    };
    let mut upstream = connect_upstream(socket).await?;
    rpc_request(
        &mut upstream,
        "initialize",
        json!({"clientInfo": {"name":"loopflow_close", "version":env!("CARGO_PKG_VERSION")}}),
    )
    .await?;
    let mut cursor = Value::Null;
    loop {
        let loaded = rpc_request(
            &mut upstream,
            "thread/loaded/list",
            json!({"cursor":cursor,"limit":100}),
        )
        .await?;
        let threads = loaded["data"]
            .as_array()
            .ok_or_else(|| anyhow!("loaded threads response has no data"))?;
        for id in threads {
            let mut current = id
                .as_str()
                .ok_or_else(|| anyhow!("loaded thread has no ID"))?
                .to_owned();
            let mut ancestors = std::collections::HashSet::new();
            while Some(current.as_str()) != thread.map(AgentSessionId::as_str) {
                if !ancestors.insert(current.clone()) {
                    return Err(anyhow!("provider thread parent cycle"));
                }
                let detail =
                    rpc_request(&mut upstream, "thread/read", json!({"threadId":current})).await?;
                // Codex's own subagents are part of this AgentProcess's work. A
                // separately started conversation must survive this exit.
                current = detail
                    .pointer("/thread/parentThreadId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        anyhow!(
                            "AgentProcess still serves another conversation; leaving it running"
                        )
                    })?
                    .to_owned();
            }
        }
        let next = loaded.get("nextCursor").cloned().unwrap_or(Value::Null);
        if next.is_null() {
            break;
        }
        if next == cursor {
            return Err(anyhow!("loaded threads returned a repeated cursor"));
        }
        cursor = next;
    }
    upstream.close(None).await?;
    Ok(())
}

async fn initialized(endpoint: &Path, name: &str) -> Result<Upstream> {
    let mut upstream = connect_upstream(UnixStream::connect(endpoint).await?).await?;
    rpc_request(
        &mut upstream,
        "initialize",
        json!({
            "clientInfo": {"name":name, "version":env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi":true}
        }),
    )
    .await?;
    upstream
        .send(Message::Text(
            json!({"method":"initialized"}).to_string().into(),
        ))
        .await?;
    Ok(upstream)
}

/// Read all native turn pages before displaying the conversation, and say how
/// many there are. A thread with turns has received its first input, so history
/// settles what a dispatch receipt could only guess at.
pub(crate) async fn recover_history(
    store: &SqliteStore,
    session: &str,
    thread: &AgentSessionId,
    endpoint: &Path,
) -> Result<usize> {
    let mut upstream = initialized(endpoint, "loopflow_history").await?;
    let mut history = CodexStream::new(FactWriter::new(
        store.clone(),
        session.to_owned(),
        None,
        None,
        Default::default(),
    ));
    let mut cursor = Value::Null;
    let mut count = 0;
    loop {
        let result = rpc_request(
            &mut upstream,
            "thread/turns/list",
            json!({
                "threadId":thread, "cursor":cursor, "limit":100,
                "sortDirection":"asc", "itemsView":"notLoaded"
            }),
        )
        .await?;
        count += result["data"]
            .as_array()
            .ok_or_else(|| anyhow!("Native history response has no turns"))?
            .len();
        history
            .inbound(&json!({"result":result}), Some(thread))
            .await
            .1?;
        let next = result.get("nextCursor").cloned().unwrap_or(Value::Null);
        if next.is_null() {
            break;
        }
        if next == cursor {
            return Err(anyhow!("Native history returned a repeated cursor"));
        }
        cursor = next;
    }
    upstream.close(None).await?;
    Ok(count)
}

/// Start or resume the conversation with lf's prepared configuration, so the
/// native client only has to join it. Codex cannot resume a thread with no
/// rollout, and creates one on the first turn or a name (verified on 0.166.0);
/// naming it lets the client attach before the first turn.
pub(crate) async fn open_thread(
    endpoint: &Path,
    params: Value,
    name: &str,
) -> Result<AgentSessionId> {
    let mut upstream = initialized(endpoint, "loopflow_start").await?;
    let mut params = params;
    let thread = match params["threadId"].as_str() {
        Some(thread) => {
            let thread = AgentSessionId::from(thread);
            rpc_within(&mut upstream, "thread/resume", params, STARTUP_LIMIT).await?;
            thread
        }
        None => {
            // What Codex's own terminal asks for. Its `codex_tui` dynamic tools
            // stay out: only the client that declares them can describe them.
            params["threadSource"] = json!("user");
            params["historyMode"] = json!("paginated");
            let started = rpc_within(&mut upstream, "thread/start", params, STARTUP_LIMIT).await?;
            let thread = started
                .pointer("/thread/id")
                .and_then(Value::as_str)
                .map(AgentSessionId::from)
                .ok_or_else(|| anyhow!("Codex returned no native conversation ID"))?;
            rpc_request(
                &mut upstream,
                "thread/name/set",
                json!({"threadId":thread, "name":name}),
            )
            .await?;
            thread
        }
    };
    upstream.close(None).await?;
    Ok(thread)
}

/// The conversation an observer watches and the native client it answers for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexObserver {
    pub(crate) database: PathBuf,
    pub(crate) session: String,
    pub(crate) attachment: SessionAttachment,
    pub(crate) endpoint: PathBuf,
    pub(crate) thread: AgentSessionId,
    pub(crate) account: Option<ProviderAccountId>,
    pub(crate) isolate: Option<bool>,
    /// PID and birth of the native client this attachment opened.
    pub(crate) client: (u32, i64),
    /// The first turn, submitted once this observer can record it.
    pub(crate) input: Option<String>,
}

/// Start the observer beside the native client, subscribed before the first
/// turn. It outlives this process and never holds the provider's lifeline.
pub(crate) fn spawn_observer(observer: &CodexObserver) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("__codex-observe")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: setsid is async-signal-safe and touches only the new child. Its
    // own session keeps the terminal's hangup and job control away.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let mut stdin = child.stdin.take().context("observer has no stdin")?;
    stdin.write_all(&serde_json::to_vec(observer)?)?;
    drop(stdin);
    // One byte means subscribed; EOF means it gave up. Both let the client open.
    let mut stdout = child.stdout.take().context("observer has no stdout")?;
    let mut ready = [0];
    stdout
        .read_exact(&mut ready)
        .context("observer stopped before subscribing")
}

/// `lf __codex-observe`: read the launch from stdin and watch until the
/// app-server goes away or the conversation moves.
pub fn run_observer() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let observer: CodexObserver = serde_json::from_str(&input)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let result = runtime.block_on(observe(observer, || {
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(b".").and_then(|()| stdout.flush());
    }));
    if let Err(error) = &result {
        tracing::warn!(error = %format!("{error:#}"), "Codex observer stopped");
    }
    result
}

/// Runs as long as its client does. A server can go before the store records why,
/// so losing it says nothing about the client: a mover may still end it.
async fn observe(observer: CodexObserver, ready: impl FnOnce()) -> Result<()> {
    let mut recorder = Recorder::new(observer).await;
    let mut ready = Some(ready);
    loop {
        // A hung app-server accepts the socket and never shakes hands.
        let subscribed = tokio::time::timeout(REPLY_LIMIT, recorder.subscribe())
            .await
            .unwrap_or_else(|_| Err(anyhow!("Codex app-server did not answer the observer")));
        match subscribed {
            Ok(mut upstream) => {
                if let Err(error) = recorder.submit_input(&mut upstream).await {
                    tracing::warn!(error = %format!("{error:#}"), "Codex first turn was not sent");
                }
                if let Some(ready) = ready.take() {
                    ready();
                }
                recorder.watch(upstream).await;
            }
            Err(error) if ready.is_some() => return Err(error),
            Err(error) => tracing::debug!(error = %format!("{error:#}"), "Codex app-server closed"),
        }
        if !recorder.client_alive() {
            return Ok(());
        }
        if recorder.attachment_moved() {
            recorder.stop_client().await;
            return Ok(());
        }
        tokio::time::sleep(REJOIN_INTERVAL).await;
    }
}

struct Recorder {
    observer: CodexObserver,
    route: Option<crate::provider_account::ProviderAccountRoute>,
    store: Option<(SqliteStore, CodexStream)>,
    retry_at: Option<Instant>,
    updates: TaskUpdates,
    /// A turn is running, or may be; updates wait for it.
    busy: bool,
    update_request: Option<(Value, TaskUpdate)>,
    /// The person moved to this thread and the store has not recorded it yet.
    selection: Option<&'static str>,
    /// Not yet subscribed to the selected thread on this connection, and the
    /// request that is trying.
    following: bool,
    resume_request: Option<Value>,
}

impl Recorder {
    async fn new(observer: CodexObserver) -> Self {
        let route = match &observer.account {
            Some(account) => crate::provider_account::resolve_provider_account_exact(
                Provider::Codex,
                Some(&observer.thread),
                Some(account),
                None,
                observer.isolate,
            )
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "Codex observer has no account route");
                None
            }),
            None => None,
        };
        Self {
            observer,
            route,
            store: None,
            retry_at: None,
            updates: TaskUpdates::default(),
            busy: true,
            update_request: None,
            selection: None,
            following: false,
            resume_request: None,
        }
    }

    /// The store, reopened at most every `STORE_RETRY` while it stays broken.
    fn stream(&mut self) -> Option<&mut (SqliteStore, CodexStream)> {
        if self.store.is_none() && self.retry_at.is_none_or(|at| at <= Instant::now()) {
            match SqliteStore::new(&self.observer.database) {
                Ok(store) => {
                    let writer = FactWriter::new(
                        store.clone(),
                        self.observer.session.clone(),
                        Some(self.observer.attachment.clone()),
                        self.route.clone(),
                        Default::default(),
                    );
                    self.store = Some((store, CodexStream::new(writer)));
                    self.retry_at = None;
                }
                Err(error) => self.gap(&error),
            }
        }
        self.store.as_mut()
    }

    /// Log a recording gap once per outage. History fills it on the next launch.
    fn gap(&mut self, error: &dyn std::fmt::Display) {
        if self.retry_at.is_none() {
            tracing::warn!(%error, session = self.observer.session, "Codex recording gap");
        }
        self.store = None;
        self.retry_at = Some(Instant::now() + STORE_RETRY);
    }

    async fn subscribe(&mut self) -> Result<Upstream> {
        let mut upstream = initialized(&self.observer.endpoint, "loopflow_observer").await?;
        let thread = self.observer.thread.clone();
        let result =
            rpc_request(&mut upstream, "thread/resume", json!({"threadId":thread})).await?;
        self.busy = turn_running(&result);
        (self.following, self.resume_request) = (false, None);
        self.record(&json!({"result":result})).await;
        if let Some((store, _)) = self.stream() {
            let store = store.clone();
            // The first read is a baseline: updates report what changes from here.
            if let Err(error) = self.pending_update(&store) {
                self.gap(&error);
            }
        }
        Ok(upstream)
    }

    /// Provider history decides on the next launch whether this was received,
    /// so nothing here needs a receipt, and a broken store cannot stop the turn.
    async fn submit_input(&mut self, upstream: &mut Upstream) -> Result<()> {
        let Some(text) = self.observer.input.take() else {
            return Ok(());
        };
        let request = json!({
            "id":format!("lf-initial-{}", uuid::Uuid::new_v4()),"method":"turn/start",
            "params":{"threadId":self.observer.thread,"input":[{"type":"text","text":text}]}
        });
        if let Some((_, history)) = self.stream() {
            if let Err(error) = history.outbound(&request).await {
                self.gap(&format!("{error:#}"));
            }
        }
        self.busy = true;
        upstream
            .send(Message::Text(request.to_string().into()))
            .await?;
        Ok(())
    }

    async fn record(&mut self, rpc: &Value) {
        // Selection first: the decoder would otherwise set the thread with no
        // history and leave the resume reference behind.
        self.select();
        let thread = self.observer.thread.clone();
        let Some((_, history)) = self.stream() else {
            return;
        };
        if let Err(error) = history.inbound(rpc, Some(&thread)).await.1 {
            self.gap(&format!("{error:#}"));
        }
    }

    fn pending_update(&mut self, store: &SqliteStore) -> Result<Option<TaskUpdate>, StoreError> {
        let (session, attachment) = (&self.observer.session, &self.observer.attachment);
        let updates = &mut self.updates;
        match store.with_session_attachment(session, attachment, || updates.pending(store, session))
        {
            Err(StoreError::InvalidAuthority(_)) => Ok(None),
            result => result,
        }
    }

    /// Until the connection closes or the attachment moves. Reading runs apart
    /// from recording, so a slow store never backs up app-server's queue for
    /// this connection.
    async fn watch(&mut self, upstream: Upstream) {
        let (mut writer, mut reader) = upstream.split();
        let (messages, mut incoming) = tokio::sync::mpsc::unbounded_channel();
        let reading = tokio::spawn(async move {
            while let Some(Ok(message)) = reader.next().await {
                if let Message::Text(text) = message {
                    if messages.send(text).is_err() {
                        break;
                    }
                }
            }
        });
        let mut clock = tokio::time::interval(WATCH_INTERVAL);
        loop {
            tokio::select! {
                message = incoming.recv() => {
                    let Some(text) = message else { break };
                    match serde_json::from_str::<Value>(&text) {
                        Ok(rpc) => self.observe_message(&rpc, &mut writer).await,
                        Err(error) => tracing::warn!(%error, "undecodable Codex message"),
                    }
                }
                _ = clock.tick() => {
                    if self.attachment_moved() {
                        break;
                    }
                    self.select();
                    if self.following && self.resume_request.is_none() {
                        self.resume(&mut writer).await;
                    }
                    if !self.busy && self.update_request.is_none() {
                        self.deliver_update(&mut writer).await;
                    }
                }
            }
        }
        reading.abort();
    }

    async fn observe_message(&mut self, rpc: &Value, writer: &mut SplitSink<Upstream, Message>) {
        if rpc["method"] == "thread/started" {
            if let Some((thread, source)) =
                person_started(rpc).filter(|(thread, _)| *thread != self.observer.thread)
            {
                self.follow(thread, source, writer).await;
            }
            return;
        }
        if self.resume_request.is_some() && rpc.get("id") == self.resume_request.as_ref() {
            self.resume_request = None;
            match rpc.get("result") {
                Some(result) => {
                    self.following = false;
                    self.busy = turn_running(result);
                }
                // A thread has no rollout to resume until its first turn; the
                // next tick asks again, and history fills what came before.
                None => tracing::debug!(error = %rpc["error"], "Codex thread not yet resumable"),
            }
        }
        let params = &rpc["params"];
        let ours = params["threadId"].as_str() == Some(self.observer.thread.as_str());
        match rpc["method"].as_str() {
            Some("turn/started") if ours => {
                self.busy = true;
                // Only our update has a request to answer for it; every other
                // turn came from the person's client.
                if self.update_request.is_none() {
                    if let (Some(turn), Some((_, history))) = (
                        params.pointer("/turn/id").and_then(Value::as_str),
                        self.stream(),
                    ) {
                        history.decoder.client_turn(turn);
                    }
                }
            }
            Some("turn/completed") if ours => self.busy = false,
            _ => {}
        }
        self.record(rpc).await;
        let answered = self
            .update_request
            .as_ref()
            .is_some_and(|(id, _)| rpc.get("id") == Some(id));
        if answered {
            let (_, update) = self.update_request.take().expect("matched update request");
            let state = if rpc.get("error").is_some() {
                self.busy = false;
                "rejected"
            } else {
                "accepted"
            };
            let session = self.observer.session.clone();
            if let Some((store, _)) = self.stream() {
                if let Err(error) =
                    store.record_task_update(&session, &update.key, state, &update.observation)
                {
                    self.gap(&error);
                }
            }
        }
    }

    /// The person started or forked a conversation in their client. The Session
    /// follows it, as Claude's `/clear` does, and the earlier one stays in its
    /// selection history.
    async fn follow(
        &mut self,
        thread: AgentSessionId,
        source: &'static str,
        writer: &mut SplitSink<Upstream, Message>,
    ) {
        tracing::info!(
            session = self.observer.session,
            from = %self.observer.thread,
            to = %thread,
            source,
            "Codex conversation changed; following it"
        );
        // Bound before the person's first turn there can reach a hook.
        if let Err(error) = crate::context_block::follow_codex_session(
            &crate::store::lf_home_dir(),
            self.observer.thread.as_str(),
            thread.as_str(),
        ) {
            tracing::warn!(%error, "Codex conversation's hooks have no lf context");
        }
        self.observer.thread = thread;
        self.selection = Some(source);
        self.select();
        (self.busy, self.following) = (true, true);
        self.resume(writer).await;
    }

    async fn resume(&mut self, writer: &mut SplitSink<Upstream, Message>) {
        let id = json!(format!("lf-follow-{}", uuid::Uuid::new_v4()));
        let request =
            json!({"id":id,"method":"thread/resume","params":{"threadId":self.observer.thread}});
        match writer.send(Message::Text(request.to_string().into())).await {
            Ok(()) => self.resume_request = Some(id),
            // The connection is gone; the rejoin subscribes to this thread.
            Err(error) => tracing::debug!(%error, "Codex observer could not resume"),
        }
    }

    /// Record the followed thread as the Session's AgentSession. A lost
    /// attachment has nothing to select; a broken store tries again later.
    fn select(&mut self) {
        let Some(source) = self.selection else {
            return;
        };
        let (session, attachment) = (
            self.observer.session.clone(),
            self.observer.attachment.clone(),
        );
        let thread = self.observer.thread.clone();
        let Some((store, _)) = self.stream() else {
            return;
        };
        let store = store.clone();
        let selected = store
            .reserve_steer_adapter(&session, &attachment)
            .and_then(|adapter| {
                store.with_provider_context(
                    &adapter,
                    thread.as_str(),
                    crate::steer::NativeBoundary::Select { source },
                    || Ok(()),
                )
            });
        match selected {
            Ok(()) => self.selection = None,
            Err(StoreError::InvalidAuthority(error)) => {
                tracing::debug!(%error, "Codex conversation change has no attachment to select");
                self.selection = None;
            }
            Err(error) => self.gap(&error),
        }
    }

    /// A store that cannot say keeps the client: only a recorded move ends it.
    fn attachment_moved(&mut self) -> bool {
        let session = self.observer.session.clone();
        let Some((store, _)) = self.stream() else {
            return false;
        };
        match store.session_attachment(&session) {
            Ok(current) => current.as_ref() != Some(&self.observer.attachment),
            Err(error) => {
                self.gap(&error);
                false
            }
        }
    }

    fn client_alive(&self) -> bool {
        let (pid, born) = self.observer.client;
        process_identity_evidence(pid, born) == ProcessIdentityEvidence::Live
    }

    /// End the exact client this attachment opened. It would otherwise keep
    /// writing to a conversation that lives elsewhere now.
    async fn stop_client(&self) {
        tracing::info!(
            session = self.observer.session,
            "attachment ended elsewhere; closing its client"
        );
        let signal = |signal| {
            if self.client_alive() {
                let pid = self.observer.client.0;
                // SAFETY: kill reads no memory; identity was just checked by PID
                // and birth, and a single PID is signalled, never a group.
                if unsafe { libc::kill(pid as libc::pid_t, signal) } != 0 {
                    tracing::warn!(error = %std::io::Error::last_os_error(), "cannot signal the client");
                }
            }
        };
        signal(libc::SIGTERM);
        tokio::time::sleep(CLIENT_STOP_GRACE).await;
        signal(libc::SIGKILL);
    }

    async fn deliver_update(&mut self, writer: &mut SplitSink<Upstream, Message>) {
        let Some((store, _)) = self.stream() else {
            return;
        };
        let store = store.clone();
        let update = match self.pending_update(&store) {
            Ok(Some(update)) => update,
            Ok(None) => return,
            Err(error) => {
                self.gap(&error);
                return;
            }
        };
        let id = json!(format!("lf-task-update-{}", uuid::Uuid::new_v4()));
        let request = json!({"id":id,"method":"turn/start","params":{
            "threadId":self.observer.thread,"input":[],
            "toolOutput":{"name":"loopflow_task_update","output":&update.text,"retain":true}
        }});
        if let Some((_, history)) = self.stream() {
            if let Err(error) = history.outbound(&request).await {
                if !matches!(
                    error.downcast_ref::<StoreError>(),
                    Some(StoreError::InvalidAuthority(_))
                ) {
                    self.gap(&format!("{error:#}"));
                }
                return;
            }
        }
        let (session, attachment) = (&self.observer.session, &self.observer.attachment);
        let message = Message::Text(request.to_string().into());
        let dispatched = super::dispatch::off_reactor(|| {
            store.with_session_dispatch(session, attachment, || {
                if !store.record_task_update(
                    session,
                    &update.key,
                    "dispatched",
                    &update.observation,
                )? {
                    return Ok(false);
                }
                super::dispatch::send_fenced(writer, message)?;
                Ok(true)
            })
        });
        match dispatched {
            Ok(true) => {
                self.busy = true;
                self.update_request = Some((id, update));
            }
            Ok(false) | Err(StoreError::InvalidAuthority(_)) => {}
            Err(error) => self.gap(&error),
        }
    }
}

/// A conversation the person's client started (`/new`) or forked (`/fork`). On
/// a Session's private app-server lf opens its own thread before the observer
/// subscribes, so every later lasting top-level start is the person's. Codex
/// subagents name their parent and stay part of the selected conversation; its
/// own helpers, such as the title writer, are ephemeral and never resumable.
fn person_started(rpc: &Value) -> Option<(AgentSessionId, &'static str)> {
    let thread = &rpc["params"]["thread"];
    if !thread["parentThreadId"].is_null() || thread["ephemeral"] == true {
        return None;
    }
    let source = if thread["forkedFromId"].is_null() {
        "new"
    } else {
        "fork"
    };
    Some((thread["id"].as_str()?.into(), source))
}

/// A resumed thread with a turn in progress, or one whose turns are unknown.
fn turn_running(result: &Value) -> bool {
    result
        .pointer("/thread/turns")
        .and_then(Value::as_array)
        .is_none_or(|turns| turns.iter().any(|turn| turn["status"] == "inProgress"))
}

async fn rpc_request(upstream: &mut Upstream, method: &str, params: Value) -> Result<Value> {
    rpc_within(upstream, method, params, REPLY_LIMIT).await
}

async fn rpc_within(
    upstream: &mut Upstream,
    method: &str,
    params: Value,
    limit: Duration,
) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    upstream
        .send(Message::Text(
            json!({"id":id,"method":method,"params":params})
                .to_string()
                .into(),
        ))
        .await?;
    tokio::time::timeout(limit, async {
        while let Some(message) = upstream.next().await {
            let Message::Text(text) = message? else {
                continue;
            };
            let rpc: Value = serde_json::from_str(&text)?;
            if rpc["id"].as_str() != Some(&id) {
                continue;
            }
            if let Some(error) = rpc.get("error") {
                return Err(anyhow!("Codex {method} failed: {error}"));
            }
            return rpc
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("Codex {method} reply has no result"));
        }
        Err(anyhow!("Codex connection ended during {method}"))
    })
    .await
    .map_err(|_| anyhow!("Codex {method} timed out"))?
}

#[cfg(test)]
mod tests {
    use super::{observe, CodexObserver};
    use crate::id::LfProcessId;
    use crate::process::SessionAttachment;
    use crate::store::sqlite::SqliteStore;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::Duration;
    use tokio::net::{UnixListener, UnixStream};
    use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

    type Provider = WebSocketStream<UnixStream>;

    fn fixture(home: &Path) -> (SqliteStore, SessionAttachment, PathBuf) {
        let database = home.join("store.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        rusqlite::Connection::open(&database)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &process, true)
            .unwrap();
        (store, attachment, database)
    }

    fn observer(
        home: &Path,
        database: PathBuf,
        attachment: SessionAttachment,
        client: u32,
    ) -> CodexObserver {
        CodexObserver {
            database,
            session: "conversation".into(),
            attachment,
            endpoint: home.join("provider.sock"),
            thread: "native-thread".into(),
            account: None,
            isolate: None,
            client: (
                client,
                crate::journal::process_started_at(client).unwrap().unwrap(),
            ),
            input: None,
        }
    }

    fn client() -> std::process::Child {
        Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::null())
            .spawn()
            .unwrap()
    }

    fn end(mut client: std::process::Child) {
        client.kill().unwrap();
        client.wait().unwrap();
    }

    async fn receive(socket: &mut Provider) -> Value {
        loop {
            let message = tokio::time::timeout(Duration::from_secs(8), socket.next())
                .await
                .expect("fixture provider waited too long")
                .unwrap()
                .unwrap();
            if let Message::Text(text) = message {
                return serde_json::from_str(&text).unwrap();
            }
        }
    }

    async fn send(socket: &mut Provider, value: Value) {
        socket
            .send(Message::Text(value.to_string().into()))
            .await
            .unwrap();
    }

    /// Accept the observer through its subscription, which must add nothing:
    /// a passive `thread/resume` leaves the client's configuration alone.
    async fn subscribed(listener: &UnixListener) -> Provider {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let initialize = receive(&mut socket).await;
        send(&mut socket, json!({"id":initialize["id"],"result":{}})).await;
        assert_eq!(receive(&mut socket).await["method"], "initialized");
        let resume = receive(&mut socket).await;
        assert_eq!(resume["method"], "thread/resume");
        assert_eq!(resume["params"], json!({"threadId":"native-thread"}));
        send(
            &mut socket,
            json!({"id":resume["id"],"result":{"thread":{"id":"native-thread","turns":[]}}}),
        )
        .await;
        socket
    }

    fn turn(method: &str, id: &str, status: &str) -> Value {
        json!({"method":method,"params":{"threadId":"native-thread",
            "turn":{"id":id,"status":status,"items":[]}}})
    }

    async fn eventually(mut done: impl FnMut() -> bool) {
        for _ in 0..200 {
            if done() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("condition never held");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn turns_a_client_started_are_recorded_with_their_origin() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (store, attachment, database) = fixture(home.path());
        let client = client();
        let listener = UnixListener::bind(home.path().join("provider.sock")).unwrap();
        let watching = tokio::spawn(observe(
            observer(home.path(), database, attachment, client.id()),
            || {},
        ));
        let mut provider = subscribed(&listener).await;
        send(&mut provider, turn("turn/started", "typed", "inProgress")).await;
        send(&mut provider, turn("turn/completed", "typed", "completed")).await;
        eventually(|| {
            store
                .session_history("conversation", 0, 0)
                .unwrap()
                .iter()
                .any(|event| event.provider_turn.as_deref() == Some("typed"))
        })
        .await;
        drop(listener);
        drop(provider);
        end(client);
        watching.await.unwrap().unwrap();
        assert!(store
            .session_turn_completed("conversation", &"native-thread".into(), "typed")
            .unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_session_follows_a_conversation_the_person_starts() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (store, attachment, database) = fixture(home.path());
        store
            .record_session_agent_session("conversation", &attachment, &"native-thread".into())
            .unwrap();
        let selected = || -> String {
            store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT provider_thread FROM agent_sessions WHERE id='conversation'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let client = client();
        let listener = UnixListener::bind(home.path().join("provider.sock")).unwrap();
        let watching = tokio::spawn(observe(
            observer(home.path(), database, attachment, client.id()),
            || {},
        ));
        let mut provider = subscribed(&listener).await;
        // Subagents name their parent, and Codex's title writer is ephemeral:
        // neither moves the Session.
        send(
            &mut provider,
            json!({"method":"thread/started","params":{"thread":
            {"id":"subagent","parentThreadId":"native-thread","forkedFromId":null}}}),
        )
        .await;
        send(
            &mut provider,
            json!({"method":"thread/started","params":{"thread":
            {"id":"title","parentThreadId":null,"ephemeral":true,"threadSource":"thread_title"}}}),
        )
        .await;
        send(
            &mut provider,
            json!({"method":"thread/started","params":{"thread":
            {"id":"fresh","parentThreadId":null,"forkedFromId":null}}}),
        )
        .await;
        let resume = receive(&mut provider).await;
        assert_eq!(resume["method"], "thread/resume");
        assert_eq!(resume["params"], json!({"threadId":"fresh"}));
        assert_eq!(selected(), "fresh");
        // A thread with no rollout yet is asked again.
        send(
            &mut provider,
            json!({"id":resume["id"],"error":{"message":"no rollout found"}}),
        )
        .await;
        let resume = receive(&mut provider).await;
        assert_eq!(resume["params"], json!({"threadId":"fresh"}));
        send(
            &mut provider,
            json!({"id":resume["id"],"result":{"thread":{"id":"fresh","turns":[]}}}),
        )
        .await;
        let fresh = |method: &str, status: &str| {
            json!({"method":method,"params":{"threadId":"fresh",
                "turn":{"id":"after-new","status":status,"items":[]}}})
        };
        send(&mut provider, fresh("turn/started", "inProgress")).await;
        send(&mut provider, fresh("turn/completed", "completed")).await;
        eventually(|| {
            store
                .session_turn_completed("conversation", &"fresh".into(), "after-new")
                .unwrap()
        })
        .await;
        let history = store.session_history("conversation", 0, 0).unwrap();
        let selection = history
            .iter()
            .find(|event| event.payload["type"] == "agent_session_selected")
            .expect("the change is in selection history");
        assert_eq!(selection.payload["provider_thread"], "native-thread");
        assert_eq!(selection.payload["next_provider_thread"], "fresh");
        assert_eq!(selection.payload["source"], "new");
        assert_eq!(selected(), "fresh");
        drop(listener);
        drop(provider);
        end(client);
        watching.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn task_update_wakes_an_idle_conversation_from_the_side() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (store, attachment, database) = fixture(home.path());
        let task = crate::durable::TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/task-updates").unwrap(),
                id: task.clone(),
                project_id: None,
                title: "Updates".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store.conn.lock().unwrap().execute("UPDATE tasks SET worktree='/task-updates',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1", [task.as_str()]).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id='conversation'",
                [],
            )
            .unwrap();
        let captured = store.session("conversation").unwrap().unwrap().captured;
        store.bind_session("conversation", captured, &task).unwrap();
        let client = client();
        let listener = UnixListener::bind(home.path().join("provider.sock")).unwrap();
        let (ready, subscribed_signal) = tokio::sync::oneshot::channel();
        let watching = tokio::spawn(observe(
            observer(home.path(), database, attachment, client.id()),
            move || ready.send(()).unwrap(),
        ));
        let mut provider = subscribed(&listener).await;
        subscribed_signal.await.unwrap();
        store.test_graph(
            "feature",
            "/task-updates",
            &[("gate", Some("failed"))],
            Some("failed"),
        );
        let request = receive(&mut provider).await;
        assert_eq!(request["method"], "turn/start");
        assert_eq!(request["params"]["threadId"], "native-thread");
        assert_eq!(request["params"]["input"], json!([]));
        assert_eq!(
            request["params"]["toolOutput"]["name"],
            "loopflow_task_update"
        );
        let progress = request["params"]["toolOutput"]["output"].as_str().unwrap();
        assert!(progress.contains("gate ("));
        assert!(progress.contains("): failed"));
        send(
            &mut provider,
            json!({"id":request["id"],"result":{"turn":{"id":"report"}}}),
        )
        .await;
        eventually(|| {
            store
                .session_history("conversation", 0, 0)
                .unwrap()
                .iter()
                .any(|event| {
                    event.payload["source"] == "task_update" && event.payload["state"] == "accepted"
                })
        })
        .await;
        drop(listener);
        drop(provider);
        end(client);
        watching.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_unopenable_store_leaves_the_observer_watching() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (_store, attachment, _) = fixture(home.path());
        let client = client();
        let listener = UnixListener::bind(home.path().join("provider.sock")).unwrap();
        // A directory is never a database, as a store mid-replacement is not.
        let database = home.path().join("not-a-database");
        std::fs::create_dir(&database).unwrap();
        let watching = tokio::spawn(observe(
            observer(home.path(), database, attachment, client.id()),
            || {},
        ));
        let mut provider = subscribed(&listener).await;
        send(&mut provider, turn("turn/started", "gap", "inProgress")).await;
        send(&mut provider, turn("turn/completed", "gap", "completed")).await;
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(!watching.is_finished());
        drop(listener);
        drop(provider);
        end(client);
        watching.await.unwrap().unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_moved_attachment_ends_only_its_own_client() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (store, attachment, database) = fixture(home.path());
        let listener = UnixListener::bind(home.path().join("provider.sock")).unwrap();
        let mut client = client();
        let watching = tokio::spawn(observe(
            observer(
                home.path(),
                database.clone(),
                attachment.clone(),
                client.id(),
            ),
            || {},
        ));
        let _provider = subscribed(&listener).await;
        assert_eq!(client.try_wait().unwrap(), None);
        let replacement = LfProcessId::new();
        rusqlite::Connection::open(&database)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [replacement.as_str()],
            )
            .unwrap();
        store
            .claim_session_attachment("conversation", Some(&attachment), &replacement, false)
            .unwrap();
        eventually(|| client.try_wait().unwrap().is_some()).await;
        watching.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn an_opened_thread_is_named_so_a_client_can_resume_it() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let endpoint = home.path().join("provider.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        let provider = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let initialize = receive(&mut socket).await;
            send(&mut socket, json!({"id":initialize["id"],"result":{}})).await;
            assert_eq!(receive(&mut socket).await["method"], "initialized");
            let start = receive(&mut socket).await;
            assert_eq!(start["method"], "thread/start");
            assert_eq!(start["params"]["developerInstructions"], "prepared");
            send(
                &mut socket,
                json!({"id":start["id"],"result":{"thread":{"id":"created"}}}),
            )
            .await;
            let name = receive(&mut socket).await;
            assert_eq!(name["method"], "thread/name/set");
            assert_eq!(name["params"]["threadId"], "created");
            send(&mut socket, json!({"id":name["id"],"result":{}})).await;
            let _ = socket.next().await;
        });
        let thread = super::open_thread(
            &endpoint,
            json!({"developerInstructions":"prepared"}),
            "Fix login",
        )
        .await
        .unwrap();
        assert_eq!(thread.as_str(), "created");
        provider.await.unwrap();
    }
}
