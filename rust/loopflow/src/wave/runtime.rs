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
//! - the loop state is the last `LoopState` event;
//! - the SSE broadcast is liveness only — a subscriber that lags resyncs from
//!   the store.
//!
//! Two independent inputs feed the journal: the resident's wire deltas
//! ([`WaveRuntime::apply_resident_delta`] — the old in-process `TurnSink`
//! vocabulary, now arriving over `POST /resident/deltas`) and user messages
//! (HTTP → journal + inbox broadcast). All appends go through one lock, so
//! journal order, cache order, and broadcast order agree — one writer appends
//! and broadcasts. This module is vendor-free: the harness lives with the
//! resident process, never here.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;

use crate::chat::turns::{ChatRole, ChatTurn, TurnDelta};
use crate::chat::types::{ConversationItem, Lifecycle};
use crate::engine::wave_config::read_wave_config;
use crate::project_session::ProjectObservation;
use crate::receipt::Receipt;
use crate::security::sanitize_fs_component;
use crate::task::TaskObservation;
use crate::wave::channel::matches_prefix;
use crate::wave::journal::{
    fold_thread, journal_path, project_observation_message, restore_pending,
    task_observation_message, EventKind, Journal, MessageId, MessageOp, PendingMessage, Usage,
};
use crate::wave::memory::Memory;
use crate::wave::playhead::{
    now_rfc3339, BodyProvenance, Playhead, PlayheadEvent, PlayheadView, QueuedInvocation,
    StepOutcome,
};
use crate::wave::state::{can_transition, LoopState};
use crate::wave::wire::{ProviderSessionRef, ResidentDelta, ResidentStateTo};

/// Capacity of the live turn broadcast. SSE clients that fall this far behind
/// get a lag error and resync from `/conversation`; the journal is the source
/// of truth, so a dropped live turn is never lost.
const TURN_BROADCAST_CAPACITY: usize = 256;

/// Capacity of the live loop-state broadcast. Transitions are rare (a few per
/// turn); a lagged subscriber just resyncs from the next transition.
const STATE_BROADCAST_CAPACITY: usize = 64;

/// Capacity of playhead snapshots. Every cursor mutation is durable; a lagged
/// client reconnects and receives the current snapshot before live frames.
const PLAYHEAD_BROADCAST_CAPACITY: usize = 64;

/// Capacity of the live memory broadcast. Curation is deliberate and rare;
/// a lagged subscriber reads MEMORY.md itself.
const MEMORY_BROADCAST_CAPACITY: usize = 64;

/// Capacity of the live inbox broadcast (resident-directed ops → the
/// `/events?inbox=true` frames and the supervisor). The journal is the
/// durable queue; a lagged subscriber resyncs from the pending replay.
const INBOX_BROADCAST_CAPACITY: usize = 256;

/// How a channel name relates to a wave's family, per [`channel_role`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelRole {
    /// The wave's own channel (raw or sanitized spelling of its name).
    Primary,
    /// A work line: a dot-descendant of the wave's channel name.
    Child,
}

/// A wave's primary channel name: the sanitized filesystem form of its name.
/// Worktree basenames — and therefore child channel names — derive from it
/// (`web/ui` mints `web-ui` worktrees and `web-ui.<run>` channels).
pub fn wave_channel_name(wave: &str) -> String {
    sanitize_fs_component(wave)
}

/// THE family-membership predicate: how `channel` relates to `wave`, or
/// `None` when it is outside the family. The message door, `/events` scoping,
/// and the ambient dot-split (`engine::wave_context`) all route through it,
/// so every consumer agrees on what the family is called. Membership compares
/// against the SANITIZED wave name ([`wave_channel_name`]) — the form channel
/// names actually carry — while the raw spelling still addresses the primary.
pub fn channel_role(wave: &str, channel: &str) -> Option<ChannelRole> {
    let family = wave_channel_name(wave);
    if channel == wave || channel == family {
        return Some(ChannelRole::Primary);
    }
    matches_prefix(channel, &family).then_some(ChannelRole::Child)
}

/// One whole-turn frame: the turn plus its wire JSON, serialized ONCE at the
/// send site so N subscribers share one serialization instead of performing N.
/// Sent at the events that are naturally O(1) per turn — a turn opening, a
/// body-session update, and finalization — where the whole turn is small or the
/// authoritative re-baseline is worth the bytes. In-turn growth rides
/// [`TurnDeltaFrame`] instead.
#[derive(Debug)]
pub struct TurnFrame {
    pub turn: ChatTurn,
    /// The turn as `/events` `turn`-frame JSON.
    pub json: String,
}

impl TurnFrame {
    fn share(turn: ChatTurn) -> Arc<Self> {
        let json = serde_json::to_string(&turn).expect("ChatTurn serializes to JSON");
        Arc::new(Self { turn, json })
    }
}

/// One live-turn increment: a [`TurnDelta`] plus its wire JSON, serialized ONCE
/// at the send site (the same Arc-share as [`TurnFrame`]). Broadcast on every
/// non-finalizing content delta so the wire carries O(fragment) per token
/// instead of the whole accumulated turn.
#[derive(Debug)]
pub struct TurnDeltaFrame {
    pub delta: TurnDelta,
    /// The increment as `/events` `turn-delta`-frame JSON.
    pub json: String,
}

impl TurnDeltaFrame {
    fn share(turn_id: String, item: ConversationItem) -> Arc<Self> {
        let delta = TurnDelta { turn_id, item };
        let json = serde_json::to_string(&delta).expect("TurnDelta serializes to JSON");
        Arc::new(Self { delta, json })
    }
}

/// What the live turn broadcast carries: either a whole turn (the client
/// replaces by id) or one increment to an open turn (the client absorbs by id).
/// One broadcast channel carries both so their order — and the lag counter that
/// guards reconstruction — stays single.
#[derive(Debug, Clone)]
pub enum TurnBroadcast {
    Whole(Arc<TurnFrame>),
    Delta(Arc<TurnDeltaFrame>),
}

#[cfg(test)]
impl TurnBroadcast {
    fn expect_whole(&self) -> &ChatTurn {
        match self {
            Self::Whole(frame) => &frame.turn,
            Self::Delta(_) => panic!("expected a whole-turn frame, got a delta"),
        }
    }

    fn expect_delta(&self) -> &TurnDelta {
        match self {
            Self::Delta(frame) => &frame.delta,
            Self::Whole(_) => panic!("expected a delta frame, got a whole turn"),
        }
    }
}

/// One resident-directed op, broadcast live to the resident's subscription
/// (`inbox` SSE frames) and the supervisor.
#[derive(Debug, Clone)]
pub enum InboxItem {
    /// A journaled user message (`message`, `steer`, `say`, or `interrupt`
    /// carrying text — "interrupt & send"), awaiting consumption (named in a
    /// `TurnStarted.answers` or `TurnSteered.answers`).
    Message(PendingMessage),
    /// A typed Task ledger observation awaiting the same durable turn
    /// consumption acknowledgement as a queued message.
    Task(TaskObservation),
    /// A typed Project ledger observation.
    Project(ProjectObservation),
    /// A bare interrupt (no text): cancel the open turn. Nothing is journaled
    /// for it — the `LoopState` transition records the interrupt itself.
    Interrupt,
    /// Skip the selected logical step. The resident interrupts the body and
    /// reports a skipped playhead outcome instead of a retryable interruption.
    Skip,
}

/// An atomic snapshot + live subscription over one wave: the thread and loop
/// state as of one instant, plus receivers that carry exactly the frames sent
/// after it (see [`WaveRuntime::subscribe_with_snapshot`]).
#[derive(Debug)]
pub struct Subscription {
    pub turns: Vec<ChatTurn>,
    /// Live turn frames ride as [`TurnBroadcast`] (whole or delta), each an
    /// `Arc`: the broadcast clones once per subscriber, so N subscribers share
    /// one allocation — and one JSON serialization — per frame instead of N.
    pub turn_rx: broadcast::Receiver<TurnBroadcast>,
    pub state: LoopState,
    pub state_rx: broadcast::Receiver<LoopState>,
    pub playhead: Option<PlayheadView>,
    pub playhead_rx: broadcast::Receiver<PlayheadView>,
    /// Live `MemoryUpdated` summaries — fired on every curation, no replay
    /// (the file itself is the durable state).
    pub memory_rx: broadcast::Receiver<String>,
    /// Memory facts added this server life, replayed on
    /// subscribe before the live stream continues.
    pub memory_adds: Vec<String>,
    pub memory_add_rx: broadcast::Receiver<String>,
    /// The pending queue as of the snapshot: journaled user messages not yet
    /// named in any `answers` — the resident's boot replay.
    pub pending: Vec<PendingMessage>,
    pub tasks: HashMap<MessageId, TaskObservation>,
    pub projects: HashMap<MessageId, ProjectObservation>,
    /// Live resident-directed ops sent after the snapshot.
    pub inbox_rx: broadcast::Receiver<InboxItem>,
}

/// The assistant turn in progress. `turn` is the snapshot the wire watches
/// grow (status `Running`), re-broadcast on every content delta and committed
/// to `thread` under the same id at finalization; the rest is bookkeeping that
/// only means anything while the turn is open.
#[derive(Debug)]
struct OpenTurn {
    turn: ChatTurn,
    /// Usage accrued from this turn's `TurnUsage` deltas.
    usage: Usage,
    /// Prose fragments so far, for `Message` item ids (`"text-<n>"`).
    text_items: usize,
    /// Message ids this turn claimed (`TurnOpened.answers` plus any mid-turn
    /// `TurnSteered.answers`). Requeued if the turn ends without completing.
    claims: Vec<MessageId>,
}

/// Everything that must stay mutually consistent: the journal (truth), the
/// thread cache (fold of it), the open turn, the loop state (last transition).
#[derive(Debug)]
struct Inner {
    journal: Journal,
    thread: Vec<ChatTurn>,
    /// The turn currently in progress, or `None` between turns. Everything
    /// that is only meaningful while a turn runs lives inside it, so closing a
    /// turn is one `take()` rather than four fields reset in step.
    open: Option<OpenTurn>,
    /// Set by a force-finalize (interrupt-deadline janitor, resident death):
    /// the journal already closed the turn, so late wire deltas for it —
    /// including the resident's own eventual `TurnFinished` — are dropped
    /// until the next `TurnOpened`.
    drop_deltas_until_opened: bool,
    state: LoopState,
    playhead: Option<Playhead>,
    /// Id of the loop's current or most recently committed assistant turn —
    /// what `journal_steered` falls back to when the turn closed during the
    /// send (the thread's *last* turn at that point is usually the steer's
    /// own user turn, which must never be named as a consumer).
    last_assistant_turn_id: Option<String>,
    /// Durable scheduler queue folded from the journal on boot.
    pending_messages: Vec<PendingMessage>,
    /// Every journaled user message by id — requeues restore pending entries
    /// from it (an id alone can't rebuild the text/op/from).
    messages: HashMap<MessageId, PendingMessage>,
    tasks: HashMap<MessageId, TaskObservation>,
    projects: HashMap<MessageId, ProjectObservation>,
    /// Memory facts added since the last externalization. The compiled
    /// checkpoint lives in MEMORY.md; this is the replayable delta after it.
    memory_adds: Vec<String>,
}

/// The whole live state of one running wave server.
#[derive(Debug)]
pub struct WaveRuntime {
    name: String,
    /// The primary channel's name — the wave name sanitized to its
    /// filesystem form ([`wave_channel_name`]); child channels are its
    /// dot-descendants.
    channel_name: String,
    repo_root: PathBuf,
    /// Journal + materialized thread + loop state, behind one lock so their
    /// orders never diverge.
    inner: Mutex<Inner>,
    /// Fans turn frames out to live SSE subscribers: a whole frame when a turn
    /// opens / updates its body session / finalizes, and a delta frame for each
    /// in-turn content increment. Frames are `Arc`-shared so a delta costs one
    /// clone (and one serialization) total, not one per subscriber.
    turn_tx: broadcast::Sender<TurnBroadcast>,
    /// Fans loop-state transitions out to live SSE subscribers (the composer
    /// keys its verb off this).
    state_tx: broadcast::Sender<LoopState>,
    /// Fans the complete playhead view after every journaled transition.
    playhead_tx: broadcast::Sender<PlayheadView>,
    /// Fans `MemoryUpdated` summaries out to live SSE subscribers.
    memory_tx: broadcast::Sender<String>,
    /// Fans `MemoryAdded` facts out to live SSE subscribers.
    memory_add_tx: broadcast::Sender<String>,
    /// Durable shared brain (read-only here; the loop curates it deliberately).
    memory: Memory,
    /// Fans resident-directed ops out to the resident's `/events?inbox=true`
    /// subscription and the supervisor. Liveness only — the journal's pending
    /// fold is the durable queue.
    inbox_tx: broadcast::Sender<InboxItem>,
    /// Whether a resident has ever been spawned for / attached to this
    /// listener. `/health` serves `loop_state: null` until then (a dormant channel
    /// has no loop to report on).
    resident_expected: AtomicBool,
}

impl WaveRuntime {
    /// Open the runtime against the wave's journal, replaying it: the thread
    /// cache is rebuilt from the log and turn ids continue from its seq.
    ///
    /// Boot janitor: turns left open by a crash are finalized as `Failed`
    /// (appended to the journal, so the log itself is closed), the messages
    /// those turns had claimed are requeued (`MessagesRequeued` — a crashed
    /// turn never answered them), and a non-idle loop state settles back to
    /// `Idle`.
    ///
    /// # Errors
    /// Journal I/O failure or an unreadable (future-versioned) journal.
    pub fn open(name: String, repo_root: PathBuf) -> anyhow::Result<Arc<Self>> {
        let (mut journal, events) = Journal::open(&journal_path(&repo_root, &name))?;
        let mut fold = fold_thread(&events);

        // Janitor: an active body belonged to the dead server process. Keep
        // its logical step selected, close only the abandoned attempt, and
        // let the fresh resident retry it in a new body.
        const ABANDONED: &str = "startup janitor: body abandoned by server restart";
        let mut playhead = fold.playhead.take();
        if let Some(state) = playhead.as_mut() {
            if let Some(active) = state.active.clone() {
                let events =
                    state.finish_body(&active.body_id, StepOutcome::Interrupted, ABANDONED)?;
                for event in events {
                    journal.append(|_| EventKind::PlayheadChanged {
                        event,
                        playhead: Box::new(state.clone()),
                    });
                }
            }
        }

        // Janitor: a turn without a TurnFinished crashed with the server.
        for mut turn in fold.open {
            let finished = journal.append(|_| EventKind::TurnFinished {
                turn_id: turn.id.clone(),
                status: Lifecycle::Failed,
                usage: Usage::empty(),
                termination_reason: Some(ABANDONED.to_string()),
            });
            turn.status = Lifecycle::Failed;
            turn.close_body(finished.at_rfc3339(), Some(ABANDONED.to_string()));
            fold.turns.push(turn);
        }
        // Janitor: what the crashed turns claimed goes back in the queue —
        // they never answered it; the next resident replay re-delivers.
        let requeued = restore_pending(
            &mut fold.pending_messages,
            &fold.messages,
            &fold.open_claims,
        );
        if !requeued.is_empty() {
            journal.append(|_| EventKind::MessagesRequeued {
                ids: requeued.clone(),
            });
        }
        // Seed the steer-consumption fallback from the replayed thread.
        let last_assistant_turn_id = fold
            .turns
            .iter()
            .rev()
            .find(|turn| turn.role == ChatRole::Assistant)
            .map(|turn| turn.id.clone());
        // Janitor: no turn is live on a fresh boot, whatever the log says.
        let state = if fold.state == LoopState::Idle {
            LoopState::Idle
        } else {
            journal.append(|_| EventKind::LoopState {
                from: fold.state.clone(),
                to: LoopState::Idle,
                reason: "startup janitor: no live turn after restart".to_string(),
            });
            LoopState::Idle
        };

        let (turn_tx, _) = broadcast::channel(TURN_BROADCAST_CAPACITY);
        let (state_tx, _) = broadcast::channel(STATE_BROADCAST_CAPACITY);
        let (playhead_tx, _) = broadcast::channel(PLAYHEAD_BROADCAST_CAPACITY);
        let (memory_tx, _) = broadcast::channel(MEMORY_BROADCAST_CAPACITY);
        let (memory_add_tx, _) = broadcast::channel(MEMORY_BROADCAST_CAPACITY);
        let (inbox_tx, _) = broadcast::channel(INBOX_BROADCAST_CAPACITY);
        let memory = Memory::for_wave(&repo_root, &name);
        Ok(Arc::new(Self {
            channel_name: wave_channel_name(&name),
            name,
            repo_root,
            inner: Mutex::new(Inner {
                journal,
                thread: fold.turns,
                open: None,
                drop_deltas_until_opened: false,
                state,
                playhead,
                last_assistant_turn_id,
                pending_messages: fold.pending_messages,
                messages: fold.messages,
                tasks: fold.tasks,
                projects: fold.projects,
                memory_adds: fold.memory_adds,
            }),
            turn_tx,
            state_tx,
            playhead_tx,
            memory_tx,
            memory_add_tx,
            memory,
            inbox_tx,
            resident_expected: AtomicBool::new(false),
        }))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The primary channel's name (the wave name, sanitized — see
    /// [`wave_channel_name`]). Family scans and default `/events` scopes key
    /// off this, never the raw name.
    pub fn channel_name(&self) -> &str {
        &self.channel_name
    }

    pub fn repo_root(&self) -> &std::path::Path {
        &self.repo_root
    }

    /// Whether the wave is paused, from GOAL.md frontmatter (`paused: true`).
    /// File-first by design — the flag lives with the goal, re-read live, no
    /// restart; the registry row's `paused` column is not consulted. A paused
    /// wave keeps serving and queueing, but the listener refuses to start
    /// turns ([`WaveRuntime::apply_resident_delta`] drops `TurnOpened`).
    pub fn paused(&self) -> bool {
        read_wave_config(&self.repo_root, &self.name)
            .and_then(|config| config.paused)
            .unwrap_or(false)
    }

    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("wave runtime lock poisoned")
    }

    /// Snapshot of the whole thread — finalized turns plus the open turn
    /// (status `Running`), if one is in progress.
    pub fn thread_snapshot(&self) -> Vec<ChatTurn> {
        snapshot_tail_locked(&self.inner(), None)
    }

    /// The last `limit` turns (open turn included, newest last), cloned
    /// inside the lock — a `/conversation?limit=N` tail never clones the
    /// whole thread. `None` serves everything.
    pub fn thread_tail(&self, limit: Option<usize>) -> Vec<ChatTurn> {
        snapshot_tail_locked(&self.inner(), limit)
    }

    /// Thread length (open turn included) without cloning a single turn —
    /// `/health`'s counter.
    pub fn thread_len(&self) -> usize {
        let inner = self.inner();
        inner.thread.len() + usize::from(inner.open.is_some())
    }

    /// The newest provider thread recorded by the durable Wave journal.
    pub fn latest_provider_session(&self) -> Option<ProviderSessionRef> {
        let inner = self.inner();
        if let Some(body) = inner
            .open
            .as_ref()
            .and_then(|open| open.turn.body.as_ref())
            .filter(|body| body_has_harness(body))
        {
            return provider_session_from_body(body);
        }
        inner
            .thread
            .iter()
            .rev()
            .filter_map(|turn| turn.body.as_ref())
            .find(|body| body_has_harness(body))
            .and_then(provider_session_from_body)
    }

    /// Current loop state, for `/health` and the composer.
    pub fn loop_state(&self) -> LoopState {
        self.inner().state.clone()
    }

    /// Current durable playhead view. `None` exists only before the first
    /// resident attachment initializes the default `wave` invocation.
    pub fn playhead(&self) -> Option<PlayheadView> {
        self.inner().playhead.as_ref().map(Playhead::view)
    }

    /// Initialize the default wave invocation once. Replay wins over code:
    /// after restart the journaled cursor is reused even if definitions moved.
    pub fn ensure_playhead(&self) -> anyhow::Result<PlayheadView> {
        let mut inner = self.inner();
        if let Some(playhead) = inner.playhead.as_ref() {
            return Ok(playhead.view());
        }
        let root = QueuedInvocation::load(&self.repo_root, "wave")?;
        let (playhead, event) = Playhead::new(root);
        let view = playhead.view();
        inner.journal.append(|_| EventKind::PlayheadChanged {
            event,
            playhead: Box::new(playhead.clone()),
        });
        inner.playhead = Some(playhead);
        let _ = self.playhead_tx.send(view.clone());
        Ok(view)
    }

    /// Enqueue a flow at the innermost active invocation. The flow is
    /// resolved now, so the queue carries stable step names and paths.
    pub fn enqueue_flow(&self, flow: &str) -> anyhow::Result<PlayheadView> {
        self.ensure_playhead()?;
        let invocation = QueuedInvocation::load(&self.repo_root, flow)?;
        let mut inner = self.inner();
        let event = inner
            .playhead
            .as_mut()
            .expect("ensure_playhead initialized it")
            .enqueue(invocation)?;
        self.journal_playhead_locked(&mut inner, vec![event])
    }

    /// Open a body attempt for the selected logical step.
    pub fn start_body(&self, body: BodyProvenance) -> anyhow::Result<PlayheadView> {
        self.ensure_playhead()?;
        let mut inner = self.inner();
        let event = inner
            .playhead
            .as_mut()
            .expect("ensure_playhead initialized it")
            .start_body(body)?;
        self.journal_playhead_locked(&mut inner, vec![event])
    }

    /// Close one body attempt. Completed and skipped bodies advance; failed
    /// and interrupted bodies leave the same logical step selected for retry.
    fn finish_body(
        &self,
        body_id: &str,
        outcome: StepOutcome,
        reason: &str,
    ) -> anyhow::Result<PlayheadView> {
        let mut inner = self.inner();
        self.finish_body_locked(&mut inner, body_id, outcome, reason)
    }

    /// The lock-held half of [`Self::finish_body`], for callers already inside
    /// the `inner` guard (which cannot re-lock through `finish_body`).
    fn finish_body_locked(
        &self,
        inner: &mut Inner,
        body_id: &str,
        outcome: StepOutcome,
        reason: &str,
    ) -> anyhow::Result<PlayheadView> {
        let events = inner
            .playhead
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("playhead is not initialized"))?
            .finish_body(body_id, outcome, reason)?;
        self.journal_playhead_locked(inner, events)
    }

    fn update_body_session(&self, body_id: &str, session_id: &str) -> anyhow::Result<PlayheadView> {
        let mut inner = self.inner();
        let event = inner
            .playhead
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("playhead is not initialized"))?
            .update_body_session(body_id, session_id)?;
        let view = self.journal_playhead_locked(&mut inner, vec![event])?;
        if let Some(open) = inner.open.as_mut() {
            if let Some(body) = open
                .turn
                .body
                .as_mut()
                .filter(|body| body.body_id == body_id)
            {
                body.session_id = Some(session_id.to_string());
                let _ = self
                    .turn_tx
                    .send(TurnBroadcast::Whole(TurnFrame::share(open.turn.clone())));
            }
        }
        Ok(view)
    }

    /// Skip a selected step that has no live body (for example after a body
    /// failed and the resident could not restart). A live body must instead
    /// receive [`InboxItem::Skip`] so its terminal turn closes before advance.
    pub fn skip_current(&self, reason: &str) -> anyhow::Result<PlayheadView> {
        self.ensure_playhead()?;
        let mut inner = self.inner();
        let playhead = inner
            .playhead
            .as_mut()
            .expect("ensure_playhead initialized it");
        if playhead.active.is_some() {
            return Err(anyhow::anyhow!("current step still has a live body"));
        }
        let step = playhead
            .current()
            .ok_or_else(|| anyhow::anyhow!("playhead has no current step"))?;
        // A skipped step's body is instantaneous: it starts and ends at the
        // same moment, having never run.
        let mut body = BodyProvenance::for_step(&step, &self.repo_root);
        let body_id = body.body_id.clone();
        body.ended_at = Some(body.started_at.clone());
        body.termination_reason = Some(reason.to_string());
        let mut events = vec![playhead.start_body(body)?];
        events.extend(playhead.finish_body(&body_id, StepOutcome::Skipped, reason)?);
        self.journal_playhead_locked(&mut inner, events)
    }

    fn journal_playhead_locked(
        &self,
        inner: &mut Inner,
        events: Vec<PlayheadEvent>,
    ) -> anyhow::Result<PlayheadView> {
        let playhead = inner
            .playhead
            .clone()
            .ok_or_else(|| anyhow::anyhow!("playhead is not initialized"))?;
        for event in events {
            inner.journal.append(|_| EventKind::PlayheadChanged {
                event,
                playhead: Box::new(playhead.clone()),
            });
        }
        let view = playhead.view();
        let _ = self.playhead_tx.send(view.clone());
        Ok(view)
    }

    /// User messages journaled but not yet consumed by a turn — the durable
    /// queue. Replayed as `inbox` frames when a resident subscribes, and the
    /// validator for the resident's `answers` declarations.
    pub fn pending_messages(&self) -> Vec<PendingMessage> {
        self.inner().pending_messages.clone()
    }

    /// Live resident-directed ops (the supervisor's revive/janitor feed; the
    /// SSE path uses [`WaveRuntime::subscribe_with_snapshot`] for a gap-free
    /// pending replay).
    pub fn subscribe_inbox(&self) -> broadcast::Receiver<InboxItem> {
        self.inbox_tx.subscribe()
    }

    /// Live loop-state transitions (no snapshot).
    pub fn subscribe_states(&self) -> broadcast::Receiver<LoopState> {
        self.state_tx.subscribe()
    }

    /// Live turn frames (no snapshot).
    pub fn subscribe_turns(&self) -> broadcast::Receiver<TurnBroadcast> {
        self.turn_tx.subscribe()
    }

    /// Whether a resident has ever been spawned for / attached to this
    /// listener (see `/health`'s `loop` field).
    pub fn resident_expected(&self) -> bool {
        self.resident_expected.load(Ordering::Relaxed)
    }

    pub fn set_resident_expected(&self) {
        self.resident_expected.store(true, Ordering::Relaxed);
    }

    // -- Channel family --
    //
    // The primary channel is the served mind and the only journaled one. Child
    // names are addresses on the shared-store bus; this runtime never brokers
    // them, it only recognizes which ones its ear should fold
    // (`crate::wave::bus`).

    /// Whether `channel` is within this wave's family: the primary channel
    /// or a dot-descendant of the sanitized wave name (see [`channel_role`]).
    pub fn in_family(&self, channel: &str) -> bool {
        channel_role(&self.name, channel).is_some()
    }

    /// Whether `channel` addresses this wave's PRIMARY channel (raw or
    /// sanitized spelling).
    pub fn is_primary(&self, channel: &str) -> bool {
        channel_role(&self.name, channel) == Some(ChannelRole::Primary)
    }

    // -- Memory (the server holds MEMORY.md's pen) --
    //
    // `update` writes the compiled ORIGIN repo's wave/<name>/MEMORY.md; `add`
    // publishes a raw fact to the replayable delta. Both journal under the
    // same lock as every other append, so the file checkpoint and the stream
    // fold agree.

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
        inner.memory_adds.clear();
        // A send error just means no live subscribers.
        let _ = self.memory_tx.send(summary.to_string());
        Ok(())
    }

    /// Publish one fact and its evidence receipts to the replayable memory
    /// stream and journal `MemoryAdded`. The live string stream carries only the
    /// prose (receipts ride the journal for the `--json` view).
    ///
    /// # Errors
    /// Journal I/O only.
    pub fn append_memory(&self, fact: &str, receipts: Vec<Receipt>) -> std::io::Result<()> {
        let mut inner = self.inner();
        inner.journal.append(|_| EventKind::MemoryAdded {
            fact: fact.to_string(),
            receipts,
        });
        inner.memory_adds.push(fact.to_string());
        let _ = self.memory_add_tx.send(fact.to_string());
        Ok(())
    }

    /// Journal one `ClaimCited` event binding a Project/KR claim to its
    /// evidence. The wave journal is the source of truth; the PM receipt
    /// overlay is a rebuildable projection written alongside this by `lf pm
    /// cite`. No live broadcast yet — claim citations have no streaming
    /// consumer (report provenance is a later slice); the journal + overlay are
    /// the durable record.
    ///
    /// # Errors
    /// Journal I/O only.
    pub fn append_claim_cited(
        &self,
        claim_id: &str,
        receipts: Vec<Receipt>,
    ) -> std::io::Result<()> {
        self.inner().journal.append(|_| EventKind::ClaimCited {
            claim_id: claim_id.to_string(),
            receipts,
        });
        Ok(())
    }

    /// Facts added since the last externalization, oldest to newest.
    pub fn memory_adds(&self) -> Vec<String> {
        self.inner().memory_adds.clone()
    }

    /// Journal this boot's `ServerStarted` — once, after replay, when the
    /// listener is bound. Folds ignore it; the record gains a restart marker.
    pub fn journal_server_started(&self, pid: u32, endpoint: &str) {
        let mut inner = self.inner();
        inner.journal.append(|_| EventKind::ServerStarted {
            pid,
            endpoint: endpoint.to_string(),
        });
    }

    /// Atomically snapshot the thread (including the open turn) and the loop
    /// state, and subscribe to live frames for both. Every broadcast happens
    /// under the same lock as the append it reflects, so the receiver sees
    /// exactly the frames sent after this snapshot — no gap, no overlap, no
    /// frame older than the snapshot. A live frame's id may match a snapshot
    /// turn: it is that turn, newer; consumers replace by id.
    pub fn subscribe_with_snapshot(&self, limit: Option<usize>) -> Subscription {
        let inner = self.inner();
        Subscription {
            turns: snapshot_tail_locked(&inner, limit),
            turn_rx: self.turn_tx.subscribe(),
            state: inner.state.clone(),
            state_rx: self.state_tx.subscribe(),
            playhead: inner.playhead.as_ref().map(Playhead::view),
            playhead_rx: self.playhead_tx.subscribe(),
            memory_rx: self.memory_tx.subscribe(),
            memory_adds: inner.memory_adds.clone(),
            memory_add_rx: self.memory_add_tx.subscribe(),
            pending: inner.pending_messages.clone(),
            tasks: inner.tasks.clone(),
            projects: inner.projects.clone(),
            inbox_rx: self.inbox_tx.subscribe(),
        }
    }

    /// Attempt a loop-state transition. Legal moves append a `LoopState` event
    /// and apply; illegal moves are refused and logged — an illegal transition
    /// is a bug, never silently applied.
    pub fn transition(&self, to: LoopState, reason: &str) -> bool {
        let mut inner = self.inner();
        self.transition_locked(&mut inner, to, reason)
    }

    fn transition_locked(&self, inner: &mut Inner, to: LoopState, reason: &str) -> bool {
        if !can_transition(&inner.state, &to) {
            tracing::warn!(
                from = inner.state.name(),
                to = to.name(),
                reason,
                "illegal loop-state transition refused"
            );
            return false;
        }
        let from = std::mem::replace(&mut inner.state, to.clone());
        inner.journal.append(|_| EventKind::LoopState {
            from,
            to: to.clone(),
            reason: reason.to_string(),
        });
        // A send error just means no live subscribers.
        let _ = self.state_tx.send(to);
        true
    }

    /// `Turning → Interrupting` for the open turn (the resident reported a
    /// cancel in flight). Returns whether the transition applied — false when
    /// no turn is live.
    fn begin_interrupt(&self, reason: &str) -> bool {
        let mut inner = self.inner();
        let LoopState::Turning { turn_id } = inner.state.clone() else {
            return false;
        };
        self.transition_locked(&mut inner, LoopState::Interrupting { turn_id }, reason)
    }

    /// Janitor: finalize the open turn without a resident terminal delta —
    /// the interrupt deadline expired with the resident silent, or the
    /// resident process died mid-turn. Journals `TurnFinished`, closes the
    /// matching playhead body without advancing its logical step, requeues
    /// what the turn had claimed (it never answered it), commits and
    /// broadcasts the turn as accumulated so far, settles the loop to `Idle`,
    /// and arms the drop guard: late wire deltas for the closed turn are
    /// ignored until the next `TurnOpened`. Returns whether there was an open
    /// turn or active body to finalize.
    pub fn force_finalize_open_turn(&self, status: Lifecycle, reason: &str) -> bool {
        let mut inner = self.inner();
        let open = inner.open.take();
        let active_body_id = inner
            .playhead
            .as_ref()
            .and_then(|playhead| playhead.active.as_ref())
            .map(|body| body.body_id.clone());
        if open.is_none() && active_body_id.is_none() {
            return false;
        }

        if let Some(OpenTurn {
            mut turn, claims, ..
        }) = open
        {
            inner.drop_deltas_until_opened = true;
            let finished = inner.journal.append(|_| EventKind::TurnFinished {
                turn_id: turn.id.clone(),
                status,
                usage: Usage::empty(),
                termination_reason: Some(reason.to_string()),
            });
            if status != Lifecycle::Completed {
                self.requeue_locked(&mut inner, &claims);
            }
            turn.status = status;
            turn.close_body(finished.at_rfc3339(), Some(reason.to_string()));
            self.transition_locked(&mut inner, LoopState::Idle, reason);
            self.commit_locked(&mut inner, turn);
        }

        if let Some(body_id) = active_body_id {
            let outcome = match status {
                Lifecycle::Interrupted => StepOutcome::Interrupted,
                Lifecycle::Completed => StepOutcome::Completed,
                Lifecycle::Pending | Lifecycle::Running | Lifecycle::Failed => StepOutcome::Failed,
            };
            self.finish_body_locked(&mut inner, &body_id, outcome, reason)
                .expect("the active body belongs to an initialized playhead");
        }
        true
    }

    /// Return claimed-but-unanswered messages to the durable queue: journal
    /// `MessagesRequeued` and restore the pending fold, exactly what the fold
    /// replays. No live inbox re-broadcast — redelivery is the pending
    /// replay's job (the resident's next subscription), never a silent
    /// double-send to a loop that may still hold its own copy.
    fn requeue_locked(&self, inner: &mut Inner, ids: &[MessageId]) {
        let restored = restore_pending(&mut inner.pending_messages, &inner.messages, ids);
        if restored.is_empty() {
            return;
        }
        inner
            .journal
            .append(|_| EventKind::MessagesRequeued { ids: restored });
    }

    /// Push a turn into the thread cache and broadcast it live. The journal
    /// events for the turn must already be appended (same lock).
    fn commit_locked(&self, inner: &mut Inner, turn: ChatTurn) -> ChatTurn {
        turn.validate()
            .expect("Wave thread entries must satisfy the ChatTurn wire invariant");
        if turn.role == ChatRole::Assistant {
            inner.last_assistant_turn_id = Some(turn.id.clone());
        }
        inner.thread.push(turn.clone());
        // A send error just means no live subscribers — the store has it. The
        // finalized whole turn re-baselines any client that grew it from deltas.
        let _ = self
            .turn_tx
            .send(TurnBroadcast::Whole(TurnFrame::share(turn.clone())));
        turn
    }

    /// Deliver one human op from the thread door, uninterpreted by the caller:
    /// the door validates SHAPE (op names, text presence) and hands the op
    /// here; what an op *means* lives in this runtime and the loop's
    /// scheduler. The thread is unattributed — bylines arrive on the bus, via
    /// [`Self::deliver_say`]. A bare interrupt (empty text) journals nothing
    /// and appends no turn — `None`; every other delivery journals a
    /// `UserMessage`, commits the user turn, and queues for the loop.
    pub fn deliver(&self, op: MessageOp, text: String) -> Option<ChatTurn> {
        if op == MessageOp::Interrupt && text.trim().is_empty() {
            self.deliver_interrupt();
            return None;
        }
        Some(self.deliver_message(text, op, None))
    }

    /// Deliver an attributed emission folded off the bus — a worker report,
    /// child-wave escalation, or CLI FYI. Same journal row, thread commit, and
    /// inbox path as any user message; the byline rides along.
    pub fn deliver_say(&self, text: String, from: String) -> ChatTurn {
        self.deliver_message(text, MessageOp::Say, Some(from))
    }

    fn deliver_message(&self, text: String, op: MessageOp, from: Option<String>) -> ChatTurn {
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
        turn.from = from.clone();
        let turn = self.commit_locked(&mut inner, turn);
        // The pending fold stays live (not boot-only): it is the replay the
        // resident's subscription serves and the validator for its `answers`.
        let pending = PendingMessage { id, op, text, from };
        inner.messages.insert(pending.id.clone(), pending.clone());
        inner.pending_messages.push(pending.clone());
        // Inbox broadcast still under the lock, so inbox order == journal
        // order — sending after release lets two deliveries invert. A send
        // error just means no live subscribers; the pending fold has it.
        let _ = self.inbox_tx.send(InboxItem::Message(pending));
        turn
    }

    /// Deliver a bare interrupt (no text). Nothing is journaled here — the
    /// resident reports the `LoopState` transition when it fires the cancel;
    /// an interrupt while idle is a no-op by design.
    pub fn deliver_interrupt(&self) {
        let _ = self.inbox_tx.send(InboxItem::Interrupt);
    }

    pub fn deliver_skip(&self) {
        let _ = self.inbox_tx.send(InboxItem::Skip);
    }

    /// Journal and queue a typed Task observation exactly once.
    pub fn deliver_task_observation(&self, observation: TaskObservation) -> bool {
        let mut inner = self.inner();
        let pending = task_observation_message(&observation);
        if inner.tasks.contains_key(&pending.id) {
            return false;
        }
        let event = inner.journal.append(|_| EventKind::TaskObserved {
            observation: observation.clone(),
        });
        let turn = ChatTurn::child_activity(
            format!("turn-{}", event.seq),
            event.at_rfc3339(),
            "task".to_string(),
            crate::chat::turns::ChildControlActivity::from_task(&observation),
        );
        self.commit_locked(&mut inner, turn);
        inner.messages.insert(pending.id.clone(), pending.clone());
        inner.tasks.insert(pending.id.clone(), observation.clone());
        inner.pending_messages.push(pending);
        let _ = self.inbox_tx.send(InboxItem::Task(observation));
        true
    }

    /// Journal and queue a typed Project observation exactly once.
    pub fn deliver_project_observation(&self, observation: ProjectObservation) -> bool {
        let mut inner = self.inner();
        let pending = project_observation_message(&observation);
        if inner.projects.contains_key(&pending.id) {
            return false;
        }
        let event = inner.journal.append(|_| EventKind::ProjectObserved {
            observation: observation.clone(),
        });
        let turn = ChatTurn::child_activity(
            format!("turn-{}", event.seq),
            event.at_rfc3339(),
            "project".to_string(),
            crate::chat::turns::ChildControlActivity::from_project(&observation),
        );
        self.commit_locked(&mut inner, turn);
        inner.messages.insert(pending.id.clone(), pending.clone());
        inner
            .projects
            .insert(pending.id.clone(), observation.clone());
        inner.pending_messages.push(pending);
        let _ = self.inbox_tx.send(InboxItem::Project(observation));
        true
    }

    /// Record an already-finalized turn as its full event triple
    /// (`TurnStarted` + `TurnItem`s + `TurnFinished`) and commit it. Text
    /// becomes a `Message` item so the fold reproduces it. Does not touch the
    /// loop state — this is for instantaneous turns (injected narration), not
    /// loop turns.
    pub fn append_finalized_turn(&self, turn: ChatTurn, answers: Vec<MessageId>) -> ChatTurn {
        let mut inner = self.inner();
        let started = inner.journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers,
            body: turn.body.clone().map(Box::new),
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
            termination_reason: None,
        });
        let committed = ChatTurn {
            id: turn_id,
            created_at: started.at_rfc3339(),
            ..turn
        };
        self.commit_locked(&mut inner, committed)
    }

    // -- The resident wire fold (same lock discipline as everything above) --
    //
    // A turn opening broadcasts the (small) whole turn; each subsequent content
    // increment broadcasts one `turn-delta` frame carrying just the item, and
    // finalization broadcasts the whole terminal turn as a re-baseline. The
    // provider stream is per-token, so re-serializing the whole accumulated turn
    // per increment was O(prose²) on the wire (68.6 MB to deliver 3.1 KB of
    // prose); a delta is O(fragment). Subscribers reconstruct through the same
    // `absorb_item` rule the listener folds with, so their open turn is
    // byte-identical to this one.

    /// Apply one ordered wire delta from the resident (`POST
    /// /resident/deltas`). Malformed sequences — deltas for a turn that isn't
    /// open, late deltas after a force-finalize, answers naming unknown
    /// messages — are dropped with a warning, never journaled: the journal
    /// stays a record of what verifiably happened.
    pub fn apply_resident_delta(&self, delta: ResidentDelta) {
        match delta {
            ResidentDelta::TurnOpened { answers } => self.resident_turn_opened(answers),
            ResidentDelta::TurnText { text } => self.resident_turn_text(text),
            ResidentDelta::TurnItem { item } => self.resident_turn_item(item),
            ResidentDelta::TurnUsage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
            } => self.resident_turn_usage(input_tokens, output_tokens, cache_read_tokens),
            ResidentDelta::TurnFinished {
                status,
                cost_usd,
                reason,
            } => self.resident_turn_finished(status, cost_usd, reason),
            ResidentDelta::TurnSteered { answers } => self.resident_turn_steered(answers),
            ResidentDelta::MessagesRequeued { ids } => self.resident_requeue(ids),
            ResidentDelta::BodyStarted { body } => {
                if let Err(err) = self.start_body(body) {
                    tracing::warn!(error = %err, "resident body start rejected");
                }
            }
            ResidentDelta::BodySessionUpdated {
                body_id,
                session_id,
            } => {
                if let Err(err) = self.update_body_session(&body_id, &session_id) {
                    tracing::warn!(error = %err, "resident body session update rejected");
                }
            }
            ResidentDelta::BodyFinished {
                body_id,
                outcome,
                reason,
            } => {
                if let Err(err) = self.finish_body(&body_id, outcome, &reason) {
                    tracing::warn!(error = %err, "resident body finish rejected");
                }
            }
            ResidentDelta::LoopState { to, reason } => match to {
                ResidentStateTo::Interrupting => {
                    if !self.begin_interrupt(&reason) {
                        tracing::warn!(reason, "resident reported Interrupting with no live turn");
                    }
                }
                ResidentStateTo::Failed => {
                    self.transition(
                        LoopState::Failed {
                            reason: reason.clone(),
                        },
                        &reason,
                    );
                }
            },
        }
    }

    fn resident_turn_opened(&self, answers: Vec<String>) {
        let paused = self.paused();
        let mut inner = self.inner();
        inner.drop_deltas_until_opened = false;
        // Defensive: an Opened over an open turn closes the stale one failed
        // (the resident's adapter prevents this; a rogue sequence must not
        // wedge the fold). What the stale turn claimed is requeued — it never
        // answered it.
        if let Some(OpenTurn {
            turn: mut stale,
            usage,
            claims,
            ..
        }) = inner.open.take()
        {
            tracing::warn!(
                turn_id = stale.id,
                "TurnOpened over an open turn; closing the stale turn as failed"
            );
            inner.journal.append(|_| EventKind::TurnFinished {
                turn_id: stale.id.clone(),
                status: Lifecycle::Failed,
                usage,
                termination_reason: Some("stale open turn closed".to_string()),
            });
            self.requeue_locked(&mut inner, &claims);
            stale.status = Lifecycle::Failed;
            self.transition_locked(&mut inner, LoopState::Idle, "stale open turn closed");
            self.commit_locked(&mut inner, stale);
        }
        // The safety valve: a paused wave (GOAL.md `paused: true`) refuses to
        // start turns — nothing journaled, the queue keeps its messages for
        // an unpaused turn, and the refused turn's deltas drop whole.
        if paused {
            tracing::warn!(
                wave = self.name,
                "wave is paused (GOAL.md frontmatter); turn refused, deltas dropped until the next TurnOpened"
            );
            inner.drop_deltas_until_opened = true;
            return;
        }
        let answers = claim_answers(&mut inner, answers);
        let claims = answers.clone();
        let body = inner
            .playhead
            .as_ref()
            .and_then(|playhead| playhead.active.clone());
        let event = inner.journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers,
            body: body.clone().map(Box::new),
        });
        let turn_id = format!("turn-{}", event.seq);
        self.transition_locked(
            &mut inner,
            LoopState::Turning {
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
            body,
            activity: None,
        };
        let _ = self
            .turn_tx
            .send(TurnBroadcast::Whole(TurnFrame::share(open.clone())));
        inner.open = Some(OpenTurn {
            turn: open,
            usage: Usage::empty(),
            text_items: 0,
            claims,
        });
    }

    fn resident_turn_text(&self, text: String) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            return;
        }
        let Some(open) = inner.open.as_mut() else {
            tracing::warn!("text delta with no open turn; dropped");
            return;
        };
        let item = ConversationItem::Message {
            id: format!("text-{}", open.text_items),
            text,
            phase: Some("stream".to_string()),
        };
        open.text_items += 1;
        self.append_turn_item_locked(&mut inner, item);
    }

    fn resident_turn_item(&self, item: ConversationItem) {
        if matches!(&item, ConversationItem::Thought { text, .. } if text.trim().is_empty()) {
            return;
        }
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            return;
        }
        if inner.open.is_none() {
            tracing::warn!("item delta with no open turn; dropped");
            return;
        }
        self.append_turn_item_locked(&mut inner, item);
    }

    /// Journal a `TurnItem` for the open turn, grow the open-turn snapshot
    /// through the one shared rule (`ChatTurn::absorb_item` — the same call
    /// the journal fold makes), and broadcast the increment alone (a
    /// `turn-delta` frame) so live subscribers grow the turn without the whole
    /// accumulated turn crossing the wire each token.
    fn append_turn_item_locked(&self, inner: &mut Inner, item: ConversationItem) {
        let open = &mut inner.open.as_mut().expect("checked by callers").turn;
        let turn_id = open.id.clone();
        open.absorb_item(item.clone());
        let frame = TurnDeltaFrame::share(turn_id.clone(), item.clone());
        inner
            .journal
            .append(|_| EventKind::TurnItem { turn_id, item });
        let _ = self.turn_tx.send(TurnBroadcast::Delta(frame));
    }

    fn resident_turn_usage(
        &self,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
    ) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            return;
        }
        let Some(open) = inner.open.as_mut() else {
            return;
        };
        open.usage.input_tokens = add_opt(open.usage.input_tokens, input_tokens);
        open.usage.output_tokens = add_opt(open.usage.output_tokens, output_tokens);
        open.usage.cache_read_tokens = add_opt(open.usage.cache_read_tokens, cache_read_tokens);
    }

    fn resident_turn_finished(
        &self,
        status: Lifecycle,
        cost_usd: Option<f64>,
        reason: Option<String>,
    ) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            tracing::debug!("late TurnFinished after a force-finalize; dropped");
            return;
        }
        let Some(OpenTurn {
            mut turn,
            mut usage,
            claims,
            ..
        }) = inner.open.take()
        else {
            tracing::warn!("TurnFinished with no open turn; dropped");
            return;
        };
        usage.cost_usd = cost_usd;
        inner.journal.append(|_| EventKind::TurnFinished {
            turn_id: turn.id.clone(),
            status,
            usage,
            termination_reason: reason.clone(),
        });
        // Any non-Completed end requeues what the turn claimed: a failed or
        // interrupted turn never answered its messages.
        if status != Lifecycle::Completed {
            self.requeue_locked(&mut inner, &claims);
        }
        turn.status = status;
        turn.close_body(now_rfc3339(), reason);
        self.transition_locked(&mut inner, LoopState::Idle, "turn finalized");
        self.commit_locked(&mut inner, turn);
    }

    /// Steer consumption (`TurnSteered.answers`). Normally the live turn
    /// consumed the message; when the turn closed between the harness
    /// accepting the input and this delta arriving (the send/journal race),
    /// consumption lands against the last assistant turn — the vendor heard
    /// the text either way, and an unmarked message would stay pending
    /// forever and be re-sent on every resident restart. A user turn is never
    /// named. With no assistant turn anywhere (unreachable through the
    /// resident's steer path, which requires an open turn) nothing is claimed
    /// or journaled — the message stays pending.
    fn resident_turn_steered(&self, answers: Vec<String>) {
        let mut inner = self.inner();
        let (turn_id, turn_live) = match inner.state.clone() {
            LoopState::Turning { turn_id } | LoopState::Interrupting { turn_id } => (turn_id, true),
            _ => match inner.last_assistant_turn_id.clone() {
                Some(turn_id) => (turn_id, false),
                None => {
                    tracing::warn!("TurnSteered with no assistant turn anywhere; kept pending");
                    return;
                }
            },
        };
        let answers = claim_answers(&mut inner, answers);
        if answers.is_empty() {
            return;
        }
        // Steered into the live turn: part of its claims, requeued with them
        // if the turn ends without completing. The boundary-race fallback
        // names a turn that already closed completed — nothing to track.
        if turn_live {
            if let Some(open) = inner.open.as_mut() {
                open.claims.extend(answers.iter().cloned());
            }
        }
        inner
            .journal
            .append(|_| EventKind::TurnSteered { turn_id, answers });
    }

    /// The resident's explicit consumption undo ([`ResidentDelta::
    /// MessagesRequeued`]): it claimed these ids but the vendor never
    /// received the input (harness send failed after the claim journaled).
    /// Restore them to the pending fold — the next replay re-delivers; ids
    /// still pending or unknown are dropped by the restore's own guards.
    fn resident_requeue(&self, ids: Vec<String>) {
        let mut inner = self.inner();
        let ids: Vec<MessageId> = ids.into_iter().map(MessageId).collect();
        // Undone claims must not requeue a second time when the turn ends.
        if let Some(open) = inner.open.as_mut() {
            open.claims.retain(|claim| !ids.contains(claim));
        }
        self.requeue_locked(&mut inner, &ids);
    }
}

fn provider_session_from_body(body: &BodyProvenance) -> Option<ProviderSessionRef> {
    let harness = body.harness.as_deref()?.trim();
    let session_id = body.session_id.as_deref()?.trim();
    if harness.is_empty() || session_id.is_empty() {
        return None;
    }
    Some(ProviderSessionRef {
        harness: harness.to_string(),
        session_id: session_id.to_string(),
    })
}

fn body_has_harness(body: &BodyProvenance) -> bool {
    body.harness
        .as_deref()
        .is_some_and(|harness| !harness.trim().is_empty())
}

/// Validate a wire `answers` declaration against the pending fold: known ids
/// are claimed (removed from pending) and returned in wire order; unknown or
/// already-consumed ids are dropped with a warning — the journal never names
/// a consumer for a message it can't account for.
fn claim_answers(inner: &mut Inner, answers: Vec<String>) -> Vec<MessageId> {
    let mut valid = Vec::new();
    for id in answers {
        let id = MessageId(id);
        if let Some(pos) = inner.pending_messages.iter().position(|m| m.id == id) {
            inner.pending_messages.remove(pos);
            valid.push(id);
        } else {
            tracing::warn!(
                id = %id,
                "resident answered an unknown or already-consumed message; dropped"
            );
        }
    }
    valid
}

/// The thread plus the open turn, in one clone. The open turn rides last:
/// clients order by the sequence in the turn id, not array position.
fn snapshot_tail_locked(inner: &Inner, limit: Option<usize>) -> Vec<ChatTurn> {
    let open_count = usize::from(inner.open.is_some());
    let total = inner.thread.len() + open_count;
    let take = limit.unwrap_or(total).min(total);
    let take_open = take.min(open_count);
    let take_thread = take - take_open;
    let mut turns = inner.thread[inner.thread.len() - take_thread..].to_vec();
    if take_open == 1 {
        turns.extend(inner.open.as_ref().map(|open| open.turn.clone()));
    }
    turns
}

fn add_opt(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
    }
}

/// Current time as an RFC3339 string for `op` frame timestamps.
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Parse the sequence out of a `"turn-<n>"` id; panics on a malformed one
    /// (ids are always minted from journal seqs).
    fn turn_seq(id: &str) -> u64 {
        id.strip_prefix("turn-")
            .and_then(|n| n.parse().ok())
            .expect("turn id minted from journal seq")
    }

    /// The message id a delivered user turn journaled (`msg-<seq>`).
    fn msg_id(turn: &ChatTurn) -> String {
        format!("msg-{}", turn_seq(&turn.id))
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
            body: None,
            activity: None,
        }
    }

    fn open_runtime(repo: &Path) -> Arc<WaveRuntime> {
        WaveRuntime::open("ship".into(), repo.to_path_buf()).expect("open runtime")
    }

    // -- Wire delta builders (the resident door's vocabulary) --

    fn d_opened(answers: &[&str]) -> ResidentDelta {
        ResidentDelta::TurnOpened {
            answers: answers.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn d_text(text: &str) -> ResidentDelta {
        ResidentDelta::TurnText { text: text.into() }
    }

    fn d_tool() -> ResidentDelta {
        ResidentDelta::TurnItem {
            item: ConversationItem::Tool {
                id: "item-tool".into(),
                name: "Bash".into(),
                status: Lifecycle::Completed,
                input: None,
                output: Some("cargo test".into()),
            },
        }
    }

    fn d_thought(id: &str, text: &str) -> ResidentDelta {
        ResidentDelta::TurnItem {
            item: ConversationItem::Thought {
                id: id.into(),
                text: text.into(),
            },
        }
    }

    fn d_usage(input: u64, output: u64) -> ResidentDelta {
        ResidentDelta::TurnUsage {
            input_tokens: Some(input),
            output_tokens: Some(output),
            cache_read_tokens: None,
        }
    }

    fn d_finished(status: Lifecycle) -> ResidentDelta {
        ResidentDelta::TurnFinished {
            status,
            cost_usd: None,
            reason: None,
        }
    }

    fn complete_body(rt: &WaveRuntime, harness: &str, session_id: Option<&str>) {
        let step = rt
            .ensure_playhead()
            .expect("initialize playhead")
            .now
            .expect("wave has a current step");
        let mut body = BodyProvenance::for_step(&step, rt.repo_root());
        body.harness = Some(harness.to_string());
        body.session_id = session_id.map(str::to_string);
        let body_id = body.body_id.clone();
        rt.apply_resident_delta(ResidentDelta::BodyStarted { body });
        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(d_text("done"));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        rt.apply_resident_delta(ResidentDelta::BodyFinished {
            body_id,
            outcome: StepOutcome::Completed,
            reason: "completed".to_string(),
        });
    }

    #[test]
    fn turns_get_monotonic_ids_from_the_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let a = rt.append_finalized_turn(progress_turn("one"), Vec::new());
        let b = rt.append_finalized_turn(progress_turn("two"), Vec::new());
        assert!(turn_seq(&b.id) > turn_seq(&a.id));
        assert_eq!(rt.thread_snapshot().len(), 2);
    }

    #[test]
    fn narrated_turns_no_longer_blob_memory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        rt.append_finalized_turn(progress_turn("landed the parser"), Vec::new());
        // The journal carries raw history; MEMORY.md stays untouched until
        // the loop curates it deliberately.
        assert_eq!(rt.memory().read(), "");
    }

    #[test]
    fn deliver_appends_user_turn_and_broadcasts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
        let turn = rt
            .deliver(MessageOp::Message, "how goes it?".into())
            .expect("user turn");
        assert_eq!(turn.role, ChatRole::User);
        assert_eq!(turn.text, "how goes it?");
        // The op rode the live inbox broadcast, id tied to its journal event.
        let InboxItem::Message(msg) = rx.try_recv().expect("inbox message") else {
            panic!("expected a message inbox item");
        };
        assert_eq!(msg.text, "how goes it?");
        assert_eq!(msg.op, MessageOp::Message);
        assert_eq!(msg.id, MessageId(msg_id(&turn)));
        // And the durable queue has it immediately — no reboot needed.
        assert_eq!(rt.pending_messages().len(), 1);
    }

    #[test]
    fn task_observation_is_typed_idempotent_and_replayable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
        let observation = crate::task::TaskObservation {
            session_id: crate::task::TaskSessionId::from_raw("ts_example"),
            issue_identifier: "INF-123".to_string(),
            event_id: 7,
            control_source: None,
            event: crate::task::TaskEventKind::DecisionRequested {
                decision_id: crate::child_session::ChildDecisionId::new(),
                prompt: "Approve the plan?".to_string(),
                options: vec!["approve".to_string(), "revise".to_string()],
            },
        };

        assert!(rt.deliver_task_observation(observation.clone()));
        assert!(!rt.deliver_task_observation(observation.clone()));
        assert!(matches!(
            rx.try_recv().expect("live Task observation"),
            InboxItem::Task(ref received) if received == &observation
        ));
        let sub = rt.subscribe_with_snapshot(None);
        assert_eq!(sub.pending.len(), 1);
        assert_eq!(
            sub.tasks.get(&MessageId(observation.inbox_id())),
            Some(&observation)
        );

        drop(rt);
        let replayed = open_runtime(tmp.path()).subscribe_with_snapshot(None);
        assert_eq!(replayed.pending.len(), 1);
        assert_eq!(
            replayed.tasks.get(&MessageId(observation.inbox_id())),
            Some(&observation)
        );
    }

    #[test]
    fn bounded_subscription_tails_replay_but_keeps_live_turns() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        for i in 0..5 {
            rt.deliver(MessageOp::Message, format!("message {i}"))
                .expect("user turn");
        }

        let mut sub = rt.subscribe_with_snapshot(Some(2));
        assert_eq!(
            sub.turns
                .iter()
                .map(|turn| turn.text.as_str())
                .collect::<Vec<_>>(),
            vec!["message 3", "message 4"]
        );

        rt.deliver(MessageOp::Message, "message 5".into())
            .expect("live user turn");
        assert_eq!(
            sub.turn_rx
                .try_recv()
                .expect("live frame")
                .expect_whole()
                .text,
            "message 5",
            "the limit applies only to replay"
        );
    }

    #[test]
    fn project_observation_is_typed_idempotent_and_replayable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
        let observation = crate::project_session::ProjectObservation {
            session_id: crate::project_session::ProjectSessionId::from_raw("ps_example"),
            project: "developer-efficiency".to_string(),
            event_id: 8,
            control_source: None,
            event: crate::project_session::ProjectEventKind::Completed {
                summary: "all KRs hold".to_string(),
            },
        };

        assert!(rt.deliver_project_observation(observation.clone()));
        assert!(!rt.deliver_project_observation(observation.clone()));
        assert!(matches!(
            rx.try_recv().expect("live Project observation"),
            InboxItem::Project(ref received) if received == &observation
        ));
        let sub = rt.subscribe_with_snapshot(None);
        assert_eq!(
            sub.projects.get(&MessageId(observation.inbox_id())),
            Some(&observation)
        );

        drop(rt);
        let replayed = open_runtime(tmp.path()).subscribe_with_snapshot(None);
        assert_eq!(
            replayed.projects.get(&MessageId(observation.inbox_id())),
            Some(&observation)
        );
    }

    #[test]
    fn deliver_say_journals_attribution_and_queues_for_the_loop() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
        let from = "worker".to_string();
        let turn = rt.deliver_say("PR landed; one surprise in the fold".into(), from.clone());
        assert_eq!(turn.role, ChatRole::User);
        assert_eq!(turn.from.as_deref(), Some("worker"));

        // Inbox: an attributed Say message the loop reacts to like any input.
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
        let rt2 = open_runtime(tmp.path());
        let pending = rt2.pending_messages();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].from, Some(from));
        assert_eq!(rt2.thread_snapshot()[0].from.as_deref(), Some("worker"));
    }

    #[test]
    fn update_memory_writes_the_origin_file_and_add_publishes_delta() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());

        rt.update_memory("# Ship\n\n- fold is truth\n", "fold is truth")
            .expect("update");
        rt.append_memory("fold is truth", vec![]).expect("append");
        rt.append_memory("bullets append", vec![]).expect("append");
        assert_eq!(
            rt.memory().read(),
            "# Ship\n\n- fold is truth\n",
            "adds do not accrete raw facts into the compiled ORIGIN file"
        );

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let summaries: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::MemoryUpdated { summary } => Some(summary.as_str()),
                _ => None,
            })
            .collect();
        let facts: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::MemoryAdded { fact, .. } => Some(fact.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(summaries, vec!["fold is truth"]);
        assert_eq!(facts, vec!["fold is truth", "bullets append"]);
    }

    #[test]
    fn subscription_replays_full_memory_facts_in_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let long_fact = "workers report via lf radio pub with the full useful detail";

        rt.append_memory(long_fact, vec![]).expect("append");
        rt.append_memory("second fact", vec![]).expect("append");

        let sub = rt.subscribe_with_snapshot(None);
        assert_eq!(
            sub.memory_adds,
            vec![long_fact.to_string(), "second fact".to_string()]
        );
        assert!(
            sub.memory_add_rx.is_empty(),
            "snapshot facts do not replay live"
        );
    }

    #[test]
    fn memory_add_replay_buffer_rebuilds_from_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let rt = open_runtime(tmp.path());
            rt.append_memory("first", vec![]).expect("append");
            rt.append_memory("second", vec![]).expect("append");
            rt.update_memory("# Ship\n\ncompiled\n", "compiled")
                .expect("update");
            rt.append_memory("third", vec![]).expect("append");
        }

        let rt = open_runtime(tmp.path());
        let sub = rt.subscribe_with_snapshot(None);
        assert_eq!(
            sub.memory_adds,
            vec!["third".to_string()],
            "the replay buffer rebuilds adds since the last externalization"
        );
    }

    #[test]
    fn deliver_interrupt_is_a_control_item_not_a_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
        rt.deliver_interrupt();
        assert!(matches!(
            rx.try_recv().expect("inbox item"),
            InboxItem::Interrupt
        ));
        // Nothing journaled, nothing in the thread, nothing pending.
        assert!(rt.thread_snapshot().is_empty());
        assert!(rt.pending_messages().is_empty());
    }

    #[test]
    fn illegal_transition_is_refused_and_leaves_state_alone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        assert_eq!(rt.loop_state(), LoopState::Idle);

        // Nothing to interrupt when idle.
        assert!(!rt.transition(
            LoopState::Interrupting {
                turn_id: "turn-1".into()
            },
            "test"
        ));
        assert_eq!(rt.loop_state(), LoopState::Idle);

        // Legal: a turn opens, then finishes.
        assert!(rt.transition(
            LoopState::Turning {
                turn_id: "turn-1".into()
            },
            "test"
        ));
        assert!(!rt.transition(
            LoopState::Turning {
                turn_id: "turn-2".into()
            },
            "test"
        ));
        assert!(rt.transition(LoopState::Idle, "test"));
    }

    #[test]
    fn resident_deltas_journal_a_turn_and_commit_it() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());

        rt.apply_resident_delta(d_opened(&[]));
        assert_eq!(
            rt.loop_state().name(),
            "turning",
            "mid-turn the loop is Turning"
        );
        rt.apply_resident_delta(d_text("hello"));
        rt.apply_resident_delta(d_tool());
        rt.apply_resident_delta(d_usage(10, 4));
        rt.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: Some(0.02),
            reason: None,
        });

        assert_eq!(rt.loop_state(), LoopState::Idle, "back to idle after turn");
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 1);
        let turn = &thread[0];
        assert_eq!(turn.text, "hello");
        assert_eq!(turn.items.len(), 1);
        assert_eq!(turn.status, Lifecycle::Completed);
        // The id comes from the journal seq domain (turn_seq panics otherwise).
        turn_seq(&turn.id);

        // The journal's TurnFinished carries the accrued usage and the cost.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let usage = events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::TurnFinished { usage, .. } => Some(usage.clone()),
                _ => None,
            })
            .expect("TurnFinished journaled");
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, Some(4));
        assert_eq!(usage.cost_usd, Some(0.02));
    }

    #[test]
    fn provider_session_survives_runtime_reopen() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        complete_body(&rt, "codex", Some("thread-resume"));
        assert_eq!(
            rt.latest_provider_session(),
            Some(ProviderSessionRef {
                harness: "codex".to_string(),
                session_id: "thread-resume".to_string(),
            })
        );

        drop(rt);
        let reopened = open_runtime(tmp.path());
        assert_eq!(
            reopened.latest_provider_session(),
            Some(ProviderSessionRef {
                harness: "codex".to_string(),
                session_id: "thread-resume".to_string(),
            })
        );
    }

    #[test]
    fn newest_harness_body_masks_an_older_provider_session() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        complete_body(&rt, "codex", Some("codex-thread"));
        complete_body(&rt, "claude", None);

        assert_eq!(rt.latest_provider_session(), None);
    }

    /// The consumption declaration is the RESIDENT's, validated by the
    /// listener: known pending ids are claimed and journaled in
    /// `TurnStarted.answers`; unknown or already-consumed ids are dropped.
    #[test]
    fn turn_opened_answers_are_validated_against_the_pending_fold() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let m1 = rt
            .deliver(MessageOp::Message, "first".into())
            .expect("user turn");
        let m2 = rt
            .deliver(MessageOp::Message, "second".into())
            .expect("user turn");
        assert_eq!(rt.pending_messages().len(), 2);

        // The turn claims both real messages plus a ghost id.
        rt.apply_resident_delta(d_opened(&[&msg_id(&m1), &msg_id(&m2), "msg-999"]));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        assert!(
            rt.pending_messages().is_empty(),
            "claimed messages leave the live pending fold"
        );

        // A second turn re-claiming a consumed id gets nothing.
        rt.apply_resident_delta(d_opened(&[&msg_id(&m1)]));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
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
            vec![MessageId(msg_id(&m1)), MessageId(msg_id(&m2))],
            "valid ids journaled, the ghost dropped"
        );
        assert!(answers[1].is_empty(), "already-consumed ids never re-claim");

        // The fold agrees on restart: nothing pending.
        let fold = fold_thread(&events);
        assert!(fold.pending_messages.is_empty());
    }

    #[test]
    fn open_turn_streams_growing_snapshots_then_the_terminal_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let sub = rt.subscribe_with_snapshot(None);
        assert!(sub.turns.is_empty());
        assert_eq!(sub.state, LoopState::Idle);
        let mut frames = sub.turn_rx;
        let mut states = sub.state_rx;

        // The turn opens empty and running as a WHOLE frame; content then
        // arrives as increments (`turn-delta`), never the re-serialized turn.
        // A subscriber reconstructs the open turn by absorbing each delta onto
        // the opened whole — so we keep a running reconstruction and assert it
        // matches the terminal turn byte for byte at the end.
        rt.apply_resident_delta(d_opened(&[]));
        let opened = frames
            .try_recv()
            .expect("opened frame")
            .expect_whole()
            .clone();
        assert_eq!(opened.status, Lifecycle::Running);
        assert_eq!(opened.text, "");
        let mut reconstruction = opened.clone();

        rt.apply_resident_delta(d_text("thinking"));
        let text_delta = frames
            .try_recv()
            .expect("text delta")
            .expect_delta()
            .clone();
        assert_eq!(text_delta.turn_id, opened.id);
        reconstruction.absorb_item(text_delta.item);
        assert_eq!(reconstruction.text, "thinking");

        // Mid-turn, the open turn rides the snapshot after the thread — the
        // listener still holds the whole turn even though the wire sent a delta.
        let mid = rt.thread_snapshot();
        assert_eq!(mid.len(), 1);
        assert_eq!(mid[0].id, opened.id);
        assert_eq!(mid[0].status, Lifecycle::Running);
        assert_eq!(mid[0].text, "thinking");

        // An item delta grows the reconstruction the same way.
        rt.apply_resident_delta(d_tool());
        let item_delta = frames
            .try_recv()
            .expect("item delta")
            .expect_delta()
            .clone();
        assert_eq!(item_delta.turn_id, opened.id);
        reconstruction.absorb_item(item_delta.item);
        assert_eq!(reconstruction.items.len(), 1);
        assert_eq!(reconstruction.text, "thinking");

        // Finalization replaces the running turn under the same id — a WHOLE
        // frame that re-baselines the reconstruction.
        rt.apply_resident_delta(d_usage(10, 5));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        let terminal = frames
            .try_recv()
            .expect("terminal frame")
            .expect_whole()
            .clone();
        assert_eq!(terminal.id, opened.id);
        assert_eq!(terminal.status, Lifecycle::Completed);

        // Reconstruction identity: growing from deltas yields the same prose and
        // items the listener finalized. (Status differs — the terminal WHOLE
        // frame carries it; that is exactly the re-baseline's job.)
        assert_eq!(reconstruction.text, terminal.text);
        assert_eq!(reconstruction.items, terminal.items);

        // No stale running turn remains anywhere.
        let after = rt.thread_snapshot();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].status, Lifecycle::Completed);
        assert!(frames.try_recv().is_err(), "no extra frames");

        // Every transition was broadcast: Idle → Turning → Idle.
        assert!(matches!(
            states.try_recv().expect("turning state frame"),
            LoopState::Turning { .. }
        ));
        assert_eq!(
            states.try_recv().expect("idle state frame"),
            LoopState::Idle
        );
        assert!(states.try_recv().is_err(), "no extra state frames");
    }

    /// The bug W2-134 kills: a per-token text delta used to re-serialize the
    /// whole accumulated turn, so a turn carrying a large tool output put that
    /// output on the wire again on every subsequent token (O(prose²)). The delta
    /// frame must carry ONLY the increment, whatever the open turn has piled up.
    #[test]
    fn a_text_delta_never_carries_the_turns_accumulated_items() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut frames = rt.subscribe_with_snapshot(None).turn_rx;

        rt.apply_resident_delta(d_opened(&[]));
        let _ = frames.try_recv().expect("opened frame");

        // A big tool output lands in the open turn.
        let big_output = "x".repeat(100_000);
        rt.apply_resident_delta(ResidentDelta::TurnItem {
            item: ConversationItem::Tool {
                id: "item-big".into(),
                name: "Bash".into(),
                status: Lifecycle::Completed,
                input: None,
                output: Some(big_output.clone()),
            },
        });
        let _ = frames.try_recv().expect("tool delta");

        // The next token's frame is O(fragment): it carries just the fragment,
        // never the 100 KB the turn now holds — the amplification is gone.
        rt.apply_resident_delta(d_text("tiny"));
        let TurnBroadcast::Delta(frame) = frames.try_recv().expect("text delta") else {
            panic!("a text increment must broadcast a delta, not a whole turn");
        };
        assert!(
            !frame.json.contains(&big_output),
            "the delta frame must not re-send the turn's accumulated items"
        );
        assert!(
            frame.json.len() < 200,
            "a token's frame stays tiny ({} bytes) regardless of turn size",
            frame.json.len()
        );
    }

    #[test]
    fn empty_thoughts_never_enter_the_thread_or_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut frames = rt.subscribe_with_snapshot(None).turn_rx;

        rt.apply_resident_delta(d_opened(&[]));
        let opened = frames.try_recv().expect("opened frame");
        assert!(opened.expect_whole().items.is_empty());

        rt.apply_resident_delta(d_thought("empty", "  \n\t"));
        assert!(frames.try_recv().is_err(), "empty thought emits no frame");
        assert!(rt.thread_snapshot()[0].items.is_empty());

        rt.apply_resident_delta(d_thought("real", "checking the retry"));
        let grown = frames.try_recv().expect("real thought frame");
        assert!(matches!(
            &grown.expect_delta().item,
            ConversationItem::Thought { id, text }
                if id == "real" && text == "checking the retry"
        ));

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let thought_items: Vec<&ConversationItem> = events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::TurnItem { item, .. } => Some(item),
                _ => None,
            })
            .collect();
        assert_eq!(thought_items.len(), 1, "only the real thought is journaled");
    }

    /// The listener-side janitor: force-finalize closes the journal and
    /// settles Idle, and the drop guard swallows the resident's late deltas —
    /// including its own eventual TurnFinished — until the next TurnOpened.
    #[test]
    fn force_finalize_closes_the_turn_and_drops_late_deltas() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let step = rt
            .ensure_playhead()
            .expect("initialize playhead")
            .now
            .expect("wave has a current step");
        let body = BodyProvenance {
            body_id: "body-dead".into(),
            invocation_id: step.invocation_id,
            step_index: step.index,
            flow: step.flow,
            step: step.step,
            iteration: step.iteration,
            session_id: Some("session-dead".into()),
            harness: Some("codex".into()),
            model: None,
            host: "host".into(),
            worktree: tmp.path().display().to_string(),
            started_at: "2026-07-09T00:00:00Z".into(),
            ended_at: None,
            termination_reason: None,
        };
        rt.apply_resident_delta(ResidentDelta::BodyStarted { body });
        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(d_text("half"));
        rt.apply_resident_delta(ResidentDelta::LoopState {
            to: ResidentStateTo::Interrupting,
            reason: "user interrupt".into(),
        });
        assert_eq!(rt.loop_state().name(), "interrupting");

        assert!(rt.force_finalize_open_turn(Lifecycle::Interrupted, "deadline"));
        assert_eq!(rt.loop_state(), LoopState::Idle);
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].status, Lifecycle::Interrupted);
        assert_eq!(thread[0].text, "half");
        let body = thread[0].body.as_ref().expect("turn keeps body provenance");
        assert!(body.ended_at.is_some());
        assert_eq!(body.termination_reason.as_deref(), Some("deadline"));
        let playhead = rt.playhead().expect("playhead survives finalization");
        assert!(playhead.active.is_none(), "the dead body releases its seat");
        assert_eq!(
            playhead.now.expect("failed step remains selected").index,
            0,
            "an interrupted body retries the same logical step"
        );

        // The journal is closed: a replay agrees, no open turn survives.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let fold = crate::wave::journal::fold_thread(&events);
        assert!(fold.open.is_empty());
        assert_eq!(fold.turns.last().unwrap().status, Lifecycle::Interrupted);
        assert!(
            fold.playhead.expect("replayed playhead").active.is_none(),
            "replay releases the dead body too"
        );

        // Nothing left to force a second time.
        assert!(!rt.force_finalize_open_turn(Lifecycle::Interrupted, "again"));

        // Late deltas for the closed turn are dropped whole…
        let journal_len = events.len();
        rt.apply_resident_delta(d_text("late text"));
        rt.apply_resident_delta(d_usage(1, 1));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        assert_eq!(events.len(), journal_len, "late deltas journal nothing");
        assert_eq!(rt.thread_snapshot().len(), 1, "thread untouched");
        assert_eq!(rt.loop_state(), LoopState::Idle, "no double transition");

        // …and the next TurnOpened clears the guard: life goes on.
        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(d_text("fresh"));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 2);
        assert_eq!(thread[1].text, "fresh");
        assert_eq!(thread[1].status, Lifecycle::Completed);
    }

    /// Family membership compares against the SANITIZED wave name: a wave
    /// whose name sanitizes (`web/ui` → `web-ui`) mints `web-ui.<run>`
    /// channels — those must pass `in_family`, and the raw name still
    /// addresses the primary. This is the one predicate server scoping and
    /// the ambient dot-split share.
    #[test]
    fn channel_role_compares_against_the_sanitized_wave_name() {
        // The raw name and its sanitized form both address the primary.
        assert_eq!(channel_role("web/ui", "web/ui"), Some(ChannelRole::Primary));
        assert_eq!(channel_role("web/ui", "web-ui"), Some(ChannelRole::Primary));
        // A child channel carries the sanitized head (worktree basenames do).
        assert_eq!(
            channel_role("web/ui", "web-ui.148e0e02"),
            Some(ChannelRole::Child)
        );
        // The un-sanitized dotted form is NOT the family (no such channel).
        assert_eq!(channel_role("web/ui", "web/ui.148e"), None);
        // A plain name is its own sanitized form.
        assert_eq!(channel_role("goals", "goals"), Some(ChannelRole::Primary));
        assert_eq!(
            channel_role("goals", "goals.148e0e02"),
            Some(ChannelRole::Child)
        );
        assert_eq!(channel_role("goals", "goalsmith"), None);
        assert_eq!(channel_role("goals", "concerto"), None);
    }

    /// A sanitized-name wave recognizes its family by the SANITIZED channel
    /// name — the form hands actually carry. The runtime brokers nothing; it
    /// only knows which bus channels its ear should fold.
    #[test]
    fn a_sanitized_wave_recognizes_its_family_by_the_sanitized_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        let rt = WaveRuntime::open("web/ui".into(), origin).expect("open runtime");
        assert_eq!(rt.channel_name(), "web-ui");
        assert!(rt.is_primary("web-ui"));
        assert!(
            rt.is_primary("web/ui"),
            "the raw spelling still addresses it"
        );
        assert!(rt.in_family("web-ui.148e"));
        assert!(!rt.in_family("web-ui-other"));
    }

    /// Claimed-but-unanswered messages are requeued when a turn ends without
    /// completing: a Failed TurnFinished returns its claims to pending, and a
    /// restart re-delivers them (never lost). A Completed turn keeps them
    /// consumed.
    #[test]
    fn failed_turn_requeues_its_claimed_messages() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let m1 = rt
            .deliver(MessageOp::Message, "do the thing".into())
            .expect("user turn");

        rt.apply_resident_delta(d_opened(&[&msg_id(&m1)]));
        assert!(rt.pending_messages().is_empty(), "claimed at open");
        // The turn fails: the vendor never answered it, back to pending.
        rt.apply_resident_delta(d_finished(Lifecycle::Failed));
        let pending = rt.pending_messages();
        assert_eq!(pending.len(), 1, "failed turn requeues its claim");
        assert_eq!(pending[0].text, "do the thing");

        // The fold agrees on restart — the requeue is journaled.
        let rt2 = open_runtime(tmp.path());
        assert_eq!(rt2.pending_messages().len(), 1);

        // A completed turn keeps its claim consumed (own journal — the shared
        // path above still holds m1's requeue, which never re-answered).
        let tmp3 = tempfile::tempdir().expect("tempdir");
        let rt3 = open_runtime(tmp3.path());
        let m2 = rt3
            .deliver(MessageOp::Message, "second".into())
            .expect("user turn");
        rt3.apply_resident_delta(d_opened(&[&msg_id(&m2)]));
        rt3.apply_resident_delta(d_finished(Lifecycle::Completed));
        assert!(
            rt3.pending_messages().is_empty(),
            "a completed turn consumes its claim for good"
        );
    }

    /// The boot janitor requeues what a CRASHED turn had claimed: a turn
    /// started (claiming a message) but never finished loses its claim to the
    /// crash — the next boot returns it to pending so a fresh resident
    /// re-delivers.
    #[test]
    fn boot_janitor_requeues_a_crashed_turns_claims() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let claimed = {
            let rt = open_runtime(tmp.path());
            let m = rt
                .deliver(MessageOp::Message, "answer me".into())
                .expect("user turn");
            // Turn opens and claims it, then the server crashes (no finish).
            rt.apply_resident_delta(d_opened(&[&msg_id(&m)]));
            assert!(rt.pending_messages().is_empty());
            msg_id(&m)
        };
        // Second life: the janitor closes the crashed turn AND requeues.
        let rt = open_runtime(tmp.path());
        let pending = rt.pending_messages();
        assert_eq!(pending.len(), 1, "crashed turn's claim is requeued");
        assert_eq!(pending[0].id, MessageId(claimed));
        // Idempotent: a third boot doesn't requeue twice.
        let rt2 = open_runtime(tmp.path());
        assert_eq!(rt2.pending_messages().len(), 1);
    }

    /// The resident's explicit consumption undo (`MessagesRequeued`): a claim
    /// the vendor never received is returned to pending, and the turn's own
    /// terminal delta does not requeue it a second time.
    #[test]
    fn resident_requeue_undoes_a_claim_at_most_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let m = rt
            .deliver(MessageOp::Steer, "steer".into())
            .expect("user turn");
        rt.apply_resident_delta(d_opened(&[&msg_id(&m)]));
        // The harness send failed after the claim: the resident undoes it.
        rt.apply_resident_delta(ResidentDelta::MessagesRequeued {
            ids: vec![msg_id(&m)],
        });
        assert_eq!(
            rt.pending_messages().len(),
            1,
            "undone claim back to pending"
        );
        // The turn then finishes failed: the already-undone claim is not
        // requeued a second time (still exactly one pending).
        rt.apply_resident_delta(d_finished(Lifecycle::Failed));
        assert_eq!(rt.pending_messages().len(), 1, "no double requeue");
    }

    /// A paused wave (GOAL.md `paused: true`) refuses to START a turn: the
    /// TurnOpened is dropped, its would-be claims stay pending, and the loop
    /// settles without a thread turn. Unpausing lets the next turn through.
    #[test]
    fn paused_wave_refuses_to_start_turns() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path();
        std::fs::create_dir_all(origin.join("wave/ship")).unwrap();
        std::fs::write(
            origin.join("wave/ship/GOAL.md"),
            "---\npaused: true\n---\nShip it.\n",
        )
        .unwrap();
        let rt = open_runtime(origin);
        assert!(rt.paused(), "GOAL.md says paused");
        let m = rt
            .deliver(MessageOp::Message, "go".into())
            .expect("user turn");

        rt.apply_resident_delta(d_opened(&[&msg_id(&m)]));
        rt.apply_resident_delta(d_text("working"));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        // No assistant turn committed; the message is still queued.
        assert!(
            rt.thread_snapshot()
                .iter()
                .all(|t| t.role == ChatRole::User),
            "paused: no assistant turn started"
        );
        assert_eq!(rt.pending_messages().len(), 1, "the message waits");

        // Unpause: the next turn goes through.
        std::fs::write(
            origin.join("wave/ship/GOAL.md"),
            "---\npaused: false\n---\nShip it.\n",
        )
        .unwrap();
        assert!(!rt.paused());
        rt.apply_resident_delta(d_opened(&[&msg_id(&m)]));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        assert!(
            rt.pending_messages().is_empty(),
            "unpaused turn answered it"
        );
    }

    /// Steer consumption over the wire: the live turn answers a steered
    /// message; the boundary race (turn closed during the send) falls back to
    /// the last assistant turn; nothing is claimed with no assistant turn
    /// anywhere.
    #[test]
    fn turn_steered_consumes_against_live_or_just_closed_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());

        let m1 = rt
            .deliver(MessageOp::Steer, "steer me".into())
            .expect("user turn");
        let m2 = rt
            .deliver(MessageOp::Steer, "me too".into())
            .expect("user turn");

        // No assistant turn anywhere: nothing journaled, both stay pending.
        rt.apply_resident_delta(ResidentDelta::TurnSteered {
            answers: vec![msg_id(&m1)],
        });
        assert_eq!(rt.pending_messages().len(), 2, "kept pending");

        // First consumed mid-turn, the normal steer path.
        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(ResidentDelta::TurnSteered {
            answers: vec![msg_id(&m1)],
        });
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));

        // The boundary race: the turn closed between the harness accepting
        // the input and the delta arriving. The marker still lands, against
        // the last assistant turn.
        rt.apply_resident_delta(ResidentDelta::TurnSteered {
            answers: vec![msg_id(&m2)],
        });

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
                (assistant_turn.clone(), vec![MessageId(msg_id(&m1))]),
                (assistant_turn, vec![MessageId(msg_id(&m2))]),
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

    /// The steer-consumption fallback is seeded from the journal on boot: a
    /// restarted runtime still names the last assistant turn, never the user
    /// turn that carried the steer text.
    #[test]
    fn turn_steered_fallback_survives_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let assistant_id = {
            let rt = open_runtime(tmp.path());
            rt.apply_resident_delta(d_opened(&[]));
            rt.apply_resident_delta(d_finished(Lifecycle::Completed));
            rt.thread_snapshot()
                .iter()
                .find(|turn| turn.role == ChatRole::Assistant)
                .expect("assistant turn")
                .id
                .clone()
        };

        let rt = open_runtime(tmp.path());
        // The steer's own user turn is now the thread's last turn.
        let steer = rt
            .deliver(MessageOp::Steer, "steer me".into())
            .expect("user turn");
        rt.apply_resident_delta(ResidentDelta::TurnSteered {
            answers: vec![msg_id(&steer)],
        });

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let steered_turn = events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::TurnSteered { turn_id, .. } => Some(turn_id.clone()),
                _ => None,
            })
            .expect("TurnSteered journaled");
        assert_eq!(
            steered_turn, assistant_id,
            "fallback names the first life's assistant turn, not the user turn"
        );
    }

    /// One journal per served mind, zero per channel. The mind's own thread is
    /// the only thing this runtime writes; hands' reports arrive through the
    /// bus and its ear (`crate::wave::bus`), never through here.
    #[test]
    fn only_the_served_mind_journals() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let rt = WaveRuntime::open("ship".into(), origin.clone()).expect("open runtime");

        rt.deliver(MessageOp::Message, "to the wave".into())
            .expect("user turn");
        rt.deliver_say("to a".into(), "ship.a".into());

        let wave = rt.thread_snapshot();
        assert_eq!(wave.len(), 2);
        assert_eq!(wave[0].text, "to the wave");
        assert_eq!(wave[1].from.as_deref(), Some("ship.a"));

        // On disk: exactly one journal, the served wave's, with both rows.
        assert_eq!(
            loopflow_test_support::journal_files_under(tmp.path()),
            vec![journal_path(&origin, "ship")]
        );
        let events = crate::wave::journal::read_events(&journal_path(&origin, "ship"));
        let messages = events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::UserMessage { .. }))
            .count();
        assert_eq!(messages, 2, "the wave's message and the folded report");

        // Consumption: both are queued for the loop across a reopen.
        let rt2 = WaveRuntime::open("ship".into(), origin.clone()).expect("reopen");
        let pending = rt2.pending_messages();
        let texts: Vec<_> = pending.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts.len(), 2);
        assert!(texts.contains(&"to the wave") && texts.contains(&"to a"));
    }

    /// A subscription's snapshot carries the pending queue (the resident's
    /// boot replay) and its receiver carries exactly the ops sent after it.
    #[test]
    fn subscription_carries_pending_replay_and_live_inbox() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        rt.deliver(MessageOp::Message, "before".into())
            .expect("user turn");

        let mut sub = rt.subscribe_with_snapshot(None);
        assert_eq!(sub.pending.len(), 1);
        assert_eq!(sub.pending[0].text, "before");
        assert!(sub.inbox_rx.try_recv().is_err(), "no frames from before");

        rt.deliver(MessageOp::Message, "after".into())
            .expect("user turn");
        rt.deliver_interrupt();
        let InboxItem::Message(live) = sub.inbox_rx.try_recv().expect("live frame") else {
            panic!("expected message");
        };
        assert_eq!(live.text, "after");
        assert!(matches!(
            sub.inbox_rx.try_recv().expect("interrupt frame"),
            InboxItem::Interrupt
        ));
    }

    /// Journal order and inbox order are one order: the inbox broadcast
    /// happens under the same lock as the append, so concurrent deliveries
    /// can never invert between the durable queue fold and the live channel.
    #[test]
    fn concurrent_deliveries_keep_inbox_order_equal_to_journal_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();

        let mut handles = Vec::new();
        for writer in 0..4 {
            let rt = rt.clone();
            handles.push(std::thread::spawn(move || {
                for i in 0..50 {
                    rt.deliver(MessageOp::Message, format!("m-{writer}-{i}"))
                        .expect("user turn");
                }
            }));
        }
        for handle in handles {
            handle.join().expect("writer thread");
        }

        let mut inbox_ids = Vec::new();
        while let Ok(item) = rx.try_recv() {
            let InboxItem::Message(message) = item else {
                panic!("only messages were delivered");
            };
            inbox_ids.push(message.id);
        }
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let journal_ids: Vec<MessageId> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::UserMessage { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(inbox_ids.len(), 200);
        assert_eq!(
            inbox_ids, journal_ids,
            "inbox consumption order == journal fold order"
        );
    }
}
