//! What a Task's most recently launched Graph is doing, read from its Processes and
//! their processes, and whether anything recorded for a Task still executes.
//! Observation only: no Graph is selected, claimed or resumed.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::durable::TaskState;
use crate::session_record::activity::{self, Activity};
use crate::store::{ProcessIdentityEvidence, StoreResult};
use loopflow_model::durable::TaskId;
use loopflow_model::id::{LfProcessId, LfSessionId};
use loopflow_storage::graph_process::{ExecutionResult, GraphExecution};
use loopflow_storage::sqlite::{
    AgentLimitRecovery, GraphExecutionFacts, OpenWork, SqliteStore, UnrunGraph,
};
use loopflow_storage::work::task::Task;

use super::error::{OpsError, OpsResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionState {
    Idle,
    Starting,
    Running,
    Stalled,
    /// Its step is waiting out a usage limit until an account frees up, or
    /// its run waits to be carried again after its carrier died.
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
    /// Where the run's carrier (`lf task run`) died before the run stopped;
    /// `null` for a run that runs, stopped on its own, or never started.
    #[serde(deserialize_with = "Option::deserialize")]
    pub died: Option<RunDied>,
    /// What happens next to a run that died, as this Machine reads it: the
    /// server here starts it again at `at`, or leaves it to `command` for
    /// `reason`. `null` unless `died` says it did.
    #[serde(deserialize_with = "Option::deserialize")]
    pub outlook: Option<crate::ops::human_session::SessionOutlook>,
}

/// A Task run's carrier died while the run was at a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunDied {
    /// The step the run reached, where it starts again.
    pub at_step: String,
    /// RFC 3339; `null` when nobody recorded the death.
    #[serde(deserialize_with = "Option::deserialize")]
    pub at: Option<String>,
}

pub fn task_execution(
    sqlite: &SqliteStore,
    task: &Task,
    status: TaskState,
) -> StoreResult<TaskExecutionSnapshot> {
    let graph = sqlite.latest_task_graphs(&[&task.id])?.remove(&task.id);
    let facts = sqlite.graph_execution_facts(&graph, &[&task.id])?;
    execution_of_latest_graph(sqlite, task, status, graph.as_ref(), &facts)
}

/// Execution of a Task whose latest Graph is already
/// read, from facts read once for every Graph of the reading.
pub fn execution_of_latest_graph(
    sqlite: &SqliteStore,
    task: &Task,
    status: TaskState,
    graph: Option<&GraphExecution>,
    facts: &GraphExecutionFacts,
) -> StoreResult<TaskExecutionSnapshot> {
    let Some(graph) = graph else {
        return Ok(open_graph_execution_from_facts(&task.id, facts)
            .unwrap_or_else(|| unrun_execution(&task.identifier, facts.unrun.get(&task.id))));
    };
    let step = graph.latest().map(|step| &step.run);
    let input = step.and_then(|step| facts.inputs.get(&step.id));
    let mut snapshot = project_execution(
        graph,
        crate::process::RecordedProcess::from(&graph.run).evidence(),
        step.map_or(ProcessIdentityEvidence::Dead, |step| {
            crate::process::os_process::step_evidence(step, &graph.run)
        }),
    );
    snapshot.captured = input.map(|(_, _, captured)| *captured);
    if snapshot.state == TaskExecutionState::Blocked {
        if let Some(question) = step.and_then(|step| facts.unanswered_questions.get(&step.id)) {
            let label = snapshot.step.as_deref().unwrap_or("Step");
            snapshot.reason = format!(
                "{label} stopped: {question}; {}",
                question_advice(&task.identifier)
            );
        }
    }
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
        if let Some(open) = open_graph_execution_from_facts(&task.id, facts) {
            snapshot = open;
        }
    }
    if matches!(
        snapshot.state,
        TaskExecutionState::Idle | TaskExecutionState::Blocked
    ) && facts.tasks_with_carrier_records.contains(&task.id)
    {
        if let Some(died) = crate::ops::task::run_recovery::died_run(sqlite, &task.id)? {
            let start = died.server_start(
                task.worktree
                    .as_deref()
                    .is_some_and(std::path::Path::is_dir),
                || crate::ops::recovery::claimed_here(sqlite, &task.id),
                crate::ops::recovery::server_runs,
            );
            let (outlook, line) =
                died.outlook(&task.identifier, loopflow_storage::rows::now_unix(), start);
            snapshot.state = match start {
                crate::ops::recovery::ServerStart::At(_) => TaskExecutionState::Waiting,
                crate::ops::recovery::ServerStart::Left(_) => TaskExecutionState::Blocked,
            };
            snapshot.reason = line;
            snapshot.died = Some(RunDied {
                at_step: died.at_step,
                at: died.at.and_then(loopflow_harness::transcript::rfc3339),
            });
            snapshot.outlook = Some(outlook);
        }
    }
    // Keep a live run visible while it drains. A finished Graph has no step.
    if status == TaskState::Paused && matches!(
        snapshot.state, TaskExecutionState::Idle | TaskExecutionState::Blocked | TaskExecutionState::Waiting
    ) {
        if let Some(label) = &snapshot.step {
            snapshot.reason = format!("{label} stopped when the Task was paused");
            snapshot.state = TaskExecutionState::Idle;
            snapshot.outlook = None;
        }
    }
    Ok(snapshot)
}

/// A Task with no Graph history: starting the Graph it took up, stopped
/// where its last attempt to start failed, or idle when it took up none.
fn unrun_execution(identifier: &str, unrun: Option<&UnrunGraph>) -> TaskExecutionSnapshot {
    let (state, reason) = match unrun {
        Some(UnrunGraph {
            stopped: Some(error),
            ..
        }) => (
            TaskExecutionState::Blocked,
            format!(
                "{}. Reopen the conversation or run `lf task run {identifier}` to retry.",
                error.trim_end().trim_end_matches('.')
            ),
        ),
        Some(UnrunGraph { graph, .. }) => {
            (TaskExecutionState::Starting, format!("Starting {graph}"))
        }
        None => (
            TaskExecutionState::Idle,
            "No Task Graph has run; independent Sessions may still be active".to_string(),
        ),
    };
    TaskExecutionSnapshot {
        state,
        reason,
        step: None,
        captured: None,
        died: None,
        outlook: None,
    }
}

/// Execution in the Task's work can predate checkout adoption. It prevents
/// overlapping runs without making that Graph part of the Task's history.
pub(crate) fn open_graph_execution(
    sqlite: &SqliteStore,
    task: &TaskId,
) -> StoreResult<Option<TaskExecutionSnapshot>> {
    let facts = sqlite.graph_execution_facts([], &[task])?;
    Ok(open_graph_execution_from_facts(task, &facts))
}

fn open_graph_execution_from_facts(
    task: &TaskId,
    facts: &GraphExecutionFacts,
) -> Option<TaskExecutionSnapshot> {
    for run in facts.open_graphs.get(task).into_iter().flatten() {
        let state = match crate::process::RecordedProcess::from(run).evidence() {
            ProcessIdentityEvidence::Live => TaskExecutionState::Running,
            ProcessIdentityEvidence::Unknown => TaskExecutionState::Unknown,
            ProcessIdentityEvidence::Dead => continue,
        };
        let graph = &run.id;
        return Some(TaskExecutionSnapshot {
            state,
            reason: format!("Graph {graph} has live or unresolved execution in the Task's work; inspect `lf history show {graph}` before launching further work"),
            step: None,
            captured: None,
            died: None,
            outlook: None,
        });
    }
    None
}

/// Ends a recorded limit reason when the step moved to another agent; the
/// agent follows.
pub const MOVED_TO: &str = "; continuing on ";

/// Ends a recorded limit reason when a provider refused the step and no
/// agent it may use was left; what would let it move follows.
pub const STOPPED: &str = "; stopped: ";

/// How a person answers the question that stopped Task `issue`'s step.
pub fn question_advice(issue: &str) -> String {
    format!("answer with `lf steer {issue} \"…\"`, then `lf task run {issue}`")
}

/// The limit that stopped a step and what would let it move, as one sentence.
/// The current `lf task run` does not retry such a run automatically.
pub fn limit_stop(recovery: &AgentLimitRecovery) -> Option<String> {
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
                "{label} is waiting for {provider} capacity until {}: {}. It goes on by itself.",
                crate::output::format_time(retry_at),
                reason.trim_end_matches('.')
            );
        }
        (TaskExecutionState::Running, None) => {
            if let Some((reason, agent)) = reason.rsplit_once(MOVED_TO) {
                snapshot.reason = format!("{label} went on under {agent} after: {reason}");
            }
        }
        (TaskExecutionState::Blocked, Some(retry_at)) => {
            snapshot.state = TaskExecutionState::Waiting;
            let after = crate::output::format_time(retry_at);
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
    graph: &GraphExecution,
    graph_process: ProcessIdentityEvidence,
    step: ProcessIdentityEvidence,
) -> TaskExecutionSnapshot {
    if graph.run.result() == ExecutionResult::Succeeded {
        return TaskExecutionSnapshot {
            state: TaskExecutionState::Idle,
            reason: format!("Graph {} finished; nothing further is launched", graph.name),
            step: None,
            captured: None,
            died: None,
            outlook: None,
        };
    }
    // A Graph that has started no step is its only run so far.
    let (label, process) = match graph.latest() {
        Some(latest) => (graph.label(latest), &latest.run),
        None => (graph.name.clone(), &graph.run),
    };
    let label = &label;
    let (state, reason) = match (graph_process, step) {
        (_, ProcessIdentityEvidence::Live) => (
            TaskExecutionState::Running,
            format!("Step {} is running {label}", process.id),
        ),
        (ProcessIdentityEvidence::Live, _) => (
            TaskExecutionState::Starting,
            format!("Graph is between steps after {label}"),
        ),
        (ProcessIdentityEvidence::Unknown, _) | (_, ProcessIdentityEvidence::Unknown) => (
            TaskExecutionState::Unknown,
            format!("Graph run at {label} has unknown process identity; inspect its effects before launching further work"),
        ),
        (ProcessIdentityEvidence::Dead, ProcessIdentityEvidence::Dead) => {
            if process.result() == loopflow_storage::graph_process::ExecutionResult::Succeeded {
                (
                    TaskExecutionState::Idle,
                    format!("Graph stopped after {label}; inspect its history and effects before launching fresh work"),
                )
            } else {
                let ended = match (&process.error, process.exit_code) {
                    (Some(error), _) => error.trim_end_matches('.').to_string(),
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
        died: None,
        outlook: None,
    }
}

// ---- Whether anything is executing

fn error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// Whether anything is executing is asked only of the OS, by each open
/// Session's and Graph run's own pid. Unknown evidence blocks. The caller's
/// own work waits on the caller, so it never does; that exempts waiting and
/// grants no authority.
pub(crate) fn execution_blockers(store: &SqliteStore, work: &[OpenWork]) -> OpsResult<Vec<String>> {
    Ok(unresolved_work(store, work)?
        .into_iter()
        .map(|(work, _)| {
            format!(
                "{} has live or unresolved execution; inspect `lf history show {}`",
                work.label(),
                work_id(work)
            )
        })
        .collect())
}

fn work_id(work: &OpenWork) -> String {
    match work {
        OpenWork::Session { id, .. } => id.to_string(),
        OpenWork::Graph { run, .. } => run.id.to_string(),
    }
}

/// Open work outside the caller's own that is not known dead, with its
/// liveness evidence.
pub(crate) fn unresolved_work<'a>(
    store: &SqliteStore,
    work: &'a [OpenWork],
) -> OpsResult<Vec<(&'a OpenWork, ProcessIdentityEvidence)>> {
    let own = OwnWork::of_caller(store)?;
    Ok(work
        .iter()
        .filter(|work| !own.contains(work))
        .map(|work| (work, work_evidence(store, work)))
        .filter(|(_, evidence)| *evidence != ProcessIdentityEvidence::Dead)
        .collect())
}

fn work_evidence(store: &SqliteStore, work: &OpenWork) -> ProcessIdentityEvidence {
    match work {
        OpenWork::Session { agent, .. } => crate::process::os_process::agent_evidence(store, agent),
        OpenWork::Graph { run, .. } => crate::process::RecordedProcess::from(run).evidence(),
    }
}

/// What the caller runs as and under: this lf, the lf it inherited, the
/// agent conversation that issued it and the Graph runs those are steps of.
struct OwnWork {
    processes: HashSet<LfProcessId>,
    graphs: HashSet<LfProcessId>,
    session: Option<LfSessionId>,
}

impl OwnWork {
    fn of_caller(store: &SqliteStore) -> OpsResult<Self> {
        let caller = crate::journal::agent_caller();
        let processes: HashSet<_> = [
            crate::journal::current_lf_process_id(),
            crate::journal::current_parent_lf_process_id(),
            caller
                .as_ref()
                .map(|caller| caller.origin_lf_process_id.clone()),
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut graphs = processes.clone();
        for process in &processes {
            graphs.extend(store.graph_of_step(process).map_err(error)?);
        }
        Ok(Self {
            processes,
            graphs,
            session: caller.map(|caller| caller.session_id),
        })
    }

    fn contains(&self, work: &OpenWork) -> bool {
        match work {
            OpenWork::Session { id, launcher, .. } => {
                self.session.as_ref() == Some(id)
                    || launcher
                        .as_ref()
                        .is_some_and(|launcher| self.processes.contains(launcher))
            }
            OpenWork::Graph { run, .. } => self.graphs.contains(&run.id),
        }
    }
}

/// A Task's work is every Session and Graph run in its checkout or bound to it.
pub fn task_execution_blockers(store: &SqliteStore, task: &TaskId) -> OpsResult<Vec<String>> {
    let work = store.task_open_work(task).map_err(error)?;
    execution_blockers(store, &work)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_limit_recovery, project_execution, TaskExecutionSnapshot, TaskExecutionState,
    };
        use crate::store::ProcessIdentityEvidence::{Dead, Live, Unknown};
    use loopflow_storage::sqlite::AgentLimitRecovery;

    fn snapshot(state: TaskExecutionState) -> TaskExecutionSnapshot {
        TaskExecutionSnapshot {
            state,
            reason: "implement: provider_rate_limited".into(),
            step: Some("implement".into()),
            captured: None,
            died: None,
            outlook: None,
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

        // A running step whose recovery names no move keeps its own reason.
        let mut running = snapshot(TaskExecutionState::Running);
        apply_limit_recovery(&mut running, &recovery(None), "LOO-1", || true);
        assert_eq!(running.state, TaskExecutionState::Running);
        assert_eq!(running.reason, "implement: provider_rate_limited");

        let mut moved = snapshot(TaskExecutionState::Running);
        let codex = AgentLimitRecovery {
            provider: "codex".into(),
            reason: format!(
                "codex_error: You’ve hit your usage limit{}claude",
                super::MOVED_TO
            ),
            ..recovery(None)
        };
        apply_limit_recovery(&mut moved, &codex, "LOO-1", || true);
        assert_eq!(
            moved.reason,
            "implement went on under claude after: codex_error: You’ve hit your usage limit"
        );

        // A step that recovered and then failed for its own reasons failed.
        let mut failed = snapshot(TaskExecutionState::Blocked);
        apply_limit_recovery(&mut failed, &recovery(None), "LOO-1", || true);
        assert_eq!(failed.state, TaskExecutionState::Blocked);

        // A limit with nowhere to go says what would let the step move.
        let mut stopped = snapshot(TaskExecutionState::Blocked);
        let nowhere = AgentLimitRecovery {
            reason: format!(
                "codex_error: You’ve hit your usage limit.{}no `agents` list is set",
                super::STOPPED
            ),
            ..recovery(None)
        };
        apply_limit_recovery(&mut stopped, &nowhere, "LOO-1", || true);
        assert_eq!(stopped.state, TaskExecutionState::Blocked);
        assert_eq!(
            stopped.reason,
            "implement stopped after a provider refusal: codex_error: You’ve hit your usage limit. no `agents` list is set; `lf task run LOO-1` starts the run again at it"
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
        let store = crate::store::open_sqlite(&directory.path().join("loopflow.db")).unwrap();
        let graph = |step, outcome| {
            let graph_process =
                store.test_graph("feature", "/repo", &[("implement", step)], outcome);
            store
                .graph_process(graph_process.as_str())
                .unwrap()
                .unwrap()
                .0
        };
        let running = graph(None, None);
        assert_eq!(
            project_execution(&running, Live, Live).state,
            TaskExecutionState::Running
        );
        assert_eq!(
            project_execution(&running, Live, Dead).state,
            TaskExecutionState::Starting
        );
        assert_eq!(
            project_execution(&running, Unknown, Dead).state,
            TaskExecutionState::Unknown
        );
        // A Graph process that died is history: nothing is running or resumable.
        let succeeded = graph(Some("succeeded"), None);
        let stopped = project_execution(&succeeded, Dead, Dead);
        assert_eq!(stopped.state, TaskExecutionState::Idle);
        assert_eq!(stopped.step.as_deref(), Some("implement"));
        let failed = graph(Some("failed"), None);
        let failed = project_execution(&failed, Dead, Dead);
        assert_eq!(failed.state, TaskExecutionState::Blocked);
        assert!(
            failed.reason.contains("launch fresh work"),
            "{}",
            failed.reason
        );
        let finished =
            project_execution(&graph(Some("succeeded"), Some("succeeded")), Dead, Dead);
        assert!(finished.step.is_none());
    }
}
