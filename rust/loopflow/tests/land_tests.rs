mod support;

use std::fs;
use std::process::{Command, Stdio};

use loopflow::ops::{land, LandOptions, NullProgress, OpsError};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn push_branch(repo: &TestRepo, name: &str) {
    let _ = Command::new("git")
        .args(["push", "-u", "origin", name])
        .current_dir(repo.path())
        .status();
}

fn local_branch_exists(repo: &TestRepo, name: &str) -> bool {
    Command::new("git")
        .args(["show-ref", "--verify", &format!("refs/heads/{name}")])
        .current_dir(repo.path())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn remote_branch_exists(repo: &TestRepo, name: &str) -> bool {
    Command::new("git")
        .arg("--git-dir")
        .arg(repo.bare_path())
        .args(["show-ref", "--verify", &format!("refs/heads/{name}")])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn gh_no_pr_script() -> &'static str {
    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi

if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi

exit 0
"#
}

fn gh_land_script(log_path: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  exit 0
fi
echo "$@" >> "{log_path}"

if [ "$1 $2" = "pr list" ]; then
  echo '[]'
  exit 0
fi

if [ "$1 $2" = "pr create" ]; then
  echo "https://example.com/pr/1"
  exit 0
fi

if [ "$1 $2" = "pr view" ]; then
  echo "https://example.com/pr/1"
  exit 0
fi

exit 0
"#
    )
}

#[test]
fn land_local_squash_merges_to_main() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
        },
        &NullProgress,
    )
    .expect("land");

    assert!(result.merged);
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo.path())
        .output()
        .expect("git rev-parse");
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, "main");
    assert!(!local_branch_exists(&repo, "feature"));
    assert!(repo.path().join("feature.txt").exists());
}

#[test]
fn land_preserves_main_on_failure() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("conflict.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    repo.checkout("main");
    repo.create_file("conflict.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    let main_head = repo.head_sha();

    repo.checkout("feature");
    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
        },
        &NullProgress,
    );

    assert!(result.is_err());
    let _ = Command::new("git")
        .args(["rebase", "--abort"])
        .current_dir(repo.path())
        .status();
    let _ = Command::new("git")
        .args(["reset", "--hard"])
        .current_dir(repo.path())
        .status();
    repo.checkout("main");
    assert_eq!(repo.head_sha(), main_head);
}

#[test]
fn land_cleans_up_remote_branch() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: true,
            create_pr: false,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
        },
        &NullProgress,
    )
    .expect("land");

    assert!(result.merged);
    assert!(!remote_branch_exists(&repo, "feature"));
}

#[test]
fn land_missing_pr_error_includes_branch_name() {
    let _env = EnvGuard::new(&[("gh", gh_no_pr_script())]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let result = land(
        repo.path(),
        &LandOptions {
            strict: true,
            local: false,
            create_pr: false,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
        },
        &NullProgress,
    );

    let Err(OpsError::Message(message)) = result else {
        panic!("expected missing PR message");
    };
    assert!(message.contains("no open PR found for branch 'feature'"));
}

#[test]
fn land_uses_cached_pr_copy_when_available() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(scratch.join("pr-title.txt"), "cached title").expect("write title");
    fs::write(scratch.join("pr-body.md"), "cached body").expect("write body");
    fs::write(scratch.join(".pr-copy-ref"), repo.head_sha()).expect("write ref");

    let log_path = repo.path().join("gh.log");
    let script = gh_land_script(log_path.to_string_lossy().as_ref());
    let _env = EnvGuard::new(&[("gh", script.as_str())]);

    let result = land(
        repo.path(),
        &LandOptions {
            strict: false,
            local: false,
            create_pr: true,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
        },
        &NullProgress,
    )
    .expect("land with cached copy");

    assert!(result.merged);
    let log = fs::read_to_string(log_path).expect("read gh log");
    assert!(log.contains("--title cached title"));
    assert!(log.contains("--body cached body"));
}

#[test]
fn land_requires_title_when_cached_pr_copy_is_stale() {
    let _env = EnvGuard::new(&[("gh", gh_no_pr_script())]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");
    push_branch(&repo, "feature");

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).expect("create scratch");
    fs::write(scratch.join("pr-title.txt"), "stale title").expect("write title");
    fs::write(scratch.join("pr-body.md"), "stale body").expect("write body");
    fs::write(
        scratch.join(".pr-copy-ref"),
        "0000000000000000000000000000000000000000",
    )
    .expect("write stale ref");

    let result = land(
        repo.path(),
        &LandOptions {
            strict: false,
            local: false,
            create_pr: true,
            worktree: None,
            commit_message: None,
            pr_title: None,
            pr_body: None,
        },
        &NullProgress,
    );

    let Err(OpsError::Message(message)) = result else {
        panic!("expected stale copy error");
    };
    assert!(message.contains("no PR title provided"));
    assert!(message.contains("lf gate"));
}
