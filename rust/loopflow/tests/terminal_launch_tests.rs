use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use loopflow_test_support::TestRepo;

#[test]
fn oversized_interactive_prompt_reports_bytes_without_changing_delivery() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let provider = bin.join("claude");
    fs::write(
        &provider,
        r#"#!/bin/sh
for arg do
  if [ "${#arg}" -gt 122880 ]; then
    printf '%s' "$arg" > received-prompt
    echo 'cmux: argument too large (maximum 122880 bytes)' >&2
    exit 2
  fi
done
exit 0
"#,
    )
    .unwrap();
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
    let instructions = format!("private-fixture-marker\n{}", "x".repeat(122_881));
    fs::write(repo.path().join(".lf/skills/large.md"), &instructions).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args([
            "--tui",
            "--no-loopflow",
            "--diff",
            "none",
            "-m",
            "claude",
            "skill",
            "large",
        ])
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path().join("lf"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("NO_COLOR", "1")
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("cmux: argument too large"), "{error}");
    assert!(error.contains("122,880 bytes"), "{error}");
    assert!(error.contains("OS ARG_MAX:"), "{error}");
    assert!(error.contains("Prompt bytes by section:"), "{error}");
    assert!(error.contains("skill_instructions:"), "{error}");
    assert!(!error.contains("private-fixture-marker"), "{error}");
    let received = fs::read_to_string(repo.path().join("received-prompt")).unwrap();
    assert!(received.contains(&instructions));
    assert!(
        error.contains(&format!(
            "prompt argument: {} UTF-8 bytes",
            loopflow::lf::output::format_int(received.len() as u64)
        )),
        "{error}"
    );
}
