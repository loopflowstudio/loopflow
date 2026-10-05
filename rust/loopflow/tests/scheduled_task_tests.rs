mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use loopflow::durable::FlowSession;
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::store::Store;
use loopflow::work::task::Task;
use loopflow_test_support::TestRepo;
use time::OffsetDateTime;

fn flow(store: &Store, task: &Task, review: bool) -> String {
    fs::create_dir_all(task.worktree.join(".lf/flows")).unwrap();
    let steps = if review {
        "- step:\n    name: demo\n    id: review\n    human: true\n"
    } else {
        "- cmd: sync --plan\n- step:\n    name: demo\n    id: review\n    human: true\n"
    };
    fs::write(task.worktree.join(".lf/flows/proof.yaml"), steps).unwrap();
    let flow = FlowSession {
        invocation: QueuedInvocation::load(&task.worktree, "proof").unwrap(),
        cursor: Default::default(),
        version: 0,
        task_id: Some(task.id.clone()),
        wave_id: Some(task.wave_id.clone()),
        cwd: task.worktree.clone(),
        message: None,
        model: None,
        current_attempt: None,
        pending_session_id: None,
        worker_generation: 0,
        claim: None,
        failure: None,
        finished: false,
        updated_at: OffsetDateTime::now_utc(),
    };
    let id = flow.id().to_string();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(store.start_task_flow(&task.id, flow))
        .unwrap();
    id
}

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_TEST_WORKER_LOG", home.join("worker.log"));
    command
}

#[test]
fn overlapping_ticks_preserve_review_hold_and_unselected_flows() {
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::new(&[
        ("tmux", "#!/bin/sh\nif [ \"$1\" = new-session ]; then\nfor arg do command=$arg; done\n/bin/sh -c \"$command\" </dev/null >\"$LF_TEST_WORKER_LOG\" 2>&1 &\nfi\nexit 0\n"),
        ("open", "#!/bin/sh\nexit 0\n"),
    ]);
    let a_path = repo.create_named_worktree("selected");
    let b_path = repo.create_named_worktree("review");
    let c_path = repo.create_named_worktree("unselected");
    let d_path = repo.create_named_worktree("held");
    let base = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let registered = support::register_task(
        home.path(),
        &a_path,
        "selected",
        String::from_utf8_lossy(&base.stdout).trim(),
    );
    let b = support::register_sibling_task(&registered, "INF-124", "review", &b_path);
    let c = support::register_sibling_task(&registered, "INF-125", "unselected", &c_path);
    let d = support::register_sibling_task(&registered, "INF-126", "held", &d_path);
    let a_flow = flow(&registered.store, &registered.task, false);
    let b_flow = flow(&registered.store, &b, true);
    let c_flow = flow(&registered.store, &c, false);
    let d_flow = flow(&registered.store, &d, false);
    for (issue, state) in [("INF-123", "on"), ("INF-124", "on"), ("INF-126", "off")] {
        let result = command(
            repo.path(),
            home.path(),
            &["task", "automate", issue, state],
        )
        .output()
        .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let first = command(repo.path(), home.path(), &["task", "reconcile", "--json"])
        .spawn()
        .unwrap();
    let second = command(repo.path(), home.path(), &["task", "reconcile", "--json"])
        .output()
        .unwrap();
    let first = first.wait_with_output().unwrap();
    assert!(
        first.status.success() || second.status.success(),
        "{}\n{}\nworker: {}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr),
        fs::read_to_string(home.path().join("worker.log")).unwrap_or_default()
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let count: i64 = db
            .query_row(
                "SELECT count(*) FROM flow_events WHERE flow_id=?1 AND kind='operation_completed'",
                [&a_flow],
                |row| row.get(0),
            )
            .unwrap();
        if count > 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "selected Flow never advanced: {} worker: {}",
            String::from_utf8_lossy(&second.stderr),
            fs::read_to_string(home.path().join("worker.log")).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM flow_events WHERE flow_id=?1 AND kind='operation_completed'",
            [&a_flow],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    for id in [&b_flow, &c_flow, &d_flow] {
        let count: i64 = db
            .query_row(
                "SELECT count(*) FROM flow_events WHERE flow_id=?1 AND kind='operation_started'",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "review, unselected and held Flows never started");
    }
    let selected: Option<bool> = db
        .query_row(
            "SELECT automation_enabled FROM tasks WHERE id=?1",
            [c.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        selected, None,
        "historical work was not retroactively enrolled"
    );
}

#[test]
fn failed_starts_exhaust_one_retry_without_changing_the_saved_flow() {
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::new(&[("tmux", "#!/bin/sh\nexit 1\n")]);
    let path = repo.create_named_worktree("failed-start");
    let base = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let registered = support::register_task(
        home.path(),
        &path,
        "failed-start",
        String::from_utf8_lossy(&base.stdout).trim(),
    );
    let saved = flow(&registered.store, &registered.task, false);
    assert!(command(
        repo.path(),
        home.path(),
        &["task", "automate", "INF-123", "on"]
    )
    .output()
    .unwrap()
    .status
    .success());
    let mut last = serde_json::Value::Null;
    for _ in 0..3 {
        let output = command(repo.path(), home.path(), &["task", "reconcile", "--json"])
            .output()
            .unwrap();
        last = serde_json::from_slice(&output.stdout).unwrap();
    }
    assert_eq!(last["tasks"][0]["retries"], 1);
    assert!(last["tasks"][0]["detail"]
        .as_str()
        .unwrap()
        .contains("exhausted automatic retries"));
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let current: String = db
        .query_row(
            "SELECT current_invocation_id FROM tasks WHERE id=?1",
            [registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(current, saved);
    let advances: i64 = db
        .query_row(
            "SELECT count(*) FROM flow_events WHERE kind='operation_started'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(advances, 0);
}

#[test]
fn failure_without_ask_reconciliation_keeps_managed_failure_stopped() {
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::new(&[("tmux", "#!/bin/sh\nexit 91\n")]);
    let path = repo.create_named_worktree("failed-decision");
    let registered =
        support::register_task(home.path(), &path, "failed-decision", &repo.head_sha());
    let id = flow(&registered.store, &registered.task, false);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let saved = runtime
        .block_on(registered.store.flow(&id))
        .unwrap()
        .unwrap();
    let failed = runtime
        .block_on(registered.store.fail_flow(
            &id,
            saved.version,
            None,
            &loopflow::durable::TaskFlowBlocker::now("Release target is missing"),
        ))
        .unwrap();
    assert!(command(
        repo.path(),
        home.path(),
        &["task", "automate", "INF-123", "on"]
    )
    .output()
    .unwrap()
    .status
    .success());
    for _ in 0..2 {
        let output = command(repo.path(), home.path(), &["task", "reconcile", "--json"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            report["tasks"][0]["detail"]
                .as_str()
                .unwrap()
                .contains("Release target is missing"),
            "{report}"
        );
        assert_eq!(report["tasks"][0]["retries"], 0);
        assert_eq!(
            runtime
                .block_on(registered.store.task_flow(&registered.task.id))
                .unwrap()
                .unwrap(),
            failed
        );
        let status = command(
            repo.path(),
            home.path(),
            &["task", "status", "INF-123", "--json"],
        )
        .output()
        .unwrap();
        assert!(status.status.success(), "{status:?}");
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(status["execution"]["execution"]["state"], "blocked");
        assert!(status["execution"]["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains("Release target is missing"));
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    let retried = runtime
        .block_on(registered.store.retry_flow(&id, Some("Use staging")))
        .unwrap();
    assert_eq!(retried.id(), failed.id());
    assert_eq!(retried.cursor.index, failed.cursor.index);
    assert_eq!(retried.cursor.iteration, failed.cursor.iteration);
    assert_eq!(
        retried.cursor.leaf().progress.direction.as_deref(),
        Some("Use staging")
    );
    assert!(retried.failure.is_none());
}
