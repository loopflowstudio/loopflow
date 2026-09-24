//! Read-only flow inspection. Persisted receipts describe progress; they never
//! authorize execution. Output and its availability remain in TaskOutputPage.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::controller::wave::playhead::{QueuedInvocation, StepKind};
use crate::durable::TaskFlowBlocker;
use crate::engine::ConcreteStep;
use crate::run_record::output::{gap, OutputGap};
use crate::store::SharedStore;
use crate::work::task::flow_history::{
    TaskFlowEvent, TaskFlowSettlement, TaskFlowStage, TaskFlowTransition,
};
use crate::work::task::{Task, TaskEventKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWatchSnapshot {
    pub task_id: String,
    pub active_stage: Option<TaskFlowStage>,
    pub invocations: Vec<TaskWatchInvocation>,
    pub runs: Vec<TaskWatchRun>,
    pub gaps: Vec<OutputGap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWatchInvocation {
    pub id: String,
    pub flow: String,
    pub stages: Vec<TaskWatchStage>,
    pub transitions: Vec<TaskWatchTransition>,
    pub settlement: Option<TaskFlowSettlement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWatchStage {
    pub step_index: u32,
    pub name: String,
    pub kind: StepKind,
    pub node_id: Option<String>,
    pub human: bool,
    pub flow_parents: Vec<String>,
    pub attempts: Vec<TaskWatchAttempt>,
}

/// Recorded progress, not provider-process liveness. Empty attempts means the
/// stage has never been observed; a bound Run need not still be running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaskWatchAttemptState {
    Entered,
    Bound,
    Ready,
    Blocked,
    Completed,
    Iterated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWatchAttempt {
    pub iteration: u32,
    pub run_id: Option<String>,
    pub state: TaskWatchAttemptState,
    pub ready_summary: Option<String>,
    pub failure: Option<TaskFlowBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWatchTransition {
    pub from: TaskFlowStage,
    pub to: TaskFlowStage,
    pub reason: TaskFlowTransition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWatchRun {
    pub run_id: String,
    /// The immutable manifest's harness label, not inferred per-attempt routing.
    pub provider: Option<String>,
    pub stage: Option<TaskFlowStage>,
}

/// Reads all retained history and manifests. Paging/discovery must be bounded
/// before this read is used for the Watch surface's one-second polling loop.
pub async fn read_task_watch(
    store: &SharedStore,
    task: &Task,
    home: &Path,
) -> Result<TaskWatchSnapshot> {
    let (position, events) = store.task_flow_history(&task.id).await?;
    let mut snapshot = TaskWatchSnapshot {
        task_id: task.id.to_string(),
        active_stage: position.as_ref().map(TaskFlowStage::from),
        invocations: Vec::new(),
        runs: Vec::new(),
        gaps: Vec::new(),
    };
    let mut bindings = BTreeMap::new();
    for event in events {
        if let TaskEventKind::Flow { event } = event.kind {
            if let TaskFlowEvent::RunBound { stage, run_id } = event.as_ref() {
                if let Some(old) = bindings.insert(run_id.to_string(), stage.clone()) {
                    anyhow::ensure!(old == *stage, "Run {run_id} has conflicting stage receipts");
                }
            }
            snapshot.record(*event);
        }
    }
    if let Some(stage) = &snapshot.active_stage {
        if !snapshot.invocations.iter().any(|invocation| {
            invocation.id == stage.invocation_id
                && invocation.stages.get(stage.step_index as usize).is_some()
        }) {
            snapshot.gaps.push(gap(
                "missing_plan",
                format!(
                    "Active invocation {} has no retained stage plan",
                    stage.invocation_id
                ),
            ));
        }
    }
    let mut seen = BTreeSet::new();
    let discovery = crate::run_record::task_run_manifests(home, task)?;
    snapshot.gaps.extend(discovery.gaps);
    for (_, manifest) in discovery.runs {
        let run_id = manifest.run_id.to_string();
        seen.insert(run_id.clone());
        snapshot.runs.push(TaskWatchRun {
            stage: bindings.get(&run_id).cloned(),
            run_id,
            provider: Some(manifest.harness),
        });
    }
    for (run_id, stage) in bindings {
        if !seen.contains(&run_id) {
            snapshot.gaps.push(gap(
                "missing_run",
                format!("Stage-bound Run {run_id} has no readable Task-attributed manifest"),
            ));
            snapshot.runs.push(TaskWatchRun {
                run_id,
                provider: None,
                stage: Some(stage),
            });
        }
    }
    Ok(snapshot)
}

impl TaskWatchInvocation {
    fn from_plan(plan: QueuedInvocation) -> Self {
        let stages = plan
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let reference = plan
                    .step_at(index as u32, 0)
                    .expect("index comes from plan steps");
                let flow_parents = match step {
                    ConcreteStep::Skill(skill) => &skill.flow_parents,
                    ConcreteStep::Op(op) => &op.flow_parents,
                    ConcreteStep::Xor(branch) => &branch.flow_parents,
                };
                TaskWatchStage {
                    step_index: reference.index,
                    name: reference.step,
                    kind: reference.kind,
                    node_id: reference.policy.id,
                    human: reference.policy.human,
                    flow_parents: flow_parents.clone(),
                    attempts: Vec::new(),
                }
            })
            .collect();
        Self {
            id: plan.id,
            flow: plan.flow,
            stages,
            transitions: Vec::new(),
            settlement: None,
        }
    }
}

impl TaskWatchAttempt {
    fn entered(iteration: u32) -> Self {
        Self {
            iteration,
            run_id: None,
            state: TaskWatchAttemptState::Entered,
            ready_summary: None,
            failure: None,
        }
    }
}

impl TaskWatchSnapshot {
    fn record(&mut self, event: TaskFlowEvent) {
        let stage = match &event {
            TaskFlowEvent::InvocationSelected { invocation } => {
                if self.invocations.iter().any(|old| old.id == invocation.id) {
                    self.gaps.push(gap(
                        "duplicate_plan",
                        format!("Invocation {} was selected more than once", invocation.id),
                    ));
                } else {
                    self.invocations
                        .push(TaskWatchInvocation::from_plan(invocation.clone()));
                }
                return;
            }
            TaskFlowEvent::Transition { from, .. } => from,
            TaskFlowEvent::StageEntered { stage }
            | TaskFlowEvent::RunBound { stage, .. }
            | TaskFlowEvent::StageReady { stage, .. }
            | TaskFlowEvent::StageBlocked { stage, .. }
            | TaskFlowEvent::InvocationSettled { stage, .. } => stage,
        }
        .clone();
        let Some(invocation) = self
            .invocations
            .iter_mut()
            .find(|invocation| invocation.id == stage.invocation_id)
        else {
            self.gaps.push(gap(
                "missing_plan",
                format!(
                    "Invocation {} has facts without its selected plan",
                    stage.invocation_id
                ),
            ));
            return;
        };
        let Some(step) = invocation.stages.get_mut(stage.step_index as usize) else {
            self.gaps.push(gap(
                "missing_stage",
                format!(
                    "Invocation {} has no recorded step {}",
                    stage.invocation_id, stage.step_index
                ),
            ));
            return;
        };
        match event {
            TaskFlowEvent::StageEntered { stage } => {
                step.attempts
                    .push(TaskWatchAttempt::entered(stage.iteration));
            }
            TaskFlowEvent::RunBound { stage, run_id } => {
                // Repeated receipts for the same Run do not create attempts or
                // regress an already-ready/blocked attempt to "bound".
                if step.attempts.iter().any(|attempt| {
                    attempt.run_id.as_deref() == Some(run_id.as_str())
                        && attempt.iteration == stage.iteration
                }) {
                    return;
                }
                if !step.attempts.last().is_some_and(|attempt| {
                    attempt.iteration == stage.iteration
                        && attempt.run_id.is_none()
                        && attempt.state == TaskWatchAttemptState::Entered
                }) {
                    step.attempts
                        .push(TaskWatchAttempt::entered(stage.iteration));
                }
                let attempt = step.attempts.last_mut().expect("attempt was inserted");
                attempt.run_id = Some(run_id.to_string());
                attempt.state = TaskWatchAttemptState::Bound;
            }
            event => {
                let iteration = stage.iteration;
                let attempt = step
                    .attempts
                    .iter_mut()
                    .rev()
                    .find(|attempt| attempt.iteration == iteration);
                match event {
                    TaskFlowEvent::StageReady { summary, .. } => {
                        if let Some(attempt) = attempt {
                            attempt.state = TaskWatchAttemptState::Ready;
                            attempt.ready_summary = Some(summary);
                        } else {
                            self.gaps.push(gap(
                                "missing_attempt",
                                "Readiness has no recorded stage entry or Run",
                            ));
                        }
                    }
                    TaskFlowEvent::StageBlocked { failure, .. } => {
                        if let Some(attempt) = attempt {
                            attempt.state = TaskWatchAttemptState::Blocked;
                            attempt.failure = Some(failure);
                        } else {
                            self.gaps.push(gap(
                                "missing_attempt",
                                "Failure has no recorded stage entry or Run",
                            ));
                        }
                    }
                    TaskFlowEvent::Transition { from, to, reason } => {
                        if reason != TaskFlowTransition::Retried {
                            if let Some(attempt) = attempt {
                                attempt.state = if reason == TaskFlowTransition::Iterated {
                                    TaskWatchAttemptState::Iterated
                                } else {
                                    TaskWatchAttemptState::Completed
                                };
                            }
                        }
                        invocation
                            .transitions
                            .push(TaskWatchTransition { from, to, reason });
                    }
                    TaskFlowEvent::InvocationSettled { outcome, .. } => {
                        if matches!(
                            outcome,
                            TaskFlowSettlement::Completed | TaskFlowSettlement::Approved
                        ) {
                            if let Some(attempt) = attempt {
                                attempt.state = TaskWatchAttemptState::Completed;
                            }
                        }
                        invocation.settlement = Some(outcome);
                    }
                    _ => unreachable!("plan, entry and binding handled above"),
                }
            }
        }
    }
}
