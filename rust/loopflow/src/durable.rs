//! Durable input and execution identity shared by Wave, Project, and Task Work.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::engine::invocation::{QueuedInvocation, StepKind, StepRef};
use crate::engine::{ConcreteStep, OccurrencePolicy};
use crate::id::{ExecId, TraceId, WaveId};

/// The exact active Run named by an in-Run process.
pub const RUN_ID_ENV: &str = "LF_RUN_ID";
pub const TASK_WORKER_CLAIM_ENV: &str = "LF_WORK_ADVANCE_CLAIM";
macro_rules! durable_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, DurableDataError> {
                let suffix = value.strip_prefix($prefix).ok_or_else(|| {
                    DurableDataError::InvalidId(format!("expected {} id", $prefix))
                })?;
                uuid::Uuid::parse_str(suffix)
                    .map_err(|error| DurableDataError::InvalidId(error.to_string()))?;
                Ok(Self(value.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = DurableDataError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurableDataError {
    #[error("invalid durable id: {0}")]
    InvalidId(String),
}

durable_id!(ProjectId, "proj_");
durable_id!(TaskId, "task_");
durable_id!(RunId, "run_");
durable_id!(HomeId, "home_");
durable_id!(ToolResponseId, "response_");
durable_id!(CronReceiptId, "cron_");

impl ProjectId {
    pub(crate) fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

impl TaskId {
    pub(crate) fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum WorkRef {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

impl WorkRef {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Wave(_) => "wave",
            Self::Project(_) => "project",
            Self::Task(_) => "task",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Wave(id) => id.as_str(),
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Home {
    pub id: HomeId,
    pub route: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub work: WorkRef,
    pub home_id: HomeId,
    #[serde(with = "time::serde::rfc3339")]
    pub placed_at: OffsetDateTime,
}

#[cfg(test)]
pub(crate) fn test_flow_invocation(
    flow: &str,
    step_index: u32,
    step: &str,
    node_id: Option<&str>,
    human: bool,
) -> QueuedInvocation {
    let steps = (0..=step_index)
        .map(|index| {
            let target = index == step_index;
            let name = if target {
                step.to_string()
            } else {
                format!("before-{index}")
            };
            crate::engine::ConcreteStep::Skill(crate::engine::ConcreteSkill {
                skill: crate::engine::Skill::named(&name),
                policy: crate::engine::OccurrencePolicy {
                    id: target.then(|| node_id.map(str::to_string)).flatten(),
                    human: target && human,
                    repeat: None,
                },
                flow_parents: Vec::new(),
            })
        })
        .collect();
    QueuedInvocation::new(flow, steps).expect("test Flow invocation has a step")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWorkerOwner {
    pub trace_id: TraceId,
    pub exec_id: ExecId,
    pub pid: u32,
    pub started_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWorkerClaim {
    pub invocation_id: String,
    pub generation: u64,
    pub position_version: u64,
    pub owner: TaskWorkerOwner,
    #[serde(with = "time::serde::rfc3339")]
    pub claimed_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFlowBlocker {
    pub run_id: Option<RunId>,
    pub reason: String,
    pub restart_required: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

impl TaskFlowBlocker {
    pub fn now(reason: impl Into<String>) -> Self {
        Self {
            run_id: None,
            reason: reason.into(),
            restart_required: false,
            observed_at: OffsetDateTime::now_utc(),
        }
    }
}

/// The current attempt at an invocation's cursor: its Run, whether the launch
/// published it, and once the Run settled, its outcome. A reserved attempt
/// is a row with `published=0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowAttempt {
    pub run_id: RunId,
    pub published: bool,
    pub outcome: Option<String>,
}

impl FlowAttempt {
    pub fn completed(&self) -> bool {
        self.outcome.as_deref() == Some("completed")
    }
}

/// Launch authority captured before starting a native turn. Observers and
/// conversation continuations do not receive this Flow capability.
#[derive(Debug, Clone)]
pub struct FlowTurnSelection {
    pub flow_id: String,
    pub version: u64,
    pub claim: Option<TaskWorkerClaim>,
    pub session_id: String,
    pub after: i64,
    pub caller_token: Option<String>,
}

/// One Flow invocation as its row holds it: the captured graph, the cursor,
/// the launch facts, the current attempt, the worker claim and the failure. A
/// Task's own invocation and a saved Flow are the same record driven by the
/// same executor; a Task's `cwd` is its worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowSession {
    /// Runtime loop parent; template composition does not create a parent.
    pub parent_id: Option<String>,
    pub invocation: QueuedInvocation,
    pub cursor: crate::engine::ExecutionCursor,
    pub version: u64,
    pub task_id: Option<TaskId>,
    pub wave_id: Option<WaveId>,
    pub cwd: std::path::PathBuf,
    pub message: Option<String>,
    pub model: Option<String>,
    pub current_attempt: Option<FlowAttempt>,
    pub pending_session_id: Option<String>,
    /// The pending review's feedback once its agent ran `lf session ready`.
    pub ready_summary: Option<String>,
    pub worker_generation: u64,
    pub claim: Option<TaskWorkerClaim>,
    pub failure: Option<TaskFlowBlocker>,
    pub finished: bool,
    pub updated_at: OffsetDateTime,
}

impl FlowSession {
    pub fn id(&self) -> &str {
        &self.invocation.id
    }

    pub fn current_step(&self) -> Option<&ConcreteStep> {
        let (steps, cursor) = self.cursor.current_body(&self.invocation.steps);
        steps.get(cursor.index)
    }

    pub fn current_plan(&self) -> &ConcreteStep {
        self.current_step()
            .expect("a persisted Flow position always selects a validated step")
    }

    pub fn current_checked(&self) -> Option<StepRef> {
        let (step, kind, policy) = match self.current_step()? {
            ConcreteStep::Skill(skill) => (
                skill.skill.name.clone(),
                StepKind::Skill,
                skill.policy.clone(),
            ),
            ConcreteStep::Op(op) => (
                op.item.display_name(),
                StepKind::Op,
                OccurrencePolicy::default(),
            ),
            ConcreteStep::Xor(branch) => (
                branch.router.name.clone(),
                StepKind::Xor,
                OccurrencePolicy::default(),
            ),
        };
        Some(StepRef {
            invocation_id: self.invocation.id.clone(),
            flow: self.invocation.flow.clone(),
            step,
            kind,
            policy,
            index: u32::try_from(self.cursor.index).ok()?,
            total: u32::try_from(self.invocation.steps.len()).ok()?,
            iteration: self.cursor.iteration,
        })
    }

    pub fn current(&self) -> StepRef {
        self.current_checked()
            .expect("a persisted Flow position always selects a validated step")
    }

    pub fn is_human(&self) -> bool {
        matches!(self.current_step(), Some(ConcreteStep::Skill(skill)) if skill.policy.human)
    }

    pub fn is_decision(&self) -> bool {
        match self.current_step() {
            Some(ConcreteStep::Xor(_)) => true,
            Some(ConcreteStep::Skill(skill)) => skill.policy.repeat.is_some(),
            _ => false,
        }
    }

    /// A decision or route its Run recorded that the driver has not settled.
    pub fn has_pending_decision(&self) -> bool {
        let leaf = self.cursor.leaf();
        match self.current_step() {
            Some(ConcreteStep::Skill(skill)) => {
                skill.policy.repeat.is_some() && leaf.progress.verdict.is_some()
            }
            Some(ConcreteStep::Xor(_)) => leaf.route.is_some(),
            _ => false,
        }
    }

    /// The name a step is reported by.
    pub fn step_name(&self) -> Option<String> {
        Some(match self.current_step()? {
            ConcreteStep::Skill(skill) => skill.skill.name.clone(),
            ConcreteStep::Op(op) => format!("op: {}", op.item.display_name()),
            ConcreteStep::Xor(branch) => branch.router.name.clone(),
        })
    }

    /// The unblock Ask key of this position's decision (`flow:<invocation>:…`).
    pub fn blocker_key(&self) -> anyhow::Result<String> {
        self.invocation.blocker_key(&self.cursor)
    }

    /// The Run a pending review's agent is running in, once launched.
    pub fn session_run_id(&self) -> Option<&RunId> {
        self.current_attempt
            .as_ref()
            .filter(|attempt| attempt.published)
            .map(|attempt| &attempt.run_id)
    }

    /// The Work the Flow was launched with, as its Runs declare it.
    pub fn declared_work(&self) -> Option<crate::session::RunWork> {
        if self.task_id.is_none() && self.wave_id.is_none() {
            return None;
        }
        Some(crate::session::RunWork {
            task_id: self.task_id.clone(),
            wave_id: self.wave_id.clone(),
            source: crate::session::WorkSource::Declared,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaskWorkerClaimOutcome {
    Claimed(TaskWorkerClaim),
    Busy(TaskWorkerClaim),
    Stale { actual_version: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Author {
    User,
    Run(RunId),
}

/// A steer projected from a Work's durable comment stream
/// (`TaskEventKind::Steer` / `ProjectEventKind::Steer`). Not a durable entity —
/// its identity is the event id, so ordering and change-detection use `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    pub id: i64,
    pub author: Author,
    pub text: String,
}

/// A steer comment surfaced for the cross-Work `lf activity` timeline: the read
/// model plus the Work it targets and when it was issued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteerComment {
    pub work: WorkRef,
    pub steer: Steer,
    pub issued_at: OffsetDateTime,
}

pub fn render_steers(steers: &[Steer]) -> String {
    if steers.is_empty() {
        return String::new();
    }
    let direction = steers
        .iter()
        .map(|steer| format!("- {}", steer.text))
        .collect::<Vec<_>>()
        .join("\n");
    format!("<lf:steers>\n{direction}\n</lf:steers>")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResponseWrite {
    pub request_id: String,
    pub choice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResponseReceipt {
    pub id: ToolResponseId,
    pub work: WorkRef,
    pub request_id: String,
    pub choice: String,
    #[serde(with = "time::serde::rfc3339")]
    pub responded_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Ready,
    Done,
    Abandoned,
}

impl WorkStatus {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub(crate) fn reason(&self) -> &'static str {
        self.label()
    }
}

impl std::fmt::Display for WorkStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbandonReceipt {
    pub work: WorkRef,
    pub reason: String,
    #[serde(with = "time::serde::rfc3339")]
    pub abandoned_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::{render_steers, Steer, WorkStatus};
    use crate::durable::Author;

    #[test]
    fn ordered_steers_render_as_one_input_projection() {
        let steer = |id: i64, text: &str| Steer {
            id,
            author: Author::User,
            text: text.to_string(),
        };
        let steers = vec![steer(1, "first"), steer(2, "second")];

        assert_eq!(
            render_steers(&steers),
            "<lf:steers>\n- first\n- second\n</lf:steers>"
        );
    }

    #[test]
    fn work_status_fixture_round_trips_every_current_variant() {
        let fixture = include_str!("../../../tests/fixtures/dto/work_statuses.json");
        let statuses: Vec<WorkStatus> = serde_json::from_str(fixture).unwrap();

        assert!(matches!(statuses[0], WorkStatus::Ready));
        assert!(matches!(statuses[1], WorkStatus::Done));
        assert!(matches!(statuses[2], WorkStatus::Abandoned));

        let encoded = serde_json::to_string(&statuses).unwrap();
        let decoded: Vec<WorkStatus> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, statuses);
    }
}
