//! Run the deadlock regression in a child so its watchdog does not depend on
//! the reactor (or store mutex) under test.

use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use futures_util::SinkExt;
use serde_json::json;
use tokio::net::UnixStream;
use tokio::sync::oneshot;
use tokio_tungstenite::{tungstenite::protocol::Role, tungstenite::Message, WebSocketStream};

use super::codex_history::History;
use crate::id::ExecId;
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreError;

#[test]
fn history_and_other_writers_progress_during_stalled_dispatch() {
    const CHILD: &str = "LF_TEST_DISPATCH_DEADLOCK_CHILD";
    if std::env::var_os(CHILD).is_some() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(stalled_dispatch());
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "harness::dispatch_tests::history_and_other_writers_progress_during_stalled_dispatch",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "dispatch regression child failed: {status}"
            );
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("provider dispatch deadlocked its reactor or database");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

async fn stalled_dispatch() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("history.db");
    let store = SqliteStore::open_ephemeral(&path).unwrap();
    let sql = rusqlite::Connection::open(&path).unwrap();
    sql.busy_timeout(Duration::from_millis(100)).unwrap();
    let exec = ExecId::new();
    sql.execute(
        "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
        [exec.as_str()],
    )
    .unwrap();
    store.test_session("conversation", "run_00000000000000000000000000000001");
    let driver = store
        .claim_session_driver("conversation", None, &exec, false)
        .unwrap();
    let (socket, _unread_peer) = UnixStream::pair().unwrap();
    let mut socket = WebSocketStream::from_raw_socket(socket, Role::Client, None).await;
    let (entered, ready) = oneshot::channel();
    let runtime = tokio::runtime::Handle::current();
    let sender_store = store.clone();
    let sender_driver = driver.clone();
    let sender = tokio::task::spawn_blocking(move || {
        sender_store
            .with_session_driver("conversation", &sender_driver, || {
                entered.send(()).unwrap();
                runtime.block_on(async {
                    // The peer never drains this frame. Only the runtime's timer
                    // can finish the send; synchronous history must not freeze it.
                    let outcome = tokio::time::timeout(
                        Duration::from_millis(200),
                        socket.send(Message::Binary(vec![0; 8 * 1024 * 1024].into())),
                    )
                    .await;
                    assert!(
                        outcome.is_err(),
                        "fixture socket did not apply backpressure"
                    );
                });
                Ok(())
            })
            .unwrap();
    });
    ready.await.unwrap();
    let mut history = History::default();
    history
        .record(
            &store,
            "conversation",
            Some(&driver),
            None,
            &json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":"turn"}}}),
        )
        .unwrap();
    // A distinct connection proves the Home's WAL writer is also free, not
    // merely this SqliteStore's mutex.
    sql.execute(
        "UPDATE execs SET command='unrelated write' WHERE id=?1",
        [exec.as_str()],
    )
    .unwrap();
    sender.await.unwrap();
    assert!(store
        .session_history("conversation", 0, 0)
        .unwrap()
        .iter()
        .any(|event| event.kind == SessionEventKind::Started
            && event.provider_turn.as_deref() == Some("turn")));

    let replacement = ExecId::new();
    sql.execute(
        "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,'fixture',1)",
        [replacement.as_str()],
    )
    .unwrap();
    let other_store = SqliteStore::open_ephemeral(&path).unwrap();
    let (entered, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let dispatch_store = store.clone();
    let dispatch_driver = driver.clone();
    let dispatch = std::thread::spawn(move || {
        dispatch_store
            .with_session_driver("conversation", &dispatch_driver, || {
                entered.send(()).unwrap();
                released.recv().unwrap();
                Ok(())
            })
            .unwrap();
    });
    ready.recv().unwrap();
    let (transferred, transfer) = mpsc::channel();
    let transfer_store = other_store.clone();
    let original = driver.clone();
    let claimant = std::thread::spawn(move || {
        let driver = transfer_store
            .claim_session_driver("conversation", Some(&original), &replacement, false)
            .unwrap();
        transferred.send(driver).unwrap();
    });
    // Transfer must wait for the native write, but history must not.
    assert!(transfer.recv_timeout(Duration::from_millis(100)).is_err());
    assert!(!store
        .session_history("conversation", 0, 0)
        .unwrap()
        .is_empty());
    release.send(()).unwrap();
    let replacement = transfer.recv_timeout(Duration::from_secs(2)).unwrap();
    dispatch.join().unwrap();
    claimant.join().unwrap();
    assert!(matches!(
        store.with_session_driver::<()>("conversation", &driver, || panic!("stale driver sent")),
        Err(StoreError::InvalidAuthority(_))
    ));
    other_store
        .with_session_driver("conversation", &replacement, || Ok(()))
        .unwrap();
}
