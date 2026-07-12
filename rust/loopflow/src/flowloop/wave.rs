//! The resident's loop: the wave tier of the loop runtime,
//! scheduled by events, publishing through the wire.
//!
//! This runs inside the resident process (the internal half of
//! `lf serve <name>`,
//! see [`crate::wave::resident`]) — never in the listener. A turn is one
//! `wave` flow (`wave_clarify → wave_pursue → wave_mutate`) run as a
//! bounded headless child in the wave home; continuity is GOAL.md + memory +
//! the chat journal riding every pass's seed, never a vendor thread.
//! Everything the loop does surfaces as [`ResidentDelta`]s sent through
//! the listener's resident door, where the journal, the open-turn snapshot,
//! SSE broadcast, and `LoopState` transitions live.
//!
//! # Scheduling
//! Input is the wave's `/events?inbox=true` subscription, parsed into
//! [`InboxItem`]s by the resident:
//! - **Message while idle** → a pass starts now; the `TurnOpened` delta's
//!   `answers` names the message plus anything already queued.
//! - **Message while a pass runs** → a `steer` reaches a capable live harness;
//!   other messages queue (append-and-coalesce, never rejected) for the next
//!   body. Harnesses without live steering degrade `steer` to that queue.
//! - **Interrupt while a pass runs** → the child is killed and the turn
//!   closes `Interrupted`; non-empty interrupt text queues for the next pass.
//! - **Interrupt while idle** → no-op; text, if any, queues like a message.
//! - **Heartbeat**: idle for [`HEARTBEAT_IDLE`] with an empty queue → a
//!   progress pass carrying a compact nudge plus the `<in_flight>` fold
//!   fetched from `GET /resident/context`.
//! - **Cron**: the wave's `crons:` frontmatter (GOAL.md, re-read at every
//!   deadline computation so edits land without a restart) arms a third
//!   deadline; a due schedule opens a system pass ("cron due: <flow> —
//!   dispatch it") exactly like the heartbeat nudge. Crons only fire while
//!   idle — a schedule that comes due mid-pass fires at the boundary (within
//!   [`CRON_GRACE`]).
//!
//! The select is `biased` toward the inbox.
//!
//! # Failure
//! A failed pass (spawn failure, nonzero exit, timeout) finishes its turn
//! `Failed` and returns the loop to idle.
//! [`MAX_CONSECUTIVE_PASS_FAILURES`] consecutive failures FAIL THE LOOP:
//! the resident reports `LoopState::Failed` over the wire and
//! [`run_loop`] returns an error — the process exits nonzero and the
//! LISTENER's supervisor owns revival (the process-level respawn ladder; a
//! human message respawns immediately). A dead loop is a dead process —
//! there is no in-process limbo. The listener disappearing (send failure,
//! inbox closed) ends the residency cleanly instead: `Ok(())`.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::chat::types::{ConversationEvent, Lifecycle, TurnUsage};
use crate::engine::flow::{available_flow_names, load_goal, render_goal, GoalRenderContext};
use crate::engine::wave_config::{read_wave_config, WaveCronDef};
use crate::harness::{default_create_harness, ApprovalPolicy, Harness};
use crate::wave::journal::{ellipsize, MessageId, MessageOp, PendingMessage};
use crate::wave::playhead::{BodyProvenance, StepKind, StepOutcome, StepRef};
use crate::wave::resident::ListenerClient;
use crate::wave::runtime::InboxItem;
use crate::wave::supervisor::sleep_until_opt;
use crate::wave::wire::{InFlightWorker, ResidentDelta, ResidentStateTo};

/// The default playlist advances continuously. Tests and specialized callers
/// can still supply a non-zero idle cadence.
pub const HEARTBEAT_IDLE: Duration = Duration::ZERO;

/// Consecutive failed turns before the loop itself is declared failed and
/// the resident exits (the listener's supervisor revives by respawning).
pub const MAX_CONSECUTIVE_PASS_FAILURES: u32 = 3;

/// How far back a never-fired (or long-idle) cron schedule is checked: an
/// occurrence within this window still fires; anything older is missed, not
/// replayed. Mirrors the dead daemon poller's grace so a wave that was down
/// over its weekly schedule still runs it on revival.
pub const CRON_GRACE: chrono::Duration = chrono::Duration::hours(24);

/// Compact nudge for heartbeat passes (the pass seed carries goal and
/// memory; the nudge only names the wake).
const HEARTBEAT_PROMPT: &str = "Heartbeat: re-read your goal and memory, then take the next \
     orchestration skill. If nothing needs doing, say so in one line.";

/// Longest task excerpt carried per worker in the `<in_flight>` section —
/// enough to recognize the dispatch, token-lean by design.
const IN_FLIGHT_TASK_CHARS: usize = 80;

fn finish_capture(capture: Option<&crate::trace::CaptureHandle>, outcome: &str) {
    let Some(capture) = capture else {
        return;
    };
    if let Err(error) = capture.finish(outcome, false) {
        tracing::warn!(%error, %outcome, "failed to finalize trace capture");
    }
}

/// The heartbeat nudge, plus a compact `<in_flight>` section when workers are
/// grinding: one line per dispatched-not-finished worker, from the listener's
/// `GET /resident/context` — the loop's orchestration turns see their workers
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
/// computation — editing the file reschedules a live loop, no restart.
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

/// The system turn a due schedule opens — the loop dispatches the flow with
/// judgment, exactly like it acts on a heartbeat nudge.
pub(crate) fn cron_prompt(due: &[WaveCronDef]) -> String {
    due.iter()
        .map(|cron| format!("cron due: {} — dispatch it", cron.flow))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Loop knobs. Production advances the playlist immediately and gives each
/// body a 30-minute timeout.
#[derive(Debug, Clone)]
pub struct LoopConfig {
    /// Idle window before a heartbeat `wave`.
    pub heartbeat_idle: Duration,
    /// Per-pass wall-clock timeout.
    pub pass_timeout: Duration,
    /// Maximum agent turns forwarded to each phase run.
    pub max_turns: Option<u32>,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            heartbeat_idle: HEARTBEAT_IDLE,
            pass_timeout: Duration::from_secs(30 * 60),
            max_turns: Some(20),
        }
    }
}

/// PATH for the loop's harness and every child the resident spawns: this
/// executable's directory first, so placed `lf` commands resolve to the binary
/// running this resident, never whatever `lf` the user's shell happens to find.
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

/// One pass's seed: the rendered goal seed, the orchestration discipline,
/// the shared loopflow operating document (pass seeds bypass context
/// assembly, so the `<lf:loopflow>` section is appended here), and the
/// wake that opened the pass. Reads GOAL.md and MEMORY.md from the ORIGIN
/// repo (reads are free; writes go through the listener's doors).
fn wave_pass_seed(origin_repo: &Path, wave: &str, wake: &str) -> String {
    let seed = build_goal_seed(origin_repo, wave);
    format!(
        "{seed}\n\n{}\n\n{}\n\n<wake>\n{wake}\n</wake>",
        orchestration_discipline(wave),
        crate::engine::prompt::loopflow_section()
    )
}

/// The wave's rendered `GOAL.md` plus current memory, or a minimal-but-real
/// fallback when there's no `GOAL.md` so the loop still has an identity.
fn build_goal_seed(repo: &Path, wave: &str) -> String {
    let memory = crate::engine::wave_context::gather_wave_memory(repo, wave).unwrap_or_default();
    match load_goal(wave, repo) {
        Ok(goal) => {
            let ctx = GoalRenderContext {
                flows: available_flow_names(repo),
                memory,
            };
            render_goal(&goal, &ctx)
        }
        Err(_) => {
            let mem_block = if memory.trim().is_empty() {
                "(memory is empty)".to_string()
            } else {
                memory
            };
            format!(
                "You are the agent of the '{wave}' wave. Drive the wave's goal \
                 forward.\n\nCurrent memory:\n{mem_block}"
            )
        }
    }
}

/// The coordinating-session discipline, promoted into the loop's system
/// prompt. A wave owns coordination, but a single local blocker is still
/// cheaper and clearer to resolve inline. Wave-specific rules only — shared
/// loopflow operating guidance is appended in
/// [`wave_pass_seed`], not duplicated here.
fn orchestration_discipline(wave: &str) -> String {
    format!(
        "You are the loop of the '{wave}' wave — its long-running orchestrator.\n\
         Discipline:\n\
         - Read state and filed tasks when available. If PM fails, report it \
         once and continue from GOAL, MEMORY, and project KRs; do not turn \
         infrastructure repair into this wave's work.\n\
         - Execute the next move inline by default. Resolve the one concrete \
         blocker between the wave and progress in this process.\n\
         - Create or select a Linear Project and Linear task before delegating \
         file-writing work. Start it with `lf task run <issue-id>`. Never \
         delegate anonymous work or the whole wave objective.\n\
         - Supervise durable Task Sessions with `lf task status`, `follow-up`, \
         `steer`, `interrupt`, `wait`, and `resume`. Each task owns one immutable \
         worktree and one PR to main; keep the Wave home free of shipping edits.\n\
         - Keep turns centered on selection, direct progress, sequencing, and \
         authored reports.\n\
         - Trust worker summaries; never re-read worker transcripts.\n\
         - A human message is steering: answer it directly and adjust course \
         before returning to the goal."
    )
}

fn lf_command() -> std::process::Command {
    if let Ok(path) = std::env::current_exe() {
        return std::process::Command::new(path);
    }
    std::process::Command::new("lf")
}

fn body_provenance(step: &StepRef, cwd: &Path) -> BodyProvenance {
    let configured = crate::engine::load_config_or_default(Some(cwd));
    let agent = configured.agent.as_deref().unwrap_or("codex");
    let (harness, model) = crate::engine::parse_agent(agent);
    let mut body = BodyProvenance::for_step(step, cwd);
    body.harness = Some(harness);
    body.model = model;
    body
}

// -- The scheduler --

/// Why the loop ended.
enum LoopEnd {
    /// The listener is gone (send failed / inbox closed): the keeper died or
    /// was replaced. Clean exit — nothing to revive on this side.
    ListenerGone,
    /// The loop itself failed (reported over the wire before ending).
    /// The resident exits nonzero; the listener's supervisor respawns.
    Failed(String),
}

/// What an inbox item means for the body now running (see
/// [`WaveLoop::inbox_action`]). The caller owns teardown — that is the only
/// part the two pass loops legitimately do differently.
enum InboxAction {
    /// Tear the body down; `skip` advances the playhead anyway.
    Interrupt { skip: bool },
    /// Not interrupt-shaped: deliver to the live body or queue for the next.
    Deliver(InboxItem),
    /// The listener hung up: tear down and end the loop.
    ListenerGone,
}

/// What a fired pass-timeout means (see [`WaveLoop::timeout_action`]).
enum TimeoutAction {
    /// The loop already ended mid-fetch: tear down and return.
    End,
    /// Delegated work is still live: renew the pass lease and keep waiting.
    Renew,
    /// Nothing is running: the pass is out of time.
    Expire,
}

/// How a pass child is spawned — a seam so tests can substitute a stub
/// process for the real `lf -b wave` invocation.
#[cfg(test)]
type SpawnPass = Box<
    dyn Fn(&Path, &StepRef, &str, Option<u32>) -> std::io::Result<tokio::process::Child> + Send,
>;

type CreateBodyHarness = Box<
    dyn Fn(
            &str,
            ApprovalPolicy,
            mpsc::UnboundedSender<ConversationEvent>,
        ) -> Result<Box<dyn Harness>>
        + Send,
>;

type PrepareBodyHarness = Box<
    dyn Fn(&str, &str, &str, Option<u32>) -> Result<crate::lf::commands::run::PreparedHarnessTurn>
        + Send,
>;

enum BodyBackend {
    /// Product path: one live vendor session is the body now playing.
    Harness {
        prepare: PrepareBodyHarness,
        create: CreateBodyHarness,
    },
    /// Test and composite-step fallback. Composite flow nodes still own their
    /// internal execution until they become first-class playhead frames.
    #[cfg(test)]
    Process(SpawnPass),
}

/// Run the wave loop until the listener disappears (`Ok`) or the
/// loop fails (`Err`, after reporting `LoopState::Failed` over the wire).
///
/// # Errors
/// Loop failure only — the caller exits the process nonzero so the
/// listener's supervisor sees a dead resident.
pub async fn run_loop(
    client: ListenerClient,
    inbox_rx: mpsc::UnboundedReceiver<InboxItem>,
    cwd: PathBuf,
    origin_repo: PathBuf,
    wave: String,
    config: LoopConfig,
) -> Result<()> {
    let backend = BodyBackend::Harness {
        prepare: Box::new(crate::lf::commands::run::prepare_harness_turn),
        create: Box::new(default_create_harness),
    };
    run_loop_with(client, inbox_rx, cwd, origin_repo, wave, config, backend).await
}

#[allow(clippy::too_many_arguments)]
async fn run_loop_with(
    client: ListenerClient,
    mut inbox_rx: mpsc::UnboundedReceiver<InboxItem>,
    cwd: PathBuf,
    origin_repo: PathBuf,
    wave: String,
    config: LoopConfig,
    backend: BodyBackend,
) -> Result<()> {
    let mut wave_loop = WaveLoop {
        client,
        cwd,
        origin_repo,
        wave,
        config,
        queue: Vec::new(),
        seen: HashSet::new(),
        backend,
        consecutive_failures: 0,
        idle_since: Instant::now(),
        cron_last_fired: HashMap::new(),
        end: None,
    };

    while wave_loop.end.is_none() {
        if !wave_loop.queue.is_empty() {
            wave_loop.start_queued_pass(&mut inbox_rx).await;
            continue;
        }
        let heartbeat_at = wave_loop.heartbeat_deadline();
        let cron_at = wave_loop.cron_deadline();
        tokio::select! {
            biased;
            item = inbox_rx.recv() => {
                match item {
                    Some(item) => wave_loop.on_inbox(item).await,
                    None => wave_loop.end = Some(LoopEnd::ListenerGone),
                }
            }
            _ = sleep_until_opt(cron_at), if cron_at.is_some() => {
                wave_loop.on_cron(&mut inbox_rx).await;
            }
            _ = sleep_until_opt(Some(heartbeat_at)) => {
                wave_loop.on_heartbeat(&mut inbox_rx).await;
            }
        }
    }

    match wave_loop.end {
        Some(LoopEnd::Failed(reason)) => Err(anyhow!(reason)),
        _ => Ok(()),
    }
}

struct WaveLoop {
    client: ListenerClient,
    cwd: PathBuf,
    origin_repo: PathBuf,
    wave: String,
    config: LoopConfig,
    backend: BodyBackend,
    queue: Vec<PendingMessage>,
    seen: HashSet<MessageId>,
    consecutive_failures: u32,
    idle_since: Instant,
    cron_last_fired: HashMap<String, DateTime<Utc>>,
    end: Option<LoopEnd>,
}

impl WaveLoop {
    fn heartbeat_deadline(&self) -> Instant {
        self.idle_since + self.config.heartbeat_idle
    }

    fn cron_deadline(&self) -> Option<Instant> {
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

    async fn on_inbox(&mut self, item: InboxItem) {
        match item {
            InboxItem::Message(message) => {
                // Idle: every op just queues — an interrupt has no pass to
                // cancel, so it seeds the next one like any other message.
                if self.seen.insert(message.id.clone()) {
                    self.queue.push(message);
                }
            }
            InboxItem::Task(observation) => {
                let message = crate::wave::journal::task_observation_message(&observation);
                if self.seen.insert(message.id.clone()) {
                    self.queue.push(message);
                }
            }
            InboxItem::Project(observation) => {
                let message = crate::wave::journal::project_observation_message(&observation);
                if self.seen.insert(message.id.clone()) {
                    self.queue.push(message);
                }
            }
            InboxItem::Interrupt | InboxItem::Skip => {}
        }
    }

    async fn on_heartbeat(&mut self, inbox_rx: &mut mpsc::UnboundedReceiver<InboxItem>) {
        let workers = self.fetch_in_flight().await;
        if self.end.is_some() {
            return;
        }
        let prompt = heartbeat_prompt(&workers);
        self.run_pass(prompt, Vec::new(), inbox_rx).await;
    }

    async fn on_cron(&mut self, inbox_rx: &mut mpsc::UnboundedReceiver<InboxItem>) {
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
            return;
        }
        for cron in &due {
            self.cron_last_fired.insert(cron_key(cron), now);
        }
        let prompt = cron_prompt(&due);
        self.run_pass(prompt, Vec::new(), inbox_rx).await;
    }

    async fn start_queued_pass(&mut self, inbox_rx: &mut mpsc::UnboundedReceiver<InboxItem>) {
        let messages = std::mem::take(&mut self.queue);
        let answers: Vec<MessageId> = messages.iter().map(|m| m.id.clone()).collect();
        let content = messages
            .iter()
            .map(|m| match &m.from {
                Some(from) => format!("[{from}] {}", m.text),
                None => m.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        self.run_pass(content, answers, inbox_rx).await;
    }

    async fn run_pass(
        &mut self,
        wake: String,
        answers: Vec<MessageId>,
        inbox_rx: &mut mpsc::UnboundedReceiver<InboxItem>,
    ) {
        let context = match self.fetch_context().await {
            Some(context) => context,
            None => return,
        };
        let Some(step) = context.playhead.now else {
            self.fail("playhead has no current step").await;
            return;
        };
        let seed = wave_pass_seed(&self.origin_repo, &self.wave, &wake);
        let answers = answers.into_iter().map(|id| id.0).collect();
        let live_skill =
            step.kind == StepKind::Skill && matches!(&self.backend, BodyBackend::Harness { .. });
        if live_skill {
            self.run_harness_pass(step, seed, answers, inbox_rx).await;
        } else {
            self.run_process_pass(step, seed, answers, inbox_rx).await;
        }
    }

    async fn run_process_pass(
        &mut self,
        step: StepRef,
        seed: String,
        answers: Vec<String>,
        inbox_rx: &mut mpsc::UnboundedReceiver<InboxItem>,
    ) {
        let body = body_provenance(&step, &self.cwd);
        let body_id = body.body_id.clone();
        self.open_body(body, answers).await;
        if self.end.is_some() {
            return;
        }

        let child = match &self.backend {
            BodyBackend::Harness { .. } => {
                spawn_wave_step(&self.cwd, &step, &seed, self.config.max_turns)
            }
            #[cfg(test)]
            BodyBackend::Process(spawn) => spawn(&self.cwd, &step, &seed, self.config.max_turns),
        };
        let child = match child {
            Ok(child) => child,
            Err(err) => {
                self.finish_failed_pass(
                    &body_id,
                    &format!("failed to spawn {} / {}: {err:#}", step.flow, step.step),
                )
                .await;
                return;
            }
        };
        let mut wait_task = tokio::spawn(async move { child.wait_with_output().await });
        let mut timeout = Box::pin(tokio::time::sleep(self.config.pass_timeout));
        loop {
            tokio::select! {
                biased;
                item = inbox_rx.recv() => {
                    match self.inbox_action(item) {
                        InboxAction::Interrupt { skip } => {
                            self.interrupt_child(&body_id, &mut wait_task, skip).await;
                            return;
                        }
                        InboxAction::Deliver(item) => self.on_inbox(item).await,
                        InboxAction::ListenerGone => {
                            wait_task.abort();
                            return;
                        }
                    }
                }
                _ = &mut timeout => {
                    match self.timeout_action().await {
                        TimeoutAction::End => {
                            wait_task.abort();
                            return;
                        }
                        TimeoutAction::Renew => {
                            timeout.as_mut().reset(Instant::now() + self.config.pass_timeout);
                            continue;
                        }
                        TimeoutAction::Expire => {
                            wait_task.abort();
                            self.finish_timed_out_pass(&body_id).await;
                            return;
                        }
                    }
                }
                result = &mut wait_task => {
                    match result {
                        Ok(output) => self.on_pass_output(&body_id, output).await,
                        Err(err) => {
                            self.finish_failed_pass(
                                &body_id,
                                &format!("wave wait task failed: {err:#}"),
                            )
                                .await;
                        }
                    }
                    return;
                }
            }
        }
    }

    async fn run_harness_pass(
        &mut self,
        step: StepRef,
        seed: String,
        answers: Vec<String>,
        inbox_rx: &mut mpsc::UnboundedReceiver<InboxItem>,
    ) {
        let mut body = body_provenance(&step, &self.cwd);
        let prepared = match &self.backend {
            BodyBackend::Harness { prepare, .. } => {
                prepare(&step.step, &seed, &self.wave, self.config.max_turns)
            }
            #[cfg(test)]
            BodyBackend::Process(_) => unreachable!("live skill requires a harness backend"),
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(err) => {
                let body_id = body.body_id.clone();
                self.open_body(body, answers).await;
                self.finish_failed_pass(
                    &body_id,
                    &format!("failed to prepare {} / {}: {err:#}", step.flow, step.step),
                )
                .await;
                return;
            }
        };
        body.harness = Some(prepared.harness.clone());
        body.model = prepared.model.clone();

        let capture = match crate::journal::trace_capture_context(
            &self.cwd,
            Some(step.flow.clone()),
            Some(step.step.clone()),
        ) {
            Some(context) => match crate::trace::CaptureHandle::begin(
                context,
                prepared.context.clone(),
                crate::trace::CaptureStart {
                    provider: prepared.harness.clone(),
                    model: prepared.model.clone(),
                    surface: "headless".to_string(),
                    input_op: "initial".to_string(),
                    gather_ms: prepared.context_gather_ms,
                    render_ms: prepared.context_render_ms,
                    raw_provider: true,
                },
            ) {
                Ok(capture) => Some(capture),
                Err(err) => {
                    let body_id = body.body_id.clone();
                    self.open_body(body, answers).await;
                    self.finish_failed_pass(
                        &body_id,
                        &format!("failed to establish trace capture: {err}"),
                    )
                    .await;
                    return;
                }
            },
            None if cfg!(test) => None,
            None => {
                let body_id = body.body_id.clone();
                self.open_body(body, answers).await;
                self.finish_failed_pass(&body_id, "trace capture identity is unavailable")
                    .await;
                return;
            }
        };

        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let (raw_tx, mut raw_rx) = mpsc::unbounded_channel();
        let harness = match &self.backend {
            BodyBackend::Harness { create, .. } => {
                create(&prepared.harness, ApprovalPolicy::AutoApprove, event_tx)
            }
            #[cfg(test)]
            BodyBackend::Process(_) => unreachable!("live skill requires a harness backend"),
        };
        let mut harness = match harness {
            Ok(harness) => harness,
            Err(err) => {
                finish_capture(capture.as_ref(), "failed");
                let body_id = body.body_id.clone();
                self.open_body(body, answers).await;
                self.finish_failed_pass(
                    &body_id,
                    &format!("failed to create {} harness: {err:#}", prepared.harness),
                )
                .await;
                return;
            }
        };
        if capture.is_some() {
            harness.set_raw_provider_sender(Some(raw_tx));
        }
        if let Err(err) = harness.start(&prepared.config).await {
            finish_capture(capture.as_ref(), "failed");
            let body_id = body.body_id.clone();
            self.open_body(body, answers).await;
            self.finish_failed_pass(
                &body_id,
                &format!("failed to start {} harness: {err:#}", prepared.harness),
            )
            .await;
            return;
        }
        body.session_id = harness.provider_session_id();
        if let Some(capture) = &capture {
            capture.set_provider_session_id(body.session_id.clone());
        }
        let mut body_session_id = body.session_id.clone();
        let body_id = body.body_id.clone();
        self.open_body(body, answers).await;
        if self.end.is_some() {
            let _ = harness.stop().await;
            finish_capture(capture.as_ref(), "interrupted");
            return;
        }
        if let Err(err) = harness.send_input(&prepared.input).await {
            let _ = harness.stop().await;
            finish_capture(capture.as_ref(), "failed");
            self.finish_failed_pass(
                &body_id,
                &format!(
                    "failed to start {} / {} turn: {err:#}",
                    step.flow, step.step
                ),
            )
            .await;
            return;
        }

        let supports_steer = harness.capabilities().supports_steer;
        let mut timeout = Box::pin(tokio::time::sleep(self.config.pass_timeout));
        let mut terminal_wait = Box::pin(tokio::time::sleep(Duration::from_secs(86_400)));
        let mut terminal_status: Option<Lifecycle> = None;
        let mut usage = TurnUsage::default();
        loop {
            tokio::select! {
                biased;
                item = inbox_rx.recv() => {
                    match self.inbox_action(item) {
                        InboxAction::Interrupt { skip } => {
                            self.interrupt_harness(&body_id, harness.as_mut(), skip).await;
                            finish_capture(capture.as_ref(), "interrupted");
                            return;
                        }
                        InboxAction::Deliver(InboxItem::Message(message))
                            if message.op == MessageOp::Steer && supports_steer =>
                        {
                            if self
                                .steer_harness(message, harness.as_mut(), capture.as_ref())
                                .await
                            {
                                timeout.as_mut().reset(Instant::now() + self.config.pass_timeout);
                            }
                        }
                        InboxAction::Deliver(item) => self.on_inbox(item).await,
                        InboxAction::ListenerGone => {
                            let _ = harness.stop().await;
                            finish_capture(capture.as_ref(), "interrupted");
                            return;
                        }
                    }
                }
                raw = raw_rx.recv(), if capture.is_some() => {
                    if let (Some(raw), Some(capture)) = (raw, capture.as_ref()) {
                        capture.record_raw(raw.stream, &raw.line);
                    }
                }
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        let _ = harness.stop().await;
                        self.finish_failed_pass(&body_id, "harness event stream closed").await;
                        finish_capture(capture.as_ref(), "failed");
                        return;
                    };
                    if let Some(capture) = &capture {
                        capture.record_conversation(event.clone());
                    }
                    if body_session_id.is_none() {
                        tokio::task::yield_now().await;
                        if let Some(session_id) = harness.provider_session_id() {
                            body_session_id = Some(session_id.clone());
                            if let Some(capture) = &capture {
                                capture.set_provider_session_id(Some(session_id.clone()));
                            }
                            self.send(vec![ResidentDelta::BodySessionUpdated {
                                body_id: body_id.clone(),
                                session_id,
                            }]).await;
                        }
                    }
                    match event {
                        ConversationEvent::TextDelta { content, .. } => {
                            self.send(vec![ResidentDelta::TurnText { text: content }]).await;
                        }
                        ConversationEvent::ItemCompleted { item, .. } => {
                            self.send(vec![ResidentDelta::TurnItem { item }]).await;
                        }
                        ConversationEvent::TurnUsage { usage: reported, .. } => {
                            usage = reported;
                            self.send(vec![ResidentDelta::TurnUsage {
                                input_tokens: Some(usage.input_tokens),
                                output_tokens: Some(usage.output_tokens),
                                cache_read_tokens: usage.cache_read_tokens,
                            }]).await;
                            if terminal_status.is_some() {
                                let status = terminal_status.take().expect("checked");
                                let outcome = if status == Lifecycle::Completed {
                                    "completed"
                                } else if status == Lifecycle::Interrupted {
                                    "interrupted"
                                } else {
                                    "failed"
                                };
                                self.finish_harness_pass(
                                    &body_id,
                                    &step,
                                    status,
                                    usage.cost_usd,
                                    harness.as_mut(),
                                ).await;
                                finish_capture(capture.as_ref(), outcome);
                                return;
                            }
                        }
                        ConversationEvent::TurnCompleted { status, .. } => {
                            terminal_status = Some(status);
                            terminal_wait.as_mut().reset(
                                Instant::now() + Duration::from_millis(100)
                            );
                        }
                        ConversationEvent::Error { code, message } => {
                            let _ = harness.stop().await;
                            self.finish_failed_pass(
                                &body_id,
                                &format!("{code}: {message}"),
                            ).await;
                            finish_capture(capture.as_ref(), "failed");
                            return;
                        }
                        ConversationEvent::TurnStarted { .. }
                        | ConversationEvent::ItemStarted { .. }
                        | ConversationEvent::ItemUpdated { .. }
                        | ConversationEvent::ReasoningDelta { .. }
                        | ConversationEvent::DiffUpdated { .. }
                        | ConversationEvent::SuggestedActions { .. }
                        | ConversationEvent::StatusChanged { .. } => {}
                    }
                    if self.end.is_some() {
                        let _ = harness.stop().await;
                        finish_capture(capture.as_ref(), "interrupted");
                        return;
                    }
                }
                _ = &mut terminal_wait, if terminal_status.is_some() => {
                    let status = terminal_status.take().expect("checked");
                    let outcome = if status == Lifecycle::Completed {
                        "completed"
                    } else if status == Lifecycle::Interrupted {
                        "interrupted"
                    } else {
                        "failed"
                    };
                    self.finish_harness_pass(
                        &body_id,
                        &step,
                        status,
                        usage.cost_usd,
                        harness.as_mut(),
                    ).await;
                    finish_capture(capture.as_ref(), outcome);
                    return;
                }
                _ = &mut timeout => {
                    match self.timeout_action().await {
                        TimeoutAction::End => {
                            let _ = harness.stop().await;
                            finish_capture(capture.as_ref(), "interrupted");
                            return;
                        }
                        TimeoutAction::Renew => {
                            timeout.as_mut().reset(Instant::now() + self.config.pass_timeout);
                            continue;
                        }
                        TimeoutAction::Expire => {
                            let _ = harness.interrupt().await;
                            let _ = harness.stop().await;
                            self.finish_timed_out_pass(&body_id).await;
                            finish_capture(capture.as_ref(), "interrupted");
                            return;
                        }
                    }
                }
            }
        }
    }

    async fn open_body(&mut self, body: BodyProvenance, answers: Vec<String>) {
        self.send(vec![
            ResidentDelta::BodyStarted { body },
            ResidentDelta::TurnOpened { answers },
        ])
        .await;
    }

    async fn steer_harness(
        &mut self,
        message: PendingMessage,
        harness: &mut dyn Harness,
        capture: Option<&crate::trace::CaptureHandle>,
    ) -> bool {
        if !self.seen.insert(message.id.clone()) {
            return false;
        }
        let id = message.id.0.clone();
        self.send(vec![ResidentDelta::TurnSteered {
            answers: vec![id.clone()],
        }])
        .await;
        if self.end.is_some() {
            return false;
        }
        let text = match &message.from {
            Some(from) => format!("[{from}] {}", message.text),
            None => message.text.clone(),
        };
        if let Some(capture) = capture {
            if let Err(err) = capture.begin_turn("steer", &text) {
                tracing::warn!(error = %err, "trace capture refused live steering; requeueing message");
                self.send(vec![ResidentDelta::MessagesRequeued { ids: vec![id] }])
                    .await;
                self.queue.push(message);
                return false;
            }
        }
        if let Err(err) = harness.send_input(&text).await {
            tracing::warn!(error = %err, "live steering failed; requeueing message");
            self.send(vec![ResidentDelta::MessagesRequeued { ids: vec![id] }])
                .await;
            self.queue.push(message);
            return false;
        }
        true
    }

    /// The interrupt protocol: announce, tear the body down, close the pass.
    /// Only the teardown differs between a harness session and a child process.
    async fn announce_interrupt(&mut self) {
        self.send(vec![ResidentDelta::LoopState {
            to: ResidentStateTo::Interrupting,
            reason: "user interrupt".to_string(),
        }])
        .await;
    }

    async fn interrupt_harness(&mut self, body_id: &str, harness: &mut dyn Harness, skip: bool) {
        self.announce_interrupt().await;
        let _ = harness.interrupt().await;
        let _ = harness.stop().await;
        self.finish_interrupted_pass(body_id, skip).await;
    }

    /// Classify an inbox item at a running body. Interrupt-shaped items
    /// resolve here — including queueing an interrupt's text for the next
    /// pass — so interrupt semantics stay in lockstep across the process and
    /// harness loops. Everything else is handed back: the loops differ in
    /// what a live body can absorb (a steer-capable harness takes input
    /// mid-turn; a child process cannot).
    fn inbox_action(&mut self, item: Option<InboxItem>) -> InboxAction {
        match item {
            Some(InboxItem::Interrupt) => InboxAction::Interrupt { skip: false },
            Some(InboxItem::Skip) => InboxAction::Interrupt { skip: true },
            Some(InboxItem::Message(message)) if message.op == MessageOp::Interrupt => {
                if self.seen.insert(message.id.clone()) {
                    self.queue.push(message);
                }
                InboxAction::Interrupt { skip: false }
            }
            Some(InboxItem::Task(observation)) => InboxAction::Deliver(InboxItem::Message(
                crate::wave::journal::task_observation_message(&observation),
            )),
            Some(item) => InboxAction::Deliver(item),
            None => {
                self.end = Some(LoopEnd::ListenerGone);
                InboxAction::ListenerGone
            }
        }
    }

    /// What a fired pass-timeout means right now. A durable Task Session can
    /// outlive this pass; its registry row is presence, so live delegated work
    /// renews the lease instead of hanging up on it at the ordinary boundary.
    async fn timeout_action(&mut self) -> TimeoutAction {
        let workers = self.fetch_in_flight().await;
        if self.end.is_some() {
            TimeoutAction::End
        } else if !workers.is_empty() {
            TimeoutAction::Renew
        } else {
            TimeoutAction::Expire
        }
    }

    async fn finish_timed_out_pass(&mut self, body_id: &str) {
        self.finish_failed_pass(
            body_id,
            &format!(
                "wave timed out after {}s",
                self.config.pass_timeout.as_secs()
            ),
        )
        .await;
    }

    async fn finish_harness_pass(
        &mut self,
        body_id: &str,
        step: &StepRef,
        status: Lifecycle,
        cost_usd: Option<f64>,
        harness: &mut dyn Harness,
    ) {
        let _ = harness.stop().await;
        match status {
            Lifecycle::Completed => {
                if let Err(err) =
                    crate::lf::commands::flow::commit_skill_work(&self.cwd, &step.step)
                {
                    self.finish_failed_pass(
                        body_id,
                        &format!("failed to commit {}: {err:#}", step.step),
                    )
                    .await;
                    return;
                }
                self.consecutive_failures = 0;
                self.finish_pass(body_id, StepOutcome::Completed, None, cost_usd)
                    .await;
            }
            Lifecycle::Interrupted => self.finish_interrupted_pass(body_id, false).await,
            Lifecycle::Failed => {
                self.finish_failed_pass(body_id, "harness turn failed")
                    .await
            }
            Lifecycle::Pending | Lifecycle::Running => {
                self.finish_failed_pass(body_id, "harness ended without a terminal status")
                    .await;
            }
        }
    }

    async fn on_pass_output(
        &mut self,
        body_id: &str,
        result: std::io::Result<std::process::Output>,
    ) {
        match result {
            Ok(output) if output.status.success() => {
                self.consecutive_failures = 0;
                self.ship_output(output).await;
                self.finish_pass(body_id, StepOutcome::Completed, None, None)
                    .await;
            }
            Ok(output) => {
                self.ship_output(output).await;
                self.finish_failed_pass(body_id, "wave step exited nonzero")
                    .await;
            }
            Err(err) => {
                self.finish_failed_pass(body_id, &format!("wave wait failed: {err:#}"))
                    .await;
            }
        }
    }

    async fn ship_output(&mut self, output: std::process::Output) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let text = match (stdout.trim(), stderr.trim()) {
            ("", "") => String::new(),
            (out, "") => out.to_string(),
            ("", err) => err.to_string(),
            (out, err) => format!("{out}\n\nstderr:\n{err}"),
        };
        if !text.is_empty() {
            self.send(vec![ResidentDelta::TurnText { text }]).await;
        }
    }

    async fn interrupt_child(
        &mut self,
        body_id: &str,
        wait_task: &mut tokio::task::JoinHandle<std::io::Result<std::process::Output>>,
        skip: bool,
    ) {
        self.announce_interrupt().await;
        wait_task.abort();
        self.finish_interrupted_pass(body_id, skip).await;
    }

    /// Every terminal end of a body, and the only place the pair is built: the
    /// turn closes and the playhead's body closes with it. The outcome picks
    /// the turn's lifecycle — a skip is an interrupted turn whose playhead
    /// advances anyway — and names itself when the caller has nothing to add.
    async fn finish_pass(
        &mut self,
        body_id: &str,
        outcome: StepOutcome,
        reason: Option<String>,
        cost_usd: Option<f64>,
    ) {
        let status = match outcome {
            StepOutcome::Completed => Lifecycle::Completed,
            StepOutcome::Skipped | StepOutcome::Interrupted => Lifecycle::Interrupted,
            StepOutcome::Failed => Lifecycle::Failed,
        };
        let reason = reason.unwrap_or_else(|| outcome.name().to_string());
        self.send(vec![
            ResidentDelta::TurnFinished {
                status,
                cost_usd,
                reason: (status != Lifecycle::Completed).then(|| reason.clone()),
            },
            ResidentDelta::BodyFinished {
                body_id: body_id.to_string(),
                outcome,
                reason,
            },
        ])
        .await;
        self.idle_since = Instant::now();
    }

    async fn finish_interrupted_pass(&mut self, body_id: &str, skip: bool) {
        self.consecutive_failures = 0;
        let (outcome, reason) = if skip {
            (StepOutcome::Skipped, "skipped by user")
        } else {
            (StepOutcome::Interrupted, "interrupted by user")
        };
        self.finish_pass(body_id, outcome, Some(reason.to_string()), None)
            .await;
    }

    async fn finish_failed_pass(&mut self, body_id: &str, reason: &str) {
        self.finish_pass(body_id, StepOutcome::Failed, Some(reason.to_string()), None)
            .await;
        self.consecutive_failures += 1;
        if self.consecutive_failures >= MAX_CONSECUTIVE_PASS_FAILURES {
            self.fail(&format!(
                "{MAX_CONSECUTIVE_PASS_FAILURES} consecutive wave failures: {reason}"
            ))
            .await;
        }
    }

    async fn fetch_in_flight(&mut self) -> Vec<InFlightWorker> {
        self.fetch_context()
            .await
            .map(|context| context.in_flight)
            .unwrap_or_default()
    }

    async fn fetch_context(&mut self) -> Option<crate::wave::wire::ContextResponse> {
        match self.client.context().await {
            Ok(context) => Some(context),
            Err(err) => {
                tracing::info!(
                    error = %format!("{err:#}"),
                    "listener unreachable; ending residency"
                );
                self.end = Some(LoopEnd::ListenerGone);
                None
            }
        }
    }

    async fn send(&mut self, deltas: Vec<ResidentDelta>) {
        if self.end.is_some() || deltas.is_empty() {
            return;
        }
        if let Err(err) = self.client.send_deltas(deltas).await {
            tracing::info!(
                error = %format!("{err:#}"),
                "listener unreachable; ending residency"
            );
            self.end = Some(LoopEnd::ListenerGone);
        }
    }

    async fn fail(&mut self, reason: &str) {
        if self.end.is_some() {
            return;
        }
        tracing::error!(
            wave = self.wave,
            reason,
            "wave loop failed; reporting and exiting"
        );
        self.send(vec![ResidentDelta::LoopState {
            to: ResidentStateTo::Failed,
            reason: reason.to_string(),
        }])
        .await;
        if self.end.is_none() {
            self.end = Some(LoopEnd::Failed(reason.to_string()));
        }
    }
}

fn spawn_wave_step(
    cwd: &Path,
    step: &StepRef,
    seed: &str,
    max_turns: Option<u32>,
) -> std::io::Result<tokio::process::Child> {
    let mut command = tokio::process::Command::from(lf_command());
    command.arg("-b");
    if let Some(max_turns) = max_turns {
        command.arg("--max-turns").arg(max_turns.to_string());
    }
    command
        .arg("__flow-step")
        .arg(&step.flow)
        .arg(step.index.to_string())
        .arg(seed)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    command.spawn()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::chat::turns::{ChatRole, ChatTurn};
    use crate::harness::Capabilities;
    use crate::wave::journal::{journal_path, EventKind, Journal};
    use crate::wave::runtime::WaveRuntime;
    use crate::wave::server::{self, ResidentDoor};
    use crate::wave::state::LoopState;
    use async_trait::async_trait;

    /// The rig: a REAL listener (runtime + router with the resident door)
    /// and a resident (subscription follower + `run_loop_with` and a
    /// stub pass spawner) connected by HTTP. Every pass is a real `sh -c`
    /// child, so pass lifecycles are real processes with real exits; the
    /// spawner records each pass's seed so tests can assert on wakes.
    struct TestLoop {
        runtime: Arc<WaveRuntime>,
        seeds: Arc<Mutex<Vec<String>>>,
        loop_task: tokio::task::JoinHandle<Result<()>>,
        /// The listener half runs on its OWN tokio runtime so a test can
        /// kill it for real: shutting the runtime down drops the accept loop
        /// AND every per-connection task (axum spawns those detached), which
        /// is what an actual dead listener process looks like on the wire.
        listener: Option<tokio::runtime::Runtime>,
        _tmp: tempfile::TempDir,
    }

    impl Drop for TestLoop {
        fn drop(&mut self) {
            if let Some(rt) = self.listener.take() {
                // Non-blocking teardown; dropping a runtime inline would
                // panic inside the async test.
                rt.shutdown_background();
            }
        }
    }

    impl TestLoop {
        fn journal_events(&self) -> Vec<EventKind> {
            let path = journal_path(self.runtime.repo_root(), "ship");
            let (_, events) = Journal::open(&path).expect("read journal");
            events.into_iter().map(|e| e.kind).collect()
        }

        fn pass_count(&self) -> usize {
            self.seeds.lock().unwrap().len()
        }

        fn seed(&self, index: usize) -> String {
            self.seeds.lock().unwrap()[index].clone()
        }
    }

    fn test_config(heartbeat: Duration) -> LoopConfig {
        LoopConfig {
            heartbeat_idle: heartbeat,
            pass_timeout: Duration::from_secs(5),
            max_turns: None,
        }
    }

    /// Boot with a far-away heartbeat; `script` is what every pass runs.
    fn boot(
        heartbeat: Duration,
        script: &'static str,
    ) -> impl std::future::Future<Output = TestLoop> {
        boot_in(tempfile::tempdir().expect("tempdir"), heartbeat, script)
    }

    async fn boot_in(
        tmp: tempfile::TempDir,
        heartbeat: Duration,
        script: &'static str,
    ) -> TestLoop {
        boot_with(tmp, test_config(heartbeat), script).await
    }

    /// Boot the loop over a script-driven process body, recording the seeds
    /// each pass was handed.
    async fn boot_with(
        tmp: tempfile::TempDir,
        config: LoopConfig,
        script: &'static str,
    ) -> TestLoop {
        let seeds = Arc::new(Mutex::new(Vec::new()));
        let spawn_seeds = seeds.clone();
        let spawn_pass: SpawnPass = Box::new(move |cwd, _step, seed, _max_turns| {
            spawn_seeds.lock().unwrap().push(seed.to_string());
            let mut command = tokio::process::Command::new("sh");
            command
                .arg("-c")
                .arg(script)
                .current_dir(cwd)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true);
            command.spawn()
        });
        boot_backend(tmp, config, BodyBackend::Process(spawn_pass), seeds).await
    }

    /// Both halves of a live loop over whichever body the test wants: the
    /// listener (runtime + HTTP surface with the resident door, on its own
    /// tokio runtime — see `TestLoop::listener`) and the resident (attach,
    /// subscribe, run the loop over the wire).
    async fn boot_backend(
        tmp: tempfile::TempDir,
        config: LoopConfig,
        backend: BodyBackend,
        seeds: Arc<Mutex<Vec<String>>>,
    ) -> TestLoop {
        let runtime =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let door = ResidentDoor::new("test-token");
        let std_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std_listener.set_nonblocking(true).unwrap();
        let addr = std_listener.local_addr().unwrap();
        let app = server::router(
            runtime.clone(),
            door,
            None,
            None,
            server::ShutdownDoor::new(),
        );
        let listener = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("listener runtime");
        listener.spawn(async move {
            let tcp = tokio::net::TcpListener::from_std(std_listener).expect("adopt listener");
            axum::serve(tcp, app).await.ok();
        });

        let client = ListenerClient::new(addr.to_string(), "test-token".to_string());
        let attach = client.attach(std::process::id()).await.expect("attach");
        assert_eq!(attach.wave, "ship");
        let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
        tokio::spawn(crate::wave::resident::follow_inbox(
            addr.to_string(),
            inbox_tx,
        ));

        let loop_task = tokio::spawn(run_loop_with(
            client,
            inbox_rx,
            tmp.path().to_path_buf(),
            tmp.path().to_path_buf(),
            "ship".into(),
            config,
            backend,
        ));
        TestLoop {
            runtime,
            seeds,
            loop_task,
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

    fn wake_of(seed: &str) -> String {
        let start = seed.find("<wake>\n").expect("seed has a wake") + "<wake>\n".len();
        let end = seed.find("\n</wake>").expect("seed closes the wake");
        seed[start..end].to_string()
    }

    struct SteeringHarness {
        events: mpsc::UnboundedSender<ConversationEvent>,
        inputs: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl Harness for SteeringHarness {
        async fn start(&mut self, _config: &crate::engine::AgentConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> Result<()> {
            let mut inputs = self.inputs.lock().expect("inputs lock");
            inputs.push(content.to_string());
            let index = inputs.len();
            drop(inputs);
            if index == 1 {
                let _ = self.events.send(ConversationEvent::TurnStarted {
                    turn_id: "vendor-turn".to_string(),
                });
                let _ = self.events.send(ConversationEvent::TextDelta {
                    turn_id: "vendor-turn".to_string(),
                    content: "hello".to_string(),
                });
            } else {
                let _ = self.events.send(ConversationEvent::TextDelta {
                    turn_id: "vendor-turn".to_string(),
                    content: " world".to_string(),
                });
                let _ = self.events.send(ConversationEvent::TurnCompleted {
                    turn_id: "vendor-turn".to_string(),
                    status: Lifecycle::Completed,
                });
                let _ = self.events.send(ConversationEvent::TurnUsage {
                    turn_id: "vendor-turn".to_string(),
                    usage: TurnUsage {
                        input_tokens: 20,
                        output_tokens: 2,
                        ..TurnUsage::default()
                    },
                });
            }
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
            }
        }

        fn provider_session_id(&self) -> Option<String> {
            (!self.inputs.lock().expect("inputs lock").is_empty())
                .then(|| "vendor-session".to_string())
        }
    }

    #[tokio::test]
    async fn steer_reaches_the_live_body_and_streams_into_one_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(tmp.path())
            .status()
            .expect("git init");
        assert!(status.success());

        let inputs = Arc::new(Mutex::new(Vec::new()));
        let harness_inputs = inputs.clone();
        let backend = BodyBackend::Harness {
            prepare: Box::new(|skill, seed, _wave, max_turns| {
                Ok(crate::lf::commands::run::PreparedHarnessTurn {
                    config: crate::engine::AgentConfig {
                        agent: Some("fake".to_string()),
                        cwd: None,
                        max_turns,
                        ..crate::engine::AgentConfig::default()
                    },
                    input: format!("{skill}\n{seed}"),
                    context: crate::trace::PreparedTurnContext::from_prompts(
                        "",
                        &format!("{skill}\n{seed}"),
                    ),
                    harness: "fake".to_string(),
                    model: None,
                    context_gather_ms: 0,
                    context_render_ms: 0,
                })
            }),
            create: Box::new(move |_name, _approval, events| {
                Ok(Box::new(SteeringHarness {
                    events,
                    inputs: harness_inputs.clone(),
                }))
            }),
        };
        let loop_ = boot_backend(
            tmp,
            test_config(Duration::from_secs(600)),
            backend,
            Arc::new(Mutex::new(Vec::new())),
        )
        .await;
        let runtime = loop_.runtime.clone();

        runtime
            .deliver(MessageOp::Message, "begin".into())
            .expect("user turn");
        wait_for("initial live input", || inputs.lock().unwrap().len() == 1).await;
        let steer = runtime
            .deliver(MessageOp::Steer, "finish".into())
            .expect("user turn");
        wait_for("completed streamed turn", || {
            runtime.thread_snapshot().iter().any(|turn| {
                turn.role == ChatRole::Assistant
                    && turn.status == Lifecycle::Completed
                    && turn.text == "hello world"
            })
        })
        .await;

        assert_eq!(inputs.lock().unwrap()[1], "finish");
        let completed = runtime
            .thread_snapshot()
            .into_iter()
            .find(|turn| turn.role == ChatRole::Assistant)
            .expect("assistant turn");
        assert_eq!(
            completed.body.and_then(|body| body.session_id),
            Some("vendor-session".to_string())
        );
        assert!(loop_.journal_events().iter().any(|kind| matches!(
            kind,
            EventKind::TurnSteered { answers, .. }
                if answers == &[message_id(&steer)]
        )));
        let playhead = runtime.playhead().expect("playhead");
        assert_eq!(playhead.now.expect("next step").step, "wave_pursue");
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
            "the loop's PATH resolves `lf` to this build first"
        );
    }

    #[test]
    fn wave_pass_seed_carries_goal_loopflow_and_wake() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let goal_dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&goal_dir).expect("goal dir");
        std::fs::write(goal_dir.join("GOAL.md"), "Ship the thing.").expect("goal");

        let seed = wave_pass_seed(tmp.path(), "ship", "hello from chat");

        assert!(seed.contains("Ship the thing."));
        assert!(seed.contains("<lf:loopflow>"));
        assert!(seed.contains("<wake>\nhello from chat\n</wake>"));
        assert_eq!(LoopConfig::default().heartbeat_idle, Duration::ZERO);
    }

    // -- Scheduling, over the full wire --

    #[tokio::test]
    async fn message_while_idle_starts_a_pass_answering_it() {
        let loop_ = boot(Duration::from_secs(600), "echo hi!").await;
        let user_turn = loop_
            .runtime
            .deliver(MessageOp::Message, "hello wave".into())
            .expect("user turn");
        wait_for("pass spawned", || loop_.pass_count() == 1).await;
        assert_eq!(wake_of(&loop_.seed(0)), "hello wave");

        wait_for("assistant turn", || {
            loop_.runtime.thread_snapshot().iter().any(|t| {
                t.role == ChatRole::Assistant
                    && t.status == Lifecycle::Completed
                    && t.text.contains("hi!")
            })
        })
        .await;

        let answers = started_answers(&loop_.journal_events());
        assert_eq!(answers, vec![vec![message_id(&user_turn)]]);
    }

    /// A say emission wakes the loop like a message: the next pass's
    /// wake carries the byline and its `TurnStarted.answers` consumes the id.
    #[tokio::test]
    async fn say_wakes_the_loop_and_is_consumed_by_the_next_pass() {
        let loop_ = boot(Duration::from_secs(600), "echo noted").await;
        let turn = loop_.runtime.deliver_say(
            "implement run-1 finished: PR #7, one surprise".into(),
            "worker".into(),
        );
        wait_for("pass spawned", || loop_.pass_count() == 1).await;
        assert_eq!(
            wake_of(&loop_.seed(0)),
            "[worker] implement run-1 finished: PR #7, one surprise",
            "the pass wake carries the byline"
        );

        wait_for("turn journaled", || {
            !started_answers(&loop_.journal_events()).is_empty()
        })
        .await;
        assert_eq!(
            started_answers(&loop_.journal_events())[0],
            vec![message_id(&turn)],
            "the say emission is consumed like any queued message"
        );
    }

    /// Messages landing mid-pass queue — never rejected, never injected —
    /// and one boundary pass drains the whole queue.
    #[tokio::test]
    async fn messages_during_a_pass_coalesce_into_one_boundary_pass() {
        let loop_ = boot(Duration::from_secs(600), "sleep 0.4; echo done").await;
        loop_
            .runtime
            .deliver(MessageOp::Message, "first".into())
            .expect("user turn");
        wait_for("pass 1 spawned", || loop_.pass_count() == 1).await;

        // Two messages land mid-pass: queued, never rejected. Give the SSE
        // hop time to reach the loop before the pass exits (the biased
        // select then guarantees they're queued before the boundary drains).
        let m2 = loop_
            .runtime
            .deliver(MessageOp::Message, "second".into())
            .expect("user turn");
        let m3 = loop_
            .runtime
            .deliver(MessageOp::Message, "third".into())
            .expect("user turn");

        // One boundary pass drains the whole queue.
        wait_for("boundary pass spawned", || loop_.pass_count() == 2).await;
        let wake = wake_of(&loop_.seed(1));
        assert!(wake.contains("second") && wake.contains("third"));

        wait_for("second TurnStarted journaled", || {
            started_answers(&loop_.journal_events()).len() == 2
        })
        .await;
        let answers = started_answers(&loop_.journal_events());
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

    /// A due schedule in GOAL.md frontmatter opens a system pass while idle.
    #[tokio::test]
    async fn cron_due_opens_a_system_pass() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Every second: due immediately at boot (grace window), and the
        // heartbeat is far away so the first pass is the cron's.
        write_goal_with_crons(tmp.path(), "  - flow: qa\n    schedule: '* * * * * *'\n");
        let loop_ = boot_in(tmp, Duration::from_secs(600), "echo ok").await;

        wait_for("cron pass spawned", || loop_.pass_count() >= 1).await;
        assert_eq!(wake_of(&loop_.seed(0)), "cron due: qa — dispatch it");
    }

    /// A schedule with no occurrence between the grace window and now stays
    /// quiet — no pass spawns.
    #[tokio::test]
    async fn cron_not_due_stays_quiet() {
        let tmp = tempfile::tempdir().expect("tempdir");
        write_goal_with_crons(
            tmp.path(),
            "  - flow: qa\n    schedule: '0 0 0 1 1 * 2099'\n",
        );
        let loop_ = boot_in(tmp, Duration::from_secs(600), "echo ok").await;

        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(loop_.pass_count(), 0, "nothing due, nothing fired");
    }

    #[tokio::test]
    async fn heartbeat_fires_when_idle_and_not_while_a_pass_runs() {
        let loop_ = boot(Duration::from_millis(50), "sleep 0.3; echo beat").await;
        // Quiet wave: the heartbeat starts a progress pass with the nudge.
        wait_for("heartbeat pass", || loop_.pass_count() == 1).await;
        assert_eq!(wake_of(&loop_.seed(0)), HEARTBEAT_PROMPT);

        // While the pass runs (0.3s against a 50ms heartbeat), no further
        // heartbeat fires.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(loop_.pass_count(), 1, "no heartbeat while a pass runs");

        // After the boundary, idle resumes and the next heartbeat fires.
        wait_for("next heartbeat", || loop_.pass_count() >= 2).await;
    }

    #[tokio::test]
    async fn heartbeat_carries_in_flight_workers_when_present() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Journal the observations before the loop boots so the first
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

        let loop_ = boot_in(tmp, Duration::from_millis(50), "echo ok").await;
        wait_for("heartbeat pass", || loop_.pass_count() == 1).await;
        let wake = wake_of(&loop_.seed(0));
        assert!(wake.starts_with(HEARTBEAT_PROMPT));
        assert!(wake.contains("<in_flight>"), "in-flight section present");
        assert!(wake.contains("run run-7 · implement: wire the observation tail · running"));
        assert!(
            !wake.contains("run-8"),
            "finished workers are not in flight"
        );
        assert!(
            wake.contains(
                "run run-9 · design: multi-line task with an indented second line · running"
            ),
            "multi-line tasks flatten to one in-flight line"
        );
    }

    // -- Failure and teardown --

    /// The failure cap ends the RESIDENT: `run_loop` returns an error
    /// after reporting `LoopState::Failed` over the wire. No in-process limbo
    /// — revival is the listener supervisor's respawn (tested in
    /// supervisor.rs).
    #[tokio::test]
    async fn failure_cap_reports_failed_and_exits_the_resident() {
        let mut loop_ = boot(Duration::from_millis(30), "exit 1").await;
        // Heartbeats keep opening passes; every pass exits nonzero.
        wait_for("loop failed", || {
            matches!(loop_.runtime.loop_state(), LoopState::Failed { .. })
        })
        .await;
        let LoopState::Failed { reason } = loop_.runtime.loop_state() else {
            unreachable!()
        };
        assert!(reason.contains("consecutive wave failures"), "{reason}");
        assert!(
            loop_.pass_count() >= MAX_CONSECUTIVE_PASS_FAILURES as usize,
            "the cap took the full ladder"
        );

        // …and the resident's loop ends with that error (process exits 1).
        let outcome = tokio::time::timeout(Duration::from_secs(5), &mut loop_.loop_task)
            .await
            .expect("loop task ends")
            .expect("loop task not cancelled");
        let err = outcome.expect_err("loop failure is an error exit");
        assert!(err.to_string().contains("consecutive wave failures"));
    }

    /// A pass overrunning its timeout is killed and finishes its turn
    /// `Failed` — one failure, not a loop death.
    #[tokio::test]
    async fn pass_timeout_kills_the_child_and_fails_the_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = LoopConfig {
            heartbeat_idle: Duration::from_secs(600),
            pass_timeout: Duration::from_millis(100),
            max_turns: None,
        };
        let loop_ = boot_with(tmp, config, "sleep 30").await;
        loop_
            .runtime
            .deliver(MessageOp::Message, "go".into())
            .expect("user turn");
        wait_for("pass spawned", || loop_.pass_count() == 1).await;

        wait_for("turn failed", || {
            loop_
                .runtime
                .thread_snapshot()
                .iter()
                .any(|t| t.role == ChatRole::Assistant && t.status == Lifecycle::Failed)
        })
        .await;
        wait_for("back to idle", || {
            loop_.runtime.loop_state() == LoopState::Idle
        })
        .await;
    }

    #[tokio::test]
    async fn active_child_loop_renews_the_pass_lease() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = LoopConfig {
            heartbeat_idle: Duration::from_secs(600),
            pass_timeout: Duration::from_millis(80),
            max_turns: None,
        };
        let loop_ = boot_with(tmp, config, "sleep 0.25; echo inhabited").await;
        assert!(loop_.runtime.journal_run_observed(
            "run-child",
            "session-child",
            "task",
            "foreground loop"
        ));
        loop_
            .runtime
            .deliver(MessageOp::Message, "go".into())
            .expect("user turn");
        wait_for("pass spawned", || loop_.pass_count() == 1).await;
        wait_for("long pass completed", || {
            loop_.runtime.thread_snapshot().iter().any(|turn| {
                turn.role == ChatRole::Assistant
                    && turn.status == Lifecycle::Completed
                    && turn.text.contains("inhabited")
            })
        })
        .await;
        assert!(
            !loop_
                .runtime
                .thread_snapshot()
                .iter()
                .any(|turn| turn.status == Lifecycle::Failed),
            "presence renewed the lease instead of timing out"
        );
    }

    /// An interrupt mid-pass kills the child and finalizes the turn
    /// `Interrupted`; the journal walks Turning → Interrupting → Idle.
    #[tokio::test]
    async fn interrupt_kills_the_pass_and_finalizes_the_turn_interrupted() {
        let loop_ = boot(Duration::from_secs(600), "sleep 30").await;
        loop_
            .runtime
            .deliver(MessageOp::Message, "start".into())
            .expect("user turn");
        wait_for("pass spawned", || loop_.pass_count() == 1).await;
        wait_for("turning", || loop_.runtime.loop_state().name() == "turning").await;

        loop_.runtime.deliver_interrupt();
        wait_for("idle again", || {
            loop_.runtime.loop_state() == LoopState::Idle
        })
        .await;

        // The turn is a well-formed interrupted record, and the journal
        // walked Turning → Interrupting → Idle.
        let thread = loop_.runtime.thread_snapshot();
        assert_eq!(
            thread.last().unwrap().status,
            Lifecycle::Interrupted,
            "partial turn finalized as a value, not a crash"
        );
        let path: Vec<(String, String)> = loop_
            .journal_events()
            .iter()
            .filter_map(|kind| match kind {
                EventKind::LoopState { from, to, .. } => {
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

    /// The listener disappearing ends the residency CLEANLY: the subscription
    /// closes, `run_loop` returns Ok — the keeper is gone, nothing to
    /// revive from this side (tmux/systemd restarts are the human's
    /// arrangement).
    #[tokio::test]
    async fn listener_death_ends_the_resident_cleanly() {
        let mut loop_ = boot(Duration::from_secs(600), "echo ok").await;

        // Kill the listener for real: its runtime goes down, every live
        // connection with it.
        loop_
            .listener
            .take()
            .expect("listener alive")
            .shutdown_background();
        // A killed listener is indistinguishable from a restarting one until
        // the retry ladder (LISTENER_RETRY_DELAYS, ~15s) exhausts — so clean
        // exit takes the full ladder by design. Timeout must clear it with CI
        // headroom.
        let outcome = tokio::time::timeout(Duration::from_secs(30), &mut loop_.loop_task)
            .await
            .expect("loop task ends after listener death")
            .expect("loop task not cancelled");
        assert!(
            outcome.is_ok(),
            "listener death is a clean exit: {outcome:?}"
        );
    }
}
