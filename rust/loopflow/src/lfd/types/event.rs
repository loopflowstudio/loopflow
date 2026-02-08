//! Event types for WebSocket streaming.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

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
}
