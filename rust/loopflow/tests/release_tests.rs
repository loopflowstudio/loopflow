mod support;

use std::fs;
use std::process::Command;

use loopflow::ops::{bump_version, generate_release, release_status, NullProgress};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn write_gh_script(pr_list: &str) -> String {
    format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list}\nJSON\n    exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
    )
}

fn write_claude_script(output: &str) -> String {
    format!("#!/bin/sh\ncat <<'EOF'\n{output}\nEOF\n")
}

fn write_gh_status_script(run_list: &str, release_view: &str) -> String {
    format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'run list')\n    cat <<'JSON'\n{run_list}\nJSON\n    exit 0;;\n  'release view')\n    cat <<'JSON'\n{release_view}\nJSON\n    exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
    )
}

fn git(repo: &TestRepo, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo.path())
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn release_generates_notes_and_writes_file() {
    let gh_script = write_gh_script(
        r#"[{"number":101,"title":"Add release command","body":"Users can now run lf release."}]"#,
    );
    let claude_script =
        write_claude_script("## Highlights\n\n- Added automated release notes generation.");
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.8.0"]);

    let version =
        generate_release(repo.path(), "v0.9.1", &NullProgress).expect("release should succeed");

    assert_eq!(version, "0.9.1");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert!(notes.starts_with("# v0.9.1\n\n"));
    assert!(notes.contains("## Highlights"));
}

#[test]
fn release_keeps_existing_header() {
    let gh_script = write_gh_script("[]");
    let claude_script = write_claude_script("# v0.9.2\n\nAlready has header.");
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    generate_release(repo.path(), "0.9.2", &NullProgress).expect("release should succeed");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert_eq!(notes.lines().next().expect("first line"), "# v0.9.2");
    assert_eq!(notes.matches("# v0.9.2").count(), 1);
}

#[test]
fn release_with_bump_keyword() {
    let gh_script = write_gh_script("[]");
    let claude_script = write_claude_script("## Changes\n\n- Bug fixes.");
    let _env = EnvGuard::new(&[
        ("gh", gh_script.as_str()),
        ("claude", claude_script.as_str()),
    ]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    let version =
        generate_release(repo.path(), "patch", &NullProgress).expect("release should succeed");
    assert_eq!(version, "0.9.2");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert!(notes.starts_with("# v0.9.2\n\n"));
}

#[test]
fn bump_version_patch() {
    assert_eq!(bump_version("v1.2.3", "patch").unwrap(), "1.2.4");
}

#[test]
fn bump_version_minor() {
    assert_eq!(bump_version("1.2.3", "minor").unwrap(), "1.3.0");
}

#[test]
fn bump_version_major() {
    assert_eq!(bump_version("v0.9.1", "major").unwrap(), "1.0.0");
}

#[test]
fn bump_version_invalid_format() {
    assert!(bump_version("1.2", "patch").is_err());
}

#[test]
fn release_status_reports_latest_tag_and_release() {
    let gh_script = write_gh_status_script(
        r#"[{"databaseId":42,"headBranch":"v0.9.1","displayTitle":"Release v0.9.1","status":"completed","conclusion":"success","url":"https://example.com/run/42"}]"#,
        r#"{"tagName":"v0.9.1"}"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.1"]);

    let status = release_status(repo.path(), None).expect("status should succeed");
    assert_eq!(status.target, "default");
    assert_eq!(status.latest_tag.as_deref(), Some("v0.9.1"));
    assert_eq!(status.workflow_status.as_deref(), Some("completed"));
    assert_eq!(status.workflow_conclusion.as_deref(), Some("success"));
    assert!(status.release_exists);
}

#[test]
fn release_status_scopes_to_named_target() {
    let gh_script = write_gh_status_script(
        r#"[{"databaseId":7,"headBranch":"cli/v1.2.3","displayTitle":"Release cli/v1.2.3","status":"completed","conclusion":"success","url":"https://example.com/run/7"}]"#,
        r#"{"tagName":"cli/v1.2.3"}"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    fs::create_dir_all(repo.path().join(".lf")).expect("create config dir");
    fs::write(
        repo.path().join(".lf/config.yaml"),
        r#"
release:
  targets:
    cli:
      tag_prefix: "cli/"
"#,
    )
    .expect("write config");
    git(&repo, &["tag", "cli/v1.2.3"]);

    let status = release_status(repo.path(), Some("cli")).expect("status should succeed");
    assert_eq!(status.target, "cli");
    assert_eq!(status.latest_tag.as_deref(), Some("cli/v1.2.3"));
    assert!(status.release_exists);
}
