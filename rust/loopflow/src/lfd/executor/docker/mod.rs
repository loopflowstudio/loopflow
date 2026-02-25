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
use crate::lfd::types::{AgentRun, AgentStatus, Wave, WaveRun, WaveRunStatus, WaveStatus};

use super::{handle_output_line, AgentExecutor, AgentRunContext, OutputContext, StartupRecovery};

mod image;
mod io;
mod recovery;
mod types;
mod workspace;

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
    agent: AgentRun,
    wave_id: LfdId,
    wave_run_id: LfdId,
    container_id: String,
}

#[derive(Debug, Clone)]
struct RehydrationPlan {
    reattach: Vec<ReattachTarget>,
    lost: Vec<AgentRun>,
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
            .resolve_workspace(context.wave_id, context.wave_run_id, cwd, context.branch)
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

        let workspace = Self::resolve_workspace_for_host_worktree(cwd);
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

        let workspace = Self::resolve_workspace_for_host_worktree(cwd);
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
        super::cleanup_workspace_worktree(worktree)?;
        Ok(())
    }

    async fn recover_startup(&self, output: &OutputHub) -> Result<StartupRecovery> {
        let backend = BollardRecoveryBackend::new(self.docker.clone());
        self.recover_startup_with_backend(&backend, output, true)
            .await
    }

    async fn cleanup_wave_workspace(&self, wave: &Wave) -> Result<()> {
        let repo = Self::resolve_host_repo(wave.repo());
        let host_worktree = crate::engine::worktrees::worktree_path(&repo, wave.name());
        let workspace = Self::docker_workspace_for_host_worktree(&repo, &host_worktree, "main");
        self.cleanup_container_worktree(&workspace).await
    }
}

#[cfg(test)]
mod tests;
