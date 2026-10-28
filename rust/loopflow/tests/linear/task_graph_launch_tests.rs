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
fn explicit_launch_policy_survives_task_runs_nested_graphs_and_provider_children() {
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
    support::select_wave_graph(
        repo.path(),
        &store,
        registered.task.wave_id.as_ref().unwrap(),
        "feature",
    );
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
        assert_eq!(
            recorded[0].1[0]["argv"],
            serde_json::json!(["ops", "sync", "--plan"])
        );
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
    let (cwd, outcome): (String, String) = db
        .query_row(
            "SELECT cwd,outcome FROM processes WHERE parent_lf_process_id=?1",
            [&parent.id],
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

/// A held Graph step: an ordinary owned execution running this script, which
/// optionally announces `ready` in the home and waits there for `release`.
fn hold_script(home: &Path, announce: bool) -> PathBuf {
    let script = home.join("hold.py");
    let ready = if announce {
        "(home / 'ready.tmp').write_text('ready')\n(home / 'ready.tmp').replace(home / 'ready')\n"
    } else {
        ""
    };
    fs::write(
        &script,
        format!(
            "import os\nfrom pathlib import Path\nimport time\nhome = Path(os.environ['LF_HOME'])\n{ready}while not (home / 'release').exists():\n    time.sleep(.02)\n"
        ),
    )
    .unwrap();
    script
}

#[test]
fn running_graphs_belong_to_the_target_checkout_and_retain_their_caller() {
    for launch in ["task", "direct", "checkout", "alias", "wt"] {
        let task = GraphTask::new();
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
        let hold = hold_script(task.home.path(), true);
        task.repo.create_file(
            ".lf/graphs/proof.yaml",
            &format!("- cmd: session serve -- python3 {}\n", hold.display()),
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

#[test]
fn task_run_returns_while_its_graph_keeps_running() {
    let task = GraphTask::new();
    let hold = hold_script(task.home.path(), false);
    task.repo.create_file(
        ".lf/graphs/held.yaml",
        &format!(
            "- cmd: session serve -- python3 {}\n- cmd: task sync --plan\n",
            hold.display()
        ),
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
        .split("Graph process ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("no Graph process named: {printed}"))
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
            "SELECT count(*) FROM processes WHERE parent_lf_process_id=?1",
            [&graph],
            |row| row.get(0),
        )
        .unwrap();
    assert!(steps >= 2, "the second step never ran: {steps}");
    // The owner writes the arrival after its Graph exits.
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

/// A headless Task requires an account record even when the provider is a
/// stand-in. Its synthetic credential never leaves the network sandbox.
fn connect_fixture_claude(home: &Path, store: &loopflow::store::Store) {
    connect_claude_account(home, store, "fixture");
}

/// Connect a managed Claude account `name`, signed in as `name@example.com`.
fn connect_claude_account(home: &Path, store: &loopflow::store::Store, name: &str) {
    let account_home = home.join("accounts/claude").join(name);
    fs::create_dir_all(&account_home).unwrap();
    let credential =
        r#"{"claudeAiOauth":{"accessToken":"fixture-only","expiresAt":4102444800000}}"#;
    fs::write(account_home.join(".credentials.json"), credential).unwrap();
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(store.upsert_provider_account(&ProviderAccount {
            provider: "claude".into(),
            account_id: ProviderAccountId::parse(name).unwrap(),
            home: Some(account_home),
            login_email: Some(
                loopflow::profile::EmailAddress::parse(&format!("{name}@example.com")).unwrap(),
            ),
            observed_email: Some(format!("{name}@example.com")),
            observed_subject: Some(name.into()),
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

/// A Claude stand-in that records each launch's arguments and account home.
const CLAUDE_RECORDER: &str = r#"#!/usr/bin/python3
import json
import os
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
print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'agent-fixture', 'subtype': 'success', 'is_error': False, 'result': 'done'}), flush=True)
for line in sys.stdin:
    pass
"#;

#[test]
fn a_task_run_takes_its_waves_agent_and_account_without_flags() {
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
    let registered = support::register_task_without_pr(
        home.path(),
        &repo.path().canonicalize().unwrap(),
        "wave-setup",
        &repo.head_sha(),
    );
    connect_claude_account(home.path(), &registered.store, "fixture");
    connect_claude_account(home.path(), &registered.store, "work");
    for (path, content) in [
        (".lf/skills/probe.md", "Probe."),
        (".lf/graphs/proof.yaml", "- step: {name: probe}\n"),
        (
            ".lf/wave/task-pr-tests/config.yaml",
            "linear:\n  initiative: initiative-task-pr-tests\nagent: claude:opus\n",
        ),
    ] {
        repo.create_file(path, content);
    }
    repo.stage_all();
    repo.commit("Set up the Wave once");
    for args in [
        &[
            "--wave",
            "task-pr-tests",
            "set",
            "account.claude",
            "work@example.com",
        ][..],
        &["-b", "task", "run", "INF-123", "proof"],
    ] {
        // Isolated launches run in the routed account's own home; a shared one
        // would sign in the native home, which is the Keychain on macOS.
        let output = command(repo.path(), home.path(), args)
            .env("LF_ACCOUNT_ISOLATION", "isolated")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let launches = fs::read_to_string(home.path().join("launches.jsonl")).unwrap();
    let launch: serde_json::Value = serde_json::from_str(launches.lines().next().unwrap()).unwrap();
    let args = launch["args"].as_array().unwrap();
    assert!(
        args.windows(2).any(|pair| pair == ["--model", "opus"]),
        "{launch}"
    );
    assert_eq!(
        Path::new(launch["claude_home"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        home.path()
            .join("accounts/claude/work")
            .canonicalize()
            .unwrap()
    );
}

/// A registered Task whose repository defines the Graphs below.
struct GraphTask {
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
                "- graph: proof\n- interactive: {name: demo, id: review}\n- graph: land-proof\n- interactive: {name: verify, id: check}\n",
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
            repo,
            home,
            registered,
            _env: env,
        };
        fixture.ok(&["task", "checkout", "INF-123"]);
        fixture
    }

    fn select_wave_graph(&self, name: &str) {
        let store =
            loopflow::store::sqlite::SqliteStore::new(&self.home.path().join("loopflow.db"))
                .unwrap();
        support::select_wave_graph(
            self.repo.path(),
            &store,
            self.registered.task.wave_id.as_ref().unwrap(),
            name,
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
        let status = self.run(&["task", "status", "INF-123", "--json"]);
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

    /// Run `lf task run …` and wait for the run it started to stop.
    fn task_run(&self, args: &[&str]) -> Ran {
        self.ran(self.run(args))
    }

    /// `lf task run …` that must succeed, waited.
    fn task_run_ok(&self, args: &[&str]) {
        let ran = self.task_run(args);
        assert!(ran.success(), "{args:?}: {}", ran.text);
    }

    /// Wait for the run a returned `lf task run` printed, if it printed one:
    /// until its carrier, the Graph process's parent, has exited.
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
        let (Some(graph), Some(log)) = (printed("running as Graph process "), printed("log:    "))
        else {
            return Ran {
                code: output.status.code(),
                text,
            };
        };
        let db = rusqlite::Connection::open(self.home.path().join("loopflow.db")).unwrap();
        let carrier: String = db
            .query_row(
                "SELECT parent_lf_process_id FROM processes WHERE id=?1",
                [&graph],
                |row| row.get(0),
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        let code = loop {
            let (done, code): (bool, Option<i32>) = db
                .query_row(
                    "SELECT completed_at IS NOT NULL, exit_code FROM processes WHERE id=?1",
                    [&carrier],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            if done {
                break code;
            }
            assert!(
                Instant::now() < deadline,
                "the run never stopped: {text}\n{}",
                fs::read_to_string(&log).unwrap_or_default()
            );
            std::thread::sleep(Duration::from_millis(50));
        };
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
    observation.item.name = "Graph chosen at first run".into();
    observation.item.branch_name = None;
    runtime
        .block_on(store.seed_issue(&scope, "linear", observation, None, None))
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
    assert!(sqlite.task_work(&task.id).unwrap().graph.is_none());
    select("at-first-run");
    fixture.task_run_ok(&["task", "run", "INF-124"]);
    let captured = sqlite.task_work(&task.id).unwrap().graph.unwrap();
    assert_eq!(captured.name, "at-first-run");
    assert_eq!(
        serde_json::to_value(&captured.position).unwrap(),
        at("review")
    );
    select("after-first-run");
    fixture.task_run_ok(&["task", "run", "INF-124"]);
    let retained = sqlite.task_work(&task.id).unwrap().graph.unwrap();
    assert_eq!(retained.name, captured.name);
    assert_eq!(serde_json::to_value(&retained.position).unwrap(), at("end"));
}

#[test]
fn a_task_takes_up_its_waves_graph_else_the_repositorys_else_code() {
    let explain = |task: &GraphTask, args: &[&str]| -> serde_json::Value {
        let preview = task.run(args);
        assert!(
            preview.status.success(),
            "{}",
            String::from_utf8_lossy(&preview.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&preview.stdout).unwrap()["action"].clone()
    };
    // Nothing named, no graph yet: the Wave default, stopping at its review.
    let task = GraphTask::new();
    let action = explain(&task, &["task", "run", "--explain", "--json"]);
    assert_eq!(
        (&action["kind"], &action["graph"], &action["take_up"]),
        (&"run".into(), &"feature".into(), &true.into())
    );
    assert_eq!(action["stops_at"], "review");
    // `-b` runs straight past it to the end.
    let action = explain(&task, &["-b", "task", "run", "--explain", "--json"]);
    assert!(action["stops_at"].is_null(), "{action}");
    task.task_run_ok(&["task", "run"]);
    let graph = task.graph();
    assert_eq!(graph["name"], "feature");
    assert_eq!(graph["position"], at("review"));
    assert_eq!(graph["interactive_steps"][0]["skill"], "demo");
    drop(task);

    // No Wave default: the repository's `graph:`, else the builtin `code`.
    let task = GraphTask::without_wave_graph(&[]);
    let action = explain(&task, &["task", "run", "--explain", "--json"]);
    assert_eq!(
        (&action["graph"], &action["take_up"]),
        (&"code".into(), &true.into())
    );
    let config = task.repo.path().join(".lf/config.yaml");
    fs::write(
        &config,
        format!("{}graph: findings\n", fs::read_to_string(&config).unwrap()),
    )
    .unwrap();
    let action = explain(&task, &["task", "run", "--explain", "--json"]);
    assert_eq!(action["graph"], "findings");
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
fn a_run_explanation_includes_interactive_steps_inside_its_first_xor() {
    let task = GraphTask::new();
    fs::write(
        task.repo.path().join(".lf/graphs/conditional.yaml"),
        "- xor:\n    paths:\n      review:\n        description: Review it\n        steps:\n          - interactive: demo\n      skip:\n        description: Nothing to review\n",
    )
    .unwrap();
    let output = task.run(&[
        "task",
        "run",
        "INF-123",
        "conditional",
        "--explain",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let preview: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(preview["action"]["stops_at"], "demo");
    assert!(support::recorded_graphs(task.home.path()).is_empty());
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
    assert!(
        task.text_status().contains("Graph findings: at findings"),
        "{}",
        task.text_status()
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
    assert!(task.text_status().contains("Graph findings: at end"));
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
fn a_stopped_run_is_started_again_from_where_it_began() {
    let task = GraphTask::new();
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    // Every attempt fails: the Task run gives up and the run holds the Task.
    let ran = task.task_run(&["task", "run", "INF-123"]);
    assert!(!ran.success());
    assert!(ran.text.contains("failed 3 times"), "{}", ran.text);
    let stopped = task.graph();
    assert_eq!(stopped["position"]["kind"], "run");
    assert_eq!(stopped["position"]["step"], "review");
    assert_eq!(stopped["position"]["running"], false);
    let carrier = stopped["position"]["lf_process_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        attempts(task.home.path(), &carrier),
        ["failed", "failed", "failed"]
    );
    let status = task.text_status();
    assert!(
        status.contains("Graph gated: stopped from review"),
        "{status}"
    );

    // The cause clears as the third attempt starts: an `lf` that defines
    // `late` the third time the gate asks for it.
    let bin = tempfile::tempdir().unwrap();
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
    // The bare run started the stopped run again from the gate, not after review.
    assert_eq!(task.graph()["position"], at("accepted"));
    // The carrier of the latest Graph process is the run that just ended.
    let task_run = rusqlite::Connection::open(task.home.path().join("loopflow.db"))
        .unwrap()
        .query_row(
            "SELECT p.parent_lf_process_id FROM graph_processes g JOIN processes p ON p.id=g.lf_process_id ORDER BY p.rowid DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(
        attempts(task.home.path(), &task_run),
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
    let carrier = task.graph()["position"]["lf_process_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(attempts(task.home.path(), &carrier).len(), 1);
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
fn interrupting_a_task_with_only_an_open_process_records_it_without_claiming_a_stop() {
    let task = GraphTask::new();
    let mut waiter = command(
        task.repo.path(),
        task.home.path(),
        &["task", "wait", "INF-123", "--timeout", "60s"],
    )
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn()
    .unwrap();
    let db = task.home.path().join("loopflow.db");
    let open = |db: &Path| -> i64 {
        rusqlite::Connection::open(db)
            .ok()
            .and_then(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM processes WHERE completed_at IS NULL AND command LIKE '%\"wait\"%'",
                    [],
                    |row| row.get(0),
                )
                .ok()
            })
            .unwrap_or(0)
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while open(&db) == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let before = interrupt_rows(task.home.path());
    let output = task.run(&["task", "interrupt", "INF-123"]);
    waiter.kill().unwrap();
    waiter.wait().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("interrupt recorded for Process"),
        "{stdout}"
    );
    assert!(!stdout.contains("stopped"), "{stdout}");
    assert_eq!(interrupt_rows(task.home.path()), before + 1);
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

    let ran = task.task_run(&["task", "run", "INF-123"]);
    assert!(!ran.success());
    assert!(ran.text.contains(stash.trim()), "{}", ran.text);
    assert!(!ran.text.contains("); starting it again"), "{}", ran.text);
    let carrier = task.graph()["position"]["lf_process_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(attempts(task.home.path(), &carrier), ["failed"]);
}

#[test]
fn completion_is_independent_of_graph_position() {
    let task = GraphTask::new();
    assert_eq!(task.state(), "idle");
    // At an interactive step, then a stopped run: both between start and end.
    task.task_run_ok(&["task", "run", "INF-123", "gated"]);
    assert_eq!(task.graph()["position"], at("review"));
    assert_eq!(task.state(), "active");
    assert!(!task.task_run(&["task", "run", "INF-123"]).success());
    assert_eq!(task.graph()["position"]["kind"], "run");
    assert_eq!(task.state(), "idle");
    // Completing it leaves it where it stood.
    let stopped = task.graph();
    task.finish_after_provider_observation(&["task", "complete", "INF-123"]);
    assert_eq!(task.state(), "done");
    assert_eq!(task.graph(), stopped);
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
    // A plain run into end hands the Task to Auto-Ship, which may land it.
    let stop = task.run(&["--task", "INF-123", "ship", "--stop"]);
    let stop = String::from_utf8_lossy(&stop.stdout);
    assert!(stop.contains("INF-123 stopped shipping"), "{stop}");
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
    rt.block_on(store.seed_issue(&scope, "linear", record, None, None))
        .unwrap();
    rt.block_on(store.seed_issue(&scope, "linear", stale, None, None))
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
    let task = GraphTask::new();
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
    let hold = hold_script(task.home.path(), true);
    write(
        ".lf/graphs/live.yaml",
        &format!(
            "- cmd: session serve -- python3 {}\n- cmd: task complete INF-123\n",
            hold.display()
        ),
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
    while !task.home.path().join("ready").exists() {
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
    task.ok(&["wave", "ensure", "other", "--json"]);
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

fn shipping_task() -> GraphTask {
    let task = GraphTask::with_tools(&[("claude", STAND_IN_CLAUDE)]);
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

fn stand_ins(task: &GraphTask) -> Vec<String> {
    fs::read_to_string(task.home.path().join("stand-ins.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The Task history lines that record a handoff to shipping.
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
fn a_batch_task_run_runs_straight_through_its_interactive_steps_to_the_end() {
    let task = shipping_task();
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
}

#[test]
fn task_ship_runs_every_remaining_step_standing_in_at_each_interactive_step() {
    let task = shipping_task();
    let output = task.run(&["-b", "--task", "INF-123", "ship", "--reason", "good to go"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("INF-123 reached its end"));
    assert_eq!(task.graph()["position"], at("end"));
    let handoff = handoffs(&task);
    assert_eq!(handoff.len(), 1);
    assert!(handoff[0].ends_with(": good to go"), "{}", handoff[0]);
    // The review step ran headless with its skill, standing in for a person.
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
fn task_ship_stands_in_at_the_step_the_task_waits_at() {
    let task = shipping_task();
    task.task_run_ok(&["task", "run", "INF-123"]);
    assert_eq!(task.graph()["position"], at("review"));
    let output = task.run(&["-b", "--task", "INF-123", "ship"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("INF-123: standing in at review"),
        "{stderr}"
    );
    assert_eq!(task.graph()["position"], at("end"));
    // Proof is not run again: the ship set out from review.
    assert_eq!(support::recorded_graphs(task.home.path()).len(), 2);
    assert_eq!(stand_ins(&task).len(), 1);
    assert_eq!(task.state(), "done");
}

#[test]
fn task_ship_holds_while_paused_and_defers_to_a_live_ship() {
    let task = shipping_task();
    task.ok(&["task", "defer", "INF-123"]);
    let deferred = task.run(&["-b", "--task", "INF-123", "ship"]);
    assert!(deferred.status.success());
    assert!(String::from_utf8_lossy(&deferred.stdout).contains("INF-123 is deferred"));
    assert!(task.graph().is_null());
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
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(handoffs(&task).len(), 1);
}

#[test]
fn task_ship_stop_withdraws_the_approval_and_ends_its_ship() {
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
    // Approval withdrawn, its run stopped where it stood.
    assert_ne!(task.state(), "shipping");
    let again = task.run(&["--task", "INF-123", "ship", "--stop"]);
    let again = String::from_utf8_lossy(&again.stdout);
    assert!(again.contains("INF-123 is not shipping"), "{again}");
}

#[test]
fn task_ship_completes_when_its_pr_merges_mid_ship() {
    let task = shipping_task();
    fs::write(task.home.path().join("hold"), "").unwrap();
    let first = command(
        task.repo.path(),
        task.home.path(),
        &["-b", "--task", "INF-123", "ship"],
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
    // Review is this graph's last step, so it checks merged work: the merge
    // completes nothing until the stand-in there finishes into `end`.
    assert_eq!(task.state(), "done");
    assert_eq!(task.graph()["position"], at("end"));
    assert_eq!(stand_ins(&task).len(), 1);
}

#[test]
fn task_ship_takes_a_merged_pr_on_to_its_check_step_instead_of_building() {
    let task = shipping_task();
    task.task_run_ok(&["task", "run", "INF-123", "verified"]);
    assert_eq!(task.graph()["position"], at("review"));
    // Someone merged the PR by hand while the Task waited at review.
    task.publish(true);
    let output = task.run(&["-b", "--task", "INF-123", "ship"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("INF-123: its PR merged; on to check"),
        "{stdout}"
    );
    assert_eq!(task.state(), "done");
    assert_eq!(task.graph()["position"], at("end"));
    // The landing steps never ran; a stand-in checked the merged work at
    // `check`, then finished.
    let turns = stand_ins(&task);
    assert_eq!(turns.len(), 1, "{turns:?}");
    assert!(turns[0].contains("at check"), "{}", turns[0]);
}

#[test]
fn task_ship_restarts_on_the_next_read_after_its_process_dies() {
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
    // Any read of this Machine's Tasks finds the dead ship and starts it again.
    task.ok(&["task", "list"]);
    wait_for("the restarted ship to reach end", || {
        task.graph()["position"] == at("end")
    });
    // The restart carries the same handoff; it is not shipped a second time.
    assert_eq!(handoffs(&task).len(), 1);
}

/// A Claude stand-in that records its first turn and fails at once, the way an
/// expired login does.
const FAILING_CLAUDE: &str = r#"#!/usr/bin/python3
import json, os, sys
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = os.environ['LF_HOME']
message = json.loads(sys.stdin.readline())
with open(os.path.join(home, 'stand-ins.jsonl'), 'a') as turns:
    turns.write(json.dumps(message['message']['content'][0]['text']) + '\n')
sys.exit(3)
"#;

#[test]
fn task_ship_restarts_after_a_failure_wait_instead_of_on_every_read() {
    let task = GraphTask::with_tools(&[("claude", FAILING_CLAUDE)]);
    connect_fixture_claude(task.home.path(), &task.registered.store);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(
            task.registered
                .store
                .set_task_agent(&task.registered.task.id, "claude"),
        )
        .unwrap();
    let output = task.run(&["-b", "--task", "INF-123", "ship"]);
    assert!(!output.status.success());
    // The ship's Task run started its failing Graph three times, then gave up.
    let attempts = stand_ins(&task).len();
    assert_eq!(attempts, 3);
    // Approved with its ship process exited, it is idle until the restart.
    assert_eq!(task.state(), "idle");
    // Reads right after the failure leave the restart waiting its minute.
    for _ in 0..3 {
        task.ok(&["task", "list"]);
    }
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(stand_ins(&task).len(), attempts);
    assert_eq!(task.state(), "idle");
    // The shipping reason names the stand-in's failure and the wait.
    let shown = task.run(&["wave", "show", "--json"]);
    let shown = String::from_utf8_lossy(&shown.stdout);
    let reason = shown
        .split("\"reason\":\"Ship failed: ")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("no failed shipping reason in {shown}"));
    assert!(reason.starts_with("agent exited with code 1"), "{reason}");
    assert!(reason.ends_with("; retrying within a minute"), "{reason}");
}

#[test]
fn task_ship_runs_new_work_on_a_reopened_task_despite_its_old_merged_pr() {
    let task = shipping_task();
    // The first delivery merged and completed the Task; then it was reopened.
    task.publish(true);
    task.ok(&["task", "complete", "INF-123"]);
    assert_eq!(task.state(), "done");
    task.ok(&["task", "reopen", "INF-123", "--reason", "new repair"]);
    assert_ne!(task.state(), "done");
    let output = task.run(&["-b", "--task", "INF-123", "ship", "--graph", "feature"]);
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
print(json.dumps({'type': 'result', 'uuid': 'stand-in-result', 'session_id': 'stand-in', 'subtype': 'success', 'is_error': False, 'result': summary, 'structured_output': {'decision': 'blocked', 'summary': None, 'reason': summary}}), flush=True)
for line in sys.stdin:
    pass
"#;

#[test]
fn task_ship_blocked_at_verification_waits_there_without_building_again() {
    let task = GraphTask::with_tools(&[("claude", BLOCKED_CLAUDE)]);
    connect_fixture_claude(task.home.path(), &task.registered.store);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(
            task.registered
                .store
                .set_task_agent(&task.registered.task.id, "claude"),
        )
        .unwrap();
    task.repo.create_file(
        ".lf/graphs/verifying.yaml",
        "- graph: proof\n- loop: proof\n  interactive: {name: verify, id: check}\n",
    );
    let first = task.run(&["-b", "--task", "INF-123", "ship", "--graph", "verifying"]);
    assert!(
        !first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    // The stand-in at check reported a dependency: the Task waits at check.
    assert_eq!(
        task.graph()["position"],
        at("check"),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(stand_ins(&task).len(), 1);
    let again = task.run(&["-b", "--task", "INF-123", "ship"]);
    let stderr = String::from_utf8_lossy(&again.stderr);
    assert!(!again.status.success(), "{stderr}");
    assert!(stderr.contains("INF-123: standing in at check"), "{stderr}");
    assert_eq!(task.graph()["position"], at("check"));
    assert_eq!(stand_ins(&task).len(), 2);
    // The second run set out from check: proof did not run again.
    let graphs = support::recorded_graphs(task.home.path());
    assert_eq!(graphs.len(), 2);
    assert_eq!(graphs[1].1.len(), 1, "{graphs:?}");
}
