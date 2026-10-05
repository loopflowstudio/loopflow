mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

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
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("HTTP_PROXY", "http://127.0.0.1:1");
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

/// The three ways to name a Task for a Flow: the Task entry, `--task`, and its
/// worktree. All block in the foreground and reach the same checks.
const LAUNCHES: [&[&str]; 3] = [
    &["-b", "task", "run", "INF-123", "proof"],
    &["-b", "--task", "INF-123", "run", "proof"],
    &["-b", "run", "proof"],
];

#[test]
fn every_task_launch_runs_in_the_foreground_under_the_same_checks() {
    for condition in ["valid", "invalid", "removed", "terminal", "moved"] {
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        repo.create_branch("launch-proof");
        let home = tempfile::tempdir().unwrap();
        let _env = support::EnvGuard::new(&[
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
        // `feature` is the fixture Project's Flow.
        for flow in ["proof", "feature"] {
            fs::write(
                repo.path().join(format!(".lf/flows/{flow}.yaml")),
                "- cmd: task sync --plan\n",
            )
            .unwrap();
        }
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
        let run = |args: &[&str]| command(repo.path(), home.path(), args).output().unwrap();
        if condition != "valid" {
            let expected = match condition {
                "terminal" => "terminal",
                "moved" => "no longer matches",
                _ => "planning",
            };
            for launch in LAUNCHES {
                let output = run(launch);
                assert!(!output.status.success(), "{condition} admitted {launch:?}");
                let error = String::from_utf8_lossy(&output.stderr);
                assert!(error.contains(expected), "{condition} {launch:?}: {error}");
            }
            assert!(flows(home.path()).is_empty(), "{condition} launched a Flow");
            continue;
        }
        // Each launch has finished when its command returns, and is its own
        // invocation; none continues another.
        for (launched, launch) in LAUNCHES.iter().enumerate() {
            let output = run(launch);
            assert!(
                output.status.success(),
                "{launch:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let flows = flows(home.path());
            assert_eq!(flows.len(), launched + 1, "{launch:?}");
            assert!(flows.iter().all(|(_, state)| state == "completed"));
        }
        // With no Flow named, the entry runs the Project's.
        let output = run(&["-b", "task", "run", "INF-123"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let (bound, effects, default): (i64, i64, String) = db
            .query_row(
                "SELECT (SELECT count(*) FROM flow_sessions WHERE task_id=?1),
                    (SELECT count(*) FROM flow_events WHERE kind='operation_completed' AND outcome='completed'),
                    (SELECT json_extract(invocation_json,'$.flow') FROM flow_sessions ORDER BY rowid DESC LIMIT 1)",
                [registered.task.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!((bound, effects), (4, 4), "each Flow ran its own operation");
        assert_eq!(default, "feature");
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
        // The worker's entry points are gone; nothing restarts a Flow.
        for removed in [
            vec!["--task", "INF-123", "flow", "start", "proof"],
            vec!["task", "restart", "INF-123"],
            vec!["task", "create", "--run", "--title", "x"],
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
        assert_eq!(flows(home.path()).len(), 4);
    }
}
