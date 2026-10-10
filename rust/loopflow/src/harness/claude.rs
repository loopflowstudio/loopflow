use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::agent::{build_claude_stream_session_args, AgentConfig};
use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::harness::claude_mapping::ReaderState;
use crate::harness::common::{spawn_stderr_logger, TurnInProgressGuard};
use crate::harness::{claude_mapping, Harness, HarnessError, RawProviderEvent, SendCurrentOutcome};
use crate::id::AgentSessionId;
use crate::provider_account::{resolve_provider_account_exact, ProviderAccountRoute};
use crate::provider_auth::Provider;
use crate::store::ProviderAccountId;

/// One persistent `claude -p --input-format stream-json` process drives every
/// turn of a run. `send_input` writes the seed message; the process stays alive
/// (holding conversation context, warm prompt cache) for the next turn instead
/// of respawning. `send_current` writes a steer mid-turn — claude applies it as
/// the next turn in the same session — and the reader coalesces the seed turn
/// and its queued steers into one `TurnCompleted` so the runner still sees one
/// boundary per `send_input`.
pub struct ClaudeHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
    config: Option<AgentConfig>,
    capture: Option<crate::agent::AgentCapture>,
    should_seed_task_prompt: bool,
    /// Vendor session id captured from the first turn's `system` event; a
    /// respawn (after interrupt/crash) resumes it via `--resume`.
    agent_session: Arc<Mutex<Option<AgentSessionId>>>,
    account_route: Option<ProviderAccountRoute>,
    requested_account_id: Option<ProviderAccountId>,
    turn_in_progress: Arc<AtomicBool>,
    /// Provider results still owed for the current runner turn: 1 for the seed,
    /// plus 1 per accepted `send_current`. The reader emits `TurnCompleted` only
    /// when this reaches 0, coalescing queued steer turns into one boundary.
    pending_results: Arc<AtomicI64>,
    /// The runner turn id every provider turn in the current coalesced boundary
    /// reports under. Set by `send_input`, read by the reader.
    current_turn_id: Arc<Mutex<Option<String>>>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    shutdown_requested: Arc<AtomicBool>,
    interrupt_requested: Arc<AtomicBool>,
}

impl std::fmt::Debug for ClaudeHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeHarness").finish()
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

impl ClaudeHarness {
    pub fn new(events: mpsc::UnboundedSender<ConversationEvent>) -> Self {
        Self {
            events,
            raw_provider: None,
            config: None,
            capture: None,
            should_seed_task_prompt: true,
            agent_session: Arc::new(Mutex::new(None)),
            account_route: None,
            requested_account_id: None,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            pending_results: Arc::new(AtomicI64::new(0)),
            current_turn_id: Arc::new(Mutex::new(None)),
            child: None,
            stdin: None,
            reader_task: None,
            stderr_task: None,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            interrupt_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Spawn the persistent stream-json process and its reader, if not already
    /// running. Resumes the captured vendor session after an interrupt/crash.
    async fn ensure_process(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }
        let config = self
            .config
            .as_mut()
            .ok_or_else(|| anyhow!("claude harness not started"))?;
        if let (Some(capture), Some((session, attachment))) =
            (&self.capture, &mut config.session_attachment)
        {
            *attachment = capture.prepare_agent_process(session, attachment)?;
            config.env.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&attachment.caller(session.clone()))?,
            );
        }
        let resume_id = self
            .agent_session
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone();
        let context_file = crate::agent::write_system_prompt_file(config, "session")?;
        let args =
            build_claude_stream_session_args(config, resume_id.as_ref(), context_file.as_deref());
        let mut cmd = Command::new("claude");
        cmd.args(&args);
        super::configure_agent_env(&mut cmd, config);
        let activation = match &self.account_route {
            Some(route) => route.launch_as(cmd.as_std_mut()).await?,
            None => None,
        };
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }
        super::configure_vendor_std_env(cmd.as_std_mut())?;
        self.shutdown_requested.store(false, Ordering::SeqCst);

        let owner = self.owner()?;
        let mut child = super::agent_process::spawn(cmd, &owner)?;
        drop(activation);
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

        self.spawn_reader(stdout, super::claude_history::History::new(owner)?);
        self.stderr_task = Some(spawn_stderr_logger(stderr, "claude_harness"));
        self.stdin = Some(stdin);
        self.child = Some(child);
        Ok(())
    }

    /// The persistent NDJSON reader: maps events for the whole process lifetime
    /// and emits one `TurnCompleted` per coalesced runner turn.
    fn spawn_reader(
        &mut self,
        stdout: tokio::process::ChildStdout,
        mut history: super::claude_history::History,
    ) {
        let events = self.events.clone();
        let raw_provider = self.raw_provider.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let pending_results = self.pending_results.clone();
        let current_turn_id = self.current_turn_id.clone();
        let shutdown = self.shutdown_requested.clone();
        let interrupted = self.interrupt_requested.clone();
        let session_slot = self.agent_session.clone();
        let account_route = self.account_route.clone();
        self.reader_task = Some(tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut state = ReaderState::default();
            let turn_id = || {
                current_turn_id
                    .lock()
                    .expect("claude turn id lock poisoned")
                    .clone()
                    .unwrap_or_default()
            };

            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(raw_provider) = &raw_provider {
                    let _ = raw_provider.send(RawProviderEvent {
                        stream: "stdout",
                        line: line.clone(),
                    });
                }
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if line.trim().is_empty() {
                    continue;
                }

                if let (Some(route), Some(signal)) = (
                    account_route.as_ref(),
                    claude_mapping::rate_limit_signal(&line),
                ) {
                    if let Err(error) = route.record_rate_limit(&signal).await {
                        tracing::warn!(%error, "failed to record Claude account rate limit");
                    }
                    if signal.limited {
                        let _ = events.send(ConversationEvent::Error {
                            code: "provider_rate_limited".to_string(),
                            message: signal.reason,
                            evidence: None,
                        });
                        pending_results.store(0, Ordering::SeqCst);
                        let _ = events.send(ConversationEvent::TurnCompleted {
                            turn_id: turn_id(),
                            status: Lifecycle::Failed,
                        });
                        turn_in_progress.store(false, Ordering::SeqCst);
                        state = ReaderState::default();
                        continue;
                    }
                }

                if let Err(error) = history.record(&line) {
                    let _ = events.send(ConversationEvent::Error {
                        code: "claude_history".into(),
                        message: error.to_string(),
                        evidence: None,
                    });
                    break;
                }
                let result = claude_mapping::process_line(&line, &turn_id(), &events, &mut state);
                if let Some(session_id) = state.take_agent_session() {
                    *session_slot
                        .lock()
                        .expect("claude provider session id lock poisoned") =
                        Some(session_id.clone());
                    if let Some(route) = &account_route {
                        if let Err(error) = route.pin_session(&session_id).await {
                            tracing::warn!(%error, "failed to pin Claude provider session account");
                        }
                    }
                }
                if let Some(status) = result {
                    // One provider turn ended. Only close the runner turn once
                    // every queued steer turn has also drained.
                    state = ReaderState::default();
                    if pending_results.fetch_sub(1, Ordering::SeqCst) <= 1 {
                        pending_results.store(0, Ordering::SeqCst);
                        let _ = events.send(ConversationEvent::TurnCompleted {
                            turn_id: turn_id(),
                            status,
                        });
                        turn_in_progress.store(false, Ordering::SeqCst);
                    }
                }
            }

            // The process ended. If a turn was open (interrupt or crash), close
            // it so the runner is never left waiting.
            if turn_in_progress.load(Ordering::SeqCst) && !shutdown.load(Ordering::Relaxed) {
                let status = if interrupted.load(Ordering::SeqCst) {
                    Lifecycle::Interrupted
                } else {
                    tracing::warn!("claude stream ended without a final result");
                    Lifecycle::Failed
                };
                for item in state.drain_open_items(status) {
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
            pending_results.store(0, Ordering::SeqCst);
            turn_in_progress.store(false, Ordering::SeqCst);
        }));
    }

    /// Tear the persistent process down and reap its tasks. The next
    /// `send_input` respawns and resumes the captured session.
    async fn kill_process(&mut self) -> Result<()> {
        if self.child.is_some() {
            super::agent_process::stop(&self.owner()?)?;
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
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }
        self.pending_results.store(0, Ordering::SeqCst);
        self.turn_in_progress.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn owner(&self) -> Result<super::agent_process::AttachmentOwner> {
        super::agent_process::open_owner(
            self.config
                .as_ref()
                .and_then(|config| config.session_attachment.as_ref()),
        )
    }

    async fn send_line(&mut self, line: String, request: String) -> Result<()> {
        let mut stdin = self
            .stdin
            .take()
            .ok_or_else(|| anyhow!("claude stdin not available"))?;
        let (store, session, attachment) = match self.owner() {
            Ok(owner) => owner,
            Err(error) => {
                self.stdin = Some(stdin);
                return Err(error);
            }
        };
        let (stdin, result) = tokio::task::spawn_blocking(move || {
            let result = store.with_session_attachment(&session, &attachment, || {
                let origin = store.session_turn_origin(&session, &attachment)?;
                // Claude has not necessarily reported a native Session yet.
                // Persist the UUID's origin before any bytes reach the provider.
                store.record_session_request(None, &request, &origin)?;
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
impl Harness for ClaudeHarness {
    fn set_capture(&mut self, capture: Option<crate::agent::AgentCapture>) {
        self.capture = capture;
    }
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
        let requested_session = self
            .agent_session
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone();
        let account_route = resolve_provider_account_exact(
            Provider::Claude,
            requested_session.as_ref(),
            self.requested_account_id.as_ref(),
        )
        .await?;
        if account_route
            .as_ref()
            .is_some_and(|route| !route.resume_requested_session())
        {
            *self
                .agent_session
                .lock()
                .expect("claude provider session id lock poisoned") = None;
        }
        self.account_route = account_route;

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

        self.config = Some(config.clone());
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

        // The first message of a run carries the task prompt as a preamble.
        let mut turn_content = content.to_string();
        if self.should_seed_task_prompt {
            let task_prompt = self
                .config
                .as_ref()
                .ok_or_else(|| anyhow!("claude harness not started"))?
                .task_prompt
                .trim()
                .to_string();
            self.should_seed_task_prompt = false;
            if !task_prompt.is_empty() {
                turn_content = format!("{task_prompt}\n\n{content}");
            }
        }

        self.interrupt_requested.store(false, Ordering::SeqCst);
        self.ensure_process().await?;

        let turn_id = uuid::Uuid::new_v4().to_string();
        *self
            .current_turn_id
            .lock()
            .expect("claude turn id lock poisoned") = Some(turn_id.clone());
        // One result owed for this seed; each accepted steer adds another.
        self.pending_results.store(1, Ordering::SeqCst);
        let _ = self.events.send(ConversationEvent::TurnStarted {
            turn_id: turn_id.clone(),
        });

        if let Err(error) = self
            .send_line(user_message_line(&turn_content, &turn_id), turn_id.clone())
            .await
        {
            // The process died between spawn and write; tear it down so the
            // next send_input respawns cleanly.
            drop(turn_guard);
            self.kill_process().await?;
            return Err(anyhow!("failed to write claude seed message: {error}"));
        }

        turn_guard.disarm();
        Ok(())
    }

    async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
        if self.stdin.is_none() {
            return SendCurrentOutcome::NotSteerable;
        }
        // Atomically join the open boundary. A separate bool check followed by
        // `fetch_add` races the reader's final `fetch_sub`: a steer could be
        // accepted after TurnCompleted and escape as a second boundary.
        if self
            .pending_results
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |pending| {
                (pending > 0).then(|| pending.saturating_add(1))
            })
            .is_err()
        {
            return SendCurrentOutcome::NotSteerable;
        }
        let request = uuid::Uuid::new_v4().to_string();
        if let Err(error) = self
            .send_line(user_message_line(content, &request), request)
            .await
        {
            self.pending_results.fetch_sub(1, Ordering::SeqCst);
            return SendCurrentOutcome::Failed {
                error: format!("failed to write claude steer: {error}"),
            };
        }
        let provider_turn_id = self
            .current_turn_id
            .lock()
            .expect("claude turn id lock poisoned")
            .clone()
            .unwrap_or_default();
        SendCurrentOutcome::Sent { provider_turn_id }
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

    fn set_agent_session(&mut self, agent_session: Option<AgentSessionId>) {
        *self
            .agent_session
            .lock()
            .expect("claude provider session id lock poisoned") = agent_session;
    }

    fn set_provider_account_id(&mut self, account_id: Option<ProviderAccountId>) {
        self.requested_account_id = account_id;
    }

    fn provider_account_id(&self) -> Option<ProviderAccountId> {
        self.account_route
            .as_ref()
            .map(|route| route.account_id().clone())
    }
}

#[cfg(test)]
mod activity_tests {
    use std::time::Duration;

    use tokio::process::Command;
    use tokio::sync::mpsc;

    use super::ClaudeHarness;
    use crate::harness::Harness;
    use crate::session_record::{
        activity, CaptureHandle, SessionCaptureSpec, SessionFlowMembership,
    };

    #[tokio::test]
    async fn claude_body_activity_does_not_require_process_group_authority() {
        let home = tempfile::tempdir().unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = ClaudeHarness::new(tx);
        harness.child = Some(
            Command::new("sleep")
                .arg("300")
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
        );
        let capture = CaptureHandle::begin_at(
            home.path(),
            SessionCaptureSpec {
                harness: "claude".into(),
                model: None,
                surface: "headless".into(),
                cwd: home.path().into(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: vec![],
                flow: SessionFlowMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        capture.observe_activity(harness.pid()).await;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if activity::read(home.path(), &capture.artifact_key()).await
                    == activity::Activity::Running
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("a live Claude child must have observable CPU without a process group");
        assert_eq!(harness.process_group_id(), None);
        // The injected child was never admitted; kill_on_drop ends it.
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Isolate provider and database selection.
    async fn claude_stop_fences_takeover_and_ends_the_recorded_groups() {
        let _lock = crate::journal::test_env_lock();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN", "PATH"]);
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("loopflow.db");
        std::env::set_var("LF_HOME", home.path());
        let store = crate::store::sqlite::SqliteStore::open_ephemeral(&database).unwrap();
        let conn = rusqlite::Connection::open(&database).unwrap();
        let process = crate::id::LfProcessId::new();
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
            let attachment = store
                .claim_session_attachment(id, None, &process, true)
                .unwrap();
            let (tx, _rx) = mpsc::unbounded_channel();
            let mut harness = ClaudeHarness::new(tx);
            let mut config = live_config();
            config.session_attachment = Some((id.into(), attachment.clone()));
            harness.config = Some(config);
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
        let original = harnesses[0].owner().unwrap().2;
        let replacement = store
            .claim_session_attachment("first", Some(&original), &process, false)
            .unwrap();
        assert!(harnesses[0].stop().await.is_err());
        assert_eq!(
            crate::journal::process_identity_evidence(recorded[0].0, recorded[0].1),
            crate::journal::ProcessIdentityEvidence::Live
        );
        harnesses[0].config.as_mut().unwrap().session_attachment =
            Some(("first".into(), replacement));
        for harness in &mut harnesses {
            harness.stop().await.unwrap();
        }
        assert_ne!(recorded[0].0, recorded[1].0);
        for (pid, _) in recorded {
            assert!(!crate::journal::OsProcess::group_is_alive(pid).unwrap());
        }
    }

    #[test]
    fn interrupted_capture_resumes_with_a_new_process_and_fences_old_snapshots() {
        use crate::session_record::{CaptureHandle, SessionCaptureSpec, SessionFlowMembership};

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
            let capture = CaptureHandle::begin_at(
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
                    flow: SessionFlowMembership::Independent,
                    work: None,
                },
            )?;
            capture.claim_session_attachment()?;
            let (session, first) = capture.session_attachment().unwrap();
            let store = crate::store::sqlite::SqliteStore::new(&home.join("loopflow.db"))?;
            runtime.block_on(async {
                let (tx, _rx) = mpsc::unbounded_channel();
                let mut harness = ClaudeHarness::new(tx);
                harness.config = Some(AgentConfig {
                    cwd: Some(home.into()),
                    session_attachment: Some((session.clone(), first.clone())),
                    ..AgentConfig::default()
                });
                harness.set_capture(Some(capture.clone().into()));
                harness.send_input("first request").await.unwrap();
                // Pipe acceptance precedes provider execution. Interrupt only
                // after this throwaway provider consumed its first request.
                observed_input(home, "first request").await;
                let first_pid = harness.pid().unwrap();
                // The native conversation survives the OS process's interruption.
                harness.set_agent_session(Some("native-conversation".into()));
                harness.interrupt().await.unwrap();
                let ended = store.process(&first.agent_process_id).unwrap().unwrap();
                assert_eq!(ended.pid, Some(first_pid));
                assert!(ended.completed_at.is_some());

                harness.send_input("resumed request").await.unwrap();
                let (_, second) = capture.session_attachment().unwrap();
                assert_ne!(first.agent_process_id, second.agent_process_id);
                assert_ne!(first.token, second.token);
                let running = store.process(&second.agent_process_id).unwrap().unwrap();
                assert_eq!(running.pid, harness.pid());
                assert_eq!(running.parent_lf_process_id, first.lf_process_id);
                assert!(running.completed_at.is_none());
                assert_eq!(store.process(&first.agent_process_id).unwrap(), Some(ended));
                assert!(capture.prepare_agent_process(&session, &first).is_err());
                assert!(store
                    .record_session_connection(&session, &first, "stale", &"stale".into())
                    .is_err());

                // Pending operations retain the old snapshot, not the capture's
                // current value. Neither a pipe write nor a signal may use it.
                harness.config.as_mut().unwrap().session_attachment =
                    Some((session.clone(), first));
                assert!(harness
                    .send_line("stale request\n".into(), "stale".into())
                    .await
                    .is_err());
                assert!(harness.stop().await.is_err());
                assert!(harness
                    .child
                    .as_mut()
                    .unwrap()
                    .try_wait()
                    .unwrap()
                    .is_none());
                harness.config.as_mut().unwrap().session_attachment =
                    Some((session.clone(), second.clone()));
                assert!(!observed_input(home, "resumed request")
                    .await
                    .contains("stale request"));
                let launches = std::fs::read_to_string(home.join("launches")).unwrap();
                assert_eq!(launches.lines().count(), 2);
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
        let mut harness = ClaudeHarness::new(tx);
        harness.config = Some(AgentConfig {
            chrome: false,
            session_attachment: None,
            system_prompt: String::new(),
            task_prompt: "task".to_string(),
            skill_invocation: None,
            agent: None,
            cwd: Some(format!("/tmp/loopflow-missing-{}", uuid::Uuid::new_v4()).into()),
            max_turns: None,
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: crate::agent::AgentWriteScope::Configured,
            execution_boundary: None,
            skip_permissions: false,
            structured_replies: Vec::new(),
            directive_relay: None,
            env: Default::default(),
        });

        let first = harness
            .send_input("first")
            .await
            .expect_err("spawn should fail for missing cwd");
        assert!(
            !matches!(
                first.downcast_ref::<HarnessError>(),
                Some(HarnessError::TurnAlreadyInProgress)
            ),
            "first failure should not be turn-in-progress"
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
        let mut harness = ClaudeHarness::new(tx);
        harness.interrupt().await.expect("noop interrupt");
        assert!(!harness.interrupt_requested.load(Ordering::SeqCst));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Isolate the stand-in provider and its homes.
    async fn agent_session_resumes_on_a_fresh_process() {
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
        let script = home.path().join("claude");
        std::fs::write(&script, r#"#!/bin/sh
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
"#).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Each harness is its own admitted conversation; both resume one
        // provider-owned AgentSession.
        let database = home.path().join("loopflow.db");
        let store = crate::store::sqlite::SqliteStore::open_ephemeral(&database).unwrap();
        let process = crate::id::LfProcessId::new();
        rusqlite::Connection::open(&database)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,1)",
                [&process],
            )
            .unwrap();
        let admitted = |id: &str| {
            store.test_session(id, &crate::session_record::new_artifact_key());
            rusqlite::Connection::open(&database)
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET interactive=0, provider='claude' WHERE id=?1",
                    [id],
                )
                .unwrap();
            let attachment = store
                .claim_session_attachment(id, None, &process, true)
                .unwrap();
            let mut config = live_config();
            config.cwd = Some(home.path().to_path_buf());
            config.session_attachment = Some((id.into(), attachment));
            config
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut first = ClaudeHarness::new(tx);
        first.config = Some(admitted("first"));
        first.send_input("remember").await.unwrap();
        let (status, answer, _) = tokio::time::timeout(Duration::from_secs(5), drive_turn(&mut rx))
            .await
            .unwrap();
        assert_eq!(status, Lifecycle::Completed);
        assert!(answer.contains("created"));
        let agent_session: AgentSessionId = first.agent_session().unwrap();

        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut resumed = ClaudeHarness::new(tx);
        resumed.set_agent_session(Some(agent_session.clone()));
        resumed.config = Some(admitted("second"));
        resumed.send_input("recall").await.unwrap();
        let (status, answer, _) = tokio::time::timeout(Duration::from_secs(5), drive_turn(&mut rx))
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

    // Live persistent-process checks against the real `claude` CLI (subscription
    // auth). Ignored by default — run explicitly with a live login:
    //   cargo test -p loopflow --lib claude::tests::live_ -- --ignored --nocapture
    fn live_config() -> AgentConfig {
        AgentConfig {
            chrome: false,
            session_attachment: None,
            system_prompt: String::new(),
            task_prompt: String::new(),
            skill_invocation: None,
            agent: None,
            cwd: Some(std::env::temp_dir()),
            max_turns: None,
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: crate::agent::AgentWriteScope::Configured,
            execution_boundary: None,
            skip_permissions: false,
            structured_replies: Vec::new(),
            directive_relay: None,
            env: Default::default(),
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
                    item: crate::chat::types::ConversationItem::Message { text: t, .. },
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
        let mut harness = ClaudeHarness::new(tx);
        let mut config = live_config();
        let _ledger = super::super::admit_for_test(&mut config);
        harness.start(&config).await.expect("start");

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

    #[tokio::test]
    #[ignore = "drives the real claude CLI; needs a live subscription login"]
    async fn live_send_current_coalesces_into_one_boundary() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = ClaudeHarness::new(tx);
        let mut config = live_config();
        let _ledger = super::super::admit_for_test(&mut config);
        harness.start(&config).await.expect("start");

        harness
            .send_input("Write a slow, detailed 200-word explanation of how a bicycle works.")
            .await
            .expect("seed turn");
        // Inject a steer while the seed turn is still generating.
        tokio::time::sleep(Duration::from_secs(2)).await;
        let outcome = harness
            .send_current("IMPORTANT: also include the word PANGOLIN in your reply.")
            .await;
        assert!(
            matches!(outcome, SendCurrentOutcome::Sent { .. }),
            "steer accepted into the live turn: {outcome:?}"
        );

        // The reader must coalesce the seed turn and the queued steer turn into
        // exactly one TurnCompleted for this one send_input.
        let (status, text, completions) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert_eq!(completions, 1, "one coalesced boundary for the send_input");
        assert!(
            text.to_uppercase().contains("PANGOLIN"),
            "the steer was incorporated: {text:?}"
        );
        // The coalesced boundary drained the pending counter.
        assert_eq!(harness.pending_results.load(Ordering::SeqCst), 0);
        assert!(!harness.turn_in_progress.load(Ordering::SeqCst));

        harness.stop().await.expect("stop");
    }
}
