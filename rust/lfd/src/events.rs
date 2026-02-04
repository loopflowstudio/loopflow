use tokio::sync::broadcast;

use crate::proto::control::Event;

/// EventHub broadcasts events to all subscribers.
/// Events are fire-and-forget - if no one is listening, they're dropped.
#[derive(Clone)]
pub struct EventHub {
    sender: broadcast::Sender<Event>,
}

impl EventHub {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        Self { sender }
    }

    /// Send an event to all subscribers.
    /// Currently unused - will be called from executor/store when emitting events.
    #[allow(dead_code)]
    pub fn send(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

/// Check if an event name matches any of the given patterns.
/// Patterns can be exact matches or glob patterns ending with ".*"
pub fn matches_patterns(event_name: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return true;
    }
    patterns.iter().any(|pattern| {
        if pattern.ends_with(".*") {
            let prefix = &pattern[..pattern.len() - 2];
            event_name.starts_with(prefix)
        } else {
            event_name == pattern
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_patterns_empty() {
        assert!(matches_patterns("wave.created", &[]));
    }

    #[test]
    fn test_matches_patterns_exact() {
        let patterns = vec!["wave.created".to_string()];
        assert!(matches_patterns("wave.created", &patterns));
        assert!(!matches_patterns("wave.updated", &patterns));
    }

    #[test]
    fn test_matches_patterns_glob() {
        let patterns = vec!["wave.*".to_string()];
        assert!(matches_patterns("wave.created", &patterns));
        assert!(matches_patterns("wave.updated", &patterns));
        assert!(!matches_patterns("agent.started", &patterns));
    }

    #[test]
    fn test_matches_patterns_multiple() {
        let patterns = vec!["wave.*".to_string(), "agent.started".to_string()];
        assert!(matches_patterns("wave.created", &patterns));
        assert!(matches_patterns("agent.started", &patterns));
        assert!(!matches_patterns("agent.ended", &patterns));
    }
}
