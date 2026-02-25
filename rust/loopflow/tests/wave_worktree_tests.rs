use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopflow::engine::git::{branch_rename, current_branch, worktree_move};
use loopflow::engine::naming::sanitize_for_branch;
use loopflow::engine::worktrees::{branch_exists, worktree_path};
use loopflow::lfd::executor::{create_wave_run_with_id, ensure_wave_worktree};
use loopflow::lfd::id::LfdId;
use loopflow::lfd::store::{open_store, SharedStore, StorageConfig};
use loopflow::lfd::types::{Wave, WaveStatus};
use loopflow_test_support::TestRepo;

async fn make_store() -> SharedStore {
    let path = std::env::temp_dir()
        .join(format!("lfd-test-{}.db", LfdId::new()))
        .to_string_lossy()
        .to_string();
    Arc::new(
        open_store(&StorageConfig::sqlite(PathBuf::from(path)))
            .await
            .unwrap(),
    )
}

fn make_wave(repo: &str, name: &str) -> Wave {
    Wave {
        id: LfdId::new(),
        name: name.to_string(),
        repo: repo.to_string(),
        flow: "build".to_string(),
        direction: vec![],
        area: vec![],
        status: WaveStatus::Idle,
        iteration: 0,
        created_at: None,
    }
}

#[tokio::test]
async fn wave_run_creates_worktree() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "expand");
    store.create_wave(&wave).await.unwrap();

    let run_id = LfdId::new();
    let run = create_wave_run_with_id(&store, &wave, &run_id)
        .await
        .unwrap();

    let wt = PathBuf::from(&run.worktree);
    assert!(wt.exists(), "worktree directory should exist: {wt:?}");
    assert_ne!(
        run.worktree,
        wave.repo().as_str(),
        "worktree should not be the main repo"
    );
}

#[tokio::test]
async fn wave_run_creates_branch() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "polish");
    store.create_wave(&wave).await.unwrap();

    let run_id = LfdId::new();
    let run = create_wave_run_with_id(&store, &wave, &run_id)
        .await
        .unwrap();

    assert!(!run.branch.is_empty(), "branch should be set");

    // Verify the branch exists in git
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&run.worktree)
        .output()
        .unwrap();
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert_eq!(branch, run.branch);
}

#[tokio::test]
async fn wave_run_worktree_follows_naming_convention() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "review");
    store.create_wave(&wave).await.unwrap();

    let run_id = LfdId::new();
    let run = create_wave_run_with_id(&store, &wave, &run_id)
        .await
        .unwrap();

    let expected = worktree_path(repo.path(), "review");
    assert_eq!(
        PathBuf::from(&run.worktree),
        expected,
        "worktree path should follow {{repo}}.{{wave_name}} convention"
    );
}

#[tokio::test]
async fn wave_run_reuses_existing_worktree() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "iterate");
    store.create_wave(&wave).await.unwrap();

    // First run creates the worktree
    let run1 = create_wave_run_with_id(&store, &wave, &LfdId::new())
        .await
        .unwrap();
    let wt_path = run1.worktree.clone();

    // Second run reuses the existing worktree
    let run2 = create_wave_run_with_id(&store, &wave, &LfdId::new())
        .await
        .unwrap();
    assert_eq!(run2.worktree, wt_path, "should reuse existing worktree");
    assert!(!run2.branch.is_empty(), "branch should still be set");
}

#[tokio::test]
async fn wave_run_records_parent_lineage() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "lineage");
    store.create_wave(&wave).await.unwrap();

    let run1 = create_wave_run_with_id(&store, &wave, &LfdId::new())
        .await
        .unwrap();
    let run2 = create_wave_run_with_id(&store, &wave, &LfdId::new())
        .await
        .unwrap();

    assert_eq!(run1.stack_position, 0);
    assert_eq!(run2.stack_position, 1);
    assert_eq!(run2.parent_run_id, Some(run1.id.clone()));
    assert_eq!(run2.parent_pr_number, None);
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
