use crate::process::attachment;
use loopflow_storage::test_fixture::process as insert_process;

#[test]
fn stale_orphan_observation_cannot_settle_a_transferred_agent() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    let session = store.test_session(
        "conversation",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let a = insert_process(&store, 1, 1);
    let b = insert_process(&store, 2, 2);
    let first = attachment::claim_session_attachment(&store, &session.id, None, &a, true).unwrap();
    store
        .record_agent_process_identity(&session.id, &first, 4242, 123)
        .unwrap();
    let observed = store.agent_processes().unwrap().remove(0);
    attachment::claim_session_attachment(&store, &session.id, Some(&first), &b, false).unwrap();
    assert!(
        !attachment::settle_agent_process(&store, &observed, || panic!("stale signal")).unwrap()
    );
    let current = store.agent_processes().unwrap().remove(0);
    assert!(attachment::settle_agent_process(&store, &current, || {
        // Settlement holds the attachment fence, not the SQLite mutex.
        assert_eq!(store.agent_state(&current.id)?.as_ref(), Some(&current));
        Ok(())
    })
    .unwrap());
    let retained = store
        .agent_process(&first.agent_process_id)
        .unwrap()
        .unwrap();
    assert_eq!(retained.pid, Some(4242));
    assert_eq!(retained.parent_lf_process_id, Some(a));
    assert!(retained.completed_at.is_some());
}

#[test]
fn settling_a_dead_headless_run_moves_it_to_history() {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
    let mut completed = Vec::new();
    for (number, id, interactive) in [(1, "run", false), (2, "conversation", true)] {
        let session = store.test_session(id, &loopflow_storage::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=?2 WHERE id=?1",
                rusqlite::params![session.id, interactive],
            )
            .unwrap();
        let owner = insert_process(&store, number, 1);
        let attachment =
            attachment::claim_session_attachment(&store, &session.id, None, &owner, true).unwrap();
        store
            .record_agent_process_identity(&session.id, &attachment, 4242, 123)
            .unwrap();
        let agent = store
            .agent_processes()
            .unwrap()
            .into_iter()
            .find(|agent| agent.session_id == id)
            .unwrap();
        assert!(attachment::settle_agent_process(&store, &agent, || Ok(())).unwrap());
        completed.push(store.session(id).unwrap().unwrap().completed_at.is_some());
    }
    assert_eq!(completed, [true, false]);
}
