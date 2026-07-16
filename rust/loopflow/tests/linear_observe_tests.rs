mod support;

use loopflow::child_session::{ChildCommand, ChildCommandKind, ChildCommandSource, ChildRef};
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

fn follow_up_command(target: &ChildRef, body: &str) -> ChildCommand {
    ChildCommand::new(
        target.clone(),
        ChildCommandSource::Linear,
        ChildCommandKind::FollowUp {
            text: body.to_string(),
        },
    )
}

/// The durable substrate under the webhook receiver: the cursor is seeded at
/// Session creation, an issue edit becomes exactly one versioned directive (and
/// a stale/duplicate delivery reverts nothing), and a human comment becomes one
/// FIFO follow-up that a redelivered webhook cannot double-apply.
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
    let target = ChildRef::Task(task.session.id.clone());
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let now = OffsetDateTime::now_utc();

    // The cursor is seeded from the launch snapshot when the Session is created,
    // so the first edit diffs against launch content rather than baselining it.
    let seeded = rt
        .block_on(task.store.task_linear_observation(&task.session.id))
        .expect("cursor read")
        .expect("cursor seeded at creation");
    assert_eq!(seeded.last_title, task.session.launch.issue.title);
    assert_eq!(seeded.last_revision, "");

    // 1. A human edits title + description → one replacement directive.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &task.session,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("edit");
    assert!(!outcome.baselined);
    assert!(outcome.directive_applied);

    let session = rt
        .block_on(task.store.get_task_session(&task.session.id))
        .expect("read session")
        .expect("session");
    assert_eq!(session.current_directive_version, 1);
    let commands = rt
        .block_on(task.store.list_child_commands(&target))
        .expect("commands");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].source, ChildCommandSource::Linear);
    assert!(
        matches!(&commands[0].kind, ChildCommandKind::Steer { text } if text.contains("New title"))
    );

    // 2. Re-deliver the same edit → no duplicate directive.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &session,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            VIEWER,
            now,
        ))
        .expect("re-deliver");
    assert!(!outcome.directive_applied);

    // 3. A human comment → one FIFO follow-up; a redelivered webhook adds nothing.
    let created = rt
        .block_on(task.store.apply_linear_comment(
            &task.session.id,
            "c-1".to_string(),
            follow_up_command(&target, "please prioritize"),
            now,
        ))
        .expect("comment");
    assert!(created.is_some(), "first delivery creates a follow-up");
    let duplicate = rt
        .block_on(task.store.apply_linear_comment(
            &task.session.id,
            "c-1".to_string(),
            follow_up_command(&target, "please prioritize"),
            now,
        ))
        .expect("duplicate comment");
    assert!(duplicate.is_none(), "redelivery is a no-op");

    let commands = rt
        .block_on(task.store.list_child_commands(&target))
        .expect("commands");
    assert_eq!(commands.len(), 2, "one directive + one follow-up, no dup");
    assert!(commands
        .iter()
        .any(|c| matches!(&c.kind, ChildCommandKind::FollowUp { text } if text.contains("please prioritize"))));

    // 4. A stale, out-of-order edit (older revision, older content) is dropped.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &session,
            edit(
                "2026-07-15T00:30:00.000Z",
                &task.session.launch.issue.title,
                &task.session.launch.issue.description,
            ),
            VIEWER,
            now,
        ))
        .expect("stale edit");
    assert!(
        !outcome.directive_applied,
        "stale content never reverts direction"
    );
    let cursor = rt
        .block_on(task.store.task_linear_observation(&task.session.id))
        .expect("cursor")
        .expect("cursor exists");
    assert_eq!(cursor.last_title, "New title");
    assert_eq!(cursor.last_revision, "2026-07-15T01:00:00.000Z");
}
