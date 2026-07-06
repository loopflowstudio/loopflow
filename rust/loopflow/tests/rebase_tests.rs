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
        "git {:?} failed: {}",
        args,
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
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::RebaseConflict { ref onto, .. }) if onto == "origin/main"),
        "expected rebase conflict error, got: {:?}",
        result
    );
}

#[test]
fn rebase_with_push() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.push_new_branch("feature");
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
            push: true,
        },
        &NullProgress,
    )
    .expect("rebase");
}

#[test]
fn rebase_stacked_branch_after_squash_merge() {
    let repo = TestRepo::new();

    // Branch A: two commits on top of main.
    repo.create_branch("branch-a");
    repo.create_file("a1.txt", "a1");
    repo.stage_all();
    repo.commit("a1");
    repo.create_file("a2.txt", "a2");
    repo.stage_all();
    repo.commit("a2");

    // Branch B: stacked on A with its own commits.
    repo.create_branch("branch-b");
    repo.create_file("b1.txt", "b1");
    repo.stage_all();
    repo.commit("b1");

    let b_head = repo.head_sha();

    // Simulate squash-merge of A into main.
    repo.checkout("main");
    git(repo.path(), &["merge", "--squash", "branch-a"]);
    git(repo.path(), &["commit", "-m", "squash merge branch-a"]);
    repo.push();

    // Rebase B onto origin/main — without fork-point detection this would conflict.
    repo.checkout("branch-b");
    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    )
    .expect("rebase of stacked branch after squash-merge should succeed");

    // B's commit (b1.txt) should be present.
    assert!(
        repo.path().join("b1.txt").exists(),
        "b1.txt should exist after rebase"
    );
    // A's changes should also be present (from the squash merge on main).
    assert!(
        repo.path().join("a1.txt").exists(),
        "a1.txt should exist from squash merge"
    );
    // HEAD should have changed (rebased onto new main).
    assert_ne!(repo.head_sha(), b_head, "HEAD should differ after rebase");
}

#[test]
fn rebase_reparents_child_onto_main_when_parent_merged() {
    // Parent `a.b` is squash-merged into main, but its local ref lingers at the
    // pre-squash tip (a squash-merge deletes origin/a.b, not the local branch).
    // The child `a.b.c` must re-parent onto main and carry ONLY its own change,
    // not the parent's already-merged commits.
    let repo = TestRepo::new();

    repo.create_branch("a.b");
    repo.create_file("p1.txt", "p1");
    repo.stage_all();
    repo.commit("p1");
    repo.create_file("p2.txt", "p2");
    repo.stage_all();
    repo.commit("p2");

    repo.create_branch("a.b.c");
    repo.create_file("child.txt", "child");
    repo.stage_all();
    repo.commit("child work");

    // Squash-merge the parent's two commits into main as a single commit.
    repo.checkout("main");
    git(repo.path(), &["merge", "--squash", "a.b"]);
    git(repo.path(), &["commit", "-m", "squash merge a.b"]);
    repo.push();
    // Local `a.b` ref is left dangling at its pre-squash tip.

    repo.checkout("a.b.c");
    let plan = plan_rebase(repo.path(), None).expect("plan rebase");
    // A merged parent is a dead base: re-parent onto the default branch.
    assert_eq!(plan.base_ref, "origin/main");
    assert!(
        plan.fork_point.is_some(),
        "should fork off the merged parent"
    );
    assert_eq!(plan.merged_parent.as_deref(), Some("a.b"));

    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    )
    .expect("re-parent onto main should succeed");

    // The child's own change is present; the parent's merged content is present
    // via main.
    assert!(repo.path().join("child.txt").exists());
    assert!(repo.path().join("p1.txt").exists());

    // The child carries ONLY its own change relative to main — not the parent's
    // now-merged commits.
    let diff = git(repo.path(), &["diff", "--name-only", "origin/main...HEAD"]);
    assert_eq!(diff, "child.txt", "child diff vs main is only its own file");
    let commits_beyond = git(repo.path(), &["rev-list", "--count", "origin/main..HEAD"]);
    assert_eq!(
        commits_beyond, "1",
        "only the child's commit sits above main"
    );

    // The merged parent's lingering local ref was pruned.
    let branches = git(repo.path(), &["branch", "--list", "a.b"]);
    assert!(branches.is_empty(), "merged local parent should be pruned");
}

#[test]
fn rebase_reparents_child_when_reworked_parent_branch_deleted() {
    // Parent `a.b` lands in a REWORKED form: main's content diverges from what
    // the child stacked on, so no content-based merge check matches. The signal
    // is purely "origin/a.b was deleted on merge" while local `a.b` lingers. The
    // child (which touches unrelated files) must still re-parent onto main and
    // carry ONLY its own change — never the stale parent's diverged content.
    let repo = TestRepo::new();

    repo.create_branch("a.b");
    repo.create_file("feature.txt", "v1\n");
    repo.stage_all();
    repo.commit("feature v1");
    repo.push_new_branch("a.b");

    repo.create_branch("a.b.c");
    repo.create_file("child.txt", "child\n");
    repo.stage_all();
    repo.commit("child work");

    // Reworked land: main gets a divergent feature.txt = v2; origin/a.b deleted.
    repo.checkout("main");
    repo.create_file("feature.txt", "v2\n");
    repo.stage_all();
    repo.commit("feature v2 (reworked in review)");
    repo.push();
    git(repo.path(), &["push", "origin", "--delete", "a.b"]);

    repo.checkout("a.b.c");
    let plan = plan_rebase(repo.path(), None).expect("plan rebase");
    assert_eq!(
        plan.base_ref, "origin/main",
        "reworked parent re-parents onto main, not the stale local a.b"
    );
    assert!(plan.fork_point.is_some());
    assert_eq!(plan.merged_parent.as_deref(), Some("a.b"));

    rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    )
    .expect("clean child re-parents cleanly onto reworked main");

    let diff = git(repo.path(), &["diff", "--name-only", "origin/main...HEAD"]);
    assert_eq!(diff, "child.txt", "child carries only its own change");
    let feature = std::fs::read_to_string(repo.path().join("feature.txt")).expect("read feature");
    assert_eq!(
        feature, "v2\n",
        "child inherits main's reworked content, not the stale parent's v1"
    );
    let branches = git(repo.path(), &["branch", "--list", "a.b"]);
    assert!(branches.is_empty(), "stale local parent should be pruned");
}

#[test]
fn rebase_surfaces_conflict_when_reworked_parent_overlaps_child() {
    // Same reworked-parent land, but now the child's own commit touches the same
    // lines the rework diverged on. Replaying the child onto main must SURFACE a
    // conflict — the correct outcome — not silently rebase onto the dead parent.
    let repo = TestRepo::new();

    repo.create_branch("a.b");
    repo.create_file("feature.txt", "v1\n");
    repo.stage_all();
    repo.commit("feature v1");
    repo.push_new_branch("a.b");

    repo.create_branch("a.b.c");
    repo.create_file("feature.txt", "v1\nchild addition\n");
    repo.stage_all();
    repo.commit("child extends feature");

    repo.checkout("main");
    repo.create_file("feature.txt", "v2 reworked\n");
    repo.stage_all();
    repo.commit("feature v2");
    repo.push();
    git(repo.path(), &["push", "origin", "--delete", "a.b"]);

    repo.checkout("a.b.c");
    let result = rebase_with_recovery(
        repo.path(),
        &RebaseOptions {
            onto: "origin/main".to_string(),
            push: false,
        },
        &NullProgress,
    );
    assert!(
        matches!(result, Err(OpsError::RebaseConflict { ref onto, .. }) if onto == "origin/main"),
        "reworked parent overlapping the child must surface a conflict, got: {result:?}"
    );
}

#[test]
fn plan_rebase_classifies_dirty_scratch_only_branch_as_reset() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("scratch/design.md", "notes");

    let plan = plan_rebase(repo.path(), None).expect("plan rebase");

    assert_eq!(plan.class, RebaseClass::ScratchOnly);
    assert_eq!(plan.strategy, RebaseStrategy::ResetToBase);
    assert_eq!(plan.unique_commits, 0);
    assert!(!plan.protected);
    assert!(plan
        .changed_files
        .iter()
        .all(|path| path.starts_with("scratch")));
}

#[test]
fn plan_rebase_handles_modified_scratch_file_with_leading_status() {
    // Regression: a working-tree-modified (unstaged) file produces a porcelain
    // line with a leading status column (" M scratch/notes.md"). Trimming the
    // first line's leading space shifts the fixed path offset and drops the
    // path's first character ("cratch/notes.md"), which no longer starts with
    // "scratch" — misclassifying the branch as clean_authored → direct_rebase.
    let repo = TestRepo::new();
    repo.create_file("scratch/notes.md", "original\n");
    repo.stage_all();
    repo.commit("add scratch notes");
    repo.push();

    repo.create_branch("feature");
    // Modify the tracked scratch file WITHOUT staging -> " M" porcelain status.
    repo.create_file("scratch/notes.md", "evolved working notes\n");

    let plan = plan_rebase(repo.path(), None).expect("plan rebase");

    assert_eq!(plan.class, RebaseClass::ScratchOnly);
    assert_eq!(plan.strategy, RebaseStrategy::ResetToBase);
    assert!(
        plan.changed_files
            .iter()
            .all(|path| path.starts_with("scratch")),
        "changed_files should all be under scratch/, got: {:?}",
        plan.changed_files
    );
}

#[test]
fn plan_rebase_ignores_upstream_changes_when_branch_is_only_behind() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.checkout("main");
    repo.create_file("main.txt", "main");
    repo.stage_all();
    repo.commit("main change");
    repo.checkout("feature");
    repo.create_file("scratch/design.md", "notes");

    let plan = plan_rebase(repo.path(), Some("main")).expect("plan rebase");

    assert_eq!(plan.class, RebaseClass::ScratchOnly);
    assert_eq!(plan.strategy, RebaseStrategy::ResetToBase);
    assert!(plan
        .changed_files
        .iter()
        .all(|path| path.starts_with("scratch")));
}

#[test]
fn plan_rebase_classifies_wave_changes_as_protected() {
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("wave/goals/MEMORY.md", "state");
    repo.stage_all();
    repo.commit("update wave memory");

    let plan = plan_rebase(repo.path(), None).expect("plan rebase");

    assert_eq!(plan.class, RebaseClass::Protected);
    assert_eq!(plan.strategy, RebaseStrategy::DirectRebase);
    assert!(plan.protected);
}

#[test]
fn plan_rebase_uses_open_stack_parent() {
    let repo = TestRepo::new();
    // Stacks are author-scoped: child jack/a.b rebases onto parent jack/a.
    repo.create_branch("jack/a");
    repo.create_file("a.txt", "a");
    repo.stage_all();
    repo.commit("a");
    // A genuinely-open parent keeps its branch on origin (pushed with a PR).
    repo.push_new_branch("jack/a");
    repo.create_branch("jack/a.b");

    let plan = plan_rebase(repo.path(), None).expect("plan rebase");

    assert_eq!(plan.stack_parent.as_deref(), Some("jack/a"));
    assert_eq!(plan.base_ref, "jack/a");
    assert_eq!(plan.class, RebaseClass::StackParentOpen);
    assert_eq!(plan.strategy, RebaseStrategy::RebaseOntoParent);
    // An open parent is not a merge: the child keeps stacking on it, unchanged.
    assert!(plan.fork_point.is_none());
    assert!(plan.merged_parent.is_none());
}
