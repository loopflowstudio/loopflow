//! Event types for WebSocket streaming.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;
use crate::lfd::types::agent::AgentStatus;

/// Event payload variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    // Connection
    Connected {
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    Ping,

    // Wave lifecycle
    WaveCreated {
        wave_id: LfdId,
        name: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveUpdated {
        wave_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveDeleted {
        wave_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveStarted {
        wave_id: LfdId,
        wave_run_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveStopped {
        wave_id: LfdId,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    WaveWaiting {
        wave_id: LfdId,
        wave_run_id: LfdId,
        step: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },

    // Worktree
    WorktreeUpdated {
        worktree: String,
        repo: String,
        branch: Option<String>,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },

    // Agent
    AgentStarted {
        agent_id: LfdId,
        step: String,
        worktree: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
    AgentEnded {
        agent_id: LfdId,
        status: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },

    // Output
    OutputLine {
        wave_id: LfdId,
        agent_id: LfdId,
        text: String,
        #[serde(with = "time::serde::rfc3339")]
        timestamp: OffsetDateTime,
    },
}

impl Event {
    pub fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    pub fn wave_created(wave_id: LfdId, name: String) -> Self {
        Self::WaveCreated {
            wave_id,
            name,
            timestamp: Self::now(),
        }
    }

    pub fn wave_updated(wave_id: LfdId) -> Self {
        Self::WaveUpdated {
            wave_id,
            timestamp: Self::now(),
        }
    }

    pub fn wave_deleted(wave_id: LfdId) -> Self {
        Self::WaveDeleted {
            wave_id,
            timestamp: Self::now(),
        }
    }

    pub fn wave_started(wave_id: LfdId, wave_run_id: LfdId) -> Self {
        Self::WaveStarted {
            wave_id,
            wave_run_id,
            timestamp: Self::now(),
        }
    }

    pub fn wave_stopped(wave_id: LfdId) -> Self {
        Self::WaveStopped {
            wave_id,
            timestamp: Self::now(),
        }
    }

    pub fn worktree_updated(worktree: String, repo: String, branch: Option<String>) -> Self {
        Self::WorktreeUpdated {
            worktree,
            repo,
            branch,
            timestamp: Self::now(),
        }
    }

    pub fn wave_waiting(wave_id: LfdId, wave_run_id: LfdId, step: String) -> Self {
        Self::WaveWaiting {
            wave_id,
            wave_run_id,
            step,
            timestamp: Self::now(),
        }
    }

    pub fn agent_started(agent_id: LfdId, step: String, worktree: String) -> Self {
        Self::AgentStarted {
            agent_id,
            step,
            worktree,
            timestamp: Self::now(),
        }
    }

    pub fn agent_ended(agent_id: LfdId, status: AgentStatus) -> Self {
        Self::AgentEnded {
            agent_id,
            status: status.as_str().to_string(),
            timestamp: Self::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_id(s: &str) -> LfdId {
        LfdId::from_raw(s)
    }

    #[test]
    fn wave_waiting_serializes_correctly() {
        let event =
            Event::wave_waiting(test_id("wave-1"), test_id("run-1"), "implement".to_string());
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "wave_waiting");
        assert_eq!(json["wave_id"], "wave-1");
        assert_eq!(json["wave_run_id"], "run-1");
        assert_eq!(json["step"], "implement");
        assert!(json["timestamp"].is_string());
    }

    #[test]
    fn agent_started_serializes_correctly() {
        let event = Event::agent_started(
            test_id("agent-1"),
            "review".to_string(),
            "/tmp/wt".to_string(),
        );
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "agent_started");
        assert_eq!(json["agent_id"], "agent-1");
        assert_eq!(json["step"], "review");
        assert_eq!(json["worktree"], "/tmp/wt");
    }

    #[test]
    fn agent_ended_serializes_correctly() {
        let event = Event::agent_ended(test_id("agent-1"), "completed".to_string());
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "agent_ended");
        assert_eq!(json["agent_id"], "agent-1");
        assert_eq!(json["status"], "completed");
    }

    #[test]
    fn event_roundtrips_through_json() {
        let id = || LfdId::new();
        let events = vec![
            Event::wave_waiting(id(), id(), "step".to_string()),
            Event::agent_started(id(), "s".to_string(), "/wt".to_string()),
            Event::agent_ended(id(), "failed".to_string()),
        ];
        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: Event = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, json2);
        }
    }
}
