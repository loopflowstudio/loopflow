use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
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
use tokio::process::Command;
use tokio::sync::Mutex;

use tracing::{info, warn};

use crate::engine::stream::StreamParser;
use crate::lfd::config::{CredentialMount, ExecutorConfig};
use crate::lfd::executor::{handle_output_line, AgentExecutor, StartupRecovery};
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Agent, AgentStatus, Wave, WaveRun, WaveRunStatus, WaveStatus};

use time::OffsetDateTime;

#[derive(Clone)]
pub struct DockerExecutor {
    store: SharedStore,
    docker: Docker,
    base_image: String,
    credential_env: Vec<String>,
    credential_mounts: Vec<DockerCredentialMount>,
    active: Arc<Mutex<HashMap<String, String>>>,
    mutation_locks: KeyedLocks,
    image_build_locks: KeyedLocks,
    prepared_runs: Arc<Mutex<HashMap<String, Instant>>>,
}

impl std::fmt::Debug for DockerExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerExecutor")
            .field("base_image", &self.base_image)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
struct KeyedLocks {
    inner: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl KeyedLocks {
    async fn for_value(&self, value: &str) -> Arc<Mutex<()>> {
        let mut locks = self.inner.lock().await;
        locks
            .entry(value.to_string())
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
    fn from_config(mount: &CredentialMount) -> std::result::Result<Self, String> {
        let relative = resolve_credential_mount(mount.name())?;
        let home = dirs::home_dir().ok_or_else(|| "home directory not available".to_string())?;
        Ok(Self {
            host_path: home.join(relative),
            container_path: format!("/home/agent/{relative}"),
        })
    }
}

fn resolve_credential_mount(name: &str) -> std::result::Result<&'static str, String> {
    let normalized = name.trim().to_ascii_lowercase();
    let key = normalized.strip_prefix("~/").unwrap_or(&normalized);
    match key {
        "claude" | ".claude" => Ok(".claude"),
        "codex" | ".codex" => Ok(".codex"),
        "gemini" | ".config/gemini" => Ok(".config/gemini"),
        "gitconfig" | ".gitconfig" => Ok(".gitconfig"),
        "ssh" | ".ssh" => Ok(".ssh"),
        "gnupg" | ".gnupg" => Ok(".gnupg"),
        _ => Err(format!(
            "unknown credential mount '{name}'. allowed mounts: claude, codex, gemini, gitconfig, ssh, gnupg"
        )),
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
            .map(DockerCredentialMount::from_config)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|err| anyhow!("invalid docker credential mount: {err}"))?;

        Ok(Self {
            store,
            docker,
            base_image: config.image.clone(),
            credential_env: config.credentials.env.clone(),
            credential_mounts,
            active: Arc::new(Mutex::new(HashMap::new())),
            mutation_locks: KeyedLocks::default(),
            image_build_locks: KeyedLocks::default(),
            prepared_runs: Arc::new(Mutex::new(HashMap::new())),
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

        let workspace = self.resolve_workspace_for_cwd(
            wave_id.as_str(),
            wave_run_id.as_str(),
            Path::new(&agent.worktree),
        )?;
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
                let mut should_fail_wave = false;
                if !matches!(run.status, WaveRunStatus::Completed | WaveRunStatus::Failed) {
                    run.status = WaveRunStatus::Failed;
                    run.error = Some("container lost during lfd restart.".to_string());
                    run.ended_at = Some(OffsetDateTime::now_utc());
                    self.store.update_wave_run(&run)?;
                    should_fail_wave = true;
                }

                if should_fail_wave {
                    if let Some(mut wave) = self.store.get_wave(&run.wave_id)? {
                        wave.status = WaveStatus::Failed;
                        let _ = self.store.update_wave(&wave);
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

    fn repo_image_tag(&self, workspace: &DockerWorkspace) -> String {
        format!("lfd-agent-{}:latest", workspace.volume.repo_key)
    }

    fn dockerfile_path(repo_source: &Path) -> PathBuf {
        repo_source.join(".lf").join("Dockerfile")
    }

    fn env_setup_path(repo_source: &Path) -> PathBuf {
        repo_source.join(".lf").join("env-setup.sh")
    }

    fn stale_sentinel_path(repo_source: &Path) -> PathBuf {
        repo_source.join(".lf").join(".docker-stale")
    }

    fn default_dockerfile(&self) -> String {
        format!(
            "FROM {}\n\n# Loopflow baseline setup (provided by loopflow/agent images)\nRUN if command -v install-loopflow.sh >/dev/null 2>&1; then install-loopflow.sh --install; else echo 'install-loopflow.sh not found in base image; skipping loopflow base setup'; fi\n\n# Project-specific setup\nCOPY .lf/env-setup.sh /tmp/env-setup.sh\nRUN if [ -f /tmp/env-setup.sh ]; then sh /tmp/env-setup.sh --install; fi\n\nWORKDIR /workspace\n",
            self.base_image
        )
    }

    fn ensure_repo_dockerfile(&self, repo_source: &Path) -> Result<PathBuf> {
        let dockerfile_path = Self::dockerfile_path(repo_source);
        if dockerfile_path.exists() {
            return Ok(dockerfile_path);
        }

        let lf_dir = repo_source.join(".lf");
        std::fs::create_dir_all(&lf_dir)?;
        std::fs::write(&dockerfile_path, self.default_dockerfile())?;
        info!(
            path = %dockerfile_path.display(),
            "generated default .lf/Dockerfile for docker executor"
        );
        Ok(dockerfile_path)
    }

    fn parse_from_ref(dockerfile: &str) -> Option<String> {
        dockerfile.lines().map(str::trim).find_map(|line| {
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let mut parts = line.split_whitespace();
            let keyword = parts.next()?;
            if !keyword.eq_ignore_ascii_case("from") {
                return None;
            }
            parts.next().map(str::to_string)
        })
    }

    fn build_fingerprint(dockerfile: &str, env_setup: &str, from_ref: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(dockerfile.as_bytes());
        hasher.update(b"\n--env-setup--\n");
        hasher.update(env_setup.as_bytes());
        hasher.update(b"\n--from--\n");
        hasher.update(from_ref.as_bytes());
        hex::encode(hasher.finalize())
    }

    async fn inspect_image_fingerprint(&self, image_tag: &str) -> Result<Option<String>> {
        match self.docker.inspect_image(image_tag).await {
            Ok(details) => Ok(details
                .config
                .and_then(|config| config.labels)
                .and_then(|labels| labels.get("io.loopflow.build-fingerprint").cloned())),
            Err(DockerError::DockerResponseServerError {
                status_code: 404, ..
            }) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    async fn build_repo_image(
        &self,
        image_tag: &str,
        dockerfile_path: &Path,
        repo_source: &Path,
        fingerprint: &str,
    ) -> Result<()> {
        let output = Command::new("docker")
            .arg("build")
            .arg("-t")
            .arg(image_tag)
            .arg("--label")
            .arg(format!("io.loopflow.build-fingerprint={fingerprint}"))
            .arg("-f")
            .arg(dockerfile_path)
            .arg(repo_source)
            .output()
            .await
            .with_context(|| {
                format!(
                    "failed to run `docker build` for {image_tag}; ensure Docker CLI is installed and available in PATH"
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(anyhow!(
                "docker build failed for {image_tag}: {}\n{}",
                stderr.trim(),
                stdout.trim()
            ));
        }
        Ok(())
    }

    async fn ensure_repo_image(&self, workspace: &DockerWorkspace) -> Result<String> {
        let image_tag = self.repo_image_tag(workspace);
        let lock = self.image_build_locks.for_value(&image_tag).await;
        let _guard = lock.lock().await;

        let dockerfile_path = self.ensure_repo_dockerfile(&workspace.repo_source)?;
        let dockerfile = std::fs::read_to_string(&dockerfile_path)?;
        let env_setup_path = Self::env_setup_path(&workspace.repo_source);
        let env_setup = if env_setup_path.exists() {
            std::fs::read_to_string(env_setup_path)?
        } else {
            String::new()
        };

        let from_ref = Self::parse_from_ref(&dockerfile).unwrap_or_else(|| self.base_image.clone());
        let fingerprint = Self::build_fingerprint(&dockerfile, &env_setup, &from_ref);
        let stale_sentinel = Self::stale_sentinel_path(&workspace.repo_source);
        let stale_requested = stale_sentinel.exists();
        let existing_fingerprint = self.inspect_image_fingerprint(&image_tag).await?;

        let should_rebuild = stale_requested
            || existing_fingerprint
                .as_deref()
                .map(|existing| existing != fingerprint)
                .unwrap_or(true);

        if should_rebuild {
            info!(
                image = %image_tag,
                stale_requested,
                "building docker agent image for repo"
            );
            self.build_repo_image(
                &image_tag,
                &dockerfile_path,
                &workspace.repo_source,
                &fingerprint,
            )
            .await?;
            if stale_requested {
                let _ = std::fs::remove_file(&stale_sentinel);
            }
        }

        Ok(image_tag)
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
                    image: Some(self.base_image.clone()),
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

    fn resolve_workspace_for_cwd(
        &self,
        wave_id: &str,
        wave_run_id: &str,
        cwd: &Path,
    ) -> Result<DockerWorkspace> {
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
        let mut workspace = Self::docker_workspace_for_wave(&repo_source, &wave.name, &branch);

        let run_worktree = run.worktree.trim_end_matches('/');
        let cwd_string = cwd.to_string_lossy();
        let cwd_trimmed = cwd_string.trim_end_matches('/');
        if let Some(suffix) = cwd_trimmed.strip_prefix(run_worktree) {
            if !suffix.is_empty() {
                workspace.container_worktree =
                    format!("{}{}", workspace.container_worktree, suffix);
                if let Some(index) = suffix
                    .strip_prefix("-fork-")
                    .and_then(|value| value.parse::<u32>().ok())
                {
                    workspace.branch = format!("{}-fork-{index}", run.id);
                }
            }
        }

        Ok(workspace)
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

        let prepared_key = format!("{wave_run_id}:{}", workspace.container_worktree);
        let should_hygiene = {
            let mut prepared = self.prepared_runs.lock().await;
            let now = Instant::now();
            let stale = prepared
                .get(&prepared_key)
                .map(|ts| now.duration_since(*ts).as_secs() > 300)
                .unwrap_or(true);
            prepared.insert(prepared_key, now);
            stale
        };

        let lock = self
            .mutation_locks
            .for_value(&workspace.volume.repo_key)
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

        let workspace = self.resolve_workspace_for_cwd(wave_id, wave_run_id, cwd)?;
        self.prepare_workspace(&workspace, wave_run_id, cwd).await?;
        let image_tag = self.ensure_repo_image(&workspace).await?;

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
                    image: Some(image_tag),
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
            .for_value(&workspace.volume.repo_key)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::config::ExecutorType;
    use crate::lfd::store::sqlite::SqliteStore;
    use crate::lfd::types::WaveRunSnapshot;
    use std::sync::Mutex as StdMutex;
    use tempfile::tempdir;

    #[test]
    fn docker_mount_spec_resolves_allowlisted_credentials() {
        let home = dirs::home_dir().expect("home directory should be available");
        let mount = DockerCredentialMount::from_config(
            &CredentialMount::try_from("claude".to_string()).expect("claude mount should parse"),
        )
        .expect("mount spec should parse");
        assert_eq!(mount.host_path, home.join(".claude"));
        assert_eq!(mount.container_path, "/home/agent/.claude");
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
        let locks = KeyedLocks::default();
        let events = Arc::new(StdMutex::new(Vec::new()));

        let lock_a = locks.for_value("repo-1").await;
        let events_a = events.clone();
        let first = tokio::spawn(async move {
            let _guard = lock_a.lock().await;
            events_a.lock().expect("lock events").push("first-start");
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            events_a.lock().expect("lock events").push("first-end");
        });

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let lock_b = locks.for_value("repo-1").await;
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

    #[tokio::test]
    async fn docker_startup_lost_agent_does_not_flip_terminal_run_wave_status() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(SqliteStore::new(&db_path).expect("db"));

        let wave = Wave {
            id: LfdId::new(),
            name: "completed-wave".to_string(),
            repo: tmp.path().to_string_lossy().to_string(),
            flow: "test-flow".to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.create_wave(&wave).expect("wave should be created");

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
        };
        store
            .create_wave_run(&run)
            .expect("wave run should be created");

        let stale_agent = make_running_agent(&run, Some("container-missing"), "step-a");
        store
            .start_agent(&stale_agent)
            .expect("stale agent should start");

        let config = ExecutorConfig {
            r#type: ExecutorType::Docker,
            image: "loopflow/agent:test".to_string(),
            credentials: Default::default(),
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
            .expect("get agent")
            .expect("agent exists");
        assert_eq!(agent_after.status, AgentStatus::Failed);
        assert!(agent_after.ended_at.is_some());

        let run_after = store
            .get_wave_run(&run.id)
            .expect("get run")
            .expect("run exists");
        assert_eq!(run_after.status, WaveRunStatus::Completed);

        let wave_after = store
            .get_wave(&wave.id)
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(wave_after.status, WaveStatus::Idle);
    }
}
