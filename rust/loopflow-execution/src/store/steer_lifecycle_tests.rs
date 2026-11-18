//! Provider lifetime and agent replacement over saved steer state.
use crate::process::attachment;
use crate::store::StoreError;
use loopflow_storage::sqlite::sessions::TerminalRetention::LiveTurn;
use loopflow_storage::steer::NativeBoundary;
use loopflow_storage::test_fixture::{hold, native_steer as native_fixture, steer as fixture};
const ACTIVATE: NativeBoundary<'static> = NativeBoundary::Activate { source: "startup" };
const CLEAR: NativeBoundary<'static> = NativeBoundary::Select { source: "clear" };
#[test]
fn pending_requests_hold_the_terminal_until_the_current_agent_settles_them() {
    let (_home, store, _adapter) = fixture();
    let first = store.session_attachment("session").unwrap().unwrap();
    let terminal = hold(&store);
    let mut activity = loopflow_storage::session::SessionActivity {
        preview: None,
        observed_at: 1,
        open_tools: 0,
        pending_input: 1,
        yielded: true,
    };
    store
        .record_session_activity("session", &first, &activity)
        .unwrap();
    for viewed in [false, true] {
        store
            .record_viewer_hold(
                &terminal,
                viewed.then_some(loopflow_storage::sqlite::ViewerHold::Viewed(1)),
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                r#"UPDATE session_activity SET program_status='{"records":[{"state":"idle"}]}' WHERE session_id='session'"#,
                [],
            )
            .unwrap();
        assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    }
    let next =
        attachment::replace_session_agent_process(&store, "session", &first, None, || Ok(true))
            .unwrap();
    // The replaced agent's requests ended with it, and its writes are stale.
    assert_eq!(store.terminal_retention(&terminal).unwrap(), None);
    store
        .record_session_activity("session", &first, &activity)
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), None);
    store
        .record_session_activity("session", &next, &activity)
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    activity.pending_input = 0;
    store
        .record_session_activity("session", &next, &activity)
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), None);
}

#[test]
fn silent_native_turns_hold_the_terminal_until_their_agent_is_replaced() {
    let (_home, store, adapter, terminal) = native_fixture();
    let first = store.session_attachment("session").unwrap().unwrap();
    store
        .record_native_turn(&adapter, "native", false, None)
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    // No editor input or open tool is needed to keep an unyielded turn.
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE held_terminals SET viewed=1 WHERE id=?1",
            [&terminal],
        )
        .unwrap();
        conn.execute(
            r#"UPDATE session_activity SET program_status='{"records":[{"state":"idle"}]}' WHERE session_id='session'"#,
            [],
        ).unwrap();
    }
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    // The turn's stop belongs to its provider.
    store
        .record_native_turn(&adapter, "native", true, Some(1))
        .unwrap();
    // A viewed prompt also cannot settle its provider-owned watch.
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    store
        .record_native_turn(&adapter, "native", true, Some(0))
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), None);
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE held_terminals SET viewed=0 WHERE id=?1",
            [&terminal],
        )
        .unwrap();
    store
        .record_native_turn(&adapter, "native", false, None)
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    assert!(store
        .record_native_turn(&adapter, "child", true, Some(0))
        .is_err());
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    let replacement =
        attachment::replace_session_agent_process(&store, "session", &first, None, || Ok(true))
            .unwrap();
    // Activity from the old provider cannot keep its replacement alive.
    assert_eq!(store.terminal_retention(&terminal).unwrap(), None);
    assert!(store
        .record_native_turn(&adapter, "native", true, Some(1))
        .is_err());
    let current = store
        .reserve_steer_adapter("session", &replacement)
        .unwrap();
    store
        .with_provider_context(&current, "native", ACTIVATE, || Ok(()))
        .unwrap();
    store
        .record_native_turn(&current, "native", true, Some(1))
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    assert!(store
        .record_native_turn(&adapter, "native", true, Some(0))
        .is_err());
    assert_eq!(store.terminal_retention(&terminal).unwrap(), Some(LiveTurn));
    store
        .record_native_turn(&current, "native", true, Some(0))
        .unwrap();
    assert_eq!(store.terminal_retention(&terminal).unwrap(), None);
}

#[test]
fn a_replaced_agent_process_cannot_select_any_conversation() {
    let (_home, store, stale) = fixture();
    let claim = store.session_attachment("session").unwrap().unwrap();
    let next =
        attachment::replace_session_agent_process(&store, "session", &claim, None, || Ok(true))
            .unwrap();
    for thread in ["native", "foreign"] {
        assert!(matches!(
            store.with_provider_context(&stale, thread, CLEAR, || Ok(())),
            Err(StoreError::InvalidAuthority(_))
        ));
    }
    let current = store.reserve_steer_adapter("session", &next).unwrap();
    store
        .with_provider_context(&current, "next", CLEAR, || Ok(()))
        .unwrap();
}
