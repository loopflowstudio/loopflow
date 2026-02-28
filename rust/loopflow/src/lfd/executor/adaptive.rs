use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::lfd::config::ExecutorConfig;
use crate::lfd::output::OutputHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::Wave;

use super::docker::DockerExecutor;
use super::sandbox::SandboxExecutor;
use super::{AgentExecutor, AgentRunContext, StartupRecovery};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Sandbox,
    Docker,
}

pub struct AdaptiveContainerExecutor {
    sandbox: Arc<dyn AgentExecutor>,
    docker: Arc<dyn AgentExecutor>,
    sandbox_available: Arc<OnceLock<bool>>,
    active_backend: Arc<Mutex<HashMap<String, Backend>>>,
}

impl std::fmt::Debug for AdaptiveContainerExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdaptiveContainerExecutor")
            .field("sandbox_available", &self.sandbox_available.get().copied())
            .finish()
    }
}

impl AdaptiveContainerExecutor {
    pub fn new(store: SharedStore, config: &ExecutorConfig) -> Result<Self> {
        let sandbox: Arc<dyn AgentExecutor> =
            Arc::new(SandboxExecutor::new(store.clone(), config.agent_timeout));
        let docker: Arc<dyn AgentExecutor> = Arc::new(DockerExecutor::new(store, config)?);
        let sandbox_available = Arc::new(OnceLock::new());
        Self::spawn_probe_task(sandbox_available.clone());

        Ok(Self {
            sandbox,
            docker,
            sandbox_available,
            active_backend: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    #[cfg(test)]
    fn new_for_tests(
        sandbox: Arc<dyn AgentExecutor>,
        docker: Arc<dyn AgentExecutor>,
        sandbox_available: Option<bool>,
    ) -> Self {
        let availability = Arc::new(OnceLock::new());
        if let Some(available) = sandbox_available {
            let _ = availability.set(available);
        }
        Self {
            sandbox,
            docker,
            sandbox_available: availability,
            active_backend: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn spawn_probe_task(sandbox_available: Arc<OnceLock<bool>>) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            let _ = sandbox_available.set(false);
            warn!("sandbox probe skipped: no tokio runtime available");
            return;
        };

        handle.spawn(async move {
            let started = Instant::now();
            let probe_result = Self::probe_sandbox_support().await;
            let available = probe_result.is_ok();
            let _ = sandbox_available.set(available);

            let elapsed_ms = started.elapsed().as_millis() as u64;
            match probe_result {
                Ok(()) => info!(elapsed_ms, "sandbox probe passed; sandbox executor enabled"),
                Err(err) => {
                    warn!(
                        elapsed_ms,
                        error = %err,
                        "sandbox probe failed; using docker executor"
                    );
                }
            }
        });
    }

    async fn probe_sandbox_support() -> Result<()> {
        let probe_id = format!("lf-probe-{}", std::process::id());

        Self::run_probe_step("version", &["sandbox", "version"]).await?;

        if let Err(err) = Self::run_probe_step(
            "create",
            &["sandbox", "create", "--name", &probe_id, "claude", "/tmp"],
        )
        .await
        {
            let _ = Self::run_probe_step("cleanup", &["sandbox", "rm", &probe_id]).await;
            return Err(err);
        }

        let exec_result =
            Self::run_probe_step("exec", &["sandbox", "exec", &probe_id, "--", "true"]).await;
        let _ = Self::run_probe_step("cleanup", &["sandbox", "rm", &probe_id]).await;
        exec_result
    }

    async fn run_probe_step(step: &str, args: &[&str]) -> Result<()> {
        let output = Command::new("docker")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|err| anyhow!("sandbox probe {step} failed to start: {err}"))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("sandbox probe {step} exited with status {}", output.status)
        } else {
            format!("sandbox probe {step} failed: {stderr}")
        };
        Err(anyhow!(message))
    }

    fn should_try_sandbox(&self, cmd: &[String]) -> bool {
        self.sandbox_available.get() == Some(&true)
            && is_sandbox_harness(cmd.first().map(String::as_str))
    }

    async fn record_backend(&self, agent_id: &str, backend: Backend) {
        self.active_backend
            .lock()
            .await
            .insert(agent_id.to_string(), backend);
    }

    async fn clear_backend(&self, agent_id: &str) {
        self.active_backend.lock().await.remove(agent_id);
    }

    fn merge_recovery(a: StartupRecovery, b: StartupRecovery) -> StartupRecovery {
        StartupRecovery {
            orphaned_runs_failed: a.orphaned_runs_failed + b.orphaned_runs_failed,
            rehydrated_agents: a.rehydrated_agents + b.rehydrated_agents,
            lost_agents_failed: a.lost_agents_failed + b.lost_agents_failed,
            orphaned_containers_removed: a.orphaned_containers_removed
                + b.orphaned_containers_removed,
            orphaned_fork_runs_cleaned: a.orphaned_fork_runs_cleaned + b.orphaned_fork_runs_cleaned,
            orphaned_fork_worktrees_removed: a.orphaned_fork_worktrees_removed
                + b.orphaned_fork_worktrees_removed,
        }
    }
}

fn is_sandbox_harness(program: Option<&str>) -> bool {
    matches!(program, Some("claude") | Some("gemini"))
}

#[async_trait]
impl AgentExecutor for AdaptiveContainerExecutor {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, context: AgentRunContext<'_>) -> Result<i32> {
        if self.should_try_sandbox(&cmd) {
            self.record_backend(context.agent_id, Backend::Sandbox)
                .await;
            match self.sandbox.run(cmd.clone(), cwd, context).await {
                Ok(exit_code) => {
                    self.clear_backend(context.agent_id).await;
                    return Ok(exit_code);
                }
                Err(err) => {
                    warn!(
                        agent_id = context.agent_id,
                        error = %err,
                        "sandbox executor failed; retrying with docker"
                    );
                }
            }
        }

        self.record_backend(context.agent_id, Backend::Docker).await;
        let result = self.docker.run(cmd, cwd, context).await;
        self.clear_backend(context.agent_id).await;
        result
    }

    async fn terminate(&self, agent_id: &str) -> Result<()> {
        let backend = self.active_backend.lock().await.get(agent_id).copied();
        match backend {
            Some(Backend::Sandbox) => self.sandbox.terminate(agent_id).await,
            Some(Backend::Docker) => self.docker.terminate(agent_id).await,
            None => Ok(()),
        }
    }

    async fn recover_startup(&self, output: &OutputHub) -> Result<StartupRecovery> {
        let sandbox = self.sandbox.recover_startup(output).await?;
        let docker = self.docker.recover_startup(output).await?;
        Ok(Self::merge_recovery(sandbox, docker))
    }

    async fn ensure_wave_workspace(&self, wave: &Wave) -> Result<()> {
        self.sandbox.ensure_wave_workspace(wave).await
    }

    async fn cleanup_ephemeral_worktree(&self, repo: &Path, worktree: &Path) -> Result<()> {
        self.docker.cleanup_ephemeral_worktree(repo, worktree).await
    }

    async fn cleanup_wave_workspace(&self, wave: &Wave) -> Result<()> {
        self.docker.cleanup_wave_workspace(wave).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use crate::lfd::output::OutputHub;
    use crate::lfd::types::Wave;

    use super::*;

    #[derive(Debug, Clone)]
    struct MockExecutor {
        run_results: Arc<Mutex<VecDeque<Result<i32>>>>,
        run_calls: Arc<Mutex<Vec<Vec<String>>>>,
        terminate_calls: Arc<Mutex<Vec<String>>>,
        recovery: StartupRecovery,
    }

    impl MockExecutor {
        fn new(run_results: Vec<Result<i32>>, recovery: StartupRecovery) -> Self {
            Self {
                run_results: Arc::new(Mutex::new(run_results.into())),
                run_calls: Arc::new(Mutex::new(Vec::new())),
                terminate_calls: Arc::new(Mutex::new(Vec::new())),
                recovery,
            }
        }

        async fn run_count(&self) -> usize {
            self.run_calls.lock().await.len()
        }

        async fn terminate_count(&self) -> usize {
            self.terminate_calls.lock().await.len()
        }
    }

    #[async_trait]
    impl AgentExecutor for MockExecutor {
        async fn run(
            &self,
            cmd: Vec<String>,
            _cwd: &Path,
            _context: AgentRunContext<'_>,
        ) -> Result<i32> {
            self.run_calls.lock().await.push(cmd);
            self.run_results
                .lock()
                .await
                .pop_front()
                .unwrap_or_else(|| Ok(0))
        }

        async fn terminate(&self, agent_id: &str) -> Result<()> {
            self.terminate_calls.lock().await.push(agent_id.to_string());
            Ok(())
        }

        async fn recover_startup(&self, _output: &OutputHub) -> Result<StartupRecovery> {
            Ok(self.recovery)
        }

        async fn ensure_wave_workspace(&self, _wave: &Wave) -> Result<()> {
            Ok(())
        }
    }

    fn run_context<'a>(output: &'a OutputHub) -> AgentRunContext<'a> {
        AgentRunContext {
            wave_id: "wave-1",
            agent_id: "agent-1",
            wave_run_id: "run-1",
            branch: None,
            output,
            output_prefix: None,
        }
    }

    #[tokio::test]
    async fn routes_claude_to_sandbox_when_probe_passed() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output = OutputHub::new(16, output_dir.path().join("output"));
        let sandbox = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let docker = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let executor =
            AdaptiveContainerExecutor::new_for_tests(sandbox.clone(), docker.clone(), Some(true));

        let exit = executor
            .run(
                vec!["claude".to_string(), "--print".to_string()],
                output_dir.path(),
                run_context(&output),
            )
            .await
            .expect("run should succeed");

        assert_eq!(exit, 0);
        assert_eq!(sandbox.run_count().await, 1);
        assert_eq!(docker.run_count().await, 0);
    }

    #[tokio::test]
    async fn falls_back_to_docker_when_sandbox_fails() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output = OutputHub::new(16, output_dir.path().join("output"));
        let sandbox = Arc::new(MockExecutor::new(
            vec![Err(anyhow!("sandbox broke"))],
            StartupRecovery::default(),
        ));
        let docker = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let executor =
            AdaptiveContainerExecutor::new_for_tests(sandbox.clone(), docker.clone(), Some(true));

        let exit = executor
            .run(
                vec!["claude".to_string(), "--print".to_string()],
                output_dir.path(),
                run_context(&output),
            )
            .await
            .expect("docker fallback should succeed");

        assert_eq!(exit, 0);
        assert_eq!(sandbox.run_count().await, 1);
        assert_eq!(docker.run_count().await, 1);
    }

    #[tokio::test]
    async fn routes_non_sandbox_harness_to_docker() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output = OutputHub::new(16, output_dir.path().join("output"));
        let sandbox = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let docker = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let executor =
            AdaptiveContainerExecutor::new_for_tests(sandbox.clone(), docker.clone(), Some(true));

        let exit = executor
            .run(
                vec!["codex".to_string(), "run".to_string()],
                output_dir.path(),
                run_context(&output),
            )
            .await
            .expect("docker run should succeed");

        assert_eq!(exit, 0);
        assert_eq!(sandbox.run_count().await, 0);
        assert_eq!(docker.run_count().await, 1);
    }

    #[tokio::test]
    async fn routes_to_docker_while_probe_pending() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output = OutputHub::new(16, output_dir.path().join("output"));
        let sandbox = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let docker = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let executor =
            AdaptiveContainerExecutor::new_for_tests(sandbox.clone(), docker.clone(), None);

        let exit = executor
            .run(
                vec!["claude".to_string(), "--print".to_string()],
                output_dir.path(),
                run_context(&output),
            )
            .await
            .expect("docker run should succeed");

        assert_eq!(exit, 0);
        assert_eq!(sandbox.run_count().await, 0);
        assert_eq!(docker.run_count().await, 1);
    }

    #[tokio::test]
    async fn terminate_dispatches_using_active_backend() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output = OutputHub::new(16, output_dir.path().join("output"));
        let sandbox = Arc::new(MockExecutor::new(
            vec![Err(anyhow!("boom"))],
            StartupRecovery::default(),
        ));
        let docker = Arc::new(MockExecutor::new(vec![Ok(0)], StartupRecovery::default()));
        let executor =
            AdaptiveContainerExecutor::new_for_tests(sandbox.clone(), docker.clone(), Some(true));

        let run_future = executor.run(
            vec!["claude".to_string(), "--print".to_string()],
            output_dir.path(),
            run_context(&output),
        );
        let _ = tokio::time::timeout(Duration::from_millis(50), run_future)
            .await
            .expect("run should finish")
            .expect("fallback should succeed");

        executor.record_backend("agent-2", Backend::Sandbox).await;
        executor
            .terminate("agent-2")
            .await
            .expect("terminate should dispatch");

        assert_eq!(sandbox.terminate_count().await, 1);
        assert_eq!(docker.terminate_count().await, 0);
    }

    #[tokio::test]
    async fn startup_recovery_merges_backend_reports() {
        let output_dir = tempfile::tempdir().expect("tempdir");
        let output = OutputHub::new(16, output_dir.path().join("output"));
        let sandbox = Arc::new(MockExecutor::new(
            vec![Ok(0)],
            StartupRecovery {
                orphaned_runs_failed: 1,
                orphaned_containers_removed: 2,
                ..Default::default()
            },
        ));
        let docker = Arc::new(MockExecutor::new(
            vec![Ok(0)],
            StartupRecovery {
                orphaned_runs_failed: 3,
                rehydrated_agents: 4,
                ..Default::default()
            },
        ));
        let executor = AdaptiveContainerExecutor::new_for_tests(sandbox, docker, Some(true));

        let recovery = executor
            .recover_startup(&output)
            .await
            .expect("recovery should succeed");

        assert_eq!(recovery.orphaned_runs_failed, 4);
        assert_eq!(recovery.rehydrated_agents, 4);
        assert_eq!(recovery.orphaned_containers_removed, 2);
    }
}
