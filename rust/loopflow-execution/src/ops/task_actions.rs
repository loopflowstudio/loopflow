//! The next recommended Task operation projected from Work, delivery, and controller evidence.

use serde::{Deserialize, Serialize};

use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};
use loopflow_storage::execution::TaskState;
use loopflow_storage::work::task::{CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskAction {
    Run,
    OpenPr,
    NoAction,
}

impl TaskAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::OpenPr => "open_pr",
            Self::NoAction => "no_action",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskActionModel {
    pub recommended: Option<TaskAction>,
    pub reason: String,
}

#[derive(Debug)]
pub struct TaskActionEvidence<'a> {
    pub status: TaskState,
    pub execution: Option<&'a TaskExecutionSnapshot>,
    pub latest_pr_phase: Option<PrPhase>,
    pub latest_pr_merge_request: Option<&'a PrMergeRequest>,
    pub latest_pr_presentation_current: Option<bool>,
    /// The Task's Graph stands at `end`.
    pub at_end: bool,
    pub open_children: u32,
    pub completion_refusal: Option<&'a str>,
    pub run_refusal: Option<&'a str>,
    pub ci: Option<&'a CiObservation>,
    pub predecessor_phase: Option<PrPhase>,
    pub launch_refusal: Option<&'a str>,
}

pub fn derive_task_actions(evidence: &TaskActionEvidence) -> TaskActionModel {
    if evidence.status == TaskState::Paused {
        return action(TaskAction::NoAction, "Task is paused");
    }
    if evidence.status.is_terminal() {
        return action(TaskAction::NoAction, "Task is terminal");
    }
    if let Some(execution) = evidence
        .execution
        .filter(|execution| execution.state == TaskExecutionState::Running)
    {
        return action(TaskAction::NoAction, &execution.reason);
    }
    let finished = Finished::of(evidence.status, evidence.latest_pr_phase, evidence.at_end);
    if finished.is_none() {
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
    let model = match finished {
        Some(finished) => finished_action(finished, evidence),
        None => phase_action(evidence),
    };
    let model = apply_predecessor(model, evidence.predecessor_phase);
    apply_run_refusal(model, evidence.run_refusal)
}

fn phase_action(evidence: &TaskActionEvidence) -> TaskActionModel {
    match evidence.latest_pr_phase {
        Some(PrPhase::Open) if evidence.latest_pr_presentation_current == Some(false) => action(
            TaskAction::Run,
            "refresh the reviewer-facing PR title and body for the current head, then settle it with `lf land`",
        ),
        Some(PrPhase::Open) => match evidence.ci {
            Some(ci) if ci.state == CiState::Failing => {
                action(TaskAction::Run, ci_failure_reason(ci))
            }
            _ if evidence.latest_pr_merge_request.is_none() => action(
                TaskAction::Run,
                "PR is published but settlement is not armed; run `lf land`",
            ),
            Some(ci) if ci.state == CiState::Passing => {
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
        Some(PrPhase::Publishing) => action(TaskAction::Run, "retry publication"),
        Some(PrPhase::Abandoned) => {
            action(TaskAction::NoAction, "PR closed without merge; reopen the same PR or abandon the Task")
        }
        Some(PrPhase::Working) | None => action(
            TaskAction::Run,
            "run the next work with `lf task run`; inspect earlier Graphs and independent Sessions first",
        ),
        Some(phase) => action(TaskAction::NoAction, format!("unsupported PR phase: {}", phase.as_str())),
    }
}

/// How an open Task's own work ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finished {
    /// Its PR merged.
    Merged,
    /// It has no PR and its Graph stands at `end`.
    AtEnd,
}

impl Finished {
    /// `None` while the Task still has work of its own, or is closed.
    pub fn of(status: TaskState, pr: Option<PrPhase>, at_end: bool) -> Option<Self> {
        if status.is_terminal() {
            return None;
        }
        match pr {
            Some(PrPhase::Merged) => Some(Self::Merged),
            None if at_end => Some(Self::AtEnd),
            _ => None,
        }
    }

    /// With children open the Task stays open to conduct them; with none it
    /// waits only to be completed.
    pub fn reason(self, open_children: u32) -> String {
        let word = match self {
            Self::Merged => "Merged",
            Self::AtEnd => "Finished",
        };
        match open_children {
            0 => format!("{word}; complete the Task"),
            1 => format!("{word} · 1 child open"),
            count => format!("{word} · {count} children open"),
        }
    }
}

fn finished_action(finished: Finished, evidence: &TaskActionEvidence) -> TaskActionModel {
    if evidence.open_children == 0 {
        if let Some(refusal) = evidence.completion_refusal {
            return action(TaskAction::NoAction, refusal);
        }
    }
    action(
        TaskAction::NoAction,
        finished.reason(evidence.open_children),
    )
}

fn apply_predecessor(model: TaskActionModel, predecessor: Option<PrPhase>) -> TaskActionModel {
    match predecessor {
        Some(PrPhase::Abandoned) => action(
            TaskAction::Run,
            "parent PR was abandoned; sync or abandon this stack",
        ),
        _ => model,
    }
}

fn apply_run_refusal(model: TaskActionModel, refusal: Option<&str>) -> TaskActionModel {
    if !matches!(model.recommended, Some(TaskAction::Run)) {
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
    use crate::ops::task_execution::{TaskExecutionSnapshot, TaskExecutionState};
    use loopflow_storage::execution::TaskState;
    use loopflow_storage::work::task::{
        CiObservation, CiState, PrMergeMode, PrMergeRequest, PrPhase,
    };

    fn evidence<'a>(phase: PrPhase, ci: Option<&'a CiObservation>) -> TaskActionEvidence<'a> {
        TaskActionEvidence {
            status: TaskState::Idle,
            execution: None,
            latest_pr_phase: Some(phase),
            latest_pr_merge_request: None,
            latest_pr_presentation_current: Some(true),
            at_end: false,
            open_children: 0,
            completion_refusal: None,
            run_refusal: None,
            ci,
            predecessor_phase: None,
            launch_refusal: None,
        }
    }

    #[test]
    fn abandoned_parent_never_recommends_resuming_terminal_or_canceling_tasks() {
        for (status, reason) in [
            (TaskState::Done, "Task is terminal"),
            (TaskState::Abandoned, "Task is terminal"),
        ] {
            let mut evidence = evidence(PrPhase::Working, None);
            evidence.status = status;
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
                died: None,
                outlook: None,
            };
            let mut evidence = evidence(PrPhase::Open, None);
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
    fn nonresumable_execution_blocker_names_the_user_as_next_owner() {
        let mut evidence = evidence(PrPhase::Working, None);
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
        let evidence = evidence(PrPhase::Open, Some(&ci));

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::Run));
        assert_eq!(
            model.reason,
            "PR is published but settlement is not armed; run `lf land`"
        );
    }

    #[test]
    fn stale_or_missing_pr_copy_is_actionable_before_merge_state() {
        let mut evidence = evidence(PrPhase::Open, None);
        evidence.latest_pr_presentation_current = Some(false);

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::Run));
        assert!(model.reason.contains("reviewer-facing PR title and body"));
    }

    #[test]
    fn a_merged_task_with_open_children_is_not_asked_to_complete() {
        let mut evidence = evidence(PrPhase::Merged, None);
        assert_eq!(
            derive_task_actions(&evidence).reason,
            "Merged; complete the Task"
        );
        evidence.open_children = 1;
        assert_eq!(
            derive_task_actions(&evidence).reason,
            "Merged · 1 child open"
        );
        // A new Graph pass running on a merged Task reports the live run, not the earlier merge.
        let running = TaskExecutionSnapshot {
            state: TaskExecutionState::Running,
            reason: "code at implement".into(),
            step: Some("implement".into()),
            captured: None,
            died: None,
            outlook: None,
        };
        evidence.execution = Some(&running);
        assert_eq!(derive_task_actions(&evidence).reason, "code at implement");
    }

    #[test]
    fn a_task_with_no_pr_at_the_end_of_its_graph_is_finished() {
        let mut evidence = evidence(PrPhase::Working, None);
        evidence.latest_pr_phase = None;
        assert_eq!(
            derive_task_actions(&evidence).recommended,
            Some(TaskAction::Run)
        );
        evidence.at_end = true;
        evidence.open_children = 2;
        let model = derive_task_actions(&evidence);
        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(model.reason, "Finished · 2 children open");
        evidence.open_children = 0;
        let model = derive_task_actions(&evidence);
        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(model.reason, "Finished; complete the Task");

        // At `end` with a PR still open, the PR is what is left.
        evidence.latest_pr_phase = Some(PrPhase::Open);
        assert!(derive_task_actions(&evidence).reason.contains("`lf land`"));
    }

    #[test]
    fn run_refusal_suppresses_run_during_a_working_pr() {
        let mut evidence = evidence(PrPhase::Working, None);
        evidence.run_refusal = Some("Task worktree is still initializing");

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
        };
        let mut evidence = evidence(PrPhase::Open, Some(&ci));
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
        };
        let mut evidence = evidence(PrPhase::Open, Some(&ci));
        evidence.latest_pr_merge_request = Some(&request);

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(model.reason, "GitHub auto-merge is settling head head");
    }

    #[test]
    fn repository_named_failure_requires_repair() {
        let ci = CiObservation {
            head_sha: "head".to_string(),
            state: CiState::Failing,
            failing_checks: vec![loopflow_storage::work::task::CiCheck {
                name: "scratch-clear".to_string(),
                url: None,
            }],
            observed_at: OffsetDateTime::now_utc(),
        };
        let request = PrMergeRequest {
            mode: PrMergeMode::User,
            requested_at: OffsetDateTime::now_utc(),
            head_sha: ci.head_sha.clone(),
        };
        let mut evidence = evidence(PrPhase::Open, Some(&ci));
        evidence.latest_pr_merge_request = Some(&request);

        let model = derive_task_actions(&evidence);

        assert_eq!(model.recommended, Some(TaskAction::Run));
    }
}
