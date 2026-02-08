use std::path::PathBuf;
use std::sync::Arc;

use loopflow::engine::worktrees::worktree_path;
use loopflow::lfd::executor::{create_wave_run_with_id, ensure_wave_worktree};
use loopflow::lfd::id::LfdId;
use loopflow::lfd::store::sqlite::SqliteStore;
use loopflow::lfd::store::SharedStore;
use loopflow::lfd::types::{Wave, WaveStatus};
use loopflow_test_support::TestRepo;

fn make_store() -> SharedStore {
    let path = std::env::temp_dir()
        .join(format!("lfd-test-{}.db", LfdId::new()))
        .to_string_lossy()
        .to_string();
    Arc::new(SqliteStore::new(&PathBuf::from(path)).unwrap())
}

fn make_wave(repo: &str, name: &str) -> Wave {
    Wave {
        id: LfdId::new(),
        name: name.to_string(),
        repo: repo.to_string(),
        flow: "ship".to_string(),
        direction: vec![],
        area: vec![],
        status: WaveStatus::Idle,
        iteration: 0,
        created_at: None,
    }
}

#[test]
fn wave_run_creates_worktree() {
    let repo = TestRepo::new();
    let store = make_store();
    let wave = make_wave(&repo.path().to_string_lossy(), "expand");
    store.create_wave(&wave).unwrap();

    let run_id = LfdId::new();
    let run = create_wave_run_with_id(&store, &wave, &run_id).unwrap();

    let wt = PathBuf::from(&run.worktree);
    assert!(wt.exists(), "worktree directory should exist: {wt:?}");
    assert_ne!(
        run.worktree, wave.repo,
        "worktree should not be the main repo"
    );
}

#[test]
fn wave_run_creates_branch() {
    let repo = TestRepo::new();
    let store = make_store();
    let wave = make_wave(&repo.path().to_string_lossy(), "polish");
    store.create_wave(&wave).unwrap();

    let run_id = LfdId::new();
    let run = create_wave_run_with_id(&store, &wave, &run_id).unwrap();

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

#[test]
fn wave_run_worktree_follows_naming_convention() {
    let repo = TestRepo::new();
    let store = make_store();
    let wave = make_wave(&repo.path().to_string_lossy(), "review");
    store.create_wave(&wave).unwrap();

    let run_id = LfdId::new();
    let run = create_wave_run_with_id(&store, &wave, &run_id).unwrap();

    let expected = worktree_path(repo.path(), "review");
    assert_eq!(
        PathBuf::from(&run.worktree),
        expected,
        "worktree path should follow {{repo}}.{{wave_name}} convention"
    );
}

#[test]
fn wave_run_reuses_existing_worktree() {
    let repo = TestRepo::new();
    let store = make_store();
    let wave = make_wave(&repo.path().to_string_lossy(), "iterate");
    store.create_wave(&wave).unwrap();

    // First run creates the worktree
    let run1 = create_wave_run_with_id(&store, &wave, &LfdId::new()).unwrap();
    let wt_path = run1.worktree.clone();

    // Second run reuses the existing worktree
    let run2 = create_wave_run_with_id(&store, &wave, &LfdId::new()).unwrap();
    assert_eq!(run2.worktree, wt_path, "should reuse existing worktree");
    assert!(!run2.branch.is_empty(), "branch should still be set");
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
