//! Public launch and continuation contract, with providers recording their real process inputs.
#![cfg(unix)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use loopflow_test_support::TestRepo;
use serde_json::Value;

struct Fixture {
    repo: TestRepo,
    home: tempfile::TempDir,
}

impl Fixture {
    fn new(provider: &str) -> Self {
        let fixture = Self {
            repo: TestRepo::new(),
            home: tempfile::tempdir().unwrap(),
        };
        fs::create_dir(fixture.home.path().join("bin")).unwrap();
        fixture.repo.create_file(
            ".lf/config.yaml",
            &format!("agent: {provider}\ndiff: false\ndiff_files: false\npaste: false\n"),
        );
        for harness in ["claude", "codex", "opencode"] {
            let script = fixture.home.path().join("bin").join(harness);
            fs::write(&script, r#"#!/usr/bin/python3
import json, os, sys, runpy
args = sys.argv[1:]
if '--version' in args:
    print('fixture'); sys.exit(0)
if '--dangerously-bypass-hook-trust' in args:
    sys.exit(2)
record = {'args': args, 'cwd': os.getcwd(), 'opencode': os.environ.get('OPENCODE_CONFIG_CONTENT'), 'participant': os.environ.get('LF_USER_NAME'), 'account': os.environ.get('LF_PROVIDER_ACCOUNT_ID'), 'codex_home': os.environ.get('CODEX_HOME')}
if '--append-system-prompt-file' in args:
    record['context'] = open(args[args.index('--append-system-prompt-file')+1]).read()
stream = '--input-format' in args
if stream:
    message = json.loads(sys.stdin.readline())
    record['input'] = message
elif os.path.basename(sys.argv[0]) == 'claude':
    record['input'] = sys.stdin.read()
with open(os.environ['HOME']+'/received.json', 'w') as out:
    json.dump(record, out)
if stream:
    token = args[args.index('--resume')+1] if '--resume' in args else 'stream-fixture'
    print(json.dumps({'type':'system','subtype':'init','session_id':token}), flush=True)
    message['session_id'] = token
    print(json.dumps(message), flush=True)
    print(json.dumps({'type':'result','session_id':token,'subtype':'success','result':'done'}), flush=True)
elif os.path.basename(sys.argv[0]) == 'codex':
    sys.path.insert(0, SUPPORT_DIRECTORY)
    runpy.run_path(SUPPORT_DIRECTORY+'/native_codex.py', run_name='__main__')
elif os.path.basename(sys.argv[0]) == 'opencode':
    print('timestamp=2026-10-09T00:00:00Z level=INFO run=tui message=created id=ses_fixture directory='+os.getcwd(), file=sys.stderr)
"#.replace("SUPPORT_DIRECTORY", &serde_json::to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support")).unwrap())).unwrap();
            fs::set_permissions(script, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fixture
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .env_clear()
            .env("HOME", self.home.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.home.path().join("bin").display()),
            )
            .env("LF_HOME", self.home.path().join("machine"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("CLAUDE_CONFIG_DIR", self.home.path().join(".claude"))
            .env("CODEX_HOME", self.home.path().join(".codex"))
            .env("LF_USER_NAME", "Session Fixture")
            .stdin(Stdio::null())
            .current_dir(self.repo.path());
        command
    }

    fn run(&self, args: &[&str]) -> Value {
        let output = self.command().args(args).output().unwrap();
        success(&output);
        self.received()
    }

    fn received(&self) -> Value {
        let mut received: Value =
            serde_json::from_slice(&fs::read(self.home.path().join("received.json")).unwrap())
                .unwrap();
        let machine = self.home.path().join("machine");
        if let Ok(params) = fs::read(machine.join("native-config.json")) {
            received["params"] = serde_json::from_slice(&params).unwrap();
            received["turns"] =
                serde_json::from_slice(&fs::read(machine.join("native-turns.json")).unwrap())
                    .unwrap();
            received["token"] = fs::read_to_string(machine.join("native-thread"))
                .unwrap()
                .into();
        }
        received
    }

    fn session(&self) -> String {
        let db = rusqlite::Connection::open(self.home.path().join("machine/loopflow.db")).unwrap();
        db.query_row(
            "SELECT id FROM agent_sessions ORDER BY created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    fn create_wave(&self) {
        let store = loopflow::store::sqlite::SqliteStore::new(
            &self.home.path().join("machine/loopflow.db"),
        )
        .unwrap();
        store
            .create_wave(&loopflow::work::wave::Wave::new(
                loopflow::id::WaveId::new(),
                "fixture".into(),
                loopflow::repository::CanonicalRepo::discover(self.repo.path())
                    .unwrap()
                    .to_string(),
            ))
            .unwrap();
    }

    fn settings(&self, provider: &str, yolo: bool, chrome: bool) {
        self.repo.create_file(".lf/config.yaml", &format!("agent: {provider}\nyolo: {yolo}\nchrome: {chrome}\ndiff: false\ndiff_files: false\npaste: false\n"));
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn args(value: &Value) -> Vec<&str> {
    value["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect()
}
fn pair(value: &Value, flag: &str, expected: &str) {
    assert!(
        args(value).windows(2).any(|p| p == [flag, expected]),
        "{value}"
    );
}
fn bypass(value: &Value, provider: &str, enabled: bool) {
    match provider {
        "claude" => assert_eq!(
            args(value).contains(&"--dangerously-skip-permissions"),
            enabled,
            "{value}"
        ),
        "codex" => {
            assert_eq!(
                value["params"]["sandbox"],
                if enabled {
                    "danger-full-access"
                } else {
                    "workspace-write"
                },
                "{value}"
            );
            assert_eq!(
                value["params"]["approvalPolicy"],
                if enabled { "never" } else { "on-request" },
                "{value}"
            );
            assert!(!args(value).contains(&"--dangerously-bypass-approvals-and-sandbox"));
        }
        "opencode" => {
            let config: Value = serde_json::from_str(value["opencode"].as_str().unwrap()).unwrap();
            assert_eq!(
                config["permission"]["*"].as_str() == Some("allow"),
                enabled,
                "{value}"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn terminal_launch_and_resume_use_current_policy_and_preserve_native_identity() {
    for provider in ["claude", "codex", "opencode"] {
        let fixture = Fixture::new(provider);
        fixture.settings(provider, true, true);
        let first = fixture.run(&["-i", ":", "--", "--- keep spaces and 雪"]);
        bypass(&first, provider, true);
        assert_eq!(
            Path::new(first["cwd"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            fixture.repo.path().canonicalize().unwrap()
        );
        match provider {
            "claude" => assert!(first["input"]
                .as_str()
                .unwrap()
                .contains("--- keep spaces and 雪")),
            "codex" => assert!(first["turns"][0]
                .to_string()
                .contains("--- keep spaces and 雪")),
            _ => assert!(args(&first)
                .iter()
                .any(|arg| arg.contains("--- keep spaces and 雪"))),
        }
        assert_eq!(first["participant"], "Session Fixture");
        let token = if provider == "claude" {
            let a = args(&first);
            a[a.iter().position(|a| *a == "--session-id").unwrap() + 1].to_string()
        } else if provider == "opencode" {
            "ses_fixture".into()
        } else {
            first["token"].as_str().unwrap().to_string()
        };
        if provider == "claude" {
            assert!(args(&first).contains(&"--chrome"));
        }
        let session = fixture.session();
        fixture.settings(&format!("{provider}:current-model"), false, false);
        let resumed = fixture.run(&["session", "connect", &session]);
        bypass(&resumed, provider, false);
        assert!(args(&resumed).contains(&token.as_str()));
        assert!(!args(&resumed)
            .iter()
            .any(|arg| arg.contains("--- keep spaces")));
        assert!(!args(&resumed).contains(&"--session-id"));
        if provider == "codex" {
            assert_eq!(resumed["params"]["model"], "current-model");
            assert_eq!(resumed["turns"].as_array().unwrap().len(), 2);
            assert!(!resumed["turns"][1].to_string().contains("--- keep spaces"));
        } else {
            pair(&resumed, "--model", "current-model");
        }
        if provider == "claude" {
            assert!(args(&resumed).contains(&"--no-chrome"));
            assert_eq!(first["context"], resumed["context"]);
        }
        assert_eq!(fixture.session(), session);
        let override_resume =
            fixture.run(&["--yolo", "--chrome", "on", "session", "connect", &session]);
        bypass(&override_resume, provider, true);
        if provider == "claude" {
            assert!(args(&override_resume).contains(&"--chrome"));
        }
    }
}

#[test]
fn repo_false_overrides_global_true_and_cli_can_enable_bypass() {
    let fixture = Fixture::new("claude");
    fs::create_dir_all(fixture.home.path().join("machine")).unwrap();
    fs::write(
        fixture.home.path().join("machine/config.yaml"),
        "yolo: true\nchrome: true\n",
    )
    .unwrap();
    fixture.repo.create_file(
        ".claude/settings.json",
        r#"{"permissions":{"defaultMode":"auto"}}"#,
    );
    fixture.settings("claude", false, false);
    let configured = fixture.run(&["-i", ":", "hello"]);
    bypass(&configured, "claude", false);
    assert!(args(&configured).contains(&"--no-chrome"));
    let explicit = fixture.run(&["-i", "--yolo", "--chrome", "on", ":", "hello"]);
    bypass(&explicit, "claude", true);
    assert!(args(&explicit).contains(&"--chrome"));
}

#[test]
fn headless_continuation_and_flow_keep_current_options_without_repeating_the_skill() {
    let fixture = Fixture::new("claude:sonnet");
    fixture
        .repo
        .create_file(".lf/skills/probe.md", "INITIAL_SKILL_ONLY");
    fixture.settings("claude:sonnet", true, true);
    fixture.create_wave();
    let first = fixture.run(&[
        "-b",
        "--wave",
        "fixture",
        "--output-schema",
        r#"{"type":"object"}"#,
        "probe",
        "first turn",
    ]);
    pair(&first, "--input-format", "stream-json");
    pair(&first, "--json-schema", r#"{"type":"object"}"#);
    assert!(args(&first).contains(&"--chrome"));
    assert!(first["input"].to_string().contains("INITIAL_SKILL_ONLY"));
    let session = fixture.session();
    fixture.settings("claude:haiku", false, false);
    let resumed = fixture.run(&[
        "-b",
        "session",
        "connect",
        &session,
        "--message",
        "second turn",
    ]);
    pair(&resumed, "--resume", "stream-fixture");
    pair(&resumed, "--model", "haiku");
    assert!(args(&resumed).contains(&"--no-chrome"));
    assert!(!args(&resumed).contains(&"--json-schema"));
    assert!(!resumed["input"].to_string().contains("INITIAL_SKILL_ONLY"));
    assert!(resumed["input"].to_string().contains("second turn"));
    fixture
        .repo
        .create_file(".lf/flows/launch-probe.yaml", "- probe\n");
    let flowed = fixture.run(&["--yolo", "--chrome", "on", "flow", "launch-probe"]);
    bypass(&flowed, "claude", true);
    assert!(args(&flowed).contains(&"--chrome"));
}

#[test]
fn previews_do_not_start_a_provider_and_terminal_schema_is_explicitly_unsupported() {
    let fixture = Fixture::new("claude");
    success(
        &fixture
            .command()
            .args(["--context", ":", "preview"])
            .output()
            .unwrap(),
    );
    assert!(!fixture.home.path().join("received.json").exists());
    let output = fixture
        .command()
        .args(["-i", "--output-schema", "{}", ":", "hello"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("require a headless Session"));
    assert!(!fixture.home.path().join("received.json").exists());

    fixture.run(&["-i", ":", "create history"]);
    let session = fixture.session();
    fs::remove_file(fixture.home.path().join("received.json")).unwrap();
    let output = fixture
        .command()
        .args(["--output-schema", "{}", "session", "connect", &session])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("require a headless Session"));
    assert!(!fixture.home.path().join("received.json").exists());
    assert_eq!(fixture.session(), session);
}

#[test]
fn machine_dispatch_resolves_destination_policy_and_forwards_launch_and_resume_overrides() {
    let local = Fixture::new("codex:local-model");
    let remote = Fixture::new("claude:remote-model");
    local.settings("codex:local-model", true, true);
    remote.settings("claude:remote-model", false, false);
    let remote_bin = remote.home.path().join(".local/bin");
    fs::create_dir_all(&remote_bin).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_lf"), remote_bin.join("lf")).unwrap();
    for provider in ["claude", "codex", "opencode"] {
        std::os::unix::fs::symlink(
            remote.home.path().join("bin").join(provider),
            remote_bin.join(provider),
        )
        .unwrap();
    }
    let environment = serde_json::json!({
        "HOME": remote.home.path(),
        "LF_HOME": remote.home.path().join("machine"),
        "LF_BIN": env!("CARGO_BIN_EXE_lf"),
        "PATH": format!("{}:/usr/bin:/bin", remote.home.path().join("bin").display()),
        "CLAUDE_CONFIG_DIR": remote.home.path().join(".claude"),
        "CODEX_HOME": remote.home.path().join(".codex"),
    });
    let ssh = local.home.path().join("bin/ssh");
    fs::write(
        &ssh,
        format!(
            "#!/usr/bin/python3\nimport os, sys\nos.chdir({cwd})\nos.execve('/bin/bash', ['bash', '-c', sys.argv[-1]], {environment})\n",
            cwd = serde_json::to_string(remote.repo.path()).unwrap(),
        ),
    )
    .unwrap();
    fs::set_permissions(ssh, fs::Permissions::from_mode(0o755)).unwrap();
    success(
        &local
            .command()
            .args(["machine", "add", "fixture", "--repo"])
            .arg(remote.repo.path())
            .output()
            .unwrap(),
    );
    let dispatch = |arguments: &[&str]| {
        success(
            &local
                .command()
                .args(["--machine", "fixture"])
                .args(arguments)
                .output()
                .unwrap(),
        );
        assert!(!local.home.path().join("received.json").exists());
        remote.received()
    };
    let first = dispatch(&["-i", ":", "destination defaults"]);
    bypass(&first, "claude", false);
    pair(&first, "--model", "remote-model");
    assert!(args(&first).contains(&"--no-chrome"));
    assert_eq!(
        Path::new(first["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        remote.repo.path().canonicalize().unwrap()
    );
    let arguments = args(&first);
    let native = arguments[arguments
        .iter()
        .position(|arg| *arg == "--session-id")
        .unwrap()
        + 1];
    let session = remote.session();
    let resumed = dispatch(&["session", "connect", &session]);
    bypass(&resumed, "claude", false);
    pair(&resumed, "--resume", native);
    pair(&resumed, "--model", "remote-model");
    assert!(args(&resumed).contains(&"--no-chrome"));
    let explicit = dispatch(&[
        "--yolo",
        "--chrome",
        "on",
        "-a",
        "claude:haiku",
        "session",
        "connect",
        &session,
    ]);
    bypass(&explicit, "claude", true);
    pair(&explicit, "--resume", native);
    pair(&explicit, "--model", "haiku");
    assert!(args(&explicit).contains(&"--chrome"));
    assert_eq!(remote.session(), session);
    let explicit = dispatch(&[
        "-i",
        "--yolo",
        "--chrome",
        "on",
        "-a",
        "claude:sonnet",
        ":",
        "--",
        "--- literal prompt",
    ]);
    bypass(&explicit, "claude", true);
    pair(&explicit, "--model", "sonnet");
    assert!(args(&explicit).contains(&"--chrome"));
    assert!(explicit["input"]
        .as_str()
        .unwrap()
        .contains("--- literal prompt"));
}

#[path = "support/mod.rs"]
mod support;

#[test]
fn task_conversation_preparation_is_passive_and_connect_uses_shared_launch_settings() {
    let fixture = Fixture::new("claude:sonnet");
    fixture.settings("claude:sonnet", true, true);
    let base = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(fixture.repo.path())
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let registered = support::register_task_without_pr(
        &fixture.home.path().join("machine"),
        fixture.repo.path(),
        "main",
        base.trim(),
    );
    let unrelated = TestRepo::new();
    let output = fixture
        .command()
        .current_dir(unrelated.path())
        .args([
            "session",
            "connect",
            "--task",
            registered.task.id.as_str(),
            "--prepare",
            "--json",
        ])
        .output()
        .unwrap();
    success(&output);
    let prepared: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!fixture.home.path().join("received.json").exists());
    let id = prepared["id"].as_str().unwrap();
    let preview = fixture
        .command()
        .args(["session", "connect", id, "--prepare", "--json"])
        .output()
        .unwrap();
    success(&preview);
    assert!(!fixture.home.path().join("received.json").exists());
    // A newer conversation in the same checkout must not displace the primary.
    fixture.run(&["-i", ":", "another conversation"]);
    let explicit = fixture
        .command()
        .current_dir(unrelated.path())
        .args([
            "--task",
            registered.task.id.as_str(),
            "resume",
            "--prepare",
            "--json",
        ])
        .output()
        .unwrap();
    success(&explicit);
    assert_eq!(
        serde_json::from_slice::<Value>(&explicit.stdout).unwrap()["id"],
        id
    );
    let first = fixture.run(&["resume"]);
    bypass(&first, "claude", true);
    assert!(args(&first).contains(&"--chrome"));
    let native = args(&first)[args(&first)
        .iter()
        .position(|a| *a == "--session-id")
        .unwrap()
        + 1]
    .to_owned();
    fixture.settings("claude:haiku", false, false);
    let resumed = fixture.run(&["resume"]);
    bypass(&resumed, "claude", false);
    pair(&resumed, "--resume", &native);
    pair(&resumed, "--model", "haiku");
    assert!(args(&resumed).contains(&"--no-chrome"));
}

#[test]
fn native_codex_failed_bootstrap_preserves_account_and_pending_input() {
    let fixture = Fixture::new("codex");
    let machine = fixture.home.path().join("machine");
    support::register_codex_account(&machine);
    fs::write(machine.join("fail-terminal"), "").unwrap();
    let failed = fixture
        .command()
        .args([
            "-i",
            "--only-account",
            "fixture@example.com",
            "--isolate",
            "--yolo",
            ":",
            "Preserve this initial request",
        ])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let session = fixture.session();
    fs::remove_file(machine.join("fail-terminal")).unwrap();
    fixture.settings("codex:current-model", false, false);
    let resumed = fixture.run(&["session", "connect", &session]);
    bypass(&resumed, "codex", false);
    assert_eq!(resumed["account"], "fixture");
    assert_eq!(resumed["params"]["model"], "current-model");
    assert!(resumed["turns"][0]
        .to_string()
        .contains("Preserve this initial request"));
    assert_eq!(fixture.session(), session);
}

#[test]
fn native_resume_preserves_the_selected_account_and_account_home_mode() {
    for selection in ["--account", "--only-account"] {
        let fixture = Fixture::new("codex");
        let machine = fixture.home.path().join("machine");
        support::register_codex_account(&machine);
        let first = fixture.run(&[
            "-i",
            selection,
            "fixture@example.com",
            "--isolate",
            "--yolo",
            ":",
            "hello",
        ]);
        assert_eq!(first["account"], "fixture");
        let profile = machine.join("accounts/codex/fixture");
        assert_eq!(Path::new(first["codex_home"].as_str().unwrap()), profile);
        let session = fixture.session();
        fixture.settings("codex", false, false);
        let resumed = fixture.run(&["--isolate", "session", "connect", &session]);
        assert_eq!(resumed["account"], "fixture");
        assert_eq!(resumed["codex_home"], first["codex_home"]);
        bypass(&resumed, "codex", false);
        let shared = fixture.run(&["--shared", "session", "connect", &session]);
        assert_eq!(shared["account"], "fixture");
        assert_eq!(Path::new(shared["codex_home"].as_str().unwrap()), profile);
    }
}

#[test]
fn linked_worktree_metadata_is_accessible_on_launch_and_resume() {
    for provider in ["claude", "codex"] {
        let fixture = Fixture::new(provider);
        fixture.repo.stage_all();
        fixture.repo.commit("fixture config");
        let worktree = fixture.home.path().join("work tree with spaces");
        success(
            &Command::new("git")
                .args(["worktree", "add", "-b", "fixture"])
                .arg(&worktree)
                .current_dir(fixture.repo.path())
                .output()
                .unwrap(),
        );
        success(
            &fixture
                .command()
                .current_dir(&worktree)
                .args(["-i", ":", "--", "--- literal prompt"])
                .output()
                .unwrap(),
        );
        let first = fixture.received();
        // Input stays outside argv, including a prompt that looks like an option.
        assert!(!args(&first)
            .iter()
            .any(|arg| arg.contains("--- literal prompt")));
        if provider == "claude" {
            assert_eq!(args(&first).last(), Some(&"--"));
            assert!(first["input"]
                .as_str()
                .unwrap()
                .contains("--- literal prompt"));
        } else {
            assert!(first["turns"][0].to_string().contains("--- literal prompt"));
        }
        let session = fixture.session();
        let resumed = fixture.run(&["session", "connect", &session]);
        for received in [first, resumed] {
            let arguments = args(&received);
            let root = if provider == "codex" {
                received["params"]["config"]["sandbox_workspace_write.writable_roots"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_str)
                    .find(|root| {
                        Path::new(root).canonicalize().ok()
                            == fixture.repo.path().canonicalize().ok()
                    })
                    .unwrap()
            } else {
                arguments[arguments
                    .iter()
                    .position(|arg| *arg == "--add-dir")
                    .unwrap()
                    + 1]
            };
            assert_eq!(
                Path::new(root).canonicalize().unwrap(),
                fixture.repo.path().canonicalize().unwrap()
            );
            assert_eq!(
                Path::new(received["cwd"].as_str().unwrap())
                    .canonicalize()
                    .unwrap(),
                worktree.canonicalize().unwrap()
            );
        }
    }
}

#[test]
fn codex_headless_start_and_resume_receive_current_policy_on_the_wire() {
    let fixture = Fixture::new("codex:first-model");
    let machine = fixture.home.path().join("machine");
    support::register_codex_account(&machine);
    fixture.create_wave();
    let script = support::codex_app_server_script("done", "").replace(
        "read -r thread_start",
        "read -r thread_start\nprintf '%s' \"$thread_start\" > \"$HOME/received.json\"",
    );
    fs::write(fixture.home.path().join("bin/codex"), script).unwrap();
    fixture.settings("codex:first-model", true, false);
    let first = fixture.run(&["-b", "--wave", "fixture", ":", "first turn"]);
    assert_eq!(first["method"], "thread/start");
    assert_eq!(first["params"]["model"], "first-model");
    assert_eq!(first["params"]["sandbox"], "danger-full-access");
    assert_eq!(first["params"]["approvalPolicy"], "never");
    assert!(first["params"]["developerInstructions"]
        .as_str()
        .unwrap()
        .contains("<lf:user>"));
    let session = fixture.session();
    fixture.settings("codex:current-model", false, false);
    let resumed = fixture.run(&[
        "-b",
        "session",
        "connect",
        &session,
        "--message",
        "next turn",
    ]);
    assert_eq!(resumed["method"], "thread/resume");
    assert_eq!(resumed["params"]["threadId"], "thread-test");
    assert_eq!(resumed["params"]["model"], "current-model");
    assert_eq!(resumed["params"]["sandbox"], "workspace-write");
    assert_eq!(resumed["params"]["approvalPolicy"], "never");
}

#[test]
fn opencode_headless_start_and_resume_preserve_history_and_reload_policy() {
    let fixture = Fixture::new("opencode:opencode/first-model");
    fixture.create_wave();
    let script = include_str!("support/opencode_server.py").replace("__WAIT__", "False").replace(
        "            if evidence := os.environ.get(\"LF_TEST_REPLAY_EVIDENCE\"):",
        "            (HOME / 'received.json').write_text(json.dumps({'session':self.path.split('/')[2], 'request':body, 'config':json.loads(os.environ['OPENCODE_CONFIG_CONTENT'])}))\n            if evidence := os.environ.get(\"LF_TEST_REPLAY_EVIDENCE\"):"
    );
    fs::write(fixture.home.path().join("bin/opencode"), script).unwrap();
    fixture.settings("opencode:opencode/first-model", true, false);
    let first = fixture.run(&["-b", "--wave", "fixture", ":", "first turn"]);
    assert_eq!(first["request"]["model"]["modelID"], "first-model");
    assert_eq!(first["config"]["permission"]["*"], "allow");
    let session = fixture.session();
    fixture.settings("opencode:opencode/current-model", false, false);
    let resumed = fixture.run(&[
        "-b",
        "session",
        "connect",
        &session,
        "--message",
        "next turn",
    ]);
    assert_eq!(resumed["session"], first["session"]);
    assert_eq!(resumed["request"]["model"]["modelID"], "current-model");
    assert!(resumed["config"]["permission"].is_null());
    assert!(!resumed["request"]["parts"]
        .to_string()
        .contains("first turn"));
    assert!(resumed["request"]["parts"]
        .to_string()
        .contains("next turn"));
}

#[test]
fn native_resume_survives_missing_optional_capture_payload() {
    let fixture = Fixture::new("claude");
    let first = fixture.run(&["-i", ":", "hello"]);
    let native = args(&first)[args(&first)
        .iter()
        .position(|arg| *arg == "--session-id")
        .unwrap()
        + 1]
    .to_owned();
    let session = fixture.session();
    // Remove the optional on-disk prompt material, leaving durable native identity intact.
    let runs = fixture.home.path().join("machine/runs");
    for prefix in fs::read_dir(runs).unwrap() {
        for capture in fs::read_dir(prefix.unwrap().path()).unwrap() {
            let manifest = capture.unwrap().path().join("manifest.json");
            if manifest.is_file() {
                fs::remove_file(manifest).unwrap();
            }
        }
    }
    fixture.settings("claude:haiku", true, false);
    let resumed = fixture.run(&["session", "connect", &session]);
    pair(&resumed, "--resume", &native);
    pair(&resumed, "--model", "haiku");
    bypass(&resumed, "claude", true);
    assert!(!args(&resumed).contains(&"--append-system-prompt-file"));
}

#[test]
fn wave_targets_prepare_reuse_and_open_in_the_selected_checkout_in_both_formats() {
    let fixture = Fixture::new("claude:sonnet");
    fixture.repo.stage_all();
    fixture.repo.commit("provider settings");
    fixture.create_wave();
    let other = TestRepo::new();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("machine/loopflow.db"))
            .unwrap();
    let wave = store.list_waves(None).unwrap().into_iter().next().unwrap();
    let wave_id = wave.id().as_str();
    let missing = fixture
        .command()
        .current_dir(other.path())
        .args(["resume", "--wave", wave_id, "--prepare"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
    let first = fixture
        .command()
        .current_dir(other.path())
        .args(["connect", "--wave", wave_id, "--prepare", "--json"])
        .output()
        .unwrap();
    success(&first);
    let prepared: Value = serde_json::from_slice(&first.stdout).unwrap();
    let id = prepared["id"].as_str().unwrap();
    let saved = store.session(id).unwrap().unwrap();
    let again = fixture
        .command()
        .args(["--wave", wave_id, "session", "connect", "--prepare"])
        .output()
        .unwrap();
    success(&again);
    assert!(String::from_utf8(again.stdout).unwrap().contains(id));
    let explicit = fixture
        .command()
        .args(["session", "connect", id, "--prepare", "--json"])
        .output()
        .unwrap();
    success(&explicit);
    assert_eq!(
        serde_json::from_slice::<Value>(&explicit.stdout).unwrap()["id"],
        id
    );
    assert!(!fixture.home.path().join("received.json").exists());
    fs::create_dir_all(saved.cwd.join(".lf")).unwrap();
    fs::write(saved.cwd.join(".lf/config.yaml"), "agent: claude:haiku\nyolo: true\nchrome: true\npaste: false\ndiff: false\ndiff_files: false\n").unwrap();
    for argv in [
        vec!["session", "connect", "--wave", wave_id],
        vec!["connect", "--wave", wave_id, "--json"],
        vec!["--wave", wave_id, "resume"],
        vec!["connect", id, "--takeover"],
    ] {
        success(
            &fixture
                .command()
                .current_dir(other.path())
                .args(argv)
                .output()
                .unwrap(),
        );
        let received = fixture.received();
        assert_eq!(
            Path::new(received["cwd"].as_str().unwrap())
                .canonicalize()
                .unwrap(),
            saved.cwd.canonicalize().unwrap()
        );
        bypass(&received, "claude", true);
        pair(&received, "--model", "haiku");
        assert_eq!(
            store.session(id).unwrap().unwrap().artifact_key,
            saved.artifact_key
        );
    }
    let previous = fixture.received();
    let native = args(&previous)
        .windows(2)
        .find(|pair| pair[0] == "--resume")
        .unwrap()[1]
        .to_owned();
    let continued = fixture.run(&[
        "-b",
        "resume",
        "--wave",
        wave_id,
        "--message",
        "next Wave turn",
    ]);
    pair(&continued, "--resume", &native);
    pair(&continued, "--model", "haiku");
    assert!(continued["input"].to_string().contains("next Wave turn"));
    let takeover = fixture
        .command()
        .args(["connect", id, "--takeover", "--prepare", "--json"])
        .output()
        .unwrap();
    success(&takeover);
    let takeover: Value = serde_json::from_slice(&takeover.stdout).unwrap();
    assert_eq!(takeover["id"], id);
    assert!(takeover["open_argv"]
        .as_array()
        .unwrap()
        .iter()
        .any(|arg| arg == "--takeover"));
    let next = fixture
        .command()
        .args(["session", "new", id, "--json"])
        .output()
        .unwrap();
    success(&next);
    let next: Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_ne!(next["id"], id);
    assert!(store.session(id).unwrap().unwrap().completed_at.is_some());
    assert_eq!(
        store
            .session(next["id"].as_str().unwrap())
            .unwrap()
            .unwrap()
            .cwd,
        saved.cwd
    );
}

#[test]
fn contextual_resume_uses_repository_recency_but_unowned_worktrees_stay_local() {
    let fixture = Fixture::new("claude:sonnet");
    fixture.repo.stage_all();
    fixture.repo.commit("provider settings");
    let linked = fixture.home.path().join("linked");
    success(
        &Command::new("git")
            .args(["worktree", "add", "-b", "linked"])
            .arg(&linked)
            .current_dir(fixture.repo.path())
            .output()
            .unwrap(),
    );
    fixture.run(&["-i", ":", "main conversation"]);
    let main_id = fixture.session();
    success(
        &fixture
            .command()
            .current_dir(&linked)
            .args(["-i", ":", "linked conversation"])
            .output()
            .unwrap(),
    );
    let db = rusqlite::Connection::open(fixture.home.path().join("machine/loopflow.db")).unwrap();
    let linked_id: String = db
        .query_row(
            "SELECT id FROM agent_sessions WHERE id != ?1",
            [&main_id],
            |row| row.get(0),
        )
        .unwrap();
    // Native human recency wins over newer assistant activity and repository primaries.
    let native = args(&fixture.received())
        .windows(2)
        .find(|pair| pair[0] == "--session-id")
        .unwrap()[1]
        .to_owned();
    let history = fixture
        .home
        .path()
        .join(format!(".claude/projects/fixture/{native}.jsonl"));
    fs::create_dir_all(history.parent().unwrap()).unwrap();
    fs::write(
        &history,
        serde_json::json!({"type":"user", "userType":"external", "sessionId": native, "timestamp":"2099-01-01T00:00:00.000Z", "message":{"content":"continue"}}).to_string() + "\n",
    )
    .unwrap();
    // Repository primary is a candidate, never the main-checkout default.
    success(
        &fixture
            .command()
            .args(["session", "ensure", "--json"])
            .output()
            .unwrap(),
    );
    let selected = fixture
        .command()
        .args(["resume", "--prepare", "--json"])
        .output()
        .unwrap();
    success(&selected);
    assert_eq!(
        serde_json::from_slice::<Value>(&selected.stdout).unwrap()["id"],
        linked_id
    );
    db.execute(
        "UPDATE agent_sessions SET created_at=99999999999 WHERE id=?1",
        [&main_id],
    )
    .unwrap();
    let selected = fixture
        .command()
        .current_dir(&linked)
        .args(["session", "resume", "--prepare", "--json"])
        .output()
        .unwrap();
    success(&selected);
    assert_eq!(
        serde_json::from_slice::<Value>(&selected.stdout).unwrap()["id"],
        linked_id
    );
    db.execute(
        "UPDATE agent_sessions SET created_at=1 WHERE id=?1",
        [&main_id],
    )
    .unwrap();
    // Selection in main launches with the linked checkout's current policy.
    fs::write(
        linked.join(".lf/config.yaml"),
        "agent: claude:haiku\nyolo: true\npaste: false\ndiff: false\ndiff_files: false\n",
    )
    .unwrap();
    let resumed = fixture.run(&["resume"]);
    pair(&resumed, "--resume", &native);
    pair(&resumed, "--model", "haiku");
    bypass(&resumed, "claude", true);
    assert_eq!(
        Path::new(resumed["cwd"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        linked.canonicalize().unwrap()
    );
    for argv in [
        vec!["resume", &linked_id],
        vec!["connect", &linked_id, "--wave", "fixture"],
        vec!["--task", "missing", "connect", &linked_id],
        vec!["session", "replace", &linked_id],
        vec!["connect", &linked_id, "--replace"],
    ] {
        assert!(!fixture
            .command()
            .args(argv)
            .output()
            .unwrap()
            .status
            .success());
    }
}

#[test]
fn missing_native_history_fails_without_replacing_the_selected_conversation() {
    let fixture = Fixture::new("claude:sonnet");
    fixture.run(&["-i", ":", "existing history"]);
    let id = fixture.session();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&fixture.home.path().join("machine/loopflow.db"))
            .unwrap();
    let saved = store.session(&id).unwrap().unwrap();
    let received = fixture.received();
    let native = args(&received)
        .windows(2)
        .find(|pair| pair[0] == "--session-id")
        .unwrap()[1]
        .to_owned();
    fs::write(
        fixture.home.path().join("bin/claude"),
        r#"#!/bin/sh
if [ "$1" = --version ]; then exit 0; fi
printf '%s\n' "$@" > "$HOME/failed_args"
echo 'Native history is missing' >&2
exit 3
"#,
    )
    .unwrap();
    for argv in [vec!["resume"], vec!["connect", &id]] {
        let result = fixture.command().args(argv).output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("Native history is missing"));
        let current = store.session(&id).unwrap().unwrap();
        assert_eq!(current.artifact_key, saved.artifact_key);
        let attempted = fs::read_to_string(fixture.home.path().join("failed_args")).unwrap();
        assert!(attempted
            .lines()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair == ["--resume", native.as_str()]));
        let db =
            rusqlite::Connection::open(fixture.home.path().join("machine/loopflow.db")).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
