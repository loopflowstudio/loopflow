//! Trigger and PendingActivation types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

/// Flow name used for CI failure remediation runs.
pub const CI_FIX_FLOW: &str = "ci-fix";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Signal {
    Repo = 1,
    Wave = 2,
    CiFailure = 3,
    Block = 4,
}

impl Signal {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Repo),
            2 => Some(Self::Wave),
            3 => Some(Self::CiFailure),
            4 => Some(Self::Block),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Repo => "repo",
            Self::Wave => "wave",
            Self::CiFailure => "ci_failure",
            Self::Block => "block",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivationOutcome {
    Queued,
    Coalesced,
    Dropped,
    Dispatched,
}

impl ActivationOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Coalesced => "coalesced",
            Self::Dropped => "dropped",
            Self::Dispatched => "dispatched",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "coalesced" => Some(Self::Coalesced),
            "dropped" => Some(Self::Dropped),
            "dispatched" => Some(Self::Dispatched),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub source_wave_id: Option<LfdId>,
    pub signal: Signal,
    pub flow: Option<String>,
    pub last_main_sha: Option<String>,
    pub last_triggered_at: Option<i64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Maximum iterations per cycle. When a wave's iteration count reaches this
    /// value, the loop ticker pauses the wave instead of re-triggering.
    #[serde(default)]
    pub max_iterations: Option<u32>,
}

fn default_enabled() -> bool {
    true
}

impl Trigger {
    pub fn new(id: LfdId, wave_id: LfdId, signal: Signal) -> Self {
        Self {
            id,
            wave_id,
            source_wave_id: None,
            signal,
            flow: None,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
            max_iterations: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingActivation {
    pub id: LfdId,
    pub wave_id: LfdId,
    /// Which trigger fired. `None` for manual activations.
    pub trigger_id: Option<LfdId>,
    pub reason: String,
    pub from_sha: String,
    pub to_sha: String,
    pub queued_at: i64,
    #[serde(default = "default_target_branch")]
    pub target_branch: String,
}

fn default_target_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationLog {
    pub id: LfdId,
    pub wave_id: LfdId,
    /// Which trigger fired. `None` for manual activations.
    pub trigger_id: Option<LfdId>,
    pub reason: String,
    pub outcome: ActivationOutcome,
    pub created_at: i64,
}

impl ActivationLog {
    pub fn new(
        wave_id: LfdId,
        trigger_id: Option<LfdId>,
        reason: String,
        outcome: ActivationOutcome,
    ) -> Self {
        Self {
            id: LfdId::new(),
            wave_id,
            trigger_id,
            reason,
            outcome,
            created_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Signal;

    #[test]
    fn signal_round_trips() {
        assert_eq!(Signal::from_i32(1), Some(Signal::Repo));
        assert_eq!(Signal::from_i32(2), Some(Signal::Wave));
        assert_eq!(Signal::from_i32(3), Some(Signal::CiFailure));
        assert_eq!(Signal::from_i32(4), Some(Signal::Block));
        assert_eq!(Signal::from_i32(0), None);
        assert_eq!(Signal::from_i32(99), None);
    }
}
