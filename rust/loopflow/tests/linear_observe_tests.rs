mod support;

use loopflow::child::ChildRef;
use loopflow::ops::linear_observe::reconcile_linear_observation;
use loopflow::pm::IssueObservation;
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};
use time::OffsetDateTime;

const VIEWER: &str = "user-loopflow";

fn edit(revision: &str, title: &str, description: &str) -> IssueObservation {
    // The webhook issue-edit path (and the catch-up read) carry no comments —
    // comments arrive one at a time through `apply_linear_comment`.
    IssueObservation {
        revision: revision.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        comments: vec![],
    }
}

/// The durable substrate under the webhook receiver: the cursor is seeded at
/// Task creation, an issue edit becomes exactly one Steer (and a
/// stale/duplicate delivery reverts nothing), and a user comment becomes one
/// FIFO Steer that a redelivered webhook cannot double-apply.
#[test]
fn linear_edits_and_comments_stream_into_task_control_exactly_once() {
    let home = tempfile::TempDir::new().expect("temp home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/linear-observe";
    repo.create_branch(branch);
    repo.create_file("proof.txt", "linear observe\n");
    repo.stage_all();
    repo.commit("seed");
    repo.push_new_branch(branch);
    let task = register_task(home.path(), repo.path(), branch, &base);
    let target = ChildRef::Task(task.task.id.clone());
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let now = OffsetDateTime::now_utc();

    // The cursor is seeded from the Task directive when the Task is created,
    // so the first edit diffs against launch content rather than baselining it.
    let seeded = rt
        .block_on(task.store.task_linear_observation(&task.task.id))
        .expect("cursor read")
        .expect("cursor seeded at creation");
    assert_eq!(seeded.last_title, task.task.plan.title);
    assert_eq!(seeded.last_revision, "");

    // 1. A user edits title + description → one Steer.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &task.task,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("edit");
    assert!(!outcome.baselined);
    assert!(outcome.content_steer_applied);

    let persisted_task = rt
        .block_on(task.store.get_task(&task.task.id))
        .expect("read Task")
        .expect("Task");
    let work = rt
        .block_on(task.store.work_for_child(&target))
        .expect("work");
    let steers = rt
        .block_on(task.store.work_steers(&work))
        .expect("Work steers");
    assert_eq!(steers.len(), 1);
    assert!(steers[0].text.contains("New title"));

    // 2. Re-deliver the same edit → no duplicate directive.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &persisted_task,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("re-deliver");
    assert!(!outcome.content_steer_applied);

    // 3. A user comment → one FIFO Steer; a redelivered webhook adds nothing.
    let created = rt
        .block_on(task.store.apply_linear_comment(
            &task.task.id,
            "c-1".to_string(),
            "please prioritize".to_string(),
            now,
        ))
        .expect("comment");
    assert!(created.is_some(), "first delivery creates a follow-up");
    let duplicate = rt
        .block_on(task.store.apply_linear_comment(
            &task.task.id,
            "c-1".to_string(),
            "please prioritize".to_string(),
            now,
        ))
        .expect("duplicate comment");
    assert!(duplicate.is_none(), "redelivery is a no-op");

    let steers = rt
        .block_on(task.store.work_steers(&work))
        .expect("Work steers");
    assert_eq!(steers.len(), 2, "one edit + one comment, no dup");
    assert!(steers[1].text.contains("please prioritize"));

    // 4. A stale, out-of-order edit (older revision, older content) is dropped.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &persisted_task,
            edit(
                "2026-07-15T00:30:00.000Z",
                &task.task.plan.title,
                &task.task.plan.description,
            ),
            VIEWER,
            now,
        ))
        .expect("stale edit");
    assert!(
        !outcome.content_steer_applied,
        "stale content never reverts direction"
    );
    let cursor = rt
        .block_on(task.store.task_linear_observation(&task.task.id))
        .expect("cursor")
        .expect("cursor exists");
    assert_eq!(cursor.last_title, "New title");
    assert_eq!(cursor.last_revision, "2026-07-15T01:00:00.000Z");
}
