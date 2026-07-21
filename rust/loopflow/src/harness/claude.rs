use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::engine::agent::{build_claude_session_turn_args, AgentConfig};
use crate::harness::claude_mapping::ReaderState;
use crate::harness::common::{spawn_stderr_logger, TurnInProgressGuard};
use crate::harness::{claude_mapping, Harness, HarnessError, RawProviderEvent};
use crate::provider_account::{resolve_provider_account_exact, ProviderAccountRoute};
use crate::provider_auth::Provider;
use crate::store::ProviderAccountId;

pub struct ClaudeHarness {
    events: mpsc::UnboundedSender<ConversationEvent>,
    raw_provider: Option<mpsc::UnboundedSender<RawProviderEvent>>,
    config: Option<AgentConfig>,
    should_seed_task_prompt: bool,
    /// Vendor session id captured from the first turn's `system` event;
    /// subsequent turns resume it via `--resume`.
    provider_session_id: Arc<Mutex<Option<String>>>,
    account_route: Option<ProviderAccountRoute>,
    requested_account_id: Option<ProviderAccountId>,
    turn_in_progress: Arc<AtomicBool>,
    child: Option<Child>,
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
            child: None,
            reader_task: None,
            stderr_task: None,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            interrupt_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn kill_turn_process(&mut self) {
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

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| anyhow!("claude harness not started"))?;
        let mut turn_content = content.to_string();
        if self.should_seed_task_prompt {
            self.should_seed_task_prompt = false;
            if !config.task_prompt.trim().is_empty() {
                turn_content = format!("{}\n\n{}", config.task_prompt.trim(), content);
            }
        }

        let turn_id = format!("turn_{}", uuid::Uuid::new_v4());

        let _ = self.events.send(ConversationEvent::TurnStarted {
            turn_id: turn_id.clone(),
        });

        let resume_id = self
            .provider_session_id
            .lock()
            .expect("claude provider session id lock poisoned")
            .clone();
        let args = build_claude_session_turn_args(&turn_content, config, resume_id.as_deref());
        let mut cmd = Command::new("claude");
        cmd.args(&args);
        if let Some(route) = &self.account_route {
            route.apply_tokio(&mut cmd);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::null());

        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }
        super::configure_vendor_tokio_env(&mut cmd)?;
        super::configure_agent_authority(&mut cmd, config.authority);

        self.shutdown_requested.store(false, Ordering::SeqCst);
        self.interrupt_requested.store(false, Ordering::SeqCst);

        let mut child = cmd
            .spawn()
            .map_err(|err| anyhow!("failed to spawn claude: {err}"))?;

        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(anyhow!("failed to capture claude stdout"));
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                return Err(anyhow!("failed to capture claude stderr"));
            }
        };

        self.child = Some(child);

        // Spawn reader task for NDJSON stdout.
        let events = self.events.clone();
        let raw_provider = self.raw_provider.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let shutdown = self.shutdown_requested.clone();
        let interrupted = self.interrupt_requested.clone();
        let session_slot = self.provider_session_id.clone();
        let account_route = self.account_route.clone();
        let reader_turn_id = turn_id.clone();
        self.reader_task = Some(tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut state = ReaderState::default();
            let mut saw_turn_completed = false;

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
                        saw_turn_completed = true;
                        break;
                    }
                }

                let done =
                    claude_mapping::process_line(&line, &reader_turn_id, &events, &mut state);
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
                if done {
                    saw_turn_completed = true;
                    break;
                }
            }

            if !saw_turn_completed && !shutdown.load(Ordering::Relaxed) {
                // The per-turn process died without a result event: either we
                // killed it (interrupt) or it crashed (failed).
                let status = if interrupted.load(Ordering::SeqCst) {
                    Lifecycle::Interrupted
                } else {
                    tracing::warn!(
                        turn_id = %reader_turn_id,
                        "claude turn ended without result event"
                    );
                    Lifecycle::Failed
                };
                for item in state.drain_open_items(status) {
                    let _ = events.send(ConversationEvent::ItemCompleted {
                        turn_id: reader_turn_id.clone(),
                        item,
                    });
                }
                let _ = events.send(ConversationEvent::TurnCompleted {
                    turn_id: reader_turn_id,
                    status,
                });
            }

            turn_in_progress.store(false, Ordering::SeqCst);
        }));

        self.stderr_task = Some(spawn_stderr_logger(stderr, "claude_harness"));

        turn_guard.disarm();
        Ok(())
    }

    async fn interrupt(&mut self) -> Result<()> {
        // Claude has no per-turn cancel RPC: every turn is its own
        // `claude -p` process. Interrupt kills that process (no cooperative
        // grace); the reader finalizes the turn as Interrupted, and the
        // vendor session — addressed by the captured session id — survives
        // for the next `--resume` turn.
        if !self.turn_in_progress.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.interrupt_requested.store(true, Ordering::SeqCst);
        self.kill_turn_process().await;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);
        self.kill_turn_process().await;
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
            authority: crate::engine::agent::AgentAuthority::Inherit,
            skip_permissions: false,
            structured_replies: Vec::new(),
            directive_relay: None,
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
}
