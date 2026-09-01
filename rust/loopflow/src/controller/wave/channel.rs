//! The platform-agnostic chat channel — a Discord-shaped read/write model.
//!
//! One channel per wave (the journal is it). `read` returns the recent
//! conversation **including the bot's own posts**, exactly like reading a Discord
//! channel — so an observer can tell it already replied without any consumption
//! tracking. Discord is a bridge that writes into this same model; `lf chat` is
//! its native client. Nothing here exists that a Discord channel does not.

use serde::{Deserialize, Serialize};

use crate::chat::turns::{ChatRole, ChatTurn};

/// Who spoke a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Author {
    /// A person. `name` is the platform handle where one exists, else empty.
    Human { name: String },
    /// The wave itself — its own posted replies. Reading these back is how the
    /// wave knows it has already answered.
    Bot,
    /// A message mirrored in from an external platform (e.g. Discord).
    Bridge { platform: String, user: String },
}

/// One message in a wave's chat channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub author: Author,
    pub content: String,
    /// The message this replies to, when the platform threads replies.
    pub reply_to: Option<String>,
    /// RFC 3339 timestamp of when the message was posted.
    pub at: String,
}

impl Message {
    /// True when the wave posted this message itself.
    pub fn is_own(&self) -> bool {
        matches!(self.author, Author::Bot)
    }

    /// Project one folded thread turn into a channel message. Returns the turn's
    /// journal sequence alongside it so a reader can advance its cursor.
    ///
    /// A `User` turn is a human/bridged message; an `Assistant` turn is the
    /// wave's own post. The runtime overlays bridged-author identity from the
    /// source record; a bare local read reports `Human`.
    pub(crate) fn from_turn(turn: ChatTurn) -> Option<(u64, Message)> {
        let seq = turn.id.strip_prefix("turn-")?.parse::<u64>().ok()?;
        let author = match turn.role {
            ChatRole::User => Author::Human {
                name: String::new(),
            },
            ChatRole::Assistant => Author::Bot,
        };
        Some((
            seq,
            Message {
                id: turn.id,
                author,
                content: turn.text,
                reply_to: None,
                at: turn.created_at,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_and_author_round_trip_on_the_wire() {
        for author in [
            Author::Human {
                name: "jack".to_string(),
            },
            Author::Bot,
            Author::Bridge {
                platform: "discord".to_string(),
                user: "42".to_string(),
            },
        ] {
            let message = Message {
                id: "turn-7".to_string(),
                author: author.clone(),
                content: "hello".to_string(),
                reply_to: Some("turn-6".to_string()),
                at: "2026-08-28T00:00:00Z".to_string(),
            };
            let encoded = serde_json::to_string(&message).expect("encode");
            let decoded: Message = serde_json::from_str(&encoded).expect("decode");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn assistant_turns_read_as_the_bots_own_posts() {
        use crate::chat::turns::{ChatRole, ChatTurn};
        use crate::chat::types::Lifecycle;
        let assistant = ChatTurn {
            id: "turn-3".to_string(),
            role: ChatRole::Assistant,
            text: "answered".to_string(),
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at: "2026-08-28T00:00:00Z".to_string(),
            body: None,
            activity: None,
        };
        let (seq, message) = Message::from_turn(assistant).expect("mapped");
        assert_eq!(seq, 3);
        assert!(message.is_own());
    }
}
