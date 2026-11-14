//! One association rule for CLI and Desktop. Membership is additive to binding;
//! it never rewrites event attribution or follows causal parent Processes.

use std::collections::{HashMap, HashSet};

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
    format!("tw.checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) AND tw.worktree!='' AND ({cwd}=rtrim(tw.worktree,'/') OR instr({cwd},rtrim(tw.worktree,'/')||'/')=1)")
}

fn has_relocations(conn: &rusqlite::Connection) -> StoreResult<bool> {
    // Read-only clients also inspect released databases before promotion.
    // Those schemas cannot have relocation receipts; leave them untouched.
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='relocated_task_processes' AND type='table')",
        [], |row| row.get(0),
    )?)
}

fn process_membership(conn: &rusqlite::Connection, process: &str) -> StoreResult<String> {
    Ok(if has_relocations(conn)? {
        relocated_process_membership(process)
    } else {
        checkout(&format!("{process}.cwd"))
    })
}

fn relocated_process_membership(process: &str) -> String {
    let current = checkout(&format!("{process}.cwd"));
    format!("(({current}) AND NOT EXISTS(SELECT 1 FROM relocated_task_processes r WHERE r.process_id={process}.id)) OR EXISTS(SELECT 1 FROM relocated_task_processes r WHERE r.task_id=tw.id AND r.process_id={process}.id)")
}

pub(super) fn session_membership(session: &str) -> String {
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

/// The Tasks a Session belongs to. A checkout holding the Session's cwd has
/// that cwd or one of its ancestors as its worktree, so candidates come from
/// `tasks_checkout_root` rather than every Task. Explicit binding also counts.
pub(super) fn session_tasks(session: &str) -> String {
    // Probe checkout roots from the cwd outward. Keep paths lexical: repeated
    // slashes and relative paths have the same meaning as in `checkout`. CROSS
    // JOIN keeps ancestors outside the index probe instead of scanning Tasks.
    format!(
        "WITH RECURSIVE up(path) AS (
            SELECT rtrim({session}.cwd,'/') WHERE {session}.primary_scope IS NULL
            UNION ALL
            SELECT rtrim(rtrim(path,replace(path,'/','')),'/') FROM up WHERE path!='')
         SELECT id FROM tasks WHERE id={session}.task_id AND {session}.primary_scope IS NULL
         UNION
         SELECT tw.id FROM up CROSS JOIN tasks tw
            ON tw.checkout_machine_id=(SELECT id FROM machines WHERE is_local=1)
            AND rtrim(tw.worktree,'/')=up.path AND tw.worktree!=''
         WHERE up.path!='' OR substr({session}.cwd,1,1)='/' OR {session}.cwd=''
         ORDER BY id"
    )
}

pub(super) fn lf_process_ids(conn: &rusqlite::Connection, selector: &str) -> StoreResult<String> {
    let (unmoved, relocated) = if has_relocations(conn)? {
        (
            "AND NOT EXISTS(SELECT 1 FROM relocated_task_processes r WHERE r.process_id=ae.id)",
            "UNION SELECT r.process_id FROM tw JOIN relocated_task_processes r ON r.task_id=tw.id",
        )
    } else {
        ("", "")
    };
    // History and attachments share one query-local membership; the partial
    // index skips retained events that name no Process. In BINARY path order,
    // [root+'/', root+'0') is exactly the descendants of root. CROSS JOIN keeps
    // the selected checkouts outside the cwd probes; '%' and '_' stay literal.
    Ok(format!("WITH members AS MATERIALIZED ({}), tw AS MATERIALIZED ({})
        SELECT ae.id FROM tw CROSS JOIN processes ae
        WHERE tw.checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) AND tw.worktree!=''
            AND (ae.cwd=rtrim(tw.worktree,'/') OR
                (ae.cwd>=rtrim(tw.worktree,'/')||'/' AND ae.cwd<rtrim(tw.worktree,'/')||'0'))
            {unmoved}
        {relocated}
        UNION SELECT se.lf_process_id FROM session_events se INDEXED BY session_process_membership WHERE se.session_id IN (SELECT id FROM members) AND se.lf_process_id IS NOT NULL
        UNION SELECT p.attached_lf_process_id FROM processes p WHERE p.kind='agent' AND p.agent_session_id IN (SELECT id FROM members) AND p.attached_lf_process_id IS NOT NULL
        UNION SELECT p.id FROM processes p WHERE p.kind='agent' AND p.agent_session_id IN (SELECT id FROM members)",
        session_ids(selector), tasks(selector)))
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
                    SELECT session_id AS id FROM session_events INDEXED BY session_events_process
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

/// Each Task among `task_ids` paired with each Graph started for it. The Graph
/// recorded its Task at launch, so the pairs come from an index probe per Task.
fn task_graph_pairs(task_ids: &str) -> String {
    format!(
        "SELECT task_id, lf_process_id AS graph FROM graph_processes WHERE task_id IN ({task_ids})"
    )
}

pub(super) fn graphs_of_task(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Vec<crate::ops::graph_process::GraphProcess>> {
    super::graph_inventory::graphs_in(
        conn,
        &format!("e.id IN (SELECT graph FROM ({}))", task_graph_pairs("?1")),
        &[&task.as_str()],
    )
}

/// Each of `tasks`' completed Graph steps, by step process, for all of them in
/// one reading.
fn completed_graph_steps(
    conn: &rusqlite::Connection,
    tasks: &[&TaskId],
) -> StoreResult<HashMap<TaskId, HashSet<LfProcessId>>> {
    let mut query = conn.prepare(&format!(
        "SELECT pairs.task_id, gs.lf_process_id FROM ({}) pairs
         JOIN graph_process_steps gs ON gs.graph_lf_process_id=pairs.graph
         JOIN processes s ON s.id=gs.lf_process_id
         WHERE s.completed_at IS NOT NULL",
        task_graph_pairs("SELECT value FROM json_each(?1)")
    ))?;
    let ids = super::children::ids_json(tasks.iter().map(|id| id.as_str()));
    let mut completed = HashMap::<TaskId, HashSet<LfProcessId>>::new();
    for row in query.query_map([&ids], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (task, step) = row?;
        let invalid = |error: &dyn std::fmt::Display| {
            crate::store::StoreError::InvalidData(error.to_string())
        };
        let task = TaskId::parse(&task).map_err(|error| invalid(&error))?;
        let step = LfProcessId::parse(&step).map_err(|error| invalid(&error))?;
        completed.entry(task).or_default().insert(step);
    }
    Ok(completed)
}

/// Each of `tasks`' most recently launched Graph, for all of them in one reading.
fn latest_task_graphs(
    conn: &rusqlite::Connection,
    tasks: &[&TaskId],
) -> StoreResult<HashMap<TaskId, crate::ops::graph_process::GraphProcess>> {
    let mut latest = HashMap::new();
    let mut query = conn.prepare(&format!(
        "SELECT task_id, graph FROM ({}) pairs
         ORDER BY (SELECT e.rowid FROM processes e WHERE e.id=pairs.graph)",
        task_graph_pairs("SELECT value FROM json_each(?1)")
    ))?;
    let ids = super::children::ids_json(tasks.iter().map(|id| id.as_str()));
    for row in query.query_map([&ids], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (task, graph) = row?;
        latest.insert(task, graph);
    }
    let ids = super::children::ids_json(latest.values().map(String::as_str));
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
/// run left, and that run with the cursor its next attempt sets out from: where
/// it began, then the step after the last one it finished.
pub(crate) struct TaskGraphRow {
    pub graph: CapturedGraph,
    pub step: String,
    pub run: Option<(ExecutionCursor, LfProcess)>,
}

/// A Task's Graph as another Machine takes it up: the graph it took up, the
/// step it stands at, and the cursor its run sets out from again, if it is on
/// one. The run's Process stays behind.
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
                super::processes::read_lf_process,
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
/// granting merge authority unless a person stopped shipping it.
#[allow(clippy::too_many_arguments)]
fn start_run_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    (step, stopped): (&str, Option<&LfProcessId>),
    start: &ExecutionCursor,
    by: &LfProcessId,
    ships: bool,
) -> StoreResult<bool> {
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
            Approval::Route,
        )?;
    }
    Ok(true)
}

/// A Task handed off to finish on its own: since when, the process that
/// approved it, this Machine's process carrying it there, and how many times
/// in a row that process was started again after it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskShipping {
    pub shipped_at: i64,
    pub approved_by: Option<LfProcessId>,
    pub process: Option<LfProcessId>,
    pub restarts: u32,
}

/// How a Task comes to ship. A person's stop blocks only a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Approval {
    /// `lf ship`: approves it and lifts any stop.
    Explicit,
    /// A run reaching `end`: approves it unless a person stopped shipping.
    Route,
}

/// A person's `lf ship --stop`: when, and the process that ran it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShipStop {
    pub at: i64,
    pub by: Option<LfProcessId>,
    pub person: Option<String>,
}

pub(super) fn ship_stop_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Option<ShipStop>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT ship_stopped_at,ship_stopped_lf_process_id,ship_stopped_person FROM tasks
             WHERE id=?1 AND ship_stopped_at IS NOT NULL",
            [task.as_str()],
            |row| {
                Ok(ShipStop {
                    at: row.get(0)?,
                    by: row.get(1)?,
                    person: row.get(2)?,
                })
            },
        )
        .optional()?)
}

pub(super) fn shipping_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskShipping>> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            &format!("{SHIPPING_SELECT} WHERE id=?1 AND shipped_at IS NOT NULL"),
            [task.as_str()],
            read_shipping,
        )
        .optional()?
        .map(|(_, shipping)| shipping))
}

const SHIPPING_SELECT: &str =
    "SELECT id,shipped_at,ship_approved_lf_process_id,ship_lf_process_id,ship_restarts FROM tasks";

fn read_shipping(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, TaskShipping)> {
    Ok((
        row.get(0)?,
        TaskShipping {
            shipped_at: row.get(1)?,
            approved_by: row.get(2)?,
            process: row.get(3)?,
            restarts: row.get(4)?,
        },
    ))
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
            "UPDATE tasks SET ship_lf_process_id=NULL,ship_approved_lf_process_id=NULL,ship_restarts=0
             WHERE id=?1",
            [task.as_str()],
        )?;
    }
    Ok(shipping)
}

/// Approve Auto-Ship, carried by `by`. A route writes nothing and returns
/// `None` while a person's stop stands; an explicit approval lifts the stop.
fn ship_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    by: &LfProcessId,
    note: &str,
    approval: Approval,
) -> StoreResult<Option<TaskShipping>> {
    let state = super::durable::task_state_in(tx, task)?;
    if state.is_terminal() {
        return Err(crate::store::StoreError::InvalidAuthority(format!(
            "Task is {state}; it cannot ship"
        )));
    }
    match approval {
        Approval::Route if ship_stop_in(tx, task)?.is_some() => return Ok(None),
        Approval::Route => {}
        Approval::Explicit => {
            tx.execute(
                "UPDATE tasks SET ship_stopped_at=NULL,ship_stopped_lf_process_id=NULL,ship_stopped_person=NULL WHERE id=?1",
                [task.as_str()],
            )?;
        }
    }
    if shipping_in(tx, task)?.is_none() {
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
        tx.execute(
            "UPDATE tasks SET ship_approved_lf_process_id=?2 WHERE id=?1",
            rusqlite::params![task.as_str(), by.as_str()],
        )?;
    }
    tx.execute(
        "UPDATE tasks SET ship_lf_process_id=?2 WHERE id=?1",
        rusqlite::params![task.as_str(), by.as_str()],
    )?;
    shipping_in(tx, task)
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
            ship_in(
                tx,
                task,
                carrier,
                "its run reached the end",
                Approval::Route,
            )?;
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
pub(super) fn task_open_process_ids(selector: &str, filter: &str) -> String {
    format!(
        "SELECT process FROM ({})",
        task_open_process_pairs(selector, filter)
    )
}

/// Unfinished Processes paired with every selected Task whose work contains them.
pub(super) fn task_open_process_pairs(selector: &str, filter: &str) -> String {
    let session = session_membership("a");
    format!(
        "WITH task_scope AS MATERIALIZED (SELECT id,worktree,checkout_machine_id FROM tasks WHERE id IN ({selector})),
        open AS MATERIALIZED (SELECT p.id,p.cwd,p.agent_session_id,p.attached_lf_process_id FROM processes p INDEXED BY processes_unfinished
            WHERE p.completed_at IS NULL AND ({filter})),
        sessions AS (SELECT DISTINCT se.lf_process_id AS process,se.session_id AS session
            FROM open CROSS JOIN session_events se ON se.lf_process_id=open.id
            UNION SELECT p.attached_lf_process_id,p.agent_session_id FROM open CROSS JOIN processes p INDEXED BY agent_process_attachment ON p.attached_lf_process_id=open.id WHERE p.kind='agent'
            UNION SELECT id,agent_session_id FROM open WHERE agent_session_id IS NOT NULL)
        SELECT tw.id AS task_id,open.id AS process FROM open JOIN task_scope tw ON {}
        UNION SELECT tw.id,s.process FROM sessions s CROSS JOIN agent_sessions a ON a.id=s.session
            JOIN task_scope tw ON ({session})",
        relocated_process_membership("open")
    )
}

/// One of a Task's Graph processes with its inventory entry.
type MemberGraph = (
    crate::ops::graph_process::GraphProcess,
    crate::durable::GraphProcessInventoryEntry,
);

/// A Task's Sessions and its Graphs. The entries name the Task being read and
/// its Wave, read once for all of them.
fn members(
    tx: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<(Vec<TaskSession>, Vec<MemberGraph>)> {
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
    let graphs = graphs_of_task(tx, task)?;
    let wave = if graphs.is_empty() {
        None
    } else {
        super::durable::task_wave_in(tx, task)?
    };
    let graphs = graphs
        .into_iter()
        .map(|graph| {
            let entry = super::graph_inventory::entry_of(&graph, Some(task.clone()), wave.clone())?;
            Ok((graph, entry))
        })
        .collect::<StoreResult<Vec<_>>>()?;
    Ok((sessions, graphs))
}

impl SqliteStore {
    /// Owners and held terminals of unfinished member Sessions, without
    /// loading retained history.
    pub(crate) fn task_session_hosts(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<crate::transport::Host>> {
        use crate::transport::Host;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT DISTINCT s.transport_lf_process_id,s.held_terminal_id FROM agent_sessions s
             WHERE s.id IN ({}) AND s.completed_at IS NULL
                 AND (s.transport_lf_process_id IS NOT NULL OR s.held_terminal_id IS NOT NULL)",
            session_ids("?1")
        ))?;
        let rows = query.query_map([task.as_str()], |row| {
            Ok((
                row.get::<_, Option<LfProcessId>>(0)?,
                row.get::<_, Option<crate::id::TerminalId>>(1)?,
            ))
        })?;
        rows.map(|row| {
            // A held Session also names the launcher that started it; the
            // terminal is what hosts it.
            Ok(match row? {
                (_, Some(terminal)) => Host::Terminal(terminal),
                (Some(owner), None) => Host::Owner(owner),
                (None, None) => unreachable!("the query selects hosted Sessions"),
            })
        })
        .collect()
    }

    pub(crate) fn task_ids_in_wave(
        &self,
        wave: &crate::id::WaveId,
    ) -> StoreResult<std::collections::HashSet<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT t.id FROM projects p JOIN tasks t ON t.project_id=p.id WHERE p.wave_id=?1",
        )?;
        let rows = statement.query_map([wave], |row| row.get::<_, String>(0))?;
        rows.map(|id| {
            TaskId::parse(&id?)
                .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
        })
        .collect()
    }

    pub(crate) fn session_task_ids(&self, session: &str) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT tw.id FROM agent_sessions a JOIN tasks tw ON tw.id IN ({})
             WHERE a.id=?1 ORDER BY tw.id",
            session_tasks("a")
        ))?;
        let rows = query.query_map([session], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            TaskId::parse(&row?)
                .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
        })
        .collect()
    }

    /// Current workspace facts, without enumerating mechanical Process history.
    /// The Task's overview and, for each of its Graphs in the same order, what
    /// `lf graph show --processes` says of it.
    pub fn task_overview(
        &self,
        task: &TaskId,
    ) -> StoreResult<(
        crate::task_work::TaskOverview,
        Vec<crate::durable::GraphProcessDetail>,
    )> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let (sessions, graphs) = members(&tx, task)?;
        let graph = task_graph_row(&tx, task)?;
        tx.commit()?;
        drop(conn);
        let details = graphs
            .iter()
            .map(|(graph, entry)| graph.detail(entry.clone()))
            .collect();
        Ok((
            crate::task_work::TaskOverview {
                sessions,
                graph_processes: graphs.into_iter().map(|(_, entry)| entry).collect(),
                graph: graph.map(|row| self.read_task_graph(row)),
            },
            details,
        ))
    }

    /// Read all three owners in one SQLite snapshot, including closed history.
    pub fn task_work(&self, task: &TaskId) -> StoreResult<TaskWork> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let (sessions, graphs) = members(&tx, task)?;
        let graphs = graphs.into_iter().map(|(_, entry)| entry).collect();
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
    pub(crate) fn task_open_processes(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<crate::process::ProcessEntry>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let mut query = conn.prepare(&format!(
            "{} WHERE e.id IN ({}) ORDER BY e.started_at DESC,e.id",
            super::processes::PROCESS_SELECT,
            task_open_process_ids("?1", "1"),
        ))?;
        let processes = query
            .query_map([task.as_str()], super::processes::read_process)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(processes)
    }

    /// Whether a command with no recorded exit may still be running.
    /// Unknown is not stopped.
    pub(crate) fn process_may_run(&self, process: &crate::process::LfProcess) -> bool {
        crate::journal::lf_evidence(self, &process.id)
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
            Some((cursor, process)) => TaskGraphPosition::Run {
                at: row
                    .graph
                    .step_name_of(&cursor)
                    .unwrap_or_else(|| row.step.clone()),
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
    /// as stopped and the next `lf task run` starts it again where it stopped.
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

    /// The run `carrier` carries finished a step: its next attempt, or the
    /// next `lf task run`, sets out from `cursor`. A run that no longer
    /// carries the Task writes nothing.
    pub(crate) fn advance_task_graph_run(
        &self,
        task: &TaskId,
        carrier: &LfProcessId,
        cursor: &ExecutionCursor,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let cursor = serde_json::to_string(cursor)?;
        conn.execute(
            "UPDATE task_graphs SET run_start=?3,updated_at=?4
             WHERE task_id=?1 AND lf_process_id=?2 AND run_start IS NOT ?3",
            rusqlite::params![
                task.as_str(),
                carrier.as_str(),
                cursor,
                crate::store::rows::now_unix()
            ],
        )?;
        Ok(())
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
            ship_in(&tx, task, by, "its run reached the end", Approval::Route)?;
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

    /// Hand the Task off to finish on its own, carried by `by`, lifting any
    /// stop. Shipping that already began keeps its start; only the carrier changes.
    pub(crate) fn ship_task(
        &self,
        task: &TaskId,
        by: &LfProcessId,
        note: &str,
    ) -> StoreResult<TaskShipping> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let shipping =
            ship_in(&tx, task, by, note, Approval::Explicit)?.expect("explicit approval ships");
        tx.commit()?;
        Ok(shipping)
    }

    /// Withdraw a Task's Auto-Ship approval. With `stopped_by`, a person
    /// stopped it: the stop stands, even when it was not shipping, until
    /// `lf ship` approves it again. Returns the approval and the process that
    /// carried it, when it was approved.
    pub(crate) fn stop_shipping(
        &self,
        task: &TaskId,
        note: &str,
        stopped_by: Option<ShipStop>,
    ) -> StoreResult<Option<TaskShipping>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        if let Some(by) = stopped_by {
            tx.execute(
                "UPDATE tasks SET ship_stopped_at=?2,ship_stopped_lf_process_id=?3,ship_stopped_person=?4 WHERE id=?1",
                rusqlite::params![task.as_str(), by.at, by.by, by.person],
            )?;
        }
        let Some(shipping) = unship_in(&tx, task)? else {
            tx.commit()?;
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

    /// The person's stop that keeps a run from approving the Task, if one stands.
    pub(crate) fn task_ship_stop(&self, task: &TaskId) -> StoreResult<Option<ShipStop>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        ship_stop_in(&conn, task)
    }

    /// Handed-off Tasks whose checkout is on this Machine, with their handoffs.
    pub(crate) fn local_shipping_tasks(&self) -> StoreResult<Vec<(TaskId, TaskShipping)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "{SHIPPING_SELECT} WHERE shipped_at IS NOT NULL
             AND checkout_machine_id=(SELECT id FROM machines WHERE is_local=1)"
        ))?;
        let rows = query
            .query_map([], read_shipping)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, shipping)| {
                let id = TaskId::parse(&id)
                    .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
                Ok((id, shipping))
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

    /// Each of `tasks`' most recently launched Graph, read for all of them at once.
    pub(crate) fn latest_task_graphs(
        &self,
        tasks: &[&TaskId],
    ) -> StoreResult<HashMap<TaskId, crate::ops::graph_process::GraphProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        latest_task_graphs(&conn, tasks)
    }

    /// Each of `tasks`' completed Graph steps, read for all of them at once.
    pub(crate) fn completed_graph_steps(
        &self,
        tasks: &[&TaskId],
    ) -> StoreResult<HashMap<TaskId, HashSet<LfProcessId>>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        completed_graph_steps(&conn, tasks)
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
    use crate::process::{ProcessFilter, ProcessWorkFilter};
    use crate::session::SessionFilter;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn process_membership_upgrade_probes_cwd_and_session_indexes() {
        use crate::store::migrations::{apply_before_migration, apply_pending_migrations};
        use rusqlite::{types::Value, StatementStatus};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "session_list_indexes");
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1);
             INSERT INTO tasks(id,repository_id,issue_identifier,worktree,checkout_machine_id,created_at)
             VALUES('task_11111111111111111111111111111111','repo_11111111111111111111111111111111',
                'FIX-1','/repo/task',(SELECT id FROM machines WHERE is_local=1),1);
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id)
             VALUES('member','Member','human',1,0,'/elsewhere','task_11111111111111111111111111111111');
             WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<6000)
             INSERT INTO processes(id,trace_id,cwd,started_at,completed_at,kind)
             SELECT printf('00000000-0000-4000-8000-%012x',i),
                    printf('00000000-0000-4000-8000-%012x',i),'/unrelated/'||i,1,2,
                    CASE WHEN i>3000 THEN 'agent' ELSE 'lf' END FROM n;
             UPDATE processes SET cwd='/repo/task' WHERE id='00000000-0000-4000-8000-000000000001';
             UPDATE processes SET cwd='/repo/task/sub' WHERE id='00000000-0000-4000-8000-000000000002';
             UPDATE processes SET cwd='/repo/task-other' WHERE id='00000000-0000-4000-8000-000000000003';
             INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload)
             VALUES('member','started','history','00000000-0000-4000-8000-000000000004',1,'{}');
             UPDATE processes SET agent_session_id='member',attached_lf_process_id='00000000-0000-4000-8000-000000000005'
                WHERE id='00000000-0000-4000-8000-000000001001';
             INSERT INTO relocated_task_processes(task_id,process_id)
             VALUES('task_11111111111111111111111111111111','00000000-0000-4000-8000-000000000006');",
        ).unwrap();
        let sql = format!(
            "{} WHERE e.id IN ({}) ORDER BY e.started_at DESC,e.id",
            super::super::processes::PROCESS_SELECT,
            super::lf_process_ids(&conn, "?1").unwrap()
        );
        let read = || {
            let mut query = conn.prepare(&sql).unwrap();
            let width = query.column_count();
            let rows = query
                .query_map(["FIX-1"], |row| {
                    (0..width)
                        .map(|i| row.get::<_, Value>(i))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            (
                rows,
                query.get_status(StatementStatus::VmStep),
                query.get_status(StatementStatus::FullscanStep),
            )
        };
        let (before, before_steps, _) = read();
        apply_pending_migrations(&conn);
        let (after, after_steps, scans) = read();
        assert_eq!(after, before);
        let ids: Vec<_> = after.iter().map(|row| row[0].clone()).collect();
        assert_eq!(
            ids,
            [1, 2, 4, 5, 6, 4097].map(|i| Value::Text(format!("00000000-0000-4000-8000-{i:012x}")))
        );
        assert!(
            after_steps < before_steps / 4,
            "VM steps: {before_steps} -> {after_steps}"
        );
        assert!(scans < 100, "fullscan steps: {scans}");
        let plans: Vec<String> = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(["FIX-1"], |row| row.get(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for index in [
            "processes_cwd (cwd=?)",
            "processes_cwd (cwd>? AND cwd<?)",
            "processes_agent_session",
        ] {
            assert!(
                plans.iter().any(|line| line.contains(index)),
                "missing {index}: {plans:?}"
            );
        }
    }

    #[test]
    fn indexed_process_membership_keeps_lexical_paths_and_relocations() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1);
             INSERT INTO tasks(id,repository_id,issue_identifier,worktree,checkout_machine_id,created_at)
             VALUES('task_11111111111111111111111111111111','repo_11111111111111111111111111111111',
                'FIX-1','/repo/task',(SELECT id FROM machines WHERE is_local=1),1),
                ('task_22222222222222222222222222222222','repo_11111111111111111111111111111111',
                'FIX-2','/other',(SELECT id FROM machines WHERE is_local=1),1);",
        ).unwrap();
        for cwd in [
            None,
            Some(""),
            Some("/"),
            Some("//"),
            Some("/repo/task"),
            Some("/repo/task/"),
            Some("/repo/task//sub"),
            Some("/repo/task-other"),
            Some("/repo/task0"),
            Some("relative/sub"),
            Some("relative"),
            Some("relative-other"),
            Some("/repo/%_"),
            Some("/repo/%_/é"),
            Some("/repo/AB/x"),
            Some("/repo//task/sub"),
        ] {
            let id = LfProcessId::new();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?1,?2,1)",
                params![id, cwd],
            )
            .unwrap();
        }
        conn.execute_batch(
            "INSERT INTO relocated_task_processes(task_id,process_id)
             SELECT 'task_22222222222222222222222222222222',id FROM processes WHERE cwd='/repo/task';
             INSERT INTO relocated_task_processes(task_id,process_id)
             SELECT 'task_11111111111111111111111111111111',id FROM processes WHERE cwd='/repo/AB/x';",
        ).unwrap();
        let original = format!(
            "SELECT ae.id FROM processes ae JOIN ({}) tw ON ({}) ORDER BY ae.id",
            super::tasks("?1"),
            super::process_membership(&conn, "ae").unwrap()
        );
        let indexed = format!(
            "SELECT id FROM ({}) ORDER BY id",
            super::lf_process_ids(&conn, "?1").unwrap()
        );
        for root in [
            "/repo/task",
            "/repo/task/",
            "/repo//task//",
            "/repo/%_",
            "relative/",
            "/",
            "///",
            "",
        ] {
            for local in [true, false] {
                conn.execute("UPDATE tasks SET worktree=?1,checkout_machine_id=CASE WHEN ?2 THEN (SELECT id FROM machines WHERE is_local=1) END WHERE issue_identifier='FIX-1'", params![root,local]).unwrap();
                let read = |sql: &str| {
                    conn.prepare(sql)
                        .unwrap()
                        .query_map(["FIX-1"], |row| row.get::<_, String>(0))
                        .unwrap()
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap()
                };
                assert_eq!(
                    read(&indexed),
                    read(&original),
                    "root={root:?}, local={local}"
                );
            }
        }
    }

    #[test]
    fn session_membership_upgrade_probes_task_indexes() {
        use crate::store::migrations::{apply_before_migration, apply_pending_migrations};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "session_list_indexes");
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1);
             WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<1000)
             INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,worktree,checkout_machine_id,created_at)
             SELECT printf('task_%032x',i),'repo_11111111111111111111111111111111','FIX-'||i,'Task','/repo/'||i,
                (SELECT id FROM machines WHERE is_local=1),1 FROM n;
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id)
             VALUES('probe','Probe','human',1,0,'/repo/999/sub','task_00000000000000000000000000000001');
             UPDATE tasks SET primary_session_id='probe' WHERE worktree='/repo/999';"
        ).unwrap();
        let sql = format!(
            "SELECT (SELECT json_group_array(id) FROM ({})),
                EXISTS(SELECT 1 FROM tasks p WHERE p.primary_session_id=a.id)
             FROM agent_sessions a WHERE id=?1",
            super::session_tasks("a")
        );
        let read = || {
            let mut query = conn.prepare(&sql).unwrap();
            let result: (String, bool) = query
                .query_row(["probe"], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap();
            (result, query.get_status(rusqlite::StatementStatus::VmStep))
        };
        let (before, before_steps) = read();
        apply_pending_migrations(&conn);
        let (after, after_steps) = read();
        assert_eq!(after, before);
        assert_eq!(after, (r#"["task_00000000000000000000000000000001","task_000000000000000000000000000003e7"]"#.into(), true));
        assert!(
            after_steps < before_steps / 4,
            "VM steps: {before_steps} -> {after_steps}"
        );
        let plans: Vec<String> = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap()
            .query_map(["probe"], |row| row.get(3))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            plans
                .iter()
                .any(|line| line.contains("tasks_checkout_root") && line.contains("<expr>=?")),
            "{plans:?}"
        );
        assert!(
            plans
                .iter()
                .any(|line| line.contains("tasks_primary_session")),
            "{plans:?}"
        );
        assert_eq!(
            conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
    }

    #[test]
    fn reverse_session_membership_keeps_lexical_checkout_and_binding_rules() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let repository = store.ensure_repository("/repo").unwrap();
        let remote = crate::durable::MachineId::new();
        store.add_machine(&remote, "remote", "remote").unwrap();
        let roots = [
            Some("/"),
            Some("/repo"),
            Some("/repo/task///"),
            Some("/repo/task/nested"),
            Some("/repo//task"),
            Some("relative"),
            Some("relative//nested/"),
            Some("/literal%_"),
            Some(""),
            None,
            Some("/foreign"),
        ];
        let tasks: Vec<_> = roots.iter().map(|_| TaskId::new()).collect();
        {
            let conn = store.conn.lock().unwrap();
            for (index, (id, root)) in tasks.iter().zip(roots).enumerate() {
                conn.execute(
                    "INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,worktree,checkout_machine_id,created_at)
                     VALUES(?1,?2,?1,'Task',?3,CASE WHEN ?4 THEN ?5 ELSE (SELECT id FROM machines WHERE is_local=1) END,1)",
                    params![id.as_str(), repository.as_str(), root, index == 10, remote.as_str()],
                ).unwrap();
            }
        }
        let mut case = 0;
        for (cwd, expected) in [
            ("/repo/task/nested/file", vec![0, 1, 2, 3]),
            ("/repo/task///", vec![0, 1, 2]),
            ("/repo/task//nested/file", vec![0, 1, 2]),
            ("/repo//task/file", vec![0, 1, 4]),
            ("/repo/task-other", vec![0, 1]),
            ("/repository", vec![0]),
            ("relative//nested/file", vec![5, 6]),
            ("relative/", vec![5]),
            ("elsewhere", vec![]),
            ("", vec![0]),
            ("///", vec![0]),
            ("/literal%_/file", vec![0, 7]),
            ("/literalAB/file", vec![0]),
            ("/foreign/file", vec![0]),
        ] {
            for binding in [None, Some(2), Some(8), Some(9), Some(10)] {
                for scope in [None, Some("repository"), Some("wave")] {
                    let session = format!("lexical-{case}");
                    case += 1;
                    {
                        let conn = store.conn.lock().unwrap();
                        conn.execute(
                            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,primary_scope,interactive)
                             VALUES(?1,'Lexical','human',1,0,?2,?3,?4,1)",
                            params![session, cwd, binding.map(|index| tasks[index].as_str()), scope],
                        ).unwrap();
                        super::super::sessions::test_capture(
                            &conn,
                            &session,
                            &crate::session_record::new_artifact_key(),
                        );
                        conn.execute(
                            "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
                            params![tasks[2].as_str(), session],
                        )
                        .unwrap();
                    }
                    let mut ids: Vec<_> = if scope.is_none() {
                        expected
                            .iter()
                            .copied()
                            .chain(binding)
                            .map(|index| tasks[index].clone())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
                    ids.dedup();
                    assert_eq!(
                        store.session_task_ids(&session).unwrap(),
                        ids,
                        "{cwd:?}, {binding:?}, {scope:?}"
                    );
                    let summary = store.session_summary(&session, 0).unwrap().unwrap();
                    assert_eq!(summary.task_ids, ids);
                    assert!(summary.task_primary);
                }
            }
        }
        assert!(store.session_task_ids("missing").unwrap().is_empty());
    }

    #[test]
    fn request_stop_retains_its_person_without_an_os_process() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: TaskId::new(),
                repository_id: store.ensure_repository("/request-stop").unwrap(),
                project_id: None,
                title: "Stop".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let stop = super::ShipStop {
            at: 25,
            by: None,
            person: Some("Casey".into()),
        };
        assert!(store
            .stop_shipping(&task.id, "Casey", Some(stop.clone()))
            .unwrap()
            .is_none());
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(store.task_ship_stop(&task.id).unwrap(), Some(stop));
        let count: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

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
                    interactive_name: None,
                    returns: None,
                    sources: Vec::new(),
                },
            )],
        };
        store.take_up_task_graph(&task, &definition).unwrap();
        let before = store.task_graph(&task).unwrap().unwrap();
        let row = store.task_graph_row(&task).unwrap().unwrap();
        let start = crate::graph::runner::ExecutionCursor::default();
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
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
                "UPDATE tasks SET checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert_eq!(
            store.local_shipping_tasks().unwrap(),
            [(task.clone(), shipping.clone())]
        );
        // Shipping again keeps when it began.
        let again = store.ship_task(&task, &by, "again").unwrap();
        assert_eq!(again.shipped_at, shipping.shipped_at);
        // Stop withdraws the approval; the run still going keeps it moving.
        assert_eq!(shipping.approved_by, Some(by.clone()));
        let stopper = LfProcessId::new();
        assert_eq!(
            store
                .stop_shipping(
                    &task,
                    "stop",
                    Some(super::ShipStop {
                        at: crate::store::rows::now_unix(),
                        by: Some(stopper.clone()),
                        person: None
                    })
                )
                .unwrap(),
            Some(shipping)
        );
        assert_eq!(state(), crate::durable::TaskState::Moving);
        assert_eq!(store.stop_shipping(&task, "stop", None).unwrap(), None);
        // The person's stop stands: a relaunch running to the end does not approve it.
        let relaunch = store.task_graph_row(&task).unwrap().unwrap();
        assert!(store
            .start_task_graph_run(&task, &relaunch, &start, &by, true)
            .unwrap());
        assert!(store.task_shipping(&task).unwrap().is_none());
        assert_eq!(state(), crate::durable::TaskState::Moving);
        assert_eq!(
            store.task_ship_stop(&task).unwrap().unwrap().by,
            Some(stopper)
        );
        // Approved again with nothing running, it is idle until its ship process
        // restarts. Only `lf ship` lifts the stop.
        store.ship_task(&task, &by, "again").unwrap();
        assert!(store.task_ship_stop(&task).unwrap().is_none());
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
            for (id, n, worktree) in [
                (&task, 1, "/repo/task/"),
                (&sibling, 2, "/repo/task-other"),
                (&idle, 3, "/repo/idle"),
            ] {
                conn.execute("INSERT INTO tasks(repository_id,id,project_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES('repo_proof',?1,?2,?3,?4,1,(SELECT id FROM machines WHERE is_local=1))", params![id.as_str(), project.as_str(), format!("PROOF-{n}"), worktree]).unwrap();
            }
            for (cwd, owner) in [
                ("/repo/task", Some(&task)),
                ("/repo/task-other", Some(&sibling)),
                ("/repo/task/deep/er", Some(&task)),
                ("/repo/task-other/x", Some(&sibling)),
                ("/elsewhere", None),
            ] {
                let id = LfProcessId::new();
                conn.execute(
                    "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,?3,1)",
                    params![id, TraceId::new(), cwd],
                )
                .unwrap();
                graphs.push((id, owner));
            }
        }
        let graph = crate::graph::compiled::CompiledGraph::new("proof", &[]);
        for (id, owner) in &graphs {
            store.record_graph_process(id, &graph, *owner).unwrap();
        }
        let latest = store.latest_task_graphs(&[&task, &sibling, &idle]).unwrap();
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
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/repo/task',1,(SELECT id FROM machines WHERE is_local=1))", params![task.as_str(), project.as_str()]).unwrap();
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
                .prepare(&super::task_open_process_ids("?1", "1"))
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
        // A Graph started for the Task is its work wherever it ran, and other
        // Tasks' Graphs cost one Task's reading nothing.
        conn.execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id) VALUES(?1,'proof','{}',?2)",
            params![finished, task.as_str()],
        )
        .unwrap();
        let pairs = super::task_graph_pairs("?1");
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
        let others: Vec<String> = conn
            .prepare("SELECT id FROM tasks WHERE id!=?1")
            .unwrap()
            .query_map([task.as_str()], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for other in &others {
            let graph = LfProcessId::new();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/unrelated',1)",
                params![graph, TraceId::new()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id) VALUES(?1,'proof','{}',?2)",
                params![graph, other],
            )
            .unwrap();
        }
        let after = read_graphs();
        assert!(
            after <= before * 2,
            "Other Tasks' Graphs increased one Task's Graph reading: {before} → {after}"
        );
        // Every Task's Graphs come from the Graph records, so Sessions that
        // reach no Graph cost nothing however many Tasks are read. On October
        // 18, 2026 each Task rescanned all 4,246 of Jack Heart's Sessions:
        // 2.3 s for 622 Tasks.
        let every_task = super::task_graph_pairs("SELECT id FROM tasks");
        let read_every_task = || {
            let mut statement = conn.prepare(&every_task).unwrap();
            let pairs = statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .count();
            assert_eq!(pairs, others.len() + 1);
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
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','Retained Task',?3,1,(SELECT id FROM machines WHERE is_local=1))",params![task.as_str(),project.as_str(),repo.path().to_str().unwrap()]).unwrap();
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
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/repo/task',1,(SELECT id FROM machines WHERE is_local=1))",params![task.as_str(),project.as_str()]).unwrap();
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
        // A Wave conversation has a `wave_id`, so it is not an orphan.
        for expected in ["b-repo", "d-orphan"] {
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
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1','/missing/task%_',1,(SELECT id FROM machines WHERE is_local=1))", params![task.as_str(), project.as_str()]).unwrap();
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
            .all(|process| process.completed_at().is_none()));
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
        assert!(open
            .iter()
            .any(|process| process.id() == unfinished.as_str()));
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
        let ids = |processes: Vec<crate::process::ProcessEntry>| {
            processes
                .iter()
                .map(|process| process.id().to_owned())
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
            .any(|process| process.id() == mechanical.as_str()));
        assert!(work
            .processes
            .iter()
            .any(|process| process.id() == bound_process.as_str()));
        assert_eq!(
            store.session_task_ids("manual").unwrap(),
            std::slice::from_ref(&task)
        );
        assert!(store.session_task_ids("sibling").unwrap().is_empty());
        let summaries = store
            .session_summaries(
                &SessionFilter {
                    task: Some("PROOF-1".into()),
                    headless: true,
                    ..Default::default()
                },
                0,
            )
            .unwrap();
        assert_eq!(
            summaries.len(),
            work.sessions
                .iter()
                .filter(|session| session.completed_at.is_none())
                .count()
        );
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
                &ProcessFilter {
                    performed_work: Some(ProcessWorkFilter::Task(task.clone())),
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
                .map(crate::process::ProcessEntry::id)
                .collect::<Vec<_>>(),
            [graph.steps[0].process.id.as_str()]
        );

        let child = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'child','PROOF-2','/missing/task%_/child',1,(SELECT id FROM machines WHERE is_local=1))", params![child.as_str(), project.as_str()]).unwrap();
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id) VALUES('child-session','Child','human',1,0,'/missing/task%_/child',?1,?2)", params![child.as_str(), wave]).unwrap();
        }
        // A Graph in a nested checkout started for the deepest Task, as its
        // launch binds it; the enclosing Task keeps the nested Session.
        let child_graph = store.test_graph(
            "child",
            "/missing/task%_/child",
            &[("implement", None)],
            None,
        );
        let graph_ids = |task: &TaskId| {
            store
                .task_work(task)
                .unwrap()
                .graph_processes
                .into_iter()
                .map(|graph| graph.summary.id)
                .collect::<Vec<_>>()
        };
        let enclosing = graph_ids(&task);
        assert!(!enclosing.contains(&child_graph.to_string()));
        assert_eq!(graph_ids(&child), [child_graph.to_string()]);
        let work = store.task_work(&task).unwrap();
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
        // Without its checkout the Task keeps the Graphs started for it.
        let work = store.task_work(&task).unwrap();
        assert_eq!(
            work.sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            ["history"]
        );
        assert_eq!(graph_ids(&task), enclosing);

        // A Session that later names a taskless Graph does not make it the
        // Task's: the Graph's Task is what its launch recorded.
        let taskless = store.test_graph("taskless", "/elsewhere", &[], None);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload) VALUES('history','started','taskless-graph',?1,3,'{}')", [&taskless]).unwrap();
        }
        assert_eq!(graph_ids(&task), enclosing);
        let (_, entry) = store.graph_process(taskless.as_str()).unwrap().unwrap();
        assert_eq!(entry.summary.task_id, None);
    }
}
