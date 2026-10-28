use crate::support;

use loopflow::ops::task::task_stack;
use loopflow::ops::{
    continue_sync_for_resolution, plan_sync, recover_sync, sync_with_plan, sync_with_recovery,
    NullProgress, OpsError, SyncClass, SyncOptions, SyncRecovery, SyncStrategy,
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
        loopflow::git::intervention_state(repo.path()).unwrap(),
        Some("merge"),
        "the owned conflict must remain for whoever continues it"
    );
}

#[test]
fn second_identical_conflict_reuses_resolution_without_recovery() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let original_head = repo.head_sha();
    let recovery = start_conflicting_recovery(&repo);
    recover_sync(recovery, || {
        repo.create_file("conflict.txt", "reviewed resolution\n");
        let result = loopflow::git::continue_merge(repo.path(), None)?;
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
        loopflow::git::intervention_state(repo.path()).unwrap(),
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
        loopflow::git::intervention_state(repo.path()).unwrap(),
        Some("merge")
    );
}

#[test]
fn zero_exit_recovery_is_rejected_while_sequencer_remains() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, || Ok(()));

    assert!(
        matches!(result, Err(OpsError::Message(ref message)) if message.contains("still reports an active merge")),
        "expected a postcondition failure, got {result:?}"
    );
    assert_eq!(
        loopflow::git::intervention_state(repo.path()).unwrap(),
        Some("merge")
    );
}

#[test]
fn recovery_abort_cannot_masquerade_as_success() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, || {
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

    let result = recover_sync(recovery, || {
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

    let result = recover_sync(recovery, || {
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
fn an_edit_beside_a_committed_resolution_leaves_the_sync_verified() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    let recovery = start_conflicting_recovery(&repo);

    let result = recover_sync(recovery, || {
        repo.create_file("conflict.txt", "resolved\n");
        git(repo.path(), &["add", "conflict.txt"]);
        git(
            repo.path(),
            &["-c", "core.editor=true", "commit", "--no-edit"],
        );
        repo.create_file("README.md", "another writer's edit\n");
        Ok(())
    });

    result.expect("another writer's edit does not fail the sync");
    assert_eq!(
        std::fs::read_to_string(repo.path().join("README.md")).unwrap(),
        "another writer's edit\n"
    );
}

#[test]
fn continue_completes_beside_existing_tracked_edits() {
    let _env = EnvGuard::new(&[]);
    let repo = create_conflicting_repo();
    repo.create_file("README.md", "edit in progress\n");
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
    continue_sync_for_resolution(repo.path(), false).expect("continue beside existing edits");

    assert_eq!(
        loopflow::git::intervention_state(repo.path()).unwrap(),
        None
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("README.md")).unwrap(),
        "edit in progress\n"
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
        loopflow::git::intervention_state(repo.path()).unwrap(),
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
fn stacked_child_targets_main_after_merged_parent_checkout_is_removed() {
    // A landed parent's removed checkout is ordinary state: the child plans
    // its sync onto the default branch without the parent's git dir.
    let home = tempfile::TempDir::new().unwrap();
    let repo = TestRepo::new();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let parent_path = repo.create_named_worktree("parent");
    let parent =
        support::register_task_with_pr(home.path(), &parent_path, "parent", &repo.head_sha());
    let child_path = repo.create_named_worktree("child");
    let child = support::register_sibling_task(&parent, "INF-124", "child", &child_path);
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
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(parent.store.update_task_pr(&parent_pr))
        .unwrap();
    runtime
        .block_on(parent.store.stack_task_placement(&child, &parent.pr.id))
        .unwrap();
    parent_pr.merge_commit = Some(repo.head_sha());
    runtime
        .block_on(parent.store.update_task_pr(&parent_pr))
        .unwrap();
    git(
        repo.path(),
        &[
            "worktree",
            "remove",
            "--force",
            parent_path.to_str().unwrap(),
        ],
    );

    let stacked = task_stack(&child_path)
        .expect("stack resolves without the parent checkout")
        .expect("child is stacked");
    assert_eq!(stacked.parent_branch, None);
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["sync", "--plan"])
        .current_dir(&child_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("base: main\n"), "{stdout}");
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
    let message = result.unwrap_err().to_string();
    assert!(message.lines().count() < 5, "{message}");
    assert!(!message.contains("rebase") && !message.contains("reset"));
    assert!(repo.path().join("child.txt").exists());
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
fn a_commit_after_planning_is_never_reset_away() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("main.txt", "main");
    repo.stage_all();
    repo.commit("main work");
    repo.push();
    repo.create_branch("feature");
    git(repo.path(), &["reset", "--hard", "HEAD^"]);
    let plan = plan_sync(repo.path(), Some("origin/main"), None).expect("plan sync");
    assert_eq!(plan.strategy, SyncStrategy::ResetToBase);

    repo.create_file("late.txt", "late");
    repo.stage_all();
    repo.commit("landed while the fetch ran");
    let head = git(repo.path(), &["rev-parse", "HEAD"]);

    let options = SyncOptions {
        onto: "origin/main".to_string(),
        push: false,
        fork_base: None,
    };
    assert!(sync_with_plan(repo.path(), &options, &plan, &NullProgress).is_err());
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD"]), head);
}

#[test]
fn untracked_scratch_is_not_assumed_to_be_disposable() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("scratch/design.md", "notes");

    let plan = plan_sync(repo.path(), None, None).expect("plan sync");
    assert_eq!(plan.class, SyncClass::CleanAuthored);
    assert_eq!(plan.strategy, SyncStrategy::MergeTarget);
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
    assert_eq!(plan.class, SyncClass::CleanAuthored);
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
    repo.create_file(".lf/wave/goals/MEMORY.md", "state");
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
    recover_sync(recovery, || {
        assert_eq!(
            repo.head_sha(),
            original,
            "merge never detaches or replays HEAD"
        );
        repo.create_file("conflict.txt", "reviewed final resolution\n");
        assert!(loopflow::git::continue_merge(repo.path(), None)?.success);
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
        "shared.txt"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("scratch/design.md")).unwrap(),
        "updated parent design\n"
    );
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
            let directory = loopflow::git::absolute_git_dir(repo.path()).unwrap();
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

/// Run an operation step the way a Graph process does: as its own `lf` command.
fn run_graph_command(repo: &std::path::Path, command: &loopflow::graph::Command) {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(&command.argv()[1..])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn saved_graph_command_migrates_and_merges_through_the_cli_path() {
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
    let command: loopflow::graph::Command =
        serde_json::from_str(r#"{"command":"rebase","args":["origin/main"]}"#).unwrap();
    run_graph_command(repo.path(), &command);
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
fn saved_graph_sync_follows_task_parent_and_skips_an_already_contained_head() {
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
    let parent = support::register_task_with_pr(home.path(), repo.path(), "parent", &fork);
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
    runtime
        .block_on(parent.store.stack_task_placement(&child, &parent.pr.id))
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
    let sync = || run_graph_command(&child_path, &command);
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
        loopflow::git::intervention_state(&child_path).unwrap(),
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
    assert!(!loopflow::git::is_ancestor(&child_path, &main_head, "HEAD").unwrap());
    assert_eq!(
        std::fs::read_to_string(child_path.join("shared.txt")).unwrap(),
        "child\n"
    );
    let merged_head = git(&child_path, &["rev-parse", "HEAD"]);
    sync();
    assert_eq!(git(&child_path, &["rev-parse", "HEAD"]), merged_head);
    let saved = runtime
        .block_on(parent.store.get_task(&child.id))
        .unwrap()
        .unwrap();
    assert_eq!(saved.base_commit, parent_head);
    assert_eq!(saved.parent_pr_id, Some(parent.pr.id));
    assert!(
        runtime
            .block_on(parent.store.active_task_pr(&child.id))
            .unwrap()
            .is_none(),
        "sync updates placement without inventing a PR"
    );
}

#[test]
fn persistent_refresh_preserves_local_plans_and_followup_after_merge() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let persistent = loopflow::git::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    std::fs::write(persistent.path.join("memory.md"), "accepted\n").unwrap();
    loopflow::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Memory"))
        .unwrap();
    git(repo.path(), &["merge", "--squash", &persistent.branch]);
    repo.commit("Merged document PR");
    repo.push();
    std::fs::create_dir_all(persistent.path.join("scratch")).unwrap();
    std::fs::write(persistent.path.join("scratch/plan.md"), "private\n").unwrap();
    std::fs::write(persistent.path.join("memory.md"), "followup\n").unwrap();
    sync_with_recovery(
        &persistent.path,
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(persistent.path.join("memory.md")).unwrap(),
        "followup\n"
    );
    assert_eq!(
        std::fs::read_to_string(persistent.path.join("scratch/plan.md")).unwrap(),
        "private\n"
    );
    // Everything committed had landed, so the branch restarts from main
    // instead of carrying its squash-merged commit into the next PR.
    assert_eq!(
        git(&persistent.path, &["rev-parse", "HEAD"]),
        git(&persistent.path, &["rev-parse", "origin/main"])
    );
    loopflow::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Followup"))
        .unwrap();
    assert!(git(
        &persistent.path,
        &["diff", "origin/main...HEAD", "--", "memory.md"]
    )
    .contains("+followup"));
    assert_eq!(
        git(
            &persistent.path,
            &["rev-list", "--count", "origin/main..HEAD"]
        )
        .trim(),
        "1"
    );
}

#[test]
fn persistent_memory_conflict_keeps_private_scratch_and_recovery() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("memory.md", "base\n");
    repo.stage_all();
    repo.commit("base memory");
    repo.push();
    let persistent = loopflow::git::worktrees::ensure_agent_worktree(
        repo.path(),
        loopflow::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
    )
    .unwrap();
    std::fs::write(persistent.path.join("memory.md"), "persistent\n").unwrap();
    loopflow::ops::commit_selected(
        &persistent.path,
        &["memory.md".into()],
        Some("Local decision"),
    )
    .unwrap();
    std::fs::create_dir_all(persistent.path.join("scratch")).unwrap();
    std::fs::write(persistent.path.join("scratch/private.md"), "private\n").unwrap();
    repo.create_file("memory.md", "upstream\n");
    repo.stage_all();
    repo.commit("competing decision");
    repo.push();
    let error = sync_with_recovery(
        &persistent.path,
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
    assert!(std::fs::read_to_string(persistent.path.join("memory.md"))
        .unwrap()
        .contains("<<<<<<<"));
    assert_eq!(
        std::fs::read_to_string(persistent.path.join("scratch/private.md")).unwrap(),
        "private\n"
    );
}

#[test]
fn refresh_and_sync_follow_the_default_branch_upstream_in_a_fork() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let upstream = tempfile::tempdir().unwrap();
    git(
        repo.path(),
        &[
            "clone",
            repo.bare_path().to_str().unwrap(),
            upstream.path().to_str().unwrap(),
        ],
    );
    git(
        upstream.path(),
        &["config", "user.name", "Upstream Fixture"],
    );
    git(
        upstream.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    // A non-bare local upstream is sufficient for fetch-only integration.
    git(
        repo.path(),
        &[
            "remote",
            "add",
            "upstream",
            upstream.path().to_str().unwrap(),
        ],
    );
    git(repo.path(), &["fetch", "upstream"]);
    git(
        repo.path(),
        &["branch", "--set-upstream-to=upstream/main", "main"],
    );
    let origin_head = git(repo.path(), &["rev-parse", "origin/main"]);
    std::fs::write(upstream.path().join("upstream.txt"), "upstream work").unwrap();
    git(upstream.path(), &["add", "upstream.txt"]);
    git(upstream.path(), &["commit", "-m", "upstream work"]);
    let upstream_head = git(upstream.path(), &["rev-parse", "HEAD"]);
    repo.create_branch("feature");
    repo.create_file("feature.txt", "local work");
    repo.stage_all();
    repo.commit("feature work");
    loopflow::ops::checkout::refresh_main(repo.path(), &NullProgress).unwrap();
    assert_eq!(git(repo.path(), &["rev-parse", "main"]), upstream_head);
    assert_eq!(git(repo.path(), &["rev-parse", "origin/main"]), origin_head);
    let plan = plan_sync(repo.path(), None, None).unwrap();
    assert_eq!(plan.base_ref, "upstream/main");
    sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: plan.base_ref,
            push: false,
            fork_base: None,
        },
        &NullProgress,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("upstream.txt")).unwrap(),
        "upstream work"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("feature.txt")).unwrap(),
        "local work"
    );
}

#[test]
fn cli_sync_to_explicit_local_branch_needs_no_remote_access() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("feature.txt", "local feature");
    repo.stage_all();
    repo.commit("feature work");
    repo.checkout("main");
    repo.create_file("base.txt", "local base");
    repo.stage_all();
    repo.commit("base work");
    repo.checkout("feature");
    git(
        repo.path(),
        &[
            "remote",
            "set-url",
            "origin",
            "/nonexistent/lf-demo-remote.git",
        ],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["sync", "--manual", "main"])
        .current_dir(repo.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("base.txt")).unwrap(),
        "local base"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("feature.txt")).unwrap(),
        "local feature"
    );
}

#[test]
fn sync_preserves_work_with_ignored_telemetry() {
    assert_sync_preserves_work_with_telemetry(true);
}

#[test]
fn sync_preserves_work_with_unignored_telemetry() {
    assert_sync_preserves_work_with_telemetry(false);
}

fn create_telemetry_sync_repo(ignored: bool) -> TestRepo {
    let repo = TestRepo::new();
    repo.create_file(
        ".gitignore",
        if ignored {
            "/.lf/tmp/\n/ignored.txt\n"
        } else {
            "/ignored.txt\n"
        },
    );
    for path in ["modified.txt", "deleted.txt", "renamed.txt"] {
        repo.create_file(path, "original\n");
    }
    repo.stage_all();
    repo.commit("base files");
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature\n");
    repo.stage_all();
    repo.commit("feature work");
    repo.checkout("main");
    repo.create_file("upstream.txt", "upstream\n");
    repo.stage_all();
    repo.commit("upstream work");
    repo.push();
    repo.checkout("feature");
    repo
}

fn run_sync_cli(repo: &TestRepo) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["sync", "origin/main"])
        .current_dir(repo.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap()
}

fn assert_sync_succeeds(repo: &TestRepo) {
    let output = run_sync_cli(repo);
    assert!(
        output.status.success(),
        "lf sync failed in {}: {}\n{}",
        repo.path().display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_sync_preserves_work_with_telemetry(ignored: bool) {
    let _env = EnvGuard::new(&[]);
    let repo = create_telemetry_sync_repo(ignored);
    let original_head = repo.head_sha();
    repo.create_file("modified.txt", "staged edit\n");
    repo.create_file("staged.txt", "staged addition\n");
    git(repo.path(), &["mv", "renamed.txt", "renamed file.txt"]);
    git(repo.path(), &["add", "modified.txt", "staged.txt"]);
    repo.create_file("modified.txt", "staged edit\nunstaged edit\n");
    std::fs::remove_file(repo.path().join("deleted.txt")).unwrap();
    let additions = [
        "space name.txt",
        "日本語.txt",
        "line\nbreak.txt",
        "-leading.txt",
        ":(exclude)literal.txt",
        ".lf/tmp-neighbor/user.txt",
        "nested/.lf/tmp/user.txt",
    ];
    for path in additions {
        repo.create_file(path, "new work\n");
    }
    let telemetry = [".lf/tmp/metrics/ops.jsonl", ".lf/tmp/.hidden/nested.jsonl"];
    for path in telemetry {
        repo.create_file(path, "{\"existing\":true}\n");
    }
    repo.create_file("ignored.txt", "ignored bytes\n");

    assert_sync_succeeds(&repo);
    let checkpoint = git(repo.path(), &["rev-parse", "HEAD^1"]);
    assert_eq!(
        git(repo.path(), &["rev-parse", &format!("{checkpoint}^")]),
        original_head
    );
    git(
        repo.path(),
        &["merge-base", "--is-ancestor", "origin/main", "HEAD"],
    );
    for (path, expected) in [
        ("modified.txt", "staged edit\nunstaged edit"),
        ("staged.txt", "staged addition"),
        ("renamed file.txt", "original"),
    ]
    .into_iter()
    .chain(additions.map(|path| (path, "new work")))
    {
        for revision in [&checkpoint, "HEAD"] {
            assert_eq!(
                git(repo.path(), &["show", &format!("{revision}:{path}")]),
                expected
            );
        }
        assert_eq!(
            std::fs::read_to_string(repo.path().join(path)).unwrap(),
            format!("{expected}\n")
        );
    }
    for revision in [&checkpoint, "HEAD"] {
        assert!(git(
            repo.path(),
            &[
                "ls-tree",
                "-r",
                "--name-only",
                revision,
                "--",
                ".lf/tmp",
                "ignored.txt",
                "deleted.txt",
                "renamed.txt",
            ],
        )
        .is_empty());
    }
    assert_eq!(git(repo.path(), &["show", "HEAD:upstream.txt"]), "upstream");
    let synced_head = repo.head_sha();
    assert_sync_succeeds(&repo);
    assert_eq!(repo.head_sha(), synced_head);
    for path in telemetry {
        // Sync appends to ops.jsonl; preserve its existing bytes as well.
        assert!(std::fs::read_to_string(repo.path().join(path))
            .unwrap()
            .starts_with("{\"existing\":true}\n"));
    }
    assert_eq!(
        std::fs::read_to_string(repo.path().join("ignored.txt")).unwrap(),
        "ignored bytes\n"
    );
    assert!(git(repo.path(), &["diff", "HEAD"]).is_empty());
    assert!(git(repo.path(), &["stash", "list"]).is_empty());
}

#[test]
fn sync_preserves_work_with_whitespace_only_filename() {
    let _env = EnvGuard::new(&[]);
    let repo = create_telemetry_sync_repo(true);
    repo.create_file("   ", "ordinary work\n");

    assert_sync_succeeds(&repo);
    for revision in ["HEAD^1", "HEAD"] {
        assert_eq!(
            git(repo.path(), &["show", &format!("{revision}:   ")]),
            "ordinary work"
        );
    }
    assert_eq!(
        std::fs::read_to_string(repo.path().join("   ")).unwrap(),
        "ordinary work\n"
    );
}

#[test]
fn sync_preserves_work_with_prestaged_telemetry() {
    let _env = EnvGuard::new(&[]);
    for ignored in [false, true] {
        for ordinary_edits in [false, true] {
            let repo = create_telemetry_sync_repo(ignored);
            let modified = ".lf/tmp/modified.jsonl";
            let deleted = ".lf/tmp/deleted.jsonl";
            for path in [modified, deleted] {
                repo.create_file(path, "committed telemetry\n");
            }
            git(repo.path(), &["add", "-f", "--", modified, deleted]);
            repo.commit("existing telemetry baseline");
            let original_head = repo.head_sha();
            let telemetry_tree = git(repo.path(), &["rev-parse", "HEAD:.lf/tmp"]);
            let telemetry = ".lf/tmp/.hidden/staged\ntelemetry.jsonl";
            for path in [telemetry, modified] {
                repo.create_file(path, "staged telemetry\n");
            }
            git(repo.path(), &["rm", "--", deleted]);
            git(repo.path(), &["add", "-f", "--", telemetry, modified]);
            let staged_telemetry = git(repo.path(), &["diff", "--cached", "--binary"]);
            for path in [telemetry, modified] {
                repo.create_file(path, "staged telemetry\nworking telemetry\n");
            }
            let working_telemetry = git(repo.path(), &["diff", "--binary"]);
            if ordinary_edits {
                repo.create_file("modified.txt", "staged work\n");
                repo.create_file("ignored.txt", "explicitly staged work\n");
                git(repo.path(), &["add", "-f", "modified.txt", "ignored.txt"]);
                repo.create_file("modified.txt", "staged work\nworking work\n");
                repo.create_file("new.txt", "new work\n");
            }

            assert_sync_succeeds(&repo);
            let parent = git(repo.path(), &["rev-parse", "HEAD^1"]);
            if ordinary_edits {
                assert_eq!(
                    git(repo.path(), &["rev-parse", &format!("{parent}^")]),
                    original_head
                );
                for revision in [&parent, "HEAD"] {
                    for (path, content) in [
                        ("modified.txt", "staged work\nworking work"),
                        ("ignored.txt", "explicitly staged work"),
                        ("new.txt", "new work"),
                    ] {
                        assert_eq!(
                            git(repo.path(), &["show", &format!("{revision}:{path}")]),
                            content
                        );
                    }
                }
            } else {
                assert_eq!(parent, original_head);
            }
            for revision in [&parent, "HEAD"] {
                assert_eq!(
                    git(repo.path(), &["rev-parse", &format!("{revision}:.lf/tmp")]),
                    telemetry_tree
                );
            }
            assert_eq!(git(repo.path(), &["show", "HEAD:upstream.txt"]), "upstream");
            let synced_head = repo.head_sha();
            assert_sync_succeeds(&repo);
            assert_eq!(repo.head_sha(), synced_head);
            assert_eq!(
                git(repo.path(), &["diff", "--cached", "--binary"]),
                staged_telemetry
            );
            assert_eq!(git(repo.path(), &["diff", "--binary"]), working_telemetry);
            assert!(!repo.path().join(deleted).exists());
            assert!(git(repo.path(), &["stash", "list"]).is_empty());
        }
    }
}

#[test]
fn sync_preserves_work_with_missing_or_only_telemetry_without_checkpoint() {
    let _env = EnvGuard::new(&[]);
    for ignored in [false, true] {
        for present in [false, true] {
            let repo = create_telemetry_sync_repo(ignored);
            let original_head = repo.head_sha();
            if present {
                repo.create_file(".lf/tmp/local.jsonl", "local telemetry\n");
            }
            assert_sync_succeeds(&repo);
            assert_eq!(git(repo.path(), &["rev-parse", "HEAD^1"]), original_head);
            assert!(git(repo.path(), &["ls-tree", "-r", "HEAD", "--", ".lf/tmp"]).is_empty());
            assert_eq!(git(repo.path(), &["show", "HEAD:upstream.txt"]), "upstream");
            if present {
                assert_eq!(
                    std::fs::read_to_string(repo.path().join(".lf/tmp/local.jsonl")).unwrap(),
                    "local telemetry\n"
                );
            }
            assert!(git(repo.path(), &["stash", "list"]).is_empty());
        }
    }
}

#[test]
fn sync_preserves_work_with_conflicting_telemetry_in_recovery_stash() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let telemetry = ".lf/tmp/local.jsonl";
    repo.create_file(".gitignore", "/.lf/tmp/\n");
    repo.create_file(telemetry, "baseline\n");
    git(repo.path(), &["add", "-f", ".gitignore", telemetry]);
    repo.commit("tracked telemetry baseline");
    repo.create_branch("feature");
    repo.create_file("feature.txt", "feature\n");
    repo.stage_all();
    repo.commit("feature work");
    repo.checkout("main");
    repo.create_file(telemetry, "upstream\n");
    repo.stage_all();
    repo.commit("upstream telemetry");
    repo.push();
    repo.checkout("feature");
    repo.create_file(telemetry, "staged telemetry\n");
    git(repo.path(), &["add", "-f", telemetry]);
    repo.create_file(telemetry, "working telemetry\n");
    repo.create_file("ordinary.txt", "ordinary work\n");

    let output = run_sync_cli(&repo);
    assert!(!output.status.success());
    let stash = git(repo.path(), &["rev-parse", "refs/stash"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("git stash apply --index {stash}")),
        "{stderr}"
    );
    for (revision, content) in [
        ("HEAD^1", "baseline"),
        ("HEAD", "upstream"),
        ("refs/stash^2", "staged telemetry"),
        ("refs/stash", "working telemetry"),
    ] {
        assert_eq!(
            git(repo.path(), &["show", &format!("{revision}:{telemetry}")]),
            content
        );
    }
    assert_eq!(
        git(repo.path(), &["show", "HEAD:ordinary.txt"]),
        "ordinary work"
    );
    assert!(!repo.path().join(".lf/tmp/metrics/ops.jsonl").exists());
}

#[test]
fn sync_preserves_work_with_locked_index_and_reports_failure() {
    let _env = EnvGuard::new(&[]);
    let repo = create_telemetry_sync_repo(true);
    repo.create_file("modified.txt", "staged\n");
    git(repo.path(), &["add", "modified.txt"]);
    repo.create_file("modified.txt", "staged\nunstaged\n");
    repo.create_file("untracked.txt", "new work\n");
    repo.create_file(".lf/tmp/local.jsonl", "local telemetry\n");
    let head = repo.head_sha();
    let index = std::fs::read(repo.path().join(".git/index")).unwrap();
    let status = git(repo.path(), &["status", "--porcelain"]);
    repo.create_file(".git/index.lock", "lock owner\n");

    let output = run_sync_cli(&repo);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("index.lock"), "{stderr}");
    assert_eq!(repo.head_sha(), head);
    assert_eq!(
        std::fs::read(repo.path().join(".git/index")).unwrap(),
        index
    );
    assert_eq!(git(repo.path(), &["status", "--porcelain"]), status);
    for (path, expected) in [
        ("modified.txt", "staged\nunstaged\n"),
        ("untracked.txt", "new work\n"),
        (".lf/tmp/local.jsonl", "local telemetry\n"),
        (".git/index.lock", "lock owner\n"),
    ] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(path)).unwrap(),
            expected
        );
    }
    assert!(!repo.path().join(".lf/tmp/metrics/ops.jsonl").exists());
    assert!(git(repo.path(), &["stash", "list"]).is_empty());
}

#[test]
fn ordinary_scratch_changes_are_authored_work() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("scratch/research.md", "existing research");
    repo.stage_all();
    repo.commit("Repository research");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("scratch/research.md", "updated research");
    let plan = plan_sync(repo.path(), None, None).unwrap();
    assert_eq!(plan.class, SyncClass::CleanAuthored);
    assert_eq!(plan.strategy, SyncStrategy::MergeTarget);
    assert!(!plan.scratch_stashed);
}

#[test]
fn stacked_sync_merges_ordinary_scratch_from_the_parent() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("parent");
    repo.create_file("scratch/design.md", "parent design");
    repo.stage_all();
    repo.commit("Parent design");
    let fork = repo.head_sha();
    repo.create_branch("child");
    repo.create_file("scratch/child.md", "child design");
    repo.stage_all();
    repo.commit("Child design");
    repo.checkout("parent");
    repo.create_file("scratch/parent.md", "parent addition");
    repo.stage_all();
    repo.commit("Parent addition");
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
    assert_eq!(
        std::fs::read_to_string(repo.path().join("scratch/parent.md")).unwrap(),
        "parent addition"
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("scratch/child.md")).unwrap(),
        "child design"
    );
}

#[test]
fn independently_added_scratch_is_merged_without_losing_either_version() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_branch("feature");
    repo.create_file("scratch/design.md", "local notes\n");
    repo.stage_all();
    repo.commit("Local notes");
    repo.checkout("main");
    repo.create_file("scratch/design.md", "upstream notes\n");
    repo.stage_all();
    repo.commit("Upstream notes");
    repo.push();
    repo.checkout("feature");
    let plan = plan_sync(repo.path(), None, None).unwrap();
    assert_eq!(plan.strategy, SyncStrategy::MergeTarget);
    let result = sync_with_recovery(
        repo.path(),
        &SyncOptions {
            onto: "origin/main".into(),
            push: false,
            fork_base: None,
        },
        &NullProgress,
    );
    assert!(matches!(result, Err(OpsError::SyncConflict { .. })));
    let notes = std::fs::read_to_string(repo.path().join("scratch/design.md")).unwrap();
    assert!(notes.contains("local notes"));
    assert!(notes.contains("upstream notes"));
}

#[test]
fn graph_sync_keeps_an_unpublished_scratch_commit_and_its_edits() {
    // LOO-578: a kickoff commit adding only scratch notes, then implementation
    // edits, then a Graph's `op: sync`. The branch must keep its commit.
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("lib.txt", "original\n");
    repo.stage_all();
    repo.commit("library");
    repo.push();
    repo.create_branch("feature");
    repo.create_file("scratch/plan.md", "plan\n");
    repo.create_file("scratch/questions.md", "questions\n");
    repo.stage_all();
    repo.commit("kickoff");
    let kickoff = repo.head_sha();
    repo.checkout("main");
    repo.create_file("main.txt", "upstream\n");
    repo.stage_all();
    repo.commit("upstream");
    repo.push();
    repo.checkout("feature");
    repo.create_file("scratch/plan.md", "plan, implemented\n");
    repo.create_file("lib.txt", "implemented\n");
    repo.create_file("src/new.rs", "fn new() {}\n");

    let command: loopflow::graph::Command =
        serde_json::from_str(r#"{"command":"sync","args":["origin/main"]}"#).unwrap();
    run_graph_command(repo.path(), &command);

    git(
        repo.path(),
        &["merge-base", "--is-ancestor", &kickoff, "HEAD"],
    );
    git(
        repo.path(),
        &["merge-base", "--is-ancestor", "origin/main", "HEAD"],
    );
    for (path, content) in [
        ("scratch/plan.md", "plan, implemented\n"),
        ("scratch/questions.md", "questions\n"),
        ("lib.txt", "implemented\n"),
        ("src/new.rs", "fn new() {}\n"),
        ("main.txt", "upstream\n"),
    ] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(path)).unwrap(),
            content
        );
    }
    assert!(!git(repo.path(), &["stash", "list"]).contains("lf checkout update"));
}

#[test]
fn sync_runs_the_strategy_it_records_for_a_dirty_branch() {
    // LOO-578: `lf sync` classified the branch with its edits, stashed them,
    // then classified again on the cleaned tree and ran that plan instead.
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    repo.create_file("lib.txt", "original\n");
    repo.stage_all();
    repo.commit("library");
    repo.push();
    repo.create_branch("feature");
    repo.checkout("main");
    repo.create_file("main.txt", "upstream\n");
    repo.stage_all();
    repo.commit("upstream");
    repo.push();
    repo.checkout("feature");
    repo.create_file("lib.txt", "edited\n");

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["sync", "origin/main"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");

    let metrics = std::fs::read_to_string(repo.path().join(".lf/tmp/metrics/ops.jsonl")).unwrap();
    let metric: serde_json::Value = serde_json::from_str(metrics.lines().last().unwrap()).unwrap();
    assert_eq!(metric["class"], "clean_authored");
    assert_eq!(metric["strategy"], "merge_target");
    // A merge commits even here; a reset would leave HEAD at the target.
    assert_ne!(
        repo.head_sha(),
        git(repo.path(), &["rev-parse", "origin/main"])
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("lib.txt")).unwrap(),
        "edited\n"
    );
    git(
        repo.path(),
        &["merge-base", "--is-ancestor", "origin/main", "HEAD"],
    );
}

#[test]
fn uncommitted_edits_that_conflict_with_main_reach_the_resolver_not_a_stash() {
    // LOO-659: a Graph's `sync` stashed an implement step's edits, merged main,
    // then failed restoring them, leaving hours of work only in the stash.
    let resolver = r#"#!/usr/bin/python3
import json, subprocess, sys
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
message = json.loads(sys.stdin.readline())
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'fixture-thread'}), flush=True)
open('conflict.txt', 'w').write('feature and main\n')
subprocess.run(['git', 'add', 'conflict.txt'], check=True)
subprocess.run(['git', '-c', 'core.editor=true', 'commit', '--no-edit'], check=True, stdout=sys.stderr)
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': 'resolved'}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'fixture-thread', 'subtype': 'success', 'is_error': False, 'result': 'resolved'}), flush=True)
for line in sys.stdin:
    pass
"#;
    let _env = EnvGuard::new(&[("claude", resolver)]);
    let repo = TestRepo::new();
    repo.create_file("conflict.txt", "original\n");
    repo.create_file(".lf/config.yaml", "agent: claude\n");
    repo.create_file(".gitignore", ".lf/prompts/\n.lf/tmp/\n");
    repo.stage_all();
    repo.commit("original");
    repo.push();
    repo.create_branch("feature");
    repo.checkout("main");
    repo.create_file("conflict.txt", "main line\n");
    repo.stage_all();
    repo.commit("main work");
    repo.push();
    repo.checkout("feature");
    repo.create_file("conflict.txt", "feature line\n");
    repo.create_file("src/new.rs", "fn new() {}\n");

    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["sync", "origin/main"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");

    assert!(git(repo.path(), &["stash", "list"]).is_empty());
    assert_eq!(git(repo.path(), &["status", "--porcelain"]), "");
    git(
        repo.path(),
        &["merge-base", "--is-ancestor", "origin/main", "HEAD"],
    );
    let checkpoint = git(repo.path(), &["rev-parse", "HEAD^1"]);
    assert_eq!(
        git(
            repo.path(),
            &["show", &format!("{checkpoint}:conflict.txt")]
        ),
        "feature line"
    );
    for (path, content) in [
        ("conflict.txt", "feature and main\n"),
        ("src/new.rs", "fn new() {}\n"),
    ] {
        assert_eq!(
            std::fs::read_to_string(repo.path().join(path)).unwrap(),
            content
        );
    }
}
