mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow::ops::{commit_selected, commit_workflow, CommitOptions, NullProgress};
use loopflow_test_support::TestRepo;
use support::EnvGuard;

fn git_output(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn last_commit_message(repo: &TestRepo) -> String {
    git_output(repo.path(), &["log", "-1", "--pretty=%B"])
        .trim()
        .to_string()
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

    let committed = commit_workflow(repo.path(), &options, &NullProgress, &|_| {}).expect("commit");
    assert!(committed);
    assert_eq!(last_commit_message(&repo), "test commit");
}

#[test]
fn commit_with_push() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("push.txt", "hello");

    let options = CommitOptions {
        push: true,
        ..commit_options("push commit")
    };

    let committed = commit_workflow(repo.path(), &options, &NullProgress, &|_| {}).expect("commit");
    assert!(committed);
    assert_eq!(repo.head_sha(), repo.bare_head_sha());
}

#[test]
fn commit_skips_empty() {
    let repo = TestRepo::new();
    let before = repo.head_sha();

    let options = commit_options("skip");

    let committed = commit_workflow(repo.path(), &options, &NullProgress, &|_| {}).expect("commit");
    assert!(!committed);
    assert_eq!(before, repo.head_sha());
}

#[test]
fn selected_commit_saves_untracked_scratch_without_unrelated_staged_work() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("other.md", "staged\n");
    repo.stage_all();
    repo.create_file("other.md", "later edit\n");
    repo.create_file("scratch/design.md", "design\n");
    repo.create_file("scratch/questions.md", "questions\n");

    commit_selected(
        repo.path(),
        &["scratch/design.md".into(), "scratch/questions.md".into()],
        Some("Save design"),
    )
    .unwrap();

    assert_eq!(
        git_output(repo.path(), &["show", "HEAD:scratch/design.md"]),
        "design\n"
    );
    assert_eq!(
        git_output(repo.path(), &["show", "HEAD:scratch/questions.md"]),
        "questions\n"
    );
    assert_eq!(
        git_output(repo.path(), &["diff", "--cached", "--name-only"]),
        "other.md\n"
    );
    assert_eq!(git_output(repo.path(), &["show", ":other.md"]), "staged\n");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("other.md")).unwrap(),
        "later edit\n"
    );
}

#[test]
fn selected_commit_with_no_changes_explains_what_to_do() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let before = repo.head_sha();

    let error = commit_selected(repo.path(), &["README.md".into()], Some("No changes"))
        .unwrap_err()
        .to_string();

    assert!(error.contains("Nothing selected to commit"), "{error}");
    assert!(error.contains("select changed paths"), "{error}");
    assert_eq!(repo.head_sha(), before);
}

#[test]
fn empty_git_commit_retains_stdout_diagnostic() {
    let repo = TestRepo::new();
    let before = repo.head_sha();

    let error = loopflow::git::commit(repo.path(), "No changes", &|command| {
        command.env("LC_ALL", "C");
    })
    .unwrap_err()
    .to_string();

    assert!(error.contains("nothing to commit"), "{error}");
    assert_eq!(repo.head_sha(), before);
}

#[test]
fn commit_failures_report_hook_output_or_exit_status() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let before = repo.head_sha();
    repo.create_file("notes.md", "save me\n");
    let hook = repo.path().join(".git/hooks/pre-commit");

    for (body, diagnostic) in [
        ("echo hook-stdout", "hook-stdout"),
        ("echo hook-stderr >&2", "hook-stderr"),
        ("", "exit status: 1 (no output)"),
    ] {
        std::fs::write(&hook, format!("#!/bin/sh\n{body}\nexit 1\n")).unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        for selected in [true, false] {
            let error = if selected {
                commit_selected(repo.path(), &["notes.md".into()], Some("Save notes")).unwrap_err()
            } else {
                commit_workflow(
                    repo.path(),
                    &commit_options("Save notes"),
                    &NullProgress,
                    &|_| {},
                )
                .unwrap_err()
            }
            .to_string();
            assert!(error.contains("git commit"), "{error}");
            assert!(error.contains(diagnostic), "{error}");
            assert_eq!(repo.head_sha(), before);
        }
    }
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
    let committed = commit_workflow(repo.path(), &options, &NullProgress, &|_| {}).expect("commit");
    assert!(committed);
    let message = last_commit_message(&repo);
    assert!(
        message.starts_with("lf implement"),
        "expected 'lf implement' prefix, got: {message}"
    );
}

#[test]
fn persistent_selection_preserves_scratch_and_unrelated_index() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("scratch/design.md", "private design\n");
    repo.create_file("scratch/.gitkeep", "");
    repo.stage_all();
    repo.commit("existing scratch");
    repo.push();
    let persistent = loopflow::git::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    // Use local HEAD as the fixture base rather than the older remote.
    let git = |args: &[&str]| git_output(&persistent.path, args);
    git(&["merge", "main"]);
    std::fs::write(persistent.path.join("memory.md"), "durable\n").unwrap();
    std::fs::write(persistent.path.join("other.md"), "staged\n").unwrap();
    git(&["add", "other.md"]);
    std::fs::write(persistent.path.join("other.md"), "later edit\n").unwrap();
    std::fs::write(
        persistent.path.join("scratch/design.md"),
        "latest private plan\n",
    )
    .unwrap();

    commit_selected(&persistent.path, &["memory.md".into()], Some("Save memory")).unwrap();

    assert_eq!(git(&["show", "HEAD:memory.md"]), "durable\n");
    assert_eq!(
        git(&[
            "ls-tree",
            "-r",
            "--name-only",
            "HEAD",
            "--",
            "scratch",
            "other.md"
        ]),
        "scratch/.gitkeep\nscratch/design.md\n"
    );
    assert_eq!(git(&["show", ":other.md"]), "staged\n");
    assert_eq!(
        std::fs::read_to_string(persistent.path.join("other.md")).unwrap(),
        "later edit\n"
    );
    assert_eq!(
        std::fs::read_to_string(persistent.path.join("scratch/design.md")).unwrap(),
        "latest private plan\n"
    );
    commit_selected(
        &persistent.path,
        &["scratch/design.md".into()],
        Some("Update existing research"),
    )
    .unwrap();
    assert_eq!(
        git(&["show", "HEAD:scratch/design.md"]),
        "latest private plan\n"
    );
    assert_eq!(git(&["show", ":other.md"]), "staged\n");
    commit_workflow(
        &persistent.path,
        &commit_options("Checkpoint durable changes"),
        &NullProgress,
        &|_| {},
    )
    .unwrap();
    assert_eq!(
        git(&["ls-tree", "-r", "--name-only", "HEAD", "--", "scratch"]),
        "scratch/.gitkeep\nscratch/design.md\n"
    );
    assert!(persistent.path.join("scratch/design.md").exists());
}

#[test]
fn rejected_push_does_not_rewrite_remote_history() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("shared.txt", "already published");
    repo.stage_all();
    repo.commit("published work");
    repo.push();
    let published = repo.bare_head_sha();
    git_output(repo.path(), &["reset", "--hard", "HEAD~1"]);
    repo.create_file("replacement.txt", "divergent local work");
    let result = commit_workflow(
        repo.path(),
        &CommitOptions {
            push: true,
            ..commit_options("replacement")
        },
        &NullProgress,
        &|_| {},
    );
    assert!(
        result.is_err(),
        "ordinary publication must not force a rejected push"
    );
    assert_eq!(repo.bare_head_sha(), published);
    assert_eq!(last_commit_message(&repo), "replacement");
}

#[test]
fn persistent_workspace_checkpoints_added_scratch_until_explicit_cleanup() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let persistent = loopflow::git::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(persistent.path.join("scratch")).unwrap();
    std::fs::write(
        persistent.path.join("scratch/design.md"),
        "repository content",
    )
    .unwrap();
    loopflow::ops::commit_selected(
        &persistent.path,
        &["scratch/design.md".into()],
        Some("Save design"),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(persistent.path.join("scratch/design.md")).unwrap(),
        "repository content"
    );
    assert_eq!(
        git_output(&persistent.path, &["show", "HEAD:scratch/design.md"]),
        "repository content"
    );
    assert_eq!(loopflow::ops::scratch::clear(&persistent.path).unwrap(), 1);
    assert!(!persistent.path.join("scratch/design.md").exists());
}
