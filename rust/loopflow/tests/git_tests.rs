use std::process::Command;

use loopflow::engine::git::{
    commit, create_branch, current_branch, get_default_branch, is_ancestor, is_clean, rebase,
    sync_main,
};
use loopflow_test_support::TestRepo;

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed in {:?}: {}",
        args,
        dir,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

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

/// Calling sync_main from a feature worktree must not leave the sibling main
/// worktree with a stale working tree. Before the fix, sync_main advanced
/// refs/heads/main via update-ref while the main worktree's HEAD file and
/// working tree stayed at the old commit — `git status` on main then reported
/// the merged commits as uncommitted deletions.
#[test]
fn sync_main_from_feature_worktree_keeps_sibling_main_clean() {
    let repo = TestRepo::new();
    let bare = repo.bare_path().to_path_buf();

    // Advance origin/main via a second clone so the main worktree doesn't
    // already hold the new commit.
    let pusher = tempfile::TempDir::new().unwrap();
    git(pusher.path(), &["clone", bare.to_str().unwrap(), "."]);
    git(pusher.path(), &["config", "user.email", "t@t"]);
    git(pusher.path(), &["config", "user.name", "t"]);
    std::fs::write(pusher.path().join("upstream.txt"), "from upstream").unwrap();
    git(pusher.path(), &["add", "."]);
    git(pusher.path(), &["commit", "-m", "upstream change"]);
    git(pusher.path(), &["push", "origin", "main"]);
    let upstream_sha = git(pusher.path(), &["rev-parse", "HEAD"]);

    // Create a feature worktree off the (now stale) main worktree.
    let feature = repo.create_named_worktree("myfeature");

    // Feature worktree calls sync_main to fast-forward local main.
    sync_main(&feature, "main").unwrap();

    // Invariant: the main worktree must remain clean and match origin/main.
    let status = git(repo.path(), &["status", "--porcelain"]);
    assert!(
        status.is_empty(),
        "main worktree should be clean after sync_main from feature worktree, got:\n{status}"
    );
    let main_head = git(repo.path(), &["rev-parse", "HEAD"]);
    assert_eq!(
        main_head, upstream_sha,
        "main worktree HEAD should match origin/main after sync"
    );
}

/// Dirty local edits on the main worktree must survive a sync_main call from
/// a sibling worktree.
#[test]
fn sync_main_from_feature_preserves_dirty_work_on_main() {
    let repo = TestRepo::new();
    let bare = repo.bare_path().to_path_buf();

    // Advance origin/main via a second clone.
    let pusher = tempfile::TempDir::new().unwrap();
    git(pusher.path(), &["clone", bare.to_str().unwrap(), "."]);
    git(pusher.path(), &["config", "user.email", "t@t"]);
    git(pusher.path(), &["config", "user.name", "t"]);
    std::fs::write(pusher.path().join("upstream.txt"), "upstream").unwrap();
    git(pusher.path(), &["add", "."]);
    git(pusher.path(), &["commit", "-m", "upstream"]);
    git(pusher.path(), &["push", "origin", "main"]);

    // Leave uncommitted work in the main worktree.
    std::fs::write(repo.path().join("scratch_note.txt"), "wip").unwrap();

    let feature = repo.create_named_worktree("myfeature");
    sync_main(&feature, "main").unwrap();

    // Uncommitted file must be back after stash pop.
    assert_eq!(
        std::fs::read_to_string(repo.path().join("scratch_note.txt")).unwrap(),
        "wip",
        "uncommitted file on main should be preserved across sync_main"
    );
    // And the pulled commit's file must be present too.
    assert!(
        repo.path().join("upstream.txt").exists(),
        "main worktree should have fast-forwarded to origin/main"
    );
}

/// Local edits to paths the default branch *rewrote* must not be popped back
/// over the synced tree — a 3-way merge there silently resurrects stale files
/// and reverts the merged work. The edits are preserved in a stash instead.
#[test]
fn sync_main_does_not_revert_rewritten_paths() {
    let repo = TestRepo::new();
    let bare = repo.bare_path().to_path_buf();

    // Main commits `old.txt`, then origin/main rewrites that area: delete
    // `old.txt`, add `new.txt` (mirrors a wave recut landing on main).
    std::fs::write(repo.path().join("old.txt"), "v1").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-m", "add old.txt"]);
    git(repo.path(), &["push", "origin", "main"]);

    let pusher = tempfile::TempDir::new().unwrap();
    git(pusher.path(), &["clone", bare.to_str().unwrap(), "."]);
    git(pusher.path(), &["config", "user.email", "t@t"]);
    git(pusher.path(), &["config", "user.name", "t"]);
    std::fs::remove_file(pusher.path().join("old.txt")).unwrap();
    std::fs::write(pusher.path().join("new.txt"), "v2").unwrap();
    git(pusher.path(), &["add", "-A"]);
    git(
        pusher.path(),
        &["commit", "-m", "recut: old.txt -> new.txt"],
    );
    git(pusher.path(), &["push", "origin", "main"]);

    // Dirty edit on main to the very path the recut rewrote.
    std::fs::write(repo.path().join("old.txt"), "local wip").unwrap();

    let feature = repo.create_named_worktree("myfeature");
    sync_main(&feature, "main").unwrap();

    // The synced tree wins: old.txt is gone, new.txt present. No silent revert.
    assert!(
        !repo.path().join("old.txt").exists(),
        "old.txt must stay deleted; the stash must not be popped over the recut"
    );
    assert!(
        repo.path().join("new.txt").exists(),
        "main worktree should have synced to origin/main"
    );
    // The local edit is not lost — it's preserved in a stash for recovery.
    let stash_list = git(repo.path(), &["stash", "list"]);
    assert!(
        stash_list.contains("sync_main: auto-stash"),
        "local edit should be preserved in a stash, got: {stash_list:?}"
    );
}
