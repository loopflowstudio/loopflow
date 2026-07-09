//! Codex app-server driver, targeting the codex-cli 0.142.5 protocol.
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

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::chat::types::{ConversationEvent, ConversationItem, TurnUsage};
use crate::engine::agent::{
    build_codex_thread_start_params, system_prompt_with_structured_replies, AgentConfig,
};
use crate::harness::codex_mapping::ItemPhase;
use crate::harness::common::spawn_stderr_logger;
use crate::harness::lf_tag::LfTagParser;
use crate::harness::{codex_mapping, ApprovalPolicy, Capabilities, Harness};

/// SIGKILL an entire process group. Killing only the direct child orphans
/// the real app-server when `codex` on PATH is an npm shim that spawns it as
/// a grandchild (verified live — the orphan kept running and held the stdio
/// pipes open). Shared by `stop()` and the interrupt hook.
fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    // SAFETY: plain syscall; a negative pid targets the process group we
    // created for the child at spawn (process_group(0)).
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL)
    };
    #[cfg(not(unix))]
    let _ = pid;
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

/// Reader-local state threaded through `process_notification`.
pub(super) struct NotificationState {
    turn_in_progress: Arc<AtomicBool>,
    provider_session_id: Arc<Mutex<Option<String>>>,
    /// Shared with the harness so steer/interrupt can address the live turn.
    current_turn_id: Arc<Mutex<Option<String>>>,
    thread_id_tx: Option<oneshot::Sender<String>>,
    /// Latest thread/tokenUsage/updated snapshot, reported at turn/completed.
    pending_usage: Option<TurnUsage>,
    tag_parser: LfTagParser,
}

impl NotificationState {
    pub(super) fn new(
        turn_in_progress: Arc<AtomicBool>,
        provider_session_id: Arc<Mutex<Option<String>>>,
        current_turn_id: Arc<Mutex<Option<String>>>,
        thread_id_tx: Option<oneshot::Sender<String>>,
    ) -> Self {
        Self {
            turn_in_progress,
            provider_session_id,
            current_turn_id,
            thread_id_tx,
            pending_usage: None,
            tag_parser: LfTagParser::default(),
        }
    }

    fn resolve_turn_id(&self, turn_id_from_params: Option<String>) -> String {
        turn_id_from_params
            .or_else(|| {
                self.current_turn_id
                    .lock()
                    .expect("codex turn id lock poisoned")
                    .clone()
            })
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn set_current_turn_id(&self, turn_id: Option<String>) {
        *self
            .current_turn_id
            .lock()
            .expect("codex turn id lock poisoned") = turn_id;
    }

    fn record_thread_id(&mut self, thread_id: String) {
        *self
            .provider_session_id
            .lock()
            .expect("codex provider session id lock poisoned") = Some(thread_id.clone());
        if let Some(tx) = self.thread_id_tx.take() {
            let _ = tx.send(thread_id);
        }
    }
}

/// Dispatch one codex app-server notification into conversation events.
///
/// This is the production dispatch: the live reader task and the conformance
/// replay both call it, so trace tests pin real behavior instead of a copy.
pub(super) fn process_notification(
    method: &str,
    params: &Value,
    state: &mut NotificationState,
    events: &mpsc::UnboundedSender<ConversationEvent>,
) {
    let turn_id_from_params = codex_mapping::extract_turn_id(params);

    match method {
        "thread/started" => {
            if let Some(thread_id) = codex_mapping::extract_thread_id(params) {
                state.record_thread_id(thread_id);
            }
        }
        "turn/started" => {
            let tid =
                turn_id_from_params.unwrap_or_else(|| format!("turn_{}", uuid::Uuid::new_v4()));
            state.turn_in_progress.store(true, Ordering::Relaxed);
            state.set_current_turn_id(Some(tid.clone()));
            let _ = events.send(ConversationEvent::TurnStarted { turn_id: tid });
        }
        "turn/completed" => {
            let tid = state.resolve_turn_id(turn_id_from_params);
            for parsed_event in state.tag_parser.finish_turn(&tid) {
                let _ = events.send(parsed_event);
            }
            state.turn_in_progress.store(false, Ordering::Relaxed);
            state.set_current_turn_id(None);
            let status = codex_mapping::map_turn_status(params);
            let _ = events.send(ConversationEvent::TurnCompleted {
                turn_id: tid.clone(),
                status,
            });
            let _ = events.send(ConversationEvent::TurnUsage {
                turn_id: tid,
                usage: state.pending_usage.take().unwrap_or_default(),
            });
        }
        "thread/tokenUsage/updated" => {
            // Usage arrives mid-turn as cumulative snapshots; hold the latest
            // and report it with the terminal TurnCompleted.
            state.pending_usage = Some(codex_mapping::map_token_usage(params));
        }
        "item/started" | "item/completed" => {
            // The server echoes the client's own input (turn/start and
            // turn/steer text) back as userMessage items; the caller already
            // knows what it sent, so don't surface those as items.
            if codex_mapping::map_item_type(params) == "userMessage" {
                return;
            }
            let tid = state.resolve_turn_id(turn_id_from_params);
            if method == "item/started" {
                let item = codex_mapping::build_item(params, ItemPhase::Started);
                let _ = events.send(ConversationEvent::ItemStarted { turn_id: tid, item });
            } else {
                let item = codex_mapping::build_item(params, ItemPhase::Completed);
                let _ = events.send(ConversationEvent::ItemCompleted { turn_id: tid, item });
            }
        }
        "item/agentMessage/delta" => {
            if let Some(content) = codex_mapping::delta_content(params) {
                let tid = state.resolve_turn_id(turn_id_from_params);
                for parsed_event in state.tag_parser.consume_text(&tid, &content) {
                    let _ = events.send(parsed_event);
                }
            }
        }
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            if let Some(content) = codex_mapping::delta_content(params) {
                let tid = state.resolve_turn_id(turn_id_from_params);
                let _ = events.send(ConversationEvent::ReasoningDelta {
                    turn_id: tid,
                    content,
                });
            }
        }
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" | "item/plan/delta" => {
            if let Some(data) = codex_mapping::map_item_delta(method, params) {
                let tid = state.resolve_turn_id(turn_id_from_params);
                let item_id = codex_mapping::map_item_id(params);
                let _ = events.send(ConversationEvent::ItemUpdated {
                    turn_id: tid,
                    item_id,
                    data,
                });
            }
        }
        "turn/diff/updated" => {
            if let Some(diff) = params
                .get("diff")
                .and_then(Value::as_str)
                .map(ToString::to_string)
            {
                let tid = state.resolve_turn_id(turn_id_from_params);
                let _ = events.send(ConversationEvent::DiffUpdated { turn_id: tid, diff });
            }
        }
        "error" => {
            // ErrorNotification: {threadId, turnId, error: TurnError, willRetry}.
            let message = params
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("codex error")
                .to_string();
            let will_retry = params
                .get("willRetry")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if will_retry {
                // The vendor keeps the turn alive and retries on its own. A
                // terminal Error here would finalize a turn that is still
                // running: the next send_input becomes turn/steer into a
                // "failed" turn, the real turn/completed then finds nothing
                // open, and the scheduler wedges (verified cascade). Surface
                // the error non-terminally instead — it still lands in the
                // journal as a turn item.
                tracing::warn!(message, "codex reported a retryable error; turn continues");
                let tid = state.resolve_turn_id(turn_id_from_params);
                let _ = events.send(ConversationEvent::ItemCompleted {
                    turn_id: tid,
                    item: ConversationItem::Thought {
                        id: format!("retry_{}", uuid::Uuid::new_v4()),
                        text: format!("codex error (will retry): {message}"),
                    },
                });
            } else {
                let _ = events.send(ConversationEvent::Error {
                    code: "codex_error".to_string(),
                    message,
                });
            }
        }
        // Known 0.142.5 chatter with no conversation-level meaning.
        "thread/status/changed"
        | "account/rateLimits/updated"
        | "account/updated"
        | "mcpServer/startupStatus/updated"
        | "remoteControl/status/changed" => {
            tracing::debug!(method, "ignoring codex app-server status notification");
        }
        _ => {
            // Unknown notifications silently ignored.
        }
    }
}

/// Map a JSON-RPC error response (`{"error":{"code":-32600,"message":..},"id":N}`)
/// to a harness error event. Called by the reader for any response frame that
/// carries an `error` object.
pub(super) fn process_rpc_error(error: &Value, events: &mpsc::UnboundedSender<ConversationEvent>) {
    let code = error
        .get("code")
        .and_then(Value::as_i64)
        .map(|c| c.to_string())
        .unwrap_or_else(|| "codex_error".to_string());
    let _ = events.send(ConversationEvent::Error {
        code,
        message: error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("codex rpc error")
            .to_string(),
    });
}

pub struct CodexHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    approval: ApprovalPolicy,
    child: Option<Child>,
    outbound_tx: Option<mpsc::Sender<OutboundRpc>>,
    writer_task: Option<JoinHandle<()>>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    next_request_id: i64,
    turn_in_progress: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
    provider_session_id: Arc<Mutex<Option<String>>>,
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
    /// Pid of the live child's process group; 0 = none. Read by the interrupt
    /// hook so SIGINT/SIGTERM/SIGHUP kill the whole codex group before the
    /// process exits — the signal handler exits without running destructors,
    /// so `kill_on_drop` never fires on that path (observed live: `tmux
    /// kill-session` orphaned the app-server pair).
    child_group: Arc<AtomicU32>,
    interrupt_hook_registered: bool,
}

impl std::fmt::Debug for CodexHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexHarness").finish()
    }
}

impl CodexHarness {
    pub fn new(events: mpsc::UnboundedSender<ConversationEvent>, approval: ApprovalPolicy) -> Self {
        Self {
            events,
            approval,
            child: None,
            outbound_tx: None,
            writer_task: None,
            reader_task: None,
            stderr_task: None,
            next_request_id: 1,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            provider_session_id: Arc::new(Mutex::new(None)),
            current_turn_id: Arc::new(Mutex::new(None)),
            initialize_request_id: Arc::new(AtomicI64::new(0)),
            thread_start_request_id: Arc::new(AtomicI64::new(0)),
            launch: None,
            should_seed_prompt: true,
            child_group: Arc::new(AtomicU32::new(0)),
            interrupt_hook_registered: false,
        }
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

    fn thread_id(&self) -> Option<String> {
        self.provider_session_id
            .lock()
            .expect("codex provider session id lock poisoned")
            .clone()
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
    }
}

#[async_trait]
impl Harness for CodexHarness {
    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }
        self.shutdown_requested.store(false, Ordering::Relaxed);
        self.launch = Some(config.clone());
        self.should_seed_prompt = true;
        *self
            .provider_session_id
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
        if text.is_empty() {
            return Ok(());
        }
        let turn_text = if self.should_seed_prompt {
            self.should_seed_prompt = false;
            if let Some(launch) = &self.launch {
                let mut parts = Vec::new();
                let system_prompt = system_prompt_with_structured_replies(launch);
                if !system_prompt.trim().is_empty() {
                    parts.push(system_prompt.trim().to_string());
                }
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
            .thread_id()
            .ok_or_else(|| anyhow!("codex thread not started"))?;
        let input = json!([{ "type": "text", "text": turn_text }]);

        // Steer requires the active turn id as a precondition
        // (`expectedTurnId`); without one in hand the turn is effectively
        // over, so start a new turn instead.
        let steer_turn_id = if self.turn_in_progress.load(Ordering::Relaxed) {
            self.turn_id()
        } else {
            None
        };
        match steer_turn_id {
            Some(turn_id) => {
                self.send_request(
                    "turn/steer",
                    json!({
                        "threadId": thread_id,
                        "expectedTurnId": turn_id,
                        "input": input,
                    }),
                )
                .await?;
            }
            None => {
                self.send_request(
                    "turn/start",
                    json!({ "threadId": thread_id, "input": input }),
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        // Cooperative cancel over the live connection: codex ends the
        // in-flight turn and reports it as turn/completed with status
        // "interrupted", which maps to Lifecycle::Interrupted. The app-server
        // process and thread stay alive for the next turn.
        if !self.turn_in_progress.load(Ordering::Relaxed) {
            return Ok(());
        }
        let (Some(thread_id), Some(turn_id)) = (self.thread_id(), self.turn_id()) else {
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

        let _ = self.interrupt().await;

        if let Some(child) = self.child.as_mut() {
            if let Some(pid) = child.id() {
                kill_process_group(pid);
            }
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        self.child = None;
        self.child_group.store(0, Ordering::Release);
        self.turn_in_progress.store(false, Ordering::Relaxed);

        self.shutdown_tasks().await;

        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_steer: true,
        }
    }

    fn provider_session_id(&self) -> Option<String> {
        self.thread_id()
    }
}

impl CodexHarness {
    async fn start_inner(&mut self, launch: &AgentConfig) -> Result<()> {
        let mut command = Command::new("codex");
        command
            // Subcommand, not flag: codex-cli >= 0.142 renamed `--app-server`
            // to `codex app-server` (verified against 0.142.5).
            .arg("app-server")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            // Dropping the harness (e.g. a run task is aborted)
            // must not leak a live app-server.
            .kill_on_drop(true);
        // Own process group so stop() can kill everything under the `codex`
        // entry point, including the real app-server binary that npm shims
        // spawn as a grandchild.
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|err| anyhow!("failed to spawn codex app-server: {err}"))?;

        // Publish the group pid for the interrupt hook: the signal handler
        // (SIGINT/SIGTERM/SIGHUP — see bin/lf.rs) exits the process before
        // destructors run, so `kill_on_drop` never fires on that path. The
        // hook is what keeps `tmux kill-session` from orphaning the
        // app-server group. Registered once per harness; restarts just
        // update the atomic.
        if let Some(pid) = child.id() {
            self.child_group.store(pid, Ordering::Release);
        }
        if !self.interrupt_hook_registered {
            self.interrupt_hook_registered = true;
            let group = Arc::clone(&self.child_group);
            crate::engine::agent::register_interrupt_cleanup(move || {
                let pid = group.swap(0, Ordering::AcqRel);
                if pid != 0 {
                    kill_process_group(pid);
                }
            });
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("missing codex stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing codex stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing codex stderr"))?;

        let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundRpc>(128);
        let writer_task = tokio::spawn(async move {
            let mut writer = stdin;
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
                let Ok(line) = serde_json::to_string(&payload) else {
                    continue;
                };
                if writer.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if writer.write_all(b"\n").await.is_err() {
                    break;
                }
                if writer.flush().await.is_err() {
                    break;
                }
            }
        });

        let (initialized_tx, initialized_rx) = oneshot::channel::<()>();
        let (thread_id_tx, thread_id_rx) = oneshot::channel::<String>();
        let turn_in_progress = self.turn_in_progress.clone();
        let shutdown_requested = self.shutdown_requested.clone();
        let event_tx = self.events.clone();
        let approval_tx = outbound_tx.clone();
        let approval = self.approval;
        let provider_session_id = self.provider_session_id.clone();
        let current_turn_id = self.current_turn_id.clone();
        let initialize_request_id = self.initialize_request_id.clone();
        let thread_start_request_id = self.thread_start_request_id.clone();
        let reader_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut initialized_tx = Some(initialized_tx);
            let mut state = NotificationState::new(
                turn_in_progress.clone(),
                provider_session_id,
                current_turn_id,
                Some(thread_id_tx),
            );

            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };

                let method = value
                    .get("method")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let params = value.get("params").cloned().unwrap_or_else(|| json!({}));

                if method.is_empty() {
                    // Response frame.
                    if let Some(error) = value.get("error") {
                        process_rpc_error(error, &event_tx);
                        continue;
                    }
                    let id = value.get("id").and_then(Value::as_i64);
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
                    // The thread/start response carries the vendor thread id.
                    if id.is_some() && id == Some(thread_start_request_id.load(Ordering::Relaxed)) {
                        thread_start_request_id.store(0, Ordering::Relaxed);
                        if let Some(thread_id) = value
                            .get("result")
                            .and_then(codex_mapping::extract_thread_id)
                        {
                            state.record_thread_id(thread_id);
                        }
                    }
                    continue;
                }

                // Any server request (method + id) is an approval request
                // (item/commandExecution/requestApproval and friends); answer
                // it per the configured Loopflow response policy. The user's
                // Codex approval policy decides whether these requests occur.
                if let Some(id) = value.get("id") {
                    let result = match approval {
                        ApprovalPolicy::AutoApprove => json!({ "decision": "accept" }),
                    };
                    let _ = approval_tx
                        .send(OutboundRpc::Response {
                            id: id.clone(),
                            result,
                        })
                        .await;
                    continue;
                }

                process_notification(method, &params, &mut state, &event_tx);
            }

            turn_in_progress.store(false, Ordering::Relaxed);
            if !shutdown_requested.load(Ordering::Relaxed) {
                let _ = event_tx.send(ConversationEvent::Error {
                    code: "codex_disconnected".to_string(),
                    message: "codex app-server disconnected".to_string(),
                });
            }
        });

        let stderr_task = spawn_stderr_logger(stderr, "harness::codex");

        self.child = Some(child);
        self.outbound_tx = Some(outbound_tx);
        self.writer_task = Some(writer_task);
        self.reader_task = Some(reader_task);
        self.stderr_task = Some(stderr_task);

        // Handshake: initialize -> response -> client `initialized`.
        let init_id = self.next_request_id;
        self.initialize_request_id.store(init_id, Ordering::Relaxed);
        self.send_request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "loopflow",
                    "title": "loopflow",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        )
        .await?;
        tokio::time::timeout(Duration::from_secs(15), initialized_rx)
            .await
            .map_err(|_| anyhow!("timed out waiting for codex initialize response"))?
            .map_err(|_| anyhow!("codex initialize channel closed"))?;
        self.send_notification("initialized").await?;

        let thread_params = build_codex_thread_start_params(launch);
        // The thread params include Loopflow's conservative defaults only when
        // Codex config is missing or less permissive. More permissive user or
        // repo config, such as danger-full-access, is left alone.
        // Publish the request id before sending so the reader can match the
        // response even if it races the send.
        let request_id = self.next_request_id;
        self.thread_start_request_id
            .store(request_id, Ordering::Relaxed);
        self.send_request("thread/start", Value::Object(thread_params))
            .await?;

        // The vendor thread id arrives either in the thread/start response or
        // a thread/started notification. Wait briefly so callers can persist
        // it before the first turn; a miss degrades to provider_session_id()
        // returning None rather than failing startup.
        match tokio::time::timeout(Duration::from_secs(10), thread_id_rx).await {
            Ok(Ok(thread_id)) => {
                tracing::debug!(thread_id = %thread_id, "codex thread started");
            }
            Ok(Err(_)) => {
                tracing::warn!("codex reader ended before announcing a thread id");
            }
            Err(_) => {
                tracing::warn!(
                    "timed out waiting for codex thread id; provider_session_id unavailable"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay_state() -> (NotificationState, Arc<Mutex<Option<String>>>) {
        let slot = Arc::new(Mutex::new(None));
        let state = NotificationState::new(
            Arc::new(AtomicBool::new(false)),
            slot.clone(),
            Arc::new(Mutex::new(None)),
            None,
        );
        (state, slot)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kill_process_group_reaches_the_grandchild() {
        // The npm-shim shape: the direct child backgrounds a grandchild
        // (same process group) and exits. The grandchild touches a flag
        // file after a short sleep; killing the group must take it down
        // before the sleep finishes, so the flag never appears. This is
        // the same kill the interrupt hook fires on SIGINT/SIGTERM/SIGHUP.
        let tmp = tempfile::tempdir().unwrap();
        let flag = tmp.path().join("survived");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(format!("(sleep 1 && touch {}) &", flag.display()));
        command.process_group(0);
        let mut child = command.spawn().unwrap();
        let pid = child.id().unwrap();
        // Let the shell fork the grandchild and exit.
        let _ = child.wait().await;

        kill_process_group(pid);

        // Past the grandchild's sleep: if it leaked, the flag would exist.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(!flag.exists(), "grandchild outlived the group kill");
    }

    #[test]
    fn thread_started_notification_records_provider_session_id() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (mut state, slot) = replay_state();

        process_notification(
            "thread/started",
            &json!({ "thread": { "id": "thread_abc", "sessionId": "thread_abc" } }),
            &mut state,
            &tx,
        );

        assert_eq!(slot.lock().unwrap().as_deref(), Some("thread_abc"));
    }

    #[test]
    fn turn_started_sets_turn_in_progress_and_turn_id() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (mut state, _slot) = replay_state();
        let in_progress = state.turn_in_progress.clone();
        let turn_slot = state.current_turn_id.clone();

        process_notification(
            "turn/started",
            &json!({ "threadId": "thread_1", "turn": { "id": "turn_1", "status": "inProgress" } }),
            &mut state,
            &tx,
        );
        assert!(in_progress.load(Ordering::Relaxed));
        assert_eq!(turn_slot.lock().unwrap().as_deref(), Some("turn_1"));

        process_notification(
            "turn/completed",
            &json!({ "threadId": "thread_1", "turn": { "id": "turn_1", "status": "interrupted", "error": null } }),
            &mut state,
            &tx,
        );
        assert!(!in_progress.load(Ordering::Relaxed));
        assert!(turn_slot.lock().unwrap().is_none());

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(matches!(
            events[1],
            ConversationEvent::TurnCompleted {
                status: crate::chat::types::Lifecycle::Interrupted,
                ..
            }
        ));
    }
}
