//! Codex app-server harness, targeting the codex-cli 0.142.5 protocol.
//!
//! Protocol shapes verified live (hand-driven session + probes) and against
//! `codex app-server generate-json-schema` (v2 bundle):
//! - `initialize {clientInfo}` -> response; the CLIENT then sends the
//!   `initialized` notification (there is no server-side "initialized").
//! - `thread/start {cwd, model?, approvalPolicy?, sandbox?}` -> response with
//!   `thread.id`; also mirrored as a `thread/started` notification.
//! - `turn/start {threadId, input: [{type:"text", text}]}`.
//! - `turn/steer {threadId, expectedTurnId, input: [...]}` -> `{turnId}`;
//!   injects a userMessage item into the running turn (probed live; sending
//!   `content` instead of `input` is a -32600 "missing field `input`").
//! - `turn/interrupt {threadId, turnId}` -> `{}`; the turn then ends with
//!   `turn/completed` status "interrupted" (probed live).

use crate::id::AgentSessionId;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use crate::agent::{build_codex_thread_start_params, AgentConfig};
use crate::chat::types::ConversationEvent;
use crate::harness::codex_decode::{self, CodexDecoder};
use crate::harness::common::spawn_stderr_logger;
use crate::harness::fact::{Decoded, ProviderFact};
use crate::harness::fact_writer::FactWriter;
use crate::harness::{ApprovalPolicy, Harness, HarnessError, SendCurrentOutcome};
use crate::os_process::{
    agent_process_lifeline_path, hold_agent_process_lifeline, kill_process_group,
};
use crate::provider_account::{resolve_provider_account_exact, ProviderAccountRoute};
use crate::provider_auth::Provider;
use crate::store::ProviderAccountId;

fn build_thread_request(
    launch: &AgentConfig,
    resume_agent_session: Option<&AgentSessionId>,
) -> (&'static str, serde_json::Map<String, Value>) {
    let mut params = build_codex_thread_start_params(launch);
    match resume_agent_session {
        Some(session_id) => {
            params.insert(
                "threadId".to_string(),
                Value::String(session_id.to_string()),
            );
            ("thread/resume", params)
        }
        None => ("thread/start", params),
    }
}

#[derive(Debug)]
enum OutboundRpc {
    Request {
        id: i64,
        method: String,
        params: Value,
    },
    /// Client notification (no id, no params), e.g. `initialized`.
    Notification {
        method: String,
    },
    Response {
        id: Value,
        result: Value,
    },
}

type RpcResult = std::result::Result<Value, String>;
type PendingRequests = Arc<Mutex<HashMap<i64, oneshot::Sender<RpcResult>>>>;
type RetiredRequests = Arc<Mutex<HashSet<i64>>>;

/// Holds a correlated request's slot in `pending_requests` and releases it on
/// drop, so a caller that stops waiting (timeout, early return) never strands
/// its waiter. Without this, only a late response or shutdown would clear the
/// entry — a server that simply never replies would leak one per attempt.
struct PendingReply {
    id: i64,
    requests: PendingRequests,
    retired: RetiredRequests,
    rx: Option<oneshot::Receiver<RpcResult>>,
}

impl PendingReply {
    /// Wait for the correlated response, releasing the slot either way.
    async fn recv(mut self) -> std::result::Result<RpcResult, oneshot::error::RecvError> {
        let rx = self.rx.take().expect("pending reply awaited once");
        rx.await
    }
}

impl Drop for PendingReply {
    fn drop(&mut self) {
        let removed = self
            .requests
            .lock()
            .expect("codex pending requests lock poisoned")
            .remove(&self.id);
        if removed.is_some() {
            self.retired
                .lock()
                .expect("codex retired requests lock poisoned")
                .insert(self.id);
        }
    }
}

/// Classify a `turn/steer` error response.
///
/// Codex 0.144.5 answers every steer rejection with JSON-RPC `-32600`, so the
/// code cannot separate "this Turn will not take input" from "Loopflow sent a
/// bad request". Only the message distinguishes them. Observed live:
///
/// - `no active turn to steer` — the Turn ended between observation and
///   delivery. This is the expected Turn-boundary race, not a fault.
/// - `expected active turn id `X` but found `Y`` — the Turn rotated; our fence
///   correctly refused to steer a Turn we did not observe.
/// - `Invalid request: ...` / `thread not found: ...` — Loopflow bugs.
///
/// Unrecognized messages stay `Failed` so a real defect stays loud rather than
/// being silently absorbed as ordinary provider policy.
fn classify_steer_rejection(error: String) -> SendCurrentOutcome {
    if error.contains("no active turn to steer") || error.contains("expected active turn id") {
        return SendCurrentOutcome::NotSteerable;
    }
    SendCurrentOutcome::Failed { error }
}

/// One connection's decoder and the writer of what it establishes, shared by
/// the client-to-server and server-to-client tasks.
#[derive(Debug)]
pub(super) struct CodexStream {
    pub(super) decoder: CodexDecoder,
    pub(super) writer: FactWriter,
}

impl CodexStream {
    pub(super) fn new(writer: FactWriter) -> Self {
        Self {
            decoder: CodexDecoder::default(),
            writer,
        }
    }

    /// Decode and record a server message for the caller's known `thread`.
    pub(super) async fn inbound(
        &mut self,
        rpc: &Value,
        thread: Option<&AgentSessionId>,
    ) -> (Vec<Decoded>, Result<()>) {
        let decoded = self.decoder.decode(rpc, thread);
        let written = self.writer.write(&decoded).await;
        (decoded, written)
    }

    /// Freeze a turn request's origin and record what the client sent.
    pub(super) async fn outbound(&mut self, rpc: &Value) -> Result<()> {
        if let Some(request) = codex_decode::turn_request(rpc) {
            super::dispatch::off_reactor(|| self.writer.outbound(&request))?;
        }
        let decoded = self.decoder.outbound(rpc);
        self.writer.write(&decoded).await
    }
}

pub struct CodexHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    approval: ApprovalPolicy,
    child: Option<Child>,
    outbound_tx: Option<mpsc::Sender<OutboundRpc>>,
    pending_requests: PendingRequests,
    /// Correlated calls whose callers stopped waiting. A late response for one
    /// of these is transport history, not a new provider failure.
    retired_requests: RetiredRequests,
    writer_task: Option<JoinHandle<()>>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    next_request_id: i64,
    turn_in_progress: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
    agent_session: Arc<Mutex<Option<AgentSessionId>>>,
    resume_agent_session: Option<AgentSessionId>,
    account_route: Option<ProviderAccountRoute>,
    requested_account_id: Option<ProviderAccountId>,
    /// Live turn id (from turn/started, cleared at turn/completed); steer and
    /// interrupt address the turn with it.
    current_turn_id: Arc<Mutex<Option<String>>>,
    /// Request id of the in-flight `initialize` call; the reader completes the
    /// handshake when the matching response arrives. 0 = none pending.
    initialize_request_id: Arc<AtomicI64>,
    /// Request id of the in-flight `thread/start` call; the reader mines the
    /// matching response for the vendor thread id. 0 = none pending.
    thread_start_request_id: Arc<AtomicI64>,
    launch: Option<AgentConfig>,
    should_seed_prompt: bool,
    /// Retain the group from spawn through failed startup, before `child` is
    /// installed. The harness alone mutates it; the lifeline covers lf death.
    child_group: Option<u32>,
    agent_directory: Option<tempfile::TempDir>,
    native_start: Option<(std::path::PathBuf, Value)>,
    session_attachment: Option<super::agent_process::AttachmentOwner>,
}

impl std::fmt::Debug for CodexHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexHarness").finish()
    }
}

impl CodexHarness {
    pub(crate) fn account_route(&self) -> Option<ProviderAccountRoute> {
        self.account_route.clone()
    }

    pub fn new(events: mpsc::UnboundedSender<ConversationEvent>, approval: ApprovalPolicy) -> Self {
        Self {
            events,
            approval,
            child: None,
            outbound_tx: None,
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            retired_requests: Arc::new(Mutex::new(HashSet::new())),
            writer_task: None,
            reader_task: None,
            stderr_task: None,
            next_request_id: 1,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            agent_session: Arc::new(Mutex::new(None)),
            resume_agent_session: None,
            account_route: None,
            requested_account_id: None,
            current_turn_id: Arc::new(Mutex::new(None)),
            initialize_request_id: Arc::new(AtomicI64::new(0)),
            thread_start_request_id: Arc::new(AtomicI64::new(0)),
            launch: None,
            should_seed_prompt: true,
            child_group: None,
            agent_directory: None,
            native_start: None,
            session_attachment: None,
        }
    }

    pub(crate) fn native_start(&self) -> Option<(std::path::PathBuf, Value)> {
        self.native_start.clone()
    }

    async fn send_request(&mut self, method: &str, params: Value) -> Result<i64> {
        let Some(tx) = &self.outbound_tx else {
            return Err(anyhow!("codex harness not started"));
        };
        let id = self.next_request_id;
        self.next_request_id += 1;
        tx.send(OutboundRpc::Request {
            id,
            method: method.to_string(),
            params,
        })
        .await
        .map_err(|_| anyhow!("codex writer task unavailable"))?;
        Ok(id)
    }

    async fn send_observed_request(&mut self, method: &str, params: Value) -> Result<PendingReply> {
        let Some(outbound) = &self.outbound_tx else {
            return Err(anyhow!("codex harness not started"));
        };
        let id = self.next_request_id;
        self.next_request_id += 1;
        let (reply, reply_rx) = oneshot::channel();
        self.pending_requests
            .lock()
            .expect("codex pending requests lock poisoned")
            .insert(id, reply);
        let pending = PendingReply {
            id,
            requests: self.pending_requests.clone(),
            retired: self.retired_requests.clone(),
            rx: Some(reply_rx),
        };
        if outbound
            .send(OutboundRpc::Request {
                id,
                method: method.to_string(),
                params,
            })
            .await
            .is_err()
        {
            // `pending` drops here, releasing the slot.
            return Err(anyhow!("codex writer task unavailable"));
        }
        Ok(pending)
    }

    async fn send_notification(&mut self, method: &str) -> Result<()> {
        let Some(tx) = &self.outbound_tx else {
            return Err(anyhow!("codex harness not started"));
        };
        tx.send(OutboundRpc::Notification {
            method: method.to_string(),
        })
        .await
        .map_err(|_| anyhow!("codex writer task unavailable"))?;
        Ok(())
    }

    fn turn_id(&self) -> Option<String> {
        self.current_turn_id
            .lock()
            .expect("codex turn id lock poisoned")
            .clone()
    }

    async fn shutdown_tasks(&mut self) {
        self.outbound_tx.take();

        // Abort the reader before waiting on the writer: the reader holds a
        // clone of the outbound sender (for approval responses), and the
        // writer only exits once every sender is dropped. Waiting on the
        // writer first deadlocks when the server's stdout outlives the
        // direct child (observed live: an npm-shim `codex` leaves its real
        // app-server grandchild holding the pipe).
        if let Some(handle) = self.reader_task.take() {
            handle.abort();
            let _ = handle.await;
        }
        if let Some(handle) = self.writer_task.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.stderr_task.take() {
            handle.abort();
            let _ = handle.await;
        }
        self.pending_requests
            .lock()
            .expect("codex pending requests lock poisoned")
            .clear();
        self.retired_requests
            .lock()
            .expect("codex retired requests lock poisoned")
            .clear();
    }
}

#[async_trait]
impl Harness for CodexHarness {
    fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    fn process_group_id(&self) -> Option<u32> {
        self.child_group.filter(|pid| *pid > 1)
    }

    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        if self.outbound_tx.is_some() {
            return Ok(());
        }
        self.shutdown_requested.store(false, Ordering::Relaxed);
        self.launch = Some(config.clone());
        self.should_seed_prompt = true;
        let account_route = resolve_provider_account_exact(
            Provider::Codex,
            self.resume_agent_session.as_ref(),
            self.requested_account_id.as_ref(),
            crate::work::wave::context::ambient_wave_id().as_ref(),
        )
        .await?;
        self.account_route = account_route;
        *self
            .agent_session
            .lock()
            .expect("codex provider session id lock poisoned") = None;
        *self
            .current_turn_id
            .lock()
            .expect("codex turn id lock poisoned") = None;

        let start_result = self.start_inner(config).await;
        if let Err(err) = start_result {
            let _ = self.stop().await;
            return Err(err);
        }
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        let text = content.trim();
        let invocation = self
            .should_seed_prompt
            .then(|| {
                self.launch
                    .as_ref()
                    .and_then(|launch| launch.skill_invocation.as_ref())
            })
            .flatten();
        if text.is_empty() && invocation.is_none() {
            return Ok(());
        }
        if self.turn_in_progress.load(Ordering::Relaxed) {
            return Err(HarnessError::TurnAlreadyInProgress.into());
        }
        let turn_text = if self.should_seed_prompt {
            self.should_seed_prompt = false;
            if let Some(launch) = &self.launch {
                let mut parts = Vec::new();
                if !launch.task_prompt.trim().is_empty() {
                    parts.push(launch.task_prompt.trim().to_string());
                }
                parts.push(text.to_string());
                parts.join("\n\n")
            } else {
                text.to_string()
            }
        } else {
            text.to_string()
        };

        let thread_id = self
            .agent_session()
            .ok_or_else(|| anyhow!("codex thread not started"))?;
        let mut input = Vec::new();
        if let Some(invocation) = invocation {
            input.push(json!({ "type": "text", "text": invocation.codex_prompt() }));
        }
        input.push(json!({ "type": "text", "text": turn_text }));

        let params = json!({ "threadId": thread_id, "input": input });
        self.send_request("turn/start", params).await?;
        Ok(())
    }

    async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
        let text = content.trim();
        if text.is_empty() {
            return SendCurrentOutcome::Failed {
                error: "steer input is empty".to_string(),
            };
        }
        if !self.turn_in_progress.load(Ordering::Relaxed) {
            return SendCurrentOutcome::NotSteerable;
        }
        let (Some(thread_id), Some(turn_id)) = (self.agent_session(), self.turn_id()) else {
            return SendCurrentOutcome::NotSteerable;
        };
        let input = json!([{ "type": "text", "text": text }]);
        let reply = match self
            .send_observed_request(
                "turn/steer",
                json!({
                    "threadId": thread_id,
                    "expectedTurnId": turn_id,
                    "input": input,
                }),
            )
            .await
        {
            Ok(reply) => reply,
            Err(error) => {
                return SendCurrentOutcome::Failed {
                    error: error.to_string(),
                };
            }
        };
        match tokio::time::timeout(Duration::from_secs(15), reply.recv()).await {
            Ok(Ok(Ok(result))) => match result.get("turnId").and_then(Value::as_str) {
                Some(received) if received == turn_id => SendCurrentOutcome::Sent {
                    provider_turn_id: turn_id,
                },
                received => SendCurrentOutcome::Unknown {
                    provider_turn_id: received.map(ToString::to_string),
                    error: "Codex steer response did not confirm the expected Turn".to_string(),
                },
            },
            // A rejection is a response, not a lost message: the provider
            // definitively did not take the input, so the seed still carries it.
            Ok(Ok(Err(error))) => classify_steer_rejection(error),
            Ok(Err(_)) => SendCurrentOutcome::Unknown {
                provider_turn_id: Some(turn_id),
                error: "Codex steer response channel closed".to_string(),
            },
            Err(_) => SendCurrentOutcome::Unknown {
                provider_turn_id: Some(turn_id),
                error: "timed out waiting for Codex steer response".to_string(),
            },
        }
    }

    async fn interrupt(&mut self) -> Result<()> {
        // Cooperative cancel over the live connection: codex ends the
        // in-flight turn and reports it as turn/completed with status
        // "interrupted", which maps to Lifecycle::Interrupted. The app-server
        // process and thread stay alive for the next turn.
        if !self.turn_in_progress.load(Ordering::Relaxed) {
            return Ok(());
        }
        let (Some(thread_id), Some(turn_id)) = (self.agent_session(), self.turn_id()) else {
            return Ok(());
        };
        self.send_request(
            "turn/interrupt",
            json!({ "threadId": thread_id, "turnId": turn_id }),
        )
        .await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::Relaxed);

        if self.session_attachment.is_some()
            && (self.agent_session().is_some() || self.native_start.is_some())
        {
            // The invocation closes its AgentProcess under the attachment lock
            // during settlement, without holding SQLite across provider I/O.
            // Harness teardown only drops this connection after takeover.
            self.child.take();
            self.child_group = None;
            if let Some(directory) = self.agent_directory.take() {
                let _ = directory.keep();
            }
            self.shutdown_tasks().await;
            return Ok(());
        }

        let _ = self.interrupt().await;

        let mut terminate = || {
            if let Some(pid) = self.child_group.take() {
                kill_process_group(pid);
            }
            Ok(())
        };
        if let Some((store, session, expected)) = &self.session_attachment {
            if store
                .with_session_attachment(session, expected, terminate)
                .is_err()
            {
                self.child.take();
                if let Some(directory) = self.agent_directory.take() {
                    let _ = directory.keep();
                }
                self.shutdown_tasks().await;
                return Ok(());
            }
        } else {
            terminate()?;
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.wait().await;
        }
        self.child = None;
        self.child_group = None;
        self.turn_in_progress.store(false, Ordering::Relaxed);

        self.shutdown_tasks().await;
        self.agent_directory.take();

        Ok(())
    }

    fn agent_session(&self) -> Option<AgentSessionId> {
        self.agent_session
            .lock()
            .expect("codex provider session id lock poisoned")
            .clone()
    }

    fn set_agent_session(&mut self, agent_session: Option<AgentSessionId>) {
        self.resume_agent_session = agent_session;
    }

    fn set_provider_account_id(&mut self, account_id: Option<ProviderAccountId>) {
        self.requested_account_id = account_id;
    }

    fn provider_account_id(&self) -> Option<ProviderAccountId> {
        self.account_route
            .as_ref()
            .map(|route| route.account_id().clone())
    }
}

impl CodexHarness {
    async fn start_inner(&mut self, launch: &AgentConfig) -> Result<()> {
        let owner = super::agent_process::open_owner(launch.session_attachment.as_ref())?;
        self.session_attachment = Some(owner.clone());
        let (store, session, _) = &owner;
        let connection = store.session_connection(session)?;
        let saved_thread = store.session_thread(session)?;
        if let Some(thread) = &saved_thread {
            if self.resume_agent_session.as_ref() != Some(thread)
                && !(self.approval == ApprovalPolicy::NativeClient
                    && self.resume_agent_session.is_none())
            {
                anyhow::bail!(
                    "Saved conversation thread differs; reconnect with its recorded provider"
                );
            }
        }
        let directory = if connection.is_none() {
            // Codex refuses a socket parent other users could write to. A
            // umask of 002 (Ubuntu's default) would otherwise make it 0775.
            Some(
                tempfile::Builder::new()
                    .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
                    .tempdir()?,
            )
        } else {
            None
        };
        let endpoint = connection
            .as_ref()
            .map(|(endpoint, _)| std::path::PathBuf::from(endpoint))
            .unwrap_or_else(|| {
                directory
                    .as_ref()
                    .expect("new AgentProcess owns a directory")
                    .path()
                    .join("agent.sock")
            });
        let mut command = Command::new("codex");
        if let Some(route) = &self.account_route {
            command.args(route.provider_args());
        }
        command
            // Subcommand, not flag: codex-cli >= 0.142 renamed `--app-server`
            // to `codex app-server` (verified against 0.142.5).
            .arg("app-server")
            .args(["--listen", &format!("unix://{}", endpoint.display())])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            // Failed startup owns this child; after connection, stop consults
            // the Session fence before deciding whether to terminate it.
            .kill_on_drop(false);
        super::configure_agent_env(&mut command, launch);
        if let Some(cwd) = &launch.cwd {
            command.current_dir(cwd);
        }
        // Held until the AgentProcess has spawned, so a concurrent switch cannot
        // replace the native login between activation and startup.
        let activation = match &self.account_route {
            Some(route) => route.launch_as(command.as_std_mut()).await?,
            None => None,
        };
        super::configure_vendor_std_env(command.as_std_mut())?;
        // Codex's shell policy need not inherit arbitrary app-server environment.
        // Tool authority belongs to this conversation, including when another
        // conversation later shares its app-server. Pass only explicit launch and
        // freshly resolved lf executable/Machine values as thread configuration.
        let tool_environment = super::conversation_environment(command.as_std(), launch);
        // The app-server can host another conversation. Only this thread receives
        // its caller/capture provenance; app-server defaults must not lend it to a
        // newly admitted sibling.
        for name in crate::agent::EXECUTION_IDENTITY_ENV {
            command.env_remove(name);
        }

        let lifeline = agent_process_lifeline_path(&endpoint);
        let mut child = if connection.is_none() {
            Some(super::agent_process::spawn(
                command,
                Some(&lifeline),
                &owner,
            )?)
        } else {
            None
        };
        drop(activation);

        // The AgentProcess runs in its own group and deliberately survives this
        // harness, so no destructor or signal hook can be what stops it. The
        // lifeline ties it to the processes that drive it: when the last one
        // ends, by return, signal, panic or SIGKILL, the group is terminated.
        if let Some(pid) = child.as_ref().and_then(tokio::process::Child::id) {
            self.child_group = Some(pid);
        } else {
            // Reconnecting adopts the AgentProcess; one that predates lifelines has none.
            hold_agent_process_lifeline(&lifeline).map_err(|error| {
                anyhow!(
                    "Codex AgentProcess is stopping after its attached lf exited ({error}); retry"
                )
            })?;
        }

        self.agent_directory = directory;
        let socket = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(child) = &mut child {
                    if let Some(status) = child.try_wait()? {
                        // Codex says why it stopped; the person needs that line.
                        let mut stderr = String::new();
                        if let Some(mut pipe) = child.stderr.take() {
                            let _ = tokio::time::timeout(
                                Duration::from_secs(1),
                                tokio::io::AsyncReadExt::read_to_string(&mut pipe, &mut stderr),
                            )
                            .await;
                        }
                        return Err(anyhow!(
                            "codex app-server exited before opening its socket ({status}){}",
                            crate::os_process::exit_reason(&stderr)
                        ));
                    }
                }
                match UnixStream::connect(&endpoint).await {
                    Ok(socket) => return Ok(socket),
                    Err(error)
                        if connection.is_none()
                            && matches!(
                                error.kind(),
                                std::io::ErrorKind::NotFound
                                    | std::io::ErrorKind::ConnectionRefused
                            ) =>
                    {
                        tokio::time::sleep(Duration::from_millis(20)).await
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        })
        .await
        .map_err(|_| anyhow!("Codex app-server did not open its socket within 15s"))??;
        let socket = super::codex_connection::connect_upstream(socket).await?;
        let (mut writer, mut reader) = socket.split();
        let stderr = child.as_mut().and_then(|child| child.stderr.take());

        let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundRpc>(128);
        let authority = owner.clone();
        let writer_events = self.events.clone();
        let stream = Arc::new(tokio::sync::Mutex::new(CodexStream::new(FactWriter::new(
            owner.0.clone(),
            owner.1.clone(),
            Some(owner.2.clone()),
            self.account_route.clone(),
            Default::default(),
        ))));
        let writer_stream = stream.clone();
        let writer_task = tokio::spawn(async move {
            while let Some(message) = outbound_rx.recv().await {
                let payload = match message {
                    OutboundRpc::Request { id, method, params } => {
                        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
                    }
                    OutboundRpc::Notification { method } => {
                        json!({ "jsonrpc": "2.0", "method": method })
                    }
                    OutboundRpc::Response { id, result } => {
                        json!({ "jsonrpc": "2.0", "id": id, "result": result })
                    }
                };
                let recorded = writer_stream.lock().await.outbound(&payload).await;
                if let Err(error) = recorded {
                    let _ = writer_events.send(ConversationEvent::Error {
                        code: "codex_dispatch_rejected".into(),
                        message: error.to_string(),
                        evidence: None,
                    });
                    break;
                }
                let message = Message::Text(payload.to_string().into());
                let (store, session, expected) = authority.clone();
                let dispatched = tokio::task::spawn_blocking(move || {
                    let outcome = store.with_session_dispatch(&session, &expected, || {
                        super::dispatch::send_fenced(&mut writer, message)
                    });
                    (writer, outcome)
                })
                .await;
                let Ok((returned, outcome)) = dispatched else {
                    break;
                };
                writer = returned;
                if let Err(message) = outcome.map_err(|error| error.to_string()) {
                    let _ = writer_events.send(ConversationEvent::Error {
                        code: "codex_dispatch_rejected".into(),
                        message,
                        evidence: None,
                    });
                    break;
                }
            }
        });

        let (initialized_tx, initialized_rx) = oneshot::channel::<()>();
        let (thread_id_tx, thread_id_rx) = oneshot::channel::<AgentSessionId>();
        let turn_in_progress = self.turn_in_progress.clone();
        let shutdown_requested = self.shutdown_requested.clone();
        let event_tx = self.events.clone();
        let approval_tx = outbound_tx.clone();
        let approval = self.approval;
        let agent_session = self.agent_session.clone();
        let current_turn_id = self.current_turn_id.clone();
        let initialize_request_id = self.initialize_request_id.clone();
        let thread_start_request_id = self.thread_start_request_id.clone();
        let pending_requests = self.pending_requests.clone();
        let retired_requests = self.retired_requests.clone();
        let account_route = self.account_route.clone();
        let reader_owner = owner.clone();
        let reader_task = tokio::spawn(async move {
            let mut initialized_tx = Some(initialized_tx);
            let mut thread_id_tx = Some(thread_id_tx);

            while let Some(Ok(message)) = reader.next().await {
                let Message::Text(line) = message else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let (store, session, _) = &reader_owner;
                let decoded = {
                    let thread = super::dispatch::off_reactor(|| store.session_thread(session));
                    let (decoded, written) = stream
                        .lock()
                        .await
                        .inbound(&value, thread.as_ref().ok().and_then(Option::as_ref))
                        .await;
                    if let Err(error) = thread.map_err(anyhow::Error::from).and(written) {
                        let _ = event_tx.send(ConversationEvent::Error {
                            code: "conversation_history_unavailable".into(),
                            message: error.to_string(),
                            evidence: None,
                        });
                    }
                    decoded
                };
                for item in decoded {
                    match item {
                        Decoded::Display(event) => {
                            match &event {
                                ConversationEvent::TurnStarted { turn_id } => {
                                    turn_in_progress.store(true, Ordering::Relaxed);
                                    *current_turn_id.lock().expect("codex turn id lock poisoned") =
                                        Some(turn_id.clone());
                                }
                                ConversationEvent::TurnCompleted { .. } => {
                                    turn_in_progress.store(false, Ordering::Relaxed);
                                    *current_turn_id.lock().expect("codex turn id lock poisoned") =
                                        None;
                                }
                                _ => {}
                            }
                            let _ = event_tx.send(event);
                        }
                        Decoded::Fact(ProviderFact::SessionObserved(thread_id)) => {
                            *agent_session
                                .lock()
                                .expect("codex provider session id lock poisoned") =
                                Some(thread_id.clone());
                            if let Some(tx) = thread_id_tx.take() {
                                let _ = tx.send(thread_id);
                            }
                        }
                        Decoded::Fact(ProviderFact::RateLimit(signal))
                            if signal.limited && account_route.is_some() =>
                        {
                            let _ = event_tx.send(ConversationEvent::Error {
                                code: "provider_rate_limited".to_string(),
                                message: signal.reason,
                                evidence: None,
                            });
                        }
                        Decoded::Fact(_) => {}
                    }
                }

                if value.get("method").is_some() {
                    // Any server request (method + id) is an approval request
                    // (item/commandExecution/requestApproval and friends); answer
                    // it per the configured Loopflow response policy. The user's
                    // Codex approval policy decides whether these requests occur.
                    if let Some(id) = value.get("id") {
                        let result = match approval {
                            ApprovalPolicy::AutoApprove => json!({ "decision": "accept" }),
                            ApprovalPolicy::NativeClient => continue,
                        };
                        let _ = approval_tx
                            .send(OutboundRpc::Response {
                                id: id.clone(),
                                result,
                            })
                            .await;
                    }
                    continue;
                }

                // Response frame.
                let id = value.get("id").and_then(Value::as_i64);
                let pending = id.and_then(|id| {
                    pending_requests
                        .lock()
                        .expect("codex pending requests lock poisoned")
                        .remove(&id)
                });
                if let Some(pending) = pending {
                    let result = match value.get("error") {
                        Some(error) => Err(error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("codex RPC failed")
                            .to_string()),
                        None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = pending.send(result);
                    continue;
                }
                if id.is_some_and(|id| {
                    retired_requests
                        .lock()
                        .expect("codex retired requests lock poisoned")
                        .remove(&id)
                }) {
                    continue;
                }
                if let Some(error) = value.get("error") {
                    let _ = event_tx.send(codex_decode::rpc_error(error));
                    continue;
                }
                // The initialize response completes the handshake; the
                // harness then sends the client `initialized`
                // notification (there is no server-side "initialized").
                if id.is_some() && id == Some(initialize_request_id.load(Ordering::Relaxed)) {
                    initialize_request_id.store(0, Ordering::Relaxed);
                    if let Some(tx) = initialized_tx.take() {
                        let _ = tx.send(());
                    }
                    continue;
                }
                // The thread/start response may resume a turn already running.
                if id.is_some() && id == Some(thread_start_request_id.load(Ordering::Relaxed)) {
                    thread_start_request_id.store(0, Ordering::Relaxed);
                    if let Some(turn) = value
                        .pointer("/result/thread/turns")
                        .and_then(Value::as_array)
                        .and_then(|turns| turns.iter().find(|turn| turn["status"] == "inProgress"))
                    {
                        let turn = turn["id"].as_str().map(str::to_owned);
                        turn_in_progress.store(true, Ordering::Relaxed);
                        *current_turn_id.lock().expect("codex turn id lock poisoned") =
                            turn.clone();
                        stream.lock().await.decoder.resume_turn(turn);
                    }
                }
            }

            turn_in_progress.store(false, Ordering::Relaxed);
            let pending = std::mem::take(
                &mut *pending_requests
                    .lock()
                    .expect("codex pending requests lock poisoned"),
            );
            drop(pending);
            retired_requests
                .lock()
                .expect("codex retired requests lock poisoned")
                .clear();
            if !shutdown_requested.load(Ordering::Relaxed) {
                let _ = event_tx.send(ConversationEvent::Error {
                    code: "codex_disconnected".to_string(),
                    message: "codex app-server disconnected".to_string(),
                    evidence: None,
                });
            }
        });

        let stderr_task = stderr.map(|stderr| spawn_stderr_logger(stderr, "harness::codex"));

        self.child = child;
        self.outbound_tx = Some(outbound_tx);
        self.writer_task = Some(writer_task);
        self.reader_task = Some(reader_task);
        self.stderr_task = stderr_task;

        // Handshake: initialize -> response -> client `initialized`.
        let init_id = self.next_request_id;
        self.initialize_request_id.store(init_id, Ordering::Relaxed);
        let initialize = json!({
            "clientInfo": {
                "name": "loopflow",
                "title": "loopflow",
                "version": env!("CARGO_PKG_VERSION"),
            }
        });
        self.send_request("initialize", initialize).await?;
        tokio::time::timeout(Duration::from_secs(15), initialized_rx)
            .await
            .map_err(|_| anyhow!("timed out waiting for codex initialize response"))?
            .map_err(|_| anyhow!("codex initialize channel closed"))?;
        self.send_notification("initialized").await?;

        let (thread_method, mut thread_params) =
            build_thread_request(launch, self.resume_agent_session.as_ref());
        // Keep shell policy on the thread. Repeating -c after app-server also
        // replaces the wrapper's global config overrides in Codex's CLI parser.
        let mut config = json!({
            "shell_environment_policy.set": tool_environment,
            "allow_login_shell": false,
            "features.shell_snapshot": false,
        });
        let native = self.approval == ApprovalPolicy::NativeClient;
        if native
            && launch.write_scope == crate::agent::AgentWriteScope::Configured
            && !launch.skip_permissions
        {
            // The native UI supplies interactive approval policy. The shared
            // builder's unattended default belongs only to headless turns.
            thread_params.remove("approvalPolicy");
        }
        let roots = if native && launch.write_scope == crate::agent::AgentWriteScope::Configured {
            launch
                .cwd
                .as_deref()
                .map(crate::agent::workspace_add_dirs)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        // Resolve through the selected provider/account once. Thread overrides
        // must retain both configured writable roots and wrapper hook declarations.
        let inherited = if native && (!roots.is_empty() || launch.conversation_context.is_some()) {
            let inherited = self
                .send_observed_request(
                    "config/read",
                    json!({"cwd":launch.cwd,"includeLayers":true}),
                )
                .await?
                .recv()
                .await?
                .map_err(|error| anyhow!("Codex config/read failed: {error}"))?;
            anyhow::ensure!(
                inherited["config"].is_object(),
                "Codex config/read returned no configuration"
            );
            inherited
        } else {
            Value::Null
        };
        if !roots.is_empty() {
            let mut writable = inherited
                .pointer("/config/sandbox_workspace_write/writable_roots")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for root in roots {
                let root = json!(root);
                if !writable.contains(&root) {
                    writable.push(root);
                }
            }
            config["sandbox_workspace_write.writable_roots"] = json!(writable);
        }
        if let Some(context) = &launch.conversation_context {
            let session_hooks = inherited["layers"]
                .as_array()
                .and_then(|layers| {
                    layers
                        .iter()
                        .find(|layer| layer["name"]["type"] == "sessionFlags")
                })
                .and_then(|layer| layer["config"].get("hooks"));
            config["hooks"] =
                super::context::codex_config(context, session_hooks)?["hooks"].clone();
        }
        thread_params.insert("config".into(), config);
        if self.approval == ApprovalPolicy::NativeClient {
            let (store, session, attachment) = &owner;
            store.record_agent_endpoint(session, attachment, &endpoint.to_string_lossy())?;
            self.native_start = Some((endpoint, Value::Object(thread_params)));
            return Ok(());
        }
        // The thread params include Loopflow's conservative defaults only when
        // Codex config is missing or less permissive. More permissive user or
        // repo config, such as danger-full-access, is left alone.
        // Publish the request id before sending so the reader can match the
        // response even if it races the send.
        let request_id = self.next_request_id;
        self.thread_start_request_id
            .store(request_id, Ordering::Relaxed);
        self.send_request(thread_method, Value::Object(thread_params))
            .await?;

        // The vendor thread id arrives either in the thread/start response or
        // a thread/started notification. Wait briefly so callers can persist
        // it before the first turn; a miss degrades to agent_session()
        // returning None rather than failing startup.
        match tokio::time::timeout(Duration::from_secs(10), thread_id_rx).await {
            Ok(Ok(thread_id)) => {
                let (store, session, attachment) = &owner;
                if let Some(context) = &launch.conversation_context {
                    store.with_session_attachment(session, attachment, || {
                        Ok(context.bind_codex_session(thread_id.as_str()))
                    })??;
                }
                store.record_session_connection(
                    session,
                    attachment,
                    &endpoint.to_string_lossy(),
                    &thread_id,
                )?;
                tracing::debug!(thread_id = %thread_id, "codex thread started");
            }
            Ok(Err(_)) => {
                tracing::warn!("codex reader ended before announcing a thread id");
            }
            Err(_) => {
                tracing::warn!("timed out waiting for codex thread id; agent_session unavailable");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn native_skill_precedes_request_and_clipboard_without_continuation_replay() {
        use crate::skills::catalog::{SkillDialect, SkillSource};
        use crate::skills::invocation::SkillInvocation;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("SKILL.md");
        let source = "---\nname: probe\ndescription: Inspect the supplied asset\n---\nRead ./asset.txt before answering.\n";
        std::fs::write(&path, source).unwrap();
        std::fs::write(directory.path().join("asset.txt"), "Native asset.").unwrap();
        let skill = SkillSource {
            name: "probe".into(),
            path: Some(path.clone()),
            dialect: SkillDialect::Codex,
        }
        .load()
        .unwrap();
        let request = "Find the bug.\n\n<lf:clipboard>\nPasted evidence.\n</lf:clipboard>";
        let (mut harness, mut outbound, _, _, _events) = steerable_harness();
        harness.turn_in_progress.store(false, Ordering::Relaxed);
        harness.launch = Some(AgentConfig {
            skill_invocation: Some(SkillInvocation {
                skill,
                arguments: "exact \"two words\" 雪".into(),
            }),
            ..AgentConfig::default()
        });

        harness.send_input(request).await.unwrap();
        let OutboundRpc::Request { method, params, .. } = outbound.recv().await.unwrap() else {
            panic!("expected first turn request");
        };
        assert_eq!(method, "turn/start");
        assert_eq!(params["threadId"], "thread_1");
        assert_eq!(
            params["input"],
            json!([
                { "type": "text", "text": format!("[$probe]({}) exact \"two words\" 雪", path.display()) },
                { "type": "text", "text": request }
            ])
        );
        // The reference still selects the original declarations and adjacent assets.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert_eq!(
            std::fs::read_to_string(path.parent().unwrap().join("asset.txt")).unwrap(),
            "Native asset."
        );

        harness.send_input("Now explain the fix.").await.unwrap();
        let OutboundRpc::Request { method, params, .. } = outbound.recv().await.unwrap() else {
            panic!("expected continuation request");
        };
        assert_eq!(method, "turn/start");
        assert_eq!(params["threadId"], "thread_1");
        assert_eq!(
            params["input"],
            json!([{ "type": "text", "text": "Now explain the fix." }])
        );
    }

    #[tokio::test]
    async fn failed_startup_retains_the_group_until_stop() {
        let mut child = Command::new("/bin/sleep")
            .env_clear()
            .arg("60")
            .process_group(0)
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = CodexHarness::new(tx, ApprovalPolicy::AutoApprove);
        // Startup can fail before installing the child handle in the harness.
        harness.child_group = child.id();
        assert_eq!(harness.process_group_id(), child.id());
        harness.stop().await.unwrap();
        let status = tokio::time::timeout(Duration::from_secs(2), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(!status.success());
        assert_eq!(harness.process_group_id(), None);
    }

    #[test]
    fn saved_thread_rejection_precedes_spawn_and_leaves_the_conversation_resumable() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _binary = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let store =
            crate::store::sqlite::SqliteStore::open_ephemeral(&ledger.home().join("loopflow.db"))
                .unwrap();
        store.test_session("saved", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(ledger.home().join("loopflow.db")).unwrap();
        let process = crate::id::LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [process.as_str()],
        )
        .unwrap();
        let old = store
            .claim_session_attachment("saved", None, &process, true)
            .unwrap();
        store
            .record_session_connection("saved", &old, "/missing.sock", &"saved-thread".into())
            .unwrap();
        let attachment = store
            .claim_session_attachment("saved", Some(&old), &process, true)
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = CodexHarness::new(tx, ApprovalPolicy::AutoApprove);
        let config = AgentConfig {
            session_attachment: Some(("saved".into(), attachment.clone())),
            // Even a mistakenly reached spawn cannot launch a real provider.
            cwd: Some(ledger.home().join("absent")),
            ..Default::default()
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime.block_on(harness.start_inner(&config)).unwrap_err();
        assert!(error
            .to_string()
            .contains("Saved conversation thread differs"));
        let released = store
            .release_session_attachment("saved", &attachment)
            .unwrap();
        let retry = store
            .claim_session_attachment("saved", Some(&released), &process, true)
            .unwrap();
        harness.resume_agent_session = Some("saved-thread".into());
        let config = AgentConfig {
            session_attachment: Some(("saved".into(), retry)),
            ..config
        };
        let error = runtime.block_on(harness.start_inner(&config)).unwrap_err();
        assert!(
            error.to_string().contains("failed to spawn codex"),
            "{error}"
        );
        assert_eq!(
            store.session_thread("saved").unwrap(),
            Some("saved-thread".into())
        );
        assert!(store
            .session_history("saved", 0, 100)
            .unwrap()
            .iter()
            .all(|event| event.kind != crate::session::SessionEventKind::Completed));
    }

    #[test]
    fn start_and_resume_add_instructions_without_replacing_native_base() {
        let config = AgentConfig {
            system_prompt: "Fixed additions.".into(),
            ..Default::default()
        };
        let saved_session = AgentSessionId::from("saved-native-thread");
        for session in [None, Some(&saved_session)] {
            let (_, params) = build_thread_request(&config, session);
            assert_eq!(params["developerInstructions"], "Fixed additions.");
            assert!(!params.contains_key("baseInstructions"));
            assert!(!params.contains_key("model_instructions_file"));
        }
    }

    #[test]
    fn pinned_codex_session_uses_thread_resume() {
        let launch = AgentConfig {
            cwd: Some("/tmp/project".into()),
            ..AgentConfig::default()
        };
        let (method, params) = build_thread_request(&launch, Some(&"thread_abc".into()));

        assert_eq!(method, "thread/resume");
        assert_eq!(
            params.get("threadId").and_then(Value::as_str),
            Some("thread_abc")
        );
        assert_eq!(
            params.get("cwd").and_then(Value::as_str),
            Some("/tmp/project")
        );
    }

    #[test]
    fn new_codex_session_uses_thread_start() {
        let (method, params) = build_thread_request(&AgentConfig::default(), None);
        assert_eq!(method, "thread/start");
        assert!(!params.contains_key("threadId"));
    }

    /// A steer-ready harness plus the channels a test needs to answer it:
    /// the outbound RPC stream and the pending-waiter map.
    fn steerable_harness() -> (
        CodexHarness,
        mpsc::Receiver<OutboundRpc>,
        PendingRequests,
        RetiredRequests,
        mpsc::UnboundedReceiver<ConversationEvent>,
    ) {
        let (events, event_rx) = mpsc::unbounded_channel();
        let mut harness = CodexHarness::new(events, ApprovalPolicy::AutoApprove);
        *harness.agent_session.lock().expect("thread id lock") = Some("thread_1".into());
        *harness.current_turn_id.lock().expect("turn id lock") = Some("turn_1".to_string());
        harness.turn_in_progress.store(true, Ordering::Relaxed);
        let (outbound, outbound_rx) = mpsc::channel(1);
        harness.outbound_tx = Some(outbound);
        let pending = harness.pending_requests.clone();
        let retired = harness.retired_requests.clone();
        (harness, outbound_rx, pending, retired, event_rx)
    }

    /// Await the steer request and answer it with `reply`.
    async fn answer_steer(
        outbound_rx: &mut mpsc::Receiver<OutboundRpc>,
        pending: &PendingRequests,
        reply: RpcResult,
    ) {
        let OutboundRpc::Request { id, .. } = outbound_rx.recv().await.expect("steer request")
        else {
            panic!("current send must be an RPC request");
        };
        pending
            .lock()
            .expect("pending requests lock")
            .remove(&id)
            .expect("pending steer response")
            .send(reply)
            .expect("steer receiver");
    }

    /// Every rejection Codex 0.144.5 answers with `-32600`. Only the message
    /// separates an expected Turn-boundary race from a Loopflow bug, and the
    /// two must not read the same to the controller: a race falls back to the
    /// seed quietly, a bug stays loud.
    #[tokio::test]
    async fn steer_rejections_separate_provider_policy_from_loopflow_bugs() {
        // Observed live against codex-cli 0.144.5.
        for message in [
            "no active turn to steer",
            "expected active turn id `x` but found `y`",
        ] {
            assert_eq!(
                classify_steer_rejection(message.to_string()),
                SendCurrentOutcome::NotSteerable,
                "{message} is the Turn declining input, not a fault"
            );
        }
        for message in [
            "Invalid request: invalid type: null, expected a string",
            "thread not found: 019f0000",
        ] {
            assert_eq!(
                classify_steer_rejection(message.to_string()),
                SendCurrentOutcome::Failed {
                    error: message.to_string(),
                },
                "{message} is our own defect and must stay loud"
            );
        }
    }

    /// The Turn ending between observation and delivery is the ordinary race,
    /// so it reports NotSteerable and the input falls back to the next seed.
    #[tokio::test]
    async fn steer_against_an_ended_turn_is_not_steerable() {
        let (mut harness, mut outbound_rx, pending, _retired, _events) = steerable_harness();
        let send = tokio::spawn(async move { harness.send_current("change direction").await });

        answer_steer(
            &mut outbound_rx,
            &pending,
            Err("no active turn to steer".to_string()),
        )
        .await;

        assert_eq!(
            send.await.expect("send task"),
            SendCurrentOutcome::NotSteerable
        );
        assert!(
            pending.lock().expect("pending lock").is_empty(),
            "a rejected steer must not strand its waiter"
        );
    }

    /// A response naming a different Turn cannot prove delivery to the Turn we
    /// observed, so it stays Unknown rather than claiming Sent.
    #[tokio::test]
    async fn steer_confirming_a_different_turn_is_unknown() {
        let (mut harness, mut outbound_rx, pending, _retired, _events) = steerable_harness();
        let send = tokio::spawn(async move { harness.send_current("change direction").await });

        answer_steer(
            &mut outbound_rx,
            &pending,
            Ok(json!({ "turnId": "turn_other" })),
        )
        .await;

        assert_eq!(
            send.await.expect("send task"),
            SendCurrentOutcome::Unknown {
                provider_turn_id: Some("turn_other".to_string()),
                error: "Codex steer response did not confirm the expected Turn".to_string(),
            }
        );
    }

    /// A dropped connection mid-send is ambiguous: the provider may already
    /// hold the input. It reports Unknown and never silently retries.
    #[tokio::test]
    async fn steer_losing_the_connection_is_unknown_and_releases_its_waiter() {
        let (mut harness, mut outbound_rx, pending, _retired, _events) = steerable_harness();
        let send = tokio::spawn(async move { harness.send_current("change direction").await });

        let OutboundRpc::Request { id, .. } = outbound_rx.recv().await.expect("steer request")
        else {
            panic!("current send must be an RPC request");
        };
        // Drop the sender without replying: the reader task dying mid-flight.
        drop(
            pending
                .lock()
                .expect("pending lock")
                .remove(&id)
                .expect("pending steer response"),
        );

        assert!(matches!(
            send.await.expect("send task"),
            SendCurrentOutcome::Unknown { .. }
        ));
        assert!(pending.lock().expect("pending lock").is_empty());
    }

    /// A provider that never answers must not strand its waiter. Before the
    /// guard, only a late response or shutdown cleared the slot, so a silent
    /// server leaked one entry per attempt.
    #[tokio::test(start_paused = true)]
    async fn steer_timeout_is_unknown_and_releases_its_waiter() {
        let (mut harness, mut outbound_rx, pending, _retired, _events) = steerable_harness();
        let leaked = pending.clone();
        let send = tokio::spawn(async move { harness.send_current("change direction").await });

        // Take the request but never answer it.
        let OutboundRpc::Request { .. } = outbound_rx.recv().await.expect("steer request") else {
            panic!("current send must be an RPC request");
        };
        assert_eq!(leaked.lock().expect("pending lock").len(), 1);

        assert!(matches!(
            send.await.expect("send task"),
            SendCurrentOutcome::Unknown { .. }
        ));
        assert!(
            leaked.lock().expect("pending lock").is_empty(),
            "a timed-out steer must release its waiter, not wait for shutdown"
        );
    }

    /// A response arriving after the caller gave up finds no waiter and is
    /// dropped. It must not panic or resurrect a duplicate same-Turn attempt.
    #[tokio::test(start_paused = true)]
    async fn a_late_steer_response_cannot_revive_a_timed_out_send() {
        let (mut harness, mut outbound_rx, pending, retired, _events) = steerable_harness();
        let late = pending.clone();
        let send = tokio::spawn(async move { harness.send_current("change direction").await });

        let OutboundRpc::Request { id, .. } = outbound_rx.recv().await.expect("steer request")
        else {
            panic!("current send must be an RPC request");
        };
        assert!(matches!(
            send.await.expect("send task"),
            SendCurrentOutcome::Unknown { .. }
        ));

        // The reader's late-response path: no waiter remains to answer, and
        // the retired id consumes the response without turning it into a new
        // provider error.
        assert!(late.lock().expect("pending lock").remove(&id).is_none());
        assert!(retired.lock().expect("retired lock").remove(&id));
    }

    #[tokio::test]
    async fn current_send_names_the_exact_codex_turn() {
        let (events, _event_rx) = mpsc::unbounded_channel();
        let mut harness = CodexHarness::new(events, ApprovalPolicy::AutoApprove);
        *harness.agent_session.lock().expect("thread id lock") = Some("thread_1".into());
        *harness.current_turn_id.lock().expect("turn id lock") = Some("turn_1".to_string());
        harness.turn_in_progress.store(true, Ordering::Relaxed);
        let (outbound, mut outbound_rx) = mpsc::channel(1);
        harness.outbound_tx = Some(outbound);
        let pending_requests = harness.pending_requests.clone();
        let send = tokio::spawn(async move { harness.send_current("change direction").await });

        let OutboundRpc::Request { id, method, params } =
            outbound_rx.recv().await.expect("steer request")
        else {
            panic!("current send must be an RPC request");
        };
        assert_eq!(method, "turn/steer");
        assert_eq!(
            params.get("expectedTurnId").and_then(Value::as_str),
            Some("turn_1")
        );
        pending_requests
            .lock()
            .expect("pending requests lock")
            .remove(&id)
            .expect("pending steer response")
            .send(Ok(json!({ "turnId": "turn_1" })))
            .expect("steer receiver");
        assert_eq!(
            send.await.expect("send task"),
            SendCurrentOutcome::Sent {
                provider_turn_id: "turn_1".to_string(),
            }
        );
    }
}
