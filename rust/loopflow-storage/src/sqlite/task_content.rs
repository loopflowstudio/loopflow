//! Task fields and the tracker actions they ask for commit together, before provider I/O.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::TransactionBehavior;

use crate::planning_sync::PlanningChange;
use crate::rows::now_unix;
use crate::work::task::Task;
use crate::work::task::TaskUpdate;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{ProjectId, TaskId};

use super::SqliteStore;

impl SqliteStore {
    /// Record an upstream disposition without changing the Task's local execution.
    pub fn record_task_disposition(
        &self,
        task: &loopflow_model::durable::TaskId,
        disposition: crate::work::edit::Disposition,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        planning_write::tracker(
            &tx,
            &crate::work::edit::WorkEdit::for_target(
                task.as_str(),
                &[crate::work::edit::WorkValue::Disposition(disposition)],
            ),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The tombstone and its `delete` action are one save; a repeat changes nothing.
    pub fn delete_task(&self, id: &TaskId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        let deleted: bool = tx.query_row(
            "SELECT planning_deleted_at IS NOT NULL FROM tasks WHERE id=?1",
            [id.as_str()],
            |r| r.get(0),
        )?;
        if !deleted {
            planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    id.as_str(),
                    &[Edit::TaskDeletedAt(Some(now_unix()))],
                ),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn pending_task_changes(&self, task: &TaskId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::tracker_actions::queued_changes_in(
            &conn,
            &super::tracker_actions::ActionOwner::Task(task.clone()),
        )
    }

    /// Membership and its pending effect commit together without transferring execution.
    pub fn move_task(
        &self,
        id: &TaskId,
        expected_project: Option<&ProjectId>,
        destination: Option<&ProjectId>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        move_task_in(&tx, id, expected_project, destination)?;
        tx.commit()?;
        Ok(())
    }

    pub fn validate_task_move(
        &self,
        id: &TaskId,
        expected_project: Option<&ProjectId>,
        destination: Option<&ProjectId>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        validate_move(&snapshot, id, expected_project, destination).map(|_| ())
    }

    pub fn validate_task_edit(
        &self,
        id: &TaskId,
        expected_revision: u64,
        patch: &TaskUpdate,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        validate_edit(&conn, id, expected_revision, patch).map(|_| ())
    }

    pub fn require_task_not_deleted(&self, id: &TaskId) -> StoreResult<()> {
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
        if let Some(value) = &patch.parent {
            edits.push(Edit::TaskParent(
                value.as_ref().map(|parent| parent.as_str().to_owned()),
            ));
        }
        super::planning_write::local(
            &tx,
            &crate::work::edit::WorkEdit::for_target(id.as_str(), &edits),
        )
        .map(|_| ())?;
        if let Some(rank) = patch.rank {
            super::planning_order::reorder_in(&tx, &task, rank)?;
        }
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(task)
    }

    /// `id` takes `to`'s place, moving into `to`'s Project when it is in
    /// another; returns that Project's order.
    pub fn reorder_task(&self, id: &TaskId, to: &TaskId) -> StoreResult<Vec<String>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        // The moved Task is the one that is missing; a missing place is a bad argument.
        let target = super::children::task_on(&tx, to)?
            .ok_or_else(|| StoreError::InvalidData(format!("{to} is no Task; nothing changed.")))?;
        super::children::require_task_not_deleted(&tx, &task)?;
        super::children::require_task_not_deleted(&tx, &target)?;
        let order = super::planning_order::move_in(&tx, &task, &target)?;
        tx.commit()?;
        Ok(order)
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
    if let Some(Some(parent)) = &patch.parent {
        if let Some(refusal) =
            super::planning_write::parent_refusal(conn, id.as_str(), parent.as_str())?
        {
            return Err(StoreError::InvalidAuthority(refusal.into()));
        }
    }
    Ok(task)
}

/// Move the Task into `destination`, as one planning edit its tracker sends.
pub(super) fn move_task_in(
    conn: &rusqlite::Connection,
    id: &TaskId,
    expected_project: Option<&ProjectId>,
    destination: Option<&ProjectId>,
) -> StoreResult<()> {
    let current = validate_move(conn, id, expected_project, destination)?;
    if current.project_id.as_ref() == destination {
        return Ok(());
    }
    super::planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            id.as_str(),
            &[Edit::TaskProject(destination.map(ToString::to_string))],
        ),
    )
    .map(|_| ())
}

fn validate_move(
    conn: &rusqlite::Connection,
    id: &TaskId,
    expected_project: Option<&ProjectId>,
    destination: Option<&ProjectId>,
) -> StoreResult<Task> {
    let current = super::children::task_on(conn, id)?.ok_or(StoreError::NotFound)?;
    super::children::require_task_not_deleted(conn, &current)?;
    if current.project_id.as_ref() == destination {
        return Ok(current);
    }
    if current.project_id.as_ref() != expected_project {
        return Err(StoreError::InvalidAuthority(
            "Task membership changed before moving".into(),
        ));
    }
    // A move names its Project, so any of the repository's Projects will do;
    // only new work lands in the Wave's configured one.
    if let Some(destination) = destination {
        let same_repo: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo WHERE p.id=?1 AND r.id=?2)",
            rusqlite::params![destination.as_str(), current.repository_id.as_str()], |row| row.get(0),
        )?;
        if !same_repo {
            return Err(StoreError::InvalidAuthority(
                "Task moves stay in one repository".into(),
            ));
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {

    use crate::work::task::{NewTask, TaskUpdate};
    use loopflow_model::durable::TaskId;

    #[test]
    fn rank_edits_stay_within_repository_and_project_membership() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("db")).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let path = path.to_str().unwrap();
        let repo = crate::test_fixture::repository(&store, path);
        let other_repo = crate::test_fixture::repository(&store, home.path().to_str().unwrap());
        let project = crate::test_fixture::project(&store, path, "product");
        let groups: Vec<Vec<_>> = [
            (repo.clone(), None),
            (repo, Some(project.id)),
            (other_repo, None),
        ]
        .into_iter()
        .map(|(repository_id, project_id)| {
            (0..3)
                .map(|index| {
                    store
                        .create_task(&NewTask {
                            parent_task_id: None,
                            id: TaskId::new(),
                            repository_id: repository_id.clone(),
                            project_id: project_id.clone(),
                            title: format!("Task {index}"),
                            description: String::new(),
                            due_date: None,
                        })
                        .unwrap()
                })
                .collect()
        })
        .collect();
        for (selected, group) in groups.iter().enumerate() {
            let task = &group[2];
            store
                .edit_task(
                    &task.id,
                    task.revision,
                    &TaskUpdate {
                        rank: Some(0),
                        ..Default::default()
                    },
                )
                .unwrap();
            for (index, group) in groups.iter().enumerate() {
                let ranks: Vec<_> = group
                    .iter()
                    .map(|task| store.task(&task.id).unwrap().unwrap().rank)
                    .collect();
                assert_eq!(
                    ranks,
                    if index <= selected {
                        vec![1, 2, 0]
                    } else {
                        vec![0, 1, 2]
                    }
                );
            }
            let current = store.task(&task.id).unwrap().unwrap();
            let before = store.revisions().unwrap();
            store
                .edit_task(
                    &task.id,
                    current.revision,
                    &TaskUpdate {
                        rank: Some(0),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(store.revisions().unwrap(), before);
        }
    }
    #[test]
    fn upstream_disposition_keeps_local_execution() {
        let (_home, store, task) = crate::test_fixture::store_with_task();
        let before = store.task(&task).unwrap().unwrap();
        store.conn.lock().unwrap().execute(
            "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,'{\"name\":\"feature\",\"steps\":[]}','start',1)",
            [task.as_str()],
        ).unwrap();
        store
            .record_task_disposition(
                &task,
                crate::work::edit::Disposition {
                    planning_state: Some("canceled".into()),
                    planning_completed: 0,
                    planning_completed_at: None,
                },
            )
            .unwrap();
        let after = store.task(&task).unwrap().unwrap();
        assert_eq!(after.state.as_deref(), Some("canceled"));
        assert_eq!(after.worktree, before.worktree);
        assert_eq!(after.branch, before.branch);
        let graph = store.task_graph_row(&task).unwrap().unwrap();
        assert_eq!(graph.step, "start");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&graph.graph).unwrap()["name"],
            "feature"
        );
    }
}
