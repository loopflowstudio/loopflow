mod support;

use loopflow::pm::{IssueComment, IssueObservation};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard};
use time::OffsetDateTime;

fn edit(revision: &str, title: &str, description: &str) -> IssueObservation {
    IssueObservation {
        revision: revision.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        comments: vec![],
    }
}

/// The store retains ordered direction across duplicate and stale observations.
/// This proves ingestion, not provider delivery or a webhook receiver.
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

    // Prepare a later explicit return to the original definition before either
    // write. Ingestion compares with its committed cursor, not a saved proposal.
    let restored = edit(
        "2026-07-15T02:00:00.000Z",
        &task.task.plan.title,
        &task.task.plan.description,
    );

    // 1. A user edits title + description → one Steer.
    let outcome = rt
        .block_on(task.store.apply_linear_observation(
            &task.task.id,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            now,
        ))
        .expect("edit");
    assert!(!outcome.baselined);
    assert!(outcome.content_steer_applied);

    let steers = rt
        .block_on(task.store.task_steers(&task.task.id))
        .expect("Work steers");
    assert_eq!(steers.len(), 1);
    assert!(steers[0].text.contains("New title"));

    // 2. Re-deliver the same edit → no duplicate directive.
    let outcome = rt
        .block_on(task.store.apply_linear_observation(
            &task.task.id,
            edit("2026-07-15T01:00:00.000Z", "New title", "New body"),
            now,
        ))
        .expect("re-deliver");
    assert!(!outcome.content_steer_applied);

    // 3. One observation writes both the visible comment and its FIFO Steer.
    let mut observation = edit("2026-07-15T01:00:00.000Z", "New title", "New body");
    observation.comments.push(IssueComment {
        id: "c-1".into(),
        revision: Some("2026-07-15T01:00:00.000Z".into()),
        created_at: None,
        body: "please prioritize".into(),
        author_id: Some("person".into()),
        author_name: Some("Maya".into()),
    });
    for expected in [1, 0] {
        let outcome = rt
            .block_on(
                task.store
                    .apply_linear_observation(&task.task.id, observation.clone(), now),
            )
            .expect("comment observation");
        assert_eq!(outcome.follow_ups_created.len(), expected);
    }
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let body: String = conn
        .query_row("SELECT body FROM task_comments WHERE id='c-1'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(body, "please prioritize");

    let steers = rt
        .block_on(task.store.task_steers(&task.task.id))
        .expect("Work steers");
    assert_eq!(steers.len(), 2, "one edit + one comment, no dup");
    assert!(steers[1].text.contains("please prioritize"));

    // 4. A stale, out-of-order edit (older revision, older content) is dropped.
    let outcome = rt
        .block_on(task.store.apply_linear_observation(
            &task.task.id,
            edit(
                "2026-07-15T00:30:00.000Z",
                &task.task.plan.title,
                &task.task.plan.description,
            ),
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
    let outcome = rt
        .block_on(
            task.store
                .apply_linear_observation(&task.task.id, restored, now),
        )
        .expect("explicit return to the original definition");
    assert!(outcome.content_steer_applied);
    let steers = rt.block_on(task.store.task_steers(&task.task.id)).unwrap();
    assert_eq!(steers.len(), 3);
    assert!(steers[2].text.contains(&task.task.plan.title));
}
