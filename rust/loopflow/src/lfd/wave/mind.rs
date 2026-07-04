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
//! protection and worktree bootstrap arrive with the lfd-registration phase.
//! Approval policy is `AutoApprove` until Decisions land.

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
use crate::lfd::wave::journal::MessageId;
use crate::lfd::wave::memory::Memory;
use crate::lfd::wave::runtime::{TurnSink, UserMessage, WaveRuntime};
use crate::lfd::wave::state::MindState;

/// How long the mind sits idle (empty queue, no turn) before a heartbeat
/// turn. Each heartbeat burns a subscription turn on the vendor plan, so the
/// quiet-wave cadence is deliberately coarse; messages and turn boundaries
/// drive the mind the rest of the time.
pub const HEARTBEAT_IDLE: Duration = Duration::from_secs(300);

/// Consecutive failed turns before the mind itself is declared `Failed` and
/// the heartbeat stops. A user message resets the count and revives the mind.
pub const MAX_CONSECUTIVE_TURN_FAILURES: u32 = 3;

/// Compact nudge for heartbeat turns. The thread is persistent, so heartbeats
/// never re-send the seed — the first turn carried it.
const HEARTBEAT_PROMPT: &str = "Heartbeat: re-read your goal and memory, then take the next \
     orchestration step. If nothing needs doing, say so in one line.";

/// Scheduler knobs. `Default` is production: codex vendor, 5-minute heartbeat.
#[derive(Debug, Clone)]
pub struct MindConfig {
    /// Vendor label journaled in `ThreadStarted`.
    pub vendor: String,
    /// Idle window before a heartbeat turn (see [`HEARTBEAT_IDLE`]).
    pub heartbeat_idle: Duration,
}

impl Default for MindConfig {
    fn default() -> Self {
        Self {
            vendor: "codex".to_string(),
            heartbeat_idle: HEARTBEAT_IDLE,
        }
    }
}

/// The mind's operating prompt: the rendered goal seed plus the orchestration
/// discipline. Rides the harness `AgentConfig` system prompt, which the codex
/// driver prepends to the first turn of the thread.
pub fn mind_agent_config(runtime: &WaveRuntime, cwd: &Path) -> AgentConfig {
    let seed = build_goal_seed(runtime.repo_root(), runtime.name(), runtime.memory());
    AgentConfig {
        system_prompt: format!("{seed}\n\n{}", orchestration_discipline(runtime.name())),
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
/// prompt: the mind orchestrates, it never grinds inline.
fn orchestration_discipline(wave: &str) -> String {
    format!(
        "You are the mind of the '{wave}' wave — its long-running orchestrator.\n\
         Discipline:\n\
         - Never grind inline. Read state, decide, dispatch work to subagents \
         (via `lfq worker run` when available), fold what you learn into \
         wave/{wave}/MEMORY.md, and answer the human.\n\
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
/// Turn-grained for the MVP wire: token deltas (`TextDelta`,
/// `ReasoningDelta`) and `ItemStarted`/`ItemUpdated` phases are dropped;
/// prose lands once, via the completed `Message` item (codex sends the final
/// agent message text in `item/completed`). Codex reports usage *after*
/// `TurnCompleted`, so finalization is held until the trailing `TurnUsage`
/// (or flushed by whatever event comes next) — the `Finished` delta then
/// carries the assembled turn whole, as the sink expects.
#[derive(Debug, Default)]
pub struct EventAdapter {
    /// The turn being assembled, mirroring what the sink journals.
    open: Option<ChatTurn>,
    /// Completed turn awaiting its trailing `TurnUsage` before `Finished`.
    finished: Option<ChatTurn>,
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
                });
                deltas.push(TurnDelta::Opened);
            }
            ConversationEvent::ItemCompleted { item, .. } => {
                self.flush_finished(&mut deltas, None);
                let Some(open) = self.open.as_mut() else {
                    tracing::warn!("harness item outside a turn; dropped");
                    return deltas;
                };
                if let ConversationItem::Message { text, .. } = item {
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
            ConversationEvent::TurnCompleted { status, .. } => {
                self.flush_finished(&mut deltas, None);
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
    }

    fn flush_finished(&mut self, deltas: &mut Vec<TurnDelta>, cost_usd: Option<f64>) {
        if let Some(turn) = self.finished.take() {
            deltas.push(TurnDelta::Finished { turn, cost_usd });
        }
    }
}

// -- The scheduler --

/// Run the mind until the inbox closes (server shutdown) — the one loop that
/// replaces both the old progress arm and the old chat consumer.
pub async fn run_mind(
    runtime: Arc<WaveRuntime>,
    mut inbox_rx: mpsc::UnboundedReceiver<UserMessage>,
    harness: Box<dyn Harness>,
    mut events_rx: mpsc::UnboundedReceiver<ConversationEvent>,
    cwd: PathBuf,
    config: MindConfig,
) {
    let mut mind = Mind {
        sink: TurnSink::new(runtime.clone()),
        adapter: EventAdapter::new(),
        runtime,
        harness,
        cwd,
        config,
        queue: Vec::new(),
        consecutive_failures: 0,
        in_flight: false,
        failed: false,
        started: false,
        idle_since: Instant::now(),
    };

    if let Err(err) = mind.start_thread().await {
        mind.fail(&format!("mind failed to start: {err:#}"));
    }

    let mut events_open = true;
    loop {
        let heartbeat_at = mind.heartbeat_deadline();
        tokio::select! {
            // Biased toward the inbox: a message that arrived before a turn
            // boundary is queued before the boundary drains, so coalescing
            // is deterministic.
            biased;
            msg = inbox_rx.recv() => {
                let Some(msg) = msg else { break };
                mind.on_message(msg).await;
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
            _ = heartbeat_sleep(heartbeat_at) => {
                mind.on_heartbeat().await;
            }
        }
    }

    // Server shutdown: end the vendor session (kills the child process).
    let _ = mind.harness.stop().await;
}

async fn heartbeat_sleep(deadline: Option<Instant>) {
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

    async fn on_message(&mut self, message: UserMessage) {
        self.queue.push(message);
        if self.failed {
            self.revive().await;
        } else if self.started && !self.in_flight {
            self.start_queued_turn().await;
        }
        // else: turning — queued; the next boundary drains it.
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
        self.send_turn(HEARTBEAT_PROMPT.to_string(), Vec::new())
            .await;
    }

    /// Drain the whole queue into one turn; its `TurnStarted.answers` names
    /// every consumed message.
    async fn start_queued_turn(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let messages = std::mem::take(&mut self.queue);
        let answers: Vec<MessageId> = messages.iter().map(|m| m.id.clone()).collect();
        let content = messages
            .iter()
            .map(|m| m.text.as_str())
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

    /// Scriptless mock: records `send_input`/`set_provider_session_id`; the
    /// TEST drives the event stream directly through the channel it created,
    /// so turn lifecycles are fully deterministic.
    struct MockHarness {
        inputs: Arc<Mutex<Vec<String>>>,
        seeded: Arc<Mutex<Option<String>>>,
        thread_id: String,
        starts: Arc<Mutex<u32>>,
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
            Ok(())
        }
        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_steer: true,
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
        let (runtime, inbox_rx) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let inputs = Arc::new(Mutex::new(Vec::new()));
        let seeded = Arc::new(Mutex::new(None));
        let starts = Arc::new(Mutex::new(0));
        let harness = Box::new(MockHarness {
            inputs: inputs.clone(),
            seeded: seeded.clone(),
            thread_id: "thread-new".to_string(),
            starts: starts.clone(),
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
            },
        ));
        TestMind {
            runtime,
            events: events_tx,
            inputs,
            seeded,
            starts,
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

    #[tokio::test]
    async fn message_while_idle_starts_a_turn_answering_it() {
        let mind = boot(Duration::from_secs(600));
        let user_turn = mind.runtime.deliver_user_message("hello mind".into());
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
        mind.runtime.deliver_user_message("first".into());
        wait_for("turn 1 sent", || mind.input_count() == 1).await;
        mind.emit(ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        });

        // Two messages land mid-turn: queued, never rejected. The biased
        // select guarantees they're queued before the boundary drains.
        let m2 = mind.runtime.deliver_user_message("second".into());
        let m3 = mind.runtime.deliver_user_message("third".into());
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
        let wake = mind.runtime.deliver_user_message("are you alive?".into());
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
        mind.runtime.deliver_user_message("go".into());
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
}
