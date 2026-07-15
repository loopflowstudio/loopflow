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
    ChildCommandState, ChildDecisionId, ChildDirective, ChildDirectiveId, ChildLeaseState,
    ChildProcessGeneration, DirectiveKind,
};
use crate::engine::InteractionPolicy;
use crate::id::WaveId;
use crate::interaction_review::{
    InteractionReview, InteractionReviewDisposition, InteractionReviewId,
    InteractionReviewMessageAuthor,
};
use crate::project_session::ProjectSessionId;
use crate::session_context::TaskLaunchReceipt;

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

/// One required check that is not passing, named so the `ci-fix` skill can
/// resolve the exact failure from its logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiCheck {
    pub name: String,
    pub url: Option<String>,
}

/// The required-check reading for one PR head. `head_sha` pins it: a reading is
/// stale — and never wakes work — once the PR's head moves past it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiObservation {
    pub head_sha: String,
    pub state: CiState,
    pub failing_checks: Vec<CiCheck>,
    pub observed_at: OffsetDateTime,
    /// The failing-check set a `ci-fix` wake has already fired for at this head,
    /// sorted. `None` until a wake fires. This is the dedup key: a wake is
    /// warranted only when the current failing set differs from it, so a repeated
    /// poll or a coalesced multi-check delivery never wakes a second body. The
    /// marker rides this JSON column — absent on readings written before the wake
    /// landed, hence `serde(default)` rather than a migration.
    #[serde(default)]
    pub woken_failure_set: Option<Vec<String>>,
}

impl CiObservation {
    /// The current failing required checks by name, sorted — the dedup key's
    /// content half. Empty unless `state` is `Failing`.
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

    /// Whether this reading warrants a `ci-fix` wake: the head is failing and no
    /// wake has fired for this exact failing set yet. `false` once a wake for the
    /// same set is recorded (repeat poll / coalesced delivery) — the wake is
    /// re-armed only when the head moves (a fresh reading) or the failing set
    /// changes.
    pub fn wake_warranted(&self) -> bool {
        self.state == CiState::Failing
            && self.woken_failure_set.as_deref() != Some(self.failure_set().as_slice())
    }

    /// Record that a wake fired for the current failing set, so the next reading
    /// with the same `(head, failing set)` does not wake again.
    pub fn mark_woken(&mut self) {
        self.woken_failure_set = Some(self.failure_set());
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
    /// failing required check the wake has not yet fired for
    /// ([`CiObservation::wake_warranted`]). This is the one automated path allowed
    /// to restart a submitted Task, and only on fresh current-head failure
    /// evidence — never a blind wake over passing, pending, or already-woken work,
    /// and never past the terminal, abandon, or publishing bars.
    pub fn ci_fix_restart_bar(&self, active_pr: Option<&TaskPr>) -> Option<String> {
        if let Some(bar) = self.terminal_or_abandon_bar() {
            return Some(bar);
        }
        if let Some(pr) = active_pr {
            match pr.phase() {
                PrPhase::Publishing => return Some(self.publishing_bar()),
                // An open PR restarts only with a warranted ci-fix; otherwise it
                // stays barred exactly as the supervisor bar leaves it.
                PrPhase::Open if !pr.fresh_ci().is_some_and(CiObservation::wake_warranted) => {
                    return Some(self.open_pr_bar(pr));
                }
                PrPhase::Open | PrPhase::Working | PrPhase::Merged | PrPhase::Abandoned => {}
            }
        }
        None
    }

    fn terminal_or_abandon_bar(&self) -> Option<String> {
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

    fn open_pr_bar(&self, pr: &TaskPr) -> String {
        let number = pr.github().expect("open Task PR passed validation").number;
        format!(
            "Task {} submitted pull request #{} and is awaiting review; \
             resume it explicitly with `lf task resume {}` to answer review",
            self.launch.issue.identifier, number, self.launch.issue.identifier,
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
        if matches!(self.pm_writeback, PmWritebackState::Pending { .. })
            && self.status != TaskSessionStatus::Completed
            && self.gate_cycle == 0
        {
            return Err(TaskDataError::InvalidInvariant(
                "pending PM completion requires a completed Task or an active gate cycle"
                    .to_string(),
            ));
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
            woken_failure_set: None,
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

    fn failing(head: &str, checks: &[&str], woken: Option<Vec<String>>) -> super::CiObservation {
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
            woken_failure_set: woken,
        }
    }

    #[test]
    fn ci_wake_dedup_fires_once_per_head_and_failure_set() {
        // A fresh failing reading warrants a wake.
        let mut obs = failing("h1", &["build", "lint"], None);
        assert!(obs.wake_warranted());
        // The failing set is order-independent and deduplicated.
        assert_eq!(
            obs.failure_set(),
            vec!["build".to_string(), "lint".to_string()]
        );

        // Once a wake fires, the same (head, set) does not wake again — a repeat
        // poll or a coalesced multi-check delivery is absorbed.
        obs.mark_woken();
        assert!(!obs.wake_warranted());
        let same_set_reordered = failing("h1", &["lint", "build"], obs.woken_failure_set.clone());
        assert!(!same_set_reordered.wake_warranted());

        // A changed failing set at the same head re-arms one new wake.
        let new_check = failing(
            "h1",
            &["build", "lint", "audit"],
            obs.woken_failure_set.clone(),
        );
        assert!(new_check.wake_warranted());

        // A passing or pending reading never warrants a wake.
        let mut green = obs.clone();
        green.state = super::CiState::Passing;
        assert!(!green.wake_warranted());
        let mut pending = obs.clone();
        pending.state = super::CiState::Pending;
        assert!(!pending.wake_warranted());
    }

    #[test]
    fn ci_fix_restart_bar_permits_only_a_warranted_open_pr_wake() {
        let session = task_session(); // status Waiting, no abandon intent

        // Open PR, fresh failing head, not yet woken: the ci-fix wake is permitted
        // where the plain supervisor restart stays barred.
        let warranted = open_pr("h1", Some(failing("h1", &["build"], None)));
        assert!(session.supervisor_restart_bar(Some(&warranted)).is_some());
        assert!(session.ci_fix_restart_bar(Some(&warranted)).is_none());

        // Already woken for this (head, set): not warranted → still barred.
        let woken = open_pr(
            "h1",
            Some(failing("h1", &["build"], Some(vec!["build".into()]))),
        );
        assert!(session.ci_fix_restart_bar(Some(&woken)).is_some());

        // Passing head → not warranted → barred.
        let mut green_obs = failing("h1", &[], None);
        green_obs.state = super::CiState::Passing;
        let green = open_pr("h1", Some(green_obs));
        assert!(session.ci_fix_restart_bar(Some(&green)).is_some());

        // Stale reading (observation head != PR head) → fresh_ci None → barred.
        let stale = open_pr("h2", Some(failing("h1", &["build"], None)));
        assert!(session.ci_fix_restart_bar(Some(&stale)).is_some());

        // Terminal intent dominates even a warranted wake.
        let mut terminal = task_session();
        terminal.status = TaskSessionStatus::Completed;
        assert!(terminal.ci_fix_restart_bar(Some(&warranted)).is_some());
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
