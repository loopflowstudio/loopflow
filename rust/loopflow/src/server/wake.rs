//! Steers and Task updates for idle managed conversations, pushed by the
//! Machine server. Each managed Session's boundaries register it here; when a
//! steer is saved or the store's `task_updates` revision moves, one watcher
//! reads every idle registration and wakes the conversations it has news for.
//! No process or clock per Session: Claude takes the message in its
//! cross-session inbox, Codex as a turn its app-server starts. A conversation
//! the server cannot reach hears it at its next boundary.
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
    /// The Session's context delivery file, where rendered steers point.
    delivery: PathBuf,
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
pub(super) fn register(
    adapter: &SteerAdapter,
    thread: &str,
    delivery: &std::path::Path,
    wake: Option<Wake>,
    idle: bool,
) {
    let mut registered = registrations();
    match wake {
        Some(wake) => {
            registered.insert(
                adapter.session.clone(),
                Registration {
                    adapter: adapter.clone(),
                    thread: thread.to_owned(),
                    delivery: delivery.to_owned(),
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
        let revision = match open
            .task_update_revision()
            .and_then(|updates| Ok((updates, open.steer_revision()?)))
        {
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

/// Deliver `registration`'s pending steers and Task update, if any. The
/// update is recorded as dispatched before it is sent; the steer batch is
/// confirmed once sent, so a failed send returns it to pending for the next
/// boundary. A conversation that cannot be reached is forgotten; its next
/// boundary registers it again and hears the news there.
fn wake(store: &SqliteStore, registration: &Registration) {
    let session = &registration.adapter.session;
    let (adapter, thread) = (&registration.adapter, registration.thread.as_str());
    let delivered = (|| -> Result<bool> {
        // Connect before claiming, so an unreachable conversation keeps its
        // news for its next boundary.
        let reach = match &registration.wake {
            Wake::Claude(inbox) => Reach::Inbox(open_inbox(inbox)?),
            Wake::Codex(endpoint) => Reach::Codex(endpoint),
        };
        let steers = super::boundary::wake_steers(store, &registration.delivery, adapter, thread)?;
        let update = crate::harness::task_updates::claim(store, adapter, thread)?;
        let text = match (&steers, &update) {
            (None, None) => return Ok(false),
            (Some(steers), Some(update)) => format!("{}{update}", steers.text),
            (Some(steers), None) => steers.text.clone(),
            (None, Some(update)) => update.clone(),
        };
        match reach {
            Reach::Inbox((mut socket, token)) => send_inbox(&mut socket, &token, thread, &text)?,
            Reach::Codex(endpoint) => start_codex_turn(endpoint, thread, &text)?,
        }
        if let Some(steers) = steers {
            store.confirm_session_steers(
                adapter,
                &[steers.batch],
                super::boundary::HOOK_DEADLINE.as_secs(),
            )?;
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

enum Reach<'a> {
    Inbox((UnixStream, String)),
    Codex(&'a std::path::Path),
}

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
    let content = format!(
        "<cross-session-message from=\"loopflow\" from-name=\"Loopflow\">\n{}\n</cross-session-message>",
        text.trim_end()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
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
    use crate::context_block::ContextDelivery;
    use crate::server::wire::ClaudeInbox;
    use crate::steer::{NativeBoundary, SteerAdapter};
    use crate::store::sqlite::SqliteStore;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};

    /// An interactive Session bound to a Task, whose provider selected
    /// conversation `native`, and its delivery file.
    fn fixture(
        session: &str,
    ) -> (
        tempfile::TempDir,
        SqliteStore,
        SteerAdapter,
        crate::durable::TaskId,
        PathBuf,
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
        let repo = home.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let delivery = home.path().join("delivery.json");
        ContextDelivery {
            repository: None,
            repo,
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
            home: home.path().to_owned(),
        }
        .save(&delivery)
        .unwrap();
        (home, store, adapter, task, delivery)
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

    fn inbox(home: &Path, name: &str) -> Wake {
        Wake::Claude(ClaudeInbox {
            socket: home.join(format!("{name}.sock")),
            key: home.join(format!("{name}.key")),
        })
    }

    fn received(listener: &UnixListener) -> Vec<serde_json::Value> {
        let (mut stream, _) = listener.accept().unwrap();
        let mut wire = String::new();
        stream.read_to_string(&mut wire).unwrap();
        wire.lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn an_idle_claude_conversation_takes_direction_and_updates_in_its_inbox_once() {
        let (home, store, adapter, task, delivery) = fixture("wake-claude");
        let listener = UnixListener::bind(home.path().join("inbox.sock")).unwrap();
        std::fs::write(home.path().join("inbox.key"), "peer-key\n").unwrap();
        register(
            &adapter,
            "native",
            &delivery,
            Some(inbox(home.path(), "inbox")),
            true,
        );
        comment(&store, &task, "c-1", "use the staging bucket");
        comment(&store, &task, "c-2", "PR: https://example.test/pr/1");
        after_turn(&store, "wake-claude");
        let frames = received(&listener);
        assert_eq!(
            frames[0],
            serde_json::json!({"type":"auth","token":"peer-key"})
        );
        assert_eq!(frames[1]["session_id"], "native");
        assert_eq!(frames[1]["priority"], "next");
        let content = frames[1]["message"]["content"].as_str().unwrap();
        assert!(content.starts_with("<cross-session-message from=\"loopflow\""));
        assert!(content.contains("use the staging bucket"), "a steer wakes");
        assert!(content.contains("New comment from jack: PR: https://example.test/pr/1"));
        assert_eq!(
            store.session_steers("wake-claude", 0, 8).unwrap().pending,
            0
        );
        // Woken, it is in a turn: nothing more goes out until its Stop.
        comment(&store, &task, "c-3", "and the eu region");
        after_turn(&store, "wake-claude");
        register(
            &adapter,
            "native",
            &delivery,
            Some(inbox(home.path(), "inbox")),
            true,
        );
        after_turn(&store, "wake-claude");
        let wire = received(&listener)[1]["message"]["content"].to_string();
        assert!(wire.contains("and the eu region"));
        assert!(
            !wire.contains("staging bucket") && !wire.contains("pr/1"),
            "delivered news is not repeated"
        );
    }

    #[test]
    fn an_unreachable_inbox_leaves_its_news_for_the_next_boundary() {
        let (home, store, adapter, task, delivery) = fixture("wake-gone");
        std::fs::write(home.path().join("gone.key"), "peer-key").unwrap();
        register(
            &adapter,
            "native",
            &delivery,
            Some(inbox(home.path(), "gone")),
            true,
        );
        comment(&store, &task, "c-1", "use the staging bucket");
        comment(&store, &task, "c-2", "PR: https://example.test/pr/1");
        after_turn(&store, "wake-gone");
        assert!(super::registrations().get("wake-gone").is_none());
        assert_eq!(store.session_steers("wake-gone", 0, 8).unwrap().pending, 1);
        let update = crate::harness::task_updates::read(&store, "wake-gone")
            .unwrap()
            .expect("still news at the next boundary");
        assert!(update.text.contains("pr/1"));
    }
}
