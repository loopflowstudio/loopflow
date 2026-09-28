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
    // The provider is the harness recorded on this Run's own manifest.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    assert!(manifest["harness"].is_string());
    assert_eq!(first["provider"], manifest["harness"]);
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
        "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s' \"$LF_RUN_ID\" > \"$LF_RESUME_PROOF.tmp\"\nmv \"$LF_RESUME_PROOF.tmp\" \"$LF_RESUME_PROOF\"\nprintf '%s\\n' 'message=created id=ses_rename-proof' >&2\nread -r input\n",
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

    // Completing by that Run ID acts on the Ask boundary, never on the Run's
    // provider history as if it were an interactive Session.
    let early = run(home.path(), &["session", "complete", &replacement]);
    assert!(!early.status.success(), "{early:?}");
    assert!(
        String::from_utf8_lossy(&early.stderr).contains("not marked this ready"),
        "{early:?}"
    );
    assert_eq!(
        listed(home.path(), id)["ready_summary"],
        serde_json::Value::Null
    );
    let path = sessions.join(format!("{id}.json"));
    let mut saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    saved["ready_summary"] = serde_json::json!("Ship to staging");
    std::fs::write(&path, saved.to_string()).unwrap();
    let completed = run(home.path(), &["session", "complete", &replacement]);
    assert!(completed.status.success(), "{completed:?}");
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        saved["status"],
        serde_json::json!({"completed": {"summary": "Ship to staging"}})
    );
}

#[cfg(unix)]
#[test]
fn claude_missing_history_reopens_archived_seed_then_resumes() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    // Simulation of Claude persistence, not a native loader acceptance test.
    for exit_code in [0, 17] {
        let home = tempfile::tempdir().unwrap();
        let root = home.path();
        let sessions = root.join("human-sessions");
        let bin = root.join("bin");
        let history = root.join("claude-home");
        for dir in [&sessions, &bin, &history] {
            std::fs::create_dir(dir).unwrap();
        }
        let id = "ask_claude-recovery";
        let record_path = sessions.join(format!("{id}.json"));
        let mut record = serde_json::json!({
            "id": id, "parent_run_id": "run_00000000000000000000000000000002",
            "parent_run_dir": root.join("parent"), "work": null, "work_selector": null,
            "title": "Review recovery", "detail": "proof", "prompt": "ORIGINAL_TASK_MARKER",
            "cwd": root, "model": "claude:haiku", "session_run_id": null,
            "ready_summary": "Keep this readiness", "status": "waiting"
        });
        std::fs::write(&record_path, record.to_string()).unwrap();
        let provider = bin.join("claude");
        std::fs::write(&provider, r#"#!/bin/sh
if [ "$1" = --version ]; then exit 0; fi
printf '%s\n' "$@" > "$LF_PROOF/args"
printf '%s' "$CLAUDE_CONFIG_DIR" > "$LF_PROOF/root"
printf '%s' "$LF_RUN_ID" > "$LF_PROOF/current-run"
if [ -f "$LF_PROOF/fail" ]; then exit "$LF_EXIT_CODE"; fi
if [ -f "$LF_PROOF/reject" ]; then exit 19; fi
previous=
for argument in "$@"; do
  if [ "$previous" = --session-id ]; then session="$argument"; fi
  if [ "$previous" = --resume ]; then printf '%s' "$argument" > "$LF_PROOF/resumed"; fi
  previous="$argument"
done
if [ -n "$session" ]; then
  mkdir -p "$CLAUDE_CONFIG_DIR/projects/another-project"
  printf '{"type":"user","sessionId":"%s","uuid":"11111111-1111-4111-8111-111111111111","timestamp":"2026-09-28T12:00:00Z","message":{"content":"saved seed"}}\n' "$session" > "$CLAUDE_CONFIG_DIR/projects/another-project/$session.jsonl"
fi
touch "$LF_PROOF/started"
read -r input
"#).unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
        // An accidental reroute fails locally instead of reaching an installed provider.
        std::fs::write(bin.join("opencode"), "#!/bin/sh\nexit 91\n").unwrap();
        std::fs::set_permissions(bin.join("opencode"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        let make_command = || {
            let mut cmd = command(root, &["session", "open", id]);
            cmd.current_dir(root)
                .env(
                    "PATH",
                    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
                )
                .env("CLAUDE_CONFIG_DIR", &history)
                .env("LF_PROOF", root)
                .env("LF_EXIT_CODE", exit_code.to_string())
                .env_remove("LF_ACCOUNT_LEASE")
                .env_remove("LF_ACCOUNT_SELECTION")
                .env_remove("LF_HUMAN_SESSION")
                .env_remove("LF_FLOW_STEP");
            cmd
        };
        std::fs::write(root.join("fail"), "").unwrap();
        let _first = make_command().output().unwrap();
        let original = std::fs::read_to_string(root.join("current-run")).unwrap();
        let original_dir = root.join("runs").join(&original[4..6]).join(&original);
        let events = std::fs::read_to_string(original_dir.join("events.jsonl")).unwrap();
        assert!(!events.contains("provider_session_observed"));
        let terminal: serde_json::Value =
            serde_json::from_slice(&std::fs::read(original_dir.join("terminal.json")).unwrap())
                .unwrap();
        assert_eq!(
            terminal["outcome"],
            if exit_code == 0 {
                "completed"
            } else {
                "failed"
            }
        );
        let reference: serde_json::Value = serde_json::from_slice(
            &std::fs::read(original_dir.join("provider-session.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(reference["history_root"], history.to_str().unwrap());
        if exit_code != 0 {
            let project = history.join("projects/old-project");
            std::fs::create_dir_all(&project).unwrap();
            std::fs::write(
                project.join(format!(
                    "{}.jsonl",
                    reference["provider_session_id"].as_str().unwrap()
                )),
                "",
            )
            .unwrap();
        }
        // Exercise old split-channel captures as well as a changed present-day seed.
        let context_path = original_dir.join("context.json");
        let saved_context = std::fs::read(&context_path).unwrap();
        let old_binding = std::fs::read(&record_path).unwrap();
        std::fs::write(&context_path, "corrupt").unwrap();
        let refused = make_command().output().unwrap();
        assert!(!refused.status.success());
        assert_eq!(std::fs::read(&record_path).unwrap(), old_binding);
        std::fs::write(&context_path, saved_context).unwrap();
        let mut context: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&context_path).unwrap()).unwrap();
        context["context"]["system"] = serde_json::json!({"text":"ARCHIVED_SYSTEM_MARKER"});
        let bytes = serde_json::to_vec(&context).unwrap();
        std::fs::write(&context_path, &bytes).unwrap();
        let manifest_path = original_dir.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        manifest["context"]["content_sha256"] = hex::encode(Sha256::digest(&bytes)).into();
        manifest["context"]["bytes"] = bytes.len().into();
        std::fs::write(&manifest_path, manifest.to_string()).unwrap();
        record = serde_json::from_slice(&std::fs::read(&record_path).unwrap()).unwrap();
        record["model"] = "opencode".into();
        record["prompt"] = "CHANGED_TASK_MARKER".into();
        std::fs::write(&record_path, record.to_string()).unwrap();
        rename(root, &[id, "Preserved review name"]);
        std::fs::remove_file(root.join("fail")).unwrap();
        let before = record.clone();
        let mut replacement = None;
        for resume in [false, true] {
            let mut child = make_command()
                .env("CLAUDE_CONFIG_DIR", root.join("changed-default"))
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(20);
            while !root.join("started").exists()
                && Instant::now() < deadline
                && child.try_wait().unwrap().is_none()
            {
                std::thread::sleep(Duration::from_millis(10));
            }
            let started = root.join("started").exists();
            if !started {
                let _ = child.kill();
            }
            // The boundary lock is released at client registration, before the
            // interactive lifetime. Use that publication barrier before exit.
            if started {
                let name = hex::encode(&Sha256::digest(id.as_bytes())[..16]);
                let lock = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(sessions.join(format!(".{name}.launch.lock")))
                    .unwrap();
                fs2::FileExt::lock_exclusive(&lock).unwrap();
                drop(lock);
            }
            let _ = child
                .stdin
                .take()
                .map(|mut stdin| stdin.write_all(b"done\n"));
            let output = child.wait_with_output().unwrap();
            assert!(started && output.status.success(), "{output:?}");
            let args = std::fs::read_to_string(root.join("args")).unwrap();
            assert_eq!(
                std::fs::read_to_string(root.join("root")).unwrap(),
                history.to_str().unwrap()
            );
            let after: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&record_path).unwrap()).unwrap();
            let next = after["session_run_id"].as_str().unwrap().to_string();
            if resume {
                assert_eq!(Some(&next), replacement.as_ref());
                assert!(args.contains("--resume"));
                assert!(!args.contains("ORIGINAL_TASK_MARKER"));
            } else {
                assert_ne!(next, original);
                assert!(
                    args.contains("ORIGINAL_TASK_MARKER")
                        && args.contains("ARCHIVED_SYSTEM_MARKER")
                );
                assert!(!args.contains("CHANGED_TASK_MARKER"));
                assert!(args.contains("haiku"));
                replacement = Some(next.clone());
            }
            let mut expected = before.clone();
            expected["session_run_id"] = next.into();
            assert_eq!(after, expected);
            assert_eq!(listed(root, id)["title"], "Preserved review name");
            let all: Vec<serde_json::Value> =
                serde_json::from_slice(&run(root, &["session", "list", "--all", "--json"]).stdout)
                    .unwrap();
            assert_eq!(all.len(), 1);
            std::fs::remove_file(root.join("started")).unwrap();
        }
        let next = replacement.unwrap();
        let next_dir = root.join("runs").join(&next[4..6]).join(&next);
        let next_ref: serde_json::Value =
            serde_json::from_slice(&std::fs::read(next_dir.join("provider-session.json")).unwrap())
                .unwrap();
        let transcript = history.join("projects/another-project").join(format!(
            "{}.jsonl",
            next_ref["provider_session_id"].as_str().unwrap()
        ));
        let saved_history = std::fs::read(&transcript).unwrap();
        let saved_binding = std::fs::read(&record_path).unwrap();
        // Existing history rejected by the provider never selects replacement.
        std::fs::write(root.join("reject"), "").unwrap();
        assert!(!make_command().output().unwrap().status.success());
        assert_eq!(std::fs::read(&record_path).unwrap(), saved_binding);
        assert_eq!(std::fs::read(&transcript).unwrap(), saved_history);
        std::fs::remove_file(root.join("reject")).unwrap();
        for contents in ["{", "{\"type\":\"file-history-snapshot\"}"] {
            std::fs::write(&transcript, contents).unwrap();
            let metadata = run(root, &["session", "open", id, "--json"]);
            assert!(metadata.status.success());
            assert!(!make_command().output().unwrap().status.success());
            assert_eq!(std::fs::read(&record_path).unwrap(), saved_binding);
            assert_eq!(std::fs::read_to_string(&transcript).unwrap(), contents);
        }
        std::fs::write(&transcript, &saved_history).unwrap();
        std::fs::rename(&history, root.join("unavailable-home")).unwrap();
        assert!(!make_command().output().unwrap().status.success());
        assert_eq!(std::fs::read(&record_path).unwrap(), saved_binding);
        std::fs::rename(root.join("unavailable-home"), &history).unwrap();
        assert_eq!(std::fs::read(&context_path).unwrap(), bytes);
    }
}
