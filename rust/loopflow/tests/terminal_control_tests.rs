use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn terminal_control_accepts_a_model_option() {
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let claude = bin.join("claude");
    fs::write(&claude, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();

    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["-m", "claude"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("LF_HOME", temp.path().join("home"))
        .env("PATH", path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
