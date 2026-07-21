//! Durable execution of one Linear Task.
//!
//! A Task owns one durable worktree and provider transcript. Ordered PRs
//! own the serial branches that advance the Task. Publication intent is recorded
//! before GitHub is called, then the GitHub PR record is attached to that intent.

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child::{prefixed_uuid_id, AbandonIntent};
use crate::durable::RunId;
pub use crate::durable::TaskId;
use crate::id::WaveId;
use crate::planning::TaskPlan;
use crate::project::ProjectId;

pub mod actions;
pub mod runner;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskDataError {
    #[error("invalid Task id: {0}")]
    InvalidId(String),
    #[error("invalid Task: {0}")]
    InvalidInvariant(String),
}

prefixed_uuid_id!(TaskPrId, "pr_", TaskDataError, TaskDataError::InvalidId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskLifecyclePhase {
    First,
    Loop,
    Finally,
}

impl TaskLifecyclePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Loop => "loop",
            Self::Finally => "finally",
        }
    }

    pub(crate) fn storage_str(self) -> &'static str {
        match self {
            Self::First => "kickoff",
            Self::Loop => "iterate",
            Self::Finally => "gate",
        }
    }

    pub(crate) fn from_storage_str(value: &str) -> Result<Self, TaskDataError> {
        match value {
            "kickoff" => Ok(Self::First),
            "iterate" => Ok(Self::Loop),
            "gate" => Ok(Self::Finally),
            _ => Err(TaskDataError::InvalidInvariant(format!(
                "invalid stored Task lifecycle phase: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPhasePlan {
    pub flow: String,
}

impl TaskPhasePlan {
    fn validate(&self, phase: TaskLifecyclePhase) -> Result<(), TaskDataError> {
        if self.flow.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(format!(
                "{} flow cannot be empty",
                phase.as_str()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLifecyclePlan {
    pub first: TaskPhasePlan,
    #[serde(rename = "loop")]
    pub loop_: TaskPhasePlan,
    pub finally: TaskPhasePlan,
}

impl TaskLifecyclePlan {
    pub fn standard(
        first_flow: impl Into<String>,
        loop_flow: impl Into<String>,
        finally_flow: impl Into<String>,
    ) -> Self {
        Self {
            first: TaskPhasePlan {
                flow: first_flow.into(),
            },
            loop_: TaskPhasePlan {
                flow: loop_flow.into(),
            },
            finally: TaskPhasePlan {
                flow: finally_flow.into(),
            },
        }
    }

    pub fn defaults() -> Self {
        Self::standard("task-design", "slice", "ship")
    }
    pub fn phase(&self, phase: TaskLifecyclePhase) -> &TaskPhasePlan {
        match phase {
            TaskLifecyclePhase::First => &self.first,
            TaskLifecyclePhase::Loop => &self.loop_,
            TaskLifecyclePhase::Finally => &self.finally,
        }
    }

    fn validate(&self) -> Result<(), TaskDataError> {
        self.first.validate(TaskLifecyclePhase::First)?;
        self.loop_.validate(TaskLifecyclePhase::Loop)?;
        self.finally.validate(TaskLifecyclePhase::Finally)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGateProposal {
    pub done: bool,
    pub reason: String,
}

impl TaskGateProposal {
    fn validate(&self) -> Result<(), TaskDataError> {
        if self.reason.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(
                "gate proposal reason cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

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

/// The required check `lf pr land` greens itself, by clearing `scratch/`.
///
/// See [`CiCheck::land_time_precondition`] for what this class means and the one
/// rule for admitting a name to it. `crate::ops::land::clear_scratch` is the step
/// that resolves this check; `land_time_precondition_names_a_real_ci_job` pins
/// this literal against `.github/workflows/ci.yml`.
const LAND_TIME_PRECONDITION_CHECK: &str = "scratch-clear";

/// One required check that is not passing, named so the `ci-fix` skill can
/// resolve the exact failure from its logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiCheck {
    pub name: String,
    pub url: Option<String>,
}

impl CiCheck {
    /// Whether this check asserts a *land-time precondition* — a condition
    /// `lf pr land` establishes itself, so no repair turn can green it.
    ///
    /// This is the one class of required check a Task body cannot act on.
    /// `scratch-clear` fails whenever `scratch/` holds anything but `.gitkeep`,
    /// which is true of every PR carrying its own design doc — i.e. every Task PR
    /// during first and loop, by construction — and
    /// `crate::ops::land::clear_scratch` is what greens it, not a code change. A
    /// body woken to "repair" it could only delete the Task's design artifact,
    /// to green a check land greens anyway.
    ///
    /// A name belongs here only when an `lf pr land` step is what resolves it.
    /// This is not a catalogue of CI jobs, and it is not a mute button for checks
    /// that are merely hard to fix: a check anyone *could* fix by changing the
    /// tree does not belong here, however annoying it is.
    pub fn land_time_precondition(&self) -> bool {
        self.name == LAND_TIME_PRECONDITION_CHECK
    }
}

/// The required-check reading for one PR head. `head_sha` pins it: a reading is
/// stale — and never wakes work — once the PR's head moves past it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiObservation {
    pub head_sha: String,
    pub state: CiState,
    pub failing_checks: Vec<CiCheck>,
    pub observed_at: OffsetDateTime,
}

impl CiObservation {
    /// The current failing required checks by name, sorted — the content half of
    /// a [`CiIncident`]'s identity. Empty unless `state` is `Failing`.
    pub fn failure_set(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .failing_checks
            .iter()
            .map(|check| check.name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Whether this reading makes a `ci-fix` wake *legal*: the current head is
    /// failing a required check that a repair turn could actually act on.
    ///
    /// This asks only about legality. Whether a wake has already fired for this
    /// exact failure is a separate question with a separate owner — the durable
    /// CI incident, keyed on the incident identity and claimed by one Run. Those
    /// two questions used to be conflated in one mutable JSON marker on this
    /// struct, which meant the wake was deduplicated by a value re-derived on
    /// every reconcile and committed only once a body had already been born.
    ///
    /// A head whose failures are *all* land-time preconditions is red and not
    /// repairable ([`CiCheck::land_time_precondition`]): waking a body there
    /// spends a full turn on work whose only successful action is destructive.
    /// The reading still reports the failure — this refuses the wake, it does not
    /// deny the red — so `lf ci` and `lf task status` are unchanged.
    pub fn wake_legal(&self) -> bool {
        if self.state != CiState::Failing {
            return false;
        }
        // An unnamed failure is one we could not classify, not one proven
        // harmless, so it still wakes: a filter that swallows unknown failures is
        // a mute button. Only a non-empty set whose every member land resolves is
        // provably not a repair.
        self.failing_checks.is_empty()
            || self
                .failing_checks
                .iter()
                .any(|check| !check.land_time_precondition())
    }

    /// Whether this head is red *only* on land-time preconditions — failing, with
    /// at least one named failure, and every named failure one that
    /// [`CiCheck::land_time_precondition`] resolves at land.
    ///
    /// This is the dual of [`CiObservation::wake_legal`] within the failing
    /// state: such a head holds nothing a Task body could repair (`lf pr land`
    /// greens it by clearing `scratch/`), so it does not belong to CI repair.
    /// The action model and Waves supervision read it to stop recommending a
    /// doomed Resume or labelling settlement preparation as "fixing CI".
    ///
    /// False for a passing or pending head, and false the moment any failure is a
    /// real leaf or an unclassified one — the same anti-mute-button rule as
    /// `wake_legal`: an unnamed failure keeps the head owned by CI.
    pub fn only_land_time_preconditions(&self) -> bool {
        self.state == CiState::Failing
            && !self.failing_checks.is_empty()
            && self
                .failing_checks
                .iter()
                .all(|check| check.land_time_precondition())
    }
}

/// One failed CI head carried forward after the PR's current observation moves
/// on. This is evidence about the recovery loop, never a wake queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiIncident {
    pub identity: String,
    pub task_id: TaskId,
    pub pr_id: TaskPrId,
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
    /// Active or most recent Run selected to repair this incident. A successor
    /// may replace it only after the prior Run lost execution authority.
    pub claimed_run_id: Option<RunId>,
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
    Degraded { reason: String },
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
pub enum AfterMerge {
    ContinueTask,
    CompleteTask,
}

impl AfterMerge {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContinueTask => "continue_task",
            Self::CompleteTask => "complete_task",
        }
    }
}

impl FromStr for AfterMerge {
    type Err = TaskDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "continue_task" => Ok(Self::ContinueTask),
            "complete_task" => Ok(Self::CompleteTask),
            _ => Err(TaskDataError::InvalidInvariant(format!(
                "invalid after-merge disposition: {value}"
            ))),
        }
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
    pub after_merge: AfterMerge,
    pub next_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrPublication {
    pub requested_at: OffsetDateTime,
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
    /// Another Task's PR this worktree was placed on, or `None` when rooted on
    /// the default branch. `base_commit` is that parent's exact fork commit; the
    /// link clears after the parent merges and this PR collapses onto main.
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
    pub linear_attachment_id: Option<String>,
    /// Id of the loopflow-managed Linear comment carrying the PR URL and state.
    /// Its presence switches the writeback from `commentCreate` to `commentUpdate`.
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

    /// The explicit merge request, only while it names the current PR head.
    pub fn merge_request(&self) -> Option<&PrMergeRequest> {
        let request = self.publication.as_ref()?.merge.as_ref()?;
        (self.head_sha() == Some(request.head_sha.as_str())).then_some(request)
    }

    /// Settlement disposition. A PR merged without an explicit request safely
    /// continues the Task; only a head-pinned request may complete it.
    pub fn after_merge(&self) -> AfterMerge {
        self.merge_request()
            .map_or(AfterMerge::ContinueTask, |request| request.after_merge)
    }

    pub fn next_slug(&self) -> Option<&str> {
        self.merge_request()
            .and_then(|request| request.next_slug.as_deref())
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
        if let Some(publication) = &self.publication {
            if let Some(github) = &publication.github {
                if github.number == 0 || github.url.trim().is_empty() {
                    return Err(TaskDataError::InvalidInvariant(
                        "GitHub PR number and URL cannot be empty".to_string(),
                    ));
                }
            }
            if let Some(request) = &publication.merge {
                if request.next_slug.as_deref().is_some_and(|slug| {
                    slug.split('-').any(|word| {
                        word.is_empty()
                            || !word
                                .bytes()
                                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                    })
                }) {
                    return Err(TaskDataError::InvalidInvariant(
                        "next branch slug must be lowercase kebab-case".to_string(),
                    ));
                }
                if request.after_merge == AfterMerge::CompleteTask && request.next_slug.is_some() {
                    return Err(TaskDataError::InvalidInvariant(
                        "a completing pull request cannot name a next branch".to_string(),
                    ));
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
pub enum PmWritebackOperation {
    CompleteTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PmWritebackState {
    Current,
    Pending {
        operation: PmWritebackOperation,
        error: String,
    },
}

/// Freshness of the GitHub observation behind the Task's returned PR state.
/// The durable attempt metadata lives on `TaskPr`; this derived view tells one
/// caller whether it read GitHub, reused a recent reading, or opened a degraded
/// circuit while preserving the cached PR fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
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
    /// Current planning facts from the PM system.
    pub plan: TaskPlan,
    pub pm_writeback: PmWritebackState,
    /// Root ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
    /// Required runtime parent. Every Task reports through one durable Project
    /// Work; its Wave retains root inspection and override authority.
    pub project_id: ProjectId,
    pub worktree: PathBuf,
    pub workspace_slug: String,
    /// Three pinned phase flows.
    pub lifecycle: TaskLifecyclePlan,
    /// Current phase entry. `phase_epoch` advances on every transition,
    /// including Finally → Loop, so stale bodies cannot rewind the Task.
    pub lifecycle_phase: TaskLifecyclePhase,
    pub phase_epoch: u32,
    pub phase_cursor: u32,
    pub phase_iteration: u32,
    /// Number of Gate entries attempted by this Task.
    pub gate_cycle: u32,
    /// Outcome proposed by Iterate and awaiting Gate approval.
    pub gate_proposal: Option<TaskGateProposal>,
    /// Provider/model selection for the next body generation. This is mutable
    /// lease state, not Task identity.
    pub agent: String,
    /// Harness family for the next/current body generation.
    pub provider: String,
    /// Transcript handle reusable only by a compatible provider generation.
    pub provider_session_id: Option<String>,
    /// Set when abandonment is *requested*, not when it is applied. No launch
    /// path may start a Run for Task Work carrying this.
    pub abandon_intent: Option<AbandonIntent>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    /// Per-command view derived from the active PR's durable observation cache.
    #[serde(skip)]
    pub observation: Observation,
}

impl Task {
    /// Why a supervisor must not start another process generation, if it must not.
    ///
    /// Two intents bar an automatic restart:
    ///
    /// - abandonment has been *requested* — the runner has not consumed the
    ///   command yet, but the decision is made;
    /// - publication or merge was explicitly requested. A merely published PR
    ///   remains ordinary Task continuity and does not bar the supervisor.
    ///
    /// The third was the 2026-07-14 W2-129 failure: `Open` is not terminal
    /// and carries no live process, so it reads exactly like Work that
    /// merely stopped. A wake therefore launched generation 2, which reopened
    /// the flow at `task/clarify` and began re-doing work whose PR (#878) was
    /// already awaiting a merge. An explicit merge request is not an invitation
    /// to start over.
    ///
    /// A User may still `lf task resume` a submitted Task explicitly;
    /// this bars the supervisor, not the operator.
    pub fn supervisor_restart_bar(&self, active_pr: Option<&TaskPr>) -> Option<String> {
        if let Some(bar) = self.terminal_or_abandon_bar() {
            return Some(bar);
        }
        if let Some(pr) = active_pr {
            match pr.phase() {
                PrPhase::Publishing => return Some(self.publishing_bar()),
                PrPhase::Open if pr.merge_request().is_some() => {
                    return Some(self.open_pr_bar(pr));
                }
                PrPhase::Open => {}
                PrPhase::Working | PrPhase::Merged | PrPhase::Abandoned => {}
            }
        }
        None
    }

    /// The restart bar for an automated `ci-fix` wake. Identical to the supervisor
    /// bar, except an `Open` PR is *permitted* when its current head carries a
    /// failing required check ([`CiObservation::wake_legal`]). This is the one
    /// automated path allowed to restart a submitted Task, and only on fresh
    /// current-head failure evidence — never a blind wake over passing or pending
    /// work, and never past the terminal, abandon, or publishing bars.
    ///
    /// The bar answers legality only. The incident's Run claim answers whether
    /// this failure already owns execution.
    pub fn ci_fix_restart_bar(&self, active_pr: Option<&TaskPr>) -> Option<String> {
        if let Some(bar) = self.terminal_or_abandon_bar() {
            return Some(bar);
        }
        if let Some(pr) = active_pr {
            match pr.phase() {
                PrPhase::Publishing => return Some(self.publishing_bar()),
                // An open PR restarts only on a current-head required-check
                // failure; otherwise it stays barred exactly as the supervisor
                // bar leaves it.
                PrPhase::Open
                    if pr.merge_request().is_some()
                        && !pr.fresh_ci().is_some_and(CiObservation::wake_legal) =>
                {
                    return Some(self.open_pr_bar(pr));
                }
                PrPhase::Open | PrPhase::Working | PrPhase::Merged | PrPhase::Abandoned => {}
            }
        }
        None
    }

    /// The product-level bar that holds for every automatic restart intent.
    /// Terminal execution state belongs to Work and is checked by the caller.
    pub(crate) fn terminal_or_abandon_bar(&self) -> Option<String> {
        if let Some(intent) = &self.abandon_intent {
            return Some(format!(
                "Task {} is being abandoned: {}",
                self.plan.identifier, intent.reason
            ));
        }
        None
    }

    fn publishing_bar(&self) -> String {
        format!(
            "Task {} requested PR publication but has no GitHub PR record; \
             resume it explicitly with `lf task resume {}` to retry publication",
            self.plan.identifier, self.plan.identifier,
        )
    }

    /// The open-PR restart refusal. Only a *supervisor* (`supervisor_restart_bar`
    /// / a non-wake-legal `ci_fix_restart_bar`) ever reads this — an operator
    /// resume takes the abandon-only `ExplicitResume` bar. The text names the
    /// explicit settlement owner instead of recommending `lf task resume`, which
    /// a supervisor re-running would only self-loop on.
    fn open_pr_bar(&self, pr: &TaskPr) -> String {
        let number = pr.github().expect("open Task PR passed validation").number;
        let request = pr
            .merge_request()
            .expect("open PR restart bar requires a current merge request");
        let short = request.head_sha.chars().take(12).collect::<String>();
        match request.mode {
            PrMergeMode::User => format!(
                "Task {} requested a user merge of pull request #{} at head {}. \
                 The supervisor will not restart it until that explicit merge \
                 request settles or the head changes.",
                self.plan.identifier, number, short,
            ),
            PrMergeMode::Auto => format!(
                "Task {} requested GitHub auto-merge of pull request #{} at head {}. \
                 The supervisor will not restart it until that explicit merge \
                 request settles or the head changes.",
                self.plan.identifier, number, short,
            ),
        }
    }

    pub fn validate(&self) -> Result<(), TaskDataError> {
        if self.workspace_slug.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(format!(
                "Task {} requires a workspace slug",
                self.id
            )));
        }
        self.lifecycle.validate()?;
        if self.phase_epoch == 0 {
            return Err(TaskDataError::InvalidInvariant(
                "Task lifecycle phase epoch must be positive".to_string(),
            ));
        }
        if self.lifecycle_phase == TaskLifecyclePhase::Finally && self.gate_proposal.is_none() {
            return Err(TaskDataError::InvalidInvariant(
                "Task finally phase requires a proposed outcome".to_string(),
            ));
        }
        if self.lifecycle_phase != TaskLifecyclePhase::Finally && self.gate_proposal.is_some() {
            return Err(TaskDataError::InvalidInvariant(
                "Task gate proposal is valid only during finally phase".to_string(),
            ));
        }
        if let Some(proposal) = &self.gate_proposal {
            proposal.validate()?;
        }
        if matches!(self.pm_writeback, PmWritebackState::Pending { .. }) && self.gate_cycle == 0 {
            return Err(TaskDataError::InvalidInvariant(
                "pending PM completion requires an active gate cycle".to_string(),
            ));
        }
        Ok(())
    }

    pub fn phase_plan(&self) -> &TaskPhasePlan {
        self.lifecycle.phase(self.lifecycle_phase)
    }

    pub fn lifecycle_cycle(&self) -> u32 {
        match self.lifecycle_phase {
            TaskLifecyclePhase::First => 0,
            TaskLifecyclePhase::Loop => self.gate_cycle + 1,
            TaskLifecyclePhase::Finally => self.gate_cycle,
        }
    }

    pub fn enter_loop(&mut self) -> Result<(), TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::First
            && self.lifecycle_phase != TaskLifecyclePhase::Finally
        {
            return Err(TaskDataError::InvalidInvariant(
                "only first or finally may enter loop".to_string(),
            ));
        }
        self.lifecycle_phase = TaskLifecyclePhase::Loop;
        self.phase_epoch += 1;
        self.phase_cursor = 0;
        self.phase_iteration = 0;
        self.gate_proposal = None;
        self.updated_at = OffsetDateTime::now_utc();
        Ok(())
    }

    pub fn enter_finally(&mut self, proposal: TaskGateProposal) -> Result<(), TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::Loop {
            return Err(TaskDataError::InvalidInvariant(
                "only loop may enter finally".to_string(),
            ));
        }
        proposal.validate()?;
        self.lifecycle_phase = TaskLifecyclePhase::Finally;
        self.phase_epoch += 1;
        self.phase_cursor = 0;
        self.phase_iteration = 0;
        self.gate_cycle += 1;
        self.gate_proposal = Some(proposal);
        self.updated_at = OffsetDateTime::now_utc();
        Ok(())
    }

    pub fn approved_gate_proposal(&self) -> Result<TaskGateProposal, TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::Finally {
            return Err(TaskDataError::InvalidInvariant(
                "only finally may approve a proposed outcome".to_string(),
            ));
        }
        self.gate_proposal.clone().ok_or_else(|| {
            TaskDataError::InvalidInvariant("Task gate has no proposed outcome".to_string())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventKind {
    Started,
    BodyHandedOff {
        handoff: crate::child::ChildBodyHandoff,
    },
    Progress {
        summary: String,
    },
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
    Completed {
        summary: String,
    },
    Failed {
        error: String,
        resumable: bool,
    },
}

impl TaskEventKind {
    /// Whether the event crosses the required Task → Project boundary.
    pub fn is_project_observable(&self) -> bool {
        !matches!(self, Self::Started | Self::Progress { .. })
    }

    /// Whether a Project-observable Task event also belongs in the root Wave.
    /// This currently mirrors the Project boundary; the server-topology design
    /// must decide whether the duplicate delivery remains necessary.
    pub fn is_root_wave_observable(&self) -> bool {
        self.is_project_observable()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub id: i64,
    pub task_id: TaskId,
    pub kind: TaskEventKind,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskObservation {
    pub task_id: TaskId,
    pub issue_identifier: String,
    pub event_id: i64,
    pub event: TaskEventKind,
}

impl TaskObservation {
    pub fn inbox_id(&self) -> String {
        format!("task-{}-{}", self.task_id, self.event_id)
    }

    pub fn prompt(&self) -> String {
        let payload = serde_json::to_string(&self.event)
            .expect("Task observation always serializes to structured JSON");
        format!(
            "<task_observation task_id=\"{}\" issue=\"{}\" event_id=\"{}\">\n{}\n</task_observation>",
            self.task_id, self.issue_identifier, self.event_id, payload
        )
    }
}

/// The durable cursor for streaming human Linear edits into one Task.
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

/// One Linear observation, ready to persist atomically as Task direction. The
/// directive is applied only if the stored title/description still differ
/// (compare-and-set), and each follow-up becomes a command only on its first
/// entry into the ledger — so overlapping polls, restarts, and out-of-order
/// responses never duplicate direction.
#[derive(Debug, Clone)]
pub struct LinearObservationApply {
    pub task_id: TaskId,
    pub revision: String,
    pub title: String,
    pub description: String,
    pub observed_at: OffsetDateTime,
    /// A title/description edit to persist as one authored Steer.
    pub content_steer: Option<String>,
    /// Human comments observed this pass, oldest first.
    pub follow_ups: Vec<LinearFollowUp>,
}

#[derive(Debug, Clone)]
pub struct LinearFollowUp {
    pub comment_id: String,
    pub text: String,
}

/// What one [`LinearObservationApply`] actually wrote — enough for the caller to
/// report receipts without re-reading the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearObservationOutcome {
    /// The Task had no cursor yet: this observation seeded the baseline and
    /// emitted no direction (existing comments are marked seen, not replayed).
    pub baselined: bool,
    pub content_steer_applied: bool,
    pub follow_ups_created: Vec<crate::durable::SteerId>,
}

#[cfg(test)]
mod tests {
    use super::{
        AfterMerge, GithubPr, PmWritebackOperation, PmWritebackState, PrPhase, PrPublication, Task,
        TaskGateProposal, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskObservation, TaskPr,
        TaskPrId,
    };
    use crate::planning::{LinearIssueId, TaskPlan};

    fn task() -> Task {
        let now = time::OffsetDateTime::now_utc();
        Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("issue-1").unwrap(),
                identifier: "INF-123".to_string(),
                title: "Ship it".to_string(),
                description: String::new(),
                pm_snapshot_synced_at: 1,
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: crate::project::ProjectId::new(),
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
            lifecycle: TaskLifecyclePlan::defaults(),
            lifecycle_phase: TaskLifecyclePhase::Loop,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        }
    }

    #[test]
    fn task_ids_are_prefixed_and_round_trip() {
        let task = TaskId::new();
        assert_eq!(TaskId::parse(task.as_str()).unwrap(), task);
        let pr = TaskPrId::new();
        assert_eq!(TaskPrId::parse(pr.as_str()).unwrap(), pr);
    }

    #[test]
    fn task_observation_has_a_stable_structured_inbox_identity() {
        let observation = TaskObservation {
            task_id: TaskId::from_raw("ts_example"),
            issue_identifier: "INF-123".to_string(),
            event_id: 42,
            event: super::TaskEventKind::Failed {
                error: "provider stopped".to_string(),
                resumable: true,
            },
        };

        assert_eq!(observation.inbox_id(), "task-ts_example-42");
        assert!(observation.prompt().contains("<task_observation"));
        assert!(observation.prompt().contains("\"kind\":\"failed\""));
    }

    #[test]
    fn pending_pm_writeback_has_a_stable_json_shape() {
        let state = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
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
    fn merge_request_contains_its_disposition() {
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
                github: Some(GithubPr {
                    number: 872,
                    url: "https://github.com/loopflowstudio/loopflow/pull/872".to_string(),
                    head_sha: Some("head".to_string()),
                }),
                merge: Some(super::PrMergeRequest {
                    mode: super::PrMergeMode::User,
                    requested_at: now,
                    head_sha: "head".to_string(),
                    after_merge: AfterMerge::ContinueTask,
                    next_slug: Some("released_upgrade".to_string()),
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
        assert!(pr.validate().is_err());

        let merge = pr.publication.as_mut().unwrap().merge.as_mut().unwrap();
        merge.next_slug = Some("released-upgrade".to_string());
        assert!(pr.validate().is_ok());

        pr.publication
            .as_mut()
            .unwrap()
            .merge
            .as_mut()
            .unwrap()
            .after_merge = AfterMerge::CompleteTask;
        assert!(pr.validate().is_err());
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
        // Head has moved on: the stale reading never surfaces (and never wakes work).
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

    fn failing(head: &str, checks: &[&str]) -> super::CiObservation {
        super::CiObservation {
            head_sha: head.to_string(),
            state: super::CiState::Failing,
            failing_checks: checks
                .iter()
                .map(|name| super::CiCheck {
                    name: name.to_string(),
                    url: None,
                })
                .collect(),
            observed_at: time::OffsetDateTime::now_utc(),
        }
    }

    fn with_merge_request(mut pr: TaskPr, mode: super::PrMergeMode) -> TaskPr {
        let head_sha = pr.head_sha().expect("test PR has a head").to_string();
        pr.publication.as_mut().unwrap().merge = Some(super::PrMergeRequest {
            mode,
            requested_at: time::OffsetDateTime::now_utc(),
            head_sha,
            after_merge: AfterMerge::ContinueTask,
            next_slug: None,
        });
        pr
    }

    /// The observation answers legality — is this head failing *now* — and nothing
    /// else. Whether a wake already fired for a failure is the command ledger's
    /// question, keyed on the incident identity; it used to be a mutable marker on
    /// this struct, which is what let a repeat poll race a body's birth.
    #[test]
    fn ci_wake_legality_follows_only_the_current_reading() {
        let obs = failing("h1", &["build", "lint"]);
        assert!(obs.wake_legal());

        // The failing set is order-independent and deduplicated: it is the content
        // half of the incident identity, so two readings of one failure must hash
        // the same however GitHub ordered them.
        assert_eq!(
            obs.failure_set(),
            vec!["build".to_string(), "lint".to_string()]
        );
        assert_eq!(
            failing("h1", &["lint", "build"]).failure_set(),
            obs.failure_set()
        );
        assert_eq!(
            failing("h1", &["build", "build"]).failure_set(),
            vec!["build".to_string()]
        );

        // A passing or pending reading is never legal to wake on.
        let mut green = obs.clone();
        green.state = super::CiState::Passing;
        assert!(!green.wake_legal());
        let mut pending = obs.clone();
        pending.state = super::CiState::Pending;
        assert!(!pending.wake_legal());
    }

    /// A head red *only* on a land-time precondition is not a repair a body can
    /// perform: `lf pr land` clears `scratch/`, and the only action a woken body
    /// could take is deleting the Task's design doc.
    ///
    /// The direction that matters is the second half. Suppression fires only when
    /// every named failure is land-resolved — a real leaf alongside it still
    /// arms, and an *unnamed* failure still arms, because a filter that swallows
    /// failures it cannot classify is a mute button rather than a classifier.
    #[test]
    fn a_head_red_only_on_a_land_time_precondition_is_not_wakeable() {
        assert!(!failing("h1", &["scratch-clear"]).wake_legal());

        // A real leaf alongside it is still a repair worth waking for.
        assert!(failing("h1", &["scratch-clear", "rust-test"]).wake_legal());
        assert!(failing("h1", &["rust-test"]).wake_legal());

        // Failing with nothing named: unclassified, not proven harmless.
        assert!(failing("h1", &[]).wake_legal());

        // The reading stays honest — this refuses the wake, it does not deny the
        // red. Status and `lf ci` still name the failure.
        let obs = failing("h1", &["scratch-clear"]);
        assert_eq!(obs.state, super::CiState::Failing);
        assert_eq!(obs.failure_set(), vec!["scratch-clear".to_string()]);
    }

    /// The land-resolved predicate is the exact dual of `wake_legal` within the
    /// failing state, so the two must never both refuse the same head.
    #[test]
    fn only_land_time_preconditions_is_the_dual_of_wake_legal() {
        // Red only on scratch-clear: land-resolved, and no wake.
        let scratch = failing("h1", &["scratch-clear"]);
        assert!(scratch.only_land_time_preconditions());
        assert!(!scratch.wake_legal());

        // A real leaf beside it (or alone): not land-resolved, wake arms.
        for obs in [
            failing("h1", &["scratch-clear", "rust-test"]),
            failing("h1", &["rust-test"]),
        ] {
            assert!(!obs.only_land_time_preconditions());
            assert!(obs.wake_legal());
        }

        // Unclassified (empty) failure: not "only preconditions", still wakes.
        let empty = failing("h1", &[]);
        assert!(!empty.only_land_time_preconditions());
        assert!(empty.wake_legal());

        // Passing/pending is never "only preconditions".
        let mut green = scratch.clone();
        green.state = super::CiState::Passing;
        assert!(!green.only_land_time_preconditions());
        let mut pending = scratch.clone();
        pending.state = super::CiState::Pending;
        assert!(!pending.only_land_time_preconditions());
    }

    /// The one literal, pinned against the workflow that defines it. A rename of
    /// the CI job turns this red *at the const* instead of silently re-arming
    /// wakes in production — the anti-rot guard for a name that cannot be derived.
    #[test]
    fn land_time_precondition_names_a_real_ci_job() {
        let workflow =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/ci.yml");
        let yaml = std::fs::read_to_string(&workflow)
            .unwrap_or_else(|e| panic!("read {}: {e}", workflow.display()));
        let job = format!("\n  {}:", super::LAND_TIME_PRECONDITION_CHECK);
        assert!(
            yaml.contains(&job),
            "no job named `{}` in {} — the const no longer names a real check, so \
             every design-carrying PR is arming ci-fix wakes again",
            super::LAND_TIME_PRECONDITION_CHECK,
            workflow.display()
        );
    }

    #[test]
    fn ci_fix_restart_bar_permits_only_a_failing_open_pr_wake() {
        let task = task(); // status Waiting, no abandon intent

        // Publication alone is not a restart bar.
        let published = open_pr("h1", None);
        assert!(task.supervisor_restart_bar(Some(&published)).is_none());
        assert!(task.ci_fix_restart_bar(Some(&published)).is_none());

        // Open PR, fresh failing head: the ci-fix wake is permitted where the plain
        // supervisor restart stays barred.
        let legal = with_merge_request(
            open_pr("h1", Some(failing("h1", &["build"]))),
            super::PrMergeMode::Auto,
        );
        assert!(task.supervisor_restart_bar(Some(&legal)).is_some());
        assert!(task.ci_fix_restart_bar(Some(&legal)).is_none());

        // Passing head → not legal → barred.
        let mut green_obs = failing("h1", &[]);
        green_obs.state = super::CiState::Passing;
        let green = with_merge_request(open_pr("h1", Some(green_obs)), super::PrMergeMode::Auto);
        assert!(task.ci_fix_restart_bar(Some(&green)).is_some());

        // Stale reading (observation head != PR head) → fresh_ci None → barred.
        let stale = with_merge_request(
            open_pr("h2", Some(failing("h1", &["build"]))),
            super::PrMergeMode::Auto,
        );
        assert!(task.ci_fix_restart_bar(Some(&stale)).is_some());

        // Red only on a land-time precondition → no repair exists → barred, and
        // the automated restart is the one path that could have overridden the
        // open-PR bar. A real leaf beside it still permits the wake.
        let land_only = with_merge_request(
            open_pr("h1", Some(failing("h1", &["scratch-clear"]))),
            super::PrMergeMode::Auto,
        );
        assert!(task.ci_fix_restart_bar(Some(&land_only)).is_some());
        let mixed = with_merge_request(
            open_pr("h1", Some(failing("h1", &["scratch-clear", "rust-test"]))),
            super::PrMergeMode::Auto,
        );
        assert!(task.ci_fix_restart_bar(Some(&mixed)).is_none());

        // The bar does not deduplicate. A head that already woke a body still reads
        // as legal here — refusing the second launch is the ledger's job, and
        // asking the question twice is what let the two answers drift.
        assert!(task.ci_fix_restart_bar(Some(&legal)).is_none());
    }

    #[test]
    fn task_rejects_impossible_lifecycle_and_writeback_state() {
        let mut task = task();
        task.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: "too early".to_string(),
        };
        assert!(task.validate().is_err());

        task.gate_cycle = 1;
        assert!(task.validate().is_ok());

        task.lifecycle.loop_.flow.clear();
        assert!(task.validate().is_err());
    }

    #[test]
    fn task_lifecycle_repeats_loop_and_finally_until_approval() {
        let mut task = task();
        task.lifecycle_phase = TaskLifecyclePhase::First;

        assert_eq!(task.lifecycle_cycle(), 0);
        task.enter_loop().unwrap();
        assert_eq!(task.lifecycle_phase, TaskLifecyclePhase::Loop);
        assert_eq!(task.lifecycle_cycle(), 1);
        assert_eq!(task.phase_epoch, 2);

        let proposal = TaskGateProposal {
            done: false,
            reason: "iteration needs another pass".to_string(),
        };
        task.phase_cursor = 2;
        task.phase_iteration = 3;
        task.enter_finally(proposal.clone()).unwrap();
        assert_eq!(task.lifecycle_phase, TaskLifecyclePhase::Finally);
        assert_eq!(task.lifecycle_cycle(), 1);
        assert_eq!(task.gate_cycle, 1);
        assert_eq!(task.approved_gate_proposal().unwrap(), proposal);
        assert_eq!((task.phase_cursor, task.phase_iteration), (0, 0));

        task.enter_loop().unwrap();
        assert_eq!(task.lifecycle_phase, TaskLifecyclePhase::Loop);
        assert_eq!(task.lifecycle_cycle(), 2);
        assert_eq!(task.gate_proposal, None);
        assert_eq!(task.phase_epoch, 4);
    }

    #[test]
    fn task_lifecycle_uses_public_names_without_rewriting_storage() {
        for (phase, public, stored) in [
            (TaskLifecyclePhase::First, "first", "kickoff"),
            (TaskLifecyclePhase::Loop, "loop", "iterate"),
            (TaskLifecyclePhase::Finally, "finally", "gate"),
        ] {
            assert_eq!(phase.as_str(), public);
            assert_eq!(phase.storage_str(), stored);
            assert_eq!(TaskLifecyclePhase::from_storage_str(stored).unwrap(), phase);
        }
    }

    #[test]
    fn standard_lifecycle_pins_each_phase_flow() {
        let plan = TaskLifecyclePlan::standard("task-design", "code", "ship");
        assert_eq!(plan.first.flow, "task-design");
        assert_eq!(plan.loop_.flow, "code");
        assert_eq!(plan.finally.flow, "ship");
    }
}
