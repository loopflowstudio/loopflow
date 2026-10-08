//! Task fields and their delivery evidence commit together, before provider I/O.

use rusqlite::{params, TransactionBehavior};
use serde_json::Value;

use crate::durable::{ProjectId, TaskId};
use crate::planning::PlanningChange;
use crate::pm::PmItemUpdate;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::task::Task;

use super::planning_changes::PlanningChanges;
use super::SqliteStore;

impl SqliteStore {
    /// The tombstone and pending effect are one save; retries retain the first identity.
    pub fn delete_task(&self, id: &TaskId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        let changed = tx.execute("UPDATE tasks SET planning_deleted_at=?2,planning_revision=planning_revision+1,updated_at=?2
            WHERE id=?1 AND planning_deleted_at IS NULL", params![id.as_str(), now_unix()])?;
        if changed == 1 {
            PlanningChanges::Task(id).record(
                &tx,
                "deleted",
                serde_json::json!(false),
                serde_json::json!(true),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn pending_task_changes(&self, task: &TaskId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        PlanningChanges::Task(task).pending(&conn)
    }

    /// Refile only unallocated work; membership and its pending effect commit together.
    pub(crate) fn refile_unplaced_task(
        &self,
        id: &TaskId,
        expected_project: &ProjectId,
        destination: &ProjectId,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_not_deleted(&tx, &current)?;
        if current.project_id == *destination {
            return Ok(());
        }
        if current.project_id != *expected_project {
            return Err(StoreError::InvalidAuthority(
                "Task membership changed before refiling".into(),
            ));
        }
        super::durable::require_selected_project(&tx, destination)?;
        let changed = tx.execute(
            &format!("UPDATE tasks SET project_id=?2,planning_revision=planning_revision+1,updated_at=?3
             WHERE id=?1 AND worktree IS NULL AND started_at IS NULL AND abandon_requested_at IS NULL
             AND NOT EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1)
             AND NOT EXISTS(SELECT 1 FROM task_workflows WHERE task_id=?1)
             AND NOT EXISTS(SELECT 1 FROM task_prs WHERE task_id=?1)
             AND NOT EXISTS({})", super::task_work::process_lfids("?1")),
            params![id.as_str(), destination.as_str(), now_unix()],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidAuthority(
                "a Task with recorded work retains its owning Wave".into(),
            ));
        }
        PlanningChanges::Task(id).record(
            &tx,
            "project_id",
            serde_json::json!(current.project_id),
            serde_json::json!(destination),
        )?;
        tx.commit()?;
        Ok(())
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
            super::planning_order::reorder_in(&tx, &task.project_id, id, rank)?;
        }
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }
}
