//! Everything orchestration reads is in `loopflow.db`: deleting `history.db`
//! loses history and usage, never what the Session list, Graph routing or a
//! failed step sees.

use loopflow_storage::session::SessionFilter;
use loopflow_storage::sqlite::SqliteStore;
use serde_json::json;

/// Every state read a projection, routing or a failed step makes about one
/// Session and its input.
fn state(store: &SqliteStore, session: &str, input: &str) -> String {
    let filter = SessionFilter {
        headless: true,
        ..SessionFilter::default()
    };
    let summaries =
        crate::store::session_projection::session_summaries(store, &filter, 100).unwrap();
    let thread = "thread".into();
    format!(
        "{summaries:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
        store.session(session).unwrap(),
        store.input_answered_turn(input).unwrap(),
        store.input_turn_end(input).unwrap(),
        store.recorded_turn_end(session, &thread, "turn").unwrap(),
        store.provider_transition(session).unwrap(),
    )
}

#[test]
fn state_reads_answer_the_same_with_history_db_deleted() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let store = crate::store::opening::open_ephemeral(&database).unwrap();
    let input = loopflow_storage::session_record::new_artifact_key();
    let session = store.test_session("conversation", &input);
    let thread = "thread".into();
    loopflow_storage::test_fixture::record_captured_start(&store, &session.id, "turn");
    store.record_turn_usage(
        &session.id,
        &thread,
        "turn",
        2,
        &json!({"provider":"claude","final":true,"usage":{"input_tokens":3,"output_tokens":4}}),
    );
    let completed = json!({"status":"completed"});
    store.record_turn_completion(&session.id, &thread, "turn", 3, &completed);
    store
        .record_answered_turn_end(session.captured.unwrap(), &thread, "turn", 3, &completed)
        .unwrap();
    store
        .execute_history(
            "INSERT INTO session_history(session_id,receipt_key,captured_event,observed_at,payload)
             VALUES(?1,?2||':manifest.json',?3,1,'{\"source\":\"manifest.json\",\"evidence\":{}}')",
            rusqlite::params![session.id, input, session.captured],
        )
        .unwrap();
    assert_eq!(store.session_turns(&session.id).unwrap().len(), 1);
    let with_history = state(&store, &session.id, &input);
    assert!(with_history.contains("completed"), "{with_history}");
    drop(store);

    let history = loopflow_storage::history::history_path(&database);
    for suffix in ["", "-wal", "-shm"] {
        let file = format!("{}{suffix}", history.display());
        let _ = std::fs::remove_file(file);
    }
    let store = crate::store::opening::open_ephemeral(&database).unwrap();
    assert!(store.session_turns(&session.id).unwrap().is_empty());
    assert_eq!(state(&store, &session.id, &input), with_history);
}
