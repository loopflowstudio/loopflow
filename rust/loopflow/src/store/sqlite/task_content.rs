//! Task fields and their delivery evidence commit together, before provider I/O.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::TransactionBehavior;

use crate::durable::{ProjectId, TaskId};
use crate::store::planning_sync::PlanningChange;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::task::Task;
use crate::work::task::TaskUpdate;

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
                &crate::work::edit::WorkEdit::for_target(
                    id.as_str(),
                    &[Edit::TaskDeletedAt(Some(now_unix()))],
                ),
            )?
        };
        if changed {
            tx.execute("INSERT INTO task_changes(id,task_id,value_json,base_json)
                SELECT ?1,t.id,'true',CASE WHEN o.id IS NOT NULL THEN json_object('revision',o.revision,'value',json('false')) END
                FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
                LEFT JOIN pm_items o ON o.id=t.external_issue_id AND o.repo=w.repo AND o.provider='linear'
                WHERE t.id=?2", rusqlite::params![uuid::Uuid::new_v4().to_string(),id.as_str()])?;
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
        let current = validate_refile(&tx, id, expected_project, destination)?;
        if current.project_id == *destination {
            return Ok(());
        }
        PlanningChanges::Task(id).save(&tx, &[Edit::TaskProject(destination.to_string())])?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn validate_task_refile(
        &self,
        id: &TaskId,
        expected_project: &ProjectId,
        destination: &ProjectId,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.transaction()?;
        validate_refile(&snapshot, id, expected_project, destination).map(|_| ())
    }

    pub(crate) fn validate_task_edit(
        &self,
        id: &TaskId,
        expected_revision: u64,
        patch: &TaskUpdate,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        validate_edit(&conn, id, expected_revision, patch).map(|_| ())
    }

    pub(crate) fn require_task_not_deleted(&self, id: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let task = super::children::task_on(&conn, id)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_not_deleted(&conn, &task)
    }

    pub fn edit_task(
        &self,
        id: &TaskId,
        expected_revision: u64,
        patch: &TaskUpdate,
    ) -> StoreResult<Task> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = validate_edit(&tx, id, expected_revision, patch)?;
        let mut edits = Vec::new();
        if let Some(value) = &patch.title {
            edits.push(Edit::TaskTitle(Some(value.clone())));
        }
        if let Some(value) = &patch.description {
            edits.push(Edit::TaskDescription(Some(value.clone())));
        }
        if let Some(value) = &patch.assignee {
            edits.push(Edit::TaskAssignee(value.clone()));
        }
        PlanningChanges::Task(id).save(&tx, &edits)?;
        if let Some(rank) = patch.rank {
            super::planning_order::reorder_in(&tx, &task.project_id, id, rank)?;
        }
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }
}

fn validate_edit(
    conn: &rusqlite::Connection,
    id: &TaskId,
    expected_revision: u64,
    patch: &TaskUpdate,
) -> StoreResult<Task> {
    let task = super::children::task_on(conn, id)?.ok_or(StoreError::NotFound)?;
    super::children::require_task_not_deleted(conn, &task)?;
    if task.revision != expected_revision {
        return Err(StoreError::InvalidAuthority(
            "Task changed; read its current revision before editing".into(),
        ));
    }
    if patch
        .title
        .as_ref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(StoreError::InvalidData("Task title cannot be empty".into()));
    }
    Ok(task)
}

fn validate_refile(
    conn: &rusqlite::Connection,
    id: &TaskId,
    expected_project: &ProjectId,
    destination: &ProjectId,
) -> StoreResult<Task> {
    let current = super::children::task_on(conn, id)?.ok_or(StoreError::NotFound)?;
    super::children::require_task_not_deleted(conn, &current)?;
    if current.project_id == *destination {
        return Ok(current);
    }
    if current.project_id != *expected_project {
        return Err(StoreError::InvalidAuthority(
            "Task membership changed before refiling".into(),
        ));
    }
    super::durable::require_selected_project(conn, destination)?;
    let unallocated: bool = conn.query_row(
        &format!(
            "SELECT worktree IS NULL AND started_at IS NULL AND abandon_requested_at IS NULL
         AND NOT EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1)
         AND NOT EXISTS(SELECT 1 FROM task_workflows WHERE task_id=?1)
         AND NOT EXISTS(SELECT 1 FROM task_prs WHERE task_id=?1)
         AND NOT EXISTS({}) FROM tasks WHERE id=?1",
            super::task_work::lf_process_ids(conn, "?1")?
        ),
        [id.as_str()],
        |row| row.get(0),
    )?;
    if !unallocated {
        return Err(StoreError::InvalidAuthority(
            "a Task with recorded work retains its owning Wave".into(),
        ));
    }
    Ok(current)
}
