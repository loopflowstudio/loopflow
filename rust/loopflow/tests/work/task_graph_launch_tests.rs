use crate::test_linear_input::SeedLinear;
use crate::test_support as support;
use loopflow_execution::test_support::task_graph::holding_claude;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use loopflow_execution::work::task::{GithubPr, PrPublication};
use loopflow_test_support::{MachineServer, TestRepo};

fn command(repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(loopflow_test_support::lf_binary());
    for (name, _) in
        std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
    {
        command.env_remove(name);
    }
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", loopflow_test_support::lf_binary())
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("HTTP_PROXY", "http://127.0.0.1:1");
    command
}

/// The three ways to name a Task for a Graph: the Task entry, `--task`, and its
/// worktree. All block in the foreground; only the Task entry requires
/// admission to start Task work.
const LAUNCHES: [&[&str]; 3] = [
    &["-b", "task", "run", "INF-123", "proof"],
    &["-b", "--task", "INF-123", "graph", "proof"],
    &["-b", "graph", "proof"],
];

/// Linear received lf's state writes before someone changed the Task there; an
/// unsent one would otherwise outrank what Linear reports next.
fn deliver_state_writes(
    store: &loopflow_execution::store::Store,
    task: &loopflow_model::durable::TaskId,
) {
    use loopflow_storage::sqlite::tracker_actions::{
        ActionKind, ActionOwner, ActionPhase, Resolution,
    };
    let delivered = Resolution::Confirmed {
        revision: None,
        remote_id: None,
    };
    for action in store
        .sqlite
        .tracker_actions_for(&ActionOwner::Task(task.clone()))
        .unwrap()
    {
        if action.kind == ActionKind::State
            && matches!(action.phase, ActionPhase::Queued | ActionPhase::Sent)
        {
            store
                .sqlite
                .resolve_tracker_action(&action.id, &delivered)
                .unwrap();
        }
    }
}

#[test]
fn task_run_checks_admission_while_standalone_graphs_run_with_task_context() {
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
        let config = repo.path().join(".lf/config.yaml");
        fs::write(
            &config,
            format!("{}\ngraph: proof\n", fs::read_to_string(&config).unwrap()),
        )
        .unwrap();
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
        fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
        // The repository selects `proof`, initially only a Graph.
        fs::write(
            repo.path().join(".lf/graphs/proof.yaml"),
            "- cmd: task sync --plan\n",
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let scope = repo.path().canonicalize().unwrap().display().to_string();
        let mut record: crate::test_linear_input::IssueFixture =
            loopflow_test_support::retained_task_input(&registered.database, &scope, "INF-123");
        record.project.as_mut().unwrap().revision = Some("2026-10-07T12:00:00Z".into());
        record.item.revision = Some("2026-10-04T12:00:00Z".into());
        if condition == "terminal" {
            record.item.state = Some("canceled".into());
        }
        if condition == "team" {
            record.item.team_id = Some("another-team".into());
        }
        if condition == "moved" {
            record.project.as_mut().unwrap().id = "another-project".into();
            record.item.project_id = Some("another-project".into());
        }
        deliver_state_writes(&registered.store, &registered.task.id);
        runtime
            .block_on(registered.store.seed_issue(&scope, "linear", record, None))
            .unwrap();
        // Old valid planning still admits a launch without reaching the provider.
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .execute_batch(
                "UPDATE tracker_items SET observed_at=1; UPDATE tracker_projects SET observed_at=1;",
            )
            .unwrap();
        if condition.starts_with("invalid") || condition == "removed" {
            runtime
                .block_on(registered.store.record_tracker_event(
                    registered.task.tracker_id.as_ref().unwrap().as_str(),
                    None,
                    condition == "removed",
                ))
                .unwrap();
        }
        if condition == "invalid_without_inventory" {
            rusqlite::Connection::open(home.path().join("loopflow.db"))
                .unwrap()
                .execute("DELETE FROM tracker_items", [])
                .unwrap();
        }
        let run = |args: &[&str]| command(repo.path(), home.path(), args).output().unwrap();
        if condition != "valid" {
            let expected = match condition {
                "terminal" => "terminal",
                "moved" | "team" => "invalid ownership",
                "removed" => "deleted",
                _ => "planning",
            };
            for (launched, launch) in LAUNCHES.iter().enumerate() {
                let output = run(launch);
                let error = String::from_utf8_lossy(&output.stderr);
                let graphs = support::recorded_graphs(home.path());
                if launched == 0 {
                    assert!(!output.status.success(), "{condition} admitted {launch:?}");
                    assert!(error.contains(expected), "{condition} {launch:?}: {error}");
                    assert!(graphs.is_empty(), "{condition} started Task work");
                } else {
                    // The Graph starts even when its `task sync --plan` command
                    // cannot use the retained planning input.
                    assert_eq!(graphs.len(), launched, "{condition} {launch:?}: {error}");
                    let (outcome, steps) = graphs.last().unwrap();
                    assert_eq!(steps.len(), 1, "{condition} {launch:?}: {error}");
                    assert_eq!(steps[0]["label"], "sync --plan");
                    assert_eq!(
                        outcome.as_deref(),
                        Some(if output.status.success() {
                            "succeeded"
                        } else {
                            "failed"
                        }),
                        "{condition} {launch:?}: {error}"
                    );
                }
            }
            continue;
        }
        // Each launch has finished when its command returns, and is its own
        // Graph; none continues another. The Task run takes up `proof` and
        // reaches its end; completion waits for the unmerged PR.
        for (launched, launch) in LAUNCHES.iter().enumerate() {
            let output = run(launch);
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!error.contains("Started requires recorded Task work"));
            if launched == 0 {
                assert!(error.contains("pull request has not merged"), "{error}");
            } else {
                assert!(output.status.success(), "{launch:?}: {error}");
            }
            let graphs = support::recorded_graphs(home.path());
            assert_eq!(graphs.len(), launched + 1, "{launch:?}");
            assert!(graphs
                .iter()
                .all(|(outcome, _)| outcome.as_deref() == Some("succeeded")));
            assert!(runtime
                .block_on(registered.store.task_owned(&registered.task.id))
                .unwrap());
        }
        // However the Task was named, the Graph left the same Processes.
        let recorded = support::recorded_graphs(home.path());
        assert_eq!(recorded[0].1.len(), 1);
        assert_eq!(recorded[0].1[0]["label"], "sync --plan");
        assert!(recorded.iter().all(|(_, steps)| *steps == recorded[0].1));
        // At its end the Task has nothing left to run; completion retries
        // without replaying the run that reached it.
        let output = run(&["-b", "task", "run", "INF-123"]);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("is at the end of graph proof"), "{error}");
        let output = run(&["task", "complete", "INF-123"]);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("pull request has not merged"), "{error}");
        assert_eq!(support::recorded_graphs(home.path()).len(), 3);
        let status = run(&["desktop", "task", "INF-123", "--json"]);
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(
            status["execution"]["work"]["graph_processes"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert!(status["execution"]["work"]["graph_processes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|graph| graph["state"] == "completed"));
        let graph = &status["execution"]["work"]["graph"];
        assert_eq!(
            (&graph["name"], &graph["position"]),
            (&"proof".into(), &at("end"))
        );
        assert!(status["execution"]["completion_pending"].is_string());
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
        // The worker's entry points are gone; nothing restarts a Graph.
        for removed in [
            vec!["--task", "INF-123", "graph", "start", "proof"],
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
        assert_eq!(support::recorded_graphs(home.path()).len(), 3);
    }
}

#[test]
fn failed_placement_is_observed_without_starting_a_task() {
    let task = GraphTask::new();
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let parent =
        loopflow_test_support::RunningParent::insert(&task.home.path().join("loopflow.db"));
    let output = parent
        .adopt(&mut command(
            task.repo.path(),
            task.home.path(),
            &["--task", "missing-task", "graph", "proof"],
        ))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(support::recorded_graphs(task.home.path()).is_empty());
    // A failed launch for another selector writes no ownership entry anywhere.
    let entries: i64 = db
        .query_row(
            "SELECT count(*) FROM task_ownership WHERE task_id!=?1",
            [task.registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(entries, 0);
}

// Release the fixture even when an assertion fails, so no held child escapes.
struct HeldGraph {
    child: Child,
    release: PathBuf,
    caller: PathBuf,
}

impl Drop for HeldGraph {
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

/// The skill step `holding_task`'s stand-in holds until `release`.
const HOLD_STEP: &str = "- step: {name: hold}\n";
const HOLD_SKILL: (&str, &str) = (".lf/skills/hold.md", "Hold.");

/// The fixture with a `hold` skill whose provider holds its step open until
/// the test writes `release` in the home.
fn holding_task() -> GraphTask {
    let task = GraphTask::with_tools(&[("claude", holding_claude().as_str())]);
    loopflow_execution::config::files::update(&task.home.path().join("config.yaml"), |value| {
        value["agent"] = "claude".into();
        Ok(())
    })
    .unwrap();
    task.repo.create_file(HOLD_SKILL.0, HOLD_SKILL.1);
    task
}

#[test]
fn running_graphs_belong_to_the_target_checkout_and_retain_their_caller() {
    for launch in ["task", "direct", "checkout", "alias", "wt"] {
        let task = holding_task();
        // A real sibling checkout, managed through the public placement command.
        task.ok(&["wt", "create", "caller"]);
        // Worktree discovery is read-only and does not select an ambient Home.
        let caller = {
            loopflow_execution::git::worktrees::list_worktrees(task.repo.path())
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
        task.repo.create_file(".lf/graphs/proof.yaml", HOLD_STEP);
        let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
        // The checkout already claimed the Task; inspection and Graphs add no entry.
        let entries = || {
            db.query_row(
                "SELECT count(*) FROM task_ownership WHERE task_id=?1",
                [task.registered.task.id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        let claimed = entries();
        // Explicit placement and passive inspection must not claim the Task again.
        let read = command(
            &caller,
            task.home.path(),
            &["--task", "INF-123", "desktop", "task", "INF-123", "--json"],
        )
        .output()
        .unwrap();
        assert!(
            read.status.success(),
            "{}",
            String::from_utf8_lossy(&read.stderr)
        );
        assert_eq!(entries(), claimed);
        assert!(support::recorded_graphs(task.home.path()).is_empty());

        let parent = loopflow_model::id::LfProcessId::new();
        let trace = loopflow_model::id::TraceId::new();
        let (cwd, args): (&Path, &[&str]) = match launch {
            "task" => (&caller, &["-b", "task", "run", "INF-123", "proof"]),
            "direct" => (&caller, &["-b", "--task", "INF-123", "graph", "proof"]),
            "wt" => (&caller, &["-b", "--wt", "launch-proof", "graph", "proof"]),
            "alias" => (&alias, &["-b", "graph", "proof"]),
            _ => (task.repo.path(), &["-b", "graph", "proof"]),
        };
        let log_path = task.home.path().join("launch.log");
        let log = fs::File::create(&log_path).unwrap();
        let mut held = HeldGraph {
            child: command(cwd, task.home.path(), args)
                .env("LF_PROCESS_ID", parent.as_str())
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
        assert_eq!(entries(), claimed, "{launch}");
        // The Task's background conversation runs the same stand-in, so `ready`
        // can arrive before the Graph's step; wait for the Graph itself.
        let status = loop {
            let status = task.status();
            if !status["work"]["graph_processes"]
                .as_array()
                .unwrap()
                .is_empty()
                || Instant::now() >= deadline
            {
                break status;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let graphs = status["work"]["graph_processes"].as_array().unwrap();
        assert_eq!(graphs.len(), 1, "{launch}: {status}");
        assert_eq!(graphs[0]["state"], "current");
        assert_eq!(graphs[0]["task_id"], task.registered.task.id.as_str());
        let graph_process = graphs[0]["id"].as_str().unwrap();
        let recorded: String = db
            .query_row(
                "SELECT cwd FROM graph_processes WHERE lf_process_id=?1",
                [graph_process],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            Path::new(&recorded),
            task.repo.path().canonicalize().unwrap()
        );
        let inventory = task.run(&["graph", "list", "--runs", "--for-task", "INF-123", "--json"]);
        assert!(
            inventory.status.success(),
            "{}",
            String::from_utf8_lossy(&inventory.stderr)
        );
        let inventory: loopflow_execution::durable::GraphProcessPage =
            serde_json::from_slice(&inventory.stdout).unwrap();
        assert_eq!(inventory.entries.len(), 1);
        assert_eq!(inventory.entries[0].summary.id, graph_process);
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
        println!("{launch}: Started; target status and inventory show current Graph; caller retained; exit 0");
    }
}

#[test]
fn task_run_open_reuses_the_primary_while_the_graph_is_running() {
    task_run_open_while_running(false);
}

#[test]
fn task_run_open_attachment_failure_leaves_the_graph_running() {
    task_run_open_while_running(true);
}

fn task_run_open_while_running(fail_open: bool) {
    let task = holding_task();
    let lf = task.open_fixture();
    if fail_open {
        fs::write(task.home.path().join("open-fails"), "").unwrap();
    }
    task.repo.create_file(".lf/graphs/held.yaml", HOLD_STEP);
    task.repo.create_file(
        ".lf/graphs/held-flow.yaml",
        "- graph: held\n- interactive: {name: demo, id: review}\n",
    );
    let out = task.home.path().join("open.out");
    let log = fs::File::create(&out).unwrap();
    let child = command(
        task.repo.path(),
        task.home.path(),
        &["task", "run", "INF-123", "held-flow", "--open"],
    )
    .env("LF_BIN", &lf)
    .stdin(std::process::Stdio::null())
    .stdout(log.try_clone().unwrap())
    .stderr(log)
    .spawn()
    .unwrap();
    let mut held = HeldGraph {
        child,
        release: task.home.path().join("release"),
        caller: task.home.path().join("unused-caller"),
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = held.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "opening waited for the Graph: {}",
            fs::read_to_string(&out).unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let text = fs::read_to_string(&out).unwrap();
    assert_eq!(status.success(), !fail_open, "{text}");
    let launched = text.find("running as Graph run").unwrap();
    let opened = text
        .find(if fail_open {
            "Conversation exited"
        } else {
            ": Attached"
        })
        .unwrap();
    assert!(launched < opened, "opening preceded launch: {text}");
    assert_eq!(
        fs::read_to_string(task.home.path().join("opened"))
            .unwrap()
            .trim(),
        "opening-primary"
    );
    assert_eq!(
        task.status()["work"]["graph_processes"][0]["state"],
        "current"
    );
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    // Besides the conversation a selected agent starts beside a checked-out
    // Task, opening made none.
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM agent_sessions WHERE task_id=?1 AND interactive=1
             AND skill IS NOT 'task-session'",
            [task.registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    fs::write(&held.release, "").unwrap();
    assert!(task
        .ran(std::process::Output {
            status,
            stdout: text.into_bytes(),
            stderr: Vec::new()
        })
        .success());
}

#[test]
fn task_run_open_waits_for_foreground_runs() {
    for mode in ["-b", "-i"] {
        let task = holding_task();
        let lf = task.open_fixture();
        task.repo.create_file(
            ".lf/graphs/foreground.yaml",
            &format!("{HOLD_STEP}- cmd: task sync --plan\n"),
        );
        let out = task.home.path().join("foreground.out");
        let log = fs::File::create(&out).unwrap();
        let mut held = HeldGraph {
            child: command(
                task.repo.path(),
                task.home.path(),
                &[mode, "task", "run", "INF-123", "foreground", "--open"],
            )
            .env("LF_BIN", &lf)
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap(),
            release: task.home.path().join("release"),
            caller: task.home.path().join("unused-caller"),
        };
        let deadline = Instant::now() + Duration::from_secs(60);
        while !task.home.path().join("ready").exists() {
            assert!(
                held.child.try_wait().unwrap().is_none() && Instant::now() < deadline,
                "{mode}: {}",
                fs::read_to_string(&out).unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(held.child.try_wait().unwrap().is_none());
        assert!(!task.home.path().join("opened").exists());
        fs::write(&held.release, "").unwrap();
        let status = loop {
            if let Some(status) = held.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "{mode}: {}",
                fs::read_to_string(&out).unwrap()
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        let text = fs::read_to_string(&out).unwrap();
        assert!(status.success(), "{mode}: {text}");
        assert!(text.contains(": Attached"), "{mode}: {text}");
        assert_eq!(
            fs::read_to_string(task.home.path().join("opened"))
                .unwrap()
                .trim(),
            "opening-primary"
        );
        let outcomes: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(task.home.path().join("opened-graphs.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(outcomes, serde_json::json!(["succeeded"]), "{mode}");
    }
}

#[test]
fn task_run_open_successful_quote_launches_and_opens_nothing() {
    let task = GraphTask::new();
    let lf = task.open_fixture();
    task.ok(&[
        "ondemand",
        "add",
        "quote",
        "--provider",
        "modal",
        "--vcpus",
        "8",
        "--memory-gb",
        "16",
        "--max",
        "1",
        "--max-usd-per-hour",
        "1",
    ]);
    let before = task.graph();
    let output = command(
        task.repo.path(),
        task.home.path(),
        &[
            "task", "run", "INF-123", "--rent", "quote", "--quote", "--open",
        ],
    )
    .env("LF_BIN", &lf)
    .stdin(std::process::Stdio::null())
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("would rent 8 vCPU 16 GB burst on modal at $0.95184/h ceiling, ≤ $3.80736")
            && text.contains("runs nothing"),
        "{text}"
    );
    assert!(!task.home.path().join("opened").exists());
    assert!(support::recorded_graphs(task.home.path()).is_empty());
    assert_eq!(task.graph(), before);
    assert!(task
        .registered
        .store
        .sqlite
        .rentals(None)
        .unwrap()
        .is_empty());
}

#[test]
fn task_run_open_leaves_failed_launches_and_reads_alone() {
    let task = GraphTask::new();
    let lf = task.open_fixture();
    let opened = task.home.path().join("opened");
    for (args, success) in [
        (
            vec!["task", "run", "INF-123", "--open", "--from", "missing-step"],
            false,
        ),
        // Task runs have no assembled prompt for --context to print.
        (vec!["task", "run", "INF-123", "--open", "--context"], false),
        (vec!["task", "run", "INF-123", "--open", "--explain"], true),
        // A quote without --pool or --rent is refused before launch.
        (vec!["task", "run", "INF-123", "--open", "--quote"], false),
        // A plain waiting run leaves presentation alone too.
        (vec!["task", "run", "INF-123", "--from", "review"], true),
    ] {
        let output = command(task.repo.path(), task.home.path(), &args)
            .env("LF_BIN", &lf)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.success(), success, "{args:?}: {output:?}");
        assert!(!opened.exists(), "{args:?}: {output:?}");
        assert!(support::recorded_graphs(task.home.path()).is_empty());
    }
}

#[test]
fn task_run_open_opens_the_selected_waiting_task() {
    let task = GraphTask::new();
    let lf = task.open_fixture();
    let opened = task.home.path().join("opened");
    // Omitted and root Task selectors retain the same primary, including --repo.
    for args in [
        vec!["task", "run", "--from", "review", "--open"],
        vec!["task", "run", "--from", "review", "--open", "--json"],
        vec!["task", "run", "--from", "review", "--open", "--", "INF-123"],
        vec![
            "--repo",
            task.repo.path().to_str().unwrap(),
            "task",
            "run",
            "--from",
            "review",
            "--open",
        ],
        vec![
            "--task", "INF-123", "task", "run", "--from", "review", "--open",
        ],
    ] {
        let waiting = command(task.repo.path(), task.home.path(), &args)
            .env("LF_BIN", &lf)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(waiting.status.success(), "{args:?}: {waiting:?}");
        if args.contains(&"--json") {
            // Exactly the `task.run` value; the open receipt goes to stderr.
            serde_json::from_slice::<serde_json::Value>(&waiting.stdout)
                .unwrap_or_else(|error| panic!("{error}: {waiting:?}"));
        }
        assert_eq!(
            fs::read_to_string(&opened).unwrap().trim(),
            "opening-primary"
        );
        fs::remove_file(&opened).unwrap();
    }
    assert!(support::recorded_graphs(task.home.path()).is_empty());
}

#[test]
fn task_run_returns_while_its_graph_keeps_running() {
    let task = holding_task();
    task.repo.create_file(
        ".lf/graphs/held.yaml",
        &format!("{HOLD_STEP}- cmd: task sync --plan\n"),
    );
    task.repo.create_file(
        ".lf/graphs/held-flow.yaml",
        "- graph: held\n- interactive: {name: demo, id: review}\n",
    );
    task.select_wave_graph("held-flow");
    let home = task.home.path();
    let (out, returned, release) = (
        home.join("launcher.out"),
        home.join("returned"),
        home.join("release"),
    );
    // A launching shell that keeps living after `lf task run` returns.
    let mut shell = Command::new("/bin/sh");
    for (name, _) in
        std::env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("LF_"))
    {
        shell.env_remove(name);
    }
    std::os::unix::process::CommandExt::process_group(&mut shell, 0);
    let mut shell = shell
        .arg("-c")
        .arg(format!(
            "'{}' task run INF-123 > '{}' 2>&1 < /dev/null; touch '{}'; sleep 600",
            loopflow_test_support::lf_binary(),
            out.display(),
            returned.display()
        ))
        .current_dir(task.repo.path())
        .env("LF_HOME", home)
        .env("LF_BIN", loopflow_test_support::lf_binary())
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("HTTP_PROXY", "http://127.0.0.1:1")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !returned.exists() {
        assert!(
            Instant::now() < deadline,
            "lf task run never returned: {}",
            fs::read_to_string(&out).unwrap_or_default()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let printed = fs::read_to_string(&out).unwrap();
    let graph = printed
        .split("Graph run ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("no Graph run named: {printed}"))
        .to_string();
    assert!(printed.contains(&format!("graph-{graph}.log")), "{printed}");
    assert!(
        printed.contains("follow: lf task status INF-123"),
        "{printed}"
    );
    // Returned while the first step is still held.
    let graphs = task.status()["work"]["graph_processes"].clone();
    assert_eq!(graphs[0]["id"], graph.as_str());
    assert_eq!(graphs[0]["state"], "current");
    // The launching shell and everything in its process group go away.
    let killed = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", shell.id())])
        .status()
        .unwrap();
    assert!(killed.success());
    shell.wait().unwrap();
    fs::write(&release, "").unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let graphs = task.status()["work"]["graph_processes"].clone();
        if graphs[0]["state"] != "current" {
            assert_eq!(graphs[0]["state"], "completed", "{graphs}");
            break;
        }
        assert!(Instant::now() < deadline, "Graph never finished");
        std::thread::sleep(Duration::from_millis(50));
    }
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let steps: i64 = db
        .query_row(
            "SELECT count(*) FROM graph_process_steps WHERE graph_lf_process_id=?1",
            [&graph],
            |row| row.get(0),
        )
        .unwrap();
    assert!(steps >= 2, "the second step never ran: {steps}");
    // The carrier writes the arrival after its Graph exits.
    let deadline = Instant::now() + Duration::from_secs(30);
    while task.graph()["position"] != at("review") {
        assert!(Instant::now() < deadline, "{}", task.graph());
        std::thread::sleep(Duration::from_millis(50));
    }
    let status = task.run(&["task", "status", "INF-123"]);
    let status = String::from_utf8_lossy(&status.stdout);
    let log = home
        .canonicalize()
        .unwrap()
        .join("logs")
        .join(format!("graph-{graph}.log"));
    assert!(
        status.contains(&format!("Graph: {graph}"))
            && status.contains(&format!("log: {}", log.display())),
        "{status}"
    );
    // The step that ran after the launcher was gone wrote to the log.
    let written = fs::read_to_string(&log).unwrap();
    assert!(written.contains("op: sync --plan"), "{written}");
}

#[test]
fn a_refused_bare_task_run_returns_its_refusal() {
    let task = GraphTask::new();
    let output = command(
        task.repo.path(),
        task.home.path(),
        &["task", "run", "missing-task", "proof"],
    )
    .stdin(std::process::Stdio::null())
    .output()
    .unwrap();
    assert!(!refusal(output).is_empty());
    assert!(support::recorded_graphs(task.home.path()).is_empty());
}

/// A Claude stand-in that records each launch's arguments, home and the login
/// that home carries: the stand-in Keychain on macOS, else its file.
const CLAUDE_RECORDER: &str = r#"#!/usr/bin/python3
import glob
import json
import os
import shutil
import sys

if sys.argv[1:] == ['auth', 'status']:
    sys.exit(0)
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
# The launch's login probe is not a launch.
if sys.argv[1:3] == ['auth', 'status']:
    print(json.dumps({'loggedIn': True}))
    sys.exit(0)
security = shutil.which('security')
stores = glob.glob(os.path.join(os.path.dirname(security), 'keychain', '*')) if security else []
claude_home = os.environ.get('CLAUDE_CONFIG_DIR') or os.path.join(os.environ['HOME'], '.claude')
stores.append(os.path.join(claude_home, '.credentials.json'))
login = next((text for text in (open(path).read() for path in stores if os.path.isfile(path)) if 'claudeAiOauth' in text), None)
with open(os.path.join(os.environ['LF_HOME'], 'launches.jsonl'), 'a') as launches:
    launches.write(json.dumps({'args': sys.argv[1:], 'claude_home': claude_home, 'claude_login': login}) + '\n')
if '--input-format' not in sys.argv:
    sys.exit(0)
message = json.loads(sys.stdin.readline())
message['session_id'] = 'agent-fixture'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'agent-fixture'}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'agent-fixture', 'subtype': 'success', 'is_error': False, 'result': 'done'}), flush=True)
for line in sys.stdin:
    pass
"#;

#[test]
fn a_task_run_takes_its_waves_agent_without_flags() {
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("wave-setup");
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(
        &[
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("claude", CLAUDE_RECORDER),
        ],
        Some(home.path()),
    );
    let _server = MachineServer::start_with_path(
        loopflow_test_support::lf_binary(),
        home.path(),
        &std::env::var("PATH").unwrap(),
    );
    let registered = support::register_task_without_pr(
        home.path(),
        &repo.path().canonicalize().unwrap(),
        "wave-setup",
        &repo.head_sha(),
    );

    for (path, content) in [
        (".lf/skills/probe.md", "Probe."),
        (
            ".lf/graphs/proof.yaml",
            "- step: {name: probe}\n- interactive: {name: demo}\n",
        ),
    ] {
        repo.create_file(path, content);
    }
    repo.create_file(
        ".lf/wave/task-pr-tests/config.yaml",
        &format!(
            "id: {}\nlinear:\n  initiative: initiative-task-pr-tests\nagent: claude:opus\n",
            registered.task.wave_id.as_ref().unwrap()
        ),
    );
    repo.stage_all();
    repo.commit("Set up the Wave once");
    {
        let args = &["task", "run", "INF-123", "proof"];
        let output = command(repo.path(), home.path(), args)
            .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    wait_for("the Task conversation and Graph launches", || {
        fs::read_to_string(home.path().join("launches.jsonl"))
            .is_ok_and(|launches| launches.lines().count() == 2)
    });
    let launches = fs::read_to_string(home.path().join("launches.jsonl")).unwrap();
    let launches: Vec<serde_json::Value> = launches
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect();
    assert_eq!(
        launches.len(),
        2,
        "the Task conversation and Graph both launch"
    );
    // Task workers and the managed conversation both report turn boundaries.
    for launch in launches {
        let args = launch["args"].as_array().unwrap();
        let settings = args
            .iter()
            .position(|arg| arg == "--settings")
            .map(|flag| fs::read_to_string(args[flag + 1].as_str().unwrap()).unwrap())
            .unwrap_or_else(|| panic!("{launch}"));
        assert!(settings.contains("\"Stop\""), "{settings}");
        assert!(
            args.windows(2).any(|pair| pair == ["--model", "opus"]),
            "{launch}"
        );
        // The native home is inherited, without installing a saved login.
        assert_eq!(
            launch["claude_home"].as_str().map(Path::new),
            Some(home.path().join(".claude").as_path())
        );
        assert!(launch["claude_login"].is_null());
    }
}

/// A registered Task whose repository defines the Graphs below.
struct GraphTask {
    /// Agent steps start only once the Task's Machine server answers.
    server: MachineServer,
    repo: TestRepo,
    home: tempfile::TempDir,
    registered: support::RegisteredTask,
    _env: support::EnvGuard,
}

/// How one `lf task run` ended once its run stopped: a bare run returns as its
/// Graph process registers and keeps going in that Graph's log.
struct Ran {
    code: Option<i32>,
    /// Everything it printed, its log included.
    text: String,
}

impl Ran {
    fn success(&self) -> bool {
        self.code == Some(0)
    }
}

impl GraphTask {
    /// A retained primary and a stand-in for terminal attachment, not Graph execution.
    fn open_fixture(&self) -> PathBuf {
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        db.execute(
            "INSERT INTO agent_sessions(
                id,title,title_source,created_at,input_published,interactive,task_id,wave_id,cwd,provider
             ) VALUES('opening-primary','Task conversation','generated',unixepoch(),1,1,?1,?2,?3,'claude')",
            rusqlite::params![
                self.registered.task.id.as_str(),
                self.registered.task.wave_id.as_ref().unwrap().as_str(),
                self.repo.path().to_str().unwrap(),
            ],
        )
        .unwrap();
        db.execute(
            "INSERT OR REPLACE INTO task_primary_sessions(task_id,session_id) VALUES(?1,'opening-primary')",
            [self.registered.task.id.as_str()],
        )
        .unwrap();
        db.execute(
            "UPDATE agent_sessions SET task_id=?1,wave_id=(SELECT p.wave_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1) WHERE id='opening-primary'",
            [self.registered.task.id.as_str()],
        )
        .unwrap();
        let lf = self.home.path().join("lf-open-fixture");
        fs::write(
            &lf,
            format!(
                r#"#!/usr/bin/env python3
import json, os, sqlite3, subprocess, sys
from pathlib import Path
if sys.argv[1:3] == ['session', 'resume']:
    home = Path(os.environ['LF_HOME'])
    # The Graph's background Task conversation resumes the same Session; only
    # lf open's resume is the opening under test.
    parent = subprocess.run(['ps', '-o', 'command=', '-p', str(os.getppid())], capture_output=True, text=True).stdout.split()
    if 'open' not in parent[1:]:
        sys.exit(0)
    # Capture completion at attachment, not after the caller exits.
    with sqlite3.connect(home / 'loopflow.db') as db:
        outcomes = [row[0] for row in db.execute("SELECT CASE WHEN ended_at IS NULL THEN NULL WHEN exit_code=0 THEN 'succeeded' ELSE 'failed' END FROM graph_processes")]
    (home / 'opened-graphs.json').write_text(json.dumps(outcomes))
    with (home / 'opened').open('a') as opened:
        opened.write(sys.argv[3] + '\n')
    sys.exit(9 if (home / 'open-fails').exists() else 0)
os.execv({binary}, [{binary}, *sys.argv[1:]])
"#,
                binary = serde_json::to_string(&loopflow_test_support::lf_binary()).unwrap(),
            ),
        )
        .unwrap();
        fs::set_permissions(&lf, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        lf
    }

    /// The fixture, its Wave selecting `feature`.
    fn new() -> Self {
        let task = Self::without_wave_graph(&[]);
        task.select_wave_graph("feature");
        task
    }

    /// The fixture with these executables on PATH as well.
    fn with_tools(tools: &[(&str, &str)]) -> Self {
        let task = Self::without_wave_graph(tools);
        task.select_wave_graph("feature");
        task
    }

    fn without_wave_graph(tools: &[(&str, &str)]) -> Self {
        let repo = TestRepo::new();
        support::bind_task_planning(&repo);
        let home = tempfile::tempdir().unwrap();
        let mut entries = vec![
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
        ];
        entries.extend_from_slice(tools);
        let env = support::EnvGuard::new(&entries);
        for (path, content) in [
            (".lf/graphs/proof.yaml", "- cmd: task sync --plan\n"),
            (".lf/graphs/land-proof.yaml", "- cmd: task sync --plan\n"),
            // Fails until the repository defines a Graph named `late`.
            (".lf/graphs/gate.yaml", "- cmd: graph show late\n"),
            // Accepting fails until `late` exists.
            (
                ".lf/graphs/gated.yaml",
                "- graph: proof\n- interactive: {name: demo, id: review}\n- graph: gate\n- interactive: {name: demo, id: accepted}\n",
            ),
            // A step that is done, then one that a limit can hold.
            (
                ".lf/graphs/held.yaml",
                "- cmd: task sync --plan\n- cmd: graph show late\n",
            ),
            // The same, with a stop for a person: running it turns no auto on.
            (
                ".lf/graphs/held-review.yaml",
                "- cmd: task sync --plan\n- cmd: graph show late\n- interactive: {name: demo, id: review}\n",
            ),
            // The Wave's default, `feature`, as this repository defines it.
            (
                ".lf/graphs/feature.yaml",
                "- graph: proof\n- interactive: {name: demo, id: review}\n",
            ),
            // No PR: nothing runs after the findings.
            (
                ".lf/graphs/findings.yaml",
                "- graph: proof\n- interactive: {name: research, id: findings}\n",
            ),
            // Merged work is checked at a last interactive step before `end`.
            (
                ".lf/graphs/verified.yaml",
                "- graph: proof\n- interactive: {name: demo, id: review}\n- graph: land-proof\n- interactive: {name: research, id: check}\n",
            ),
        ] {
            let path = repo.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        // The Task's branch starts from these definitions and holds nothing.
        repo.stage_all();
        repo.commit("Define the fixture Graphs");
        repo.create_branch("launch-proof");
        let registered = support::register_task_without_pr(
            home.path(),
            &repo.path().canonicalize().unwrap(),
            "launch-proof",
            &repo.head_sha(),
        );
        let fixture = Self {
            // The server restarts what died, with this fixture's tools.
            server: MachineServer::start_with_path(
                loopflow_test_support::lf_binary(),
                home.path(),
                &std::env::var("PATH").unwrap(),
            ),
            repo,
            home,
            registered,
            _env: env,
        };
        fixture.ok(&["task", "checkout", "INF-123"]);
        fixture
    }

    fn select_wave_graph(&self, name: &str) {
        self.set_wave_setting("graph", name);
    }

    fn set_wave_setting(&self, key: &str, value: &str) {
        let store =
            loopflow_execution::store::open_sqlite(&self.home.path().join("loopflow.db")).unwrap();
        support::set_wave_setting(
            self.repo.path(),
            &store,
            self.registered.task.wave_id.as_ref().unwrap(),
            key,
            value,
        );
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
        let status = self.run(&["desktop", "task", "INF-123", "--json"]);
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        status["execution"].clone()
    }

    /// Planning status, independent of where the Task stands on its Graph.
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
        deliver_state_writes(store, &self.registered.task.id);
        let mut record: crate::test_linear_input::IssueFixture =
            loopflow_test_support::retained_task_input(
                &self.registered.database,
                &scope,
                "INF-123",
            );
        record.item.revision = Some("2026-10-06T12:00:00Z".into());
        record.item.state = Some("completed".into());
        record.item.completed = true;
        runtime
            .block_on(store.seed_issue(&scope, "linear", record, None))
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

    /// Run `lf task run …` and wait for the run it started to stop.
    fn task_run(&self, args: &[&str]) -> Ran {
        self.ran(self.run(args))
    }

    /// `lf task run …` that must succeed, waited.
    fn task_run_ok(&self, args: &[&str]) {
        let ran = self.task_run(args);
        assert!(
            ran.success(),
            "{args:?} exited {:?}: {}",
            ran.code,
            ran.text
        );
    }

    /// Wait for the run a returned `lf task run` printed, if it printed one:
    /// until the Task's records show nothing running and its carrier has
    /// exited. Its code is its last Graph run's: a carrier's result is its Graph's.
    fn ran(&self, output: std::process::Output) -> Ran {
        let mut text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let printed = |label: &str| {
            text.split(label)
                .nth(1)
                .and_then(|rest| rest.split_whitespace().next())
                .map(str::to_owned)
        };
        let (Some(graph), Some(log)) = (printed("running as Graph run "), printed("log:    "))
        else {
            return Ran {
                code: output.status.code(),
                text,
            };
        };
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        let task: String = db
            .query_row(
                "SELECT task_id FROM graph_processes WHERE lf_process_id=?1",
                [&graph],
                |row| row.get(0),
            )
            .unwrap();
        // The carrier's pid while its records show it, then the OS until it exits.
        let mut carrier: Option<u32> = None;
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let (pid, graphs): (Option<u32>, bool) = db
                .query_row(
                    "SELECT (SELECT run_pid FROM task_graphs WHERE task_id=?1),
                        EXISTS(SELECT 1 FROM graph_processes WHERE task_id=?1 AND ended_at IS NULL)",
                    [&task],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            carrier = carrier.or(pid);
            // SAFETY: signal 0 only asks whether the pid exists.
            let carrier_runs = carrier.is_some_and(|pid| unsafe { libc::kill(pid as i32, 0) } == 0);
            if pid.is_none() && !graphs && !carrier_runs {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "the run never stopped: {text}\n{}",
                fs::read_to_string(&log).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        let code: Option<i32> = db
            .query_row(
                "SELECT exit_code FROM graph_processes WHERE task_id=?1
                 ORDER BY started_at DESC,rowid DESC LIMIT 1",
                [&task],
                |row| row.get(0),
            )
            .unwrap();
        text.push_str(&fs::read_to_string(&log).unwrap_or_default());
        Ran { code, text }
    }

    fn finish_after_provider_observation(&self, args: &[&str]) {
        self.ok(args);
        assert_eq!(self.state(), "done");
        assert!(self.status()["completion_pending"].is_null());
        let graph = self.graph();
        self.complete_in_linear();
        self.ok(&["task", "complete", "INF-123"]);
        assert_eq!(self.graph(), graph);
        assert_eq!(self.state(), "done");
    }

    fn graph(&self) -> serde_json::Value {
        self.status()["work"]["graph"].clone()
    }

    fn text_status(&self) -> String {
        String::from_utf8_lossy(&self.run(&["task", "status", "INF-123"]).stdout).to_string()
    }
}

fn at(step: &str) -> serde_json::Value {
    serde_json::json!({"kind": "at", "step": step})
}

fn refusal(output: std::process::Output) -> String {
    assert!(!output.status.success());
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// An `lf` that runs the real one, except that `graph show late` runs `script`.
fn lf_shim(dir: &Path, script: &str) -> PathBuf {
    let lf = dir.join("lf");
    fs::write(
        &lf,
        format!(
            "#!/bin/sh\ncase \"$*\" in *'graph show late'*)\n{script}\nesac\nexec '{}' \"$@\"\n",
            loopflow_test_support::lf_binary()
        ),
    )
    .unwrap();
    fs::set_permissions(&lf, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    lf
}

#[test]
fn checkout_leaves_graph_selection_to_the_first_run() {
    let fixture = GraphTask::new();
    fixture.repo.push_new_branch("main");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = &fixture.registered.store;
    let scope = fixture
        .repo
        .path()
        .canonicalize()
        .unwrap()
        .display()
        .to_string();
    let sqlite =
        loopflow_execution::store::open_sqlite(&fixture.home.path().join("loopflow.db")).unwrap();
    let mut observation: crate::test_linear_input::IssueFixture =
        loopflow_test_support::retained_task_input(
            &fixture.registered.database,
            &scope,
            &fixture.registered.task.identifier,
        );
    observation.item.id = "unplaced-issue".into();
    observation.item.identifier = "INF-124".into();
    observation.item.name = "Graph chosen at first run".into();
    observation.item.branch_name = None;
    runtime
        .block_on(store.seed_issue(&scope, "linear", observation, None))
        .unwrap();
    let task = runtime
        .block_on(store.get_task_by_issue("INF-124"))
        .unwrap()
        .unwrap();
    let select = |name: &str| {
        fs::write(
            fixture.repo.path().join(format!(".lf/graphs/{name}.yaml")),
            "- graph: proof\n- interactive: {name: demo, id: review}\n",
        )
        .unwrap();
        support::select_wave_graph(
            fixture.repo.path(),
            &sqlite,
            task.wave_id.as_ref().unwrap(),
            name,
        );
    };
    select("before-checkout");
    fixture.ok(&["task", "checkout", "INF-124", "--name", "first-graph"]);
    assert!(
        loopflow_execution::store::task_work::task_work(&sqlite, &task.id)
            .unwrap()
            .graph
            .is_none()
    );
    select("at-first-run");
    fixture.task_run_ok(&["task", "run", "INF-124"]);
    let captured = loopflow_execution::store::task_work::task_work(&sqlite, &task.id)
        .unwrap()
        .graph
        .unwrap();
    assert_eq!(captured.name, "at-first-run");
    assert_eq!(
        serde_json::to_value(&captured.position).unwrap(),
        at("review")
    );
    select("after-first-run");
    fixture.task_run_ok(&["task", "run", "INF-124"]);
    let retained = loopflow_execution::store::task_work::task_work(&sqlite, &task.id)
        .unwrap()
        .graph
        .unwrap();
    assert_eq!(retained.name, captured.name);
    assert_eq!(serde_json::to_value(&retained.position).unwrap(), at("end"));
}

#[test]
fn a_task_takes_up_its_waves_graph_and_keeps_the_one_it_named() {
    // Nothing named, no graph yet: the Wave default, stopping at its review.
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run"]);
    let graph = task.graph();
    assert_eq!(graph["name"], "feature");
    assert_eq!(graph["position"], at("review"));
    assert_eq!(graph["interactive_steps"][0]["skill"], "demo");
    let output = task.run(&["task", "status", "INF-123", "--json"]);
    assert!(output.status.success(), "{output:?}");
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut fields: Vec<_> = status
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "auto",
            "children",
            "comments",
            "conversation",
            "graph",
            "graph_processes",
            "parent",
            "reading",
            "sessions",
            "stack_parent",
            "summary",
            "task"
        ]
    );
    assert_eq!(status["graph"], graph);
    assert_eq!(
        status["task"]["graph"],
        serde_json::json!({"name": "feature", "step": "review"})
    );
    assert_eq!(status["task"]["next"], "lf task run INF-123");
    drop(task);

    // A Task that named its own keeps it when later runs name nothing.
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "findings"]);
    task.task_run_ok(&["task", "run", "INF-123"]);
    let graph = task.graph();
    assert_eq!(graph["name"], "findings");
    assert_eq!(graph["position"], at("end"));
}

#[test]
fn task_create_run_starts_the_graph_and_admits_the_request_as_first_turn() {
    let task = GraphTask::without_wave_graph(&[]);
    task.ok(&["set", "--global", "default_agent", "claude"]);
    // New Tasks branch from the remote's main, which carries the fixture Graphs.
    assert!(Command::new("git")
        .args(["push", "origin", "main"])
        .current_dir(task.repo.path())
        .output()
        .unwrap()
        .status
        .success());
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let tasks = || -> i64 {
        db.query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
            .unwrap()
    };
    // An unknown graph is refused before any Task exists.
    let before = tasks();
    let refused = task.run(&["task", "create", "never filed", "--run", "--graph", "nope"]);
    assert!(!refused.status.success());
    assert_eq!(tasks(), before);

    let request = "fix the flaky retry test\n\nIt fails one run in ten.";
    let output = task.run(&[
        "task", "create", request, "--run", "--graph", "feature", "--json",
    ]);
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    let ran = task.ran(output);
    assert!(ran.success(), "{}", ran.text);
    let issue = receipt["task"]["identifier"].as_str().unwrap();
    assert_eq!(receipt["task"]["title"], "fix the flaky retry test");
    assert_eq!(receipt["task"]["description"], request);
    assert_eq!(receipt["graph"], "feature");
    // Creation returns when the Graph registers, before its first step finishes.
    wait_for("the created Task to reach review", || {
        let status = task.run(&["desktop", "task", issue, "--json"]);
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        status["execution"]["work"]["graph"]["position"] == at("review")
    });
    let status = task.run(&["desktop", "task", issue, "--json"]);
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let graph = &status["execution"]["work"]["graph"];
    assert_eq!(graph["name"], "feature");
    assert_eq!(graph["position"], at("review"));
    // The admitted primary conversation opens with the request, word for word.
    let session = receipt["session_id"].as_str().unwrap();
    let first_turn: String = db
        .query_row(
            "SELECT request FROM agent_sessions WHERE id=?1",
            [session],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(first_turn, request);
    let primary: String = db
        .query_row(
            "SELECT session_id FROM task_primary_sessions WHERE task_id=?1",
            [receipt["task"]["id"].as_str().unwrap()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(primary, session);

    // `-b` runs to the end headless and returns no conversation to open.
    let output = task.run(&[
        "-b", "task", "create", "headless", "--run", "--graph", "proof", "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(receipt["session_id"].is_null(), "{receipt}");
    assert_eq!(receipt["graph"], "proof");

    // A run that fails still reports the filed Task, so nothing files it twice.
    let before = tasks();
    let output = task.run(&[
        "-b", "task", "create", "fails", "--run", "--graph", "gate", "--json",
    ]);
    assert!(!output.status.success());
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    assert_eq!(receipt["task"]["title"], "fails");
    assert_eq!(receipt["graph"], "gate", "{receipt}");
    assert_eq!(receipt["milestone"], "started", "{receipt}");
    assert!(receipt["session_id"].is_null(), "{receipt}");
    assert_eq!(tasks(), before + 1);
}

#[test]
fn nothing_after_the_last_interactive_step_reaches_end_without_a_pr() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "findings"]);
    let graph = task.graph();
    assert_eq!(graph["name"], "findings");
    assert_eq!(graph["interactive_steps"][0]["skill"], "research");
    assert_eq!(graph["position"], at("findings"));
    assert_eq!(task.state(), "active");
    let status = task.text_status();
    assert!(
        status.contains("Graph findings: Waiting for you at findings."),
        "{status}"
    );
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    task.repo
        .create_file("findings.md", "Accepted recommendation\n");
    task.repo.stage_all();
    task.repo.commit("Record research findings");
    task.repo
        .create_file("draft.md", "Retained working notes\n");
    task.finish_after_provider_observation(&["task", "run", "INF-123"]);
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(
        fs::read_to_string(task.repo.path().join("draft.md")).unwrap(),
        "Retained working notes\n"
    );
    assert!(task.repo.path().join("findings.md").exists());
    // Nothing remained to run.
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    // Reaching the end completes research without creating a PR.
    assert_eq!(task.state(), "done");
    assert!(task.status()["pr"].is_null());
    let error = refusal(task.run(&["task", "run", "INF-123"]));
    assert!(error.contains("is done"), "{error}");
    assert!(task.text_status().contains("Graph findings: At its end."));
}

#[test]
fn a_run_continues_after_the_step_it_stopped_at_and_from_goes_back_or_skips() {
    let task = GraphTask::new();
    fs::write(
        task.repo.path().join(".lf/graphs/late.yaml"),
        "- cmd: task sync --plan\n",
    )
    .unwrap();
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    assert_eq!(task.graph()["position"], at("review"));
    // The next run continues after review and stops before accepted.
    task.task_run_ok(&["task", "run", "INF-123"]);
    assert_eq!(task.graph()["position"], at("accepted"));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
    // The demo was not good enough after all: go back to review. Nothing runs.
    let waits = task.task_run(&["task", "run", "INF-123", "--from", "review"]);
    assert!(waits.success(), "{}", waits.text);
    assert!(waits.text.contains("waits at review"), "{}", waits.text);
    assert_eq!(task.graph()["position"], at("review"));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
    // With `--json` it prints what `task.run` returns to a client.
    let waits = task.run(&["task", "run", "INF-123", "--from", "review", "--json"]);
    assert!(waits.status.success(), "{}", refusal(waits.clone()));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&waits.stdout).unwrap(),
        serde_json::json!({"state": "waiting", "issue": "INF-123", "step": "review"})
    );
    // From a step that runs, the run starts there: proof again, to review.
    task.task_run_ok(&["task", "run", "INF-123", "--from", "proof"]);
    assert_eq!(task.graph()["position"], at("review"));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 3);
    // Skip ahead to accepted.
    task.task_run_ok(&["task", "run", "INF-123", "--from", "accepted"]);
    assert_eq!(task.graph()["position"], at("accepted"));
    let error = refusal(task.run(&["task", "run", "INF-123", "--from", "nowhere"]));
    assert!(
        error.contains("graph gated has no step \"nowhere\""),
        "{error}"
    );
    assert!(error.contains("review"), "{error}");
    // Taking up another graph starts it over.
    task.task_run_ok(&["task", "run", "INF-123", "findings"]);
    let graph = task.graph();
    assert_eq!(graph["name"], "findings");
    assert_eq!(graph["position"], at("findings"));
}

#[test]
fn lf_graph_in_the_worktree_does_not_move_the_task() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    let before = task.graph();
    task.ok(&["-b", "graph", "land-proof"]);
    task.ok(&["-b", "--task", "INF-123", "graph", "land-proof"]);
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 3);
    assert_eq!(task.graph(), before);
}

/// The last Graph run recorded, as a mark for [`attempts`].
fn last_graph(home: &Path) -> i64 {
    rusqlite::Connection::open(home.join("loopflow.db"))
        .unwrap()
        .query_row(
            "SELECT COALESCE(MAX(rowid),0) FROM graph_processes",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

/// The Graph runs recorded after `mark`, by how they ended, oldest first.
fn attempts(home: &Path, mark: i64) -> Vec<String> {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let mut rows = db
        .prepare(
            "SELECT CASE WHEN ended_at IS NULL THEN 'running' WHEN exit_code=0 THEN 'succeeded'
                 WHEN exit_code=130 AND error IS NULL THEN 'interrupted' ELSE 'failed' END
             FROM graph_processes WHERE rowid>?1 ORDER BY rowid",
        )
        .unwrap();
    let outcomes = rows
        .query_map([mark], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    outcomes
}

#[test]
fn a_run_that_ended_waiting_on_a_limit_starts_again_at_that_step_without_replaying_the_done_ones() {
    let mut task = GraphTask::new();
    let failed = task.task_run(&["task", "run", "INF-123", "held"]);
    assert!(!failed.success(), "{}", failed.text);
    drop(task.server);
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let steps_of_latest_graph = || -> Vec<String> {
        db.prepare(
            "SELECT (SELECT json_extract(n.value,'$.label') FROM json_each(g.compiled,'$.steps') n
                     WHERE json_extract(n.value,'$.key')=s.node)
             FROM graph_process_steps s JOIN graph_processes g ON g.lf_process_id=s.graph_lf_process_id
             WHERE g.rowid=(SELECT MAX(rowid) FROM graph_processes) ORDER BY s.seq",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    };
    // The last attempt started at the failed step.
    let steps = steps_of_latest_graph();
    assert_eq!(steps.len(), 1, "{steps:?}");
    // The step ended waiting on a limit; the wait is over.
    let waiting: String = db
        .query_row(
            "SELECT lf_process_id FROM graph_process_steps WHERE graph_lf_process_id=
                (SELECT lf_process_id FROM graph_processes ORDER BY rowid DESC LIMIT 1)
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    db.execute(
        "INSERT INTO agent_limit_recoveries(lf_process_id,provider,reason,retry_at,updated_at)
         VALUES(?1,'claude','rejected: five_hour',1,1)",
        [&waiting],
    )
    .unwrap();
    // Record the server's restart without running it, so the explicit run
    // below is the only Graph that starts.
    let bin = tempfile::tempdir().unwrap();
    let (lf, _) = restart_recorder(bin.path());
    task.server = MachineServer::start_with_path(
        lf.to_str().unwrap(),
        task.home.path(),
        &std::env::var("PATH").unwrap(),
    );
    let read = command(
        task.repo.path(),
        task.home.path(),
        &["task", "status", "INF-123"],
    )
    .output()
    .unwrap();
    assert!(
        String::from_utf8_lossy(&read.stdout).contains("ended while waiting for claude capacity")
    );

    let again = task.task_run(&["task", "run", "INF-123"]);
    assert!(!again.success(), "{}", again.text);
    let steps = steps_of_latest_graph();
    assert_eq!(steps.len(), 1, "{steps:?}");
    assert!(steps[0].contains("late"), "{steps:?}");
}

/// An `lf` that records the restart the server would start, with the
/// agent settings it saw, in place of running it. Returns it and its record.
fn restart_recorder(bin: &Path) -> (PathBuf, PathBuf) {
    let restarts = bin.join("restarts");
    let lf = bin.join("lf");
    fs::write(
        &lf,
        format!(
            "#!/bin/sh\ncase \"$*\" in *'task run INF-123')\n  printf '%s\\n' \"$*|$LF_AGENT_OVERRIDE|$LF_SETTINGS\" > '{}'.$$; mv '{}'.$$ '{}'; exit 0;;\nesac\nexec '{}' \"$@\"\n",
            restarts.display(),
            restarts.display(),
            restarts.display(),
            loopflow_test_support::lf_binary()
        ),
    )
    .unwrap();
    fs::set_permissions(&lf, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    (lf, restarts)
}

/// What the server starts for a run of `graph` that ended waiting on a
/// limit with `recorded` as its explicit agent, when that reader exported its
/// own choice: the restart's arguments and the agent settings it saw.
fn restart_of_a_finished_wait(graph: &str, recorded: Option<&str>) -> String {
    let task = GraphTask::new();
    let failed = task.task_run(&["task", "run", "INF-123", graph]);
    assert!(!failed.success(), "{}", failed.text);
    drop(task.server);
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let waiting: String = db
        .query_row(
            "SELECT lf_process_id FROM graph_process_steps WHERE graph_lf_process_id=
                (SELECT lf_process_id FROM graph_processes ORDER BY rowid DESC LIMIT 1)
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    db.execute(
        "INSERT INTO agent_limit_recoveries(lf_process_id,provider,reason,retry_at,agent,updated_at)
         VALUES(?1,'claude','rejected: five_hour',1,?2,1)",
        rusqlite::params![waiting, recorded],
    )
    .unwrap();

    let bin = tempfile::tempdir().unwrap();
    let (lf, restarts) = restart_recorder(bin.path());
    let read = command(
        task.repo.path(),
        task.home.path(),
        &["task", "status", "INF-123"],
    )
    .env("LF_BIN", &lf)
    .env("LF_AGENT_OVERRIDE", "codex:reader")
    .env("LF_SETTINGS", "agent: codex:reader")
    .output()
    .unwrap();
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    assert!(!restarts.exists());
    let _server = MachineServer::start_with_path(
        lf.to_str().unwrap(),
        task.home.path(),
        &std::env::var("PATH").unwrap(),
    );
    wait_for("the restart", || restarts.exists());
    fs::read_to_string(restarts).unwrap().trim().to_string()
}

#[test]
fn a_restarted_wait_keeps_its_explicit_agent() {
    assert_eq!(
        restart_of_a_finished_wait("held-review", Some("claude:opus")),
        "--agent claude:opus task run INF-123||{}"
    );
}

#[test]
fn a_restarted_wait_with_no_explicit_agent_ignores_the_readers() {
    assert_eq!(
        restart_of_a_finished_wait("held-review", None),
        "task run INF-123||{}"
    );
}

/// With no stop for a person ahead the run may merge; it is carried again
/// as `lf task run` all the same.
#[test]
fn an_auto_tasks_restarted_wait_keeps_its_explicit_agent() {
    assert_eq!(
        restart_of_a_finished_wait("held", Some("claude:opus")),
        "--agent claude:opus task run INF-123||{}"
    );
}

/// November 12: a step refused by a usage limit, with no agent to move to, was
/// relaunched three times into the same refusal.
#[test]
fn a_run_stopped_by_a_usage_limit_is_not_started_again() {
    let task = GraphTask::new();
    // The failing step ends having recorded that a limit left it nowhere to go.
    let reason = format!(
        "codex_error: You’ve hit your usage limit{}there is no `agents` list",
        loopflow_execution::ops::task_execution::STOPPED
    );
    let lf = task.home.path().join("lf-limit-fixture");
    fs::write(
        &lf,
        format!(
            r#"#!/usr/bin/env python3
import os, sqlite3, subprocess, sys
binary = {binary}
if sys.argv[-3:] != ['graph', 'show', 'late']:
    os.execv(binary, [binary, *sys.argv[1:]])
code = subprocess.call([binary, *sys.argv[1:]])
db = sqlite3.connect(os.path.join(os.environ['LF_HOME'], 'loopflow.db'), timeout=30)
with db:
    db.execute(
        "INSERT INTO agent_limit_recoveries(lf_process_id,provider,reason,updated_at) "
        "SELECT s.lf_process_id,'codex',?,1 FROM graph_process_steps s WHERE s.graph_lf_process_id="
        "(SELECT lf_process_id FROM graph_processes ORDER BY rowid DESC LIMIT 1) "
        "ORDER BY s.seq DESC LIMIT 1",
        [{reason}],
    )
sys.exit(code)
"#,
            binary = serde_json::to_string(&loopflow_test_support::lf_binary()).unwrap(),
            reason = serde_json::to_string(&reason).unwrap(),
        ),
    )
    .unwrap();
    fs::set_permissions(&lf, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let output = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "task", "run", "INF-123", "held-review"],
    )
    .env("LF_BIN", &lf)
    .output()
    .unwrap();
    let ran = task.ran(output);
    assert!(!ran.success(), "{}", ran.text);
    assert!(
        ran.text.contains(
            "stopped after a provider refusal at graph show late: codex_error: You’ve hit your usage limit. there is no `agents` list; `lf task run INF-123` starts the run again at it"
        ),
        "{}",
        ran.text
    );
    assert!(!ran.text.contains("starting it again"), "{}", ran.text);
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    // `-b` writes no auto; nothing starts the stopped run again.
    for _ in 0..2 {
        let status = task.text_status();
        assert!(!status.contains("auto:"), "{status}");
        assert!(
            status.contains("stopped after a provider refusal"),
            "{status}"
        );
        assert!(status.contains("there is no `agents` list"), "{status}");
        assert!(
            status.contains("`lf task run INF-123` starts the run again at it"),
            "{status}"
        );
    }
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);

    // Once capacity is available, a manual run picks up the refused step; with
    // auto off it waits at review for a person.
    task.ok(&["task", "auto", "off", "INF-123"]);
    fs::write(
        task.repo.path().join(".lf/graphs/late.yaml"),
        "- cmd: task sync --plan\n",
    )
    .unwrap();
    task.task_run_ok(&["task", "run", "INF-123"]);
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 2);
    assert_eq!(graphs[0].1.len(), 2);
    assert_eq!(graphs[1].1.len(), 1);
    assert_eq!(graphs[1].1[0]["label"], "graph show late");
    assert_eq!(task.graph()["position"], at("review"));
}

/// `worker`: one `implement` step, then a person's review.
fn define_worker_graph(task: &GraphTask) {
    task.repo.create_file(
        ".lf/graphs/worker.yaml",
        "- implement\n- interactive: {name: demo, id: review}\n",
    );
    task.repo.stage_all();
    task.repo.commit("Define a worker followed by a review");
}

fn opencode_stand_in() -> String {
    include_str!("../../../loopflow-execution/tests/support/opencode_cli.py")
        .replace("__WAIT__", "False")
        .replace(
            "Path(__file__).resolve().parent.parent",
            "Path(os.environ['LF_HOME'])",
        )
}

fn antigravity_stand_in() -> String {
    include_str!("../../../loopflow-execution/tests/support/agy_cli.py").replace(
        "Path(__file__).resolve().parent.parent",
        "Path(os.environ['LF_HOME'])",
    )
}

fn pi_stand_in() -> String {
    format!(
        "#!/usr/bin/python3 -I\n{}",
        include_str!("../../../loopflow-execution/src/harness/testdata/pi_rpc.py").replace(
            "Path.cwd() / \"pi-conversation.jsonl\"",
            "Path(os.environ['LF_HOME']) / (os.environ['LF_CAPTURE_KEY'] + '.jsonl')"
        )
    )
}

/// One `implement` step on each stand-in: it settles and the Graph moves on,
/// with its answer, native thread and usage recorded. While its one tool
/// runs, `lf steer` saves a steer; the hooks lf gave the harness return the
/// step's context before the tool and the steer after it.
#[test]
fn a_task_step_runs_as_a_worker_on_every_harness() {
    let opencode = opencode_stand_in();
    let pi = pi_stand_in();
    for (provider, agent, script, answer) in [
        (
            "opencode",
            "opencode:opencode/fixture",
            opencode,
            "Fixture completed.",
        ),
        ("pi", "pi:fixture/model", pi, "answer 1:"),
        ("agy", "agy", antigravity_stand_in(), "answer to:"),
        ("claude", "claude", holding_claude(), "Held."),
        (
            "codex",
            "codex",
            support::codex_app_server_script("Worker finished.", ""),
            "Worker finished.",
        ),
    ] {
        let task = GraphTask::with_tools(&[(provider, &script)]);
        define_worker_graph(&task);
        let home = task.home.path();
        fs::write(home.join("release"), "").unwrap();
        // Each stand-in runs the hooks lf gave its harness around one tool.
        fs::write(
            home.join("tool_turn.py"),
            include_str!("../../../loopflow-execution/tests/support/tool_turn.py"),
        )
        .unwrap();

        let started = task.run(&["task", "run", "INF-123", "worker", "-a", agent]);
        wait_for("the worker's tool", || home.join("holding").exists());
        task.ok(&["steer", "INF-123", "WORKER_DIRECTION"]);
        fs::write(home.join("tool-done"), "").unwrap();
        let ran = task.ran(started);
        assert!(ran.success(), "{provider}: {}", ran.text);
        assert_eq!(task.graph()["position"], at("review"), "{provider}");
        let graphs = support::recorded_graphs(task.home.path());
        assert_eq!(graphs.len(), 1, "{provider}");
        assert_eq!(graphs[0].1.len(), 1, "{provider}");

        let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
        let (session, thread): (String, String) = db
            .query_row(
                "SELECT id, provider_thread FROM agent_sessions
                 WHERE task_id=?1 AND provider=?2 AND skill='implement'
                   AND interactive=0 AND completed_at IS NOT NULL",
                rusqlite::params![task.registered.task.id.as_str(), provider],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(!thread.is_empty(), "{provider}");
        let history = task.run(&["history", "show", &session, "--final"]);
        assert!(
            history.status.success(),
            "{provider}: {}",
            String::from_utf8_lossy(&history.stderr)
        );
        assert!(
            String::from_utf8_lossy(&history.stdout).contains(answer),
            "{provider}: {history:?}"
        );

        // The start hook named this worker's Session and Task, and the hook
        // after the tool brought the steer saved while the tool ran.
        let context = fs::read_to_string(home.join("hook-context")).unwrap();
        assert!(
            context.contains(&format!("This Session: {session}\n")),
            "{provider}: {context}"
        );
        assert!(
            context.contains("This Session's Task: INF-123"),
            "{provider}: {context}"
        );
        let steer = fs::read_to_string(home.join("hook-steer")).unwrap();
        assert!(steer.contains("WORKER_DIRECTION"), "{provider}: {steer}");

        let runs = task.run(&[
            "history", "usage", "--days", "0", "--task", "INF-123", "--json",
        ]);
        let runs: serde_json::Value = serde_json::from_slice(&runs.stdout).unwrap();
        let usage = runs
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["session_id"] == session.as_str())
            .map(|run| &run["usage"])
            .unwrap();
        // Pi's stand-in counts its one model call its own way.
        let (input, output) = if provider == "pi" { (10, 3) } else { (20, 5) };
        assert_eq!(usage["input_tokens"], input, "{provider}: {usage}");
        assert_eq!(usage["output_tokens"], output, "{provider}: {usage}");
    }
}

/// OpenCode cancels a headless question and lets the model assume an answer;
/// Pi waits on its dialog. Either way the worker's turn fails with what was
/// asked and the run stops there: a second attempt would only ask again. The
/// answer is a steer, which the next run's conversation hears.
#[test]
fn a_question_with_nobody_there_ends_the_step() {
    for (provider, agent, script) in [
        ("opencode", "opencode:opencode/fixture", opencode_stand_in()),
        ("pi", "pi:fixture/model", pi_stand_in()),
    ] {
        let task = GraphTask::with_tools(&[(provider, &script)]);
        define_worker_graph(&task);
        let home = task.home.path();
        fs::write(home.join("ask-question"), "").unwrap();

        let asked = task.task_run(&["task", "run", "INF-123", "worker", "-a", agent]);
        assert!(!asked.success(), "{provider}: {}", asked.text);
        let reason = "asked a question with nobody there to answer it: Use the fixture?";
        assert!(asked.text.contains(reason), "{provider}: {}", asked.text);
        assert!(
            asked
                .text
                .contains("answer with `lf steer INF-123 \"…\"`, then `lf task run INF-123`"),
            "{provider}: {}",
            asked.text
        );
        assert!(
            !asked.text.contains("starting it again"),
            "{provider}: {}",
            asked.text
        );
        assert_eq!(support::recorded_graphs(home).len(), 1, "{provider}");
        assert_eq!(task.graph()["position"]["running"], false, "{provider}");
        assert!(!asked.text.contains("?."), "{provider}: {}", asked.text);
        let status = task.text_status();
        assert!(status.contains(reason), "{provider}: {status}");
        assert!(
            status.contains("answer with `lf steer INF-123 \"…\"`, then `lf task run INF-123`"),
            "{provider}: {status}"
        );
        assert!(
            !status.contains("launch fresh work"),
            "{provider}: {status}"
        );
        assert!(
            !status.contains("Fixture completed."),
            "{provider}: {status}"
        );

        // The answer reaches the conversation the next run starts.
        task.ok(&["steer", "INF-123", "USE_THE_FIXTURE"]);
        fs::remove_file(home.join("ask-question")).unwrap();
        fs::write(
            home.join("tool_turn.py"),
            include_str!("../../../loopflow-execution/tests/support/tool_turn.py"),
        )
        .unwrap();
        fs::write(home.join("tool-done"), "").unwrap();
        task.task_run_ok(&["task", "run", "INF-123", "-a", agent]);
        assert_eq!(task.graph()["position"], at("review"), "{provider}");
        let heard = ["hook-context", "hook-steer"]
            .map(|hook| fs::read_to_string(home.join(hook)).unwrap())
            .join("\n");
        assert!(heard.contains("USE_THE_FIXTURE"), "{provider}: {heard}");
    }
}

/// `lf task interrupt` ends a worker's turn while it runs: the turn is recorded
/// interrupted, nothing starts the run again, and the next `lf task run` takes
/// the step up and moves the Graph on.
#[test]
fn a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again_on_claude_and_codex() {
    interrupt_a_worker("claude", "claude", &holding_claude(), "Held.");
    interrupt_a_worker(
        "codex",
        "codex",
        &support::codex_app_server_script("Worker finished.", ""),
        "Worker finished.",
    );
}

#[test]
fn a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again_on_opencode_and_pi() {
    interrupt_a_worker(
        "opencode",
        "opencode:opencode/fixture",
        &opencode_stand_in(),
        "Fixture completed.",
    );
    interrupt_a_worker("pi", "pi:fixture/model", &pi_stand_in(), "answer 1:");
}

#[test]
fn a_worker_stops_on_interrupt_and_the_next_run_starts_its_step_again_on_antigravity() {
    interrupt_a_worker("agy", "agy", &antigravity_stand_in(), "answer to:");
}

fn interrupt_a_worker(provider: &str, agent: &str, script: &str, answer: &str) {
    {
        let task = GraphTask::with_tools(&[(provider, script)]);
        define_worker_graph(&task);
        let home = task.home.path();
        fs::write(home.join("hold-turn"), "").unwrap();
        fs::write(home.join("release"), "").unwrap();

        let mark = last_graph(home);
        let started = task.run(&["task", "run", "INF-123", "worker", "-a", agent]);
        // The stand-in says when the worker's own turn is under way.
        wait_for("the worker's turn", || home.join("holding").exists());
        task.ok(&["task", "interrupt", "INF-123"]);
        let stopped = task.ran(started);
        assert_eq!(stopped.code, Some(130), "{provider}: {}", stopped.text);
        assert_eq!(attempts(home, mark), ["interrupted"], "{provider}");
        assert_eq!(task.graph()["position"]["running"], false, "{provider}");

        let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
        let worker = |ended: &str| -> (String, Option<String>, Option<String>) {
            db.query_row(
                &format!(
                    "SELECT s.id, s.provider_thread,
                        (SELECT turn_status FROM session_events
                          WHERE session_id=s.id AND kind='captured'
                          ORDER BY rowid DESC LIMIT 1)
                     FROM agent_sessions s
                     WHERE s.task_id=?1 AND s.provider=?2 AND s.skill='implement'
                       AND s.interactive=0 {ended}
                     ORDER BY s.rowid DESC LIMIT 1"
                ),
                rusqlite::params![task.registered.task.id.as_str(), provider],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|error| panic!("{provider}: no worker Session: {error}"))
        };
        let (interrupted, thread, turn) = worker("");
        assert_eq!(turn.as_deref(), Some("interrupted"), "{provider}");
        assert!(
            thread.is_some_and(|thread| !thread.is_empty()),
            "{provider}"
        );
        // The stand-in is gone with its turn: nothing answers after the stop.
        let history = task.run(&["history", "show", &interrupted, "--final"]);
        assert!(
            !String::from_utf8_lossy(&history.stdout).contains(answer),
            "{provider}: {history:?}"
        );

        // Nobody continues a Run: the step starts again in a conversation of its own.
        fs::remove_file(home.join("hold-turn")).unwrap();
        task.task_run_ok(&["task", "run", "INF-123", "-a", agent]);
        assert_eq!(task.graph()["position"], at("review"), "{provider}");
        let (resumed, _, turn) = worker("AND s.completed_at IS NOT NULL");
        assert_ne!(resumed, interrupted, "{provider}");
        assert_eq!(turn.as_deref(), Some("completed"), "{provider}");
        let history = task.run(&["history", "show", &resumed, "--final"]);
        assert!(
            String::from_utf8_lossy(&history.stdout).contains(answer),
            "{provider}: {history:?}"
        );
    }
}

/// A harness that is not installed fails the same way on every attempt, so the
/// run is not started again; with another agent in the order the step runs there.
#[test]
fn a_step_that_cannot_launch_is_not_started_again() {
    let codex = support::codex_app_server_script("Worker finished.", "");
    let task = GraphTask::with_tools(&[("codex", &codex)]);
    define_worker_graph(&task);

    let pinned = task.task_run(&[
        "task",
        "run",
        "INF-123",
        "worker",
        "-a",
        "opencode:opencode/fixture",
    ]);
    assert!(!pinned.success(), "{}", pinned.text);
    let hint = "'opencode' is not installed. Install OpenCode 2 or later";
    assert!(pinned.text.contains(hint), "{}", pinned.text);
    assert!(
        pinned.text.contains("stopped before any agent started"),
        "{}",
        pinned.text
    );
    assert!(
        !pinned.text.contains("starting it again"),
        "{}",
        pinned.text
    );
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    let status = task.text_status();
    assert!(status.contains("'opencode' is not installed"), "{status}");

    task.ok(&[
        "set",
        "--global",
        "agents",
        "opencode:opencode/fixture,codex",
    ]);
    let moved = task.task_run(&["task", "run", "INF-123"]);
    assert!(moved.success(), "{}", moved.text);
    assert_eq!(task.graph()["position"], at("review"));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let ran_on_codex: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1 AND provider='codex'
                 AND skill='implement' AND interactive=0 AND completed_at IS NOT NULL)",
            [task.registered.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(ran_on_codex, "{}", moved.text);
    let recorded: String = db
        .query_row("SELECT reason FROM agent_limit_recoveries", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(
        recorded.starts_with("'opencode' is not installed")
            && recorded.ends_with(&format!(
                "{}codex",
                loopflow_execution::ops::task_execution::MOVED_TO
            )),
        "{recorded}"
    );
}

#[test]
fn a_failed_command_step_is_started_again() {
    let task = GraphTask::new();
    let failed = task.task_run(&["task", "run", "INF-123", "held-review"]);
    assert!(!failed.success(), "{}", failed.text);
    assert!(
        failed.text.contains(
            "The run of held-review from start failed (attempt 1 of 3); starting it again at graph show late."
        ),
        "{}",
        failed.text
    );
    // The first attempt finished `task sync`; the retries start at the failed step.
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 3);
    assert_eq!(graphs[0].1.len(), 2);
    for retry in &graphs[1..] {
        assert_eq!(retry.1.len(), 1);
        assert_eq!(retry.1[0]["label"], "graph show late");
    }
    let stopped = task.graph();
    assert_eq!(stopped["position"]["step"], "start");
    assert_eq!(stopped["position"]["at"], "graph show late");
    assert_eq!(stopped["position"]["running"], false);
    let status = task.text_status();
    assert!(
        status.contains("Graph held-review: Stopped at graph show late."),
        "{status}"
    );
}

#[test]
fn from_starts_a_stopped_run_where_it_names() {
    let task = GraphTask::new();
    let failed = task.task_run(&["task", "run", "INF-123", "held-review"]);
    assert!(!failed.success(), "{}", failed.text);
    fs::write(
        task.repo.path().join(".lf/graphs/late.yaml"),
        "- cmd: task sync --plan\n",
    )
    .unwrap();
    task.task_run_ok(&["task", "run", "INF-123", "--from", "start"]);
    let graphs = support::recorded_graphs(task.home.path());
    let steps = &graphs.last().unwrap().1;
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0]["label"], "sync --plan");
    assert_eq!(task.graph()["position"], at("review"));
}

#[test]
fn a_run_that_failed_after_moving_agents_starts_again_at_the_step_that_failed() {
    let task = GraphTask::new();
    let failed = task.task_run(&["task", "run", "INF-123", "held-review"]);
    assert!(!failed.success(), "{}", failed.text);
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    db.execute(
        "INSERT INTO agent_limit_recoveries(lf_process_id,provider,reason,updated_at)
         SELECT s.lf_process_id,'codex',?1,1 FROM graph_process_steps s WHERE s.graph_lf_process_id=
             (SELECT lf_process_id FROM graph_processes ORDER BY rowid DESC LIMIT 1)
         ORDER BY s.seq DESC LIMIT 1",
        [format!(
            "codex_error: You’ve hit your usage limit{}claude",
            loopflow_execution::ops::task_execution::MOVED_TO
        )],
    )
    .unwrap();
    fs::write(
        task.repo.path().join(".lf/graphs/late.yaml"),
        "- cmd: task sync --plan\n",
    )
    .unwrap();
    task.task_run_ok(&["task", "run", "INF-123"]);
    let graphs = support::recorded_graphs(task.home.path());
    let steps = &graphs.last().unwrap().1;
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["label"], "graph show late");
    assert_eq!(task.graph()["position"], at("review"));
}

#[test]
fn a_stopped_run_is_started_again_at_the_step_that_failed() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    // Every attempt fails: the Task run gives up and the run holds the Task.
    let mark = last_graph(task.home.path());
    let ran = task.task_run(&["task", "run", "INF-123"]);
    assert!(!ran.success());
    assert!(ran.text.contains("failed 3 times"), "{}", ran.text);
    let stopped = task.graph();
    assert_eq!(stopped["position"]["kind"], "run");
    assert_eq!(stopped["position"]["step"], "review");
    assert_eq!(stopped["position"]["at"], "graph show late");
    assert_eq!(stopped["position"]["running"], false);
    assert_eq!(
        attempts(task.home.path(), mark),
        ["failed", "failed", "failed"]
    );
    let status = task.text_status();
    assert!(status.contains("Graph gated: Stopped at gate."), "{status}");

    // The cause clears as the third attempt starts: an `lf` that defines
    // `late` the third time the gate asks for it.
    let bin = tempfile::tempdir().unwrap();
    let mark = last_graph(task.home.path());
    let lf = lf_shim(
        bin.path(),
        &format!(
            "  echo asked >> '{count}'\n  [ \"$(wc -l < '{count}')\" -ge 3 ] && printf -- '- cmd: task sync --plan\\n' > '{late}';;",
            count = bin.path().join("asked").display(),
            late = task.repo.path().join(".lf/graphs/late.yaml").display(),
        ),
    );
    let ran = task.ran(
        command(
            task.repo.path(),
            task.home.path(),
            &["task", "run", "INF-123"],
        )
        .env("LF_BIN", &lf)
        .output()
        .unwrap(),
    );
    assert!(ran.success(), "{}", ran.text);
    // The bare run started the stopped run again at the gate, not after review.
    assert_eq!(task.graph()["position"], at("accepted"));
    assert_eq!(
        attempts(task.home.path(), mark),
        ["failed", "failed", "succeeded"]
    );
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 7);
}

#[test]
fn a_held_command_stops_the_task_run_without_repeating_the_graph() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    let bin = tempfile::tempdir().unwrap();
    let lf = lf_shim(bin.path(), "  exit 3;;");
    let ran = task.ran(
        command(
            task.repo.path(),
            task.home.path(),
            &["task", "run", "INF-123"],
        )
        .env("LF_BIN", &lf)
        .output()
        .unwrap(),
    );
    assert_eq!(ran.code, Some(3), "{}", ran.text);
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
    let graph = task.graph();
    assert_eq!(graph["position"]["kind"], "run");
    assert_eq!(graph["position"]["running"], false);
    assert!(!ran.text.contains("); starting it again"), "{}", ran.text);
}

fn interrupt_rows(home: &Path) -> i64 {
    rusqlite::Connection::open(home.join("loopflow.db"))
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM task_events WHERE json_extract(kind_json,'$.kind')='interrupt'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn an_interrupt_during_an_operation_stops_the_graph_before_its_next_step() {
    let task = GraphTask::new();
    for (path, content) in [
        (
            ".lf/graphs/interrupting.yaml",
            "- cmd: task interrupt INF-123\n- cmd: task sync --plan\n",
        ),
        (
            ".lf/graphs/interrupted.yaml",
            "- graph: interrupting\n- interactive: {name: demo, id: review}\n",
        ),
    ] {
        fs::write(task.repo.path().join(path), content).unwrap();
    }
    task.select_wave_graph("interrupted");
    let mark = last_graph(task.home.path());
    let ran = task.task_run(&["task", "run", "INF-123", "interrupted"]);
    assert_eq!(ran.code, Some(130), "{}", ran.text);
    assert!(
        ran.text.contains("INF-123: stopped Graph interrupted"),
        "{}",
        ran.text
    );
    assert!(!ran.text.contains("); starting it again"), "{}", ran.text);
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].1.len(), 1, "the second step never started");
    assert_eq!(attempts(task.home.path(), mark), ["interrupted"]);
    // Status reads the interrupted run as stopped, not failed.
    let status = command(
        task.repo.path(),
        task.home.path(),
        &["task", "status", "INF-123"],
    )
    .output()
    .unwrap();
    let status = String::from_utf8_lossy(&status.stdout);
    let line = status
        .lines()
        .find(|line| line.trim_start().starts_with("Graph: "))
        .unwrap_or_else(|| panic!("{status}"));
    assert!(
        line.contains("Stopped") && !line.contains("ailed"),
        "{status}"
    );
}

#[test]
fn interrupting_an_idle_task_records_nothing_and_says_so() {
    let task = GraphTask::new();
    let before = interrupt_rows(task.home.path());
    let output = task.run(&["task", "interrupt", "INF-123", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["receipt"]["kind"], "no_live_run");
    assert_eq!(interrupt_rows(task.home.path()), before);
}

#[test]
fn a_graph_that_fails_with_retained_edits_is_not_started_again() {
    // LOO-578: starting a Graph over on top of a checkout-update stash strands
    // the edits it holds more deeply with every attempt.
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    let checkout = task.repo.path();
    fs::write(checkout.join("work.txt"), "unrestored\n").unwrap();
    let message = format!("lf checkout update: caller edits in {}", checkout.display());
    let pushed = std::process::Command::new("git")
        .args(["stash", "push", "--include-untracked", "-m", &message])
        .current_dir(checkout)
        .status()
        .unwrap();
    assert!(pushed.success());
    let stash = String::from_utf8(
        std::process::Command::new("git")
            .args(["rev-parse", "refs/stash"])
            .current_dir(checkout)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();

    let mark = last_graph(task.home.path());
    let ran = task.task_run(&["task", "run", "INF-123"]);
    assert!(!ran.success());
    assert!(ran.text.contains(stash.trim()), "{}", ran.text);
    assert!(!ran.text.contains("); starting it again"), "{}", ran.text);
    assert_eq!(attempts(task.home.path(), mark), ["failed"]);
}

#[test]
fn completion_is_independent_of_graph_position() {
    let task = GraphTask::new();
    assert_eq!(task.state(), "idle");
    // At an interactive step, then a stopped run: both between start and end.
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    assert_eq!(task.graph()["position"], at("review"));
    assert_eq!(task.state(), "active");
    // Wait for the carrier to exit before reading the stopped position.
    let failed = task.task_run(&["-b", "task", "run", "INF-123"]);
    assert_eq!(failed.code, Some(1), "{}", failed.text);
    let status = task.status();
    let stopped = &status["work"]["graph"];
    assert_eq!(stopped["position"]["kind"], "run");
    assert_eq!(stopped["position"]["running"], false);
    assert_eq!(status["status"], "idle");
    // Completing it leaves it where it stood.
    task.finish_after_provider_observation(&["task", "complete", "INF-123"]);
    assert_eq!(task.state(), "done");
    assert_eq!(&task.graph(), stopped);
    let text = task.text_status();
    assert!(text.contains("\nINF-123  done\n"), "{text}");
    // The retired completion flags do not return with the alias.
    assert!(!task
        .run(&["task", "complete", "INF-123", "--summary", "x"])
        .status
        .success());
}

#[test]
fn end_is_retained_while_the_tasks_pr_blocks_completion() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "findings"]);
    task.publish(false);
    // Reaching end requests completion; the unmerged PR refuses it.
    let error = refusal(task.run(&["task", "run", "INF-123"]));
    assert!(error.to_lowercase().contains("pull request"), "{error}");
    assert_eq!(task.graph()["position"], at("end"));
    // Nothing runs while completion waits on the PR, so the Task reads idle.
    assert_eq!(task.state(), "idle");
    // Nothing turned auto on; a stop stands at `end` all the same.
    let stop = task.run(&["--task", "INF-123", "task", "auto", "off"]);
    let stop = String::from_utf8_lossy(&stop.stdout);
    assert!(stop.contains("INF-123: auto off"), "{stop}");
    let error = refusal(task.run(&["task", "complete", "INF-123"]));
    assert!(error.to_lowercase().contains("pull request"), "{error}");
    assert_eq!(task.graph()["position"], at("end"));
    // At end, a run points to `--from`.
    let error = refusal(task.run(&["task", "run", "INF-123"]));
    assert!(error.contains("--from <step>"), "{error}");
    assert_eq!(task.state(), "idle");
}

#[test]
fn local_reopening_preserves_delivery_and_graph_and_supersedes_pending_completion() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "findings"]);
    task.publish(false);
    assert!(!task.run(&["task", "run", "INF-123"]).status.success());
    assert!(!task.status()["completion_pending"].is_null());
    let graph = task.graph();
    assert_eq!(graph["position"], at("end"));
    let graphs = support::recorded_graphs(task.home.path());
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
    assert_eq!(task.graph(), graph);
    assert_eq!(support::recorded_graphs(task.home.path()), graphs);
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
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    task.ok(&["task", "reopen", "INF-123"]);
    assert_eq!(task.state(), "idle");
    assert_eq!(task.graph(), graph);
    task.ok(&["task", "reopen", "INF-123"]);
    assert_eq!(task.graph(), graph);
    assert!(task.repo.path().exists());
}

#[test]
fn linear_completion_and_reopening_preserve_the_graph() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    let graph = task.graph();
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    assert_eq!(task.graph(), graph);
    assert!(task.status()["completion_pending"].is_null());
    let rt = tokio::runtime::Runtime::new().unwrap();
    let scope = task
        .repo
        .path()
        .canonicalize()
        .unwrap()
        .display()
        .to_string();
    let store = &task.registered.store;
    let mut record: crate::test_linear_input::IssueFixture =
        loopflow_test_support::retained_task_input(&task.registered.database, &scope, "INF-123");
    let stale = record.clone();
    record.item.revision = Some("2026-10-07T12:00:00Z".into());
    record.item.state = Some("started".into());
    record.item.completed = false;
    rt.block_on(store.seed_issue(&scope, "linear", record, None))
        .unwrap();
    rt.block_on(store.seed_issue(&scope, "linear", stale, None))
        .unwrap();
    assert_eq!(task.state(), "active");
    assert_eq!(task.graph(), graph);
    assert!(task.status()["completion_pending"].is_null());
}

#[test]
fn linear_completing_a_task_that_never_started_withdraws_it() {
    let task = GraphTask::new();
    task.complete_in_linear();
    assert!(task.status()["completion_pending"].is_null());
    let ran = task.task_run(&["task", "run", "INF-123", "gated"]);
    assert!(!ran.success());
    assert!(
        ran.text.contains("terminal") || ran.text.contains("is done"),
        "{}",
        ran.text
    );
    assert!(support::recorded_graphs(task.home.path()).is_empty());
    assert_eq!(task.state(), "done");
}

#[test]
fn completion_preserves_retained_session_input_and_unknown_process_history() {
    let task = GraphTask::new();
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let exec = uuid::Uuid::new_v4().to_string();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    db.execute("INSERT INTO processes(id,trace_id,cwd,started_at,command) VALUES(?1,?1,?2,?3,'historical inspection')",
        rusqlite::params![exec, task.repo.path().canonicalize().unwrap().to_str().unwrap(), now]).unwrap();
    db.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,interactive,task_id,wave_id,cwd)
        VALUES('retained-input','retained input','generated',?1,0,0,?2,?3,?4)",
        rusqlite::params![now, task.registered.task.id.as_str(), task.registered.task.wave_id.as_ref().unwrap().as_str(), task.repo.path().to_str().unwrap()]).unwrap();
    db.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('retained-input','captured',?1,1,'{}')", [uuid::Uuid::new_v4().simple().to_string()]).unwrap();
    db.execute(
        "UPDATE agent_sessions SET current_capture=?1 WHERE id='retained-input'",
        [db.last_insert_rowid()],
    )
    .unwrap();
    let sessions: String = db.query_row("SELECT json_object('published',input_published,'completed',completed_at,'cwd',cwd) FROM agent_sessions WHERE id='retained-input'", [], |row| row.get(0)).unwrap();
    task.finish_after_provider_observation(&["task", "complete", "INF-123"]);
    assert!(task.graph().is_null());
    for _ in 0..2 {
        task.ok(&["task", "complete", "INF-123"]);
        assert_eq!(task.state(), "done");
        assert!(task.repo.path().exists());
    }
    let after: String = db.query_row("SELECT json_object('published',input_published,'completed',completed_at,'cwd',cwd) FROM agent_sessions WHERE id='retained-input'", [], |row| row.get(0)).unwrap();
    assert_eq!(sessions, after);
    let unfinished: bool = db
        .query_row(
            "SELECT completed_at IS NULL AND outcome IS NULL FROM processes WHERE id=?1",
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
fn merged_delivery_completes_without_disposition_and_is_idempotent() {
    let task = GraphTask::new();
    task.publish(true);
    task.task_run_ok(&["task", "run", "INF-123", "findings"]);
    assert_ne!(task.state(), "done");
    assert!(task.repo.path().exists());
    task.finish_after_provider_observation(&["task", "complete", "INF-123"]);
    let graph = task.graph();
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.graph(), graph);
    let status = task.status();
    assert_eq!(status["status"], "done");
    assert_eq!(status["pr"]["publication"]["github"]["number"], 42);
    assert!(status.get("follow_through").is_none());
    assert!(!task
        .run(&["task", "follow-up", "INF-123", "--none", "obsolete"])
        .status
        .success());
}

#[test]
fn unmerged_completion_in_graph_is_retryable_without_replaying_it() {
    let task = GraphTask::new();
    fs::write(
        task.repo.path().join(".lf/graphs/finish-proof.yaml"),
        "- cmd: task complete INF-123\n",
    )
    .unwrap();
    fs::write(
        task.repo.path().join(".lf/graphs/delivery.yaml"),
        "- graph: finish-proof\n",
    )
    .unwrap();
    task.publish(false);
    task.select_wave_graph("delivery");
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "delivery"]));
    assert!(task.status()["completion_pending"].is_string(), "{error}");
    let graphs = support::recorded_graphs(task.home.path());
    assert!(!graphs.is_empty());
    assert!(graphs
        .iter()
        .all(|graph| graph.0.as_deref() == Some("failed")));
    task.publish(true);
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.state(), "done");
    let graph = task.graph();
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.graph(), graph);
    assert_eq!(support::recorded_graphs(task.home.path()), graphs);
}

#[test]
fn provider_completion_preserves_a_live_run_and_its_graph_process_records_the_arrival() {
    let task = holding_task();
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
    write(".lf/graphs/live-graph.yaml", "- graph: live\n");
    write(HOLD_SKILL.0, HOLD_SKILL.1);
    write(
        ".lf/graphs/live.yaml",
        &format!("{HOLD_STEP}- cmd: task complete INF-123\n"),
    );
    // The run names the checkout's graph file; no Wave selection is needed.
    let log_path = task.home.path().join("live.log");
    let log = fs::File::create(&log_path).unwrap();
    let mut held = HeldGraph {
        child: command(
            task.repo.path(),
            task.home.path(),
            &["-b", "task", "run", "INF-123", "live-graph"],
        )
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap(),
        release: task.home.path().join("release"),
        caller: task.home.path().join("unused"),
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    // The native Task conversation also writes `ready`. Wait for the Graph's
    // headless provider before moving the Task out of its launch Wave.
    while !task.home.path().join("stream-ready").exists() {
        assert!(
            held.child.try_wait().unwrap().is_none() && Instant::now() < deadline,
            "{}",
            fs::read_to_string(&log_path).unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let before = task.graph();
    assert_eq!(before["position"]["running"], true);
    task.publish(false);
    let placement = task.status();
    assert!(placement["pr"]["id"].is_string());
    // `wave add` would make the Project in Linear, which this fixture can't reach.
    let repo = task.repo.path().canonicalize().unwrap();
    loopflow_execution::test_support::seed_wave_project(
        &task.registered.store.sqlite,
        repo.to_str().unwrap(),
        "other",
    )
    .unwrap();
    task.ok(&["task", "move", "INF-123", "--wave", "other"]);
    let moved = task.status();
    assert_eq!(moved["wave"], "other");
    assert_eq!(moved["pr"], placement["pr"]);
    assert_eq!(moved["worktree"], placement["worktree"]);
    assert_eq!(moved["branch"], placement["branch"]);
    assert_eq!(task.graph(), before);
    assert!(held.child.try_wait().unwrap().is_none());
    task.ok(&["task", "move", "INF-123", "--no-wave"]);
    assert_eq!(task.status()["pr"], placement["pr"]);
    assert_eq!(task.graph(), before);
    assert!(held.child.try_wait().unwrap().is_none());
    task.ok(&[
        "task",
        "move",
        "INF-123",
        "--wave",
        task.registered.task.wave_id.as_ref().unwrap().as_str(),
    ]);
    assert_eq!(task.graph(), before);
    assert_eq!(task.status()["worktree"], placement["worktree"]);
    assert_eq!(task.status()["branch"], placement["branch"]);
    assert_eq!(task.status()["pr"], placement["pr"]);
    assert!(held.child.try_wait().unwrap().is_none());
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    assert_eq!(task.graph(), before);
    task.ok(&["task", "reopen", "INF-123"]);
    assert_ne!(task.state(), "done");
    assert_eq!(task.graph(), before);
    assert!(held.child.try_wait().unwrap().is_none());
    assert!(task.repo.path().exists());
    task.publish(true);
    fs::write(&held.release, "").unwrap();
    assert!(
        held.child.wait().unwrap().success(),
        "{}",
        fs::read_to_string(&log_path).unwrap()
    );
    assert_eq!(task.graph()["position"], at("end"));
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].0.as_deref(), Some("succeeded"));
}

#[test]
fn successful_final_graph_keeps_arrival_when_completion_fails_and_never_replays() {
    let task = GraphTask::new();
    task.repo
        .create_file(".lf/graphs/final.yaml", "- graph: proof\n");
    task.select_wave_graph("final");
    task.publish(false);
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "final"]));
    assert!(task.status()["completion_pending"].is_string(), "{error}");
    let arrived = task.graph();
    assert_eq!(arrived["position"], at("end"));
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].0.as_deref(), Some("succeeded"));
    task.publish(true);
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.graph(), arrived);
    assert_eq!(support::recorded_graphs(task.home.path()), graphs);
}

/// A Claude stand-in that records each first turn and, while `hold` exists in
/// LF_HOME, waits before answering.
const STAND_IN_CLAUDE: &str = r#"#!/usr/bin/python3
import json, os, sys, time
if sys.argv[1:] == ['auth', 'status']:
    sys.exit(0)
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = os.environ['LF_HOME']
message = json.loads(sys.stdin.readline())
with open(os.path.join(home, 'stand-ins.jsonl'), 'a') as turns:
    turns.write(json.dumps(message['message']['content'][0]['text']) + '\n')
while os.path.exists(os.path.join(home, 'hold')):
    time.sleep(0.1)
message['session_id'] = 'stand-in'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'stand-in'}), flush=True)
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': 'The proof passes; advance.'}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'stand-in-result', 'session_id': 'stand-in', 'subtype': 'success', 'is_error': False, 'result': 'The proof passes; advance.'}), flush=True)
for line in sys.stdin:
    pass
"#;

fn stand_in_task() -> GraphTask {
    let task = GraphTask::with_tools(&[("claude", STAND_IN_CLAUDE)]);

    loopflow_execution::config::files::update(&task.home.path().join("config.yaml"), |value| {
        value["agent"] = "claude".into();
        Ok(())
    })
    .unwrap();
    task
}

fn stand_ins(task: &GraphTask) -> Vec<String> {
    fs::read_to_string(task.home.path().join("stand-ins.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The Task history lines that record auto turning on.
fn handoffs(task: &GraphTask) -> Vec<String> {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(
            task.registered
                .store
                .task_events_after(&task.registered.task.id, 0),
        )
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.kind {
            loopflow_execution::work::task::TaskEventKind::Progress { summary }
                if summary.starts_with("auto on: ") =>
            {
                Some(summary)
            }
            _ => None,
        })
        .collect()
}

/// Whether the Task carries a steer saying `direction`.
fn steered(task: &GraphTask, direction: &str) -> bool {
    task.registered
        .store
        .task_comments(&task.registered.task.id)
        .unwrap()
        .comments
        .iter()
        .any(|comment| {
            comment.body.starts_with(direction) && comment.body.contains("loopflow-steer:")
        })
}

fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(90);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn a_batch_task_run_runs_straight_through_its_interactive_steps_to_the_end() {
    let task = stand_in_task();
    let output = task.run(&["-b", "task", "run", "INF-123"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(task.graph()["position"], at("end"));
    // One Graph process ran proof, then review headless, standing in.
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    let turns = stand_ins(&task);
    assert_eq!(turns.len(), 1);
    assert!(turns[0].contains("at review"), "{}", turns[0]);
    assert_eq!(task.state(), "done");
    // `-b` stood in for its own run; it turned no auto on.
    assert!(task.status()["auto"].is_null());
}

#[test]
fn auto_on_runs_every_remaining_step_standing_in_at_each_interactive_step() {
    let task = stand_in_task();
    let output = task.run(&[
        "-b",
        "--task",
        "INF-123",
        "task",
        "auto",
        "on",
        "--reason",
        "good to go",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(handoffs(&task).len(), 1);
    // `--reason` is the run's direction: a steer on the Task.
    assert!(steered(&task, "good to go"));
    // The review step ran headless with its skill, standing in for a person.
    let turns = stand_ins(&task);
    assert_eq!(turns.len(), 1);
    assert!(turns[0].contains("You are standing in for"), "{}", turns[0]);
    assert!(turns[0].contains("at review"), "{}", turns[0]);
    // Reaching the end completed the Task; nothing remains to run.
    assert_eq!(task.state(), "done");
    let error = refusal(task.run(&["-b", "--task", "INF-123", "task", "auto", "on"]));
    assert!(error.contains("INF-123 is done"), "{error}");
}

#[test]
fn a_waited_auto_on_stands_in_at_the_step_the_task_waits_at() {
    let task = stand_in_task();
    task.task_run_ok(&["task", "run", "INF-123"]);
    assert_eq!(task.graph()["position"], at("review"));
    let output = task.run(&["-b", "--task", "INF-123", "task", "auto", "on"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("INF-123: standing in at review"),
        "{stderr}"
    );
    assert_eq!(task.graph()["position"], at("end"));
    // Proof is not run again: the run set out from review.
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
    assert_eq!(stand_ins(&task).len(), 1);
    assert_eq!(task.state(), "done");
}

#[test]
fn auto_on_at_a_waiting_step_stands_in_there_with_one_graph_run() {
    let task = stand_in_task();
    task.task_run_ok(&["task", "run", "INF-123"]);
    assert_eq!(task.graph()["position"], at("review"));
    let graphs = support::recorded_graphs(task.home.path()).len();
    // Turning auto on is `lf task auto on`: nobody is waited for, so the Task runs.
    let on = task.task_run(&["task", "auto", "on", "INF-123", "--reason", "good to go"]);
    assert!(on.success(), "{}", on.text);
    assert!(
        on.text.contains("INF-123: standing in at review"),
        "{}",
        on.text
    );
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), graphs + 1);
    assert_eq!(stand_ins(&task).len(), 1);
    assert_eq!(handoffs(&task).len(), 1);
    // `--reason` is the run's direction: a steer on the Task.
    assert!(steered(&task, "good to go"));
    let status = task.status();
    assert_eq!(status["auto"]["state"], "on", "{status}");
    assert_eq!(status["auto"]["by"]["kind"], "person");
}

#[test]
fn auto_on_a_task_that_never_ran_leaves_exactly_one_run() {
    let task = stand_in_task();
    let on = task.task_run(&["task", "auto", "on", "INF-123"]);
    assert!(on.success(), "{}", on.text);
    assert!(on.text.contains("running as Graph run"), "{}", on.text);
    // The one run took every step, standing in at review.
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    assert_eq!(stand_ins(&task).len(), 1);
    assert_eq!(task.status()["auto"]["state"], "on");
}

#[test]
fn a_waves_auto_on_takes_every_step_of_its_tasks_and_an_off_still_wins() {
    let task = stand_in_task();
    task.set_wave_setting("auto", "on");
    // A bare run: the Wave's setting, not the command, stands in at review.
    task.task_run_ok(&["task", "run", "INF-123"]);
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(stand_ins(&task).len(), 1);
    assert!(
        task.text_status().contains("auto: on by wave config"),
        "{}",
        task.text_status()
    );
    // Reopened and taken up again, a Task keeps the off someone gave it.
    task.ok(&["task", "reopen", "INF-123"]);
    task.ok(&["task", "auto", "off", "INF-123"]);
    task.task_run_ok(&["task", "run", "INF-123", "findings"]);
    assert_eq!(task.graph()["position"], at("findings"));
    assert_eq!(stand_ins(&task).len(), 1);
    assert!(
        task.text_status().contains("auto: off by "),
        "{}",
        task.text_status()
    );
}

#[test]
fn an_off_before_the_first_run_outlasts_a_waves_auto_on() {
    let task = stand_in_task();
    task.set_wave_setting("auto", "on");
    // The Task has taken up no Graph; the off is the Task's all the same.
    let off = task.run(&["task", "auto", "off", "INF-123"]);
    let off = String::from_utf8_lossy(&off.stdout);
    assert!(off.contains("INF-123: auto off"), "{off}");
    task.task_run_ok(&["task", "run", "INF-123"]);
    assert_eq!(task.graph()["position"], at("review"));
    assert!(stand_ins(&task).is_empty());
    assert!(
        task.text_status().contains("auto: off by "),
        "{}",
        task.text_status()
    );
}

#[test]
fn auto_on_a_running_task_reports_its_run_and_starts_nothing() {
    let task = stand_in_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "task", "run", "INF-123"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    // Off first, so the `on` below is the run's.
    task.run(&["--task", "INF-123", "task", "auto", "off"]);
    assert!(
        task.text_status().contains("auto: off by "),
        "{}",
        task.text_status()
    );
    let shipped = task.run(&["--task", "INF-123", "task", "auto", "on"]);
    let text = String::from_utf8_lossy(&shipped.stdout);
    assert!(
        shipped.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&shipped.stderr)
    );
    assert!(text.contains("at review running as Graph run"), "{text}");
    // `feature` here has no landing step, so even with auto on it omits `merges:`.
    assert!(!text.contains("merges:"), "{text}");
    assert!(
        task.text_status().contains("auto: on by "),
        "{}",
        task.text_status()
    );
    fs::remove_file(task.home.path().join("hold")).unwrap();
    assert!(first.wait().unwrap().success());
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    assert_eq!(stand_ins(&task).len(), 1);
}

#[test]
fn merges_warning_follows_reachable_landing_steps() {
    let task = stand_in_task();
    task.repo.create_file(
        ".lf/graphs/post-merge-stop.yaml",
        "- graph: proof\n- interactive: {name: demo, id: review}\n- cmd: pr land\n- interactive: {name: research, id: check}\n",
    );
    task.repo.stage_all();
    task.repo.commit("Define post-merge-stop Graph");

    // Non-landing Graph with no interactive stops omits `merges:`.
    let no_merge = task.task_run(&["task", "run", "INF-123", "proof"]);
    assert!(no_merge.success(), "{}", no_merge.text);
    assert!(!no_merge.text.contains("merges:"), "{}", no_merge.text);
    // Reaching `end` completed the Task; reopen it for the post-merge-stop checks.
    task.ok(&["task", "reopen", "INF-123"]);

    // Custom Graph with a post-merge stop omits `merges:` before the pre-merge stop.
    let before_review = task.task_run(&["task", "run", "INF-123", "post-merge-stop"]);
    assert!(before_review.success(), "{}", before_review.text);
    assert_eq!(task.graph()["position"], at("review"));
    assert!(
        !before_review.text.contains("merges:"),
        "{}",
        before_review.text
    );

    // Continuing past `review` crosses `pr land` before `check`, so it warns `merges:`.
    let crossing_land = task.task_run(&["task", "run", "INF-123"]);
    assert!(
        crossing_land.text.contains("merges:"),
        "{}",
        crossing_land.text
    );

    // Turning auto on while a landing Graph is live reports `merges:`.
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "task", "run", "INF-123", "--from", "review"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    let live = task.run(&["--task", "INF-123", "task", "auto", "on"]);
    let text = String::from_utf8_lossy(&live.stdout);
    assert!(
        live.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&live.stderr)
    );
    assert!(text.contains("at review running as Graph run"), "{text}");
    assert!(text.contains("merges:"), "{text}");
    // Turn auto off so the live run stops cleanly at `pr land`.
    task.run(&["--task", "INF-123", "task", "auto", "off"]);
    fs::remove_file(task.home.path().join("hold")).unwrap();
    assert!(first.wait().unwrap().success());
    assert_eq!(task.graph()["position"], at("pr land"));
}

#[test]
fn auto_off_kills_nothing_and_its_run_carries_on() {
    let task = stand_in_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "task", "auto", "on"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    // State says a Graph runs; the setting says who takes its steps.
    assert_eq!(task.state(), "moving");
    assert!(
        task.text_status().contains("moving · auto: on by "),
        "{}",
        task.text_status()
    );
    let stopped = task.run(&["--task", "INF-123", "task", "auto", "off"]);
    let stopped = String::from_utf8_lossy(&stopped.stdout);
    assert!(stopped.contains("INF-123: auto off"), "{stopped}");
    // Auto is off; the step already standing in finishes.
    assert_eq!(task.state(), "moving");
    fs::remove_file(task.home.path().join("hold")).unwrap();
    assert!(first.wait().unwrap().success());
    let again = task.run(&["--task", "INF-123", "task", "auto", "off"]);
    let again = String::from_utf8_lossy(&again.stdout);
    assert!(again.contains("INF-123: auto off"), "{again}");
    assert!(
        task.text_status().contains("auto: off by "),
        "{}",
        task.text_status()
    );
    let json = task.run(&["task", "status", "INF-123", "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(json["auto"]["state"], "off", "{json}");
}

#[test]
fn auto_off_sticks_through_a_headless_relaunch() {
    let task = stand_in_task();
    task.repo.create_file(
        ".lf/graphs/landing.yaml",
        "- graph: proof\n- cmd: pr land\n",
    );
    // A person turns auto off; it stands though auto was never on.
    task.task_run_ok(&["task", "run", "INF-123"]);
    let stopped = task.run(&["--task", "INF-123", "task", "auto", "off"]);
    assert!(stopped.status.success());
    // A run with nothing interactive ahead would merge; under the stop it
    // waits at the landing step instead.
    let ran = task.task_run(&["task", "run", "INF-123", "landing"]);
    assert!(ran.success(), "{}", ran.text);
    assert!(
        ran.text.contains("Waiting at pr land: auto is off (")
            && ran.text.contains("`lf task auto on INF-123` lands it."),
        "{}",
        ran.text
    );
    assert_eq!(task.graph()["position"], at("pr land"));
    // Relaunched headless from its first step, `-b` leaves the stop.
    let again = task.run(&["-b", "task", "run", "INF-123", "--from", "proof"]);
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert_eq!(task.graph()["position"], at("pr land"));
    assert!(handoffs(&task).is_empty());
    let status = task.status();
    assert_eq!(status["auto"]["state"], "off", "{status}");
    assert_eq!(status["auto"]["by"]["kind"], "person");
    // Status says why it waits and what lands it, not "finished" and a run.
    let text = task.text_status();
    assert!(
        text.contains("latest Graph: Waiting at pr land: auto is off (")
            && text.contains("`lf task auto on INF-123` lands it."),
        "{text}"
    );
    assert!(text.contains("action: no_action"), "{text}");
    // Standing at the landing step, a plain relaunch waits there again.
    let waiting = task.task_run(&["task", "run", "INF-123"]);
    assert!(waiting.success(), "{}", waiting.text);
    assert_eq!(task.graph()["position"], at("pr land"));
    assert!(
        task.text_status().contains("auto: off by "),
        "{}",
        task.text_status()
    );
}

#[test]
fn a_run_that_fails_the_same_way_stops_and_keeps_auto_on() {
    let task = GraphTask::new();
    let mark = last_graph(task.home.path());
    // `gate` refuses identically until the repository defines `late`.
    let output = task.run(&[
        "-b", "--task", "INF-123", "task", "auto", "on", "--graph", "gate",
    ]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("failed 3 times with the same error"),
        "{stderr}"
    );
    // The foreground run finished all three attempts and left auto on.
    assert_eq!(
        attempts(task.home.path(), mark),
        ["failed", "failed", "failed"]
    );
    assert_eq!(task.state(), "idle");
    assert_eq!(task.status()["auto"]["state"], "on");
    let stopped = task.run(&["--task", "INF-123", "task", "auto", "off"]);
    let stopped = String::from_utf8_lossy(&stopped.stdout);
    assert!(stopped.contains("INF-123: auto off"), "{stopped}");
}

#[test]
fn an_auto_run_completes_when_its_pr_merges_mid_run() {
    let task = stand_in_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "task", "auto", "on"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    // Someone merges the PR by hand while the stand-in is still working.
    task.publish(true);
    fs::remove_file(task.home.path().join("hold")).unwrap();
    let output = first.wait_with_output().unwrap();
    assert!(output.status.success());
    // The merge interrupts nothing: the stand-in at review finishes into `end`.
    assert_eq!(task.state(), "done");
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(stand_ins(&task).len(), 1);
}

#[test]
fn an_auto_run_meets_a_hand_merged_pr_at_its_landing_step() {
    let task = stand_in_task();
    task.task_run_ok(&["task", "run", "INF-123", "verified"]);
    assert_eq!(task.graph()["position"], at("review"));
    // Someone merged the PR by hand while the Task waited at review.
    task.publish(true);
    let output = task.run(&["-b", "--task", "INF-123", "task", "auto", "on"]);
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // The run stood in at review, its landing step met the merge, and a
    // stand-in checked the merged work at `check`.
    assert_eq!(task.state(), "done");
    assert_eq!(task.graph()["position"], at("end"));
    let turns = stand_ins(&task);
    assert_eq!(turns.len(), 2, "{turns:?}");
    assert!(turns[1].contains("at check"), "{}", turns[1]);
}

#[test]
fn an_auto_run_whose_carrier_dies_still_reaches_its_end() {
    let task = stand_in_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "task", "auto", "on"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    first.kill().unwrap();
    first.wait().unwrap();
    fs::remove_file(task.home.path().join("hold")).unwrap();
    // The Graph outlives its carrier and arrives at `end` itself.
    wait_for("the run to reach end", || {
        task.graph()["position"] == at("end")
    });
    // Auto was turned on once.
    assert_eq!(handoffs(&task).len(), 1);
}

fn debug_run(task: &GraphTask) -> String {
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let mut out = String::new();
    for sql in [
        "SELECT json_array(step,run_start,lf_process_id,run_pid,restarts,restarted_at) FROM task_graphs",
        "SELECT json_array(lf_process_id,graph,started_at,ended_at,pid,exit_code) FROM graph_processes",
        "SELECT json_array(task_id,auto) FROM task_graphs",
    ] {
        let mut query = db.prepare(sql).unwrap();
        for row in query.query_map([], |row| row.get::<_, String>(0)).unwrap() {
            out.push_str(&row.unwrap());
            out.push('\n');
        }
    }
    out
}

/// Start `lf task run INF-123 <args>` in its own process group, holding at its
/// stand-in, and once `before` ran, SIGKILL its Graph and steps the way a
/// crash would, with its carrier too when `carrier`.
fn kill_a_carried_run(task: &GraphTask, args: &[&str], carrier: bool, before: impl FnOnce()) {
    use std::os::unix::process::CommandExt;
    let turns = stand_ins(task).len();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut launcher = command(
        task.repo.path(),
        task.home.path(),
        &[&["task", "run", "INF-123"], args].concat(),
    )
    .process_group(0)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || stand_ins(task).len() > turns);
    before();
    // The run's carrier is the detached `lf task run` the launcher let go;
    // the Graph and its steps run in groups of their own. A crash takes them
    // all, the carrier first so it cannot start its Graph again.
    let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
    let pids: Vec<i32> = db
        .prepare(
            "SELECT run_pid FROM task_graphs WHERE run_pid IS NOT NULL AND ?1
             UNION ALL SELECT pid FROM graph_processes WHERE ended_at IS NULL AND pid IS NOT NULL
             UNION ALL SELECT pid FROM graph_process_steps WHERE ended_at IS NULL AND pid IS NOT NULL",
        )
        .unwrap()
        .query_map([carrier], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(pids.len() > usize::from(carrier), "{pids:?}");
    for pid in pids {
        // SAFETY: kill and killpg only send signals to processes this test started.
        unsafe {
            libc::kill(pid, libc::SIGKILL);
            libc::killpg(pid, libc::SIGKILL);
        }
    }
    launcher.wait().unwrap();
}

#[test]
fn a_killed_graph_is_started_again_by_its_carrier() {
    let task = stand_in_task();
    task.repo.create_file(
        ".lf/graphs/carried.yaml",
        "- demo\n- interactive: {name: demo, id: review}\n",
    );
    task.repo.stage_all();
    task.repo.commit("Define a Graph with a step before review");
    // A SIGKILL is no person's stop: the carrier starts the Graph again.
    kill_a_carried_run(&task, &["carried"], false, || {});
    fs::remove_file(task.home.path().join("hold")).unwrap();
    wait_for("the run to stop at review", || {
        task.graph()["position"] == at("review")
    });
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 2, "{}", debug_run(&task));
    assert_eq!(task.status()["execution"]["state"], "idle");
}

#[test]
fn a_task_run_whose_carrier_died_is_carried_again_and_gives_up_after_five() {
    let task = stand_in_task();
    task.repo.create_file(
        ".lf/graphs/carried.yaml",
        "- demo\n- interactive: {name: demo, id: review}\n",
    );
    task.repo.stage_all();
    task.repo.commit("Define a Graph with a step before review");
    kill_a_carried_run(&task, &["carried"], true, || {});
    fs::remove_file(task.home.path().join("hold")).unwrap();
    // The recovery pass runs `lf task run` again, which stops at review as
    // the first one would have; nothing turned auto on.
    wait_for("the carried run to stop at review", || {
        task.graph()["position"] == at("review")
    });
    assert!(task.status()["auto"].is_null());
    let graphs = support::recorded_graphs(task.home.path()).len();
    assert_eq!(graphs, 2, "one Graph killed, one carried again");
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(support::recorded_graphs(task.home.path()).len(), graphs);

    // A run carried again five times in a row waits for a person.
    kill_a_carried_run(&task, &["--from", "demo"], true, || {
        let db = rusqlite::Connection::open(task.home.path().join("loopflow.db")).unwrap();
        db.execute(
            "UPDATE task_graphs SET restarts=5,restarted_at=unixepoch() WHERE task_id=?1",
            [task.registered.task.id.as_str()],
        )
        .unwrap();
    });
    fs::remove_file(task.home.path().join("hold")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let execution = loop {
        let execution = task.status()["execution"].clone();
        if execution["outlook"]["kind"] == "waiting_for_you" || Instant::now() > deadline {
            break execution;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    // The conversation's words: what it left it for, then the command.
    let reason = execution["reason"].as_str().unwrap_or_default();
    assert!(
        reason.starts_with("Run died ")
            && reason.ends_with("at demo · started 5 times; `lf task run INF-123` starts it again"),
        "{execution} {}",
        debug_run(&task)
    );
    assert_eq!(execution["state"], "blocked");
    assert_eq!(execution["died"]["at_step"], "demo");
    assert_eq!(execution["outlook"]["reason"], "started 5 times");
    assert_eq!(execution["outlook"]["command"], "lf task run INF-123");
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        support::recorded_graphs(task.home.path()).len(),
        graphs + 1,
        "nothing carried it again"
    );
}

#[test]
fn an_auto_run_unassigned_mid_run_waits_and_lets_its_running_step_finish() {
    let task = stand_in_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "task", "auto", "on"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    task.ok(&["task", "unassign", "INF-123"]);
    first.kill().unwrap();
    first.wait().unwrap();
    // Unassigning stops nothing already running.
    fs::remove_file(task.home.path().join("hold")).unwrap();
    wait_for("the running step to finish", || {
        task.graph()["position"] == at("end")
    });
    assert_eq!(stand_ins(&task).len(), 1);
}

#[test]
fn auto_on_runs_new_work_on_a_reopened_task_despite_its_old_merged_pr() {
    let task = stand_in_task();
    // The first delivery merged and completed the Task; then it was reopened.
    task.publish(true);
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.state(), "done");
    task.ok(&["task", "reopen", "INF-123", "--reason", "new repair"]);
    assert_ne!(task.state(), "done");
    let output = task.run(&[
        "-b", "--task", "INF-123", "task", "auto", "on", "--graph", "feature",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The old merge settled nothing: the new graph ran, standing in at review.
    assert!(!stdout.contains("its PR merged"), "{stdout}");
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    assert_eq!(stand_ins(&task).len(), 1);
    assert_eq!(task.graph()["position"], at("end"));
}

/// A Claude stand-in that records its first turn and answers that the step is
/// blocked on something only a person can do.
const BLOCKED_CLAUDE: &str = r#"#!/usr/bin/python3
import json, os, sys
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = os.environ['LF_HOME']
message = json.loads(sys.stdin.readline())
with open(os.path.join(home, 'stand-ins.jsonl'), 'a') as turns:
    turns.write(json.dumps(message['message']['content'][0]['text']) + '\n')
message['session_id'] = 'stand-in'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'stand-in'}), flush=True)
summary = 'Main-source checks pass; the installed release must be checked first.'
decision = {'decision': 'blocked', 'summary': None, 'reason': summary}
keep_claude_turn(message, summary, decision)
print(json.dumps({'type': 'result', 'uuid': 'stand-in-result', 'session_id': 'stand-in', 'subtype': 'success', 'is_error': False, 'result': summary, 'structured_output': decision}), flush=True)
for line in sys.stdin:
    pass
"#;

#[test]
fn an_auto_run_blocked_at_verification_waits_there_without_building_again() {
    let task =
        GraphTask::with_tools(&[("claude", support::claude_stand_in(BLOCKED_CLAUDE).as_str())]);
    loopflow_execution::config::files::update(&task.home.path().join("config.yaml"), |value| {
        value["agent"] = "claude".into();
        Ok(())
    })
    .unwrap();
    task.repo.create_file(
        ".lf/graphs/verifying.yaml",
        "- loop:\n    - graph: proof\n    - interactive: {name: research, id: check}\n",
    );
    task.repo.stage_all();
    task.repo.commit("Define the verifying Graph");
    let held = Some(i32::from(loopflow_execution::process::GraphHeld::EXIT));
    let first = task.run(&[
        "-b",
        "--task",
        "INF-123",
        "task",
        "auto",
        "on",
        "--graph",
        "verifying",
    ]);
    assert_eq!(first.status.code(), held, "{first:?}");
    // The stand-in at check reported a dependency: the Task waits at check.
    assert_eq!(
        task.graph()["position"],
        at("check"),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let status = task.text_status();
    assert!(
        status.contains("Graph is blocked: Main-source checks pass; the installed release must be checked first"),
        "{status}"
    );
    assert_eq!(stand_ins(&task).len(), 1);
    let again = task.run(&["-b", "--task", "INF-123", "task", "auto", "on"]);
    let stderr = String::from_utf8_lossy(&again.stderr);
    assert_eq!(again.status.code(), held, "{again:?}");
    assert!(stderr.contains("INF-123: standing in at check"), "{stderr}");
    assert_eq!(task.graph()["position"], at("check"));
    assert_eq!(stand_ins(&task).len(), 2);
    // The second run set out from check: proof did not run again.
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 2);
    assert_eq!(graphs[1].1.len(), 1, "{graphs:?}");
}
