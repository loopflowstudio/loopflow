use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::engine::agent::{AgentConfig, ClaudeArgs};
use crate::lfd::sessions::harness::claude_mapping::ReaderState;
use crate::lfd::sessions::harness::common::{spawn_stderr_logger, TurnInProgressGuard};
use crate::lfd::sessions::harness::{claude_mapping, Harness, HarnessError};
use crate::lfd::sessions::types::{SessionEvent, TurnStatus};

pub struct ClaudeHarness {
    events: broadcast::Sender<SessionEvent>,
    config: Option<AgentConfig>,
    should_seed_task_prompt: bool,
    provider_session_id: Option<String>,
    turn_in_progress: Arc<AtomicBool>,
    child: Option<Child>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    shutdown_requested: Arc<AtomicBool>,
}

impl std::fmt::Debug for ClaudeHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeHarness").finish()
    }
}

/// Build CLI args for a Claude invocation.
fn build_args(content: &str, config: &AgentConfig, resume_id: Option<&str>) -> Vec<String> {
    let mut args = vec!["-p".to_string(), content.to_string()];

    let claude_args = ClaudeArgs {
        model: config.model.as_deref().and_then(ClaudeArgs::resolve_model),
        system_prompt: Some(config.system_prompt.clone()),
        system_prompt_file: None,
        skip_permissions: config.skip_permissions,
        max_turns: config.max_turns,
        stream: true,
        chrome: false,
        resume_id: resume_id.map(str::to_string),
    };
    args.extend(claude_args.to_args());

    args
}

impl ClaudeHarness {
    pub fn new(events: broadcast::Sender<SessionEvent>) -> Self {
        Self {
            events,
            config: None,
            should_seed_task_prompt: true,
            provider_session_id: None,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            child: None,
            reader_task: None,
            stderr_task: None,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl Harness for ClaudeHarness {
    async fn start(&mut self, config: &AgentConfig) -> Result<()> {
        // Validate claude binary on PATH.
        let output = Command::new("claude").arg("--version").output().await;
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

        let _ = self.events.send(SessionEvent::TurnStarted {
            turn_id: turn_id.clone(),
        });

        let args = build_args(&turn_content, config, self.provider_session_id.as_deref());
        let mut cmd = Command::new("claude");
        cmd.args(&args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::null());

        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }

        self.shutdown_requested.store(false, Ordering::SeqCst);

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
        let turn_in_progress = self.turn_in_progress.clone();
        let shutdown = self.shutdown_requested.clone();
        let reader_turn_id = turn_id.clone();
        self.reader_task = Some(tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut state = ReaderState::default();
            let mut saw_turn_completed = false;

            while let Ok(Some(line)) = lines.next_line().await {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if line.trim().is_empty() {
                    continue;
                }

                if claude_mapping::process_line(&line, &reader_turn_id, &events, &mut state) {
                    saw_turn_completed = true;
                    break;
                }
            }

            if !saw_turn_completed && !shutdown.load(Ordering::Relaxed) {
                tracing::warn!(
                    turn_id = %reader_turn_id,
                    "claude turn ended without result event"
                );
                let _ = events.send(SessionEvent::TurnCompleted {
                    turn_id: reader_turn_id,
                    status: TurnStatus::Failed,
                });
            }

            turn_in_progress.store(false, Ordering::SeqCst);
        }));

        self.stderr_task = Some(spawn_stderr_logger(stderr, "claude_harness"));

        turn_guard.disarm();
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);

        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        if let Some(task) = self.reader_task.take() {
            task.abort();
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }

        self.turn_in_progress.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn set_provider_session_id(&mut self, provider_session_id: Option<String>) {
        self.provider_session_id = provider_session_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_minimal() {
        let config = AgentConfig {
            system_prompt: String::new(),
            task_prompt: "task".to_string(),
            model: None,
            cwd: Some("/tmp".into()),
            max_turns: None,
            skip_permissions: false,
        };
        let args = build_args("hello", &config, None);
        assert_eq!(
            args,
            vec!["-p", "hello", "--output-format", "stream-json", "--verbose"]
        );
    }

    #[test]
    fn build_args_full() {
        let config = AgentConfig {
            system_prompt: "Be concise".to_string(),
            task_prompt: "task".to_string(),
            model: Some("claude-sonnet-4-5-20250514".to_string()),
            cwd: Some("/tmp".into()),
            max_turns: Some(5),
            skip_permissions: true,
        };
        let args = build_args("fix tests", &config, Some("sess_abc"));
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"sess_abc".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-5-20250514".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"5".to_string()));
        assert!(args.contains(&"--append-system-prompt".to_string()));
        assert!(args.contains(&"Be concise".to_string()));
    }

    #[test]
    fn build_args_resume_without_extras() {
        let config = AgentConfig {
            system_prompt: String::new(),
            task_prompt: "task".to_string(),
            model: None,
            cwd: Some("/tmp".into()),
            max_turns: None,
            skip_permissions: false,
        };
        let args = build_args("next", &config, Some("sess_123"));
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"sess_123".to_string()));
        assert!(!args.contains(&"--model".to_string()));
        assert!(!args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(!args.contains(&"--max-turns".to_string()));
    }

    #[tokio::test]
    async fn send_input_spawn_failure_releases_turn_guard() {
        let (tx, _rx) = broadcast::channel(16);
        let mut harness = ClaudeHarness::new(tx);
        harness.config = Some(AgentConfig {
            system_prompt: String::new(),
            task_prompt: "task".to_string(),
            model: None,
            cwd: Some(format!("/tmp/loopflow-missing-{}", uuid::Uuid::new_v4()).into()),
            max_turns: None,
            skip_permissions: false,
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
}
