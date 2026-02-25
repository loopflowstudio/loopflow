use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::engine::agent::{build_codex_thread_start_params, AgentConfig};
use crate::lfd::sessions::harness::codex_mapping::ItemPhase;
use crate::lfd::sessions::harness::common::spawn_stderr_logger;
use crate::lfd::sessions::harness::{codex_mapping, Harness};
use crate::lfd::sessions::types::SessionEvent;

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

pub struct CodexHarness {
    events: mpsc::UnboundedSender<SessionEvent>,
    child: Option<Child>,
    outbound_tx: Option<mpsc::Sender<OutboundRpc>>,
    writer_task: Option<JoinHandle<()>>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    next_request_id: i64,
    turn_in_progress: Arc<AtomicBool>,
    current_turn_id: Arc<Mutex<Option<String>>>,
    shutdown_requested: Arc<AtomicBool>,
    launch: Option<AgentConfig>,
    should_seed_prompt: bool,
}

impl std::fmt::Debug for CodexHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexHarness").finish()
    }
}

impl CodexHarness {
    pub fn new(events: mpsc::UnboundedSender<SessionEvent>) -> Self {
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
            launch: None,
            should_seed_prompt: true,
        }
    }

    async fn send_request(&mut self, method: &str, params: Value) -> Result<()> {
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
        .map_err(|_| anyhow!("codex writer task unavailable"))
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
                if !launch.system_prompt.trim().is_empty() {
                    parts.push(launch.system_prompt.trim().to_string());
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
            .await
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

impl CodexHarness {
    async fn start_inner(&mut self, launch: &AgentConfig) -> Result<()> {
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
                let turn_id_from_params = codex_mapping::extract_turn_id(&params);

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
                        let tid =
                            resolve_turn_id(turn_id_from_params.as_deref(), &current_turn_id).await;
                        turn_in_progress.store(false, Ordering::Relaxed);
                        *current_turn_id.lock().await = None;
                        let status = codex_mapping::map_turn_status(&params);
                        let _ = event_tx.send(SessionEvent::TurnCompleted {
                            turn_id: tid,
                            status,
                        });
                    }
                    "item/started" => {
                        let tid =
                            resolve_turn_id(turn_id_from_params.as_deref(), &current_turn_id).await;
                        let item = codex_mapping::build_item(&params, ItemPhase::Started);
                        let _ = event_tx.send(SessionEvent::ItemStarted { turn_id: tid, item });
                    }
                    "item/completed" => {
                        let tid =
                            resolve_turn_id(turn_id_from_params.as_deref(), &current_turn_id).await;
                        let item = codex_mapping::build_item(&params, ItemPhase::Completed);
                        let _ = event_tx.send(SessionEvent::ItemCompleted { turn_id: tid, item });
                    }
                    "item/agentMessage/delta" => {
                        if let Some(content) = codex_mapping::text_content(&params) {
                            let tid =
                                resolve_turn_id(turn_id_from_params.as_deref(), &current_turn_id)
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
                        if let Some(content) = codex_mapping::text_content(&params) {
                            let tid =
                                resolve_turn_id(turn_id_from_params.as_deref(), &current_turn_id)
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
                        if let Some(data) = codex_mapping::map_item_delta(method, &params) {
                            let tid =
                                resolve_turn_id(turn_id_from_params.as_deref(), &current_turn_id)
                                    .await;
                            let item_id = codex_mapping::map_item_id(&params);
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
                            let tid =
                                resolve_turn_id(turn_id_from_params.as_deref(), &current_turn_id)
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

        let stderr_task = spawn_stderr_logger(stderr, "lfd::sessions::codex");

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
        self.send_request("thread/start", Value::Object(thread_params))
            .await?;

        Ok(())
    }
}
