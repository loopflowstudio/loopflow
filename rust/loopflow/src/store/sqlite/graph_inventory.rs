//! GraphProcess: what a Graph process recorded, read back with its Processes. Only a
//! Graph process writes it, and only by appending. Reading selects nothing and grants
//! neither attachment nor process authority.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::path::PathBuf;

use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{GraphProcessFilter, GraphProcessInventoryEntry, GraphProcessPage, TaskId};
use crate::graph::compiled::CompiledGraph;
use crate::id::{LfProcessId, WaveId};
use crate::ops::graph_process::{GraphProcess, GraphProcessStep};
use crate::session::{GraphProcessSummary, GraphProcessSummaryState};
use crate::store::{StoreError, StoreResult};

use super::processes::{read_lf_process, PROCESS_SELECT};
use super::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// Graphs whose Graph process `e` matches `scope`, oldest first, each with the
/// steps its Graph process recorded in launch order.
pub(super) fn graphs_in(
    conn: &Connection,
    scope: &str,
    values: &[&dyn rusqlite::ToSql],
) -> StoreResult<Vec<GraphProcess>> {
    let columns = PROCESS_SELECT
        .strip_suffix(" FROM processes e")
        .expect("Process select ends with its table");
    let graph_processes = conn
        .prepare(&format!(
            "{columns},g.graph,g.compiled FROM graph_processes g JOIN processes e ON e.id=g.lf_process_id
             WHERE ({scope}) ORDER BY e.rowid"
        ))?
        .query_map(values, |row| {
            Ok((
                read_lf_process(row)?,
                row.get::<_, String>("graph")?,
                row.get::<_, String>("compiled")?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // Every Graph's steps in one statement, so a reading of many Graphs costs two.
    let ids = super::children::ids_json(
        graph_processes
            .iter()
            .map(|(process, _, _)| process.id.as_str()),
    );
    let mut steps = HashMap::<String, Vec<GraphProcessStep>>::new();
    for row in conn
        .prepare(&format!(
            "{columns},gs.graph_lf_process_id,gs.node,gs.iterations FROM graph_process_steps gs
             JOIN processes e ON e.id=gs.lf_process_id
             WHERE gs.graph_lf_process_id IN (SELECT value FROM json_each(?1)) ORDER BY gs.seq"
        ))?
        .query_map([&ids], |row| {
            Ok((
                read_lf_process(row)?,
                row.get::<_, String>("graph_lf_process_id")?,
                row.get::<_, u32>("node")?,
                row.get::<_, String>("iterations")?,
            ))
        })?
    {
        let (process, graph, key, iterations) = row?;
        steps.entry(graph).or_default().push(GraphProcessStep {
            process,
            key,
            iterations: serde_json::from_str(&iterations)?,
        });
    }
    graph_processes
        .into_iter()
        .map(|(graph_process, name, compiled)| {
            Ok(GraphProcess {
                compiled: serde_json::from_str::<CompiledGraph>(&compiled)?,
                steps: steps.remove(graph_process.id.as_str()).unwrap_or_default(),
                process: graph_process,
                name,
            })
        })
        .collect()
}

/// What the Graph process says of the Graph, including the Task it started for.
pub(super) fn entry_in(
    conn: &Connection,
    graph: &GraphProcess,
) -> StoreResult<GraphProcessInventoryEntry> {
    let process = &graph.process;
    let task = conn
        .query_row(
            "SELECT task_id FROM graph_processes WHERE lf_process_id=?1",
            [&process.id],
            |row| row.get::<_, Option<String>>(0),
        )?
        .map(|id| TaskId::parse(&id))
        .transpose()
        .map_err(invalid)?;
    let wave = match &task {
        Some(task) => super::durable::task_wave_in(conn, task)?,
        None => conn
            .query_row(
                "SELECT c.wave_id FROM session_events c JOIN processes e ON e.id=c.lf_process_id
                 WHERE c.kind='captured' AND e.parent_lf_process_id=?1 AND c.wave_id IS NOT NULL
                 ORDER BY c.seq DESC LIMIT 1",
                [&process.id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|id| WaveId::parse(&id))
            .transpose()
            .map_err(invalid)?,
    };
    entry_of(graph, task, wave)
}

/// The Graph's inventory entry once its Task and Wave are known.
pub(crate) fn entry_of(
    graph: &GraphProcess,
    task_id: Option<TaskId>,
    wave_id: Option<WaveId>,
) -> StoreResult<GraphProcessInventoryEntry> {
    let process = &graph.process;
    Ok(GraphProcessInventoryEntry {
        summary: GraphProcessSummary {
            id: process.id.to_string(),
            name: graph.name.clone(),
            state: GraphProcessSummaryState::of_process(
                process.outcome.as_deref(),
                process.completed_at,
            ),
            task_id,
            wave_id,
            updated_at: process
                .completed_at
                .or(graph.latest().map(|step| step.process.started_at))
                .unwrap_or(process.started_at),
        },
        repo: process.repo.clone(),
        ended_at: process.completed_at,
    })
}

/// What a reading of Graphs' execution asks of the store once: the input each
/// Process captured last, open Graph execution in the selected Tasks' work,
/// and the receipt files of unfinished Processes.
#[derive(Debug, Default)]
pub(crate) struct GraphExecutionFacts {
    pub open_processes: HashMap<TaskId, Vec<(LfProcessId, crate::process::LfProcess)>>,
    pub inputs: HashMap<LfProcessId, (String, String, i64)>,
    pub receipts: HashMap<LfProcessId, PathBuf>,
    pub limit_recoveries: HashMap<LfProcessId, AgentLimitRecovery>,
}

/// How a headless step is getting past a usage limit. `retry_at` set means
/// nothing can launch before then; none means another account took the step on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentLimitRecovery {
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
    /// Whether `process` runs under a Graph, through any process between.
    pub(crate) fn process_under_graph(&self, process: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "WITH RECURSIVE up(id) AS (
                 SELECT ?1
                 UNION
                 SELECT p.parent_lf_process_id FROM processes p JOIN up ON p.id=up.id
                 WHERE p.parent_lf_process_id IS NOT NULL)
             SELECT EXISTS(SELECT 1 FROM up JOIN graph_processes g ON g.lf_process_id=up.id)",
            [process],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn agent_limit_recovery(
        &self,
        step: &LfProcessId,
    ) -> StoreResult<Option<AgentLimitRecovery>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(limit_recoveries(&conn, &[step])?.remove(step))
    }

    /// Say how step Process `id` is recovering from a usage limit.
    pub(crate) fn record_agent_limit_recovery(
        &self,
        id: &LfProcessId,
        provider: &str,
        reason: &str,
        retry_at: Option<i64>,
        agent: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO agent_limit_recoveries(lf_process_id,provider,reason,retry_at,agent,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(lf_process_id) DO UPDATE SET provider=excluded.provider,
                reason=excluded.reason,retry_at=excluded.retry_at,agent=excluded.agent,
                updated_at=excluded.updated_at",
            params![
                id,
                provider,
                reason,
                retry_at,
                agent,
                crate::store::rows::now_unix()
            ],
        )?;
        Ok(())
    }

    /// `step`'s wait is over: its launch is under way, or someone interrupted it.
    pub(crate) fn end_agent_limit_wait(&self, step: &LfProcessId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_limit_recoveries SET retry_at=NULL,updated_at=?2
             WHERE retry_at IS NOT NULL AND lf_process_id=?1",
            params![step, crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// The Tasks of every wait whose time has come. A run restarted at its step
    /// leaves the old wait behind, so a Task here may hold nothing any more.
    pub(crate) fn tasks_past_a_limit_wait(&self, now: i64) -> StoreResult<HashSet<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let waits = conn
            .prepare("SELECT lf_process_id FROM agent_limit_recoveries WHERE retry_at<=?1")?
            .query_map([now], |row| row.get(0))?
            .collect::<Result<Vec<LfProcessId>, _>>()?;
        let mut tasks = HashSet::new();
        for process in waits {
            if let Some(task) = super::task_work::task_of_process(&conn, &process)? {
                tasks.insert(
                    TaskId::parse(&task)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                );
            }
        }
        Ok(tasks)
    }

    /// Graphs in Graph-process-id order after `after`, at most `limit`.
    pub fn graph_inventory(
        &self,
        filter: &GraphProcessFilter,
        after: Option<&str>,
        limit: NonZeroU32,
    ) -> StoreResult<GraphProcessPage> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let graphs = match &filter.task_id {
            Some(task) => super::task_work::graphs_of_task(&conn, task)?,
            None => graphs_in(&conn, "1", &[])?,
        };
        let search = filter.search.as_deref().map(str::to_lowercase);
        let mut entries = Vec::new();
        for graph in &graphs {
            let entry = entry_in(&conn, graph)?;
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
    pub(crate) fn graph_process(
        &self,
        selector: &str,
    ) -> StoreResult<Option<(GraphProcess, GraphProcessInventoryEntry)>> {
        let Some(process) = self.resolve_process(selector)? else {
            return Ok(None);
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(graph) = graphs_in(&conn, "e.id=?1", &[&process.id()])?.pop() else {
            return Ok(None);
        };
        let entry = entry_in(&conn, &graph)?;
        Ok(Some((graph, entry)))
    }

    /// Graphs run from `cwd`, oldest first.
    pub(crate) fn graphs_at(&self, cwd: &std::path::Path) -> StoreResult<Vec<GraphProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        graphs_in(&conn, "e.cwd=?1", &[&cwd.to_string_lossy()])
    }

    /// A Graph process's one write about its Graph: the name and graph it
    /// launched with, and the Task it started for.
    pub(crate) fn record_graph_process(
        &self,
        graph_process: &LfProcessId,
        compiled: &CompiledGraph,
        task: Option<&TaskId>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id) VALUES(?1,?2,?3,?4)",
            params![
                graph_process,
                compiled.name,
                serde_json::to_string(compiled)?,
                task.map(TaskId::as_str)
            ],
        )?;
        Ok(())
    }

    /// A Graph process's record of one step it started, as that step's Process.
    pub(crate) fn record_graph_step(
        &self,
        graph_process: &LfProcessId,
        step: &LfProcessId,
        key: u32,
        iterations: &[Vec<u32>],
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations) VALUES(?1,?2,?3,?4)",
            params![graph_process, step, key, serde_json::to_string(iterations)?],
        )?;
        Ok(())
    }

    /// Where `parent`'s children end now; a child started later lies beyond it.
    /// A running parent's children are never forgotten, and SQLite gives a new
    /// row a rowid above every remaining one, so forgetting other finished rows
    /// cannot put a later child behind this mark.
    pub(crate) fn process_mark(&self, parent: &LfProcessId) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT COALESCE(MAX(rowid),0) FROM processes WHERE parent_lf_process_id=?1",
            [parent],
            |row| row.get(0),
        )?)
    }

    /// The first Process `parent` started after `mark`.
    pub(crate) fn child_process_after(
        &self,
        parent: &LfProcessId,
        mark: i64,
    ) -> StoreResult<Option<LfProcessId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT id FROM processes WHERE parent_lf_process_id=?1 AND rowid>?2 ORDER BY rowid LIMIT 1",
                params![parent, mark],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The first Graph process `parent` started after `mark`. Other children
    /// it started meanwhile, such as a Task conversation started beside its
    /// run, may be recorded before or after it.
    pub(crate) fn graph_child_after(
        &self,
        parent: &LfProcessId,
        mark: i64,
    ) -> StoreResult<Option<LfProcessId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT e.id FROM processes e JOIN graph_processes g ON g.lf_process_id=e.id
                 WHERE e.parent_lf_process_id=?1 AND e.rowid>?2 ORDER BY e.rowid LIMIT 1",
                params![parent, mark],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The conversation, captured input and its event a process opened last.
    pub(crate) fn process_input(
        &self,
        process: &LfProcessId,
    ) -> StoreResult<Option<(String, String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(process_inputs(&conn, &[process])?.remove(process))
    }

    /// Execution facts of `graphs`: their latest steps' captured inputs and
    /// open Graph execution in `tasks`, and the receipts of unfinished Processes.
    pub(crate) fn graph_execution_facts<'a>(
        &self,
        graphs: impl IntoIterator<Item = &'a GraphProcess>,
        tasks: &[&TaskId],
    ) -> StoreResult<GraphExecutionFacts> {
        let graphs: Vec<_> = graphs.into_iter().collect();
        let steps: Vec<_> = graphs
            .iter()
            .filter_map(|graph| graph.latest().map(|step| &step.process.id))
            .collect();
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let pairs = super::task_work::task_open_process_pairs(
            "SELECT value FROM json_each(?1)",
            "p.kind='lf' AND EXISTS(SELECT 1 FROM graph_processes g WHERE g.lf_process_id IN (p.id,p.parent_lf_process_id))",
        );
        let ids = super::children::ids_json(tasks.iter().map(|task| task.as_str()));
        let mut open_processes =
            HashMap::<TaskId, Vec<(LfProcessId, crate::process::LfProcess)>>::new();
        let columns = PROCESS_SELECT
            .strip_suffix(" FROM processes e")
            .expect("Process select ends with its table");
        // Keep membership outside the Process lookup: ORDER BY otherwise
        // persuades SQLite to scan all history through processes_recent.
        let mut query = conn.prepare(&format!(
            "{columns},pairs.task_id,g.lf_process_id FROM ({pairs}) pairs
             CROSS JOIN processes e ON e.id=pairs.process
             JOIN graph_processes g ON g.lf_process_id=coalesce(
                (SELECT own.lf_process_id FROM graph_processes own WHERE own.lf_process_id=e.id),e.parent_lf_process_id)
             ORDER BY e.started_at DESC,e.id"
        ))?;
        for row in query.query_map([&ids], |row| {
            Ok((
                row.get::<_, String>(19)?,
                row.get::<_, LfProcessId>(20)?,
                read_lf_process(row)?,
            ))
        })? {
            let (task, graph, process) = row?;
            let task = TaskId::parse(&task).map_err(invalid)?;
            open_processes
                .entry(task)
                .or_default()
                .push((graph, process));
        }
        let unfinished = super::children::ids_json(
            graphs
                .iter()
                .flat_map(|graph| {
                    std::iter::once(&graph.process).chain(graph.latest().map(|step| &step.process))
                })
                .filter(|process| process.completed_at.is_none())
                .map(|process| process.id.as_str())
                .chain(
                    open_processes
                        .values()
                        .flatten()
                        .map(|(_, process)| process.id.as_str()),
                ),
        );
        let inputs = process_inputs(&conn, &steps)?;
        let limit_recoveries = limit_recoveries(&conn, &steps)?;
        let home = conn
            .path()
            .filter(|path| !path.is_empty())
            .and_then(|path| std::path::Path::new(path).parent())
            .map(std::path::Path::to_path_buf);
        let mut receipts = HashMap::new();
        if let Some(home) = home {
            for row in conn
                .prepare(
                    "SELECT id,receipt_locator FROM processes
                     WHERE id IN (SELECT value FROM json_each(?1)) AND receipt_locator IS NOT NULL",
                )?
                .query_map([&unfinished], |row| {
                    Ok((row.get::<_, LfProcessId>(0)?, row.get::<_, String>(1)?))
                })?
            {
                let (id, locator) = row?;
                receipts.insert(id, crate::journal::process_receipt_path(&home, &locator));
            }
        }
        Ok(GraphExecutionFacts {
            open_processes,
            inputs,
            receipts,
            limit_recoveries,
        })
    }
}

#[cfg(test)]
impl SqliteStore {
    /// Record a Graph process and one step Process per `(label, outcome)`, as a Graph
    /// run from `cwd` leaves them.
    pub(crate) fn test_graph(
        &self,
        name: &str,
        cwd: &str,
        steps: &[(&str, Option<&str>)],
        graph_outcome: Option<&str>,
    ) -> LfProcessId {
        use crate::graph::ConcreteSkill;
        use crate::graph::ConcreteStep;
        use crate::graph::Skill;
        let graph_process = LfProcessId::new();
        let insert = |id: &LfProcessId,
                      parent: Option<&LfProcessId>,
                      argv: Vec<String>,
                      outcome: Option<&str>| {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,repo,cwd,started_at,completed_at,outcome)
                 VALUES(?1,?8,?2,?3,?4,?4,?5,?6,?7)",
                params![
                    id,
                    parent,
                    serde_json::to_string(&argv).unwrap(),
                    cwd,
                    crate::store::rows::now_unix(),
                    outcome.map(|_| crate::store::rows::now_unix()),
                    outcome,
                    crate::id::TraceId::new()
                ],
            )
            .unwrap();
        };
        insert(
            &graph_process,
            None,
            vec!["lf".into(), "run".into(), name.into()],
            graph_outcome,
        );
        let compiled: Vec<_> = steps
            .iter()
            .map(|(label, _)| {
                ConcreteStep::Skill(ConcreteSkill {
                    skill: Skill::named(label),
                    id: None,
                    interactive: false,
                    interactive_name: None,
                    returns: None,
                    sources: Vec::new(),
                })
            })
            .collect();
        // A Graph started in a checkout is that Task's, as the launch binds it.
        let task = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            super::task_work::task_of_process(&conn, &graph_process)
                .unwrap()
                .map(|task| TaskId::parse(&task).unwrap())
        };
        self.record_graph_process(
            &graph_process,
            &CompiledGraph::new(name, &compiled),
            task.as_ref(),
        )
        .unwrap();
        for (index, (label, outcome)) in steps.iter().enumerate() {
            let step = LfProcessId::new();
            let mut argv = vec!["lf".to_string()];
            argv.extend(label.split(' ').map(str::to_string));
            insert(&step, Some(&graph_process), argv, *outcome);
            self.record_graph_step(&graph_process, &step, index as u32, &[vec![]])
                .unwrap();
        }
        graph_process
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use crate::durable::GraphProcessFilter;
    use crate::session::GraphProcessSummaryState;
    use crate::store::sqlite::SqliteStore;

    /// Two waits on one Task's step and a later wait still running its time.
    #[test]
    fn a_task_past_its_limit_wait_is_found_once_and_only_when_due() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let repository = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO tasks(repository_id,id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id)
                 VALUES(?1,?2,'issue','PROOF-1','/repo/task',1,(SELECT id FROM machines WHERE is_local=1))",
                rusqlite::params![repository.as_str(), task.as_str()],
            )
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
                    &steps[step].process.id,
                    "claude",
                    "limit",
                    Some(retry_at),
                    None,
                )
                .unwrap();
        };
        wait(0, 200);
        assert!(store.tasks_past_a_limit_wait(100).unwrap().is_empty());
        wait(1, 100);
        assert_eq!(store.tasks_past_a_limit_wait(200).unwrap().len(), 1);
        assert!(store.tasks_past_a_limit_wait(200).unwrap().contains(&task));
    }

    #[test]
    fn a_step_lands_after_its_graphs_mark_when_an_unrelated_rowid_is_reused() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let process = |parent: Option<&crate::id::LfProcessId>, pid: u32| {
            let process = crate::process::LfProcess {
                id: crate::id::LfProcessId::new(),
                pid: Some(pid),
                os_started_at: None,
                trace_id: crate::id::TraceId::new(),
                parent_lf_process_id: parent.cloned(),
                via_agent: Some(false),
                caller_session_id: None,
                caller_agent_process_id: None,
                command: None,
                repo: None,
                cwd: None,
                started_at: 1,
                completed_at: None,
                outcome: None,
                exit_code: None,
                signal: None,
                error: None,
            };
            store.record_process(&process).unwrap();
            process
        };
        let graph = process(None, 10);
        // A concurrent `lf task status` holds the highest rowid when the Graph
        // marks, then exits and is forgotten before the step registers.
        let status = process(None, 11);
        let mark = store.process_mark(&graph.id).unwrap();
        store
            .record_process(&crate::process::LfProcess {
                completed_at: Some(2),
                outcome: Some("succeeded".into()),
                ..status
            })
            .unwrap();
        let step = process(Some(&graph.id), 12);
        assert_eq!(
            store.child_process_after(&graph.id, mark).unwrap(),
            Some(step.id)
        );
    }

    #[test]
    fn a_task_run_finds_its_graph_after_a_conversation_started_beside_it() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let [carrier, conversation, graph] = std::array::from_fn(|_| crate::id::LfProcessId::new());
        let execute = |sql: &str, params: &[&dyn rusqlite::ToSql]| {
            store.conn.lock().unwrap().execute(sql, params).unwrap();
        };
        let process = |id: &crate::id::LfProcessId, parent: Option<&crate::id::LfProcessId>| {
            execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,started_at) VALUES(?1,'fixture',?2,1)",
                rusqlite::params![id, parent],
            );
        };
        process(&carrier, None);
        let mark = store.process_mark(&carrier).unwrap();
        // The conversation started beside the run registers before its Graph.
        process(&conversation, Some(&carrier));
        process(&graph, Some(&carrier));
        assert_eq!(store.graph_child_after(&carrier, mark).unwrap(), None);
        execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'proof','{}')",
            rusqlite::params![graph],
        );
        assert_eq!(
            store.graph_child_after(&carrier, mark).unwrap(),
            Some(graph)
        );
    }

    #[test]
    fn step_input_is_the_steps_own_capture() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).unwrap();
        let [step, nested] = std::array::from_fn(|_| crate::id::LfProcessId::new());
        {
            let conn = store.conn.lock().unwrap();
            let process = |id: &crate::id::LfProcessId,
                           parent: Option<&crate::id::LfProcessId>,
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
                rusqlite::params![crate::id::LfProcessId::new(), &step],
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
            store.process_input(&crate::id::LfProcessId::new()).unwrap(),
            None
        );
    }

    #[test]
    fn graphs_are_their_graph_and_step_processes() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).unwrap();
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
            .graph_inventory(&GraphProcessFilter::default(), None, all)
            .unwrap();
        assert_eq!(page.entries.len(), 3);
        let state = |id: &crate::id::LfProcessId| {
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
        // The record is append-only, and a step is a process its Graph process started.
        let conn = store.conn.lock().unwrap();
        assert!(conn
            .execute("UPDATE graph_processes SET graph='other'", [])
            .is_err());
        assert!(conn
            .execute("UPDATE graph_process_steps SET node=9", [])
            .is_err());
        assert!(conn
            .execute(
                "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations) VALUES(?1,?2,0,'[]')",
                rusqlite::params![running, done],
            )
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
            )
            .unwrap();
        assert_eq!(repo.entries.len(), 1);
        assert_eq!(repo.entries[0].summary.id, done.as_str());
        let first = store
            .graph_inventory(
                &GraphProcessFilter::default(),
                None,
                NonZeroU32::new(2).unwrap(),
            )
            .unwrap();
        let rest = store
            .graph_inventory(&GraphProcessFilter::default(), first.next.as_deref(), all)
            .unwrap();
        assert_eq!((first.entries.len(), rest.entries.len()), (2, 1));
    }

    #[test]
    fn released_flow_records_read_as_graphs_after_the_rename() {
        use crate::graph::compiled::CompiledGraph;
        use crate::graph::{ConcreteSkill, ConcreteStep, Skill};

        let _ledger = crate::journal::TestLedgerGuard::new();
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("released.db");
        let conn = rusqlite::Connection::open(&database).unwrap();
        crate::store::migrations::apply_before_migration(&conn, "rename_flow_to_graph");
        let step = ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named("implement"),
            id: None,
            interactive: false,
            interactive_name: None,
            returns: None,
            sources: Vec::new(),
        });
        let compiled = CompiledGraph::new("pursue", &[step]);
        let wave = crate::id::WaveId::new();
        let task = crate::durable::TaskId::new();
        let fixture = include_str!("../../../tests/fixtures/migrations/graph_rename_released.sql")
            .replace("{compiled}", &serde_json::to_string(&compiled).unwrap())
            .replace("{wave}", wave.as_str())
            .replace("{project}", crate::durable::ProjectId::new().as_str())
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
        crate::store::migrations::upgrade_from_migration(&conn, "rename_flow_to_graph");
        drop(conn);

        let store = SqliteStore::new(&database).unwrap();
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
