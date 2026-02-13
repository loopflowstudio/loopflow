use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bollard::container::{
    Config as DockerContainerConfig, CreateContainerOptions, InspectContainerOptions,
    ListContainersOptions, LogOutput, LogsOptions, RemoveContainerOptions, StartContainerOptions,
    StopContainerOptions, WaitContainerOptions,
};
use bollard::errors::Error as DockerError;
use bollard::models::{ContainerInspectResponse, HostConfig, Mount, MountTypeEnum};
use bollard::volume::CreateVolumeOptions;
use bollard::Docker;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use tracing::{debug, error, info, warn};

use crate::engine::agent::{build_agent_command, LaunchConfig};
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::config::{load_config, load_config_or_default};
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteFork, ConcreteItem, ConcreteStep, FlowAction,
    ForkSelect, Step,
};
use crate::engine::git::{create_branch, current_branch, hash_areas, push_with_upstream};
use crate::engine::naming::{format_branch_name, generate_word_pair};
use crate::engine::platform::kill_process;
use crate::engine::prompt::{
    drop_native_instruction_docs, format_context_prompt, format_prompt, format_task_prompt,
    gather_context, trim_context_with_breakdown, write_prompt_log, Document, GatherContextOpts,
    DEFAULT_CONTEXT_BUDGET,
};
use crate::engine::stream::{render_event, ParseResult, StreamParser};
use crate::engine::worktree::{create_worktree, remove_worktree};
use crate::engine::worktrees::{
    branch_exists, create_with_schema, schedule_upstream_sync, worktree_path as wave_worktree_path,
};

use time::OffsetDateTime;

use crate::lfd::config::{ExecutorConfig, ExecutorType};
use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
use crate::lfd::output::{OutputEvent, OutputHub};
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::{ForkRun, ForkRunStatus, SharedStore};
use crate::lfd::types::{
    Agent, AgentStatus, Event, StimulusKind, Summary, Wave, WaveRun, WaveRunSnapshot,
    WaveRunStatus, WaveStatus,
};

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn run(
        &self,
        cmd: Vec<String>,
        cwd: &Path,
        wave_id: &str,
        agent_id: &str,
        wave_run_id: &str,
        output: &OutputHub,
    ) -> Result<i32>;
    async fn terminate(&self, agent_id: &str) -> Result<()>;
    async fn recover_startup(&self, _output: &OutputHub) -> Result<StartupRecovery> {
        Ok(StartupRecovery::default())
    }
    async fn cleanup_wave(&self, _wave: &Wave) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartupRecovery {
    pub orphaned_runs_failed: u32,
    pub rehydrated_agents: u32,
    pub lost_agents_failed: u32,
    pub orphaned_containers_removed: u32,
}

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

#[derive(Clone)]
pub struct DockerExecutor {
    store: SharedStore,
    docker: Docker,
    image: String,
    credential_env: Vec<String>,
    credential_mounts: Vec<DockerCredentialMount>,
    active: Arc<Mutex<HashMap<String, String>>>,
    mutation_locks: RepoMutationLocks,
    prepared_runs: Arc<Mutex<HashSet<String>>>,
}

impl std::fmt::Debug for DockerExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerExecutor")
            .field("image", &self.image)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
struct RepoMutationLocks {
    inner: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl RepoMutationLocks {
    async fn for_key(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.inner.lock().await;
        locks
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[derive(Debug, Clone)]
struct DockerCredentialMount {
    host_path: PathBuf,
    container_path: String,
}

impl DockerCredentialMount {
    fn from_spec(spec: &str) -> std::result::Result<Self, String> {
        let mut parts = spec.splitn(2, ':');
        let host_path = parts
            .next()
            .ok_or_else(|| "missing host path".to_string())?
            .trim();
        let container_path = parts
            .next()
            .ok_or_else(|| "missing container path".to_string())?
            .trim();
        if host_path.is_empty() || container_path.is_empty() {
            return Err("mount paths must be non-empty".to_string());
        }
        if !container_path.starts_with('/') {
            return Err("container path must be absolute".to_string());
        }

        let host_path = Self::expand_host_path(host_path);
        if !host_path.is_absolute() {
            return Err("host path must be absolute or use ~/...".to_string());
        }

        Ok(Self {
            host_path,
            container_path: container_path.to_string(),
        })
    }

    fn expand_host_path(path: &str) -> PathBuf {
        if path == "~" {
            if let Some(home) = dirs::home_dir() {
                return home;
            }
        }
        if let Some(rest) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(rest);
            }
        }
        PathBuf::from(path)
    }
}

const CONTAINER_WORKSPACE: &str = "/workspace";
const CONTAINER_REPOS_ROOT: &str = "/workspace/repos";
const LOCAL_REPO_MOUNT: &str = "/host-repo";
const HOST_WORKTREE_MOUNT: &str = "/host-worktree";

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoVolumeIdentity {
    repo_key: String,
    volume_name: String,
}

impl RepoVolumeIdentity {
    fn from_identity(identity: &RepoIdentity) -> Self {
        let repo_hash = short_hash(&identity.canonical, 16);
        let mut slug = sanitize_token(&identity.canonical);
        if slug.is_empty() {
            slug = "repo".to_string();
        }
        if slug.len() > 36 {
            slug.truncate(36);
        }
        Self {
            repo_key: format!("{slug}-{repo_hash}"),
            volume_name: format!("lfd-repo-{}", short_hash(&identity.canonical, 32)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoIdentity {
    canonical: String,
    has_remote: bool,
}

impl RepoIdentity {
    fn from_repo(repo: &Path) -> Self {
        if let Some(remote) = canonical_repo_url(repo) {
            return Self {
                canonical: remote,
                has_remote: true,
            };
        }

        let absolute = repo
            .canonicalize()
            .unwrap_or_else(|_| repo.to_path_buf())
            .to_string_lossy()
            .to_string();
        Self {
            canonical: format!("local:{}", short_hash(&absolute, 32)),
            has_remote: false,
        }
    }
}

#[derive(Debug, Clone)]
struct DockerWorkspace {
    volume: RepoVolumeIdentity,
    repo_source: PathBuf,
    container_shared_clone: String,
    container_worktree: String,
    branch: String,
    has_remote: bool,
}

#[derive(Debug, Clone)]
struct ReattachTarget {
    agent: Agent,
    wave_id: LfdId,
    wave_run_id: LfdId,
    container_id: String,
}

#[derive(Debug, Clone)]
struct RehydrationPlan {
    reattach: Vec<ReattachTarget>,
    lost: Vec<Agent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectedContainer {
    id: String,
    running: bool,
}

#[async_trait]
trait DockerRecoveryBackend: Send + Sync {
    async fn inspect_container(&self, container_ref: &str) -> Result<Option<InspectedContainer>>;
    async fn list_managed_containers(&self) -> Result<Vec<String>>;
    async fn stop_container(&self, container_id: &str) -> Result<()>;
    async fn remove_container(&self, container_id: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
struct BollardRecoveryBackend {
    docker: Docker,
}

impl BollardRecoveryBackend {
    fn new(docker: Docker) -> Self {
        Self { docker }
    }
}

#[async_trait]
impl DockerRecoveryBackend for BollardRecoveryBackend {
    async fn inspect_container(&self, container_ref: &str) -> Result<Option<InspectedContainer>> {
        match self
            .docker
            .inspect_container(container_ref, None::<InspectContainerOptions>)
            .await
        {
            Ok(details) => Ok(inspected_container(details)),
            Err(err) if is_container_not_found(&err) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    async fn list_managed_containers(&self) -> Result<Vec<String>> {
        let mut filters = HashMap::new();
        filters.insert(
            "label".to_string(),
            vec!["io.loopflow.managed=true".to_string()],
        );

        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                filters,
                ..Default::default()
            }))
            .await?;
        Ok(containers
            .into_iter()
            .filter_map(|container| container.id)
            .collect())
    }

    async fn stop_container(&self, container_id: &str) -> Result<()> {
        match self
            .docker
            .stop_container(container_id, Some(StopContainerOptions { t: 1 }))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) if is_container_not_found(&err) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    async fn remove_container(&self, container_id: &str) -> Result<()> {
        let options = RemoveContainerOptions {
            force: true,
            v: true,
            link: false,
        };
        match self
            .docker
            .remove_container(container_id, Some(options))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) if is_container_not_found(&err) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

fn short_hash(value: &str, chars: usize) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hash = hex::encode(digest);
    hash.truncate(chars);
    hash
}

fn sanitize_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn normalize_repo_url(raw: &str) -> String {
    let mut value = raw.trim().trim_end_matches('/').to_string();
    if let Some(stripped) = value.strip_suffix(".git") {
        value = stripped.to_string();
    }
    if value.starts_with("git@") {
        if let Some((host, path)) = value.split_once(':') {
            value = format!("ssh://{host}/{path}");
        }
    }
    if let Some((scheme, rest)) = value.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if let Some((host, tail)) = rest.split_once('/') {
            let host = host.to_ascii_lowercase();
            value = format!("{scheme}://{host}/{}", tail.trim_start_matches('/'));
        } else {
            value = format!("{scheme}://{}", rest.to_ascii_lowercase());
        }
    }
    value
}

fn canonical_repo_url(repo: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    Some(normalize_repo_url(&raw))
}

impl DockerExecutor {
    pub fn new(store: SharedStore, config: &ExecutorConfig) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        let credential_mounts = config
            .credentials
            .mounts
            .iter()
            .filter_map(|spec| match DockerCredentialMount::from_spec(spec) {
                Ok(mount) => Some(mount),
                Err(err) => {
                    warn!(
                        mount = %spec,
                        error = %err,
                        "invalid docker credential mount; expected host:container"
                    );
                    None
                }
            })
            .collect();

        Ok(Self {
            store,
            docker,
            image: config.image.clone(),
            credential_env: config.credentials.env.clone(),
            credential_mounts,
            active: Arc::new(Mutex::new(HashMap::new())),
            mutation_locks: RepoMutationLocks::default(),
            prepared_runs: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    fn build_container_name(agent_id: &str) -> String {
        format!("lfd-agent-{}", agent_id.replace('_', "-"))
    }

    fn build_helper_container_name(label: &str) -> String {
        format!(
            "lfd-prep-{}-{}",
            sanitize_token(label),
            uuid::Uuid::new_v4().simple()
        )
    }

    fn build_agent_labels(
        agent_id: &str,
        wave_id: &str,
        wave_run_id: &str,
    ) -> HashMap<String, String> {
        HashMap::from([
            ("io.loopflow.managed".to_string(), "true".to_string()),
            ("io.loopflow.agent-id".to_string(), agent_id.to_string()),
            ("io.loopflow.wave-id".to_string(), wave_id.to_string()),
            (
                "io.loopflow.wave-run-id".to_string(),
                wave_run_id.to_string(),
            ),
        ])
    }

    fn rewrite_command_paths(
        cmd: Vec<String>,
        host_root: &Path,
        container_root: &str,
    ) -> Vec<String> {
        let host_root = host_root.to_string_lossy();
        let host_root = if host_root == "/" {
            "/".to_string()
        } else {
            host_root.trim_end_matches('/').to_string()
        };
        cmd.into_iter()
            .map(|arg| Self::rewrite_command_arg_path(&arg, &host_root, container_root))
            .collect()
    }

    async fn active_container_ids(&self) -> HashSet<String> {
        self.active
            .lock()
            .await
            .values()
            .cloned()
            .collect::<HashSet<_>>()
    }

    async fn find_running_container(
        &self,
        backend: &dyn DockerRecoveryBackend,
        agent: &Agent,
    ) -> Result<Option<String>> {
        let container_name = Self::build_container_name(agent.id.as_str());
        let persisted_ref = agent
            .container_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());
        for container_ref in persisted_ref
            .into_iter()
            .chain(std::iter::once(container_name.as_str()))
        {
            match backend.inspect_container(container_ref).await {
                Ok(Some(container)) if container.running => return Ok(Some(container.id)),
                Ok(Some(_)) => {}
                Ok(None) => {}
                Err(err) => {
                    warn!(
                        agent_id = %agent.id,
                        container_ref,
                        error = %err,
                        "failed inspecting container during startup recovery"
                    );
                }
            }
        }

        Ok(None)
    }

    async fn plan_rehydration(
        &self,
        backend: &dyn DockerRecoveryBackend,
    ) -> Result<RehydrationPlan> {
        let agents = self.store.list_agents()?;
        let mut plan = RehydrationPlan {
            reattach: Vec::new(),
            lost: Vec::new(),
        };

        for agent in agents
            .into_iter()
            .filter(|agent| agent.status == AgentStatus::Running && agent.ended_at.is_none())
        {
            let Some(wave_run_id) = agent.wave_run_id.clone() else {
                plan.lost.push(agent);
                continue;
            };

            let Some(run) = self.store.get_wave_run(&wave_run_id)? else {
                plan.lost.push(agent);
                continue;
            };

            match self.find_running_container(backend, &agent).await? {
                Some(container_id) => plan.reattach.push(ReattachTarget {
                    agent,
                    wave_id: run.wave_id.clone(),
                    wave_run_id,
                    container_id,
                }),
                None => plan.lost.push(agent),
            }
        }

        Ok(plan)
    }

    fn spawn_reattach_task(
        &self,
        output: OutputHub,
        target: ReattachTarget,
    ) -> tokio::task::JoinHandle<()> {
        let executor = self.clone();
        tokio::spawn(async move {
            let result = executor
                .reattach_agent(&output, &target.agent, &target.wave_id, &target.wave_run_id)
                .await;
            if let Err(err) = executor
                .finalize_reattached_agent(
                    &target.agent,
                    &target.wave_id,
                    &target.wave_run_id,
                    result,
                )
                .await
            {
                warn!(
                    agent_id = %target.agent.id,
                    wave_run_id = %target.wave_run_id,
                    error = %err,
                    "failed finalizing reattached container"
                );
            }
        })
    }

    async fn reattach_agent(
        &self,
        output: &OutputHub,
        agent: &Agent,
        wave_id: &LfdId,
        wave_run_id: &LfdId,
    ) -> Result<i32> {
        let container_id = self
            .active
            .lock()
            .await
            .get(agent.id.as_str())
            .cloned()
            .ok_or_else(|| anyhow!("active container missing for reattach"))?;

        let workspace = self.resolve_workspace(wave_id.as_str(), wave_run_id.as_str())?;
        let exit_code = self
            .wait_for_container_with_logs(
                &container_id,
                output,
                wave_id.as_str(),
                wave_run_id.as_str(),
                agent.id.as_str(),
            )
            .await;

        self.active.lock().await.remove(agent.id.as_str());

        let sync_result = self
            .sync_to_host_worktree(&workspace, Path::new(&agent.worktree))
            .await;
        self.remove_container(&container_id).await;
        sync_result?;

        exit_code
    }

    async fn finalize_reattached_agent(
        &self,
        agent: &Agent,
        wave_id: &LfdId,
        wave_run_id: &LfdId,
        result: Result<i32>,
    ) -> Result<()> {
        let ended_at = OffsetDateTime::now_utc().unix_timestamp();
        let (agent_status, run_status, run_error) = match result {
            Ok(0) => (AgentStatus::Completed, WaveRunStatus::Completed, None),
            Ok(code) => (
                AgentStatus::Failed,
                WaveRunStatus::Failed,
                Some(format!("reattached agent exited with code {code}")),
            ),
            Err(err) => (
                AgentStatus::Failed,
                WaveRunStatus::Failed,
                Some(format!("reattached agent failed: {err}")),
            ),
        };

        let _ = self
            .store
            .end_agent(&agent.id, agent_status.as_i32(), ended_at);

        let mut next_wave_status = None;
        if let Some(mut run) = self.store.get_wave_run(wave_run_id)? {
            if !matches!(run.status, WaveRunStatus::Completed | WaveRunStatus::Failed) {
                run.status = run_status;
                run.ended_at = Some(OffsetDateTime::now_utc());
                run.error = run_error;
                self.store.update_wave_run(&run)?;
                next_wave_status = Some(if run_status == WaveRunStatus::Completed {
                    WaveStatus::Idle
                } else {
                    WaveStatus::Failed
                });
            }
        }

        if let Some(wave_status) = next_wave_status {
            if let Some(mut wave) = self.store.get_wave(wave_id)? {
                wave.status = wave_status;
                let _ = self.store.update_wave(&wave);
            }
        }

        Ok(())
    }

    async fn mark_agent_lost(&self, agent: &Agent) -> Result<()> {
        let ended_at = OffsetDateTime::now_utc().unix_timestamp();
        let _ = self
            .store
            .end_agent(&agent.id, AgentStatus::Failed.as_i32(), ended_at);

        if let Some(wave_run_id) = &agent.wave_run_id {
            if let Some(mut run) = self.store.get_wave_run(wave_run_id)? {
                if !matches!(run.status, WaveRunStatus::Completed | WaveRunStatus::Failed) {
                    run.status = WaveRunStatus::Failed;
                    run.error = Some("container lost during lfd restart.".to_string());
                    run.ended_at = Some(OffsetDateTime::now_utc());
                    self.store.update_wave_run(&run)?;
                }

                if let Some(mut wave) = self.store.get_wave(&run.wave_id)? {
                    wave.status = WaveStatus::Failed;
                    let _ = self.store.update_wave(&wave);
                }
            }
        }

        Ok(())
    }

    async fn cleanup_orphaned_containers(
        &self,
        backend: &dyn DockerRecoveryBackend,
    ) -> Result<u32> {
        let active_ids = self.active_container_ids().await;
        let mut removed = 0u32;
        let containers = backend.list_managed_containers().await?;
        for container_id in containers {
            if active_ids.contains(&container_id) {
                continue;
            }
            if let Err(err) = backend.stop_container(&container_id).await {
                warn!(
                    container_id,
                    error = %err,
                    "failed stopping orphaned managed container"
                );
            }
            if let Err(err) = backend.remove_container(&container_id).await {
                warn!(
                    container_id,
                    error = %err,
                    "failed removing orphaned managed container"
                );
                continue;
            }
            info!(container_id, "removed orphaned managed container");
            removed += 1;
        }
        Ok(removed)
    }

    async fn recover_startup_with_backend(
        &self,
        backend: &dyn DockerRecoveryBackend,
        output: &OutputHub,
        spawn_reattach: bool,
    ) -> Result<StartupRecovery> {
        let plan = self.plan_rehydration(backend).await?;

        for lost in &plan.lost {
            if let Err(err) = self.mark_agent_lost(lost).await {
                warn!(
                    agent_id = %lost.id,
                    error = %err,
                    "failed marking lost container state"
                );
            }
        }

        for target in plan.reattach.iter().cloned() {
            self.active
                .lock()
                .await
                .insert(target.agent.id.to_string(), target.container_id.clone());
            if spawn_reattach {
                std::mem::drop(self.spawn_reattach_task(output.clone(), target));
            }
        }

        let orphaned_containers_removed = self.cleanup_orphaned_containers(backend).await?;
        Ok(StartupRecovery {
            orphaned_runs_failed: 0,
            rehydrated_agents: plan.reattach.len() as u32,
            lost_agents_failed: plan.lost.len() as u32,
            orphaned_containers_removed,
        })
    }

    fn rewrite_command_arg_path(arg: &str, host_root: &str, container_root: &str) -> String {
        if arg == host_root {
            return container_root.to_string();
        }

        match arg.strip_prefix(host_root) {
            Some(suffix) if suffix.starts_with('/') => format!("{container_root}{suffix}"),
            _ => arg.to_string(),
        }
    }

    fn collect_env(&self) -> Vec<String> {
        self.credential_env
            .iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .map(|value| format!("{name}={value}"))
            })
            .collect()
    }

    fn build_mounts_for(
        volume_name: &str,
        credential_mounts: &[DockerCredentialMount],
    ) -> Vec<Mount> {
        let mut mounts = vec![Mount {
            target: Some(CONTAINER_WORKSPACE.to_string()),
            source: Some(volume_name.to_string()),
            typ: Some(MountTypeEnum::VOLUME),
            read_only: Some(false),
            ..Default::default()
        }];

        for credential_mount in credential_mounts {
            mounts.push(Mount {
                target: Some(credential_mount.container_path.clone()),
                source: Some(credential_mount.host_path.to_string_lossy().to_string()),
                typ: Some(MountTypeEnum::BIND),
                read_only: Some(true),
                ..Default::default()
            });
        }

        mounts
    }

    fn build_mounts(&self, volume_name: &str) -> Vec<Mount> {
        Self::build_mounts_for(volume_name, &self.credential_mounts)
    }

    fn bind_mount(path: &Path, target: &str, read_only: bool) -> Mount {
        Mount {
            target: Some(target.to_string()),
            source: Some(path.to_string_lossy().to_string()),
            typ: Some(MountTypeEnum::BIND),
            read_only: Some(read_only),
            ..Default::default()
        }
    }

    fn helper_mounts(&self, workspace: &DockerWorkspace, extra: Vec<Mount>) -> Vec<Mount> {
        let mut mounts = self.build_mounts(&workspace.volume.volume_name);
        mounts.extend(extra);
        mounts
    }

    async fn remove_container(&self, container_id: &str) {
        let options = RemoveContainerOptions {
            force: true,
            v: true,
            link: false,
        };
        if let Err(err) = self
            .docker
            .remove_container(container_id, Some(options))
            .await
        {
            warn!(container_id, error = %err, "failed to remove container");
        }
    }

    async fn wait_for_container_with_logs(
        &self,
        container_id: &str,
        output: &OutputHub,
        wave_id: &str,
        wave_run_id: &str,
        agent_id: &str,
    ) -> Result<i32> {
        let logs_task = tokio::spawn(Self::stream_logs(
            self.docker.clone(),
            container_id.to_string(),
            output.clone(),
            wave_id.to_string(),
            wave_run_id.to_string(),
            agent_id.to_string(),
        ));

        let mut wait_stream = self
            .docker
            .wait_container(container_id, None::<WaitContainerOptions<String>>);
        let wait_result = wait_stream.next().await;

        let _ = logs_task.await;
        let status =
            wait_result.ok_or_else(|| anyhow!("docker wait stream ended without status"))??;
        Ok(status.status_code as i32)
    }

    async fn stream_logs(
        docker: Docker,
        container_id: String,
        output: OutputHub,
        wave_id: String,
        wave_run_id: String,
        agent_id: String,
    ) {
        let mut logs = docker.logs(
            &container_id,
            Some(LogsOptions::<String> {
                follow: true,
                stdout: true,
                stderr: true,
                timestamps: false,
                tail: "all".to_string(),
                ..Default::default()
            }),
        );

        let mut parser = StreamParser::new();
        let mut pending = String::new();
        while let Some(entry) = logs.next().await {
            match entry {
                Ok(LogOutput::StdOut { message })
                | Ok(LogOutput::StdErr { message })
                | Ok(LogOutput::Console { message }) => {
                    pending.push_str(&String::from_utf8_lossy(&message));
                    while let Some(newline) = pending.find('\n') {
                        let mut line = pending.drain(..=newline).collect::<String>();
                        if line.ends_with('\n') {
                            line.pop();
                        }
                        if line.ends_with('\r') {
                            line.pop();
                        }
                        handle_output_line(
                            &line,
                            &mut parser,
                            &output,
                            &wave_id,
                            &wave_run_id,
                            &agent_id,
                        );
                    }
                }
                Err(err) => {
                    warn!(container_id, error = %err, "failed streaming container logs");
                    break;
                }
                _ => {}
            }
        }

        if !pending.is_empty() {
            handle_output_line(
                &pending,
                &mut parser,
                &output,
                &wave_id,
                &wave_run_id,
                &agent_id,
            );
        }
    }

    async fn run_helper_command(
        &self,
        label: &str,
        cmd: Vec<String>,
        mounts: Vec<Mount>,
        working_dir: Option<String>,
    ) -> Result<String> {
        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Self::build_helper_container_name(label),
                    platform: None,
                }),
                DockerContainerConfig {
                    image: Some(self.image.clone()),
                    cmd: Some(cmd),
                    working_dir,
                    env: Some(self.collect_env()),
                    user: Some("root".to_string()),
                    host_config: Some(HostConfig {
                        mounts: Some(mounts),
                        network_mode: Some("bridge".to_string()),
                        privileged: Some(false),
                        cap_drop: Some(vec!["ALL".to_string()]),
                        auto_remove: Some(false),
                        ..Default::default()
                    }),
                    labels: Some(HashMap::from([(
                        "io.loopflow.managed".to_string(),
                        "true".to_string(),
                    )])),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        let container_id = container.id;

        if let Err(err) = self
            .docker
            .start_container(&container_id, None::<StartContainerOptions<String>>)
            .await
        {
            self.remove_container(&container_id).await;
            return Err(err.into());
        }

        let mut wait_stream = self
            .docker
            .wait_container(&container_id, None::<WaitContainerOptions<String>>);
        let wait_result = wait_stream.next().await;

        let mut logs = self.docker.logs(
            &container_id,
            Some(LogsOptions::<String> {
                follow: false,
                stdout: true,
                stderr: true,
                timestamps: false,
                tail: "all".to_string(),
                ..Default::default()
            }),
        );
        let mut output = String::new();
        while let Some(entry) = logs.next().await {
            match entry {
                Ok(LogOutput::StdOut { message })
                | Ok(LogOutput::StdErr { message })
                | Ok(LogOutput::Console { message }) => {
                    output.push_str(&String::from_utf8_lossy(&message));
                }
                Err(err) => {
                    warn!(container_id, error = %err, "failed reading helper logs");
                }
                _ => {}
            }
        }

        self.remove_container(&container_id).await;

        let status =
            wait_result.ok_or_else(|| anyhow!("docker wait stream ended without status"))??;
        if status.status_code != 0 {
            return Err(anyhow!(
                "docker helper '{}' failed ({}): {}",
                label,
                status.status_code,
                output.trim()
            ));
        }
        Ok(output)
    }

    async fn ensure_volume(&self, volume_name: &str) -> Result<()> {
        if self.docker.inspect_volume(volume_name).await.is_ok() {
            return Ok(());
        }

        let mut labels = HashMap::new();
        labels.insert("io.loopflow.managed".to_string(), "true".to_string());
        labels.insert("io.loopflow.kind".to_string(), "repo-volume".to_string());

        let _ = self
            .docker
            .create_volume(CreateVolumeOptions::<String> {
                name: volume_name.to_string(),
                labels,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    fn docker_workspace_for_wave(
        repo_source: &Path,
        wave_name: &str,
        branch: &str,
    ) -> DockerWorkspace {
        let repo_identity = RepoIdentity::from_repo(repo_source);
        let volume = RepoVolumeIdentity::from_identity(&repo_identity);
        let wave_slug = {
            let slug = sanitize_token(wave_name);
            if slug.is_empty() {
                short_hash(wave_name, 12)
            } else {
                slug
            }
        };
        DockerWorkspace {
            container_shared_clone: format!("{CONTAINER_REPOS_ROOT}/{}/main", volume.repo_key),
            container_worktree: format!(
                "{CONTAINER_REPOS_ROOT}/{}/worktrees/{wave_slug}",
                volume.repo_key
            ),
            volume,
            repo_source: repo_source.to_path_buf(),
            branch: branch.to_string(),
            has_remote: repo_identity.has_remote,
        }
    }

    fn resolve_wave_run_branch(run: &WaveRun, wave: &Wave) -> String {
        if !run.branch.trim().is_empty() {
            return run.branch.clone();
        }
        let fallback = sanitize_token(&wave.name);
        if fallback.is_empty() {
            "main".to_string()
        } else {
            fallback
        }
    }

    fn resolve_host_repo(repo: &str) -> PathBuf {
        let repo_path = PathBuf::from(repo);
        crate::engine::worktrees::main_repo_root(&repo_path).unwrap_or(repo_path)
    }

    fn resolve_workspace(&self, wave_id: &str, wave_run_id: &str) -> Result<DockerWorkspace> {
        let wave_id = LfdId::from_raw(wave_id);
        let wave = self
            .store
            .get_wave(&wave_id)?
            .ok_or_else(|| anyhow!("wave not found for docker run"))?;
        let run_id = LfdId::from_raw(wave_run_id);
        let run = self
            .store
            .get_wave_run(&run_id)?
            .ok_or_else(|| anyhow!("wave run not found for docker run"))?;
        let repo_source = Self::resolve_host_repo(&wave.repo);
        let branch = Self::resolve_wave_run_branch(&run, &wave);
        Ok(Self::docker_workspace_for_wave(
            &repo_source,
            &wave.name,
            &branch,
        ))
    }

    async fn git_command(
        &self,
        workspace: &DockerWorkspace,
        label: &str,
        cmd: Vec<String>,
        include_local_repo: bool,
    ) -> Result<String> {
        let mut mounts = Vec::new();
        if include_local_repo {
            mounts.push(Self::bind_mount(
                &workspace.repo_source,
                LOCAL_REPO_MOUNT,
                true,
            ));
        }
        self.run_helper_command(label, cmd, self.helper_mounts(workspace, mounts), None)
            .await
    }

    async fn is_git_repo(&self, workspace: &DockerWorkspace, repo_path: &str) -> bool {
        self.git_command(
            workspace,
            "git-probe",
            vec![
                "git".to_string(),
                "-C".to_string(),
                repo_path.to_string(),
                "rev-parse".to_string(),
                "--is-inside-work-tree".to_string(),
            ],
            false,
        )
        .await
        .is_ok()
    }

    async fn ensure_shared_clone(&self, workspace: &DockerWorkspace) -> Result<()> {
        if self
            .is_git_repo(workspace, &workspace.container_shared_clone)
            .await
        {
            return Ok(());
        }

        let root_path = Path::new(&workspace.container_shared_clone)
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("invalid shared clone path"))?;
        self.run_helper_command(
            "mkdir",
            vec![
                "mkdir".to_string(),
                "-p".to_string(),
                root_path.to_string_lossy().to_string(),
            ],
            self.build_mounts(&workspace.volume.volume_name),
            None,
        )
        .await?;

        let remote = canonical_repo_url(&workspace.repo_source);
        let (source, include_local_repo) = if let Some(url) = remote {
            (url, false)
        } else {
            (LOCAL_REPO_MOUNT.to_string(), true)
        };

        self.git_command(
            workspace,
            "git-clone",
            vec![
                "git".to_string(),
                "clone".to_string(),
                source,
                workspace.container_shared_clone.clone(),
            ],
            include_local_repo,
        )
        .await?;

        Ok(())
    }

    async fn fetch_shared_clone(&self, workspace: &DockerWorkspace) -> Result<()> {
        self.git_command(
            workspace,
            "git-fetch",
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_shared_clone.clone(),
                "fetch".to_string(),
                "--prune".to_string(),
                "origin".to_string(),
            ],
            !workspace.has_remote,
        )
        .await?;
        Ok(())
    }

    async fn ensure_worktree(&self, workspace: &DockerWorkspace) -> Result<()> {
        if self
            .is_git_repo(workspace, &workspace.container_worktree)
            .await
        {
            return Ok(());
        }

        let parent = Path::new(&workspace.container_worktree)
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("invalid worktree path"))?;
        self.run_helper_command(
            "mkdir",
            vec![
                "mkdir".to_string(),
                "-p".to_string(),
                parent.to_string_lossy().to_string(),
            ],
            self.build_mounts(&workspace.volume.volume_name),
            None,
        )
        .await?;

        let local_branch_ref = format!("refs/heads/{}", workspace.branch);
        let has_local_branch = self
            .git_command(
                workspace,
                "git-show-ref-local",
                vec![
                    "git".to_string(),
                    "-C".to_string(),
                    workspace.container_shared_clone.clone(),
                    "show-ref".to_string(),
                    "--verify".to_string(),
                    "--quiet".to_string(),
                    local_branch_ref,
                ],
                false,
            )
            .await
            .is_ok();

        let command = if has_local_branch {
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_shared_clone.clone(),
                "worktree".to_string(),
                "add".to_string(),
                "--force".to_string(),
                workspace.container_worktree.clone(),
                workspace.branch.clone(),
            ]
        } else {
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_shared_clone.clone(),
                "worktree".to_string(),
                "add".to_string(),
                "--force".to_string(),
                "-B".to_string(),
                workspace.branch.clone(),
                workspace.container_worktree.clone(),
                "HEAD".to_string(),
            ]
        };

        self.git_command(workspace, "git-worktree-add", command, false)
            .await?;
        Ok(())
    }

    async fn run_hygiene(&self, workspace: &DockerWorkspace) -> Result<()> {
        let target_ref = format!("refs/heads/{}", workspace.branch);
        let target = if self
            .git_command(
                workspace,
                "git-rev-parse",
                vec![
                    "git".to_string(),
                    "-C".to_string(),
                    workspace.container_worktree.clone(),
                    "rev-parse".to_string(),
                    "--verify".to_string(),
                    target_ref.clone(),
                ],
                false,
            )
            .await
            .is_ok()
        {
            target_ref
        } else {
            "HEAD".to_string()
        };

        self.git_command(
            workspace,
            "git-reset",
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_worktree.clone(),
                "reset".to_string(),
                "--hard".to_string(),
                target,
            ],
            false,
        )
        .await?;

        self.git_command(
            workspace,
            "git-clean",
            vec![
                "git".to_string(),
                "-C".to_string(),
                workspace.container_worktree.clone(),
                "clean".to_string(),
                "-fdx".to_string(),
            ],
            false,
        )
        .await?;
        Ok(())
    }

    async fn sync_to_host_worktree(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
    ) -> Result<()> {
        let script = format!(
            "set -eu\nfind {HOST_WORKTREE_MOUNT} -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {{}} +\ntar -C '{}' --exclude=.git -cf - . | tar -C {HOST_WORKTREE_MOUNT} -xf -",
            workspace.container_worktree
        );
        self.run_helper_command(
            "sync-host",
            vec!["sh".to_string(), "-lc".to_string(), script],
            self.helper_mounts(
                workspace,
                vec![Self::bind_mount(host_worktree, HOST_WORKTREE_MOUNT, false)],
            ),
            None,
        )
        .await?;
        Ok(())
    }

    async fn prepare_workspace(
        &self,
        workspace: &DockerWorkspace,
        wave_run_id: &str,
        host_worktree: &Path,
    ) -> Result<()> {
        self.ensure_volume(&workspace.volume.volume_name).await?;

        let should_hygiene = {
            let mut prepared = self.prepared_runs.lock().await;
            prepared.insert(wave_run_id.to_string())
        };

        let lock = self
            .mutation_locks
            .for_key(&workspace.volume.repo_key)
            .await;
        {
            let _guard = lock.lock().await;
            self.ensure_shared_clone(workspace).await?;
            if should_hygiene {
                self.fetch_shared_clone(workspace).await?;
            }
            self.ensure_worktree(workspace).await?;
        }

        if should_hygiene {
            self.run_hygiene(workspace).await?;
        }

        self.sync_to_host_worktree(workspace, host_worktree).await?;

        Ok(())
    }
}

fn inspected_container(details: ContainerInspectResponse) -> Option<InspectedContainer> {
    let id = details.id?;
    let running = details
        .state
        .as_ref()
        .and_then(|state| state.running)
        .unwrap_or(false);
    Some(InspectedContainer { id, running })
}

fn is_container_not_found(err: &DockerError) -> bool {
    matches!(
        err,
        DockerError::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

#[async_trait]
impl AgentExecutor for DockerExecutor {
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

        let workspace = self.resolve_workspace(wave_id, wave_run_id)?;
        self.prepare_workspace(&workspace, wave_run_id, cwd).await?;

        let container_name = Self::build_container_name(agent_id);
        let cmd = Self::rewrite_command_paths(cmd, cwd, &workspace.container_worktree);
        let env = self.collect_env();
        let mounts = self.build_mounts(&workspace.volume.volume_name);
        let labels = Self::build_agent_labels(agent_id, wave_id, wave_run_id);

        let host_config = HostConfig {
            mounts: Some(mounts),
            network_mode: Some("bridge".to_string()),
            privileged: Some(false),
            cap_drop: Some(vec!["ALL".to_string()]),
            auto_remove: Some(false),
            ..Default::default()
        };

        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name,
                    platform: None,
                }),
                DockerContainerConfig {
                    image: Some(self.image.clone()),
                    cmd: Some(cmd),
                    working_dir: Some(workspace.container_worktree.clone()),
                    env: Some(env),
                    user: Some("root".to_string()),
                    host_config: Some(host_config),
                    labels: Some(labels),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await?;

        let container_id = container.id;
        let agent_lfd_id = LfdId::from_raw(agent_id);
        let _ = self.store.update_agent_status(
            &agent_lfd_id,
            AgentStatus::Running.as_i32(),
            None,
            Some(&container_id),
        );
        self.active
            .lock()
            .await
            .insert(agent_id.to_string(), container_id.clone());

        if let Err(err) = self
            .docker
            .start_container(&container_id, None::<StartContainerOptions<String>>)
            .await
        {
            self.active.lock().await.remove(agent_id);
            self.remove_container(&container_id).await;
            return Err(err.into());
        }

        let exit_code = self
            .wait_for_container_with_logs(&container_id, output, wave_id, wave_run_id, agent_id)
            .await;
        self.active.lock().await.remove(agent_id);
        self.remove_container(&container_id).await;

        self.sync_to_host_worktree(&workspace, cwd).await?;

        exit_code
    }

    async fn terminate(&self, agent_id: &str) -> Result<()> {
        let container_id = self.active.lock().await.remove(agent_id);
        if let Some(container_id) = container_id {
            let _ = self
                .docker
                .stop_container(&container_id, Some(StopContainerOptions { t: 1 }))
                .await;
            self.remove_container(&container_id).await;
        }
        Ok(())
    }

    async fn recover_startup(&self, output: &OutputHub) -> Result<StartupRecovery> {
        let backend = BollardRecoveryBackend::new(self.docker.clone());
        self.recover_startup_with_backend(&backend, output, true)
            .await
    }

    async fn cleanup_wave(&self, wave: &Wave) -> Result<()> {
        let repo = Self::resolve_host_repo(&wave.repo);
        let workspace = Self::docker_workspace_for_wave(&repo, &wave.name, "main");
        if self
            .docker
            .inspect_volume(&workspace.volume.volume_name)
            .await
            .is_err()
        {
            return Ok(());
        }

        let lock = self
            .mutation_locks
            .for_key(&workspace.volume.repo_key)
            .await;
        let _guard = lock.lock().await;
        if !self
            .is_git_repo(&workspace, &workspace.container_shared_clone)
            .await
        {
            return Ok(());
        }

        let _ = self
            .git_command(
                &workspace,
                "git-worktree-remove",
                vec![
                    "git".to_string(),
                    "-C".to_string(),
                    workspace.container_shared_clone.clone(),
                    "worktree".to_string(),
                    "remove".to_string(),
                    "--force".to_string(),
                    workspace.container_worktree.clone(),
                ],
                false,
            )
            .await;
        Ok(())
    }
}

async fn read_stream<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    output: OutputHub,
    wave_id: String,
    wave_run_id: String,
    agent_id: String,
) {
    let mut parser = StreamParser::new();
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        handle_output_line(
            &line,
            &mut parser,
            &output,
            &wave_id,
            &wave_run_id,
            &agent_id,
        );
    }
}

fn handle_output_line(
    line: &str,
    parser: &mut StreamParser,
    output: &OutputHub,
    wave_id: &str,
    wave_run_id: &str,
    agent_id: &str,
) {
    match parser.feed_line(line) {
        ParseResult::Events(events) => {
            for event in &events {
                let (stdout, stderr) = render_event(event, false);
                let text = if !stdout.is_empty() { stdout } else { stderr };
                if !text.is_empty() {
                    send_output(output, wave_id, wave_run_id, agent_id, text);
                }
            }
        }
        ParseResult::Skipped => {}
        ParseResult::Passthrough => {
            send_output(output, wave_id, wave_run_id, agent_id, line.to_string());
        }
    }
}

fn send_output(output: &OutputHub, wave_id: &str, wave_run_id: &str, agent_id: &str, text: String) {
    output.send(OutputEvent {
        wave_id: wave_id.to_string(),
        wave_run_id: wave_run_id.to_string(),
        agent_id: agent_id.to_string(),
        text,
    });
}

#[derive(Clone)]
pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    output: OutputHub,
    runner: Arc<dyn AgentExecutor>,
    event_hub: EventHub,
    executor_type: ExecutorType,
}

impl WaveExecutor {
    pub fn new(
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        output: OutputHub,
        event_hub: EventHub,
        config: ExecutorConfig,
    ) -> Result<Self> {
        let executor_type = config.r#type;
        let runner: Arc<dyn AgentExecutor> = match executor_type {
            ExecutorType::Docker => Arc::new(DockerExecutor::new(store.clone(), &config)?),
            ExecutorType::Local => Arc::new(LocalProcessExecutor::new(store.clone())),
        };
        Ok(Self {
            store,
            scheduler,
            output,
            runner,
            event_hub,
            executor_type,
        })
    }

    #[cfg(test)]
    pub fn with_runner(
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        output: OutputHub,
        event_hub: EventHub,
        runner: Arc<dyn AgentExecutor>,
    ) -> Self {
        Self {
            store,
            scheduler,
            output,
            runner,
            event_hub,
            executor_type: ExecutorType::Local,
        }
    }

    pub fn executor_type(&self) -> ExecutorType {
        self.executor_type
    }

    pub async fn recover_startup(&self) -> Result<StartupRecovery> {
        self.runner.recover_startup(&self.output).await
    }

    pub async fn cleanup_wave_workspace(&self, wave: &Wave) -> Result<()> {
        self.runner.cleanup_wave(wave).await
    }

    pub async fn terminate_agent(&self, agent_id: &LfdId) -> Result<()> {
        self.runner.terminate(agent_id.as_str()).await
    }

    pub async fn execute(&self, run_id: &LfdId) -> Result<()> {
        let mut run = self
            .store
            .get_wave_run(run_id)?
            .ok_or_else(|| anyhow!("wave run not found"))?;
        if run.status == WaveRunStatus::Completed || run.status == WaveRunStatus::Failed {
            return Ok(());
        }

        let wave = self
            .store
            .get_wave(&run.wave_id)?
            .ok_or_else(|| anyhow!("wave not found"))?;
        info!(run_id = %run.id, flow = %run.snapshot.flow, repo = %run.snapshot.repo, "loading flow");
        let flow = load_flow(&run.snapshot.flow, Path::new(&run.snapshot.repo))?;
        let plan = expand_flow(&flow, Path::new(&run.snapshot.repo))?;
        debug!(run_id = %run.id, plan_items = plan.len(), "flow expanded");

        loop {
            let current_flow_parents = flow_parents_for_index(&plan, run.step_index);
            if run.flow_parents != current_flow_parents {
                run.flow_parents = current_flow_parents;
                self.store.update_wave_run(&run)?;
            }

            match next_action(&plan, run.step_index as usize) {
                FlowAction::RunStep { step } => {
                    // Ensure area summary is fresh before each step
                    if let Err(err) = self.ensure_summary_fresh(&wave, &run).await {
                        warn!(run_id = %run.id, error = %err, "summary refresh failed, continuing");
                    }
                    info!(run_id = %run.id, step = %step.step.name, step_index = run.step_index, "running step");
                    let exit_code = self.run_step(&wave, &mut run, &step).await?;
                    if exit_code == 0 {
                        run.step_index += 1;
                        run.status = WaveRunStatus::Running;
                        run.flow_parents = flow_parents_for_index(&plan, run.step_index);
                        self.store.update_wave_run(&run)?;
                        self.event_hub.send(Event::wave_updated(wave.id.clone()));
                    } else {
                        self.fail_run(&mut run, &wave, format!("step {} failed", step.step.name))?;
                        return Ok(());
                    }
                }
                FlowAction::WaitInteractive { step } => {
                    let model = step
                        .step
                        .model
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let worktree = run.worktree.clone();
                    let agent = build_agent_for_step(
                        &run.id,
                        &run.snapshot.repo,
                        &worktree,
                        &step,
                        AgentStatus::Waiting,
                        &model,
                    );
                    self.store.start_agent(&agent)?;
                    run.status = WaveRunStatus::Waiting;
                    run.flow_parents = step.flow_parents.clone();
                    self.store.update_wave_run(&run)?;
                    self.set_wave_status(&wave.id, WaveStatus::Waiting);
                    self.event_hub.send(Event::wave_waiting(
                        wave.id.clone(),
                        run.id.clone(),
                        step.step.name.clone(),
                    ));
                    return Ok(());
                }
                FlowAction::Fork { fork } => match &fork.select {
                    ForkSelect::All => {
                        info!(
                            run_id = %run.id,
                            branches = fork.branches.len(),
                            step_index = run.step_index,
                            synthesize = ?fork.synthesize,
                            "running fork (all branches)"
                        );
                        self.run_fork(&wave, &mut run, &plan, &fork).await?;
                        if run.status == WaveRunStatus::Failed {
                            return Ok(());
                        }
                    }
                    ForkSelect::One | ForkSelect::Prompt { .. } => {
                        info!(run_id = %run.id, step_index = run.step_index, "running fork (choose)");
                        self.run_choose(&wave, &mut run, &plan, &fork).await?;
                        if run.status == WaveRunStatus::Failed {
                            return Ok(());
                        }
                    }
                },
                FlowAction::Complete => {
                    run.status = WaveRunStatus::Completed;
                    run.ended_at = Some(OffsetDateTime::now_utc());

                    let is_recurring = self
                        .store
                        .list_stimuli(Some(&wave.id))
                        .map(|stimuli| {
                            stimuli.iter().any(|s| {
                                matches!(
                                    s.kind,
                                    StimulusKind::Loop | StimulusKind::Watch | StimulusKind::Cron
                                )
                            })
                        })
                        .unwrap_or(false);

                    // Auto-create PR (draft for manual, ready for auto stimulus).
                    let worktree = run.worktree.clone();
                    match tokio::task::spawn_blocking(move || {
                        auto_create_pr(Path::new(&worktree), is_recurring)
                    })
                    .await
                    {
                        Ok(Some(pr)) => {
                            info!(run_id = %run.id, url = %pr.url, "auto-created PR");
                            run.snapshot.pr = Some(pr);
                        }
                        Ok(None) => {}
                        Err(err) => {
                            warn!(run_id = %run.id, error = %err, "failed to auto-create PR");
                        }
                    }

                    // For recurring waves, advance to a new branch so the
                    // next iteration gets its own PR.
                    if run.snapshot.pr.is_some() && is_recurring {
                        let wt = run.worktree.clone();
                        let name = wave.name.clone();
                        match tokio::task::spawn_blocking(move || {
                            advance_branch(Path::new(&wt), &name)
                        })
                        .await
                        {
                            Ok(Ok(new_branch)) => {
                                info!(
                                    run_id = %run.id,
                                    new_branch = %new_branch,
                                    "advanced to new branch for next iteration"
                                );
                            }
                            Ok(Err(err)) => {
                                warn!(
                                    run_id = %run.id,
                                    error = %err,
                                    "failed to advance branch"
                                );
                            }
                            Err(err) => {
                                warn!(
                                    run_id = %run.id,
                                    error = %err,
                                    "advance_branch task panicked"
                                );
                            }
                        }
                    }

                    self.store.update_wave_run(&run)?;
                    // Wave goes back to Idle after a run completes — the run
                    // is done, but the wave is ready for its next iteration.
                    self.set_wave_status(&wave.id, WaveStatus::Idle);
                    self.event_hub.send(Event::wave_updated(wave.id.clone()));
                    return Ok(());
                }
            }
        }
    }

    fn set_wave_status(&self, wave_id: &LfdId, status: WaveStatus) {
        if let Ok(Some(mut wave)) = self.store.get_wave(wave_id) {
            wave.status = status;
            if let Err(err) = self.store.update_wave(&wave) {
                error!(wave_id = %wave_id, ?status, error = %err, "failed to update wave status");
            }
        }
    }

    fn fail_run(&self, run: &mut WaveRun, wave: &Wave, error: String) -> Result<()> {
        run.status = WaveRunStatus::Failed;
        run.ended_at = Some(OffsetDateTime::now_utc());
        run.error = Some(error);
        self.store.update_wave_run(run)?;
        self.set_wave_status(&wave.id, WaveStatus::Failed);
        self.event_hub.send(Event::wave_updated(wave.id.clone()));
        Ok(())
    }

    async fn run_step(&self, wave: &Wave, run: &mut WaveRun, step: &ConcreteStep) -> Result<i32> {
        let worktree = run.worktree.clone();
        debug!(run_id = %run.id, step = %step.step.name, worktree = %worktree, "building step prompt");
        let (prompt, model, launch) = build_step_prompt(
            &worktree,
            step,
            &run.snapshot.direction,
            Some(&wave.name),
            Some((&self.store, &wave.id)),
        )?;
        let cmd = build_agent_command(&model, &prompt, &launch);
        info!(run_id = %run.id, step = %step.step.name, model = %model, "launching agent");

        let agent = build_agent_for_step(
            &run.id,
            &run.snapshot.repo,
            &worktree,
            step,
            AgentStatus::Running,
            &model,
        );
        let agent_id = agent.id.clone();
        self.store.start_agent(&agent)?;
        self.event_hub.send(Event::agent_started(
            agent_id.clone(),
            step.step.name.clone(),
            worktree.clone(),
        ));

        let exit_code = self
            .runner
            .run(
                cmd,
                Path::new(&worktree),
                run.wave_id.as_str(),
                agent_id.as_str(),
                run.id.as_str(),
                &self.output,
            )
            .await?;

        let ended_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let status = if exit_code == 0 {
            AgentStatus::Completed
        } else {
            AgentStatus::Failed
        };
        self.store.end_agent(&agent_id, status.as_i32(), ended_at)?;
        self.event_hub.send(Event::agent_ended(agent_id, status));

        Ok(exit_code)
    }

    // Summary management

    /// Check if the wave's area summary is fresh; regenerate if stale or missing.
    pub(crate) async fn ensure_summary_fresh(&self, wave: &Wave, run: &WaveRun) -> Result<()> {
        if wave.area.is_empty() {
            return Ok(());
        }

        let worktree_path = Path::new(&run.worktree);
        let current_hash = match hash_areas(worktree_path, &wave.area) {
            Ok(h) => h,
            Err(err) => {
                warn!(wave = %wave.name, error = %err, "failed to hash areas, skipping summary");
                return Ok(());
            }
        };

        if let Ok(Some(existing)) = self.store.get_summary(&wave.id) {
            if existing.source_hash == current_hash {
                debug!(wave = %wave.name, "summary is fresh");
                return Ok(());
            }
            info!(wave = %wave.name, "summary is stale, regenerating");
        } else {
            info!(wave = %wave.name, "no summary found, generating");
        }

        self.run_internal_summarize(wave, run, &current_hash).await
    }

    /// Run the builtin summarize step as an internal agent and store the result.
    async fn run_internal_summarize(
        &self,
        wave: &Wave,
        run: &WaveRun,
        source_hash: &str,
    ) -> Result<()> {
        let template = get_builtin_ops_prompt("summarize")
            .ok_or_else(|| anyhow!("builtin summarize prompt not found"))?;

        let config = load_config_or_default(Some(Path::new(&run.worktree)));
        let token_budget = config.summary_tokens;

        // Build the prompt with area paths as content guidance
        let area_list = wave.area.join(", ");
        let prompt = template
            .replace("{token_budget}", &token_budget.to_string())
            .replace(
                "{content}",
                &format!("Read and summarize these paths: {area_list}"),
            );

        let model = config.agent_model.clone();
        let launch = LaunchConfig {
            auto: true,
            stream: true,
            skip_permissions: config.yolo,
            model_variant: None,
            chrome: false,
            cwd: Some(PathBuf::from(&run.worktree)),
            context_file: None,
            ..Default::default()
        };

        let cmd = build_agent_command(&model, &prompt, &launch);
        info!(wave = %wave.name, model = %model, "running internal summarize step");

        let step = ConcreteStep {
            step: Step {
                name: "_summarize".to_string(),
                model: Some(model.clone()),
                directions: Vec::new(),
                interactive: Some(false),
                content: None,
            },
            flow_parents: Vec::new(),
        };

        let agent = build_agent_for_step(
            &run.id,
            &run.snapshot.repo,
            &run.worktree,
            &step,
            AgentStatus::Running,
            &config.agent_model,
        );
        let agent_id = agent.id.clone();
        self.store.start_agent(&agent)?;

        let exit_code = self
            .runner
            .run(
                cmd,
                Path::new(&run.worktree),
                wave.id.as_str(),
                agent_id.as_str(),
                run.id.as_str(),
                &self.output,
            )
            .await?;

        let ended_at = OffsetDateTime::now_utc().unix_timestamp();
        let status = if exit_code == 0 {
            AgentStatus::Completed
        } else {
            AgentStatus::Failed
        };
        self.store.end_agent(&agent_id, status.as_i32(), ended_at)?;

        if exit_code != 0 {
            warn!(wave = %wave.name, exit_code, "summarize step failed, continuing without summary");
            return Ok(());
        }

        // Read the summary file the agent wrote
        let summary_path = Path::new(&run.worktree).join(".lf/summary.md");
        match std::fs::read_to_string(&summary_path) {
            Ok(content) if !content.trim().is_empty() => {
                let summary = Summary {
                    id: LfdId::new(),
                    wave_id: wave.id.clone(),
                    content,
                    source_hash: source_hash.to_string(),
                    token_budget: token_budget as u32,
                    model: config.agent_model,
                    created_at: Some(OffsetDateTime::now_utc()),
                };
                self.store.upsert_summary(&summary)?;
                info!(wave = %wave.name, "summary stored");
            }
            Ok(_) => {
                warn!(wave = %wave.name, "summarize step produced empty output");
            }
            Err(err) => {
                warn!(wave = %wave.name, error = %err, "failed to read summary file");
            }
        }

        Ok(())
    }

    async fn run_choose(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        if fork.branches.is_empty() {
            self.fail_run(run, wave, "fork has no branches".to_string())?;
            return Ok(());
        }

        let selected = fork
            .branches
            .first()
            .ok_or_else(|| anyhow!("fork has no branches"))?
            .clone();

        if selected.step.interactive.unwrap_or(false) {
            self.fail_run(
                run,
                wave,
                "interactive fork branches are not supported".to_string(),
            )?;
            return Ok(());
        }

        let exit_code = self.run_step(wave, run, &selected).await?;
        if exit_code != 0 {
            self.fail_run(
                run,
                wave,
                format!("fork step {} failed", selected.step.name),
            )?;
            return Ok(());
        }

        run.step_index += 1;
        run.status = WaveRunStatus::Running;
        run.flow_parents = flow_parents_for_index(plan, run.step_index);
        self.store.update_wave_run(run)?;
        self.event_hub.send(Event::wave_updated(wave.id.clone()));
        Ok(())
    }

    async fn run_fork(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        if self.executor_type == ExecutorType::Docker {
            self.fail_run(
                run,
                wave,
                "fork(select=all) is not supported by the docker executor yet".to_string(),
            )?;
            return Ok(());
        }

        let mut fork_runs = Vec::new();
        for (index, branch) in fork.branches.iter().enumerate() {
            if branch.step.interactive.unwrap_or(false) {
                self.fail_run(
                    run,
                    wave,
                    "interactive fork branches are not supported".to_string(),
                )?;
                return Ok(());
            }

            let fork_worktree = fork_worktree_path(run, index as u32);
            if !Path::new(&fork_worktree).exists() {
                debug!(
                    run_id = %run.id,
                    branch_index = index,
                    step = %branch.step.name,
                    worktree = %fork_worktree,
                    "creating fork worktree"
                );
                create_worktree(
                    Path::new(&run.snapshot.repo),
                    Path::new(&fork_worktree),
                    &format!("{}-fork-{}", run.id, index),
                )?;
            }

            let fork_run = ForkRun {
                id: LfdId::new(),
                wave_run_id: run.id.clone(),
                step_index: run.step_index,
                branch_index: index as u32,
                status: ForkRunStatus::Pending,
                worktree: fork_worktree,
            };
            self.store.upsert_fork_run(&fork_run)?;
            fork_runs.push((fork_run, branch.clone()));
        }

        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(fork_runs.len());
        let mut handles = Vec::new();

        let wave_directions = run.snapshot.direction.clone();
        for (fork_run, step) in fork_runs.iter() {
            let store = self.store.clone();
            let runner = self.runner.clone();
            let output = self.output.clone();
            let scheduler = self.scheduler.clone();
            let event_hub = self.event_hub.clone();
            let cancel = cancel.clone();
            let tx = tx.clone();
            let fork_wave_id = wave.id.clone();
            let wave_run_id = run.id.clone();
            let wave_repo = run.snapshot.repo.clone();
            let worktree = fork_run.worktree.clone();
            let fork_run_id = fork_run.id.clone();
            let fork_run = fork_run.clone();
            let step = step.clone();
            let wave_directions = wave_directions.clone();

            let handle = tokio::spawn(async move {
                if cancel.is_cancelled() {
                    return;
                }

                loop {
                    let (acquired, _) = scheduler.acquire(fork_run_id.as_str()).await;
                    if acquired {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }

                let _ = store.upsert_fork_run(&ForkRun {
                    status: ForkRunStatus::Running,
                    ..fork_run.clone()
                });

                debug!(
                    fork_run_id = %fork_run_id,
                    step = %step.step.name,
                    worktree = %worktree,
                    directions = ?wave_directions,
                    "building fork branch prompt"
                );
                let prompt = build_step_prompt(
                    &worktree,
                    &step,
                    &wave_directions,
                    None,
                    Some((&store, &fork_wave_id)),
                );
                let (prompt, model, launch) = match prompt {
                    Ok(result) => result,
                    Err(err) => {
                        error!(
                            fork_run_id = %fork_run_id,
                            step = %step.step.name,
                            error = %err,
                            "fork branch prompt build failed"
                        );
                        let _ = tx.send((fork_run_id.to_string(), Err(err))).await;
                        scheduler.release(fork_run_id.as_str());
                        return;
                    }
                };
                let cmd = build_agent_command(&model, &prompt, &launch);
                info!(
                    fork_run_id = %fork_run_id,
                    step = %step.step.name,
                    model = %model,
                    cmd_len = cmd.len(),
                    "launching fork branch agent"
                );
                let agent = build_agent_for_step(
                    &wave_run_id,
                    &wave_repo,
                    &worktree,
                    &step,
                    AgentStatus::Running,
                    &model,
                );
                let _ = store.start_agent(&agent);
                event_hub.send(Event::agent_started(
                    agent.id.clone(),
                    step.step.name.clone(),
                    worktree.clone(),
                ));

                let result = runner
                    .run(
                        cmd,
                        Path::new(&worktree),
                        fork_wave_id.as_str(),
                        agent.id.as_str(),
                        wave_run_id.as_str(),
                        &output,
                    )
                    .await;

                // End the agent in the store (mirrors run_step behavior).
                let ended_at = time::OffsetDateTime::now_utc().unix_timestamp();
                let agent_status = match &result {
                    Ok(0) => AgentStatus::Completed,
                    _ => AgentStatus::Failed,
                };
                let _ = store.end_agent(&agent.id, agent_status.as_i32(), ended_at);
                event_hub.send(Event::agent_ended(agent.id.clone(), agent_status));

                let status = match &result {
                    Ok(0) => {
                        info!(fork_run_id = %fork_run_id, step = %step.step.name, "fork branch completed");
                        ForkRunStatus::Completed
                    }
                    Ok(code) => {
                        warn!(fork_run_id = %fork_run_id, step = %step.step.name, exit_code = code, "fork branch failed");
                        ForkRunStatus::Failed
                    }
                    Err(err) => {
                        error!(fork_run_id = %fork_run_id, step = %step.step.name, error = %err, "fork branch error");
                        ForkRunStatus::Failed
                    }
                };
                let _ = store.upsert_fork_run(&ForkRun {
                    status,
                    ..fork_run.clone()
                });
                let _ = tx.send((fork_run_id.to_string(), result)).await;
                scheduler.release(fork_run_id.as_str());
            });

            handles.push(handle);
        }

        let mut failures = None;
        let mut completed = 0usize;
        let total = fork_runs.len();
        debug!(run_id = %run.id, total_branches = total, "waiting for fork results");
        while let Some((fork_id, result)) = rx.recv().await {
            match result {
                Ok(0) => {
                    completed += 1;
                    debug!(run_id = %run.id, completed, total, "fork branch done");
                    if completed == total {
                        break;
                    }
                }
                Ok(code) => {
                    failures = Some(format!("fork branch {} exited with code {}", fork_id, code));
                    break;
                }
                Err(err) => {
                    failures = Some(format!("fork branch {} error: {}", fork_id, err));
                    break;
                }
            }
        }

        if let Some(error) = failures {
            error!(run_id = %run.id, error = %error, "fork failed");
            cancel.cancel();
            for handle in handles {
                handle.abort();
            }
            self.cleanup_fork(run, &fork_runs).await;
            self.fail_run(run, wave, error)?;
            return Ok(());
        }

        if let Some(step_name) = fork.synthesize.as_deref() {
            let synth_step = ConcreteStep {
                step: Step::named(step_name),
                flow_parents: fork.flow_parents.clone(),
            };
            let exit_code = self.run_step(wave, run, &synth_step).await?;
            if exit_code != 0 {
                self.cleanup_fork(run, &fork_runs).await;
                self.fail_run(run, wave, format!("synthesize {} failed", step_name))?;
                return Ok(());
            }
        }

        self.cleanup_fork(run, &fork_runs).await;
        run.step_index += 1;
        run.status = WaveRunStatus::Running;
        run.flow_parents = flow_parents_for_index(plan, run.step_index);
        self.store.update_wave_run(run)?;
        self.event_hub.send(Event::wave_updated(wave.id.clone()));
        Ok(())
    }

    async fn cleanup_fork(&self, run: &WaveRun, fork_runs: &[(ForkRun, ConcreteStep)]) {
        for (fork_run, _) in fork_runs {
            let worktree_path = Path::new(&fork_run.worktree);
            if worktree_path.join(".git").exists() {
                let _ = remove_worktree(worktree_path, true);
            }
            self.scheduler.release(fork_run.id.as_str());
        }
        let _ = self.store.delete_fork_runs(&run.id, run.step_index);
    }
}

/// Create a wave run with a worktree and branch for the wave.
pub fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
) -> anyhow::Result<WaveRun> {
    let last_run = store
        .list_wave_runs(Some(&wave.id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let main_repo = Path::new(&wave.repo);
    let (wt_path, branch) = ensure_wave_worktree(main_repo, &wave.name)?;

    let run = WaveRun {
        id: run_id.clone(),
        wave_id: wave.id.clone(),
        snapshot: WaveRunSnapshot {
            repo: wave.repo.clone(),
            flow: wave.flow.clone(),
            direction: wave.direction.clone(),
            area: wave.area.clone(),
            pr: None,
        },
        iteration,
        step_index: 0,
        status: WaveRunStatus::Running,
        worktree: wt_path,
        branch,
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
    };
    store.create_wave_run(&run)?;
    if let Ok(Some(mut wave)) = store.get_wave(&wave.id) {
        wave.status = WaveStatus::Running;
        wave.iteration = iteration;
        if let Err(err) = store.update_wave(&wave) {
            warn!(wave_id = %wave.id, error = %err, "failed to set wave status to running");
        }
    }
    Ok(run)
}

/// Create a worktree for this wave, or reuse the existing one.
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<(String, String)> {
    let wt = wave_worktree_path(main_repo, wave_name);
    if wt.exists() {
        let branch = current_branch(&wt)?.unwrap_or_default();
        if !branch.is_empty() {
            schedule_upstream_sync(wt.clone(), branch.clone());
        }
        return Ok((wt.to_string_lossy().to_string(), branch));
    }

    let config = load_config(Some(main_repo)).ok().flatten();
    let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
    let result = create_with_schema(main_repo, wave_name, None, branch_config)?;
    Ok((result.path.to_string_lossy().to_string(), result.branch))
}

fn fork_worktree_path(run: &WaveRun, branch_index: u32) -> String {
    format!("{}-fork-{branch_index}", run.worktree)
}

fn merge_directions(base: &[String], extra: &[String]) -> Vec<String> {
    if extra.is_empty() {
        return base.to_vec();
    }
    let mut combined = base.to_vec();
    for item in extra {
        if !combined.contains(item) {
            combined.push(item.clone());
        }
    }
    combined
}

fn flow_parents_for_index(items: &[ConcreteItem], step_index: u32) -> Vec<String> {
    match items.get(step_index as usize) {
        Some(ConcreteItem::Step(step)) => step.flow_parents.clone(),
        Some(ConcreteItem::Fork(fork)) => fork.flow_parents.clone(),
        None => Vec::new(),
    }
}

fn build_step_prompt(
    worktree: &str,
    step: &ConcreteStep,
    directions: &[String],
    wave: Option<&str>,
    summary_source: Option<(&SharedStore, &LfdId)>,
) -> Result<(String, String, LaunchConfig)> {
    let config = load_config_or_default(Some(Path::new(worktree)));
    let directions = merge_directions(directions, &step.step.directions);
    let opts = GatherContextOpts {
        repo_root: PathBuf::from(worktree),
        step: Some(step.step.name.clone()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions,
        files: Vec::new(),
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: config.paste,
        area: config.area.clone(),
        wave: wave.map(str::to_string),
    };

    let mut components = gather_context(&opts)?;
    let repo_root = PathBuf::from(worktree);
    drop_native_instruction_docs(&mut components, &repo_root);

    // Inject wave summary if available
    if let Some((store, wave_id)) = summary_source {
        if let Ok(Some(summary)) = store.get_summary(wave_id) {
            components.summaries.push(Document {
                path: "wave-summary".to_string(),
                content: summary.content,
                category: "summaries".to_string(),
            });
        }
    }
    let (components, _breakdown) = trim_context_with_breakdown(components, DEFAULT_CONTEXT_BUDGET);

    // Log full prompt, then write context/task split for --append-system-prompt-file
    let _ = write_prompt_log(
        &repo_root,
        &format_prompt(&components),
        &step.step.name,
        None,
    );
    let task_prompt = format_task_prompt(&components);
    let context_file = write_prompt_log(
        &repo_root,
        &format_context_prompt(&components),
        &format!("{}.context", step.step.name),
        None,
    )
    .ok();

    let model = step
        .step
        .model
        .clone()
        .unwrap_or_else(|| config.agent_model.clone());
    let launch = LaunchConfig {
        auto: true,
        stream: true,
        skip_permissions: config.yolo,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo_root),
        context_file,
        ..Default::default()
    };

    Ok((task_prompt, model, launch))
}

fn build_agent_for_step(
    wave_run_id: &LfdId,
    repo: &str,
    worktree: &str,
    step: &ConcreteStep,
    status: AgentStatus,
    model: &str,
) -> Agent {
    Agent {
        id: LfdId::new(),
        step: step.step.name.clone(),
        repo: repo.to_string(),
        worktree: worktree.to_string(),
        wave_run_id: Some(wave_run_id.clone()),
        status,
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        pid: None,
        container_id: None,
        model: model.to_string(),
        run_mode: "auto".to_string(),
    }
}

/// Commit any remaining changes, push, and create a draft PR.
/// When `mark_ready` is true (auto-stimulus waves), converts the draft to a real PR.
/// Returns the PR info if successful, None if skipped or failed.
fn auto_create_pr(worktree: &Path, mark_ready: bool) -> Option<crate::lfd::types::PullRequest> {
    use crate::ops::{
        commit_workflow, current_pr, generate_pr_message, update_pr, CommitOptions, NullProgress,
    };

    let commit_options = CommitOptions {
        add: true,
        lint: false,
        push: true,
        create_draft_pr: true,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: None,
    };
    if let Err(err) = commit_workflow(worktree, &commit_options, &NullProgress) {
        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: commit/push failed");
        return None;
    }

    match current_pr(worktree) {
        Ok(Some(pr)) => {
            let mut title = None;

            // Update the draft PR with an LLM-generated title and description,
            // matching what `lf ops pr` produces.
            match generate_pr_message(worktree) {
                Ok(message) => {
                    title = Some(message.title.clone());
                    if let Err(err) = update_pr(worktree, pr.number, &message.title, &message.body)
                    {
                        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to update title/body");
                    }
                }
                Err(err) => {
                    warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to generate PR message");
                }
            }

            // Auto-stimulus waves get their draft promoted to a real PR.
            if mark_ready {
                if let Err(err) = crate::ops::mark_ready(worktree) {
                    warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to mark PR ready");
                }
            }

            Some(crate::lfd::types::PullRequest {
                url: pr.url,
                number: Some(pr.number as u32),
                state: Some(pr.state),
                branch: Some(pr.branch),
                title,
            })
        }
        Ok(None) => {
            debug!(worktree = %worktree.display(), "auto-create PR: no PR found after push");
            None
        }
        Err(err) => {
            warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to fetch PR info");
            None
        }
    }
}

/// Create a new branch in the worktree for the next loop iteration.
fn advance_branch(worktree: &Path, wave_name: &str) -> anyhow::Result<String> {
    let config = load_config_or_default(Some(worktree));
    let branch_config = config.branch_names.as_ref();
    let mut new_branch = format_branch_name(wave_name, branch_config, worktree)
        .map_err(|e| anyhow!("failed to generate branch name: {e}"))?;

    while branch_exists(worktree, &new_branch)? {
        new_branch = format!("{new_branch}.{}", generate_word_pair());
    }

    create_branch(worktree, &new_branch)?;
    push_with_upstream(worktree, "origin", &new_branch)?;
    Ok(new_branch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::store::sqlite::SqliteStore;
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;
    use tokio::io::{AsyncWriteExt, DuplexStream};

    async fn write_lines(mut writer: DuplexStream, lines: &[&str]) {
        for line in lines {
            writer
                .write_all(line.as_bytes())
                .await
                .expect("writer should accept line");
            writer
                .write_all(b"\n")
                .await
                .expect("writer should accept newline");
        }
        writer.shutdown().await.expect("writer should shut down");
    }

    #[tokio::test]
    async fn read_stream_renders_stream_json_events() {
        let output_dir = tempdir().expect("tempdir should be created");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let (writer, reader) = tokio::io::duplex(4096);

        let write_task = tokio::spawn(write_lines(
            writer,
            &[
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/lib.rs"}}]}}"#,
                r#"{"type":"result","subtype":"success"}"#,
            ],
        ));

        read_stream(
            reader,
            output.clone(),
            "wave-1".to_string(),
            "run-1".to_string(),
            "agent-1".to_string(),
        )
        .await;

        write_task.await.expect("writer task should complete");

        let lines = output.read_log("run-1").expect("output log should exist").0;

        assert_eq!(lines, vec!["hello", "-> Read  src/lib.rs", "ok"]);
    }

    #[tokio::test]
    async fn read_stream_skips_known_events_and_passes_through_unknown_lines() {
        let output_dir = tempdir().expect("tempdir should be created");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let (writer, reader) = tokio::io::duplex(4096);

        let write_task = tokio::spawn(write_lines(
            writer,
            &[
                r#"{"type":"system","message":"skip me"}"#,
                r#"{"type":"mystery","payload":42}"#,
                "plain text line",
            ],
        ));

        read_stream(
            reader,
            output.clone(),
            "wave-1".to_string(),
            "run-2".to_string(),
            "agent-1".to_string(),
        )
        .await;

        write_task.await.expect("writer task should complete");

        let lines = output.read_log("run-2").expect("output log should exist").0;

        assert_eq!(
            lines,
            vec![r#"{"type":"mystery","payload":42}"#, "plain text line"]
        );
    }

    #[test]
    fn docker_mount_spec_requires_host_and_container_paths() {
        let home = dirs::home_dir().expect("home directory should be available");
        let mount = DockerCredentialMount::from_spec("~/.claude:/home/agent/.claude")
            .expect("mount spec should parse");
        assert_eq!(mount.host_path, home.join(".claude"));
        assert_eq!(mount.container_path, "/home/agent/.claude");
        assert!(DockerCredentialMount::from_spec("missing-colon").is_err());
        assert!(DockerCredentialMount::from_spec(":/home/agent/.claude").is_err());
        assert!(DockerCredentialMount::from_spec("relative:/home/agent/.claude").is_err());
        assert!(DockerCredentialMount::from_spec("/tmp/claude:relative").is_err());
    }

    #[test]
    fn docker_rewrites_paths_into_workspace() {
        let cmd = vec![
            "claude".to_string(),
            "--append-system-prompt-file".to_string(),
            "/tmp/worktree/.lf/prompt.md".to_string(),
            "-C".to_string(),
            "/tmp/worktree".to_string(),
            "--danger".to_string(),
            "/tmp/worktree-copy".to_string(),
        ];
        let rewritten = DockerExecutor::rewrite_command_paths(
            cmd,
            Path::new("/tmp/worktree"),
            "/workspace/repos/repo/worktrees/wave",
        );
        assert_eq!(
            rewritten,
            vec![
                "claude".to_string(),
                "--append-system-prompt-file".to_string(),
                "/workspace/repos/repo/worktrees/wave/.lf/prompt.md".to_string(),
                "-C".to_string(),
                "/workspace/repos/repo/worktrees/wave".to_string(),
                "--danger".to_string(),
                "/tmp/worktree-copy".to_string(),
            ]
        );
    }

    #[test]
    fn docker_workspace_mount_uses_volume() {
        let mounts = DockerExecutor::build_mounts_for("lfd-repo-abc", &[]);
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].typ, Some(MountTypeEnum::VOLUME));
        assert_eq!(mounts[0].source, Some("lfd-repo-abc".to_string()));
        assert_eq!(mounts[0].target, Some(CONTAINER_WORKSPACE.to_string()));
    }

    #[test]
    fn repo_volume_identity_is_deterministic_and_safe() {
        let repo = tempdir().expect("tempdir");
        let first = RepoVolumeIdentity::from_identity(&RepoIdentity::from_repo(repo.path()));
        let second = RepoVolumeIdentity::from_identity(&RepoIdentity::from_repo(repo.path()));

        assert_eq!(first, second);
        assert!(first.volume_name.starts_with("lfd-repo-"));
        assert!(first
            .volume_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.'));
    }

    #[test]
    fn repo_identity_falls_back_to_path_hash() {
        let repo = tempdir().expect("tempdir");
        let identity_a = RepoIdentity::from_repo(repo.path());
        let identity_b = RepoIdentity::from_repo(repo.path());
        assert_eq!(identity_a, identity_b);
        assert!(!identity_a.has_remote);
        assert!(identity_a.canonical.starts_with("local:"));
    }

    #[test]
    fn normalize_repo_url_handles_common_forms() {
        assert_eq!(
            normalize_repo_url("git@GitHub.com:LoopflowStudio/loopflow.git"),
            "ssh://git@github.com/LoopflowStudio/loopflow"
        );
        assert_eq!(
            normalize_repo_url("HTTPS://GITHUB.COM/loopflowstudio/loopflow.git/"),
            "https://github.com/loopflowstudio/loopflow"
        );
    }

    #[tokio::test]
    async fn repo_mutation_locks_serialize_same_repo_key() {
        let locks = RepoMutationLocks::default();
        let events = Arc::new(StdMutex::new(Vec::new()));

        let lock_a = locks.for_key("repo-1").await;
        let events_a = events.clone();
        let first = tokio::spawn(async move {
            let _guard = lock_a.lock().await;
            events_a.lock().expect("lock events").push("first-start");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            events_a.lock().expect("lock events").push("first-end");
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let lock_b = locks.for_key("repo-1").await;
        let events_b = events.clone();
        let second = tokio::spawn(async move {
            let _guard = lock_b.lock().await;
            events_b.lock().expect("lock events").push("second");
        });

        first.await.expect("first task should finish");
        second.await.expect("second task should finish");

        assert_eq!(
            events.lock().expect("events").as_slice(),
            ["first-start", "first-end", "second"]
        );
    }

    #[derive(Debug, Clone)]
    struct MockDockerRecoveryBackend {
        inspected: Arc<StdMutex<HashMap<String, InspectedContainer>>>,
        managed: Arc<StdMutex<Vec<String>>>,
        stopped: Arc<StdMutex<Vec<String>>>,
        removed: Arc<StdMutex<Vec<String>>>,
    }

    impl MockDockerRecoveryBackend {
        fn new(inspected: HashMap<String, InspectedContainer>, managed: Vec<String>) -> Self {
            Self {
                inspected: Arc::new(StdMutex::new(inspected)),
                managed: Arc::new(StdMutex::new(managed)),
                stopped: Arc::new(StdMutex::new(Vec::new())),
                removed: Arc::new(StdMutex::new(Vec::new())),
            }
        }

        fn stopped(&self) -> Vec<String> {
            self.stopped.lock().expect("stopped lock").clone()
        }

        fn removed(&self) -> Vec<String> {
            self.removed.lock().expect("removed lock").clone()
        }
    }

    #[async_trait]
    impl DockerRecoveryBackend for MockDockerRecoveryBackend {
        async fn inspect_container(
            &self,
            container_ref: &str,
        ) -> Result<Option<InspectedContainer>> {
            Ok(self
                .inspected
                .lock()
                .expect("inspected lock")
                .get(container_ref)
                .cloned())
        }

        async fn list_managed_containers(&self) -> Result<Vec<String>> {
            Ok(self.managed.lock().expect("managed lock").clone())
        }

        async fn stop_container(&self, container_id: &str) -> Result<()> {
            self.stopped
                .lock()
                .expect("stopped lock")
                .push(container_id.to_string());
            Ok(())
        }

        async fn remove_container(&self, container_id: &str) -> Result<()> {
            self.removed
                .lock()
                .expect("removed lock")
                .push(container_id.to_string());
            Ok(())
        }
    }

    fn create_running_wave_and_run(
        store: &SharedStore,
        repo: &Path,
        name: &str,
    ) -> (Wave, WaveRun) {
        let wave = Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: repo.to_string_lossy().to_string(),
            flow: "test-flow".to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Running,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.create_wave(&wave).expect("wave should be created");

        let run = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: repo.to_string_lossy().to_string(),
                flow: "test-flow".to_string(),
                direction: vec![],
                area: vec![],
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: repo.to_string_lossy().to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
        };
        store
            .create_wave_run(&run)
            .expect("wave run should be created");
        (wave, run)
    }

    fn make_running_agent(run: &WaveRun, container_id: Option<&str>, name: &str) -> Agent {
        Agent {
            id: LfdId::new(),
            step: name.to_string(),
            repo: run.snapshot.repo.clone(),
            worktree: run.worktree.clone(),
            wave_run_id: Some(run.id.clone()),
            status: AgentStatus::Running,
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            pid: None,
            container_id: container_id.map(str::to_string),
            model: "claude-code".to_string(),
            run_mode: "auto".to_string(),
        }
    }

    #[test]
    fn docker_agent_labels_include_rehydration_metadata() {
        let labels = DockerExecutor::build_agent_labels("agent-1", "wave-1", "run-1");
        assert_eq!(
            labels.get("io.loopflow.managed").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            labels.get("io.loopflow.agent-id").map(String::as_str),
            Some("agent-1")
        );
        assert_eq!(
            labels.get("io.loopflow.wave-id").map(String::as_str),
            Some("wave-1")
        );
        assert_eq!(
            labels.get("io.loopflow.wave-run-id").map(String::as_str),
            Some("run-1")
        );
    }

    #[tokio::test]
    async fn docker_startup_rehydrates_running_agents_and_cleans_orphans() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(SqliteStore::new(&db_path).expect("db"));

        let (rehydrated_wave, rehydrated_run) =
            create_running_wave_and_run(&store, tmp.path(), "rehydrated-wave");
        let (lost_wave, lost_run) = create_running_wave_and_run(&store, tmp.path(), "lost-wave");

        let rehydrated_agent =
            make_running_agent(&rehydrated_run, Some("container-live"), "step-a");
        let lost_agent = make_running_agent(&lost_run, Some("container-missing"), "step-b");
        store
            .start_agent(&rehydrated_agent)
            .expect("rehydrated agent should start");
        store
            .start_agent(&lost_agent)
            .expect("lost agent should start");

        let config = ExecutorConfig {
            r#type: ExecutorType::Docker,
            image: "loopflow/agent:test".to_string(),
            credentials: Default::default(),
        };
        let executor = DockerExecutor::new(store.clone(), &config).expect("executor");
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).expect("output dir");
        let output = OutputHub::new(16, output_dir);

        let backend = MockDockerRecoveryBackend::new(
            HashMap::from([(
                "container-live".to_string(),
                InspectedContainer {
                    id: "container-live".to_string(),
                    running: true,
                },
            )]),
            vec!["container-live".to_string(), "container-orphan".to_string()],
        );

        let recovery = executor
            .recover_startup_with_backend(&backend, &output, false)
            .await
            .expect("startup recovery should succeed");

        assert_eq!(recovery.rehydrated_agents, 1);
        assert_eq!(recovery.lost_agents_failed, 1);
        assert_eq!(recovery.orphaned_containers_removed, 1);

        let active = executor.active.lock().await;
        assert_eq!(
            active.get(rehydrated_agent.id.as_str()).map(String::as_str),
            Some("container-live")
        );
        assert!(active.get(lost_agent.id.as_str()).is_none());
        drop(active);

        let lost_agent_after = store
            .get_agent(&lost_agent.id)
            .expect("get lost agent")
            .expect("lost agent exists");
        assert_eq!(lost_agent_after.status, AgentStatus::Failed);
        assert!(lost_agent_after.ended_at.is_some());

        let lost_run_after = store
            .get_wave_run(&lost_run.id)
            .expect("get lost run")
            .expect("lost run exists");
        assert_eq!(lost_run_after.status, WaveRunStatus::Failed);
        assert_eq!(
            lost_run_after.error.as_deref(),
            Some("container lost during lfd restart.")
        );

        let lost_wave_after = store
            .get_wave(&lost_wave.id)
            .expect("get lost wave")
            .expect("lost wave exists");
        assert_eq!(lost_wave_after.status, WaveStatus::Failed);

        let rehydrated_run_after = store
            .get_wave_run(&rehydrated_run.id)
            .expect("get rehydrated run")
            .expect("rehydrated run exists");
        assert_eq!(rehydrated_run_after.status, WaveRunStatus::Running);

        let rehydrated_wave_after = store
            .get_wave(&rehydrated_wave.id)
            .expect("get rehydrated wave")
            .expect("rehydrated wave exists");
        assert_eq!(rehydrated_wave_after.status, WaveStatus::Running);

        assert_eq!(backend.stopped(), vec!["container-orphan".to_string()]);
        assert_eq!(backend.removed(), vec!["container-orphan".to_string()]);
    }

    struct MockRunner;

    #[async_trait]
    impl AgentExecutor for MockRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            _cwd: &Path,
            _wave_id: &str,
            _agent_id: &str,
            _wave_run_id: &str,
            _output: &OutputHub,
        ) -> Result<i32> {
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn execute_emits_wave_updated_on_step_advance() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path();

        // Create a two-step flow
        let flow_dir = repo.join(".lf/flows");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::write(flow_dir.join("test-flow.yaml"), "- step-a\n- step-b\n").unwrap();

        // Create step files so load_step resolves
        let step_dir = repo.join(".lf/steps");
        std::fs::create_dir_all(&step_dir).unwrap();
        std::fs::write(step_dir.join("step-a.md"), "do step a").unwrap();
        std::fs::write(step_dir.join("step-b.md"), "do step b").unwrap();

        // Set up store
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(SqliteStore::new(&db_path).unwrap());

        let wave_id = LfdId::new();
        let run_id = LfdId::new();

        let wave = Wave {
            id: wave_id.clone(),
            name: "test-wave".to_string(),
            repo: repo.to_string_lossy().to_string(),
            flow: "test-flow".to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Running,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.create_wave(&wave).unwrap();

        let run = WaveRun {
            id: run_id.clone(),
            wave_id: wave_id.clone(),
            snapshot: WaveRunSnapshot {
                repo: repo.to_string_lossy().to_string(),
                flow: "test-flow".to_string(),
                direction: vec![],
                area: vec![],
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: repo.to_string_lossy().to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
        };
        store.create_wave_run(&run).unwrap();

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let mut rx = event_hub.subscribe();

        let executor = WaveExecutor::with_runner(
            store.clone(),
            scheduler,
            output,
            event_hub,
            Arc::new(MockRunner),
        );

        executor.execute(&run_id).await.unwrap();

        // Collect all wave_updated events
        let mut wave_updated_count = 0;
        while let Ok(event) = rx.try_recv() {
            let json = serde_json::to_value(&event).unwrap();
            if json["type"] == "wave_updated" {
                wave_updated_count += 1;
            }
        }

        // Two steps means two step advances (step-a -> step-b, step-b -> complete),
        // plus one final wave_updated when the run completes.
        // After step-a: step_index 0->1, emit wave_updated
        // After step-b: step_index 1->2, emit wave_updated
        // Run completes: emit wave_updated
        assert_eq!(
            wave_updated_count, 3,
            "expected wave_updated after each step advance and on completion"
        );
    }

    #[tokio::test]
    async fn execute_fails_fork_all_with_docker_executor() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path();

        let flow_dir = repo.join(".lf/flows");
        std::fs::create_dir_all(&flow_dir).expect("flow dir should exist");
        std::fs::write(
            flow_dir.join("fork-flow.yaml"),
            r#"
- fork:
    branches:
      - step: { name: step-a }
      - step: { name: step-b }
    select: all
"#,
        )
        .expect("flow file should be written");

        let step_dir = repo.join(".lf/steps");
        std::fs::create_dir_all(&step_dir).expect("step dir should exist");
        std::fs::write(step_dir.join("step-a.md"), "do step a").expect("step file should write");
        std::fs::write(step_dir.join("step-b.md"), "do step b").expect("step file should write");

        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(SqliteStore::new(&db_path).expect("db should open"));

        let wave_id = LfdId::new();
        let run_id = LfdId::new();
        let wave = Wave {
            id: wave_id.clone(),
            name: "fork-wave".to_string(),
            repo: repo.to_string_lossy().to_string(),
            flow: "fork-flow".to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Running,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.create_wave(&wave).expect("wave should be created");

        let run = WaveRun {
            id: run_id.clone(),
            wave_id: wave_id.clone(),
            snapshot: WaveRunSnapshot {
                repo: repo.to_string_lossy().to_string(),
                flow: "fork-flow".to_string(),
                direction: vec![],
                area: vec![],
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: repo.to_string_lossy().to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
        };
        store
            .create_wave_run(&run)
            .expect("wave run should be created");

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let executor = WaveExecutor {
            store: store.clone(),
            scheduler,
            output,
            runner: Arc::new(MockRunner),
            event_hub,
            executor_type: ExecutorType::Docker,
        };

        executor
            .execute(&run_id)
            .await
            .expect("execution should finish");

        let updated_run = store
            .get_wave_run(&run_id)
            .expect("run fetch should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, WaveRunStatus::Failed);
        assert_eq!(
            updated_run.error.as_deref(),
            Some("fork(select=all) is not supported by the docker executor yet")
        );
    }
}
