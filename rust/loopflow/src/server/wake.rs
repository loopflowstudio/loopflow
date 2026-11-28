//! Steers and Task updates for idle managed conversations, pushed by the
//! Machine server. The store records which conversations are idle. Each
//! launcher selects its endpoint in `loopflow_execution::steer::idle_conversations`.
//! When a steer is saved or the `task_updates` revision moves, one watcher wakes
//! each one it has news for. A server restart forgets nothing. No process or
//! clock per Session: Claude takes the message in its cross-session inbox,
//! Codex as a turn its app-server starts, OpenCode and Pi as a turn their
//! plugin or extension starts, and Antigravity via a VT-guarded bracketed paste
//! into its held terminal whose `PreInvocation` hook reads the waiting steers.
//! A conversation the server cannot reach hears it at its next boundary.
//!
//! A steer naming a stopped interactive Session starts it, once per steer,
//! in a held terminal as `session.continue` does; the resumed conversation
//! reads its steers as its first input.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context, Result};
use serde_json::json;

use loopflow_execution::steer::{IdleConversation, IdleReach, Inbox, WAKE_PROMPT};
use loopflow_execution::transport::registry::{Registry, WakeTerminal};
use loopflow_model::id::LfSessionId;
use loopflow_storage::sqlite::SqliteStore;

/// How long the watcher waits for a store change before reading the revision
/// anyway: the change signal can miss a checkpoint or a Machine's sleep.
const UNSIGNALLED: Duration = Duration::from_secs(60);
/// How soon the watcher retries an idle held terminal that was busy (e.g. typed
/// draft, open menu, or recent typing) when news arrived.
const DEFERRED_RETRY: Duration = Duration::from_secs(2);
/// Delay between post-Stop redraw retries before handing off to the watcher.
const REDRAW_RETRY: Duration = Duration::from_millis(300);
const SEND_DEADLINE: Duration = Duration::from_secs(2);
/// A provider's turn start loads nothing new; it answers once the turn exists.
const TURN_START_DEADLINE: Duration = Duration::from_secs(15);

/// Nudge handle shared by boundary follow-ups and the recovery/wake watcher.
/// Calling
/// [`retry_deferred`](Self::retry_deferred) flags a deferred turn-end wake and
/// breaks the watcher's `StoreChanges::wait` out of `UNSIGNALLED` immediately.
#[derive(Debug, Clone)]
struct WakeNudge {
    state: Arc<(AtomicBool, loopflow_storage::changes::StoreWaker)>,
}

impl WakeNudge {
    fn new(waker: loopflow_storage::changes::StoreWaker) -> Self {
        Self {
            state: Arc::new((AtomicBool::new(false), waker)),
        }
    }

    fn retry_deferred(&self) {
        self.state.0.store(true, Ordering::Release);
        self.state.1.wake();
    }

    fn take_pending(&self) -> bool {
        self.state.0.swap(false, Ordering::AcqRel)
    }
}

/// Server-owned handle for waking idle sessions from boundary follow-ups and
/// nudging the background watcher when a held-terminal wake is deferred.
#[derive(Debug, Clone, Default)]
pub(super) struct Waker {
    terminals: Option<Arc<Registry>>,
    nudge: Option<WakeNudge>,
}

impl Waker {
    /// Wake `session` if it is idle and has news. For a held-terminal wake
    /// right after `Stop`, the TUI may still be redrawing its idle prompt; give
    /// it two brief retries before handing off to the background watcher.
    pub(super) async fn after_boundary(&self, store: &SqliteStore, session: &str) -> Result<()> {
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(REDRAW_RETRY).await;
            }
            let store = store.clone();
            let session = session.to_owned();
            let terminals = self.terminals.clone();
            let deferred = loopflow_execution::journal::request::spawn_blocking(move || {
                wake_idle(&store, Some(&session), terminals.as_deref())
            })
            .await?;
            if !deferred {
                return Ok(());
            }
        }
        if let Some(nudge) = &self.nudge {
            nudge.retry_deferred();
        }
        Ok(())
    }
}

/// The revisions this watcher last woke conversations for, and the held
/// terminals it starts stopped Sessions in. Driven by `recovery`'s single
/// store-watching thread.
#[derive(Default)]
pub(super) struct Watcher {
    seen: Option<(i64, i64)>,
    last_wake_pass: Option<SystemTime>,
    terminals: Option<Arc<Registry>>,
    nudge: Option<WakeNudge>,
    deferred: bool,
}

impl Watcher {
    pub(super) fn new(
        terminals: Option<Arc<Registry>>,
        signal: loopflow_storage::changes::StoreWaker,
    ) -> (Self, Waker) {
        let nudge = Some(WakeNudge::new(signal));
        let waker = Waker {
            terminals: terminals.clone(),
            nudge: nudge.clone(),
        };
        let watcher = Self {
            terminals,
            nudge,
            ..Self::default()
        };
        (watcher, waker)
    }

    pub(super) fn wait_timeout(&self) -> Duration {
        if self.deferred {
            DEFERRED_RETRY
        } else {
            UNSIGNALLED
        }
    }

    /// Wake idle conversations if the revisions moved since the last reading,
    /// or retry held terminals that were busy on an earlier pass. The periodic
    /// pass also picks up steers read before a terminal became reachable.
    /// The first reading wakes too: what arrived while no server ran reaches
    /// each idle conversation now. One that already heard it gets nothing;
    /// claimed updates and confirmed steers say so.
    pub(super) fn read_with(&mut self, store: &SqliteStore) {
        match store
            .task_update_revision()
            .and_then(|updates| Ok((updates, store.steer_revision()?)))
        {
            Ok(revision) => {
                let moved = self.seen != Some(revision);
                let nudged = self.nudge.as_ref().is_some_and(WakeNudge::take_pending);
                let now = SystemTime::now();
                let due = self.last_wake_pass.is_none_or(|last| {
                    !now.duration_since(last)
                        .is_ok_and(|elapsed| elapsed < UNSIGNALLED)
                });
                if moved {
                    self.seen = Some(revision);
                    if let Some(terminals) = &self.terminals {
                        let mut agents = loopflow_execution::agent::MachineAgents::default();
                        start_stopped(
                            store,
                            |session| {
                                loopflow_execution::ops::recovery::launch_failure(
                                    store,
                                    session,
                                    &mut agents,
                                )
                            },
                            |session, request| hold(terminals, session, request),
                        );
                    }
                }
                if moved || self.deferred || nudged || due {
                    self.last_wake_pass = Some(now);
                    self.deferred = wake_idle(store, None, self.terminals.as_deref());
                }
            }
            Err(error) => {
                tracing::warn!(%error, "steer and Task update revisions unread");
            }
        }
    }

    #[cfg(test)]
    fn read(&mut self, path: &Path) {
        if !path.exists() {
            return;
        }
        if let Ok(store) = loopflow_execution::store::open_sqlite(path) {
            self.read_with(&store);
        }
    }
}

/// Start each stopped Session the steers saved since the last reading name,
/// once, with `start(session, request)`: the request is the first steer that
/// named it. A Session whose harness cannot start here is not spawned; that,
/// or a start that fails, is recorded as its `start_failure`, and its steers
/// wait, unread, for its next start. A later steer tries again.
fn start_stopped(
    store: &SqliteStore,
    mut readiness: impl FnMut(&LfSessionId) -> Result<Option<String>>,
    mut start: impl FnMut(&LfSessionId, String) -> Result<()>,
) {
    let wakes = match loopflow_execution::steer::sessions_to_start(store) {
        Ok(wakes) => wakes,
        Err(error) => {
            tracing::warn!(%error, "steers unread for stopped Sessions");
            return;
        }
    };
    for wake in wakes {
        let session = LfSessionId::from(wake.session.as_str());
        let ready = readiness(&session).unwrap_or_else(|error| {
            // An unreadable harness check does not cost the steer its start.
            tracing::warn!(%session, error = %format!("{error:#}"), "could not check this Session’s harness");
            None
        });
        let failure = match ready {
            Some(failure) => Some(failure),
            None => start(&session, wake.steer.clone())
                .err()
                .map(|error| format!("{error:#}")),
        };
        if let Some(failure) = &failure {
            tracing::warn!(%session, steer = wake.steer, %failure, "stopped Session not started; its steers wait for its next start");
        }
        if let Err(error) = store.record_start_failure(&session, failure.as_deref()) {
            tracing::warn!(%session, %error, "start failure unrecorded");
        }
    }
}

/// `session.continue`'s held start, unless a held terminal already runs it.
fn hold(terminals: &Registry, session: &LfSessionId, request: String) -> Result<()> {
    let start = super::service::continuing(session.clone(), request, 0, 0, None)
        .map_err(|error| anyhow!("{}", error.message))?;
    terminals.start_unless_held(start)?;
    Ok(())
}

fn wake_idle(store: &SqliteStore, session: Option<&str>, terminals: Option<&Registry>) -> bool {
    match loopflow_execution::steer::idle_conversations(store, session) {
        Ok(idle) => {
            let mut deferred = false;
            for conversation in idle {
                deferred |= wake(store, &conversation, terminals);
            }
            deferred
        }
        Err(error) => {
            tracing::warn!(session, %error, "idle conversations unread");
            false
        }
    }
}

/// Deliver `conversation`'s pending steers, Task updates among them; with
/// none, the conversation is not touched. Provider-API endpoints claim and
/// stamp steers once sent, so a failed send leaves a claim that lapses for the
/// next boundary. Held-terminal wakes (`IdleReach::Terminal`) leave steers
/// unclaimed and paste the trigger prompt once the VT screen shows an empty
/// idle prompt; the provider's `PreInvocation` hook claims and stamps them
/// once the turn starts. Returns `true` when a held terminal with news was not
/// yet ready to paste into.
fn wake(
    store: &SqliteStore,
    conversation: &IdleConversation,
    terminals: Option<&Registry>,
) -> bool {
    let IdleConversation {
        adapter,
        thread,
        reach,
        ..
    } = conversation;
    let outcome = (|| -> Result<bool> {
        let session = &adapter.session;
        loopflow_execution::harness::task_updates::write(store, session)?;
        if !store.has_undelivered_steers(session)? {
            return Ok(false);
        }
        // Connect before claiming, so an unreachable conversation keeps its
        // news for its next boundary.
        let (thread, reach) = match (reach, thread.as_deref()) {
            (
                IdleReach::Terminal {
                    terminal,
                    prompt_ready,
                },
                _,
            ) => {
                let Some(terminals) = terminals else {
                    return Ok(false);
                };
                return match terminals.wake_idle_prompt(terminal, WAKE_PROMPT, *prompt_ready)? {
                    WakeTerminal::Injected => {
                        store.record_woken(adapter)?;
                        Ok(false)
                    }
                    WakeTerminal::NotReady => Ok(true),
                    WakeTerminal::Absent => Ok(false),
                };
            }
            (_, None) => return Ok(false),
            (IdleReach::Inbox(Inbox::Claude(key)), Some(thread)) => {
                match open_inbox(key, thread)? {
                    Some((socket, token)) => (thread, Reach::Claude(socket, token)),
                    None => return Ok(false),
                }
            }
            (IdleReach::Codex(endpoint), Some(thread)) => (thread, Reach::Codex(endpoint)),
            (IdleReach::Inbox(Inbox::Socket(socket)), Some(thread)) => {
                (thread, Reach::Socket(open_socket(socket)?))
            }
        };
        let steers = loopflow_execution::steer::claim_complete(store, adapter, thread)?;
        let Some(steers) = steers else {
            return Ok(false);
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
        Ok(false)
    })();
    match outcome {
        Ok(deferred) => deferred,
        Err(error) => {
            tracing::warn!(session = adapter.session, error = %format!("{error:#}"), "idle conversation not woken; its steers wait for its next boundary");
            false
        }
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
    loopflow_execution::runtime::block_on(async {
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
    use loopflow_execution::harness::{launcher, ReopenSteers};
    use loopflow_execution::process::attachment;
    use loopflow_harness::HarnessKind;

    use super::{start_stopped, wake_idle, Registry, Watcher};
    use crate::server::wire::{BoundaryMoment, SessionBoundary};

    fn after_turn(store: &SqliteStore, session: &str, terminals: Option<&Registry>) -> bool {
        wake_idle(store, Some(session), terminals)
    }
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
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("loopflow.db"))
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
                parent_task_id: None,
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
                "UPDATE agent_sessions SET interactive=1,provider='claude' WHERE id=?1",
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
            self.home.path().join("loopflow.db")
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

        fn boundary(
            &self,
            store: &SqliteStore,
            moment: BoundaryMoment,
            callback: serde_json::Value,
            inbox: Option<Inbox>,
        ) -> Option<super::super::boundary::After> {
            super::super::boundary::answer(
                store,
                &SessionBoundary {
                    moment,
                    delivery: self.delivery.clone(),
                    adapter: Some(self.adapter.clone()),
                    callback,
                    gap: None,
                    printed: Vec::new(),
                    inbox,
                },
            )
            .unwrap()
            .1
        }

        /// The conversation's turn ends; its hook names its inbox.
        fn stop(&self, store: &SqliteStore, inbox: &Path) {
            self.boundary(
                store,
                BoundaryMoment::Stop,
                serde_json::json!({"hook_event_name":"Stop","session_id":"native"}),
                Some(Inbox::Claude(inbox.to_owned())),
            );
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
        count: usize,
    ) -> std::sync::mpsc::Receiver<serde_json::Value> {
        let listener = UnixListener::bind(socket).unwrap();
        let (taken, messages) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(count) {
                let mut stream = stream.unwrap();
                let mut wire = String::new();
                stream.read_to_string(&mut wire).unwrap();
                if taken.send(serde_json::from_str(&wire).unwrap()).is_err() {
                    return;
                }
                let _ = std::io::Write::write_all(&mut stream, answer.as_bytes());
            }
        });
        messages
    }

    /// A Codex app-server that answers one client's `turn/start`: the request
    /// it took.
    fn codex_app_server(endpoint: &Path) -> std::thread::JoinHandle<serde_json::Value> {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;
        let listener = UnixListener::bind(endpoint).unwrap();
        listener.set_nonblocking(true).unwrap();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::UnixListener::from_std(listener).unwrap();
                    let (stream, _) = listener.accept().await.unwrap();
                    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                    async fn receive(
                        socket: &mut tokio_tungstenite::WebSocketStream<tokio::net::UnixStream>,
                    ) -> serde_json::Value {
                        loop {
                            if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
                                return serde_json::from_str(&text).unwrap();
                            }
                        }
                    }
                    let initialize = receive(&mut socket).await;
                    let answer = serde_json::json!({"id":initialize["id"],"result":{}});
                    socket
                        .send(Message::Text(answer.to_string().into()))
                        .await
                        .unwrap();
                    assert_eq!(receive(&mut socket).await["method"], "initialized");
                    let start = receive(&mut socket).await;
                    let answer =
                        serde_json::json!({"id":start["id"],"result":{"turn":{"id":"woken"}}});
                    socket
                        .send(Message::Text(answer.to_string().into()))
                        .await
                        .unwrap();
                    let _ = socket.next().await;
                    start
                })
        })
    }

    #[test]
    fn an_idle_opencode_or_pi_work_session_starts_a_turn_on_its_direction_once() {
        for (provider, thread) in [("opencode", "native"), ("pi", "/pi/session.jsonl")] {
            let session = format!("wake-{provider}");
            let fixture = fixture(&session);
            let socket = fixture.home.path().join("s");
            let messages = extension(&socket, "started\n", 2);
            fixture.settle(provider, &socket);
            fixture.comment(&fixture.store, "c-1", "use the staging bucket");
            after_turn(&fixture.store, &session, None);
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
            wake_idle(&fixture.store, None, None);
            assert!(messages.try_recv().is_err(), "{provider} woken mid-turn");
            fixture.settle(provider, &socket);
            after_turn(&fixture.store, &session, None);
            let text = messages.try_recv().expect(provider)["text"].to_string();
            assert!(text.contains("and the eu region"));
            assert!(!text.contains("staging bucket"), "{text}");
        }
    }

    const HOLDER_BUILD: &str = "feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface";

    /// A real holder + Registry in `home` running a shell script that simulates
    /// an open Antigravity TUI in a held terminal.
    fn held_agy_terminal(
        store: &SqliteStore,
        session: &str,
        script: &str,
        wait_for: &str,
    ) -> (
        std::sync::Arc<super::Registry>,
        loopflow_model::id::TerminalId,
    ) {
        use loopflow_execution::terminal_holder::{serve, Place};
        use loopflow_execution::transport::registry::{connect, Registry, Start};
        use loopflow_execution::transport::wire::{frame, Incoming, Request, Tag};
        use std::io::Write;
        use std::time::{Duration, Instant};

        let place = Place::of(HOLDER_BUILD).unwrap();
        let (ready, listening) = std::sync::mpsc::channel();
        let served = place.clone();
        std::thread::spawn(move || {
            let _ = serve(&served, HOLDER_BUILD, move || {
                let _ = ready.send(());
            });
        });
        listening.recv_timeout(Duration::from_secs(10)).unwrap();
        let registry = Registry::open(HOLDER_BUILD).unwrap();
        let terminal = registry
            .start(Start {
                session: Some(loopflow_model::id::LfSessionId::from(session)),
                request: format!("wake-{session}"),
                argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
                cwd: "/".into(),
                env: vec![("PATH".into(), "/usr/bin:/bin".into())],
                rows: 24,
                cols: 80,
                placement: None,
            })
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET held_terminal_id=?2 WHERE id=?1",
                rusqlite::params![session, terminal.as_str()],
            )
            .unwrap();
        // Wait until Hosted's ghostty_vt::Terminal has parsed the screen text.
        let mut socket = connect(&terminal).unwrap();
        let attach = Incoming {
            id: 1,
            request: Request::Attach {
                focused: None,
                rows: 24,
                cols: 80,
                name: None,
                from: None,
            },
        };
        socket
            .write_all(&frame(Tag::Request, &serde_json::to_vec(&attach).unwrap()))
            .unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let mut seen = Vec::new();
        let needle = wait_for.as_bytes();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !seen.windows(needle.len()).any(|w| w == needle) {
            assert!(
                Instant::now() < deadline,
                "held terminal never showed {wait_for:?}: {:?}",
                String::from_utf8_lossy(&seen)
            );
            let mut buf = [0u8; 4096];
            if let Ok(n) = socket.read(&mut buf) {
                if n == 0 {
                    break;
                }
                seen.extend_from_slice(&buf[..n]);
            }
        }
        drop(socket);
        (registry, terminal)
    }

    fn wait_until(what: &str, mut ready: impl FnMut() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !ready() {
            assert!(std::time::Instant::now() < deadline, "timed out: {what}");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn agy_idle_screen() -> String {
        let rule = "─".repeat(80);
        format!(
            r"\033[?2004h\033[2J\033[H{rule}\r\n\033[94m>\033[m\r\n{rule}\r\n? for shortcuts   auto · Gemini 4 Neon · high\r\n"
        )
    }

    fn agy_idle_script(pasted: &Path) -> String {
        let idle = agy_idle_screen();
        format!(
            "stty -echo\nprintf '{idle}'\nIFS= read -r line\nprintf '%s' \"$line\" > \"{}\"\n",
            pasted.display()
        )
    }

    fn assert_wake_pasted(pasted: &Path) {
        let expected = format!("\x1b[200~{}\x1b[201~", super::WAKE_PROMPT);
        wait_until("wake prompt pasted", || {
            std::fs::metadata(pasted).is_ok_and(|file| file.len() >= expected.len() as u64)
        });
        assert_eq!(std::fs::read_to_string(pasted).unwrap(), expected);
    }

    #[test]
    fn every_harness_wakes_its_idle_work_session() {
        for harness in HarnessKind::ALL {
            check_waiting_steer(harness, WakeAt::TurnEnd);
        }
    }

    #[test]
    fn reopened_work_sessions_take_waiting_steers() {
        for harness in HarnessKind::ALL {
            match launcher(harness).capabilities().reopen_steers {
                ReopenSteers::ServerBoundary => check_waiting_steer(harness, WakeAt::Reopen),
                // session_launch_contract runs every PromptHook fixture through lf resume.
                ReopenSteers::PromptHook => continue,
            }
        }
    }

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    enum WakeAt {
        TurnEnd,
        Reopen,
    }

    fn pin_lf_home(
        home: &Path,
    ) -> (
        crate::test_ambient::EnvGuard,
        std::sync::MutexGuard<'static, ()>,
    ) {
        let lock = loopflow_execution::journal::test_env_lock();
        let guard = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
        std::env::set_var("LF_HOME", home);
        // Tuple fields drop in order: restore LF_HOME before releasing the lock.
        (guard, lock)
    }

    fn check_waiting_steer(harness: HarnessKind, wake_at: WakeAt) {
        let session = "waiting";
        let fixture = fixture(session);
        // Pi's session_start and Antigravity's held terminal use LF_HOME.
        let _home = ((harness == HarnessKind::Pi && wake_at == WakeAt::Reopen)
            || harness == HarnessKind::Antigravity)
            .then(|| pin_lf_home(fixture.home.path()));
        let store = &fixture.store;
        let socket = fixture.home.path().join("s");
        let thread = if harness == HarnessKind::Pi {
            "/pi/session.jsonl"
        } else {
            "native"
        };
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider=?2,provider_thread=?3 WHERE id=?1",
                [session, harness.as_str(), thread],
            )
            .unwrap();

        // Each listener returns the actual turn addressed to this conversation.
        type ReadTurn = Box<dyn FnOnce() -> String>;
        let mut terminals = None;
        let (inbox, message): (Option<Inbox>, ReadTurn) = match harness {
            HarnessKind::Claude => {
                let listener = UnixListener::bind(&socket).unwrap();
                (
                    Some(Inbox::Claude(fixture.claude(&socket))),
                    Box::new(move || {
                        let frames = received(&listener);
                        assert_eq!(frames[1]["session_id"], thread);
                        frames[1]["message"]["content"].as_str().unwrap().to_owned()
                    }),
                )
            }
            HarnessKind::Codex => {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE agent_sessions SET agent_endpoint=?2 WHERE id=?1",
                        rusqlite::params![session, socket.to_str().unwrap()],
                    )
                    .unwrap();
                let server = codex_app_server(&socket);
                (
                    None,
                    Box::new(move || {
                        let start = server.join().unwrap();
                        assert_eq!(start["method"], "turn/start");
                        assert_eq!(start["params"]["threadId"], thread);
                        start["params"]["toolOutput"]["output"].to_string()
                    }),
                )
            }
            HarnessKind::OpenCode | HarnessKind::Pi => {
                let messages = extension(&socket, "started\n", 1);
                (
                    Some(Inbox::Socket(socket)),
                    Box::new(move || {
                        let message = messages.recv().unwrap();
                        assert_eq!(message["thread"], thread);
                        message["text"].as_str().unwrap().to_owned()
                    }),
                )
            }
            HarnessKind::Antigravity => {
                let pasted = fixture.home.path().join("pasted");
                let script = agy_idle_script(&pasted);
                let (registry, _) = held_agy_terminal(store, session, &script, "? for shortcuts");
                terminals = Some(registry);
                let store_clone = store.clone();
                let repo = fixture.home.path().join("repo");
                let adapter = fixture.adapter.clone();
                (
                    None,
                    Box::new(move || {
                        assert_wake_pasted(&pasted);
                        // Steers stay unstamped until agy's PreInvocation runs.
                        assert_eq!(
                            store_clone.session_steers(session, 0, 8).unwrap().pending,
                            1
                        );
                        let claimed = loopflow_execution::steer::claim(
                            &store_clone,
                            &repo,
                            &adapter,
                            thread,
                            NativeBoundary::Turn,
                        )
                        .unwrap()
                        .expect("PreInvocation claims waiting steer");
                        loopflow_execution::steer::delivered(&store_clone, session, &claimed);
                        claimed.text
                    }),
                )
            }
        };

        let report = |moment, callback, inbox| fixture.boundary(store, moment, callback, inbox);
        let event = |name| serde_json::json!({"hook_event_name":name, "session_id":thread});
        let steer = || {
            store
                .save_session_steer(session, "Fixture author", "use the other table")
                .unwrap()
        };
        if wake_at == WakeAt::Reopen {
            // The previous process quit mid-turn. Its successor's first
            // boundary makes the saved conversation reachable before input.
            report(BoundaryMoment::Tool, event("UserPromptSubmit"), None);
            store
                .conn
                .lock()
                .unwrap()
                .execute("UPDATE session_activity SET agent_process_id='earlier'", [])
                .unwrap();
            steer();
            let (moment, callback) = match harness {
                HarnessKind::Codex | HarnessKind::Claude => (
                    BoundaryMoment::Start,
                    serde_json::json!({"hook_event_name":"SessionStart","session_id":thread,"source":"resume"}),
                ),
                HarnessKind::OpenCode => (BoundaryMoment::Tool, event("OpenCodeMessagesTransform")),
                HarnessKind::Pi => (
                    BoundaryMoment::Event,
                    serde_json::json!({"type":"session_start","sessionFile":thread}),
                ),
                HarnessKind::Antigravity => {
                    unreachable!("Antigravity reopens through its launch prompt")
                }
            };
            let after = report(moment, callback, inbox);
            assert!(
                matches!(after, Some(super::super::boundary::After::Reachable(id)) if id == session),
                "{harness:?} {wake_at:?}"
            );
        } else {
            report(BoundaryMoment::Stop, event("Stop"), inbox);
            steer();
        }
        assert!(
            !after_turn(store, session, terminals.as_deref()),
            "{harness:?} {wake_at:?} should wake immediately"
        );
        let text = message();
        assert!(
            text.contains("use the other table"),
            "{harness:?} {wake_at:?}: {text}"
        );
        let page = store.session_steers(session, 0, 8).unwrap();
        assert_eq!(page.items.len(), 1, "{harness:?} {wake_at:?}");
        assert!(
            page.items[0].delivered_at.is_some(),
            "{harness:?} {wake_at:?}"
        );
        assert_eq!(page.pending, 0, "{harness:?} {wake_at:?}");
        if let Some(registry) = terminals {
            registry.close();
        }
    }

    #[test]
    fn an_open_idle_antigravity_with_a_draft_defers_wake_until_the_prompt_is_empty() {
        let session = "wake-agy-draft";
        let fixture = fixture(session);
        let _home = pin_lf_home(fixture.home.path());
        let store = &fixture.store;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='agy',provider_thread='native' WHERE id=?1",
                [session],
            )
            .unwrap();
        let clear = fixture.home.path().join("clear");
        let cleared = fixture.home.path().join("cleared");
        let pasted = fixture.home.path().join("pasted");
        let rule = "─".repeat(80);
        let idle = agy_idle_screen();
        let script = format!(
            r#"stty -echo
printf '\033[?2004h\033[2J\033[H{rule}\r\n> keep this draft\r\n{rule}\r\nauto · Gemini 4 Neon · high\r\n'
while [ ! -f "{clear}" ]; do sleep 0.05; done
printf '{idle}'
: > "{cleared}"
IFS= read -r line
printf '%s' "$line" > "{pasted}"
"#,
            clear = clear.display(),
            cleared = cleared.display(),
            pasted = pasted.display(),
        );
        let (registry, _) = held_agy_terminal(store, session, &script, "keep this draft");
        fixture.boundary(
            store,
            BoundaryMoment::Stop,
            serde_json::json!({"hook_event_name":"Stop","session_id":"native"}),
            None,
        );
        store
            .save_session_steer(session, "Fixture author", "wait until draft clears")
            .unwrap();
        let path = fixture.store_path();
        let mut watcher = Watcher {
            terminals: Some(registry.clone()),
            ..Watcher::default()
        };
        watcher.read(&path);
        assert!(watcher.deferred, "draft must defer held-terminal wake");
        assert!(!pasted.exists(), "draft must not be clobbered");
        assert_eq!(store.session_steers(session, 0, 8).unwrap().pending, 1);

        // Once the person clears the draft, the next watcher tick wakes it
        // without needing another steer or revision move.
        std::fs::write(&clear, "").unwrap();
        wait_until("draft cleared", || cleared.exists());
        // Poll until Hosted has read the cleared screen from the holder.
        wait_until("deferred wake fired", || {
            watcher.read(&path);
            !watcher.deferred
        });
        assert_wake_pasted(&pasted);
        registry.close();
    }

    #[test]
    fn a_steer_read_during_antigravity_launch_wakes_once_it_is_reachable() {
        let session = "wake-agy-opening";
        let fixture = fixture(session);
        let _home = pin_lf_home(fixture.home.path());
        let store = &fixture.store;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='agy',provider_thread=NULL WHERE id=?1",
                [session],
            )
            .unwrap();
        let pasted = fixture.home.path().join("pasted");
        let (registry, _) =
            held_agy_terminal(store, session, &agy_idle_script(&pasted), "? for shortcuts");
        store
            .save_session_steer(session, "Fixture author", "arrived while opening")
            .unwrap();
        let mut watcher = Watcher {
            terminals: Some(registry.clone()),
            ..Watcher::default()
        };
        watcher.read_with(store);
        assert!(!watcher.deferred);

        // The launch records a reachable idle session without saving another
        // steer or Task update, and the provider has no opening hook.
        assert!(store.record_first_boundary(&fixture.adapter).unwrap());
        watcher.last_wake_pass = Some(std::time::UNIX_EPOCH);
        watcher.read_with(store);
        assert_wake_pasted(&pasted);
        registry.close();
    }

    #[test]
    fn wake_nudge_wakes_waiting_watcher_without_a_revision_move() {
        let session = "wake-nudge";
        let fixture = fixture(session);
        let _home = pin_lf_home(fixture.home.path());
        let store = &fixture.store;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider='agy',provider_thread='native' WHERE id=?1",
                [session],
            )
            .unwrap();
        let pasted = fixture.home.path().join("pasted");
        let script = agy_idle_script(&pasted);
        let (registry, _) = held_agy_terminal(store, session, &script, "? for shortcuts");

        // Steer lands mid-turn: the watcher observes the revision while the
        // session is working, so `watcher.deferred` stays false.
        fixture.boundary(
            store,
            BoundaryMoment::Tool,
            serde_json::json!({"hook_event_name":"UserPromptSubmit","session_id":"native"}),
            None,
        );
        store
            .save_session_steer(session, "Fixture author", "nudge after turn end")
            .unwrap();
        let path = fixture.store_path();
        let mut changes = loopflow_storage::changes::StoreChanges::watch(&path);
        let (mut watcher, waker) = Watcher::new(Some(registry.clone()), changes.waker());
        watcher.read_with(store);
        assert!(!watcher.deferred);

        // Turn ends: `session_activity` flips to yielded without moving either
        // `task_update_revision` or `steer_revision`.
        fixture.boundary(
            store,
            BoundaryMoment::Stop,
            serde_json::json!({"hook_event_name":"Stop","session_id":"native"}),
            None,
        );

        // Arm `StoreChanges` after the Stop write so no WAL event is pending,
        // and confirm the watcher would otherwise wait `UNSIGNALLED` (60 s).
        changes.wait(std::time::Duration::from_millis(1));
        assert_eq!(watcher.wait_timeout(), super::UNSIGNALLED);

        assert!(!pasted.exists(), "the steer is still waiting");
        // Deliver the nudge before waiting so the Linux fallback poll cannot
        // return from the wait before the nudge is sent.
        let nudged_at = std::time::Instant::now();
        waker.nudge.unwrap().retry_deferred();
        changes.wait(watcher.wait_timeout());
        assert!(
            nudged_at.elapsed() < std::time::Duration::from_secs(5),
            "WakeNudge must break UNSIGNALLED immediately"
        );
        watcher.read_with(store);
        assert!(!watcher.deferred);
        assert_wake_pasted(&pasted);
        registry.close();
    }

    #[test]
    fn a_work_session_that_starts_no_turn_keeps_its_news_for_the_next_boundary() {
        // Nothing listens on the first socket; the second takes the message
        // and closes without starting a turn.
        for refused in [false, true] {
            let session = format!("wake-refused-{refused}");
            let fixture = fixture(&session);
            let socket = fixture.home.path().join("s");
            let _messages = refused.then(|| extension(&socket, "", 1));
            fixture.settle("opencode", &socket);
            fixture.comment(&fixture.store, "c-1", "use the staging bucket");
            after_turn(&fixture.store, &session, None);
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
        after_turn(&fixture.store, "wake-claude", None);
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
        wake_idle(&fixture.store, None, None);
        fixture.stop(&fixture.store, &inbox);
        after_turn(&fixture.store, "wake-claude", None);
        let wire = received(&listener)[1]["message"]["content"].to_string();
        assert!(wire.contains("and the eu region"));
        assert!(
            !wire.contains("staging bucket") && !wire.contains("pr/1"),
            "delivered news is not repeated"
        );
    }

    #[test]
    fn paused_task_delivers_steers_and_child_updates_to_its_idle_conversation() {
        let session = "paused-inbox";
        let fixture = fixture(session);
        let store = &fixture.store;
        store.choose_task_primary(&fixture.task, session).unwrap();
        store
            .take_up_task_graph(&fixture.task, r#"{"name":"code","steps":[]}"#)
            .unwrap();
        let (listener, _) = fixture.idle_claude();
        listener.set_nonblocking(true).unwrap();
        store
            .begin_task_pause(
                &fixture.task,
                &loopflow_storage::process::Starter::Person { name: None },
                vec![],
            )
            .unwrap();
        let child = store
            .create_task(&loopflow_storage::work::task::NewTask {
                id: loopflow_model::durable::TaskId::new(),
                parent_task_id: Some(fixture.task.clone()),
                repository_id: store.task(&fixture.task).unwrap().unwrap().repository_id,
                project_id: None,
                title: "Child".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .abandon(
                &loopflow_model::durable::WorkRef::Task(child.id),
                "superseded",
            )
            .unwrap();
        fixture.comment(store, "c-1", "Review the staged changes");
        loopflow_execution::harness::task_updates::write(store, session).unwrap();
        after_turn(store, session, None);
        let frames = received(&listener);
        let content = frames[1]["message"]["content"].as_str().unwrap();
        assert!(content.contains("Review the staged changes"), "{content}");
        assert!(content.contains("was abandoned"), "{content}");
        assert_eq!(store.session_steers(session, 0, 8).unwrap().pending, 0);
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
        wake_idle(&restarted, None, None);
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

    #[test]
    fn an_idle_task_conversation_wakes_only_when_its_graph_settles() {
        use loopflow_storage::graph_process::RunOwner;

        let fixture = fixture("wake-graph");
        let (listener, inbox) = fixture.idle_claude();
        listener.set_nonblocking(true).unwrap();
        let store = &fixture.store;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/wake')",
                [fixture.task.as_str()],
            )
            .unwrap();
        let run = store.test_graph(
            "feature",
            "/wake",
            &[("implement", Some("succeeded")), ("gate", None)],
            None,
        );
        let path = fixture.store_path();
        let mut watcher = Watcher::default();
        watcher.read(&path);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );

        store
            .finish_run(RunOwner::GraphRun, &run, 20, Some(1), Some("lint failed"))
            .unwrap();
        watcher.read(&path);
        let content = received(&listener)[1]["message"]["content"].to_string();
        assert!(content.contains(&format!("Run feature ({run}) · failed")));
        assert!(content.contains("lint failed"));

        fixture.stop(store, &inbox);
        after_turn(store, "wake-graph", None);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
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
        after_turn(&fixture.store, "wake-bare", None);
        wake_idle(&fixture.store, None, None);
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
        after_turn(&fixture.store, "wake-gone", None);
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

    /// The steers saved since the last reading start each stopped Session
    /// they name once, however many name it; a running one and a steer
    /// already read start nothing.
    #[test]
    fn steers_start_a_stopped_session_once() {
        let fixture = fixture("wake-stopped");
        let started = |store: &SqliteStore| {
            let mut starts = Vec::new();
            start_stopped(
                store,
                |_| Ok(None),
                |session, request| {
                    starts.push((session.to_string(), request));
                    Ok(())
                },
            );
            starts
        };
        let running = fixture
            .store
            .save_session_steer("wake-stopped", "jack", "while running")
            .unwrap();
        assert!(
            started(&fixture.store).is_empty(),
            "{running:?} is read at a boundary"
        );
        fixture
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET agent_ended_at=unixepoch(), agent_pid=NULL, agent_os_started_at=NULL WHERE agent_process_id=?1",
                [&fixture.adapter.agent_process_id],
            )
            .unwrap();
        let first = fixture
            .store
            .save_session_steer("wake-stopped", "jack", "what word did I give you?")
            .unwrap();
        fixture
            .store
            .save_session_steer("wake-stopped", "jack", "and spell it backwards")
            .unwrap();
        assert_eq!(
            started(&fixture.store),
            [("wake-stopped".to_string(), first.id.to_string())]
        );
        // A restarted server's first reading starts nothing it read before.
        let restarted = loopflow_execution::store::open_sqlite(&fixture.store_path()).unwrap();
        assert!(started(&restarted).is_empty());
        assert_eq!(
            fixture
                .store
                .session_steers("wake-stopped", 0, 8)
                .unwrap()
                .pending,
            3,
            "unread until the started conversation takes them"
        );
    }

    /// A steer naming a stopped Session whose harness has no login spawns
    /// nothing, and `lf session list` says why. After a login, the next
    /// steer starts it and the failure is gone.
    #[test]
    fn a_denied_wakeup_shows_its_start_failure_until_a_later_steer_starts_it() {
        let fixture = fixture("wake-denied");
        fixture
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET agent_ended_at=unixepoch(), agent_pid=NULL, agent_os_started_at=NULL WHERE agent_process_id=?1",
                [&fixture.adapter.agent_process_id],
            )
            .unwrap();
        let listed = || {
            let shared = std::sync::Arc::new(loopflow_execution::store::Store {
                sqlite: fixture.store.clone(),
            });
            loopflow_execution::ops::human_session::listed(&shared, "wake-denied")
                .unwrap()
                .unwrap()
                .status_line
                .unwrap()
        };
        let login = "codex has no usable login on this Machine; run `codex login`";
        let mut starts = Vec::new();
        let read = |ready: Option<&str>, starts: &mut Vec<String>| {
            start_stopped(
                &fixture.store,
                |_| {
                    Ok(ready.map(|reason| {
                        format!(
                            "{}{reason}",
                            loopflow_execution::ops::recovery::WAITING_FOR_HARNESS
                        )
                    }))
                },
                |_, request| {
                    starts.push(request);
                    Ok(())
                },
            )
        };

        fixture
            .store
            .save_session_steer("wake-denied", "jack", "are you there?")
            .unwrap();
        read(Some(login), &mut starts);
        assert!(starts.is_empty(), "an unavailable harness is not spawned");
        let line = listed();
        assert!(
            line.starts_with(&format!("could not start: {login}")),
            "{line}"
        );

        // The same steer is not read twice; a new one, after a login, starts it.
        read(None, &mut starts);
        assert!(starts.is_empty());
        let next = fixture
            .store
            .save_session_steer("wake-denied", "jack", "logged in now")
            .unwrap();
        read(None, &mut starts);
        assert_eq!(starts, [next.id.to_string()]);
        let line = listed();
        assert!(!line.contains("could not start"), "{line}");
    }

    fn park_breakpoint(
        fixture: &Fixture,
        step: &str,
        text: &str,
    ) -> loopflow_model::id::LfProcessId {
        let store = &fixture.store;
        store
            .take_up_task_graph(&fixture.task, r#"{"name":"feature","steps":[]}"#)
            .unwrap();
        let carrier = loopflow_model::id::LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&carrier],
            )
            .unwrap();
            conn.execute(
                "UPDATE task_graphs SET breakpoint=NULL,lf_process_id=?2,run_start='{\"index\":0}' WHERE task_id=?1",
                rusqlite::params![fixture.task.as_str(), carrier.as_str()],
            )
            .unwrap();
        }
        let record =
            serde_json::json!({"id": carrier.as_str(), "cursor": {"index": 1}}).to_string();
        store
            .arrive_task_graph(&fixture.task, &carrier, step, Some((&record, text)))
            .unwrap();
        carrier
    }

    #[test]
    fn a_held_breakpoint_wakes_the_idle_primary_on_unpause() {
        use loopflow_storage::graph_process::RunOwner;
        use loopflow_storage::process::Starter;

        let session = "held-review";
        let fixture = fixture(session);
        let store = &fixture.store;
        store.choose_task_primary(&fixture.task, session).unwrap();
        store
            .take_up_task_graph(&fixture.task, r#"{"name":"feature","steps":[]}"#)
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/wake')",
                [fixture.task.as_str()],
            )
            .unwrap();
        let (listener, _) = fixture.idle_claude();
        listener.set_nonblocking(true).unwrap();
        let run = store.test_graph("feature", "/wake", &[("implement", None)], None);
        let mut watcher = Watcher::default();
        watcher.read_with(store);

        store
            .begin_task_pause(&fixture.task, &Starter::Person { name: None }, vec![])
            .unwrap();
        store.settle_task_pause(&fixture.task, true).unwrap();
        park_breakpoint(&fixture, "after-explore", "Compare scratch/explore.html");
        store
            .finish_run(RunOwner::GraphRun, &run, 20, Some(0), None)
            .unwrap();
        watcher.read_with(store);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "the arrival and its Graph progress wait during the hold"
        );

        store.release_task_pause(&fixture.task, None).unwrap();
        watcher.read_with(store);
        let frames = received(&listener);
        let content = frames[1]["message"]["content"].as_str().unwrap();
        assert!(
            content.contains("Compare scratch/explore.html"),
            "{content}"
        );
        assert_eq!(content.matches("Compare scratch/explore.html").count(), 1);
    }

    /// Breakpoint delivery through provider inboxes and at an Antigravity turn
    /// boundary, including a busy primary and a failed held start after restart.
    #[test]
    fn breakpoint_delivery_covers_five_harnesses_busy_restart_and_failed_start() {
        // 1. Open-idle Claude + busy turn delivery.
        let claude = fixture("brk-claude");
        claude
            .store
            .choose_task_primary(&claude.task, "brk-claude")
            .unwrap();
        let (listener, inbox) = claude.idle_claude();
        listener.set_nonblocking(true).unwrap();
        // Busy turn: watcher does not wake until the turn ends.
        claude
            .store
            .record_native_turn(&claude.adapter, "native", false, None)
            .unwrap();
        park_breakpoint(&claude, "after-explore", "Compare scratch/explore.html");
        wake_idle(&claude.store, None, None);
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "busy primary is not woken mid-turn"
        );
        claude.stop(&claude.store, &inbox);
        after_turn(&claude.store, "brk-claude", None);
        listener.set_nonblocking(false).unwrap();
        let content = received(&listener)[1]["message"]["content"].to_string();
        assert!(content.contains("Breakpoint review "), "{content}");
        assert!(
            content.contains("Compare scratch/explore.html"),
            "{content}"
        );

        // 2. Open-idle Codex.
        let codex = fixture("brk-codex");
        codex
            .store
            .choose_task_primary(&codex.task, "brk-codex")
            .unwrap();
        let endpoint = codex.home.path().join("codex.sock");
        {
            let conn = codex.store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_sessions SET provider='codex',agent_endpoint=?2 WHERE id=?1",
                rusqlite::params!["brk-codex", endpoint.to_string_lossy()],
            )
            .unwrap();
        }
        codex
            .store
            .record_native_turn(&codex.adapter, "native", true, Some(0))
            .unwrap();
        park_breakpoint(&codex, "after-explore", "Codex review artifact");
        let server = codex_app_server(&endpoint);
        wake_idle(&codex.store, Some("brk-codex"), None);
        let start = server.join().unwrap();
        let output = start["params"]["toolOutput"]["output"].to_string();
        assert!(output.contains("Codex review artifact"), "{output}");

        // 3. Open-idle OpenCode and Pi.
        for provider in ["opencode", "pi"] {
            let session = format!("brk-{provider}");
            let fx = fixture(&session);
            fx.store.choose_task_primary(&fx.task, &session).unwrap();
            let socket = fx.home.path().join("ext.sock");
            let messages = extension(&socket, "started\n", 1);
            fx.settle(provider, &socket);
            park_breakpoint(&fx, "after-explore", &format!("{provider} review"));
            wake_idle(&fx.store, Some(&session), None);
            let text = messages.try_recv().expect(provider)["text"].to_string();
            assert!(text.contains(&format!("{provider} review")), "{text}");
        }

        // 4. Antigravity: next-turn claim while open, then held-start after quit
        //    with server restart before delivery and a failed start first.
        let agy = fixture("brk-agy");
        agy.store.choose_task_primary(&agy.task, "brk-agy").unwrap();
        {
            let conn = agy.store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_sessions SET provider='antigravity' WHERE id='brk-agy'",
                [],
            )
            .unwrap();
        }
        park_breakpoint(&agy, "after-explore", "Agy next-turn review");
        let claimed = loopflow_execution::steer::claim(
            &agy.store,
            agy.home.path(),
            &agy.adapter,
            "native",
            loopflow_execution::steer::NativeBoundary::Turn,
        )
        .unwrap()
        .expect("next-turn hook receives breakpoint");
        assert!(
            claimed.text.contains("Agy next-turn review"),
            "{}",
            claimed.text
        );
        loopflow_execution::steer::delivered(&agy.store, "brk-agy", &claimed);

        // Quit during work, then a second breakpoint arrives while server is down.
        {
            let conn = agy.store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_sessions SET agent_ended_at=unixepoch(),agent_pid=NULL,agent_os_started_at=NULL WHERE id='brk-agy'",
                [],
            )
            .unwrap();
        }
        let second = park_breakpoint(&agy, "after-ship-review", "Agy held-start review");
        let restarted = loopflow_execution::store::open_sqlite(&agy.store_path()).unwrap();
        let mut starts = Vec::new();
        start_stopped(
            &restarted,
            |_| Ok(Some("antigravity unavailable".into())),
            |session, request| {
                starts.push((session.to_string(), request));
                Ok(())
            },
        );
        assert!(starts.is_empty(), "failed readiness spawns nothing");
        assert_eq!(
            restarted.session_steers("brk-agy", 0, 8).unwrap().pending,
            1,
            "failed start leaves the breakpoint steer unread"
        );
        let manual = restarted.take_steers_to_start("brk-agy").unwrap();
        assert_eq!(manual.len(), 1);
        assert_eq!(manual[0].revision, second.as_str());
        assert!(manual[0].text.contains("Agy held-start review"));
    }
}
