use loopflow_model::durable::TaskId;
use loopflow_storage::session_record::new_artifact_key;
use loopflow_storage::work::task::NewTask;

#[test]
fn membership_moves_preserve_work_and_capture_owners() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    let repo = tempfile::tempdir().unwrap();
    let foreign_repo = tempfile::tempdir().unwrap();
    let canonical = repo.path().canonicalize().unwrap();
    let path = canonical.to_str().unwrap();
    let repository = crate::repository::location::ensure_repository(&store, path).unwrap();
    let product =
        crate::work::wave::bootstrap::ensure_wave_project(&store, path, "product").unwrap();
    let other = crate::work::wave::bootstrap::ensure_wave_project(&store, path, "other").unwrap();
    let foreign = crate::work::wave::bootstrap::ensure_wave_project(
        &store,
        foreign_repo.path().to_str().unwrap(),
        "foreign",
    )
    .unwrap();
    let task = store
        .create_task(&NewTask {
            id: TaskId::new(),
            repository_id: repository,
            project_id: None,
            title: "One Task".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    let session = store.test_session("conversation", &new_artifact_key());
    let bound =
        crate::store::sessions::bind_session(&store, &session.id, session.captured, &task.id)
            .unwrap();
    let captured = crate::store::sessions::replace_session_input(
        &store,
        bound.captured,
        loopflow_storage::session::LfSession {
            artifact_key: Some(new_artifact_key()),
            ..bound
        },
        None,
    )
    .unwrap();
    let before = store.session_history(&session.id, 0, 0).unwrap();
    {
        let _snapshot = store.read_snapshot().unwrap();
        store
            .validate_task_move(&task.id, None, Some(&product.id))
            .unwrap();
        assert!(store.task(&task.id).unwrap().unwrap().project_id.is_none());
    }
    store.move_task(&task.id, None, Some(&product.id)).unwrap();
    assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), before);
    assert_eq!(
        store.session(&session.id).unwrap().unwrap().wave_id,
        Some(product.wave_id.clone())
    );
    // A stale attachment's inherited Wave hint cannot rewrite current membership.
    let next = crate::store::sessions::replace_session_input(
        &store,
        captured.captured,
        loopflow_storage::session::LfSession {
            artifact_key: Some(new_artifact_key()),
            ..captured
        },
        None,
    )
    .unwrap();
    assert_eq!(next.wave_id, Some(product.wave_id.clone()));
    assert!(store.move_task(&task.id, None, Some(&other.id)).is_err());
    assert!(store
        .move_task(&task.id, Some(&product.id), Some(&foreign.id))
        .is_err());
    store
        .move_task(&task.id, Some(&product.id), Some(&product.id))
        .unwrap();
    store.move_task(&task.id, Some(&product.id), None).unwrap();
    let final_input = crate::store::sessions::replace_session_input(
        &store,
        next.captured,
        loopflow_storage::session::LfSession {
            artifact_key: Some(new_artifact_key()),
            ..next
        },
        None,
    )
    .unwrap();
    assert!(final_input.wave_id.is_none());
    let conn = store.conn.lock().unwrap();
    let owners = conn.prepare("SELECT wave_id FROM session_events WHERE session_id='conversation' AND kind='captured' ORDER BY seq").unwrap()
        .query_map([], |row| row.get::<_, Option<String>>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        owners,
        vec![None, None, Some(product.wave_id.to_string()), None]
    );
    drop(conn);
    let retained = store.task(&task.id).unwrap().unwrap();
    assert_eq!(retained.repository_id, task.repository_id);
    assert!(retained.project_id.is_none());
    assert_eq!(
        store.bound_sessions().unwrap()[0].session_id,
        session.id.as_str()
    );
}
