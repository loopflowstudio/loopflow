use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::engine::platform::kill_process;
use crate::lfd::executor::{read_stream, AgentExecutor, StartupRecovery};
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::AgentStatus;

pub struct LocalProcessExecutor {
    store: SharedStore,
    active: Arc<Mutex<HashMap<String, u32>>>,
}

impl std::fmt::Debug for LocalProcessExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalProcessExecutor").finish()
    }
}

impl LocalProcessExecutor {
    pub fn new(store: SharedStore) -> Self {
        Self {
            store,
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl AgentExecutor for LocalProcessExecutor {
    async fn run(
        &self,
        cmd: Vec<String>,
        cwd: &Path,
        wave_id: &str,
        agent_id: &str,
        wave_run_id: &str,
        output: &OutputHub,
    ) -> Result<i32> {
        if cmd.is_empty() {
            return Err(anyhow!("empty agent command"));
        }

        let agent_id_string = agent_id.to_string();
        let mut command = Command::new(&cmd[0]);
        command.args(&cmd[1..]);
        command.current_dir(cwd);
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let mut child = command.spawn()?;

        // Record the PID so the process can be killed on stop.
        if let Some(pid) = child.id() {
            let agent_lfd_id = LfdId::from_raw(agent_id);
            let _ = self.store.update_agent_status(
                &agent_lfd_id,
                AgentStatus::Running.as_i32(),
                Some(pid),
                None,
            );
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

        let stdout_task = tokio::spawn(read_stream(
            stdout,
            output.clone(),
            wave_id.to_string(),
            wave_run_id.to_string(),
            agent_id.to_string(),
        ));
        let stderr_task = tokio::spawn(read_stream(
            stderr,
            output.clone(),
            wave_id.to_string(),
            wave_run_id.to_string(),
            agent_id.to_string(),
        ));

        let status = child.wait().await?;
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
        let orphaned_runs_failed = self.store.fail_orphaned_runs()?;
        Ok(StartupRecovery {
            orphaned_runs_failed,
            ..Default::default()
        })
    }
}
