use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::lfd::sessions::adapter::SessionAdapter;
use crate::lfd::sessions::types::{SessionConfig, SessionEvent, TurnStatus};

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

pub struct CodexAdapter {
    events: broadcast::Sender<SessionEvent>,
    child: Option<Child>,
    outbound_tx: Option<mpsc::Sender<OutboundRpc>>,
    writer_task: Option<JoinHandle<()>>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    next_request_id: i64,
    turn_in_progress: Arc<AtomicBool>,
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

    fn map_tool_id(params: &Value) -> String {
        params
            .get("tool_id")
            .and_then(Value::as_str)
            .or_else(|| params.get("item_id").and_then(Value::as_str))
            .or_else(|| params.get("id").and_then(Value::as_str))
            .or_else(|| {
                params
                    .get("item")
                    .and_then(|item| item.get("id"))
                    .and_then(Value::as_str)
            })
            .unwrap_or("unknown")
            .to_string()
    }

    fn map_turn_status(params: &Value) -> TurnStatus {
        let status = params
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match status {
            "interrupted" | "cancelled" => TurnStatus::Interrupted,
            "failed" | "error" => TurnStatus::Failed,
            _ => TurnStatus::Completed,
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

    fn map_notification(method: &str, params: &Value) -> Option<SessionEvent> {
        match method {
            "item/agentMessage/delta" => {
                Self::text_content(params).map(|content| SessionEvent::TextDelta { content })
            }
            "item/agentMessage/completed" | "item/agentMessage/done" => {
                Self::text_content(params).map(|content| SessionEvent::TextDone { content })
            }
            "item/started" => {
                let kind = params
                    .get("kind")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        params
                            .get("item")
                            .and_then(|item| item.get("kind"))
                            .and_then(Value::as_str)
                    })
                    .unwrap_or("tool");
                let name = kind.to_string();
                let input = params.get("input").cloned().or_else(|| {
                    params
                        .get("item")
                        .and_then(|item| item.get("input"))
                        .cloned()
                });
                Some(SessionEvent::ToolStarted {
                    tool_id: Self::map_tool_id(params),
                    name,
                    input,
                })
            }
            "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
                Self::text_content(params).map(|content| SessionEvent::ToolOutput {
                    tool_id: Self::map_tool_id(params),
                    content,
                })
            }
            "item/completed" => Some(SessionEvent::ToolDone {
                tool_id: Self::map_tool_id(params),
            }),
            "error" => Some(SessionEvent::Error {
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
            }),
            _ => None,
        }
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

                if !method.is_empty() {
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

                    match method {
                        "turn/started" => {
                            turn_in_progress.store(true, Ordering::Relaxed);
                            let _ = event_tx.send(SessionEvent::TurnStarted);
                            continue;
                        }
                        "turn/completed" => {
                            turn_in_progress.store(false, Ordering::Relaxed);
                            let status = CodexAdapter::map_turn_status(&params);
                            let _ = event_tx.send(SessionEvent::TurnCompleted { status });
                            continue;
                        }
                        _ => {}
                    }

                    if let Some(event) = CodexAdapter::map_notification(method, &params) {
                        let _ = event_tx.send(event);
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
