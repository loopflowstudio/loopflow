pub mod planning;

use loopflow_test_support::env_lock;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use base64::prelude::*;
use loopflow::id::WaveId;
use loopflow::linear::{LinearIssueId, LinearProjectId};
use loopflow::store::{StorageConfig, Store};
use loopflow::work::project::{Project, ProjectId};
use loopflow::work::task::{LinearWritebackState, Task, TaskId, TaskPr, TaskPrId};
use loopflow::work::wave::Wave;
use loopflow_test_support::TestRepo;
use tempfile::TempDir;
use time::OffsetDateTime;

pub mod ambient;

/// Agent shells such as Codex export the developer's real provider home
/// (`CODEX_HOME=~/.codex`). An `lf` fixture child that inherits it would swap
/// fixture logins into that home, so every test binary drops both variables
/// before `main`, while no other thread can read the environment. Fixtures
/// that need a provider home set one explicitly.
#[ctor::ctor(unsafe)]
fn scrub_inherited_provider_homes() {
    env::remove_var("CODEX_HOME");
    env::remove_var("CLAUDE_CONFIG_DIR");
}

#[cfg(unix)]
#[allow(dead_code)] // Shared native provider fixture compiled into multiple test crates.
pub fn install_native_codex(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(bin).unwrap();
    let provider = bin.join("codex");
    fs::write(
        &provider,
        format!(
            "#!/bin/sh\nexec /usr/bin/python3 '{}' \"$@\"\n",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/native_codex.py")
        ),
    )
    .unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
}

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
    serde_json::json!({"data":{"repository":{"mergeCommitAllowed":true,"squashMergeAllowed":true,"rebaseMergeAllowed":true,"pullRequest":{
        "id":queued.unwrap_or("PR_fixture"), "number":number,
        "url":format!("https://example.com/pr/{number}"),
        "state":state, "isDraft":false, "headRefName":"fixture", "headRefOid":head,
        "mergedAt":if state == "MERGED" { Some("2026-09-29T00:00:00Z") } else { None },
        "mergeCommit":if state == "MERGED" { Some(serde_json::json!({"oid":head})) } else { None },
        "mergeStateStatus":merge_state, "isMergeQueueEnabled":request == Some("awaiting_queue") || queued.is_some(), "isInMergeQueue":queued.is_some(),
        "autoMergeRequest":request.map(|_| serde_json::json!({"enabledAt":"2026-09-29T00:00:00Z"})),
        "mergeQueueEntry":queued.map(|_| serde_json::json!({"id":"queue-entry"})),
        "timelineItems":{"nodes":[]}
    }}}}).to_string()
}

struct HomeOverride {
    _ambient: ambient::EnvGuard,
    previous_lf_home: Option<OsString>,
    _temp: TempDir,
}

impl HomeOverride {
    fn new_temp() -> Self {
        let ambient = ambient::EnvGuard::new();
        let temp = TempDir::new().expect("temp home dir");
        let previous_lf_home = env::var_os("LF_HOME");
        env::remove_var("LF_HOME");
        env::set_var("LF_HOME", temp.path());
        Self {
            _ambient: ambient,
            previous_lf_home,
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
printf '%s\n' '{"jsonrpc":"2.0","method":"item/completed","params":{"threadId":"thread-test","turnId":"turn-test","item":{"type":"agentMessage","id":"message-test","text":__OUTPUT__,"phase":"final_answer"}}}'
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

#[allow(dead_code)] // Shared synthetic account for provider-backed CLI proofs.
pub fn register_codex_account(home: &Path) {
    register_codex_login(home, "fixture");
}

/// A connected synthetic Codex login `<name>@example.com`, account id `<name>`.
#[allow(dead_code)]
pub fn register_codex_login(home: &Path, name: &str) {
    let account_home = home.join("accounts/codex").join(name);
    fs::create_dir_all(&account_home).unwrap();
    let email = format!("{name}@example.com");
    let email = email.as_str();
    let claims =
        BASE64_URL_SAFE_NO_PAD.encode(serde_json::json!({"email": email, "sub": name}).to_string());
    fs::write(
        account_home.join("auth.json"),
        serde_json::json!({"tokens": {
            "access_token": "synthetic-fixture-token",
            "id_token": format!("h.{claims}.s")
        }})
        .to_string(),
    )
    .unwrap();
    let store = loopflow::store::sqlite::SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    store
        .upsert_provider_account(&loopflow::store::ProviderAccount {
            provider: "codex".into(),
            account_id: loopflow::store::ProviderAccountId::parse(name).unwrap(),
            home: Some(account_home),
            login_email: Some(loopflow::profile::EmailAddress::parse(email).unwrap()),
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: loopflow::store::CredentialState::Connected,
            routing_state: loopflow::store::RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: now,
            updated_at: now,
        })
        .unwrap();
}

pub struct EnvGuard {
    previous_path: Option<String>,
    previous_home: Option<String>,
    previous_lf_home: Option<OsString>,
    previous_lf_bin: Option<OsString>,
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
        let cli = loopflow_test_support::lf_binary();
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
        let ambient = ambient::EnvGuard::new();
        let previous_lf_bin = env::var_os("LF_BIN");
        env::set_var("LF_BIN", cli);
        let lf_home = TempDir::new().expect("temp lf machine dir");
        env::remove_var("LF_HOME");
        if let Some(home) = home {
            // Keep HOME-based config discovery intact while isolating its store.
            env::set_var("LF_HOME", home.join(".lf"));
        } else {
            env::set_var("LF_HOME", lf_home.path());
        }
        // A shared launch signs its provider's native home in as the routed
        // account; that home must never be the developer's own.
        env::set_var("CODEX_HOME", lf_home.path().join("native-codex"));
        env::set_var("CLAUDE_CONFIG_DIR", lf_home.path().join("native-claude"));
        Self {
            _lock: lock,
            previous_path,
            previous_home,
            previous_lf_home,
            previous_lf_bin,
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
        match &self.previous_lf_bin {
            Some(previous) => env::set_var("LF_BIN", previous),
            None => env::remove_var("LF_BIN"),
        }
        match &self.previous_lf_home {
            Some(prev) => env::set_var("LF_HOME", prev),
            None => env::remove_var("LF_HOME"),
        }
        // The pre-main scrub left no provider home to restore.
        env::remove_var("CODEX_HOME");
        env::remove_var("CLAUDE_CONFIG_DIR");
    }
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub fn bind_task_planning(repo: &TestRepo) {
    repo.create_file(
        ".lf/config.yaml",
        "planning:\n  provider: linear\n  team: team-task-pr-tests\n",
    );
    repo.create_file(".lf/wave/task-pr-tests/GOAL.md", "Keep work.\n");
    repo.create_file(
        ".lf/wave/task-pr-tests/config.yaml",
        "linear:\n  initiative: initiative-task-pr-tests\n",
    );
    repo.stage_all();
    repo.commit("Bind fixture planning before creating Task checkouts");
}

/// Select a Wave's default graph in its committed `config.yaml` in `repo`.
#[allow(dead_code)] // Shared helper compiled into integration tests without Wave graphs.
pub fn select_wave_graph(
    repo: &Path,
    store: &loopflow::store::sqlite::SqliteStore,
    wave: &WaveId,
    name: &str,
) {
    let wave = store.get_wave(wave).unwrap().unwrap();
    let path = repo.join(".lf/wave").join(wave.slug()).join("config.yaml");
    let mut config: serde_yaml_ng::Mapping = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_yaml_ng::from_str(&content).ok())
        .unwrap_or_default();
    config.insert("graph".into(), name.into());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_yaml_ng::to_string(&config).unwrap()).unwrap();
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub struct RegisteredTask {
    pub database: std::path::PathBuf,
    pub store: Store,
    pub task: Task,
    pub pr: TaskPr,
}

/// Seed a retained PR row explicitly, independently of checkout placement.
#[allow(dead_code)] // Shared helper compiled into tests without delivery fixtures.
pub fn register_task_with_pr(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    let registered = register_task_fixture(home, worktree, branch, base_commit);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(registered.store.insert_task_pr(&registered.pr))
        .unwrap();
    registered
}

#[allow(dead_code)] // Shared helper compiled into suites with published Tasks only.
pub fn register_task_without_pr(
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

        summary: String::new(),
        status: loopflow::work::project::ProjectStatus::Started,
        linear_id: Some(
            LinearProjectId::new(format!("project-{}", WaveId::new())).expect("project id"),
        ),
        slug: "task-pr-tests".to_string(),
        name: "Task PR tests".to_string(),
        description: "Keep Task PR transitions durable.".to_string(),
        krs: Vec::new(),
        metric_targets: Vec::new(),
        pm_snapshot_synced_at: Some(now.unix_timestamp()),

        wave_id: wave.id().clone(),
        created_at: now,
        updated_at: now,
    };
    let task = Task {
        repository_id: loopflow::store::sqlite::SqliteStore::new(&home.join("loopflow.db"))
            .unwrap()
            .ensure_repository(wave.repo())
            .unwrap(),
        id: TaskId::new(),

        revision: 0,
        linear_id: Some(LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id")),
        identifier: "INF-123".to_string(),
        title: "Prove Task PR transitions".to_string(),
        description: "Exercise the persisted lifecycle.".to_string(),
        pm_snapshot_synced_at: Some(now.unix_timestamp()),

        pm_writeback: LinearWritebackState::Current,
        wave_id: Some(wave.id().clone()),
        project_id: Some(project.id.clone()),
        worktree: Some(worktree.to_path_buf()),
        workspace_slug: "task-pr-proof".to_string(),
        branch: branch.to_string(),
        base_commit: base_commit.to_string(),
        parent_pr_id: None,
        agent: None,
        created_at: now,
        updated_at: now,
        rank: 0,
        assignee: None,
        due_date: None,
        regressed_in: None,
        state: None,
        completed_at: None,
        observation: loopflow::work::task::Observation::NotRequired,
    };
    let mut pr = TaskPr {
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
        rusqlite::Connection::open(home.join("loopflow.db"))
            .expect("open fixture database")
            .execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                rusqlite::params![wave.id(), project.id.as_str()],
            )
            .expect("select fixture Project");
        planning::seed_unplaced_task(&home.join("loopflow.db"), &task);
        planning::seed_linear_admission(&home.join("loopflow.db"), &wave, &project, &task);
        pr.task_id = task.id.clone();
        store
            .place_task(
                &task.id,
                task.worktree.as_ref().unwrap(),
                &task.workspace_slug,
                &pr,
                None,
            )
            .await
            .expect("create test Task");
        store
            .append_task_event(
                &task.id,
                &loopflow::work::task::TaskEventKind::CheckoutReady {
                    branch: branch.to_string(),
                    base_commit: base_commit.to_string(),
                },
            )
            .await
            .unwrap();
    });
    RegisteredTask {
        store,
        task,
        pr,
        database: home.join("loopflow.db"),
    }
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
    task.linear_id =
        Some(LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id"));
    task.identifier = identifier.to_string();
    task.title = format!("Sibling {identifier}");
    task.workspace_slug = branch.to_string();
    task.branch = branch.to_string();
    task.worktree = Some(worktree.to_path_buf());
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
    planning::seed_unplaced_task(&registered.database, &task);
    runtime
        .block_on(registered.store.place_task(
            &task.id,
            task.worktree.as_ref().unwrap(),
            &task.workspace_slug,
            &pr,
            None,
        ))
        .expect("create sibling Task");
    if worktree.is_dir() {
        runtime
            .block_on(registered.store.append_task_event(
                &task.id,
                &loopflow::work::task::TaskEventKind::CheckoutReady {
                    branch: branch.to_string(),
                    base_commit: task.base_commit.clone(),
                },
            ))
            .unwrap();
    }
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

/// Record a Graph the way `lf graph` leaves one: a Graph process, its GraphProcess row
/// and one step Process in `cwd`, both exited. Returns the Graph process's id, which
/// names the Graph.
#[allow(dead_code)] // Shared helper compiled into integration tests that record no Graph.
pub fn record_graph(home: &Path, cwd: &Path, graph: &str, label: &str, outcome: &str) -> String {
    use loopflow::graph::{ConcreteSkill, ConcreteStep, Skill};
    let db = rusqlite::Connection::open(home.join("loopflow.db")).expect("open test registry");
    let graph_process = loopflow::id::LfProcessId::new();
    let step = loopflow::id::LfProcessId::new();
    for (id, parent, argv) in [
        (graph_process.clone(), None, vec!["lf", "graph", graph]),
        (
            step.clone(),
            Some(graph_process.clone()),
            vec!["lf", "--batch", "skill", label],
        ),
    ] {
        db.execute(
            "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,repo,cwd,started_at,completed_at,outcome)
             VALUES(?1,?2,?3,?4,?5,?5,1,2,?6)",
            rusqlite::params![
                id.as_str(),
                loopflow::id::TraceId::new().as_str(),
                parent.as_ref().map(|parent| parent.as_str().to_owned()),
                serde_json::to_string(&argv).expect("argv serializes"),
                cwd.to_string_lossy(),
                outcome
            ],
        )
        .expect("record Graph Process");
    }
    let compiled = loopflow::graph::compiled::CompiledGraph::new(
        graph,
        &[ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(label),
            id: None,
            interactive: false,
            returns: None,
            sources: Vec::new(),
        })],
    );
    db.execute(
        "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,?2,?3)",
        rusqlite::params![
            graph_process.as_str(),
            graph,
            serde_json::to_string(&compiled).expect("compiled serializes")
        ],
    )
    .expect("record GraphProcess");
    db.execute(
        "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations) VALUES(?1,?2,0,'[[]]')",
        rusqlite::params![graph_process.as_str(), step.as_str()],
    )
    .expect("record Graph step");
    graph_process.to_string()
}

/// Every Graph in launch order: its process's outcome and each recorded step as
/// `{graph, label, key, iterations, argv}`, `argv` without the binary path.
#[allow(dead_code)] // Shared helper compiled into integration tests that read no Graph.
pub fn recorded_graphs(home: &Path) -> Vec<(Option<String>, Vec<serde_json::Value>)> {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).expect("open test registry");
    let mut graph_processes = db
        .prepare(
            "SELECT d.id,d.outcome,g.graph,g.compiled FROM graph_processes g JOIN processes d ON d.id=g.lf_process_id
             ORDER BY d.rowid",
        )
        .expect("select Graph processes");
    let rows = graph_processes
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("read Graph processes")
        .collect::<rusqlite::Result<Vec<(String, Option<String>, String, String)>>>()
        .expect("decode Graph processes");
    rows.into_iter()
        .map(|(graph_process, outcome, graph, compiled)| {
            let compiled: loopflow::graph::compiled::CompiledGraph =
                serde_json::from_str(&compiled).expect("a Graph compiled");
            fn label(nodes: &[loopflow::graph::compiled::GraphNode], key: u32) -> Option<String> {
                nodes.iter().find_map(|node| {
                    if node.key == key {
                        return Some(node.label.clone());
                    }
                    node.paths.iter().find_map(|path| label(&path.steps, key))
                })
            }
            let mut steps = db
                .prepare(
                    "SELECT e.command,s.node,s.iterations FROM graph_process_steps s
                     JOIN processes e ON e.id=s.lf_process_id WHERE s.graph_lf_process_id=?1 ORDER BY s.seq",
                )
                .expect("select Graph steps");
            let steps = steps
                .query_map([graph_process], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .expect("read Graph steps")
                .map(|row| {
                    let (argv, key, iterations) = row.expect("a Graph step");
                    let argv: Vec<String> = serde_json::from_str(&argv).expect("a JSON argv");
                    serde_json::json!({
                        "graph": graph,
                        "label": label(&compiled.steps, key).expect("a compiled node"),
                        "key": key,
                        "iterations": serde_json::from_str::<serde_json::Value>(&iterations)
                            .expect("iteration counts"),
                        "argv": argv[1..],
                    })
                })
                .collect();
            (outcome, steps)
        })
        .collect()
}

/// Managed skill execution in a Task checkout needs a connected agent account.
#[allow(dead_code)] // Shared fixture compiled into multiple test crates.
pub fn managed_codex_account(home: &Path) {
    let account_home = home.join("fixture-codex");
    std::fs::create_dir(&account_home).unwrap();
    let claims =
        BASE64_URL_SAFE_NO_PAD.encode(r#"{"email":"fixture@example.com","sub":"fixture"}"#);
    std::fs::write(
        account_home.join("auth.json"),
        serde_json::json!({"tokens":{"access_token":"fixture","id_token":format!("h.{claims}.s")}})
            .to_string(),
    )
    .unwrap();
    let store = loopflow::store::sqlite::SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let account_id = loopflow::store::ProviderAccountId::parse("fixture").unwrap();
    store
        .upsert_provider_account(&loopflow::store::ProviderAccount {
            provider: "codex".into(),
            account_id: account_id.clone(),
            home: Some(account_home),
            login_email: Some(
                loopflow::profile::EmailAddress::parse("fixture@example.com").unwrap(),
            ),
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: loopflow::store::CredentialState::Connected,
            routing_state: loopflow::store::RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        })
        .unwrap();
    store
        .set_provider_route(&loopflow::profile::ProviderRoute {
            scope: loopflow::profile::RouteScope::Default,
            provider: loopflow::provider_auth::Provider::Codex,
            accounts: vec![account_id],
            created_at: 1,
            updated_at: 1,
        })
        .unwrap();
}
