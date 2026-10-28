//! GraphProcess: what a Graph process recorded, read back with its Processes. Only a
//! Graph process writes it, and only by appending. Reading selects nothing and grants
//! neither attachment nor process authority.

use std::num::NonZeroU32;

use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{GraphProcessFilter, GraphProcessInventoryEntry, GraphProcessPage, TaskId};
use crate::graph::compiled::CompiledGraph;
use crate::id::{LfProcessId, WaveId};
use crate::ops::graph_process::{GraphProcess, GraphProcessStep};
use crate::session::{GraphProcessSummary, GraphProcessSummaryState};
use crate::store::{StoreError, StoreResult};

use super::processes::{read_process, PROCESS_SELECT};
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
                read_process(row)?,
                row.get::<_, String>("graph")?,
                row.get::<_, String>("compiled")?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut steps = conn.prepare(&format!(
        "{columns},gs.node,gs.iterations FROM graph_process_steps gs JOIN processes e ON e.id=gs.lf_process_id
         WHERE gs.graph_lf_process_id=?1 ORDER BY gs.seq"
    ))?;
    graph_processes
        .into_iter()
        .map(|(graph_process, name, compiled)| {
            let steps = steps
                .query_map([&graph_process.id], |row| {
                    Ok((
                        read_process(row)?,
                        row.get::<_, u32>("node")?,
                        row.get::<_, String>("iterations")?,
                    ))
                })?
                .map(|row| {
                    let (process, key, iterations) = row?;
                    Ok(GraphProcessStep {
                        process,
                        key,
                        iterations: serde_json::from_str(&iterations)?,
                    })
                })
                .collect::<StoreResult<Vec<_>>>()?;
            Ok(GraphProcess {
                compiled: serde_json::from_str::<CompiledGraph>(&compiled)?,
                process: graph_process,
                name,
                steps,
            })
        })
        .collect()
}

/// What the Graph process says of the Graph, and the Work its checkout names.
pub(super) fn entry_in(
    conn: &Connection,
    graph: &GraphProcess,
) -> StoreResult<GraphProcessInventoryEntry> {
    let process = &graph.process;
    let state =
        GraphProcessSummaryState::of_process(process.outcome.as_deref(), process.completed_at);
    let task = super::task_work::task_of_process(conn, &process.id)?;
    let wave: Option<String> = match &task {
        Some(task) => conn
            .query_row(
                "SELECT p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
                [task],
                |row| row.get(0),
            )
            .optional()?,
        None => conn
            .query_row(
                "SELECT c.wave_id FROM session_events c JOIN processes e ON e.id=c.lf_process_id
                 WHERE c.kind='captured' AND e.parent_lf_process_id=?1 AND c.wave_id IS NOT NULL
                 ORDER BY c.seq DESC LIMIT 1",
                [&process.id],
                |row| row.get(0),
            )
            .optional()?,
    };
    Ok(GraphProcessInventoryEntry {
        summary: GraphProcessSummary {
            id: process.id.to_string(),
            name: graph.name.clone(),
            state,
            task_id: task
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: wave
                .map(|id| WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            updated_at: process
                .completed_at
                .or(graph.latest().map(|step| step.process.started_at))
                .unwrap_or(process.started_at),
        },
        repo: process.repo.clone(),
        ended_at: process.completed_at,
    })
}

impl SqliteStore {
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
        let Some(graph) = graphs_in(&conn, "e.id=?1", &[&process.id])?.pop() else {
            return Ok(None);
        };
        let entry = entry_in(&conn, &graph)?;
        Ok(Some((graph, entry)))
    }

    /// Whether `process` is a Graph process.
    pub(crate) fn runs_graph(&self, process: &LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=?1)",
            [process],
            |row| row.get(0),
        )?)
    }

    /// Graphs run from `cwd`, oldest first.
    pub(crate) fn graphs_at(&self, cwd: &std::path::Path) -> StoreResult<Vec<GraphProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        graphs_in(&conn, "e.cwd=?1", &[&cwd.to_string_lossy()])
    }

    /// A Graph process's one write about its Graph: the name and graph it launched with.
    pub(crate) fn record_graph_process(
        &self,
        graph_process: &LfProcessId,
        compiled: &CompiledGraph,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,?2,?3)",
            params![
                graph_process,
                compiled.name,
                serde_json::to_string(compiled)?
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

    pub(crate) fn graph_entry(
        &self,
        graph: &GraphProcess,
    ) -> StoreResult<GraphProcessInventoryEntry> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        entry_in(&conn, graph)
    }

    /// The conversation, captured input and its event a process opened last.
    /// A process that delegated to its transport owner captures through the
    /// command that transport serves, so the delegation chain counts as itself.
    pub(crate) fn process_input(
        &self,
        process: &LfProcessId,
    ) -> StoreResult<Option<(String, String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                // Agent Processes record a bare executable name, not an argv array.
                "WITH RECURSIVE delegated(id,serves) AS (
                     SELECT id,CASE WHEN json_valid(command) THEN json_extract(command,'$[1]')='session' AND json_extract(command,'$[2]')='serve' END
                     FROM processes WHERE id=?1
                     UNION
                     SELECT p.id,CASE WHEN json_valid(p.command) THEN json_extract(p.command,'$[1]')='session' AND json_extract(p.command,'$[2]')='serve' END
                     FROM processes p JOIN delegated d ON p.parent_lf_process_id=d.id
                     WHERE d.serves OR (CASE WHEN json_valid(p.command) THEN json_extract(p.command,'$[1]')='session' AND json_extract(p.command,'$[2]')='serve' END))
                 SELECT session_id,receipt_key,seq FROM session_events
                 WHERE kind='captured' AND lf_process_id IN (SELECT id FROM delegated)
                 ORDER BY seq DESC LIMIT 1",
                params![process],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?)
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
                    returns: None,
                    sources: Vec::new(),
                })
            })
            .collect();
        self.record_graph_process(&graph_process, &CompiledGraph::new(name, &compiled))
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

    #[test]
    fn a_step_lands_after_its_graphs_mark_when_an_unrelated_rowid_is_reused() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let process = |parent: Option<&crate::id::LfProcessId>, pid: u32| {
            let process = crate::process::LfProcess {
                id: crate::id::LfProcessId::new(),
                pid: Some(pid),
                kind: crate::process::ProcessKind::Lf,
                agent_session_id: None,
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
    fn step_input_is_read_through_transport_delegation() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).unwrap();
        let [step, transport, served, nested] =
            std::array::from_fn(|_| crate::id::LfProcessId::new());
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
            process(
                &transport,
                Some(&step),
                &["lf", "session", "serve", "--", "lf"],
            );
            process(&served, Some(&transport), &["lf", "--skill-input", "in"]);
            // A command the agent ran is not the step's own capture.
            process(&nested, Some(&served), &["lf", "-b", "skill", "other"]);
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
                ("step-session", "step-input", &served),
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
        for process in [&step, &transport, &served] {
            let (session, input, _) = store.process_input(process).unwrap().unwrap();
            assert_eq!(
                (session.as_str(), input.as_str()),
                ("step-session", "step-input")
            );
        }
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
        crate::store::migrations::apply_before_current_draft(&conn, "rename_flow_to_graph");
        let step = ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named("implement"),
            id: None,
            interactive: false,
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
        crate::store::migrations::upgrade_from_draft(&conn, "rename_flow_to_graph");
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
