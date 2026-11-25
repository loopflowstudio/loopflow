//! GraphExecution: what a Graph run and its steps recorded of themselves. What each
//! launched with is immutable; only its end moves. Reading selects nothing and
//! grants neither attachment nor process authority.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;

use rusqlite::{params, Connection, OptionalExtension};

use crate::execution::{GraphProcessFilter, GraphProcessInventoryEntry, GraphProcessPage};
use crate::graph::CompiledGraph;
use crate::graph_process::{
    Execution, ExecutionResult, ExecutionStart, GraphExecution, GraphStep, RunOwner,
};
use crate::session::{GraphProcessSummary, GraphProcessSummaryState};
use crate::{StoreError, StoreResult};
use loopflow_model::durable::TaskId;
use loopflow_model::id::{LfProcessId, WaveId};

use super::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// The columns every `Execution` owner shares, prefixed by `alias`.
pub(super) fn run_columns(alias: &str) -> String {
    [
        "lf_process_id",
        "started_at",
        "ended_at",
        "pid",
        "os_started_at",
        "exit_code",
        "error",
    ]
    .map(|column| format!("{alias}.{column}"))
    .join(",")
}

/// A run read from the leading columns `run_columns` selects.
pub(super) fn read_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<Execution> {
    Ok(Execution {
        id: row.get(0)?,
        // A run a released lf recorded without its Process row has no start.
        started_at: row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
        ended_at: row.get(2)?,
        pid: row.get(3)?,
        os_started_at: row.get(4)?,
        exit_code: row.get(5)?,
        error: row.get(6)?,
    })
}

/// Graphs whose Graph run `g` matches `scope`, oldest first, each with the
/// steps it recorded in launch order.
pub(super) fn graphs_in(
    conn: &Connection,
    scope: &str,
    values: &[&dyn rusqlite::ToSql],
) -> StoreResult<Vec<GraphExecution>> {
    let graph_processes = conn
        .prepare(&format!(
            "SELECT {},g.repo,g.cwd,g.graph,g.compiled,g.caller_session_id,g.task_id FROM graph_processes g
             WHERE ({scope}) ORDER BY g.rowid",
            run_columns("g")
        ))?
        .query_map(values, |row| {
            Ok((
                read_run(row)?,
                row.get::<_, Option<String>>("repo")?,
                row.get::<_, Option<String>>("cwd")?,
                row.get::<_, String>("graph")?,
                row.get::<_, String>("compiled")?,
                row.get::<_, Option<loopflow_model::id::LfSessionId>>("caller_session_id")?,
                row.get::<_, Option<String>>("task_id")?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // Every Graph's steps in one statement, so a reading of many Graphs costs two.
    let ids = super::children::ids_json(graph_processes.iter().map(|(run, ..)| run.id.as_str()));
    let mut steps = HashMap::<String, Vec<GraphStep>>::new();
    for row in conn
        .prepare(&format!(
            "SELECT {},gs.graph_lf_process_id,gs.node,gs.iterations FROM graph_process_steps gs
             WHERE gs.graph_lf_process_id IN (SELECT value FROM json_each(?1)) ORDER BY gs.seq",
            run_columns("gs")
        ))?
        .query_map([&ids], |row| {
            Ok((
                read_run(row)?,
                row.get::<_, String>("graph_lf_process_id")?,
                row.get::<_, u32>("node")?,
                row.get::<_, String>("iterations")?,
            ))
        })?
    {
        let (run, graph, key, iterations) = row?;
        steps.entry(graph).or_default().push(GraphStep {
            run,
            key,
            iterations: serde_json::from_str(&iterations)?,
        });
    }
    graph_processes
        .into_iter()
        .map(|(run, repo, cwd, name, compiled, caller_session, task)| {
            Ok(GraphExecution {
                task_id: task
                    .map(|id| TaskId::parse(&id))
                    .transpose()
                    .map_err(invalid)?,
                compiled: serde_json::from_str::<CompiledGraph>(&compiled)?,
                steps: steps.remove(run.id.as_str()).unwrap_or_default(),
                run,
                repo,
                caller_session,
                cwd,
                name,
            })
        })
        .collect()
}

/// What the Graph process says of the Graph, including the Task it started for.
pub(super) fn entry_in(
    conn: &Connection,
    graph: &GraphExecution,
) -> StoreResult<GraphProcessInventoryEntry> {
    let wave = match &graph.task_id {
        Some(task) => super::durable::task_wave_in(conn, task)?,
        None => conn
            .query_row(
                "SELECT c.wave_id FROM graph_process_steps gs
                 JOIN session_events c ON c.lf_process_id=gs.lf_process_id
                 WHERE gs.graph_lf_process_id=?1 AND c.kind='captured' AND c.wave_id IS NOT NULL
                 ORDER BY c.seq DESC LIMIT 1",
                [graph.id()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|id| WaveId::parse(&id))
            .transpose()
            .map_err(invalid)?,
    };
    Ok(entry_of(graph, wave))
}

/// The Graph's inventory entry in its Wave.
pub fn entry_of(graph: &GraphExecution, wave_id: Option<WaveId>) -> GraphProcessInventoryEntry {
    let run = &graph.run;
    GraphProcessInventoryEntry {
        summary: GraphProcessSummary {
            id: run.id.to_string(),
            name: graph.name.clone(),
            state: GraphProcessSummaryState::of_run(run.result()),
            task_id: graph.task_id.clone(),
            wave_id,
            updated_at: run
                .ended_at
                .or(graph.latest().map(|step| step.run.started_at))
                .unwrap_or(run.started_at),
        },
        repo: graph.repo.clone(),
        ended_at: run.ended_at,
    }
}

/// Facts read together for Graph execution: each step's latest captured input,
/// unanswered question and limit recovery, plus the selected Tasks' Graph state.
#[derive(Debug, Default)]
pub struct GraphExecutionFacts {
    pub open_graphs: HashMap<TaskId, Vec<Execution>>,
    pub unrun: HashMap<TaskId, UnrunGraph>,
    pub tasks_with_carrier_records: HashSet<TaskId>,
    pub inputs: HashMap<LfProcessId, (String, String, i64)>,
    pub unanswered_questions: HashMap<LfProcessId, String>,
    pub limit_recoveries: HashMap<LfProcessId, AgentLimitRecovery>,
}

/// The Graph a Task took up and never ran: it stands at `start`, no run
/// carries it and no GraphProcess was started for it. A Task waiting at an
/// interactive step has run. A conversation opening on such a Task
/// owes its setup, and the Task reads as starting that Graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrunGraph {
    pub graph: String,
    /// The error of the Task's latest event when it is a resumable failure:
    /// the last attempt to start stopped there.
    pub stopped: Option<String>,
}

fn unrun_graphs_in(conn: &Connection, ids: &str) -> StoreResult<HashMap<TaskId, UnrunGraph>> {
    let rows = conn
        .prepare(
            "SELECT t.task_id,json_extract(t.graph,'$.name'),
                (SELECT e.kind_json FROM task_events e WHERE e.task_id=t.task_id ORDER BY e.id DESC LIMIT 1)
             FROM task_graphs t
             WHERE t.task_id IN (SELECT value FROM json_each(?1)) AND t.step='start'
               AND t.run_start IS NULL
               AND NOT EXISTS(SELECT 1 FROM graph_processes g WHERE g.task_id=t.task_id)",
        )?
        .query_map([ids], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(task, graph, event)| {
            let stopped = match event.as_deref().map(serde_json::from_str) {
                Some(Ok(crate::work::task::TaskEventKind::Failed {
                    error,
                    resumable: true,
                })) => Some(error),
                Some(Err(error)) => return Err(invalid(error)),
                _ => None,
            };
            Ok((
                TaskId::parse(&task).map_err(invalid)?,
                UnrunGraph { graph, stopped },
            ))
        })
        .collect()
}

/// How a headless step is getting past a usage limit. `retry_at` set means
/// nothing can launch before then; none means another account took the step on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLimitRecovery {
    pub provider: String,
    pub reason: String,
    pub retry_at: Option<i64>,
    /// The step's explicit agent choice, if it had one.
    pub agent: Option<String>,
}

/// How each of `steps` is recovering from a usage limit.
fn limit_recoveries(
    conn: &Connection,
    steps: &[&LfProcessId],
) -> StoreResult<HashMap<LfProcessId, AgentLimitRecovery>> {
    let ids = super::children::ids_json(steps.iter().map(|id| id.as_str()));
    let rows = conn
        .prepare(
            "SELECT lf_process_id,provider,reason,retry_at,agent FROM agent_limit_recoveries
             WHERE lf_process_id IN (SELECT value FROM json_each(?1))",
        )?
        .query_map([&ids], |row| {
            Ok((
                row.get(0)?,
                AgentLimitRecovery {
                    provider: row.get(1)?,
                    reason: row.get(2)?,
                    retry_at: row.get(3)?,
                    agent: row.get(4)?,
                },
            ))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(rows)
}

/// The conversation, captured input and its event each of `processes` opened
/// last.
fn process_inputs(
    conn: &Connection,
    processes: &[&LfProcessId],
) -> StoreResult<HashMap<LfProcessId, (String, String, i64)>> {
    let ids = super::children::ids_json(processes.iter().map(|id| id.as_str()));
    let rows = conn
        .prepare(
            "SELECT lf_process_id,session_id,receipt_key,MAX(seq) FROM session_events
             WHERE lf_process_id IN (SELECT value FROM json_each(?1)) AND kind='captured'
             GROUP BY lf_process_id",
        )?
        .query_map([&ids], |row| {
            Ok((row.get(0)?, (row.get(1)?, row.get(2)?, row.get(3)?)))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

impl SqliteStore {
    /// The Graph run that recorded `step` as one of its steps.
    pub fn graph_of_step(&self, step: &LfProcessId) -> StoreResult<Option<LfProcessId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT graph_lf_process_id FROM graph_process_steps WHERE lf_process_id=?1",
                [step],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn agent_limit_recovery(
        &self,
        step: &LfProcessId,
    ) -> StoreResult<Option<AgentLimitRecovery>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(limit_recoveries(&conn, &[step])?.remove(step))
    }

    /// Say how step Process `id` is recovering from a usage limit.
    pub fn record_agent_limit_recovery(
        &self,
        id: &LfProcessId,
        provider: &str,
        reason: &str,
        retry_at: Option<i64>,
        agent: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            // Only a Graph step waits out a limit; any other lf has nothing to note.
            "INSERT INTO agent_limit_recoveries(lf_process_id,provider,reason,retry_at,agent,updated_at)
             SELECT ?1,?2,?3,?4,?5,?6
             WHERE EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=?1)
             ON CONFLICT(lf_process_id) DO UPDATE SET provider=excluded.provider,
                reason=excluded.reason,retry_at=excluded.retry_at,agent=excluded.agent,
                updated_at=excluded.updated_at",
            params![
                id,
                provider,
                reason,
                retry_at,
                agent,
                crate::rows::now_unix()
            ],
        )?;
        Ok(())
    }

    /// `provider`'s last refusal here, its reason and when, if no agent of
    /// that provider has launched since.
    pub fn unanswered_refusal(&self, provider: &str) -> StoreResult<Option<(String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT r.reason,r.updated_at FROM agent_limit_recoveries r
                 WHERE r.provider=?1 AND NOT EXISTS(
                     SELECT 1 FROM agent_sessions s
                     WHERE s.provider=?1 AND s.agent_started_at>r.updated_at)
                 ORDER BY r.updated_at DESC LIMIT 1",
                [provider],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    /// `step`'s wait is over: its launch is under way, or someone interrupted it.
    pub fn end_agent_limit_wait(&self, step: &LfProcessId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_limit_recoveries SET retry_at=NULL,updated_at=?2
             WHERE retry_at IS NOT NULL AND lf_process_id=?1",
            params![step, crate::rows::now_unix()],
        )?;
        Ok(())
    }

    /// The Tasks of every wait whose time has come. A run restarted at its step
    /// leaves the old wait behind, so a Task here may hold nothing any more.
    pub fn tasks_past_a_limit_wait(&self, now: i64) -> StoreResult<HashSet<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let waits = conn
            .prepare("SELECT lf_process_id FROM agent_limit_recoveries WHERE retry_at<=?1")?
            .query_map([now], |row| row.get(0))?
            .collect::<Result<Vec<LfProcessId>, _>>()?;
        let mut tasks = HashSet::new();
        for step in waits {
            let task = conn
                .query_row(
                    "SELECT g.task_id FROM graph_process_steps gs
                     JOIN graph_processes g ON g.lf_process_id=gs.graph_lf_process_id
                     WHERE gs.lf_process_id=?1 AND g.task_id IS NOT NULL",
                    [&step],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(task) = task {
                tasks.insert(TaskId::parse(&task).map_err(invalid)?);
            }
        }
        Ok(tasks)
    }

    /// Graphs in Graph-process-id order after `after`, at most `limit`.
    /// `observe` reads each run's result as the caller sees it: a run whose
    /// pid has died reads as ended before the liveness pass records it.
    pub fn graph_inventory(
        &self,
        filter: &GraphProcessFilter,
        after: Option<&str>,
        limit: NonZeroU32,
        observe: impl Fn(&Execution) -> crate::graph_process::ExecutionResult,
    ) -> StoreResult<GraphProcessPage> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let graphs = match &filter.task_id {
            Some(task) => super::task_work::graphs_of_task(&conn, task)?,
            None => graphs_in(&conn, "1", &[])?,
        };
        let search = filter.search.as_deref().map(str::to_lowercase);
        let mut entries = Vec::new();
        for graph in &graphs {
            let mut entry = entry_in(&conn, graph)?;
            if entry.summary.state == GraphProcessSummaryState::Current {
                entry.summary.state = GraphProcessSummaryState::of_run(observe(&graph.run));
            }
            let summary = &entry.summary;
            if filter
                .repo
                .as_ref()
                .is_some_and(|repo| entry.repo.as_ref() != Some(repo))
                || filter
                    .wave_id
                    .as_ref()
                    .is_some_and(|wave| summary.wave_id.as_ref() != Some(wave))
                || (filter.taskless && summary.task_id.is_some())
                || filter.state.is_some_and(|state| summary.state != state)
                || search.as_ref().is_some_and(|search| {
                    !summary.name.to_lowercase().contains(search) && !summary.id.contains(search)
                })
                || after.is_some_and(|after| summary.id.as_str() <= after)
            {
                continue;
            }
            entries.push(entry);
        }
        entries.sort_by(|left, right| left.summary.id.cmp(&right.summary.id));
        let limit = limit.get() as usize;
        let next = (entries.len() > limit).then(|| entries[limit - 1].summary.id.clone());
        entries.truncate(limit);
        Ok(GraphProcessPage { entries, next })
    }

    /// One Graph by its Graph process id or a unique prefix of it.
    pub fn graph_process(
        &self,
        selector: &str,
    ) -> StoreResult<Option<(GraphExecution, GraphProcessInventoryEntry)>> {
        let canonical = selector.to_ascii_lowercase();
        let conn = self.conn.lock().expect("store mutex poisoned");
        // Exact identity wins; otherwise a unique literal prefix.
        let mut graphs = graphs_in(&conn, "g.lf_process_id=?1", &[&canonical])?;
        if graphs.is_empty() {
            let pattern = format!("{}%", canonical.replace(['%', '_'], ""));
            graphs = graphs_in(&conn, "g.lf_process_id LIKE ?1", &[&pattern])?;
        }
        if graphs.len() != 1 {
            return Ok(None);
        }
        let graph = graphs.pop().expect("one Graph matched");
        let entry = entry_in(&conn, &graph)?;
        Ok(Some((graph, entry)))
    }

    /// Graphs run from `cwd`, oldest first.
    pub fn graphs_at(&self, cwd: &std::path::Path) -> StoreResult<Vec<GraphExecution>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        graphs_in(&conn, "g.cwd=?1", &[&cwd.to_string_lossy()])
    }

    /// A Graph run's record of itself as it starts: the graph it launched
    /// with, the Task it belongs to ([`super::task_work::starting_task_in`]),
    /// the OS process it runs as and the held terminal it runs in.
    #[allow(clippy::too_many_arguments)]
    pub fn record_graph(
        &self,
        graph_process: &LfProcessId,
        compiled: &CompiledGraph,
        task: Option<&TaskId>,
        start: &ExecutionStart,
        repo: Option<&str>,
        cwd: Option<&str>,
        held_terminal: Option<&loopflow_model::id::TerminalId>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let task = super::task_work::starting_task_in(
            &conn,
            task,
            cwd.map(std::path::Path::new),
            start.caller_session.as_ref(),
        )?;
        conn.execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id,started_at,pid,os_started_at,repo,cwd,held_terminal_id,caller_session_id)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                graph_process,
                compiled.name,
                serde_json::to_string(compiled)?,
                task.as_ref().map(TaskId::as_str),
                start.started_at,
                start.pid,
                start.os_started_at,
                repo,
                cwd,
                held_terminal,
                start.caller_session.as_ref(),
            ],
        )?;
        Ok(())
    }

    /// A Graph run's record of one step it spawned, under the id it minted
    /// for the step.
    pub fn record_graph_step(
        &self,
        graph_process: &LfProcessId,
        step: &LfProcessId,
        key: u32,
        iterations: &[Vec<u32>],
        start: &ExecutionStart,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations,started_at,pid,os_started_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                graph_process,
                step,
                key,
                serde_json::to_string(iterations)?,
                start.started_at,
                start.pid,
                start.os_started_at,
            ],
        )?;
        Ok(())
    }

    /// The end of the run `id` on the record `owner`, unless it already
    /// recorded one: the run writes its own at exit, and its spawner writes
    /// what it saw if the run died first. Clears the OS process it ran as.
    pub fn finish_run(
        &self,
        owner: RunOwner,
        id: &LfProcessId,
        ended_at: i64,
        exit_code: Option<i32>,
        error: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        finish_run_in(&conn, owner, id.as_str(), ended_at, exit_code, error)
    }

    /// The OS process a reserved step now runs as, unless it already ended.
    pub fn record_graph_step_pid(
        &self,
        step: &LfProcessId,
        pid: u32,
        os_started_at: Option<i64>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE graph_process_steps SET pid=?2,os_started_at=?3
             WHERE lf_process_id=?1 AND ended_at IS NULL",
            params![step, pid, os_started_at],
        )?;
        Ok(())
    }

    /// Whether the Graph run `id` has recorded itself.
    pub fn graph_run_recorded(&self, id: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=?1)",
            [id],
            |row| row.get(0),
        )?)
    }

    /// When the Graph run `id` failed, the time it ended and why: the error
    /// of its last failed step, else its own.
    pub fn graph_failure_cause(&self, id: &LfProcessId) -> StoreResult<Option<(i64, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(run) = graphs_in(&conn, "g.lf_process_id=?1", &[id])?.pop() else {
            return Ok(None);
        };
        if run.run.result() != ExecutionResult::Failed {
            return Ok(None);
        }
        let at = run.run.ended_at.expect("a failed run has ended");
        let step = run
            .steps
            .iter()
            .rev()
            .find(|step| step.run.result() == ExecutionResult::Failed)
            .and_then(|step| step.run.error.clone());
        Ok(Some((
            at,
            step.or(run.run.error).unwrap_or_else(|| "it failed".into()),
        )))
    }

    /// What the agent of the Graph run `id`'s last failed step asked with
    /// nobody there to answer, if it failed on that.
    pub fn graph_unanswered_question(&self, id: &LfProcessId) -> StoreResult<Option<String>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let Some(run) = graphs_in(&conn, "g.lf_process_id=?1", &[id])?.pop() else {
            return Ok(None);
        };
        let Some(step) = run
            .steps
            .iter()
            .rev()
            .find(|step| step.run.result() == ExecutionResult::Failed)
        else {
            return Ok(None);
        };
        Ok(super::turns::unanswered_questions_in(&conn, &[&step.run.id])?.remove(&step.run.id))
    }

    /// Whether the Graph run `id` failed at a skill step that opened no
    /// conversation: lf refused the launch before any agent started, and
    /// would refuse the same launch again.
    pub fn graph_failed_before_its_agent(&self, id: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(run) = graphs_in(&conn, "g.lf_process_id=?1", &[id])?.pop() else {
            return Ok(false);
        };
        let Some(step) = run
            .steps
            .last()
            .filter(|step| step.run.result() == ExecutionResult::Failed)
        else {
            return Ok(false);
        };
        let skill = run
            .compiled
            .node_at(step.key)
            .is_some_and(|node| node.kind == crate::graph::GraphNodeKind::Skill);
        Ok(skill && process_inputs(&conn, &[&step.run.id])?.is_empty())
    }

    /// The conversation, captured input and its event a process opened last.
    pub fn process_input(
        &self,
        process: &LfProcessId,
    ) -> StoreResult<Option<(String, String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(process_inputs(&conn, &[process])?.remove(process))
    }

    /// One recorded Graph step and its Graph run, resolved against the Graph
    /// definition that run captured.
    pub fn graph_step_detail(
        &self,
        step: &LfProcessId,
    ) -> StoreResult<Option<(LfProcessId, crate::execution::GraphStepProcess)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row = conn
            .query_row(
                &format!(
                    "SELECT {},gs.graph_lf_process_id,gs.node,gs.iterations,g.compiled
                     FROM graph_process_steps gs
                     JOIN graph_processes g ON g.lf_process_id=gs.graph_lf_process_id
                     WHERE gs.lf_process_id=?1",
                    run_columns("gs")
                ),
                [step],
                |row| {
                    Ok((
                        read_run(row)?,
                        row.get::<_, LfProcessId>(7)?,
                        row.get::<_, u32>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()?;
        let Some((run, graph_id, key, iterations, compiled)) = row else {
            return Ok(None);
        };
        let compiled: CompiledGraph = serde_json::from_str(&compiled)?;
        let label = compiled
            .node_at(key)
            .map(|node| node.label.clone())
            .unwrap_or_default();
        Ok(Some((
            graph_id,
            crate::execution::GraphStepProcess {
                lf_process_id: run.id.clone(),
                label,
                key,
                iterations: serde_json::from_str(&iterations)?,
                started_at: run.started_at,
                completed_at: run.ended_at,
                outcome: run.result().outcome().map(str::to_owned),
                exit_code: run.exit_code,
                error: run.error,
            },
        )))
    }

    /// Execution facts of `graphs`: their latest steps' captured inputs, and
    /// unended Graph runs in `tasks`' work.
    pub fn graph_execution_facts<'a>(
        &self,
        graphs: impl IntoIterator<Item = &'a GraphExecution>,
        tasks: &[&TaskId],
    ) -> StoreResult<GraphExecutionFacts> {
        let graphs: Vec<_> = graphs.into_iter().collect();
        let steps: Vec<_> = graphs
            .iter()
            .filter_map(|graph| graph.latest().map(|step| &step.run.id))
            .collect();
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let ids = super::children::ids_json(tasks.iter().map(|task| task.as_str()));
        let mut open_graphs = HashMap::<TaskId, Vec<Execution>>::new();
        let mut query = conn.prepare(&format!(
            "SELECT {},pairs.task_id FROM ({}) pairs
             JOIN graph_processes g ON g.lf_process_id=pairs.graph
             ORDER BY g.started_at DESC,g.lf_process_id",
            run_columns("g"),
            super::task_work::task_open_graph_pairs("SELECT value FROM json_each(?1)")
        ))?;
        for row in query.query_map([&ids], |row| Ok((row.get::<_, String>(7)?, read_run(row)?)))? {
            let (task, run) = row?;
            let task = TaskId::parse(&task).map_err(invalid)?;
            open_graphs.entry(task).or_default().push(run);
        }
        let inputs = process_inputs(&conn, &steps)?;
        let unanswered_questions = super::turns::unanswered_questions_in(&conn, &steps)?;
        let limit_recoveries = limit_recoveries(&conn, &steps)?;
        let unrun = unrun_graphs_in(&conn, &ids)?;
        let mut carriers = conn.prepare(
            "SELECT task_id FROM task_graphs WHERE task_id IN (SELECT value FROM json_each(?1))
             AND (run_pid IS NOT NULL OR run_died_at IS NOT NULL)",
        )?;
        let tasks_with_carrier_records = carriers
            .query_map([&ids], |row| row.get::<_, String>(0))?
            .map(|id| TaskId::parse(&id?).map_err(invalid))
            .collect::<StoreResult<_>>()?;
        Ok(GraphExecutionFacts {
            open_graphs,
            unrun,
            tasks_with_carrier_records,
            inputs,
            unanswered_questions,
            limit_recoveries,
        })
    }

    /// The Graph `task` took up, when it never ran it.
    pub fn unrun_task_graph(&self, task: &TaskId) -> StoreResult<Option<UnrunGraph>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let ids = super::children::ids_json([task.as_str()]);
        Ok(unrun_graphs_in(&conn, &ids)?.remove(task))
    }
}

#[cfg(any(test, feature = "test-support"))]
impl SqliteStore {
    /// Record a Graph process and one step Process per `(label, outcome)`, as a Graph
    /// run from `cwd` leaves them.
    pub fn test_graph(
        &self,
        name: &str,
        cwd: &str,
        steps: &[(&str, Option<&str>)],
        graph_outcome: Option<&str>,
    ) -> LfProcessId {
        let graph_process = LfProcessId::new();
        let now = crate::rows::now_unix();
        let start = ExecutionStart::reserved(now);
        // The journal still records each run as a Process beside its Graph record.
        let process = |id: &LfProcessId, parent: Option<&LfProcessId>, outcome: Option<&str>| {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,repo,cwd,started_at,completed_at,outcome)
                 VALUES(?1,?1,?2,?3,?3,?4,?5,?6)",
                params![id, parent, cwd, now, outcome.map(|_| now), outcome],
            )
            .unwrap();
        };
        // The exit each outcome stands for, as the run would record it.
        let finish = |owner: RunOwner, id: &LfProcessId, outcome: Option<&str>| {
            let Some(outcome) = outcome else { return };
            let (exit, error) = match outcome {
                "succeeded" => (Some(0), None),
                "interrupted" => (Some(130), None),
                "unknown" => (None, None),
                _ => (Some(1), Some("it failed")),
            };
            self.finish_run(owner, id, now, exit, error).unwrap();
        };
        let compiled = crate::graph::CompiledGraph {
            name: name.into(),
            steps: steps
                .iter()
                .enumerate()
                .map(|(index, (label, _))| crate::graph::GraphNode {
                    key: index as u32,
                    id: None,
                    label: (*label).into(),
                    kind: crate::graph::GraphNodeKind::Skill,
                    interactive: false,
                    returns_to: None,
                    sources: Vec::new(),
                    paths: Vec::new(),
                })
                .collect(),
        };
        process(&graph_process, None, graph_outcome);
        self.record_graph(
            &graph_process,
            &compiled,
            None,
            &start,
            Some(cwd),
            Some(cwd),
            None,
        )
        .unwrap();
        for (index, (_, outcome)) in steps.iter().enumerate() {
            let step = LfProcessId::new();
            process(&step, Some(&graph_process), *outcome);
            self.record_graph_step(&graph_process, &step, index as u32, &[vec![]], &start)
                .unwrap();
            finish(RunOwner::GraphStep, &step, *outcome);
        }
        finish(RunOwner::GraphRun, &graph_process, graph_outcome);
        graph_process
    }
}

/// Write a run's end on the one record `owner` names.
pub(super) fn finish_run_in(
    conn: &rusqlite::Connection,
    owner: RunOwner,
    id: &str,
    ended_at: i64,
    exit_code: Option<i32>,
    error: Option<&str>,
) -> StoreResult<()> {
    let sql = match owner {
        // A carrier's result is its Graph run's: its end clears the OS
        // process and keeps its id, where the next run sets out from.
        RunOwner::TaskCarrier => {
            conn.execute(
                "UPDATE task_graphs SET run_pid=NULL,run_os_started_at=NULL
                 WHERE lf_process_id=?1 AND run_pid IS NOT NULL",
                [id],
            )?;
            return Ok(());
        }
        RunOwner::GraphRun => {
            "UPDATE graph_processes SET ended_at=?2,exit_code=?3,error=?4,pid=NULL,os_started_at=NULL
             WHERE lf_process_id=?1 AND ended_at IS NULL"
        }
        RunOwner::GraphStep => {
            "UPDATE graph_process_steps SET ended_at=?2,exit_code=?3,error=?4,pid=NULL,os_started_at=NULL
             WHERE lf_process_id=?1 AND ended_at IS NULL"
        }
    };
    conn.execute(sql, params![id, ended_at, exit_code, error])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use crate::execution::GraphProcessFilter;
    use crate::graph_process::{Execution, RunOwner};
    use crate::session::GraphProcessSummaryState;

    /// Two waits on one Task's step and a later wait still running its time.
    #[test]
    fn a_task_past_its_limit_wait_is_found_once_and_only_when_due() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let repository = crate::test_fixture::repository(&store, "/repo");
        let task = loopflow_model::durable::TaskId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(&format!(
                "INSERT INTO tasks(repository_id,id,external_issue_id,issue_identifier,created_at)
                 VALUES('{repository}','{task}','issue','PROOF-1',1);
                 INSERT INTO task_checkouts(task_id,worktree) VALUES('{task}','/repo/task');",
                repository = repository.as_str(),
                task = task.as_str(),
            ))
            .unwrap();
        let graph = store.test_graph(
            "code",
            "/repo/task",
            &[("implement", None), ("gate", None)],
            None,
        );
        let steps = store
            .graph_process(graph.as_str())
            .unwrap()
            .unwrap()
            .0
            .steps;
        let wait = |step: usize, retry_at| {
            store
                .record_agent_limit_recovery(
                    &steps[step].run.id,
                    "claude",
                    "limit",
                    Some(retry_at),
                    None,
                )
                .unwrap();
        };
        // A bare lf that hit a limit is no step: nothing to note, no error.
        store
            .record_agent_limit_recovery(
                &loopflow_model::id::LfProcessId::default(),
                "claude",
                "limit",
                Some(1),
                None,
            )
            .unwrap();
        wait(0, 200);
        assert!(store.tasks_past_a_limit_wait(100).unwrap().is_empty());
        wait(1, 100);
        assert_eq!(store.tasks_past_a_limit_wait(200).unwrap().len(), 1);
        assert!(store.tasks_past_a_limit_wait(200).unwrap().contains(&task));
    }

    /// A refusal stands until an agent of its provider launches again.
    #[test]
    fn a_refusal_stands_until_its_provider_launches_again() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let graph = store.test_graph("code", "/repo", &[("implement", None)], None);
        let step = store
            .graph_process(graph.as_str())
            .unwrap()
            .unwrap()
            .0
            .steps[0]
            .run
            .id
            .clone();
        store
            .record_agent_limit_recovery(&step, "codex", "no login", None, None)
            .unwrap();
        let (reason, refused_at) = store.unanswered_refusal("codex").unwrap().unwrap();
        assert_eq!(reason, "no login");
        assert_eq!(store.unanswered_refusal("claude").unwrap(), None);
        // Another provider's launch answers nothing; Codex's own does.
        let launch = |id: &str, provider: &str| {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,
                        provider,agent_started_at)
                     VALUES(?1,'Agent','human',1,0,'/repo',?2,?3)",
                    rusqlite::params![id, provider, refused_at + 1],
                )
                .unwrap();
        };
        launch("claude-agent", "claude");
        assert!(store.unanswered_refusal("codex").unwrap().is_some());
        launch("codex-agent", "codex");
        assert_eq!(store.unanswered_refusal("codex").unwrap(), None);
    }

    #[test]
    fn a_failed_graph_names_and_shows_its_last_failed_steps_error() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let graph = store.test_graph(
            "code",
            "/repo",
            &[("implement", Some("succeeded")), ("gate", None)],
            None,
        );
        let steps = store
            .graph_process(graph.as_str())
            .unwrap()
            .unwrap()
            .0
            .steps;
        assert_eq!(store.graph_failure_cause(&graph).unwrap(), None);
        store
            .finish_run(
                RunOwner::GraphStep,
                &steps[1].run.id,
                5,
                Some(1),
                Some("gate: tests failed"),
            )
            .unwrap();
        // The step's own exit stands; its spawner's later write does not replace it.
        store
            .finish_run(RunOwner::GraphStep, &steps[1].run.id, 6, None, None)
            .unwrap();
        store
            .finish_run(
                RunOwner::GraphRun,
                &graph,
                7,
                Some(1),
                Some("the run of code failed"),
            )
            .unwrap();
        assert_eq!(
            store.graph_failure_cause(&graph).unwrap(),
            Some((7, "gate: tests failed".into()))
        );
        let (graph, entry) = store.graph_process(&graph.as_str()[..8]).unwrap().unwrap();
        assert_eq!(graph.steps[1].run.ended_at, Some(5));
        assert_eq!(graph.steps[1].run.pid, None);
        // `lf graph show <RUN_ID> --json` shows why the step failed.
        let shown = &graph.detail(entry).steps[1];
        assert_eq!(
            (shown.exit_code, shown.error.as_deref()),
            (Some(1), Some("gate: tests failed"))
        );
    }

    #[test]
    fn step_input_is_the_steps_own_capture() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&directory.path().join("loopflow.db")).unwrap();
        let [step, nested] = std::array::from_fn(|_| loopflow_model::id::LfProcessId::new());
        {
            let conn = store.conn.lock().unwrap();
            let process = |id: &loopflow_model::id::LfProcessId,
                           parent: Option<&loopflow_model::id::LfProcessId>,
                           argv: &[&str]| {
                conn.execute(
                    "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,started_at)
                     VALUES(?1,'fixture',?2,?3,1)",
                    rusqlite::params![id, parent, serde_json::to_string(argv).unwrap()],
                )
                .unwrap();
            };
            process(&step, None, &["lf", "--skill-input", "in"]);
            // A command the agent ran is not the step's own capture.
            process(&nested, Some(&step), &["lf", "-b", "skill", "other"]);
            // Agent Processes record a bare executable name instead of argv.
            conn.execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,started_at)
                 VALUES(?1,'fixture',?2,'codex',1)",
                rusqlite::params![loopflow_model::id::LfProcessId::new(), &step],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
                 VALUES('step-session','Step','human',1,0,'/repo'),('nested-session','Nested','human',1,0,'/repo')",
                [],
            )
            .unwrap();
            for (session, key, process) in [
                ("step-session", "step-input", &step),
                ("nested-session", "nested-input", &nested),
            ] {
                conn.execute(
                    "INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload)
                     VALUES(?1,'captured',?2,?3,1,'{}')",
                    rusqlite::params![session, key, process],
                )
                .unwrap();
            }
        }
        let (session, input, _) = store.process_input(&step).unwrap().unwrap();
        assert_eq!(
            (session.as_str(), input.as_str()),
            ("step-session", "step-input")
        );
        let (session, input, _) = store.process_input(&nested).unwrap().unwrap();
        assert_eq!(
            (session.as_str(), input.as_str()),
            ("nested-session", "nested-input")
        );
        assert_eq!(
            store
                .process_input(&loopflow_model::id::LfProcessId::new())
                .unwrap(),
            None
        );
    }

    #[test]
    fn graphs_are_their_graph_and_step_processes() {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&directory.path().join("loopflow.db")).unwrap();
        let running = store.test_graph(
            "feature",
            "/repo",
            &[("design", Some("succeeded")), ("implement", None)],
            None,
        );
        let failed = store.test_graph("ship", "/repo", &[("gate", Some("failed"))], Some("failed"));
        let done = store.test_graph(
            "code",
            "/other",
            &[("implement", Some("succeeded"))],
            Some("succeeded"),
        );

        let all = NonZeroU32::new(10).unwrap();
        let page = store
            .graph_inventory(&GraphProcessFilter::default(), None, all, Execution::result)
            .unwrap();
        assert_eq!(page.entries.len(), 3);
        let state = |id: &loopflow_model::id::LfProcessId| {
            page.entries
                .iter()
                .find(|entry| entry.summary.id == id.as_str())
                .unwrap()
                .summary
                .state
        };
        assert_eq!(state(&running), GraphProcessSummaryState::Current);
        assert_eq!(state(&failed), GraphProcessSummaryState::Stopped);
        assert_eq!(state(&done), GraphProcessSummaryState::Completed);

        let (graph, entry) = store.graph_process(running.as_str()).unwrap().unwrap();
        assert_eq!(entry.summary.name, "feature");
        assert_eq!(
            graph
                .steps
                .iter()
                .map(|step| graph.label(step))
                .collect::<Vec<_>>(),
            ["design", "implement"]
        );
        // What a run launched with is immutable; only its lifecycle moves.
        let conn = store.conn.lock().unwrap();
        assert!(conn
            .execute("UPDATE graph_processes SET graph='other'", [])
            .is_err());
        assert!(conn
            .execute("UPDATE graph_process_steps SET node=9", [])
            .is_err());
        drop(conn);
        let repo = store
            .graph_inventory(
                &GraphProcessFilter {
                    repo: Some("/other".into()),
                    ..Default::default()
                },
                None,
                all,
                Execution::result,
            )
            .unwrap();
        assert_eq!(repo.entries.len(), 1);
        assert_eq!(repo.entries[0].summary.id, done.as_str());
        let first = store
            .graph_inventory(
                &GraphProcessFilter::default(),
                None,
                NonZeroU32::new(2).unwrap(),
                Execution::result,
            )
            .unwrap();
        let rest = store
            .graph_inventory(
                &GraphProcessFilter::default(),
                first.next.as_deref(),
                all,
                Execution::result,
            )
            .unwrap();
        assert_eq!((first.entries.len(), rest.entries.len()), (2, 1));
    }

    #[test]
    fn released_flow_records_read_as_graphs_after_the_rename() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("released.db");
        let conn = rusqlite::Connection::open(&database).unwrap();
        crate::migration_runner::apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "rename_flow_to_graph",
        );
        let compiled = crate::graph::CompiledGraph {
            name: "pursue".into(),
            steps: vec![crate::graph::GraphNode {
                id: None,
                key: 0,
                returns_to: None,
                sources: vec![],
                paths: vec![],
                label: "implement".into(),
                interactive: false,
                kind: crate::graph::GraphNodeKind::Skill,
            }],
        };
        let wave = loopflow_model::id::WaveId::new();
        let task = loopflow_model::durable::TaskId::new();
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/migrations/graph_rename_released.sql"
        ))
        .replace("{compiled}", &serde_json::to_string(&compiled).unwrap())
        .replace("{wave}", wave.as_str())
        .replace(
            "{project}",
            loopflow_model::durable::ProjectId::new().as_str(),
        )
        .replace("{task}", task.as_str())
        .replace("{repo}", directory.path().to_str().unwrap());
        conn.execute_batch(&fixture).unwrap();
        let revisions_before: i64 = conn
            .query_row(
                "SELECT revision FROM store_revisions WHERE domain='flows'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        crate::migration_runner::upgrade_from_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "rename_flow_to_graph",
        );
        drop(conn);

        let store = crate::test_fixture::open(&database).unwrap();
        let (graph, entry) = store
            .graph_process("00000000-0000-4000-8000-00000000f100")
            .unwrap()
            .expect("the released Flow process reads as a Graph process");
        assert_eq!(
            (graph.name.as_str(), graph.compiled.clone()),
            ("pursue", compiled)
        );
        assert_eq!(graph.steps.len(), 1);
        assert_eq!(entry.summary.id, "00000000-0000-4000-8000-00000000f100");
        assert_eq!(store.revisions().unwrap().graphs, revisions_before);

        // A captured graph not at `end` is dropped by the ship draft; none
        // that remains names its Graphs as `flow`.
        let conn = store.conn.lock().unwrap();
        let stored: Option<String> = rusqlite::OptionalExtension::optional(conn.query_row(
            "SELECT graph FROM task_graphs WHERE task_id=?1",
            [task.as_str()],
            |row| row.get(0),
        ))
        .unwrap();
        assert!(stored.is_none_or(|stored| !stored.contains("\"flow\"")));
        drop(conn);
        let events = store.task_events_after(&task, 0).unwrap();
        assert!(
            events.iter().any(|event| matches!(
                &event.kind,
                crate::work::task::TaskEventKind::GraphFinished { graph, .. } if graph == "pursue"
            )),
            "{events:?}"
        );
    }
}
