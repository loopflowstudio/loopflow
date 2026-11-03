//! Local-only Git planning. Provider attempts and execution never enter this store.
use super::{
    planning_write::{self, WriteOrigin},
    SqliteStore,
};
use crate::planning_exchange::{PlanningMutation, PlanningSnapshot};
use crate::planning_git::PlanningDestination;
use crate::store::{
    PeerPlanningStatus, PeerProjectionConflict, PlanningJournal, StoreError, StoreResult,
};
use crate::work::edit::WorkValue;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

impl SqliteStore {
    pub(crate) fn record_peer_fetch(
        &self,
        repo: &str,
        destination: &str,
        revision: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE planning_destinations SET fetched_revision=?3
            WHERE repo=?1 AND id=?2 AND fetched_revision IS NOT ?3",
            params![repo, destination, revision],
        )?;
        Ok(())
    }
    pub(crate) fn record_peer_error(
        &self,
        repo: &str,
        destination: &str,
        acquisition: bool,
        error: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let column = if acquisition {
            "acquisition_error"
        } else {
            "publication_error"
        };
        conn.execute(
            &format!(
                "UPDATE planning_destinations SET {column}=?3
                WHERE repo=?1 AND id=?2 AND {column} IS NOT ?3"
            ),
            params![repo, destination, error],
        )?;
        Ok(())
    }
    pub(crate) fn record_peer_publication(
        &self,
        repo: &str,
        destination: &str,
        revision: &str,
        state: &str,
        snapshot: &PlanningSnapshot,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE planning_destinations SET publication_revision=?3,publication_state=?4,
                publication_digest=?5,publication_error=NULL
            WHERE repo=?1 AND id=?2 AND (publication_revision IS NOT ?3
                OR publication_state IS NOT ?4 OR publication_digest IS NOT ?5
                OR publication_error IS NOT NULL)",
            params![
                repo,
                destination,
                revision,
                state,
                planning_digest(snapshot)?
            ],
        )?;
        Ok(())
    }
    pub fn provision_planning_user_key(&self, key: &str) -> StoreResult<()> {
        let id = uuid::Uuid::parse_str(key).map_err(invalid)?;
        if id.to_string() != key {
            return Err(invalid("use a canonical planning user UUID"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved: Option<String> = tx
            .query_row("SELECT user_key FROM planning_user", [], |r| r.get(0))
            .optional()?;
        match saved {
            Some(saved) if saved != key => {
                return Err(invalid(
                    "planning user key already provisioned; retain existing plan bindings",
                ))
            }
            Some(_) => {}
            None => {
                tx.execute(
                    "INSERT INTO planning_user(singleton,user_key) VALUES(1,?1)",
                    [key],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn planning_user_key(&self) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row("SELECT user_key FROM planning_user", [], |r| r.get(0))
            .optional()
            .map_err(Into::into)
    }
    pub fn bind_peer_planning(
        &self,
        repo: &str,
        destination: &PlanningDestination,
    ) -> StoreResult<String> {
        PlanningDestination::new(destination.endpoint(), destination.reference())
            .map_err(invalid)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(key) = destination
            .reference()
            .strip_prefix("refs/loopflow/planning/users/")
        {
            let saved: Option<String> = tx
                .query_row("SELECT user_key FROM planning_user", [], |r| r.get(0))
                .optional()?;
            if saved.as_deref() != Some(key) {
                return Err(invalid(
                    "recover the planning user key before binding its plan",
                ));
            }
        }
        let id = destination.id();
        tx.execute(
            "INSERT INTO planning_destinations(repo,id,endpoint,reference) VALUES(?1,?2,?3,?4)
            ON CONFLICT(repo,id) DO NOTHING",
            params![repo, id, destination.endpoint(), destination.reference()],
        )?;
        tx.commit()?;
        Ok(id)
    }
    pub(crate) fn peer_planning_destination(
        &self,
        repo: &str,
        id: &str,
    ) -> StoreResult<Option<PlanningDestination>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT endpoint,reference FROM planning_destinations WHERE repo=?1 AND id=?2",
                params![repo, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(endpoint, reference)| {
            PlanningDestination::new(&endpoint, &reference).map_err(invalid)
        })
        .transpose()
    }
    pub fn export_peer_planning(
        &self,
        repo: &str,
        destination: &str,
    ) -> StoreResult<PlanningSnapshot> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_destination(&tx, repo, destination)?;
        route_repository(&tx, repo, destination)?;
        let snapshot = export_in(&tx, repo, destination, true)?;
        tx.commit()?;
        Ok(snapshot)
    }
    pub fn import_peer_planning(
        &self,
        repo: &str,
        destination: &str,
        revision: &str,
        incoming: &PlanningSnapshot,
    ) -> StoreResult<()> {
        let incoming = incoming.canonicalized().map_err(invalid)?;
        incoming.validate().map_err(invalid)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let mut tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_destination(&tx, repo, destination)?;
        route_repository(&tx, repo, destination)?;
        let merged = export_in(&tx, repo, destination, true)?
            .merge(&incoming)
            .map_err(invalid)?;
        for (id, change) in &incoming.changes {
            planning_write::retain(&tx, id, change)?;
            tx.execute(
                "INSERT OR IGNORE INTO planning_peer_routes(mutation_id,repo,destination) VALUES(?1,?2,?3)",
                params![id, repo, destination],
            )?;
        }
        let edits: Vec<_> = merged
            .winners()
            .map(|(_, change)| change.edit.clone())
            .collect();
        let grouped = planning_write::grouped(&edits);
        let mut pending: Vec<_> = grouped
            .iter()
            .map(|(record, edits)| (*record, edits))
            .collect();
        let mut conflicts = BTreeMap::new();
        let mut inserted = std::collections::HashSet::new();
        while !pending.is_empty() {
            let mut retry = Vec::new();
            let mut progress = false;
            conflicts.clear();
            for ((table, target), edits) in pending {
                let save = tx.savepoint()?;
                let result = (|| {
                    let created = planning_write::ensure_record(&save, repo, table, target, edits)?;
                    if !planning_write::read(&save, table, target)?
                        .iter()
                        .all(|value| {
                            edits
                                .iter()
                                .any(|edit| edit.change.field() == value.field())
                        })
                    {
                        return Err(invalid("incomplete planning record"));
                    }
                    require_repository(&save, repo, table, target)?;
                    // Wave selection points to Projects inserted later in this import.
                    let selected: Vec<_> = edits
                        .iter()
                        .filter(|e| !matches!(e.change, WorkValue::WaveProject(_)))
                        .cloned()
                        .collect();
                    planning_write::write(&save, &selected, WriteOrigin::Import)?;
                    require_repository(&save, repo, table, target)?;
                    Ok::<_, StoreError>(created)
                })();
                match result {
                    Ok(created) => {
                        save.commit()?;
                        if created {
                            inserted.insert((table, target));
                        }
                        progress = true;
                    }
                    Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ref e, _)))
                        if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        save.finish()?;
                        conflicts.insert(
                            (table, target),
                            "Planning parent or identity conflicts with retained local work"
                                .to_owned(),
                        );
                        retry.push(((table, target), edits));
                    }
                    Err(error) => return Err(error),
                }
            }
            if !progress {
                break;
            }
            pending = retry;
        }
        let heads = merged.heads_by_key();
        for (&(table, target), edits) in &grouped {
            if conflicts.contains_key(&(table, target)) {
                continue;
            }
            for edit in edits
                .iter()
                .filter(|e| matches!(e.change, WorkValue::WaveProject(_)))
            {
                let save = tx.savepoint()?;
                match planning_write::write(&save, std::slice::from_ref(edit), WriteOrigin::Import)
                {
                    Ok(_) => save.commit()?,
                    Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ref e, _)))
                        if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        save.finish()?;
                        conflicts.insert(
                            (table, target),
                            "Selected Project is not available locally".into(),
                        );
                    }
                    Err(StoreError::InvalidAuthority(reason)) => {
                        save.finish()?;
                        conflicts.insert((table, target), reason);
                    }
                    Err(e) => return Err(e),
                }
            }
            for edit in edits {
                if matches!(edit.change, WorkValue::WaveProject(_))
                    && conflicts.contains_key(&(table, target))
                {
                    continue;
                }
                planning_write::observe(
                    &tx,
                    edit,
                    heads.get(&edit.key()).into_iter().flatten().copied(),
                )?;
            }
        }
        // Ownership entries ride on marked comments. An owner whose open claim
        // the exchanged log no longer shows claims again. A teammate's newly
        // arrived direction comment steers the Task as a local one would.
        let mut touched = std::collections::HashSet::new();
        for &(table, target) in grouped.keys() {
            if table != "task_comments" || conflicts.contains_key(&(table, target)) {
                continue;
            }
            let (task, body, author, created_at): (String, String, String, Option<String>) = tx
                .query_row(
                    "SELECT task_id,body,author,created_at FROM task_comments WHERE id=?1",
                    [target],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;
            let task = crate::durable::TaskId::from_raw(task);
            super::task_ownership::ingest_marker_in(&tx, &task, &body, created_at.as_deref())?;
            if inserted.contains(&(table, target)) {
                let comment = crate::work::task::comment::TaskComment {
                    id: target.to_string(),
                    author: serde_json::from_str(&author)?,
                    body,
                    created_at,
                };
                super::task_comments::comment_steer_in(&tx, &task, &comment)?;
            }
            touched.insert(task);
        }
        for task in &touched {
            super::task_ownership::reassert_in(&tx, task)?;
        }

        tx.execute(
            "UPDATE planning_peer_conflicts SET active=0 WHERE repo=?1",
            [repo],
        )?;
        for ((table, target), reason) in conflicts {
            tx.execute("INSERT INTO planning_peer_conflicts(repo,kind,object_id,reason,active) VALUES(?1,?2,?3,?4,1) ON CONFLICT(kind,object_id,reason) DO UPDATE SET active=1",params![repo,table,target,reason])?;
        }
        tx.execute("INSERT INTO planning_peer_imports(repo,destination,revision) VALUES(?1,?2,?3) ON CONFLICT(repo,destination) DO UPDATE SET revision=excluded.revision WHERE revision IS NOT excluded.revision",params![repo,destination,revision])?;
        tx.commit()?;
        Ok(())
    }
    pub fn peer_planning_status(
        &self,
        repo: &str,
        active: Option<&str>,
    ) -> StoreResult<Vec<PeerPlanningStatus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query=conn.prepare("SELECT d.id,d.reference,d.id IS ?2,d.fetched_revision,d.acquisition_error,d.publication_revision,d.publication_state,d.publication_error,d.publication_digest,i.revision FROM planning_destinations d LEFT JOIN planning_peer_imports i ON i.repo=d.repo AND i.destination=d.id WHERE d.repo=?1 ORDER BY d.id")?;
        let rows = query.query_map(params![repo, active], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (
                id,
                reference,
                active,
                fetched_revision,
                acquisition_error,
                publication_revision,
                publication_state,
                publication_error,
                digest,
                imported_revision,
            ) = row?;
            let snapshot = export_in(&conn, repo, &id, active)?;
            let pending_local = digest.as_deref() != Some(planning_digest(&snapshot)?.as_str());
            let selected_records = snapshot
                .changes
                .values()
                .map(|change| {
                    (
                        planning_write::table(&change.edit.change),
                        change.edit.target.as_str(),
                    )
                })
                .collect::<BTreeSet<_>>()
                .len() as u64;
            let mut conflicts = conn.prepare(
                "SELECT object_id,reason FROM planning_peer_conflicts WHERE repo=?1 AND active=1",
            )?;
            let conflicts = conflicts
                .query_and_then([repo], |row| -> StoreResult<_> {
                    Ok(PeerProjectionConflict {
                        target: row.get(0)?,
                        reason: row.get(1)?,
                    })
                })?
                .collect::<StoreResult<Vec<_>>>()?;
            result.push(PeerPlanningStatus {
                id,
                reference,
                active,
                selected_records,
                imported_revision,
                fetched_revision,
                acquisition_error,
                publication_revision,
                publication_state,
                publication_error,
                pending_local,
                conflicts,
            });
        }
        Ok(result)
    }
    pub fn peer_planning_journal(
        &self,
        repo: &str,
        active: Option<&str>,
    ) -> StoreResult<Vec<PlanningJournal>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let destinations = conn
            .prepare("SELECT id FROM planning_destinations WHERE repo=?1 ORDER BY id")?
            .query_map([repo], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        destinations
            .into_iter()
            .map(|destination| {
                let snapshot = export_in(
                    &conn,
                    repo,
                    &destination,
                    Some(destination.as_str()) == active,
                )?;
                Ok(PlanningJournal {
                    candidates: snapshot.winners().map(|(id, _)| id.to_owned()).collect(),
                    changes: snapshot.changes,
                    destination,
                })
            })
            .collect()
    }
}

fn planning_digest(snapshot: &PlanningSnapshot) -> StoreResult<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(snapshot.to_bytes().map_err(invalid)?)
    ))
}
fn require_destination(conn: &Connection, repo: &str, destination: &str) -> StoreResult<()> {
    if !conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM planning_destinations WHERE repo=?1 AND id=?2)",
        params![repo, destination],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(invalid("planning destination is missing"));
    }
    Ok(())
}
fn require_repository(conn: &Connection, repo: &str, table: &str, target: &str) -> StoreResult<()> {
    let sql = match table {
        "waves" => "SELECT repo FROM waves WHERE id=?1",
        "projects" => "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
        "tasks" => "SELECT w.repo FROM tasks t JOIN repository_plans w ON w.id=t.repository_id WHERE t.id=?1",
        "task_comments" => "SELECT w.repo FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN repository_plans w ON w.id=t.repository_id WHERE c.id=?1",
        _ => return Err(invalid("unknown Work table")),
    };
    let actual: String = conn.query_row(sql, [target], |row| row.get(0))?;
    if actual != repo {
        return Err(invalid("planning record belongs to another repository"));
    }
    Ok(())
}

// Current ownership selects new exports. Retained routes keep prior destinations,
// tombstones and rejected records readable after ownership or configuration changes.
const OWNED_CHANGES: &str = "WITH owned(record_table,object_id) AS (
    SELECT 'waves',id FROM waves WHERE repo=?1
    UNION ALL SELECT 'projects',p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1
    UNION ALL SELECT 'tasks',t.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id WHERE w.repo=?1
    UNION ALL SELECT 'task_comments',c.id FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN repository_plans w ON w.id=t.repository_id WHERE w.repo=?1
)";

fn route_repository(conn: &Connection, repo: &str, destination: &str) -> StoreResult<()> {
    conn.execute(
        &format!(
            "{OWNED_CHANGES}
            INSERT OR IGNORE INTO planning_peer_routes(mutation_id,repo,destination)
            SELECT c.id,?1,?2 FROM planning_peer_changes c
            JOIN owned o ON o.record_table=c.record_table AND o.object_id=c.object_id"
        ),
        params![repo, destination],
    )?;
    Ok(())
}
fn export_in(
    conn: &Connection,
    repo: &str,
    destination: &str,
    include_current: bool,
) -> StoreResult<PlanningSnapshot> {
    let mut query = conn.prepare(
        &format!("{OWNED_CHANGES}
            SELECT c.id,c.mutation FROM planning_peer_changes c
            WHERE c.id IN (SELECT mutation_id FROM planning_peer_routes WHERE repo=?1 AND destination=?2)
            OR (?3 AND EXISTS(SELECT 1 FROM owned o WHERE o.record_table=c.record_table AND o.object_id=c.object_id))"),
    )?;
    let changes = query
        .query_and_then(
            params![repo, destination, include_current],
            |row| -> StoreResult<_> {
                Ok((
                    row.get(0)?,
                    serde_json::from_str::<PlanningMutation>(&row.get::<_, String>(1)?)?,
                ))
            },
        )?
        .collect::<StoreResult<BTreeMap<_, _>>>()?;
    let snapshot = PlanningSnapshot { changes };
    snapshot.validate().map_err(invalid)?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::id::WaveId;
    use crate::planning_exchange::{PlanningMutation, PlanningSnapshot};
    use crate::planning_git::PlanningDestination;
    use crate::store::sqlite::{planning_write, SqliteStore};
    use crate::work::edit::{WorkEdit, WorkValue};
    use crate::work::wave::Wave;

    fn destination(store: &SqliteStore, repo: &str, name: &str) -> String {
        let route = PlanningDestination::new(
            "/synthetic.git",
            &format!("refs/loopflow/planning/shared/{name}"),
        )
        .unwrap();
        store.bind_peer_planning(repo, &route).unwrap()
    }

    #[test]
    fn routing_retains_each_destination_across_switches_moves_and_missing_work() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let wave = Wave::new(WaveId::new(), "original".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let first = destination(&store, "/repo", "first");
        let original = store.export_peer_planning("/repo", &first).unwrap();
        assert!(!original.changes.is_empty());

        let second = destination(&store, "/repo", "second");
        {
            let conn = store.conn.lock().unwrap();
            planning_write::local(
                &conn,
                &WorkEdit::for_target(wave.id().as_str(), &[WorkValue::WaveName("next".into())]),
            )
            .unwrap();
        }
        let current = store.export_peer_planning("/repo", &second).unwrap();
        assert!(current.changes.len() > original.changes.len());
        let revision = store.revisions().unwrap();
        let statuses = store.peer_planning_status("/repo", Some(&second)).unwrap();
        assert_eq!(store.revisions().unwrap(), revision);
        assert!(statuses.iter().find(|s| s.id == second).unwrap().active);
        assert!(!statuses.iter().find(|s| s.id == first).unwrap().active);
        let journals = store.peer_planning_journal("/repo", Some(&second)).unwrap();
        assert_eq!(
            journals
                .iter()
                .find(|j| j.destination == first)
                .unwrap()
                .changes,
            original.changes
        );
        assert_eq!(
            journals
                .iter()
                .find(|j| j.destination == second)
                .unwrap()
                .changes,
            current.changes
        );
        // Linear selection does not erase Git history or move later edits to old refs.
        assert!(store
            .peer_planning_status("/repo", None)
            .unwrap()
            .iter()
            .all(|s| !s.active));

        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE waves SET repo='/moved' WHERE id=?1",
                [wave.id().as_str()],
            )
            .unwrap();
        let moved = destination(&store, "/moved", "moved");
        assert_eq!(
            store.export_peer_planning("/moved", &moved).unwrap(),
            current
        );
        // No live owner remains to reconstruct the original routing from.
        store
            .conn
            .lock()
            .unwrap()
            .execute("DELETE FROM waves WHERE id=?1", [wave.id().as_str()])
            .unwrap();
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(
            store.export_peer_planning("/repo", &first).unwrap(),
            original
        );
        assert_eq!(
            store.export_peer_planning("/repo", &second).unwrap(),
            current
        );
        assert_eq!(
            store.export_peer_planning("/moved", &moved).unwrap(),
            current
        );
        let before = store.revisions().unwrap();
        assert_eq!(
            store.export_peer_planning("/moved", &moved).unwrap(),
            current
        );
        assert_eq!(store.revisions().unwrap(), before);
    }

    #[test]
    fn rejected_orphan_import_retains_its_route_without_claiming_work() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let route = destination(&store, "/repo", "incoming");
        let incoming = PlanningSnapshot {
            changes: [(
                "orphan".into(),
                PlanningMutation {
                    edit: WorkEdit {
                        target: "project-orphan".into(),
                        change: WorkValue::ProjectWave(WaveId::new().to_string()),
                    },
                    clock: 1,
                    parents: BTreeSet::new(),
                },
            )]
            .into(),
        };
        for _ in 0..2 {
            store
                .import_peer_planning("/repo", &route, "revision", &incoming)
                .unwrap();
            assert_eq!(
                store.export_peer_planning("/repo", &route).unwrap(),
                incoming
            );
        }
        let status = store.peer_planning_status("/repo", None).unwrap().remove(0);
        assert_eq!(status.conflicts.len(), 1);
        assert_eq!(status.selected_records, 1);
        assert_eq!(
            store
                .conn
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .conn
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM planning_peer_observed", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let unrelated = destination(&store, "/repo", "unrelated");
        assert!(store
            .export_peer_planning("/repo", &unrelated)
            .unwrap()
            .changes
            .is_empty());
        assert_eq!(
            store.export_peer_planning("/repo", &route).unwrap(),
            incoming
        );
    }
}
