use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn bare_lf_launches_a_conversational_prompt_with_a_model_option() {
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let claude = bin.join("claude");
    fs::write(
        &claude,
        r#"#!/bin/sh
printf '%s\n' "$@"
"#,
    )
    .unwrap();
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).unwrap();

    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for args in [
        &["init", "--quiet"][..],
        &["commit", "--quiet", "--allow-empty", "-m", "start"][..],
    ] {
        let status = Command::new("git")
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap();
        assert!(status.success());
    }

    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["-m", "claude"])
        // Outside the checkout: the launch prompt carries the working directory's
        // scratch notes, and a branch with large notes exceeds Linux's argument limit.
        .current_dir(&repo)
        .env("LF_HOME", temp.path().join("home"))
        .env("PATH", path)
        .output()
        .unwrap();

    let prompt = String::from_utf8_lossy(&output.stdout);
    assert!(prompt.contains("<lf:skill:default>"), "{prompt}");
    assert!(!prompt.contains("<lf:skill:repo/operate>"), "{prompt}");
    assert!(!prompt.contains("lf session list --json"), "{prompt}");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
