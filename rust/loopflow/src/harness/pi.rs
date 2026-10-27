//! Pi 1.3 RPC transport. Pi owns models, credentials and its JSONL conversation.
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::agent_process::AttachmentOwner;
use super::fact::{Decoded, ProviderFact, ProviderTurn};
use super::fact_writer::FactWriter;
use super::pi_decode::Turn;
use super::{Harness, HarnessError, SendCurrentOutcome};
use crate::agent::AgentConfig;
use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::id::AgentSessionId;
use crate::process::SessionAttachment;
use crate::steer::NativeBoundary;

#[derive(Debug)]
struct Response {
    value: Value,
    // Snapshot in stream order: a later settlement cannot change the receipt.
    turn: Option<String>,
}

#[derive(Debug, Default)]
struct State {
    turn: Option<Turn>,
    /// Writes what this process's turns establish; present once started.
    writer: Option<FactWriter>,
    responses: HashMap<String, oneshot::Sender<Response>>,
    dead: bool,
}

impl State {
    /// Record a turn's facts and hand back its display events.
    fn record(&mut self, decoded: Vec<Decoded>) -> Result<Vec<ConversationEvent>> {
        self.writer
            .as_mut()
            .ok_or_else(|| anyhow!("Pi has not started"))?
            .record(&decoded)?;
        Ok(decoded
            .into_iter()
            .filter_map(|decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            })
            .collect())
    }
}

#[derive(Debug)]
pub struct PiHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    state: Arc<Mutex<State>>,
    attachment: Option<(String, SessionAttachment)>,
    session: Option<AgentSessionId>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader: Option<JoinHandle<()>>,
    stderr: Option<JoinHandle<()>>,
    seed: Option<String>,
}

/// Pi accepts missing paths as new sessions. Validate retained files before launch.
pub(crate) fn validate_resume(token: &AgentSessionId) -> Result<()> {
    let path = Path::new(token.as_str());
    anyhow::ensure!(
        path.is_absolute() && path.is_file(),
        "Pi conversation file is missing: {token}; restore it before resuming"
    );
    let file = std::fs::File::open(path)?;
    let line = std::io::BufReader::new(file)
        .lines()
        .next()
        .transpose()?
        .unwrap_or_default();
    let header: Value = serde_json::from_str(&line).context("read Pi conversation header")?;
    anyhow::ensure!(
        header["type"] == "session" && header["id"].as_str().is_some(),
        "Invalid Pi conversation file: {token}"
    );
    Ok(())
}

pub(crate) fn options(config: &AgentConfig, context_file: Option<&Path>) -> Result<Vec<String>> {
    anyhow::ensure!(config.write_scope != crate::agent::AgentWriteScope::Worktree || config.execution_boundary.is_some(), "Pi has no provider-only worktree sandbox; use a managed Task in an OS environment with the required boundary");
    let mut args = Vec::new();
    if let Some(model) = crate::config::parse_agent(config.agent()).1 {
        args.extend(["--model".into(), model]);
    }
    if let Some(path) = context_file {
        args.extend(["--append-system-prompt".into(), path.display().to_string()]);
    }
    // Project trust is separate from tool permissions; retain the user's policy.
    Ok(args)
}

impl PiHarness {
    pub fn new(events: mpsc::UnboundedSender<ConversationEvent>) -> Self {
        Self {
            events,
            state: Arc::default(),
            attachment: None,
            session: None,
            child: None,
            stdin: None,
            reader: None,
            stderr: None,
            seed: None,
        }
    }

    fn owner(&self) -> Result<AttachmentOwner> {
        super::agent_process::open_owner(self.attachment.as_ref())
    }

    async fn write(&mut self, value: &Value) -> Result<()> {
        let (store, session, attachment) = self.owner()?;
        let mut stdin = self
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Pi stdin is closed"))?;
        let line = format!("{value}\n");
        let (stdin, result) = tokio::task::spawn_blocking(move || {
            let result = store.with_session_dispatch(&session, &attachment, || {
                super::dispatch::write_fenced(&mut stdin, line.as_bytes())
            });
            (stdin, result)
        })
        .await?;
        self.stdin = Some(stdin);
        result?;
        Ok(())
    }

    async fn request_observed(&mut self, mut value: Value) -> Result<Response> {
        let id = uuid::Uuid::new_v4().to_string();
        value["id"] = json!(id);
        let (tx, rx) = oneshot::channel();
        self.state
            .lock()
            .expect("Pi state lock poisoned")
            .responses
            .insert(id.clone(), tx);
        let result = async {
            self.write(&value).await?;
            tokio::time::timeout(Duration::from_secs(15), rx)
                .await
                .context("Pi RPC response timed out (requires Pi 1.3 or later)")?
                .context("Pi exited before responding")
        }
        .await;
        self.state
            .lock()
            .expect("Pi state lock poisoned")
            .responses
            .remove(&id);
        result
    }

    async fn request(&mut self, value: Value) -> Result<Value> {
        let command = value["type"].as_str().unwrap_or("command").to_owned();
        let response = self.request_observed(value).await?.value;
        anyhow::ensure!(
            response["success"] == true,
            "Pi {}: {}",
            command,
            response["error"].as_str().unwrap_or("command failed")
        );
        Ok(response["data"].clone())
    }

    async fn start_process(&mut self, config: &AgentConfig) -> Result<()> {
        let owner = self.owner()?;
        if let Some(token) = self.session.as_ref().or(config.resume_token.as_ref()) {
            validate_resume(token)?;
        }
        let context = crate::agent::write_system_prompt_file(config, "pi")?;
        let mut command = Command::new("pi");
        command
            .args(["--mode", "rpc"])
            .args(options(config, context.as_deref())?);
        if let Some(token) = self.session.as_ref().or(config.resume_token.as_ref()) {
            command.args(["--session", token.as_str()]);
        }
        command.args(super::context::pi_args(
            config.conversation_context.as_ref(),
            false,
        )?);
        super::configure_agent_env(&mut command, config);
        if let Some(context) = &config.conversation_context {
            context.configure_hooks(command.as_std_mut())?;
        }
        super::configure_vendor_std_env(command.as_std_mut())?;
        if let Some(cwd) = &config.cwd {
            command.current_dir(cwd);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = super::agent_process::spawn(command, None, &owner)
            .with_context(|| crate::agent::missing_agent_message("pi"))?;
        let (store, session, attachment) = owner;
        self.state.lock().expect("Pi state lock poisoned").writer = Some(FactWriter::new(
            store,
            session,
            Some(attachment),
            None,
            Default::default(),
        ));
        self.stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("Pi stdout was piped");
        self.stderr = child
            .stderr
            .take()
            .map(|stderr| super::common::spawn_stderr_logger(stderr, "pi"));
        self.child = Some(child);
        self.reader = Some(self.spawn_reader(stdout));
        let info = self.request(json!({"type":"get_state"})).await?;
        let file = info["sessionFile"]
            .as_str()
            .filter(|f| Path::new(f).is_absolute())
            .ok_or_else(|| anyhow!("Pi did not report a persistent conversation file"))?;
        if let Some(expected) = self.session.as_ref().or(config.resume_token.as_ref()) {
            anyhow::ensure!(
                file == expected.as_str(),
                "Pi opened a different conversation than requested"
            );
        }
        self.session = Some(file.into());
        self.state
            .lock()
            .expect("Pi state lock poisoned")
            .record(vec![ProviderFact::SessionObserved(file.into()).into()])?;
        Ok(())
    }

    fn spawn_reader(&self, stdout: ChildStdout) -> JoinHandle<()> {
        let state = self.state.clone();
        let events = self.events.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let outcome: Result<()> = async {
                while let Some(line) = lines.next_line().await? {
                    let value: Value = serde_json::from_str(&line).context("Invalid Pi RPC JSON")?;
                    let mut state = state.lock().expect("Pi state lock poisoned");
                    if value["type"] == "response" {
                        if let Some(tx) = value["id"].as_str().and_then(|id| state.responses.remove(id)) {
                            let _ = tx.send(Response { value, turn: state.turn.as_ref().map(|turn| turn.id().to_owned()) });
                        }
                        continue;
                    }
                    if value["type"] == "extension_error" {
                        bail!(
                            "Pi extension failed: {}",
                            value["error"].as_str().unwrap_or("unknown extension error")
                        );
                    }
                    if value["type"] == "extension_ui_request" {
                        // A headless extension dialog requires a person; never silently answer it.
                        if matches!(value["method"].as_str(), Some("select" | "confirm" | "input" | "editor")) {
                            bail!("Pi extension requested interactive input; open this Session in Pi to answer it");
                        }
                    }
                    if let Some(turn) = state.turn.as_mut() {
                        let decoded = turn.event(&value);
                        let mapped = state.record(decoded)?;
                        let complete = mapped.iter().any(|e| {
                            matches!(e, ConversationEvent::TurnCompleted { .. })
                        });
                        for event in mapped {
                            let _ = events.send(event);
                        }
                        if complete {
                            state.turn = None;
                        }
                    }
                }
                bail!("Pi RPC stream closed")
            }.await;
            let mut state = state.lock().expect("Pi state lock poisoned");
            state.dead = true;
            state.responses.clear();
            if let Some(mut turn) = state.turn.take() {
                let decoded = turn.finish(Lifecycle::Failed);
                let mapped = display(&decoded);
                if let Err(error) = state.record(decoded) {
                    tracing::warn!(%error, "could not retain Pi failure");
                }
                for event in mapped {
                    let _ = events.send(event);
                }
            }
            let _ = events.send(ConversationEvent::Error {
                code: "pi_disconnected".into(),
                message: outcome
                    .expect_err("Pi reader returns only after an error or EOF")
                    .to_string(),
                evidence: None,
            });
        })
    }
}

#[async_trait]
impl Harness for PiHarness {
    fn set_agent_session(&mut self, session: Option<AgentSessionId>) {
        self.session = session;
    }
    fn agent_session(&self) -> Option<AgentSessionId> {
        self.session.clone()
    }
    fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }
    fn process_group_id(&self) -> Option<u32> {
        self.pid()
    }

    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        self.attachment = config.session_attachment.clone();
        self.seed = Some(match &config.skill_invocation {
            Some(skill) => skill.terminal_input("pi", config)?.1,
            None => config.task_prompt.clone(),
        });
        if let Err(error) = self.start_process(config).await {
            let _ = self.stop().await;
            return Err(error);
        }
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut state = self.state.lock().expect("Pi state lock poisoned");
            anyhow::ensure!(
                !state.dead,
                "Pi process exited; resume the Session to continue"
            );
            if state.turn.is_some() {
                return Err(HarnessError::TurnAlreadyInProgress.into());
            }
            let thread = self
                .session
                .clone()
                .ok_or_else(|| anyhow!("Pi has not started"))?;
            let turn = ProviderTurn {
                thread,
                turn: id.clone(),
            };
            state.record(vec![ProviderFact::TurnAnswered {
                turn: turn.clone(),
                request: id.clone(),
            }
            .into()])?;
            state.turn = Some(Turn::new(turn));
        }
        let message = match self.seed.take().filter(|s| !s.is_empty()) {
            Some(seed) if content.is_empty() => seed,
            Some(seed) => format!("{seed}\n\n{content}"),
            None => content.into(),
        };
        let _ = self
            .events
            .send(ConversationEvent::TurnStarted { turn_id: id });
        let result = self
            .request(json!({"type":"prompt", "message":message}))
            .await
            .and_then(|data| {
                anyhow::ensure!(
                    matches!(data["disposition"].as_str(), Some("started" | "queued")),
                    "Pi did not start a model run (disposition: {}); RPC requires Pi 1.3 or later",
                    data["disposition"]
                );
                Ok(())
            });
        if let Err(error) = result {
            let mapped = {
                let mut state = self.state.lock().expect("Pi state lock poisoned");
                match state.turn.take() {
                    Some(mut turn) => state.record(turn.finish(Lifecycle::Failed))?,
                    None => Vec::new(),
                }
            };
            for event in mapped {
                let _ = self.events.send(event);
            }
            return Err(error);
        }
        Ok(())
    }

    async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
        if content.trim().is_empty() {
            return SendCurrentOutcome::Failed {
                error: "steer input is empty".into(),
            };
        }
        let turn = self
            .state
            .lock()
            .expect("Pi state lock poisoned")
            .turn
            .as_ref()
            .map(|turn| turn.id().to_owned());
        let Some(turn) = turn else {
            return SendCurrentOutcome::NotSteerable;
        };
        if let Err(error) = self.owner() {
            return SendCurrentOutcome::Failed {
                error: error.to_string(),
            };
        }
        // Pi owns the queue. An acknowledgment proves acceptance, not incorporation.
        // Missing responses and a settlement racing the acknowledgment are uncertain.
        match self
            .request_observed(json!({"type":"steer", "message":content}))
            .await
        {
            Ok(response) if response.value["success"] == false => SendCurrentOutcome::Failed {
                error: response.value["error"]
                    .as_str()
                    .unwrap_or("Pi rejected steering")
                    .into(),
            },
            Ok(response)
                if response.value["success"] == true
                    && response.value["data"]["disposition"] == "queued"
                    && response.turn.as_deref() == Some(&turn) =>
            {
                SendCurrentOutcome::Sent {
                    provider_turn_id: turn,
                }
            }
            Ok(response) => SendCurrentOutcome::Unknown {
                provider_turn_id: Some(turn),
                error: format!(
                    "Pi steer did not confirm queuing during the active turn (disposition: {})",
                    response.value["data"]["disposition"]
                ),
            },
            Err(error) => SendCurrentOutcome::Unknown {
                provider_turn_id: Some(turn),
                error: error.to_string(),
            },
        }
    }

    async fn interrupt(&mut self) -> Result<()> {
        self.request(json!({"type":"clear_queue"})).await?;
        self.request(json!({"type":"abort"})).await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if self.child.is_some() {
            // Use the shared attachment-fenced process-group teardown.
            super::agent_process::stop(&self.owner()?)?;
            self.stdin = None;
            // Shared teardown already reaps the group leader.
            self.child = None;
        }
        if let Some(reader) = self.reader.take() {
            reader.abort();
            let _ = reader.await;
        }
        if let Some(stderr) = self.stderr.take() {
            stderr.abort();
            let _ = stderr.await;
        }
        Ok(())
    }
}

fn display(decoded: &[Decoded]) -> Vec<ConversationEvent> {
    decoded
        .iter()
        .filter_map(|decoded| match decoded {
            Decoded::Display(event) => Some(event.clone()),
            Decoded::Fact(_) => None,
        })
        .collect()
}

/// The native extension is an owned provider callback, fenced like context delivery.
pub fn native_event() -> anyhow::Result<()> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "Pi native event exceeds 32 MiB"
    );
    let value: Value = serde_json::from_slice(&bytes)?;
    let adapter: crate::steer::SteerAdapter =
        serde_json::from_str(&std::env::var(crate::steer::PROVIDER_CONTEXT_ENV)?)?;
    let store = crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
    observe_native(&store, &adapter, &value)
}

fn observe_native(
    store: &crate::store::sqlite::SqliteStore,
    adapter: &crate::steer::SteerAdapter,
    value: &Value,
) -> anyhow::Result<()> {
    let file = value["sessionFile"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Pi did not report a session file"))?;
    anyhow::ensure!(
        std::path::Path::new(file).is_absolute(),
        "Pi session file must be absolute"
    );
    let thread = AgentSessionId::from(file);
    let startup = value["type"] == "session_start";
    let boundary = if startup {
        NativeBoundary::Activate {
            source: "pi_session_start",
        }
    } else {
        NativeBoundary::Turn
    };
    store.with_provider_context(adapter, file, boundary, || {
        let attachment = store.session_attachment(&adapter.session)?.ok_or_else(|| {
            crate::store::StoreError::InvalidAuthority("Pi has no attachment".into())
        })?;
        let mut writer = FactWriter::new(
            store.clone(),
            adapter.session.to_string(),
            Some(attachment),
            None,
            Default::default(),
        );
        writer.record(&[ProviderFact::SessionObserved(thread.clone()).into()])?;
        let session = store
            .session(&adapter.session)?
            .ok_or_else(|| crate::store::StoreError::InvalidData("Pi Session missing".into()))?;
        let input = session
            .input_key()
            .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?
            .clone();
        let source = format!("pi-native:{}", uuid::Uuid::new_v4());
        store.retain_session_observation(
            &session,
            &crate::session::SessionObservation {
                artifact_key: input.clone(),
                source: source.clone(),
                observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                task_id: session.task_id.clone(),
                wave_id: session.wave_id.clone(),
                payload: json!({"input_id":input,"source":source,"evidence":value}),
            },
        )?;
        if startup {
            crate::session_record::write_provider_session(
                &crate::session_record::capture_dir(&input)
                    .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?,
                &thread,
                None,
            )
            .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?;
            return Ok(());
        }
        let decoded = super::pi_decode::native(&thread, value)
            .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?;
        writer.record(&decoded)
    })?;
    Ok(())
}
