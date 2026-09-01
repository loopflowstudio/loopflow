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
//! Three inputs feed the journal: the resident's wire deltas
//! ([`WaveRuntime::apply_resident_delta`] — the old in-process `TurnSink`
//! vocabulary, now arriving over `POST /resident/deltas`), human messages, and
//! typed registry inputs. All appends go through one lock, so journal order,
//! cache order, and broadcast order agree — one writer appends and broadcasts.
//! This module is vendor-free: the harness lives with the resident process,
//! never here.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

use crate::chat::turns::{ChatRole, ChatTurn, TurnDelta};
use crate::chat::types::{ConversationItem, Lifecycle};
use crate::controller::wave::channel::{Author, Message};
use crate::controller::wave::chat::{
    ChatBacking, ChatMessageSource, ConversationEpoch, WaveChatMessage,
};
#[cfg(test)]
use crate::controller::wave::journal::JournalAppendStage;
use crate::controller::wave::journal::{
    fold_thread, journal_path, project_observation_message, promotion_wake_message,
    restore_pending, task_observation_message, ConversationEpochImport, DiscordAttachment,
    DiscordChatBinding, DiscordDelivery, DiscordMessagePart, DiscordMessageSource, EventKind,
    Journal, JournalAppendError, MessageId, MessageOp, PendingMessage,
};
use crate::controller::wave::playhead::{
    now_rfc3339, BodyProvenance, Playhead, PlayheadEvent, PlayheadView, QueuedInvocation,
    StepOutcome,
};
use crate::controller::wave::state::{can_transition, LoopState};
use crate::controller::wave::wire::{ProviderSessionRef, ResidentDelta, ResidentStateTo};
use crate::work::project::ProjectObservation;
use crate::work::task::TaskObservation;
use crate::work::wave::config::read_wave_config;
use crate::work::wave::PromotionWake;

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

/// Capacity of the live inbox broadcast (resident-directed ops → the
/// `/events?inbox=true` frames and the supervisor). The journal is the
/// durable queue; a lagged subscriber resyncs from the pending replay.
const INBOX_BROADCAST_CAPACITY: usize = 256;

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
    /// A journaled user message (`message`, `steer`, or `interrupt`
    /// carrying text — "interrupt & send"), awaiting consumption (named in a
    /// `TurnStarted.answers` or `TurnSteered.answers`).
    Message(PendingMessage),
    /// A typed Task ledger observation awaiting the same durable turn
    /// consumption acknowledgement as a queued message.
    Task(TaskObservation),
    /// A typed Project ledger observation.
    Project(ProjectObservation),
    /// The one-time typed wake derived from this Wave's durable promotion occurrence.
    Promotion {
        parent_wave_id: crate::id::WaveId,
        parent: String,
    },
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
    pub epoch: ConversationEpoch,
    pub turns: Vec<ChatTurn>,
    /// Live turn frames ride as [`TurnBroadcast`] (whole or delta), each an
    /// `Arc`: the broadcast clones once per subscriber, so N subscribers share
    /// one allocation — and one JSON serialization — per frame instead of N.
    pub turn_rx: broadcast::Receiver<TurnBroadcast>,
    pub state: LoopState,
    pub state_rx: broadcast::Receiver<LoopState>,
    pub playhead: Option<PlayheadView>,
    pub playhead_rx: broadcast::Receiver<PlayheadView>,
    /// The pending queue as of the snapshot: typed observation and promotion
    /// inputs not yet named in any `answers` — the resident's boot replay.
    pub pending: Vec<PendingMessage>,
    /// Chat messages the wave has not yet answered as of the snapshot. Chat is
    /// observed, not drained, so it never enters `pending`; the inbox replay
    /// hands these to the observe loop so a message that arrived before the
    /// subscription (or before a restart) still gets a look.
    pub chat_tail: Vec<PendingMessage>,
    pub tasks: HashMap<MessageId, TaskObservation>,
    pub projects: HashMap<MessageId, ProjectObservation>,
    pub(crate) promotions: HashMap<MessageId, PromotionWake>,
    /// Live resident-directed ops sent after the snapshot.
    pub inbox_rx: broadcast::Receiver<InboxItem>,
}

/// Durable Discord state the listener-owned adapter needs to resume.
#[derive(Debug, Clone)]
pub struct DiscordSnapshot {
    pub attachment: Option<DiscordAttachment>,
    pub deliveries: Vec<DiscordDelivery>,
}

#[derive(Debug, Clone, Copy)]
enum DiscordInput {
    Provider,
    Authored(MessageOp),
}

impl DiscordInput {
    fn op(self) -> MessageOp {
        match self {
            Self::Provider => MessageOp::Message,
            Self::Authored(op) => op,
        }
    }
}

/// The assistant turn in progress. `turn` is the snapshot the wire watches
/// grow (status `Running`), re-broadcast on every content delta and committed
/// to `thread` under the same id at finalization; the rest is bookkeeping that
/// only means anything while the turn is open.
#[derive(Debug)]
struct OpenTurn {
    turn: ChatTurn,
    /// Prose fragments so far, for `Message` item ids (`"text-<n>"`).
    text_items: usize,
    /// Message ids this turn claimed (`TurnOpened.answers` plus any mid-turn
    /// `TurnSteered.answers`). Requeued if the turn ends without completing.
    claims: Vec<MessageId>,
    /// Channel message this turn explicitly answers. This is a visible reply
    /// edge, independent of the old scheduler-consumption claims.
    reply_to: Option<MessageId>,
}

/// Everything that must stay mutually consistent: the journal (truth), the
/// thread cache (fold of it), the open turn, the loop state (last transition).
#[derive(Debug)]
struct Inner {
    journal: Journal,
    thread: Vec<ChatTurn>,
    conversation_epochs: Vec<ConversationEpoch>,
    conversation_epoch_turns: HashMap<String, Vec<String>>,
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
    /// Every journaled input by id — requeues restore pending entries from it.
    messages: HashMap<MessageId, PendingMessage>,
    discord: Option<DiscordAttachment>,
    discord_deliveries: HashMap<String, DiscordDelivery>,
    /// Completed assistant turn id → channel message it explicitly answers.
    chat_reply_targets: HashMap<String, MessageId>,
    tasks: HashMap<MessageId, TaskObservation>,
    projects: HashMap<MessageId, ProjectObservation>,
    promotions: HashMap<MessageId, PromotionWake>,
}

/// The whole live state of one running wave server.
#[derive(Debug)]
pub struct WaveRuntime {
    name: String,
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
    /// Fans resident-directed ops out to the resident's `/events?inbox=true`
    /// subscription and the supervisor. Liveness only — the journal's pending
    /// fold is the durable queue.
    inbox_tx: broadcast::Sender<InboxItem>,
    /// Whether a resident has ever been spawned for / attached to this
    /// listener. `/health` serves `loop_state: null` until then (a dormant listener
    /// has no loop to report on).
    resident_expected: AtomicBool,
}

/// A human-authored chat write either commits to the active local epoch or is
/// rejected because its active authority is Discord.
#[derive(Debug, thiserror::Error)]
pub enum ChatWriteError {
    #[error("this Wave chat is backed by Discord")]
    OpenDiscord,
    #[error(transparent)]
    Journal(#[from] JournalAppendError),
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
        Self::open_with_backing(name, repo_root, ChatBacking::Local)
    }

    /// Open the runtime with one boot-atomic conversation authority.
    ///
    /// A backing change starts a new append-only epoch. Reopening with the
    /// same backing resumes the existing epoch instead of inventing a restart
    /// boundary.
    ///
    /// # Errors
    /// Journal I/O failure or an unreadable (future-versioned) journal.
    pub fn open_with_backing(
        name: String,
        repo_root: PathBuf,
        backing: ChatBacking,
    ) -> anyhow::Result<Arc<Self>> {
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

        initialize_conversation_epoch(
            &mut journal,
            &mut fold.conversation_epochs,
            &mut fold.conversation_epoch_turns,
            &fold.turns,
            &fold.discord_turn_bindings,
            backing.clone(),
        );

        let planned_turns = fold
            .discord_deliveries
            .values()
            .map(|delivery| delivery.turn_id.clone())
            .collect::<std::collections::HashSet<_>>();
        let active_epoch = fold
            .conversation_epochs
            .last()
            .cloned()
            .expect("conversation epoch initialized before delivery recovery");
        // Recovery: a completed assistant turn whose delivery was never
        // journaled (a crash between commit and plan) is replanned here. An
        // explicit channel reply edge survives in the fold; autonomous turns
        // without one are recovered as top-level posts.
        let completed_turns = fold
            .turns
            .iter()
            .filter(|turn| turn.role == ChatRole::Assistant && turn.status == Lifecycle::Completed)
            .filter(|turn| !planned_turns.contains(&turn.id))
            .filter(|turn| turn_journal_seq(turn).is_some_and(|seq| seq > active_epoch.journal_seq))
            .cloned()
            .collect::<Vec<_>>();
        for turn in &completed_turns {
            let Some(binding) = active_epoch.backing.discord_binding() else {
                continue;
            };
            let Some(delivery) = build_discord_delivery(
                turn,
                fold.chat_reply_targets.get(&turn.id),
                &fold.messages,
                &binding,
            ) else {
                continue;
            };
            journal.append(|_| EventKind::DiscordChatSendPlanned {
                delivery_id: delivery.delivery_id.clone(),
                turn_id: delivery.turn_id.clone(),
                binding: Some(delivery.binding.clone()),
                sources: delivery.sources.clone(),
                parts: delivery.parts.clone(),
            });
            fold.discord_deliveries
                .insert(delivery.delivery_id.clone(), delivery);
        }

        let (turn_tx, _) = broadcast::channel(TURN_BROADCAST_CAPACITY);
        let (state_tx, _) = broadcast::channel(STATE_BROADCAST_CAPACITY);
        let (playhead_tx, _) = broadcast::channel(PLAYHEAD_BROADCAST_CAPACITY);
        let (inbox_tx, _) = broadcast::channel(INBOX_BROADCAST_CAPACITY);
        Ok(Arc::new(Self {
            name,
            repo_root,
            inner: Mutex::new(Inner {
                journal,
                thread: fold.turns,
                conversation_epochs: fold.conversation_epochs,
                conversation_epoch_turns: fold.conversation_epoch_turns,
                open: None,
                drop_deltas_until_opened: false,
                state,
                playhead,
                last_assistant_turn_id,
                pending_messages: fold.pending_messages,
                messages: fold.messages,
                discord: fold.discord,
                discord_deliveries: fold.discord_deliveries,
                chat_reply_targets: fold.chat_reply_targets,
                tasks: fold.tasks,
                projects: fold.projects,
                promotions: fold.promotions,
            }),
            turn_tx,
            state_tx,
            playhead_tx,
            inbox_tx,
            resident_expected: AtomicBool::new(false),
        }))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn repo_root(&self) -> &std::path::Path {
        &self.repo_root
    }

    pub fn active_conversation_epoch(&self) -> ConversationEpoch {
        self.inner()
            .conversation_epochs
            .last()
            .cloned()
            .expect("an open runtime always has an active conversation epoch")
    }

    pub fn conversation_epochs(&self) -> Vec<ConversationEpoch> {
        self.inner().conversation_epochs.clone()
    }

    pub fn is_imported_conversation_epoch(&self, epoch_id: &str) -> bool {
        self.inner().conversation_epoch_turns.contains_key(epoch_id)
    }

    pub fn chat_messages(
        &self,
        epoch_id: Option<&str>,
        limit: Option<usize>,
    ) -> Vec<WaveChatMessage> {
        let inner = self.inner();
        let selected = match epoch_id {
            Some(id) => inner
                .conversation_epochs
                .iter()
                .find(|epoch| epoch.id == id),
            None => inner.conversation_epochs.last(),
        };
        let Some(epoch) = selected else {
            return Vec::new();
        };
        if !matches!(epoch.backing, ChatBacking::Local) {
            return Vec::new();
        }
        let imported_turns = inner.conversation_epoch_turns.get(&epoch.id);
        let end_seq = inner
            .conversation_epochs
            .iter()
            .find(|candidate| candidate.number == epoch.number + 1)
            .map(|candidate| candidate.journal_seq)
            .unwrap_or(u64::MAX);
        let turns = snapshot_tail_locked(&inner, None)
            .into_iter()
            .filter_map(|turn| {
                let journal_seq = turn_journal_seq(&turn)?;
                let belongs = imported_turns.map_or_else(
                    || journal_seq > epoch.journal_seq && journal_seq < end_seq,
                    |turn_ids| turn_ids.contains(&turn.id),
                );
                belongs.then(|| WaveChatMessage {
                    epoch_id: epoch.id.clone(),
                    source: ChatMessageSource::Local { journal_seq },
                    turn,
                })
            })
            .collect::<Vec<_>>();
        tail_chat_messages(turns, limit)
    }

    pub fn committed_local_message(&self, turn: ChatTurn) -> WaveChatMessage {
        let epoch = self.active_conversation_epoch();
        debug_assert!(matches!(epoch.backing, ChatBacking::Local));
        let journal_seq = turn_journal_seq(&turn)
            .expect("a runtime-committed ChatTurn id carries its journal sequence");
        WaveChatMessage {
            epoch_id: epoch.id,
            source: ChatMessageSource::Local { journal_seq },
            turn,
        }
    }

    pub fn discord_snapshot(&self) -> DiscordSnapshot {
        let inner = self.inner();
        let mut deliveries = inner
            .discord_deliveries
            .values()
            .cloned()
            .collect::<Vec<_>>();
        deliveries.sort_by_key(|delivery| {
            delivery
                .turn_id
                .strip_prefix("turn-")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(u64::MAX)
        });
        DiscordSnapshot {
            attachment: inner.discord.clone(),
            deliveries,
        }
    }

    /// Record the initial channel head before any history can be imported.
    pub fn try_attach_discord(
        &self,
        binding: DiscordChatBinding,
        bot_user_id: String,
        cursor: Option<String>,
    ) -> Result<(), JournalAppendError> {
        let mut inner = self.inner();
        let cursor = match inner.discord.as_ref() {
            Some(attached)
                if attached.binding == binding && attached.bot_user_id == bot_user_id =>
            {
                return Ok(())
            }
            Some(attached) if attached.binding == binding => attached.cursor.clone(),
            _ => cursor,
        };
        inner
            .journal
            .try_append(|_| EventKind::DiscordChatAttached {
                binding: binding.clone(),
                bot_user_id: bot_user_id.clone(),
                cursor: cursor.clone(),
            })?;
        inner.discord = Some(DiscordAttachment {
            binding,
            bot_user_id,
            cursor,
        });
        Ok(())
    }

    /// Journal a Discord input before a cursor can advance. Re-fetching the
    /// same provider identity is an idempotent no-op.
    pub fn try_deliver_discord(
        &self,
        text: String,
        source: DiscordMessageSource,
    ) -> Result<bool, JournalAppendError> {
        self.try_deliver_discord_input(text, source, DiscordInput::Provider)
    }

    pub(crate) fn try_deliver_discord_authored(
        &self,
        text: String,
        source: DiscordMessageSource,
        op: MessageOp,
    ) -> Result<bool, JournalAppendError> {
        self.try_deliver_discord_input(text, source, DiscordInput::Authored(op))
    }

    fn try_deliver_discord_input(
        &self,
        text: String,
        source: DiscordMessageSource,
        input: DiscordInput,
    ) -> Result<bool, JournalAppendError> {
        let mut inner = self.inner();
        let active_binding = inner
            .conversation_epochs
            .last()
            .and_then(|epoch| epoch.backing.discord_binding());
        if active_binding.as_ref() != Some(&source.binding) {
            return Ok(false);
        }
        if inner.messages.values().any(|known| {
            known.source.as_ref().is_some_and(|known| {
                known.binding == source.binding && known.message_id == source.message_id
            })
        }) {
            return Ok(false);
        }
        let event = inner.journal.try_append(|seq| {
            let id = MessageId(format!("msg-{seq}"));
            match input {
                DiscordInput::Provider => EventKind::DiscordUserMessage {
                    id,
                    text: text.clone(),
                    source: source.clone(),
                },
                DiscordInput::Authored(op) => EventKind::DiscordAuthoredMessage {
                    id,
                    op,
                    text: text.clone(),
                    source: source.clone(),
                },
            }
        })?;
        let id = MessageId(format!("msg-{}", event.seq));
        let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text.clone());
        turn.created_at = event.at_rfc3339();
        self.commit_locked(&mut inner, turn);
        let pending = PendingMessage {
            id,
            op: input.op(),
            text: format!("[{}]\n{}", source.uri(), text),
            source: Some(source.clone()),
        };
        inner.messages.insert(pending.id.clone(), pending.clone());
        // Plain chat is observed off the channel tail, never drained from a
        // queue; only steers/interrupts still fold into pending for the live
        // body's consumption (that path is retired when steers become task
        // comments).
        if pending.op != MessageOp::Message {
            inner.pending_messages.push(pending.clone());
        }
        let _ = self.inbox_tx.send(InboxItem::Message(pending));
        Ok(true)
    }

    pub fn try_advance_discord_cursor(
        &self,
        binding: &DiscordChatBinding,
        message_id: String,
    ) -> Result<(), JournalAppendError> {
        let mut inner = self.inner();
        let Some(attached) = inner.discord.as_ref() else {
            return Ok(());
        };
        if &attached.binding != binding || attached.cursor.as_deref() == Some(&message_id) {
            return Ok(());
        }
        inner
            .journal
            .try_append(|_| EventKind::DiscordChatCursorAdvanced {
                binding: binding.clone(),
                message_id: message_id.clone(),
            })?;
        if let Some(attached) = inner.discord.as_mut() {
            attached.cursor = Some(message_id);
        }
        Ok(())
    }

    pub fn try_confirm_discord_part(
        &self,
        delivery_id: &str,
        part_id: &str,
        provider_message_id: String,
    ) -> Result<(), JournalAppendError> {
        let mut inner = self.inner();
        let already_confirmed = inner
            .discord_deliveries
            .get(delivery_id)
            .and_then(|delivery| delivery.confirmed.get(part_id))
            .is_some();
        if already_confirmed {
            return Ok(());
        }
        inner
            .journal
            .try_append(|_| EventKind::DiscordChatSendConfirmed {
                delivery_id: delivery_id.to_string(),
                part_id: part_id.to_string(),
                provider_message_id: provider_message_id.clone(),
            })?;
        if let Some(delivery) = inner.discord_deliveries.get_mut(delivery_id) {
            delivery
                .confirmed
                .insert(part_id.to_string(), provider_message_id);
        }
        Ok(())
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

    /// Read the chat channel as unified [`Message`]s after `since` (a turn
    /// sequence), **including the wave's own posts** — the Discord-shaped read
    /// that lets an observer see it already replied. `None` reads from the start;
    /// an in-progress reply is included once it has text.
    pub fn read_channel(&self, since: Option<u64>) -> Vec<Message> {
        let inner = self.inner();
        snapshot_tail_locked(&inner, None)
            .into_iter()
            .filter_map(Message::from_turn)
            .filter(|(seq, _)| since.is_none_or(|cursor| *seq > cursor))
            .map(|(seq, mut message)| {
                let source = inner
                    .messages
                    .get(&MessageId(format!("msg-{seq}")))
                    .and_then(|pending| pending.source.as_ref());
                if let Some(source) = source {
                    message.author = Author::Bridge {
                        platform: "discord".to_string(),
                        user: source.author_id.clone(),
                    };
                }
                if message.is_own() {
                    message.reply_to = inner
                        .chat_reply_targets
                        .get(&message.id)
                        .and_then(channel_turn_id_for_message);
                }
                message
            })
            .collect()
    }

    /// One durable chat trigger for a subscriber connect or resident restart.
    /// The newest human/app-authored message without a completed `reply_to` edge
    /// is replayed even when a later unrelated bot post exists. The relation is
    /// channel history, not consumption: both messages remain readable. `seen`
    /// dedupes this trigger against the live inbox within one process.
    pub fn unanswered_chat_tail(&self) -> Vec<PendingMessage> {
        unanswered_chat_tail_locked(&self.inner())
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

    /// Initialize the default Wave invocation, or reset stale execution state
    /// once no body is active. Inbox messages live outside the playhead and
    /// remain pending for the fresh root.
    pub fn ensure_playhead(&self) -> anyhow::Result<PlayheadView> {
        let mut inner = self.inner();
        if inner
            .playhead
            .as_ref()
            .is_some_and(|playhead| playhead.active.is_some())
        {
            return Ok(inner
                .playhead
                .as_ref()
                .expect("active body belongs to an initialized playhead")
                .view());
        }
        let root = QueuedInvocation::load(&self.repo_root, "wave")?;
        if inner
            .playhead
            .as_ref()
            .is_some_and(|playhead| playhead.definitions_match(&self.repo_root, &root))
        {
            return Ok(inner
                .playhead
                .as_ref()
                .expect("matching playhead is initialized")
                .view());
        }
        if inner.playhead.is_none() {
            let (playhead, event) = Playhead::new(root);
            inner.playhead = Some(playhead);
            return self.journal_playhead_locked(&mut inner, vec![event]);
        }

        let (playhead, event) = Playhead::reset(root);
        inner.playhead = Some(playhead);
        self.journal_playhead_locked(&mut inner, vec![event])
    }

    /// Enqueue a flow at the innermost active invocation. The queue keeps the
    /// expanded plan until a definition change resets the playhead.
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
            epoch: inner
                .conversation_epochs
                .last()
                .cloned()
                .expect("an open runtime always has an active conversation epoch"),
            turns: snapshot_tail_locked(&inner, limit),
            turn_rx: self.turn_tx.subscribe(),
            state: inner.state.clone(),
            state_rx: self.state_tx.subscribe(),
            playhead: inner.playhead.as_ref().map(Playhead::view),
            playhead_rx: self.playhead_tx.subscribe(),
            pending: inner.pending_messages.clone(),
            chat_tail: unanswered_chat_tail_locked(&inner),
            tasks: inner.tasks.clone(),
            projects: inner.projects.clone(),
            promotions: inner.promotions.clone(),
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
            mut turn,
            claims,
            reply_to,
            ..
        }) = open
        {
            inner.drop_deltas_until_opened = true;
            let finished = inner.journal.append(|_| EventKind::TurnFinished {
                turn_id: turn.id.clone(),
                status,
                termination_reason: Some(reason.to_string()),
            });
            if status != Lifecycle::Completed {
                self.requeue_locked(&mut inner, &claims);
            }
            turn.status = status;
            turn.close_body(finished.at_rfc3339(), Some(reason.to_string()));
            self.transition_locked(&mut inner, LoopState::Idle, reason);
            let committed = self.commit_locked(&mut inner, turn);
            if status == Lifecycle::Completed {
                if let Some(message_id) = reply_to {
                    inner
                        .chat_reply_targets
                        .insert(committed.id.clone(), message_id);
                }
                self.plan_discord_delivery_locked(&mut inner, &committed);
            }
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

    /// Deliver one op from the thread door, uninterpreted by the caller:
    /// the door validates SHAPE (op names, text presence) and hands the op
    /// here; what an op *means* lives in this runtime and the loop's
    /// scheduler. A bare interrupt (empty text) journals nothing
    /// and appends no turn — `None`; every other delivery journals a
    /// `UserMessage`, commits the user turn, and queues for the loop.
    pub fn deliver(&self, op: MessageOp, text: String) -> Option<ChatTurn> {
        self.try_deliver(op, text)
            .expect("journal truth must accept a message before runtime delivery")
    }

    /// Deliver one human op only after its journal row is durable.
    ///
    /// The HTTP door uses this form so a failed write returns a non-success
    /// response and leaves the transcript, pending queue, and live broadcasts
    /// untouched. The caller may retry the same message.
    ///
    /// # Errors
    /// The message's journal append could not be written and flushed.
    pub fn try_deliver(
        &self,
        op: MessageOp,
        text: String,
    ) -> Result<Option<ChatTurn>, JournalAppendError> {
        if op == MessageOp::Interrupt && text.trim().is_empty() {
            self.deliver_interrupt();
            return Ok(None);
        }
        self.try_deliver_message(text, op).map(Some)
    }

    /// Deliver through the product write door, governed by the active epoch.
    /// A bare interrupt is a Wave control and remains available in either
    /// backing; authored text never falls through to a local shadow thread.
    ///
    /// # Errors
    /// Discord-backed authored text is rejected at this local write boundary;
    /// the server routes it through the attached provider before calling here.
    /// Without a provider attachment, the active epoch owns its Open-in-Discord
    /// action. Local journal append failures leave every projection untouched.
    pub fn try_deliver_authored(
        &self,
        op: MessageOp,
        text: String,
    ) -> Result<Option<ChatTurn>, ChatWriteError> {
        if op == MessageOp::Interrupt && text.trim().is_empty() {
            self.deliver_interrupt();
            return Ok(None);
        }
        if matches!(
            self.active_conversation_epoch().backing,
            ChatBacking::Discord { .. }
        ) {
            return Err(ChatWriteError::OpenDiscord);
        }
        self.try_deliver(op, text).map_err(ChatWriteError::from)
    }

    fn try_deliver_message(
        &self,
        text: String,
        op: MessageOp,
    ) -> Result<ChatTurn, JournalAppendError> {
        let mut inner = self.inner();
        let event = inner.journal.try_append(|seq| EventKind::UserMessage {
            id: MessageId(format!("msg-{seq}")),
            op,
            text: text.clone(),
        })?;
        let id = MessageId(format!("msg-{}", event.seq));
        let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text.clone());
        turn.created_at = event.at_rfc3339();
        let turn = self.commit_locked(&mut inner, turn);
        // Chat is a stream to observe: a message signals the resident to observe
        // (inbox broadcast) and is readable via read_channel — it is never queued
        // as pending input to consume.
        let pending = PendingMessage {
            id,
            op,
            text,
            source: None,
        };
        inner.messages.insert(pending.id.clone(), pending.clone());
        // Plain chat is observed off the channel tail, never drained; only
        // steers/interrupts still fold into pending for the live body to
        // consume (retired when steers become task comments).
        if pending.op != MessageOp::Message {
            inner.pending_messages.push(pending.clone());
        }
        // Inbox broadcast still under the lock, so inbox order == journal
        // order — sending after release lets two deliveries invert.
        let _ = self.inbox_tx.send(InboxItem::Message(pending));
        Ok(turn)
    }

    #[cfg(test)]
    pub(crate) fn fail_next_journal_append(&self, failure: JournalAppendStage) {
        self.inner().journal.fail_next_append(failure);
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

    /// Journal and queue the typed promotion wake exactly once per durable occurrence.
    pub(crate) fn deliver_promotion_wake(&self, wake: PromotionWake) -> bool {
        let mut inner = self.inner();
        let pending = promotion_wake_message(&wake);
        if inner.promotions.contains_key(&pending.id) {
            return false;
        }
        inner.journal.append(|_| EventKind::PromotionObserved {
            parent_wave_id: wake.parent_wave_id.clone(),
            parent: wake.parent.clone(),
        });
        inner.messages.insert(pending.id.clone(), pending.clone());
        inner.promotions.insert(pending.id.clone(), wake.clone());
        inner.pending_messages.push(pending);
        let _ = self.inbox_tx.send(InboxItem::Promotion {
            parent_wave_id: wake.parent_wave_id,
            parent: wake.parent,
        });
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
            ResidentDelta::TurnReplyTo { message_id } => self.resident_turn_reply_to(message_id),
            ResidentDelta::TurnItem { item } => self.resident_turn_item(item),
            ResidentDelta::TurnFinished { status, reason } => {
                self.resident_turn_finished(status, reason)
            }
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
            body,
            activity: None,
        };
        let _ = self
            .turn_tx
            .send(TurnBroadcast::Whole(TurnFrame::share(open.clone())));
        inner.open = Some(OpenTurn {
            turn: open,
            text_items: 0,
            claims,
            reply_to: None,
        });
    }

    fn resident_turn_reply_to(&self, message_id: String) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            return;
        }
        let message_id = MessageId(message_id);
        if !inner
            .messages
            .get(&message_id)
            .is_some_and(|message| message.op == MessageOp::Message)
        {
            tracing::warn!(message_id = %message_id.0, "reply target is not a chat message; dropped");
            return;
        }
        let Some(turn_id) = inner.open.as_ref().map(|open| open.turn.id.clone()) else {
            tracing::warn!("reply relation with no open turn; dropped");
            return;
        };
        inner.journal.append(|_| EventKind::ChatReplyLinked {
            turn_id,
            message_id: message_id.clone(),
        });
        if let Some(open) = inner.open.as_mut() {
            open.reply_to = Some(message_id);
        }
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

    fn resident_turn_finished(&self, status: Lifecycle, reason: Option<String>) {
        let mut inner = self.inner();
        if inner.drop_deltas_until_opened {
            tracing::debug!("late TurnFinished after a force-finalize; dropped");
            return;
        }
        let Some(OpenTurn {
            mut turn,
            claims,
            reply_to,
            ..
        }) = inner.open.take()
        else {
            tracing::warn!("TurnFinished with no open turn; dropped");
            return;
        };
        inner.journal.append(|_| EventKind::TurnFinished {
            turn_id: turn.id.clone(),
            status,
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
        let committed = self.commit_locked(&mut inner, turn);
        if status == Lifecycle::Completed {
            if let Some(message_id) = reply_to {
                inner
                    .chat_reply_targets
                    .insert(committed.id.clone(), message_id);
            }
            self.plan_discord_delivery_locked(&mut inner, &committed);
        }
    }

    fn plan_discord_delivery_locked(&self, inner: &mut Inner, turn: &ChatTurn) {
        let Some(binding) = inner
            .conversation_epochs
            .last()
            .and_then(|epoch| epoch.backing.discord_binding())
        else {
            return;
        };
        let Some(delivery) = build_discord_delivery(
            turn,
            inner.chat_reply_targets.get(&turn.id),
            &inner.messages,
            &binding,
        ) else {
            return;
        };
        if inner.discord_deliveries.contains_key(&delivery.delivery_id) {
            return;
        }
        inner.journal.append(|_| EventKind::DiscordChatSendPlanned {
            delivery_id: delivery.delivery_id.clone(),
            turn_id: delivery.turn_id.clone(),
            binding: Some(delivery.binding.clone()),
            sources: delivery.sources.clone(),
            parts: delivery.parts.clone(),
        });
        inner
            .discord_deliveries
            .insert(delivery.delivery_id.clone(), delivery);
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

fn initialize_conversation_epoch(
    journal: &mut Journal,
    epochs: &mut Vec<ConversationEpoch>,
    epoch_turns: &mut HashMap<String, Vec<String>>,
    turns: &[ChatTurn],
    discord_turn_bindings: &HashMap<String, DiscordChatBinding>,
    backing: ChatBacking,
) {
    let migrating_legacy = epochs.is_empty() && !turns.is_empty();
    if migrating_legacy {
        let imported = legacy_conversation_epochs(turns, discord_turn_bindings);
        journal.append(|_| EventKind::ConversationEpochsImported {
            epochs: imported.clone(),
        });
        for item in imported {
            epoch_turns.insert(item.epoch.id.clone(), item.turn_ids);
            epochs.push(item.epoch);
        }
    }
    if !migrating_legacy
        && epochs.last().is_some_and(|epoch| {
            epoch.backing == backing && !epoch.id.starts_with("chat-epoch-legacy-")
        })
    {
        return;
    }
    let number = epochs.last().map_or(1, |epoch| epoch.number + 1);
    let event = journal.append(|seq| EventKind::ConversationEpochStarted {
        epoch_id: format!("chat-epoch-{seq}"),
        number,
        backing: backing.clone(),
    });
    let at = event.at_rfc3339();
    if let Some(previous) = epochs.last_mut() {
        previous.ended_at = Some(at.clone());
    }
    epochs.push(ConversationEpoch {
        id: format!("chat-epoch-{}", event.seq),
        number,
        backing,
        journal_seq: event.seq,
        started_at: at,
        ended_at: None,
    });
}

fn legacy_conversation_epochs(
    turns: &[ChatTurn],
    discord_turn_bindings: &HashMap<String, DiscordChatBinding>,
) -> Vec<ConversationEpochImport> {
    let mut imported: Vec<ConversationEpochImport> = Vec::new();
    for turn in turns {
        let backing = discord_turn_bindings
            .get(&turn.id)
            .map(ChatBacking::discord)
            .unwrap_or(ChatBacking::Local);
        if let Some(active) = imported
            .last_mut()
            .filter(|active| active.epoch.backing == backing)
        {
            active.turn_ids.push(turn.id.clone());
            continue;
        }
        let number = imported.len() as u64 + 1;
        if let Some(previous) = imported.last_mut() {
            previous.epoch.ended_at = Some(turn.created_at.clone());
        }
        imported.push(ConversationEpochImport {
            epoch: ConversationEpoch {
                id: format!("chat-epoch-legacy-{number}"),
                number,
                backing,
                journal_seq: turn_journal_seq(turn).unwrap_or(1).saturating_sub(1),
                started_at: turn.created_at.clone(),
                ended_at: None,
            },
            turn_ids: vec![turn.id.clone()],
        });
    }
    imported
}

fn turn_journal_seq(turn: &ChatTurn) -> Option<u64> {
    turn.id.strip_prefix("turn-")?.parse().ok()
}

/// The newest chat message (`msg-<seq>`, human or app-authored), used only to
/// trigger one re-read of the channel after connect/restart. Observation and
/// promotion inputs carry typed ids, so they never leak into this trigger.
fn unanswered_chat_tail_locked(inner: &Inner) -> Vec<PendingMessage> {
    let message_seq = |message: &PendingMessage| {
        message
            .id
            .0
            .strip_prefix("msg-")
            .and_then(|seq| seq.parse::<u64>().ok())
    };
    inner
        .messages
        .values()
        .filter(|message| message.op == MessageOp::Message)
        .filter(|message| {
            !inner
                .chat_reply_targets
                .values()
                .any(|reply_to| reply_to == &message.id)
        })
        .filter(|message| message_seq(message).is_some())
        .max_by_key(|message| message_seq(message).unwrap_or_default())
        .cloned()
        .into_iter()
        .collect()
}

fn channel_turn_id_for_message(message_id: &MessageId) -> Option<String> {
    message_id
        .0
        .strip_prefix("msg-")
        .map(|seq| format!("turn-{seq}"))
}

fn tail_chat_messages(
    messages: Vec<WaveChatMessage>,
    limit: Option<usize>,
) -> Vec<WaveChatMessage> {
    let take = limit.unwrap_or(messages.len()).min(messages.len());
    messages[messages.len() - take..].to_vec()
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

fn build_discord_delivery(
    turn: &ChatTurn,
    reply_to: Option<&MessageId>,
    messages: &HashMap<MessageId, PendingMessage>,
    binding: &DiscordChatBinding,
) -> Option<DiscordDelivery> {
    if turn.text.trim().is_empty() {
        return None;
    }
    // Replies carry an explicit Discord-shaped edge. Autonomous governance
    // turns have no edge and are posted top-level; chronology never decides
    // that a bot turn answered a human message.
    let sources = reply_to
        .and_then(|message_id| messages.get(message_id))
        .and_then(|message| message.source.clone())
        .filter(|source| &source.binding == binding)
        .into_iter()
        .collect();
    let mut hasher = Sha256::new();
    hasher.update(binding.guild_id.as_bytes());
    hasher.update(binding.channel_id.as_bytes());
    hasher.update(turn.id.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let delivery_id = format!("discord-{}", &digest[..24]);
    let parts = split_discord_content(&turn.text)
        .into_iter()
        .enumerate()
        .map(|(index, content)| DiscordMessagePart {
            part_id: format!("part-{}", index + 1),
            nonce: format!("lf-{}-{index}", &digest[..16]),
            content,
        })
        .collect();
    Some(DiscordDelivery {
        delivery_id,
        turn_id: turn.id.clone(),
        binding: binding.clone(),
        sources,
        parts,
        confirmed: HashMap::new(),
    })
}

fn split_discord_content(content: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut utf16_units = 0;
    for (index, character) in content.char_indices() {
        let units = character.len_utf16();
        if utf16_units + units > 2_000 {
            parts.push(content[start..index].to_string());
            start = index;
            utf16_units = 0;
        }
        utf16_units += units;
    }
    if start < content.len() {
        parts.push(content[start..].to_string());
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::controller::wave::playhead::{StepKind, StepPlan};
    use crate::engine::OccurrencePolicy;

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
            body: None,
            activity: None,
        }
    }

    fn open_runtime(repo: &Path) -> Arc<WaveRuntime> {
        WaveRuntime::open("ship".into(), repo.to_path_buf()).expect("open runtime")
    }

    /// Put a durable wake in the pending fold the way a governance observation
    /// does (chat no longer queues, so the pending/consumption machinery is
    /// exercised through observations). Returns its pending id.
    fn deliver_wake(rt: &WaveRuntime, n: i64, summary: &str) -> MessageId {
        let observation = crate::work::task::TaskObservation {
            task_id: crate::work::task::TaskId::from_raw(format!("task_{n}")),
            issue_identifier: format!("INF-{n}"),
            event_id: n,
            event: crate::work::task::TaskEventKind::Progress {
                summary: summary.to_string(),
            },
        };
        assert!(rt.deliver_task_observation(observation.clone()));
        MessageId(observation.inbox_id())
    }

    fn open_discord_runtime(repo: &Path, binding: &DiscordChatBinding) -> Arc<WaveRuntime> {
        WaveRuntime::open_with_backing(
            "ship".into(),
            repo.to_path_buf(),
            ChatBacking::discord(binding),
        )
        .expect("open Discord runtime")
    }

    #[test]
    fn read_channel_returns_the_conversation_including_the_bots_own_posts() {
        use crate::controller::wave::channel::Author;
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = open_runtime(tmp.path());
        runtime
            .deliver(MessageOp::Message, "what's the top task?".into())
            .expect("human message");
        runtime.append_finalized_turn(progress_turn("The top task is LOO-258."), Vec::new());

        let all = runtime.read_channel(None);
        assert_eq!(all.len(), 2);
        assert!(matches!(all[0].author, Author::Human { .. }));
        assert_eq!(all[0].content, "what's the top task?");
        // The wave's own reply reads back — this is how it knows it answered.
        assert!(all[1].is_own());
        assert_eq!(all[1].content, "The top task is LOO-258.");

        // Reading after the human turn's cursor skips it, keeping the own reply.
        let cursor = all[0]
            .id
            .strip_prefix("turn-")
            .and_then(|seq| seq.parse::<u64>().ok())
            .expect("turn seq");
        let after = runtime.read_channel(Some(cursor));
        assert_eq!(after.len(), 1);
        assert!(after[0].is_own());
    }

    #[test]
    fn a_later_bot_post_never_hides_the_restart_chat_trigger() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = open_runtime(tmp.path());
        runtime
            .deliver(MessageOp::Message, "please answer this".into())
            .expect("human message");

        // This can be governance news that happened to finish after the human
        // message; chronology does not prove that it answered the message.
        runtime.append_finalized_turn(progress_turn("unrelated task finished"), Vec::new());

        let triggers = runtime.unanswered_chat_tail();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].text, "please answer this");
    }

    #[test]
    fn explicit_chat_reply_relation_survives_restart_without_consumption() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = open_runtime(tmp.path());
        runtime
            .deliver(MessageOp::Message, "what is two plus two?".into())
            .expect("human message");
        let message_id = runtime.unanswered_chat_tail()[0].id.0.clone();
        runtime.apply_resident_delta(d_opened(&[]));
        runtime.apply_resident_delta(ResidentDelta::TurnReplyTo { message_id });
        runtime.apply_resident_delta(d_text("Four."));
        runtime.apply_resident_delta(d_finished(Lifecycle::Completed));

        assert!(runtime.unanswered_chat_tail().is_empty());
        let channel = runtime.read_channel(None);
        assert_eq!(channel[1].reply_to.as_deref(), Some(channel[0].id.as_str()));

        drop(runtime);
        let reopened = open_runtime(tmp.path());
        assert!(reopened.unanswered_chat_tail().is_empty());
        let channel = reopened.read_channel(None);
        assert_eq!(channel[1].reply_to.as_deref(), Some(channel[0].id.as_str()));
    }

    #[test]
    fn stale_wave_definition_resets_once_and_preserves_pending_input() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = journal_path(tmp.path(), "ship");
        let (mut journal, _) = Journal::open(&path).expect("open journal");
        let mut root = QueuedInvocation {
            id: "wave-root".to_string(),
            flow: "wave".to_string(),
            steps: ["wave_clarify", "wave_pursue", "wave_mutate"]
                .into_iter()
                .map(|name| StepPlan {
                    name: name.to_string(),
                    kind: StepKind::Skill,
                    policy: OccurrencePolicy::default(),
                })
                .collect(),
        };
        root.steps[1].kind = StepKind::Op;
        let (playhead, event) = Playhead::resume_root(root, 2, 7).expect("legacy playhead");
        journal.append(|_| EventKind::PlayheadChanged {
            event,
            playhead: Box::new(playhead),
        });
        drop(journal);

        let rt = open_runtime(tmp.path());
        // A governance wake is the pending input the reset must preserve.
        deliver_wake(&rt, 1, "recover");
        let reset = rt.ensure_playhead().expect("reset stale definition");
        let current = reset.now.as_ref().expect("fresh root step");
        assert_eq!(current.step, "wave/operate");
        assert_eq!(current.index, 0);
        assert_eq!(current.iteration, 0);
        assert_eq!(reset.stack.len(), 1);
        assert!(reset.stack[0].queue.is_empty());
        assert_ne!(reset.stack[0].id, "wave-root");
        assert_eq!(rt.pending_messages().len(), 1);

        let repeated = rt.ensure_playhead().expect("idempotent definition check");
        assert_eq!(repeated, reset);
        drop(rt);

        let reopened = open_runtime(tmp.path());
        let current = reopened
            .playhead()
            .expect("replayed reset playhead")
            .now
            .expect("selected fresh step");
        assert_eq!(current.step, "wave/operate");
        assert_eq!(current.iteration, 0);
        assert_eq!(reopened.pending_messages().len(), 1);
        drop(reopened);

        let (_, events) = Journal::open(&path).expect("read reset journal");
        let resets = events
            .iter()
            .filter(|event| {
                matches!(
                    &event.kind,
                    EventKind::PlayheadChanged {
                        event: PlayheadEvent::DefinitionReset,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(resets, 1);
    }

    #[test]
    fn stale_wave_definition_waits_for_the_active_body_to_close() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let playhead = rt.ensure_playhead().expect("initialize playhead");
        let body = BodyProvenance::for_step(&playhead.now.expect("current step"), tmp.path());
        let body_id = body.body_id.clone();
        rt.start_body(body).expect("start body");

        let flows = tmp.path().join(".lf/flows");
        std::fs::create_dir_all(&flows).expect("flow directory");
        std::fs::write(flows.join("wave.yaml"), "- replacement\n").expect("replacement flow");

        assert_eq!(
            rt.ensure_playhead()
                .expect("active body stays pinned")
                .now
                .expect("original step remains")
                .step,
            "wave/operate"
        );
        rt.finish_body(&body_id, StepOutcome::Failed, "body ended")
            .expect("close active body");
        assert_eq!(
            rt.ensure_playhead()
                .expect("reset after body closes")
                .now
                .expect("fresh root")
                .step,
            "replacement"
        );
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

    fn d_finished(status: Lifecycle) -> ResidentDelta {
        ResidentDelta::TurnFinished {
            status,
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
        // Chat is observed via the live broadcast, not queued as pending input.
        assert!(rt.pending_messages().is_empty());
    }

    #[test]
    fn task_observation_is_typed_idempotent_and_replayable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let mut rx = rt.subscribe_inbox();
        let observation = crate::work::task::TaskObservation {
            task_id: crate::work::task::TaskId::from_raw("task_example"),
            issue_identifier: "INF-123".to_string(),
            event_id: 7,
            event: crate::work::task::TaskEventKind::Progress {
                summary: "Task is waiting on its parent".to_string(),
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
        let observation = crate::work::project::ProjectObservation {
            project_id: crate::work::project::ProjectId::from_raw("proj_example"),
            project: "developer-efficiency".to_string(),
            event_id: 8,
            event: crate::work::project::ProjectEventKind::Completed {
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
    fn consumed_promotion_stays_consumed_and_deduplicated_after_reopen() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wake = crate::work::wave::PromotionWake {
            parent_wave_id: crate::id::WaveId::new(),
            parent: "platform".to_string(),
        };
        let id = wake.inbox_id();
        let rt = open_runtime(tmp.path());
        assert!(rt.deliver_promotion_wake(wake.clone()));
        rt.apply_resident_delta(d_opened(&[&id]));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        assert!(rt.pending_messages().is_empty());
        drop(rt);
        let reopened = open_runtime(tmp.path());
        assert!(reopened.pending_messages().is_empty());
        assert!(
            !reopened.deliver_promotion_wake(wake),
            "replay keeps the deterministic promotion id deduplicated"
        );
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("read journal");
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(&event.kind, EventKind::PromotionObserved { .. }))
                .count(),
            1
        );
        assert!(!events
            .iter()
            .any(|event| matches!(&event.kind, EventKind::UserMessage { .. })));
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
        rt.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
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

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        assert!(events
            .iter()
            .any(|event| matches!(event.kind, EventKind::TurnFinished { .. })));
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
        let m1 = deliver_wake(&rt, 1, "first");
        let m2 = deliver_wake(&rt, 2, "second");
        assert_eq!(rt.pending_messages().len(), 2);

        // The turn claims both real wakes plus a ghost id.
        rt.apply_resident_delta(d_opened(&[&m1.0, &m2.0, "msg-999"]));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        assert!(
            rt.pending_messages().is_empty(),
            "claimed wakes leave the live pending fold"
        );

        // A second turn re-claiming a consumed id gets nothing.
        rt.apply_resident_delta(d_opened(&[&m1.0]));
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
            vec![m1.clone(), m2.clone()],
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
        let fold = crate::controller::wave::journal::fold_thread(&events);
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

    /// Claimed-but-unanswered messages are requeued when a turn ends without
    /// completing: a Failed TurnFinished returns its claims to pending, and a
    /// restart re-delivers them (never lost). A Completed turn keeps them
    /// consumed.
    #[test]
    fn failed_turn_requeues_its_claimed_messages() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let m1 = deliver_wake(&rt, 1, "do the thing");

        rt.apply_resident_delta(d_opened(&[&m1.0]));
        assert!(rt.pending_messages().is_empty(), "claimed at open");
        // The turn fails: the vendor never answered it, back to pending.
        rt.apply_resident_delta(d_finished(Lifecycle::Failed));
        let pending = rt.pending_messages();
        assert_eq!(pending.len(), 1, "failed turn requeues its claim");
        assert_eq!(pending[0].id, m1);

        // The fold agrees on restart — the requeue is journaled.
        let rt2 = open_runtime(tmp.path());
        assert_eq!(rt2.pending_messages().len(), 1);

        // A completed turn keeps its claim consumed (own journal — the shared
        // path above still holds m1's requeue, which never re-answered).
        let tmp3 = tempfile::tempdir().expect("tempdir");
        let rt3 = open_runtime(tmp3.path());
        let m2 = deliver_wake(&rt3, 2, "second");
        rt3.apply_resident_delta(d_opened(&[&m2.0]));
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
            let m = deliver_wake(&rt, 1, "answer me");
            // Turn opens and claims it, then the server crashes (no finish).
            rt.apply_resident_delta(d_opened(&[&m.0]));
            assert!(rt.pending_messages().is_empty());
            m.0
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
        let m = deliver_wake(&rt, 1, "steer");
        rt.apply_resident_delta(d_opened(&[&m.0]));
        // The harness send failed after the claim: the resident undoes it.
        rt.apply_resident_delta(ResidentDelta::MessagesRequeued {
            ids: vec![m.0.clone()],
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
        let m = deliver_wake(&rt, 1, "go");

        rt.apply_resident_delta(d_opened(&[&m.0]));
        rt.apply_resident_delta(d_text("working"));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        // No assistant turn committed; the wake is still queued.
        assert!(
            rt.thread_snapshot()
                .iter()
                .all(|t| t.role == ChatRole::User),
            "paused: no assistant turn started"
        );
        assert_eq!(rt.pending_messages().len(), 1, "the wake waits");

        // Unpause: the next turn goes through.
        std::fs::write(
            origin.join("wave/ship/GOAL.md"),
            "---\npaused: false\n---\nShip it.\n",
        )
        .unwrap();
        assert!(!rt.paused());
        rt.apply_resident_delta(d_opened(&[&m.0]));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));
        assert!(
            rt.pending_messages().is_empty(),
            "unpaused turn answered it"
        );
    }

    // (Removed: turn_steered_consumes / turn_steered_fallback — mid-turn
    // consumption of a chat steer against the pending fold is obsolete. Chat is
    // a stream to observe; wave-level steering is an observed channel message,
    // and task steering rides task comments + send_current, not TurnSteered.)

    /// The served Wave owns one journal for all accepted thread messages.
    #[test]
    fn only_the_served_mind_journals() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let rt = WaveRuntime::open("ship".into(), origin.clone()).expect("open runtime");

        rt.deliver(MessageOp::Message, "to the wave".into())
            .expect("user turn");
        rt.deliver(MessageOp::Message, "to a".into());

        let wave = rt.thread_snapshot();
        assert_eq!(wave.len(), 2);
        assert_eq!(wave[0].text, "to the wave");

        // On disk: exactly one journal, the served wave's, with both rows.
        assert_eq!(
            loopflow_test_support::journal_files_under(tmp.path()),
            vec![journal_path(&origin, "ship")]
        );
        let events = crate::controller::wave::journal::read_events(&journal_path(&origin, "ship"));
        let messages = events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::UserMessage { .. }))
            .count();
        assert_eq!(messages, 2);

        // Readable across a reopen (chat is a stream to observe, not queued).
        let rt2 = WaveRuntime::open("ship".into(), origin.clone()).expect("reopen");
        let texts: Vec<_> = rt2
            .read_channel(None)
            .into_iter()
            .map(|message| message.content)
            .collect();
        assert_eq!(texts.len(), 2);
        assert!(texts.contains(&"to the wave".to_string()) && texts.contains(&"to a".to_string()));
    }

    /// A subscription's snapshot carries the pending queue (the resident's
    /// boot replay) and its receiver carries exactly the ops sent after it.
    #[test]
    fn subscription_carries_pending_replay_and_live_inbox() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        // A governance wake (observation) is the pending-replay carrier; chat is
        // observed live, never queued.
        let before = deliver_wake(&rt, 1, "before");

        let mut sub = rt.subscribe_with_snapshot(None);
        assert_eq!(sub.pending.len(), 1);
        assert_eq!(sub.pending[0].id, before);
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

    #[test]
    fn discord_chat_input_is_durable_before_cursor_and_deduplicates_after_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let binding = DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        };
        let source = DiscordMessageSource {
            binding: binding.clone(),
            message_id: "101".into(),
            author_id: "human".into(),
        };
        let rt = open_discord_runtime(tmp.path(), &binding);
        rt.try_attach_discord(binding.clone(), "bot".into(), Some("100".into()))
            .expect("attach at current head");
        assert!(rt
            .try_deliver_discord("hello".into(), source.clone())
            .expect("journal input"));
        assert!(!rt
            .try_deliver_discord("hello".into(), source.clone())
            .expect("duplicate input"));
        assert_eq!(
            rt.discord_snapshot()
                .attachment
                .expect("attached")
                .cursor
                .as_deref(),
            Some("100"),
            "input commit does not advance the fetch cursor"
        );

        drop(rt);
        let reopened = open_discord_runtime(tmp.path(), &binding);
        assert!(!reopened
            .try_deliver_discord("hello".into(), source)
            .expect("refetched input"));
        // The discord message is readable (a channel turn), not queued as pending.
        let channel = reopened.read_channel(None);
        assert_eq!(channel.len(), 1, "one readable discord message");
        assert_eq!(channel[0].content, "hello");
        assert!(matches!(
            &channel[0].author,
            Author::Bridge { platform, user }
                if platform == "discord" && user == "human"
        ));
        assert!(reopened.pending_messages().is_empty());
        reopened
            .try_advance_discord_cursor(&binding, "101".into())
            .expect("commit cursor");
        drop(reopened);
        assert_eq!(
            open_discord_runtime(tmp.path(), &binding)
                .discord_snapshot()
                .attachment
                .expect("attached")
                .cursor
                .as_deref(),
            Some("101")
        );
    }

    // (Removed: discord_app_steer_keeps_its_operation_after_restart — an app
    // steer queued in the pending fold is obsolete; discord messages are
    // observed, not queued.)

    #[test]
    fn discord_chat_answer_is_planned_in_chunks_before_receipts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let binding = DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        };
        let rt = open_discord_runtime(tmp.path(), &binding);
        rt.try_attach_discord(binding.clone(), "bot".into(), None)
            .expect("attach");
        rt.try_deliver_discord(
            "question".into(),
            DiscordMessageSource {
                binding: binding.clone(),
                message_id: "101".into(),
                author_id: "human".into(),
            },
        )
        .expect("deliver");
        // Observe: the reply is an instantaneous turn (no consumption); delivery
        // to the Discord epoch still chunks the outbound content.
        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(d_text(&"x".repeat(2_001)));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));

        let delivery = rt
            .discord_snapshot()
            .deliveries
            .into_iter()
            .next()
            .expect("send intent");
        assert_eq!(delivery.parts.len(), 2);
        assert_eq!(delivery.parts[0].content.chars().count(), 2_000);
        assert!(delivery.parts.iter().all(|part| part.nonce.len() <= 25));
        assert!(delivery.confirmed.is_empty());

        rt.try_confirm_discord_part(
            &delivery.delivery_id,
            &delivery.parts[0].part_id,
            "provider-1".into(),
        )
        .expect("confirm first part");
        drop(rt);
        let reopened = open_discord_runtime(tmp.path(), &binding);
        let resumed = &reopened.discord_snapshot().deliveries[0];
        assert_eq!(resumed.confirmed.len(), 1);
        assert_eq!(resumed.parts.len(), 2);
    }

    #[test]
    fn discord_epoch_routes_autonomous_agent_speech_as_a_top_level_message() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let binding = DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        };
        let rt = open_discord_runtime(tmp.path(), &binding);
        rt.try_attach_discord(binding.clone(), "bot".into(), None)
            .expect("attach");

        rt.apply_resident_delta(d_opened(&[]));
        rt.apply_resident_delta(d_text("autonomous update"));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));

        let delivery = rt
            .discord_snapshot()
            .deliveries
            .into_iter()
            .next()
            .expect("active Discord epoch chooses delivery");
        assert_eq!(delivery.binding, binding);
        assert!(
            delivery.sources.is_empty(),
            "no reply target means top-level"
        );
        assert_eq!(delivery.parts[0].content, "autonomous update");
    }

    #[test]
    fn discord_epoch_delivers_speech_that_claims_a_typed_task_observation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let binding = DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        };
        let rt = open_discord_runtime(tmp.path(), &binding);
        let observation = crate::work::task::TaskObservation {
            task_id: crate::work::task::TaskId::from_raw("task_example"),
            issue_identifier: "INF-123".into(),
            event_id: 7,
            event: crate::work::task::TaskEventKind::Progress {
                summary: "Task is waiting on its parent".into(),
            },
        };
        assert!(rt.deliver_task_observation(observation.clone()));

        rt.apply_resident_delta(d_opened(&[&observation.inbox_id()]));
        rt.apply_resident_delta(d_text("I handled the child update."));
        rt.apply_resident_delta(d_finished(Lifecycle::Completed));

        let delivery = rt
            .discord_snapshot()
            .deliveries
            .into_iter()
            .next()
            .expect("typed input cannot suppress active-backing speech");
        assert_eq!(delivery.binding, binding);
        assert!(
            delivery.sources.is_empty(),
            "typed input is not a reply target"
        );
        assert_eq!(delivery.parts[0].content, "I handled the child update.");
    }

    #[test]
    fn wave_chat_backing_switch_rejects_parallel_local_compose() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let binding = DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        };
        let local = open_runtime(tmp.path());
        let local_epoch = local.active_conversation_epoch();
        assert_eq!(local_epoch.number, 1);
        assert_eq!(local_epoch.backing, ChatBacking::Local);
        local
            .try_deliver_authored(MessageOp::Message, "local question".into())
            .expect("local write")
            .expect("local turn");
        let local_messages = local.chat_messages(None, None);
        assert_eq!(local_messages.len(), 1);
        assert!(matches!(
            local_messages[0].source,
            ChatMessageSource::Local { .. }
        ));
        drop(local);

        let discord = open_discord_runtime(tmp.path(), &binding);
        let discord_epoch = discord.active_conversation_epoch();
        assert_eq!(discord_epoch.number, 2);
        assert_eq!(discord_epoch.backing, ChatBacking::discord(&binding));
        let epochs = discord.conversation_epochs();
        assert_eq!(epochs.len(), 2);
        assert!(epochs[0].ended_at.is_some());
        assert_eq!(
            discord.chat_messages(Some(&local_epoch.id), None),
            local_messages,
            "the earlier local epoch remains byte-identical"
        );

        let before =
            crate::controller::wave::journal::read_events(&journal_path(tmp.path(), "ship"));
        let before_pending = discord.pending_messages();
        let error = discord
            .try_deliver_authored(MessageOp::Message, "shadow message".into())
            .expect_err("Discord mode rejects Loopflow compose");
        assert!(matches!(error, ChatWriteError::OpenDiscord));
        let after =
            crate::controller::wave::journal::read_events(&journal_path(tmp.path(), "ship"));
        assert_eq!(before, after, "rejection appends no local journal event");
        assert_eq!(before_pending, discord.pending_messages());
        assert!(discord.chat_messages(None, None).is_empty());

        discord
            .try_deliver_authored(MessageOp::Interrupt, String::new())
            .expect("bare interrupt remains available");
        drop(discord);
        let reopened = open_discord_runtime(tmp.path(), &binding);
        assert_eq!(
            reopened.active_conversation_epoch().id,
            discord_epoch.id,
            "a restart with the same backing resumes the epoch"
        );
        drop(reopened);

        let local_again = open_runtime(tmp.path());
        assert_eq!(local_again.active_conversation_epoch().number, 3);
        assert_eq!(
            local_again.active_conversation_epoch().backing,
            ChatBacking::Local
        );
        assert_eq!(local_again.conversation_epochs().len(), 3);
    }

    #[test]
    fn legacy_local_epoch_survives_the_migration_restart() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = journal_path(tmp.path(), "ship");
        let (mut journal, _) = Journal::open(&path).expect("legacy journal");
        journal.append(|_| EventKind::UserMessage {
            id: MessageId("legacy-message".into()),
            op: MessageOp::Message,
            text: "before epochs".into(),
        });
        drop(journal);

        let migrated = open_runtime(tmp.path());
        let epochs = migrated.conversation_epochs();
        assert_eq!(epochs.len(), 2);
        assert_eq!(epochs[0].id, "chat-epoch-legacy-1");
        assert_eq!(epochs[0].backing, ChatBacking::Local);
        let legacy_messages = migrated.chat_messages(Some(&epochs[0].id), None);
        assert_eq!(legacy_messages.len(), 1);
        assert_eq!(legacy_messages[0].turn.text, "before epochs");
        drop(migrated);

        let reopened = open_runtime(tmp.path());
        assert_eq!(reopened.conversation_epochs(), epochs);
        assert_eq!(
            reopened.chat_messages(Some("chat-epoch-legacy-1"), None),
            legacy_messages
        );
    }

    #[test]
    fn legacy_mixed_chat_imports_truthful_backing_epochs_atomically() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = journal_path(tmp.path(), "ship");
        let binding = DiscordChatBinding {
            guild_id: "guild".into(),
            channel_id: "channel".into(),
        };
        let (mut journal, _) = Journal::open(&path).expect("legacy journal");
        journal.append(|_| EventKind::UserMessage {
            id: MessageId("local-message".into()),
            op: MessageOp::Message,
            text: "local history".into(),
        });
        journal.append(|_| EventKind::DiscordUserMessage {
            id: MessageId("discord-message".into()),
            text: "provider history".into(),
            source: DiscordMessageSource {
                binding: binding.clone(),
                message_id: "101".into(),
                author_id: "human".into(),
            },
        });
        drop(journal);

        let migrated = open_runtime(tmp.path());
        let epochs = migrated.conversation_epochs();
        assert_eq!(epochs.len(), 3);
        assert_eq!(epochs[0].backing, ChatBacking::Local);
        assert_eq!(epochs[1].backing, ChatBacking::discord(&binding));
        assert_eq!(epochs[2].backing, ChatBacking::Local);
        assert_eq!(
            migrated.chat_messages(Some(&epochs[0].id), None)[0]
                .turn
                .text,
            "local history"
        );
        assert!(
            migrated.chat_messages(Some(&epochs[1].id), None).is_empty(),
            "Discord history remains provider-projected, never mislabeled local"
        );
        let imported = crate::controller::wave::journal::read_events(&path)
            .into_iter()
            .filter(|event| matches!(event.kind, EventKind::ConversationEpochsImported { .. }))
            .count();
        assert_eq!(imported, 1, "the entire legacy catalog is one append");
        drop(migrated);

        assert_eq!(open_runtime(tmp.path()).conversation_epochs(), epochs);
    }
}
