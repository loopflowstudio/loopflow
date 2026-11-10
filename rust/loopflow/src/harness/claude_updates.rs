//! One asynchronous native hook observes Task work for its owning provider.
//! Claude supplies its inbox and ephemeral child token to that hook. They never
//! leave process memory. Provider capability fences survive attachment-only moves.
use anyhow::Result;

use super::claude_inbox::Inbox;
use super::task_updates::TaskUpdate;
use crate::steer::{NativeBoundary, SteerAdapter};
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

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
    let mut recovery = super::claude_recovery::Recovery::default();
    let recovery_adapter = adapter.clone();
    super::task_updates::watch(
        home,
        input,
        adapter,
        thread,
        |store| {
            if let Some(path) = input["transcript_path"].as_str() {
                recovery.tick(
                    store,
                    &recovery_adapter,
                    thread,
                    std::path::Path::new(path),
                    &socket,
                    &token,
                )?;
            }
            Ok(())
        },
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
    let mut inbox = Inbox::connect(path)?;
    // Use Claude's native peer envelope so its transcript can attribute and
    // collapse the message instead of rendering a user-style fallback block.
    let body = update
        .text
        .strip_prefix("<lf:task-update>\n")
        .and_then(|body| body.strip_suffix("\n</lf:task-update>"))
        .unwrap_or(&update.text);
    let content = format!(
        "<cross-session-message from=\"loopflow\" from-name=\"Loopflow worker progress\">\n{}\n</cross-session-message>",
        body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    );
    store.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
        if !store.record_task_update(
            &adapter.session,
            &update.key,
            "dispatched",
            &update.observation,
        )? {
            return Ok(());
        }
        // No acknowledgement exists on this connection. A successful write is
        // not consumption; retain uncertainty and never replay it on reconnect.
        inbox.send(token, thread, &uuid::Uuid::new_v4().to_string(), &content)
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
            observation: serde_json::json!({}),
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
        assert_eq!(frames[1]["message"]["content"],
            "<cross-session-message from=\"loopflow\" from-name=\"Loopflow worker progress\">\ngate: failed\n</cross-session-message>");
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
            observation: serde_json::json!({}),
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
