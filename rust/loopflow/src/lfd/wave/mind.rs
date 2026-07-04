//! The wave's mind: one persistent vendor thread, scheduled by events.
//!
//! This replaces the old one-shot pipeline (`codex exec` per pass) and the
//! canned chat reply with a single long-lived harness session — the codex
//! app-server, driven through the [`Harness`] trait. The mind's identity is
//! its operating prompt (the rendered `GOAL.md` seed plus the orchestration
//! discipline); everything it says flows through the existing
//! [`TurnSink`] vocabulary, so journaling, the open-turn snapshot, SSE
//! broadcast, and `MindState` transitions are unchanged.
//!
//! # Scheduling
//! - **Message while idle** → a turn starts now; its `TurnStarted.answers`
//!   names the message plus anything already queued.
//! - **Message while turning** → queued (append-and-coalesce, never
//!   rejected); at the turn boundary one turn drains the whole queue and
//!   `answers` names every consumed id.
//! - **Steer while turning**, on a steer-capable harness → injected into the
//!   live turn (`send_input` mid-turn; codex maps it to `turn/steer`). The
//!   current turn consumed it, journaled as `TurnSteered.answers`. On a
//!   non-capable harness, while interrupting, or while idle, a steer degrades
//!   to a queued message — the `UserMessage{op: Steer}` row records the
//!   intent either way.
//! - **Interrupt while turning** → `Turning → Interrupting`, cooperative
//!   `harness.interrupt()`; the harness finalizes the open turn
//!   `Interrupted` (codex reports `turn/completed{interrupted}`) and the
//!   boundary settles the mind to `Idle`. Text on the interrupt ("interrupt &
//!   send") is queued first, so the post-interrupt boundary starts the next
//!   turn with `answers` naming it. If the harness never delivers the
//!   terminal event within [`INTERRUPT_DEADLINE`], the janitor force-path
//!   fires: journal `TurnFinished{Interrupted}`, settle to `Idle`, log
//!   loudly.
//! - **Interrupt while idle** → no-op (nothing to cancel); text, if any,
//!   starts the next turn immediately.
//! - **Heartbeat**: idle for [`HEARTBEAT_IDLE`] with an empty queue → a
//!   progress turn carrying a compact nudge ([`HEARTBEAT_PROMPT`]). Only the
//!   first turn of a thread carries the full seed — the codex driver prepends
//!   the `AgentConfig` system prompt to the first turn's input; the
//!   persistent thread retains context after that.
//!
//! The scheduler's select is `biased` toward the inbox: a message that
//! arrived before a turn boundary is always queued before the boundary
//! drains, so coalescing is deterministic.
//!
//! # Failure
//! A failed turn is `TurnFinished { status: Failed }` and the mind returns to
//! `Idle`. [`MAX_CONSECUTIVE_TURN_FAILURES`] consecutive failures (or a
//! terminal harness error) move the mind to `MindState::Failed` and stop the
//! heartbeat. The next user message revives it (`Failed → Idle`, allowed by
//! the state table) — a human talking to the wave brings it back, restarting
//! the vendor thread if it died. Algedonic attention wiring is a later phase.
//!
//! # Resume
//! On boot the last journaled `ThreadStarted` id is seeded via
//! [`Harness::set_provider_session_id`]. The codex driver does NOT honor
//! this for app-server threads — `CodexHarness::start` clears the slot and
//! always issues a fresh `thread/start` — so on codex every boot is a cold
//! start. We seed anyway (drivers that take resume state honor it), then
//! journal the id the vendor actually announced as a new `ThreadStarted`
//! row: the break in continuity is explicit in the log, never silently
//! papered over. The journal replay still restores the full visible thread.
//!
//! # cwd
//! The mind runs in the repo root the server was started from (run `lf wave`
//! from the wave's worktree, per loopflow discipline). Main-checkout
//! protection and worktree bootstrap are still to come.
//! Approval policy is `AutoApprove` until Decisions land.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::engine::agent::AgentConfig;
use crate::engine::flow::{available_flow_names, load_goal, render_goal, GoalRenderContext};
use crate::lfd::conversations::harness::{is_terminal_harness_error, Harness};
use crate::lfd::conversations::turns::{ChatRole, ChatTurn, TurnDelta};
use crate::lfd::conversations::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::lfd::wave::journal::{MessageId, MessageOp};
use crate::lfd::wave::memory::Memory;
use crate::lfd::wave::runtime::{InboxItem, TurnSink, UserMessage, WaveRuntime};
use crate::lfd::wave::state::MindState;

/// How long the mind sits idle (empty queue, no turn) before a heartbeat
/// turn. Each heartbeat burns a subscription turn on the vendor plan, so the
/// quiet-wave cadence is deliberately coarse; messages and turn boundaries
/// drive the mind the rest of the time.
pub const HEARTBEAT_IDLE: Duration = Duration::from_secs(300);

/// Consecutive failed turns before the mind itself is declared `Failed` and
/// the heartbeat stops. A user message resets the count and revives the mind.
pub const MAX_CONSECUTIVE_TURN_FAILURES: u32 = 3;

/// How long `Interrupting` may last before the janitor force-path fires. The
/// harness's cooperative cancel should finalize the turn in well under this;
/// past the deadline the open turn is force-finalized `Interrupted` and the
/// mind settles to `Idle` — the one transient state the mind could otherwise
/// wedge in (the HumanLayer stuck-`interrupting` lesson).
pub const INTERRUPT_DEADLINE: Duration = Duration::from_secs(10);

/// Compact nudge for heartbeat turns. The thread is persistent, so heartbeats
/// never re-send the seed — the first turn carried it.
const HEARTBEAT_PROMPT: &str = "Heartbeat: re-read your goal and memory, then take the next \
     orchestration step. If nothing needs doing, say so in one line.";

/// Longest task excerpt carried per worker in the `<in_flight>` section —
/// enough to recognize the dispatch, token-lean by design.
const IN_FLIGHT_TASK_CHARS: usize = 80;

/// The heartbeat nudge, plus a compact `<in_flight>` section when workers are
/// grinding: one line per dispatched-not-finished worker (run id, flow, task
/// excerpt, status), folded from the journal's lfd observations — the mind's
/// orchestration turns see their workers without re-reading transcripts.
pub fn heartbeat_prompt(runtime: &WaveRuntime) -> String {
    let workers = runtime.in_flight_workers();
    if workers.is_empty() {
        return HEARTBEAT_PROMPT.to_string();
    }
    let mut prompt = String::from(HEARTBEAT_PROMPT);
    prompt.push_str("\n\n<in_flight>\n");
    for worker in &workers {
        let task: String = if worker.task.chars().count() > IN_FLIGHT_TASK_CHARS {
            let mut excerpt: String = worker.task.chars().take(IN_FLIGHT_TASK_CHARS).collect();
            excerpt.push('…');
            excerpt
        } else {
            worker.task.clone()
        };
        prompt.push_str(&format!(
            "- run {} · {}: {} · running\n",
            worker.run_id, worker.flow, task
        ));
    }
    prompt.push_str("</in_flight>");
    prompt
}

/// Scheduler knobs. `Default` is production: codex vendor, 5-minute
/// heartbeat, 10-second interrupt deadline.
#[derive(Debug, Clone)]
pub struct MindConfig {
    /// Vendor label journaled in `ThreadStarted`.
    pub vendor: String,
    /// Idle window before a heartbeat turn (see [`HEARTBEAT_IDLE`]).
    pub heartbeat_idle: Duration,
    /// Janitor bound on `Interrupting` (see [`INTERRUPT_DEADLINE`]).
    pub interrupt_deadline: Duration,
}

impl Default for MindConfig {
    fn default() -> Self {
        Self {
            vendor: "codex".to_string(),
            heartbeat_idle: HEARTBEAT_IDLE,
            interrupt_deadline: INTERRUPT_DEADLINE,
        }
    }
}

/// PATH for the mind's harness and every child the server spawns: this
/// executable's directory first, so the discipline commands (`lf q worker
/// run …`) resolve to the binary running this wave server, never whatever
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
/// prepends to the first turn of the thread.
pub fn mind_agent_config(runtime: &WaveRuntime, cwd: &Path) -> AgentConfig {
    let seed = build_goal_seed(runtime.repo_root(), runtime.name(), runtime.memory());
    AgentConfig {
        system_prompt: format!(
            "{seed}\n\n{}\n\n{}",
            orchestration_discipline(runtime.name()),
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
                roadmap: String::new(),
                memory: memory.read(),
                metrics: Vec::new(),
                in_flight: Vec::new(),
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

// -- Adapter: harness ConversationEvents → the wave's TurnDelta vocabulary --

/// Adapts the harness's [`ConversationEvent`] stream into [`TurnDelta`]s so
/// the existing [`TurnSink`] pipeline (journal, open-turn snapshot, SSE,
/// `MindState`) keeps working unchanged — one pipeline, not a fork.
///
/// Turn-grained for the MVP wire: reasoning deltas and item phases are
/// dropped. Prose lands once: prefer a completed `Message` item when the
/// harness has one (codex), otherwise fold buffered `TextDelta`s at the turn
/// boundary (Claude/OpenCode). Harnesses emit `TurnUsage` after
/// `TurnCompleted` (possibly empty); finalization is held until that trailing
/// usage so the `Finished` delta carries the assembled turn whole.
#[derive(Debug, Default)]
pub struct EventAdapter {
    /// The turn being assembled, mirroring what the sink journals.
    open: Option<ChatTurn>,
    /// Completed turn awaiting its trailing `TurnUsage` before `Finished`.
    finished: Option<ChatTurn>,
    /// Streaming prose from harnesses that do not send a final Message item.
    buffered_text_delta: String,
    /// Whether this turn already received final prose as a Message item.
    saw_message_item: bool,
}

impl EventAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, event: &ConversationEvent) -> Vec<TurnDelta> {
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
                self.saw_message_item = false;
                deltas.push(TurnDelta::Opened);
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
                if let ConversationItem::Message { text, .. } = item {
                    self.saw_message_item = true;
                    if !open.text.is_empty() {
                        open.text.push('\n');
                    }
                    open.text.push_str(text);
                    deltas.push(TurnDelta::Text(text.clone()));
                } else {
                    open.items.push(item.clone());
                    deltas.push(TurnDelta::Item(item.clone()));
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
                    deltas.push(TurnDelta::Usage {
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
        if let Some(mut turn) = self.open.take() {
            turn.status = status;
            self.finished = Some(turn);
        }
        self.buffered_text_delta.clear();
        self.saw_message_item = false;
    }

    fn flush_buffered_text(&mut self, deltas: &mut Vec<TurnDelta>) {
        if self.saw_message_item || self.buffered_text_delta.is_empty() {
            self.buffered_text_delta.clear();
            return;
        }
        let Some(open) = self.open.as_mut() else {
            self.buffered_text_delta.clear();
            return;
        };
        if !open.text.is_empty() {
            open.text.push('\n');
        }
        open.text.push_str(&self.buffered_text_delta);
        deltas.push(TurnDelta::Text(std::mem::take(
            &mut self.buffered_text_delta,
        )));
    }

    fn flush_finished(&mut self, deltas: &mut Vec<TurnDelta>, cost_usd: Option<f64>) {
        if let Some(turn) = self.finished.take() {
            deltas.push(TurnDelta::Finished { turn, cost_usd });
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

/// Run the mind until the inbox closes (server shutdown) — the one loop that
/// replaces both the old progress arm and the old chat consumer.
pub async fn run_mind(
    runtime: Arc<WaveRuntime>,
    mut inbox_rx: mpsc::UnboundedReceiver<InboxItem>,
    harness: Box<dyn Harness>,
    mut events_rx: mpsc::UnboundedReceiver<ConversationEvent>,
    cwd: PathBuf,
    config: MindConfig,
    observer: Option<Arc<crate::lfd::wave::registry::StoreObserver>>,
) {
    let pending_messages = runtime.pending_messages();
    let mut mind = Mind {
        sink: TurnSink::new(runtime.clone()),
        adapter: EventAdapter::new(),
        runtime,
        harness,
        cwd,
        config,
        observer,
        queue: pending_messages,
        consecutive_failures: 0,
        in_flight: false,
        failed: false,
        started: false,
        idle_since: Instant::now(),
        interrupt_deadline: None,
    };

    if let Err(err) = mind.start_thread().await {
        mind.fail(&format!("mind failed to start: {err:#}"));
    } else if !mind.queue.is_empty() {
        mind.start_queued_turn().await;
    }

    let mut events_open = true;
    loop {
        let heartbeat_at = mind.heartbeat_deadline();
        let interrupt_at = mind.interrupt_deadline;
        tokio::select! {
            // Biased toward the inbox: a message that arrived before a turn
            // boundary is queued before the boundary drains, so coalescing
            // is deterministic.
            biased;
            item = inbox_rx.recv() => {
                let Some(item) = item else { break };
                mind.on_inbox(item).await;
            }
            event = events_rx.recv(), if events_open => {
                match event {
                    Some(event) => mind.on_event(event).await,
                    None => {
                        events_open = false;
                        mind.fail("harness event stream ended");
                    }
                }
            }
            _ = sleep_until_opt(interrupt_at), if interrupt_at.is_some() => {
                mind.on_interrupt_deadline().await;
            }
            _ = sleep_until_opt(heartbeat_at) => {
                mind.on_heartbeat().await;
            }
        }
    }

    // Server shutdown: end the vendor session (kills the child process).
    let _ = mind.harness.stop().await;
}

async fn sleep_until_opt(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

struct Mind {
    runtime: Arc<WaveRuntime>,
    harness: Box<dyn Harness>,
    sink: TurnSink,
    adapter: EventAdapter,
    cwd: PathBuf,
    config: MindConfig,
    /// Registry poller, when this server is registered: refreshed once
    /// before every turn so the `<in_flight>` fold is current.
    observer: Option<Arc<crate::lfd::wave::registry::StoreObserver>>,
    /// Messages awaiting a turn. The journal's fold is the durable queue
    /// (`UserMessage`s not named in any `TurnStarted.answers`); this is the
    /// scheduler's working copy.
    queue: Vec<UserMessage>,
    consecutive_failures: u32,
    /// A turn's input has been sent and its `Finished` hasn't landed yet.
    in_flight: bool,
    /// Mirrors `MindState::Failed` for scheduling (heartbeat off, revive on
    /// message).
    failed: bool,
    /// The vendor session is alive (start succeeded, no terminal error).
    started: bool,
    idle_since: Instant,
    /// Janitor bound while `Interrupting`: set when the cancel fires, cleared
    /// at the turn boundary. Past it, the force-path finalizes the turn.
    interrupt_deadline: Option<Instant>,
}

impl Mind {
    fn heartbeat_deadline(&self) -> Option<Instant> {
        (self.started && !self.failed && !self.in_flight && self.queue.is_empty())
            .then(|| self.idle_since + self.config.heartbeat_idle)
    }

    /// Start (or restart) the vendor thread and journal `ThreadStarted` —
    /// the mind's first durable act, before any turn (borrowed-handle rule).
    /// See the module doc for what resume actually does on codex.
    async fn start_thread(&mut self) -> Result<()> {
        if let Some(previous) = self.runtime.last_thread_id() {
            self.harness.set_provider_session_id(Some(previous));
        }
        let agent_config = mind_agent_config(&self.runtime, &self.cwd);
        self.harness.start(&agent_config).await?;
        self.started = true;
        match self.harness.provider_session_id() {
            Some(thread_id) => {
                self.runtime
                    .journal_thread_started(&self.config.vendor, &thread_id);
            }
            None => tracing::warn!(
                vendor = self.config.vendor,
                "vendor announced no thread id; ThreadStarted not journaled"
            ),
        }
        Ok(())
    }

    async fn on_inbox(&mut self, item: InboxItem) {
        match item {
            InboxItem::Message(message) => match message.op {
                // A say emission (worker report, child-wave escalation) wakes
                // the mind exactly like a message: queued, coalesced, answered.
                MessageOp::Message | MessageOp::Say => self.on_message(message).await,
                MessageOp::Steer => self.on_steer(message).await,
                MessageOp::Interrupt => self.on_interrupt(Some(message)).await,
            },
            InboxItem::Interrupt => self.on_interrupt(None).await,
        }
    }

    async fn on_message(&mut self, message: UserMessage) {
        self.queue.push(message);
        if self.failed {
            self.revive().await;
        } else if self.started && !self.in_flight {
            self.start_queued_turn().await;
        }
        // else: turning — queued; the next boundary drains it.
    }

    /// Steer: inject into the live turn when there is one and the harness can
    /// steer; the current turn consumed the message (`TurnSteered.answers`).
    /// Otherwise — idle, interrupting, non-capable harness — degrade to a
    /// queued message; the next boundary answers it. The `UserMessage{op:
    /// Steer}` row was journaled at delivery either way: intent in the log,
    /// consumption recording what actually happened.
    async fn on_steer(&mut self, message: UserMessage) {
        let steerable = self.in_flight
            && self.interrupt_deadline.is_none()
            && self.harness.capabilities().supports_steer;
        if !steerable {
            self.on_message(message).await;
            return;
        }
        match self.harness.send_input(&message.text).await {
            Ok(()) => {
                if !self.runtime.journal_steered(vec![message.id.clone()]) {
                    // The turn closed between the check and the send; the
                    // message reached the vendor as the next turn's input.
                    tracing::warn!("steer landed at a turn boundary; consumption not marked");
                }
            }
            Err(err) => {
                // The harness is broken, not the message: keep it queued so a
                // revival re-sends it.
                self.queue.push(message);
                self.fail(&format!("steer send_input failed: {err:#}"));
            }
        }
    }

    /// Interrupt: cancel the open turn (cooperative, deadline-bounded). Text
    /// riding the interrupt ("interrupt & send") is queued first so the
    /// post-interrupt boundary starts the next turn answering it. While idle
    /// there is nothing to cancel — a no-op, except that text starts the next
    /// turn immediately. While already interrupting, only the text queues.
    async fn on_interrupt(&mut self, message: Option<UserMessage>) {
        if let Some(message) = message {
            self.queue.push(message);
        }
        if self.failed {
            // Nothing to interrupt; text (if any) revives the mind like any
            // other message.
            if !self.queue.is_empty() {
                self.revive().await;
            }
            return;
        }
        if !self.in_flight {
            // Idle: no-op; "interrupt & send" text becomes the next turn now.
            if self.started && !self.queue.is_empty() {
                self.start_queued_turn().await;
            }
            return;
        }
        if self.interrupt_deadline.is_some() {
            return; // already interrupting; the boundary (or janitor) settles it
        }
        if !self.runtime.begin_interrupt("user interrupt") {
            tracing::warn!("interrupt with a turn in flight but no Turning state; ignored");
            return;
        }
        if let Err(err) = self.harness.interrupt().await {
            // Cooperative cancel failed; the deadline janitor bounds the wait.
            tracing::warn!(error = %format!("{err:#}"), "harness interrupt failed; janitor deadline armed");
        }
        self.interrupt_deadline = Some(Instant::now() + self.config.interrupt_deadline);
    }

    /// The janitor force-path: the harness swallowed the interrupt (no
    /// terminal event within the deadline). Journal `TurnFinished
    /// {Interrupted}`, settle to `Idle`, drop the sink/adapter's open-turn
    /// state so a late terminal event is ignored, and drain the queue.
    async fn on_interrupt_deadline(&mut self) {
        self.interrupt_deadline = None;
        tracing::error!(
            wave = self.runtime.name(),
            deadline_secs = self.config.interrupt_deadline.as_secs_f64(),
            "interrupt deadline expired; force-finalizing the open turn as interrupted"
        );
        self.sink.abandon_open();
        self.adapter = EventAdapter::new();
        self.runtime.force_finalize_open_turn(
            Lifecycle::Interrupted,
            "interrupt deadline: harness never delivered the terminal event",
        );
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

        let mut boundary_status = None;
        for delta in self.adapter.feed(&event) {
            if let TurnDelta::Finished { turn, .. } = &delta {
                boundary_status = Some(turn.status);
            }
            self.sink.on_delta(delta);
        }

        if let Some(reason) = terminal_reason {
            // The vendor session is gone; any open turn was finalized failed
            // above. This is a mind failure, not a turn failure.
            self.started = false;
            self.in_flight = false;
            self.fail(&format!("harness disconnected: {reason}"));
            return;
        }
        if let Some(status) = boundary_status {
            self.on_turn_boundary(status).await;
        }
    }

    async fn on_turn_boundary(&mut self, status: Lifecycle) {
        self.in_flight = false;
        self.idle_since = Instant::now();
        // An interrupted turn finalized in time: the janitor stands down.
        self.interrupt_deadline = None;
        if status == Lifecycle::Failed {
            self.consecutive_failures += 1;
            if self.consecutive_failures >= MAX_CONSECUTIVE_TURN_FAILURES {
                self.fail(&format!(
                    "{MAX_CONSECUTIVE_TURN_FAILURES} consecutive turn failures"
                ));
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
        self.refresh_observations().await;
        let prompt = heartbeat_prompt(&self.runtime);
        self.send_turn(prompt, Vec::new()).await;
    }

    /// One registry poll before a turn, so the turn's view of in-flight
    /// workers (the `<in_flight>` fold) is current rather than one poll
    /// cadence stale.
    async fn refresh_observations(&self) {
        if let Some(observer) = &self.observer {
            observer.poll_once().await;
        }
    }

    /// Drain the whole queue into one turn; its `TurnStarted.answers` names
    /// every consumed message.
    async fn start_queued_turn(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        self.refresh_observations().await;
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
            // so a revival re-sends them.
            self.queue = messages;
        }
    }

    /// Send one turn's input. Returns whether the send was accepted; a send
    /// error fails the mind (the harness is broken, not the turn).
    async fn send_turn(&mut self, content: String, answers: Vec<MessageId>) -> bool {
        self.sink.expect_answers(answers);
        match self.harness.send_input(&content).await {
            Ok(()) => {
                self.in_flight = true;
                true
            }
            Err(err) => {
                self.sink.expect_answers(Vec::new());
                self.fail(&format!("send_input failed: {err:#}"));
                false
            }
        }
    }

    /// `Failed → Idle` on a user message: a human talking to the wave revives
    /// it. Restarts the vendor thread if the session died (journaling the new
    /// `ThreadStarted`), then answers the queue.
    async fn revive(&mut self) {
        if !self
            .runtime
            .transition(MindState::Idle, "user message revived the mind")
        {
            return;
        }
        self.failed = false;
        self.consecutive_failures = 0;
        self.idle_since = Instant::now();
        if !self.started {
            let _ = self.harness.stop().await;
            if let Err(err) = self.start_thread().await {
                self.fail(&format!("mind failed to restart: {err:#}"));
                return;
            }
        }
        self.start_queued_turn().await;
    }

    fn fail(&mut self, reason: &str) {
        if self.failed {
            return;
        }
        tracing::error!(
            wave = self.runtime.name(),
            reason,
            "wave mind failed; heartbeat stopped (a user message revives it)"
        );
        self.failed = true;
        self.in_flight = false;
        // A dead mind has no turn for the janitor to force-finalize; the
        // failure path already closed whatever was open.
        self.interrupt_deadline = None;
        self.runtime.transition(
            MindState::Failed {
                reason: reason.to_string(),
            },
            reason,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use async_trait::async_trait;

    use crate::lfd::conversations::harness::Capabilities;
    use crate::lfd::conversations::types::TurnUsage;
    use crate::lfd::wave::journal::{journal_path, EventKind, Journal};

    /// Scriptless mock: records `send_input`/`interrupt`/
    /// `set_provider_session_id`; the TEST drives the event stream directly
    /// through the channel it created, so turn lifecycles are fully
    /// deterministic. `interrupt` records the call and nothing else — a
    /// "responsive" harness is simulated by the test emitting the terminal
    /// event afterwards; a "swallowing" harness by not emitting it.
    struct MockHarness {
        inputs: Arc<Mutex<Vec<String>>>,
        seeded: Arc<Mutex<Option<String>>>,
        thread_id: String,
        starts: Arc<Mutex<u32>>,
        interrupts: Arc<Mutex<u32>>,
        supports_steer: bool,
    }

    #[async_trait]
    impl Harness for MockHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            *self.starts.lock().unwrap() += 1;
            Ok(())
        }
        async fn send_input(&mut self, content: &str) -> Result<()> {
            self.inputs.lock().unwrap().push(content.to_string());
            Ok(())
        }
        async fn interrupt(&mut self) -> Result<()> {
            *self.interrupts.lock().unwrap() += 1;
            Ok(())
        }
        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_steer: self.supports_steer,
                supports_interrupt: true,
            }
        }
        fn provider_session_id(&self) -> Option<String> {
            Some(self.thread_id.clone())
        }
        fn set_provider_session_id(&mut self, provider_session_id: Option<String>) {
            *self.seeded.lock().unwrap() = provider_session_id;
        }
    }

    struct TestMind {
        runtime: Arc<WaveRuntime>,
        events: mpsc::UnboundedSender<ConversationEvent>,
        inputs: Arc<Mutex<Vec<String>>>,
        seeded: Arc<Mutex<Option<String>>>,
        starts: Arc<Mutex<u32>>,
        interrupts: Arc<Mutex<u32>>,
        _tmp: tempfile::TempDir,
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

    fn boot(heartbeat: Duration) -> TestMind {
        boot_in(tempfile::tempdir().expect("tempdir"), heartbeat)
    }

    fn boot_in(tmp: tempfile::TempDir, heartbeat: Duration) -> TestMind {
        boot_with(tmp, heartbeat, INTERRUPT_DEADLINE, true)
    }

    fn boot_with(
        tmp: tempfile::TempDir,
        heartbeat: Duration,
        interrupt_deadline: Duration,
        supports_steer: bool,
    ) -> TestMind {
        let (runtime, inbox_rx) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let seeded = Arc::new(Mutex::new(None));
        let starts = Arc::new(Mutex::new(0));
        let interrupts = Arc::new(Mutex::new(0));
        let harness = Box::new(MockHarness {
            inputs: inputs.clone(),
            seeded: seeded.clone(),
            thread_id: "thread-new".to_string(),
            starts: starts.clone(),
            interrupts: interrupts.clone(),
            supports_steer,
        });
        tokio::spawn(run_mind(
            runtime.clone(),
            inbox_rx,
            harness,
            events_rx,
            tmp.path().to_path_buf(),
            MindConfig {
                vendor: "codex".to_string(),
                heartbeat_idle: heartbeat,
                interrupt_deadline,
            },
            None,
        ));
        TestMind {
            runtime,
            events: events_tx,
            inputs,
            seeded,
            starts,
            interrupts,
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
        let finished = deltas
            .iter()
            .find_map(|delta| match delta {
                TurnDelta::Finished { turn, .. } => Some(turn.clone()),
                _ => None,
            })
            .expect("turn finished");
        assert!(finished.items.is_empty(), "no empty item on the wire");
        assert!(finished.text.is_empty(), "no stray text from empty items");
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
            [TurnDelta::Text(text)] if text == "Hello from OpenCode"
        ));

        let deltas = adapter.feed(&ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage::default(),
        });
        let finished = deltas
            .iter()
            .find_map(|delta| match delta {
                TurnDelta::Finished { turn, .. } => Some(turn.clone()),
                _ => None,
            })
            .expect("turn finished");
        assert_eq!(finished.text, "Hello from OpenCode");
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
            [TurnDelta::Text(text)] if text == "Hello from Codex"
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
        let finished = deltas
            .iter()
            .find_map(|delta| match delta {
                TurnDelta::Finished { turn, .. } => Some(turn.clone()),
                _ => None,
            })
            .expect("turn finished");
        assert_eq!(finished.text, "Hello from Codex");
    }

    /// The mind's prompt carries the one shared loopflow operating document —
    /// exactly once, and the mind-specific discipline no longer duplicates it.
    #[test]
    fn mind_prompt_carries_the_shared_loopflow_document_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (runtime, _inbox) =
            WaveRuntime::open("ship".to_string(), tmp.path().to_path_buf()).expect("open runtime");
        let prompt = mind_agent_config(&runtime, tmp.path()).system_prompt;

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

    /// A say emission wakes the mind like a message: the next turn's input
    /// carries the byline and its `TurnStarted.answers` consumes the id.
    #[tokio::test]
    async fn say_wakes_the_mind_and_is_consumed_by_the_next_turn() {
        let mind = boot(Duration::from_secs(600));
        let turn = mind.runtime.deliver_say(
            "implement run-1 finished: PR #7, one surprise".into(),
            crate::lfd::wave::journal::Attribution {
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

    #[tokio::test]
    async fn message_while_idle_starts_a_turn_answering_it() {
        let mind = boot(Duration::from_secs(600));
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

    #[tokio::test]
    async fn messages_while_turning_coalesce_into_one_boundary_turn() {
        let mind = boot(Duration::from_secs(600));
        mind.runtime
            .deliver_user_message("first".into(), MessageOp::Message);
        wait_for("turn 1 sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });

        // Two messages land mid-turn: queued, never rejected. The biased
        // select guarantees they're queued before the boundary drains.
        let m2 = mind
            .runtime
            .deliver_user_message("second".into(), MessageOp::Message);
        let m3 = mind
            .runtime
            .deliver_user_message("third".into(), MessageOp::Message);
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        });
        mind.emit(ConversationEvent::TurnUsage {
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

    #[tokio::test]
    async fn heartbeat_fires_when_idle_and_not_while_turning() {
        let mind = boot(Duration::from_millis(50));
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
        wait_for("next heartbeat", || mind.input_count() >= 2).await;
    }

    #[tokio::test]
    async fn heartbeat_carries_in_flight_workers_when_present() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Journal the observations before the mind boots so the first
        // heartbeat deterministically sees them.
        {
            let (runtime, _rx) =
                WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
            assert!(runtime.journal_worker_dispatched(
                "run-7",
                "sess-7",
                "implement",
                "wire the observation tail",
            ));
            assert!(runtime.journal_worker_dispatched("run-8", "sess-8", "design", "next item"));
            assert!(runtime.journal_worker_finished(
                "run-8",
                crate::lfd::wave::journal::WorkerOutcome::Completed,
                "landed",
            ));
        }

        let mind = boot_in(tmp, Duration::from_millis(50));
        wait_for("heartbeat turn", || mind.input_count() == 1).await;
        let prompt = mind.inputs.lock().unwrap()[0].clone();
        assert!(prompt.starts_with(HEARTBEAT_PROMPT));
        assert!(prompt.contains("<in_flight>"), "in-flight section present");
        assert!(prompt.contains("run run-7 · implement: wire the observation tail · running"));
        assert!(
            !prompt.contains("run-8"),
            "finished workers are not in flight"
        );
    }

    #[tokio::test]
    async fn failure_cap_fails_the_mind_and_a_message_revives_it() {
        let mind = boot(Duration::from_millis(30));
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
        wait_for("mind failed", || {
            matches!(mind.runtime.mind_state(), MindState::Failed { .. })
        })
        .await;

        // Heartbeat is stopped: no new turns while failed.
        let sends = mind.input_count();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(mind.input_count(), sends, "heartbeat stopped while failed");

        // A user message revives the mind (Failed → Idle) and gets answered.
        let wake = mind
            .runtime
            .deliver_user_message("are you alive?".into(), MessageOp::Message);
        wait_for("revival turn sent", || mind.input_count() == sends + 1).await;
        assert_eq!(mind.runtime.mind_state(), MindState::Idle);
        mind.emit_turn("back!", Lifecycle::Completed);
        wait_for("revival turn journaled", || {
            started_answers(&mind.journal_events())
                .last()
                .is_some_and(|answers| answers == &vec![message_id(&wake)])
        })
        .await;
    }

    #[tokio::test]
    async fn thread_started_is_journaled_before_the_first_turn() {
        let mind = boot(Duration::from_secs(600));
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
            let (runtime, _rx) =
                WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
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

        let mind = boot_in(tmp, Duration::from_secs(600));
        // The previous id was offered for resume (the codex driver ignores
        // it — documented cold start — but the seam is exercised)…
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

    #[tokio::test]
    async fn boot_with_unanswered_messages_replays_them_to_the_mind() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let expected_message_id = {
            let (runtime, _rx) =
                WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
            let turn = runtime
                .deliver_user_message("answer this after restart".into(), MessageOp::Message);
            message_id(&turn)
        };

        let mind = boot_in(tmp, Duration::from_secs(600));
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
        let mind = boot(Duration::from_secs(600));
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
        wait_for("back to idle", || {
            mind.runtime.mind_state() == MindState::Idle
        })
        .await;
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
            Duration::from_secs(600),
            INTERRUPT_DEADLINE,
            false, // supports_steer = false
        );
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
        tokio::time::sleep(Duration::from_millis(100)).await;
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

    // -- Interrupting --

    #[tokio::test]
    async fn interrupt_finalizes_the_turn_interrupted_and_settles_idle() {
        let mind = boot(Duration::from_secs(600));
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
        assert_eq!(mind.runtime.mind_state().name(), "interrupting");

        // The harness cancels cooperatively: terminal event arrives.
        mind.emit(ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Interrupted,
        });
        mind.emit(ConversationEvent::TurnUsage {
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
        let mind = boot(Duration::from_secs(600));
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
        let mind = boot(Duration::from_secs(600));
        wait_for("started", || *mind.starts.lock().unwrap() == 1).await;

        mind.runtime.deliver_interrupt();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(mind.runtime.mind_state(), MindState::Idle);
        assert_eq!(*mind.interrupts.lock().unwrap(), 0, "nothing to cancel");
        assert_eq!(mind.input_count(), 0, "no turn started");
    }

    #[tokio::test]
    async fn interrupt_deadline_forces_the_turn_closed_when_the_harness_swallows_it() {
        // Swallowing harness: interrupt is recorded but no terminal event
        // ever arrives. A short deadline stands in for the 10s janitor bound.
        let mind = boot_with(
            tempfile::tempdir().expect("tempdir"),
            Duration::from_secs(600),
            Duration::from_millis(50),
            true,
        );
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

        // No terminal event — the janitor force-path fires at the deadline.
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
}
