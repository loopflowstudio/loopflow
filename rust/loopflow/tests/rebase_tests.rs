use loopflow::ops::{
    continue_rebase_for_resolution, plan_rebase, rebase_with_recovery, recover_rebase,
    NullProgress, OpsError, RebaseClass, RebaseOptions, RebaseRecovery, RebaseStrategy,
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

fn create_conflicting_repo() -> TestRepo {
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
    repo
}

fn start_conflicting_recovery(repo: &TestRepo) -> RebaseRecovery {
    let error = rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect_err("fixture must conflict");
    match error {
        OpsError::RebaseConflict {
            recovery: Some(recovery),
            ..
        } => *recovery,
        other => panic!("expected owned recovery, got {other:?}"),
    }
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
    let repo = create_conflicting_repo();
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
    assert_eq!(
        loopflow::engine::git::intervention_state(repo.path()).unwrap(),
        Some("rebase"),
        "the owned conflict must remain available to the recovery child"
    );
}

#[test]
fn second_identical_conflict_reuses_resolution_without_recovery() {
    let repo = create_conflicting_repo();
    let original_head = repo.head_sha();
    let recovery = start_conflicting_recovery(&repo);
    recover_rebase(recovery, |_context| {
        repo.create_file("conflict.txt", "reviewed resolution\n");
        let result = loopflow::engine::git::continue_rebase(repo.path())?;
        assert!(result.success, "the reviewed resolution must complete");
        Ok(())
    })
    .expect("record first reviewed resolution");

    git(repo.path(), &["reset", "--hard", &original_head]);
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect("rerere should finish the repeated conflict mechanically");

    assert_eq!(
        std::fs::read_to_string(repo.path().join("conflict.txt")).expect("resolved file"),
        "reviewed resolution\n"
    );
    assert_eq!(
        loopflow::engine::git::intervention_state(repo.path()).unwrap(),
        None
    );
}

#[test]
fn preexisting_rebase_is_refused_without_abort_or_head_movement() {
    let repo = create_conflicting_repo();
    let output = Command::new("git")
        .args(["rebase", "origin/main"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "fixture must stop on a conflict");
    let conflicted_head = git(repo.path(), &["rev-parse", "HEAD"]);

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
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("already exists")),
        "expected a preflight refusal, got {result:?}"
    );
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD"]), conflicted_head);
    assert_eq!(
        loopflow::engine::git::intervention_state(repo.path()).unwrap(),
        Some("rebase")
    );
}

#[test]
fn zero_exit_recovery_is_rejected_while_sequencer_remains() {
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_rebase(recovery, |_context| Ok(()));

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("still reports an active rebase")),
        "expected a postcondition failure, got {result:?}"
    );
    assert_eq!(
        loopflow::engine::git::intervention_state(repo.path()).unwrap(),
        Some("rebase")
    );
}

#[test]
fn recovery_abort_cannot_masquerade_as_success() {
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_rebase(recovery, |_context| {
        git(repo.path(), &["rebase", "--abort"]);
        Ok(())
    });

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("pinned target") && message.contains("not an ancestor")),
        "expected target ancestry failure, got {result:?}"
    );
}

#[test]
fn recovery_on_wrong_branch_cannot_masquerade_as_success() {
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_rebase(recovery, |_context| {
        git(repo.path(), &["rebase", "--abort"]);
        git(repo.path(), &["checkout", "main"]);
        Ok(())
    });

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("expected branch feature, found main")),
        "expected branch postcondition failure, got {result:?}"
    );
}

#[test]
fn recovery_with_detached_head_cannot_masquerade_as_success() {
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_rebase(recovery, |_context| {
        git(repo.path(), &["rebase", "--abort"]);
        git(repo.path(), &["checkout", "--detach"]);
        Ok(())
    });

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("HEAD is detached")),
        "expected detached-HEAD postcondition failure, got {result:?}"
    );
}

#[test]
fn recovery_with_new_tracked_dirt_cannot_masquerade_as_success() {
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_rebase(recovery, |_context| {
        repo.create_file("conflict.txt", "resolved\n");
        git(repo.path(), &["add", "conflict.txt"]);
        git(
            repo.path(),
            &["-c", "core.editor=true", "rebase", "--continue"],
        );
        repo.create_file("conflict.txt", "dirty after resolution\n");
        Ok(())
    });

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("new tracked dirty state")),
        "expected dirty-state postcondition failure, got {result:?}"
    );
}

#[test]
fn stale_owned_rebase_can_be_explicitly_continued() {
    let repo = create_conflicting_repo();
    let result = rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );
    assert!(matches!(result, Err(OpsError::RebaseConflict { .. })));
    drop(result);

    repo.create_file("conflict.txt", "main and feature\n");
    continue_rebase_for_resolution(repo.path(), false).expect("adopt and continue stale rebase");

    assert_eq!(
        loopflow::engine::git::intervention_state(repo.path()).unwrap(),
        None
    );
    assert_eq!(
        git(repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]),
        "feature"
    );
}

#[test]
fn linked_worktrees_own_rebases_independently() {
    let repo = TestRepo::new();
    repo.create_file("conflict.txt", "base\n");
    repo.stage_all();
    repo.commit("shared base");
    repo.push();
    repo.create_branch("feature-one");
    repo.create_file("conflict.txt", "feature one\n");
    repo.stage_all();
    repo.commit("feature one");
    repo.checkout("main");
    let second = repo.create_named_worktree("feature-two");
    std::fs::write(second.join("conflict.txt"), "feature two\n").unwrap();
    git(&second, &["add", "conflict.txt"]);
    git(&second, &["commit", "-m", "feature two"]);
    repo.create_file("conflict.txt", "main advance\n");
    repo.stage_all();
    repo.commit("main advance");
    repo.push();
    repo.checkout("feature-one");

    let first = rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );
    let second_result = rebase_with_recovery(
        &second,
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );

    assert!(matches!(first, Err(OpsError::RebaseConflict { .. })));
    assert!(matches!(
        second_result,
        Err(OpsError::RebaseConflict { .. })
    ));
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
}
