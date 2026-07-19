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

use std::collections::{HashMap, HashSet};
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
use crate::harness::{
    codex_mapping, ApprovalPolicy, Harness, HarnessError, RawProviderEvent, SendCurrentOutcome,
};
use crate::provider_account::{resolve_provider_account_exact, ProviderAccountRoute};
use crate::provider_auth::Provider;
use crate::store::ProviderAccountId;

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

fn build_thread_request(
    launch: &AgentConfig,
    resume_provider_session_id: Option<&str>,
) -> (&'static str, serde_json::Map<String, Value>) {
    let mut params = build_codex_thread_start_params(launch);
    match resume_provider_session_id {
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

/// Reader-local state threaded through `process_notification`.
pub(super) struct NotificationState {
    turn_in_progress: Arc<AtomicBool>,
    provider_session_id: Arc<Mutex<Option<String>>>,
    /// Shared with the harness so steer/interrupt can address the live turn.
    current_turn_id: Arc<Mutex<Option<String>>>,
    thread_id_tx: Option<oneshot::Sender<String>>,
    /// Latest thread/tokenUsage/updated snapshot, reported at turn/completed.
    pending_usage: Option<TurnUsage>,
    /// Cumulative thread totals already attributed to completed turns. Codex
    /// reports lifetime-of-thread numbers; each turn reports the difference so
    /// its usage means the same thing as Claude's per-turn report. `None`
    /// until the first snapshot seeds it — a resumed thread arrives carrying
    /// history that belongs to earlier launches, not to this turn.
    reported: Option<ReportedTotals>,
    /// Codex closes each streamed agent message with the full text again.
    /// Remember which item ids already arrived as deltas so completion is a
    /// recovery fallback, not a second copy of the prose.
    streamed_agent_messages: HashSet<String>,
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
            reported: None,
            streamed_agent_messages: HashSet::new(),
            tag_parser: LfTagParser::default(),
        }
    }

    /// Convert the latest cumulative snapshot into this turn's own usage and
    /// advance the attributed baseline. Input is reported net of cache reads —
    /// the same shape Claude reports — with the gross figure in
    /// `total_input_tokens`.
    fn take_turn_usage(&mut self) -> TurnUsage {
        let Some(snapshot) = self.pending_usage.take() else {
            return TurnUsage::default();
        };
        let baseline = self.reported.unwrap_or_default();
        let gross_input = snapshot.input_tokens.saturating_sub(baseline.gross_input);
        let output = snapshot.output_tokens.saturating_sub(baseline.output);
        let reasoning = snapshot
            .reasoning_tokens
            .unwrap_or(0)
            .saturating_sub(baseline.reasoning);
        let cached = snapshot
            .cache_read_tokens
            .unwrap_or(0)
            .saturating_sub(baseline.cached);
        self.reported = Some(ReportedTotals {
            gross_input: snapshot.input_tokens.max(baseline.gross_input),
            output: snapshot.output_tokens.max(baseline.output),
            reasoning: snapshot
                .reasoning_tokens
                .unwrap_or(0)
                .max(baseline.reasoning),
            cached: snapshot.cache_read_tokens.unwrap_or(0).max(baseline.cached),
        });
        TurnUsage {
            input_tokens: gross_input.saturating_sub(cached),
            output_tokens: output,
            total_input_tokens: Some(gross_input),
            peak_input_tokens: snapshot.peak_input_tokens,
            context_window_tokens: snapshot.context_window_tokens,
            reasoning_tokens: Some(reasoning),
            cache_read_tokens: Some(cached),
            cache_write_tokens: None,
            model: None,
            cost_usd: None,
        }
    }

    /// On the first snapshot of the process, everything the thread consumed
    /// before this request belongs to earlier launches of a resumed session —
    /// baseline it out using the request-sized `last` report.
    fn seed_reported_baseline(&mut self, params: &Value) {
        if self.reported.is_some() {
            return;
        }
        let total = |key: &str| {
            params
                .pointer(&format!("/tokenUsage/total/{key}"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        let last = |key: &str| {
            params
                .pointer(&format!("/tokenUsage/last/{key}"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };
        self.reported = Some(ReportedTotals {
            gross_input: total("inputTokens").saturating_sub(last("inputTokens")),
            output: total("outputTokens").saturating_sub(last("outputTokens")),
            reasoning: total("reasoningOutputTokens").saturating_sub(last("reasoningOutputTokens")),
            cached: total("cachedInputTokens").saturating_sub(last("cachedInputTokens")),
        });
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
            let usage = state.take_turn_usage();
            let _ = events.send(ConversationEvent::TurnUsage {
                turn_id: tid,
                usage,
            });
        }
        "thread/tokenUsage/updated" => {
            // Usage arrives mid-turn as cumulative snapshots. Keep the latest
            // lifetime totals and the highest single-request window pressure.
            state.seed_reported_baseline(params);
            let mut latest = codex_mapping::map_token_usage(params);
            if let Some(previous) = state.pending_usage.take() {
                retain_higher_context_pressure(&mut latest, &previous);
            }
            state.pending_usage = Some(latest);
        }
        "item/started" | "item/completed" => {
            // The server echoes the client's own input (turn/start and
            // turn/steer text) back as userMessage items; the caller already
            // knows what it sent, so don't surface those as items.
            let item_type = codex_mapping::map_item_type(params);
            if item_type == "userMessage" {
                return;
            }
            // agentMessage/delta is the live prose stream. The matching
            // item/completed repeats the entire message, but remains useful
            // as a fallback if a provider version omits the deltas.
            if item_type == "agentMessage"
                && (method == "item/started"
                    || state
                        .streamed_agent_messages
                        .contains(&codex_mapping::map_item_id(params)))
            {
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
                state
                    .streamed_agent_messages
                    .insert(codex_mapping::map_item_id(params));
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
                    evidence: None,
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

/// Cumulative thread totals (gross input includes cache reads, as codex
/// reports them) already attributed to completed turns.
#[derive(Debug, Default, Clone, Copy)]
struct ReportedTotals {
    gross_input: u64,
    output: u64,
    reasoning: u64,
    cached: u64,
}

fn retain_higher_context_pressure(latest: &mut TurnUsage, previous: &TurnUsage) {
    let Some(previous_peak) = previous.peak_input_tokens else {
        return;
    };
    let Some(previous_window) = previous.context_window_tokens else {
        return;
    };
    let latest_is_higher = match (latest.peak_input_tokens, latest.context_window_tokens) {
        (Some(peak), Some(window)) => {
            u128::from(peak) * u128::from(previous_window)
                >= u128::from(previous_peak) * u128::from(window)
        }
        _ => false,
    };
    if !latest_is_higher {
        latest.peak_input_tokens = Some(previous_peak);
        latest.context_window_tokens = Some(previous_window);
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
        evidence: None,
    });
}

pub struct CodexHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
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
    provider_session_id: Arc<Mutex<Option<String>>>,
    resume_provider_session_id: Option<String>,
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
            raw_provider: None,
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
            provider_session_id: Arc::new(Mutex::new(None)),
            resume_provider_session_id: None,
            account_route: None,
            requested_account_id: None,
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
    fn set_raw_provider_sender(
        &mut self,
        raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
    ) {
        self.raw_provider = raw_provider;
    }

    fn process_group_id(&self) -> Option<u32> {
        let group = self.child_group.load(Ordering::SeqCst);
        (group > 1).then_some(group)
    }

    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }
        self.shutdown_requested.store(false, Ordering::Relaxed);
        self.launch = Some(config.clone());
        self.should_seed_prompt = true;
        let requested_session = self.resume_provider_session_id.clone();
        let account_route = resolve_provider_account_exact(
            Provider::Codex,
            requested_session.as_deref(),
            self.requested_account_id.as_ref(),
        )
        .await?;
        self.resume_provider_session_id = match &account_route {
            Some(route) if route.resume_requested_session() => requested_session,
            Some(_) => None,
            None => requested_session,
        };
        self.account_route = account_route;
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
        if self.turn_in_progress.load(Ordering::Relaxed) {
            return Err(HarnessError::TurnAlreadyInProgress.into());
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

        self.send_request(
            "turn/start",
            json!({ "threadId": thread_id, "input": input }),
        )
        .await?;
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
        let (Some(thread_id), Some(turn_id)) = (self.thread_id(), self.turn_id()) else {
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

    fn provider_session_id(&self) -> Option<String> {
        self.thread_id()
    }

    fn set_provider_session_id(&mut self, provider_session_id: Option<String>) {
        self.resume_provider_session_id = provider_session_id;
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
        let mut command = Command::new("codex");
        if self
            .account_route
            .as_ref()
            .is_some_and(ProviderAccountRoute::uses_native_home)
        {
            command.args(["-c", "cli_auth_credentials_store=\"file\""]);
        }
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
        if let Some(route) = &self.account_route {
            route.apply_tokio(&mut command);
        }
        // Own process group so stop() can kill everything under the `codex`
        // entry point, including the real app-server binary that npm shims
        // spawn as a grandchild.
        #[cfg(unix)]
        command.process_group(0);
        super::configure_vendor_tokio_env(&mut command)?;
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
        let raw_provider = self.raw_provider.clone();
        let approval_tx = outbound_tx.clone();
        let approval = self.approval;
        let provider_session_id = self.provider_session_id.clone();
        let current_turn_id = self.current_turn_id.clone();
        let initialize_request_id = self.initialize_request_id.clone();
        let thread_start_request_id = self.thread_start_request_id.clone();
        let pending_requests = self.pending_requests.clone();
        let retired_requests = self.retired_requests.clone();
        let account_route = self.account_route.clone();
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
                if let Some(raw_provider) = &raw_provider {
                    let _ = raw_provider.send(RawProviderEvent {
                        stream: "notification",
                        line: line.clone(),
                    });
                }
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
                        process_rpc_error(error, &event_tx);
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
                    // The thread/start response carries the vendor thread id.
                    if id.is_some() && id == Some(thread_start_request_id.load(Ordering::Relaxed)) {
                        thread_start_request_id.store(0, Ordering::Relaxed);
                        if let Some(thread_id) = value
                            .get("result")
                            .and_then(codex_mapping::extract_thread_id)
                        {
                            state.record_thread_id(thread_id.clone());
                            if let Some(route) = &account_route {
                                if let Err(error) = route.pin_session(&thread_id).await {
                                    tracing::warn!(%error, "failed to pin Codex provider session account");
                                }
                            }
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

                if method == "account/rateLimits/updated" {
                    if let (Some(route), Some(signal)) = (
                        account_route.as_ref(),
                        codex_mapping::rate_limit_signal(&params),
                    ) {
                        if let Err(error) = route.record_rate_limit(&signal).await {
                            tracing::warn!(%error, "failed to record Codex account rate limit");
                        }
                        if signal.limited {
                            let _ = event_tx.send(ConversationEvent::Error {
                                code: "provider_rate_limited".to_string(),
                                message: signal.reason,
                                evidence: None,
                            });
                            continue;
                        }
                    }
                }

                let previous_session = state
                    .provider_session_id
                    .lock()
                    .expect("codex provider session id lock poisoned")
                    .clone();
                process_notification(method, &params, &mut state, &event_tx);
                let current_session = state
                    .provider_session_id
                    .lock()
                    .expect("codex provider session id lock poisoned")
                    .clone();
                if current_session != previous_session {
                    if let (Some(route), Some(session_id)) =
                        (account_route.as_ref(), current_session.as_deref())
                    {
                        if let Err(error) = route.pin_session(session_id).await {
                            tracing::warn!(%error, "failed to pin Codex provider session account");
                        }
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

        let (thread_method, thread_params) =
            build_thread_request(launch, self.resume_provider_session_id.as_deref());
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

    /// Codex reports cumulative thread totals; each completed turn must
    /// report only its own spend, with input net of cache reads.
    #[test]
    fn a_second_turn_reports_only_its_own_spend() {
        let (mut state, _slot) = replay_state();
        let usage = |gross, output, cached| {
            serde_json::json!({
                "tokenUsage": {
                    "total": {"inputTokens": gross, "outputTokens": output,
                              "cachedInputTokens": cached, "reasoningOutputTokens": 0},
                    "last": {"inputTokens": gross},
                    "modelContextWindow": 200_000
                }
            })
        };

        state.pending_usage = Some(codex_mapping::map_token_usage(&usage(16_065, 5, 9_600)));
        let first = state.take_turn_usage();
        assert_eq!(first.input_tokens, 6_465);
        assert_eq!(first.total_input_tokens, Some(16_065));
        assert_eq!(first.cache_read_tokens, Some(9_600));
        assert_eq!(first.output_tokens, 5);

        state.pending_usage = Some(codex_mapping::map_token_usage(&usage(20_065, 12, 13_100)));
        let second = state.take_turn_usage();
        assert_eq!(second.input_tokens, 500, "gross Δ4000 minus cached Δ3500");
        assert_eq!(second.total_input_tokens, Some(4_000));
        assert_eq!(second.cache_read_tokens, Some(3_500));
        assert_eq!(second.output_tokens, 7);

        // A turn that reported no usage stays zero rather than repeating.
        let quiet = state.take_turn_usage();
        assert_eq!(quiet.input_tokens, 0);
        assert_eq!(quiet.output_tokens, 0);
    }

    /// A resumed thread's first snapshot carries every earlier launch's
    /// tokens in `total`; only the request-sized `last` belongs to this turn.
    #[test]
    fn a_resumed_thread_baselines_out_prior_history() {
        let (mut state, _slot) = replay_state();
        let params = serde_json::json!({
            "tokenUsage": {
                "total": {"inputTokens": 100_000, "outputTokens": 9_000,
                          "cachedInputTokens": 80_000, "reasoningOutputTokens": 500},
                "last": {"inputTokens": 4_000, "outputTokens": 50,
                         "cachedInputTokens": 3_000, "reasoningOutputTokens": 10},
                "modelContextWindow": 200_000
            }
        });
        state.seed_reported_baseline(&params);
        state.pending_usage = Some(codex_mapping::map_token_usage(&params));

        let usage = state.take_turn_usage();
        assert_eq!(usage.total_input_tokens, Some(4_000));
        assert_eq!(usage.cache_read_tokens, Some(3_000));
        assert_eq!(usage.input_tokens, 1_000);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.reasoning_tokens, Some(10));
    }

    #[test]
    fn pinned_codex_session_uses_thread_resume() {
        let launch = AgentConfig {
            cwd: Some("/tmp/project".into()),
            ..AgentConfig::default()
        };
        let (method, params) = build_thread_request(&launch, Some("thread_abc"));

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
        *harness.provider_session_id.lock().expect("thread id lock") = Some("thread_1".to_string());
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
        *harness.provider_session_id.lock().expect("thread id lock") = Some("thread_1".to_string());
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

    #[test]
    fn completed_agent_message_does_not_repeat_streamed_prose() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (mut state, _slot) = replay_state();

        process_notification(
            "item/agentMessage/delta",
            &json!({
                "turnId": "turn_1",
                "itemId": "msg_1",
                "delta": "Hello"
            }),
            &mut state,
            &tx,
        );
        process_notification(
            "item/completed",
            &json!({
                "turnId": "turn_1",
                "item": {
                    "id": "msg_1",
                    "type": "agentMessage",
                    "text": "Hello",
                    "phase": "final_answer"
                }
            }),
            &mut state,
            &tx,
        );

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            ConversationEvent::TextDelta { content, .. } if content == "Hello"
        ));
    }

    #[test]
    fn completed_agent_message_is_a_fallback_without_deltas() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (mut state, _slot) = replay_state();

        process_notification(
            "item/completed",
            &json!({
                "turnId": "turn_1",
                "item": {
                    "id": "msg_1",
                    "type": "agentMessage",
                    "text": "Recovered",
                    "phase": "final_answer"
                }
            }),
            &mut state,
            &tx,
        );

        assert!(matches!(
            rx.try_recv().expect("fallback message"),
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { ref text, .. },
                ..
            } if text == "Recovered"
        ));
    }
}
