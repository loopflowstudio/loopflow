mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::durable::TaskId;
use loopflow::ops::resolve_work_binding;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::TestRepo;
use serde_json::json;

fn git(repo: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn source(branch: &str, commit: &str) -> String {
    json!({"branch":branch,"commit":commit,"task_id":TaskId::new(),"identifier":"FIX-1","issue_id":null})
        .to_string()
}

#[test]
fn missing_remote_branch_or_commit_is_named_without_creating_a_task() {
    let repo = TestRepo::new();
    repo.push();
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
    let missing = loopflow::ops::task::task_checkout(repo.path(), "FIX-1", Default::default())
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
    let missing = loopflow::ops::task::task_checkout(repo.path(), "FIX-1", Default::default())
        .unwrap_err()
        .to_string();
    assert!(
        missing.contains(branch) && missing.contains(&unpublished) && missing.contains("push"),
        "{missing}"
    );
    assert!(runtime.block_on(store.list_tasks(None)).unwrap().is_empty());
    assert_eq!(
        loopflow::git::worktrees::list_worktrees(repo.path())
            .unwrap()
            .len(),
        1
    );
    assert_eq!(repo.head_sha(), unpublished);
    assert_eq!(
        loopflow::git::rev_parse(repo.path(), &format!("origin/{branch}")).unwrap(),
        initial
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("unpublished.txt")).unwrap(),
        "local implementation"
    );
    std::env::remove_var("LF_TASK_SOURCE");
}

#[test]
fn machine_selector_names_unpushed_source_work_before_connecting_and_keeps_legacy_identity() {
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
    let fixture = support::register_task_without_pr(
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
    repo.create_file("unfinished.txt", "source work");
    let args = ["--task", "INF-123", "skill", "implement"].map(str::to_string);
    let cli = loopflow::lf::commands::ssh::parse_remote_command(&args).unwrap();
    let invoke = || {
        loopflow::lf::commands::ssh::run("unreachable.invalid", false, &cli, &args, None)
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
