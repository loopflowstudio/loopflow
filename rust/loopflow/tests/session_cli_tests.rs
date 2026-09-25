use std::process::{Command, Output};

fn command(home: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("LF_HOME", home)
        .env("LF_CONTROL_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("LF_CONTROL_DB_PATH", home.join("loopflow.db"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env_remove("LF_RUN_ID")
        .env_remove("LF_RUN_DIR")
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
    assert!(!help.contains("advance"));
    assert!(!help.contains("iterate"));
    assert!(help.contains("complete"));
    assert!(!help.contains("accept"));
    assert!(!help.contains("decline"));
    assert!(!help.contains("send-back"));

    for args in [
        &["session", "ready"][..],
        &["session", "complete"],
        &["session", "open"],
    ] {
        let output = run(home.path(), args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("required"));
    }

    for args in [
        &["session", "open", "missing-session", "--json"][..],
        &["session", "complete", "missing-session"],
    ] {
        let output = run(home.path(), args);
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            "Error: Session missing-session was not found"
        );
    }
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
    let id = format!("ask_{}", uuid::Uuid::new_v4().simple());
    let sessions = home.path().join("human-sessions");
    std::fs::create_dir(&sessions).unwrap();
    let record = serde_json::json!({
        "id": id, "parent_run_id": loopflow::durable::RunId::new(),
        "parent_run_dir": home.path(), "work": null, "work_selector": null,
        "title": "Keep this development review", "detail": "loop-decide",
        "prompt": "Choose the next proof", "skill": "unblock",
        "cwd": env!("CARGO_MANIFEST_DIR"), "model": "codex", "session_run_id": null,
        "ready_summary": null, "status": "waiting", "retain_completed": true
    });
    std::fs::write(sessions.join(format!("{id}.json")), record.to_string()).unwrap();
    let command = |binary: &str| {
        let mut command = Command::new(binary);
        command
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("PATH", &path)
            .env("LF_HOME", home.path())
            .env("LF_DB_PATH", home.path().join("loopflow.db"))
            .env("RUST_LOG", "off");
        for name in [
            "LF_BIN",
            "CARGO_BIN_EXE_lf",
            "LF_CONTROL_BIN",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
            "LF_RUN_ID",
            "LF_RUN_DIR",
            "LF_RUN_CONTEXT",
            "LF_HUMAN_SESSION",
            "LF_FLOW_STEP",
        ] {
            command.env_remove(name);
        }
        command
    };
    let prepared = command(env!("CARGO_BIN_EXE_lf"))
        .args(["session", "open", &id, "--json"])
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
    assert_eq!(
        std::fs::canonicalize(argv[0].as_str().unwrap()).unwrap(),
        std::fs::canonicalize(env!("CARGO_BIN_EXE_lf")).unwrap()
    );
    let reopened = command(argv[0].as_str().unwrap())
        .args(argv[1..].iter().map(|arg| arg.as_str().unwrap()))
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
}

#[test]
fn unopened_session_has_a_run_before_any_provider_is_started() {
    let home = tempfile::tempdir().unwrap();
    let sessions = home.path().join("human-sessions");
    std::fs::create_dir(&sessions).unwrap();
    // A stored Ask from before required Run references. Read-only discovery
    // must not invent a Run; explicit opening prepares it without spawning.
    let id = "ask_prepared-proof";
    let record = serde_json::json!({
        "id": id,
        "parent_run_id": "run_00000000000000000000000000000002",
        "parent_run_dir": home.path().join("parent"),
        "work": null, "work_selector": null,
        "title": "A question", "detail": "proof", "prompt": "Do not launch",
        "cwd": env!("CARGO_MANIFEST_DIR"), "model": "codex",
        "session_run_id": null, "ready_summary": null, "status": "waiting"
    });
    std::fs::write(
        sessions.join(format!("{id}.json")),
        serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let before = run(home.path(), &["session", "list", "--all", "--json"]);
    assert!(!before.status.success());
    assert!(String::from_utf8_lossy(&before.stderr).contains("predates prepared Runs"));
    assert!(!home.path().join("runs").exists());
    let opened = run(home.path(), &["session", "open", id, "--json"]);
    assert!(
        opened.status.success(),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let opened: serde_json::Value = serde_json::from_slice(&opened.stdout).unwrap();
    let run_id = opened["run_id"].as_str().unwrap();
    assert_eq!(opened["state"], "waiting");
    assert_ne!(run_id, record["parent_run_id"].as_str().unwrap());
    let dir = home.path().join("runs").join(&run_id[4..6]).join(run_id);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["run_id"], run_id);
    assert_eq!(manifest["parent_run_id"], record["parent_run_id"]);
    assert!(dir.join("prepared").exists());
    assert!(!dir.join("provider-clients").exists());
    assert!(!dir.join("terminal.json").exists());
    let inspected = run(home.path(), &["runs", run_id, "--json"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspected: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(inspected["id"], run_id);
    assert_eq!(inspected["parent_run_id"], record["parent_run_id"]);
    assert!(inspected["outcome"].is_null());
    let listed = run(home.path(), &["session", "list", "--all", "--json"]);
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed[0]["run_id"], run_id);
    let reopened = run(home.path(), &["session", "open", id, "--json"]);
    assert!(reopened.status.success());
    let reopened: serde_json::Value = serde_json::from_slice(&reopened.stdout).unwrap();
    assert_eq!(reopened["run_id"], run_id);
    assert!(!dir.join("events.jsonl").exists());
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
        let sessions = home.path().join("human-sessions");
        std::fs::create_dir(&sessions).unwrap();
        let id = "ask_resume-proof";
        let record = serde_json::json!({
            "id": id,
            "parent_run_id": "run_00000000000000000000000000000002",
            "parent_run_dir": home.path().join("parent"),
            "work": null, "work_selector": null,
            "title": "Resume", "detail": "proof", "prompt": "Local proof",
            "cwd": home.path(), "model": "opencode",
            "session_run_id": null, "ready_summary": null, "status": "waiting"
        });
        std::fs::write(sessions.join(format!("{id}.json")), record.to_string()).unwrap();
        let prepared = run(home.path(), &["session", "open", id, "--json"]);
        assert!(prepared.status.success(), "{:?}", prepared);
        let prepared: serde_json::Value = serde_json::from_slice(&prepared.stdout).unwrap();
        let run_id = prepared["run_id"].as_str().unwrap();
        let dir = home.path().join("runs").join(&run_id[4..6]).join(run_id);
        // Old live Runs must survive the history reader's seven-day window.
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
        manifest["created_at"] = serde_json::json!("2020-01-01T00:00:00Z");
        std::fs::write(dir.join("manifest.json"), manifest.to_string()).unwrap();

        // The native history belongs to this isolated fixture. The executable below
        // stands in for the provider; both opens exercise the real CLI and locks.
        if resume {
            std::fs::write(
                dir.join("provider-session.json"),
                serde_json::json!({
                    "schema_version": 1, "provider_session_id": "ses_resume-proof",
                    "account_id": null
                })
                .to_string(),
            )
            .unwrap();
        }
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let provider = bin.join("opencode");
        std::fs::write(
        &provider,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s' \"$LF_RUN_ID\" > \"$LF_RESUME_PROOF\"\nprintf '%s\\n' 'message=created id=ses_resume-proof' >&2\nread -r input\n",
    )
    .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        let evidence = home.path().join("resumed");
        let mut first = command(home.path(), &["session", "open", id])
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
        let deadline = Instant::now() + Duration::from_secs(10);
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
        let mut second = command(home.path(), &["session", "open", id, "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while second.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let blocked = second.try_wait().unwrap().is_none();
        if blocked {
            second.kill().unwrap();
        }
        let reopened = second.wait_with_output().unwrap();
        let active = run(home.path(), &["runs", "--active", "--json"]);
        // Release the owned provider before asserting, including on the failure path.
        first.stdin.take().unwrap().write_all(b"done\n").unwrap();
        let first = first.wait_with_output().unwrap();
        assert!(first.status.success(), "{:?}", first);
        assert!(
            !blocked,
            "Session metadata open waited for the resumed provider to exit"
        );
        assert!(reopened.status.success(), "{:?}", reopened);
        let reopened: serde_json::Value = serde_json::from_slice(&reopened.stdout).unwrap();
        assert_eq!(reopened["run_id"], run_id);
        assert_eq!(std::fs::read_to_string(evidence).unwrap(), run_id);
        assert_eq!(reopened["state"], "active");
        assert!(active.status.success(), "{:?}", active);
        let active: serde_json::Value = serde_json::from_slice(&active.stdout).unwrap();
        assert_eq!(active["gaps"], serde_json::json!([]), "{active}");
        assert_eq!(active["runs"].as_array().unwrap().len(), 1, "{active}");
        assert_eq!(active["runs"][0]["id"], run_id);
        assert_eq!(active["runs"][0]["processes"][0]["state"], "waiting");
        let ended = run(home.path(), &["runs", "--active", "--json"]);
        let ended: serde_json::Value = serde_json::from_slice(&ended.stdout).unwrap();
        assert_eq!(ended["runs"], serde_json::json!([]));
        if !resume {
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
            assert_eq!(manifest["run_id"], run_id);
            assert!(manifest["context"].is_object());
            assert!(!dir.join("prepared").exists());
        }
    }
}

fn prepare_ask(home: &std::path::Path, id: &str, title: &str) -> (String, std::path::PathBuf) {
    let sessions = home.join("human-sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let record = serde_json::json!({
        "id": id,
        "parent_run_id": "run_00000000000000000000000000000002",
        "parent_run_dir": home.join("parent"),
        "work": null, "work_selector": null,
        "title": title, "detail": "proof", "prompt": "Do not launch",
        "cwd": env!("CARGO_MANIFEST_DIR"), "model": "codex",
        "session_run_id": null, "ready_summary": null, "status": "waiting"
    });
    std::fs::write(sessions.join(format!("{id}.json")), record.to_string()).unwrap();
    let opened = run(home, &["session", "open", id, "--json"]);
    assert!(opened.status.success(), "{opened:?}");
    let opened: serde_json::Value = serde_json::from_slice(&opened.stdout).unwrap();
    let run_id = opened["run_id"].as_str().unwrap().to_string();
    let dir = home.join("runs").join(&run_id[4..6]).join(&run_id);
    (run_id, dir)
}

fn listed(home: &std::path::Path, id: &str) -> serde_json::Value {
    let output = run(home, &["session", "list", "--all", "--json"]);
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
    let id = "ask_naming-proof";
    let (run_id, dir) = prepare_ask(home.path(), id, "Which release target?");
    let seeded = listed(home.path(), id);
    assert_eq!(seeded["title"], "Which release target?");
    assert_eq!(seeded["title_source"], "generated");

    let suggested = rename(home.path(), &[id, "Release", "target", "--suggest"]);
    assert_eq!(suggested["title"], "Release target");
    assert_eq!(suggested["title_source"], "generated");
    let named = rename(home.path(), &[id, "Launch notes"]);
    assert_eq!(named["title"], "Launch notes");
    assert_eq!(named["title_source"], "human");
    assert_eq!(named["run_id"], run_id.as_str());

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
    let reopened = run(home.path(), &["session", "open", id, "--json"]);
    let reopened: serde_json::Value = serde_json::from_slice(&reopened.stdout).unwrap();
    assert_eq!(reopened["title"], "Launch notes");
    // Naming touches only the Run's name record, never provider state.
    assert!(!dir.join("provider-clients").exists());
    assert!(!dir.join("events.jsonl").exists());
    assert!(dir.join("prepared").exists());
}

#[test]
fn raw_sessions_are_named_by_a_stable_word_pair() {
    let home = tempfile::tempdir().unwrap();
    // A prepared Run with no skill, detached from its Ask and given native
    // history, lists as a raw interactive Session.
    let (run_id, dir) = prepare_ask(home.path(), "ask_raw-proof", "Unused seed");
    std::fs::remove_file(home.path().join("human-sessions/ask_raw-proof.json")).unwrap();
    std::fs::write(
        dir.join("provider-session.json"),
        serde_json::json!({
            "schema_version": 1, "provider_session_id": "ses_raw-proof", "account_id": null
        })
        .to_string(),
    )
    .unwrap();
    let first = listed(home.path(), &run_id);
    assert_eq!(first["kind"], "interactive");
    assert_eq!(first["title_source"], "generated");
    let title = first["title"].as_str().unwrap();
    let (magical, musical) = title.split_once('-').expect("magical-musical pair");
    assert!(!magical.is_empty() && !musical.is_empty() && !musical.contains('-'));
    assert_eq!(listed(home.path(), &run_id)["title"], title);
    assert!(!dir.join("session-name.json").exists());

    let named = rename(home.path(), &[&run_id, "Morning", "triage"]);
    assert_eq!(named["title"], "Morning triage");
    assert_eq!(listed(home.path(), &run_id)["title_source"], "human");
}

#[cfg(unix)]
#[test]
fn boundary_names_follow_run_ids_and_replacement_runs() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().unwrap();
    let sessions = home.path().join("human-sessions");
    std::fs::create_dir(&sessions).unwrap();
    let id = "ask_rename-replacement";
    let record = serde_json::json!({
        "id": id,
        "parent_run_id": "run_00000000000000000000000000000002",
        "parent_run_dir": home.path().join("parent"),
        "work": null, "work_selector": null,
        "title": "Which release target?", "detail": "proof", "prompt": "Local proof",
        "cwd": home.path(), "model": "opencode",
        "session_run_id": null, "ready_summary": null, "status": "waiting"
    });
    std::fs::write(sessions.join(format!("{id}.json")), record.to_string()).unwrap();
    let prepared = run(home.path(), &["session", "open", id, "--json"]);
    assert!(prepared.status.success(), "{prepared:?}");
    let prepared: serde_json::Value = serde_json::from_slice(&prepared.stdout).unwrap();
    let first_run = prepared["run_id"].as_str().unwrap().to_string();
    let first_dir = home
        .path()
        .join("runs")
        .join(&first_run[4..6])
        .join(&first_run);

    // The operating instruction passes the Session's own `$LF_RUN_ID`, which
    // names the boundary's Run rather than the boundary. It must reach the
    // boundary even before provider history exists.
    let suggested = rename(home.path(), &[&first_run, "Release target", "--suggest"]);
    assert_eq!(suggested["id"], id);
    assert_eq!(suggested["kind"], "ask");
    assert_eq!(suggested["title"], "Release target");
    let named = rename(home.path(), &[id, "Launch notes"]);
    assert_eq!(named["title_source"], "human");

    // A consumed launch that never produced provider history is replaced by a
    // new Run on the next open. The human name belongs to the Session.
    std::fs::remove_file(first_dir.join("prepared")).unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let provider = bin.join("opencode");
    std::fs::write(
        &provider,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s' \"$LF_RUN_ID\" > \"$LF_RESUME_PROOF\"\nprintf '%s\\n' 'message=created id=ses_rename-proof' >&2\nread -r input\n",
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let evidence = home.path().join("launched");
    let mut opened = command(home.path(), &["session", "open", id])
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
    let deadline = Instant::now() + Duration::from_secs(10);
    while !evidence.exists() && Instant::now() < deadline && opened.try_wait().unwrap().is_none() {
        std::thread::sleep(Duration::from_millis(20));
    }
    let started = evidence.exists();
    // Leave the provider waiting while the agent-facing rename runs inside it.
    let replacement = std::fs::read_to_string(&evidence).unwrap_or_default();
    let inside = command(
        home.path(),
        &[
            "session",
            "rename",
            &replacement,
            "Better guess",
            "--suggest",
            "--json",
        ],
    )
    .env("LF_RUN_ID", &replacement)
    .output()
    .unwrap();
    let _ = opened
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(b"done\n"));
    let opened = opened.wait_with_output().unwrap();
    assert!(started, "fixture provider did not start: {opened:?}");
    assert!(opened.status.success(), "{opened:?}");
    assert_ne!(replacement, first_run);

    assert!(inside.status.success(), "{inside:?}");
    let inside: serde_json::Value = serde_json::from_slice(&inside.stdout).unwrap();
    assert_eq!(inside["id"], id);
    assert_eq!(inside["kind"], "ask");
    assert_eq!(inside["run_id"], replacement.as_str());
    assert_eq!(inside["title"], "Launch notes");
    assert_eq!(inside["title_source"], "human");

    let readback = listed(home.path(), id);
    assert_eq!(readback["run_id"], replacement.as_str());
    assert_eq!(readback["title"], "Launch notes");
    assert_eq!(readback["title_source"], "human");
    // The boundary's Run never appears as a second, interactive Session.
    let all: Vec<serde_json::Value> =
        serde_json::from_slice(&run(home.path(), &["session", "list", "--all", "--json"]).stdout)
            .unwrap();
    assert_eq!(all.len(), 1, "{all:?}");
}
