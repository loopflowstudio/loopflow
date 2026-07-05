//! Turn vocabulary: `ChatTurn`, the wire type Concerto consumes.
//!
//! The wave's mind runs on a persistent harness session
//! ([`crate::harness`]) inside the RESIDENT process; its
//! `ConversationEvent` stream is adapted into resident wire deltas (see
//! [`crate::wave::mind::EventAdapter`] and [`crate::wave::wire`]) and folded
//! by the listener's runtime into journaled, broadcast turns.
//!
//! Mapping:
//! - a human message becomes one `user` turn;
//! - each agent turn (text + tool activity, closed by the vendor's turn
//!   completion) becomes one `assistant` turn whose `items` capture the
//!   commands/edits/messages it ran.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::conversation::types::{ConversationItem, Lifecycle};

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
    /// Speaker label for attributed emissions (`lf chat` — worker reports,
    /// child-wave escalations). Absent for the mind's own turns and plain
    /// user turns.
    pub from: Option<String>,
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
            from: None,
        }
    }

    /// The one turn-growth rule every projection shares: `Message` prose
    /// joins into `text` (newline-separated), every other item appends to
    /// `items`. The listener's open-turn snapshot, the journal fold
    /// (`fold_thread`), and the resident's adapter (`EventAdapter`) all grow
    /// turns through this — a second copy is the live-vs-replay split-brain
    /// the journal exists to kill.
    pub fn absorb_item(&mut self, item: ConversationItem) {
        if let ConversationItem::Message { text, .. } = &item {
            self.push_text(text);
        } else {
            self.items.push(item);
        }
    }

    /// Join a prose fragment into the turn text, newline-separated.
    pub fn push_text(&mut self, fragment: &str) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(fragment);
    }
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

    #[test]
    fn attributed_turn_round_trips_and_absent_from_decodes_none() {
        let mut turn = ChatTurn::user("turn-1".into(), "worker report".into());
        turn.from = Some("worker".to_string());
        let value = serde_json::to_value(&turn).expect("serialize");
        assert_eq!(value["from"], "worker");
        let decoded: ChatTurn = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded.from.as_deref(), Some("worker"));

        // Absent `from` is None — no default masking on the wire.
        let mut value =
            serde_json::to_value(ChatTurn::user("turn-2".into(), "hi".into())).expect("serialize");
        value.as_object_mut().expect("object").remove("from");
        let decoded: ChatTurn = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded.from, None);
    }

    #[test]
    fn absorb_item_joins_prose_and_appends_the_rest() {
        let mut turn = ChatTurn::user("turn-3".into(), String::new());
        turn.absorb_item(ConversationItem::Message {
            id: "m-1".into(),
            text: "first".into(),
            phase: None,
        });
        turn.absorb_item(ConversationItem::Tool {
            id: "t-1".into(),
            name: "Bash".into(),
            status: Lifecycle::Completed,
            input: None,
            output: None,
        });
        turn.absorb_item(ConversationItem::Message {
            id: "m-2".into(),
            text: "second".into(),
            phase: None,
        });

        assert_eq!(turn.text, "first\nsecond");
        assert_eq!(turn.items.len(), 1, "prose joins text, tools append");
    }
}
