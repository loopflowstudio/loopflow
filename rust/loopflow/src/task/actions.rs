//! The next legal Task action.

use serde::{Deserialize, Serialize};

use crate::durable::WorkStatus;
use crate::task::{AfterMerge, CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskAction {
    Resume,
    OpenPr,
    StartNextPr,
    Complete,
    NoAction,
}

impl TaskAction {
    pub fn as_str(self) -> &'static str {
        match self {
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
    if !matches!(evidence.status, WorkStatus::Done | WorkStatus::Abandoned)
        && !evidence.abandon_intent
    {
        if let Some(refusal) = evidence.launch_refusal {
            return action(TaskAction::NoAction, refusal);
        }
    }
    let model = if matches!(evidence.status, WorkStatus::Done | WorkStatus::Abandoned) {
        action(TaskAction::NoAction, "Task is terminal")
    } else if evidence.abandon_intent {
        action(TaskAction::NoAction, "Task is being abandoned")
    } else {
        phase_action(evidence)
    };
    let model = apply_predecessor(model, evidence.predecessor_phase);
    apply_resume_refusal(model, evidence.resume_refusal)
}

fn phase_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    match evidence.latest_pr_phase {
        Some(PrPhase::Open) if evidence.latest_pr_presentation_current == Some(false) => action(
            TaskAction::Resume,
            "refresh the reviewer-facing PR title and body for the current head, then settle it with `lf pr land -c`",
        ),
        Some(PrPhase::Open) => match evidence.ci {
            Some(ci) if ci.state == CiState::Failing && !ci.only_land_time_preconditions() => {
                action(TaskAction::Resume, ci_failure_reason(ci))
            }
            _ if evidence.latest_pr_merge_request.is_none() => action(
                TaskAction::Resume,
                "PR is published but settlement is not armed; run `lf pr land -c`",
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
    if matches!(evidence.status, WorkStatus::Done | WorkStatus::Abandoned) {
        action(TaskAction::NoAction, "Task is terminal")
    } else {
        action(TaskAction::Resume, "resume the parked Task")
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
    use crate::task::{AfterMerge, CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase};

    fn evidence<'a>(
        phase: PrPhase,
        after_merge: Option<AfterMerge>,
        ci: Option<&'a CiObservation>,
    ) -> TaskActionEvidence<'a> {
        TaskActionEvidence {
            status: WorkStatus::Ready,
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
    fn invalid_lifecycle_suppresses_serial_pr_continuation() {
        let mut evidence = evidence(PrPhase::Merged, Some(AfterMerge::ContinueTask), None);
        evidence.predecessor_phase = Some(PrPhase::Abandoned);
        evidence.resume_refusal = Some("Task has no active PR to resume");
        evidence.launch_refusal = Some(
            "Task INT-10 cannot launch: loop phase is missing autonomous_progress; abandon and replace it with valid flows",
        );

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(
            model.reason,
            "Task INT-10 cannot launch: loop phase is missing autonomous_progress; abandon and replace it with valid flows"
        );
    }

    #[test]
    fn nonresumable_execution_blocker_names_the_user_as_next_owner() {
        let mut evidence = evidence(PrPhase::Working, None, None);
        evidence.launch_refusal = Some(
            "Task execution boundary is blocked: linked Git index.lock is not writable; correct the filesystem capability before starting a new Run",
        );

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(
            model.reason,
            "Task execution boundary is blocked: linked Git index.lock is not writable; correct the filesystem capability before starting a new Run"
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
            "PR is published but settlement is not armed; run `lf pr land -c`"
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
            failing_checks: vec![crate::task::CiCheck {
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
