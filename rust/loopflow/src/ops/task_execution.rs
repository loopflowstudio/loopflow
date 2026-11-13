//! What a Task's most recently launched Graph is doing, read from its Processes and
//! their processes, and whether anything recorded for a Task still executes.
//! Observation only: no Graph is selected, claimed or resumed.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::durable::GraphProcessDetail;
use crate::durable::TaskId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::graph_process::GraphProcess;
use crate::process::ProcessEntry;
use crate::session::GraphProcessSummaryState;
use crate::session_record::activity::{self, Activity};
use crate::store::sqlite::{entry_of, AgentLimitRecovery, GraphExecutionFacts, SqliteStore};
use crate::store::{SharedStore, StoreResult};
use crate::work::task::Task;

use super::error::{OpsError, OpsResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionState {
    Idle,
    Starting,
    Running,
    Stalled,
    /// Its step is waiting out a usage limit until an account frees up.
    Waiting,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
            open_graph_execution(sqlite, &task.id)?.unwrap_or(TaskExecutionSnapshot {
                state: TaskExecutionState::Idle,
                reason: "No Task Graph has run; independent Sessions may still be active"
                    .to_string(),
                step: None,
                captured: None,
            }),
            None,
        ));
    };
    let entry = entry_of(graph, Some(task.id.clone()), task.wave_id.clone())?;
    let evidence = |process: &crate::process::LfProcess| match process.completed_at {
        Some(_) => ProcessIdentityEvidence::Dead,
        None => crate::journal::lf_process_evidence(
            process,
            &match facts.receipts.get(&process.id) {
                Some(path) => crate::journal::read_process_receipt(path),
                None => Ok(None),
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
    if let Some(recovery) = step.and_then(|step| facts.limit_recoveries.get(&step.id)) {
        apply_limit_recovery(&mut snapshot, recovery, &task.identifier, || {
            crate::ops::task::limit_recovery::restarts_by_itself(sqlite, &task.id).unwrap_or_else(
                |error| {
                    tracing::warn!("limit recovery for {}: {error}", task.identifier);
                    false
                },
            )
        });
    }
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
    if matches!(
        snapshot.state,
        TaskExecutionState::Idle | TaskExecutionState::Blocked
    ) {
        if let Some(open) = open_graph_execution(sqlite, &task.id)? {
            snapshot = open;
        }
    }
    Ok((snapshot, Some(graph.detail(entry))))
}

/// Execution in the Task's work can predate checkout adoption. It prevents
/// overlapping runs without making that Graph part of the Task's history.
pub(crate) fn open_graph_execution(
    sqlite: &SqliteStore,
    task: &TaskId,
) -> StoreResult<Option<TaskExecutionSnapshot>> {
    for process in sqlite.task_open_processes(task)? {
        let Some(lf) = process.as_lf() else {
            continue;
        };
        // The Graph process itself, or one of its steps.
        let graph = match lf.parent_lf_process_id.as_ref() {
            _ if sqlite.runs_graph(&lf.id)? => &lf.id,
            Some(parent) if sqlite.runs_graph(parent)? => parent,
            _ => continue,
        };
        let state = match crate::journal::process_entry_evidence(sqlite, &process) {
            ProcessIdentityEvidence::Live => TaskExecutionState::Running,
            ProcessIdentityEvidence::Unknown => TaskExecutionState::Unknown,
            ProcessIdentityEvidence::Dead => continue,
        };
        return Ok(Some(TaskExecutionSnapshot {
            state,
            reason: format!("Graph {graph} has live or unresolved execution in the Task's work; inspect `lf history show {graph}` before launching further work"),
            step: None,
            captured: None,
        }));
    }
    Ok(None)
}

/// Ends a recorded limit reason when the step moved to another agent rather
/// than another account; the agent follows.
pub(crate) const MOVED_TO: &str = "; continuing on ";

/// Ends a recorded limit reason when a provider refused the step and no
/// agent it may use was left; what would let it move follows.
pub(crate) const STOPPED: &str = "; stopped: ";

/// The limit that stopped a step and what would let it move, as one sentence.
/// The current `lf task run` does not retry such a run automatically.
pub(crate) fn limit_stop(recovery: &AgentLimitRecovery) -> Option<String> {
    let (reason, advice) = recovery.reason.rsplit_once(STOPPED)?;
    // Providers often end their own words with a period.
    let reason = reason.trim_end_matches('.');
    Some(format!("{reason}. {advice}"))
}

/// A step recovering from a limit says so. One that died waiting is where the
/// next `lf task run` starts; status promises that happens by itself only when
/// the recovery sweep would do it (`restarts_by_itself`).
fn apply_limit_recovery(
    snapshot: &mut TaskExecutionSnapshot,
    recovery: &AgentLimitRecovery,
    issue: &str,
    restarts_by_itself: impl FnOnce() -> bool,
) {
    let label = snapshot.step.as_deref().unwrap_or("Step");
    let provider = &recovery.provider;
    let reason = &recovery.reason;
    match (snapshot.state, recovery.retry_at) {
        (TaskExecutionState::Running, Some(retry_at)) => {
            snapshot.state = TaskExecutionState::Waiting;
            snapshot.reason = format!(
                "{label} is waiting for {provider} capacity until {}: {reason}. It goes on by itself.",
                crate::lf::output::format_time(retry_at)
            );
        }
        (TaskExecutionState::Running, None) => {
            snapshot.reason = match reason.rsplit_once(MOVED_TO) {
                Some((reason, agent)) => format!("{label} went on under {agent} after: {reason}"),
                None => {
                    format!("{label} went on under another {provider} account after: {reason}")
                }
            };
        }
        (TaskExecutionState::Blocked, Some(retry_at)) => {
            snapshot.state = TaskExecutionState::Waiting;
            let after = crate::lf::output::format_time(retry_at);
            snapshot.reason = if restarts_by_itself() {
                format!("{label} ended while waiting for {provider} capacity; the run starts again at it by itself after {after}")
            } else {
                format!("{label} ended while waiting for {provider} capacity until {after}; `lf task run {issue}` starts the run again at it")
            };
        }
        (TaskExecutionState::Blocked, None) => {
            if let Some(stop) = limit_stop(recovery) {
                snapshot.reason =
                    format!("{label} stopped after a provider refusal: {stop}; `lf task run {issue}` starts the run again at it");
            }
        }
        _ => {}
    }
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
    processes: &[ProcessEntry],
) -> OpsResult<Vec<String>> {
    Ok(unresolved_processes(store, processes)?
        .into_iter()
        .map(|(process, _)| {
            let id = process.id();
            format!("Process {id} has live or unresolved execution; inspect `lf history show {id}`")
        })
        .collect())
}

/// Open Processes outside the caller's own lineage that are not known dead,
/// with their liveness evidence.
pub(crate) fn unresolved_processes<'a>(
    store: &SqliteStore,
    processes: &'a [ProcessEntry],
) -> OpsResult<Vec<(&'a ProcessEntry, ProcessIdentityEvidence)>> {
    let mut lineage = HashSet::new();
    let mut next = crate::journal::current_lf_process_id();
    while let Some(id) = next.filter(|id| lineage.insert(id.clone())) {
        next = store
            .lf_process(&id)
            .map_err(error)?
            .and_then(|process| process.parent_lf_process_id);
    }
    Ok(processes
        .iter()
        .filter(|process| {
            process.completed_at().is_none()
                && !process
                    .as_lf()
                    .is_some_and(|process| lineage.contains(&process.id))
        })
        .map(|process| {
            (
                process,
                crate::journal::process_entry_evidence(store, process),
            )
        })
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
    use super::{
        apply_limit_recovery, project_execution, TaskExecutionSnapshot, TaskExecutionState,
    };
    use crate::journal::ProcessIdentityEvidence::{Dead, Live, Unknown};
    use crate::session::GraphProcessSummaryState;
    use crate::store::sqlite::AgentLimitRecovery;
    use crate::store::sqlite::SqliteStore;

    fn snapshot(state: TaskExecutionState) -> TaskExecutionSnapshot {
        TaskExecutionSnapshot {
            state,
            reason: "implement: provider_rate_limited".into(),
            step: Some("implement".into()),
            captured: None,
        }
    }

    fn recovery(retry_at: Option<i64>) -> AgentLimitRecovery {
        AgentLimitRecovery {
            provider: "claude".into(),
            reason: "rejected: five_hour".into(),
            retry_at,
            agent: None,
        }
    }

    #[test]
    fn status_tells_waiting_retrying_and_a_plain_failure_apart() {
        let mut waiting = snapshot(TaskExecutionState::Running);
        apply_limit_recovery(
            &mut waiting,
            &recovery(Some(1_900_000_000)),
            "LOO-1",
            || true,
        );
        assert_eq!(waiting.state, TaskExecutionState::Waiting);
        assert!(waiting.reason.contains("waiting for claude capacity until"));

        let mut retrying = snapshot(TaskExecutionState::Running);
        apply_limit_recovery(&mut retrying, &recovery(None), "LOO-1", || true);
        assert_eq!(retrying.state, TaskExecutionState::Running);
        assert!(retrying.reason.contains("another claude account"));

        let mut moved = snapshot(TaskExecutionState::Running);
        let codex = AgentLimitRecovery {
            provider: "codex".into(),
            reason: format!(
                "codex_error: You've hit your usage limit{}claude",
                super::MOVED_TO
            ),
            ..recovery(None)
        };
        apply_limit_recovery(&mut moved, &codex, "LOO-1", || true);
        assert_eq!(
            moved.reason,
            "implement went on under claude after: codex_error: You've hit your usage limit"
        );

        // A step that moved accounts and then failed for its own reasons failed.
        let mut failed = snapshot(TaskExecutionState::Blocked);
        apply_limit_recovery(&mut failed, &recovery(None), "LOO-1", || true);
        assert_eq!(failed.state, TaskExecutionState::Blocked);

        // A limit with nowhere to go says what would let the step move.
        let mut stopped = snapshot(TaskExecutionState::Blocked);
        let nowhere = AgentLimitRecovery {
            reason: format!(
                "codex_error: You've hit your usage limit.{}no `agents` list is set",
                super::STOPPED
            ),
            ..recovery(None)
        };
        apply_limit_recovery(&mut stopped, &nowhere, "LOO-1", || true);
        assert_eq!(stopped.state, TaskExecutionState::Blocked);
        assert_eq!(
            stopped.reason,
            "implement stopped after a provider refusal: codex_error: You've hit your usage limit. no `agents` list is set; `lf task run LOO-1` starts the run again at it"
        );

        let mut ended = snapshot(TaskExecutionState::Blocked);
        apply_limit_recovery(&mut ended, &recovery(Some(1_900_000_000)), "LOO-1", || true);
        assert_eq!(ended.state, TaskExecutionState::Waiting);
        assert!(ended.reason.contains("starts again at it by itself"));

        // An unassigned Task's run waits for someone to start it.
        let mut unassigned = snapshot(TaskExecutionState::Blocked);
        apply_limit_recovery(
            &mut unassigned,
            &recovery(Some(1_900_000_000)),
            "LOO-1",
            || false,
        );
        assert_eq!(unassigned.state, TaskExecutionState::Waiting);
        assert!(!unassigned.reason.contains("by itself"));
        assert!(unassigned
            .reason
            .contains("`lf task run LOO-1` starts the run again at it"));
    }

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
