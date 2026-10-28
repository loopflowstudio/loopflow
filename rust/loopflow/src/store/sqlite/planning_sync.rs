//! Project and Task delivery presentation comes from the existing receipts.
use std::collections::HashMap;

use rusqlite::{Connection, Row};
use serde_json::Value;

use super::children::ids_json;
use super::SqliteStore;
use crate::durable::{ProjectId, TaskId};
use crate::store::planning_sync::{PlanningSyncChange, PlanningSyncState, PlanningSyncStatus};
use crate::store::StoreResult;

impl SqliteStore {
    pub fn task_planning_sync(&self, task: &TaskId) -> StoreResult<PlanningSyncStatus> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let status = task_planning_syncs_in(&tx, std::slice::from_ref(task))?
            .remove(task)
            .ok_or(crate::store::StoreError::NotFound)?;
        tx.commit()?;
        Ok(status)
    }

    pub fn project_planning_sync(&self, project: &ProjectId) -> StoreResult<PlanningSyncStatus> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )?;
        let connected = crate::ops::planning_client::connected(&repo);
        let ids = ids_json([project.as_str()]);
        let mut changes = fields(&tx, "project", &ids, connected, false)?;
        append(&tx, &mut changes,
            "SELECT c.successor_id,'creation',COALESCE(json_extract(c.export_json,'$.input'),c.local_plan_json),c.export_attempted OR c.export_link_attempted,c.export_error,NULL,p.id
             FROM project_transitions c JOIN projects p ON p.id=c.successor_id
             WHERE p.id IN (SELECT value FROM json_each(?1)) AND ?2 AND p.external_project_id IS NULL AND c.local_plan_json IS NOT NULL",
            &ids, connected)?;
        tx.commit()?;
        Ok(PlanningSyncStatus {
            connected,
            changes: changes.into_iter().map(|(_, change)| change).collect(),
        })
    }
}

/// Each Task's delivery presentation, in one statement per receipt kind for
/// each planning connection state (connection is a per-repository fact).
pub(super) fn task_planning_syncs_in(
    conn: &Connection,
    tasks: &[TaskId],
) -> StoreResult<HashMap<TaskId, PlanningSyncStatus>> {
    let ids = ids_json(tasks.iter().map(TaskId::as_str));
    let mut statement = conn.prepare(
        "SELECT t.id,w.repo,p.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
         LEFT JOIN projects p ON p.id=t.project_id WHERE t.id IN (SELECT value FROM json_each(?1))",
    )?;
    let owners = statement
        .query_map([ids], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut connected_repos = HashMap::new();
    let mut groups: HashMap<bool, Vec<(String, Option<String>)>> = HashMap::new();
    for (task, repo, project) in owners {
        let connected = *connected_repos
            .entry(repo.clone())
            .or_insert_with(|| crate::ops::planning_client::connected(&repo));
        groups.entry(connected).or_default().push((task, project));
    }
    let mut statuses = HashMap::new();
    for (connected, group) in groups {
        let task_ids = ids_json(group.iter().map(|(task, _)| task.as_str()));
        let project_ids = ids_json(group.iter().filter_map(|(_, project)| project.as_deref()));
        let task_fields = fields(conn, "task", &task_ids, connected, false)?;
        let project_fields = fields(conn, "project", &project_ids, connected, true)?;
        let mut receipts = Vec::new();
        append(conn, &mut receipts,
            "SELECT d.id,'state',json_quote(d.target),d.attempted,d.error,
                CASE WHEN d.conflict_json IS NOT NULL THEN json_quote(json_extract(d.conflict_json,'$.state')) END,d.task_id
             FROM task_state_deliveries d WHERE d.task_id IN (SELECT value FROM json_each(?1)) AND
             (d.conflict_json IS NOT NULL OR (?2 AND d.settled=0 AND
                (d.attempted=1 OR d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=d.task_id)))) ORDER BY d.seq",
            &task_ids, connected)?;
        append(conn, &mut receipts,
            "SELECT d.comment_id,'comment',d.comment_json,0,d.error,d.conflicting_comment_json,c.task_id
             FROM task_comments c JOIN task_comment_deliveries d ON d.comment_id=c.id
             WHERE c.task_id IN (SELECT value FROM json_each(?1)) AND (d.conflicting_comment_json IS NOT NULL OR (?2 AND d.acknowledged=0))
             ORDER BY c.created_at,c.id", &task_ids, connected)?;
        append(conn, &mut receipts,
            "SELECT c.task_id,'creation',COALESCE(json_extract(c.export_json,'$.input'),json_object('title',c.title,'description',c.description)),
                c.export_attempted,c.export_error,NULL,c.task_id FROM task_creation_intents c
             JOIN tasks t ON t.id=c.task_id WHERE c.task_id IN (SELECT value FROM json_each(?1)) AND ?2 AND t.external_issue_id IS NULL
             AND (t.planning_deleted_at IS NULL OR c.export_attempted=1)", &task_ids, connected)?;
        for (task, project) in group {
            let mut changes = owned(&task_fields, &task);
            if let Some(project) = &project {
                changes.extend(owned(&project_fields, project));
            }
            changes.extend(owned(&receipts, &task));
            statuses.insert(
                TaskId::from_raw(task),
                PlanningSyncStatus { connected, changes },
            );
        }
    }
    Ok(statuses)
}

fn owned(changes: &[(String, PlanningSyncChange)], owner: &str) -> Vec<PlanningSyncChange> {
    changes
        .iter()
        .filter(|(id, _)| id == owner)
        .map(|(_, change)| change.clone())
        .collect()
}

/// Field changes of every owner in `ids` (a JSON array), each with its owner.
fn fields(
    conn: &Connection,
    owner: &str,
    ids: &str,
    connected: bool,
    order_only: bool,
) -> StoreResult<Vec<(String, PlanningSyncChange)>> {
    let mut changes = Vec::new();
    let filter = if order_only {
        "AND c.field='task_order'"
    } else {
        ""
    };
    append(conn, &mut changes, &format!(
        "SELECT c.id,c.field,
            CASE WHEN c.field IN ('deleted','task_order') THEN c.value_json ELSE json_quote(json_extract(c.value_json,'$.value')) END,c.attempted,c.error,
            CASE WHEN c.conflict_json IS NOT NULL THEN CASE WHEN c.field IN ('deleted','task_order') THEN json_quote(json_extract(c.conflict_json,'$.value')) ELSE json_quote(json_extract(c.conflict_json,'$.value.value')) END END,
            c.{owner}_id
         FROM {owner}_changes c WHERE c.{owner}_id IN (SELECT value FROM json_each(?1)) {filter} AND
         (c.conflict_json IS NOT NULL OR (?2 AND c.acknowledged=0 AND
            (c.attempted=1 OR c.seq=(SELECT max(seq) FROM {owner}_changes WHERE {owner}_id=c.{owner}_id AND field=c.field))))
         ORDER BY c.seq"), ids, connected)?;
    Ok(changes)
}

/// Rows of `sql` (owner in column 6) bound to `ids` and `connected`.
fn append(
    conn: &Connection,
    changes: &mut Vec<(String, PlanningSyncChange)>,
    sql: &str,
    ids: &str,
    connected: bool,
) -> StoreResult<()> {
    let mut query = conn.prepare(sql)?;
    for change in query.query_and_then(rusqlite::params![ids, connected], |row| {
        Ok::<_, crate::store::StoreError>((row.get::<_, String>(6)?, read_change(row)?))
    })? {
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
