//! One association rule for CLI and Desktop. Membership is additive to binding;
//! it never rewrites event attribution or follows causal parent Processes.

use std::collections::HashMap;

use crate::durable::TaskId;
use crate::graph::runner::ExecutionCursor;
use crate::id::LfProcessId;
use crate::process::LfProcess;
use crate::store::StoreResult;
use crate::task_graph::{CapturedGraph, TaskGraph, TaskGraphPosition, END, START};
use crate::task_work::{TaskSession, TaskWork};

use super::SqliteStore;

fn tasks(selector: &str) -> String {
    format!("SELECT id,worktree,checkout_machine_id FROM tasks WHERE id={selector} OR issue_identifier={selector} OR external_issue_id={selector}")
}

fn checkout(cwd: &str) -> String {
    // Component boundaries avoid /repo.task matching /repo.task-other. Paths
    // remain usable after checkout removal; SQL LIKE would treat '%' as syntax.
    format!("tw.checkout_machine_id=(SELECT id FROM machines WHERE route='local') AND tw.worktree!='' AND ({cwd}=rtrim(tw.worktree,'/') OR instr({cwd},rtrim(tw.worktree,'/')||'/')=1)")
}

fn process_membership(conn: &rusqlite::Connection, process: &str) -> StoreResult<String> {
    let current = checkout(&format!("{process}.cwd"));
    // Read-only clients also inspect released databases before promotion.
    // Those schemas cannot have relocation receipts; leave them untouched.
    let relocations: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='relocated_task_processes' AND type='table')",
        [], |row| row.get(0),
    )?;
    Ok(if relocations {
        format!("(({current}) AND NOT EXISTS(SELECT 1 FROM relocated_task_processes r WHERE r.process_id={process}.id)) OR EXISTS(SELECT 1 FROM relocated_task_processes r WHERE r.task_id=tw.id AND r.process_id={process}.id)")
    } else {
        current
    })
}

fn session_membership(session: &str) -> String {
    format!(
        "{session}.primary_scope IS NULL
         AND ({session}.task_id=tw.id OR ({}))",
        checkout(&format!("{session}.cwd")),
    )
}

pub(super) fn session_ids(selector: &str) -> String {
    format!(
        "SELECT a.id FROM agent_sessions a JOIN ({}) tw ON ({})",
        tasks(selector),
        session_membership("a")
    )
}

pub(super) fn session_tasks(session: &str) -> String {
    format!(
        "SELECT tw.id FROM tasks tw WHERE {} ORDER BY tw.id",
        session_membership(session)
    )
}

pub(super) fn lf_process_ids(conn: &rusqlite::Connection, selector: &str) -> StoreResult<String> {
    // History and attachments share one query-local membership; the partial
    // index skips retained events that name no Process.
    Ok(format!("WITH members AS MATERIALIZED ({})
        SELECT ae.id FROM processes ae JOIN ({}) tw ON ({})
        UNION SELECT se.lf_process_id FROM session_events se INDEXED BY session_process_membership WHERE se.session_id IN (SELECT id FROM members) AND se.lf_process_id IS NOT NULL
        UNION SELECT p.attached_lf_process_id FROM processes p WHERE p.kind='agent' AND p.agent_session_id IN (SELECT id FROM members) AND p.attached_lf_process_id IS NOT NULL
        UNION SELECT p.id FROM processes p WHERE p.kind='agent' AND p.agent_session_id IN (SELECT id FROM members)",
        session_ids(selector), tasks(selector), process_membership(conn, "ae")?))
}

/// Prefer the deepest checkout when membership names more than one Task.
/// Explicit Session bindings use the same association as Task inventory.
pub(super) fn task_of_process(
    conn: &rusqlite::Connection,
    process: &LfProcessId,
) -> StoreResult<Option<String>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            &format!(
                "WITH members AS MATERIALIZED (
                    SELECT session_id AS id FROM session_events INDEXED BY session_process_membership
                        WHERE lf_process_id=?1
                    UNION SELECT agent_session_id FROM processes
                        WHERE kind='agent' AND attached_lf_process_id=?1 AND agent_session_id IS NOT NULL)
                 SELECT tw.id FROM tasks tw JOIN processes e ON e.id=?1
                 WHERE ({}) OR EXISTS(SELECT 1 FROM agent_sessions a
                    WHERE a.id IN (SELECT id FROM members) AND ({}))
                 ORDER BY length(tw.worktree) DESC, tw.id LIMIT 1",
                process_membership(conn, "e")?, session_membership("a")
            ),
            [process],
            |row| row.get(0),
        )
        .optional()?)
}

/// Each Task in `tasks` paired with each Graph among its work. Membership is
/// driven from the Graphs, so asking for every Task costs one reading: a Graph
/// joins the checkout that is its directory or one of its ancestors, the same
/// containment `checkout` tests from the other side.
fn task_graph_pairs(conn: &rusqlite::Connection, tasks: &str) -> StoreResult<String> {
    let relocations: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='relocated_task_processes' AND type='table')",
        [], |row| row.get(0),
    )?;
    let (unmoved, relocated) = if relocations {
        (
            "WHERE NOT EXISTS(SELECT 1 FROM relocated_task_processes r WHERE r.process_id=f.id)",
            "UNION SELECT r.task_id, r.process_id FROM relocated_task_processes r JOIN tw ON tw.id=r.task_id
                WHERE r.process_id IN (SELECT id FROM f)",
        )
    } else {
        ("", "")
    };
    Ok(format!(
        "WITH RECURSIVE tw AS MATERIALIZED ({tasks}),
            f AS MATERIALIZED (SELECT e.id, e.cwd FROM graph_processes fp JOIN processes e ON e.id=fp.lf_process_id),
            up(id, path) AS (SELECT id, cwd FROM f {unmoved}
                UNION ALL SELECT id, rtrim(rtrim(path, replace(path,'/','')),'/') FROM up WHERE path!=''),
            checkouts AS MATERIALIZED (SELECT id, rtrim(worktree,'/') AS root FROM tw
                WHERE checkout_machine_id=(SELECT id FROM machines WHERE route='local') AND worktree!=''),
            links(session_id, graph) AS (
                SELECT se.session_id, se.lf_process_id FROM session_events se INDEXED BY session_events_process
                    WHERE se.lf_process_id IN (SELECT id FROM f)
                UNION SELECT p.agent_session_id, p.attached_lf_process_id FROM processes p
                    WHERE p.kind='agent' AND p.attached_lf_process_id IN (SELECT id FROM f) AND p.agent_session_id IS NOT NULL
                UNION SELECT p.agent_session_id, p.id FROM processes p
                    WHERE p.kind='agent' AND p.id IN (SELECT id FROM f) AND p.agent_session_id IS NOT NULL)
         SELECT c.id AS task_id, up.id AS graph FROM up JOIN checkouts c ON c.root=up.path
         {relocated}
         UNION SELECT tw.id, l.graph FROM links l JOIN agent_sessions a ON a.id=l.session_id JOIN tw ON ({})",
        session_membership("a"),
    ))
}

pub(super) fn graphs_of_task(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Vec<crate::ops::graph_process::GraphProcess>> {
    super::graph_inventory::graphs_in(
        conn,
        &format!(
            "e.id IN (SELECT graph FROM ({}))",
            task_graph_pairs(conn, &tasks("?1"))?
        ),
        &[&task.as_str()],
    )
}

/// Each Task's most recently launched Graph, for every Task in one reading.
fn latest_task_graphs(
    conn: &rusqlite::Connection,
) -> StoreResult<HashMap<TaskId, crate::ops::graph_process::GraphProcess>> {
    let mut latest = HashMap::new();
    let mut query = conn.prepare(&format!(
        "SELECT task_id, graph FROM ({}) pairs
         ORDER BY (SELECT e.rowid FROM processes e WHERE e.id=pairs.graph)",
        task_graph_pairs(conn, "SELECT id,worktree,checkout_machine_id FROM tasks")?
    ))?;
    for row in query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (task, graph) = row?;
        latest.insert(task, graph);
    }
    let ids = serde_json::to_string(&latest.values().collect::<Vec<_>>())?;
    let graphs: HashMap<_, _> = super::graph_inventory::graphs_in(
        conn,
        "e.id IN (SELECT value FROM json_each(?1))",
        &[&ids],
    )?
    .into_iter()
    .map(|graph| (graph.process.id.to_string(), graph))
    .collect();
    latest
        .into_iter()
        .map(|(task, graph)| {
            let task = TaskId::parse(&task)
                .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
            let graph = graphs.get(&graph).cloned().ok_or_else(|| {
                crate::store::StoreError::InvalidData(format!("Graph {graph} has no Graph process"))
            })?;
            Ok((task, graph))
        })
        .collect()
}

/// The Task's Graph row: the graph it took up, the step it stands at or its
/// run left, and that run with the cursor it began from.
pub(crate) struct TaskGraphRow {
    pub graph: CapturedGraph,
    pub step: String,
    pub run: Option<(ExecutionCursor, LfProcess)>,
}

/// A Task's Graph as another Machine takes it up: the graph it took up, the
/// step it stands at, and the cursor its run began from, if it is on one. The
/// run's Process stays behind.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TaskGraphRecord {
    pub graph: CapturedGraph,
    pub step: String,
    pub run_start: Option<ExecutionCursor>,
}

fn task_graph_row(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<Option<TaskGraphRow>> {
    use rusqlite::OptionalExtension;
    let Some((graph, step, start, process)) = conn
        .query_row(
            "SELECT graph,step,run_start,lf_process_id FROM task_graphs WHERE task_id=?1",
            [task.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let run = match start.zip(process) {
        Some((start, process)) => Some((
            serde_json::from_str(&start)?,
            conn.query_row(
                &format!("{} WHERE e.id=?1", super::processes::PROCESS_SELECT),
                [process],
                super::processes::read_process,
            )?,
        )),
        None => None,
    };
    Ok(Some(TaskGraphRow {
        graph: serde_json::from_str(&graph)?,
        step,
        run,
    }))
}

/// Put the Task on a run from `start`, carried by `by`, if it still stands
/// where the caller read it: at `step`, with `stopped` the run that last left
/// it, if any. With `ships`, the run reaches `end` and hands the Task off,
/// granting merge authority.
#[allow(clippy::too_many_arguments)]
fn start_run_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    (step, stopped): (&str, Option<&LfProcessId>),
    start: &ExecutionCursor,
    by: &LfProcessId,
    ships: bool,
) -> StoreResult<bool> {
    if super::durable::task_state_in(tx, task)? == crate::durable::TaskState::Deferred {
        return Err(crate::store::StoreError::InvalidAuthority(
            "Task is deferred; resume it before running a Graph".into(),
        ));
    }
    let left = tx.execute(
        "UPDATE task_graphs SET run_start=?2,lf_process_id=?3,updated_at=?4
         WHERE task_id=?1 AND step=?5 AND lf_process_id IS ?6",
        rusqlite::params![
            task.as_str(),
            serde_json::to_string(start)?,
            by.as_str(),
            crate::store::rows::now_unix(),
            step,
            stopped.map(LfProcessId::as_str)
        ],
    )?;
    if left == 0 {
        return Ok(false);
    }
    // No person takes part on the rest of the graph: running it hands the Task off.
    if ships {
        ship_in(
            tx,
            task,
            by,
            "it runs to the end with no interactive step left",
        )?;
    }
    Ok(true)
}

/// A Task handed off to finish on its own: since when, this Machine's
/// process carrying it there, and how many times in a row that process was
/// started again after it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskShipping {
    pub shipped_at: i64,
    pub process: Option<LfProcessId>,
    pub restarts: u32,
}
pub(super) fn shipping_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskShipping>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT shipped_at,ship_lf_process_id,ship_restarts FROM tasks
         WHERE id=?1 AND shipped_at IS NOT NULL",
        [task.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()?
    .map(|(shipped_at, process, restarts)| shipping(shipped_at, process, restarts))
    .transpose()
}

fn shipping(shipped_at: i64, process: Option<String>, restarts: u32) -> StoreResult<TaskShipping> {
    Ok(TaskShipping {
        shipped_at,
        restarts,
        process: process
            .map(|id| {
                LfProcessId::parse(&id)
                    .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
            })
            .transpose()?,
    })
}

/// End the Task's handoff, returning what it was, when it was shipping.
pub(super) fn unship_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskShipping>> {
    let shipping = shipping_in(conn, task)?;
    if shipping.is_some() {
        super::planning_write::local(
            conn,
            &crate::work::edit::WorkEdit::for_target(
                task.as_str(),
                &[crate::work::edit::WorkValue::TaskShippedAt(None)],
            ),
        )?;
        conn.execute(
            "UPDATE tasks SET ship_lf_process_id=NULL,ship_restarts=0 WHERE id=?1",
            [task.as_str()],
        )?;
    }
    Ok(shipping)
}

fn ship_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    by: &LfProcessId,
    note: &str,
) -> StoreResult<TaskShipping> {
    let state = super::durable::task_state_in(tx, task)?;
    if state.is_terminal() {
        return Err(crate::store::StoreError::InvalidAuthority(format!(
            "Task is {state}; it cannot ship"
        )));
    }
    let shipped_at = match shipping_in(tx, task)? {
        Some(shipping) => shipping.shipped_at,
        None => {
            let now = crate::store::rows::now_unix();
            super::planning_write::local(
                tx,
                &crate::work::edit::WorkEdit::for_target(
                    task.as_str(),
                    &[crate::work::edit::WorkValue::TaskShippedAt(Some(now))],
                ),
            )?;
            super::children::insert_task_event_in(
                tx,
                task,
                &crate::work::task::TaskEventKind::Progress {
                    summary: format!("Shipping: {note}"),
                },
            )?;
            now
        }
    };
    tx.execute(
        "UPDATE tasks SET ship_lf_process_id=?2 WHERE id=?1",
        rusqlite::params![task.as_str(), by.as_str()],
    )?;
    Ok(TaskShipping {
        shipped_at,
        process: Some(by.clone()),
        restarts: shipping_in(tx, task)?.map_or(0, |shipping| shipping.restarts),
    })
}

/// The run `carrier` started stopped at `step`, or reached `end`: put the
/// Task there. A Task another run or a person moved meanwhile stays put.
/// Returns whether this put the Task at `end`, which requests completion.
fn arrive_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    carrier: &LfProcessId,
    step: &str,
) -> StoreResult<bool> {
    let arrived = tx.execute(
        "UPDATE task_graphs SET step=?2,run_start=NULL,lf_process_id=NULL,updated_at=?3
         WHERE task_id=?1 AND lf_process_id=?4",
        rusqlite::params![
            task.as_str(),
            step,
            crate::store::rows::now_unix(),
            carrier.as_str()
        ],
    )? > 0;
    let ended = arrived && step == END;
    if ended {
        // A routed path may reach end even though another path had an
        // interactive step, so launch-time prediction is not sufficient.
        if !super::durable::task_state_in(tx, task)?.is_terminal() {
            ship_in(tx, task, carrier, "its run reached the end")?;
        }
        super::children::request_completion_in(tx, task, None)?;
    }
    Ok(ended)
}

fn set_step_in(tx: &rusqlite::Transaction<'_>, task: &TaskId, step: &str) -> StoreResult<bool> {
    Ok(tx.execute(
        "UPDATE task_graphs SET step=?2,run_start=NULL,lf_process_id=NULL,updated_at=?3 WHERE task_id=?1",
        rusqlite::params![task.as_str(), step, crate::store::rows::now_unix()],
    )? > 0)
}

/// Match unfinished Processes through checkout, Session history and attachments.
/// Start from the small unfinished inventory and retain the attachment partial index.
fn task_open_process_ids(conn: &rusqlite::Connection) -> StoreResult<String> {
    let session = session_membership("a");
    Ok(format!(
        "WITH task_scope AS MATERIALIZED (SELECT id,worktree,checkout_machine_id FROM tasks WHERE id=?1),
        open AS MATERIALIZED (SELECT id,cwd,agent_session_id,attached_lf_process_id FROM processes INDEXED BY processes_unfinished
            WHERE completed_at IS NULL),
        sessions AS (SELECT DISTINCT se.lf_process_id AS process,se.session_id AS session
            FROM open CROSS JOIN session_events se ON se.lf_process_id=open.id
            UNION SELECT p.attached_lf_process_id,p.agent_session_id FROM open CROSS JOIN processes p INDEXED BY agent_process_attachment ON p.attached_lf_process_id=open.id WHERE p.kind='agent'
            UNION SELECT id,agent_session_id FROM open WHERE agent_session_id IS NOT NULL)
        SELECT open.id FROM open JOIN task_scope tw ON {}
        UNION SELECT s.process FROM sessions s JOIN agent_sessions a ON a.id=s.session
            JOIN task_scope tw ON ({session})",
        process_membership(conn, "open")?
    ))
}

fn members(
    tx: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<(
    Vec<TaskSession>,
    Vec<crate::durable::GraphProcessInventoryEntry>,
)> {
    let sessions = tx
        .prepare(&format!(
            "SELECT s.id,s.title,s.interactive,{},s.completed_at
            FROM agent_sessions s WHERE s.id IN ({}) ORDER BY s.created_at,s.id",
            super::sessions::SESSION_GRAPH,
            session_ids("?1")
        ))?
        .query_and_then([task.as_str()], |row| {
            Ok(TaskSession {
                id: row.get(0)?,
                title: row.get(1)?,
                interactive: row.get(2)?,
                graph_lf_process_id: row.get(3)?,
                completed_at: row.get(4)?,
            })
        })?
        .collect::<StoreResult<Vec<_>>>()?;
    let graphs = graphs_of_task(tx, task)?
        .iter()
        .map(|graph| super::graph_inventory::entry_in(tx, graph))
        .collect::<StoreResult<Vec<_>>>()?;
    Ok((sessions, graphs))
}

impl SqliteStore {
    pub(crate) fn session_task_ids(&self, session: &str) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT tw.id FROM tasks tw JOIN agent_sessions a ON a.id=?1 WHERE {} ORDER BY tw.id",
            session_membership("a")
        ))?;
        let rows = query.query_map([session], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            TaskId::parse(&row?)
                .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
        })
        .collect()
    }

    /// Current workspace facts, without enumerating mechanical Process history.
    pub fn task_overview(&self, task: &TaskId) -> StoreResult<crate::task_work::TaskOverview> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let (sessions, graph_processes) = members(&tx, task)?;
        let graph = task_graph_row(&tx, task)?;
        tx.commit()?;
        drop(conn);
        Ok(crate::task_work::TaskOverview {
            sessions,
            graph_processes,
            graph: graph.map(|row| self.read_task_graph(row)),
        })
    }

    /// Read all three owners in one SQLite snapshot, including closed history.
    pub fn task_work(&self, task: &TaskId) -> StoreResult<TaskWork> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let (sessions, graphs) = members(&tx, task)?;
        let processes = tx
            .prepare(&format!(
                "{} WHERE e.id IN ({}) ORDER BY e.started_at DESC,e.id",
                super::processes::PROCESS_SELECT,
                lf_process_ids(&tx, "?1")?
            ))?
            .query_map([task.as_str()], super::processes::read_process)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let graph = task_graph_row(&tx, task)?;
        tx.commit()?;
        drop(conn);
        Ok(TaskWork {
            sessions,
            graph_processes: graphs,
            processes,
            graph: graph.map(|row| self.read_task_graph(row)),
        })
    }

    /// Single-Task execution gates need neither other Tasks nor retained Session/Graph payloads.
    pub(crate) fn task_open_processes(&self, task: &TaskId) -> StoreResult<Vec<LfProcess>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let mut query = conn.prepare(&format!(
            "{} WHERE e.id IN ({}) ORDER BY e.started_at DESC,e.id",
            super::processes::PROCESS_SELECT,
            task_open_process_ids(&conn)?,
        ))?;
        let processes = query
            .query_map([task.as_str()], super::processes::read_process)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(processes)
    }

    /// Whether a command with no recorded exit may still be running.
    /// Unknown is not stopped.
    pub(crate) fn process_may_run(&self, process: &crate::process::LfProcess) -> bool {
        crate::journal::process_evidence(self, &process.id)
            != crate::journal::ProcessIdentityEvidence::Dead
    }

    /// The Task's Graph and where it stands.
    pub(crate) fn task_graph(&self, task: &TaskId) -> StoreResult<Option<TaskGraph>> {
        let row = self.task_graph_row(task)?;
        Ok(row.map(|row| self.read_task_graph(row)))
    }

    /// The Task's Graph as captured, for running it.
    pub(crate) fn task_graph_row(&self, task: &TaskId) -> StoreResult<Option<TaskGraphRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_graph_row(&conn, task)
    }

    /// Whether the carrying run still runs is its Process's, read here.
    fn read_task_graph(&self, row: TaskGraphRow) -> TaskGraph {
        let position = match row.run {
            None => TaskGraphPosition::At { step: row.step },
            Some((_, process)) => TaskGraphPosition::Run {
                step: row.step,
                lf_process_id: process.id.to_string(),
                running: process.completed_at.is_none() && self.process_may_run(&process),
            },
        };
        TaskGraph {
            name: row.graph.name.clone(),
            interactive_steps: row.graph.interactive_summary(),
            position,
        }
    }

    /// The Task's Graph as it moves to another Machine.
    pub(crate) fn task_graph_record(&self, task: &TaskId) -> StoreResult<Option<TaskGraphRecord>> {
        use rusqlite::OptionalExtension;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some((graph, step, run_start)) = conn
            .query_row(
                "SELECT graph,step,run_start FROM task_graphs WHERE task_id=?1",
                [task.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?
        else {
            return Ok(None);
        };
        Ok(Some(TaskGraphRecord {
            graph: serde_json::from_str(&graph)?,
            step,
            run_start: run_start.as_deref().map(serde_json::from_str).transpose()?,
        }))
    }

    /// Take up a Graph that moved here. A Task on a run stays on it, carried
    /// by `by`, the receiving Process: it ends with the move, so the run reads
    /// as stopped and the next `lf task run` starts it again from where it began.
    pub(crate) fn place_moved_task_graph(
        &self,
        task: &TaskId,
        record: &TaskGraphRecord,
        by: Option<&LfProcessId>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let run = record.run_start.as_ref().zip(by);
        conn.execute(
            "INSERT OR REPLACE INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6)",
            rusqlite::params![
                task.as_str(),
                serde_json::to_string(&record.graph)?,
                record.step,
                run.map(|(start, _)| serde_json::to_string(start)).transpose()?,
                run.map(|(_, by)| by.as_str()),
                crate::store::rows::now_unix()
            ],
        )?;
        Ok(())
    }

    /// Take up `graph` for the Task as it is defined now, at `start`. It
    /// replaces the Task's earlier Graph.
    pub(crate) fn take_up_task_graph(
        &self,
        task: &TaskId,
        graph: &CapturedGraph,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT OR REPLACE INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                task.as_str(),
                serde_json::to_string(graph)?,
                START,
                crate::store::rows::now_unix()
            ],
        )?;
        Ok(())
    }

    /// Put the Task on a run from `start`, carried by `by`. Returns false,
    /// writing nothing, when the Task no longer stands where `row` read it, so
    /// two runs cannot both set out.
    pub(crate) fn start_task_graph_run(
        &self,
        task: &TaskId,
        row: &TaskGraphRow,
        start: &ExecutionCursor,
        by: &LfProcessId,
        ships: bool,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let stopped = row.run.as_ref().map(|(_, process)| &process.id);
        let started = start_run_in(&tx, task, (&row.step, stopped), start, by, ships)?;
        tx.commit()?;
        Ok(started)
    }

    /// The run `carrier` carries stopped at `step` (or reached `end`): put the
    /// Task there. Returns whether it reached `end`, requesting completion.
    pub(crate) fn arrive_task_graph(
        &self,
        task: &TaskId,
        carrier: &LfProcessId,
        step: &str,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let ended = arrive_in(&tx, task, carrier, step)?;
        tx.commit()?;
        Ok(ended)
    }

    /// Put the Task at `end` with nothing left to run, requesting completion.
    /// Returns whether it reached `end` now.
    pub(crate) fn end_task_graph(&self, task: &TaskId, by: &LfProcessId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let ended = tx.execute(
            "UPDATE task_graphs SET step=?2,updated_at=?3 WHERE task_id=?1 AND step!=?2 AND lf_process_id IS NULL",
            rusqlite::params![task.as_str(), END, crate::store::rows::now_unix()],
        )? > 0;
        if ended {
            ship_in(&tx, task, by, "its run reached the end")?;
            super::children::request_completion_in(&tx, task, None)?;
        }
        tx.commit()?;
        Ok(ended)
    }

    /// Put the Task at `step` without running anything. Returns false when the
    /// Task has no Graph.
    pub(crate) fn set_task_graph_step(&self, task: &TaskId, step: &str) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let set = set_step_in(&tx, task, step)?;
        tx.commit()?;
        Ok(set)
    }

    /// Hand the Task off to finish on its own, carried by `by`. Shipping that
    /// already began keeps its start; only the carrier changes.
    pub(crate) fn ship_task(
        &self,
        task: &TaskId,
        by: &LfProcessId,
        note: &str,
    ) -> StoreResult<TaskShipping> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let shipping = ship_in(&tx, task, by, note)?;
        tx.commit()?;
        Ok(shipping)
    }

    /// Withdraw a Task's Auto-Ship approval. Returns the approval and the process
    /// that carried it, when it was approved.
    pub(crate) fn stop_shipping(
        &self,
        task: &TaskId,
        note: &str,
    ) -> StoreResult<Option<TaskShipping>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let Some(shipping) = unship_in(&tx, task)? else {
            return Ok(None);
        };
        super::children::insert_task_event_in(
            &tx,
            task,
            &crate::work::task::TaskEventKind::Progress {
                summary: format!("Stopped shipping: {note}"),
            },
        )?;
        tx.commit()?;
        Ok(Some(shipping))
    }

    /// When the Task was handed off and the process carrying it, if it is shipping.
    pub(crate) fn task_shipping(&self, task: &TaskId) -> StoreResult<Option<TaskShipping>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        shipping_in(&conn, task)
    }

    /// Handed-off Tasks whose checkout is on this Machine, with their handoffs.
    pub(crate) fn local_shipping_tasks(&self) -> StoreResult<Vec<(TaskId, TaskShipping)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT id,shipped_at,ship_lf_process_id,ship_restarts FROM tasks
             WHERE shipped_at IS NOT NULL
             AND checkout_machine_id=(SELECT id FROM machines WHERE route='local')",
        )?;
        let rows = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, shipped_at, process, restarts)| {
                let id = TaskId::parse(&id)
                    .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
                Ok((id, shipping(shipped_at, process, restarts)?))
            })
            .collect()
    }

    /// Count one more restart after a failed ship process.
    pub(crate) fn count_ship_restart(&self, task: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tasks SET ship_restarts=ship_restarts+1 WHERE id=?1",
            [task.as_str()],
        )?;
        Ok(())
    }

    /// A ship step completed: the next failure restarts without waiting long.
    pub(crate) fn reset_ship_restarts(&self, task: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tasks SET ship_restarts=0 WHERE id=?1",
            [task.as_str()],
        )?;
        Ok(())
    }

    /// Each Task's most recently launched Graph, read for every Task at once.
    pub(crate) fn latest_task_graphs(
        &self,
    ) -> StoreResult<HashMap<TaskId, crate::ops::graph_process::GraphProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        latest_task_graphs(&conn)
    }

    /// Every Graph among the Task's work, oldest first.
    pub(crate) fn task_graphs(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<crate::ops::graph_process::GraphProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        graphs_of_task(&conn, task)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use rusqlite::params;

    use crate::durable::{GraphProcessFilter, ProjectId, TaskId};
    use crate::id::{LfProcessId, TraceId, WaveId};
    use crate::process::{LfProcessFilter, LfProcessWorkFilter};
    use crate::session::SessionFilter;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn defer_holds_the_lifecycle_and_a_run_with_nothing_interactive_ahead_ships() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let by = LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo/task',1)",
                params![by, crate::id::TraceId::new()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
                params![project.as_str(), wave],
            )
            .unwrap();
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE waves SET current_project_id=?1 WHERE id=?2",
                params![project.as_str(), wave],
            )
            .unwrap();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/repo").unwrap(),
                id: task.clone(),
                project_id: Some(project),
                title: "Retained Task".into(),
                description: "Brief".into(),
                due_date: None,
            })
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET worktree='/repo/task' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        let definition = crate::task_graph::CapturedGraph {
            name: "proof".into(),
            steps: vec![crate::graph::ConcreteStep::Skill(
                crate::graph::ConcreteSkill {
                    skill: crate::graph::Skill::named("pursue"),
                    id: None,
                    interactive: false,
                    returns: None,
                    sources: Vec::new(),
                },
            )],
        };
        store.take_up_task_graph(&task, &definition).unwrap();
        let before = store.task_graph(&task).unwrap().unwrap();
        let row = store.task_graph_row(&task).unwrap().unwrap();
        let start = crate::graph::runner::ExecutionCursor::default();
        store.set_task_deferred(&task, true).unwrap();
        store.set_task_deferred(&task, true).unwrap();
        assert_eq!(
            store.task_state(&task).unwrap(),
            crate::durable::TaskState::Deferred
        );
        assert!(store
            .start_task_graph_run(&task, &row, &start, &LfProcessId::new(), true)
            .is_err());
        assert_eq!(store.task_graph(&task).unwrap().unwrap(), before);
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(
            store.task_state(&task).unwrap(),
            crate::durable::TaskState::Deferred
        );
        store.set_task_deferred(&task, false).unwrap();
        assert_eq!(
            store.task_state(&task).unwrap(),
            crate::durable::TaskState::Idle
        );
        assert_eq!(store.task_graph(&task).unwrap().unwrap(), before);
        assert_eq!(
            store.task(&task).unwrap().unwrap().worktree,
            Some(std::path::PathBuf::from("/repo/task"))
        );

        // No interactive step remains ahead: running it hands the Task off.
        assert!(store
            .start_task_graph_run(&task, &row, &start, &by, true)
            .unwrap());
        // A second run reading the same position cannot also set out.
        assert!(!store
            .start_task_graph_run(&task, &row, &start, &LfProcessId::new(), true)
            .unwrap());
        let state = || store.task_state(&task).unwrap();
        assert_eq!(state(), crate::durable::TaskState::Shipping);
        let shipping = store.task_shipping(&task).unwrap().unwrap();
        assert_eq!(shipping.process, Some(by.clone()));
        // Only the checkout's Machine restarts its ship process.
        assert!(store.local_shipping_tasks().unwrap().is_empty());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET checkout_machine_id=(SELECT id FROM machines WHERE route='local') WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert_eq!(
            store.local_shipping_tasks().unwrap(),
            [(task.clone(), shipping.clone())]
        );
        // Defer holds shipping without ending it.
        store.set_task_deferred(&task, true).unwrap();
        assert_eq!(state(), crate::durable::TaskState::Deferred);
        store.set_task_deferred(&task, false).unwrap();
        assert_eq!(state(), crate::durable::TaskState::Shipping);
        // Shipping again keeps when it began.
        let again = store.ship_task(&task, &by, "again").unwrap();
        assert_eq!(again.shipped_at, shipping.shipped_at);
        // Stop withdraws the approval; the run still going keeps it moving.
        assert_eq!(store.stop_shipping(&task, "stop").unwrap(), Some(shipping));
        assert_eq!(state(), crate::durable::TaskState::Moving);
        assert_eq!(store.stop_shipping(&task, "stop").unwrap(), None);
        // Approved again with nothing running, it is idle until its ship process restarts.
        store.ship_task(&task, &by, "again").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE processes SET completed_at=2 WHERE id=?1", [&by])
            .unwrap();
        assert_eq!(state(), crate::durable::TaskState::Idle);
    }

    #[test]
    fn end_request_is_atomic_and_survives_reopening_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        {
            let mut conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)",params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,workspace_slug,branch,base_commit,created_at,updated_at,issue_title,issue_description,pm_snapshot_synced_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/repo','proof','proof','head',1,1,'Findings','Accepted findings',1)",params![task.as_str(),project.as_str()]).unwrap();
            let carrier = LfProcessId::new();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
                params![carrier, TraceId::new()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at) VALUES(?1,'{\"name\":\"findings\",\"steps\":[]}','start','{\"index\":0}',?2,1)",
                params![task.as_str(), carrier],
            )
            .unwrap();
            conn.execute_batch("CREATE TRIGGER refuse_request BEFORE INSERT ON task_events WHEN json_extract(NEW.kind_json,'$.kind')='completion_requested' BEGIN SELECT RAISE(ABORT,'unavailable'); END").unwrap();
            let at_end = |conn: &rusqlite::Connection| -> bool {
                conn.query_row(
                    "SELECT step='end' FROM task_graphs WHERE task_id=?1",
                    [task.as_str()],
                    |row| row.get(0),
                )
                .unwrap()
            };
            {
                let tx = conn.transaction().unwrap();
                assert!(super::arrive_in(&tx, &task, &carrier, super::END).is_err());
            }
            assert!(!at_end(&conn));
            assert!(super::shipping_in(&conn, &task).unwrap().is_none());
            conn.execute_batch("DROP TRIGGER refuse_request").unwrap();
            let tx = conn.transaction().unwrap();
            assert!(super::arrive_in(&tx, &task, &carrier, super::END).unwrap());
            tx.commit().unwrap();
        }
        assert!(store.task_shipping(&task).unwrap().is_some());
        let (request, _) = store.task_completion_pending(&task).unwrap().unwrap();
        store
            .fail_task_completion(&task, request, "provider unavailable")
            .unwrap();
        let graph = store.task_graph(&task).unwrap();
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(
            store.task_completion_pending(&task).unwrap(),
            Some((request, "provider unavailable".into()))
        );
        let retained = store.task(&task).unwrap().unwrap();
        assert!(store.complete_task(&retained, request).unwrap());
        assert!(store.complete_task(&retained, request).unwrap());
        assert!(store.task_completion_pending(&task).unwrap().is_none());
        assert_eq!(store.task_graph(&task).unwrap(), graph);
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            super::super::durable::task_state_in(&conn, &task).unwrap(),
            crate::durable::TaskState::Done
        );
        let completed: i64 = conn.query_row("SELECT count(*) FROM task_events WHERE task_id=?1 AND json_extract(kind_json,'$.kind')='completed'",[task.as_str()],|row| row.get(0)).unwrap();
        assert_eq!(completed, 1);
        drop(conn);
        store.reopen_task(&task, Some("Continue findings")).unwrap();
        assert!(!store.complete_task(&retained, request).unwrap());
        assert!(store.task_completion_pending(&task).unwrap().is_none());
        assert_eq!(store.task_graph(&task).unwrap(), graph);
        assert_ne!(
            store.task_state(&task).unwrap(),
            crate::durable::TaskState::Done
        );
    }

    #[test]
    fn reaching_end_without_remaining_steps_approves_shipping() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let task = TaskId::new();
        let by = LfProcessId::new();
        store
            .create_task(&crate::work::task::NewTask {
                repository_id: store.ensure_repository("/repo").unwrap(),
                id: task.clone(),
                project_id: None,
                title: "Finish review".into(),
                description: "No steps remain".into(),
                due_date: None,
            })
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
                params![by, TraceId::new()],
            )
            .unwrap();
        store
            .take_up_task_graph(
                &task,
                &crate::task_graph::CapturedGraph {
                    name: "review".into(),
                    steps: Vec::new(),
                },
            )
            .unwrap();
        store.set_task_graph_step(&task, "review").unwrap();
        assert!(store.task_shipping(&task).unwrap().is_none());
        assert!(store.end_task_graph(&task, &by).unwrap());
        assert!(store.task_completion_pending(&task).unwrap().is_some());
        assert_eq!(
            store.task_shipping(&task).unwrap().unwrap().process,
            Some(by.clone())
        );
        assert!(!store.end_task_graph(&task, &by).unwrap());
    }

    #[test]
    fn every_task_latest_graph_matches_its_own_graphs() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let (task, sibling, idle) = (TaskId::new(), TaskId::new(), TaskId::new());
        let mut graphs = Vec::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(), wave]).unwrap();
            conn.execute(
                "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_proof','/repo',1)",
                [],
            )
            .unwrap();
            // A sibling whose path extends the Task's is not inside it.
            for (id, n, worktree) in [
                (&task, 1, "/repo/task/"),
                (&sibling, 2, "/repo/task-other"),
                (&idle, 3, "/repo/idle"),
            ] {
                conn.execute("INSERT INTO tasks(repository_id,id,project_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES('repo_proof',?1,?2,?3,?4,1,(SELECT id FROM machines WHERE route='local'))", params![id.as_str(), project.as_str(), format!("PROOF-{n}"), worktree]).unwrap();
            }
            for cwd in [
                "/repo/task",
                "/repo/task-other",
                "/repo/task/deep/er",
                "/repo/task-other/x",
                "/elsewhere",
            ] {
                let id = LfProcessId::new();
                conn.execute(
                    "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,?3,1)",
                    params![id, TraceId::new(), cwd],
                )
                .unwrap();
                graphs.push(id);
            }
        }
        let graph = crate::graph::compiled::CompiledGraph::new("proof", &[]);
        for id in &graphs {
            store.record_graph_process(id, &graph).unwrap();
        }
        let latest = store.latest_task_graphs().unwrap();
        for id in [&task, &sibling, &idle] {
            assert_eq!(
                latest.get(id),
                store.task_graphs(id).unwrap().last(),
                "{id}"
            );
        }
        assert_eq!(
            latest[&task].process.cwd.as_deref(),
            Some("/repo/task/deep/er")
        );
        assert_eq!(
            latest[&sibling].process.cwd.as_deref(),
            Some("/repo/task-other/x")
        );
        assert!(!latest.contains_key(&idle));
    }

    #[test]
    fn process_membership_does_not_read_events_that_name_no_process() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let turn = LfProcessId::new();
        let finished = LfProcessId::new();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
            [&wave],
        )
        .unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(), wave]).unwrap();
        conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/repo/task',1,(SELECT id FROM machines WHERE route='local'))", params![task.as_str(), project.as_str()]).unwrap();
        // The Session is bound, and its Processes ran elsewhere: only event
        // history associates them with the Task.
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,task_id,wave_id,cwd)
            VALUES('bound','bound','human',1,0,?1,?2,'/elsewhere')", params![task.as_str(), wave]).unwrap();
        for (id, completed) in [(&turn, None), (&finished, Some(2))] {
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at,completed_at,outcome) VALUES(?1,?2,'/elsewhere',1,?3,?4)",
                params![id, TraceId::new(), completed, completed.map(|_: i64| "succeeded")],
            )
            .unwrap();
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload)
                VALUES('bound','started',?1,?1,1,'{}')", [id]).unwrap();
        }
        let read = |unfinished: bool, expected: &[&LfProcessId]| {
            let mut query = conn
                .prepare(&format!(
                    "{} WHERE {} e.id IN ({}) ORDER BY e.started_at DESC,e.id",
                    super::super::processes::PROCESS_SELECT,
                    if unfinished {
                        "e.completed_at IS NULL AND"
                    } else {
                        ""
                    },
                    super::lf_process_ids(&conn, "?1").unwrap()
                ))
                .unwrap();
            let mut ids = query
                .query_map([task.as_str()], |row| row.get::<_, LfProcessId>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            let mut expected: Vec<_> = expected.iter().map(|id| (*id).clone()).collect();
            expected.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            assert_eq!(ids, expected);
            query.get_status(rusqlite::StatementStatus::VmStep)
        };
        let before = (read(true, &[&turn]), read(false, &[&turn, &finished]));
        for n in 0..2_000 {
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
                VALUES('bound','observed',?1,1,'{}')",
                [n.to_string()],
            )
            .unwrap();
        }
        let after = (read(true, &[&turn]), read(false, &[&turn, &finished]));
        assert!(
            after.0 <= before.0 * 2 && after.1 <= before.1 * 2,
            "Events naming no Process increased membership work: {before:?} → {after:?}"
        );
        // Cleanup must ignore completed Process history as it grows, including
        // attachment lookup for unrelated completed providers.
        let cleanup_cost = || {
            let mut query = conn
                .prepare(&super::task_open_process_ids(&conn).unwrap())
                .unwrap();
            let ids = query
                .query_map([task.as_str()], |row| row.get::<_, LfProcessId>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(ids, vec![turn.clone()]);
            (
                query.get_status(rusqlite::StatementStatus::VmStep),
                query.get_status(rusqlite::StatementStatus::FullscanStep),
            )
        };
        let before_cleanup = cleanup_cost();
        for n in 0..2_000 {
            let id = LfProcessId::new();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at,completed_at)
                VALUES(?1,?1,'/unrelated',1,2)",
                [&id],
            )
            .unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute(
                "INSERT INTO tasks(repository_id,id,project_id,issue_identifier,created_at,branch)
                VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,?3,1,?3)",
                params![TaskId::new().as_str(), project.as_str(), format!("OLD-{n}")],
            )
            .unwrap();
        }
        let after_cleanup = cleanup_cost();
        eprintln!("cleanup VM/full-scan steps: {before_cleanup:?} → {after_cleanup:?}");
        assert!(
            after_cleanup.0 <= before_cleanup.0 * 2 && after_cleanup.1 <= before_cleanup.1 * 2,
            "Unrelated history increased cleanup scans: {before_cleanup:?} → {after_cleanup:?}"
        );
        // Graph reads start from the Graphs without losing historical association
        // when the Graph ran outside the Task's checkout.
        conn.execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'proof','{}')",
            [&finished],
        )
        .unwrap();
        let pairs = super::task_graph_pairs(&conn, &super::tasks("?1")).unwrap();
        let read_graphs = || {
            let mut statement = conn.prepare(&pairs).unwrap();
            let selected = statement
                .query_map([task.as_str()], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(selected, vec![finished.to_string()]);
            statement.get_status(rusqlite::StatementStatus::VmStep)
        };
        let before = read_graphs();
        for _ in 0..2_000 {
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/unrelated',1)",
                params![LfProcessId::new(), TraceId::new()],
            )
            .unwrap();
        }
        let after = read_graphs();
        assert!(
            after <= before * 2,
            "Unrelated commands increased Graph membership work: {before} → {after}"
        );
        // Every Task's Graphs are one reading driven from the Graphs, so Sessions
        // that reach no Graph cost nothing however many Tasks are read. On
        // October 18, 2026 each Task rescanned all 4,246 of Jack Heart's
        // Sessions: 2.3 s for 622 Tasks.
        let every_task =
            super::task_graph_pairs(&conn, "SELECT id,worktree,checkout_machine_id FROM tasks")
                .unwrap();
        let read_every_task = || {
            let mut statement = conn.prepare(&every_task).unwrap();
            let pairs = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(pairs, vec![(task.to_string(), finished.to_string())]);
            statement.get_status(rusqlite::StatementStatus::VmStep)
        };
        let before = read_every_task();
        for n in 0..2_000 {
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,task_id,wave_id,cwd)
                VALUES(?1,'quiet','human',1,0,?2,?3,'/repo/task')", params![format!("quiet-{n}"), task.as_str(), wave]).unwrap();
        }
        let after = read_every_task();
        assert!(
            after <= before * 2,
            "Sessions reaching no Graph increased every Task's Graph reading: {before} → {after}"
        );
    }

    #[tokio::test]
    async fn task_checkout_machine_keeps_session_filters_beside_other_machines() {
        let repo = loopflow_test_support::TestRepo::new();
        let other = loopflow_test_support::TestRepo::new();
        let dir = tempfile::tempdir().unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(repo.path(), &alias).unwrap();
        std::fs::create_dir(repo.path().join("sub")).unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                dir.path().join("db"),
            ))
            .await
            .unwrap(),
        );
        let home = store.local_machine().await.unwrap().id;
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        {
            let conn = store.sqlite.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','Retained Task',?3,1,(SELECT id FROM machines WHERE route='local'))",params![task.as_str(),project.as_str(),repo.path().to_str().unwrap()]).unwrap();
            for (id, path, scope) in [
                ("a-alias", alias.join("sub"), None),
                ("b-subdir", repo.path().join("sub"), None),
                ("c-repo", repo.path().to_path_buf(), Some("repository")),
                ("d-orphan", other.path().to_path_buf(), None),
            ] {
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,primary_scope,interactive) VALUES(?1,?1,'human',1,0,?2,?3,1)",params![id,path.to_str().unwrap(),scope]).unwrap();
                super::super::sessions::test_capture(
                    &conn,
                    id,
                    &crate::session_record::new_artifact_key(),
                );
            }
        }
        let remote = crate::durable::MachineId::new();
        store
            .sqlite
            .add_machine(&remote, "worker", "worker")
            .unwrap();
        let mut filter = SessionFilter {
            task: Some("PROOF-1".into()),
            limit: 1,
            after: Some(String::new()),
            ..Default::default()
        };
        for expected in ["a-alias", "b-subdir"] {
            let page = crate::ops::human_session::list(&store, &filter)
                .await
                .unwrap();
            assert_eq!(page.len(), 1);
            assert_eq!(page[0].id, expected);
            assert_eq!(page[0].task_ids, vec![task.clone()]);
            assert_eq!(page[0].workspace.as_ref().unwrap().machine_id, home);
            filter.after = Some(page[0].id.to_string());
        }
        filter.task = None;
        filter.orphan = true;
        filter.after = Some(String::new());
        for expected in ["c-repo", "d-orphan"] {
            let page = crate::ops::human_session::list(&store, &filter)
                .await
                .unwrap();
            assert_eq!(page.len(), 1);
            assert_eq!(page[0].id, expected);
            assert!(page[0].task_ids.is_empty());
            filter.after = Some(page[0].id.to_string());
        }
    }

    #[test]
    fn scoped_sessions_stay_out_of_tasks_and_orphan_pages_include_only_unassociated_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/repo/task',1,(SELECT id FROM machines WHERE route='local'))",params![task.as_str(),project.as_str()]).unwrap();
            for (id, scope, wave_id, cwd) in [
                ("a-task", None, None, "/repo/task/sub"),
                ("b-repo", Some("repository"), None, "/repo/task"),
                ("c-wave", Some("wave"), Some(wave.as_str()), "/repo/task"),
                ("d-orphan", None, None, "/repo/other"),
                (
                    "e-wave-attribution",
                    None,
                    Some(wave.as_str()),
                    "/repo/task",
                ),
            ] {
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,primary_scope,wave_id,interactive) VALUES(?1,?1,'human',1,0,?2,?3,?4,1)",params![id,cwd,scope,wave_id]).unwrap();
                super::super::sessions::test_capture(
                    &conn,
                    id,
                    &crate::session_record::new_artifact_key(),
                );
            }
        }
        assert_eq!(
            store.session_task_ids("a-task").unwrap(),
            vec![task.clone()]
        );
        assert_eq!(
            store.session_task_ids("e-wave-attribution").unwrap(),
            vec![task]
        );
        for id in ["b-repo", "c-wave", "d-orphan"] {
            assert!(store.session_task_ids(id).unwrap().is_empty());
        }
        let mut filter = SessionFilter {
            orphan: true,
            limit: 1,
            after: Some(String::new()),
            ..Default::default()
        };
        for expected in ["b-repo", "c-wave", "d-orphan"] {
            let page = store.session_summaries(&filter, 0).unwrap();
            assert_eq!(page.len(), 1);
            assert_eq!(page[0].id, expected);
            filter.after = Some(page[0].id.to_string());
        }
        assert!(store.session_summaries(&filter, 0).unwrap().is_empty());
    }

    #[test]
    fn task_work_includes_checkout_binding_and_mechanical_history_without_granting_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let mechanical = LfProcessId::new();
        let bound_process = LfProcessId::new();
        let sibling = LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(), wave]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/missing/task%_',1,(SELECT id FROM machines WHERE route='local'))", params![task.as_str(), project.as_str()]).unwrap();
            for (id, cwd, bound, complete) in [
                ("manual", "/missing/task%_/src", false, false),
                ("conversation", "/missing/task%_", false, false),
                ("history", "/elsewhere", true, true),
                ("sibling", "/missing/task%_-other", false, false),
                ("wildcard", "/missing/taskAB", false, false),
            ] {
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id,completed_at)
                    VALUES(?1,?1,'human',1,0,?2,?3,?4,?5)", params![id,cwd,bound.then(|| task.as_str()),bound.then_some(wave.as_str()),complete.then_some(2)]).unwrap();
            }
            for id in ["manual", "conversation", "history", "sibling", "wildcard"] {
                super::super::sessions::test_capture(
                    &conn,
                    id,
                    &crate::session_record::new_artifact_key(),
                );
            }
            for (id, cwd) in [
                (&mechanical, "/missing/task%_"),
                (&bound_process, "/elsewhere"),
                (&sibling, "/missing/task%_-other"),
            ] {
                conn.execute("INSERT INTO processes(id,trace_id,cwd,started_at,completed_at,outcome) VALUES(?1,?2,?3,1,2,'succeeded')", params![id, TraceId::new(), cwd]).unwrap();
            }
            // Binding includes earlier Processes in the inventory without assigning
            // their earlier performed work or usage to this Task.
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload) VALUES('history','started','before-bind',?1,1,'{}')", [&bound_process]).unwrap();
            conn.execute(
                "UPDATE processes SET parent_lf_process_id=?1 WHERE id=?2",
                params![mechanical, sibling],
            )
            .unwrap();
        }
        // A Graph is the Task's work when its Processes ran in the checkout.
        let independent = store.test_graph(
            "independent",
            "/missing/task%_/sub",
            &[("implement", None)],
            None,
        );
        store.test_graph("unrelated", "/other", &[("implement", None)], None);
        let work = store.task_work(&task).unwrap();
        assert_eq!(
            work.sessions
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["conversation", "history", "manual"]
        );
        assert_eq!(
            work.graph_processes
                .iter()
                .map(|f| f.summary.id.as_str())
                .collect::<Vec<_>>(),
            [independent.as_str()]
        );
        assert_eq!(
            work.graph_processes[0].summary.task_id.as_ref(),
            Some(&task)
        );
        assert_eq!(work.processes.len(), 4);
        // The open reading agrees with the full one about unfinished Processes.
        let initial_open = store.task_open_processes(&task).unwrap();
        assert_eq!(initial_open.len(), 2);
        assert!(initial_open
            .iter()
            .all(|process| process.completed_at.is_none()));
        let unfinished = LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/elsewhere',1)",
                params![unfinished, TraceId::new()],
            )
            .unwrap();
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload) VALUES('history','started','open',?1,1,'{}')", [&unfinished]).unwrap();
        }
        let open = store.task_open_processes(&task).unwrap();
        assert_eq!(open.len(), 3);
        assert!(open.iter().any(|process| process.id == unfinished));
        // With every Process unfinished, each membership path agrees: checkout,
        // Session event and attachment.
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TABLE finished AS SELECT id,completed_at FROM processes;
             UPDATE processes SET completed_at=NULL;",
            )
            .unwrap();
        let ids = |processes: Vec<crate::process::LfProcess>| {
            processes
                .into_iter()
                .map(|process| process.id)
                .collect::<Vec<_>>()
        };
        let all = ids(store.task_work(&task).unwrap().processes);
        assert_eq!(all.len(), 5);
        assert_eq!(ids(store.task_open_processes(&task).unwrap()), all);
        store.conn.lock().unwrap().execute_batch(
            "UPDATE processes SET completed_at=(SELECT completed_at FROM finished WHERE finished.id=processes.id);
             DROP TABLE finished;",
        )
        .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM session_events WHERE receipt_key='open'", [])
                .unwrap();
            conn.execute("DELETE FROM processes WHERE id=?1", [&unfinished])
                .unwrap();
        }
        assert!(work
            .processes
            .iter()
            .any(|process| process.id == mechanical));
        assert!(work
            .processes
            .iter()
            .any(|process| process.id == bound_process));
        assert_eq!(
            store.session_task_ids("manual").unwrap(),
            std::slice::from_ref(&task)
        );
        assert!(store.session_task_ids("sibling").unwrap().is_empty());
        let summaries = store
            .session_summaries(
                &SessionFilter {
                    task: Some("PROOF-1".into()),
                    interactive: None,
                    history: true,
                    ..Default::default()
                },
                0,
            )
            .unwrap();
        assert_eq!(summaries.len(), work.sessions.len());
        assert!(summaries
            .iter()
            .all(|session| session.task_ids == std::slice::from_ref(&task)));
        let graphs = store
            .graph_inventory(
                &GraphProcessFilter {
                    task_id: Some(task.clone()),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(100).unwrap(),
            )
            .unwrap();
        assert_eq!(graphs.entries, work.graph_processes);
        // The Graph's step is the only work performed in the checkout.
        let performed = store
            .processes(
                &LfProcessFilter {
                    performed_work: Some(LfProcessWorkFilter::Task(task.clone())),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(100).unwrap(),
            )
            .unwrap();
        let (graph, _) = store.graph_process(independent.as_str()).unwrap().unwrap();
        assert_eq!(
            performed
                .entries
                .iter()
                .map(|process| &process.id)
                .collect::<Vec<_>>(),
            [&graph.steps[0].process.id]
        );

        let child = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'child','PROOF-2','/missing/task%_/child',1,(SELECT id FROM machines WHERE route='local'))", params![child.as_str(), project.as_str()]).unwrap();
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id) VALUES('child-session','Child','human',1,0,'/missing/task%_/child',?1,?2)", params![child.as_str(), wave]).unwrap();
        }
        // A nested checkout's work is also its enclosing Task's.
        let child_graph = store.test_graph(
            "child",
            "/missing/task%_/child",
            &[("implement", None)],
            None,
        );
        let work = store.task_work(&task).unwrap();
        assert!(work
            .graph_processes
            .iter()
            .any(|graph| graph.summary.id == child_graph.as_str()));
        assert!(work
            .sessions
            .iter()
            .any(|session| session.id == "child-session"));
        let memberships = store.session_task_ids("child-session").unwrap();
        assert!(memberships.contains(&task) && memberships.contains(&child));
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET worktree='' WHERE id=?1", [task.as_str()])
                .unwrap();
        }
        let work = store.task_work(&task).unwrap();
        assert_eq!(
            work.sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            ["history"]
        );
        assert!(work.graph_processes.is_empty());

        // A Graph explicitly bound through a Session remains visible after the
        // checkout disappears. Inventory and Task status use the same rule.
        let bound_graph = store.test_graph("bound", "/elsewhere", &[], None);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload) VALUES('history','started','bound-graph',?1,3,'{}')", [&bound_graph]).unwrap();
        }
        let work = store.task_work(&task).unwrap();
        assert_eq!(work.graph_processes.len(), 1);
        assert_eq!(work.graph_processes[0].summary.id, bound_graph.as_str());
        assert_eq!(
            work.graph_processes[0].summary.task_id.as_ref(),
            Some(&task)
        );
        let (_, entry) = store.graph_process(bound_graph.as_str()).unwrap().unwrap();
        assert_eq!(entry.summary.task_id.as_ref(), Some(&task));
        assert_eq!(entry.summary.wave_id.as_ref(), Some(&wave));
    }
}
