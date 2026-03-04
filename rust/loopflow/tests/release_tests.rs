mod support;

use std::fs;
use std::process::Command;

use loopflow::ops::{
    bump_version, generate_release, release_bump, release_check, release_status, release_tag,
    NullProgress,
};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn write_gh_script(pr_list: &str) -> String {
    format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh version 2.0.0'\n  exit 0\nfi\ncase \"$1 $2\" in\n  'pr list')\n    cat <<'JSON'\n{pr_list}\nJSON\n    exit 0;;\nesac\necho \"unexpected gh invocation: $@\" >&2\nexit 1\n"
    )
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

fn git_output(repo: &TestRepo, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn git_output_bare(repo: &TestRepo, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git --git-dir {:?} {:?} failed: {}",
        repo.bare_path(),
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn release_generates_notes_and_writes_file() {
    let gh_script = write_gh_script(
        r#"[{"number":101,"title":"Add release command","body":"Users can now run lf release."}]"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.8.0"]);

    let version =
        generate_release(repo.path(), "v0.9.1", &NullProgress).expect("release should succeed");

    assert_eq!(version, "0.9.1");

    let notes = fs::read_to_string(repo.path().join("RELEASE_NOTES.md")).expect("read notes");
    assert!(notes.starts_with("# v0.9.1\n\n"));
    assert!(notes.contains("## Merged PRs"));
    assert!(notes.contains("- #101 Add release command"));
}

#[test]
fn release_keeps_existing_header() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

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
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

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
fn release_check_returns_merged_prs() {
    let gh_script = write_gh_script(
        r#"[{"number":42,"title":"Add feature","body":"New stuff","additions":100,"deletions":10,"changedFiles":3}]"#,
    );
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    git(&repo, &["tag", "v0.9.0"]);

    let prs = release_check(repo.path(), None).expect("check should succeed");
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 42);
    assert_eq!(prs[0].title, "Add feature");
}

#[test]
fn release_check_returns_empty_without_tag() {
    let gh_script = write_gh_script("[]");
    let _env = EnvGuard::new(&[("gh", gh_script.as_str())]);

    let repo = TestRepo::new();
    // No tags — release_check should return empty vec (no tag to compare against)
    let prs = release_check(repo.path(), None).expect("check should succeed");
    assert!(prs.is_empty());
}

#[test]
fn release_bump_updates_cargo_toml() {
    let repo = TestRepo::new();
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.9.0\"\n",
    )
    .unwrap();

    release_bump(repo.path(), "0.9.1", None, &NullProgress).expect("bump should succeed");

    let content = fs::read_to_string(repo.path().join("Cargo.toml")).unwrap();
    assert!(content.contains("version = \"0.9.1\""));
    assert!(!content.contains("version = \"0.9.0\""));
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

#[test]
fn release_tag_is_idempotent_when_remote_tag_matches_head() {
    let repo = TestRepo::new();
    let head = git_output(&repo, &["rev-parse", "HEAD"]);

    let tag = release_tag(repo.path(), "0.9.1", None).expect("first tag should succeed");
    assert_eq!(tag, "v0.9.1");

    let second = release_tag(repo.path(), "0.9.1", None).expect("second tag should be no-op");
    assert_eq!(second, "v0.9.1");

    let remote_tag = git_output_bare(&repo, &["rev-parse", "refs/tags/v0.9.1"]);
    assert_eq!(remote_tag, head);
}

#[test]
fn release_tag_fails_if_remote_tag_points_to_different_commit() {
    let repo = TestRepo::new();
    release_tag(repo.path(), "0.9.1", None).expect("initial tag should succeed");

    repo.create_file("CHANGELOG.md", "new release prep");
    repo.stage_all();
    repo.commit("change after tag");

    let err = release_tag(repo.path(), "0.9.1", None).expect_err("mismatched tag should fail");
    let message = err.to_string();
    assert!(message.contains("already exists on origin"));
    assert!(message.contains("expected"));
}
