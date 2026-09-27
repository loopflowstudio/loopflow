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
    let (id, ..) = prepare_ask(
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
fn asked_session_has_a_run_before_any_provider_is_started() {
    let home = tempfile::tempdir().unwrap();
    let (id, run_id, dir) = prepare_ask(
        home.path(),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        "codex",
        "Do not launch",
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["run_id"], run_id.as_str());
    assert_eq!(manifest["parent_run_id"], CALLER);
    let opened = run(home.path(), &["session", "open", &id, "--json"]);
    assert!(
        opened.status.success(),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let opened: serde_json::Value = serde_json::from_slice(&opened.stdout).unwrap();
    assert_eq!(opened["run_id"], run_id.as_str());
    assert_eq!(opened["state"], "waiting");
    assert!(dir.join("prepared").exists());
    assert!(!dir.join("provider-clients").exists());
    assert!(!dir.join("terminal.json").exists());
    let inspected = run(home.path(), &["runs", &run_id, "--json"]);
    assert!(
        inspected.status.success(),
        "{}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspected: serde_json::Value = serde_json::from_slice(&inspected.stdout).unwrap();
    assert_eq!(inspected["id"], run_id.as_str());
    assert_eq!(inspected["parent_run_id"], CALLER);
    assert!(inspected["outcome"].is_null());
    let listed = run(home.path(), &["session", "list", "--all", "--json"]);
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed[0]["run_id"], run_id.as_str());
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
        let (id, run_id, dir) = prepare_ask(home.path(), home.path(), "opencode", "Local proof");
        let (id, run_id) = (id.as_str(), run_id.as_str());
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
        let mut second = command(home.path(), &["session", "open", id, "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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

const CALLER: &str = "run_00000000000000000000000000000002";

/// Ask through the real CLI from a stand-in caller Run, then stop the caller.
/// The Session keeps waiting: (Session id, Run id, Run directory).
fn prepare_ask(
    home: &std::path::Path,
    cwd: &std::path::Path,
    harness: &str,
    question: &str,
) -> (String, String, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let caller = home.join("caller");
    std::fs::create_dir_all(&caller).unwrap();
    let manifest = serde_json::json!({
        "schema_version": 1, "run_id": CALLER, "parent_run_id": null,
        "created_at": "2026-01-01T00:00:00Z", "harness": harness, "model": null,
        "surface": "headless", "cwd": cwd, "repo": null, "worktree": null,
        "skill": "proof", "subjects": [], "flow": null, "launch": null, "context": null,
        "runtime_path": null, "runtime_digest": null, "host": "test", "boot_id": null
    });
    std::fs::write(caller.join("manifest.json"), manifest.to_string()).unwrap();
    // The background launcher only acknowledges; each test opens the Session itself.
    let launcher = home.join("launcher");
    std::fs::create_dir_all(&launcher).unwrap();
    let tmux = launcher.join("tmux");
    std::fs::write(
        &tmux,
        "#!/bin/sh\nif [ \"$1\" = has-session ]; then exit 1; fi\n",
    )
    .unwrap();
    std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut asking = command(home, &["ask", question])
        .current_dir(cwd)
        .env(
            "PATH",
            format!(
                "{}:{}",
                launcher.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("LF_RUN_ID", CALLER)
        .env("LF_RUN_DIR", &caller)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(180);
    let stored = loop {
        let stored = rusqlite::Connection::open_with_flags(
            home.join("loopflow.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .and_then(|db| {
            db.query_row(
                "SELECT id, current_run_id FROM sessions WHERE kind='ask' AND request=?1",
                [question],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
        });
        if let Ok(stored) = stored {
            break stored;
        }
        if asking.try_wait().unwrap().is_some() || Instant::now() >= deadline {
            let _ = asking.kill();
            panic!("lf ask stored no Session: {:?}", asking.wait_with_output());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    asking.kill().unwrap();
    asking.wait().unwrap();
    let dir = home.join("runs").join(&stored.1[4..6]).join(&stored.1);
    (stored.0, stored.1, dir)
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
    let (id, run_id, dir) = prepare_ask(
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
    // Naming touches only the Session row, never the Run or provider state.
    assert!(!dir.join("session-name.json").exists());
    assert!(!dir.join("provider-clients").exists());
    assert!(!dir.join("events.jsonl").exists());
    assert!(dir.join("prepared").exists());
}

#[cfg(unix)]
#[test]
fn boundary_names_follow_run_ids_and_replacement_runs() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    let home = tempfile::tempdir().unwrap();
    let (id, first_run, first_dir) = prepare_ask(
        home.path(),
        home.path(),
        "opencode",
        "Which release target?",
    );
    let id = id.as_str();

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
    let ready = command(home.path(), &["session", "ready", "Ship to staging"])
        .env("LF_RUN_ID", &replacement)
        .env(
            "LF_HUMAN_SESSION",
            serde_json::json!({"kind": "ask", "id": id}).to_string(),
        )
        .output()
        .unwrap();
    assert!(ready.status.success(), "{ready:?}");
    let completed = run(home.path(), &["session", "complete", &replacement]);
    assert!(completed.status.success(), "{completed:?}");
    assert!(String::from_utf8_lossy(&completed.stdout).contains("Ship to staging"));
    let again = run(home.path(), &["session", "complete", id]);
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already complete"),
        "{again:?}"
    );
    assert!(!home
        .path()
        .join("human-sessions")
        .join(format!("{id}.json"))
        .exists());
}
