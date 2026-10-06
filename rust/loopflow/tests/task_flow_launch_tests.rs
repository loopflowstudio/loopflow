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
        // The Project names a repository Flow, which is not a workflow: a
        // run that names nothing is refused and starts no Flow.
        let output = run(&["-b", "task", "run", "INF-123"]);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("which is not a workflow"), "{error}");
        assert!(error.contains("lf wave update-plan --workflow"), "{error}");
        assert_eq!(support::recorded_flows(home.path()).len(), 3);
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
            3
        );
        assert!(status["execution"]["work"]["flows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|flow| flow["state"] == "completed"));
        // The Task took up no workflow and ran every named Flow ad hoc.
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
        assert_eq!(support::recorded_flows(home.path()).len(), 3);
    }
}

/// A registered Task whose repository defines the Flows and workflows below.
struct WorkflowTask {
    repo: TestRepo,
    home: tempfile::TempDir,
    registered: support::RegisteredTask,
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
        for (path, content) in [
            (".lf/flows/proof.yaml", "- cmd: task sync --plan\n"),
            (".lf/flows/land-proof.yaml", "- cmd: task sync --plan\n"),
            (".lf/flows/broken.yaml", "- cmd: flow show no-such-flow\n"),
            // Fails until the repository defines a Flow named `late`.
            (".lf/flows/gate.yaml", "- cmd: flow show late\n"),
            // Another round succeeds; accepting fails until `late` exists.
            (
                ".lf/workflows/gated.yaml",
                "nodes:\n  review: demo\n  accepted: demo\nedges:\n  - {from: start, to: review, flow: proof}\n  - {from: review, to: review, flow: proof}\n  - {from: review, to: accepted, flow: gate}\n  - {from: accepted, to: end}\n",
            ),
            // The Project's workflow, `feature`, as this repository defines it.
            (
                ".lf/workflows/feature.yaml",
                "nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: proof}\n  - {from: review, to: end}\n",
            ),
            // No PR: the last edge runs nothing.
            (
                ".lf/workflows/findings.yaml",
                "nodes:\n  findings: research\nedges:\n  - {from: start, to: findings, flow: proof}\n  - {from: findings, to: end}\n",
            ),
            // Several PRs: the landing edge returns to its node.
            (
                ".lf/workflows/rounds.yaml",
                "nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: proof}\n  - {from: review, to: review, flow: land-proof}\n  - {from: review, to: end, flow: broken}\n",
            ),
        ] {
            let path = repo.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        // The Task's branch starts from these definitions and holds nothing.
        repo.stage_all();
        repo.commit("Define the fixture Flows and workflows");
        let registered = support::register_task(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "launch-proof",
            &repo.head_sha(),
        );
        Self {
            repo,
            home,
            registered,
            _env: env,
        }
    }

    fn status(&self) -> serde_json::Value {
        let status = self.run(&["task", "status", "INF-123", "--json"]);
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        status["execution"].clone()
    }

    /// The Task's state as `lf task status` reads it from the Workflow.
    fn state(&self) -> String {
        self.status()["status"].as_str().unwrap().to_string()
    }

    /// Linear now calls the Task complete, as last read.
    fn complete_in_linear(&self) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let scope = self
            .repo
            .path()
            .canonicalize()
            .unwrap()
            .display()
            .to_string();
        let store = &self.registered.store;
        let mut record = runtime
            .block_on(store.pm_task_observation(&scope, "linear", "INF-123"))
            .unwrap()
            .record
            .unwrap();
        record.item.revision = Some("2026-10-06T12:00:00Z".into());
        record.item.state = Some("completed".into());
        record.item.completed = true;
        runtime
            .block_on(store.put_pm_task(&scope, "linear", record))
            .unwrap();
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
        self.status()["work"]["workflow"].clone()
    }
}

fn at(node: &str) -> serde_json::Value {
    serde_json::json!({"kind": "node", "node": node})
}

/// Each move's kind and the edge it names, oldest first.
fn moves(workflow: &serde_json::Value) -> Vec<(String, Option<u64>)> {
    workflow["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["kind"].as_str().unwrap().into(),
                entry["edge"].as_u64(),
            )
        })
        .collect()
}

fn refusal(output: std::process::Output) -> String {
    assert!(!output.status.success());
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn a_task_takes_up_its_projects_workflow_and_keeps_one_it_named() {
    // Nothing named, no Workflow yet: the Project's.
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123"]);
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "feature");
    assert_eq!(workflow["position"], at("review"));
    drop(task);

    // A Task that named its own keeps it when later runs name nothing.
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    task.ok(&["-b", "task", "run", "INF-123"]);
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "findings");
    assert_eq!(workflow["position"], at("end"));
}

#[test]
fn a_workflow_with_no_landing_edge_reaches_its_end_without_a_pr() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "findings");
    assert_eq!(workflow["nodes"][0]["skill"], "research");
    assert_eq!(workflow["position"], at("findings"));
    assert_eq!(workflow["outgoing"], serde_json::json!([1]));
    // The edge's Flow is an ordinary Flow exec; its driver chose the edge and,
    // having succeeded, wrote the arrival.
    let flows = support::recorded_flows(task.home.path());
    assert_eq!(flows.len(), 1);
    let chose = |edge| ("chose".to_string(), Some(edge));
    assert_eq!(
        moves(&workflow),
        [
            ("took_up".into(), None),
            chose(0),
            ("arrived".into(), Some(0))
        ]
    );
    let history = workflow["history"].as_array().unwrap();
    assert_eq!(history[1]["exec_id"], history[2]["exec_id"]);
    assert_eq!(history[1]["actor"], "person");
    assert_eq!(history[2]["actor"], "edge");
    task.ok(&["task", "run", "INF-123"]);
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("end"));
    assert_eq!(moves(&workflow).last(), Some(&chose(1)));
    assert_eq!(support::recorded_flows(task.home.path()).len(), 1);
    // Reaching the end is completion: the Task is done, its empty PR slot is
    // retired, and nothing is left to run.
    assert_eq!(task.state(), "done");
    assert!(task.status()["active_pr"].is_null());
    let error = refusal(task.run(&["-b", "task", "run", "INF-123"]));
    assert!(error.contains("is done"), "{error}");
    let status = task.run(&["task", "status", "INF-123"]);
    assert!(String::from_utf8_lossy(&status.stdout).contains("Workflow findings: at end"));
}

#[test]
fn a_landing_edge_that_returns_to_its_node_can_be_taken_again() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    for _ in 0..2 {
        task.ok(&["-b", "task", "run", "INF-123", "land-proof"]);
        assert_eq!(task.workflow()["position"], at("review"));
    }
    let chosen = |workflow: &serde_json::Value| -> Vec<u64> {
        moves(workflow)
            .into_iter()
            .filter(|(kind, _)| kind == "chose")
            .filter_map(|(_, edge)| edge)
            .collect()
    };
    assert_eq!(chosen(&task.workflow()), [0, 1, 1]);
    // Two edges leave review, so the entry asks which, and names them.
    let error = refusal(task.run(&["-b", "task", "run", "INF-123"]));
    assert!(error.contains("land-proof (to review)"), "{error}");
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "proof"]));
    assert!(error.contains("proof does not leave review"), "{error}");
    assert_eq!(chosen(&task.workflow()), [0, 1, 1]);
}

#[test]
fn a_landing_that_settles_later_is_recorded_by_moving_the_task() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    // The landing edge's Flow stops before its end: the Task stays on it.
    assert!(!task
        .run(&["-b", "task", "run", "INF-123", "broken"])
        .status
        .success());
    let workflow = task.workflow();
    assert_eq!(workflow["position"]["kind"], "edge");
    assert_eq!(workflow["position"]["edge"], 2);
    assert_eq!(workflow["position"]["running"], false);
    assert_eq!(moves(&workflow).last().unwrap().0, "chose");
    // The work finished elsewhere; a person says so.
    task.ok(&[
        "task",
        "move",
        "INF-123",
        "end",
        "--reason",
        "merged by hand",
    ]);
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("end"));
    let set = workflow["history"].as_array().unwrap().last().unwrap();
    assert_eq!(set["kind"], "set");
    assert_eq!(
        (&set["from"], &set["to"]),
        (&"review".into(), &"end".into())
    );
    assert_eq!(set["edge"], 2);
    assert_eq!(set["note"], "merged by hand");
    assert_eq!(set["actor"], "person");
    assert_eq!(set["session_id"], serde_json::Value::Null);
    // One Flow exec reached the node; the failing edge was attempted three times.
    assert_eq!(support::recorded_flows(task.home.path()).len(), 4);
    // The Task is done; nothing runs until it is put back on its workflow.
    assert_eq!(task.state(), "done");
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "findings"]));
    assert!(error.contains("lf task move INF-123"), "{error}");
}

#[test]
fn a_stopped_edge_is_chosen_again_and_the_task_can_go_back() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    assert!(!task
        .run(&["-b", "task", "run", "INF-123", "gate"])
        .status
        .success());
    let stopped = task.workflow();
    assert_eq!(stopped["position"]["edge"], 2);
    assert_eq!(stopped["position"]["running"], false);
    assert_eq!(stopped["outgoing"], serde_json::json!([1, 2]));
    let status = task.run(&["task", "status", "INF-123"]);
    let status = String::from_utf8_lossy(&status.stdout).to_string();
    assert!(
        status.contains("Workflow gated: stopped on gate (review → accepted)"),
        "{status}"
    );
    // Once its cause is fixed the same edge is chosen again and arrives.
    let late = task.repo.path().join(".lf/flows/late.yaml");
    fs::write(late, "- cmd: task sync --plan\n").unwrap();
    task.ok(&["-b", "task", "run", "INF-123", "gate"]);
    assert_eq!(task.workflow()["position"], at("accepted"));
    // The demo was not good enough after all: go back and take the loop.
    task.ok(&[
        "task",
        "move",
        "INF-123",
        "review",
        "--reason",
        "one more round",
    ]);
    assert_eq!(task.workflow()["position"], at("review"));
    task.ok(&["-b", "task", "run", "INF-123", "proof"]);
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("review"));
    let kinds: Vec<_> = moves(&workflow).into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(
        kinds,
        ["took_up", "chose", "arrived", "chose", "chose", "arrived", "set", "chose", "arrived"]
    );
    let error = refusal(task.run(&["task", "move", "INF-123", "nowhere"]));
    assert!(
        error.contains("Nodes: start, review, accepted, end"),
        "{error}"
    );
    // Taking up another workflow starts over and keeps the history.
    let before = workflow["history"].as_array().unwrap().len();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "findings");
    assert_eq!(workflow["position"], at("findings"));
    assert_eq!(workflow["history"].as_array().unwrap().len(), before + 3);
    assert_eq!(workflow["history"][0]["workflow"], "gated");
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

/// The Flow execs started beneath `task_run`, by outcome, oldest first.
fn attempts(home: &Path, task_run: &str) -> Vec<String> {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let mut rows = db
        .prepare(
            "SELECT d.outcome FROM flow_execs f JOIN execs d ON d.id=f.exec_id
             WHERE d.parent_exec_id=?1 ORDER BY d.rowid",
        )
        .unwrap();
    let outcomes = rows
        .query_map([task_run], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    outcomes
}

#[test]
fn one_task_run_starts_its_flow_again_until_an_attempt_succeeds_or_attempts_run_out() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    let carrier = |workflow: &serde_json::Value| {
        let chose = workflow["history"].as_array().unwrap().last().unwrap();
        chose["exec_id"].as_str().unwrap().to_string()
    };
    // Every attempt fails: the Task run gives up and the edge holds the Task.
    let output = task.run(&["-b", "task", "run", "INF-123", "gate"]);
    let error = refusal(output);
    assert!(error.contains("Flow gate failed 3 times"), "{error}");
    let stopped = task.workflow();
    assert_eq!(stopped["position"]["edge"], 2);
    assert_eq!(stopped["position"]["running"], false);
    assert_eq!(stopped["position"]["exec_id"], carrier(&stopped).as_str());
    assert_eq!(
        attempts(task.home.path(), &carrier(&stopped)),
        ["failed", "failed", "failed"]
    );

    // The cause clears as the third attempt starts: an `lf` that defines
    // `late` the third time the gate asks for it.
    let bin = tempfile::tempdir().unwrap();
    let lf = bin.path().join("lf");
    fs::write(
        &lf,
        format!(
            "#!/bin/sh\ncase \"$*\" in *'flow show late'*)\n  echo asked >> '{count}'\n  [ \"$(wc -l < '{count}')\" -ge 3 ] && printf -- '- cmd: task sync --plan\\n' > '{late}';;\nesac\nexec '{real}' \"$@\"\n",
            count = bin.path().join("asked").display(),
            late = task.repo.path().join(".lf/flows/late.yaml").display(),
            real = env!("CARGO_BIN_EXE_lf"),
        ),
    )
    .unwrap();
    fs::set_permissions(&lf, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let output = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "task", "run", "INF-123", "gate"],
    )
    .env("LF_BIN", &lf)
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("accepted"));
    // One Task run: chosen once, arrived once, three Flow execs beneath it.
    let kinds: Vec<_> = moves(&workflow).into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(
        kinds,
        ["took_up", "chose", "arrived", "chose", "chose", "arrived"]
    );
    let arrived = workflow["history"].as_array().unwrap().last().unwrap();
    let task_run = arrived["exec_id"].as_str().unwrap();
    assert_eq!(
        attempts(task.home.path(), task_run),
        ["failed", "failed", "succeeded"]
    );
    assert_eq!(support::recorded_flows(task.home.path()).len(), 7);
}

#[test]
fn a_tasks_state_is_read_from_where_it_stands_on_its_workflow() {
    let task = WorkflowTask::new();
    assert_eq!(task.state(), "not_ready");
    // A node, then a stopped edge: both between start and end.
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    assert_eq!(task.workflow()["position"], at("review"));
    assert_eq!(task.state(), "active");
    assert!(!task
        .run(&["-b", "task", "run", "INF-123", "gate"])
        .status
        .success());
    assert_eq!(task.workflow()["position"]["kind"], "edge");
    assert_eq!(task.state(), "active");
    task.ok(&["task", "move", "INF-123", "start"]);
    assert_eq!(task.state(), "ready");
    // Reaching `end` is completion, and leaving it reopens the Task.
    task.ok(&["task", "move", "INF-123", "end", "--reason", "Delivered"]);
    assert_eq!(task.state(), "done");
    let text =
        String::from_utf8_lossy(&task.run(&["task", "status", "INF-123"]).stdout).to_string();
    assert!(text.contains("\nINF-123  done\n"), "{text}");
    task.ok(&["task", "move", "INF-123", "review"]);
    assert_eq!(task.state(), "active");
    // The command `end` replaced is gone.
    assert!(!task
        .run(&["task", "complete", "INF-123", "--summary", "x"])
        .status
        .success());
}

#[test]
fn a_task_with_no_workflow_reaches_end_on_one_with_nothing_between() {
    let task = WorkflowTask::new();
    task.ok(&["task", "move", "INF-123", "end"]);
    assert_eq!(task.state(), "done");
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "unplanned");
    assert_eq!(workflow["position"], at("end"));
    assert_eq!(moves(&workflow), [("set".into(), None)]);
}

#[test]
fn end_is_refused_while_the_tasks_pr_is_unsettled() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    task.repo.create_file("notes.md", "unpublished\n");
    task.repo.stage_all();
    task.repo.commit("Unpublished work");
    // By its edge or by hand, the Task stays where it was.
    for reach in [
        &["task", "run", "INF-123"][..],
        &["task", "move", "INF-123", "end"],
    ] {
        let error = refusal(task.run(reach));
        assert!(error.contains("unpublished pull request"), "{error}");
        assert_eq!(task.workflow()["position"], at("findings"));
        assert_eq!(task.state(), "active");
    }
}

#[test]
fn linear_completing_an_active_task_is_shown_and_holds_it_from_end() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    task.complete_in_linear();
    let conflict = task.status()["planning_conflict"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        conflict.contains("lf task move INF-123 end --force"),
        "{conflict}"
    );
    // Work goes on.
    task.ok(&["-b", "task", "run", "INF-123", "proof"]);
    assert_eq!(task.workflow()["position"], at("review"));
    // `end` waits for a person to say so.
    task.ok(&["task", "move", "INF-123", "accepted"]);
    for reach in [
        &["task", "run", "INF-123"][..],
        &["task", "move", "INF-123", "end"],
    ] {
        let error = refusal(task.run(reach));
        assert!(error.contains("--force"), "{error}");
        assert_eq!(task.state(), "active");
    }
    task.ok(&[
        "task", "move", "INF-123", "end", "--force", "--reason", "Agreed",
    ]);
    assert_eq!(task.state(), "done");
    assert!(task.status()["planning_conflict"].is_null());
    let workflow = task.workflow();
    let set = workflow["history"].as_array().unwrap().last().unwrap();
    assert_eq!(
        set["note"],
        "Agreed (forced: Linear already called it complete)"
    );
}

#[test]
fn linear_completing_a_task_that_never_started_withdraws_it() {
    let task = WorkflowTask::new();
    task.complete_in_linear();
    assert!(task.status()["planning_conflict"].is_null());
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "gated"]));
    assert!(error.contains("terminal"), "{error}");
    assert!(support::recorded_flows(task.home.path()).is_empty());
    assert_eq!(task.state(), "not_ready");
}
