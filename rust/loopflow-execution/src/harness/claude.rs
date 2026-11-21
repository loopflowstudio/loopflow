use crate::harness::LaunchKind;
use crate::process::OsProcess;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::AgentConfig;
use crate::harness::common::TurnInProgressGuard;
use crate::harness::fact_writer::{FactWriter, RequestOrigins};
use crate::harness::Recording;
use crate::harness::{Driver as StreamDriver, HarnessError};
use crate::provider_auth::Provider;
use loopflow_harness::claude_decode::ClaudeDecoder;
use loopflow_harness::fact::{Decoded, ProviderFact};
use loopflow_model::chat::types::{ConversationEvent, Lifecycle};
use loopflow_model::id::AgentSessionId;

/// One persistent `claude -p --input-format stream-json` process drives every
/// turn of a run. `send_input` writes the seed message; the process stays alive
/// (holding conversation context, warm prompt cache) for the next turn instead
/// of respawning. Steers reach a running turn through the between-tool hook.
pub struct Driver {
    events: mpsc::UnboundedSender<ConversationEvent>,
    config: AgentConfig,
    owner: Recording,
    kind: LaunchKind,
    runtime: Option<crate::process::LfProcessRuntime>,
    should_seed_task_prompt: bool,
    skill_input: Option<String>,
    /// Vendor session id captured from the first turn's `system` event; a
    /// respawn (after interrupt/crash) resumes it via `--resume`.
    agent_session: Arc<Mutex<Option<AgentSessionId>>>,
    turn_in_progress: Arc<AtomicBool>,
    /// The runner turn id every provider turn in the current coalesced boundary
    /// reports under. Set by `send_input`, read by the reader.
    current_turn_id: Arc<Mutex<Option<String>>>,
    requests: RequestOrigins,
    /// Decoder for the running process's stream; the sending side registers
    /// each request it writes so the echo can be correlated.
    decoder: Arc<Mutex<ClaudeDecoder>>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader_task: Option<JoinHandle<()>>,
    shutdown_requested: Arc<AtomicBool>,
    interrupt_requested: Arc<AtomicBool>,
}

impl std::fmt::Debug for Driver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Driver").finish()
    }
}

/// Serialize one Claude Code stream-json user message.
fn user_message_line(text: &str, id: &str) -> String {
    let message = serde_json::json!({
        "type": "user", "uuid": id,
        "message": {"role": "user", "content": [{"type": "text", "text": text}]},
    });
    format!("{message}\n")
}

impl Driver {
    pub fn new(
        launch: &super::Launch<'_>,
        events: mpsc::UnboundedSender<ConversationEvent>,
    ) -> Self {
        Self {
            events,
            config: launch.config.clone(),
            owner: launch.recording.clone(),
            kind: launch.kind,
            runtime: None,
            should_seed_task_prompt: true,
            skill_input: None,
            agent_session: Arc::new(Mutex::new(launch.start.thread())),
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            current_turn_id: Arc::new(Mutex::new(None)),
            requests: Arc::new(Mutex::new(HashMap::new())),
            decoder: Arc::new(Mutex::new(ClaudeDecoder::default())),
            child: None,
            stdin: None,
            reader_task: None,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            interrupt_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Remember a request's origin before writing it, so its echo can claim it.
    fn register_request(&self, request: &str, origin: crate::session::SessionTurnOrigin) {
        self.requests
            .lock()
            .expect("Claude request lock poisoned")
            .insert(request.to_owned(), origin);
        self.decoder
            .lock()
            .expect("claude decoder lock poisoned")
            .outbound(request);
    }

    fn turn_origin(&self) -> Result<crate::session::SessionTurnOrigin> {
        let Recording {
            store,
            session,
            attachment,
        } = self.owner.clone();
        Ok(store.session_turn_origin(&session, &attachment)?)
    }

    /// Spawn the persistent stream-json process and its reader, if not already
    /// running. Resumes the captured vendor session after an interrupt/crash.
    async fn ensure_process(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }
        let config = &mut self.config;
        if let Some(runtime) = &self.runtime {
            let Recording {
                store: _,
                session,
                attachment,
            } = &mut self.owner;
            *attachment = runtime.prepare_agent_process(session, attachment)?;
            config.env.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(
                    &attachment.caller(session.clone().into(), crate::journal::current_trace_id()),
                )?,
            );
        }
        let resume_id = self
            .agent_session
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone();
        let context_file = crate::agent::write_system_prompt_file(config, "session")?;
        let mut args = stream_args(config, resume_id.as_ref(), context_file.as_deref())?;
        if let Some(invocation) = &config.skill_invocation {
            let (flags, input) = invocation.claude_input(config)?;
            args.extend(flags);
            self.skill_input = Some(input);
        }
        let mut cmd = Command::new("claude");
        cmd.args(&args);
        if let Some(context) = &config.conversation_context {
            cmd.args(super::context::claude_args(context, self.kind)?);
        }
        super::configure_agent_env(&mut cmd, config);
        if let Some(context) = &config.conversation_context {
            context.configure_hooks(cmd.as_std_mut())?;
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        super::configure_vendor_std_env(cmd.as_std_mut())?;
        self.shutdown_requested.store(false, Ordering::SeqCst);

        let owner = self.owner.clone();
        let mut child = super::agent_process::spawn(cmd, None, &owner, self.kind)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to capture claude stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture claude stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("failed to capture claude stderr"))?;

        let Recording {
            store,
            session,
            attachment,
        } = owner;
        self.decoder = Arc::new(Mutex::new(ClaudeDecoder::default()));
        self.spawn_reader(
            stdout,
            stderr,
            FactWriter::new(store, session, Some(attachment), self.requests.clone()),
        );
        self.stdin = Some(stdin);
        self.child = Some(child);
        Ok(())
    }

    /// The persistent NDJSON reader: maps events for the whole process lifetime
    /// and emits one `TurnCompleted` per coalesced runner turn.
    fn spawn_reader(
        &mut self,
        stdout: tokio::process::ChildStdout,
        stderr: tokio::process::ChildStderr,
        mut writer: FactWriter,
    ) {
        let events = self.events.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let current_turn_id = self.current_turn_id.clone();
        let shutdown = self.shutdown_requested.clone();
        let interrupted = self.interrupt_requested.clone();
        let session_slot = self.agent_session.clone();
        let decoder = self.decoder.clone();
        self.reader_task = Some(tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut stderr = BufReader::new(stderr).lines();
            let mut stderr_open = true;
            let mut diagnostics = VecDeque::new();
            let turn_id = || {
                current_turn_id
                    .lock()
                    .expect("claude turn id lock poisoned")
                    .clone()
                    .unwrap_or_default()
            };

            loop {
                let line = tokio::select! {
                    line = lines.next_line() => line,
                    line = stderr.next_line(), if stderr_open => {
                        match line {
                            Ok(Some(line)) => remember_stderr(&mut diagnostics, line),
                            _ => stderr_open = false,
                        }
                        continue;
                    }
                };
                let Ok(Some(line)) = line else {
                    break;
                };
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if line.trim().is_empty() {
                    continue;
                }

                let decoded = decoder
                    .lock()
                    .expect("claude decoder lock poisoned")
                    .decode(&line, &turn_id());
                let written = match decoded {
                    Ok(decoded) => writer.write(&decoded).map(|()| decoded),
                    Err(error) => Err(error),
                };
                let decoded = match written {
                    Ok(decoded) => decoded,
                    Err(error) => {
                        let _ = events.send(ConversationEvent::Error {
                            code: "claude_history".into(),
                            message: error.to_string(),
                            evidence: None,
                        });
                        break;
                    }
                };
                for item in decoded {
                    match item {
                        Decoded::Display(event) => {
                            let _ = events.send(event);
                        }
                        Decoded::Fact(ProviderFact::SessionObserved(session_id)) => {
                            *session_slot
                                .lock()
                                .expect("claude provider session id lock poisoned") =
                                Some(session_id);
                        }
                        Decoded::Fact(ProviderFact::TurnRejected { reason })
                            if turn_in_progress.load(Ordering::SeqCst) =>
                        {
                            let _ = events.send(ConversationEvent::Error {
                                code: "provider_rate_limited".to_string(),
                                message: reason,
                                evidence: None,
                            });
                            // The result still carries the provider's refusal
                            // and reset instructions. Let it finish the turn.
                        }
                        Decoded::Fact(ProviderFact::TurnCompleted { status, .. }) => {
                            diagnostics.clear();
                            let _ = events.send(ConversationEvent::TurnCompleted {
                                turn_id: turn_id(),
                                status,
                            });
                            turn_in_progress.store(false, Ordering::SeqCst);
                        }
                        Decoded::Fact(_) => {}
                    }
                }
            }

            // The process ended. If a turn was open (interrupt or crash), close
            // it so the runner is never left waiting.
            if turn_in_progress.load(Ordering::SeqCst) && !shutdown.load(Ordering::Relaxed) {
                // Read the failed client's final stderr. A descendant retaining
                // the pipe must not hold the exit indefinitely.
                let _ = tokio::time::timeout(Duration::from_secs(1), async {
                    while let Ok(Some(line)) = stderr.next_line().await {
                        remember_stderr(&mut diagnostics, line);
                    }
                })
                .await;

                // A signal ending lf takes Claude with it, and nothing reads
                // this reader's events any more: it records the end itself.
                let signalled = crate::agent::interrupted();
                if signalled {
                    let ended = decoder
                        .lock()
                        .expect("claude decoder lock poisoned")
                        .interrupt_pending();
                    if let Err(error) = writer.write(&ended) {
                        tracing::warn!(%error, "could not record Claude's interrupted turn");
                    }
                }
                let status = if signalled || interrupted.load(Ordering::SeqCst) {
                    Lifecycle::Interrupted
                } else {
                    tracing::warn!("claude stream ended without a final result");
                    let _ = events.send(ConversationEvent::Error {
                        code: "claude_exit".into(),
                        message: format!(
                            "Claude exited without a final result{}",
                            crate::os_process::exit_reason(
                                &diagnostics.iter().cloned().collect::<Vec<_>>().join("\n")
                            )
                        ),
                        evidence: None,
                    });
                    Lifecycle::Failed
                };
                let open = decoder
                    .lock()
                    .expect("claude decoder lock poisoned")
                    .drain_open_items(status);
                for item in open {
                    let _ = events.send(ConversationEvent::ItemCompleted {
                        turn_id: turn_id(),
                        item,
                    });
                }
                let _ = events.send(ConversationEvent::TurnCompleted {
                    turn_id: turn_id(),
                    status,
                });
            }
            turn_in_progress.store(false, Ordering::SeqCst);
        }));
    }

    /// Tear the persistent process down and reap its tasks. The next
    /// `send_input` respawns and resumes the captured session.
    async fn kill_process(&mut self) -> Result<()> {
        if self.child.is_some() {
            super::agent_process::stop(&self.owner.clone())?;
            // Common close proves group death and reaps our leader. Dropping
            // the Tokio handle also permits its reaper to collect a late zombie.
            self.child = None;
        }
        self.stdin = None;
        if let Some(task) = self.reader_task.take() {
            let mut task = task;
            if tokio::time::timeout(Duration::from_secs(2), &mut task)
                .await
                .is_err()
            {
                tracing::warn!("timed out waiting for claude reader task shutdown; aborting");
                task.abort();
                let _ = task.await;
            }
        }
        self.turn_in_progress.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send_line(&mut self, line: String) -> Result<()> {
        let mut stdin = self
            .stdin
            .take()
            .ok_or_else(|| anyhow!("claude stdin not available"))?;
        let Recording {
            store,
            session,
            attachment,
        } = self.owner.clone();
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
}

#[async_trait]
impl StreamDriver for Driver {
    fn set_runtime(&mut self, runtime: Option<crate::process::LfProcessRuntime>) {
        self.runtime = runtime;
    }
    fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }

    async fn start(&mut self) -> Result<()> {
        let config = self.config.clone();
        let config = &config;
        let requested_session = self
            .agent_session
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone();
        if let Some(thread) = requested_session.as_ref() {
            crate::transcript::adopt_native(Provider::Claude, thread)?;
        }

        // Validate claude binary on PATH.
        let mut version_command = Command::new("claude");
        version_command.arg("--version");
        super::configure_agent_env(&mut version_command, config);
        let output = version_command.output().await;
        match output {
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout);
                tracing::info!(version = %version.trim(), "claude binary found");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(anyhow!(
                    "claude --version failed (exit {}): {stderr}",
                    out.status
                ));
            }
            Err(err) => {
                return Err(anyhow!(
                    "claude binary not found on PATH: {err}. Install Claude Code first."
                ));
            }
        }

        self.should_seed_task_prompt = true;
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        if self
            .turn_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(HarnessError::TurnAlreadyInProgress.into());
        }
        let mut turn_guard = TurnInProgressGuard::new(self.turn_in_progress.clone());

        self.interrupt_requested.store(false, Ordering::SeqCst);
        self.ensure_process().await?;
        let skill_input = self
            .should_seed_task_prompt
            .then(|| self.skill_input.take())
            .flatten();

        // The first message of a run carries the task prompt as a preamble.
        let mut turn_content = content.to_string();
        if self.should_seed_task_prompt {
            let task_prompt = self.config.task_prompt.trim().to_string();
            self.should_seed_task_prompt = false;
            if !task_prompt.is_empty() {
                turn_content = format!("{task_prompt}\n\n{content}");
            }
        }

        let origin = self.turn_origin()?;
        let turn_id = uuid::Uuid::new_v4().to_string();
        self.register_request(&turn_id, origin);
        *self
            .current_turn_id
            .lock()
            .expect("claude turn id lock poisoned") = Some(turn_id.clone());
        let _ = self.events.send(ConversationEvent::TurnStarted {
            turn_id: turn_id.clone(),
        });

        // Send non-query skill context and the correlated query through one
        // write/error path, so either write failure tears down the process.
        let mut seed = String::new();
        if let Some(input) = skill_input {
            if !turn_content.is_empty() {
                seed = format!(
                    "{}\n",
                    serde_json::json!({
                        "type": "user", "shouldQuery": false,
                        "message": {"role": "user", "content": turn_content}
                    })
                );
            }
            turn_content = input;
        }
        seed.push_str(&user_message_line(&turn_content, &turn_id));
        if let Err(error) = self.send_line(seed).await {
            // The process died between spawn and write; tear it down so the
            // next send_input respawns cleanly.
            drop(turn_guard);
            self.kill_process().await?;
            return Err(anyhow!("failed to write claude seed message: {error}"));
        }

        turn_guard.disarm();
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        // Claude's stream-json input has no in-band cancel, so interrupt tears
        // the process down: the reader finalizes the open turn as Interrupted,
        // and the next send_input respawns and `--resume`s the captured session
        // (which survives the kill). The steer that prompted the interrupt rides
        // that next seed.
        if !self.turn_in_progress.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.interrupt_requested.store(true, Ordering::SeqCst);
        self.kill_process().await
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        self.kill_process().await
    }

    fn agent_session(&self) -> Option<AgentSessionId> {
        self.agent_session
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone()
    }
}

fn remember_stderr(lines: &mut VecDeque<String>, line: String) {
    tracing::debug!(stderr_target = "claude_harness", "{line}");
    if lines.len() == 32 {
        lines.pop_front();
    }
    lines.push_back(line);
}

#[cfg(test)]
mod activity_tests {
    use tokio::process::Command;
    use tokio::sync::mpsc;

    use super::Driver;
    use crate::harness::Driver as StreamDriver;
    use crate::session_record::activity;

    #[tokio::test]
    async fn claude_body_activity_does_not_require_process_group_authority() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &crate::agent::AgentConfig::default(),
                &crate::harness::admit_for_test(tempfile::tempdir().unwrap().path()),
            ),
            tx,
        );
        harness.child = Some(
            Command::new("sleep")
                .arg("300")
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
        );
        assert!(
            activity::sample(harness.pid()).await.is_some(),
            "a live Claude child must have observable CPU without a process group"
        );
        assert_eq!(harness.process_group_id(), None);
        // The injected child was never admitted; kill_on_drop ends it.
    }
}

#[cfg(test)]
mod tests {
    use crate::process::attachment;

    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Isolate provider and database selection.
    async fn claude_stop_ends_each_recorded_group() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN", "PATH"]);
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("loopflow.db");
        std::env::set_var("LF_HOME", home.path());
        let store = crate::store::opening::open_ephemeral(&database).unwrap();
        let conn = rusqlite::Connection::open(&database).unwrap();
        let process = loopflow_model::id::LfProcessId::new();
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&process],
        )
        .unwrap();
        let script = home.path().join("claude");
        std::fs::write(&script, "#!/bin/sh\nwhile read -r line; do printf '%s\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"ok\"}'; done\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Vendor setup rebuilds PATH from the process environment, not AgentConfig.
        let mut paths = vec![home.path().to_path_buf()];
        paths.extend(std::env::split_paths(&original_path));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let mut harnesses = Vec::new();
        let mut recorded = Vec::new();
        for id in ["first", "second"] {
            store.test_session(id, &crate::session_record::new_artifact_key());
            conn.execute(
                "UPDATE agent_sessions SET interactive=0, provider='claude' WHERE id=?1",
                [id],
            )
            .unwrap();
            let attachment =
                attachment::claim_session_attachment(&store, id, None, &process).unwrap();
            let (tx, _rx) = mpsc::unbounded_channel();

            let config = live_config();
            let (config, recording) = (
                config,
                Recording {
                    store: store.clone(),
                    session: id.into(),
                    attachment: attachment.clone(),
                },
            );
            let mut harness = Driver::new(
                &crate::harness::launch_for_test(
                    loopflow_harness::HarnessKind::Claude,
                    &config,
                    &recording,
                ),
                tx,
            );
            harness.send_input("one turn").await.unwrap();
            let (pid, birth) = store
                .agent_process_identity(id)
                .unwrap()
                .expect("managed Claude publishes exact AgentProcess identity");
            assert_eq!(harness.pid(), Some(pid));
            assert!(birth > 0);
            recorded.push((pid, birth));
            // Retain the first process while the same Process starts the next step.
            harnesses.push(harness);
        }
        for harness in &mut harnesses {
            harness.stop().await.unwrap();
        }
        assert_ne!(recorded[0].0, recorded[1].0);
        for (pid, _) in recorded {
            assert!(crate::process::OsProcess::group(pid).unwrap().is_empty());
        }
    }

    #[test]
    fn interrupted_capture_retains_policy_after_settings_change_and_fences_old_snapshots() {
        use crate::session_record::{SessionCaptureSpec, SessionGraphMembership};

        async fn observed_input(home: &std::path::Path, text: &str) -> String {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let inputs = std::fs::read_to_string(home.join("inputs")).unwrap_or_default();
                    if inputs.contains(text) {
                        break inputs;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap()
        }

        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let _env = crate::test_ambient::EnvGuard::clear(&["PATH", "LF_BIN"]);
        let home = ledger.home();
        std::fs::create_dir_all(home.join(".lf")).unwrap();
        std::fs::write(
            home.join(".lf/config.yaml"),
            "agent: claude:sonnet\nyolo: true\nchrome: true\n",
        )
        .unwrap();
        let settings = crate::config::load_config(Some(home)).unwrap().unwrap();
        // Without explicit YOLO, an already-bypassing vendor setting omits the
        // CLI override. This makes an accidental policy reload observable.
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::write(
            home.join(".claude/settings.json"),
            r#"{"permissions":{"defaultMode":"bypassPermissions"}}"#,
        )
        .unwrap();
        let script = home.join("claude");
        std::fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> launches\nwhile read -r line; do printf '%s\\n' \"$line\" >> inputs; done\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("PATH", format!("{}:/usr/bin:/bin", home.display()));
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        crate::journal::with_runtime(home, &["lf".into(), "skill".into()], || {
            let capture = crate::session_record::SessionRecorder::begin_at(
                home,
                SessionCaptureSpec {
                    harness: "claude".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: home.into(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![],
                    graph: SessionGraphMembership::Independent,
                    work: None,
                },
            )
            .map(crate::process::LfProcessRuntime::new)?;
            capture.claim_session_attachment()?;
            let (session, first) = capture.session_attachment().unwrap();
            let store = crate::store::open_sqlite(&home.join("loopflow.db"))?;
            runtime.block_on(async {
                let (tx, _rx) = mpsc::unbounded_channel();

                let (config, recording) = (
                    AgentConfig {
                        agent: crate::config::Agent::parse(settings.agent().unwrap()).unwrap(),
                        chrome: settings.chrome,
                        yolo: settings.yolo,
                        cwd: Some(home.into()),
                        ..crate::agent::AgentConfig::default()
                    },
                    Recording {
                        store: store.clone(),
                        session: session.clone(),
                        attachment: first.clone(),
                    },
                );
                let mut harness = Driver::new(
                    &crate::harness::launch_for_test(
                        loopflow_harness::HarnessKind::Claude,
                        &config,
                        &recording,
                    ),
                    tx,
                );
                harness.set_runtime(Some(capture.clone()));
                harness.send_input("first request").await.unwrap();
                // Pipe acceptance precedes provider execution. Interrupt only
                // after this throwaway provider consumed its first request.
                observed_input(home, "first request").await;
                let first_pid = harness.pid().unwrap();
                let other_home = tempfile::tempdir().unwrap();
                let other_store =
                    crate::store::opening::open_ephemeral(&other_home.path().join("loopflow.db"))
                        .unwrap();
                let selection = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
                std::env::set_var("LF_HOME", other_home.path());
                // The native conversation survives the OS process's interruption.
                *harness.agent_session.lock().unwrap() = Some("native-conversation".into());
                harness.interrupt().await.unwrap();
                assert!(other_store.agent_processes().unwrap().is_empty());
                drop(selection);
                let ended = store
                    .agent_process(&first.agent_process_id)
                    .unwrap()
                    .unwrap();
                assert_eq!(ended.pid, Some(first_pid));
                assert!(ended.completed_at.is_some());

                std::fs::write(
                    home.join(".lf/config.yaml"),
                    "agent: claude:haiku\nyolo: false\nchrome: false\n",
                )
                .unwrap();
                let current = crate::config::load_config(Some(home)).unwrap().unwrap();
                assert!(!current.yolo && current.chrome == Some(false));
                assert_eq!(current.agent.as_deref(), Some("claude:haiku"));
                harness.send_input("resumed request").await.unwrap();
                let (_, second) = capture.session_attachment().unwrap();
                assert_ne!(first.agent_process_id, second.agent_process_id);
                assert_ne!(first.token, second.token);
                let running = store
                    .agent_process(&second.agent_process_id)
                    .unwrap()
                    .unwrap();
                assert_eq!(running.pid, harness.pid());
                assert_eq!(running.parent_lf_process_id, first.lf_process_id);
                assert!(running.completed_at.is_none());
                assert_eq!(
                    store.agent_process(&first.agent_process_id).unwrap(),
                    Some(ended)
                );
                assert!(capture.prepare_agent_process(&session, &first).is_err());
                assert!(store
                    .record_session_connection(&session, &first, "stale", &"stale".into())
                    .is_err());

                // Pending operations retain the old snapshot, not the capture's
                // current value. Neither a pipe write nor a signal may use it.
                harness.owner.attachment = first;
                assert!(harness.send_line("stale request\n".into()).await.is_err());
                assert!(harness.stop().await.is_err());
                assert!(harness
                    .child
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .unwrap()
                    .is_none());
                harness.owner.attachment = second.clone();
                assert!(!observed_input(home, "resumed request")
                    .await
                    .contains("stale request"));
                let launches = std::fs::read_to_string(home.join("launches")).unwrap();
                assert_eq!(launches.lines().count(), 2);
                for launch in launches.lines() {
                    assert!(
                        launch.contains("--dangerously-skip-permissions"),
                        "{launch}"
                    );
                    assert!(launch.contains("--chrome"), "{launch}");
                    assert!(launch.contains("--model sonnet"), "{launch}");
                    assert!(!launch.contains("--no-chrome"), "{launch}");
                    assert!(!launch.contains("haiku"), "{launch}");
                }
                assert!(launches
                    .lines()
                    .nth(1)
                    .unwrap()
                    .contains("--resume native-conversation"));
                harness.stop().await.unwrap();
                capture.finish("completed").unwrap();
                let settled = store.session_attachment(&session).unwrap().unwrap();
                assert_eq!(settled.agent_process_id, second.agent_process_id);
                assert_eq!(settled.lf_process_id, None);
            });
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn send_input_spawn_failure_releases_turn_guard() {
        let (tx, _rx) = mpsc::unbounded_channel();

        let config = AgentConfig {
            agent: crate::config::Agent::parse("claude").unwrap(),
            task_prompt: "task".into(),
            cwd: Some(format!("/tmp/loopflow-missing-{}", uuid::Uuid::new_v4()).into()),
            ..crate::agent::AgentConfig::default()
        };
        let ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(ledger.home());
        let _env = crate::test_ambient::EnvGuard::clear(&["PATH", "LF_BIN"]);
        std::env::set_var("PATH", ledger.home());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let (config, recording) = (config, recording);
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &config,
                &recording,
            ),
            tx,
        );

        let first = harness
            .send_input("first")
            .await
            .expect_err("spawn should fail for missing cwd");
        assert!(
            first.to_string().contains("failed to spawn claude"),
            "{first}"
        );

        let second = harness
            .send_input("second")
            .await
            .expect_err("turn guard should be released after setup failure");
        assert!(
            !matches!(
                second.downcast_ref::<HarnessError>(),
                Some(HarnessError::TurnAlreadyInProgress)
            ),
            "second failure should not be turn-in-progress"
        );
    }

    #[tokio::test]
    async fn interrupt_without_turn_is_noop() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &crate::agent::AgentConfig::default(),
                &crate::harness::admit_for_test(tempfile::tempdir().unwrap().path()),
            ),
            tx,
        );
        harness.interrupt().await.expect("noop interrupt");
        assert!(!harness.interrupt_requested.load(Ordering::SeqCst));
    }

    /// A stand-in `claude` on an isolated PATH and home, with a store whose
    /// headless sessions it attaches to.
    struct StandInClaude {
        home: tempfile::TempDir,
        store: loopflow_storage::sqlite::SqliteStore,
        database: std::path::PathBuf,
        process: loopflow_model::id::LfProcessId,
        _environment: crate::test_ambient::EnvGuard,
        _ambient: crate::test_ambient::EnvGuard,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl StandInClaude {
        fn new(script: &str) -> Self {
            let _lock = crate::journal::test_env_lock();
            let _ambient = crate::test_ambient::EnvGuard::new();
            let _environment = crate::test_ambient::EnvGuard::clear(&[
                "LF_HOME",
                "LF_BIN",
                "PATH",
                "HOME",
                "CLAUDE_CONFIG_DIR",
            ]);
            let home = tempfile::tempdir().unwrap();
            for key in ["LF_HOME", "HOME", "CLAUDE_CONFIG_DIR"] {
                std::env::set_var(key, home.path());
            }
            std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
            // Admission samples the child's birth through ps before exec.
            std::env::set_var("PATH", format!("{}:/usr/bin:/bin", home.path().display()));
            let claude = home.path().join("claude");
            std::fs::write(&claude, script).unwrap();
            std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
            let database = home.path().join("loopflow.db");
            let store = crate::store::opening::open_ephemeral(&database).unwrap();
            let process = loopflow_model::id::LfProcessId::new();
            rusqlite::Connection::open(&database)
                .unwrap()
                .execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                    [&process],
                )
                .unwrap();
            Self {
                home,
                store,
                database,
                process,
                _environment,
                _ambient,
                _lock,
            }
        }

        /// Config for a fresh headless Session the stand-in attaches to.
        fn attach(&self, id: &str) -> (AgentConfig, Recording) {
            self.store
                .test_session(id, &crate::session_record::new_artifact_key());
            rusqlite::Connection::open(&self.database)
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET interactive=0, provider='claude' WHERE id=?1",
                    [id],
                )
                .unwrap();
            let attachment =
                attachment::claim_session_attachment(&self.store, id, None, &self.process).unwrap();
            let mut config = live_config();
            config.cwd = Some(self.home.path().to_path_buf());
            (
                config,
                Recording {
                    store: self.store.clone(),
                    session: id.into(),
                    attachment,
                },
            )
        }

        /// How many provider turns were recorded complete for a Session.
        fn completions(&self, id: &str) -> usize {
            self.store
                .session_turns(id)
                .unwrap()
                .into_iter()
                .filter(|turn| turn.ended_at.is_some())
                .count()
        }
    }

    #[tokio::test]
    async fn agent_session_resumes_on_a_fresh_process() {
        let claude = StandInClaude::new(
            r#"#!/bin/sh
resume=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = "--resume" ]; then shift; resume="$1"; fi
    shift
done
while read -r line; do
    if [ -z "$resume" ]; then
        printf '%s' remembered > memory
        answer=created
    elif [ "$resume" = "vendor-conversation" ] && [ -f memory ]; then
        answer=remembered
    else
        exit 1
    fi
    printf '%s\n' '{"type":"system","subtype":"init","session_id":"vendor-conversation"}'
    printf '{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"%s"}}\n' "$answer"
    printf '{"type":"result","subtype":"success","session_id":"vendor-conversation","result":"%s"}\n' "$answer"
done
"#,
        );
        // Each harness is its own admitted conversation; both resume one
        // provider-owned AgentSession.
        let (tx, mut rx) = mpsc::unbounded_channel();

        let (config, recording) = claude.attach("first");
        let mut first = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &config,
                &recording,
            ),
            tx,
        );
        first.send_input("remember").await.unwrap();
        let (status, answer, _) =
            tokio::time::timeout(Duration::from_secs(30), drive_turn(&mut rx))
                .await
                .unwrap();
        assert_eq!(status, Lifecycle::Completed);
        assert!(answer.contains("created"));
        let agent_session: AgentSessionId = first.agent_session().unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();

        let (config, recording) = claude.attach("second");
        let launch = crate::harness::Launch {
            start: crate::harness::Start::Resume {
                thread: &agent_session,
            },
            ..crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &config,
                &recording,
            )
        };
        let mut resumed = Driver::new(&launch, tx);
        resumed.send_input("recall").await.unwrap();
        let (status, answer, _) =
            tokio::time::timeout(Duration::from_secs(30), drive_turn(&mut rx))
                .await
                .unwrap();
        let different_process = first.pid() != resumed.pid();
        first.stop().await.unwrap();
        resumed.stop().await.unwrap();
        assert!(different_process);
        assert_eq!(status, Lifecycle::Completed);
        assert!(answer.contains("remembered"));
        assert_eq!(resumed.agent_session(), Some(agent_session));
    }

    #[tokio::test]
    async fn a_turn_with_background_tasks_completes_on_the_turn_their_notifications_start() {
        // Claude Code's shape, from a live stream-json probe: the turn ends while
        // the suite runs, then Claude starts its own turn once it finishes.
        let claude = StandInClaude::new(
            r#"#!/bin/sh
while read -r line; do
    uuid=$(printf '%s' "$line" | sed -n 's/.*"uuid":"\([^"]*\)".*/\1/p')
    printf '%s\n' '{"type":"system","subtype":"init","session_id":"vendor-conversation"}'
    printf '{"type":"user","uuid":"%s","session_id":"vendor-conversation","message":{"content":"seed"}}\n' "$uuid"
    printf '%s\n' '{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"suite"}]}'
    printf '%s\n' '{"type":"result","subtype":"success","session_id":"vendor-conversation","result":"WAITING"}'
    sleep 1
    touch survived
    printf '%s\n' '{"type":"system","subtype":"task_notification","task_id":"suite","status":"completed"}'
    printf '%s\n' '{"type":"system","subtype":"background_tasks_changed","tasks":[]}'
    printf '%s\n' '{"type":"system","subtype":"init","session_id":"vendor-conversation"}'
    printf '%s\n' '{"type":"result","subtype":"success","session_id":"vendor-conversation","result":"FINISHED"}'
done
"#,
        );
        let (tx, mut rx) = mpsc::unbounded_channel();

        let (config, admitted) = claude.attach("step");
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &config,
                &admitted,
            ),
            tx,
        );
        harness.send_input("run the suites").await.unwrap();
        let (status, _, _) = tokio::time::timeout(Duration::from_secs(30), drive_turn(&mut rx))
            .await
            .unwrap();
        let survived = claude.home.path().join("survived").exists();
        harness.stop().await.unwrap();

        assert_eq!(status, Lifecycle::Completed);
        assert!(survived, "the provider ran until its tasks finished");
        assert_eq!(claude.completions("step"), 1);
    }

    // Live persistent-process checks against the real `claude` CLI (subscription
    // auth). Ignored by default — run explicitly with a live login:
    //   cargo test -p loopflow --lib claude::tests::live_ -- --ignored --nocapture
    fn live_config() -> AgentConfig {
        AgentConfig {
            agent: crate::config::Agent::parse("claude").unwrap(),
            cwd: Some(std::env::temp_dir()),
            ..crate::agent::AgentConfig::default()
        }
    }

    async fn drive_turn(
        rx: &mut mpsc::UnboundedReceiver<ConversationEvent>,
    ) -> (Lifecycle, String, usize) {
        let mut text = String::new();
        let mut completions = 0usize;
        loop {
            match tokio::time::timeout(Duration::from_secs(180), rx.recv()).await {
                Ok(Some(ConversationEvent::TextDelta { content, .. })) => text.push_str(&content),
                Ok(Some(ConversationEvent::ItemCompleted {
                    item: loopflow_model::chat::types::ConversationItem::Message { text: t, .. },
                    ..
                })) => text.push_str(&t),
                Ok(Some(ConversationEvent::TurnCompleted { status, .. })) => {
                    completions += 1;
                    return (status, text, completions);
                }
                Ok(Some(_)) => {}
                Ok(None) => panic!("event channel closed before TurnCompleted"),
                Err(_) => panic!("timed out waiting for a turn"),
            }
        }
    }

    #[tokio::test]
    #[ignore = "drives the real claude CLI; needs a live subscription login"]
    async fn live_persistent_process_handles_sequential_turns() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let config = live_config();
        let _ledger = crate::journal::TestLedgerGuard::new();
        let recording = super::super::admit_for_test(_ledger.home());
        let mut harness = Driver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Claude,
                &config,
                &recording,
            ),
            tx,
        );
        harness.start().await.expect("start");

        harness
            .send_input("Reply with exactly: ALPHA")
            .await
            .expect("first turn");
        let (status, text, _) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert!(text.contains("ALPHA"), "first turn text: {text:?}");

        // Same persistent process, second turn — the session id must be stable.
        let session_after_first = harness.agent_session();
        harness
            .send_input("Reply with exactly: BETA")
            .await
            .expect("second turn");
        let (status, text, _) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert!(text.contains("BETA"), "second turn text: {text:?}");
        assert_eq!(
            session_after_first,
            harness.agent_session(),
            "the persistent process keeps one vendor session across turns"
        );

        harness.stop().await.expect("stop");
    }
}

/// The conversation each live Claude process has open, from the
/// `sessions/<pid>.json` Claude keeps in its home and rewrites when `/clear`
/// or `/resume` changes it. Claude versions before 2.1.318 may keep none.
fn conversation_owners(home: &Path, by_pid: &HashMap<u32, &OsProcess>) -> HashMap<u32, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Owner {
        pid: u32,
        session_id: String,
        /// Milliseconds.
        started_at: i64,
    }
    std::fs::read_dir(home.join("sessions"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()? != "json" {
                return None;
            }
            let owner: Owner = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
            let process = by_pid.get(&owner.pid)?;
            // A file left by an exited process names whoever reused its pid.
            ((owner.started_at / 1000 - process.started_at).abs()
                <= crate::transcript::raw::START_SLACK)
                .then_some((owner.pid, owner.session_id))
        })
        .collect()
}

impl super::Launcher for loopflow_harness::Claude {
    fn conversation_owners(
        &self,
        home: &Path,
        processes: &HashMap<u32, &OsProcess>,
    ) -> HashMap<u32, String> {
        conversation_owners(home, processes)
    }

    fn recognizes(&self, process: &crate::process::OsProcess) -> bool {
        if process.command == "claude" {
            return true;
        }
        let name = &process.command;
        if !name.contains('.') || !name.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return false;
        }
        let Some(executable) = crate::process::OsProcess::executable(process.pid) else {
            return false;
        };
        executable.parent().is_some_and(|versions| {
            versions.file_name().is_some_and(|name| name == "versions")
                && versions
                    .parent()
                    .is_some_and(|dir| dir.file_name().is_some_and(|name| name == "claude"))
        })
    }
    fn provider(&self) -> Option<crate::provider_auth::Provider> {
        Some(crate::provider_auth::Provider::Claude)
    }
    fn recent_transcripts(
        &self,
        home: &std::path::Path,
        cwd: &std::path::Path,
        since: i64,
        owned: &std::collections::HashSet<&str>,
    ) -> Vec<(String, loopflow_harness::transcript::Source, i64)> {
        crate::transcript::raw::unowned(
            crate::transcript::raw::in_directory(
                &loopflow_harness::transcript::live::claude_project(home, cwd),
                |path| Some(path.file_stem()?.to_str()?.to_owned()),
            )
            .into_iter()
            .filter(|(_, _, at)| *at >= since)
            .map(|(id, path, at)| (id, loopflow_harness::transcript::Source::File(path), at))
            .collect(),
            owned,
        )
    }
    fn classify_failure(&self, code: &str, message: &str) -> Option<crate::agent::AgentFailure> {
        crate::agent::provider_failure(code, message)
    }
    fn capabilities(&self) -> super::Capabilities {
        super::Capabilities {
            thread: super::ThreadNaming {
                bare: super::ThreadName::Assigned,
                managed: super::ThreadName::Assigned,
            },
            initial_prompt_on_resume: false,
            native_skills: Some(crate::skills::catalog::SkillDialect::Claude),
            installed_skills: super::InstalledSkills {
                global: ".claude/skills",
                repository: ".claude/skills",
            },
            turn_limit: true,
            output_schema: true,
            respawns_after_crash: true,
            install_hint: concat!(
                "Install it with `npm install -g @anthropic-ai/claude-code`, ",
                "then open `claude` to sign in.",
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
        let model = launch.config.agent.model.clone();
        args.extend(crate::agent::claude_options(
            launch.config,
            model.as_deref(),
            launch.context_file,
            launch.kind.mode(),
        ));
        if let Some(thread) = &client.thread {
            args.extend([
                if matches!(launch.start, super::Start::New) {
                    "--session-id"
                } else {
                    "--resume"
                }
                .into(),
                thread.to_string(),
            ]);
        }
        if let Some(context) = &launch.config.conversation_context {
            context.configure_hooks(&mut client.process)?;
        }
        client
            .process
            .args(
                launch
                    .config
                    .conversation_context
                    .as_ref()
                    .map(|context| super::context::claude_args(context, launch.kind))
                    .transpose()?
                    .unwrap_or_default(),
            )
            .args(args);
        if let Some(input) = input {
            client.process.arg("--");
            crate::agent::configure_native_input(&mut client.process, &input)?;
        }
        super::claude_trust::accept_created_worktree(&client.process);
        if super::claude_trust::awaits_dialog(&client.process) {
            super::native_session::report_folder_trust_dialog(launch.recording);
        }
        Ok(client)
    }
}

pub fn stream_args(
    config: &AgentConfig,
    thread: Option<&AgentSessionId>,
    context_file: Option<&std::path::Path>,
) -> Result<Vec<String>> {
    let model = config.agent.model.clone();
    let mut args = vec![
        "-p".into(),
        "--replay-user-messages".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
    ];
    args.extend(crate::agent::claude_options(
        config,
        model.as_deref(),
        context_file,
        super::Mode::Headless,
    ));
    if let Some(thread) = thread {
        args.extend(["--resume".into(), thread.to_string()]);
    }
    Ok(args)
}
