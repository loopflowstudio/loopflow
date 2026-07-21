use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use loopflow::id::WaveId;
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::project::{Project, ProjectId};
use loopflow::store::{open_store, StorageConfig, Store, CONTROL_DB_PATH_ENV, CONTROL_HOME_ENV};
use loopflow::task::{PmWritebackState, Task, TaskId, TaskPr, TaskPrId};
use loopflow::wave::Wave;
use tempfile::TempDir;
use time::OffsetDateTime;

/// Ambient authority a live agent process exports. Tests must never inherit the
/// real Run that invoked the suite.
const AMBIENT_AGENT_ENV: [&str; 4] = [
    "LF_RUN_CONTEXT",
    "LF_RUN_LEASE",
    "LF_WAVE_ID",
    "LF_ACCOUNT_LEASE",
];

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
    previous_ambient_authority: Vec<(&'static str, Option<OsString>)>,
    _bin: TempDir,
    _lf_home: TempDir,
}

impl EnvGuard {
    #[allow(dead_code)] // Shared helper compiled into multiple test crates.
    pub fn new(entries: &[(&str, &str)]) -> Self {
        Self::_with_home_and_path(entries, None, true)
    }

    #[allow(dead_code)] // Shared helper used by tests that require PATH isolation.
    pub fn new_isolated(entries: &[(&str, &str)]) -> Self {
        Self::_with_home_and_path(entries, None, false)
    }

    #[allow(dead_code)] // Shared helper used only by tests that need HOME isolation.
    pub fn with_home(entries: &[(&str, &str)], home: Option<&Path>) -> Self {
        Self::_with_home_and_path(entries, home, true)
    }

    fn _with_home_and_path(
        entries: &[(&str, &str)],
        home: Option<&Path>,
        include_existing_path: bool,
    ) -> Self {
        let lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
        let bin = TempDir::new().expect("temp bin dir");
        for (name, content) in entries {
            write_executable(bin.path(), name, content);
        }
        let previous_path = env::var("PATH").ok();
        let new_path = match (&previous_path, include_existing_path) {
            (Some(prev), true) => format!("{}:{}", bin.path().display(), prev),
            _ => bin.path().display().to_string(),
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
        let previous_ambient_authority = AMBIENT_AGENT_ENV
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
            previous_ambient_authority,
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
        for (name, prev) in &self.previous_ambient_authority {
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
    pub task: Task,
    pub pr: TaskPr,
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub fn register_task(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    register_task_with_process(home, worktree, branch, base_commit, true, false)
}

#[allow(dead_code)] // Shared helper compiled into integration tests without this incident shape.
pub fn register_unrun_task(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    register_task_with_process(home, worktree, branch, base_commit, false, false)
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub fn register_active_task(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    register_task_with_process(home, worktree, branch, base_commit, true, true)
}

fn register_task_with_process(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
    completed_boundary: bool,
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
    let project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new(format!("project-{}", WaveId::new())).expect("project id"),
            slug: "task-pr-tests".to_string(),
            name: "Task PR tests".to_string(),
            prompt_context: "Keep Task PR transitions durable.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        iteration: 1,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: Some("task-pr-project".to_string()),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    let task = Task {
        id: TaskId::new(),
        plan: TaskPlan {
            id: LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id"),
            identifier: "INF-123".to_string(),
            title: "Prove Task PR transitions".to_string(),
            description: "Exercise the persisted lifecycle.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: project.id.clone(),
        worktree: worktree.to_path_buf(),
        workspace_slug: "task-pr-proof".to_string(),
        lifecycle: loopflow::task::TaskLifecyclePlan::defaults(),
        lifecycle_phase: loopflow::task::TaskLifecyclePhase::Loop,
        phase_epoch: 1,
        phase_cursor: 0,
        phase_iteration: 0,
        gate_cycle: 0,
        gate_proposal: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: loopflow::task::Observation::NotRequired,
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
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
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
    };
    runtime.block_on(async {
        store.create_wave(&wave).await.expect("create test wave");
        store
            .create_project(&project)
            .await
            .expect("create test project");
        store
            .create_task(&task, &pr)
            .await
            .expect("create test Task");
        let work = store
            .work_for_child(&loopflow::child::ChildRef::Task(task.id.clone()))
            .await
            .expect("resolve test Task Work");
        if !completed_boundary {
            return;
        }
        let (_, lease) = store
            .reserve_run(&work, loopflow::durable::RunTrigger::User)
            .await
            .expect("reserve completed test Task Run");
        store
            .advance_run(
                &lease,
                loopflow::durable::RunAdvance::RunStarting {
                    containment: loopflow::durable::Containment::ProcessGroup { id: 1 },
                    cwd: worktree.to_path_buf(),
                },
            )
            .await
            .expect("start test Task Run");
        let loopflow::durable::AdvanceReceipt::Invocation(invocation) = store
            .advance_run(
                &lease,
                loopflow::durable::RunAdvance::InvocationStarting {
                    route: loopflow::durable::InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "test".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .expect("start test Task Invocation")
        else {
            unreachable!("InvocationStarting returns Invocation")
        };
        let loopflow::durable::AdvanceReceipt::Turn(turn) = store
            .advance_run(
                &lease,
                loopflow::durable::RunAdvance::TurnStarting {
                    invocation_id: invocation.id.clone(),
                },
            )
            .await
            .expect("start test Task Turn")
        else {
            unreachable!("TurnStarting returns Turn")
        };
        store
            .advance_run(
                &lease,
                loopflow::durable::RunAdvance::TurnActive {
                    turn_id: turn.id.clone(),
                    provider_turn_id: None,
                },
            )
            .await
            .expect("activate test Task Turn");
        store
            .advance_run(
                &lease,
                loopflow::durable::RunAdvance::TurnEnded {
                    turn_id: turn.id,
                    outcome: loopflow::durable::BoundaryState::Succeeded,
                },
            )
            .await
            .expect("finish test Task Turn");
        store
            .advance_run(
                &lease,
                loopflow::durable::RunAdvance::InvocationEnded {
                    invocation_id: invocation.id,
                    outcome: loopflow::durable::BoundaryState::Succeeded,
                },
            )
            .await
            .expect("finish test Task Launch");
        store
            .stop_run(
                &lease,
                loopflow::durable::StopCause::Requested,
                loopflow::durable::ContainmentObservation::Absent,
            )
            .await
            .expect("finish test Task Run");
        if active {
            store
                .reserve_run(&work, loopflow::durable::RunTrigger::User)
                .await
                .expect("reserve test Task Run");
        }
    });
    RegisteredTask { store, task, pr }
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
