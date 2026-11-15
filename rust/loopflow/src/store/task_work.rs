//! Task membership and saved Graph positions, with observed execution state.
use crate::store::{
    sqlite::{task_work::TaskGraphRow, SqliteStore},
    StoreResult,
};
use crate::task_graph::{TaskGraph, TaskGraphPosition};
use crate::task_work::{TaskOverview, TaskWork};
use loopflow_model::durable::TaskId;

/// Unknown execution remains potentially running.
pub(crate) fn process_may_run(store: &SqliteStore, process: &crate::process::LfProcess) -> bool {
    crate::journal::lf_process_evidence(
        process,
        &crate::journal::recorded_process_receipt(store, process),
        crate::journal::process_identity_evidence,
    ) != crate::store::ProcessIdentityEvidence::Dead
}

pub(crate) fn task_graph(store: &SqliteStore, task: &TaskId) -> StoreResult<Option<TaskGraph>> {
    Ok(store
        .task_graph_row(task)?
        .map(|row| read_task_graph(store, row)))
}

pub fn task_work(store: &SqliteStore, task: &TaskId) -> StoreResult<TaskWork> {
    let records = store.task_work_records(task)?;
    Ok(TaskWork {
        sessions: records.sessions,
        graph_processes: records.graph_processes,
        processes: records.processes,
        graph: records.graph.map(|row| read_task_graph(store, row)),
    })
}

pub(crate) fn task_overview(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<(TaskOverview, Vec<crate::durable::GraphProcessDetail>)> {
    let (records, details) = store.task_overview_records(task)?;
    Ok((
        TaskOverview {
            sessions: records.sessions,
            graph_processes: records.graph_processes,
            graph: records.graph.map(|row| read_task_graph(store, row)),
        },
        details,
    ))
}

pub(crate) fn task_progress(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<(Option<TaskGraph>, Vec<crate::durable::GraphProcessDetail>)> {
    let (graph, details) = store.task_progress_records(task)?;
    Ok((graph.map(|row| read_task_graph(store, row)), details))
}

fn read_task_graph(store: &SqliteStore, row: TaskGraphRow) -> TaskGraph {
    let position = match row.run {
        None => TaskGraphPosition::At { step: row.step },
        Some((cursor, process)) => TaskGraphPosition::Run {
            at: row
                .graph
                .step_name_of(&cursor)
                .unwrap_or_else(|| row.step.clone()),
            step: row.step,
            lf_process_id: process.id.to_string(),
            running: process.completed_at.is_none() && process_may_run(store, &process),
        },
    };
    TaskGraph {
        name: row.graph.name.clone(),
        interactive_steps: row.graph.interactive_summary(),
        position,
    }
}

#[cfg(test)]
mod tests {
    use super::read_task_graph;
    use crate::store::sqlite::task_work::TaskGraphRow;
    use crate::task_graph::{CapturedGraph, TaskGraphPosition};

    #[test]
    fn a_saved_run_without_identity_stays_potentially_running() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let process = crate::process::LfProcess {
            id: loopflow_model::id::LfProcessId::new(),
            pid: None,
            os_started_at: None,
            trace_id: loopflow_model::id::TraceId::new(),
            parent_lf_process_id: None,
            via_agent: None,
            caller_session_id: None,
            caller_agent_process_id: None,
            command: None,
            repo: None,
            cwd: None,
            started_at: i64::MAX,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        let row = |process| TaskGraphRow {
            graph: CapturedGraph {
                name: "proof".into(),
                steps: Vec::new(),
            },
            step: "implement".into(),
            run: Some((Default::default(), process)),
        };
        let graph = read_task_graph(&store, row(process.clone()));
        assert!(matches!(
            graph.position,
            TaskGraphPosition::Run { running: true, .. }
        ));
        let mut ended = process;
        ended.completed_at = Some(10);
        let graph = read_task_graph(&store, row(ended));
        assert!(matches!(
            graph.position,
            TaskGraphPosition::Run { running: false, .. }
        ));
    }
}
