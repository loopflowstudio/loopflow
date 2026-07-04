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

/// Folds a live `StreamEvent` sequence into `ChatTurn`s.
///
/// One builder spans the whole life of a wave server; `feed` is called for every
/// event parsed from the agent's stream. It opens an assistant turn on the first
/// content event and closes it on a `Result`, returning the completed turn so the
/// server can broadcast it. `snapshot` exposes the turn currently in progress so a
/// late subscriber sees partial text.
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

    fn open_turn(&mut self) -> &mut ChatTurn {
        if self.open.is_none() {
            self.next_index += 1;
            self.open = Some(ChatTurn {
                id: format!("turn-{}", self.next_index),
                role: ChatRole::Assistant,
                text: String::new(),
                status: Lifecycle::Running,
                items: Vec::new(),
                created_at: ChatTurn::now_rfc3339(),
            });
        }
        self.open.as_mut().expect("turn just opened")
    }

    /// Feed one stream event. Returns `Some(turn)` when a turn is finalized.
    pub fn feed(&mut self, event: &StreamEvent) -> Option<ChatTurn> {
        match event {
            StreamEvent::Text(text) => {
                let turn = self.open_turn();
                if !turn.text.is_empty() {
                    turn.text.push('\n');
                }
                turn.text.push_str(text);
                None
            }
            StreamEvent::ToolUse { name, summary } => {
                let index = self.open_turn().items.len();
                let turn = self.open_turn();
                turn.items.push(tool_item(index, name, summary));
                None
            }
            StreamEvent::Usage { .. } => None,
            StreamEvent::Result { subtype, .. } => {
                let mut turn = self.open.take()?;
                turn.status = match subtype {
                    ResultSubtype::Success => Lifecycle::Completed,
                    ResultSubtype::Error => Lifecycle::Failed,
                };
                Some(turn)
            }
        }
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

    #[test]
    fn text_and_result_produce_one_completed_turn() {
        let mut builder = TurnBuilder::new();
        assert!(builder.feed(&StreamEvent::Text("hello".into())).is_none());
        assert!(builder.feed(&StreamEvent::Text("world".into())).is_none());
        let turn = builder
            .feed(&StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            })
            .expect("turn finalized on result");
        assert_eq!(turn.id, "turn-1");
        assert_eq!(turn.role, ChatRole::Assistant);
        assert_eq!(turn.text, "hello\nworld");
        assert_eq!(turn.status, Lifecycle::Completed);
        assert!(builder.snapshot().is_none());
    }

    #[test]
    fn tool_use_becomes_command_item() {
        let mut builder = TurnBuilder::new();
        builder.feed(&StreamEvent::ToolUse {
            name: "Bash".into(),
            summary: "cargo test".into(),
        });
        let snap = builder.snapshot().expect("open turn");
        assert_eq!(snap.items.len(), 1);
        assert!(matches!(
            &snap.items[0],
            ConversationItem::Command { command, .. } if command == &vec!["cargo test".to_string()]
        ));
    }

    #[test]
    fn error_result_marks_turn_failed() {
        let mut builder = TurnBuilder::new();
        builder.feed(&StreamEvent::Text("boom".into()));
        let turn = builder
            .feed(&StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            })
            .expect("turn finalized");
        assert_eq!(turn.status, Lifecycle::Failed);
    }

    #[test]
    fn each_turn_gets_a_fresh_id() {
        let mut builder = TurnBuilder::new();
        builder.feed(&StreamEvent::Text("one".into()));
        let first = builder
            .feed(&StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            })
            .expect("first turn");
        builder.feed(&StreamEvent::Text("two".into()));
        let second = builder
            .feed(&StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            })
            .expect("second turn");
        assert_eq!(first.id, "turn-1");
        assert_eq!(second.id, "turn-2");
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
