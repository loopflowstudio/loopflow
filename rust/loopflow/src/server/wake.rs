//! Steers and Task updates for idle managed conversations, pushed by the
//! Machine server. The store says which conversations are idle and how to
//! reach them (`SqliteStore::idle_conversations`); when a steer is saved or
//! the store's `task_updates` revision moves, one watcher reads them and wakes
//! each one it has news for. A server restart forgets nothing. No process or
//! clock per Session: Claude takes the message in its cross-session inbox,
//! Codex as a turn its app-server starts. A conversation the server cannot
//! reach hears it at its next boundary.
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::json;

use crate::steer::{IdleConversation, IdleReach};
use crate::store::sqlite::SqliteStore;

/// How long the watcher waits for a store change before reading the revision
/// anyway: the change signal can miss a checkpoint or a Machine's sleep.
const UNSIGNALLED: Duration = Duration::from_secs(60);
const SEND_DEADLINE: Duration = Duration::from_secs(2);
/// A Codex turn start loads nothing new; it answers once the turn exists.
const CODEX_DEADLINE: Duration = Duration::from_secs(15);

/// Wake `session` now if it is idle and has news: a turn just ended, so an
/// update that arrived during it waits no longer. Blocking.
pub(super) fn after_turn(store: &SqliteStore, session: &str) {
    wake_idle(store, Some(session));
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
        if store.is_none() && path.exists() {
            store = SqliteStore::new(&path)
                .inspect_err(|error| tracing::warn!(%error, "Task update watcher has no store"))
                .ok();
        }
        if let Some(open) = &store {
            match open
                .task_update_revision()
                .and_then(|updates| Ok((updates, open.steer_revision()?)))
            {
                // The first reading is a baseline: what arrived while no
                // server ran waits for each conversation's next boundary.
                Ok(revision) if seen.is_none() => seen = Some(revision),
                Ok(revision) if seen != Some(revision) => {
                    seen = Some(revision);
                    wake_idle(open, None);
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "steer and Task update revisions unread");
                    store = None;
                }
            }
        }
        changes.wait(UNSIGNALLED);
    }
}

fn wake_idle(store: &SqliteStore, session: Option<&str>) {
    match store.idle_conversations(session) {
        Ok(idle) => {
            for conversation in idle {
                wake(store, &conversation);
            }
        }
        Err(error) => tracing::warn!(session, %error, "idle conversations unread"),
    }
}

/// Deliver `conversation`'s pending steers and Task update, if any. The
/// update is recorded as dispatched before it is sent; the steer batch is
/// confirmed once sent, so a failed send returns it to pending for the next
/// boundary. Once sent, the conversation is recorded as in a turn, so the
/// next revision starts no second one.
fn wake(store: &SqliteStore, conversation: &IdleConversation) {
    let IdleConversation {
        adapter,
        thread,
        cwd,
        reach,
    } = conversation;
    let delivered = (|| -> Result<()> {
        // Connect before claiming, so an unreachable conversation keeps its
        // news for its next boundary.
        let reach = match reach {
            IdleReach::Claude(key) => match open_inbox(key, thread)? {
                Some((socket, token)) => Reach::Inbox(socket, token),
                None => return Ok(()),
            },
            IdleReach::Codex(endpoint) => Reach::Codex(endpoint),
        };
        let steers = super::boundary::wake_steers(store, cwd, adapter, thread)?;
        let update = crate::harness::task_updates::claim(store, adapter, thread)?;
        let text: String = steers
            .iter()
            .map(|steers| steers.text.as_str())
            .chain(update.as_deref())
            .collect();
        if text.is_empty() {
            return Ok(());
        }
        match reach {
            Reach::Inbox(mut socket, token) => send_inbox(&mut socket, &token, thread, &text)?,
            Reach::Codex(endpoint) => start_codex_turn(endpoint, thread, &text)?,
        }
        store.record_woken(adapter)?;
        if let Some(steers) = steers {
            store.confirm_session_steers(
                adapter,
                &[steers.batch],
                super::boundary::HOOK_DEADLINE.as_secs(),
            )?;
        }
        Ok(())
    })();
    if let Err(error) = delivered {
        tracing::warn!(session = adapter.session, error = %format!("{error:#}"), "idle conversation not woken; its next turn hears the update");
    }
}

enum Reach<'a> {
    Inbox(UnixStream, String),
    Codex(&'a Path),
}

/// Claude's inbox socket and key for `thread`, from the key it publishes
/// beside its session record, `<config>/sessions/<pid>.<sha256 of socket>.key`;
/// `<pid>.json` names the socket. `None` once that Claude exited or moved to
/// another conversation, whose own boundary records it. Its `status` is not
/// read: a Stop hook can run before it says `idle`.
fn open_inbox(key: &Path, thread: &str) -> Result<Option<(UnixStream, String)>> {
    let pid = key
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.split('.').next())
        .ok_or_else(|| anyhow!("Claude inbox key {} names no process", key.display()))?;
    let record = key.with_file_name(format!("{pid}.json"));
    let record: serde_json::Value = match std::fs::read(&record) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("Claude session record {} unreadable", record.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if record["sessionId"].as_str() != Some(thread) {
        return Ok(None);
    }
    let socket = record["messagingSocketPath"]
        .as_str()
        .ok_or_else(|| anyhow!("Claude session record {pid}.json names no inbox"))?;
    let token = std::fs::read_to_string(key)
        .with_context(|| format!("Claude inbox key {} unreadable", key.display()))?;
    let socket = UnixStream::connect(socket)
        .with_context(|| format!("Claude inbox {socket} unreachable"))?;
    socket.set_write_timeout(Some(SEND_DEADLINE))?;
    Ok(Some((socket, token.trim().to_owned())))
}

fn start_codex_turn(endpoint: &Path, thread: &str, text: &str) -> Result<()> {
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
    use super::{after_turn, wake_idle};
    use crate::context_block::ContextDelivery;
    use crate::server::wire::{BoundaryMoment, SessionBoundary};
    use crate::steer::{NativeBoundary, SteerAdapter};
    use crate::store::sqlite::SqliteStore;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};

    struct Fixture {
        home: tempfile::TempDir,
        store: SqliteStore,
        adapter: SteerAdapter,
        task: crate::durable::TaskId,
        delivery: PathBuf,
    }

    /// A live interactive Session bound to a Task, whose provider selected
    /// conversation `native`, and its delivery file.
    fn fixture(session: &str) -> Fixture {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session(session, &crate::session_record::new_artifact_key());
        let process = store.test_lf_process();
        let claim = store
            .claim_session_attachment(session, None, &process, false)
            .unwrap();
        let pid = std::process::id();
        store
            .record_agent_process_identity(
                session,
                &claim,
                pid,
                crate::journal::process_started_at(pid).unwrap().unwrap(),
            )
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
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_sessions SET interactive=1 WHERE id=?1",
                [session],
            )
            .unwrap();
            conn.execute(
                "UPDATE processes SET agent_interactive=1 WHERE id=?1",
                [&adapter.agent_process_id],
            )
            .unwrap();
        }
        let captured = store.session(session).unwrap().unwrap().captured;
        store.bind_session(session, captured, &task).unwrap();
        // The conversation's first boundary records what it already knows.
        assert!(crate::harness::task_updates::read(&store, session)
            .unwrap()
            .is_none());
        let repo = home.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET cwd=?2 WHERE id=?1",
                rusqlite::params![session, repo.to_string_lossy()],
            )
            .unwrap();
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
        Fixture {
            home,
            store,
            adapter,
            task,
            delivery,
        }
    }

    impl Fixture {
        /// A Claude inbox as Claude publishes it: the session record naming
        /// `socket` for conversation `native`, and the peer key beside it.
        fn claude(&self, socket: &Path) -> PathBuf {
            let sessions = self.home.path().join("sessions");
            std::fs::create_dir_all(&sessions).unwrap();
            std::fs::write(
                sessions.join("4242.json"),
                serde_json::json!({"pid":4242,"sessionId":"native","messagingSocketPath":socket,"status":"idle"}).to_string(),
            )
            .unwrap();
            let key = sessions.join("4242.9c1d.key");
            std::fs::write(&key, "peer-key\n").unwrap();
            key
        }

        /// The conversation's turn ends; its hook names its inbox.
        fn stop(&self, store: &SqliteStore, inbox: &Path) {
            super::super::boundary::answer(
                store,
                &SessionBoundary {
                    moment: BoundaryMoment::Stop,
                    delivery: self.delivery.clone(),
                    adapter: Some(self.adapter.clone()),
                    callback: serde_json::json!({"hook_event_name":"Stop","session_id":"native"}),
                    gap: None,
                    printed: Vec::new(),
                    inbox: Some(inbox.to_owned()),
                },
            )
            .unwrap();
        }

        fn comment(&self, store: &SqliteStore, id: &str, body: &str) {
            store
                .append_task_comment(
                    &self.task,
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
        let fixture = fixture("wake-claude");
        let socket = fixture.home.path().join("inbox.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let inbox = fixture.claude(&socket);
        fixture.stop(&fixture.store, &inbox);
        fixture.comment(&fixture.store, "c-1", "use the staging bucket");
        fixture.comment(&fixture.store, "c-2", "PR: https://example.test/pr/1");
        after_turn(&fixture.store, "wake-claude");
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
            fixture
                .store
                .session_steers("wake-claude", 0, 8)
                .unwrap()
                .pending,
            0
        );
        // Woken, it is in a turn: nothing more goes out until its Stop.
        fixture.comment(&fixture.store, "c-3", "and the eu region");
        wake_idle(&fixture.store, None);
        fixture.stop(&fixture.store, &inbox);
        after_turn(&fixture.store, "wake-claude");
        let wire = received(&listener)[1]["message"]["content"].to_string();
        assert!(wire.contains("and the eu region"));
        assert!(
            !wire.contains("staging bucket") && !wire.contains("pr/1"),
            "delivered news is not repeated"
        );
    }

    #[test]
    fn a_restarted_server_still_wakes_a_conversation_idle_since_before() {
        let fixture = fixture("wake-restart");
        let socket = fixture.home.path().join("inbox.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let inbox = fixture.claude(&socket);
        fixture.stop(&fixture.store, &inbox);
        // A new server process: nothing but the store carries over.
        let restarted = SqliteStore::new(&fixture.home.path().join("store.db")).unwrap();
        fixture.comment(&restarted, "c-1", "PR: https://example.test/pr/1");
        wake_idle(&restarted, None);
        let content = received(&listener)[1]["message"]["content"].to_string();
        assert!(content.contains("pr/1"));
    }

    #[test]
    fn an_unreachable_inbox_leaves_its_news_for_the_next_boundary() {
        let fixture = fixture("wake-gone");
        let inbox = fixture.claude(&fixture.home.path().join("gone.sock"));
        fixture.stop(&fixture.store, &inbox);
        fixture.comment(&fixture.store, "c-1", "use the staging bucket");
        fixture.comment(&fixture.store, "c-2", "PR: https://example.test/pr/1");
        after_turn(&fixture.store, "wake-gone");
        assert_eq!(
            fixture
                .store
                .idle_conversations(Some("wake-gone"))
                .unwrap()
                .len(),
            1,
            "still idle"
        );
        assert_eq!(
            fixture
                .store
                .session_steers("wake-gone", 0, 8)
                .unwrap()
                .pending,
            1
        );
        let update = crate::harness::task_updates::read(&fixture.store, "wake-gone")
            .unwrap()
            .expect("still news at the next boundary");
        assert!(update.text.contains("pr/1"));
    }
}
