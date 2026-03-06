use loopflow::ops::{rebase_with_recovery, NullProgress, OpsError, RebaseOptions};
use loopflow_test_support::TestRepo;
use std::process::Command;

fn git(repo: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn rebase_onto_main_succeeds() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    repo.create_file("main.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    repo.push();

    repo.checkout("feature");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    )
    .expect("rebase");
}

#[test]
fn rebase_conflict_returns_error() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("conflict.txt", "feature line\n");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    repo.create_file("conflict.txt", "main line\n");
    repo.stage_all();
    repo.commit("main work");
    repo.push();

    repo.checkout("feature");
    let result = rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::RebaseConflict { ref onto, .. }) if onto == "origin/main"),
        "expected rebase conflict error, got: {:?}",
        result
    );
}

#[test]
fn rebase_with_push() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.push_new_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    repo.create_file("main.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    repo.push();

    repo.checkout("feature");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: true,
        },
        &NullProgress,
    )
    .expect("rebase");
}

#[test]
fn rebase_stacked_branch_after_squash_merge() {
    let repo = TestRepo::new();

    // Branch A: two commits on top of main.
    repo.create_branch("branch-a");
    repo.create_file("a1.txt", "a1");
    repo.stage_all();
    repo.commit("a1");
    repo.create_file("a2.txt", "a2");
    repo.stage_all();
    repo.commit("a2");

    // Branch B: stacked on A with its own commits.
    repo.create_branch("branch-b");
    repo.create_file("b1.txt", "b1");
    repo.stage_all();
    repo.commit("b1");

    let b_head = repo.head_sha();

    // Simulate squash-merge of A into main.
    repo.checkout("main");
    git(repo.path(), &["merge", "--squash", "branch-a"]);
    git(repo.path(), &["commit", "-m", "squash merge branch-a"]);
    repo.push();

    // Rebase B onto origin/main — without fork-point detection this would conflict.
    repo.checkout("branch-b");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    )
    .expect("rebase of stacked branch after squash-merge should succeed");

    // B's commit (b1.txt) should be present.
    assert!(
        repo.path().join("b1.txt").exists(),
        "b1.txt should exist after rebase"
    );
    // A's changes should also be present (from the squash merge on main).
    assert!(
        repo.path().join("a1.txt").exists(),
        "a1.txt should exist from squash merge"
    );
    // HEAD should have changed (rebased onto new main).
    assert_ne!(repo.head_sha(), b_head, "HEAD should differ after rebase");
}
