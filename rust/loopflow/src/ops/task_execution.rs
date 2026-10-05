//! What a Task's most recently launched Flow is doing, read from its row and
//! its processes. Observation only: no Flow is selected, claimed or resumed.

use serde::{Deserialize, Serialize};

use crate::durable::{FlowSession, TaskId};
use crate::engine::invocation::StepRef;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::task_flow::{LatestTaskFlow, TaskFlowRecord};
use crate::session_record::activity::{self, Activity};
use crate::store::{SharedStore, StoreResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionState {
    Idle,
    Starting,
    Running,
    Stalled,
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

/// Execution and the Flow record read from the same row.
pub(crate) async fn task_execution_and_flow(
    store: &SharedStore,
    task_id: &TaskId,
) -> StoreResult<(TaskExecutionSnapshot, TaskFlowRecord)> {
    let latest = store.latest_task_flow(task_id).await?;
    let flow = latest.as_ref().filter(|flow| !flow.finished);
    let driver_live = flow.is_some_and(|flow| crate::ops::flow_run::driver_live(flow.id()));
    let mut snapshot = project_execution(flow, driver_live);
    if let Some(flow) = flow.filter(|flow| flow.failure.is_none()) {
        if let Some(exec) = store.sqlite.pending_flow_step_exec(flow.id())? {
            match crate::journal::exec_process_evidence(&store.sqlite, &exec) {
                ProcessIdentityEvidence::Live => {
                    snapshot.state = TaskExecutionState::Running;
                    snapshot.reason = format!(
                        "Step Exec {exec} is running {}",
                        flow.step_name().as_deref().unwrap_or("Flow completion")
                    );
                }
                ProcessIdentityEvidence::Unknown if !driver_live => {
                    snapshot.state = TaskExecutionState::Unknown;
                    snapshot.reason = format!("Step Exec {exec} has unknown process identity; inspect its pending effect before launching further work");
                }
                ProcessIdentityEvidence::Unknown | ProcessIdentityEvidence::Dead => {}
            }
        }
    }
    if snapshot.state == TaskExecutionState::Running {
        if let Some(attempt) = flow.and_then(|flow| flow.current_attempt.as_ref()) {
            let captured = attempt.captured;
            match activity::read(&crate::store::lf_home_dir(), &attempt.run_id).await {
                Activity::Stalled => {
                    snapshot.state = TaskExecutionState::Stalled;
                    snapshot.reason = format!("Session event {captured} is stalled: no event or sampled body/tool CPU progress for five minutes. Interrupt the Task, inspect its effects, then launch fresh work.");
                }
                Activity::Unknown => {
                    snapshot.state = TaskExecutionState::Unknown;
                    snapshot.reason = format!("Session event {captured} is active; activity samples are unavailable or stale. Inspect its Session before launching further work.");
                }
                Activity::Running => {}
            }
        }
    }
    // A retained cursor is history once its Task is terminal. Independent
    // Sessions remain discoverable through the Session inventory.
    let status = store
        .work_status(&crate::durable::WorkRef::Task(task_id.clone()))
        .await?;
    if status != crate::durable::WorkStatus::Ready {
        snapshot.state = TaskExecutionState::Idle;
        snapshot.reason = format!("Task is {status}");
        snapshot.step = None;
        snapshot.captured = None;
    }
    let record = match latest.as_ref() {
        Some(flow) if !flow.finished => {
            TaskFlowRecord::Latest(LatestTaskFlow::new(flow, &snapshot))
        }
        Some(flow) => {
            let flow = flow.invocation.flow.clone();
            snapshot.reason = format!("Flow {flow} finished; nothing further is launched");
            TaskFlowRecord::Finished { flow }
        }
        None => TaskFlowRecord::None,
    };
    Ok((snapshot, record))
}

fn project_execution(flow: Option<&FlowSession>, driver_live: bool) -> TaskExecutionSnapshot {
    let Some(flow) = flow else {
        return TaskExecutionSnapshot {
            state: TaskExecutionState::Idle,
            reason: "No unfinished Task Flow; independent Sessions may still be active".to_string(),
            step: None,
            captured: None,
        };
    };
    // The driver records the cursor past the last step before completion.
    let Some(step) = flow.current_checked() else {
        return TaskExecutionSnapshot {
            state: if driver_live {
                TaskExecutionState::Running
            } else {
                TaskExecutionState::Idle
            },
            reason: "Flow steps are complete; its driver is recording completion".into(),
            step: None,
            captured: None,
        };
    };
    let captured = flow
        .current_attempt
        .as_ref()
        .map(|attempt| attempt.captured)
        .or_else(|| flow.failure.as_ref().and_then(|failure| failure.captured));
    let (state, reason) = if let Some(failure) = &flow.failure {
        (
            TaskExecutionState::Blocked,
            format!(
                "{}{}. Inspect its history and effects, then launch fresh work",
                failure.reason,
                failure
                    .captured
                    .map(|event| format!(" (Session event {event})"))
                    .unwrap_or_default()
            ),
        )
    } else if flow.is_human() {
        (
            TaskExecutionState::Idle,
            format!(
                "Flow stopped at historical review {}; discuss it in the Task conversation and launch fresh work",
                step.step
            ),
        )
    } else if driver_live && captured.is_some() {
        (
            TaskExecutionState::Running,
            format!("Flow is running {}", step.step),
        )
    } else if driver_live {
        (
            TaskExecutionState::Starting,
            format!("Flow is starting {}", step.step),
        )
    } else {
        (
            TaskExecutionState::Idle,
            format!(
                "Flow stopped at {}; inspect its history and effects before launching fresh work",
                step.step
            ),
        )
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
    use crate::durable::{test_flow_invocation, FlowAttempt, FlowSession, TaskFlowBlocker, TaskId};
    use time::OffsetDateTime;

    fn flow() -> FlowSession {
        FlowSession {
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
            failure: None,
            finished: false,
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn completed_steps_remain_readable_until_the_driver_records_completion() {
        let mut flow = flow();
        flow.cursor.index = flow.invocation.steps.len();
        let snapshot = project_execution(Some(&flow), true);
        assert_eq!(snapshot.state, TaskExecutionState::Running);
        assert!(snapshot.step.is_none());
        assert_eq!(
            project_execution(Some(&flow), false).state,
            TaskExecutionState::Idle
        );
    }

    #[test]
    fn a_dead_driver_leaves_history_not_a_position_to_resume() {
        let mut flow = flow();
        assert_eq!(
            project_execution(None, false).state,
            TaskExecutionState::Idle
        );
        assert_eq!(
            project_execution(Some(&flow), true).state,
            TaskExecutionState::Starting
        );
        flow.current_attempt = Some(FlowAttempt {
            captured: 1,
            run_id: crate::session_record::new_artifact_key(),
            published: true,
            outcome: None,
        });
        assert_eq!(
            project_execution(Some(&flow), true).state,
            TaskExecutionState::Running
        );
        let stopped = project_execution(Some(&flow), false);
        assert_eq!(stopped.state, TaskExecutionState::Idle);
        assert!(stopped.reason.contains("launching fresh work"));
        flow.failure = Some(TaskFlowBlocker::now("implement Run failed"));
        assert_eq!(
            project_execution(Some(&flow), false).state,
            TaskExecutionState::Blocked
        );
        // A legacy review position is history; it neither waits nor blocks.
        flow.failure = None;
        flow.invocation = test_flow_invocation("review", 0, "demo", Some("demo"), true);
        assert_eq!(
            project_execution(Some(&flow), false).state,
            TaskExecutionState::Idle
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
