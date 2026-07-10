//! Channels are ephemeral names on the live agent bus.
//!
//! A served wave journals its own thread because a mind must survive restarts.
//! Child names (`goals.148e0e02`) are topics only: publishing broadcasts one
//! tagged frame to listeners that are connected now and writes no channel
//! record. Dots form the subscription tree; they do not imply ownership or a
//! worktree path.

use crate::chat::turns::ChatTurn;

// Family MEMBERSHIP lives in one place: `runtime::channel_role` (it compares
// against the sanitized wave name — the form channel names actually carry).
// This module keeps only the name and wire mechanics.

/// Whether `channel` matches a subtree `prefix`: the prefix itself or any
/// dot-descendant of it.
pub fn matches_prefix(channel: &str, prefix: &str) -> bool {
    channel == prefix
        || channel
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

/// The family head a channel name belongs to — the wave, its first dot
/// segment (`goals.148e0e02` → `goals`; `goals` → `goals`).
pub fn family_head(channel: &str) -> &str {
    channel.split('.').next().unwrap_or(channel)
}

/// One live frame from a child channel, tagged with its name so a family
/// subscription can tell the streams apart (the primary channel's frames ride
/// untagged — absent means the wave's own channel). A topic has no past, so a
/// frame is only ever forwarded, never folded: the tagged wire JSON is the
/// whole frame, serialized once at the send site so N subscribers share one
/// serialization.
#[derive(Debug, Clone)]
pub struct ChannelFrame {
    pub channel: String,
    pub json: std::sync::Arc<str>,
}

/// A child channel's turn as `/events` wire JSON: the `Turn` object plus one
/// extra key, `"channel"`. Additive — the primary channel's frames stay
/// untagged, so a family of one is byte-identical to the pre-family wire.
/// Shared by the live send site (once per frame) and the replay path.
pub fn tagged_turn_json(channel: &str, turn: &ChatTurn) -> String {
    let mut value = serde_json::to_value(turn).unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(map) = &mut value {
        map.insert(
            "channel".to_string(),
            serde_json::Value::String(channel.to_string()),
        );
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_naming_rules() {
        assert_eq!(family_head("goals.148e0e02"), "goals");
        assert_eq!(family_head("goals"), "goals");
        assert!(matches_prefix("goals", "goals"));
        assert!(matches_prefix("goals.148e0e02", "goals"));
        assert!(matches_prefix("goals.a.b", "goals.a"));
        assert!(!matches_prefix("goals.ab", "goals.a"));
        assert!(!matches_prefix("goalsmith", "goals"));
        assert!(!matches_prefix("concerto", "goals"));
    }

    #[test]
    fn tagged_turns_carry_the_topic_without_changing_the_turn() {
        let turn = ChatTurn::user("turn-1".into(), "hello".into());
        let tagged: serde_json::Value =
            serde_json::from_str(&tagged_turn_json("goals.hand", &turn)).unwrap();
        assert_eq!(tagged["channel"], "goals.hand");
        assert_eq!(tagged["id"], "turn-1");
        assert_eq!(tagged["text"], "hello");
    }
}
