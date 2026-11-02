//! `lf` children that see only a test's home.

use std::path::Path;

/// `lf <args>` in `repo`, with only `home`'s environment and piped stdio.
pub fn isolated_lf(
    repo: &Path,
    home: &Path,
    args: &[impl AsRef<std::ffi::OsStr>],
) -> std::process::Child {
    std::process::Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .current_dir(repo)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
}

/// What `lf <args>` prints. `doctor` prints its report before exiting 1 on a
/// failed check, so only empty output is a failed read.
pub fn isolated_read(repo: &Path, home: &Path, args: &[&str], input: Option<&str>) -> String {
    let mut child = isolated_lf(repo, home, args);
    if let Some(input) = input {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success() || (args == ["doctor", "--json"] && !output.stdout.is_empty()),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
