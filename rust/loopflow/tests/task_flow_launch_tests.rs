mod support;

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use loopflow_test_support::TestRepo;

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in
        std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
    {
        command.env_remove(name);
    }
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_TEST_FLOW_LOG", home.join("flow.log"));
    command
}

fn flows(home: &Path) -> Vec<(String, String)> {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let mut query = db
        .prepare("SELECT id,state FROM flow_sessions ORDER BY rowid")
        .unwrap();
    let rows = query
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap();
    rows.collect::<rusqlite::Result<_>>().unwrap()
}

#[test]
fn flow_start_launches_a_fresh_flow_and_refuses_invalid_planning() {
    for condition in ["valid", "invalid", "removed", "terminal", "moved"] {
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        repo.create_branch("launch-proof");
        let home = tempfile::tempdir().unwrap();
        // A local process stands in for tmux; the real Flow driver and op execute.
        let _env = support::EnvGuard::new(&[
            ("tmux", "#!/bin/sh\nif [ \"$1\" = new-session ]; then\nfor arg do command=$arg; done\n/bin/sh -c \"$command\" </dev/null >>\"$LF_TEST_FLOW_LOG\" 2>&1 &\nfi\nexit 0\n"),
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
        ]);
        let registered = support::register_task(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "launch-proof",
            &repo.head_sha(),
        );
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        fs::write(
            repo.path().join(".lf/flows/proof.yaml"),
            "- cmd: task sync --plan\n",
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
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
        // Old valid planning still admits a launch without reaching the provider.
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
        let start = || {
            command(
                repo.path(),
                home.path(),
                &["--task", "INF-123", "flow", "start", "proof", "--json"],
            )
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .output()
            .unwrap()
        };
        let log = || fs::read_to_string(home.path().join("flow.log")).unwrap_or_default();
        if condition != "valid" {
            let output = start();
            assert!(!output.status.success(), "{condition} admitted a launch");
            let error = String::from_utf8_lossy(&output.stderr);
            let expected = match condition {
                "terminal" => "terminal",
                "moved" => "no longer matches",
                _ => "planning",
            };
            assert!(error.contains(expected), "{condition}: {error}");
            assert!(flows(home.path()).is_empty(), "{condition} launched a Flow");
            continue;
        }
        // Each start is its own invocation; the first is never continued.
        for launched in 1..=2 {
            let output = start();
            assert!(
                output.status.success(),
                "{} flow: {}",
                String::from_utf8_lossy(&output.stderr),
                log()
            );
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let flows = flows(home.path());
                if flows.len() == launched && flows.iter().all(|(_, state)| state == "completed") {
                    break;
                }
                assert!(
                    flows.len() <= launched && Instant::now() < deadline,
                    "launch {launched} did not complete: {flows:?}\n{}",
                    log()
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let (bound, effects): (i64, i64) = db
            .query_row(
                "SELECT (SELECT count(*) FROM flow_sessions WHERE task_id=?1),
                    (SELECT count(*) FROM flow_events WHERE kind='operation_completed' AND outcome='completed')",
                [registered.task.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((bound, effects), (2, 2), "each Flow ran its own operation");
        let task = runtime
            .block_on(registered.store.get_task(&registered.task.id))
            .unwrap()
            .unwrap();
        assert_eq!(task.worktree, registered.task.worktree);
        assert_eq!(
            runtime
                .block_on(registered.store.active_task_pr(&registered.task.id))
                .unwrap()
                .unwrap()
                .id,
            registered.pr.id
        );
        // No command restarts or resumes a launched Flow.
        for removed in [
            vec!["task", "restart", "INF-123"],
            vec!["--task", "INF-123", "flow", "start", "--retry"],
            vec!["task", "__worker", registered.task.id.as_str()],
        ] {
            assert!(
                !command(repo.path(), home.path(), &removed)
                    .output()
                    .unwrap()
                    .status
                    .success(),
                "{removed:?}"
            );
        }
        assert_eq!(flows(home.path()).len(), 2);
    }
}
