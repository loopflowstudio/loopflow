use crate::support;

use std::process::{Command, Output};

fn command(home: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        let key = name.to_string_lossy();
        if key.starts_with("LF_") || key.starts_with("LOOPFLOW_") {
            command.env_remove(name);
        }
    }
    command
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env_remove("LF_CAPTURE_KEY")
        .env_remove("LF_RUN_DIR")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_GRAPH_ID")
        .env_remove("LF_HUMAN_SESSION")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_TERMINAL_ID")
        .env_remove("LF_TERMINAL_TTY")
        .env("RUST_LOG", "off");
    command
}

#[cfg(unix)]
fn install_codex_fixture(home: &std::path::Path, thread: Option<&str>) {
    crate::support::install_native_codex(&home.join("bin"));
    if let Some(thread) = thread {
        std::fs::write(home.join("native-thread"), thread).unwrap();
        std::fs::write(
            home.join("native-turns.json"),
            r#"[{"id":"previous","status":"completed","items":[]}]"#,
        )
        .unwrap();
    }
}

/// Every Session row, whatever its mode or state.
const EVERY_SESSION: &[&str] = &[
    "session",
    "list",
    "--all",
    "--interactive",
    "all",
    "--history",
    "--json",
];

fn run(home: &std::path::Path, args: &[&str]) -> Output {
    command(home, args).output().unwrap()
}

#[test]
fn session_cli_uses_one_truthful_resolution_contract() {
    let home = tempfile::tempdir().unwrap();
    let help = run(home.path(), &["session", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(!help
        .lines()
        .any(|line| line.trim_start().starts_with("ask ")));
    for args in [&["ask", "Help"][..], &["session", "ask", "Help"]] {
        assert!(!run(home.path(), args).status.success());
    }
    assert!(!help.contains("advance"));
    assert!(!help.contains("iterate"));
    assert!(!help.contains("complete"));
    assert!(!help.contains("ready"));
    assert!(help.contains("resume"));
    assert!(help.contains("new"));
    assert!(!help.contains("connect"));
    assert!(!help.contains("attach"));
    assert!(!help.contains("accept"));
    assert!(!help.contains("decline"));
    assert!(!help.contains("send-back"));

    for flag in ["--resume", "--events"] {
        let removed = run(home.path(), &["history", "show", "historical-input", flag]);
        assert_eq!(removed.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&removed.stderr)
            .contains(&format!("unexpected argument '{flag}'")));
    }

    for args in [
        &["session", "ready", "Feedback"][..],
        &["session", "complete", "missing-session"],
        &["session", "stop-client", "missing-input"],
        &["session", "serve-graph"],
        &["session", "ensure"],
        &["session", "connect", "missing-session"],
        &["session", "attach", "missing-session"],
    ] {
        let output = run(home.path(), args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
    }
    let output = run(
        home.path(),
        &[
            "session",
            "resume",
            "missing-session",
            "--prepare",
            "--json",
        ],
    );
    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "Error: Session missing-session was not found"
    );
}

#[cfg(unix)]
#[test]
fn development_session_handoff_keeps_its_binary_and_home() {
    use std::os::unix::fs::PermissionsExt;

    if loopflow::build_info::provenance().is_release() {
        return; // This fixture exercises an uninstalled development binary.
    }
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let other = bin.join("lf");
    std::fs::write(&other, "#!/bin/sh\necho wrong-Machine >&2\nexit 91\n").unwrap();
    std::fs::set_permissions(&other, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths([bin, "/usr/bin".into(), "/bin".into()]).unwrap();
    let (id, ..) = prepare_conversation(
        home.path(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "codex",
        "Keep this development review",
    );
    let command = |binary: &str| {
        let mut command = Command::new(binary);
        command
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("PATH", &path)
            .env("LF_HOME", home.path())
            .env("RUST_LOG", "off");
        for name in [
            "LF_BIN",
            "CARGO_BIN_EXE_lf",
            "LF_CAPTURE_KEY",
            "LF_RUN_DIR",
            "LF_TRACE_ID",
            "LF_PROCESS_ID",
            "LF_HUMAN_SESSION",
            "LF_GRAPH_ID",
        ] {
            command.env_remove(name);
        }
        command
    };
    let prepared = command(env!("CARGO_BIN_EXE_lf"))
        .args(["session", "resume", &id, "--prepare", "--json"])
        .output()
        .unwrap();
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let output = command(env!("CARGO_BIN_EXE_lf"))
        .args(["desktop", "sessions", "--json", "--all"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let listed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let argv = listed[0]["open_argv"].as_array().unwrap();
    let other_home = tempfile::tempdir().unwrap();
    let reopened = command(argv[0].as_str().unwrap())
        .args(argv[1..].iter().map(|arg| arg.as_str().unwrap()))
        .env("LF_HOME", other_home.path())
        .arg("--prepare")
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        reopened.status.success(),
        "{}",
        String::from_utf8_lossy(&reopened.stderr)
    );
    let reopened: serde_json::Value = serde_json::from_slice(&reopened.stdout).unwrap();
    assert_eq!(reopened["id"], id);
    assert!(!other_home.path().join("loopflow.db").exists());
}

#[test]
fn prepared_conversation_keeps_captured_input_without_a_run_before_provider_start() {
    let home = tempfile::tempdir().unwrap();
    let (id, run_id, dir) = prepare_conversation(
        home.path(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "codex",
        "Do not launch",
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["artifact_key"], run_id.as_str());
    assert_eq!(manifest["caller_artifact_key"], CALLER);
    let opened = run(
        home.path(),
        &["session", "resume", &id, "--prepare", "--json"],
    );
    assert!(
        opened.status.success(),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let opened: serde_json::Value = serde_json::from_slice(&opened.stdout).unwrap();
    assert!(opened.get("run_id").is_none());
    assert_eq!(opened["state"], "unknown");
    assert!(dir.join("prepared").exists());
    assert!(!dir.join("provider-clients").exists());
    assert!(!dir.join("terminal.json").exists());
    let inspected = run(home.path(), &["history", "show", &id, "--json"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspected: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(inspected["artifact_key"], run_id.as_str());
    assert_eq!(inspected["caller_artifact_key"], CALLER);
    assert!(inspected["recorded_outcome"].is_null());
    let listed = run(home.path(), EVERY_SESSION);
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert!(listed[0].get("run_id").is_none());
    assert!(!dir.join("events.jsonl").exists());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM session_events WHERE kind='captured' AND session_id=?1",
            [&id],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[cfg(unix)]
#[test]
fn boundary_launch_and_resume_remain_openable_while_provider_waits() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    for resume in [true, false] {
        let home = tempfile::tempdir().unwrap();
        let (id, run_id, dir) =
            prepare_conversation(home.path(), home.path(), "opencode", "Local proof");
        let (id, run_id) = (id.as_str(), run_id.as_str());
        // Old live Runs must survive the history reader's seven-day window.
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
        manifest["created_at"] = serde_json::json!("2020-01-01T00:00:00Z");
        std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();

        // The native history belongs to this isolated fixture. The executable below
        // stands in for the provider; both opens exercise the real CLI and locks.
        if resume {
            let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
            db.execute(
                "UPDATE agent_sessions SET provider_thread='ses_resume-proof' WHERE id=?1",
                [id],
            )
            .unwrap();
            db.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
                SELECT id,'observed',?2||':fixture-native',created_at,json_object('input_id',?2,'source','provider-session:fixture','evidence',json_object('schema_version',1,'provider_session_id','ses_resume-proof','account_id',NULL)),current_capture
                FROM agent_sessions WHERE id=?1", [id,run_id]).unwrap();
            std::fs::remove_file(dir.join("manifest.json")).unwrap();
        }
        std::fs::write(
            home.path().join("native-sessions.json"),
            r#"{"ses_resume-proof":[]}"#,
        )
        .unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let provider = bin.join("opencode");
        std::fs::write(
            &provider,
            include_str!("support/opencode_server.py").replace("__WAIT__", "True"),
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !resume {
            let rejected = bin.join("rejecting-lf");
            std::fs::write(
                &rejected,
                "#!/bin/sh\necho 'fixture launcher rejected invocation' >&2\nexit 2\n",
            )
            .unwrap();
            std::fs::set_permissions(&rejected, std::fs::Permissions::from_mode(0o755)).unwrap();
            let failed = command(home.path(), &["session", "resume", id])
                .env("LF_BIN", &rejected)
                .output()
                .unwrap();
            assert!(!failed.status.success());
            let error = String::from_utf8_lossy(&failed.stderr);
            assert!(error.contains(rejected.to_str().unwrap()), "{error}");
            assert!(error.contains("sha256"), "{error}");
            assert!(
                error.contains(home.path().join("loopflow.db").to_str().unwrap()),
                "{error}"
            );
            assert!(error.contains("before becoming resumable"), "{error}");
            assert!(error.contains(run_id), "{error}");
            assert!(!error.contains("Local proof"), "prompt leaked: {error}");
        }
        let evidence = home.path().join("resumed");
        let mut first = command(home.path(), &["session", "resume", id])
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("LF_RESUME_PROOF", &evidence)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // An upper bound: a debug `lf` on a busy machine launches slowly.
        let deadline = Instant::now() + Duration::from_secs(180);
        while !evidence.exists() && Instant::now() < deadline && first.try_wait().unwrap().is_none()
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        if !evidence.exists() {
            let _ = first.kill();
            let output = first.wait_with_output().unwrap();
            panic!(
                "fixture provider did not start: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let selector = id;
        // Drain metadata independently of process exit: a captured prompt can
        // exceed the OS pipe buffer while the provider is deliberately waiting.
        let metadata_stdout = home.path().join("metadata-stdout");
        let metadata_stderr = home.path().join("metadata-stderr");
        let mut second = command(
            home.path(),
            &["session", "resume", selector, "--prepare", "--json"],
        )
        .stdout(Stdio::from(
            std::fs::File::create(&metadata_stdout).unwrap(),
        ))
        .stderr(Stdio::from(
            std::fs::File::create(&metadata_stderr).unwrap(),
        ))
        .spawn()
        .unwrap();
        // The provider waits on stdin indefinitely, so a slow machine cannot pass by accident.
        let deadline = Instant::now() + Duration::from_secs(60);
        while second.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let blocked = second.try_wait().unwrap().is_none();
        if blocked {
            second.kill().unwrap();
        }
        let mut reopened = second.wait_with_output().unwrap();
        reopened.stdout = std::fs::read(metadata_stdout).unwrap();
        reopened.stderr = std::fs::read(metadata_stderr).unwrap();
        let active = run(home.path(), EVERY_SESSION);
        // Release the owned provider before asserting, including on the failure path.
        first.stdin.take().unwrap().write_all(b"done\n").unwrap();
        let first = first.wait_with_output().unwrap();
        assert!(first.status.success(), "{:?}", first);
        assert!(
            !blocked,
            "Session metadata open waited for the provider (resume={resume}) to exit: {reopened:?}"
        );
        assert!(reopened.status.success(), "{:?}", reopened);
        let reopened: serde_json::Value = serde_json::from_slice(&reopened.stdout).unwrap();
        assert!(reopened.get("run_id").is_none());
        let selected: String = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap()
            .query_row("SELECT receipt_key FROM session_events WHERE seq=(SELECT current_capture FROM agent_sessions WHERE id=?1)", [id], |row| row.get(0)).unwrap();
        assert_eq!(std::fs::read_to_string(evidence).unwrap(), selected);
        let caller: loopflow::process::AgentCaller =
            serde_json::from_slice(&std::fs::read(home.path().join("resumed.caller")).unwrap())
                .unwrap();
        assert_eq!(caller.session_id, id);
        let recorded: (Option<loopflow::id::LfProcessId>, String) = rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT p.id,p.parent_lf_process_id FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (
                caller.agent_process_id,
                caller.origin_lf_process_id.to_string()
            ),
            recorded
        );

        assert_eq!(reopened["state"], "active");
        let state = |output: std::process::Output| {
            assert!(output.status.success(), "{output:?}");
            let rows: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            rows.as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == id)
                .unwrap_or_else(|| panic!("{rows}"))["state"]
                .clone()
        };
        assert_eq!(state(active), "active");
        let ended = run(home.path(), EVERY_SESSION);
        assert_ne!(state(ended), "active");
        if !resume {
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
            assert_eq!(manifest["artifact_key"], run_id);
            assert!(manifest["context"].is_object());
            assert!(!dir.join("prepared").exists());
            let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
            let completed_at: Option<i64> = database
                .query_row(
                    "SELECT completed_at FROM agent_sessions WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(completed_at.is_some(), "the exited orphan is retired");
        }
    }
}

const CALLER: &str = "run_00000000000000000000000000000002";

/// A prepared conversation, before a provider has started.
fn prepare_conversation(
    home: &std::path::Path,
    cwd: &std::path::Path,
    harness: &str,
    title: &str,
) -> (String, String, std::path::PathBuf) {
    let store = loopflow::store::sqlite::SqliteStore::new(&home.join("loopflow.db")).unwrap();
    let input = format!("run_{}", uuid::Uuid::new_v4().simple());
    let session = store
        .create_session(
            loopflow::session::LfSession {
                captured: None,
                id: format!("session_{input}").into(),
                artifact_key: Some(input.clone()),
                caller_artifact_key: Some(CALLER.into()),
                input_published: false,
                cwd: cwd.into(),
                skill: None,
                provider: Some(harness.into()),
                model: None,
                node: None,
                iterations: None,
                task_id: None,
                wave_id: None,
                graph_lf_process_id: None,
                work_source: None,
                bound_at: None,
                interactive: true,
                repo: None,
                title: title.into(),
                title_source: loopflow::session::TitleSource::Generated,
                request: Some(title.into()),
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            },
            None,
        )
        .unwrap();
    let dir = home.join("runs").join(&input[4..6]).join(&input);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1, "artifact_key": input, "caller_artifact_key": CALLER,
        "created_at": "2026-01-01T00:00:00Z", "harness": harness, "model": null,
        "surface": "tui", "cwd": cwd, "repo": null, "worktree": null,
        "skill": null, "subjects": [], "flow": {"kind":"independent"}, "exec": null, "context": null,
        "runtime_path": null, "runtime_digest": null, "host": "test", "boot_id": null
    });
    std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(dir.join("prepared"), "").unwrap();
    rusqlite::Connection::open(home.join("loopflow.db"))
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET input_published=1 WHERE id=?1",
            [&session.id],
        )
        .unwrap();
    (session.id.to_string(), input, dir)
}

#[cfg(unix)]
#[test]
fn headless_resume_preserves_a_held_owners_capture_on_both_harnesses() {
    use std::os::unix::fs::PermissionsExt;

    for harness in ["claude", "codex"] {
        let home = tempfile::tempdir().unwrap();
        let cwd = home.path().join("work");
        let bin = home.path().join("bin");
        std::fs::create_dir(&cwd).unwrap();
        std::fs::create_dir(&bin).unwrap();
        let provider = bin.join(harness);
        std::fs::write(
            &provider,
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo fixture; exit 0; fi\necho unexpected-provider-launch >&2\nexit 97\n",
        ).unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (id, input, dir) =
            prepare_conversation(home.path(), &cwd, harness, "Preserve this draft");
        let manifest = std::fs::read(dir.join("manifest.json")).unwrap();
        let store =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        let process = loopflow::id::LfProcessId::new();
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment(&id, None, &process, true)
            .unwrap();
        let before = store.session(&id).unwrap().unwrap();
        let history = store.session_history(&id, 0, 0).unwrap();
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let output = command(
            home.path(),
            &[
                "--batch",
                "session",
                "resume",
                &id,
                "--message",
                "another instruction",
            ],
        )
        .current_dir(&cwd)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("HOME", home.path())
        .output()
        .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("already has an attached LfProcess"),
            "{harness}: {error}"
        );
        assert!(!error.contains("unexpected-provider-launch"), "{error}");
        assert_eq!(store.session(&id).unwrap().unwrap(), before);
        assert_eq!(store.session_attachment(&id).unwrap(), Some(attachment));
        assert_eq!(store.session_history(&id, 0, 0).unwrap(), history);
        assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), manifest);
        assert!(dir.join("prepared").exists());
        assert_eq!(
            store.session(&id).unwrap().unwrap().artifact_key.as_ref(),
            Some(&input)
        );
    }
}

#[cfg(unix)]
#[test]
fn headless_resume_reads_the_saved_workspace_and_retains_process_provenance() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().unwrap();
    let saved = home.path().join("saved");
    let caller = home.path().join("caller");
    let bin = home.path().join("bin");
    for (path, marker) in [(&saved, "SAVED_CONTEXT"), (&caller, "CALLER_CONTEXT")] {
        std::fs::create_dir_all(path.join("scratch")).unwrap();
        std::fs::write(path.join("scratch/context.md"), marker).unwrap();
    }
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    std::fs::write(
        &provider,
        r#"#!/bin/sh
set -eu
if [ "${1:-}" = --version ]; then exit 0; fi
pwd -P > "$LF_TEST_RESUME_PROOF.cwd"
printf '%s\n' "$@" > "$LF_TEST_RESUME_PROOF.args"
printf '%s\n' "$LF_AGENT_CALLER" > "$LF_TEST_RESUME_PROOF.caller"
python3 - "$@" <<'PYTHON'
import json, os, pathlib, subprocess, sys
args = sys.argv[1:]
settings = json.loads(pathlib.Path(args[args.index("--settings") + 1]).read_text())
hook = settings["hooks"]["SessionStart"][0]["hooks"][0]["command"]
event = {"session_id": args[args.index("--resume") + 1], "hook_event_name": "SessionStart", "source": "resume"}
block = json.loads(subprocess.check_output(hook, shell=True, input=json.dumps(event).encode()))["hookSpecificOutput"]["additionalContext"]
pathlib.Path(os.environ["LF_TEST_RESUME_PROOF"] + ".context").write_text(block)
PYTHON
python3 -c '
import json, os, pathlib, sys
message = json.loads(sys.stdin.readline())
pathlib.Path(os.environ["LF_TEST_RESUME_PROOF"] + ".input").write_text(message["message"]["content"][0]["text"])
message["session_id"] = "fixture-native"
print(json.dumps(message), flush=True)
print(json.dumps({"type":"system","subtype":"init","session_id":"fixture-native"}), flush=True)
print(json.dumps({"type":"result","uuid":"continued","session_id":"fixture-native","subtype":"success","result":"continued"}), flush=True)
for line in sys.stdin:
    pass
'
"#,
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (id, input, dir) = prepare_conversation(home.path(), &saved, "claude", "Original input");
    let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    database
        .execute(
            "UPDATE agent_sessions SET provider_thread='fixture-native',skill='missing-original-skill' WHERE id=?1",
            [&id],
        )
        .unwrap();
    let original = std::fs::read(dir.join("manifest.json")).unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let wave = loopflow::work::wave::Wave::new(
        loopflow::id::WaveId::new(),
        "caller-wave".into(),
        caller.to_str().unwrap().into(),
    );
    store.create_wave(&wave).unwrap();
    std::fs::create_dir_all(saved.join(".lf/wave/caller-wave")).unwrap();
    std::fs::write(
        saved.join(".lf/wave/caller-wave/GOAL.md"),
        "CALLER_WAVE_CONTEXT",
    )
    .unwrap();
    let before = store.session(&id).unwrap().unwrap();
    let history = store.session_history(&id, 0, 0).unwrap();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let output = command(
        home.path(),
        &["-b", "session", "resume", &id, "--message", "Continue here"],
    )
    .current_dir(&caller)
    .env("PATH", path)
    .env("HOME", home.path())
    .env("LF_TEST_RESUME_PROOF", home.path().join("proof"))
    // A calling agent's Wave must not supply a different conversation's context.
    .env("LF_WAVE_ID", wave.id().as_str())
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sent = std::fs::read_to_string(home.path().join("proof.context")).unwrap();
    assert!(sent.contains("SAVED_CONTEXT"), "{sent}");
    let sent_input = std::fs::read_to_string(home.path().join("proof.input")).unwrap();
    assert!(sent_input.contains("Continue here"), "{sent_input}");
    assert!(!sent.contains("CALLER_CONTEXT"), "{sent}");
    assert!(!sent.contains("CALLER_WAVE_CONTEXT"), "{sent}");
    assert!(!sent.contains("missing-original-skill"), "{sent}");
    assert_eq!(
        std::fs::read_to_string(home.path().join("proof.cwd"))
            .unwrap()
            .trim(),
        saved.canonicalize().unwrap().to_str().unwrap()
    );
    let args = std::fs::read_to_string(home.path().join("proof.args")).unwrap();
    assert!(args.contains("--resume\nfixture-native\n"), "{args}");
    let after = store.session(&id).unwrap().unwrap();
    assert_eq!(after.cwd, before.cwd);
    assert_eq!(after.skill, before.skill);
    assert_eq!(after.task_id, before.task_id);
    assert_eq!(after.wave_id, before.wave_id);
    assert_ne!(after.artifact_key.as_ref(), Some(&input));
    assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
    assert!(store
        .session_history(&id, 0, 0)
        .unwrap()
        .starts_with(&history));
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            home.path()
                .join("runs")
                .join(&after.artifact_key.as_deref().unwrap()[..2])
                .join(after.artifact_key.as_ref().unwrap())
                .join("manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["cwd"], saved.to_str().unwrap());
    let agent_caller: loopflow::process::AgentCaller =
        serde_json::from_slice(&std::fs::read(home.path().join("proof.caller")).unwrap()).unwrap();
    assert_eq!(agent_caller.session_id, id);
    let process_cwd = store
        .process(&agent_caller.origin_lf_process_id)
        .unwrap()
        .unwrap()
        .cwd
        .unwrap();
    assert_eq!(
        std::path::Path::new(&process_cwd).canonicalize().unwrap(),
        caller.canonicalize().unwrap()
    );
}

#[test]
fn available_lists_only_conversations_ready_for_input() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let provider = Child(Command::new("/bin/sleep").arg("60").spawn().unwrap());
    let started = time::OffsetDateTime::now_utc().unix_timestamp();
    let home = tempfile::tempdir().unwrap();
    let cwd = home.path().join("work");
    std::fs::create_dir(&cwd).unwrap();
    let (working, _, _) = prepare_conversation(home.path(), &cwd, "codex", "Working");
    let (asked, _, _) = prepare_conversation(home.path(), &cwd, "codex", "Asked");
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let parent = loopflow::id::LfProcessId::new();
    db.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
        [&parent],
    )
    .unwrap();
    for (id, pending) in [(&working, 0), (&asked, 1)] {
        let attachment = store
            .claim_session_attachment(id, None, &parent, true)
            .unwrap();
        db.execute(
            "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
            rusqlite::params![attachment.agent_process_id, provider.0.id(), started],
        )
        .unwrap();
        db.execute(
            "INSERT INTO session_activity(session_id,attachment_token,agent_process_id,observed_at,open_tools,pending_input,yielded)
             SELECT s.id,p.attachment_token,p.id,unixepoch(),1,?2,0 FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
            rusqlite::params![id, pending],
        )
        .unwrap();
    }
    let list = |flag: &str| run(home.path(), &["session", "list", "--all", "--json", flag]);
    let available = list("--available");
    assert!(available.status.success(), "{available:?}");
    let available: Vec<serde_json::Value> = serde_json::from_slice(&available.stdout).unwrap();
    assert_eq!(available.len(), 1);
    assert_eq!(available[0]["id"], asked.as_str());
    assert_eq!(available[0]["available"], true);
    assert!(listed(home.path(), &working)["availability"].is_null());
    assert!(!list("--needs-me").status.success());
}

fn listed(home: &std::path::Path, id: &str) -> serde_json::Value {
    let output = run(
        home,
        &["desktop", "sessions", "--all", "--history", "--json"],
    );
    assert!(output.status.success(), "{output:?}");
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    sessions
        .into_iter()
        .find(|session| session["id"] == id)
        .unwrap_or_else(|| panic!("Session {id} is not listed"))
}

fn rename(home: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let output = run(
        home,
        &[&["session", "rename"][..], args, &["--json"]].concat(),
    );
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn session_names_are_shared_and_human_names_win() {
    let home = tempfile::tempdir().unwrap();
    let (id, _run_id, dir) = prepare_conversation(
        home.path(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "codex",
        "Which release target?",
    );
    let id = id.as_str();
    let seeded = listed(home.path(), id);
    assert_eq!(seeded["title"], "Which release target?");
    assert_eq!(seeded["title_source"], "generated");

    let suggested = rename(home.path(), &[id, "Release", "target", "--suggest"]);
    assert_eq!(suggested["title"], "Release target");
    assert_eq!(suggested["title_source"], "generated");
    let named = rename(home.path(), &[id, "Launch notes"]);
    assert_eq!(named["title"], "Launch notes");
    assert_eq!(named["title_source"], "human");
    assert!(named.get("run_id").is_none());

    let later = run(
        home.path(),
        &["session", "rename", id, "Better guess", "--suggest"],
    );
    assert!(later.status.success(), "{later:?}");
    assert!(String::from_utf8_lossy(&later.stdout).contains("keeps its human-assigned name"));
    let blank = run(home.path(), &["session", "rename", id, "  "]);
    assert!(!blank.status.success());
    let missing = run(home.path(), &["session", "rename", "missing-session", "x"]);
    assert_eq!(
        String::from_utf8_lossy(&missing.stderr).trim(),
        "Error: Session missing-session was not found"
    );

    let readback = listed(home.path(), id);
    assert_eq!(readback["title"], "Launch notes");
    assert_eq!(readback["title_source"], "human");
    let reopened = run(
        home.path(),
        &["session", "resume", id, "--prepare", "--json"],
    );
    let reopened: serde_json::Value = serde_json::from_slice(&reopened.stdout).unwrap();
    assert_eq!(reopened["title"], "Launch notes");
    // Naming touches only the Session row, never capture or provider state.
    assert!(!dir.join("session-name.json").exists());
    assert!(!dir.join("provider-clients").exists());
    assert!(!dir.join("events.jsonl").exists());
    assert!(dir.join("prepared").exists());
}

#[cfg(unix)]
#[test]
fn terminal_titles_follow_session_rename_and_reconnect_without_provider_accounts() {
    use std::fs;
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    for (provider, host, bound) in [
        ("claude", "absent", true),
        ("codex", "absent", true),
        ("claude", "absent", false),
        ("claude", "missing", false),
        ("codex", "failure", false),
        ("claude", "timeout", false),
    ] {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            "AGENTS.md",
            "# Loopflow operating guide\nFollow the repository instructions.",
        );
        let task = bound.then(|| {
            support::register_task_with_pr(
                home.path(),
                &repo.path().canonicalize().unwrap(),
                "main",
                &repo.head_sha(),
            )
        });
        let title = |name: &str| match &task {
            Some(task) => format!("{} {name}", task.task.identifier),
            None => name.to_string(),
        };
        let mut id = String::new();
        let native = uuid::Uuid::new_v4().to_string();
        let transcript = if provider == "codex" {
            home.path()
                .join("codex/sessions")
                .join(format!("rollout-{native}.jsonl"))
        } else {
            home.path()
                .join("claude/projects/test")
                .join(format!("{native}.jsonl"))
        };
        fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        fs::write(
            transcript,
            serde_json::json!({"cwd": repo.path()}).to_string(),
        )
        .unwrap();
        let isolated_command = |args: &[&str]| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
            command
                .env_clear()
                .args(args)
                .current_dir(repo.path())
                .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
                .env("LF_HOME", home.path())
                .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
                .env("RUST_LOG", "off")
                .env("HOME", home.path())
                .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
                .env("CODEX_HOME", home.path().join("codex"));
            command
        };
        let inspect = |args: &[&str]| {
            let output = isolated_command(args).output().unwrap();
            assert!(output.status.success(), "{provider}/{host}: {output:?}");
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
        };
        let write_script = |name: &str, script: &str| {
            let path = bin.join(name);
            fs::write(&path, script).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        };
        write_script(
            provider,
            r#"#!/bin/sh
if [ "$1" = --dangerously-bypass-hook-trust ] && [ "$2" = --model ]; then
    echo "error: a value is required for '--model <MODEL>' but none was supplied" >&2
    exit 2
fi
printf '%s\n' "$@" > "$LF_HOME/args"
printf '%s' "${CLAUDE_CODE_DISABLE_TERMINAL_TITLE-}" > "$LF_HOME/title-disabled"
touch "$LF_HOME/ready"
while [ ! -f "$LF_HOME/release" ]; do sleep 0.02; done
printf 'latest agent message: done\n'
"#,
        );
        if provider == "codex" {
            install_codex_fixture(home.path(), None);
        }
        write_script(
            "cmux",
            r#"#!/bin/sh
case "$TITLE_HOST" in timeout) exec /bin/sleep 10;; esac
exit 2
"#,
        );
        if host == "missing" {
            fs::remove_file(bin.join("cmux")).unwrap();
        }

        for name in ["Plan store migration", "Release notes"] {
            let first = id.is_empty();
            let expected = title(name);
            let mut master = -1;
            let mut slave = -1;
            // SAFETY: valid output pointers, null selects default PTY settings.
            assert_eq!(
                unsafe {
                    libc::openpty(
                        &mut master,
                        &mut slave,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                },
                0
            );
            // SAFETY: openpty returned two independently owned descriptors.
            let mut master = unsafe { fs::File::from_raw_fd(master) };
            // SAFETY: the fresh slave descriptor is owned only by this File.
            let slave = unsafe { fs::File::from_raw_fd(slave) };
            let mut launch = if first {
                isolated_command(&[
                    "-i",
                    "--agent",
                    provider,
                    ":",
                    "Plan store migration for archived tasks",
                ])
            } else {
                isolated_command(&["session", "resume", &id])
            };
            launch
                .env("TITLE_HOST", host)
                .stdin(Stdio::piped())
                .stdout(Stdio::from(slave.try_clone().unwrap()))
                .stderr(Stdio::from(slave));
            if host != "absent" {
                launch
                    .env("CMUX_WORKSPACE_ID", "fixture-workspace")
                    .env("CMUX_SURFACE_ID", "fixture-surface");
            }
            let _ = fs::remove_file(home.path().join("ready"));
            let _ = fs::remove_file(home.path().join("release"));
            let mut child = launch.spawn().unwrap();
            drop(launch);
            let output_reader = std::thread::spawn(move || {
                let mut output = Vec::new();
                // Linux reports EIO when the last PTY slave closes; retain bytes.
                let _ = master.read_to_end(&mut output);
                String::from_utf8_lossy(&output).into_owned()
            });
            let deadline = Instant::now() + Duration::from_secs(30);
            while !home.path().join("ready").exists()
                && Instant::now() < deadline
                && child.try_wait().unwrap().is_none()
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            let started = home.path().join("ready").exists();
            if !started {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "{provider}/{host} failed to start: {}",
                    output_reader.join().unwrap()
                );
            }
            if first {
                let listed = inspect(&["desktop", "sessions", "--all", "--json"]);
                assert_eq!(listed.as_array().unwrap().len(), 1, "{listed}");
                id = listed[0]["id"].as_str().unwrap().to_string();
                assert_eq!(listed[0]["title"], title(name));
                if let Some(task) = &task {
                    assert!(
                        listed[0]["task_ids"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|id| id == task.task.id.as_str()),
                        "{listed}"
                    );
                }
            }
            let args = fs::read_to_string(home.path().join("args")).unwrap();
            if provider == "claude" {
                assert!(args.contains(&format!("--name\n{expected}\n")), "{args}");
                assert_eq!(
                    fs::read_to_string(home.path().join("title-disabled")).unwrap(),
                    "1"
                );
            } else {
                assert!(args.contains("tui.terminal_title=[]"), "{args}");
                assert!(args.contains("--remote\n"), "{args}");
            }
            if first {
                let renamed = inspect(&["session", "rename", &id, "Release notes", "--json"]);
                assert_eq!(renamed["title"], "Release notes");
            } else {
                assert!(
                    args.lines().any(|arg| matches!(arg, "resume" | "--resume")),
                    "{args}"
                );
            }
            if first && provider == "claude" {
                let store =
                    loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db"))
                        .unwrap();
                let input = store.session(&id).unwrap().unwrap().artifact_key.unwrap();
                record_native(home.path(), &id, &input, &native);
            }
            fs::write(home.path().join("release"), "done").unwrap();
            let result = child.wait_with_output().unwrap();
            let terminal = output_reader.join().unwrap();
            assert!(result.status.success(), "{provider}/{host}: {terminal}");
            assert!(
                terminal.contains(&format!("\x1b]0;{expected}\x07")),
                "{terminal:?}"
            );
            assert_eq!(
                terminal.matches("\x1b]0;").count(),
                1,
                "no OSC writer alongside native output"
            );
            assert!(
                terminal.contains("latest agent message: done"),
                "{terminal}"
            );
            let listed = inspect(&["session", "list", "--all", "--history", "--json"]);
            assert_eq!(
                listed
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["id"] == id)
                    .unwrap()["title"],
                "Release notes"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn session_names_survive_capture_replacement() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().unwrap();
    let (id, first_run, first_dir) = prepare_conversation(
        home.path(),
        home.path(),
        "opencode",
        "Which release target?",
    );
    let id = id.as_str();

    // Capture keys and history prefixes cannot select a conversation mutation.
    for selector in [first_run.as_str(), &first_run[..12]] {
        let rejected = run(
            home.path(),
            &["session", "rename", selector, "Wrong target"],
        );
        assert!(!rejected.status.success(), "{rejected:?}");
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("was not found"));
    }
    assert_eq!(listed(home.path(), id)["title"], "Which release target?");
    let suggested = rename(home.path(), &[id, "Release target", "--suggest"]);
    assert_eq!(suggested["id"], id);
    assert_eq!(suggested["title"], "Release target");
    let named = rename(home.path(), &[id, "Launch notes"]);
    assert_eq!(named["title_source"], "human");

    // A consumed launch that never produced provider history is replaced by a
    // new capture on the next open. The human name belongs to the Session.
    std::fs::remove_file(first_dir.join("prepared")).unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("opencode");
    std::fs::write(
        &provider,
        include_str!("support/opencode_server.py").replace("__WAIT__", "True"),
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let evidence = home.path().join("resume-proof");
    let mut opened = command(home.path(), &["session", "resume", id])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("LF_RESUME_PROOF", &evidence)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // An upper bound: a debug `lf` on a busy machine launches slowly.
    // The shell creates the file before writing the id, so wait for the id.
    let deadline = Instant::now() + Duration::from_secs(180);
    let written = || std::fs::read_to_string(&evidence).unwrap_or_default();
    while written().is_empty() && Instant::now() < deadline && opened.try_wait().unwrap().is_none()
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    let started = evidence.exists();
    // Leave the provider waiting while the agent-facing rename runs inside it.
    let replacement = written();
    let inside = command(
        home.path(),
        &[
            "session",
            "rename",
            id,
            "Better guess",
            "--suggest",
            "--json",
        ],
    )
    .env("LF_CAPTURE_KEY", &replacement)
    .output()
    .unwrap();
    let _ = opened
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(b"done\n"));
    let opened = opened.wait_with_output().unwrap();
    assert!(started, "fixture provider did not start: {opened:?}");
    assert!(opened.status.success(), "{opened:?}");
    let prompt = std::fs::read_to_string(evidence.with_extension("args")).unwrap();
    assert!(prompt.contains("Which release target?"));
    assert!(!prompt.contains("The originating Loopflow Run is blocked"));
    assert_ne!(replacement, first_run);

    assert!(inside.status.success(), "{inside:?}");
    let inside: serde_json::Value = serde_json::from_slice(&inside.stdout).unwrap();
    assert_eq!(inside["id"], id);
    assert!(inside.get("run_id").is_none());
    assert_eq!(inside["title"], "Launch notes");
    assert_eq!(inside["title_source"], "human");

    let readback = listed(home.path(), id);
    assert!(readback.get("run_id").is_none());
    assert_eq!(readback["title"], "Launch notes");
    assert_eq!(readback["title_source"], "human");
    // A capture never appears as a second conversation.
    let all: Vec<serde_json::Value> = serde_json::from_slice(
        &run(
            home.path(),
            &["session", "list", "--all", "--history", "--json"],
        )
        .stdout,
    )
    .unwrap();
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(readback["state"], "closed", "the exited orphan is retired");

    assert!(!home
        .path()
        .join("human-sessions")
        .join(format!("{id}.json"))
        .exists());
}

#[test]
fn session_inventory_pages_are_explicit_bounded_and_complete() {
    let home = tempfile::tempdir().unwrap();
    let output = run(
        home.path(),
        &[
            "session", "list", "--page", "--json", "--all", "--limit", "1",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let page: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(page["entries"], serde_json::json!([]));
    assert!(page["next"].is_null());
    for args in [
        &["session", "list", "--page"][..],
        &["session", "list", "--page", "--json", "--limit", "0"],
        &["session", "list", "--page", "--json", "--offset", "1"],
        &["session", "list", "--json", "--after", "old"],
    ] {
        assert!(
            !run(home.path(), args).status.success(),
            "accepted {args:?}"
        );
    }
}

#[test]
fn resume_shorthand_help_and_empty_worktree() {
    let home = tempfile::tempdir().unwrap();
    for args in [&["resume", "--help"][..], &["session", "resume", "--help"]] {
        let output = run(home.path(), args);
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("[ID]"));
        assert!(String::from_utf8_lossy(&output.stdout).contains("--task"));
    }
    for args in [&["resume"][..], &["session", "resume"]] {
        let output = command(home.path(), args)
            .current_dir(home.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not a git work tree"),
            "{output:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn terminal_first_launch_and_failed_startup_reopen_the_same_conversation() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let (id, _, _) = prepare_conversation(home.path(), home.path(), "opencode", "Retained request");
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("opencode");
    // Removing the fixture executable must never fall through to a real provider.
    let path = std::env::join_paths([bin, "/usr/bin".into(), "/bin".into()]).unwrap();
    let open = |args: &[&str]| {
        command(home.path(), args)
            .current_dir(home.path())
            .env("PATH", &path)
            .env("HOME", home.path())
            .output()
            .unwrap()
    };
    let write_provider = |body: &str| {
        std::fs::write(&provider, body).unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    // The private server exits before reporting readiness. OpenCode reports
    // that failure; retry must still reach the provider in the same Session.
    write_provider("#!/bin/sh\nexit 0\n");
    let first = open(&["session", "resume", &id]);
    assert!(!first.status.success());
    assert!(String::from_utf8_lossy(&first.stderr).contains("private server failed to start"));
    write_provider(&include_str!("support/opencode_server.py").replace("__WAIT__", "False"));
    let retry = open(&["session", "resume", &id]);
    assert!(retry.status.success(), "{retry:?}");

    // Discovery finds an executable, but its absent interpreter prevents spawn.
    write_provider(&format!(
        "#!{}\n",
        home.path().join("missing-interpreter").display()
    ));
    let failed = open(&["session", "resume", &id]);
    assert!(!failed.status.success());
    write_provider(&include_str!("support/opencode_server.py").replace("__WAIT__", "False"));
    for _ in 0..2 {
        let result = open(&["session", "resume", &id]);
        assert!(result.status.success(), "{result:?}");
    }
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let session = store.session(&id).unwrap().unwrap();
    assert_eq!(session.id.as_str(), id);
    let history = store.session_history(&id, 0, 1000).unwrap();
    // Failed and exited starts are retained as their own AgentProcess records.
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    for (state, minimum) in [("spawn_failed", 1), ("exited", 1)] {
        let recorded: i64 = db
            .query_row(
                "SELECT count(*) FROM processes
                 WHERE kind='agent' AND agent_session_id=?1 AND spawn_state=?2",
                [id.as_str(), state],
                |row| row.get(0),
            )
            .unwrap();
        assert!(recorded >= minimum, "{state}: {recorded}");
    }
    assert!(
        history
            .iter()
            .filter(|event| event.kind == loopflow::session::SessionEventKind::Captured)
            .count()
            >= 2
    );
    let count: i64 = db
        .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
    assert!(history
        .iter()
        .any(|event| event.payload["evidence"]["provider_session_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("ses_"))));
}

#[cfg(unix)]
#[test]
fn provider_switch_without_transcript_preserves_conversation_and_retries() {
    let home = tempfile::tempdir().unwrap();
    let (id, input, _) =
        prepare_conversation(home.path(), home.path(), "claude", "Keep this conversation");
    let native = uuid::Uuid::new_v4().to_string();
    record_native(home.path(), &id, &input, &native);
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let before = store.session(&id).unwrap().unwrap();
    let history = store.session_history(&id, 0, 1000).unwrap();
    let connect = |prepare: bool| {
        let mut args = vec!["session", "resume", &id, "--agent", "codex"];
        if prepare {
            args.extend(["--prepare", "--json"]);
        }
        command(home.path(), &args)
            .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
            .env("CODEX_HOME", home.path().join("codex"))
            .env(
                "PATH",
                format!("{}/bin:/usr/bin:/bin", home.path().display()),
            )
            .output()
            .unwrap()
    };
    for prepare in [true, false] {
        let failed = connect(prepare);
        assert!(!failed.status.success());
        let error = String::from_utf8_lossy(&failed.stderr);
        assert!(
            error.contains("transcript") && error.contains("retry"),
            "{error}"
        );
        let retained = store.session(&id).unwrap().unwrap();
        assert_eq!(retained.artifact_key.as_ref(), Some(&input));
        assert_eq!(retained.provider, before.provider);
        assert_eq!(retained.title, before.title);
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let thread: String = db
            .query_row(
                "SELECT provider_thread FROM agent_sessions WHERE id=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(thread, native);
        assert_eq!(
            store.session_history(&id, 0, 1000).unwrap().len(),
            history.len()
        );
    }
    let transcripts = home.path().join("claude/projects/fixture");
    std::fs::create_dir_all(&transcripts).unwrap();
    std::fs::write(transcripts.join(format!("{native}.jsonl")),
        r#"{"type":"user","userType":"external","message":{"content":"Keep the original request"}}"#).unwrap();
    let prepared = connect(true);
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    assert_eq!(
        store.session(&id).unwrap().unwrap().artifact_key.as_ref(),
        Some(&input)
    );
    install_codex_fixture(home.path(), None);
    let recovered = connect(false);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let continued = store.session(&id).unwrap().unwrap();
    assert_eq!(continued.id.as_str(), id);
    assert_eq!(continued.title, before.title);
    assert_eq!(continued.provider.as_deref(), Some("codex"));
    assert!(continued
        .request
        .unwrap()
        .contains("Keep the original request"));
    assert!(store.session_history(&id, 0, 1000).unwrap().len() > history.len());
}

/// A Claude conversation with a saved native thread and transcript.
fn claude_conversation_with_transcript(home: &std::path::Path) -> (String, String) {
    let (id, input, _) = prepare_conversation(home, home, "claude", "Keep this conversation");
    let native = uuid::Uuid::new_v4().to_string();
    record_native(home, &id, &input, &native);
    let transcripts = home.join("claude/projects/fixture");
    std::fs::create_dir_all(&transcripts).unwrap();
    std::fs::write(
        transcripts.join(format!("{native}.jsonl")),
        r#"{"type":"user","userType":"external","message":{"content":"Keep the original request"}}"#,
    )
    .unwrap();
    (id, input)
}

fn prepared(home: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let output = command(home, args)
        .env("CLAUDE_CONFIG_DIR", home.join("claude"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[cfg(unix)]
#[test]
fn session_new_restarts_the_same_conversation_on_a_seeded_thread() {
    let home = tempfile::tempdir().unwrap();
    let (id, input) = claude_conversation_with_transcript(home.path());
    let record = prepared(home.path(), &["session", "new", &id, "--prepare", "--json"]);
    assert_eq!(record["id"], id.as_str());
    let argv = record["open_argv"].as_array().unwrap();
    assert_eq!(argv[argv.len() - 3..], ["session", "resume", id.as_str()]);
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let restarted = store.session(&id).unwrap().unwrap();
    assert_eq!(restarted.title, "Keep this conversation");
    assert_eq!(restarted.provider.as_deref(), Some("claude"));
    assert_ne!(restarted.artifact_key.as_ref(), Some(&input));
    assert!(restarted
        .request
        .unwrap()
        .contains("Keep the original request"));
    // The old thread stays in the conversation's history.
    assert!(store
        .session_history(&id, 0, 1000)
        .unwrap()
        .iter()
        .any(|event| event.payload["input_id"] == input.as_str()));
}

#[cfg(unix)]
#[test]
fn resume_fork_continues_in_a_new_conversation_and_leaves_the_original() {
    let home = tempfile::tempdir().unwrap();
    let (id, input) = claude_conversation_with_transcript(home.path());
    let forked = prepared(
        home.path(),
        &["session", "resume", &id, "--fork", "--prepare", "--json"],
    );
    let fork = forked["id"].as_str().unwrap();
    assert_ne!(fork, id);
    let argv = forked["open_argv"].as_array().unwrap();
    assert_eq!(argv[argv.len() - 3..], ["session", "resume", fork]);
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let original = store.session(&id).unwrap().unwrap();
    assert_eq!(original.artifact_key.as_ref(), Some(&input));
    assert!(original.completed_at.is_none());
    let fork = store.session(fork).unwrap().unwrap();
    assert_eq!(fork.provider.as_deref(), Some("claude"));
    assert_eq!(fork.cwd, original.cwd);
    assert!(fork.request.unwrap().contains("Keep the original request"));
}

#[cfg(unix)]
#[test]
fn provider_switch_stops_startup_output_before_codex_opens() {
    use std::fs;
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().unwrap();
    let (id, input, _) = prepare_conversation(
        home.path(),
        home.path(),
        "claude",
        "Continue this conversation",
    );
    let native = uuid::Uuid::new_v4().to_string();
    record_native(home.path(), &id, &input, &native);
    let transcripts = home.path().join("claude/projects/fixture");
    fs::create_dir_all(&transcripts).unwrap();
    fs::write(
        transcripts.join(format!("{native}.jsonl")),
        r#"{"type":"user","userType":"external","message":{"content":"Keep the original request"}}"#,
    )
    .unwrap();
    install_codex_fixture(home.path(), None);
    let provider = home.path().join("bin/codex");
    let script = fs::read_to_string(&provider).unwrap().replace(
        "#!/bin/sh\n",
        "#!/bin/sh\ncase \" $* \" in *' --remote '*) printf 'NATIVE_TERMINAL_READY\\n' >&2; sleep 1;; esac\n",
    );
    fs::write(&provider, script).unwrap();

    let mut master = -1;
    let mut slave = -1;
    // SAFETY: valid output pointers; null selects default PTY settings.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // SAFETY: openpty returned independently owned descriptors.
    let mut master = unsafe { fs::File::from_raw_fd(master) };
    // SAFETY: the fresh slave descriptor is owned only by this File.
    let slave = unsafe { fs::File::from_raw_fd(slave) };
    let mut launch = command(home.path(), &["session", "resume", &id, "-a", "codex"]);
    launch
        .process_group(0)
        .env_clear()
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
        .env("CODEX_HOME", home.path().join("codex"))
        .env(
            "PATH",
            format!("{}/bin:/usr/bin:/bin", home.path().display()),
        )
        .env("TERM", "xterm-256color")
        .env("RUST_LOG", "off")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(slave));
    let mut child = launch.spawn().unwrap();
    drop(launch);
    let reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        // Linux reports EIO when the last slave closes; retain the captured bytes.
        let _ = master.read_to_end(&mut output);
        String::from_utf8_lossy(&output).into_owned()
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait().unwrap().is_none() {
        // SAFETY: only this fixture's newly created process group is signaled.
        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    }
    let status = child.wait().unwrap();
    let terminal = reader.join().unwrap();
    assert!(status.success(), "{terminal:?}");
    let (_, native_output) = terminal.split_once("NATIVE_TERMINAL_READY").unwrap();
    assert!(
        !native_output.contains("Preparing conversation") && !native_output.contains("\x1b[2K"),
        "launcher wrote into the native terminal: {native_output:?}"
    );
    assert!(fs::read_to_string(
        home.path()
            .join("runs")
            .join(&input[4..6])
            .join(input)
            .join("conversation.md")
    )
    .unwrap()
    .contains("Keep the original request"));
}

fn record_native(home: &std::path::Path, id: &str, input: &str, native: &str) {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
        [id, native],
    )
    .unwrap();
    db.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
        SELECT id,'observed',?2||':fixture-native',created_at,json_object('input_id',?2,'source','provider-session:fixture','evidence',json_object('schema_version',1,'provider_session_id',?3,'account_id',NULL)),current_capture
        FROM agent_sessions WHERE id=?1", [id,input,native]).unwrap();
}

#[cfg(unix)]
#[test]
fn resume_admits_native_claude_and_codex_ids_and_keeps_their_identity() {
    use std::os::unix::fs::PermissionsExt;
    for provider in ["claude", "codex"] {
        let home = tempfile::tempdir().unwrap();
        let native = home.path().join(provider);
        let id = uuid::Uuid::new_v4().to_string();
        let transcript = if provider == "codex" {
            native
                .join("sessions/2026/10/04")
                .join(format!("rollout-2026-10-04T00-00-00-{id}.jsonl"))
        } else {
            native.join("projects/test").join(format!("{id}.jsonl"))
        };
        std::fs::create_dir_all(transcript.parent().unwrap()).unwrap();
        std::fs::write(
            &transcript,
            serde_json::json!({"cwd":home.path()}).to_string(),
        )
        .unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let executable = bin.join(provider);
        std::fs::write(
            &executable,
            "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s\\n' \"$@\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        if provider == "codex" {
            install_codex_fixture(home.path(), Some(&id));
        }
        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        let resume = |args: &[&str]| {
            command(home.path(), args)
                .current_dir(home.path())
                .env("PATH", &path)
                .env("CODEX_HOME", home.path().join("codex"))
                .env("CLAUDE_CONFIG_DIR", home.path().join("claude"))
                .output()
                .unwrap()
        };
        for args in [
            vec!["resume", id.as_str()],
            vec!["session", "resume", id.as_str()],
        ] {
            let output = resume(&args);
            assert!(output.status.success(), "{provider}: {output:?}");
            assert!(String::from_utf8_lossy(&output.stdout).contains(&id));
        }
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let count: i64 = db
            .query_row(
                "SELECT count(DISTINCT session_id) FROM session_events WHERE json_extract(payload,'$.evidence.provider_session_id')=?1",
                [&id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        // The same UUID in both providers must remain ambiguous, with no admission.
        let ambiguous = uuid::Uuid::new_v4().to_string();
        for relative in [
            format!("claude/projects/test/{ambiguous}.jsonl"),
            format!("codex/sessions/2026/10/04/rollout-time-{ambiguous}.jsonl"),
        ] {
            let file = home.path().join(relative);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "{}").unwrap();
        }
        let output = resume(&["resume", &ambiguous]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("ambiguous"),
            "{output:?}"
        );
        let count: i64 = db
            .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

#[cfg(unix)]
#[test]
fn program_status_cli_observes_available_without_completing_work() {
    use std::io::Write;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let home = tempfile::tempdir().unwrap();
    let (id, _, _) = prepare_conversation(home.path(), home.path(), "sleep", "Status fixture");
    let mut provider = Child(Command::new("/bin/sleep").arg("60").spawn().unwrap());
    let now = time::OffsetDateTime::now_utc();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let lf_process_id = loopflow::id::LfProcessId::new();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'00000000-0000-0000-0000-000000000001',1)",
        [&lf_process_id],
    )
    .unwrap();
    let attachment = store
        .claim_session_attachment(&id, None, &lf_process_id, true)
        .unwrap();
    conn.execute(
        "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
        rusqlite::params![
            attachment.agent_process_id,
            provider.0.id(),
            now.unix_timestamp()
        ],
    )
    .unwrap();
    conn.execute(
        "UPDATE processes SET terminal_id='fixture-pane' WHERE id=?1",
        [&lf_process_id],
    )
    .unwrap();
    let mut observer = Child(
        command(
            home.path(),
            &[
                "session",
                "observe-status",
                &id,
                "--terminal",
                "fixture-pane",
                "--agent-process",
                attachment.agent_process_id.as_str(),
            ],
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap(),
    );
    let mut input = observer.0.stdin.take().unwrap();
    let records = |state: &str| {
        serde_json::json!({"seen":true,"records":[{
        "state":state,"id":"worker","kind":if state=="blocked" { Some("question") } else { None },
        "progress":null,"app":"fixture","title":null,"msg":"Use **literal** text?"}]})
    };
    let wait_for = |state: &str| {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let output = run(home.path(), &["desktop", "sessions", "--json", "--all"]);
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let items: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            if let Some(item) = items
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == id && s["program_status"]["records"][0]["state"] == state)
            {
                return item.clone();
            }
            assert!(
                Instant::now() < until,
                "observer did not publish {state}: {items}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    // The pipe also fragments a JSON frame across reads.
    let blocked = records("blocked").to_string() + "\n";
    input.write_all(&blocked.as_bytes()[..17]).unwrap();
    input.write_all(&blocked.as_bytes()[17..]).unwrap();
    let item = wait_for("blocked");
    assert_eq!(item["availability"], "available");
    assert_eq!(
        item["program_status"]["records"][0]["msg"],
        "Use **literal** text?"
    );
    let page = run(
        home.path(),
        &[
            "session",
            "list",
            "--json",
            "--all",
            "--available",
            "--page",
            "--limit",
            "1",
        ],
    );
    let page: serde_json::Value = serde_json::from_slice(&page.stdout).unwrap();
    assert_eq!(page["entries"][0]["id"], id);
    for state in ["working", "done"] {
        writeln!(input, "{}", records(state)).unwrap();
        assert!(wait_for(state)["availability"].is_null());
    }
    assert!(store.session(&id).unwrap().unwrap().completed_at.is_none());
    assert!(store
        .process(&lf_process_id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
    let mut invalid = records("blocked");
    invalid["records"][0]["msg"] = serde_json::json!("bad\ncontrol");
    writeln!(input, "{invalid}").unwrap();
    drop(input);
    assert!(!observer.0.wait().unwrap().success());
    assert_eq!(
        wait_for("done")["program_status"]["records"][0]["state"],
        "done"
    );
    provider.0.kill().unwrap();
    provider.0.wait().unwrap();
    let output = run(home.path(), &["desktop", "sessions", "--json", "--all"]);
    let items: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ended = items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == id)
        .unwrap();
    assert_eq!(ended["state"], "closed");
    assert!(ended["program_status"].is_null());
    assert!(ended["availability"].is_null());
    let attachment = store.session_attachment(&id).unwrap();
    store
        .claim_session_attachment(&id, attachment.as_ref(), &lf_process_id, true)
        .unwrap();
    let stale = run(
        home.path(),
        &[
            "session",
            "observe-status",
            &id,
            "--terminal",
            "fixture-pane",
        ],
    );
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("Session provider changed"));
    let output = run(home.path(), &["desktop", "sessions", "--json", "--all"]);
    let items: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let item = items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == id)
        .unwrap();
    assert!(item["program_status"].is_null());
    assert!(item["availability"].is_null());
}

#[test]
fn instruction_inspection_reports_recorded_reasons_and_missing_evidence() {
    let home = tempfile::tempdir().unwrap();
    let (id, _, dir) = prepare_conversation(
        home.path(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "codex",
        "Do not launch",
    );
    let path = dir.join("context.json");
    let mut artifact = serde_json::json!({
        "context": {"system": null, "task": {"assets": []}, "decisions": []}
    });
    artifact["context"]["decisions"] = serde_json::json!([
        {
            "position": 0, "kind": "operating_instructions", "scope": "global",
            "label": "work", "source_path": null, "decision": "excluded",
            "reason": "no Task or Wave association", "original_bytes": null,
            "original_tokens": null, "asset_position": null
        },
        {
            "position": 1, "kind": "memory", "scope": "repo",
            "label": "memory", "source_path": null, "decision": "included",
            "reason": "captured memory", "original_bytes": null,
            "original_tokens": null, "asset_position": null
        }
    ]);
    std::fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();
    let args = ["history", "show", &id, "--instructions"];
    let output = run(home.path(), &args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("excluded  work — no Task or Wave association"));
    assert!(!text.contains("captured memory"));
    let output = run(
        home.path(),
        &["history", "show", &id, "--instructions", "--json"],
    );
    assert!(output.status.success());
    let decisions: Vec<loopflow::trace::ContextDecision> =
        serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].reason, "no Task or Wave association");
    assert!(!run(
        home.path(),
        &["history", "show", &id, "--instructions", "--context"]
    )
    .status
    .success());
    std::fs::remove_file(path).unwrap();
    let output = run(home.path(), &args);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Instruction evidence unavailable"));
}

#[test]
fn hooks_from_an_earlier_lf_never_block_their_provider() {
    let home = tempfile::tempdir().unwrap();
    let output = command(home.path(), &["__planning-check"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    // Claude treats exit 2 at Stop as "keep working", so a removed callback exits 0.
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("restart the Session"));
}
