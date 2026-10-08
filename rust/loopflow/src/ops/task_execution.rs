//! What a Task's most recently launched Flow is doing, read from its Processes and
//! their processes. Observation only: no Flow is selected, claimed or resumed.

use serde::{Deserialize, Serialize};

use crate::durable::FlowProcessDetail;
use crate::durable::TaskId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::flow_process::FlowProcess;
use crate::session::FlowProcessSummaryState;
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

/// Execution and the Flow record read from the same Processes.
pub(crate) async fn task_execution_and_flow(
    store: &SharedStore,
    task_id: &TaskId,
) -> StoreResult<(TaskExecutionSnapshot, Option<FlowProcessDetail>)> {
    let Some(flow) = store.sqlite.task_flows(task_id)?.pop() else {
        return Ok((
            TaskExecutionSnapshot {
                state: TaskExecutionState::Idle,
                reason: "No Task Flow has run; independent Sessions may still be active"
                    .to_string(),
                step: None,
                captured: None,
            },
            None,
        ));
    };
    let entry = store.sqlite.flow_entry(&flow)?;
    let evidence = |process: &crate::process::Process| match process.completed_at {
        Some(_) => ProcessIdentityEvidence::Dead,
        None => crate::journal::process_evidence(&store.sqlite, &process.lfid),
    };
    let step = flow.latest().map(|step| &step.process);
    let input = match step {
        Some(step) => store.sqlite.process_input(&step.lfid)?,
        None => None,
    };
    let mut snapshot = project_execution(
        &flow,
        entry.summary.state,
        evidence(&flow.driver),
        step.map_or(ProcessIdentityEvidence::Dead, evidence),
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
    Ok((snapshot, Some(flow.detail(entry))))
