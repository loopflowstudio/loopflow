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
    prefixed_uuid_id, AbandonIntent, ChildCommandEffect, ChildCommandId, ChildCommandState,
    ChildDecisionId, ChildDirectiveId, ChildExecutionContext, ChildProcessGeneration,
    DirectiveKind,
};
use crate::id::WaveId;
use crate::project_session::ProjectSessionId;
use crate::session_context::TaskLaunchReceipt;

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
    pub publication: Option<PrPublication>,
    pub merge_commit: Option<String>,
    pub abandoned_at: Option<OffsetDateTime>,
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
    /// Provider/model selection captured when the session is reserved.
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    /// Latest launch generation, retained after that process exits.
    pub latest_process: Option<ChildProcessGeneration>,
    /// The binary and store every generation of this Session relaunches with.
    /// `None` only for a Session created before the context was pinned: nothing
    /// recorded which `lf` spawned it, so it refuses to launch rather than let
    /// the calling process guess.
    pub execution: Option<ChildExecutionContext>,
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
        if let Some(pr) = active_pr {
            match pr.phase() {
                PrPhase::Publishing => {
                    return Some(format!(
                        "Task {} requested PR publication but has no GitHub receipt; \
                         resume it explicitly with `lf task resume {}` to retry publication",
                        self.launch.issue.identifier, self.launch.issue.identifier,
                    ));
                }
                PrPhase::Open => {
                    let number = pr.github().expect("open Task PR passed validation").number;
                    return Some(format!(
                        "Task {} submitted pull request #{} and is awaiting review; \
                         resume it explicitly with `lf task resume {}` to answer review",
                        self.launch.issue.identifier, number, self.launch.issue.identifier,
                    ));
                }
                PrPhase::Working | PrPhase::Merged | PrPhase::Abandoned => {}
            }
        }
        None
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
        if matches!(self.pm_writeback, PmWritebackState::Pending { .. })
            && self.status != TaskSessionStatus::Completed
        {
            return Err(TaskDataError::InvalidInvariant(
                "pending PM completion is valid only for a completed Task".to_string(),
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
            tmux_name,
            started_at: now,
        });
        self.set_status(TaskSessionStatus::Starting, "task process is starting");
        generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventKind {
    Started,
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
                Self::DecisionRequested { .. } | Self::DecisionResolved { .. }
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

#[cfg(test)]
mod tests {
    use super::{
        AfterMerge, ChildCommandId, ChildCommandState, GithubPr, PmWritebackOperation,
        PmWritebackState, PrPhase, PrPublication, TaskEventKind, TaskObservation, TaskPr, TaskPrId,
        TaskSession, TaskSessionId, TaskSessionStatus,
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
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            execution: Some(crate::child_session::ChildExecutionContext::for_tests()),
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
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
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
            publication: Some(PrPublication {
                requested_at: now,
                after_merge: AfterMerge::Review,
                next_slug: Some("released_upgrade".to_string()),
                github: Some(GithubPr {
                    number: 872,
                    url: "https://github.com/loopflowstudio/loopflow/pull/872".to_string(),
                }),
            }),
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
        };
        assert!(pr.validate().is_err());

        let publication = pr.publication.as_mut().unwrap();
        publication.next_slug = Some("released-upgrade".to_string());
        assert!(pr.validate().is_ok());

        pr.publication.as_mut().unwrap().after_merge = AfterMerge::CompleteTask;
        assert!(pr.validate().is_err());
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
    }
}
