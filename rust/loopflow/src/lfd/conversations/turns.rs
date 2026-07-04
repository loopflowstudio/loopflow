//! Turn assembly: agent stream events → `ChatTurn`s.
//!
//! The wave's inner pass runs `codex exec --json` (see [`crate::engine::stream`],
//! which normalizes codex/claude/opencode JSON into [`StreamEvent`]s). This module
//! adds the richer turn/item layer on top: it folds a live stream of events into
//! [`ChatTurn`]s, the wire type the per-wave chat [`server`](super::server) serves
//! to Concerto.
//!
//! Mapping:
//! - the operator's standing directive becomes one `user` turn;
//! - each agent turn (text + tool activity, closed by a result) becomes one
//!   `assistant` turn whose `items` capture the commands/edits/messages it ran.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::engine::stream::{ResultSubtype, StreamEvent};
use crate::lfd::conversations::types::{ConversationItem, Lifecycle};

/// Who authored a turn. Mirrors Swift `MessageRole` (user/assistant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

/// One turn in a wave's conversation — the unit the chat server streams.
///
/// Wire type consumed by Concerto. Every field is required (no serde defaults):
/// the same shape round-trips through Rust and Swift.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatTurn {
    /// Stable within a running server: `"turn-1"`, `"turn-2"`, …
    pub id: String,
    pub role: ChatRole,
    /// Accumulated assistant prose (or the human message for a `user` turn).
    pub text: String,
    /// Lifecycle of the turn. A `user` turn is always `Completed`.
    pub status: Lifecycle,
    /// Tool/command/file/message items the agent produced, in order.
    pub items: Vec<ConversationItem>,
    /// RFC 3339 timestamp of when the turn opened.
    pub created_at: String,
}

impl ChatTurn {
    fn now_rfc3339() -> String {
        OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default()
    }

    /// A completed `user` turn carrying a human message.
    pub fn user(id: String, text: String) -> Self {
        Self {
            id,
            role: ChatRole::User,
            text,
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at: Self::now_rfc3339(),
        }
    }
}

/// Incremental outcome of feeding one stream event to a [`TurnBuilder`].
///
/// This is the seam the wave's journal hangs off: the builder still assembles
/// whole [`ChatTurn`]s, but every increment surfaces to the caller so it can
/// be recorded (as `TurnStarted`/`TurnItem`/`TurnFinished` events) the moment
/// it happens, not only at finalization.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnDelta {
    /// A new assistant turn opened (first content event).
    Opened,
    /// A prose fragment appended to the open turn's text.
    Text(String),
    /// An item added to the open turn.
    Item(ConversationItem),
    /// Token usage reported for the open turn.
    Usage {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
    },
    /// The open turn finalized. Carries the assembled turn whole.
    Finished {
        turn: ChatTurn,
        cost_usd: Option<f64>,
    },
}

/// Folds a live `StreamEvent` sequence into `ChatTurn`s.
///
/// One builder spans one subagent pass; `feed` is called for every event
/// parsed from the agent's stream. It opens an assistant turn on the first
/// content event and closes it on a `Result`, surfacing every increment as a
/// [`TurnDelta`]. `snapshot` exposes the turn currently in progress so a late
/// subscriber sees partial text.
///
/// Builder-local turn ids (`"turn-<n>"`, counting from 1) are placeholders:
/// the wave runtime reassigns ids from the journal's seq domain when it
/// records the turn.
#[derive(Debug, Default)]
pub struct TurnBuilder {
    next_index: u64,
    open: Option<ChatTurn>,
}

impl TurnBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The assistant turn currently being assembled, if any.
    pub fn snapshot(&self) -> Option<&ChatTurn> {
        self.open.as_ref()
    }

    /// Open a turn if none is open. Returns whether one was opened.
    fn ensure_open(&mut self) -> bool {
        if self.open.is_some() {
            return false;
        }
        self.next_index += 1;
        self.open = Some(ChatTurn {
            id: format!("turn-{}", self.next_index),
            role: ChatRole::Assistant,
            text: String::new(),
            status: Lifecycle::Running,
            items: Vec::new(),
            created_at: ChatTurn::now_rfc3339(),
        });
        true
    }

    /// Feed one stream event, returning every increment it produced (a first
    /// content event yields `Opened` plus its content delta).
    pub fn feed(&mut self, event: &StreamEvent) -> Vec<TurnDelta> {
        let mut deltas = Vec::new();
        match event {
            StreamEvent::Text(text) => {
                if self.ensure_open() {
                    deltas.push(TurnDelta::Opened);
                }
                let turn = self.open.as_mut().expect("turn just ensured open");
                if !turn.text.is_empty() {
                    turn.text.push('\n');
                }
                turn.text.push_str(text);
                deltas.push(TurnDelta::Text(text.clone()));
            }
            StreamEvent::ToolUse { name, summary } => {
                if self.ensure_open() {
                    deltas.push(TurnDelta::Opened);
                }
                let turn = self.open.as_mut().expect("turn just ensured open");
                let item = tool_item(turn.items.len(), name, summary);
                turn.items.push(item.clone());
                deltas.push(TurnDelta::Item(item));
            }
            StreamEvent::Usage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
            } => {
                deltas.push(TurnDelta::Usage {
                    input_tokens: *input_tokens,
                    output_tokens: *output_tokens,
                    cache_read_tokens: *cache_read_tokens,
                });
            }
            StreamEvent::Result {
                subtype, cost_usd, ..
            } => {
                if let Some(mut turn) = self.open.take() {
                    turn.status = match subtype {
                        ResultSubtype::Success => Lifecycle::Completed,
                        ResultSubtype::Error => Lifecycle::Failed,
                    };
                    deltas.push(TurnDelta::Finished {
                        turn,
                        cost_usd: *cost_usd,
                    });
                }
            }
        }
        deltas
    }

    /// Close any in-progress turn (e.g. the pass ended without a `Result`),
    /// marking it with the given terminal status — `Failed` for a crash,
    /// `Interrupted` for an operator interrupt.
    pub fn finish_open(&mut self, status: Lifecycle) -> Option<ChatTurn> {
        let mut turn = self.open.take()?;
        turn.status = status;
        Some(turn)
    }
}

fn tool_item(index: usize, name: &str, summary: &str) -> ConversationItem {
    use crate::lfd::conversations::types::FileEdit;

    let id = format!("item-{index}");
    match name {
        "Bash" => ConversationItem::Command {
            id,
            command: vec![summary.to_string()],
            cwd: String::new(),
            status: Lifecycle::Completed,
            output: None,
            exit_code: None,
            duration_ms: None,
        },
        "Edit" | "Write" => ConversationItem::File {
            id,
            changes: summary
                .split(", ")
                .filter(|p| !p.is_empty())
                .map(|p| FileEdit {
                    path: p.to_string(),
                    kind: None,
                    diff: None,
                })
                .collect(),
            status: Lifecycle::Completed,
        },
        _ => ConversationItem::Tool {
            id,
            name: name.to_string(),
            status: Lifecycle::Completed,
            input: None,
            output: if summary.is_empty() {
                None
            } else {
                Some(summary.to_string())
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed events, returning only the finalized turns.
    fn feed_all(builder: &mut TurnBuilder, events: &[StreamEvent]) -> Vec<ChatTurn> {
        events
            .iter()
            .flat_map(|e| builder.feed(e))
            .filter_map(|d| match d {
                TurnDelta::Finished { turn, .. } => Some(turn),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn text_and_result_produce_one_completed_turn() {
        let mut builder = TurnBuilder::new();
        let first = builder.feed(&StreamEvent::Text("hello".into()));
        assert_eq!(
            first,
            vec![TurnDelta::Opened, TurnDelta::Text("hello".into())],
            "first content event opens the turn"
        );
        assert_eq!(
            builder.feed(&StreamEvent::Text("world".into())),
            vec![TurnDelta::Text("world".into())]
        );
        let turns = feed_all(
            &mut builder,
            &[StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            }],
        );
        assert_eq!(turns.len(), 1);
        let turn = &turns[0];
        assert_eq!(turn.id, "turn-1");
        assert_eq!(turn.role, ChatRole::Assistant);
        assert_eq!(turn.text, "hello\nworld");
        assert_eq!(turn.status, Lifecycle::Completed);
        assert!(builder.snapshot().is_none());
    }

    #[test]
    fn tool_use_becomes_command_item_and_surfaces_as_delta() {
        let mut builder = TurnBuilder::new();
        let deltas = builder.feed(&StreamEvent::ToolUse {
            name: "Bash".into(),
            summary: "cargo test".into(),
        });
        assert_eq!(deltas[0], TurnDelta::Opened);
        assert!(matches!(
            &deltas[1],
            TurnDelta::Item(ConversationItem::Command { command, .. })
                if command == &vec!["cargo test".to_string()]
        ));
        let snap = builder.snapshot().expect("open turn");
        assert_eq!(snap.items.len(), 1);
        assert!(matches!(
            &snap.items[0],
            ConversationItem::Command { command, .. } if command == &vec!["cargo test".to_string()]
        ));
    }

    #[test]
    fn usage_surfaces_as_delta() {
        let mut builder = TurnBuilder::new();
        builder.feed(&StreamEvent::Text("working".into()));
        let deltas = builder.feed(&StreamEvent::Usage {
            input_tokens: Some(10),
            output_tokens: Some(5),
            cache_read_tokens: None,
        });
        assert_eq!(
            deltas,
            vec![TurnDelta::Usage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                cache_read_tokens: None,
            }]
        );
    }

    #[test]
    fn error_result_marks_turn_failed() {
        let mut builder = TurnBuilder::new();
        builder.feed(&StreamEvent::Text("boom".into()));
        let turns = feed_all(
            &mut builder,
            &[StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            }],
        );
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].status, Lifecycle::Failed);
    }

    #[test]
    fn result_without_open_turn_produces_nothing() {
        let mut builder = TurnBuilder::new();
        let deltas = builder.feed(&StreamEvent::Result {
            subtype: ResultSubtype::Success,
            cost_usd: None,
            duration_secs: None,
        });
        assert!(deltas.is_empty());
    }

    #[test]
    fn each_turn_gets_a_fresh_id() {
        let mut builder = TurnBuilder::new();
        let turns = feed_all(
            &mut builder,
            &[
                StreamEvent::Text("one".into()),
                StreamEvent::Result {
                    subtype: ResultSubtype::Success,
                    cost_usd: None,
                    duration_secs: None,
                },
                StreamEvent::Text("two".into()),
                StreamEvent::Result {
                    subtype: ResultSubtype::Success,
                    cost_usd: None,
                    duration_secs: None,
                },
            ],
        );
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].id, "turn-1");
        assert_eq!(turns[1].id, "turn-2");
    }

    #[test]
    fn user_turn_round_trips_through_json() {
        let turn = ChatTurn::user("turn-0".into(), "please fix the build".into());
        let value = serde_json::to_value(&turn).expect("serialize");
        let decoded: ChatTurn = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, turn);
        assert_eq!(decoded.role, ChatRole::User);
    }
}
