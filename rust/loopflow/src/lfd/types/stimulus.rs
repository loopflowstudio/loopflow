//! Stimulus and PendingActivation types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    #[default]
    Unspecified = 0,
    Once = 1,
    Loop = 2,
    Watch = 3,
    Cron = 4,
    Listen = 5,
    CiFailure = 6,
}

impl Signal {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Once,
            2 => Self::Loop,
            3 => Self::Watch,
            4 => Self::Cron,
            5 => Self::Listen,
            6 => Self::CiFailure,
            _ => Self::Unspecified,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActivationSource {
    #[default]
    Poll = 0,
    Push = 1,
    Listen = 2,
    Manual = 3,
}

impl ActivationSource {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Push,
            2 => Self::Listen,
            3 => Self::Manual,
            _ => Self::Poll,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Poll => "poll",
            Self::Push => "push",
            Self::Listen => "listen",
            Self::Manual => "manual",
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
pub struct Stimulus {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub source_wave_id: Option<LfdId>,
    pub signal: Signal,
    pub flow: Option<String>,
    pub cron: Option<String>,
    pub last_main_sha: Option<String>,
    pub last_triggered_at: Option<i64>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl Stimulus {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, wave_id: LfdId, signal: Signal) -> Self {
        Self {
            id,
            wave_id,
            source_wave_id: None,
            signal,
            flow: None,
            cron: None,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingActivation {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub stimulus_id: LfdId,
    pub source: ActivationSource,
    pub reason: String,
    pub from_sha: String,
    pub to_sha: String,
    pub queued_at: i64,
}

impl PendingActivation {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, wave_id: LfdId, stimulus_id: LfdId) -> Self {
        Self {
            id,
            wave_id,
            stimulus_id,
            source: ActivationSource::Poll,
            reason: String::new(),
            from_sha: String::new(),
            to_sha: String::new(),
            queued_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationLog {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub stimulus_id: LfdId,
    pub source: ActivationSource,
    pub reason: String,
    pub outcome: ActivationOutcome,
    pub created_at: i64,
}

impl ActivationLog {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(
        wave_id: LfdId,
        stimulus_id: LfdId,
        source: ActivationSource,
        reason: String,
        outcome: ActivationOutcome,
    ) -> Self {
        Self {
            id: LfdId::new(),
            wave_id,
            stimulus_id,
            source,
            reason,
            outcome,
            created_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ActivationSource, Signal};

    #[test]
    fn listen_signal_storage_value_is_stable() {
        assert_eq!(Signal::Listen.as_i32(), 5);
        assert_eq!(Signal::from_i32(5), Signal::Listen);
    }

    #[test]
    fn ci_failure_signal_storage_value_is_stable() {
        assert_eq!(Signal::CiFailure.as_i32(), 6);
        assert_eq!(Signal::from_i32(6), Signal::CiFailure);
    }

    #[test]
    fn activation_source_storage_value_is_stable() {
        assert_eq!(ActivationSource::Poll.as_i32(), 0);
        assert_eq!(ActivationSource::Push.as_i32(), 1);
        assert_eq!(ActivationSource::Listen.as_i32(), 2);
        assert_eq!(ActivationSource::Manual.as_i32(), 3);
        assert_eq!(ActivationSource::from_i32(3), ActivationSource::Manual);
    }
}
