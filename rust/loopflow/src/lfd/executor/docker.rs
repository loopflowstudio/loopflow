use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bollard::container::LogOutput;
use bollard::errors::Error as DockerError;
use bollard::models::{
    ContainerCreateBody, ContainerInspectResponse, HostConfig, Mount, MountTypeEnum,
    VolumeCreateOptions,
};
use bollard::query_parameters::{
    BuildImageOptions, BuilderVersion, CreateContainerOptions, CreateImageOptions,
    InspectContainerOptions, ListContainersOptions, LogsOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions, WaitContainerOptions,
};
use bollard::Docker;
use bytes::Bytes;
use futures_util::StreamExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

use time::OffsetDateTime;

use crate::engine::git::current_branch;
use crate::engine::stream::StreamParser;
use crate::lfd::config::{CredentialMount, ExecutorConfig, ExecutorLimitsConfig};
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Agent, AgentStatus, Wave, WaveRun, WaveRunStatus, WaveStatus};

use super::{handle_output_line, AgentExecutor, AgentRunContext, OutputContext, StartupRecovery};

#[derive(Clone)]
pub struct DockerExecutor {
    store: SharedStore,
    docker: Docker,
    image: String,
    agent_timeout: std::time::Duration,
    limits: ExecutorLimitsConfig,
    credential_env: Vec<String>,
    credential_mounts: Vec<DockerCredentialMount>,
    active: Arc<Mutex<HashMap<String, String>>>,
    mutation_locks: RepoMutationLocks,
    image_build_locks: RepoMutationLocks,
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
    read_only: bool,
}

impl DockerCredentialMount {
    fn from_config(mount: &CredentialMount) -> std::result::Result<Vec<Self>, String> {
        let (paths, read_only) = resolve_credential_mount(mount.name())?;
        let home = dirs::home_dir().ok_or_else(|| "home directory not available".to_string())?;
        Ok(paths
            .iter()
            .map(|relative| Self {
                host_path: home.join(relative),
                container_path: format!("/home/agent/{relative}"),
                read_only,
            })
            .collect())
    }
}

/// Map a credential mount name to paths under $HOME and a read-only flag.
/// Some agents need multiple files (e.g. Claude needs both .claude/ and .claude.json).
/// Agent config dirs are read-write (agents write debug/session data); keys are read-only.
fn resolve_credential_mount(
    name: &str,
) -> std::result::Result<(&'static [&'static str], bool), String> {
    let normalized = name.trim().to_ascii_lowercase();
    let key = normalized.strip_prefix("~/").unwrap_or(&normalized);
    match key {
        // read-write: agents write debug/session files
        "claude" | ".claude" => Ok((&[".claude", ".claude.json"], false)),
        "codex" | ".codex" => Ok((&[".codex"], false)),
        "gemini" | ".config/gemini" => Ok((&[".config/gemini"], false)),
        // read-only: keys and config
        "gitconfig" | ".gitconfig" => Ok((&[".gitconfig"], true)),
        "ssh" | ".ssh" => Ok((&[".ssh"], true)),
        "gnupg" | ".gnupg" => Ok((&[".gnupg"], true)),
        _ => Err(format!(
            "unknown credential mount '{name}'. allowed mounts: claude, codex, gemini, gitconfig, ssh, gnupg"
        )),
    }
}

const CONTAINER_WORKSPACE: &str = "/workspace";
const CONTAINER_REPOS_ROOT: &str = "/workspace/repos";
const LOCAL_REPO_MOUNT: &str = "/host-repo";
const HOST_WORKTREE_MOUNT: &str = "/host-worktree";
const AGENT_USER: &str = "agent";

/// API keys auto-forwarded to containers when present in host environment.
/// OAuth tokens live in the OS keychain and can't be mounted, so API keys
/// are the primary auth mechanism for containerized agents.
const AGENT_API_KEYS: &[&str] = &["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY"];

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
            Err(err) if is_docker_not_found(&err) => Ok(None),
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
            .list_containers(Some(ListContainersOptions {
                all: true,
                filters: Some(filters),
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
            .stop_container(container_id, Some(stop_container_options()))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) if is_docker_not_found(&err) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    async fn remove_container(&self, container_id: &str) -> Result<()> {
        match self
            .docker
            .remove_container(container_id, Some(remove_container_options()))
            .await
        {
            Ok(_) => Ok(()),
            Err(err) if is_docker_not_found(&err) => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

fn short_hash(value: &str, chars: usize) -> String {
    super::helpers::short_hash(value, chars)
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
        let credential_mounts: Vec<_> = config
            .credentials
            .mounts
            .iter()
            .flat_map(|spec| match DockerCredentialMount::from_config(spec) {
                Ok(mounts) => mounts
                    .into_iter()
                    .filter(|mount| {
                        if !mount.host_path.exists() {
                            warn!(
                                mount = %spec.name(),
                                host_path = %mount.host_path.display(),
                                "credential mount host path not found; skipping"
                            );
                            return false;
                        }
                        true
                    })
                    .collect::<Vec<_>>(),
                Err(err) => {
                    warn!(
                        mount = %spec.name(),
                        error = %err,
                        "invalid docker credential mount; skipping"
                    );
                    vec![]
                }
            })
            .collect();

        info!(
            image = %config.image,
            credential_env = ?config.credentials.env,
            credential_mounts = credential_mounts.len(),
            "docker executor initialized"
        );

        Ok(Self {
            store,
            docker,
            image: config.image.clone(),
            agent_timeout: config.agent_timeout,
            limits: config.limits.clone(),
            credential_env: config.credentials.env.clone(),
            credential_mounts,
            active: Arc::new(Mutex::new(HashMap::new())),
            mutation_locks: RepoMutationLocks::default(),
            image_build_locks: RepoMutationLocks::default(),
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
        let agents = self.store.list_agents().await?;
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

            let Some(run) = self.store.get_wave_run(&wave_run_id).await? else {
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

        let workspace = self
            .resolve_workspace(
                wave_id.as_str(),
                wave_run_id.as_str(),
                Path::new(&agent.worktree),
            )
            .await?;
        let exit_code = self
            .wait_for_container_with_logs(
                &container_id,
                OutputContext {
                    wave_id: wave_id.to_string(),
                    wave_run_id: wave_run_id.to_string(),
                    agent_id: agent.id.to_string(),
                    output: output.clone(),
                    output_prefix: None,
                },
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
            .end_agent(&agent.id, agent_status.as_i32(), ended_at)
            .await;

        let mut next_wave_status = None;
        if let Some(mut run) = self.store.get_wave_run(wave_run_id).await? {
            if !matches!(run.status, WaveRunStatus::Completed | WaveRunStatus::Failed) {
                run.status = run_status;
                run.ended_at = Some(OffsetDateTime::now_utc());
                run.error = run_error;
                self.store.update_wave_run(&run).await?;
                next_wave_status = Some(if run_status == WaveRunStatus::Completed {
                    WaveStatus::Idle
                } else {
                    WaveStatus::Failed
                });
            }
        }

        if let Some(wave_status) = next_wave_status {
            if let Some(mut wave) = self.store.get_wave(wave_id).await? {
                wave.status = wave_status;
                let _ = self.store.update_wave(&wave).await;
            }
        }

        Ok(())
    }

    async fn mark_agent_lost(&self, agent: &Agent) -> Result<()> {
        let ended_at = OffsetDateTime::now_utc().unix_timestamp();
        let _ = self
            .store
            .end_agent(&agent.id, AgentStatus::Failed.as_i32(), ended_at)
            .await;

        if let Some(wave_run_id) = &agent.wave_run_id {
            if let Some(mut run) = self.store.get_wave_run(wave_run_id).await? {
                let mut should_fail_wave = false;
                if !matches!(run.status, WaveRunStatus::Completed | WaveRunStatus::Failed) {
                    run.status = WaveRunStatus::Failed;
                    run.error = Some("container lost during lfd restart.".to_string());
                    run.ended_at = Some(OffsetDateTime::now_utc());
                    self.store.update_wave_run(&run).await?;
                    should_fail_wave = true;
                }

                if should_fail_wave {
                    if let Some(mut wave) = self.store.get_wave(&run.wave_id).await? {
                        wave.status = WaveStatus::Failed;
                        let _ = self.store.update_wave(&wave).await;
                    }
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
            ..Default::default()
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
        let mut env: Vec<String> = self
            .credential_env
            .iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .map(|value| format!("{name}={value}"))
            })
            .collect();

        // Auto-forward well-known agent API keys when present in host env.
        // OAuth tokens live in the macOS keychain and can't be mounted into containers,
        // so API keys are the primary auth mechanism for containerized agents.
        for key in AGENT_API_KEYS {
            if self.credential_env.iter().any(|e| e == key) {
                continue; // Already included via explicit config
            }
            if let Ok(value) = std::env::var(key) {
                env.push(format!("{key}={value}"));
            }
        }

        // When SSH credentials are mounted, build a GIT_SSH_COMMAND that:
        // - Bypasses the host SSH config (may have macOS-only options like UseKeychain)
        // - Accepts new host keys so first-contact clones succeed
        // - Explicitly lists available private keys from the mounted .ssh dir
        if let Some(ssh_mount) = self
            .credential_mounts
            .iter()
            .find(|m| m.container_path.ends_with(".ssh"))
        {
            let key_args = Self::discover_ssh_keys(&ssh_mount.host_path, &ssh_mount.container_path);
            env.push(format!(
                "GIT_SSH_COMMAND=ssh -F /dev/null \
                 -o StrictHostKeyChecking=accept-new \
                 -o UserKnownHostsFile=/tmp/.ssh_known_hosts{}",
                key_args,
            ));
        }

        env
    }

    /// Scan host SSH directory for private key files, returning `-i <path>` args
    /// using container paths.
    fn discover_ssh_keys(host_ssh_dir: &Path, container_ssh_dir: &str) -> String {
        let mut key_args = String::new();
        if let Ok(entries) = std::fs::read_dir(host_ssh_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip public keys, config, known_hosts, and other non-key files
                if name_str.ends_with(".pub")
                    || name_str.starts_with("known_hosts")
                    || name_str == "config"
                    || name_str == "authorized_keys"
                    || name_str.starts_with('.')
                {
                    continue;
                }
                // Only include regular files
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    key_args.push_str(&format!(" -i {container_ssh_dir}/{name_str}"));
                }
            }
        }
        key_args
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
                read_only: Some(credential_mount.read_only),
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
        if let Err(err) = self
            .docker
            .remove_container(container_id, Some(remove_container_options()))
            .await
        {
            warn!(container_id, error = %err, "failed to remove container");
        }
    }

    async fn wait_for_container_with_logs(
        &self,
        container_id: &str,
        context: OutputContext,
    ) -> Result<i32> {
        let logs_task = tokio::spawn(Self::stream_logs(
            self.docker.clone(),
            container_id.to_string(),
            context,
        ));

        let mut wait_stream = self
            .docker
            .wait_container(container_id, None::<WaitContainerOptions>);
        let wait_result = tokio::time::timeout(self.agent_timeout, wait_stream.next()).await;

        match wait_result {
            Ok(Some(result)) => {
                let tail_lines = logs_task.await.unwrap_or_default();
                match result {
                    Ok(status) => {
                        let code = status.status_code as i32;
                        if code != 0 && !tail_lines.is_empty() {
                            let tail = tail_lines.join("\n");
                            warn!(
                                container_id,
                                exit_code = code,
                                tail = %tail,
                                "agent container exited with non-zero code"
                            );
                        }
                        Ok(code)
                    }
                    Err(err) => {
                        // Log tail output for diagnostics
                        if !tail_lines.is_empty() {
                            let tail = tail_lines.join("\n");
                            warn!(
                                container_id,
                                tail = %tail,
                                "agent container output before failure"
                            );
                        }
                        // Inspect the container for more context on failure
                        let inspect = self
                            .docker
                            .inspect_container(container_id, None::<InspectContainerOptions>)
                            .await;
                        let detail = match inspect {
                            Ok(info) => {
                                let state = info.state.as_ref();
                                format!(
                                    "status={} exit_code={} error={} oom={}",
                                    state
                                        .and_then(|s| s.status.as_ref().map(|v| format!("{v:?}")))
                                        .unwrap_or_default(),
                                    state.and_then(|s| s.exit_code).unwrap_or(-1),
                                    state.and_then(|s| s.error.as_deref()).unwrap_or(""),
                                    state.and_then(|s| s.oom_killed).unwrap_or(false),
                                )
                            }
                            Err(inspect_err) => format!("(inspect failed: {inspect_err})"),
                        };
                        Err(anyhow!("Docker container wait error: {err} [{detail}]"))
                    }
                }
            }
            Ok(None) => {
                let _tail = logs_task.await;
                Err(anyhow!("docker wait stream ended without status"))
            }
            Err(_) => {
                let _ = self
                    .docker
                    .stop_container(container_id, Some(stop_container_options()))
                    .await;
                let _tail = logs_task.await;
                Err(anyhow!(
                    "agent execution timed out after {}",
                    humantime::format_duration(self.agent_timeout)
                ))
            }
        }
    }

    /// Stream container logs to the output hub, returning the last 20 lines
    /// for diagnostic logging on failure.
    async fn stream_logs(
        docker: Docker,
        container_id: String,
        context: OutputContext,
    ) -> Vec<String> {
        let mut logs = docker.logs(&container_id, Some(logs_options(true)));
        const TAIL_SIZE: usize = 20;
        let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();

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
                        if tail.len() >= TAIL_SIZE {
                            tail.pop_front();
                        }
                        tail.push_back(line.clone());
                        handle_output_line(&line, &mut parser, &context);
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
            if tail.len() >= TAIL_SIZE {
                tail.pop_front();
            }
            tail.push_back(pending.clone());
            handle_output_line(&pending, &mut parser, &context);
        }

        tail.into()
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
                    name: Some(Self::build_helper_container_name(label)),
                    ..Default::default()
                }),
                ContainerCreateBody {
                    image: Some(self.image.clone()),
                    cmd: Some(cmd),
                    working_dir,
                    env: Some(self.collect_env()),
                    user: Some(AGENT_USER.to_string()),
                    host_config: Some(container_host_config(mounts, &self.limits)),
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
            .start_container(&container_id, None::<StartContainerOptions>)
            .await
        {
            self.remove_container(&container_id).await;
            return Err(err.into());
        }

        let mut wait_stream = self
            .docker
            .wait_container(&container_id, None::<WaitContainerOptions>);
        let wait_result = wait_stream.next().await;

        let mut logs = self.docker.logs(&container_id, Some(logs_options(false)));
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

        let status = wait_result
            .ok_or_else(|| anyhow!("docker wait stream ended without status"))?
            .map_err(|err| {
                anyhow!(
                    "docker helper '{}' wait failed: {:?} output={}",
                    label,
                    err,
                    output.trim()
                )
            })?;
        if status.status_code != 0 {
            return Err(anyhow!(
                "docker helper '{}' failed (exit {}): {}",
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

        let _ = self
            .docker
            .create_volume(VolumeCreateOptions {
                name: Some(volume_name.to_string()),
                labels: Some(HashMap::from([
                    ("io.loopflow.managed".to_string(), "true".to_string()),
                    ("io.loopflow.kind".to_string(), "repo-volume".to_string()),
                ])),
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    // -- Image lifecycle ---------------------------------------------------------

    async fn image_exists(&self, image: &str) -> bool {
        self.docker.inspect_image(image).await.is_ok()
    }

    async fn pull_image(&self, image: &str) -> Result<()> {
        let options = CreateImageOptions {
            from_image: Some(image.to_string()),
            ..Default::default()
        };
        let mut stream = self.docker.create_image(Some(options), None, None);
        while let Some(result) = stream.next().await {
            result?;
        }
        Ok(())
    }

    async fn ensure_base_image(&self) -> Result<()> {
        if self.image_exists(&self.image).await {
            return Ok(());
        }

        info!(image = %self.image, "base image not found locally, pulling");
        match self.pull_image(&self.image).await {
            Ok(()) => {
                info!(image = %self.image, "base image pulled");
                Ok(())
            }
            Err(err) => Err(anyhow!(
                "base image '{}' not found and pull failed: {}. \
                 Build it with: docker build -t {} docker/agent/",
                self.image,
                err,
                self.image
            )),
        }
    }

    async fn repo_image_needs_build(&self, image: &str, stale_marker: &Path) -> bool {
        !self.image_exists(image).await || stale_marker.exists()
    }

    fn build_context_dockerignore(repo_source: &Path) -> ignore::gitignore::Gitignore {
        let mut builder = ignore::gitignore::GitignoreBuilder::new(repo_source);
        let _ = builder.add(repo_source.join(".dockerignore"));
        match builder.build() {
            Ok(ignore) => ignore,
            Err(_) => ignore::gitignore::Gitignore::empty(),
        }
    }

    fn list_context_paths(repo_source: &Path) -> Vec<PathBuf> {
        let dockerignore = Self::build_context_dockerignore(repo_source);
        let walker = ignore::WalkBuilder::new(repo_source)
            .hidden(false)
            .standard_filters(false)
            .build();

        let mut paths = Vec::new();
        for entry in walker.flatten() {
            let path = entry.path();
            if path == repo_source {
                continue;
            }
            let is_dir = path.is_dir();
            if dockerignore
                .matched_path_or_any_parents(path, is_dir)
                .is_ignore()
            {
                continue;
            }
            let rel = match path.strip_prefix(repo_source) {
                Ok(rel) if !rel.as_os_str().is_empty() => rel.to_path_buf(),
                _ => continue,
            };
            paths.push(rel);
        }
        paths.sort();
        paths
    }

    fn build_image_context(repo_source: &Path) -> Result<Bytes> {
        let dockerfile_rel = PathBuf::from(".lf/Dockerfile");
        let mut included = Self::list_context_paths(repo_source);
        if repo_source.join(&dockerfile_rel).exists() && !included.contains(&dockerfile_rel) {
            included.push(dockerfile_rel);
            included.sort();
        }

        let mut archive = Vec::new();
        {
            let mut tar = tar::Builder::new(&mut archive);
            for rel in included {
                let abs = repo_source.join(&rel);
                if abs.is_dir() {
                    tar.append_dir(&rel, &abs)?;
                } else {
                    tar.append_path_with_name(&abs, &rel)?;
                }
            }
            tar.finish()?;
        }
        Ok(Bytes::from(archive))
    }

    async fn build_repo_image(&self, repo_source: &Path, tag: &str) -> Result<()> {
        let context = Self::build_image_context(repo_source)?;
        let options = BuildImageOptions {
            dockerfile: ".lf/Dockerfile".to_string(),
            t: Some(tag.to_string()),
            rm: true,
            forcerm: true,
            version: BuilderVersion::BuilderBuildKit,
            ..Default::default()
        };
        let mut stream = self
            .docker
            .build_image(options, None, Some(bollard::body_full(context)));
        let mut build_error = None;
        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(error) = info.error {
                        build_error = Some(error);
                    }
                }
                Err(err) => {
                    build_error = Some(err.to_string());
                }
            }
        }
        if let Some(error) = build_error {
            return Err(anyhow!("docker api build for '{}' failed: {}", tag, error));
        }

        Ok(())
    }

    async fn ensure_repo_image(&self, repo_source: &Path) -> Result<String> {
        self.ensure_base_image().await?;

        let dockerfile_path = repo_source.join(".lf/Dockerfile");
        if !dockerfile_path.exists() {
            return Ok(self.image.clone());
        }

        let identity = RepoIdentity::from_repo(repo_source);
        let volume_id = RepoVolumeIdentity::from_identity(&identity);
        let repo_image = format!("lfd-agent-{}:latest", volume_id.repo_key);
        let stale_marker = repo_source.join(".lf/.docker-stale");
        if !self
            .repo_image_needs_build(&repo_image, &stale_marker)
            .await
        {
            return Ok(repo_image);
        }

        // Serialize concurrent builds for the same repo image.
        let lock = self.image_build_locks.for_key(&repo_image).await;
        let _guard = lock.lock().await;

        // Re-check after acquiring lock — another wave may have built it.
        if !self
            .repo_image_needs_build(&repo_image, &stale_marker)
            .await
        {
            return Ok(repo_image);
        }

        info!(
            image = %repo_image,
            repo = %repo_source.display(),
            "building per-repo agent image"
        );
        self.build_repo_image(repo_source, &repo_image).await?;

        // Remove stale marker after successful build.
        if stale_marker.exists() {
            if let Err(err) = std::fs::remove_file(&stale_marker) {
                warn!(
                    path = %stale_marker.display(),
                    error = %err,
                    "failed to remove .docker-stale marker"
                );
            }
        }

        Ok(repo_image)
    }

    fn worktree_slug_from_host_path(host_worktree: &Path) -> String {
        let fallback = host_worktree.to_string_lossy().to_string();
        let slug = host_worktree
            .file_name()
            .and_then(|name| name.to_str())
            .map(sanitize_token)
            .unwrap_or_default();
        if slug.is_empty() {
            short_hash(&fallback, 12)
        } else {
            slug
        }
    }

    fn infer_fork_branch_from_worktree(host_worktree: &Path, wave_run_id: &str) -> Option<String> {
        let name = host_worktree.file_name()?.to_str()?;
        let index = name.rsplit_once("-fork-")?.1.parse::<u32>().ok()?;
        Some(format!("{wave_run_id}-fork-{index}"))
    }

    fn docker_workspace_for_host_worktree(
        repo_source: &Path,
        host_worktree: &Path,
        branch: &str,
    ) -> DockerWorkspace {
        let repo_identity = RepoIdentity::from_repo(repo_source);
        let volume = RepoVolumeIdentity::from_identity(&repo_identity);
        let worktree_slug = Self::worktree_slug_from_host_path(host_worktree);
        DockerWorkspace {
            container_shared_clone: format!("{CONTAINER_REPOS_ROOT}/{}/main", volume.repo_key),
            container_worktree: format!(
                "{CONTAINER_REPOS_ROOT}/{}/worktrees/{worktree_slug}",
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

    fn resolve_host_repo_from_worktree(worktree: &Path) -> PathBuf {
        crate::engine::worktrees::main_repo_root(worktree).unwrap_or_else(|_| {
            worktree
                .canonicalize()
                .unwrap_or_else(|_| worktree.to_path_buf())
        })
    }

    fn resolve_workspace_for_host_worktree(
        host_worktree: &Path,
        wave_run_id: Option<&str>,
        fallback_branch: &str,
    ) -> DockerWorkspace {
        let repo_source = Self::resolve_host_repo_from_worktree(host_worktree);
        let branch = Self::resolve_workspace_branch(host_worktree, wave_run_id, fallback_branch);
        Self::docker_workspace_for_host_worktree(&repo_source, host_worktree, &branch)
    }

    fn resolve_workspace_branch(cwd: &Path, wave_run_id: Option<&str>, fallback: &str) -> String {
        if cwd.join(".git").exists() {
            if let Ok(Some(branch)) = current_branch(cwd) {
                if !branch.trim().is_empty() && branch != "HEAD" {
                    return branch;
                }
            }
        }

        if let Some(wave_run_id) = wave_run_id {
            if let Some(branch) = Self::infer_fork_branch_from_worktree(cwd, wave_run_id) {
                return branch;
            }
        }

        if fallback.trim().is_empty() {
            "main".to_string()
        } else {
            fallback.to_string()
        }
    }

    async fn resolve_workspace(
        &self,
        wave_id: &str,
        wave_run_id: &str,
        cwd: &Path,
    ) -> Result<DockerWorkspace> {
        let wave_id = LfdId::from_raw(wave_id);
        let wave = self
            .store
            .get_wave(&wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave not found for docker run"))?;
        let run_id = LfdId::from_raw(wave_run_id);
        let run = self
            .store
            .get_wave_run(&run_id)
            .await?
            .ok_or_else(|| anyhow!("wave run not found for docker run"))?;
        let repo_source = Self::resolve_host_repo(&run.snapshot.repo);
        let fallback_branch = Self::resolve_wave_run_branch(&run, &wave);
        let branch = Self::resolve_workspace_branch(cwd, Some(wave_run_id), &fallback_branch);
        Ok(Self::docker_workspace_for_host_worktree(
            &repo_source,
            cwd,
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
            "set -eu\nfind {HOST_WORKTREE_MOUNT} -mindepth 1 -maxdepth 1 ! -name .git ! -name .lf -exec rm -rf {{}} +\ntar -C '{}' --exclude=.git --exclude=.lf -cf - . | tar -C {HOST_WORKTREE_MOUNT} -xf -",
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

    /// Copy .lf/ directory from host worktree into the container volume.
    /// Prompt and context files are written to the host before the executor runs;
    /// this step ensures they're available inside the container.
    async fn sync_lf_to_volume(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
    ) -> Result<()> {
        let host_lf = host_worktree.join(".lf");
        if !host_lf.exists() {
            return Ok(());
        }
        let script = format!(
            "set -eu\nrm -rf '{0}/.lf'\ncp -a {HOST_WORKTREE_MOUNT}/.lf '{0}/.lf'",
            workspace.container_worktree
        );
        self.run_helper_command(
            "sync-lf",
            vec!["sh".to_string(), "-lc".to_string(), script],
            self.helper_mounts(
                workspace,
                vec![Self::bind_mount(host_worktree, HOST_WORKTREE_MOUNT, true)],
            ),
            None,
        )
        .await?;
        Ok(())
    }

    async fn prepare_workspace(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
    ) -> Result<()> {
        self.ensure_volume(&workspace.volume.volume_name).await?;

        let should_hygiene = {
            let mut prepared = self.prepared_runs.lock().await;
            prepared.insert(Self::prepared_workspace_key(workspace, host_worktree))
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

    fn normalize_relative_workspace_path(relative_path: &str) -> Result<PathBuf> {
        let path = Path::new(relative_path);
        if path.is_absolute() {
            return Err(anyhow!("workspace path must be relative"));
        }

        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::Normal(segment) => normalized.push(segment),
                _ => {
                    return Err(anyhow!(
                        "workspace path may not contain parent traversal components"
                    ));
                }
            }
        }

        if normalized.as_os_str().is_empty() {
            return Err(anyhow!("workspace path must not be empty"));
        }
        Ok(normalized)
    }

    async fn ensure_container_worktree(&self, workspace: &DockerWorkspace) -> Result<()> {
        self.ensure_volume(&workspace.volume.volume_name).await?;
        let lock = self
            .mutation_locks
            .for_key(&workspace.volume.repo_key)
            .await;
        let _guard = lock.lock().await;
        self.ensure_shared_clone(workspace).await?;
        self.ensure_worktree(workspace).await?;
        Ok(())
    }

    async fn write_file_to_volume(
        &self,
        workspace: &DockerWorkspace,
        host_worktree: &Path,
        relative_path: &str,
    ) -> Result<()> {
        let normalized = Self::normalize_relative_workspace_path(relative_path)?;
        let host_source = Path::new(HOST_WORKTREE_MOUNT).join(&normalized);
        let container_target = Path::new(&workspace.container_worktree).join(&normalized);
        let container_parent = container_target
            .parent()
            .ok_or_else(|| anyhow!("workspace file target has no parent"))?
            .to_string_lossy()
            .to_string();

        let helper_mounts = self.helper_mounts(
            workspace,
            vec![Self::bind_mount(host_worktree, HOST_WORKTREE_MOUNT, true)],
        );
        self.run_helper_command(
            "workspace-write-mkdir",
            vec!["mkdir".to_string(), "-p".to_string(), container_parent],
            helper_mounts.clone(),
            None,
        )
        .await?;
        self.run_helper_command(
            "workspace-write-copy",
            vec![
                "cp".to_string(),
                host_source.to_string_lossy().to_string(),
                container_target.to_string_lossy().to_string(),
            ],
            helper_mounts,
            None,
        )
        .await?;
        Ok(())
    }

    async fn remove_file_from_volume(
        &self,
        workspace: &DockerWorkspace,
        relative_path: &str,
    ) -> Result<()> {
        let normalized = Self::normalize_relative_workspace_path(relative_path)?;
        let container_target = Path::new(&workspace.container_worktree).join(&normalized);

        self.run_helper_command(
            "workspace-remove-file",
            vec![
                "rm".to_string(),
                "-f".to_string(),
                container_target.to_string_lossy().to_string(),
            ],
            self.build_mounts(&workspace.volume.volume_name),
            None,
        )
        .await?;
        Ok(())
    }

    async fn cleanup_container_worktree(&self, workspace: &DockerWorkspace) -> Result<()> {
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
            .is_git_repo(workspace, &workspace.container_shared_clone)
            .await
        {
            return Ok(());
        }

        if let Err(err) = self
            .git_command(
                workspace,
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
            .await
        {
            warn!(
                worktree = %workspace.container_worktree,
                error = %err,
                "failed removing docker fork worktree"
            );
        }

        Ok(())
    }

    fn prepared_workspace_key(workspace: &DockerWorkspace, host_worktree: &Path) -> String {
        format!(
            "{}:{}",
            workspace.volume.repo_key,
            host_worktree.to_string_lossy()
        )
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

fn is_docker_not_found(err: &DockerError) -> bool {
    matches!(
        err,
        DockerError::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

fn stop_container_options() -> StopContainerOptions {
    StopContainerOptions {
        t: Some(1),
        ..Default::default()
    }
}

fn remove_container_options() -> RemoveContainerOptions {
    RemoveContainerOptions {
        force: true,
        v: true,
        link: false,
    }
}

fn logs_options(follow: bool) -> LogsOptions {
    LogsOptions {
        follow,
        stdout: true,
        stderr: true,
        timestamps: false,
        tail: "all".to_string(),
        ..Default::default()
    }
}

fn container_host_config(mounts: Vec<Mount>, limits: &ExecutorLimitsConfig) -> HostConfig {
    HostConfig {
        mounts: Some(mounts),
        network_mode: Some("bridge".to_string()),
        privileged: Some(false),
        cap_drop: Some(vec!["ALL".to_string()]),
        memory: Some(limits.memory),
        memory_swap: Some(limits.memory_swap),
        cpu_quota: Some(limits.cpu_quota),
        pids_limit: Some(limits.pids_limit),
        security_opt: Some(vec!["no-new-privileges:true".to_string()]),
        auto_remove: Some(false),
        ..Default::default()
    }
}

#[async_trait]
impl AgentExecutor for DockerExecutor {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, context: AgentRunContext<'_>) -> Result<i32> {
        if cmd.is_empty() {
            return Err(anyhow!("empty agent command"));
        }

        let output_context: OutputContext = context.into();
        let workspace = self
            .resolve_workspace(context.wave_id, context.wave_run_id, cwd)
            .await
            .inspect_err(
                |e| warn!(agent_id = context.agent_id, error = %e, "resolve_workspace failed"),
            )?;
        let agent_image = self
            .ensure_repo_image(&workspace.repo_source)
            .await
            .inspect_err(
                |e| warn!(agent_id = context.agent_id, error = %e, "ensure_repo_image failed"),
            )?;
        self.prepare_workspace(&workspace, cwd).await.inspect_err(
            |e| warn!(agent_id = context.agent_id, error = %e, "prepare_workspace failed"),
        )?;

        // Sync .lf/ from host worktree to container volume — prompt/context files
        // are written to the host worktree before the executor is called, but
        // prepare_workspace creates the container worktree from git which doesn't
        // have these runtime artifacts.
        self.sync_lf_to_volume(&workspace, cwd).await.inspect_err(
            |e| warn!(agent_id = context.agent_id, error = %e, "sync_lf_to_volume failed"),
        )?;

        let container_name = Self::build_container_name(context.agent_id);
        let cmd = Self::rewrite_command_paths(cmd, cwd, &workspace.container_worktree);
        // Strip flags that require host-side services unavailable in containers
        let cmd: Vec<String> = cmd.into_iter().filter(|arg| arg != "--chrome").collect();
        let env = self.collect_env();
        let mounts = self.build_mounts(&workspace.volume.volume_name);
        let labels =
            Self::build_agent_labels(context.agent_id, context.wave_id, context.wave_run_id);

        info!(
            agent_id = context.agent_id,
            image = %agent_image,
            workdir = %workspace.container_worktree,
            volume = %workspace.volume.volume_name,
            cmd = ?cmd,
            "creating agent container"
        );

        let host_config = container_host_config(mounts, &self.limits);

        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Some(container_name),
                    ..Default::default()
                }),
                ContainerCreateBody {
                    image: Some(agent_image),
                    cmd: Some(cmd),
                    working_dir: Some(workspace.container_worktree.clone()),
                    env: Some(env),
                    user: Some(AGENT_USER.to_string()),
                    host_config: Some(host_config),
                    labels: Some(labels),
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    ..Default::default()
                },
            )
            .await?;

        let container_id = container.id;
        let agent_lfd_id = LfdId::from_raw(context.agent_id);
        let _ = self
            .store
            .update_agent_status(
                &agent_lfd_id,
                AgentStatus::Running.as_i32(),
                None,
                Some(&container_id),
            )
            .await;
        self.active
            .lock()
            .await
            .insert(context.agent_id.to_string(), container_id.clone());

        if let Err(err) = self
            .docker
            .start_container(&container_id, None::<StartContainerOptions>)
            .await
        {
            self.active.lock().await.remove(context.agent_id);
            self.remove_container(&container_id).await;
            return Err(err.into());
        }

        let exit_code = self
            .wait_for_container_with_logs(&container_id, output_context)
            .await;
        self.active.lock().await.remove(context.agent_id);
        self.remove_container(&container_id).await;

        self.sync_to_host_worktree(&workspace, cwd).await?;

        exit_code
    }

    async fn terminate(&self, agent_id: &str) -> Result<()> {
        let container_id = self.active.lock().await.remove(agent_id);
        if let Some(container_id) = container_id {
            let _ = self
                .docker
                .stop_container(&container_id, Some(stop_container_options()))
                .await;
            self.remove_container(&container_id).await;
        }
        Ok(())
    }

    async fn write_to_workspace(
        &self,
        cwd: &Path,
        relative_path: &str,
        content: &[u8],
    ) -> Result<()> {
        super::write_workspace_file(cwd, relative_path, content)?;

        let workspace = Self::resolve_workspace_for_host_worktree(cwd, None, "main");
        self.ensure_container_worktree(&workspace).await?;
        self.prepared_runs
            .lock()
            .await
            .insert(Self::prepared_workspace_key(&workspace, cwd));
        self.write_file_to_volume(&workspace, cwd, relative_path)
            .await?;
        Ok(())
    }

    async fn remove_from_workspace(&self, cwd: &Path, relative_path: &str) -> Result<()> {
        super::remove_workspace_file(cwd, relative_path)?;

        let workspace = Self::resolve_workspace_for_host_worktree(cwd, None, "main");
        if self
            .docker
            .inspect_volume(&workspace.volume.volume_name)
            .await
            .is_ok()
        {
            let _ = self
                .remove_file_from_volume(&workspace, relative_path)
                .await;
        }
        Ok(())
    }

    async fn cleanup_ephemeral_worktree(&self, repo: &Path, worktree: &Path) -> Result<()> {
        let repo_source =
            crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
        let workspace = Self::docker_workspace_for_host_worktree(&repo_source, worktree, "main");
        self.cleanup_container_worktree(&workspace).await?;
        super::cleanup_host_worktree(worktree)?;
        Ok(())
    }

    async fn recover_startup(&self, output: &OutputHub) -> Result<StartupRecovery> {
        let backend = BollardRecoveryBackend::new(self.docker.clone());
        self.recover_startup_with_backend(&backend, output, true)
            .await
    }

    async fn cleanup_wave(&self, wave: &Wave) -> Result<()> {
        let repo = Self::resolve_host_repo(&wave.repo);
        let host_worktree = crate::engine::worktrees::worktree_path(&repo, &wave.name);
        let workspace = Self::docker_workspace_for_host_worktree(&repo, &host_worktree, "main");
        self.cleanup_container_worktree(&workspace).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::config::{ExecutorLimitsConfig, ExecutorType};
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{WaveRunKind, WaveRunSnapshot};
    use std::io::Cursor;
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    #[test]
    fn docker_mount_spec_resolves_allowlisted_credentials() {
        let mounts = DockerCredentialMount::from_config(
            &CredentialMount::try_from("claude".to_string()).expect("claude mount should parse"),
        )
        .expect("mount spec should parse");
        // "claude" expands to both .claude/ and .claude.json
        assert_eq!(mounts.len(), 2);
        assert!(mounts[0].host_path.ends_with(".claude"));
        assert_eq!(mounts[0].container_path, "/home/agent/.claude");
        assert!(!mounts[0].read_only);
        assert!(mounts[1].host_path.ends_with(".claude.json"));
        assert_eq!(mounts[1].container_path, "/home/agent/.claude.json");
        assert!(DockerCredentialMount::from_config(
            &CredentialMount::try_from("unknown".to_string())
                .expect("unknown mount name is still valid syntax"),
        )
        .is_err());
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
    fn docker_workspace_uses_host_worktree_name() {
        let repo = Path::new("/tmp/repo");
        let host_worktree = Path::new("/tmp/repo.wave-fork-1");
        let workspace =
            DockerExecutor::docker_workspace_for_host_worktree(repo, host_worktree, "branch");
        assert!(workspace
            .container_worktree
            .ends_with("/worktrees/repo-wave-fork-1"));
    }

    #[test]
    fn resolve_workspace_branch_infers_fork_branch_from_path() {
        let cwd = Path::new("/tmp/repo.wave-fork-2");
        let branch = DockerExecutor::resolve_workspace_branch(cwd, Some("run-123"), "main");
        assert_eq!(branch, "run-123-fork-2");
    }

    #[test]
    fn normalize_relative_workspace_path_rejects_traversal() {
        assert!(DockerExecutor::normalize_relative_workspace_path("../etc/passwd").is_err());
        assert!(DockerExecutor::normalize_relative_workspace_path("/absolute/path").is_err());
        assert!(DockerExecutor::normalize_relative_workspace_path("").is_err());
        assert_eq!(
            DockerExecutor::normalize_relative_workspace_path(".lf/fork-manifest.json")
                .unwrap()
                .to_str()
                .unwrap(),
            ".lf/fork-manifest.json"
        );
        // CurDir components are stripped
        assert_eq!(
            DockerExecutor::normalize_relative_workspace_path("./file.txt")
                .unwrap()
                .to_str()
                .unwrap(),
            "file.txt"
        );
        // Parent traversal after normal component is still rejected
        assert!(DockerExecutor::normalize_relative_workspace_path("nested/../file.txt").is_err());
    }

    fn list_archive_entries(bytes: Bytes) -> Vec<String> {
        let cursor = Cursor::new(bytes);
        let mut archive = tar::Archive::new(cursor);
        let mut entries = Vec::new();
        for entry in archive.entries().expect("tar entries") {
            let entry = entry.expect("tar entry");
            let path = entry.path().expect("entry path");
            let normalized = path
                .to_string_lossy()
                .trim_start_matches("./")
                .trim_end_matches('/')
                .to_string();
            entries.push(normalized);
        }
        entries.sort();
        entries
    }

    #[test]
    fn build_image_context_respects_dockerignore_but_keeps_dockerfile() {
        let repo = tempdir().expect("tempdir");
        std::fs::write(repo.path().join(".dockerignore"), ".lf/\n*.log\n")
            .expect("write dockerignore");
        std::fs::create_dir_all(repo.path().join(".lf")).expect("create .lf");
        std::fs::write(repo.path().join(".lf/Dockerfile"), "FROM scratch\n")
            .expect("write dockerfile");
        std::fs::write(repo.path().join(".lf/secret.txt"), "ignored").expect("write ignored file");
        std::fs::write(repo.path().join("kept.txt"), "keep").expect("write kept");
        std::fs::write(repo.path().join("ignored.log"), "ignore").expect("write ignored");

        let context = DockerExecutor::build_image_context(repo.path()).expect("context");
        let entries = list_archive_entries(context);

        assert!(entries.contains(&".dockerignore".to_string()));
        assert!(entries.contains(&".lf/Dockerfile".to_string()));
        assert!(entries.contains(&"kept.txt".to_string()));
        assert!(!entries.contains(&".lf/secret.txt".to_string()));
        assert!(!entries.contains(&"ignored.log".to_string()));
    }

    #[test]
    fn docker_host_config_applies_limits_and_no_new_privileges() {
        let limits = ExecutorLimitsConfig::default();
        let host_config = container_host_config(vec![], &limits);

        assert_eq!(host_config.memory, Some(limits.memory));
        assert_eq!(host_config.memory_swap, Some(limits.memory_swap));
        assert_eq!(host_config.cpu_quota, Some(limits.cpu_quota));
        assert_eq!(host_config.pids_limit, Some(limits.pids_limit));
        assert_eq!(
            host_config.security_opt,
            Some(vec!["no-new-privileges:true".to_string()])
        );
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

    async fn create_running_wave_and_run(
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
            schema_ref: None,
            schema_name: None,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .create_wave(&wave)
            .await
            .expect("wave should be created");

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
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store
            .create_wave_run(&run)
            .await
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
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db"),
        );

        let (rehydrated_wave, rehydrated_run) =
            create_running_wave_and_run(&store, tmp.path(), "rehydrated-wave").await;
        let (lost_wave, lost_run) =
            create_running_wave_and_run(&store, tmp.path(), "lost-wave").await;

        let rehydrated_agent =
            make_running_agent(&rehydrated_run, Some("container-live"), "step-a");
        let lost_agent = make_running_agent(&lost_run, Some("container-missing"), "step-b");
        store
            .start_agent(&rehydrated_agent)
            .await
            .expect("rehydrated agent should start");
        store
            .start_agent(&lost_agent)
            .await
            .expect("lost agent should start");

        let config = ExecutorConfig {
            r#type: ExecutorType::Docker,
            image: "loopflow/agent:test".to_string(),
            credentials: Default::default(),
            agent_timeout: std::time::Duration::from_secs(45 * 60),
            limits: ExecutorLimitsConfig::default(),
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
            .await
            .expect("get lost agent")
            .expect("lost agent exists");
        assert_eq!(lost_agent_after.status, AgentStatus::Failed);
        assert!(lost_agent_after.ended_at.is_some());

        let lost_run_after = store
            .get_wave_run(&lost_run.id)
            .await
            .expect("get lost run")
            .expect("lost run exists");
        assert_eq!(lost_run_after.status, WaveRunStatus::Failed);
        assert_eq!(
            lost_run_after.error.as_deref(),
            Some("container lost during lfd restart.")
        );

        let lost_wave_after = store
            .get_wave(&lost_wave.id)
            .await
            .expect("get lost wave")
            .expect("lost wave exists");
        assert_eq!(lost_wave_after.status, WaveStatus::Failed);

        let rehydrated_run_after = store
            .get_wave_run(&rehydrated_run.id)
            .await
            .expect("get rehydrated run")
            .expect("rehydrated run exists");
        assert_eq!(rehydrated_run_after.status, WaveRunStatus::Running);

        let rehydrated_wave_after = store
            .get_wave(&rehydrated_wave.id)
            .await
            .expect("get rehydrated wave")
            .expect("rehydrated wave exists");
        assert_eq!(rehydrated_wave_after.status, WaveStatus::Running);

        assert_eq!(backend.stopped(), vec!["container-orphan".to_string()]);
        assert_eq!(backend.removed(), vec!["container-orphan".to_string()]);
    }

    #[tokio::test]
    async fn docker_startup_lost_agent_does_not_flip_terminal_run_wave_status() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db"),
        );

        let wave = Wave {
            id: LfdId::new(),
            name: "completed-wave".to_string(),
            repo: tmp.path().to_string_lossy().to_string(),
            flow: "test-flow".to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Idle,
            iteration: 0,
            schema_ref: None,
            schema_name: None,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .create_wave(&wave)
            .await
            .expect("wave should be created");

        let run = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: tmp.path().to_string_lossy().to_string(),
                flow: "test-flow".to_string(),
                direction: vec![],
                area: vec![],
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Completed,
            worktree: tmp.path().to_string_lossy().to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: Some(OffsetDateTime::now_utc()),
            error: None,
            flow_parents: vec![],
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store
            .create_wave_run(&run)
            .await
            .expect("wave run should be created");

        let stale_agent = make_running_agent(&run, Some("container-missing"), "step-a");
        store
            .start_agent(&stale_agent)
            .await
            .expect("stale agent should start");

        let config = ExecutorConfig {
            r#type: ExecutorType::Docker,
            image: "loopflow/agent:test".to_string(),
            credentials: Default::default(),
            agent_timeout: std::time::Duration::from_secs(45 * 60),
            limits: ExecutorLimitsConfig::default(),
        };
        let executor = DockerExecutor::new(store.clone(), &config).expect("executor");
        let output_dir = tmp.path().join("output");
        std::fs::create_dir_all(&output_dir).expect("output dir");
        let output = OutputHub::new(16, output_dir);

        let backend = MockDockerRecoveryBackend::new(HashMap::new(), Vec::new());

        let recovery = executor
            .recover_startup_with_backend(&backend, &output, false)
            .await
            .expect("startup recovery should succeed");

        assert_eq!(recovery.rehydrated_agents, 0);
        assert_eq!(recovery.lost_agents_failed, 1);
        assert_eq!(recovery.orphaned_containers_removed, 0);

        let agent_after = store
            .get_agent(&stale_agent.id)
            .await
            .expect("get agent")
            .expect("agent exists");
        assert_eq!(agent_after.status, AgentStatus::Failed);
        assert!(agent_after.ended_at.is_some());

        let run_after = store
            .get_wave_run(&run.id)
            .await
            .expect("get run")
            .expect("run exists");
        assert_eq!(run_after.status, WaveRunStatus::Completed);

        let wave_after = store
            .get_wave(&wave.id)
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(wave_after.status, WaveStatus::Idle);
    }
}
