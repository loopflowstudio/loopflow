mod support;

use std::process::Command;

use loopflow::ops::{commit_workflow, CommitOptions, NullProgress, OpsError};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn last_commit_message(repo: &TestRepo) -> String {
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .current_dir(repo.path())
        .output()
        .expect("git log");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn commit_options(message: &str) -> CommitOptions {
    CommitOptions {
        add: true,
        message: Some(message.to_string()),
        ..CommitOptions::for_task("commit")
    }
}

#[test]
fn commit_stages_and_commits() {
    let repo = TestRepo::new();
    repo.create_file("notes.txt", "hello");

    let options = commit_options("test commit");

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    assert_eq!(last_commit_message(&repo), "test commit");
}

#[test]
fn commit_with_push() {
    let repo = TestRepo::new();
    repo.create_file("push.txt", "hello");

    let options = CommitOptions {
        push: true,
        ..commit_options("push commit")
    };

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    assert_eq!(repo.head_sha(), repo.bare_head_sha());
}

#[test]
fn commit_skips_empty() {
    let repo = TestRepo::new();
    let before = repo.head_sha();

    let options = commit_options("skip");

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(!committed);
    assert_eq!(before, repo.head_sha());
}

#[test]
fn commit_with_lint_failure() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 0\n")]);
    let repo = TestRepo::new();
    repo.create_file(".lf/config.yaml", "lint: 'false'\nagent_model: claude\n");
    repo.create_file("bad.py", "print('hi')\n");

    let options = CommitOptions {
        lint: true,
        ..commit_options("lint")
    };

    let result = commit_workflow(repo.path(), &options, &NullProgress);
    assert!(matches!(result, Err(OpsError::LintFailed)));
}

#[test]
fn commit_with_message_override() {
    let repo = TestRepo::new();
    repo.create_file("override.txt", "hello");

    let options = commit_options("override message");

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    assert_eq!(last_commit_message(&repo), "override message");
}
