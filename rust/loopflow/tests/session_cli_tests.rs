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
    std::fs::write(&other, "#!/bin/sh\necho wrong-Home >&2\nexit 91\n").unwrap();
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
                "#!/bin/sh\necho 'unexpected argument --tui' >&2\nexit 2\n",
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
            loopflow::session::AgentSession {
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
                flow_id: None,
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
            "INSERT INTO session_activity(session_id,driver_generation,observed_at,open_tools,pending_input,yielded)
             SELECT id,driver_generation,unixepoch(),1,?2,0 FROM agent_sessions WHERE id=?1",
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
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
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
    std::fs::write(&provider, "#!/bin/sh\nexit 1\n").unwrap();
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
