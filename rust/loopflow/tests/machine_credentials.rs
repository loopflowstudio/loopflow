//! Public receive boundary: real CLI and isolated native provider fixtures.
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use base64::prelude::*;
use serde_json::json;

fn command(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("LF_PROVIDER_TOKEN_KEY_PATH", home.join("key"))
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", home.join("bin").display()),
        )
        .current_dir(home);
    command
}

fn receive(home: &Path, payload: &[u8], machine: Option<&str>) -> Output {
    let mut command = command(home);
    command.args(["machine", "credentials", "receive"]);
    if let Some(machine) = machine {
        command.env("LF_EXPECTED_MACHINE_ID", machine);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // A rejected identity may close stdin before it consumes the payload.
    let _ = child.stdin.take().unwrap().write_all(payload);
    child.wait_with_output().unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn receive_registers_one_native_login_and_rejects_wrong_machine_without_exposing_bytes() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(home.path().join("bin")).unwrap();
    let codex = home.path().join("bin/codex");
    fs::write(&codex, "#!/bin/sh\nread -r initialize\necho '{\"id\":1,\"result\":{}}'\nread -r initialized\nread -r request\necho '{\"id\":2,\"result\":{\"account\":{\"email\":\"person@example.com\"}}}'\n").unwrap();
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o700)).unwrap();
    let identity = command(home.path())
        .args(["machine", "id"])
        .output()
        .unwrap();
    assert_success(&identity);
    let machine = String::from_utf8(identity.stdout).unwrap();
    let claims = BASE64_URL_SAFE_NO_PAD
        .encode(json!({"email":"person@example.com","sub":"person-123"}).to_string());
    let native = json!({"tokens":{"access_token":"fixture-private-access","refresh_token":"fixture-private-refresh","id_token":format!("header.{claims}.sig")}}).to_string();
    let payload = serde_json::to_vec(&json!({"provider":"codex","login":"person@example.com","subject":"person-123","credential":native})).unwrap();
    let rejected = receive(
        home.path(),
        &payload,
        Some("home_00000000000000000000000000000000"),
    );
    assert!(!rejected.status.success());
    assert!(!home.path().join("accounts/codex").exists());
    let received = receive(home.path(), &payload, Some(machine.trim()));
    assert_success(&received);
    let repeated = receive(home.path(), &payload, Some(machine.trim()));
    assert_success(&repeated);
    for output in [&rejected, &received, &repeated] {
        for bytes in [&output.stdout, &output.stderr] {
            assert!(!String::from_utf8_lossy(bytes).contains("fixture-private"));
        }
    }
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM provider_accounts WHERE provider='codex'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    let credential_home: String = db
        .query_row(
            "SELECT home FROM provider_accounts WHERE provider='codex'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let path = Path::new(&credential_home).join("auth.json");
    assert_eq!(fs::read_to_string(&path).unwrap(), native);
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn unadded_machine_is_rejected_before_any_local_credential_command() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir(home.path().join("bin")).unwrap();
    for executable in ["gh", "ssh", "codex", "claude", "security", "doppler"] {
        let path = home.path().join("bin").join(executable);
        fs::write(
            &path,
            "#!/bin/sh\ntouch \"$HOME/credential-command-ran\"\nexit 99\n",
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let output = command(home.path())
        .args(["machine", "connect", "unadded", "github"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!home.path().join("credential-command-ran").exists());
}
