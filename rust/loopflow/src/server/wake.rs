//! Steers and Task updates for idle managed conversations, pushed by the
//! Machine server. The store says which conversations are idle and how to
//! reach them (`loopflow_execution::steer::idle_conversations`); when a steer is saved or
//! the store's `task_updates` revision moves, one watcher reads them and wakes
//! each one it has news for. A server restart forgets nothing. No process or
//! clock per Session: Claude takes the message in its cross-session inbox,
//! Codex as a turn its app-server starts, OpenCode and Pi as a turn their
//! plugin or extension starts. A conversation the server cannot reach hears
//! it at its next boundary.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::json;

use loopflow_execution::steer::{IdleConversation, IdleReach, Inbox};
use loopflow_storage::sqlite::SqliteStore;

/// How long the watcher waits for a store change before reading the revision
/// anyway: the change signal can miss a checkpoint or a Machine's sleep.
const UNSIGNALLED: Duration = Duration::from_secs(60);
const SEND_DEADLINE: Duration = Duration::from_secs(2);
/// A provider's turn start loads nothing new; it answers once the turn exists.
const TURN_START_DEADLINE: Duration = Duration::from_secs(15);

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
            loopflow_execution::journal::request::isolate(watch);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "Task updates wait for each conversation's next turn");
    }
}

fn watch() {
    let path = loopflow_execution::store::default_db_path();
    let mut changes = loopflow_storage::changes::StoreChanges::watch(&path);
    let mut watcher = Watcher::default();
    loop {
        watcher.read(&path);
        changes.wait(UNSIGNALLED);
    }
}

/// The watcher's store and the revisions it last woke conversations for.
#[derive(Default)]
struct Watcher {
    store: Option<SqliteStore>,
    seen: Option<(i64, i64)>,
}

impl Watcher {
    /// Wake idle conversations if the revisions moved since the last reading.
    /// The first reading wakes too: what arrived while no server ran reaches
    /// each idle conversation now. One that already heard it gets nothing;
    /// claimed updates and confirmed steers say so.
    fn read(&mut self, path: &Path) {
        if self.store.is_none() && path.exists() {
            self.store = loopflow_execution::store::open_sqlite(path)
                .inspect_err(|error| tracing::warn!(%error, "Task update watcher has no store"))
                .ok();
        }
        let Some(store) = &self.store else {
            return;
        };
        match store
            .task_update_revision()
            .and_then(|updates| Ok((updates, store.steer_revision()?)))
        {
            Ok(revision) if self.seen != Some(revision) => {
                self.seen = Some(revision);
                wake_idle(store, None);
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "steer and Task update revisions unread");
                self.store = None;
            }
        }
    }
}

fn wake_idle(store: &SqliteStore, session: Option<&str>) {
    match loopflow_execution::steer::idle_conversations(store, session) {
        Ok(idle) => {
            for conversation in idle {
                wake(store, &conversation);
            }
        }
        Err(error) => tracing::warn!(session, %error, "idle conversations unread"),
    }
}

/// Deliver `conversation`'s pending steers, Task updates among them; with
/// none, the conversation is not touched. The steers are stamped once sent,
/// so a failed send leaves a claim that lapses for the next boundary. Once
/// sent, the conversation is recorded as in a turn, so the next revision
/// starts no second one.
fn wake(store: &SqliteStore, conversation: &IdleConversation) {
    let IdleConversation {
        adapter,
        thread,
        cwd,
        reach,
    } = conversation;
    let delivered = (|| -> Result<()> {
        let session = &adapter.session;
        loopflow_execution::harness::task_updates::write(store, session)?;
        if !store.has_undelivered_steers(session)? {
            return Ok(());
        }
        // Connect before claiming, so an unreachable conversation keeps its
        // news for its next boundary.
        let reach = match reach {
            IdleReach::Inbox(Inbox::Claude(key)) => match open_inbox(key, thread)? {
                Some((socket, token)) => Reach::Claude(socket, token),
                None => return Ok(()),
            },
            IdleReach::Codex(endpoint) => Reach::Codex(endpoint),
            IdleReach::Inbox(Inbox::Socket(socket)) => Reach::Socket(open_socket(socket)?),
        };
        let steers = loopflow_execution::steer::claim(
            store,
            cwd,
            adapter,
            thread,
            loopflow_execution::steer::NativeBoundary::Turn,
        )?;
        let Some(steers) = steers else {
            return Ok(());
        };
        match reach {
            Reach::Claude(mut socket, token) => {
                send_inbox(&mut socket, &token, thread, &steers.text)?
            }
            Reach::Codex(endpoint) => start_codex_turn(endpoint, thread, &steers.text)?,
            Reach::Socket(mut socket) => start_extension_turn(&mut socket, thread, &steers.text)?,
        }
        store.record_woken(adapter)?;
        loopflow_execution::steer::delivered(store, session, &steers);
        Ok(())
    })();
    if let Err(error) = delivered {
        tracing::warn!(session = adapter.session, error = %format!("{error:#}"), "idle conversation not woken; its steers wait for its next boundary");
    }
}

enum Reach<'a> {
    Claude(UnixStream, String),
    Codex(&'a Path),
    Socket(UnixStream),
}

/// The socket an OpenCode plugin or Pi extension listens on while its work
/// session runs.
fn open_socket(socket: &Path) -> Result<UnixStream> {
    let stream = UnixStream::connect(socket)
        .with_context(|| format!("wake socket {} unreachable", socket.display()))?;
    stream.set_write_timeout(Some(SEND_DEADLINE))?;
    stream.set_read_timeout(Some(TURN_START_DEADLINE))?;
    Ok(stream)
}

/// One JSON line naming the conversation and the message, then this side
/// closes. The plugin or extension asks its provider for a turn and answers
/// `started` once the provider took the message; a turn already running
/// queues it as a steer.
fn start_extension_turn(socket: &mut UnixStream, thread: &str, text: &str) -> Result<()> {
    socket.write_all(format!("{}\n", json!({"thread":thread,"text":text})).as_bytes())?;
    socket.shutdown(std::net::Shutdown::Write)?;
    let mut answer = String::new();
    socket
        .read_to_string(&mut answer)
        .context("the provider did not answer the turn start")?;
    anyhow::ensure!(
        answer.trim() == "started",
        "the provider did not start a turn"
    );
    Ok(())
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
            TURN_START_DEADLINE,
            loopflow_execution::harness::codex_connection::deliver_update(endpoint, thread, text),
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
    use loopflow_execution::process::attachment;

    use super::{after_turn, wake_idle, Watcher};
    use crate::server::wire::{BoundaryMoment, SessionBoundary};
    use loopflow_execution::context_block::ContextDelivery;
    use loopflow_execution::steer::{Inbox, NativeBoundary, SteerAdapter};
    use loopflow_storage::sqlite::SqliteStore;
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};

    struct Fixture {
        home: tempfile::TempDir,
        store: SqliteStore,
        adapter: SteerAdapter,
        task: loopflow_model::durable::TaskId,
        delivery: PathBuf,
    }

    /// A live interactive Session bound to a Task, whose provider selected
    /// conversation `native`, and its delivery file.
    fn fixture(session: &str) -> Fixture {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("store.db"))
                .unwrap();
        store.test_session(
            session,
            &loopflow_execution::session_record::new_artifact_key(),
        );
        let process = store.test_lf_process();
        let claim = attachment::claim_session_attachment(&store, session, None, &process).unwrap();
        let pid = std::process::id();
        store
            .record_agent_process_identity(
                session,
                &claim,
                pid,
                loopflow_execution::process::os_process::process_started_at(pid)
                    .unwrap()
                    .unwrap(),
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
        let task = loopflow_model::durable::TaskId::new();
        store
            .create_task(&loopflow_storage::work::task::NewTask {
                repository_id: loopflow_execution::repository::location::ensure_repository(
                    &store, "/wake",
                )
                .unwrap(),
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
                "UPDATE agent_sessions SET agent_interactive=1 WHERE agent_process_id=?1",
                [&adapter.agent_process_id],
            )
            .unwrap();
        }
        let captured = store.session(session).unwrap().unwrap().captured;
        loopflow_execution::store::sessions::bind_session(&store, session, captured, &task)
            .unwrap();
        // The conversation's first boundary records what it already knows.
        loopflow_execution::harness::task_updates::write(&store, session).unwrap();
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
            identity: None,
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
        fn store_path(&self) -> PathBuf {
            self.home.path().join("store.db")
        }

        /// A Claude conversation whose turn ended, listening on its inbox.
        fn idle_claude(&self) -> (UnixListener, PathBuf) {
            let socket = self.home.path().join("inbox.sock");
            let listener = UnixListener::bind(&socket).unwrap();
            let inbox = self.claude(&socket);
            self.stop(&self.store, &inbox);
            (listener, inbox)
        }

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
                    inbox: Some(Inbox::Claude(inbox.to_owned())),
                },
            )
            .unwrap();
        }

        /// An OpenCode or Pi work session's turn ends, as its plugin or
        /// extension reports it, naming the socket it listens on.
        fn settle(&self, provider: &str, socket: &Path) {
            let thread = match provider {
                "pi" => "/pi/session.jsonl",
                _ => "native",
            };
            self.store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET provider=?1,provider_thread=?2 WHERE id=?3",
                    [provider, thread, &self.adapter.session],
                )
                .unwrap();
            let report = |moment, callback| {
                super::super::boundary::answer(
                    &self.store,
                    &SessionBoundary {
                        moment,
                        delivery: self.delivery.clone(),
                        adapter: Some(self.adapter.clone()),
                        callback,
                        gap: None,
                        printed: Vec::new(),
                        inbox: Some(Inbox::Socket(socket.to_owned())),
                    },
                )
                .unwrap();
            };
            if provider == "pi" {
                let answer = serde_json::json!({"type":"message_end","message":{"role":"assistant",
                    "content":[{"type":"text","text":"Done."}],"stopReason":"stop"}});
                report(
                    BoundaryMoment::Event,
                    serde_json::json!({"type":"agent_settled","aborted":false,"sessionFile":thread,
                        "turn":uuid::Uuid::new_v4().to_string(),"openTools":0,"pendingInput":0,
                        "events":[answer]}),
                );
                report(BoundaryMoment::Stop, serde_json::json!({}));
            } else {
                report(
                    BoundaryMoment::Stop,
                    serde_json::json!({"hook_event_name":"Stop","session_id":thread}),
                );
            }
        }

        fn comment(&self, store: &SqliteStore, id: &str, body: &str) {
            store
                .append_task_comment(
                    &self.task,
                    &loopflow_model::comment::TaskComment {
                        id: id.into(),
                        body: body.into(),
                        author: loopflow_model::comment::TaskCommentAuthor::Person {
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

    /// A plugin or extension listening for the server: every message it
    /// takes, each answered as `answer`.
    fn extension(
        socket: &Path,
        answer: &'static str,
    ) -> std::sync::mpsc::Receiver<serde_json::Value> {
        let listener = UnixListener::bind(socket).unwrap();
        let (taken, messages) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut wire = String::new();
                stream.read_to_string(&mut wire).unwrap();
                let _ = std::io::Write::write_all(&mut stream, answer.as_bytes());
                if taken.send(serde_json::from_str(&wire).unwrap()).is_err() {
                    return;
                }
            }
        });
        messages
    }

    #[test]
    fn an_idle_opencode_or_pi_work_session_starts_a_turn_on_its_direction_once() {
        for (provider, thread) in [("opencode", "native"), ("pi", "/pi/session.jsonl")] {
            let session = format!("wake-{provider}");
            let fixture = fixture(&session);
            let socket = fixture.home.path().join("s");
            let messages = extension(&socket, "started\n");
            fixture.settle(provider, &socket);
            fixture.comment(&fixture.store, "c-1", "use the staging bucket");
            after_turn(&fixture.store, &session);
            let message = messages.try_recv().expect(provider);
            assert_eq!(message["thread"], thread);
            let text = message["text"].as_str().unwrap();
            assert!(text.contains("use the staging bucket"), "{text}");
            assert_eq!(
                fixture
                    .store
                    .session_steers(&session, 0, 8)
                    .unwrap()
                    .pending,
                0
            );
            // Woken, it is in a turn: nothing more goes out until that ends.
            fixture.comment(&fixture.store, "c-2", "and the eu region");
            wake_idle(&fixture.store, None);
            assert!(messages.try_recv().is_err(), "{provider} woken mid-turn");
            fixture.settle(provider, &socket);
            after_turn(&fixture.store, &session);
            let text = messages.try_recv().expect(provider)["text"].to_string();
            assert!(text.contains("and the eu region"));
            assert!(!text.contains("staging bucket"), "{text}");
        }
    }

    #[test]
    fn a_work_session_that_starts_no_turn_keeps_its_news_for_the_next_boundary() {
        // Nothing listens on the first socket; the second takes the message
        // and closes without starting a turn.
        for refused in [false, true] {
            let session = format!("wake-refused-{refused}");
            let fixture = fixture(&session);
            let socket = fixture.home.path().join("s");
            let _messages = refused.then(|| extension(&socket, ""));
            fixture.settle("opencode", &socket);
            fixture.comment(&fixture.store, "c-1", "use the staging bucket");
            after_turn(&fixture.store, &session);
            assert_eq!(
                loopflow_execution::steer::idle_conversations(&fixture.store, Some(&session))
                    .unwrap()
                    .len(),
                1,
                "still idle"
            );
            // Claimed or not, nothing was stamped as read.
            let page = fixture.store.session_steers(&session, 0, 8).unwrap();
            assert!(
                !page.items.is_empty() && page.items.iter().all(|row| row.delivered_at.is_none()),
                "refused: {refused}: {:?}",
                page.items
            );
        }
    }

    #[test]
    fn an_idle_claude_conversation_takes_direction_and_updates_in_its_inbox_once() {
        let fixture = fixture("wake-claude");
        let (listener, inbox) = fixture.idle_claude();
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
        assert!(content.contains("Task update "), "{content}");
        assert!(
            content.contains("new comment from jack: PR: https://example.test/pr/1"),
            "{content}"
        );
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
        let (listener, inbox) = fixture.idle_claude();
        // News lands while no server runs; a new server process carries over
        // nothing but the store, and its first reading delivers it.
        fixture.comment(&fixture.store, "c-1", "PR: https://example.test/pr/1");
        let path = fixture.store_path();
        Watcher::default().read(&path);
        let content = received(&listener)[1]["message"]["content"].to_string();
        assert!(content.contains("pr/1"));
        // News after the restart wakes it too.
        let restarted = loopflow_execution::store::open_sqlite(&path).unwrap();
        fixture.stop(&restarted, &inbox);
        fixture.comment(&restarted, "c-2", "PR: https://example.test/pr/2");
        wake_idle(&restarted, None);
        let content = received(&listener)[1]["message"]["content"].to_string();
        assert!(content.contains("pr/2") && !content.contains("pr/1"));
    }

    #[test]
    fn a_revision_with_nothing_new_leaves_the_conversation_alone() {
        let fixture = fixture("wake-quiet");
        let (listener, _) = fixture.idle_claude();
        listener.set_nonblocking(true).unwrap();
        let mut watcher = Watcher::default();
        let path = fixture.store_path();
        watcher.read(&path);
        // Another Task's Graph moves.
        fixture
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates'",
                [],
            )
            .unwrap();
        watcher.read(&path);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "no socket opened"
        );
    }

    /// A bare interactive `lf <skill>` in the Task's checkout: never woken,
    /// it hears the update at its next prompt.
    #[test]
    fn a_bare_conversation_is_not_woken_and_hears_the_update_at_its_next_prompt() {
        let fixture = fixture("wake-bare");
        fixture
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET work_source='checkout' WHERE id='wake-bare'",
                [],
            )
            .unwrap();
        // Bare hooks read steers directly; they never report a Stop or inbox.
        let socket = fixture.home.path().join("inbox.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fixture.claude(&socket);
        listener.set_nonblocking(true).unwrap();
        fixture.comment(&fixture.store, "c-1", "PR: https://example.test/pr/1");
        after_turn(&fixture.store, "wake-bare");
        wake_idle(&fixture.store, None);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "never woken"
        );
        loopflow_execution::harness::task_updates::write(&fixture.store, "wake-bare").unwrap();
        let update = loopflow_execution::steer::claim(
            &fixture.store,
            fixture.home.path(),
            &fixture.adapter,
            "native",
            loopflow_execution::steer::NativeBoundary::Turn,
        )
        .unwrap()
        .expect("the next prompt's hook hears the Task");
        assert!(update.text.contains("pr/1"));
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
            loopflow_execution::steer::idle_conversations(&fixture.store, Some("wake-gone"))
                .unwrap()
                .len(),
            1,
            "still idle"
        );
        let page = fixture.store.session_steers("wake-gone", 0, 8).unwrap();
        assert_eq!(page.pending, 2, "the direction and the Task update");
        assert!(page.items.iter().any(|row| row.steer.text.contains("pr/1")));
    }
    #[test]
    fn an_unknown_provider_identity_is_never_woken() {
        let fixture = fixture("wake-unknown");
        let inbox = fixture.claude(&fixture.home.path().join("unknown.sock"));
        fixture.stop(&fixture.store, &inbox);
        assert_eq!(
            loopflow_execution::steer::idle_conversations(&fixture.store, Some("wake-unknown"))
                .unwrap()
                .len(),
            1
        );
        let conn = rusqlite::Connection::open(fixture.store_path()).unwrap();
        conn.execute("UPDATE processes SET os_started_at=NULL WHERE id=(SELECT agent_process_id FROM agent_sessions WHERE id='wake-unknown')", []).unwrap();
        assert!(loopflow_execution::steer::idle_conversations(
            &fixture.store,
            Some("wake-unknown")
        )
        .unwrap()
        .is_empty());
    }
}
