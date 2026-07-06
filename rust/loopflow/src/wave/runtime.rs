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
//! Two independent inputs feed the journal: the resident's wire deltas
//! ([`WaveRuntime::apply_resident_delta`] — the old in-process `TurnSink`
//! vocabulary, now arriving over `POST /resident/deltas`) and user messages
//! (HTTP → journal + inbox broadcast). All appends go through one lock, so
//! journal order, cache order, and broadcast order agree — one writer appends
//! and broadcasts. This module is vendor-free: the harness lives with the
//! resident process, never here.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::broadcast;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::chat::types::{ConversationItem, Lifecycle};
use crate::engine::wave_config::read_wave_config;
use crate::lfd::security::sanitize_fs_component;
use crate::wave::channel::{matches_prefix, scan_child_channels, ChannelFrame, ChildChannel};
use crate::wave::journal::{
    channel_opened_turn, fold_thread, fold_workers, journal_path, restore_pending,
    run_completed_turn, Attribution, EventKind, Journal, MessageId, MessageOp, PendingMessage,
    Usage, WorkerOutcome, WorkerRecord,
};
use crate::wave::memory::Memory;
use crate::wave::state::{can_transition, MindState};
use crate::wave::wire::{ResidentDelta, ResidentStateTo};

/// Capacity of the live turn broadcast. SSE clients that fall this far behind
/// get a lag error and resync from `/conversation`; the journal is the source
/// of truth, so a dropped live turn is never lost.
const TURN_BROADCAST_CAPACITY: usize = 256;

/// Capacity of the live mind-state broadcast. Transitions are rare (a few per
/// turn); a lagged subscriber just resyncs from the next transition.
const STATE_BROADCAST_CAPACITY: usize = 64;

/// Capacity of the live memory broadcast. Curation is deliberate and rare;
/// a lagged subscriber reads MEMORY.md itself.
const MEMORY_BROADCAST_CAPACITY: usize = 64;

/// Capacity of the family bus (child-channel turn frames). Same reasoning as
/// the primary turn broadcast: liveness only, a lagged subscriber resyncs.
const FAMILY_BROADCAST_CAPACITY: usize = 256;

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
/// (`web/ui` mints `web-ui` worktrees and `web-ui.<run>` channels; see
/// `lfd::security::sanitize_fs_component`).
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

/// One live turn frame: the turn plus its wire JSON, serialized ONCE at the
/// send site so N subscribers share one serialization instead of performing
/// N (the delta-granular wire — sending increments instead of whole turns —
/// stays future work).
#[derive(Debug)]
pub struct TurnFrame {
    pub turn: ChatTurn,
    /// The turn as `/events` `turn`-frame JSON.
    pub json: String,
}

impl TurnFrame {
    fn share(turn: ChatTurn) -> Arc<Self> {
        let json = serde_json::to_string(&turn).unwrap_or_default();
        Arc::new(Self { turn, json })
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
    /// Live turn frames ride as `Arc<TurnFrame>`s: the broadcast clones once
    /// per subscriber, so N subscribers share one allocation — and one JSON
    /// serialization — per frame instead of N.
    pub turn_rx: broadcast::Receiver<Arc<TurnFrame>>,
    pub state: MindState,
    pub state_rx: broadcast::Receiver<MindState>,
    /// Live `MemoryUpdated` summaries — fired on every curation, no replay
    /// (the file itself is the durable state).
    pub memory_rx: broadcast::Receiver<String>,
    /// Memory facts added since the last externalization, replayed on
    /// subscribe before the live stream continues.
    pub memory_adds: Vec<String>,
    pub memory_add_rx: broadcast::Receiver<String>,
    /// The pending queue as of the snapshot: journaled user messages not yet
    /// named in any `answers` — the resident's boot replay.
    pub pending: Vec<PendingMessage>,
    /// Live resident-directed ops sent after the snapshot.
    pub inbox_rx: broadcast::Receiver<InboxItem>,
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
    /// Usage accrued for the open turn from `TurnUsage` deltas.
    open_usage: Usage,
    /// Count of prose fragments in the open turn, for `Message` item ids
    /// (`"text-<n>"`).
    open_text_items: usize,
    /// Set by a force-finalize (interrupt-deadline janitor, resident death):
    /// the journal already closed the turn, so late wire deltas for it —
    /// including the resident's own eventual `TurnFinished` — are dropped
    /// until the next `TurnOpened`.
    drop_deltas_until_opened: bool,
    state: MindState,
    thread_id: Option<String>,
    /// Id of the mind's current or most recently committed assistant turn —
    /// what `journal_steered` falls back to when the turn closed during the
    /// send (the thread's *last* turn at that point is usually the steer's
    /// own user turn, which must never be named as a consumer).
    last_assistant_turn_id: Option<String>,
    /// Dispatched workers, folded from `RunObserved`/`RunCompleted`
    /// observations (the lfd tail). Keyed on run id — the idempotence guard:
    /// a run dispatches once and finishes once, however many times the
    /// observer sees it (live event + reconnect snapshot).
    workers: Vec<WorkerRecord>,
    /// Durable scheduler queue folded from the journal on boot.
    pending_messages: Vec<PendingMessage>,
    /// Every journaled user message by id — requeues restore pending entries
    /// from it (an id alone can't rebuild the text/op/from).
    messages: HashMap<MessageId, PendingMessage>,
    /// Message ids the OPEN turn claimed (`TurnOpened.answers` plus any
    /// mid-turn `TurnSteered.answers`). Requeued if the turn ends without
    /// completing; cleared on any close.
    open_turn_claims: Vec<MessageId>,
    /// Run ids whose `ChannelOpened` is already journaled — the dispatch
    /// notification door's idempotence guard, folded from the journal.
    opened_channel_runs: HashSet<String>,
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
    /// Journal + materialized thread + mind state, behind one lock so their
    /// orders never diverge.
    inner: Mutex<Inner>,
    /// Fans turn frames out to live SSE subscribers: open-turn snapshots as a
    /// turn grows, then the terminal turn under the same id. Frames are
    /// `Arc`-shared so a delta costs one clone (and one serialization) total,
    /// not one per subscriber.
    turn_tx: broadcast::Sender<Arc<TurnFrame>>,
    /// Fans mind-state transitions out to live SSE subscribers (the composer
    /// keys its verb off this).
    state_tx: broadcast::Sender<MindState>,
    /// Fans `MemoryUpdated` summaries out to live SSE subscribers.
    memory_tx: broadcast::Sender<String>,
    /// Fans `MemoryAdded` facts out to live SSE subscribers.
    memory_add_tx: broadcast::Sender<String>,
    /// Durable shared brain (read-only here; the mind curates it deliberately).
    memory: Memory,
    /// Fans resident-directed ops out to the resident's `/events?inbox=true`
    /// subscription and the supervisor. Liveness only — the journal's pending
    /// fold is the durable queue.
    inbox_tx: broadcast::Sender<InboxItem>,
    /// Whether a resident has ever been spawned for / attached to this
    /// listener. `/health` serves `mind: null` until then (a dormant channel
    /// has no mind to report on).
    resident_expected: AtomicBool,
    /// The channel family's child channels, materialized on demand. This
    /// server holds the pen for every one of them (single-writer per journal
    /// file, all pens in one process); the primary channel stays in `inner`
    /// so its hot path is untouched.
    children: Mutex<HashMap<String, Arc<ChildChannel>>>,
    /// The family bus: child-channel turn frames, tagged with their channel
    /// name. The primary channel's frames ride the dedicated broadcasts above
    /// (untagged — absent channel means the wave's own).
    family_tx: broadcast::Sender<ChannelFrame>,
}

impl WaveRuntime {
    /// Open the runtime against the wave's journal, replaying it: the thread
    /// cache is rebuilt from the log and turn ids continue from its seq.
    ///
    /// Boot janitor: turns left open by a crash are finalized as `Failed`
    /// (appended to the journal, so the log itself is closed), the messages
    /// those turns had claimed are requeued (`MessagesRequeued` — a crashed
    /// turn never answered them), and a non-idle mind state settles back to
    /// `Idle`.
    ///
    /// # Errors
    /// Journal I/O failure or an unreadable (future-versioned) journal.
    pub fn open(name: String, repo_root: PathBuf) -> anyhow::Result<Arc<Self>> {
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
        let (memory_tx, _) = broadcast::channel(MEMORY_BROADCAST_CAPACITY);
        let (memory_add_tx, _) = broadcast::channel(MEMORY_BROADCAST_CAPACITY);
        let (family_tx, _) = broadcast::channel(FAMILY_BROADCAST_CAPACITY);
        let (inbox_tx, _) = broadcast::channel(INBOX_BROADCAST_CAPACITY);
        let memory = Memory::for_wave(&repo_root, &name);
        Ok(Arc::new(Self {
            channel_name: wave_channel_name(&name),
            name,
            repo_root,
            inner: Mutex::new(Inner {
                journal,
                thread: fold.turns,
                open_turn: None,
                open_usage: Usage::empty(),
                open_text_items: 0,
                drop_deltas_until_opened: false,
                state,
                thread_id: fold.thread_id,
                last_assistant_turn_id,
                workers,
                pending_messages: fold.pending_messages,
                messages: fold.messages,
                open_turn_claims: Vec::new(),
                opened_channel_runs: fold.opened_channel_runs,
                memory_adds: fold.memory_adds,
            }),
            turn_tx,
            state_tx,
            memory_tx,
            memory_add_tx,
            memory,
            inbox_tx,
            resident_expected: AtomicBool::new(false),
            children: Mutex::new(HashMap::new()),
            family_tx,
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
        snapshot_locked(&self.inner())
    }

    /// The last `limit` turns (open turn included, newest last), cloned
    /// inside the lock — a `/conversation?limit=N` tail never clones the
    /// whole thread. `None` serves everything.
    pub fn thread_tail(&self, limit: Option<usize>) -> Vec<ChatTurn> {
        let inner = self.inner();
        let open_count = usize::from(inner.open_turn.is_some());
        let total = inner.thread.len() + open_count;
        let take = limit.unwrap_or(total).min(total);
        let take_open = take.min(open_count);
        let take_thread = take - take_open;
        let mut turns = inner.thread[inner.thread.len() - take_thread..].to_vec();
        if take_open == 1 {
            turns.extend(inner.open_turn.clone());
        }
        turns
    }

    /// Thread length (open turn included) without cloning a single turn —
    /// `/health`'s counter.
    pub fn thread_len(&self) -> usize {
        let inner = self.inner();
        inner.thread.len() + usize::from(inner.open_turn.is_some())
    }

    /// Current mind state, for `/health` and the composer.
    pub fn mind_state(&self) -> MindState {
        self.inner().state.clone()
    }

    /// The last journaled vendor thread id, if any — the mind's resume handle.
    pub fn last_thread_id(&self) -> Option<String> {
        self.inner().thread_id.clone()
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

    /// Live mind-state transitions (no snapshot).
    pub fn subscribe_states(&self) -> broadcast::Receiver<MindState> {
        self.state_tx.subscribe()
    }

    /// Live turn frames (no snapshot).
    pub fn subscribe_turns(&self) -> broadcast::Receiver<Arc<TurnFrame>> {
        self.turn_tx.subscribe()
    }

    /// Whether a resident has ever been spawned for / attached to this
    /// listener (see `/health`'s `mind` field).
    pub fn resident_expected(&self) -> bool {
        self.resident_expected.load(Ordering::Relaxed)
    }

    pub fn set_resident_expected(&self) {
        self.resident_expected.store(true, Ordering::Relaxed);
    }

    // -- Worker observations (the lfd tail's write surface) --
    //
    // These are OBSERVATIONS, not commands: the server tails lfd's event
    // stream and records confirmed facts. Both appends are idempotent keyed
    // on run id, so a live event plus a reconnect snapshot never journals a
    // worker twice.

    /// Journal a `RunObserved` observation. Returns false (and appends
    /// nothing) when the run is already known.
    pub fn journal_run_observed(
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
        inner.journal.append(|_| EventKind::RunObserved {
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

    /// Journal a `RunCompleted` observation and commit its thread-visible
    /// turn on the PRIMARY channel ("run <id> completed/failed · <summary>",
    /// broadcast live) — a worker that died without reporting still ends
    /// visibly, failure summary on the wire. Returns false (and appends
    /// nothing) when the run was never dispatched or already finished.
    pub fn journal_run_completed(
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
        let event = inner.journal.append(|_| EventKind::RunCompleted {
            run_id: run_id.to_string(),
            outcome,
            summary: summary.to_string(),
        });
        let turn = run_completed_turn(&event, run_id, outcome, summary);
        self.commit_locked(&mut inner, turn);
        true
    }

    /// Whether a `RunObserved` is already journaled for `run_id` —
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

    // -- Channel family (this server holds every child channel's pen) --
    //
    // The primary (wave) channel lives in `inner`, exactly as before —
    // mind-attached, hot path untouched. Child channels are pure streams
    // (no mind, no memory), materialized on demand from their worktree
    // journals and folded separately; the family view folds upward through
    // the tagged `family_tx` bus. Consumption markers never cross journals.

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

    /// Materialize (or fetch) a child channel by name. `Ok(None)` when the
    /// channel's worktree is gone — a landed/deleted work line's channel just
    /// ends (its journal died with the tree; the flagged persistent archive,
    /// `~/.lf/journal/<repo>/<worktree>`, is unbuilt). Errors on a name
    /// outside this wave's family, or journal I/O.
    pub fn child_channel(&self, name: &str) -> anyhow::Result<Option<Arc<ChildChannel>>> {
        if channel_role(&self.name, name) != Some(ChannelRole::Child) {
            anyhow::bail!("channel '{name}' is not a child of wave '{}'", self.name);
        }
        let mut children = self.children.lock().expect("children lock poisoned");
        if let Some(existing) = children.get(name) {
            if existing.alive() {
                return Ok(Some(existing.clone()));
            }
            // The worktree vanished under an open pen: the channel ended.
            children.remove(name);
            return Ok(None);
        }
        let Some(channel) = ChildChannel::open(&self.repo_root, name)? else {
            return Ok(None);
        };
        let channel = Arc::new(channel);
        children.insert(name.to_string(), channel.clone());
        Ok(Some(channel))
    }

    /// Deliver one op to a named channel of this family. The wave's own name
    /// routes through the primary path ([`WaveRuntime::deliver`], byte
    /// identical to an unaddressed delivery); a child name journals in that
    /// channel's worktree journal and broadcasts tagged. `Ok(None)` = nothing
    /// appended (a bare interrupt).
    ///
    /// Fold-upward doctrine: a `say` on a child channel is a worker report —
    /// the mind must hear it. The child journal keeps its record, AND the
    /// same speech lands on the primary channel attributed to the work line
    /// ("[goals.148e] landed PR #42"), queued for the mind like any input;
    /// consumption is marked parent-side (it entered the parent's pending).
    pub fn deliver_to_channel(
        &self,
        channel: &str,
        op: MessageOp,
        text: String,
        from: Option<Attribution>,
    ) -> anyhow::Result<Option<ChatTurn>> {
        if self.is_primary(channel) {
            return Ok(self.deliver(op, text, from));
        }
        let Some(child) = self.child_channel(channel)? else {
            anyhow::bail!(
                "channel '{channel}' has no live worktree — the work line is gone or was never opened"
            );
        };
        let turn = child.deliver(&self.family_tx, op, text.clone(), from.clone())?;
        if turn.is_some() && op == MessageOp::Say {
            // The byline is the CHANNEL — which work line spoke — richer than
            // the say's own label; the sender's session id rides along.
            let session_id = from.and_then(|from| from.session_id);
            self.deliver_say(
                text,
                Attribution {
                    session_id,
                    label: channel.to_string(),
                },
            );
        }
        Ok(turn)
    }

    /// Journal a `ChannelOpened` fact on the PRIMARY channel (the dispatch
    /// notification door) and commit its thread-visible turn. Idempotent on
    /// `run_id`: a repeated knock returns `None` and appends nothing.
    pub fn journal_channel_opened(&self, name: &str, run_id: &str) -> Option<ChatTurn> {
        let mut inner = self.inner();
        if inner.opened_channel_runs.contains(run_id) {
            return None;
        }
        inner.opened_channel_runs.insert(run_id.to_string());
        let event = inner.journal.append(|_| EventKind::ChannelOpened {
            name: name.to_string(),
            run_id: run_id.to_string(),
        });
        let turn = channel_opened_turn(&event, name);
        Some(self.commit_locked(&mut inner, turn))
    }

    /// Subscribe to the family bus and snapshot every live child channel
    /// matching `prefix` (channels discovered on disk are materialized —
    /// this server holds their pens). Bus first, snapshots second: a frame
    /// can arrive twice across the boundary, never be missed — turn frames
    /// are upserts by (channel, id), so duplicates are harmless by contract.
    pub fn subscribe_children(
        &self,
        prefix: &str,
    ) -> (
        Vec<(String, Vec<ChatTurn>)>,
        broadcast::Receiver<ChannelFrame>,
    ) {
        self.sweep_dead_children();
        let rx = self.family_tx.subscribe();
        let mut snapshots = Vec::new();
        for name in scan_child_channels(&self.repo_root, &self.channel_name) {
            if !matches_prefix(&name, prefix) {
                continue;
            }
            match self.child_channel(&name) {
                Ok(Some(channel)) => snapshots.push((name, channel.thread_snapshot())),
                Ok(None) => {} // vanished between the scan and the open
                Err(err) => {
                    tracing::warn!(channel = name, error = %err, "child channel failed to open");
                }
            }
        }
        (snapshots, rx)
    }

    /// Subscribe to the family bus and snapshot exactly ONE child channel —
    /// the strict `?channel=` scope, no descendants (a `?prefix=` subtree
    /// goes through [`WaveRuntime::subscribe_children`]). Bus first, snapshot
    /// second, same duplicate-not-missed contract. The snapshot is `None`
    /// when the channel has no live worktree yet — the subscription still
    /// carries later frames if the channel opens.
    pub fn subscribe_child(
        &self,
        name: &str,
    ) -> (Option<Vec<ChatTurn>>, broadcast::Receiver<ChannelFrame>) {
        self.sweep_dead_children();
        let rx = self.family_tx.subscribe();
        let snapshot = match self.child_channel(name) {
            Ok(Some(channel)) => Some(channel.thread_snapshot()),
            Ok(None) => None,
            Err(err) => {
                tracing::warn!(channel = name, error = %err, "child channel failed to open");
                None
            }
        };
        (snapshot, rx)
    }

    /// Drop dead child channels (worktree gone): a lingering entry pins the
    /// journal's file handle until removed. Runs on every family subscribe
    /// and on the store observer's poll cadence.
    pub fn sweep_dead_children(&self) {
        self.children
            .lock()
            .expect("children lock poisoned")
            .retain(|_, channel| channel.alive());
    }

    // -- Memory (the server holds MEMORY.md's pen) --
    //
    // Both writes go to the ORIGIN repo's wave/<name>/MEMORY.md (the runtime
    // opens against the main repo root — the file seeds read) and journal
    // under the same lock as every other append, so the journal order and the
    // file's history agree. Nothing else writes the file while a server is
    // live.

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

    /// Append one curated fact as a Markdown bullet and journal `MemoryAdded`.
    ///
    /// # Errors
    /// File I/O only.
    pub fn append_memory(&self, fact: &str, _summary: &str) -> std::io::Result<()> {
        let mut inner = self.inner();
        let mut content = self.memory.read();
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(&format!("- {fact}\n"));
        self.memory.write(&content)?;
        inner.journal.append(|_| EventKind::MemoryAdded {
            fact: fact.to_string(),
        });
        inner.memory_adds.push(fact.to_string());
        let _ = self.memory_add_tx.send(fact.to_string());
        Ok(())
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
            memory_rx: self.memory_tx.subscribe(),
            memory_adds: inner.memory_adds.clone(),
            memory_add_rx: self.memory_add_tx.subscribe(),
            pending: inner.pending_messages.clone(),
            inbox_rx: self.inbox_tx.subscribe(),
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

    /// `Turning → Interrupting` for the open turn (the resident reported a
    /// cancel in flight). Returns whether the transition applied — false when
    /// no turn is live.
    fn begin_interrupt(&self, reason: &str) -> bool {
        let mut inner = self.inner();
        let MindState::Turning { turn_id } = inner.state.clone() else {
            return false;
        };
        self.transition_locked(&mut inner, MindState::Interrupting { turn_id }, reason)
    }

    /// Janitor: finalize the open turn without a resident terminal delta —
    /// the interrupt deadline expired with the resident silent, or the
    /// resident process died mid-turn. Journals `TurnFinished`, requeues what
    /// the turn had claimed (it never answered it), commits and broadcasts
    /// the turn as accumulated so far, settles the mind to `Idle`, and arms
    /// the drop guard: late wire deltas for the closed turn are ignored until
    /// the next `TurnOpened`. Returns whether there was an open turn to
    /// finalize.
    pub fn force_finalize_open_turn(&self, status: Lifecycle, reason: &str) -> bool {
        let mut inner = self.inner();
        let Some(mut turn) = inner.open_turn.take() else {
            return false;
        };
        inner.drop_deltas_until_opened = true;
        inner.open_usage = Usage::empty();
        inner.open_text_items = 0;
        inner.journal.append(|_| EventKind::TurnFinished {
            turn_id: turn.id.clone(),
            status,
            usage: Usage::empty(),
        });
        let claims = std::mem::take(&mut inner.open_turn_claims);
        if status != Lifecycle::Completed {
            self.requeue_locked(&mut inner, &claims);
        }
        turn.status = status;
        self.transition_locked(&mut inner, MindState::Idle, reason);
        self.commit_locked(&mut inner, turn);
        true
    }

    /// Return claimed-but-unanswered messages to the durable queue: journal
    /// `MessagesRequeued` and restore the pending fold, exactly what the fold
    /// replays. No live inbox re-broadcast — redelivery is the pending
    /// replay's job (the resident's next subscription), never a silent
    /// double-send to a mind that may still hold its own copy.
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
        if turn.role == ChatRole::Assistant {
            inner.last_assistant_turn_id = Some(turn.id.clone());
        }
        inner.thread.push(turn.clone());
        // A send error just means no live subscribers — the store has it.
        let _ = self.turn_tx.send(TurnFrame::share(turn.clone()));
        turn
    }

    /// Deliver one resident-directed op, uninterpreted by the caller: the
    /// door validates SHAPE (op names, text/`from` presence) and hands the op
    /// here; what an op *means* lives in this runtime and the mind's
    /// scheduler. A bare interrupt (empty text) journals nothing and appends
    /// no turn — `None`; every other delivery journals a `UserMessage`,
    /// commits the user turn, and queues for the mind.
    pub fn deliver(
        &self,
        op: MessageOp,
        text: String,
        from: Option<Attribution>,
    ) -> Option<ChatTurn> {
        if op == MessageOp::Interrupt && text.trim().is_empty() {
            self.deliver_interrupt();
            return None;
        }
        Some(self.deliver_message(text, op, from))
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
    /// resident reports the `MindState` transition when it fires the cancel;
    /// an interrupt while idle is a no-op by design.
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

    // -- The resident wire fold (same lock discipline as everything above) --
    //
    // Every content delta (opened / text / item) re-broadcasts the open-turn
    // snapshot under the same id. No debounce: deltas are item-granular from
    // the vendor stream (one per completed item, not per token), so the
    // natural rate is well under any flood threshold, and a suppressed
    // trailing frame would leave subscribers stale through a long tool call.
    // A throttle earns its place with the part-grained wire, not before.

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
            ResidentDelta::TurnFinished { status, cost_usd } => {
                self.resident_turn_finished(status, cost_usd)
            }
            ResidentDelta::TurnSteered { answers } => self.resident_turn_steered(answers),
            ResidentDelta::MessagesRequeued { ids } => self.resident_requeue(ids),
            ResidentDelta::MindState { to, reason } => match to {
                ResidentStateTo::Interrupting => {
                    if !self.begin_interrupt(&reason) {
                        tracing::warn!(reason, "resident reported Interrupting with no live turn");
                    }
                }
                ResidentStateTo::Failed => {
                    self.transition(
                        MindState::Failed {
                            reason: reason.clone(),
                        },
                        &reason,
                    );
                }
            },
            ResidentDelta::ThreadStarted { vendor, thread_id } => {
                self.journal_thread_started(&vendor, &thread_id);
            }
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
        if let Some(mut stale) = inner.open_turn.take() {
            tracing::warn!(
                turn_id = stale.id,
                "TurnOpened over an open turn; closing the stale turn as failed"
            );
            let stale_usage = std::mem::replace(&mut inner.open_usage, Usage::empty());
            inner.journal.append(|_| EventKind::TurnFinished {
                turn_id: stale.id.clone(),
                status: Lifecycle::Failed,
                usage: stale_usage,
            });
            let claims = std::mem::take(&mut inner.open_turn_claims);
            self.requeue_locked(&mut inner, &claims);
            stale.status = Lifecycle::Failed;
            self.transition_locked(&mut inner, MindState::Idle, "stale open turn closed");
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
        inner.open_turn_claims = answers.clone();
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
        let _ = self.turn_tx.send(TurnFrame::share(open.clone()));
        inner.open_turn = Some(open);
        inner.open_usage = Usage::empty();
        inner.open_text_items = 0;
    }

    fn resident_turn_text(&self, text: String) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            return;
        }
        if inner.open_turn.is_none() {
            tracing::warn!("text delta with no open turn; dropped");
            return;
        }
        let item = ConversationItem::Message {
            id: format!("text-{}", inner.open_text_items),
            text,
            phase: None,
        };
        inner.open_text_items += 1;
        self.append_turn_item_locked(&mut inner, item);
    }

    fn resident_turn_item(&self, item: ConversationItem) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            return;
        }
        if inner.open_turn.is_none() {
            tracing::warn!("item delta with no open turn; dropped");
            return;
        }
        self.append_turn_item_locked(&mut inner, item);
    }

    /// Journal a `TurnItem` for the open turn, grow the open-turn snapshot
    /// through the one shared rule (`ChatTurn::absorb_item` — the same call
    /// the journal fold makes), and re-broadcast it so live subscribers watch
    /// the turn in progress.
    fn append_turn_item_locked(&self, inner: &mut Inner, item: ConversationItem) {
        let open = inner.open_turn.as_mut().expect("checked by callers");
        let turn_id = open.id.clone();
        open.absorb_item(item.clone());
        let frame = TurnFrame::share(open.clone());
        inner
            .journal
            .append(|_| EventKind::TurnItem { turn_id, item });
        let _ = self.turn_tx.send(frame);
    }

    fn resident_turn_usage(
        &self,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
    ) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened || inner.open_turn.is_none() {
            return;
        }
        inner.open_usage.input_tokens = add_opt(inner.open_usage.input_tokens, input_tokens);
        inner.open_usage.output_tokens = add_opt(inner.open_usage.output_tokens, output_tokens);
        inner.open_usage.cache_read_tokens =
            add_opt(inner.open_usage.cache_read_tokens, cache_read_tokens);
    }

    fn resident_turn_finished(&self, status: Lifecycle, cost_usd: Option<f64>) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            tracing::debug!("late TurnFinished after a force-finalize; dropped");
            return;
        }
        let Some(mut turn) = inner.open_turn.take() else {
            tracing::warn!("TurnFinished with no open turn; dropped");
            return;
        };
        let mut usage = std::mem::replace(&mut inner.open_usage, Usage::empty());
        usage.cost_usd = cost_usd;
        inner.open_text_items = 0;
        inner.journal.append(|_| EventKind::TurnFinished {
            turn_id: turn.id.clone(),
            status,
            usage,
        });
        // Any non-Completed end requeues what the turn claimed: a failed or
        // interrupted turn never answered its messages.
        let claims = std::mem::take(&mut inner.open_turn_claims);
        if status != Lifecycle::Completed {
            self.requeue_locked(&mut inner, &claims);
        }
        turn.status = status;
        self.transition_locked(&mut inner, MindState::Idle, "turn finalized");
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
            MindState::Turning { turn_id } | MindState::Interrupting { turn_id } => (turn_id, true),
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
            inner.open_turn_claims.extend(answers.iter().cloned());
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
        inner.open_turn_claims.retain(|claim| !ids.contains(claim));
        self.requeue_locked(&mut inner, &ids);
    }
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
fn snapshot_locked(inner: &Inner) -> Vec<ChatTurn> {
    let mut turns = inner.thread.clone();
    turns.extend(inner.open_turn.clone());
    turns
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
        }
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
        // the mind curates it deliberately.
        assert_eq!(rt.memory().read(), "");
    }

    #[test]
    fn deliver_user_message_appends_user_turn_and_broadcasts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
        let turn = rt.deliver_user_message("how goes it?".into(), MessageOp::Message);
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
    fn deliver_say_journals_attribution_and_queues_for_the_mind() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
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
        let rt2 = open_runtime(tmp.path());
        let pending = rt2.pending_messages();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].from, Some(from));
        assert_eq!(rt2.thread_snapshot()[0].from.as_deref(), Some("worker"));
    }

    #[test]
    fn update_memory_writes_the_origin_file_and_journals() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
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
        assert_eq!(summaries, vec!["fold is truth"]);
        let facts: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::MemoryAdded { fact } => Some(fact.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(facts, vec!["bullets append"]);
    }

    #[test]
    fn subscription_replays_full_memory_facts_in_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let long_fact = "workers report via lf chat with the full useful detail";

        rt.append_memory(long_fact, "workers report")
            .expect("append");
        rt.append_memory("second fact", "second").expect("append");

        let sub = rt.subscribe_with_snapshot();
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
    fn memory_externalization_resets_add_replay_buffer() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());

        rt.append_memory("first", "first").expect("append");
        rt.append_memory("second", "second").expect("append");
        rt.update_memory("# Ship\n\ncompiled\n", "compiled")
            .expect("update");
        rt.append_memory("third", "third").expect("append");

        let sub = rt.subscribe_with_snapshot();
        assert_eq!(sub.memory_adds, vec!["third".to_string()]);
        assert_eq!(
            rt.memory().read(),
            "# Ship\n\ncompiled\n- third\n",
            "the compiled checkpoint still carries the file state"
        );
    }

    #[test]
    fn memory_add_replay_buffer_rebuilds_from_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let rt = open_runtime(tmp.path());
            rt.append_memory("first", "first").expect("append");
            rt.append_memory("second", "second").expect("append");
            rt.update_memory("# Ship\n\ncompiled\n", "compiled")
                .expect("update");
            rt.append_memory("third", "third").expect("append");
        }

        let rt = open_runtime(tmp.path());
        let sub = rt.subscribe_with_snapshot();
        assert_eq!(sub.memory_adds, vec!["third".to_string()]);
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
    fn thread_id_round_trips_through_the_journal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let rt = open_runtime(tmp.path());
            assert_eq!(rt.last_thread_id(), None);
            rt.apply_resident_delta(ResidentDelta::ThreadStarted {
                vendor: "codex".into(),
                thread_id: "thread-abc".into(),
            });
            assert_eq!(rt.last_thread_id().as_deref(), Some("thread-abc"));
        }
        // A restarted runtime folds the resume handle back out of the log.
        let rt = open_runtime(tmp.path());
        assert_eq!(rt.last_thread_id().as_deref(), Some("thread-abc"));
    }

    #[test]
    fn illegal_transition_is_refused_and_leaves_state_alone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
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
    fn resident_deltas_journal_a_turn_and_commit_it() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());

        rt.apply_resident_delta(d_opened(&[]));
        assert_eq!(
            rt.mind_state().name(),
            "turning",
            "mid-turn the mind is Turning"
        );
        rt.apply_resident_delta(d_text("hello"));
        rt.apply_resident_delta(d_tool());
        rt.apply_resident_delta(d_usage(10, 4));
        rt.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: Some(0.02),
        });

        assert_eq!(rt.mind_state(), MindState::Idle, "back to idle after turn");
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

    /// The consumption declaration is the RESIDENT's, validated by the
    /// listener: known pending ids are claimed and journaled in
    /// `TurnStarted.answers`; unknown or already-consumed ids are dropped.
    #[test]
    fn turn_opened_answers_are_validated_against_the_pending_fold() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let m1 = rt.deliver_user_message("first".into(), MessageOp::Message);
        let m2 = rt.deliver_user_message("second".into(), MessageOp::Message);
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
        let sub = rt.subscribe_with_snapshot();
        assert!(sub.turns.is_empty());
        assert_eq!(sub.state, MindState::Idle);
        let mut frames = sub.turn_rx;
        let mut states = sub.state_rx;

        // The turn opens empty and running, then the text lands in a second
        // frame under the same id.
        rt.apply_resident_delta(d_opened(&[]));
        let opened = frames.try_recv().expect("opened frame").turn.clone();
        assert_eq!(opened.status, Lifecycle::Running);
        assert_eq!(opened.text, "");
        rt.apply_resident_delta(d_text("thinking"));
        let grown = frames.try_recv().expect("text frame");
        assert_eq!(grown.turn.id, opened.id);
        assert_eq!(grown.turn.text, "thinking");
        assert_eq!(grown.turn.status, Lifecycle::Running);

        // Mid-turn, the open turn rides the snapshot after the thread.
        let mid = rt.thread_snapshot();
        assert_eq!(mid.len(), 1);
        assert_eq!(mid[0].id, opened.id);
        assert_eq!(mid[0].status, Lifecycle::Running);

        // An item delta grows the same snapshot.
        rt.apply_resident_delta(d_tool());
        let with_item = frames.try_recv().expect("item frame");
        assert_eq!(with_item.turn.id, opened.id);
        assert_eq!(with_item.turn.items.len(), 1);
        assert_eq!(with_item.turn.text, "thinking");

        // Finalization replaces the running turn under the same id.
        rt.apply_resident_delta(d_usage(10, 5));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        let terminal = frames.try_recv().expect("terminal frame");
        assert_eq!(terminal.turn.id, opened.id);
        assert_eq!(terminal.turn.status, Lifecycle::Completed);

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

    /// The listener-side janitor: force-finalize closes the journal and
    /// settles Idle, and the drop guard swallows the resident's late deltas —
    /// including its own eventual TurnFinished — until the next TurnOpened.
    #[test]
    fn force_finalize_closes_the_turn_and_drops_late_deltas() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(d_text("half"));
        rt.apply_resident_delta(ResidentDelta::MindState {
            to: ResidentStateTo::Interrupting,
            reason: "user interrupt".into(),
        });
        assert_eq!(rt.mind_state().name(), "interrupting");

        assert!(rt.force_finalize_open_turn(Lifecycle::Interrupted, "deadline"));
        assert_eq!(rt.mind_state(), MindState::Idle);
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].status, Lifecycle::Interrupted);
        assert_eq!(thread[0].text, "half");

        // The journal is closed: a replay agrees, no open turn survives.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let fold = crate::wave::journal::fold_thread(&events);
        assert!(fold.open.is_empty());
        assert_eq!(fold.turns.last().unwrap().status, Lifecycle::Interrupted);

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
        assert_eq!(rt.mind_state(), MindState::Idle, "no double transition");

        // …and the next TurnOpened clears the guard: life goes on.
        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(d_text("fresh"));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 2);
        assert_eq!(thread[1].text, "fresh");
        assert_eq!(thread[1].status, Lifecycle::Completed);
    }

    /// Fold-upward doctrine: a `say` on a CHILD channel journals there AND
    /// lands on the primary channel's inbox as attributed speech, so the
    /// mind's next turn can answer it. The child's own journal keeps its
    /// record; consumption is parent-side.
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

    /// A sanitized-name wave routes a child delivery by the sanitized channel
    /// and folds it upward with the channel byline — end to end through
    /// `deliver_to_channel`, the door's path.
    #[test]
    fn sanitized_wave_delivers_and_folds_child_channels() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(crate::wave::channel::child_worktree_path(
            &origin,
            "web-ui.148e",
        ))
        .unwrap();
        let rt = WaveRuntime::open("web/ui".into(), origin).expect("open runtime");
        assert_eq!(rt.channel_name(), "web-ui");
        assert!(rt.in_family("web-ui.148e"));
        rt.deliver_to_channel(
            "web-ui.148e",
            MessageOp::Say,
            "child report".into(),
            Some(Attribution {
                session_id: None,
                label: "worker".into(),
            }),
        )
        .expect("deliver")
        .expect("appends");
        // Forwarded to the primary, bylined with the channel.
        assert_eq!(rt.thread_snapshot()[0].from.as_deref(), Some("web-ui.148e"));
    }

    #[test]
    fn child_say_forwards_to_the_primary_inbox_as_attributed_speech() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(crate::wave::channel::child_worktree_path(
            &origin,
            "ship.148e",
        ))
        .unwrap();
        let rt = WaveRuntime::open("ship".into(), origin.clone()).expect("open runtime");
        let mut inbox = rt.subscribe_inbox();

        rt.deliver_to_channel(
            "ship.148e",
            MessageOp::Say,
            "landed PR #42".into(),
            Some(Attribution {
                session_id: Some("sess-9".into()),
                label: "worker".into(),
            }),
        )
        .expect("deliver")
        .expect("say appends");

        // The child journal kept its own row.
        let child = rt.child_channel("ship.148e").unwrap().expect("child live");
        assert_eq!(child.thread_snapshot().len(), 1);
        assert_eq!(child.thread_snapshot()[0].text, "landed PR #42");

        // The primary inbox received the forwarded speech, bylined with the
        // CHANNEL (which work line spoke), session id carried through.
        let InboxItem::Message(msg) = inbox.try_recv().expect("forwarded inbox item") else {
            panic!("expected a message");
        };
        assert_eq!(msg.op, MessageOp::Say);
        assert_eq!(msg.text, "landed PR #42");
        assert_eq!(
            msg.from.as_ref().map(|f| f.label.as_str()),
            Some("ship.148e")
        );
        assert_eq!(
            msg.from.as_ref().and_then(|f| f.session_id.as_deref()),
            Some("sess-9")
        );

        // It is on the PRIMARY thread and pending queue — a boundary turn
        // answering it drains the queue.
        assert_eq!(rt.thread_snapshot().len(), 1);
        assert_eq!(rt.pending_messages().len(), 1);
        let claimed = msg_id(&rt.thread_snapshot()[0]);
        rt.apply_resident_delta(d_opened(&[&claimed]));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        assert!(
            rt.pending_messages().is_empty(),
            "the boundary turn answered the forwarded report"
        );
    }

    /// A non-say op on a child channel (a plain message) journals in the
    /// child only — no fold-upward: forwarding is the report doctrine, not
    /// every child utterance.
    #[test]
    fn child_non_say_does_not_forward() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(crate::wave::channel::child_worktree_path(&origin, "ship.a"))
            .unwrap();
        let rt = WaveRuntime::open("ship".into(), origin).expect("open runtime");
        rt.deliver_to_channel("ship.a", MessageOp::Message, "note to self".into(), None)
            .expect("deliver")
            .expect("appends");
        assert!(
            rt.thread_snapshot().is_empty(),
            "a plain child message never reaches the primary thread"
        );
        assert!(rt.pending_messages().is_empty());
    }

    /// `RunCompleted` commits a thread-visible turn on the primary channel —
    /// the died-silently backstop: a worker that never reported still ends
    /// visibly, the failure summary on the wire.
    #[test]
    fn run_completed_commits_a_thread_visible_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        rt.journal_run_observed("run-abcdef12", "sess-1", "implement", "wire it");
        let mut frames = rt.subscribe_turns();

        assert!(rt.journal_run_completed(
            "run-abcdef12",
            WorkerOutcome::Failed,
            "session failed; error: boom"
        ));
        let turn = &rt.thread_snapshot()[0];
        assert!(
            turn.text.contains("run run-abcd"),
            "short id: {}",
            turn.text
        );
        assert!(turn.text.contains("failed"));
        assert!(turn.text.contains("boom"), "failure summary on the turn");
        assert_eq!(turn.from.as_deref(), Some("observer"));

        // It rode the live turn broadcast.
        let frame = frames.try_recv().expect("run-completed frame");
        assert_eq!(frame.turn.id, turn.id);

        // Never queued for the mind (only UserMessage rows feed pending).
        assert!(rt.pending_messages().is_empty());

        // A restart folds the same turn back out of the journal, once.
        let rt2 = open_runtime(tmp.path());
        let completed: Vec<_> = rt2
            .thread_snapshot()
            .into_iter()
            .filter(|t| t.from.as_deref() == Some("observer"))
            .collect();
        assert_eq!(completed.len(), 1);
    }

    /// Claimed-but-unanswered messages are requeued when a turn ends without
    /// completing: a Failed TurnFinished returns its claims to pending, and a
    /// restart re-delivers them (never lost). A Completed turn keeps them
    /// consumed.
    #[test]
    fn failed_turn_requeues_its_claimed_messages() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let m1 = rt.deliver_user_message("do the thing".into(), MessageOp::Message);

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
        let m2 = rt3.deliver_user_message("second".into(), MessageOp::Message);
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
            let m = rt.deliver_user_message("answer me".into(), MessageOp::Message);
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
        let m = rt.deliver_user_message("steer".into(), MessageOp::Steer);
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
    /// TurnOpened is dropped, its would-be claims stay pending, and the mind
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
        let m = rt.deliver_user_message("go".into(), MessageOp::Message);

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

    #[test]
    fn worker_observations_are_idempotent_and_survive_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let rt = open_runtime(tmp.path());
            assert!(rt.journal_run_observed("run-1", "sess-1", "implement", "wire it"));
            // Same run seen again (reconnect snapshot): guarded, not journaled.
            assert!(!rt.journal_run_observed("run-1", "sess-1", "implement", "wire it"));
            assert_eq!(rt.in_flight_workers().len(), 1);
            // A finish for a run never dispatched is refused.
            assert!(!rt.journal_run_completed("run-9", WorkerOutcome::Failed, "?"));
            assert!(rt.journal_run_completed("run-1", WorkerOutcome::Completed, "pr landed"));
            assert!(!rt.journal_run_completed(
                "run-1",
                WorkerOutcome::Completed,
                "pr landed again"
            ));
            assert!(rt.in_flight_workers().is_empty());
        }

        // A restarted runtime folds the same guard state back out of the log:
        // the finished run stays finished, a new run dispatches normally.
        let rt = open_runtime(tmp.path());
        assert!(!rt.journal_run_observed("run-1", "sess-1", "implement", "wire it"));
        assert!(rt.journal_run_observed("run-2", "sess-2", "design", "sketch it"));
        assert_eq!(rt.in_flight_workers().len(), 1);
        assert_eq!(rt.in_flight_workers()[0].run_id, "run-2");

        // Exactly one RunObserved per run in the journal itself.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let dispatched: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::RunObserved { run_id, .. } => Some(run_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(dispatched, vec!["run-1", "run-2"]);
        let finished: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::RunCompleted { run_id, .. } => Some(run_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(finished, vec!["run-1"]);
    }

    /// Steer consumption over the wire: the live turn answers a steered
    /// message; the boundary race (turn closed during the send) falls back to
    /// the last assistant turn; nothing is claimed with no assistant turn
    /// anywhere.
    #[test]
    fn turn_steered_consumes_against_live_or_just_closed_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());

        let m1 = rt.deliver_user_message("steer me".into(), MessageOp::Steer);
        let m2 = rt.deliver_user_message("me too".into(), MessageOp::Steer);

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
        let steer = rt.deliver_user_message("steer me".into(), MessageOp::Steer);
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

    /// The channel family: parent + two children, threads independent,
    /// consumption local. Delivering into a child journals in THAT worktree's
    /// journal, never the parent's; the parent's pending queue (consumption
    /// machinery) sees only its own messages.
    #[test]
    fn family_folds_keep_channels_independent_and_consumption_local() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        for channel in ["ship.a", "ship.b"] {
            std::fs::create_dir_all(crate::wave::channel::child_worktree_path(&origin, channel))
                .unwrap();
        }
        let rt = WaveRuntime::open("ship".into(), origin.clone()).expect("open runtime");

        rt.deliver_user_message("to the wave".into(), MessageOp::Message);
        rt.deliver_to_channel(
            "ship.a",
            MessageOp::Say,
            "to a".into(),
            Some(Attribution {
                session_id: None,
                label: "worker".into(),
            }),
        )
        .expect("deliver a")
        .expect("appended");
        rt.deliver_to_channel("ship.b", MessageOp::Message, "to b".into(), None)
            .expect("deliver b")
            .expect("appended");

        // The parent holds its own turn plus the child `say` folded up (the
        // report doctrine); the child `message` to ship.b does NOT forward.
        let wave = rt.thread_snapshot();
        assert_eq!(wave.len(), 2);
        assert_eq!(wave[0].text, "to the wave");
        assert_eq!(
            wave[1].text, "to a",
            "the say folded up; the message did not"
        );
        let a = rt.child_channel("ship.a").unwrap().expect("a live");
        let b = rt.child_channel("ship.b").unwrap().expect("b live");
        assert_eq!(a.thread_snapshot().len(), 1);
        assert_eq!(a.thread_snapshot()[0].text, "to a");
        assert_eq!(a.thread_snapshot()[0].from.as_deref(), Some("worker"));
        assert_eq!(b.thread_snapshot().len(), 1);
        assert_eq!(b.thread_snapshot()[0].text, "to b");

        // On disk: three separate journals, each with one UserMessage.
        for (root, channel) in [
            (origin.clone(), "ship".to_string()),
            (
                crate::wave::channel::child_worktree_path(&origin, "ship.a"),
                "ship.a".to_string(),
            ),
            (
                crate::wave::channel::child_worktree_path(&origin, "ship.b"),
                "ship.b".to_string(),
            ),
        ] {
            let events = crate::wave::journal::read_events(&journal_path(&root, &channel));
            let messages = events
                .iter()
                .filter(|e| matches!(e.kind, EventKind::UserMessage { .. }))
                .count();
            // The parent carries its own message plus the child say's forward;
            // each child carries exactly its own.
            let expected = if channel == "ship" { 2 } else { 1 };
            assert_eq!(messages, expected, "message count in {channel}'s journal");
        }

        // Consumption: the parent's queue holds its own message AND the
        // forwarded report (so the mind answers it) — but NOT the plain
        // `message` to ship.b, which never folds up.
        let rt2 = WaveRuntime::open("ship".into(), origin.clone()).expect("reopen");
        let pending = rt2.pending_messages();
        assert_eq!(
            pending.len(),
            2,
            "the wave message and the forwarded report"
        );
        let texts: Vec<_> = pending.iter().map(|m| m.text.as_str()).collect();
        assert!(texts.contains(&"to the wave") && texts.contains(&"to a"));
        assert!(
            !texts.contains(&"to b"),
            "the child message never folded up"
        );

        // Family membership is enforced; a foreign name is refused.
        assert!(rt2
            .deliver_to_channel("other.x", MessageOp::Message, "?".into(), None)
            .is_err());
    }

    /// A vanished worktree ends its channel: deliveries refuse, folds skip
    /// it, nothing panics — and the family's other channels are untouched.
    #[test]
    fn vanished_worktree_ends_the_channel_without_panic() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let doomed = crate::wave::channel::child_worktree_path(&origin, "ship.gone");
        std::fs::create_dir_all(&doomed).unwrap();
        let rt = WaveRuntime::open("ship".into(), origin).expect("open runtime");

        rt.deliver_to_channel("ship.gone", MessageOp::Message, "hi".into(), None)
            .expect("deliver while alive")
            .expect("appended");

        // The work line lands/deletes: the tree (journal with it) is gone.
        std::fs::remove_dir_all(&doomed).unwrap();
        assert!(
            rt.deliver_to_channel("ship.gone", MessageOp::Message, "again".into(), None)
                .is_err(),
            "speech to an ended channel refuses"
        );
        assert!(
            rt.child_channel("ship.gone").unwrap().is_none(),
            "the ended channel is no longer materialized"
        );
        let (snapshots, _rx) = rt.subscribe_children("ship");
        assert!(snapshots.is_empty(), "family fold skips the ended channel");
        // A never-opened channel behaves the same (no dir, no resurrection).
        assert!(rt.child_channel("ship.never").unwrap().is_none());
    }

    /// `ChannelOpened` (the dispatch notification): the thread shows the
    /// opening, once per run id, and the guard survives restart.
    #[test]
    fn journal_channel_opened_is_idempotent_and_thread_visible() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let turn = rt
            .journal_channel_opened("ship.148e0e02", "run-1")
            .expect("first knock journals");
        assert_eq!(turn.text, "work line ship.148e0e02 opened");
        assert_eq!(turn.from.as_deref(), Some("dispatch"));
        assert!(
            rt.journal_channel_opened("ship.148e0e02", "run-1")
                .is_none(),
            "second knock for the same run appends nothing"
        );
        assert_eq!(rt.thread_snapshot().len(), 1);

        // Restart: the guard folds back out of the journal.
        let rt2 = open_runtime(tmp.path());
        assert!(rt2
            .journal_channel_opened("ship.148e0e02", "run-1")
            .is_none());
        assert_eq!(rt2.thread_snapshot().len(), 1);
        assert_eq!(
            rt2.thread_snapshot()[0].text,
            "work line ship.148e0e02 opened"
        );
    }

    /// A subscription's snapshot carries the pending queue (the resident's
    /// boot replay) and its receiver carries exactly the ops sent after it.
    #[test]
    fn subscription_carries_pending_replay_and_live_inbox() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        rt.deliver_user_message("before".into(), MessageOp::Message);

        let mut sub = rt.subscribe_with_snapshot();
        assert_eq!(sub.pending.len(), 1);
        assert_eq!(sub.pending[0].text, "before");
        assert!(sub.inbox_rx.try_recv().is_err(), "no frames from before");

        rt.deliver_user_message("after".into(), MessageOp::Message);
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
                    rt.deliver_user_message(format!("m-{writer}-{i}"), MessageOp::Message);
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
