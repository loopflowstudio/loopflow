use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::engine::agent::{
    build_codex_thread_start_params, system_prompt_with_structured_replies, AgentConfig,
};
use crate::lfd::conversations::harness::codex_mapping::ItemPhase;
use crate::lfd::conversations::harness::common::spawn_stderr_logger;
use crate::lfd::conversations::harness::lf_tag::LfTagParser;
use crate::lfd::conversations::harness::{codex_mapping, ApprovalPolicy, Capabilities, Harness};
use crate::lfd::conversations::types::ConversationEvent;

#[derive(Debug)]
enum OutboundRpc {
    Request {
        id: i64,
        method: String,
        params: Value,
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
    thread_id_tx: Option<oneshot::Sender<String>>,
    current_turn_id: Option<String>,
    tag_parser: LfTagParser,
}

impl NotificationState {
    pub(super) fn new(
        turn_in_progress: Arc<AtomicBool>,
        provider_session_id: Arc<Mutex<Option<String>>>,
        thread_id_tx: Option<oneshot::Sender<String>>,
    ) -> Self {
        Self {
            turn_in_progress,
            provider_session_id,
            thread_id_tx,
            current_turn_id: None,
            tag_parser: LfTagParser::default(),
        }
    }

    fn resolve_turn_id(&self, turn_id_from_params: Option<String>) -> String {
        turn_id_from_params
            .or_else(|| self.current_turn_id.clone())
            .unwrap_or_else(|| "unknown".to_string())
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
            state.current_turn_id = Some(tid.clone());
            let _ = events.send(ConversationEvent::TurnStarted { turn_id: tid });
        }
        "turn/completed" => {
            let tid = state.resolve_turn_id(turn_id_from_params);
            for parsed_event in state.tag_parser.finish_turn(&tid) {
                let _ = events.send(parsed_event);
            }
            state.turn_in_progress.store(false, Ordering::Relaxed);
            state.current_turn_id = None;
            let status = codex_mapping::map_turn_status(params);
            let _ = events.send(ConversationEvent::TurnCompleted {
                turn_id: tid.clone(),
                status,
            });
            let _ = events.send(ConversationEvent::TurnUsage {
                turn_id: tid,
                usage: codex_mapping::map_turn_usage(params),
            });
        }
        "item/started" => {
            let tid = state.resolve_turn_id(turn_id_from_params);
            let item = codex_mapping::build_item(params, ItemPhase::Started);
            let _ = events.send(ConversationEvent::ItemStarted { turn_id: tid, item });
        }
        "item/completed" => {
            let tid = state.resolve_turn_id(turn_id_from_params);
            let item = codex_mapping::build_item(params, ItemPhase::Completed);
            let _ = events.send(ConversationEvent::ItemCompleted { turn_id: tid, item });
        }
        "item/agentMessage/delta" => {
            if let Some(content) = codex_mapping::text_content(params) {
                let tid = state.resolve_turn_id(turn_id_from_params);
                for parsed_event in state.tag_parser.consume_text(&tid, &content) {
                    let _ = events.send(parsed_event);
                }
            }
        }
        "item/agentMessage/completed" | "item/agentMessage/done" => {
            // Final agent message text is captured in item/completed.
        }
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            if let Some(content) = codex_mapping::text_content(params) {
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
            let _ = events.send(ConversationEvent::Error {
                code: params
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("codex_error")
                    .to_string(),
                message: params
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("codex error")
                    .to_string(),
            });
        }
        _ => {
            // Unknown notifications silently ignored.
        }
    }
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
    /// Request id of the in-flight `thread/start` call; the reader mines the
    /// matching response for the vendor thread id. 0 = none pending.
    thread_start_request_id: Arc<AtomicI64>,
    launch: Option<AgentConfig>,
    should_seed_prompt: bool,
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
            thread_start_request_id: Arc::new(AtomicI64::new(0)),
            launch: None,
            should_seed_prompt: true,
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

    async fn shutdown_tasks(&mut self) {
        self.outbound_tx.take();

        if let Some(handle) = self.writer_task.take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.reader_task.take() {
            handle.abort();
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
        let turn_content = if self.should_seed_prompt {
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

        let method = if self.turn_in_progress.load(Ordering::Relaxed) {
            "turn/steer"
        } else {
            "turn/start"
        };

        self.send_request(method, json!({ "content": turn_content }))
            .await?;
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
        self.send_request("turn/interrupt", json!({})).await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::Relaxed);

        if self.turn_in_progress.load(Ordering::Relaxed) {
            let _ = self.send_request("turn/interrupt", json!({})).await;
        }

        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        self.child = None;
        self.turn_in_progress.store(false, Ordering::Relaxed);

        self.shutdown_tasks().await;

        Ok(())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_steer: true,
            supports_interrupt: true,
        }
    }

    fn provider_session_id(&self) -> Option<String> {
        self.provider_session_id
            .lock()
            .expect("codex provider session id lock poisoned")
            .clone()
    }
}

impl CodexHarness {
    async fn start_inner(&mut self, launch: &AgentConfig) -> Result<()> {
        let mut child = Command::new("codex")
            // Subcommand, not flag: codex-cli >= 0.142 renamed `--app-server`
            // to `codex app-server` (verified against 0.142.5).
            .arg("app-server")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            // Dropping the harness (e.g. the wave mind's task is aborted)
            // must not leak a live app-server.
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| anyhow!("failed to spawn codex app-server: {err}"))?;

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
        let thread_start_request_id = self.thread_start_request_id.clone();
        let reader_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut initialized_tx = Some(initialized_tx);
            let mut state = NotificationState::new(
                turn_in_progress.clone(),
                provider_session_id,
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
                    // Response frame. The only one mined is the thread/start
                    // response, which carries the vendor thread id.
                    let pending = thread_start_request_id.load(Ordering::Relaxed);
                    if pending != 0 && value.get("id").and_then(Value::as_i64) == Some(pending) {
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

                if method == "initialized" {
                    if let Some(tx) = initialized_tx.take() {
                        let _ = tx.send(());
                    }
                    continue;
                }

                // Any server request (method + id) is an approval request;
                // answer it per the configured policy.
                if let Some(id) = value.get("id") {
                    let result = match approval {
                        ApprovalPolicy::AutoApprove => json!("accept"),
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

        let stderr_task = spawn_stderr_logger(stderr, "lfd::conversations::codex");

        self.child = Some(child);
        self.outbound_tx = Some(outbound_tx);
        self.writer_task = Some(writer_task);
        self.reader_task = Some(reader_task);
        self.stderr_task = Some(stderr_task);

        self.send_request("initialize", json!({})).await?;
        tokio::time::timeout(Duration::from_secs(15), initialized_rx)
            .await
            .map_err(|_| anyhow!("timed out waiting for codex initialize"))?
            .map_err(|_| anyhow!("codex initialize channel closed"))?;

        let thread_params = build_codex_thread_start_params(launch);
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
        let state = NotificationState::new(Arc::new(AtomicBool::new(false)), slot.clone(), None);
        (state, slot)
    }

    #[test]
    fn thread_started_notification_records_provider_session_id() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let (mut state, slot) = replay_state();

        process_notification(
            "thread/started",
            &json!({ "thread": { "id": "thread_abc" } }),
            &mut state,
            &tx,
        );

        assert_eq!(slot.lock().unwrap().as_deref(), Some("thread_abc"));
    }

    #[test]
    fn turn_started_sets_turn_in_progress() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (mut state, _slot) = replay_state();
        let in_progress = state.turn_in_progress.clone();

        process_notification(
            "turn/started",
            &json!({ "turn": { "id": "turn_1" } }),
            &mut state,
            &tx,
        );
        assert!(in_progress.load(Ordering::Relaxed));

        process_notification(
            "turn/completed",
            &json!({ "turn": { "id": "turn_1", "status": "interrupted" } }),
            &mut state,
            &tx,
        );
        assert!(!in_progress.load(Ordering::Relaxed));

        let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert!(matches!(
            events[1],
            ConversationEvent::TurnCompleted {
                status: crate::lfd::conversations::types::Lifecycle::Interrupted,
                ..
            }
        ));
    }
}
