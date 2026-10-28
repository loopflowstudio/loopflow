//! What a Task's most recently launched Graph is doing, read from its Processes and
//! their processes, and whether anything recorded for a Task still executes.
//! Observation only: no Graph is selected, claimed or resumed.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::durable::GraphProcessDetail;
use crate::durable::TaskId;
use crate::journal::{process_evidence, ProcessIdentityEvidence};
use crate::ops::graph_process::GraphProcess;
use crate::process::LfProcess;
use crate::session::GraphProcessSummaryState;
use crate::session_record::activity::{self, Activity};
use crate::store::sqlite::{entry_of, GraphExecutionFacts, SqliteStore};
use crate::store::{SharedStore, StoreResult};
use crate::work::task::Task;

use super::error::{OpsError, OpsResult};

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
    /// Label of the latest Graph's latest step, while that Graph is unfinished.
    pub step: Option<String>,
    /// The Session input that step captured, when it opened one.
    pub captured: Option<i64>,
}

pub(crate) async fn task_execution(
    store: &SharedStore,
    task: &Task,
) -> StoreResult<TaskExecutionSnapshot> {
    let graph = store.sqlite.task_graphs(&task.id)?.pop();
    let facts = store.sqlite.graph_execution_facts(&graph)?;
    Ok(
        execution_of_latest_graph(&store.sqlite, task, graph.as_ref(), &facts)
            .await?
            .0,
    )
}

/// Execution and the Graph record of a Task whose latest Graph is already
/// read, from facts read once for every Graph of the reading.
pub(crate) async fn execution_of_latest_graph(
    sqlite: &SqliteStore,
    task: &Task,
    graph: Option<&GraphProcess>,
    facts: &GraphExecutionFacts,
) -> StoreResult<(TaskExecutionSnapshot, Option<GraphProcessDetail>)> {
    let Some(graph) = graph else {
        return Ok((
            TaskExecutionSnapshot {
                state: TaskExecutionState::Idle,
                reason: "No Task Graph has run; independent Sessions may still be active"
                    .to_string(),
                step: None,
                captured: None,
            },
            None,
        ));
    };
    let entry = entry_of(graph, Some(task.id.clone()), task.wave_id.clone())?;
    let evidence = |process: &crate::process::LfProcess| match process.completed_at {
        Some(_) => ProcessIdentityEvidence::Dead,
        None => crate::journal::recorded_process_evidence(
            process,
            &match facts.receipts.get(&process.id) {
                Some(path) if process.kind != crate::process::ProcessKind::Agent => {
                    crate::journal::read_process_receipt(path)
                }
                _ => Ok(None),
            },
            crate::journal::process_identity_evidence,
        ),
    };
    let step = graph.latest().map(|step| &step.process);
    let input = step.and_then(|step| facts.inputs.get(&step.id));
    let mut snapshot = project_execution(
        graph,
        entry.summary.state,
        evidence(&graph.process),
        step.map_or(ProcessIdentityEvidence::Dead, evidence),
    );
    snapshot.captured = input.map(|(_, _, captured)| *captured);
    if snapshot.state == TaskExecutionState::Running {
        if let (Some((session, _, captured)), Some(step)) = (input, step) {
            match activity::read(sqlite, session, step.started_at) {
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
    Ok((snapshot, Some(graph.detail(entry))))
}

/// A Graph runs while a process of its own does; with both gone, how its
/// latest step ended says why it stopped.
fn project_execution(
    graph: &GraphProcess,
    state: GraphProcessSummaryState,
    graph_process: ProcessIdentityEvidence,
    step: ProcessIdentityEvidence,
) -> TaskExecutionSnapshot {
    if state == GraphProcessSummaryState::Completed {
        return TaskExecutionSnapshot {
            state: TaskExecutionState::Idle,
            reason: format!("Graph {} finished; nothing further is launched", graph.name),
            step: None,
            captured: None,
        };
    }
    // A Graph process that has started no step is the Graph's only Process so far.
    let (label, process) = match graph.latest() {
        Some(latest) => (graph.label(latest), &latest.process),
        None => (graph.name.clone(), &graph.process),
    };
    let label = &label;
    let (state, reason) = match (graph_process, step) {
        (_, ProcessIdentityEvidence::Live) => (
            TaskExecutionState::Running,
            format!("Step Process {} is running {label}", process.id),
        ),
        (ProcessIdentityEvidence::Live, _) => (
            TaskExecutionState::Starting,
            format!("Graph is between steps after {label}"),
        ),
        (ProcessIdentityEvidence::Unknown, _) | (_, ProcessIdentityEvidence::Unknown) => (
            TaskExecutionState::Unknown,
            format!("Graph Processes at {label} have unknown process identity; inspect their effects before launching further work"),
        ),
        (ProcessIdentityEvidence::Dead, ProcessIdentityEvidence::Dead) => {
            if process.outcome.as_deref() == Some("succeeded") {
                (
                    TaskExecutionState::Idle,
                    format!("Graph stopped after {label}; inspect its history and effects before launching fresh work"),
                )
            } else {
                let ended = match (&process.error, process.exit_code) {
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

// ---- Whether anything is executing

fn error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// Whether anything is executing is asked only of the OS, about recorded
/// Processes. Unknown evidence blocks. The caller's lineage waits on the caller,
/// so it never does; that exempts waiting and grants no authority.
pub(crate) fn execution_blockers(
    store: &SqliteStore,
    processes: &[LfProcess],
) -> OpsResult<Vec<String>> {
    Ok(unresolved_processes(store, processes)?
        .into_iter()
        .map(|(process, _)| {
            format!(
                "Process {} has live or unresolved execution; inspect `lf history show {}`",
                process.id, process.id
            )
        })
        .collect())
}

/// Open Processes outside the caller's own lineage that are not known dead,
/// with their liveness evidence.
pub(crate) fn unresolved_processes<'a>(
    store: &SqliteStore,
    processes: &'a [LfProcess],
) -> OpsResult<Vec<(&'a LfProcess, ProcessIdentityEvidence)>> {
    let mut lineage = HashSet::new();
    let mut next = crate::journal::current_lf_process_id();
    while let Some(id) = next.filter(|id| lineage.insert(id.clone())) {
        next = store
            .process(&id)
            .map_err(error)?
            .and_then(|process| process.parent_lf_process_id);
    }
    Ok(processes
        .iter()
        .filter(|process| process.completed_at.is_none() && !lineage.contains(&process.id))
        .map(|process| (process, process_evidence(store, &process.id)))
        .filter(|(_, evidence)| *evidence != ProcessIdentityEvidence::Dead)
        .collect())
}

/// A Task's work is every Process in its checkout or bound through a Session.
pub(crate) fn task_execution_blockers(
    store: &SqliteStore,
    task: &TaskId,
) -> OpsResult<Vec<String>> {
    let processes = store.task_open_processes(task).map_err(error)?;
    execution_blockers(store, &processes)
}

#[cfg(test)]
mod tests {
    use super::{project_execution, TaskExecutionState};
    use crate::journal::ProcessIdentityEvidence::{Dead, Live, Unknown};
    use crate::session::GraphProcessSummaryState;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn a_graph_runs_while_its_processes_do_and_blocks_on_a_failed_step() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).unwrap();
        let graph = |outcome| {
            let graph_process =
                store.test_graph("feature", "/repo", &[("implement", outcome)], None);
            store
                .graph_process(graph_process.as_str())
                .unwrap()
                .unwrap()
                .0
        };
        let state = |graph, graph_process, step| {
            project_execution(
                graph,
                GraphProcessSummaryState::Current,
                graph_process,
                step,
            )
        };
        let running = graph(None);
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
        // A Graph process that died is history: nothing is running or resumable.
        let succeeded = graph(Some("succeeded"));
        let stopped = state(&succeeded, Dead, Dead);
        assert_eq!(stopped.state, TaskExecutionState::Idle);
        assert_eq!(stopped.step.as_deref(), Some("implement"));
        let failed = graph(Some("failed"));
        let failed = state(&failed, Dead, Dead);
        assert_eq!(failed.state, TaskExecutionState::Blocked);
        assert!(
            failed.reason.contains("launch fresh work"),
            "{}",
            failed.reason
        );
        let finished =
            project_execution(&succeeded, GraphProcessSummaryState::Completed, Dead, Dead);
        assert!(finished.step.is_none());
    }
}
