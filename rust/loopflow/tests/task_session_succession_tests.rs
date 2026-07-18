mod support;

use std::path::PathBuf;

use loopflow::child_session::ChildRef;
use loopflow::ops::linear_observe::reconcile_linear_observation;
use loopflow::pm::IssueObservation;
use loopflow::task::{
    TaskLifecyclePhase, TaskLifecyclePlan, TaskPr, TaskPrId, TaskSession, TaskSessionId,
    TaskSessionStatus,
};
use loopflow::webhook::{ingest_event, WebhookEvent, WebhookOutcome};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};
use time::OffsetDateTime;

const VIEWER: &str = "user-loopflow";

fn edit(revision: &str, title: &str, description: &str) -> IssueObservation {
    IssueObservation {
        revision: revision.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        comments: vec![],
    }
}

/// Build a successor Task Session for the same Linear issue as `predecessor`,
/// with a fresh id and worktree so it can coexist with the terminal predecessor
/// under the partial unique indexes. Its sequence-1 Working PR is distinct.
fn successor_session(
    predecessor: &TaskSession,
    predecessor_pr: &TaskPr,
    now: OffsetDateTime,
) -> (TaskSession, TaskPr) {
    let mut session = predecessor.clone();
    session.id = TaskSessionId::new();
    session.status = TaskSessionStatus::Created;
    session.status_reason = "Task Session succeeds a terminal predecessor".to_string();
    session.status_at = now;
    session.worktree = PathBuf::from(format!("{}-successor", predecessor.worktree.display()));
    session.workspace_slug = format!("{}-2", predecessor.workspace_slug);
    session.provider_session_id = None;
    session.lifecycle = TaskLifecyclePlan::standard("task");
    session.lifecycle_phase = TaskLifecyclePhase::Kickoff;
    session.phase_epoch = 1;
    session.phase_cursor = 0;
    session.phase_iteration = 0;
    session.gate_cycle = 0;
    session.gate_proposal = None;
    session.abandon_intent = None;
    session.created_at = now;
    session.updated_at = now;
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_session_id: session.id.clone(),
        sequence: 1,
        slug: session.workspace_slug.clone(),
        branch: format!("{}-2", predecessor_pr.branch),
        base_commit: predecessor_pr.base_commit.clone(),
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        created_at: now,
        updated_at: now,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
    };
    (session, pr)
}

/// The core contract: a terminal Task predecessor's Linear observation cursor,
/// title/body revision, and ingested-comment ledger are carried (re-keyed) onto
/// its successor in one transaction with successor creation. An edit and a
/// comment that race recovery are each delivered exactly once — never
/// baselined away or duplicated — successor polling resumes from the
/// predecessor cursor, historical receipts stay attributable to the
/// predecessor, and a crash/retry succession is idempotent.
#[test]
fn succession_carries_direction_and_racing_recovery_is_exactly_once() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-succession";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "task succession\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);
    let registered = register_task(home.path(), repo.path(), branch, &base);
    let store = registered.store;
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let now = OffsetDateTime::now_utc();
    let mut predecessor = registered.session;
    let predecessor_pr = registered.pr;
    let predecessor_target = ChildRef::Task(predecessor.id.clone());

    // 1. The predecessor receives an edit and a human comment — each once.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &store,
            &predecessor,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("predecessor edit");
    assert!(outcome.content_steer_applied);
    let created = rt
        .block_on(store.apply_linear_comment(
            &predecessor.id,
            "c-1".to_string(),
            "please prioritize".to_string(),
            now,
        ))
        .expect("predecessor comment");
    assert!(created.is_some());

    // The predecessor's cursor and ledger advanced.
    let predecessor_cursor = rt
        .block_on(store.task_linear_observation(&predecessor.id))
        .expect("cursor read")
        .expect("cursor present");
    assert_eq!(predecessor_cursor.last_title, "New title");
    assert_eq!(predecessor_cursor.last_revision, "2026-07-15T01:00:00.000Z");

    // 2. The predecessor terminates.
    predecessor.set_status(TaskSessionStatus::Completed, "predecessor landed");
    rt.block_on(store.update_task_session(&predecessor))
        .expect("complete predecessor");

    // 3. Create the successor, carrying the cursor and ledger in one transaction.
    let (successor, successor_pr) = successor_session(&predecessor, &predecessor_pr, now);
    let initial = "Carry the predecessor's direction forward.";
    let succession = rt
        .block_on(store.reserve_task_session_successor(
            &predecessor,
            &successor,
            &successor_pr,
            loopflow::durable::Author::User,
            initial,
        ))
        .expect("succession");
    assert!(succession.created, "first succession creates the successor");
    let successor = succession.session;
    let successor_target = ChildRef::Task(successor.id.clone());

    // 4. The successor resumes polling from the predecessor cursor: the carried
    //    cursor matches the already-applied edit, and the carried ledger holds
    //    the already-delivered comment. A racing re-delivery of the SAME edit
    //    and comment is deduped — exactly once, not duplicated, not baselined.
    let racing_edit = rt
        .block_on(reconcile_linear_observation(
            &store,
            &successor,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("racing edit re-delivery");
    assert!(
        !racing_edit.content_steer_applied,
        "carried cursor dedups the racing edit"
    );
    let racing_comment = rt
        .block_on(store.apply_linear_comment(
            &successor.id,
            "c-1".to_string(),
            "please prioritize".to_string(),
            now,
        ))
        .expect("racing comment re-delivery");
    assert!(
        racing_comment.is_none(),
        "carried ledger dedups the racing comment"
    );

    // The cursor was re-keyed off the predecessor (no orphaned state) and onto
    // the successor at the predecessor's last revision.
    let predecessor_cursor_after = rt
        .block_on(store.task_linear_observation(&predecessor.id))
        .expect("cursor read");
    assert!(
        predecessor_cursor_after.is_none(),
        "cursor re-keyed off the terminal predecessor"
    );
    let successor_cursor = rt
        .block_on(store.task_linear_observation(&successor.id))
        .expect("cursor read")
        .expect("successor cursor carried");
    assert_eq!(successor_cursor.last_title, "New title");
    assert_eq!(
        successor_cursor.last_revision, "2026-07-15T01:00:00.000Z",
        "successor resumes from the predecessor cursor"
    );

    // 5. Both Session-era ids resolve to the same stable Work. Its current
    //    Epoch begins with only the explicit carry Steer; old-Epoch input is
    //    not selected into the recovered boundary.
    let predecessor_work = rt
        .block_on(store.work_for_child(&predecessor_target))
        .expect("predecessor work");
    let successor_work = rt
        .block_on(store.work_for_child(&successor_target))
        .expect("successor work");
    assert_eq!(predecessor_work, successor_work);
    let successor_seed = rt
        .block_on(store.boundary_seed(&successor_work))
        .expect("successor seed");
    assert_eq!(successor_seed.steers.len(), 1);
    assert_eq!(successor_seed.steers[0].text, initial);

    // 6. Resolution prefers the non-terminal successor, by issue id and by
    //    identifier, over the terminal predecessor.
    let by_id = rt
        .block_on(store.get_task_session_by_issue(predecessor.launch.issue.id.as_str()))
        .expect("resolve by id")
        .expect("resolved");
    assert_eq!(by_id.id, successor.id);
    let by_identifier = rt
        .block_on(store.get_task_session_by_issue(&predecessor.launch.issue.identifier))
        .expect("resolve by identifier")
        .expect("resolved");
    assert_eq!(by_identifier.id, successor.id);

    // 7. A NEW edit and comment on the successor are each delivered once.
    let new_edit = rt
        .block_on(reconcile_linear_observation(
            &store,
            &successor,
            edit("2026-07-15T02:00:00.000Z", "Newer title", "Newer body"),
            VIEWER,
            now,
        ))
        .expect("new edit");
    assert!(new_edit.content_steer_applied, "new edit delivered once");
    let new_comment = rt
        .block_on(store.apply_linear_comment(
            &successor.id,
            "c-2".to_string(),
            "after succession".to_string(),
            now,
        ))
        .expect("new comment");
    assert!(new_comment.is_some(), "new comment delivered once");
    let successor_seed = rt
        .block_on(store.boundary_seed(&successor_work))
        .expect("successor seed");
    assert_eq!(
        successor_seed.steers.len(),
        3,
        "carry + one new edit + one new comment on the successor"
    );

    // 8. Crash/retry: a second succession is idempotent — it returns the
    //    existing non-terminal successor without re-keying or duplicating.
    let retry = rt
        .block_on(store.reserve_task_session_successor(
            &predecessor,
            &successor,
            &successor_pr,
            loopflow::durable::Author::User,
            initial,
        ))
        .expect("retry succession");
    assert!(!retry.created, "second succession is a no-op");
    assert_eq!(retry.session.id, successor.id);
    let cursor_after_retry = rt
        .block_on(store.task_linear_observation(&successor.id))
        .expect("cursor read")
        .expect("cursor still present");
    assert_eq!(
        cursor_after_retry.last_title, "Newer title",
        "idempotent retry moves nothing"
    );
    let successor_seed_after_retry = rt
        .block_on(store.boundary_seed(&successor_work))
        .expect("successor seed");
    assert_eq!(
        successor_seed_after_retry.steers.len(),
        3,
        "idempotent retry adds no Steers"
    );
}

/// The webhook integration boundary: a verified webhook resolves to the
/// non-terminal successor after succession, so an edit and a comment that race
/// recovery are delivered exactly once across the predecessor→successor
/// boundary, and a new edit and comment land once on the successor.
#[test]
fn webhooks_resolve_to_the_successor_across_the_boundary() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/task-succession-webhook";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "task succession webhook\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);
    let registered = register_task(home.path(), repo.path(), branch, &base);
    let store = registered.store;
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let now = OffsetDateTime::now_utc();
    let mut predecessor = registered.session;
    let predecessor_pr = registered.pr;
    let issue_id = predecessor.launch.issue.id.as_str().to_string();

    let edit_event = |revision: &str, title: &str, description: &str| WebhookEvent::IssueEdit {
        issue_id: issue_id.clone(),
        title: title.to_string(),
        description: description.to_string(),
        revision: revision.to_string(),
        actor_id: Some("user-human".to_string()),
    };
    let comment_event = |id: &str, body: &str| WebhookEvent::Comment {
        issue_id: issue_id.clone(),
        comment_id: id.to_string(),
        body: body.to_string(),
        author_id: Some("user-human".to_string()),
    };

    // The predecessor receives one edit and one comment through the webhook.
    assert_eq!(
        rt.block_on(ingest_event(
            &store,
            edit_event("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("predecessor edit"),
        WebhookOutcome::Edit {
            steer_applied: true
        }
    );
    assert_eq!(
        rt.block_on(ingest_event(
            &store,
            comment_event("c-1", "please prioritize"),
            VIEWER,
            now,
        ))
        .expect("predecessor comment"),
        WebhookOutcome::Comment { delivered: true }
    );

    // The predecessor terminates and its direction carries onto a successor.
    predecessor.set_status(TaskSessionStatus::Abandoned, "predecessor abandoned");
    rt.block_on(store.update_task_session(&predecessor))
        .expect("abandon predecessor");
    let (successor, successor_pr) = successor_session(&predecessor, &predecessor_pr, now);
    let initial = "Carry the predecessor's direction forward.";
    let succession = rt
        .block_on(store.reserve_task_session_successor(
            &predecessor,
            &successor,
            &successor_pr,
            loopflow::durable::Author::User,
            initial,
        ))
        .expect("succession");
    assert!(succession.created);
    let successor = succession.session;

    // The same webhook edit and comment, redelivered (racing recovery), now
    // resolve to the successor and are deduped by the carried cursor and ledger.
    assert_eq!(
        rt.block_on(ingest_event(
            &store,
            edit_event("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("racing edit"),
        WebhookOutcome::Edit {
            steer_applied: false
        }
    );
    assert_eq!(
        rt.block_on(ingest_event(
            &store,
            comment_event("c-1", "please prioritize"),
            VIEWER,
            now,
        ))
        .expect("racing comment"),
        WebhookOutcome::Comment { delivered: false }
    );

    // A new edit and comment resolve to the successor and land once.
    assert_eq!(
        rt.block_on(ingest_event(
            &store,
            edit_event("2026-07-15T02:00:00.000Z", "Newer title", "Newer body"),
            VIEWER,
            now,
        ))
        .expect("new edit"),
        WebhookOutcome::Edit {
            steer_applied: true
        }
    );
    assert_eq!(
        rt.block_on(ingest_event(
            &store,
            comment_event("c-2", "after succession"),
            VIEWER,
            now,
        ))
        .expect("new comment"),
        WebhookOutcome::Comment { delivered: true }
    );

    // Both Session-era ids still resolve to the stable Work. Its current Epoch
    // contains the explicit carry plus one edit and one comment.
    let predecessor_work = rt
        .block_on(store.work_for_child(&ChildRef::Task(predecessor.id.clone())))
        .expect("predecessor work");
    let successor_work = rt
        .block_on(store.work_for_child(&ChildRef::Task(successor.id.clone())))
        .expect("successor work");
    assert_eq!(predecessor_work, successor_work);
    let successor_seed = rt
        .block_on(store.boundary_seed(&successor_work))
        .expect("successor seed");
    assert_eq!(
        successor_seed.steers.len(),
        3,
        "carry + one new edit + one new comment; the racing redelivery added nothing"
    );
}
