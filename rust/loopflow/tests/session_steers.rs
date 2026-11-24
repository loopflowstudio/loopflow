//! Public launch proofs for the Session delivery cutover.
use crate::support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use loopflow_test_support::{MachineServer, TestRepo};

/// A bare skill, with optional Task attribution. The stand-in checks context
/// hooks and claims a steer through its tool hook without a Machine server.
fn launch_startup_proof(
    repo: &TestRepo,
    home: &std::path::Path,
    task: Option<&str>,
) -> std::process::Output {
    launch_startup_proof_through(repo, home, env!("CARGO_BIN_EXE_lf").as_ref(), task)
}

/// The same launch, with `lf_bin` as the `lf` it runs for its own commands.
fn launch_startup_proof_through(
    repo: &TestRepo,
    home: &std::path::Path,
    lf_bin: &std::path::Path,
    task: Option<&str>,
) -> std::process::Output {
    let bin = home.join("bin");
    fs::create_dir(&bin).unwrap();
    let skill = repo.path().join(".claude/skills/startup-proof");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: startup-proof\ndescription: Startup fixture\n---\nReturn the fixture answer.\n",
    )
    .unwrap();
    let clipboard = format!("CLIPBOARD_START{}CLIPBOARD_END", "界".repeat(50_000));
    fs::write(home.join("clipboard"), &clipboard).unwrap();
    for name in ["pbpaste", "xclip", "xsel"] {
        let path = bin.join(name);
        fs::write(&path, "#!/bin/sh\ncat \"$LF_HOME/clipboard\"\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let provider = bin.join("claude");
    fs::write(&provider, support::claude_stand_in(r#"#!/usr/bin/python3
import json, os, pathlib, shlex, subprocess, sys
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = pathlib.Path(os.environ['LF_HOME'])
sys.stderr = open(home / 'provider-errors', 'w', buffering=1)
caller = json.loads(os.environ['LF_AGENT_CALLER'])
# Drain both messages before launching hooks so hook latency cannot block the seed write.
context = json.loads(sys.stdin.readline())
query = json.loads(sys.stdin.readline())
settings = json.load(open(sys.argv[sys.argv.index('--settings') + 1]))
hook = settings['hooks']['SessionStart'][0]['hooks'][0]['command']
arguments = shlex.split(hook)
assert 'LF_PROVIDER_CONTEXT' not in os.environ
assert 'Stop' not in settings['hooks'], list(settings['hooks'])
commands = [h['command'].split()[-1] for group in settings['hooks']['SessionStart'] for h in group['hooks']]
assert commands == ['start', 'compact'], commands
output = subprocess.check_output(hook, shell=True, input=json.dumps({'session_id': 'startup-native', 'hook_event_name': 'SessionStart', 'source': 'startup'}).encode())
context_text = json.loads(output)['hookSpecificOutput']['additionalContext']
assert len(context_text.encode()) <= 10000
if (home / 'expect-steer').exists():
    assert 'STARTUP_DIRECTION' in context_text
    assert '[excerpt; full source above]' in context_text
(home / 'startup-complete').touch()
# A bare launch's tool hook reads its steers from the store, with no server.
adapter = json.loads(os.environ['LF_BARE_STEER'])
assert adapter['agent_process_id'] == caller['agent_process_id']
tool = settings['hooks']['PostToolUse'][0]['hooks'][0]['command']
def after_tool():
    callback = {'session_id': 'startup-native', 'hook_event_name': 'PostToolUse'}
    return subprocess.check_output(tool, shell=True, input=json.dumps(callback).encode()).decode()
assert after_tool() == '', 'no steer yet'
seen = pathlib.Path(os.environ['LF_STEER_SEEN'])
checked = seen.stat().st_mtime_ns
assert after_tool() == '' and seen.stat().st_mtime_ns == checked, 'lf ran with no new steer'
subprocess.check_call([arguments[2], 'steer', caller['session_id'], 'BARE_DIRECTION'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
steered = json.loads(after_tool())['hookSpecificOutput']
assert steered['hookEventName'] == 'PostToolUse' and 'BARE_DIRECTION' in steered['additionalContext'], steered
assert after_tool() == '', 'the steer arrived twice'
assert context['shouldQuery'] is False
expected = (home / 'clipboard').read_text()
assert expected in context['message']['content'], 'incomplete non-query context'
assert query['message']['content'][0]['text'].endswith('QUERY_END'), 'incomplete query'
assert len(json.dumps(context).encode()) > 131133
assert query['uuid']
query['session_id'] = 'startup-native'
keep_claude_turn(query, 'CORRELATED_STARTUP_ANSWER')
print(json.dumps(query), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'startup-native'}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'answer-id', 'session_id': 'startup-native', 'subtype': 'success', 'is_error': False, 'result': 'CORRELATED_STARTUP_ANSWER'}), flush=True)
sys.stdin.read()
"#)).unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", lf_bin)
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        // The Keychain item lf signs the shared home in with names its user.
        .env("USER", "fixture")
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("GIT_ALLOW_PROTOCOL", "file")
        .current_dir(repo.path())
        .args(task.into_iter().flat_map(|task| ["--task", task]))
        .args([
            "-b",
            "-a",
            "claude",
            "-c",
            "skill",
            "startup-proof",
            "QUERY_END",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn assert_launched(output: &std::process::Output, home: &std::path::Path) {
    assert!(
        output.status.success(),
        "{}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(home.join("provider-errors")).unwrap_or_default()
    );
    assert!(home.join("startup-complete").exists());
}

#[test]
fn claude_startup_preserves_large_combined_seed_and_correlated_answer() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _task =
        support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let store = loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
    let steer = store
        .save_repository_steer(
            &loopflow_execution::repository::discover(repo.path()).unwrap(),
            "Fixture author",
            &format!("STARTUP_DIRECTION {}", "界<&>".repeat(20_000)),
        )
        .unwrap();
    fs::write(home.path().join("expect-steer"), "").unwrap();
    // The bare startup hook reads the steer directly, even with a server present.
    let _server = MachineServer::start(env!("CARGO_BIN_EXE_lf"), home.path());
    let output = launch_startup_proof(&repo, home.path(), Some("INF-123"));
    assert_launched(&output, home.path());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let session: String = db
        .query_row(
            "SELECT id FROM agent_sessions WHERE provider_thread='startup-native'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // The answer is the one Claude kept for the query's turn.
    let answer = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .args(["history", "show", &session, "--final"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(answer.status.success(), "{answer:?}");
    assert_eq!(
        String::from_utf8_lossy(&answer.stdout).trim(),
        "CORRELATED_STARTUP_ANSWER"
    );
    let receipts = store.session_steers(&session, 0, 32).unwrap();
    assert_eq!(receipts.items[0].steer.id, steer.id);
    // The startup hook claimed it, printed it and stamped it.
    assert!(receipts.items[0].delivered_at.is_some());
    assert_eq!(
        store
            .session_steer_source(&session, &steer.id)
            .unwrap()
            .text,
        steer.text
    );
}

/// A bare `lf <skill>` in a Task's checkout needs no Machine server: it
/// counts toward the Task, its two context hooks hand the provider its block
/// from checkout files, and its tool hook reads a steer saved for its Session
/// from the store. Nothing asks a server, so no gap is logged.
#[test]
fn a_bare_skill_in_a_task_checkout_takes_a_steer_and_asks_no_server() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let task =
        support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let output = launch_startup_proof(&repo, home.path(), None);
    assert_launched(&output, home.path());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let (counted, source): (String, String) = db
        .query_row(
            "SELECT task_id,work_source FROM agent_sessions WHERE provider_thread='startup-native'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(counted, task.task.id.to_string());
    assert_eq!(source, "checkout");
    let delivered: Option<i64> = db
        .query_row("SELECT delivered_at FROM steer_delivered", [], |r| r.get(0))
        .unwrap();
    assert!(delivered.is_some());
    let log = fs::read_to_string(home.path().join("logs/lf.log")).unwrap_or_default();
    assert!(!log.contains("Loopflow server unavailable"), "{log}");
}

/// Naming a Task on a direct skill launch requires no Machine server.
#[test]
fn a_task_attributed_skill_launch_needs_no_machine_server() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _task =
        support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    // The selected installation's `lf install server` reaches no supervisor.
    let lf = home.path().join("lf");
    fs::write(
        &lf,
        format!(
            "#!/bin/sh\nif [ \"$1 $2\" = \"install server\" ]; then\n  echo 'no supervisor here' >&2\n  exit 1\nfi\nexec {} \"$@\"\n",
            env!("CARGO_BIN_EXE_lf")
        ),
    )
    .unwrap();
    fs::set_permissions(&lf, fs::Permissions::from_mode(0o755)).unwrap();
    let output = launch_startup_proof_through(&repo, home.path(), &lf, Some("INF-123"));
    assert_launched(&output, home.path());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("no supervisor here"));
}
/// A Task checkout with a connected account, a Claude stand-in that marks
/// `provider-started` (and `provider-resumed` for `--resume`), and an `LF_BIN`
/// whose `lf install server` fails, so no Machine server can start.
struct InteractiveTask {
    repo: TestRepo,
    home: tempfile::TempDir,
    lf: std::path::PathBuf,
}

impl InteractiveTask {
    fn new() -> Self {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let _task =
            support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
        let bin = home.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(
            bin.join("claude"),
            "#!/bin/sh
[ \"$1\" = --version ] && { echo fixture; exit 0; }
touch \"$LF_HOME/provider-started\"
for arg in \"$@\"; do [ \"$arg\" = --resume ] && touch \"$LF_HOME/provider-resumed\"; done
exit 0\n",
        )
        .unwrap();
        let lf = home.path().join("lf");
        fs::write(
            &lf,
            format!(
                "#!/bin/sh\nif [ \"$1 $2\" = \"install server\" ]; then\n  echo 'no supervisor here' >&2\n  exit 1\nfi\nexec {} \"$@\"\n",
                env!("CARGO_BIN_EXE_lf")
            ),
        )
        .unwrap();
        for path in [bin.join("claude"), lf.clone()] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self { repo, home, lf }
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        self.run_program(env!("CARGO_BIN_EXE_lf"), args)
    }

    fn run_program(
        &self,
        program: impl AsRef<std::ffi::OsStr>,
        args: &[impl AsRef<std::ffi::OsStr>],
    ) -> std::process::Output {
        Command::new(program)
            .env_clear()
            .env("HOME", self.home.path())
            .env("LF_HOME", self.home.path())
            .env("LF_BIN", &self.lf)
            .env("CLAUDE_CONFIG_DIR", self.home.path().join(".claude"))
            .env("USER", "fixture")
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.home.path().join("bin").display()),
            )
            .env("GIT_ALLOW_PROTOCOL", "file")
            .current_dir(self.repo.path())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// A Task whose Claude conversation launched beside a Machine server that
    /// has since stopped.
    fn launched_then_stopped() -> Self {
        let task = Self::new();
        let server = MachineServer::start(env!("CARGO_BIN_EXE_lf"), task.home.path());
        let launched = task.run(&["--task", "INF-123", "-i", "-a", "claude", ":", "hello"]);
        assert!(
            launched.status.success(),
            "{}",
            String::from_utf8_lossy(&launched.stderr)
        );
        drop(server);
        task
    }

    /// The Session id and `open_argv` that `args --prepare --json` returns.
    fn prepared(&self, args: &[&str]) -> (String, Vec<String>) {
        let output = self.run(&[args, &["--prepare", "--json"]].concat());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let record: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        (
            record["id"].as_str().unwrap().to_string(),
            serde_json::from_value(record["open_argv"].clone()).unwrap(),
        )
    }

    fn marked(&self, name: &str) -> bool {
        self.home.path().join(name).exists()
    }
}

fn assert_names_missing_server(output: &std::process::Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("Loopflow server is not running") && stderr.contains("lf install server"),
        "{stderr}"
    );
}

/// Interactive direct launches also keep their caller with Task attribution.
#[test]
fn an_interactive_task_attributed_launch_needs_no_machine_server() {
    let task = InteractiveTask::new();
    let output = task.run(&["--task", "INF-123", "-i", "-a", "claude", ":", "hello"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(task.marked("provider-started"));
}

/// A Task resume opens a managed conversation: `lf -i --task X resume` of the
/// Task's recorded Claude conversation, after the server stopped, fails naming
/// it instead of continuing unrecorded.
#[test]
fn an_interactive_task_resume_with_no_machine_server_fails_naming_it() {
    let task = InteractiveTask::launched_then_stopped();
    assert_names_missing_server(&task.run(&["-i", "--task", "INF-123", "resume"]));
    assert!(!task.marked("provider-resumed"));
}

/// Desktop opens a Task's conversation by preparing it, then running the
/// returned `open_argv` in a terminal. A prepare naming the Task returns a
/// command that resumes managed, so with the server stopped it fails naming it,
/// as `lf resume --task` does.
#[test]
fn a_prepared_task_resume_opens_managed_and_fails_naming_a_missing_server() {
    let task = InteractiveTask::launched_then_stopped();
    let (id, argv) = task.prepared(&["session", "resume", "--task", "INF-123"]);
    assert!(
        argv.ends_with(&["session".into(), "resume".into(), id]),
        "{argv:?}"
    );
    assert_names_missing_server(&task.run_program(&argv[0], &argv[1..]));
    assert!(!task.marked("provider-resumed"));
}

/// Desktop's Task workspace reopens a pane that already has a Session by its
/// id, naming the Task beside it: the id selects the managed conversation.
#[test]
fn a_task_pane_reopened_by_session_id_fails_naming_a_missing_server() {
    let task = InteractiveTask::launched_then_stopped();
    let (id, _) = task.prepared(&["session", "resume", "--task", "INF-123"]);
    let (_, argv) = task.prepared(&["session", "resume", &id, "--task", "INF-123"]);
    assert!(
        argv.ends_with(&["session".into(), "resume".into(), id]),
        "{argv:?}"
    );
    assert_names_missing_server(&task.run_program(&argv[0], &argv[1..]));
    assert!(!task.marked("provider-resumed"));
}

#[test]
fn public_context_capability_rejects_children_and_replacement() {
    use std::io::Write;
    use std::time::{Duration, Instant};

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    // Exercise the bare Task launch's steer capability through the Machine
    // server's callback path as well as through its direct hook.
    let _task =
        support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let store = loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
    let mut server = Some(MachineServer::start(env!("CARGO_BIN_EXE_lf"), home.path()));
    let provider = home.path().join("claude");
    fs::write(&provider, r#"#!/usr/bin/python3
import json, os, pathlib, sys, time
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = pathlib.Path(os.environ['LF_HOME'])
message = json.loads(sys.stdin.readline())
message['session_id'] = 'handoff-native'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'handoff-native'}), flush=True)
print(json.dumps({'type': 'assistant', 'session_id': 'handoff-native', 'message': {'id': 'held', 'role': 'assistant', 'content': [{'type': 'text', 'text': 'Holding'}]}}), flush=True)
(home / 'handoff.tmp').write_text(json.dumps({'delivery': os.environ['LF_CONTEXT_DELIVERY'], 'caller': os.environ['LF_AGENT_CALLER'], 'capture': os.environ['LF_CAPTURE_KEY'], 'capability': os.environ['LF_BARE_STEER']}))
os.replace(home / 'handoff.tmp', home / 'handoff.json')
deadline = time.monotonic() + 15
while not (home / 'finish').exists() and time.monotonic() < deadline:
    time.sleep(.01)
"#).unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
    let startup_log = home.path().join("startup.log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .env("USER", "fixture")
        .env("PATH", format!("{}:/usr/bin:/bin", home.path().display()))
        .current_dir(repo.path())
        .args(["--task", "INF-123", "-v", "-b", "-a", "claude"])
        .args([":", "Hold this conversation"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(&startup_log).unwrap())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !home.path().join("handoff.json").exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!(
                "lf exited {status} before the provider started: {}",
                fs::read_to_string(&startup_log).unwrap()
            );
        }
        assert!(
            Instant::now() < deadline,
            "provider did not start within 10s: {}",
            fs::read_to_string(&startup_log).unwrap()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let declaration: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("handoff.json")).unwrap()).unwrap();
    let caller: serde_json::Value =
        serde_json::from_str(declaration["caller"].as_str().unwrap()).unwrap();
    let session = caller["session_id"].as_str().unwrap();
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "ATTACH ?1 AS history",
        [
            loopflow_storage::history::history_path(&home.path().join("loopflow.db"))
                .to_string_lossy()
                .as_ref(),
        ],
    )
    .unwrap();
    loop {
        let native: Option<String> = db
            .query_row(
                "SELECT provider_thread FROM agent_sessions WHERE id=?1",
                [session],
                |r| r.get(0),
            )
            .unwrap();
        if native.as_deref() == Some("handoff-native") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "native identity was not recorded"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // Reconnect reopens the conversation the capture published.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let published: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM session_events WHERE session_id=?1
                 AND kind='captured' AND provider_thread='handoff-native')",
                [session],
                |r| r.get(0),
            )
            .unwrap();
        if published {
            break;
        }
        assert!(Instant::now() < deadline, "resume target was not published");
        std::thread::sleep(Duration::from_millis(10));
    }
    let first = store.session_attachment(session).unwrap().unwrap();
    // The Machine server records each boundary; the hook claims its own steers.
    let hook = |payload: serde_json::Value| {
        let mut hook = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("LF_AGENT_CALLER", declaration["caller"].as_str().unwrap())
            .env("LF_CAPTURE_KEY", declaration["capture"].as_str().unwrap())
            .env(
                "LF_CONTEXT_DELIVERY",
                declaration["delivery"].as_str().unwrap(),
            )
            .env(
                "LF_PROVIDER_CONTEXT",
                declaration["capability"].as_str().unwrap(),
            )
            .env("PATH", "/usr/bin:/bin")
            .current_dir(repo.path())
            .args(["__context-block", "--moment", "start"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        hook.stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        hook.wait_with_output().unwrap()
    };
    let direction = store
        .save_session_steer(session, "Fixture author", "HANDOFF_DIRECTION")
        .unwrap();
    let payload = serde_json::json!({"session_id":"handoff-native", "hook_event_name":"PostToolUse", "tool_use_id":"handoff-tool", "turn_id":"native-turn"});
    let received = hook(payload.clone());
    assert!(
        received.status.success(),
        "{}",
        String::from_utf8_lossy(&received.stderr)
    );
    assert!(String::from_utf8_lossy(&received.stdout).contains("HANDOFF_DIRECTION"));
    // Printed and stamped by the hook that claimed it; the next hook repeats nothing.
    let read = &store.session_steers(session, 0, 32).unwrap().items[0];
    assert_eq!(read.steer.id, direction.id);
    assert!(read.delivered_at.is_some());
    let mut next_tool = payload.clone();
    next_tool["tool_use_id"] = serde_json::json!("handoff-tool-next");
    let next = hook(next_tool);
    assert!(next.status.success());
    assert!(!String::from_utf8_lossy(&next.stdout).contains("HANDOFF_DIRECTION"));
    let pending = store
        .save_session_steer(session, "Fixture author", "NEXT_DIRECTION")
        .unwrap();
    let mut nested = payload.clone();
    nested["agent_id"] = serde_json::json!("child");
    let rejected = hook(nested);
    assert!(rejected.stdout.is_empty());
    let mut foreign = payload.clone();
    foreign["session_id"] = serde_json::json!("child-native");
    // A foreign conversation's refused authority is a recording gap: its tool
    // completes with the launch's plain context and no steer reaches it.
    let rejected = hook(foreign);
    assert!(rejected.status.success());
    let rejected = String::from_utf8_lossy(&rejected.stdout);
    assert!(!rejected.contains("NEXT_DIRECTION") && !rejected.contains("HANDOFF_DIRECTION"));
    assert_eq!(store.session_steers(session, 0, 32).unwrap().pending, 1);
    // Exercise the shipped OpenCode plugin against the real Machine server.
    // Later requests retain the steer in the model context without claiming it again.
    let plugin = home.path().join("transform.mjs");
    let delivery_path = declaration["delivery"].as_str().unwrap();
    let delivery: serde_json::Value =
        serde_json::from_slice(&fs::read(delivery_path).unwrap()).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["server", "status"])
        .env("LF_HOME", home.path())
        .output()
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let plugin_server = serde_json::json!({"socket":status["socket"],
        "revision":loopflow::server::wire::REVISION,
        "min_revision":loopflow::server::wire::MIN_REVISION,
        "platform":std::env::consts::OS, "name":"lf opencode plugin", "version":"fixture"});
    fs::write(&plugin, format!("const spec = {};\nconst delivery = {};\n{}\n{}",
        serde_json::json!({"executable":env!("CARGO_BIN_EXE_lf"),"delivery":delivery,"instructions":"","turns":true,"server":plugin_server}),
        serde_json::to_string(delivery_path).unwrap(),
        include_str!("../../loopflow-execution/src/harness/server_connection.js"),
        include_str!("../../loopflow-execution/src/harness/context_opencode.js"))).unwrap();
    let probe = home.path().join("transform-proof.mjs");
    fs::write(
        &probe,
        r#"
import assert from 'node:assert/strict';
import plugin from './transform.mjs';
// The plugin's server sockets are unref'd; keep the probe alive until done.
const alive = setInterval(() => {}, 1000);
const hooks = {};
await plugin.setup({session:{hook:async(name, callback) => { hooks[name] = callback; }},event:{subscribe:async function*(){}}});
const output = {sessionID:'handoff-native',system:[],messages:[]};
await hooks.context(output);
const first = JSON.stringify(output);
assert(first.includes('NEXT_DIRECTION'));
assert.equal(JSON.stringify(structuredClone(output)), first);
const later = {sessionID:'handoff-native',system:[],messages:[]};
await hooks.context(later);
assert.equal(JSON.stringify(later).match(/NEXT_DIRECTION/g)?.length, 1);
assert.equal(output.messages.length,0); // stored/native conversation remains untouched
clearInterval(alive);

"#,
    )
    .unwrap();
    let protocol = Command::new("node")
        .arg(&probe)
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env(
            "LF_PROVIDER_CONTEXT",
            declaration["capability"].as_str().unwrap(),
        )
        .env("LF_STEER_SEEN", home.path().join("plugin-seen"))
        .output()
        .unwrap();
    assert!(
        protocol.status.success(),
        "{}",
        String::from_utf8_lossy(&protocol.stderr)
    );
    let page = store.session_steers(session, 0, 32).unwrap();
    assert_eq!(page.pending, 0);
    assert!(page
        .items
        .iter()
        .find(|row| row.steer.id == pending.id)
        .unwrap()
        .delivered_at
        .is_some());
    store
        .save_session_steer(session, "Fixture author", "FAILED_TOOL_DIRECTION")
        .unwrap();
    let failed = hook(serde_json::json!({"session_id":"handoff-native",
        "hook_event_name":"PostToolUseFailure","tool_use_id":"failed-tool"}));
    assert!(
        failed.status.success(),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
    assert!(String::from_utf8_lossy(&failed.stdout).contains("FAILED_TOOL_DIRECTION"));
    // `/clear` starts another native conversation in the same provider process.
    store
        .save_session_steer(session, "Fixture author", "CLEARED_DIRECTION")
        .unwrap();
    let cleared = hook(serde_json::json!({"session_id":"cleared-native",
        "hook_event_name":"SessionStart","source":"clear"}));
    assert!(
        cleared.status.success(),
        "{}",
        String::from_utf8_lossy(&cleared.stderr)
    );
    let cleared = String::from_utf8_lossy(&cleared.stdout).into_owned();
    assert!(cleared.contains("CLEARED_DIRECTION"));
    assert!(cleared.contains("additionalContext"));
    // The replaced conversation keeps working without steers.
    let old_conversation = hook(payload.clone());
    assert!(old_conversation.status.success());
    assert!(!String::from_utf8_lossy(&old_conversation.stdout).contains("_DIRECTION"));
    store
        .save_session_steer(session, "Fixture author", "AFTER_CLEAR_DIRECTION")
        .unwrap();
    let after_clear = hook(serde_json::json!({"session_id":"cleared-native",
        "hook_event_name":"PostToolUse","tool_use_id":"cleared-tool"}));
    assert!(
        after_clear.status.success(),
        "{}",
        String::from_utf8_lossy(&after_clear.stderr)
    );
    assert!(String::from_utf8_lossy(&after_clear.stdout).contains("AFTER_CLEAR_DIRECTION"));
    // With the server stopped the hook still reads its steer from the store,
    // and the conversation keeps one recording gap until the server answers.
    drop(server.take());
    store
        .save_session_steer(session, "Fixture author", "WAITED_DIRECTION")
        .unwrap();
    let unanswered = serde_json::json!({"session_id":"cleared-native",
        "hook_event_name":"PostToolUse","tool_use_id":"unanswered-tool"});
    let read = [true, false].map(|_| {
        let missed = hook(unanswered.clone());
        assert!(
            missed.status.success(),
            "{}",
            String::from_utf8_lossy(&missed.stderr)
        );
        String::from_utf8_lossy(&missed.stdout).contains("WAITED_DIRECTION")
    });
    assert_eq!(read, [true, false]);
    let log = fs::read_to_string(home.path().join("logs/lf.log")).unwrap();
    let down = "recording gap: session boundary: Loopflow server unavailable";
    assert_eq!(log.matches(down).count(), 1, "{log}");
    let _server = MachineServer::start(env!("CARGO_BIN_EXE_lf"), home.path());
    let answered = hook(serde_json::json!({"session_id":"cleared-native",
        "hook_event_name":"PostToolUse","tool_use_id":"answered-tool"}));
    assert!(answered.status.success());
    let gaps: i64 = db
        .query_row(
            "SELECT count(*) FROM history.session_history WHERE session_id=?1
             AND json_extract(payload,'$.source')='store-gap'",
            [session],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(gaps, 1);
    let marker = {
        use sha2::Digest;
        home.path().join("context-deliveries").join(format!(
            "{:x}.store-gap",
            sha2::Sha256::digest(b"cleared-native")
        ))
    };
    assert!(!marker.exists(), "the answered hook closes its gap");
    let selected: String = db
        .query_row(
            "SELECT json_extract(payload,'$.transition.from_provider_session') FROM history.session_history
             WHERE session_id=?1 AND json_extract(payload,'$.type')='provider_transition'
             AND json_extract(payload,'$.transition.to_provider_session')='cleared-native'",
            [session],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(selected, "handoff-native");
    fs::write(home.path().join("finish"), "").unwrap();
    child.wait().unwrap();
    // The lifeline watchdog ends the provider group up to two seconds later.
    let group: i64 = db
        .query_row(
            "SELECT pid FROM processes WHERE id=?1",
            [&first.agent_process_id],
            |r| r.get(0),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Command::new("/bin/kill")
        .args(["-0", "--", &format!("-{group}")])
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
    {
        assert!(Instant::now() < deadline, "provider group outlived its run");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn public_commands_save_steers_without_launching_providers() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let task =
        support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_ALLOW_PROTOCOL", "file")
            .current_dir(repo.path())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    run(&["--repo", ".", "steer", "REPOSITORY_DIRECTION"]);
    run(&[
        "--wave",
        task.task.wave_id.as_ref().unwrap().as_str(),
        "steer",
        "WAVE_DIRECTION",
    ]);
    // A thread entry can sort after the comment this command saves. Its
    // timestamp must not decide which comment's steer the command returns.
    loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db"))
        .unwrap()
        .append_task_comment(
            &task.task.id,
            &loopflow_execution::work::task::comment::TaskComment {
                id: "later-thread-entry".into(),
                author: loopflow_execution::work::task::comment::TaskCommentAuthor::Integration,
                body: "Existing thread entry".into(),
                created_at: Some("9999-12-31T23:59:59Z".into()),
            },
        )
        .unwrap();
    run(&["steer", task.task.id.as_str(), "TASK_DIRECTION"]);
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,input_published,repo,task_id,wave_id)
        VALUES('idle','Idle fixture','human',1,?1,1,?2,?3,?4)",
        rusqlite::params![
            repo.path().to_str().unwrap(),
            loopflow_execution::repository::discover(repo.path())
                .unwrap()
                .to_string(),
            task.task.id.as_str(),
            task.task.wave_id.as_ref().unwrap().as_str()
        ],
    )
    .unwrap();
    let artifact = format!("run_{}", uuid::Uuid::new_v4().simple());
    conn.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
        VALUES('idle','captured',?1,1,json_object('artifact_key',?1))",
        [artifact.as_str()],
    )
    .unwrap();
    conn.execute(
        "UPDATE agent_sessions SET current_capture=?1 WHERE id='idle'",
        [conn.last_insert_rowid()],
    )
    .unwrap();
    run(&["steer", "idle", "DIRECT_DIRECTION"]);
    let page: serde_json::Value = serde_json::from_str(&run(&[
        "session", "steers", "idle", "--json", "--limit", "2",
    ]))
    .unwrap();
    assert_eq!(page["pending"], 4, "{page}");
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    let cursor = page["next_after"].as_i64().unwrap().to_string();
    let rest: serde_json::Value = serde_json::from_str(&run(&[
        "session", "steers", "idle", "--json", "--after", &cursor,
    ]))
    .unwrap();
    assert_eq!(rest["items"].as_array().unwrap().len(), 2);
    let steer = rest["items"][1]["steer"]["id"].as_str().unwrap();
    let source: serde_json::Value =
        serde_json::from_str(&run(&["session", "steers", "idle", "--source", steer])).unwrap();
    assert_eq!(source["text"], "DIRECT_DIRECTION");
    let provider_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM processes WHERE kind='agent'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(provider_count, 0);
}

/// A stopped Claude conversation with a saved thread, and a stand-in Claude
/// on `bin` that appends each launch's first input to `first-input`.
fn stopped_claude_conversation(home: &std::path::Path) -> String {
    let (id, input, _) =
        support::prepare_conversation(home, home, "claude", "Remember the word mango");
    let native = uuid::Uuid::new_v4().to_string();
    support::record_native(home, &id, &input, &native);
    let transcripts = home.join(".claude/projects/fixture");
    fs::create_dir_all(&transcripts).unwrap();
    fs::write(
        transcripts.join(format!("{native}.jsonl")),
        r#"{"type":"user","userType":"external","message":{"content":"remember the word mango"}}"#,
    )
    .unwrap();
    fs::write(
        home.join(".claude.json"),
        serde_json::json!({"projects": {"/": {"hasTrustDialogAccepted": true}}}).to_string(),
    )
    .unwrap();
    let bin = home.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        r#"#!/bin/sh
if [ "$1" = --version ]; then echo '2.1.302'; exit; fi
{ cat; printf '\n--launch--\n'; } >> "$LF_HOME/first-input"
"#,
    )
    .unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
    id
}

/// What the stand-in has been given once it has launched `launches` times.
fn first_inputs(home: &std::path::Path, launches: usize) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let text = fs::read_to_string(home.join("first-input")).unwrap_or_default();
        if text.matches("--launch--").count() >= launches {
            return text;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{launches} launches; saw {text:?}\n{}",
            fs::read_to_string(home.join("logs/lf.log")).unwrap_or_default()
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// A steer naming a stopped conversation returns at once; the Machine server
/// starts the conversation in a held terminal, which takes the steer as its
/// first input. Steers saved while no server runs start it once, together,
/// when the server is back.
#[test]
fn a_steer_starts_a_stopped_conversation_on_the_machine_server() {
    let home = tempfile::Builder::new()
        .prefix("lf-steer-")
        .tempdir_in("/tmp")
        .unwrap();
    let id = stopped_claude_conversation(home.path());
    let path = format!("{}:/usr/bin:/bin", home.path().join("bin").display());
    let lf = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", "/usr/bin:/bin")
            .current_dir(home.path())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    let server = MachineServer::start_with_path(env!("CARGO_BIN_EXE_lf"), home.path(), &path);
    let saved = lf(&["steer", &id, "WHAT_WORD_DIRECTION"]);
    assert!(
        saved.contains(&format!("the Machine server starts {id} to read it")),
        "{saved}"
    );
    let given = first_inputs(home.path(), 1);
    assert!(given.contains("WHAT_WORD_DIRECTION"), "{given}");
    let page: serde_json::Value =
        serde_json::from_str(&lf(&["session", "steers", &id, "--json"])).unwrap();
    assert_eq!(page["pending"], 0, "{page}");

    drop(server);
    lf(&["steer", &id, "FIRST_WAITING_DIRECTION"]);
    lf(&["steer", &id, "SECOND_WAITING_DIRECTION"]);
    let _server = MachineServer::start_with_path(env!("CARGO_BIN_EXE_lf"), home.path(), &path);
    let given = first_inputs(home.path(), 2);
    let second = given.split("--launch--").nth(1).unwrap();
    assert!(
        second.contains("FIRST_WAITING_DIRECTION") && second.contains("SECOND_WAITING_DIRECTION"),
        "one start reads both: {given}"
    );
    assert!(!second.contains("WHAT_WORD_DIRECTION"), "{given}");
}
