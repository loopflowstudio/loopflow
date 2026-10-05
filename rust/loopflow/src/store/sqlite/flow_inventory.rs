//! Flows read back from Execs: a driver and the step Execs it started. Reading
//! selects nothing and grants neither driver nor process authority.

use std::num::NonZeroU32;

use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::{FlowFilter, FlowInventoryEntry, FlowPage, TaskId};
use crate::id::{ExecId, WaveId};
use crate::ops::flow_run::{FlowExecs, FlowStep, FLOW_STEP_COMMAND};
use crate::session::{FlowSummary, FlowSummaryState};
use crate::store::{StoreError, StoreResult};

use super::execs::{read_exec, EXEC_SELECT};
use super::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// Step Execs matching `scope` (a condition on `e`), grouped under their
/// drivers in launch order. A step whose driver left no Exec is not a Flow.
pub(super) fn flows_in(
    conn: &Connection,
    scope: &str,
    values: &[&dyn rusqlite::ToSql],
) -> StoreResult<Vec<FlowExecs>> {
    let steps = conn
        .prepare(&format!(
            "{EXEC_SELECT} WHERE e.parent_exec_id IS NOT NULL
             AND instr(e.command,'{FLOW_STEP_COMMAND}')>0 AND ({scope}) ORDER BY e.rowid"
        ))?
        .query_map(values, read_exec)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut flows: Vec<FlowExecs> = Vec::new();
    for exec in steps {
        let Some(step) = FlowStep::of_exec(&exec) else {
            continue;
        };
        let parent = exec.parent_exec_id.clone().expect("selected with a parent");
        if let Some(flow) = flows.iter_mut().find(|flow| flow.driver.id == parent) {
            flow.steps.push((step, exec));
            continue;
        }
        let driver = conn
            .query_row(
                &format!("{EXEC_SELECT} WHERE e.id=?1"),
                [&parent],
                read_exec,
            )
            .optional()?;
        if let Some(driver) = driver {
            flows.push(FlowExecs {
                driver,
                steps: vec![(step, exec)],
            });
        }
    }
    for flow in &mut flows {
        flow.steps.sort_by_key(|(step, _)| step.seq);
    }
    Ok(flows)
}

/// What the driver's Exec says of the Flow, and the Work its checkout names.
pub(super) fn entry_in(conn: &Connection, flow: &FlowExecs) -> StoreResult<FlowInventoryEntry> {
    let driver = &flow.driver;
    let state = match (driver.outcome.as_deref(), driver.completed_at) {
        (Some("succeeded"), _) => FlowSummaryState::Completed,
        (None, None) => FlowSummaryState::Current,
        _ => FlowSummaryState::Stopped,
    };
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
            name: flow.name().to_owned(),
            state,
            task_id: task
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: wave
                .map(|id| WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            updated_at: driver.completed_at.unwrap_or(flow.latest().1.started_at),
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
            Some(task) => flows_in(
                &conn,
                &format!("e.id IN ({})", super::task_work::exec_ids("?1")),
                &[&task.as_str()],
            )?,
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
    pub(crate) fn flow_execs(
        &self,
        selector: &str,
    ) -> StoreResult<Option<(FlowExecs, FlowInventoryEntry)>> {
        let Some(driver) = self.resolve_exec(selector)? else {
            return Ok(None);
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(flow) = flows_in(&conn, "e.parent_exec_id=?1", &[&driver.id])?.pop() else {
            return Ok(None);
        };
        let entry = entry_in(&conn, &flow)?;
        Ok(Some((flow, entry)))
    }

    /// An operation step is work begun for the Task whose checkout it ran in.
    pub(crate) fn mark_task_started(&self, task: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tasks SET started_at=?2 WHERE id=?1 AND started_at IS NULL",
            params![task.as_str(), crate::store::rows::now_unix()],
        )?;
        Ok(())
    }

    /// Flows whose steps ran in `cwd`, oldest first.
    pub(crate) fn flows_at(&self, cwd: &std::path::Path) -> StoreResult<Vec<FlowExecs>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flows_in(&conn, "e.cwd=?1", &[&cwd.to_string_lossy()])
    }

    /// The Exec `driver` started for its `seq`th step launch, newest first.
    pub(crate) fn flow_step_exec(
        &self,
        driver: &ExecId,
        seq: u32,
    ) -> StoreResult<Option<crate::exec::Exec>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "{EXEC_SELECT} WHERE e.parent_exec_id=?1
             AND instr(e.command,'{FLOW_STEP_COMMAND}')>0 ORDER BY e.rowid DESC"
        ))?;
        for exec in query.query_map(params![driver], read_exec)? {
            let exec = exec?;
            if FlowStep::of_exec(&exec).is_some_and(|step| step.seq == seq) {
                return Ok(Some(exec));
            }
        }
        Ok(None)
    }

    pub(crate) fn flow_entry(&self, flow: &FlowExecs) -> StoreResult<FlowInventoryEntry> {
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
        let conn = self.conn.lock().expect("store mutex poisoned");
        let driver = ExecId::new();
        let insert = |id: &ExecId,
                      parent: Option<&ExecId>,
                      argv: Vec<String>,
                      outcome: Option<&str>| {
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
        for (index, (label, outcome)) in steps.iter().enumerate() {
            let step = FlowStep {
                flow: name.into(),
                seq: index as u32 + 1,
                label: (*label).into(),
                cursor: crate::engine::ExecutionCursor {
                    index,
                    ..Default::default()
                },
                key: index as u32,
                iterations: vec![vec![]],
                skill: None,
                output: None,
                session: None,
            };
            // A label of several words is an operation step; one word a skill.
            let mut argv = vec!["lf".to_string()];
            if label.contains(' ') {
                argv.extend([
                    crate::ops::flow_run::FLOW_STEP_COMMAND.into(),
                    step.arg().unwrap(),
                ]);
                argv.extend(label.split(' ').map(str::to_string));
            } else {
                argv.extend([
                    crate::ops::flow_run::FLOW_STEP_ARG.into(),
                    step.arg().unwrap(),
                    "skill".into(),
                    (*label).into(),
                ]);
            }
            insert(&ExecId::new(), Some(&driver), argv, *outcome);
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

        let (flow, entry) = store.flow_execs(running.as_str()).unwrap().unwrap();
        assert_eq!(entry.summary.name, "feature");
        assert_eq!(
            flow.steps
                .iter()
                .map(|(step, _)| step.label.as_str())
                .collect::<Vec<_>>(),
            ["design", "implement"]
        );
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
