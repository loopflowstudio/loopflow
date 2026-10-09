//! Delivery evidence for local Task decisions; never another lifecycle owner.

use rusqlite::{params, Connection, OptionalExtension};

use crate::durable::TaskId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

#[derive(Debug, Clone)]
pub(crate) struct TaskStateDelivery {
    pub id: String,
    pub task_id: TaskId,
    pub target: String,
    pub attempted: bool,
    base_revision: Option<String>,
    base_state: Option<String>,
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
    conn.execute(
        "UPDATE tasks SET planning_state=?2,planning_completed=(?2='completed'),
         planning_completed_at=?3,planning_revision=planning_revision+1 WHERE id=?1",
        params![task.as_str(), target, completed_at],
    )?;
    let move_seq = conn.query_row(
        "SELECT max(seq) FROM task_workflow_moves WHERE task_id=?1",
        [task.as_str()],
        |row| row.get(0),
    )?;
    record_in(
        conn,
        task,
        &uuid::Uuid::new_v4().to_string(),
        target,
        move_seq,
        None,
    )
}

/// Save delivery separately from the planning value. Peer imports have their own
/// stable mutation identity and no local Workflow move; retries retain attempts.
pub(super) fn record_in(
    conn: &Connection,
    task: &TaskId,
    receipt: &str,
    target: &str,
    move_seq: Option<i64>,
    baseline: Option<&serde_json::Value>,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO task_state_deliveries(id,task_id,move_seq,target,base_revision,base_state)
         SELECT ?2,t.id,?4,?3,
             CASE WHEN ?5 IS NULL THEN json_extract(i.body,'$.revision') ELSE json_extract(?5,'$.revision') END,
             CASE WHEN ?5 IS NULL THEN json_extract(i.body,'$.state') ELSE json_extract(?5,'$.value.planning_state') END
         FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
         LEFT JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
         WHERE t.id=?1 ON CONFLICT(id) DO NOTHING",
        params![task.as_str(), receipt, target, move_seq, baseline.map(serde_json::Value::to_string)],
    )?;
    Ok(())
}

/// Acquisition and delivery use the same transaction rule. No execution rows change.
pub(super) fn reconcile_in(
    conn: &Connection,
    task: &TaskId,
    observed: &crate::pm::PmItem,
) -> StoreResult<bool> {
    reconcile_observation(conn, task, observed, false)
}

/// The journal selected a concurrent provider winner. Unlike acquisition alone,
/// this retires a losing intention even when the provider baseline is unchanged.
pub(super) fn adopt_peer_in(
    conn: &Connection,
    task: &TaskId,
    observed: &crate::pm::PmItem,
) -> StoreResult<()> {
    reconcile_observation(conn, task, observed, true).map(|_| ())
}

fn reconcile_observation(
    conn: &Connection,
    task: &TaskId,
    observed: &crate::pm::PmItem,
    peer_winner: bool,
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
        base_revision,
        attempted,
        ..
    } = delivery;
    if super::planning::revision_nanos(observed.revision.as_deref())?
        < super::planning::revision_nanos(revision.as_deref())?
    {
        return Ok(false);
    }
    let Some(state) = observed.state.as_deref() else {
        return Ok(true);
    };
    if state == target {
        conn.execute(
            "UPDATE task_state_deliveries SET settled=1,error=NULL WHERE id=?1",
            [&id],
        )?;
        return Ok(false);
    }
    // An unrelated provider revision does not conflict with an unattempted save.
    // After an uncertain write, a changed revision may represent completion/reopening.
    if !peer_winner
        && observed.state == base_state
        && !(attempted && observed.revision != base_revision)
    {
        return Ok(true);
    }
    conn.execute(
        "UPDATE task_state_deliveries SET settled=1,conflict_json=?2,error=NULL WHERE id=?1",
        params![id, serde_json::to_string(observed)?],
    )?;
    conn.execute(
        "UPDATE tasks SET planning_state=?2,planning_completed=?3,planning_completed_at=?4,
         planning_provider_revision=?5,planning_revision=planning_revision+1 WHERE id=?1",
        params![
            task.as_str(),
            observed.state,
            observed.completed,
            observed.completed_at,
            observed.revision
        ],
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
        let mut query = conn.prepare("SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
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

    /// Reconcile a delivery's read; true means its saved intention remains pending.
    pub(crate) fn observe_task_state(
        &self,
        delivery: &TaskStateDelivery,
        observed: &crate::pm::PmItem,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        // Retain the provider baseline for the next local save. A response for an
        // older delivery cannot ingest over the current decision.
        let repo: Option<String> = tx
            .query_row(
                "SELECT w.repo FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
             JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
             WHERE d.id=?1 AND d.task_id=?2 AND d.settled=0 AND t.external_issue_id=?3
             AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)",
                params![delivery.id, delivery.task_id.as_str(), observed.id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(repo) = repo else { return Ok(false) };
        if !super::planning::put_item(
            &tx,
            &repo,
            "linear",
            super::super::rows::now_unix(),
            observed,
        )? {
            return Ok(false);
        }
        let pending = reconcile_in(&tx, &delivery.task_id, observed)?;
        tx.commit()?;
        Ok(pending)
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
    pub(crate) fn attempt_task_state(&self, delivery: &TaskStateDelivery) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE task_state_deliveries SET attempted=1
             WHERE id=?1 AND attempted=0 AND settled=0 AND conflict_json IS NULL
             AND seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?2)",
            params![delivery.id, delivery.task_id.as_str()],
        )? == 1)
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
