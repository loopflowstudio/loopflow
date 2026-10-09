mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use loopflow::work::task::{GithubPr, PrPublication};
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
    &["-b", "--task", "INF-123", "flow", "proof"],
    &["-b", "flow", "proof"],
];

#[test]
fn every_task_launch_runs_in_the_foreground_under_the_same_checks() {
    for condition in [
        "valid",
        "invalid",
        "invalid_without_inventory",
        "removed",
        "terminal",
        "moved",
        "team",
    ] {
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        repo.create_branch("launch-proof");
        let home = tempfile::tempdir().unwrap();
        let _env = support::EnvGuard::new(&[
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
        ]);
        let registered = support::register_task_with_pr(
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
        record.project.as_mut().unwrap().workflow = "proof".into();
        record.project.as_mut().unwrap().revision = Some("2026-10-07T12:00:00Z".into());
        record.item.revision = Some("2026-10-04T12:00:00Z".into());
        if condition == "terminal" {
            record.item.state = Some("canceled".into());
        }
        if condition == "team" {
            record.item.team_id = Some("another-team".into());
        }
        if condition == "moved" {
            // The retained parent still selects feature; give that Workflow a proof edge
            // so this fixture reaches the planning mismatch admission check.
            fs::create_dir_all(repo.path().join(".lf/workflows")).unwrap();
            fs::write(
                repo.path().join(".lf/workflows/feature.yaml"),
                "edges:\n  - {from: start, to: end, flow: proof}\n",
            )
            .unwrap();
            record.project.as_mut().unwrap().id = "another-project".into();
            record.item.project_id = Some("another-project".into());
        }
        runtime
            .block_on(
                registered
                    .store
                    .put_pm_task(&scope, "linear", record, None, None),
            )
            .unwrap();
        // Old valid planning still admits a launch without reaching the provider.
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .execute_batch(
                "UPDATE pm_items SET observed_at=1; UPDATE pm_projects SET observed_at=1;",
            )
            .unwrap();
        if condition.starts_with("invalid") || condition == "removed" {
            runtime
                .block_on(registered.store.observe_pm_issue_change(
                    registered.task.plan.linear_id.as_ref().unwrap().as_str(),
                    None,
                    condition == "removed",
                ))
                .unwrap();
        }
        if condition == "invalid_without_inventory" {
            rusqlite::Connection::open(home.path().join("loopflow.db"))
                .unwrap()
                .execute("DELETE FROM pm_items", [])
                .unwrap();
        }
        let run = |args: &[&str]| command(repo.path(), home.path(), args).output().unwrap();
        if condition != "valid" {
            let expected = match condition {
                "terminal" => "terminal",
                "moved" | "team" => "no longer matches",
                "removed" => "deleted",
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
            assert!(!String::from_utf8_lossy(&output.stderr)
                .contains("Started requires recorded Task work"));
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
            assert!(runtime
                .block_on(registered.store.task_started(&registered.task.id))
                .unwrap());
        }
        // However the Task was named, the Flow left the same Processes.
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
        assert!(
            error.contains("lf wave workflow set <name> <wave>"),
            "{error}"
        );
        assert_eq!(support::recorded_flows(home.path()).len(), 3);
        let status = run(&["task", "status", "INF-123", "--json"]);
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(
            status["execution"]["work"]["flow_processes"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert!(status["execution"]["work"]["flow_processes"]
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

#[test]
fn failed_placement_is_observed_without_starting_a_task() {
    let task = WorkflowTask::new();
    let output = task.run(&["--task", "missing-task", "run", "proof"]);
    assert!(!output.status.success());
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let (cwd, outcome): (String, String) = db
        .query_row(
            "SELECT cwd,outcome FROM processes ORDER BY rowid DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(Path::new(&cwd), task.repo.path().canonicalize().unwrap());
    assert_eq!(outcome, "failed");
    assert!(support::recorded_flows(task.home.path()).is_empty());
    let started: Option<i64> = db
        .query_row(
            "SELECT started_at FROM tasks WHERE id=?1",
            [task.registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(started, None);
}

// Release the fixture even when an assertion fails, so no held child escapes.
struct HeldFlow {
    child: Child,
    release: PathBuf,
    caller: PathBuf,
}

impl Drop for HeldFlow {
    fn drop(&mut self) {
        let _ = fs::write(&self.release, "");
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.caller);
    }
}

#[test]
fn running_flows_belong_to_the_target_checkout_and_retain_their_caller() {
    for launch in ["task", "direct", "checkout", "alias", "wt"] {
        let task = WorkflowTask::new();
        // A real sibling checkout, managed through the public placement command.
        task.ok(&["wt", "create", "caller"]);
        // Worktree discovery is read-only and does not select an ambient Home.
        let caller = {
            loopflow::engine::worktrees::list_worktrees(task.repo.path())
                .unwrap()
                .into_iter()
                .find(|wt| wt.path != task.repo.path().canonicalize().unwrap())
                .unwrap()
                .path
        };
        let alias_root = tempfile::tempdir().unwrap();
        let alias = alias_root.path().join("target");
        #[cfg(unix)]
        std::os::unix::fs::symlink(task.repo.path(), &alias).unwrap();
        task.repo
            .create_file(".lf/flows/proof.yaml", "- cmd: __telemetry-scorecard\n");
        task.repo.create_file(
            "scripts/lifecycle_scorecard.py",
            r#"
import json
import os
from pathlib import Path
import time

home = Path(os.environ['LF_HOME'])
(home / 'ready.tmp').write_text(os.environ['LF_PROCESS_LFID'])
(home / 'ready.tmp').replace(home / 'ready')
while not (home / 'release').exists():
    time.sleep(.02)
print(json.dumps({'report': {'ok': True}, 'metric_observations': [], 'text': ''}))
"#,
        );
        let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
        let started = || {
            db.query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [task.registered.task.id.as_str()],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap()
        };
        // Explicit placement and passive inspection must not start the Task.
        let read = command(
            &caller,
            task.home.path(),
            &["--task", "INF-123", "task", "status", "INF-123", "--json"],
        )
        .output()
        .unwrap();
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert!(started().is_none());
        assert!(support::recorded_flows(task.home.path()).is_empty());

        let parent = loopflow::id::ProcessLfid::new();
        let trace = loopflow::id::TraceId::new();
        db.execute("INSERT INTO processes(lfid,trace_id,cwd,command,started_at) VALUES(?1,?2,?3,'caller',1)",
            rusqlite::params![parent, trace, caller.to_str().unwrap()]).unwrap();
        let (cwd, args): (&Path, &[&str]) = match launch {
            "task" => (&caller, &["-b", "task", "run", "INF-123", "proof"]),
            "direct" => (&caller, &["-b", "--task", "INF-123", "run", "proof"]),
            "wt" => (&caller, &["-b", "--wt", "launch-proof", "run", "proof"]),
            "alias" => (&alias, &["-b", "run", "proof"]),
            _ => (task.repo.path(), &["-b", "run", "proof"]),
        };
        let log_path = task.home.path().join("launch.log");
        let log = fs::File::create(&log_path).unwrap();
        let mut held = HeldFlow {
            child: command(cwd, task.home.path(), args)
                .env("LF_PROCESS_LFID", parent.as_str())
                .env("LF_TRACE_ID", trace.as_str())
                .stdout(log.try_clone().unwrap())
                .stderr(log)
                .spawn()
                .unwrap(),
            release: task.home.path().join("release"),
            caller: caller.clone(),
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        while !task.home.path().join("ready").exists() {
            assert!(
                held.child.try_wait().unwrap().is_none() && Instant::now() < deadline,
                "{launch}: {}",
                fs::read_to_string(&log_path).unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(started().is_some(), "{launch}");
        let status = task.status();
        let flows = status["work"]["flow_processes"].as_array().unwrap();
        assert_eq!(flows.len(), 1, "{launch}: {status}");
        assert_eq!(flows[0]["state"], "current");
        assert_eq!(flows[0]["task_id"], task.registered.task.id.as_str());
        let driver = flows[0]["id"].as_str().unwrap();
        let recorded: (String, String) = db
            .query_row(
                "SELECT cwd,parent_process_lfid FROM processes WHERE lfid=?1",
                [driver],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            Path::new(&recorded.0),
            task.repo.path().canonicalize().unwrap()
        );
        let ancestor = if launch == "task" {
            db.query_row(
                "SELECT parent_process_lfid FROM processes WHERE lfid=?1",
                [&recorded.1],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
        } else {
            recorded.1
        };
        assert_eq!(ancestor, parent.as_str());
        let original: String = db
            .query_row("SELECT cwd FROM processes WHERE lfid=?1", [parent], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(Path::new(&original), caller);
        let inventory = task.run(&[
            "flow",
            "list",
            "--processes",
            "--for-task",
            "INF-123",
            "--json",
        ]);
        assert!(
            inventory.status.success(),
            "{}",
            String::from_utf8_lossy(&inventory.stderr)
        );
        let inventory: loopflow::durable::FlowProcessPage =
            serde_json::from_slice(&inventory.stdout).unwrap();
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(inventory.entries[0].summary.id, driver);
        assert_eq!(
            inventory.entries[0].summary.task_id.as_ref(),
            Some(&task.registered.task.id)
        );
        fs::write(&held.release, "").unwrap();
        assert!(
            held.child.wait().unwrap().success(),
            "{}",
            fs::read_to_string(&log_path).unwrap()
        );
        assert!(!fs::read_to_string(&log_path)
            .unwrap()
            .contains("did not record"));
        println!("{launch}: Started; target status and inventory show current Flow; caller retained; exit 0");
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
            // An ordinary repeatable edge returns to its review node.
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
        repo.create_branch("launch-proof");
        let registered = support::register_task_without_pr(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "launch-proof",
            &repo.head_sha(),
        );
        let fixture = Self {
            repo,
            home,
            registered,
            _env: env,
        };
        fixture.ok(&["task", "checkout", "INF-123"]);
        fixture
    }

    fn select_workflow(&self, name: &str) {
        let store =
            loopflow::store::sqlite::SqliteStore::new(&self.home.path().join("loopflow.db"))
                .unwrap();
        let content =
            fs::read_to_string(self.repo.path().join(format!(".lf/workflows/{name}.yaml")))
                .unwrap();
        store
            .select_project_workflow(&self.registered.task.project_id, name, &content)
            .unwrap();
    }

    fn publish(&self, merged: bool) {
        let mut pr = self.registered.pr.clone();
        pr.publication = Some(PrPublication {
            requested_at: time::OffsetDateTime::now_utc(),
            presentation: None,
            github: Some(GithubPr {
                number: 42,
                url: "https://github.com/fixture/repo/pull/42".into(),
                head_sha: Some(self.repo.head_sha()),
            }),
            merge: None,
        });
        if merged {
            pr.merge_commit = Some(self.repo.head_sha());
        }
        let runtime = tokio::runtime::Runtime::new().unwrap();
        if runtime
            .block_on(self.registered.store.active_task_pr(&pr.task_id))
            .unwrap()
            .is_some()
        {
            runtime
                .block_on(self.registered.store.update_task_pr(&pr))
                .unwrap();
        } else {
            runtime
                .block_on(self.registered.store.insert_task_pr(&pr))
                .unwrap();
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

    /// Planning status, independent of Workflow position.
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
            .block_on(store.put_pm_task(&scope, "linear", record, None, None))
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

    fn finish_after_provider_observation(&self, args: &[&str]) {
        self.ok(args);
        assert_eq!(self.state(), "done");
        assert!(self.status()["completion_pending"].is_null());
        let workflow = self.workflow();
        self.complete_in_linear();
        self.ok(&["task", "complete", "INF-123"]);
        assert_eq!(self.workflow(), workflow);
        assert_eq!(self.state(), "done");
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
fn checkout_leaves_workflow_selection_to_the_first_run() {
    let fixture = WorkflowTask::new();
    fixture.repo.push_new_branch("main");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = &fixture.registered.store;
    let sqlite =
        loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
            .unwrap();
    let mut snapshot = runtime
        .block_on(store.pm_snapshot(&fixture.registered.task.wave_id))
        .unwrap()
        .unwrap();
    let mut item = snapshot.snapshot.items[0].clone();
    item.id = "unplaced-issue".into();
    item.identifier = "INF-124".into();
    item.name = "Workflow chosen at first run".into();
    item.branch_name = None;
    snapshot.snapshot.items.push(item);
    runtime
        .block_on(store.put_pm_snapshot(snapshot, None))
        .unwrap();
    let task = runtime
        .block_on(store.get_task_by_issue("INF-124"))
        .unwrap()
        .unwrap();
    let select = |name: &str| {
        sqlite.select_project_workflow(
            &task.project_id,
            name,
            "nodes: {review: demo}\nedges: [{from: start, to: review, flow: proof}, {from: review, to: end}]\n",
        ).unwrap();
    };
    select("before-checkout");
    fixture.ok(&["task", "checkout", "INF-124", "--name", "first-workflow"]);
    assert!(sqlite.task_work(&task.id).unwrap().workflow.is_none());
    select("at-first-run");
    fixture.ok(&["-b", "task", "run", "INF-124"]);
    let captured = sqlite.task_work(&task.id).unwrap().workflow.unwrap();
    assert_eq!(captured.definition.name, "at-first-run");
    assert_eq!(
        serde_json::to_value(&captured.position).unwrap(),
        at("review")
    );
    select("after-first-run");
    fixture.ok(&["-b", "task", "run", "INF-124"]);
    let retained = sqlite.task_work(&task.id).unwrap().workflow.unwrap();
    assert_eq!(retained.definition, captured.definition);
    assert_eq!(serde_json::to_value(&retained.position).unwrap(), at("end"));
}

#[test]
fn a_task_takes_up_its_projects_workflow_and_keeps_one_it_named() {
    // Nothing named, no Workflow yet: the Project's.
    let task = WorkflowTask::new();
    let preview = task.run(&["task", "run", "--explain", "--json"]);
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).unwrap();
    assert_eq!(preview["action"]["to"], "review");
    task.ok(&["-b", "task", "run"]);
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "feature");
    assert_eq!(workflow["position"], at("review"));
    drop(task);

    // A Task that named its own keeps it when later runs name nothing.
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    task.finish_after_provider_observation(&["-b", "task", "run", "INF-123"]);
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
    // The edge's Flow is an ordinary Flow process; its driver chose the edge and,
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
    assert_eq!(history[1]["process_lfid"], history[2]["process_lfid"]);
    assert_eq!(history[1]["actor"], "person");
    assert_eq!(history[2]["actor"], "edge");
    task.repo
        .create_file("findings.md", "Accepted recommendation\n");
    task.repo.stage_all();
    task.repo.commit("Record research findings");
    task.repo
        .create_file("draft.md", "Retained working notes\n");
    task.finish_after_provider_observation(&["task", "run", "INF-123"]);
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("end"));
    assert_eq!(
        fs::read_to_string(task.repo.path().join("draft.md")).unwrap(),
        "Retained working notes\n"
    );
    assert!(task.repo.path().join("findings.md").exists());
    assert_eq!(moves(&workflow).last(), Some(&chose(1)));
    assert_eq!(support::recorded_flows(task.home.path()).len(), 1);
    // Reaching the end completes research without creating a PR.
    assert_eq!(task.state(), "done");
    assert!(task.status()["pr"].is_null());
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
    task.finish_after_provider_observation(&[
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
    // One Flow process reached the node; the failing edge was attempted three times.
    assert_eq!(support::recorded_flows(task.home.path()).len(), 4);
    // The Task is done; nothing runs until it is put back on its workflow.
    assert_eq!(task.state(), "done");
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "findings"]));
    assert!(error.contains("lf task reopen INF-123"), "{error}");
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
fn task_edges_launch_flows_named_after_collection_commands() {
    let task = WorkflowTask::new();
    fs::write(
        task.repo.path().join(".lf/flows/list.yaml"),
        "- cmd: task sync --plan\n",
    )
    .unwrap();
    fs::write(
        task.repo.path().join(".lf/workflows/reserved.yaml"),
        "nodes:\n  review: demo\nedges:\n  - {from: start, to: review, flow: list}\n",
    )
    .unwrap();
    task.ok(&["-b", "task", "run", "INF-123", "reserved"]);
    assert_eq!(
        task.workflow()["position"],
        serde_json::json!({"kind":"node","node":"review"})
    );
    assert_eq!(support::recorded_flows(task.home.path()).len(), 1);
}

#[test]
fn a_plain_flow_process_in_the_worktree_does_not_move_the_task() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    let before = task.workflow();
    task.ok(&["-b", "flow", "land-proof"]);
    task.ok(&["-b", "--task", "INF-123", "flow", "land-proof"]);
    assert_eq!(support::recorded_flows(task.home.path()).len(), 3);
    assert_eq!(task.workflow(), before);
    // A workflow is traversed, never run as a Flow.
    let refused = task.run(&["-b", "flow", "rounds"]);
    assert!(!refused.status.success());
    let error = String::from_utf8_lossy(&refused.stderr).to_string();
    assert!(error.contains("lf task run <issue> rounds"), "{error}");
}

#[test]
fn a_task_takes_up_the_workflow_when_an_autonomous_flow_has_the_same_name() {
    let task = WorkflowTask::new();
    fs::write(
        task.repo.path().join(".lf/flows/feature.yaml"),
        "- cmd: task sync --plan\n",
    )
    .unwrap();
    task.ok(&["-b", "task", "run", "INF-123", "feature"]);
    let captured = task.workflow();
    assert_eq!(captured["name"], "feature");
    assert_eq!(
        captured["position"],
        serde_json::json!({"kind":"node","node":"review"})
    );
    task.ok(&["-b", "flow", "feature"]);
    assert_eq!(task.workflow(), captured);
    assert_eq!(support::recorded_flows(task.home.path()).len(), 2);
}

/// The Flow processes started beneath `task_run`, by outcome, oldest first.
fn attempts(home: &Path, task_run: &str) -> Vec<String> {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let mut rows = db
        .prepare(
            "SELECT d.outcome FROM flow_processes f JOIN processes d ON d.lfid=f.process_lfid
             WHERE d.parent_process_lfid=?1 ORDER BY d.rowid",
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
        chose["process_lfid"].as_str().unwrap().to_string()
    };
    // Every attempt fails: the Task run gives up and the edge holds the Task.
    let output = task.run(&["-b", "task", "run", "INF-123", "gate"]);
    let error = refusal(output);
    assert!(error.contains("Flow gate failed 3 times"), "{error}");
    let stopped = task.workflow();
    assert_eq!(stopped["position"]["edge"], 2);
    assert_eq!(stopped["position"]["running"], false);
    assert_eq!(
        stopped["position"]["process_lfid"],
        carrier(&stopped).as_str()
    );
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
    // One Task run: chosen once, arrived once, three Flow processes beneath it.
    let kinds: Vec<_> = moves(&workflow).into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(
        kinds,
        ["took_up", "chose", "arrived", "chose", "chose", "arrived"]
    );
    let arrived = workflow["history"].as_array().unwrap().last().unwrap();
    let task_run = arrived["process_lfid"].as_str().unwrap();
    assert_eq!(
        attempts(task.home.path(), task_run),
        ["failed", "failed", "succeeded"]
    );
    assert_eq!(support::recorded_flows(task.home.path()).len(), 7);
}

#[test]
fn a_held_command_stops_the_task_run_without_repeating_the_flow() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    let bin = tempfile::tempdir().unwrap();
    let lf = bin.path().join("lf");
    fs::write(
        &lf,
        format!(
            "#!/bin/sh\ncase \"$*\" in *'flow show late'*) exit 3;; esac\nexec '{}' \"$@\"\n",
            env!("CARGO_BIN_EXE_lf")
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
    assert_eq!(
        output.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(support::recorded_flows(task.home.path()).len(), 2);
    let workflow = task.workflow();
    assert_eq!(workflow["position"]["kind"], "edge");
    assert_eq!(workflow["position"]["running"], false);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("starting it again"));
}

#[test]
fn completion_is_independent_of_workflow_readiness() {
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
    // Moving away from end does not reopen a completed Task.
    task.finish_after_provider_observation(&[
        "task",
        "move",
        "INF-123",
        "end",
        "--reason",
        "Delivered",
    ]);
    assert_eq!(task.state(), "done");
    let text =
        String::from_utf8_lossy(&task.run(&["task", "status", "INF-123"]).stdout).to_string();
    assert!(text.contains("\nINF-123  done\n"), "{text}");
    task.ok(&["task", "move", "INF-123", "review"]);
    assert_eq!(task.state(), "done");
    // The retired completion flags do not return with the alias.
    assert!(!task
        .run(&["task", "complete", "INF-123", "--summary", "x"])
        .status
        .success());
}

#[test]
fn a_task_with_no_workflow_reaches_end_on_one_with_nothing_between() {
    let task = WorkflowTask::new();
    task.finish_after_provider_observation(&["task", "move", "INF-123", "end"]);
    assert_eq!(task.state(), "done");
    let workflow = task.workflow();
    assert_eq!(workflow["name"], "unplanned");
    assert_eq!(workflow["position"], at("end"));
    assert_eq!(moves(&workflow), [("set".into(), None)]);
}

#[test]
fn end_is_retained_while_the_tasks_pr_blocks_completion() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    task.publish(false);
    // Arrival is retained while each completion retry still refuses unsettled delivery.
    for reach in [
        &["task", "run", "INF-123"][..],
        &["task", "move", "INF-123", "end"],
        &["task", "complete", "INF-123"],
    ] {
        let error = refusal(task.run(reach));
        assert!(error.to_lowercase().contains("pull request"), "{error}");
        assert_eq!(task.workflow()["position"], at("end"));
        assert_eq!(task.state(), "active");
    }
}

#[test]
fn local_reopening_preserves_delivery_and_workflow_and_supersedes_pending_completion() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    task.publish(false);
    assert!(!task
        .run(&["task", "move", "INF-123", "end"])
        .status
        .success());
    assert!(!task.status()["completion_pending"].is_null());
    let workflow = task.workflow();
    let flows = support::recorded_flows(task.home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let pr = runtime
        .block_on(
            task.registered
                .store
                .active_task_pr(&task.registered.task.id),
        )
        .unwrap();
    task.ok(&[
        "task",
        "reopen",
        "INF-123",
        "--reason",
        "Continue accepted work",
    ]);
    assert!(task.status()["completion_pending"].is_null());
    assert_eq!(task.workflow(), workflow);
    assert_eq!(support::recorded_flows(task.home.path()), flows);
    assert_eq!(
        runtime
            .block_on(
                task.registered
                    .store
                    .active_task_pr(&task.registered.task.id)
            )
            .unwrap(),
        pr
    );
    task.ok(&["task", "move", "INF-123", "end"]);
    assert!(task.status()["completion_pending"].is_null());
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    task.ok(&["task", "reopen", "INF-123"]);
    assert_eq!(task.state(), "active");
    assert_eq!(task.workflow(), workflow);
    task.ok(&["task", "reopen", "INF-123"]);
    assert_eq!(task.workflow(), workflow);
    assert!(task.repo.path().exists());
}

#[test]
fn linear_completion_and_reopening_preserve_workflow_and_supersede_old_end() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    let workflow = task.workflow();
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    assert_eq!(task.workflow(), workflow);
    assert!(task.status()["completion_pending"].is_null());
    task.ok(&["task", "move", "INF-123", "end"]);
    let ended = task.workflow();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let scope = task
        .repo
        .path()
        .canonicalize()
        .unwrap()
        .display()
        .to_string();
    let store = &task.registered.store;
    let mut record = rt
        .block_on(store.pm_task_observation(&scope, "linear", "INF-123"))
        .unwrap()
        .record
        .unwrap();
    let stale = record.clone();
    record.item.revision = Some("2026-10-07T12:00:00Z".into());
    record.item.state = Some("started".into());
    record.item.completed = false;
    rt.block_on(store.put_pm_task(&scope, "linear", record, None, None))
        .unwrap();
    rt.block_on(store.put_pm_task(&scope, "linear", stale, None, None))
        .unwrap();
    assert_eq!(task.state(), "active");
    assert_eq!(task.workflow(), ended);
    assert!(task.status()["completion_pending"].is_null());
    task.ok(&["task", "move", "INF-123", "end"]);
    assert_eq!(task.state(), "active");
}

#[test]
fn linear_completing_a_task_that_never_started_withdraws_it() {
    let task = WorkflowTask::new();
    task.complete_in_linear();
    assert!(task.status()["completion_pending"].is_null());
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "gated"]));
    assert!(
        error.contains("terminal") || error.contains("is done"),
        "{error}"
    );
    assert!(support::recorded_flows(task.home.path()).is_empty());
    assert_eq!(task.state(), "done");
}

#[test]
fn workflow_restart_keeps_the_captured_graph_and_execution_history() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    let before = task.workflow();
    let processes = support::recorded_flows(task.home.path());
    fs::write(
        task.repo.path().join(".lf/workflows/rounds.yaml"),
        "invalid: source\n",
    )
    .unwrap();
    task.ok(&["task", "workflow", "restart", "INF-123"]);
    let after = task.workflow();
    assert_eq!(
        after["position"],
        serde_json::json!({"kind":"node","node":"start"})
    );
    assert_eq!(after["nodes"], before["nodes"]);
    assert_eq!(after["edges"], before["edges"]);
    assert_eq!(
        after["history"].as_array().unwrap().len(),
        before["history"].as_array().unwrap().len() + 1
    );
    assert_eq!(support::recorded_flows(task.home.path()), processes);
    let shown = task.run(&["task", "workflow", "show", "INF-123", "--json"]);
    assert!(shown.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&shown.stdout).unwrap(),
        after
    );
}

#[test]
fn completion_preserves_retained_session_input_and_unknown_process_history() {
    let task = WorkflowTask::new();
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let exec = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    db.execute("INSERT INTO processes(lfid,trace_id,cwd,started_at,command) VALUES(?1,?1,?2,?3,'historical inspection')",
        rusqlite::params![exec, task.repo.path().canonicalize().unwrap().to_str().unwrap(), now]).unwrap();
    db.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,interactive,task_id,wave_id,cwd)
        VALUES('retained-input','retained input','generated',?1,0,0,?2,?3,?4)",
        rusqlite::params![now, task.registered.task.id.as_str(), task.registered.task.wave_id.as_str(), task.repo.path().to_str().unwrap()]).unwrap();
    db.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('retained-input','captured',?1,1,'{}')", [uuid::Uuid::new_v4().simple().to_string()]).unwrap();
    db.execute(
        "UPDATE agent_sessions SET current_capture=?1 WHERE id='retained-input'",
        [db.last_insert_rowid()],
    )
    .unwrap();
    let sessions: String = db.query_row("SELECT json_object('published',input_published,'completed',completed_at,'cwd',cwd) FROM agent_sessions WHERE id='retained-input'", [], |row| row.get(0)).unwrap();
    task.finish_after_provider_observation(&["task", "complete", "INF-123"]);
    assert!(task.workflow().is_null());
    for args in [
        &["task", "complete", "INF-123"][..],
        &["task", "move", "INF-123", "end"][..],
        &["task", "complete", "INF-123"][..],
    ] {
        task.ok(args);
        assert_eq!(task.state(), "done");
        assert!(task.repo.path().exists());
    }
    let after: String = db.query_row("SELECT json_object('published',input_published,'completed',completed_at,'cwd',cwd) FROM agent_sessions WHERE id='retained-input'", [], |row| row.get(0)).unwrap();
    assert_eq!(sessions, after);
    let unfinished: bool = db
        .query_row(
            "SELECT completed_at IS NULL AND outcome IS NULL FROM processes WHERE lfid=?1",
            [&exec],
            |row| row.get(0),
        )
        .unwrap();
    assert!(unfinished);
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM task_events WHERE json_extract(kind_json,'$.kind')='completed'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn merged_delivery_requires_a_disposition_and_follow_through_completion_is_idempotent() {
    let task = WorkflowTask::new();
    task.publish(true);
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    for args in [
        &["task", "run", "INF-123"][..],
        &["task", "complete", "INF-123"],
        &[
            "task",
            "move",
            "INF-123",
            "end",
            "--reason",
            "Try bypassing",
        ],
    ] {
        let error = refusal(task.run(args));
        assert!(error.to_lowercase().contains("follow-through"), "{error}");
        assert_eq!(task.workflow()["position"], at("end"));
        assert_eq!(task.state(), "active");
    }
    assert!(task.repo.path().exists());
    task.ok(&[
        "task",
        "follow-up",
        "INF-123",
        "--none",
        "Accepted checks are complete",
    ]);
    task.ok(&[
        "task",
        "follow-up",
        "INF-123",
        "--none",
        "Retry does not replace the reason",
    ]);
    task.finish_after_provider_observation(&["task", "complete", "INF-123"]);
    let workflow = task.workflow();
    task.ok(&["task", "complete", "INF-123"]);
    task.ok(&["task", "move", "INF-123", "end"]);
    assert_eq!(task.workflow(), workflow);
    let status = task.status();
    assert_eq!(status["status"], "done");
    assert_eq!(status["pr"]["publication"]["github"]["number"], 42);
    assert_eq!(
        status["follow_through"]["reason"],
        "Accepted checks are complete"
    );
}

#[test]
fn provider_completed_delivery_can_file_and_finish_without_reopening() {
    use std::os::unix::fs::PermissionsExt;

    let task = WorkflowTask::new();
    task.publish(true);
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    task.complete_in_linear();
    let before = task.status();
    let workflow = task.workflow();
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let processes: Vec<(String, Option<i64>, Option<String>)> = db
        .prepare("SELECT lfid,completed_at,outcome FROM processes")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    // Reconciliation/completion cannot remove the workspace still needed to file.
    task.ok(&["task", "complete", "INF-123"]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let saved = runtime
        .block_on(task.registered.store.get_task(&task.registered.task.id))
        .unwrap()
        .unwrap();
    assert!(saved.worktree.as_ref().unwrap().exists());
    assert_eq!(task.state(), "done");

    // A normal edge or a misleadingly named local Flow cannot start new work.
    let error = refusal(task.run(&["-b", "--task", "INF-123", "run", "proof"]));
    assert!(error.contains("terminal"), "{error}");
    let override_path = task.repo.path().join(".lf/flows/finish-delivery.yaml");
    fs::write(&override_path, "- cmd: task sync --plan\n").unwrap();
    assert!(
        refusal(task.run(&["-b", "--task", "INF-123", "run", "finish-delivery"]))
            .contains("terminal")
    );
    fs::remove_file(override_path).unwrap();

    // Simulate the agent's judgment, not the filing operations: its commands
    // cross the same public CLI/store path used by the operator's recovery Flow.
    support::register_codex_account(task.home.path());
    let bin = tempfile::tempdir().unwrap();
    let provider = bin.path().join("codex");
    fs::write(
        &provider,
        support::codex_app_server_script(
            "Accepted follow-through filed",
            r#"set -eu
if [ "$1" = --version ]; then echo "codex fixture"; exit 0; fi
for attempt in 1 2; do
  "$LF_BIN" task follow-up INF-123 --title "Verify installed command" --notes "Run the released command; retain its result" --due 2026-10-09 >&2
done
for attempt in 1 2; do
  "$LF_BIN" task follow-up INF-123 --finish "Installed proof belongs to the child" >&2
done
"$LF_BIN" task complete INF-123 >&2
"#,
        ),
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    let output = command(
        task.repo.path(),
        task.home.path(),
        &[
            "-b",
            "--agent",
            "codex",
            "--task",
            "INF-123",
            "run",
            "finish-delivery",
        ],
    )
    .env("HOME", task.home.path())
    .env("CODEX_HOME", task.home.path().join(".codex"))
    .env(
        "PATH",
        format!(
            "{}:{}",
            bin.path().display(),
            std::env::var("PATH").unwrap()
        ),
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let after = task.status();
    assert_eq!(after["status"], "done");
    assert_eq!(task.workflow(), workflow);
    assert_eq!(after["pr"], before["pr"]);
    assert_eq!(
        after["follow_through"]["intents"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        after["follow_through"]["links"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        after["follow_through"]["reason"],
        "Installed proof belongs to the child"
    );
    let child_id = after["follow_through"]["intents"][0]["issue_id"]
        .as_str()
        .unwrap();
    let child = runtime
        .block_on(task.registered.store.get_task_by_issue(child_id))
        .unwrap()
        .unwrap();
    assert!(
        !loopflow::store::sqlite::SqliteStore::new(&task.home.path().join("loopflow.db"))
            .unwrap()
            .task_state(&child.id)
            .unwrap()
            .is_terminal()
    );
    assert_eq!(
        runtime
            .block_on(task.registered.store.list_tasks(None))
            .unwrap()
            .len(),
        2
    );
    assert_eq!(db.query_row(
        "SELECT count(*) FROM task_events WHERE task_id=?1 AND json_extract(kind_json,'$.kind')='follow_through_disposition'",
        [task.registered.task.id.as_str()], |row| row.get::<_, i64>(0)).unwrap(), 1);
    for (id, completed, outcome) in processes {
        assert_eq!(
            db.query_row(
                "SELECT completed_at,outcome FROM processes WHERE lfid=?1",
                [id],
                |row| Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<String>>(1)?
                ))
            )
            .unwrap(),
            (completed, outcome)
        );
    }
    let disposition = after["follow_through"].clone();
    task.ok(&[
        "task",
        "follow-up",
        "INF-123",
        "--title",
        "Ignored retry",
        "--notes",
        "The saved obligation wins",
    ]);
    task.ok(&[
        "task",
        "follow-up",
        "INF-123",
        "--finish",
        "Ignored retry reason",
    ]);
    assert_eq!(task.status()["follow_through"], disposition);
    assert_eq!(
        runtime
            .block_on(task.registered.store.get_task(&saved.id))
            .unwrap()
            .unwrap()
            .worktree,
        saved.worktree
    );
    // Resolved delivery admits neither another obligation nor another recovery Flow.
    assert!(!task
        .run(&[
            "task",
            "follow-up",
            "INF-123",
            "--key",
            "new-scope",
            "--title",
            "Unaccepted work",
            "--notes",
            "Not part of delivery"
        ])
        .status
        .success());
    assert!(!task
        .run(&["-b", "--task", "INF-123", "run", "finish-delivery"])
        .status
        .success());
    assert_eq!(task.workflow(), workflow);
}

#[test]
fn provider_completion_without_merged_delivery_does_not_admit_first_filing() {
    for published in [false, true] {
        let task = WorkflowTask::new();
        if published {
            task.publish(false);
        }
        task.complete_in_linear();
        task.ok(&["task", "complete", "INF-123"]);
        assert!(task.registered.task.worktree.as_ref().unwrap().exists());
        assert!(!task
            .run(&[
                "task",
                "follow-up",
                "INF-123",
                "--title",
                "New scope",
                "--notes",
                "No accepted merged delivery"
            ])
            .status
            .success());
        assert!(!task
            .run(&["-b", "--task", "INF-123", "run", "finish-delivery"])
            .status
            .success());
        assert_eq!(task.state(), "done");
        assert!(task.status()["follow_through"]["intents"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}

#[test]
fn failed_completion_in_finishing_flow_is_retryable_without_replaying_it() {
    let task = WorkflowTask::new();
    fs::write(
        task.repo.path().join(".lf/flows/finish-proof.yaml"),
        "- cmd: task complete INF-123\n",
    )
    .unwrap();
    fs::write(
        task.repo.path().join(".lf/workflows/delivery.yaml"),
        "edges:\n  - {from: start, to: end, flow: finish-proof}\n",
    )
    .unwrap();
    task.publish(true);
    task.select_workflow("delivery");
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "delivery"]));
    assert!(task.status()["completion_pending"].is_string(), "{error}");
    task.ok(&[
        "task",
        "follow-up",
        "INF-123",
        "--none",
        "Accepted checks complete",
    ]);
    task.ok(&["task", "complete", "INF-123"]);
    task.ok(&["task", "move", "INF-123", "end"]);
    assert_eq!(task.state(), "done");
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("end"));
    let ends = workflow["history"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["to"] == "end" && entry["kind"] != "chose")
        .count();
    assert_eq!(ends, 1);
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.workflow(), workflow);
    let flows = support::recorded_flows(task.home.path());
    assert!(!flows.is_empty());
    assert!(flows.iter().all(|flow| flow.0.as_deref() == Some("failed")));
}

#[test]
fn provider_completion_preserves_a_live_edge_and_the_driver_records_its_real_arrival() {
    let task = WorkflowTask::new();
    let worktree = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(task.registered.store.get_task(&task.registered.task.id))
        .unwrap()
        .unwrap()
        .worktree
        .unwrap();
    let write = |path: &str, content: &str| {
        let path = worktree.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    };
    write(
        ".lf/workflows/live-workflow.yaml",
        "edges:\n  - {from: start, to: end, flow: live}\n",
    );
    write(
        ".lf/flows/live.yaml",
        "- cmd: __telemetry-scorecard\n- cmd: task complete INF-123\n",
    );
    write(
        "scripts/lifecycle_scorecard.py",
        r#"
import json
import os
from pathlib import Path
import time
home = Path(os.environ['LF_HOME'])
(home / 'ready').write_text('ready')
while not (home / 'release').exists():
    time.sleep(.02)
print(json.dumps({'report': {'ok': True}, 'metric_observations': [], 'text': ''}))
"#,
    );
    task.ok(&[
        "project",
        "workflow",
        "set",
        task.registered.task.project_id.as_str(),
        "live-workflow",
        "--file",
        worktree
            .join(".lf/workflows/live-workflow.yaml")
            .to_str()
            .unwrap(),
    ]);
    let log_path = task.home.path().join("live.log");
    let log = fs::File::create(&log_path).unwrap();
    let mut held = HeldFlow {
        child: command(
            task.repo.path(),
            task.home.path(),
            &["-b", "task", "run", "INF-123", "live-workflow"],
        )
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap(),
        release: task.home.path().join("release"),
        caller: task.home.path().join("unused"),
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while !task.home.path().join("ready").exists() {
        assert!(
            held.child.try_wait().unwrap().is_none() && Instant::now() < deadline,
            "{}",
            fs::read_to_string(&log_path).unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let before = task.workflow();
    assert_eq!(before["position"]["running"], true);
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    assert_eq!(task.workflow(), before);
    task.ok(&["task", "reopen", "INF-123"]);
    assert_eq!(task.state(), "active");
    assert_eq!(task.workflow(), before);
    assert!(held.child.try_wait().unwrap().is_none());
    assert!(task.repo.path().exists());
    fs::write(&held.release, "").unwrap();
    assert!(
        held.child.wait().unwrap().success(),
        "{}",
        fs::read_to_string(&log_path).unwrap()
    );
    let after = task.workflow();
    assert_eq!(after["position"], at("end"));
    assert_eq!(
        after["history"].as_array().unwrap().last().unwrap()["kind"],
        "arrived"
    );
    assert_eq!(support::recorded_flows(task.home.path()).len(), 1);
    assert_eq!(
        support::recorded_flows(task.home.path())[0].0.as_deref(),
        Some("succeeded")
    );
}

#[test]
fn successful_final_flow_keeps_arrival_when_completion_fails_and_never_replays() {
    let task = WorkflowTask::new();
    task.repo.create_file(
        ".lf/workflows/final.yaml",
        "edges:\n  - {from: start, to: end, flow: proof}\n",
    );
    task.select_workflow("final");
    task.publish(false);
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "final"]));
    assert!(task.status()["completion_pending"].is_string(), "{error}");
    let arrived = task.workflow();
    assert_eq!(arrived["position"], at("end"));
    assert_eq!(
        arrived["history"].as_array().unwrap().last().unwrap()["kind"],
        "arrived"
    );
    let flows = support::recorded_flows(task.home.path());
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].0.as_deref(), Some("succeeded"));
    task.publish(true);
    task.ok(&[
        "task",
        "follow-up",
        "INF-123",
        "--none",
        "Accepted checks complete",
    ]);
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.workflow(), arrived);
    assert_eq!(support::recorded_flows(task.home.path()), flows);
}
