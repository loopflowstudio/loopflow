//! Durable state for one Task.
//!
//! A Task owns one durable checkout, an optional PR, and Workflow progression.
//! Sessions, Graphs and Processes record work against that state.

pub mod comment;

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use crate::durable::TaskId;
use crate::id::prefixed_uuid_id;
use crate::id::WaveId;
use crate::linear::{LinearBindingError, LinearIssueId};
use crate::pr_landing::PrLandingId;
use crate::work::project::ProjectId;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskUpdate {
    pub rank: Option<u32>,
    /// Omitted preserves assignment; explicit None clears it.
    pub assignee: Option<Option<String>>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub regressed_in: Option<String>,
}

/// The caller retains this identity across a retry; a separate create mints another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTask {
    pub due_date: Option<String>,
    pub id: crate::durable::TaskId,
    pub repository_id: crate::durable::RepositoryId,
    pub project_id: Option<crate::durable::ProjectId>,
    pub title: String,
    pub description: String,
}

/// A release version as `regressed_in` stores it, `v` and three numbers:
/// `0.15.3` and `v0.15.3` both read as `v0.15.3`.
pub fn release_version(text: &str) -> Option<String> {
    let numbers = text.trim().strip_prefix('v').unwrap_or(text.trim());
    let parts: Vec<&str> = numbers.split('.').collect();
    (parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())))
    .then(|| format!("v{numbers}"))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskDataError {
    #[error("invalid Task id: {0}")]
    InvalidId(String),
    #[error("invalid Task: {0}")]
    InvalidInvariant(String),
}

prefixed_uuid_id!(TaskPrId, "pr_", TaskDataError, TaskDataError::InvalidId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubPr {
    pub number: u32,
    pub url: String,
    /// The PR's current head commit, from `gh pr list --json headRefOid`. CI
    /// evidence is authoritative only for this head. `None` on rows written
    /// before head SHAs were recorded.
    pub head_sha: Option<String>,
}

/// The state of a PR head's required checks, as last observed by a reconcile.
/// Failure dominates: any failing required check makes the head `Failing`
/// regardless of what else is still pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiState {
    Pending,
    Passing,
    Failing,
}

impl CiState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Passing => "passing",
            Self::Failing => "failing",
        }
    }
}

/// One required check that is not passing, named so the `ci-fix` skill can
/// resolve the exact failure from its logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiCheck {
    pub name: String,
    pub url: Option<String>,
}

/// The required-check reading for one PR head. `head_sha` pins it: a reading is
/// stale — and never starts repair — once the PR's head moves past it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiObservation {
    pub head_sha: String,
    pub state: CiState,
    pub failing_checks: Vec<CiCheck>,
    pub observed_at: OffsetDateTime,
}

/// One failed CI head carried forward after the PR's current observation moves
/// on. This is evidence about the recovery loop, never a wake queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiIncident {
    pub identity: String,
    pub landing_id: Option<PrLandingId>,
    pub task_id: Option<TaskId>,
    pub pr_id: Option<TaskPrId>,
    pub repo: String,
    pub pr_number: u32,
    pub failed_head_sha: String,
    /// The first authoritative post-turn remote head that settlement observed to
    /// differ from `failed_head_sha` — the head the repair body actually shipped
    /// for this incident. `None` until a ci-fix body advances the head; written
    /// once and never overwritten by a later push.
    pub repaired_head_sha: Option<String>,
    pub failure_set: Vec<String>,
    pub provider_completed_at: Option<OffsetDateTime>,
    pub poll_observed_at: Option<OffsetDateTime>,
    pub webhook_received_at: Option<OffsetDateTime>,
    /// Landing generation that most recently responded to this incident.
    pub claimed_landing_generation: Option<u64>,
    pub responded_at: Option<OffsetDateTime>,
    pub green_at: Option<OffsetDateTime>,
    pub merged_at: Option<OffsetDateTime>,
    pub blocked_at: Option<OffsetDateTime>,
    pub blocked_reason: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// The last attempt to refresh one persisted GitHub PR. This metadata lives on
/// `TaskPr` beside the cached PR fields: it bounds repeated reads across `lf`
/// processes without making GitHub the source of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubObservation {
    pub checked_at: OffsetDateTime,
    pub result: GithubObservationResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GithubObservationResult {
    Fresh,
    Partial { reason: String },
    Degraded { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskPrRepairKind {
    AvoidableRebaseAgent,
    ManualGitRepair,
}

impl TaskPrRepairKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::AvoidableRebaseAgent => "avoidable_rebase_agent",
            Self::ManualGitRepair => "manual_git_repair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrPhase {
    Working,
    Publishing,
    Open,
    Merged,
    Abandoned,
}

impl PrPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Publishing => "publishing",
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Working | Self::Publishing | Self::Open)
    }

    pub fn is_settled(self) -> bool {
        matches!(self, Self::Merged | Self::Abandoned)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrMergeMode {
    User,
    Auto,
}

impl PrMergeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Auto => "auto",
        }
    }
}

impl FromStr for PrMergeMode {
    type Err = TaskDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "user" => Ok(Self::User),
            "auto" => Ok(Self::Auto),
            _ => Err(TaskDataError::InvalidInvariant(format!(
                "invalid PR merge mode: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrMergeRequest {
    pub mode: PrMergeMode,
    pub requested_at: OffsetDateTime,
    pub head_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrPresentation {
    pub title: String,
    pub body: String,
    pub head_sha: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrPublication {
    pub requested_at: OffsetDateTime,
    pub presentation: Option<PrPresentation>,
    pub github: Option<GithubPr>,
    pub merge: Option<PrMergeRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPr {
    pub id: TaskPrId,
    pub task_id: TaskId,
    pub sequence: u32,
    pub slug: String,
    pub branch: String,
    pub base_commit: String,
    /// Selected parent PR, or `None` when rooted on the default branch.
    /// Selection does not move Git: `base_commit` remains the child's last
    /// recorded fork until sync succeeds. The link clears after the parent
    /// merges and this PR collapses onto main.
    pub parent_pr_id: Option<TaskPrId>,
    pub publication: Option<PrPublication>,
    pub merge_commit: Option<String>,
    pub abandoned_at: Option<OffsetDateTime>,
    /// The most recent required-check reading for this PR's open head. `None`
    /// until the head has been observed; ignored once the head moves past
    /// `CiObservation::head_sha`.
    pub ci_observation: Option<CiObservation>,
    /// Last GitHub refresh attempt. A fresh result coalesces reads briefly; a
    /// degraded result opens a longer circuit while the durable PR fields stand.
    pub github_observation: Option<GithubObservation>,
    /// Id of the first-class Linear attachment linking this PR on its owning
    /// issue. `None` until the PR is first published; carried forward so later
    /// publishes update the same attachment in place.
    #[serde(deserialize_with = "crate::id::deserialize_uuid_option")]
    pub linear_attachment_id: Option<String>,
    /// Id of the loopflow-managed Linear comment carrying the PR URL and state.
    /// Its presence switches the writeback from `commentCreate` to `commentUpdate`.
    #[serde(deserialize_with = "crate::id::deserialize_uuid_option")]
    pub linear_comment_id: Option<String>,
    /// `None` when the last Linear linkage writeback succeeded; the last error
    /// string when it degraded. The GitHub publication still succeeded — this only
    /// records that the Linear side is behind, and is cleared on the next success.
    pub linear_link_error: Option<String>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl TaskPr {
    pub fn phase(&self) -> PrPhase {
        if self.abandoned_at.is_some() {
            PrPhase::Abandoned
        } else if self.merge_commit.is_some() {
            PrPhase::Merged
        } else if self
            .publication
            .as_ref()
            .is_some_and(|publication| publication.github.is_some())
        {
            PrPhase::Open
        } else if self.publication.is_some() {
            PrPhase::Publishing
        } else {
            PrPhase::Working
        }
    }

    pub fn github(&self) -> Option<&GithubPr> {
        self.publication
            .as_ref()
            .and_then(|publication| publication.github.as_ref())
    }

    /// The current head SHA of the open PR, when recorded.
    pub fn head_sha(&self) -> Option<&str> {
        self.github().and_then(|github| github.head_sha.as_deref())
    }

    /// Reviewer-facing copy, only while it describes the current published head.
    pub fn presentation(&self) -> Option<&PrPresentation> {
        let presentation = self.publication.as_ref()?.presentation.as_ref()?;
        (self.head_sha() == Some(presentation.head_sha.as_str())).then_some(presentation)
    }

    /// The explicit merge request, only while it names the current PR head.
    pub fn merge_request(&self) -> Option<&PrMergeRequest> {
        let request = self.publication.as_ref()?.merge.as_ref()?;
        (self.head_sha() == Some(request.head_sha.as_str())).then_some(request)
    }

    /// The CI reading, but only while it still describes the PR's current head.
    /// Once the head moves, the reading is stale and this returns `None` — the
    /// same freshness rule that keeps stale failures from waking work.
    pub fn fresh_ci(&self) -> Option<&CiObservation> {
        let observation = self.ci_observation.as_ref()?;
        match self.head_sha() {
            Some(head) if head == observation.head_sha => Some(observation),
            _ => None,
        }
    }

    /// Whether the current published head satisfies its merge checks.
    pub fn merge_checks_passed(&self) -> bool {
        self.phase() == PrPhase::Open
            && self
                .fresh_ci()
                .is_some_and(|observation| observation.state == CiState::Passing)
    }

    pub fn is_active(&self) -> bool {
        self.phase().is_active()
    }

    pub fn is_settled(&self) -> bool {
        self.phase().is_settled()
    }

    pub fn validate(&self) -> Result<(), TaskDataError> {
        self._validate(true)
    }

    /// Accept historical rows whose reviewer copy was never persisted.
    pub(crate) fn validate_persisted(&self) -> Result<(), TaskDataError> {
        self._validate(false)
    }

    fn _validate(&self, require_presentation: bool) -> Result<(), TaskDataError> {
        if self.sequence == 0 {
            return Err(TaskDataError::InvalidInvariant(
                "task pull request sequence starts at 1".to_string(),
            ));
        }
        if self.slug.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(
                "task PR slug cannot be empty".to_string(),
            ));
        }
        if self.branch.trim().is_empty() || self.base_commit.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(
                "task PR requires a branch and base commit".to_string(),
            ));
        }
        if let Some(presentation) = self
            .publication
            .as_ref()
            .and_then(|publication| publication.presentation.as_ref())
        {
            if presentation.title.trim().is_empty()
                || presentation.body.trim().is_empty()
                || presentation.head_sha.trim().is_empty()
            {
                return Err(TaskDataError::InvalidInvariant(
                    "PR presentation requires non-empty title, body, and head".to_string(),
                ));
            }
        }
        if let Some(publication) = &self.publication {
            if let Some(github) = &publication.github {
                if github.number == 0 || github.url.trim().is_empty() {
                    return Err(TaskDataError::InvalidInvariant(
                        "GitHub PR number and URL cannot be empty".to_string(),
                    ));
                }
            }
            if let Some(request) = &publication.merge {
                match publication.presentation.as_ref() {
                    Some(presentation) if presentation.head_sha != request.head_sha => {
                        return Err(TaskDataError::InvalidInvariant(
                            "merge request and PR copy must name the same head".to_string(),
                        ));
                    }
                    None if require_presentation => {
                        return Err(TaskDataError::InvalidInvariant(
                            "merge request requires reviewer-facing PR copy".to_string(),
                        ));
                    }
                    Some(_) | None => {}
                }
                let github = publication.github.as_ref().ok_or_else(|| {
                    TaskDataError::InvalidInvariant(
                        "merge request requires a GitHub PR".to_string(),
                    )
                })?;
                if request.head_sha.trim().is_empty()
                    || github.head_sha.as_deref() != Some(request.head_sha.as_str())
                {
                    return Err(TaskDataError::InvalidInvariant(
                        "merge request must name the current GitHub PR head".to_string(),
                    ));
                }
            }
        }
        if self.merge_commit.is_some() && self.github().is_none() {
            return Err(TaskDataError::InvalidInvariant(
                "merged PR requires a GitHub PR".to_string(),
            ));
        }
        if self.merge_commit.is_some() && self.abandoned_at.is_some() {
            return Err(TaskDataError::InvalidInvariant(
                "a PR cannot be both merged and abandoned".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LinearWritebackOperation {
    CompleteTask,
    ReopenTask,
    CancelTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LinearWritebackState {
    Current,
    Pending {
        operation: LinearWritebackOperation,
        error: String,
    },
}

/// Freshness of the GitHub observation behind the Task's returned PR state.
/// The durable attempt metadata lives on `TaskPr`; this derived view tells one
/// caller whether it read GitHub, reused a recent reading, or opened a degraded
/// circuit while preserving the cached PR fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "freshness", rename_all = "snake_case")]
pub enum Observation {
    /// No remote read applies, as for an unpublished working PR.
    #[default]
    NotRequired,
    /// GitHub answered during this reconcile.
    Fresh { observed_at: OffsetDateTime },
    /// A recent successful reading was reused without spending another request.
    Cached { observed_at: OffsetDateTime },
    /// A bounded read failed. The reason and retry boundary are durable, so
    /// later local controls reuse the cached state without hammering GitHub.
    Degraded {
        reason: String,
        cached_as_of: OffsetDateTime,
        retry_at: OffsetDateTime,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    /// Local optimistic-write revision; independent of Linear observation age.
    pub revision: u64,
    pub identifier: String,
    pub title: String,
    pub description: String,
    /// Shared planning fields; independent of local execution and Workflow position.
    pub rank: u32,
    pub assignee: Option<String>,
    pub due_date: Option<String>,
    /// The release this bug regressed in (`v0.15.3`); `stable` skips releases
    /// it covers until the release that ships the fix.
    pub regressed_in: Option<String>,
    /// Linear's state type, `completed` once done; the lifecycle is `TaskState`.
    pub state: Option<String>,
    pub completed_at: Option<String>,
    /// Optional Linear binding, not a second owner of this record.
    pub linear_id: Option<LinearIssueId>,
    /// Acquisition time of the retained Linear observation, never a write clock.
    pub pm_snapshot_synced_at: Option<i64>,
    pub pm_writeback: LinearWritebackState,
    /// Read projection from the Project join, validated against the Project on save.
    pub wave_id: Option<WaveId>,
    /// Stable repository identity, independent of planning membership.
    pub repository_id: crate::durable::RepositoryId,
    pub project_id: Option<ProjectId>,
    pub worktree: Option<PathBuf>,
    pub workspace_slug: String,
    pub branch: String,
    pub base_commit: String,
    pub parent_pr_id: Option<TaskPrId>,
    /// Explicit choice for every Graph step; None uses the step/config defaults.
    pub agent: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    /// Per-command view derived from the active PR's durable observation cache.
    #[serde(skip)]
    pub observation: Observation,
}

impl Task {
    pub fn linear_id(&self) -> Result<&LinearIssueId, LinearBindingError> {
        self.linear_id
            .as_ref()
            .ok_or(LinearBindingError::NoLinearMapping)
    }

    pub fn worktree(&self) -> Result<&PathBuf, TaskDataError> {
        self.worktree.as_ref().ok_or_else(|| {
            TaskDataError::InvalidInvariant(format!(
                "Task {} has no checkout; place it with `lf checkout {}`",
                self.id, self.id
            ))
        })
    }

    pub fn validate(&self) -> Result<(), TaskDataError> {
        if self.worktree.is_some() && self.workspace_slug.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(format!(
                "Task {} requires a workspace slug",
                self.id
            )));
        }
        Ok(())
    }
}

/// Historical keep-open scope, retained for event decoding only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFollowUp {
    pub outcome: String,
    pub evidence: String,
    pub check_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventKind {
    // Historical follow-through events remain readable; no current writers.
    FollowThroughIntent {
        intent: serde_json::Value,
    },
    FollowThroughLinked {
        link: serde_json::Value,
    },
    FollowThroughRelationConfirmed {
        #[serde(deserialize_with = "crate::id::deserialize_uuid")]
        relation_id: String,
        #[serde(deserialize_with = "crate::id::deserialize_task_or_uuid")]
        source_issue_id: String,
        #[serde(deserialize_with = "crate::id::deserialize_task_or_uuid")]
        target_issue_id: String,
    },
    FollowThroughDisposition {
        reason: String,
    },
    FollowThroughConversion {
        reason: String,
    },
    CheckoutInitializing {
        branch: String,
        path: String,
        base_commit: String,
    },
    CheckoutReady {
        branch: String,
        base_commit: String,
    },
    WorktreeInitializing {
        pr_id: TaskPrId,
        sequence: u32,
        branch: String,
        path: String,
        base_commit: String,
    },
    Started,
    BodyHandedOff {
        handoff: crate::child::ChildBodyHandoff,
    },
    Progress {
        summary: String,
    },
    // Historical keep-open decision; no current writer.
    FollowUp {
        remaining: Option<TaskFollowUp>,
        reason: String,
    },
    // Retained for reading recorded decisions; no current writer.
    HistoricalUncertaintyAccepted {
        #[serde(alias = "exec_ids")] // Append-only decisions keep their original bytes.
        lf_process_ids: Vec<crate::id::LfProcessId>,
        reason: String,
    },
    // The rename migration rewrote `task_events`; Project `TaskObserved` copies keep their bytes.
    #[serde(alias = "flow_finished")]
    GraphFinished {
        #[serde(deserialize_with = "crate::id::deserialize_uuid")]
        invocation_id: String,
        #[serde(alias = "flow")]
        graph: String,
        summary: String,
    },
    /// Historical authored direction; the immutable publication owns its text.
    Steer {
        steer_id: crate::id::SteerId,
    },
    /// A request to end the Task's current turn so the next one re-reads its
    /// direction immediately. A live run acts on interrupts issued after it
    /// launched; there is nothing to interrupt otherwise. Not a report out.
    Interrupt,
    PrStarted {
        pr_id: TaskPrId,
        sequence: u32,
        branch: String,
        base_commit: String,
    },
    PrOpened {
        pr_id: TaskPrId,
        sequence: u32,
        number: u32,
        url: String,
    },
    PrMerged {
        pr_id: TaskPrId,
        sequence: u32,
        number: u32,
        url: String,
        merge_commit: String,
    },
    CompletionRequested {
        reason: Option<String>,
    },
    Completed {
        summary: String,
    },
    Failed {
        error: String,
        resumable: bool,
    },
}

impl TaskEventKind {
    /// Whether the event should wake the owning Wave.
    pub fn is_wave_observable(&self) -> bool {
        !matches!(
            self,
            Self::CheckoutInitializing { .. }
                | Self::CheckoutReady { .. }
                | Self::WorktreeInitializing { .. }
                | Self::Started
                | Self::Progress { .. }
                | Self::HistoricalUncertaintyAccepted { .. }
                | Self::GraphFinished { .. }
                | Self::Steer { .. }
                | Self::Interrupt
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub id: i64,
    pub task_id: TaskId,
    pub kind: TaskEventKind,
    pub created_at: OffsetDateTime,
}

/// The durable cursor for streaming Linear edits by participants into one Task.
/// It is the exactly-once ledger — what issue revision and comments have already
/// become Task direction — plus the health of the last observation, so
/// `lf task status` can show stale reads and their degraded reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLinearObservation {
    pub task_id: TaskId,
    /// Linear issue `updatedAt` last folded in. Monotonic: a read whose revision
    /// is older is dropped as a stale/out-of-order response.
    pub last_revision: String,
    /// Content basis for the next title/description diff.
    pub last_title: String,
    pub last_description: String,
    pub last_success_at: OffsetDateTime,
    /// `Some` after a failed observation (auth/quota/network); `None` is healthy.
    pub degraded_reason: Option<String>,
    pub updated_at: OffsetDateTime,
}

/// Direction persisted from one observation, without re-reading the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearObservationOutcome {
    /// The Task had no cursor yet: this observation seeded the baseline and
    /// emitted no content Steer; participant comments are still imported.
    pub baselined: bool,
    pub content_steer_applied: bool,
    /// Event ids of the steer comments this observation appended.
    pub follow_ups_created: Vec<i64>,
}

#[cfg(test)]
mod tests {
    use super::{
        GithubPr, LinearWritebackOperation, LinearWritebackState, PrPhase, PrPublication, Task,
        TaskEventKind, TaskId, TaskPr, TaskPrId,
    };
    use crate::linear::LinearIssueId;

    #[test]
    fn events_recorded_before_the_graph_rename_still_read() {
        let kind: TaskEventKind = serde_json::from_str(
            r#"{"kind":"flow_finished","invocation_id":"7d4455b3-f215-4762-965c-85ff2b0876a5","flow":"ship","summary":"merged"}"#,
        )
        .unwrap();
        assert!(matches!(kind, TaskEventKind::GraphFinished { graph, .. } if graph == "ship"));
    }

    fn task() -> Task {
        let now = time::OffsetDateTime::now_utc();
        Task {
            repository_id: crate::durable::RepositoryId::new(),
            id: TaskId::new(),

            revision: 0,
            linear_id: Some(LinearIssueId::new("issue-1").unwrap()),
            identifier: "INF-123".to_string(),
            title: "Ship it".to_string(),
            description: String::new(),
            pm_snapshot_synced_at: Some(1),

            pm_writeback: LinearWritebackState::Current,
            wave_id: Some(crate::id::WaveId::new()),
            project_id: Some(crate::work::project::ProjectId::new()),
            worktree: Some("/tmp/task".into()),
            workspace_slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            agent: None,
            created_at: now,
            updated_at: now,
            rank: 0,
            assignee: None,
            due_date: None,
            regressed_in: None,
            state: None,
            completed_at: None,
            observation: crate::work::task::Observation::NotRequired,
        }
    }

    #[test]
    fn task_content_round_trip_keeps_binding_revision_and_observation_age() {
        let mut task = task();
        task.revision = 17;
        task.pm_snapshot_synced_at = Some(42);
        let value = serde_json::to_value(&task).unwrap();
        assert!(value.get("plan").is_none());
        assert_eq!(value["title"], "Ship it");
        assert_eq!(value["revision"], 17);
        assert_eq!(value["linear_id"], "issue-1");
        assert_eq!(value["pm_snapshot_synced_at"], 42);
        assert_eq!(serde_json::from_value::<Task>(value).unwrap(), task);
    }

    #[test]
    fn task_ids_are_prefixed_and_round_trip() {
        let task = TaskId::new();
        assert_eq!(TaskId::parse(task.as_str()).unwrap(), task);
        let pr = TaskPrId::new();
        assert_eq!(TaskPrId::parse(pr.as_str()).unwrap(), pr);
    }

    #[test]
    fn pending_pm_writeback_has_a_stable_json_shape() {
        let state = LinearWritebackState::Pending {
            operation: LinearWritebackOperation::CompleteTask,
            error: "offline".to_string(),
        };

        assert_eq!(
            serde_json::to_value(state).unwrap(),
            serde_json::json!({
                "state": "pending",
                "operation": "complete_task",
                "error": "offline"
            })
        );
    }

    #[test]
    fn pr_phase_is_derived_from_durable_evidence() {
        let now = time::OffsetDateTime::now_utc();
        let mut pr = TaskPr {
            id: TaskPrId::new(),
            task_id: TaskId::new(),
            sequence: 1,
            slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
        };
        assert_eq!(pr.phase(), PrPhase::Working);

        pr.publication = Some(PrPublication {
            requested_at: now,
            presentation: None,
            github: None,
            merge: None,
        });
        assert_eq!(pr.phase(), PrPhase::Publishing);

        pr.publication.as_mut().unwrap().github = Some(GithubPr {
            number: 872,
            url: "https://github.com/loopflowstudio/loopflow/pull/872".to_string(),
            head_sha: None,
        });
        assert_eq!(pr.phase(), PrPhase::Open);

        pr.merge_commit = Some("def".to_string());
        assert_eq!(pr.phase(), PrPhase::Merged);
        assert!(pr.validate().is_ok());

        pr.abandoned_at = Some(now);
        assert_eq!(pr.phase(), PrPhase::Abandoned);
        assert!(pr.validate().is_err());
    }

    #[test]
    fn merge_request_retains_exact_head_and_publication_evidence() {
        let now = time::OffsetDateTime::now_utc();
        let mut pr = TaskPr {
            id: TaskPrId::new(),
            task_id: TaskId::new(),
            sequence: 1,
            slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                presentation: Some(super::PrPresentation {
                    title: "Ship it".to_string(),
                    body: "Reviewer context".to_string(),
                    head_sha: "head".to_string(),
                }),
                github: Some(GithubPr {
                    number: 872,
                    url: "https://github.com/loopflowstudio/loopflow/pull/872".to_string(),
                    head_sha: Some("head".to_string()),
                }),
                merge: Some(super::PrMergeRequest {
                    mode: super::PrMergeMode::User,
                    requested_at: now,
                    head_sha: "head".to_string(),
                }),
            }),
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
        };
        assert!(pr.validate().is_ok());
        assert!(pr.presentation().is_some());

        let mut persisted = pr.clone();
        persisted.publication.as_mut().unwrap().presentation = None;
        assert!(persisted.validate().is_err());
        assert!(persisted.validate_persisted().is_ok());

        pr.publication
            .as_mut()
            .unwrap()
            .github
            .as_mut()
            .unwrap()
            .head_sha = Some("new-head".to_string());
        assert!(pr.presentation().is_none());
        assert!(pr.merge_request().is_none());
        pr.publication
            .as_mut()
            .unwrap()
            .github
            .as_mut()
            .unwrap()
            .head_sha = Some("head".to_string());

        assert!(pr.validate().is_ok());
        assert!(pr.merge_request().is_some());
    }

    fn open_pr(head_sha: &str, observation: Option<super::CiObservation>) -> TaskPr {
        let now = time::OffsetDateTime::now_utc();
        TaskPr {
            id: TaskPrId::new(),
            task_id: TaskId::new(),
            sequence: 1,
            slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                presentation: None,
                github: Some(GithubPr {
                    number: 900,
                    url: "https://github.com/loopflow/loopflow/pull/900".to_string(),
                    head_sha: Some(head_sha.to_string()),
                }),
                merge: None,
            }),
            merge_commit: None,
            abandoned_at: None,
            ci_observation: observation,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn fresh_ci_ignores_a_reading_for_a_past_head() {
        let now = time::OffsetDateTime::now_utc();
        let observation = super::CiObservation {
            head_sha: "old-head".to_string(),
            state: super::CiState::Failing,
            failing_checks: vec![super::CiCheck {
                name: "build".to_string(),
                url: None,
            }],
            observed_at: now,
        };
        // Reading matches the current head: fresh.
        let current = open_pr("old-head", Some(observation.clone()));
        assert_eq!(
            current.fresh_ci().map(|ci| ci.state),
            Some(super::CiState::Failing)
        );
        // Head has moved on: the stale reading never surfaces or starts repair.
        let moved = open_pr("new-head", Some(observation));
        assert!(moved.fresh_ci().is_none());
    }

    #[test]
    fn merge_checks_require_current_head_passing_checks() {
        let observation = |head: &str, state| super::CiObservation {
            head_sha: head.to_string(),
            state,
            failing_checks: Vec::new(),
            observed_at: time::OffsetDateTime::now_utc(),
        };

        assert!(open_pr(
            "current",
            Some(observation("current", super::CiState::Passing))
        )
        .merge_checks_passed());
        assert!(!open_pr(
            "current",
            Some(observation("current", super::CiState::Pending))
        )
        .merge_checks_passed());
        assert!(
            !open_pr("current", Some(observation("old", super::CiState::Passing)))
                .merge_checks_passed()
        );
        assert!(!open_pr("current", None).merge_checks_passed());
    }

    #[test]
    fn pending_pm_writeback_is_valid_without_a_controller() {
        let mut task = task();
        task.pm_writeback = LinearWritebackState::Pending {
            operation: LinearWritebackOperation::CompleteTask,
            error: "too early".to_string(),
        };
        assert!(task.validate().is_ok());
    }
}
