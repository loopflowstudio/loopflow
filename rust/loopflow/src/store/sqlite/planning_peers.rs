//! Local-only Git planning. Provider attempts and execution never enter this store.
use super::{
    planning_write::{self, PlanningEdit, WriteOrigin},
    SqliteStore,
};
use crate::engine::planning_exchange::{
    PlanningKind, PlanningMutation, PlanningObject, PlanningSnapshot,
};
use crate::engine::planning_git::PlanningDestination;
use crate::store::{
    PeerPlanningRecord, PeerPlanningStatus, PeerPlanningValue, PeerProjectionConflict, StoreError,
    StoreResult,
};
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
    pub fn use_peer_planning(&self, repo: &str, destination: Option<&str>) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(destination) = destination {
            require_destination(&tx, repo, destination)?;
            tx.execute(
                "INSERT INTO planning_active(repo,destination) VALUES(?1,?2)
                ON CONFLICT(repo) DO UPDATE SET destination=excluded.destination
                WHERE planning_active.destination IS NOT excluded.destination",
                params![repo, destination],
            )?;
        } else {
            tx.execute("DELETE FROM planning_active WHERE repo=?1", [repo])?;
        }
        if let Some(destination) = destination {
            enroll_repository(&tx, repo, destination)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn export_peer_planning(
        &self,
        repo: &str,
        destination: &str,
    ) -> StoreResult<PlanningSnapshot> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_destination(&tx, repo, destination)?;
        enroll_repository(&tx, repo, destination)?;
        let snapshot = export_in(&tx, repo, destination)?;
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
        incoming.validate().map_err(invalid)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let mut tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_destination(&tx, repo, destination)?;
        enroll_repository(&tx, repo, destination)?;
        let merged = export_in(&tx, repo, destination)?
            .merge(incoming)
            .map_err(invalid)?;
        for (id, change) in &incoming.changes {
            planning_write::retain(&tx, id, change)?;
            tx.execute("INSERT OR IGNORE INTO planning_members(kind,object_id,repo,destination) VALUES(?1,?2,?3,?4)",params![change.object.kind.as_str(),change.object.id,repo,destination])?;
        }
        let mut grouped = BTreeMap::<PlanningObject, Vec<PlanningEdit>>::new();
        for (_, change) in merged.winners() {
            grouped
                .entry(change.object.clone())
                .or_default()
                .push(PlanningEdit::from_value(
                    change.object.kind,
                    &change.field,
                    change.value.clone(),
                )?);
        }
        let mut pending: Vec<_> = grouped.iter().collect();
        let mut conflicts = BTreeMap::new();
        while !pending.is_empty() {
            let mut retry = Vec::new();
            let mut progress = false;
            conflicts.clear();
            for (object, edits) in pending {
                let save = tx.savepoint()?;
                let result = (|| {
                    if !object
                        .kind
                        .fields()
                        .iter()
                        .all(|field| edits.iter().any(|edit| edit.field() == *field))
                    {
                        return Err(invalid("incomplete planning record"));
                    }
                    planning_write::ensure_record(&save, repo, object, edits)?;
                    require_repository(&save, repo, object)?;
                    // Wave selection points to Projects inserted later in this import.
                    let selected: Vec<_> = edits
                        .iter()
                        .filter(|e| !matches!(e, PlanningEdit::WaveProject(_)))
                        .cloned()
                        .collect();
                    planning_write::write(&save, object, &selected, WriteOrigin::Import)?;
                    require_repository(&save, repo, object)?;
                    save.execute("INSERT OR IGNORE INTO planning_members(kind,object_id,repo,destination) VALUES(?1,?2,?3,?4)",params![object.kind.as_str(),object.id,repo,destination])?;
                    Ok::<_, StoreError>(())
                })();
                match result {
                    Ok(()) => {
                        save.commit()?;
                        progress = true;
                    }
                    Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ref e, _)))
                        if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        save.finish()?;
                        conflicts.insert(
                            object.clone(),
                            "Planning parent or identity conflicts with retained local work"
                                .to_owned(),
                        );
                        retry.push((object, edits));
                    }
                    Err(error) => return Err(error),
                }
            }
            if !progress {
                break;
            }
            pending = retry;
        }
        for (object, edits) in &grouped {
            if conflicts.contains_key(object) {
                continue;
            }
            for edit in edits
                .iter()
                .filter(|e| matches!(e, PlanningEdit::WaveProject(_)))
            {
                let save = tx.savepoint()?;
                match planning_write::write(
                    &save,
                    object,
                    std::slice::from_ref(edit),
                    WriteOrigin::Import,
                ) {
                    Ok(_) => save.commit()?,
                    Err(StoreError::Sqlite(rusqlite::Error::SqliteFailure(ref e, _)))
                        if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                    {
                        save.finish()?;
                        conflicts.insert(
                            object.clone(),
                            "Selected Project is not available locally".into(),
                        );
                    }
                    Err(StoreError::InvalidAuthority(reason)) => {
                        save.finish()?;
                        conflicts.insert(object.clone(), reason);
                    }
                    Err(e) => return Err(e),
                }
            }
            {
                for field in object.kind.fields() {
                    if field == "current_project_id" && conflicts.contains_key(object) {
                        continue;
                    }
                    planning_write::observe(
                        &tx,
                        object,
                        field,
                        merged
                            .heads()
                            .filter(|(_, c)| c.object == *object && c.field == field)
                            .map(|(id, _)| id),
                    )?;
                }
            }
        }
        tx.execute(
            "UPDATE planning_peer_conflicts SET active=0 WHERE repo=?1",
            [repo],
        )?;
        for (object, reason) in conflicts {
            tx.execute("INSERT INTO planning_peer_conflicts(repo,kind,object_id,reason,active) VALUES(?1,?2,?3,?4,1) ON CONFLICT(kind,object_id,reason) DO UPDATE SET active=1",params![repo,object.kind.as_str(),object.id,reason])?;
        }
        tx.execute("INSERT INTO planning_peer_imports(repo,destination,revision) VALUES(?1,?2,?3) ON CONFLICT(repo,destination) DO UPDATE SET revision=excluded.revision WHERE revision IS NOT excluded.revision",params![repo,destination,revision])?;
        tx.commit()?;
        Ok(())
    }
    pub fn peer_planning_status(&self, repo: &str) -> StoreResult<Vec<PeerPlanningStatus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query=conn.prepare("SELECT d.id,d.reference,a.destination=d.id,d.fetched_revision,d.acquisition_error,d.publication_revision,d.publication_state,d.publication_error,d.publication_digest,i.revision FROM planning_destinations d LEFT JOIN planning_active a ON a.repo=d.repo LEFT JOIN planning_peer_imports i ON i.repo=d.repo AND i.destination=d.id WHERE d.repo=?1 ORDER BY d.id")?;
        let rows = query.query_map([repo], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<bool>>(2)?,
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
            let snapshot = export_in(&conn, repo, &id)?;
            let pending_local =
                Some(digest.as_deref() != Some(planning_digest(&snapshot)?.as_str()));
            let mut records = BTreeMap::new();
            let winners: BTreeSet<_> = snapshot.winners().map(|(id, _)| id).collect();
            for (change_id, change) in &snapshot.changes {
                let record =
                    records
                        .entry(change.object.clone())
                        .or_insert_with(|| PeerPlanningRecord {
                            object: change.object.clone(),
                            local_id: change.object.id.clone(),
                            destination: Some(id.clone()),
                            references: Vec::new(),
                            values: Vec::new(),
                        });
                record.values.push(PeerPlanningValue {
                    id: change_id.clone(),
                    field: change.field.clone(),
                    value_json: serde_json::to_string(&change.value)?,
                    candidate: winners.contains(change_id.as_str()),
                    author: None,
                    observed_at: None,
                });
            }
            let mut conflicts=conn.prepare("SELECT kind,object_id,reason FROM planning_peer_conflicts WHERE repo=?1 AND active=1")?;
            let conflicts = conflicts
                .query_map([repo], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?
                .map(|row| {
                    let (kind, id, reason) = row?;
                    Ok(PeerProjectionConflict {
                        object: PlanningObject {
                            kind: serde_json::from_value(kind.into())?,
                            id,
                        },
                        reason,
                    })
                })
                .collect::<StoreResult<Vec<_>>>()?;
            result.push(PeerPlanningStatus {
                id,
                reference,
                active: active.unwrap_or(false),
                selected_records: records.len() as u64,
                imported_revision,
                fetched_revision,
                acquisition_error,
                publication_revision,
                publication_state,
                publication_error,
                pending_local,
                local_error: None,
                conflicts,
                records: records.into_values().collect(),
                recovery_error: None,
            });
        }
        Ok(result)
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
fn require_repository(conn: &Connection, repo: &str, object: &PlanningObject) -> StoreResult<()> {
    let sql=match object.kind{PlanningKind::Wave=>"SELECT repo FROM waves WHERE id=?1",PlanningKind::Project=>"SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",PlanningKind::Task=>"SELECT w.repo FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE t.id=?1",PlanningKind::Comment=>"SELECT w.repo FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE c.id=?1"};
    let actual: String = conn.query_row(sql, [&object.id], |r| r.get(0))?;
    if actual != repo {
        return Err(invalid("planning record belongs to another repository"));
    }
    Ok(())
}
fn enroll_repository(conn: &Connection, repo: &str, destination: &str) -> StoreResult<()> {
    let mut q=conn.prepare("SELECT 'wave',id FROM waves WHERE repo=?1 UNION ALL SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1 UNION ALL SELECT 'task',t.id FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1 UNION ALL SELECT 'comment',c.id FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1")?;
    let rows = q
        .query_map([repo], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (kind, id) in rows {
        let object = PlanningObject {
            kind: serde_json::from_value(kind.clone().into())?,
            id,
        };
        conn.execute("INSERT INTO planning_members(kind,object_id,repo,destination) VALUES(?1,?2,?3,?4) ON CONFLICT(kind,object_id) DO UPDATE SET destination=excluded.destination WHERE repo=excluded.repo AND destination IS NOT excluded.destination",params![kind,object.id,repo,destination])?;
    }
    Ok(())
}
fn export_in(conn: &Connection, repo: &str, destination: &str) -> StoreResult<PlanningSnapshot> {
    let mut q=conn.prepare("SELECT c.id,c.kind,c.object_id,c.field,c.value,c.clock,c.parents FROM planning_peer_changes c JOIN planning_members m ON m.kind=c.kind AND m.object_id=c.object_id WHERE m.repo=?1 AND m.destination=?2")?;
    let rows = q.query_map(params![repo, destination], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, i64>(5)?,
            r.get::<_, String>(6)?,
        ))
    })?;
    let mut snapshot = PlanningSnapshot::default();
    for row in rows {
        let (id, kind, object, field, value, clock, parents) = row?;
        snapshot.changes.insert(
            id,
            PlanningMutation {
                object: PlanningObject {
                    kind: serde_json::from_value(kind.into())?,
                    id: object,
                },
                field,
                value: serde_json::from_str(&value)?,
                clock,
                parents: serde_json::from_str(&parents)?,
            },
        );
    }
    snapshot.validate().map_err(invalid)?;
    Ok(snapshot)
}
