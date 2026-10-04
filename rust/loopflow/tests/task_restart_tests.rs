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
        "- cmd: task sync --plan\n- step:\n    name: demo\n    id: review\n    human: true\n"
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
        ready_summary: None,
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
fn restart_uses_old_valid_planning_and_preserves_invalid_work() {
    for condition in ["valid", "invalid", "removed", "terminal", "moved", "advice"] {
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        repo.create_branch("restart-proof");
        let home = tempfile::tempdir().unwrap();
        let _env = support::EnvGuard::new(&[
            ("tmux", "#!/bin/sh\nif [ \"$1\" = new-session ]; then\nfor arg do command=$arg; done\n/bin/sh -c \"$command\" </dev/null >\"$LF_TEST_WORKER_LOG\" 2>&1 &\nfi\nexit 0\n"),
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
        ]);
        let registered = support::register_task(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "restart-proof",
            &repo.head_sha(),
        );
        let old_id = flow(&registered.store, &registered.task, true);
        // The replacement executes a real mechanical step before stopping for review.
        fs::write(
            repo.path().join(".lf/flows/proof.yaml"),
            "- cmd: task sync --plan\n- step:\n    name: demo\n    id: review\n    human: true\n",
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let old = runtime
            .block_on(registered.store.flow(&old_id))
            .unwrap()
            .unwrap();
        let scope = repo.path().canonicalize().unwrap().display().to_string();
        let mut record = runtime
            .block_on(
                registered
                    .store
                    .pm_task_observation(&scope, "linear", "INF-123"),
            )
            .unwrap()
            .record
            .unwrap();
        record.item.revision = Some("2026-10-04T12:00:00Z".into());
        if condition == "terminal" {
            record.item.state = Some("canceled".into());
        }
        if condition == "moved" {
            record.project.as_mut().unwrap().id = "another-project".into();
            record.item.project_id = Some("another-project".into());
        }
        runtime
            .block_on(registered.store.put_pm_task(&scope, "linear", record))
            .unwrap();
        // Age the acquired facts without pretending a stale provider response won.
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .execute_batch(
                "UPDATE pm_items SET observed_at=1; UPDATE pm_projects SET observed_at=1;",
            )
            .unwrap();
        if condition == "invalid" || condition == "removed" {
            runtime
                .block_on(registered.store.observe_pm_issue_change(
                    registered.task.plan.id.as_str(),
                    None,
                    condition == "removed",
                ))
                .unwrap();
        }
        let mut args = vec!["task", "restart", "INF-123", "--flow", "proof", "--json"];
        if condition == "advice" {
            args.push("new direction requires publication");
        }
        let output = command(repo.path(), home.path(), &args)
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .output()
            .unwrap();
        let current = runtime
            .block_on(registered.store.task_flow(&registered.task.id))
            .unwrap()
            .unwrap();
        if condition == "valid" {
            assert!(
                output.status.success(),
                "{} worker: {}",
                String::from_utf8_lossy(&output.stderr),
                fs::read_to_string(home.path().join("worker.log")).unwrap_or_default()
            );
            assert_ne!(current.id(), old.id());
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let current = runtime
                    .block_on(registered.store.task_flow(&registered.task.id))
                    .unwrap()
                    .unwrap();
                if current.is_human() {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "replacement never reached review: {current:?}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(runtime
                .block_on(registered.store.flow(&old_id))
                .unwrap()
                .is_some());
            let task = runtime
                .block_on(registered.store.get_task(&registered.task.id))
                .unwrap()
                .unwrap();
            assert_eq!(task.id, registered.task.id);
            assert_eq!(task.project_id, registered.task.project_id);
            assert_eq!(task.wave_id, registered.task.wave_id);
            assert_eq!(task.worktree, registered.task.worktree);
            assert_eq!(task.plan.pm_snapshot_synced_at, 1);
        } else {
            assert!(!output.status.success(), "{condition} admitted restart");
            let error = String::from_utf8_lossy(&output.stderr);
            let expected = match condition {
                "terminal" => "terminal",
                "moved" => "no longer matches",
                "advice" => "advice was not published",
                _ => "planning",
            };
            assert!(error.contains(expected), "{condition}: {error}");
            assert_eq!(current, old, "{condition} replaced existing execution");
        }
        assert_eq!(
            runtime
                .block_on(registered.store.active_task_pr(&registered.task.id))
                .unwrap()
                .unwrap()
                .id,
            registered.pr.id
        );
    }
}
