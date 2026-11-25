use crate::support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow_execution::ops::task::{self, task_status};
use loopflow_execution::ops::task_actions::TaskAction;
use loopflow_execution::work::task::{GithubPr, PrPublication, TaskEventKind};
use loopflow_test_support::TestRepo;
use support::{register_task_with_pr, EnvGuard};

fn unbound_command(cli: &Path, repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(cli);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LF_") {
            command.env_remove(name);
        }
    }
    command.current_dir(repo).args(args);
    command
}

#[test]
fn stacked_checkout_preserves_inherited_scratch_and_edits() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    repo.create_branch("parent");
    repo.create_file("scratch/design.md", "parent design");
    repo.create_file("scratch/review/notes.md", "parent review");
    repo.stage_all();
    repo.commit("Parent notes");
    let parent_head = repo.head_sha();
    let parent = register_task_with_pr(home.path(), repo.path(), "parent", &parent_head);
    let child =
        support::register_sibling_task(&parent, "INF-124", "child", &target.path().join("child"));
    let worktree = child.worktree.as_ref().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut parent_pr = parent.pr.clone();
    parent_pr.publication = Some(PrPublication {
        requested_at: parent_pr.created_at,
        presentation: None,
        github: Some(GithubPr {
            number: 41,
            url: "https://github.com/fixture/repo/pull/41".into(),
            head_sha: Some(parent_pr.base_commit.clone()),
        }),
        merge: None,
    });
    runtime
        .block_on(parent.store.update_task_pr(&parent_pr))
        .unwrap();
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute("DELETE FROM task_prs WHERE task_id=?1", [child.id.as_str()])
        .unwrap();
    runtime
        .block_on(
            parent
                .store
                .stack_task_placement(&child, &parent.pr.id, &child.base_commit),
        )
        .unwrap();

    let checkout = || {
        loopflow_execution::ops::task::task_checkout(
            repo.path(),
            "INF-124",
            loopflow_execution::ops::task::TaskCheckoutOptions::default(),
        )
        .unwrap()
    };
    checkout();
    assert_eq!(
        fs::read_to_string(worktree.join("scratch/design.md")).unwrap(),
        "parent design"
    );
    assert!(runtime
        .block_on(parent.store.active_task_pr(&child.id))
        .unwrap()
        .is_none());
    assert_eq!(
        loopflow_execution::git::rev_parse(worktree, "HEAD").unwrap(),
        parent_head
    );
    fs::write(worktree.join("scratch/design.md"), "child design").unwrap();
    checkout();
    assert_eq!(
        loopflow_execution::git::rev_parse(worktree, "HEAD").unwrap(),
        parent_head
    );
    assert_eq!(
        fs::read_to_string(worktree.join("scratch/design.md")).unwrap(),
        "child design"
    );
    assert_eq!(repo.head_sha(), parent_head);
    assert_eq!(
        fs::read_to_string(repo.path().join("scratch/design.md")).unwrap(),
        "parent design"
    );
    assert!(repo.path().join("scratch/review/notes.md").exists());
    fs::write(worktree.join("scratch/child-only.md"), "child notes").unwrap();
    for args in [vec!["add", "scratch"], vec!["commit", "-m", "Child notes"]] {
        assert!(Command::new("git")
            .current_dir(worktree)
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    assert_eq!(
        loopflow_execution::ops::scratch::clear(worktree).unwrap(),
        1
    );
    assert_eq!(
        fs::read_to_string(worktree.join("scratch/design.md")).unwrap(),
        "child design"
    );
    assert!(worktree.join("scratch/review/notes.md").exists());
    assert!(!worktree.join("scratch/child-only.md").exists());
}

#[test]
fn checkout_restores_exact_task_history_from_a_dirty_checkout() {
    let repo = TestRepo::new();
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["branch", "test/checkout-recovery"])
        .status()
        .unwrap()
        .success());
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let mut fixture = register_task_with_pr(
        home.path(),
        repo.path(),
        "test/checkout-recovery",
        &repo.head_sha(),
    );
    fixture.task.worktree = Some(target.path().join("checkout"));
    let worktree = fixture.task.worktree.as_ref().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)
             ON CONFLICT(task_id) DO UPDATE SET worktree=excluded.worktree",
            rusqlite::params![fixture.task.id.as_str(), worktree.display().to_string()],
        )
        .unwrap();
    fixture.pr = runtime
        .block_on(fixture.store.active_task_pr(&fixture.task.id))
        .unwrap()
        .unwrap();
    let invoking = repo.create_named_worktree("dirty-invoker");
    fs::write(repo.path().join("main-notes"), "keep main edits").unwrap();
    fs::write(invoking.join("caller-notes"), "keep caller edits").unwrap();
    let checkout = || {
        unbound_command(
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &invoking,
            &["task", "checkout", "INF-123", "--json"],
        )
        .env("LF_HOME", home.path())
        .output()
        .unwrap()
    };
    let first = checkout();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        loopflow_execution::git::rev_parse(worktree, "HEAD").unwrap(),
        fixture.pr.base_commit
    );
    fs::write(worktree.join("work.txt"), "committed Task work").unwrap();
    for args in [
        vec!["add", "work.txt"],
        vec!["commit", "-m", "Preserve Task work"],
    ] {
        assert!(Command::new("git")
            .current_dir(worktree)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    let head = loopflow_execution::git::rev_parse(worktree, "HEAD").unwrap();
    fs::remove_dir_all(worktree).unwrap();
    let restored = checkout();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        loopflow_execution::git::rev_parse(worktree, "HEAD").unwrap(),
        head
    );
    assert_eq!(
        fs::read_to_string(worktree.join("work.txt")).unwrap(),
        "committed Task work"
    );
    // Losing the branch as well must not silently restart from the base.
    fs::remove_dir_all(worktree).unwrap();
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["update-ref", "-d", "refs/heads/test/checkout-recovery"])
        .status()
        .unwrap()
        .success());
    let missing = checkout();
    assert!(!missing.status.success());
    assert!(!worktree.exists());
    let recover = |commit: &str| {
        unbound_command(
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &invoking,
            &[
                "task",
                "checkout",
                "INF-123",
                "--recover-from",
                commit,
                "--json",
            ],
        )
        .env("LF_HOME", home.path())
        .output()
        .unwrap()
    };
    assert!(!recover("missing-object").status.success());
    assert!(!worktree.exists());
    let recovered = recover(&head);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert_eq!(
        loopflow_execution::git::rev_parse(worktree, "HEAD").unwrap(),
        head
    );
    assert_eq!(
        fs::read_to_string(worktree.join("work.txt")).unwrap(),
        "committed Task work"
    );
    assert!(recover(&head).status.success());
    assert!(!recover(&fixture.pr.base_commit).status.success());
    assert_eq!(
        loopflow_execution::git::rev_parse(worktree, "HEAD").unwrap(),
        head
    );
    let persisted = runtime
        .block_on(fixture.store.get_task(&fixture.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(persisted.worktree, fixture.task.worktree);
    assert_eq!(
        runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap()
            .unwrap(),
        fixture.pr
    );
    let events = runtime
        .block_on(fixture.store.task_events_after(&fixture.task.id, 0))
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, TaskEventKind::CheckoutReady { .. }))
            .count(),
        1
    );
    fs::remove_dir_all(worktree).unwrap();
    fs::create_dir(worktree).unwrap();
    fs::write(worktree.join("notes"), "unregistered work").unwrap();
    let occupied = checkout();
    assert!(!occupied.status.success());
    assert!(String::from_utf8_lossy(&occupied.stderr).contains("occupied"));
    assert_eq!(
        fs::read_to_string(worktree.join("notes")).unwrap(),
        "unregistered work"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("main-notes")).unwrap(),
        "keep main edits"
    );
    assert_eq!(
        fs::read_to_string(invoking.join("caller-notes")).unwrap(),
        "keep caller edits"
    );
}

#[test]
fn initializing_worktree_keeps_status_wait_and_roadmap_readable() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/initializing-task";
    repo.create_branch(branch);
    let mut task = register_task_with_pr(
        home.path(),
        &repo.path().canonicalize().unwrap(),
        branch,
        &base,
    );
    let missing_worktree = home.path().join("not-yet-created-worktree");
    task.task.worktree = Some(missing_worktree.clone());
    let runtime = tokio::runtime::Runtime::new().expect("initialization fixture runtime");
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)
             ON CONFLICT(task_id) DO UPDATE SET worktree=excluded.worktree",
            rusqlite::params![
                task.task.id.as_str(),
                missing_worktree.display().to_string()
            ],
        )
        .expect("seed declared Task worktree");
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
    let run_lf = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env_remove("LF_WAVE_ID")
            .current_dir(repo.path())
            .output()
            .expect("run lf read surface")
    };
    let status = run_lf(&["desktop", "task", "INF-123", "--json"]);
    assert!(
        status.status.success(),
        "status stays readable: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    let status = &status["execution"];
    assert_eq!(status["execution"]["state"], "idle");
    assert_eq!(status["work"]["sessions"], serde_json::json!([]));
    assert_eq!(status["work"]["graph_processes"], serde_json::json!([]));
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
    assert_eq!(wait["task"]["actions"], status["actions"]);

    let roadmap = run_lf(&["desktop", "plan", "task-pr-tests", "--json"]);
    assert!(
        roadmap.status.success(),
        "roadmap stays readable: {}",
        String::from_utf8_lossy(&roadmap.stderr)
    );
    let roadmap: serde_json::Value = serde_json::from_slice(&roadmap.stdout).expect("roadmap JSON");
    assert_eq!(roadmap["tasks"]["state"], "ok", "roadmap: {roadmap:#}");
    let roadmap_task = &roadmap["tasks"]["items"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "INF-123");
    assert_eq!(roadmap_task["actions"]["recommended"], "no_action");
    assert!(roadmap_task["condition"]["reason"]
        .as_str()
        .expect("roadmap condition reason")
        .contains("is initializing worktree"));
    let projected = task_status(
        repo.path(),
        Some("INF-123"),
        loopflow::lf::navigation::command_merges,
    )
    .expect("read Task")
    .execution
    .expect("execution");
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
    let stale = run_lf(&["desktop", "task", "INF-123", "--json"]);
    assert!(
        stale.status.success(),
        "stale initialization stays readable"
    );
    let stale: serde_json::Value =
        serde_json::from_slice(&stale.stdout).expect("stale status JSON");
    let stale = &stale["execution"];
    assert_eq!(stale["actions"]["recommended"], "no_action");
    assert!(stale["actions"]["reason"]
        .as_str()
        .expect("stale action reason")
        .contains("initialization did not complete"));
    let stale_roadmap = run_lf(&["desktop", "plan", "task-pr-tests", "--json"]);
    assert!(
        stale_roadmap.status.success(),
        "stale roadmap stays readable"
    );
    let stale_roadmap: serde_json::Value =
        serde_json::from_slice(&stale_roadmap.stdout).expect("stale roadmap JSON");
    let stale_condition = &stale_roadmap["tasks"]["items"][0]["condition"];
    assert_eq!(stale_condition["state"], "blocked");
    assert!(stale_condition["reason"]
        .as_str()
        .expect("stale roadmap reason")
        .contains("initialization did not complete"));
}

#[test]
fn missing_worktree_status_is_actionable_and_read_only() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let (task, missing_path, branch) = {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/missing-worktree";
        repo.create_branch(branch);
        let task = register_task_with_pr(home.path(), repo.path(), branch, &base);
        let recorded = task.task.worktree.clone().expect("recorded checkout");
        (task, recorded, branch.to_string())
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

    let snapshot = task_status(
        &missing_path,
        Some("INF-123"),
        loopflow::lf::navigation::command_merges,
    )
    .expect("status survives the absent worktree")
    .execution
    .expect("execution");

    assert_eq!(snapshot.actions.recommended, Some(TaskAction::NoAction));
    assert!(snapshot
        .actions
        .reason
        .contains(&missing_path.display().to_string()));
    assert!(snapshot.actions.reason.contains(&branch));
    assert!(snapshot.actions.reason.contains("lf task run INF-123"));
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

fn close_fixture_pr(fixture: &support::RegisteredTask, checkout: &Path) {
    let database = rusqlite::Connection::open(&fixture.database).unwrap();
    database
        .execute(
            "UPDATE task_checkouts SET worktree=?2 WHERE task_id=?1",
            rusqlite::params![fixture.task.id.as_str(), checkout.to_str()],
        )
        .unwrap();
    database
        .execute(
            "UPDATE task_prs SET publication_requested_at=1,github_number=7,
         github_url='https://example.test/pr/7',abandoned_at=2 WHERE id=?1",
            [fixture.pr.id.as_str()],
        )
        .unwrap();
}

fn abandon_fixture(fixture: &support::RegisteredTask) {
    let work = loopflow_model::durable::WorkRef::Task(fixture.task.id.clone());
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(fixture.store.abandon(&work, "Stopped"))
        .unwrap();
}

#[test]
fn reopen_archives_the_closed_pr_and_checks_out_from_current_main() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[("gh", "#!/bin/sh\nexit 1\n")], home.path());
    let repo = TestRepo::new();
    let fixture =
        register_task_with_pr(home.path(), repo.path(), "test/abandoned", &repo.head_sha());
    close_fixture_pr(&fixture, &home.path().join("deleted-checkout"));
    abandon_fixture(&fixture);
    let read = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };

    let reopened = read(&["task", "reopen", "INF-123", "--json"]);
    assert_eq!(reopened["state"], "started");
    assert!(reopened["worktree"].is_null());
    let status = task_status(repo.path(), Some("INF-123"))
        .unwrap()
        .execution
        .unwrap();
    assert!(status.pr.is_none());
    assert_eq!(
        status.actions.recommended,
        Some(TaskAction::Run),
        "{:?}",
        status.actions
    );
    let plan = read(&["desktop", "plan", "--task", "INF-123", "--json"]);
    let row = &plan["tasks"]["items"][0];
    assert_eq!(row["task"]["identifier"], "INF-123");
    assert_eq!(row["actions"]["recommended"], "run");
    assert_eq!(row["run_control"], serde_json::json!({"unavailable": null}));

    repo.create_file("after-abandon.txt", "main moved");
    repo.stage_all();
    repo.commit("Advance main");
    repo.push();
    let placed = task::task_checkout(repo.path(), "INF-123", Default::default()).unwrap();
    let checkout = placed.worktree().unwrap();
    assert_eq!(
        loopflow_execution::git::rev_parse(checkout, "HEAD").unwrap(),
        repo.head_sha()
    );
    assert_eq!(placed.base_commit, repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let event = runtime
        .block_on(fixture.store.latest_task_event(&placed.id))
        .unwrap()
        .unwrap();
    assert!(
        matches!(event.kind, TaskEventKind::CheckoutReady { base_commit, .. } if base_commit == placed.base_commit)
    );

    let context = task::task_pr_context(checkout).unwrap().unwrap();
    let body = format!(
        "> **Task:** [{} · {}]({})",
        context.title,
        context.identifier,
        context.url.unwrap()
    );
    assert!(task::request_task_pr_publication(checkout, "Second attempt", &body).unwrap());
    let history = runtime
        .block_on(fixture.store.task_prs(&placed.id))
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].github().unwrap().number, 7);
    assert!(history[0].abandoned_at.is_some());
    assert_eq!(history[1].sequence, 2);
    assert_eq!(
        runtime
            .block_on(fixture.store.active_task_pr(&placed.id))
            .unwrap(),
        Some(history[1].clone())
    );
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["worktree", "remove"])
        .arg(checkout)
        .status()
        .unwrap()
        .success());
}

#[test]
fn reopened_checkout_adopts_a_newly_published_remote_branch() {
    let home = tempfile::tempdir().unwrap();
    let gh = r#"#!/bin/sh
case "$1 $2" in
    "--version ") echo 'gh version fixture' ;;
    "pr list") echo '[{"number":8,"url":"https://example.test/pr/8","state":"OPEN","isDraft":false,"baseRefName":"main"}]' ;;
    *) exit 1 ;;
esac
"#;
    let _env = EnvGuard::with_lf_home(&[("gh", gh)], home.path());
    let repo = TestRepo::new();
    let branch = "test/restored";
    let fixture = register_task_with_pr(home.path(), repo.path(), branch, &repo.head_sha());
    close_fixture_pr(&fixture, &home.path().join("deleted-checkout"));
    abandon_fixture(&fixture);
    assert!(task::task_reopen("INF-123", None)
        .unwrap()
        .worktree
        .is_none());
    // A collaborator publishes the branch again before this Machine checks it out.
    repo.create_branch(branch);
    repo.create_file("collaborator.txt", "published");
    repo.stage_all();
    repo.commit("Restore remote branch");
    repo.push_new_branch(branch);
    repo.checkout("main");
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["branch", "-D", branch])
        .status()
        .unwrap()
        .success());
    let placed = task::task_checkout(repo.path(), "INF-123", Default::default()).unwrap();
    let checkout = placed.worktree().unwrap();
    assert!(checkout.join("collaborator.txt").exists());
    let pr = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(fixture.store.active_task_pr(&placed.id))
        .unwrap()
        .unwrap();
    assert_eq!((pr.sequence, pr.github().unwrap().number), (2, 8));
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["worktree", "remove"])
        .arg(checkout)
        .status()
        .unwrap()
        .success());
}

#[test]
fn reopen_keeps_surviving_checkout_or_branch_and_allows_completion() {
    for keep_checkout in [true, false] {
        let repo = TestRepo::new();
        let machine = tempfile::tempdir().unwrap();
        let _env = EnvGuard::with_lf_home(&[], machine.path());
        let branch = "test/retained";
        let base = repo.head_sha();
        repo.create_branch(branch);
        repo.create_file("retained.txt", "local work");
        repo.stage_all();
        repo.commit("Retain this commit");
        let retained = repo.head_sha();
        repo.checkout("main");
        let checkout = machine.path().join("checkout");
        let fixture = register_task_with_pr(machine.path(), repo.path(), branch, &base);
        if keep_checkout {
            // Even a surviving directory whose branch has gone can hold local edits.
            fs::create_dir(&checkout).unwrap();
            fs::write(checkout.join("draft.txt"), "draft").unwrap();
            assert!(Command::new("git")
                .current_dir(repo.path())
                .args(["branch", "-D", branch])
                .status()
                .unwrap()
                .success());
        }
        close_fixture_pr(&fixture, &checkout);
        if keep_checkout {
            // A PR closed externally stays current while the Task itself is open.
            let status = task_status(repo.path(), Some("INF-123"))
                .unwrap()
                .execution
                .unwrap();
            assert_eq!(status.actions.recommended, Some(TaskAction::NoAction));
            assert!(
                status.actions.reason.contains("PR closed without merge"),
                "{:?}",
                status.actions
            );
        }
        abandon_fixture(&fixture);
        let reopened = task::task_reopen("INF-123", None).unwrap();
        assert_eq!(reopened.worktree().unwrap(), &checkout);
        if keep_checkout {
            assert_eq!(
                fs::read_to_string(checkout.join("draft.txt")).unwrap(),
                "draft"
            );
        } else {
            task::task_checkout(repo.path(), "INF-123", Default::default()).unwrap();
            assert_eq!(
                loopflow_execution::git::rev_parse(&checkout, "HEAD").unwrap(),
                retained
            );
        }
        task::task_complete(repo.path(), "INF-123", None, None).unwrap();
        let status = task_status(repo.path(), Some("INF-123"))
            .unwrap()
            .execution
            .unwrap();
        assert_eq!(status.status, loopflow_execution::durable::TaskState::Done);
        assert!(status.pr.is_none());
    }
}

#[test]
fn research_checkout_restores_and_reads_files_without_a_pull_request() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let parent = register_task_with_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let research = support::register_sibling_task(
        &parent,
        "INF-124",
        "research",
        &target.path().join("research"),
    );
    let worktree = research.worktree.as_ref().unwrap();

    let restored = loopflow_execution::ops::task::task_checkout(
        repo.path(),
        "INF-124",
        loopflow_execution::ops::task::TaskCheckoutOptions::default(),
    )
    .unwrap();
    assert_eq!(restored.id, research.id);
    fs::write(worktree.join("findings.md"), "Use the existing store.\n").unwrap();
    let result = Command::new("git")
        .current_dir(worktree)
        .args(["add", "findings.md"])
        .status()
        .unwrap();
    assert!(result.success());
    let result = Command::new("git")
        .current_dir(worktree)
        .args(["commit", "-m", "Record research findings"])
        .status()
        .unwrap();
    assert!(result.success());
    fs::write(worktree.join("draft.md"), "Keep this draft.\n").unwrap();
    let changes = loopflow_execution::ops::task::task_changes("INF-124", "parent", None).unwrap();
    assert!(changes
        .files
        .iter()
        .any(|file| file.path == "findings.md" && file.committed));
    assert!(changes
        .files
        .iter()
        .any(|file| file.path == "draft.md" && file.untracked));
    let snapshot = loopflow_execution::ops::task::task_snapshot(
        &restored,
        loopflow::lf::navigation::command_merges,
    )
    .unwrap();
    assert!(snapshot.pr.is_none());
    assert_eq!(snapshot.branch.as_deref(), Some("research"));
    assert!(runtime
        .block_on(parent.store.task_prs(&research.id))
        .unwrap()
        .is_empty());
    let json = serde_json::to_value(snapshot).unwrap();
    assert!(json["pr"].is_null());
    assert!(json.get("prs").is_none());
    assert!(json.get("active_pr").is_none());
    let binding = runtime
        .block_on(loopflow_execution::ops::resolve_work_binding(
            &std::sync::Arc::new(parent.store),
            worktree,
            "task:INF-124",
        ))
        .unwrap();
    assert!(binding.context.contains("Branch: research"));
    assert!(!binding.context.contains("PR 1:"));
}
