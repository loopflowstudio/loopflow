//! What a Project or Task has waiting for its tracker, and what the tracker
//! won, read from the effect queue.
use std::collections::HashMap;

use rusqlite::{Connection, Row};
use serde_json::Value;

use super::children::ids_json;
use super::SqliteStore;
use crate::planning_sync::{PlanningSyncChange, PlanningSyncReceipts, PlanningSyncState};
use crate::StoreResult;
use loopflow_model::durable::{ProjectId, TaskId};

impl SqliteStore {
    pub fn task_planning_receipts(&self, task: &TaskId) -> StoreResult<PlanningSyncReceipts> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let status = task_planning_receipts_in(&tx, std::slice::from_ref(task))?
            .remove(task)
            .ok_or(crate::StoreError::NotFound)?;
        tx.commit()?;
        Ok(status)
    }

    pub fn project_planning_receipts(
        &self,
        project: &ProjectId,
    ) -> StoreResult<PlanningSyncReceipts> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )?;
        let ids = ids_json([project.as_str()]);
        let changes = effects(&tx, "project_id", &ids, false)?;
        tx.commit()?;
        Ok(PlanningSyncReceipts {
            repo,
            changes: changes.into_iter().map(|(_, change)| change).collect(),
        })
    }
}

/// Each Task's receipts, in one statement per kind across all repositories.
pub(super) fn task_planning_receipts_in(
    conn: &Connection,
    tasks: &[TaskId],
) -> StoreResult<HashMap<TaskId, PlanningSyncReceipts>> {
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
    let task_ids = ids_json(owners.iter().map(|(task, _, _)| task.as_str()));
    let project_ids = ids_json(
        owners
            .iter()
            .filter_map(|(_, _, project)| project.as_deref()),
    );
    let mut task_fields = by_owner(effects(conn, "task_id", &task_ids, false)?);
    let project_fields = by_owner(effects(conn, "project_id", &project_ids, true)?);
    let mut statuses = HashMap::new();
    for (task, repo, project) in owners {
        let mut changes = task_fields.remove(&task).unwrap_or_default();
        if let Some(changes_in_project) = project
            .as_ref()
            .and_then(|project| project_fields.get(project))
        {
            changes.extend(changes_in_project.iter().cloned());
        }
        statuses.insert(
            TaskId::from_raw(task),
            PlanningSyncReceipts { repo, changes },
        );
    }
    Ok(statuses)
}

fn by_owner(
    changes: Vec<(String, PlanningSyncChange)>,
) -> HashMap<String, Vec<PlanningSyncChange>> {
    let mut owners = HashMap::<_, Vec<_>>::new();
    for (owner, change) in changes {
        owners.entry(owner).or_default().push(change);
    }
    owners
}

/// Unsettled effects of every owner in `ids` (a JSON array), and each
/// subject's latest conflict until a later save of it settles, with owners.
fn effects(
    conn: &Connection,
    owner: &str,
    ids: &str,
    order_only: bool,
) -> StoreResult<Vec<(String, PlanningSyncChange)>> {
    let mut changes = Vec::new();
    let filter = if order_only { "AND e.kind='order'" } else { "" };
    append(conn, &mut changes, &format!(
        "SELECT e.id,
            CASE e.kind WHEN 'field' THEN e.subject WHEN 'create' THEN 'creation' WHEN 'delete' THEN 'deleted'
                WHEN 'order' THEN 'task_order' ELSE e.kind END,
            CASE e.kind WHEN 'field' THEN json_quote(json_extract(e.value_json,'$.value'))
                WHEN 'state' THEN json_quote(json_extract(e.value_json,'$.value.planning_state'))
                WHEN 'comment' THEN json_extract(e.value_json,'$.value')
                WHEN 'delete' THEN 'true' ELSE e.value_json END,
            e.phase='attempted',e.error,
            CASE WHEN e.conflict_json IS NOT NULL THEN CASE e.kind
                WHEN 'field' THEN json_quote(json_extract(e.conflict_json,'$.value.value'))
                WHEN 'state' THEN json_quote(json_extract(e.conflict_json,'$.value.value.planning_state'))
                WHEN 'comment' THEN json_extract(e.conflict_json,'$.value')
                ELSE json_quote(json_extract(e.conflict_json,'$.value')) END END,
            e.{owner}
         FROM tracker_effects e WHERE e.{owner} IN (SELECT value FROM json_each(?1)) {filter} AND
         (e.phase IN ('pending','attempted') OR (e.conflict_json IS NOT NULL AND NOT EXISTS(
             SELECT 1 FROM tracker_effects later WHERE later.{owner}=e.{owner} AND later.kind=e.kind
             AND later.subject IS e.subject AND later.seq>e.seq AND later.phase='acknowledged')))
         ORDER BY e.seq"), ids)?;
    Ok(changes)
}

/// Rows of `sql` (owner in column 6) bound to `ids`.
fn append(
    conn: &Connection,
    changes: &mut Vec<(String, PlanningSyncChange)>,
    sql: &str,
    ids: &str,
) -> StoreResult<()> {
    let mut query = conn.prepare(sql)?;
    for change in query.query_and_then([ids], |row| {
        Ok::<_, crate::StoreError>((row.get::<_, String>(6)?, read_change(row)?))
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
