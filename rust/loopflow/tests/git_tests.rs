use std::process::Command;

use loopflow::engine::git::{
    commit, create_branch, current_branch, get_default_branch, is_ancestor, is_clean, rebase,
};
use loopflow_test_support::TestRepo;

// =============================================================================
// Branch operations
// =============================================================================

#[test]
fn current_branch_returns_branch_name() {
    let repo = TestRepo::new();

    let branch = current_branch(repo.path()).unwrap();
    assert!(branch.is_some());
    let name = branch.unwrap();
    assert_eq!(name, "main");
}

#[test]
fn create_branch_and_switch() {
    let repo = TestRepo::new();

    create_branch(repo.path(), "feature-x").unwrap();

    let branch = current_branch(repo.path()).unwrap();
    assert_eq!(branch.as_deref(), Some("feature-x"));
}

#[test]
fn get_default_branch_returns_main_or_master() {
    let repo = TestRepo::new();

    let default = get_default_branch(repo.path()).unwrap();
    assert_eq!(default, "main");
}

// =============================================================================
// Working tree state
// =============================================================================

#[test]
fn clean_repo_reports_clean() {
    let repo = TestRepo::new();

    assert!(is_clean(repo.path()).unwrap());
}

#[test]
fn dirty_repo_reports_dirty() {
    let repo = TestRepo::new();

    repo.create_file("dirty.txt", "uncommitted");

    assert!(!is_clean(repo.path()).unwrap());
}

// =============================================================================
// Commit operations
// =============================================================================

#[test]
fn commit_creates_commit_with_staged_changes() {
    let repo = TestRepo::new();

    // Create and stage a new file
    repo.create_file("new.txt", "content");
    Command::new("git")
        .args(["add", "new.txt"])
        .current_dir(repo.path())
        .output()
        .expect("git add");

    commit(repo.path(), "add new file").unwrap();

    assert!(is_clean(repo.path()).unwrap());
}

// =============================================================================
// Ancestry and merge detection
// =============================================================================

#[test]
fn is_ancestor_detects_linear_history() {
    let repo = TestRepo::new();
    repo.create_file("first.txt", "first");
    repo.stage_all();
    repo.commit("first");
    let first_sha = repo.head_sha();

    repo.create_file("second.txt", "second");
    repo.stage_all();
    repo.commit("second");
    let second_sha = repo.head_sha();

    // first is ancestor of second
    assert!(is_ancestor(repo.path(), &first_sha, &second_sha).unwrap());
    // second is NOT ancestor of first
    assert!(!is_ancestor(repo.path(), &second_sha, &first_sha).unwrap());
}

#[test]
fn is_ancestor_same_commit() {
    let repo = TestRepo::new();
    let sha = repo.head_sha();

    // A commit is an ancestor of itself
    assert!(is_ancestor(repo.path(), &sha, &sha).unwrap());
}

// =============================================================================
// Rebase operations
// =============================================================================

#[test]
fn rebase_onto_same_branch_succeeds() {
    let repo = TestRepo::new();

    let default = get_default_branch(repo.path()).unwrap();
    let result = rebase(repo.path(), &default, None).unwrap();

    assert!(result.success);
}

#[test]
fn rebase_feature_onto_main() {
    let repo = TestRepo::new();

    let default = get_default_branch(repo.path()).unwrap();
    let initial_sha = repo.head_sha();

    // Create feature branch
    create_branch(repo.path(), "feature").unwrap();
    repo.create_file("feature.txt", "feature work");
    repo.stage_all();
    repo.commit("feature work");

    // Go back to main and add commit
    repo.checkout(&default);
    repo.create_file("main.txt", "main work");
    repo.stage_all();
    repo.commit("main work");
    let main_sha = repo.head_sha();

    // Back to feature and rebase
    repo.checkout("feature");

    let result = rebase(repo.path(), &default, None).unwrap();
    assert!(result.success);

    // Feature should now be based on main's latest
    assert!(is_ancestor(repo.path(), &main_sha, &repo.head_sha()).unwrap());
    assert!(is_ancestor(repo.path(), &initial_sha, &repo.head_sha()).unwrap());
}

// =============================================================================
// Sync operations
// =============================================================================

// Note: sync_main requires a remote, so we test the is_clean prerequisite instead
#[test]
fn sync_would_fail_on_dirty_tree() {
    let repo = TestRepo::new();

    repo.create_file("dirty.txt", "uncommitted");

    // sync_main checks is_clean first - verify the check would fail
    assert!(!is_clean(repo.path()).unwrap());
}
