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

use crate::id::AgentSessionId;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::{client_async, tungstenite::Message};

use crate::chat::types::{ConversationEvent, ConversationItem, TurnUsage};
use crate::engine::agent::{build_codex_thread_start_params, AgentConfig};
use crate::engine::process::{
    bind_group_to_driver, engine_lifeline_path, hold_engine_lifeline, kill_process_group,
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

/// Reader-local state threaded through `process_notification`.
pub(super) struct NotificationState {
    turn_in_progress: Arc<AtomicBool>,
    agent_session: Arc<Mutex<Option<AgentSessionId>>>,
    /// Shared with the harness so steer/interrupt can address the live turn.
    current_turn_id: Arc<Mutex<Option<String>>>,
    thread_id_tx: Option<oneshot::Sender<AgentSessionId>>,
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
    reported_error: Option<String>,
    tag_parser: LfTagParser,
}

impl NotificationState {
    pub(super) fn new(
        turn_in_progress: Arc<AtomicBool>,
        agent_session: Arc<Mutex<Option<AgentSessionId>>>,
        current_turn_id: Arc<Mutex<Option<String>>>,
        thread_id_tx: Option<oneshot::Sender<AgentSessionId>>,
    ) -> Self {
        Self {
            turn_in_progress,
            agent_session,
            current_turn_id,
            thread_id_tx,
            pending_usage: None,
            reported: None,
            streamed_agent_messages: HashSet::new(),
            reported_error: None,
            tag_parser: LfTagParser::default(),
        }
    }

    /// Convert the latest thread-lifetime snapshot into this Turn's cumulative
    /// usage without advancing the completed-Turn baseline.
    fn current_turn_usage(&self) -> Option<TurnUsage> {
        let snapshot = self.pending_usage.as_ref()?;
        if !snapshot.is_reported() {
            return None;
        }
        let baseline = self.reported.unwrap_or_default();
        let gross_input = snapshot
            .input_tokens
            .map(|value| value.saturating_sub(baseline.gross_input));
        let output = snapshot
            .output_tokens
            .map(|value| value.saturating_sub(baseline.output));
        let reasoning = snapshot
            .reasoning_tokens
            .map(|value| value.saturating_sub(baseline.reasoning));
        let cached = snapshot
            .cache_read_tokens
            .map(|value| value.saturating_sub(baseline.cached));
        Some(TurnUsage {
            input_tokens: gross_input.map(|gross| gross.saturating_sub(cached.unwrap_or(0))),
            output_tokens: output,
            total_input_tokens: gross_input,
            peak_input_tokens: snapshot.peak_input_tokens,
            context_window_tokens: snapshot.context_window_tokens,
            reasoning_tokens: reasoning,
            cache_read_tokens: cached,
            cache_write_tokens: None,
            model: None,
            cost_usd: None,
        })
    }

    /// Finish the Turn and advance the attributed thread baseline.
    fn take_turn_usage(&mut self) -> Option<TurnUsage> {
        let usage = self.current_turn_usage()?;
        let snapshot = self.pending_usage.take()?;
        let baseline = self.reported.unwrap_or_default();
        self.reported = Some(ReportedTotals {
            gross_input: snapshot
                .input_tokens
                .unwrap_or(baseline.gross_input)
                .max(baseline.gross_input),
            output: snapshot
                .output_tokens
                .unwrap_or(baseline.output)
                .max(baseline.output),
            reasoning: snapshot
                .reasoning_tokens
                .unwrap_or(0)
                .max(baseline.reasoning),
            cached: snapshot.cache_read_tokens.unwrap_or(0).max(baseline.cached),
        });
        Some(usage)
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

    fn record_thread_id(&mut self, thread_id: AgentSessionId) {
        *self
            .agent_session
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
            state.reported_error = None;
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
            if let Some(usage) = state.take_turn_usage() {
                let _ = events.send(ConversationEvent::UsageCheckpoint {
                    turn_id: tid.clone(),
                    usage,
                    final_receipt: true,
                });
            }
            if status == crate::chat::types::Lifecycle::Failed {
                if let Some(message) = params
                    .pointer("/turn/error/message")
                    .and_then(Value::as_str)
                    .filter(|message| state.reported_error.as_deref() != Some(*message))
                {
                    let _ = events.send(ConversationEvent::Error {
                        code: "codex_error".into(),
                        message: message.to_owned(),
                        evidence: None,
                    });
                }
            }
            let _ = events.send(ConversationEvent::TurnCompleted {
                turn_id: tid,
                status,
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
            if let (Some(turn_id), Some(usage)) = (
                state
                    .current_turn_id
                    .lock()
                    .expect("codex turn id lock poisoned")
                    .clone(),
                state.current_turn_usage(),
            ) {
                let _ = events.send(ConversationEvent::UsageCheckpoint {
                    turn_id,
                    usage,
                    final_receipt: false,
                });
            }
        }
        "item/started" | "item/completed" => {
            // The server echoes the client's own input (turn/start and
            // turn/steer text) back as userMessage items; the caller already
            // knows what it sent, so don't surface those as items.
            let item_type = codex_mapping::map_item_type(params);
            if item_type == "userMessage" {
                return;
            }
            // agentMessage/delta is the live prose stream. Preserve a matching
            // final-answer completion as a durable phase receipt; the shared
            // turn fold recognizes that its prose was already streamed.
            if item_type == "agentMessage" {
                if method == "item/started" {
                    return;
                }
                if state
                    .streamed_agent_messages
                    .contains(&codex_mapping::map_item_id(params))
                {
                    let item = codex_mapping::build_item(params, ItemPhase::Completed);
                    if matches!(
                        &item,
                        ConversationItem::Message { phase, .. }
                            if phase.as_deref() == Some("final_answer")
                    ) {
                        let tid = state.resolve_turn_id(turn_id_from_params);
                        let _ =
                            events.send(ConversationEvent::ItemCompleted { turn_id: tid, item });
                    }
                    return;
                }
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
                state.reported_error = Some(message.clone());
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
    /// Pid of the live child's process group; 0 = none. `stop()` kills it;
    /// every other way this process can end is covered by the driver
    /// lifeline bound at spawn.
    child_group: Arc<AtomicU32>,
    engine_directory: Option<tempfile::TempDir>,
    endpoint: Option<PathBuf>,
    session_driver: Option<(
        crate::store::sqlite::SqliteStore,
        String,
        crate::process::SessionDriver,
    )>,
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
            agent_session: Arc::new(Mutex::new(None)),
            resume_agent_session: None,
            account_route: None,
            requested_account_id: None,
            current_turn_id: Arc::new(Mutex::new(None)),
            initialize_request_id: Arc::new(AtomicI64::new(0)),
            thread_start_request_id: Arc::new(AtomicI64::new(0)),
            launch: None,
            should_seed_prompt: true,
            child_group: Arc::new(AtomicU32::new(0)),
            engine_directory: None,
            endpoint: None,
            session_driver: None,
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
    fn process_id(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

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
        if self.outbound_tx.is_some() {
            return Ok(());
        }
        self.shutdown_requested.store(false, Ordering::Relaxed);
        self.launch = Some(config.clone());
        self.should_seed_prompt = true;
        let requested_session = self.resume_agent_session.clone();
        let account_route = resolve_provider_account_exact(
            Provider::Codex,
            requested_session.as_ref(),
            self.requested_account_id.as_ref(),
        )
        .await?;
        self.resume_agent_session = match &account_route {
            Some(route) if route.resume_requested_session() => requested_session,
            Some(_) => None,
            None => requested_session,
        };
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
        let mut input = vec![json!({ "type": "text", "text": turn_text })];
        if let Some(invocation) = invocation {
            input.push(json!({ "type": "text", "text": invocation.codex_prompt() }));
        }

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

        if self.session_driver.is_some() && self.agent_session().is_some() {
            // Managed engines close when the invocation settles its driver, under
            // the same ownership transaction as takeover. Harness teardown
            // only drops this connection; a replaced driver cannot stop work.
            self.child.take();
            self.child_group.store(0, Ordering::Release);
            if let Some(directory) = self.engine_directory.take() {
                let _ = directory.keep();
            }
            self.shutdown_tasks().await;
            return Ok(());
        }

        let _ = self.interrupt().await;

        let group = self.child_group.clone();
        let terminate = || {
            let pid = group.swap(0, Ordering::AcqRel);
            if pid != 0 {
                kill_process_group(pid);
            }
            Ok(())
        };
        if let Some((store, session, expected)) = &self.session_driver {
            if store
                .with_session_driver(session, expected, terminate)
                .is_err()
            {
                self.child.take();
                if let Some(directory) = self.engine_directory.take() {
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
        self.child_group.store(0, Ordering::Release);
        self.turn_in_progress.store(false, Ordering::Relaxed);

        self.shutdown_tasks().await;
        self.engine_directory.take();

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
        self.session_driver = launch
            .session_driver
            .as_ref()
            .map(|(session, driver)| {
                let path = crate::store::database_path_from_env()?;
                Ok::<_, anyhow::Error>((
                    crate::store::sqlite::SqliteStore::new(&path)?,
                    session.clone(),
                    driver.clone(),
                ))
            })
            .transpose()?;
        let connection = self
            .session_driver
            .as_ref()
            .map(|(store, session, _)| store.session_connection(session))
            .transpose()?
            .flatten();
        let saved_thread = self
            .session_driver
            .as_ref()
            .map(|(store, session, _)| store.session_thread(session))
            .transpose()?
            .flatten();
        if let Some(thread) = &saved_thread {
            if self.resume_agent_session.as_ref() != Some(thread) {
                anyhow::bail!(
                    "Saved conversation thread differs; reconnect with its recorded provider"
                );
            }
        }
        let directory = if connection.is_none() {
            Some(
                tempfile::Builder::new()
                    .prefix("lf-codex-")
                    .tempdir_in("/tmp")?,
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
                    .expect("new engine owns a directory")
                    .path()
                    .join("engine.sock")
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
        // Held until the engine has spawned, so a concurrent switch cannot
        // replace the native login between activation and startup.
        let activation = match &self.account_route {
            Some(route) => route.launch_as(command.as_std_mut()).await?,
            None => None,
        };
        // Own process group so stop() can kill everything under the `codex`
        // entry point, including the real app-server binary that npm shims
        // spawn as a grandchild.
        #[cfg(unix)]
        command.process_group(0);
        super::configure_vendor_std_env(command.as_std_mut())?;
        // A login shell/snapshot can replace the launcher's PATH with the
        // installation, losing a development Session's executable/Machine.
        command.args([
            "-c",
            "allow_login_shell=false",
            "-c",
            "features.shell_snapshot=false",
        ]);

        // Codex's shell policy need not inherit arbitrary engine environment.
        // Tool authority belongs to this conversation, including when another
        // conversation later shares its engine. Pass only explicit launch and
        // freshly resolved lf executable/Machine values as thread configuration.
        let tool_environment = super::conversation_environment(command.as_std(), launch);
        // The engine can host another conversation. Only this thread receives
        // its caller/capture provenance; engine defaults must not lend it to a
        // newly admitted sibling.
        for name in crate::engine::agent::EXECUTION_IDENTITY_ENV {
            command.env_remove(name);
        }

        if connection.is_none() {
            if let Some((store, session, driver)) = &self.session_driver {
                store.record_session_provider_launch(session, driver, true)?;
            }
        }
        let mut child = if connection.is_none() {
            Some(
                command
                    .spawn()
                    .map_err(|err| anyhow!("failed to spawn codex app-server: {err}"))?,
            )
        } else {
            None
        };
        drop(activation);

        // The engine runs in its own group and deliberately survives this
        // harness, so no destructor or signal hook can be what stops it. The
        // lifeline ties it to the processes that drive it: when the last one
        // ends, by return, signal, panic or SIGKILL, the group is terminated.
        let lifeline = engine_lifeline_path(&endpoint);
        if let Some(pid) = child.as_ref().and_then(tokio::process::Child::id) {
            self.child_group.store(pid, Ordering::Release);
            if let Some((store, session, driver)) = &self.session_driver {
                if let Some(started_at) = crate::journal::process_started_at(pid)? {
                    store.record_session_provider_process(session, driver, pid, started_at)?;
                }
            }
            bind_group_to_driver(pid, Some(&lifeline)).map_err(|error| {
                anyhow!("failed to bind codex app-server to its driver: {error}")
            })?;
        } else {
            // Reconnecting adopts the engine; one that predates lifelines has none.
            hold_engine_lifeline(&lifeline).map_err(|error| {
                anyhow!("Codex engine is stopping after its driver exited ({error}); retry")
            })?;
        }

        self.endpoint = Some(endpoint.clone());
        self.engine_directory = directory;
        let socket = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(child) = &mut child {
                    if let Some(status) = child.try_wait()? {
                        return Err(anyhow!(
                            "codex app-server exited before opening its socket: {status}"
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
        .await??;
        let (socket, _) = client_async("ws://localhost", socket).await?;
        let (mut writer, mut reader) = socket.split();
        let stderr = child.as_mut().and_then(|child| child.stderr.take());

        let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundRpc>(128);
        let authority = self.session_driver.clone();
        let writer_events = self.events.clone();
        let native_history = Arc::new(Mutex::new(super::codex_history::History::default()));
        let writer_history = native_history.clone();
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
                writer_history
                    .lock()
                    .expect("codex history lock poisoned")
                    .request(&payload);
                let message = Message::Text(payload.to_string().into());
                let outcome = if let Some((store, session, expected)) = authority.clone() {
                    let dispatched = tokio::task::spawn_blocking(move || {
                        let outcome = store.with_session_driver(&session, &expected, || {
                            super::dispatch::send_fenced(&mut writer, message)
                        });
                        (writer, outcome)
                    })
                    .await;
                    let Ok((returned, outcome)) = dispatched else {
                        break;
                    };
                    writer = returned;
                    outcome.map_err(|error| error.to_string())
                } else {
                    writer
                        .send(message)
                        .await
                        .map_err(|error| error.to_string())
                };
                if let Err(message) = outcome {
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
        let raw_provider = self.raw_provider.clone();
        let approval_tx = outbound_tx.clone();
        let approval = self.approval;
        let agent_session = self.agent_session.clone();
        let current_turn_id = self.current_turn_id.clone();
        let initialize_request_id = self.initialize_request_id.clone();
        let thread_start_request_id = self.thread_start_request_id.clone();
        let pending_requests = self.pending_requests.clone();
        let retired_requests = self.retired_requests.clone();
        let account_route = self.account_route.clone();
        let history = self.session_driver.clone();
        let reader_task = tokio::spawn(async move {
            let mut initialized_tx = Some(initialized_tx);
            let mut state = NotificationState::new(
                turn_in_progress.clone(),
                agent_session,
                current_turn_id,
                Some(thread_id_tx),
            );

            while let Some(Ok(message)) = reader.next().await {
                let Message::Text(line) = message else {
                    continue;
                };
                if let Some(raw_provider) = &raw_provider {
                    let _ = raw_provider.send(RawProviderEvent {
                        stream: "notification",
                        line: line.to_string(),
                    });
                }
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if let Some((store, session, driver)) = &history {
                    let recorded = super::dispatch::off_reactor(|| {
                        native_history
                            .lock()
                            .expect("codex history lock poisoned")
                            .record(store, session, Some(driver), None, &value)
                    });
                    if let Err(error) = recorded {
                        let _ = event_tx.send(ConversationEvent::Error {
                            code: "conversation_history_unavailable".into(),
                            message: error.to_string(),
                            evidence: None,
                        });
                    }
                }

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
                        if let Some(turn) = value
                            .pointer("/result/thread/turns")
                            .and_then(Value::as_array)
                            .and_then(|turns| {
                                turns.iter().find(|turn| turn["status"] == "inProgress")
                            })
                        {
                            state.turn_in_progress.store(true, Ordering::Relaxed);
                            *state
                                .current_turn_id
                                .lock()
                                .expect("codex turn id lock poisoned") =
                                turn["id"].as_str().map(str::to_owned);
                        }
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
                    .agent_session
                    .lock()
                    .expect("codex provider session id lock poisoned")
                    .clone();
                process_notification(method, &params, &mut state, &event_tx);
                let current_session = state
                    .agent_session
                    .lock()
                    .expect("codex provider session id lock poisoned")
                    .clone();
                if current_session != previous_session {
                    if let (Some(route), Some(session_id)) =
                        (account_route.as_ref(), current_session.as_ref())
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
        let config = json!({
            "shell_environment_policy.set": tool_environment,
            "allow_login_shell": false,
            "features.shell_snapshot": false,
        });
        thread_params.insert("config".into(), config);
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
                if let Some((store, session, driver)) = &self.session_driver {
                    store.record_session_connection(
                        session,
                        driver,
                        &endpoint.to_string_lossy(),
                        &thread_id,
                    )?;
                }
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
        let process = crate::id::ProcessLfid::new();
        sql.execute(
            "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'fixture',1)",
            [process.as_str()],
        )
        .unwrap();
        let old = store
            .claim_session_driver("saved", None, &process, true)
            .unwrap();
        store
            .record_session_connection("saved", &old, "/missing.sock", &"saved-thread".into())
            .unwrap();
        let driver = store
            .claim_session_driver("saved", Some(&old), &process, true)
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = CodexHarness::new(tx, ApprovalPolicy::AutoApprove);
        let config = AgentConfig {
            session_driver: Some(("saved".into(), driver.clone())),
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
        let released = store.release_session_driver("saved", &driver).unwrap();
        let retry = store
            .claim_session_driver("saved", Some(&released), &process, true)
            .unwrap();
        harness.resume_agent_session = Some("saved-thread".into());
        let config = AgentConfig {
            session_driver: Some(("saved".into(), retry)),
            ..config
        };
        let error = runtime.block_on(harness.start_inner(&config)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to spawn codex app-server"),
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

    fn replay_state() -> (NotificationState, Arc<Mutex<Option<AgentSessionId>>>) {
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
        let first = state.take_turn_usage().expect("first usage");
        assert_eq!(first.input_tokens, Some(6_465));
        assert_eq!(first.total_input_tokens, Some(16_065));
        assert_eq!(first.cache_read_tokens, Some(9_600));
        assert_eq!(first.output_tokens, Some(5));

        state.pending_usage = Some(codex_mapping::map_token_usage(&usage(20_065, 12, 13_100)));
        let second = state.take_turn_usage().expect("second usage");
        assert_eq!(
            second.input_tokens,
            Some(500),
            "gross Δ4000 minus cached Δ3500"
        );
        assert_eq!(second.total_input_tokens, Some(4_000));
        assert_eq!(second.cache_read_tokens, Some(3_500));
        assert_eq!(second.output_tokens, Some(7));

        // A turn that reported no usage stays missing rather than repeating.
        assert_eq!(state.take_turn_usage(), None);
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

        let usage = state.take_turn_usage().expect("resumed usage");
        assert_eq!(usage.total_input_tokens, Some(4_000));
        assert_eq!(usage.cache_read_tokens, Some(3_000));
        assert_eq!(usage.input_tokens, Some(1_000));
        assert_eq!(usage.output_tokens, Some(50));
        assert_eq!(usage.reasoning_tokens, Some(10));
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

    #[test]
    fn thread_started_notification_records_agent_session() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (mut state, slot) = replay_state();

        process_notification(
            "thread/started",
            &json!({ "thread": { "id": "thread_abc", "sessionId": "thread_abc" } }),
            &mut state,
            &tx,
        );

        assert_eq!(*slot.lock().unwrap(), Some("thread_abc".into()));
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
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            ConversationEvent::TextDelta { content, .. } if content == "Hello"
        ));
        assert!(matches!(
            &events[1],
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { text, phase, .. },
                ..
            } if text == "Hello" && phase.as_deref() == Some("final_answer")
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
