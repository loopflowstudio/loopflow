use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use loopflow::id::WaveId;
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::store::{PmSnapshotRow, StorageConfig, Store};
use loopflow::work::project::{Project, ProjectId};
use loopflow::work::task::{PmWritebackState, Task, TaskId, TaskPr, TaskPrId};
use loopflow::work::wave::Wave;
use tempfile::TempDir;
use time::OffsetDateTime;

mod ambient;

#[allow(dead_code)] // Shared GitHub fixture compiled into multiple test crates.
pub fn github_checks_page(head: &str, checks: &[(&str, &str, bool)]) -> String {
    let nodes: Vec<_> = checks
        .iter()
        .map(|(name, conclusion, required)| {
            serde_json::json!({
                "__typename":"CheckRun", "name":name, "status":"COMPLETED",
                "conclusion":conclusion, "isRequired":required,
                "startedAt":"2026-09-29T00:00:00Z", "detailsUrl":format!("https://ci/{name}"),
                "checkSuite":{"workflowRun":null}
            })
        })
        .collect();
    serde_json::json!({"head":head,"commit":head,"contexts":{
        "nodes":nodes,"pageInfo":{"hasNextPage":false,"endCursor":null}
    }})
    .to_string()
}

#[allow(dead_code)] // Shared GitHub fixture compiled into multiple test crates.
pub fn github_merge_response(
    number: u64,
    head: &str,
    state: &str,
    merge_state: &str,
    request: Option<&str>,
) -> String {
    let queued = request.and_then(|request| request.strip_prefix("queued:"));
    serde_json::json!({"data":{"repository":{"pullRequest":{
        "id":queued.unwrap_or("PR_fixture"), "number":number,
        "url":format!("https://example.com/pr/{number}"),
        "state":state, "isDraft":false, "headRefName":"fixture", "headRefOid":head,
        "mergedAt":if state == "MERGED" { Some("2026-09-29T00:00:00Z") } else { None },
        "mergeCommit":if state == "MERGED" { Some(serde_json::json!({"oid":head})) } else { None },
        "mergeStateStatus":merge_state, "isMergeQueueEnabled":request == Some("awaiting_queue") || queued.is_some(),
        "autoMergeRequest":request.map(|_| serde_json::json!({"enabledAt":"2026-09-29T00:00:00Z"})),
        "mergeQueueEntry":queued.map(|_| serde_json::json!({"id":"queue-entry"}))
    }}}}).to_string()
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct HomeOverride {
    _ambient: ambient::EnvGuard,
    previous_lf_home: Option<OsString>,
    previous_db_path: Option<OsString>,
    _temp: TempDir,
}

impl HomeOverride {
    fn new_temp() -> Self {
        let ambient = ambient::EnvGuard::new();
        let temp = TempDir::new().expect("temp home dir");
        let previous_lf_home = env::var_os("LF_HOME");
        let previous_db_path = env::var_os("LF_DB_PATH");
        env::remove_var("LF_HOME");
        env::remove_var("LF_DB_PATH");
        env::set_var("LF_HOME", temp.path());
        Self {
            _ambient: ambient,
            previous_lf_home,
            previous_db_path,
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
    }
}

#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn with_clean_home<T>(f: impl FnOnce() -> T) -> T {
    let _lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let _home = HomeOverride::new_temp();
    f()
}

#[allow(dead_code)] // Shared provider mock compiled into multiple test crates.
pub fn codex_app_server_script(output: &str, setup: &str) -> String {
    let output = serde_json::to_string(output)
        .expect("encode mock Codex output")
        .replace('\'', r#"'"'"'"#);
    codex_socket_script(&r#"#!/bin/sh
__SETUP__
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_start
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"thread-test"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn-test"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"thread-test","turn":{"id":"turn-test","status":"inProgress"}}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"thread-test","turnId":"turn-test","itemId":"message-test","delta":__OUTPUT__}}'
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"thread-test","turn":{"id":"turn-test","status":"completed"}}}'
if [ -n "$LF_TEST_CODEX_STDIO" ]; then exit 0; fi
while read -r line; do :; done
"#
    .replace("__SETUP__", setup)
    .replace("__OUTPUT__", &output))
}

#[allow(dead_code)] // Shared provider transport compiled into multiple test crates.
pub fn codex_socket_script(script: &str) -> String {
    let bridge = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/codex_socket.py")
        .replace('\'', r#"'"'"'"#);
    format!(
        r#"#!/bin/sh
if [ -z "$LF_TEST_CODEX_STDIO" ]; then
    case "$*" in
        *--listen*) exec python3 '{bridge}' "$0" "$@" ;;
    esac
fi
{script}
"#,
        script = script.strip_prefix("#!/bin/sh\n").unwrap_or(script),
    )
}

pub struct EnvGuard {
    previous_path: Option<String>,
    previous_home: Option<String>,
    previous_lf_home: Option<OsString>,
    previous_db_path: Option<OsString>,
    _ambient: ambient::EnvGuard,
    _bin: TempDir,
    _lf_home: TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
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
        let ambient = ambient::EnvGuard::new();
        let lf_home = TempDir::new().expect("temp lf home dir");
        env::remove_var("LF_HOME");
        env::remove_var("LF_DB_PATH");
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
            _ambient: ambient,
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
    register_task_fixture(home, worktree, branch, base_commit)
}

#[allow(dead_code)] // Shared helper compiled into integration tests without this incident shape.
pub fn register_unrun_task(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    register_task_fixture(home, worktree, branch, base_commit)
}

fn register_task_fixture(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.join("loopflow.db")),
        ))
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
            flow: "feature".into(),
            status: loopflow::pm::ProjectStatus::Started,
            id: LinearProjectId::new(format!("project-{}", WaveId::new())).expect("project id"),
            slug: "task-pr-tests".to_string(),
            name: "Task PR tests".to_string(),
            prompt_context: "Keep Task PR transitions durable.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        iteration: 0,
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
        agent: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: loopflow::work::task::Observation::NotRequired,
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
        let pm_payload = serde_json::json!({
            "projects": [{
                "id": project.plan.id.as_str(),
                "slug": project.plan.slug.as_str(),
                "name": project.plan.name.as_str(),
                "summary": "",
                "metric_targets": [],
                "flow": project.plan.flow,
                "status": project.plan.status,
                "krs": [],
                "initiative_ids": ["initiative-task-pr-tests"],
                "team_ids": ["team-task-pr-tests"]
            }],
            "items": [{
                "id": task.plan.id.as_str(),
                "identifier": task.plan.identifier.as_str(),
                "url": "https://linear.app/loopflow/issue/INF-123/prove-task-pr-transitions",
                "name": task.plan.title.as_str(),
                "description": task.plan.description.as_str(),
                "rank": 1,
                "completed": false,
                "project_id": project.plan.id.as_str(),
                "project": project.plan.slug.as_str(),
                "team_id": "team-task-pr-tests",
                "assignee": null
            }]
        })
        .to_string();
        store
            .put_pm_snapshot(PmSnapshotRow {
                wave_id: wave.id().clone(),
                provider: "linear".to_string(),
                initiative: "initiative-task-pr-tests".to_string(),
                synced_at: now.unix_timestamp(),
                payload: pm_payload,
            })
            .await
            .expect("cache Task PR context");
        store
            .create_project(&project)
            .await
            .expect("create test project");
        store
            .create_task(&task, &pr)
            .await
            .expect("create test Task");
    });
    RegisteredTask { store, task, pr }
}

/// A second Task in the fixture's Wave and Project, tracking `branch` from its
/// own `worktree`, so a test can tell "this checkout's Task" from "another
/// Task". Registered worktrees are unique, so the sibling needs a path of its
/// own; `TestRepo::create_named_worktree` supplies a real one with the branch.
#[allow(dead_code)] // Shared helper compiled into integration tests with one Task.
pub fn register_sibling_task(
    registered: &RegisteredTask,
    identifier: &str,
    branch: &str,
    worktree: &Path,
) -> Task {
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let now = OffsetDateTime::now_utc();
    let mut task = registered.task.clone();
    task.id = TaskId::new();
    task.plan.id = LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id");
    task.plan.identifier = identifier.to_string();
    task.plan.title = format!("Sibling {identifier}");
    task.workspace_slug = branch.to_string();
    task.worktree = worktree.to_path_buf();
    task.created_at = now;
    task.updated_at = now;
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        slug: branch.to_string(),
        branch: branch.to_string(),
        created_at: now,
        updated_at: now,
        ..registered.pr.clone()
    };
    runtime
        .block_on(registered.store.create_task(&task, &pr))
        .expect("create sibling Task");
    task
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
