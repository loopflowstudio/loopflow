use std::path::Path;
use std::process::Command;

use loopflow_engine::git::{
    commit, create_branch, current_branch, get_default_branch, is_ancestor, is_clean, rebase,
};
use tempfile::TempDir;

fn init_repo(dir: &Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .expect("git config email");
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir)
        .output()
        .expect("git config name");
}

fn make_commit(dir: &Path, message: &str) {
    let file = dir.join(format!("{}.txt", message.replace(' ', "_")));
    std::fs::write(&file, message).expect("write file");
    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(dir)
        .output()
        .expect("git commit");
}

// =============================================================================
// Branch operations
// =============================================================================

#[test]
fn current_branch_returns_branch_name() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    let branch = current_branch(repo).unwrap();
    assert!(branch.is_some());
    // Could be "main" or "master" depending on git config
    let name = branch.unwrap();
    assert!(name == "main" || name == "master");
}

#[test]
fn create_branch_and_switch() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    create_branch(repo, "feature-x").unwrap();

    let branch = current_branch(repo).unwrap();
    assert_eq!(branch.as_deref(), Some("feature-x"));
}

#[test]
fn get_default_branch_returns_main_or_master() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    let default = get_default_branch(repo).unwrap();
    assert!(default == "main" || default == "master");
}

// =============================================================================
// Working tree state
// =============================================================================

#[test]
fn clean_repo_reports_clean() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    assert!(is_clean(repo).unwrap());
}

#[test]
fn dirty_repo_reports_dirty() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    std::fs::write(repo.join("dirty.txt"), "uncommitted").unwrap();

    assert!(!is_clean(repo).unwrap());
}

// =============================================================================
// Commit operations
// =============================================================================

#[test]
fn commit_creates_commit_with_staged_changes() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    // Create and stage a new file
    std::fs::write(repo.join("new.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "new.txt"])
        .current_dir(repo)
        .output()
        .expect("git add");

    commit(repo, "add new file").unwrap();

    assert!(is_clean(repo).unwrap());
}

// =============================================================================
// Ancestry and merge detection
// =============================================================================

#[test]
fn is_ancestor_detects_linear_history() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "first");

    let first_sha = get_head_sha(repo);

    make_commit(repo, "second");

    let second_sha = get_head_sha(repo);

    // first is ancestor of second
    assert!(is_ancestor(repo, &first_sha, &second_sha).unwrap());
    // second is NOT ancestor of first
    assert!(!is_ancestor(repo, &second_sha, &first_sha).unwrap());
}

#[test]
fn is_ancestor_same_commit() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "only");

    let sha = get_head_sha(repo);

    // A commit is an ancestor of itself
    assert!(is_ancestor(repo, &sha, &sha).unwrap());
}

// =============================================================================
// Rebase operations
// =============================================================================

#[test]
fn rebase_onto_same_branch_succeeds() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    let default = get_default_branch(repo).unwrap();
    let result = rebase(repo, &default, None).unwrap();

    assert!(result.success);
}

#[test]
fn rebase_feature_onto_main() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    let default = get_default_branch(repo).unwrap();
    let initial_sha = get_head_sha(repo);

    // Create feature branch
    create_branch(repo, "feature").unwrap();
    make_commit(repo, "feature work");

    // Go back to main and add commit
    Command::new("git")
        .args(["checkout", &default])
        .current_dir(repo)
        .output()
        .unwrap();
    make_commit(repo, "main work");
    let main_sha = get_head_sha(repo);

    // Back to feature and rebase
    Command::new("git")
        .args(["checkout", "feature"])
        .current_dir(repo)
        .output()
        .unwrap();

    let result = rebase(repo, &default, None).unwrap();
    assert!(result.success);

    // Feature should now be based on main's latest
    assert!(is_ancestor(repo, &main_sha, &get_head_sha(repo)).unwrap());
    assert!(is_ancestor(repo, &initial_sha, &get_head_sha(repo)).unwrap());
}

// =============================================================================
// Sync operations
// =============================================================================

// Note: sync_main requires a remote, so we test the is_clean prerequisite instead
#[test]
fn sync_would_fail_on_dirty_tree() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    init_repo(repo);
    make_commit(repo, "initial");

    std::fs::write(repo.join("dirty.txt"), "uncommitted").unwrap();

    // sync_main checks is_clean first - verify the check would fail
    assert!(!is_clean(repo).unwrap());
}

// =============================================================================
// Helpers
// =============================================================================

fn get_head_sha(repo: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .expect("git rev-parse");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
