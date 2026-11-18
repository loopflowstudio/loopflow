//! One association rule for CLI and Desktop. Membership is additive to binding;
//! it never rewrites event attribution or follows causal parent Processes.

use std::collections::{HashMap, HashSet};

use crate::task_session::TaskSession;
use crate::StoreResult;
use loopflow_model::durable::TaskId;
use loopflow_model::id::LfProcessId;

use crate::graph_process::{Execution, ExecutionStart};

use super::SqliteStore;

pub const START: &str = "start";
pub const END: &str = "end";

#[derive(Debug)]
pub struct TaskOverviewRecords {
    pub sessions: Vec<TaskSession>,
    pub graph_processes: Vec<crate::execution::GraphProcessInventoryEntry>,
    pub graph: Option<SavedTaskGraphRow>,
}
#[derive(Debug)]
pub struct TaskWorkRecords {
    pub sessions: Vec<TaskSession>,
    pub graph_processes: Vec<crate::execution::GraphProcessInventoryEntry>,
    pub processes: Vec<crate::process::ProcessEntry>,
    pub graph: Option<SavedTaskGraphRow>,
}

/// Each Task with this Machine's checkout of it: `worktree` is NULL for a Task
/// not checked out here.
const TASK_CHECKOUTS: &str =
    "SELECT t.id,c.worktree FROM tasks t LEFT JOIN task_checkouts c ON c.task_id=t.id";

fn tasks(selector: &str) -> String {
    format!("{TASK_CHECKOUTS} WHERE t.id={selector} OR t.issue_identifier={selector} OR t.external_issue_id={selector}")
}

fn checkout(cwd: &str) -> String {
    // Component boundaries avoid /repo.task matching /repo.task-other. Paths
    // remain usable after checkout removal; SQL LIKE would treat '%' as syntax.
    // `tw` is any relation with the Task's `id`; the checkout is looked up here.
    format!("EXISTS(SELECT 1 FROM task_checkouts c WHERE c.task_id=tw.id AND ({cwd}=rtrim(c.worktree,'/') OR instr({cwd},rtrim(c.worktree,'/')||'/')=1))")
}

fn has_relocations(conn: &rusqlite::Connection) -> StoreResult<bool> {
    // Read-only clients also inspect released databases before promotion.
    // Those schemas cannot have relocation receipts; leave them untouched.
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='relocated_task_processes' AND type='table')",
        [], |row| row.get(0),
    )?)
}

/// The cwd-membership reading `lf_process_ids` must agree with.
#[cfg(test)]
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
/// `task_checkouts_root` rather than every Task. Explicit binding also counts.
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
         SELECT c.task_id FROM up CROSS JOIN task_checkouts c
            ON rtrim(c.worktree,'/')=up.path
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
        WHERE tw.worktree IS NOT NULL
            AND (ae.cwd=rtrim(tw.worktree,'/') OR
                (ae.cwd>=rtrim(tw.worktree,'/')||'/' AND ae.cwd<rtrim(tw.worktree,'/')||'0'))
            {unmoved}
        {relocated}
        UNION SELECT se.lf_process_id FROM session_events se INDEXED BY session_process_membership WHERE se.session_id IN (SELECT id FROM members) AND se.lf_process_id IS NOT NULL
        UNION SELECT h.lf_process_id FROM history.session_history h WHERE h.session_id IN (SELECT id FROM members) AND h.lf_process_id IS NOT NULL
        UNION SELECT t.lf_process_id FROM history.turns t WHERE t.session_id IN (SELECT id FROM members) AND t.lf_process_id IS NOT NULL
        UNION SELECT p.attached_lf_process_id FROM processes p WHERE p.kind='agent' AND p.agent_session_id IN (SELECT id FROM members) AND p.attached_lf_process_id IS NOT NULL
        UNION SELECT p.id FROM processes p WHERE p.kind='agent' AND p.agent_session_id IN (SELECT id FROM members)",
        session_ids(selector), tasks(selector)))
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
) -> StoreResult<Vec<crate::graph_process::GraphExecution>> {
    super::graph_inventory::graphs_in(
        conn,
        &format!(
            "g.lf_process_id IN (SELECT graph FROM ({}))",
            task_graph_pairs("?1")
        ),
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
         WHERE gs.ended_at IS NOT NULL",
        task_graph_pairs("SELECT value FROM json_each(?1)")
    ))?;
    let ids = super::children::ids_json(tasks.iter().map(|id| id.as_str()));
    let mut completed = HashMap::<TaskId, HashSet<LfProcessId>>::new();
    for row in query.query_map([&ids], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (task, step) = row?;
        let invalid =
            |error: &dyn std::fmt::Display| crate::StoreError::InvalidData(error.to_string());
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
) -> StoreResult<HashMap<TaskId, crate::graph_process::GraphExecution>> {
    let mut latest = HashMap::new();
    let mut query = conn.prepare(&format!(
        "SELECT task_id, graph FROM ({}) pairs
         ORDER BY (SELECT g.rowid FROM graph_processes g WHERE g.lf_process_id=pairs.graph)",
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
        "g.lf_process_id IN (SELECT value FROM json_each(?1))",
        &[&ids],
    )?
    .into_iter()
    .map(|graph| (graph.run.id.to_string(), graph))
    .collect();
    latest
        .into_iter()
        .map(|(task, graph)| {
            let task = TaskId::parse(&task)
                .map_err(|error| crate::StoreError::InvalidData(error.to_string()))?;
            let graph = graphs.get(&graph).cloned().ok_or_else(|| {
                crate::StoreError::InvalidData(format!("Graph {graph} has no Graph process"))
            })?;
            Ok((task, graph))
        })
        .collect()
}

/// The Task's Graph row: the graph it took up, the step it stands at or its
/// run left, and that run with the cursor its next attempt sets out from: where
/// it began, then the step after the last one it finished.
#[derive(Debug)]
pub struct SavedTaskGraphRow {
    pub graph: String,
    pub step: String,
    pub run: Option<(String, crate::graph_process::TaskCarrier)>,
}

/// A Task's Graph as another Machine takes it up: the graph it took up, the
/// step it stands at, and the cursor its run sets out from again, if it is on
/// one. The run's Process stays behind.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedTaskGraphRecord {
    pub graph: String,
    pub step: String,
    pub run_start: Option<String>,
}

fn task_graph_row(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Option<SavedTaskGraphRow>> {
    use rusqlite::OptionalExtension;
    let Some((graph, step, start, carrier)) = conn
        .query_row(
            "SELECT graph,step,run_start,lf_process_id,run_pid,run_os_started_at
             FROM task_graphs WHERE task_id=?1",
            [task.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<LfProcessId>>(3)?
                        .map(|id| -> rusqlite::Result<_> {
                            Ok(crate::graph_process::TaskCarrier {
                                id,
                                pid: row.get(4)?,
                                os_started_at: row.get(5)?,
                            })
                        })
                        .transpose()?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let run = start.zip(carrier);
    Ok(Some(SavedTaskGraphRow { graph, step, run }))
}

/// Put the Task on a run from `cursor`, carried by `by` as it started at
/// `start`, if it still stands where the caller read it: at `step`, with `stopped` the run that last left
/// it, if any. With `ships`, the run reaches `end` and hands the Task off,
/// granting merge authority unless a person stopped shipping it.
#[allow(clippy::too_many_arguments)]
fn start_run_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    (step, stopped): (&str, Option<&LfProcessId>),
    cursor: &str,
    (by, start): (&LfProcessId, &ExecutionStart),
    ships: bool,
) -> StoreResult<bool> {
    let left = tx.execute(
        "UPDATE task_graphs SET run_start=?2,lf_process_id=?3,run_pid=?7,run_os_started_at=?8,
            updated_at=?4
         WHERE task_id=?1 AND step=?5 AND lf_process_id IS ?6",
        rusqlite::params![
            task.as_str(),
            cursor,
            by.as_str(),
            crate::rows::now_unix(),
            step,
            stopped.map(LfProcessId::as_str),
            start.pid,
            start.os_started_at
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
            (by, start),
            "it runs to the end with no interactive step left",
            Approval::Route,
        )?;
    }
    Ok(true)
}

/// A Task handed off to finish on its own: since when, the process that
/// approved it, this Machine's run carrying it there, and how many times in a
/// row that run was started again after it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskShipping {
    pub shipped_at: i64,
    pub approved_by: Option<LfProcessId>,
    pub carrier: Option<Execution>,
    pub restarts: u32,
}

/// How a Task comes to ship. A person's stop blocks only a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approval {
    /// `lf ship`: approves it and lifts any stop.
    Explicit,
    /// A run reaching `end`: approves it unless a person stopped shipping.
    Route,
}

/// A person's `lf ship --stop`: when, and the process that ran it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShipStop {
    pub at: i64,
    pub by: Option<LfProcessId>,
    pub person: Option<String>,
}

pub(super) fn ship_stop_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Option<ShipStop>> {
    Ok(ship_log_in(conn, task)?.stop)
}

/// One kind of row in a Task's Auto-Ship log (`task_ship_events`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ShipEvent {
    Approved,
    Carried,
    Restarted,
    Stopped,
    Withdrawn,
}

impl ShipEvent {
    fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Carried => "carried",
            Self::Restarted => "restarted",
            Self::Stopped => "stopped",
            Self::Withdrawn => "withdrawn",
        }
    }
}

/// The latest Auto-Ship approval, as a move carries it to the Task's next Machine.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShipApproval {
    pub at: i64,
    pub by: Option<LfProcessId>,
}

/// What a Task's Auto-Ship log says now.
#[derive(Debug, Default)]
struct ShipLog {
    approval: Option<ShipApproval>,
    carrier: Option<Execution>,
    restarts: u32,
    stop: Option<ShipStop>,
}

/// Restarts counted in a row. The repair backoff a restart waits
/// (`ops::pr_landing::repair_backoff`) stops growing after this many, so
/// later ones add no row; the migration's backfill keeps the same cap.
pub const SHIP_RESTARTS_COUNTED: u32 = 6;

/// Read from the latest approval or stop: each starts the log over.
fn ship_log_in(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<ShipLog> {
    let mut query = conn.prepare_cached(
        "SELECT kind,at,lf_process_id,person,pid,os_started_at,ended_at,exit_code,error
         FROM task_ship_events WHERE task_id=?1
         AND id>=IFNULL((SELECT max(id) FROM task_ship_events
             WHERE task_id=?1 AND kind IN ('approved','stopped')),0)
         ORDER BY id",
    )?;
    let mut rows = query.query([task.as_str()])?;
    let mut log = ShipLog::default();
    while let Some(row) = rows.next()? {
        let at: i64 = row.get(1)?;
        let process: Option<LfProcessId> = row.get(2)?;
        let run = |id: &LfProcessId| -> rusqlite::Result<Execution> {
            Ok(Execution {
                id: id.clone(),
                started_at: at,
                pid: row.get(4)?,
                os_started_at: row.get(5)?,
                ended_at: row.get(6)?,
                exit_code: row.get(7)?,
                error: row.get(8)?,
            })
        };
        match serde_json::from_value::<ShipEvent>(row.get::<_, String>(0)?.into())? {
            ShipEvent::Approved => {
                log = ShipLog {
                    carrier: process.as_ref().map(run).transpose()?,
                    approval: Some(ShipApproval { at, by: process }),
                    ..ShipLog::default()
                }
            }
            ShipEvent::Carried => log.carrier = process.as_ref().map(run).transpose()?,
            ShipEvent::Restarted => log.restarts += 1,
            ShipEvent::Stopped => {
                log = ShipLog {
                    stop: Some(ShipStop {
                        at,
                        by: process,
                        person: row.get(3)?,
                    }),
                    ..ShipLog::default()
                }
            }
            ShipEvent::Withdrawn => {
                log = ShipLog {
                    stop: log.stop.take(),
                    ..ShipLog::default()
                }
            }
        }
    }
    Ok(log)
}

/// A Task has one carrier: a new `carried` row replaces the last. An
/// approving or carrying lf on this Machine records the OS process it runs as.
fn append_ship_event_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
    event: ShipEvent,
    at: i64,
    (process, start): (Option<&LfProcessId>, Option<&ExecutionStart>),
    person: Option<&str>,
) -> StoreResult<()> {
    if event == ShipEvent::Carried {
        conn.execute(
            "DELETE FROM task_ship_events WHERE task_id=?1 AND kind='carried'",
            [task.as_str()],
        )?;
    }
    conn.execute(
        "INSERT INTO task_ship_events(task_id,kind,at,lf_process_id,person,pid,os_started_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            task.as_str(),
            event.as_str(),
            at,
            process.map(LfProcessId::as_str),
            person,
            start.and_then(|start| start.pid),
            start.and_then(|start| start.os_started_at)
        ],
    )?;
    Ok(())
}

pub(super) fn shipping_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskShipping>> {
    use rusqlite::OptionalExtension;
    let Some(shipped_at) = conn
        .query_row(
            "SELECT shipped_at FROM tasks WHERE id=?1 AND shipped_at IS NOT NULL",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?
    else {
        return Ok(None);
    };
    let log = ship_log_in(conn, task)?;
    Ok(Some(TaskShipping {
        shipped_at,
        approved_by: log.approval.and_then(|approval| approval.by),
        carrier: log.carrier,
        restarts: log.restarts,
    }))
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
    }
    let log = ship_log_in(conn, task)?;
    if log.approval.is_some() || log.carrier.is_some() || log.restarts > 0 {
        let now = crate::rows::now_unix();
        append_ship_event_in(conn, task, ShipEvent::Withdrawn, now, (None, None), None)?;
    }
    Ok(shipping)
}

/// Approve Auto-Ship, carried by `by`. A route writes nothing and returns
/// `None` while a person's stop stands; an explicit approval lifts the stop.
fn ship_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    (by, start): (&LfProcessId, &ExecutionStart),
    note: &str,
    approval: Approval,
) -> StoreResult<Option<TaskShipping>> {
    let state = super::durable::task_state_in(tx, task)?;
    if state.is_terminal() {
        return Err(crate::StoreError::InvalidAuthority(format!(
            "Task is {state}; it cannot ship"
        )));
    }
    let log = ship_log_in(tx, task)?;
    if approval == Approval::Route && log.stop.is_some() {
        return Ok(None);
    }
    let now = crate::rows::now_unix();
    let shipping = shipping_in(tx, task)?.is_some();
    if !shipping {
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
    }
    // `lf ship` also lifts a stop that stands beside shipping a peer recorded.
    if !shipping || log.stop.is_some() {
        append_ship_event_in(
            tx,
            task,
            ShipEvent::Approved,
            now,
            (Some(by), Some(start)),
            None,
        )?;
    } else if log.carrier.as_ref().map(|carrier| &carrier.id) != Some(by) {
        append_ship_event_in(
            tx,
            task,
            ShipEvent::Carried,
            now,
            (Some(by), Some(start)),
            None,
        )?;
    }
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
    use rusqlite::OptionalExtension;
    // The carrier's OS process, which a hand-off at `end` makes its ship carrier's.
    let start = tx
        .query_row(
            "SELECT run_pid,run_os_started_at FROM task_graphs WHERE task_id=?1 AND lf_process_id=?2",
            rusqlite::params![task.as_str(), carrier.as_str()],
            |row| {
                Ok(ExecutionStart {
                    started_at: crate::rows::now_unix(),
                    pid: row.get(0)?,
                    os_started_at: row.get(1)?,
                })
            },
        )
        .optional()?;
    let arrived = tx.execute(
        "UPDATE task_graphs SET step=?2,run_start=NULL,lf_process_id=NULL,run_pid=NULL,run_os_started_at=NULL,updated_at=?3
         WHERE task_id=?1 AND lf_process_id=?4",
        rusqlite::params![
            task.as_str(),
            step,
            crate::rows::now_unix(),
            carrier.as_str()
        ],
    )? > 0;
    let ended = arrived && step == END;
    if ended {
        // A routed path may reach end even though another path had an
        // interactive step, so launch-time prediction is not sufficient.
        if !super::durable::task_state_in(tx, task)?.is_terminal() {
            let start = start.expect("an arrived run was on the Task");
            ship_in(
                tx,
                task,
                (carrier, &start),
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
        "UPDATE task_graphs SET step=?2,run_start=NULL,lf_process_id=NULL,run_pid=NULL,run_os_started_at=NULL,updated_at=?3 WHERE task_id=?1",
        rusqlite::params![task.as_str(), step, crate::rows::now_unix()],
    )? > 0)
}

/// Match unfinished Processes through checkout, captured inputs and
/// attachments; state only, never `history.db`.
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
        "WITH task_scope AS MATERIALIZED (SELECT scoped.id,c.worktree FROM tasks scoped
            LEFT JOIN task_checkouts c ON c.task_id=scoped.id WHERE scoped.id IN ({selector})),
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
    crate::graph_process::GraphExecution,
    crate::execution::GraphProcessInventoryEntry,
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
    Ok((sessions, member_graphs(tx, task)?))
}

/// A Task's Graphs with their inventory entries.
fn member_graphs(tx: &rusqlite::Connection, task: &TaskId) -> StoreResult<Vec<MemberGraph>> {
    let graphs = graphs_of_task(tx, task)?;
    let wave = if graphs.is_empty() {
        None
    } else {
        super::durable::task_wave_in(tx, task)?
    };
    graphs
        .into_iter()
        .map(|graph| {
            let entry = super::graph_inventory::entry_of(&graph, Some(task.clone()), wave.clone())?;
            Ok((graph, entry))
        })
        .collect()
}

impl SqliteStore {
    /// Held terminals of unfinished member Sessions, without loading
    /// retained history.
    pub fn task_session_terminals(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<loopflow_model::id::TerminalId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT DISTINCT s.held_terminal_id FROM agent_sessions s
             WHERE s.id IN ({}) AND s.completed_at IS NULL AND s.held_terminal_id IS NOT NULL",
            session_ids("?1")
        ))?;
        let terminals = query
            .query_map([task.as_str()], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(terminals)
    }

    pub fn task_ids_in_wave(
        &self,
        wave: &loopflow_model::id::WaveId,
    ) -> StoreResult<std::collections::HashSet<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT t.id FROM projects p JOIN tasks t ON t.project_id=p.id WHERE p.wave_id=?1",
        )?;
        let rows = statement.query_map([wave], |row| row.get::<_, String>(0))?;
        rows.map(|id| {
            TaskId::parse(&id?).map_err(|error| crate::StoreError::InvalidData(error.to_string()))
        })
        .collect()
    }

    pub fn session_task_ids(&self, session: &str) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT tw.id FROM agent_sessions a JOIN tasks tw ON tw.id IN ({})
             WHERE a.id=?1 ORDER BY tw.id",
            session_tasks("a")
        ))?;
        let rows = query.query_map([session], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            TaskId::parse(&row?).map_err(|error| crate::StoreError::InvalidData(error.to_string()))
        })
        .collect()
    }

    /// Current workspace facts, without enumerating mechanical Process history.
    /// The Task's overview and, for each of its Graphs in the same order, what
    /// `lf graph show --processes` says of it.
    pub fn task_overview_records(
        &self,
        task: &TaskId,
    ) -> StoreResult<(
        TaskOverviewRecords,
        Vec<crate::execution::GraphProcessDetail>,
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
            TaskOverviewRecords {
                sessions,
                graph_processes: graphs.into_iter().map(|(_, entry)| entry).collect(),
                graph,
            },
            details,
        ))
    }

    /// The Task's Graph position and what `lf graph show --processes` says of
    /// each of its Graphs, without listing its Sessions: a hook reads this.
    pub fn task_progress_records(
        &self,
        task: &TaskId,
    ) -> StoreResult<(
        Option<SavedTaskGraphRow>,
        Vec<crate::execution::GraphProcessDetail>,
    )> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let graphs = member_graphs(&tx, task)?;
        let graph = task_graph_row(&tx, task)?;
        tx.commit()?;
        drop(conn);
        let details = graphs
            .iter()
            .map(|(graph, entry)| graph.detail(entry.clone()))
            .collect();
        Ok((graph, details))
    }

    /// Read all three owners in one SQLite snapshot, including closed history.
    pub fn task_work_records(&self, task: &TaskId) -> StoreResult<TaskWorkRecords> {
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
        Ok(TaskWorkRecords {
            sessions,
            graph_processes: graphs,
            processes,
            graph,
        })
    }

    /// Single-Task execution gates need neither other Tasks nor retained Session/Graph payloads.
    pub fn task_open_processes(
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

    /// The Task's Graph as captured, for running it.
    pub fn task_graph_row(&self, task: &TaskId) -> StoreResult<Option<SavedTaskGraphRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_graph_row(&conn, task)
    }

    /// The Task's Graph as it moves to another Machine.
    pub fn task_graph_record(&self, task: &TaskId) -> StoreResult<Option<SavedTaskGraphRecord>> {
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
        Ok(Some(SavedTaskGraphRecord {
            graph,
            step,
            run_start,
        }))
    }

    /// Take up a Graph that moved here. A Task on a run stays on it, carried
    /// by `by`, the receiving Process: it ends with the move, so the run reads
    /// as stopped and the next `lf task run` starts it again where it stopped.
    pub fn place_moved_task_graph(
        &self,
        task: &TaskId,
        record: &SavedTaskGraphRecord,
        by: Option<&LfProcessId>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let run = record.run_start.as_ref().zip(by);
        conn.execute(
            "INSERT OR REPLACE INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6)",
            rusqlite::params![
                task.as_str(),
                record.graph,
                record.step,
                run.map(|(start, _)| start),
                run.map(|(_, by)| by.as_str()),
                crate::rows::now_unix()
            ],
        )?;
        Ok(())
    }

    /// Take up `graph` for the Task as it is defined now, at `start`. It
    /// replaces the Task's earlier Graph.
    pub fn take_up_task_graph(&self, task: &TaskId, graph: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT OR REPLACE INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,?3,?4)",
            rusqlite::params![task.as_str(), graph, START, crate::rows::now_unix()],
        )?;
        Ok(())
    }

    /// Put the Task on a run from `cursor`, carried by `by`. Returns false,
    /// writing nothing, when the Task no longer stands where `row` read it, so
    /// two runs cannot both set out.
    pub fn start_task_graph_run(
        &self,
        task: &TaskId,
        step: &str,
        stopped: Option<&LfProcessId>,
        cursor: &str,
        carrier: (&LfProcessId, &ExecutionStart),
        ships: bool,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let started = start_run_in(&tx, task, (step, stopped), cursor, carrier, ships)?;
        tx.commit()?;
        Ok(started)
    }

    /// The run `carrier` carries finished a step: its next attempt, or the
    /// next `lf task run`, sets out from `cursor`. A run that no longer
    /// carries the Task writes nothing.
    pub fn advance_task_graph_run(
        &self,
        task: &TaskId,
        carrier: &LfProcessId,
        cursor: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_graphs SET run_start=?3,updated_at=?4
             WHERE task_id=?1 AND lf_process_id=?2 AND run_start IS NOT ?3",
            rusqlite::params![
                task.as_str(),
                carrier.as_str(),
                cursor,
                crate::rows::now_unix()
            ],
        )?;
        Ok(())
    }

    /// The run `carrier` carries stopped at `step` (or reached `end`): put the
    /// Task there. Returns whether it reached `end`, requesting completion.
    pub fn arrive_task_graph(
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
    pub fn end_task_graph(
        &self,
        task: &TaskId,
        by: (&LfProcessId, &ExecutionStart),
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let ended = tx.execute(
            "UPDATE task_graphs SET step=?2,updated_at=?3 WHERE task_id=?1 AND step!=?2 AND lf_process_id IS NULL",
            rusqlite::params![task.as_str(), END, crate::rows::now_unix()],
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
    pub fn set_task_graph_step(&self, task: &TaskId, step: &str) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let set = set_step_in(&tx, task, step)?;
        tx.commit()?;
        Ok(set)
    }

    /// Hand the Task off to finish on its own, carried by `by`, lifting any
    /// stop. Shipping that already began keeps its start; only the carrier changes.
    pub fn ship_task(
        &self,
        task: &TaskId,
        by: (&LfProcessId, &ExecutionStart),
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
    pub fn stop_shipping(
        &self,
        task: &TaskId,
        note: &str,
        stopped_by: Option<ShipStop>,
    ) -> StoreResult<Option<TaskShipping>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let shipping = shipping_in(&tx, task)?;
        if let Some(by) = stopped_by {
            append_ship_event_in(
                &tx,
                task,
                ShipEvent::Stopped,
                by.at,
                (by.by.as_ref(), None),
                by.person.as_deref(),
            )?;
        }
        unship_in(&tx, task)?;
        let Some(shipping) = shipping else {
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
    pub fn task_shipping(&self, task: &TaskId) -> StoreResult<Option<TaskShipping>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        shipping_in(&conn, task)
    }

    /// The person's stop that keeps a run from approving the Task, if one stands.
    pub fn task_ship_stop(&self, task: &TaskId) -> StoreResult<Option<ShipStop>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        ship_stop_in(&conn, task)
    }

    /// Handed-off Tasks whose checkout is on this Machine, with their handoffs.
    pub fn local_shipping_tasks(&self) -> StoreResult<Vec<(TaskId, TaskShipping)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let ids = {
            let mut query = conn.prepare(
                "SELECT id FROM tasks WHERE shipped_at IS NOT NULL
                 AND id IN (SELECT task_id FROM task_checkouts)",
            )?;
            let rows = query.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let mut shipping = Vec::new();
        for id in ids {
            let id = TaskId::parse(&id)
                .map_err(|error| crate::StoreError::InvalidData(error.to_string()))?;
            if let Some(handoff) = shipping_in(&conn, &id)? {
                shipping.push((id, handoff));
            }
        }
        Ok(shipping)
    }

    /// Count one more restart after a failed ship process, up to
    /// [`SHIP_RESTARTS_COUNTED`].
    pub fn count_ship_restart(&self, task: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if ship_log_in(&conn, task)?.restarts >= SHIP_RESTARTS_COUNTED {
            return Ok(());
        }
        let now = crate::rows::now_unix();
        append_ship_event_in(&conn, task, ShipEvent::Restarted, now, (None, None), None)
    }

    /// A ship step completed: the next failure restarts without waiting long.
    pub fn reset_ship_restarts(&self, task: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "DELETE FROM task_ship_events WHERE task_id=?1 AND kind='restarted'",
            [task.as_str()],
        )?;
        Ok(())
    }

    /// The Task's standing Auto-Ship approval, which a move carries along.
    pub fn task_ship_approval(&self, task: &TaskId) -> StoreResult<Option<ShipApproval>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if shipping_in(&conn, task)?.is_none() {
            return Ok(None);
        }
        Ok(ship_log_in(&conn, task)?.approval)
    }

    /// A move arrived carrying `approval`: the Task ships here as it did
    /// where it came from, from the same start, with no carrier yet.
    pub fn take_in_ship_approval(&self, task: &TaskId, approval: &ShipApproval) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        if shipping_in(&tx, task)?.is_none() {
            super::planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    task.as_str(),
                    &[crate::work::edit::WorkValue::TaskShippedAt(Some(
                        approval.at,
                    ))],
                ),
            )?;
        }
        if ship_log_in(&tx, task)?.approval.as_ref() != Some(approval) {
            append_ship_event_in(
                &tx,
                task,
                ShipEvent::Approved,
                approval.at,
                (approval.by.as_ref(), None),
                None,
            )?;
            // The approving Process ran where the Task came from; nothing carries it here yet.
            append_ship_event_in(
                &tx,
                task,
                ShipEvent::Carried,
                approval.at,
                (None, None),
                None,
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Each of `tasks`' most recently launched Graph, read for all of them at once.
    pub fn latest_task_graphs(
        &self,
        tasks: &[&TaskId],
    ) -> StoreResult<HashMap<TaskId, crate::graph_process::GraphExecution>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        latest_task_graphs(&conn, tasks)
    }

    /// Each of `tasks`' completed Graph steps, read for all of them at once.
    pub fn completed_graph_steps(
        &self,
        tasks: &[&TaskId],
    ) -> StoreResult<HashMap<TaskId, HashSet<LfProcessId>>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        completed_graph_steps(&conn, tasks)
    }

    /// Every Graph among the Task's work, oldest first.
    pub fn task_graphs(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<crate::graph_process::GraphExecution>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        graphs_of_task(&conn, task)
    }
}

#[cfg(test)]
mod tests {

    use rusqlite::params;

    use loopflow_model::durable::{ProjectId, TaskId};
    use loopflow_model::id::{LfProcessId, TraceId, WaveId};

    #[test]
    fn process_membership_upgrade_probes_cwd_and_session_indexes() {
        use crate::migration_runner::{apply_before_migration, apply_pending_migrations};
        use rusqlite::{types::Value, StatementStatus};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "session_list_indexes",
        );
        crate::history::attach_empty(&conn);
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
             INSERT INTO history.turns(session_id,provider_thread,provider_turn,lf_process_id,started_at)
             VALUES('member','thread','turn','00000000-0000-4000-8000-000000000004',1);
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
        // The upgrade moves the released checkout into `task_checkouts`.
        apply_pending_migrations(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
        );
        let (after, _, scans) = read();
        let ids: Vec<_> = after.iter().map(|row| row[0].clone()).collect();
        assert_eq!(
            ids,
            [1, 2, 4, 5, 6, 4097].map(|i| Value::Text(format!("00000000-0000-4000-8000-{i:012x}")))
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
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1);
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
             VALUES('task_11111111111111111111111111111111','repo_11111111111111111111111111111111','FIX-1',1),
                ('task_22222222222222222222222222222222','repo_11111111111111111111111111111111','FIX-2',1);
             INSERT INTO task_checkouts(task_id,worktree)
             VALUES('task_11111111111111111111111111111111','/repo/task'),
                ('task_22222222222222222222222222222222','/other');",
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
                let task = "task_11111111111111111111111111111111";
                conn.execute("DELETE FROM task_checkouts WHERE task_id=?1", [task])
                    .unwrap();
                if local && !root.is_empty() {
                    conn.execute(
                        "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
                        params![task, root],
                    )
                    .unwrap();
                }
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
        use crate::migration_runner::{apply_before_migration, apply_pending_migrations};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "session_list_indexes",
        );
        crate::history::attach_empty(&conn);
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
                EXISTS(SELECT 1 FROM task_primary_sessions p WHERE p.session_id=a.id)
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
        // The upgrade moves the released checkouts into `task_checkouts` and the
        // primary Session into `task_primary_sessions`.
        apply_pending_migrations(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
        );
        let (after, _) = read();
        assert_eq!(after, (r#"["task_00000000000000000000000000000001","task_000000000000000000000000000003e7"]"#.into(), true));
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
                .any(|line| line.contains("task_checkouts_root") && line.contains("<expr>=?")),
            "{plans:?}"
        );
        assert!(
            plans
                .iter()
                .any(|line| line.contains("task_primary_sessions") && line.contains("INDEX")),
            "{plans:?}"
        );
        assert_eq!(
            conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
    }

    #[test]
    fn request_stop_retains_its_person_without_an_os_process() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = crate::test_fixture::open(&path).unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: TaskId::new(),
                repository_id: crate::test_fixture::repository(&store, "/request-stop"),
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
        let store = crate::test_fixture::open(&path).unwrap();
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
    fn end_request_is_atomic_and_survives_reopening_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = crate::test_fixture::open(&path).unwrap();
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
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,workspace_slug,created_at,updated_at,issue_title,issue_description,pm_snapshot_synced_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?1),?2,?1,'issue','PROOF-1','proof',1,1,'Findings','Accepted findings',1)",params![project.as_str(),task.as_str()]).unwrap();
            conn.execute("INSERT INTO task_checkouts(task_id,worktree,branch,base_commit) VALUES(?1,'/repo','proof','head')",params![task.as_str()]).unwrap();
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
        let graph = store.task_graph_row(&task).unwrap().map(|row| row.graph);
        drop(store);
        let store = crate::test_fixture::open(&path).unwrap();
        assert_eq!(
            store.task_completion_pending(&task).unwrap(),
            Some((request, "provider unavailable".into()))
        );
        let retained = store.task(&task).unwrap().unwrap();
        assert!(store.complete_task(&retained, request).unwrap());
        assert!(store.complete_task(&retained, request).unwrap());
        assert!(store.task_completion_pending(&task).unwrap().is_none());
        assert_eq!(
            store.task_graph_row(&task).unwrap().map(|row| row.graph),
            graph
        );
        assert!(store.task(&task).unwrap().unwrap().completed_at.is_some());
        let conn = store.conn.lock().unwrap();
        let completed: i64 = conn.query_row("SELECT count(*) FROM task_events WHERE task_id=?1 AND json_extract(kind_json,'$.kind')='completed'",[task.as_str()],|row| row.get(0)).unwrap();
        assert_eq!(completed, 1);
        drop(conn);
        store.reopen_task(&task, Some("Continue findings")).unwrap();
        assert!(!store.complete_task(&retained, request).unwrap());
        assert!(store.task_completion_pending(&task).unwrap().is_none());
        assert_eq!(
            store.task_graph_row(&task).unwrap().map(|row| row.graph),
            graph
        );
        assert!(store.task(&task).unwrap().unwrap().completed_at.is_none());
    }

    #[test]
    fn every_task_latest_graph_matches_its_own_graphs() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
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
                conn.execute("INSERT INTO tasks(repository_id,id,project_id,issue_identifier,created_at) VALUES('repo_proof',?1,?2,?3,1)",params![id.as_str(),project.as_str(),format!("PROOF-{n}")]).unwrap();
                conn.execute(
                    "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
                    params![id.as_str(), worktree],
                )
                .unwrap();
            }
            for (cwd, owner) in [
                ("/repo/task", Some(&task)),
                ("/repo/task-other", Some(&sibling)),
                ("/repo/task/deep/er", Some(&task)),
                ("/repo/task-other/x", Some(&sibling)),
                ("/elsewhere", None),
            ] {
                graphs.push((LfProcessId::new(), cwd, owner));
            }
        }
        let graph = crate::graph::CompiledGraph {
            name: "proof".into(),
            steps: vec![],
        };
        for (id, cwd, owner) in &graphs {
            let start = crate::graph_process::ExecutionStart::reserved(1);
            store
                .record_graph(id, &graph, *owner, &start, None, Some(cwd))
                .unwrap();
        }
        let latest = store.latest_task_graphs(&[&task, &sibling, &idle]).unwrap();
        for id in [&task, &sibling, &idle] {
            assert_eq!(
                latest.get(id),
                store.task_graphs(id).unwrap().last(),
                "{id}"
            );
        }
        assert_eq!(latest[&task].cwd.as_deref(), Some("/repo/task/deep/er"));
        assert_eq!(latest[&sibling].cwd.as_deref(), Some("/repo/task-other/x"));
        assert!(!latest.contains_key(&idle));
    }

    #[test]
    fn process_membership_does_not_read_events_that_name_no_process() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
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
        conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,created_at) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?1),?2,?1,'issue','PROOF-1',1)",params![project.as_str(),task.as_str()]).unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo/task')",
            params![task.as_str()],
        )
        .unwrap();
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
                "INSERT INTO tasks(repository_id,id,project_id,issue_identifier,created_at)
                VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,?3,1)",
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

    #[test]
    fn a_moved_task_ships_where_it_arrives_from_its_approval_with_no_carrier() {
        let (laptop_dir, mini_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let id = TaskId::new();
        let open = |dir: &tempfile::TempDir| {
            let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
            store
                .create_task(&crate::work::task::NewTask {
                    id: id.clone(),
                    repository_id: crate::test_fixture::repository(&store, "/moving"),
                    project_id: None,
                    title: "Move".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
            store
        };
        let (laptop, mini) = (open(&laptop_dir), open(&mini_dir));
        let approver = LfProcessId::new();
        let shipping = laptop
            .ship_task(
                &id,
                (
                    &approver,
                    &crate::graph_process::ExecutionStart::reserved(1),
                ),
                "ship",
            )
            .unwrap();
        let approval = laptop.task_ship_approval(&id).unwrap().unwrap();
        assert_eq!(approval.by, Some(approver.clone()));
        // Delivered twice, as a retried move is.
        mini.take_in_ship_approval(&id, &approval).unwrap();
        mini.take_in_ship_approval(&id, &approval).unwrap();
        assert_eq!(
            mini.task_shipping(&id).unwrap(),
            Some(super::TaskShipping {
                shipped_at: approval.at,
                approved_by: Some(approver),
                carrier: None,
                restarts: 0,
            })
        );
        assert_eq!(shipping.shipped_at, approval.at);
    }

    #[test]
    fn a_ship_process_failing_all_day_keeps_its_log_short() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        let id = TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                id: id.clone(),
                repository_id: crate::test_fixture::repository(&store, "/shipping"),
                project_id: None,
                title: "Ship".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .ship_task(
                &id,
                (
                    &LfProcessId::new(),
                    &crate::graph_process::ExecutionStart::reserved(1),
                ),
                "ship",
            )
            .unwrap();
        // A day of restarts at the longest backoff, each carried by a new process.
        let mut carrier = LfProcessId::new();
        for _ in 0..48 {
            store.count_ship_restart(&id).unwrap();
            carrier = LfProcessId::new();
            store
                .ship_task(
                    &id,
                    (&carrier, &crate::graph_process::ExecutionStart::reserved(1)),
                    "ship",
                )
                .unwrap();
        }
        let shipping = store.task_shipping(&id).unwrap().unwrap();
        assert_eq!(shipping.restarts, super::SHIP_RESTARTS_COUNTED);
        assert_eq!(shipping.carrier.map(|run| run.id), Some(carrier));
        let rows = || -> i64 {
            let conn = store.conn.lock().unwrap();
            conn.query_row(
                "SELECT count(*) FROM task_ship_events WHERE task_id=?1",
                [id.as_str()],
                |row| row.get(0),
            )
            .unwrap()
        };
        // The approval, its carrier and the counted restarts.
        assert_eq!(rows(), 2 + i64::from(super::SHIP_RESTARTS_COUNTED));

        store.reset_ship_restarts(&id).unwrap();
        assert_eq!(store.task_shipping(&id).unwrap().unwrap().restarts, 0);
        assert_eq!(rows(), 2);
    }

    #[test]
    fn a_released_tasks_shipping_columns_become_its_auto_ship_log() {
        use crate::migration_runner::{apply_before_migration, upgrade_from_migration};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_machine_local",
        );
        let (shipping, stopped) = (TaskId::new(), TaskId::new());
        let (approver, carrier) = (LfProcessId::new(), LfProcessId::new());
        // The carrier failed; the upgrade carries its end onto the ship log.
        conn.execute_batch(&format!(
            "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES('{approver}','t1','/repo',1);
             INSERT INTO processes(id,trace_id,cwd,started_at,completed_at,outcome,exit_code,error)
             VALUES('{carrier}','t2','/repo',2,12,'failed',1,'provider unavailable');
             INSERT INTO repository_plans(id,repo,selected) VALUES('repo_1','/repo',1);
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at,shipped_at,
                 ship_approved_lf_process_id,ship_lf_process_id,ship_restarts)
             VALUES('{shipping}','repo_1','SHIP-1',1,10,'{approver}','{carrier}',2);
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at,ship_stopped_at,
                 ship_stopped_person)
             VALUES('{stopped}','repo_1','SHIP-2',1,20,'Casey');"
        ))
        .unwrap();
        upgrade_from_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_machine_local",
        );
        let handoff = super::shipping_in(&conn, &shipping).unwrap().unwrap();
        assert_eq!(
            (handoff.shipped_at, handoff.approved_by, handoff.restarts),
            (10, Some(approver), 2)
        );
        let run = handoff.carrier.unwrap();
        assert_eq!(run.id, carrier);
        assert_eq!(run.result(), crate::graph_process::ExecutionResult::Failed);
        assert_eq!(run.error.as_deref(), Some("provider unavailable"));
        assert_eq!(super::ship_stop_in(&conn, &shipping).unwrap(), None);
        assert_eq!(super::shipping_in(&conn, &stopped).unwrap(), None);
        assert_eq!(
            super::ship_stop_in(&conn, &stopped).unwrap(),
            Some(super::ShipStop {
                at: 20,
                by: None,
                person: Some("Casey".into()),
            })
        );
        assert!(conn
            .execute("UPDATE task_ship_events SET at=0", [])
            .is_err());
    }
}
