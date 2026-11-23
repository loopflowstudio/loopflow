//! Codex app-server harness, targeting the codex-cli 0.142.5 protocol.
//!
//! Protocol shapes verified live (hand-driven session + probes) and against
//! `codex app-server generate-json-schema` (v2 bundle):
//! - `initialize {clientInfo}` -> response; the CLIENT then sends the
//!   `initialized` notification (there is no server-side "initialized").
//! - `thread/start {cwd, model?, approvalPolicy?, sandbox?}` -> response with
//!   `thread.id`; also mirrored as a `thread/started` notification.
//! - `turn/start {threadId, input: [{type:"text", text}]}`.
//! - `turn/interrupt {threadId, turnId}` -> `{}`; the turn then ends with
//!   `turn/completed` status "interrupted" (probed live).

use crate::harness::LaunchKind;
use crate::harness::Recording;
use crate::process::attachment;
use loopflow_model::id::AgentSessionId;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

use crate::agent::{build_codex_thread_start_params, AgentConfig};
use crate::harness::common::spawn_stderr_logger;
use crate::harness::fact_writer::FactWriter;
use crate::harness::{ApprovalPolicy, Driver as StreamDriver, HarnessError};
use crate::os_process::{
    agent_process_lifeline_path, hold_agent_process_lifeline, kill_process_group,
};
use crate::provider_auth::Provider;
use loopflow_harness::codex_decode::{self, CodexDecoder};
use loopflow_harness::fact::{Decoded, ProviderFact};
use loopflow_model::chat::types::ConversationEvent;

fn build_thread_request(
    launch: &AgentConfig,
    resume_agent_session: Option<&AgentSessionId>,
    mode: super::Mode,
) -> (&'static str, serde_json::Map<String, Value>) {
    let mut params = build_codex_thread_start_params(launch, mode);
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

/// What an AgentProcess started by `command` would inherit, including the
/// native configuration and any wrapper's overrides, read from a short-lived
/// stdio app-server started the same way.
async fn read_inherited_config(
    command: &std::process::Command,
    cwd: Option<&std::path::Path>,
) -> Result<Value> {
    let mut probe = Command::new(command.get_program());
    probe.args(command.get_args()).arg("app-server");
    for (name, value) in command.get_envs() {
        match value {
            Some(value) => probe.env(name, value),
            None => probe.env_remove(name),
        };
    }
    if let Some(directory) = command.get_current_dir() {
        probe.current_dir(directory);
    }
    let mut connection = super::codex_rpc::Connection::start(&mut probe)
        .await
        .map_err(|error| anyhow!("Codex config/read failed: {error}"))?;
    let inherited = connection
        .request_raw("config/read", json!({"cwd": cwd, "includeLayers": true}))
        .await
        .map_err(|error| anyhow!("Codex config/read failed: {error}"))?;
    if let Some(error) = inherited.get("error") {
        anyhow::bail!("Codex config/read failed: {error}");
    }
    let inherited = inherited["result"].clone();
    anyhow::ensure!(
        inherited["config"].is_object(),
        "Codex config/read returned no configuration"
    );
    Ok(inherited)
}

/// `-c key=value` pairs for dotted config keys; Codex parses each value as TOML.
pub(crate) fn config_overrides(config: &Value) -> Result<Vec<String>> {
    let Value::Object(entries) = config else {
        anyhow::bail!("Codex configuration is not a table");
    };
    let mut args = Vec::with_capacity(entries.len() * 2);
    for (key, value) in entries {
        args.push("-c".to_string());
        args.push(format!("{key}={}", toml_inline(value)?));
    }
    Ok(args)
}

/// JSON's escapes are TOML basic-string escapes; TOML also forbids raw DEL.
fn toml_string(text: &str) -> Result<String> {
    Ok(serde_json::to_string(text)?.replace('\u{7f}', "\\u007F"))
}

fn toml_inline(value: &Value) -> Result<String> {
    Ok(match value {
        Value::Null => anyhow::bail!("TOML has no null"),
        Value::Bool(_) | Value::Number(_) => value.to_string(),
        Value::String(text) => toml_string(text)?,
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(toml_inline)
                .collect::<Result<Vec<_>>>()?
                .join(",")
        ),
        Value::Object(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| Ok(format!("{}={}", toml_string(key)?, toml_inline(value)?)))
                .collect::<Result<Vec<_>>>()?
                .join(",")
        ),
    })
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

/// One connection's decoder and the writer of what it establishes, shared by
/// the client-to-server and server-to-client tasks.
#[derive(Debug)]
pub(super) struct CodexStream {
    pub decoder: CodexDecoder,
    pub writer: FactWriter,
}

impl CodexStream {
    pub fn new(writer: FactWriter) -> Self {
        Self {
            decoder: CodexDecoder::default(),
            writer,
        }
    }

    /// Decode and record a server message for the caller's known `thread`.
    pub(super) fn inbound(
        &mut self,
        rpc: &Value,
        thread: Option<&AgentSessionId>,
    ) -> (Vec<Decoded>, Result<()>) {
        let decoded = self.decoder.decode(rpc, thread);
        let written = self.writer.write(&decoded);
        (decoded, written)
    }

    /// Freeze a turn request's origin and record what the client sent.
    pub(super) fn outbound(&mut self, rpc: &Value) -> Result<()> {
        if let Some(request) = codex_decode::turn_request(rpc) {
            super::dispatch::off_reactor(|| self.writer.outbound(&request))?;
        }
        let decoded = self.decoder.outbound(rpc);
        self.writer.write(&decoded)
    }
}

pub struct Driver {
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
    agent_session: Arc<Mutex<Option<AgentSessionId>>>,
    resume_agent_session: Option<AgentSessionId>,
    /// Live turn id (from turn/started, cleared at turn/completed); steer and
    /// interrupt address the turn with it.
    current_turn_id: Arc<Mutex<Option<String>>>,
    /// Request id of the in-flight `initialize` call; the reader completes the
    /// handshake when the matching response arrives. 0 = none pending.
    initialize_request_id: Arc<AtomicI64>,
    /// Request id of the in-flight `thread/start` call; the reader mines the
    /// matching response for the vendor thread id. 0 = none pending.
    thread_start_request_id: Arc<AtomicI64>,
    launch: AgentConfig,
    should_seed_prompt: bool,
    /// Retain the group from spawn through failed startup, before `child` is
    /// installed. The harness alone mutates it; the lifeline covers lf death.
    child_group: Option<u32>,
    agent_directory: Option<tempfile::TempDir>,
    native_start: Option<(std::path::PathBuf, Value)>,
    owner: Recording,
    kind: LaunchKind,
}

impl std::fmt::Debug for Driver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Driver").finish()
    }
}

impl Driver {
    pub fn new(
        launch: &super::Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
        approval: ApprovalPolicy,
    ) -> Self {
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
            agent_session: Arc::new(Mutex::new(None)),
            resume_agent_session: launch.start.thread(),
            current_turn_id: Arc::new(Mutex::new(None)),
            initialize_request_id: Arc::new(AtomicI64::new(0)),
            thread_start_request_id: Arc::new(AtomicI64::new(0)),
            launch: launch.config.clone(),
            should_seed_prompt: true,
            child_group: None,
            agent_directory: None,
            native_start: None,
            owner: launch.recording.clone(),
            kind: launch.kind,
        }
    }

    pub(crate) fn native_start(&self) -> Option<(std::path::PathBuf, Value)> {
        self.native_start.clone()
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
impl StreamDriver for Driver {
    fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    fn process_group_id(&self) -> Option<u32> {
        self.child_group.filter(|pid| *pid > 1)
    }

    async fn start(&mut self) -> Result<()> {
        let config = self.launch.clone();
        let config = &config;
        let recording = self.owner.clone();
        let recording = &recording;
        if self.outbound_tx.is_some() {
            return Ok(());
        }
        self.shutdown_requested.store(false, Ordering::Relaxed);
        self.should_seed_prompt = true;
        if let Some(thread) = self.resume_agent_session.as_ref() {
            crate::transcript::adopt_native(Provider::Codex, thread)?;
        }
        *self
            .agent_session
            .lock()
            .expect("codex provider session id lock poisoned") = None;
        *self
            .current_turn_id
            .lock()
            .expect("codex turn id lock poisoned") = None;

        let start_result = self.start_inner(config, recording).await;
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
            .then(|| self.launch.skill_invocation.as_ref())
            .flatten();
        if text.is_empty() && invocation.is_none() {
            return Ok(());
        }
        if self.turn_in_progress.load(Ordering::Relaxed) {
            return Err(HarnessError::TurnAlreadyInProgress.into());
        }
        let turn_text = if self.should_seed_prompt {
            self.should_seed_prompt = false;
            {
                let launch = &self.launch;
                let mut parts = Vec::new();
                if !launch.task_prompt.trim().is_empty() {
                    parts.push(launch.task_prompt.trim().to_string());
                }
                parts.push(text.to_string());
                parts.join("\n\n")
            }
        } else {
            text.to_string()
        };

        let thread_id = self
            .agent_session()
            .ok_or_else(|| anyhow!("codex thread not started"))?;
        let mut input = Vec::new();
        if let Some(invocation) = invocation {
            input.push(json!({ "type": "text", "text": invocation.codex_prompt() }));
        }
        input.push(json!({ "type": "text", "text": turn_text }));

        let params = json!({ "threadId": thread_id, "input": input });
        self.send_request("turn/start", params).await?;
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

        if self.agent_session().is_some() || self.native_start.is_some() {
            // The invocation closes its AgentProcess under the attachment lock
            // during settlement, without holding SQLite across provider I/O.
            // Driver teardown only drops this connection after takeover.
            self.child.take();
            self.child_group = None;
            if let Some(directory) = self.agent_directory.take() {
                let _ = directory.keep();
            }
            self.shutdown_tasks().await;
            return Ok(());
        }

        let _ = self.interrupt().await;

        let terminate = || {
            if let Some(pid) = self.child_group.take() {
                kill_process_group(pid);
            }
            Ok(())
        };
        let Recording {
            store,
            session,
            attachment: expected,
        } = &self.owner;
        {
            if store
                .with_session_attachment(session, expected, terminate)
                .is_err()
            {
                self.child.take();
                if let Some(directory) = self.agent_directory.take() {
                    let _ = directory.keep();
                }
                self.shutdown_tasks().await;
                return Ok(());
            }
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.wait().await;
        }
        self.child = None;
        self.child_group = None;
        self.turn_in_progress.store(false, Ordering::Relaxed);

        self.shutdown_tasks().await;
        self.agent_directory.take();

        Ok(())
    }

    fn agent_session(&self) -> Option<AgentSessionId> {
        self.agent_session
            .lock()
            .expect("codex provider session id lock poisoned")
            .clone()
    }
}

impl Driver {
    /// lf's configuration for this AgentProcess's conversations, as dotted Codex
    /// config keys: shell policy and hooks, plus configured writable roots for
    /// the native client. Both extend what the server would otherwise inherit.
    async fn conversation_config(
        &self,
        command: &std::process::Command,
        launch: &AgentConfig,
        tool_environment: std::collections::BTreeMap<String, String>,
    ) -> Result<Value> {
        let mut config = json!({
            "shell_environment_policy.set": tool_environment,
            "allow_login_shell": false,
            "features.shell_snapshot": false,
        });
        let native = self.approval == ApprovalPolicy::NativeClient;
        let roots = if native {
            launch
                .cwd
                .as_deref()
                .map(crate::agent::workspace_add_dirs)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let inherited = if native && (!roots.is_empty() || launch.conversation_context.is_some()) {
            read_inherited_config(command, launch.cwd.as_deref()).await?
        } else {
            Value::Null
        };
        if !roots.is_empty() {
            let mut writable = inherited
                .pointer("/config/sandbox_workspace_write/writable_roots")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for root in roots {
                let root = json!(root);
                if !writable.contains(&root) {
                    writable.push(root);
                }
            }
            config["sandbox_workspace_write.writable_roots"] = json!(writable);
        }
        if let Some(context) = &launch.conversation_context {
            let session_hooks = inherited["layers"]
                .as_array()
                .and_then(|layers| {
                    layers
                        .iter()
                        .find(|layer| layer["name"]["type"] == "sessionFlags")
                })
                .and_then(|layer| layer["config"].get("hooks"));
            config["hooks"] = super::context::codex_hooks(context, session_hooks, self.kind)?;
        }
        Ok(config)
    }

    async fn start_inner(&mut self, launch: &AgentConfig, recording: &Recording) -> Result<()> {
        let owner = recording.clone();
        let Recording {
            store,
            session,
            attachment: _,
        } = &owner;
        let connection = attachment::session_connection(store, session)?;
        let saved_thread = store.session_thread(session)?;
        if let Some(thread) = &saved_thread {
            if self.resume_agent_session.as_ref() != Some(thread)
                && !(self.approval == ApprovalPolicy::NativeClient
                    && self.resume_agent_session.is_none())
            {
                anyhow::bail!(
                    "Saved conversation thread differs; reconnect with its recorded provider"
                );
            }
        }
        let directory = if connection.is_none() {
            // Codex refuses a socket parent other users could write to. A
            // umask of 002 (Ubuntu's default) would otherwise make it 0775.
            Some(
                tempfile::Builder::new()
                    .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
                    .tempdir()?,
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
                    .expect("new AgentProcess owns a directory")
                    .path()
                    .join("agent.sock")
            });
        let mut command = Command::new("codex");
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            // Failed startup owns this child; after connection, stop consults
            // the Session fence before deciding whether to terminate it.
            .kill_on_drop(false);
        super::configure_agent_env(&mut command, launch);
        if let (ApprovalPolicy::NativeClient, None, Some(context)) =
            (self.approval, &connection, &launch.conversation_context)
        {
            // Hooks run on the app-server. A conversation the person starts
            // there with /new or /fork has no binding of its own and reads this.
            context.configure_hooks(command.as_std_mut())?;
        }
        super::configure_vendor_std_env(command.as_std_mut())?;
        // Codex's shell policy need not inherit arbitrary app-server environment.
        // Pass only explicit launch and freshly resolved lf executable/Machine
        // values as configuration.
        let tool_environment = super::conversation_environment(command.as_std(), launch);
        // Caller/capture provenance reaches tools only through that policy.
        crate::agent::remove_caller_session_env(command.as_std_mut());
        let native = self.approval == ApprovalPolicy::NativeClient;
        // A native client attached answers its own approvals; otherwise
        // nobody is there to.
        let mode = if native {
            super::Mode::Interactive
        } else {
            super::Mode::Headless
        };
        let (thread_method, mut thread_params) =
            build_thread_request(launch, self.resume_agent_session.as_ref(), mode);
        if native {
            // A native Session's AgentProcess is private to it, so lf's
            // configuration is the server's own: a conversation the person
            // starts with /new or /fork gets it too. A reconnect joins a server
            // that already has it.
            let instructions = thread_params.remove("developerInstructions");
            if connection.is_some() {
                // Rejoining a live server must keep the conversation's current tier.
                thread_params.remove("serviceTier");
            } else {
                let mut config = self
                    .conversation_config(command.as_std(), launch, tool_environment)
                    .await?;
                if let Some(tier) = launch.codex_service_tier {
                    config["service_tier"] = tier.as_str().into();
                }
                if let Some(instructions) = instructions {
                    config["developer_instructions"] = instructions;
                }
                // Before the subcommand, after any wrapper's own
                // overrides: a repeat after `app-server` would replace those.
                command.args(config_overrides(&config)?);
            }
        } else {
            let config = self
                .conversation_config(command.as_std(), launch, tool_environment)
                .await?;
            thread_params.insert("config".into(), config);
        }
        command
            // Subcommand, not flag: codex-cli >= 0.142 renamed `--app-server`
            // to `codex app-server` (verified against 0.142.5).
            .arg("app-server")
            .args(["--listen", &format!("unix://{}", endpoint.display())]);

        let lifeline = agent_process_lifeline_path(&endpoint);
        let mut child = if connection.is_none() {
            Some(super::agent_process::spawn(
                command,
                Some(&lifeline),
                &owner,
                self.kind,
            )?)
        } else {
            None
        };

        // The AgentProcess runs in its own group and deliberately survives this
        // harness, so no destructor or signal hook can be what stops it. The
        // lifeline ties it to the processes that drive it: when the last one
        // ends, by return, signal, panic or SIGKILL, the group is terminated.
        if let Some(pid) = child.as_ref().and_then(tokio::process::Child::id) {
            self.child_group = Some(pid);
        } else {
            // Reconnecting adopts the AgentProcess; one that predates lifelines has none.
            hold_agent_process_lifeline(&lifeline).map_err(|error| {
                anyhow!(
                    "Codex AgentProcess is stopping after its attached lf exited ({error}); retry"
                )
            })?;
        }

        self.agent_directory = directory;
        let socket = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if let Some(child) = &mut child {
                    if let Some(status) = child.try_wait()? {
                        // Codex says why it stopped; the person needs that line.
                        let mut stderr = String::new();
                        if let Some(mut pipe) = child.stderr.take() {
                            let _ = tokio::time::timeout(
                                Duration::from_secs(1),
                                tokio::io::AsyncReadExt::read_to_string(&mut pipe, &mut stderr),
                            )
                            .await;
                        }
                        return Err(anyhow!(
                            "codex app-server exited before opening its socket ({status}){}",
                            crate::os_process::exit_reason(&stderr)
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
        .await
        .map_err(|_| anyhow!("Codex app-server did not open its socket within 15s"))??;
        let socket = super::codex_connection::connect_upstream(socket).await?;
        let (mut writer, mut reader) = socket.split();
        let stderr = child.as_mut().and_then(|child| child.stderr.take());

        let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundRpc>(128);
        let authority = owner.clone();
        let writer_events = self.events.clone();
        let stream = Arc::new(tokio::sync::Mutex::new(CodexStream::new(FactWriter::new(
            owner.store.clone(),
            owner.session.clone(),
            Some(owner.attachment.clone()),
            Default::default(),
        ))));
        let writer_stream = stream.clone();
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
                let recorded = writer_stream.lock().await.outbound(&payload);
                if let Err(error) = recorded {
                    let _ = writer_events.send(ConversationEvent::Error {
                        code: "codex_dispatch_rejected".into(),
                        message: error.to_string(),
                        evidence: None,
                    });
                    break;
                }
                let message = Message::Text(payload.to_string().into());
                let Recording {
                    store,
                    session,
                    attachment: expected,
                } = authority.clone();
                let dispatched = tokio::task::spawn_blocking(move || {
                    let outcome = store.with_session_dispatch(&session, &expected, || {
                        super::dispatch::send_fenced(&mut writer, message)
                    });
                    (writer, outcome)
                })
                .await;
                let Ok((returned, outcome)) = dispatched else {
                    break;
                };
                writer = returned;
                if let Err(message) = outcome.map_err(|error| error.to_string()) {
                    let _ = writer_events.send(ConversationEvent::Error {
                        code: "codex_dispatch_rejected".into(),
                        message,
                        evidence: None,
                    });
                    break;
                }
            }
        });

        // The owned headless server loses its driver on exit. Settle its open
        // turn before the process records its own end.
        if child.is_some() && self.approval != ApprovalPolicy::NativeClient {
            let open_stream = Arc::downgrade(&stream);
            let Recording { store, session, .. } = owner.clone();
            crate::agent::register_interrupt_cleanup(move || {
                let Some(stream) = open_stream.upgrade() else {
                    return;
                };
                let thread = match store.session_thread(&session) {
                    Ok(Some(thread)) => thread,
                    Ok(None) => return,
                    Err(error) => {
                        tracing::warn!(%error, "could not read the interrupted Codex thread");
                        return;
                    }
                };
                let mut stream = stream.blocking_lock();
                if let Some(ended) = stream.decoder.interrupt_pending(thread) {
                    if let Err(error) = stream.writer.record(&[ended]) {
                        tracing::warn!(%error, "could not record Codex's interrupted turn");
                    }
                }
            });
        }

        let (initialized_tx, initialized_rx) = oneshot::channel::<()>();
        let (thread_id_tx, thread_id_rx) = oneshot::channel::<AgentSessionId>();
        let turn_in_progress = self.turn_in_progress.clone();
        let shutdown_requested = self.shutdown_requested.clone();
        let event_tx = self.events.clone();
        let approval_tx = outbound_tx.clone();
        let approval = self.approval;
        let agent_session = self.agent_session.clone();
        let current_turn_id = self.current_turn_id.clone();
        let initialize_request_id = self.initialize_request_id.clone();
        let thread_start_request_id = self.thread_start_request_id.clone();
        let reader_owner = owner.clone();
        let reader_task = tokio::spawn(async move {
            let mut initialized_tx = Some(initialized_tx);
            let mut thread_id_tx = Some(thread_id_tx);

            while let Some(Ok(message)) = reader.next().await {
                let Message::Text(line) = message else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let Recording {
                    store,
                    session,
                    attachment: _,
                } = &reader_owner;
                let decoded = {
                    let thread = super::dispatch::off_reactor(|| store.session_thread(session));
                    let (decoded, written) = stream
                        .lock()
                        .await
                        .inbound(&value, thread.as_ref().ok().and_then(Option::as_ref));
                    if let Err(error) = thread.map_err(anyhow::Error::from).and(written) {
                        let _ = event_tx.send(ConversationEvent::Error {
                            code: "conversation_history_unavailable".into(),
                            message: error.to_string(),
                            evidence: None,
                        });
                    }
                    decoded
                };
                for item in decoded {
                    match item {
                        Decoded::Display(event) => {
                            match &event {
                                ConversationEvent::TurnStarted { turn_id } => {
                                    turn_in_progress.store(true, Ordering::Relaxed);
                                    *current_turn_id.lock().expect("codex turn id lock poisoned") =
                                        Some(turn_id.clone());
                                }
                                ConversationEvent::TurnCompleted { .. } => {
                                    turn_in_progress.store(false, Ordering::Relaxed);
                                    *current_turn_id.lock().expect("codex turn id lock poisoned") =
                                        None;
                                }
                                _ => {}
                            }
                            let _ = event_tx.send(event);
                        }
                        Decoded::Fact(ProviderFact::SessionObserved(thread_id)) => {
                            *agent_session
                                .lock()
                                .expect("codex provider session id lock poisoned") =
                                Some(thread_id.clone());
                            if let Some(tx) = thread_id_tx.take() {
                                let _ = tx.send(thread_id);
                            }
                        }
                        Decoded::Fact(_) => {}
                    }
                }

                if value.get("method").is_some() {
                    // Any server request (method + id) is an approval request
                    // (item/commandExecution/requestApproval and friends); answer
                    // it per the configured Loopflow response policy. The user's
                    // Codex approval policy decides whether these requests occur.
                    if let Some(id) = value.get("id") {
                        let result = match approval {
                            ApprovalPolicy::AutoApprove => json!({ "decision": "accept" }),
                            ApprovalPolicy::NativeClient => continue,
                        };
                        let _ = approval_tx
                            .send(OutboundRpc::Response {
                                id: id.clone(),
                                result,
                            })
                            .await;
                    }
                    continue;
                }

                // Response frame.
                let id = value.get("id").and_then(Value::as_i64);
                if let Some(error) = value.get("error") {
                    let _ = event_tx.send(codex_decode::rpc_error(error));
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
                // The thread/start response may resume a turn already running.
                if id.is_some() && id == Some(thread_start_request_id.load(Ordering::Relaxed)) {
                    thread_start_request_id.store(0, Ordering::Relaxed);
                    if let Some(turn) = value
                        .pointer("/result/thread/turns")
                        .and_then(Value::as_array)
                        .and_then(|turns| turns.iter().find(|turn| turn["status"] == "inProgress"))
                    {
                        let turn = turn["id"].as_str().map(str::to_owned);
                        turn_in_progress.store(true, Ordering::Relaxed);
                        *current_turn_id.lock().expect("codex turn id lock poisoned") =
                            turn.clone();
                        stream.lock().await.decoder.resume_turn(turn);
                    }
                }
            }

            turn_in_progress.store(false, Ordering::Relaxed);
            if !shutdown_requested.load(Ordering::Relaxed) {
                let _ = event_tx.send(ConversationEvent::Error {
                    code: "codex_disconnected".to_string(),
                    message: "codex app-server disconnected".to_string(),
                    evidence: None,
                });
            }
        });

        let diagnostic = Arc::new(Mutex::new(String::new()));
        let stderr_task = stderr
            .map(|stderr| spawn_stderr_logger(stderr, "harness::codex", Some(diagnostic.clone())));

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
        let initialized = async {
            self.send_request("initialize", initialize).await?;
            tokio::time::timeout(Duration::from_secs(15), initialized_rx)
                .await
                .map_err(|_| anyhow!("timed out waiting for codex initialize response"))?
                .map_err(|_| anyhow!("codex initialize channel closed"))
        }
        .await;
        if let Err(error) = initialized {
            if let Some(mut task) = self.stderr_task.take() {
                if tokio::time::timeout(Duration::from_secs(1), &mut task)
                    .await
                    .is_err()
                {
                    task.abort();
                }
            }
            let diagnostic = diagnostic.lock().expect("stderr diagnostic lock poisoned");
            return Err(anyhow!(
                "{error}{}",
                crate::os_process::exit_reason(&diagnostic)
            ));
        }
        self.send_notification("initialized").await?;

        if self.approval == ApprovalPolicy::NativeClient {
            let Recording {
                store,
                session,
                attachment,
            } = &owner;
            store.record_agent_endpoint(session, attachment, &endpoint.to_string_lossy())?;
            self.native_start = Some((endpoint, Value::Object(thread_params)));
            return Ok(());
        }
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
                let Recording {
                    store,
                    session,
                    attachment,
                } = &owner;
                if let Some(context) = &launch.conversation_context {
                    store.with_session_attachment(session, attachment, || {
                        let mut context = context.clone();
                        context.identify(store, session)?;
                        Ok(context.bind_codex_session(thread_id.as_str()))
                    })??;
                }
                store.record_session_connection(
                    session,
                    attachment,
                    &endpoint.to_string_lossy(),
                    &thread_id,
                )?;
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

impl super::Launcher for loopflow_harness::Codex {
    fn validate_agent_process_close(
        &self,
        store: &loopflow_storage::sqlite::SqliteStore,
        session: &str,
        process: &loopflow_model::id::AgentProcessId,
    ) -> crate::store::StoreResult<()> {
        // The server may host conversations outside this AgentProcess.
        let endpoint = store.agent_endpoint(process)?.ok_or_else(|| {
            crate::store::StoreError::InvalidAuthority(
                "Codex AgentProcess connection is unavailable".into(),
            )
        })?;
        let thread = store.session_thread(session)?;
        super::codex_connection::validate_agent_process_close(&endpoint, thread.as_ref())
            .map_err(|error| crate::store::StoreError::InvalidAuthority(error.to_string()))
    }

    fn provider(&self) -> Option<crate::provider_auth::Provider> {
        Some(crate::provider_auth::Provider::Codex)
    }
    fn terminal_error(&self, code: &str) -> bool {
        matches!(code, "codex_disconnected" | "codex_dispatch_rejected")
    }
    fn classify_failure(&self, code: &str, message: &str) -> Option<crate::agent::AgentFailure> {
        crate::agent::provider_failure(code, message)
    }
    fn capabilities(&self) -> super::Capabilities {
        super::Capabilities {
            thread: super::ThreadNaming {
                bare: super::ThreadName::Reported,
                managed: super::ThreadName::Reported,
            },
            initial_prompt_on_resume: true,
            native_skills: Some(crate::skills::catalog::SkillDialect::Codex),
            installed_skills: super::InstalledSkills {
                global: ".agents/skills",
                repository: ".agents/skills",
                global_registry: None,
            },
            turn_limit: false,
            output_schema: false,
            respawns_after_crash: false,
            install_hint:
                "Install it with `npm install -g @openai/codex`, then sign in with `codex login`.",
        }
    }
    fn driver(
        &self,
        launch: &super::Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
        approval: super::ApprovalPolicy,
    ) -> Box<dyn super::Driver> {
        Box::new(Driver::new(launch, events, approval))
    }
    fn open(&self, launch: &super::Launch<'_>) -> Result<super::native_session::NativeClient> {
        let (mut args, input) = launch.terminal_input()?;
        let mut client = super::native_session::NativeClient::new(launch)?;
        if !launch.kind.is_managed() {
            args.extend(crate::agent::codex_terminal_options(
                launch.config,
                launch.kind,
            )?);
            if let super::Start::Resume { thread } = launch.start {
                args.extend(["resume".into(), "--".into(), thread.to_string()]);
            } else {
                args.push("--".into());
            }
            args.extend(input.filter(|input| !input.is_empty()));
            if let Some(context) = &launch.config.conversation_context {
                let delivery = context.configure_hooks(&mut client.process)?;
                if client.thread.is_none() {
                    client.reported_thread =
                        Some(crate::context_block::reported_thread_path(&delivery))
                            .filter(|path| std::fs::write(path, "").is_ok());
                }
            }
        } else {
            let mut config = launch.config.clone();
            config.task_prompt = input.unwrap_or_default();
            if matches!(launch.start, super::Start::New) {
                config.skill_invocation = None;
            }
            let opened = std::thread::scope(|scope| {
                scope
                    .spawn(|| {
                        tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()?
                            .block_on(open_native(&super::Launch {
                                config: &config,
                                ..*launch
                            }))
                    })
                    .join()
                    .map_err(|_| anyhow!("Codex startup worker panicked"))?
            })?;
            args = vec![
                "--remote".into(),
                format!("unix://{}", opened.endpoint.display()),
                "resume".into(),
                "--".into(),
                opened.thread.to_string(),
            ];
            client.thread = Some(opened.thread);
            client.remote = Some(opened.endpoint);
        }
        client.process.args(args);
        Ok(client)
    }
}

struct OpenedCodex {
    thread: AgentSessionId,
    endpoint: std::path::PathBuf,
}

/// Start the app-server and open the conversation with lf's configuration, so
/// the native client only joins it. Nothing here outlives the client's start.
async fn open_native(launch: &super::Launch<'_>) -> Result<OpenedCodex> {
    use crate::harness::{codex_connection, ApprovalPolicy};

    let Recording {
        store,
        session,
        attachment,
    } = launch.recording;
    let saved = launch.start.thread().or(store.session_thread(session)?);
    let (events, _events) = tokio::sync::mpsc::unbounded_channel();
    let launch_value = super::Launch {
        start: super::Start::from_thread(saved.as_ref()),
        ..*launch
    };
    let mut harness = Driver::new(&launch_value, events, ApprovalPolicy::NativeClient);

    match tokio::time::timeout(std::time::Duration::from_secs(20), harness.start()).await {
        Ok(result) => result?,
        Err(_) => {
            harness.stop().await?;
            bail!("Codex app-server did not open the conversation within 20s; the owned provider was stopped");
        }
    }
    let (endpoint, params) = harness
        .native_start()
        .ok_or_else(|| anyhow!("Codex startup ended without its native launch configuration"))?;
    harness.stop().await?;
    // Recover saved history before opening the UI, without replaying its input.
    let turns = match &saved {
        Some(thread) => tokio::time::timeout(
            std::time::Duration::from_secs(20),
            codex_connection::recover_history(store, session, thread, &endpoint),
        )
        .await
        .context("Codex native history recovery timed out")??,
        None => 0,
    };
    let title = store
        .session(session)?
        .map(|session| session.title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Loopflow".to_string());
    let thread = codex_connection::open_thread(&endpoint, params, &title).await?;
    store.record_session_connection(session, attachment, &endpoint.to_string_lossy(), &thread)?;
    let opening = turns == 0 && !launch.config.task_prompt.is_empty();
    if let Some(context) = &launch.config.conversation_context {
        // Hooks run on the app-server, which never inherits the terminal's
        // launch file. Bind before the client can send a turn.
        let mut context = context.clone();
        context.identify(store, session)?;
        context.bind_codex_session(thread.as_str())?;
        let adapter = store.reserve_steer_adapter(session, attachment)?;
        let delivery = crate::context_block::native_delivery_path(&context.home, thread.as_str());
        report_first_boundary(adapter, delivery, &thread, opening);
    }
    if opening {
        // Provider history settles on the next read whether it ran, so a
        // broken store cannot stop the turn.
        codex_connection::start_turn(
            store,
            session,
            attachment,
            &endpoint,
            &thread,
            &launch.config.task_prompt,
        )
        .await
        .context("Codex first turn was not sent")?;
    }
    Ok(OpenedCodex { thread, endpoint })
}

/// Codex runs its start hook only inside a turn, so a reopened conversation
/// would report nothing until someone types. The launch reports the process's
/// first boundary itself: a start, which lets the server wake the idle
/// conversation with steers saved while it was closed, or the opening prompt,
/// sent before its turn so the wake leaves a working conversation alone. A
/// server that does not answer is a recording gap: the steers wait for the
/// first typed turn.
fn report_first_boundary(
    adapter: crate::steer::SteerAdapter,
    delivery: std::path::PathBuf,
    thread: &AgentSessionId,
    opening: bool,
) {
    use crate::machine::wire::{BoundaryMoment, ServerOperation, SessionBoundary};
    let (moment, callback) = match opening {
        true => (
            BoundaryMoment::Tool,
            json!({"hook_event_name":"UserPromptSubmit","session_id":thread.as_str()}),
        ),
        false => (
            BoundaryMoment::Start,
            json!({"hook_event_name":"SessionStart","session_id":thread.as_str(),"source":"resume"}),
        ),
    };
    let operation = ServerOperation::SessionBoundary(SessionBoundary {
        moment,
        delivery,
        adapter: Some(adapter),
        callback,
        gap: None,
        printed: Vec::new(),
        inbox: None,
    });
    let answered = crate::machine::socket_path().and_then(|socket| {
        crate::machine::client::hook_request(
            &socket,
            operation,
            crate::machine::client::HOOK_DEADLINE,
        )
    });
    if let Err(error) = answered {
        tracing::warn!(error = %format!("{error:#}"), "Codex's first boundary unrecorded; waiting steers wait for its first turn");
    }
}

#[cfg(test)]
mod tests {
    use crate::process::attachment;

    use super::*;

    #[tokio::test]
    async fn native_skill_precedes_request_and_clipboard_without_continuation_replay() {
        use crate::skills::catalog::{SkillDialect, SkillSource};
        use crate::skills::invocation::SkillInvocation;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("SKILL.md");
        let source = "---\nname: probe\ndescription: Inspect the supplied asset\n---\nRead ./asset.txt before answering.\n";
        std::fs::write(&path, source).unwrap();
        std::fs::write(directory.path().join("asset.txt"), "Native asset.").unwrap();
        let skill = SkillSource {
            name: "probe".into(),
            path: Some(path.clone()),
            dialect: SkillDialect::Codex,
        }
        .load()
        .unwrap();
        let request = "Find the bug.\n\n<lf:clipboard>\nPasted evidence.\n</lf:clipboard>";
        let (events, _events) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Codex,
                &crate::agent::AgentConfig::default(),
                &crate::harness::admit_for_test(tempfile::tempdir().unwrap().path()),
            ),
            events,
            ApprovalPolicy::AutoApprove,
        );
        *harness.agent_session.lock().expect("thread id lock") = Some("thread_1".into());
        let (sender, mut outbound) = mpsc::channel(1);
        harness.outbound_tx = Some(sender);
        harness.launch = AgentConfig {
            skill_invocation: Some(SkillInvocation {
                skill,
                arguments: "exact \"two words\" 雪".into(),
            }),
            ..crate::agent::AgentConfig::default()
        };

        harness.send_input(request).await.unwrap();
        let OutboundRpc::Request { method, params, .. } = outbound.recv().await.unwrap() else {
            panic!("expected first turn request");
        };
        assert_eq!(method, "turn/start");
        assert_eq!(params["threadId"], "thread_1");
        assert_eq!(
            params["input"],
            json!([
                { "type": "text", "text": format!("[$probe]({}) exact \"two words\" 雪", path.display()) },
                { "type": "text", "text": request }
            ])
        );
        // The reference still selects the original declarations and adjacent assets.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
        assert_eq!(
            std::fs::read_to_string(path.parent().unwrap().join("asset.txt")).unwrap(),
            "Native asset."
        );

        harness.send_input("Now explain the fix.").await.unwrap();
        let OutboundRpc::Request { method, params, .. } = outbound.recv().await.unwrap() else {
            panic!("expected continuation request");
        };
        assert_eq!(method, "turn/start");
        assert_eq!(params["threadId"], "thread_1");
        assert_eq!(
            params["input"],
            json!([{ "type": "text", "text": "Now explain the fix." }])
        );
    }

    #[tokio::test]
    async fn failed_startup_retains_the_group_until_stop() {
        let mut child = Command::new("/bin/sleep")
            .env_clear()
            .arg("60")
            .process_group(0)
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        // Stopping signals the group only under the admitted attachment, so
        // its ledger outlives the driver.
        let home = tempfile::tempdir().unwrap();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Codex,
                &crate::agent::AgentConfig::default(),
                &crate::harness::admit_for_test(home.path()),
            ),
            tx,
            ApprovalPolicy::AutoApprove,
        );
        // Startup can fail before installing the child handle in the harness.
        harness.child_group = child.id();
        assert_eq!(harness.process_group_id(), child.id());
        harness.stop().await.unwrap();
        let status = tokio::time::timeout(Duration::from_secs(2), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(!status.success());
        assert_eq!(harness.process_group_id(), None);
    }

    #[test]
    fn saved_thread_rejection_precedes_spawn_and_leaves_the_conversation_resumable() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _binary = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let store =
            crate::store::opening::open_ephemeral(&ledger.home().join("loopflow.db")).unwrap();
        store.test_session("saved", &crate::session_record::new_artifact_key());
        let sql = rusqlite::Connection::open(ledger.home().join("loopflow.db")).unwrap();
        let process = loopflow_model::id::LfProcessId::new();
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [process.as_str()],
        )
        .unwrap();
        let old = attachment::claim_session_attachment(&store, "saved", None, &process).unwrap();
        store
            .record_session_connection("saved", &old, "/missing.sock", &"saved-thread".into())
            .unwrap();
        let attachment =
            attachment::claim_session_attachment(&store, "saved", Some(&old), &process).unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Codex,
                &crate::agent::AgentConfig::default(),
                &crate::harness::admit_for_test(tempfile::tempdir().unwrap().path()),
            ),
            tx,
            ApprovalPolicy::AutoApprove,
        );
        let config = AgentConfig {
            // Even a mistakenly reached spawn cannot launch a real provider.
            cwd: Some(ledger.home().join("absent")),
            ..crate::agent::AgentConfig::default()
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime
            .block_on(harness.start_inner(
                &config,
                &Recording {
                    store: store.clone(),
                    session: "saved".into(),
                    attachment: attachment.clone(),
                },
            ))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("Saved conversation thread differs"));
        let released = store
            .release_session_attachment("saved", &attachment)
            .unwrap();
        let retry =
            attachment::claim_session_attachment(&store, "saved", Some(&released), &process)
                .unwrap();
        harness.resume_agent_session = Some("saved-thread".into());
        let error = runtime
            .block_on(harness.start_inner(
                &config,
                &Recording {
                    store: store.clone(),
                    session: "saved".into(),
                    attachment: retry,
                },
            ))
            .unwrap_err();
        assert!(
            error.to_string().contains("failed to spawn codex"),
            "{error}"
        );
        assert_eq!(
            store.session_thread("saved").unwrap(),
            Some("saved-thread".into())
        );
        assert!(store
            .session_turns("saved")
            .unwrap()
            .iter()
            .all(|turn| turn.ended_at.is_none()));
    }

    #[test]
    fn start_and_resume_add_instructions_without_replacing_native_base() {
        let config = AgentConfig {
            system_prompt: "Fixed additions.".into(),
            ..crate::agent::AgentConfig::default()
        };
        let saved_session = AgentSessionId::from("saved-native-thread");
        for session in [None, Some(&saved_session)] {
            let (_, params) =
                build_thread_request(&config, session, crate::harness::Mode::Headless);
            assert_eq!(params["developerInstructions"], "Fixed additions.");
            assert!(!params.contains_key("baseInstructions"));
            assert!(!params.contains_key("model_instructions_file"));
        }
    }

    #[test]
    fn pinned_codex_session_uses_thread_resume() {
        let launch = AgentConfig {
            cwd: Some("/tmp/project".into()),
            ..crate::agent::AgentConfig::default()
        };
        let (method, params) = build_thread_request(
            &launch,
            Some(&"thread_abc".into()),
            crate::harness::Mode::Headless,
        );

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
        let (method, params) = build_thread_request(
            &crate::agent::AgentConfig::default(),
            None,
            crate::harness::Mode::Headless,
        );
        assert_eq!(method, "thread/start");
        assert!(!params.contains_key("threadId"));
    }
}
