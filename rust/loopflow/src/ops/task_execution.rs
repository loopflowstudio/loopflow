//! Read the existing Task Flow claim; Work's `ready` state does not mean idle.

use serde::{Deserialize, Serialize};

use crate::durable::{FlowPosition, RunId, TaskId};
use crate::engine::invocation::StepRef;
use crate::journal::{task_worker_owner_evidence, ProcessIdentityEvidence};
use crate::store::{SharedStore, StoreResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionState {
    Idle,
    Starting,
    Running,
    Human,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskExecutionSnapshot {
    pub state: TaskExecutionState,
    pub reason: String,
    pub step: Option<StepRef>,
    pub run_id: Option<RunId>,
}

pub(crate) async fn task_execution(
    store: &SharedStore,
    task_id: &TaskId,
) -> StoreResult<TaskExecutionSnapshot> {
    let position = store.flow_position(task_id).await?;
    let evidence = position
        .as_ref()
        .and_then(|position| position.claim.as_ref())
        .map(|claim| task_worker_owner_evidence(&claim.owner));
    let mut snapshot = project_execution(position.as_ref(), evidence);
    if position.is_none() {
        if let Some(crate::work::task::TaskEventKind::FlowFinished { flow, .. }) = store
            .task_events_after(task_id, 0)
            .await?
            .into_iter()
            .rev()
            .map(|event| event.kind)
            .find(|kind| matches!(kind, crate::work::task::TaskEventKind::FlowFinished { .. }))
        {
            snapshot.reason = format!("Flow {flow} finished; no further steps are scheduled");
        }
    }
    Ok(snapshot)
}

fn project_execution(
    position: Option<&FlowPosition>,
    evidence: Option<ProcessIdentityEvidence>,
) -> TaskExecutionSnapshot {
    let Some(position) = position else {
        return TaskExecutionSnapshot {
            state: TaskExecutionState::Idle,
            reason: "No active Task Flow; independent Runs may still be active".to_string(),
            step: None,
            run_id: None,
        };
    };
    let step = position.current();
    let run_id = position
        .claim
        .as_ref()
        .and_then(|claim| claim.worker_run_id.clone())
        .or_else(|| position.session_run_id.clone());
    let (state, reason) = if let Some(failure) = &position.failure {
        (TaskExecutionState::Blocked, failure.reason.clone())
    } else if position.is_human() {
        (
            TaskExecutionState::Human,
            format!("Waiting for your review at {}", step.step),
        )
    } else {
        match evidence {
            Some(ProcessIdentityEvidence::Live) if run_id.is_some() => (
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
                    "Worker liveness is unknown at {}; inspect its Run before recovery",
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
        run_id,
    }
}

#[cfg(test)]
mod tests {
    use super::{project_execution, TaskExecutionSnapshot, TaskExecutionState};
    use crate::durable::{
        test_flow_invocation, FlowPosition, RunId, TaskId, TaskWorkerClaim, TaskWorkerOwner,
    };
    use crate::id::{ExecId, TraceId};
    use crate::journal::ProcessIdentityEvidence;
    use time::OffsetDateTime;

    #[test]
    fn worker_liveness_is_separate_from_durable_ready_work() {
        let mut position = FlowPosition {
            task_id: TaskId::new(),
            invocation: test_flow_invocation("slice", 0, "implement", None, false),
            session_run_id: None,
            ready_summary: None,
            cursor: crate::engine::ExecutionCursor {
                index: 0,
                iteration: 0,
                ..Default::default()
            },
            version: 1,
            worker_generation: 0,
            claim: None,
            failure: None,

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
            worker_run_id: Some(RunId::new()),
            claimed_at: OffsetDateTime::now_utc(),
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
    }
}
