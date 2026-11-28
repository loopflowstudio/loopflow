//! OpenCode 2. A headless turn is one `opencode run --standalone --format json`
//! process; a conversation in a terminal is `opencode --standalone` itself.
//! lf names the conversation (`--session ses_…`), and the context plugin
//! supplies instructions, context, steers and turn reports in both.
use crate::harness::LaunchKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::common::{Run, Stderr};

use crate::agent::AgentConfig;
use crate::harness::fact_writer::FactWriter;
use crate::harness::Recording;
use crate::harness::{Driver as StreamDriver, HarnessError};
use crate::steer::{IdleEndpoints, IdleReach, Inbox};
use loopflow_harness::fact::{ProviderFact, ProviderTurn};
use loopflow_harness::opencode_decode::Turn;
use loopflow_harness::Harness;
use loopflow_model::chat::types::ConversationEvent;
use loopflow_model::id::AgentSessionId;

#[derive(Debug, Default)]
struct State {
    /// Input lf sent whose turn OpenCode has not named yet.
    sent: Option<AgentSessionId>,
    turn: Option<Turn>,
    /// The last turn this driver named.
    named: Option<String>,
    /// Writes what the current process's turn establishes.
    writer: Option<FactWriter>,
}

impl State {
    /// Start the turn the sent input began. OpenCode names a turn by the user
    /// message it stored for the input, which is how its transcript is read;
    /// a run whose message cannot be found gets a name of lf's own.
    fn begin(&mut self, events: &mpsc::UnboundedSender<ConversationEvent>) -> Option<&mut Turn> {
        if let Some(thread) = self.sent.take() {
            let home = loopflow_harness::OpenCode.transcript_home();
            let id = loopflow_harness::transcript::opencode::last_request(
                &loopflow_harness::transcript::opencode::store(&home),
                thread.as_str(),
            )
            .filter(|id| self.named.as_ref() != Some(id))
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            self.named = Some(id.clone());
            let turn = ProviderTurn {
                thread: thread.clone(),
                turn: id.clone(),
            };
            self.turn = Some(Turn::new(turn.clone()));
            super::common::publish(
                self.writer.as_mut(),
                vec![
                    ProviderFact::SessionObserved(thread).into(),
                    ProviderFact::TurnAnswered {
                        turn,
                        request: id.clone(),
                    }
                    .into(),
                    ConversationEvent::TurnStarted { turn_id: id }.into(),
                ],
                events,
            );
        }
        self.turn.as_mut()
    }
}

#[derive(Debug)]
pub struct Driver {
    events: mpsc::UnboundedSender<ConversationEvent>,
    state: Arc<Mutex<State>>,
    config: AgentConfig,
    owner: Recording,
    runtime: Option<crate::process::LfProcessRuntime>,
    kind: LaunchKind,
    thread: Option<AgentSessionId>,
    first_turn: bool,
    run: Run,
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
            runtime: None,
            kind: launch.kind,
            thread: launch.start.thread(),
            first_turn: true,
            run: Run::default(),
        }
    }

    /// The run for one turn, and where its stderr goes.
    fn command(&self, config: &AgentConfig, thread: &AgentSessionId) -> Result<(Command, Stderr)> {
        let mut command = Command::new("opencode");
        command.args(["run", "--standalone", "--format", "json", "--auto"]);
        command.args(["--session", thread.as_str()]);
        super::configure_agent_env(&mut command, config);
        command.envs(environment(config, None, self.kind, thread)?);
        super::configure_vendor_std_env(command.as_std_mut())?;
        let stderr = Stderr::pipe(&mut command, config, "opencode")?;
        Ok((command, stderr))
    }

    fn spawn_reader(
        &self,
        mut child: Child,
        stdout: ChildStdout,
        stderr: Stderr,
        stopped: Arc<AtomicBool>,
    ) -> JoinHandle<()> {
        let state = self.state.clone();
        let events = self.events.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    tracing::debug!(line = %line, "OpenCode printed a line that is not JSON");
                    continue;
                };
                let mut state = state.lock().expect("OpenCode state lock poisoned");
                if let Some(turn) = state.begin(&events) {
                    let decoded = turn.event(&value);
                    if turn.asked() {
                        // Nobody can answer: end the run before it assumes one.
                        let _ = child.start_kill();
                    }
                    super::common::publish(state.writer.as_mut(), decoded, &events);
                }
            }
            let exited = child.wait().await.is_ok_and(|status| status.success());
            let mut state = state.lock().expect("OpenCode state lock poisoned");
            state.begin(&events);
            let Some(mut turn) = state.turn.take() else {
                return;
            };
            // Stopped by this driver, or with the lf process a signal is ending.
            let interrupted = stopped.load(Ordering::SeqCst) || crate::agent::interrupted();
            let mut decoded = Vec::new();
            if !exited && !interrupted {
                let detail = stderr.read();
                if let Some(line) = detail.lines().rev().find(|line| !line.trim().is_empty()) {
                    decoded.extend(turn.fail(line));
                }
            }
            decoded.extend(turn.finish(exited, interrupted));
            super::common::publish(state.writer.as_mut(), decoded, &events);
        })
    }
}

#[async_trait]
impl StreamDriver for Driver {
    fn set_runtime(&mut self, runtime: Option<crate::process::LfProcessRuntime>) {
        self.runtime = runtime;
    }

    fn pid(&self) -> Option<u32> {
        self.run.pid
    }

    fn process_group_id(&self) -> Option<u32> {
        self.run.group()
    }

    async fn start(&mut self) -> Result<()> {
        if self.thread.is_none() {
            self.thread = Some(super::Launcher::thread_name(
                &loopflow_harness::OpenCode,
                self.owner.assigned_thread()?,
            ));
        }
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        let Some(message) = super::common::turn_content(content, &self.config, self.first_turn)
        else {
            return Ok(());
        };
        {
            let state = self.state.lock().expect("OpenCode state lock poisoned");
            if state.sent.is_some() || state.turn.is_some() {
                return Err(HarnessError::TurnAlreadyInProgress.into());
            }
        }
        let thread = self
            .thread
            .clone()
            .ok_or_else(|| anyhow!("OpenCode has not started"))?;
        self.run.settle(&self.owner).await?;
        super::common::next_agent_process(
            &mut self.owner,
            self.runtime.as_ref(),
            &mut self.config,
        )?;
        let owner = self.owner.clone();
        let mut config = self.config.clone();
        if let Some(context) = &mut config.conversation_context {
            context.identify(&owner.store, &owner.session)?;
        }
        let (command, stderr) = self.command(&config, &thread)?;
        let mut child = super::agent_process::spawn(command, None, &owner, self.kind)?;
        let stopped = self.run.began(child.id());
        let stdin = child.stdin.take().expect("OpenCode stdin was piped");
        let stdout = child.stdout.take().expect("OpenCode stdout was piped");

        {
            let mut state = self.state.lock().expect("OpenCode state lock poisoned");
            state.writer = Some(super::common::writer(&owner));
            state.sent = Some(thread);
        }
        self.run.reader = Some(self.spawn_reader(child, stdout, stderr, stopped));

        // `opencode run` reads its message from stdin and starts at end of input.
        if let Err(error) = super::common::send(owner, stdin, message.into_bytes()).await {
            self.run.settle(&self.owner).await?;
            return Err(error);
        }
        self.first_turn = false;
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        // A run has no in-band cancel: stopping its process ends the turn, and
        // the next input continues the conversation by its `--session`.
        {
            let state = self.state.lock().expect("OpenCode state lock poisoned");
            if state.sent.is_none() && state.turn.is_none() {
                return Ok(());
            }
        }
        self.run.stop(&self.owner).await
    }

    async fn stop(&mut self) -> Result<()> {
        self.run.stop(&self.owner).await
    }

    fn agent_session(&self) -> Option<AgentSessionId> {
        self.thread.clone()
    }
}

fn environment(
    config: &AgentConfig,
    context_file: Option<&std::path::Path>,
    kind: LaunchKind,
    thread: &AgentSessionId,
) -> Result<std::collections::BTreeMap<String, String>> {
    let mut environment = config.env.clone();
    // An inherited value is the calling OpenCode conversation's, with its plugin.
    let mut settings: serde_json::Value = environment
        .get("OPENCODE_CONFIG_CONTENT")
        .map(|value| serde_json::from_str(value))
        .transpose()?
        .unwrap_or_else(|| serde_json::json!({}));
    if !settings.is_object() {
        bail!("OPENCODE_CONFIG_CONTENT must be a JSON object");
    }
    // The TUI has no `--model`; configuration sets it for workers and work sessions alike.
    if let Some(model) = &config.agent.model {
        settings["model"] = serde_json::json!(model);
    }
    if config.skips_permissions(kind.mode()) {
        settings["permissions"] =
            serde_json::json!([{ "action":"*", "resource":"*", "effect":"allow" }]);
    }
    if let Some(context) = &config.conversation_context {
        crate::harness::context::add_opencode_context(
            &mut settings,
            context,
            &crate::agent::system_prompt_with_structured_replies(config),
            kind,
            thread.as_str(),
        )?;
    } else {
        let instructions = match context_file {
            Some(path) => Some(path.to_owned()),
            None => crate::agent::write_system_prompt_file(config, "opencode")?,
        };
        if let Some(path) = instructions {
            settings
                .as_object_mut()
                .expect("configuration is an object")
                .entry("instructions")
                .or_insert_with(|| serde_json::json!([]))
                .as_array_mut()
                .ok_or_else(|| anyhow::anyhow!("OpenCode instructions must be an array"))?
                .push(serde_json::json!(path));
        }
    }
    environment.insert("OPENCODE_CONFIG_CONTENT".into(), settings.to_string());
    Ok(environment)
}

impl super::Launcher for loopflow_harness::OpenCode {
    fn locate_transcript(
        &self,
        _store: &loopflow_storage::sqlite::SqliteStore,
        id: &AgentSessionId,
        _cwd: &std::path::Path,
    ) -> Result<crate::transcript::Located> {
        let home = self.transcript_home();
        let store = loopflow_harness::transcript::opencode::store(&home);
        Ok(if store.is_file() {
            crate::transcript::Located::Found {
                source: loopflow_harness::transcript::Source::OpenCode {
                    store,
                    id: id.to_string(),
                },
                home: Some(home),
            }
        } else {
            crate::transcript::Located::Gone(crate::transcript::Gap::FileMissing { home })
        })
    }
    fn recent_transcripts(
        &self,
        home: &std::path::Path,
        cwd: &std::path::Path,
        since: i64,
        _owned: &std::collections::HashSet<&str>,
    ) -> Vec<(String, loopflow_harness::transcript::Source, i64)> {
        let store = loopflow_harness::transcript::opencode::store(home);
        loopflow_harness::transcript::opencode::written_in(&store, cwd, since)
            .into_iter()
            .map(|(id, at)| {
                (
                    id.clone(),
                    loopflow_harness::transcript::Source::OpenCode {
                        store: store.clone(),
                        id,
                    },
                    at,
                )
            })
            .collect()
    }
    fn classify_failure(&self, code: &str, message: &str) -> Option<crate::agent::AgentFailure> {
        // "Insufficient balance. This request is pay-as-you-go, billed to
        // OpenCode Models credits." and, for a model it has no route to,
        // "Model unavailable: opencode/<name>" (2.0.32). OpenCode retries a
        // transient failure itself.
        let words = message.to_ascii_lowercase();
        if words.contains("insufficient balance") {
            Some(crate::agent::AgentFailure::UsageLimit)
        } else if words.contains("model unavailable") {
            Some(crate::agent::AgentFailure::ModelUnavailable)
        } else {
            crate::agent::routing_failure(code, message)
        }
    }
    fn idle_reach(&self, endpoints: &IdleEndpoints) -> Option<IdleReach> {
        endpoints
            .socket
            .clone()
            .map(|path| IdleReach::Inbox(Inbox::Socket(path)))
    }

    fn capabilities(&self) -> super::Capabilities {
        super::Capabilities {
            thread: super::ThreadNaming {
                bare: super::ThreadName::Assigned,
                managed: super::ThreadName::Assigned,
            },
            initial_prompt_on_resume: false,
            reopen_steers: super::ReopenSteers::ServerBoundary,
            native_skills: None,
            installed_skills: super::InstalledSkills {
                global: ".agents/skills",
                repository: ".agents/skills",
                global_registry: None,
            },
            turn_limit: false,
            output_schema: false,
            respawns_after_crash: false,
            install_hint: concat!(
                "Install OpenCode 2 or later with `npm install -g @opencode/cli`; ",
                "the `opencode-ai` package stops at 1.x.",
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
    fn thread_name(&self, assigned: AgentSessionId) -> AgentSessionId {
        // OpenCode creates the conversation a `--session` names, and accepts
        // only names that start with `ses`.
        format!("ses_{}", assigned.as_str().replace('-', "")).into()
    }
    fn open(&self, launch: &super::Launch<'_>) -> Result<super::native_session::NativeClient> {
        let (_, input) = launch.terminal_input()?;
        let mut client = super::native_session::NativeClient::new(launch)?;
        let thread = client
            .thread
            .clone()
            .ok_or_else(|| anyhow!("OpenCode conversation has no name"))?;
        client
            .process
            .envs(environment(
                launch.config,
                launch.context_file,
                launch.kind,
                &thread,
            )?)
            .args(["--standalone", "--session", thread.as_str()]);
        if let Some(prompt) = input.filter(|prompt| !prompt.is_empty()) {
            crate::agent::validate_terminal_turn(&prompt)?;
            client.process.args(["--prompt", &prompt]);
        }
        client.stderr = super::common::diagnostics(launch.config, "opencode")?;
        Ok(client)
    }
}

#[cfg(test)]
mod tests {
    use crate::harness::LaunchKind;
    use crate::process::attachment;

    use super::super::common::drive_turn;
    use super::{mpsc, AgentConfig, Command, Driver, StreamDriver};
    use loopflow_model::chat::types::Lifecycle;

    #[tokio::test]
    async fn stop_clears_the_child_and_its_group() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = crate::store::opening::open_ephemeral(&database).unwrap();
        store.test_session("stop", &crate::session_record::new_artifact_key());
        let process = loopflow_model::id::LfProcessId::new();
        let conn = rusqlite::Connection::open(&database).unwrap();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&process],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET provider='opencode',interactive=0 WHERE id='stop'",
            [],
        )
        .unwrap();
        let first = attachment::claim_session_attachment(&store, "stop", None, &process).unwrap();
        let owner = crate::harness::Recording {
            store: store.clone(),
            session: "stop".to_string(),
            attachment: first.clone(),
        };
        let mut command = Command::new("/bin/sleep");
        command.env_clear().arg("60");
        let child =
            super::super::agent_process::spawn(command, None, &owner, LaunchKind::Batch).unwrap();
        let pid = child.id().unwrap();
        let birth = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::OpenCode,
                &crate::agent::AgentConfig::default(),
                &owner,
            ),
            tx,
        );
        harness.run.pid = Some(pid);
        assert_eq!(harness.process_group_id(), Some(pid));
        harness.stop().await.unwrap();
        assert_eq!(harness.pid(), None);
        assert_eq!(harness.process_group_id(), None);
        assert_eq!(
            crate::process::os_process::process_identity_evidence(pid, birth),
            crate::store::ProcessIdentityEvidence::Dead
        );
        drop(child);
    }

    #[test]
    fn build_turn_content_includes_task_prompt_on_first_turn() {
        let content = super::super::common::turn_content(
            "",
            &AgentConfig {
                task_prompt: "task".to_string(),
                ..crate::agent::AgentConfig::default()
            },
            true,
        );
        assert_eq!(content.as_deref(), Some("task"));
    }

    #[test]
    fn the_configured_model_reaches_opencode_through_its_configuration() {
        let config = AgentConfig {
            agent: crate::config::Agent::parse("opencode:opencode/gemini-4-carbon").unwrap(),
            ..AgentConfig::default()
        };
        let environment = super::environment(
            &config,
            Some(std::path::Path::new("/tmp/context.md")),
            LaunchKind::WorkSession,
            &loopflow_model::id::AgentSessionId::from("ses_model"),
        )
        .unwrap();
        let settings: serde_json::Value =
            serde_json::from_str(&environment["OPENCODE_CONFIG_CONTENT"]).unwrap();
        assert_eq!(settings["model"], "opencode/gemini-4-carbon");
    }

    /// A stand-in `opencode` that records its argv and stdin, then prints
    /// `lines` and leaves with `code`.
    fn stand_in(home: &std::path::Path, lines: &str, code: i32) -> crate::test_ambient::EnvGuard {
        use std::os::unix::fs::PermissionsExt;
        let bin = home.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let script = bin.join("opencode");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{home}/argv'\ncat > '{home}/stdin'\ncat <<'LINES'\n{lines}\nLINES\nexit {code}\n",
                home = home.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let guard = crate::test_ambient::EnvGuard::clear(&["PATH", "LF_BIN"]);
        std::env::set_var("PATH", format!("{}:/usr/bin:/bin", bin.display()));
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        guard
    }

    #[tokio::test]
    async fn each_turn_is_one_run_that_continues_the_named_conversation() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let _path = stand_in(
            ledger.home(),
            r#"{"type":"step_start","part":{}}
{"type":"text","part":{"text":"first answer"}}"#,
            0,
        );
        let config = AgentConfig {
            agent: crate::config::Agent::parse("opencode:opencode/fixture").unwrap(),
            task_prompt: "the task".into(),
            cwd: Some(ledger.home().to_owned()),
            ..AgentConfig::default()
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::OpenCode,
                &config,
                &recording,
            ),
            tx,
        );
        harness.start().await.unwrap();
        let thread = harness.agent_session().expect("lf names the conversation");
        assert!(thread.as_str().starts_with("ses_"), "{thread}");

        harness.send_input("and this").await.unwrap();
        let (status, text, errors) = drive_turn(&mut rx).await;
        assert_eq!(
            (status, text.as_str()),
            (Lifecycle::Completed, "first answer")
        );
        assert!(errors.is_empty(), "{errors:?}");
        let read = |name: &str| std::fs::read_to_string(ledger.home().join(name)).unwrap();
        assert_eq!(read("stdin"), "the task\n\nand this");
        assert_eq!(
            read("argv").lines().collect::<Vec<_>>(),
            [
                "run",
                "--standalone",
                "--format",
                "json",
                "--auto",
                "--session",
                thread.as_str()
            ]
        );

        harness.send_input("again").await.unwrap();
        let (status, ..) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert_eq!(read("stdin"), "again", "the task seeds only the first turn");
        assert!(read("argv").ends_with(&format!("--session\n{thread}\n")));
        harness.stop().await.unwrap();
    }

    #[tokio::test]
    async fn a_run_that_leaves_with_an_error_fails_the_turn_with_its_reason() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let _path = stand_in(
            ledger.home(),
            r#"{"type":"error","error":{"type":"unknown","message":"model is not available"}}"#,
            1,
        );
        let config = AgentConfig {
            agent: crate::config::Agent::parse("opencode").unwrap(),
            cwd: Some(ledger.home().to_owned()),
            ..AgentConfig::default()
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::OpenCode,
                &config,
                &recording,
            ),
            tx,
        );
        harness.start().await.unwrap();
        harness.send_input("go").await.unwrap();
        let (status, _, errors) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Failed);
        assert_eq!(errors, ["model is not available"]);
        harness.stop().await.unwrap();
    }

    /// Needs the `opencode` CLI on PATH with credentials for its default model:
    ///   cargo test -p loopflow-execution --lib opencode::tests::live_ -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "drives the real opencode run; needs the opencode CLI and credentials"]
    async fn live_basic_turn_completes() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let config = AgentConfig {
            agent: crate::config::Agent::parse("opencode").unwrap(),
            cwd: Some(std::env::temp_dir()),
            ..crate::agent::AgentConfig::default()
        };
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::OpenCode,
                &config,
                &recording,
            ),
            tx,
        );
        harness.start().await.expect("start");
        harness
            .send_input("Reply with exactly: ALPHA")
            .await
            .expect("seed turn");
        let (status, text, errors) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed, "{errors:?}");
        assert!(text.to_uppercase().contains("ALPHA"), "turn text: {text:?}");
        harness.stop().await.expect("stop");
    }
}
