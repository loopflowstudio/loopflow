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
                FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
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

    /// Membership and its pending effect commit together without transferring execution.
    pub(crate) fn move_task(
        &self,
        id: &TaskId,
        expected_project: Option<&ProjectId>,
        destination: Option<&ProjectId>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = validate_move(&tx, id, expected_project, destination)?;
        if current.project_id.as_ref() == destination {
            return Ok(());
        }
        PlanningChanges::Task(id).save(
            &tx,
            &[Edit::TaskProject(destination.map(ToString::to_string))],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn validate_task_move(
        &self,
        id: &TaskId,
        expected_project: Option<&ProjectId>,
        destination: Option<&ProjectId>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        validate_move(&snapshot, id, expected_project, destination).map(|_| ())
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
        // The caller labels the Linear issue first; this copy serves local display.
        if let Some(value) = &patch.regressed_in {
            super::planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    id.as_str(),
                    &[Edit::TaskRegressedIn(Some(value.clone()))],
                ),
            )?;
        }
        if let Some(rank) = patch.rank {
            super::planning_order::reorder_in(&tx, &task, rank)?;
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
    if let Some(destination) = destination {
        super::durable::require_selected_project(conn, destination)?;
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
    use crate::durable::TaskId;
    use crate::session_record::new_artifact_key;
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::{NewTask, TaskUpdate};

    #[test]
    fn rank_edits_stay_within_repository_and_project_membership() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let path = path.to_str().unwrap();
        let repo = store.ensure_repository(path).unwrap();
        let other_repo = store
            .ensure_repository(home.path().to_str().unwrap())
            .unwrap();
        let project = store.ensure_wave_project(path, "product").unwrap();
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
    fn membership_moves_preserve_work_and_capture_owners() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repo = tempfile::tempdir().unwrap();
        let foreign_repo = tempfile::tempdir().unwrap();
        let canonical = repo.path().canonicalize().unwrap();
        let path = canonical.to_str().unwrap();
        let repository = store.ensure_repository(path).unwrap();
        let product = store.ensure_wave_project(path, "product").unwrap();
        let other = store.ensure_wave_project(path, "other").unwrap();
        let foreign = store
            .ensure_wave_project(foreign_repo.path().to_str().unwrap(), "foreign")
            .unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id: repository,
                project_id: None,
                title: "One Task".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let session = store.test_session("conversation", &new_artifact_key());
        let bound = store
            .bind_session(&session.id, session.captured, &task.id)
            .unwrap();
        let captured = store
            .replace_session_input(
                bound.captured,
                crate::session::LfSession {
                    artifact_key: Some(new_artifact_key()),
                    ..bound
                },
            )
            .unwrap();
        let before = store.session_history(&session.id, 0, 0).unwrap();
        {
            let _snapshot = store.read_snapshot().unwrap();
            store
                .validate_task_move(&task.id, None, Some(&product.id))
                .unwrap();
            assert!(store.task(&task.id).unwrap().unwrap().project_id.is_none());
        }
        store.move_task(&task.id, None, Some(&product.id)).unwrap();
        assert_eq!(store.session_history(&session.id, 0, 0).unwrap(), before);
        assert_eq!(
            store.session(&session.id).unwrap().unwrap().wave_id,
            Some(product.wave_id.clone())
        );
        // A stale attachment's inherited Wave hint cannot rewrite current membership.
        let next = store
            .replace_session_input(
                captured.captured,
                crate::session::LfSession {
                    artifact_key: Some(new_artifact_key()),
                    ..captured
                },
            )
            .unwrap();
        assert_eq!(next.wave_id, Some(product.wave_id.clone()));
        assert!(store.move_task(&task.id, None, Some(&other.id)).is_err());
        assert!(store
            .move_task(&task.id, Some(&product.id), Some(&foreign.id))
            .is_err());
        store
            .move_task(&task.id, Some(&product.id), Some(&product.id))
            .unwrap();
        store.move_task(&task.id, Some(&product.id), None).unwrap();
        let final_input = store
            .replace_session_input(
                next.captured,
                crate::session::LfSession {
                    artifact_key: Some(new_artifact_key()),
                    ..next
                },
            )
            .unwrap();
        assert!(final_input.wave_id.is_none());
        let conn = store.conn.lock().unwrap();
        let owners = conn.prepare("SELECT wave_id FROM session_events WHERE session_id='conversation' AND kind='captured' ORDER BY seq").unwrap()
            .query_map([], |row| row.get::<_, Option<String>>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(
            owners,
            vec![None, None, Some(product.wave_id.to_string()), None]
        );
        drop(conn);
        let retained = store.task(&task.id).unwrap().unwrap();
        assert_eq!(retained.repository_id, task.repository_id);
        assert!(retained.project_id.is_none());
        assert_eq!(
            store.bound_sessions().unwrap()[0].session_id,
            session.id.as_str()
        );
    }
}
