//! Delivery evidence for local Task decisions; never another lifecycle owner.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::TaskId;
use crate::linear::StateHistory;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

#[derive(Debug, Clone)]
pub(crate) struct TaskStateDelivery {
    pub id: String,
    pub task_id: TaskId,
    pub target: String,
    pub attempted: bool,
    pub base_revision: Option<String>,
    pub base_state: Option<String>,
}

fn read_delivery(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskStateDelivery> {
    Ok(TaskStateDelivery {
        id: row.get(0)?,
        task_id: TaskId::from_raw(row.get::<_, String>(1)?),
        target: row.get(2)?,
        attempted: row.get(3)?,
        base_revision: row.get(4)?,
        base_state: row.get(5)?,
    })
}

pub(super) fn queue_in(conn: &Connection, task: &TaskId, target: &str) -> StoreResult<()> {
    let completed_at = (target == "completed")
        .then(|| {
            time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)
        })
        .transpose()
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            task.as_str(),
            &[Edit::Disposition(crate::work::edit::Disposition {
                planning_state: Some(target.into()),
                planning_completed: i64::from(target == "completed"),
                planning_completed_at: completed_at,
            })],
        ),
    )?;
    conn.execute(
        "INSERT INTO task_state_deliveries(id,task_id,target,base_revision,base_state)
         SELECT ?2,t.id,?3,i.revision,i.state
         FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
         LEFT JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
         WHERE t.id=?1",
        params![task.as_str(), uuid::Uuid::new_v4().to_string(), target],
    )?;
    Ok(())
}

/// Acquisition and delivery use the same transaction rule. No execution rows change.
/// Only state delivery, holding the Task's delivery lock, supplies known history.
pub(crate) fn reconcile_in(
    conn: &Connection,
    task: &TaskId,
    observed_revision: Option<&str>,
    observed: &crate::work::edit::Disposition,
    evidence: &str,
    history: StateHistory,
) -> StoreResult<bool> {
    let pending: Option<(TaskStateDelivery, Option<String>)> = conn.query_row(
        "SELECT d.id,d.task_id,d.target,d.attempted,d.base_revision,d.base_state,t.planning_provider_revision
         FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
         WHERE d.task_id=?1 AND d.settled=0
         AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?1)",
        [task.as_str()],
        |row| Ok((read_delivery(row)?, row.get(6)?)),
    ).optional()?;
    let Some((delivery, revision)) = pending else {
        return Ok(false);
    };
    let TaskStateDelivery {
        id,
        target,
        base_state,
        attempted,
        ..
    } = delivery;
    if crate::linear::observations::revision_nanos(observed_revision)?
        < crate::linear::observations::revision_nanos(revision.as_deref())?
    {
        return Ok(false);
    }
    let Some(state) = observed.planning_state.as_deref() else {
        return Ok(true);
    };
    if state == target {
        conn.execute(
            "UPDATE task_state_deliveries SET settled=1,error=NULL WHERE id=?1",
            [&id],
        )?;
        return Ok(false);
    }
    // An unchanged state is pending unless history proves an uncertain write
    // landed or the issue moved. Only proof of no effect re-arms the write.
    if observed.planning_state == base_state {
        match (attempted, history) {
            (false, _) | (true, StateHistory::Unknown) => return Ok(true),
            (true, StateHistory::Unchanged) => {
                conn.execute(
                    "UPDATE task_state_deliveries SET attempted=0,base_revision=?2 WHERE id=?1",
                    params![id, observed_revision],
                )?;
                return Ok(true);
            }
            (true, StateHistory::Changed) => {}
        }
    }
    conn.execute(
        "UPDATE task_state_deliveries SET settled=1,conflict_json=?2,error=NULL WHERE id=?1",
        params![id, evidence],
    )?;
    planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            task.as_str(),
            &[Edit::Disposition(observed.clone())],
        ),
    )?;
    conn.execute(
        "UPDATE tasks SET planning_provider_revision=?2 WHERE id=?1",
        params![task.as_str(), observed_revision],
    )?;
    super::children::insert_task_event_in(conn, task, &crate::work::task::TaskEventKind::Progress {
        summary: format!("Adopted Linear state {state}; local intention {target} remains in delivery {id}. Local Workflow unchanged."),
    })?;
    Ok(false)
}

impl SqliteStore {
    pub(crate) fn pending_planning_tasks(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<crate::work::task::Task>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT t.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
            WHERE w.repo=?1 AND t.external_issue_id IS NOT NULL AND (EXISTS(
                SELECT 1 FROM task_state_deliveries d WHERE d.task_id=t.id AND d.settled=0
                AND d.conflict_json IS NULL
                AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)) OR EXISTS(
                SELECT 1 FROM task_comments c JOIN task_comment_deliveries d ON d.comment_id=c.id
                WHERE c.task_id=t.id AND d.acknowledged=0 AND d.conflicting_comment_json IS NULL))")?;
        let ids = query
            .query_map([repo], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                super::children::task_on(&conn, &TaskId::from_raw(id))?.ok_or(StoreError::NotFound)
            })
            .collect()
    }

    pub(crate) fn pending_task_state(
        &self,
        task: &TaskId,
    ) -> StoreResult<Option<TaskStateDelivery>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT d.id,d.task_id,d.target,d.attempted,d.base_revision,d.base_state
             FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
             WHERE d.task_id=?1 AND d.settled=0 AND t.external_issue_id IS NOT NULL
             AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)",
            [task.as_str()],
            read_delivery,
        )
        .optional()
        .map_err(StoreError::from)
    }

    /// Acquire the effect only while the captured decision is still current.
    /// A canceled request remains attempted, so a later read must resolve it.
    pub(crate) fn attempt_task_state(
        &self,
        delivery: &TaskStateDelivery,
        retry: bool,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE task_state_deliveries SET attempted=1
             WHERE id=?1 AND (attempted=0 OR ?3) AND settled=0 AND conflict_json IS NULL
             AND seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?2)",
            params![delivery.id, delivery.task_id.as_str(), retry],
        )? == 1)
    }

    /// Positive rate-limit rejection is not an uncertain provider effect.
    pub(crate) fn reject_task_state(&self, delivery: &TaskStateDelivery) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("UPDATE task_state_deliveries SET attempted=0 WHERE id=?1 AND settled=0 AND conflict_json IS NULL", [&delivery.id])?;
        Ok(())
    }

    /// Failed delivery retains its receipt; only provider observation settles state.
    pub(crate) fn task_state_error(
        &self,
        delivery: &TaskStateDelivery,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_state_deliveries SET error=?2
             WHERE id=?1 AND settled=0 AND conflict_json IS NULL AND error IS NOT ?2",
            params![delivery.id, error],
        )?;
        Ok(())
    }
}
