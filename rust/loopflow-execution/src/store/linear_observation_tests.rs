//! Linear observations stay adapter-internal; ingestion preserves ordered direction.
use loopflow_storage::planning::{TrackerComment, TrackerObservation};
use time::OffsetDateTime;

fn edit(revision: &str, title: &str, description: &str) -> TrackerObservation {
    TrackerObservation {
        revision: revision.into(),
        title: title.into(),
        description: description.into(),
        comments: vec![],
    }
}

#[test]
fn linear_edits_and_comments_stream_into_task_control_exactly_once() {
    let (_home, store, mut task, pr) = loopflow_storage::test_fixture::task();
    task.title = "Original title".into();
    task.description = "Original description".into();
    store.seed_task(&task, &pr, None).unwrap();
    let seeded = store.task_linear_observation(&task.id).unwrap().unwrap();
    assert_eq!(seeded.last_title, task.title);
    assert_eq!(seeded.last_revision, "");
    let now = OffsetDateTime::now_utc();
    let restored = edit("2026-07-15T02:00:00.000Z", &task.title, &task.description);
    let observation = edit("2026-07-15T01:00:00.000Z", "New title", "New body");
    let outcome = crate::store::ownership::apply_linear_observation(
        &store,
        &task.id,
        &observation,
        now,
        false,
    )
    .unwrap();
    assert!(!outcome.baselined);
    assert!(outcome.content_steer_applied);
    let steers = store.task_steers(&task.id).unwrap();
    assert_eq!(steers.len(), 1);
    assert!(steers[0].text.contains("New title"));
    assert!(
        !crate::store::ownership::apply_linear_observation(
            &store,
            &task.id,
            &observation,
            now,
            false
        )
        .unwrap()
        .content_steer_applied
    );

    let mut observation = observation;
    observation.comments.push(TrackerComment {
        id: "c-1".into(),
        revision: Some("2026-07-15T01:00:00.000Z".into()),
        created_at: None,
        body: "please prioritize".into(),
        author_id: Some("person".into()),
        author_name: Some("Maya".into()),
    });
    for expected in [1, 0] {
        let outcome = crate::store::ownership::apply_linear_observation(
            &store,
            &task.id,
            &observation,
            now,
            false,
        )
        .unwrap();
        assert_eq!(outcome.follow_ups_created.len(), expected);
    }
    assert_eq!(
        store.task_comment_thread(&task.id).unwrap().thread.comments[0].body,
        "please prioritize"
    );
    let steers = store.task_steers(&task.id).unwrap();
    assert_eq!(steers.len(), 2, "one edit + one comment, no dup");
    assert!(steers[1].text.contains("please prioritize"));

    let stale = edit("2026-07-15T00:30:00.000Z", &task.title, &task.description);
    assert!(
        !crate::store::ownership::apply_linear_observation(&store, &task.id, &stale, now, false)
            .unwrap()
            .content_steer_applied
    );
    let cursor = store.task_linear_observation(&task.id).unwrap().unwrap();
    assert_eq!(cursor.last_title, "New title");
    assert_eq!(cursor.last_revision, "2026-07-15T01:00:00.000Z");
    assert!(
        crate::store::ownership::apply_linear_observation(&store, &task.id, &restored, now, false)
            .unwrap()
            .content_steer_applied
    );
    let steers = store.task_steers(&task.id).unwrap();
    assert_eq!(steers.len(), 3);
    assert!(steers[2].text.contains(&task.title));
}
