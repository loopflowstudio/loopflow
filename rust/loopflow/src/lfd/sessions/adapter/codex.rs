use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::lfd::sessions::adapter::SessionAdapter;
use crate::lfd::sessions::types::{
    FileEdit, ItemDelta, ItemStatus, SessionConfig, SessionEvent, SessionItem, TurnStatus,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemPhase {
    Started,
    Completed,
}

pub struct CodexAdapter {
    events: broadcast::Sender<SessionEvent>,
    child: Option<Child>,
    outbound_tx: Option<mpsc::Sender<OutboundRpc>>,
    writer_task: Option<JoinHandle<()>>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    next_request_id: i64,
    turn_in_progress: Arc<AtomicBool>,
    current_turn_id: Arc<Mutex<Option<String>>>,
    shutdown_requested: Arc<AtomicBool>,
}

impl std::fmt::Debug for CodexAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexAdapter").finish()
    }
}

impl CodexAdapter {
    pub fn new(events: broadcast::Sender<SessionEvent>) -> Self {
        Self {
            events,
            child: None,
            outbound_tx: None,
            writer_task: None,
            reader_task: None,
            stderr_task: None,
            next_request_id: 1,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            current_turn_id: Arc::new(Mutex::new(None)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn send_request(&mut self, method: &str, params: Value) -> Result<()> {
        let Some(tx) = &self.outbound_tx else {
            return Err(anyhow!("codex adapter not started"));
        };
        let id = self.next_request_id;
        self.next_request_id += 1;
        tx.send(OutboundRpc::Request {
            id,
            method: method.to_string(),
            params,
        })
        .await
        .map_err(|_| anyhow!("codex writer task unavailable"))
    }

    fn extract_turn_id(params: &Value) -> Option<String> {
        params
            .get("turn")
            .and_then(|t| t.get("id"))
            .and_then(Value::as_str)
            .or_else(|| params.get("turnId").and_then(Value::as_str))
            .map(ToString::to_string)
    }

    fn item_payload(params: &Value) -> &Value {
        params.get("item").unwrap_or(params)
    }

    fn text_field(value: &Value, key: &str) -> Option<String> {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(ToString::to_string)
    }

    fn required_text_field(value: &Value, key: &str) -> String {
        Self::text_field(value, key).unwrap_or_default()
    }

    fn map_item_id(params: &Value) -> String {
        Self::item_payload(params)
            .get("id")
            .and_then(Value::as_str)
            .or_else(|| params.get("itemId").and_then(Value::as_str))
            .or_else(|| params.get("id").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_string()
    }

    fn map_item_type(params: &Value) -> &str {
        Self::item_payload(params)
            .get("type")
            .and_then(Value::as_str)
            .or_else(|| params.get("kind").and_then(Value::as_str))
            .unwrap_or("tool")
    }

    fn map_turn_status(params: &Value) -> TurnStatus {
        let turn_status = params
            .get("turn")
            .and_then(|t| t.get("status"))
            .and_then(Value::as_str)
            .or_else(|| params.get("status").and_then(Value::as_str))
            .unwrap_or_default();
        match turn_status {
            "interrupted" | "cancelled" => TurnStatus::Interrupted,
            "failed" | "error" => TurnStatus::Failed,
            _ => TurnStatus::Completed,
        }
    }

    fn map_item_status(params: &Value) -> ItemStatus {
        let status = Self::item_payload(params)
            .get("status")
            .and_then(Value::as_str)
            .or_else(|| params.get("status").and_then(Value::as_str))
            .unwrap_or("in_progress");
        match status {
            "completed" => ItemStatus::Completed,
            "failed" | "error" => ItemStatus::Failed,
            "declined" => ItemStatus::Declined,
            _ => ItemStatus::InProgress,
        }
    }

    fn parse_command(item: &Value) -> Vec<String> {
        item.get("command")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn build_item(params: &Value, phase: ItemPhase) -> SessionItem {
        let id = Self::map_item_id(params);
        let item_type = Self::map_item_type(params);
        let item = Self::item_payload(params);
        let status = Self::map_item_status(params);
        let completed = phase == ItemPhase::Completed;

        match item_type {
            "commandExecution" => SessionItem::Command {
                id,
                command: Self::parse_command(item),
                cwd: Self::required_text_field(item, "cwd"),
                status,
                output: if completed {
                    Self::text_field(item, "aggregatedOutput")
                } else {
                    None
                },
                exit_code: if completed {
                    item.get("exitCode")
                        .and_then(Value::as_i64)
                        .map(|v| v as i32)
                } else {
                    None
                },
                duration_ms: if completed {
                    item.get("durationMs").and_then(Value::as_u64)
                } else {
                    None
                },
            },
            "fileChange" => SessionItem::File {
                id,
                changes: parse_file_changes(item),
                status,
            },
            "mcpToolCall" => SessionItem::Tool {
                id,
                name: Self::mcp_tool_name(item),
                status,
                input: Some(item.get("arguments").cloned().unwrap_or_else(|| json!({}))),
                output: if completed {
                    Self::text_field(item, "result").or_else(|| Self::text_field(item, "error"))
                } else {
                    None
                },
            },
            "agentMessage" => SessionItem::Message {
                id,
                text: Self::required_text_field(item, "text"),
                phase: Self::text_field(item, "phase"),
            },
            "plan" => SessionItem::Thought {
                id,
                text: Self::required_text_field(item, "text"),
            },
            _ => SessionItem::Tool {
                id,
                name: item_type.to_string(),
                status,
                input: item.get("input").cloned(),
                output: if completed {
                    Self::text_field(item, "output")
                } else {
                    None
                },
            },
        }
    }

    fn mcp_tool_name(item: &Value) -> String {
        let tool = Self::required_text_field(item, "tool");
        let server = Self::text_field(item, "server").unwrap_or_default();
        if server.is_empty() {
            if tool.is_empty() {
                "mcp_tool_call".to_string()
            } else {
                tool
            }
        } else if tool.is_empty() {
            server
        } else {
            format!("{server}/{tool}")
        }
    }

    fn map_item_delta(method: &str, params: &Value) -> Option<ItemDelta> {
        let content = Self::text_content(params)?;
        match method {
            "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
                Some(ItemDelta::Output { content })
            }
            "item/plan/delta" => Some(ItemDelta::PlanText { content }),
            _ => None,
        }
    }

    fn text_content(params: &Value) -> Option<String> {
        params
            .get("content")
            .and_then(Value::as_str)
            .or_else(|| params.get("delta").and_then(Value::as_str))
            .or_else(|| params.get("output").and_then(Value::as_str))
            .map(ToString::to_string)
    }

    async fn resolve_turn_id(
        turn_id_from_params: Option<&str>,
        current_turn_id: &Arc<Mutex<Option<String>>>,
    ) -> String {
        if let Some(turn_id) = turn_id_from_params {
            return turn_id.to_string();
        }
        current_turn_id
            .lock()
            .await
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
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

fn parse_file_changes(item: &Value) -> Vec<FileEdit> {
    item.get("changes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|c| FileEdit {
                    path: c
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    kind: c
                        .get("kind")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    diff: c
                        .get("diff")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl SessionAdapter for CodexAdapter {
    async fn start(&mut self, config: &SessionConfig) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }
        self.shutdown_requested.store(false, Ordering::Relaxed);

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

        let method = if self.turn_in_progress.load(Ordering::Relaxed) {
            "turn/steer"
        } else {
            "turn/start"
        };

        self.send_request(method, json!({ "content": text })).await
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
        *self.current_turn_id.lock().await = None;

        self.shutdown_tasks().await;

        Ok(())
    }
}

impl CodexAdapter {
    async fn start_inner(&mut self, config: &SessionConfig) -> Result<()> {
        let mut child = Command::new("codex")
            .arg("--app-server")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|err| anyhow!("failed to spawn codex --app-server: {err}"))?;

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
        let turn_in_progress = self.turn_in_progress.clone();
        let current_turn_id = self.current_turn_id.clone();
        let shutdown_requested = self.shutdown_requested.clone();
        let event_tx = self.events.clone();
        let approval_tx = outbound_tx.clone();
        let reader_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut initialized_tx = Some(initialized_tx);

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
                    continue;
                }

                if method == "initialized" {
                    if let Some(tx) = initialized_tx.take() {
                        let _ = tx.send(());
                    }
                    continue;
                }

                // Any server request (method + id) is treated as approval and accepted.
                if let Some(id) = value.get("id") {
                    let _ = approval_tx
                        .send(OutboundRpc::Response {
                            id: id.clone(),
                            result: json!("accept"),
                        })
                        .await;
                    continue;
                }

                // Resolve turn_id from notification params or tracked state.
                let turn_id_from_params = CodexAdapter::extract_turn_id(&params);

                match method {
                    "turn/started" => {
                        let tid = turn_id_from_params
                            .clone()
                            .unwrap_or_else(|| format!("turn_{}", uuid::Uuid::new_v4()));
                        turn_in_progress.store(true, Ordering::Relaxed);
                        *current_turn_id.lock().await = Some(tid.clone());
                        let _ = event_tx.send(SessionEvent::TurnStarted { turn_id: tid });
                    }
                    "turn/completed" => {
                        let tid = CodexAdapter::resolve_turn_id(
                            turn_id_from_params.as_deref(),
                            &current_turn_id,
                        )
                        .await;
                        turn_in_progress.store(false, Ordering::Relaxed);
                        *current_turn_id.lock().await = None;
                        let status = CodexAdapter::map_turn_status(&params);
                        let _ = event_tx.send(SessionEvent::TurnCompleted {
                            turn_id: tid,
                            status,
                        });
                    }
                    "item/started" => {
                        let tid = CodexAdapter::resolve_turn_id(
                            turn_id_from_params.as_deref(),
                            &current_turn_id,
                        )
                        .await;
                        let item = CodexAdapter::build_item(&params, ItemPhase::Started);
                        let _ = event_tx.send(SessionEvent::ItemStarted { turn_id: tid, item });
                    }
                    "item/completed" => {
                        let tid = CodexAdapter::resolve_turn_id(
                            turn_id_from_params.as_deref(),
                            &current_turn_id,
                        )
                        .await;
                        let item = CodexAdapter::build_item(&params, ItemPhase::Completed);
                        let _ = event_tx.send(SessionEvent::ItemCompleted { turn_id: tid, item });
                    }
                    "item/agentMessage/delta" => {
                        if let Some(content) = CodexAdapter::text_content(&params) {
                            let tid = CodexAdapter::resolve_turn_id(
                                turn_id_from_params.as_deref(),
                                &current_turn_id,
                            )
                            .await;
                            let _ = event_tx.send(SessionEvent::TextDelta {
                                turn_id: tid,
                                content,
                            });
                        }
                    }
                    "item/agentMessage/completed" | "item/agentMessage/done" => {
                        // Final agent message text is captured in item/completed.
                    }
                    "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                        if let Some(content) = CodexAdapter::text_content(&params) {
                            let tid = CodexAdapter::resolve_turn_id(
                                turn_id_from_params.as_deref(),
                                &current_turn_id,
                            )
                            .await;
                            let _ = event_tx.send(SessionEvent::ReasoningDelta {
                                turn_id: tid,
                                content,
                            });
                        }
                    }
                    "item/commandExecution/outputDelta"
                    | "item/fileChange/outputDelta"
                    | "item/plan/delta" => {
                        if let Some(data) = CodexAdapter::map_item_delta(method, &params) {
                            let tid = CodexAdapter::resolve_turn_id(
                                turn_id_from_params.as_deref(),
                                &current_turn_id,
                            )
                            .await;
                            let item_id = CodexAdapter::map_item_id(&params);
                            let _ = event_tx.send(SessionEvent::ItemUpdated {
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
                            let tid = CodexAdapter::resolve_turn_id(
                                turn_id_from_params.as_deref(),
                                &current_turn_id,
                            )
                            .await;
                            let _ = event_tx.send(SessionEvent::DiffUpdated { turn_id: tid, diff });
                        }
                    }
                    "error" => {
                        let _ = event_tx.send(SessionEvent::Error {
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

            turn_in_progress.store(false, Ordering::Relaxed);
            if !shutdown_requested.load(Ordering::Relaxed) {
                let _ = event_tx.send(SessionEvent::Error {
                    code: "codex_disconnected".to_string(),
                    message: "codex app-server disconnected".to_string(),
                });
            }
        });

        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!(target: "lfd::sessions::codex", stderr = %line, "codex stderr");
            }
        });

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

        let mut thread_params = serde_json::Map::new();
        if let Some(model) = config.model.as_deref() {
            thread_params.insert("model".to_string(), Value::String(model.to_string()));
        }
        if let Some(cwd) = config.cwd.as_deref() {
            thread_params.insert("cwd".to_string(), Value::String(cwd.to_string()));
        }
        self.send_request("thread/start", Value::Object(thread_params))
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_item_maps_file_change_to_file() {
        let params = json!({
            "item": {
                "id": "item_1",
                "type": "fileChange",
                "status": "completed",
                "changes": [
                    {"path": "src/main.rs", "kind": "update", "diff": "-a\n+b"}
                ]
            }
        });

        let item = CodexAdapter::build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::File {
                id,
                changes,
                status,
            } => {
                assert_eq!(id, "item_1");
                assert_eq!(status, ItemStatus::Completed);
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].path, "src/main.rs");
            }
            other => panic!("expected file item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_agent_message_to_message() {
        let params = json!({
            "item": {
                "id": "item_2",
                "type": "agentMessage",
                "text": "Done",
                "phase": "final"
            }
        });

        let item = CodexAdapter::build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::Message { id, text, phase } => {
                assert_eq!(id, "item_2");
                assert_eq!(text, "Done");
                assert_eq!(phase.as_deref(), Some("final"));
            }
            other => panic!("expected message item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_plan_to_thought() {
        let params = json!({
            "item": {
                "id": "item_3",
                "type": "plan",
                "text": "Run tests first"
            }
        });

        let item = CodexAdapter::build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::Thought { id, text } => {
                assert_eq!(id, "item_3");
                assert_eq!(text, "Run tests first");
            }
            other => panic!("expected thought item, got {other:?}"),
        }
    }

    #[test]
    fn build_item_maps_mcp_tool_call_to_generic_tool() {
        let params = json!({
            "item": {
                "id": "item_4",
                "type": "mcpToolCall",
                "status": "completed",
                "server": "github",
                "tool": "search",
                "arguments": { "query": "regression" },
                "result": "ok"
            }
        });

        let item = CodexAdapter::build_item(&params, ItemPhase::Completed);
        match item {
            SessionItem::Tool {
                id,
                name,
                status,
                input,
                output,
            } => {
                assert_eq!(id, "item_4");
                assert_eq!(name, "github/search");
                assert_eq!(status, ItemStatus::Completed);
                assert_eq!(input, Some(json!({ "query": "regression" })));
                assert_eq!(output.as_deref(), Some("ok"));
            }
            other => panic!("expected tool item, got {other:?}"),
        }
    }
}
