use crate::process::attachment;
use loopflow_model::id::LfProcessId;
use loopflow_storage::sqlite::provider_transitions::TransitionKind;

#[test]
fn reservations_do_not_report_success_and_observed_replacements_keep_lineage() {
    let home = tempfile::tempdir().unwrap();
    let store = loopflow_storage::test_fixture::open(&home.path().join("store.db")).unwrap();
    store.test_session(
        "conversation",
        &loopflow_storage::session_record::new_artifact_key(),
    );
    let owner = LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'test',1)",
            [&owner],
        )
        .unwrap();
    let first = attachment::claim_session_attachment(&store, "conversation", None, &owner).unwrap();
    store.execute_history(
"INSERT INTO session_history(session_id,receipt_key,observed_at,payload,captured_event)
         SELECT id,'unreadable',1,'not JSON',current_capture FROM agent_sessions WHERE id='conversation'",
        [],
    ).unwrap();
    store
        .record_session_agent_session("conversation", &first, &"native".into())
        .unwrap();
    assert!(store.provider_transition("conversation").unwrap().is_none());
    store
        .record_agent_process_identity("conversation", &first, std::process::id(), 1)
        .unwrap();
    store
        .record_session_agent_session("conversation", &first, &"native".into())
        .unwrap();
    let started = store.provider_transition("conversation").unwrap().unwrap();
    assert_eq!(started.kind, TransitionKind::Started);
    store
        .record_session_agent_session("conversation", &first, &"native".into())
        .unwrap();
    assert_eq!(
        store.provider_transition("conversation").unwrap().unwrap(),
        started
    );
    let next = attachment::replace_session_agent_process(
        &store,
        "conversation",
        &first,
        Some(&"native".into()),
        || Ok(true),
    )
    .unwrap();
    assert_eq!(
        store.provider_transition("conversation").unwrap().unwrap(),
        started
    );
    store
        .record_agent_process_identity("conversation", &next, std::process::id(), 1)
        .unwrap();
    store
        .record_session_agent_session("conversation", &next, &"native".into())
        .unwrap();
    let resumed = store.provider_transition("conversation").unwrap().unwrap();
    assert_eq!(resumed.kind, TransitionKind::Resumed);
    assert_eq!(resumed.from_agent_process, Some(first.agent_process_id));
    assert_eq!(resumed.to_agent_process, next.agent_process_id);
}
