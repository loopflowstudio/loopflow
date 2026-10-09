//! Project and Task delivery presentation comes from the existing receipts.
use rusqlite::{Connection, Row};
use serde_json::Value;

use super::SqliteStore;
use crate::durable::{ProjectId, TaskId};
use crate::planning::{PlanningSyncChange, PlanningSyncState, PlanningSyncStatus};
use crate::store::StoreResult;

impl SqliteStore {
    pub fn task_planning_sync(&self, task: &TaskId) -> StoreResult<PlanningSyncStatus> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let (repo, project): (String, String) = tx.query_row(
            "SELECT w.repo,p.id FROM tasks t JOIN projects p ON p.id=t.project_id
             JOIN waves w ON w.id=p.wave_id WHERE t.id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let connected = crate::ops::linear_observe::connected(&repo);
        let mut changes = fields(&tx, "task", task.as_str(), connected, false)?;
        changes.extend(fields(&tx, "project", &project, connected, true)?);
        append(&tx, &mut changes,
            "SELECT d.id,'state',json_quote(d.target),d.attempted,d.error,
                CASE WHEN d.conflict_json IS NOT NULL THEN json_quote(json_extract(d.conflict_json,'$.state')) END
             FROM task_state_deliveries d WHERE d.task_id=?1 AND
             (d.conflict_json IS NOT NULL OR (?2 AND d.settled=0 AND
                (d.attempted=1 OR d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?1)))) ORDER BY d.seq",
            task.as_str(), connected)?;
        append(&tx, &mut changes,
            "SELECT d.comment_id,'comment',d.comment_json,0,d.error,d.conflicting_comment_json
             FROM task_comments c JOIN task_comment_deliveries d ON d.comment_id=c.id
             WHERE c.task_id=?1 AND (d.conflicting_comment_json IS NOT NULL OR (?2 AND d.acknowledged=0))
             ORDER BY c.created_at,c.id", task.as_str(), connected)?;
        append(
            &tx,
            &mut changes,
            "SELECT origin_id,'creation',input,attempted,error,NULL FROM planning_exports
             WHERE kind='task' AND id=?1 AND ?2",
            task.as_str(),
            connected,
        )?;
        tx.commit()?;
        Ok(PlanningSyncStatus { connected, changes })
    }

    pub fn project_planning_sync(&self, project: &ProjectId) -> StoreResult<PlanningSyncStatus> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )?;
        let connected = crate::ops::linear_observe::connected(&repo);
        let mut changes = fields(&tx, "project", project.as_str(), connected, false)?;
        append(
            &tx,
            &mut changes,
            "SELECT origin_id,'creation',input,attempted,error,NULL FROM planning_exports
             WHERE kind='project' AND id=?1 AND ?2",
            project.as_str(),
            connected,
        )?;
        tx.commit()?;
        Ok(PlanningSyncStatus { connected, changes })
    }
}

fn fields(
    conn: &Connection,
    owner: &str,
    id: &str,
    connected: bool,
    order_only: bool,
) -> StoreResult<Vec<PlanningSyncChange>> {
    let mut changes = Vec::new();
    let filter = if order_only {
        "AND c.field='task_order'"
    } else {
        ""
    };
    append(conn, &mut changes, &format!(
        "SELECT c.id,c.field,c.value_json,c.attempted,c.error,
            CASE WHEN c.conflict_json IS NOT NULL THEN json_quote(json_extract(c.conflict_json,'$.value')) END
         FROM {owner}_changes c WHERE c.{owner}_id=?1 {filter} AND
         (c.conflict_json IS NOT NULL OR (?2 AND c.acknowledged=0 AND
            (c.field='deleted' OR (c.field='task_order' AND c.id IN (SELECT id FROM planning_order_deliveries)) OR (c.field!='task_order' AND (c.attempted=1 OR c.seq=(SELECT max(seq) FROM {owner}_changes WHERE {owner}_id=?1 AND field=c.field))))))
         ORDER BY c.seq"), id, connected)?;
    Ok(changes)
}

fn append(
    conn: &Connection,
    changes: &mut Vec<PlanningSyncChange>,
    sql: &str,
    id: &str,
    connected: bool,
) -> StoreResult<()> {
    let mut query = conn.prepare(sql)?;
    for change in query.query_and_then(rusqlite::params![id, connected], read_change)? {
        changes.push(change?);
    }
    Ok(())
}

fn read_change(row: &Row<'_>) -> StoreResult<PlanningSyncChange> {
    let local: String = row.get(2)?;
    let attempted: bool = row.get(3)?;
    let remote: Option<String> = row.get(5)?;
    Ok(PlanningSyncChange {
        id: row.get(0)?,
        field: row.get(1)?,
        state: if remote.is_some() {
            PlanningSyncState::AdoptedLinear
        } else if attempted {
            PlanningSyncState::Uncertain
        } else {
            PlanningSyncState::Pending
        },
        local_value: serde_json::from_str(&local)?,
        linear_value: remote
            .map(|v| serde_json::from_str::<Value>(&v))
            .transpose()?,
        error: row.get(4)?,
    })
}
