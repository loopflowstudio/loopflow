use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AttentionKind {
    InteractiveStep,
    Algedonic,
}

impl AttentionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InteractiveStep => "interactive_step",
            Self::Algedonic => "algedonic",
        }
    }
}

impl std::str::FromStr for AttentionKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "interactive_step" => Ok(Self::InteractiveStep),
            "algedonic" => Ok(Self::Algedonic),
            _ => Err(format!("unknown attention kind: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AttentionStatus {
    Surfaced,
    Viewed,
    Resolved,
}

impl AttentionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Surfaced => "surfaced",
            Self::Viewed => "viewed",
            Self::Resolved => "resolved",
        }
    }
}

impl std::str::FromStr for AttentionStatus {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "surfaced" => Ok(Self::Surfaced),
            "viewed" => Ok(Self::Viewed),
            "resolved" => Ok(Self::Resolved),
            _ => Err(format!("unknown attention status: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionItem {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub run_id: Option<LfdId>,
    pub kind: AttentionKind,
    pub status: AttentionStatus,
    pub title: String,
    pub summary: String,
    pub context: Value,
    #[serde(with = "time::serde::rfc3339")]
    pub surfaced_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub viewed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub resolved_at: Option<OffsetDateTime>,
}
