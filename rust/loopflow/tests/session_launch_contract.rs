//! Public launch and continuation contract, with providers recording their real process inputs.
#![cfg(unix)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use loopflow_test_support::{MachineServer, TestRepo};
use serde_json::Value;

/// A Claude stand-in that counts its dispatches and reports a `rejected`
/// rate limit before each result; `refused` also fails the turn.
fn rate_limited_fixture(refused: bool) -> Fixture {
    let fixture = Fixture::new("claude:opus");
    let provider = fixture.home.path().join("bin/claude");
    let mut source = fs::read_to_string(&provider).unwrap().replace(
        "    message = json.loads(sys.stdin.readline())",
        "    with open(os.environ['HOME']+'/dispatches', 'a') as out:\n        out.write('dispatch\\n')\n    message = json.loads(sys.stdin.readline())",
    ).replace(
        "    print(json.dumps({'type':'result'",
        "    print(json.dumps({'type':'rate_limit_event','rate_limit_info':{'status':'rejected','rate_limit_type':'five_hour'}}), flush=True)\n    print(json.dumps({'type':'result'",
    );
    if refused {
        source = source.replace(
            "'subtype':'success','result':'done'",
            "'subtype':'error_during_execution','is_error':True,'result':\"You've hit your limit.\\nResets 3pm (America/Los_Angeles).\"",
        );
    }
    fs::write(provider, source).unwrap();
    fixture
}

impl Fixture {
    fn dispatches(&self) -> String {
        fs::read_to_string(self.home.path().join("dispatches")).unwrap()
    }
}

#[test]
fn a_graph_step_refused_on_its_pinned_agent_shows_the_message_and_waits() {
    let fixture = rate_limited_fixture(true);
    fixture.repo.create_file(".lf/skills/probe.md", "probe");
    fixture
        .repo
        .create_file(".lf/graphs/refusal-probe.yaml", "- probe\n");
    // The real waits are minutes long: read until the wait is announced, then
    // stop the run. Its schedule and final stop are unit-tested.
    let mut child = fixture
        .command()
        .args(["-b", "-a", "claude:opus", "graph", "refusal-probe"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    let stderr = std::io::BufReader::new(child.stderr.take().unwrap());
    let (lines, announced) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in stderr.lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                return;
            }
        }
    });
    let mut stderr = String::new();
    while !stderr.contains("before trying again") {
        let line = announced
            .recv_timeout(std::time::Duration::from_secs(60))
            .unwrap_or_else(|_| panic!("no usage-limit wait was announced: {stderr}"));
        stderr.push_str(&line);
        stderr.push('\n');
    }
    // The Graph and its step share the process group started above.
    let group = i32::try_from(child.id()).unwrap();
    Command::new("/bin/kill")
        .args(["-TERM", "--", &format!("-{group}")])
        .status()
        .unwrap();
    child.wait().unwrap();

    assert!(stderr.contains("You've hit your limit."), "{stderr}");
    assert!(
        stderr.contains("Resets 3pm (America/Los_Angeles)."),
        "{stderr}"
    );
    assert!(stderr.contains("Waiting until "), "{stderr}");
    // It waits before a second dispatch, never instead of the first.
    assert_eq!(fixture.dispatches(), "dispatch\n");
}

#[test]
fn headless_work_outside_a_graph_stops_at_a_usage_limit_with_the_provider_message() {
    let fixture = rate_limited_fixture(true);
    let output = fixture
        .command()
        .args(["-b", "-a", "claude:opus", ":", "say done"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("You've hit your limit."), "{stderr}");
    assert!(!stderr.contains("Waiting until "), "{stderr}");
    assert_eq!(fixture.dispatches(), "dispatch\n");
}

#[test]
fn a_turn_the_provider_finishes_succeeds_despite_a_rejected_rate_limit_event() {
    let fixture = rate_limited_fixture(false);
    let output = fixture
        .command()
        .args(["-b", "-a", "claude:opus", ":", "say done"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    success(&output);
    assert_eq!(fixture.dispatches(), "dispatch\n");
}

struct Fixture {
    /// Task and Wave launches start their agent only once it answers.
    _server: MachineServer,
    repo: TestRepo,
    home: tempfile::TempDir,
}

impl Fixture {
    fn new(provider: &str) -> Self {
        let home = tempfile::tempdir().unwrap();
        support::seed_store(&home.path().join("machine"));
        let fixture = Self {
            _server: MachineServer::start(env!("CARGO_BIN_EXE_lf"), &home.path().join("machine")),
            repo: TestRepo::new(),
            home,
        };
        fs::create_dir(fixture.home.path().join("bin")).unwrap();
        fixture.repo.create_file(
            ".lf/config.yaml",
            &format!("agent: {provider}\ndiff: false\ndiff_files: false\npaste: false\n"),
        );
        for harness in ["claude", "codex", "opencode"] {
            let script = fixture.home.path().join("bin").join(harness);
            let source = if harness == "opencode" {
                include_str!("../../loopflow-execution/tests/support/opencode_cli.py")
                    .replace("__WAIT__", "False")
            } else {
                r#"#!/usr/bin/python3
import json, os, sys, runpy
from pathlib import Path
args = sys.argv[1:]
if '--version' in args:
    print('fixture'); sys.exit(0)
if '--dangerously-bypass-hook-trust' in args:
    sys.exit(2)
# The launch's login probe; `HOME/signed-out` models a Machine with no login
# of its own, which a forwarded Claude setup token signs in for the run.
signed_out = os.path.exists(os.environ['HOME']+'/signed-out') and not os.environ.get('CLAUDE_CODE_OAUTH_TOKEN')
if args[:2] == ['auth', 'status']:
    print(json.dumps({'loggedIn': not signed_out})); sys.exit(1 if signed_out else 0)
if args[:2] == ['login', 'status']:
    print('Not logged in' if signed_out else 'Logged in using ChatGPT'); sys.exit(1 if signed_out else 0)
record = {'args': args, 'cwd': os.getcwd(), 'opencode': os.environ.get('OPENCODE_CONFIG_CONTENT'), 'participant': os.environ.get('LF_USER_NAME'), 'account': os.environ.get('LF_PROVIDER_ACCOUNT_ID'), 'codex_home': os.environ.get('CODEX_HOME'), 'forwarded': os.environ.get('DEPLOY_TOKEN')}
if os.path.basename(sys.argv[0]) in ('claude','codex'):
    provider = os.path.basename(sys.argv[0])
    native = os.environ['CLAUDE_CONFIG_DIR' if provider == 'claude' else 'CODEX_HOME']
    auth = native + ('/.credentials.json' if provider == 'claude' else '/auth.json')
    record['native_auth'] = json.load(open(auth)) if os.path.isfile(auth) else None

if '--append-system-prompt-file' in args:
    record['context'] = open(args[args.index('--append-system-prompt-file')+1]).read()
stream = '--input-format' in args
if stream:
    message = json.loads(sys.stdin.readline())
    record['input'] = message
elif os.path.basename(sys.argv[0]) == 'claude':
    if '--session-id' in args:
        project = Path(native) / 'projects' / 'fixture'
        project.mkdir(parents=True, exist_ok=True)
        (project / (args[args.index('--session-id')+1] + '.jsonl')).write_text('{}\n')
    # Resume opens history with no seed; a held terminal keeps stdin open.
    record['input'] = '' if '--resume' in args else sys.stdin.read()
with open(os.environ['HOME']+'/received.json.tmp', 'w') as out:
    json.dump(record, out)
os.replace(os.environ['HOME']+'/received.json.tmp', os.environ['HOME']+'/received.json')
if stream:
    token = args[args.index('--resume')+1] if '--resume' in args else 'stream-fixture'
    print(json.dumps({'type':'system','subtype':'init','session_id':token}), flush=True)
    message['session_id'] = token
    print(json.dumps(message), flush=True)
    print(json.dumps({'type':'result','session_id':token,'subtype':'success','result':'done'}), flush=True)
elif os.path.basename(sys.argv[0]) == 'codex':
    sys.path.insert(0, SUPPORT_DIRECTORY)
    runpy.run_path(SUPPORT_DIRECTORY+'/native_codex.py', run_name='__main__')
"#.replace("SUPPORT_DIRECTORY", &serde_json::to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../loopflow-execution/tests/support")).unwrap())
            };
            fs::write(&script, source).unwrap();
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
            received["server"] = serde_json::from_slice(
                &fs::read(machine.join("native-server-config.json")).unwrap(),
            )
            .unwrap();
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
        let store =
            loopflow_execution::store::open_sqlite(&self.home.path().join("machine/loopflow.db"))
                .unwrap();
        loopflow_execution::work::wave::bootstrap::upsert_wave(
            &store,
            &loopflow_execution::work::wave::Wave::new(
                loopflow_model::id::WaveId::new(),
                "fixture".into(),
                loopflow_execution::repository::discover(self.repo.path())
                    .unwrap()
                    .to_string(),
            ),
        )
        .unwrap();
    }

    fn settings(&self, provider: &str, yolo: bool, chrome: bool) {
        self.repo.create_file(".lf/config.yaml", &format!("agent: {provider}\nyolo: {yolo}\nchrome: {chrome}\ndiff: false\ndiff_files: false\npaste: false\n"));
    }
}

/// Each OpenCode launch's private stderr log, found where it is retained.
fn files_named(dir: &Path, file: &str) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_named(&path, file));
        } else if path.file_name().is_some_and(|name| name == file) {
            found.push(path);
        }
    }
    found
}

/// The Machine's store, with Session history attached as `history` and turns as `usage`.
fn machine_db(home: &Path) -> rusqlite::Connection {
    let store = home.join("machine/loopflow.db");
    let db = rusqlite::Connection::open(&store).unwrap();
    db.execute(
        "ATTACH ?1 AS history",
        [loopflow_storage::history::history_path(&store)
            .to_string_lossy()
            .as_ref()],
    )
    .unwrap();
    db.execute(
        "ATTACH ?1 AS usage",
        [loopflow_storage::history::history_path(&store)
            .to_string_lossy()
            .as_ref()],
    )
    .unwrap();
    db
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
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
            if enabled {
                assert_eq!(value["params"]["sandbox"], "danger-full-access", "{value}");
            } else {
                assert!(value["params"].get("sandbox").is_none(), "{value}");
            }
            assert_eq!(
                value["params"]["approvalPolicy"],
                // Unset leaves the person's own Codex configuration in charge.
                if enabled {
                    Value::from("never")
                } else {
                    Value::Null
                },
                "{value}"
            );
            assert!(!args(value).contains(&"--dangerously-bypass-approvals-and-sandbox"));
        }
        "opencode" => {
            let config = &value["server"];
            assert_eq!(
                config["permissions"][0]["effect"].as_str() == Some("allow"),
                enabled,
                "{value}"
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn opencode_diagnostics_stay_off_the_terminal_on_launch_and_resume() {
    let fixture = Fixture::new("opencode");
    fs::write(
        fixture.home.path().join("bin/opencode"),
        include_str!("../../loopflow-execution/tests/support/opencode_cli.py").replace("__WAIT__", "False")
            .replace("    _record_launch()\n    if False:", r#"    _record_launch()
    mode = 'resume' if (HOME / 'opened').exists() else 'launch'
    (HOME / 'opened').touch()
    os.write(1, b'native composer\n')
    for attempt in range(3):
        os.write(2, f'level=ERROR message="stream error" mode={mode} attempt={attempt} error="Model access is disabled"\n'.encode())
    subprocess.run(['/bin/sh', '-c', 'printf "background diagnostic\\n" >&2'], check=True)
    os.write(2, b'non-UTF8: \xff\x1b[2Junterminated')
    if False:"#),
    )
    .unwrap();

    let first = fixture
        .command()
        .args(["-i", ":", "launch"])
        .output()
        .unwrap();
    let session = fixture.session();
    let resumed = fixture
        .command()
        .args(["session", "resume", &session])
        .output()
        .unwrap();
    for (output, mode) in [(&first, "launch"), (&resumed, "resume")] {
        success(output);
        assert_eq!(output.stdout, b"native composer\n");
        let stderr = String::from_utf8(output.stderr.clone()).unwrap();
        assert!(!stderr.contains("stream error"), "{stderr}");
        assert!(!stderr.contains("background diagnostic"), "{stderr}");
        assert!(!stderr.contains("diagnostics:"), "{stderr}");
        let logs = files_named(fixture.home.path(), "opencode.stderr.log");
        assert_eq!(logs.len(), 1, "{logs:?}");
        let path = &logs[0];
        let log = fs::read(path).unwrap();
        let text = String::from_utf8_lossy(&log);
        for attempt in 0..3 {
            assert!(
                text.contains(&format!("mode={mode} attempt={attempt}")),
                "{text}"
            );
        }
        assert!(text.contains("Model access is disabled"));
        assert!(text.contains("background diagnostic"));
        assert!(log.ends_with(b"non-UTF8: \xff\x1b[2Junterminated"));
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(fixture.session(), session);
}

#[test]
fn opencode_startup_failure_reports_its_exit_and_keeps_diagnostics() {
    let fixture = Fixture::new("opencode");
    fs::write(
        fixture.home.path().join("bin/opencode"),
        "#!/bin/sh\nprintf 'startup failure without a session\\n' >&2\nexit 23\n",
    )
    .unwrap();
    let output = fixture
        .command()
        .args(["-i", ":", "launch"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("opencode exited with exit status: 23"),
        "{stderr}"
    );
    let logs = files_named(fixture.home.path(), "opencode.stderr.log");
    assert_eq!(logs.len(), 1, "{logs:?}");
    assert_eq!(
        fs::read(&logs[0]).unwrap(),
        b"startup failure without a session\n"
    );
}

#[test]
fn terminal_launch_and_resume_use_current_policy_and_preserve_native_identity() {
    for provider in ["claude", "codex", "opencode"] {
        let fixture = Fixture::new(provider);
        fixture.settings(provider, true, true);
        if provider != "opencode" {
            let native = fixture.home.path().join(format!(".{provider}"));
            fs::create_dir_all(&native).unwrap();
            let file = if provider == "claude" {
                ".credentials.json"
            } else {
                "auth.json"
            };
            fs::write(native.join(file), br#"{"fixture_login":"A"}"#).unwrap();
            let machine = fixture.home.path().join("machine");
            let saved = machine.join(format!("accounts/{provider}/retired"));
            fs::create_dir_all(&saved).unwrap();
            fs::write(saved.join(file), br#"{"fixture_login":"B"}"#).unwrap();
            rusqlite::Connection::open(machine.join("loopflow.db")).unwrap().execute(
                "INSERT INTO provider_accounts(provider,account_id,home,credential_state,cooldown_until,created_at,updated_at) VALUES(?1,'retired',?2,'missing',4000000000,1,1)",
                rusqlite::params![provider,saved.to_str().unwrap()]).unwrap();
        }
        let session = (provider == "codex").then(|| {
            crate::support::prepare_native_conversation(
                &fixture.home.path().join("machine/loopflow.db"),
                fixture.repo.path(),
                "codex",
                None,
                "--- keep spaces and 雪",
            )
        });
        let launch = match &session {
            Some(session) => vec!["-i", "resume", session],
            None => vec!["-i", ":", "--", "--- keep spaces and 雪"],
        };
        let first = fixture.run(&launch);
        bypass(&first, provider, true);
        if provider != "opencode" {
            assert_eq!(first["native_auth"]["fixture_login"], "A");
        }
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
            args(&first)[args(&first)
                .iter()
                .position(|arg| *arg == "--session")
                .unwrap()
                + 1]
            .to_owned()
        } else {
            first["token"].as_str().unwrap().to_string()
        };
        if provider == "claude" {
            assert!(args(&first).contains(&"--chrome"));
        }
        let session = fixture.session();
        let current_model = if provider == "opencode" {
            "opencode/current-model"
        } else {
            "current-model"
        };
        fixture.settings(&format!("{provider}:{current_model}"), false, false);
        let resumed = fixture.run(&["session", "resume", &session]);
        bypass(&resumed, provider, false);
        if provider != "opencode" {
            assert_eq!(resumed["native_auth"]["fixture_login"], "A");
            assert!(resumed["account"].is_null());
        }
        assert!(args(&resumed).contains(&token.as_str()));
        assert!(!args(&resumed)
            .iter()
            .any(|arg| arg.contains("--- keep spaces")));
        assert!(!args(&resumed).contains(&"--session-id"));
        if provider == "codex" {
            assert_eq!(resumed["params"]["model"], "current-model");
            assert_eq!(resumed["turns"].as_array().unwrap().len(), 2);
            assert!(!resumed["turns"][1].to_string().contains("--- keep spaces"));
        } else if provider == "opencode" {
            assert!(!args(&resumed).contains(&"--model"));
        } else {
            pair(&resumed, "--model", "current-model");
        }
        if provider == "claude" {
            assert!(args(&resumed).contains(&"--no-chrome"));
            assert_eq!(first["context"], resumed["context"]);
        }
        assert_eq!(fixture.session(), session);
    }
}

#[test]
fn headless_launch_and_graph_keep_current_options() {
    let fixture = Fixture::new("claude:sonnet");
    fixture
        .repo
        .create_file(".lf/skills/probe.md", "INITIAL_SKILL_ONLY");
    fixture.settings("claude:sonnet", true, true);
    fixture.repo.stage_all();
    fixture.repo.commit("Define the probe skill");
    fixture.create_wave();
    let first = fixture.run(&["-b", "--wave", "fixture", "probe", "first turn"]);
    pair(&first, "--input-format", "stream-json");
    assert!(first["input"].to_string().contains("INITIAL_SKILL_ONLY"));
    fixture.settings("claude:haiku", false, false);
    fixture
        .repo
        .create_file(".lf/graphs/launch-probe.yaml", "- probe\n");
    fixture.repo.stage_all();
    fixture.repo.commit("Define the probe Graph");
    let flowed = fixture.run(&["--yolo", "--chrome", "on", "graph", "launch-probe"]);
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("require a headless Run"));
    assert!(!fixture.home.path().join("received.json").exists());

    fixture.run(&["-i", ":", "create history"]);
    let session = fixture.session();
    fs::remove_file(fixture.home.path().join("received.json")).unwrap();
    let output = fixture
        .command()
        .args(["--output-schema", "{}", "session", "resume", &session])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("lf -b steer"));
    assert!(!fixture.home.path().join("received.json").exists());
    assert_eq!(fixture.session(), session);
}

#[test]
fn machine_dispatch_resolves_destination_policy_and_forwards_launch_and_resume_overrides() {
    let local = Fixture::new("codex:local-model");
    let remote = Fixture::new("claude:remote-model");
    local.settings("codex:local-model", true, true);
    let remote_auth = remote.home.path().join(".claude");
    fs::create_dir_all(&remote_auth).unwrap();
    fs::write(
        remote_auth.join(".credentials.json"),
        br#"{"fixture_login":"remote-A"}"#,
    )
    .unwrap();

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
            "#!/usr/bin/python3\nimport os, sys\nassert 'credentials' not in sys.argv[-1], 'unexpected credential transfer'\nos.chdir({cwd})\nos.execve('/bin/bash', ['bash', '-c', sys.argv[-1]], {environment})\n",
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
    assert_eq!(first["native_auth"]["fixture_login"], "remote-A");
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
    let resumed = dispatch(&["session", "resume", &session]);
    bypass(&resumed, "claude", false);
    assert_eq!(resumed["native_auth"]["fixture_login"], "remote-A");
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
        "resume",
        &session,
    ]);
    bypass(&explicit, "claude", true);
    pair(&explicit, "--resume", native);
    pair(&explicit, "--model", "haiku");
    assert!(args(&explicit).contains(&"--chrome"));
    assert_eq!(remote.session(), session);
}

/// `remote` as the Machine `fixture` of `local`, behind a stand-in `ssh` that
/// logs its argv; returns the log.
fn remote_machine(local: &Fixture, remote: &Fixture) -> std::path::PathBuf {
    let remote_bin = remote.home.path().join(".local/bin");
    fs::create_dir_all(&remote_bin).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_lf"), remote_bin.join("lf")).unwrap();
    let environment = serde_json::json!({
        "HOME": remote.home.path(),
        "LF_HOME": remote.home.path().join("machine"),
        "LF_BIN": env!("CARGO_BIN_EXE_lf"),
        "PATH": format!("{}:/usr/bin:/bin", remote.home.path().join("bin").display()),
        "CLAUDE_CONFIG_DIR": remote.home.path().join(".claude"),
        "CODEX_HOME": remote.home.path().join(".codex"),
    });
    let argv_log = local.home.path().join("ssh-argv");
    let ssh = local.home.path().join("bin/ssh");
    fs::write(
        &ssh,
        format!(
            "#!/usr/bin/python3\nimport json, os, sys\nopen({log}, 'a').write(json.dumps(sys.argv) + '\\n')\nos.chdir({cwd})\nos.execve('/bin/bash', ['bash', '-c', sys.argv[-1]], {environment})\n",
            log = serde_json::to_string(&argv_log).unwrap(),
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
    argv_log
}

/// Per-run secrets `forward` names reach a remote launch by name only: never
/// in argv, logs or either store.
#[test]
fn forwarded_secrets_reach_a_remote_launch_only_by_name() {
    const TOKEN: &str = "fixture-deploy-token-secret";
    const AUTH: &str = r#"{"refresh_token":"fixture-auth-json-secret"}"#;
    let local = Fixture::new("claude:opus");
    let remote = Fixture::new("claude:opus");
    fs::write(
        local.home.path().join("machine/config.yaml"),
        "forward:\n  env: [DEPLOY_TOKEN]\n  files: [~/.config/deploy/auth.json]\n",
    )
    .unwrap();
    fs::create_dir_all(local.home.path().join(".config/deploy")).unwrap();
    fs::write(local.home.path().join(".config/deploy/auth.json"), AUTH).unwrap();
    let argv_log = remote_machine(&local, &remote);
    let launch = |token: Option<&str>| {
        let mut command = local.command();
        if let Some(token) = token {
            command.env("DEPLOY_TOKEN", token);
        }
        command
            .args(["--machine", "fixture", "-b", ":", "say done"])
            .output()
            .unwrap()
    };

    // An environment file a failed launch never read is swept by the next delivery.
    let stale = remote.home.path().join(".lf/forward/unread");
    fs::create_dir_all(stale.parent().unwrap()).unwrap();
    fs::write(&stale, "export DEPLOY_TOKEN=old\n").unwrap();
    fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600))
        .unwrap();
    let output = launch(Some(TOKEN));
    success(&output);
    assert_eq!(remote.received()["forwarded"], TOKEN);
    let copied = remote.home.path().join(".config/deploy/auth.json");
    assert_eq!(fs::read_to_string(&copied).unwrap(), AUTH);
    assert_eq!(
        fs::metadata(&copied).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(copied.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    // The forwarded environment's transient file is gone once the launch read it.
    assert_eq!(
        fs::read_dir(remote.home.path().join(".lf/forward"))
            .unwrap()
            .count(),
        0
    );
    let mut seen = vec![
        fs::read(&argv_log).unwrap(),
        output.stdout.clone(),
        output.stderr.clone(),
    ];
    for fixture in [&local, &remote] {
        for file in files_named(&fixture.home.path().join("machine"), "loopflow.db")
            .into_iter()
            .chain(files_named(
                &fixture.home.path().join("machine"),
                "loopflow.db-wal",
            ))
            .chain(files_named(&fixture.home.path().join("machine"), "lf.log"))
        {
            seen.push(fs::read(file).unwrap());
        }
    }
    for bytes in &seen {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains(TOKEN), "a forwarded value leaked: {text}");
        assert!(
            !text.contains("fixture-auth-json-secret"),
            "a file leaked: {text}"
        );
    }

    // A destination already holding the same file says nothing about it.
    let output = launch(Some(TOKEN));
    success(&output);
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("keeps its own"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    // A destination with its own login keeps it.
    fs::write(&copied, "destination login").unwrap();
    let output = launch(Some(TOKEN));
    success(&output);
    assert_eq!(fs::read_to_string(&copied).unwrap(), "destination login");
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("keeps its own ~/.config/deploy/auth.json"));

    // A named value this Machine cannot supply stops before anything reaches SSH.
    fs::remove_file(&argv_log).unwrap();
    let output = launch(None);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("forward.env names DEPLOY_TOKEN"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!argv_log.exists());

    // A command that launches no agent carries nothing, so it needs no value.
    let output = local
        .command()
        .args(["--machine", "fixture", "machine", "self", "--json"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("forward.env"), "{stderr}");
    assert!(argv_log.exists(), "{stderr}");
}

/// A Machine whose client reports no login refuses the launch before any agent
/// starts, naming the command that signs it in where the person reads it.
#[test]
fn a_signed_out_machine_refuses_the_launch_with_the_command_that_signs_it_in() {
    let local = Fixture::new("codex");
    let remote = Fixture::new("codex");
    remote_machine(&local, &remote);
    fs::write(remote.home.path().join("signed-out"), "").unwrap();
    let output = local
        .command()
        .args(["--machine", "fixture", "-b", "-a", "codex", ":", "say done"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("fixture has no Codex login; run `lf machine login codex fixture`"),
        "{stderr}"
    );
    assert!(!remote.home.path().join("received.json").exists());

    // Claude on another Machine is the forwarded token; with it, the launch goes ahead.
    let config = local.home.path().join("machine/config.yaml");
    let registered = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        format!("{registered}forward:\n  env: [CLAUDE_CODE_OAUTH_TOKEN]\n"),
    )
    .unwrap();
    let claude = |token: &str| {
        local
            .command()
            .env("CLAUDE_CODE_OAUTH_TOKEN", token)
            .args([
                "--machine",
                "fixture",
                "-b",
                "-a",
                "claude",
                ":",
                "say done",
            ])
            .output()
            .unwrap()
    };
    // Named but with no value here, Claude reads signed out there.
    let output = claude("");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains(
            "fixture has no Claude login; launch where CLAUDE_CODE_OAUTH_TOKEN has a value \
             (`doppler secrets set CLAUDE_CODE_OAUTH_TOKEN`, or run under `doppler run`) \
             and `forward.env` names it (`lf set --global forward.env`)"
        ),
        "{stderr}"
    );
    assert!(!remote.home.path().join("received.json").exists());
    success(&claude("fixture-setup-token"));
    fs::remove_file(remote.home.path().join("received.json")).unwrap();

    // A missing Claude token is Claude's login, so Codex launches still run.
    fs::remove_file(remote.home.path().join("signed-out")).unwrap();
    success(
        &local
            .command()
            .env("CLAUDE_CODE_OAUTH_TOKEN", "")
            .args(["--machine", "fixture", "-b", "-a", "codex", ":", "say done"])
            .output()
            .unwrap(),
    );
    fs::write(&config, registered).unwrap();

    // Here, the same check names this Machine's own sign-in.
    fs::write(local.home.path().join("signed-out"), "").unwrap();
    let output = local
        .command()
        .args(["-b", "-a", "codex", ":", "say done"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("This Machine has no Codex login; run `lf machine login codex`"),
        "{stderr}"
    );
    assert!(!local.home.path().join("received.json").exists());

    // Signed in, the launch goes ahead.
    fs::remove_file(local.home.path().join("signed-out")).unwrap();
    success(
        &local
            .command()
            .args(["-b", "-a", "codex", ":", "say done"])
            .output()
            .unwrap(),
    );
}

#[path = "support/mod.rs"]
mod support;

#[test]
fn task_conversation_preparation_is_passive_and_connect_uses_shared_launch_settings() {
    let fixture = Fixture::new("claude:sonnet");
    let _env = support::EnvGuard::with_lf_home(&[], &fixture.home.path().join("machine"));
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
            "resume",
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
        .args(["session", "resume", id, "--prepare", "--json"])
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
    fixture.repo.create_file(
        ".lf/config.yaml",
        "agents: [claude:sonnet, codex]\nyolo: true\nchrome: true\ndiff: false\ndiff_files: false\npaste: false\n",
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
fn another_agent_resumes_a_failed_codex_bootstrap_on_its_own_input() {
    let fixture = Fixture::new("codex");
    let machine = fixture.home.path().join("machine");
    fixture.create_wave();
    // A chat is a bare Codex terminal; this one exits before starting a thread.
    fs::write(machine.join("fail-terminal"), "").unwrap();
    let failed = fixture
        .command()
        .args([
            "-i",
            "--wave",
            "fixture",
            ":",
            "Preserve this initial request",
        ])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let session = fixture.session();
    fs::remove_file(machine.join("fail-terminal")).unwrap();
    let resumed = fixture.run(&["-a", "claude", "session", "resume", &session]);
    // A fresh Claude input, not the reopened Codex bootstrap.
    assert!(args(&resumed).contains(&"--session-id"), "{resumed}");
    let provider: String = rusqlite::Connection::open(machine.join("loopflow.db"))
        .unwrap()
        .query_row(
            "SELECT provider FROM agent_sessions WHERE id=?1",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(provider, "claude");
    assert_eq!(fixture.session(), session);
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
        let prepared = (provider == "codex").then(|| {
            crate::support::prepare_native_conversation(
                &fixture.home.path().join("machine/loopflow.db"),
                &worktree,
                "codex",
                None,
                "--- literal prompt",
            )
        });
        let launch = match &prepared {
            Some(session) => vec!["-i", "resume", session],
            None => vec!["-i", ":", "--", "--- literal prompt"],
        };
        success(
            &fixture
                .command()
                .current_dir(&worktree)
                .args(&launch)
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
        let resumed = fixture.run(&["session", "resume", &session]);
        for received in [first, resumed] {
            let arguments = args(&received);
            let root = if provider == "codex" {
                received["server"]["sandbox_workspace_write.writable_roots"]
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
fn codex_headless_start_receives_current_policy_on_the_wire() {
    let fixture = Fixture::new("codex:first-model");
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
}

#[test]
fn live_taskless_provider_switch_preserves_identity_and_accepts_terminal_input() {
    let fixture = Fixture::new("claude");
    let script = fixture.home.path().join("bin/claude");
    let source = fs::read_to_string(&script).unwrap().replace(
        "if stream:\n    token =",
        "if not stream:\n    import time\n    while True: time.sleep(0.1)\nif stream:\n    token =",
    );
    fs::write(script, source).unwrap();
    let mut first = fixture
        .command()
        .args(["-i", ":", "original request"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !fixture.home.path().join("received.json").exists() {
        assert!(
            first.try_wait().unwrap().is_none(),
            "first launch ended early"
        );
        if std::time::Instant::now() >= deadline {
            let _ = first.kill();
            first.wait().unwrap();
            panic!("Claude fixture never started");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let received = fixture.received();
    let native = args(&received)[args(&received)
        .iter()
        .position(|arg| *arg == "--session-id")
        .unwrap()
        + 1];
    let session = fixture.session();
    let transcript = fixture
        .home
        .path()
        .join(format!(".claude/projects/fixture/{native}.jsonl"));
    fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    fs::write(
        &transcript,
        serde_json::json!({
            "type": "user", "userType": "external",
            "message": {"content": "Preserve the original design"}
        })
        .to_string(),
    )
    .unwrap();
    let machine = fixture.home.path().join("machine");
    let store = loopflow_execution::store::open_sqlite(&machine.join("loopflow.db")).unwrap();
    let before = store.session(&session).unwrap().unwrap();
    assert!(before.completed_at.is_none());
    assert!(before.task_id.is_none() && before.wave_id.is_none());
    let switched = fixture
        .command()
        .args(["-i", "resume", &session, "-a", "codex"])
        .output()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while first.try_wait().unwrap().is_none() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let _ = first.kill();
    first.wait().unwrap();
    success(&switched);
    // A taskless chat reopens as a chat: bare Codex takes the history as argv.
    let imported = fixture.received();
    assert!(
        args(&imported)
            .iter()
            .any(|arg| arg.contains("Preserve the original design")),
        "{imported}"
    );
    assert_eq!(fixture.session(), session);
    let after = store.session(&session).unwrap().unwrap();
    assert_eq!(after.title, before.title);
    assert_eq!(after.provider.as_deref(), Some("codex"));
    assert_ne!(after.captured, before.captured);
    assert_eq!(
        store
            .session_for_artifact(before.artifact_key.as_deref().unwrap())
            .unwrap()
            .unwrap()
            .id,
        after.id
    );
}

#[test]
fn resume_switches_a_bound_agy_conversation_to_claude_in_its_original_worktree() {
    let fixture = Fixture::new("agy");
    let agy = fixture.home.path().join("bin/agy");
    fs::write(
        &agy,
        format!(
            "{}\nsys.stdin.readline()\n",
            include_str!("../../loopflow-execution/tests/support/agy_cli.py")
        ),
    )
    .unwrap();
    fs::set_permissions(agy, fs::Permissions::from_mode(0o755)).unwrap();
    let origin = fixture
        .repo
        .create_named_worktree("research")
        .canonicalize()
        .unwrap();
    let delivery = fixture.repo.create_named_worktree("delivery");
    let created = fixture
        .command()
        .args([
            "task",
            "create",
            "--title",
            "Delivery",
            "--worktree",
            delivery.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    success(&created);
    let created: Value = serde_json::from_slice(&created.stdout).unwrap();
    let task = created["task"]["id"].as_str().unwrap();
    let mut first = fixture
        .command()
        .current_dir(&origin)
        .args(["-i", "-a", "agy", ":", "Keep the draft here"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !fixture.home.path().join("tui-heard.json").exists() {
        assert!(
            first.try_wait().unwrap().is_none(),
            "agy exited before its first turn"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "agy did not finish its first turn"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let id = fixture.session();
    let bound = fixture
        .command()
        .args(["session", "bind", &id, "--task", task])
        .output()
        .unwrap();
    success(&bound);

    let switched = fixture
        .command()
        .current_dir(&origin)
        .args(["resume", "-a", "claude:fable"])
        .output()
        .unwrap();
    drop(first.stdin.take());
    first.wait().unwrap();
    success(&switched);
    let received = fixture.received();
    assert_eq!(received["cwd"], origin.to_str().unwrap());
    let prompt = received["input"].as_str().unwrap();
    assert!(prompt.contains("Keep the draft here"), "{prompt}");
    assert!(prompt.contains("## assistant\n\nanswer to:"), "{prompt}");
    let card = fixture
        .command()
        .args(["session", "card", &id, "--json"])
        .output()
        .unwrap();
    success(&card);
    let card: Value = serde_json::from_slice(&card.stdout).unwrap();
    assert_eq!(card["session"]["provider"], "claude");
    assert_eq!(card["session"]["task_ids"], serde_json::json!([task]));
}

#[test]
fn provider_switch_imports_session_history_with_current_policy() {
    let fixture = Fixture::new("claude");
    fixture.create_wave();
    let first = fixture.run(&["-i", "--wave", "fixture", ":", "original request"]);
    let native = args(&first)[args(&first)
        .iter()
        .position(|arg| *arg == "--session-id")
        .unwrap()
        + 1]
    .to_owned();
    let session = fixture.session();
    let transcript = fixture
        .home
        .path()
        .join(format!(".claude/projects/fixture/{native}.jsonl"));
    fs::create_dir_all(transcript.parent().unwrap()).unwrap();
    fs::write(
        &transcript,
        serde_json::json!({
            "type": "user", "userType": "external",
            "message": {"content": "Preserve the original design"}
        })
        .to_string(),
    )
    .unwrap();
    let script = support::codex_app_server_script("done", "")
        .replace(
            "read -r thread_start",
            "read -r thread_start\nprintf '%s' \"$thread_start\" > \"$HOME/received.json\"",
        )
        .replace(
            "read -r turn_start",
            "read -r turn_start\nprintf '%s' \"$turn_start\" > \"$HOME/turn.json\"",
        );
    fs::write(fixture.home.path().join("bin/codex"), script).unwrap();
    fixture.settings("claude", true, false);
    let imported = fixture.run(&[
        "-b",
        "-a",
        "codex:new-model",
        "steer",
        &session,
        "Continue the design",
    ]);
    assert_eq!(imported["method"], "thread/start");
    assert!(imported["params"]["threadId"].is_null());
    assert_eq!(imported["params"]["model"], "new-model");
    assert_eq!(imported["params"]["sandbox"], "danger-full-access");
    assert_eq!(imported["params"]["approvalPolicy"], "never");
    let turn = fs::read_to_string(fixture.home.path().join("turn.json")).unwrap();
    assert!(turn.contains("Preserve the original design"), "{turn}");
    assert!(turn.contains("Continue the design"), "{turn}");
    assert_eq!(fixture.session(), session);
}

#[test]
fn opencode_headless_start_selects_the_model_and_permissions() {
    for managed in [false, true] {
        let fixture = Fixture::new("opencode:opencode/first-model");
        if managed {
            fixture.create_wave();
        }
        let script = include_str!("../../loopflow-execution/tests/support/opencode_cli.py")
            .replace("__WAIT__", "False");
        fs::write(fixture.home.path().join("bin/opencode"), script).unwrap();
        fixture.settings("opencode:opencode/first-model", true, false);
        let first = fixture.run(if managed {
            &["-b", "--wave", "fixture", ":", "first turn"]
        } else {
            &["-b", ":", "first turn"]
        });
        assert_eq!(
            serde_json::from_slice::<Value>(
                &fs::read(fixture.home.path().join("native-model.json")).unwrap()
            )
            .unwrap()["id"],
            "first-model"
        );
        assert_eq!(first["server"]["permissions"][0]["effect"], "allow");
        let plugin = first["server"]["plugins"][0]
            .as_str()
            .unwrap()
            .strip_prefix("file://")
            .unwrap();
        let source = fs::read_to_string(Path::new(plugin).join("index.js")).unwrap();
        let mut lines = source.lines();
        let spec: Value = serde_json::from_str(
            lines
                .next()
                .unwrap()
                .strip_prefix("const spec = ")
                .unwrap()
                .trim_end_matches(';'),
        )
        .unwrap();
        let delivery: String = serde_json::from_str(
            lines
                .next()
                .unwrap()
                .strip_prefix("const delivery = ")
                .unwrap()
                .trim_end_matches(';'),
        )
        .unwrap();
        let session = fixture.session();
        assert!(!spec["instructions"].as_str().unwrap().contains(&session));
        for moment in ["start", "compact"] {
            let output = fixture
                .command()
                .args([
                    "__context-block",
                    "--delivery",
                    &delivery,
                    "--moment",
                    moment,
                ])
                .output()
                .unwrap();
            success(&output);
            let context: Value = serde_json::from_slice(&output.stdout).unwrap();
            let text = context["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap();
            assert!(
                text.starts_with(&format!("This Session: {session}\n")),
                "{text}"
            );
            assert!(!text.contains("This Session's Task:"));
        }
    }
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
    let resumed = fixture.run(&["session", "resume", &session]);
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
        loopflow_execution::store::open_sqlite(&fixture.home.path().join("machine/loopflow.db"))
            .unwrap();
    let wave = store.list_waves(None).unwrap().into_iter().next().unwrap();
    let wave_id = wave.id().as_str();
    let alias = fixture
        .command()
        .current_dir(other.path())
        .args(["resume", "--wave", wave_id, "--prepare"])
        .output()
        .unwrap();
    success(&alias);
    let first = fixture
        .command()
        .current_dir(other.path())
        .args(["resume", "--wave", wave_id, "--prepare", "--json"])
        .output()
        .unwrap();
    success(&first);
    let prepared: Value = serde_json::from_slice(&first.stdout).unwrap();
    let id = prepared["id"].as_str().unwrap();
    let saved = store.session(id).unwrap().unwrap();
    fs::create_dir(saved.cwd.join("nested")).unwrap();
    let contextual = fixture
        .command()
        .current_dir(saved.cwd.join("nested"))
        .args(["resume", "--prepare", "--json"])
        .output()
        .unwrap();
    success(&contextual);
    assert_eq!(
        serde_json::from_slice::<Value>(&contextual.stdout).unwrap()["id"],
        id
    );
    let again = fixture
        .command()
        .args(["--wave", wave_id, "session", "resume", "--prepare"])
        .output()
        .unwrap();
    success(&again);
    assert!(String::from_utf8(again.stdout).unwrap().contains(id));
    let explicit = fixture
        .command()
        .args(["session", "resume", id, "--prepare", "--json"])
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
    // Preparation captures no input; the first launch does and later ones reuse it.
    assert!(saved.artifact_key.is_none());
    let mut launched = None;
    for argv in [
        vec!["session", "resume", "--wave", wave_id],
        vec!["resume", "--wave", wave_id, "--json"],
        vec!["--wave", wave_id, "resume"],
        vec!["resume", id],
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
        let key = store.session(id).unwrap().unwrap().artifact_key;
        assert!(key.is_some());
        assert_eq!(&key, launched.get_or_insert_with(|| key.clone()));
    }
    let previous = fixture.received();
    let native = args(&previous)
        .windows(2)
        .find(|pair| pair[0] == "--resume")
        .unwrap()[1]
        .to_owned();
    let continued = fixture.run(&["-b", "steer", id, "next Wave turn"]);
    pair(&continued, "--resume", &native);
    pair(&continued, "--model", "haiku");
    assert!(continued["input"].to_string().contains("next Wave turn"));
    // One prepared shape: the argv itself takes over whatever holds the conversation.
    let prepared = fixture
        .command()
        .args(["resume", id, "--prepare", "--json"])
        .output()
        .unwrap();
    success(&prepared);
    let prepared: Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert_eq!(prepared["id"], id);
    let argv = prepared["open_argv"].as_array().unwrap();
    assert_eq!(argv[argv.len() - 3..], ["session", "resume", id]);
}

#[test]
fn contextual_resume_uses_scope_and_recovers_unassociated_checkouts() {
    let fixture = Fixture::new("claude:sonnet");
    fixture.repo.create_file(
        ".lf/skills/repo/session.md",
        "Repository conversation override\n",
    );
    fixture.repo.stage_all();
    fixture.repo.commit("provider settings");
    let linked = fixture.repo.create_named_worktree("unassociated");
    let prepare = |cwd: &Path, args: &[&str]| -> Value {
        let output = fixture
            .command()
            .current_dir(cwd)
            .args(args)
            .args(["--prepare", "--json"])
            .output()
            .unwrap();
        success(&output);
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let first = prepare(fixture.repo.path(), &["resume"]);
    let store =
        loopflow_execution::store::open_sqlite(&fixture.home.path().join("machine/loopflow.db"))
            .unwrap();
    assert_eq!(
        store
            .session(first["id"].as_str().unwrap())
            .unwrap()
            .unwrap()
            .skill
            .as_deref(),
        Some("repo/session")
    );
    let id = first["id"].as_str().unwrap();
    let saved = Path::new(first["cwd"].as_str().unwrap());
    assert_ne!(saved, fixture.repo.path());
    fs::create_dir(saved.join("nested")).unwrap();
    let alias = fixture.home.path().join("alias");
    std::os::unix::fs::symlink(saved.join("nested"), &alias).unwrap();
    for cwd in [fixture.repo.path(), saved, &saved.join("nested"), &alias] {
        for command in [vec!["resume"], vec!["session", "resume"]] {
            assert_eq!(prepare(cwd, &command)["id"], id);
        }
    }
    let db = rusqlite::Connection::open(fixture.home.path().join("machine/loopflow.db")).unwrap();
    let count = || {
        db.query_row("SELECT count(*) FROM agent_sessions", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
    };
    // A same-name repository under the configured lookup root must not steal
    // the recovery command printed for this unrelated linked checkout.
    let other = TestRepo::new();
    let lookup_root = fixture.home.path().join("src");
    fs::create_dir_all(&lookup_root).unwrap();
    std::os::unix::fs::symlink(
        other.path(),
        lookup_root.join(fixture.repo.path().file_name().unwrap()),
    )
    .unwrap();
    let before = count();
    {
        let verb = "resume";
        let output = fixture
            .command()
            .current_dir(&linked)
            .args([verb, "--prepare"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("This worktree has no associated work conversation"));
        assert_eq!(count(), before);
        let recovery = error
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("lf --repo "))
            .unwrap()
            .split("  ")
            .next()
            .unwrap();
        let argv = shlex::split(recovery).unwrap();
        let args: Vec<&str> = argv.iter().skip(1).map(String::as_str).collect();
        assert_eq!(prepare(&linked, &args)["id"], id);
        assert_eq!(prepare(&linked, &[verb, id])["id"], id);
    }
    // An unassociated checkout continues the conversation last opened there.
    success(
        &fixture
            .command()
            .current_dir(&linked)
            .args(["-i", ":", "local conversation"])
            .output()
            .unwrap(),
    );
    let local = count();
    // A terminal conversation stays unfinished after its client exits; the
    // stub provider completes it.
    let conversation: String = db
        .query_row(
            "UPDATE agent_sessions SET completed_at=NULL WHERE cwd=?1 RETURNING id",
            [linked.canonicalize().unwrap().to_str().unwrap()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(prepare(&linked, &["resume"])["id"], conversation);
    assert_eq!(count(), local);
    let before = local;
    let explanation = fixture
        .command()
        .args(["--explain", "--json", "resume"])
        .output()
        .unwrap();
    success(&explanation);
    let explanation: Value = serde_json::from_slice(&explanation.stdout).unwrap();
    assert_eq!(explanation["resolution"]["session"]["value"], id);
    assert_eq!(count(), before);
    assert_eq!(prepare(&linked, &["resume", id])["id"], id);
    // Beside a Task or Wave flag, the id still selects the Session.
    for argv in [
        vec!["resume", id, "--wave", "fixture"],
        vec!["--task", "missing", "resume", id],
    ] {
        success(&fixture.command().args(argv).output().unwrap());
        assert_eq!(count(), before);
    }
}

#[test]
fn provider_rejected_resume_keeps_the_selected_conversation() {
    let fixture = Fixture::new("claude:sonnet");
    fixture.run(&["-i", ":", "existing history"]);
    let id = fixture.session();
    let store =
        loopflow_execution::store::open_sqlite(&fixture.home.path().join("machine/loopflow.db"))
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
    for argv in [vec!["resume", &id], vec!["session", "resume", &id]] {
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

#[test]
fn headless_step_keeps_prompt_context_in_the_store_without_a_stream_file() {
    let fixture = Fixture::new("claude:sonnet");
    let script = fixture.home.path().join("bin/claude");
    fs::write(
        &script,
        r#"#!/usr/bin/python3 -I
import json, sys
if '--version' in sys.argv:
    print('fixture'); sys.exit(0)
message = json.loads(sys.stdin.readline())
def say(line): print(json.dumps(line), flush=True)
say({'type': 'system', 'subtype': 'init', 'session_id': 'context-fixture'})
say({**message, 'session_id': 'context-fixture'})
for id, fresh in [('m1', 900), ('m2', 4000)]:
    usage = {'input_tokens': fresh, 'cache_read_input_tokens': 100, 'output_tokens': 5}
    say({'type': 'assistant', 'session_id': 'context-fixture', 'message': {'id': id,
        'role': 'assistant', 'content': [{'type': 'text', 'text': 'done'}], 'usage': usage}})
say({'type': 'result', 'session_id': 'context-fixture', 'subtype': 'success', 'result': 'done'})
"#,
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    fixture.repo.create_file(".lf/skills/probe.md", "Probe.");
    fixture.repo.stage_all();
    fixture.repo.commit("Define the probe skill");

    success(
        &fixture
            .command()
            .args(["-b", "probe", "measure"])
            .output()
            .unwrap(),
    );

    let machine = fixture.home.path().join("machine");
    assert_eq!(files_named(&machine.join("runs"), "manifest.json").len(), 1);
    assert!(files_named(&machine.join("runs"), "events.jsonl").is_empty());
    let output = fixture
        .command()
        .args(["history", "usage", "--context", "--json"])
        .output()
        .unwrap();
    success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let step = &report["steps"][0];
    assert_eq!(step["first_request_tokens"], 1000, "{report}");
    assert_eq!(step["peak_request_tokens"], 4100, "{report}");
}

fn pi_fixture() -> Fixture {
    let fixture = Fixture::new("pi:fixture/model");
    let script = fixture.home.path().join("bin/pi");
    fs::write(
        &script,
        format!(
            "#!/usr/bin/python3 -I\n{}",
            include_str!("../../loopflow-execution/src/harness/testdata/pi_rpc.py")
        ),
    )
    .unwrap();
    fs::set_permissions(script, fs::Permissions::from_mode(0o755)).unwrap();
    fixture
}

#[test]
fn pi_public_headless_skill_delivers_context() {
    for managed in [false, true] {
        let fixture = pi_fixture();
        if managed {
            fixture.create_wave();
        }
        fixture.repo.create_file(
            ".lf/skills/pi-proof.md",
            &format!(
                "Preserve this Pi skill and its exact arguments. LARGE_START{}LARGE_END",
                "雪<&>".repeat(20_000)
            ),
        );
        fixture
            .repo
            .create_file("scratch/pi-context.md", "Pi context sentinel.");
        // Pi keeps its conversation under its own home; the stand-in writes
        // it in the checkout, which is no edit of the skill's.
        fixture
            .repo
            .create_file(".gitignore", "pi-conversation.jsonl\n");
        fixture.repo.stage_all();
        fixture.repo.commit("Define the Pi skill and its context");
        let output = fixture
            .command()
            .env(
                "PI_FIXTURE_RECORD",
                fixture.home.path().join("received.json"),
            )
            .args(if managed {
                vec![
                    "-b",
                    "--wave",
                    "fixture",
                    "skill",
                    "pi-proof",
                    "Unicode 雪 stays intact",
                ]
            } else {
                vec!["-b", "skill", "pi-proof", "Unicode 雪 stays intact"]
            })
            .output()
            .unwrap();
        success(&output);
        let received = fixture.received();
        assert!(received["input"]
            .as_str()
            .unwrap()
            .contains("Preserve this Pi skill"));
        assert!(received["input"]
            .as_str()
            .unwrap()
            .contains("Unicode 雪 stays intact"));
        assert!(received["input"]
            .as_str()
            .unwrap()
            .contains(&format!("LARGE_START{}LARGE_END", "雪<&>".repeat(20_000))));
        assert!(received["context"]
            .as_str()
            .unwrap()
            .contains("Pi context sentinel"));
        assert!(received["context"]
            .as_str()
            .unwrap()
            .starts_with(&format!("This Session: {}\n", fixture.session())));
        assert!(!received["system"]
            .as_str()
            .unwrap()
            .contains("This Session:"));
        assert_eq!(received["caller"]["session_id"], fixture.session());
        assert_eq!(received["lf_bin"], env!("CARGO_BIN_EXE_lf"));
    }
}

#[test]
fn pi_native_callbacks_retain_identity_final_output_and_usage() {
    let fixture = pi_fixture();
    // Pi's callbacks carry provider context only for a managed launch.
    fixture.create_wave();
    let prepared = crate::support::prepare_native_conversation(
        &fixture.home.path().join("machine/loopflow.db"),
        fixture.repo.path(),
        "pi",
        None,
        "Native Pi input",
    );
    let output = fixture
        .command()
        .env(
            "PI_FIXTURE_RECORD",
            fixture.home.path().join("received.json"),
        )
        .args(["-i", "resume", &prepared])
        .output()
        .unwrap();
    success(&output);
    let session = fixture.session();
    let db = machine_db(fixture.home.path());
    let (provider, thread): (String, String) = db
        .query_row(
            "SELECT provider,provider_thread FROM agent_sessions WHERE id=?1",
            [&session],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(provider, "pi");
    assert_eq!(
        thread,
        fixture
            .repo
            .path()
            .canonicalize()
            .unwrap()
            .join("pi-conversation.jsonl")
            .display()
            .to_string()
    );
    assert_eq!(pi_final_answer(&fixture, &session), "native answer");
    let tokens: i64 = db.query_row("SELECT total_input_tokens FROM history.turns WHERE session_id=?1 AND usage_at IS NOT NULL ORDER BY id DESC LIMIT 1", [&session], |row| row.get(0)).unwrap();
    assert_eq!(tokens, 26);
    let receipts: i64 = db
        .query_row(
            "SELECT count(*) FROM history.turns WHERE session_id=?1 AND usage_at IS NOT NULL",
            [&session],
            |row| row.get(0),
        )
        .unwrap();
    // The turn's three native receipts merge into one turn's usage.
    assert_eq!(receipts, 1);
    let received = fixture.received();
    assert!(args(&received).contains(&"--extension"));
    assert!(received["input"]
        .as_str()
        .unwrap()
        .contains("Native Pi input"));
}

#[test]
fn pi_native_crash_keeps_the_paid_step_and_interrupts_the_turn() {
    let fixture = pi_fixture();
    fixture.create_wave();
    let prepared = crate::support::prepare_native_conversation(
        &fixture.home.path().join("machine/loopflow.db"),
        fixture.repo.path(),
        "pi",
        None,
        "Crash after a paid model step",
    );
    let output = fixture
        .command()
        .env("PI_FIXTURE_CRASH", "1")
        .args(["-i", "resume", &prepared])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let session = fixture.session();
    let db = machine_db(fixture.home.path());
    let (count, tokens): (i64, i64) = db.query_row(
        "SELECT count(*), total_input_tokens FROM history.turns WHERE session_id=?1 AND usage_at IS NOT NULL",
        [&session], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    assert_eq!((count, tokens), (1, 13));
    // The exit read closes the turn the transcript left open.
    let statuses: Vec<String> = db
        .prepare("SELECT status FROM history.turns WHERE session_id=?1 AND ended_at IS NOT NULL")
        .unwrap()
        .query_map([&session], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(statuses, ["interrupted"]);
}

#[test]
fn pi_native_failed_start_reopens_without_replacing_the_loopflow_session() {
    let fixture = pi_fixture();
    fixture.create_wave();
    let prepared = crate::support::prepare_native_conversation(
        &fixture.home.path().join("machine/loopflow.db"),
        fixture.repo.path(),
        "pi",
        None,
        "Initial input",
    );
    let provider = fixture.home.path().join("bin/pi");
    let original = fs::read(&provider).unwrap();
    fs::write(&provider, "#!/bin/sh\nexit 9\n").unwrap();
    let failed = fixture
        .command()
        .args(["-i", "resume", &prepared])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let id = fixture.session();
    fs::write(&provider, original).unwrap();
    let resumed = fixture
        .command()
        .args(["-i", "resume", &id])
        .output()
        .unwrap();
    success(&resumed);
    assert_eq!(fixture.session(), id);
    let db = machine_db(fixture.home.path());
    let sessions: i64 = db
        .query_row(
            "SELECT count(*) FROM agent_sessions WHERE provider='pi'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(sessions, 1);
    assert_eq!(pi_final_answer(&fixture, &id), "native answer");
}

/// What `lf history show --final` prints for a Session, read from Pi's file.
fn pi_final_answer(fixture: &Fixture, session: &str) -> String {
    let output = fixture
        .command()
        .args(["history", "show", session, "--final"])
        .output()
        .unwrap();
    success(&output);
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn native_auth_owns_launches_with_a_populated_catalog() {
    for provider in ["claude", "codex"] {
        for task_owned in [false, true] {
            let fixture = Fixture::new(provider);
            let machine = fixture.home.path().join("machine");
            let _env = support::EnvGuard::with_lf_home(&[], &machine);
            let native = fixture.home.path().join(format!(".{provider}"));
            fs::create_dir_all(&native).unwrap();
            let credential = if provider == "claude" {
                ".credentials.json"
            } else {
                "auth.json"
            };
            let login = native.join(credential);
            let bytes = br#"{"fixture_login":"A"}"#;
            fs::write(&login, bytes).unwrap();
            let saved = machine.join(format!("accounts/{provider}/saved"));
            fs::create_dir_all(&saved).unwrap();
            fs::write(saved.join(credential), br#"{"fixture_login":"B"}"#).unwrap();
            let db = rusqlite::Connection::open(machine.join("loopflow.db")).unwrap();
            db.execute("INSERT INTO provider_accounts(provider,account_id,home,credential_state,utilization_percent,cooldown_until,cooldown_reason,created_at,updated_at) VALUES(?1,'saved',?2,'missing',100,4000000000,'fixture',1,1)",
                rusqlite::params![provider, saved.to_str().unwrap()]).unwrap();
            // Released preferences stay readable but have no launch authority.
            fs::write(machine.join("config.yaml"), format!("account: {{{provider}: [saved]}}\naccounts: {{{provider}: {{saved: {{login_email: saved@example.com}}}}}}\n")).unwrap();
            let executable = fixture.home.path().join("bin").join(provider);
            let source = if provider == "claude" {
                let original = fs::read_to_string(&executable).unwrap();
                original.replace(
                    "args = sys.argv[1:]",
                    r#"args = sys.argv[1:]
if '--version' not in args:
    assert os.environ['CLAUDE_CONFIG_DIR'] == os.environ['HOME']+'/.claude'
    assert 'LF_PROVIDER_ACCOUNT_ID' not in os.environ
    try:
        auth = json.load(open(os.environ['CLAUDE_CONFIG_DIR']+'/.credentials.json'))
    except FileNotFoundError:
        sys.stdin.readline()
        print('native login required', file=sys.stderr); sys.exit(34)
    # An exported API key never reaches Claude: it bills the native login.
    assert 'ANTHROPIC_API_KEY' not in os.environ
    with open(os.environ['HOME']+'/auth-seen.json','w') as out: json.dump(auth,out)
"#,
                )
            } else {
                support::codex_app_server_script(
                    "done",
                    r#"
[ "$CODEX_HOME" = "$HOME/.codex" ] || exit 35
[ -z "$LF_PROVIDER_ACCOUNT_ID" ] || exit 36
[ -f "$CODEX_HOME/auth.json" ] || { echo 'native login required' >&2; exit 34; }
[ -z "${OPENAI_API_KEY+set}" ] || exit 37
cp "$CODEX_HOME/auth.json" "$HOME/auth-seen.json"
"#,
                )
            };
            let source = if provider == "claude" {
                source.replace("    message['session_id'] = token", "    print(json.dumps({'type':'rate_limit_event','rate_limit_info':{'status':'allowed_warning','utilization':1.0}}), flush=True)\n    message['session_id'] = token")
            } else {
                source.replace("read -r turn_start", "read -r turn_start\necho '{\"method\":\"account/rateLimits/updated\",\"params\":{\"rateLimits\":{\"primary\":{\"usedPercent\":100,\"windowDurationMins\":300}}}}'")
            };
            fs::write(&executable, source).unwrap();
            let task = task_owned.then(|| {
                let base = Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .current_dir(fixture.repo.path())
                    .output()
                    .unwrap();
                support::register_task_without_pr(
                    &machine,
                    fixture.repo.path(),
                    "main",
                    String::from_utf8(base.stdout).unwrap().trim(),
                )
                .task
            });
            let mut command = fixture.command();
            command.arg("-b").env(
                if provider == "claude" {
                    "ANTHROPIC_API_KEY"
                } else {
                    "OPENAI_API_KEY"
                },
                "fixture-native-api-key",
            );
            if let Some(task) = &task {
                command.args(["--task", task.id.as_str()]);
            }
            success(&command.args([":", "finish the fixture"]).output().unwrap());
            let session = fixture.session();
            let thread = || {
                db.query_row(
                    "SELECT provider_thread FROM agent_sessions WHERE id=?1",
                    [&session],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
            };
            assert!(!thread().is_empty());
            let seen: Value = serde_json::from_slice(
                &fs::read(fixture.home.path().join("auth-seen.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(seen["fixture_login"], "A");
            assert_eq!(fs::read(&login).unwrap(), bytes);
            assert_eq!(
                fs::read(saved.join(credential)).unwrap(),
                br#"{"fixture_login":"B"}"#
            );
            let state: (String, i64, Option<i64>) = db.query_row("SELECT credential_state,cooldown_until,last_selected_at FROM provider_accounts WHERE provider=?1 AND account_id='saved'", [provider], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
            assert_eq!(state, ("missing".into(), 4000000000, None));
            let claims: i64 = db
                .query_row(
                    "SELECT COUNT(*) FROM provider_session_accounts",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(claims, 0);
            // Removing the native login reaches the provider's own error, despite B.
            fs::remove_file(&login).unwrap();
            let failed = fixture
                .command()
                .args(["-b", ":", "try without login"])
                .output()
                .unwrap();
            assert!(!failed.status.success());
            assert!(
                String::from_utf8_lossy(&failed.stderr).contains("native login required"),
                "{provider}, Task={task_owned}: {}",
                String::from_utf8_lossy(&failed.stderr)
            );
        }
    }
}
