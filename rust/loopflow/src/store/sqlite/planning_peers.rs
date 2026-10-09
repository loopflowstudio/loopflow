//! Peer receipts and projection commit together on the existing planning tables.
//! Export reads retained mutation identities; it never creates a new edit.

use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::durable::{ProjectId, TaskId};
use crate::engine::planning_exchange::{
    LinearObservation, PlanningKind, PlanningMutation, PlanningObject, PlanningSnapshot,
};
use crate::engine::planning_git::PlanningDestination;
use crate::id::WaveId;
use crate::store::{PeerPlanningStatus, PeerProjectionConflict, StoreError, StoreResult};

use super::planning_changes::PlanningChanges;
use super::SqliteStore;

// Projection and delivery share the same field-keyed winners, including the
// mutation identity and provenance. Retries never rebuild a value-only index.
type WinningFields<'a> = BTreeMap<&'a str, (&'a str, &'a PlanningMutation)>;

// Prepare winners and provider frontiers once, before projection retries.
// The snapshot still owns every mutation, losing value and causal link.
#[derive(Default)]
struct ObjectChanges<'a> {
    winners: WinningFields<'a>,
    observations: Vec<&'a LinearObservation>,
    creation: Vec<&'a Value>,
}

fn changes_by_object(snapshot: &PlanningSnapshot) -> BTreeMap<&PlanningObject, ObjectChanges<'_>> {
    let mut objects: BTreeMap<_, ObjectChanges<'_>> = BTreeMap::new();
    for (id, change) in snapshot.winners() {
        objects
            .entry(&change.object)
            .or_default()
            .winners
            .insert(change.field.as_str(), (id, change));
    }
    for change in snapshot
        .changes
        .values()
        .filter(|change| change.field == "creation" || change.linear.is_some())
    {
        let object = objects
            .get_mut(&change.object)
            .expect("every retained object has a winning field");
        if change.field == "creation" {
            object.creation.push(&change.value);
        }
        let Some(observation) = &change.linear else {
            continue;
        };
        let mapping = match change.object.kind {
            PlanningKind::Task => "external_issue_id",
            PlanningKind::Project => "external_project_id",
            // Comment membership carries provenance too, but content owns acquisition.
            PlanningKind::Comment if change.field == "content" => {
                object.observations.push(observation);
                continue;
            }
            _ => continue,
        };
        if object.winners.get(mapping).is_some_and(|(_, winner)| {
            winner.value.as_str().is_some()
                && winner.value.as_str() == observation.body["id"].as_str()
        }) {
            object.observations.push(observation);
        }
    }
    objects
}

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// Content is parsed by its common owner, not by a second SQL Markdown parser.
/// Capture in the caller's transaction, using the same journal and causal heads
/// as scalar triggers. Import suppresses echo; readback only adds missing facts.
pub(super) fn capture_project_content(
    conn: &Connection,
    project: &ProjectId,
    content: &crate::pm::ProjectContent,
) -> StoreResult<()> {
    let (importing, observation, observed): (bool, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT importing,observation,fields FROM planning_peer_context WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    if importing {
        return Ok(());
    }
    let observation: Option<LinearObservation> = observation
        .map(|body| serde_json::from_str(&body))
        .transpose()?;
    let observed: Option<Value> = observed
        .map(|body| serde_json::from_str(&body))
        .transpose()?;
    let mut query = conn.prepare(
        "SELECT c.kind,c.object_id,c.field,c.value,c.clock,c.linear,c.parents,c.id
        FROM planning_peer_changes c JOIN planning_peer_heads h ON h.id=c.id
        WHERE c.kind='project' AND c.object_id=?1",
    )?;
    let mut heads = PlanningSnapshot::default();
    let mut rows = query.query([project.as_str()])?;
    while let Some(row) = rows.next()? {
        heads.changes.insert(row.get(7)?, read_mutation(row)?);
    }
    let winners: BTreeMap<_, _> = heads
        .winners()
        .map(|(_, c)| (c.field.as_str(), c))
        .collect();
    let fields = serde_json::to_value(content)?;
    for (field, value) in fields.as_object().expect("Project content is an object") {
        let linear = observation
            .as_ref()
            .filter(|_| observed.as_ref().and_then(|o| o.get(field)) == Some(value));
        let unchanged = winners
            .get(field.as_str())
            .is_some_and(|winner| winner.value == *value);
        let frontier_retained = linear.is_none_or(|observation| {
            heads.changes.values().any(|head| {
                head.field == *field
                    && head.value == *value
                    && head.linear.as_ref().is_some_and(|prior| {
                        prior.body["id"] == observation.body["id"]
                            && prior.revision() == observation.revision()
                    })
            })
        });
        if unchanged && frontier_retained {
            continue;
        }
        conn.execute("INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
            SELECT lower(hex(randomblob(16))),'project',?1,?2,?3,
                max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),?4,
                (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='project' AND object_id=?1 AND field=?2)",
            params![project.as_str(),field,value.to_string(),linear.map(serde_json::to_string).transpose()?])?;
    }
    Ok(())
}

fn content_field(field: &str) -> bool {
    matches!(field, "workflow" | "krs" | "metric_targets")
}

/// Mark only fields equal to the accepted provider fact. Reconciliation may
/// preserve a pending local value; that value must not acquire Linear priority.
pub(super) fn observe_task(
    conn: &Connection,
    provider: &str,
    item: &crate::pm::PmItem,
    project: &str,
    observed_at: i64,
) -> StoreResult<()> {
    let observation = LinearObservation {
        body: serde_json::to_value(item)?,
        observed_at,
    };
    let mut fields = observation.fields(PlanningKind::Task)?;
    fields["project_id"] = Value::String(project.into());
    observe_in(conn, provider, &observation, &fields)
}

pub(super) fn observe_project(
    conn: &Connection,
    provider: &str,
    project: &crate::pm::PmProject,
    observed_at: i64,
) -> StoreResult<()> {
    let observation = LinearObservation {
        body: serde_json::to_value(project)?,
        observed_at,
    };
    let fields = observation.fields(PlanningKind::Project)?;
    observe_in(conn, provider, &observation, &fields)
}

pub(super) fn observe_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &crate::pm::IssueComment,
    observed_at: i64,
) -> StoreResult<()> {
    let observation = LinearObservation {
        body: serde_json::to_value(comment)?,
        observed_at,
    };
    let mut fields = observation.fields(PlanningKind::Comment)?;
    fields["task_id"] = Value::String(task.to_string());
    observe_in(conn, "linear", &observation, &fields)
}

fn observe_in(
    conn: &Connection,
    provider: &str,
    observation: &LinearObservation,
    fields: &Value,
) -> StoreResult<()> {
    conn.execute(
        "UPDATE planning_peer_context SET observation=?1,fields=?2",
        params![
            (provider == "linear")
                .then_some(observation)
                .map(serde_json::to_string)
                .transpose()?,
            fields.to_string(),
        ],
    )?;
    Ok(())
}

pub(super) fn clear_observation(conn: &Connection) -> StoreResult<()> {
    conn.execute(
        "UPDATE planning_peer_context SET observation=NULL,fields=NULL",
        [],
    )?;
    Ok(())
}

impl SqliteStore {
    /// Select routing for future root Waves only. Existing membership and all
    /// descendant routing survive a switch, including a return to local-only.
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
        tx.commit()?;
        Ok(())
    }

    pub fn peer_planning_status(&self, repo: &str) -> StoreResult<Vec<PeerPlanningStatus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let mut query = tx.prepare(
            "SELECT d.id,d.reference,EXISTS(SELECT 1 FROM planning_active a
                WHERE a.repo=d.repo AND a.destination=d.id),
                (SELECT count(*) FROM planning_members m WHERE m.repo=d.repo AND m.destination=d.id),
                (SELECT revision FROM planning_peer_imports i WHERE i.repo=d.repo AND i.destination=d.id),
                d.fetched_revision,d.acquisition_error,d.publication_revision,
                d.publication_state,d.publication_error,d.publication_digest
            FROM planning_destinations d WHERE d.repo=?1 ORDER BY d.id",
        )?;
        let rows = query
            .query_map([repo], |row| {
                Ok((
                    PeerPlanningStatus {
                        id: row.get(0)?,
                        reference: row.get(1)?,
                        active: row.get(2)?,
                        selected_records: row.get::<_, i64>(3)? as u64,
                        imported_revision: row.get(4)?,
                        fetched_revision: row.get(5)?,
                        acquisition_error: row.get(6)?,
                        publication_revision: row.get(7)?,
                        publication_state: row.get(8)?,
                        publication_error: row.get(9)?,
                        pending_local: None,
                        local_error: None,
                        conflicts: Vec::new(),
                    },
                    row.get::<_, Option<String>>(10)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(query);
        let mut statuses = Vec::new();
        for (mut status, digest) in rows {
            status.conflicts = projection_conflicts_in(&tx, repo, &status.id)?;
            match export_selected(&tx, repo, &status.id) {
                Ok((snapshot, held)) => {
                    status.pending_local = Some(match digest {
                        Some(digest) => digest != planning_digest(&snapshot)?,
                        None => !snapshot.changes.is_empty(),
                    });
                    status.conflicts.extend(
                        held.into_iter()
                            .map(|(object, reason)| PeerProjectionConflict { object, reason }),
                    );
                }
                // A damaged journal is destination-local evidence, not a reason
                // to hide the other plans or claim no local changes. SQL failures
                // still fail the read. Never display raw journal contents here.
                Err(StoreError::InvalidData(_) | StoreError::Serde(_)) => {
                    status.local_error = Some(
                        "Local planning journal is invalid; pending changes and sharing holds are unknown. Retained plans and sync receipts are unchanged.".into(),
                    );
                }
                Err(error) => return Err(error),
            }
            statuses.push(status);
        }
        tx.commit()?;
        Ok(statuses)
    }

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

    /// Provision once, or recover the same user key on another machine. Callers
    /// generate a key only for an explicit new identity, never during connection.
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

    /// Joining binds an empty selection. Existing records require a separate,
    /// explicit selection; incoming records may not claim unselected identities.
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

    // Dispatch needs only routing, not the status projection of every journal.
    // A broken destination must not prevent attempts against independent plans.
    pub(crate) fn peer_planning_destination_ids(&self, repo: &str) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT id FROM planning_destinations WHERE repo=?1 ORDER BY id")?;
        let ids = query
            .query_map([repo], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
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

    /// Explicitly include entire Waves and their existing descendants. Never
    /// select by name or silently include an ancestor's other work. Private
    /// references hold exchange, not selection of independent Waves.
    pub fn select_peer_waves(
        &self,
        repo: &str,
        destination: &str,
        waves: &[WaveId],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_destination(&tx, repo, destination)?;
        for wave in waves {
            let owner: Option<String> = tx
                .query_row("SELECT repo FROM waves WHERE id=?1", [wave], |r| r.get(0))
                .optional()?;
            if owner.as_deref() != Some(repo) {
                return Err(invalid("selected Wave does not belong to this repository"));
            }
            let mut query = tx.prepare("WITH RECURSIVE selected(id) AS (
                SELECT id FROM waves WHERE id=?1 UNION SELECT w.id FROM waves w JOIN selected s ON w.parent_wave_id=s.id)
                SELECT 'wave',id FROM selected
                UNION ALL SELECT 'project',id FROM projects WHERE wave_id IN selected
                UNION ALL SELECT 'task',t.id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE p.wave_id IN selected
                UNION ALL SELECT 'comment',c.id FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN projects p ON p.id=t.project_id WHERE p.wave_id IN selected")?;
            let rows = query.query_map([wave], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (kind, id) = row?;
                let object = PlanningObject {
                    kind: serde_json::from_value(Value::String(kind))?,
                    id,
                };
                require_repository(&tx, &object, repo)?;
                enroll(&tx, repo, destination, &object)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn export_peer_planning(
        &self,
        repo: &str,
        destination: &str,
    ) -> StoreResult<PlanningSnapshot> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let (snapshot, _) = export_selected(&tx, repo, destination)?;
        tx.commit()?;
        Ok(snapshot)
    }

    /// A revision acknowledges the retained journal, allowed projections and
    /// explicit conflicts, not complete projection. Export remains a separate
    /// read of eligible history. No execution writer is called.
    pub fn import_peer_planning(
        &self,
        repo: &str,
        destination: &str,
        revision: &str,
        incoming: &PlanningSnapshot,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let mut tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved = export_in(&tx, repo, destination)?;
        let merged = saved.merge(incoming).map_err(invalid)?;
        reserve_incoming(&tx, repo, destination, incoming)?;
        let mut objects = changes_by_object(&merged);
        let held = selection_conflicts(&tx, repo, destination, &merged)?;
        retain_mutations(&tx, &saved, incoming)?;
        tx.execute("UPDATE planning_peer_context SET importing=1", [])?;
        for (object, changes) in &objects {
            // Validated mutations have known fields and one winner per field.
            if !object
                .kind
                .fields()
                .iter()
                .all(|field| changes.winners.contains_key(field))
            {
                return Err(invalid(format!("incomplete planning record {}", object.id)));
            }
            match object.kind {
                PlanningKind::Wave => {
                    crate::id::WaveId::parse(&object.id).map_err(invalid)?;
                }
                PlanningKind::Project => {
                    crate::durable::ProjectId::parse(&object.id).map_err(invalid)?;
                }
                PlanningKind::Task => {
                    crate::durable::TaskId::parse(&object.id).map_err(invalid)?;
                }
                PlanningKind::Comment => {}
            }
        }
        let mut pending = objects
            .iter_mut()
            .filter(|(object, _)| !held.contains_key(*object))
            .map(|(&object, changes)| {
                changes.observations = latest_observations(changes.observations.iter().copied())?;
                Ok((object, &*changes))
            })
            .collect::<StoreResult<Vec<_>>>()?;
        let mut conflicts = loop {
            let count = pending.len();
            let mut retry = Vec::new();
            // Only the final attempt describes a current conflict. A parent
            // projected in this pass may replace an earlier missing-parent error.
            let mut conflicts = BTreeSet::new();
            for (object, changes) in pending {
                let savepoint = tx.savepoint()?;
                match insert_and_project(&savepoint, object, repo, changes, &merged) {
                    Ok(()) => {
                        savepoint.commit()?;
                    }
                    Err(error) if projection_conflict(&error) => {
                        // Roll back and release before retaining contrary evidence.
                        savepoint.finish()?;
                        if let StoreError::ProjectMembershipConflict { project_id } = &error {
                            // The rejected projection rolls back, not the contrary
                            // relationship evidence. Independent objects still commit.
                            super::planning::retain_membership_conflict(
                                &tx, repo, "linear", project_id,
                            )?;
                        }
                        conflicts.insert((object.clone(), error.to_string()));
                        retry.push((object, changes));
                    }
                    Err(error) => return Err(error),
                }
            }
            // Parent ordering may need another pass, but a conflict never blocks
            // an independent object or causes an unbounded retry.
            if retry.len() == count || retry.is_empty() {
                break conflicts;
            }
            pending = retry;
        };
        // Wave selection points back at Projects. Set it only after identity and
        // ownership projection, rather than deferring all foreign-key checks.
        for (&object, changes) in &objects {
            if object.kind != PlanningKind::Wave
                || held.contains_key(object)
                || !exists(&tx, object)?
            {
                continue;
            }
            match project_fields(
                &tx,
                object,
                std::iter::once((
                    "current_project_id",
                    &changes.winners["current_project_id"].1.value,
                )),
            ) {
                Ok(()) => {}
                Err(error) if projection_conflict(&error) => {
                    conflicts.insert((object.clone(), error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        reconcile_conflicts(&tx, repo, destination, &conflicts)?;
        tx.execute("UPDATE planning_peer_context SET importing=0", [])?;
        tx.execute(
            "INSERT INTO planning_peer_imports(repo,destination,revision) VALUES(?1,?2,?3)
            ON CONFLICT(repo,destination) DO UPDATE SET revision=excluded.revision
            WHERE planning_peer_imports.revision IS NOT excluded.revision",
            params![repo, destination, revision],
        )?;
        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    fn peer_projection_conflicts(&self, repo: &str) -> StoreResult<Vec<PeerProjectionConflict>> {
        Ok(self
            .peer_planning_status(repo)?
            .into_iter()
            .flat_map(|status| status.conflicts)
            .collect())
    }
}

fn planning_digest(snapshot: &PlanningSnapshot) -> StoreResult<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(snapshot.to_bytes().map_err(invalid)?)
    ))
}

fn require_destination(conn: &Connection, repo: &str, destination: &str) -> StoreResult<()> {
    let found: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM planning_destinations WHERE repo=?1 AND id=?2)",
        params![repo, destination],
        |r| r.get(0),
    )?;
    if !found {
        return Err(invalid(
            "bind a planning destination before synchronization",
        ));
    }
    Ok(())
}

fn member_destination(
    conn: &Connection,
    repo: &str,
    object: &PlanningObject,
) -> StoreResult<Option<String>> {
    conn.query_row(
        "SELECT destination FROM planning_members WHERE kind=?1 AND object_id=?2 AND repo=?3",
        params![object.kind.as_str(), object.id, repo],
        |r| r.get(0),
    )
    .optional()
    .map_err(Into::into)
}

fn enroll(
    conn: &Connection,
    repo: &str,
    destination: &str,
    object: &PlanningObject,
) -> StoreResult<()> {
    if let Some(saved) = member_destination(conn, repo, object)? {
        if saved != destination {
            return Err(invalid(format!(
                "planning record {} belongs to another selected plan",
                object.id
            )));
        }
        return Ok(());
    }
    conn.execute(
        "INSERT INTO planning_members(kind,object_id,repo,destination) VALUES(?1,?2,?3,?4)",
        params![object.kind.as_str(), object.id, repo, destination],
    )?;
    Ok(())
}

fn recorded(conn: &Connection, object: &PlanningObject) -> StoreResult<bool> {
    let journal: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM planning_peer_changes WHERE kind=?1 AND object_id=?2)",
        params![object.kind.as_str(), object.id],
        |r| r.get(0),
    )?;
    Ok(journal || exists(conn, object)?)
}

fn reserve_incoming(
    conn: &Connection,
    repo: &str,
    destination: &str,
    incoming: &PlanningSnapshot,
) -> StoreResult<()> {
    // Merge validated mutations before membership can change. Check ownership
    // without resolving field winners or copying their values.
    for object in incoming.objects() {
        let selected = member_destination(conn, repo, object)?;
        if selected.as_deref() != Some(destination) {
            require_repository(conn, object, repo)?;
        }
        if selected.is_none() && recorded(conn, object)? {
            return Err(invalid(format!(
                "incoming planning record {} is local and unselected; joining cannot merge it",
                object.id
            )));
        }
        enroll(conn, repo, destination, object)?;
    }
    for (_, change) in incoming.winners() {
        if let Some(object) = reference(change) {
            if unselected(conn, repo, destination, &object)? {
                return Err(invalid(format!(
                    "planning reference {} belongs to unselected work",
                    object.id
                )));
            }
        }
    }
    Ok(())
}

fn reference(change: &PlanningMutation) -> Option<PlanningObject> {
    let (kind, value) = match change.field.as_str() {
        "creation" => (
            if change.object.kind == PlanningKind::Task {
                PlanningKind::Project
            } else {
                PlanningKind::Wave
            },
            &change.value["export"]["parent"],
        ),
        "parent_wave_id" | "wave_id" => (PlanningKind::Wave, &change.value),
        "current_project_id" | "project_id" => (PlanningKind::Project, &change.value),
        "task_id" => (PlanningKind::Task, &change.value),
        _ => return None,
    };
    value.as_str().map(|id| PlanningObject {
        kind,
        id: id.into(),
    })
}

fn unselected(
    conn: &Connection,
    repo: &str,
    destination: &str,
    object: &PlanningObject,
) -> StoreResult<bool> {
    Ok(match member_destination(conn, repo, object)? {
        Some(saved) => saved != destination,
        None => recorded(conn, object)?,
    })
}

/// A common-writer move never enrolls its new parent. Hold the affected object's
/// entire history and dependents, not the destination. Checking losing references
/// too prevents a later move back from publishing previously private ancestry.
/// Omission is not deletion: peers retain their last shared values.
fn selection_conflicts(
    conn: &Connection,
    repo: &str,
    destination: &str,
    snapshot: &PlanningSnapshot,
) -> StoreResult<BTreeMap<PlanningObject, String>> {
    let mut dependents: BTreeMap<PlanningObject, BTreeSet<&PlanningObject>> = BTreeMap::new();
    for change in snapshot.changes.values() {
        if let Some(parent) = reference(change) {
            dependents.entry(parent).or_default().insert(&change.object);
        }
    }
    let mut pending = BTreeSet::new();
    for parent in dependents.keys() {
        if unselected(conn, repo, destination, parent)? {
            pending.insert(parent.clone());
        }
    }
    let mut held = BTreeMap::new();
    for object in snapshot.objects() {
        if belongs_elsewhere(conn, object, repo)? {
            held.insert(object.clone(), "planning exchange held: record moved outside this repository; local work and peer history are retained".into());
            pending.insert(object.clone());
        }
    }
    // Follow each dependency once, including Wave/selected-Project cycles,
    // rather than rescanning every record for each level of held ancestry.
    while let Some(parent) = pending.pop_first() {
        for object in dependents.remove(&parent).into_iter().flatten() {
            if let Entry::Vacant(entry) = held.entry(object.clone()) {
                entry.insert(format!(
                    "planning exchange held: reference {} is outside the exchangeable selection; local work and peer history are retained",
                    parent.id
                ));
                pending.insert(object.clone());
            }
        }
    }
    Ok(held)
}

fn retain_mutations(
    conn: &Connection,
    saved: &PlanningSnapshot,
    incoming: &PlanningSnapshot,
) -> StoreResult<()> {
    let mut added: Vec<_> = incoming
        .changes
        .iter()
        .filter(|(id, _)| !saved.changes.contains_key(*id))
        .collect();
    added.sort_by_key(|(id, change)| (change.clock, *id));
    // Merge checked this repository's identities. New receipts must also
    // agree with any identity retained elsewhere in the same store.
    let mut query = conn.prepare_cached(
        "SELECT kind,object_id,field,value,clock,linear,parents FROM planning_peer_changes WHERE id=?1",
    )?;
    for (id, change) in added {
        let mut rows = query.query([id])?;
        if let Some(row) = rows.next()? {
            if read_mutation(row)? != *change {
                return Err(invalid(format!(
                    "planning change {id} has conflicting contents"
                )));
            }
            continue;
        }
        conn.execute(
            "INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                id,
                change.object.kind.as_str(),
                change.object.id,
                change.field,
                change.value.to_string(),
                change.clock,
                change
                    .linear
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                serde_json::to_string(&change.parents)?
            ],
        )?;
    }
    Ok(())
}

// Status and publication must compare the same eligible history. Retained private
// references stay in the journal but cannot become pending publication content.
fn export_selected(
    conn: &Connection,
    repo: &str,
    destination: &str,
) -> StoreResult<(PlanningSnapshot, BTreeMap<PlanningObject, String>)> {
    let mut snapshot = export_in(conn, repo, destination)?;
    let held = selection_conflicts(conn, repo, destination, &snapshot)?;
    snapshot
        .changes
        .retain(|_, change| !held.contains_key(&change.object));
    Ok((snapshot, held))
}

fn export_in(conn: &Connection, repo: &str, destination: &str) -> StoreResult<PlanningSnapshot> {
    require_destination(conn, repo, destination)?;
    let mut query = conn.prepare("SELECT c.kind,c.object_id,c.field,c.value,c.clock,c.linear,c.parents,c.id
        FROM planning_peer_changes c JOIN planning_members m ON m.kind=c.kind AND m.object_id=c.object_id
        WHERE m.repo=?1 AND m.destination=?2")?;
    let mut snapshot = PlanningSnapshot::default();
    let mut rows = query.query(params![repo, destination])?;
    while let Some(row) = rows.next()? {
        snapshot.changes.insert(row.get(7)?, read_mutation(row)?);
    }
    snapshot.validate().map_err(invalid)?;
    let mut query = conn.prepare(
        "SELECT h.id FROM planning_peer_heads h
         JOIN planning_peer_changes c ON c.id=h.id
         JOIN planning_members m ON m.kind=c.kind AND m.object_id=c.object_id
         WHERE m.repo=?1 AND m.destination=?2",
    )?;
    let actual = query
        .query_map(params![repo, destination], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    if !snapshot
        .heads()
        .map(|(id, _)| id)
        .eq(actual.iter().map(String::as_str))
    {
        return Err(invalid(
            "planning causal heads disagree with retained mutations",
        ));
    }
    Ok(snapshot)
}

/// SELECT kind,object_id,field,value,clock,linear,parents
fn read_mutation(row: &rusqlite::Row<'_>) -> StoreResult<PlanningMutation> {
    Ok(PlanningMutation {
        object: PlanningObject {
            kind: serde_json::from_value(Value::String(row.get(0)?))?,
            id: row.get(1)?,
        },
        field: row.get(2)?,
        value: serde_json::from_str(&row.get::<_, String>(3)?)?,
        clock: row.get(4)?,
        linear: row
            .get::<_, Option<String>>(5)?
            .map(|body| serde_json::from_str(&body))
            .transpose()?,
        parents: serde_json::from_str(&row.get::<_, String>(6)?)?,
    })
}

fn table(kind: PlanningKind) -> &'static str {
    match kind {
        PlanningKind::Wave => "waves",
        PlanningKind::Project => "projects",
        PlanningKind::Task => "tasks",
        PlanningKind::Comment => "task_comments",
    }
}

fn require_repository(conn: &Connection, object: &PlanningObject, repo: &str) -> StoreResult<()> {
    if belongs_elsewhere(conn, object, repo)? {
        return Err(invalid(format!(
            "planning {} belongs to another repository",
            object.id
        )));
    }
    Ok(())
}

fn belongs_elsewhere(conn: &Connection, object: &PlanningObject, repo: &str) -> StoreResult<bool> {
    let sql = match object.kind {
        PlanningKind::Wave => "SELECT repo FROM waves WHERE id=?1",
        PlanningKind::Project => "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
        PlanningKind::Task => "SELECT w.repo FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE t.id=?1",
        PlanningKind::Comment => "SELECT w.repo FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE c.id=?1",
    };
    let owner: Option<String> = conn.query_row(sql, [&object.id], |r| r.get(0)).optional()?;
    let retained: Option<String> = conn
        .query_row(
            "SELECT repo FROM planning_peer_conflicts WHERE kind=?1 AND object_id=?2 LIMIT 1",
            params![object.kind.as_str(), object.id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(owner.as_deref().is_some_and(|owner| owner != repo)
        || retained.as_deref().is_some_and(|owner| owner != repo))
}

fn exists(conn: &Connection, object: &PlanningObject) -> StoreResult<bool> {
    Ok(conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE id=?1)",
            table(object.kind)
        ),
        [&object.id],
        |r| r.get(0),
    )?)
}

fn insert_and_project(
    conn: &Connection,
    object: &PlanningObject,
    repo: &str,
    changes: &ObjectChanges<'_>,
    snapshot: &PlanningSnapshot,
) -> StoreResult<()> {
    let winners = &changes.winners;
    if object.kind == PlanningKind::Comment {
        project_comment(conn, object, changes)?;
        return require_repository(conn, object, repo);
    }
    if !exists(conn, object)? {
        let required = |key: &str| {
            winners
                .get(key)
                .and_then(|(_, change)| change.value.as_str())
                .ok_or_else(|| invalid(format!("missing {key} for {}", object.id)))
        };
        match object.kind {
            PlanningKind::Wave => {
                conn.execute(
                    "INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES(?1,?2,?3,unixepoch(),?4)",
                    params![object.id, required("name")?, repo, winners["parent_wave_id"].1.value.as_str()],
                )?;
            }
            PlanningKind::Project => {
                conn.execute(
                    "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,unixepoch())",
                    params![object.id, required("wave_id")?],
                )?;
            }
            PlanningKind::Task => {
                // New planning records have no placement. Initialize local required
                // fields just as the common writer does; never import these values.
                conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,created_at,updated_at,workspace_slug) VALUES(?1,?2,?3,unixepoch(),unixepoch(),'')",
                    params![object.id,required("project_id")?,required("issue_identifier")?])?;
            }
            PlanningKind::Comment => unreachable!("comments use the common thread writer"),
        }
    }
    if object.kind == PlanningKind::Wave {
        validate_wave(conn, object, winners, repo)?;
    }
    if let Some((_, winner)) = winners.get("creation") {
        super::planning_export::import_peer_receipts(
            conn,
            object,
            &winner.value,
            &changes.creation,
        )?;
    }
    let previous = delivery_fields(conn, object)?;
    acquire_linear_frontier(conn, object, changes, repo)?;
    project_fields(
        conn,
        object,
        winners
            .values()
            .filter(|(_, change)| {
                !(change.field == "creation"
                    || object.kind == PlanningKind::Wave && change.field == "current_project_id"
                    || object.kind == PlanningKind::Project && content_field(&change.field))
            })
            .map(|(_, change)| (change.field.as_str(), &change.value)),
    )?;
    if object.kind == PlanningKind::Project {
        let content = serde_json::from_value(serde_json::json!({
            "workflow":winners["workflow"].1.value,
            "krs":winners["krs"].1.value,
            "metric_targets":winners["metric_targets"].1.value,
        }))?;
        super::project_content::save_content(conn, &ProjectId::from_raw(&object.id), &content)?;
    }
    project_delivery_fields(conn, object, winners, snapshot, &previous)?;
    if let Some(observation) = changes.observations.last() {
        if matches!(object.kind, PlanningKind::Task | PlanningKind::Project) {
            super::planning_export::attach_in(
                conn,
                repo,
                object.kind == PlanningKind::Project,
                &observation.body,
            )?;
        }
    }
    require_repository(conn, object, repo)
}

fn project_comment(
    conn: &Connection,
    object: &PlanningObject,
    changes: &ObjectChanges<'_>,
) -> StoreResult<()> {
    let winners = &changes.winners;
    let task = TaskId::from_raw(
        winners["task_id"]
            .1
            .value
            .as_str()
            .ok_or_else(|| invalid("comment requires a Task"))?,
    );
    let winner = winners["content"].1;
    for observation in &changes.observations {
        let comment = serde_json::from_value(observation.body.clone())?;
        if !super::task_comments::ingest_task_comment(
            conn,
            &task,
            &comment,
            observation.observed_at,
        )? {
            return Err(invalid(
                "peer provider frontier is older than retained evidence",
            ));
        }
    }
    if winner.linear.is_none() {
        let content = &winner.value;
        let comment = crate::ops::pm::TaskComment {
            id: object.id.clone(),
            body: content["body"]
                .as_str()
                .expect("validated comment body")
                .into(),
            author: serde_json::from_str(content["author"].as_str().expect("validated author"))?,
            created_at: content["created_at"].as_str().map(str::to_owned),
        };
        super::task_comments::insert_authored_comment(conn, &task, &comment)?;
    }
    Ok(())
}

/// Superseded facts remain in the journal, not in acquisition. Keep every body
/// at the latest revision so the common reader can reject contradictions; receipt
/// time orders acquisition within that frontier, never across provider revisions.
fn latest_observations<'a>(
    observations: impl Iterator<Item = &'a LinearObservation>,
) -> StoreResult<Vec<&'a LinearObservation>> {
    let mut latest = None;
    let mut retained = Vec::new();
    for observation in observations {
        let revision = super::planning::revision_nanos(observation.revision())?;
        if revision < latest {
            continue;
        }
        if revision > latest {
            latest = revision;
            retained.clear();
        }
        retained.push(observation);
    }
    retained.sort_by_key(|observation| observation.observed_at);
    retained.dedup();
    Ok(retained)
}

/// Reuse provider acquisition's revision and equal-revision checks, rather than
/// treating peer receipt time as a fresh read. This is inside the object's
/// projection savepoint: rejected ownership never advances its provider cache.
fn acquire_linear_frontier(
    conn: &Connection,
    object: &PlanningObject,
    changes: &ObjectChanges<'_>,
    repo: &str,
) -> StoreResult<()> {
    let winners = &changes.winners;
    let (provider_table, mapping) = match object.kind {
        PlanningKind::Task => ("pm_items", "external_issue_id"),
        PlanningKind::Project => ("pm_projects", "external_project_id"),
        _ => return Ok(()),
    };
    let Some(provider_id) = winners[mapping].1.value.as_str() else {
        return Ok(());
    };
    let mut accepted = true;
    for observation in &changes.observations {
        let retained: Option<(String, i64)> = conn.query_row(
            &format!("SELECT body,observed_at FROM {provider_table} WHERE repo=?1 AND provider='linear' AND id=?2"),
            params![repo, provider_id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?;
        // Reuse the retained age for an identical fact, but still call the
        // acquisition owner: removal/archive can invalidate an unchanged body.
        let mut observed_at = observation.observed_at;
        if let Some((body, acquired)) = retained {
            if serde_json::from_str::<Value>(&body)? == observation.body {
                observed_at = observed_at.max(acquired);
            }
        }
        accepted = match object.kind {
            PlanningKind::Task => {
                let item = serde_json::from_value(observation.body.clone())?;
                let accepted = super::planning::put_item(conn, repo, "linear", observed_at, &item)?;
                if accepted {
                    conn.execute("UPDATE pm_items SET needs_refresh=0 WHERE repo=?1 AND provider='linear' AND id=?2 AND needs_refresh!=0", params![repo, provider_id])?;
                }
                accepted
            }
            PlanningKind::Project => {
                let project = serde_json::from_value(observation.body.clone())?;
                super::planning::validate_project_membership(conn, repo, "linear", &project)?;
                super::planning::put_project(conn, repo, "linear", observed_at, &project)?
            }
            _ => unreachable!("only Tasks and Projects have entity frontiers"),
        };
    }
    // Even a locally authored winner cannot erase retained provider removal or
    // unresolved relationship evidence. These facts have independent ordering.
    match object.kind {
        PlanningKind::Task => {
            let removed: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM pm_issue_changes WHERE issue_id=?1 AND removed=1)",
                [provider_id],
                |row| row.get(0),
            )?;
            if removed {
                return Err(invalid("Linear removal prevents peer Task projection"));
            }
        }
        PlanningKind::Project => {
            let retained: Option<(String, bool, bool)> = conn
                .query_row(
                    "SELECT body,archived,membership_unresolved FROM pm_projects
                 WHERE repo=?1 AND provider='linear' AND id=?2",
                    params![repo, provider_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            if let Some((body, archived, unresolved)) = retained {
                if archived {
                    return Err(invalid("Linear archive prevents peer Project projection"));
                }
                if unresolved {
                    return Err(StoreError::ProjectMembershipConflict {
                        project_id: provider_id.into(),
                    });
                }
                let mut project: crate::pm::PmProject = serde_json::from_str(&body)?;
                // Never promote the entity revision into relationship authority.
                project.initiative_ids = serde_json::from_str(
                    winners["planning_initiatives"]
                        .1
                        .value
                        .as_str()
                        .expect("validated relationship JSON"),
                )?;
                project.team_ids = serde_json::from_str(
                    winners["planning_teams"]
                        .1
                        .value
                        .as_str()
                        .expect("validated relationship JSON"),
                )?;
                super::planning::validate_project_membership(conn, repo, "linear", &project)?;
            }
        }
        _ => unreachable!("only Tasks and Projects have entity frontiers"),
    }
    if !accepted {
        return Err(invalid(
            "peer provider frontier is older than retained evidence",
        ));
    }
    // A mapping is not a creation acknowledgement. Only an accepted provider
    // body can reconcile the original attempt, including after a peer supplied
    // the mapping first. Keep this inside the object's projection savepoint.
    if let Some(observation) = changes
        .observations
        .last()
        .filter(|_| matches!(object.kind, PlanningKind::Task | PlanningKind::Project))
    {
        super::planning_export::attach_in(
            conn,
            repo,
            object.kind == PlanningKind::Project,
            &observation.body,
        )?;
    }
    conn.execute(
        &format!(
            "UPDATE {} SET (planning_provider_revision,pm_snapshot_synced_at)=(
        SELECT json_extract(body,'$.revision'),observed_at
        FROM {provider_table} WHERE repo=?2 AND provider='linear' AND id=?3)
        WHERE id=?1 AND (planning_provider_revision,pm_snapshot_synced_at) IS NOT (
        SELECT json_extract(body,'$.revision'),observed_at
        FROM {provider_table} WHERE repo=?2 AND provider='linear' AND id=?3)",
            table(object.kind)
        ),
        params![object.id, repo, provider_id],
    )?;
    Ok(())
}

// These are the scalar fields delivered by the common field writer. State,
// comments, deletion, creation and order keep their distinct receipt protocols.
pub(super) fn delivery_field(kind: PlanningKind, field: &str) -> Option<&'static str> {
    match (kind, field) {
        (PlanningKind::Task, "issue_title") | (PlanningKind::Project, "project_name") => {
            Some("name")
        }
        (PlanningKind::Task, "issue_description") => Some("description"),
        (PlanningKind::Task, "planning_assignee") => Some("assignee"),
        (PlanningKind::Task, "project_id") => Some("project_id"),
        (PlanningKind::Project, "project_summary") => Some("summary"),
        (PlanningKind::Project, "workflow") => Some("workflow"),
        (PlanningKind::Project, "krs") => Some("krs"),
        (PlanningKind::Project, "metric_targets") => Some("metric_targets"),
        (PlanningKind::Project, "status") => Some("status"),
        _ => None,
    }
}

fn delivery_fields(
    conn: &Connection,
    object: &PlanningObject,
) -> StoreResult<BTreeMap<String, Value>> {
    let mut columns = object
        .kind
        .fields()
        .iter()
        .filter(|field| {
            delivery_field(object.kind, field).is_some()
                && !(object.kind == PlanningKind::Project && content_field(field))
        })
        .map(|field| format!("'{field}',{field}"))
        .collect::<Vec<_>>();
    if object.kind == PlanningKind::Task {
        columns.push("'disposition',json_object('planning_state',planning_state,'planning_completed',planning_completed,'planning_completed_at',planning_completed_at)".into());
    }
    if columns.is_empty() {
        return Ok(BTreeMap::new());
    }
    // insert_and_project has already established the row in this transaction.
    let body: String = conn.query_row(
        &format!(
            "SELECT json_object({}) FROM {} WHERE id=?1",
            columns.join(","),
            table(object.kind)
        ),
        [&object.id],
        |row| row.get(0),
    )?;
    let mut fields: BTreeMap<String, Value> = serde_json::from_str(&body)?;
    if object.kind == PlanningKind::Project {
        let content = super::project_content::read_content(conn, &ProjectId::from_raw(&object.id))?;
        fields.extend(serde_json::from_value::<BTreeMap<String, Value>>(
            serde_json::to_value(content)?,
        )?);
    }
    Ok(fields)
}

fn project_delivery_fields(
    conn: &Connection,
    object: &PlanningObject,
    winners: &WinningFields<'_>,
    snapshot: &PlanningSnapshot,
    previous: &BTreeMap<String, Value>,
) -> StoreResult<()> {
    let task = TaskId::from_raw(&object.id);
    let project = ProjectId::from_raw(&object.id);
    let owner = match object.kind {
        PlanningKind::Task => PlanningChanges::Task(&task),
        PlanningKind::Project => PlanningChanges::Project(&project),
        _ => return Ok(()),
    };
    for &(id, change) in winners.values() {
        if object.kind == PlanningKind::Task && change.field == "disposition" {
            if let Some(observation) = &change.linear {
                super::task_state_delivery::adopt_peer_in(
                    conn,
                    &task,
                    &serde_json::from_value(observation.body.clone())?,
                )?;
            } else if previous.get("disposition") != Some(&change.value) {
                // Only lifecycle decisions have state delivery. Cached states such
                // as started/backlog remain planning, not invented local decisions.
                if let Some(target @ ("completed" | "unstarted" | "canceled")) =
                    change.value["planning_state"].as_str()
                {
                    super::task_state_delivery::record_in(
                        conn,
                        &task,
                        &format!("peer:{id}:disposition"),
                        target,
                        None,
                        linear_predecessor(snapshot, change).as_ref(),
                    )?;
                }
            }
            continue;
        }
        let Some(field) = delivery_field(object.kind, &change.field) else {
            continue;
        };
        if let Some(observation) = &change.linear {
            owner.adopt_peer_linear(
                conn,
                field,
                id,
                change.value.clone(),
                observation.revision(),
            )?;
        } else if previous.get(&change.field) != Some(&change.value) {
            owner.record_value(
                conn,
                field,
                &format!("peer:{id}:{field}"),
                change.value.clone(),
                linear_predecessor(snapshot, change),
            )?;
        }
    }
    Ok(())
}

fn linear_predecessor(snapshot: &PlanningSnapshot, change: &PlanningMutation) -> Option<Value> {
    let mut pending: Vec<_> = change.parents.iter().map(String::as_str).collect();
    let mut visited = BTreeSet::new();
    let mut latest = None;
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let prior = &snapshot.changes[id];
        if let Some(observation) = &prior.linear {
            let candidate = (observation.revision_time(), prior.clock, id);
            if latest.is_none_or(|previous| candidate > previous) {
                latest = Some(candidate);
            }
        }
        pending.extend(prior.parents.iter().map(String::as_str));
    }
    latest.map(|(_, _, id)| {
        let prior = &snapshot.changes[id];
        serde_json::json!({"value":prior.value,"revision":prior.linear.as_ref().and_then(LinearObservation::revision)})
    })
}

fn projection_conflict(error: &StoreError) -> bool {
    match error {
        StoreError::Sqlite(rusqlite::Error::SqliteFailure(code, message)) => {
            matches!(
                code.extended_code,
                rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                    | rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY
            ) || (code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER
                && matches!(
                    message.as_deref(),
                    Some(
                        "Project would change AgentSession ancestry"
                            | "Task would change AgentSession ancestry"
                            | "selected Project cannot change Wave ownership"
                            | "selected Project belongs to another Wave"
                    )
                ))
        }
        StoreError::InvalidData(reason) => matches!(
            reason.as_str(),
            "competing planning creation receipts"
                | "comment identity already belongs to another comment"
                | "comment belongs to another Task"
                | "Wave parent would create a cycle"
                | "Wave parent is unavailable"
                | "peer provider frontier is older than retained evidence"
                | "Linear removal prevents peer Task projection"
                | "Linear archive prevents peer Project projection"
        ),
        StoreError::ProjectMembershipConflict { .. }
        | StoreError::ProviderObservationConflict { .. } => true,
        _ => false,
    }
}

fn projection_conflicts_in(
    conn: &Connection,
    repo: &str,
    destination: &str,
) -> StoreResult<Vec<PeerProjectionConflict>> {
    let mut query = conn.prepare(
        "SELECT c.kind,c.object_id,c.reason FROM planning_peer_conflicts c
         JOIN planning_members m ON m.kind=c.kind AND m.object_id=c.object_id AND m.repo=c.repo
         WHERE c.repo=?1 AND m.destination=?2 AND c.active=1
         ORDER BY c.kind,c.object_id,c.reason",
    )?;
    let mut rows = query.query(params![repo, destination])?;
    let mut conflicts = Vec::new();
    while let Some(row) = rows.next()? {
        conflicts.push(PeerProjectionConflict {
            object: PlanningObject {
                kind: serde_json::from_value(Value::String(row.get(0)?))?,
                id: row.get(1)?,
            },
            reason: row.get(2)?,
        });
    }
    Ok(conflicts)
}

fn reconcile_conflicts(
    conn: &Connection,
    repo: &str,
    destination: &str,
    conflicts: &BTreeSet<(PlanningObject, String)>,
) -> StoreResult<()> {
    for PeerProjectionConflict { object, reason } in
        projection_conflicts_in(conn, repo, destination)?
    {
        if !conflicts.contains(&(object.clone(), reason.clone())) {
            conn.execute(
                "UPDATE planning_peer_conflicts SET active=0 WHERE kind=?1 AND object_id=?2 AND reason=?3",
                params![object.kind.as_str(), object.id, reason],
            )?;
        }
    }
    for (object, reason) in conflicts {
        conn.execute(
            "INSERT INTO planning_peer_conflicts(repo,kind,object_id,reason,active) VALUES(?1,?2,?3,?4,1)
             ON CONFLICT(kind,object_id,reason) DO UPDATE SET active=1 WHERE active=0",
            params![repo,object.kind.as_str(),object.id,reason],
        )?;
    }
    Ok(())
}

fn validate_wave(
    conn: &Connection,
    object: &PlanningObject,
    winners: &WinningFields<'_>,
    repo: &str,
) -> StoreResult<()> {
    let id = crate::id::WaveId::parse(&object.id).map_err(invalid)?;
    let name = winners["name"]
        .1
        .value
        .as_str()
        .ok_or_else(|| invalid("Wave name must be text"))?;
    let parent = winners["parent_wave_id"]
        .1
        .value
        .as_str()
        .map(crate::id::WaveId::parse)
        .transpose()
        .map_err(invalid)?;
    super::validate_wave_parent(conn, &id, name, repo, parent.as_ref()).map_err(|error| {
        // Validation reads only live parents. A missing or retired parent is
        // an unresolved relationship, not an operational failure of the import.
        if matches!(
            error,
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows)
        ) {
            invalid("Wave parent is unavailable")
        } else {
            error
        }
    })
}

fn project_fields<'a>(
    conn: &Connection,
    object: &PlanningObject,
    fields: impl IntoIterator<Item = (&'a str, &'a Value)>,
) -> StoreResult<()> {
    let mut columns = BTreeMap::new();
    for (field, value) in fields {
        if field == "disposition" {
            // The merged snapshot has already validated names and grouped values.
            columns.extend(
                value
                    .as_object()
                    .expect("validated planning field group")
                    .iter()
                    .map(|(key, value)| (key.as_str(), value)),
            );
        } else {
            columns.insert(field, value);
        }
    }
    let assignments = columns
        .keys()
        .map(|key| format!("{key}=json_extract(?2,'$.{key}')"))
        .collect::<Vec<_>>()
        .join(",");
    let changed = columns
        .keys()
        .map(|key| format!("{key} IS NOT json_extract(?2,'$.{key}')"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let revision = if object.kind == PlanningKind::Task {
        ",planning_revision=planning_revision+1"
    } else {
        ""
    };
    // Names came from the portable allowlist, never arbitrary incoming SQL.
    // Advance the common optimistic revision, but not on repeated delivery.
    conn.execute(
        &format!(
            "UPDATE {} SET {assignments}{revision} WHERE id=?1 AND ({changed})",
            table(object.kind)
        ),
        params![object.id, serde_json::to_string(&columns)?],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use serde_json::json;

    use super::SqliteStore;
    use crate::durable::{ProjectId, TaskId};
    use crate::engine::planning_exchange::PlanningSnapshot;
    use crate::engine::planning_git::PlanningDestination;
    use crate::id::{ProcessLfid, TraceId, WaveId};
    use crate::ops::pm::{TaskComment, TaskCommentAuthor};
    use crate::work::wave::Wave;

    fn binding() -> PlanningDestination {
        PlanningDestination::new("/synthetic/remote", "refs/loopflow/planning/shared/fixture")
            .unwrap()
    }

    fn destination() -> String {
        binding().id()
    }

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        for repo in ["/source", "/target", "/other"] {
            store.bind_peer_planning(repo, &binding()).unwrap();
        }
        (home, store)
    }

    fn export(store: &SqliteStore, repo: &str) -> PlanningSnapshot {
        store.export_peer_planning(repo, &destination()).unwrap()
    }

    fn import(store: &SqliteStore, repo: &str, revision: &str, incoming: &PlanningSnapshot) {
        store
            .import_peer_planning(repo, &destination(), revision, incoming)
            .unwrap();
    }

    fn import_revision(store: &SqliteStore, repo: &str, destination: &str) -> Option<String> {
        store
            .peer_planning_status(repo)
            .unwrap()
            .into_iter()
            .find(|status| status.id == destination)
            .unwrap()
            .imported_revision
    }

    fn seed(store: &SqliteStore) -> TaskId {
        let wave = Wave::new(WaveId::new(), "planning".into(), "/source".into());
        store.create_wave(&wave).unwrap();
        let project = ProjectId::new();
        let task = TaskId::new();
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,project_prompt_context)
            VALUES(?1,?2,1,1,'chapter','Chapter','')", params![project.as_str(),wave.id()]).unwrap();
        conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
            VALUES(?1,?2,'FIX-1','Original','Brief',1,1,'')",params![task.as_str(),project.as_str()]).unwrap();
        super::super::project_content::capture_content(&conn, &project).unwrap();
        drop(conn);
        store
            .select_peer_waves("/source", &destination(), std::slice::from_ref(wave.id()))
            .unwrap();
        task
    }

    fn linear_seed(store: &SqliteStore) -> (WaveId, crate::store::PmSnapshotRow, TaskId) {
        let wave = Wave::new(WaveId::new(), "planning".into(), "/source".into());
        store.create_wave(&wave).unwrap();
        let mut snapshot: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.items.truncate(1);
        snapshot.items[0].revision = Some("2026-10-08T10:00:00Z".into());
        let row = crate::store::PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".into(),
            initiative: "initiative".into(),
            synced_at: 42,
            snapshot,
        };
        store.put_pm_snapshot(&row).unwrap();
        store
            .select_peer_waves("/source", &destination(), std::slice::from_ref(wave.id()))
            .unwrap();
        let task = store
            .task_by_issue(&row.snapshot.items[0].id)
            .unwrap()
            .unwrap()
            .id;
        (wave.id().clone(), row, task)
    }

    fn edit_title(store: &SqliteStore, task: &TaskId, title: &str) {
        let current = store.task(task).unwrap().unwrap();
        store
            .edit_task(
                task,
                current.plan.revision,
                &crate::pm::PmItemUpdate {
                    name: Some(title.into()),
                    ..Default::default()
                },
            )
            .unwrap();
    }

    fn complete(store: &SqliteStore, task: &TaskId) {
        let record = store.task(task).unwrap().unwrap();
        assert!(store
            .complete_task(
                &record,
                None,
                &super::super::task_work::EndMove::Set,
                None,
                None,
            )
            .unwrap());
    }

    fn acquire_comment(store: &SqliteStore, task: &TaskId, comment: &crate::pm::IssueComment) {
        let mut conn = store.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        super::super::task_comments::ingest_task_comment(&tx, task, comment, 42).unwrap();
        tx.commit().unwrap();
    }

    fn comment_receipt(
        store: &SqliteStore,
        id: &str,
    ) -> (String, bool, Option<String>, Option<String>) {
        store.conn.lock().unwrap().query_row(
            "SELECT comment_json,acknowledged,error,conflicting_comment_json FROM task_comment_deliveries WHERE comment_id=?1",
            [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).unwrap()
    }

    #[test]
    fn peer_creation_readback_reconciles_attempt_after_mapping_only_import() {
        use super::super::planning_changes::PlanningChanges;

        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, mut row, existing) = linear_seed(&source);
        let project = source.task(&existing).unwrap().unwrap().project_id;
        source
            .bind_project(&wave, row.snapshot.items[0].project_id.as_deref().unwrap())
            .unwrap();
        let task = TaskId::new();
        source
            .create_task(&crate::planning::NewTask {
                id: task.clone(),
                project_id: project.clone(),
                title: "Initial".into(),
                description: "Brief".into(),
            })
            .unwrap();
        edit_title(&source, &task, "Captured title");
        let owner = PlanningChanges::Task(&task);
        let creation = source
            .prepare_planning_export(owner, "team", "initiative")
            .unwrap();
        assert!(source
            .attempt_planning_export(owner, &creation.input, false)
            .unwrap());
        source.planning_export_error(owner, "lost reply").unwrap();
        let captured = source.pending_task_changes(&task).unwrap();
        let captured = captured.iter().find(|c| c.field == "name").unwrap();
        target
            .import_peer_planning(
                "/target",
                &destination(),
                "created",
                &export(&source, "/source"),
            )
            .unwrap();
        assert_eq!(
            target.planning_export_attempts(owner).unwrap(),
            (true, false)
        );
        let received = target
            .prepare_planning_export(owner, "other-team", "other-initiative")
            .unwrap();
        assert_eq!(received, creation);
        assert!(!target
            .attempt_planning_export(owner, &creation.input, false)
            .unwrap());
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "created", &export(&source, "/source"));
        assert_eq!(target.revisions().unwrap(), revisions);
        // Entity provenance is usable without fabricating complete-list evidence
        // or Machine placement. Positive contrary membership still blocks reads.
        assert!(target.selected_planning_project(&wave).unwrap().is_some());
        assert_eq!(
            target.project_readiness(&wave).unwrap().observed_at,
            Some(row.synced_at)
        );
        {
            let conn = target.conn.lock().unwrap();
            for table in ["pm_wave_projects", "work_placements"] {
                assert_eq!(
                    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
            }
            conn.execute("UPDATE pm_projects SET membership_unresolved=1", [])
                .unwrap();
        }
        assert!(target.selected_planning_project(&wave).is_err());
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE pm_projects SET membership_unresolved=0,archived=1",
                [],
            )
            .unwrap();
        assert!(target.selected_planning_project(&wave).is_err());
        target
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE pm_projects SET archived=0", [])
            .unwrap();
        edit_title(&source, &task, "Later save");
        let later = source.pending_task_changes(&task).unwrap();
        let later = later.iter().find(|c| c.field == "name").unwrap();
        let workflow = source.workflow(&task).unwrap();
        let state = source.task_state(&task).unwrap();
        let events = source.recent_task_events(&task, 100).unwrap();

        // A peer association without a provider body carries identity, not an
        // acknowledgement of the originating machine's creation attempt.
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                params![task.as_str(), creation.id],
            )
            .unwrap();
        source
            .import_peer_planning(
                "/source",
                &destination(),
                "mapping",
                &export(&target, "/target"),
            )
            .unwrap();
        let receipt = || {
            source.conn.lock().unwrap().query_row(
                "SELECT export_json,export_attempted,export_error FROM task_creation_intents WHERE task_id=?1",
                [task.as_str()], |r| Ok((r.get::<_,String>(0)?,r.get::<_,bool>(1)?,r.get::<_,Option<String>>(2)?)),
            ).unwrap()
        };
        let uncertain = receipt();
        assert!(uncertain.1);
        assert_eq!(uncertain.2.as_deref(), Some("lost reply"));
        assert_eq!(
            source.task(&task).unwrap().unwrap().plan.title,
            "Later save"
        );

        let mut item = row.snapshot.items[0].clone();
        item.id.clone_from(&creation.id);
        item.identifier = "NEW-1".into();
        item.name = "Captured title".into();
        item.description = "Brief".into();
        item.revision = Some("2026-10-08T11:00:00Z".into());
        row.snapshot.items = vec![item];
        row.synced_at += 1;
        target.put_pm_snapshot(&row).unwrap();
        let observed = export(&target, "/target");
        import(&source, "/source", "observed", &observed);
        assert_eq!(receipt(), (uncertain.0, true, None));
        let conn = source.conn.lock().unwrap();
        let acknowledged: bool = conn
            .query_row(
                "SELECT acknowledged FROM task_changes WHERE id=?1",
                [&captured.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(acknowledged);
        let retained: (bool, String, String) = conn
            .query_row(
                "SELECT acknowledged,value_json,conflict_json FROM task_changes WHERE id=?1",
                [&later.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(!retained.0);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&retained.1).unwrap(),
            "Later save"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&retained.2).unwrap()["value"],
            "Captured title"
        );
        drop(conn);
        assert_eq!(source.task_by_issue("NEW-1").unwrap().unwrap().id, task);
        assert_eq!(source.workflow(&task).unwrap(), workflow);
        assert_eq!(source.task_state(&task).unwrap(), state);
        assert_eq!(source.recent_task_events(&task, 100).unwrap(), events);
        let revisions = source.revisions().unwrap();
        import(&source, "/source", "observed", &observed);
        assert_eq!(source.revisions().unwrap(), revisions);
    }

    #[test]
    fn peer_creation_retains_captured_parent_after_later_move() {
        use super::super::planning_changes::PlanningChanges;
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, existing) = linear_seed(&source);
        let original = source.task(&existing).unwrap().unwrap().project_id;
        source
            .bind_project(&wave, &row.snapshot.projects[0].id)
            .unwrap();
        let task = TaskId::new();
        source
            .create_task(&crate::planning::NewTask {
                id: task.clone(),
                project_id: original.clone(),
                title: "Created".into(),
                description: "Brief".into(),
            })
            .unwrap();
        let owner = PlanningChanges::Task(&task);
        let creation = source
            .prepare_planning_export(owner, "team", "initiative")
            .unwrap();
        source
            .attempt_planning_export(owner, &creation.input, false)
            .unwrap();
        let other = Wave::new(WaveId::new(), "other".into(), "/source".into());
        source.create_wave(&other).unwrap();
        let destination_project = source.ensure_project(other.id(), "Destination").unwrap();
        source
            .select_peer_waves("/source", &destination(), std::slice::from_ref(other.id()))
            .unwrap();
        source
            .refile_unplaced_task(&task, &original, &destination_project)
            .unwrap();
        let moved = export(&source, "/source");
        import(&target, "/target", "moved", &moved);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(
            target.task(&task).unwrap().unwrap().project_id,
            destination_project
        );
        assert_eq!(
            target
                .prepare_planning_export(owner, "other", "other")
                .unwrap(),
            creation
        );
        assert_eq!(
            target
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT project_id FROM task_creation_intents WHERE task_id=?1",
                    [task.as_str()],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            original.as_str()
        );
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "moved", &moved);
        assert_eq!(target.revisions().unwrap(), revisions);
    }

    #[test]
    fn peer_project_creation_and_link_readback_preserve_later_saves_and_execution() {
        use super::super::planning_changes::PlanningChanges;

        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let wave = Wave::new(WaveId::new(), "planning".into(), "/source".into());
        source.create_wave(&wave).unwrap();
        let project = source.ensure_project(wave.id(), "Created Project").unwrap();
        source
            .select_peer_waves("/source", &destination(), std::slice::from_ref(wave.id()))
            .unwrap();
        source
            .edit_project(&project, None, Some("Captured summary"))
            .unwrap();
        let task = TaskId::new();
        source
            .create_task(&crate::planning::NewTask {
                id: task.clone(),
                project_id: project.clone(),
                title: "Work".into(),
                description: "Brief".into(),
            })
            .unwrap();
        let owner = PlanningChanges::Project(&project);
        let creation = source
            .prepare_planning_export(owner, "team", "initiative")
            .unwrap();
        assert!(source
            .attempt_planning_export(owner, &creation.input, false)
            .unwrap());
        assert!(source
            .attempt_planning_export(owner, &creation.input, true)
            .unwrap());
        source
            .planning_export_error(owner, "lost attachment response")
            .unwrap();
        let incoming = export(&source, "/source");
        import(&target, "/target", "uncertain", &incoming);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(
            target
                .prepare_planning_export(owner, "other", "other")
                .unwrap(),
            creation
        );
        assert_eq!(
            target.planning_export_attempts(owner).unwrap(),
            (true, true)
        );
        assert!(!target
            .attempt_planning_export(owner, &creation.input, true)
            .unwrap());
        assert!(!target
            .attempt_planning_export(owner, &creation.input, false)
            .unwrap());
        assert_eq!(
            target
                .conn
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM project_transitions", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        preserve_execution(&target, wave.id(), &task);
        let task_before = target.task(&task).unwrap();
        let workflow = target.workflow(&task).unwrap();
        let events = target.recent_task_events(&task, 100).unwrap();
        let execution = target.revisions().unwrap();
        target
            .edit_project(&project, None, Some("Later summary"))
            .unwrap();
        let later = target
            .pending_project_changes(&project)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "summary")
            .unwrap();

        // Mapping alone does not settle the link; accepted membership readback does.
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET external_project_id=?2 WHERE id=?1",
                params![project.as_str(), creation.id],
            )
            .unwrap();
        let error = || {
            target
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT export_error FROM projects WHERE id=?1",
                    [project.as_str()],
                    |r| r.get::<_, Option<String>>(0),
                )
                .unwrap()
        };
        assert_eq!(error().as_deref(), Some("lost attachment response"));
        let mut observed: crate::pm::PmProject =
            serde_json::from_value(creation.model.clone()).unwrap();
        observed.id = creation.id.clone();
        observed.initiative_ids = vec!["initiative".into()];
        observed.team_ids = vec!["team".into()];
        observed.revision = Some("2026-10-09T12:00:00Z".into());
        source
            .put_pm_project(wave.id(), "linear", "initiative", &observed, 100)
            .unwrap();
        let readback = export(&source, "/source");
        import(&target, "/target", "observed", &readback);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(error(), None);
        let retained: (bool, String) = target
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT acknowledged,value_json FROM project_changes WHERE id=?1",
                [&later.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(!retained.0);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&retained.1).unwrap(),
            "Later summary"
        );
        assert_eq!(target.task(&task).unwrap(), task_before);
        assert_eq!(target.workflow(&task).unwrap(), workflow);
        assert_eq!(target.recent_task_events(&task, 100).unwrap(), events);
        let revisions = target.revisions().unwrap();
        assert_eq!(revisions.sessions, execution.sessions);
        assert_eq!(revisions.flows, execution.flows);
        assert_eq!(revisions.processes, execution.processes);
        import(&target, "/target", "observed", &readback);
        assert_eq!(target.revisions().unwrap(), revisions);
        assert_eq!(
            target
                .prepare_planning_export(owner, "other", "other")
                .unwrap(),
            creation
        );

        // A later accepted move is not frozen to the creation Initiative.
        observed.initiative_ids = vec!["later-initiative".into()];
        let conn = target.conn.lock().unwrap();
        super::super::planning_export::attach_in(
            &conn,
            "/target",
            true,
            &serde_json::to_value(&observed).unwrap(),
        )
        .unwrap();
        drop(conn);
        assert_eq!(error(), None);
    }

    #[test]
    fn peer_creation_rejects_execution_payload_and_retains_competing_effects() {
        use super::super::planning_changes::PlanningChanges;
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let wave = Wave::new(WaveId::new(), "planning".into(), "/source".into());
        source.create_wave(&wave).unwrap();
        let project = source.ensure_project(wave.id(), "Project").unwrap();
        source
            .select_peer_waves("/source", &destination(), std::slice::from_ref(wave.id()))
            .unwrap();
        let owner = PlanningChanges::Project(&project);
        let creation = source
            .prepare_planning_export(owner, "team", "initiative")
            .unwrap();
        source
            .attempt_planning_export(owner, &creation.input, false)
            .unwrap();
        let base = export(&source, "/source");
        let receipt_id = base
            .winners()
            .find(|(_, c)| c.field == "creation")
            .unwrap()
            .0
            .to_string();
        let mut malformed = base.clone();
        malformed.changes.get_mut(&receipt_id).unwrap().value["export"]["model"]["process_lfid"] =
            json!("forbidden");
        assert!(target
            .import_peer_planning("/target", &destination(), "malformed", &malformed)
            .is_err());
        assert!(export(&target, "/target").changes.is_empty());
        import(&target, "/target", "base", &base);
        let mut competing = base.clone();
        let mut other = competing.changes[&receipt_id].clone();
        other.value["export"]["link_id"] = json!(uuid::Uuid::new_v4().to_string());
        other.parents.clear();
        competing.changes.insert("competing-effect".into(), other);
        import(&target, "/target", "conflict", &competing);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .iter()
            .any(|c| c.reason.contains("competing planning creation receipts")));
        assert_eq!(export(&target, "/target"), competing);
        assert_eq!(
            target
                .prepare_planning_export(owner, "other", "other")
                .unwrap(),
            creation
        );
        assert_eq!(
            target.planning_export_attempts(owner).unwrap(),
            (true, false)
        );
    }

    #[test]
    fn peer_project_content_merges_independent_fields_and_retains_losing_edits() {
        for linear in [false, true] {
            let (_source_home, source) = store();
            let (_target_home, target) = store();
            let task = if linear {
                linear_seed(&source).2
            } else {
                seed(&source)
            };
            let original = source.task(&task).unwrap().unwrap();
            let project = &original.project_id;
            let base = export(&source, "/source");
            import(&target, "/target", "base", &base);
            preserve_execution(&target, &original.wave_id, &task);
            let execution = target.revisions().unwrap();
            let task_before = target.task(&task).unwrap();
            let workflow_before = target.workflow(&task).unwrap();
            let baseline =
                super::super::project_content::read_content(&source.conn.lock().unwrap(), project)
                    .unwrap();
            let mut left = baseline.clone();
            left.krs = vec![crate::pm::PmKr {
                text: "Keep the independent KR".into(),
                holds: false,
            }];
            source.update_project_content(project, &left).unwrap();
            let mut right = baseline.clone();
            right.workflow = "peer-review".into();
            right.metric_targets = vec![crate::pm::ChapterMetricTarget {
                metric_id: "fixture/coverage".into(),
                target: crate::work::wave::metrics::MetricTarget::AtLeast { value: 1.0 },
            }];
            target.update_project_content(project, &right).unwrap();
            let incoming = export(&source, "/source");
            assert!(incoming
                .changes
                .values()
                .all(|c| c.field != "project_prompt_context"));
            let kr_mutation = incoming
                .winners()
                .find(|(_, c)| c.object.id == project.as_str() && c.field == "krs")
                .unwrap()
                .0
                .to_owned();
            import(&target, "/target", "independent", &incoming);
            right.krs = left.krs.clone();
            let merged = target.planning_project(project).unwrap();
            assert_eq!(merged.krs, right.krs);
            assert_eq!(merged.metric_targets, right.metric_targets);
            assert_eq!(merged.workflow, right.workflow);
            let receipt = target
                .pending_project_changes(project)
                .unwrap()
                .into_iter()
                .find(|c| c.field == "krs")
                .unwrap();
            assert_eq!(receipt.id, format!("peer:{kr_mutation}:krs"));
            assert_eq!(receipt.value, json!(right.krs));
            if linear {
                assert_eq!(receipt.base.as_ref().unwrap()["value"], json!(baseline.krs));
                assert_eq!(
                    receipt.base.as_ref().unwrap()["revision"],
                    json!(merged.revision)
                );
            } else {
                assert!(receipt.base.is_none());
            }
            target
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE project_changes SET attempted=1,error='lost reply' WHERE id=?1",
                    [&receipt.id],
                )
                .unwrap();
            let union = export(&target, "/target");
            import(&source, "/source", "union", &union);
            assert_eq!(source.planning_project(project).unwrap(), merged);
            import(&target, "/target", "independent", &incoming);
            let receipt_state: (bool,bool,Option<String>,Option<String>) = target.conn.lock().unwrap().query_row(
                "SELECT attempted,acknowledged,error,conflict_json FROM project_changes WHERE id=?1", [&receipt.id],
                |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
            assert_eq!(
                receipt_state,
                (true, false, Some("lost reply".into()), None)
            );
            assert_eq!(export(&target, "/target"), union);

            // Concurrent edits of the same semantic field retain both values.
            left = right.clone();
            left.krs[0].text = "Source's later KR".into();
            right.krs[0].text = "Target's later KR".into();
            source.update_project_content(project, &left).unwrap();
            target.update_project_content(project, &right).unwrap();
            let a = export(&source, "/source");
            let b = export(&target, "/target");
            let combined = a.merge(&b).unwrap();
            let winner = combined
                .winners()
                .find(|(_, c)| c.object.id == project.as_str() && c.field == "krs")
                .unwrap()
                .1
                .value
                .clone();
            import(&target, "/target", "concurrent", &a);
            import(&source, "/source", "concurrent", &b);
            for (store, repo) in [(&source, "/source"), (&target, "/target")] {
                assert_eq!(json!(store.planning_project(project).unwrap().krs), winner);
                let journal = store.export_peer_planning(repo, &destination()).unwrap();
                assert_eq!(journal, combined);
                for value in [json!(left.krs), json!(right.krs)] {
                    assert!(journal
                        .changes
                        .values()
                        .any(|c| c.object.id == project.as_str()
                            && c.field == "krs"
                            && c.value == value));
                }
            }
            assert_eq!(target.task(&task).unwrap(), task_before);
            assert_eq!(target.workflow(&task).unwrap(), workflow_before);
            let after = target.revisions().unwrap();
            assert_eq!(
                (after.sessions, after.processes, after.flows),
                (execution.sessions, execution.processes, execution.flows)
            );
        }
    }

    #[test]
    fn peer_project_content_linear_winner_retires_loser_and_retains_receipt() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        let project = source.task(&task).unwrap().unwrap().project_id;
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        let mut content =
            super::super::project_content::read_content(&target.conn.lock().unwrap(), &project)
                .unwrap();
        content.krs = vec![crate::pm::PmKr {
            text: "Retain losing local KR".into(),
            holds: false,
        }];
        target.update_project_content(&project, &content).unwrap();
        let receipt = target
            .pending_project_changes(&project)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "krs")
            .unwrap();
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE project_changes SET attempted=1,error='lost reply' WHERE id=?1",
                [&receipt.id],
            )
            .unwrap();
        let mut observed = row.snapshot.projects[0].clone();
        observed.krs = vec![crate::pm::PmKr {
            text: "Accepted provider KR".into(),
            holds: true,
        }];
        observed.revision = Some("2026-10-08T15:00:00Z".into());
        source
            .put_pm_project(&wave, "linear", "initiative", &observed, 70)
            .unwrap();
        let incoming = export(&source, "/source");
        let winner = incoming
            .winners()
            .find(|(_, c)| c.object.id == project.as_str() && c.field == "krs")
            .unwrap()
            .1;
        assert_eq!(winner.linear.as_ref().unwrap().body, json!(observed));
        import(&target, "/target", "observed", &incoming);
        let actual = target.planning_project(&project).unwrap();
        assert_eq!(actual.krs, observed.krs);
        assert_eq!(actual.workflow, observed.workflow);
        let state: (String,bool,bool,Option<String>) = target.conn.lock().unwrap().query_row(
            "SELECT value_json,attempted,acknowledged,conflict_json FROM project_changes WHERE id=?1", [&receipt.id],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&state.0).unwrap(),
            json!(content.krs)
        );
        assert!(state.1);
        assert!(!state.2);
        assert!(state.3.is_some());
        assert!(target
            .pending_project_changes(&project)
            .unwrap()
            .iter()
            .all(|c| c.field != "krs"));
        let before = export(&target, "/target");
        import(&target, "/target", "observed", &incoming);
        assert_eq!(export(&target, "/target"), before);
    }

    #[test]
    fn malformed_project_content_aborts_import_without_partial_projection() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let task = seed(&source);
        let project = source.task(&task).unwrap().unwrap().project_id;
        let base = export(&source, "/source");
        for (field, value) in [
            (
                "krs",
                json!([{"text":"Hidden execution","holds":false,"process_lfid":"forbidden"}]),
            ),
            (
                "metric_targets",
                json!([
                    {"metric_id":"duplicate","target":{"kind":"at_least","value":1}},
                    {"metric_id":"duplicate","target":{"kind":"at_most","value":2}},
                ]),
            ),
            ("workflow", json!(null)),
        ] {
            let mut invalid = base.clone();
            let change = invalid
                .changes
                .values_mut()
                .find(|c| c.object.id == project.as_str() && c.field == field)
                .unwrap();
            change.value = value;
            assert!(target
                .import_peer_planning("/target", &destination(), "invalid", &invalid)
                .is_err());
            assert!(target.task(&task).unwrap().is_none());
            assert!(target.project(&project).unwrap().is_none());
            assert!(export(&target, "/target").changes.is_empty());
            assert_eq!(import_revision(&target, "/target", &destination()), None);
        }
    }

    #[test]
    fn peer_project_content_capture_failure_rolls_back_content_and_receipts() {
        let (_home, store) = store();
        let task = seed(&store);
        let project = store.task(&task).unwrap().unwrap().project_id;
        let before = store.project(&project).unwrap();
        let journal = export(&store, "/source");
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_content_capture BEFORE INSERT ON planning_peer_changes
            WHEN NEW.kind='project' AND NEW.field='krs'
            BEGIN SELECT RAISE(ABORT,'injected semantic capture failure'); END;",
            )
            .unwrap();
        let content = crate::pm::ProjectContent {
            workflow: "review".into(),
            metric_targets: vec![],
            krs: vec![crate::pm::PmKr {
                text: "Must not partially save".into(),
                holds: false,
            }],
        };
        assert!(store.update_project_content(&project, &content).is_err());
        assert_eq!(store.project(&project).unwrap(), before);
        assert_eq!(export(&store, "/source"), journal);
        assert!(store.pending_project_changes(&project).unwrap().is_empty());
    }

    #[test]
    fn peer_status_does_not_hide_database_failures_as_a_damaged_plan() {
        let (_home, store) = store();
        seed(&store);
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TABLE planning_peer_heads")
            .unwrap();
        assert!(matches!(
            store.peer_planning_status("/source"),
            Err(crate::store::StoreError::Sqlite(_))
        ));
    }

    #[test]
    fn peer_comments_preserve_lost_replies_and_adopt_provider_edits_without_echo() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, _, task) = linear_seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        preserve_execution(&target, &wave, &task);
        let execution = target.revisions().unwrap();
        let workflow = target.workflow(&task).unwrap();
        let task_before = target.task(&task).unwrap();
        let steers = target.task_steers(&task).unwrap();
        let comment = TaskComment {
            id: "peer-comment".into(),
            body: "Keep this direction".into(),
            author: TaskCommentAuthor::Person {
                name: Some("Maya".into()),
            },
            created_at: Some("2026-10-08T10:00:00Z".into()),
        };
        source.append_task_comment(&task, &comment).unwrap();
        let authored = export(&source, "/source");
        import(&target, "/target", "authored", &authored);
        assert_eq!(
            target.pending_task_comments(&task).unwrap(),
            vec![comment.clone()]
        );
        target
            .record_comment_delivery(&comment.id, Some("lost response"))
            .unwrap();
        let uncertain = comment_receipt(&target, &comment.id);
        assert!(!uncertain.1);
        assert_eq!(uncertain.2.as_deref(), Some("lost response"));
        import(&target, "/target", "authored", &authored);
        assert_eq!(comment_receipt(&target, &comment.id), uncertain);
        let mut observed = crate::pm::IssueComment {
            id: comment.id.clone(),
            body: comment.body.clone(),
            author_id: Some("person".into()),
            author_name: Some("Maya".into()),
            created_at: comment.created_at.clone(),
            revision: Some("2026-10-08T10:01:00Z".into()),
        };
        acquire_comment(&source, &task, &observed);
        let confirmed = export(&source, "/source");
        assert!(confirmed
            .changes
            .values()
            .any(|change| change.object.id == comment.id
                && change
                    .linear
                    .as_ref()
                    .is_some_and(|fact| fact.body == serde_json::to_value(&observed).unwrap()
                        && fact.observed_at == 42)));
        import(&target, "/target", "confirmed", &confirmed);
        assert!(target.pending_task_comments(&task).unwrap().is_empty());
        let receipt = comment_receipt(&target, &comment.id);
        assert!(receipt.1);
        assert_eq!(receipt.2, None);
        assert_eq!(
            serde_json::from_str::<TaskComment>(&receipt.0).unwrap(),
            comment
        );
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "confirmed", &confirmed);
        assert_eq!(target.revisions().unwrap(), revisions);
        assert_eq!(export(&target, "/target"), confirmed);
        observed.body = "Provider correction".into();
        observed.author_name = Some("Quinn".into());
        observed.revision = Some("2026-10-08T10:02:00Z".into());
        acquire_comment(&source, &task, &observed);
        let amended = export(&source, "/source");
        import(&target, "/target", "amended", &amended);
        assert_eq!(
            target.task_comments(&task).unwrap().comments,
            vec![TaskComment::from(&observed)]
        );
        target
            .record_comment_delivery(&comment.id, Some("late failure"))
            .unwrap();
        assert_eq!(comment_receipt(&target, &comment.id), receipt);
        assert!(target.pending_task_comments(&task).unwrap().is_empty());
        assert_eq!(export(&target, "/target"), amended);
        assert_eq!(target.workflow(&task).unwrap(), workflow);
        assert_eq!(target.task(&task).unwrap(), task_before);
        assert_eq!(target.task_steers(&task).unwrap(), steers);
        let after = target.revisions().unwrap();
        assert_eq!(
            (after.sessions, after.processes, after.flows),
            (execution.sessions, execution.processes, execution.flows)
        );
    }

    #[test]
    fn peer_comments_retain_losing_delivery_and_isolate_equal_revision_conflicts() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_, _, task) = linear_seed(&source);
        let comment = TaskComment {
            id: "conflicting-comment".into(),
            body: "Original direction".into(),
            author: TaskCommentAuthor::Person {
                name: Some("Maya".into()),
            },
            created_at: Some("2026-10-08T10:00:00Z".into()),
        };
        source.append_task_comment(&task, &comment).unwrap();
        let authored = export(&source, "/source");
        import(&target, "/target", "authored", &authored);
        target
            .record_comment_delivery(&comment.id, Some("lost response"))
            .unwrap();
        let observed = crate::pm::IssueComment {
            id: comment.id.clone(),
            body: "Edited on Linear".into(),
            author_id: Some("person".into()),
            author_name: Some("Quinn".into()),
            created_at: comment.created_at.clone(),
            revision: Some("2026-10-08T10:01:00Z".into()),
        };
        acquire_comment(&source, &task, &observed);
        let provider = export(&source, "/source");
        import(&target, "/target", "provider", &provider);
        let receipt = comment_receipt(&target, &comment.id);
        assert!(!receipt.1);
        assert_eq!(
            serde_json::from_str::<TaskComment>(&receipt.0).unwrap(),
            comment
        );
        assert_eq!(
            serde_json::from_str::<crate::pm::IssueComment>(receipt.3.as_ref().unwrap()).unwrap(),
            observed
        );
        assert_eq!(
            target.task_comments(&task).unwrap().conflicts[&comment.id],
            comment.body
        );
        assert!(target.pending_task_comments(&task).unwrap().is_empty());
        // A separately accepted contradictory body at the same provider revision
        // stays journal evidence; it cannot overwrite the thread or its receipt.
        let (_other_home, other) = store();
        import(&other, "/source", "authored", &authored);
        let contradictory = crate::pm::IssueComment {
            body: "Different same revision".into(),
            ..observed.clone()
        };
        acquire_comment(&other, &task, &contradictory);
        let independent = Wave::new(WaveId::new(), "independent".into(), "/source".into());
        other.create_wave(&independent).unwrap();
        other
            .select_peer_waves(
                "/source",
                &destination(),
                std::slice::from_ref(independent.id()),
            )
            .unwrap();
        let incoming = export(&other, "/source");
        import(&target, "/target", "contradiction", &incoming);
        assert!(target.get_wave(independent.id()).unwrap().is_some());
        assert_eq!(
            target.task_comments(&task).unwrap().comments,
            vec![TaskComment::from(&observed)]
        );
        assert_eq!(comment_receipt(&target, &comment.id), receipt);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .iter()
            .any(|conflict| conflict.object.id == comment.id));
        assert_eq!(
            export(&target, "/target"),
            provider.merge(&incoming).unwrap()
        );
        assert_eq!(
            import_revision(&target, "/target", &destination()).as_deref(),
            Some("contradiction")
        );
        import(&target, "/target", "contradiction", &incoming);
        assert_eq!(comment_receipt(&target, &comment.id), receipt);
    }

    #[test]
    fn peer_comments_acquired_without_local_delivery_never_echo() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_, _, task) = linear_seed(&source);
        let comment = crate::pm::IssueComment {
            id: "provider-only".into(),
            body: "Provider direction".into(),
            author_id: Some("person".into()),
            author_name: Some("Quinn".into()),
            created_at: Some("2026-10-08T10:00:00Z".into()),
            revision: None,
        };
        acquire_comment(&source, &task, &comment);
        // Another comment's newer frontier must not suppress this unversioned
        // comment or its later edit. Provider ordering belongs to each object.
        let independent = crate::pm::IssueComment {
            id: "provider-second".into(),
            body: "Independent direction".into(),
            author_name: Some("Maya".into()),
            created_at: Some("2026-10-08T10:05:00Z".into()),
            revision: Some("2026-10-08T14:00:00Z".into()),
            ..comment.clone()
        };
        acquire_comment(&source, &task, &independent);
        let incoming = export(&source, "/source");
        import(&target, "/target", "provider", &incoming);
        let imported = target.task(&task).unwrap().unwrap();
        assert!(imported.worktree.is_none());
        assert!(imported.workspace_slug.is_empty());
        assert!(target.workflow(&task).unwrap().is_none());
        assert_eq!(
            target.task_comments(&task).unwrap().comments,
            vec![TaskComment::from(&comment), TaskComment::from(&independent)]
        );
        assert!(target.pending_task_comments(&task).unwrap().is_empty());
        let count: i64 = target
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM task_comment_deliveries", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
        assert_eq!(export(&target, "/target"), incoming);
        // A versioned edit supersedes the unversioned body without replaying it
        // as a contradictory acquisition on the next import.
        let edited = crate::pm::IssueComment {
            body: "Corrected provider direction".into(),
            revision: Some("2026-10-08T11:00:00Z".into()),
            ..comment.clone()
        };
        acquire_comment(&source, &task, &edited);
        let incoming = export(&source, "/source");
        import(&target, "/target", "edited", &incoming);
        assert_eq!(
            target.task_comments(&task).unwrap().comments,
            vec![TaskComment::from(&edited), TaskComment::from(&independent)]
        );
        assert!(target.pending_task_comments(&task).unwrap().is_empty());
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "edited", &incoming);
        assert_eq!(target.revisions().unwrap(), revisions);
        let mut malformed = incoming.clone();
        let fact = malformed
            .changes
            .values_mut()
            .find_map(|change| {
                (change.object.id == comment.id)
                    .then_some(change.linear.as_mut())
                    .flatten()
            })
            .unwrap();
        fact.body["process_lfid"] = json!("not planning");
        assert!(target
            .import_peer_planning("/target", &destination(), "malformed", &malformed)
            .is_err());
        assert_eq!(
            import_revision(&target, "/target", &destination()).as_deref(),
            Some("edited")
        );
    }

    #[test]
    fn peer_disposition_queues_delivery_without_workflow_and_retains_uncertain_attempts() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_, row, task) = linear_seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        let workflow = target.workflow(&task).unwrap();
        let state = target.task_state(&task).unwrap();
        let events = target.recent_task_events(&task, 100).unwrap();
        complete(&source, &task);
        let completed = export(&source, "/source");
        import(&target, "/target", "completed", &completed);
        assert_eq!(
            target
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .completed_at,
            source
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .completed_at,
        );
        let delivery = target.pending_task_state(&task).unwrap().unwrap();
        assert!(delivery.id.starts_with("peer:"));
        assert_eq!(delivery.target, "completed");
        assert!(!delivery.attempted);
        let baseline: (Option<i64>, Option<String>, Option<String>) = target
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT move_seq,base_revision,base_state FROM task_state_deliveries WHERE id=?1",
                [&delivery.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            baseline,
            (
                None,
                row.snapshot.items[0].revision.clone(),
                row.snapshot.items[0].state.clone()
            )
        );
        assert!(target.attempt_task_state(&delivery).unwrap());
        target.task_state_error(&delivery, "lost reply").unwrap();
        let revision = target.revisions().unwrap();
        import(&target, "/target", "completed", &completed);
        assert_eq!(target.revisions().unwrap(), revision);
        assert_eq!(
            target.pending_task_state(&task).unwrap().unwrap().id,
            delivery.id
        );

        // A later peer save supersedes delivery, never erases the uncertain effect.
        super::super::task_state_delivery::queue_in(
            &source.conn.lock().unwrap(),
            &task,
            "unstarted",
        )
        .unwrap();
        let reopened = export(&source, "/source");
        import(&target, "/target", "reopened", &reopened);
        let later = target.pending_task_state(&task).unwrap().unwrap();
        assert_ne!(later.id, delivery.id);
        assert_eq!(later.target, "unstarted");
        assert!(!later.attempted);
        let retained: (bool, bool, Option<String>) = target
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT attempted,settled,error FROM task_state_deliveries WHERE id=?1",
                [&delivery.id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(retained, (true, false, Some("lost reply".into())));
        // A response for the older completion cannot ingest over the reopening.
        let mut stale = row.snapshot.items[0].clone();
        stale.state = Some("completed".into());
        stale.completed = true;
        stale.revision = Some("2026-10-08T11:00:00Z".into());
        assert!(!target.observe_task_state(&delivery, &stale).unwrap());
        assert_eq!(
            target.pending_task_state(&task).unwrap().unwrap().id,
            later.id
        );
        super::super::task_state_delivery::queue_in(
            &source.conn.lock().unwrap(),
            &task,
            "canceled",
        )
        .unwrap();
        let canceled = export(&source, "/source");
        import(&target, "/target", "canceled", &canceled);
        assert_eq!(
            target.pending_task_state(&task).unwrap().unwrap().target,
            "canceled"
        );
        assert_eq!(target.workflow(&task).unwrap(), workflow);
        assert_eq!(target.task_state(&task).unwrap(), state);
        assert_eq!(target.recent_task_events(&task, 100).unwrap(), events);
    }

    #[test]
    fn peer_linear_disposition_retires_concurrent_intention_without_moving_workflow() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_, mut row, task) = linear_seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        complete(&target, &task);
        let delivery = target.pending_task_state(&task).unwrap().unwrap();
        let workflow = target.workflow(&task).unwrap();
        let state = target.task_state(&task).unwrap();
        // Same provider state, a newer concurrent fact: peer policy selects Linear.
        row.snapshot.items[0].revision = Some("2026-10-08T11:00:00Z".into());
        row.synced_at += 1;
        source.put_pm_snapshot(&row).unwrap();
        let observed = export(&source, "/source");
        import(&target, "/target", "observed", &observed);
        assert!(target.pending_task_state(&task).unwrap().is_none());
        let receipt: (String, bool, bool, String) = target.conn.lock().unwrap().query_row(
            "SELECT target,attempted,settled,conflict_json FROM task_state_deliveries WHERE id=?1",
            [&delivery.id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
        ).unwrap();
        assert_eq!(
            (&*receipt.0, receipt.1, receipt.2),
            ("completed", false, true)
        );
        assert_eq!(
            serde_json::from_str::<crate::pm::PmItem>(&receipt.3).unwrap(),
            row.snapshot.items[0]
        );
        assert_eq!(
            target
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .state,
            row.snapshot.items[0].state
        );
        assert_eq!(target.workflow(&task).unwrap(), workflow);
        assert_eq!(target.task_state(&task).unwrap(), state);
        let revision = target.revisions().unwrap();
        import(&target, "/target", "observed", &observed);
        assert_eq!(target.revisions().unwrap(), revision);
    }

    #[test]
    fn peer_cached_non_lifecycle_state_does_not_invent_a_delivery() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let task = seed(&source);
        source
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_state='started' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        let snapshot = export(&source, "/source");
        import(&target, "/target", "started", &snapshot);
        assert_eq!(
            target
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .state
                .as_deref(),
            Some("started")
        );
        let count: i64 = target
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM task_state_deliveries WHERE task_id=?1",
                [task.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn peer_disposition_receipt_failure_rolls_back_planning_and_checkpoint() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_, _, task) = linear_seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        complete(&source, &task);
        let completed = export(&source, "/source");
        let before = target.planning_task(&task).unwrap();
        target
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_state_receipt BEFORE INSERT ON task_state_deliveries
             WHEN NEW.id LIKE 'peer:%' BEGIN SELECT RAISE(ABORT,'receipt failure'); END;",
            )
            .unwrap();
        assert!(target
            .import_peer_planning("/target", &destination(), "completed", &completed)
            .is_err());
        assert_eq!(target.planning_task(&task).unwrap(), before);
        assert_eq!(
            import_revision(&target, "/target", &destination()).as_deref(),
            Some("base")
        );
        assert_eq!(export(&target, "/target"), base);
        assert!(target.pending_task_state(&task).unwrap().is_none());
        target
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_state_receipt")
            .unwrap();
        import(&target, "/target", "completed", &completed);
        assert_eq!(
            target.pending_task_state(&task).unwrap().unwrap().target,
            "completed"
        );
    }

    #[test]
    fn peer_field_delivery_keeps_source_baseline_identity_and_late_saves() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_, row, task) = linear_seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        assert!(target.pending_task_changes(&task).unwrap().is_empty());
        edit_title(&source, &task, "Peer title");
        let project = source.task(&task).unwrap().unwrap().project_id;
        source
            .edit_project(&project, Some("Peer project"), Some("Peer summary"))
            .unwrap();
        let incoming = export(&source, "/source");
        import(&target, "/target", "edited", &incoming);
        let receipts = target.pending_task_changes(&task).unwrap();
        let title = receipts.iter().find(|c| c.field == "name").unwrap();
        assert_eq!(title.value, "Peer title");
        assert_eq!(
            title.base.as_ref().unwrap()["value"],
            row.snapshot.items[0].name
        );
        let project_receipts = target.pending_project_changes(&project).unwrap();
        assert!(project_receipts
            .iter()
            .any(|c| c.field == "name" && c.value == "Peer project"));
        assert!(project_receipts
            .iter()
            .any(|c| c.field == "summary" && c.value == "Peer summary"));
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "edited", &incoming);
        assert_eq!(target.pending_task_changes(&task).unwrap(), receipts);
        assert_eq!(
            target.pending_project_changes(&project).unwrap(),
            project_receipts
        );
        assert_eq!(target.revisions().unwrap(), revisions);
        // Repeating an older acquisition cannot erase a save made after import.
        edit_title(&target, &task, "Later local save");
        let later = target.pending_task_changes(&task).unwrap();
        import(&target, "/target", "edited", &incoming);
        assert_eq!(
            target.task(&task).unwrap().unwrap().plan.title,
            "Later local save"
        );
        assert_eq!(target.pending_task_changes(&task).unwrap(), later);
    }

    #[test]
    fn peer_linear_winner_retires_delivery_without_acknowledging_uncertain_write() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        target
            .import_peer_planning(
                "/target",
                &destination(),
                "base",
                &export(&source, "/source"),
            )
            .unwrap();
        edit_title(&target, &task, "Uncertain local title");
        let receipt = target
            .pending_task_changes(&task)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "name")
            .unwrap();
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE task_changes SET attempted=1,error='lost response' WHERE id=?1",
                [&receipt.id],
            )
            .unwrap();
        let mut item = row.snapshot.items[0].clone();
        item.name = "Linear title".into();
        item.revision = Some("2026-10-08T11:00:00Z".into());
        source
            .put_pm_task(
                "/source",
                "linear",
                &crate::store::PmTaskRecord {
                    item,
                    project: Some(row.snapshot.projects[0].clone()),
                    observed_at: 43,
                },
                Some((&wave, "initiative")),
            )
            .unwrap();
        let incoming = export(&source, "/source");
        import(&target, "/target", "linear", &incoming);
        assert_eq!(
            target.task(&task).unwrap().unwrap().plan.title,
            "Linear title"
        );
        assert!(target.pending_task_changes(&task).unwrap().is_empty());
        let conn = target.conn.lock().unwrap();
        let saved: (bool, bool, String, String, String) = conn.query_row(
            "SELECT attempted,acknowledged,value_json,conflict_json,error FROM task_changes WHERE id=?1",
            [&receipt.id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
        ).unwrap();
        assert!(saved.0);
        assert!(!saved.1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved.2).unwrap(),
            "Uncertain local title"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved.3).unwrap()["value"],
            "Linear title"
        );
        assert_eq!(saved.4, "lost response");
        drop(conn);
        let status = target.task_planning_sync(&task).unwrap();
        assert!(status
            .changes
            .iter()
            .any(|c| c.id == receipt.id
                && c.state == crate::planning::PlanningSyncState::AdoptedLinear));
        let revision = target.revisions().unwrap();
        import(&target, "/target", "linear", &incoming);
        assert_eq!(target.revisions().unwrap(), revision);
    }

    #[test]
    fn peer_linear_frontier_survives_older_and_equal_value_acquisition_without_echo() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        target
            .import_peer_planning(
                "/target",
                &destination(),
                "base",
                &export(&source, "/source"),
            )
            .unwrap();
        // Execution remains a local fact throughout provider/peer reconciliation.
        preserve_execution(&target, &wave, &task);
        edit_title(&target, &task, "Uncertain local title");
        let receipt = target
            .pending_task_changes(&task)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "name")
            .unwrap();
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE task_changes SET attempted=1,error='lost response' WHERE id=?1",
                [&receipt.id],
            )
            .unwrap();
        let mut record = crate::store::PmTaskRecord {
            item: row.snapshot.items[0].clone(),
            project: Some(row.snapshot.projects[0].clone()),
            observed_at: 43,
        };
        record.item.name = "Linear winner".into();
        record.item.revision = Some("2026-10-08T11:00:00Z".into());
        source
            .put_pm_task("/source", "linear", &record, Some((&wave, "initiative")))
            .unwrap();
        // No planning value changes in this second observation. Its frontier must
        // nevertheless cross the peer boundary and fence a later older response.
        record.item.revision = Some("2026-10-08T12:00:00Z".into());
        record.observed_at = 44;
        source
            .put_pm_task("/source", "linear", &record, Some((&wave, "initiative")))
            .unwrap();
        let incoming = export(&source, "/source");
        let execution_before = target.revisions().unwrap();
        import(&target, "/target", "newer", &incoming);
        let before_reads = export(&target, "/target");
        let retained = target.task(&task).unwrap().unwrap();
        assert_eq!(retained.plan.title, "Linear winner");
        assert_eq!(
            retained.worktree.as_deref(),
            Some(std::path::Path::new("/retained/work"))
        );
        let execution_after = target.revisions().unwrap();
        assert_eq!(execution_before.sessions, execution_after.sessions);
        assert_eq!(execution_before.processes, execution_after.processes);
        assert_eq!(execution_before.flows, execution_after.flows);
        for (revision, title) in [
            ("2026-10-08T10:00:00Z", row.snapshot.items[0].name.as_str()),
            ("2026-10-08T11:00:00Z", "Linear winner"),
            ("2026-10-08T12:00:00Z", "Linear winner"),
        ] {
            let mut delayed = record.clone();
            delayed.item.revision = Some(revision.into());
            delayed.item.name = title.into();
            delayed.observed_at = 100; // receipt age cannot order provider facts
            target
                .put_pm_task("/target", "linear", &delayed, Some((&wave, "initiative")))
                .unwrap();
            assert_eq!(
                target.task(&task).unwrap().unwrap().plan.title,
                "Linear winner"
            );
            let conn = target.conn.lock().unwrap();
            let frontier: String = conn.query_row(
                "SELECT json_extract(body,'$.revision') FROM pm_items WHERE repo='/target' AND id=?1",
                [&record.item.id], |row| row.get(0),
            ).unwrap();
            assert_eq!(frontier, "2026-10-08T12:00:00Z");
            let saved: (bool, bool, String, String) = conn
                .query_row(
                    "SELECT attempted,acknowledged,error,value_json FROM task_changes WHERE id=?1",
                    [&receipt.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();
            assert_eq!(
                saved,
                (
                    true,
                    false,
                    "lost response".into(),
                    json!("Uncertain local title").to_string()
                )
            );
        }
        assert_eq!(export(&target, "/target"), before_reads);
        assert!(target.pending_task_changes(&task).unwrap().is_empty());
        // A same-revision contradiction rejects atomically, retaining frontier,
        // receipt history and the existing imported journal.
        record.item.name = "Conflicting same revision".into();
        assert!(target
            .put_pm_task("/target", "linear", &record, Some((&wave, "initiative")))
            .is_err());
        assert_eq!(export(&target, "/target"), before_reads);
    }

    #[test]
    fn peer_task_same_revision_conflict_retains_evidence_and_independent_imports() {
        assert_peer_provider_conflict_isolated(false);
    }

    #[test]
    fn peer_project_same_revision_conflict_retains_evidence_and_independent_imports() {
        assert_peer_provider_conflict_isolated(true);
    }

    fn assert_peer_provider_conflict_isolated(project_conflict: bool) {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        let project = target.task(&task).unwrap().unwrap().project_id;
        preserve_execution(&target, &wave, &task);

        // Each machine legitimately accepted a different body at the same new
        // provider revision. Neither journal is a malformed document.
        for (store, repo, title) in [
            (&source, "/source", "Peer observation"),
            (&target, "/target", "Retained observation"),
        ] {
            if project_conflict {
                let mut observed = row.snapshot.projects[0].clone();
                observed.name = title.into();
                observed.revision = Some("2026-10-08T11:00:00Z".into());
                store
                    .put_pm_project(&wave, "linear", "initiative", &observed, 60)
                    .unwrap();
            } else {
                let mut item = row.snapshot.items[0].clone();
                item.name = title.into();
                item.revision = Some("2026-10-08T11:00:00Z".into());
                store
                    .put_pm_task(
                        repo,
                        "linear",
                        &crate::store::PmTaskRecord {
                            item,
                            project: None,
                            observed_at: 60,
                        },
                        Some((&wave, "initiative")),
                    )
                    .unwrap();
            }
        }
        edit_title(&target, &task, "Uncertain local title");
        target
            .edit_project(&project, Some("Uncertain local Project"), None)
            .unwrap();
        {
            let conn = target.conn.lock().unwrap();
            conn.execute(
                "UPDATE task_changes SET attempted=1,error='lost response' WHERE task_id=?1",
                [task.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE project_changes SET attempted=1,error='lost response' WHERE project_id=?1",
                [project.as_str()],
            )
            .unwrap();
        }
        let task_receipts = target.pending_task_changes(&task).unwrap();
        let project_receipts = target.pending_project_changes(&project).unwrap();
        assert!(!task_receipts.is_empty());
        assert!(!project_receipts.is_empty());
        let retained_task = target.task(&task).unwrap().unwrap();
        let retained_project = target.project(&project).unwrap().unwrap();
        let workflow = target.workflow(&task).unwrap();
        let execution = target.revisions().unwrap();
        let journal = export(&target, "/target");
        let (table, provider_id, rejected_id) = if project_conflict {
            (
                "pm_projects",
                &row.snapshot.projects[0].id,
                project.as_str(),
            )
        } else {
            ("pm_items", &row.snapshot.items[0].id, task.as_str())
        };
        let frontier = || -> (String, i64) {
            target.conn.lock().unwrap().query_row(
                &format!("SELECT body,observed_at FROM {table} WHERE repo='/target' AND provider='linear' AND id=?1"),
                [provider_id], |r| Ok((r.get(0)?, r.get(1)?)),
            ).unwrap()
        };
        let accepted = frontier();
        let independent = Wave::new(WaveId::new(), "independent".into(), "/source".into());
        source.create_wave(&independent).unwrap();
        source
            .select_peer_waves(
                "/source",
                &destination(),
                std::slice::from_ref(independent.id()),
            )
            .unwrap();
        let incoming = export(&source, "/source");
        incoming.validate().unwrap();
        let retained = journal.merge(&incoming).unwrap();

        import(&target, "/target", "conflicting", &incoming);
        assert!(target.get_wave(independent.id()).unwrap().is_some());
        assert_eq!(frontier(), accepted);
        assert_eq!(target.task(&task).unwrap().unwrap(), retained_task);
        assert_eq!(target.project(&project).unwrap().unwrap(), retained_project);
        assert_eq!(target.workflow(&task).unwrap(), workflow);
        assert_eq!(target.pending_task_changes(&task).unwrap(), task_receipts);
        for (owner, receipts) in [("task", &task_receipts), ("project", &project_receipts)] {
            for receipt in receipts {
                let state: (bool, bool, Option<String>, Option<String>) = target.conn.lock().unwrap().query_row(
                    &format!("SELECT attempted,acknowledged,conflict_json,error FROM {owner}_changes WHERE id=?1"),
                    [&receipt.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                ).unwrap();
                assert_eq!(state, (true, false, None, Some("lost response".into())));
            }
        }
        assert_eq!(
            target.pending_project_changes(&project).unwrap(),
            project_receipts
        );
        let after = target.revisions().unwrap();
        assert_eq!(
            (after.sessions, after.processes, after.flows),
            (execution.sessions, execution.processes, execution.flows)
        );
        assert_eq!(export(&target, "/target"), retained);
        assert_eq!(
            import_revision(&target, "/target", &destination()).as_deref(),
            Some("conflicting")
        );
        let conflicts = target.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].object.id, rejected_id);
        assert!(conflicts[0].reason.contains("unordered or conflicting"));
        // Reading back a retained conflict must not append mutations, change
        // its reason or manufacture a fresh acquisition.
        import(&target, "/target", "conflicting", &incoming);
        assert_eq!(target.revisions().unwrap(), after);
        assert_eq!(
            target.peer_projection_conflicts("/target").unwrap(),
            conflicts
        );
    }

    #[test]
    fn malformed_peer_observations_roll_back_independent_records_and_checkpoint() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        preserve_execution(&target, &wave, &task);
        edit_title(&target, &task, "Retained local title");
        let receipts = target.pending_task_changes(&task).unwrap();
        let journal = export(&target, "/target");
        let before = target.revisions().unwrap();
        let retained_task = target.task(&task).unwrap().unwrap();
        let workflow = target.workflow(&task).unwrap();
        let mut item = row.snapshot.items[0].clone();
        item.name = "New observation".into();
        item.revision = Some("2026-10-08T11:00:00Z".into());
        source
            .put_pm_task(
                "/source",
                "linear",
                &crate::store::PmTaskRecord {
                    item,
                    project: None,
                    observed_at: 60,
                },
                Some((&wave, "initiative")),
            )
            .unwrap();
        let independent = Wave::new(WaveId::new(), "independent".into(), "/source".into());
        source.create_wave(&independent).unwrap();
        source
            .select_peer_waves(
                "/source",
                &destination(),
                std::slice::from_ref(independent.id()),
            )
            .unwrap();
        let incoming = export(&source, "/source");
        for defect in ["revision", "execution", "value"] {
            let mut malformed = incoming.clone();
            let change = malformed
                .changes
                .values_mut()
                .find(|change| {
                    change.object.id == task.as_str()
                        && change.field == "issue_title"
                        && change.value == "New observation"
                })
                .unwrap();
            match defect {
                "revision" => {
                    change.linear.as_mut().unwrap().body["revision"] = json!("not a revision")
                }
                "execution" => change.linear.as_mut().unwrap().body["worktree"] = json!("/private"),
                "value" => change.value = json!("Not the observed value"),
                _ => unreachable!(),
            }
            let error = target
                .import_peer_planning("/target", &destination(), "malformed", &malformed)
                .unwrap_err();
            assert!(matches!(error, crate::store::StoreError::InvalidData(_)));
            assert!(target.get_wave(independent.id()).unwrap().is_none());
            assert_eq!(target.task(&task).unwrap().unwrap(), retained_task);
            assert_eq!(target.workflow(&task).unwrap(), workflow);
            assert_eq!(target.pending_task_changes(&task).unwrap(), receipts);
            assert_eq!(export(&target, "/target"), journal);
            assert_eq!(
                import_revision(&target, "/target", &destination()).as_deref(),
                Some("base")
            );
            assert!(target
                .peer_projection_conflicts("/target")
                .unwrap()
                .is_empty());
            assert_eq!(target.revisions().unwrap(), before);
        }
    }

    #[test]
    fn peer_rejection_preserves_removed_tasks_and_archived_projects() {
        for (archived, new_frontier) in [(false, false), (false, true), (true, false), (true, true)]
        {
            let (_source_home, source) = store();
            let (_target_home, target) = store();
            let (wave, row, task) = linear_seed(&source);
            let base = export(&source, "/source");
            import(&target, "/target", "base", &base);
            let project = target.task(&task).unwrap().unwrap().project_id;
            preserve_execution(&target, &wave, &task);
            if archived {
                target
                    .confirm_pm_project_archival("/target", "linear", &row.snapshot.projects[0], 60)
                    .unwrap();
            } else {
                target
                    .observe_pm_issue_change(
                        &row.snapshot.items[0].id,
                        Some("2026-10-08T13:00:00Z"),
                        true,
                    )
                    .unwrap();
            }
            let retained_task = target.task(&task).unwrap().unwrap();
            let retained_project = target.project(&project).unwrap().unwrap();
            let workflow = target.workflow(&task).unwrap();
            let execution = target.revisions().unwrap();
            let journal = export(&target, "/target");
            // An unrelated selected Wave must still enter the same import.
            let independent = Wave::new(WaveId::new(), "independent".into(), "/source".into());
            source.create_wave(&independent).unwrap();
            source
                .select_peer_waves(
                    "/source",
                    &destination(),
                    std::slice::from_ref(independent.id()),
                )
                .unwrap();
            if !new_frontier {
                // Identical cached bodies must not bypass later removal/archive.
                if archived {
                    source
                        .edit_project(&project, Some("Rejected local name"), None)
                        .unwrap();
                } else {
                    edit_title(&source, &task, "Rejected local title");
                }
            } else if archived {
                let mut changed = row.snapshot.projects[0].clone();
                changed.name = "Must not replace archived Project".into();
                changed.revision = Some("2026-10-08T14:00:00Z".into());
                source
                    .put_pm_project(&wave, "linear", "initiative", &changed, 70)
                    .unwrap();
            } else {
                let mut item = row.snapshot.items[0].clone();
                item.name = "Must not replace removed Task".into();
                item.revision = Some("2026-10-08T14:00:00Z".into());
                source
                    .put_pm_task(
                        "/source",
                        "linear",
                        &crate::store::PmTaskRecord {
                            item,
                            project: None,
                            observed_at: 70,
                        },
                        Some((&wave, "initiative")),
                    )
                    .unwrap();
            }
            let incoming = export(&source, "/source");
            import(&target, "/target", "rejected", &incoming);
            assert!(target.get_wave(independent.id()).unwrap().is_some());
            assert_eq!(target.task(&task).unwrap().unwrap(), retained_task);
            assert_eq!(target.project(&project).unwrap().unwrap(), retained_project);
            assert_eq!(target.workflow(&task).unwrap(), workflow);
            let after = target.revisions().unwrap();
            assert_eq!(
                (after.sessions, after.processes, after.flows),
                (execution.sessions, execution.processes, execution.flows)
            );
            assert_eq!(
                export(&target, "/target"),
                journal.merge(&incoming).unwrap()
            );
            assert_eq!(
                import_revision(&target, "/target", &destination()).as_deref(),
                Some("rejected")
            );
            let rejected_id = if archived {
                project.as_str()
            } else {
                task.as_str()
            };
            let conflicts = target.peer_projection_conflicts("/target").unwrap();
            assert!(conflicts
                .iter()
                .any(|conflict| conflict.object.id == rejected_id));
            import(&target, "/target", "rejected", &incoming);
            assert_eq!(target.revisions().unwrap(), after);
            assert_eq!(
                target.peer_projection_conflicts("/target").unwrap(),
                conflicts
            );
        }
    }

    #[test]
    fn peer_membership_conflict_survives_projection_rollback_and_independent_import() {
        // Neither a newer entity revision nor an unproven relationship-only edit
        // supplies the separately ordered membership evidence.
        for provider_observation in [false, true] {
            let (_source_home, source) = store();
            let (_target_home, target) = store();
            let (wave, row, task) = linear_seed(&source);
            let base = export(&source, "/source");
            import(&target, "/target", "base", &base);
            let project = target.task(&task).unwrap().unwrap().project_id;
            preserve_execution(&target, &wave, &task);
            let retained = target.project(&project).unwrap().unwrap();
            let retained_task = target.task(&task).unwrap().unwrap();
            let execution = target.revisions().unwrap();
            let workflow = target.workflow(&task).unwrap();
            if provider_observation {
                let mut changed = row.snapshot.projects[0].clone();
                changed.team_ids = vec!["other-team".into()];
                changed.name = "Newer entity with unordered membership".into();
                changed.revision = Some("2099-01-01T00:00:00Z".into());
                source
                    .reconcile_pm_project_teams(&wave, "linear", "initiative", &changed, 70)
                    .unwrap();
            } else {
                source
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE projects SET planning_teams='[\"other-team\"]' WHERE id=?1",
                        [project.as_str()],
                    )
                    .unwrap();
            }
            let independent = Wave::new(WaveId::new(), "independent".into(), "/source".into());
            source.create_wave(&independent).unwrap();
            source
                .select_peer_waves(
                    "/source",
                    &destination(),
                    std::slice::from_ref(independent.id()),
                )
                .unwrap();
            let incoming = export(&source, "/source");
            import(&target, "/target", "disputed", &incoming);
            assert!(target.get_wave(independent.id()).unwrap().is_some());
            assert_eq!(target.project(&project).unwrap().unwrap(), retained);
            assert_eq!(target.task(&task).unwrap().unwrap(), retained_task);
            assert_eq!(target.workflow(&task).unwrap(), workflow);
            let after = target.revisions().unwrap();
            assert_eq!(
                (after.sessions, after.processes, after.flows),
                (execution.sessions, execution.processes, execution.flows)
            );
            let conn = target.conn.lock().unwrap();
            let (body, unresolved): (String, bool) = conn.query_row(
                "SELECT body,membership_unresolved FROM pm_projects WHERE repo='/target' AND id=?1",
                [&row.snapshot.projects[0].id], |row| Ok((row.get(0)?, row.get(1)?)),
            ).unwrap();
            assert!(unresolved);
            assert_eq!(
                serde_json::from_str::<crate::pm::PmProject>(&body).unwrap(),
                row.snapshot.projects[0]
            );
            drop(conn);
            assert_eq!(export(&target, "/target"), incoming);
            let conflicts = target.peer_projection_conflicts("/target").unwrap();
            assert!(conflicts
                .iter()
                .any(|conflict| conflict.object.id == project.as_str()
                    && conflict.reason.contains("relationship ordering evidence")));
            import(&target, "/target", "disputed", &incoming);
            assert_eq!(target.revisions().unwrap(), after);
            assert_eq!(
                target.peer_projection_conflicts("/target").unwrap(),
                conflicts
            );
        }
    }

    fn preserve_execution(store: &SqliteStore, wave: &WaveId, task: &TaskId) {
        let driver = ProcessLfid::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
                VALUES('retained','Retained','human',1,0,'/retained/work',?1,?2)", params![task.as_str(),wave]).unwrap();
            conn.execute(
                "UPDATE tasks SET worktree='/retained/work' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,?2,1)",
                params![driver, TraceId::new()],
            )
            .unwrap();
        }
        let definition = crate::engine::workflow::WorkflowDefinition {
            name: "review".into(),
            nodes: vec![crate::engine::workflow::WorkflowNode {
                name: "review".into(),
                skill: "review".into(),
                description: None,
            }],
            edges: Vec::new(),
        };
        store
            .take_up_workflow(task, &definition, &driver, None)
            .unwrap();
        store
            .set_workflow_node(task, "review", &driver, None)
            .unwrap();
    }

    #[test]
    fn equal_value_provider_revision_does_not_relabel_a_pending_local_value() {
        let (_home, source) = store();
        let (wave, row, task) = linear_seed(&source);
        edit_title(&source, &task, "Saved locally");
        let receipt = source.pending_task_changes(&task).unwrap();
        let mut item = row.snapshot.items[0].clone();
        item.revision = Some("2026-10-08T12:00:00Z".into());
        source
            .put_pm_task(
                "/source",
                "linear",
                &crate::store::PmTaskRecord {
                    item,
                    project: Some(row.snapshot.projects[0].clone()),
                    observed_at: 45,
                },
                Some((&wave, "initiative")),
            )
            .unwrap();
        assert_eq!(
            source.task(&task).unwrap().unwrap().plan.title,
            "Saved locally"
        );
        assert_eq!(source.pending_task_changes(&task).unwrap(), receipt);
        let snapshot = export(&source, "/source");
        let (_, title) = snapshot
            .winners()
            .find(|(_, change)| change.object.id == task.as_str() && change.field == "issue_title")
            .unwrap();
        assert!(title.linear.is_none());
        assert!(snapshot.changes.values().any(|change| {
            change.object.id == task.as_str()
                && change.linear.as_ref().is_some_and(|observation| {
                    observation.revision() == Some("2026-10-08T12:00:00Z")
                })
        }));
    }

    #[test]
    fn peer_project_frontier_keeps_equal_value_revision_and_rejects_old_detail() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, _) = linear_seed(&source);
        let mut project = row.snapshot.projects[0].clone();
        project.name = "Current Project".into();
        for revision in ["2026-10-08T11:00:00Z", "2026-10-08T12:00:00Z"] {
            project.revision = Some(revision.into());
            source
                .put_pm_project(&wave, "linear", "initiative", &project, 45)
                .unwrap();
        }
        target
            .import_peer_planning(
                "/target",
                &destination(),
                "project",
                &export(&source, "/source"),
            )
            .unwrap();
        let before = export(&target, "/target");
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "project", &before);
        assert_eq!(target.revisions().unwrap(), revisions);
        project.name = "Delayed Project".into();
        project.revision = Some("2026-10-08T11:30:00Z".into());
        let accepted = target
            .put_pm_project(&wave, "linear", "initiative", &project, 100)
            .unwrap();
        assert_eq!(accepted.name, "Current Project");
        assert_eq!(accepted.revision.as_deref(), Some("2026-10-08T12:00:00Z"));
        assert_eq!(export(&target, "/target"), before);
    }

    #[test]
    fn peer_local_winner_preserves_older_attempt_and_receipts_rollback_with_import() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let task = seed(&source);
        let base = export(&source, "/source");
        import(&target, "/target", "base", &base);
        edit_title(&target, &task, "Attempted");
        let receipt = target
            .pending_task_changes(&task)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "name")
            .unwrap();
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE task_changes SET attempted=1 WHERE id=?1",
                [&receipt.id],
            )
            .unwrap();
        source
            .import_peer_planning(
                "/source",
                &destination(),
                "attempted",
                &export(&target, "/target"),
            )
            .unwrap();
        edit_title(&source, &task, "Successor");
        let incoming = export(&source, "/source");
        let before = target.pending_task_changes(&task).unwrap();
        let journal_before = export(&target, "/target");
        target
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fail_peer_receipt BEFORE INSERT ON task_changes
             WHEN NEW.id LIKE 'peer:%' BEGIN SELECT RAISE(ABORT,'receipt failure'); END;",
            )
            .unwrap();
        assert!(target
            .import_peer_planning("/target", &destination(), "successor", &incoming)
            .is_err());
        assert_eq!(target.task(&task).unwrap().unwrap().plan.title, "Attempted");
        assert_eq!(target.pending_task_changes(&task).unwrap(), before);
        assert_eq!(export(&target, "/target"), journal_before);
        assert_eq!(
            import_revision(&target, "/target", &destination()).as_deref(),
            Some("base")
        );
        target
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_peer_receipt")
            .unwrap();
        import(&target, "/target", "successor", &incoming);
        let successor = target
            .pending_task_changes(&task)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "name")
            .unwrap();
        assert_eq!(successor.value, "Successor");
        assert_ne!(successor.id, receipt.id);
        let conn = target.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT attempted,acknowledged,conflict_json FROM task_changes WHERE id=?1",
                [&receipt.id],
                |r| Ok((
                    r.get::<_, bool>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, Option<String>>(2)?
                ))
            )
            .unwrap(),
            (true, false, None)
        );
        drop(conn);
        // The common sender cannot start the successor until the older effect is
        // observed. That readback then advances its unchanged baseline, not its value.
        assert!(!target
            .attempt_planning_field(super::PlanningChanges::Task(&task), &successor, None,)
            .unwrap());
        let conn = target.conn.lock().unwrap();
        let accepted: serde_json::Value = super::PlanningChanges::Task(&task)
            .reconcile(
                &conn,
                &json!({"name":"Attempted","revision":"2026-10-08T12:00:00Z"}),
            )
            .unwrap();
        assert_eq!(accepted["name"], "Successor");
        assert!(conn
            .query_row(
                "SELECT acknowledged FROM task_changes WHERE id=?1",
                [&receipt.id],
                |r| r.get::<_, bool>(0)
            )
            .unwrap());
        drop(conn);
        let pending = target.pending_task_changes(&task).unwrap();
        let retained = pending.iter().find(|c| c.id == successor.id).unwrap();
        assert_eq!(retained.value, "Successor");
        assert_eq!(retained.base.as_ref().unwrap()["value"], "Attempted");
    }

    #[test]
    fn a_shared_endpoint_does_not_transfer_repository_ownership() {
        let (_home, store) = store();
        let task = seed(&store);
        let before = export(&store, "/source");
        let error = store
            .import_peer_planning("/target", &destination(), "foreign", &before)
            .unwrap_err();
        assert!(error.to_string().contains("another repository"), "{error}");
        assert_eq!(import_revision(&store, "/target", &destination()), None);
        assert_eq!(export(&store, "/source"), before);
        assert!(store.task(&task).unwrap().is_some());
    }

    #[test]
    fn refiling_to_private_work_holds_only_affected_history_and_keeps_receiving() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let original = left.task(&task).unwrap().unwrap();
        let sibling = TaskId::new();
        left.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,updated_at,workspace_slug,issue_description) VALUES(?1,?2,'FIX-2','Sibling',1,1,'','')",
            params![sibling.as_str(), original.project_id.as_str()],
        ).unwrap();
        let base = export(&left, "/source");
        import(&right, "/target", "base", &base);
        let private = Wave::new(WaveId::new(), "private".into(), "/source".into());
        left.create_wave(&private).unwrap();
        let private_project = ProjectId::new();
        {
            let conn = left.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,status,project_slug,project_name,project_prompt_context) VALUES(?1,?2,1,'started','fixture','Fixture','')",
                params![private_project.as_str(), private.id()],
            )
            .unwrap();
            super::super::project_content::capture_content(&conn, &private_project).unwrap();
            conn.execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                params![private.id(), private_project.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE projects SET status='started' WHERE id=?1",
                [original.project_id.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                params![original.wave_id, original.project_id.as_str()],
            )
            .unwrap();
        }
        left.refile_unplaced_task(&task, &original.project_id, &private_project)
            .unwrap();
        left.append_task_comment(
            &task,
            &TaskComment {
                id: "private-comment".into(),
                body: "Private after move".into(),
                author: TaskCommentAuthor::Person {
                    name: Some("Author".into()),
                },
                created_at: Some("2026-10-08T12:00:00Z".into()),
            },
        )
        .unwrap();
        let held = left.peer_projection_conflicts("/source").unwrap();
        assert_eq!(held.len(), 2);
        assert!(held.iter().any(|c| c.object.id == task.as_str()));
        let outgoing = export(&left, "/source");
        let bytes = String::from_utf8(outgoing.to_bytes().unwrap()).unwrap();
        assert!(!bytes.contains(private_project.as_str()));
        assert!(!bytes.contains("Private after move"));
        assert!(!outgoing.objects().iter().any(|o| o.id == task.as_str()));
        import(&right, "/target", "held", &outgoing);
        // Omission retains the peer's last shared Task, not a deletion or a move.
        assert_eq!(
            right.task(&task).unwrap().unwrap().project_id,
            original.project_id
        );
        let remote = right.task(&sibling).unwrap().unwrap();
        right
            .edit_task(
                &sibling,
                remote.plan.revision,
                &crate::pm::PmItemUpdate {
                    name: Some("Independent remote edit".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        let received = export(&right, "/target");
        import(&left, "/source", "remote", &received);
        let portable = export(&left, "/source");
        assert!(!portable.objects().iter().any(|o| o.id == task.as_str()));
        assert_eq!(
            left.task(&task).unwrap().unwrap().project_id,
            private_project
        );
        assert_eq!(
            left.task(&sibling).unwrap().unwrap().plan.title,
            "Independent remote edit"
        );
        assert_eq!(
            import_revision(&left, "/source", &destination()).as_deref(),
            Some("remote")
        );
        assert_eq!(left.peer_projection_conflicts("/source").unwrap(), held);
        assert!(left
            .pending_task_changes(&task)
            .unwrap()
            .iter()
            .any(|c| c.field == "project_id"));
        // Returning to the shared parent does not leak the losing private reference.
        left.refile_unplaced_task(&task, &private_project, &original.project_id)
            .unwrap();
        assert!(!export(&left, "/source")
            .objects()
            .iter()
            .any(|o| o.id == task.as_str()));
        // Only explicit selection makes that retained history exchangeable again.
        left.select_peer_waves(
            "/source",
            &destination(),
            std::slice::from_ref(private.id()),
        )
        .unwrap();
        let released = export(&left, "/source");
        assert!(released.objects().iter().any(|o| o.id == task.as_str()));
        assert!(left
            .peer_projection_conflicts("/source")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn held_wave_selection_cycles_keep_descendants_private_until_explicit_selection() {
        let (_home, store) = store();
        let task = seed(&store);
        let saved = store.task(&task).unwrap().unwrap();
        let private = Wave::new(WaveId::new(), "private".into(), "/source".into());
        let independent = Wave::new(WaveId::new(), "independent".into(), "/source".into());
        store.create_wave(&private).unwrap();
        store.create_wave(&independent).unwrap();
        store
            .append_task_comment(
                &task,
                &TaskComment {
                    id: "held-comment".into(),
                    body: "Retain this with the Task".into(),
                    author: TaskCommentAuthor::Person { name: None },
                    created_at: Some("2026-10-08T12:00:00Z".into()),
                },
            )
            .unwrap();
        // The Wave selects its Project, which references the Wave. Holding the
        // Wave must also hold the Project, Task and comment without looping.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE waves SET parent_wave_id=?2,current_project_id=?3 WHERE id=?1",
                params![saved.wave_id, private.id(), saved.project_id.as_str()],
            )
            .unwrap();
        let held = store.peer_projection_conflicts("/source").unwrap();
        assert_eq!(held.len(), 4);
        // An existing hold cannot veto explicit selection of independent work.
        store
            .select_peer_waves(
                "/source",
                &destination(),
                std::slice::from_ref(independent.id()),
            )
            .unwrap();
        let outgoing = export(&store, "/source");
        assert!(outgoing
            .objects()
            .iter()
            .all(|object| object.id == independent.id().as_str()));
        assert!(!outgoing.changes.is_empty());
        assert!(store.task(&task).unwrap().is_some());
        assert_eq!(store.task_comments(&task).unwrap().comments.len(), 1);
        store
            .select_peer_waves(
                "/source",
                &destination(),
                std::slice::from_ref(private.id()),
            )
            .unwrap();
        assert!(store
            .peer_projection_conflicts("/source")
            .unwrap()
            .is_empty());
        let released = export(&store, "/source");
        assert!(held
            .iter()
            .all(|conflict| released.objects().contains(&conflict.object)));
    }

    #[test]
    fn provider_move_to_unselected_project_preserves_execution_and_sibling_exchange() {
        let (_home, store) = store();
        let wave = Wave::new(WaveId::new(), "planning".into(), "/source".into());
        store.create_wave(&wave).unwrap();
        let mut snapshot: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.items.truncate(2);
        for item in &mut snapshot.items {
            item.state = Some("unstarted".into());
        }
        let row = crate::store::PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".into(),
            initiative: "initiative".into(),
            synced_at: 42,
            snapshot,
        };
        store.put_pm_snapshot(&row).unwrap();
        store
            .select_peer_waves("/source", &destination(), std::slice::from_ref(wave.id()))
            .unwrap();
        let task = store
            .task_by_issue(&row.snapshot.items[0].id)
            .unwrap()
            .unwrap();
        let sibling = store
            .task_by_issue(&row.snapshot.items[1].id)
            .unwrap()
            .unwrap();
        // Existing private Project enters this Wave without selecting its history.
        let private_wave = Wave::new(WaveId::new(), "private".into(), "/source".into());
        store.create_wave(&private_wave).unwrap();
        let private_id = ProjectId::new();
        let mut project = row.snapshot.projects[0].clone();
        project.id = "private-project".into();
        project.slug = "private-project".into();
        project.name = "Private project".into();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at,project_slug,project_name,project_prompt_context) VALUES(?1,?2,?3,1,'fixture','Fixture','')", params![private_id.as_str(),private_wave.id(),project.id]).unwrap();
            super::super::project_content::capture_content(&conn, &private_id).unwrap();
            conn.execute(
                "UPDATE projects SET wave_id=?2 WHERE id=?1",
                params![private_id.as_str(), wave.id()],
            )
            .unwrap();
        }
        preserve_execution(&store, wave.id(), &task.id);
        let workflow = store.workflow(&task.id).unwrap();
        let mut item = row.snapshot.items[0].clone();
        item.project_id = Some(project.id.clone());
        item.project = Some(project.slug.clone());
        item.revision = Some("2026-10-08T12:00:00Z".into());
        store
            .put_pm_task(
                "/source",
                "linear",
                &crate::store::PmTaskRecord {
                    item,
                    project: Some(project),
                    observed_at: 43,
                },
                Some((wave.id(), "initiative")),
            )
            .unwrap();
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().project_id,
            private_id
        );
        let outgoing = export(&store, "/source");
        assert!(!outgoing.objects().iter().any(|o| o.id == task.id.as_str()));
        assert!(outgoing
            .objects()
            .iter()
            .any(|o| o.id == sibling.id.as_str()));
        assert!(!String::from_utf8(outgoing.to_bytes().unwrap())
            .unwrap()
            .contains(private_id.as_str()));
        assert!(store
            .peer_projection_conflicts("/source")
            .unwrap()
            .iter()
            .any(|c| c.object.id == task.id.as_str()));
        let (_peer_home, peer) = self::store();
        import(&peer, "/target", "provider-move", &outgoing);
        assert_eq!(
            peer.task(&sibling.id).unwrap().unwrap().plan.title,
            sibling.plan.title
        );
        assert_eq!(store.workflow(&task.id).unwrap(), workflow);
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT worktree FROM tasks WHERE id=?1",
                [task.id.as_str()],
                |r| { r.get::<_, String>(0) }
            )
            .unwrap(),
            "/retained/work"
        );
        assert_eq!(
            conn.query_row(
                "SELECT task_id FROM agent_sessions WHERE id='retained'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            task.id.as_str()
        );
        assert!(conn.query_row("SELECT EXISTS(SELECT 1 FROM planning_peer_changes WHERE kind='task' AND object_id=?1 AND field='project_id' AND linear IS NOT NULL AND json_extract(value,'$')=?2)", params![task.id.as_str(),private_id.as_str()], |r| r.get::<_,bool>(0)).unwrap());
    }

    #[test]
    fn export_validates_only_the_selected_plans_causal_heads() {
        let (_home, store) = store();
        seed(&store);
        let before = export(&store, "/source");
        let unrelated = Wave::new(WaveId::new(), "private".into(), "/source".into());
        store.create_wave(&unrelated).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM planning_peer_heads WHERE object_id=?1",
                [unrelated.id()],
            )
            .unwrap();
        assert_eq!(export(&store, "/source"), before);

        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM planning_peer_heads WHERE id=?1",
                [before.changes.keys().next().unwrap()],
            )
            .unwrap();
        let error = store
            .export_peer_planning("/source", &destination())
            .unwrap_err();
        assert!(
            error.to_string().contains("causal heads disagree"),
            "{error}"
        );
    }

    #[test]
    fn joining_a_plan_keeps_unrelated_local_work_out_of_its_publication() {
        use crate::engine::planning_git::{PlanningGit, PlanningPublication};
        use loopflow_test_support::TestRepo;

        let remote = TestRepo::new();
        let destination = PlanningDestination::resolve(
            remote.path(),
            "origin",
            "refs/loopflow/planning/shared/join-test",
        )
        .unwrap();
        let id = destination.id();
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let shared = Wave::new(WaveId::new(), "shared".into(), "/source".into());
        left.create_wave(&shared).unwrap();
        left.bind_peer_planning("/source", &destination).unwrap();
        left.select_peer_waves("/source", &id, std::slice::from_ref(shared.id()))
            .unwrap();
        let private = Wave::new(WaveId::new(), "private".into(), "/target".into());
        right.create_wave(&private).unwrap();
        let private_project = ProjectId::new();
        let private_task = TaskId::new();
        {
            let conn = right.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,project_slug,project_name,project_prompt_context) VALUES(?1,?2,1,'fixture','Fixture','')",
                params![private_project.as_str(), private.id()],
            )
            .unwrap();
            super::super::project_content::capture_content(&conn, &private_project).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,worktree,updated_at,workspace_slug,issue_description) VALUES(?1,?2,'PRIVATE-1','Private draft',1,'/retained/private',1,'','')", params![private_task.as_str(),private_project.as_str()]).unwrap();
        }
        right.bind_peer_planning("/target", &destination).unwrap();
        assert!(right
            .export_peer_planning("/target", &id)
            .unwrap()
            .changes
            .is_empty());
        let transport = PlanningGit::new(remote.path(), &destination).unwrap();
        let initial = transport
            .save(
                &left
                    .export_peer_planning("/source", &id)
                    .unwrap()
                    .to_bytes()
                    .unwrap(),
                None,
                None,
            )
            .unwrap();
        assert_eq!(
            transport.publish(&initial.revision).unwrap(),
            PlanningPublication::Confirmed
        );
        let fetched = transport.fetch().unwrap().unwrap();
        let received = PlanningSnapshot::from_bytes(&fetched.bytes).unwrap();
        right
            .import_peer_planning("/target", &id, fetched.revision.as_str(), &received)
            .unwrap();
        // A new Task under the shared Wave inherits selection. The private Task
        // remains readable/editable but neither its contents nor history escapes.
        let project = ProjectId::new();
        let task = TaskId::new();
        {
            let conn = right.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,project_slug,project_name,project_prompt_context) VALUES(?1,?2,1,'fixture','Fixture','')",
                params![project.as_str(), shared.id()],
            )
            .unwrap();
            super::super::project_content::capture_content(&conn, &project).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,updated_at,workspace_slug,issue_description) VALUES(?1,?2,'SHARED-1','Shared work',1,1,'','')", params![task.as_str(),project.as_str()]).unwrap();
            conn.execute(
                "UPDATE tasks SET issue_title='Still private' WHERE id=?1",
                [private_task.as_str()],
            )
            .unwrap();
        }
        let selected = right.export_peer_planning("/target", &id).unwrap();
        assert!(selected
            .objects()
            .iter()
            .any(|object| object.id == task.as_str()));
        assert!(
            !selected
                .objects()
                .iter()
                .any(|object| object.id == private_task.as_str()
                    || object.id == private.id().as_str())
        );
        let published = transport
            .save(
                &selected.to_bytes().unwrap(),
                Some(&initial.revision),
                Some(&fetched.revision),
            )
            .unwrap();
        assert_eq!(
            transport.publish(&published.revision).unwrap(),
            PlanningPublication::Confirmed
        );
        let readback =
            PlanningSnapshot::from_bytes(&transport.fetch().unwrap().unwrap().bytes).unwrap();
        assert_eq!(readback, selected);
        assert_eq!(
            right
                .planning_task(&private_task)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Still private"
        );
        assert_eq!(
            right
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT worktree FROM tasks WHERE id=?1",
                    [private_task.as_str()],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "/retained/private"
        );
    }

    #[test]
    fn joining_cannot_claim_existing_ids_or_references_from_another_plan() {
        let (_home, store) = store();
        let task = seed(&store);
        let original = export(&store, "/source");
        let shared =
            PlanningDestination::new("/synthetic/remote", "refs/loopflow/planning/shared/other")
                .unwrap();
        let id = store.bind_peer_planning("/source", &shared).unwrap();
        assert!(store
            .import_peer_planning("/source", &id, "overlap", &original)
            .is_err());
        assert!(store
            .export_peer_planning("/source", &id)
            .unwrap()
            .changes
            .is_empty());
        assert_eq!(import_revision(&store, "/source", &id), None);
        let mut referencing = original.clone();
        referencing
            .changes
            .retain(|_, change| change.object.id == task.as_str());
        let other_task = TaskId::new();
        for change in referencing.changes.values_mut() {
            change.object.id = other_task.to_string();
        }
        // Re-identify mutations as well, leaving the reference to the private Project.
        referencing.changes = referencing
            .changes
            .into_iter()
            .map(|(id, change)| (format!("new-{id}"), change))
            .collect();
        assert!(store
            .import_peer_planning("/source", &id, "private-parent", &referencing)
            .is_err());
        assert!(store.task(&other_task).unwrap().is_none());
        assert_eq!(export(&store, "/source"), original);
    }

    #[test]
    fn unselected_rows_and_retained_journals_cannot_be_claimed_on_join() {
        let (_source_home, source) = store();
        let task = seed(&source);
        let incoming = export(&source, "/source");
        let (_target_home, target) = store();
        // Reproduce an already-known local identity, without selecting it.
        let wave_id = incoming
            .objects()
            .iter()
            .find(|object| object.kind == crate::engine::planning_exchange::PlanningKind::Wave)
            .unwrap()
            .id
            .clone();
        target
            .create_wave(&Wave::new(
                WaveId::parse(&wave_id).unwrap(),
                "Local name".into(),
                "/target".into(),
            ))
            .unwrap();
        assert!(target
            .import_peer_planning("/target", &destination(), "collision", &incoming)
            .is_err());
        assert!(export(&target, "/target").changes.is_empty());
        assert!(target.task(&task).unwrap().is_none());
        target
            .conn
            .lock()
            .unwrap()
            .execute("DELETE FROM waves WHERE id=?1", [&wave_id])
            .unwrap();
        // Removing the live row cannot make its retained losing edits publishable.
        assert!(target
            .import_peer_planning("/target", &destination(), "retained-collision", &incoming)
            .is_err());
        assert_eq!(import_revision(&target, "/target", &destination()), None);
    }

    #[test]
    fn importing_one_destination_does_not_clear_another_destinations_conflicts() {
        let (_first_home, first) = store();
        let task = seed(&first);
        let (_second_home, second) = store();
        seed(&second);
        let (_target_home, target) = store();
        target
            .create_wave(&Wave::new(
                WaveId::new(),
                "planning".into(),
                "/target".into(),
            ))
            .unwrap();
        let other =
            PlanningDestination::new("/synthetic/remote", "refs/loopflow/planning/shared/other")
                .unwrap();
        let other_id = target.bind_peer_planning("/target", &other).unwrap();
        target
            .import_peer_planning(
                "/target",
                &destination(),
                "first",
                &export(&first, "/source"),
            )
            .unwrap();
        let first_conflicts = target.peer_projection_conflicts("/target").unwrap();
        assert!(!first_conflicts.is_empty());
        target
            .import_peer_planning("/target", &other_id, "second", &export(&second, "/source"))
            .unwrap();
        let both = target.peer_projection_conflicts("/target").unwrap();
        assert!(both.len() > first_conflicts.len());
        for conflict in first_conflicts {
            assert!(both.contains(&conflict));
        }
        import(&target, "/target", "retry", &Default::default());
        assert_eq!(target.peer_projection_conflicts("/target").unwrap(), both);

        let conflicts_for = |id: &str| {
            target
                .peer_planning_status("/target")
                .unwrap()
                .into_iter()
                .find(|status| status.id == id)
                .unwrap()
                .conflicts
        };
        let second_conflicts = conflicts_for(&other_id);
        let wave_id = first.task(&task).unwrap().unwrap().wave_id;
        first
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE waves SET name='resolved' WHERE id=?1", [&wave_id])
            .unwrap();
        target
            .import_peer_planning(
                "/target",
                &destination(),
                "resolved",
                &export(&first, "/source"),
            )
            .unwrap();
        assert!(conflicts_for(&destination()).is_empty());
        assert_eq!(conflicts_for(&other_id), second_conflicts);
        assert!(target.task(&task).unwrap().is_some());
    }

    #[test]
    fn active_selection_routes_only_future_roots_and_preserves_descendants_on_switch() {
        let (home, store) = store();
        let private = Wave::new(WaveId::new(), "private".into(), "/source".into());
        store.create_wave(&private).unwrap();
        store
            .use_peer_planning("/source", Some(&destination()))
            .unwrap();
        let selected = Wave::new(WaveId::new(), "selected".into(), "/source".into());
        store.create_wave(&selected).unwrap();
        let private_child = Wave::new(WaveId::new(), "child".into(), "/source".into())
            .with_parent(private.id().clone());
        store.create_wave(&private_child).unwrap();
        let foreign = Wave::new(WaveId::new(), "foreign".into(), "/other".into());
        store.create_wave(&foreign).unwrap();
        assert_eq!(
            store.peer_planning_status("/source").unwrap()[0].selected_records,
            1
        );
        let revisions = store.revisions().unwrap();
        store
            .use_peer_planning("/source", Some(&destination()))
            .unwrap();
        assert_eq!(store.revisions().unwrap(), revisions);
        assert!(store.use_peer_planning("/source", Some("missing")).is_err());
        assert!(store.peer_planning_status("/source").unwrap()[0].active);
        drop(store);
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        assert!(store.peer_planning_status("/source").unwrap()[0].active);
        store.use_peer_planning("/source", None).unwrap();
        let child = Wave::new(WaveId::new(), "child".into(), "/source".into())
            .with_parent(selected.id().clone());
        store.create_wave(&child).unwrap();
        let local = Wave::new(WaveId::new(), "local".into(), "/source".into());
        store.create_wave(&local).unwrap();
        let status = store.peer_planning_status("/source").unwrap();
        assert!(!status[0].active);
        assert_eq!(status[0].selected_records, 2);
        let exported = export(&store, "/source");
        assert_eq!(exported.objects().len(), 2);
        assert!(exported
            .objects()
            .iter()
            .all(|object| object.id == selected.id().as_str() || object.id == child.id().as_str()));
    }

    #[test]
    fn importing_a_root_keeps_its_destination_even_when_another_plan_is_active() {
        let (_source_home, source) = store();
        seed(&source);
        let incoming = export(&source, "/source");
        let (_target_home, target) = store();
        let other =
            PlanningDestination::new("/synthetic/other", "refs/loopflow/planning/shared/other")
                .unwrap();
        let other_id = target.bind_peer_planning("/target", &other).unwrap();
        target
            .use_peer_planning("/target", Some(&other_id))
            .unwrap();
        import(&target, "/target", "incoming", &incoming);
        assert!(target
            .export_peer_planning("/target", &other_id)
            .unwrap()
            .changes
            .is_empty());
        assert_eq!(export(&target, "/target"), incoming);
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "incoming", &incoming);
        assert_eq!(target.revisions().unwrap(), revisions);
        let statuses = target.peer_planning_status("/target").unwrap();
        let imported = statuses.iter().find(|s| s.id == destination()).unwrap();
        assert!(!imported.active);
        assert_eq!(imported.imported_revision.as_deref(), Some("incoming"));
    }

    #[test]
    fn recovered_user_key_and_destination_survive_reopening_without_reselection() {
        let (home, store) = store();
        let key = "00000000-0000-4000-8000-000000000001";
        let binding = PlanningDestination::new(
            "/synthetic/remote",
            &format!("refs/loopflow/planning/users/{key}"),
        )
        .unwrap();
        assert_eq!(store.planning_user_key().unwrap(), None);
        assert!(store.bind_peer_planning("/source", &binding).is_err());
        store.provision_planning_user_key(key).unwrap();
        store.provision_planning_user_key(key).unwrap();
        let id = store.bind_peer_planning("/source", &binding).unwrap();
        assert!(store
            .provision_planning_user_key("00000000-0000-4000-8000-000000000002")
            .is_err());
        drop(store);
        let reopened = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        assert_eq!(reopened.planning_user_key().unwrap().as_deref(), Some(key));
        assert_eq!(
            reopened.peer_planning_destination("/source", &id).unwrap(),
            Some(binding.clone())
        );
        let revisions = reopened.revisions().unwrap();
        reopened.bind_peer_planning("/source", &binding).unwrap();
        assert_eq!(reopened.revisions().unwrap(), revisions);
        let (_other_home, other) = self::store();
        other.provision_planning_user_key(key).unwrap();
        assert_eq!(other.bind_peer_planning("/target", &binding).unwrap(), id);
    }

    #[test]
    fn peer_migration_preserves_the_populated_predecessor() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations(version TEXT PRIMARY KEY,applied_at INTEGER NOT NULL);",
        )
        .unwrap();
        let mut upgrade = None;
        for migration in crate::store::migration_catalog::MIGRATIONS {
            if let Some((before, after)) = migration.sql.split_once("-- draft: planning_peers") {
                conn.execute_batch(before).unwrap();
                upgrade = Some(after.to_string());
                break;
            }
            conn.execute_batch(migration.sql).unwrap();
        }
        if upgrade.is_none() {
            for draft in crate::build_info::migration_draft_manifest() {
                if draft.name == "planning_peers" {
                    upgrade = Some(draft.sql.to_string());
                    break;
                }
                conn.execute_batch(draft.sql).unwrap();
            }
        }
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('00000000-0000-0000-0000-000000000001','planning','/fixture',1);
            INSERT INTO projects(id,wave_id,created_at) VALUES('project','00000000-0000-0000-0000-000000000001',1);
            INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,worktree,agent)
            VALUES('task','project','FIX-1','Retain identity',1,'/retained','codex');
            INSERT INTO task_comments(id,task_id,body,author,created_at,provider_revision)
            VALUES('acquired','task','Provider body','{\"kind\":\"integration\"}',NULL,'2026-10-08T10:00:00Z');",
        )
        .unwrap();
        let retained_content = "workflow: review\n\n## KRs\n- [x] Keep the accepted proof\n";
        conn.execute(
            "UPDATE projects SET project_prompt_context=?1,workflow='review' WHERE id='project'",
            [retained_content],
        )
        .unwrap();
        let created = ProjectId::new();
        conn.execute("INSERT INTO projects(id,wave_id,created_at,project_name,project_slug,project_prompt_context) VALUES(?1,'00000000-0000-0000-0000-000000000001',1,'Created','created','')", [created.as_str()]).unwrap();
        let model = super::super::plan_read::project_in(&conn, &created).unwrap();
        let id = uuid::Uuid::parse_str(created.as_str().strip_prefix("proj_").unwrap())
            .unwrap()
            .to_string();
        let receipt = json!({"id":id,"model":model,"through":1,"initiative":"initiative","link_id":uuid::Uuid::new_v4().to_string(),
            "input":{"id":id,"teamIds":["team"],"name":"Created","description":"", "content":crate::pm::render_project_content(&crate::pm::ProjectContent {
                workflow:String::new(),krs:vec![],metric_targets:vec![],
            }),"useDefaultTemplate":false}});
        conn.execute("INSERT INTO project_changes(seq,id,project_id,field,value_json) VALUES(1,'captured',?1,'name','\"Created\"'),(2,'later',?1,'name','\"Later\"')", [created.as_str()]).unwrap();
        conn.execute("INSERT INTO project_transitions(wave_id,successor_id,created_at,export_json,export_attempted,export_link_attempted,export_error)
            VALUES('00000000-0000-0000-0000-000000000001',?1,1,?2,1,1,'lost attachment')", params![created.as_str(),receipt.to_string()]).unwrap();
        conn.execute_batch(&upgrade.expect("peer draft or materialized migration"))
            .unwrap();
        super::super::project_content::seed_peer_content(&conn).unwrap();
        let (body, attempted, linked, error): (String,bool,bool,String) = conn.query_row(
            "SELECT export_json,export_attempted,export_link_attempted,export_error FROM projects WHERE id=?1", [created.as_str()],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        let migrated: super::super::planning_export::PlanningExport =
            serde_json::from_str(&body).unwrap();
        assert_eq!(migrated.captured, ["captured".to_string()].into());
        assert_eq!(migrated.input, receipt["input"]);
        assert!(attempted && linked);
        assert_eq!(error, "lost attachment");
        assert_eq!(
            conn.query_row(
                "SELECT settled_at FROM project_transitions WHERE successor_id=?1",
                [created.as_str()],
                |r| r.get::<_, Option<i64>>(0)
            )
            .unwrap(),
            None
        );
        assert!(conn
            .prepare("SELECT export_json FROM project_transitions")
            .is_err());

        conn.execute("INSERT INTO planning_destinations(repo,id,endpoint,reference) VALUES('/fixture',?1,?2,?3)",
            params![destination(),binding().endpoint(),binding().reference()]).unwrap();
        assert!(super::export_in(&conn, "/fixture", &destination())
            .unwrap()
            .changes
            .is_empty());
        conn.execute(
            "INSERT INTO planning_members(kind,object_id,repo,destination)
            SELECT DISTINCT kind,object_id,'/fixture',?1 FROM planning_peer_changes
            UNION SELECT 'comment',id,'/fixture',?1 FROM task_comments",
            [destination()],
        )
        .unwrap();
        let before = super::export_in(&conn, "/fixture", &destination()).unwrap();
        assert!(!before.changes.is_empty());
        let content: std::collections::BTreeMap<_, _> = before
            .winners()
            .filter(|(_, c)| c.object.id == "project")
            .map(|(_, c)| (c.field.as_str(), c.value.clone()))
            .collect();
        assert_eq!(content["workflow"], json!("review"));
        assert_eq!(
            content["krs"],
            json!([{"text":"Keep the accepted proof","holds":true}])
        );
        assert_eq!(content["metric_targets"], json!([]));
        assert!(!content.contains_key("project_prompt_context"));
        assert_eq!(
            conn.query_row(
                "SELECT project_prompt_context FROM projects WHERE id='project'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            retained_content
        );
        assert!(!before
            .changes
            .values()
            .any(|change| change.object.id == "acquired" && change.field == "content"));
        let acquired = crate::pm::IssueComment {
            id: "acquired".into(),
            body: "Provider body".into(),
            author_id: None,
            author_name: None,
            created_at: None,
            revision: Some("2026-10-08T10:00:00Z".into()),
        };
        super::super::task_comments::ingest_task_comment(
            &conn,
            &TaskId::from_raw("task"),
            &acquired,
            42,
        )
        .unwrap();
        let before = super::export_in(&conn, "/fixture", &destination()).unwrap();
        assert!(before
            .changes
            .values()
            .any(|change| change.object.id == "acquired" && change.linear.is_some()));
        let deliveries: i64 = conn
            .query_row("SELECT count(*) FROM task_comment_deliveries", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(deliveries, 0);
        conn.execute("UPDATE tasks SET issue_title='Changed' WHERE id='task'", [])
            .unwrap();
        let after = super::export_in(&conn, "/fixture", &destination()).unwrap();
        assert_eq!(after.changes.len(), before.changes.len() + 1);
        let retained: (String, String) = conn
            .query_row(
                "SELECT worktree,agent FROM tasks WHERE id='task'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(retained, ("/retained".into(), "codex".into()));
    }

    #[test]
    fn peer_import_keeps_identity_execution_and_concurrent_local_saves() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = export(&left, "/source");
        assert_eq!(base, export(&left, "/source"));
        import(&right, "/target", "first", &base);
        assert_eq!(export(&right, "/target"), base);
        let driver = ProcessLfid::new();
        {
            let conn = right.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET worktree='/retained/checkout',agent='codex' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO processes(lfid,trace_id,started_at,completed_at,outcome) VALUES(?1,?2,1,2,'succeeded')",params![driver,TraceId::new()]).unwrap();
        }
        let definition = crate::engine::workflow::WorkflowDefinition {
            name: "review".into(),
            nodes: vec![crate::engine::workflow::WorkflowNode {
                name: "review".into(),
                skill: "review".into(),
                description: None,
            }],
            edges: Vec::new(),
        };
        right
            .take_up_workflow(&task, &definition, &driver, None)
            .unwrap();
        right
            .set_workflow_node(&task, "review", &driver, None)
            .unwrap();
        let workflow = right.workflow(&task).unwrap();
        {
            let conn = left.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET issue_title='New title',planning_completed=1,planning_state='completed' WHERE id=?1",[task.as_str()]).unwrap();
        }
        {
            let conn = right.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET issue_description='Concurrent brief' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        }
        let comment = TaskComment {
            id: "comment-1".into(),
            body: "Retain the direction".into(),
            author: TaskCommentAuthor::Person {
                name: Some("Maya".into()),
            },
            created_at: Some("2026-10-08T12:00:00Z".into()),
        };
        left.append_task_comment(&task, &comment).unwrap();
        let incoming = export(&left, "/source");
        import(&right, "/target", "second", &incoming);
        let merged = export(&right, "/target");
        let revisions = right.revisions().unwrap();
        import(&right, "/target", "second", &incoming);
        assert_eq!(export(&right, "/target"), merged);
        assert_eq!(right.revisions().unwrap(), revisions);
        assert_eq!(
            import_revision(&right, "/target", &destination()).as_deref(),
            Some("second")
        );
        let record = right.planning_task(&task).unwrap().record.unwrap();
        assert_eq!(record.item.name, "New title");
        assert_eq!(record.item.description, "Concurrent brief");
        assert!(record.item.completed);
        assert_eq!(right.task_comments(&task).unwrap().comments, vec![comment]);
        assert_eq!(right.workflow(&task).unwrap(), workflow);
        let conn = right.conn.lock().unwrap();
        let placement: (String, String) = conn
            .query_row(
                "SELECT worktree,agent FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(placement, ("/retained/checkout".into(), "codex".into()));
        drop(conn);
        import(&left, "/source", "third", &merged);
        assert_eq!(export(&left, "/source"), export(&right, "/target"));
    }

    #[test]
    fn duplicate_provider_identity_retains_pending_objects_without_blocking_other_tasks() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let independent = TaskId::new();
        let legacy = TaskId::new();
        let project: String = left
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT project_id FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        {
            let conn = left.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET external_issue_id='provider-1' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,updated_at,workspace_slug,issue_description) VALUES(?1,?2,'FIX-2','Independent',1,1,'','')", params![independent.as_str(), project.as_str()]).unwrap();
        }
        left.append_task_comment(
            &task,
            &TaskComment {
                id: "pending-comment".into(),
                body: "Retain even without a projected Task".into(),
                author: TaskCommentAuthor::Person {
                    name: Some("Maya".into()),
                },
                created_at: Some("2026-10-08T12:00:00Z".into()),
            },
        )
        .unwrap();
        let incoming = export(&left, "/source");
        let mut parents = incoming.clone();
        parents.changes.retain(|_, change| {
            matches!(
                change.object.kind,
                crate::engine::planning_exchange::PlanningKind::Wave
                    | crate::engine::planning_exchange::PlanningKind::Project
            )
        });
        import(&right, "/target", "parents", &parents);
        right.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,created_at,worktree,updated_at,workspace_slug,issue_description) VALUES(?1,?2,'provider-1','FIX-1','Legacy identity',1,'/legacy/checkout',1,'','')",
            params![legacy.as_str(), project.as_str()],
        ).unwrap();
        let expected = export(&right, "/target").merge(&incoming).unwrap();
        import(&right, "/target", "conflict", &incoming);
        assert!(right.task(&task).unwrap().is_none());
        assert_eq!(
            right
                .planning_task(&independent)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Independent"
        );
        assert_eq!(
            right
                .planning_task(&legacy)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Legacy identity"
        );
        let conflicts = right.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts
            .iter()
            .any(|c| c.object.id == task.as_str() && c.reason.contains("external_issue_id")));
        assert!(conflicts.iter().any(|c| c.object.id == "pending-comment"));
        let exported = export(&right, "/target");
        assert_eq!(exported, expected);
        let revisions = right.revisions().unwrap();
        import(&right, "/target", "conflict", &incoming);
        assert_eq!(right.revisions().unwrap(), revisions);
        assert_eq!(
            right.peer_projection_conflicts("/target").unwrap(),
            conflicts
        );
        // Pending identities cannot be claimed by another repository.
        assert!(right
            .import_peer_planning("/other", &destination(), "cross-repo", &incoming)
            .is_err());

        // An explicit mapping repair allows a later acquisition to project the
        // retained journal even if that acquisition supplies no new mutations.
        right
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET external_issue_id=NULL WHERE id=?1",
                [legacy.as_str()],
            )
            .unwrap();
        import(&right, "/target", "repaired", &Default::default());
        assert!(right
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(right.task_comments(&task).unwrap().comments.len(), 1);
        let conn = right.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT worktree FROM tasks WHERE id=?1",
                [legacy.as_str()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "/legacy/checkout"
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM planning_peer_conflicts WHERE active=0",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn protected_session_ancestry_does_not_block_independent_planning() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = export(&left, "/source");
        import(&right, "/target", "base", &base);
        let (project, wave): (String, String) = right.conn.lock().unwrap().query_row(
            "SELECT t.project_id,p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        right.conn.lock().unwrap().execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
             VALUES('session-retained','Retained','human',1,0,'/target/task',?1,?2)",
            params![task.as_str(), wave],
        ).unwrap();
        let moved_wave = Wave::new(WaveId::new(), "destination".into(), "/source".into());
        left.create_wave(&moved_wave).unwrap();
        left.select_peer_waves(
            "/source",
            &destination(),
            std::slice::from_ref(moved_wave.id()),
        )
        .unwrap();
        let moved_project = ProjectId::new();
        let independent = TaskId::new();
        {
            let conn = left.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,project_slug,project_name,project_prompt_context) VALUES(?1,?2,1,'fixture','Fixture','')",
                params![moved_project.as_str(), moved_wave.id()],
            )
            .unwrap();
            super::super::project_content::capture_content(&conn, &moved_project).unwrap();
            conn.execute(
                "UPDATE tasks SET project_id=?2 WHERE id=?1",
                params![task.as_str(), moved_project.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,updated_at,workspace_slug,issue_description) VALUES(?1,?2,'FIX-2','Still progresses',1,1,'','')", params![independent.as_str(),project.as_str()]).unwrap();
        }
        let incoming = export(&left, "/source");
        let expected = export(&right, "/target").merge(&incoming).unwrap();
        import(&right, "/target", "move", &incoming);
        assert_eq!(
            right
                .planning_task(&independent)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Still progresses"
        );
        let conflicts = right.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].reason.contains("AgentSession ancestry"));
        let exported = export(&right, "/target");
        assert_eq!(exported, expected);
        let conn = right.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT project_id FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            project
        );
        assert_eq!(
            conn.query_row(
                "SELECT wave_id FROM agent_sessions WHERE id='session-retained'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            wave
        );
    }

    #[test]
    fn wave_selection_waits_for_projects_and_reports_conflicts_without_revision_churn() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let (project, wave): (String, String) = left.conn.lock().unwrap().query_row(
            "SELECT t.project_id,p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                params![wave, project.as_str()],
            )
            .unwrap();
        let incoming = export(&left, "/source");
        import(&right, "/target", "selected", &incoming);
        assert!(right
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(
            right
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT current_project_id FROM waves WHERE id=?1",
                    [&wave],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            project
        );

        // A concurrent selection can name an unavailable Project. The selection
        // stays pending; an existing selected Project is never erased to fit it.
        let mut missing = incoming.clone();
        let (id, parent) = missing
            .changes
            .iter()
            .find(|(_, change)| {
                change.field == "current_project_id" && change.value == json!(project)
            })
            .unwrap();
        let mut selection = parent.clone();
        selection.parents = [id.clone()].into();
        selection.clock += 1;
        selection.value = json!(ProjectId::new().as_str());
        missing
            .changes
            .insert("missing-selection".into(), selection);
        import(&right, "/target", "pending-selection", &missing);
        assert_eq!(right.peer_projection_conflicts("/target").unwrap().len(), 1);
        let revisions = right.revisions().unwrap();
        import(&right, "/target", "pending-selection", &missing);
        assert_eq!(right.revisions().unwrap(), revisions);
        assert_eq!(
            right
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT current_project_id FROM waves WHERE id=?1",
                    [&wave],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            project
        );
    }

    #[test]
    fn missing_wave_parent_retains_the_existing_wave_while_tasks_progress() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = export(&left, "/source");
        import(&right, "/target", "base", &base);
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET issue_title='Independent edit' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        let mut incoming = export(&left, "/source");
        let (id, previous) = incoming
            .changes
            .iter()
            .find(|(_, change)| change.field == "parent_wave_id")
            .unwrap();
        let wave = previous.object.id.clone();
        let mut parent = previous.clone();
        parent.parents = [id.clone()].into();
        parent.clock += 1;
        parent.value = json!(WaveId::new().as_str());
        incoming.changes.insert("missing-parent".into(), parent);
        let expected = export(&right, "/target").merge(&incoming).unwrap();
        import(&right, "/target", "missing-parent", &incoming);
        assert_eq!(
            right
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Independent edit"
        );
        let conflicts = right.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].reason.contains("parent is unavailable"));
        let exported = export(&right, "/target");
        assert_eq!(exported, expected);
        assert_eq!(
            right
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT parent_wave_id FROM waves WHERE id=?1",
                    [&wave],
                    |r| r.get::<_, Option<String>>(0)
                )
                .unwrap(),
            None
        );
    }

    #[test]
    fn projection_retry_reports_only_the_current_conflict() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        // The root sorts before its child, so its new parent is unavailable on
        // the first pass. Once the child projects, the actual conflict is a cycle.
        let root = Wave::new(
            WaveId::parse("00000000-0000-4000-8000-000000000001").unwrap(),
            "root".into(),
            "/source".into(),
        );
        left.create_wave(&root).unwrap();
        left.select_peer_waves("/source", &destination(), std::slice::from_ref(root.id()))
            .unwrap();
        let base = export(&left, "/source");
        import(&right, "/target", "base", &base);
        let child = Wave::new(
            WaveId::parse("00000000-0000-4000-8000-000000000002").unwrap(),
            "child".into(),
            "/source".into(),
        )
        .with_parent(root.id().clone());
        left.create_wave(&child).unwrap();
        let mut incoming = export(&left, "/source");
        let (id, previous) = incoming
            .changes
            .iter()
            .find(|(_, change)| {
                change.object.id == root.id().as_str() && change.field == "parent_wave_id"
            })
            .unwrap();
        let mut parent = previous.clone();
        parent.parents = [id.clone()].into();
        parent.clock += 1;
        parent.value = json!(child.id());
        incoming.changes.insert("cyclic-parent".into(), parent);

        import(&right, "/target", "cycle", &incoming);
        let conflicts = right.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].object.id, root.id().as_str());
        assert!(conflicts[0].reason.contains("cycle"));
        assert_eq!(export(&right, "/target"), incoming);
        // Repeating the same acquisition cannot churn status by retiring an
        // obsolete first-pass error that should never have been published.
        let revisions = right.revisions().unwrap();
        import(&right, "/target", "cycle", &incoming);
        assert_eq!(right.revisions().unwrap(), revisions);
        assert_eq!(
            right.peer_projection_conflicts("/target").unwrap(),
            conflicts
        );
    }

    #[test]
    fn failed_projection_rolls_back_import_receipts_and_allows_exact_retry() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = export(&left, "/source");
        // Completeness is checked against the same winners used for projection.
        let mut incomplete = base.clone();
        incomplete
            .changes
            .retain(|_, change| change.field != "issue_title");
        let error = right
            .import_peer_planning("/target", &destination(), "incomplete", &incomplete)
            .unwrap_err();
        assert!(error.to_string().contains("incomplete planning record"));
        assert!(export(&right, "/target").changes.is_empty());
        assert!(import_revision(&right, "/target", &destination()).is_none());
        right.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER interrupt_peer BEFORE INSERT ON tasks BEGIN SELECT RAISE(ABORT,'simulated interruption'); END;").unwrap();
        assert!(right
            .import_peer_planning("/target", &destination(), "candidate", &base)
            .is_err());
        assert!(export(&right, "/target").changes.is_empty());
        assert!(import_revision(&right, "/target", &destination()).is_none());
        right
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER interrupt_peer;")
            .unwrap();
        import(&right, "/target", "candidate", &base);
        assert!(right.planning_task(&task).unwrap().record.is_some());
        let mut conflicting = base.clone();
        conflicting
            .changes
            .values_mut()
            .find(|c| c.field == "issue_title")
            .unwrap()
            .value = json!("Reused identity");
        assert!(right
            .import_peer_planning("/target", &destination(), "invalid", &conflicting)
            .is_err());
        assert_eq!(
            import_revision(&right, "/target", &destination()).as_deref(),
            Some("candidate")
        );
        assert_eq!(export(&right, "/target"), base);
    }

    #[test]
    fn reused_changes_in_another_repository_preserve_the_original_and_import_checkpoint() {
        let (_home, store) = store();
        seed(&store);
        let original = export(&store, "/source");
        let mut reused = original.clone();
        // Use a new object so repository ownership cannot mask the global
        // mutation-ID collision this fixture is meant to exercise.
        reused
            .changes
            .retain(|_, change| change.field == "issue_title");
        let change = reused.changes.values_mut().next().unwrap();
        change.object.id = TaskId::new().to_string();
        change.value = json!("Conflicting identity from another plan");
        let error = store
            .import_peer_planning("/other", &destination(), "reused", &reused)
            .unwrap_err();
        assert!(
            error.to_string().contains("conflicting contents"),
            "{error}"
        );
        assert_eq!(export(&store, "/source"), original);
        assert!(export(&store, "/other").changes.is_empty());
        assert!(import_revision(&store, "/other", &destination()).is_none());
    }

    #[test]
    fn common_writers_capture_causal_reopening_and_explicit_deletion_without_echo() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=1,planning_state='completed' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        let completed = export(&left, "/source");
        import(&right, "/target", "done", &completed);
        right
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=0,planning_state='unstarted' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        import(&right, "/target", "delayed", &completed);
        assert!(
            !right
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .completed
        );
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_deleted_at=42 WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        right
            .import_peer_planning(
                "/target",
                &destination(),
                "deleted",
                &export(&left, "/source"),
            )
            .unwrap();
        assert_eq!(
            right.planning_task(&task).unwrap().state,
            crate::store::PlanningState::Removed
        );
        let retained = export(&right, "/target");
        import(&right, "/target", "omission", &Default::default());
        assert_eq!(export(&right, "/target"), retained);
        assert!(right.planning_task(&task).unwrap().record.is_some());
    }
}
