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
