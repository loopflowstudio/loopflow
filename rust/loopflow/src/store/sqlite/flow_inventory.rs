//! FlowExec: what a Flow's driver recorded, read back with its Execs. Only a
//! driver writes it, and only by appending. Reading selects nothing and grants
//! neither driver nor process authority.

use std::num::NonZeroU32;

use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{FlowFilter, FlowInventoryEntry, FlowPage, TaskId};
use crate::engine::flow_graph::FlowGraph;
use crate::id::{ExecId, WaveId};
use crate::ops::flow_run::{FlowExec, FlowExecStep};
use crate::session::{FlowSummary, FlowSummaryState};
use crate::store::{StoreError, StoreResult};

use super::execs::{read_exec, EXEC_SELECT};
use super::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// Flows whose driver Exec `e` matches `scope`, oldest first, each with the
/// steps its driver recorded in launch order.
pub(super) fn flows_in(
    conn: &Connection,
    scope: &str,
    values: &[&dyn rusqlite::ToSql],
) -> StoreResult<Vec<FlowExec>> {
    let columns = EXEC_SELECT
        .strip_suffix(" FROM execs e")
        .expect("Exec select ends with its table");
    let drivers = conn
        .prepare(&format!(
            "{columns},f.flow,f.graph FROM flow_execs f JOIN execs e ON e.id=f.exec_id
             WHERE ({scope}) ORDER BY e.rowid"
        ))?
        .query_map(values, |row| {
            Ok((
                read_exec(row)?,
                row.get::<_, String>(15)?,
                row.get::<_, String>(16)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut steps = conn.prepare(&format!(
        "{columns},fs.node,fs.iterations FROM flow_exec_steps fs JOIN execs e ON e.id=fs.exec_id
         WHERE fs.flow_exec_id=?1 ORDER BY fs.seq"
    ))?;
    drivers
        .into_iter()
        .map(|(driver, name, graph)| {
            let steps = steps
                .query_map([&driver.id], |row| {
                    Ok((
                        read_exec(row)?,
                        row.get::<_, u32>(15)?,
                        row.get::<_, String>(16)?,
                    ))
                })?
                .map(|row| {
                    let (exec, key, iterations) = row?;
                    Ok(FlowExecStep {
                        exec,
                        key,
                        iterations: serde_json::from_str(&iterations)?,
                    })
                })
                .collect::<StoreResult<Vec<_>>>()?;
            Ok(FlowExec {
                graph: serde_json::from_str::<FlowGraph>(&graph)?,
                driver,
                name,
                steps,
            })
        })
        .collect()
}

/// What the driver's Exec says of the Flow, and the Work its checkout names.
pub(super) fn entry_in(conn: &Connection, flow: &FlowExec) -> StoreResult<FlowInventoryEntry> {
    let driver = &flow.driver;
    let state = FlowSummaryState::of_driver(driver.outcome.as_deref(), driver.completed_at);
    let task: Option<String> = match &driver.cwd {
        Some(cwd) => conn
            .query_row(
                "SELECT tw.id FROM tasks tw WHERE tw.worktree!='' AND (?1=rtrim(tw.worktree,'/')
                    OR instr(?1,rtrim(tw.worktree,'/')||'/')=1)
                 ORDER BY length(tw.worktree) DESC LIMIT 1",
                [cwd],
                |row| row.get(0),
            )
            .optional()?,
        None => None,
    };
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
                "SELECT c.wave_id FROM session_events c JOIN execs e ON e.id=c.exec_id
                 WHERE c.kind='captured' AND e.parent_exec_id=?1 AND c.wave_id IS NOT NULL
                 ORDER BY c.seq DESC LIMIT 1",
                [&driver.id],
                |row| row.get(0),
            )
            .optional()?,
    };
    Ok(FlowInventoryEntry {
        summary: FlowSummary {
            id: driver.id.to_string(),
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
            updated_at: driver
                .completed_at
                .or(flow.latest().map(|step| step.exec.started_at))
                .unwrap_or(driver.started_at),
        },
        repo: driver.repo.clone(),
        ended_at: driver.completed_at,
    })
}

impl SqliteStore {
    /// Flows in driver-id order after `after`, at most `limit`.
    pub fn flow_inventory(
        &self,
        filter: &FlowFilter,
        after: Option<&str>,
        limit: NonZeroU32,
    ) -> StoreResult<FlowPage> {
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
        Ok(FlowPage { entries, next })
    }

    /// One Flow by its driver Exec id or a unique prefix of it.
    pub(crate) fn flow_exec(
        &self,
        selector: &str,
    ) -> StoreResult<Option<(FlowExec, FlowInventoryEntry)>> {
        let Some(driver) = self.resolve_exec(selector)? else {
            return Ok(None);
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(flow) = flows_in(&conn, "e.id=?1", &[&driver.id])?.pop() else {
            return Ok(None);
        };
        let entry = entry_in(&conn, &flow)?;
        Ok(Some((flow, entry)))
    }

    /// A Flow run from a Task's checkout is work begun for that Task.
    pub(crate) fn mark_task_started(&self, task: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tasks SET started_at=?2 WHERE id=?1 AND started_at IS NULL",
            params![task.as_str(), crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// Flows run from `cwd`, oldest first.
    pub(crate) fn flows_at(&self, cwd: &std::path::Path) -> StoreResult<Vec<FlowExec>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flows_in(&conn, "e.cwd=?1", &[&cwd.to_string_lossy()])
    }

    /// A driver's one write about its Flow: the name and graph it launched with.
    pub(crate) fn record_flow_exec(
        &self,
        driver: &ExecId,
        flow: &str,
        graph: &FlowGraph,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO flow_execs(exec_id,flow,graph) VALUES(?1,?2,?3)",
            params![driver, flow, serde_json::to_string(graph)?],
        )?;
        Ok(())
    }

    /// A driver's record of one step it started, as that step's Exec.
    pub(crate) fn record_flow_step(
        &self,
        driver: &ExecId,
        step: &ExecId,
        key: u32,
        iterations: &[Vec<u32>],
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO flow_exec_steps(flow_exec_id,exec_id,node,iterations) VALUES(?1,?2,?3,?4)",
            params![driver, step, key, serde_json::to_string(iterations)?],
        )?;
        Ok(())
    }

    /// Where the Exec table ends now; a child started later lies beyond it.
    pub(crate) fn exec_mark(&self) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(
            conn.query_row("SELECT COALESCE(MAX(rowid),0) FROM execs", [], |row| {
                row.get(0)
            })?,
        )
    }

    /// The first Exec `parent` started after `mark`.
    pub(crate) fn child_exec_after(
        &self,
        parent: &ExecId,
        mark: i64,
    ) -> StoreResult<Option<ExecId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT id FROM execs WHERE parent_exec_id=?1 AND rowid>?2 ORDER BY rowid LIMIT 1",
                params![parent, mark],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(crate) fn flow_entry(&self, flow: &FlowExec) -> StoreResult<FlowInventoryEntry> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        entry_in(&conn, flow)
    }

    /// The conversation, captured input and its event an Exec opened last.
    pub(crate) fn exec_input(&self, exec: &ExecId) -> StoreResult<Option<(String, String, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT session_id,receipt_key,seq FROM session_events
                 WHERE kind='captured' AND exec_id=?1 ORDER BY seq DESC LIMIT 1",
                params![exec],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?)
    }
}

#[cfg(test)]
impl SqliteStore {
    /// Record a driver and one step Exec per `(label, outcome)`, as a Flow
    /// run from `cwd` leaves them.
    pub(crate) fn test_flow(
        &self,
        name: &str,
        cwd: &str,
        steps: &[(&str, Option<&str>)],
        driver_outcome: Option<&str>,
    ) -> ExecId {
        use crate::engine::{ConcreteSkill, ConcreteStep, Skill};
        let driver = ExecId::new();
        let insert = |id: &ExecId,
                      parent: Option<&ExecId>,
                      argv: Vec<String>,
                      outcome: Option<&str>| {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "INSERT INTO execs(id,trace_id,parent_exec_id,command,repo,cwd,started_at,completed_at,outcome)
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
            &driver,
            None,
            vec!["lf".into(), "run".into(), name.into()],
            driver_outcome,
        );
        let compiled: Vec<_> = steps
            .iter()
            .map(|(label, _)| {
                ConcreteStep::Skill(ConcreteSkill {
                    skill: Skill::named(label),
                    id: None,
                    human: false,
                    repeat: None,
                    sources: Vec::new(),
                })
            })
            .collect();
        self.record_flow_exec(&driver, name, &FlowGraph::new(name, &compiled))
            .unwrap();
        for (index, (label, outcome)) in steps.iter().enumerate() {
            let step = ExecId::new();
            let mut argv = vec!["lf".to_string()];
            argv.extend(label.split(' ').map(str::to_string));
            insert(&step, Some(&driver), argv, *outcome);
            self.record_flow_step(&driver, &step, index as u32, &[vec![]])
                .unwrap();
        }
        driver
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use crate::durable::FlowFilter;
    use crate::session::FlowSummaryState;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn flows_are_their_driver_and_step_execs() {
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
            .flow_inventory(&FlowFilter::default(), None, all)
            .unwrap();
        assert_eq!(page.entries.len(), 3);
        let state = |id: &crate::id::ExecId| {
            page.entries
                .iter()
                .find(|entry| entry.summary.id == id.as_str())
                .unwrap()
                .summary
                .state
        };
        assert_eq!(state(&running), FlowSummaryState::Current);
        assert_eq!(state(&failed), FlowSummaryState::Stopped);
        assert_eq!(state(&done), FlowSummaryState::Completed);

        let (flow, entry) = store.flow_exec(running.as_str()).unwrap().unwrap();
        assert_eq!(entry.summary.name, "feature");
        assert_eq!(
            flow.steps
                .iter()
                .map(|step| flow.label(step))
                .collect::<Vec<_>>(),
            ["design", "implement"]
        );
        // The record is append-only, and a step is an Exec its driver started.
        let conn = store.conn.lock().unwrap();
        assert!(conn
            .execute("UPDATE flow_execs SET flow='other'", [])
            .is_err());
        assert!(conn
            .execute("UPDATE flow_exec_steps SET node=9", [])
            .is_err());
        assert!(conn
            .execute(
                "INSERT INTO flow_exec_steps(flow_exec_id,exec_id,node,iterations) VALUES(?1,?2,0,'[]')",
                rusqlite::params![running, done],
            )
            .is_err());
        drop(conn);
        let repo = store
            .flow_inventory(
                &FlowFilter {
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
            .flow_inventory(&FlowFilter::default(), None, NonZeroU32::new(2).unwrap())
            .unwrap();
        let rest = store
            .flow_inventory(&FlowFilter::default(), first.next.as_deref(), all)
            .unwrap();
        assert_eq!((first.entries.len(), rest.entries.len()), (2, 1));
    }
}
