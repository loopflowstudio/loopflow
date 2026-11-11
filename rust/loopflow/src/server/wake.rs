//! Task updates for idle managed conversations, pushed by the Machine server.
//! Each managed Session's boundaries register it here; when the store's
//! `task_updates` revision moves, one watcher reads every idle registration
//! and wakes the conversations it has news for. No process or clock per
//! Session: Claude takes the update in its cross-session inbox, Codex as a
//! turn its app-server starts. A conversation the server cannot reach hears it
//! at its next boundary.
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::json;

use crate::server::wire::ClaudeInbox;
use crate::steer::SteerAdapter;
use crate::store::sqlite::SqliteStore;

/// How long the watcher waits for a store change before reading the revision
/// anyway: the change signal can miss a checkpoint or a Machine's sleep.
const UNSIGNALLED: Duration = Duration::from_secs(60);
const SEND_DEADLINE: Duration = Duration::from_secs(2);
/// A Codex turn start loads nothing new; it answers once the turn exists.
const CODEX_DEADLINE: Duration = Duration::from_secs(15);

/// How the server reaches a conversation that is not in a turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Wake {
    Claude(ClaudeInbox),
    /// The Session's private app-server.
    Codex(PathBuf),
}

#[derive(Debug, Clone)]
struct Registration {
    adapter: SteerAdapter,
    thread: String,
    wake: Wake,
    /// The last boundary was a turn end.
    idle: bool,
}

static REGISTRATIONS: LazyLock<Mutex<HashMap<String, Registration>>> =
    LazyLock::new(Default::default);

fn registrations() -> std::sync::MutexGuard<'static, HashMap<String, Registration>> {
    REGISTRATIONS.lock().expect("wake registrations poisoned")
}

/// Record what a managed Session's boundary says about reaching it. A
/// conversation with no way to wake it is forgotten.
pub(super) fn register(adapter: &SteerAdapter, thread: &str, wake: Option<Wake>, idle: bool) {
    let mut registered = registrations();
    match wake {
        Some(wake) => {
            registered.insert(
                adapter.session.clone(),
                Registration {
                    adapter: adapter.clone(),
                    thread: thread.to_owned(),
                    wake,
                    idle,
                },
            );
        }
        None => {
            registered.remove(&adapter.session);
        }
    }
}

/// Wake `session` now if it is registered, idle and has news: a turn just
/// ended, so an update that arrived during it waits no longer. Blocking.
pub(super) fn after_turn(store: &SqliteStore, session: &str) {
    let registration = registrations().get(session).cloned();
    if let Some(registration) = registration.filter(|registration| registration.idle) {
        wake(store, &registration);
    }
}

/// Start the Machine's one watcher. It runs as long as the server.
pub(super) fn start() {
    let spawned = std::thread::Builder::new()
        .name("task-update-wake".into())
        .spawn(|| {
            crate::journal::request::isolate(watch);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "Task updates wait for each conversation's next turn");
    }
}

fn watch() {
    let path = crate::store::default_db_path();
    let mut changes = crate::store::changes::StoreChanges::watch(&path);
    let mut store = None;
    let mut seen = None;
    loop {
        changes.wait(UNSIGNALLED);
        if registrations()
            .values()
            .all(|registration| !registration.idle)
        {
            continue;
        }
        if store.is_none() && path.exists() {
            store = SqliteStore::new(&path)
                .inspect_err(|error| tracing::warn!(%error, "Task update watcher has no store"))
                .ok();
        }
        let Some(open) = &store else {
            continue;
        };
        let revision = match open.task_update_revision() {
            Ok(revision) => revision,
            Err(error) => {
                tracing::warn!(%error, "Task update revision unread");
                store = None;
                continue;
            }
        };
        if seen == Some(revision) {
            continue;
        }
        seen = Some(revision);
        let idle: Vec<Registration> = registrations()
            .values()
            .filter(|registration| registration.idle)
            .cloned()
            .collect();
        for registration in idle {
            wake(open, &registration);
        }
    }
}

/// Deliver `registration`'s update, if any, recorded as dispatched first. A
/// conversation that cannot be reached is forgotten; its next boundary
/// registers it again and hears the update there.
fn wake(store: &SqliteStore, registration: &Registration) {
    let session = &registration.adapter.session;
    let (adapter, thread) = (&registration.adapter, registration.thread.as_str());
    let delivered = (|| -> Result<bool> {
        match &registration.wake {
            Wake::Claude(inbox) => {
                let (mut socket, token) = open_inbox(inbox)?;
                let Some(text) = crate::harness::task_updates::claim(store, adapter, thread)?
                else {
                    return Ok(false);
                };
                send_inbox(&mut socket, &token, thread, &text)?;
            }
            Wake::Codex(endpoint) => {
                let Some(text) = crate::harness::task_updates::claim(store, adapter, thread)?
                else {
                    return Ok(false);
                };
                start_codex_turn(endpoint, thread, &text)?;
            }
        }
        Ok(true)
    })();
    match delivered {
        Ok(true) => {
            // A woken conversation is in a turn until its Stop says otherwise.
            if let Some(current) = registrations().get_mut(session) {
                current.idle = false;
            }
        }
        Ok(false) => {}
        Err(error) => {
            tracing::warn!(session, error = %format!("{error:#}"), "idle conversation not woken; its next turn hears the update");
            registrations().remove(session);
        }
    }
}

/// Connect before claiming, so an unreachable conversation keeps its update
/// for its next boundary.
fn open_inbox(inbox: &ClaudeInbox) -> Result<(UnixStream, String)> {
    let token = std::fs::read_to_string(&inbox.key)
        .with_context(|| format!("Claude inbox key {} unreadable", inbox.key.display()))?;
    let socket = UnixStream::connect(&inbox.socket)
        .with_context(|| format!("Claude inbox {} unreachable", inbox.socket.display()))?;
    socket.set_write_timeout(Some(SEND_DEADLINE))?;
    Ok((socket, token.trim().to_owned()))
}

fn start_codex_turn(endpoint: &std::path::Path, thread: &str, text: &str) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        tokio::time::timeout(
            CODEX_DEADLINE,
            crate::harness::codex_connection::deliver_update(endpoint, thread, text),
        )
        .await
        .map_err(|_| anyhow!("Codex turn start timed out"))?
    })
}

/// Claude's own peer envelope, so its transcript attributes the message to
/// lf instead of rendering it as something the person typed. `next` runs it
/// as the conversation's next turn.
fn send_inbox(socket: &mut UnixStream, token: &str, thread: &str, text: &str) -> Result<()> {
    let body = text
        .strip_prefix("<lf:task-update>\n")
        .and_then(|body| body.strip_suffix("\n</lf:task-update>"))
        .unwrap_or(text);
    let content = format!(
        "<cross-session-message from=\"loopflow\" from-name=\"Loopflow worker progress\">\n{}\n</cross-session-message>",
        body.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    );
    let bytes = format!(
        "{}\n{}\n",
        json!({"type":"auth","token":token}),
        json!({"type":"user","session_id":thread,"uuid":uuid::Uuid::new_v4().to_string(),
            "priority":"next","from":"loopflow","message":{"role":"user","content":content}})
    );
    // No acknowledgement exists on this connection: a write is not
    // consumption, and the dispatched receipt is never replayed.
    socket.write_all(bytes.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{after_turn, register, Wake};
    use crate::server::wire::ClaudeInbox;
    use crate::steer::{NativeBoundary, SteerAdapter};
    use crate::store::sqlite::SqliteStore;
    use std::io::Read;
    use std::os::unix::net::UnixListener;

    /// An interactive Session bound to a Task, whose provider selected
    /// conversation `native`, with a comment already in its history.
    fn fixture(
        session: &str,
    ) -> (
        tempfile::TempDir,
        SqliteStore,
        SteerAdapter,
        crate::durable::TaskId,
    ) {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session(session, &crate::session_record::new_artifact_key());
        let process = store.test_lf_process();
        let claim = store
            .claim_session_attachment(session, None, &process, false)
            .unwrap();
        store
            .record_agent_process_identity(session, &claim, std::process::id(), 1)
            .unwrap();
        let adapter = store.reserve_steer_adapter(session, &claim).unwrap();
        store
            .with_provider_context(
                &adapter,
                "native",
                NativeBoundary::Activate { source: "startup" },
                || Ok(()),
            )
            .unwrap();
        let task = crate::durable::TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/wake").unwrap(),
                id: task.clone(),
                project_id: None,
                title: "Wake".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
                [session],
            )
            .unwrap();
        let captured = store.session(session).unwrap().unwrap().captured;
        store.bind_session(session, captured, &task).unwrap();
        // The conversation's first boundary records what it already knows.
        assert!(crate::harness::task_updates::read(&store, session)
            .unwrap()
            .is_none());
        (home, store, adapter, task)
    }

    fn comment(store: &SqliteStore, task: &crate::durable::TaskId, id: &str, body: &str) {
        store
            .append_task_comment(
                task,
                &crate::work::task::comment::TaskComment {
                    id: id.into(),
                    body: body.into(),
                    author: crate::work::task::comment::TaskCommentAuthor::Person {
                        name: Some("jack".into()),
                    },
                    created_at: Some("2026-11-11T09:00:00Z".into()),
                },
            )
            .unwrap();
    }

    #[test]
    fn an_idle_claude_conversation_takes_the_update_in_its_inbox_once() {
        let (home, store, adapter, task) = fixture("wake-claude");
        let socket = home.path().join("inbox.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let key = home.path().join("inbox.key");
        std::fs::write(&key, "peer-key\n").unwrap();
        let inbox = Wake::Claude(ClaudeInbox { socket, key });
        register(&adapter, "native", Some(inbox), true);
        comment(&store, &task, "c-1", "use the staging bucket");
        after_turn(&store, "wake-claude");
        let (mut stream, _) = listener.accept().unwrap();
        let mut wire = String::new();
        stream.read_to_string(&mut wire).unwrap();
        let frames: Vec<serde_json::Value> = wire
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            frames[0],
            serde_json::json!({"type":"auth","token":"peer-key"})
        );
        assert_eq!(frames[1]["session_id"], "native");
        assert_eq!(frames[1]["priority"], "next");
        let content = frames[1]["message"]["content"].as_str().unwrap();
        assert!(content.starts_with("<cross-session-message from=\"loopflow\""));
        assert!(content.contains("New comment from jack: use the staging bucket"));
        // Woken, it is in a turn: nothing more goes out until its Stop.
        comment(&store, &task, "c-2", "and the eu region");
        after_turn(&store, "wake-claude");
        register(
            &adapter,
            "native",
            Some(Wake::Claude(ClaudeInbox {
                socket: home.path().join("inbox.sock"),
                key: home.path().join("inbox.key"),
            })),
            true,
        );
        after_turn(&store, "wake-claude");
        let (mut stream, _) = listener.accept().unwrap();
        let mut wire = String::new();
        stream.read_to_string(&mut wire).unwrap();
        assert!(wire.contains("and the eu region"));
        assert!(
            !wire.contains("staging bucket"),
            "a delivered update is not repeated"
        );
    }

    #[test]
    fn an_unreachable_inbox_leaves_the_update_for_the_next_boundary() {
        let (home, store, adapter, task) = fixture("wake-gone");
        let inbox = Wake::Claude(ClaudeInbox {
            socket: home.path().join("gone.sock"),
            key: home.path().join("gone.key"),
        });
        std::fs::write(home.path().join("gone.key"), "peer-key").unwrap();
        register(&adapter, "native", Some(inbox), true);
        comment(&store, &task, "c-1", "use the staging bucket");
        after_turn(&store, "wake-gone");
        assert!(super::registrations().get("wake-gone").is_none());
        let update = crate::harness::task_updates::read(&store, "wake-gone")
            .unwrap()
            .expect("still news at the next boundary");
        assert!(update.text.contains("use the staging bucket"));
    }
}
