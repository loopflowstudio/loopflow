use crate::process::attachment;
use loopflow_storage::query_plans::{growing_scans, statements};
use loopflow_storage::test_fixture::retirement as fixture;

#[test]
fn session_history_reads_plan_by_window() {
    let (_home, store, task, _viewer) = fixture();
    let input = crate::session_record::new_artifact_key();
    store.test_session("history", &input);
    store.record_turn_completion(
        "history",
        &"thread".into(),
        "turn",
        1,
        &serde_json::json!({"status":"completed"}),
    );
    let mut traced = statements(&store, || {
        crate::store::sessions::recent_conversation_history(&store, None, None, None, 100, 50)
            .unwrap();
        store
            .pending_conversation_history(None, None, None, 100, true)
            .unwrap();
        crate::store::sessions::input_history(&store, &input).unwrap();
    });
    // A Task drill's window is the Task (Desktop Task history, `task.context`).
    // Its selector resolution scans `tasks` for short-id prefixes once per
    // drill, whatever the history holds; only the history read is checked.
    let drill = statements(&store, || {
        store
            .pending_conversation_history(None, None, Some(task.as_str()), 0, true)
            .unwrap();
    });
    let history = drill
        .into_iter()
        .filter(|sql| sql.starts_with("WITH windowed"));
    let before = traced.len();
    traced.extend(history);
    assert!(traced.len() > before, "the drill reads history");
    let scans = growing_scans(&store, &traced);
    assert!(scans.is_empty(), "{}", scans.join("\n"));
}

#[test]
fn a_hook_reading_its_steers_and_task_updates_plans_by_index() {
    let (_home, store, task, _viewer) = fixture();
    store.test_session("hooked", &crate::session_record::new_artifact_key());
    let process = store.test_lf_process();
    let claim = attachment::claim_session_attachment(&store, "hooked", None, &process).unwrap();
    store
        .record_agent_process_identity("hooked", &claim, std::process::id(), 1)
        .unwrap();
    let adapter = store.reserve_steer_adapter("hooked", &claim).unwrap();
    store
        .with_provider_context(
            &adapter,
            "thread",
            crate::steer::NativeBoundary::Activate { source: "startup" },
            || Ok(()),
        )
        .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET cwd='/repo.task/src',interactive=1 WHERE id='hooked'",
            [],
        )
        .unwrap();
    store
        .save_session_steer("hooked", "person", "Look here")
        .unwrap();
    let traced = statements(&store, || {
        // The first reading writes the Task's baseline; the next update
        // compares against it.
        crate::harness::task_updates::write(&store, "hooked").unwrap();
        let reading = store.task_update_reading(&task).unwrap();
        store
            .save_task_update(
                &task,
                reading.as_deref(),
                reading.as_deref().unwrap(),
                &[("update:-:fixture".into(), "Task update".into())],
            )
            .unwrap();
        store.baseline_task_updates("hooked").unwrap();
        let (claimed, _) = store.claim_session_steers(&adapter, "thread").unwrap();
        assert_eq!(claimed.len(), 1);
    });
    let scans = growing_scans(&store, &traced);
    assert!(scans.is_empty(), "{}", scans.join("\n"));
}
