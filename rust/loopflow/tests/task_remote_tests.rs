mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::durable::TaskId;
use loopflow::ops::resolve_work_binding;
use loopflow::planning::LinearIssueId;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::TestRepo;
use serde_json::json;

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn source(branch: &str, commit: &str) -> String {
    json!({
        "issue": "FIX-1", "branch": branch, "commit": commit,
        "planning": {
            "observed_at": 1791360000,
            "item": {"id":"issue-1", "identifier":"FIX-1", "branch_name":"outdated-linear-name",
                "revision":"2026-10-07T00:00:00Z", "url":null, "name":"Portable task",
                "description":"Existing implementation", "rank":0, "completed":false,
                "state":"started", "project_id":"project-1", "project":"chapter",
                "team_id":"team-1", "assignee":null},
            "project": {"id":"project-1", "slug":"chapter", "name":"Chapter",
                "summary":"", "metric_targets":[], "workflow":"", "status":"started",
                "krs":[], "initiative_ids":["initiative-1"], "team_ids":["team-1"]}
        }
    })
    .to_string()
}

fn repository() -> TestRepo {
    let repo = TestRepo::new();
    repo.create_file(
        ".lf/config.yaml",
        "pm:\n  provider: linear\n  linear_team: team-1\n",
    );
    repo.create_file(
        "wave/product/GOAL.md",
        "---\npm:\n  linear_initiative: initiative-1\n---\nKeep working.\n",
    );
    repo.stage_all();
    repo.commit("Repository planning");
    repo.push();
    repo
}

#[test]
fn cold_machines_adopt_pushed_code_once_with_the_same_task_id() {
    let repo = repository();
    let machines = tempfile::tempdir().unwrap();
    let remote = String::from_utf8(
        Command::new("git")
            .current_dir(repo.path())
            .args(["remote", "get-url", "origin"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let remote = remote.trim();
    // Clone before the branch exists: neither machine has a remote-tracking ref.
    for name in ["first", "second"] {
        git(machines.path(), &["clone", remote, name]);
        git(
            &machines.path().join(name),
            &["config", "user.name", "Fixture"],
        );
        git(
            &machines.path().join(name),
            &["config", "user.email", "fixture@example.test"],
        );
    }
    let branch = "test/portable";
    repo.create_branch(branch);
    repo.create_file("implementation.txt", "already implemented\n");
    repo.stage_all();
    repo.commit("Pushed implementation without a PR");
    git(repo.path(), &["push", "origin", branch]);
    let head = repo.head_sha();
    let expected_id = TaskId::from_issue(&LinearIssueId::new("issue-1").unwrap());
    for name in ["first", "second"] {
        let home = tempfile::tempdir().unwrap();
        let _env = support::EnvGuard::with_lf_home(
            &[(
                "gh",
                "#!/bin/sh\n[ \"$1\" = --version ] && exit 0\necho '[]'\n",
            )],
            home.path(),
        );
        std::env::set_var("LF_TASK_SOURCE", source(branch, &head));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = std::sync::Arc::new(
            runtime
                .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                    home.path().join("loopflow.db"),
                )))
                .unwrap(),
        );
        let checkout = machines.path().join(name);
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .current_dir(&checkout)
            .args(["--task", "FIX-1", "context", "--json"])
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let first = runtime
            .block_on(resolve_work_binding(&store, &checkout, "task:FIX-1"))
            .unwrap();
        let task = runtime
            .block_on(store.get_task_by_issue("FIX-1"))
            .unwrap()
            .unwrap();
        assert_eq!(task.id, expected_id);
        assert_eq!(task.plan.pm_snapshot_synced_at, 1791360000);
        assert_eq!(first.cwd, task.worktree);
        assert_eq!(
            fs::read_to_string(task.worktree.join("implementation.txt")).unwrap(),
            "already implemented\n"
        );
        assert_eq!(
            loopflow::engine::git::rev_parse(&task.worktree, "HEAD").unwrap(),
            head
        );
        fs::write(task.worktree.join("local-notes"), "keep local work").unwrap();
        let second = runtime
            .block_on(resolve_work_binding(&store, &checkout, "task:FIX-1"))
            .unwrap();
        assert_eq!(second.cwd, first.cwd);
        assert_eq!(runtime.block_on(store.list_tasks(None)).unwrap().len(), 1);
        assert_eq!(runtime.block_on(store.task_prs(&task.id)).unwrap().len(), 1);
        assert_eq!(
            loopflow::engine::worktrees::list_worktrees(&checkout)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            fs::read_to_string(task.worktree.join("local-notes")).unwrap(),
            "keep local work"
        );
        let workflows: i64 = rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row("SELECT count(*) FROM task_workflows", [], |row| row.get(0))
            .unwrap();
        assert_eq!(workflows, 0);
        std::env::remove_var("LF_TASK_SOURCE");
        fs::remove_dir_all(task.worktree).unwrap();
    }
}

#[test]
fn missing_remote_branch_or_commit_is_named_without_creating_a_task() {
    let repo = repository();
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = std::sync::Arc::new(
        runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                home.path().join("loopflow.db"),
            )))
            .unwrap(),
    );
    let branch = "test/unpushed";
    repo.create_branch(branch);
    let initial = repo.head_sha();
    std::env::set_var("LF_TASK_SOURCE", source(branch, &initial));
    let missing = runtime
        .block_on(resolve_work_binding(&store, repo.path(), "task:FIX-1"))
        .unwrap_err()
        .to_string();
    assert!(
        missing.contains(branch) && missing.contains("push"),
        "{missing}"
    );
    git(repo.path(), &["push", "origin", branch]);
    repo.create_file("unpublished.txt", "local implementation");
    repo.stage_all();
    repo.commit("Unpushed work");
    let unpublished = repo.head_sha();
    std::env::set_var("LF_TASK_SOURCE", source(branch, &unpublished));
    let missing = runtime
        .block_on(resolve_work_binding(&store, repo.path(), "task:FIX-1"))
        .unwrap_err()
        .to_string();
    assert!(
        missing.contains(branch) && missing.contains(&unpublished) && missing.contains("push"),
        "{missing}"
    );
    assert!(runtime.block_on(store.list_tasks(None)).unwrap().is_empty());
    assert_eq!(
        loopflow::engine::worktrees::list_worktrees(repo.path())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(repo.head_sha(), unpublished);
    assert_eq!(
        loopflow::engine::git::rev_parse(repo.path(), &format!("origin/{branch}")).unwrap(),
        initial
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("unpublished.txt")).unwrap(),
        "local implementation"
    );
    std::env::remove_var("LF_TASK_SOURCE");
}

#[test]
fn ssh_names_unpushed_source_work_before_connecting_and_keeps_legacy_identity() {
    let repo = TestRepo::new();
    repo.create_file(
        ".lf/config.yaml",
        "pm:\n  provider: linear\n  linear_team: team-task-pr-tests\n",
    );
    repo.stage_all();
    repo.commit("Planning config");
    repo.push();
    let branch = "test/source";
    repo.create_branch(branch);
    git(repo.path(), &["push", "origin", branch]);
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_lf_home(&[], home.path());
    let fixture = support::register_unrun_task(
        home.path(),
        &repo.path().canonicalize().unwrap(),
        branch,
        &repo.head_sha(),
    );
    let store = std::sync::Arc::new(fixture.store);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let binding = runtime
        .block_on(resolve_work_binding(&store, repo.path(), "task:INF-123"))
        .unwrap();
    assert_eq!(binding.work.id(), fixture.task.id.as_str());
    assert_ne!(fixture.task.id, TaskId::from_issue(&fixture.task.plan.id));
    repo.create_file("unfinished.txt", "source work");
    let args = ["--task", "INF-123", "context", "--json"].map(str::to_string);
    let invoke = || {
        loopflow::lf::commands::ssh::run(
            "unreachable.invalid",
            None,
            &[],
            false,
            &loopflow::provider_account::lease::AccountSelection::default(),
            &args,
        )
        .unwrap_err()
        .to_string()
    };
    let dirty = invoke();
    assert!(
        dirty.contains(branch) && dirty.contains("uncommitted"),
        "{dirty}"
    );
    repo.stage_all();
    repo.commit("Unpushed implementation");
    let head = repo.head_sha();
    let missing = invoke();
    assert!(
        missing.contains(branch) && missing.contains(&head),
        "{missing}"
    );
    assert_eq!(repo.head_sha(), head);
    assert_eq!(
        fs::read_to_string(repo.path().join("unfinished.txt")).unwrap(),
        "source work"
    );
    assert_eq!(
        runtime
            .block_on(store.get_task_by_issue("INF-123"))
            .unwrap()
            .unwrap()
            .id,
        fixture.task.id
    );
}

#[test]
fn remote_planning_cannot_revive_a_removed_issue() {
    let repo = repository();
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = std::sync::Arc::new(
        runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                home.path().join("loopflow.db"),
            )))
            .unwrap(),
    );
    runtime
        .block_on(store.observe_pm_issue_change("issue-1", None, true))
        .unwrap();
    std::env::set_var("LF_TASK_SOURCE", source("main", &repo.head_sha()));
    let result = runtime.block_on(resolve_work_binding(&store, repo.path(), "task:FIX-1"));
    assert!(result.is_err());
    assert!(runtime.block_on(store.list_tasks(None)).unwrap().is_empty());
    let scope = loopflow::repository::CanonicalRepo::discover(repo.path())
        .unwrap()
        .to_string();
    let retained = runtime
        .block_on(store.pm_task_observation(&scope, "linear", "issue-1"))
        .unwrap();
    assert_eq!(retained.state, loopflow::store::PlanningState::Removed);
    assert!(retained.record.is_none());
    std::env::remove_var("LF_TASK_SOURCE");
}
