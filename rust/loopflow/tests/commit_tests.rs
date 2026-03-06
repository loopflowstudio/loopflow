use std::process::Command;

use loopflow::ops::{commit_workflow, CommitOptions, NullProgress};
use loopflow_test_support::TestRepo;

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
fn commit_with_message_override() {
    let repo = TestRepo::new();
    repo.create_file("override.txt", "hello");

    let options = commit_options("override message");

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    assert_eq!(last_commit_message(&repo), "override message");
}

#[test]
fn commit_generates_message_when_none() {
    let repo = TestRepo::new();
    repo.create_file("needs-message.txt", "hello");

    let options = CommitOptions {
        add: true,
        ..CommitOptions::for_task("implement")
    };

    // Without an agent available, generation fails and falls back to prefix-only.
    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    let message = last_commit_message(&repo);
    assert!(
        message.starts_with("lf implement"),
        "expected 'lf implement' prefix, got: {message}"
    );
}
