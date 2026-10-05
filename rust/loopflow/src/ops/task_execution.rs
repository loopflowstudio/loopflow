//! What a Task's most recently launched Flow is doing, read from its Execs and
//! their processes. Observation only: no Flow is selected, claimed or resumed.

use serde::{Deserialize, Serialize};

use crate::durable::TaskId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::flow_run::{FlowExecs, HANDED_OFF_EXIT};
use crate::ops::task_flow::{LatestTaskFlow, TaskFlowRecord};
use crate::session::FlowSummaryState;
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
    /// Label of the latest Flow's latest step, while that Flow is unfinished.
    pub step: Option<String>,
    /// The Session input that step captured, when it opened one.
    pub captured: Option<i64>,
}

pub(crate) async fn task_execution(
    store: &SharedStore,
    task_id: &TaskId,
) -> StoreResult<TaskExecutionSnapshot> {
    Ok(task_execution_and_flow(store, task_id).await?.0)
}

/// Execution and the Flow record read from the same Execs.
pub(crate) async fn task_execution_and_flow(
    store: &SharedStore,
    task_id: &TaskId,
) -> StoreResult<(TaskExecutionSnapshot, TaskFlowRecord)> {
    let Some(flow) = store.sqlite.task_flows(task_id)?.pop() else {
        return Ok((
            TaskExecutionSnapshot {
                state: TaskExecutionState::Idle,
                reason: "No Task Flow has run; independent Sessions may still be active"
                    .to_string(),
                step: None,
                captured: None,
            },
            TaskFlowRecord::None,
        ));
    };
    let entry = store.sqlite.flow_entry(&flow)?;
    let evidence = |exec: &crate::exec::Exec| match exec.completed_at {
        Some(_) => ProcessIdentityEvidence::Dead,
        None => crate::journal::exec_process_evidence(&store.sqlite, &exec.id),
    };
    let (_, step) = flow.latest();
    let input = store.sqlite.exec_input(&step.id)?;
    let mut snapshot = project_execution(
        &flow,
        entry.summary.state,
        evidence(&flow.driver),
        evidence(step),
    );
    snapshot.captured = input.as_ref().map(|(_, _, captured)| *captured);
    if snapshot.state == TaskExecutionState::Running {
        if let Some((_, input, captured)) = &input {
            match activity::read(&crate::store::lf_home_dir(), input).await {
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
    // An unfinished Flow is history once its Task is terminal. Independent
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
    let record = if entry.summary.state == FlowSummaryState::Completed {
        TaskFlowRecord::Finished {
            flow: entry.summary.name,
        }
    } else {
        TaskFlowRecord::Latest(LatestTaskFlow::new(flow.detail(entry), &snapshot))
    };
    Ok((snapshot, record))
}

/// A Flow runs while a process of its own does; with both gone, how its
/// latest step ended says why it stopped.
fn project_execution(
    flow: &FlowExecs,
    state: FlowSummaryState,
    driver: ProcessIdentityEvidence,
    step: ProcessIdentityEvidence,
) -> TaskExecutionSnapshot {
    let (latest, exec) = flow.latest();
    let label = &latest.label;
    if state == FlowSummaryState::Completed {
        return TaskExecutionSnapshot {
            state: TaskExecutionState::Idle,
            reason: format!("Flow {} finished; nothing further is launched", flow.name()),
            step: None,
            captured: None,
        };
    }
    let (state, reason) = match (driver, step) {
        (_, ProcessIdentityEvidence::Live) => (
            TaskExecutionState::Running,
            format!("Step Exec {} is running {label}", exec.id),
        ),
        (ProcessIdentityEvidence::Live, _) => (
            TaskExecutionState::Starting,
            format!("Flow is between steps after {label}"),
        ),
        (ProcessIdentityEvidence::Unknown, _) | (_, ProcessIdentityEvidence::Unknown) => (
            TaskExecutionState::Unknown,
            format!("Flow Execs at {label} have unknown process identity; inspect their effects before launching further work"),
        ),
        (ProcessIdentityEvidence::Dead, ProcessIdentityEvidence::Dead) => {
            if exec.exit_code == Some(i32::from(HANDED_OFF_EXIT)) {
                (
                    TaskExecutionState::Idle,
                    format!("Flow stopped at {label}; its landing is watched separately"),
                )
            } else if exec.outcome.as_deref() == Some("succeeded") {
                (
                    TaskExecutionState::Idle,
                    format!("Flow stopped after {label}; inspect its history and effects before launching fresh work"),
                )
            } else {
                let ended = match (&exec.error, exec.exit_code) {
                    (Some(error), _) => error.clone(),
                    (None, Some(code)) => format!("process exited with status {code}"),
                    (None, None) => "process left no exit record".to_string(),
                };
                (
                    TaskExecutionState::Blocked,
                    format!("{label}: {ended}. Inspect its history and effects, then launch fresh work"),
                )
            }
        }
    };
    TaskExecutionSnapshot {
        state,
        reason,
        step: Some(label.clone()),
        captured: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{project_execution, TaskExecutionState};
    use crate::journal::ProcessIdentityEvidence::{Dead, Live, Unknown};
    use crate::session::FlowSummaryState;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn a_flow_runs_while_its_processes_do_and_blocks_on_a_failed_step() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).unwrap();
        let flow = |outcome| {
            let driver = store.test_flow("feature", "/repo", &[("implement", outcome)], None);
            store.flow_execs(driver.as_str()).unwrap().unwrap().0
        };
        let state =
            |flow, driver, step| project_execution(flow, FlowSummaryState::Current, driver, step);
        let running = flow(None);
        assert_eq!(
            state(&running, Live, Live).state,
            TaskExecutionState::Running
        );
        assert_eq!(
            state(&running, Live, Dead).state,
            TaskExecutionState::Starting
        );
        assert_eq!(
            state(&running, Unknown, Dead).state,
            TaskExecutionState::Unknown
        );
        // A driver that died is history: nothing is running or resumable.
        let succeeded = flow(Some("succeeded"));
        let stopped = state(&succeeded, Dead, Dead);
        assert_eq!(stopped.state, TaskExecutionState::Idle);
        assert_eq!(stopped.step.as_deref(), Some("implement"));
        let failed = flow(Some("failed"));
        let failed = state(&failed, Dead, Dead);
        assert_eq!(failed.state, TaskExecutionState::Blocked);
        assert!(
            failed.reason.contains("launch fresh work"),
            "{}",
            failed.reason
        );
        let finished = project_execution(&succeeded, FlowSummaryState::Completed, Dead, Dead);
        assert!(finished.step.is_none());
    }
}
