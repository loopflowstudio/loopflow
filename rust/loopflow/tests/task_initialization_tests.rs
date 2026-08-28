mod support;

use std::process::Command;

use loopflow::id::WaveId;
use loopflow::ops::task::{
    task_run, task_snapshot, task_status, TaskFlowOverrides, TaskLaunchOptions,
};
use loopflow::ops::task_actions::TaskAction;
use loopflow::store::{open_store, PmSnapshotRow, StorageConfig};
use loopflow::work::task::TaskEventKind;
use loopflow_test_support::TestRepo;
use support::{register_unrun_task, EnvGuard};

fn materialize_status_truth(home: &std::path::Path) {
    let runtime = tokio::runtime::Runtime::new().expect("status truth fixture runtime");
    runtime
        .block_on(open_store(&StorageConfig::sqlite(home.join("loopflow.db"))))
        .expect("open status truth fixture registry");
}

fn seed_new_task_snapshot(home: &std::path::Path, repo: &TestRepo) {
    repo.create_file(
        ".lf/config.yaml",
        "pm:\n  provider: linear\n  linear_team: team-loo\n",
    );
    repo.create_file(
        "wave/task-launch/GOAL.md",
        "---\npm:\n  linear_initiative: initiative-task-launch\n---\n\n## Objective\n\nProve Task launch validation.\n",
    );
    repo.stage_all();
    repo.commit("seed Task launch snapshot");
    repo.push();

    let payload = serde_json::json!({
        "projects": [{
            "id": "project-task-launch",
            "slug": "task-launch",
            "name": "Task launch",
            "summary": "Validate lifecycle plans before persistence.",
            "definition": "Invalid lifecycle plans never create managed Tasks.",
            "flows": {"first": null, "loop": null, "finally": null},
            "krs": [],
            "initiative_ids": ["initiative-task-launch"],
            "team_ids": ["team-loo"]
        }],
        "items": [{
            "id": "issue-new-task",
            "identifier": "LOO-NEW",
            "url": null,
            "name": "Reject an unavailable lifecycle",
            "description": "The Task must not be persisted.",
            "rank": 1,
            "completed": false,
            "project_id": "project-task-launch",
            "project": "task-launch",
            "team_id": "team-loo",
            "assignee": null
        }]
    });
    materialize_status_truth(home);
    let runtime = tokio::runtime::Runtime::new().expect("new Task snapshot runtime");
    let store = runtime
        .block_on(open_store(&StorageConfig::sqlite(home.join("loopflow.db"))))
        .expect("open new Task registry");
    let wave_id = WaveId::new();
    let database = rusqlite::Connection::open(home.join("loopflow.db"))
        .expect("open new Task registry fixture");
    database
        .execute(
            "INSERT INTO waves (\
                 id, name, repo, created_at, parent_wave_id, promoted_at,\
                 retired_at, superseded_by_wave_id, retirement_reason\
             ) VALUES (?1, ?2, ?3, ?4, NULL, NULL, NULL, NULL, NULL)",
            rusqlite::params![
                wave_id.as_str(),
                "task-launch",
                std::fs::canonicalize(repo.path())
                    .expect("canonical Task launch repository")
                    .display()
                    .to_string(),
                time::OffsetDateTime::now_utc().unix_timestamp(),
            ],
        )
        .expect("seed Task launch Wave");
    runtime
        .block_on(store.put_pm_snapshot(PmSnapshotRow {
            wave_id,
            provider: "linear".to_string(),
            initiative: "initiative-task-launch".to_string(),
            synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            payload: serde_json::to_string(&payload).expect("serialize Task launch snapshot"),
        }))
        .expect("seed new Task snapshot");
}

#[test]
fn new_task_launch_rejects_an_unavailable_flow_before_persistence() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    seed_new_task_snapshot(home.path(), &repo);

    let error = task_run(
        repo.path(),
        "LOO-NEW",
        TaskLaunchOptions {
            flows: TaskFlowOverrides {
                loop_: Some("removed-task-flow".to_string()),
                ..TaskFlowOverrides::default()
            },
            ..TaskLaunchOptions::default()
        },
    )
    .expect_err("an unavailable flow must reject the launch request");

    let error = error.to_string();
    assert!(error.contains("removed-task-flow"), "launch error: {error}");
    assert!(error.contains("flow not found"), "launch error: {error}");
    let database = rusqlite::Connection::open(home.path().join("loopflow.db"))
        .expect("open rejected Task registry");
    let task_count: i64 = database
        .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
        .expect("count rejected Tasks");
    let pr_count: i64 = database
        .query_row("SELECT count(*) FROM task_prs", [], |row| row.get(0))
        .expect("count rejected Task PRs");
    assert_eq!(task_count, 0, "the rejected launch persists no Task");
    assert_eq!(pr_count, 0, "the rejected launch persists no Task PR");
}

#[test]
fn initializing_worktree_keeps_status_wait_and_roadmap_readable() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    materialize_status_truth(home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/initializing-task";
    repo.create_branch(branch);
    let mut task = register_unrun_task(home.path(), repo.path(), branch, &base);
    let missing_worktree = home.path().join("not-yet-created-worktree");
    task.task.worktree = missing_worktree.clone();
    let runtime = tokio::runtime::Runtime::new().expect("initialization fixture runtime");
    runtime
        .block_on(task.store.update_task(&task.task))
        .expect("publish declared Task worktree");
    runtime
        .block_on(task.store.append_task_event(
            &task.task.id,
            &TaskEventKind::WorktreeInitializing {
                pr_id: task.pr.id.clone(),
                sequence: task.pr.sequence,
                branch: task.pr.branch.clone(),
                path: missing_worktree.display().to_string(),
                base_commit: task.pr.base_commit.clone(),
            },
        ))
        .expect("publish initialization marker");
    std::fs::create_dir_all(&missing_worktree)
        .expect("simulate a partially created worktree directory");
    let project = runtime
        .block_on(task.store.get_project(&task.task.project_id))
        .expect("read owning Project")
        .expect("owning Project exists");
    let payload = serde_json::json!({
        "projects": [{
            "id": project.plan.id.as_str(),
            "slug": project.plan.slug,
            "name": project.plan.name,
            "summary": project.plan.prompt_context,
            "definition": project.plan.prompt_context,
            "flows": {"first": null, "loop": null, "finally": null},
            "krs": [],
            "initiative_ids": ["initialization-initiative"],
            "team_ids": ["initialization-team"]
        }],
        "items": [{
            "id": task.task.plan.id.as_str(),
            "identifier": task.task.plan.identifier,
            "url": null,
            "name": task.task.plan.title,
            "description": task.task.plan.description,
            "rank": 1,
            "completed": false,
            "project_id": project.plan.id.as_str(),
            "project": project.plan.slug,
            "team_id": "initialization-team",
            "assignee": null
        }]
    });
    runtime
        .block_on(task.store.put_pm_snapshot(PmSnapshotRow {
            wave_id: task.task.wave_id.clone(),
            provider: "linear".to_string(),
            initiative: "initialization-initiative".to_string(),
            synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            payload: serde_json::to_string(&payload).expect("serialize PM snapshot"),
        }))
        .expect("seed roadmap planning");
    let run_lf = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env("LF_DB_PATH", home.path().join("loopflow.db"))
            .env_remove("LF_WAVE_ID")
            .current_dir(repo.path())
            .output()
            .expect("run lf read surface")
    };
    let status = run_lf(&["task", "status", "INF-123", "--json"]);
    assert!(
        status.status.success(),
        "status stays readable: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(status["actions"]["recommended"], "no_action");
    assert!(status["actions"]["reason"]
        .as_str()
        .expect("status action reason")
        .contains("is initializing worktree"));

    let wait = run_lf(&["task", "wait", "INF-123", "--timeout", "0s", "--json"]);
    assert!(
        wait.status.success(),
        "wait stays readable: {}",
        String::from_utf8_lossy(&wait.stderr)
    );
    let wait: serde_json::Value = serde_json::from_slice(&wait.stdout).expect("wait JSON");
    assert_eq!(wait["actions"], status["actions"]);

    let roadmap = run_lf(&["roadmap", "--wave", "task-pr-tests", "--json"]);
    assert!(
        roadmap.status.success(),
        "roadmap stays readable: {}",
        String::from_utf8_lossy(&roadmap.stderr)
    );
    let roadmap: serde_json::Value = serde_json::from_slice(&roadmap.stdout).expect("roadmap JSON");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["projects"]["state"], "ok", "roadmap wave: {wave:#}");
    let roadmap_task = &wave["projects"]["items"][0]["tasks"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "INF-123");
    assert_eq!(
        roadmap_task["attention"]["actions"]["recommended"],
        "no_action"
    );
    assert!(roadmap_task["attention"]["reason"]
        .as_str()
        .expect("roadmap attention reason")
        .contains("is initializing worktree"));
    let projected =
        task_snapshot(&task_status("INF-123").expect("read Task")).expect("project Task status");
    assert_eq!(projected.actions.recommended, Some(TaskAction::NoAction));

    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .expect("open stale initialization fixture")
        .execute(
            "UPDATE task_events SET created_at=?2 WHERE task_id=?1",
            rusqlite::params![
                task.task.id.as_str(),
                time::OffsetDateTime::now_utc().unix_timestamp() - 301,
            ],
        )
        .expect("age the initialization marker");
    let stale = run_lf(&["task", "status", "INF-123", "--json"]);
    assert!(
        stale.status.success(),
        "stale initialization stays readable"
    );
    let stale: serde_json::Value =
        serde_json::from_slice(&stale.stdout).expect("stale status JSON");
    assert_eq!(stale["actions"]["recommended"], "no_action");
    assert!(stale["actions"]["reason"]
        .as_str()
        .expect("stale action reason")
        .contains("initialization did not complete"));
    let stale_roadmap = run_lf(&["roadmap", "--wave", "task-pr-tests", "--json"]);
    assert!(
        stale_roadmap.status.success(),
        "stale roadmap stays readable"
    );
    let stale_roadmap: serde_json::Value =
        serde_json::from_slice(&stale_roadmap.stdout).expect("stale roadmap JSON");
    let stale_attention =
        &stale_roadmap["waves"][0]["projects"]["items"][0]["tasks"][0]["attention"];
    assert_eq!(stale_attention["level"], "red");
    assert!(stale_attention["reason"]
        .as_str()
        .expect("stale roadmap reason")
        .contains("initialization did not complete"));
}

#[test]
fn missing_worktree_status_is_actionable_and_read_only() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    materialize_status_truth(home.path());
    let (task, missing_path, branch) = {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/missing-worktree";
        repo.create_branch(branch);
        let task = register_unrun_task(home.path(), repo.path(), branch, &base);
        (task, repo.path().to_path_buf(), branch.to_string())
    };
    assert!(!missing_path.exists(), "fixture worktree is absent");
    let runtime = tokio::runtime::Runtime::new().expect("missing Task runtime");
    let before_task = runtime
        .block_on(task.store.get_task(&task.task.id))
        .expect("read Task before status")
        .expect("Task exists before status");
    let before_prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read PRs before status");

    let status = task_status("INF-123").expect("status survives the absent worktree");
    let snapshot = task_snapshot(&status).expect("project missing-worktree status");

    assert_eq!(snapshot.actions.recommended, Some(TaskAction::NoAction));
    assert!(snapshot
        .actions
        .reason
        .contains(&missing_path.display().to_string()));
    assert!(snapshot.actions.reason.contains(&branch));
    assert!(snapshot.actions.reason.contains("lf task resume INF-123"));
    assert!(snapshot
        .actions
        .reason
        .contains("identity and PR history are unchanged"));
    assert_eq!(
        runtime
            .block_on(task.store.get_task(&task.task.id))
            .expect("reread Task after status")
            .expect("Task remains registered"),
        before_task
    );
    assert_eq!(
        runtime
            .block_on(task.store.task_prs(&task.task.id))
            .expect("reread PRs after status"),
        before_prs
    );
}
