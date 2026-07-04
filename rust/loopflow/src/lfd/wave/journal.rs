//! The wave's append-only event log — runtime truth for the live agent.
//!
//! One JSONL file per wave at `.lf/journal/waves/<name>/journal.jsonl`
//! (already covered by the repo's `.lf/journal/` gitignore entry — the log is
//! per-machine, never committed). Every projection is a fold over it: the
//! thread is the conversation events, the mind state is the last `MindState`
//! event, the message queue is `UserMessage`s not yet named in any
//! `TurnStarted.answers`. Store is truth; the SSE broadcast bus is liveness.
//!
//! These events are internal persistence, NOT wire DTOs — there is no
//! Swift/Python mirror obligation. The no-defaults discipline still applies:
//! every field is required or explicitly `Option`, because replay integrity
//! depends on explicit fields. Each line carries `v: 1` so the format can be
//! migrated.
//!
//! Two kinds have no producers yet: `WorkerDispatched`/`WorkerFinished`
//! arrive with the lfd-observation phase (the server tails lfd's event
//! stream — these are confirmed facts, not commands); `MemoryUpdated`
//! arrives when the mind starts curating MEMORY.md deliberately. They are
//! defined now so the log format is settled before its producers land.
//! `ThreadStarted` is produced by the mind: the vendor thread id is its
//! first durable act, journaled before the first turn.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::conversations::turns::{ChatRole, ChatTurn};
use crate::lfd::conversations::types::{ConversationItem, Lifecycle};
use crate::lfd::wave::state::MindState;

/// Current journal format version, stamped on every line.
const FORMAT_VERSION: u32 = 1;

/// Identifies one user message within a wave (`"msg-<seq>"`, from the seq of
/// its `UserMessage` event).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub String);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How a user message asks to be handled. The POST /messages wire body is
/// still `{text}` — every message is `Message` until steering lands with the
/// mind phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageOp {
    /// Append; queued; the next turn answers it.
    Message,
    /// Inject into the current turn; falls back to `Message` when idle.
    Steer,
    /// Cancel the current turn; non-empty text becomes the next turn.
    Interrupt,
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

/// How a dispatched worker ended. No producers until the lfd-observation
/// phase, which may extend this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOutcome {
    Completed,
    Failed,
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
            .unwrap_or_default()
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
    },
    TurnStarted {
        turn_id: String,
        /// Consumption marker: the queued user messages this turn's prompt
        /// consumed. Queue = `UserMessage`s not named in any `answers`.
        answers: Vec<MessageId>,
    },
    TurnItem {
        turn_id: String,
        /// A `Message` item is a prose fragment (folds into `ChatTurn.text`);
        /// anything else folds into `ChatTurn.items`.
        item: ConversationItem,
    },
    TurnFinished {
        turn_id: String,
        status: Lifecycle,
        usage: Usage,
    },
    // -- mind lifecycle --
    ThreadStarted {
        vendor: String,
        thread_id: String,
    },
    MindState {
        from: MindState,
        to: MindState,
        reason: String,
    },
    // -- orchestration (observations, not commands) --
    WorkerDispatched {
        run_id: String,
        session_id: String,
        flow: String,
        task: String,
    },
    WorkerFinished {
        run_id: String,
        outcome: WorkerOutcome,
        summary: String,
    },
    // -- memory --
    MemoryUpdated {
        summary: String,
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

/// Append-only writer over one wave's JSONL log.
///
/// There is exactly one `Journal` per running wave, owned by the runtime and
/// serialized behind its lock — one writer appends and broadcasts; readers
/// fold. Appends flush per line (no fsync — a lost tail is a truncated tail,
/// which `open` tolerates).
#[derive(Debug)]
pub struct Journal {
    file: File,
    next_seq: u64,
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
        Ok((Self { file, next_seq }, events))
    }

    /// The seq the next appended event will get. Callers that embed ids in
    /// the event body (e.g. `"turn-<seq>"`) build them from this.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// Append one event, building its kind from the seq it will get (so ids
    /// like `"turn-<seq>"` live inside the event that mints them). Flushes the
    /// line. A write failure is logged, not propagated — the in-memory state
    /// stays live and the fault is loud in the logs.
    pub fn append(&mut self, build: impl FnOnce(u64) -> EventKind) -> Event {
        let event = Event {
            v: FORMAT_VERSION,
            seq: self.next_seq,
            at: OffsetDateTime::now_utc(),
            kind: build(self.next_seq),
        };
        self.next_seq += 1;
        match serde_json::to_string(&event) {
            Ok(line) => {
                if let Err(err) = writeln!(self.file, "{line}").and_then(|_| self.file.flush()) {
                    tracing::error!(seq = event.seq, error = %err, "failed to append journal event");
                }
            }
            Err(err) => {
                tracing::error!(seq = event.seq, error = %err, "failed to serialize journal event");
            }
        }
        event
    }
}

/// The thread and mind state materialized from a journal.
#[derive(Debug)]
pub struct ThreadFold {
    /// User turns and finalized assistant turns, in commit order (user turns
    /// at their `UserMessage` event; assistant turns at their `TurnFinished`).
    pub turns: Vec<ChatTurn>,
    /// Turns started but never finished — the crash tail. The boot janitor
    /// finalizes these as `Failed`.
    pub open: Vec<ChatTurn>,
    /// Last `MindState` transition's destination; `Idle` if none.
    pub state: MindState,
    /// Last `ThreadStarted`'s vendor thread id — the resume handle for the
    /// mind's persistent vendor session.
    pub thread_id: Option<String>,
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
    let mut state = MindState::Idle;
    let mut thread_id: Option<String> = None;

    for event in events {
        match &event.kind {
            EventKind::UserMessage { text, .. } => {
                let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text.clone());
                turn.created_at = event.at_rfc3339();
                turns.push(turn);
            }
            EventKind::TurnStarted { turn_id, .. } => {
                open.push(ChatTurn {
                    id: turn_id.clone(),
                    role: ChatRole::Assistant,
                    text: String::new(),
                    status: Lifecycle::Running,
                    items: Vec::new(),
                    created_at: event.at_rfc3339(),
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
                if let ConversationItem::Message { text, .. } = item {
                    if !turn.text.is_empty() {
                        turn.text.push('\n');
                    }
                    turn.text.push_str(text);
                } else {
                    turn.items.push(item.clone());
                }
            }
            EventKind::TurnFinished {
                turn_id, status, ..
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
                turns.push(turn);
            }
            EventKind::MindState { to, .. } => {
                state = to.clone();
            }
            EventKind::ThreadStarted {
                thread_id: started, ..
            } => {
                thread_id = Some(started.clone());
            }
            EventKind::WorkerDispatched { .. }
            | EventKind::WorkerFinished { .. }
            | EventKind::MemoryUpdated { .. } => {}
        }
    }

    ThreadFold {
        turns,
        open,
        state,
        thread_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn unknown_format_version_is_an_error() {
        let (_tmp, path) = open_tmp();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "{\"v\":2,\"seq\":1,\"at\":\"2026-07-04T00:00:00Z\",\"kind\":{\"type\":\"memory_updated\",\"summary\":\"x\"}}\n",
        )
        .unwrap();
        assert!(Journal::open(&path).is_err());
    }

    #[test]
    fn event_round_trips_every_kind() {
        let kinds = vec![
            user_message(1, "hi"),
            EventKind::TurnStarted {
                turn_id: "turn-2".into(),
                answers: vec![MessageId("msg-1".into())],
            },
            EventKind::TurnItem {
                turn_id: "turn-2".into(),
                item: ConversationItem::Message {
                    id: "text-0".into(),
                    text: "working on it".into(),
                    phase: None,
                },
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
            },
            EventKind::ThreadStarted {
                vendor: "codex".into(),
                thread_id: "thread-abc".into(),
            },
            EventKind::MindState {
                from: MindState::Idle,
                to: MindState::Turning {
                    turn_id: "turn-2".into(),
                },
                reason: "turn opened".into(),
            },
            EventKind::WorkerDispatched {
                run_id: "run-1".into(),
                session_id: "sess-1".into(),
                flow: "design".into(),
                task: "sketch the journal".into(),
            },
            EventKind::WorkerFinished {
                run_id: "run-1".into(),
                outcome: WorkerOutcome::Completed,
                summary: "landed".into(),
            },
            EventKind::MemoryUpdated {
                summary: "learned the fold".into(),
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

    #[test]
    fn fold_joins_message_items_into_text_and_keeps_state() {
        let (_tmp, path) = open_tmp();
        let (mut journal, _) = Journal::open(&path).expect("open");
        let mut events = Vec::new();
        events.push(journal.append(|seq| user_message(seq, "how goes it?")));
        events.push(journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers: vec![],
        }));
        let turn_id = "turn-2".to_string();
        events.push(journal.append(|_| EventKind::MindState {
            from: MindState::Idle,
            to: MindState::Turning {
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
        }));
        events.push(journal.append(|_| EventKind::ThreadStarted {
            vendor: "codex".into(),
            thread_id: "thread-abc".into(),
        }));

        let fold = fold_thread(&events);
        assert!(fold.open.is_empty());
        assert_eq!(fold.thread_id.as_deref(), Some("thread-abc"));
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
            MindState::Turning {
                turn_id: turn_id.clone()
            }
        );
    }

    #[test]
    fn fold_surfaces_unfinished_turns_as_open() {
        let (_tmp, path) = open_tmp();
        let (mut journal, _) = Journal::open(&path).expect("open");
        let started = journal.append(|seq| EventKind::TurnStarted {
            turn_id: format!("turn-{seq}"),
            answers: vec![],
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
