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
    assert!(help.contains("approve"));
    assert!(help.contains("iterate"));
    assert!(!help.contains("accept"));
    assert!(!help.contains("decline"));
    assert!(!help.contains("send-back"));

    for args in [
        &["session", "ready"][..],
        &["session", "approve", "missing-session"],
        &["session", "iterate", "missing-session"],
    ] {
        let output = run(home.path(), args);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("required"));
    }

    for args in [
        &["session", "open", "missing-session", "--json"][..],
        &["session", "complete", "missing-session"],
        &["session", "approve", "missing-session", "Verified"],
        &[
            "session",
            "iterate",
            "missing-session",
            "Needs another pass",
        ],
    ] {
        let output = run(home.path(), args);
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            "Error: Session missing-session was not found"
        );
    }
}
