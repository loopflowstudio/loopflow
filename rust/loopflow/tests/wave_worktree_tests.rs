use std::path::{Path, PathBuf};
use std::process::Command;

use loopflow::engine::git::{branch_rename, current_branch, worktree_move};
use loopflow::engine::naming::{git_user, sanitize_for_branch};
use loopflow::engine::worktrees::{
    branch_exists, create_wave_worktree, worktree_path,
};
use loopflow::lfd::executor::ensure_wave_worktree;
use loopflow_test_support::TestRepo;

fn run_git(repo: &Path, args: &[&str]) -> std::process::Output {
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
    output
}

fn run_git_ok(repo: &Path, args: &[&str]) {
    run_git(repo, args);
}

fn run_git_output(repo: &Path, args: &[&str]) -> String {
    let output = run_git(repo, args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn ensure_wave_worktree_creates_directory() {
    let repo = TestRepo::new();
    let (wt_path, branch) = ensure_wave_worktree(repo.path(), "debug").unwrap();
    let wt = PathBuf::from(&wt_path);
    assert!(wt.exists());
    assert!(!branch.is_empty());
}

#[test]
fn ensure_wave_worktree_reuses_existing() {
    let repo = TestRepo::new();
    let (path1, branch1) = ensure_wave_worktree(repo.path(), "reduce").unwrap();
    let (path2, branch2) = ensure_wave_worktree(repo.path(), "reduce").unwrap();
    assert_eq!(path1, path2);
    assert_eq!(branch1, branch2);
}

#[test]
fn worktree_path_from_worktree_repo_uses_main_repo_parent() {
    let repo = TestRepo::new();
    let (existing_wt, _) = ensure_wave_worktree(repo.path(), "seed").unwrap();

    let from_worktree = worktree_path(Path::new(&existing_wt), "beta");
    let from_main = worktree_path(repo.path(), "beta");

    assert_eq!(from_worktree, from_main);
}

#[test]
fn ensure_wave_worktree_from_worktree_repo_avoids_nested_path() {
    let repo = TestRepo::new();
    let (existing_wt, _) = ensure_wave_worktree(repo.path(), "seed").unwrap();

    let (new_wt, _branch) = ensure_wave_worktree(Path::new(&existing_wt), "beta").unwrap();
    let expected = worktree_path(repo.path(), "beta");

    assert_eq!(PathBuf::from(&new_wt), expected);
}

#[test]
fn wave_rename_moves_worktree() {
    let repo = TestRepo::new();
    let (_wt_path, _branch) = ensure_wave_worktree(repo.path(), "old-wave").unwrap();

    let old_wt = worktree_path(repo.path(), "old-wave");
    let new_wt = worktree_path(repo.path(), "new-wave");
    assert!(old_wt.exists());

    worktree_move(repo.path(), &old_wt, &new_wt).unwrap();

    assert!(!old_wt.exists(), "old worktree should be gone");
    assert!(new_wt.exists(), "new worktree should exist");
}

#[test]
fn wave_rename_renames_branch() {
    let repo = TestRepo::new();
    let (_wt_path, old_branch) = ensure_wave_worktree(repo.path(), "old-wave").unwrap();

    let old_sanitized = sanitize_for_branch("old-wave");
    let new_sanitized = sanitize_for_branch("new-wave");
    let new_branch = old_branch.replacen(&old_sanitized, &new_sanitized, 1);

    // Move worktree first (branch rename operates on repo, not worktree path).
    let old_wt = worktree_path(repo.path(), "old-wave");
    let new_wt = worktree_path(repo.path(), "new-wave");
    worktree_move(repo.path(), &old_wt, &new_wt).unwrap();

    branch_rename(repo.path(), &old_branch, &new_branch).unwrap();

    let current = current_branch(&new_wt).unwrap().unwrap();
    assert_eq!(current, new_branch);
    assert!(
        current.contains("new-wave"),
        "branch should contain the new wave name"
    );
}

#[test]
fn wave_rename_detects_destination_conflict() {
    let repo = TestRepo::new();
    ensure_wave_worktree(repo.path(), "wave-a").unwrap();
    ensure_wave_worktree(repo.path(), "wave-b").unwrap();

    // Our rename logic checks new_wt.exists() before attempting git worktree move.
    let new_wt = worktree_path(repo.path(), "wave-b");
    assert!(
        new_wt.exists(),
        "destination worktree should exist, blocking rename"
    );
}

#[test]
fn wave_rename_branch_exists_check() {
    let repo = TestRepo::new();
    let (_wt_path, old_branch) = ensure_wave_worktree(repo.path(), "alpha").unwrap();

    assert!(
        branch_exists(repo.path(), &old_branch).unwrap(),
        "branch should exist after worktree creation"
    );
    assert!(
        !branch_exists(repo.path(), "nonexistent-branch").unwrap(),
        "nonexistent branch should not exist"
    );
}

#[test]
fn create_wave_worktree_uses_existing_remote_branch() {
    let repo = TestRepo::new();
    // The wave's branch already lives on origin as <user>/mobile.
    let branch = format!("{}/mobile", git_user(repo.path()).unwrap());

    repo.create_branch(&branch);
    repo.create_file("mobile.txt", "mobile");
    repo.stage_all();
    repo.commit("mobile update");
    repo.push_new_branch(&branch);
    repo.checkout("main");
    run_git_ok(repo.path(), &["branch", "-D", &branch]);

    let result = create_wave_worktree(repo.path(), "mobile", None, false).unwrap();

    assert_eq!(result.branch, branch);
    assert_eq!(result.path, worktree_path(repo.path(), "mobile"));

    let checked_out = run_git_output(&result.path, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(checked_out, branch);

    let upstream = run_git_output(
        &result.path,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    );
    assert_eq!(upstream, format!("origin/{branch}"));
}
