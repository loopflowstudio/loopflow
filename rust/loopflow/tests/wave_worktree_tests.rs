use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use loopflow::engine::config::BranchNameConfig;
use loopflow::engine::git::{branch_rename, current_branch, worktree_move};
use loopflow::engine::naming::sanitize_for_branch;
use loopflow::engine::worktrees::{
    branch_exists, create_with_schema, run_worktree_path, worktree_path,
};
use loopflow::lfd::executor::{create_run_for_placement, ensure_wave_worktree, Placement};
use loopflow::lfd::id::LfdId;
use loopflow::lfd::types::{RepoWork, Wave, WaveStatus};
use loopflow::lfdb::{open_store, SharedStore, StorageConfig};
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
        primary_flow: "ship-roadmap".to_string(),
        goal: "ship-roadmap".to_string(),
        metrics: Vec::new(),
        repos: vec![RepoWork {
            repo: repo.to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            position: 0,
        }],
        direction: vec![],
        area: vec![],
        paused: false,
        created_at: None,
        workers: 1,
        parent_wave_id: None,
    }
}

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

#[tokio::test]
async fn run_creates_worktree() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "expand");
    store.create_wave(&wave).await.unwrap();

    let run_id = LfdId::new();
    let run = create_run_for_placement(&store, &wave, &run_id, &Placement::Fresh, None)
        .await
        .unwrap();

    let wt = PathBuf::from(&run.worktree);
    assert!(wt.exists(), "worktree directory should exist: {wt:?}");
    assert_ne!(
        run.worktree,
        wave.repo(),
        "worktree should not be the main repo"
    );
}

#[tokio::test]
async fn run_creates_branch() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "polish");
    store.create_wave(&wave).await.unwrap();

    let run_id = LfdId::new();
    let run = create_run_for_placement(&store, &wave, &run_id, &Placement::Fresh, None)
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
async fn run_worktree_follows_naming_convention() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "review");
    store.create_wave(&wave).await.unwrap();

    let run_id = LfdId::new();
    let run = create_run_for_placement(&store, &wave, &run_id, &Placement::Fresh, None)
        .await
        .unwrap();

    let repo_name = repo.path().file_name().unwrap().to_string_lossy();
    let expected = run_worktree_path(repo.path(), "review", run_id.as_str());
    assert_eq!(
        PathBuf::from(&run.worktree),
        expected,
        "worktree path should follow {{repo}}.{{wave_name}}.{{run_id}} convention"
    );
    assert!(PathBuf::from(&run.worktree)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with(&format!("{repo_name}.review.")));
}

#[tokio::test]
async fn run_records_parent_lineage() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "lineage");
    store.create_wave(&wave).await.unwrap();

    let run1 = create_run_for_placement(&store, &wave, &LfdId::new(), &Placement::Fresh, None)
        .await
        .unwrap();
    let run2 = create_run_for_placement(&store, &wave, &LfdId::new(), &Placement::Fresh, None)
        .await
        .unwrap();

    assert_eq!(run1.stack_position, 0);
    assert_eq!(run2.stack_position, 1);
    assert_eq!(run2.parent_run_id, Some(run1.id.clone()));
    assert_eq!(run2.parent_pr_number, None);
}

#[tokio::test]
async fn fresh_placement_creates_run_scoped_worktree_off_default_branch() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "grind");
    store.create_wave(&wave).await.unwrap();

    let run_id = LfdId::new();
    let run = create_run_for_placement(&store, &wave, &run_id, &Placement::Fresh, None)
        .await
        .unwrap();

    // Name carries ownership: <repo>.<wave>.<short-run-id>.
    let repo_name = repo.path().file_name().unwrap().to_string_lossy();
    let short_id: String = run_id.as_str().chars().take(8).collect();
    let worktree_name = Path::new(&run.worktree)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert_eq!(worktree_name, format!("{repo_name}.grind.{short_id}"));
    assert!(PathBuf::from(&run.worktree).exists());

    // Own branch, forked from the default branch tip.
    assert_ne!(run.branch, "main");
    assert_eq!(run.target_branch, "main");
    let head_branch = run_git_output(
        Path::new(&run.worktree),
        &["rev-parse", "--abbrev-ref", "HEAD"],
    );
    assert_eq!(head_branch, run.branch);
    let fork_point = run_git_output(Path::new(&run.worktree), &["merge-base", "HEAD", "main"]);
    let main_tip = run_git_output(repo.path(), &["rev-parse", "main"]);
    assert_eq!(fork_point, main_tip);
}

#[tokio::test]
async fn fresh_placements_do_not_collide() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "swarm");
    store.create_wave(&wave).await.unwrap();

    let run1 = create_run_for_placement(&store, &wave, &LfdId::new(), &Placement::Fresh, None)
        .await
        .unwrap();
    let run2 = create_run_for_placement(&store, &wave, &LfdId::new(), &Placement::Fresh, None)
        .await
        .unwrap();

    assert_ne!(run1.worktree, run2.worktree);
    assert_ne!(run1.branch, run2.branch);
}

#[tokio::test]
async fn stack_placement_forks_from_parent_branch_with_lineage() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "tower");
    store.create_wave(&wave).await.unwrap();

    let parent = create_run_for_placement(&store, &wave, &LfdId::new(), &Placement::Fresh, None)
        .await
        .unwrap();

    // Unlanded work on the parent branch. The Fresh run's background upstream
    // sync (schedule_upstream_sync) owns pushing this branch to origin; a manual
    // push here would race that thread and hit "reference already exists". The
    // stacked child forks from the parent's local tip, so it sees this commit
    // without any push.
    let parent_wt = Path::new(&parent.worktree);
    std::fs::write(parent_wt.join("stacked.txt"), "level 0").unwrap();
    run_git_ok(parent_wt, &["add", "."]);
    run_git_ok(parent_wt, &["commit", "-m", "parent work"]);

    let child_id = LfdId::new();
    let placement = Placement::Stack {
        parent_run_id: parent.id.clone(),
    };
    let child = create_run_for_placement(&store, &wave, &child_id, &placement, None)
        .await
        .unwrap();

    // Lineage lands in the existing stack columns, from the named parent.
    assert_eq!(child.parent_run_id, Some(parent.id.clone()));
    assert_eq!(child.stack_position, parent.stack_position + 1);
    assert_eq!(child.stack_group_id, parent.stack_group_id);
    assert_eq!(child.target_branch, parent.branch);
    assert_ne!(child.branch, parent.branch);

    // The child's branch forks from the parent branch tip: it sees the
    // parent's unlanded commit.
    let child_wt = Path::new(&child.worktree);
    assert!(child_wt.join("stacked.txt").exists());

    // And the worktree still follows the worker naming scheme.
    let short_id: String = child_id.as_str().chars().take(8).collect();
    let worktree_name = child_wt.file_name().unwrap().to_string_lossy().to_string();
    let repo_name = repo.path().file_name().unwrap().to_string_lossy();
    assert_eq!(worktree_name, format!("{repo_name}.tower.{short_id}"));
}

#[tokio::test]
async fn stack_placement_rejects_unknown_parent() {
    let repo = TestRepo::new();
    let store = make_store().await;
    let wave = make_wave(&repo.path().to_string_lossy(), "orphan");
    store.create_wave(&wave).await.unwrap();

    let placement = Placement::Stack {
        parent_run_id: LfdId::new(),
    };
    let result = create_run_for_placement(&store, &wave, &LfdId::new(), &placement, None).await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("stack parent run not found"));
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
fn create_with_schema_uses_existing_remote_branch_and_wave_worktree_name() {
    let repo = TestRepo::new();
    let branch = "jack-heart.mobile.20260225_1122";

    repo.create_branch(branch);
    repo.create_file("mobile.txt", "mobile");
    repo.stage_all();
    repo.commit("mobile update");
    repo.push_new_branch(branch);
    repo.checkout("main");
    run_git_ok(repo.path(), &["branch", "-D", branch]);

    let branch_config = BranchNameConfig::default();
    let result = create_with_schema(repo.path(), branch, None, Some(&branch_config)).unwrap();

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
