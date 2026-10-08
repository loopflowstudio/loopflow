//! Task fields and their delivery evidence commit together, before provider I/O.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::durable::TaskId;
use crate::planning::PlanningChange;
use crate::pm::{PmItem, PmItemUpdate};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::task::Task;

use super::SqliteStore;

fn record_change(
    conn: &Connection,
    task: &TaskId,
    field: &str,
    previous: Value,
    value: Value,
) -> StoreResult<bool> {
    if previous == value {
        return Ok(false);
    }
    let body: Option<String> = conn
        .query_row(
            "SELECT i.body FROM tasks t JOIN projects p ON p.id=t.project_id
         JOIN waves w ON w.id=p.wave_id
         JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
         WHERE t.id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let base = body
        .map(|body| -> StoreResult<Value> {
            let body: Value = serde_json::from_str(&body)?;
            Ok(serde_json::json!({"revision":body["revision"],"value":body[field]}))
        })
        .transpose()?;
    conn.execute(
        "INSERT INTO task_changes(id,task_id,field,value_json,base_json) VALUES(?1,?2,?3,?4,?5)",
        params![
            uuid::Uuid::new_v4().to_string(),
            task.as_str(),
            field,
            value.to_string(),
            base.map(|v| v.to_string())
        ],
    )?;
    Ok(true)
}

pub(super) fn retain_edits(
    conn: &Connection,
    task: &TaskId,
    observed: &PmItem,
) -> StoreResult<PmItem> {
    let mut saved = serde_json::to_value(observed)?;
    for change in pending_in(conn, task)? {
        let remote = saved[&change.field].clone();
        if remote != change.value
            && change
                .base
                .as_ref()
                .is_none_or(|base| base["value"] != remote)
        {
            conn.execute(
                "UPDATE task_changes SET conflict_json=?2 WHERE id=?1 AND conflict_json IS NULL",
                params![
                    change.id,
                    serde_json::json!({"revision":observed.revision,"value":remote}).to_string()
                ],
            )?;
        }
        saved[&change.field] = change.value;
    }
    Ok(serde_json::from_value(saved)?)
}

fn pending_in(conn: &Connection, task: &TaskId) -> StoreResult<Vec<PlanningChange>> {
    let mut query = conn.prepare(
        "SELECT id,field,value_json,base_json,conflict_json FROM task_changes c
         WHERE task_id=?1 AND acknowledged=0 AND seq=(SELECT max(seq) FROM task_changes
             WHERE task_id=c.task_id AND field=c.field) ORDER BY seq",
    )?;
    let rows = query.query_map([task.as_str()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, field, value, base, conflict) = row?;
        Ok(PlanningChange {
            id,
            field,
            value: serde_json::from_str(&value)?,
            base: base.map(|v| serde_json::from_str(&v)).transpose()?,
            conflict: conflict.map(|v| serde_json::from_str(&v)).transpose()?,
        })
    })
    .collect()
}

impl SqliteStore {
    pub fn pending_task_changes(&self, task: &TaskId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        pending_in(&conn, task)
    }

    pub fn edit_task(
        &self,
        id: &TaskId,
        expected_revision: u64,
        patch: &PmItemUpdate,
    ) -> StoreResult<Task> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_not_deleted(&tx, &task)?;
        if task.plan.revision != expected_revision {
            return Err(StoreError::InvalidAuthority(
                "Task changed; read its current revision before editing".into(),
            ));
        }
        if patch
            .name
            .as_ref()
            .is_some_and(|title| title.trim().is_empty())
        {
            return Err(StoreError::InvalidData("Task title cannot be empty".into()));
        }
        let current = super::plan_read::task_in(&tx, id)?
            .record
            .ok_or(StoreError::NotFound)?
            .item;
        let mut changed = false;
        for (field, previous, value) in [
            (
                "name",
                Value::String(current.name),
                patch.name.as_ref().map(|v| Value::String(v.clone())),
            ),
            (
                "description",
                Value::String(current.description),
                patch.description.as_ref().map(|v| Value::String(v.clone())),
            ),
            (
                "assignee",
                serde_json::to_value(current.assignee)?,
                patch
                    .assignee
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?,
            ),
        ] {
            if let Some(value) = value {
                changed |= record_change(&tx, id, field, previous, value)?;
            }
        }
        if changed {
            tx.execute(
                "UPDATE tasks SET issue_title=COALESCE(?2,issue_title),issue_description=COALESCE(?3,issue_description),
                 planning_assignee=CASE WHEN ?4 THEN ?5 ELSE planning_assignee END,
                 planning_revision=planning_revision+1,updated_at=?6 WHERE id=?1",
                params![id.as_str(),patch.name,patch.description,patch.assignee.is_some(),patch.assignee.as_ref().and_then(|v|v.as_deref()),now_unix()],
            )?;
        }
        if let Some(rank) = patch.rank {
            let mut query = tx.prepare(
                "SELECT id,planning_rank FROM tasks WHERE project_id=?1 AND planning_deleted_at IS NULL
                 ORDER BY planning_rank,created_at,id",
            )?;
            let mut ordered = query
                .query_map([task.project_id.as_str()], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            ordered.retain(|(other, _)| other != id.as_str());
            ordered.insert(
                (rank as usize).min(ordered.len()),
                (id.to_string(), current.rank),
            );
            for (rank, (other, previous)) in ordered.iter().enumerate() {
                if record_change(
                    &tx,
                    &TaskId::from_raw(other),
                    "rank",
                    serde_json::json!(previous),
                    serde_json::json!(rank),
                )? {
                    tx.execute(
                        "UPDATE tasks SET planning_rank=?2,planning_revision=planning_revision+1,updated_at=?3 WHERE id=?1",
                        params![other,rank as u32,now_unix()],
                    )?;
                }
            }
        }
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }
}
