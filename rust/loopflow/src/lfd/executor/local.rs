use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::engine::platform::kill_process;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{AgentStatus, Wave};

use super::{read_stream, AgentExecutor, AgentRunContext, OutputContext, StartupRecovery};

pub struct LocalProcessExecutor {
    store: SharedStore,
    active: Arc<Mutex<HashMap<String, u32>>>,
    agent_timeout: Duration,
}

impl std::fmt::Debug for LocalProcessExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalProcessExecutor").finish()
    }
}

impl LocalProcessExecutor {
    pub fn new(store: SharedStore, agent_timeout: Duration) -> Self {
        Self {
            store,
            active: Arc::new(Mutex::new(HashMap::new())),
            agent_timeout,
        }
    }
}

#[async_trait]
impl AgentExecutor for LocalProcessExecutor {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, context: AgentRunContext<'_>) -> Result<i32> {
        if cmd.is_empty() {
            return Err(anyhow!("empty agent command"));
        }

        let agent_id_string = context.agent_id.to_string();
        let output_context: OutputContext = context.into();
        let mut command = Command::new(&cmd[0]);
        command.args(&cmd[1..]);
        command.current_dir(cwd);
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let mut child = command.spawn()?;

        // Record the PID so the process can be killed on stop.
        if let Some(pid) = child.id() {
            let agent_lfd_id = LfdId::from_raw(context.agent_id);
            let _ = self
                .store
                .update_agent_status(
                    &agent_lfd_id,
                    AgentStatus::Running.as_i32(),
                    Some(pid),
                    None,
                )
                .await;
            self.active
                .lock()
                .await
                .insert(agent_id_string.clone(), pid);
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing stderr"))?;

        let stdout_task = tokio::spawn(read_stream(stdout, output_context.clone()));
        let stderr_task = tokio::spawn(read_stream(stderr, output_context));

        let status = match tokio::time::timeout(self.agent_timeout, child.wait()).await {
            Ok(result) => result?,
            Err(_) => {
                let _ = child.start_kill();
                let _ = child.wait().await;
                let _ = stdout_task.await;
                let _ = stderr_task.await;
                self.active.lock().await.remove(&agent_id_string);
                return Err(anyhow!(
                    "agent execution timed out after {}",
                    humantime::format_duration(self.agent_timeout)
                ));
            }
        };
        let _ = stdout_task.await;
        let _ = stderr_task.await;
        self.active.lock().await.remove(&agent_id_string);

        let exit_code = status.code().unwrap_or(1);
        Ok(exit_code)
    }

    async fn terminate(&self, agent_id: &str) -> Result<()> {
        if let Some(pid) = self.active.lock().await.remove(agent_id) {
            kill_process(pid);
        }
        Ok(())
    }

    async fn recover_startup(&self, _output: &OutputHub) -> Result<StartupRecovery> {
        let orphaned_runs_failed = self.store.fail_orphaned_runs().await?;
        Ok(StartupRecovery {
            orphaned_runs_failed,
            ..Default::default()
        })
    }

    async fn ensure_wave_workspace(&self, wave: &Wave) -> Result<()> {
        let repo = wave.repo().clone();
        let wave_name = wave.name().clone();
        tokio::task::spawn_blocking(move || {
            super::ensure_wave_worktree(Path::new(&repo), &wave_name).map(|_| ())
        })
        .await
        .map_err(|err| anyhow!("failed preparing wave workspace: {err}"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::output::OutputHub;
    use crate::lfd::store::{open_store, StorageConfig};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn local_executor_times_out_long_running_agent() {
        let tmp = tempdir().expect("tempdir");
        let db = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db))
                .await
                .expect("sqlite store"),
        );
        let executor = LocalProcessExecutor::new(store, Duration::from_millis(50));
        let output = OutputHub::new(16, tmp.path().join("output"));

        let result = executor
            .run(
                vec!["sh".to_string(), "-c".to_string(), "sleep 1".to_string()],
                tmp.path(),
                AgentRunContext {
                    wave_id: "wave-timeout",
                    agent_id: "agent-timeout",
                    wave_run_id: "run-timeout",
                    branch: None,
                    output: &output,
                    output_prefix: None,
                },
            )
            .await;

        assert!(result.is_err());
        assert!(result
            .expect_err("timeout should fail")
            .to_string()
            .contains("timed out"));
    }
}
