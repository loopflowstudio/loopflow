mod support;

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;
use std::process::Command;

use loopflow::durable::TaskId;
use loopflow::ops::resolve_work_binding;
use loopflow::planning::LinearIssueId;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::TestRepo;
use serde_json::json;
use sha2::{Digest, Sha256};

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
    json!({
        "branch": branch, "commit": commit,
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
fn machine_selector_runs_a_skill_in_the_adopted_checkout_and_reuses_it() {
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.push();
    let target = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let origin = git(repo.path(), &["remote", "get-url", "origin"]);
    git(target.path(), &["clone", &origin, "repo"]);
    let branch = "test/ssh-pushed-code";
    repo.create_branch(branch);
    repo.create_file("implementation.txt", "already implemented\n");
    repo.create_file(
        ".lf/skills/inspect-code.md",
        "---\nagent: claude\n---\nInspect implementation.txt.\n",
    );
    repo.stage_all();
    repo.commit("Implementation and skill absent from the target clone");
    git(repo.path(), &["push", "origin", branch]);

    let origin_home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(&[], Some(origin_home.path()));
    let fixture = support::register_unrun_task(
        &origin_home.path().join(".lf"),
        &repo.path().canonicalize().unwrap(),
        branch,
        &repo.head_sha(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let target_store = runtime
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            target.path().join(".lf/loopflow.db"),
        )))
        .unwrap();
    let account_home = target.path().join(".lf/accounts/claude/fixture");
    fs::create_dir_all(&account_home).unwrap();
    let credential = json!({"claudeAiOauth": {
        "accessToken": "synthetic-fixture-token", "expiresAt": 4102444800000i64,
    }})
    .to_string();
    fs::write(account_home.join(".credentials.json"), &credential).unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    runtime
        .block_on(
            target_store.upsert_provider_account(&loopflow::store::ProviderAccount {
                provider: "claude".into(),
                account_id: loopflow::store::ProviderAccountId::parse("fixture").unwrap(),
                home: Some(account_home),
                login_email: Some(
                    loopflow::profile::EmailAddress::parse("fixture@example.test").unwrap(),
                ),
                observed_email: Some("fixture@example.test".into()),
                observed_subject: Some("fixture".into()),
                observed_credential_digest: Some(format!(
                    "{:x}",
                    Sha256::digest(credential.as_bytes())
                )),
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
            }),
        )
        .unwrap();
    let bin = target.path().join(".local/bin");
    fs::create_dir_all(&bin).unwrap();
    symlink(env!("CARGO_BIN_EXE_lf"), bin.join("lf")).unwrap();
    for (name, script) in [
        ("gh", "#!/bin/sh\n[ \"$1\" = auth ] && exit 1\necho '[]'\n"),
        ("security", "#!/bin/sh\nexit 1\n"),
        // Execute exactly the stdin delivered to SSH in an independent environment.
        // The SSH network and authentication are simulated; dispatch and Git are real.
        (
            "ssh",
            r#"#!/bin/sh
for arg in "$@"; do remote_command="$arg"; done
exec /usr/bin/env -i HOME="$FIXTURE_TARGET" LF_HOME="$FIXTURE_TARGET/.lf" \
    LF_BIN="$LF_BIN" PATH="$FIXTURE_TARGET/.local/bin:/usr/bin:/bin" \
    GIT_ALLOW_PROTOCOL=file GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null \
    /bin/bash -c "$remote_command"
"#,
        ),
        (
            "claude",
            r#"#!/bin/sh
[ "$1" = --version ] && exit 0
[ -z "$LF_TASK_SOURCE" ] || exit 41
cat implementation.txt >> "$LF_HOME/seen-implementation"
pwd >> "$LF_HOME/seen-checkout"
read -r input
echo '{"type":"system","subtype":"init","session_id":"ssh-fixture"}'
echo '{"type":"result","subtype":"success","is_error":false,"result":"inspected","session_id":"ssh-fixture"}'
"#,
        ),
    ] {
        let path = bin.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let command = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .current_dir(repo.path())
            .env_clear()
            .env("HOME", origin_home.path())
            .env("LF_HOME", origin_home.path().join(".lf"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("FIXTURE_TARGET", target.path())
            .env("GIT_ALLOW_PROTOCOL", "file")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(args)
            .output()
            .unwrap()
    };
    let added = command(&["machine", "add", "fixture", "--repo", "repo", "--json"]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    let machine: serde_json::Value = serde_json::from_slice(&added.stdout).unwrap();
    let invoke = |selector: &str, task: &str| {
        command(&[
            "--machine",
            selector,
            "--task",
            task,
            "--isolate",
            "--batch",
            "skill",
            "inspect-code",
        ])
    };
    for selector in ["fixture", machine["id"].as_str().unwrap()] {
        let output = invoke(selector, fixture.task.id.as_str());
        assert!(
            output.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read_to_string(target.path().join(".lf/seen-implementation")).unwrap(),
        "already implemented\nalready implemented\n"
    );
    let tasks = runtime.block_on(target_store.list_tasks(None)).unwrap();
    assert_eq!(tasks.len(), 1);
    let task = &tasks[0];
    assert_eq!(
        task.id,
        TaskId::from_issue(fixture.task.plan.linear_id.as_ref().unwrap())
    );
    assert_ne!(task.id, fixture.task.id);
    assert_eq!(
        runtime
            .block_on(target_store.task_prs(&task.id))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        loopflow::engine::worktrees::list_worktrees(&target.path().join("repo"))
            .unwrap()
            .len(),
        2
    );
    let seen = fs::read_to_string(target.path().join(".lf/seen-checkout")).unwrap();
    assert!(seen
        .lines()
        .all(|path| Path::new(path) == task.worktree().unwrap()));
    assert_eq!(
        loopflow::engine::git::current_branch(task.worktree().unwrap())
            .unwrap()
            .as_deref(),
        Some(branch)
    );

    // A later source commit must not reset a retained, dirty target checkout.
    fs::write(
        task.worktree().unwrap().join("local-notes"),
        "keep local work",
    )
    .unwrap();
    let retained_head = loopflow::engine::git::rev_parse(task.worktree().unwrap(), "HEAD").unwrap();
    repo.create_file("implementation.txt", "new pushed implementation\n");
    repo.stage_all();
    repo.commit("Further source work");
    git(repo.path(), &["push", "origin", branch]);
    let output = invoke("fixture", "INF-123");
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("lf sync") && error.contains(&repo.head_sha()),
        "{error}"
    );
    assert_eq!(
        loopflow::engine::git::rev_parse(task.worktree().unwrap(), "HEAD").unwrap(),
        retained_head
    );
    assert_eq!(
        fs::read_to_string(task.worktree().unwrap().join("local-notes")).unwrap(),
        "keep local work"
    );
}

#[test]
fn cold_machines_adopt_pushed_code_once_with_the_same_task_id() {
    let repo = repository();
    let machines = tempfile::tempdir().unwrap();
    let remote = git(repo.path(), &["remote", "get-url", "origin"]);
    // Clone before the branch exists: neither machine has a remote-tracking ref.
    for name in ["first", "second"] {
        git(machines.path(), &["clone", &remote, name]);
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
        assert_eq!(task.plan.pm_snapshot_synced_at, Some(1791360000));
        assert_eq!(&first.cwd, task.worktree().unwrap());
        assert_eq!(
            fs::read_to_string(task.worktree().unwrap().join("implementation.txt")).unwrap(),
            "already implemented\n"
        );
        assert_eq!(
            loopflow::engine::git::rev_parse(task.worktree().unwrap(), "HEAD").unwrap(),
            head
        );
        fs::write(
            task.worktree().unwrap().join("local-notes"),
            "keep local work",
        )
        .unwrap();
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
            fs::read_to_string(task.worktree().unwrap().join("local-notes")).unwrap(),
            "keep local work"
        );
        let workflows: i64 = rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row("SELECT count(*) FROM task_workflows", [], |row| row.get(0))
            .unwrap();
        assert_eq!(workflows, 0);
        std::env::remove_var("LF_TASK_SOURCE");
        fs::remove_dir_all(task.worktree().unwrap()).unwrap();
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
    assert_ne!(
        fixture.task.id,
        TaskId::from_issue(fixture.task.plan.linear_id.as_ref().unwrap())
    );
    repo.create_file("unfinished.txt", "source work");
    let args = ["--task", "INF-123", "context", "--json"].map(str::to_string);
    let invoke = || {
        loopflow::lf::commands::ssh::run("unreachable.invalid", false, &args)
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
