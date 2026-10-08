//! Task fields and their delivery evidence commit together, before provider I/O.

use rusqlite::{params, TransactionBehavior};
use serde_json::Value;

use crate::durable::TaskId;
use crate::planning::PlanningChange;
use crate::pm::PmItemUpdate;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::task::Task;

use super::planning_changes::PlanningChanges;
use super::SqliteStore;

impl SqliteStore {
    pub fn pending_task_changes(&self, task: &TaskId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        PlanningChanges::Task(task).pending(&conn)
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
                changed |= PlanningChanges::Task(id).record(&tx, field, previous, value)?;
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
                if PlanningChanges::Task(&TaskId::from_raw(other)).record(
                    &tx,
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
