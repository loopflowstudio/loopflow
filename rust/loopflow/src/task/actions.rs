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
    OpenPr,
    StartNextPr,
    Complete,
    NoAction,
}

impl TaskAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recover => "recover",
            Self::Resume => "resume",
            Self::OpenPr => "open_pr",
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

pub struct TaskActionEvidence<'a> {
    pub status: WorkStatus,
    pub latest_pr_phase: Option<PrPhase>,
    pub latest_pr_after_merge: Option<AfterMerge>,
    pub completion_refusal: Option<&'a str>,
    pub resume_refusal: Option<&'a str>,
    pub ci: Option<&'a CiObservation>,
    pub process_alive: Option<bool>,
    pub predecessor_phase: Option<PrPhase>,
    pub abandon_intent: bool,
    pub local_progress_unsettled: Option<bool>,
}

pub fn derive_task_actions(evidence: &TaskActionEvidence) -> TaskActionModel {
    let model = if matches!(evidence.status, WorkStatus::Done | WorkStatus::Abandoned) {
        action(TaskAction::NoAction, "Task is terminal")
    } else if evidence.abandon_intent {
        action(TaskAction::NoAction, "Task is being abandoned")
    } else {
        phase_action(evidence)
    };
    apply_predecessor(model, evidence.predecessor_phase)
}

fn phase_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    match evidence.latest_pr_phase {
        Some(PrPhase::Open) => match evidence.ci {
            Some(ci) if ci.only_land_time_preconditions() || ci.state == CiState::Passing => {
                action(TaskAction::OpenPr, "checks passed; open the PR")
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
    match evidence
        .latest_pr_after_merge
        .expect("a merged Task PR has an after-merge disposition")
    {
        AfterMerge::ContinueTask => action(
            TaskAction::StartNextPr,
            "PR merged; continue the Task on its next PR",
        ),
        AfterMerge::CompleteTask => match evidence.completion_refusal {
            Some(refusal) => action(TaskAction::NoAction, refusal),
            None => action(TaskAction::Complete, "PR merged; complete the Task"),
        },
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

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::{derive_task_actions, TaskAction, TaskActionEvidence};
    use crate::durable::WorkStatus;
    use crate::task::{AfterMerge, CiObservation, CiState, PrPhase};

    fn evidence<'a>(
        phase: PrPhase,
        after_merge: Option<AfterMerge>,
        ci: Option<&'a CiObservation>,
    ) -> TaskActionEvidence<'a> {
        TaskActionEvidence {
            status: WorkStatus::Ready,
            latest_pr_phase: Some(phase),
            latest_pr_after_merge: after_merge,
            completion_refusal: None,
            resume_refusal: None,
            ci,
            process_alive: None,
            predecessor_phase: None,
            abandon_intent: false,
            local_progress_unsettled: Some(false),
        }
    }

    #[test]
    fn merged_continue_task_starts_the_next_pr_without_review_state() {
        let model = derive_task_actions(&evidence(
            PrPhase::Merged,
            Some(AfterMerge::ContinueTask),
            None,
        ));

        assert_eq!(model.recommended, Some(TaskAction::StartNextPr));
        assert_eq!(model.reason, "PR merged; continue the Task on its next PR");
    }

    #[test]
    fn passing_open_pr_only_recommends_opening_the_pr() {
        let ci = CiObservation {
            head_sha: "head".to_string(),
            state: CiState::Passing,
            failing_checks: Vec::new(),
            observed_at: OffsetDateTime::now_utc(),
        };
        let evidence = evidence(PrPhase::Open, Some(AfterMerge::ContinueTask), Some(&ci));

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::OpenPr));
        assert_eq!(model.reason, "checks passed; open the PR");
    }
}
