//! One association rule for CLI and Desktop. A Task's work is its Sessions and
//! its Graph runs; each records its Task once, when it starts
//! ([`starting_task_in`]), and reads follow that column, never a path.

use std::collections::{HashMap, HashSet};

use crate::task_session::TaskSession;
use crate::StoreResult;
use loopflow_model::durable::TaskId;
use loopflow_model::id::{LfProcessId, LfSessionId};

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
    pub graph: Option<SavedTaskGraphRow>,
}

fn tasks(selector: &str) -> String {
    format!("SELECT id FROM tasks WHERE id={selector} OR issue_identifier={selector} OR external_issue_id={selector}")
}

/// Whether a Session counts toward its `task_id`: Repository and Wave
/// primaries keep their own scope.
pub(super) fn counts_for_task(session: &str) -> String {
    format!("{session}.primary_scope IS NULL")
}

/// A Session belongs to the Task it recorded when it was created.
pub(super) fn session_membership(session: &str) -> String {
    format!("{} AND {session}.task_id=tw.id", counts_for_task(session))
}

pub(super) fn session_ids(selector: &str) -> String {
    format!(
        "SELECT a.id FROM agent_sessions a JOIN ({}) tw ON ({})",
        tasks(selector),
        session_membership("a")
    )
}

/// The Task a Session belongs to, as a subquery of at most one row.
pub(super) fn session_tasks(session: &str) -> String {
    format!(
        "SELECT {session}.task_id AS id WHERE {session}.task_id IS NOT NULL AND {}",
        counts_for_task(session)
    )
}

/// SQL: directory `cwd` is `root` or below it. Whole path components:
/// `/repo.task` does not hold `/repo.task-other`.
pub(super) fn within(cwd: &str, root: &str) -> String {
    format!("({cwd}=rtrim({root},'/') OR instr({cwd},rtrim({root},'/')||'/')=1)")
}

/// The Task whose checkout here is the deepest one holding `cwd`, as a scalar
/// subquery. Probes checkout roots from the cwd outward, so each ancestor is
/// one index seek; relative paths walk the same way as absolute ones.
pub(super) fn deepest_checkout(cwd: &str) -> String {
    format!(
        "(WITH RECURSIVE up(path) AS (
            SELECT rtrim({cwd},'/') WHERE {cwd} IS NOT NULL
            UNION ALL
            SELECT rtrim(rtrim(path,replace(path,'/','')),'/') FROM up WHERE path!='')
         SELECT c.task_id FROM up CROSS JOIN task_checkouts c INDEXED BY task_checkouts_root
            ON rtrim(c.worktree,'/')=up.path
         WHERE up.path!='' OR substr({cwd},1,1)='/' OR {cwd}=''
         ORDER BY length(up.path) DESC, c.task_id LIMIT 1)"
    )
}

/// The Task a Session or a Graph run belongs to, read once as it starts: the
/// Task it names, else the deepest checkout holding its directory on this
/// Machine, else the Task of the Session that started it. A command run inside
/// a checkout is that Task's work whoever ran it.
pub(super) fn starting_task_in(
    conn: &rusqlite::Connection,
    named: Option<&TaskId>,
    cwd: Option<&str>,
    started_by: Option<&LfSessionId>,
) -> StoreResult<Option<TaskId>> {
    if let Some(named) = named {
        return Ok(Some(named.clone()));
    }
    Ok(conn
        .query_row(
            &format!(
                "SELECT coalesce({},(SELECT s.task_id FROM agent_sessions s WHERE s.id=?2 AND {}))",
                deepest_checkout("?1"),
                counts_for_task("s")
            ),
            rusqlite::params![cwd, started_by],
            |row| row.get::<_, Option<String>>(0),
        )?
        .map(TaskId::from_raw))
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
    /// The Task's auto state, which names who takes part next.
    pub auto: Option<AutoState>,
}

/// A Task's Graph as another Machine takes it up: the graph it took up, the
/// step it stands at and the cursor its run sets out from again, if it is on
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
            "SELECT graph,step,run_start,lf_process_id,run_pid,run_os_started_at,run_died_at
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
                                died_at: row.get(6)?,
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
    let auto = auto_in(conn, task)?.map(|mark| mark.state);
    Ok(Some(SavedTaskGraphRow {
        graph,
        step,
        run,
        auto,
    }))
}

/// Put the Task on a run from `cursor`, carried by `by` as it started at
/// `start`, if it still stands where the caller read it: at `step`, with `stopped` the run that last left
/// it, if any.
fn start_run_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    (step, stopped): (&str, Option<&LfProcessId>),
    cursor: &str,
    (by, start): (&LfProcessId, &ExecutionStart),
) -> StoreResult<bool> {
    let left = tx.execute(
        "UPDATE task_graphs SET run_start=?2,lf_process_id=?3,run_pid=?7,run_os_started_at=?8,
            run_died_at=NULL,updated_at=?4
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
    Ok(left > 0)
}

/// How many times a Task's run was carried again after its carrier died, since
/// it last finished a step, and when it last was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskRunRestarts {
    pub count: u32,
    pub at: Option<i64>,
}

fn run_restarts_in(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<TaskRunRestarts> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT restarts,restarted_at FROM task_graphs WHERE task_id=?1",
            [task.as_str()],
            |row| {
                Ok(TaskRunRestarts {
                    count: row.get(0)?,
                    at: row.get(1)?,
                })
            },
        )
        .optional()?
        .unwrap_or(TaskRunRestarts { count: 0, at: None }))
}

/// Auto, a setting on the Task: whether its interactive steps run headless,
/// standing in for the person, and whether its landing may merge.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AutoState {
    /// `lf task auto on` or an `auto: on` configuration turned it on.
    On,
    /// `lf task auto off` turned it off; `-b` leaves it off.
    Off,
}

impl AutoState {
    /// The word people type and the store keeps: `on` or `off`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }

    pub(super) fn parse(value: &str) -> StoreResult<Self> {
        match value {
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            other => Err(crate::StoreError::InvalidData(format!(
                "unknown auto state {other:?}"
            ))),
        }
    }
}

/// The Task's auto mark: its state, when it was set and who set it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct Auto {
    pub state: AutoState,
    pub at: i64,
    pub by: crate::process::Starter,
}

/// Write the Task's mark, replacing or keeping (`on_conflict`) one it holds.
/// Returns how many rows changed.
fn write_auto_in(
    conn: &rusqlite::Connection,
    task: &TaskId,
    mark: &Auto,
    on_conflict: &str,
) -> StoreResult<usize> {
    Ok(conn.execute(
        &format!(
            "INSERT OR {on_conflict} INTO task_auto(task_id,state,set_at,set_by) VALUES(?1,?2,?3,?4)"
        ),
        rusqlite::params![
            task.as_str(),
            mark.state.as_str(),
            mark.at,
            serde_json::to_string(&mark.by)?
        ],
    )?)
}

pub(super) fn auto_in(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<Option<Auto>> {
    use rusqlite::OptionalExtension;
    let Some((state, at, by)) = conn
        .query_row(
            "SELECT state,set_at,set_by FROM task_auto WHERE task_id=?1",
            [task.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    Ok(Some(Auto {
        state: AutoState::parse(&state)?,
        at,
        by: serde_json::from_str(&by)?,
    }))
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
        "UPDATE task_graphs SET step=?2,run_start=NULL,lf_process_id=NULL,run_pid=NULL,run_os_started_at=NULL,
             run_died_at=NULL,
             updated_at=?3,restarts=0,restarted_at=NULL
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
        super::children::request_completion_in(tx, task, None)?;
    }
    Ok(ended)
}

fn set_step_in(tx: &rusqlite::Transaction<'_>, task: &TaskId, step: &str) -> StoreResult<bool> {
    Ok(tx.execute(
        "UPDATE task_graphs SET step=?2,run_start=NULL,lf_process_id=NULL,run_pid=NULL,run_os_started_at=NULL,
             run_died_at=NULL,
             updated_at=?3,restarts=0,restarted_at=NULL WHERE task_id=?1",
        rusqlite::params![task.as_str(), step, crate::rows::now_unix()],
    )? > 0)
}

/// Unended Graph runs paired with each selected Task they occupy: the Task a
/// run recorded, or for a run that recorded none, the Task whose checkout
/// holds it. A Task can take a checkout a run is already in; that run stays
/// out of the Task's history and still keeps a second run from starting
/// beside it. The few unended runs seed it.
pub(super) fn task_open_graph_pairs(selector: &str) -> String {
    format!(
        "SELECT tw.id AS task_id,g.lf_process_id AS graph
         FROM graph_processes g INDEXED BY graph_processes_unended
         JOIN tasks tw ON tw.id IN ({selector}) AND (g.task_id=tw.id OR (g.task_id IS NULL
            AND EXISTS(SELECT 1 FROM task_checkouts c WHERE c.task_id=tw.id AND {})))
         WHERE g.ended_at IS NULL",
        within("g.cwd", "c.worktree")
    )
}

/// What runs in a Task's work, or in a checkout no Task holds: an open
/// Session whose agent started and has not ended, or a Graph run that has
/// not ended. Whether it still runs is asked of the OS by its own pid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenWork {
    Session {
        id: loopflow_model::id::LfSessionId,
        agent: loopflow_model::id::AgentProcessId,
        /// The lf that attached its current agent.
        launcher: Option<LfProcessId>,
        held_terminal: Option<loopflow_model::id::TerminalId>,
    },
    Graph {
        run: Execution,
        held_terminal: Option<loopflow_model::id::TerminalId>,
    },
}

impl OpenWork {
    /// What a person names it by: the Session or the Graph run.
    pub fn label(&self) -> String {
        match self {
            Self::Session { id, .. } => format!("Session {id}"),
            Self::Graph { run, .. } => format!("Graph run {}", run.id),
        }
    }

    pub fn held_terminal(&self) -> Option<&loopflow_model::id::TerminalId> {
        match self {
            Self::Session { held_terminal, .. } | Self::Graph { held_terminal, .. } => {
                held_terminal.as_ref()
            }
        }
    }
}

/// Open work whose Session `a` or Graph run `g` matches the scopes, which
/// share their parameters.
fn open_work_in(
    conn: &rusqlite::Connection,
    sessions: &str,
    graphs: &str,
    values: &[&dyn rusqlite::ToSql],
) -> StoreResult<Vec<OpenWork>> {
    let mut work = conn
        .prepare(&format!(
            "SELECT a.id,a.agent_process_id,a.agent_launcher_lf_process_id,a.held_terminal_id
             FROM agent_sessions a INDEXED BY agent_sessions_unended_agent
             WHERE a.agent_process_id IS NOT NULL AND a.agent_ended_at IS NULL
                AND a.agent_started_at IS NOT NULL AND a.completed_at IS NULL AND ({sessions})
             ORDER BY a.created_at,a.id"
        ))?
        .query_map(values, |row| {
            Ok(OpenWork::Session {
                id: row.get(0)?,
                agent: row.get(1)?,
                launcher: row.get(2)?,
                held_terminal: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let runs = conn
        .prepare(&format!(
            "SELECT {},g.held_terminal_id FROM graph_processes g INDEXED BY graph_processes_unended
             WHERE g.ended_at IS NULL AND ({graphs}) ORDER BY g.rowid",
            super::graph_inventory::run_columns("g")
        ))?
        .query_map(values, |row| {
            Ok(OpenWork::Graph {
                run: super::graph_inventory::read_run(row)?,
                held_terminal: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    work.extend(runs);
    Ok(work)
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
    /// The Task holding `dir` on this Machine: the one whose checkout is the
    /// deepest holding it, so a checkout nested inside another is its own
    /// Task's; two Tasks recorded on one root resolve to the lower id. `dir`
    /// matches as named and as it resolves through symlinks. Every "which Task
    /// is this directory" question asks here.
    pub fn task_holding(&self, dir: &std::path::Path) -> StoreResult<Option<TaskId>> {
        let resolved = dir.canonicalize().ok().filter(|resolved| resolved != dir);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                &format!(
                    "SELECT coalesce({},{})",
                    deepest_checkout("?1"),
                    deepest_checkout("?2")
                ),
                rusqlite::params![
                    dir.to_string_lossy(),
                    resolved.as_deref().map(std::path::Path::to_string_lossy)
                ],
                |row| row.get::<_, Option<String>>(0),
            )?
            .map(TaskId::from_raw))
    }

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

    /// The Task's overview and, for each of its Graphs in the same order, what
    /// `lf graph show` says of it.
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

    /// The Task's Graph position and what `lf graph show` says of
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

    /// Read the Task's Sessions, Graph runs and Graph in one SQLite snapshot,
    /// including closed history.
    pub fn task_work_records(&self, task: &TaskId) -> StoreResult<TaskWorkRecords> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let (sessions, graphs) = members(&tx, task)?;
        let graphs = graphs.into_iter().map(|(_, entry)| entry).collect();
        let graph = task_graph_row(&tx, task)?;
        tx.commit()?;
        drop(conn);
        Ok(TaskWorkRecords {
            sessions,
            graph_processes: graphs,
            graph,
        })
    }

    /// What runs in the Task's work, for execution gates; no history.
    pub fn task_open_work(&self, task: &TaskId) -> StoreResult<Vec<OpenWork>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        open_work_in(
            &conn,
            &format!(
                "EXISTS(SELECT 1 FROM tasks tw WHERE tw.id=?1 AND ({}))",
                session_membership("a")
            ),
            &format!(
                "g.lf_process_id IN (SELECT graph FROM ({}))",
                task_open_graph_pairs("?1")
            ),
            &[&task.as_str()],
        )
    }

    /// What runs in `worktree` or below it, for a checkout no Task holds.
    pub fn checkout_open_work(&self, worktree: &std::path::Path) -> StoreResult<Vec<OpenWork>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        open_work_in(
            &conn,
            &within("a.cwd", "?1"),
            &within("g.cwd", "?1"),
            &[&worktree.to_string_lossy()],
        )
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

    /// A Task that moved here holds the auto mark it left with, or none.
    pub fn place_moved_task_auto(&self, task: &TaskId, mark: Option<&Auto>) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        match mark {
            Some(mark) => write_auto_in(&conn, task, mark, "REPLACE")?,
            None => conn.execute("DELETE FROM task_auto WHERE task_id=?1", [task.as_str()])?,
        };
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

    /// Take `graph` up for a Task that has none. Returns false, writing
    /// nothing, when it has one: another run took its Graph up first.
    pub fn take_up_first_task_graph(&self, task: &TaskId, graph: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "INSERT OR IGNORE INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,?3,?4)",
            rusqlite::params![task.as_str(), graph, START, crate::rows::now_unix()],
        )? > 0)
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
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let started = start_run_in(&tx, task, (step, stopped), cursor, carrier)?;
        tx.commit()?;
        Ok(started)
    }

    /// The run `carrier` carries finished a step: its next attempt, or the
    /// next `lf task run`, sets out from `cursor`, and a later death of its
    /// carrier is carried again at once. A run that no longer carries the
    /// Task writes nothing.
    pub fn advance_task_graph_run(
        &self,
        task: &TaskId,
        carrier: &LfProcessId,
        cursor: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_graphs SET run_start=?3,updated_at=?4,restarts=0,restarted_at=NULL
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
    pub fn end_task_graph(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let ended = tx.execute(
            "UPDATE task_graphs SET step=?2,updated_at=?3 WHERE task_id=?1 AND step!=?2 AND lf_process_id IS NULL",
            rusqlite::params![task.as_str(), END, crate::rows::now_unix()],
        )? > 0;
        if ended {
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

    /// Set the Task's auto mark, noting it in the Task's history. With
    /// `unset_only`, a standing mark stays and nothing is written. Returns the
    /// mark it had.
    pub fn set_task_auto(
        &self,
        task: &TaskId,
        mark: &Auto,
        unset_only: bool,
    ) -> StoreResult<Option<Auto>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let before = auto_in(&tx, task)?;
        let on_conflict = if unset_only { "IGNORE" } else { "REPLACE" };
        if write_auto_in(&tx, task, mark, on_conflict)? == 0 {
            return Ok(before);
        }
        if before.as_ref().map(|before| before.state) != Some(mark.state) {
            let summary = format!("auto {}: {}", mark.state.as_str(), mark.by);
            super::children::insert_task_event_in(
                &tx,
                task,
                &crate::work::task::TaskEventKind::Progress { summary },
            )?;
        }
        tx.commit()?;
        Ok(before)
    }

    /// The Task's auto mark, if anyone set one.
    pub fn task_auto(&self, task: &TaskId) -> StoreResult<Option<Auto>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        auto_in(&conn, task)
    }

    /// Tasks checked out here whose run's carrier has not cleared its OS
    /// process, or was found dead.
    pub fn local_task_runs(&self) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT task_id FROM task_graphs WHERE (run_pid IS NOT NULL OR run_died_at IS NOT NULL)
             AND task_id IN (SELECT task_id FROM task_checkouts)",
        )?;
        let rows = query.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|id| {
            TaskId::parse(&id?).map_err(|error| crate::StoreError::InvalidData(error.to_string()))
        })
        .collect()
    }

    /// How many times the Task's run was carried again since it last finished
    /// a step, and when it last was.
    pub fn task_run_restarts(&self, task: &TaskId) -> StoreResult<TaskRunRestarts> {
        run_restarts_in(&self.conn.lock().expect("store mutex poisoned"), task)
    }

    /// Count one more start of the Task's run, if it still stands where the
    /// caller read it, carried by the `carrier` that died. Returns false,
    /// writing nothing, when the carrier recorded its own exit or another
    /// start came first.
    pub fn count_task_run_restart(
        &self,
        task: &TaskId,
        carrier: &LfProcessId,
        seen: u32,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE task_graphs SET restarts=restarts+1,restarted_at=?4
             WHERE task_id=?1 AND lf_process_id=?2 AND restarts=?3
               AND (run_pid IS NOT NULL OR run_died_at IS NOT NULL)",
            rusqlite::params![
                task.as_str(),
                carrier.as_str(),
                seen,
                crate::rows::now_unix()
            ],
        )? > 0)
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
    use loopflow_model::id::{LfProcessId, LfSessionId, TraceId, WaveId};

    use super::OpenWork;

    #[test]
    fn work_keeps_its_task_when_its_repository_moves_and_its_path_is_reused() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        let repository = crate::test_fixture::repository(&store, "/repo");
        let task = |title: &str| {
            store
                .create_task(&crate::work::task::NewTask {
                    id: TaskId::new(),
                    repository_id: repository.clone(),
                    project_id: None,
                    title: title.into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap()
                .id
        };
        let (first, second) = (task("First"), task("Second"));
        let checkout = |task: &TaskId| {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo/task')",
                    [task.as_str()],
                )
                .unwrap();
        };
        // Neither names a Task: each belongs to the checkout it starts in.
        let conversation = |cwd: &str| {
            let mut session = crate::test_fixture::unpublished_conversation(None, None, 1);
            session.work_source = None;
            session.interactive = true;
            session.cwd = cwd.into();
            store.create_session_prepared(session, None).unwrap().id
        };
        let graph = crate::graph::CompiledGraph {
            name: "proof".into(),
            steps: vec![],
        };
        let run = |cwd: &str, started_by: Option<&LfSessionId>| {
            let id = LfProcessId::new();
            let mut start = crate::graph_process::ExecutionStart::reserved(1);
            start.caller_session = started_by.cloned();
            store
                .record_graph(&id, &graph, None, &start, None, Some(cwd), None)
                .unwrap();
            id
        };
        checkout(&first);
        let first_conversation = conversation("/repo/task/src");
        let first_run = run("/repo/task/src", None);
        // A run the conversation starts elsewhere is its Task's work too,
        // and still is once the conversation is the Task's primary.
        let elsewhere = run("/tmp", Some(&first_conversation));
        store
            .choose_task_primary(&first, first_conversation.as_str())
            .unwrap();
        let from_primary = run("/tmp", Some(&first_conversation));
        // The repository moves, and another Task's clone takes the old path.
        store.relocate_repository(&repository, "/moved").unwrap();
        checkout(&second);
        let second_conversation = conversation("/repo/task");
        // The checkout holding a run outranks the Session that started it.
        let second_run = run("/repo/task", Some(&first_conversation));
        let work = |task: &TaskId| {
            let records = store.task_work_records(task).unwrap();
            let sessions: Vec<_> = records.sessions.into_iter().map(|s| s.id).collect();
            let runs: Vec<_> = store
                .task_graphs(task)
                .unwrap()
                .iter()
                .map(|graph| graph.id().clone())
                .collect();
            assert_eq!(records.graph_processes.len(), runs.len());
            (sessions, runs)
        };
        assert_eq!(
            work(&first),
            (
                vec![first_conversation],
                vec![first_run, elsewhere, from_primary]
            )
        );
        assert_eq!(
            work(&second),
            (vec![second_conversation], vec![second_run.clone()])
        );
        let open: Vec<_> = store
            .task_open_work(&second)
            .unwrap()
            .iter()
            .map(OpenWork::label)
            .collect();
        assert_eq!(open, [format!("Graph run {second_run}")]);
    }

    #[test]
    fn the_task_holding_a_directory_is_its_deepest_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        let repository = crate::test_fixture::repository(&store, "/repo");
        let task = |worktree: &str| {
            let id = store
                .create_task(&crate::work::task::NewTask {
                    id: TaskId::new(),
                    repository_id: repository.clone(),
                    project_id: None,
                    title: worktree.into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap()
                .id;
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
                    [id.as_str(), worktree],
                )
                .unwrap();
            id
        };
        let (outer, nested) = (task("/repo/outer"), task("/repo/outer/nested"));
        let (one, two) = (task("/repo/shared"), task("/repo/shared/"));
        let holding = |path: &str| store.task_holding(std::path::Path::new(path)).unwrap();
        assert_eq!(holding("/repo/outer/src"), Some(outer.clone()));
        assert_eq!(holding("/repo/outer/nested/src"), Some(nested));
        // A sibling sharing a name prefix is not inside the checkout.
        assert_eq!(holding("/repo/outer/nestedness"), Some(outer));
        let lower = if one.as_str() < two.as_str() {
            one
        } else {
            two
        };
        assert_eq!(holding("/repo/shared"), Some(lower));
        assert_eq!(holding("/repo"), None);
        assert_eq!(holding("/elsewhere"), None);
        // A directory named through a symlink is in the checkout it resolves to.
        let real = dir.path().canonicalize().unwrap().join("real");
        std::fs::create_dir_all(real.join("src")).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let resolved = task(real.to_str().unwrap());
        assert_eq!(holding(link.join("src").to_str().unwrap()), Some(resolved));
    }

    #[test]
    fn upgrade_gives_graph_runs_that_named_no_task_theirs() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_facts_once",
        );
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_1','/repo',1);
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES
                ('task_1','repo_1','FIX-1',1),('task_2','repo_1','FIX-2',1);
             INSERT INTO task_checkouts(task_id,worktree) VALUES('task_1','/repo/1');
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id)
             VALUES('caller','Caller','human',1,0,'/elsewhere','task_2');
             INSERT INTO task_primary_sessions(task_id,session_id) VALUES('task_2','caller');
             INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id,started_at,cwd,caller_session_id) VALUES
                ('inside','code','{}',NULL,1,'/repo/1/src','caller'),
                ('called','code','{}',NULL,1,'/tmp','caller'),
                ('named','code','{}','task_2',1,'/repo/1',NULL),
                ('nowhere','code','{}',NULL,1,'/tmp',NULL);",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("task_facts_once"))
            .unwrap();
        let runs: Vec<(String, Option<String>)> = conn
            .prepare("SELECT lf_process_id,task_id FROM graph_processes ORDER BY lf_process_id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            runs,
            [
                ("called".into(), Some("task_2".into())),
                ("inside".into(), Some("task_1".into())),
                ("named".into(), Some("task_2".into())),
                ("nowhere".into(), None),
            ]
        );
        // A run's Task is fixed again once the upgrade has recorded it.
        assert!(conn
            .execute(
                "UPDATE graph_processes SET task_id='task_1' WHERE lf_process_id='nowhere'",
                [],
            )
            .is_err());
    }

    #[test]
    fn upgrade_records_the_task_each_session_belonged_to_by_path() {
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
             VALUES('bound','Bound','human',1,0,'/repo/999','task_00000000000000000000000000000001');
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,work_source)
             VALUES('inherited','Inherited','human',1,0,'/tmp','task_00000000000000000000000000000002','inherited');
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES
                ('helper','Helper','human',1,0,'/repo/99/sub'),
                ('nested','Nested','human',1,0,'/repo/1/x'),
                ('outside','Outside','human',1,0,'/tmp'),
                ('primary','Primary','human',1,0,'/elsewhere');
             UPDATE tasks SET primary_session_id='primary' WHERE worktree='/repo/5';",
        )
        .unwrap();
        // The upgrade moves the released checkouts into `task_checkouts` and the
        // primary Session's mark onto its row, then records each
        // Session's Task.
        apply_pending_migrations(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
        );
        let sessions: Vec<(String, Option<String>, Option<String>)> = conn
            .prepare("SELECT id,task_id,work_source FROM agent_sessions ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let task = |i: u32| Some(format!("task_{i:032x}"));
        let checkout = Some("checkout".to_owned());
        assert_eq!(
            sessions,
            [
                ("bound".into(), task(1), None),
                ("helper".into(), task(99), checkout.clone()),
                // A Task declared through the starting Session is declared.
                ("inherited".into(), task(2), Some("declared".into())),
                ("nested".into(), task(1), checkout),
                ("outside".into(), None, None),
                ("primary".into(), task(5), None),
            ]
        );
        assert_eq!(
            conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
    }

    #[test]
    fn an_auto_off_stands_until_auto_on() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let store = crate::test_fixture::open(&path).unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: TaskId::new(),
                repository_id: crate::test_fixture::repository(&store, "/auto"),
                project_id: None,
                title: "Auto".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let mark = |state, at| super::Auto {
            state,
            at,
            by: crate::process::Starter::Person {
                name: Some("Casey".into()),
            },
        };
        let on = mark(super::AutoState::On, 10);
        let stopped = mark(super::AutoState::Off, 20);
        // The Task holds its mark before it takes up a Graph.
        assert_eq!(store.set_task_auto(&task.id, &on, false).unwrap(), None);
        assert_eq!(
            store.set_task_auto(&task.id, &stopped, false).unwrap(),
            Some(on)
        );
        // A configured default keeps a stop; taking up a graph, then another, keeps it too.
        assert_eq!(
            store
                .set_task_auto(&task.id, &mark(super::AutoState::On, 30), true)
                .unwrap(),
            Some(stopped.clone())
        );
        for graph in ["code", "feature"] {
            store
                .take_up_task_graph(&task.id, &format!(r#"{{"name":"{graph}","steps":[]}}"#))
                .unwrap();
        }
        drop(store);
        let store = crate::test_fixture::open(&path).unwrap();
        assert_eq!(store.task_auto(&task.id).unwrap(), Some(stopped));
        // `lf task auto on` lifts it.
        store
            .set_task_auto(&task.id, &mark(super::AutoState::On, 40), false)
            .unwrap();
        assert_eq!(
            store.task_auto(&task.id).unwrap(),
            Some(mark(super::AutoState::On, 40))
        );
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
            conn.execute_batch("DROP TRIGGER refuse_request").unwrap();
            let tx = conn.transaction().unwrap();
            assert!(super::arrive_in(&tx, &task, &carrier, super::END).unwrap());
            tx.commit().unwrap();
        }
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
                .record_graph(id, &graph, *owner, &start, None, Some(cwd), None)
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
    fn a_moved_task_keeps_its_auto_mark_where_it_arrives() {
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
        let on = super::Auto {
            state: super::AutoState::On,
            at: 7,
            by: crate::process::Starter::Person {
                name: Some("Casey".into()),
            },
        };
        laptop.set_task_auto(&id, &on, false).unwrap();
        // It has taken up no Graph. Delivered twice, as a retried move is.
        let mark = laptop.task_auto(&id).unwrap();
        mini.place_moved_task_auto(&id, mark.as_ref()).unwrap();
        mini.place_moved_task_auto(&id, mark.as_ref()).unwrap();
        assert_eq!(mini.task_auto(&id).unwrap(), Some(on));
        // Reopened where it went, it comes back with none.
        laptop.place_moved_task_auto(&id, None).unwrap();
        assert_eq!(laptop.task_auto(&id).unwrap(), None);
    }

    #[test]
    fn a_released_tasks_ship_log_becomes_its_auto_mark() {
        use crate::migration_runner::{apply_before_migration, upgrade_from_migration};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_auto",
        );
        let (shipping, stopped, withdrawn, never) =
            (TaskId::new(), TaskId::new(), TaskId::new(), TaskId::new());
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_1','/repo',1);",
        )
        .unwrap();
        for (task, issue) in [
            (&shipping, "SHIP-1"),
            (&stopped, "SHIP-2"),
            (&withdrawn, "SHIP-3"),
            (&never, "SHIP-4"),
        ] {
            conn.execute_batch(&format!(
                "INSERT INTO tasks(id,repository_id,issue_identifier,created_at,shipped_at)
                 VALUES('{task}','repo_1','{issue}',1,10);"
            ))
            .unwrap();
        }
        // The stopped Task never ran: it has no Graph, and its stop still arrives.
        for task in [&shipping, &withdrawn, &never] {
            conn.execute(
                "INSERT INTO task_graphs(task_id,graph,step,updated_at)
                 VALUES(?1,'{\"name\":\"code\",\"steps\":[]}','start',1)",
                [task.as_str()],
            )
            .unwrap();
        }
        conn.execute_batch(&format!(
            "INSERT INTO task_ship_events(task_id,kind,at,person) VALUES
                 ('{stopped}','approved',10,NULL),('{stopped}','stopped',20,'Casey'),
                 ('{shipping}','stopped',5,'Casey'),('{shipping}','approved',30,NULL),
                 ('{withdrawn}','approved',10,NULL),('{withdrawn}','withdrawn',40,NULL);"
        ))
        .unwrap();
        upgrade_from_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_auto",
        );
        let mark = |task| super::auto_in(&conn, task).unwrap();
        let person = |name: Option<&str>| crate::process::Starter::Person {
            name: name.map(str::to_owned),
        };
        assert_eq!(
            mark(&shipping),
            Some(super::Auto {
                state: super::AutoState::On,
                at: 30,
                by: person(None),
            })
        );
        assert_eq!(
            mark(&stopped),
            Some(super::Auto {
                state: super::AutoState::Off,
                at: 20,
                by: person(Some("Casey")),
            })
        );
        assert_eq!(mark(&withdrawn).unwrap().state, super::AutoState::Off);
        assert_eq!(mark(&never), None);
        // A released lf reads no Task as shipping.
        let shipped: i64 = conn
            .query_row(
                "SELECT count(*) FROM tasks WHERE shipped_at IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(shipped, 0);
    }
}
