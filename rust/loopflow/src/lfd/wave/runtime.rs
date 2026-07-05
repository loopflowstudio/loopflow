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
//! - the vendor thread id is the last `ThreadStarted` event — the mind's
//!   resume handle;
//! - the SSE broadcast is liveness only — a subscriber that lags resyncs from
//!   the store.
//!
//! Two independent inputs feed the journal: the mind's harness events (via
//! [`TurnSink`], driven by the mind's scheduler in [`super::mind`]) and user
//! messages (HTTP → inbox channel). All appends go through one lock, so
//! journal order, cache order, and broadcast order agree — one writer appends
//! and broadcasts.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{broadcast, mpsc};

use crate::lfd::conversations::turns::{ChatRole, ChatTurn, TurnDelta};
use crate::lfd::conversations::types::{ConversationItem, Lifecycle};
use crate::lfd::wave::journal::{
    fold_thread, fold_workers, journal_path, Attribution, Event, EventKind, Journal, MessageId,
    MessageOp, PendingMessage, Usage, WorkerOutcome, WorkerRecord,
};
use crate::lfd::wave::memory::Memory;
use crate::lfd::wave::state::{can_transition, MindState};

/// Capacity of the live turn broadcast. SSE clients that fall this far behind
/// get a lag error and resync from `/conversation`; the journal is the source
/// of truth, so a dropped live turn is never lost.
const TURN_BROADCAST_CAPACITY: usize = 256;

/// Capacity of the live mind-state broadcast. Transitions are rare (a few per
/// turn); a lagged subscriber just resyncs from the next transition.
const STATE_BROADCAST_CAPACITY: usize = 64;

/// A journaled user message pulled from the inbox, awaiting consumption
/// (named in a `TurnStarted.answers` or `TurnSteered.answers`).
#[derive(Debug, Clone)]
pub struct UserMessage {
    pub id: MessageId,
    pub op: MessageOp,
    pub text: String,
    /// Attribution for `Say` emissions; `None` for plain user messages.
    pub from: Option<Attribution>,
}

/// One item from the HTTP surface to the mind's scheduler.
#[derive(Debug, Clone)]
pub enum InboxItem {
    /// A journaled user message (`message`, `steer`, or `interrupt` carrying
    /// text — "interrupt & send").
    Message(UserMessage),
    /// A bare interrupt (no text): cancel the open turn. Nothing is journaled
    /// for it — the `MindState` transition records the interrupt itself.
    Interrupt,
}

/// An atomic snapshot + live subscription over one wave: the thread and mind
/// state as of one instant, plus receivers that carry exactly the frames sent
/// after it (see [`WaveRuntime::subscribe_with_snapshot`]).
#[derive(Debug)]
pub struct Subscription {
    pub turns: Vec<ChatTurn>,
    pub turn_rx: broadcast::Receiver<ChatTurn>,
    pub state: MindState,
    pub state_rx: broadcast::Receiver<MindState>,
}

/// Everything that must stay mutually consistent: the journal (truth), the
/// thread cache (fold of it), the open-turn snapshot, the mind state (last
/// transition), and the vendor thread id (last `ThreadStarted`).
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
    thread_id: Option<String>,
    /// Dispatched workers, folded from `WorkerDispatched`/`WorkerFinished`
    /// observations (the lfd tail). Keyed on run id — the idempotence guard:
    /// a run dispatches once and finishes once, however many times the
    /// observer sees it (live event + reconnect snapshot).
    workers: Vec<WorkerRecord>,
    /// Durable scheduler queue folded from the journal on boot.
    pending_messages: Vec<PendingMessage>,
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
    /// Fans mind-state transitions out to live SSE subscribers (the composer
    /// keys its verb off this).
    state_tx: broadcast::Sender<MindState>,
    /// Durable shared brain (read-only here; the mind curates it deliberately).
    memory: Memory,
    /// In-process user-message inbox (a channel, not a file).
    inbox_tx: mpsc::UnboundedSender<InboxItem>,
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
    ) -> anyhow::Result<(Arc<Self>, mpsc::UnboundedReceiver<InboxItem>)> {
        let (mut journal, events) = Journal::open(&journal_path(&repo_root, &name))?;
        let mut fold = fold_thread(&events);
        let workers = fold_workers(&events);

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
        let (state_tx, _) = broadcast::channel(STATE_BROADCAST_CAPACITY);
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
                thread_id: fold.thread_id,
                workers,
                pending_messages: fold.pending_messages,
            }),
            turn_tx,
            state_tx,
            memory,
            inbox_tx,
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

    /// The last journaled vendor thread id, if any — the mind's resume handle.
    pub fn last_thread_id(&self) -> Option<String> {
        self.inner().thread_id.clone()
    }

    /// User messages journaled before a restart but not yet consumed by a
    /// turn. The mind drains this once at boot before listening to the live
    /// inbox.
    pub fn pending_messages(&self) -> Vec<UserMessage> {
        self.inner()
            .pending_messages
            .iter()
            .map(|message| UserMessage {
                id: message.id.clone(),
                op: message.op,
                text: message.text.clone(),
                from: message.from.clone(),
            })
            .collect()
    }

    // -- Worker observations (the lfd tail's write surface) --
    //
    // These are OBSERVATIONS, not commands: the server tails lfd's event
    // stream and records confirmed facts. Both appends are idempotent keyed
    // on run id, so a live event plus a reconnect snapshot never journals a
    // worker twice.

    /// Journal a `WorkerDispatched` observation. Returns false (and appends
    /// nothing) when the run is already known.
    pub fn journal_worker_dispatched(
        &self,
        run_id: &str,
        session_id: &str,
        flow: &str,
        task: &str,
    ) -> bool {
        let mut inner = self.inner();
        if inner.workers.iter().any(|w| w.run_id == run_id) {
            return false;
        }
        inner.journal.append(|_| EventKind::WorkerDispatched {
            run_id: run_id.to_string(),
            session_id: session_id.to_string(),
            flow: flow.to_string(),
            task: task.to_string(),
        });
        inner.workers.push(WorkerRecord {
            run_id: run_id.to_string(),
            session_id: session_id.to_string(),
            flow: flow.to_string(),
            task: task.to_string(),
            finished: None,
        });
        true
    }

    /// Journal a `WorkerFinished` observation. Returns false (and appends
    /// nothing) when the run was never dispatched or already finished.
    pub fn journal_worker_finished(
        &self,
        run_id: &str,
        outcome: WorkerOutcome,
        summary: &str,
    ) -> bool {
        let mut inner = self.inner();
        let Some(pos) = inner
            .workers
            .iter()
            .position(|w| w.run_id == run_id && w.finished.is_none())
        else {
            return false;
        };
        inner.workers[pos].finished = Some(outcome);
        inner.journal.append(|_| EventKind::WorkerFinished {
            run_id: run_id.to_string(),
            outcome,
            summary: summary.to_string(),
        });
        true
    }

    /// Whether a `WorkerDispatched` is already journaled for `run_id` —
    /// the observer checks before fetching run details it won't need.
    pub fn worker_known(&self, run_id: &str) -> bool {
        self.inner().workers.iter().any(|w| w.run_id == run_id)
    }

    /// Workers dispatched and not yet finished — folded into the mind's
    /// heartbeat seed as the `<in_flight>` section.
    pub fn in_flight_workers(&self) -> Vec<WorkerRecord> {
        self.inner()
            .workers
            .iter()
            .filter(|w| w.finished.is_none())
            .cloned()
            .collect()
    }

    // -- Memory (the server holds MEMORY.md's pen) --
    //
    // Both writes go to the ORIGIN repo's wave/<name>/MEMORY.md (the runtime
    // opens against the main repo root — the file seeds read) and journal
    // `MemoryUpdated` under the same lock as every other append, so the
    // journal order and the file's history agree. Nothing else writes the
    // file while a server is live.

    /// Replace MEMORY.md wholesale and journal `MemoryUpdated {summary}`.
    ///
    /// # Errors
    /// File I/O only; the journal append is best-effort like every append.
    pub fn update_memory(&self, content: &str, summary: &str) -> std::io::Result<()> {
        let mut inner = self.inner();
        self.memory.write(content)?;
        inner.journal.append(|_| EventKind::MemoryUpdated {
            summary: summary.to_string(),
        });
        Ok(())
    }

    /// Append one curated fact as a Markdown bullet and journal it.
    ///
    /// # Errors
    /// File I/O only.
    pub fn append_memory(&self, fact: &str, summary: &str) -> std::io::Result<()> {
        let mut inner = self.inner();
        let mut content = self.memory.read();
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&format!("- {fact}\n"));
        self.memory.write(&content)?;
        inner.journal.append(|_| EventKind::MemoryUpdated {
            summary: summary.to_string(),
        });
        Ok(())
    }

    /// Journal the vendor thread the mind runs on. The borrowed-handle rule:
    /// this is the mind's first durable act, appended before its first turn.
    pub fn journal_thread_started(&self, vendor: &str, thread_id: &str) {
        let mut inner = self.inner();
        inner.journal.append(|_| EventKind::ThreadStarted {
            vendor: vendor.to_string(),
            thread_id: thread_id.to_string(),
        });
        inner.thread_id = Some(thread_id.to_string());
    }

    /// Atomically snapshot the thread (including the open turn) and the mind
    /// state, and subscribe to live frames for both. Every broadcast happens
    /// under the same lock as the append it reflects, so the receiver sees
    /// exactly the frames sent after this snapshot — no gap, no overlap, no
    /// frame older than the snapshot. A live frame's id may match a snapshot
    /// turn: it is that turn, newer; consumers replace by id.
    pub fn subscribe_with_snapshot(&self) -> Subscription {
        let inner = self.inner();
        Subscription {
            turns: snapshot_locked(&inner),
            turn_rx: self.turn_tx.subscribe(),
            state: inner.state.clone(),
            state_rx: self.state_tx.subscribe(),
        }
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
            to: to.clone(),
            reason: reason.to_string(),
        });
        // A send error just means no live subscribers.
        let _ = self.state_tx.send(to);
        true
    }

    /// `Turning → Interrupting` for the open turn (a user interrupt landed).
    /// Returns whether the transition applied — false when no turn is live.
    pub fn begin_interrupt(&self, reason: &str) -> bool {
        let mut inner = self.inner();
        let MindState::Turning { turn_id } = inner.state.clone() else {
            return false;
        };
        self.transition_locked(&mut inner, MindState::Interrupting { turn_id }, reason)
    }

    /// Journal that the open turn consumed steered messages mid-flight
    /// (`TurnSteered.answers` — see [`crate::lfd::wave::journal`]). Returns
    /// whether the marker was appended — false when no turn is turning, in
    /// which case the caller should have queued instead of steered.
    /// Journal steer consumption for `answers`. Normally the live turn
    /// consumed the message; when the turn closed between the harness
    /// accepting the input and this call (the send/journal race), consumption
    /// is journaled against the just-closed turn — the vendor heard the text
    /// either way, and an unmarked message would stay pending forever and be
    /// re-sent as a fresh turn on every restart. Returns `false` only when
    /// there is no turn at all to consume against (nothing journaled — the
    /// caller keeps the message queued).
    pub fn journal_steered(&self, answers: Vec<MessageId>) -> bool {
        let mut inner = self.inner();
        let turn_id = match inner.state.clone() {
            MindState::Turning { turn_id } | MindState::Interrupting { turn_id } => turn_id,
            _ => match inner.thread.last() {
                Some(turn) => turn.id.clone(),
                None => return false,
            },
        };
        inner
            .journal
            .append(|_| EventKind::TurnSteered { turn_id, answers });
        true
    }

    /// Janitor: finalize the open turn without a harness terminal event (the
    /// interrupt deadline expired — the harness never delivered
    /// `TurnCompleted`). Journals `TurnFinished`, commits and broadcasts the
    /// turn as accumulated so far, and settles the mind to `Idle`. Returns
    /// whether there was an open turn to finalize.
    pub fn force_finalize_open_turn(&self, status: Lifecycle, reason: &str) -> bool {
        let mut inner = self.inner();
        let Some(mut turn) = inner.open_turn.take() else {
            return false;
        };
        inner.journal.append(|_| EventKind::TurnFinished {
            turn_id: turn.id.clone(),
            status,
            usage: Usage::empty(),
        });
        turn.status = status;
        self.transition_locked(&mut inner, MindState::Idle, reason);
        self.commit_locked(&mut inner, turn);
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

    /// Deliver a user message: append its `UserMessage` event (recording the
    /// op — intent), commit the user turn (id from the event's seq), and hand
    /// it to the inbox for the mind's scheduler. Returns the stored user turn
    /// so the HTTP handler can echo it.
    pub fn deliver_user_message(&self, text: String, op: MessageOp) -> ChatTurn {
        self.deliver_message(text, op, None)
    }

    /// Deliver an attributed emission (`lf chat` — a worker report, child-wave
    /// escalation, or CLI FYI). Same journal row, thread commit, and inbox
    /// path as any user message; the byline rides along.
    pub fn deliver_say(&self, text: String, from: Attribution) -> ChatTurn {
        self.deliver_message(text, MessageOp::Say, Some(from))
    }

    fn deliver_message(&self, text: String, op: MessageOp, from: Option<Attribution>) -> ChatTurn {
        let (turn, id) = {
            let mut inner = self.inner();
            let event = inner.journal.append(|seq| EventKind::UserMessage {
                id: MessageId(format!("msg-{seq}")),
                op,
                text: text.clone(),
                from: from.clone(),
            });
            let id = MessageId(format!("msg-{}", event.seq));
            let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text.clone());
            turn.created_at = event.at_rfc3339();
            turn.from = from.as_ref().map(|from| from.label.clone());
            (self.commit_locked(&mut inner, turn), id)
        };
        // Unbounded inbox: delivering a message never blocks the mind.
        let _ = self
            .inbox_tx
            .send(InboxItem::Message(UserMessage { id, op, text, from }));
        turn
    }

    /// Deliver a bare interrupt (no text). Nothing is journaled here — the
    /// mind journals the `MindState` transition when it fires the cancel; an
    /// interrupt while idle is a no-op by design.
    pub fn deliver_interrupt(&self) {
        let _ = self.inbox_tx.send(InboxItem::Interrupt);
    }

    /// Record an already-finalized turn as its full event triple
    /// (`TurnStarted` + `TurnItem`s + `TurnFinished`) and commit it. Text
    /// becomes a `Message` item so the fold reproduces it. Does not touch the
    /// mind state — this is for instantaneous turns (injected narration), not
    /// mind turns.
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

    // -- TurnSink internals (same lock discipline as everything above) --
    //
    // Every content delta (opened / text / item) re-broadcasts the open-turn
    // snapshot under the same id. No debounce: deltas are item-granular from
    // the vendor stream (one per completed item, not per token), so the
    // natural rate is well under any flood threshold, and a suppressed
    // trailing frame would leave subscribers stale through a long tool call.
    // A throttle earns its place with the part-grained wire, not before.

    fn sink_turn_started(&self, answers: Vec<MessageId>) -> Event {
        let mut inner = self.inner();
        let event = inner.journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers,
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
            from: None,
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
        // Grow the open-turn snapshot through the one shared rule
        // (`ChatTurn::absorb_item` — the same call the journal fold makes)
        // and re-broadcast it so live subscribers watch the turn in progress.
        let Some(open) = inner.open_turn.as_mut() else {
            return;
        };
        open.absorb_item(item);
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

/// Folds the mind's [`TurnDelta`]s into journal events and committed turns:
/// `Opened` → `TurnStarted` (+ `Idle → Turning`, claiming any expected
/// answers), text/items → `TurnItem`s, usage accrues, `Finished` →
/// `TurnFinished` (+ `Turning → Idle`) and the turn commits to the thread.
/// One sink spans the mind's whole life; it resets itself at each `Finished`.
#[derive(Debug)]
pub struct TurnSink {
    runtime: Arc<WaveRuntime>,
    open: Option<OpenTurn>,
    /// The queued `MessageId`s the next `Opened` claims as
    /// `TurnStarted.answers` — the consumption marker. Set by the scheduler
    /// before it sends a turn's input; taken when the turn opens.
    pending_answers: Vec<MessageId>,
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
            pending_answers: Vec::new(),
        }
    }

    /// Declare which queued messages the next turn answers. The next `Opened`
    /// delta journals them in its `TurnStarted.answers`.
    pub fn expect_answers(&mut self, answers: Vec<MessageId>) {
        self.pending_answers = answers;
    }

    /// Drop the open-turn record without finalizing it — the runtime janitor
    /// already journaled the terminal event (interrupt deadline force-path).
    /// A late harness terminal for that turn is then ignored instead of
    /// journaled twice.
    pub fn abandon_open(&mut self) {
        self.open = None;
    }

    pub fn on_delta(&mut self, delta: TurnDelta) {
        match delta {
            TurnDelta::Opened => {
                let answers = std::mem::take(&mut self.pending_answers);
                let event = self.runtime.sink_turn_started(answers);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::conversations::types::{ConversationEvent, Lifecycle, TurnUsage};
    use crate::lfd::wave::mind::EventAdapter;

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
            from: None,
        }
    }

    fn open_runtime(
        repo: &std::path::Path,
    ) -> (Arc<WaveRuntime>, mpsc::UnboundedReceiver<InboxItem>) {
        WaveRuntime::open("ship".into(), repo.to_path_buf()).expect("open runtime")
    }

    /// Feed one harness event through the production pipeline
    /// (adapter → deltas → sink).
    fn feed(adapter: &mut EventAdapter, sink: &mut TurnSink, event: ConversationEvent) {
        for delta in adapter.feed(&event) {
            sink.on_delta(delta);
        }
    }

    fn ev_started() -> ConversationEvent {
        ConversationEvent::TurnStarted {
            turn_id: "vendor-turn".into(),
        }
    }

    fn ev_text(text: &str) -> ConversationEvent {
        ConversationEvent::ItemCompleted {
            turn_id: "vendor-turn".into(),
            item: ConversationItem::Message {
                id: "item".into(),
                text: text.into(),
                phase: None,
            },
        }
    }

    fn ev_tool() -> ConversationEvent {
        ConversationEvent::ItemCompleted {
            turn_id: "vendor-turn".into(),
            item: ConversationItem::Tool {
                id: "item-tool".into(),
                name: "Bash".into(),
                status: Lifecycle::Completed,
                input: None,
                output: Some("cargo test".into()),
            },
        }
    }

    fn ev_completed(status: Lifecycle) -> ConversationEvent {
        ConversationEvent::TurnCompleted {
            turn_id: "vendor-turn".into(),
            status,
        }
    }

    fn ev_usage(input: u64, output: u64) -> ConversationEvent {
        ConversationEvent::TurnUsage {
            turn_id: "vendor-turn".into(),
            usage: TurnUsage {
                input_tokens: input,
                output_tokens: output,
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: None,
                cost_usd: Some(0.02),
            },
        }
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
        let turn = rt.deliver_user_message("how goes it?".into(), MessageOp::Message);
        assert_eq!(turn.role, ChatRole::User);
        assert_eq!(turn.text, "how goes it?");
        // The message landed in the inbox for the mind, id tied to its event.
        let InboxItem::Message(msg) = rx.try_recv().expect("inbox message") else {
            panic!("expected a message inbox item");
        };
        assert_eq!(msg.text, "how goes it?");
        assert_eq!(msg.op, MessageOp::Message);
        assert_eq!(msg.id, MessageId(format!("msg-{}", turn_seq(&turn.id))));
    }

    #[test]
    fn deliver_say_journals_attribution_and_queues_for_the_mind() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, mut rx) = open_runtime(tmp.path());
        let from = Attribution {
            session_id: Some("sess-9".into()),
            label: "worker".into(),
        };
        let turn = rt.deliver_say("PR landed; one surprise in the fold".into(), from.clone());
        assert_eq!(turn.role, ChatRole::User);
        assert_eq!(turn.from.as_deref(), Some("worker"));

        // Inbox: an attributed Say message the mind reacts to like any input.
        let InboxItem::Message(msg) = rx.try_recv().expect("inbox item") else {
            panic!("expected a message inbox item");
        };
        assert_eq!(msg.op, MessageOp::Say);
        assert_eq!(msg.from, Some(from.clone()));

        // Journal: the UserMessage row carries the attribution, and a
        // restarted runtime folds it back into the pending queue.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let EventKind::UserMessage {
            op, from: stored, ..
        } = &events[0].kind
        else {
            panic!("expected UserMessage");
        };
        assert_eq!(*op, MessageOp::Say);
        assert_eq!(stored.as_ref(), Some(&from));
        let (rt2, _rx2) = open_runtime(tmp.path());
        let pending = rt2.pending_messages();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].from, Some(from));
        assert_eq!(rt2.thread_snapshot()[0].from.as_deref(), Some("worker"));
    }

    #[test]
    fn update_memory_writes_the_origin_file_and_journals() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        rt.update_memory("# Ship\n\n- fold is truth\n", "fold is truth")
            .expect("update");
        assert_eq!(rt.memory().read(), "# Ship\n\n- fold is truth\n");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("wave/ship/MEMORY.md")).expect("origin file"),
            "# Ship\n\n- fold is truth\n",
            "the ORIGIN repo's file is the one written"
        );

        rt.append_memory("bullets append", "bullets append")
            .expect("append");
        assert_eq!(
            rt.memory().read(),
            "# Ship\n\n- fold is truth\n- bullets append\n"
        );

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let summaries: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::MemoryUpdated { summary } => Some(summary.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(summaries, vec!["fold is truth", "bullets append"]);
    }

    #[test]
    fn deliver_interrupt_is_a_control_item_not_a_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, mut rx) = open_runtime(tmp.path());
        rt.deliver_interrupt();
        assert!(matches!(
            rx.try_recv().expect("inbox item"),
            InboxItem::Interrupt
        ));
        // Nothing journaled, nothing in the thread.
        assert!(rt.thread_snapshot().is_empty());
    }

    #[test]
    fn thread_id_round_trips_through_the_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let (rt, _rx) = open_runtime(tmp.path());
            assert_eq!(rt.last_thread_id(), None);
            rt.journal_thread_started("codex", "thread-abc");
            assert_eq!(rt.last_thread_id().as_deref(), Some("thread-abc"));
        }
        // A restarted runtime folds the resume handle back out of the log.
        let (rt, _rx) = open_runtime(tmp.path());
        assert_eq!(rt.last_thread_id().as_deref(), Some("thread-abc"));
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
    fn turn_sink_journals_a_harness_turn_and_commits_it() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        let mut sink = TurnSink::new(rt.clone());
        let mut adapter = EventAdapter::new();

        feed(&mut adapter, &mut sink, ev_started());
        assert_eq!(
            rt.mind_state().name(),
            "turning",
            "mid-turn the mind is Turning"
        );
        feed(&mut adapter, &mut sink, ev_text("hello"));
        feed(&mut adapter, &mut sink, ev_tool());
        feed(&mut adapter, &mut sink, ev_completed(Lifecycle::Completed));
        feed(&mut adapter, &mut sink, ev_usage(10, 4));

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
    fn turn_started_claims_the_expected_answers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        let mut sink = TurnSink::new(rt.clone());
        let mut adapter = EventAdapter::new();

        sink.expect_answers(vec![MessageId("msg-1".into()), MessageId("msg-2".into())]);
        feed(&mut adapter, &mut sink, ev_started());
        feed(&mut adapter, &mut sink, ev_completed(Lifecycle::Completed));
        feed(&mut adapter, &mut sink, ev_usage(1, 1));

        // The journal's TurnStarted carries the consumption marker; a second
        // turn without expect_answers claims nothing.
        feed(&mut adapter, &mut sink, ev_started());
        feed(&mut adapter, &mut sink, ev_completed(Lifecycle::Completed));
        feed(&mut adapter, &mut sink, ev_usage(1, 1));

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen journal");
        let answers: Vec<Vec<MessageId>> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::TurnStarted { answers, .. } => Some(answers.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(answers.len(), 2);
        assert_eq!(
            answers[0],
            vec![MessageId("msg-1".into()), MessageId("msg-2".into())]
        );
        assert!(answers[1].is_empty());
    }

    #[test]
    fn open_turn_streams_growing_snapshots_then_the_terminal_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        let sub = rt.subscribe_with_snapshot();
        assert!(sub.turns.is_empty());
        assert_eq!(sub.state, MindState::Idle);
        let mut frames = sub.turn_rx;
        let mut states = sub.state_rx;

        let mut sink = TurnSink::new(rt.clone());
        let mut adapter = EventAdapter::new();

        // The turn opens empty and running, then the text lands in a second
        // frame under the same id.
        feed(&mut adapter, &mut sink, ev_started());
        let opened = frames.try_recv().expect("opened frame");
        assert_eq!(opened.status, Lifecycle::Running);
        assert_eq!(opened.text, "");
        feed(&mut adapter, &mut sink, ev_text("thinking"));
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
        feed(&mut adapter, &mut sink, ev_tool());
        let with_item = frames.try_recv().expect("item frame");
        assert_eq!(with_item.id, opened.id);
        assert_eq!(with_item.items.len(), 1);
        assert_eq!(with_item.text, "thinking");

        // Finalization replaces the running turn under the same id.
        feed(&mut adapter, &mut sink, ev_completed(Lifecycle::Completed));
        feed(&mut adapter, &mut sink, ev_usage(10, 5));
        let terminal = frames.try_recv().expect("terminal frame");
        assert_eq!(terminal.id, opened.id);
        assert_eq!(terminal.status, Lifecycle::Completed);

        // No stale running turn remains anywhere.
        let after = rt.thread_snapshot();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].status, Lifecycle::Completed);
        assert!(frames.try_recv().is_err(), "no extra frames");

        // Every transition was broadcast: Idle → Turning → Idle.
        assert!(matches!(
            states.try_recv().expect("turning state frame"),
            MindState::Turning { .. }
        ));
        assert_eq!(
            states.try_recv().expect("idle state frame"),
            MindState::Idle
        );
        assert!(states.try_recv().is_err(), "no extra state frames");
    }

    #[test]
    fn force_finalize_open_turn_closes_journal_and_settles_idle() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        let mut sink = TurnSink::new(rt.clone());
        let mut adapter = EventAdapter::new();
        feed(&mut adapter, &mut sink, ev_started());
        feed(&mut adapter, &mut sink, ev_text("half"));
        assert!(rt.begin_interrupt("user interrupt"));
        assert_eq!(rt.mind_state().name(), "interrupting");

        assert!(rt.force_finalize_open_turn(Lifecycle::Interrupted, "deadline"));
        assert_eq!(rt.mind_state(), MindState::Idle);
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].status, Lifecycle::Interrupted);
        assert_eq!(thread[0].text, "half");

        // The journal is closed: a replay agrees, no open turn survives.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let fold = crate::lfd::wave::journal::fold_thread(&events);
        assert!(fold.open.is_empty());
        assert_eq!(fold.turns.last().unwrap().status, Lifecycle::Interrupted);

        // Nothing left to force a second time.
        assert!(!rt.force_finalize_open_turn(Lifecycle::Interrupted, "again"));
    }

    #[test]
    fn worker_observations_are_idempotent_and_survive_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let (rt, _rx) = open_runtime(tmp.path());
            assert!(rt.journal_worker_dispatched("run-1", "sess-1", "implement", "wire it"));
            // Same run seen again (reconnect snapshot): guarded, not journaled.
            assert!(!rt.journal_worker_dispatched("run-1", "sess-1", "implement", "wire it"));
            assert_eq!(rt.in_flight_workers().len(), 1);

            // A finish for a run never dispatched is refused.
            assert!(!rt.journal_worker_finished("run-9", WorkerOutcome::Failed, "?"));
            assert!(rt.journal_worker_finished("run-1", WorkerOutcome::Completed, "pr landed"));
            assert!(!rt.journal_worker_finished(
                "run-1",
                WorkerOutcome::Completed,
                "pr landed again"
            ));
            assert!(rt.in_flight_workers().is_empty());
        }

        // A restarted runtime folds the same guard state back out of the log:
        // the finished run stays finished, a new run dispatches normally.
        let (rt, _rx) = open_runtime(tmp.path());
        assert!(!rt.journal_worker_dispatched("run-1", "sess-1", "implement", "wire it"));
        assert!(rt.journal_worker_dispatched("run-2", "sess-2", "design", "sketch it"));
        assert_eq!(rt.in_flight_workers().len(), 1);
        assert_eq!(rt.in_flight_workers()[0].run_id, "run-2");

        // Exactly one WorkerDispatched per run in the journal itself.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let dispatched: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::WorkerDispatched { run_id, .. } => Some(run_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(dispatched, vec!["run-1", "run-2"]);
        let finished: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::WorkerFinished { run_id, .. } => Some(run_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(finished, vec!["run-1"]);
    }

    #[test]
    fn journal_steered_consumes_against_live_or_just_closed_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (rt, _rx) = open_runtime(tmp.path());
        assert!(
            !rt.journal_steered(vec![MessageId("msg-0".into())]),
            "no turn anywhere: nothing to consume against"
        );

        // Two real queued messages, ids from the journal fold.
        rt.deliver_user_message("steer me".into(), MessageOp::Message);
        rt.deliver_user_message("me too".into(), MessageOp::Message);
        let pending_ids: Vec<MessageId> = {
            let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
            fold_thread(&events)
                .pending_messages
                .into_iter()
                .map(|pending| pending.id)
                .collect()
        };
        assert_eq!(pending_ids.len(), 2);

        // First consumed mid-turn, the normal steer path.
        let mut sink = TurnSink::new(rt.clone());
        let mut adapter = EventAdapter::new();
        feed(&mut adapter, &mut sink, ev_started());
        assert!(rt.journal_steered(vec![pending_ids[0].clone()]));
        feed(&mut adapter, &mut sink, ev_completed(Lifecycle::Completed));
        feed(&mut adapter, &mut sink, ev_usage(1, 1));

        // The send/journal boundary race: the turn closed between the harness
        // accepting the input and the consumption write. The marker must
        // still land (against the just-closed turn) or the message stays
        // pending forever and is re-sent on every restart.
        assert!(
            rt.journal_steered(vec![pending_ids[1].clone()]),
            "boundary race consumes against the just-closed turn"
        );

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let assistant_turn = rt
            .thread_snapshot()
            .iter()
            .find(|turn| turn.role == ChatRole::Assistant)
            .map(|turn| turn.id.clone())
            .expect("assistant turn");
        let steered: Vec<_> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::TurnSteered { turn_id, answers } => {
                    Some((turn_id.clone(), answers.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            steered,
            vec![
                (assistant_turn.clone(), vec![pending_ids[0].clone()]),
                (assistant_turn, vec![pending_ids[1].clone()]),
            ],
            "both markers name the turn that heard the text"
        );

        // And the fold agrees: neither message re-sends after a restart.
        let fold = fold_thread(&events);
        assert!(
            fold.pending_messages.is_empty(),
            "consumed messages never re-send: {:?}",
            fold.pending_messages
        );
    }
}
