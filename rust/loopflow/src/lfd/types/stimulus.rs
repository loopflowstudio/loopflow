//! Stimulus and PendingActivation types.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StimulusKind {
    #[default]
    Unspecified = 0,
    Once = 1,
    Loop = 2,
    Watch = 3,
    Cron = 4,
    Listen = 5,
}

impl StimulusKind {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Once,
            2 => Self::Loop,
            3 => Self::Watch,
            4 => Self::Cron,
            5 => Self::Listen,
            _ => Self::Unspecified,
        }
    }

    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stimulus {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub source_wave_id: Option<LfdId>,
    pub kind: StimulusKind,
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
    pub fn new(id: LfdId, wave_id: LfdId, kind: StimulusKind) -> Self {
        Self {
            id,
            wave_id,
            source_wave_id: None,
            kind,
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
            from_sha: String::new(),
            to_sha: String::new(),
            queued_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StimulusKind;

    #[test]
    fn listen_stimulus_kind_storage_value_is_stable() {
        assert_eq!(StimulusKind::Listen.as_i32(), 5);
        assert_eq!(StimulusKind::from_i32(5), StimulusKind::Listen);
    }
}
