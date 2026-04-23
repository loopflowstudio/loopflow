use std::process::Command;

use loopflow::ops::{
    list_branch_candidates, prune_branches, BranchListOptions, BranchPruneOptions, NullProgress,
};
use loopflow_test_support::TestRepo;

fn git(repo: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(repo: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git command should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn create_remote_branch(repo: &TestRepo, branch: &str) {
    repo.create_branch(branch);
    repo.create_file(&format!("{branch}.txt"), branch);
    repo.stage_all();
    repo.commit(&format!("add {branch}"));
    repo.push_new_branch(branch);
    repo.checkout("main");
}

#[test]
fn list_branch_candidates_defaults_to_current_git_user() {
    let repo = TestRepo::new();
    create_remote_branch(&repo, "jack.feature.20260401_1200");

    let candidates = list_branch_candidates(
        repo.path(),
        &BranchListOptions {
            user: None,
            wave: None,
            stale: None,
            created_before: None,
            merged: false,
            include_open_prs: false,
            default_user_if_empty: true,
        },
    )
    .expect("list branches");

    assert!(candidates
        .iter()
        .any(|candidate| candidate.branch == "jack.feature.20260401_1200"));
    assert!(!candidates
        .iter()
        .any(|candidate| candidate.branch == "main"));
}

#[test]
fn list_branch_candidates_filters_by_wave_segment() {
    let repo = TestRepo::new();
    create_remote_branch(&repo, "jack.redesign.20260401_1200");
    create_remote_branch(&repo, "jack.other.20260401_1200");

    let candidates = list_branch_candidates(
        repo.path(),
        &BranchListOptions {
            user: Some("@me".to_string()),
            wave: Some("redesign".to_string()),
            stale: None,
            created_before: None,
            merged: false,
            include_open_prs: false,
            default_user_if_empty: false,
        },
    )
    .expect("list branches");

    let names = candidates
        .iter()
        .map(|candidate| candidate.branch.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["jack.redesign.20260401_1200"]);
}

#[test]
fn prune_branches_dry_run_leaves_remote_branch() {
    let repo = TestRepo::new();
    create_remote_branch(&repo, "jack.cleanup.20260401_1200");

    let candidates = prune_branches(
        repo.path(),
        &BranchPruneOptions {
            user: Some("@me".to_string()),
            wave: Some("cleanup".to_string()),
            stale: None,
            created_before: None,
            merged: false,
            include_open_prs: false,
            dry_run: true,
            yes: true,
        },
        &NullProgress,
    )
    .expect("prune branches");

    assert_eq!(candidates.len(), 1);
    git(
        repo.path(),
        &[
            "show-ref",
            "--verify",
            "refs/remotes/origin/jack.cleanup.20260401_1200",
        ],
    );
}

#[test]
fn prune_branches_deletes_remote_branch_only() {
    let repo = TestRepo::new();
    create_remote_branch(&repo, "jack.stale.20260401_1200");

    let candidates = prune_branches(
        repo.path(),
        &BranchPruneOptions {
            user: Some("@me".to_string()),
            wave: Some("stale".to_string()),
            stale: None,
            created_before: None,
            merged: false,
            include_open_prs: false,
            dry_run: false,
            yes: true,
        },
        &NullProgress,
    )
    .expect("prune branches");

    assert_eq!(candidates.len(), 1);
    let remote_refs = git_stdout(repo.path(), &["ls-remote", "--heads", "origin"]);
    assert!(!remote_refs.contains("jack.stale.20260401_1200"));
    git(
        repo.path(),
        &[
            "show-ref",
            "--verify",
            "refs/heads/jack.stale.20260401_1200",
        ],
    );
}

#[test]
fn prune_branches_requires_filter() {
    let repo = TestRepo::new();
    let err = prune_branches(
        repo.path(),
        &BranchPruneOptions {
            user: None,
            wave: None,
            stale: None,
            created_before: None,
            merged: false,
            include_open_prs: false,
            dry_run: true,
            yes: true,
        },
        &NullProgress,
    )
    .expect_err("filter required");

    assert!(err.to_string().contains("without at least one filter"));
}
