mod support;

use std::process::Command;

use loopflow_ops::{commit_workflow, CommitOptions, NullProgress, OpsError};
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

#[test]
fn commit_stages_and_commits() {
    let repo = TestRepo::new();
    repo.create_file("notes.txt", "hello");

    let options = CommitOptions {
        add: true,
        lint: false,
        push: false,
        create_draft_pr: false,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: Some("test commit".to_string()),
    };

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    assert_eq!(last_commit_message(&repo), "test commit");
}

#[test]
fn commit_with_push() {
    let repo = TestRepo::new();
    repo.create_file("push.txt", "hello");

    let options = CommitOptions {
        add: true,
        lint: false,
        push: true,
        create_draft_pr: false,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: Some("push commit".to_string()),
    };

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    assert_eq!(repo.head_sha(), repo.bare_head_sha());
}

#[test]
fn commit_skips_empty() {
    let repo = TestRepo::new();
    let before = repo.head_sha();

    let options = CommitOptions {
        add: true,
        lint: false,
        push: false,
        create_draft_pr: false,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: Some("skip".to_string()),
    };

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(!committed);
    assert_eq!(before, repo.head_sha());
}

#[test]
fn commit_with_lint_failure() {
    let _env = EnvGuard::new(&[("claude", "#!/bin/sh\nexit 0\n")]);
    let repo = TestRepo::new();
    repo.create_file(
        ".lf/config.yaml",
        "lint_check: 'false'\nagent_model: claude\n",
    );
    repo.create_file("bad.py", "print('hi')\n");

    let options = CommitOptions {
        add: true,
        lint: true,
        push: false,
        create_draft_pr: false,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: Some("lint".to_string()),
    };

    let result = commit_workflow(repo.path(), &options, &NullProgress);
    assert!(matches!(result, Err(OpsError::LintFailed)));
}

#[test]
fn commit_with_message_override() {
    let repo = TestRepo::new();
    repo.create_file("override.txt", "hello");

    let options = CommitOptions {
        add: true,
        lint: false,
        push: false,
        create_draft_pr: false,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: Some("override message".to_string()),
    };

    let committed = commit_workflow(repo.path(), &options, &NullProgress).expect("commit");
    assert!(committed);
    assert_eq!(last_commit_message(&repo), "override message");
}
