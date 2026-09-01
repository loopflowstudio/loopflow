use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::engine::agent::{build_claude_stream_session_args, AgentConfig};
use crate::harness::claude_mapping::ReaderState;
use crate::harness::common::{spawn_stderr_logger, TurnInProgressGuard};
use crate::harness::{claude_mapping, Harness, HarnessError, RawProviderEvent, SendCurrentOutcome};
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
    should_seed_task_prompt: bool,
    /// Vendor session id captured from the first turn's `system` event; a
    /// respawn (after interrupt/crash) resumes it via `--resume`.
    provider_session_id: Arc<Mutex<Option<String>>>,
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
fn user_message_line(text: &str) -> String {
    let message = serde_json::json!({
        "type": "user",
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
            should_seed_task_prompt: true,
            provider_session_id: Arc::new(Mutex::new(None)),
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
            .as_ref()
            .ok_or_else(|| anyhow!("claude harness not started"))?;
        let resume_id = self
            .provider_session_id
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone();
        let args = build_claude_stream_session_args(config, resume_id.as_deref());
        let mut cmd = Command::new("claude");
        cmd.args(&args);
        super::configure_agent_env(&mut cmd, config);
        if let Some(route) = &self.account_route {
            route.apply_tokio(&mut cmd);
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }
        super::configure_vendor_tokio_env(&mut cmd)?;
        self.shutdown_requested.store(false, Ordering::SeqCst);

        let mut child = cmd
            .spawn()
            .map_err(|err| anyhow!("failed to spawn claude: {err}"))?;
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

        self.spawn_reader(stdout);
        self.stderr_task = Some(spawn_stderr_logger(stderr, "claude_harness"));
        self.stdin = Some(stdin);
        self.child = Some(child);
        Ok(())
    }

    /// The persistent NDJSON reader: maps events for the whole process lifetime
    /// and emits one `TurnCompleted` per coalesced runner turn.
    fn spawn_reader(&mut self, stdout: tokio::process::ChildStdout) {
        let events = self.events.clone();
        let raw_provider = self.raw_provider.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let pending_results = self.pending_results.clone();
        let current_turn_id = self.current_turn_id.clone();
        let shutdown = self.shutdown_requested.clone();
        let interrupted = self.interrupt_requested.clone();
        let session_slot = self.provider_session_id.clone();
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

                let result = claude_mapping::process_line(&line, &turn_id(), &events, &mut state);
                if let Some(session_id) = state.take_provider_session_id() {
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
    async fn kill_process(&mut self) {
        self.stdin = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
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
    }
}

#[async_trait]
impl Harness for ClaudeHarness {
    fn set_raw_provider_sender(
        &mut self,
        raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
    ) {
        self.raw_provider = raw_provider;
    }

    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        let requested_session = self
            .provider_session_id
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone();
        let account_route = resolve_provider_account_exact(
            Provider::Claude,
            requested_session.as_deref(),
            self.requested_account_id.as_ref(),
        )
        .await?;
        if account_route
            .as_ref()
            .is_some_and(|route| !route.resume_requested_session())
        {
            *self
                .provider_session_id
                .lock()
                .expect("claude provider session id lock poisoned") = None;
        }
        self.account_route = account_route;

        // Validate claude binary on PATH.
        let mut version_command = Command::new("claude");
        version_command.arg("--version");
        super::configure_agent_env(&mut version_command, config);
        if let Some(route) = &self.account_route {
            route.apply_tokio(&mut version_command);
        }
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

        let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
        *self
            .current_turn_id
            .lock()
            .expect("claude turn id lock poisoned") = Some(turn_id.clone());
        // One result owed for this seed; each accepted steer adds another.
        self.pending_results.store(1, Ordering::SeqCst);
        let _ = self.events.send(ConversationEvent::TurnStarted {
            turn_id: turn_id.clone(),
        });

        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("claude stdin not available"))?;
        if let Err(error) = stdin
            .write_all(user_message_line(&turn_content).as_bytes())
            .await
        {
            // The process died between spawn and write; tear it down so the
            // next send_input respawns cleanly.
            drop(turn_guard);
            self.kill_process().await;
            return Err(anyhow!("failed to write claude seed message: {error}"));
        }
        let _ = stdin.flush().await;

        turn_guard.disarm();
        Ok(())
    }

    async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
        let Some(stdin) = self.stdin.as_mut() else {
            return SendCurrentOutcome::NotSteerable;
        };
        // Atomically join the open boundary. A separate bool check followed by
        // `fetch_add` races the reader's final `fetch_sub`: a steer could be
        // accepted after TurnCompleted and escape as a second boundary.
        if self
            .pending_results
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |pending| {
                (pending > 0).then(|| pending.saturating_add(1))
            })
            .is_err()
        {
            return SendCurrentOutcome::NotSteerable;
        }
        if let Err(error) = stdin.write_all(user_message_line(content).as_bytes()).await {
            self.pending_results.fetch_sub(1, Ordering::SeqCst);
            return SendCurrentOutcome::Failed {
                error: format!("failed to write claude steer: {error}"),
            };
        }
        let _ = stdin.flush().await;
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
        self.kill_process().await;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        self.kill_process().await;
        Ok(())
    }

    fn provider_session_id(&self) -> Option<String> {
        self.provider_session_id
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone()
    }

    fn set_provider_session_id(&mut self, provider_session_id: Option<String>) {
        *self
            .provider_session_id
            .lock()
            .expect("claude provider session id lock poisoned") = provider_session_id;
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
mod tests {
    use super::*;

    // build_claude_session_turn_args coverage lives in engine::agent::tests.

    #[tokio::test]
    async fn send_input_spawn_failure_releases_turn_guard() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut harness = ClaudeHarness::new(tx);
        harness.config = Some(AgentConfig {
            system_prompt: String::new(),
            task_prompt: "task".to_string(),
            agent: None,
            cwd: Some(format!("/tmp/loopflow-missing-{}", uuid::Uuid::new_v4()).into()),
            max_turns: None,
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: crate::engine::agent::AgentWriteScope::Configured,
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

    // Live persistent-process checks against the real `claude` CLI (subscription
    // auth). Ignored by default — run explicitly with a live login:
    //   cargo test -p loopflow --lib claude::tests::live_ -- --ignored --nocapture
    fn live_config() -> AgentConfig {
        AgentConfig {
            system_prompt: String::new(),
            task_prompt: String::new(),
            agent: None,
            cwd: Some(std::env::temp_dir()),
            max_turns: None,
            resume_token: None,
            provider_account_id: None,
            provider_account_authority_home: None,
            write_scope: crate::engine::agent::AgentWriteScope::Configured,
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
        harness.start(&live_config()).await.expect("start");

        harness
            .send_input("Reply with exactly: ALPHA")
            .await
            .expect("first turn");
        let (status, text, _) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert!(text.contains("ALPHA"), "first turn text: {text:?}");

        // Same persistent process, second turn — the session id must be stable.
        let session_after_first = harness.provider_session_id();
        harness
            .send_input("Reply with exactly: BETA")
            .await
            .expect("second turn");
        let (status, text, _) = drive_turn(&mut rx).await;
        assert_eq!(status, Lifecycle::Completed);
        assert!(text.contains("BETA"), "second turn text: {text:?}");
        assert_eq!(
            session_after_first,
            harness.provider_session_id(),
            "the persistent process keeps one vendor session across turns"
        );

        harness.stop().await.expect("stop");
    }

    #[tokio::test]
    #[ignore = "drives the real claude CLI; needs a live subscription login"]
    async fn live_send_current_coalesces_into_one_boundary() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut harness = ClaudeHarness::new(tx);
        harness.start(&live_config()).await.expect("start");

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
