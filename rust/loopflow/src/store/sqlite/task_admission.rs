//! One permanent first-execution decision on the Task's creating Machine.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{MachineId, TaskAdmission, TaskAdmissionAuthority, TaskId, WorkRef};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) fn authority_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskAdmissionAuthority>> {
    let value: Option<String> = conn.query_row(
        "SELECT admission_authority FROM tasks WHERE id=?1",
        [task.as_str()],
        |row| row.get(0),
    )?;
    value
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(Into::into)
}

pub(super) fn owner_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<MachineId>> {
    let value: Option<String> = conn
        .query_row(
            "SELECT machine_id FROM task_admissions WHERE task_id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    value
        .map(|value| {
            MachineId::parse(&value).map_err(|error| StoreError::InvalidData(error.to_string()))
        })
        .transpose()
}

fn claim_in(conn: &Connection, task: &TaskId, candidate: &MachineId) -> StoreResult<MachineId> {
    if let Some(owner) = owner_in(conn, task)? {
        return Ok(owner);
    }
    let saved = super::children::task_on(conn, task)?.ok_or(StoreError::NotFound)?;
    super::children::require_task_planning(conn, &saved)?;
    if let Some(project) = &saved.project_id {
        super::durable::require_selected_project(conn, project)?;
    }
    if super::durable::task_state_in(conn, task)?.is_terminal() {
        return Err(StoreError::InvalidAuthority(
            "terminal Task cannot reserve first execution".into(),
        ));
    }
    let (started, recorded): (bool, Option<String>) = conn.query_row(
        "SELECT worktree IS NOT NULL OR started_at IS NOT NULL,checkout_machine_id FROM tasks WHERE id=?1",
        [task.as_str()], |row| Ok((row.get(0)?,row.get(1)?)),
    )?;
    if started && recorded.is_none() {
        return Err(StoreError::InvalidAuthority(
            "started Task has unknown execution Machine; admission cannot relocate it".into(),
        ));
    }
    let owner = if let Some(recorded) = recorded {
        MachineId::parse(&recorded).map_err(|error| StoreError::InvalidData(error.to_string()))?
    } else {
        let placement = super::durable::placement_in(conn, &WorkRef::Task(task.clone()))?;
        if placement.provenance != crate::durable::PlacementProvenance::LocalDefault
            && placement.machine_id != *candidate
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Task delegation now selects {}; no admission granted",
                placement.machine_id
            )));
        }
        candidate.clone()
    };
    conn.execute(
        "INSERT INTO task_admissions(task_id,machine_id) VALUES(?1,?2)",
        params![task.as_str(), owner.as_str()],
    )?;
    Ok(owner)
}

/// Library placement shares the same local arbitration; remote authority requires
/// a positively confirmed receipt from the operation before this transaction.
pub(super) fn require_admission_in(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    let Some(authority) = authority_in(conn, task)? else {
        return Ok(());
    };
    let local = super::durable::map_local_machine(conn)?.id;
    let owner = if authority.machine_id == local {
        Some(claim_in(conn, task, &local)?)
    } else {
        owner_in(conn, task)?
    };
    if owner.as_ref() != Some(&local) {
        return Err(StoreError::InvalidAuthority("Task first execution belongs elsewhere or its admission has not been confirmed; no checkout allocated".into()));
    }
    Ok(())
}

impl SqliteStore {
    pub fn task_admission(
        &self,
        task: &TaskId,
        request: &str,
    ) -> StoreResult<Option<TaskAdmission>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let Some(authority) = authority_in(&tx, task)? else {
            return Ok(None);
        };
        Ok(Some(TaskAdmission {
            request: request.into(),
            task_id: task.clone(),
            authority,
            owner: owner_in(&tx, task)?,
        }))
    }

    pub fn claim_task_admission(
        &self,
        task: &TaskId,
        candidate: &MachineId,
        request: &str,
    ) -> StoreResult<TaskAdmission> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let authority = authority_in(&tx, task)?.ok_or_else(|| {
            StoreError::InvalidAuthority("Task has no established first-start authority".into())
        })?;
        if authority.machine_id != super::durable::map_local_machine(&tx)?.id {
            return Err(StoreError::InvalidAuthority(
                "only the Task's creating Machine may grant first execution".into(),
            ));
        }
        let owner = claim_in(&tx, task, candidate)?;
        tx.commit()?;
        Ok(TaskAdmission {
            request: request.into(),
            task_id: task.clone(),
            authority,
            owner: Some(owner),
        })
    }

    pub(crate) fn retain_task_admission(&self, receipt: &TaskAdmission) -> StoreResult<()> {
        let owner = receipt.owner.as_ref().ok_or_else(|| {
            StoreError::InvalidData("an unclaimed reading is not an admission receipt".into())
        })?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if authority_in(&tx, &receipt.task_id)?.as_ref() != Some(&receipt.authority) {
            return Err(StoreError::InvalidAuthority(
                "Task admission authority changed while requesting execution".into(),
            ));
        }
        if let Some(saved) = owner_in(&tx, &receipt.task_id)? {
            if saved != *owner {
                return Err(StoreError::InvalidAuthority(
                    "conflicting Task admission receipts".into(),
                ));
            }
        } else {
            tx.execute(
                "INSERT INTO task_admissions(task_id,machine_id) VALUES(?1,?2)",
                params![receipt.task_id.as_str(), owner.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::SqliteStore;
    use crate::durable::{MachineId, TaskId, WorkRef};
    use crate::store::sqlite::planning_write;
    use crate::work::edit::{WorkEdit, WorkValue};
    use crate::work::task::NewTask;

    fn new_task(store: &SqliteStore, repo: &str) -> TaskId {
        let repo = crate::repository::CanonicalRepo::discover(std::path::Path::new(repo))
            .unwrap()
            .to_string();
        let project = store.ensure_wave_project(&repo, "work").unwrap();
        store
            .create_task(&NewTask {
                repository_id: store.ensure_repository(&repo).unwrap(),
                id: TaskId::new(),
                project_id: Some(project.id),
                title: "One execution".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap()
            .id
    }

    #[test]
    fn simultaneous_admission_and_lost_reply_retry_keep_one_owner() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let task = new_task(&store, home.path().to_str().unwrap());
        let candidates = [MachineId::new(), MachineId::new()];
        let barrier = Arc::new(Barrier::new(2));
        let workers: Vec<_> = candidates
            .iter()
            .map(|candidate| {
                let (path, task, candidate, barrier) = (
                    path.clone(),
                    task.clone(),
                    candidate.clone(),
                    barrier.clone(),
                );
                std::thread::spawn(move || {
                    let peer = SqliteStore::open_ephemeral(&path).unwrap();
                    barrier.wait();
                    peer.claim_task_admission(&task, &candidate, "concurrent")
                        .unwrap()
                })
            })
            .collect();
        let replies: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(replies[0].owner, replies[1].owner);
        let owner = replies[0].owner.as_ref().unwrap();
        assert!(candidates.contains(owner));
        // Neither lost acknowledgement nor a later delegation edit frees the claim.
        let other = MachineId::new();
        store
            .add_machine(&other, "other", "other", "/repo")
            .unwrap();
        let wave = store.task(&task).unwrap().unwrap().wave_id;
        store
            .place_work(&WorkRef::Wave(wave.clone().unwrap()), &other)
            .unwrap();
        let retry = store.claim_task_admission(&task, &other, "retry").unwrap();
        assert_eq!(retry.owner.as_ref(), Some(owner));
        assert_eq!(retry.request, "retry");
        assert_eq!(
            store.task_execution_route(&task).unwrap().machine_id,
            *owner
        );
        assert!(store.task(&task).unwrap().unwrap().worktree.is_none());
        assert!(!store.task_started(&task).unwrap());
        let conn = store.conn.lock().unwrap();
        assert!(conn
            .execute(
                "UPDATE task_admissions SET machine_id=?2 WHERE task_id=?1",
                rusqlite::params![task.as_str(), other.as_str()]
            )
            .is_err());
        assert!(conn
            .execute(
                "DELETE FROM task_admissions WHERE task_id=?1",
                [task.as_str()]
            )
            .is_err());
    }

    #[tokio::test]
    async fn exchange_preserves_authority_but_never_imports_admission() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let source_repo = a.path().canonicalize().unwrap();
        let target_repo = b.path().canonicalize().unwrap();
        let source = SqliteStore::open_ephemeral(&source_repo.as_path().join("db")).unwrap();
        let target = SqliteStore::open_ephemeral(&target_repo.as_path().join("db")).unwrap();
        let task = new_task(&source, source_repo.as_path().to_str().unwrap());
        let destination = crate::planning_git::PlanningDestination::new(
            "/fixture/planning.git",
            "refs/loopflow/planning/shared/admission",
        )
        .unwrap();
        let source_dest = source
            .bind_peer_planning(source_repo.as_path().to_str().unwrap(), &destination)
            .unwrap();
        let target_dest = target
            .bind_peer_planning(target_repo.as_path().to_str().unwrap(), &destination)
            .unwrap();
        let snapshot = source
            .export_peer_planning(source_repo.as_path().to_str().unwrap(), &source_dest)
            .unwrap();
        target
            .import_peer_planning(
                target_repo.as_path().to_str().unwrap(),
                &target_dest,
                "initial",
                &snapshot,
            )
            .unwrap();
        let authority = source
            .task_admission(&task, "source")
            .unwrap()
            .unwrap()
            .authority;
        let imported = target.task_admission(&task, "target").unwrap().unwrap();
        assert_eq!(imported.authority, authority);
        assert!(imported.owner.is_none());
        let local = target.local_machine().unwrap().id;
        assert!(target
            .claim_task_admission(&task, &local, "unauthorized")
            .is_err());
        let unavailable = crate::ops::task_admission::ensure_local(&target, &task)
            .await
            .unwrap_err();
        assert!(
            unavailable.to_string().contains("authority"),
            "{unavailable}"
        );
        assert!(target.task(&task).unwrap().unwrap().worktree.is_none());
        assert!(target
            .task_admission(&task, "unchanged")
            .unwrap()
            .unwrap()
            .owner
            .is_none());

        let receipt = source
            .claim_task_admission(&task, &local, "accepted")
            .unwrap();
        let snapshot = source
            .export_peer_planning(source_repo.as_path().to_str().unwrap(), &source_dest)
            .unwrap();
        target
            .import_peer_planning(
                target_repo.as_path().to_str().unwrap(),
                &target_dest,
                "claimed",
                &snapshot,
            )
            .unwrap();
        assert!(target
            .task_admission(&task, "still-unclaimed")
            .unwrap()
            .unwrap()
            .owner
            .is_none());
        target.retain_task_admission(&receipt).unwrap();
        crate::ops::task_admission::ensure_local(&target, &task)
            .await
            .unwrap();
        assert_eq!(
            target.task_execution_route(&task).unwrap().machine_id,
            local
        );
        let changed = crate::durable::TaskAdmissionAuthority {
            machine_id: local,
            ..authority
        };
        assert!(planning_write::local(
            &target.conn.lock().unwrap(),
            &WorkEdit::for_target(
                task.as_str(),
                &[WorkValue::TaskAdmissionAuthority(Some(changed))]
            )
        )
        .unwrap_err()
        .to_string()
        .contains("immutable"));
    }

    #[test]
    fn delegation_selects_first_owner_but_unknown_started_location_never_moves() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let task = new_task(&store, home.path().to_str().unwrap());
        let local = store.local_machine().unwrap().id;
        let remote = MachineId::new();
        store
            .add_machine(&remote, "remote", "remote", "/repo")
            .unwrap();
        let wave = store.task(&task).unwrap().unwrap().wave_id;
        store
            .place_work(&WorkRef::Wave(wave.clone().unwrap()), &remote)
            .unwrap();
        assert!(store
            .claim_task_admission(&task, &local, "stale")
            .unwrap_err()
            .to_string()
            .contains("delegation"));
        assert_eq!(
            store
                .claim_task_admission(&task, &remote, "current")
                .unwrap()
                .owner,
            Some(remote)
        );
        let started = new_task(&store, home.path().to_str().unwrap());
        store.conn.lock().unwrap().execute("INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,'{\"kind\":\"started\"}',1)", [started.as_str()]).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET started_at=1 WHERE id=?1",
                [started.as_str()],
            )
            .unwrap();
        assert!(store
            .claim_task_admission(&started, &local, "unknown")
            .unwrap_err()
            .to_string()
            .contains("unknown execution Machine"));
    }
}
