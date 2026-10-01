//! Read the existing Task Flow claim; Work's `ready` state does not mean idle.

use serde::{Deserialize, Serialize};

use crate::durable::{FlowSession, TaskId};
use crate::engine::invocation::StepRef;
use crate::journal::{task_worker_owner_evidence, ProcessIdentityEvidence};
use crate::ops::task_flow::{PinnedTaskFlow, TaskFlowRecord};
use crate::session_record::activity::{self, Activity};
use crate::store::{SharedStore, StoreError, StoreResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionState {
    Idle,
    Starting,
    Running,
    Stalled,
    Human,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExecutionSnapshot {
    pub state: TaskExecutionState,
    pub reason: String,
    pub step: Option<StepRef>,
    pub captured: Option<i64>,
}

pub(crate) async fn task_execution(
    store: &SharedStore,
    task_id: &TaskId,
) -> StoreResult<TaskExecutionSnapshot> {
    Ok(task_execution_and_flow(store, task_id).await?.0)
}

/// Execution and the Flow record read from the same saved position.
pub(crate) async fn task_execution_and_flow(
    store: &SharedStore,
    task_id: &TaskId,
) -> StoreResult<(TaskExecutionSnapshot, TaskFlowRecord)> {
    let position = store.task_flow(task_id).await?;
    let evidence = position
        .as_ref()
        .and_then(|position| position.claim.as_ref())
        .map(|claim| task_worker_owner_evidence(&claim.owner));
    let mut snapshot = project_execution(position.as_ref(), evidence);
    if let Some(position) = position.as_ref().filter(|flow| flow.failure.is_none()) {
        if let Some(exec) = store.sqlite.pending_flow_step_exec(position.id())? {
            match crate::journal::exec_process_evidence(&store.sqlite, &exec) {
                ProcessIdentityEvidence::Live => {
                    snapshot.state = TaskExecutionState::Running;
                    snapshot.reason =
                        format!("Step Exec {exec} is running {}", position.current().step);
                }
                ProcessIdentityEvidence::Unknown => {
                    snapshot.state = TaskExecutionState::Unknown;
                    snapshot.reason = format!("Step Exec {exec} has unknown process identity; inspect its pending effect before recovery");
                }
                ProcessIdentityEvidence::Dead => {}
            }
        }
    }
    if let (TaskExecutionState::Running, Some(position)) = (snapshot.state, position.as_ref()) {
        if let Some(reason) = crate::ops::human_session::task_waiting_unblock(store, position)
            .await
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
        {
            snapshot.state = TaskExecutionState::Blocked;
            snapshot.reason = reason;
        }
    }
    if snapshot.state == TaskExecutionState::Running {
        if let Some(attempt) = position
            .as_ref()
            .and_then(|flow| flow.current_attempt.as_ref())
        {
            let captured = attempt.captured;
            match activity::read(&crate::store::lf_home_dir(), &attempt.run_id).await {
                Activity::Stalled => {
                    snapshot.state = TaskExecutionState::Stalled;
                    snapshot.reason = format!("Session event {captured} is stalled: no event or sampled body/tool CPU progress for five minutes. Interrupt the Task, then resume it.");
                }
                Activity::Unknown => {
                    snapshot.state = TaskExecutionState::Unknown;
                    snapshot.reason = format!("Session event {captured} is active; activity samples are unavailable or stale. Inspect its Session before recovery.");
                }
                Activity::Running => {}
            }
        }
    }
    if let Some(failure) = position
        .as_ref()
        .and_then(|position| position.failure.as_ref())
    {
        let recovery = if failure.restart_required {
            "Only Stop & restart can clear this blocker".to_string()
        } else {
            let task = store.get_task(task_id).await?.ok_or(StoreError::NotFound)?;
            format!("Complete the unblock Session, then run `lf --task {} flow start`. If this blocker has no Session, supply `--reason \"<what changed>\"` after correcting it.", task.plan.identifier)
        };
        snapshot.reason.push_str(&format!(". {recovery}"));
    }
    let record = match position.as_ref() {
        Some(position) => TaskFlowRecord::Pinned(PinnedTaskFlow::new(position, &snapshot)),
        None => match store
            .task_events_after(task_id, 0)
            .await?
            .into_iter()
            .rev()
            .map(|event| event.kind)
            .find(|kind| matches!(kind, crate::work::task::TaskEventKind::FlowFinished { .. }))
        {
            Some(crate::work::task::TaskEventKind::FlowFinished { flow, .. }) => {
                snapshot.reason = format!("Flow {flow} finished; no further steps are scheduled");
                TaskFlowRecord::Finished { flow }
            }
            _ => TaskFlowRecord::None,
        },
    };
    Ok((snapshot, record))
}

fn project_execution(
    position: Option<&FlowSession>,
    evidence: Option<ProcessIdentityEvidence>,
) -> TaskExecutionSnapshot {
    let Some(position) = position else {
        return TaskExecutionSnapshot {
            state: TaskExecutionState::Idle,
            reason: "No active Task Flow; independent Sessions may still be active".to_string(),
            step: None,
            captured: None,
        };
    };
    let step = position.current();
    let captured = position
        .current_attempt
        .as_ref()
        .map(|attempt| attempt.captured)
        .or_else(|| {
            position
                .failure
                .as_ref()
                .and_then(|failure| failure.captured)
        });
    let (state, reason) = if let Some(failure) = &position.failure {
        (
            TaskExecutionState::Blocked,
            match &failure.captured {
                Some(event) => format!("{} (Session event {event})", failure.reason),
                None => failure.reason.clone(),
            },
        )
    } else if position.is_human() {
        (
            TaskExecutionState::Human,
            format!("Waiting for your review at {}", step.step),
        )
    } else {
        match evidence {
            Some(ProcessIdentityEvidence::Live) if captured.is_some() => (
                TaskExecutionState::Running,
                format!("Task worker is running {}", step.step),
            ),
            Some(ProcessIdentityEvidence::Live) => (
                TaskExecutionState::Starting,
                format!("Task worker is starting {}", step.step),
            ),
            Some(ProcessIdentityEvidence::Unknown) => (
                TaskExecutionState::Unknown,
                format!(
                    "Worker liveness is unknown at {}; inspect its Session before recovery",
                    step.step
                ),
            ),
            Some(ProcessIdentityEvidence::Dead) => (
                TaskExecutionState::Idle,
                format!(
                    "Task worker stopped at {}; resume through Task controls",
                    step.step
                ),
            ),
            None => (
                TaskExecutionState::Idle,
                format!(
                    "Task Flow is ready at {}; no advancement worker is claimed",
                    step.step
                ),
            ),
        }
    };
    TaskExecutionSnapshot {
        state,
        reason,
        step: Some(step),
        captured,
    }
}

#[cfg(test)]
mod tests {
    use super::{project_execution, TaskExecutionSnapshot, TaskExecutionState};
    use crate::durable::{
        test_flow_invocation, FlowAttempt, FlowSession, TaskId, TaskWorkerClaim, TaskWorkerOwner,
    };
    use crate::id::{ExecId, TraceId};
    use crate::journal::ProcessIdentityEvidence;
    use time::OffsetDateTime;

    #[test]
    fn worker_liveness_is_separate_from_durable_ready_work() {
        let mut position = FlowSession {
            invocation: test_flow_invocation("slice", 0, "implement", None, false),
            cursor: crate::engine::ExecutionCursor {
                index: 0,
                iteration: 0,
                ..Default::default()
            },
            version: 1,
            task_id: Some(TaskId::new()),
            wave_id: None,
            cwd: "/repo".into(),
            message: None,
            model: None,
            current_attempt: None,
            pending_session_id: None,
            ready_summary: None,
            worker_generation: 0,
            claim: None,
            failure: None,
            finished: false,
            updated_at: OffsetDateTime::now_utc(),
        };
        assert_eq!(
            project_execution(Some(&position), None).state,
            TaskExecutionState::Idle
        );
        assert_eq!(
            project_execution(Some(&position), Some(ProcessIdentityEvidence::Live)).state,
            TaskExecutionState::Starting
        );
        position.claim = Some(TaskWorkerClaim {
            invocation_id: position.invocation.id.clone(),
            generation: 1,
            position_version: 1,
            owner: TaskWorkerOwner {
                trace_id: TraceId::new(),
                exec_id: ExecId::new(),
                pid: 123,
                started_at: 1,
            },
            claimed_at: OffsetDateTime::now_utc(),
        });
        position.current_attempt = Some(FlowAttempt {
            captured: 1,
            run_id: crate::session_record::new_artifact_key(),
            published: true,
            outcome: None,
        });
        assert_eq!(
            project_execution(Some(&position), Some(ProcessIdentityEvidence::Live)).state,
            TaskExecutionState::Running
        );
        assert_eq!(
            project_execution(Some(&position), Some(ProcessIdentityEvidence::Unknown)).state,
            TaskExecutionState::Unknown
        );
        assert_eq!(
            project_execution(Some(&position), Some(ProcessIdentityEvidence::Dead)).state,
            TaskExecutionState::Idle
        );
        position.invocation = test_flow_invocation("review", 0, "demo", Some("demo"), true);
        assert_eq!(
            project_execution(Some(&position), None).state,
            TaskExecutionState::Human
        );
    }

    #[test]
    fn execution_fixture_round_trips_without_defaults() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_execution.json"
        ))
        .unwrap();
        let snapshot: TaskExecutionSnapshot = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(snapshot.state, TaskExecutionState::Unknown);
        assert_eq!(serde_json::to_value(snapshot).unwrap(), value);
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_execution_stalled.json"
        ))
        .unwrap();
        let snapshot: TaskExecutionSnapshot = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(snapshot.state, TaskExecutionState::Stalled);
        assert!(snapshot
            .reason
            .contains(&snapshot.captured.unwrap().to_string()));
        assert_eq!(serde_json::to_value(snapshot).unwrap(), value);
    }
}
