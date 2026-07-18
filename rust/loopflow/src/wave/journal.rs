//! The wave's append-only event log — runtime truth for the live agent.
//!
//! One JSONL file per served wave at
//! `.lf/journal/waves/<name>/journal.jsonl` under the origin repo (already
//! covered by the repo's `.lf/journal/` gitignore entry — the log is
//! per-machine, never committed). Work-line channels are ephemeral bus topics
//! and never own journals. Every projection is a fold over the wave's log:
//! the thread is the conversation events, the loop state is the last
//! `LoopState` event, and the message queue is `UserMessage`s not yet named in
//! any `TurnStarted.answers` or `TurnSteered.answers`. The journal is truth;
//! SSE is liveness.
//!
//! These events are internal persistence, NOT wire DTOs — there is no
//! Swift/Python mirror obligation. The no-defaults discipline still applies:
//! every field is required or explicitly `Option`, because replay integrity
//! depends on explicit fields. Each line carries `v: 1` so the format can be
//! migrated.
//!
//! `RunObserved`/`RunCompleted` remain readable for journals written before
//! Project and Tasks replaced generic workers. New child lifecycle
//! facts arrive as typed Project and Task observations. `MemoryUpdated` and
//! `MemoryAdded` are produced by the server's memory routes (`lf memory
//! update`/`add` — the server holds MEMORY.md's pen). `ServerStarted` is
//! appended once per boot, after replay — restarts are forensically visible
//! in the record.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::chat::types::{ConversationItem, Lifecycle};
use crate::project::ProjectObservation;
use crate::receipt::Receipt;
use crate::task::TaskObservation;
use crate::wave::playhead::{BodyProvenance, Playhead, PlayheadEvent};
use crate::wave::state::LoopState;

/// Current journal format version, stamped on every line.
const FORMAT_VERSION: u32 = 1;

/// Identifies one user message within a wave (`"msg-<seq>"`, from the seq of
/// its `UserMessage` event).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub String);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How a user message asks to be handled. This is both the journaled op and
/// the `POST /messages` wire op (`{op, text}`, snake_case) — explicit at the
/// API, never inferred. The journaled op records *intent*; what actually
/// happened is recorded by consumption (`TurnStarted.answers` for queued
/// messages, `TurnSteered.answers` for mid-turn injection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageOp {
    /// Append; queued; the next turn answers it.
    Message,
    /// Inject into the current turn; degrades to `Message` when idle or when
    /// the harness can't steer.
    Steer,
    /// Cancel the current turn; non-empty text becomes the next turn.
    Interrupt,
    /// An attributed emission (`lf radio pub`): a worker report, child-wave
    /// escalation, or CLI FYI. Lands in the thread as an attributed statement
    /// AND queues for the loop exactly like `Message` — same consumption
    /// machinery, `TurnStarted.answers` can name it.
    Say,
}

/// Token usage accrued over one turn. Providers report different subsets, so
/// every field is explicitly optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
}

impl Usage {
    /// Explicitly-empty usage (nothing reported), e.g. for janitor-finalized
    /// turns. Not a serde default — absent fields are still a parse error.
    pub fn empty() -> Self {
        Self {
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cost_usd: None,
        }
    }
}

/// How an observed worker ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOutcome {
    Completed,
    Failed,
}

impl WorkerOutcome {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// A journaled user message that has not yet been consumed by a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingMessage {
    pub id: MessageId,
    pub op: MessageOp,
    pub text: String,
    /// The byline a `Say` emission arrived under ("worker", "wave goals",
    /// "bus"); `None` for the unattributed human thread.
    pub from: Option<String>,
}

/// One journal row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Format version (`FORMAT_VERSION`).
    pub v: u32,
    /// Monotonic per-wave sequence. Turn and message ids derive from it, so a
    /// restarted server continues the same id space.
    pub seq: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub at: OffsetDateTime,
    pub kind: EventKind,
}

impl Event {
    /// The event timestamp as RFC 3339, the format `ChatTurn.created_at` uses.
    pub fn at_rfc3339(&self) -> String {
        self.at
            .format(&time::format_description::well_known::Rfc3339)
            .expect("journal timestamps are representable as RFC 3339")
    }
}

/// What happened. See the module doc for which kinds have producers today.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    // -- conversation --
    UserMessage {
        id: MessageId,
        op: MessageOp,
        text: String,
        /// The byline of a `Say` emission; `None` for plain user messages.
        /// It rides the existing `UserMessage` row (an emission is a user
        /// message with a byline), so the queue fold — `UserMessage`s not
        /// named in any `answers` — stays untouched: no new event kind, no
        /// second inbox to desync.
        from: Option<String>,
    },
    TurnStarted {
        turn_id: String,
        /// Consumption marker: the queued user messages this turn's prompt
        /// consumed. Queue = `UserMessage`s not named in any `answers`.
        answers: Vec<MessageId>,
        /// Exact body attempt producing this assistant span. Instantaneous
        /// injected turns carry `None`.
        body: Option<Box<BodyProvenance>>,
    },
    TurnItem {
        turn_id: String,
        /// A prose `Message` folds into `ChatTurn.text`; a `commentary` message
        /// and every non-message item stay in `ChatTurn.items` (see
        /// `ChatTurn::absorb_item`).
        item: ConversationItem,
    },
    TurnSteered {
        turn_id: String,
        /// Consumption marker for steered messages: these `UserMessage`s were
        /// injected into the turn *while it ran* (harness `send_input`
        /// mid-turn), so the current turn answers them — no later turn will.
        /// `TurnStarted.answers` can't be amended (append-only log), so
        /// mid-turn consumption gets its own row.
        answers: Vec<MessageId>,
    },
    /// The undo of a consumption claim: these messages were named in an
    /// `answers` but the turn that claimed them never completed (janitor- or
    /// force-finalized, failed, or the resident reported the vendor never
    /// received them). The fold returns them to the pending queue, so a
    /// resident replay re-delivers them instead of losing them forever.
    MessagesRequeued {
        ids: Vec<MessageId>,
    },
    TurnFinished {
        turn_id: String,
        status: Lifecycle,
        usage: Usage,
        termination_reason: Option<String>,
    },
    // -- loop lifecycle --
    LoopState {
        from: LoopState,
        to: LoopState,
        reason: String,
    },
    /// One durable playhead transition plus the complete state after it. The
    /// event explains why the cursor moved; the snapshot makes restart replay
    /// exact without re-running scheduling decisions.
    PlayheadChanged {
        event: PlayheadEvent,
        playhead: Box<Playhead>,
    },
    // -- orchestration (observations, not commands) --
    RunObserved {
        run_id: String,
        session_id: String,
        flow: String,
        task: String,
    },
    RunCompleted {
        run_id: String,
        outcome: WorkerOutcome,
        summary: String,
    },
    TaskObserved {
        observation: TaskObservation,
    },
    ProjectObserved {
        observation: ProjectObservation,
    },
    // -- legacy channels --
    /// A work-line channel opened under this wave. No current code produces
    /// this event; retaining the variant lets existing journals replay.
    ChannelOpened {
        /// The child channel's name — exactly the worktree basename minus
        /// the repo prefix (`goals.148e0e02`).
        name: String,
        run_id: String,
    },
    // -- memory --
    /// A compiled memory checkpoint was written to `MEMORY.md`. Clears the
    /// replayable add delta because the checkpoint is now the seed.
    MemoryUpdated {
        summary: String,
    },
    /// A fact published to the stream (`lf memory add`). Accumulates into the
    /// replayable delta until the next `MemoryUpdated`.
    MemoryAdded {
        fact: String,
        /// Evidence receipts binding the fact to the raw records that justify
        /// it. `#[serde(default)]` is deliberate replayed-log evolution: a
        /// `MemoryAdded` row written before receipts existed lacks the field,
        /// and `read_events` stops the whole fold on the first parse error — so
        /// an absent list must decode as empty, not truncate the journal. The
        /// `Receipt` type itself stays default-free as a wire DTO.
        #[serde(default)]
        receipts: Vec<Receipt>,
    },
    // -- server lifecycle --
    /// One boot of the wave server, appended after replay. Folds ignore it;
    /// it exists so restarts are visible in the forensic record.
    ServerStarted {
        pid: u32,
        endpoint: String,
    },
}

/// Path of a wave's journal: `.lf/journal/waves/<wave>/journal.jsonl` under
/// the repo root.
pub fn journal_path(repo_root: &Path, wave: &str) -> PathBuf {
    repo_root
        .join(".lf")
        .join("journal")
        .join("waves")
        .join(wave)
        .join("journal.jsonl")
}

/// What a read-only journal consumer can honestly say about its evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOnlyJournalState {
    Available,
    Missing,
    Partial,
    Unavailable,
}

/// Events recovered without taking ownership of the Wave journal.
#[derive(Debug)]
pub struct ReadOnlyJournal {
    pub events: Vec<Event>,
    pub state: ReadOnlyJournalState,
    pub detail: Option<String>,
}

/// Read a journal's events without becoming a writer, retaining evidence state.
///
/// The ambient-context read path (every `lf` run inside a wave) folds over
/// this from arbitrary processes, so it must never create the file or
/// truncate a torn tail — the running wave server owns the pen. A torn or
/// future-version tail returns the valid prefix as `Partial`; missing and
/// unreadable files stay distinct from a valid empty journal.
pub fn read_events_with_state(path: &Path) -> ReadOnlyJournal {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return ReadOnlyJournal {
                events: Vec::new(),
                state: ReadOnlyJournalState::Missing,
                detail: Some("No durable Wave Chat history exists yet.".to_string()),
            };
        }
        Err(err) => {
            return ReadOnlyJournal {
                events: Vec::new(),
                state: ReadOnlyJournalState::Unavailable,
                detail: Some(format!("Could not read durable Wave Chat history: {err}")),
            };
        }
    };
    let mut events = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Event>(trimmed) {
            Ok(event) if event.v == FORMAT_VERSION => events.push(event),
            Ok(event) => {
                tracing::warn!(
                    path = %path.display(),
                    version = event.v,
                    "journal line from another format version; stopping read-only fold"
                );
                return ReadOnlyJournal {
                    events,
                    state: ReadOnlyJournalState::Partial,
                    detail: Some(format!(
                        "Durable Wave Chat history stops at line {}: format v{} is incompatible with v{FORMAT_VERSION}.",
                        index + 1,
                        event.v
                    )),
                };
            }
            Err(_) => {
                return ReadOnlyJournal {
                    events,
                    state: ReadOnlyJournalState::Partial,
                    detail: Some(format!(
                        "Durable Wave Chat history stops at unreadable line {}.",
                        index + 1
                    )),
                };
            }
        }
    }
    ReadOnlyJournal {
        events,
        state: ReadOnlyJournalState::Available,
        detail: None,
    }
}

/// Read the valid event prefix for callers that do not present evidence state.
pub fn read_events(path: &Path) -> Vec<Event> {
    read_events_with_state(path).events
}

// -- Console narration --------------------------------------------------
//
// The console is a human-readable projection of the journal: every append
// emits exactly one tracing line. INFO narrates everything meaningful; the
// item-level prose and thoughts (bulky — the thread has them in full) ride
// at DEBUG (`RUST_LOG=loopflow=debug`). The tap lives here, at the
// single-writer choke point, so no producer can journal silently.

/// Loudness of one narrated line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NarrationLevel {
    Info,
    Debug,
}

/// One rendered console line.
#[derive(Debug)]
struct Narration {
    level: NarrationLevel,
    line: String,
}

fn info(line: String) -> Narration {
    Narration {
        level: NarrationLevel::Info,
        line,
    }
}

fn debug(line: String) -> Narration {
    Narration {
        level: NarrationLevel::Debug,
        line,
    }
}

/// Narration state for one in-progress turn: the item count for its closing
/// line, and whether its first prose fragment already gave the gist at INFO.
#[derive(Debug)]
struct TurnNarration {
    turn_id: String,
    items: usize,
    text_shown: bool,
}

/// Renders journal events as compact console lines. Owned by the [`Journal`],
/// so it sees exactly the appends (never the boot replay).
#[derive(Debug, Default)]
struct Narrator {
    turns: Vec<TurnNarration>,
}

impl Narrator {
    fn narrate(&mut self, kind: &EventKind) {
        let narration = self.render(kind);
        match narration.level {
            NarrationLevel::Info => tracing::info!("{}", narration.line),
            NarrationLevel::Debug => tracing::debug!("{}", narration.line),
        }
    }

    /// One console line per event. The match is exhaustive on purpose — a new
    /// `EventKind` fails compilation here instead of going silent on the
    /// console.
    fn render(&mut self, kind: &EventKind) -> Narration {
        match kind {
            EventKind::UserMessage { id, op, text, from } => {
                let op_tag = match op {
                    MessageOp::Message | MessageOp::Say => "",
                    MessageOp::Steer => "(steer) ",
                    MessageOp::Interrupt => "(interrupt) ",
                };
                let byline = from
                    .as_ref()
                    .map(|from| format!("[{from}] "))
                    .unwrap_or_default();
                info(format!(
                    "chat ← {op_tag}{byline}\"{}\" ({id})",
                    ellipsize(text, 60)
                ))
            }
            EventKind::TurnStarted {
                turn_id, answers, ..
            } => {
                let turn = self.turn_mut(turn_id);
                turn.items = 0;
                turn.text_shown = false;
                info(format!("turn {turn_id} opened{}", answers_segment(answers)))
            }
            EventKind::TurnItem { turn_id, item } => {
                let turn = self.turn_mut(turn_id);
                turn.items += 1;
                match item {
                    ConversationItem::Command {
                        command, status, ..
                    } => info(format!(
                        "  $ {} → {}",
                        ellipsize(&command.join(" "), 70),
                        status.name()
                    )),
                    ConversationItem::Message { text, .. } => {
                        if turn.text_shown {
                            debug(format!("  loop: \"{}\"", ellipsize(text, 120)))
                        } else {
                            turn.text_shown = true;
                            info(format!("loop: \"{}\"", ellipsize(text, 80)))
                        }
                    }
                    ConversationItem::Thought { text, .. } => {
                        debug(format!("  thought: \"{}\"", ellipsize(text, 120)))
                    }
                    ConversationItem::File {
                        changes, status, ..
                    } => {
                        let what = match changes.as_slice() {
                            [only] => only.path.clone(),
                            many => format!("{} files", many.len()),
                        };
                        info(format!("  edit {what} → {}", status.name()))
                    }
                    ConversationItem::Tool { name, status, .. } => {
                        info(format!("  tool {name} → {}", status.name()))
                    }
                }
            }
            EventKind::TurnSteered { turn_id, answers } => info(format!(
                "turn {turn_id} steered{}",
                answers_segment(answers)
            )),
            EventKind::MessagesRequeued { ids } => {
                let ids: Vec<&str> = ids.iter().map(|id| id.0.as_str()).collect();
                info(format!("messages requeued: {}", ids.join(", ")))
            }
            EventKind::TurnFinished {
                turn_id,
                status,
                usage,
                ..
            } => {
                let items = self.finish_turn(turn_id);
                let plural = if items == 1 { "" } else { "s" };
                info(format!(
                    "turn {turn_id} {} · {items} item{plural}{}",
                    status.name(),
                    usage_segment(usage)
                ))
            }
            EventKind::LoopState { from, to, reason } => {
                info(format!("state {} → {} ({reason})", from.name(), to.name()))
            }
            EventKind::PlayheadChanged { event, .. } => match event {
                PlayheadEvent::FlowEnqueued { flow, .. } => {
                    info(format!("playhead enqueued · {flow}"))
                }
                PlayheadEvent::InvocationStarted { flow, .. } => {
                    info(format!("playhead entered · {flow}"))
                }
                PlayheadEvent::InvocationCompleted { flow, .. } => {
                    info(format!("playhead completed · {flow}"))
                }
                PlayheadEvent::StepStarted { step, .. } => {
                    info(format!("playhead now · {} / {}", step.flow, step.step))
                }
                PlayheadEvent::BodySessionUpdated { session_id, .. } => {
                    info(format!("playhead session · {session_id}"))
                }
                PlayheadEvent::StepFinished {
                    step,
                    outcome,
                    reason,
                    ..
                } => info(format!(
                    "playhead {} · {} / {} ({reason})",
                    outcome.name(),
                    step.flow,
                    step.step
                )),
            },
            EventKind::RunObserved {
                run_id, flow, task, ..
            } => info(format!(
                "observed worker {} flow={flow} started · {}",
                short_id(run_id),
                ellipsize(task, 60)
            )),
            EventKind::RunCompleted {
                run_id,
                outcome,
                summary,
            } => info(format!(
                "observed run {} {} · {}",
                short_id(run_id),
                outcome.name(),
                ellipsize(summary, 60)
            )),
            EventKind::TaskObserved { observation } => info(format!(
                "observed task {} event {} · {}",
                observation.issue_identifier,
                observation.event_id,
                ellipsize(&observation.prompt(), 70)
            )),
            EventKind::ProjectObserved { observation } => info(format!(
                "observed project {} event {} · {}",
                observation.project,
                observation.event_id,
                ellipsize(&observation.prompt(), 70)
            )),
            EventKind::ChannelOpened { name, run_id } => {
                info(format!("channel {name} opened · run {}", short_id(run_id)))
            }
            EventKind::MemoryUpdated { summary } => {
                info(format!("memory curated: {}", ellipsize(summary, 70)))
            }
            EventKind::MemoryAdded { fact, .. } => {
                info(format!("memory added: {}", ellipsize(fact, 70)))
            }
            EventKind::ServerStarted { pid, endpoint } => {
                info(format!("server started · pid {pid} · {endpoint}"))
            }
        }
    }

    fn turn_mut(&mut self, turn_id: &str) -> &mut TurnNarration {
        if let Some(pos) = self.turns.iter().position(|t| t.turn_id == turn_id) {
            return &mut self.turns[pos];
        }
        self.turns.push(TurnNarration {
            turn_id: turn_id.to_string(),
            items: 0,
            text_shown: false,
        });
        self.turns.last_mut().expect("just pushed")
    }

    fn finish_turn(&mut self, turn_id: &str) -> usize {
        match self.turns.iter().position(|t| t.turn_id == turn_id) {
            Some(pos) => self.turns.remove(pos).items,
            None => 0,
        }
    }
}

/// Flatten whitespace and cap at `max` chars (with an ellipsis when cut).
/// Shared by journal narration and the CLI thread renderer.
pub(crate) fn ellipsize(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let mut cut: String = flat.chars().take(max).collect();
    cut.push('…');
    cut
}

/// Humanized token count: `812`, `1.4k`, `192k`.
fn fmt_tokens(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let k = n as f64 / 1000.0;
    if k < 10.0 {
        format!("{k:.1}k")
    } else {
        format!("{k:.0}k")
    }
}

fn answers_segment(answers: &[MessageId]) -> String {
    if answers.is_empty() {
        return String::new();
    }
    let ids: Vec<&str> = answers.iter().map(|id| id.0.as_str()).collect();
    format!(" (answers: {})", ids.join(", "))
}

fn usage_segment(usage: &Usage) -> String {
    let mut parts = Vec::new();
    if let Some(input) = usage.input_tokens {
        parts.push(format!("{} in", fmt_tokens(input)));
    }
    if let Some(output) = usage.output_tokens {
        parts.push(format!("{} out", fmt_tokens(output)));
    }
    if parts.is_empty() {
        return String::new();
    }
    let mut segment = format!(" · {}", parts.join(" / "));
    if let Some(cached) = usage.cache_read_tokens {
        segment.push_str(&format!(" ({} cached)", fmt_tokens(cached)));
    }
    segment
}

/// A ledger identity shortened for the console (ids correlate by prefix).
/// Shared by the run, exec, and trace surfaces.
pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Append-only writer over one wave's JSONL log.
///
/// There is exactly one `Journal` per running wave, owned by the runtime and
/// serialized behind its lock — one writer appends and broadcasts; readers
/// fold. Appends flush per line (no fsync — a lost tail is a truncated tail,
/// which `open` tolerates). Every append also narrates one console line (see
/// [`Narrator`]); replayed events on `open` are not re-narrated.
#[derive(Debug)]
pub struct Journal {
    file: File,
    next_seq: u64,
    narrator: Narrator,
    #[cfg(test)]
    next_append_failure: Option<JournalAppendStage>,
}

/// A journal append failed before it became durable.
#[derive(Debug, thiserror::Error)]
#[error("journal append at seq {seq} failed during {operation}: {source}")]
pub struct JournalAppendError {
    seq: u64,
    operation: &'static str,
    #[source]
    source: std::io::Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JournalAppendStage {
    Write,
    Flush,
}

impl Journal {
    /// Open (or create) the journal at `path`, replaying existing events.
    ///
    /// A corrupt or partial trailing region (crash mid-write) is tolerated:
    /// the file is truncated back to the last parseable line and a warning is
    /// logged — boot never fails on a torn write. A line with an unknown
    /// format version is a real error (future data must not be destroyed).
    ///
    /// # Errors
    /// I/O failure, or a journal written by a newer format version.
    pub fn open(path: &Path) -> anyhow::Result<(Self, Vec<Event>)> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err.into()),
        };

        let mut events = Vec::new();
        let mut good_bytes = 0usize;
        let mut offset = 0usize;
        for line in raw.split_inclusive('\n') {
            let start = offset;
            offset += line.len();
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<Event>(trimmed) {
                Ok(event) if event.v == FORMAT_VERSION => {
                    events.push(event);
                    good_bytes = offset;
                }
                Ok(event) => {
                    anyhow::bail!(
                        "journal {} has format v{} at seq {}; this build reads v{FORMAT_VERSION}",
                        path.display(),
                        event.v,
                        event.seq,
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        byte_offset = start,
                        error = %err,
                        "journal has an unparseable tail (crash mid-write?); truncating to last parseable line"
                    );
                    break;
                }
            }
        }
        if good_bytes < raw.len() {
            let file = OpenOptions::new().write(true).open(path)?;
            file.set_len(good_bytes as u64)?;
        }

        let next_seq = events.last().map(|e| e.seq + 1).unwrap_or(1);
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok((
            Self {
                file,
                next_seq,
                narrator: Narrator::default(),
                #[cfg(test)]
                next_append_failure: None,
            },
            events,
        ))
    }

    /// The seq the next appended event will get. `append` reads the field
    /// directly; this accessor lets a test assert the id space survives a
    /// reopen.
    #[cfg(test)]
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Append one event, stopping the process if persistence fails.
    ///
    /// Most runtime paths cannot recover locally: their next action projects
    /// the event into memory, so continuing would split the live view from
    /// restart truth. Request paths that can report failure use
    /// [`Self::try_append`] directly instead.
    pub fn append(&mut self, build: impl FnOnce(u64) -> EventKind) -> Event {
        self.try_append(build)
            .expect("journal truth must persist before projecting an event in memory")
    }

    /// Append and flush one event before advancing its sequence or narrating
    /// it. A failed write is rolled back to the prior byte boundary so the
    /// same runtime can retry without leaving a torn tail or consuming an id.
    ///
    /// # Errors
    /// Serialization, file inspection, write, flush, or rollback failure.
    pub fn try_append(
        &mut self,
        build: impl FnOnce(u64) -> EventKind,
    ) -> Result<Event, JournalAppendError> {
        let event = Event {
            v: FORMAT_VERSION,
            seq: self.next_seq,
            at: OffsetDateTime::now_utc(),
            kind: build(self.next_seq),
        };
        let mut line = serde_json::to_vec(&event).map_err(|source| JournalAppendError {
            seq: event.seq,
            operation: "serialization",
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
        })?;
        line.push(b'\n');
        let checkpoint = self
            .file
            .metadata()
            .map_err(|source| JournalAppendError {
                seq: event.seq,
                operation: "file inspection",
                source,
            })?
            .len();

        let write_result = match self.take_injected_failure(JournalAppendStage::Write) {
            Some(source) => Err(source),
            None => self.file.write_all(&line),
        };
        if let Err(source) = write_result {
            return Err(self.rollback(event.seq, checkpoint, "write", source));
        }

        let flush_result = match self.take_injected_failure(JournalAppendStage::Flush) {
            Some(source) => Err(source),
            None => self.file.flush(),
        };
        if let Err(source) = flush_result {
            return Err(self.rollback(event.seq, checkpoint, "flush", source));
        }

        self.next_seq += 1;
        self.narrator.narrate(&event.kind);
        Ok(event)
    }

    fn rollback(
        &mut self,
        seq: u64,
        checkpoint: u64,
        operation: &'static str,
        source: std::io::Error,
    ) -> JournalAppendError {
        match self.file.set_len(checkpoint) {
            Ok(()) => JournalAppendError {
                seq,
                operation,
                source,
            },
            Err(rollback) => JournalAppendError {
                seq,
                operation: "write rollback",
                source: std::io::Error::new(
                    source.kind(),
                    format!(
                        "{operation} failed: {source}; truncating to byte {checkpoint} also failed: {rollback}"
                    ),
                ),
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn fail_next_append(&mut self, failure: JournalAppendStage) {
        self.next_append_failure = Some(failure);
    }

    fn take_injected_failure(
        &mut self,
        #[cfg_attr(not(test), allow(unused_variables))] expected: JournalAppendStage,
    ) -> Option<std::io::Error> {
        #[cfg(test)]
        if self.next_append_failure == Some(expected) {
            self.next_append_failure = None;
            return Some(std::io::Error::other("injected journal append failure"));
        }
        None
    }
}

/// The thread and loop state materialized from a journal.
#[derive(Debug)]
pub struct ThreadFold {
    /// User turns and finalized assistant turns, in commit order (user turns
    /// at their `UserMessage` event; assistant turns at their `TurnFinished`).
    pub turns: Vec<ChatTurn>,
    /// Turns started but never finished — the crash tail. The boot janitor
    /// finalizes these as `Failed`.
    pub open: Vec<ChatTurn>,
    /// Last `LoopState` transition's destination; `Idle` if none.
    pub state: LoopState,
    /// Last durable playhead snapshot, absent before the first resident or
    /// enqueue initializes the default wave flow.
    pub playhead: Option<Playhead>,
    /// User messages not named by any `TurnStarted.answers` or
    /// `TurnSteered.answers` (minus what `MessagesRequeued` restored); this
    /// seeds the scheduler queue on restart.
    pub pending_messages: Vec<PendingMessage>,
    /// Every journaled user message by id — `MessagesRequeued` restores
    /// pending entries from it (an id alone can't rebuild the text/op/from).
    pub messages: HashMap<MessageId, PendingMessage>,
    /// Typed Task observations indexed by their synthetic consumption id.
    pub tasks: HashMap<MessageId, TaskObservation>,
    /// Typed Project observations indexed by their synthetic consumption id.
    pub projects: HashMap<MessageId, ProjectObservation>,
    /// Message ids claimed (`answers`) by turns still open at the end of the
    /// log — the crash tail's consumption. The boot janitor requeues these
    /// when it finalizes the crashed turns as `Failed`.
    pub open_claims: Vec<MessageId>,
    /// Memory facts added this server life — the replayable stream a fresh
    /// subscriber gets before going live. Rebuilt from the journal on restart.
    pub memory_adds: Vec<String>,
}

/// One curated memory fact with its evidence receipts. Wire type for
/// `lf memory log --json`: both fields required, no serde defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryFact {
    pub fact: String,
    pub receipts: Vec<Receipt>,
}

/// The curated facts (with receipts) live in the current replayable delta —
/// every `MemoryAdded` since the last `MemoryUpdated` checkpoint, oldest first.
/// This is the receipt-bearing view behind `lf memory log --json`; the plain
/// `memory_adds` fold above drops receipts because the live stream never needed
/// them.
pub fn memory_facts(events: &[Event]) -> Vec<MemoryFact> {
    let mut facts = Vec::new();
    for event in events {
        match &event.kind {
            EventKind::MemoryAdded { fact, receipts } => facts.push(MemoryFact {
                fact: fact.clone(),
                receipts: receipts.clone(),
            }),
            EventKind::MemoryUpdated { .. } => facts.clear(),
            _ => {}
        }
    }
    facts
}

/// Materialize a historical `ChannelOpened` event during journal replay.
fn legacy_channel_opened_turn(event: &Event, name: &str) -> ChatTurn {
    let mut turn = ChatTurn::user(
        format!("turn-{}", event.seq),
        format!("work line {name} opened"),
    );
    turn.created_at = event.at_rfc3339();
    turn.from = Some("worker".to_string());
    turn
}

/// The thread-visible turn a `RunCompleted` observation materializes: the
/// worker's ending as a bylined statement, never queued for the loop (only
/// `UserMessage` rows feed the pending queue). Covers the died-silently case
/// — a worker that never reported still ends visibly, failure summary on the
/// wire. Shared by the fold and the live append so replay and the live
/// thread agree byte for byte.
pub fn run_completed_turn(
    event: &Event,
    run_id: &str,
    outcome: WorkerOutcome,
    summary: &str,
) -> ChatTurn {
    let mut text = format!("run {} {}", short_id(run_id), outcome.name());
    if !summary.trim().is_empty() {
        text.push_str(&format!(" · {}", summary.trim()));
    }
    let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text);
    turn.created_at = event.at_rfc3339();
    turn.from = Some("observer".to_string());
    turn
}

/// Re-queue `ids` into `pending`: each id known in `messages` and not already
/// pending is appended, in `ids` order. Returns the ids actually restored —
/// unknown or still-pending ids are skipped (idempotent by construction).
/// Shared by the fold (`MessagesRequeued` events) and the live runtime, which
/// journals exactly what this restored.
pub fn restore_pending(
    pending: &mut Vec<PendingMessage>,
    messages: &HashMap<MessageId, PendingMessage>,
    ids: &[MessageId],
) -> Vec<MessageId> {
    let mut restored = Vec::new();
    for id in ids {
        if pending.iter().any(|message| &message.id == id) {
            continue;
        }
        let Some(message) = messages.get(id) else {
            tracing::warn!(id = %id, "requeue of an unknown message id; dropped");
            continue;
        };
        pending.push(message.clone());
        restored.push(id.clone());
    }
    restored
}

/// Fold journal events into the thread — the pure function the in-memory
/// `Vec<ChatTurn>` cache materializes.
///
/// `Message` items are prose fragments: they join into `ChatTurn.text` with
/// `'\n'`, exactly as the live open-turn snapshot accumulates text. All other
/// items land in `ChatTurn.items`.
pub fn fold_thread(events: &[Event]) -> ThreadFold {
    let mut turns: Vec<ChatTurn> = Vec::new();
    // In-order list, not a map: the crash tail keeps its start order.
    let mut open: Vec<ChatTurn> = Vec::new();
    let mut state = LoopState::Idle;
    let mut playhead: Option<Playhead> = None;
    let mut pending_messages: Vec<PendingMessage> = Vec::new();
    let mut messages: HashMap<MessageId, PendingMessage> = HashMap::new();
    let mut tasks: HashMap<MessageId, TaskObservation> = HashMap::new();
    let mut projects: HashMap<MessageId, ProjectObservation> = HashMap::new();
    let mut consumed_messages: HashSet<MessageId> = HashSet::new();
    // Claims (`answers`) per still-open turn — the crash tail's consumption,
    // exported so the boot janitor can requeue it.
    let mut claims_by_open_turn: HashMap<String, Vec<MessageId>> = HashMap::new();
    let mut memory_adds: Vec<String> = Vec::new();

    for event in events {
        match &event.kind {
            EventKind::UserMessage { id, op, text, from } => {
                let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text.clone());
                turn.created_at = event.at_rfc3339();
                turn.from = from.clone();
                turns.push(turn);
                let message = PendingMessage {
                    id: id.clone(),
                    op: *op,
                    text: text.clone(),
                    from: from.clone(),
                };
                if !consumed_messages.contains(id) {
                    pending_messages.push(message.clone());
                }
                messages.insert(id.clone(), message);
            }
            EventKind::TaskObserved { observation } => {
                let message = task_observation_message(observation);
                let turn = ChatTurn::child_activity(
                    format!("turn-{}", event.seq),
                    event.at_rfc3339(),
                    "task".to_string(),
                    crate::chat::turns::ChildControlActivity::from_task(observation),
                );
                turns.push(turn);
                if !consumed_messages.contains(&message.id) {
                    pending_messages.push(message.clone());
                }
                tasks.insert(message.id.clone(), observation.clone());
                messages.insert(message.id.clone(), message);
            }
            EventKind::ProjectObserved { observation } => {
                let message = project_observation_message(observation);
                let turn = ChatTurn::child_activity(
                    format!("turn-{}", event.seq),
                    event.at_rfc3339(),
                    "project".to_string(),
                    crate::chat::turns::ChildControlActivity::from_project(observation),
                );
                turns.push(turn);
                if !consumed_messages.contains(&message.id) {
                    pending_messages.push(message.clone());
                }
                projects.insert(message.id.clone(), observation.clone());
                messages.insert(message.id.clone(), message);
            }
            EventKind::TurnStarted {
                turn_id,
                answers,
                body,
            } => {
                mark_consumed(&mut pending_messages, &mut consumed_messages, answers);
                claims_by_open_turn.insert(turn_id.clone(), answers.clone());
                open.push(ChatTurn {
                    id: turn_id.clone(),
                    role: ChatRole::Assistant,
                    text: String::new(),
                    status: Lifecycle::Running,
                    items: Vec::new(),
                    created_at: event.at_rfc3339(),
                    from: None,
                    body: body.as_deref().cloned(),
                    activity: None,
                });
            }
            EventKind::TurnItem { turn_id, item } => {
                let Some(turn) = open.iter_mut().find(|t| &t.id == turn_id) else {
                    tracing::warn!(
                        turn_id,
                        seq = event.seq,
                        "TurnItem for a turn that isn't open"
                    );
                    continue;
                };
                turn.absorb_item(item.clone());
            }
            EventKind::TurnFinished {
                turn_id,
                status,
                termination_reason,
                ..
            } => {
                let Some(pos) = open.iter().position(|t| &t.id == turn_id) else {
                    tracing::warn!(
                        turn_id,
                        seq = event.seq,
                        "TurnFinished for a turn that isn't open"
                    );
                    continue;
                };
                let mut turn = open.remove(pos);
                turn.status = *status;
                turn.close_body(event.at_rfc3339(), termination_reason.clone());
                claims_by_open_turn.remove(turn_id);
                turns.push(turn);
            }
            EventKind::LoopState { to, .. } => {
                state = to.clone();
            }
            EventKind::PlayheadChanged {
                event,
                playhead: snapshot,
            } => {
                if let PlayheadEvent::BodySessionUpdated {
                    body_id,
                    session_id,
                } = event
                {
                    if let Some(body) = open
                        .iter_mut()
                        .filter_map(|turn| turn.body.as_mut())
                        .find(|body| &body.body_id == body_id)
                    {
                        body.session_id = Some(session_id.clone());
                    }
                }
                playhead = Some(snapshot.as_ref().clone());
            }
            // Steer consumption affects the queue fold, not the thread: the
            // steered text is already a user turn via its `UserMessage` row.
            EventKind::TurnSteered { turn_id, answers } => {
                mark_consumed(&mut pending_messages, &mut consumed_messages, answers);
                // Steered into a still-open turn: part of that turn's claims
                // (requeued with them if the turn crashes). The fallback arm
                // names an already-closed turn — the vendor heard the text,
                // nothing to requeue.
                if let Some(claims) = claims_by_open_turn.get_mut(turn_id) {
                    claims.extend(answers.iter().cloned());
                }
            }
            EventKind::MessagesRequeued { ids } => {
                restore_pending(&mut pending_messages, &messages, ids);
            }
            EventKind::ChannelOpened { name, .. } => {
                turns.push(legacy_channel_opened_turn(event, name));
            }
            EventKind::RunCompleted {
                run_id,
                outcome,
                summary,
            } => {
                turns.push(run_completed_turn(event, run_id, *outcome, summary));
            }
            EventKind::MemoryAdded { fact, .. } => {
                memory_adds.push(fact.clone());
            }
            EventKind::MemoryUpdated { .. } => {
                memory_adds.clear();
            }
            EventKind::RunObserved { .. } | EventKind::ServerStarted { .. } => {}
        }
    }

    // The crash tail's claims, in the open turns' start order.
    let open_claims = open
        .iter()
        .filter_map(|turn| claims_by_open_turn.remove(&turn.id))
        .flatten()
        .collect();

    ThreadFold {
        turns,
        open,
        state,
        playhead,
        pending_messages,
        messages,
        tasks,
        projects,
        open_claims,
        memory_adds,
    }
}

pub fn task_observation_message(observation: &TaskObservation) -> PendingMessage {
    PendingMessage {
        id: MessageId(observation.inbox_id()),
        op: MessageOp::Message,
        text: observation.prompt(),
        from: Some("task".to_string()),
    }
}

pub fn project_observation_message(observation: &ProjectObservation) -> PendingMessage {
    PendingMessage {
        id: MessageId(observation.inbox_id()),
        op: MessageOp::Message,
        text: observation.prompt(),
        from: Some("project".to_string()),
    }
}

fn mark_consumed(
    pending_messages: &mut Vec<PendingMessage>,
    consumed_messages: &mut HashSet<MessageId>,
    answers: &[MessageId],
) {
    if answers.is_empty() {
        return;
    }
    for answer in answers {
        consumed_messages.insert(answer.clone());
    }
    pending_messages.retain(|message| !consumed_messages.contains(&message.id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::EvidenceKind;

    #[test]
    fn memory_fact_fixture_round_trips_every_evidence_kind() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/receipt.json"
        ));
        let fact: MemoryFact = serde_json::from_str(fixture).expect("decode receipt fixture");
        let kinds: Vec<EvidenceKind> = fact.receipts.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EvidenceKind::ChatTurn,
                EvidenceKind::WorkerReport,
                EvidenceKind::Trace,
                EvidenceKind::Pm,
                EvidenceKind::Pr,
            ],
        );
        // A PR reference keeps its `@sha` and its wave differs from the others —
        // the cross-wave case doctor detects downstream.
        let pr = fact.receipts.last().expect("pr receipt");
        assert_eq!(pr.reference, "loopflow/loopflow#912@abc1234");
        assert_eq!(pr.wave, "auditability");

        let reencoded = serde_json::to_string(&fact).expect("serialize");
        let decoded: MemoryFact = serde_json::from_str(&reencoded).expect("re-decode");
        assert_eq!(fact, decoded);
    }

    #[test]
    fn memory_facts_keep_receipts_and_reset_at_each_checkpoint() {
        let events = vec![
            Event {
                v: FORMAT_VERSION,
                seq: 1,
                at: OffsetDateTime::UNIX_EPOCH,
                kind: EventKind::MemoryAdded {
                    fact: "pre-checkpoint fact".into(),
                    receipts: vec![Receipt::new(EvidenceKind::ChatTurn, "turn-1", "ship")],
                },
            },
            Event {
                v: FORMAT_VERSION,
                seq: 2,
                at: OffsetDateTime::UNIX_EPOCH,
                kind: EventKind::MemoryUpdated {
                    summary: "compiled".into(),
                },
            },
            Event {
                v: FORMAT_VERSION,
                seq: 3,
                at: OffsetDateTime::UNIX_EPOCH,
                kind: EventKind::MemoryAdded {
                    fact: "post-checkpoint fact".into(),
                    receipts: vec![Receipt::new(EvidenceKind::Pr, "o/r#5", "ship")],
                },
            },
        ];
        let facts = memory_facts(&events);
        assert_eq!(facts.len(), 1, "the checkpoint cleared the earlier delta");
        assert_eq!(facts[0].fact, "post-checkpoint fact");
        assert_eq!(
            facts[0].receipts,
            vec![Receipt::new(EvidenceKind::Pr, "o/r#5", "ship")]
        );
    }

    /// A `MemoryAdded` row written before receipts existed decodes with an empty
    /// list, so `read_events` never truncates an old journal at the first fact.
    #[test]
    fn legacy_memory_added_without_receipts_decodes_empty() {
        let line = r#"{"v":1,"seq":4,"at":"2026-07-04T00:00:00Z","kind":{"type":"memory_added","fact":"legacy fact"}}"#;
        let event: Event = serde_json::from_str(line).expect("decode legacy MemoryAdded");
        match event.kind {
            EventKind::MemoryAdded { fact, receipts } => {
                assert_eq!(fact, "legacy fact");
                assert!(receipts.is_empty());
            }
            other => panic!("expected MemoryAdded, got {other:?}"),
        }
    }

    fn open_tmp() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = journal_path(tmp.path(), "ship");
        (tmp, path)
    }

    fn user_message(seq: u64, text: &str) -> EventKind {
        EventKind::UserMessage {
            id: MessageId(format!("msg-{seq}")),
            op: MessageOp::Message,
            text: text.to_string(),
            from: None,
        }
    }

    fn task_observation() -> crate::task::TaskObservation {
        crate::task::TaskObservation {
            task_id: crate::task::TaskId::from_raw("task_example"),
            issue_identifier: "INF-123".to_string(),
            event_id: 42,
            event: crate::task::TaskEventKind::Failed {
                error: "provider stopped".to_string(),
                resumable: true,
            },
        }
    }

    #[test]
    fn append_then_open_replays_events_and_continues_seq() {
        let (_tmp, path) = open_tmp();
        {
            let (mut journal, events) = Journal::open(&path).expect("open");
            assert!(events.is_empty());
            assert_eq!(journal.next_seq(), 1);
            journal.append(|seq| user_message(seq, "hello"));
            journal.append(|seq| user_message(seq, "again"));
        }
        let (journal, events) = Journal::open(&path).expect("reopen");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].seq, 1);
        assert_eq!(events[1].seq, 2);
        assert_eq!(journal.next_seq(), 3);
    }

    #[test]
    fn corrupt_trailing_line_is_truncated_not_fatal() {
        let (_tmp, path) = open_tmp();
        {
            let (mut journal, _) = Journal::open(&path).expect("open");
            journal.append(|seq| user_message(seq, "kept"));
        }
        // Simulate a crash mid-write: a torn, unparseable tail.
        let mut raw = std::fs::read_to_string(&path).expect("read");
        raw.push_str(r#"{"v":1,"seq":2,"at":"2026-"#);
        std::fs::write(&path, &raw).expect("corrupt");

        let (mut journal, events) = Journal::open(&path).expect("reopen tolerates tail");
        assert_eq!(events.len(), 1);
        assert_eq!(journal.next_seq(), 2);
        // The tail is gone from disk and the next append lands cleanly.
        journal.append(|seq| user_message(seq, "after crash"));
        drop(journal);
        let (_, events) = Journal::open(&path).expect("reopen again");
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].seq, 2);
    }

    #[test]
    fn read_events_never_touches_the_file() {
        let (_tmp, path) = open_tmp();
        {
            let (mut journal, _) = Journal::open(&path).expect("open");
            journal.append(|seq| user_message(seq, "kept"));
        }
        // A torn tail: read_events returns the clean prefix and leaves the
        // file byte-identical — the wave server owns the pen.
        let mut raw = std::fs::read_to_string(&path).expect("read");
        raw.push_str(r#"{"v":1,"seq":2,"at":"2026-"#);
        std::fs::write(&path, &raw).expect("corrupt");

        let read = read_events_with_state(&path);
        assert_eq!(read.events.len(), 1);
        assert_eq!(read.state, ReadOnlyJournalState::Partial);
        assert!(read.detail.as_deref().unwrap().contains("line 2"));
        assert_eq!(std::fs::read_to_string(&path).expect("reread"), raw);

        // Missing file: explicitly missing, and still not created.
        let ghost = path.parent().unwrap().join("ghost.jsonl");
        let missing = read_events_with_state(&ghost);
        assert!(missing.events.is_empty());
        assert_eq!(missing.state, ReadOnlyJournalState::Missing);
        assert!(!ghost.exists());
    }

    #[test]
    fn unknown_format_version_is_an_error() {
        let (_tmp, path) = open_tmp();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "{\"v\":2,\"seq\":1,\"at\":\"2026-07-04T00:00:00Z\",\"kind\":{\"type\":\"memory_added\",\"fact\":\"x\"}}\n",
        )
        .unwrap();
        assert!(Journal::open(&path).is_err());
    }

    #[test]
    fn event_round_trips_every_kind() {
        let kinds = vec![
            user_message(1, "hi"),
            EventKind::UserMessage {
                id: MessageId("msg-9".into()),
                op: MessageOp::Say,
                text: "worker report: PR landed".into(),
                from: Some("worker".into()),
            },
            EventKind::TurnStarted {
                turn_id: "turn-2".into(),
                answers: vec![MessageId("msg-1".into())],
                body: None,
            },
            EventKind::TurnItem {
                turn_id: "turn-2".into(),
                item: ConversationItem::Message {
                    id: "text-0".into(),
                    text: "working on it".into(),
                    phase: None,
                },
            },
            EventKind::TurnSteered {
                turn_id: "turn-2".into(),
                answers: vec![MessageId("msg-3".into())],
            },
            EventKind::TurnFinished {
                turn_id: "turn-2".into(),
                status: Lifecycle::Completed,
                usage: Usage {
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    cache_read_tokens: None,
                    cost_usd: Some(0.01),
                },
                termination_reason: None,
            },
            EventKind::LoopState {
                from: LoopState::Idle,
                to: LoopState::Turning {
                    turn_id: "turn-2".into(),
                },
                reason: "turn opened".into(),
            },
            EventKind::RunObserved {
                run_id: "run-1".into(),
                session_id: "sess-1".into(),
                flow: "design".into(),
                task: "sketch the journal".into(),
            },
            EventKind::RunCompleted {
                run_id: "run-1".into(),
                outcome: WorkerOutcome::Completed,
                summary: "landed".into(),
            },
            EventKind::TaskObserved {
                observation: task_observation(),
            },
            EventKind::ChannelOpened {
                name: "ship.148e0e02".into(),
                run_id: "run-1".into(),
            },
            EventKind::MemoryUpdated {
                summary: "learned the fold".into(),
            },
            EventKind::MemoryAdded {
                fact: "learned a fact".into(),
                receipts: Vec::new(),
            },
            EventKind::ServerStarted {
                pid: 4242,
                endpoint: "127.0.0.1:50123".into(),
            },
        ];
        for (i, kind) in kinds.into_iter().enumerate() {
            let event = Event {
                v: FORMAT_VERSION,
                seq: i as u64 + 1,
                at: OffsetDateTime::now_utc(),
                kind,
            };
            let line = serde_json::to_string(&event).expect("serialize");
            let decoded: Event = serde_json::from_str(&line).expect("deserialize");
            assert_eq!(decoded, event);
        }
    }

    /// New appends carry the post-rename kind names on disk — a run completion
    /// serializes as `run_completed`, never the retired `worker_finished`.
    #[test]
    fn new_appends_carry_the_renamed_kind_names() {
        let (_tmp, path) = open_tmp();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let (mut journal, _events) = Journal::open(&path).expect("journal opens");

        journal.append(|_| EventKind::RunCompleted {
            run_id: "run-2".into(),
            outcome: WorkerOutcome::Failed,
            summary: "process gone".into(),
        });
        let raw = std::fs::read_to_string(&path).unwrap();
        let last = raw.lines().last().unwrap();
        assert!(last.contains("\"run_completed\""), "{last}");
        assert!(!last.contains("worker_finished"), "{last}");
    }

    /// Every `EventKind` narrates without panicking. The compile-time half of
    /// the guarantee lives in `Narrator::render` itself: its match has no
    /// catch-all, so a future event kind fails compilation rather than
    /// silently skipping narration.
    #[test]
    fn narration_renders_every_event_kind() {
        let kinds = vec![
            user_message(1, "hi"),
            EventKind::TurnStarted {
                turn_id: "turn-2".into(),
                answers: vec![],
                body: None,
            },
            EventKind::TurnItem {
                turn_id: "turn-2".into(),
                item: ConversationItem::Message {
                    id: "text-0".into(),
                    text: "working on it".into(),
                    phase: None,
                },
            },
            EventKind::TurnItem {
                turn_id: "turn-2".into(),
                item: ConversationItem::Command {
                    id: "cmd-0".into(),
                    command: vec!["cargo".into(), "test".into()],
                    cwd: "/repo".into(),
                    status: Lifecycle::Completed,
                    output: None,
                    exit_code: Some(0),
                    duration_ms: Some(1200),
                },
            },
            EventKind::TurnItem {
                turn_id: "turn-2".into(),
                item: ConversationItem::Thought {
                    id: "thought-0".into(),
                    text: "hmm".into(),
                },
            },
            EventKind::TurnItem {
                turn_id: "turn-2".into(),
                item: ConversationItem::File {
                    id: "file-0".into(),
                    changes: vec![],
                    status: Lifecycle::Completed,
                },
            },
            EventKind::TurnItem {
                turn_id: "turn-2".into(),
                item: ConversationItem::Tool {
                    id: "tool-0".into(),
                    name: "Bash".into(),
                    status: Lifecycle::Completed,
                    input: None,
                    output: None,
                },
            },
            EventKind::TurnSteered {
                turn_id: "turn-2".into(),
                answers: vec![MessageId("msg-3".into())],
            },
            EventKind::TurnFinished {
                turn_id: "turn-2".into(),
                status: Lifecycle::Completed,
                usage: Usage::empty(),
                termination_reason: None,
            },
            EventKind::LoopState {
                from: LoopState::Idle,
                to: LoopState::Turning {
                    turn_id: "turn-2".into(),
                },
                reason: "turn opened".into(),
            },
            EventKind::RunObserved {
                run_id: "run-1".into(),
                session_id: "sess-1".into(),
                flow: "design".into(),
                task: "sketch the journal".into(),
            },
            EventKind::RunCompleted {
                run_id: "run-1".into(),
                outcome: WorkerOutcome::Completed,
                summary: "landed".into(),
            },
            EventKind::TaskObserved {
                observation: task_observation(),
            },
            EventKind::ChannelOpened {
                name: "ship.148e0e02".into(),
                run_id: "run-1".into(),
            },
            EventKind::ServerStarted {
                pid: 4242,
                endpoint: "127.0.0.1:50123".into(),
            },
        ];
        let mut narrator = Narrator::default();
        for kind in &kinds {
            let narration = narrator.render(kind);
            assert!(!narration.line.is_empty(), "silent narration for {kind:?}");
        }
    }

    #[test]
    fn typed_task_observation_uses_the_existing_durable_consumption_fold() {
        let observation = task_observation();
        let observation_id = MessageId(observation.inbox_id());
        let observed = Event {
            v: FORMAT_VERSION,
            seq: 1,
            at: OffsetDateTime::now_utc(),
            kind: EventKind::TaskObserved {
                observation: observation.clone(),
            },
        };
        let pending = fold_thread(std::slice::from_ref(&observed));
        assert_eq!(pending.pending_messages.len(), 1);
        assert_eq!(pending.tasks.get(&observation_id), Some(&observation));
        assert_eq!(pending.turns.len(), 1);
        assert!(pending.turns[0].text.is_empty());
        assert_eq!(
            pending.turns[0]
                .activity
                .as_ref()
                .map(|activity| activity.id.as_str()),
            Some(observation_id.0.as_str())
        );

        let consumed = fold_thread(&[
            observed,
            Event {
                v: FORMAT_VERSION,
                seq: 2,
                at: OffsetDateTime::now_utc(),
                kind: EventKind::TurnStarted {
                    turn_id: "turn-2".to_string(),
                    answers: vec![observation_id],
                    body: None,
                },
            },
        ]);
        assert!(consumed.pending_messages.is_empty());
    }

    /// A fixed event sequence renders the console a human would want to read:
    /// chat with bylines and ops, turn open/close with items and usage, the
    /// prose gist once at INFO with the rest at DEBUG, legacy worker observations,
    /// memory curation.
    #[test]
    fn narration_demo_reads_like_a_console() {
        let mut narrator = Narrator::default();
        let mut render = |kind: EventKind| {
            let narration = narrator.render(&kind);
            println!(
                "{:5} {}",
                format!("{:?}", narration.level).to_uppercase(),
                narration.line
            );
            narration
        };

        let n = render(EventKind::UserMessage {
            id: MessageId("msg-1".into()),
            op: MessageOp::Message,
            text: "how is the reactive server refactor going?".into(),
            from: None,
        });
        assert_eq!(n.level, NarrationLevel::Info);
        assert_eq!(
            n.line,
            "chat ← \"how is the reactive server refactor going?\" (msg-1)"
        );

        let n = render(EventKind::UserMessage {
            id: MessageId("msg-2".into()),
            op: MessageOp::Say,
            text: "run-42 landed: PR #12 merged, one clippy fix on the side".into(),
            from: Some("worker".into()),
        });
        assert_eq!(
            n.line,
            "chat ← [worker] \"run-42 landed: PR #12 merged, one clippy fix on the side\" (msg-2)"
        );

        let n = render(EventKind::UserMessage {
            id: MessageId("msg-3".into()),
            op: MessageOp::Steer,
            text: "focus on the journal tests first".into(),
            from: None,
        });
        assert_eq!(
            n.line,
            "chat ← (steer) \"focus on the journal tests first\" (msg-3)"
        );

        let n = render(EventKind::LoopState {
            from: LoopState::Idle,
            to: LoopState::Turning {
                turn_id: "turn-4".into(),
            },
            reason: "turn opened".into(),
        });
        assert_eq!(n.line, "state idle → turning (turn opened)");

        let n = render(EventKind::TurnStarted {
            turn_id: "turn-4".into(),
            answers: vec![MessageId("msg-1".into()), MessageId("msg-2".into())],
            body: None,
        });
        assert_eq!(n.line, "turn turn-4 opened (answers: msg-1, msg-2)");

        // First prose fragment gives the gist at INFO ...
        let n = render(EventKind::TurnItem {
            turn_id: "turn-4".into(),
            item: ConversationItem::Message {
                id: "text-0".into(),
                text: "Checking the worker reports,\nthen answering the chat.".into(),
                phase: None,
            },
        });
        assert_eq!(n.level, NarrationLevel::Info);
        assert_eq!(
            n.line,
            "loop: \"Checking the worker reports, then answering the chat.\""
        );

        let n = render(EventKind::TurnItem {
            turn_id: "turn-4".into(),
            item: ConversationItem::Command {
                id: "cmd-0".into(),
                command: vec!["git".into(), "log".into(), "--oneline".into(), "-5".into()],
                cwd: "/repo".into(),
                status: Lifecycle::Completed,
                output: None,
                exit_code: Some(0),
                duration_ms: Some(80),
            },
        });
        assert_eq!(n.line, "  $ git log --oneline -5 → completed");

        // ... the rest of the prose and every thought ride at DEBUG.
        let n = render(EventKind::TurnItem {
            turn_id: "turn-4".into(),
            item: ConversationItem::Message {
                id: "text-1".into(),
                text: "The build worker is still grinding; I'll start the doc pass.".into(),
                phase: None,
            },
        });
        assert_eq!(n.level, NarrationLevel::Debug);
        let n = render(EventKind::TurnItem {
            turn_id: "turn-4".into(),
            item: ConversationItem::Thought {
                id: "thought-0".into(),
                text: "the queue is empty after this".into(),
            },
        });
        assert_eq!(n.level, NarrationLevel::Debug);

        let n = render(EventKind::TurnSteered {
            turn_id: "turn-4".into(),
            answers: vec![MessageId("msg-3".into())],
        });
        assert_eq!(n.line, "turn turn-4 steered (answers: msg-3)");

        let n = render(EventKind::TurnFinished {
            turn_id: "turn-4".into(),
            status: Lifecycle::Completed,
            usage: Usage {
                input_tokens: Some(192_400),
                output_tokens: Some(1_400),
                cache_read_tokens: Some(182_000),
                cost_usd: Some(0.42),
            },
            termination_reason: None,
        });
        assert_eq!(
            n.line,
            "turn turn-4 completed · 4 items · 192k in / 1.4k out (182k cached)"
        );

        let n = render(EventKind::RunObserved {
            run_id: "run-8c1d2e3f4a".into(),
            session_id: "sess-9".into(),
            flow: "build".into(),
            task: "wire the narration tap into the journal".into(),
        });
        assert_eq!(
            n.line,
            "observed worker run-8c1d flow=build started · wire the narration tap into the journal"
        );

        let n = render(EventKind::RunCompleted {
            run_id: "run-8c1d2e3f4a".into(),
            outcome: WorkerOutcome::Completed,
            summary: "narration tap landed, suite green".into(),
        });
        assert_eq!(
            n.line,
            "observed run run-8c1d completed · narration tap landed, suite green"
        );

        let n = render(EventKind::MemoryAdded {
            fact: "workers report through the memory stream".into(),
            receipts: Vec::new(),
        });
        assert_eq!(
            n.line,
            "memory added: workers report through the memory stream"
        );

        let n = render(EventKind::MemoryUpdated {
            summary: "journal is the console's source of truth".into(),
        });
        assert_eq!(
            n.line,
            "memory curated: journal is the console's source of truth"
        );

        let n = render(EventKind::ServerStarted {
            pid: 4242,
            endpoint: "127.0.0.1:50123".into(),
        });
        assert_eq!(n.line, "server started · pid 4242 · 127.0.0.1:50123");

        let n = render(EventKind::ChannelOpened {
            name: "ship.148e0e02".into(),
            run_id: "run-8c1d2e3f4a".into(),
        });
        assert_eq!(n.line, "channel ship.148e0e02 opened · run run-8c1d");
    }

    /// Historical `ChannelOpened` rows still fold into a thread-visible turn.
    #[test]
    fn fold_materializes_legacy_channel_opened_as_a_worker_turn() {
        let events = vec![Event {
            v: FORMAT_VERSION,
            seq: 1,
            at: OffsetDateTime::now_utc(),
            kind: EventKind::ChannelOpened {
                name: "ship.148e0e02".into(),
                run_id: "run-7".into(),
            },
        }];
        let fold = fold_thread(&events);
        assert_eq!(fold.turns.len(), 1);
        assert_eq!(fold.turns[0].text, "work line ship.148e0e02 opened");
        assert_eq!(fold.turns[0].from.as_deref(), Some("worker"));
        assert_eq!(fold.turns[0].id, "turn-1");
        assert!(
            fold.pending_messages.is_empty(),
            "a channel opening never queues for the loop"
        );
    }

    /// Long text is flattened and cut; a fresh turn resets the prose gist.
    #[test]
    fn narration_truncates_and_resets_per_turn() {
        let mut narrator = Narrator::default();
        let long = "x".repeat(100);
        let n = narrator.render(&EventKind::UserMessage {
            id: MessageId("msg-1".into()),
            op: MessageOp::Message,
            text: long.clone(),
            from: None,
        });
        assert_eq!(n.line, format!("chat ← \"{}…\" (msg-1)", "x".repeat(60)));

        for turn in ["turn-2", "turn-5"] {
            narrator.render(&EventKind::TurnStarted {
                turn_id: turn.into(),
                answers: vec![],
                body: None,
            });
            let n = narrator.render(&EventKind::TurnItem {
                turn_id: turn.into(),
                item: ConversationItem::Message {
                    id: "text-0".into(),
                    text: "gist".into(),
                    phase: None,
                },
            });
            assert_eq!(n.level, NarrationLevel::Info, "each turn gets one gist");
            narrator.render(&EventKind::TurnFinished {
                turn_id: turn.into(),
                status: Lifecycle::Completed,
                usage: Usage::empty(),
                termination_reason: None,
            });
        }
    }

    #[test]
    fn fold_joins_message_items_into_text_and_keeps_state() {
        let (_tmp, path) = open_tmp();
        let (mut journal, _) = Journal::open(&path).expect("open");
        let mut events = Vec::new();
        events.push(journal.append(|seq| user_message(seq, "how goes it?")));
        events.push(journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers: vec![],
            body: None,
        }));
        let turn_id = "turn-2".to_string();
        events.push(journal.append(|_| EventKind::LoopState {
            from: LoopState::Idle,
            to: LoopState::Turning {
                turn_id: turn_id.clone(),
            },
            reason: "turn opened".into(),
        }));
        events.push(journal.append(|_| EventKind::TurnItem {
            turn_id: turn_id.clone(),
            item: ConversationItem::Message {
                id: "text-0".into(),
                text: "first".into(),
                phase: None,
            },
        }));
        events.push(journal.append(|_| EventKind::TurnItem {
            turn_id: turn_id.clone(),
            item: ConversationItem::Tool {
                id: "item-0".into(),
                name: "Bash".into(),
                status: Lifecycle::Completed,
                input: None,
                output: None,
            },
        }));
        events.push(journal.append(|_| EventKind::TurnItem {
            turn_id: turn_id.clone(),
            item: ConversationItem::Message {
                id: "text-1".into(),
                text: "second".into(),
                phase: None,
            },
        }));
        events.push(journal.append(|_| EventKind::TurnFinished {
            turn_id: turn_id.clone(),
            status: Lifecycle::Completed,
            usage: Usage::empty(),
            termination_reason: None,
        }));

        let fold = fold_thread(&events);
        assert!(fold.open.is_empty());
        assert_eq!(fold.turns.len(), 2);
        assert_eq!(fold.turns[0].role, ChatRole::User);
        assert_eq!(fold.turns[0].id, "turn-1");
        let assistant = &fold.turns[1];
        assert_eq!(assistant.id, "turn-2");
        assert_eq!(assistant.text, "first\nsecond");
        assert_eq!(assistant.items.len(), 1);
        assert_eq!(assistant.status, Lifecycle::Completed);
        assert_eq!(
            fold.state,
            LoopState::Turning {
                turn_id: turn_id.clone()
            }
        );
    }

    /// A `Say` emission is a user message with a byline: the fold puts its
    /// attribution on the wire turn and queues it as consumable input, and a
    /// `TurnStarted.answers` naming it consumes it like any other message.
    #[test]
    fn fold_treats_say_as_attributed_consumable_input() {
        let say = |seq: u64| EventKind::UserMessage {
            id: MessageId(format!("msg-{seq}")),
            op: MessageOp::Say,
            text: "landed the parser PR".to_string(),
            from: Some("worker".into()),
        };
        let events = vec![Event {
            v: FORMAT_VERSION,
            seq: 1,
            at: OffsetDateTime::now_utc(),
            kind: say(1),
        }];
        let fold = fold_thread(&events);
        assert_eq!(fold.turns.len(), 1);
        assert_eq!(fold.turns[0].role, ChatRole::User);
        assert_eq!(fold.turns[0].from.as_deref(), Some("worker"));
        assert_eq!(fold.pending_messages.len(), 1, "say queues for the loop");
        assert_eq!(fold.pending_messages[0].op, MessageOp::Say);
        assert_eq!(fold.pending_messages[0].from.as_deref(), Some("worker"));

        // Consumption: a turn answering it drains the queue.
        let consumed = vec![
            events[0].clone(),
            Event {
                v: FORMAT_VERSION,
                seq: 2,
                at: OffsetDateTime::now_utc(),
                kind: EventKind::TurnStarted {
                    turn_id: "turn-2".into(),
                    answers: vec![MessageId("msg-1".into())],
                    body: None,
                },
            },
        ];
        let fold = fold_thread(&consumed);
        assert!(fold.pending_messages.is_empty(), "answered say is consumed");
    }

    #[test]
    fn fold_surfaces_unfinished_turns_as_open() {
        let (_tmp, path) = open_tmp();
        let (mut journal, _) = Journal::open(&path).expect("open");
        let started = journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers: vec![],
            body: None,
        });
        let turn_id = match &started.kind {
            EventKind::TurnStarted { turn_id, .. } => turn_id.clone(),
            _ => unreachable!(),
        };
        let item = journal.append(|_| EventKind::TurnItem {
            turn_id,
            item: ConversationItem::Message {
                id: "text-0".into(),
                text: "half a thought".into(),
                phase: None,
            },
        });

        let fold = fold_thread(&[started, item]);
        assert!(fold.turns.is_empty());
        assert_eq!(fold.open.len(), 1);
        assert_eq!(fold.open[0].text, "half a thought");
        assert_eq!(fold.open[0].status, Lifecycle::Running);
    }
}
