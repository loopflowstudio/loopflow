use crate::test_linear_input::SeedLinear;
use crate::test_support as support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use loopflow::store::{CredentialState, ProviderAccount, ProviderAccountId, RoutingState};
use loopflow::work::task::{GithubPr, PrPublication};
use loopflow_test_support::TestRepo;
use sha2::{Digest, Sha256};

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

/// The shared native Codex stand-in: an app-server that records each turn
/// and a terminal client that attaches to it.
const CODEX_NATIVE: &str = concat!(
    "#!/bin/sh\nexec /usr/bin/python3 '",
    env!("CARGO_MANIFEST_DIR"),
    "/tests/support/native_codex.py' \"$@\"\n"
);

/// The three ways to name a Task for a Graph: the Task entry, `--task`, and its
/// worktree. All block in the foreground and reach the same checks.
const LAUNCHES: [&[&str]; 3] = [
    &["-b", "task", "run", "INF-123", "proof"],
    &["-b", "--task", "INF-123", "graph", "proof"],
    &["-b", "graph", "proof"],
];

#[test]
fn explicit_launch_policy_survives_task_workflow_nested_graphs_and_provider_children() {
    let repo = TestRepo::new();
    support::bind_task_planning(&repo);
    repo.create_branch("agent-proof");
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_home(
        &[
            ("open", "#!/bin/sh\nexit 0\n"),
            ("gh", "#!/bin/sh\nexit 1\n"),
            ("codex", CODEX_NATIVE),
            (
                "claude",
                r#"#!/usr/bin/python3
import json
import os
import subprocess
import sys

if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
with open(os.path.join(os.environ['LF_HOME'], 'launches.jsonl'), 'a') as launches:
    launches.write(json.dumps({'args': sys.argv[1:], 'claude_home': os.environ.get('CLAUDE_CONFIG_DIR')}) + '\n')
message = json.loads(sys.stdin.readline())
message['session_id'] = 'agent-fixture'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'agent-fixture'}), flush=True)
if os.environ.get('LF_AGENT_CHILD') != 'yes':
    with open(os.path.join(os.environ['LF_HOME'], 'child-output'), 'w') as output:
        subprocess.run(
            [os.environ['LF_BIN'], '-b', 'skill', 'child', '-a', 'codex'],
            env={**os.environ, 'LF_AGENT_CHILD': 'yes'},
            stdout=output, stderr=subprocess.STDOUT, check=True, timeout=30,
        )
print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'agent-fixture', 'subtype': 'success', 'is_error': False, 'result': 'done'}), flush=True)
for line in sys.stdin:
    pass
"#,
            ),
        ],
        Some(home.path()),
    );
    let registered = support::register_task_without_pr(
        home.path(),
        &repo.path().canonicalize().unwrap(),
        "agent-proof",
        &repo.head_sha(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    connect_fixture_claude(home.path(), &registered.store);
    runtime
        .block_on(
            registered
                .store
                .set_task_agent(&registered.task.id, "codex"),
        )
        .unwrap();
    for (path, content) in [
        (".lf/skills/first.md", "---\nagent: codex\n---\nFirst step."),
        (
            ".lf/skills/second.md",
            "---\nagent: codex\n---\nSecond step.",
        ),
        (
            ".lf/skills/child.md",
            "---\nagent: codex\n---\nProvider-issued child.",
        ),
        (
            ".lf/graphs/pursue.yaml",
            "- step: {name: first, agent: codex}\n- graph: inner\n",
        ),
        (
            ".lf/graphs/inner.yaml",
            "- step: {name: second, agent: codex}\n",
        ),
        (
            ".lf/graphs/feature.yaml",
            "- graph: pursue\n- interactive: {name: demo, id: review}\n",
        ),
    ] {
        let path = repo.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    repo.stage_all();
    repo.commit("Define agent selection fixtures");
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let workflow = fs::read_to_string(repo.path().join(".lf/graphs/feature.yaml")).unwrap();
    store
        .select_wave_workflow(
            registered.task.wave_id.as_ref().unwrap(),
            "feature",
            &workflow,
        )
        .unwrap();
    // Direct Task binding used to prefer the saved Task agent over -a.
    for args in [
        vec![
            "-b",
            "--yolo",
            "--chrome",
            "on",
            "--isolate",
            "--task",
            "INF-123",
            "skill",
            "first",
            "-a",
            "claude:sonnet",
        ],
        vec![
            "-b",
            "--yolo",
            "--chrome",
            "on",
            "--isolate",
            "-a",
            "claude:sonnet",
            "task",
            "run",
            "INF-123",
            "pursue",
        ],
    ] {
        let output = command(repo.path(), home.path(), &args).output().unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}\nchild: {}",
            String::from_utf8_lossy(&output.stderr),
            fs::read_to_string(home.path().join("child-output")).unwrap_or_default()
        );
    }
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let sessions = db
        .prepare("SELECT skill, provider FROM agent_sessions ORDER BY skill")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        sessions,
        ["child", "child", "child", "first", "first", "second"]
            .map(|skill| (skill.to_owned(), "claude".to_owned()))
    );
    assert_eq!(support::recorded_graphs(home.path()).len(), 1);
    let launches = fs::read_to_string(home.path().join("launches.jsonl")).unwrap();
    assert_eq!(launches.lines().count(), 6);
    for launch in launches.lines() {
        let received: serde_json::Value = serde_json::from_str(launch).unwrap();
        let args = received["args"].as_array().unwrap();
        assert!(args.contains(&serde_json::json!("--chrome")), "{launch}");
        assert!(
            !args.contains(&serde_json::json!("--no-chrome")),
            "{launch}"
        );
        assert!(
            args.contains(&serde_json::json!("--dangerously-skip-permissions")),
            "{launch}"
        );
        assert!(
            args.windows(2).any(|pair| pair == ["--model", "sonnet"]),
            "{launch}"
        );
        assert_eq!(
            Path::new(received["claude_home"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            home.path()
                .join("accounts/claude/fixture")
                .canonicalize()
                .unwrap()
        );
    }
    let session: String = db
        .query_row(
            "SELECT id FROM agent_sessions WHERE skill='first' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    // Connecting with another provider continues the same Session from a log;
    // contextual resume reaches the same open.
    let transcript = home
        .path()
        .join("accounts/claude/fixture/projects/fixture/agent-fixture.jsonl");
    fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    fs::write(
        &transcript,
        [
            serde_json::json!({"type":"user","userType":"external","message":{"content":"Rename the parser module"}}),
            serde_json::json!({"type":"assistant","message":{"content":[
                {"type":"text","text":"Renaming it now."},
                {"type":"tool_use","id":"read","name":"Read","input":{"file_path":"src/parser.rs"}},
                {"type":"tool_use","id":"move","name":"Bash","input":{"command":"git mv src/parser.rs src/syntax.rs"}}]}}),
            serde_json::json!({"type":"user","userType":"external","toolUseResult":"done","message":{"content":[
                {"type":"tool_result","tool_use_id":"read","content":"successful parser source"},
                {"type":"tool_result","tool_use_id":"move","is_error":true,"content":"Permission to use Bash has been denied."}]}}),
        ]
        .map(|row| row.to_string())
        .join("\n"),
    )
    .unwrap();
    // The first Codex app-server crashes before the conversation opens.
    fs::write(home.path().join("fail-server"), "").unwrap();
    let crashed = command(
        repo.path(),
        home.path(),
        &["session", "connect", &session, "-a", "codex"],
    )
    .output()
    .unwrap();
    assert!(!crashed.status.success());
    // Retrying without -a relaunches Codex with the same continuation.
    fs::remove_file(home.path().join("fail-server")).unwrap();
    let imported = command(repo.path(), home.path(), &["session", "connect", &session])
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        db.query_row(
            "SELECT provider FROM agent_sessions WHERE id=?1",
            [&session],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "codex"
    );
    let turns: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.path().join("native-turns.json")).unwrap())
            .unwrap();
    let turn = turns[0]["items"][0]["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|input| input["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(turn.contains("began in claude"), "{turn}");
    assert!(turn.contains("Rename the parser module"), "{turn}");
    // The skill ran in the Claude conversation; the continuation does not re-send it.
    assert!(!turn.contains("<lf:skill:first"), "{turn}");
    let log_path = turn
        .split_whitespace()
        .find_map(|word| {
            word.strip_suffix(";")
                .filter(|path| path.ends_with("conversation.md"))
        })
        .unwrap_or_else(|| panic!("no log path in {turn}"));
    let log = fs::read_to_string(log_path).unwrap();
    for expected in [
        "Rename the parser module",
        "Renaming it now.",
        "- tool: Bash",
        "Permission to use Bash has been denied.",
    ] {
        assert!(log.contains(expected), "{expected} missing from {log}");
    }
    assert!(!log.contains("successful parser source"), "{log}");

    // A separate invocation with no -a keeps skill and step declarations.
    let unbound = TestRepo::new();
    for (path, content) in [
        (
            ".lf/skills/default-proof.md",
            "---\nagent: claude:haiku\n---\nUse the declared agent.",
        ),
        (
            ".lf/graphs/default-proof.yaml",
            "- step: {name: default-proof, agent: 'claude:sonnet'}\n",
        ),
    ] {
        let path = unbound.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    unbound.stage_all();
    unbound.commit("Define default agent fixtures");
    for owner in ["skill", "graph"] {
        let output = command(
            unbound.path(),
            home.path(),
            &["-b", "--isolate", owner, "default-proof"],
        )
        .env("LF_AGENT_CHILD", "yes")
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let models = db
        .prepare("SELECT model FROM agent_sessions WHERE skill='default-proof' ORDER BY model")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(models, ["haiku", "sonnet"]);
}

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
        let config = repo.path().join(".lf/config.yaml");
        fs::write(
            &config,
            format!(
                "{}\nworkflow: proof\n",
                fs::read_to_string(&config).unwrap()
            ),
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
        runtime
            .block_on(
                registered
                    .store
                    .seed_issue(&scope, "linear", record, None, None),
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
                    registered.task.linear_id.as_ref().unwrap().as_str(),
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
                "moved" | "team" => "invalid ownership",
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
                support::recorded_graphs(home.path()).is_empty(),
                "{condition} launched a Graph"
            );
            continue;
        }
        // Each launch has finished when its command returns, and is its own
        // Graph; none continues another.
        for (launched, launch) in LAUNCHES.iter().enumerate() {
            let output = run(launch);
            assert!(!String::from_utf8_lossy(&output.stderr)
                .contains("Started requires recorded Task work"));
            assert!(
                output.status.success(),
                "{launch:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
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
        assert_eq!(
            recorded[0].1[0]["argv"],
            serde_json::json!(["sync", "--plan"])
        );
        assert!(recorded.iter().all(|(_, steps)| *steps == recorded[0].1));
        // The repository names a Graph, which is not a Workflow: a
        // run that names nothing is refused and starts no Graph.
        let output = run(&["-b", "task", "run", "INF-123"]);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("repository Workflow \"proof\" is unavailable"),
            "{error}"
        );
        assert_eq!(support::recorded_graphs(home.path()).len(), 3);
        let status = run(&["task", "status", "INF-123", "--json"]);
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
        // The Task took up no workflow and ran every named Graph ad hoc.
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
        // The worker's entry points are gone; nothing restarts a Graph.
        for removed in [
            vec!["--task", "INF-123", "graph", "start", "proof"],
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
        assert_eq!(support::recorded_graphs(home.path()).len(), 3);
    }
}

#[test]
fn failed_placement_is_observed_without_starting_a_task() {
    let task = WorkflowTask::new();
    let output = task.run(&["--task", "missing-task", "graph", "proof"]);
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

#[test]
fn running_graphs_belong_to_the_target_checkout_and_retain_their_caller() {
    for launch in ["task", "direct", "checkout", "alias", "wt"] {
        let task = WorkflowTask::new();
        // A real sibling checkout, managed through the public placement command.
        task.ok(&["wt", "create", "caller"]);
        // Worktree discovery is read-only and does not select an ambient Home.
        let caller = {
            loopflow::git::worktrees::list_worktrees(task.repo.path())
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
            .create_file(".lf/graphs/proof.yaml", "- cmd: __telemetry-scorecard\n");
        task.repo.create_file(
            "scripts/lifecycle_scorecard.py",
            r#"
import json
import os
from pathlib import Path
import time

home = Path(os.environ['LF_HOME'])
(home / 'ready.tmp').write_text(os.environ['LF_PROCESS_ID'])
(home / 'ready.tmp').replace(home / 'ready')
while not (home / 'release').exists():
    time.sleep(.02)
print(json.dumps({'report': {'ok': True}, 'metric_observations': [], 'text': ''}))
"#,
        );
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
            &["--task", "INF-123", "task", "status", "INF-123", "--json"],
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

        let parent = loopflow::id::LfProcessId::new();
        let trace = loopflow::id::TraceId::new();
        db.execute(
            "INSERT INTO processes(id,trace_id,cwd,command,started_at) VALUES(?1,?2,?3,'caller',1)",
            rusqlite::params![parent, trace, caller.to_str().unwrap()],
        )
        .unwrap();
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
        let status = task.status();
        let graphs = status["work"]["graph_processes"].as_array().unwrap();
        assert_eq!(graphs.len(), 1, "{launch}: {status}");
        assert_eq!(graphs[0]["state"], "current");
        assert_eq!(graphs[0]["task_id"], task.registered.task.id.as_str());
        let graph_process = graphs[0]["id"].as_str().unwrap();
        let recorded: String = db
            .query_row(
                "SELECT cwd FROM processes WHERE id=?1",
                [graph_process],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            Path::new(&recorded),
            task.repo.path().canonicalize().unwrap()
        );
        // Execution retains the caller through its viewer and transport owner;
        // the number of intervening Processes is not the attribution contract.
        let caller_retained: bool = db
            .query_row(
                "WITH RECURSIVE ancestors(id,parent_lf_process_id) AS (
                    SELECT id,parent_lf_process_id FROM processes WHERE id=?1
                    UNION ALL
                    SELECT p.id,p.parent_lf_process_id FROM processes p
                    JOIN ancestors a ON p.id=a.parent_lf_process_id
                 ) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id=?2)",
                rusqlite::params![graph_process, parent],
                |row| row.get(0),
            )
            .unwrap();
        assert!(caller_retained, "{launch}: Graph lost its original caller");
        let original: String = db
            .query_row("SELECT cwd FROM processes WHERE id=?1", [parent], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(Path::new(&original), caller);
        let inventory = task.run(&[
            "graph",
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
        let inventory: loopflow::durable::GraphProcessPage =
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

/// A headless Task requires an account record even when the provider is a
/// stand-in. Its synthetic credential never leaves the network sandbox.
fn connect_fixture_claude(home: &Path, store: &loopflow::store::Store) {
    let account_home = home.join("accounts/claude/fixture");
    fs::create_dir_all(&account_home).unwrap();
    let credential =
        r#"{"claudeAiOauth":{"accessToken":"fixture-only","expiresAt":4102444800000}}"#;
    fs::write(account_home.join(".credentials.json"), credential).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(store.upsert_provider_account(&ProviderAccount {
            provider: "claude".into(),
            account_id: ProviderAccountId::parse("fixture").unwrap(),
            home: Some(account_home),
            login_email: Some(
                loopflow::profile::EmailAddress::parse("fixture@example.com").unwrap(),
            ),
            observed_email: Some("fixture@example.com".into()),
            observed_subject: Some("fixture".into()),
            observed_credential_digest: Some(format!(
                "{:x}",
                Sha256::digest(credential.as_bytes())
            )),
            observed_plan: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        }))
        .unwrap();
}

/// A registered Task whose repository defines the Graphs and workflows below.
struct WorkflowTask {
    repo: TestRepo,
    home: tempfile::TempDir,
    registered: support::RegisteredTask,
    _env: support::EnvGuard,
}

impl WorkflowTask {
    fn new() -> Self {
        Self::with_tools(&[])
    }

    /// The fixture with these executables on PATH as well.
    fn with_tools(tools: &[(&str, &str)]) -> Self {
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
            (".lf/graphs/broken.yaml", "- cmd: graph show no-such-graph\n"),
            // Fails until the repository defines a Graph named `late`.
            (".lf/graphs/gate.yaml", "- cmd: graph show late\n"),
            // Another round succeeds; accepting fails until `late` exists.
            (
                ".lf/graphs/gated.yaml",
                "- graph: proof\n- loop: proof\n  interactive: {name: demo, id: review}\n- graph: gate\n- interactive: {name: demo, id: accepted}\n",
            ),
            // The Wave's default, `feature`, as this repository defines it.
            (
                ".lf/graphs/feature.yaml",
                "- graph: proof\n- interactive: {name: demo, id: review}\n",
            ),
            // No PR: the last route runs nothing.
            (
                ".lf/graphs/findings.yaml",
                "- graph: proof\n- interactive: {name: research, id: findings}\n",
            ),
            // An ordinary repeatable route returns to its review stage.
            (
                ".lf/graphs/rounds.yaml",
                "- graph: proof\n- graph: land-proof\n- loop: land-proof\n  interactive: {name: demo, id: review}\n- graph: broken\n",
            ),
        ] {
            let path = repo.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        // The Task's branch starts from these definitions and holds nothing.
        repo.stage_all();
        repo.commit("Define the fixture Graphs and graphs");
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
        fixture.select_workflow("feature");
        fixture
    }

    fn select_workflow(&self, name: &str) {
        let store =
            loopflow::store::sqlite::SqliteStore::new(&self.home.path().join("loopflow.db"))
                .unwrap();
        let content =
            fs::read_to_string(self.repo.path().join(format!(".lf/graphs/{name}.yaml"))).unwrap();
        store
            .select_wave_workflow(
                self.registered.task.wave_id.as_ref().unwrap(),
                name,
                &content,
            )
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
            .block_on(store.seed_issue(&scope, "linear", record, None, None))
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
    let scope = fixture
        .repo
        .path()
        .canonicalize()
        .unwrap()
        .display()
        .to_string();
    let sqlite =
        loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("loopflow.db"))
            .unwrap();
    let mut observation: crate::test_linear_input::IssueFixture =
        loopflow_test_support::retained_task_input(
            &fixture.registered.database,
            &scope,
            &fixture.registered.task.identifier,
        );
    observation.item.id = "unplaced-issue".into();
    observation.item.identifier = "INF-124".into();
    observation.item.name = "Workflow chosen at first run".into();
    observation.item.branch_name = None;
    runtime
        .block_on(store.seed_issue(&scope, "linear", observation, None, None))
        .unwrap();
    let task = runtime
        .block_on(store.get_task_by_issue("INF-124"))
        .unwrap()
        .unwrap();
    let select = |name: &str| {
        sqlite
            .select_wave_workflow(
                task.wave_id.as_ref().unwrap(),
                name,
                "- graph: proof\n- interactive: {name: demo, id: review}\n",
            )
            .unwrap();
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
fn a_task_takes_up_its_waves_workflow_and_keeps_one_it_named() {
    // Nothing named, no Workflow yet: the Wave default.
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
    // The edge's Graph is an ordinary Graph process; it chose the edge and,
    // having succeeded, wrote the arrival.
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 1);
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
    assert_eq!(history[1]["lf_process_id"], history[2]["lf_process_id"]);
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
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
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
    // The landing edge's Graph stops before its end: the Task stays on it.
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
        "workflow",
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
    // One Graph process reached the node; the failing edge was attempted three times.
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 4);
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
    let late = task.repo.path().join(".lf/graphs/late.yaml");
    fs::write(late, "- cmd: task sync --plan\n").unwrap();
    task.ok(&["-b", "task", "run", "INF-123", "gate"]);
    assert_eq!(task.workflow()["position"], at("accepted"));
    // The demo was not good enough after all: go back and take the loop.
    task.ok(&[
        "task",
        "workflow",
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
    let error = refusal(task.run(&["task", "workflow", "move", "INF-123", "nowhere"]));
    assert!(
        error.contains("Stages: start, review, accepted, end"),
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
fn task_edges_launch_graphs_named_after_collection_commands() {
    let task = WorkflowTask::new();
    fs::write(
        task.repo.path().join(".lf/graphs/list.yaml"),
        "- cmd: task sync --plan\n",
    )
    .unwrap();
    fs::write(
        task.repo.path().join(".lf/graphs/reserved.yaml"),
        "- graph: list\n- interactive: {name: demo, id: review}\n",
    )
    .unwrap();
    task.select_workflow("reserved");
    task.ok(&["-b", "task", "run", "INF-123", "reserved"]);
    assert_eq!(
        task.workflow()["position"],
        serde_json::json!({"kind":"node","node":"review"})
    );
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
}

#[test]
fn a_plain_graph_process_in_the_worktree_does_not_move_the_task() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    let before = task.workflow();
    task.ok(&["-b", "graph", "land-proof"]);
    task.ok(&["-b", "--task", "INF-123", "graph", "land-proof"]);
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 3);
    assert_eq!(task.workflow(), before);
    // Run as a plain Graph, a graph stops at its first stage and moves no Task.
    let staged = task.run(&["-b", "graph", "rounds"]);
    assert!(staged.status.success());
    let stderr = String::from_utf8_lossy(&staged.stderr).to_string();
    assert!(stderr.contains("Reached review"), "{stderr}");
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 4);
    assert_eq!(task.workflow(), before);
}

#[test]
fn a_task_takes_up_the_workflow_when_an_autonomous_graph_has_the_same_name() {
    let task = WorkflowTask::new();
    fs::write(
        task.repo.path().join(".lf/graphs/feature.yaml"),
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
    task.ok(&["-b", "graph", "feature"]);
    assert_eq!(task.workflow(), captured);
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
}

/// The Graph processes started beneath `task_run`, by outcome, oldest first.
fn attempts(home: &Path, task_run: &str) -> Vec<String> {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let mut rows = db
        .prepare(
            "SELECT d.outcome FROM graph_processes g JOIN processes d ON d.id=g.lf_process_id
             WHERE d.parent_lf_process_id=?1 ORDER BY d.rowid",
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
fn one_task_run_starts_its_graph_again_until_an_attempt_succeeds_or_attempts_run_out() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    let carrier = |workflow: &serde_json::Value| {
        let chose = workflow["history"].as_array().unwrap().last().unwrap();
        chose["lf_process_id"].as_str().unwrap().to_string()
    };
    // Every attempt fails: the Task run gives up and the edge holds the Task.
    let output = task.run(&["-b", "task", "run", "INF-123", "gate"]);
    let error = refusal(output);
    assert!(error.contains("Edge gate failed 3 times"), "{error}");
    let stopped = task.workflow();
    assert_eq!(stopped["position"]["edge"], 2);
    assert_eq!(stopped["position"]["running"], false);
    assert_eq!(
        stopped["position"]["lf_process_id"],
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
            "#!/bin/sh\ncase \"$*\" in *'graph show late'*)\n  echo asked >> '{count}'\n  [ \"$(wc -l < '{count}')\" -ge 3 ] && printf -- '- cmd: task sync --plan\\n' > '{late}';;\nesac\nexec '{real}' \"$@\"\n",
            count = bin.path().join("asked").display(),
            late = task.repo.path().join(".lf/graphs/late.yaml").display(),
            real = loopflow_test_support::lf_binary(),
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
    // One Task run: chosen once, arrived once, three Graph processes beneath it.
    let kinds: Vec<_> = moves(&workflow).into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(
        kinds,
        ["took_up", "chose", "arrived", "chose", "chose", "arrived"]
    );
    let arrived = workflow["history"].as_array().unwrap().last().unwrap();
    let task_run = arrived["lf_process_id"].as_str().unwrap();
    assert_eq!(
        attempts(task.home.path(), task_run),
        ["failed", "failed", "succeeded"]
    );
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 7);
}

#[test]
fn a_held_command_stops_the_task_run_without_repeating_the_graph() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    let bin = tempfile::tempdir().unwrap();
    let lf = bin.path().join("lf");
    fs::write(
        &lf,
        format!(
            "#!/bin/sh\ncase \"$*\" in *'graph show late'*) exit 3;; esac\nexec '{}' \"$@\"\n",
            loopflow_test_support::lf_binary()
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
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
    let workflow = task.workflow();
    assert_eq!(workflow["position"]["kind"], "edge");
    assert_eq!(workflow["position"]["running"], false);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("starting it again"));
}

#[test]
fn completion_is_independent_of_workflow_readiness() {
    let task = WorkflowTask::new();
    assert_eq!(task.state(), "unstarted");
    // A node, then a stopped edge: both between start and end.
    task.ok(&["-b", "task", "run", "INF-123", "gated"]);
    assert_eq!(task.workflow()["position"], at("review"));
    assert_eq!(task.state(), "working");
    assert!(!task
        .run(&["-b", "task", "run", "INF-123", "gate"])
        .status
        .success());
    assert_eq!(task.workflow()["position"]["kind"], "edge");
    assert_eq!(task.state(), "working");
    task.ok(&["task", "workflow", "move", "INF-123", "start"]);
    assert_eq!(task.state(), "unstarted");
    // Moving away from end does not reopen a completed Task.
    task.finish_after_provider_observation(&[
        "task",
        "workflow",
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
    task.ok(&["task", "workflow", "move", "INF-123", "review"]);
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
    task.finish_after_provider_observation(&["task", "workflow", "move", "INF-123", "end"]);
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
        &["task", "workflow", "move", "INF-123", "end"],
        &["task", "complete", "INF-123"],
    ] {
        let error = refusal(task.run(reach));
        assert!(error.to_lowercase().contains("pull request"), "{error}");
        assert_eq!(task.workflow()["position"], at("end"));
        // The route into `end` handed it off; it stays shipping until it completes.
        assert_eq!(task.state(), "shipping");
    }
}

#[test]
fn local_reopening_preserves_delivery_and_workflow_and_supersedes_pending_completion() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    task.publish(false);
    assert!(!task
        .run(&["task", "workflow", "move", "INF-123", "end"])
        .status
        .success());
    assert!(!task.status()["completion_pending"].is_null());
    let workflow = task.workflow();
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
    assert_eq!(task.workflow(), workflow);
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
    task.ok(&["task", "workflow", "move", "INF-123", "end"]);
    assert!(task.status()["completion_pending"].is_null());
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    task.ok(&["task", "reopen", "INF-123"]);
    assert_eq!(task.state(), "working");
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
    task.ok(&["task", "workflow", "move", "INF-123", "end"]);
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
    let mut record: crate::test_linear_input::IssueFixture =
        loopflow_test_support::retained_task_input(&task.registered.database, &scope, "INF-123");
    let stale = record.clone();
    record.item.revision = Some("2026-10-07T12:00:00Z".into());
    record.item.state = Some("started".into());
    record.item.completed = false;
    rt.block_on(store.seed_issue(&scope, "linear", record, None, None))
        .unwrap();
    rt.block_on(store.seed_issue(&scope, "linear", stale, None, None))
        .unwrap();
    assert_eq!(task.state(), "working");
    assert_eq!(task.workflow(), ended);
    assert!(task.status()["completion_pending"].is_null());
    task.ok(&["task", "workflow", "move", "INF-123", "end"]);
    assert_eq!(task.state(), "working");
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
    assert!(support::recorded_graphs(task.home.path()).is_empty());
    assert_eq!(task.state(), "done");
}

#[test]
fn workflow_restart_keeps_the_captured_graph_and_execution_history() {
    let task = WorkflowTask::new();
    task.ok(&["-b", "task", "run", "INF-123", "rounds"]);
    let before = task.workflow();
    let processes = support::recorded_graphs(task.home.path());
    fs::write(
        task.repo.path().join(".lf/graphs/rounds.yaml"),
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
    assert_eq!(support::recorded_graphs(task.home.path()), processes);
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
    assert!(task.workflow().is_null());
    for args in [
        &["task", "complete", "INF-123"][..],
        &["task", "workflow", "move", "INF-123", "end"][..],
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
    let task = WorkflowTask::new();
    task.publish(true);
    task.ok(&["-b", "task", "run", "INF-123", "findings"]);
    assert_ne!(task.state(), "done");
    assert!(task.repo.path().exists());
    task.finish_after_provider_observation(&["task", "complete", "INF-123"]);
    let workflow = task.workflow();
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.workflow(), workflow);
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
    let task = WorkflowTask::new();
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
    task.select_workflow("delivery");
    let error = refusal(task.run(&["-b", "task", "run", "INF-123", "delivery"]));
    assert!(task.status()["completion_pending"].is_string(), "{error}");
    task.publish(true);
    task.ok(&["task", "complete", "INF-123"]);
    task.ok(&["task", "workflow", "move", "INF-123", "end"]);
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
    let graphs = support::recorded_graphs(task.home.path());
    assert!(!graphs.is_empty());
    assert!(graphs
        .iter()
        .all(|graph| graph.0.as_deref() == Some("failed")));
}

#[test]
fn provider_completion_preserves_a_live_edge_and_the_graph_process_records_its_real_arrival() {
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
    write(".lf/graphs/live-workflow.yaml", "- graph: live\n");
    write(
        ".lf/graphs/live.yaml",
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
        "wave",
        "workflow",
        "set",
        "live-workflow",
        task.registered.task.wave_id.as_ref().unwrap().as_str(),
        "--file",
        worktree
            .join(".lf/graphs/live-workflow.yaml")
            .to_str()
            .unwrap(),
    ]);
    let log_path = task.home.path().join("live.log");
    let log = fs::File::create(&log_path).unwrap();
    let mut held = HeldGraph {
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
    task.publish(false);
    let placement = task.status();
    assert!(placement["pr"]["id"].is_string());
    task.ok(&["wave", "ensure", "other", "--json"]);
    task.ok(&["task", "move", "INF-123", "--wave", "other"]);
    let moved = task.status();
    assert_eq!(moved["wave"], "other");
    assert_eq!(moved["pr"], placement["pr"]);
    assert_eq!(moved["worktree"], placement["worktree"]);
    assert_eq!(moved["branch"], placement["branch"]);
    assert_eq!(task.workflow(), before);
    assert!(held.child.try_wait().unwrap().is_none());
    task.ok(&["task", "move", "INF-123", "--no-wave"]);
    assert_eq!(task.status()["pr"], placement["pr"]);
    assert_eq!(task.workflow(), before);
    assert!(held.child.try_wait().unwrap().is_none());
    task.ok(&[
        "task",
        "move",
        "INF-123",
        "--wave",
        task.registered.task.wave_id.as_ref().unwrap().as_str(),
    ]);
    assert_eq!(task.workflow(), before);
    assert_eq!(task.status()["worktree"], placement["worktree"]);
    assert_eq!(task.status()["branch"], placement["branch"]);
    assert_eq!(task.status()["pr"], placement["pr"]);
    assert!(held.child.try_wait().unwrap().is_none());
    task.complete_in_linear();
    assert_eq!(task.state(), "done");
    assert_eq!(task.workflow(), before);
    task.ok(&["task", "reopen", "INF-123"]);
    assert_eq!(task.state(), "working");
    assert_eq!(task.workflow(), before);
    assert!(held.child.try_wait().unwrap().is_none());
    assert!(task.repo.path().exists());
    task.publish(true);
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
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 1);
    assert_eq!(
        support::recorded_graphs(task.home.path())[0].0.as_deref(),
        Some("succeeded")
    );
}

#[test]
fn successful_final_graph_keeps_arrival_when_completion_fails_and_never_replays() {
    let task = WorkflowTask::new();
    task.repo
        .create_file(".lf/graphs/final.yaml", "- graph: proof\n");
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
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].0.as_deref(), Some("succeeded"));
    task.publish(true);
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.workflow(), arrived);
    assert_eq!(support::recorded_graphs(task.home.path()), graphs);
}

/// A Claude stand-in that records each first turn and, while `hold` exists in
/// LF_HOME, waits before answering.
const STAND_IN_CLAUDE: &str = r#"#!/usr/bin/python3
import json, os, sys, time
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

fn shipping_task() -> WorkflowTask {
    let task = WorkflowTask::with_tools(&[("claude", STAND_IN_CLAUDE)]);
    connect_fixture_claude(task.home.path(), &task.registered.store);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(
            task.registered
                .store
                .set_task_agent(&task.registered.task.id, "claude"),
        )
        .unwrap();
    task
}

fn stand_ins(task: &WorkflowTask) -> Vec<String> {
    fs::read_to_string(task.home.path().join("stand-ins.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The Task history lines that record a handoff to shipping.
fn handoffs(task: &WorkflowTask) -> Vec<String> {
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
            loopflow::work::task::TaskEventKind::Progress { summary }
                if summary.starts_with("Shipping: ") =>
            {
                Some(summary)
            }
            _ => None,
        })
        .collect()
}

fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(90);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn task_ship_runs_every_remaining_step_standing_in_at_each_node() {
    let task = shipping_task();
    let output = task.run(&["-b", "--task", "INF-123", "ship", "--reason", "good to go"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("INF-123 reached its end"));
    let workflow = task.workflow();
    assert_eq!(workflow["position"], at("end"));
    assert_eq!(
        moves(&workflow),
        [
            ("took_up".into(), None),
            ("chose".into(), Some(0)),
            ("arrived".into(), Some(0)),
            ("chose".into(), Some(1)),
        ]
    );
    let handoff = handoffs(&task);
    assert_eq!(handoff.len(), 1);
    assert!(handoff[0].ends_with(": good to go"), "{}", handoff[0]);
    // The review node ran headless with its skill, standing in for a person.
    let turns = stand_ins(&task);
    assert_eq!(turns.len(), 1);
    assert!(turns[0].contains("You are standing in for"), "{}", turns[0]);
    assert!(turns[0].contains("at review"), "{}", turns[0]);
    // Reaching the end completed the Task; nothing remains to ship.
    assert_eq!(task.state(), "done");
    let error = refusal(task.run(&["-b", "--task", "INF-123", "ship"]));
    assert!(error.contains("INF-123 is complete"), "{error}");
}

#[test]
fn task_ship_holds_while_paused_and_defers_to_a_live_ship() {
    let task = shipping_task();
    task.ok(&["task", "pause", "INF-123"]);
    let paused = task.run(&["-b", "--task", "INF-123", "ship"]);
    assert!(paused.status.success());
    assert!(String::from_utf8_lossy(&paused.stdout).contains("INF-123 is paused"));
    assert!(task.workflow().is_null());
    task.ok(&["task", "resume", "INF-123"]);

    // The first ship holds at its stand-in; a second one finds it live.
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "ship"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    let second = task.run(&["-b", "--task", "INF-123", "ship"]);
    assert!(second.status.success());
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("INF-123 is already shipping"),
        "{}",
        String::from_utf8_lossy(&second.stdout)
    );
    fs::remove_file(task.home.path().join("hold")).unwrap();
    assert!(first.wait().unwrap().success());
    assert_eq!(task.workflow()["position"], at("end"));
    assert_eq!(handoffs(&task).len(), 1);
}

#[test]
fn task_ship_stop_returns_the_task_to_working_and_ends_its_ship() {
    let task = shipping_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "ship"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    wait_for("the stand-in", || !stand_ins(&task).is_empty());
    assert_eq!(task.state(), "shipping");
    let stopped = task.run(&["--task", "INF-123", "ship", "--stop"]);
    let stopped = String::from_utf8_lossy(&stopped.stdout);
    assert!(stopped.contains("INF-123 stopped shipping"), "{stopped}");
    assert!(!first.wait().unwrap().success());
    fs::remove_file(task.home.path().join("hold")).unwrap();
    assert_eq!(task.state(), "working");
    let again = task.run(&["--task", "INF-123", "ship", "--stop"]);
    let again = String::from_utf8_lossy(&again.stdout);
    assert!(again.contains("INF-123 is not shipping"), "{again}");
}

#[test]
fn task_ship_restarts_from_reconcile_after_its_process_dies() {
    let task = shipping_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let mut first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "ship"],
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
    // The schedule's minute check finds the dead ship and starts it again.
    let _ = task.run(&["task", "reconcile"]);
    wait_for("the restarted ship to reach end", || {
        task.workflow()["position"] == at("end")
    });
    // The restart carries the same handoff; it is not shipped a second time.
    assert_eq!(handoffs(&task).len(), 1);
}
