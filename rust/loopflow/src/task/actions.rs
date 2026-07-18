//! The next legal Task action.

use serde::{Deserialize, Serialize};

use crate::durable::WorkStatus;
use crate::task::{AfterMerge, CiObservation, CiState, PrPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskAction {
    Recover,
    Resume,
    Review,
    StartNextPr,
    Complete,
    NoAction,
}

impl TaskAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recover => "recover",
            Self::Resume => "resume",
            Self::Review => "review",
            Self::StartNextPr => "start_next_pr",
            Self::Complete => "complete",
            Self::NoAction => "no_action",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActionModel {
    pub recommended: Option<TaskAction>,
    pub reason: String,
}

impl TaskActionModel {
    pub fn no_task() -> Self {
        Self {
            recommended: None,
            reason: "Task is ready to start".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReviewGateState {
    Requested,
    Active,
    Approved,
    ChangesRequested,
}

pub struct TaskActionEvidence<'a> {
    pub status: WorkStatus,
    pub latest_pr_phase: Option<PrPhase>,
    pub latest_pr_after_merge: Option<AfterMerge>,
    pub latest_pr_next_slug: Option<&'a str>,
    pub completion_refusal: Option<&'a str>,
    pub resume_refusal: Option<&'a str>,
    pub pending_directive: bool,
    pub ci: Option<&'a CiObservation>,
    pub process_alive: Option<bool>,
    pub predecessor_phase: Option<PrPhase>,
    pub review_gate: Option<ReviewGateState>,
    pub abandon_intent: bool,
    pub local_progress_unsettled: Option<bool>,
}

pub fn derive_task_actions(evidence: &TaskActionEvidence) -> TaskActionModel {
    let model = if matches!(evidence.status, WorkStatus::Done | WorkStatus::Abandoned) {
        action(TaskAction::NoAction, "Task is terminal")
    } else if evidence.abandon_intent {
        action(TaskAction::NoAction, "Task is being abandoned")
    } else if matches!(
        evidence.review_gate,
        Some(ReviewGateState::Requested | ReviewGateState::Active)
    ) && evidence.latest_pr_phase != Some(PrPhase::Merged)
    {
        action(TaskAction::Review, "awaiting review disposition")
    } else if evidence.review_gate == Some(ReviewGateState::ChangesRequested)
        && evidence.resume_refusal.is_none()
    {
        action(TaskAction::Resume, "address requested changes")
    } else {
        phase_action(evidence)
    };
    apply_predecessor(model, evidence.predecessor_phase)
}

fn phase_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    match evidence.latest_pr_phase {
        Some(PrPhase::Open) => match evidence.ci {
            Some(ci) if ci.only_land_time_preconditions() || ci.state == CiState::Passing => {
                action(TaskAction::Review, "checks passed; awaiting review")
            }
            Some(ci) if ci.state == CiState::Failing => {
                action(TaskAction::Resume, ci_failure_reason(ci))
            }
            Some(_) => action(TaskAction::NoAction, "required checks still running"),
            None => action(
                TaskAction::NoAction,
                "required checks have not been observed",
            ),
        },
        Some(PrPhase::Publishing) => action(TaskAction::Resume, "retry publication"),
        Some(PrPhase::Merged) => merged_action(evidence),
        Some(PrPhase::Abandoned) => {
            action(TaskAction::StartNextPr, "PR abandoned; start the next PR")
        }
        Some(PrPhase::Working) => body_action(evidence),
        None if evidence.resume_refusal.is_some() => action(
            TaskAction::NoAction,
            evidence.resume_refusal.expect("checked above"),
        ),
        None => body_action(evidence),
    }
}

fn merged_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    if let Some(refusal) = evidence.completion_refusal {
        let next = if evidence.pending_directive
            || evidence.review_gate == Some(ReviewGateState::ChangesRequested)
        {
            TaskAction::StartNextPr
        } else {
            TaskAction::Review
        };
        return action(next, refusal);
    }
    match evidence.latest_pr_after_merge {
        Some(AfterMerge::CompleteTask) | None => {
            action(TaskAction::Complete, "PR merged; complete the Task")
        }
        Some(AfterMerge::Review) if evidence.review_gate != Some(ReviewGateState::Approved) => {
            action(TaskAction::Review, "merged; answer the post-merge review")
        }
        Some(AfterMerge::Review) if evidence.latest_pr_next_slug.is_some() => action(
            TaskAction::StartNextPr,
            "merged and reviewed; start the next PR",
        ),
        Some(AfterMerge::Review) => action(
            TaskAction::Complete,
            "merged and reviewed; complete the Task",
        ),
    }
}

fn body_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    match evidence.process_alive {
        Some(true) => action(TaskAction::NoAction, "Task body is working"),
        Some(false) if evidence.local_progress_unsettled == Some(true) => action(
            TaskAction::Recover,
            "Task body stopped with unsettled work; recover to continue",
        ),
        Some(false) => action(
            TaskAction::Recover,
            "Task body stopped; recover to continue",
        ),
        None => action(TaskAction::Resume, "resume the parked Task"),
    }
}

fn apply_predecessor(model: TaskActionModel, predecessor: Option<PrPhase>) -> TaskActionModel {
    match predecessor {
        Some(PrPhase::Abandoned) => action(
            TaskAction::Resume,
            "parent PR was abandoned; rebase or abandon this stack",
        ),
        Some(PrPhase::Merged) | None => model,
        Some(_)
            if matches!(
                model.recommended,
                Some(TaskAction::Complete | TaskAction::StartNextPr)
            ) =>
        {
            action(TaskAction::NoAction, "waiting for parent PR to merge")
        }
        Some(_) => model,
    }
}

fn action(recommended: TaskAction, reason: impl Into<String>) -> TaskActionModel {
    TaskActionModel {
        recommended: Some(recommended),
        reason: reason.into(),
    }
}

pub fn ci_failure_reason(ci: &CiObservation) -> String {
    let names = ci
        .failing_checks
        .iter()
        .map(|check| check.name.as_str())
        .collect::<Vec<_>>();
    if names.is_empty() {
        "required checks failed".into()
    } else {
        format!("required checks failed: {}", names.join(", "))
    }
}
