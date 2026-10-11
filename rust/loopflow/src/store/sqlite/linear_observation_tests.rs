//! Linear observations stay adapter-internal; ingestion preserves ordered direction.
use crate::durable::TaskId;
use crate::linear::{LinearComment, LinearIssueObservation};
use crate::store::sqlite::SqliteStore;
use crate::work::task::NewTask;
use time::OffsetDateTime;

fn edit(revision: &str, title: &str, description: &str) -> LinearIssueObservation {
    LinearIssueObservation {
        revision: revision.into(),
        title: title.into(),
        description: description.into(),
        comments: vec![],
    }
}

#[test]
fn linear_edits_and_comments_stream_into_task_control_exactly_once() {
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
    let project = store
        .ensure_wave_project(&home.path().to_string_lossy(), "inbox")
        .unwrap();
    let task = store
        .create_task(&NewTask {
            id: TaskId::new(),
            project_id: project.id,
            title: "Original title".into(),
            description: "Original description".into(),
            due_date: None,
        })
        .unwrap();
    // Placement seeds the cursor from the admitted definition. No checkout or
    // Git process is needed to exercise its ingestion transaction.
    super::seed_task_linear_observation(&store.conn.lock().unwrap(), &task).unwrap();
    let seeded = store.task_linear_observation(&task.id).unwrap().unwrap();
    assert_eq!(seeded.last_title, task.title);
    assert_eq!(seeded.last_revision, "");
    let now = OffsetDateTime::now_utc();
    let restored = edit("2026-07-15T02:00:00.000Z", &task.title, &task.description);
    let observation = edit("2026-07-15T01:00:00.000Z", "New title", "New body");
    let outcome = store
        .apply_linear_observation(&task.id, &observation, now)
        .unwrap();
    assert!(!outcome.baselined);
    assert!(outcome.content_steer_applied);
    let steers = store.task_steers(&task.id).unwrap();
    assert_eq!(steers.len(), 1);
    assert!(steers[0].text.contains("New title"));
    assert!(
        !store
            .apply_linear_observation(&task.id, &observation, now)
            .unwrap()
            .content_steer_applied
    );

    let mut observation = observation;
    observation.comments.push(LinearComment {
        id: "c-1".into(),
        revision: Some("2026-07-15T01:00:00.000Z".into()),
        created_at: None,
        body: "please prioritize".into(),
        author_id: Some("person".into()),
        author_name: Some("Maya".into()),
    });
    for expected in [1, 0] {
        let outcome = store
            .apply_linear_observation(&task.id, &observation, now)
            .unwrap();
        assert_eq!(outcome.follow_ups_created.len(), expected);
    }
    assert_eq!(
        store.task_comments(&task.id).unwrap().comments[0].body,
        "please prioritize"
    );
    let steers = store.task_steers(&task.id).unwrap();
    assert_eq!(steers.len(), 2, "one edit + one comment, no dup");
    assert!(steers[1].text.contains("please prioritize"));

    let stale = edit("2026-07-15T00:30:00.000Z", &task.title, &task.description);
    assert!(
        !store
            .apply_linear_observation(&task.id, &stale, now)
            .unwrap()
            .content_steer_applied
    );
    let cursor = store.task_linear_observation(&task.id).unwrap().unwrap();
    assert_eq!(cursor.last_title, "New title");
    assert_eq!(cursor.last_revision, "2026-07-15T01:00:00.000Z");
    assert!(
        store
            .apply_linear_observation(&task.id, &restored, now)
            .unwrap()
            .content_steer_applied
    );
    let steers = store.task_steers(&task.id).unwrap();
    assert_eq!(steers.len(), 3);
    assert!(steers[2].text.contains(&task.title));
}
