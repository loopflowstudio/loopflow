use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use loopflow::child_session::ChildProcessGeneration;
use loopflow::id::WaveId;
use loopflow::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
use loopflow::session_context::{
    LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
    ProjectLaunchReceipt, TaskLaunchReceipt,
};
use loopflow::store::{open_store, StorageConfig, Store, CONTROL_DB_PATH_ENV, CONTROL_HOME_ENV};

/// Ambient Task-Session env vars a live `lf __task` process exports. When the
/// test suite itself runs inside a Task Session, these leak in and steer
/// `task_for_worktree` at the wrong (real) session's store. Every EnvGuard
/// clears them so task-aware tests resolve by worktree path, restoring them on
/// drop.
const AMBIENT_TASK_ENV: [&str; 3] = [
    "LF_TASK_SESSION_ID",
    "LF_TASK_GENERATION",
    "LF_TASK_LEASE_TOKEN",
];
use loopflow::task::{
    PmWritebackState, TaskPr, TaskPrId, TaskSession, TaskSessionId, TaskSessionStatus,
};
use loopflow::wave::Wave;
use tempfile::TempDir;
use time::OffsetDateTime;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct HomeOverride {
    previous_lf_home: Option<OsString>,
    previous_db_path: Option<OsString>,
    previous_control_home: Option<OsString>,
    previous_control_db_path: Option<OsString>,
    _temp: TempDir,
}

impl HomeOverride {
    fn new_temp() -> Self {
        let temp = TempDir::new().expect("temp home dir");
        let previous_lf_home = env::var_os("LF_HOME");
        let previous_db_path = env::var_os("LF_DB_PATH");
        let previous_control_home = env::var_os(CONTROL_HOME_ENV);
        let previous_control_db_path = env::var_os(CONTROL_DB_PATH_ENV);
        env::remove_var("LF_HOME");
        env::remove_var("LF_DB_PATH");
        env::remove_var(CONTROL_HOME_ENV);
        env::remove_var(CONTROL_DB_PATH_ENV);
        env::set_var("LF_HOME", temp.path());
        Self {
            previous_lf_home,
            previous_db_path,
            previous_control_home,
            previous_control_db_path,
            _temp: temp,
        }
    }
}

impl Drop for HomeOverride {
    fn drop(&mut self) {
        match &self.previous_lf_home {
            Some(prev) => env::set_var("LF_HOME", prev),
            None => env::remove_var("LF_HOME"),
        }
        match &self.previous_db_path {
            Some(prev) => env::set_var("LF_DB_PATH", prev),
            None => env::remove_var("LF_DB_PATH"),
        }
        match &self.previous_control_home {
            Some(prev) => env::set_var(CONTROL_HOME_ENV, prev),
            None => env::remove_var(CONTROL_HOME_ENV),
        }
        match &self.previous_control_db_path {
            Some(prev) => env::set_var(CONTROL_DB_PATH_ENV, prev),
            None => env::remove_var(CONTROL_DB_PATH_ENV),
        }
    }
}

#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn with_clean_home<T>(f: impl FnOnce() -> T) -> T {
    let _lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let _home = HomeOverride::new_temp();
    f()
}

pub struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_path: Option<String>,
    previous_home: Option<String>,
    previous_lf_home: Option<OsString>,
    previous_db_path: Option<OsString>,
    previous_control_home: Option<OsString>,
    previous_control_db_path: Option<OsString>,
    previous_ambient_task: Vec<(&'static str, Option<OsString>)>,
    _bin: TempDir,
    _lf_home: TempDir,
}

impl EnvGuard {
    #[allow(dead_code)] // Shared helper compiled into multiple test crates.
    pub fn new(entries: &[(&str, &str)]) -> Self {
        Self::with_home(entries, None)
    }

    #[allow(dead_code)] // Shared helper used only by tests that need HOME isolation.
    pub fn with_home(entries: &[(&str, &str)], home: Option<&Path>) -> Self {
        let lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
        let bin = TempDir::new().expect("temp bin dir");
        for (name, content) in entries {
            write_executable(bin.path(), name, content);
        }
        let previous_path = env::var("PATH").ok();
        let new_path = match &previous_path {
            Some(prev) => format!("{}:{}", bin.path().display(), prev),
            None => bin.path().display().to_string(),
        };
        env::set_var("PATH", new_path);
        let previous_home = env::var("HOME").ok();
        if let Some(home) = home {
            env::set_var("HOME", home);
        }
        let previous_lf_home = env::var_os("LF_HOME");
        let previous_db_path = env::var_os("LF_DB_PATH");
        let previous_control_home = env::var_os(CONTROL_HOME_ENV);
        let previous_control_db_path = env::var_os(CONTROL_DB_PATH_ENV);
        let previous_ambient_task = AMBIENT_TASK_ENV
            .iter()
            .map(|name| {
                let prev = env::var_os(name);
                env::remove_var(name);
                (*name, prev)
            })
            .collect();
        let lf_home = TempDir::new().expect("temp lf home dir");
        env::remove_var("LF_HOME");
        env::remove_var("LF_DB_PATH");
        env::remove_var(CONTROL_HOME_ENV);
        env::remove_var(CONTROL_DB_PATH_ENV);
        if home.is_some() {
            // Keep HOME-based config discovery intact while isolating its store.
            env::set_var("LF_DB_PATH", lf_home.path().join("loopflow.db"));
        } else {
            env::set_var("LF_HOME", lf_home.path());
        }
        Self {
            _lock: lock,
            previous_path,
            previous_home,
            previous_lf_home,
            previous_db_path,
            previous_control_home,
            previous_control_db_path,
            previous_ambient_task,
            _bin: bin,
            _lf_home: lf_home,
        }
    }

    #[allow(dead_code)] // Shared helper used by tests that exercise the local registry.
    pub fn with_lf_home(entries: &[(&str, &str)], home: &Path) -> Self {
        let guard = Self::with_home(entries, None);
        env::set_var("LF_HOME", home);
        guard
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.previous_path {
            env::set_var("PATH", prev);
        } else {
            env::remove_var("PATH");
        }
        if let Some(prev) = &self.previous_home {
            env::set_var("HOME", prev);
        } else {
            env::remove_var("HOME");
        }
        match &self.previous_lf_home {
            Some(prev) => env::set_var("LF_HOME", prev),
            None => env::remove_var("LF_HOME"),
        }
        match &self.previous_db_path {
            Some(prev) => env::set_var("LF_DB_PATH", prev),
            None => env::remove_var("LF_DB_PATH"),
        }
        match &self.previous_control_home {
            Some(prev) => env::set_var(CONTROL_HOME_ENV, prev),
            None => env::remove_var(CONTROL_HOME_ENV),
        }
        match &self.previous_control_db_path {
            Some(prev) => env::set_var(CONTROL_DB_PATH_ENV, prev),
            None => env::remove_var(CONTROL_DB_PATH_ENV),
        }
        for (name, prev) in &self.previous_ambient_task {
            match prev {
                Some(prev) => env::set_var(name, prev),
                None => env::remove_var(name),
            }
        }
    }
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub struct RegisteredTask {
    pub store: Store,
    pub session: TaskSession,
    pub pr: TaskPr,
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub fn register_task(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    register_task_with_process(home, worktree, branch, base_commit, false)
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub fn register_active_task(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    register_task_with_process(home, worktree, branch, base_commit, true)
}

fn register_task_with_process(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
    active: bool,
) -> RegisteredTask {
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let store = runtime
        .block_on(open_store(&StorageConfig::sqlite(home.join("loopflow.db"))))
        .expect("open task test store");
    let now = OffsetDateTime::now_utc();
    let wave = Wave::new(
        WaveId::new(),
        "task-pr-tests".to_string(),
        worktree.display().to_string(),
    );
    let project = ProjectSession {
        id: ProjectSessionId::new(),
        launch: ProjectLaunchReceipt {
            project: LinearProjectSnapshot {
                id: LinearProjectId::new(format!("project-{}", WaveId::new())).expect("project id"),
                slug: "task-pr-tests".to_string(),
                name: "Task PR tests".to_string(),
                prompt_context: "Keep Task PR transitions durable.".to_string(),
            },
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        current_directive_version: 0,
        incorporated_directive_version: 0,
        status: ProjectSessionStatus::Running,
        status_reason: "test project is running".to_string(),
        status_at: now,
        iteration: 1,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: Some("task-pr-project".to_string()),
        latest_process: Some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "task-pr-project".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("task-pr-project".to_string()),
            started_at: now,
            state: loopflow::child_session::ChildLeaseState::Legacy,
            outcome: None,
            provenance: None,
        }),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    let session = TaskSession {
        id: TaskSessionId::new(),
        launch: TaskLaunchReceipt {
            issue: LinearIssueSnapshot {
                id: LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id"),
                identifier: "INF-123".to_string(),
                title: "Prove Task PR transitions".to_string(),
                description: "Exercise the persisted lifecycle.".to_string(),
            },
            project: project.launch.project.clone(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_session_id: project.id.clone(),
        current_directive_version: 0,
        incorporated_directive_version: 0,
        status: if active {
            TaskSessionStatus::Running
        } else {
            TaskSessionStatus::Waiting
        },
        status_reason: if active {
            "test Task is running".to_string()
        } else {
            "test Task is waiting".to_string()
        },
        status_at: now,
        worktree: worktree.to_path_buf(),
        workspace_slug: "task-pr-proof".to_string(),
        lifecycle: loopflow::task::TaskLifecyclePlan::standard("task"),
        lifecycle_phase: loopflow::task::TaskLifecyclePhase::Iterate,
        phase_epoch: 1,
        phase_cursor: 0,
        phase_iteration: 0,
        gate_cycle: 0,
        gate_proposal: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        latest_process: active.then_some(ChildProcessGeneration {
            generation: 1,
            pid: None,
            process_group_id: None,
            tmux_name: "task-github-cache".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("task-github-cache".to_string()),
            started_at: now,
            state: loopflow::child_session::ChildLeaseState::Legacy,
            outcome: None,
            provenance: None,
        }),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: loopflow::task::Observation::NotRequired,
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_session_id: session.id.clone(),
        sequence: 1,
        slug: session.workspace_slug.clone(),
        branch: branch.to_string(),
        base_commit: base_commit.to_string(),
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        created_at: now,
        updated_at: now,
        ci_observation: None,
        github_observation: None,
    };
    runtime.block_on(async {
        store.create_wave(&wave).await.expect("create test wave");
        store
            .create_project_session(&project)
            .await
            .expect("create test project");
        store
            .create_task_session(&session, &pr)
            .await
            .expect("create test Task");
    });
    RegisteredTask { store, session, pr }
}

/// A fake `open` / `xdg-open` that records each invocation to `marker`, so a
/// test can count presentation attempts through the recorded boundary. Register
/// it under both `open` and `xdg-open` so the platform opener records on either
/// OS.
#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn counting_open_script(marker: &Path) -> String {
    format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", marker.display())
}

/// Count recorded presentation attempts written by `counting_open_script`.
#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn presentation_attempts(marker: &Path) -> usize {
    std::fs::read_to_string(marker)
        .map(|log| log.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0)
}

fn write_executable(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}
