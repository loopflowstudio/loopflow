mod support;

use std::process::Command;

use loopflow::ops::{commit_workflow, CommitOptions, NullProgress};
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
    let _env = EnvGuard::new(&[]);
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

#[test]
fn persistent_selection_preserves_scratch_and_unrelated_index() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("scratch/design.md", "private design\n");
    repo.create_file("scratch/.gitkeep", "");
    repo.stage_all();
    repo.commit("existing scratch");
    let persistent = loopflow::engine::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::engine::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    // Use local HEAD as the fixture base rather than the older remote.
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .current_dir(&persistent.path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
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

    loopflow::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Save memory"))
        .unwrap();

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
        "scratch/.gitkeep\n"
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
    commit_workflow(
        &persistent.path,
        &commit_options("Checkpoint durable changes"),
        &NullProgress,
    )
    .unwrap();
    assert_eq!(
        git(&["ls-tree", "-r", "--name-only", "HEAD", "--", "scratch"]),
        "scratch/.gitkeep\n"
    );
    assert!(persistent.path.join("scratch/design.md").exists());
}
