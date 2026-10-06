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
            assert!(
                support::recorded_flows(home.path()).is_empty(),
                "{condition} launched a Flow"
            );
            continue;
        }
        // Each launch has finished when its command returns, and is its own
        // Flow; none continues another.
        for (launched, launch) in LAUNCHES.iter().enumerate() {
            let output = run(launch);
            assert!(
                output.status.success(),
                "{launch:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let flows = support::recorded_flows(home.path());
            assert_eq!(flows.len(), launched + 1, "{launch:?}");
            assert!(flows
                .iter()
                .all(|(outcome, _)| outcome.as_deref() == Some("succeeded")));
        }
        // However the Task was named, the Flow left the same Execs.
        let recorded = support::recorded_flows(home.path());
        assert_eq!(recorded[0].1.len(), 1);
        assert_eq!(
            recorded[0].1[0]["argv"],
            serde_json::json!(["sync", "--plan"])
        );
        assert!(recorded.iter().all(|(_, steps)| *steps == recorded[0].1));
        // With no Flow named, the entry runs the Project's.
        let output = run(&["-b", "task", "run", "INF-123"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let recorded = support::recorded_flows(home.path());
        assert_eq!(recorded.len(), 4);
        assert_eq!(recorded[3].1[0]["flow"], "feature");
        let status = run(&["task", "status", "INF-123", "--json"]);
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(
            status["execution"]["work"]["flows"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert!(status["execution"]["work"]["flows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|flow| flow["state"] == "completed"));
        // The Project's default names a repository Flow, so the Task took up
        // no workflow and ran every Flow ad hoc.
        assert!(status["execution"]["work"]["workflow"].is_null());
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
        assert_eq!(support::recorded_flows(home.path()).len(), 4);
    }
}

/// A registered Task whose repository defines the Flows and workflows below.
struct WorkflowTask {
    repo: TestRepo,
    home: tempfile::TempDir,
    _env: support::EnvGuard,
}

impl WorkflowTask {
    fn new() -> Self {
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        repo.create_branch("launch-proof");
        let home = tempfile::tempdir().unwrap();
        let env = support::EnvGuard::new(&[
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
        ]);
        support::register_task(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "launch-proof",
            &repo.head_sha(),
        );
        for (path, content) in [
            (".lf/flows/proof.yaml", "- cmd: task sync --plan\n"),
            (".lf/flows/land-proof.yaml", "- cmd: task sync --plan\n"),
            (".lf/flows/broken.yaml", "- cmd: flow show no-such-flow\n"),
            // No PR: the last edge runs nothing.
            (
                ".lf/workflows/findings.yaml",
                "stages:\n  findings: research\nedges:\n  - {from: start, to: findings, flow: proof}\n  - {from: findings, to: end}\n",
            ),
            // Several PRs: the landing edge returns to its stage.
            (
                ".lf/workflows/rounds.yaml",
                "stages:\n  review: demo\nedges:\n  - {from: start, to: review, flow: proof}\n  - {from: review, to: review, flow: land-proof}\n  - {from: review, to: end, flow: broken}\n",
            ),
        ] {
            let path = repo.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        Self {
            repo,
            home,
            _env: env,
        }
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        command(self.repo.path(), self.home.path(), args)
            .output()
            .unwrap()
    }

    fn ok(&self, args: &[&str]) {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn workflow(&self) -> serde_json::Value {
        let status = self.run(&["task", "status", "INF-123", "--json"]);
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        status["execution"]["work"]["workflow"].clone()
    }
}

fn at(stage: &str) -> serde_json::Value {
    serde_json::json!({"kind": "stage", "stage": stage})
}

#[test]
fn a_workflow_with_no_landing_edge_reaches_its_end_without_a_pr() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "findings");
    assert_eq!(workflow["stages"][0]["skill"], "research");
    assert_eq!(workflow["position"], at("findings"));
    // The edge's Flow is an ordinary Flow run, and its driver is the traversal.
    let flows = support::recorded_flows(task.home.path());
    assert_eq!(flows.len(), 1);
    assert_eq!(workflow["traversals"].as_array().unwrap().len(), 1);
    task.ok(&["task", "run", "INF-123"]);
    assert_eq!(task.workflow()["position"], at("end"));
    assert_eq!(support::recorded_flows(task.home.path()).len(), 1);
    // Reaching the end neither completes the Task nor leaves an edge to run.
    let refused = task.run(&["-b", "task", "run", "INF-123"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("reached its end"));
    let status = task.run(&["task", "status", "INF-123"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("Workflow findings: at end"));
}

#[test]
fn a_landing_edge_that_returns_to_its_stage_can_be_taken_again() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    for _ in 0..2 {
        task.ok(&["-b", "task", "run", "INF-123", "land-proof"]);
        assert_eq!(task.workflow()["position"], at("review"));
    }
    let workflow = task.workflow();
    let edges: Vec<_> = workflow["traversals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|traversal| traversal["edge"].as_u64().unwrap())
        .collect();
    assert_eq!(edges, [0, 1, 1]);
    // Two edges leave review, so the entry asks which, and names them.
    let refused = task.run(&["-b", "task", "run", "INF-123"]);
    assert!(!refused.status.success());
    let error = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(error.contains("land-proof (to review)"), "{error}");
    let refused = task.run(&["-b", "task", "run", "INF-123", "proof"]);
    assert!(!refused.status.success());
    let error = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(error.contains("proof does not leave review"), "{error}");
    assert_eq!(task.workflow()["traversals"].as_array().unwrap().len(), 3);
}

#[test]
fn a_failed_edge_leaves_the_task_at_the_stage_it_left() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    let failed = task.run(&["-b", "task", "run", "INF-123", "broken"]);
    assert!(!failed.status.success());
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("review"));
    assert_eq!(workflow["traversals"].as_array().unwrap().len(), 2);
    // Taking up another workflow keeps that history and starts over.
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "findings");
    assert_eq!(workflow["traversals"].as_array().unwrap().len(), 1);
}

#[test]
fn a_plain_flow_run_in_the_worktree_does_not_move_the_task() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    let before = task.workflow();
    task.ok(&["-b", "run", "land-proof"]);
    task.ok(&["-b", "--task", "INF-123", "run", "land-proof"]);
    assert_eq!(support::recorded_flows(task.home.path()).len(), 3);
    assert_eq!(task.workflow(), before);
    // A workflow is traversed, never run as a Flow.
    let refused = task.run(&["-b", "run", "rounds"]);
    assert!(!refused.status.success());
    let error = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(error.contains("lf task run <issue> rounds"), "{error}");
}
