mod support;

use loopflow::child_session::{ChildCommandKind, ChildCommandSource, ChildRef};
use loopflow::ops::linear_observe::reconcile_linear_observation;
use loopflow::pm::{IssueComment, IssueObservation};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};
use time::OffsetDateTime;

const VIEWER: &str = "user-loopflow";

fn human(id: &str, body: &str) -> IssueComment {
    IssueComment {
        id: id.to_string(),
        body: body.to_string(),
        author_id: Some("user-human".to_string()),
    }
}

fn observation(
    revision: &str,
    title: &str,
    description: &str,
    comments: Vec<IssueComment>,
) -> IssueObservation {
    IssueObservation {
        revision: revision.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        comments,
    }
}

/// The full writer path: baseline a Session against the issue, then prove an
/// edit becomes exactly one versioned directive, a new human comment becomes one
/// FIFO follow-up, re-reading the same revision changes nothing, and a stale
/// out-of-order read is dropped rather than reverting direction.
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

    // The launch snapshot's title/description, so the first read is a no-op
    // baseline that never replays the pre-existing comment as direction.
    let launch_title = task.session.launch.issue.title.clone();
    let launch_desc = task.session.launch.issue.description.clone();

    // 1. Baseline.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &task.session,
            observation(
                "2026-07-15T00:00:00.000Z",
                &launch_title,
                &launch_desc,
                vec![human("c-old", "old chatter")],
            ),
            VIEWER,
            now,
        ))
        .expect("baseline");
    assert!(outcome.baselined);
    let commands = rt
        .block_on(task.store.list_child_commands(&target))
        .expect("commands");
    assert!(commands.is_empty(), "baseline emits no direction");

    // 2. A human edits title + description and leaves a new comment. Loopflow's
    // own writeback comment must not reach the worker.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &task.session,
            observation(
                "2026-07-15T01:00:00.000Z",
                "New title",
                "New body",
                vec![
                    human("c-old", "old chatter"),
                    human("c-1", "please prioritize"),
                    IssueComment {
                        id: "c-loopflow".to_string(),
                        body: "PR: https://x".to_string(),
                        author_id: Some(VIEWER.to_string()),
                    },
                ],
            ),
            VIEWER,
            now,
        ))
        .expect("edit");
    assert!(!outcome.baselined);
    assert!(outcome.directive_applied);
    assert_eq!(
        outcome.follow_ups_created.len(),
        1,
        "only the new human comment"
    );

    let session = rt
        .block_on(task.store.get_task_session(&task.session.id))
        .expect("read session")
        .expect("session");
    assert_eq!(session.current_directive_version, 1);

    let commands = rt
        .block_on(task.store.list_child_commands(&target))
        .expect("commands");
    assert_eq!(commands.len(), 2);
    assert!(commands
        .iter()
        .all(|c| c.source == ChildCommandSource::Linear));
    assert!(commands.iter().any(
        |c| matches!(&c.kind, ChildCommandKind::Steer { text } if text.contains("New title"))
    ));
    assert!(commands
        .iter()
        .any(|c| matches!(&c.kind, ChildCommandKind::FollowUp { text } if text.contains("please prioritize"))));

    // 3. Re-read the same revision: no duplicate directive, no duplicate follow-up.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &session,
            observation(
                "2026-07-15T01:00:00.000Z",
                "New title",
                "New body",
                vec![
                    human("c-old", "old chatter"),
                    human("c-1", "please prioritize"),
                ],
            ),
            VIEWER,
            now,
        ))
        .expect("re-read");
    assert!(!outcome.directive_applied);
    assert!(outcome.follow_ups_created.is_empty());
    let commands = rt
        .block_on(task.store.list_child_commands(&target))
        .expect("commands");
    assert_eq!(commands.len(), 2, "idempotent re-read adds nothing");

    // 4. A stale, out-of-order read (older revision, older content) is dropped.
    let outcome = rt
        .block_on(reconcile_linear_observation(
            &task.store,
            &session,
            observation(
                "2026-07-15T00:30:00.000Z",
                &launch_title,
                &launch_desc,
                vec![],
            ),
            VIEWER,
            now,
        ))
        .expect("stale read");
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
