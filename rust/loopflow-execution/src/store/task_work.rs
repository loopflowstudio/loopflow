//! Task membership and saved Graph positions, with observed execution state.
use crate::graph::runner::ExecutionCursor;
use crate::store::{sqlite::SqliteStore, StoreResult};
use crate::task_graph::{CapturedGraph, TaskGraph, TaskGraphPosition, TaskGraphRun};
use crate::task_work::{TaskOverview, TaskWork};
use loopflow_model::durable::TaskId;
use loopflow_model::id::LfProcessId;
use loopflow_storage::graph_process::{ExecutionStart, TaskCarrier};
use loopflow_storage::sqlite::task_work::SavedTaskGraphRow;

/// A carrier whose OS process cannot be read stays potentially running.
pub(crate) fn carrier_may_run(carrier: &TaskCarrier) -> bool {
    crate::process::os_process::carrier_evidence(carrier)
        != crate::store::ProcessIdentityEvidence::Dead
}

pub fn task_graph(store: &SqliteStore, task: &TaskId) -> StoreResult<Option<TaskGraph>> {
    store
        .task_graph_row(task)?
        .map(|row| decode_row(row).map(read_task_graph))
        .transpose()
}

pub fn task_work(store: &SqliteStore, task: &TaskId) -> StoreResult<TaskWork> {
    let records = store.task_work_records(task)?;
    Ok(TaskWork {
        sessions: records.sessions,
        graph_processes: records.graph_processes,
        graph: records
            .graph
            .map(|row| decode_row(row).map(read_task_graph))
            .transpose()?,
    })
}

pub fn task_overview(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<(TaskOverview, Vec<crate::durable::GraphProcessDetail>)> {
    let (records, details) = store.task_overview_records(task)?;
    Ok((
        TaskOverview {
            sessions: records.sessions,
            graph_processes: records.graph_processes,
            graph: records
                .graph
                .map(|row| decode_row(row).map(read_task_graph))
                .transpose()?,
        },
        details,
    ))
}

/// Task conversations keep their last settled position while a run advances.
/// A run arrives at its last step before it records its outcome, so the
/// position waits for every run to settle.
pub(crate) fn settled_task_position(
    row: Option<SavedTaskGraphRow>,
    previous: Option<&TaskGraphPosition>,
    run_in_progress: bool,
) -> StoreResult<Option<TaskGraphPosition>> {
    let Some(row) = row else {
        return Ok(None);
    };
    if run_in_progress {
        return Ok(previous.cloned());
    }
    Ok(match row.run {
        None => Some(match row.breakpoint {
            Some(record) => TaskGraphPosition::Breakpoint {
                step: row.step,
                arrival: serde_json::from_str::<BreakpointArrival>(&record)?.id,
            },
            None => TaskGraphPosition::At { step: row.step },
        }),
        Some((_, carrier)) if carrier_may_run(&carrier) => previous.cloned(),
        Some((cursor, carrier)) => {
            let graph: CapturedGraph = serde_json::from_str(&row.graph)?;
            let cursor = serde_json::from_str(&cursor)?;
            Some(TaskGraphPosition::Run {
                at: graph
                    .step_name_of(&cursor)
                    .unwrap_or_else(|| row.step.clone()),
                step: row.step,
                lf_process_id: carrier.id.to_string(),
                running: false,
            })
        }
    })
}

fn read_task_graph(row: TaskGraphRow) -> TaskGraph {
    let run = row.run.as_ref().map(|(cursor, process)| TaskGraphRun {
        cursor,
        lf_process_id: process.id.to_string(),
        running: carrier_may_run(process),
    });
    TaskGraph::read(&row.graph, row.step, row.breakpoint.as_ref(), run, row.auto)
}

#[cfg(test)]
mod tests {
    use super::read_task_graph;
    use super::TaskGraphRow;
    use crate::task_graph::{CapturedGraph, TaskGraphPosition};

    #[test]
    fn a_run_reads_running_while_its_carrier_lives() {
        let row = |pid: Option<u32>| TaskGraphRow {
            stored_graph: String::new(),
            breakpoint: None,
            graph: CapturedGraph {
                name: "proof".into(),
                steps: Vec::new(),
            },
            step: "implement".into(),
            run: Some((
                Default::default(),
                loopflow_storage::graph_process::TaskCarrier {
                    id: loopflow_model::id::LfProcessId::new(),
                    pid,
                    os_started_at: pid.map(|pid| {
                        crate::process::os_process::process_started_at(pid)
                            .unwrap()
                            .unwrap()
                    }),
                    died_at: None,
                },
            )),
            auto: None,
        };
        let running = |row| match read_task_graph(row).position {
            TaskGraphPosition::Run { running, .. } => running,
            TaskGraphPosition::Breakpoint { .. } | TaskGraphPosition::At { .. } => {
                panic!("the Task is on a run")
            }
        };
        assert!(running(row(Some(std::process::id()))));

        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let dead = row(Some(child.id()));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!running(dead));
        // Its end cleared the carrier's OS process: the run stopped.
        assert!(!running(row(None)));
    }
    #[test]
    fn moved_task_graph_json_keeps_nested_values_and_stored_bytes() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let task = loopflow_model::durable::TaskId::new();
        let carrier = loopflow_model::id::LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1)", []).unwrap();
            conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES(?1,'repo_11111111111111111111111111111111','GRAPH-1',1)", [task.as_str()]).unwrap();
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?2,1)",
                rusqlite::params![carrier, loopflow_model::id::TraceId::new()],
            )
            .unwrap();
        let record = super::TaskGraphRecord {
            breakpoint: None,
            graph: CapturedGraph {
                name: "proof".into(),
                steps: Vec::new(),
            },
            step: "review".into(),
            run_start: Some(Default::default()),
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(
            json["graph"],
            serde_json::json!({"name":"proof", "steps":[]})
        );
        super::place_moved_task_graph(&store, &task, &record, Some(&carrier)).unwrap();
        assert_eq!(
            json["run_start"],
            serde_json::to_value(record.run_start.as_ref().unwrap()).unwrap()
        );
        let saved = store.task_graph_record(&task).unwrap().unwrap();
        assert_eq!(saved.graph, serde_json::to_string(&record.graph).unwrap());
        assert_eq!(
            saved.run_start,
            Some(serde_json::to_string(record.run_start.as_ref().unwrap()).unwrap())
        );
        assert_eq!(
            super::task_graph_record(&store, &task).unwrap(),
            Some(record)
        );
        store.take_up_task_graph(&task, "{}").unwrap();
        // SQL stores valid JSON; application reads retain typed decoding failures.
        assert!(matches!(
            super::task_graph_row(&store, &task),
            Err(crate::store::StoreError::Serde(_))
        ));
        assert!(matches!(
            super::task_graph_record(&store, &task),
            Err(crate::store::StoreError::Serde(_))
        ));
    }
}

/// The Task's Graph row: the graph it took up, the step it stands at or its
/// run left, and that run with the cursor its next attempt sets out from: where
/// it began, then the step after the last one it finished.
#[derive(Debug)]
pub struct TaskGraphRow {
    stored_graph: String,
    pub breakpoint: Option<BreakpointArrival>,
    pub graph: CapturedGraph,
    pub step: String,
    pub run: Option<(ExecutionCursor, TaskCarrier)>,
    pub auto: Option<loopflow_storage::sqlite::AutoState>,
}

impl TaskGraphRow {
    /// Resolve a released wait in its original coordinates before expanding
    /// the capture and every saved cursor together. Reads alone do not write.
    pub(crate) fn prepare_continuation(&mut self) -> (bool, Option<ExecutionCursor>) {
        let mut pending = if self.run.is_none() && self.breakpoint.is_none() {
            self.graph
                .interactive_step(&self.step)
                .map(|place| place.cursor())
        } else {
            None
        };
        let converted = self.graph.expand_interactive(
            self.run
                .iter_mut()
                .map(|(cursor, _)| cursor)
                .chain(
                    self.breakpoint
                        .iter_mut()
                        .map(|arrival| &mut arrival.cursor),
                )
                .chain(pending.iter_mut()),
        );
        (converted, pending)
    }
}

/// One visit to a breakpoint. A carrier can park only once. Its globally
/// unique ID also identifies the arrival after a Graph replacement or move.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BreakpointArrival {
    pub id: LfProcessId,
    pub cursor: ExecutionCursor,
}

/// A Task's Graph as another Machine takes it up: the graph it took up, the
/// step it stands at and the cursor its run sets out from again, if it is on
/// one. The run's Process stays behind.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TaskGraphRecord {
    pub breakpoint: Option<BreakpointArrival>,
    pub graph: CapturedGraph,
    pub step: String,
    pub run_start: Option<ExecutionCursor>,
}

fn decode_row(row: SavedTaskGraphRow) -> StoreResult<TaskGraphRow> {
    Ok(TaskGraphRow {
        breakpoint: row
            .breakpoint
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?,
        graph: serde_json::from_str(&row.graph)?,
        stored_graph: row.graph,
        step: row.step,
        run: row
            .run
            .map(|(cursor, process)| serde_json::from_str(&cursor).map(|cursor| (cursor, process)))
            .transpose()
            .map_err(crate::store::StoreError::from)?,
        auto: row.auto,
    })
}
pub fn task_graph_row(store: &SqliteStore, task: &TaskId) -> StoreResult<Option<TaskGraphRow>> {
    store.task_graph_row(task)?.map(decode_row).transpose()
}
pub(crate) fn task_graph_record(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<Option<TaskGraphRecord>> {
    store
        .task_graph_record(task)?
        .map(|row| {
            Ok(TaskGraphRecord {
                breakpoint: row
                    .breakpoint
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?,
                graph: serde_json::from_str(&row.graph)?,
                step: row.step,
                run_start: row
                    .run_start
                    .as_deref()
                    .map(serde_json::from_str)
                    .transpose()?,
            })
        })
        .transpose()
}
pub(crate) fn place_moved_task_graph(
    store: &SqliteStore,
    task: &TaskId,
    record: &TaskGraphRecord,
    by: Option<&LfProcessId>,
) -> StoreResult<()> {
    store.place_moved_task_graph(
        task,
        &super::sqlite::task_work::SavedTaskGraphRecord {
            breakpoint: record
                .breakpoint
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            graph: serde_json::to_string(&record.graph)?,
            step: record.step.clone(),
            run_start: record
                .run_start
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
        },
        by,
    )
}
pub fn take_up_task_graph(
    store: &SqliteStore,
    task: &TaskId,
    graph: &CapturedGraph,
) -> StoreResult<()> {
    store.take_up_task_graph(task, &serde_json::to_string(graph)?)
}
pub fn start_task_graph_run(
    store: &SqliteStore,
    task: &TaskId,
    row: &TaskGraphRow,
    cursor: &ExecutionCursor,
    converted: Option<&CapturedGraph>,
    carrier: (&LfProcessId, &ExecutionStart),
) -> StoreResult<bool> {
    store.start_task_graph_run(
        task,
        loopflow_storage::sqlite::task_work::TaskGraphClaim {
            graph: &row.stored_graph,
            step: &row.step,
            stopped: row.run.as_ref().map(|(_, carrier)| &carrier.id),
            arrival: row.breakpoint.as_ref().map(|stop| &stop.id),
        },
        (
            &serde_json::to_string(cursor)?,
            converted.map(serde_json::to_string).transpose()?.as_deref(),
        ),
        carrier,
    )
}
pub fn advance_task_graph_run(
    store: &SqliteStore,
    task: &TaskId,
    carrier: &LfProcessId,
    cursor: &ExecutionCursor,
) -> StoreResult<()> {
    store.advance_task_graph_run(task, carrier, &serde_json::to_string(cursor)?)
}
