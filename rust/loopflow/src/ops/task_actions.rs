//! The next recommended Task operation projected from Work, delivery, and controller evidence.

use serde::{Deserialize, Serialize};

use crate::durable::WorkStatus;
use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};
use crate::work::task::{AfterMerge, CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskAction {
    Resume,
    OpenPr,
    StartNextPr,
    NoAction,
}

impl TaskAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resume => "resume",
            Self::OpenPr => "open_pr",
            Self::StartNextPr => "start_next_pr",
            Self::NoAction => "no_action",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActionModel {
    pub recommended: Option<TaskAction>,
    pub reason: String,
}

#[derive(Debug)]
pub struct TaskActionEvidence<'a> {
    pub status: WorkStatus,
    pub execution: Option<&'a TaskExecutionSnapshot>,
    pub latest_pr_phase: Option<PrPhase>,
    pub latest_pr_after_merge: Option<AfterMerge>,
    pub latest_pr_merge_request: Option<&'a PrMergeRequest>,
    pub latest_pr_presentation_current: Option<bool>,
    pub completion_refusal: Option<&'a str>,
    pub resume_refusal: Option<&'a str>,
    pub ci: Option<&'a CiObservation>,
    pub predecessor_phase: Option<PrPhase>,
    pub abandon_intent: bool,
    pub launch_refusal: Option<&'a str>,
}

pub fn derive_task_actions(evidence: &TaskActionEvidence) -> TaskActionModel {
    if matches!(evidence.status, WorkStatus::Done | WorkStatus::Abandoned) {
        return action(TaskAction::NoAction, "Task is terminal");
    }
    if evidence.abandon_intent {
        return action(TaskAction::NoAction, "Task is being abandoned");
    }
    if let Some(remaining) = evidence
        .completion_refusal
        .filter(|reason| reason.contains("Remaining work:"))
    {
        return action(TaskAction::NoAction, remaining);
    }
    if evidence.latest_pr_phase != Some(PrPhase::Merged) {
        if let Some(execution) = evidence
            .execution
            .filter(|execution| execution.state != TaskExecutionState::Idle)
        {
            return action(TaskAction::NoAction, &execution.reason);
        }
        if let Some(refusal) = evidence.launch_refusal {
            return action(TaskAction::NoAction, refusal);
        }
    }
    let model = phase_action(evidence);
    let model = apply_predecessor(model, evidence.predecessor_phase);
    apply_resume_refusal(model, evidence.resume_refusal)
}

fn phase_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    match evidence.latest_pr_phase {
        Some(PrPhase::Open) if evidence.latest_pr_presentation_current == Some(false) => action(
            TaskAction::Resume,
            "refresh the reviewer-facing PR title and body for the current head, then settle it with `lf land`",
        ),
        Some(PrPhase::Open) => match evidence.ci {
            Some(ci) if ci.state == CiState::Failing && !ci.only_land_time_preconditions() => {
                action(TaskAction::Resume, ci_failure_reason(ci))
            }
            _ if evidence.latest_pr_merge_request.is_none() => action(
                TaskAction::Resume,
                "PR is published but settlement is not armed; run `lf land`",
            ),
            Some(ci) if ci.only_land_time_preconditions() || ci.state == CiState::Passing => {
                let request = evidence
                    .latest_pr_merge_request
                    .expect("checked merge request above");
                let short = request.head_sha.chars().take(12).collect::<String>();
                match request.mode {
                    PrMergeMode::User => {
                        action(TaskAction::OpenPr, format!("merge head {short} on GitHub"))
                    }
                    PrMergeMode::Auto => action(
                        TaskAction::NoAction,
                        format!("GitHub auto-merge is settling head {short}"),
                    ),
                }
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
        Some(PrPhase::Working) | None => action(
            TaskAction::Resume,
            "run the next work with `lf task run`; inspect earlier Flows and independent Sessions first",
        ),
    }
}

fn merged_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    if let Some(refusal) = evidence.completion_refusal {
        return action(TaskAction::NoAction, refusal);
    }
    match evidence
        .latest_pr_after_merge
        .expect("a merged Task PR has an after-merge disposition")
    {
        AfterMerge::ContinueTask => match evidence.latest_pr_merge_request.and_then(|request| request.next_slug.as_deref()) {
            Some(next) => action(TaskAction::StartNextPr, format!("PR merged; remaining PR work: {next}")),
            None => action(TaskAction::NoAction, "PR merged; earlier delivery kept the Task open without a recorded remaining outcome. Inspect accepted scope, then record `lf task follow-up` or move it to `end`"),
        },
        AfterMerge::CompleteTask => action(
            TaskAction::NoAction,
            "PR merged; delivery reconciliation completes the Task",
        ),
    }
}

fn apply_predecessor(model: TaskActionModel, predecessor: Option<PrPhase>) -> TaskActionModel {
    match predecessor {
        Some(PrPhase::Abandoned) => action(
            TaskAction::Resume,
            "parent PR was abandoned; sync or abandon this stack",
        ),
        Some(PrPhase::Merged) | None => model,
        Some(_) if matches!(model.recommended, Some(TaskAction::StartNextPr)) => {
            action(TaskAction::NoAction, "waiting for parent PR to merge")
        }
        Some(_) => model,
    }
}

fn apply_resume_refusal(model: TaskActionModel, refusal: Option<&str>) -> TaskActionModel {
    if !matches!(model.recommended, Some(TaskAction::Resume)) {
        return model;
    }
    match refusal {
        Some(refusal) => action(TaskAction::NoAction, refusal),
        None => model,
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
    use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};
    use crate::work::task::{
        AfterMerge, CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase,
    };

    fn evidence<'a>(
        phase: PrPhase,
        after_merge: Option<AfterMerge>,
        ci: Option<&'a CiObservation>,
    ) -> TaskActionEvidence<'a> {
        TaskActionEvidence {
            status: WorkStatus::Ready,
            execution: None,
            latest_pr_phase: Some(phase),
            latest_pr_after_merge: after_merge,
            latest_pr_merge_request: None,
            latest_pr_presentation_current: Some(true),
            completion_refusal: None,
            resume_refusal: None,
            ci,
            predecessor_phase: None,
            abandon_intent: false,
            launch_refusal: None,
        }
    }

    #[test]
    fn abandoned_parent_never_recommends_resuming_terminal_or_canceling_tasks() {
        for (status, abandon_intent, reason) in [
            (WorkStatus::Done, false, "Task is terminal"),
            (WorkStatus::Abandoned, false, "Task is terminal"),
            (WorkStatus::Ready, true, "Task is being abandoned"),
        ] {
            let mut evidence = evidence(PrPhase::Working, None, None);
            evidence.status = status;
            evidence.abandon_intent = abandon_intent;
            evidence.predecessor_phase = Some(PrPhase::Abandoned);
            let model = derive_task_actions(&evidence);
            assert_eq!(model.recommended, Some(TaskAction::NoAction));
            assert_eq!(model.reason, reason);
        }
    }

    #[test]
    fn active_or_uncertain_execution_never_recommends_another_implementation() {
        for state in [
            TaskExecutionState::Starting,
            TaskExecutionState::Running,
            TaskExecutionState::Unknown,
            TaskExecutionState::Blocked,
        ] {
            let execution = TaskExecutionSnapshot {
                state,
                reason: "Inspect the existing worker".into(),
                step: None,
                captured: None,
            };
            let mut evidence = evidence(PrPhase::Open, None, None);
            evidence.execution = Some(&execution);
            evidence.launch_refusal = Some("next launch configuration is invalid");
            evidence.latest_pr_presentation_current = Some(false);
            evidence.predecessor_phase = Some(PrPhase::Abandoned);
            let model = derive_task_actions(&evidence);
            assert_eq!(model.recommended, Some(TaskAction::NoAction));
            assert_eq!(model.reason, execution.reason);
        }
    }

    #[test]
    fn historical_continuation_needs_a_specific_remaining_outcome() {
        let mut evidence = evidence(PrPhase::Merged, Some(AfterMerge::ContinueTask), None);
        let model = derive_task_actions(&evidence);
        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert!(model.reason.contains("recorded remaining outcome"));
        let request = PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: OffsetDateTime::now_utc(),
            head_sha: "delivered-head".into(),
            after_merge: AfterMerge::ContinueTask,
            next_slug: Some("second-part".into()),
        };
        evidence.latest_pr_merge_request = Some(&request);
        let model = derive_task_actions(&evidence);
        assert_eq!(model.recommended, Some(TaskAction::StartNextPr));
        assert!(model.reason.contains("second-part"));
    }

    #[test]
    fn nonresumable_execution_blocker_names_the_user_as_next_owner() {
        let mut evidence = evidence(PrPhase::Working, None, None);
        evidence.launch_refusal = Some(
            "Task execution boundary is blocked: linked Git index.lock is not writable; correct the filesystem capability before starting a new Session",
        );

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(
            model.reason,
            "Task execution boundary is blocked: linked Git index.lock is not writable; correct the filesystem capability before starting a new Session"
        );
    }

    #[test]
    fn passing_published_pr_requires_settlement_to_be_armed() {
        let ci = CiObservation {
            head_sha: "head".to_string(),
            state: CiState::Passing,
            failing_checks: Vec::new(),
            observed_at: OffsetDateTime::now_utc(),
        };
        let evidence = evidence(PrPhase::Open, Some(AfterMerge::ContinueTask), Some(&ci));

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::Resume));
        assert_eq!(
            model.reason,
            "PR is published but settlement is not armed; run `lf land`"
        );
    }

    #[test]
    fn stale_or_missing_pr_copy_is_actionable_before_merge_state() {
        let mut evidence = evidence(PrPhase::Open, Some(AfterMerge::CompleteTask), None);
        evidence.latest_pr_presentation_current = Some(false);

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::Resume));
        assert!(model.reason.contains("reviewer-facing PR title and body"));
    }

    #[test]
    fn resume_refusal_suppresses_resume_during_a_working_pr() {
        let mut evidence = evidence(PrPhase::Working, None, None);
        evidence.resume_refusal = Some("Task worktree is still initializing");

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(model.reason, "Task worktree is still initializing");
    }

    #[test]
    fn user_merge_request_recommends_the_exact_merge() {
        let ci = CiObservation {
            head_sha: "head-1234567890".to_string(),
            state: CiState::Passing,
            failing_checks: Vec::new(),
            observed_at: OffsetDateTime::now_utc(),
        };
        let request = PrMergeRequest {
            mode: PrMergeMode::User,
            requested_at: OffsetDateTime::now_utc(),
            head_sha: ci.head_sha.clone(),
            after_merge: AfterMerge::ContinueTask,
            next_slug: None,
        };
        let mut evidence = evidence(PrPhase::Open, Some(AfterMerge::ContinueTask), Some(&ci));
        evidence.latest_pr_merge_request = Some(&request);

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::OpenPr));
        assert_eq!(model.reason, "merge head head-1234567 on GitHub");
    }

    #[test]
    fn auto_merge_request_is_owned_by_github() {
        let ci = CiObservation {
            head_sha: "head".to_string(),
            state: CiState::Passing,
            failing_checks: Vec::new(),
            observed_at: OffsetDateTime::now_utc(),
        };
        let request = PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: OffsetDateTime::now_utc(),
            head_sha: ci.head_sha.clone(),
            after_merge: AfterMerge::ContinueTask,
            next_slug: None,
        };
        let mut evidence = evidence(PrPhase::Open, Some(AfterMerge::ContinueTask), Some(&ci));
        evidence.latest_pr_merge_request = Some(&request);

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(model.reason, "GitHub auto-merge is settling head head");
    }

    #[test]
    fn land_only_failure_does_not_reopen_the_task_body() {
        let ci = CiObservation {
            head_sha: "head".to_string(),
            state: CiState::Failing,
            failing_checks: vec![crate::work::task::CiCheck {
                name: "scratch-clear".to_string(),
                url: None,
            }],
            observed_at: OffsetDateTime::now_utc(),
        };
        let request = PrMergeRequest {
            mode: PrMergeMode::User,
            requested_at: OffsetDateTime::now_utc(),
            head_sha: ci.head_sha.clone(),
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
        };
        let mut evidence = evidence(PrPhase::Open, Some(AfterMerge::CompleteTask), Some(&ci));
        evidence.latest_pr_merge_request = Some(&request);

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::OpenPr));
    }
}
