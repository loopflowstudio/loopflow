//! Durable ownership and progress for one watched pull-request landing.

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child::prefixed_uuid_id;
use crate::durable::HomeId;
use crate::work::task::{AfterMerge, TaskId};

pub(crate) const SUPERVISOR_STALE_AFTER: time::Duration = time::Duration::minutes(2);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrLandingDataError {
    #[error("invalid PR landing id: {0}")]
    InvalidId(String),
    #[error("invalid PR landing: {0}")]
    InvalidInvariant(String),
}

prefixed_uuid_id!(
    PrLandingId,
    "landing_",
    PrLandingDataError,
    PrLandingDataError::InvalidId
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrLandingState {
    Watching,
    Repairing,
    Merged,
    Closed,
    Blocked,
}

impl PrLandingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Watching => "watching",
            Self::Repairing => "repairing",
            Self::Merged => "merged",
            Self::Closed => "closed",
            Self::Blocked => "blocked",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Merged | Self::Closed | Self::Blocked)
    }
}

impl FromStr for PrLandingState {
    type Err = PrLandingDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "watching" => Ok(Self::Watching),
            "repairing" => Ok(Self::Repairing),
            "merged" => Ok(Self::Merged),
            "closed" => Ok(Self::Closed),
            "blocked" => Ok(Self::Blocked),
            _ => Err(PrLandingDataError::InvalidInvariant(format!(
                "invalid stored landing state: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "placement", rename_all = "snake_case")]
pub enum LandingPlacement {
    Local,
    Home { home_id: HomeId },
}

impl LandingPlacement {
    pub(crate) fn storage_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Home { .. } => "home",
        }
    }

    pub(crate) fn home_id(&self) -> Option<&HomeId> {
        match self {
            Self::Local => None,
            Self::Home { home_id } => Some(home_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LandingSupervisor {
    pub placement: LandingPlacement,
    pub process_id: u32,
    pub generation: u64,
    pub heartbeat_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandingClaim {
    pub placement: LandingPlacement,
    pub process_id: u32,
    pub heartbeat_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrLanding {
    pub id: PrLandingId,
    pub repo: String,
    pub pr_number: u32,
    pub worktree: PathBuf,
    pub branch: String,
    pub task_id: Option<TaskId>,
    pub requested_head_sha: String,
    pub observed_head_sha: String,
    pub merge_commit: Option<String>,
    pub after_merge: Option<AfterMerge>,
    pub next_slug: Option<String>,
    pub state: PrLandingState,
    pub generation: u64,
    pub supervisor: Option<LandingSupervisor>,
    pub repair_count: u32,
    pub blocked_reason: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPrLanding {
    pub repo: String,
    pub pr_number: u32,
    pub worktree: PathBuf,
    pub branch: String,
    pub task_id: Option<TaskId>,
    pub requested_head_sha: String,
    pub after_merge: Option<AfterMerge>,
    pub next_slug: Option<String>,
}

impl PrLanding {
    pub fn new(input: NewPrLanding, now: OffsetDateTime) -> Result<Self, PrLandingDataError> {
        let landing = Self {
            id: PrLandingId::new(),
            repo: input.repo,
            pr_number: input.pr_number,
            worktree: input.worktree,
            branch: input.branch,
            task_id: input.task_id,
            observed_head_sha: input.requested_head_sha.clone(),
            requested_head_sha: input.requested_head_sha,
            merge_commit: None,
            after_merge: input.after_merge,
            next_slug: input.next_slug,
            state: PrLandingState::Watching,
            generation: 1,
            supervisor: None,
            repair_count: 0,
            blocked_reason: None,
            created_at: now,
            updated_at: now,
        };
        landing.validate()?;
        Ok(landing)
    }

    pub fn validate(&self) -> Result<(), PrLandingDataError> {
        if self.repo.trim().is_empty()
            || self.pr_number == 0
            || self.branch.trim().is_empty()
            || self.requested_head_sha.trim().is_empty()
            || self.observed_head_sha.trim().is_empty()
            || self.generation == 0
        {
            return Err(PrLandingDataError::InvalidInvariant(
                "landing requires repository, PR, branch, heads, and generation".to_string(),
            ));
        }
        if self.after_merge == Some(AfterMerge::CompleteTask) && self.next_slug.is_some() {
            return Err(PrLandingDataError::InvalidInvariant(
                "a completing landing cannot name a next branch".to_string(),
            ));
        }
        if self.task_id.is_none() && (self.after_merge.is_some() || self.next_slug.is_some()) {
            return Err(PrLandingDataError::InvalidInvariant(
                "direct landing cannot carry Task disposition".to_string(),
            ));
        }
        if self.state == PrLandingState::Merged && self.merge_commit.is_none() {
            return Err(PrLandingDataError::InvalidInvariant(
                "merged landing requires a merge commit".to_string(),
            ));
        }
        if self.state == PrLandingState::Blocked && self.blocked_reason.is_none() {
            return Err(PrLandingDataError::InvalidInvariant(
                "blocked landing requires a reason".to_string(),
            ));
        }
        if self.state != PrLandingState::Blocked && self.blocked_reason.is_some() {
            return Err(PrLandingDataError::InvalidInvariant(
                "only a blocked landing carries a reason".to_string(),
            ));
        }
        if let Some(supervisor) = &self.supervisor {
            if supervisor.generation != self.generation || supervisor.process_id == 0 {
                return Err(PrLandingDataError::InvalidInvariant(
                    "landing supervisor must name its current generation and process".to_string(),
                ));
            }
        }
        Ok(())
    }
}
