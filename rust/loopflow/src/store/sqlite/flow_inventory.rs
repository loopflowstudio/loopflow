//! FlowProcess: what a Flow process recorded, read back with its Processes. Only a
//! Flow process writes it, and only by appending. Reading selects nothing and grants
//! neither attachment nor process authority.

use std::num::NonZeroU32;

use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{FlowProcessFilter, FlowProcessInventoryEntry, FlowProcessPage, TaskId};
use crate::flow::graph::FlowGraph;
use crate::id::{LfProcessId, WaveId};
use crate::ops::flow_process::{FlowProcess, FlowProcessStep};
use crate::session::{FlowProcessSummary, FlowProcessSummaryState};
use crate::store::{StoreError, StoreResult};

use super::processes::{read_process, PROCESS_SELECT};
use super::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// Flows whose Flow process `e` matches `scope`, oldest first, each with the
/// steps its Flow process recorded in launch order.
pub(super) fn flows_in(
    conn: &Connection,
    scope: &str,
    values: &[&dyn rusqlite::ToSql],
) -> StoreResult<Vec<FlowProcess>> {
    let columns = PROCESS_SELECT
        .strip_suffix(" FROM processes e")
        .expect("Process select ends with its table");
    let flow_processes = conn
        .prepare(&format!(
            "{columns},f.flow,f.graph FROM flow_processes f JOIN processes e ON e.id=f.lf_process_id
             WHERE ({scope}) ORDER BY e.rowid"
        ))?
        .query_map(values, |row| {
            Ok((
                read_process(row)?,
                row.get::<_, String>("flow")?,
                row.get::<_, String>("graph")?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut steps = conn.prepare(&format!(
        "{columns},fs.node,fs.iterations FROM flow_process_steps fs JOIN processes e ON e.id=fs.lf_process_id
         WHERE fs.flow_lf_process_id=?1 ORDER BY fs.seq"
    ))?;
    flow_processes
        .into_iter()
        .map(|(flow_process, name, graph)| {
            let steps = steps
                .query_map([&flow_process.id], |row| {
                    Ok((
                        read_process(row)?,
                        row.get::<_, u32>("node")?,
                        row.get::<_, String>("iterations")?,
                    ))
                })?
                .map(|row| {
                    let (process, key, iterations) = row?;
                    Ok(FlowProcessStep {
                        process,
                        key,
                        iterations: serde_json::from_str(&iterations)?,
                    })
                })
                .collect::<StoreResult<Vec<_>>>()?;
            Ok(FlowProcess {
                graph: serde_json::from_str::<FlowGraph>(&graph)?,
                process: flow_process,
                name,
                steps,
            })
        })
        .collect()
}

/// What the Flow process says of the Flow, and the Work its checkout names.
pub(super) fn entry_in(
    conn: &Connection,
    flow: &FlowProcess,
) -> StoreResult<FlowProcessInventoryEntry> {
    let process = &flow.process;
    let state =
        FlowProcessSummaryState::of_process(process.outcome.as_deref(), process.completed_at);
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
    Ok(FlowProcessInventoryEntry {
        summary: FlowProcessSummary {
            id: process.id.to_string(),
            name: flow.name.clone(),
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
                .or(flow.latest().map(|step| step.process.started_at))
                .unwrap_or(process.started_at),
        },
        repo: process.repo.clone(),
        ended_at: process.completed_at,
    })
}

impl SqliteStore {
    /// Flows in Flow-process-id order after `after`, at most `limit`.
    pub fn flow_inventory(
        &self,
        filter: &FlowProcessFilter,
        after: Option<&str>,
        limit: NonZeroU32,
    ) -> StoreResult<FlowProcessPage> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flows = match &filter.task_id {
            Some(task) => super::task_work::flows_of_task(&conn, task)?,
            None => flows_in(&conn, "1", &[])?,
        };
        let search = filter.search.as_deref().map(str::to_lowercase);
        let mut entries = Vec::new();
        for flow in &flows {
            let entry = entry_in(&conn, flow)?;
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
        Ok(FlowProcessPage { entries, next })
    }

    /// One Flow by its Flow process id or a unique prefix of it.
    pub(crate) fn flow_process(
        &self,
        selector: &str,
    ) -> StoreResult<Option<(FlowProcess, FlowProcessInventoryEntry)>> {
        let Some(process) = self.resolve_process(selector)? else {
            return Ok(None);
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(flow) = flows_in(&conn, "e.id=?1", &[&process.id])?.pop() else {
            return Ok(None);
        };
        let entry = entry_in(&conn, &flow)?;
        Ok(Some((flow, entry)))
    }

    /// Flows run from `cwd`, oldest first.
    pub(crate) fn flows_at(&self, cwd: &std::path::Path) -> StoreResult<Vec<FlowProcess>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flows_in(&conn, "e.cwd=?1", &[&cwd.to_string_lossy()])
    }

    /// A Flow process's one write about its Flow: the name and graph it launched with.
    pub(crate) fn record_flow_process(
        &self,
        flow_process: &LfProcessId,
        graph: &FlowGraph,
        task: Option<&TaskId>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO flow_processes(lf_process_id,flow,graph) VALUES(?1,?2,?3)",
            params![flow_process, graph.name, serde_json::to_string(graph)?],
        )?;
        let associated = super::task_work::task_of_process(&tx, flow_process)?;
        if let Some(task) = task.map(TaskId::as_str).or(associated.as_deref()) {
            tx.execute(
                "UPDATE tasks SET started_at=?2 WHERE id=?1 AND started_at IS NULL",
                params![task, crate::store::rows::now_unix()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// A Flow process's record of one step it started, as that step's Process.
    pub(crate) fn record_flow_step(
        &self,
        flow_process: &LfProcessId,
        step: &LfProcessId,
        key: u32,
        iterations: &[Vec<u32>],
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO flow_process_steps(flow_lf_process_id,lf_process_id,node,iterations) VALUES(?1,?2,?3,?4)",
            params![flow_process, step, key, serde_json::to_string(iterations)?],
        )?;
        Ok(())
    }

    /// Where the Process table ends now; a child started later lies beyond it.
    pub(crate) fn process_mark(&self) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(
            conn.query_row("SELECT COALESCE(MAX(rowid),0) FROM processes", [], |row| {
                row.get(0)
            })?,
        )
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

    pub(crate) fn flow_entry(&self, flow: &FlowProcess) -> StoreResult<FlowProcessInventoryEntry> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        entry_in(&conn, flow)
    }

    /// The conversation, captured input and its event a process opened last.
    pub(crate) fn process_input(
        &self,
        process: &LfProcessId,
    ) -> StoreResult<Option<(String, String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT session_id,receipt_key,seq FROM session_events
                 WHERE kind='captured' AND lf_process_id=?1 ORDER BY seq DESC LIMIT 1",
                params![process],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?)
    }
}

#[cfg(test)]
impl SqliteStore {
    /// Record a Flow process and one step Process per `(label, outcome)`, as a Flow
    /// run from `cwd` leaves them.
    pub(crate) fn test_flow(
        &self,
        name: &str,
        cwd: &str,
        steps: &[(&str, Option<&str>)],
        flow_outcome: Option<&str>,
    ) -> LfProcessId {
        use crate::flow::ConcreteSkill;
        use crate::flow::ConcreteStep;
        use crate::flow::Skill;
        let flow_process = LfProcessId::new();
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
            &flow_process,
            None,
            vec!["lf".into(), "run".into(), name.into()],
            flow_outcome,
        );
        let compiled: Vec<_> = steps
            .iter()
            .map(|(label, _)| {
                ConcreteStep::Skill(ConcreteSkill {
                    skill: Skill::named(label),
                    id: None,
                    human: false,
                    returns: None,
                    sources: Vec::new(),
                })
            })
            .collect();
        self.record_flow_process(&flow_process, &FlowGraph::new(name, &compiled), None)
            .unwrap();
        for (index, (label, outcome)) in steps.iter().enumerate() {
            let step = LfProcessId::new();
            let mut argv = vec!["lf".to_string()];
            argv.extend(label.split(' ').map(str::to_string));
            insert(&step, Some(&flow_process), argv, *outcome);
            self.record_flow_step(&flow_process, &step, index as u32, &[vec![]])
                .unwrap();
        }
        flow_process
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use crate::durable::FlowProcessFilter;
    use crate::session::FlowProcessSummaryState;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn failed_task_start_records_no_flow() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("loopflow.db")).unwrap();
        let task = crate::durable::TaskId::new();
        let project = crate::durable::ProjectId::new();
        let wave = crate::id::WaveId::new();
        let flow_process = crate::id::LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", rusqlite::params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES(?1,?2,'issue','PROOF-1','/repo/task',1)",rusqlite::params![task.as_str(),project.as_str()]).unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo/caller',1)",
                rusqlite::params![flow_process, crate::id::TraceId::new()],
            )
            .unwrap();
        }
        let graph = crate::flow::graph::FlowGraph::new("proof", &[]);
        let error = store
            .record_flow_process(&flow_process, &graph, Some(&task))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Started requires recorded Task work"),
            "{error}"
        );
        assert!(store.flow_process(flow_process.as_str()).unwrap().is_none());
        let conn = store.conn.lock().unwrap();
        let started: Option<i64> = conn
            .query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(started, None);
    }

    #[test]
    fn flows_are_their_flow_and_step_processes() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).unwrap();
        let running = store.test_flow(
            "feature",
            "/repo",
            &[("design", Some("succeeded")), ("implement", None)],
            None,
        );
        let failed = store.test_flow("ship", "/repo", &[("gate", Some("failed"))], Some("failed"));
        let done = store.test_flow(
            "code",
            "/other",
            &[("implement", Some("succeeded"))],
            Some("succeeded"),
        );

        let all = NonZeroU32::new(10).unwrap();
        let page = store
            .flow_inventory(&FlowProcessFilter::default(), None, all)
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
        assert_eq!(state(&running), FlowProcessSummaryState::Current);
        assert_eq!(state(&failed), FlowProcessSummaryState::Stopped);
        assert_eq!(state(&done), FlowProcessSummaryState::Completed);

        let (flow, entry) = store.flow_process(running.as_str()).unwrap().unwrap();
        assert_eq!(entry.summary.name, "feature");
        assert_eq!(
            flow.steps
                .iter()
                .map(|step| flow.label(step))
                .collect::<Vec<_>>(),
            ["design", "implement"]
        );
        // The record is append-only, and a step is a process its Flow process started.
        let conn = store.conn.lock().unwrap();
        assert!(conn
            .execute("UPDATE flow_processes SET flow='other'", [])
            .is_err());
        assert!(conn
            .execute("UPDATE flow_process_steps SET node=9", [])
            .is_err());
        assert!(conn
            .execute(
                "INSERT INTO flow_process_steps(flow_lf_process_id,lf_process_id,node,iterations) VALUES(?1,?2,0,'[]')",
                rusqlite::params![running, done],
            )
            .is_err());
        drop(conn);
        let repo = store
            .flow_inventory(
                &FlowProcessFilter {
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
            .flow_inventory(
                &FlowProcessFilter::default(),
                None,
                NonZeroU32::new(2).unwrap(),
            )
            .unwrap();
        let rest = store
            .flow_inventory(&FlowProcessFilter::default(), first.next.as_deref(), all)
            .unwrap();
        assert_eq!((first.entries.len(), rest.entries.len()), (2, 1));
    }
}
