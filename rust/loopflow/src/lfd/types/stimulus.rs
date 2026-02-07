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
}

impl StimulusKind {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::Once,
            2 => Self::Loop,
            3 => Self::Watch,
            4 => Self::Cron,
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
    pub kind: StimulusKind,
    pub cron: String,
    pub last_main_sha: Option<String>,
    pub last_triggered_at: Option<i64>,
    pub enabled: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
}

impl Stimulus {
    #[allow(dead_code)] // Convenience constructor for tests and future use.
    pub fn new(id: LfdId, wave_id: LfdId, kind: StimulusKind) -> Self {
        Self {
            id,
            wave_id,
            kind,
            cron: String::new(),
            last_main_sha: None,
            last_triggered_at: None,
            enabled: true,
            created_at: Some(OffsetDateTime::now_utc()),
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
