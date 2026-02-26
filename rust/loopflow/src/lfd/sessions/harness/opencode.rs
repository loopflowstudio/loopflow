use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Method;
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::engine::agent::AgentConfig;
use crate::lfd::sessions::harness::common::{spawn_stderr_logger, TurnInProgressGuard};
use crate::lfd::sessions::harness::{opencode_mapping, Harness, HarnessError};
use crate::lfd::sessions::opencode_runtime;
use crate::lfd::sessions::types::SessionEvent;

const OPENCODE_DISCONNECTED_CODE: &str = "opencode_disconnected";

pub struct OpenCodeHarness {
    events: mpsc::UnboundedSender<SessionEvent>,
    client: reqwest::Client,
    config: Option<AgentConfig>,
    should_seed_prompt: bool,
    turn_in_progress: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
    child: Option<Child>,
    stderr_task: Option<JoinHandle<()>>,
    sse_task: Option<JoinHandle<()>>,
    server_base_url: Option<String>,
    provider_session_id: Option<String>,
}

impl std::fmt::Debug for OpenCodeHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenCodeHarness").finish()
    }
}

impl OpenCodeHarness {
    pub fn new(events: mpsc::UnboundedSender<SessionEvent>) -> Self {
        Self {
            events,
            client: reqwest::Client::new(),
            config: None,
            should_seed_prompt: true,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            child: None,
            stderr_task: None,
            sse_task: None,
            server_base_url: None,
            provider_session_id: None,
        }
    }

    async fn start_inner(&mut self, config: &AgentConfig) -> Result<()> {
        let port = allocate_port()?;
        let mut command = Command::new("opencode");
        command
            .arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        if let Some(cwd) = &config.cwd {
            command.current_dir(cwd);
        }

        let mut child = command
            .spawn()
            .map_err(|err| anyhow!("failed to spawn opencode serve: {err}"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing opencode stderr"))?;

        let base_url = format!("http://127.0.0.1:{port}");
        if let Err(err) = wait_for_server(&self.client, &base_url, &mut child).await {
            shutdown_child(&mut child).await;
            return Err(err);
        }

        let provider_session_id = match create_provider_session(&self.client, &base_url).await {
            Ok(provider_session_id) => provider_session_id,
            Err(err) => {
                shutdown_child(&mut child).await;
                return Err(err);
            }
        };

        let _ = self.events.send(SessionEvent::ProviderSessionId {
            provider_session_id: provider_session_id.clone(),
        });

        let event_tx = self.events.clone();
        let client = self.client.clone();
        let shutdown_requested = self.shutdown_requested.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let reader_base_url = base_url.clone();
        let reader_session_id = provider_session_id.clone();

        let sse_task = tokio::spawn(async move {
            let stream_url = format!("{reader_base_url}/event");
            let request = client
                .get(&stream_url)
                .header(reqwest::header::ACCEPT, "text/event-stream");
            let mut response = match request.send().await {
                Ok(response) => response,
                Err(err) => {
                    send_disconnect_error(
                        &event_tx,
                        &shutdown_requested,
                        format!("failed to connect to OpenCode SSE stream: {err}"),
                    );
                    return;
                }
            };
            if let Err(err) = response.error_for_status_ref() {
                send_disconnect_error(
                    &event_tx,
                    &shutdown_requested,
                    format!("OpenCode SSE stream failed: {err}"),
                );
                return;
            }

            let mut parser = SseParser::default();
            let mut state = opencode_mapping::ReaderState::new(reader_session_id.clone());

            loop {
                if shutdown_requested.load(Ordering::Relaxed) {
                    break;
                }

                let chunk = match response.chunk().await {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        tracing::warn!(error = %err, "opencode SSE chunk read failed");
                        break;
                    }
                };
                let Some(chunk) = chunk else {
                    break;
                };

                for payload in parser.push(&chunk) {
                    if payload.trim().is_empty() || payload.trim() == "[DONE]" {
                        continue;
                    }

                    let parsed = serde_json::from_str::<Value>(&payload);
                    let raw = match parsed {
                        Ok(raw) => raw,
                        Err(err) => {
                            tracing::debug!(error = %err, payload = %payload, "invalid SSE data");
                            continue;
                        }
                    };

                    let mapped = opencode_mapping::map_event(&raw, &mut state);
                    for event in mapped.events {
                        match &event {
                            SessionEvent::TurnStarted { .. } => {
                                turn_in_progress.store(true, Ordering::SeqCst)
                            }
                            SessionEvent::TurnCompleted { .. } => {
                                turn_in_progress.store(false, Ordering::SeqCst)
                            }
                            _ => {}
                        }
                        let _ = event_tx.send(event);
                    }

                    for request_id in mapped.permission_requests {
                        if let Err(err) = approve_permission(
                            &client,
                            &reader_base_url,
                            &reader_session_id,
                            &request_id,
                        )
                        .await
                        {
                            tracing::warn!(
                                request_id = %request_id,
                                error = %err,
                                "failed to auto-approve OpenCode permission request"
                            );
                        }
                    }
                }
            }

            turn_in_progress.store(false, Ordering::SeqCst);
            send_disconnect_error(
                &event_tx,
                &shutdown_requested,
                "OpenCode event stream disconnected",
            );
        });

        let stderr_task = spawn_stderr_logger(stderr, "lfd::sessions::opencode");

        let opencode_pid = child.id();
        if let Some(pid) = opencode_pid {
            if let Err(err) = opencode_runtime::register_opencode_server(pid) {
                tracing::warn!(
                    opencode_pid = pid,
                    error = %err,
                    "failed to register OpenCode server runtime metadata"
                );
            }
        }

        self.child = Some(child);
        self.stderr_task = Some(stderr_task);
        self.sse_task = Some(sse_task);
        self.server_base_url = Some(base_url);
        self.provider_session_id = Some(provider_session_id);
        Ok(())
    }
}

#[async_trait]
impl Harness for OpenCodeHarness {
    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }

        self.shutdown_requested.store(false, Ordering::SeqCst);
        self.config = Some(config.clone());
        self.should_seed_prompt = true;

        let start_result = self.start_inner(config).await;
        if let Err(err) = start_result {
            let _ = self.stop().await;
            return Err(err);
        }
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| anyhow!("opencode harness not started"))?;

        let first_turn = self.should_seed_prompt;
        let turn_content = build_turn_content(content, config, first_turn);
        let Some(turn_content) = turn_content else {
            return Ok(());
        };

        if self
            .turn_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(HarnessError::TurnAlreadyInProgress.into());
        }
        let mut turn_guard = TurnInProgressGuard::new(self.turn_in_progress.clone());

        let base_url = self
            .server_base_url
            .clone()
            .ok_or_else(|| anyhow!("opencode server not started"))?;
        let provider_session_id = self
            .provider_session_id
            .clone()
            .ok_or_else(|| anyhow!("opencode provider session id is not available"))?;

        let payload = build_turn_payload(&turn_content, config, first_turn);

        let message_url = format!("{base_url}/session/{provider_session_id}/message");
        send_request_with_retry(&self.client, Method::POST, &message_url, Some(payload)).await?;

        self.should_seed_prompt = false;
        turn_guard.disarm();
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);

        if let (Some(base_url), Some(provider_session_id)) =
            (&self.server_base_url, &self.provider_session_id)
        {
            let abort_url = format!("{base_url}/session/{provider_session_id}/abort");
            let _ =
                send_request_with_retry(&self.client, Method::POST, &abort_url, Some(json!({})))
                    .await;

            let delete_url = format!("{base_url}/session/{provider_session_id}");
            let _ = send_request_with_retry(&self.client, Method::DELETE, &delete_url, None).await;
        }

        let opencode_pid = self.child.as_ref().and_then(|child| child.id());
        if let Some(child) = self.child.as_mut() {
            shutdown_child(child).await;
        }
        self.child = None;
        if let Some(pid) = opencode_pid {
            if let Err(err) = opencode_runtime::unregister_opencode_server(pid) {
                tracing::warn!(
                    opencode_pid = pid,
                    error = %err,
                    "failed to unregister OpenCode server runtime metadata"
                );
            }
        }

        if let Some(task) = self.sse_task.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
            let _ = task.await;
        }

        self.turn_in_progress.store(false, Ordering::SeqCst);
        self.provider_session_id = None;
        self.server_base_url = None;

        Ok(())
    }

    fn set_provider_session_id(&mut self, provider_session_id: Option<String>) {
        self.provider_session_id = provider_session_id;
    }
}

async fn shutdown_child(child: &mut Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

async fn create_provider_session(client: &reqwest::Client, base_url: &str) -> Result<String> {
    let session_url = format!("{base_url}/session");
    let response =
        send_request_with_retry(client, Method::POST, &session_url, Some(json!({}))).await?;
    let body: Value = response
        .json()
        .await
        .map_err(|err| anyhow!("failed to parse opencode session response: {err}"))?;

    parse_session_id(&body).ok_or_else(|| {
        anyhow!(
            "opencode session response did not include session id: {}",
            body
        )
    })
}

fn send_disconnect_error(
    event_tx: &mpsc::UnboundedSender<SessionEvent>,
    shutdown_requested: &AtomicBool,
    message: impl Into<String>,
) {
    if shutdown_requested.load(Ordering::Relaxed) {
        return;
    }

    let _ = event_tx.send(SessionEvent::Error {
        code: OPENCODE_DISCONNECTED_CODE.to_string(),
        message: message.into(),
    });
}

fn allocate_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|err| anyhow!("failed to allocate port for OpenCode: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| anyhow!("failed to read allocated OpenCode port: {err}"))?
        .port();
    Ok(port)
}

async fn wait_for_server(
    client: &reqwest::Client,
    base_url: &str,
    child: &mut Child,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut delay = Duration::from_millis(100);

    loop {
        if let Some(exit_status) = child
            .try_wait()
            .map_err(|err| anyhow!("failed to poll opencode serve process: {err}"))?
        {
            return Err(anyhow!(
                "opencode serve exited before becoming ready: {exit_status}"
            ));
        }

        if client.get(base_url).send().await.is_ok() {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for opencode serve health check at {base_url}"
            ));
        }

        tokio::time::sleep(delay).await;
        delay = std::cmp::min(delay * 2, Duration::from_secs(1));
    }
}

async fn send_request_with_retry(
    client: &reqwest::Client,
    method: Method,
    url: &str,
    payload: Option<Value>,
) -> Result<reqwest::Response> {
    let mut attempt = 0;

    loop {
        let mut request = client.request(method.clone(), url);
        if let Some(body) = payload.clone() {
            request = request.json(&body);
        }

        match request.send().await {
            Ok(response) if response.status().is_server_error() && attempt == 0 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Ok(response) => {
                return response
                    .error_for_status()
                    .map_err(|err| anyhow!("OpenCode request failed ({method} {url}): {err}"));
            }
            Err(err) if attempt == 0 && (err.is_timeout() || err.is_connect()) => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(err) => {
                return Err(anyhow!("OpenCode request failed ({method} {url}): {err}"));
            }
        }
    }
}

async fn approve_permission(
    client: &reqwest::Client,
    base_url: &str,
    session_id: &str,
    request_id: &str,
) -> Result<()> {
    let url = format!("{base_url}/session/{session_id}/permissions/{request_id}");
    let payload = json!({ "response": "always" });
    let _ = send_request_with_retry(client, Method::POST, &url, Some(payload)).await?;
    Ok(())
}

fn parse_session_id(value: &Value) -> Option<String> {
    value
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn build_turn_content(content: &str, config: &AgentConfig, first_turn: bool) -> Option<String> {
    if first_turn {
        let mut parts = Vec::new();
        if !config.task_prompt.trim().is_empty() {
            parts.push(config.task_prompt.trim().to_string());
        }
        if !content.trim().is_empty() {
            parts.push(content.trim().to_string());
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    } else {
        let text = content.trim();
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
}

fn build_turn_payload(content: &str, config: &AgentConfig, first_turn: bool) -> Value {
    let mut payload = json!({
        "parts": [
            { "type": "text", "text": content }
        ]
    });

    if first_turn && !config.system_prompt.trim().is_empty() {
        payload["system"] = Value::String(config.system_prompt.trim().to_string());
    }

    payload
}

#[derive(Debug, Default)]
struct SseParser {
    buffer: String,
}

impl SseParser {
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
        if self.buffer.contains('\r') {
            self.buffer = self.buffer.replace("\r\n", "\n").replace('\r', "\n");
        }

        let mut events = Vec::new();
        while let Some(separator) = self.buffer.find("\n\n") {
            let frame = self.buffer[..separator].to_string();
            self.buffer.drain(..separator + 2);
            if let Some(data) = parse_data_frame(&frame) {
                events.push(data);
            }
        }
        events
    }
}

fn parse_data_frame(frame: &str) -> Option<String> {
    let mut data_lines = Vec::new();
    for line in frame.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            data_lines.push(data.trim_start().to_string());
        }
    }
    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_turn_content_includes_task_prompt_on_first_turn() {
        let content = build_turn_content(
            "",
            &AgentConfig {
                task_prompt: "task".to_string(),
                ..Default::default()
            },
            true,
        );
        assert_eq!(content.as_deref(), Some("task"));
    }

    #[test]
    fn sse_parser_collects_data_lines() {
        let mut parser = SseParser::default();
        let events = parser.push(b"event: message\ndata: {\"a\":1}\n\n");
        assert_eq!(events, vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn sse_parser_handles_split_crlf_frames() {
        let mut parser = SseParser::default();
        assert!(parser.push(b"data: {\"a\":").is_empty());
        let events = parser.push(b"1}\r\n\r\n");
        assert_eq!(events, vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn parse_session_id_requires_canonical_top_level_id() {
        assert_eq!(
            parse_session_id(&json!({"id": "session_1"})),
            Some("session_1".to_string())
        );
        assert_eq!(
            parse_session_id(&json!({"session": {"id": "session_2"}})),
            None
        );
        assert_eq!(parse_session_id(&json!({"sessionID": "session_3"})), None);
    }
}
