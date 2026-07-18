//! Durable execution of one Linear Task.
//!
//! A Task Session owns one durable worktree and provider transcript. Ordered PRs
//! own the serial branches that advance the Task. Publication intent is recorded
//! before GitHub is called, then the GitHub receipt is attached to that intent.

use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child_session::{
    prefixed_uuid_id, AbandonIntent, ChildCommand, ChildCommandEffect, ChildCommandId,
    ChildCommandSource, ChildCommandState, ChildDecisionId, ChildDirective, ChildDirectiveId,
    ChildLeaseState, ChildProcessGeneration, DirectiveKind,
};
use crate::engine::InteractionPolicy;
use crate::id::WaveId;
use crate::interaction_review::{
    InteractionReview, InteractionReviewDisposition, InteractionReviewId,
    InteractionReviewMessageAuthor,
};
use crate::project_session::ProjectSessionId;
use crate::session_context::TaskLaunchReceipt;

pub mod actions;
pub(crate) mod interactive_rendezvous;
pub mod runner;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskDataError {
    #[error("invalid task id: {0}")]
    InvalidId(String),
    #[error("invalid task session status: {0}")]
    InvalidStatus(String),
    #[error("invalid task session: {0}")]
    InvalidInvariant(String),
}

prefixed_uuid_id!(
    TaskSessionId,
    "ts_",
    TaskDataError,
    TaskDataError::InvalidId
);

prefixed_uuid_id!(TaskPrId, "pr_", TaskDataError, TaskDataError::InvalidId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskSessionStatus {
    Created,
    Starting,
    Running,
    Waiting,
    Blocked,
    Failed,
    Completed,
    Abandoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskLifecyclePhase {
    Kickoff,
    Iterate,
    Gate,
}

impl TaskLifecyclePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kickoff => "kickoff",
            Self::Iterate => "iterate",
            Self::Gate => "gate",
        }
    }
}

impl FromStr for TaskLifecyclePhase {
    type Err = TaskDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "kickoff" => Ok(Self::Kickoff),
            "iterate" => Ok(Self::Iterate),
            "gate" => Ok(Self::Gate),
            _ => Err(TaskDataError::InvalidInvariant(format!(
                "invalid Task lifecycle phase: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPhasePlan {
    pub flow: String,
    pub interaction_policy: InteractionPolicy,
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
    pub kickoff: TaskPhasePlan,
    pub iterate: TaskPhasePlan,
    pub gate: TaskPhasePlan,
}

impl TaskLifecyclePlan {
    pub fn standard(iterate_flow: impl Into<String>) -> Self {
        Self {
            kickoff: TaskPhasePlan {
                flow: "task-kickoff".to_string(),
                interaction_policy: InteractionPolicy::Require,
            },
            iterate: TaskPhasePlan {
                flow: iterate_flow.into(),
                interaction_policy: InteractionPolicy::Defer,
            },
            gate: TaskPhasePlan {
                flow: "task-gate".to_string(),
                interaction_policy: InteractionPolicy::Require,
            },
        }
    }

    pub fn headless(iterate_flow: impl Into<String>) -> Self {
        let mut plan = Self::standard(iterate_flow);
        plan.defer_all_interactions();
        plan
    }

    pub fn defer_all_interactions(&mut self) {
        self.kickoff.interaction_policy = InteractionPolicy::Defer;
        self.iterate.interaction_policy = InteractionPolicy::Defer;
        self.gate.interaction_policy = InteractionPolicy::Defer;
    }

    pub fn all_interactions_deferred(&self) -> bool {
        [
            self.kickoff.interaction_policy,
            self.iterate.interaction_policy,
            self.gate.interaction_policy,
        ]
        .into_iter()
        .all(|policy| policy == InteractionPolicy::Defer)
    }

    pub fn phase(&self, phase: TaskLifecyclePhase) -> &TaskPhasePlan {
        match phase {
            TaskLifecyclePhase::Kickoff => &self.kickoff,
            TaskLifecyclePhase::Iterate => &self.iterate,
            TaskLifecyclePhase::Gate => &self.gate,
        }
    }

    fn validate(&self) -> Result<(), TaskDataError> {
        self.kickoff.validate(TaskLifecyclePhase::Kickoff)?;
        self.iterate.validate(TaskLifecyclePhase::Iterate)?;
        self.gate.validate(TaskLifecyclePhase::Gate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGateProposal {
    pub status: TaskSessionStatus,
    pub reason: String,
}

impl TaskGateProposal {
    fn validate(&self) -> Result<(), TaskDataError> {
        if self.status.is_process_active() || self.status == TaskSessionStatus::Created {
            return Err(TaskDataError::InvalidInvariant(format!(
                "{} is not a gate-settle outcome",
                self.status.as_str()
            )));
        }
        if self.reason.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(
                "gate proposal reason cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

impl TaskSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }

    pub fn is_process_active(self) -> bool {
        matches!(self, Self::Starting | Self::Running)
    }

    /// Coarsen durable intent to the shared shape the body projection reads, so
    /// one `observe` serves Project and Task Sessions alike.
    pub fn body_intent(self) -> crate::child_session::BodyIntent {
        use crate::child_session::BodyIntent;
        match self {
            Self::Created | Self::Starting | Self::Running => BodyIntent::Active,
            Self::Waiting => BodyIntent::Waiting,
            Self::Blocked => BodyIntent::Blocked,
            Self::Failed => BodyIntent::Failed,
            Self::Completed | Self::Abandoned => BodyIntent::Terminal,
        }
    }
}

impl FromStr for TaskSessionStatus {
    type Err = TaskDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "created" => Ok(Self::Created),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "completed" => Ok(Self::Completed),
            "abandoned" => Ok(Self::Abandoned),
            _ => Err(TaskDataError::InvalidStatus(value.to_string())),
        }
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
    /// during kickoff and iterate, by construction — and
    /// `crate::ops::land::clear_scratch` is what greens it, not a code change. A
    /// body woken to "repair" it could only delete the artifact the reviewer
    /// reads, to green a check land greens anyway.
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
    /// `ChildCommandKind::CiFix` ledger, keyed on the incident identity. Those
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
    /// greens it by clearing `scratch/`), so it is *reviewable* rather than owned
    /// by CI. The action model and Waves supervision read it to stop recommending
    /// a doomed Resume and to stop labelling the reviewer's turn as "fixing CI".
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
    pub task_session_id: TaskSessionId,
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
    pub trigger_command_id: Option<ChildCommandId>,
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
    Review,
    CompleteTask,
}

impl AfterMerge {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Review => "review",
            Self::CompleteTask => "complete_task",
        }
    }
}

impl FromStr for AfterMerge {
    type Err = TaskDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "review" => Ok(Self::Review),
            "complete_task" => Ok(Self::CompleteTask),
            _ => Err(TaskDataError::InvalidInvariant(format!(
                "invalid after-merge disposition: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrPublication {
    pub requested_at: OffsetDateTime,
    pub after_merge: AfterMerge,
    pub next_slug: Option<String>,
    pub github: Option<GithubPr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPr {
    pub id: TaskPrId,
    pub task_session_id: TaskSessionId,
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

    /// The receipt-resolution identity for this PR: its repository, number, and
    /// the commit sha(s) it carries. `None` until the PR has a GitHub identity
    /// (an `owner/repo` URL and number). A `pr:` receipt resolves against this.
    pub fn pr_identity(&self) -> Option<crate::receipt::PrIdentity> {
        let github = self.github()?;
        let repo = crate::receipt::github_repo_slug(&github.url)?;
        let shas = self
            .merge_commit
            .iter()
            .chain(github.head_sha.iter())
            .cloned()
            .collect();
        Some(crate::receipt::PrIdentity {
            repo,
            number: github.number,
            shas,
        })
    }

    /// The current head SHA of the open PR, when recorded.
    pub fn head_sha(&self) -> Option<&str> {
        self.github().and_then(|github| github.head_sha.as_deref())
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

    /// Whether semantic Task review may begin for this published head.
    pub fn review_ready(&self) -> bool {
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
            if publication.next_slug.as_deref().is_some_and(|slug| {
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
            if publication.after_merge == AfterMerge::CompleteTask
                && publication.next_slug.is_some()
            {
                return Err(TaskDataError::InvalidInvariant(
                    "a completing pull request cannot name a next branch".to_string(),
                ));
            }
            if let Some(github) = &publication.github {
                if github.number == 0 || github.url.trim().is_empty() {
                    return Err(TaskDataError::InvalidInvariant(
                        "GitHub PR number and URL cannot be empty".to_string(),
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
    /// Repair: the Task was prematurely completed in the PM while its gates were
    /// still open. Reopen the PM issue so the PM row reconverges with the
    /// Session, PR, and review state.
    ReopenTask,
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
pub struct TaskSession {
    pub id: TaskSessionId,
    /// Immutable PM evidence captured before placement.
    pub launch: TaskLaunchReceipt,
    pub pm_writeback: PmWritebackState,
    /// Root ownership. Wave name and checkout are resolved from this id.
    pub wave_id: WaveId,
    /// Required runtime parent. Every Task reports through one durable Project
    /// Session; its Wave retains root inspection and override authority.
    pub project_session_id: ProjectSessionId,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: TaskSessionStatus,
    pub status_reason: String,
    pub status_at: OffsetDateTime,
    pub worktree: PathBuf,
    pub workspace_slug: String,
    /// Three pinned phase flows and their reviewer-routing policies.
    pub lifecycle: TaskLifecyclePlan,
    /// Current phase entry. `phase_epoch` advances on every transition,
    /// including Gate → Iterate, so stale bodies cannot rewind the Session.
    pub lifecycle_phase: TaskLifecyclePhase,
    pub phase_epoch: u32,
    pub phase_cursor: u32,
    pub phase_iteration: u32,
    /// Number of Gate entries attempted by this Task.
    pub gate_cycle: u32,
    /// Outcome proposed by Iterate and awaiting Gate approval.
    pub gate_proposal: Option<TaskGateProposal>,
    /// Provider/model selection for the next body generation. This is mutable
    /// lease state, not Task Session identity.
    pub agent: String,
    /// Harness family for the next/current body generation.
    pub provider: String,
    /// Transcript handle reusable only by a compatible provider generation.
    pub provider_session_id: Option<String>,
    /// Latest launch generation, retained after that process exits. Its
    /// [`crate::child_session::BinaryProvenance`] is the audit record of which
    /// lf launched it — a Session no longer pins a binary of its own.
    pub latest_process: Option<ChildProcessGeneration>,
    /// Set when abandonment is *requested*, not when it is applied. No launch
    /// path may start a process for a Session carrying this.
    pub abandon_intent: Option<AbandonIntent>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    /// Per-command view derived from the active PR's durable observation cache.
    #[serde(skip)]
    pub observation: Observation,
}

impl TaskSession {
    /// Why a supervisor must not start another process generation, if it must not.
    ///
    /// Three intents are terminal for *automatic* restart, and only the first is
    /// terminal for the Session itself:
    ///
    /// - the Session is `Completed` or `Abandoned` — the work is over;
    /// - abandonment has been *requested* — the runner has not consumed the
    ///   command yet, but the decision is made;
    /// - the active PR is `Publishing` or `Open` — publication was requested
    ///   and the work must not restart from clarification
    ///   and belongs to review.
    ///
    /// The third was the 2026-07-14 W2-129 failure: `Open` is not terminal
    /// and carries no live process, so it reads exactly like a Session that
    /// merely stopped. A wake therefore launched generation 2, which reopened
    /// the flow at `task_clarify` and began re-doing work whose PR (#878) was
    /// already open for review. An open PR is not an invitation to start over.
    ///
    /// A human may still `lf task resume` a submitted Session to answer review;
    /// this bars the supervisor, not the operator.
    pub fn supervisor_restart_bar(&self, active_pr: Option<&TaskPr>) -> Option<String> {
        if let Some(bar) = self.terminal_or_abandon_bar() {
            return Some(bar);
        }
        if let Some(pr) = active_pr {
            match pr.phase() {
                PrPhase::Publishing => return Some(self.publishing_bar()),
                PrPhase::Open => return Some(self.open_pr_bar(pr)),
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
    /// The bar answers legality only. "Have we already woken for this failure?"
    /// is the command ledger's question, answered once by the incident identity
    /// on `ChildCommandKind::CiFix` — not asked again here.
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
                PrPhase::Open if !pr.fresh_ci().is_some_and(CiObservation::wake_legal) => {
                    return Some(self.open_pr_bar(pr));
                }
                PrPhase::Open | PrPhase::Working | PrPhase::Merged | PrPhase::Abandoned => {}
            }
        }
        None
    }

    /// The two bars that hold for *every* automatic restart intent: the work is
    /// over, or its abandonment is already decided.
    pub(crate) fn terminal_or_abandon_bar(&self) -> Option<String> {
        if self.status.is_terminal() {
            return Some(format!(
                "Task {} is {}; terminal Task Sessions do not restart",
                self.launch.issue.identifier,
                self.status.as_str()
            ));
        }
        if let Some(intent) = &self.abandon_intent {
            return Some(format!(
                "Task {} is being abandoned: {}",
                self.launch.issue.identifier, intent.reason
            ));
        }
        None
    }

    fn publishing_bar(&self) -> String {
        format!(
            "Task {} requested PR publication but has no GitHub receipt; \
             resume it explicitly with `lf task resume {}` to retry publication",
            self.launch.issue.identifier, self.launch.issue.identifier,
        )
    }

    /// The open-PR restart refusal. Only a *supervisor* (`supervisor_restart_bar`
    /// / a non-wake-legal `ci_fix_restart_bar`) ever reads this — an operator
    /// resume takes the abandon-only `ExplicitResume` bar and answers review — so
    /// the text names the real next owner (the reviewer/operator) instead of
    /// recommending `lf task resume`, which a supervisor re-running would only
    /// self-loop on.
    fn open_pr_bar(&self, pr: &TaskPr) -> String {
        let number = pr.github().expect("open Task PR passed validation").number;
        format!(
            "Task {} submitted pull request #{} and is in review. A supervisor \
             cannot restart a submitted Task — an open PR is not an invitation to \
             start over. This is the reviewer's to advance: an operator answering \
             review resumes from a clean operator shell; if review is blocked, \
             escalate to the owner.",
            self.launch.issue.identifier, number,
        )
    }

    pub fn validate(&self) -> Result<(), TaskDataError> {
        if self.status.is_process_active() && self.latest_process.is_none() {
            return Err(TaskDataError::InvalidInvariant(format!(
                "{} requires a latest process generation",
                self.status.as_str()
            )));
        }
        if self.workspace_slug.trim().is_empty() {
            return Err(TaskDataError::InvalidInvariant(format!(
                "Task Session {} requires a workspace slug",
                self.id
            )));
        }
        self.lifecycle.validate()?;
        if self.phase_epoch == 0 {
            return Err(TaskDataError::InvalidInvariant(
                "Task lifecycle phase epoch must be positive".to_string(),
            ));
        }
        if self.lifecycle_phase == TaskLifecyclePhase::Gate && self.gate_proposal.is_none() {
            return Err(TaskDataError::InvalidInvariant(
                "Task gate phase requires a proposed outcome".to_string(),
            ));
        }
        if self.lifecycle_phase != TaskLifecyclePhase::Gate && self.gate_proposal.is_some() {
            return Err(TaskDataError::InvalidInvariant(
                "Task gate proposal is valid only during gate phase".to_string(),
            ));
        }
        if let Some(proposal) = &self.gate_proposal {
            proposal.validate()?;
        }
        if let PmWritebackState::Pending { operation, .. } = &self.pm_writeback {
            match operation {
                PmWritebackOperation::CompleteTask => {
                    if self.status != TaskSessionStatus::Completed && self.gate_cycle == 0 {
                        return Err(TaskDataError::InvalidInvariant(
                            "pending PM completion requires a completed Task or an active gate cycle"
                                .to_string(),
                        ));
                    }
                }
                PmWritebackOperation::ReopenTask => {
                    if self.status == TaskSessionStatus::Completed {
                        return Err(TaskDataError::InvalidInvariant(
                            "pending PM reopen requires the Task to no longer be completed"
                                .to_string(),
                        ));
                    }
                }
            }
        }
        if self.incorporated_directive_version > self.current_directive_version {
            return Err(TaskDataError::InvalidInvariant(
                "incorporated directive version exceeds current direction".to_string(),
            ));
        }
        Ok(())
    }

    pub fn set_status(&mut self, status: TaskSessionStatus, reason: impl Into<String>) {
        let now = OffsetDateTime::now_utc();
        self.status = status;
        self.status_reason = reason.into();
        self.status_at = now;
        self.updated_at = now;
    }

    pub fn begin_generation(&mut self, tmux_name: String) -> u32 {
        let generation = self
            .latest_process
            .as_ref()
            .map_or(1, |process| process.generation + 1);
        let now = OffsetDateTime::now_utc();
        self.latest_process = Some(ChildProcessGeneration {
            generation,
            pid: None,
            process_group_id: None,
            tmux_name,
            agent: self.agent.clone(),
            provider: self.provider.clone(),
            provider_session_id: self.provider_session_id.clone(),
            started_at: now,
            state: ChildLeaseState::Reserved,
            outcome: None,
            provenance: None,
        });
        self.set_status(TaskSessionStatus::Starting, "task process is starting");
        generation
    }

    pub fn phase_plan(&self) -> &TaskPhasePlan {
        self.lifecycle.phase(self.lifecycle_phase)
    }

    pub fn lifecycle_cycle(&self) -> u32 {
        match self.lifecycle_phase {
            TaskLifecyclePhase::Kickoff => 0,
            TaskLifecyclePhase::Iterate => self.gate_cycle + 1,
            TaskLifecyclePhase::Gate => self.gate_cycle,
        }
    }

    pub fn enter_iterate(&mut self) -> Result<(), TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::Kickoff
            && self.lifecycle_phase != TaskLifecyclePhase::Gate
        {
            return Err(TaskDataError::InvalidInvariant(
                "only kickoff or gate may enter iterate".to_string(),
            ));
        }
        self.lifecycle_phase = TaskLifecyclePhase::Iterate;
        self.phase_epoch += 1;
        self.phase_cursor = 0;
        self.phase_iteration = 0;
        self.gate_proposal = None;
        self.updated_at = OffsetDateTime::now_utc();
        Ok(())
    }

    pub fn enter_gate(&mut self, proposal: TaskGateProposal) -> Result<(), TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::Iterate {
            return Err(TaskDataError::InvalidInvariant(
                "only iterate may enter gate".to_string(),
            ));
        }
        proposal.validate()?;
        self.lifecycle_phase = TaskLifecyclePhase::Gate;
        self.phase_epoch += 1;
        self.phase_cursor = 0;
        self.phase_iteration = 0;
        self.gate_cycle += 1;
        self.gate_proposal = Some(proposal);
        self.updated_at = OffsetDateTime::now_utc();
        Ok(())
    }

    pub fn approved_gate_proposal(&self) -> Result<TaskGateProposal, TaskDataError> {
        if self.lifecycle_phase != TaskLifecyclePhase::Gate {
            return Err(TaskDataError::InvalidInvariant(
                "only gate may approve a proposed outcome".to_string(),
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
        handoff: crate::child_session::ChildBodyHandoff,
    },
    BodyLeaseChanged {
        process: ChildProcessGeneration,
    },
    /// Loopflow re-dispatched a Session whose body died, with no human asking.
    /// The durable record of a recovery: `attempt` bounds the retry and this
    /// event is what [`crate::child_session::count_recovery_attempts`] counts.
    BodyRecoveryAttempted {
        generation: u32,
        attempt: u32,
        reason: String,
    },
    StatusChanged {
        from: TaskSessionStatus,
        to: TaskSessionStatus,
        reason: String,
    },
    CommandChanged {
        command_id: ChildCommandId,
        state: ChildCommandState,
        effect: Option<ChildCommandEffect>,
        error: Option<String>,
    },
    DirectiveChanged {
        directive_id: ChildDirectiveId,
        version: u32,
        directive_kind: DirectiveKind,
    },
    DirectiveIncorporated {
        directive_id: ChildDirectiveId,
        version: u32,
        summary: String,
    },
    /// An out-of-band Wave/Operator attestation that closed a merged, complete
    /// Task's applied-but-unincorporated final directive. Distinct from
    /// `DirectiveIncorporated`, which is the body's own acknowledgement — this
    /// records who attested the semantic handoff from outside the Session, so
    /// the trail never reads as a forged body acknowledgement.
    DirectiveReconciled {
        directive_id: ChildDirectiveId,
        version: u32,
        summary: String,
        attested_by: ChildCommandSource,
    },
    DecisionRequested {
        decision_id: ChildDecisionId,
        prompt: String,
        options: Vec<String>,
    },
    DecisionResolved {
        decision_id: ChildDecisionId,
        choice: String,
        message: Option<String>,
    },
    Progress {
        summary: String,
    },
    InteractionReviewRequested {
        review: Box<InteractionReview>,
    },
    InteractionReviewMessage {
        review_id: InteractionReviewId,
        author: InteractionReviewMessageAuthor,
        text: String,
    },
    InteractionReviewCompleted {
        review_id: InteractionReviewId,
        disposition: InteractionReviewDisposition,
        outcome: String,
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
        match self {
            Self::Started | Self::Progress { .. } => false,
            Self::InteractionReviewMessage {
                author: InteractionReviewMessageAuthor::Reviewer,
                ..
            }
            | Self::InteractionReviewCompleted { .. } => false,
            Self::BodyLeaseChanged { process } => matches!(
                process.state,
                ChildLeaseState::Revoked | ChildLeaseState::Finished
            ),
            Self::CommandChanged { state, .. } => state.is_terminal(),
            _ => true,
        }
    }

    /// Whether a Project-supervised Task event also belongs in the root Wave.
    /// Routine decisions stay at the immediate Project boundary; a Project
    /// escalates by emitting its own `DecisionRequested` event.
    pub fn is_root_wave_observable(&self) -> bool {
        self.is_project_observable()
            && !matches!(
                self,
                Self::DecisionRequested { .. }
                    | Self::DecisionResolved { .. }
                    | Self::InteractionReviewRequested { .. }
                    | Self::InteractionReviewMessage { .. }
                    | Self::InteractionReviewCompleted { .. }
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub id: i64,
    pub session_id: TaskSessionId,
    pub kind: TaskEventKind,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskObservation {
    pub session_id: TaskSessionId,
    pub issue_identifier: String,
    pub event_id: i64,
    pub control_source: Option<crate::child_session::ChildCommandSource>,
    pub event: TaskEventKind,
}

impl TaskObservation {
    pub fn inbox_id(&self) -> String {
        format!("task-{}-{}", self.session_id, self.event_id)
    }

    pub fn prompt(&self) -> String {
        let payload = serde_json::to_string(&serde_json::json!({
            "control_source": self.control_source,
            "event": self.event,
        }))
        .expect("Task observation always serializes to structured JSON");
        format!(
            "<task_observation session_id=\"{}\" issue=\"{}\" event_id=\"{}\">\n{}\n</task_observation>",
            self.session_id, self.issue_identifier, self.event_id, payload
        )
    }
}

/// The durable cursor for streaming human Linear edits into one Task Session.
/// It is the exactly-once ledger — what issue revision and comments have already
/// become Task direction — plus the health of the last observation, so
/// `lf task status` can show stale reads and their degraded reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLinearObservation {
    pub session_id: TaskSessionId,
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
    pub session_id: TaskSessionId,
    pub revision: String,
    pub title: String,
    pub description: String,
    pub observed_at: OffsetDateTime,
    /// A title/description edit to persist: the Steer command and its
    /// replacement directive. `None` when title and description are unchanged.
    pub directive: Option<(ChildCommand, ChildDirective)>,
    /// Human comments observed this pass, oldest first.
    pub follow_ups: Vec<LinearFollowUp>,
}

#[derive(Debug, Clone)]
pub struct LinearFollowUp {
    pub comment_id: String,
    pub command: ChildCommand,
}

/// What one [`LinearObservationApply`] actually wrote — enough for the caller to
/// report receipts without re-reading the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearObservationOutcome {
    /// The Session had no cursor yet: this observation seeded the baseline and
    /// emitted no direction (existing comments are marked seen, not replayed).
    pub baselined: bool,
    pub directive_applied: bool,
    pub superseded: Vec<ChildCommandId>,
    pub follow_ups_created: Vec<ChildCommandId>,
}

/// The result of carrying a terminal Task Session's direction onto a successor.
///
/// `created` is `true` when this call inserted the successor and re-keyed the
/// Linear observation cursor and ingested-comment ledger onto it; `false` when a
/// non-terminal successor already existed (a concurrent or retried run), in
/// which case the carry transaction was a no-op and the existing Session is
/// returned. Historical receipts (`child_commands`, `child_directives`) stay on
/// the predecessor for attribution in both cases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSessionSuccession {
    pub session: TaskSession,
    pub created: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        AfterMerge, ChildCommandId, ChildCommandState, GithubPr, PmWritebackOperation,
        PmWritebackState, PrPhase, PrPublication, TaskEventKind, TaskGateProposal,
        TaskLifecyclePhase, TaskLifecyclePlan, TaskObservation, TaskPr, TaskPrId, TaskSession,
        TaskSessionId, TaskSessionStatus,
    };
    use crate::child_session::ChildDecisionId;
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        TaskLaunchReceipt,
    };

    fn task_session() -> TaskSession {
        let now = time::OffsetDateTime::now_utc();
        TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-1").unwrap(),
                    identifier: "INF-123".to_string(),
                    title: "Ship it".to_string(),
                    description: String::new(),
                },
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new("project-1").unwrap(),
                    slug: "runtime".to_string(),
                    name: "Runtime".to_string(),
                    prompt_context: "Definition".to_string(),
                },
                pm_snapshot_synced_at: 1,
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_session_id: crate::project_session::ProjectSessionId::new(),
            current_directive_version: 1,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Created,
            status_reason: "created".to_string(),
            status_at: now,
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
            lifecycle: TaskLifecyclePlan::standard("task"),
            lifecycle_phase: TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        }
    }

    #[test]
    fn task_ids_are_prefixed_and_round_trip() {
        let session = TaskSessionId::new();
        assert_eq!(TaskSessionId::parse(session.as_str()).unwrap(), session);
        let pr = TaskPrId::new();
        assert_eq!(TaskPrId::parse(pr.as_str()).unwrap(), pr);
    }

    #[test]
    fn task_observation_has_a_stable_structured_inbox_identity() {
        let observation = TaskObservation {
            session_id: TaskSessionId::from_raw("ts_example"),
            issue_identifier: "INF-123".to_string(),
            event_id: 42,
            control_source: None,
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
    fn only_completed_and_abandoned_tasks_are_terminal() {
        assert!(TaskSessionStatus::Completed.is_terminal());
        assert!(TaskSessionStatus::Abandoned.is_terminal());
        assert!(!TaskSessionStatus::Waiting.is_terminal());
        assert!(!TaskSessionStatus::Failed.is_terminal());
    }

    #[test]
    fn project_observes_command_outcomes_not_transport_chatter() {
        let event = |state| TaskEventKind::CommandChanged {
            command_id: ChildCommandId::new(),
            state,
            effect: None,
            error: None,
        };

        assert!(!event(ChildCommandState::Persisted).is_project_observable());
        assert!(!event(ChildCommandState::Claimed).is_project_observable());
        assert!(!event(ChildCommandState::Delivering).is_project_observable());
        assert!(event(ChildCommandState::Accepted).is_project_observable());
        assert!(event(ChildCommandState::Failed).is_project_observable());
        assert!(event(ChildCommandState::Uncertain).is_project_observable());
    }

    #[test]
    fn project_supervision_keeps_routine_task_decisions_out_of_the_root_wave() {
        let requested = TaskEventKind::DecisionRequested {
            decision_id: ChildDecisionId::new(),
            prompt: "Which parser shape?".to_string(),
            options: vec!["strict".to_string(), "permissive".to_string()],
        };
        let resolved = TaskEventKind::DecisionResolved {
            decision_id: ChildDecisionId::new(),
            choice: "strict".to_string(),
            message: None,
        };

        assert!(requested.is_project_observable());
        assert!(resolved.is_project_observable());
        assert!(!requested.is_root_wave_observable());
        assert!(!resolved.is_root_wave_observable());
        assert!(TaskEventKind::Failed {
            error: "provider stopped".to_string(),
            resumable: true,
        }
        .is_root_wave_observable());
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
            task_session_id: TaskSessionId::new(),
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
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: None,
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
    fn publication_contains_its_github_receipt_and_disposition() {
        let now = time::OffsetDateTime::now_utc();
        let mut pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: TaskSessionId::new(),
            sequence: 1,
            slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                after_merge: AfterMerge::Review,
                next_slug: Some("released_upgrade".to_string()),
                github: Some(GithubPr {
                    number: 872,
                    url: "https://github.com/loopflowstudio/loopflow/pull/872".to_string(),
                    head_sha: None,
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

        let publication = pr.publication.as_mut().unwrap();
        publication.next_slug = Some("released-upgrade".to_string());
        assert!(pr.validate().is_ok());

        pr.publication.as_mut().unwrap().after_merge = AfterMerge::CompleteTask;
        assert!(pr.validate().is_err());
    }

    #[test]
    fn pr_identity_maps_repo_number_and_carried_shas() {
        let mut pr = open_pr("headsha", None);
        let open = pr.pr_identity().expect("open PR has an identity");
        assert_eq!(open.repo, "loopflow/loopflow");
        assert_eq!(open.number, 900);
        assert_eq!(open.shas, vec!["headsha".to_string()]);

        // A merged PR resolves against both its merge commit and its last head,
        // so a claim pinned to either sha still drills.
        pr.merge_commit = Some("mergesha".to_string());
        let merged = pr.pr_identity().expect("merged PR has an identity");
        assert_eq!(
            merged.shas,
            vec!["mergesha".to_string(), "headsha".to_string()]
        );

        // No GitHub identity yet (still working) → no receipt identity to match.
        let working = TaskPr {
            publication: None,
            ..pr
        };
        assert_eq!(working.pr_identity(), None);
    }

    fn open_pr(head_sha: &str, observation: Option<super::CiObservation>) -> TaskPr {
        let now = time::OffsetDateTime::now_utc();
        TaskPr {
            id: TaskPrId::new(),
            task_session_id: TaskSessionId::new(),
            sequence: 1,
            slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "abc".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                after_merge: AfterMerge::Review,
                next_slug: None,
                github: Some(GithubPr {
                    number: 900,
                    url: "https://github.com/loopflow/loopflow/pull/900".to_string(),
                    head_sha: Some(head_sha.to_string()),
                }),
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
    fn review_ready_requires_current_head_passing_checks() {
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
        .review_ready());
        assert!(!open_pr(
            "current",
            Some(observation("current", super::CiState::Pending))
        )
        .review_ready());
        assert!(
            !open_pr("current", Some(observation("old", super::CiState::Passing))).review_ready()
        );
        assert!(!open_pr("current", None).review_ready());
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
    /// could take is deleting the design doc under review.
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

    /// The reviewable-despite-red predicate the action model and Waves
    /// supervision adopt: it is the exact dual of `wake_legal` within the failing
    /// state, so the two must never both refuse the same head.
    #[test]
    fn only_land_time_preconditions_is_the_dual_of_wake_legal() {
        // Red only on scratch-clear: reviewable, and no wake.
        let scratch = failing("h1", &["scratch-clear"]);
        assert!(scratch.only_land_time_preconditions());
        assert!(!scratch.wake_legal());

        // A real leaf beside it (or alone): not reviewable, wake arms.
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
        let session = task_session(); // status Waiting, no abandon intent

        // Open PR, fresh failing head: the ci-fix wake is permitted where the plain
        // supervisor restart stays barred.
        let legal = open_pr("h1", Some(failing("h1", &["build"])));
        assert!(session.supervisor_restart_bar(Some(&legal)).is_some());
        assert!(session.ci_fix_restart_bar(Some(&legal)).is_none());

        // Passing head → not legal → barred.
        let mut green_obs = failing("h1", &[]);
        green_obs.state = super::CiState::Passing;
        let green = open_pr("h1", Some(green_obs));
        assert!(session.ci_fix_restart_bar(Some(&green)).is_some());

        // Stale reading (observation head != PR head) → fresh_ci None → barred.
        let stale = open_pr("h2", Some(failing("h1", &["build"])));
        assert!(session.ci_fix_restart_bar(Some(&stale)).is_some());

        // Red only on a land-time precondition → no repair exists → barred, and
        // the automated restart is the one path that could have overridden the
        // open-PR bar. A real leaf beside it still permits the wake.
        let land_only = open_pr("h1", Some(failing("h1", &["scratch-clear"])));
        assert!(session.ci_fix_restart_bar(Some(&land_only)).is_some());
        let mixed = open_pr("h1", Some(failing("h1", &["scratch-clear", "rust-test"])));
        assert!(session.ci_fix_restart_bar(Some(&mixed)).is_none());

        // Terminal intent dominates even a legal wake.
        let mut terminal = task_session();
        terminal.status = TaskSessionStatus::Completed;
        assert!(terminal.ci_fix_restart_bar(Some(&legal)).is_some());

        // The bar does not deduplicate. A head that already woke a body still reads
        // as legal here — refusing the second launch is the ledger's job, and
        // asking the question twice is what let the two answers drift.
        assert!(session.ci_fix_restart_bar(Some(&legal)).is_none());
    }

    #[test]
    fn task_session_rejects_impossible_process_and_writeback_state() {
        let mut session = task_session();
        session.status = TaskSessionStatus::Running;
        assert!(session.validate().is_err());

        session.status = TaskSessionStatus::Waiting;
        session.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: "too early".to_string(),
        };
        assert!(session.validate().is_err());

        session.status = TaskSessionStatus::Completed;
        assert!(session.validate().is_ok());

        session.lifecycle.iterate.flow.clear();
        assert!(session.validate().is_err());
    }

    #[test]
    fn pending_reopen_writeback_requires_an_uncompleted_task() {
        let mut session = task_session();
        session.status = TaskSessionStatus::Completed;
        session.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::ReopenTask,
            error: "premature completion".to_string(),
        };
        assert!(session.validate().is_err());

        session.status = TaskSessionStatus::Waiting;
        assert!(session.validate().is_ok());
    }

    #[test]
    fn pending_reopen_writeback_has_a_stable_json_shape() {
        let state = PmWritebackState::Pending {
            operation: PmWritebackOperation::ReopenTask,
            error: "premature completion".to_string(),
        };

        assert_eq!(
            serde_json::to_value(state).unwrap(),
            serde_json::json!({
                "state": "pending",
                "operation": "reopen_task",
                "error": "premature completion"
            })
        );
    }

    #[test]
    fn task_lifecycle_repeats_iterate_and_gate_until_approval() {
        let mut session = task_session();
        session.lifecycle_phase = TaskLifecyclePhase::Kickoff;

        assert_eq!(session.lifecycle_cycle(), 0);
        session.enter_iterate().unwrap();
        assert_eq!(session.lifecycle_phase, TaskLifecyclePhase::Iterate);
        assert_eq!(session.lifecycle_cycle(), 1);
        assert_eq!(session.phase_epoch, 2);

        let proposal = TaskGateProposal {
            status: TaskSessionStatus::Waiting,
            reason: "pull request is ready for review".to_string(),
        };
        session.phase_cursor = 2;
        session.phase_iteration = 3;
        session.enter_gate(proposal.clone()).unwrap();
        assert_eq!(session.lifecycle_phase, TaskLifecyclePhase::Gate);
        assert_eq!(session.lifecycle_cycle(), 1);
        assert_eq!(session.gate_cycle, 1);
        assert_eq!(session.approved_gate_proposal().unwrap(), proposal);
        assert_eq!((session.phase_cursor, session.phase_iteration), (0, 0));

        session.enter_iterate().unwrap();
        assert_eq!(session.lifecycle_phase, TaskLifecyclePhase::Iterate);
        assert_eq!(session.lifecycle_cycle(), 2);
        assert_eq!(session.gate_proposal, None);
        assert_eq!(session.phase_epoch, 4);
    }

    #[test]
    fn headless_policy_defers_every_phase_without_changing_its_flows() {
        let mut plan = TaskLifecyclePlan::standard("code");
        assert!(!plan.all_interactions_deferred());

        plan.defer_all_interactions();

        assert!(plan.all_interactions_deferred());
        assert_eq!(plan.kickoff.flow, "task-kickoff");
        assert_eq!(plan.iterate.flow, "code");
        assert_eq!(plan.gate.flow, "task-gate");
        assert_eq!(plan, TaskLifecyclePlan::headless("code"));
    }
}
