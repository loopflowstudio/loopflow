mod support;

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
        .env_remove("LF_PROCESS_LFID")
        .env_remove("LF_FLOW_ID")
        .env_remove("LF_HUMAN_SESSION")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_TERMINAL_ID")
        .env_remove("LF_TERMINAL_TTY")
        .env("RUST_LOG", "off");
    command
}

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
    assert!(help.contains("connect"));
    assert!(!help.contains("accept"));
    assert!(!help.contains("decline"));
    assert!(!help.contains("send-back"));

    let removed = run(
        home.path(),
        &["monitor", "show", "historical-input", "--resume"],
    );
    assert!(!removed.status.success());
    assert!(String::from_utf8_lossy(&removed.stderr).contains("unexpected argument '--resume'"));

    for args in [
        &["session", "ready", "Feedback"][..],
        &["session", "complete", "missing-session"],
        &["session", "stop-client", "missing-input"],
        &["session", "serve-flow"],
    ] {
        let output = run(home.path(), args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
    }
    let output = run(home.path(), &["session", "connect"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("required"));
    let output = run(
        home.path(),
        &["session", "connect", "missing-session", "--json"],
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
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let path = std::env::join_paths(paths).unwrap();
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
            "LF_PROCESS_LFID",
            "LF_HUMAN_SESSION",
            "LF_FLOW_ID",
        ] {
            command.env_remove(name);
        }
        command
    };
    let prepared = command(env!("CARGO_BIN_EXE_lf"))
        .args(["session", "connect", &id, "--json"])
        .output()
        .unwrap();
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let output = command(env!("CARGO_BIN_EXE_lf"))
        .args(["session", "list", "--json", "--all"])
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
    let opened = run(home.path(), &["session", "connect", &id, "--json"]);
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
    let inspected = run(home.path(), &["monitor", "show", &id, "--json"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspected: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(inspected["artifact_key"], run_id.as_str());
    assert_eq!(inspected["caller_artifact_key"], CALLER);
    assert!(inspected["recorded_outcome"].is_null());
    let listed = run(home.path(), &["session", "list", "--all", "--json"]);
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
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let provider = bin.join("opencode");
        std::fs::write(
        &provider,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s' \"$LF_CAPTURE_KEY\" > \"$LF_RESUME_PROOF\"\nprintf '%s' \"$LF_AGENT_CALLER\" > \"$LF_RESUME_PROOF.caller\"\nprintf '%s\\n' 'message=created id=ses_resume-proof' >&2\nread -r input\n",
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
            let failed = command(home.path(), &["session", "connect", id])
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
            assert!(!error.contains("Local proof"), "prompt leaked: {error}");
        }
        let evidence = home.path().join("resumed");
        let mut first = command(home.path(), &["session", "connect", id])
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
            &["session", "connect", selector, "--try", "--json"],
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
        let active = run(home.path(), &["monitor", "active", "--json"]);
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
        let recorded: (i64, String) = rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT provider_generation,provider_process_lfid FROM agent_sessions WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (
                caller.provider_generation,
                caller.origin_process_lfid.to_string()
            ),
            recorded
        );

        assert_eq!(reopened["state"], "active");
        assert!(active.status.success(), "{:?}", active);
        let active: serde_json::Value = serde_json::from_slice(&active.stdout).unwrap();
        assert_eq!(active["gaps"], serde_json::json!([]), "{active}");
        assert_eq!(active["sessions"].as_array().unwrap().len(), 1, "{active}");
        assert_eq!(active["sessions"][0]["id"], id);
        assert_eq!(active["sessions"][0]["processes"][0]["state"], "waiting");
        let ended = run(home.path(), &["monitor", "active", "--json"]);
        let ended: serde_json::Value = serde_json::from_slice(&ended.stdout).unwrap();
        assert_eq!(ended["sessions"], serde_json::json!([]));
        if !resume {
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
            assert_eq!(manifest["artifact_key"], run_id);
            assert!(manifest["context"].is_object());
            assert!(!dir.join("prepared").exists());
            let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
            let failure: String = database
                .query_row(
                    "SELECT error FROM processes WHERE error LIKE '%rejecting-lf%' AND error LIKE '%before becoming resumable%'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(failure.contains(run_id), "{failure}");
            assert!(failure.contains("sha256"), "{failure}");
            assert!(failure.contains(home.path().to_str().unwrap()), "{failure}");
            assert!(!failure.contains("Local proof"), "prompt leaked: {failure}");
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

#[test]
fn native_title_callback_keeps_lf_as_the_naming_owner() {
    use std::io::Write;
    use std::process::Stdio;

    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".lf")).unwrap();
    let (id, _, _) = prepare_conversation(home.path(), home.path(), "claude", "Original purpose");
    let database = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    database
        .execute(
            "UPDATE agent_sessions SET provider_thread='native-owned' WHERE id=?1",
            [&id],
        )
        .unwrap();
    for thread in ["native-owned", "native-plain"] {
        let mut child = command(home.path(), &["__session-title", "claude"])
            .env("HOME", home.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        write!(
            child.stdin.take().unwrap(),
            "{}",
            serde_json::json!({
                "cwd": home.path(), "session_id": thread, "hook_event_name":"UserPromptSubmit",
                "prompt":"A different request entirely"
            })
        )
        .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{output:?}");
        if thread == "native-owned" {
            assert!(output.stdout.is_empty(), "{output:?}");
        } else {
            let title: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                title["hookSpecificOutput"]["sessionTitle"],
                "different request entirely"
            );
        }
    }
}

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
                id: format!("session_{input}"),
                artifact_key: input.clone(),
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
                flow_process_lfid: None,
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
    (session.id, input, dir)
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
        let process = loopflow::id::ProcessLfid::new();
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .execute(
                "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let driver = store
            .claim_session_driver(&id, None, &process, true)
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
                "--no-loopflow",
                "session",
                "resume",
                &id,
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
        assert!(error.contains("already has a driver"), "{harness}: {error}");
        assert!(!error.contains("unexpected-provider-launch"), "{error}");
        assert_eq!(store.session(&id).unwrap().unwrap(), before);
        assert_eq!(store.session_driver(&id).unwrap(), Some(driver));
        assert_eq!(store.session_history(&id, 0, 0).unwrap(), history);
        assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), manifest);
        assert!(dir.join("prepared").exists());
        assert_eq!(store.session(&id).unwrap().unwrap().artifact_key, input);
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
if [ "${1:-}" = --version ]; then exit 0; fi
pwd -P > "$LF_TEST_RESUME_PROOF.cwd"
printf '%s\n' "$@" > "$LF_TEST_RESUME_PROOF.args"
printf '%s\n' "$LF_AGENT_CALLER" > "$LF_TEST_RESUME_PROOF.caller"
while [ "$#" -gt 0 ]; do
    if [ "$1" = --append-system-prompt-file ]; then
        cat "$2" > "$LF_TEST_RESUME_PROOF.context"
        break
    fi
    shift
done
cat > "$LF_TEST_RESUME_PROOF.input"
printf '%s\n' '{"type":"result","session_id":"fixture-native","subtype":"success","result":"continued"}'
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
    std::fs::create_dir_all(saved.join("wave/caller-wave")).unwrap();
    std::fs::write(
        saved.join("wave/caller-wave/GOAL.md"),
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
        &[
            "-b",
            "--no-loopflow",
            "session",
            "resume",
            &id,
            "Continue here",
        ],
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
    assert!(sent.contains("Continue here"), "{sent}");
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
    assert_ne!(after.artifact_key, input);
    assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
    assert!(store
        .session_history(&id, 0, 0)
        .unwrap()
        .starts_with(&history));
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            home.path()
                .join("runs")
                .join(&after.artifact_key[..2])
                .join(&after.artifact_key)
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
        .process(&agent_caller.origin_process_lfid)
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
fn waiting_lists_only_conversations_waiting_on_a_person() {
    let home = tempfile::tempdir().unwrap();
    let cwd = home.path().join("work");
    std::fs::create_dir(&cwd).unwrap();
    let (working, _, _) = prepare_conversation(home.path(), &cwd, "codex", "Working");
    let (asked, _, _) = prepare_conversation(home.path(), &cwd, "codex", "Asked");
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    for (id, pending) in [(&working, 0), (&asked, 1)] {
        db.execute(
            "INSERT INTO session_activity(session_id,driver_generation,provider_generation,observed_at,open_tools,pending_input,yielded)
             SELECT id,driver_generation,provider_generation,unixepoch(),1,?2,0 FROM agent_sessions WHERE id=?1",
            rusqlite::params![id, pending],
        )
        .unwrap();
    }
    let list = |flag: &str| run(home.path(), &["session", "list", "--all", "--json", flag]);
    let waiting = list("--waiting");
    assert!(waiting.status.success(), "{waiting:?}");
    let waiting: Vec<serde_json::Value> = serde_json::from_slice(&waiting.stdout).unwrap();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0]["id"], asked.as_str());
    assert_eq!(waiting[0]["attention"], "waiting");
    assert!(listed(home.path(), &working)["attention"].is_null());
    assert!(!list("--needs-me").status.success());
}

fn listed(home: &std::path::Path, id: &str) -> serde_json::Value {
    let output = run(home, &["session", "list", "--all", "--history", "--json"]);
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
    let reopened = run(home.path(), &["session", "connect", id, "--json"]);
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
    use std::io::{Read, Write};
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
            Some(task) => format!("{} {name}", task.task.plan.identifier),
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
IFS= read -r answer
printf 'latest agent message: %s\n' "$answer"
"#,
        );
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
                isolated_command(&["session", "connect", &id])
            };
            launch
                .env("TITLE_HOST", host)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::from(slave));
            if host != "absent" {
                launch
                    .env("CMUX_WORKSPACE_ID", "fixture-workspace")
                    .env("CMUX_SURFACE_ID", "fixture-surface");
            }
            let _ = fs::remove_file(home.path().join("ready"));
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
                let listed = inspect(&["session", "list", "--all", "--json"]);
                assert_eq!(listed.as_array().unwrap().len(), 1, "{listed}");
                id = listed[0]["id"].as_str().unwrap().to_string();
                assert_eq!(listed[0]["title"], name);
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
            if first {
                let store =
                    loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db"))
                        .unwrap();
                let input = store.session(&id).unwrap().unwrap().artifact_key;
                record_native(home.path(), &id, &input, &native);
            }
            child.stdin.take().unwrap().write_all(b"done\n").unwrap();
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
            assert!(String::from_utf8_lossy(&result.stdout).contains("latest agent message: done"));
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
        "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s' \"$LF_CAPTURE_KEY\" > \"$LF_RESUME_PROOF.tmp\"\nmv \"$LF_RESUME_PROOF.tmp\" \"$LF_RESUME_PROOF\"\nprintf '%s\\n' \"$@\" > \"$LF_RESUME_PROOF.args\"\nprintf '%s\\n' 'message=created id=ses_rename-proof' >&2\nread -r input\n",
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let evidence = home.path().join("launched");
    let mut opened = command(home.path(), &["session", "connect", id])
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
    }
    for args in [&["resume"][..], &["session", "resume"]] {
        let output = command(home.path(), args)
            .current_dir(home.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("No interactive session found in this worktree"),
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
    // The terminal exits before publishing native history. OpenCode reports
    // that failure; retry must still reach the provider in the same Session.
    write_provider("#!/bin/sh\nexit 0\n");
    let first = open(&["session", "connect", &id]);
    assert!(!first.status.success());
    assert!(String::from_utf8_lossy(&first.stderr).contains("did not report a resumable session"));
    write_provider("#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s\\n' 'message=created id=ses_retained' >&2\n");
    let retry = open(&["session", "resume"]);
    assert!(retry.status.success(), "{retry:?}");

    // Discovery finds an executable, but its absent interpreter prevents spawn.
    write_provider(&format!(
        "#!{}\n",
        home.path().join("missing-interpreter").display()
    ));
    let failed = open(&["session", "connect", &id, "--replace"]);
    assert!(!failed.status.success());
    assert!(!String::from_utf8_lossy(&failed.stderr).contains("no confirmed engine exit"));
    write_provider("#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s\\n' 'message=created id=ses_retained' >&2\n");
    for args in [
        vec!["session", "resume"],
        vec!["session", "connect", &id, "--replace"],
    ] {
        let result = open(&args);
        assert!(result.status.success(), "{result:?}");
    }
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let session = store.session(&id).unwrap().unwrap();
    assert_eq!(session.id, id);
    let history = store.session_history(&id, 0, 1000).unwrap();
    assert!(history
        .iter()
        .any(|event| event.payload["phase"] == "spawn_failed"));
    assert!(history
        .iter()
        .any(|event| event.payload["phase"] == "exited"));
    assert!(
        history
            .iter()
            .filter(|event| event.kind == loopflow::session::SessionEventKind::Captured)
            .count()
            >= 2
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let count: i64 = db
        .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
    assert!(history
        .iter()
        .any(|event| event.payload["evidence"]["provider_session_id"] == "ses_retained"));
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
fn resume_selects_human_input_in_the_physical_worktree_and_records_opening() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let home = tempfile::tempdir().unwrap();
    let repo = home.path().join("repo");
    let sibling = home.path().join("sibling");
    std::fs::create_dir_all(repo.join("subdir")).unwrap();
    std::fs::create_dir(&sibling).unwrap();
    assert!(Command::new("git")
        .args(["init", "--quiet"])
        .arg(&repo)
        .status()
        .unwrap()
        .success());
    let link = home.path().join("link");
    symlink(&repo, &link).unwrap();
    let (a, input_a, _) = prepare_conversation(home.path(), &link.join("subdir"), "codex", "A");
    let (b, input_b, _) = prepare_conversation(home.path(), &repo, "codex", "B");
    let (background, _, _) = prepare_conversation(home.path(), &repo, "codex", "Background");
    let (other, _, _) = prepare_conversation(home.path(), &sibling, "codex", "Other");
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute(
        "UPDATE agent_sessions SET interactive=0,created_at=9999999999 WHERE id=?1",
        [&background],
    )
    .unwrap();
    db.execute(
        "UPDATE agent_sessions SET created_at=9999999999 WHERE id=?1",
        [&other],
    )
    .unwrap();
    db.execute("UPDATE agent_sessions SET completed_at=2 WHERE id=?1", [&a])
        .unwrap();
    record_native(home.path(), &a, &input_a, "native-a");
    record_native(home.path(), &b, &input_b, "native-b");
    let codex = home.path().join("codex");
    std::fs::create_dir(&codex).unwrap();
    std::fs::write(
        codex.join("history.jsonl"),
        "{\"session_id\":\"native-b\",\"ts\":10}\n{\"session_id\":\"native-a\",\"ts\":20}\n",
    )
    .unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("codex");
    std::fs::write(&provider, "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s' \"$LF_CAPTURE_KEY\" > \"$CODEX_HOME/opened\"\n").unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    // An unavailable fixture must never fall through to a real provider.
    let path = std::env::join_paths([bin, "/usr/bin".into(), "/bin".into()]).unwrap();
    let resume = |args: &[&str]| {
        command(home.path(), args)
            .current_dir(repo.join("subdir"))
            .env("PATH", &path)
            .env("CODEX_HOME", &codex)
            .output()
            .unwrap()
    };
    let result = resume(&["resume"]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        std::fs::read_to_string(codex.join("opened")).unwrap(),
        input_a
    );
    let openings: i64 = db.query_row("SELECT count(*) FROM session_events WHERE session_id=?1 AND json_extract(payload,'$.type')='interactive_opened'", [&a], |row| row.get(0)).unwrap();
    assert_eq!(openings, 1);
    // No native input: the recorded opening supplies recency, including completed conversations.
    std::fs::remove_file(codex.join("history.jsonl")).unwrap();
    let result = resume(&["session", "resume"]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(
        std::fs::read_to_string(codex.join("opened")).unwrap(),
        input_a
    );
    for selector in [&b, "native-b"] {
        let result = resume(&["resume", selector]);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(
            std::fs::read_to_string(codex.join("opened")).unwrap(),
            input_b
        );
    }
    // An unavailable provider must not add another opening receipt.
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o644)).unwrap();
    let before: i64 = db.query_row("SELECT count(*) FROM session_events WHERE json_extract(payload,'$.type')='interactive_opened'", [], |row| row.get(0)).unwrap();
    assert!(!resume(&["resume", &b]).status.success());
    let after: i64 = db.query_row("SELECT count(*) FROM session_events WHERE json_extract(payload,'$.type')='interactive_opened'", [], |row| row.get(0)).unwrap();
    assert_eq!(before, after);
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
fn program_status_cli_observes_waiting_without_completing_work() {
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
    let (id, _, dir) = prepare_conversation(home.path(), home.path(), "sleep", "Status fixture");
    let mut provider = Child(Command::new("/bin/sleep").arg("60").spawn().unwrap());
    let now = time::OffsetDateTime::now_utc();
    std::fs::create_dir_all(dir.join("provider-clients")).unwrap();
    std::fs::write(
        dir.join("provider-clients")
            .join(format!("{}.json", provider.0.id())),
        serde_json::json!({"schema_version":1,"pid":provider.0.id(),"terminal_id":"fixture-pane",
            "started_at":now.format(&time::format_description::well_known::Rfc3339).unwrap()})
        .to_string(),
    )
    .unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let process_lfid = loopflow::id::ProcessLfid::new();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute(
        "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'00000000-0000-0000-0000-000000000001',1)",
        [&process_lfid],
    )
    .unwrap();
    // A native conversation needs no lf driver claim for passive display.
    let generation = "0".to_string();
    let mut observer = Child(
        command(
            home.path(),
            &[
                "session",
                "observe-status",
                &id,
                "--terminal",
                "fixture-pane",
                "--generation",
                &generation,
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
            let output = run(home.path(), &["session", "list", "--json", "--all"]);
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
    assert_eq!(item["attention"], "waiting");
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
            "--waiting",
            "--page",
            "--limit",
            "1",
        ],
    );
    let page: serde_json::Value = serde_json::from_slice(&page.stdout).unwrap();
    assert_eq!(page["entries"][0]["id"], id);
    for state in ["working", "done"] {
        if state == "done" {
            provider.0.kill().unwrap();
            provider.0.wait().unwrap();
        }
        writeln!(input, "{}", records(state)).unwrap();
        assert!(wait_for(state)["attention"].is_null());
    }
    assert!(store.session(&id).unwrap().unwrap().completed_at.is_none());
    assert!(store
        .process(&process_lfid)
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
    let driver = store.session_driver(&id).unwrap();
    store
        .claim_session_driver(&id, driver.as_ref(), &process_lfid, true)
        .unwrap();
    let stale = run(
        home.path(),
        &[
            "session",
            "observe-status",
            &id,
            "--terminal",
            "fixture-pane",
            "--generation",
            &generation,
        ],
    );
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("Session provider changed"));
    let output = run(home.path(), &["session", "list", "--json", "--all"]);
    let items: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let item = items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == id)
        .unwrap();
    assert!(item["program_status"].is_null());
    assert!(item["attention"].is_null());
}
