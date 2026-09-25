use std::process::{Command, Output};

fn run(home: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("LF_HOME", home)
        .env("RUST_LOG", "off")
        .output()
        .unwrap()
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
}
