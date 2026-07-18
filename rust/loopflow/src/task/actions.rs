//! Legal-action model for Task Sessions.
//!
//! One pure function ([`derive_task_actions`]) computes the seven lifecycle
//! actions from a total evidence bundle. Every surface (`lf task status`,
//! `lf status`, `lf roadmap`, Mac app) consumes this one model — no
//! client-side re-derivation.

use serde::{Deserialize, Serialize};

use crate::task::{AfterMerge, CiObservation, CiState, PrPhase, TaskSessionStatus};

/// The six lifecycle actions a Task Session can take, computed from total
/// evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskAction {
    Recover,
    Resume,
    Review,
    StartNextPr,
    Reconcile,
    Complete,
    NoAction,
}

impl TaskAction {
    /// Canonical order used by [`TaskActionModel::actions`].
    pub const ALL: [TaskAction; 7] = [
        TaskAction::Recover,
        TaskAction::Resume,
        TaskAction::Review,
        TaskAction::StartNextPr,
        TaskAction::Reconcile,
        TaskAction::Complete,
        TaskAction::NoAction,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recover => "recover",
            Self::Resume => "resume",
            Self::Review => "review",
            Self::StartNextPr => "start_next_pr",
            Self::Reconcile => "reconcile",
            Self::Complete => "complete",
            Self::NoAction => "no_action",
        }
    }
}

/// One action's legal status and the reason behind it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActionStatus {
    pub action: TaskAction,
    pub available: bool,
    /// Why the action is legal when available; the blocking fact when not.
    pub reason: String,
}

/// The complete legal-action model for a Task Session. All seven actions in
/// canonical order, each Legal or Blocked with a reason. `recommended` is
/// always one of the available actions, or `None` only when no session exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActionModel {
    pub recommended: Option<TaskAction>,
    pub actions: Vec<TaskActionStatus>,
}

impl TaskActionModel {
    /// No session exists: all seven blocked, no recommendation.
    pub fn no_session() -> Self {
        Self {
            recommended: None,
            actions: TaskAction::ALL
                .map(|action| TaskActionStatus {
                    action,
                    available: false,
                    reason: "no Task Session; start one with `lf task run`".to_string(),
                })
                .to_vec(),
        }
    }

    pub fn status(&self, action: TaskAction) -> Option<&TaskActionStatus> {
        self.actions.iter().find(|s| s.action == action)
    }
}

/// The state of an interaction-review gate, if one is active or recently
/// completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReviewGateState {
    Requested,
    Active,
    Approved,
    ChangesRequested,
}

/// Total evidence bundle from which the legal-action model is derived.
/// Built by the snapshot builders (which hold the durable `TaskPr`), not
/// from the wire `PrSnapshot`.
pub struct TaskActionEvidence<'a> {
    pub status: TaskSessionStatus,
    pub latest_pr_phase: Option<PrPhase>,
    pub latest_pr_after_merge: Option<AfterMerge>,
    pub latest_pr_next_slug: Option<&'a str>,
    /// The exact refusal returned by `lf task complete`, when its gate is shut.
    pub completion_refusal: Option<&'a str>,
    /// The exact refusal returned by `lf task resume`, when no active PR exists.
    pub resume_refusal: Option<&'a str>,
    pub pending_directive: bool,
    /// True when the current (pending) directive was already delivered to a body
    /// (`applied_at` set) but never incorporated. This is the reconcilable shape:
    /// the work shipped, only the acknowledgement was interrupted. A pending
    /// directive that was never applied is a post-land steer that still needs a
    /// body, so it keeps `StartNextPr` rather than `Reconcile`.
    pub directive_applied: bool,
    pub ci: Option<&'a CiObservation>,
    /// Some(true)=live, Some(false)=dead, None=process not expected for status.
    pub process_alive: Option<bool>,
    /// None=rooted on default branch; Some=parent PR phase.
    pub predecessor_phase: Option<PrPhase>,
    pub review_gate: Option<ReviewGateState>,
    pub abandon_intent: bool,
    pub local_progress_unsettled: Option<bool>,
}

/// Derive the legal-action model from total evidence. One pure function;
/// every surface consumes its output.
///
/// Precedence: review gate > active PR phase > body liveness > status.
pub fn derive_task_actions(evidence: &TaskActionEvidence) -> TaskActionModel {
    if evidence.status.is_terminal() {
        return one_action(
            TaskAction::NoAction,
            if evidence.status == TaskSessionStatus::Abandoned {
                "Task is abandoned"
            } else {
                "Task is completed"
            },
            |_| "Task is terminal".to_string(),
        );
    }
    if evidence.abandon_intent {
        return one_action(TaskAction::NoAction, "Task is being abandoned", |_| {
            "Task is being abandoned".to_string()
        });
    }

    // Review gate takes precedence over PR phase and body liveness.
    if let Some(gate) = evidence.review_gate {
        match gate {
            ReviewGateState::Requested | ReviewGateState::Active
                if evidence.latest_pr_phase != Some(PrPhase::Merged) =>
            {
                return one_action(TaskAction::Review, "awaiting review disposition", |a| {
                    match a {
                        TaskAction::Review => unreachable!(),
                        TaskAction::Resume => {
                            "review in progress; resume after the review resolves".into()
                        }
                        TaskAction::Complete => {
                            "review gate is active; complete after the review approves".into()
                        }
                        TaskAction::StartNextPr => {
                            "review gate is active; advance after the review approves".into()
                        }
                        TaskAction::Recover => "body is not dead; a review gate is active".into(),
                        TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                        TaskAction::NoAction => "action available: review the gate".into(),
                    }
                });
            }
            ReviewGateState::ChangesRequested if evidence.resume_refusal.is_none() => {
                return one_action(TaskAction::Resume, "address requested changes", |a| {
                    match a {
                        TaskAction::Resume => unreachable!(),
                        TaskAction::Review => "review returned changes; resume to fix".into(),
                        TaskAction::Complete => "review requested changes; fix them first".into(),
                        TaskAction::StartNextPr => {
                            "review requested changes; fix them first".into()
                        }
                        TaskAction::Recover => "body is not dead; resume to address changes".into(),
                        TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                        TaskAction::NoAction => {
                            "action available: address requested changes".into()
                        }
                    }
                });
            }
            ReviewGateState::Requested
            | ReviewGateState::Active
            | ReviewGateState::ChangesRequested => {}
            ReviewGateState::Approved => {}
        }
    }

    let model = match evidence.latest_pr_phase {
        Some(PrPhase::Open) => open_pr_model(evidence),
        Some(PrPhase::Publishing) => one_action(TaskAction::Resume, "retry publication", |a| {
            match a {
                TaskAction::Resume => unreachable!(),
                TaskAction::Review => "PR not yet open on GitHub".into(),
                TaskAction::Complete => "PR is not yet published".into(),
                TaskAction::StartNextPr => "PR is not yet published".into(),
                TaskAction::Recover => "body is not dead; retry publication".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::NoAction => "action available: retry publication".into(),
            }
        }),
        Some(PrPhase::Merged) => merged_pr_model(evidence),
        Some(PrPhase::Abandoned) => one_action(
            TaskAction::StartNextPr,
            "PR abandoned; start the next PR",
            |a| {
                match a {
                TaskAction::StartNextPr => unreachable!(),
                TaskAction::Review => "PR was abandoned; no open PR to review".into(),
                TaskAction::Complete => "PR was abandoned; nothing to complete".into(),
                TaskAction::Resume => evidence
                    .resume_refusal
                    .unwrap_or("PR was abandoned; no active PR to resume")
                    .into(),
                TaskAction::Recover => "body is not dead; start the next PR".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::NoAction => "action available: start the next PR".into(),
            }
            },
        ),
        Some(PrPhase::Working) => body_model(evidence),
        None if evidence.resume_refusal.is_some() => one_action(
            TaskAction::NoAction,
            evidence.resume_refusal.expect("checked above"),
            |a| {
                match a {
                TaskAction::NoAction => unreachable!(),
                TaskAction::Resume => evidence.resume_refusal.expect("checked above").into(),
                TaskAction::Recover => "no Task PR exists to recover".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::Review => "no open PR to review".into(),
                TaskAction::Complete => "no merged PR to complete from".into(),
                TaskAction::StartNextPr => "no settled PR to advance from".into(),
            }
            },
        ),
        None => body_model(evidence),
    };

    apply_predecessor_overlay(model, evidence)
}

/// Open PR: action depends on CI state.
fn open_pr_model(evidence: &TaskActionEvidence) -> TaskActionModel {
    // A head red only on land-time preconditions (`scratch-clear`) is reviewable,
    // not resumable: no repair turn can green it, and `lf pr land` clears
    // `scratch/` itself. Route it to Review exactly as a passing head — the
    // reviewer, not a doomed ci-fix body, owns it. Every Task PR is red this way
    // pre-land by construction.
    if evidence
        .ci
        .is_some_and(|ci| ci.only_land_time_preconditions())
    {
        return open_pr_reviewable("checks passed except scratch-clear; awaiting review");
    }
    match evidence.ci.map(|ci| ci.state) {
        Some(CiState::Pending) => {
            one_action(TaskAction::NoAction, "required checks still running", |a| {
                match a {
                TaskAction::NoAction => unreachable!(),
                TaskAction::Review => "checks still running".into(),
                TaskAction::Resume => "awaiting CI".into(),
                TaskAction::Complete => "PR is open, not merged".into(),
                TaskAction::StartNextPr => "PR is open, not merged".into(),
                TaskAction::Recover => "body is not dead; CI is running".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
            }
            })
        }
        Some(CiState::Failing) => one_action(
            TaskAction::Resume,
            ci_failure_reason(evidence.ci.unwrap()),
            |a| {
                match a {
                TaskAction::Resume => unreachable!(),
                TaskAction::Review => "required checks failed".into(),
                TaskAction::Complete => "PR is open, not merged".into(),
                TaskAction::StartNextPr => "PR is open, not merged".into(),
                TaskAction::Recover => "body is not dead; fix the checks".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::NoAction => "action available: fix failing required checks".into(),
            }
            },
        ),
        Some(CiState::Passing) => open_pr_reviewable("checks passed; awaiting review"),
        None => one_action(
            TaskAction::NoAction,
            "required checks have not been observed",
            |a| {
                match a {
                TaskAction::NoAction => unreachable!(),
                TaskAction::Review => "required checks have not been observed".into(),
                TaskAction::Resume => "awaiting CI evidence".into(),
                TaskAction::Complete => "PR is open, not merged".into(),
                TaskAction::StartNextPr => "PR is open, not merged".into(),
                TaskAction::Recover => "body is not dead; awaiting CI evidence".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
            }
            },
        ),
    }
}

/// An open PR whose checks admit review: recommend Review, block the rest. Shared
/// by a passing head and a head red only on land-time preconditions.
fn open_pr_reviewable(reason: &str) -> TaskActionModel {
    one_action(TaskAction::Review, reason, |a| {
        match a {
        TaskAction::Review => unreachable!(),
        TaskAction::Resume => "awaiting review; resume after review to address feedback".into(),
        TaskAction::Complete => "PR is open, not merged".into(),
        TaskAction::StartNextPr => "PR is open, not merged".into(),
        TaskAction::Recover => "body is not dead; PR is open for review".into(),
        TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
        TaskAction::NoAction => "action available: review the PR".into(),
    }
    })
}

/// Merged PR: action depends on the after-merge disposition and review gate.
fn merged_pr_model(evidence: &TaskActionEvidence) -> TaskActionModel {
    if let Some(refusal) = evidence.completion_refusal {
        // A pending directive that was already delivered to a body but never
        // incorporated is reconcilable: the work shipped in the merged head, only
        // the acknowledgement turn was interrupted. It settles by an out-of-band
        // Wave/Operator attestation, never another provider turn — so recommend
        // Reconcile and bar the provider actions. A *not-applied* pending
        // directive falls through to `StartNextPr`, which still needs a body.
        //
        // Only a merged `CompleteTask` publication is reconcilable: a merged
        // `Review`/next-PR disposition with a pending applied directive is a serial
        // successor, not a Task settling — it must keep `StartNextPr`.
        if evidence.pending_directive
            && evidence.directive_applied
            && evidence.latest_pr_after_merge == Some(AfterMerge::CompleteTask)
        {
            return one_action(TaskAction::Reconcile, refusal, |a| match a {
                TaskAction::Reconcile => unreachable!(),
                TaskAction::Complete => refusal.into(),
                TaskAction::StartNextPr => {
                    "directive already delivered; reconcile it, do not start another PR".into()
                }
                TaskAction::Resume => {
                    "PR is merged and the directive shipped; reconcile it, not resume".into()
                }
                TaskAction::Review => "merged and complete; nothing to review".into(),
                TaskAction::Recover => "body is not dead; reconcile the applied directive".into(),
                TaskAction::NoAction => {
                    "action available: attest and reconcile the applied directive".into()
                }
            });
        }
        if evidence.pending_directive
            || evidence.review_gate == Some(ReviewGateState::ChangesRequested)
        {
            return one_action(TaskAction::StartNextPr, refusal, |a| {
                match a {
                TaskAction::StartNextPr => unreachable!(),
                TaskAction::Complete => refusal.into(),
                TaskAction::Review => "merged follow-up belongs in the next PR".into(),
                TaskAction::Resume => evidence
                    .resume_refusal
                    .unwrap_or("PR is merged; no active PR to resume")
                    .into(),
                TaskAction::Recover => "body is not dead; start the next PR".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::NoAction => "action available: start the next PR".into(),
            }
            });
        }
        return one_action(TaskAction::Review, refusal, |a| {
            match a {
            TaskAction::Review => unreachable!(),
            TaskAction::Complete => refusal.into(),
            TaskAction::Resume => evidence
                .resume_refusal
                .unwrap_or("PR is merged; no active PR to resume")
                .into(),
            TaskAction::StartNextPr => "completion is blocked by a required review".into(),
            TaskAction::Recover => "body is not dead; a completion gate is open".into(),
            TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
            TaskAction::NoAction => "action available: resolve the completion gate".into(),
        }
        });
    }

    match evidence.latest_pr_after_merge {
        Some(AfterMerge::CompleteTask) => {
            one_action(TaskAction::Complete, "PR merged; complete the Task", |a| {
                match a {
                TaskAction::Complete => unreachable!(),
                TaskAction::StartNextPr => "PR dispositions the Task complete".into(),
                TaskAction::Review => "PR is merged; nothing to review".into(),
                TaskAction::Resume => evidence
                    .resume_refusal
                    .unwrap_or("PR is merged; no active PR to resume")
                    .into(),
                TaskAction::Recover => "body is not dead; complete the Task".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::NoAction => "action available: complete the Task".into(),
            }
            })
        }
        Some(AfterMerge::Review) => match evidence.review_gate {
            Some(ReviewGateState::Approved) => {
                if evidence.latest_pr_next_slug.is_some() {
                    one_action(
                        TaskAction::StartNextPr,
                        "merged and reviewed; start the next PR",
                        |a| {
                            match a {
                            TaskAction::StartNextPr => unreachable!(),
                            TaskAction::Complete => "next PR is queued; start it instead".into(),
                            TaskAction::Review => "post-merge review is approved".into(),
                            TaskAction::Resume => evidence
                                .resume_refusal
                                .unwrap_or("PR is merged; no active PR to resume")
                                .into(),
                            TaskAction::Recover => "body is not dead; start the next PR".into(),
                            TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                            TaskAction::NoAction => "action available: start the next PR".into(),
                        }
                        },
                    )
                } else {
                    one_action(
                        TaskAction::Complete,
                        "merged and reviewed; complete the Task",
                        |a| {
                            match a {
                            TaskAction::Complete => unreachable!(),
                            TaskAction::StartNextPr => {
                                "no next PR queued; complete the Task".into()
                            }
                            TaskAction::Review => "post-merge review is approved".into(),
                            TaskAction::Resume => evidence
                                .resume_refusal
                                .unwrap_or("PR is merged; no active PR to resume")
                                .into(),
                            TaskAction::Recover => "body is not dead; complete the Task".into(),
                            TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                            TaskAction::NoAction => "action available: complete the Task".into(),
                        }
                        },
                    )
                }
            }
            _ => one_action(
                TaskAction::Review,
                "merged; answer the post-merge review",
                |a| {
                    match a {
                    TaskAction::Review => unreachable!(),
                    TaskAction::Resume => evidence
                        .resume_refusal
                        .unwrap_or("PR is merged; no active PR to resume")
                        .into(),
                    TaskAction::Complete => "post-merge review is not yet approved".into(),
                    TaskAction::StartNextPr => "post-merge review is not yet approved".into(),
                    TaskAction::Recover => "body is not dead; a post-merge review is active".into(),
                    TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                    TaskAction::NoAction => "action available: answer the post-merge review".into(),
                }
                },
            ),
        },
        None => one_action(TaskAction::Complete, "PR merged; complete the Task", |a| {
            match a {
                TaskAction::Complete => unreachable!(),
                TaskAction::StartNextPr => "no next PR queued".into(),
                TaskAction::Review => "PR is merged; nothing to review".into(),
                TaskAction::Resume => "PR is merged; complete the Task".into(),
                TaskAction::Recover => "body is not dead; complete the Task".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::NoAction => "action available: complete the Task".into(),
            }
        }),
    }
}

/// No active PR or PR in Working phase: action depends on body liveness.
fn body_model(evidence: &TaskActionEvidence) -> TaskActionModel {
    match evidence.process_alive {
        Some(true) => one_action(TaskAction::NoAction, "Task body is working", |a| {
            match a {
            TaskAction::NoAction => unreachable!(),
            TaskAction::Recover => "body is alive; use attach/interrupt to interact".into(),
            TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
            TaskAction::Resume => "body is working; nothing to resume".into(),
            TaskAction::Review => "no open PR to review".into(),
            TaskAction::Complete => "implementation not finished".into(),
            TaskAction::StartNextPr => "no merged PR to advance from".into(),
        }
        }),
        Some(false) => {
            let reason = if evidence.local_progress_unsettled == Some(true) {
                "Task body stopped with unsettled work; recover to continue"
            } else {
                "Task body stopped; recover to continue"
            };
            one_action(TaskAction::Recover, reason, |a| {
                match a {
                TaskAction::Recover => unreachable!(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::Resume => "no parked step to resume — recover the body first".into(),
                TaskAction::Review => "no open PR to review".into(),
                TaskAction::Complete => "implementation not finished".into(),
                TaskAction::StartNextPr => "no merged PR to advance from".into(),
                TaskAction::NoAction => "body is dead; recover to continue".into(),
            }
            })
        }
        None => one_action(TaskAction::Resume, "resume the parked session", |a| {
            match a {
                TaskAction::Resume => unreachable!(),
                TaskAction::Recover => "body is not dead; the session is parked".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::Review => "no open PR to review".into(),
                TaskAction::Complete => "implementation not finished".into(),
                TaskAction::StartNextPr => "no merged PR to advance from".into(),
                TaskAction::NoAction => "action available: resume the session".into(),
            }
        }),
    }
}

/// Apply the stack-predecessor overlay: when the active PR's parent has not
/// merged, `Complete` and `StartNextPr` are blocked. When the parent was
/// abandoned, `Resume` takes over.
fn apply_predecessor_overlay(
    model: TaskActionModel,
    evidence: &TaskActionEvidence,
) -> TaskActionModel {
    match evidence.predecessor_phase {
        Some(PrPhase::Merged) | None => model,
        Some(PrPhase::Abandoned) => one_action(
            TaskAction::Resume,
            "parent PR was abandoned; re-base or abandon this stack",
            |a| {
                match a {
                TaskAction::Resume => unreachable!(),
                TaskAction::Review => "parent PR was abandoned; re-base first".into(),
                TaskAction::Complete => "parent PR was abandoned; re-base first".into(),
                TaskAction::StartNextPr => "parent PR was abandoned; re-base first".into(),
                TaskAction::Recover => "body is not dead; re-base the stack".into(),
                TaskAction::Reconcile => "reconcile applies only to a merged, complete Task with an applied but unincorporated directive".into(),
                TaskAction::NoAction => "action available: re-base or abandon the stack".into(),
            }
            },
        ),
        Some(_) => {
            let stack_reason = "stacked on a parent PR that has not merged; land the parent first";
            let mut actions = model.actions;
            let mut recommendation_blocked = false;
            for status in &mut actions {
                if matches!(
                    status.action,
                    TaskAction::Complete | TaskAction::StartNextPr
                ) {
                    if status.available {
                        recommendation_blocked = true;
                    }
                    status.available = false;
                    status.reason = stack_reason.to_string();
                }
            }
            let recommended = if recommendation_blocked {
                for status in &mut actions {
                    if status.action == TaskAction::NoAction {
                        status.available = true;
                        status.reason = "waiting for parent PR to merge".to_string();
                    }
                }
                Some(TaskAction::NoAction)
            } else {
                model.recommended
            };
            TaskActionModel {
                recommended,
                actions,
            }
        }
    }
}

/// Build a model with one available action and five blocked.
fn one_action(
    recommended: TaskAction,
    reason: impl AsRef<str>,
    block_fn: impl Fn(TaskAction) -> String,
) -> TaskActionModel {
    let reason = reason.as_ref().to_string();
    let actions = TaskAction::ALL
        .map(|action| {
            if action == recommended {
                TaskActionStatus {
                    action,
                    available: true,
                    reason: reason.clone(),
                }
            } else {
                TaskActionStatus {
                    action,
                    available: false,
                    reason: block_fn(action),
                }
            }
        })
        .to_vec();
    TaskActionModel {
        recommended: Some(recommended),
        actions,
    }
}

/// A one-line reason naming the failing required checks.
pub fn ci_failure_reason(ci: &CiObservation) -> String {
    let names: Vec<&str> = ci
        .failing_checks
        .iter()
        .map(|check| check.name.as_str())
        .collect();
    if names.is_empty() {
        "required checks failed".to_string()
    } else {
        format!("required checks failed: {}", names.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        derive_task_actions, ReviewGateState, TaskAction, TaskActionEvidence, TaskActionModel,
    };
    use crate::task::{AfterMerge, CiCheck, CiObservation, CiState, PrPhase, TaskSessionStatus};
    use time::OffsetDateTime;

    const STATUSES: [TaskSessionStatus; 8] = [
        TaskSessionStatus::Created,
        TaskSessionStatus::Starting,
        TaskSessionStatus::Running,
        TaskSessionStatus::Waiting,
        TaskSessionStatus::Blocked,
        TaskSessionStatus::Failed,
        TaskSessionStatus::Completed,
        TaskSessionStatus::Abandoned,
    ];
    const PR_PHASES: [Option<PrPhase>; 6] = [
        None,
        Some(PrPhase::Working),
        Some(PrPhase::Publishing),
        Some(PrPhase::Open),
        Some(PrPhase::Merged),
        Some(PrPhase::Abandoned),
    ];
    const PREDECESSORS: [Option<PrPhase>; 4] = [
        None,
        Some(PrPhase::Open),
        Some(PrPhase::Merged),
        Some(PrPhase::Abandoned),
    ];
    const GATES: [Option<ReviewGateState>; 4] = [
        None,
        Some(ReviewGateState::Requested),
        Some(ReviewGateState::Approved),
        Some(ReviewGateState::ChangesRequested),
    ];

    fn ci(state: CiState) -> CiObservation {
        CiObservation {
            head_sha: "head".into(),
            state,
            failing_checks: match state {
                CiState::Failing => vec![CiCheck {
                    name: "tests-result".into(),
                    url: None,
                }],
                _ => vec![],
            },
            observed_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn evidence(status: TaskSessionStatus) -> TaskActionEvidence<'static> {
        TaskActionEvidence {
            status,
            latest_pr_phase: None,
            latest_pr_after_merge: None,
            latest_pr_next_slug: None,
            completion_refusal: None,
            resume_refusal: None,
            pending_directive: false,
            directive_applied: false,
            ci: None,
            process_alive: None,
            predecessor_phase: None,
            review_gate: None,
            abandon_intent: false,
            local_progress_unsettled: None,
        }
    }

    /// Every model must be internally coherent: all six actions present exactly
    /// once in canonical order, a recommendation that is itself available, and a
    /// non-empty reason on every entry so a blocked action always explains the
    /// blocking fact.
    fn assert_coherent(model: &TaskActionModel, case: &str) {
        let listed: Vec<TaskAction> = model.actions.iter().map(|s| s.action).collect();
        assert_eq!(
            listed,
            TaskAction::ALL.to_vec(),
            "{case}: actions must be exhaustive and in canonical order"
        );
        for status in &model.actions {
            assert!(
                !status.reason.trim().is_empty(),
                "{case}: {:?} has no reason",
                status.action
            );
        }
        match model.recommended {
            Some(action) => {
                let status = model
                    .status(action)
                    .unwrap_or_else(|| panic!("{case}: recommended action is not listed"));
                assert!(
                    status.available,
                    "{case}: recommended {action:?} is not available"
                );
                let available: Vec<TaskAction> = model
                    .actions
                    .iter()
                    .filter(|s| s.available)
                    .map(|s| s.action)
                    .collect();
                assert_eq!(
                    available,
                    vec![action],
                    "{case}: exactly the recommended action is available"
                );
            }
            None => assert!(
                model.actions.iter().all(|s| !s.available),
                "{case}: no recommendation means nothing is available"
            ),
        }
    }

    #[test]
    fn every_status_pr_predecessor_and_gate_combination_is_coherent() {
        let readings = [
            None,
            Some(CiState::Pending),
            Some(CiState::Passing),
            Some(CiState::Failing),
        ];
        let mut cases = 0;
        for status in STATUSES {
            for phase in PR_PHASES {
                for predecessor in PREDECESSORS {
                    for gate in GATES {
                        for reading in readings {
                            for after_merge in [
                                None,
                                Some(AfterMerge::Review),
                                Some(AfterMerge::CompleteTask),
                            ] {
                                for alive in [None, Some(true), Some(false)] {
                                    let observation = reading.map(ci);
                                    let mut ev = evidence(status);
                                    ev.latest_pr_phase = phase;
                                    ev.predecessor_phase = predecessor;
                                    ev.review_gate = gate;
                                    ev.ci = observation.as_ref();
                                    ev.latest_pr_after_merge = after_merge;
                                    ev.process_alive = alive;
                                    let model = derive_task_actions(&ev);
                                    assert_coherent(
                                        &model,
                                        &format!(
                                            "{status:?}/{phase:?}/parent={predecessor:?}/gate={gate:?}/ci={reading:?}/{after_merge:?}/alive={alive:?}"
                                        ),
                                    );
                                    cases += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 8 * 6 * 4 * 4 * 4 * 3 * 3);
    }

    /// The directive's named fix: a Task waiting on an open PR whose required
    /// checks pass advertises Review, never Resume.
    #[test]
    fn waiting_on_a_passing_open_pr_advertises_review_not_resume() {
        let passing = ci(CiState::Passing);
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.ci = Some(&passing);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Review));
        assert_eq!(
            model.status(TaskAction::Review).unwrap().reason,
            "checks passed; awaiting review"
        );
        let resume = model.status(TaskAction::Resume).unwrap();
        assert!(!resume.available);
        assert_eq!(
            resume.reason,
            "awaiting review; resume after review to address feedback"
        );
    }

    #[test]
    fn open_pr_ci_state_picks_between_waiting_fixing_and_reviewing() {
        let cases = [
            (CiState::Pending, TaskAction::NoAction),
            (CiState::Failing, TaskAction::Resume),
            (CiState::Passing, TaskAction::Review),
        ];
        for (state, expected) in cases {
            let observation = ci(state);
            let mut ev = evidence(TaskSessionStatus::Waiting);
            ev.latest_pr_phase = Some(PrPhase::Open);
            ev.ci = Some(&observation);
            assert_eq!(
                derive_task_actions(&ev).recommended,
                Some(expected),
                "CI {state:?} should recommend {expected:?}"
            );
        }
    }

    /// A failing observation whose failing checks are exactly `names`.
    fn ci_failing(names: &[&str]) -> CiObservation {
        CiObservation {
            head_sha: "head".into(),
            state: CiState::Failing,
            failing_checks: names
                .iter()
                .map(|name| CiCheck {
                    name: (*name).into(),
                    url: None,
                })
                .collect(),
            observed_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    /// The ENG-33 fix: a design-carrying PR red only on `scratch-clear` is
    /// reviewable. `lf pr land` greens that check itself, so a woken body could
    /// only delete the doc under review — recommend Review, never Resume, and do
    /// not block Review behind the red.
    ///
    /// Sabotage: revert the `only_land_time_preconditions` branch in
    /// `open_pr_model` and this recommends Resume again, going red here.
    #[test]
    fn open_pr_red_only_on_scratch_clear_is_reviewable_not_resumable() {
        let observation = ci_failing(&["scratch-clear"]);
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.ci = Some(&observation);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Review));
        assert!(!model.status(TaskAction::Resume).unwrap().available);
        assert_eq!(
            model.status(TaskAction::Review).unwrap().reason,
            "checks passed except scratch-clear; awaiting review"
        );
    }

    /// The other half, which must survive the fix: a real leaf still recommends
    /// Resume and still blocks Review, even alongside a land-time precondition.
    /// This test passes with the ENG-33 bug fully present — that is why the class
    /// went undetected.
    #[test]
    fn open_pr_with_a_real_leaf_still_resumes_even_beside_scratch_clear() {
        let observation = ci_failing(&["scratch-clear", "rust-test"]);
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.ci = Some(&observation);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Resume));
        assert!(!model.status(TaskAction::Review).unwrap().available);
    }

    #[test]
    fn open_pr_without_ci_evidence_waits_instead_of_advertising_review() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(
            model.status(TaskAction::NoAction).unwrap().reason,
            "required checks have not been observed"
        );
        assert_eq!(
            model.status(TaskAction::Review).unwrap().reason,
            "required checks have not been observed"
        );
    }

    #[test]
    fn failing_checks_are_named_in_the_resume_reason() {
        let failing = ci(CiState::Failing);
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.ci = Some(&failing);
        let model = derive_task_actions(&ev);
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            "required checks failed: tests-result"
        );
    }

    /// Recover is for a dead body with no PR in front of it; Resume is for a
    /// parked session with a named next step.
    #[test]
    fn dead_body_without_a_pr_recovers_rather_than_resumes() {
        let mut ev = evidence(TaskSessionStatus::Running);
        ev.latest_pr_phase = Some(PrPhase::Working);
        ev.process_alive = Some(false);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Recover));
        assert!(!model.status(TaskAction::Resume).unwrap().available);
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            "no parked step to resume — recover the body first"
        );
    }

    #[test]
    fn a_live_body_is_left_alone() {
        let mut ev = evidence(TaskSessionStatus::Running);
        ev.latest_pr_phase = Some(PrPhase::Working);
        ev.process_alive = Some(true);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(
            model.status(TaskAction::Recover).unwrap().reason,
            "body is alive; use attach/interrupt to interact"
        );
    }

    #[test]
    fn merged_pr_dispositioned_complete_recommends_completing_the_task() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::CompleteTask);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Complete));
        assert!(!model.status(TaskAction::StartNextPr).unwrap().available);
        assert_eq!(
            model.status(TaskAction::StartNextPr).unwrap().reason,
            "PR dispositions the Task complete"
        );
    }

    #[test]
    fn merged_review_blocker_names_the_gate_and_never_resumes() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::CompleteTask);
        ev.completion_refusal = Some(
            "Task W2-285 cannot complete until its gates close: required project review ir_proof is completed without approval",
        );
        ev.resume_refusal =
            Some("Task W2-285 has no active PR to resume; pull request #1037 merged");

        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Review));
        assert_eq!(
            model.status(TaskAction::Complete).unwrap().reason,
            ev.completion_refusal.unwrap()
        );
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            ev.resume_refusal.unwrap()
        );
        assert!(model
            .actions
            .iter()
            .all(|status| status.reason != "implementation not finished"));
    }

    #[test]
    fn merged_pending_directive_starts_the_serial_successor() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::CompleteTask);
        ev.completion_refusal = Some(
            "Task W2-286 cannot complete until its gates close: directive v2 is not yet incorporated; acknowledge it or re-steer before completing",
        );
        ev.resume_refusal =
            Some("Task W2-286 has no active PR to resume; pull request #1032 merged");
        ev.pending_directive = true;

        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::StartNextPr));
        assert_eq!(
            model.status(TaskAction::Complete).unwrap().reason,
            ev.completion_refusal.unwrap()
        );
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            ev.resume_refusal.unwrap()
        );
    }

    /// The named fix: a merged, complete Task whose final directive was *applied*
    /// but never incorporated recommends Reconcile — never a provider turn.
    #[test]
    fn merged_applied_but_unincorporated_directive_reconciles_not_resumes() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::CompleteTask);
        ev.completion_refusal = Some(
            "Task W2-308 cannot complete until its gates close: directive v5 is not yet incorporated; acknowledge it or re-steer before completing",
        );
        ev.resume_refusal =
            Some("Task W2-308 has no active PR to resume; pull request #1064 merged");
        ev.pending_directive = true;
        ev.directive_applied = true;

        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Reconcile));
        let start_next = model.status(TaskAction::StartNextPr).unwrap();
        assert!(!start_next.available);
        assert_eq!(
            start_next.reason,
            "directive already delivered; reconcile it, do not start another PR"
        );
        let resume = model.status(TaskAction::Resume).unwrap();
        assert!(!resume.available);
        assert_eq!(
            resume.reason,
            "PR is merged and the directive shipped; reconcile it, not resume"
        );
        assert_eq!(
            model.status(TaskAction::Complete).unwrap().reason,
            ev.completion_refusal.unwrap()
        );
    }

    /// Sabotage guard: a merged *non-`CompleteTask`* publication (a serial next-PR
    /// disposition) with a pending applied directive must never become
    /// reconcilable — it stays `StartNextPr`. Dropping the `CompleteTask` guard in
    /// `merged_pr_model` flips this to `Reconcile` and fails here.
    #[test]
    fn a_merged_review_disposition_never_reconciles_even_when_applied() {
        for disposition in [Some(AfterMerge::Review), None] {
            let mut ev = evidence(TaskSessionStatus::Waiting);
            ev.latest_pr_phase = Some(PrPhase::Merged);
            ev.latest_pr_after_merge = disposition;
            ev.completion_refusal = Some(
                "Task W2-9 cannot complete until its gates close: directive v3 is not yet incorporated; acknowledge it or re-steer before completing",
            );
            ev.resume_refusal =
                Some("Task W2-9 has no active PR to resume; pull request #9 merged");
            ev.pending_directive = true;
            ev.directive_applied = true;

            let model = derive_task_actions(&ev);
            assert_eq!(
                model.recommended,
                Some(TaskAction::StartNextPr),
                "disposition {disposition:?} must not be reconcilable"
            );
            assert!(!model.status(TaskAction::Reconcile).unwrap().available);
        }
    }

    #[test]
    fn abandoned_latest_pr_starts_next_and_never_resumes() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Abandoned);
        ev.resume_refusal =
            Some("Task W2-283 has no active PR to resume; pull request #1039 abandoned");

        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::StartNextPr));
        assert!(!model.status(TaskAction::Resume).unwrap().available);
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            ev.resume_refusal.unwrap()
        );
    }

    #[test]
    fn missing_pr_history_never_recommends_resume() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.resume_refusal =
            Some("Task W2-legacy has no active PR to resume; no PR history recorded");

        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            ev.resume_refusal.unwrap()
        );
    }

    #[test]
    fn merged_and_approved_with_a_next_slug_starts_the_next_pr() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::Review);
        ev.review_gate = Some(ReviewGateState::Approved);
        ev.latest_pr_next_slug = Some("parser-proof");
        assert_eq!(
            derive_task_actions(&ev).recommended,
            Some(TaskAction::StartNextPr)
        );

        // Same disposition with nothing queued settles the Task instead.
        ev.latest_pr_next_slug = None;
        assert_eq!(
            derive_task_actions(&ev).recommended,
            Some(TaskAction::Complete)
        );
    }

    #[test]
    fn an_unanswered_post_merge_review_gate_outranks_completion() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::Review);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Review));
        assert_eq!(
            model.status(TaskAction::Complete).unwrap().reason,
            "post-merge review is not yet approved"
        );
    }

    /// The review gate outranks PR phase, CI, and body liveness.
    #[test]
    fn an_active_review_gate_outranks_the_pr_and_ci_evidence() {
        let passing = ci(CiState::Passing);
        for gate in [ReviewGateState::Requested, ReviewGateState::Active] {
            let mut ev = evidence(TaskSessionStatus::Waiting);
            ev.latest_pr_phase = Some(PrPhase::Open);
            ev.ci = Some(&passing);
            ev.review_gate = Some(gate);
            let model = derive_task_actions(&ev);
            assert_eq!(model.recommended, Some(TaskAction::Review));
            assert_eq!(
                model.status(TaskAction::Review).unwrap().reason,
                "awaiting review disposition"
            );
        }
    }

    #[test]
    fn a_changes_requested_gate_sends_the_task_back_to_resume() {
        let passing = ci(CiState::Passing);
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.ci = Some(&passing);
        ev.review_gate = Some(ReviewGateState::ChangesRequested);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Resume));
        assert_eq!(
            model.status(TaskAction::Review).unwrap().reason,
            "review returned changes; resume to fix"
        );
    }

    /// An unmerged stack parent blocks settlement but leaves the child PR
    /// reviewable — the same fact `stacked_collapse` enforces.
    #[test]
    fn an_unmerged_stack_parent_blocks_settlement_with_the_stack_fact() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::CompleteTask);
        ev.predecessor_phase = Some(PrPhase::Open);
        let model = derive_task_actions(&ev);

        let complete = model.status(TaskAction::Complete).unwrap();
        assert!(!complete.available);
        assert_eq!(
            complete.reason,
            "stacked on a parent PR that has not merged; land the parent first"
        );
        assert_eq!(model.recommended, Some(TaskAction::NoAction));
    }

    #[test]
    fn an_unmerged_stack_parent_still_lets_the_child_pr_be_reviewed() {
        let passing = ci(CiState::Passing);
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.ci = Some(&passing);
        ev.predecessor_phase = Some(PrPhase::Open);
        assert_eq!(
            derive_task_actions(&ev).recommended,
            Some(TaskAction::Review)
        );
    }

    #[test]
    fn an_abandoned_stack_parent_asks_for_a_rebase() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.predecessor_phase = Some(PrPhase::Abandoned);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Resume));
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            "parent PR was abandoned; re-base or abandon this stack"
        );
    }

    #[test]
    fn a_merged_parent_leaves_the_child_untouched() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Merged);
        ev.latest_pr_after_merge = Some(AfterMerge::CompleteTask);
        ev.predecessor_phase = Some(PrPhase::Merged);
        assert_eq!(
            derive_task_actions(&ev).recommended,
            Some(TaskAction::Complete)
        );
    }

    #[test]
    fn a_publishing_pr_resumes_to_retry_publication() {
        let mut ev = evidence(TaskSessionStatus::Waiting);
        ev.latest_pr_phase = Some(PrPhase::Publishing);
        let model = derive_task_actions(&ev);

        assert_eq!(model.recommended, Some(TaskAction::Resume));
        assert_eq!(
            model.status(TaskAction::Review).unwrap().reason,
            "PR not yet open on GitHub"
        );
    }

    /// Terminal Tasks and Tasks being abandoned offer nothing, whatever the PR
    /// and gate evidence says.
    #[test]
    fn terminal_tasks_offer_no_action_regardless_of_pr_evidence() {
        let passing = ci(CiState::Passing);
        for status in [TaskSessionStatus::Completed, TaskSessionStatus::Abandoned] {
            let mut ev = evidence(status);
            ev.latest_pr_phase = Some(PrPhase::Open);
            ev.ci = Some(&passing);
            ev.review_gate = Some(ReviewGateState::Requested);
            let model = derive_task_actions(&ev);
            assert_eq!(model.recommended, Some(TaskAction::NoAction));
            assert_eq!(
                model.status(TaskAction::Review).unwrap().reason,
                "Task is terminal"
            );
        }
    }

    #[test]
    fn an_abandoning_task_offers_no_action() {
        let mut ev = evidence(TaskSessionStatus::Running);
        ev.latest_pr_phase = Some(PrPhase::Open);
        ev.abandon_intent = true;
        let model = derive_task_actions(&ev);
        assert_eq!(model.recommended, Some(TaskAction::NoAction));
        assert_eq!(
            model.status(TaskAction::Resume).unwrap().reason,
            "Task is being abandoned"
        );
    }

    #[test]
    fn a_task_with_no_session_recommends_nothing_and_explains_why() {
        let model = TaskActionModel::no_session();
        assert_eq!(model.recommended, None);
        assert!(model.actions.iter().all(|s| !s.available));
        assert_eq!(
            model.status(TaskAction::Recover).unwrap().reason,
            "no Task Session; start one with `lf task run`"
        );
    }
}
