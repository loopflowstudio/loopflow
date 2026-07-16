//! One interactive flow exercise, conducted by a human or a parent agent.

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child_session::prefixed_uuid_id;
use crate::engine::InteractionPolicy;
use crate::id::WaveId;
use crate::project_session::ProjectSessionId;
use crate::task::{TaskLifecyclePhase, TaskSessionId};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InteractionReviewDataError {
    #[error("invalid interaction-review id: {0}")]
    InvalidId(String),
    #[error("invalid interaction-review status: {0}")]
    InvalidStatus(String),
    #[error("invalid interaction-review disposition: {0}")]
    InvalidDisposition(String),
    #[error("invalid interaction review: {0}")]
    InvalidInvariant(String),
}

prefixed_uuid_id!(
    InteractionReviewId,
    "ir_",
    InteractionReviewDataError,
    InteractionReviewDataError::InvalidId
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
#[non_exhaustive]
pub enum InteractionReviewer {
    Human,
    Project(ProjectSessionId),
    Wave(WaveId),
}

impl InteractionReviewer {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Project(_) => "project",
            Self::Wave(_) => "wave",
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Human => None,
            Self::Project(id) => Some(id.as_str()),
            Self::Wave(id) => Some(id.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InteractionReviewStatus {
    Requested,
    Active,
    Completed,
}

impl InteractionReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Active => "active",
            Self::Completed => "completed",
        }
    }

    pub fn is_terminal(self) -> bool {
        self == Self::Completed
    }
}

impl FromStr for InteractionReviewStatus {
    type Err = InteractionReviewDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "requested" => Ok(Self::Requested),
            "active" => Ok(Self::Active),
            "completed" => Ok(Self::Completed),
            _ => Err(InteractionReviewDataError::InvalidStatus(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InteractionReviewDisposition {
    Approved,
    ChangesRequested,
}

impl InteractionReviewDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
        }
    }
}

impl FromStr for InteractionReviewDisposition {
    type Err = InteractionReviewDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "approved" => Ok(Self::Approved),
            "changes_requested" => Ok(Self::ChangesRequested),
            _ => Err(InteractionReviewDataError::InvalidDisposition(
                value.to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionReviewMessageAuthor {
    Task,
    Reviewer,
}

impl InteractionReviewMessageAuthor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Reviewer => "reviewer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionReviewPr {
    pub number: u32,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionReviewEvidence {
    pub worktree: PathBuf,
    pub branch: String,
    pub base_commit: String,
    pub head_commit: String,
    pub worktree_fingerprint: String,
    pub pr: Option<InteractionReviewPr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionReview {
    pub id: InteractionReviewId,
    pub wave_id: WaveId,
    pub project_session_id: ProjectSessionId,
    pub task_session_id: TaskSessionId,
    pub phase: TaskLifecyclePhase,
    pub phase_epoch: u32,
    pub flow: String,
    pub step: String,
    pub step_index: u32,
    pub phase_iteration: u32,
    pub policy: InteractionPolicy,
    pub reviewer: InteractionReviewer,
    pub status: InteractionReviewStatus,
    pub reason: String,
    pub prompt: String,
    pub evidence: InteractionReviewEvidence,
    pub requested_by_generation: u32,
    pub reviewer_generation: Option<u32>,
    pub disposition: Option<InteractionReviewDisposition>,
    pub outcome: Option<String>,
    pub requested_at: OffsetDateTime,
    pub completed_at: Option<OffsetDateTime>,
}

impl InteractionReview {
    pub fn validate(&self) -> Result<(), InteractionReviewDataError> {
        for (name, value) in [
            ("flow", self.flow.as_str()),
            ("step", self.step.as_str()),
            ("reason", self.reason.as_str()),
            ("prompt", self.prompt.as_str()),
            ("branch", self.evidence.branch.as_str()),
            ("base commit", self.evidence.base_commit.as_str()),
            ("head commit", self.evidence.head_commit.as_str()),
            (
                "worktree fingerprint",
                self.evidence.worktree_fingerprint.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(InteractionReviewDataError::InvalidInvariant(format!(
                    "{name} cannot be empty"
                )));
            }
        }
        if !self.evidence.worktree.is_absolute() {
            return Err(InteractionReviewDataError::InvalidInvariant(
                "worktree must be absolute".to_string(),
            ));
        }
        if self.phase_epoch == 0 || self.requested_by_generation == 0 {
            return Err(InteractionReviewDataError::InvalidInvariant(
                "phase epoch and requesting generation must be positive".to_string(),
            ));
        }
        if let Some(pr) = &self.evidence.pr {
            if pr.number == 0 || pr.url.trim().is_empty() {
                return Err(InteractionReviewDataError::InvalidInvariant(
                    "PR evidence requires a positive number and URL".to_string(),
                ));
            }
        }
        match (self.policy, &self.reviewer) {
            (InteractionPolicy::Require, InteractionReviewer::Human)
            | (InteractionPolicy::Defer, InteractionReviewer::Project(_))
            | (InteractionPolicy::Defer, InteractionReviewer::Wave(_)) => {}
            _ => {
                return Err(InteractionReviewDataError::InvalidInvariant(
                    "reviewer does not match the interaction policy".to_string(),
                ))
            }
        }
        match &self.reviewer {
            InteractionReviewer::Human => {}
            InteractionReviewer::Project(id) if id == &self.project_session_id => {}
            InteractionReviewer::Wave(id) if id == &self.wave_id => {}
            InteractionReviewer::Project(_) | InteractionReviewer::Wave(_) => {
                return Err(InteractionReviewDataError::InvalidInvariant(
                    "reviewer is not the owning parent".to_string(),
                ))
            }
        }
        match self.status {
            InteractionReviewStatus::Requested | InteractionReviewStatus::Active => {
                if self.disposition.is_some()
                    || self.outcome.is_some()
                    || self.completed_at.is_some()
                {
                    return Err(InteractionReviewDataError::InvalidInvariant(
                        "open reviews cannot carry terminal evidence".to_string(),
                    ));
                }
            }
            InteractionReviewStatus::Completed => {
                if self.disposition.is_none()
                    || self
                        .outcome
                        .as_ref()
                        .is_none_or(|outcome| outcome.trim().is_empty())
                    || self.completed_at.is_none()
                {
                    return Err(InteractionReviewDataError::InvalidInvariant(
                        "completed reviews require disposition, outcome, and completion time"
                            .to_string(),
                    ));
                }
            }
        }
        if matches!(
            self.reviewer,
            InteractionReviewer::Project(_) | InteractionReviewer::Wave(_)
        ) && self.status == InteractionReviewStatus::Completed
            && self.reviewer_generation.is_none()
        {
            return Err(InteractionReviewDataError::InvalidInvariant(
                "completed agent reviews require the reviewer generation".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review() -> InteractionReview {
        let project_session_id = ProjectSessionId::new();
        InteractionReview {
            id: InteractionReviewId::new(),
            wave_id: WaveId::new(),
            project_session_id: project_session_id.clone(),
            task_session_id: TaskSessionId::new(),
            phase: TaskLifecyclePhase::Gate,
            phase_epoch: 3,
            flow: "task-gate".to_string(),
            step: "demo".to_string(),
            step_index: 0,
            phase_iteration: 0,
            policy: InteractionPolicy::Defer,
            reviewer: InteractionReviewer::Project(project_session_id),
            status: InteractionReviewStatus::Requested,
            reason: "show the delivered behavior".to_string(),
            prompt: "Conduct the demo exercise.".to_string(),
            evidence: InteractionReviewEvidence {
                worktree: PathBuf::from("/repo.task"),
                branch: "jack/task".to_string(),
                base_commit: "base".to_string(),
                head_commit: "head".to_string(),
                worktree_fingerprint: "fingerprint".to_string(),
                pr: None,
            },
            requested_by_generation: 3,
            reviewer_generation: None,
            disposition: None,
            outcome: None,
            requested_at: OffsetDateTime::now_utc(),
            completed_at: None,
        }
    }

    #[test]
    fn headless_review_requires_a_parent_and_completed_evidence() {
        let mut value = review();
        assert!(value.validate().is_ok());
        value.reviewer = InteractionReviewer::Human;
        assert!(value.validate().is_err());
        value.reviewer = InteractionReviewer::Project(value.project_session_id.clone());
        value.status = InteractionReviewStatus::Completed;
        assert!(value.validate().is_err());
    }
}
