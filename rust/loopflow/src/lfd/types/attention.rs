use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

/// Two attention paths:
/// - `Interactive`: `lf` is at a step that needs a human. Created and resolved by `lf` via API.
/// - `Algedonic`: something is wrong and the system is escalating. Created by daemon policy
///   (repair chain exhaustion, queue blocks) or by `lf` when an agent decides to escalate.
///   Routes through the wave hierarchy — child wave escalates to parent, only root escalates
///   to human.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AttentionKind {
    DesignReview,
    CodeReview,
    Calibration,
    QueueFailure,
    StepFailure,
}

impl AttentionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DesignReview => "design_review",
            Self::CodeReview => "code_review",
            Self::Calibration => "calibration",
            Self::QueueFailure => "queue_failure",
            Self::StepFailure => "step_failure",
        }
    }
}

impl std::str::FromStr for AttentionKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "design_review" => Ok(Self::DesignReview),
            "code_review" => Ok(Self::CodeReview),
            "calibration" => Ok(Self::Calibration),
            "queue_failure" => Ok(Self::QueueFailure),
            "step_failure" => Ok(Self::StepFailure),
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
