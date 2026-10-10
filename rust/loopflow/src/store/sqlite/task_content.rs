//! Task fields and their delivery evidence commit together, before provider I/O.

use super::planning_write::{self, PlanningEdit as Edit};
use crate::engine::planning_exchange::PlanningKind;
use rusqlite::TransactionBehavior;
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
        let deleted: bool = tx.query_row(
            "SELECT planning_deleted_at IS NOT NULL FROM tasks WHERE id=?1",
            [id.as_str()],
            |r| r.get(0),
        )?;
        let changed = if deleted {
            false
        } else {
            planning_write::local(
                &tx,
                PlanningKind::Task,
                id.as_str(),
                &[Edit::TaskDeletedAt(Some(now_unix()))],
            )?
        };
        if changed {
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
        let available: bool = tx.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND worktree IS NULL AND started_at IS NULL AND abandon_requested_at IS NULL
             AND NOT EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1)
             AND NOT EXISTS(SELECT 1 FROM task_workflows WHERE task_id=?1)
             AND NOT EXISTS(SELECT 1 FROM task_prs WHERE task_id=?1)
             AND NOT EXISTS({}))", super::task_work::process_lfids("?1")),
            [id.as_str()],|r|r.get(0))?;
        if !available {
            return Err(StoreError::InvalidAuthority(
                "a Task with recorded work retains its owning Wave".into(),
            ));
        }
        planning_write::local(
            &tx,
            PlanningKind::Task,
            id.as_str(),
            &[Edit::TaskProject(destination.to_string())],
        )?;
        PlanningChanges::Task(id).record(
            &tx,
            "project_id",
            serde_json::json!(current.project_id),
            serde_json::json!(destination),
        )?;
        super::durable::inherit_task_placement(
            &tx,
            &super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?,
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
            let mut edits = Vec::new();
            if let Some(value) = &patch.name {
                edits.push(Edit::TaskTitle(Some(value.clone())));
            }
            if let Some(value) = &patch.description {
                edits.push(Edit::TaskDescription(Some(value.clone())));
            }
            if let Some(value) = &patch.assignee {
                edits.push(Edit::TaskAssignee(value.clone()));
            }
            planning_write::local(&tx, PlanningKind::Task, id.as_str(), &edits)?;
        }
        if let Some(rank) = patch.rank {
            super::planning_order::reorder_in(&tx, &task.project_id, id, rank)?;
        }
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }
}
