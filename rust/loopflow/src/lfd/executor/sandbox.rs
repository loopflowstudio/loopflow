use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::engine::platform::kill_process;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{AgentStatus, Wave};

use super::{read_stream, AgentExecutor, AgentRunContext, OutputContext, StartupRecovery};

/// Maps agent_id → exec PID (if spawned). Tracks active sandbox runs for
/// terminate dispatch and cleanup.
type ActiveRuns = Arc<Mutex<HashMap<String, Option<u32>>>>;

pub struct SandboxExecutor {
    store: SharedStore,
    active: ActiveRuns,
    agent_timeout: Duration,
}

impl std::fmt::Debug for SandboxExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxExecutor").finish()
    }
}

impl SandboxExecutor {
    pub fn new(store: SharedStore, agent_timeout: Duration) -> Self {
        Self {
            store,
            active: Arc::new(Mutex::new(HashMap::new())),
            agent_timeout,
        }
    }

    fn sandbox_id(agent_id: &str) -> String {
        format!("lf-{agent_id}")
    }

    async fn create_sandbox(sandbox_id: &str, cwd: &Path) -> Result<()> {
        let cwd_str = cwd.to_string_lossy();
        Self::sandbox_cmd(&["create", "--name", sandbox_id, "claude", &cwd_str]).await
    }

    async fn remove_sandbox(sandbox_id: &str) -> Result<()> {
        Self::sandbox_cmd(&["rm", sandbox_id]).await
    }

    async fn stop_sandbox(sandbox_id: &str) -> Result<()> {
        Self::sandbox_cmd(&["stop", sandbox_id]).await
    }

    /// Run a `docker sandbox <subcmd>` and return an error if it fails.
    async fn sandbox_cmd(args: &[&str]) -> Result<()> {
        let output = Self::run_docker(args).await?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            format!("status {}", output.status)
        } else {
            stderr
        };
        Err(anyhow!(
            "docker sandbox {} failed: {detail}",
            args.join(" ")
        ))
    }

    /// Run `docker sandbox <args>` and return the raw output.
    async fn run_docker(args: &[&str]) -> Result<std::process::Output> {
        let mut full_args = vec!["sandbox"];
        full_args.extend(args);
        Command::new("docker")
            .args(&full_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|err| anyhow!("failed to run `docker sandbox {}`: {err}", args.join(" ")))
    }

    async fn list_managed_sandboxes() -> Result<Vec<String>> {
        let output = Self::run_docker(&["ls", "--quiet"]).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow!("docker sandbox ls failed: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .map(str::trim)
            .filter(|name| !name.is_empty() && name.starts_with("lf-"))
            .map(ToOwned::to_owned)
            .collect())
    }

    async fn sandbox_exec_uses_legacy_separator() -> Result<bool> {
        let output = Self::run_docker(&["exec", "--help"]).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow!("docker sandbox exec --help failed: {stderr}"));
        }

        let help = String::from_utf8_lossy(&output.stdout);
        if help.contains("SANDBOX COMMAND [ARG...]") {
            return Ok(false);
        }
        if help.contains("SANDBOX -- COMMAND") {
            return Ok(true);
        }

        Err(anyhow!(
            "unsupported docker sandbox exec usage; expected direct or legacy syntax"
        ))
    }

    async fn collect_exec_env(&self, program: &str) -> Vec<(String, String)> {
        let mut env = BTreeMap::new();

        for env_name in crate::lfd::provider_auth::api_key_env_names() {
            if !crate::lfd::provider_auth::api_key_env_allowed_for_program(program, env_name) {
                continue;
            }
            let Ok(value) = std::env::var(env_name) else {
                continue;
            };
            if value.trim().is_empty() {
                continue;
            }
            env.insert(env_name.to_string(), value);
        }

        for (key, value) in crate::lfd::provider_auth::provider_env_vars(&self.store).await {
            if !crate::lfd::provider_auth::provider_env_allowed_for_program(program, &key) {
                continue;
            }
            env.insert(key, value);
        }

        env.into_iter().collect()
    }
}

#[async_trait]
impl AgentExecutor for SandboxExecutor {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, context: AgentRunContext<'_>) -> Result<i32> {
        if cmd.is_empty() {
            return Err(anyhow!("empty agent command"));
        }

        let sandbox_id = Self::sandbox_id(context.agent_id);
        Self::create_sandbox(&sandbox_id, cwd).await?;

        let agent_id = context.agent_id.to_string();
        self.active.lock().await.insert(agent_id.clone(), None);

        let output_context: OutputContext = context.into();
        let program = cmd[0].as_str();
        let env = self.collect_exec_env(program).await;
        let legacy_exec_separator = Self::sandbox_exec_uses_legacy_separator().await?;

        let mut command = Command::new("docker");
        command.arg("sandbox").arg("exec");
        for (key, value) in &env {
            command.arg("-e").arg(format!("{key}={value}"));
        }
        command.arg("-w").arg(cwd);
        command.arg(&sandbox_id);
        if legacy_exec_separator {
            command.arg("--");
        }
        command.args(&cmd);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let run_result: Result<i32> = async {
            let mut child = command
                .spawn()
                .map_err(|err| anyhow!("failed to launch sandbox exec: {err}"))?;

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
                if let Some(entry) = self.active.lock().await.get_mut(&agent_id) {
                    *entry = Some(pid);
                }
            }

            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("missing sandbox exec stdout"))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| anyhow!("missing sandbox exec stderr"))?;

            let stdout_task = tokio::spawn(read_stream(stdout, output_context.clone()));
            let stderr_task = tokio::spawn(read_stream(stderr, output_context));

            let status = match tokio::time::timeout(self.agent_timeout, child.wait()).await {
                Ok(result) => result.map_err(|err| anyhow!("sandbox exec failed: {err}"))?,
                Err(_) => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    let _ = Self::stop_sandbox(&sandbox_id).await;
                    return Err(anyhow!(
                        "agent execution timed out after {}",
                        humantime::format_duration(self.agent_timeout)
                    ));
                }
            };

            let _ = stdout_task.await;
            let _ = stderr_task.await;

            Ok(status.code().unwrap_or(1))
        }
        .await;

        self.active.lock().await.remove(&agent_id);
        if let Err(err) = Self::remove_sandbox(&sandbox_id).await {
            warn!(sandbox_id, error = %err, "failed to remove sandbox during cleanup");
        }

        run_result
    }

    async fn terminate(&self, agent_id: &str) -> Result<()> {
        let exec_pid = self.active.lock().await.remove(agent_id).flatten();
        if let Some(pid) = exec_pid {
            kill_process(pid);
        }
        let sandbox_id = Self::sandbox_id(agent_id);
        let _ = Self::stop_sandbox(&sandbox_id).await;
        let _ = Self::remove_sandbox(&sandbox_id).await;
        Ok(())
    }

    async fn recover_startup(&self, _output: &OutputHub) -> Result<StartupRecovery> {
        let mut removed = 0u32;
        match Self::list_managed_sandboxes().await {
            Ok(sandboxes) => {
                for sandbox_id in sandboxes {
                    match Self::remove_sandbox(&sandbox_id).await {
                        Ok(()) => {
                            removed += 1;
                        }
                        Err(err) => {
                            warn!(sandbox_id, error = %err, "failed to remove orphaned sandbox");
                        }
                    }
                }
            }
            Err(err) => {
                warn!(error = %err, "failed to list sandboxes during recovery");
            }
        }

        let orphaned_runs_failed = self.store.fail_orphaned_runs().await?;
        debug!(
            orphaned_runs_failed,
            removed, "sandbox startup recovery finished"
        );

        Ok(StartupRecovery {
            orphaned_runs_failed,
            orphaned_containers_removed: removed,
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
    use std::ffi::OsString;
    use std::sync::OnceLock;

    use tempfile::tempdir;
    use tokio::sync::Mutex;

    use crate::lfd::output::OutputHub;
    use crate::lfd::store::{open_store, StorageConfig};

    use super::*;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        vars: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn snapshot(vars: &[&'static str]) -> Self {
            Self {
                vars: vars
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.vars {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }

    fn install_fake_docker(log_path: &Path) -> tempfile::TempDir {
        let dir = tempdir().expect("tempdir");
        let script = dir.path().join("docker");
        let content = r#"#!/bin/sh
set -eu
echo "$@" >> "${FAKE_DOCKER_LOG:?missing log path}"
if [ "${1:-}" != "sandbox" ]; then
  echo "expected sandbox command" >&2
  exit 1
fi
shift
subcmd="${1:-}"
shift || true
case "$subcmd" in
  create)
    exit 0
    ;;
  exec)
    if [ "${1:-}" = "--help" ]; then
      echo "Usage: docker sandbox exec [OPTIONS] SANDBOX COMMAND [ARG...]"
      exit 0
    fi
    sandbox_seen=0
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -e|-w)
          shift 2
          ;;
        --)
          shift
          break
          ;;
        *)
          if [ "$sandbox_seen" -eq 0 ]; then
            sandbox_seen=1
            shift
          else
            break
          fi
          ;;
      esac
    done
    "$@"
    ;;
  stop|rm|version)
    exit 0
    ;;
  ls)
    echo "lf-managed"
    echo "manual-sandbox"
    ;;
  *)
    echo "unknown sandbox subcommand: $subcmd" >&2
    exit 1
    ;;
esac
"#;
        std::fs::write(&script, content).expect("write fake docker script");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&script).expect("metadata").permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions).expect("chmod script");
        }

        let old_path = std::env::var_os("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", dir.path().display(), old_path.to_string_lossy());
        std::env::set_var("PATH", new_path);
        std::env::set_var("FAKE_DOCKER_LOG", log_path);

        dir
    }

    fn run_context<'a>(
        output: &'a OutputHub,
        agent_id: &'a str,
        run_id: &'a str,
    ) -> AgentRunContext<'a> {
        AgentRunContext {
            wave_id: "wave-1",
            agent_id,
            wave_run_id: run_id,
            branch: None,
            output,
            output_prefix: None,
        }
    }

    #[tokio::test]
    async fn sandbox_executor_runs_agent_and_streams_output() {
        let _lock = env_lock().lock().await;
        let _guard = EnvGuard::snapshot(&["PATH", "FAKE_DOCKER_LOG"]);

        let tmp = tempdir().expect("tempdir");
        let log_path = tmp.path().join("docker.log");
        let _fake_docker = install_fake_docker(&log_path);

        let store = Arc::new(
            open_store(&StorageConfig::sqlite(tmp.path().join("test.db")))
                .await
                .expect("sqlite store"),
        );
        let executor = SandboxExecutor::new(store, Duration::from_secs(30));
        let output = OutputHub::new(16, tmp.path().join("output"));

        let exit = executor
            .run(
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "printf 'sandbox-ok\\n'".to_string(),
                ],
                tmp.path(),
                run_context(&output, "agent-1", "run-1"),
            )
            .await
            .expect("sandbox run should succeed");

        assert_eq!(exit, 0);
        let lines = output.read_log("run-1").expect("run log exists").0;
        assert_eq!(lines, vec!["sandbox-ok"]);

        let docker_log = std::fs::read_to_string(log_path).expect("docker log exists");
        assert!(docker_log.contains("sandbox create --name lf-agent-1"));
        assert!(docker_log.contains("sandbox exec"));
        assert!(docker_log.contains("sandbox rm lf-agent-1"));
    }

    #[tokio::test]
    async fn sandbox_executor_timeout_stops_and_removes_sandbox() {
        let _lock = env_lock().lock().await;
        let _guard = EnvGuard::snapshot(&["PATH", "FAKE_DOCKER_LOG"]);

        let tmp = tempdir().expect("tempdir");
        let log_path = tmp.path().join("docker.log");
        let _fake_docker = install_fake_docker(&log_path);

        let store = Arc::new(
            open_store(&StorageConfig::sqlite(tmp.path().join("test.db")))
                .await
                .expect("sqlite store"),
        );
        let executor = SandboxExecutor::new(store, Duration::from_millis(30));
        let output = OutputHub::new(16, tmp.path().join("output"));

        let result = executor
            .run(
                vec!["sh".to_string(), "-c".to_string(), "sleep 1".to_string()],
                tmp.path(),
                run_context(&output, "agent-timeout", "run-timeout"),
            )
            .await;

        assert!(result.is_err());
        assert!(result
            .expect_err("timeout should error")
            .to_string()
            .contains("timed out"));

        let docker_log = std::fs::read_to_string(log_path).expect("docker log exists");
        assert!(docker_log.contains("sandbox stop lf-agent-timeout"));
        assert!(docker_log.contains("sandbox rm lf-agent-timeout"));
    }

    #[tokio::test]
    async fn sandbox_recovery_removes_only_managed_sandboxes() {
        let _lock = env_lock().lock().await;
        let _guard = EnvGuard::snapshot(&["PATH", "FAKE_DOCKER_LOG"]);

        let tmp = tempdir().expect("tempdir");
        let log_path = tmp.path().join("docker.log");
        let _fake_docker = install_fake_docker(&log_path);

        let store = Arc::new(
            open_store(&StorageConfig::sqlite(tmp.path().join("test.db")))
                .await
                .expect("sqlite store"),
        );
        let executor = SandboxExecutor::new(store, Duration::from_secs(30));
        let output = OutputHub::new(16, tmp.path().join("output"));

        let recovery = executor
            .recover_startup(&output)
            .await
            .expect("recovery should succeed");

        assert_eq!(recovery.orphaned_containers_removed, 1);
        let docker_log = std::fs::read_to_string(log_path).expect("docker log exists");
        assert!(docker_log.contains("sandbox rm lf-managed"));
        assert!(!docker_log.contains("sandbox rm manual-sandbox"));
    }
}
