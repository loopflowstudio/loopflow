//! Turn vocabulary: `ChatTurn` (the wire type Concerto consumes) and
//! `TurnDelta` (the increment vocabulary the wave journal hangs off).
//!
//! The wave's mind runs on a persistent harness session
//! ([`crate::lfd::conversations::harness`]); its `ConversationEvent` stream is
//! adapted into `TurnDelta`s (see [`crate::lfd::wave::mind::EventAdapter`])
//! and folded by the wave runtime's `TurnSink` into journaled, broadcast
//! turns.
//!
//! Mapping:
//! - a human message becomes one `user` turn;
//! - each agent turn (text + tool activity, closed by the vendor's turn
//!   completion) becomes one `assistant` turn whose `items` capture the
//!   commands/edits/messages it ran.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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

/// Incremental outcome of one step of turn assembly.
///
/// This is the seam the wave's journal hangs off: every increment surfaces to
/// the runtime's `TurnSink` so it can be recorded (as
/// `TurnStarted`/`TurnItem`/`TurnFinished` events) the moment it happens, not
/// only at finalization.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnDelta {
    /// A new assistant turn opened.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_turn_round_trips_through_json() {
        let turn = ChatTurn::user("turn-0".into(), "please fix the build".into());
        let value = serde_json::to_value(&turn).expect("serialize");
        let decoded: ChatTurn = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, turn);
        assert_eq!(decoded.role, ChatRole::User);
    }
}
