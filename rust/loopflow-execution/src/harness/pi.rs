//! Pi 1.3 RPC transport. Pi owns models, credentials and its JSONL conversation.
use crate::harness::LaunchKind;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
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

use super::fact_writer::FactWriter;
use super::Recording;
use super::{Driver as StreamDriver, HarnessError};
use crate::agent::AgentConfig;
use crate::steer::NativeBoundary;
use loopflow_harness::fact::{Decoded, ProviderFact, ProviderTurn};
use loopflow_harness::pi_decode::{self, Turn};
use loopflow_model::chat::types::{ConversationEvent, Lifecycle};
use loopflow_model::id::AgentSessionId;

#[derive(Debug, Default)]
struct State {
    turn: Option<Turn>,
    /// Writes what this process's turns establish; present once started.
    writer: Option<FactWriter>,
    responses: HashMap<String, oneshot::Sender<Value>>,
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
pub struct Driver {
    events: mpsc::UnboundedSender<ConversationEvent>,
    state: Arc<Mutex<State>>,
    config: AgentConfig,
    owner: Recording,
    kind: LaunchKind,
    session: Option<AgentSessionId>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader: Option<JoinHandle<()>>,
    stderr: Option<JoinHandle<()>>,
    seed: Option<String>,
}

/// Pi accepts missing paths as new sessions. Validate retained files before
/// launch, and return the file to pass as `--session`.
pub(crate) fn validate_resume(token: &AgentSessionId) -> Result<PathBuf> {
    let path = conversation_file(token);
    anyhow::ensure!(
        path.is_absolute() && path.is_file(),
        "Pi conversation file is missing: {token}; restore it before resuming"
    );
    let file = std::fs::File::open(&path)?;
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
    Ok(path)
}

/// Pi arguments that reopen a recorded thread. A bare launch's id with no file
/// yet never got a reply, so Pi starts it fresh under that id, as it would
/// alone. A managed launch's recorded file must still exist.
pub(crate) fn resume_args(token: &AgentSessionId) -> Result<[String; 2]> {
    // An unfound bare id stays the relative token.
    if conversation_file(token).is_relative() {
        return Ok(["--session-id".into(), token.to_string()]);
    }
    Ok([
        "--session".into(),
        validate_resume(token)?.display().to_string(),
    ])
}

/// The file holding a recorded Pi thread. A managed launch records the file Pi
/// reports; a bare launch records the `--session-id` lf gave it, which Pi
/// keeps as `<agent dir>/sessions/<cwd>/<time>_<id>.jsonl`.
pub(crate) fn conversation_file(token: &AgentSessionId) -> PathBuf {
    let path = Path::new(token.as_str());
    if path.is_absolute() {
        return path.to_owned();
    }
    let agent_dir = std::env::var_os("PI_CODING_AGENT_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".pi").join("agent")));
    let suffix = format!("_{token}.jsonl");
    agent_dir
        .and_then(|dir| std::fs::read_dir(dir.join("sessions")).ok())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|project| std::fs::read_dir(project.path()).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|file| {
            file.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(&suffix))
        })
        .unwrap_or_else(|| path.to_owned())
}

pub fn options(config: &AgentConfig, context_file: Option<&Path>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = config.agent.model.clone() {
        args.extend(["--model".into(), model]);
    }
    if let Some(path) = context_file {
        args.extend(["--append-system-prompt".into(), path.display().to_string()]);
    }
    // Project trust is separate from tool permissions; retain the user's policy.
    args
}

impl Driver {
    pub fn new(
        launch: &super::Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
    ) -> Self {
        Self {
            events,
            state: Arc::default(),
            config: launch.config.clone(),
            owner: launch.recording.clone(),
            kind: launch.kind,
            session: launch.start.thread(),
            child: None,
            stdin: None,
            reader: None,
            stderr: None,
            seed: None,
        }
    }

    async fn write(&mut self, value: &Value) -> Result<()> {
        let Recording {
            store,
            session,
            attachment,
        } = self.owner.clone();
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

    async fn request(&mut self, mut value: Value) -> Result<Value> {
        let command = value["type"].as_str().unwrap_or("command").to_owned();
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
        let response = result?;
        anyhow::ensure!(
            response["success"] == true,
            "Pi {}: {}",
            command,
            response["error"].as_str().unwrap_or("command failed")
        );
        Ok(response["data"].clone())
    }

    async fn start_process(&mut self, config: &AgentConfig) -> Result<()> {
        let owner = self.owner.clone();
        let resumed = self.session.as_ref().map(validate_resume).transpose()?;
        let context = crate::agent::write_system_prompt_file(config, "pi")?;
        let mut command = Command::new("pi");
        command
            .args(["--mode", "rpc"])
            .args(options(config, context.as_deref()));
        if let Some(file) = &resumed {
            command.arg("--session").arg(file);
        }
        command.args(super::context::pi_args(
            config.conversation_context.as_ref(),
            false,
            self.kind,
        )?);
        super::configure_agent_env(&mut command, config);
        if let Some(context) = &config.conversation_context {
            context.configure_hooks(command.as_std_mut())?;
        }
        super::configure_vendor_std_env(command.as_std_mut())?;
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = super::agent_process::spawn(command, None, &owner, self.kind)
            .with_context(|| {
                crate::agent::missing_agent_message(loopflow_harness::HarnessKind::Pi)
            })?;
        let Recording {
            store,
            session,
            attachment,
        } = owner;
        self.state.lock().expect("Pi state lock poisoned").writer = Some(FactWriter::new(
            store,
            session,
            Some(attachment),
            Default::default(),
        ));
        self.stdin = child.stdin.take();
        let stdout = child.stdout.take().expect("Pi stdout was piped");
        self.stderr = child
            .stderr
            .take()
            .map(|stderr| super::common::spawn_stderr_logger(stderr, "pi", None));
        self.child = Some(child);
        self.reader = Some(self.spawn_reader(stdout));
        let info = self.request(json!({"type":"get_state"})).await?;
        let file = info["sessionFile"]
            .as_str()
            .filter(|f| Path::new(f).is_absolute())
            .ok_or_else(|| anyhow!("Pi did not report a persistent conversation file"))?;
        if let Some(expected) = &resumed {
            anyhow::ensure!(
                Path::new(file) == expected,
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
            let mut code = "pi_disconnected";
            let outcome: Result<()> = async {
                while let Some(line) = lines.next_line().await? {
                    let value: Value =
                        serde_json::from_str(&line).context("Invalid Pi RPC JSON")?;
                    let mut state = state.lock().expect("Pi state lock poisoned");
                    if value["type"] == "response" {
                        if let Some(tx) = value["id"]
                            .as_str()
                            .and_then(|id| state.responses.remove(id))
                        {
                            let _ = tx.send(value);
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
                        if matches!(
                            value["method"].as_str(),
                            Some("select" | "confirm" | "input" | "editor")
                        ) {
                            let question = [&value["title"], &value["message"]]
                                .iter()
                                .filter_map(|text| text.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            code = loopflow_model::chat::types::UNANSWERED_QUESTION;
                            bail!(loopflow_harness::fact::unanswered_question("Pi", &question));
                        }
                    }
                    if let Some(turn) = state.turn.as_mut() {
                        let decoded = turn.event(&value);
                        let mapped = state.record(decoded)?;
                        let complete = mapped
                            .iter()
                            .any(|e| matches!(e, ConversationEvent::TurnCompleted { .. }));
                        for event in mapped {
                            let _ = events.send(event);
                        }
                        if complete {
                            state.turn = None;
                        }
                    }
                }
                bail!("Pi RPC stream closed")
            }
            .await;
            let mut state = state.lock().expect("Pi state lock poisoned");
            state.dead = true;
            state.responses.clear();
            // A signal ending lf takes Pi with it: that turn was interrupted.
            let interrupted = crate::agent::interrupted();
            let message = outcome
                .expect_err("Pi reader returns only after an error or EOF")
                .to_string();
            if let Some(mut turn) = state.turn.take() {
                let decoded = if interrupted {
                    turn.finish(Lifecycle::Interrupted, None)
                } else {
                    let error = loopflow_model::chat::types::turn_error(code, &message);
                    turn.finish(Lifecycle::Failed, Some(error))
                };
                let mapped = display(&decoded);
                if let Err(error) = state.record(decoded) {
                    tracing::warn!(%error, "could not retain Pi failure");
                }
                for event in mapped {
                    let _ = events.send(event);
                }
            }
            if interrupted {
                return;
            }
            let _ = events.send(ConversationEvent::Error {
                code: code.into(),
                message,
                evidence: None,
            });
        })
    }
}

#[async_trait]
impl StreamDriver for Driver {
    fn agent_session(&self) -> Option<AgentSessionId> {
        self.session.clone()
    }
    fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }
    fn process_group_id(&self) -> Option<u32> {
        self.pid()
    }

    async fn start(&mut self) -> Result<()> {
        let config = self.config.clone();
        let config = &config;
        self.seed = Some(match &config.skill_invocation {
            Some(skill) => skill.terminal_input(&loopflow_harness::Pi, config)?.1,
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
                    Some(mut turn) => state.record(turn.finish(Lifecycle::Failed, None))?,
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

    async fn interrupt(&mut self) -> Result<()> {
        self.request(json!({"type":"clear_queue"})).await?;
        self.request(json!({"type":"abort"})).await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if self.child.is_some() {
            // Use the shared attachment-fenced process-group teardown.
            super::agent_process::stop(&self.owner.clone())?;
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

/// Record one event the native extension reported over its `session.boundary`
/// connection: Pi's identity, activity and usage.
pub fn observe_native(
    store: &loopflow_storage::sqlite::SqliteStore,
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
        let decoded = pi_decode::native(&thread, value)
            .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?;
        writer.record(&decoded)
    })?;
    Ok(())
}

impl super::Launcher for loopflow_harness::Pi {
    fn recognizes(&self, process: &crate::process::OsProcess) -> bool {
        process.command == "node"
            && crate::process::OsProcess::arguments(process.pid)
                .ok()
                .flatten()
                .is_some_and(|args| runs_pi(&args))
    }
    fn locate_transcript(
        &self,
        _store: &loopflow_storage::sqlite::SqliteStore,
        id: &AgentSessionId,
        _cwd: &Path,
    ) -> Result<crate::transcript::Located> {
        let path = conversation_file(id);
        Ok(if path.is_file() {
            crate::transcript::Located::Found {
                source: loopflow_harness::transcript::Source::File(path),
                home: None,
            }
        } else {
            crate::transcript::Located::Gone(crate::transcript::Gap::FileMissing {
                home: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            })
        })
    }
    fn transcript_retention(
        &self,
        thread: &AgentSessionId,
    ) -> (crate::transcript::RetentionState, Option<i64>) {
        match loopflow_harness::transcript::modified(thread.as_str().as_ref()) {
            Some(at) => (crate::transcript::RetentionState::Provider, Some(at)),
            None => (crate::transcript::RetentionState::Facts, None),
        }
    }
    fn recent_transcripts(
        &self,
        home: &Path,
        cwd: &Path,
        since: i64,
        owned: &std::collections::HashSet<&str>,
    ) -> Vec<(String, loopflow_harness::transcript::Source, i64)> {
        let name = cwd
            .to_string_lossy()
            .trim_start_matches('/')
            .replace('/', "-");
        crate::transcript::raw::unowned(
            crate::transcript::raw::in_directory(
                &home.join("sessions").join(format!("--{name}--")),
                |path| Some(path.to_str()?.to_owned()),
            )
            .into_iter()
            .filter(|(_, path, at)| {
                *at >= since
                    && loopflow_harness::transcript::recorded_cwd(path).as_deref() == Some(cwd)
            })
            .map(|(id, path, at)| (id, loopflow_harness::transcript::Source::File(path), at))
            .collect(),
            owned,
        )
    }
    fn classify_failure(&self, code: &str, message: &str) -> Option<crate::agent::AgentFailure> {
        // Pi retries a transient failure itself.
        crate::agent::routing_failure(code, message)
    }
    fn capabilities(&self) -> super::Capabilities {
        super::Capabilities {
            thread: super::ThreadNaming {
                bare: super::ThreadName::Assigned,
                managed: super::ThreadName::Reported,
            },
            initial_prompt_on_resume: false,
            native_skills: None,
            turn_limit: false,
            output_schema: false,
            respawns_after_crash: false,
            install_hint: concat!(
                "Install Pi 1.3 or later with ",
                "`npm install -g --ignore-scripts @earendil-works/pi-coding-agent`. ",
                "Configure its model and credentials in Pi.",
            ),
        }
    }
    fn driver(
        &self,
        launch: &super::Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
        _approval: super::ApprovalPolicy,
    ) -> Box<dyn super::Driver> {
        Box::new(Driver::new(launch, events))
    }

    fn open(&self, launch: &super::Launch<'_>) -> Result<super::native_session::NativeClient> {
        let mut client = super::native_session::NativeClient::new(launch)?;
        let (mut args, input) = launch.terminal_input()?;
        args.extend(options(launch.config, launch.context_file));
        match launch.start {
            super::Start::Resume { thread } => args.extend(resume_args(thread)?),
            super::Start::New => {
                if let Some(thread) = &client.thread {
                    args.extend(["--session-id".into(), thread.to_string()]);
                }
            }
        }
        if let Some(input) = input.filter(|input| !input.is_empty()) {
            crate::agent::validate_terminal_turn(&input)?;
            args.extend(["--".into(), input]);
        }
        if let Some(context) = &launch.config.conversation_context {
            context.configure_hooks(&mut client.process)?;
        }
        client
            .process
            .args(super::context::pi_args(
                launch.config.conversation_context.as_ref(),
                true,
                launch.kind,
            )?)
            .args(args);
        Ok(client)
    }
}

fn runs_pi(arguments: &[String]) -> bool {
    if arguments.first().is_some_and(|title| title == "pi") {
        return true;
    }
    let Some(script) = arguments.get(1).map(Path::new) else {
        return false;
    };
    script.file_name().is_some_and(|name| name == "pi")
        || script.components().any(|part| {
            part.as_os_str()
                .to_string_lossy()
                .ends_with("pi-coding-agent")
        })
}

#[cfg(test)]
mod recognition_tests {
    use super::runs_pi;
    #[test]
    fn pi_is_its_title_or_the_node_script_its_package_installs() {
        let pi = |arguments: &[&str]| {
            runs_pi(&arguments.iter().map(|a| a.to_string()).collect::<Vec<_>>())
        };
        assert!(pi(&["pi"]), "Pi's own process title");
        assert!(pi(&["node", "/Users/me/.nvm/versions/node/v24/bin/pi"]));
        assert!(pi(&[
            "node",
            "/usr/lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js"
        ]));
        assert!(!pi(&["node", "/usr/lib/node_modules/vite/bin/vite.js"]));
        assert!(!pi(&["pi-rpc"]), "lf launches Pi under RPC");
    }
}
