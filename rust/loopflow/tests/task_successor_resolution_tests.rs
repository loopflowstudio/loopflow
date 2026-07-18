//! W2-249 — current Task Session resolution after recovery.
//!
//! The store tests prove the centralized resolution rule. These tests drive the
//! real operational consumers (`ingest_event` for PR publication, `task_status`
//! for status/roadmap) over a terminal predecessor plus a live successor sharing
//! an issue and worktree, proving each consumer routes to the live successor
//! rather than the terminal history.

mod support;

use loopflow::child_session::ChildRef;
use loopflow::ops::task::{task_stack, task_status};
use loopflow::task::{AfterMerge, GithubPr, PrPublication, TaskPr, TaskPrId, TaskSessionStatus};
use loopflow::webhook::{ingest_event, WebhookEvent, WebhookOutcome};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard, RegisteredTask};
use time::OffsetDateTime;

const VIEWER: &str = "user-loopflow";

/// A `gh` with no open PRs, so `task_status`'s PR reconcile is a no-op and the
/// resolved Session returns unchanged.
fn gh_empty_pr_script() -> &'static str {
    "#!/bin/sh
if [ \"$1\" = \"--version\" ]; then
  exit 0
fi
if [ \"$1 $2\" = \"pr list\" ]; then
  echo '[]'
  exit 0
fi
exit 0
"
}

/// Mark the registered Task a completed predecessor, then insert a live successor
/// that shares its issue id, identifier, and worktree (recovery history).
fn successor_sharing_the_issue(
    task: &RegisteredTask,
    successor_branch: &str,
    parent_pr_id: Option<TaskPrId>,
) {
    let rt = tokio::runtime::Runtime::new().expect("successor runtime");
    rt.block_on(async {
        let mut predecessor = task.session.clone();
        predecessor.set_status(TaskSessionStatus::Completed, "PR merged");
        task.store
            .update_task_session(&predecessor)
            .await
            .expect("complete predecessor");

        let mut successor = task.session.clone();
        successor.set_status(TaskSessionStatus::Waiting, "recovered attempt");
        // Fresh id; issue id, identifier, and worktree stay shared.
        successor.id = loopflow::task::TaskSessionId::new();
        successor.created_at = OffsetDateTime::now_utc();
        successor.updated_at = successor.created_at;
        let successor_pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: successor.id.clone(),
            sequence: 1,
            slug: successor.workspace_slug.clone(),
            branch: successor_branch.to_string(),
            base_commit: task.pr.base_commit.clone(),
            parent_pr_id,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: successor.created_at,
            updated_at: successor.updated_at,
        };
        task.store
            .create_task_session(&successor, &successor_pr)
            .await
            .expect("insert live successor");
    });
}

/// PR publication: a verified webhook routes Task control to the live successor,
/// never to the completed predecessor that shares the issue.
#[test]
fn webhook_routes_control_to_the_live_successor() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_empty_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/predecessor";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "predecessor\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);

    let task = register_task(home.path(), repo.path(), branch, &base);
    successor_sharing_the_issue(&task, "jack/successor", None);
    let issue_id = task.session.launch.issue.id.as_str().to_string();
    let rt = tokio::runtime::Runtime::new().expect("runtime");

    // Resolve the live successor's id directly from the store for comparison.
    let live = rt
        .block_on(task.store.get_task_session_by_issue(&issue_id))
        .expect("read")
        .expect("a live successor exists");
    assert_ne!(
        live.id, task.session.id,
        "resolution must pick the successor"
    );

    let outcome = rt
        .block_on(ingest_event(
            &task.store,
            WebhookEvent::Comment {
                issue_id: issue_id.clone(),
                comment_id: "c-successor".to_string(),
                body: "steer the recovered attempt".to_string(),
                author_id: Some("user-human".to_string()),
            },
            VIEWER,
            OffsetDateTime::now_utc(),
        ))
        .expect("comment");
    assert_eq!(outcome, WebhookOutcome::Comment { delivered: true });

    // The webhook resolves the live successor Session, while control addresses
    // the one stable Work and its current Epoch.
    let successor_work = rt
        .block_on(task.store.work_for_child(&ChildRef::Task(live.id.clone())))
        .expect("successor work");
    let successor_seed = rt
        .block_on(task.store.boundary_seed(&successor_work))
        .expect("successor seed");
    assert_eq!(successor_seed.steers.len(), 1);
    assert!(successor_seed.steers[0]
        .text
        .contains("steer the recovered attempt"));
    let predecessor_work = rt
        .block_on(
            task.store
                .work_for_child(&ChildRef::Task(task.session.id.clone())),
        )
        .expect("predecessor work");
    assert_eq!(predecessor_work, successor_work);
}

/// Status/roadmap: `lf task status` resolves the live successor, not the
/// completed predecessor sharing its identifier and worktree.
#[test]
fn task_status_resolves_the_live_successor() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_empty_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/status-predecessor";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "predecessor\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);

    let task = register_task(home.path(), repo.path(), branch, &base);
    successor_sharing_the_issue(&task, "jack/status-successor", None);

    let resolved = task_status("INF-123").expect("task status");
    assert_ne!(
        resolved.id, task.session.id,
        "status must pick the successor"
    );
    assert_eq!(
        resolved.status,
        TaskSessionStatus::Waiting,
        "the live successor is the current attempt"
    );
}

/// Stacking resolves the shared worktree to the live successor and therefore
/// returns the successor's child PR, never the terminal predecessor's PR.
#[test]
fn task_stack_resolves_the_live_successor_pr_from_shared_worktree() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_empty_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/stack-predecessor";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "predecessor\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);

    let task = register_task(home.path(), repo.path(), branch, &base);
    successor_sharing_the_issue(&task, "jack/stack-successor", Some(task.pr.id.clone()));

    let stacked = task_stack(repo.path())
        .expect("stack resolution")
        .expect("the successor PR is stacked");
    assert_ne!(stacked.child.task_session_id, task.session.id);
    assert_eq!(stacked.child.branch, "jack/stack-successor");
    assert_eq!(stacked.parent_branch.as_deref(), Some(branch));
}

/// A terminal-only history (no live successor) still resolves, so `task status`
/// on a completed Task reports it rather than claiming none exists.
#[test]
fn task_status_falls_back_to_terminal_history_when_no_successor_is_live() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[("gh", gh_empty_pr_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/terminal-only";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "terminal only\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);

    let task = register_task(home.path(), repo.path(), branch, &base);
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let mut settled = task.pr.clone();
    settled.publication = Some(PrPublication {
        requested_at: OffsetDateTime::now_utc(),
        after_merge: AfterMerge::CompleteTask,
        next_slug: None,
        github: Some(GithubPr {
            number: 900,
            url: "https://example.com/pr/900".to_string(),
            head_sha: None,
        }),
    });
    settled.merge_commit = Some("merge-terminal-history".to_string());
    rt.block_on(task.store.settle_task_pr(&settled, None))
        .expect("settle PR");
    let mut completed = task.session.clone();
    completed.set_status(TaskSessionStatus::Completed, "PR merged");
    rt.block_on(task.store.update_task_session(&completed))
        .expect("complete");

    let resolved = task_status("INF-123").expect("task status");
    assert_eq!(
        resolved.id, task.session.id,
        "the terminal predecessor is current"
    );
    assert_eq!(resolved.status, TaskSessionStatus::Completed);
}
