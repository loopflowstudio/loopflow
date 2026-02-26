use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use anyhow::Result;
use async_trait::async_trait;
use bollard::models::MountTypeEnum;
use bytes::Bytes;
use tempfile::tempdir;
use time::OffsetDateTime;

use crate::lfd::config::{CredentialMount, ExecutorConfig, ExecutorLimitsConfig, ExecutorType};
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::store::{open_store, SharedStore, StorageConfig};
use crate::lfd::types::{
    AgentRun, AgentStatus, Wave, WaveRun, WaveRunSnapshot, WaveRunStatus, WaveStatus,
};

use super::{
    container_host_config, normalize_repo_url, DockerCredentialMount, DockerExecutor,
    DockerRecoveryBackend, InspectedContainer, RepoIdentity, RepoMutationLocks, RepoVolumeIdentity,
    CONTAINER_PREFIX_AGENT, CONTAINER_PREFIX_PREP, CONTAINER_WORKSPACE, LABEL_AGENT_ID, LABEL_KIND,
    LABEL_KIND_REPO_VOLUME, LABEL_MANAGED, LABEL_WAVE_ID, LABEL_WAVE_RUN_ID, VOLUME_PREFIX,
};

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

    let gh_mount = DockerCredentialMount::from_config(
        &CredentialMount::try_from("gh".to_string()).expect("gh mount should parse"),
    )
    .expect("gh mount should resolve");
    assert_eq!(gh_mount.len(), 1);
    assert!(gh_mount[0].host_path.ends_with(".config/gh"));
    assert_eq!(gh_mount[0].container_path, "/home/agent/.config/gh");
    assert!(gh_mount[0].read_only);
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
    let mounts = DockerExecutor::build_mounts_for(&format!("{VOLUME_PREFIX}abc"), &[]);
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].typ, Some(MountTypeEnum::VOLUME));
    assert_eq!(mounts[0].source, Some(format!("{VOLUME_PREFIX}abc")));
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
fn resolve_workspace_branch_prefers_explicit_context_branch() {
    let cwd = Path::new("/tmp/repo.wave-fork-2");
    let branch = DockerExecutor::resolve_workspace_branch(cwd, Some("feature-branch"), "main");
    assert_eq!(branch, "feature-branch");
}

#[test]
fn resolve_workspace_branch_uses_fallback_without_context_or_git_branch() {
    let cwd = Path::new("/tmp/repo.wave-fork-2");
    let branch = DockerExecutor::resolve_workspace_branch(cwd, None, "main");
    assert_eq!(branch, "main");
}

#[test]
fn resolve_workspace_branch_for_recovery_infers_fork_branch_from_path() {
    let cwd = Path::new("/tmp/repo.wave-fork-2");
    let branch = DockerExecutor::resolve_workspace_branch_for_recovery(cwd, "run-123", "main");
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
    std::fs::write(repo.path().join(".dockerignore"), ".lf/\n*.log\n").expect("write dockerignore");
    std::fs::create_dir_all(repo.path().join(".lf")).expect("create .lf");
    std::fs::write(repo.path().join(".lf/Dockerfile"), "FROM scratch\n").expect("write dockerfile");
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
    assert!(first.volume_name.starts_with(VOLUME_PREFIX));
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
    async fn inspect_container(&self, container_ref: &str) -> Result<Option<InspectedContainer>> {
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
        created_at: Some(OffsetDateTime::now_utc()),
        serialized: false,
    };
    store
        .create_wave(&wave)
        .await
        .expect("wave should be created");

    let run = WaveRun {
        id: LfdId::new(),
        wave_id: wave.id().clone(),
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
        activation_log_id: None,
        parent_run_id: None,
        parent_pr_number: None,
        stack_position: 0,
        stack_group_id: wave.id().to_string(),
        stack_status: crate::lfd::types::WaveRunStackStatus::Active,
        lineage_inferred: false,
    };
    store
        .create_wave_run(&run)
        .await
        .expect("wave run should be created");
    (wave, run)
}

fn make_running_agent(run: &WaveRun, container_id: Option<&str>, name: &str) -> AgentRun {
    AgentRun {
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
        agent: "claude-code".to_string(),
        run_mode: "auto".to_string(),
    }
}

#[test]
fn docker_agent_labels_include_rehydration_metadata() {
    let labels = DockerExecutor::build_agent_labels("agent-1", "wave-1", "run-1");
    assert_eq!(labels.get(LABEL_MANAGED).map(String::as_str), Some("true"));
    assert_eq!(
        labels.get(LABEL_AGENT_ID).map(String::as_str),
        Some("agent-1")
    );
    assert_eq!(
        labels.get(LABEL_WAVE_ID).map(String::as_str),
        Some("wave-1")
    );
    assert_eq!(
        labels.get(LABEL_WAVE_RUN_ID).map(String::as_str),
        Some("run-1")
    );
}

#[test]
fn docker_container_name_prefixes_match_contract_constants() {
    let container_name = DockerExecutor::build_container_name("agent_1");
    assert!(container_name.starts_with(CONTAINER_PREFIX_AGENT));
    assert_eq!(container_name, format!("{CONTAINER_PREFIX_AGENT}agent-1"));

    let helper_name = DockerExecutor::build_helper_container_name("sync-lfs");
    assert!(helper_name.starts_with(CONTAINER_PREFIX_PREP));
}

#[test]
fn managed_container_filter_uses_managed_label_contract() {
    assert_eq!(
        super::managed_label_filter(),
        format!("{LABEL_MANAGED}=true")
    );
}

#[test]
fn repo_volume_labels_use_contract_keys_and_values() {
    let labels = super::image::repo_volume_labels();
    assert_eq!(labels.get(LABEL_MANAGED).map(String::as_str), Some("true"));
    assert_eq!(
        labels.get(LABEL_KIND).map(String::as_str),
        Some(LABEL_KIND_REPO_VOLUME)
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
    let (lost_wave, lost_run) = create_running_wave_and_run(&store, tmp.path(), "lost-wave").await;

    let rehydrated_agent = make_running_agent(&rehydrated_run, Some("container-live"), "step-a");
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
    let executor = match DockerExecutor::new(store.clone(), &config) {
        Ok(executor) => executor,
        Err(err) => {
            eprintln!("skipping test: docker unavailable ({err})");
            return;
        }
    };
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
        .get_wave(lost_wave.id())
        .await
        .expect("get lost wave")
        .expect("lost wave exists");
    assert_eq!(lost_wave_after.status(), WaveStatus::Failed);

    let rehydrated_run_after = store
        .get_wave_run(&rehydrated_run.id)
        .await
        .expect("get rehydrated run")
        .expect("rehydrated run exists");
    assert_eq!(rehydrated_run_after.status, WaveRunStatus::Running);

    let rehydrated_wave_after = store
        .get_wave(rehydrated_wave.id())
        .await
        .expect("get rehydrated wave")
        .expect("rehydrated wave exists");
    assert_eq!(rehydrated_wave_after.status(), WaveStatus::Running);

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
        created_at: Some(OffsetDateTime::now_utc()),
        serialized: false,
    };
    store
        .create_wave(&wave)
        .await
        .expect("wave should be created");

    let run = WaveRun {
        id: LfdId::new(),
        wave_id: wave.id().clone(),
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
        activation_log_id: None,
        parent_run_id: None,
        parent_pr_number: None,
        stack_position: 0,
        stack_group_id: wave.id().to_string(),
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
    let executor = match DockerExecutor::new(store.clone(), &config) {
        Ok(executor) => executor,
        Err(err) => {
            eprintln!("skipping test: docker unavailable ({err})");
            return;
        }
    };
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
        .get_wave(wave.id())
        .await
        .expect("get wave")
        .expect("wave exists");
    assert_eq!(wave_after.status(), WaveStatus::Idle);
}
