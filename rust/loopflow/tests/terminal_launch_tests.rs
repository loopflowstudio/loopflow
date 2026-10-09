use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use loopflow_test_support::TestRepo;

#[test]
fn missing_codex_capture_retains_launch_without_adopting_unrelated_history() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let native = home.path().join("codex");
    let bin = home.path().join("bin");
    fs::create_dir(&native).unwrap();
    fs::create_dir(&bin).unwrap();
    let config = "model=\"fixture\"\n";
    fs::write(native.join("config.toml"), config).unwrap();
    fs::write(
        native.join("history.jsonl"),
        "{\"session_id\":\"unrelated-native-session\",\"ts\":9999999999}\n",
    )
    .unwrap();
    let provider = bin.join("codex");
    // The parser rejects the deliberately incomplete probe. The terminal exits
    // successfully without running its capture hook, as disabled hooks can do.
    fs::write(
        &provider,
        r#"#!/bin/sh
if [ "$1" = --dangerously-bypass-hook-trust ] && [ "$2" = --model ]; then
  echo "error: a value is required for '--model <MODEL>' but none was supplied" >&2
  exit 2
fi
exit 0
"#,
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["-i", "--diff", "none", "-a", "codex", ":", "hello"])
        .env_clear()
        .env("HOME", home.path())
        .env("CODEX_HOME", &native)
        .env("LF_HOME", home.path().join("lf"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .current_dir(repo.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("Codex exited without recording its native Session ID"),
        "{stderr}"
    );
    assert_eq!(
        fs::read_to_string(native.join("config.toml")).unwrap(),
        config
    );
    assert_eq!(fs::read_dir(&native).unwrap().count(), 2);
    let db = rusqlite::Connection::open(home.path().join("lf/loopflow.db")).unwrap();
    let captures: i64 = db.query_row(
        "SELECT count(*) FROM session_events WHERE json_extract(payload,'$.evidence.provider_session_id') IS NOT NULL",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(captures, 0);
    let sessions: i64 = db
        .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sessions, 1, "failed capture preserves its own Session");
}
