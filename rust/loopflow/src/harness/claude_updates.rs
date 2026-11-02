//! One asynchronous native hook observes Task work for its owning provider.
//! Claude supplies its inbox and ephemeral child token to that hook. They never
//! leave process memory. Provider capability fences survive attachment-only moves.
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use anyhow::Result;
use serde_json::json;

use super::task_updates::TaskUpdate;
use crate::steer::{NativeBoundary, SteerAdapter};
use crate::store::sqlite::SqliteStore;
use crate::store::{StoreError, StoreResult};

pub(crate) fn watch(home: &std::path::Path, input: &serde_json::Value) -> Result<()> {
    let (Ok(socket), Ok(token), Some(thread), Ok(adapter)) = (
        std::env::var("CLAUDE_CODE_MESSAGING_SOCKET"),
        std::env::var("CLAUDE_CODE_MESSAGING_TOKEN"),
        input["session_id"].as_str(),
        std::env::var(crate::steer::PROVIDER_CONTEXT_ENV),
    ) else {
        return Ok(());
    };
    let adapter: SteerAdapter = serde_json::from_str(&adapter)?;
    super::task_updates::watch(
        home,
        input,
        adapter,
        thread,
        |store, adapter, thread, update| deliver(store, adapter, thread, &socket, &token, update),
    )
}

fn deliver(
    store: &SqliteStore,
    adapter: &SteerAdapter,
    thread: &str,
    path: &str,
    token: &str,
    update: &TaskUpdate,
) -> StoreResult<()> {
    let io_error = |error: std::io::Error| StoreError::InvalidData(error.to_string());
    let mut socket = UnixStream::connect(path).map_err(io_error)?;
    socket
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(io_error)?;
    let bytes = format!(
        "{}\n{}\n",
        json!({"type":"auth","token":token}),
        json!({"type":"user","session_id":thread,"uuid":uuid::Uuid::new_v4().to_string(),
            "priority":"next","from":"loopflow", "message":{"role":"user","content":update.text}})
    );
    store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
        if !store.record_task_update(&adapter.session, &update.key, "dispatched")? {
            return Ok(());
        }
        socket.write_all(bytes.as_bytes()).map_err(io_error)?;
        // No acknowledgement exists on this connection. A successful write is
        // not consumption; retain uncertainty and never replay it on reconnect.
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::deliver;
    use crate::harness::task_updates::TaskUpdate;
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;
    use std::io::Read;
    use std::os::unix::net::UnixListener;

    #[test]
    fn task_update_inbox_preserves_identity_and_uncertainty() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        store.test_session("reader", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment("reader", None, &process, true)
            .unwrap();
        store
            .record_session_connection("reader", &attachment, "fixture", &"native-thread".into())
            .unwrap();
        let adapter = store.reserve_steer_adapter("reader", &attachment).unwrap();
        let path = home.path().join("inbox.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let token = uuid::Uuid::new_v4().to_string();
        let update = TaskUpdate {
            key: "failure".into(),
            text: "gate: failed".into(),
        };
        deliver(
            &store,
            &adapter,
            "native-thread",
            path.to_str().unwrap(),
            &token,
            &update,
        )
        .unwrap();
        let (mut socket, _) = listener.accept().unwrap();
        let mut wire = String::new();
        socket.read_to_string(&mut wire).unwrap();
        let frames: Vec<serde_json::Value> = wire
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(frames[0]["token"].as_str() == Some(token.as_str()));
        assert_eq!(frames[1]["session_id"], "native-thread");
        assert_eq!(frames[1]["priority"], "next");
        assert_eq!(frames[1]["message"]["content"], "gate: failed");
        deliver(
            &store,
            &adapter,
            "native-thread",
            path.to_str().unwrap(),
            &token,
            &update,
        )
        .unwrap();
        let (mut socket, _) = listener.accept().unwrap();
        let mut wire = String::new();
        socket.read_to_string(&mut wire).unwrap();
        assert!(wire.is_empty(), "uncertain delivery is never replayed");
        let wrong = TaskUpdate {
            key: "different".into(),
            text: "stopped".into(),
        };
        assert!(deliver(
            &store,
            &adapter,
            "another-thread",
            path.to_str().unwrap(),
            &token,
            &wrong
        )
        .is_err());
        assert!(!store
            .session_history("reader", 0, 0)
            .unwrap()
            .iter()
            .any(|event| event.payload["key"] == "different"));
    }
}
