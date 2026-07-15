use loopflow::ops::{
    plan_rebase, rebase_with_recovery, NullProgress, OpsError, RebaseClass, RebaseOptions,
    RebaseStrategy,
};
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
        "git {args:?} failed: {}",
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
            fork_base: None,
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
            fork_base: None,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::RebaseConflict { ref onto, .. }) if onto == "origin/main"),
        "expected rebase conflict, got {result:?}"
    );
}

#[test]
fn rebase_after_squash_merge_replays_only_unique_work() {
    let repo = TestRepo::new();
    repo.create_branch("parent");
    repo.create_file("a1.txt", "a1");
    repo.stage_all();
    repo.commit("a1");
    repo.create_file("a2.txt", "a2");
    repo.stage_all();
    repo.commit("a2");

    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    repo.checkout("main");
    git(repo.path(), &["merge", "--squash", "parent"]);
    git(repo.path(), &["commit", "-m", "squash parent"]);
    repo.push();

    repo.checkout("feature");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect("rebase after squash merge");

    assert!(repo.path().join("feature.txt").exists());
    assert_eq!(
        git(repo.path(), &["diff", "--name-only", "origin/main...HEAD"]),
        "feature.txt"
    );
}

#[test]
fn stacked_child_collapses_onto_main_dropping_squashed_parent() {
    // A child stacked on a parent whose two commits both edit the same file:
    // once squash-merged, `git cherry` cannot match the combined patch, so the
    // durable fork base is the only signal that drops the parent's work cleanly.
    let repo = TestRepo::new();
    repo.create_branch("parent");
    repo.create_file("shared.txt", "one\n");
    repo.stage_all();
    repo.commit("parent one");
    repo.create_file("shared.txt", "one\ntwo\n");
    repo.stage_all();
    repo.commit("parent two");
    let parent_tip = git(repo.path(), &["rev-parse", "HEAD"]);

    repo.create_branch("child");
    repo.create_file("child.txt", "child");
    repo.stage_all();
    repo.commit("child work");

    repo.checkout("main");
    git(repo.path(), &["merge", "--squash", "parent"]);
    git(repo.path(), &["commit", "-m", "squash parent"]);
    repo.push();

    repo.checkout("child");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: Some(parent_tip),
        },
        &NullProgress,
    )
    .expect("stacked child collapses onto main");

    assert!(repo.path().join("child.txt").exists());
    assert_eq!(
        git(repo.path(), &["diff", "--name-only", "origin/main...HEAD"]),
        "child.txt",
        "only the child's own file should remain over main"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("shared.txt")).unwrap(),
        "one\ntwo\n",
        "the squashed parent content must not be reintroduced or conflicted"
    );
}

#[test]
fn stacked_rebase_refuses_when_base_is_not_an_ancestor() {
    // A fork base that is not an ancestor of HEAD means the child's own history
    // was rewritten; replaying would rewrite history blindly, so refuse.
    let repo = TestRepo::new();
    repo.create_branch("sibling");
    repo.create_file("sibling.txt", "sibling");
    repo.stage_all();
    repo.commit("sibling work");
    let unrelated = git(repo.path(), &["rev-parse", "HEAD"]);

    repo.checkout("main");
    repo.create_branch("child");
    repo.create_file("child.txt", "child");
    repo.stage_all();
    repo.commit("child work");

    let result = rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: Some(unrelated.clone()),
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::UnsafeRebaseBase { ref base, .. }) if *base == unrelated),
        "expected an unsafe-base refusal, got {result:?}"
    );
}

#[test]
fn dotted_branch_names_do_not_imply_a_parent() {
    let repo = TestRepo::new();
    repo.create_branch("jack/a.b");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    let plan = plan_rebase(repo.path(), None, None).expect("plan rebase");
    assert_eq!(plan.base_ref, "origin/main");
    assert_eq!(plan.class, RebaseClass::CleanAuthored);
    assert_eq!(plan.strategy, RebaseStrategy::DirectRebase);
}

#[test]
fn explicit_onto_is_the_only_alternate_base() {
    let repo = TestRepo::new();
    repo.create_branch("alternate");
    repo.create_file("alternate.txt", "alternate");
    repo.stage_all();
    repo.commit("alternate work");
    repo.create_branch("feature");

    let plan = plan_rebase(repo.path(), Some("alternate"), None).expect("plan rebase");
    assert_eq!(plan.base_ref, "alternate");
}

#[test]
fn dirty_scratch_only_branch_resets_to_base() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("scratch/design.md", "notes");

    let plan = plan_rebase(repo.path(), None, None).expect("plan rebase");
    assert_eq!(plan.class, RebaseClass::ScratchOnly);
    assert_eq!(plan.strategy, RebaseStrategy::ResetToBase);
    assert_eq!(plan.unique_commits, 0);
    assert!(!plan.protected);
}

#[test]
fn modified_scratch_file_keeps_its_leading_path_character() {
    let repo = TestRepo::new();
    repo.create_file("scratch/notes.md", "original\n");
    repo.stage_all();
    repo.commit("add scratch notes");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("scratch/notes.md", "evolved\n");

    let plan = plan_rebase(repo.path(), None, None).expect("plan rebase");
    assert_eq!(plan.class, RebaseClass::ScratchOnly);
    assert!(plan
        .changed_files
        .iter()
        .all(|path| path.starts_with("scratch")));
}

#[test]
fn wave_changes_are_protected() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("wave/goals/MEMORY.md", "state");
    repo.stage_all();
    repo.commit("update wave memory");

    let plan = plan_rebase(repo.path(), None, None).expect("plan rebase");
    assert_eq!(plan.class, RebaseClass::Protected);
    assert_eq!(plan.strategy, RebaseStrategy::DirectRebase);
    assert!(plan.protected);
}
