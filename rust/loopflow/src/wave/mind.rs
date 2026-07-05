//! The resident's scheduler: one persistent vendor thread, scheduled by
//! events, publishing through the wire.
//!
//! This runs INSIDE THE RESIDENT PROCESS (`lf wave <name> --mind-only`, see
//! [`crate::wave::resident`]) — never in the listener. The mind's identity is
//! its operating prompt (the rendered `GOAL.md` seed plus the orchestration
//! discipline); everything it does surfaces as [`ResidentDelta`]s sent
//! through the listener's resident door, where the journal, the open-turn
//! snapshot, SSE broadcast, and `MindState` transitions live.
//!
//! # Scheduling
//! Input is the wave's `/events?inbox=true` subscription, parsed into
//! [`InboxItem`]s by the resident:
//! - **Message while idle** → a turn starts now; the `TurnOpened` delta's
//!   `answers` names the message plus anything already queued — the RESIDENT
//!   decides what a turn answers; the listener validates and journals.
//! - **Message while turning** → HUMAN speech steers by default: an
//!   unattributed message (no byline) rides the explicit-steer path —
//!   injected into the live turn now, consumption declared with a
//!   `TurnSteered` delta — when the harness can steer. Attributed messages
//!   (worker reports, child-wave escalations) always queue
//!   (append-and-coalesce, never rejected): colleagues interrupt you;
//!   status reports wait. At the turn boundary one turn drains the whole
//!   queue.
//! - **Steer while turning**, on a steer-capable harness → injected into the
//!   live turn (`send_input` mid-turn); consumption declared with a
//!   `TurnSteered` delta. On a non-capable harness, while interrupting, or
//!   while idle, a steer degrades to a queued message.
//! - **Interrupt while turning** → cooperative `harness.interrupt()`, an
//!   `Interrupting` state delta, and a local deadline
//!   ([`INTERRUPT_DEADLINE`]): if the harness swallows the cancel, the
//!   resident force-closes THROUGH THE WIRE (`TurnFinished{Interrupted}`) and
//!   moves on. The listener keeps its own, longer janitor for a resident gone
//!   fully silent (see [`crate::wave::supervisor`]).
//! - **Interrupt while idle** → no-op; text, if any, starts the next turn.
//! - **Heartbeat**: idle for [`HEARTBEAT_IDLE`] with an empty queue → a
//!   progress turn carrying a compact nudge plus the `<in_flight>` fold
//!   fetched from `GET /resident/context`.
//! - **Cron**: the wave's `crons:` frontmatter (GOAL.md, re-read at every
//!   deadline computation so edits land without a restart) arms a third
//!   deadline; a due schedule opens a system turn ("cron due: <flow> —
//!   dispatch it") exactly like the heartbeat nudge. Like the heartbeat,
//!   crons only fire while idle — a schedule that comes due mid-turn fires
//!   at the boundary (within [`CRON_GRACE`]). The daemon's cron poller and
//!   the `wave_crons` table died in the collapse's organ cut.
//!
//! The select is `biased` toward the inbox.
//!
//! # Failure
//! A failed turn returns the mind to idle. [`MAX_CONSECUTIVE_TURN_FAILURES`]
//! consecutive failures, or a terminal harness error, FAIL THE MIND: the
//! resident reports `MindState::Failed` over the wire and [`run_mind`]
//! returns an error — the process exits nonzero and the LISTENER's
//! supervisor owns revival (the process-level respawn ladder; a human
//! message respawns immediately). A dead mind is a dead process — there is
//! no in-process limbo. The listener disappearing (send failure, inbox
//! closed) ends the residency cleanly instead: `Ok(())`.
//!
//! # Resume
//! The attach response carries the last journaled vendor thread id, seeded
//! via [`Harness::set_provider_session_id`]. The codex driver does NOT honor
//! this for app-server threads (every boot is a cold start); the id the
//! vendor actually announces is reported as a `ThreadStarted` delta and
//! journaled by the listener — the break in continuity is explicit in the
//! log. The visible thread survives regardless: it is the listener's fold.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::conversation::turns::{ChatRole, ChatTurn};
use crate::conversation::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::engine::agent::AgentConfig;
use crate::engine::flow::{available_flow_names, load_goal, render_goal, GoalRenderContext};
use crate::engine::wave_config::{read_wave_config, WaveCronDef};
use crate::harness::{is_terminal_harness_error, Harness};
use crate::wave::journal::{ellipsize, MessageId, MessageOp, PendingMessage};
use crate::wave::memory::Memory;
use crate::wave::resident::ListenerClient;
use crate::wave::runtime::InboxItem;
use crate::wave::supervisor::sleep_until_opt;
use crate::wave::wire::{InFlightWorker, ResidentDelta, ResidentStateTo};

/// How long the mind sits idle (empty queue, no turn) before a heartbeat
/// turn. Each heartbeat burns a subscription turn on the vendor plan, so the
/// quiet-wave cadence is deliberately coarse; messages and turn boundaries
/// drive the mind the rest of the time.
pub const HEARTBEAT_IDLE: Duration = Duration::from_secs(300);

/// Consecutive failed turns before the mind itself is declared failed and
/// the resident exits (the listener's supervisor revives by respawning).
pub const MAX_CONSECUTIVE_TURN_FAILURES: u32 = 3;

/// How long a cooperative cancel may run before the resident force-closes
/// the turn through the wire (`TurnFinished{Interrupted}`). Deliberately
/// shorter than the listener's own janitor bound
/// (`supervisor::LISTENER_INTERRUPT_DEADLINE`): the resident closes first
/// when it can; the listener's fires only for a silent resident.
pub const INTERRUPT_DEADLINE: Duration = Duration::from_secs(10);

/// How long the adapter may hold a finished turn awaiting the vendor's
/// trailing `TurnUsage` before the boundary is flushed without usage. A
/// vendor that goes silent right after `TurnCompleted` would otherwise leave
/// the adapter tracking forever — `in_flight` stuck true, heartbeat and
/// synthetic boundary both disabled — the usage-wedge.
pub const USAGE_FLUSH: Duration = Duration::from_secs(5);

/// How far back a never-fired (or long-idle) cron schedule is checked: an
/// occurrence within this window still fires; anything older is missed, not
/// replayed. Mirrors the dead daemon poller's grace so a wave that was down
/// over its weekly schedule still runs it on revival.
pub const CRON_GRACE: chrono::Duration = chrono::Duration::hours(24);

/// Compact nudge for heartbeat turns. The thread is persistent, so heartbeats
/// never re-send the seed — the first turn carried it.
const HEARTBEAT_PROMPT: &str = "Heartbeat: re-read your goal and memory, then take the next \
     orchestration step. If nothing needs doing, say so in one line.";

/// Longest task excerpt carried per worker in the `<in_flight>` section —
/// enough to recognize the dispatch, token-lean by design.
const IN_FLIGHT_TASK_CHARS: usize = 80;

/// The heartbeat nudge, plus a compact `<in_flight>` section when workers are
/// grinding: one line per dispatched-not-finished worker, from the listener's
/// `GET /resident/context` — the mind's orchestration turns see their workers
/// without re-reading transcripts.
pub fn heartbeat_prompt(workers: &[InFlightWorker]) -> String {
    if workers.is_empty() {
        return HEARTBEAT_PROMPT.to_string();
    }
    let mut prompt = String::from(HEARTBEAT_PROMPT);
    prompt.push_str("\n\n<in_flight>\n");
    for worker in workers {
        // Whitespace-flattened, so a multi-line task can't break the
        // one-line-per-worker format.
        let task = ellipsize(&worker.task, IN_FLIGHT_TASK_CHARS);
        prompt.push_str(&format!(
            "- run {} · {}: {} · running\n",
            worker.run_id, worker.flow, task
        ));
    }
    prompt.push_str("</in_flight>");
    prompt
}

// -- Cron: the third deadline ------------------------------------------------

/// The wave's cron lines, re-read from GOAL.md frontmatter on every deadline
/// computation — editing the file reschedules a live mind, no restart.
fn read_crons(origin_repo: &Path, wave: &str) -> Vec<WaveCronDef> {
    read_wave_config(origin_repo, wave)
        .and_then(|config| config.crons)
        .unwrap_or_default()
}

/// Identity of one cron line for last-fired bookkeeping: the schedule and
/// flow together, so editing either resets the line's history.
fn cron_key(cron: &WaveCronDef) -> String {
    format!("{} {}", cron.schedule, cron.flow)
}

/// The next fire time for one schedule: the first occurrence after
/// `last_fired` (or `now - CRON_GRACE` for a line that never fired).
/// Unparseable schedules never fire.
fn next_cron_fire(
    schedule: &str,
    last_fired: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let schedule = cron::Schedule::from_str(schedule).ok()?;
    let check_from = last_fired.unwrap_or(now - CRON_GRACE);
    schedule.after(&check_from).next()
}

/// The system turn a due schedule opens — the mind dispatches the flow with
/// judgment, exactly like it acts on a heartbeat nudge.
pub(crate) fn cron_prompt(due: &[WaveCronDef]) -> String {
    due.iter()
        .map(|cron| format!("cron due: {} — dispatch it", cron.flow))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Scheduler knobs. `Default` is production: codex vendor, 5-minute
/// heartbeat, 10-second interrupt deadline. (Auto-revival is no longer a
/// mind knob — the listener's supervisor respawns the process.)
#[derive(Debug, Clone)]
pub struct MindConfig {
    /// Vendor label reported in the `ThreadStarted` delta.
    pub vendor: String,
    /// Idle window before a heartbeat turn (see [`HEARTBEAT_IDLE`]).
    pub heartbeat_idle: Duration,
    /// Resident-side cancel bound (see [`INTERRUPT_DEADLINE`]).
    pub interrupt_deadline: Duration,
    /// Vendor-silence bound on the held turn boundary (see [`USAGE_FLUSH`]).
    pub usage_flush: Duration,
}

impl Default for MindConfig {
    fn default() -> Self {
        Self {
            vendor: "codex".to_string(),
            heartbeat_idle: HEARTBEAT_IDLE,
            interrupt_deadline: INTERRUPT_DEADLINE,
            usage_flush: USAGE_FLUSH,
        }
    }
}

/// PATH for the mind's harness and every child the resident spawns: this
/// executable's directory first, so the discipline commands (`lf q worker
/// run …`) resolve to the binary running this resident, never whatever
/// `lf` the user's shell happens to find (observed live: the mind's `lf`
/// was an older installed build missing `lf q`).
pub fn path_for_children() -> OsString {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return inherited;
    };
    let paths = std::iter::once(exe_dir).chain(std::env::split_paths(&inherited));
    std::env::join_paths(paths).unwrap_or(inherited)
}

/// The mind's operating prompt: the rendered goal seed, the orchestration
/// discipline, and the shared loopflow operating document (the mind's prompt
/// bypasses context assembly, so the `<lf:loopflow>` section is appended here).
/// Rides the harness `AgentConfig` system prompt, which the codex driver
/// prepends to the first turn of the thread. Reads GOAL.md and MEMORY.md from
/// the ORIGIN repo (reads are free; writes go through the listener's doors).
pub fn mind_agent_config(origin_repo: &Path, wave: &str, cwd: &Path) -> AgentConfig {
    let memory = Memory::for_wave(origin_repo, wave);
    let seed = build_goal_seed(origin_repo, wave, &memory);
    AgentConfig {
        system_prompt: format!(
            "{seed}\n\n{}\n\n{}",
            orchestration_discipline(wave),
            crate::engine::prompt::loopflow_section()
        ),
        task_prompt: String::new(),
        agent: None,
        max_turns: None,
        cwd: Some(cwd.to_path_buf()),
        skip_permissions: true,
        structured_replies: Vec::new(),
        directive_relay: None,
    }
}

/// The wave's rendered `GOAL.md` plus current memory, or a minimal-but-real
/// fallback when there's no `GOAL.md` so the mind still has an identity.
fn build_goal_seed(repo: &Path, wave: &str, memory: &Memory) -> String {
    match load_goal(wave, repo) {
        Ok(goal) => {
            let ctx = GoalRenderContext {
                flows: available_flow_names(repo),
                memory: memory.read(),
            };
            render_goal(&goal, &ctx)
        }
        Err(_) => {
            let mem = memory.read();
            let mem_block = if mem.trim().is_empty() {
                "(memory is empty)".to_string()
            } else {
                mem
            };
            format!(
                "You are the agent of the '{wave}' wave. Drive the wave's goal \
                 forward.\n\nCurrent memory:\n{mem_block}"
            )
        }
    }
}

/// The coordinating-session discipline, promoted into the mind's system
/// prompt: the mind orchestrates, it never grinds inline. Mind-specific rules
/// only — shared loopflow operating guidance is appended in
/// [`mind_agent_config`], not duplicated here.
fn orchestration_discipline(wave: &str) -> String {
    format!(
        "You are the mind of the '{wave}' wave — its long-running orchestrator.\n\
         Discipline:\n\
         - Never grind inline. Read state, decide, dispatch work to subagents \
         via `lf q worker run {wave} --flow <flow> --task \"<task>\"` (add \
         `--pool` to share the wave's worktree, `--stack <run-id>` to build on \
         an unlanded run), curate what you learn into memory, and answer the \
         human.\n\
         - Exception: trivial, single-file, sub-minute work is done inline \
         without dispatch; dispatch is for real units of work.\n\
         - Keep turns short — decisions and dispatches, not implementation.\n\
         - Trust worker summaries; never re-read worker transcripts.\n\
         - A human message is steering: answer it directly and adjust course \
         before returning to the goal."
    )
}

// -- Adapter: harness ConversationEvents → the resident wire vocabulary --

/// Adapts the harness's [`ConversationEvent`] stream into [`ResidentDelta`]s
/// — the wire the listener's fold consumes. One pipeline, not a fork: the
/// listener grows its open-turn snapshot from exactly these deltas.
///
/// Turn-grained for the MVP wire: reasoning deltas and item phases are
/// dropped. Prose lands once: prefer a completed `Message` item when the
/// harness has one (codex), otherwise fold buffered `TextDelta`s at the turn
/// boundary (Claude/OpenCode). Harnesses emit `TurnUsage` after
/// `TurnCompleted` (possibly empty); the `TurnFinished` delta is held until
/// that trailing usage so cost rides the finalization — held at most
/// [`USAGE_FLUSH`] of vendor silence, after which the scheduler flushes the
/// boundary without usage.
///
/// `TurnOpened` deltas leave `answers` empty — the scheduler injects the
/// consumption declaration before sending (it owns the queue).
#[derive(Debug, Default)]
pub struct EventAdapter {
    /// The turn being assembled — mirrors what the listener journals, so the
    /// adapter can drop empty items and duplicate prose at the source.
    open: Option<ChatTurn>,
    /// Terminal status awaiting the trailing `TurnUsage` before the
    /// `TurnFinished` delta is emitted.
    finished: Option<Lifecycle>,
    /// Streaming prose from harnesses that do not send a final Message item.
    buffered_text_delta: String,
}

impl EventAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the adapter is mid-assembly: a turn is open, or finalized and
    /// awaiting its trailing usage. The scheduler uses this to spot a vendor
    /// `TurnCompleted` for a turn the adapter never saw open — the wedge
    /// class that needs a synthetic boundary.
    pub fn tracking(&self) -> bool {
        self.open.is_some() || self.finished.is_some()
    }

    /// Finalized and still awaiting the trailing usage — the gap the
    /// scheduler's flush deadline guards (see [`USAGE_FLUSH`]).
    pub fn awaiting_usage(&self) -> bool {
        self.finished.is_some()
    }

    /// Give up on the trailing usage: emit the held `TurnFinished` (no
    /// cost). The scheduler calls this when the vendor goes silent after
    /// `TurnCompleted`.
    pub fn flush_stalled(&mut self) -> Vec<ResidentDelta> {
        let mut deltas = Vec::new();
        self.flush_finished(&mut deltas, None);
        deltas
    }

    pub fn feed(&mut self, event: &ConversationEvent) -> Vec<ResidentDelta> {
        let mut deltas = Vec::new();
        match event {
            ConversationEvent::TurnStarted { .. } => {
                self.flush_finished(&mut deltas, None);
                if self.open.is_some() {
                    // Defensive: the vendor opened a turn over an open one.
                    self.flush_buffered_text(&mut deltas);
                    self.close_open(Lifecycle::Failed);
                    self.flush_finished(&mut deltas, None);
                }
                self.open = Some(ChatTurn {
                    id: String::new(),
                    role: ChatRole::Assistant,
                    text: String::new(),
                    status: Lifecycle::Running,
                    items: Vec::new(),
                    created_at: String::new(),
                    from: None,
                });
                self.buffered_text_delta.clear();
                deltas.push(ResidentDelta::TurnOpened {
                    answers: Vec::new(),
                });
            }
            ConversationEvent::ItemCompleted { item, .. } => {
                self.flush_finished(&mut deltas, None);
                // Codex reasoning streams as deltas; the completion item
                // often carries no accumulated text. An empty Message/Thought
                // would land on the wire as a blank row — drop it here, the
                // one spot where harness items become wire items.
                if is_empty_text_item(item) {
                    return deltas;
                }
                let Some(open) = self.open.as_mut() else {
                    tracing::warn!("harness item outside a turn; dropped");
                    return deltas;
                };
                open.absorb_item(item.clone());
                if let ConversationItem::Message { text, .. } = item {
                    deltas.push(ResidentDelta::TurnText { text: text.clone() });
                } else {
                    deltas.push(ResidentDelta::TurnItem { item: item.clone() });
                }
            }
            ConversationEvent::TextDelta { content, .. } => {
                self.flush_finished(&mut deltas, None);
                if self.open.is_some() {
                    self.buffered_text_delta.push_str(content);
                }
            }
            ConversationEvent::TurnCompleted { status, .. } => {
                self.flush_finished(&mut deltas, None);
                self.flush_buffered_text(&mut deltas);
                self.close_open(*status);
            }
            ConversationEvent::TurnUsage { usage, .. } => {
                if self.finished.is_some() {
                    deltas.push(ResidentDelta::TurnUsage {
                        input_tokens: Some(usage.input_tokens),
                        output_tokens: Some(usage.output_tokens),
                        cache_read_tokens: usage.cache_read_tokens,
                    });
                    self.flush_finished(&mut deltas, usage.cost_usd);
                }
            }
            ConversationEvent::Error { .. } => {
                // A broken stream mid-turn finalizes the turn as failed; the
                // scheduler decides whether the mind itself is dead.
                self.flush_finished(&mut deltas, None);
                if self.open.is_some() {
                    self.flush_buffered_text(&mut deltas);
                    self.close_open(Lifecycle::Failed);
                    self.flush_finished(&mut deltas, None);
                }
            }
            // Token deltas, item phases, diffs, suggested actions, status —
            // finer than the turn-grained wire; the journal gains them with
            // the part-grained phase.
            _ => {}
        }
        deltas
    }

    fn close_open(&mut self, status: Lifecycle) {
        if self.open.take().is_some() {
            self.finished = Some(status);
        }
        self.buffered_text_delta.clear();
    }

    fn flush_buffered_text(&mut self, deltas: &mut Vec<ResidentDelta>) {
        let text = std::mem::take(&mut self.buffered_text_delta);
        let Some(open) = self.open.as_mut() else {
            return;
        };
        // Prose already landed as a final Message item (open.text non-empty):
        // the buffered stream is the same prose, discarded.
        if text.is_empty() || !open.text.is_empty() {
            return;
        }
        open.push_text(&text);
        deltas.push(ResidentDelta::TurnText { text });
    }

    fn flush_finished(&mut self, deltas: &mut Vec<ResidentDelta>, cost_usd: Option<f64>) {
        if let Some(status) = self.finished.take() {
            deltas.push(ResidentDelta::TurnFinished { status, cost_usd });
        }
    }
}

/// A Message/Thought whose text is empty says nothing — not a wire item.
fn is_empty_text_item(item: &ConversationItem) -> bool {
    match item {
        ConversationItem::Message { text, .. } | ConversationItem::Thought { text, .. } => {
            text.trim().is_empty()
        }
        _ => false,
    }
}

// -- The scheduler --

/// Why the mind's loop ended.
enum MindEnd {
    /// The listener is gone (send failed / inbox closed): the keeper died or
    /// was replaced. Clean exit — nothing to revive on this side.
    ListenerGone,
    /// The mind itself failed (reported over the wire before ending). The
    /// resident exits nonzero; the listener's supervisor respawns.
    Failed(String),
}

/// Run the mind until the listener disappears (`Ok`) or the mind fails
/// (`Err`, after reporting `MindState::Failed` over the wire).
///
/// # Errors
/// Mind failure only — the caller exits the process nonzero so the listener's
/// supervisor sees a dead resident.
#[allow(clippy::too_many_arguments)]
pub async fn run_mind(
    client: ListenerClient,
    mut inbox_rx: mpsc::UnboundedReceiver<InboxItem>,
    harness: Box<dyn Harness>,
    mut events_rx: mpsc::UnboundedReceiver<ConversationEvent>,
    cwd: PathBuf,
    origin_repo: PathBuf,
    wave: String,
    resume_thread_id: Option<String>,
    config: MindConfig,
) -> Result<()> {
    let mut mind = Mind {
        client,
        harness,
        adapter: EventAdapter::new(),
        cwd,
        origin_repo,
        wave,
        config,
        queue: Vec::new(),
        seen: HashSet::new(),
        pending_answers: Vec::new(),
        consecutive_failures: 0,
        in_flight: false,
        started: false,
        idle_since: Instant::now(),
        interrupt_deadline: None,
        usage_flush_at: None,
        cron_last_fired: HashMap::new(),
        end: None,
    };

    if let Err(err) = mind.start_thread(resume_thread_id).await {
        mind.fail(&format!("mind failed to start: {err:#}")).await;
    }

    let mut events_open = true;
    while mind.end.is_none() {
        let heartbeat_at = mind.heartbeat_deadline();
        let cron_at = mind.cron_deadline();
        let interrupt_at = mind.interrupt_deadline;
        let usage_flush_at = mind.usage_flush_at;
        tokio::select! {
            // Biased toward the inbox: a message that arrived before a turn
            // boundary is queued before the boundary drains, so coalescing
            // is deterministic.
            biased;
            item = inbox_rx.recv() => {
                match item {
                    Some(item) => mind.on_inbox(item).await,
                    // The subscription ended: the keeper is gone.
                    None => mind.end = Some(MindEnd::ListenerGone),
                }
            }
            event = events_rx.recv(), if events_open => {
                match event {
                    Some(event) => mind.on_event(event).await,
                    None => {
                        events_open = false;
                        mind.fail("harness event stream ended").await;
                    }
                }
            }
            _ = sleep_until_opt(interrupt_at), if interrupt_at.is_some() => {
                mind.on_interrupt_deadline().await;
            }
            // The usage-wedge escape: the vendor completed a turn and then
            // went fully silent — the trailing `TurnUsage` never came. Flush
            // the held boundary so the scheduler never wedges on it.
            _ = sleep_until_opt(usage_flush_at), if usage_flush_at.is_some() => {
                mind.on_usage_flush().await;
            }
            // The third deadline: a due cron schedule from GOAL.md
            // frontmatter opens a system turn. Armed only while idle, like
            // the heartbeat — a mid-turn due date fires at the boundary.
            _ = sleep_until_opt(cron_at), if cron_at.is_some() => {
                mind.on_cron().await;
            }
            _ = sleep_until_opt(heartbeat_at) => {
                mind.on_heartbeat().await;
            }
        }
    }

    // End of residency: end the vendor session (kills the child process).
    let _ = mind.harness.stop().await;
    match mind.end {
        Some(MindEnd::Failed(reason)) => Err(anyhow!(reason)),
        _ => Ok(()),
    }
}

struct Mind {
    client: ListenerClient,
    harness: Box<dyn Harness>,
    adapter: EventAdapter,
    cwd: PathBuf,
    origin_repo: PathBuf,
    wave: String,
    config: MindConfig,
    /// Messages awaiting a turn. The listener's journal fold is the durable
    /// queue (replayed to a fresh resident); this is the scheduler's working
    /// copy.
    queue: Vec<PendingMessage>,
    /// Every message id this resident has taken in — the dedup guard for the
    /// subscription's pending replay (a reconnect would re-offer unconsumed
    /// messages; a fresh process's replay is exactly its boot queue).
    seen: HashSet<MessageId>,
    /// The queued `MessageId`s the next `TurnOpened` declares as answers —
    /// set when a turn's input is sent, injected when the vendor opens it.
    pending_answers: Vec<MessageId>,
    consecutive_failures: u32,
    /// A turn's input has been sent and its `TurnFinished` hasn't landed yet.
    in_flight: bool,
    /// The vendor session is alive (start succeeded, no terminal error).
    started: bool,
    idle_since: Instant,
    /// Resident-side cancel bound while interrupting: armed at op receipt
    /// (BEFORE the vendor call — a slow vendor must not delay it), cleared
    /// at the turn boundary. Past it, the resident force-closes through the
    /// wire.
    interrupt_deadline: Option<Instant>,
    /// The usage-flush deadline: armed while the adapter holds a finished
    /// turn awaiting its trailing `TurnUsage`, re-armed by any vendor event,
    /// cleared when the boundary lands. Past it, the held `TurnFinished` is
    /// flushed without usage (see [`USAGE_FLUSH`]).
    usage_flush_at: Option<Instant>,
    /// When each cron line (keyed by [`cron_key`]) last opened a turn — the
    /// scheduler's working memory; a respawned resident re-checks within
    /// [`CRON_GRACE`].
    cron_last_fired: HashMap<String, DateTime<Utc>>,
    /// Set once, ends the loop (see [`MindEnd`]).
    end: Option<MindEnd>,
}

impl Mind {
    fn heartbeat_deadline(&self) -> Option<Instant> {
        (self.idle()).then(|| self.idle_since + self.config.heartbeat_idle)
    }

    /// Idle and able to open a turn: the arming condition shared by the
    /// heartbeat and cron deadlines.
    fn idle(&self) -> bool {
        self.started && self.end.is_none() && !self.in_flight && self.queue.is_empty()
    }

    /// The earliest upcoming cron fire across the wave's `crons:` frontmatter
    /// lines, as a select deadline. `None` while turning (the boundary
    /// re-arms) or when no schedule parses.
    fn cron_deadline(&self) -> Option<Instant> {
        if !self.idle() {
            return None;
        }
        let now = Utc::now();
        let next = read_crons(&self.origin_repo, &self.wave)
            .iter()
            .filter_map(|cron| {
                next_cron_fire(
                    &cron.schedule,
                    self.cron_last_fired.get(&cron_key(cron)).copied(),
                    now,
                )
            })
            .min()?;
        let wait = (next - now).to_std().unwrap_or(Duration::ZERO);
        Some(Instant::now() + wait)
    }

    /// Start the vendor thread and report `ThreadStarted` — the mind's first
    /// durable act, journaled by the listener before any turn
    /// (borrowed-handle rule). See the module doc for what resume actually
    /// does on codex.
    async fn start_thread(&mut self, resume: Option<String>) -> Result<()> {
        if let Some(previous) = resume {
            self.harness.set_provider_session_id(Some(previous));
        }
        let agent_config = mind_agent_config(&self.origin_repo, &self.wave, &self.cwd);
        self.harness.start(&agent_config).await?;
        self.started = true;
        match self.harness.provider_session_id() {
            Some(thread_id) => {
                self.send(vec![ResidentDelta::ThreadStarted {
                    vendor: self.config.vendor.clone(),
                    thread_id,
                }])
                .await;
            }
            None => tracing::warn!(
                vendor = self.config.vendor,
                "vendor announced no thread id; ThreadStarted not reported"
            ),
        }
        Ok(())
    }

    /// Ship one ordered batch to the listener. Transient transport failures
    /// retry inside the client; a refusal (replaced token, vanished wave) or
    /// an exhausted retry ladder means the keeper is gone: the residency
    /// ends cleanly.
    async fn send(&mut self, deltas: Vec<ResidentDelta>) {
        if self.end.is_some() || deltas.is_empty() {
            return;
        }
        if let Err(err) = self.client.send_deltas(deltas).await {
            tracing::info!(
                error = %format!("{err:#}"),
                "listener unreachable; ending residency"
            );
            self.end = Some(MindEnd::ListenerGone);
        }
    }

    async fn on_inbox(&mut self, item: InboxItem) {
        match item {
            InboxItem::Message(message) => {
                // Replay dedup: a reconnect re-offers unconsumed messages.
                if !self.seen.insert(message.id.clone()) {
                    return;
                }
                match message.op {
                    // Human speech steers by default: an unattributed message
                    // (no byline — the human) takes the steer path, reaching
                    // the live turn when the harness can. Attributed
                    // emissions (worker reports, child-wave escalations)
                    // always wait for the boundary — colleagues interrupt
                    // you; status reports don't.
                    MessageOp::Message | MessageOp::Say if message.from.is_none() => {
                        self.on_steer(message).await
                    }
                    MessageOp::Message | MessageOp::Say => self.on_message(message).await,
                    MessageOp::Steer => self.on_steer(message).await,
                    MessageOp::Interrupt => self.on_interrupt(Some(message)).await,
                }
            }
            InboxItem::Interrupt => self.on_interrupt(None).await,
        }
    }

    async fn on_message(&mut self, message: PendingMessage) {
        self.queue.push(message);
        if self.started && !self.in_flight {
            self.start_queued_turn().await;
        }
        // else: turning — queued; the next boundary drains it.
    }

    /// Steer: inject into the live turn when there is one and the harness can
    /// steer; the current turn consumed the message (a `TurnSteered` delta —
    /// the listener journals it against the live turn, or the last assistant
    /// turn when the boundary raced the send). Otherwise — idle,
    /// interrupting, non-capable harness — degrade to a queued message.
    /// Serves explicit `steer` ops AND human-authored plain messages (human
    /// speech steers by default; see [`Mind::on_inbox`]).
    async fn on_steer(&mut self, message: PendingMessage) {
        let steerable = self.in_flight
            && self.interrupt_deadline.is_none()
            && self.harness.capabilities().supports_steer;
        if !steerable {
            self.on_message(message).await;
            return;
        }
        // Journal the consumption FIRST: the `TurnSteered` delta is the
        // durable claim, acked by the listener before the text reaches the
        // vendor. The reverse order redelivers — send_input succeeds, the
        // POST fails, and the pending fold replays the message to the
        // respawned resident's vendor a second time. At-most-once to the
        // vendor; the failure path below undoes the claim explicitly.
        self.send(vec![ResidentDelta::TurnSteered {
            answers: vec![message.id.0.clone()],
        }])
        .await;
        if self.end.is_some() {
            // Listener gone before the claim was acked: the pending fold
            // still holds the message for the next resident's replay.
            return;
        }
        if let Err(err) = self.harness.send_input(&message.text).await {
            // Consumption is journaled but the vendor never got the text:
            // undo the claim so the listener re-queues the message for the
            // respawned resident's replay.
            self.send(vec![ResidentDelta::MessagesRequeued {
                ids: vec![message.id.0.clone()],
            }])
            .await;
            self.fail(&format!("steer send_input failed: {err:#}"))
                .await;
        }
    }

    /// Interrupt: cancel the open turn (cooperative, deadline-bounded). Text
    /// riding the interrupt ("interrupt & send") is queued first so the
    /// post-interrupt boundary starts the next turn answering it. While idle
    /// there is nothing to cancel — a no-op, except that text starts the next
    /// turn immediately. While already interrupting, only the text queues.
    async fn on_interrupt(&mut self, message: Option<PendingMessage>) {
        if let Some(message) = message {
            self.queue.push(message);
        }
        if !self.in_flight {
            // Idle: no-op; "interrupt & send" text becomes the next turn now.
            if self.started && !self.queue.is_empty() {
                self.start_queued_turn().await;
            }
            return;
        }
        if self.interrupt_deadline.is_some() {
            return; // already interrupting; the boundary (or deadline) settles it
        }
        // Arm the force-close bound FIRST — at op receipt, never after the
        // vendor call returns. `harness.interrupt()` awaits the vendor, and
        // the listener's janitor fires 20 seconds after it broadcast the op
        // (`supervisor::LISTENER_INTERRUPT_DEADLINE`): this resident-side
        // deadline must fire before it however slow the vendor is, so the
        // await below is bounded by the same deadline instead of blocking
        // the scheduler indefinitely.
        let deadline = Instant::now() + self.config.interrupt_deadline;
        self.interrupt_deadline = Some(deadline);
        self.send(vec![ResidentDelta::MindState {
            to: ResidentStateTo::Interrupting,
            reason: "user interrupt".to_string(),
        }])
        .await;
        if self.end.is_some() {
            return;
        }
        match tokio::time::timeout_at(deadline, self.harness.interrupt()).await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                // Cooperative cancel failed; the deadline bounds the wait.
                tracing::warn!(error = %format!("{err:#}"), "harness interrupt failed; deadline armed");
            }
            Err(_) => {
                tracing::warn!(
                    deadline_secs = self.config.interrupt_deadline.as_secs_f64(),
                    "harness interrupt still pending at the deadline; the wire force-close follows"
                );
            }
        }
    }

    /// The harness swallowed the interrupt (no terminal event within the
    /// deadline): force-close THROUGH THE WIRE — the listener journals
    /// `TurnFinished{Interrupted}` and settles idle — reset the adapter so a
    /// late vendor terminal produces nothing, and drain the queue.
    async fn on_interrupt_deadline(&mut self) {
        self.interrupt_deadline = None;
        tracing::error!(
            wave = self.wave,
            deadline_secs = self.config.interrupt_deadline.as_secs_f64(),
            "interrupt deadline expired; force-closing the open turn as interrupted"
        );
        self.adapter = EventAdapter::new();
        self.usage_flush_at = None;
        self.send(vec![ResidentDelta::TurnFinished {
            status: Lifecycle::Interrupted,
            cost_usd: None,
        }])
        .await;
        self.in_flight = false;
        self.idle_since = Instant::now();
        if !self.queue.is_empty() {
            self.start_queued_turn().await;
        }
    }

    async fn on_event(&mut self, event: ConversationEvent) {
        let terminal_reason = match &event {
            ConversationEvent::Error { code, message } if is_terminal_harness_error(code) => {
                Some(format!("{code}: {message}"))
            }
            _ => None,
        };

        let deltas = self.adapter.feed(&event);
        // The usage-flush deadline tracks the adapter: armed while a
        // finished turn awaits its trailing usage, re-armed by any event,
        // gone the moment the boundary flushes.
        self.usage_flush_at = self
            .adapter
            .awaiting_usage()
            .then(|| Instant::now() + self.config.usage_flush);
        let boundary_status = self.ship_deltas(deltas).await;

        if let Some(reason) = terminal_reason {
            // The vendor session is gone; any open turn was finalized failed
            // above. This is a mind failure, not a turn failure.
            self.started = false;
            self.in_flight = false;
            self.fail(&format!("harness disconnected: {reason}")).await;
            return;
        }
        if let Some(status) = boundary_status {
            self.on_turn_boundary(status).await;
        } else if self.in_flight && !self.adapter.tracking() {
            if let ConversationEvent::TurnCompleted { status, .. } = &event {
                // Belt-and-suspenders for the wedge class: the vendor closed
                // a turn the adapter never saw open (e.g. an earlier spurious
                // error finalized the record while the input kept steering
                // into the live vendor turn). Without a boundary here,
                // `in_flight` would stay true forever — heartbeat and
                // interrupt both unreachable — so the vendor's completion is
                // treated as a synthetic boundary.
                tracing::warn!(
                    wave = self.wave,
                    status = ?status,
                    "vendor turn completed with no open turn; synthetic boundary"
                );
                self.on_turn_boundary(*status).await;
            }
        }
    }

    /// Inject the consumption declaration, ship one adapter batch, and
    /// return the boundary status when the batch finalized a turn.
    async fn ship_deltas(&mut self, mut deltas: Vec<ResidentDelta>) -> Option<Lifecycle> {
        let mut boundary_status = None;
        for delta in deltas.iter_mut() {
            match delta {
                ResidentDelta::TurnOpened { answers } => {
                    // The consumption declaration: what this turn's input
                    // consumed, decided when the input was sent.
                    *answers = std::mem::take(&mut self.pending_answers)
                        .into_iter()
                        .map(|id| id.0)
                        .collect();
                }
                ResidentDelta::TurnFinished { status, .. } => {
                    boundary_status = Some(*status);
                }
                _ => {}
            }
        }
        self.send(deltas).await;
        boundary_status
    }

    /// The vendor went silent after `TurnCompleted`, swallowing the trailing
    /// `TurnUsage`: flush the held `TurnFinished` (no usage) through the
    /// normal boundary path so the scheduler never wedges on it.
    async fn on_usage_flush(&mut self) {
        self.usage_flush_at = None;
        tracing::warn!(
            wave = self.wave,
            flush_secs = self.config.usage_flush.as_secs_f64(),
            "no trailing TurnUsage from the vendor; flushing the held turn boundary"
        );
        let deltas = self.adapter.flush_stalled();
        if let Some(status) = self.ship_deltas(deltas).await {
            self.on_turn_boundary(status).await;
        }
    }

    async fn on_turn_boundary(&mut self, status: Lifecycle) {
        self.in_flight = false;
        self.idle_since = Instant::now();
        // An interrupted turn finalized in time: the deadline stands down.
        self.interrupt_deadline = None;
        if status == Lifecycle::Failed {
            self.consecutive_failures += 1;
            if self.consecutive_failures >= MAX_CONSECUTIVE_TURN_FAILURES {
                self.fail(&format!(
                    "{MAX_CONSECUTIVE_TURN_FAILURES} consecutive turn failures"
                ))
                .await;
                return;
            }
        } else {
            self.consecutive_failures = 0;
        }
        if !self.queue.is_empty() {
            self.start_queued_turn().await;
        }
    }

    async fn on_heartbeat(&mut self) {
        let workers = self.fetch_in_flight().await;
        if self.end.is_some() {
            return;
        }
        let prompt = heartbeat_prompt(&workers);
        self.send_turn(prompt, Vec::new()).await;
    }

    /// A cron deadline fired: re-check which lines are due (the file may
    /// have changed while we slept), mark them fired, and open one system
    /// turn covering all of them.
    async fn on_cron(&mut self) {
        let now = Utc::now();
        let due: Vec<WaveCronDef> = read_crons(&self.origin_repo, &self.wave)
            .into_iter()
            .filter(|cron| {
                next_cron_fire(
                    &cron.schedule,
                    self.cron_last_fired.get(&cron_key(cron)).copied(),
                    now,
                )
                .is_some_and(|fire_at| fire_at <= now)
            })
            .collect();
        if due.is_empty() {
            return; // the schedule moved under us; the loop re-arms
        }
        for cron in &due {
            self.cron_last_fired.insert(cron_key(cron), now);
        }
        tracing::info!(
            wave = self.wave,
            flows = ?due.iter().map(|cron| cron.flow.as_str()).collect::<Vec<_>>(),
            "cron due; opening a system turn"
        );
        let prompt = cron_prompt(&due);
        self.send_turn(prompt, Vec::new()).await;
    }

    /// One context fetch before a turn: freshens the listener's store fold
    /// (it polls once to serve this) and returns the in-flight workers.
    async fn fetch_in_flight(&mut self) -> Vec<InFlightWorker> {
        match self.client.context().await {
            Ok(context) => context.in_flight,
            Err(err) => {
                tracing::info!(
                    error = %format!("{err:#}"),
                    "listener unreachable; ending residency"
                );
                self.end = Some(MindEnd::ListenerGone);
                Vec::new()
            }
        }
    }

    /// Drain the whole queue into one turn; the `TurnOpened` delta's answers
    /// will name every consumed message.
    async fn start_queued_turn(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.fetch_in_flight().await;
        if self.end.is_some() {
            return;
        }
        let messages = std::mem::take(&mut self.queue);
        let answers: Vec<MessageId> = messages.iter().map(|m| m.id.clone()).collect();
        // Attributed emissions carry their byline into the prompt so the mind
        // can tell a worker report from the human.
        let content = messages
            .iter()
            .map(|m| match &m.from {
                Some(from) => format!("[{}] {}", from.label, m.text),
                None => m.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !self.send_turn(content, answers).await {
            // Send failed: the messages were never consumed; keep them queued
            // (the listener's pending fold still has them — a respawned
            // resident's replay re-delivers).
            self.queue = messages;
        }
    }

    /// Send one turn's input. Returns whether the send was accepted; a send
    /// error fails the mind (the harness is broken, not the turn).
    async fn send_turn(&mut self, content: String, answers: Vec<MessageId>) -> bool {
        self.pending_answers = answers;
        match self.harness.send_input(&content).await {
            Ok(()) => {
                self.in_flight = true;
                true
            }
            Err(err) => {
                self.pending_answers.clear();
                self.fail(&format!("send_input failed: {err:#}")).await;
                false
            }
        }
    }

    /// The mind is dead: report it over the wire and end the loop. The
    /// process exits nonzero; the LISTENER's supervisor owns revival.
    async fn fail(&mut self, reason: &str) {
        if self.end.is_some() {
            return;
        }
        tracing::error!(
            wave = self.wave,
            reason,
            "wave mind failed; reporting and exiting (the listener's supervisor \
             respawns on its ladder, or immediately on a human message)"
        );
        self.in_flight = false;
        self.interrupt_deadline = None;
        self.send(vec![ResidentDelta::MindState {
            to: ResidentStateTo::Failed,
            reason: reason.to_string(),
        }])
        .await;
        // `send` may have ended us as ListenerGone; that outranks Failed
        // (nothing left to report to).
        if self.end.is_none() {
            self.end = Some(MindEnd::Failed(reason.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use crate::conversation::types::TurnUsage;
    use crate::harness::Capabilities;
    use crate::wave::journal::{journal_path, EventKind, Journal};
    use crate::wave::runtime::WaveRuntime;
    use crate::wave::server::{self, ResidentDoor};
    use crate::wave::state::MindState;

    /// Scriptless mock: records `send_input`/`interrupt`/
    /// `set_provider_session_id`; the TEST drives the event stream directly
    /// through the channel it created, so turn lifecycles are fully
    /// deterministic. `interrupt` records the call and (unless
    /// `hang_on_interrupt`) returns — a "responsive" harness is simulated by
    /// the test emitting the terminal event afterwards; a "swallowing"
    /// harness by not emitting it; a slow-vendor cancel by hanging the call
    /// itself. `fail_next_input` makes exactly one `send_input` refuse.
    struct MockHarness {
        inputs: Arc<Mutex<Vec<String>>>,
        seeded: Arc<Mutex<Option<String>>>,
        thread_id: String,
        starts: Arc<Mutex<u32>>,
        interrupts: Arc<Mutex<u32>>,
        supports_steer: bool,
        hang_on_interrupt: bool,
        fail_next_input: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl Harness for MockHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            *self.starts.lock().unwrap() += 1;
            Ok(())
        }
        async fn send_input(&mut self, content: &str) -> Result<()> {
            if std::mem::take(&mut *self.fail_next_input.lock().unwrap()) {
                return Err(anyhow!("mock send_input refused"));
            }
            self.inputs.lock().unwrap().push(content.to_string());
            Ok(())
        }
        async fn interrupt(&mut self) -> Result<()> {
            *self.interrupts.lock().unwrap() += 1;
            if self.hang_on_interrupt {
                std::future::pending::<()>().await
            }
            Ok(())
        }
        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_steer: self.supports_steer,
            }
        }
        fn provider_session_id(&self) -> Option<String> {
            Some(self.thread_id.clone())
        }
        fn set_provider_session_id(&mut self, provider_session_id: Option<String>) {
            *self.seeded.lock().unwrap() = provider_session_id;
        }
    }

    /// A full two-halves rig, in-process but over the REAL wire: a listener
    /// (runtime + router with the resident door) and a resident (subscription
    /// follower + `run_mind` with a mock harness) connected by HTTP.
    struct TestMind {
        runtime: Arc<WaveRuntime>,
        events: mpsc::UnboundedSender<ConversationEvent>,
        inputs: Arc<Mutex<Vec<String>>>,
        seeded: Arc<Mutex<Option<String>>>,
        starts: Arc<Mutex<u32>>,
        interrupts: Arc<Mutex<u32>>,
        fail_next_input: Arc<Mutex<bool>>,
        mind: tokio::task::JoinHandle<Result<()>>,
        /// The listener half runs on its OWN tokio runtime so a test can
        /// kill it for real: shutting the runtime down drops the accept loop
        /// AND every per-connection task (axum spawns those detached), which
        /// is what an actual dead listener process looks like on the wire.
        listener: Option<tokio::runtime::Runtime>,
        _tmp: tempfile::TempDir,
    }

    impl Drop for TestMind {
        fn drop(&mut self) {
            if let Some(rt) = self.listener.take() {
                // Non-blocking teardown; dropping a runtime inline would
                // panic inside the async test.
                rt.shutdown_background();
            }
        }
    }

    impl TestMind {
        fn journal_events(&self) -> Vec<EventKind> {
            let path = journal_path(self.runtime.repo_root(), "ship");
            let (_, events) = Journal::open(&path).expect("read journal");
            events.into_iter().map(|e| e.kind).collect()
        }

        /// Drive one whole turn from the vendor side.
        fn emit_turn(&self, text: &str, status: Lifecycle) {
            self.emit(ConversationEvent::TurnStarted {
                turn_id: "vt".into(),
            });
            if !text.is_empty() {
                self.emit(ConversationEvent::ItemCompleted {
                    turn_id: "vt".into(),
                    item: ConversationItem::Message {
                        id: "m".into(),
                        text: text.into(),
                        phase: None,
                    },
                });
            }
            self.emit(ConversationEvent::TurnCompleted {
                turn_id: "vt".into(),
                status,
            });
            self.emit(ConversationEvent::TurnUsage {
                turn_id: "vt".into(),
                usage: TurnUsage {
                    input_tokens: 1,
                    output_tokens: 1,
                    reasoning_tokens: None,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    model: None,
                    cost_usd: None,
                },
            });
        }

        fn emit(&self, event: ConversationEvent) {
            self.events.send(event).expect("mind alive");
        }

        fn input_count(&self) -> usize {
            self.inputs.lock().unwrap().len()
        }
    }

    /// Test-rig knobs; `Default` is a steer-capable harness with production
    /// deadlines and a far-away heartbeat.
    struct BootOptions {
        heartbeat: Duration,
        interrupt_deadline: Duration,
        usage_flush: Duration,
        supports_steer: bool,
        hang_on_interrupt: bool,
    }

    impl Default for BootOptions {
        fn default() -> Self {
            Self {
                heartbeat: Duration::from_secs(600),
                interrupt_deadline: INTERRUPT_DEADLINE,
                usage_flush: USAGE_FLUSH,
                supports_steer: true,
                hang_on_interrupt: false,
            }
        }
    }

    fn boot(heartbeat: Duration) -> impl std::future::Future<Output = TestMind> {
        boot_in(tempfile::tempdir().expect("tempdir"), heartbeat)
    }

    async fn boot_in(tmp: tempfile::TempDir, heartbeat: Duration) -> TestMind {
        boot_with(
            tmp,
            BootOptions {
                heartbeat,
                ..BootOptions::default()
            },
        )
        .await
    }

    async fn boot_with(tmp: tempfile::TempDir, options: BootOptions) -> TestMind {
        let config = MindConfig {
            vendor: "codex".to_string(),
            heartbeat_idle: options.heartbeat,
            interrupt_deadline: options.interrupt_deadline,
            usage_flush: options.usage_flush,
        };
        // The listener half: runtime + HTTP surface with the resident door,
        // served from a dedicated tokio runtime (see TestMind::listener).
        let runtime =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let door = ResidentDoor::new("test-token");
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std_listener.set_nonblocking(true).unwrap();
        let addr = std_listener.local_addr().unwrap();
        let app = server::router(runtime.clone(), door, None, None);
        let listener = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("listener runtime");
        listener.spawn(async move {
            let tcp = tokio::net::TcpListener::from_std(std_listener).expect("adopt listener");
            axum::serve(tcp, app).await.ok();
        });

        // The resident half: attach, subscribe, run the mind over the wire.
        let client = ListenerClient::new(addr.to_string(), "test-token".to_string());
        let attach = client.attach(std::process::id()).await.expect("attach");
        assert_eq!(attach.wave, "ship");
        let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
        tokio::spawn(crate::wave::resident::follow_inbox(
            addr.to_string(),
            inbox_tx,
        ));

        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let seeded = Arc::new(Mutex::new(None));
        let starts = Arc::new(Mutex::new(0));
        let interrupts = Arc::new(Mutex::new(0));
        let fail_next_input = Arc::new(Mutex::new(false));
        let harness = Box::new(MockHarness {
            inputs: inputs.clone(),
            seeded: seeded.clone(),
            thread_id: "thread-new".to_string(),
            starts: starts.clone(),
            interrupts: interrupts.clone(),
            supports_steer: options.supports_steer,
            hang_on_interrupt: options.hang_on_interrupt,
            fail_next_input: fail_next_input.clone(),
        });
        let mind = tokio::spawn(run_mind(
            client,
            inbox_rx,
            harness,
            events_rx,
            tmp.path().to_path_buf(),
            tmp.path().to_path_buf(),
            "ship".into(),
            attach.thread_id,
            config,
        ));
        TestMind {
            runtime,
            events: events_tx,
            inputs,
            seeded,
            starts,
            interrupts,
            fail_next_input,
            mind,
            listener: Some(listener),
            _tmp: tmp,
        }
    }

    async fn wait_for(what: &str, cond: impl Fn() -> bool) {
        for _ in 0..500 {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("condition not met in time: {what}");
    }

    fn started_answers(events: &[EventKind]) -> Vec<Vec<MessageId>> {
        events
            .iter()
            .filter_map(|kind| match kind {
                EventKind::TurnStarted { answers, .. } => Some(answers.clone()),
                _ => None,
            })
            .collect()
    }

    fn message_id(turn: &ChatTurn) -> MessageId {
        let seq = turn.id.strip_prefix("turn-").expect("user turn id");
        MessageId(format!("msg-{seq}"))
    }

    #[test]
    fn path_for_children_starts_with_this_executables_dir() {
        let exe_dir = std::env::current_exe()
            .expect("current exe")
            .parent()
            .expect("exe has a dir")
            .to_path_buf();
        let path = path_for_children();
        let first = std::env::split_paths(&path).next().expect("PATH non-empty");
        assert_eq!(
            first, exe_dir,
            "the mind's PATH resolves `lf` to this build first"
        );
    }

    // -- Adapter (harness events → wire deltas) --

    #[test]
    fn empty_reasoning_completion_yields_no_wire_item() {
        let mut adapter = EventAdapter::new();
        adapter.feed(&ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        // codex reasoning arrives as deltas; item/completed carries no text.
        let deltas = adapter.feed(&ConversationEvent::ItemCompleted {
            turn_id: "vt".into(),
            item: ConversationItem::Thought {
                id: "rsn_1".into(),
                text: String::new(),
            },
        });
        assert!(deltas.is_empty(), "empty thought produces no delta");

        adapter.feed(&ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        let deltas = adapter.feed(&ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        assert!(
            matches!(
                deltas.as_slice(),
                [
                    ResidentDelta::TurnUsage { .. },
                    ResidentDelta::TurnFinished {
                        status: Lifecycle::Completed,
                        ..
                    }
                ]
            ),
            "only usage + finished cross the wire: {deltas:?}"
        );
    }

    #[test]
    fn text_delta_only_turn_finishes_with_text() {
        let mut adapter = EventAdapter::new();
        adapter.feed(&ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        let deltas = adapter.feed(&ConversationEvent::TextDelta {
            turn_id: "vt".into(),
            content: "Hello from OpenCode".into(),
        });
        assert!(
            deltas.is_empty(),
            "streaming prose buffers until the turn boundary"
        );

        let deltas = adapter.feed(&ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        assert!(matches!(
            deltas.as_slice(),
            [ResidentDelta::TurnText { text }] if text == "Hello from OpenCode"
        ));

        let deltas = adapter.feed(&ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        assert!(matches!(
            deltas.last(),
            Some(ResidentDelta::TurnFinished {
                status: Lifecycle::Completed,
                ..
            })
        ));
    }

    #[test]
    fn final_message_item_wins_over_buffered_text_delta() {
        let mut adapter = EventAdapter::new();
        adapter.feed(&ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        adapter.feed(&ConversationEvent::TextDelta {
            turn_id: "vt".into(),
            content: "Hello from Codex".into(),
        });
        let message_deltas = adapter.feed(&ConversationEvent::ItemCompleted {
            turn_id: "vt".into(),
            item: ConversationItem::Message {
                id: "msg".into(),
                text: "Hello from Codex".into(),
                phase: None,
            },
        });
        assert!(matches!(
            message_deltas.as_slice(),
            [ResidentDelta::TurnText { text }] if text == "Hello from Codex"
        ));

        let completed_deltas = adapter.feed(&ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        assert!(
            completed_deltas.is_empty(),
            "buffered deltas are discarded once a final Message item arrived"
        );

        let deltas = adapter.feed(&ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        assert!(
            !deltas
                .iter()
                .any(|delta| matches!(delta, ResidentDelta::TurnText { .. })),
            "the prose crossed the wire exactly once"
        );
    }

    /// The mind's prompt carries the one shared loopflow operating document —
    /// exactly once, and the mind-specific discipline no longer duplicates it.
    #[test]
    fn mind_prompt_carries_the_shared_loopflow_document_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prompt = mind_agent_config(tmp.path(), "ship", tmp.path()).system_prompt;

        assert_eq!(
            prompt.matches("<lf:loopflow>").count(),
            1,
            "loopflow section appears exactly once"
        );
        assert!(
            prompt.contains("lf op commit"),
            "loopflow operating guidance"
        );
        assert!(prompt.contains("lf chat --parent"), "parent escalation");
        assert!(
            prompt.contains("lf memory update") && prompt.contains("lf memory add"),
            "memory curation"
        );
        assert!(prompt.contains("server-owned"), "the file is server-owned");

        let discipline = orchestration_discipline("ship");
        assert!(
            !discipline.contains("lf chat") && !discipline.contains("lf memory"),
            "the discipline keeps only mind-specific rules"
        );
    }

    // -- Scheduling, over the full wire --

    #[tokio::test]
    async fn message_while_idle_starts_a_turn_answering_it() {
        let mind = boot(Duration::from_secs(600)).await;
        let user_turn = mind
            .runtime
            .deliver_user_message("hello mind".into(), MessageOp::Message);
        wait_for("input sent", || mind.input_count() == 1).await;
        assert_eq!(mind.inputs.lock().unwrap()[0], "hello mind");

        mind.emit_turn("hi!", Lifecycle::Completed);
        wait_for("assistant turn", || {
            mind.runtime
                .thread_snapshot()
                .iter()
                .any(|t| t.role == ChatRole::Assistant && t.status == Lifecycle::Completed)
        })
        .await;

        let answers = started_answers(&mind.journal_events());
        assert_eq!(answers, vec![vec![message_id(&user_turn)]]);
    }

    /// A say emission wakes the mind like a message: the next turn's input
    /// carries the byline and its `TurnStarted.answers` consumes the id.
    #[tokio::test]
    async fn say_wakes_the_mind_and_is_consumed_by_the_next_turn() {
        let mind = boot(Duration::from_secs(600)).await;
        let turn = mind.runtime.deliver_say(
            "implement run-1 finished: PR #7, one surprise".into(),
            crate::wave::journal::Attribution {
                session_id: Some("sess-1".into()),
                label: "worker".into(),
            },
        );
        wait_for("input sent", || mind.input_count() == 1).await;
        assert_eq!(
            mind.inputs.lock().unwrap()[0],
            "[worker] implement run-1 finished: PR #7, one surprise",
            "the turn input carries the byline"
        );

        mind.emit_turn("noted", Lifecycle::Completed);
        wait_for("turn journaled", || {
            !started_answers(&mind.journal_events()).is_empty()
        })
        .await;
        assert_eq!(
            started_answers(&mind.journal_events())[0],
            vec![message_id(&turn)],
            "the say emission is consumed like any queued message"
        );
    }

    /// Human messages mid-turn on a NON-steer harness: queued, never
    /// rejected, and one boundary turn answers them all (the pre-steer
    /// default, still the rule when the harness can't steer).
    #[tokio::test]
    async fn messages_while_turning_coalesce_into_one_boundary_turn() {
        let mind = boot_with(
            tempfile::tempdir().expect("tempdir"),
            BootOptions {
                // supports_steer = false: human speech falls back to the queue
                supports_steer: false,
                ..BootOptions::default()
            },
        )
        .await;
        mind.runtime
            .deliver_user_message("first".into(), MessageOp::Message);
        wait_for("turn 1 sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        // Two messages land mid-turn: queued, never rejected. Give the SSE
        // hop time to reach the mind before the boundary (the biased select
        // then guarantees they're queued before the boundary drains).
        let m2 = mind
            .runtime
            .deliver_user_message("second".into(), MessageOp::Message);
        let m3 = mind
            .runtime
            .deliver_user_message("third".into(), MessageOp::Message);
        tokio::time::sleep(Duration::from_millis(150)).await;
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });

        // One boundary turn drains the whole queue.
        wait_for("boundary turn sent", || mind.input_count() == 2).await;
        let second_input = mind.inputs.lock().unwrap()[1].clone();
        assert!(second_input.contains("second") && second_input.contains("third"));

        mind.emit_turn("caught up", Lifecycle::Completed);
        wait_for("second TurnStarted journaled", || {
            started_answers(&mind.journal_events()).len() == 2
        })
        .await;
        let answers = started_answers(&mind.journal_events());
        assert_eq!(answers[1], vec![message_id(&m2), message_id(&m3)]);
    }

    // -- Cron: the third deadline --

    fn write_goal_with_crons(tmp: &std::path::Path, crons_yaml: &str) {
        let dir = tmp.join("wave/ship");
        std::fs::create_dir_all(&dir).expect("wave dir");
        std::fs::write(
            dir.join("GOAL.md"),
            format!("---\ncrons:\n{crons_yaml}---\nShip.\n"),
        )
        .expect("write GOAL.md");
    }

    #[test]
    fn next_cron_fire_honors_grace_last_fired_and_garbage() {
        let now = Utc::now();

        // Never fired, hourly schedule: an occurrence within the 24h grace
        // window is due (fire_at <= now).
        let due = next_cron_fire("0 0 * * * *", None, now).expect("hourly parses");
        assert!(due <= now, "an occurrence inside the grace window is due");

        // Fired moments ago: the next occurrence is in the future.
        let fired = next_cron_fire("0 0 * * * *", Some(now), now).expect("hourly parses");
        assert!(fired > now, "a just-fired schedule waits for the next slot");

        // Garbage never fires.
        assert!(next_cron_fire("not-a-cron", None, now).is_none());
    }

    #[test]
    fn cron_prompt_names_each_due_flow() {
        let due = vec![
            WaveCronDef {
                flow: "qa".into(),
                schedule: "* * * * * *".into(),
            },
            WaveCronDef {
                flow: "wave-polish".into(),
                schedule: "0 0 0 * * Mon *".into(),
            },
        ];
        assert_eq!(
            cron_prompt(&due),
            "cron due: qa — dispatch it\ncron due: wave-polish — dispatch it"
        );
    }

    /// A due schedule in GOAL.md frontmatter opens a system turn while idle.
    #[tokio::test]
    async fn cron_due_opens_a_system_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Every second: due immediately at boot (grace window), and the
        // heartbeat is far away so the first input is the cron's.
        write_goal_with_crons(tmp.path(), "  - flow: qa\n    schedule: '* * * * * *'\n");
        let mind = boot_in(tmp, Duration::from_secs(600)).await;

        wait_for("cron turn sent", || mind.input_count() >= 1).await;
        assert_eq!(mind.inputs.lock().unwrap()[0], "cron due: qa — dispatch it");
    }

    /// A schedule with no occurrence between the grace window and now stays
    /// quiet — no turn opens.
    #[tokio::test]
    async fn cron_not_due_stays_quiet() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_goal_with_crons(
            tmp.path(),
            "  - flow: qa\n    schedule: '0 0 0 1 1 * 2099'\n",
        );
        let mind = boot_in(tmp, Duration::from_secs(600)).await;

        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(mind.input_count(), 0, "nothing due, nothing fired");
    }

    #[tokio::test]
    async fn heartbeat_fires_when_idle_and_not_while_turning() {
        let mind = boot(Duration::from_millis(50)).await;
        // Quiet wave: the heartbeat starts a progress turn with the nudge.
        wait_for("heartbeat turn", || mind.input_count() == 1).await;
        assert_eq!(mind.inputs.lock().unwrap()[0], HEARTBEAT_PROMPT);

        // While the turn runs (never completes), no further heartbeat fires.
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(mind.input_count(), 1, "no heartbeat while turning");

        // After the boundary, idle resumes and the next heartbeat fires.
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        wait_for("next heartbeat", || mind.input_count() >= 2).await;
    }

    #[tokio::test]
    async fn heartbeat_carries_in_flight_workers_when_present() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Journal the observations before the mind boots so the first
        // heartbeat deterministically sees them (served by /resident/context).
        {
            let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
            assert!(runtime.journal_run_observed(
                "run-7",
                "sess-7",
                "implement",
                "wire the observation tail",
            ));
            assert!(runtime.journal_run_observed("run-8", "sess-8", "design", "next item"));
            // Multi-line task: must flatten to one line in the prompt.
            assert!(runtime.journal_run_observed(
                "run-9",
                "sess-9",
                "design",
                "multi-line task\n  with an indented second line",
            ));
            assert!(runtime.journal_run_completed(
                "run-8",
                crate::wave::journal::WorkerOutcome::Completed,
                "landed",
            ));
        }

        let mind = boot_in(tmp, Duration::from_millis(50)).await;
        wait_for("heartbeat turn", || mind.input_count() == 1).await;
        let prompt = mind.inputs.lock().unwrap()[0].clone();
        assert!(prompt.starts_with(HEARTBEAT_PROMPT));
        assert!(prompt.contains("<in_flight>"), "in-flight section present");
        assert!(prompt.contains("run run-7 · implement: wire the observation tail · running"));
        assert!(
            !prompt.contains("run-8"),
            "finished workers are not in flight"
        );
        assert!(
            prompt.contains(
                "run run-9 · design: multi-line task with an indented second line · running"
            ),
            "multi-line tasks flatten to one in-flight line"
        );
    }

    /// The failure cap ends the RESIDENT: `run_mind` returns an error after
    /// reporting `MindState::Failed` over the wire. No in-process limbo —
    /// revival is the listener supervisor's respawn (tested in supervisor.rs).
    #[tokio::test]
    async fn failure_cap_reports_failed_and_exits_the_resident() {
        let mut mind = boot(Duration::from_millis(30)).await;
        for round in 1..=MAX_CONSECUTIVE_TURN_FAILURES as usize {
            wait_for("next turn sent", || mind.input_count() == round).await;
            mind.emit_turn("boom", Lifecycle::Failed);
            if round < MAX_CONSECUTIVE_TURN_FAILURES as usize {
                wait_for("back to idle", || {
                    mind.runtime.mind_state() == MindState::Idle
                })
                .await;
            }
        }
        // The listener's journal shows the reported failure…
        wait_for("mind failed", || {
            matches!(mind.runtime.mind_state(), MindState::Failed { .. })
        })
        .await;
        let MindState::Failed { reason } = mind.runtime.mind_state() else {
            unreachable!()
        };
        assert!(reason.contains("consecutive turn failures"), "{reason}");

        // …and the resident's loop ends with that error (process exits 1).
        let outcome = tokio::time::timeout(Duration::from_secs(5), &mut mind.mind)
            .await
            .expect("mind task ends")
            .expect("mind task not cancelled");
        let err = outcome.expect_err("mind failure is an error exit");
        assert!(err.to_string().contains("consecutive turn failures"));
    }

    /// The listener disappearing ends the residency CLEANLY: the subscription
    /// closes, `run_mind` returns Ok — the keeper is gone, nothing to revive
    /// from this side (tmux/systemd restarts are the human's arrangement).
    #[tokio::test]
    async fn listener_death_ends_the_resident_cleanly() {
        let mut mind = boot(Duration::from_secs(600)).await;
        wait_for("started", || *mind.starts.lock().unwrap() == 1).await;

        // Kill the listener for real: its runtime goes down, every live
        // connection with it.
        mind.listener
            .take()
            .expect("listener alive")
            .shutdown_background();
        // A killed listener is indistinguishable from a restarting one until
        // the retry ladder (LISTENER_RETRY_DELAYS, ~15s) exhausts — so clean
        // exit takes the full ladder by design. Timeout must clear it with CI
        // headroom.
        let outcome = tokio::time::timeout(Duration::from_secs(30), &mut mind.mind)
            .await
            .expect("mind task ends after listener death")
            .expect("mind task not cancelled");
        assert!(
            outcome.is_ok(),
            "listener death is a clean exit: {outcome:?}"
        );
    }

    #[tokio::test]
    async fn thread_started_is_journaled_before_the_first_turn() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("go".into(), MessageOp::Message);
        wait_for("input sent", || mind.input_count() == 1).await;
        mind.emit_turn("going", Lifecycle::Completed);
        wait_for("turn journaled", || {
            !started_answers(&mind.journal_events()).is_empty()
        })
        .await;

        let events = mind.journal_events();
        let thread_pos = events
            .iter()
            .position(|k| matches!(k, EventKind::ThreadStarted { .. }))
            .expect("ThreadStarted journaled");
        let turn_pos = events
            .iter()
            .position(|k| matches!(k, EventKind::TurnStarted { .. }))
            .expect("TurnStarted journaled");
        assert!(
            thread_pos < turn_pos,
            "borrowed-handle rule: the vendor thread id is durable before the first turn"
        );
        assert!(matches!(
            &events[thread_pos],
            EventKind::ThreadStarted { vendor, thread_id }
                if vendor == "codex" && thread_id == "thread-new"
        ));
    }

    #[tokio::test]
    async fn boot_with_existing_journal_seeds_resume_and_journals_the_new_thread() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // A previous life: a thread id and a finished turn in the journal.
        {
            let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
            runtime.journal_thread_started("codex", "thread-old");
            runtime.append_finalized_turn(
                ChatTurn {
                    id: String::new(),
                    role: ChatRole::Assistant,
                    text: "from the first life".into(),
                    status: Lifecycle::Completed,
                    items: Vec::new(),
                    created_at: String::new(),
                    from: None,
                },
                Vec::new(),
            );
        }

        let mind = boot_in(tmp, Duration::from_secs(600)).await;
        // The previous id was offered for resume (the attach response carried
        // it; the codex driver ignores it — documented cold start — but the
        // seam is exercised)…
        wait_for("resume seeded", || {
            mind.seeded.lock().unwrap().as_deref() == Some("thread-old")
        })
        .await;
        // …and the actually-announced thread is journaled: continuity breaks
        // are explicit, never silent.
        wait_for("new ThreadStarted", || {
            mind.runtime.last_thread_id().as_deref() == Some("thread-new")
        })
        .await;

        // The replayed thread is intact under the new thread.
        let thread = mind.runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "from the first life");
        assert_eq!(*mind.starts.lock().unwrap(), 1);
    }

    /// Unanswered messages journaled before a resident restart reach the
    /// fresh resident through the subscription's pending replay.
    #[tokio::test]
    async fn boot_with_unanswered_messages_replays_them_to_the_mind() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let expected_message_id = {
            let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
            let turn = runtime
                .deliver_user_message("answer this after restart".into(), MessageOp::Message);
            message_id(&turn)
        };

        let mind = boot_in(tmp, Duration::from_secs(600)).await;
        wait_for("pending message sent", || mind.input_count() == 1).await;
        assert_eq!(mind.inputs.lock().unwrap()[0], "answer this after restart");

        mind.emit_turn("answered after restart", Lifecycle::Completed);
        wait_for("answer marked consumed", || {
            started_answers(&mind.journal_events()).len() == 1
        })
        .await;
        assert_eq!(
            started_answers(&mind.journal_events())[0],
            vec![expected_message_id]
        );
    }

    // -- Steering --

    fn steered_answers(events: &[EventKind]) -> Vec<(String, Vec<MessageId>)> {
        events
            .iter()
            .filter_map(|kind| match kind {
                EventKind::TurnSteered { turn_id, answers } => {
                    Some((turn_id.clone(), answers.clone()))
                }
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn steer_mid_turn_reaches_the_live_turn_and_is_answered_by_it() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        // Steer mid-turn: injected into the live turn, not queued.
        let steer = mind
            .runtime
            .deliver_user_message("focus on the parser".into(), MessageOp::Steer);
        wait_for("steer injected", || mind.input_count() == 2).await;
        assert_eq!(mind.inputs.lock().unwrap()[1], "focus on the parser");

        // Consumption: the CURRENT turn answers it, via TurnSteered.
        wait_for("TurnSteered journaled", || {
            !steered_answers(&mind.journal_events()).is_empty()
        })
        .await;
        let steered = steered_answers(&mind.journal_events());
        assert_eq!(steered[0].1, vec![message_id(&steer)]);
        let open_turn_id = mind
            .runtime
            .thread_snapshot()
            .iter()
            .find(|t| t.status == Lifecycle::Running)
            .expect("open turn")
            .id
            .clone();
        assert_eq!(steered[0].0, open_turn_id, "answered by the current turn");

        // The boundary drains nothing: the steer was consumed mid-turn.
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        wait_for("back to idle", || {
            mind.runtime.mind_state() == MindState::Idle
        })
        .await;
        // No boundary turn for a steered message (give the wire a beat).
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            mind.input_count(),
            2,
            "no boundary turn for a steered message"
        );
    }

    #[tokio::test]
    async fn steer_degrades_to_queue_on_a_non_capable_harness() {
        let mind = boot_with(
            tempfile::tempdir().expect("tempdir"),
            BootOptions {
                supports_steer: false,
                ..BootOptions::default()
            },
        )
        .await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        let steer = mind
            .runtime
            .deliver_user_message("focus on the parser".into(), MessageOp::Steer);
        // Queued, not injected: no send until the boundary.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(
            mind.input_count(),
            1,
            "degraded steer waits for the boundary"
        );

        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        wait_for("boundary turn sent", || mind.input_count() == 2).await;
        assert_eq!(mind.inputs.lock().unwrap()[1], "focus on the parser");
        mind.emit_turn("refocused", Lifecycle::Completed);
        wait_for("boundary turn journaled", || {
            started_answers(&mind.journal_events()).len() == 2
        })
        .await;
        // Consumption is the normal boundary marker, not TurnSteered.
        let answers = started_answers(&mind.journal_events());
        assert_eq!(answers[1], vec![message_id(&steer)]);
        assert!(steered_answers(&mind.journal_events()).is_empty());
    }

    /// Human speech steers by default: a plain `message` op with no byline,
    /// arriving mid-turn on a steer-capable harness, is injected into the
    /// live turn (consumption via `TurnSteered`) — no boundary queue entry.
    #[tokio::test]
    async fn human_message_mid_turn_steers_the_live_turn() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        // A HUMAN message mid-turn: injected now, exactly like a steer.
        let message = mind
            .runtime
            .deliver_user_message("also check the tests".into(), MessageOp::Message);
        wait_for("message injected", || mind.input_count() == 2).await;
        assert_eq!(mind.inputs.lock().unwrap()[1], "also check the tests");

        // Consumption: the CURRENT turn answers it, via TurnSteered.
        wait_for("TurnSteered journaled", || {
            !steered_answers(&mind.journal_events()).is_empty()
        })
        .await;
        assert_eq!(
            steered_answers(&mind.journal_events())[0].1,
            vec![message_id(&message)]
        );

        // The boundary drains nothing: the message was consumed mid-turn.
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        wait_for("back to idle", || {
            mind.runtime.mind_state() == MindState::Idle
        })
        .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            mind.input_count(),
            2,
            "no boundary turn for a steered human message"
        );
    }

    /// The discrimination is the point: an ATTRIBUTED emission (a worker's
    /// say) arriving mid-turn on the same steer-capable harness still waits
    /// for the boundary — colleagues interrupt you; status reports don't.
    #[tokio::test]
    async fn worker_say_mid_turn_still_queues_for_the_boundary() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        let say = mind.runtime.deliver_say(
            "run-1 finished: PR #7".into(),
            crate::wave::journal::Attribution {
                session_id: Some("sess-1".into()),
                label: "worker".into(),
            },
        );
        // Queued, not injected: no send until the boundary.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(mind.input_count(), 1, "worker say waits for the boundary");

        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        wait_for("boundary turn sent", || mind.input_count() == 2).await;
        assert_eq!(
            mind.inputs.lock().unwrap()[1],
            "[worker] run-1 finished: PR #7"
        );
        mind.emit_turn("noted", Lifecycle::Completed);
        wait_for("boundary turn journaled", || {
            started_answers(&mind.journal_events()).len() == 2
        })
        .await;
        // Consumption is the boundary marker, never TurnSteered.
        let answers = started_answers(&mind.journal_events());
        assert_eq!(answers[1], vec![message_id(&say)]);
        assert!(steered_answers(&mind.journal_events()).is_empty());
    }

    // -- Interrupting --

    #[tokio::test]
    async fn interrupt_finalizes_the_turn_interrupted_and_settles_idle() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        mind.runtime.deliver_interrupt();
        wait_for("harness interrupt called", || {
            *mind.interrupts.lock().unwrap() == 1
        })
        .await;
        wait_for("interrupting reported over the wire", || {
            mind.runtime.mind_state().name() == "interrupting"
        })
        .await;

        // The harness cancels cooperatively: terminal event arrives.
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Interrupted,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        wait_for("idle", || mind.runtime.mind_state() == MindState::Idle).await;

        // The turn is a well-formed interrupted record, and the journal walked
        // Turning → Interrupting → Idle.
        let thread = mind.runtime.thread_snapshot();
        assert_eq!(
            thread.last().unwrap().status,
            Lifecycle::Interrupted,
            "partial turn finalized as a value, not a crash"
        );
        let path: Vec<(String, String)> = mind
            .journal_events()
            .iter()
            .filter_map(|kind| match kind {
                EventKind::MindState { from, to, .. } => {
                    Some((from.name().to_string(), to.name().to_string()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            path,
            vec![
                ("idle".to_string(), "turning".to_string()),
                ("turning".to_string(), "interrupting".to_string()),
                ("interrupting".to_string(), "idle".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn interrupt_and_send_starts_the_next_turn_answering_the_text() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        let sent = mind
            .runtime
            .deliver_user_message("drop that; fix the build".into(), MessageOp::Interrupt);
        wait_for("harness interrupt called", || {
            *mind.interrupts.lock().unwrap() == 1
        })
        .await;
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Interrupted,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });

        // The interrupt's text starts the next turn immediately after Idle…
        wait_for("next turn sent", || mind.input_count() == 2).await;
        assert_eq!(mind.inputs.lock().unwrap()[1], "drop that; fix the build");
        // …and its TurnStarted.answers names it.
        mind.emit_turn("on it", Lifecycle::Completed);
        wait_for("next turn journaled", || {
            started_answers(&mind.journal_events()).len() == 2
        })
        .await;
        let answers = started_answers(&mind.journal_events());
        assert_eq!(answers[1], vec![message_id(&sent)]);
    }

    #[tokio::test]
    async fn interrupt_while_idle_is_a_noop() {
        let mind = boot(Duration::from_secs(600)).await;
        wait_for("started", || *mind.starts.lock().unwrap() == 1).await;

        mind.runtime.deliver_interrupt();
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(mind.runtime.mind_state(), MindState::Idle);
        assert_eq!(*mind.interrupts.lock().unwrap(), 0, "nothing to cancel");
        assert_eq!(mind.input_count(), 0, "no turn started");
    }

    /// The RESIDENT-side deadline: a harness that swallows the cancel is
    /// force-closed THROUGH THE WIRE — the listener journals
    /// `TurnFinished{Interrupted}` — and the mind lives on. (The listener's
    /// own, longer janitor for a fully-silent resident is supervisor.rs's.)
    #[tokio::test]
    async fn interrupt_deadline_forces_the_turn_closed_when_the_harness_swallows_it() {
        let mind = boot_with(
            tempfile::tempdir().expect("tempdir"),
            BootOptions {
                interrupt_deadline: Duration::from_millis(50),
                ..BootOptions::default()
            },
        )
        .await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        mind.runtime.deliver_interrupt();
        wait_for("harness interrupt called", || {
            *mind.interrupts.lock().unwrap() == 1
        })
        .await;

        // No terminal event — the resident's deadline force-closes over the
        // wire at ~50ms.
        wait_for("forced idle", || {
            mind.runtime.mind_state() == MindState::Idle
        })
        .await;
        let thread = mind.runtime.thread_snapshot();
        assert_eq!(thread.last().unwrap().status, Lifecycle::Interrupted);
        let finished: Vec<Lifecycle> = mind
            .journal_events()
            .iter()
            .filter_map(|kind| match kind {
                EventKind::TurnFinished { status, .. } => Some(*status),
                _ => None,
            })
            .collect();
        assert_eq!(finished, vec![Lifecycle::Interrupted]);

        // The mind is live again: a new message starts a fresh turn.
        mind.runtime
            .deliver_user_message("still there?".into(), MessageOp::Message);
        wait_for("fresh turn sent", || mind.input_count() == 2).await;
    }

    /// The deadline arms at op receipt, not after `harness.interrupt()`
    /// returns: a vendor whose cancel call HANGS still gets force-closed at
    /// the resident's bound (well inside the listener's 20s janitor). Under
    /// the old order the deadline armed only after the unbounded await, so a
    /// hung cancel wedged the whole scheduler.
    #[tokio::test]
    async fn interrupt_deadline_fires_even_when_the_cancel_call_hangs() {
        let mind = boot_with(
            tempfile::tempdir().expect("tempdir"),
            BootOptions {
                interrupt_deadline: Duration::from_millis(50),
                hang_on_interrupt: true,
                ..BootOptions::default()
            },
        )
        .await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        mind.runtime.deliver_interrupt();
        wait_for("harness interrupt called", || {
            *mind.interrupts.lock().unwrap() == 1
        })
        .await;

        // The cancel call never returns — the deadline force-closes anyway.
        wait_for("forced idle despite the hung cancel", || {
            mind.runtime.mind_state() == MindState::Idle
        })
        .await;
        assert_eq!(
            mind.runtime.thread_snapshot().last().unwrap().status,
            Lifecycle::Interrupted
        );

        // The mind is live again: a new message starts a fresh turn.
        mind.runtime
            .deliver_user_message("still there?".into(), MessageOp::Message);
        wait_for("fresh turn sent", || mind.input_count() == 2).await;
    }

    /// Steer consumption is journaled BEFORE the vendor send (at-most-once
    /// to the vendor): when `send_input` then fails, the `TurnSteered` claim
    /// is already durable — the old order would have skipped it and let the
    /// pending fold redeliver the message to the vendor after respawn — and
    /// the resident undoes the claim with a `MessagesRequeued` delta before
    /// failing the mind.
    #[tokio::test]
    async fn steer_journals_consumption_before_the_vendor_and_requeues_on_failure() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;

        *mind.fail_next_input.lock().unwrap() = true;
        let steer = mind
            .runtime
            .deliver_user_message("focus on the parser".into(), MessageOp::Steer);

        // The failed vendor send fails the mind…
        wait_for("mind failed", || {
            matches!(mind.runtime.mind_state(), MindState::Failed { .. })
        })
        .await;
        let MindState::Failed { reason } = mind.runtime.mind_state() else {
            unreachable!()
        };
        assert!(reason.contains("steer send_input failed"), "{reason}");

        // …but the consumption claim crossed the wire FIRST: TurnSteered is
        // journaled even though the vendor never saw the text.
        let steered = steered_answers(&mind.journal_events());
        assert_eq!(steered.len(), 1, "consumption journaled before the send");
        assert_eq!(steered[0].1, vec![message_id(&steer)]);
        assert_eq!(mind.input_count(), 1, "the vendor never got the steer");
    }

    // -- The usage wedge --

    /// A vendor that emits `TurnCompleted` and then goes fully silent (no
    /// trailing `TurnUsage`) must not wedge the scheduler: the held
    /// `TurnFinished` flushes (without usage) at the flush deadline and the
    /// mind keeps scheduling.
    #[tokio::test]
    async fn vendor_silence_after_completion_flushes_the_boundary() {
        let mind = boot_with(
            tempfile::tempdir().expect("tempdir"),
            BootOptions {
                usage_flush: Duration::from_millis(80),
                ..BootOptions::default()
            },
        )
        .await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        mind.emit(ConversationEvent::ItemCompleted {
            turn_id: "vt".into(),
            item: ConversationItem::Message {
                id: "m".into(),
                text: "done".into(),
                phase: None,
            },
        });
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        // Total silence: no TurnUsage ever arrives.

        wait_for("boundary flushed without usage", || {
            mind.runtime
                .thread_snapshot()
                .last()
                .is_some_and(|t| t.status == Lifecycle::Completed)
                && mind.runtime.mind_state() == MindState::Idle
        })
        .await;

        // Unwedged: the next message starts a fresh turn.
        mind.runtime
            .deliver_user_message("next".into(), MessageOp::Message);
        wait_for("fresh turn sent", || mind.input_count() == 2).await;
    }

    // -- Wedge escape --

    /// The belt-and-suspenders escape: a vendor `turn/completed` for a turn
    /// the adapter never saw open must still reach `on_turn_boundary`, or
    /// `in_flight` sticks true forever (heartbeat and interrupt both gated
    /// off). Models the verified cascade: a spurious mid-turn error finalized
    /// the turn record, the next input steered into the still-running vendor
    /// turn, and the real completion then found nothing open.
    #[tokio::test]
    async fn synthetic_boundary_unwedges_a_completion_with_no_open_turn() {
        let mind = boot(Duration::from_secs(600)).await;
        mind.runtime
            .deliver_user_message("start".into(), MessageOp::Message);
        wait_for("turn sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });
        wait_for("turning", || mind.runtime.mind_state().name() == "turning").await;
        // Spurious non-terminal error: the adapter finalizes the turn record
        // as failed while the vendor turn keeps running.
        mind.emit(ConversationEvent::Error {
            code: "codex_error".into(),
            message: "spurious".into(),
        });
        wait_for("record finalized", || {
            mind.runtime.mind_state() == MindState::Idle
        })
        .await;

        // The next message sends input into the vendor's still-running turn:
        // in_flight goes true, but no vendor TurnStarted will ever arrive.
        mind.runtime
            .deliver_user_message("second".into(), MessageOp::Message);
        wait_for("second input sent", || mind.input_count() == 2).await;

        // The vendor's REAL completion of its turn: the adapter has nothing
        // open — the synthetic boundary must clear in_flight anyway.
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });

        // Unwedged: a fresh message starts a fresh turn immediately.
        mind.runtime
            .deliver_user_message("third".into(), MessageOp::Message);
        wait_for("scheduler unwedged: third turn sent", || {
            mind.input_count() == 3
        })
        .await;
    }
}
