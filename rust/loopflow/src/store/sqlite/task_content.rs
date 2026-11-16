//! Task fields and their delivery evidence commit together, before provider I/O.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::TransactionBehavior;

use crate::durable::{ProjectId, TaskId};
use crate::store::planning_sync::{PlanningChange, PlanningChangeRef};
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
        let current = match validate_move(&tx, id, expected_project, destination) {
            Ok(current) => current,
            Err(error) => {
                super::operations::refuse_current_in(
                    &tx,
                    "task.membership_changed",
                    &[id.as_str()],
                    &["planning"],
                    &error,
                )?;
                tx.commit()?;
                return Err(error);
            }
        };
        let changes = if current.project_id.as_ref() != destination {
            PlanningChanges::Task(id).save(
                &tx,
                &[Edit::TaskProject(destination.map(ToString::to_string))],
            )?
        } else {
            Vec::new()
        };
        commit_edit_in(&tx, id, "task.membership_changed", changes)?;
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
        let task = match validate_edit(&tx, id, expected_revision, patch) {
            Ok(task) => task,
            Err(error) => {
                super::operations::refuse_current_in(
                    &tx,
                    "task.updated",
                    &[id.as_str()],
                    &["planning"],
                    &error,
                )?;
                tx.commit()?;
                return Err(error);
            }
        };
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
        let mut changes = PlanningChanges::Task(id).save(&tx, &edits)?;
        if let Some(rank) = patch.rank {
            changes.extend(super::planning_order::reorder_in(&tx, &task, rank)?);
        }
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        commit_edit_in(&tx, id, "task.updated", changes)?;
        tx.commit()?;
        Ok(task)
    }
}

fn commit_edit_in(
    conn: &rusqlite::Connection,
    id: &TaskId,
    stage: &str,
    changes: Vec<PlanningChangeRef>,
) -> StoreResult<()> {
    if let Some(operation) = crate::journal::request::operation_id() {
        let sync = super::planning_sync::task_planning_syncs_in(conn, std::slice::from_ref(id))?
            .remove(id)
            .ok_or(StoreError::NotFound)?;
        let mut result = serde_json::to_value(crate::ops::task::TaskEdit {
            id: id.to_string(),
            sync,
        })?;
        if stage == "task.membership_changed" {
            let task = super::children::task_on(conn, id)?.ok_or(StoreError::NotFound)?;
            result["wave_id"] = serde_json::to_value(task.wave_id)?;
            result["project_id"] = serde_json::to_value(task.project_id)?;
        }
        super::operations::commit_planning_edit_in(
            conn,
            &operation,
            stage,
            id.as_str(),
            changes,
            &result,
        )?;
    }
    Ok(())
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
    fn rank_recovery_names_the_project_delivery_in_the_same_edit_commit() {
        use crate::store::planning_sync::PlanningChangeKind;
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repository = home.path().canonicalize().unwrap();
        let repository = repository.to_str().unwrap();
        let repository_id = store.ensure_repository(repository).unwrap();
        let project = store.ensure_wave_project(repository, "product").unwrap();
        let mut tasks = Vec::new();
        for title in ["First", "Second"] {
            tasks.push(
                store
                    .create_task(&NewTask {
                        id: TaskId::new(),
                        repository_id: repository_id.clone(),
                        project_id: Some(project.id.clone()),
                        title: title.into(),
                        description: String::new(),
                        due_date: None,
                    })
                    .unwrap(),
            );
        }
        let task = &tasks[1];
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "rank",
                "task.update",
                "hash",
                &serde_json::json!({}),
            )
            .unwrap();
        crate::journal::request::with_operation(Some("rank".into()), || {
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
        });
        let receipt = store.operation_receipt("rank").unwrap().unwrap();
        let changes = &receipt.commits[0].planning_changes;
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, PlanningChangeKind::Project);
        assert_eq!(changes[0].owner, project.id.as_str());
        assert_eq!(receipt.planning.unwrap().changes[0].change, changes[0]);
    }

    #[test]
    fn planning_recovery_tracks_the_exact_change_across_newer_edits_and_restart() {
        use super::super::operations::PlanningDeliveryState;
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let repository_id = store.ensure_repository("/planning-recovery").unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id,
                project_id: None,
                title: "Original".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let epoch = store.reading_identity().unwrap().1;
        // Provider completion is held. Each operation owns just its new change,
        // even when the Task has older outstanding delivery receipts.
        for (operation, title) in [("older", "First"), ("newer", "Second")] {
            store
                .admit_operation(
                    &epoch,
                    operation,
                    "task.update",
                    "hash",
                    &serde_json::json!({}),
                )
                .unwrap();
            store.dispatch_operation(operation).unwrap();
            let revision = store.task(&task.id).unwrap().unwrap().revision;
            crate::journal::request::with_operation(Some(operation.into()), || {
                store
                    .edit_task(
                        &task.id,
                        revision,
                        &TaskUpdate {
                            title: Some(title.into()),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            });
            let receipt = store.operation_receipt(operation).unwrap().unwrap();
            assert_eq!(receipt.commits[0].planning_changes.len(), 1);
            assert_eq!(
                receipt.planning.as_ref().unwrap().changes[0].state,
                PlanningDeliveryState::Pending
            );
            let change = &receipt.commits[0].planning_changes[0];
            if operation == "older" {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE task_changes SET attempted=1,error='reply lost' WHERE id=?1",
                        [&change.id],
                    )
                    .unwrap();
            }
            store
                .finish_operation(operation, Err("output lost"))
                .unwrap();
        }
        let older = store.operation_receipt("older").unwrap().unwrap();
        let original = older.commits[0].result.clone();
        let saved_change = older.commits[0].planning_changes[0].clone();
        assert_eq!(
            older.planning.unwrap().changes[0].state,
            PlanningDeliveryState::Uncertain
        );
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert!(store
            .recover_operation_result("older", "task.updated")
            .unwrap());
        let recovered = store.operation_receipt("older").unwrap().unwrap();
        assert_eq!(recovered.result.as_ref(), Some(&original));
        assert_eq!(
            recovered.planning.as_ref().unwrap().changes[0].change,
            saved_change
        );
        // The old provider acknowledgment resolves only that delivery. It does
        // not overwrite the second edit or certify the newer delivery.
        let subscription_revision = store.operation_revision().unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE task_changes SET acknowledged=1,error=NULL WHERE id=?1",
                [&saved_change.id],
            )
            .unwrap();
        assert!(store.operation_revision().unwrap() > subscription_revision);
        let delivered = store.operation_receipt("older").unwrap().unwrap();
        assert!(delivered.revision > recovered.revision);
        assert_eq!(delivered.result, Some(original));
        assert_eq!(delivered.error.as_deref(), Some("output lost"));
        let current = delivered.planning.unwrap();
        assert_eq!(current.changes[0].state, PlanningDeliveryState::Delivered);
        assert!(current.changes[0].error.is_none());
        assert_eq!(current.fence.epoch, epoch);
        assert_eq!(
            current.fence.revisions["planning"],
            store.revisions().unwrap().planning
        );
        assert_eq!(store.task(&task.id).unwrap().unwrap().title, "Second");
        let newer = store.operation_receipt("newer").unwrap().unwrap();
        assert_eq!(
            newer.planning.unwrap().changes[0].state,
            PlanningDeliveryState::Pending
        );
        // Missing evidence stays unavailable, never inferred from the Task value.
        store
            .conn
            .lock()
            .unwrap()
            .execute("DELETE FROM task_changes WHERE id=?1", [&saved_change.id])
            .unwrap();
        assert_eq!(
            store
                .operation_receipt("older")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            PlanningDeliveryState::Unavailable
        );
    }

    #[test]
    fn superseded_unsent_delivery_clears_only_its_current_failure() {
        use super::super::operations::PlanningDeliveryState;
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repository_id = store.ensure_repository("/planning-recovery").unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id,
                project_id: None,
                title: "Original".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "saved",
                "task.update",
                "hash",
                &serde_json::json!({}),
            )
            .unwrap();
        crate::journal::request::with_operation(Some("saved".into()), || {
            store
                .edit_task(
                    &task.id,
                    task.revision,
                    &TaskUpdate {
                        title: Some("First".into()),
                        ..Default::default()
                    },
                )
                .unwrap();
        });
        let saved = store.operation_receipt("saved").unwrap().unwrap();
        let change = &saved.commits[0].planning_changes[0];
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE task_changes SET error='offline' WHERE id=?1",
                [&change.id],
            )
            .unwrap();
        let pending = store.operation_receipt("saved").unwrap().unwrap();
        assert_eq!(
            pending.planning.unwrap().changes[0].error.as_deref(),
            Some("offline")
        );
        store
            .edit_task(
                &task.id,
                store.task(&task.id).unwrap().unwrap().revision,
                &TaskUpdate {
                    title: Some("Second".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let superseded = store.operation_receipt("saved").unwrap().unwrap();
        assert!(superseded.revision > pending.revision);
        let current = superseded.planning.unwrap();
        assert_eq!(current.changes[0].state, PlanningDeliveryState::Superseded);
        assert!(current.changes[0].error.is_none());
        // A conflict is separate from both delivery and supersession.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE task_changes SET conflict_json='{}' WHERE id=?1",
                [&change.id],
            )
            .unwrap();
        assert_eq!(
            store
                .operation_receipt("saved")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            PlanningDeliveryState::Conflict
        );
        assert_eq!(
            store.operation_receipt("saved").unwrap().unwrap().commits,
            saved.commits
        );
    }

    #[test]
    fn validation_refusal_resolves_only_the_uncommitted_edit_and_never_supplies_success() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repository = home.path().canonicalize().unwrap();
        let repository = repository.to_str().unwrap();
        let repository_id = store.ensure_repository(repository).unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id,
                project_id: None,
                title: "Current".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let expected = store.ensure_wave_project(repository, "expected").unwrap();
        let destination = store
            .ensure_wave_project(repository, "destination")
            .unwrap();
        let epoch = store.reading_identity().unwrap().1;
        for (id, kind, stage) in [
            ("refused-edit", "task.update", "task.updated"),
            ("refused-move", "task.move", "task.membership_changed"),
        ] {
            store
                .admit_operation(&epoch, id, kind, "hash", &serde_json::json!({}))
                .unwrap();
            assert!(store.dispatch_operation(id).unwrap());
            let before = store.revisions().unwrap();
            crate::journal::request::with_operation(Some(id.into()), || {
                if kind == "task.update" {
                    assert!(store
                        .edit_task(
                            &task.id,
                            task.revision + 1,
                            &TaskUpdate {
                                title: Some("Rejected".into()),
                                ..Default::default()
                            }
                        )
                        .is_err());
                } else {
                    assert!(store
                        .move_task(&task.id, Some(&expected.id), Some(&destination.id))
                        .is_err());
                }
            });
            let receipt = store.operation_receipt(id).unwrap().unwrap();
            assert_eq!(receipt.commits.len(), 1);
            let resolution = &receipt.commits[0];
            assert_eq!(resolution.stage, stage);
            assert_eq!(
                resolution.resolution,
                super::super::operations::OperationResolution::DefinitelyRefused
            );
            assert_eq!(resolution.resources, [task.id.to_string()]);
            assert!(resolution.result["error"].is_string());
            assert_eq!(resolution.fence.revisions["planning"], before.planning);
            assert!(!store.recover_operation_result(id, stage).unwrap());
            assert_eq!(store.revisions().unwrap(), before);
            assert!(store.pending_task_changes(&task.id).unwrap().is_empty());
        }
        assert_eq!(store.task(&task.id).unwrap().unwrap().title, "Current");
        assert!(store.task(&task.id).unwrap().unwrap().project_id.is_none());
    }

    #[test]
    fn edit_and_membership_fences_cover_noops_and_roll_back_with_their_facts() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repository = home.path().canonicalize().unwrap();
        let repository = repository.to_str().unwrap();
        let repository_id = store.ensure_repository(repository).unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id,
                project_id: None,
                title: "Original".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let epoch = store.reading_identity().unwrap().1;
        for (id, kind, stage) in [
            ("noop-edit", "task.update", "task.updated"),
            ("noop-move", "task.move", "task.membership_changed"),
        ] {
            store
                .admit_operation(&epoch, id, kind, "hash", &serde_json::json!({}))
                .unwrap();
            let before = store.revisions().unwrap().planning;
            crate::journal::request::with_operation(Some(id.into()), || {
                if kind == "task.update" {
                    store
                        .edit_task(
                            &task.id,
                            task.revision,
                            &TaskUpdate {
                                title: Some(task.title.clone()),
                                ..Default::default()
                            },
                        )
                        .unwrap();
                } else {
                    store.move_task(&task.id, None, None).unwrap();
                }
            });
            let receipt = store.operation_receipt(id).unwrap().unwrap();
            assert_eq!(receipt.commits.len(), 1);
            assert_eq!(receipt.commits[0].stage, stage);
            assert_eq!(receipt.commits[0].fence.revisions["planning"], before);
            assert_eq!(store.revisions().unwrap().planning, before);
        }
        let project = store.ensure_wave_project(repository, "product").unwrap();
        for (id, kind) in [("failed-edit", "task.update"), ("failed-move", "task.move")] {
            store
                .admit_operation(&epoch, id, kind, "hash", &serde_json::json!({}))
                .unwrap();
            // Fail the receipt write after the domain write. Both must roll back.
            store.conn.lock().unwrap().execute_batch(
                "CREATE TEMP TRIGGER reject_commit BEFORE UPDATE OF commits ON creation_receipts
                 BEGIN SELECT RAISE(ABORT,'held receipt write'); END;"
            ).unwrap();
            let before = store.revisions().unwrap();
            crate::journal::request::with_operation(Some(id.into()), || {
                if kind == "task.update" {
                    assert!(store
                        .edit_task(
                            &task.id,
                            task.revision,
                            &TaskUpdate {
                                title: Some("Must roll back".into()),
                                ..Default::default()
                            }
                        )
                        .is_err());
                } else {
                    assert!(store.move_task(&task.id, None, Some(&project.id)).is_err());
                }
            });
            store
                .conn
                .lock()
                .unwrap()
                .execute_batch("DROP TRIGGER reject_commit")
                .unwrap();
            assert_eq!(store.revisions().unwrap(), before);
            assert!(store
                .operation_receipt(id)
                .unwrap()
                .unwrap()
                .commits
                .is_empty());
            let current = store.task(&task.id).unwrap().unwrap();
            assert_eq!(current.title, task.title);
            assert_eq!(current.project_id, None);
            assert!(store.pending_task_changes(&task.id).unwrap().is_empty());
        }
    }

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
                None,
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
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "move",
                "task.move",
                "hash",
                &serde_json::json!({}),
            )
            .unwrap();
        crate::journal::request::with_operation(Some("move".into()), || {
            store.move_task(&task.id, None, Some(&product.id)).unwrap();
        });
        let committed = store.operation_receipt("move").unwrap().unwrap();
        assert_eq!(
            committed.commits[0].result["wave_id"],
            serde_json::json!(product.wave_id)
        );
        assert_eq!(
            committed.commits[0].result["project_id"],
            serde_json::json!(product.id)
        );
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
                None,
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
                None,
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
