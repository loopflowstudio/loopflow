use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use reqwest::Method;
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{opencode_worktree_config, AgentConfig, AgentWriteScope};
use crate::chat::types::{ConversationEvent, FailureEvidence};
use crate::config::parse_agent;
use crate::harness::common::{spawn_stderr_logger, TurnInProgressGuard};
use crate::harness::{
    opencode_history, opencode_mapping, ApprovalPolicy, Harness, HarnessError, RawProviderEvent,
    SendCurrentOutcome,
};
use crate::id::AgentSessionId;

pub(crate) const OPENCODE_DISCONNECTED_CODE: &str = "opencode_disconnected";

pub struct OpenCodeHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
    client: reqwest::Client,
    history: Arc<Mutex<opencode_history::History>>,
    config: Option<AgentConfig>,
    should_seed_prompt: bool,
    turn_in_progress: Arc<AtomicBool>,
    shutdown_requested: Arc<AtomicBool>,
    child: Option<Child>,
    stderr_task: Option<JoinHandle<()>>,
    sse_task: Option<JoinHandle<()>>,
    server_base_url: Option<String>,
    agent_session: Option<AgentSessionId>,
}

impl std::fmt::Debug for OpenCodeHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenCodeHarness").finish()
    }
}

impl OpenCodeHarness {
    pub fn new(
        events: mpsc::UnboundedSender<ConversationEvent>,
        _approval: ApprovalPolicy,
    ) -> Self {
        Self {
            events,
            raw_provider: None,
            client: reqwest::Client::new(),
            history: Arc::new(Mutex::new(opencode_history::History::default())),
            config: None,
            should_seed_prompt: true,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            child: None,
            stderr_task: None,
            sse_task: None,
            server_base_url: None,
            agent_session: None,
        }
    }

    async fn start_inner(&mut self, config: &AgentConfig) -> Result<()> {
        let owner = super::agent_process::open_owner(config.session_attachment.as_ref())?;
        self.history = Arc::new(Mutex::new(opencode_history::History::new(Some(
            owner.clone(),
        ))));
        let port = allocate_port()?;
        let mut command = Command::new("opencode");
        command
            .arg("serve")
            .arg("--port")
            .arg(port.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            // Lifetime belongs to the lifeline; dropping a superseded harness
            // must never signal the provider now held by another attachment.
            .kill_on_drop(false);
        if let Some(cwd) = &config.cwd {
            command.current_dir(cwd);
        }
        super::configure_agent_env(&mut command, config);
        if config.write_scope == AgentWriteScope::Worktree {
            command.env("OPENCODE_CONFIG_CONTENT", opencode_worktree_config());
        }
        super::configure_vendor_std_env(command.as_std_mut())?;
        self.child = Some(super::agent_process::spawn(command, None, &owner)?);
        let child = self.child.as_mut().expect("admitted OpenCode child");
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing opencode stderr"))?;

        let base_url = format!("http://127.0.0.1:{port}");
        wait_for_server(&self.client, &base_url, child).await?;
        let agent_session =
            open_agent_session(&self.client, &base_url, self.agent_session.as_ref()).await?;
        // The dedicated server answers each native permission once, after the
        // originating user message has been selected under the Session owner.
        let mut permissions = vec![json!({"permission":"*","pattern":"*","action":"ask"})];
        if config.write_scope == AgentWriteScope::Worktree {
            permissions
                .push(json!({"permission":"external_directory","pattern":"*","action":"deny"}));
        }
        self.client
            .patch(format!("{base_url}/session/{agent_session}"))
            .json(&json!({"permission":permissions}))
            .send()
            .await?
            .error_for_status()?;
        let (store, session, attachment) = &owner;
        store.record_session_connection(session, attachment, &base_url, &agent_session)?;

        let event_tx = self.events.clone();
        let raw_provider = self.raw_provider.clone();
        let client = self.client.clone();
        let shutdown_requested = self.shutdown_requested.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let history = self.history.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let reader_base_url = base_url.clone();
        let reader_session_id = agent_session.clone();
        let reader_model = opencode_model(config)
            .map(|(provider_id, model_id)| format!("{provider_id}/{model_id}"))
            .or_else(|| config.agent.clone());
        let stream_started_at = chrono::Utc::now().timestamp_millis();

        let sse_task = tokio::spawn(async move {
            let stream_url = format!("{reader_base_url}/event");
            let request = client
                .get(&stream_url)
                .header(reqwest::header::ACCEPT, "text/event-stream");
            let mut response = match request.send().await {
                Ok(response) => response,
                Err(err) => {
                    let evidence = disconnect_evidence(
                        None,
                        stream_started_at,
                        "connection_failed",
                        Some(&format!("{err}")),
                    );
                    send_disconnect_error(
                        &event_tx,
                        &shutdown_requested,
                        format!("failed to connect to OpenCode SSE stream: {err}"),
                        Some(evidence),
                    );
                    return;
                }
            };
            if let Err(err) = response.error_for_status_ref() {
                let evidence = disconnect_evidence(
                    None,
                    stream_started_at,
                    "response_error_status",
                    Some(&format!("{err}")),
                );
                send_disconnect_error(
                    &event_tx,
                    &shutdown_requested,
                    format!("OpenCode SSE stream failed: {err}"),
                    Some(evidence),
                );
                return;
            }

            let _ = ready_tx.send(());
            let mut parser = SseParser::default();
            let mut state = opencode_mapping::ReaderState::new(
                reader_session_id.clone(),
                reader_model,
                "opencode",
            );

            // Track how the stream ended so the disconnect evidence names the
            // root cause class: `stream_eof` (clean EOF, chunk == None) vs
            // `read_error` (transport failure, chunk().await returned Err).
            let mut disconnect_class = "stream_eof";
            let mut disconnect_message: Option<String> = None;

            loop {
                if shutdown_requested.load(Ordering::Relaxed) {
                    break;
                }

                let chunk = match response.chunk().await {
                    Ok(chunk) => chunk,
                    Err(err) => {
                        tracing::warn!(error = %err, "opencode SSE chunk read failed");
                        disconnect_class = "read_error";
                        disconnect_message = Some(format!("{err}"));
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
                    if let Some(raw_provider) = &raw_provider {
                        let _ = raw_provider.send(RawProviderEvent {
                            stream: "sse",
                            line: payload.clone(),
                        });
                    }

                    let parsed = serde_json::from_str::<Value>(&payload);
                    let raw = match parsed {
                        Ok(raw) => raw,
                        Err(err) => {
                            tracing::debug!(error = %err, payload = %payload, "invalid SSE data");
                            continue;
                        }
                    };

                    history
                        .lock()
                        .expect("OpenCode history lock poisoned")
                        .attend(&reader_session_id, &raw);
                    let mapped = opencode_mapping::map_event(&raw, &mut state);
                    // SSE is a wake edge; native messages own request identity,
                    // completion and usage. Busy/idle cannot supply those facts.
                    if matches!(
                        raw["type"].as_str(),
                        Some(
                            "message.updated"
                                | "permission.asked"
                                | "session.idle"
                                | "session.status"
                                | "session.error"
                                | "message.part.updated"
                        )
                    ) {
                        let observation = async {
                            let messages = opencode_history::read_messages(&client, &reader_base_url, &reader_session_id).await?;
                            let events = history.lock().expect("OpenCode history lock poisoned").observe(&reader_session_id, &messages)?;
                            // Emit native-correlated output before its completion,
                            // even when a snapshot gets ahead of queued SSE deltas.
                            for event in events.iter().filter(|event| matches!(event, ConversationEvent::TurnStarted { .. })) {
                                state.observe_lifecycle(event);
                                let _ = event_tx.send(event.clone());
                            }
                            let current_messages: Vec<_> = messages.iter().filter(|message| message["info"]["parentID"].as_str().is_some_and(|request| history.lock().expect("OpenCode history lock poisoned").admitted(request))).cloned().collect();
                            for event in state.observe_messages(&current_messages) { let _ = event_tx.send(event); }
                            for event in events {
                                if !matches!(event, ConversationEvent::TurnStarted { .. }) { state.observe_lifecycle(&event); }
                                if matches!(event, ConversationEvent::TurnCompleted { .. }) { turn_in_progress.store(false, Ordering::SeqCst); }
                                if !matches!(event, ConversationEvent::TurnStarted { .. }) { let _ = event_tx.send(event); }
                            }
                            for request_id in &mapped.permission_requests {
                                let assistant = raw["properties"]["tool"]["messageID"].as_str()
                                    .ok_or_else(|| anyhow!("OpenCode permission has no originating assistant message"))?;
                                let request = messages.iter().find(|message| message["info"]["id"] == assistant)
                                    .and_then(|message| message["info"]["parentID"].as_str())
                                    .ok_or_else(|| anyhow!("OpenCode permission has no originating request"))?;
                                if !history.lock().expect("OpenCode history lock poisoned").admitted(request) {
                                    return Err(anyhow!("OpenCode permission belongs to an unselected request"));
                                }
                                let owner = history.lock().expect("OpenCode history lock poisoned").owner()?;
                                opencode_history::post(owner, format!("{reader_base_url}/permission/{request_id}/reply"), json!({"reply":"once"})).await?;
                            }
                            Ok::<_, anyhow::Error>(())
                        }.await;
                        if let Err(error) = observation {
                            send_disconnect_error(
                                &event_tx,
                                &shutdown_requested,
                                format!("OpenCode native history: {error}"),
                                None,
                            );
                            return;
                        }
                    }
                    for event in mapped.events {
                        let _ = event_tx.send(event);
                    }
                }
            }

            turn_in_progress.store(false, Ordering::SeqCst);

            // Disconnect is command failure; absent native completion stays unknown.
            let turn_was_open = state.turn_is_open();
            let turn_had_content = state.turn_has_content();

            // Phase-aware reason so the durable Failed record names where the
            // stream died, not just that it did. A mid-turn disconnect is
            // already failed by the consumer's Error handler; this makes the
            // evidence actionable (pre-content vs mid-stream truncation).
            let reason = if turn_was_open {
                if turn_had_content {
                    "OpenCode event stream disconnected mid-stream after partial output"
                } else {
                    "OpenCode event stream disconnected before the turn produced any output"
                }
            } else {
                "OpenCode event stream disconnected"
            };
            let evidence = disconnect_evidence(
                Some(&state),
                stream_started_at,
                disconnect_class,
                disconnect_message.as_deref(),
            );
            send_disconnect_error(&event_tx, &shutdown_requested, reason, Some(evidence));
        });

        let stderr_task = spawn_stderr_logger(stderr, "harness::opencode");

        self.stderr_task = Some(stderr_task);
        self.sse_task = Some(sse_task);
        self.server_base_url = Some(base_url);
        self.agent_session = Some(agent_session);
        ready_rx
            .await
            .map_err(|_| anyhow!("OpenCode event stream did not connect"))?;
        Ok(())
    }
}

#[async_trait]
impl Harness for OpenCodeHarness {
    fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    fn set_raw_provider_sender(
        &mut self,
        raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
    ) {
        self.raw_provider = raw_provider;
    }

    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }

        self.shutdown_requested.store(false, Ordering::SeqCst);
        self.config = Some(config.clone());
        self.should_seed_prompt = true;

        let start_result = self.start_inner(config).await;
        if let Err(err) = start_result {
            if let Err(cleanup) = self.stop().await {
                return Err(err.context(format!("OpenCode startup cleanup refused: {cleanup}")));
            }
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
        let agent_session = self
            .agent_session
            .clone()
            .ok_or_else(|| anyhow!("opencode provider session id is not available"))?;

        let mut payload = build_turn_payload(&turn_content, config, first_turn);
        let (request, owner) = {
            let mut history = self.history.lock().expect("OpenCode history lock poisoned");
            (history.request()?, history.owner()?)
        };
        payload["messageID"] = json!(request);

        // `prompt_async` enqueues the turn and returns immediately (204); the
        // turn's boundary and output arrive over the `/event` SSE stream. The
        // blocking `/message` endpoint holds the HTTP response open until the
        // whole turn finishes, which would keep `send_input` (and its `&mut
        // self` borrow) from returning — leaving no window to call
        // `send_current` mid-turn.
        let message_url = format!("{base_url}/session/{agent_session}/prompt_async");
        opencode_history::post(owner, message_url, payload).await?;

        self.should_seed_prompt = false;
        turn_guard.disarm();
        Ok(())
    }

    async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
        let text = content.trim();
        if text.is_empty() {
            return SendCurrentOutcome::Failed {
                error: "steer input is empty".to_string(),
            };
        }
        // A steer only lands in a live turn. Idle, or before the session is up,
        // it belongs in the next boundary's durable seed.
        if !self.turn_in_progress.load(Ordering::SeqCst) {
            return SendCurrentOutcome::NotSteerable;
        }
        let (Some(base_url), Some(agent_session), Some(config)) = (
            self.server_base_url.clone(),
            self.agent_session.clone(),
            self.config.clone(),
        ) else {
            return SendCurrentOutcome::NotSteerable;
        };

        let mut payload = build_turn_payload(text, &config, false);
        let (provider_turn_id, owner) = {
            let mut history = self.history.lock().expect("OpenCode history lock poisoned");
            match (history.request(), history.owner()) {
                (Ok(request), Ok(owner)) => (request, owner),
                (Err(error), _) | (_, Err(error)) => {
                    return SendCurrentOutcome::Failed {
                        error: error.to_string(),
                    }
                }
            }
        };
        payload["messageID"] = json!(provider_turn_id);
        let steer_url = format!("{base_url}/session/{agent_session}/prompt_async");
        match opencode_history::post(owner, steer_url, payload).await {
            Ok(()) => SendCurrentOutcome::Sent { provider_turn_id },
            Err(error) => SendCurrentOutcome::Failed {
                error: format!("failed to send opencode steer: {error}"),
            },
        }
    }

    async fn interrupt(&mut self) -> Result<()> {
        // Native history records the eventual outcome of this abort request.
        if !self.turn_in_progress.load(Ordering::SeqCst) {
            return Ok(());
        }
        let base_url = self
            .server_base_url
            .clone()
            .ok_or_else(|| anyhow!("opencode server not started"))?;
        let agent_session = self
            .agent_session
            .clone()
            .ok_or_else(|| anyhow!("opencode provider session id is not available"))?;

        let abort_url = format!("{base_url}/session/{agent_session}/abort");
        let owner = super::agent_process::open_owner(
            self.config
                .as_ref()
                .and_then(|config| config.session_attachment.as_ref()),
        )?;
        opencode_history::post(owner, abort_url, json!({})).await
    }

    async fn stop(&mut self) -> Result<()> {
        if self.child.is_some() || self.server_base_url.is_some() {
            let owner = super::agent_process::open_owner(
                self.config
                    .as_ref()
                    .and_then(|config| config.session_attachment.as_ref()),
            )?;
            super::agent_process::stop(&owner)?;
            self.child = None;
        }
        self.shutdown_requested.store(true, Ordering::SeqCst);

        if let Some(task) = self.sse_task.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
            let _ = task.await;
        }

        self.turn_in_progress.store(false, Ordering::SeqCst);
        // Keep `agent_session`: the runner persists it after stop so the
        // next launch can resume the session (see `open_agent_session`).
        self.server_base_url = None;

        Ok(())
    }

    fn agent_session(&self) -> Option<AgentSessionId> {
        self.agent_session.clone()
    }

    fn process_group_id(&self) -> Option<u32> {
        // Admission makes this child the leader of its own group.
        self.pid().filter(|pid| *pid > 1)
    }

    fn set_agent_session(&mut self, agent_session: Option<AgentSessionId>) {
        self.agent_session = agent_session;
    }
}

/// A saved conversation must remain the same conversation after a retry.
async fn open_agent_session(
    client: &reqwest::Client,
    base_url: &str,
    stored: Option<&AgentSessionId>,
) -> Result<AgentSessionId> {
    if let Some(session_id) = stored {
        client
            .get(format!("{base_url}/session/{session_id}"))
            .send()
            .await?
            .error_for_status()?;
        return Ok(session_id.clone());
    }

    let session_url = format!("{base_url}/session");
    let response =
        send_request_with_retry(client, Method::POST, &session_url, Some(json!({}))).await?;
    let body: Value = response
        .json()
        .await
        .map_err(|err| anyhow!("failed to parse opencode session response: {err}"))?;

    parse_session_id(&body)
        .ok_or_else(|| anyhow!("opencode session response did not include session id: {body}"))
}

fn send_disconnect_error(
    event_tx: &mpsc::UnboundedSender<ConversationEvent>,
    shutdown_requested: &AtomicBool,
    message: impl Into<String>,
    evidence: Option<FailureEvidence>,
) {
    if shutdown_requested.load(Ordering::Relaxed) {
        return;
    }

    let _ = event_tx.send(ConversationEvent::Error {
        code: OPENCODE_DISCONNECTED_CODE.to_string(),
        message: message.into(),
        evidence,
    });
}

/// Redact credential material from an error string before it enters the
/// durable evidence channel. Strips `Authorization` header values, `Bearer`
/// tokens, and common token query parameters. The opencode `/event` URL is
/// `http://127.0.0.1:<port>/event` (no credentials), but a chained/upstream
/// error or redirect could carry auth material in its Display.
pub(crate) fn sanitize_error_message(message: &str) -> String {
    use regex::Regex;
    let mut out = message.to_string();
    // Bearer first so `Authorization: Bearer <token>` is caught by both the
    // bearer pattern and the authorization-header pattern. The bearer pattern
    // stops at whitespace or a comma so the trailing comma that delimits the
    // authorization-header value survives as a stop boundary for the next
    // pattern — otherwise `Bearer <tok>, token=abc123` lets the authorization
    // regex eat through `token=abc123` before the param redaction runs.
    let redactions: [(Regex, &str); 3] = [
        (
            Regex::new(r"(?i)bearer\s+[^,\s]+").unwrap(),
            "bearer [redacted]",
        ),
        (
            Regex::new(r"(?i)authorization:\s*[^\n,]+").unwrap(),
            "authorization: [redacted]",
        ),
        (
            Regex::new(r"(?i)(token|key|access_token|api_key)=\S+").unwrap(),
            "$1=[redacted]",
        ),
    ];
    for (re, replacement) in &redactions {
        out = re.replace_all(&out, *replacement).to_string();
    }
    out
}

/// Build disconnect evidence for the harness's own `/event` SSE stream dropping.
fn disconnect_evidence(
    state: Option<&opencode_mapping::ReaderState>,
    stream_started_at: i64,
    terminal_error_class: &str,
    terminal_error_message: Option<&str>,
) -> FailureEvidence {
    let stream_ended_at = chrono::Utc::now().timestamp_millis();
    FailureEvidence {
        model: state
            .and_then(opencode_mapping::ReaderState::model)
            .map(ToString::to_string),
        provider: state
            .map(opencode_mapping::ReaderState::provider)
            .map(ToString::to_string),
        endpoint_class: Some("harness_event_stream".to_string()),
        stream_started_at: Some(stream_started_at),
        stream_ended_at: Some(stream_ended_at),
        duration_ms: Some(stream_ended_at - stream_started_at),
        last_event_type: state
            .and_then(opencode_mapping::ReaderState::last_event_type)
            .map(ToString::to_string),
        last_event_seq: state.and_then(opencode_mapping::ReaderState::last_event_seq),
        terminal_error_class: Some(terminal_error_class.to_string()),
        terminal_error_message: terminal_error_message.map(sanitize_error_message),
        provider_output_tokens: None,
    }
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

fn parse_session_id(value: &Value) -> Option<AgentSessionId> {
    value
        .get("id")
        .and_then(Value::as_str)
        .map(AgentSessionId::from)
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

    if let Some((provider_id, model_id)) = opencode_model(config) {
        payload["model"] = json!({
            "providerID": provider_id,
            "modelID": model_id,
        });
    }

    payload
}

fn opencode_model(config: &AgentConfig) -> Option<(String, String)> {
    let agent_str = config.agent.as_deref()?;
    let (harness, variant) = parse_agent(agent_str);
    if harness != "opencode" {
        return None;
    }
    let variant = variant?;
    let (provider_id, model_id) = variant.split_once('/')?;
    if provider_id.is_empty() || model_id.is_empty() {
        return None;
    }
    Some((provider_id.to_string(), model_id.to_string()))
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

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Isolate the disposable store selection.
    async fn stop_and_drop_preserve_a_superseding_attachment() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN", "PATH"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let database = home.path().join("loopflow.db");
        let store = crate::store::sqlite::SqliteStore::open_ephemeral(&database).unwrap();
        let sql = rusqlite::Connection::open(&database).unwrap();
        let process = crate::id::LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&process],
        )
        .unwrap();
        store.test_session("opencode", &crate::session_record::new_artifact_key());
        sql.execute(
            "UPDATE agent_sessions SET interactive=0, provider='opencode'",
            [],
        )
        .unwrap();
        let first = store
            .claim_session_attachment("opencode", None, &process, true)
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = OpenCodeHarness::new(tx, ApprovalPolicy::AutoApprove);
        let config = AgentConfig {
            session_attachment: Some(("opencode".into(), first.clone())),
            ..Default::default()
        };
        // Exercise the actual launch configuration, including its drop policy.
        // Cancel startup after admission but before this stand-in serves HTTP.
        let script = home.path().join("opencode");
        std::fs::write(&script, "#!/bin/sh\nexec /bin/sleep 60\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut paths = vec![home.path().to_path_buf()];
        paths.extend(std::env::split_paths(&original_path));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let (pid, birth) = tokio::select! {
            result = harness.start(&config) => panic!("stand-in unexpectedly finished startup: {result:?}"),
            identity = async {
                tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if let Some(identity) = store.agent_process_identity("opencode").unwrap() {
                            break identity;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await.unwrap()
            } => identity,
        };
        assert_eq!(harness.pid(), Some(pid));
        let replacement = store
            .claim_session_attachment("opencode", Some(&first), &process, false)
            .unwrap();
        assert!(harness.stop().await.is_err());
        assert_eq!(harness.pid(), Some(pid));
        // Stale abort must refuse before attempting HTTP.
        harness.turn_in_progress.store(true, Ordering::SeqCst);
        harness.server_base_url = Some("http://127.0.0.1:1".into());
        harness.agent_session = Some("native".into());
        let error = harness.interrupt().await.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<crate::store::StoreError>(),
            Some(crate::store::StoreError::InvalidAuthority(_))
        ));
        drop(harness);
        assert_eq!(
            crate::journal::process_identity_evidence(pid, birth),
            crate::journal::ProcessIdentityEvidence::Live
        );
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut current = OpenCodeHarness::new(tx, ApprovalPolicy::AutoApprove);
        current.config = Some(AgentConfig {
            session_attachment: Some(("opencode".into(), replacement)),
            ..Default::default()
        });
        current.server_base_url = Some("http://127.0.0.1:1".into());
        current.stop().await.unwrap();
        assert!(current.server_base_url.is_none());
        assert!(!crate::journal::OsProcess::group_is_alive(pid).unwrap());
        assert!(store
            .process(&first.agent_process_id)
            .unwrap()
            .unwrap()
            .completed_at
            .is_some());
    }
    #[test]
    fn sanitize_error_message_redacts_credentials() {
        let input = "request to https://api.example.com/v1/chat?api_key=sk-secret123 failed: \
                     Authorization: Bearer super-secret-token, token=abc123";
        let sanitized = sanitize_error_message(input);
        assert!(
            !sanitized.contains("sk-secret123"),
            "api_key leaked: {sanitized}"
        );
        assert!(
            !sanitized.contains("super-secret-token"),
            "bearer token leaked: {sanitized}"
        );
        assert!(
            !sanitized.contains("abc123"),
            "token param leaked: {sanitized}"
        );
        assert!(sanitized.contains("api_key=[redacted]"));
        assert!(sanitized.contains("authorization: [redacted]"));
        assert!(sanitized.contains("token=[redacted]"));
    }

    #[test]
    fn sanitize_error_message_preserves_non_credential_text() {
        let input = "chunk stream ended (reqwest: EOF)";
        let sanitized = sanitize_error_message(input);
        assert_eq!(sanitized, input);
    }

    #[test]
    fn build_turn_content_includes_task_prompt_on_first_turn() {
        let content = build_turn_content(
            "",
            &AgentConfig {
                task_prompt: "task".to_string(),
                skill_invocation: None,
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
            Some("session_1".into())
        );
        assert_eq!(
            parse_session_id(&json!({"session": {"id": "session_2"}})),
            None
        );
        assert_eq!(parse_session_id(&json!({"sessionID": "session_3"})), None);
    }

    #[test]
    fn build_turn_payload_includes_explicit_opencode_model() {
        let payload = build_turn_payload(
            "hello",
            &AgentConfig {
                agent: Some("opencode:moonshotai/kimi-k2".to_string()),
                ..Default::default()
            },
            false,
        );
        assert_eq!(
            payload.get("model"),
            Some(&json!({
                "providerID": "moonshotai",
                "modelID": "kimi-k2"
            }))
        );
    }

    #[test]
    fn build_turn_payload_omits_model_for_non_opencode_agent_model() {
        let payload = build_turn_payload(
            "hello",
            &AgentConfig {
                agent: Some("claude:sonnet".to_string()),
                ..Default::default()
            },
            false,
        );
        assert!(payload.get("model").is_none());
    }

    #[test]
    fn build_turn_payload_uses_provider_default_for_bare_opencode() {
        let payload = build_turn_payload(
            "hello",
            &AgentConfig {
                agent: Some("opencode".to_string()),
                ..Default::default()
            },
            false,
        );
        assert!(payload.get("model").is_none());
    }

    // -- Fake-SSE disconnect matrix --

    use crate::chat::types::{ConversationItem, Lifecycle};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn open_agent_session_creates_only_without_saved_identity() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{port}");

        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).to_string();
                let response = if request.starts_with("GET /session/live") {
                    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}"
                } else if request.starts_with("POST /session ") {
                    "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\n{\"id\":\"new\"}"
                } else {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"
                };
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        let client = reqwest::Client::new();
        assert_eq!(
            open_agent_session(&client, &base_url, Some(&"live".into()))
                .await
                .unwrap(),
            "live".into()
        );
        assert!(open_agent_session(&client, &base_url, Some(&"gone".into()))
            .await
            .is_err());
        assert_eq!(
            open_agent_session(&client, &base_url, None).await.unwrap(),
            "new".into()
        );
    }

    // -- Live checks against the real `opencode serve` --
    //
    // Ignored by default; they spawn `opencode serve` and drive a real model,
    // so they need the `opencode` CLI on PATH and configured credentials (the
    // OpenCode Zen default, `opencode/glm-5.2`). Run explicitly with:
    //   cargo test -p loopflow --lib opencode::tests::live_ -- --ignored --nocapture

    fn live_config() -> AgentConfig {
        AgentConfig {
            chrome: false,
            session_attachment: None,
            system_prompt: String::new(),
            task_prompt: String::new(),
            skill_invocation: None,
            agent: Some("opencode".to_string()),
            cwd: Some(std::env::temp_dir()),
            max_turns: None,
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: AgentWriteScope::Configured,
            execution_boundary: None,
            skip_permissions: false,
            structured_replies: Vec::new(),
            directive_relay: None,
            env: Default::default(),
        }
    }

    /// Drain events up to the first `TurnCompleted`, accumulating assistant
    /// text, and return `(status, text)`.
    async fn drive_turn(
        rx: &mut mpsc::UnboundedReceiver<ConversationEvent>,
    ) -> (Lifecycle, String) {
        let mut text = String::new();
        loop {
            match tokio::time::timeout(Duration::from_secs(180), rx.recv()).await {
                Ok(Some(ConversationEvent::TextDelta { content, .. })) => text.push_str(&content),
                Ok(Some(ConversationEvent::ItemCompleted {
                    item: ConversationItem::Message { text: t, .. },
                    ..
                })) => text.push_str(&t),
                Ok(Some(ConversationEvent::TurnCompleted { status, .. })) => return (status, text),
                Ok(Some(_)) => {}
                Ok(None) => panic!("event channel closed before TurnCompleted"),
                Err(_) => panic!("timed out waiting for a turn"),
            }
        }
    }

    /// Assert no further `TurnCompleted` arrives within a short window — proof
    /// that the coalesced boundary was the only one for the `send_input`.
    async fn assert_no_more_completions(rx: &mut mpsc::UnboundedReceiver<ConversationEvent>) {
        loop {
            match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
                Ok(Some(ConversationEvent::TurnCompleted { .. })) => {
                    panic!("a second TurnCompleted arrived; the steer was not coalesced")
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => return,
            }
        }
    }

    #[tokio::test]
    #[ignore = "drives the real opencode serve; needs opencode CLI + credentials"]
    async fn live_basic_turn_completes() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = OpenCodeHarness::new(tx, ApprovalPolicy::AutoApprove);
        let mut config = live_config();
        let _ledger = super::super::admit_for_test(&mut config);
        harness.start(&config).await.expect("start");

        harness
            .send_input("Reply with exactly: ALPHA")
            .await
            .expect("seed turn");
        let (status, text) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert!(text.to_uppercase().contains("ALPHA"), "turn text: {text:?}");

        harness.stop().await.expect("stop");
    }

    #[tokio::test]
    #[ignore = "drives the real opencode serve; needs opencode CLI + credentials"]
    async fn live_send_current_coalesces_into_one_boundary() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = OpenCodeHarness::new(tx, ApprovalPolicy::AutoApprove);
        let mut config = live_config();
        let _ledger = super::super::admit_for_test(&mut config);
        harness.start(&config).await.expect("start");

        harness
            .send_input(
                "Write a slow, detailed 400-word essay about how a bicycle works. \
                 Take your time and be thorough.",
            )
            .await
            .expect("seed turn");
        // Inject a steer while the seed turn is still generating.
        tokio::time::sleep(Duration::from_secs(2)).await;
        let outcome = harness
            .send_current("IMPORTANT: also include the exact word PANGOLIN in your reply.")
            .await;
        assert!(
            matches!(outcome, SendCurrentOutcome::Sent { .. }),
            "steer accepted into the live turn: {outcome:?}"
        );

        // opencode keeps the session busy across the queued steer and emits one
        // idle, so the reader must produce exactly one TurnCompleted.
        let (status, text) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert!(
            text.to_uppercase().contains("PANGOLIN"),
            "the steer was incorporated: {text:?}"
        );
        assert_no_more_completions(&mut rx).await;
        assert!(!harness.turn_in_progress.load(Ordering::SeqCst));

        harness.stop().await.expect("stop");
    }
}
