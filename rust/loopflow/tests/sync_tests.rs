mod support;

use loopflow::ops::{
    continue_sync_for_resolution, plan_sync, recover_sync, sync_with_recovery, NullProgress,
    OpsError, SyncClass, SyncOptions, SyncRecovery, SyncStrategy,
};
use loopflow::work::task::{GithubPr, PrPublication};
use loopflow_test_support::TestRepo;
use std::process::Command;
use support::EnvGuard;

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

fn start_conflicting_recovery(repo: &TestRepo) -> SyncRecovery {
    let error = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect_err("fixture must conflict");
    match error {
        OpsError::SyncConflict {
            recovery: Some(recovery),
            ..
        } => *recovery,
        other => panic!("expected owned recovery, got {other:?}"),
    }
}

#[test]
fn checkout_restoration_preserves_resolver_notes_index_and_retry() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("edits.txt", "base\n");
    repo.stage_all();
    repo.commit("base");
    repo.create_file("edits.txt", "staged\n");
    repo.stage_all();
    repo.create_file("edits.txt", "unstaged\n");
    repo.create_file("scratch/questions.md", "original\n");
    repo.create_file("scratch/plan.md", "plan\n");
    let staged = git(repo.path(), &["diff", "--cached"]);
    let unstaged = git(repo.path(), &["diff"]);

    for attempt in 1..=2 {
        let result = loopflow::ops::checkout::with_preserved_edits(repo.path(), || {
            repo.create_file("scratch/questions.md", &format!("resolver {attempt}\n"));
            if attempt == 1 {
                Err(OpsError::Message("update failed after resolution".into()))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.is_ok(), attempt == 2);
        assert_eq!(git(repo.path(), &["diff", "--cached"]), staged);
        assert_eq!(git(repo.path(), &["diff"]), unstaged);
        assert_eq!(
            std::fs::read_to_string(repo.path().join("scratch/questions.md")).unwrap(),
            "original\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.path().join("scratch/plan.md")).unwrap(),
            "plan\n"
        );
        assert!(git(repo.path(), &["stash", "list"]).is_empty());
    }
    for attempt in 1..=2 {
        assert_eq!(
            std::fs::read_to_string(
                repo.path()
                    .join(format!("scratch/questions.md.lf-sync-{attempt}"))
            )
            .unwrap(),
            format!("resolver {attempt}\n")
        );
    }
}

#[test]
fn checkout_restoration_reserves_stashed_sidecar_directories() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("scratch/question.md", "original\n");
    repo.create_file("scratch/question.md.lf-sync-1/note.md", "prior note\n");
    loopflow::ops::checkout::with_preserved_edits(repo.path(), || {
        repo.create_file("scratch/question.md", "resolver\n");
        Ok(())
    })
    .unwrap();
    for (path, expected) in [
        ("scratch/question.md", "original\n"),
        ("scratch/question.md.lf-sync-1/note.md", "prior note\n"),
        ("scratch/question.md.lf-sync-2", "resolver\n"),
    ] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(path)).unwrap(),
            expected
        );
    }
    assert!(git(repo.path(), &["stash", "list"]).is_empty());
}

#[test]
fn resolved_sync_restores_original_scratch_through_checkout_boundary() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    repo.create_file("scratch/questions.md", "original question\n");
    loopflow::ops::checkout::with_preserved_edits(repo.path(), || {
        start_conflicting_recovery(&repo);
        repo.create_file("scratch/questions.md", "resolver finding\n");
        repo.create_file("conflict.txt", "resolved\n");
        git(repo.path(), &["add", "conflict.txt"]);
        continue_sync_for_resolution(repo.path(), false)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(git(repo.path(), &["show", "HEAD:conflict.txt"]), "resolved");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("scratch/questions.md")).unwrap(),
        "original question\n"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("scratch/questions.md.lf-sync-1")).unwrap(),
        "resolver finding\n"
    );
    assert!(git(repo.path(), &["stash", "list"]).is_empty());
}

#[test]
fn sync_onto_main_succeeds() {
    let _env = EnvGuard::new(&[]);
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
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect("sync");
}

#[test]
fn sync_publishes_new_existing_and_deleted_remote_branches() {
    let _env = EnvGuard::new(&[]);
    for remote_state in ["new", "existing", "deleted"] {
        let repo = TestRepo::new();
        repo.create_branch("feature");
        repo.create_file("feature.txt", "feature\n");
        repo.stage_all();
        repo.commit("feature work");
        let original_head = repo.head_sha();
        if remote_state != "new" {
            repo.push_new_branch("feature");
        }
        if remote_state == "deleted" {
            git(
                repo.bare_path(),
                &["update-ref", "-d", "refs/heads/feature"],
            );
            assert_eq!(
                git(repo.path(), &["rev-parse", "origin/feature"]),
                original_head
            );
        }
        repo.checkout("main");
        repo.create_file("main.txt", "main\n");
        repo.stage_all();
        repo.commit("main work");
        repo.push();
        repo.checkout("feature");

        let result = sync_with_recovery(
            repo.path(),
            &SyncOptions {
                onto: "origin/main".to_string(),
                push: true,
                fork_base: None,
            },
            &NullProgress,
        )
        .unwrap_or_else(|error| panic!("{remote_state}: {error}"));

        assert_ne!(result.head, original_head);
        assert_eq!(
            git(repo.bare_path(), &["rev-parse", "refs/heads/feature"]),
            result.head
        );
        assert_eq!(git(repo.path(), &["rev-parse", "@{upstream}"]), result.head);
        assert_eq!(
            git(repo.bare_path(), &["show", "feature:feature.txt"]),
            "feature"
        );
        assert_eq!(git(repo.bare_path(), &["show", "feature:main.txt"]), "main");
    }
}

#[test]
fn sync_preserves_unseen_remote_work() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature\n");
    repo.stage_all();
    repo.commit("feature work");
    repo.push_new_branch("feature");
    let published_head = repo.head_sha();
    repo.create_file("remote.txt", "concurrent work\n");
    repo.stage_all();
    repo.commit("concurrent work");
    repo.push();
    let remote_head = repo.head_sha();
    git(repo.path(), &["reset", "--hard", &published_head]);
    git(
        repo.path(),
        &["update-ref", "refs/remotes/origin/feature", &published_head],
    );
    repo.create_file("local.txt", "local follow-up\n");
    repo.stage_all();
    repo.commit("local follow-up");

    let result = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: true,
            fork_base: None,
        },
        &NullProgress,
    );

    assert!(result
        .expect_err("reject unseen work")
        .to_string()
        .contains("stale info"));
    assert_eq!(
        git(repo.bare_path(), &["rev-parse", "refs/heads/feature"]),
        remote_head
    );
    assert_eq!(
        git(repo.path(), &["show", "HEAD:local.txt"]),
        "local follow-up"
    );
}

#[cfg(unix)]
#[test]
fn sync_preserves_branch_recreated_during_push() {
    let _env = EnvGuard::new(&[]);
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "published work\n");
    repo.stage_all();
    repo.commit("published work");
    repo.push_new_branch("feature");
    let published_head = repo.head_sha();
    git(
        repo.bare_path(),
        &["update-ref", "-d", "refs/heads/feature"],
    );
    repo.create_file("local.txt", "local follow-up\n");
    repo.stage_all();
    repo.commit("local follow-up");
    // Restore the remote after the absence observation, before Git sends updates.
    repo.create_file(".git/hooks/pre-push", "#!/bin/sh\ngit --git-dir=\"$(git remote get-url origin)\" update-ref refs/heads/feature \"$(git rev-parse origin/feature)\"\n");
    std::fs::set_permissions(
        repo.path().join(".git/hooks/pre-push"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    let result = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: true,
            fork_base: None,
        },
        &NullProgress,
    );

    assert!(result
        .expect_err("reject concurrent recreation")
        .to_string()
        .contains("reference already exists"));
    assert_eq!(
        git(repo.bare_path(), &["rev-parse", "refs/heads/feature"]),
        published_head
    );
    assert_eq!(
        git(repo.path(), &["show", "HEAD:local.txt"]),
        "local follow-up"
    );
}

#[test]
fn sync_conflict_returns_error() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let result = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::SyncConflict { ref onto, .. }) if onto == "origin/main"),
        "expected sync conflict, got {result:?}"
    );
    assert_eq!(
        loopflow::engine::git::intervention_state(repo.path()).unwrap(),
        Some("merge"),
        "the owned conflict must remain available to the recovery child"
    );
}

#[test]
fn second_identical_conflict_reuses_resolution_without_recovery() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let original_head = repo.head_sha();
    let recovery = start_conflicting_recovery(&repo);
    recover_sync(recovery, |_context| {
        repo.create_file("conflict.txt", "reviewed resolution\n");
        let result = loopflow::engine::git::continue_merge(repo.path(), None)?;
        assert!(result.success, "the reviewed resolution must complete");
        Ok(())
    })
    .expect("record first reviewed resolution");

    git(repo.path(), &["reset", "--hard", &original_head]);
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
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
fn preexisting_sync_is_refused_without_abort_or_head_movement() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let output = Command::new("git")
        .args(["merge", "origin/main"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(!output.status.success(), "fixture must stop on a conflict");
    let conflicted_head = git(repo.path(), &["rev-parse", "HEAD"]);

    let result = sync_with_recovery(
        repo.path(),
        &SyncOptions {
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
        Some("merge")
    );
}

#[test]
fn zero_exit_recovery_is_rejected_while_sequencer_remains() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, |_context| Ok(()));

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("still reports an active merge")),
        "expected a postcondition failure, got {result:?}"
    );
    assert_eq!(
        loopflow::engine::git::intervention_state(repo.path()).unwrap(),
        Some("merge")
    );
}

#[test]
fn recovery_abort_cannot_masquerade_as_success() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, |_context| {
        git(repo.path(), &["merge", "--abort"]);
        Ok(())
    });

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("pinned target") && message.contains("not an ancestor")),
        "expected target ancestry failure, got {result:?}"
    );
}

#[test]
fn recovery_on_wrong_branch_cannot_masquerade_as_success() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, |_context| {
        git(repo.path(), &["merge", "--abort"]);
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
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, |_context| {
        git(repo.path(), &["merge", "--abort"]);
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
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, |_context| {
        repo.create_file("conflict.txt", "resolved\n");
        git(repo.path(), &["add", "conflict.txt"]);
        git(
            repo.path(),
            &["-c", "core.editor=true", "commit", "--no-edit"],
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
fn stale_owned_sync_can_be_explicitly_continued() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let result = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );
    assert!(matches!(result, Err(OpsError::SyncConflict { .. })));
    drop(result);

    repo.create_file("conflict.txt", "main and feature\n");
    continue_sync_for_resolution(repo.path(), false).expect("adopt and continue stale sync");

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
fn linked_worktrees_own_syncs_independently() {
    let _env = EnvGuard::new(&[]);
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

    let first = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );
    let second_result = sync_with_recovery(
        &second,
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );

    assert!(matches!(first, Err(OpsError::SyncConflict { .. })));
    assert!(matches!(second_result, Err(OpsError::SyncConflict { .. })));
}

#[test]
fn sync_after_squash_merge_leaves_only_unique_diff() {
    let _env = EnvGuard::new(&[]);
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
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .expect("sync after squash merge");

    assert!(repo.path().join("feature.txt").exists());
    assert_eq!(
        git(repo.path(), &["diff", "--name-only", "origin/main...HEAD"]),
        "feature.txt"
    );
}

#[test]
fn existing_root_child_syncs_parent_from_its_original_fork() {
    let repo = TestRepo::new();
    let original_fork = repo.head_sha();
    repo.create_branch("child");
    repo.create_file("child.txt", "authored before stacking");
    repo.stage_all();
    repo.commit("Child work before selecting a parent");
    repo.checkout("main");
    repo.create_branch("parent");
    repo.create_file("parent.txt", "parent work");
    repo.stage_all();
    repo.commit("Parent work");
    repo.push_new_branch("parent");
    let parent_head = repo.head_sha();
    repo.checkout("child");

    let verification = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/parent".into(),
            push: false,
            fork_base: Some(original_fork),
        },
        &NullProgress,
    )
    .expect("adopt the selected parent without losing child work");
    assert_eq!(verification.target_sha, parent_head);
    assert_eq!(
        git(
            repo.path(),
            &["diff", "--name-only", "origin/parent...HEAD"]
        ),
        "child.txt"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("child.txt")).unwrap(),
        "authored before stacking"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("parent.txt")).unwrap(),
        "parent work"
    );
    assert_eq!(git(repo.path(), &["rev-parse", "parent"]), parent_head);
}

#[test]
fn stacked_child_merges_main_after_parent_squash() {
    let _env = EnvGuard::new(&[]);
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
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: Some(parent_tip),
        },
        &NullProgress,
    )
    .expect("stacked child merges main");

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
fn stacked_sync_refuses_when_base_is_not_an_ancestor() {
    let _env = EnvGuard::new(&[]);
    // A fork base that is not an ancestor of HEAD means the child's own history
    // was rewritten; that base cannot compare the child and target trees.
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

    let result = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".to_string(),
            push: false,
            fork_base: Some(unrelated.clone()),
        },
        &NullProgress,
    );

    assert!(
        matches!(result, Err(OpsError::UnsafeSyncBase { ref base, .. }) if *base == unrelated),
        "expected an unsafe-base refusal, got {result:?}"
    );
}

#[test]
fn dotted_branch_names_do_not_imply_a_parent() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("jack/a.b");
    repo.create_file("feature.txt", "feature");
    repo.stage_all();
    repo.commit("feature work");

    let plan = plan_sync(repo.path(), None, None).expect("plan sync");
    assert_eq!(plan.base_ref, "origin/main");
    assert_eq!(plan.class, SyncClass::CleanAuthored);
    assert_eq!(plan.strategy, SyncStrategy::MergeTarget);
}

#[test]
fn explicit_onto_is_the_only_alternate_base() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("alternate");
    repo.create_file("alternate.txt", "alternate");
    repo.stage_all();
    repo.commit("alternate work");
    repo.create_branch("feature");

    let plan = plan_sync(repo.path(), Some("alternate"), None).expect("plan sync");
    assert_eq!(plan.base_ref, "alternate");
}

#[test]
fn dirty_scratch_only_branch_resets_to_base() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("scratch/design.md", "notes");

    let plan = plan_sync(repo.path(), None, None).expect("plan sync");
    assert_eq!(plan.class, SyncClass::ScratchOnly);
    assert_eq!(plan.strategy, SyncStrategy::ResetToBase);
    assert_eq!(plan.unique_commits, 0);
}

#[test]
fn modified_scratch_file_keeps_its_leading_path_character() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("scratch/notes.md", "original\n");
    repo.stage_all();
    repo.commit("add scratch notes");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("scratch/notes.md", "evolved\n");

    let plan = plan_sync(repo.path(), None, None).expect("plan sync");
    assert_eq!(plan.class, SyncClass::ScratchOnly);
    assert!(plan
        .changed_files
        .iter()
        .all(|path| path.starts_with("scratch")));
}

#[test]
fn wave_changes_are_protected() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("wave/goals/MEMORY.md", "state");
    repo.stage_all();
    repo.commit("update wave memory");

    let plan = plan_sync(repo.path(), None, None).expect("plan sync");
    assert_eq!(plan.class, SyncClass::Protected);
    assert_eq!(plan.strategy, SyncStrategy::MergeTarget);
}

#[test]
fn long_branch_resolves_one_merge_and_keeps_the_resolution() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    for number in 1..190 {
        repo.create_file("conflict.txt", &format!("feature revision {number}\n"));
        repo.stage_all();
        repo.commit(&format!("revision {number}"));
    }
    let original = repo.head_sha();
    let recovery = start_conflicting_recovery(&repo);
    recover_sync(recovery, |_| {
        assert_eq!(
            repo.head_sha(),
            original,
            "merge never detaches or replays HEAD"
        );
        repo.create_file("conflict.txt", "reviewed final resolution\n");
        assert!(loopflow::engine::git::continue_merge(repo.path(), None)?.success);
        Ok(())
    })
    .unwrap();
    let resolved = repo.head_sha();
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD^1"]), original);
    assert_eq!(
        git(repo.path(), &["rev-list", "--count", "origin/main..HEAD"]),
        "191"
    );
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(repo.head_sha(), resolved);
    repo.checkout("main");
    repo.create_file("later.txt", "independent upstream update\n");
    repo.stage_all();
    repo.commit("advance main");
    repo.push();
    repo.checkout("feature");
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("conflict.txt")).unwrap(),
        "reviewed final resolution\n"
    );
}

#[test]
fn stacked_scratch_stays_with_child_across_parent_updates() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("parent");
    repo.create_file("scratch/design.md", "parent design\n");
    repo.stage_all();
    repo.commit("Parent notes");
    let mut fork = repo.head_sha();
    repo.create_branch("child");
    git(repo.path(), &["rm", "-r", "scratch"]);
    repo.commit("Clear inherited scratch");
    let first_child_commit = repo.head_sha();

    for revision in 1..=3 {
        repo.checkout("parent");
        repo.create_file(
            "scratch/design.md",
            &format!("parent revision {revision}\n"),
        );
        if revision == 2 {
            git(repo.path(), &["mv", "scratch/new.md", "scratch/renamed.md"]);
        } else {
            repo.create_file("scratch/new.md", &format!("parent addition {revision}\n"));
        }
        repo.stage_all();
        repo.commit("Parent updates notes");
        let target = repo.head_sha();
        repo.checkout("child");
        if revision > 1 {
            repo.create_file("scratch/local.md", "uncommitted child notes\n");
            repo.create_file("scratch/draft.md", "untracked child notes\n");
        }
        sync_with_recovery(
            repo.path(),
            &SyncOptions {
                onto: "parent".into(),
                push: false,
                fork_base: Some(fork),
            },
            &NullProgress,
        )
        .unwrap();
        assert!(!repo.path().join("scratch/design.md").exists());
        assert!(!repo.path().join("scratch/renamed.md").exists());
        assert_eq!(git(repo.path(), &["rev-parse", "HEAD^2"]), target);
        assert!(
            loopflow::engine::git::is_ancestor(repo.path(), &first_child_commit, "HEAD").unwrap()
        );
        if revision == 1 {
            assert!(!repo.path().join("scratch").exists());
            repo.create_file("scratch/new.md", "child's own notes\n");
            repo.create_file("scratch/local.md", "committed child notes\n");
            repo.stage_all();
            repo.commit("Child notes");
        } else {
            assert_eq!(
                std::fs::read_to_string(repo.path().join("scratch/new.md")).unwrap(),
                "child's own notes\n"
            );
            assert_eq!(
                std::fs::read_to_string(repo.path().join("scratch/local.md")).unwrap(),
                "uncommitted child notes\n"
            );
            assert_eq!(
                std::fs::read_to_string(repo.path().join("scratch/draft.md")).unwrap(),
                "untracked child notes\n"
            );
            repo.create_file("scratch/local.md", "committed child notes\n");
            std::fs::remove_file(repo.path().join("scratch/draft.md")).unwrap();
        }
        fork = target;
    }
    assert_eq!(
        git(repo.path(), &["show", "parent:scratch/design.md"]),
        "parent revision 3"
    );
}

#[test]
fn child_edits_survive_live_parent_sync_then_squash_landing() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("parent");
    repo.create_file("shared.txt", "parent first\n");
    repo.stage_all();
    repo.commit("parent first");
    repo.create_file("shared.txt", "parent final\n");
    repo.create_file("scratch/design.md", "parent design\n");
    repo.stage_all();
    repo.commit("parent final");
    let fork = repo.head_sha();
    repo.create_branch("child");
    git(repo.path(), &["rm", "-r", "scratch"]);
    repo.commit("Clear inherited scratch");
    repo.create_file("shared.txt", "child edits the parent line\n");
    repo.stage_all();
    repo.commit("child edits");
    let child = repo.head_sha();
    repo.checkout("parent");
    repo.create_file("parent-later.txt", "later parent work\n");
    repo.create_file("scratch/design.md", "updated parent design\n");
    repo.stage_all();
    repo.commit("parent advances");
    let parent = repo.head_sha();
    repo.checkout("child");
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "parent".into(),
            push: false,
            fork_base: Some(fork),
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD^1"]), child);
    repo.checkout("main");
    git(repo.path(), &["merge", "--squash", "parent"]);
    git(repo.path(), &["commit", "-m", "squash parent"]);
    repo.create_file("main-later.txt", "later main work\n");
    repo.stage_all();
    repo.commit("main advances");
    repo.push();
    let main = repo.head_sha();
    repo.checkout("child");
    let before = repo.head_sha();
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: Some(parent),
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(
        git(repo.path(), &["show", "-s", "--format=%P", "HEAD"]),
        format!("{before} {main}")
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("shared.txt")).unwrap(),
        "child edits the parent line\n"
    );
    assert_eq!(
        git(repo.path(), &["diff", "--name-only", "origin/main...HEAD"]),
        "scratch/design.md\nshared.txt"
    );
    assert!(!repo.path().join("scratch").exists());
}

#[test]
fn squash_parent_conflict_retains_real_target_and_can_abort_or_continue() {
    let _env = EnvGuard::new(&[]);
    for abort in [true, false] {
        let repo = TestRepo::new();
        repo.create_branch("parent");
        repo.create_file("shared.txt", "parent\n");
        repo.stage_all();
        repo.commit("parent");
        let fork = repo.head_sha();
        repo.create_branch("child");
        repo.create_file("shared.txt", "child\n");
        repo.stage_all();
        repo.commit("child");
        let child = repo.head_sha();
        repo.checkout("main");
        git(repo.path(), &["merge", "--squash", "parent"]);
        git(repo.path(), &["commit", "-m", "squash parent"]);
        repo.create_file("shared.txt", "later upstream edit\n");
        repo.stage_all();
        repo.commit("upstream changes parent line");
        repo.push();
        let target = repo.head_sha();
        repo.checkout("child");
        let result = sync_with_recovery(
            repo.path(),
            &SyncOptions {
                onto: "origin/main".into(),
                push: false,
                fork_base: Some(fork),
            },
            &NullProgress,
        );
        assert!(matches!(result, Err(OpsError::SyncConflict { .. })));
        drop(result);
        assert_eq!(git(repo.path(), &["rev-parse", "MERGE_HEAD"]), target);
        if abort {
            loopflow::ops::abort_sync_for_resolution(repo.path(), false).unwrap();
            assert_eq!(repo.head_sha(), child);
            assert_eq!(
                std::fs::read_to_string(repo.path().join("shared.txt")).unwrap(),
                "child\n"
            );
        } else {
            // Simulate owner death before the temporary comparison target was replaced.
            let directory = loopflow::engine::git::absolute_git_dir(repo.path()).unwrap();
            std::fs::write(directory.join("MERGE_HEAD"), format!("{child}\n")).unwrap();
            repo.create_file("shared.txt", "reviewed combined content\n");
            continue_sync_for_resolution(repo.path(), false).unwrap();
            assert_eq!(
                git(repo.path(), &["show", "-s", "--format=%P", "HEAD"]),
                format!("{child} {target}")
            );
        }
    }
}

#[test]
fn saved_flow_command_migrates_and_merges_through_the_cli_path() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "saved work\n");
    repo.stage_all();
    repo.commit("saved work");
    let original = repo.head_sha();
    repo.checkout("main");
    repo.create_file("main.txt", "upstream\n");
    repo.stage_all();
    repo.commit("upstream");
    repo.push();
    let target = repo.head_sha();
    repo.checkout("feature");
    let command: loopflow::engine::flow::Command =
        serde_json::from_str(r#"{"command":"rebase","args":["origin/main"]}"#).unwrap();
    loopflow::ops::execute_flow_command(repo.path(), &command, &NullProgress).unwrap();
    assert_eq!(
        git(repo.path(), &["show", "-s", "--format=%P", "HEAD"]),
        format!("{original} {target}")
    );
    assert_eq!(
        git(repo.path(), &["rev-parse", "origin/feature"]),
        repo.head_sha()
    );
    assert_eq!(
        serde_json::to_value(command).unwrap(),
        serde_json::json!({"command":"sync", "args":["origin/main"]})
    );
}

#[test]
fn saved_flow_sync_follows_task_parent_and_skips_an_already_contained_head() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let child_dir = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(
        &[
            (
                "codex",
                "#!/bin/sh\necho 'unexpected conflict agent' >&2\nexit 97\n",
            ),
            (
                "claude",
                "#!/bin/sh\necho 'unexpected conflict agent' >&2\nexit 97\n",
            ),
            (
                "opencode",
                "#!/bin/sh\necho 'unexpected conflict agent' >&2\nexit 97\n",
            ),
        ],
        home.path(),
    );
    repo.create_branch("parent");
    repo.create_file("shared.txt", "parent\n");
    repo.stage_all();
    repo.commit("Parent work");
    repo.push_new_branch("parent");
    let fork = repo.head_sha();
    let parent = support::register_task(home.path(), repo.path(), "parent", &fork);
    let child_path = child_dir.path().join("child");
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "child",
            child_path.to_str().unwrap(),
        ],
    );
    let child = support::register_sibling_task(&parent, "INF-124", "child", &child_path);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut parent_pr = parent.pr.clone();
    parent_pr.publication = Some(PrPublication {
        requested_at: parent_pr.created_at,
        presentation: None,
        github: Some(GithubPr {
            number: 41,
            url: "https://github.com/fixture/repo/pull/41".into(),
            head_sha: Some(parent_pr.base_commit.clone()),
        }),
        merge: None,
    });
    runtime
        .block_on(parent.store.update_task_pr(&parent_pr))
        .unwrap();
    let pr = runtime
        .block_on(parent.store.active_task_pr(&child.id))
        .unwrap()
        .unwrap();
    runtime
        .block_on(parent.store.stack_task_pr(&pr, &parent.pr.id))
        .unwrap();
    std::fs::write(child_path.join("shared.txt"), "child\n").unwrap();
    git(&child_path, &["commit", "-am", "Child work"]);
    let child_head = git(&child_path, &["rev-parse", "HEAD"]);

    repo.checkout("main");
    repo.create_file("shared.txt", "conflicting main\n");
    repo.stage_all();
    repo.commit("Main moves independently");
    repo.push();
    let main_head = repo.head_sha();
    let command = serde_json::from_str(r#"{"command":"sync","args":[]}"#).unwrap();
    let sync =
        || loopflow::ops::execute_flow_command(&child_path, &command, &NullProgress).unwrap();
    assert_eq!(
        plan_sync(&child_path, Some("origin/parent"), Some(fork.clone()))
            .unwrap()
            .strategy,
        SyncStrategy::Noop
    );
    sync();
    assert_eq!(git(&child_path, &["rev-parse", "HEAD"]), child_head);
    assert_eq!(
        git(&child_path, &["ls-remote", "--heads", "origin", "child"]),
        ""
    );
    assert_eq!(
        loopflow::engine::git::intervention_state(&child_path).unwrap(),
        None
    );

    repo.checkout("parent");
    repo.create_file("parent-update.txt", "later parent work\n");
    repo.stage_all();
    repo.commit("Parent advances");
    repo.push();
    let parent_head = repo.head_sha();
    sync();
    assert_eq!(
        git(&child_path, &["show", "-s", "--format=%P", "HEAD"]),
        format!("{child_head} {parent_head}")
    );
    assert!(!loopflow::engine::git::is_ancestor(&child_path, &main_head, "HEAD").unwrap());
    assert_eq!(
        std::fs::read_to_string(child_path.join("shared.txt")).unwrap(),
        "child\n"
    );
    let merged_head = git(&child_path, &["rev-parse", "HEAD"]);
    sync();
    assert_eq!(git(&child_path, &["rev-parse", "HEAD"]), merged_head);
    let saved = runtime
        .block_on(parent.store.active_task_pr(&child.id))
        .unwrap()
        .unwrap();
    assert_eq!(saved.base_commit, parent_head);
    assert_eq!(saved.parent_pr_id, Some(parent.pr.id));
}

#[test]
fn resident_refresh_preserves_local_plans_and_followup_after_merge() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let resident = loopflow::engine::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::engine::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    std::fs::write(resident.path.join("memory.md"), "accepted\n").unwrap();
    loopflow::ops::commit_selected(&resident.path, &["memory.md".into()], Some("Memory")).unwrap();
    git(repo.path(), &["merge", "--squash", &resident.branch]);
    repo.commit("Merged document PR");
    repo.push();
    std::fs::create_dir_all(resident.path.join("scratch")).unwrap();
    std::fs::write(resident.path.join("scratch/plan.md"), "private\n").unwrap();
    std::fs::write(resident.path.join("memory.md"), "followup\n").unwrap();
    sync_with_recovery(
        &resident.path,
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(resident.path.join("memory.md")).unwrap(),
        "followup\n"
    );
    assert_eq!(
        std::fs::read_to_string(resident.path.join("scratch/plan.md")).unwrap(),
        "private\n"
    );
    // Everything committed had landed, so the branch restarts from main
    // instead of carrying its squash-merged commit into the next PR.
    assert_eq!(
        git(&resident.path, &["rev-parse", "HEAD"]),
        git(&resident.path, &["rev-parse", "origin/main"])
    );
    loopflow::ops::commit_selected(&resident.path, &["memory.md".into()], Some("Followup"))
        .unwrap();
    assert!(git(
        &resident.path,
        &["diff", "origin/main...HEAD", "--", "memory.md"]
    )
    .contains("+followup"));
    assert_eq!(
        git(
            &resident.path,
            &["rev-list", "--count", "origin/main..HEAD"]
        )
        .trim(),
        "1"
    );
}

#[test]
fn resident_memory_conflict_keeps_private_scratch_and_recovery() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("memory.md", "base\n");
    repo.stage_all();
    repo.commit("base memory");
    repo.push();
    let resident = loopflow::engine::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::engine::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    std::fs::write(resident.path.join("memory.md"), "resident\n").unwrap();
    loopflow::ops::commit_selected(
        &resident.path,
        &["memory.md".into()],
        Some("Local decision"),
    )
    .unwrap();
    std::fs::create_dir_all(resident.path.join("scratch")).unwrap();
    std::fs::write(resident.path.join("scratch/private.md"), "private\n").unwrap();
    repo.create_file("memory.md", "upstream\n");
    repo.stage_all();
    repo.commit("competing decision");
    repo.push();
    let error = sync_with_recovery(
        &resident.path,
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        OpsError::SyncConflict {
            recovery: Some(_),
            ..
        }
    ));
    assert!(std::fs::read_to_string(resident.path.join("memory.md"))
        .unwrap()
        .contains("<<<<<<<"));
    assert_eq!(
        std::fs::read_to_string(resident.path.join("scratch/private.md")).unwrap(),
        "private\n"
    );
}
