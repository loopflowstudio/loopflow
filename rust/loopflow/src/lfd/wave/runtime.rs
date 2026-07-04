//! The wave server's in-process state, folded over the journal.
//!
//! A wave is a long-lived reactive server, not a loop. Its truth is the
//! per-wave append-only [`Journal`]; everything the server holds in memory is
//! a materialized fold of it:
//!
//! - the `thread` (`Vec<ChatTurn>`) is the fold of conversation events —
//!   rebuilt from the journal on boot, so a restart keeps the full thread and
//!   turn ids continue monotonically (they derive from the journal seq);
//! - the open turn is a live snapshot grown from the same deltas the journal
//!   records — served after the finalized thread and re-broadcast as it grows,
//!   so subscribers watch a turn stream instead of minutes of silence;
//! - the mind state is the last `MindState` event;
//! - the SSE broadcast is liveness only — a subscriber that lags resyncs from
//!   the store.
//!
//! Two independent event sources feed the journal: subagent progress deltas
//! (via [`TurnSink`]) and user messages (HTTP → inbox channel). All appends
//! go through one lock, so journal order, cache order, and broadcast order
//! agree — one writer appends and broadcasts.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{broadcast, mpsc};

use crate::lfd::conversations::turns::{ChatRole, ChatTurn, TurnDelta};
use crate::lfd::conversations::types::{ConversationItem, Lifecycle};
use crate::lfd::wave::journal::{
    fold_thread, journal_path, Event, EventKind, Journal, MessageId, MessageOp, Usage,
};
use crate::lfd::wave::memory::Memory;
use crate::lfd::wave::state::{can_transition, MindState};
use crate::lfd::wave::supervisor::Supervisor;

/// Capacity of the live turn broadcast. SSE clients that fall this far behind
/// get a lag error and resync from `/conversation`; the journal is the source
/// of truth, so a dropped live turn is never lost.
const TURN_BROADCAST_CAPACITY: usize = 256;

/// A user message pulled from the inbox, awaiting a reply.
#[derive(Debug, Clone)]
pub struct UserMessage {
    pub id: MessageId,
    pub text: String,
}

/// Everything that must stay mutually consistent: the journal (truth), the
/// thread cache (fold of it), the open-turn snapshot, and the mind state
/// (last transition).
#[derive(Debug)]
struct Inner {
    journal: Journal,
    thread: Vec<ChatTurn>,
    /// The turn currently in progress, grown delta by delta (status
    /// `Running`). Served after the finalized thread and re-broadcast on every
    /// content delta so subscribers watch it grow; cleared at finalization,
    /// when the terminal turn commits to `thread` under the same id.
    open_turn: Option<ChatTurn>,
    state: MindState,
}

/// The whole live state of one running wave server.
#[derive(Debug)]
pub struct WaveRuntime {
    name: String,
    repo_root: PathBuf,
    /// Journal + materialized thread + mind state, behind one lock so their
    /// orders never diverge.
    inner: Mutex<Inner>,
    /// Fans turn frames out to live SSE subscribers: open-turn snapshots as a
    /// turn grows, then the terminal turn under the same id.
    turn_tx: broadcast::Sender<ChatTurn>,
    /// Durable shared brain (read-only here; the mind curates it deliberately).
    memory: Memory,
    /// In-process user-message inbox (a channel, not a file).
    inbox_tx: mpsc::UnboundedSender<UserMessage>,
    /// Tracks all live subagent runs.
    supervisor: Arc<Supervisor>,
}

impl WaveRuntime {
    /// Open the runtime against the wave's journal, replaying it: the thread
    /// cache is rebuilt from the log and turn ids continue from its seq.
    ///
    /// Boot janitor: turns left open by a crash are finalized as `Failed`
    /// (appended to the journal, so the log itself is closed), and a non-idle
    /// mind state settles back to `Idle`.
    ///
    /// # Errors
    /// Journal I/O failure or an unreadable (future-versioned) journal.
    pub fn open(
        name: String,
        repo_root: PathBuf,
    ) -> anyhow::Result<(Arc<Self>, mpsc::UnboundedReceiver<UserMessage>)> {
        let (mut journal, events) = Journal::open(&journal_path(&repo_root, &name))?;
        let mut fold = fold_thread(&events);

        // Janitor: a turn without a TurnFinished crashed with the server.
        for mut turn in fold.open {
            journal.append(|_| EventKind::TurnFinished {
                turn_id: turn.id.clone(),
                status: Lifecycle::Failed,
                usage: Usage::empty(),
            });
            turn.status = Lifecycle::Failed;
            fold.turns.push(turn);
        }
        // Janitor: no turn is live on a fresh boot, whatever the log says.
        let state = if fold.state == MindState::Idle {
            MindState::Idle
        } else {
            journal.append(|_| EventKind::MindState {
                from: fold.state.clone(),
                to: MindState::Idle,
                reason: "startup janitor: no live turn after restart".to_string(),
            });
            MindState::Idle
        };

        let (turn_tx, _) = broadcast::channel(TURN_BROADCAST_CAPACITY);
        let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
        let memory = Memory::for_wave(&repo_root, &name);
        let runtime = Arc::new(Self {
            name,
            repo_root,
            inner: Mutex::new(Inner {
                journal,
                thread: fold.turns,
                open_turn: None,
                state,
            }),
            turn_tx,
            memory,
            inbox_tx,
            supervisor: Supervisor::new(),
        });
        Ok((runtime, inbox_rx))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn repo_root(&self) -> &std::path::Path {
        &self.repo_root
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    pub fn supervisor(&self) -> &Arc<Supervisor> {
        &self.supervisor
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("wave runtime lock poisoned")
    }

    /// Snapshot of the whole thread — finalized turns plus the open turn
    /// (status `Running`), if one is in progress — for `/conversation`.
    pub fn thread_snapshot(&self) -> Vec<ChatTurn> {
        snapshot_locked(&self.inner())
    }

    /// Current mind state, for `/health` and the composer.
    pub fn mind_state(&self) -> MindState {
        self.inner().state.clone()
    }

    /// Atomically snapshot the thread (including the open turn) and subscribe
    /// to live frames. Every broadcast happens under the same lock as the
    /// append it reflects, so the receiver sees exactly the frames sent after
    /// this snapshot — no gap, no overlap, no frame older than the snapshot.
    /// A live frame's id may match a snapshot turn: it is that turn, newer;
    /// consumers replace by id.
    pub fn subscribe_with_snapshot(&self) -> (Vec<ChatTurn>, broadcast::Receiver<ChatTurn>) {
        let inner = self.inner();
        (snapshot_locked(&inner), self.turn_tx.subscribe())
    }

    /// Attempt a mind-state transition. Legal moves append a `MindState` event
    /// and apply; illegal moves are refused and logged — an illegal transition
    /// is a bug, never silently applied.
    pub fn transition(&self, to: MindState, reason: &str) -> bool {
        let mut inner = self.inner();
        self.transition_locked(&mut inner, to, reason)
    }

    fn transition_locked(&self, inner: &mut Inner, to: MindState, reason: &str) -> bool {
        if !can_transition(&inner.state, &to) {
            tracing::warn!(
                from = inner.state.name(),
                to = to.name(),
                reason,
                "illegal mind-state transition refused"
            );
            return false;
        }
        let from = std::mem::replace(&mut inner.state, to.clone());
        inner.journal.append(|_| EventKind::MindState {
            from,
            to,
            reason: reason.to_string(),
        });
        true
    }

    /// Push a turn into the thread cache and broadcast it live. The journal
    /// events for the turn must already be appended (same lock).
    fn commit_locked(&self, inner: &mut Inner, turn: ChatTurn) -> ChatTurn {
        inner.thread.push(turn.clone());
        // A send error just means no live subscribers — the store has it.
        let _ = self.turn_tx.send(turn.clone());
        turn
    }

    /// Deliver a user message: append its `UserMessage` event, commit the user
    /// turn (id from the event's seq), and hand it to the inbox for the chat
    /// consumer. Returns the stored user turn so the HTTP handler can echo it.
    ///
    /// Wire body is `{text}` for now, so the op is always `Message`; steering
    /// ops land with the mind phase.
    pub fn deliver_user_message(&self, text: String) -> ChatTurn {
        let (turn, id) = {
            let mut inner = self.inner();
            let event = inner.journal.append(|seq| EventKind::UserMessage {
                id: MessageId(format!("msg-{seq}")),
                op: MessageOp::Message,
                text: text.clone(),
            });
            let id = MessageId(format!("msg-{}", event.seq));
            let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text.clone());
            turn.created_at = event.at_rfc3339();
            (self.commit_locked(&mut inner, turn), id)
        };
        // Unbounded inbox: delivering a message never blocks progress.
        let _ = self.inbox_tx.send(UserMessage { id, text });
        turn
    }

    /// Record an already-finalized turn as its full event triple
    /// (`TurnStarted` + `TurnItem`s + `TurnFinished`) and commit it. Text
    /// becomes a `Message` item so the fold reproduces it. Does not touch the
    /// mind state — this is for instantaneous turns (chat replies, injected
    /// narration), not mind turns.
    pub fn append_finalized_turn(&self, turn: ChatTurn, answers: Vec<MessageId>) -> ChatTurn {
        let mut inner = self.inner();
        let started = inner.journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers,
        });
        let turn_id = format!("turn-{}", started.seq);
        if !turn.text.is_empty() {
            inner.journal.append(|_| EventKind::TurnItem {
                turn_id: turn_id.clone(),
                item: ConversationItem::Message {
                    id: "text-0".to_string(),
                    text: turn.text.clone(),
                    phase: None,
                },
            });
        }
        for item in &turn.items {
            inner.journal.append(|_| EventKind::TurnItem {
                turn_id: turn_id.clone(),
                item: item.clone(),
            });
        }
        inner.journal.append(|_| EventKind::TurnFinished {
            turn_id: turn_id.clone(),
            status: turn.status,
            usage: Usage::empty(),
        });
        let committed = ChatTurn {
            id: turn_id,
            created_at: started.at_rfc3339(),
            ..turn
        };
        self.commit_locked(&mut inner, committed)
    }

    /// Reaction 2 (chat): compose a TALK-ONLY reply from memory + current
    /// progress state and record it as a turn answering `message`. Chat
    /// observes; it does not steer progress (no memory writes, no spawning,
    /// no reprioritizing).
    pub fn reply_to(&self, message: &UserMessage) -> ChatTurn {
        let reply = compose_reply(message, &self.thread_snapshot(), &self.memory);
        self.append_finalized_turn(
            ChatTurn {
                id: String::new(),
                role: ChatRole::Assistant,
                text: reply,
                status: Lifecycle::Completed,
                items: Vec::new(),
                created_at: String::new(),
            },
            vec![message.id.clone()],
        )
    }

    // -- TurnSink internals (same lock discipline as everything above) --
    //
    // Every content delta (opened / text / item) re-broadcasts the open-turn
    // snapshot under the same id. No debounce: deltas are item-granular from
    // the vendor stream (one per completed item, not per token), so the
    // natural rate is well under any flood threshold, and a suppressed
    // trailing frame would leave subscribers stale through a long tool call.
    // A throttle earns its place with the part-grained wire, not before.

    fn sink_turn_started(&self) -> Event {
        let mut inner = self.inner();
        let event = inner.journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            // TODO(mind phase): name the queued MessageIds the pass's prompt
            // consumed. build_progress_prompt doesn't read the inbox yet, so
            // claiming answers here would be a lie.
            answers: Vec::new(),
        });
        let turn_id = format!("turn-{}", event.seq);
        self.transition_locked(
            &mut inner,
            MindState::Turning {
                turn_id: turn_id.clone(),
            },
            "turn opened",
        );
        let open = ChatTurn {
            id: turn_id,
            role: ChatRole::Assistant,
            text: String::new(),
            status: Lifecycle::Running,
            items: Vec::new(),
            created_at: event.at_rfc3339(),
        };
        let _ = self.turn_tx.send(open.clone());
        inner.open_turn = Some(open);
        event
    }

    fn sink_turn_item(&self, turn_id: &str, item: ConversationItem) {
        let mut inner = self.inner();
        inner.journal.append(|_| EventKind::TurnItem {
            turn_id: turn_id.to_string(),
            item: item.clone(),
        });
        // Grow the open-turn snapshot exactly as the fold does (`Message`
        // items join into text, the rest append to items) and re-broadcast it
        // so live subscribers watch the turn in progress.
        let Some(open) = inner.open_turn.as_mut() else {
            return;
        };
        if let ConversationItem::Message { text, .. } = &item {
            if !open.text.is_empty() {
                open.text.push('\n');
            }
            open.text.push_str(text);
        } else {
            open.items.push(item);
        }
        let _ = self.turn_tx.send(open.clone());
    }

    fn sink_turn_finished(&self, turn: ChatTurn, usage: Usage) -> ChatTurn {
        let mut inner = self.inner();
        inner.journal.append(|_| EventKind::TurnFinished {
            turn_id: turn.id.clone(),
            status: turn.status,
            usage,
        });
        self.transition_locked(&mut inner, MindState::Idle, "turn finalized");
        // The terminal turn replaces the open snapshot under the same id.
        inner.open_turn = None;
        self.commit_locked(&mut inner, turn)
    }
}

/// The thread plus the open turn, in one clone. The open turn rides last:
/// clients order by the sequence in the turn id, not array position.
fn snapshot_locked(inner: &Inner) -> Vec<ChatTurn> {
    let mut turns = inner.thread.clone();
    turns.extend(inner.open_turn.clone());
    turns
}

/// Folds a subagent pass's [`TurnDelta`]s into journal events and committed
/// turns: `Opened` → `TurnStarted` (+ `Idle → Turning`), text/items →
/// `TurnItem`s, usage accrues, `Finished` → `TurnFinished` (+ `Turning →
/// Idle`) and the turn commits to the thread. One sink per pass.
#[derive(Debug)]
pub struct TurnSink {
    runtime: Arc<WaveRuntime>,
    open: Option<OpenTurn>,
}

#[derive(Debug)]
struct OpenTurn {
    turn_id: String,
    started_at: String,
    /// Count of prose fragments, for `Message` item ids (`"text-<n>"`).
    text_items: usize,
    usage: Usage,
}

impl TurnSink {
    pub fn new(runtime: Arc<WaveRuntime>) -> Self {
        Self {
            runtime,
            open: None,
        }
    }

    pub fn on_delta(&mut self, delta: TurnDelta) {
        match delta {
            TurnDelta::Opened => {
                let event = self.runtime.sink_turn_started();
                self.open = Some(OpenTurn {
                    turn_id: format!("turn-{}", event.seq),
                    started_at: event.at_rfc3339(),
                    text_items: 0,
                    usage: Usage::empty(),
                });
            }
            TurnDelta::Text(text) => {
                let Some(open) = self.open.as_mut() else {
                    tracing::warn!("text delta with no open turn; dropped");
                    return;
                };
                let item = ConversationItem::Message {
                    id: format!("text-{}", open.text_items),
                    text,
                    phase: None,
                };
                open.text_items += 1;
                let turn_id = open.turn_id.clone();
                self.runtime.sink_turn_item(&turn_id, item);
            }
            TurnDelta::Item(item) => {
                let Some(open) = self.open.as_ref() else {
                    tracing::warn!("item delta with no open turn; dropped");
                    return;
                };
                let turn_id = open.turn_id.clone();
                self.runtime.sink_turn_item(&turn_id, item);
            }
            TurnDelta::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
            } => {
                let Some(open) = self.open.as_mut() else {
                    return;
                };
                open.usage.input_tokens = add_opt(open.usage.input_tokens, input_tokens);
                open.usage.output_tokens = add_opt(open.usage.output_tokens, output_tokens);
                open.usage.cache_read_tokens =
                    add_opt(open.usage.cache_read_tokens, cache_read_tokens);
            }
            TurnDelta::Finished { mut turn, cost_usd } => {
                let Some(open) = self.open.take() else {
                    tracing::warn!("finished delta with no open turn; recording whole");
                    self.runtime.append_finalized_turn(turn, Vec::new());
                    return;
                };
                let mut usage = open.usage;
                usage.cost_usd = cost_usd;
                turn.id = open.turn_id;
                turn.created_at = open.started_at;
                self.runtime.sink_turn_finished(turn, usage);
            }
        }
    }
}

fn add_opt(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    }
}

/// Drain the inbox, replying to each user message. One reply per message,
/// TALK-ONLY. Runs until the inbox closes (server shutdown).
pub async fn run_chat_consumer(
    runtime: Arc<WaveRuntime>,
    mut inbox_rx: mpsc::UnboundedReceiver<UserMessage>,
) {
    while let Some(message) = inbox_rx.recv().await {
        runtime.reply_to(&message);
    }
}

/// Build a talk-only reply purely by observing state: what progress has
/// narrated so far, and the head of memory. Deterministic — this is where a
/// chat LLM would slot in later, but even now it must only read, never steer.
fn compose_reply(message: &UserMessage, thread: &[ChatTurn], memory: &Memory) -> String {
    let progress: Vec<&ChatTurn> = thread
        .iter()
        .filter(|t| t.role == ChatRole::Assistant && !t.text.trim().is_empty())
        .collect();
    let latest = progress
        .last()
        .map(|t| snippet(&t.text, 200))
        .unwrap_or_else(|| "no progress narrated yet".to_string());

    let mut reply = format!(
        "You asked: \"{}\".\n\nProgress so far: {} update(s). Latest: {}.",
        snippet(&message.text, 200),
        progress.len(),
        latest,
    );

    let mem = memory.head(6);
    if !mem.trim().is_empty() {
        reply.push_str("\n\nFrom memory:\n");
        reply.push_str(&mem);
    }
    reply
}

fn snippet(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let truncated: String = flat.chars().take(max).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::conversations::types::Lifecycle;

    /// Parse the sequence out of a `"turn-<n>"` id; panics on a malformed one
    /// (ids are always minted from journal seqs).
    fn turn_seq(id: &str) -> u64 {
        id.strip_prefix("turn-")
            .and_then(|n| n.parse().ok())
            .expect("turn id minted from journal seq")
    }

    fn progress_turn(text: &str) -> ChatTurn {
        ChatTurn {
            id: String::new(),
            role: ChatRole::Assistant,
            text: text.to_string(),
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at: String::new(),
        }
    }

    fn open_runtime(
        repo: &std::path::Path,
    ) -> (Arc<WaveRuntime>, mpsc::UnboundedReceiver<UserMessage>) {
        WaveRuntime::open("ship".into(), repo.to_path_buf()).expect("open runtime")
    }

    #[test]
    fn turns_get_monotonic_ids_from_the_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        let a = rt.append_finalized_turn(progress_turn("one"), Vec::new());
        let b = rt.append_finalized_turn(progress_turn("two"), Vec::new());
        assert!(turn_seq(&b.id) > turn_seq(&a.id));
        assert_eq!(rt.thread_snapshot().len(), 2);
    }

    #[test]
    fn narrated_turns_no_longer_blob_memory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        rt.append_finalized_turn(progress_turn("landed the parser"), Vec::new());
        // The journal carries raw history; MEMORY.md stays untouched until
        // the mind curates it deliberately.
        assert_eq!(rt.memory().read(), "");
    }

    #[test]
    fn deliver_user_message_appends_user_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, mut rx) = open_runtime(tmp.path());
        let turn = rt.deliver_user_message("how goes it?".into());
        assert_eq!(turn.role, ChatRole::User);
        assert_eq!(turn.text, "how goes it?");
        // The message landed in the inbox for the consumer, id tied to its event.
        let msg = rx.try_recv().expect("inbox message");
        assert_eq!(msg.text, "how goes it?");
        assert_eq!(msg.id, MessageId(format!("msg-{}", turn_seq(&turn.id))));
    }

    #[test]
    fn reply_reflects_progress_and_memory_without_steering() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).expect("dir");
        std::fs::write(tmp.path().join("wave/ship/MEMORY.md"), "Goal: ship it.\n").expect("mem");
        let (rt, _rx) = open_runtime(tmp.path());
        rt.append_finalized_turn(progress_turn("wired the reactive server"), Vec::new());
        let mem_before = rt.memory().read();

        let reply = rt.reply_to(&UserMessage {
            id: MessageId("msg-99".into()),
            text: "status?".into(),
        });

        assert_eq!(reply.role, ChatRole::Assistant);
        assert!(reply.text.contains("1 update"));
        assert!(reply.text.contains("wired the reactive server"));
        // Talk-only: replying didn't write memory.
        assert_eq!(rt.memory().read(), mem_before);
    }

    #[test]
    fn illegal_transition_is_refused_and_leaves_state_alone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        assert_eq!(rt.mind_state(), MindState::Idle);

        // Nothing to interrupt when idle.
        assert!(!rt.transition(
            MindState::Interrupting {
                turn_id: "turn-1".into()
            },
            "test"
        ));
        assert_eq!(rt.mind_state(), MindState::Idle);

        // Legal: a turn opens, then finishes.
        assert!(rt.transition(
            MindState::Turning {
                turn_id: "turn-1".into()
            },
            "test"
        ));
        assert!(!rt.transition(
            MindState::Turning {
                turn_id: "turn-2".into()
            },
            "test"
        ));
        assert!(rt.transition(MindState::Idle, "test"));
    }

    #[test]
    fn turn_sink_journals_a_pass_and_commits_the_turn() {
        use crate::engine::stream::{ResultSubtype, StreamEvent};
        use crate::lfd::conversations::turns::TurnBuilder;

        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        let mut sink = TurnSink::new(rt.clone());
        let mut builder = TurnBuilder::new();

        for event in [
            StreamEvent::Text("hello".into()),
            StreamEvent::ToolUse {
                name: "Bash".into(),
                summary: "cargo test".into(),
            },
            StreamEvent::Usage {
                input_tokens: Some(10),
                output_tokens: Some(4),
                cache_read_tokens: None,
            },
            StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: Some(0.02),
                duration_secs: None,
            },
        ] {
            // Mid-pass the mind is Turning.
            for delta in builder.feed(&event) {
                sink.on_delta(delta);
            }
        }

        assert_eq!(rt.mind_state(), MindState::Idle, "back to idle after turn");
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 1);
        let turn = &thread[0];
        assert_eq!(turn.text, "hello");
        assert_eq!(turn.items.len(), 1);
        assert_eq!(turn.status, Lifecycle::Completed);
        // The id comes from the journal seq domain (turn_seq panics otherwise).
        turn_seq(&turn.id);
    }

    #[test]
    fn open_turn_streams_growing_snapshots_then_the_terminal_turn() {
        use crate::engine::stream::{ResultSubtype, StreamEvent};
        use crate::lfd::conversations::turns::TurnBuilder;

        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        let (snapshot, mut frames) = rt.subscribe_with_snapshot();
        assert!(snapshot.is_empty());

        let mut sink = TurnSink::new(rt.clone());
        let mut builder = TurnBuilder::new();
        let mut feed = |event: &StreamEvent| {
            for delta in builder.feed(event) {
                sink.on_delta(delta);
            }
        };

        // First content delta: an empty running turn opens and broadcasts,
        // then the text lands in a second frame under the same id.
        feed(&StreamEvent::Text("thinking".into()));
        let opened = frames.try_recv().expect("opened frame");
        assert_eq!(opened.status, Lifecycle::Running);
        assert_eq!(opened.text, "");
        let grown = frames.try_recv().expect("text frame");
        assert_eq!(grown.id, opened.id);
        assert_eq!(grown.text, "thinking");
        assert_eq!(grown.status, Lifecycle::Running);

        // Mid-turn, the open turn rides the snapshot after the thread.
        let mid = rt.thread_snapshot();
        assert_eq!(mid.len(), 1);
        assert_eq!(mid[0].id, opened.id);
        assert_eq!(mid[0].status, Lifecycle::Running);

        // An item delta grows the same snapshot.
        feed(&StreamEvent::ToolUse {
            name: "Bash".into(),
            summary: "cargo test".into(),
        });
        let with_item = frames.try_recv().expect("item frame");
        assert_eq!(with_item.id, opened.id);
        assert_eq!(with_item.items.len(), 1);
        assert_eq!(with_item.text, "thinking");

        // Finalization replaces the running turn under the same id.
        feed(&StreamEvent::Result {
            subtype: ResultSubtype::Success,
            cost_usd: None,
            duration_secs: None,
        });
        let terminal = frames.try_recv().expect("terminal frame");
        assert_eq!(terminal.id, opened.id);
        assert_eq!(terminal.status, Lifecycle::Completed);

        // No stale running turn remains anywhere.
        let after = rt.thread_snapshot();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].status, Lifecycle::Completed);
        assert!(frames.try_recv().is_err(), "no extra frames");
    }

    #[tokio::test]
    async fn chat_consumer_replies_to_each_message() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, rx) = open_runtime(tmp.path());
        let consumer = tokio::spawn(run_chat_consumer(rt.clone(), rx));

        rt.deliver_user_message("first".into());
        rt.deliver_user_message("second".into());

        // Wait for both replies to land: 2 user turns + 2 replies = 4.
        for _ in 0..50 {
            if rt.thread_snapshot().len() >= 4 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 4, "each message gets exactly one reply");
        let users = thread.iter().filter(|t| t.role == ChatRole::User).count();
        let replies = thread
            .iter()
            .filter(|t| t.role == ChatRole::Assistant)
            .count();
        assert_eq!(users, 2, "both user messages appear as turns");
        assert_eq!(replies, 2, "each message gets exactly one reply");

        consumer.abort();
    }
}
