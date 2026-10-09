mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::ops::task::task_status;
use loopflow::ops::task_actions::TaskAction;
use loopflow::work::task::{GithubPr, PrPublication, TaskEventKind};
use loopflow_test_support::TestRepo;
use support::{register_unrun_task, EnvGuard};

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
fn stacked_checkout_starts_with_one_scratch_deletion_commit() {
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
    let parent = register_unrun_task(home.path(), repo.path(), "parent", &parent_head);
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
    let pr = runtime
        .block_on(parent.store.active_task_pr(&child.id))
        .unwrap()
        .unwrap();
    runtime
        .block_on(parent.store.stack_task_pr(&pr, &parent.pr.id))
        .unwrap();

    let checkout = || {
        loopflow::ops::task::task_checkout(
            repo.path(),
            "INF-124",
            loopflow::ops::task::TaskCheckoutOptions::default(),
        )
        .unwrap()
    };
    checkout();
    assert!(!worktree.join("scratch").exists());
    assert_eq!(
        loopflow::engine::git::rev_parse(worktree, "HEAD^").unwrap(),
        parent_head
    );
    let subject = Command::new("git")
        .current_dir(worktree)
        .args(["log", "-1", "--format=%s"])
        .output()
        .unwrap();
    assert!(subject.status.success());
    assert_eq!(
        String::from_utf8_lossy(&subject.stdout).trim(),
        "Clear inherited scratch"
    );
    let child_head = loopflow::engine::git::rev_parse(worktree, "HEAD").unwrap();
    fs::create_dir(worktree.join("scratch")).unwrap();
    fs::write(worktree.join("scratch/design.md"), "child design").unwrap();
    checkout();
    assert_eq!(
        loopflow::engine::git::rev_parse(worktree, "HEAD").unwrap(),
        child_head
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
}

#[test]
fn checkout_restores_exact_task_history_from_a_dirty_checkout() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let mut fixture = register_unrun_task(
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
            "UPDATE tasks SET worktree=?2 WHERE id=?1",
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
        loopflow::engine::git::rev_parse(worktree, "HEAD").unwrap(),
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
    let head = loopflow::engine::git::rev_parse(worktree, "HEAD").unwrap();
    fs::remove_dir_all(worktree).unwrap();
    let restored = checkout();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        loopflow::engine::git::rev_parse(worktree, "HEAD").unwrap(),
        head
    );
    assert_eq!(
        fs::read_to_string(worktree.join("work.txt")).unwrap(),
        "committed Task work"
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
            .filter(|event| matches!(event.kind, TaskEventKind::PrStarted { .. }))
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
    let mut task = register_unrun_task(home.path(), repo.path(), branch, &base);
    let missing_worktree = home.path().join("not-yet-created-worktree");
    task.task.worktree = Some(missing_worktree.clone());
    let runtime = tokio::runtime::Runtime::new().expect("initialization fixture runtime");
    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE tasks SET worktree=?2 WHERE id=?1",
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
    let status = run_lf(&["task", "status", "INF-123", "--json"]);
    assert!(
        status.status.success(),
        "status stays readable: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    let status = &status["execution"];
    assert_eq!(status["execution"]["state"], "idle");
    assert_eq!(status["work"]["sessions"], serde_json::json!([]));
    assert_eq!(status["work"]["flow_processes"], serde_json::json!([]));
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
    assert_eq!(wave["tasks"]["state"], "ok", "roadmap wave: {wave:#}");
    let roadmap_task = &wave["tasks"]["items"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "INF-123");
    assert_eq!(roadmap_task["actions"]["recommended"], "no_action");
    assert!(roadmap_task["condition"]["reason"]
        .as_str()
        .expect("roadmap condition reason")
        .contains("is initializing worktree"));
    let projected = task_status(repo.path(), Some("INF-123"))
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
    let stale = run_lf(&["task", "status", "INF-123", "--json"]);
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
    let stale_roadmap = run_lf(&["roadmap", "--wave", "task-pr-tests", "--json"]);
    assert!(
        stale_roadmap.status.success(),
        "stale roadmap stays readable"
    );
    let stale_roadmap: serde_json::Value =
        serde_json::from_slice(&stale_roadmap.stdout).expect("stale roadmap JSON");
    let stale_condition = &stale_roadmap["waves"][0]["tasks"]["items"][0]["condition"];
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

    let snapshot = task_status(&missing_path, Some("INF-123"))
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

#[test]
fn saved_task_checkout_works_offline_with_and_without_linear() {
    for (connected, explicit_name) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let fixture = register_unrun_task(home.path(), repo.path(), "main", &repo.head_sha());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let original_prs = runtime
            .block_on(fixture.store.task_prs(&fixture.task.id))
            .unwrap();
        let mut snapshot = runtime
            .block_on(fixture.store.pm_snapshot(&fixture.task.wave_id))
            .unwrap()
            .unwrap();
        let mut item = snapshot.snapshot.items[0].clone();
        item.id = "offline-placement-issue".into();
        item.identifier = "INF-456".into();
        item.name = "Saved Task placement".into();
        item.branch_name = None;
        snapshot.snapshot.items.push(item);
        runtime
            .block_on(fixture.store.put_pm_snapshot(snapshot, None))
            .unwrap();
        let task = runtime
            .block_on(fixture.store.get_task_by_issue("INF-456"))
            .unwrap()
            .unwrap();
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        if connected {
            fs::create_dir_all(repo.path().join(".lf")).unwrap();
            fs::write(
                repo.path().join(".lf/config.yaml"),
                "pm:\n  provider: linear\n  linear_team: unreachable-team\n",
            )
            .unwrap();
        } else {
            conn.execute(
                "UPDATE tasks SET external_issue_id=NULL WHERE id=?1",
                [task.id.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE projects SET external_project_id=NULL WHERE id=?1",
                [task.project_id.as_str()],
            )
            .unwrap();
        }
        // A saved edit must survive allocation, even while an older provider fact remains.
        conn.execute(
            "UPDATE tasks SET issue_title='Retain all five title words',planning_revision=3 WHERE id=?1",
            [task.id.as_str()],
        )
        .unwrap();
        // Provider inventory is observation history, not a prerequisite for saved placement.
        conn.execute("DELETE FROM pm_items", []).unwrap();
        conn.execute("DELETE FROM pm_wave_projects", []).unwrap();
        let saved = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        let checkout = || {
            let mut args = vec!["task", "checkout", task.id.as_str(), "--json"];
            if explicit_name {
                args.extend(["--name", "offline-placement"]);
            }
            let mut command =
                unbound_command(Path::new(env!("CARGO_BIN_EXE_lf")), repo.path(), &args);
            for (name, _) in std::env::vars_os() {
                if name.to_string_lossy().starts_with("LINEAR_") {
                    command.env_remove(name);
                }
            }
            command
                .env("HOME", home.path())
                .env("LF_HOME", home.path())
                .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
                .env("LF_USER_NAME", "Fixture Person")
                .output()
                .unwrap()
        };
        conn.execute(
            "UPDATE tasks SET planning_state='completed',planning_completed=1 WHERE id=?1",
            [task.id.as_str()],
        )
        .unwrap();
        let refused = checkout();
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("terminal planning state"));
        assert!(runtime
            .block_on(fixture.store.task_prs(&task.id))
            .unwrap()
            .is_empty());
        assert!(runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap()
            .worktree
            .is_none());
        conn.execute(
            "UPDATE tasks SET planning_state='unstarted',planning_completed=0 WHERE id=?1",
            [task.id.as_str()],
        )
        .unwrap();
        let first = checkout();
        assert!(
            first.status.success(),
            "{}",
            String::from_utf8_lossy(&first.stderr)
        );
        let placed = runtime
            .block_on(fixture.store.get_task(&task.id))
            .unwrap()
            .unwrap();
        assert_eq!(placed.plan, saved.plan);
        assert_eq!(placed.project_id, saved.project_id);
        assert_eq!(placed.wave_id, saved.wave_id);
        assert_eq!(
            placed.workspace_slug,
            if explicit_name {
                "offline-placement".to_string()
            } else {
                format!("retain-all-five-title-{}", &task.id.as_str()[5..17])
            }
        );
        let worktree = placed.worktree.as_ref().unwrap();
        assert!(worktree.join(".git").exists());
        let prs = runtime.block_on(fixture.store.task_prs(&task.id)).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(
            prs[0].branch,
            format!(
                "lf/{}/retain-all-five-title-words",
                task.id.as_str().trim_start_matches("task_")
            )
        );
        let events = runtime
            .block_on(fixture.store.task_events_after(&task.id, 0))
            .unwrap();
        fs::remove_dir_all(worktree).unwrap();
        let retried = checkout();
        assert!(
            retried.status.success(),
            "{}",
            String::from_utf8_lossy(&retried.stderr)
        );
        assert!(worktree.join(".git").exists());
        assert_eq!(
            runtime
                .block_on(fixture.store.get_task(&task.id))
                .unwrap()
                .unwrap(),
            placed
        );
        assert_eq!(
            runtime.block_on(fixture.store.task_prs(&task.id)).unwrap(),
            prs
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.task_events_after(&task.id, 0))
                .unwrap(),
            events
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.task_prs(&fixture.task.id))
                .unwrap(),
            original_prs
        );
        let executions: i64 = conn.query_row(
            "SELECT (SELECT count(*) FROM agent_sessions WHERE task_id=?1) + (SELECT count(*) FROM task_workflows WHERE task_id=?1)",
            [task.id.as_str()], |row| row.get(0),
        ).unwrap();
        assert_eq!(executions, 0);
        fs::create_dir_all(worktree.join(".lf/flows")).unwrap();
        fs::write(
            worktree.join(".lf/flows/offline.yaml"),
            "- cmd: task sync --plan\n",
        )
        .unwrap();
        let output = unbound_command(
            Path::new(env!("CARGO_BIN_EXE_lf")),
            worktree,
            &["-b", "--task", task.id.as_str(), "flow", "offline"],
        )
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let flows = support::recorded_flows(home.path());
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].0.as_deref(), Some("succeeded"));
        assert_eq!(
            runtime
                .block_on(fixture.store.get_task(&task.id))
                .unwrap()
                .unwrap()
                .plan,
            saved.plan
        );
    }
}
