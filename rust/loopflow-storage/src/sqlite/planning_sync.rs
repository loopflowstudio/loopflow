//! Project and Task delivery presentation comes from the existing receipts.
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
        let mut changes = fields(&tx, "project", &ids, false)?;
        append(&tx, &mut changes,
            "SELECT c.successor_id,'creation',COALESCE(json_extract(c.export_json,'$.input'),c.local_plan_json),c.export_attempted OR c.export_link_attempted,c.export_error,NULL,p.id
             FROM project_transitions c JOIN projects p ON p.id=c.successor_id
             WHERE p.id IN (SELECT value FROM json_each(?1)) AND p.external_project_id IS NULL AND c.local_plan_json IS NOT NULL",
            &ids)?;
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
    let mut task_fields = by_owner(fields(conn, "task", &task_ids, false)?);
    let project_fields = by_owner(fields(conn, "project", &project_ids, true)?);
    let mut receipts = Vec::new();
    append(conn, &mut receipts,
            "SELECT d.id,'state',json_quote(d.target),d.attempted,d.error,
                CASE WHEN d.conflict_json IS NOT NULL THEN json_quote(json_extract(d.conflict_json,'$.state')) END,d.task_id
             FROM task_state_deliveries d WHERE d.task_id IN (SELECT value FROM json_each(?1)) AND
             (d.conflict_json IS NOT NULL OR (d.settled=0 AND
                (d.attempted=1 OR d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=d.task_id)))) ORDER BY d.seq",
            &task_ids)?;
    append(conn, &mut receipts,
            "SELECT d.comment_id,'comment',d.comment_json,0,d.error,d.conflicting_comment_json,c.task_id
             FROM task_comments c JOIN task_comment_deliveries d ON d.comment_id=c.id
             WHERE c.task_id IN (SELECT value FROM json_each(?1)) AND (d.conflicting_comment_json IS NOT NULL OR (d.acknowledged=0))
             ORDER BY c.created_at,c.id", &task_ids)?;
    append(conn, &mut receipts,
            "SELECT c.task_id,'creation',COALESCE(json_extract(c.export_json,'$.input'),json_object('title',c.title,'description',c.description)),
                c.export_attempted,c.export_error,NULL,c.task_id FROM task_creation_intents c
             JOIN tasks t ON t.id=c.task_id WHERE c.task_id IN (SELECT value FROM json_each(?1)) AND t.external_issue_id IS NULL
             AND (t.planning_deleted_at IS NULL OR c.export_attempted=1)", &task_ids)?;
    let mut receipts = by_owner(receipts);
    let mut statuses = HashMap::new();
    for (task, repo, project) in owners {
        let mut changes = task_fields.remove(&task).unwrap_or_default();
        if let Some(changes_in_project) = project
            .as_ref()
            .and_then(|project| project_fields.get(project))
        {
            changes.extend(changes_in_project.iter().cloned());
        }
        changes.extend(receipts.remove(&task).unwrap_or_default());
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

/// Field changes of every owner in `ids` (a JSON array), each with its owner.
fn fields(
    conn: &Connection,
    owner: &str,
    ids: &str,
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
         (c.conflict_json IS NOT NULL OR (c.acknowledged=0 AND
            (c.attempted=1 OR c.seq=(SELECT max(seq) FROM {owner}_changes WHERE {owner}_id=c.{owner}_id AND field=c.field))))
         ORDER BY c.seq"), ids)?;
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
