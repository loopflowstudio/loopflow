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
    pub base_revision: Option<String>,
    pub base_state: Option<String>,
    pub attempted: bool,
    pub conflict: Option<String>,
}

pub(super) fn queue_in(conn: &Connection, task: &TaskId, target: &str) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO task_state_deliveries(id,task_id,move_seq,target,base_revision,base_state)
         SELECT ?2,t.id,(SELECT max(seq) FROM task_workflow_moves WHERE task_id=t.id),?3,
             json_extract(i.body,'$.revision'),json_extract(i.body,'$.state')
         FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
         LEFT JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
         WHERE t.id=?1",
        params![task.as_str(), uuid::Uuid::new_v4().to_string(), target],
    )?;
    Ok(())
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
                WHERE c.task_id=t.id AND d.acknowledged=0 AND d.conflicting_comment_json IS NULL AND d.resolution IS NULL))")?;
        let ids = query
            .query_map([repo], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| {
                super::children::task_on(&conn, &TaskId::from_raw(id))?.ok_or(StoreError::NotFound)
            })
            .collect()
    }

    pub(crate) fn conflict_task_state(
        &self,
        delivery: &TaskStateDelivery,
        observed: &crate::pm::PmItem,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let error = format!("State conflict: saved locally as {}; Linear reports {} at revision {}. Use `lf task sync {} --resolve local` or `--resolve linear`",
            delivery.target, observed.state.as_deref().unwrap_or("unknown"), observed.revision.as_deref().unwrap_or("unknown"), delivery.task_id);
        conn.execute(
            "UPDATE task_state_deliveries SET conflict_json=?2,error=?3
             WHERE id=?1 AND settled=0 AND conflict_json IS NULL",
            params![
                delivery.id,
                serde_json::json!({"state":observed.state,"revision":observed.revision})
                    .to_string(),
                error
            ],
        )?;
        Ok(())
    }

    pub(crate) fn resolve_task_state(
        &self,
        delivery: &TaskStateDelivery,
        observed: &crate::pm::PmItem,
        keep_local: bool,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_state_deliveries WHERE id=?1
            AND seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?2))",
            params![delivery.id, delivery.task_id.as_str()],
            |row| row.get(0),
        )?;
        if !current {
            return Err(StoreError::InvalidAuthority(
                "Task decision changed; inspect its current synchronization".into(),
            ));
        }
        tx.execute(
            "INSERT INTO task_state_deliveries(id,task_id,move_seq,target,base_revision,base_state,settled)
                SELECT ?2,task_id,move_seq,target,?3,?4,?5 FROM task_state_deliveries WHERE id=?1",
            params![
                delivery.id,
                uuid::Uuid::new_v4().to_string(),
                observed.revision,
                observed.state,
                !keep_local,
            ],
        )?;
        super::children::insert_task_event_in(&tx, &delivery.task_id, &crate::work::task::TaskEventKind::Progress {
            summary: format!("Resolved planning state delivery {}: retained {}. Local Workflow unchanged. Linear state: {} at revision {}",
                delivery.id, if keep_local { "local decision for synchronization" } else { "Linear state" },
                observed.state.as_deref().unwrap_or("unknown"), observed.revision.as_deref().unwrap_or("unknown")),
        })?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn pending_task_state(
        &self,
        task: &TaskId,
    ) -> StoreResult<Option<TaskStateDelivery>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT d.id,d.target,d.base_revision,d.base_state,d.attempted,d.conflict_json
             FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
             WHERE d.task_id=?1 AND d.settled=0 AND t.external_issue_id IS NOT NULL
             AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)",
            [task.as_str()],
            |row| {
                Ok(TaskStateDelivery {
                    id: row.get(0)?,
                    task_id: task.clone(),
                    target: row.get(1)?,
                    base_revision: row.get(2)?,
                    base_state: row.get(3)?,
                    attempted: row.get(4)?,
                    conflict: row.get(5)?,
                })
            },
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

    /// A late response can describe its effect, but cannot settle a newer decision.
    pub(crate) fn settle_task_state(
        &self,
        delivery: &TaskStateDelivery,
        error: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_state_deliveries SET settled=?2,error=?3
             WHERE id=?1 AND settled=0 AND conflict_json IS NULL
             AND (settled IS NOT ?2 OR error IS NOT ?3)",
            params![delivery.id, error.is_none(), error],
        )?;
        Ok(())
    }
}
