use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{opencode_worktree_config, AgentConfig, AgentWriteScope};
use crate::chat::types::{ConversationEvent, FailureEvidence};
use crate::config::parse_agent;
use crate::harness::common::TurnInProgressGuard;
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
            sse_task: None,
            server_base_url: None,
            agent_session: None,
        }
    }

    fn disconnect(&mut self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        if let Some(task) = self.sse_task.take() {
            task.abort();
        }
        self.turn_in_progress.store(false, Ordering::SeqCst);
        // Keep native history available after detaching the reader.
        self.server_base_url = None;
    }

    async fn start_inner(&mut self, config: &AgentConfig) -> Result<()> {
        let owner = super::agent_process::open_owner(config.session_attachment.as_ref())?;
        self.history = Arc::new(Mutex::new(opencode_history::History::new(Some(
            owner.clone(),
        ))));
        let (store, session, attachment) = &owner;
        let endpoint = store.agent_process_endpoint(session)?;
        let base_url = if let Some(endpoint) = endpoint {
            if store.session_thread(session)?.as_ref() != self.agent_session.as_ref() {
                return Err(anyhow!(
                    "Saved OpenCode conversation differs; reconnect with its recorded provider"
                ));
            }
            // Startup consumes an already admitted attachment. Public attachment
            // must acquire custody before claiming, independently of this reader.
            let custody = crate::os_process::hold_agent_process_lifeline(
                &store.agent_process_lifeline_path(&attachment.agent_process_id)?,
            )?;
            let (pid, birth) = store
                .agent_process_identity(session)?
                .ok_or_else(|| anyhow!("OpenCode AgentProcess identity is unavailable"))?;
            store.with_session_attachment(session, attachment, || Ok(()))?;
            custody.retain(pid, birth);
            self.should_seed_prompt = false;
            endpoint
        } else {
            let port = allocate_port()?;
            // The provider owns this file descriptor, not a pipe reader in its
            // launcher. It remains writable through launcher death and takeover.
            use std::os::unix::fs::OpenOptionsExt;
            let stderr = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(
                    store
                        .agent_process_lifeline_path(&attachment.agent_process_id)?
                        .with_extension("stderr"),
                )?;
            let mut command = Command::new("opencode");
            command
                .arg("serve")
                .arg("--port")
                .arg(port.to_string())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(stderr)
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
            let base_url = format!("http://127.0.0.1:{port}");
            store.record_agent_process_endpoint(session, attachment, &base_url)?;
            self.child = Some(super::agent_process::spawn(command, &owner)?);
            let child = self.child.as_mut().expect("admitted OpenCode child");
            wait_for_server(&self.client, &base_url, child).await?;
            base_url
        };
        let agent_session = prepare_agent_session(
            owner.clone(),
            base_url.clone(),
            self.agent_session.clone(),
            config.write_scope,
        )
        .await?;

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

            let mut parser = SseParser::default();
            let mut state = opencode_mapping::ReaderState::new(
                reader_session_id.clone(),
                reader_model,
                "opencode",
            );

            // Subscribe first: changes during readback remain queued on SSE.
            // Native history, not another wake edge, recovers accepted requests.
            if let Err(error) = observe_native_messages(
                &client,
                &reader_base_url,
                &reader_session_id,
                &history,
                &mut state,
                &event_tx,
                &turn_in_progress,
            )
            .await
            {
                send_disconnect_error(
                    &event_tx,
                    &shutdown_requested,
                    format!("OpenCode native history: {error}"),
                    None,
                );
                return;
            }
            let _ = ready_tx.send(());

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
                        let observation = observe_native_messages(
                            &client,
                            &reader_base_url,
                            &reader_session_id,
                            &history,
                            &mut state,
                            &event_tx,
                            &turn_in_progress,
                        )
                        .await;
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
                    for event in mapped {
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

        self.sse_task = Some(sse_task);
        self.server_base_url = Some(base_url);
        self.agent_session = Some(agent_session);
        ready_rx
            .await
            .map_err(|_| anyhow!("OpenCode event stream did not connect"))?;
        Ok(())
    }
}

impl Drop for OpenCodeHarness {
    fn drop(&mut self) {
        self.disconnect();
    }
}

async fn observe_native_messages(
    client: &reqwest::Client,
    base_url: &str,
    session: &AgentSessionId,
    history: &Mutex<opencode_history::History>,
    state: &mut opencode_mapping::ReaderState,
    event_tx: &mpsc::UnboundedSender<ConversationEvent>,
    turn_in_progress: &AtomicBool,
) -> Result<()> {
    let snapshot = opencode_history::read_snapshot(client, base_url, session).await?;
    let (events, current_messages, owner) = {
        let mut history = history.lock().expect("OpenCode history lock poisoned");
        let events = history.observe(session, &snapshot.messages)?;
        let current_messages: Vec<_> = snapshot
            .messages
            .iter()
            .filter(|message| {
                message["info"]["parentID"]
                    .as_str()
                    .is_some_and(|request| history.admitted(request))
            })
            .collect();
        (events, current_messages, history.owner()?)
    };
    // Emit native-correlated output between start and completion, even when
    // a snapshot gets ahead of queued SSE deltas. Empty snapshots do not idle
    // an in-flight submission.
    let (starts, remaining): (Vec<_>, Vec<_>) = events
        .into_iter()
        .partition(|event| matches!(event, ConversationEvent::TurnStarted { .. }));
    for event in starts {
        state.observe_lifecycle(&event);
        turn_in_progress.store(true, Ordering::SeqCst);
        let _ = event_tx.send(event);
    }
    for event in state.observe_messages(current_messages) {
        let _ = event_tx.send(event);
    }
    for event in remaining {
        state.observe_lifecycle(&event);
        if matches!(event, ConversationEvent::TurnCompleted { .. }) {
            turn_in_progress.store(false, Ordering::SeqCst);
        }
        let _ = event_tx.send(event);
    }
    snapshot
        .reply_pending_permissions(base_url, session, &owner)
        .await
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
        if self.server_base_url.is_some() {
            return Ok(());
        }

        self.shutdown_requested.store(false, Ordering::SeqCst);
        self.config = Some(config.clone());
        self.should_seed_prompt = true;

        let start_result = self.start_inner(config).await;
        if let Err(err) = start_result {
            // No child exists before reachability is saved. Once admitted,
            // startup can have uncertain native effects: detach, never erase them.
            self.disconnect();
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
            let history = self.history.lock().expect("OpenCode history lock poisoned");
            (history.request(&agent_session)?, history.owner()?)
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
            let history = self.history.lock().expect("OpenCode history lock poisoned");
            match (history.request(&agent_session), history.owner()) {
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
            .as_ref()
            .ok_or_else(|| anyhow!("opencode server not started"))?;
        let agent_session = self
            .agent_session
            .as_ref()
            .ok_or_else(|| anyhow!("opencode provider session id is not available"))?;

        let abort_url = format!("{base_url}/session/{agent_session}/abort");
        let owner = self
            .history
            .lock()
            .expect("OpenCode history lock poisoned")
            .owner()?;
        opencode_history::post(owner, abort_url, json!({})).await
    }

    async fn stop(&mut self) -> Result<()> {
        if self.child.is_some() || self.server_base_url.is_some() {
            let owner = self
                .history
                .lock()
                .expect("OpenCode history lock poisoned")
                .owner()?;
            super::agent_process::stop(&owner)?;
            self.child = None;
        }
        self.disconnect();

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

/// One startup writer survives caller cancellation and holds frozen authority
/// through HTTP and persistence. Readback can settle configuration, never repeat
/// an uncertain native creation or permission write.
async fn prepare_agent_session(
    owner: super::agent_process::AttachmentOwner,
    base_url: String,
    stored: Option<AgentSessionId>,
    write_scope: AgentWriteScope,
) -> Result<AgentSessionId> {
    opencode_history::with_attached_http(owner, move |(store, session, attachment), client| {
        let thread = if let Some(thread) = store.session_thread(session)?.or(stored) {
            thread
        } else {
            let (first_attempt, _) = store.record_agent_process_startup_attempt(
                session,
                attachment,
                "opencode-create",
                &json!({}),
            )?;
            if !first_attempt {
                return Err(anyhow!("OpenCode native Session creation is uncertain; not creating another conversation"));
            }
            let body: Value = client
                .post(format!("{base_url}/session"))
                .json(&json!({}))
                .send()?
                .error_for_status()?
                .json()?;
            parse_session_id(&body)
                .ok_or_else(|| anyhow!("OpenCode creation returned no native Session identity"))?
        };
        // Retain identity before permission I/O: losing that response must
        // not lose the conversation which the server already created.
        store.record_session_connection(session, attachment, &base_url, &thread)?;
        let mut permissions = vec![json!({"permission":"*","pattern":"*","action":"ask"})];
        if write_scope == AgentWriteScope::Worktree {
            permissions.push(json!({
                "permission":"external_directory", "pattern":"*", "action":"deny"
            }));
        }
        let url = format!("{base_url}/session/{thread}");
        let observed: Value = client.get(&url).send()?.error_for_status()?.json()?;
        let (first_attempt, permissions) = store.record_agent_process_startup_attempt(
            session,
            attachment,
            "opencode-permissions",
            &json!(permissions),
        )?;
        if observed["permission"] != permissions {
            if !first_attempt {
                return Err(anyhow!("OpenCode permission configuration is uncertain; not replaying"));
            }
            client
                .patch(&url)
                .json(&json!({"permission":permissions}))
                .send()?
                .error_for_status()?;
        }
        Ok(thread)
    })
    .await
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
    async fn startup_retains_creation_and_permissions_after_cancellation_and_takeover() {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        for failure in [
            "creation",
            "permissions-applied",
            "permissions-unapplied",
            "cancelled-creation",
            "cancelled-permissions",
        ] {
            let home = tempfile::tempdir().unwrap();
            let database = home.path().join("loopflow.db");
            let store = crate::store::sqlite::SqliteStore::open_ephemeral(&database).unwrap();
            let sql = rusqlite::Connection::open(&database).unwrap();
            let launcher = crate::id::LfProcessId::new();
            let attacher = crate::id::LfProcessId::new();
            for id in [&launcher, &attacher] {
                sql.execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                    [id],
                )
                .unwrap();
            }
            store.test_session("opencode", &crate::session_record::new_artifact_key());
            let first = store
                .claim_session_attachment("opencode", None, &launcher, true)
                .unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            store
                .record_agent_process_endpoint("opencode", &first, &endpoint)
                .unwrap();
            let creations = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let patches = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let accepted = Arc::new(tokio::sync::Notify::new());
            let release = Arc::new(tokio::sync::Notify::new());
            let server = {
                let accepted = accepted.clone();
                let release = release.clone();
                let creations = creations.clone();
                let patches = patches.clone();
                tokio::spawn(async move {
                    let mut configured = false;
                    loop {
                        let (socket, _) = listener.accept().await.unwrap();
                        let mut socket = BufReader::new(socket);
                        let mut request = String::new();
                        socket.read_line(&mut request).await.unwrap();
                        let mut length = 0;
                        loop {
                            let mut line = String::new();
                            socket.read_line(&mut line).await.unwrap();
                            if line == "\r\n" {
                                break;
                            }
                            if let Some(value) = line.to_lowercase().strip_prefix("content-length:")
                            {
                                length = value.trim().parse::<usize>().unwrap();
                            }
                        }
                        let mut body = vec![0; length];
                        socket.read_exact(&mut body).await.unwrap();
                        let response = if request.starts_with("POST /session ") {
                            creations.fetch_add(1, Ordering::SeqCst);
                            if failure == "cancelled-creation" {
                                accepted.notify_one();
                                release.notified().await;
                            }
                            if failure == "creation" { continue; }
                            json!({"id":"native"})
                        } else if request.starts_with("PATCH /session/native ") {
                            patches.fetch_add(1, Ordering::SeqCst);
                            assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(),
                                json!({"permission":[{"permission":"*","pattern":"*","action":"ask"}]}));
                            configured = failure != "permissions-unapplied";
                            if failure == "cancelled-permissions" {
                                accepted.notify_one();
                                release.notified().await;
                            }
                            if failure.starts_with("permissions-") {
                                // The response is lost whether the mutation applied or not.
                                continue;
                            }
                            json!({"id":"native"})
                        } else {
                            assert!(request.starts_with("GET /session/native "));
                            json!({"id":"native","permission":if configured {
                                json!([{"permission":"*","pattern":"*","action":"ask"}])
                            } else { json!([]) }})
                        }.to_string();
                        socket.get_mut().write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).as_bytes()).await.unwrap();
                    }
                })
            };
            let startup = tokio::spawn(prepare_agent_session(
                (store.clone(), "opencode".into(), first.clone()),
                endpoint.clone(),
                None,
                AgentWriteScope::Configured,
            ));
            let transferred = if failure.starts_with("cancelled-") {
                tokio::time::timeout(Duration::from_secs(5), accepted.notified())
                    .await
                    .expect("server accepted startup mutation");
                startup.abort();
                assert!(startup.await.unwrap_err().is_cancelled());
                let (started_tx, started_rx) = tokio::sync::oneshot::channel();
                let mut transfer = {
                    // A separately opened store exercises the OS fence, not a
                    // shared SQLite connection's mutex.
                    let store =
                        crate::store::sqlite::SqliteStore::open_ephemeral(&database).unwrap();
                    let first = first.clone();
                    let attacher = attacher.clone();
                    tokio::task::spawn_blocking(move || {
                        started_tx.send(()).unwrap();
                        let current = store
                            .claim_session_attachment("opencode", Some(&first), &attacher, false)
                            .unwrap();
                        // The response must commit before the new owner enters.
                        assert_eq!(
                            store.session_thread("opencode").unwrap(),
                            Some("native".into())
                        );
                        current
                    })
                };
                started_rx.await.unwrap();
                assert!(
                    tokio::time::timeout(Duration::from_millis(100), &mut transfer)
                        .await
                        .is_err(),
                    "cancelling the caller released an in-flight mutation's fence"
                );
                assert_eq!(
                    store.session_attachment("opencode").unwrap(),
                    Some(first.clone())
                );
                release.notify_one();
                Some(
                    tokio::time::timeout(Duration::from_secs(5), transfer)
                        .await
                        .unwrap()
                        .unwrap(),
                )
            } else {
                assert!(startup.await.unwrap().is_err());
                None
            };
            drop(store);
            let store = crate::store::sqlite::SqliteStore::open_ephemeral(&database).unwrap();
            assert_eq!(
                store.agent_process_endpoint("opencode").unwrap(),
                Some(endpoint.clone())
            );
            assert_eq!(
                store.session_thread("opencode").unwrap(),
                if failure == "creation" {
                    None
                } else {
                    Some("native".into())
                }
            );
            let current = transferred.unwrap_or_else(|| {
                store
                    .claim_session_attachment("opencode", Some(&first), &attacher, false)
                    .unwrap()
            });
            assert_eq!(current.agent_process_id, first.agent_process_id);
            let result = prepare_agent_session(
                (store.clone(), "opencode".into(), current),
                endpoint.clone(),
                None,
                AgentWriteScope::Worktree,
            )
            .await;
            assert_eq!(
                result.is_ok(),
                failure == "permissions-applied" || failure.starts_with("cancelled-")
            );
            assert!(prepare_agent_session(
                (store, "opencode".into(), first),
                endpoint,
                None,
                AgentWriteScope::Configured
            )
            .await
            .is_err());
            assert_eq!(creations.load(Ordering::SeqCst), 1);
            assert_eq!(
                patches.load(Ordering::SeqCst),
                usize::from(failure != "creation")
            );
            server.abort();
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Isolate the disposable store selection.
    async fn reconnect_recovers_pending_input_without_spawning_or_replaying() {
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
        std::fs::write(&script, format!("#!/bin/sh\nwhile [ ! -f '{}' ]; do /bin/sleep 0.02; done\nprintf 'after detach\\n' >&2\nexec /bin/sleep 60\n", home.path().join("detached").display())).unwrap();
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
        let thread: AgentSessionId = "native".into();
        let request = harness.history.lock().unwrap().request(&thread).unwrap();
        // No SSE event follows connection: native readback alone must recover
        // this request, accepted before the launcher observed any output.
        let messages = json!([{"info":{"id":"assistant", "sessionID":"native",
            "parentID":request, "role":"assistant", "time":{"created":1}},
            "parts":[]}])
        .to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let subscribed = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let fail_readback = Arc::new(AtomicBool::new(true));
        let permission_pending = Arc::new(AtomicBool::new(true));
        // Publish the assistant while permissions are being acquired. Output
        // must use that later snapshot too, without requiring another SSE edge.
        let messages_available = Arc::new(AtomicBool::new(false));
        let server = {
            let subscribed = subscribed.clone();
            let requests = requests.clone();
            let fail_readback = fail_readback.clone();
            let permission_pending = permission_pending.clone();
            let messages_available = messages_available.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
                let mut clients = tokio::task::JoinSet::new();
                loop {
                    let (socket, _) = listener.accept().await.unwrap();
                    let subscribed = subscribed.clone();
                    let requests = requests.clone();
                    let messages = messages.clone();
                    let fail_readback = fail_readback.clone();
                    let permission_pending = permission_pending.clone();
                    let messages_available = messages_available.clone();
                    clients.spawn(async move {
                        let mut socket = BufReader::new(socket);
                        let mut line = String::new();
                        socket.read_line(&mut line).await.unwrap();
                        requests.lock().unwrap().push(line.clone());
                        if line.starts_with("GET /session/native ") {
                            let body = json!({"id":"native","permission":[{"permission":"*","pattern":"*","action":"ask"}]}).to_string();
                            socket.get_mut().write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
                            return;
                        }
                        if line.starts_with("GET /event ") {
                            subscribed.store(true, Ordering::SeqCst);
                            socket.get_mut().write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n").await.unwrap();
                            std::future::pending::<()>().await;
                        } else {
                            assert!(subscribed.load(Ordering::SeqCst));
                            if line.starts_with("POST /permission/pending/reply ") {
                                assert!(permission_pending.swap(false, Ordering::SeqCst));
                                socket.get_mut().write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ntrue").await.unwrap();
                                return;
                            }
                            assert!(line.starts_with("GET /session/native/message ") || line.starts_with("GET /permission "));
                            let messages = if line.starts_with("GET /permission ") {
                                if !fail_readback.load(Ordering::SeqCst) {
                                    messages_available.store(true, Ordering::SeqCst);
                                }
                                if permission_pending.load(Ordering::SeqCst) {
                                    json!([{"id":"pending","sessionID":"native","tool":{"messageID":"assistant"}}]).to_string()
                                } else { "[]".to_string() }
                            } else if messages_available.load(Ordering::SeqCst) { messages } else { "[]".to_string() };
                            let status = if fail_readback.load(Ordering::SeqCst) { "503 Unavailable" } else { "200 OK" };
                            socket.get_mut().write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", messages.len(), messages).as_bytes()).await.unwrap();
                        }
                    });
                }
            })
        };
        store
            .record_session_connection("opencode", &first, &endpoint, &thread)
            .unwrap();
        let replacement = store
            .claim_session_attachment("opencode", Some(&first), &process, false)
            .unwrap();
        // Changing next-launch configuration cannot refresh this harness's authority.
        harness.config.as_mut().unwrap().session_attachment =
            Some(("opencode".into(), replacement.clone()));
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
        // The old pipe reader is gone, but stderr is still a provider-owned file.
        let stderr_path = store
            .agent_process_lifeline_path(&first.agent_process_id)
            .unwrap()
            .with_extension("stderr");
        assert_eq!(
            std::fs::metadata(&stderr_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        std::fs::write(home.path().join("detached"), "").unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while std::fs::read_to_string(&stderr_path).unwrap() != "after detach\n" {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut current = OpenCodeHarness::new(tx, ApprovalPolicy::AutoApprove);
        let config = AgentConfig {
            session_attachment: Some(("opencode".into(), replacement.clone())),
            ..Default::default()
        };
        // Wrong native selection refuses without killing the surviving provider.
        assert!(current.start(&config).await.is_err());
        assert_eq!(
            store.agent_process_identity("opencode").unwrap(),
            Some((pid, birth))
        );
        current.set_agent_session(Some(thread));
        assert!(
            tokio::time::timeout(Duration::from_secs(5), current.start(&config))
                .await
                .unwrap()
                .is_err()
        );
        assert!(current.server_base_url.is_none());
        assert_eq!(
            crate::journal::process_identity_evidence(pid, birth),
            crate::journal::ProcessIdentityEvidence::Live
        );
        assert_eq!(
            store.session_connection("opencode").unwrap().unwrap().0,
            endpoint
        );
        while rx.try_recv().is_ok() {}
        fail_readback.store(false, Ordering::SeqCst);
        tokio::time::timeout(Duration::from_secs(5), current.start(&config))
            .await
            .unwrap()
            .unwrap();
        assert!(current.child.is_none(), "reconnect must not spawn");
        assert!(
            !current.should_seed_prompt,
            "reconnect must not seed the old prompt"
        );
        assert!(current.turn_in_progress.load(Ordering::SeqCst));
        assert!(
            matches!(rx.try_recv().unwrap(), ConversationEvent::TurnStarted { turn_id } if turn_id == request)
        );
        assert!(
            requests
                .lock()
                .unwrap()
                .iter()
                .all(|request| request.starts_with("GET ")
                    || request.starts_with("POST /permission/pending/reply ")),
            "recovery must not replay input or reconfigure the server"
        );
        assert!(!permission_pending.load(Ordering::SeqCst));
        assert_eq!(
            store.agent_process_identity("opencode").unwrap(),
            Some((pid, birth))
        );
        assert_eq!(replacement.agent_process_id, first.agent_process_id);
        server.abort();
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
