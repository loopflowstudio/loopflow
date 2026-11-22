//! Public credential boundary: real CLI with stand-in credential commands.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

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
