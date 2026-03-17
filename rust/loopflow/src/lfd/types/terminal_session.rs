use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TerminalSessionStatus {
    Pending,
    Attached,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

impl TerminalSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Attached => "attached",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    pub(crate) fn as_i32(self) -> i32 {
        match self {
            Self::Pending => 0,
            Self::Attached => 1,
            Self::Running => 2,
            Self::Succeeded => 3,
            Self::Failed => 4,
            Self::Canceled => 5,
        }
    }

    pub(crate) fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Pending,
            1 => Self::Attached,
            2 => Self::Running,
            3 => Self::Succeeded,
            4 => Self::Failed,
            5 => Self::Canceled,
            _ => Self::Failed,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: LfdId,
    pub wave_id: LfdId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_run_id: Option<LfdId>,
    pub step: String,
    pub agent: String,
    pub cwd: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    pub source: String,
    pub status: TerminalSessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    #[serde(skip_serializing)]
    pub completion_token: Option<String>,
}
