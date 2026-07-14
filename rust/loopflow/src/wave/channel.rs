//! Channel names: the dot tree everyone on the bus addresses each other by.
//!
//! A channel is a name, and nothing else. The bus itself is a table in the
//! shared store (`crate::wave::bus`, `store/migrations/059_bus.sql`): publishing
//! is an INSERT, subscribing is a forward poll from an id cursor, and nothing
//! brokers between them. A served wave journals its own thread because a mind
//! must survive restarts; a hand's name (`goals.148e0e02`) records nothing.
//! Dots form the subscription tree; they do not imply ownership or a worktree
//! path.

// Family MEMBERSHIP lives in one place: `runtime::channel_role` (it compares
// against the sanitized wave name — the form channel names actually carry).
// This module keeps only the name mechanics.

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

#[cfg(test)]
mod tests {
    use super::{family_head, matches_prefix};

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
}
