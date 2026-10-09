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

use super::planning::ProviderEvidence;
use super::planning_changes::PlanningChanges;
use super::SqliteStore;

const ASSOCIATION_PROJECTION_PENDING: &str = "correspondence retained; joint planning projection is unfinished; selection and effects remain held";

const SHARING_PROJECTION_PENDING: &str = "peer projection skipped by sharing hold; import required";

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
    deletions: BTreeMap<&'a str, Vec<&'a Value>>,
    orders: BTreeMap<&'a str, FieldHistory<&'a Value>>,
    evidence: BTreeMap<&'a str, FieldHistory<ProviderEvidence>>,
}

// Keep the head marker beside its value. Reordering history cannot separate
// the causal frontier from the evidence, and each decoded body is stored once.
struct FieldHistory<T> {
    entries: Vec<(T, bool)>,
}

impl<T> Default for FieldHistory<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> FieldHistory<T> {
    fn push(&mut self, value: T, is_head: bool) {
        self.entries.push((value, is_head));
    }

    fn values(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().map(|(value, _)| value)
    }

    fn heads(&self) -> impl Iterator<Item = &T> {
        self.entries
            .iter()
            .filter_map(|(value, is_head)| is_head.then_some(value))
    }
}

/// Rejected projections can retain effects only in the peer journal. Never
/// acquire a new effect from an incomplete local projection. The caller holds
/// the same write transaction through recording its attempt; acquisition and
/// local saves do not consult this check.
pub(super) fn require_projected_effects(
    conn: &Connection,
    owner: PlanningChanges<'_>,
) -> StoreResult<()> {
    let (kind, id) = owner.owner();
    let object_kind = match owner {
        PlanningChanges::Task(_) => PlanningKind::Task,
        PlanningChanges::Project(_) => PlanningKind::Project,
    };
    let (_, mapping) =
        provider_schema(object_kind).expect("planning effects have a provider mapping");
    // A rejected incoming ID can name the same provider object as a different
    // local ID. Its effects have not reached that local owner's receipts. Use
    // all retained mappings, not just the winning head: a later null/replacement
    // cannot erase an uncertain effect against the original provider identity.
    // This is effect deferral only, never implicit association or selection.
    let conflict: Option<String> = conn
        .query_row(
            &format!(
                "SELECT c.object_id FROM planning_peer_conflicts c
             WHERE c.kind=?1 AND c.active=1 AND (c.object_id=?2 OR EXISTS(
                 SELECT 1 FROM planning_associations a WHERE a.kind=c.kind AND a.origin_id=c.object_id
                 AND COALESCE(a.task_id,a.project_id)=?2) OR EXISTS(
                 SELECT 1 FROM planning_peer_provider_claims h JOIN {} local
                 ON local.id=?2 AND local.{mapping}=h.provider_id
                 WHERE h.kind=c.kind AND h.object_id=c.object_id))
             ORDER BY c.object_id LIMIT 1",
                table(object_kind)
            ),
            params![kind, id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(conflict) = conflict {
        return Err(invalid(format!(
            "Planning effects deferred: retained peer projection conflict for {kind} {conflict}; no new mutation issued",
        )));
    }
    Ok(())
}

fn changes_by_object(
    snapshot: &PlanningSnapshot,
) -> StoreResult<BTreeMap<&PlanningObject, ObjectChanges<'_>>> {
    let mut objects: BTreeMap<_, ObjectChanges<'_>> = BTreeMap::new();
    for (id, change) in snapshot.winners() {
        objects
            .entry(&change.object)
            .or_default()
            .winners
            .insert(change.field.as_str(), (id, change));
    }
    let heads: BTreeSet<_> = snapshot.heads().map(|(id, _)| id).collect();
    for (id, change) in &snapshot.changes {
        let is_head = heads.contains(id.as_str());
        let object = objects
            .get_mut(&change.object)
            .expect("every retained object has a winning field");
        if change.field == "creation" {
            object.creation.push(&change.value);
        }
        if let Some(receipt) = change.deletion_receipt() {
            object
                .deletions
                .entry(receipt)
                .or_default()
                .push(&change.value);
        }
        if let Some(receipt) = change.order_receipt() {
            object
                .orders
                .entry(receipt)
                .or_default()
                .push(&change.value, is_head);
        }
        if change.provider_evidence() {
            object
                .evidence
                .entry(change.field.as_str())
                .or_default()
                .push(serde_json::from_value(change.value.clone())?, is_head);
        }
        let Some(observation) = &change.linear else {
            continue;
        };
        let matches_mapping = match provider_schema(change.object.kind) {
            Some((_, mapping)) => object.winners.get(mapping).is_some_and(|(_, winner)| {
                winner.value.as_str().is_some()
                    && winner.value.as_str() == observation.body["id"].as_str()
            }),
            // Comment membership carries provenance too, but content owns acquisition.
            None => change.object.kind == PlanningKind::Comment && change.field == "content",
        };
        if matches_mapping {
            object.observations.push(observation);
        }
    }
    // Retain decoded evidence outside dependency retries. Removal ages
    // select the first known timestamp, never random mutation-ID traversal order.
    for (object, changes) in &mut objects {
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
        changes.observations = latest_observations(changes.observations.iter().copied())?;
        if let Some(history) = changes.evidence.get_mut("provider_removal") {
            history.entries.sort_by_key(|(fact, _)| match fact {
                ProviderEvidence::IssueChange { observed_at, .. } => *observed_at,
                _ => None,
            });
        }
    }
    Ok(objects)
}

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

/// Content is parsed by its common owner, not by a second SQL Markdown parser.
/// Capture in the caller's transaction, using the same journal and causal heads
/// as scalar triggers. Import suppresses echo; readback only adds missing facts.
pub(super) fn capture_project_content(conn: &Connection, project: &ProjectId) -> StoreResult<()> {
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
        FROM planning_peer_changes c JOIN planning_peer_observed h ON h.id=c.id
        WHERE c.kind='project' AND h.object_id=?1
            AND c.field IN ('workflow','krs','metric_targets')",
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
    // Journal the saved canonical representation, not the caller's untrimmed
    // Markdown inputs. Import has already returned without parsing or echoing it.
    let fields = serde_json::to_value(super::project_content::read_content(conn, project)?)?;
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
        let missing_origin = linear.is_some()
            && conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM planning_peer_uncaptured_heads
                 WHERE kind='project' AND object_id=?1 AND field=?2)",
                params![project.as_str(), field],
                |row| row.get(0),
            )?;
        if unchanged && frontier_retained && !missing_origin {
            continue;
        }
        conn.execute("INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
            SELECT lower(hex(randomblob(16))),'project',?1,?2,?3,
                max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),?4,
                (SELECT json_group_array(h.id) FROM planning_peer_capture_heads h JOIN planning_peer_changes c ON c.id=h.id
                    WHERE h.kind='project' AND h.object_id=?1 AND h.field=?2 AND (c.object_id=?1 OR ?4 IS NOT NULL))",
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
    /// Record exact provider correspondence without moving either identity,
    /// execution, receipts or sharing selection. Repeat calls are idempotent.
    pub fn associate_peer_planning(
        &self,
        repo: &str,
        origin: &PlanningObject,
        local_id: &str,
        provider_id: &str,
    ) -> StoreResult<()> {
        let (_, mapping) = provider_schema(origin.kind)
            .ok_or_else(|| invalid("only Tasks and Projects can be associated"))?;
        match origin.kind {
            PlanningKind::Task => {
                TaskId::parse(&origin.id).map_err(invalid)?;
                TaskId::parse(local_id).map_err(invalid)?;
            }
            PlanningKind::Project => {
                ProjectId::parse(&origin.id).map_err(invalid)?;
                ProjectId::parse(local_id).map_err(invalid)?;
            }
            _ => unreachable!("provider schema restricts association kinds"),
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let local = PlanningObject {
            kind: origin.kind,
            id: local_id.into(),
        };
        require_repository(&tx, &local, repo)?;
        // Local rows always own their own execution. Association cannot redirect
        // a second existing row, nor reinterpret an issue name as an identity.
        if exists(&tx, origin)? {
            return Err(invalid("incoming Work already exists locally; its identity and execution cannot be redirected"));
        }
        let mapped: Option<String> = tx
            .query_row(
                &format!("SELECT {mapping} FROM {} WHERE id=?1", table(origin.kind)),
                [local_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        if provider_id.is_empty() || mapped.as_deref() != Some(provider_id) {
            return Err(invalid(
                "local Work does not map to the supplied Linear identity",
            ));
        }
        let retained: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM planning_members WHERE kind=?1 AND object_id=?2 AND repo=?3)",
            params![origin.kind.as_str(), origin.id, repo], |row| row.get(0),
        )?;
        let mut query = tx.prepare(
            "SELECT DISTINCT json_extract(value,'$') FROM planning_peer_changes
             WHERE kind=?1 AND object_id=?2 AND field=?3 AND json_type(value)='text'",
        )?;
        let claims = query
            .query_map(params![origin.kind.as_str(), origin.id, mapping], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if !retained || claims != [provider_id] {
            return Err(invalid("incoming Work lacks one unambiguous retained Linear mapping; creation inputs and names are not correspondence"));
        }
        drop(query);
        let saved: Option<(String, String, String)> = tx.query_row(
            "SELECT repo,provider_id,COALESCE(task_id,project_id) FROM planning_associations WHERE kind=?1 AND origin_id=?2",
            params![origin.kind.as_str(),origin.id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).optional()?;
        if let Some(saved) = saved {
            if saved != (repo.into(), provider_id.into(), local_id.into()) {
                return Err(invalid(
                    "correspondence is already recorded for a different local Work or provider",
                ));
            }
        } else {
            tx.execute(
                "INSERT INTO planning_associations(kind,origin_id,repo,provider_id,task_id,project_id)
                 VALUES(?1,?2,?3,?4,CASE WHEN ?1='task' THEN ?5 END,CASE WHEN ?1='project' THEN ?5 END)",
                params![origin.kind.as_str(),origin.id,repo,provider_id,local_id],
            )?;
        }
        // Correspondence enables lookup, not joint projection. Keep incomplete
        // receipt composition visibly held instead of clearing mapping conflicts.
        retain_projection_conflict(&tx, repo, origin, SHARING_PROJECTION_PENDING)?;
        tx.commit()?;
        Ok(())
    }

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
        let mut rows = query.query([repo])?;
        let mut statuses = Vec::new();
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let digest: Option<String> = row.get(10)?;
            let mut conflicts = projection_conflicts_in(&tx, repo, &id)?;
            let (pending_local, local_error) = match export_selected(&tx, repo, &id) {
                Ok((snapshot, held)) => {
                    let pending = match digest {
                        Some(digest) => digest != planning_digest(&snapshot)?,
                        None => !snapshot.changes.is_empty(),
                    };
                    conflicts.retain(|conflict| {
                        conflict.reason != SHARING_PROJECTION_PENDING
                            || !held.contains_key(&conflict.object)
                    });
                    for (object, reason) in held {
                        let conflict = PeerProjectionConflict { object, reason };
                        if !conflicts.contains(&conflict) {
                            conflicts.push(conflict);
                        }
                    }
                    (Some(pending), None)
                }
                // A damaged journal is destination-local evidence, not a reason
                // to hide the other plans or claim no local changes. SQL failures
                // still fail the read. Never display raw journal contents here.
                Err(StoreError::InvalidData(_) | StoreError::Serde(_)) => (
                    None,
                    Some("Local planning journal is invalid; pending changes and sharing holds are unknown. Retained plans and sync receipts are unchanged.".into()),
                ),
                Err(error) => return Err(error),
            };
            statuses.push(PeerPlanningStatus {
                id,
                reference: row.get(1)?,
                active: row.get(2)?,
                selected_records: row.get::<_, i64>(3)? as u64,
                imported_revision: row.get(4)?,
                fetched_revision: row.get(5)?,
                acquisition_error: row.get(6)?,
                publication_revision: row.get(7)?,
                publication_state: row.get(8)?,
                publication_error: row.get(9)?,
                pending_local,
                local_error,
                conflicts,
            });
        }
        drop(rows);
        drop(query);
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
        let held = selection_conflicts(&tx, repo, destination, &merged)?;
        let objects = changes_by_object(&merged)?;
        tx.execute("UPDATE planning_peer_context SET importing=1", [])?;
        retain_mutations(&tx, &saved, incoming)?;
        // Correspondence can retain each creation origin on its local owner
        // before joint scalar projection is available. This releases no effect:
        // the sharing/projection conflict below still fences every new attempt.
        // Import suppression keeps private local edits out of the peer journal.
        for (object, changes) in &objects {
            if !held.contains_key(*object) || changes.creation.is_empty() {
                continue;
            }
            let Some(local) = associated_local_id(&tx, object.kind, &object.id, Some(repo))? else {
                continue;
            };
            let receipt = tx.savepoint()?;
            match project_creation(&receipt, object, &local, changes) {
                Ok(()) => receipt.commit()?,
                // Contradictory captured operations remain in the journal and
                // behind the existing hold; never replace the retained receipt.
                Err(error) if projection_conflict(&error) => receipt.finish()?,
                Err(error) => return Err(error),
            }
        }
        let mut pending = objects
            .iter()
            .filter(|(object, _)| !held.contains_key(*object))
            .map(|(&object, changes)| (object, changes))
            .collect::<Vec<_>>();
        let mut conflicts = loop {
            let count = pending.len();
            let mut retry = Vec::new();
            // Only the final attempt describes a current conflict. A parent
            // projected in this pass may replace an earlier missing-parent error.
            let mut conflicts = BTreeSet::new();
            for (object, changes) in pending {
                let mut acquired = validate_provider_mapping(&tx, object, changes);
                if acquired.is_ok() {
                    for (field, history) in &changes.evidence {
                        let evidence = tx.savepoint()?;
                        match acquire_provider_evidence(
                            &evidence, repo, object, changes, field, history,
                        ) {
                            Ok(()) => evidence.commit()?,
                            Err(error) if projection_conflict(&error) => {
                                evidence.finish()?;
                                acquired = Err(error);
                            }
                            Err(error) => return Err(error),
                        }
                    }
                }
                let savepoint = tx.savepoint()?;
                match acquired
                    .and_then(|()| insert_and_project(&savepoint, object, repo, changes, &merged))
                {
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
                Ok(()) => observe_projected_fields(&tx, object, &["current_project_id"])?,
                Err(error) if projection_conflict(&error) => {
                    conflicts.insert((object.clone(), error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        // Project lists can refer to Tasks first created by this import. Apply
        // only after their projection; no receipt is settled by this pass.
        for object in objects.keys().filter(|o| o.kind == PlanningKind::Project) {
            if !held.contains_key(*object)
                && !conflicts.iter().any(|(failed, _)| failed == *object)
                && exists(&tx, object)?
            {
                super::planning_order::project_pending(&tx, &ProjectId::from_raw(&object.id))?;
            }
        }
        // A skipped projection may retain new effects only in the journal.
        // Selection can later release a sharing hold without importing again;
        // keep the projection conflict until those receipts actually project.
        // The receipt records skipped projection, not its current cause. Status
        // derives the specific hold; diagnostic wording never chooses authority.
        conflicts.extend(
            held.into_keys()
                .map(|object| (object, SHARING_PROJECTION_PENDING.into())),
        );
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

/// Resolve only explicitly associated full IDs. Physical rows are resolved by
/// callers first; this does not rename durable IDs or grant execution authority.
pub(super) fn associated_local_id(
    conn: &Connection,
    kind: PlanningKind,
    origin: &str,
    repo: Option<&str>,
) -> StoreResult<Option<String>> {
    let Some((_, mapping)) = provider_schema(kind) else {
        return Ok(None);
    };
    let ancestry = match kind {
        PlanningKind::Task => {
            "JOIN projects p ON p.id=local.project_id JOIN waves w ON w.id=p.wave_id"
        }
        PlanningKind::Project => "JOIN waves w ON w.id=local.wave_id",
        _ => unreachable!("provider schema restricts association kinds"),
    };
    let mut query = conn.prepare(&format!(
        "SELECT local.id FROM planning_associations a JOIN {} local
         ON local.id=COALESCE(a.task_id,a.project_id) AND local.{mapping}=a.provider_id
         {ancestry}
         WHERE a.kind=?1 AND a.origin_id=?2 AND w.repo=a.repo AND (?3 IS NULL OR a.repo=?3)
         AND NOT EXISTS(SELECT 1 FROM planning_peer_changes c
             WHERE c.kind=a.kind AND c.object_id=a.origin_id AND c.field='{mapping}'
             AND json_type(c.value)='text' AND json_extract(c.value,'$')<>a.provider_id)",
        table(kind)
    ))?;
    query
        .query_row(params![kind.as_str(), origin, repo], |row| row.get(0))
        .optional()
        .map_err(StoreError::from)
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
        for object in references(change) {
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

fn references(change: &PlanningMutation) -> Vec<PlanningObject> {
    if change.order_receipt().is_none() {
        return reference(change).into_iter().collect();
    }
    let mut ids = BTreeSet::new();
    for list in [
        &change.value["desired"],
        &change.value["base"]["value"],
        &change.value["conflict"]["value"],
    ]
    .into_iter()
    .chain(
        change.value["effects"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|effect| [&effect["before"], &effect["after"]]),
    ) {
        ids.extend(
            list.as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        );
    }
    ids.into_iter()
        .map(|id| PlanningObject {
            kind: PlanningKind::Task,
            id: id.into(),
        })
        .collect()
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
        for parent in references(change).into_iter().chain(
            change
                .parents
                .iter()
                .map(|id| snapshot.changes[id].object.clone())
                .filter(|parent| parent != &change.object),
        ) {
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
        if member_destination(conn, repo, object)?.as_deref() != Some(destination) {
            held.insert(object.clone(), "planning exchange held: causal predecessor is outside this selection; private history is retained".into());
            pending.insert(object.clone());
        }
        if belongs_elsewhere(conn, object, repo)? {
            held.insert(object.clone(), "planning exchange held: record moved outside this repository; local work and peer history are retained".into());
            pending.insert(object.clone());
        }
        let associated: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM planning_associations WHERE kind=?1 AND repo=?2
             AND (origin_id=?3 OR COALESCE(task_id,project_id)=?3))",
            params![object.kind.as_str(), repo, object.id],
            |row| row.get(0),
        )?;
        if associated {
            held.insert(object.clone(), ASSOCIATION_PROJECTION_PENDING.into());
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
    let mut query = conn.prepare(
        "WITH RECURSIVE origins(kind,object_id) AS (
        SELECT kind,object_id FROM planning_members WHERE repo=?1 AND destination=?2
        UNION
        SELECT parent.kind,parent.object_id FROM origins o
        JOIN planning_peer_changes c ON c.kind=o.kind AND c.object_id=o.object_id
        JOIN json_each(c.parents) link
        JOIN planning_peer_changes parent ON parent.id=link.value
    ) SELECT c.kind,c.object_id,c.field,c.value,c.clock,c.linear,c.parents,c.id
        FROM planning_peer_changes c JOIN origins o ON o.kind=c.kind AND o.object_id=c.object_id",
    )?;
    let mut snapshot = PlanningSnapshot::default();
    let mut rows = query.query(params![repo, destination])?;
    while let Some(row) = rows.next()? {
        snapshot.changes.insert(row.get(7)?, read_mutation(row)?);
    }
    snapshot.validate().map_err(invalid)?;
    let mut query = conn.prepare(
        "SELECT h.id FROM json_each(?1) o JOIN planning_peer_heads h
         ON h.kind=json_extract(o.value,'$.kind') AND h.object_id=json_extract(o.value,'$.id')",
    )?;
    let actual = query
        .query_map([serde_json::to_string(&snapshot.objects())?], |row| {
            row.get::<_, String>(0)
        })?
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

fn provider_schema(kind: PlanningKind) -> Option<(&'static str, &'static str)> {
    match kind {
        PlanningKind::Task => Some(("pm_items", "external_issue_id")),
        PlanningKind::Project => Some(("pm_projects", "external_project_id")),
        _ => None,
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
        require_repository(conn, object, repo)?;
        return observe_projected_fields(conn, object, object.kind.fields());
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
                super::repositories::ensure_repository_in(conn, repo)?;
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
    project_creation(conn, object, &object.id, changes)?;
    if object.kind == PlanningKind::Task {
        for (receipt, history) in &changes.deletions {
            super::planning_changes::import_peer_deletion(
                conn,
                &TaskId::from_raw(&object.id),
                receipt,
                history,
            )?;
        }
    }
    if object.kind == PlanningKind::Project {
        for (receipt, history) in &changes.orders {
            super::planning_order::import_peer_receipt(
                conn,
                &ProjectId::from_raw(&object.id),
                receipt,
                history.values().copied(),
                history.heads().copied(),
            )?;
        }
    }
    if let Some(history) = changes.evidence.get("provider_teams") {
        project_confirmed_teams(conn, object, history)?;
    }
    let previous = delivery_fields(conn, object)?;
    // Creation readback settles captured receipts before a Linear winner can
    // retire the remaining local intentions during delivery projection.
    acquire_linear_frontier(conn, object, changes, repo)?;
    let deletion_receipts = object.kind == PlanningKind::Task && !changes.deletions.is_empty();
    if deletion_receipts {
        super::planning_changes::reconcile_deletion(conn, &TaskId::from_raw(&object.id))?;
    }
    let pending_order = if object.kind == PlanningKind::Task {
        let project = winners["project_id"].1.value.as_str();
        conn.query_row("SELECT EXISTS(SELECT 1 FROM project_changes c JOIN planning_order_current o ON o.id=c.id
            WHERE o.project_id=?1 AND c.acknowledged=0 AND c.conflict_json IS NULL)", [project], |row| row.get::<_, bool>(0))?
    } else {
        false
    };
    // Receipt-owned values are not scalar observations. Track only the fields
    // that actually project, in this same savepoint, after every writer succeeds.
    let projected: Vec<_> = object
        .kind
        .fields()
        .iter()
        .copied()
        .filter(|field| {
            !(*field == "planning_teams" && changes.evidence.contains_key("provider_teams")
                || pending_order && *field == "planning_rank"
                || deletion_receipts && *field == "planning_deleted_at"
                || object.kind == PlanningKind::Wave && *field == "current_project_id")
        })
        .collect();
    project_fields(
        conn,
        object,
        projected
            .iter()
            .copied()
            .filter(|field| !(object.kind == PlanningKind::Project && content_field(field)))
            .map(|field| (field, &winners[field].1.value)),
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
    // Projection may add receipts captured on a peer. Reconcile those too
    // against the same accepted readback, never acknowledging a later local save.
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
    require_repository(conn, object, repo)?;
    observe_projected_fields(conn, object, &projected)
}

/// Retain the evaluated frontier without synthesizing a local edit. Keeping all
/// accepted heads (including losers) lets the next save follow that decision.
/// Rejected and held objects never enter here; their prior observation survives.
fn observe_projected_fields(
    conn: &Connection,
    object: &PlanningObject,
    fields: &[&str],
) -> StoreResult<()> {
    let fields = serde_json::to_string(fields)?;
    conn.execute(
        "DELETE FROM planning_peer_observed WHERE object_id=?1 AND EXISTS (
            SELECT 1 FROM planning_peer_changes c WHERE c.id=planning_peer_observed.id
                AND c.kind=?2 AND c.field IN (SELECT value FROM json_each(?3)))",
        params![object.id, object.kind.as_str(), fields],
    )?;
    conn.execute(
        "INSERT INTO planning_peer_observed(object_id,id)
         SELECT object_id,id FROM planning_peer_heads WHERE object_id=?1 AND kind=?2
             AND field IN (SELECT value FROM json_each(?3))",
        params![object.id, object.kind.as_str(), fields],
    )?;
    Ok(())
}

fn project_creation(
    conn: &Connection,
    origin: &PlanningObject,
    local: &str,
    changes: &ObjectChanges<'_>,
) -> StoreResult<()> {
    let Some((_, winner)) = changes.winners.get("creation") else {
        return Ok(());
    };
    let task = TaskId::from_raw(local);
    let project = ProjectId::from_raw(local);
    super::planning_export::import_peer_receipts(
        conn,
        origin,
        match origin.kind {
            PlanningKind::Task => PlanningChanges::Task(&task),
            PlanningKind::Project => PlanningChanges::Project(&project),
            _ => return Err(invalid("creation requires a Task or Project")),
        },
        &winner.value,
        &changes.creation,
    )
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

/// Mapping ownership is common to scalar projection and independent evidence.
/// A peer edit cannot detach or redirect existing Work and its retained effects.
/// In particular, NULL would make a legacy record eligible for provider creation.
fn validate_provider_mapping(
    conn: &Connection,
    object: &PlanningObject,
    changes: &ObjectChanges<'_>,
) -> StoreResult<()> {
    let Some((_, mapping)) = provider_schema(object.kind) else {
        return Ok(());
    };
    let provider_id = changes.winners[mapping].1.value.as_str();
    // Preserve this Work's existing mapping and every other Work's ownership.
    // An unmapped Work may attach, but a null winner cannot clear its mapping.
    let conflict: bool = conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE
                (id=?1 AND {mapping} IS NOT NULL AND {mapping} IS NOT ?2)
                OR (id!=?1 AND ({mapping}=?2 OR EXISTS(
                    SELECT 1 FROM planning_peer_provider_claims h
                    WHERE h.kind=?3 AND h.object_id=?1 AND h.provider_id={mapping}))))",
            table(object.kind)
        ),
        params![object.id, provider_id, object.kind.as_str()],
        |row| row.get(0),
    )?;
    if conflict {
        return Err(StoreError::ProviderObservationConflict {
            entity: "planning mapping",
            id: object.id.clone(),
        });
    }
    Ok(())
}

/// Acquire independent evidence before scalar projection. A stale entity may be
/// rejected without rolling back an accepted removal, archive or Team readback.
fn acquire_provider_evidence(
    conn: &Connection,
    repo: &str,
    object: &PlanningObject,
    changes: &ObjectChanges<'_>,
    field: &str,
    history: &FieldHistory<ProviderEvidence>,
) -> StoreResult<()> {
    let (_, mapping) =
        provider_schema(object.kind).expect("provider evidence belongs to mapped Work");
    let provider_id = changes.winners[mapping].1.value.as_str();
    let mapping_conflict = || StoreError::ProviderObservationConflict {
        entity: "planning mapping",
        id: object.id.clone(),
    };
    for fact in history.values() {
        if Some(fact.id()) != provider_id {
            return Err(mapping_conflict());
        }
    }
    match field {
        "provider_invalidation" => {
            // Only the greatest known revision affects the common floor. Keep
            // every fact in the journal, without rereading the cache for each one.
            let floor = history
                .values()
                .filter_map(|fact| match fact {
                    ProviderEvidence::IssueChange {
                        revision_ns: Some(revision),
                        ..
                    }
                    | ProviderEvidence::IssueDetail {
                        revision_ns: Some(revision),
                        ..
                    } => Some((*revision, fact)),
                    _ => None,
                })
                .max_by_key(|(revision, _)| *revision);
            if let Some((_, fact)) = floor {
                super::planning::observe_issue_change_in(conn, fact)?;
            }
            invalidate_from_heads(conn, history)?;
        }

        "provider_removal" => {
            for fact in history.values() {
                super::planning::observe_issue_change_in(conn, fact)?;
            }
        }
        "provider_archive" => {
            for fact in history.values() {
                super::planning::confirm_project_archival_in(conn, repo, "linear", fact)?;
            }
        }
        "provider_teams" => {
            let Some(ProviderEvidence::Teams { project, .. }) = history.heads().next() else {
                return Ok(());
            };
            let conflict = || StoreError::ProjectMembershipConflict {
                project_id: project.id.clone(),
            };
            for head in history.heads() {
                let ProviderEvidence::Teams { project: other, .. } = head else {
                    unreachable!("validated Team evidence")
                };
                if !super::planning::same_ids(&project.team_ids, &other.team_ids)
                    || !super::planning::same_ids(&project.initiative_ids, &other.initiative_ids)
                {
                    return Err(conflict());
                }
            }
            let unresolved: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM pm_projects WHERE repo=?1 AND provider='linear' AND id=?2 AND membership_unresolved=1)",
                    params![repo,project.id], |row| row.get(0),
                )?;
            if unresolved {
                return Err(conflict());
            }
            if let Some(retained) =
                super::planning::cached_project(conn, repo, "linear", &project.id)?
            {
                if !super::planning::same_ids(&retained.initiative_ids, &project.initiative_ids)
                    || !team_history_contains(history.values(), &retained.team_ids)
                {
                    return Err(conflict());
                }
            }
            // Apply only the causal heads. Older entity bodies remain history.
            for head in history.heads() {
                super::planning::reconcile_project_teams_in(conn, repo, "linear", head)?;
            }
            project_confirmed_teams(conn, object, history)?;
        }
        _ => unreachable!("validated provider evidence"),
    }
    Ok(())
}

/// Apply only outstanding notices, never causally retired history. This also
/// runs after scalar acquisition, which may create the cache on a cold import.
fn invalidate_from_heads(
    conn: &Connection,
    history: &FieldHistory<ProviderEvidence>,
) -> StoreResult<bool> {
    let mut outstanding = false;
    for head in history.heads() {
        if matches!(head, ProviderEvidence::IssueChange { .. }) {
            super::planning::observe_issue_change_in(conn, head)?;
            outstanding = true;
        }
    }
    Ok(outstanding)
}

/// An accepted entity frontier can consume a causal detail acknowledgement;
/// scalar/list replay alone never restores freshness.
fn reconcile_task_freshness(
    conn: &Connection,
    repo: &str,
    provider_id: &str,
    history: &FieldHistory<ProviderEvidence>,
) -> StoreResult<()> {
    if invalidate_from_heads(conn, history)? {
        return Ok(());
    }
    let revision: Option<Option<String>> = conn.query_row(
        "SELECT json_extract(body,'$.revision') FROM pm_items WHERE repo=?1 AND provider='linear' AND id=?2",
        params![repo, provider_id],
        |row| row.get(0),
    ).optional()?;
    let Some(revision) = revision else {
        return Ok(());
    };
    let revision = super::planning::revision_nanos(revision.as_deref())?;
    if history.heads().any(|fact| matches!(fact, ProviderEvidence::IssueDetail { revision_ns, .. } if revision >= *revision_ns)) {
        conn.execute("UPDATE pm_items SET needs_refresh=0 WHERE repo=?1 AND provider='linear' AND id=?2 AND needs_refresh!=0", params![repo,provider_id])?;
    }
    Ok(())
}

fn project_confirmed_teams(
    conn: &Connection,
    object: &PlanningObject,
    history: &FieldHistory<ProviderEvidence>,
) -> StoreResult<()> {
    let Some(ProviderEvidence::Teams { project, .. }) = history.heads().next() else {
        return Ok(());
    };
    conn.execute(
        "UPDATE projects SET planning_teams=?2 WHERE id=?1 AND planning_teams IS NOT ?2",
        params![object.id, serde_json::to_string(&project.team_ids)?],
    )?;
    Ok(())
}

fn team_history_contains<'a>(
    mut evidence: impl Iterator<Item = &'a ProviderEvidence>,
    teams: &[String],
) -> bool {
    evidence.any(|fact| match fact {
        ProviderEvidence::Teams {
            project, previous, ..
        } => {
            super::planning::same_ids(&project.team_ids, teams)
                || previous
                    .as_ref()
                    .is_some_and(|before| super::planning::same_ids(before, teams))
        }
        _ => false,
    })
}

fn retain_confirmed_teams(
    changes: &ObjectChanges<'_>,
    project: &mut crate::pm::PmProject,
) -> StoreResult<()> {
    let Some(history) = changes.evidence.get("provider_teams") else {
        return Ok(());
    };
    if !team_history_contains(history.values(), &project.team_ids) {
        return Err(StoreError::ProjectMembershipConflict {
            project_id: project.id.clone(),
        });
    }
    if let Some(ProviderEvidence::Teams {
        project: confirmed, ..
    }) = history.heads().next()
    {
        if !super::planning::same_ids(&project.initiative_ids, &confirmed.initiative_ids) {
            return Err(StoreError::ProjectMembershipConflict {
                project_id: project.id.clone(),
            });
        }
        project.team_ids.clone_from(&confirmed.team_ids);
    }
    Ok(())
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
    let Some((provider_table, mapping)) = provider_schema(object.kind) else {
        return Ok(());
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
                super::planning::put_item(conn, repo, "linear", observed_at, &item)?
            }
            PlanningKind::Project => {
                let mut project = serde_json::from_value(observation.body.clone())?;
                retain_confirmed_teams(changes, &mut project)?;
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
                retain_confirmed_teams(changes, &mut project)?;
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
    if let Some(history) = changes.evidence.get("provider_invalidation") {
        reconcile_task_freshness(conn, repo, provider_id, history)?;
    }
    // A mapping is not a creation acknowledgement. Only an accepted provider
    // body can reconcile the original attempt, including after a peer supplied
    // the mapping first. Keep this inside the object's projection savepoint.
    if let Some(observation) = changes.observations.last() {
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
            "comment identity already belongs to another comment"
                | "comment belongs to another Task"
                | "Wave parent would create a cycle"
                | "Wave parent is unavailable"
                | "peer provider frontier is older than retained evidence"
                | "Linear removal prevents peer Task projection"
                | "Linear archive prevents peer Project projection"
        ),
        StoreError::PlanningReceiptConflict { .. }
        | StoreError::ProjectMembershipConflict { .. }
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
        retain_projection_conflict(conn, repo, object, reason)?;
    }
    Ok(())
}

fn retain_projection_conflict(
    conn: &Connection,
    repo: &str,
    object: &PlanningObject,
    reason: &str,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO planning_peer_conflicts(repo,kind,object_id,reason,active) VALUES(?1,?2,?3,?4,1)
         ON CONFLICT(kind,object_id,reason) DO UPDATE SET active=1 WHERE active=0",
        params![repo, object.kind.as_str(), object.id, reason],
    )?;
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
        seed_at(store, "/source", "planning", "FIX-1")
    }

    fn seed_at(store: &SqliteStore, repo: &str, name: &str, identifier: &str) -> TaskId {
        let wave = Wave::new(WaveId::new(), name.into(), repo.into());
        store.create_wave(&wave).unwrap();
        let project = ProjectId::new();
        let task = TaskId::new();
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,project_prompt_context)
            VALUES(?1,?2,1,1,'chapter','Chapter','')", params![project.as_str(),wave.id()]).unwrap();
        conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
            VALUES(?1,?2,?3,'Original','Brief',1,1,'')",params![task.as_str(),project.as_str(),identifier]).unwrap();
        super::capture_project_content(&conn, &project).unwrap();
        drop(conn);
        store
            .select_peer_waves(repo, &destination(), std::slice::from_ref(wave.id()))
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

    fn deletion_change(store: &SqliteStore, task: &TaskId) -> crate::planning::PlanningChange {
        store
            .pending_task_changes(task)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "deleted")
            .unwrap()
    }

    fn deletion_receipt(store: &SqliteStore, id: &str) -> serde_json::Value {
        let body: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT json_object('id',id,'deleted_at',deletion_saved_at,'value',json(value_json),'base',json(base_json),
             'attempted',attempted,'acknowledged',acknowledged,'revision',acknowledged_revision,
             'conflict',json(conflict_json),'error',error) FROM task_changes WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        serde_json::from_str(&body).unwrap()
    }

    fn execution_rows(store: &SqliteStore) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        let conn = store.conn.lock().unwrap();
        [
            "agent_sessions",
            "processes",
            "task_workflows",
            "task_workflow_moves",
            "task_prs",
            "work_placements",
            "project_transitions",
        ]
        .into_iter()
        .map(|table| {
            let mut query = conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                .unwrap();
            let count = query.column_count();
            query
                .query_map([], |row| (0..count).map(|i| row.get(i)).collect())
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        })
        .collect()
    }

    #[test]
    fn peer_rejected_frontier_defers_new_effects_without_blocking_acquisition() {
        use super::super::planning_changes::PlanningChanges;
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        preserve_execution(&target, &wave, &task);
        let execution = execution_rows(&target);
        let checkout = target.task(&task).unwrap().unwrap().worktree;
        // Model a retained provider frontier not yet captured by a planning
        // projection (migration/alternate acquisition). Use its acquisition owner.
        let mut record = crate::store::PmTaskRecord {
            item: row.snapshot.items[0].clone(),
            project: Some(row.snapshot.projects[0].clone()),
            observed_at: 60,
        };
        record.item.revision = Some("2026-10-08T12:00:00Z".into());
        super::super::planning::put_item(
            &target.conn.lock().unwrap(),
            "/target",
            "linear",
            60,
            &record.item,
        )
        .unwrap();
        source.delete_task(&task).unwrap();
        let uncertain = deletion_change(&source, &task);
        assert!(source
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &uncertain,
                row.snapshot.items[0].revision.as_deref(),
            )
            .unwrap());
        source
            .planning_field_error(PlanningChanges::Task(&task), &uncertain, "lost reply")
            .unwrap();
        // This separate local save is not attempted; the only contrary effect
        // is about to arrive in a rejected object's journal.
        target.delete_task(&task).unwrap();
        let local = deletion_change(&target, &task);
        let incoming = export(&source, "/source");
        import(&target, "/target", "stale-with-effect", &incoming);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .iter()
            .any(|c| c.object.id == task.as_str() && c.reason.contains("older than retained")));
        assert!(export(&target, "/target")
            .changes
            .iter()
            .any(|(id, mutation)| incoming.changes.get(id) == Some(mutation)
                && mutation.deletion_receipt() == Some(uncertain.id.as_str())
                && mutation.value["attempted"] == true));
        assert_eq!(deletion_receipt(&target, &local.id)["attempted"], 0);
        assert!(target
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &local,
                record.item.revision.as_deref(),
            )
            .unwrap_err()
            .to_string()
            .contains("retained peer projection conflict"));
        // A public delivery readback can acquire while the conflict remains.
        target
            .put_pm_task("/target", "linear", &record, Some((&wave, "initiative")))
            .unwrap();
        assert_eq!(
            target
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .observed_at,
            60
        );
        assert!(target
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &local,
                record.item.revision.as_deref(),
            )
            .is_err());
        assert_eq!(deletion_receipt(&target, &local.id)["attempted"], 0);
        // Only successful projection releases the conflict, after importing the
        // retained attempt into the common receipt owner. It still cannot replay.
        import(&target, "/target", "projected", &incoming);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(deletion_receipt(&target, &uncertain.id)["attempted"], 1);
        assert!(!target
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &local,
                record.item.revision.as_deref(),
            )
            .unwrap());
        assert_eq!(execution_rows(&target), execution);
        assert_eq!(target.task(&task).unwrap().unwrap().worktree, checkout);
    }

    #[test]
    fn peer_deletion_retains_attempt_until_positive_acknowledgement_without_execution() {
        use super::super::planning_changes::PlanningChanges;
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        preserve_execution(&target, &wave, &task);
        let execution = execution_rows(&target);
        let checkout = target.task(&task).unwrap().unwrap().worktree;
        source.delete_task(&task).unwrap();
        let change = deletion_change(&source, &task);
        assert!(source
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &change,
                row.snapshot.items[0].revision.as_deref()
            )
            .unwrap());
        source
            .planning_field_error(PlanningChanges::Task(&task), &change, "response lost")
            .unwrap();
        let attempted = export(&source, "/source");
        import(&target, "/target", "attempted", &attempted);
        assert_eq!(
            deletion_receipt(&target, &change.id),
            deletion_receipt(&source, &change.id)
        );
        assert_eq!(deletion_change(&target, &task), change);
        assert!(!target
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &change,
                row.snapshot.items[0].revision.as_deref()
            )
            .unwrap());
        // Neither a missing peer record nor absent inventory acknowledges deletion.
        import(&target, "/target", "omitted", &Default::default());
        let mut inventory = row.clone();
        inventory.snapshot.items.clear();
        inventory.synced_at += 1;
        target.put_pm_snapshot(&inventory).unwrap();
        assert_eq!(deletion_receipt(&target, &change.id)["acknowledged"], 0);
        assert_eq!(deletion_receipt(&target, &change.id)["attempted"], 1);
        // The exact provider mutation acknowledgement travels on the same receipt.
        assert!(source
            .acknowledge_task_deletion(&task, &change, None)
            .unwrap());
        let acknowledged = export(&source, "/source");
        import(&target, "/target", "acknowledged", &acknowledged);
        assert_eq!(deletion_receipt(&target, &change.id)["acknowledged"], 1);
        assert_eq!(
            deletion_receipt(&target, &change.id)["error"],
            serde_json::Value::Null
        );
        assert!(target
            .pending_task_changes(&task)
            .unwrap()
            .iter()
            .all(|c| c.field != "deleted"));
        target
            .planning_field_error(PlanningChanges::Task(&task), &change, "late error")
            .unwrap();
        import(&target, "/target", "late-attempt", &attempted);
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "late-attempt", &attempted);
        assert_eq!(target.revisions().unwrap(), revisions);
        assert_eq!(
            deletion_receipt(&target, &change.id),
            deletion_receipt(&source, &change.id)
        );
        assert_eq!(target.task(&task).unwrap().unwrap().worktree, checkout);
        assert_eq!(execution_rows(&target), execution);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn peer_deletion_explicit_active_evidence_retires_concurrent_losers_not_inventory() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        preserve_execution(&target, &wave, &task);
        let execution = execution_rows(&target);
        source.delete_task(&task).unwrap();
        let removed = deletion_change(&source, &task);
        let mut item = row.snapshot.items[0].clone();
        item.revision = Some("2026-10-08T12:00:00Z".into());
        source
            .put_pm_task(
                "/source",
                "linear",
                &crate::store::PmTaskRecord {
                    item: item.clone(),
                    project: Some(row.snapshot.projects[0].clone()),
                    observed_at: 50,
                },
                Some((&wave, "initiative")),
            )
            .unwrap();
        assert!(source.planning_task(&task).unwrap().state == crate::store::PlanningState::Removed);
        source
            .observe_task_deletion(&task, item.revision.as_deref().unwrap())
            .unwrap();
        assert!(source.planning_task(&task).unwrap().state != crate::store::PlanningState::Removed);
        // An offline save made later in wall time cannot defeat observed Linear activity.
        target.delete_task(&task).unwrap();
        let concurrent = deletion_change(&target, &task);
        import(&target, "/target", "active", &export(&source, "/source"));
        assert!(target.planning_task(&task).unwrap().state != crate::store::PlanningState::Removed);
        for id in [&removed.id, &concurrent.id] {
            let receipt = deletion_receipt(&target, id);
            assert_eq!(receipt["value"], true);
            assert_eq!(receipt["acknowledged"], 0);
            assert_eq!(receipt["conflict"]["value"], false);
            assert_eq!(receipt["conflict"]["revision"], json!(item.revision));
        }
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "active", &export(&source, "/source"));
        assert_eq!(target.revisions().unwrap(), revisions);
        assert_eq!(execution_rows(&target), execution);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn peer_deletion_late_history_cannot_hide_a_newer_pending_save() {
        use super::super::planning_changes::PlanningChanges;
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        source.delete_task(&task).unwrap();
        let mut item = row.snapshot.items[0].clone();
        item.revision = Some("2026-10-08T12:00:00Z".into());
        for (store, repo) in [(&source, "/source"), (&target, "/target")] {
            store
                .put_pm_task(
                    repo,
                    "linear",
                    &crate::store::PmTaskRecord {
                        item: item.clone(),
                        project: Some(row.snapshot.projects[0].clone()),
                        observed_at: 50,
                    },
                    Some((&wave, "initiative")),
                )
                .unwrap();
        }
        source
            .observe_task_deletion(&task, item.revision.as_deref().unwrap())
            .unwrap();
        // This save follows the active provider revision. The other machine's
        // older retired receipt arrives last and gets a later local sequence.
        target.delete_task(&task).unwrap();
        let current = deletion_change(&target, &task);
        let mut history = export(&source, "/source");
        let scalar_clock = export(&target, "/target")
            .changes
            .values()
            .map(|c| c.clock)
            .max()
            .unwrap()
            + 1;
        for change in history.changes.values_mut().filter(|c| {
            c.object.id == task.as_str()
                && c.field == "planning_deleted_at"
                && c.value.is_null()
                && !c.parents.is_empty()
        }) {
            change.clock = scalar_clock;
        }
        import(&target, "/target", "old-history", &history);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(deletion_change(&target, &task), current);
        assert_eq!(
            target.planning_task(&task).unwrap().state,
            crate::store::PlanningState::Removed
        );
        assert!(target
            .planning_field_owners("/target")
            .unwrap()
            .contains(&crate::durable::WorkRef::Task(task.clone())));
        assert!(target
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &current,
                item.revision.as_deref()
            )
            .unwrap());
        assert!(!target
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &current,
                item.revision.as_deref()
            )
            .unwrap());
    }

    #[test]
    fn peer_deletion_creation_readback_fills_baseline_without_acknowledging_removal() {
        use super::super::planning_changes::PlanningChanges;
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, existing) = linear_seed(&source);
        let project = source.task(&existing).unwrap().unwrap().project_id;
        source
            .bind_project(&wave, row.snapshot.items[0].project_id.as_deref().unwrap())
            .unwrap();
        let task = TaskId::new();
        source
            .create_task(&crate::planning::NewTask {
                id: task.clone(),
                project_id: project,
                title: "Created".into(),
                description: String::new(),
            })
            .unwrap();
        let owner = PlanningChanges::Task(&task);
        let creation = source
            .prepare_planning_export(owner, "team", "initiative")
            .unwrap();
        assert!(source
            .attempt_planning_export(owner, &creation.input, false)
            .unwrap());
        source.delete_task(&task).unwrap();
        let deletion = deletion_change(&source, &task);
        assert_eq!(deletion.base, None);
        let before = export(&source, "/source");
        import(&target, "/target", "before-readback", &before);
        let mut item: crate::pm::PmItem = serde_json::from_value(creation.model.clone()).unwrap();
        item.id = creation.id.clone();
        item.identifier = "FIX-CREATED".into();
        item.project_id = row.snapshot.items[0].project_id.clone();
        item.revision = Some("2026-10-08T12:00:00Z".into());
        source
            .put_pm_task(
                "/source",
                "linear",
                &crate::store::PmTaskRecord {
                    item: item.clone(),
                    project: Some(row.snapshot.projects[0].clone()),
                    observed_at: 50,
                },
                Some((&wave, "initiative")),
            )
            .unwrap();
        import(&target, "/target", "readback", &export(&source, "/source"));
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(
            deletion_receipt(&target, &deletion.id),
            deletion_receipt(&source, &deletion.id)
        );
        assert_eq!(
            deletion_change(&target, &task).base,
            Some(json!({"revision":item.revision,"value":false}))
        );
        assert_eq!(deletion_receipt(&target, &deletion.id)["acknowledged"], 0);
        import(&target, "/target", "late", &before);
        assert_eq!(
            deletion_receipt(&target, &deletion.id),
            deletion_receipt(&source, &deletion.id)
        );
        // Positive trash readback, unlike creation readback, settles this identity.
        assert!(target
            .acknowledge_task_deletion(&task, &deletion, Some("2026-10-08T13:00:00Z"))
            .unwrap());
        import(&source, "/source", "trashed", &export(&target, "/target"));
        assert_eq!(deletion_receipt(&source, &deletion.id)["acknowledged"], 1);
        assert_eq!(
            deletion_receipt(&source, &deletion.id)["revision"],
            "2026-10-08T13:00:00Z"
        );
    }

    #[test]
    fn peer_deletion_rejects_malformed_receipts_and_isolates_competing_baselines() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, _, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        preserve_execution(&target, &wave, &task);
        let execution = execution_rows(&target);
        source.delete_task(&task).unwrap();
        let incoming = export(&source, "/source");
        let (mutation, _) = incoming
            .changes
            .iter()
            .find(|(_, c)| c.deletion_receipt().is_some())
            .unwrap();
        let mut malformed = incoming.clone();
        malformed.changes.get_mut(mutation).unwrap().value["process_lfid"] = json!("not-planning");
        assert!(target
            .import_peer_planning("/target", &destination(), "invalid", &malformed)
            .is_err());
        assert_eq!(
            import_revision(&target, "/target", &destination()).as_deref(),
            Some("base")
        );
        let mut competing = incoming.clone();
        let mut other = competing.changes[mutation].clone();
        other.value["base"]["revision"] = json!("2026-10-08T11:00:00Z");
        other.clock += 1;
        other.parents.insert(mutation.clone());
        competing.changes.insert("competing-deletion".into(), other);
        import(&target, "/target", "competing", &competing);
        assert!(target.planning_task(&task).unwrap().state != crate::store::PlanningState::Removed);
        assert!(target.pending_task_changes(&task).unwrap().is_empty());
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .iter()
            .any(|c| c.object.id == task.as_str()
                && c.reason.contains("competing planning deletion receipts")));
        assert_eq!(export(&target, "/target"), competing);
        assert_eq!(execution_rows(&target), execution);
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
    fn peer_creation_prepares_unprepared_plans_without_execution_or_transitions() {
        use super::super::planning_changes::PlanningChanges;
        use crate::durable::WorkRef;

        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let task = seed(&source);
        assert!(target.repository_id("/target").unwrap().is_none());
        import(
            &target,
            "/target",
            "unprepared",
            &export(&source, "/source"),
        );
        let repository = target.repository_id("/target").unwrap().unwrap();
        assert_eq!(
            target.repository_path(&repository).unwrap().as_deref(),
            Some("/target")
        );
        let project = target.task(&task).unwrap().unwrap().project_id;
        assert_eq!(
            target.planning_export_owners("/target").unwrap(),
            vec![
                WorkRef::Project(project.clone()),
                WorkRef::Task(task.clone()),
            ]
        );
        let assert_no_local_history = || {
            let conn = target.conn.lock().unwrap();
            for table in [
                "task_creation_intents",
                "project_transitions",
                "agent_sessions",
                "processes",
                "task_workflows",
                "task_prs",
                "work_placements",
            ] {
                assert_eq!(
                    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0,
                    "{table}"
                );
            }
        };
        assert_no_local_history();
        let project_owner = PlanningChanges::Project(&project);
        let export = target
            .prepare_planning_export(project_owner, "team", "initiative")
            .unwrap();
        assert!(target
            .attempt_planning_export(project_owner, &export.input, false)
            .unwrap());
        // A mapped receipt remains discoverable until accepted readback.
        target
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET external_project_id=?2 WHERE id=?1",
                params![project.as_str(), export.id],
            )
            .unwrap();
        assert!(target.planning_export_pending(project_owner).unwrap());
        let task_owner = PlanningChanges::Task(&task);
        let creation = target
            .prepare_planning_export(task_owner, "team", "initiative")
            .unwrap();
        assert_eq!(creation.parent, project.as_str());
        assert_eq!(creation.input["title"], "Original");
        assert_eq!(
            target
                .prepare_planning_export(task_owner, "ignored", "ignored")
                .unwrap(),
            creation
        );
        assert_no_local_history();
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
                "SELECT export_json,export_attempted,export_error FROM planning_creations WHERE kind='task' AND origin_id=?1",
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
        let request = crate::planning::NewTask {
            id: task.clone(),
            project_id: original.clone(),
            title: "Created".into(),
            description: "Brief".into(),
        };
        source.create_task(&request).unwrap();
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
            source.create_task(&request).unwrap().project_id,
            destination_project,
            "the original local request stays idempotent after a move"
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
                    "SELECT count(*) FROM task_creation_intents WHERE task_id=?1",
                    [task.as_str()],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0,
            "import retains the creation receipt, not a local creation request"
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
                    "SELECT export_error FROM planning_creations WHERE kind='project' AND origin_id=?1",
                    [project.as_str()],
                    |r| r.get::<_, Option<String>>(0),
                )
                .unwrap()
        };
        assert_eq!(error().as_deref(), Some("lost attachment response"));
        assert!(target.planning_export_pending(owner).unwrap());
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
        assert!(!target.planning_export_pending(owner).unwrap());
        target.planning_export_error(owner, "late timeout").unwrap();
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
        // The rejected snapshot carries competing attachment identity. The
        // local unattempted link is not permission to send a second attachment.
        assert!(target
            .attempt_planning_export(owner, &creation.input, true)
            .unwrap_err()
            .to_string()
            .contains("retained peer projection conflict"));
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
                text: "  Keep the independent KR  ".into(),
                holds: false,
            }];
            source.update_project_content(project, &left).unwrap();
            // Exchange follows the canonical saved Markdown, not caller whitespace.
            left.krs[0].text = "Keep the independent KR".into();
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
    fn repository_association_preserves_provider_lookup_and_uncertain_effects() {
        use super::super::planning_changes::PlanningChanges;
        let (_home, source) = store();
        let (wave, row, task) = linear_seed(&source);
        preserve_execution(&source, &wave, &task);
        edit_title(&source, &task, "Uncertain title");
        let change = source.pending_task_changes(&task).unwrap().remove(0);
        assert!(source
            .attempt_planning_field(
                PlanningChanges::Task(&task),
                &change,
                row.snapshot.items[0].revision.as_deref()
            )
            .unwrap());
        let retained = source.task(&task).unwrap();
        let project = source
            .project(&retained.as_ref().unwrap().project_id)
            .unwrap();
        let effects = source.pending_task_changes(&task).unwrap();
        let journal = export(&source, "/source");
        let execution = execution_rows(&source);
        let original = source.repository_id("/source").unwrap().unwrap();
        let selected = crate::durable::RepositoryId::new();
        source.bind_repository("/source", &selected).unwrap();
        assert_eq!(
            source.task_by_issue(&row.snapshot.items[0].id).unwrap(),
            retained
        );
        assert_eq!(
            source
                .project(&retained.as_ref().unwrap().project_id)
                .unwrap(),
            project
        );
        assert_eq!(source.pending_task_changes(&task).unwrap(), effects);
        assert_eq!(export(&source, "/source"), journal);
        assert_eq!(execution_rows(&source), execution);
        assert_eq!(
            source.repository_path(&original).unwrap().as_deref(),
            Some("/source")
        );
        assert_eq!(
            source.task_execution_route(&task).unwrap().repository_id,
            selected
        );
    }

    #[test]
    fn independent_peer_tasks_converge_without_transferring_execution() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        // Both roots already own distinct identities and Work before association.
        let left_task = seed(&left);
        let right_task = seed_at(&right, "/target", "right", "RIGHT-1");
        let repository = left.repository_id("/source").unwrap().unwrap();
        let prior_repository = right.repository_id("/target").unwrap().unwrap();
        assert_ne!(repository, prior_repository);
        assert_ne!(left_task, right_task);
        let left_wave = left.task(&left_task).unwrap().unwrap().wave_id;
        let right_wave = right.task(&right_task).unwrap().unwrap().wave_id;
        let admission = left
            .validate_task_planning(&left.task(&left_task).unwrap().unwrap())
            .unwrap_err();
        assert!(admission.to_string().contains("first-start admission"));
        for (store, wave, task) in [
            (&left, &left_wave, &left_task),
            (&right, &right_wave, &right_task),
        ] {
            preserve_execution(store, wave, task);
        }
        let left_execution = execution_rows(&left);
        let right_execution = execution_rows(&right);
        let before = export(&right, "/target");
        let selected = right.peer_planning_status("/target").unwrap();
        let retained = right.task(&right_task).unwrap();
        right.bind_repository("/target", &repository).unwrap();
        assert_eq!(export(&right, "/target"), before);
        assert_eq!(right.peer_planning_status("/target").unwrap(), selected);
        assert_eq!(right.task(&right_task).unwrap(), retained);
        assert_eq!(execution_rows(&right), right_execution);
        assert_eq!(
            right.repository_path(&prior_repository).unwrap().as_deref(),
            Some("/target")
        );
        let left_route = left.task_execution_route(&left_task).unwrap();
        let right_route = right.task_execution_route(&right_task).unwrap();
        assert_ne!(left_route.machine_id, right_route.machine_id);
        assert_eq!(left_route.repository_id, right_route.repository_id);
        let left_initial = export(&left, "/source");
        let right_initial = export(&right, "/target");
        import(&left, "/source", "right", &right_initial);
        import(&right, "/target", "left", &left_initial);
        assert_eq!(export(&left, "/source"), export(&right, "/target"));
        for (store, repo) in [(&left, "/source"), (&right, "/target")] {
            assert!(store.peer_projection_conflicts(repo).unwrap().is_empty());
            assert!(store.task(&left_task).unwrap().is_some());
            assert!(store.task(&right_task).unwrap().is_some());
            assert_eq!(store.repository_id(repo).unwrap(), Some(repository.clone()));
        }
        assert!(left.task(&right_task).unwrap().unwrap().worktree.is_none());
        assert!(right.task(&left_task).unwrap().unwrap().worktree.is_none());
        assert!(right
            .validate_task_planning(&right.task(&left_task).unwrap().unwrap())
            .unwrap_err()
            .to_string()
            .contains("first-start admission"));
        right
            .validate_task_planning(&right.task(&right_task).unwrap().unwrap())
            .unwrap();
        assert_eq!(execution_rows(&left), left_execution);
        assert_eq!(execution_rows(&right), right_execution);
        assert_eq!(left.task_execution_route(&left_task).unwrap(), left_route);
        assert_eq!(
            right.task_execution_route(&right_task).unwrap(),
            right_route
        );

        // Preserve legacy local execution for an imported identity too. This
        // fixture is history, not a competing first-start admission protocol.
        preserve_named_execution(
            &right,
            &left_wave,
            &left_task,
            "legacy-left",
            "/retained/legacy-left",
        );
        let right_execution = execution_rows(&right);
        let legacy_route = right.task_execution_route(&left_task).unwrap();
        let legacy_workflow = right.workflow(&left_task).unwrap();

        // Local saves while disconnected need no peer. Reconnection imports the
        // planning disposition, never the sender's Workflow movement or cleanup.
        edit_title(&left, &left_task, "Saved offline");
        complete(&left, &left_task);
        let completed = export(&left, "/source");
        import(&right, "/target", "completed", &completed);
        assert_eq!(
            right.task(&left_task).unwrap().unwrap().plan.title,
            "Saved offline"
        );
        assert_eq!(
            right
                .planning_task(&left_task)
                .unwrap()
                .record
                .unwrap()
                .item
                .state
                .as_deref(),
            Some("completed")
        );
        assert_eq!(right.workflow(&left_task).unwrap(), legacy_workflow);
        assert_eq!(
            right.task_execution_route(&left_task).unwrap(),
            legacy_route
        );
        assert_eq!(execution_rows(&right), right_execution);
        let revision = right.revisions().unwrap();
        import(&right, "/target", "completed", &completed);
        assert_eq!(right.revisions().unwrap(), revision);
        assert_eq!(export(&right, "/target"), completed);
        assert_eq!(
            right.task_execution_route(&right_task).unwrap(),
            right_route
        );
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
    fn peer_import_observes_only_projected_fields_before_the_next_local_save() {
        for project_case in [false, true] {
            let (_source_home, source) = store();
            let (_target_home, target) = store();
            let (wave, mut row, task) = linear_seed(&source);
            let project = source.task(&task).unwrap().unwrap().project_id;
            let base = export(&source, "/source");
            import(&target, "/target", "base", &base);
            preserve_execution(&target, &wave, &task);
            let execution = execution_rows(&target);
            let (owner, parent_field, field, missing_parent) = if project_case {
                row.snapshot.projects[0].name = "Peer name".into();
                row.snapshot.projects[0].workflow = "peer-review".into();
                row.snapshot.projects[0].revision = Some("2026-10-09T12:00:00Z".into());
                (
                    project.as_str(),
                    "wave_id",
                    "workflow",
                    WaveId::new().to_string(),
                )
            } else {
                row.snapshot.items[0].name = "Peer name".into();
                row.snapshot.items[0].revision = Some("2026-10-09T12:00:00Z".into());
                (
                    task.as_str(),
                    "project_id",
                    "issue_title",
                    ProjectId::new().to_string(),
                )
            };
            row.synced_at += 1;
            source.put_pm_snapshot(&row).unwrap();
            let incoming = export(&source, "/source");
            let (peer_id, peer) = incoming
                .winners()
                .find(|(_, c)| c.object.id == owner && c.field == field)
                .unwrap();
            let (parent_id, parent) = incoming
                .winners()
                .find(|(_, c)| c.object.id == owner && c.field == parent_field)
                .unwrap();
            let mut missing = parent.clone();
            missing.parents = [parent_id.to_owned()].into();
            missing.clock = incoming.changes.values().map(|c| c.clock).max().unwrap() + 1;
            missing.value = json!(missing_parent);
            missing.linear = None;
            let mut rejected = incoming.clone();
            rejected
                .changes
                .insert("unavailable-parent".into(), missing.clone());
            for revision in ["rejected", "repeated-rejection"] {
                import(&target, "/target", revision, &rejected);
                assert!(target
                    .peer_projection_conflicts("/target")
                    .unwrap()
                    .iter()
                    .any(|c| c.object.id == owner));
            }
            assert_eq!(execution_rows(&target), execution);
            // The rejected journal survives, but capture must follow the last
            // accepted field, not the retained peer head that never projected.
            if project_case {
                let mut content = target.planning_project(&project).unwrap();
                content.workflow = "local-review".into();
                target
                    .update_project_content(
                        &project,
                        &crate::pm::ProjectContent {
                            workflow: content.workflow,
                            krs: content.krs,
                            metric_targets: content.metric_targets,
                        },
                    )
                    .unwrap();
            } else {
                edit_title(&target, &task, "Local after rejection");
            }
            let saved = export(&target, "/target");
            assert_eq!(saved.changes[peer_id], *peer);
            let local = saved
                .changes
                .values()
                .find(|c| {
                    c.object.id == owner
                        && c.field == field
                        && c.linear.is_none()
                        && c.value
                            == if project_case {
                                json!("local-review")
                            } else {
                                json!("Local after rejection")
                            }
                })
                .unwrap();
            assert!(!local.parents.contains(peer_id));
            let (base_id, baseline) = base
                .winners()
                .find(|(_, c)| c.object.id == owner && c.field == field)
                .unwrap();
            assert!(local.parents.contains(base_id));
            assert_eq!(
                super::linear_predecessor(&saved, local),
                Some(json!({
                    "value": baseline.value,
                    "revision": baseline.linear.as_ref().unwrap().revision(),
                }))
            );

            // Repair through an ordinary subsequent import, not acquisition or
            // an observation helper. The accepted field must add no echo edit.
            let mut repaired = rejected;
            missing.parents = ["unavailable-parent".into()].into();
            missing.clock += 1;
            missing.value = parent.value.clone();
            repaired.changes.insert("restored-parent".into(), missing);
            let expected = saved.merge(&repaired).unwrap();
            import(&target, "/target", "repaired", &repaired);
            assert!(target
                .peer_projection_conflicts("/target")
                .unwrap()
                .is_empty());
            assert_eq!(export(&target, "/target"), expected);
            import(&target, "/target", "older-again", &incoming);
            import(&target, "/target", "repeat-repair", &repaired);
            assert_eq!(export(&target, "/target"), expected);
            if project_case {
                let content = target.planning_project(&project).unwrap();
                assert_eq!(content.workflow, "peer-review");
                target
                    .update_project_content(
                        &project,
                        &crate::pm::ProjectContent {
                            workflow: "final-review".into(),
                            krs: content.krs,
                            metric_targets: content.metric_targets,
                        },
                    )
                    .unwrap();
            } else {
                assert_eq!(target.task(&task).unwrap().unwrap().plan.title, "Peer name");
                edit_title(&target, &task, "Final local name");
            }
            let exchanged = export(&target, "/target");
            let (_, local) = exchanged
                .winners()
                .find(|(_, c)| c.object.id == owner && c.field == field)
                .unwrap();
            assert!(local.parents.contains(peer_id));
            assert_eq!(
                super::linear_predecessor(&exchanged, local),
                Some(json!({
                    "value": peer.value, "revision": peer.linear.as_ref().unwrap().revision(),
                }))
            );
            import(&source, "/source", "return", &exchanged);
            assert_eq!(export(&source, "/source"), exchanged);
            let changes = if project_case {
                source.pending_project_changes(&project).unwrap()
            } else {
                source.pending_task_changes(&task).unwrap()
            };
            let receipt = changes
                .iter()
                .find(|c| c.field == if project_case { "workflow" } else { "name" })
                .unwrap();
            assert_eq!(receipt.value, local.value);
            assert_eq!(receipt.base, super::linear_predecessor(&exchanged, local));
            assert_eq!(execution_rows(&target), execution);
        }
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

    fn observed_state(store: &SqliteStore, repo: &str, issue: &str) -> crate::store::PlanningState {
        store
            .pm_task_observation(repo, "linear", issue)
            .unwrap()
            .state
    }

    #[test]
    fn peer_invalidation_replay_cannot_undo_detail_but_new_notices_still_invalidate() {
        for revision in [None, Some("2026-10-08T11:00:00Z")] {
            let (_source_home, source) = store();
            let (_target_home, target) = store();
            let (wave, row, task) = linear_seed(&source);
            let base = export(&source, "/source");
            import(&target, "/target", "base", &base);
            preserve_execution(&target, &wave, &task);
            let execution = execution_rows(&target);
            let item = &row.snapshot.items[0];
            source
                .observe_pm_issue_change(&item.id, revision, false)
                .unwrap();
            if revision.is_some() {
                // A later lower-revision notice retires the earlier head, not
                // its revision floor. Import must retain the greatest floor.
                source
                    .observe_pm_issue_change(&item.id, item.revision.as_deref(), false)
                    .unwrap();
            }
            let notice = export(&source, "/source");
            import(&target, "/target", "notice", &notice);
            assert_eq!(
                observed_state(&target, "/target", &item.id),
                crate::store::PlanningState::Invalid
            );
            // Detail retains the provider revision, not a receiving-time version.
            let mut detail = crate::store::PmTaskRecord {
                item: item.clone(),
                project: None,
                observed_at: 60,
            };
            detail.item.revision = Some("2026-10-08T11:00:00Z".into());
            target
                .put_pm_task("/target", "linear", &detail, None)
                .unwrap();
            // A cache-only detail does not transport a new entity frontier.
            // Its acknowledgement cannot make the source's older body fresh.
            let (_fresh_home, fresh) = store();
            import(&fresh, "/target", "base", &base);
            import(&fresh, "/target", "ack-only", &export(&target, "/target"));
            assert_eq!(
                observed_state(&fresh, "/target", &item.id),
                crate::store::PlanningState::Invalid
            );
            // The real revision in that readback also fences a stale local read.
            fresh
                .put_pm_task(
                    "/target",
                    "linear",
                    &crate::store::PmTaskRecord {
                        item: item.clone(),
                        project: None,
                        observed_at: 90,
                    },
                    None,
                )
                .unwrap();
            assert_eq!(
                observed_state(&fresh, "/target", &item.id),
                crate::store::PlanningState::Invalid
            );
            import(&source, "/source", "ack-only", &export(&target, "/target"));
            assert_eq!(
                observed_state(&source, "/source", &item.id),
                crate::store::PlanningState::Invalid
            );
            target
                .put_pm_task("/target", "linear", &detail, Some((&wave, "initiative")))
                .unwrap();
            let refreshed = export(&target, "/target");
            target
                .put_pm_task("/target", "linear", &detail, Some((&wave, "initiative")))
                .unwrap();
            assert_eq!(export(&target, "/target"), refreshed);
            import(&target, "/target", "notice", &notice);
            assert_eq!(
                observed_state(&target, "/target", &item.id),
                crate::store::PlanningState::Available
            );
            let revisions = target.revisions().unwrap();
            import(&target, "/target", "notice", &notice);
            assert_eq!(target.revisions().unwrap(), revisions);
            assert_eq!(export(&target, "/target"), refreshed);
            import(&source, "/source", "refreshed", &refreshed);
            assert_eq!(
                observed_state(&source, "/source", &item.id),
                crate::store::PlanningState::Available
            );
            // A genuinely new unversioned event follows that detail causally.
            source
                .observe_pm_issue_change(&item.id, None, false)
                .unwrap();
            import(
                &target,
                "/target",
                "new-notice",
                &export(&source, "/source"),
            );
            assert_eq!(
                observed_state(&target, "/target", &item.id),
                crate::store::PlanningState::Invalid
            );
            import(&target, "/target", "old-detail", &refreshed);
            assert_eq!(
                observed_state(&target, "/target", &item.id),
                crate::store::PlanningState::Invalid
            );
            assert_eq!(execution_rows(&target), execution);
            assert_eq!(
                target.task(&task).unwrap().unwrap().worktree.as_deref(),
                Some(std::path::Path::new("/retained/work"))
            );
        }
    }

    #[test]
    fn peer_invalidation_survives_cold_import_and_concurrent_detail() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_wave, row, _task) = linear_seed(&source);
        let item = &row.snapshot.items[0];
        source
            .observe_pm_issue_change(&item.id, None, false)
            .unwrap();
        let notice = export(&source, "/source");
        import(&target, "/target", "cold", &notice);
        assert_eq!(
            observed_state(&target, "/target", &item.id),
            crate::store::PlanningState::Invalid
        );
        let detail = crate::store::PmTaskRecord {
            item: item.clone(),
            project: None,
            observed_at: 60,
        };
        target
            .put_pm_task("/target", "linear", &detail, None)
            .unwrap();
        let refreshed = export(&target, "/target");
        // Neither receiving age nor a concurrent detail retires an unseen notice.
        source
            .observe_pm_issue_change(&item.id, None, false)
            .unwrap();
        import(
            &target,
            "/target",
            "concurrent",
            &export(&source, "/source"),
        );
        import(&source, "/source", "concurrent-detail", &refreshed);
        for (store, repo) in [(&source, "/source"), (&target, "/target")] {
            assert_eq!(
                observed_state(store, repo, &item.id),
                crate::store::PlanningState::Invalid
            );
        }
        // An actual detail observing both heads retires both, including equal bodies.
        target
            .put_pm_task("/target", "linear", &detail, None)
            .unwrap();
        import(&source, "/source", "settled", &export(&target, "/target"));
        assert_eq!(
            observed_state(&source, "/source", &item.id),
            crate::store::PlanningState::Available
        );
        // A failed/null detail is also a new invalidation, not erased by replay.
        let retained = source
            .pm_task_observation("/source", "linear", &item.id)
            .unwrap()
            .record
            .unwrap();
        source
            .invalidate_pm_task("/source", "linear", &retained)
            .unwrap();
        import(
            &target,
            "/target",
            "null-detail",
            &export(&source, "/source"),
        );
        assert_eq!(
            observed_state(&target, "/target", &item.id),
            crate::store::PlanningState::Invalid
        );
    }

    #[test]
    fn peer_provider_removal_and_archive_precede_stale_detail_without_losing_work() {
        for (archive, unknown_age) in [(false, false), (false, true), (true, false)] {
            let (_source_home, source) = store();
            let (_target_home, target) = store();
            let (wave, row, task) = linear_seed(&source);
            let base = export(&source, "/source");
            import(&target, "/target", "base", &base);
            preserve_execution(&target, &wave, &task);
            edit_title(&target, &task, "Retain unsent title");
            let project = target.task(&task).unwrap().unwrap().project_id;
            target
                .edit_project(&project, Some("Retain unsent Project"), None)
                .unwrap();
            let execution = execution_rows(&target);
            let receipts = target.pending_task_changes(&task).unwrap();
            let project_receipts = target.pending_project_changes(&project).unwrap();
            if archive {
                source
                    .confirm_pm_project_archival("/source", "linear", &row.snapshot.projects[0], 61)
                    .unwrap();
            } else {
                source
                    .observe_pm_issue_change(
                        &row.snapshot.items[0].id,
                        Some("2026-10-08T13:00:00Z"),
                        true,
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
            let mut incoming = export(&source, "/source");
            if unknown_age {
                // Released-frontier removal has no acquisition age to invent.
                incoming
                    .changes
                    .values_mut()
                    .find(|c| c.provider_evidence())
                    .unwrap()
                    .value["observed_at"] = json!(null);
            }
            let evidence = incoming
                .changes
                .values()
                .find(|c| c.provider_evidence())
                .unwrap();
            assert_eq!(evidence.value["observed_at"].is_null(), unknown_age);
            assert!(incoming
                .changes
                .values()
                .all(|c| c.deletion_receipt().is_none()));
            import(&target, "/target", "negative", &incoming);
            assert!(target.get_wave(independent.id()).unwrap().is_some());
            assert_eq!(
                target.task(&task).unwrap().unwrap().plan.title,
                "Retain unsent title"
            );
            assert_eq!(
                target.project(&project).unwrap().unwrap().plan.name,
                "Retain unsent Project"
            );
            assert_eq!(target.pending_task_changes(&task).unwrap(), receipts);
            assert_eq!(
                target.pending_project_changes(&project).unwrap(),
                project_receipts
            );
            if archive {
                let conn = target.conn.lock().unwrap();
                let (archived, age): (bool, i64) = conn.query_row("SELECT archived,observed_at FROM pm_projects WHERE repo='/target' AND id=?1", [&row.snapshot.projects[0].id], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
                assert!(archived);
                assert_eq!(age, 42); // entity acquisition was not made fresh by archive
            } else {
                assert_eq!(
                    target.planning_task(&task).unwrap().state,
                    crate::store::PlanningState::Removed
                );
            }
            let retained = export(&target, "/target");
            import(&target, "/target", "stale", &base);
            import(&target, "/target", "negative", &incoming);
            assert_eq!(export(&target, "/target"), retained);
            let revisions = target.revisions().unwrap();
            import(&target, "/target", "negative", &incoming);
            assert_eq!(target.revisions().unwrap(), revisions);
            assert_eq!(execution_rows(&target), execution);
            assert_eq!(
                target.task(&task).unwrap().unwrap().worktree.as_deref(),
                Some(std::path::Path::new("/retained/work"))
            );
        }
    }

    #[test]
    fn peer_confirmed_teams_preserve_newer_entity_and_uncertain_order() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        preserve_execution(&target, &wave, &task);
        let project = target.task(&task).unwrap().unwrap().project_id;
        let mut newer = row.snapshot.projects[0].clone();
        newer.name = "Newer entity".into();
        newer.revision = Some("2099-01-01T00:00:00Z".into());
        target
            .put_pm_project(&wave, "linear", "initiative", &newer, 90)
            .unwrap();
        // A captured uncertain order requires a complete list, never Team or entity evidence.
        let effects = json!([{"before":[task],"after":[task],"issue":row.snapshot.items[0].id,
            "input":{"prioritySortOrder":1.0},"settled":false}]);
        {
            let conn = target.conn.lock().unwrap();
            super::PlanningChanges::Project(&project)
                .record_value(
                    &conn,
                    "task_order",
                    "uncertain-order",
                    json!([task]),
                    Some(json!({"value":[task],"revision":null})),
                )
                .unwrap();
            conn.execute("UPDATE project_changes SET attempted=1,order_effects_json=?1,error='lost reply' WHERE id='uncertain-order'", [effects.to_string()]).unwrap();
        }
        let order = target.pending_project_changes(&project).unwrap();
        let execution = execution_rows(&target);
        let mut changed = row.snapshot.projects[0].clone();
        changed.team_ids = vec!["other-team".into()];
        source
            .reconcile_pm_project_teams(&wave, "linear", "initiative", &changed, 63)
            .unwrap();
        let incoming = export(&source, "/source");
        assert!(incoming
            .changes
            .values()
            .any(|c| c.field == "provider_teams"));
        import(&target, "/target", "teams", &incoming);
        let conflicts = target.peer_projection_conflicts("/target").unwrap();
        assert!(conflicts.is_empty(), "{conflicts:?}");
        let conn = target.conn.lock().unwrap();
        let (body, age): (String, i64) = conn
            .query_row(
                "SELECT body,observed_at FROM pm_projects WHERE repo='/target' AND id=?1",
                [&changed.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let accepted: crate::pm::PmProject = serde_json::from_str(&body).unwrap();
        assert_eq!(accepted.name, "Newer entity");
        assert_eq!(accepted.revision, newer.revision);
        assert_eq!(accepted.team_ids, changed.team_ids);
        assert_eq!(accepted.initiative_ids, newer.initiative_ids);
        assert_eq!(age, 90);
        let teams: String = conn
            .query_row(
                "SELECT planning_teams FROM projects WHERE id=?1",
                [project.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&teams).unwrap(),
            changed.team_ids
        );
        drop(conn);
        assert_eq!(target.pending_project_changes(&project).unwrap(), order);
        let retained = export(&target, "/target");
        assert!(retained
            .changes
            .values()
            .any(|c| c.field == "provider_teams" && c.value["observed_at"] == 63));
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "teams", &incoming);
        assert_eq!(target.revisions().unwrap(), revisions);
        assert_eq!(export(&target, "/target"), retained);
        assert_eq!(execution_rows(&target), execution);
    }

    #[test]
    fn peer_concurrent_team_confirmations_retain_both_and_isolate_projection() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        preserve_execution(&target, &wave, &task);
        let execution = execution_rows(&target);
        for (store, team) in [(&source, "source-team"), (&target, "target-team")] {
            let mut project = row.snapshot.projects[0].clone();
            project.team_ids = vec![team.into()];
            store
                .reconcile_pm_project_teams(&wave, "linear", "initiative", &project, 66)
                .unwrap();
        }
        let project = target.task(&task).unwrap().unwrap().project_id;
        let retained = target.project(&project).unwrap().unwrap();
        let journal = export(&target, "/target");
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
        import(&target, "/target", "conflicting-teams", &incoming);
        assert!(target.get_wave(independent.id()).unwrap().is_some());
        assert_eq!(target.project(&project).unwrap().unwrap(), retained);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .iter()
            .any(|c| c.object.id == project.as_str()));
        assert_eq!(
            export(&target, "/target"),
            journal.merge(&incoming).unwrap()
        );
        assert_eq!(execution_rows(&target), execution);
        // Contrary Team evidence cannot erase an independently confirmed archive.
        source
            .confirm_pm_project_archival("/source", "linear", &row.snapshot.projects[0], 77)
            .unwrap();
        import(
            &target,
            "/target",
            "archived-despite-team-conflict",
            &export(&source, "/source"),
        );
        let archived: bool = target
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT archived FROM pm_projects WHERE repo='/target' AND id=?1",
                [&row.snapshot.projects[0].id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(archived);
        assert_eq!(target.project(&project).unwrap().unwrap(), retained);
        assert_eq!(execution_rows(&target), execution);
    }

    #[test]
    fn peer_malformed_provider_evidence_rolls_back_every_object() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (wave, row, task) = linear_seed(&source);
        import(&target, "/target", "base", &export(&source, "/source"));
        preserve_execution(&target, &wave, &task);
        source
            .confirm_pm_project_archival("/source", "linear", &row.snapshot.projects[0], 60)
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
        let before = export(&target, "/target");
        let execution = execution_rows(&target);
        for invalid in ["age", "execution"] {
            let mut malformed = incoming.clone();
            let fact = malformed
                .changes
                .values_mut()
                .find(|c| c.provider_evidence())
                .unwrap();
            if invalid == "age" {
                fact.value["observed_at"] = json!(-1);
            } else {
                fact.value["project"]["worktree"] = json!("/not-planning");
            }
            assert!(target
                .import_peer_planning("/target", &destination(), "malformed", &malformed)
                .is_err());
            assert!(target.get_wave(independent.id()).unwrap().is_none());
            assert_eq!(export(&target, "/target"), before);
            assert_eq!(
                import_revision(&target, "/target", &destination()).as_deref(),
                Some("base")
            );
            assert_eq!(execution_rows(&target), execution);
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
        // An unconfirmed scalar Team edit supplies no relationship ordering.
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
        source
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET planning_teams='[\"other-team\"]' WHERE id=?1",
                [project.as_str()],
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
        let (body, unresolved): (String, bool) = conn
            .query_row(
                "SELECT body,membership_unresolved FROM pm_projects WHERE repo='/target' AND id=?1",
                [&row.snapshot.projects[0].id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
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

    fn preserve_execution(store: &SqliteStore, wave: &WaveId, task: &TaskId) {
        preserve_named_execution(store, wave, task, "retained", "/retained/work");
    }

    fn preserve_named_execution(
        store: &SqliteStore,
        wave: &WaveId,
        task: &TaskId,
        session: &str,
        checkout: &str,
    ) {
        let machine = store.local_machine().unwrap().id;
        let driver = ProcessLfid::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
                VALUES(?3,'Retained','human',1,0,?4,?1,?2)", params![task.as_str(),wave,session,checkout]).unwrap();
            conn.execute(
                "UPDATE tasks SET worktree=?3,workspace_slug='retained',checkout_machine_id=?2 WHERE id=?1",
                params![task.as_str(), machine.as_str(), checkout],
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
            super::capture_project_content(&conn, &private_project).unwrap();
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
        let held_move = left
            .pending_task_changes(&task)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "project_id")
            .unwrap();
        assert!(left
            .attempt_planning_field(
                super::super::planning_changes::PlanningChanges::Task(&task),
                &held_move,
                None,
            )
            .unwrap_err()
            .to_string()
            .contains("retained peer projection conflict"));
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
        // Selection releases publication, not a previously skipped projection.
        assert!(left
            .attempt_planning_field(
                super::super::planning_changes::PlanningChanges::Task(&task),
                &held_move,
                None,
            )
            .unwrap_err()
            .to_string()
            .contains("retained peer projection conflict"));
        import(&left, "/source", "released", &received);
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
            super::capture_project_content(&conn, &private_id).unwrap();
            conn.execute(
                "UPDATE projects SET wave_id=?2 WHERE id=?1",
                params![private_id.as_str(), wave.id()],
            )
            .unwrap();
        }
        preserve_execution(&store, wave.id(), &task.id);
        let workflow = store.workflow(&task.id).unwrap();
        store
            .observe_pm_issue_change(&row.snapshot.items[0].id, None, false)
            .unwrap();
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
        let retained: i64 = store.conn.lock().unwrap().query_row("SELECT count(*) FROM planning_peer_changes WHERE object_id=?1 AND field='provider_invalidation'", [task.id.as_str()], |row| row.get(0)).unwrap();
        assert_eq!(retained, 2); // notification and detail stay in the held journal
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
            super::capture_project_content(&conn, &private_project).unwrap();
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
            super::capture_project_content(&conn, &project).unwrap();
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
        conn.execute(
            "UPDATE tasks SET planning_deleted_at=42 WHERE id='task'",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO task_changes(id,task_id,field,value_json,base_json,attempted,error) VALUES('retained-deletion','task','deleted','true',?1,1,'lost response')", [json!({"revision":"2026-10-08T10:00:00Z","value":null}).to_string()]).unwrap();
        let retained_content = "workflow: review\n\n## KRs\n- [x] Keep the accepted proof\n";
        conn.execute(
            "UPDATE projects SET project_prompt_context=?1,workflow='review' WHERE id='project'",
            [retained_content],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_creation_intents(task_id,project_id,title,description,export_error)
            VALUES('task','project','Retain identity','','Discovery unavailable')",
            [],
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
        let created_task = TaskId::new();
        conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
            VALUES(?1,?2,'NEW-1','Captured task','Brief',1,1,'')",params![created_task.as_str(),created.as_str()]).unwrap();
        let item = super::super::plan_read::task_in(&conn, &created_task)
            .unwrap()
            .record
            .unwrap()
            .item;
        let task_uuid = uuid::Uuid::parse_str(created_task.as_str().strip_prefix("task_").unwrap())
            .unwrap()
            .to_string();
        let task_receipt = json!({"id":task_uuid,"model":item,"through":1,"initiative":"initiative","link_id":uuid::Uuid::new_v4().to_string(),
            "input":{"id":task_uuid,"teamId":"team","projectId":item.project_id,"title":item.name,"description":item.description,"assigneeId":item.assignee}});
        conn.execute(r#"INSERT INTO task_changes(seq,id,task_id,field,value_json) VALUES(2,'captured-task',?1,'name','"Captured task"'),(3,'later-task',?1,'name','"Later task"')"#, [created_task.as_str()]).unwrap();
        // The existing deletion owns sequence 1; this creation captured through 2.
        let mut task_receipt = task_receipt;
        task_receipt["through"] = json!(2);
        conn.execute("INSERT INTO task_creation_intents(task_id,project_id,title,description,export_json,export_attempted,export_error)
            VALUES(?1,?2,'Captured task','Brief',?3,1,'lost creation')",params![created_task.as_str(),created.as_str(),task_receipt.to_string()]).unwrap();
        let effects = json!([{"before":["task","other"],"after":["other","task"],"issue":"FIX-1",
            "input":{"sortOrder":12,"prioritySortOrder":0},"settled":false}]);
        conn.execute("INSERT INTO project_changes(id,project_id,field,value_json,base_json,order_effects_json,attempted,error)
            VALUES('retained-order','project','task_order','[\"other\",\"task\"]','{\"value\":[\"task\",\"other\"]}',?1,1,'lost order reply')",[effects.to_string()]).unwrap();
        conn.execute("INSERT INTO project_changes(id,project_id,field,value_json) VALUES('later-order','project','task_order','[\"task\",\"other\"]')",[]).unwrap();
        let fixture: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        conn.execute(
            "UPDATE tasks SET external_issue_id='removed-provider-id' WHERE id='task'",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO pm_issue_changes(issue_id,revision_ns,removed) VALUES('removed-provider-id',NULL,1)", []).unwrap();
        conn.execute(
            "UPDATE projects SET external_project_id=?1 WHERE id='project'",
            [&fixture.projects[0].id],
        )
        .unwrap();
        conn.execute("INSERT INTO pm_projects(repo,provider,id,observed_at,body,archived) VALUES('/fixture','linear',?1,42,?2,1)", params![fixture.projects[0].id,serde_json::to_string(&fixture.projects[0]).unwrap()]).unwrap();
        conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,created_at,issue_identifier,issue_title,workspace_slug) VALUES('invalid-task','project',?1,1,'INVALID-1','Retain invalidation','')", [&fixture.items[0].id]).unwrap();
        conn.execute("INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body,needs_refresh) VALUES('/fixture','linear',?1,?2,?3,42,?4,1)", params![fixture.items[0].id,fixture.items[0].identifier,fixture.items[0].project_id,serde_json::to_string(&fixture.items[0]).unwrap()]).unwrap();
        conn.execute_batch(&upgrade.expect("peer draft or materialized migration"))
            .unwrap();
        super::super::project_content::seed_peer_content(&conn).unwrap();
        super::super::planning::seed_peer_evidence(&conn).unwrap();
        // The released values seed accepted observations, not another mutation.
        // A first edit after upgrade follows that exact retained source identity.
        let seeded: String = conn.query_row(
            "SELECT c.id FROM planning_peer_observed o JOIN planning_peer_changes c ON c.id=o.id
             WHERE o.object_id='task' AND c.field='issue_title' AND c.value='\"Retain identity\"'",
            [], |row| row.get(0),
        ).unwrap();
        conn.execute(
            "UPDATE tasks SET issue_title='First edit after upgrade' WHERE id='task'",
            [],
        )
        .unwrap();
        let parents: String = conn.query_row(
            "SELECT c.parents FROM planning_peer_observed o JOIN planning_peer_changes c ON c.id=o.id
             WHERE o.object_id='task' AND c.field='issue_title'", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&parents).unwrap(),
            vec![seeded]
        );
        for (kind, field) in [
            (
                crate::engine::planning_exchange::PlanningKind::Task,
                "provider_removal",
            ),
            (
                crate::engine::planning_exchange::PlanningKind::Task,
                "provider_invalidation",
            ),
            (
                crate::engine::planning_exchange::PlanningKind::Project,
                "provider_archive",
            ),
        ] {
            let value: String = conn
                .query_row(
                    "SELECT value FROM planning_peer_changes WHERE field=?1",
                    [field],
                    |r| r.get(0),
                )
                .unwrap();
            let value: serde_json::Value = serde_json::from_str(&value).unwrap();
            assert!(value["observed_at"].is_null());
            super::super::planning::ProviderEvidence::validate(kind, field, &value).unwrap();
        }

        let (body, attempted, linked, error): (String,bool,bool,String) = conn.query_row(
            "SELECT export_json,export_attempted,export_link_attempted,export_error FROM planning_creations WHERE kind='project' AND origin_id=?1", [created.as_str()],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        let migrated: super::super::planning_export::PlanningExport =
            serde_json::from_str(&body).unwrap();
        assert_eq!(migrated.captured, ["captured".to_string()].into());
        assert_eq!(migrated.input, receipt["input"]);
        assert!(attempted && linked);
        assert_eq!(error, "lost attachment");
        assert!(!conn
            .query_row(
                "SELECT export_acknowledged FROM planning_creations WHERE kind='project' AND origin_id=?1",
                [created.as_str()],
                |r| r.get::<_, bool>(0),
            )
            .unwrap());
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM planning_exports WHERE kind='project' AND id=?1",
                [created.as_str()],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
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
        assert_eq!(conn.query_row(
            "SELECT export_json,export_attempted,export_error FROM planning_creations WHERE kind='task' AND origin_id='task'",
            [],|row| Ok((row.get::<_,Option<String>>(0)?,row.get::<_,bool>(1)?,row.get::<_,String>(2)?))).unwrap(),
            (None,false,"Discovery unavailable".into()));
        assert!(conn
            .prepare("SELECT export_json FROM task_creation_intents")
            .is_err());
        assert!(conn.prepare("SELECT export_json FROM projects").is_err());
        let (body,attempted,error,owner): (String,bool,String,String) = conn.query_row(
            "SELECT export_json,export_attempted,export_error,task_id FROM planning_creations WHERE kind='task' AND origin_id=?1",
            [created_task.as_str()], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
        let migrated: super::super::planning_export::PlanningExport =
            serde_json::from_str(&body).unwrap();
        assert_eq!(owner, created_task.as_str());
        assert_eq!(migrated.parent, created.as_str());
        assert_eq!(migrated.captured, ["captured-task".into()].into());
        assert_eq!(migrated.model, task_receipt["model"]);
        assert_eq!(migrated.input, task_receipt["input"]);
        assert!(attempted);
        assert_eq!(error, "lost creation");

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
        let deletion = before
            .changes
            .values()
            .find(|change| change.deletion_receipt() == Some("retained-deletion"))
            .unwrap();
        let order = before
            .changes
            .values()
            .find(|change| change.order_receipt() == Some("retained-order"))
            .unwrap();
        assert_eq!(order.value["effects"], effects);
        assert_eq!(order.value["base"], json!({"value":["task","other"]}));
        assert_eq!(order.value["attempted"], true);
        assert_eq!(order.value["acknowledged"], false);
        assert_eq!(order.value["error"], "lost order reply");
        assert_eq!(
            conn.query_row(
                "SELECT id FROM planning_order_current WHERE project_id='project'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "later-order"
        );
        assert_eq!(deletion.value["deleted_at"], 42);
        assert_eq!(deletion.value["attempted"], true);
        assert_eq!(deletion.value["acknowledged"], false);
        assert_eq!(deletion.value["error"], "lost response");
        assert_eq!(deletion.value["base"]["revision"], "2026-10-08T10:00:00Z");
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
    fn peer_mapping_replacement_preserves_provider_effects_and_execution() {
        use super::super::planning_changes::PlanningChanges;

        for replacement in [None, Some("another-provider-id")] {
            let (_source_home, source) = store();
            let (_target_home, target) = store();
            let task = seed(&source);
            let project = source.task(&task).unwrap().unwrap().project_id;
            let wave = source.project(&project).unwrap().unwrap().wave_id;
            {
                let conn = source.conn.lock().unwrap();
                conn.execute(
                    "UPDATE tasks SET external_issue_id='original-issue' WHERE id=?1",
                    [task.as_str()],
                )
                .unwrap();
                conn.execute(
                    "UPDATE projects SET external_project_id='original-project' WHERE id=?1",
                    [project.as_str()],
                )
                .unwrap();
            }
            import(&target, "/target", "base", &export(&source, "/source"));
            preserve_execution(&target, &wave, &task);
            edit_title(&target, &task, "Uncertain local title");
            let change = target.pending_task_changes(&task).unwrap().remove(0);
            assert!(target
                .attempt_planning_field(PlanningChanges::Task(&task), &change, None)
                .unwrap());
            let retained_task = target.task(&task).unwrap().unwrap();
            let retained_project = target.project(&project).unwrap().unwrap();
            let retained_effects = target.pending_task_changes(&task).unwrap();
            let retained_workflow = target.workflow(&task).unwrap();
            let execution = target.revisions().unwrap();
            {
                // This incoming document has no provider-evidence fields. Scalar
                // mapping changes must have the same ownership boundary as evidence.
                let conn = source.conn.lock().unwrap();
                conn.execute(
                    "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                    params![task.as_str(), replacement],
                )
                .unwrap();
                conn.execute(
                    "UPDATE projects SET external_project_id=?2 WHERE id=?1",
                    params![project.as_str(), replacement],
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
            import(&target, "/target", "replacement", &incoming);
            assert_eq!(target.task(&task).unwrap().unwrap(), retained_task);
            assert_eq!(target.project(&project).unwrap().unwrap(), retained_project);
            assert_eq!(
                target.pending_task_changes(&task).unwrap(),
                retained_effects
            );
            assert_eq!(target.workflow(&task).unwrap(), retained_workflow);
            let after = target.revisions().unwrap();
            assert_eq!(
                (after.sessions, after.processes, after.flows),
                (execution.sessions, execution.processes, execution.flows)
            );
            assert!(target.get_wave(independent.id()).unwrap().is_some());
            assert!(!target
                .planning_export_pending(PlanningChanges::Task(&task))
                .unwrap());
            assert!(!target
                .planning_export_pending(PlanningChanges::Project(&project))
                .unwrap());
            assert_eq!(
                target.peer_projection_conflicts("/target").unwrap().len(),
                2
            );
            let retained = export(&target, "/target");
            assert!(incoming
                .changes
                .iter()
                .all(|(id, change)| retained.changes.get(id) == Some(change)));
            import(&target, "/target", "replacement", &incoming);
            assert_eq!(target.revisions().unwrap(), after);
            assert_eq!(export(&target, "/target"), retained);
        }
    }

    #[test]
    fn correspondence_import_retains_creation_origins_and_exact_readback() {
        use super::super::planning_changes::PlanningChanges;
        use crate::engine::planning_exchange::PlanningKind;

        for (kind, reverse) in [PlanningKind::Task, PlanningKind::Project]
            .into_iter()
            .flat_map(|kind| [(kind, false), (kind, true)])
        {
            let (_source_home, source) = store();
            let incoming_task = seed(&source);
            let incoming_project = source.task(&incoming_task).unwrap().unwrap().project_id;
            let (_target_home, target) = store();
            let private = Wave::new(WaveId::new(), "private".into(), "/target".into());
            target.create_wave(&private).unwrap();
            let local_project = target.ensure_project(private.id(), "Private").unwrap();
            let local_task = TaskId::new();
            target
                .create_task(&crate::planning::NewTask {
                    id: local_task.clone(),
                    project_id: local_project.clone(),
                    title: "Private title".into(),
                    description: "Private brief".into(),
                })
                .unwrap();
            preserve_execution(&target, private.id(), &local_task);
            let execution = execution_rows(&target);
            let (origin, owner) = if kind == PlanningKind::Task {
                for (store, project) in [(&source, &incoming_project), (&target, &local_project)] {
                    store.conn.lock().unwrap().execute(
                        "UPDATE projects SET external_project_id='provider-project' WHERE id=?1",
                        [project.as_str()],
                    ).unwrap();
                }
                (
                    PlanningChanges::Task(&incoming_task),
                    PlanningChanges::Task(&local_task),
                )
            } else {
                (
                    PlanningChanges::Project(&incoming_project),
                    PlanningChanges::Project(&local_project),
                )
            };
            source
                .planning_export_error(origin, "Discovery unavailable")
                .unwrap();
            assert!(source.planning_export_pending(origin).unwrap());
            assert!(source.planning_export_attempts(origin).is_err());
            assert!(!export(&source, "/source")
                .changes
                .values()
                .any(|change| change.object.kind == kind && change.field == "creation"));
            let incoming = source
                .prepare_planning_export(origin, "team", "initiative")
                .unwrap();
            let local = target
                .prepare_planning_export(owner, "team", "initiative")
                .unwrap();
            for (store, identity, receipt) in
                [(&source, origin, &incoming), (&target, owner, &local)]
            {
                assert!(store
                    .attempt_planning_export(identity, &receipt.input, false)
                    .unwrap());
                if kind == PlanningKind::Project {
                    assert!(store
                        .attempt_planning_export(identity, &receipt.input, true)
                        .unwrap());
                }
                store
                    .planning_export_error(identity, "lost response")
                    .unwrap();
            }
            let (_, mapping) = super::provider_schema(kind).unwrap();
            for (store, identity) in [(&source, origin), (&target, owner)] {
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        &format!("UPDATE {} SET {mapping}=?2 WHERE id=?1", super::table(kind)),
                        params![identity.owner().1, incoming.id],
                    )
                    .unwrap();
            }
            let first = export(&source, "/source");
            source
                .planning_export_error(origin, "later discovery error")
                .unwrap();
            edit_title(&target, &local_task, "Later private title");
            target
                .edit_project(&local_project, None, Some("Later private summary"))
                .unwrap();
            let later_task = target.pending_task_changes(&local_task).unwrap();
            let later_project = target.pending_project_changes(&local_project).unwrap();
            let document = export(&source, "/source");
            let object = crate::engine::planning_exchange::PlanningObject {
                kind,
                id: origin.owner().1.into(),
            };
            let snapshots = if reverse {
                [&document, &first]
            } else {
                [&first, &document]
            };
            import(&target, "/target", "before-association", snapshots[0]);
            // Import alone neither guesses correspondence nor allocates receipts
            // on the private local owner.
            assert!(target.planning_export_attempts(origin).is_err());
            target
                .associate_peer_planning("/target", &object, owner.owner().1, &incoming.id)
                .unwrap();
            let retain = || import(&target, "/target", "associated", snapshots[1]);
            retain();
            let revisions = target.revisions().unwrap();
            retain();
            assert_eq!(target.revisions().unwrap(), revisions);
            for (identity, receipt) in [(origin, &incoming), (owner, &local)] {
                assert_eq!(
                    target
                        .prepare_planning_export(identity, "ignored", "ignored")
                        .unwrap(),
                    *receipt
                );
                assert_eq!(
                    target.planning_export_attempts(identity).unwrap(),
                    (true, kind == PlanningKind::Project)
                );
                assert!(target
                    .attempt_planning_export(identity, &receipt.input, false)
                    .is_err());
            }
            assert_eq!(target.planning_export_origins(owner).unwrap().len(), 2);
            let conn = target.conn.lock().unwrap();
            let receipt: (String, String, bool) = conn
                .query_row(
                    "SELECT COALESCE(task_id,project_id),export_error,export_acknowledged
                 FROM planning_creations WHERE kind=?1 AND origin_id=?2",
                    params![kind.as_str(), object.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
            assert_eq!(
                receipt,
                (
                    owner.owner().1.into(),
                    "later discovery error".into(),
                    false
                )
            );
            let private_members: i64 = conn
                .query_row(
                    "SELECT count(*) FROM planning_members WHERE object_id IN (?1,?2,?3)",
                    params![
                        local_task.as_str(),
                        local_project.as_str(),
                        private.id().as_str()
                    ],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(private_members, 0);
            let retained = super::export_in(&conn, "/target", &destination()).unwrap();
            assert_eq!(retained, document);
            drop(conn);
            assert_eq!(
                target.pending_task_changes(&local_task).unwrap(),
                later_task
            );
            assert_eq!(
                target.pending_project_changes(&local_project).unwrap(),
                later_project
            );
            assert_eq!(execution_rows(&target), execution);
            assert!(!export(&target, "/target")
                .objects()
                .iter()
                .any(|object| object.id == local_task.as_str()
                    || object.id == local_project.as_str()
                    || object.id == private.id().as_str()
                    || object.id == origin.owner().1));

            // Exact readback settles only the matching origin. The second
            // captured operation stays uncertain even beside a provider mapping.
            let mut observed = incoming.model.clone();
            observed["id"] = json!(incoming.id);
            observed["revision"] = json!("2026-10-09T12:00:00Z");
            if kind == PlanningKind::Project {
                observed["initiative_ids"] = json!(["initiative"]);
                target
                    .put_pm_project(
                        private.id(),
                        "linear",
                        "initiative",
                        &serde_json::from_value(observed).unwrap(),
                        42,
                    )
                    .unwrap();
            } else {
                let mut project = target.planning_project(&local_project).unwrap();
                project.id = "provider-project".into();
                project.initiative_ids = vec!["initiative".into()];
                target
                    .put_pm_task(
                        "/target",
                        "linear",
                        &crate::store::PmTaskRecord {
                            item: serde_json::from_value(observed).unwrap(),
                            project: Some(project),
                            observed_at: 42,
                        },
                        Some((private.id(), "initiative")),
                    )
                    .unwrap();
            }
            assert!(!target.planning_export_pending(origin).unwrap());
            assert!(target.planning_export_pending(owner).unwrap());
            retain();
            assert!(!target.planning_export_pending(origin).unwrap());
            assert!(target.planning_export_pending(owner).unwrap());
            assert_eq!(
                target
                    .prepare_planning_export(owner, "ignored", "ignored")
                    .unwrap(),
                local
            );
            assert_eq!(
                target
                    .prepare_planning_export(origin, "ignored", "ignored")
                    .unwrap(),
                incoming
            );
            let conn = target.conn.lock().unwrap();
            let receipts: Vec<(String,Option<String>,bool)> = conn.prepare(
                "SELECT origin_id,export_error,export_acknowledged FROM planning_creations WHERE kind=?1 ORDER BY origin_id"
            ).unwrap().query_map([kind.as_str()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)))
                .unwrap().collect::<Result<_,_>>().unwrap();
            assert!(receipts.contains(&(origin.owner().1.into(), None, true)));
            assert!(receipts.contains(&(
                owner.owner().1.into(),
                Some("lost response".into()),
                false
            )));
            drop(conn);
            assert_eq!(execution_rows(&target), execution);
            assert!(!export(&target, "/target")
                .objects()
                .iter()
                .any(|object| object.id == local_task.as_str()
                    || object.id == local_project.as_str()
                    || object.id == private.id().as_str()
                    || object.id == origin.owner().1));
            assert!(!super::exists(&target.conn.lock().unwrap(), &object).unwrap());
            target
                .associate_peer_planning("/target", &object, owner.owner().1, &incoming.id)
                .unwrap();
            assert_eq!(execution_rows(&target), execution);

            // A later contradictory peer mapping cannot keep resolving through
            // stale correspondence, nor redirect either retained operation.
            source
                .conn
                .lock()
                .unwrap()
                .execute(
                    &format!(
                        "UPDATE {} SET {mapping}='different-provider' WHERE id=?1",
                        super::table(kind)
                    ),
                    [origin.owner().1],
                )
                .unwrap();
            import(
                &target,
                "/target",
                "changed-mapping",
                &export(&source, "/source"),
            );
            if kind == PlanningKind::Task {
                assert!(target.task_by_issue(&object.id).unwrap().is_none());
            } else {
                assert!(target.project_by_project(&object.id).unwrap().is_none());
            }
            assert!(!target.planning_export_pending(origin).unwrap());
            assert!(target.planning_export_pending(owner).unwrap());
            assert_eq!(
                target
                    .prepare_planning_export(origin, "ignored", "ignored")
                    .unwrap(),
                incoming
            );
            assert_eq!(
                target
                    .prepare_planning_export(owner, "ignored", "ignored")
                    .unwrap(),
                local
            );
            assert_eq!(execution_rows(&target), execution);
            let requests: Vec<String> = target
                .conn
                .lock()
                .unwrap()
                .prepare("SELECT task_id FROM task_creation_intents ORDER BY task_id")
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(
                requests,
                vec![local_task.to_string()],
                "only the original local creation request may exist after import and readback"
            );
        }
    }

    #[test]
    fn peer_creation_identity_defers_legacy_effects_and_cannot_be_rekeyed() {
        use super::super::planning_changes::PlanningChanges;
        use crate::engine::planning_exchange::PlanningKind;

        for kind in [PlanningKind::Task, PlanningKind::Project] {
            let (_home, source) = store();
            let task = seed(&source);
            let project = source.task(&task).unwrap().unwrap().project_id;
            let (owner, other) = if kind == PlanningKind::Task {
                source
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE projects SET external_project_id='provider-project' WHERE id=?1",
                        [project.as_str()],
                    )
                    .unwrap();
                (PlanningChanges::Task(&task), TaskId::new().to_string())
            } else {
                (
                    PlanningChanges::Project(&project),
                    ProjectId::new().to_string(),
                )
            };
            let receipt = source
                .prepare_planning_export(owner, "team", "initiative")
                .unwrap();
            assert!(source
                .attempt_planning_export(owner, &receipt.input, false)
                .unwrap());
            source
                .planning_export_error(owner, "lost creation reply")
                .unwrap();
            let original = export(&source, "/source");
            let mut redirected = original.clone();
            for change in redirected
                .changes
                .values_mut()
                .filter(|c| c.object.kind == kind && c.object.id == owner.owner().1)
            {
                change.object.id.clone_from(&other);
            }
            assert!(redirected
                .validate()
                .unwrap_err()
                .to_string()
                .contains("invalid planning field value"));
            assert_eq!(
                source
                    .prepare_planning_export(owner, "other-team", "other-initiative")
                    .unwrap(),
                receipt
            );
            assert_eq!(
                source.planning_export_attempts(owner).unwrap(),
                (true, false)
            );
            assert_eq!(export(&source, "/source"), original);

            // Response loss can leave no mapping on the incoming Work at all.
            // A different local ID can already own that exact provider UUID.
            // The captured creation input, not an issue name, links the risk.
            let (_target_home, target) = store();
            let legacy_task = seed(&target);
            let legacy_project = target.task(&legacy_task).unwrap().unwrap().project_id;
            let legacy_wave = target.project(&legacy_project).unwrap().unwrap().wave_id;
            target
                .conn
                .lock()
                .unwrap()
                .execute("UPDATE waves SET name='legacy' WHERE id=?1", [legacy_wave])
                .unwrap();
            let (legacy_owner, change) = if kind == PlanningKind::Task {
                target
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                        params![legacy_task.as_str(), receipt.id],
                    )
                    .unwrap();
                edit_title(&target, &legacy_task, "Legacy save");
                (
                    PlanningChanges::Task(&legacy_task),
                    target.pending_task_changes(&legacy_task).unwrap().remove(0),
                )
            } else {
                target
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE projects SET external_project_id=?2 WHERE id=?1",
                        params![legacy_project.as_str(), receipt.id],
                    )
                    .unwrap();
                target
                    .edit_project(&legacy_project, Some("Legacy save"), None)
                    .unwrap();
                (
                    PlanningChanges::Project(&legacy_project),
                    target
                        .pending_project_changes(&legacy_project)
                        .unwrap()
                        .remove(0),
                )
            };
            import(&target, "/source", "lost-creation", &original);
            let error = target
                .attempt_planning_field(legacy_owner, &change, None)
                .unwrap_err();
            assert!(error.to_string().contains(owner.owner().1), "{error}");
            let retained = export(&target, "/source");
            assert!(original
                .changes
                .iter()
                .all(|(id, change)| retained.changes.get(id) == Some(change)));
            assert!(!super::exists(
                &target.conn.lock().unwrap(),
                &crate::engine::planning_exchange::PlanningObject {
                    kind,
                    id: owner.owner().1.into(),
                }
            )
            .unwrap());
        }
    }

    #[test]
    fn correspondence_lookup_keeps_one_snapshot_during_mapping_changes() {
        use std::cell::RefCell;

        use rusqlite::trace::{TraceEvent, TraceEventCodes};

        use crate::engine::planning_exchange::{PlanningKind, PlanningObject};

        thread_local! {
            static WRITER: RefCell<Option<rusqlite::Connection>> = const { RefCell::new(None) };
        }
        fn change_mapping(event: TraceEvent<'_>) {
            if matches!(event, TraceEvent::Stmt(_, sql) if sql.contains("FROM planning_associations a JOIN"))
            {
                if let Some(writer) = WRITER.with_borrow_mut(Option::take) {
                    writer
                        .execute_batch(
                            "UPDATE tasks SET external_issue_id=NULL;
                         UPDATE projects SET external_project_id=NULL;",
                        )
                        .unwrap();
                }
            }
        }

        let (_source_home, source) = store();
        let (_, row, incoming_task) = linear_seed(&source);
        let incoming_project = source.task(&incoming_task).unwrap().unwrap().project_id;
        let journal = export(&source, "/source");
        for project in [false, true] {
            let (home, target) = store();
            let (_, _, local_task) = linear_seed(&target);
            let local_project = target.task(&local_task).unwrap().unwrap().project_id;
            import(&target, "/source", "fixture", &journal);
            let (origin, local, provider) = if project {
                (
                    PlanningObject {
                        kind: PlanningKind::Project,
                        id: incoming_project.to_string(),
                    },
                    local_project.as_str(),
                    row.snapshot.projects[0].id.as_str(),
                )
            } else {
                (
                    PlanningObject {
                        kind: PlanningKind::Task,
                        id: incoming_task.to_string(),
                    },
                    local_task.as_str(),
                    row.snapshot.items[0].id.as_str(),
                )
            };
            target
                .associate_peer_planning("/source", &origin, local, provider)
                .unwrap();
            let read = || {
                if project {
                    serde_json::to_value(target.project_by_project(&origin.id).unwrap()).unwrap()
                } else {
                    serde_json::to_value(target.task_by_issue(&origin.id).unwrap()).unwrap()
                }
            };
            let before = read();
            assert!(!before.is_null());
            // Commit on another real WAL connection between physical lookup and
            // correspondence resolution. No timing sleeps or production hook.
            WRITER.with_borrow_mut(|writer| {
                *writer = Some(rusqlite::Connection::open(home.path().join("store.db")).unwrap());
            });
            target
                .conn
                .lock()
                .unwrap()
                .trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(change_mapping));
            assert_eq!(read(), before, "lookup must return one complete snapshot");
            assert!(WRITER.with_borrow(Option::is_none));
            assert!(
                read().is_null(),
                "the next lookup must recheck the changed mapping"
            );
            assert!(target
                .task(&local_task)
                .unwrap()
                .unwrap()
                .plan
                .linear_id
                .is_none());
            assert!(target
                .project(&local_project)
                .unwrap()
                .unwrap()
                .plan
                .linear_id
                .is_none());
        }
    }

    #[test]
    fn associated_readback_captures_foreign_predecessor_without_sharing_private_history() {
        use crate::engine::planning_exchange::{PlanningKind, PlanningObject};

        let (_source_home, source) = store();
        let (_, row, incoming_task) = linear_seed(&source);
        let incoming_project = source.task(&incoming_task).unwrap().unwrap().project_id;
        let incoming = export(&source, "/source");
        let (_target_home, target) = store();
        let private = Wave::new(WaveId::new(), "private".into(), "/target".into());
        target.create_wave(&private).unwrap();
        let mut local_row = row.clone();
        local_row.wave_id = private.id().clone();
        target.put_pm_snapshot(&local_row).unwrap();
        let local = target
            .task_by_issue(&row.snapshot.items[0].id)
            .unwrap()
            .unwrap();
        preserve_execution(&target, private.id(), &local.id);
        let execution = execution_rows(&target);
        import(&target, "/target", "incoming", &incoming);
        for (kind, origin, owner, provider) in [
            (
                PlanningKind::Task,
                incoming_task.as_str(),
                local.id.as_str(),
                row.snapshot.items[0].id.as_str(),
            ),
            (
                PlanningKind::Project,
                incoming_project.as_str(),
                local.project_id.as_str(),
                row.snapshot.projects[0].id.as_str(),
            ),
        ] {
            target
                .associate_peer_planning(
                    "/target",
                    &PlanningObject {
                        kind,
                        id: origin.into(),
                    },
                    owner,
                    provider,
                )
                .unwrap();
        }
        // Retention alone is not observation. The common acquisition writer now
        // reads the same fact, even though its value/revision was already local.
        target.put_pm_snapshot(&local_row).unwrap();
        edit_title(&target, &local.id, "After observed peer fact");
        target
            .edit_project(&local.project_id, Some("After observed Project fact"), None)
            .unwrap();
        let mut content = super::super::project_content::read_content(
            &target.conn.lock().unwrap(),
            &local.project_id,
        )
        .unwrap();
        content.workflow = "reviewed-peer".into();
        target
            .update_project_content(&local.project_id, &content)
            .unwrap();
        assert_eq!(execution_rows(&target), execution);
        assert!(!export(&target, "/target")
            .objects()
            .iter()
            .any(|o| o.id == local.id.as_str()
                || o.id == local.project_id.as_str()
                || o.id == private.id().as_str()));

        // Selecting the local plan elsewhere must not pull the observed origin's
        // history into that destination. Validation can read it; export cannot.
        let separate = PlanningDestination::new(
            "/synthetic/separate",
            "refs/loopflow/planning/shared/separate",
        )
        .unwrap();
        target.bind_peer_planning("/target", &separate).unwrap();
        target
            .select_peer_waves(
                "/target",
                &separate.id(),
                std::slice::from_ref(private.id()),
            )
            .unwrap();
        let retained =
            super::export_in(&target.conn.lock().unwrap(), "/target", &separate.id()).unwrap();
        let exchanged = PlanningSnapshot::from_bytes(&retained.to_bytes().unwrap()).unwrap();
        let public = target
            .export_peer_planning("/target", &separate.id())
            .unwrap();
        public.validate().unwrap();
        assert!(!public.objects().iter().any(|o| o.id == local.id.as_str()
            || o.id == incoming_task.as_str()
            || o.id == local.project_id.as_str()
            || o.id == incoming_project.as_str()));
        // An unchanged readback must preserve the subsequent local intention,
        // without relabeling it as Linear or repeatedly capturing the bridge.
        target.put_pm_snapshot(&local_row).unwrap();
        assert_eq!(
            super::export_in(&target.conn.lock().unwrap(), "/target", &separate.id()).unwrap(),
            retained
        );
        for (kind, origin, owner, field, value) in [
            (
                PlanningKind::Task,
                incoming_task.as_str(),
                local.id.as_str(),
                "issue_title",
                "After observed peer fact",
            ),
            (
                PlanningKind::Project,
                incoming_project.as_str(),
                local.project_id.as_str(),
                "project_name",
                "After observed Project fact",
            ),
            (
                PlanningKind::Project,
                incoming_project.as_str(),
                local.project_id.as_str(),
                "workflow",
                "reviewed-peer",
            ),
        ] {
            let (_, saved) = exchanged
                .winners()
                .find(|(_, c)| c.object.id == owner && c.field == field)
                .unwrap();
            assert_eq!(saved.value, value);
            let baseline = super::linear_predecessor(&exchanged, saved).unwrap();
            let (peer_id, peer) = incoming
                .winners()
                .find(|(_, c)| c.object.kind == kind && c.object.id == origin && c.field == field)
                .unwrap();
            assert_eq!(
                baseline,
                serde_json::json!({"value":peer.value,"revision":peer.linear.as_ref().unwrap().revision()})
            );
            assert!(saved
                .parents
                .iter()
                .any(|id| exchanged.changes[id].parents.contains(peer_id)));
            assert_eq!(exchanged.changes[peer_id], *peer);
        }
        import(&target, "/target", "repeat", &incoming);
        assert_eq!(execution_rows(&target), execution);
        assert_eq!(
            super::export_in(&target.conn.lock().unwrap(), "/target", &separate.id()).unwrap(),
            retained
        );
        assert!(!export(&target, "/target")
            .objects()
            .iter()
            .any(|o| o.id == local.id.as_str() || o.id == local.project_id.as_str()));
        assert_eq!(
            target
                .conn
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM task_creation_intents", [], |row| row
                    .get::<_, i64>(
                    0
                ))
                .unwrap(),
            0,
            "provider acquisition and peer import never create local requests"
        );
    }

    #[test]
    fn explicit_correspondence_resolves_local_work_but_retains_projection_holds() {
        use super::super::planning_changes::PlanningChanges;
        use crate::engine::planning_exchange::{PlanningKind, PlanningObject};

        let (_home, source) = store();
        let (_, row, incoming_task) = linear_seed(&source);
        let incoming_project = source.task(&incoming_task).unwrap().unwrap().project_id;
        let first = export(&source, "/source");
        edit_title(&source, &incoming_task, "Peer later title");
        source
            .edit_project(&incoming_project, Some("Peer later Project"), None)
            .unwrap();
        let later = export(&source, "/source");

        for reverse in [false, true] {
            let (_home, target) = store();
            let private = Wave::new(WaveId::new(), "private".into(), "/target".into());
            target.create_wave(&private).unwrap();
            let mut local_row = row.clone();
            local_row.wave_id = private.id().clone();
            target.put_pm_snapshot(&local_row).unwrap();
            let local = target
                .task_by_issue(&row.snapshot.items[0].id)
                .unwrap()
                .unwrap();
            preserve_execution(&target, private.id(), &local.id);
            let execution = execution_rows(&target);
            edit_title(&target, &local.id, "Private Task save");
            target
                .edit_project(&local.project_id, Some("Private Project save"), None)
                .unwrap();
            let task_change = target.pending_task_changes(&local.id).unwrap().remove(0);
            let project_change = target
                .pending_project_changes(&local.project_id)
                .unwrap()
                .remove(0);
            for (owner, change) in [
                (PlanningChanges::Task(&local.id), &task_change),
                (PlanningChanges::Project(&local.project_id), &project_change),
            ] {
                assert!(target
                    .attempt_planning_field(
                        owner,
                        change,
                        change
                            .base
                            .as_ref()
                            .and_then(|base| base["revision"].as_str())
                    )
                    .unwrap());
                target
                    .planning_field_error(owner, change, "lost reply")
                    .unwrap();
            }
            let retained_task = target.planning_task(&local.id).unwrap();
            let retained_project = target.planning_project(&local.project_id).unwrap();
            let task_receipts = target.pending_task_changes(&local.id).unwrap();
            let project_receipts = target.pending_project_changes(&local.project_id).unwrap();
            let task = PlanningObject {
                kind: PlanningKind::Task,
                id: incoming_task.to_string(),
            };
            let project = PlanningObject {
                kind: PlanningKind::Project,
                id: incoming_project.to_string(),
            };
            // Neither names nor provider IDs implicitly redirect a full Work ID.
            assert!(target.task_by_issue(&task.id).unwrap().is_none());
            assert!(target.project_by_project(&project.id).unwrap().is_none());
            assert!(target
                .associate_peer_planning(
                    "/target",
                    &task,
                    local.id.as_str(),
                    &row.snapshot.items[0].id
                )
                .is_err());
            let snapshots = if reverse {
                [&later, &first]
            } else {
                [&first, &later]
            };
            import(&target, "/target", "first", snapshots[0]);
            for (origin, local_id, provider) in [
                (&task, local.id.as_str(), row.snapshot.items[0].id.as_str()),
                (
                    &project,
                    local.project_id.as_str(),
                    row.snapshot.projects[0].id.as_str(),
                ),
            ] {
                assert!(target
                    .associate_peer_planning("/target", origin, local_id, "unrelated-provider")
                    .is_err());
                assert!(target
                    .associate_peer_planning("/source", origin, local_id, provider)
                    .is_err());
                target
                    .associate_peer_planning("/target", origin, local_id, provider)
                    .unwrap();
                let revision = target.revisions().unwrap();
                target
                    .associate_peer_planning("/target", origin, local_id, provider)
                    .unwrap();
                assert_eq!(target.revisions().unwrap(), revision);
            }
            import(&target, "/target", "second", snapshots[1]);
            import(&target, "/target", "repeat", snapshots[1]);
            assert_eq!(
                target.task_by_issue(&task.id).unwrap().unwrap().id,
                local.id
            );
            assert_eq!(
                target.task_by_issue(local.id.as_str()).unwrap().unwrap().id,
                local.id
            );
            assert_eq!(
                target.project_by_project(&project.id).unwrap().unwrap().id,
                local.project_id
            );
            assert_eq!(
                target
                    .project_by_project(local.project_id.as_str())
                    .unwrap()
                    .unwrap()
                    .id,
                local.project_id
            );
            assert_eq!(
                target.resolve_task_id(&task.id, Some("/source")).unwrap(),
                None
            );
            assert!(target.task(&incoming_task).unwrap().is_none());
            assert!(target.project(&incoming_project).unwrap().is_none());
            assert_eq!(target.planning_task(&local.id).unwrap(), retained_task);
            assert_eq!(
                target.planning_project(&local.project_id).unwrap(),
                retained_project
            );
            assert_eq!(
                target.pending_task_changes(&local.id).unwrap(),
                task_receipts
            );
            assert_eq!(
                target.pending_project_changes(&local.project_id).unwrap(),
                project_receipts
            );
            assert_eq!(execution_rows(&target), execution);
            let retained =
                super::export_in(&target.conn.lock().unwrap(), "/target", &destination()).unwrap();
            assert_eq!(retained, later);
            assert!(!export(&target, "/target")
                .objects()
                .iter()
                .any(|o| o == &&task
                    || o == &&project
                    || o.id == local.id.as_str()
                    || o.id == local.project_id.as_str()));
            let status = target.peer_planning_status("/target").unwrap();
            for object in [&task, &project] {
                let reasons: Vec<_> = status
                    .iter()
                    .flat_map(|d| &d.conflicts)
                    .filter(|c| &c.object == object)
                    .map(|c| c.reason.as_str())
                    .collect();
                // One current explanation, not both the skipped-projection
                // receipt and its independently derived association hold.
                assert_eq!(reasons, [super::ASSOCIATION_PROJECTION_PENDING]);
            }
            // Further local saves remain possible; correspondence grants no effect.
            edit_title(&target, &local.id, "Next private save");
            let pending = target.pending_task_changes(&local.id).unwrap();
            let next = pending.iter().find(|c| c.id != task_change.id).unwrap();
            assert!(target
                .attempt_planning_field(PlanningChanges::Task(&local.id), next, None)
                .is_err());
        }
    }

    #[test]
    fn duplicate_provider_ids_defer_legacy_effects_without_sharing_private_work() {
        use super::super::planning_changes::PlanningChanges;

        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let (_, mut row, incoming_task) = linear_seed(&source);
        let incoming_project = source.task(&incoming_task).unwrap().unwrap().project_id;
        let private = Wave::new(WaveId::new(), "private".into(), "/target".into());
        target.create_wave(&private).unwrap();
        row.wave_id = private.id().clone();
        target.put_pm_snapshot(&row).unwrap();
        let legacy = target
            .task_by_issue(&row.snapshot.items[0].id)
            .unwrap()
            .unwrap();
        assert_ne!(legacy.id, incoming_task);
        assert_ne!(legacy.project_id, incoming_project);
        preserve_execution(&target, private.id(), &legacy.id);
        target.conn.lock().unwrap().execute(
            "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
             VALUES(?1,?2,1,'retained','retained-branch','retained-base',1,1)",
            params![crate::work::task::TaskPrId::new().as_str(), legacy.id.as_str()],
        ).unwrap();
        let execution = execution_rows(&target);

        // Keep uncertainty on both sides. The incoming deletion cannot project;
        // the private legacy field receipts must not be retired or replayed.
        source.delete_task(&incoming_task).unwrap();
        let deletion = deletion_change(&source, &incoming_task);
        assert!(source
            .attempt_planning_field(
                PlanningChanges::Task(&incoming_task),
                &deletion,
                row.snapshot.items[0].revision.as_deref(),
            )
            .unwrap());
        source
            .planning_field_error(
                PlanningChanges::Task(&incoming_task),
                &deletion,
                "lost deletion reply",
            )
            .unwrap();
        edit_title(&target, &legacy.id, "Private uncertain title");
        target
            .edit_project(&legacy.project_id, Some("Private uncertain Project"), None)
            .unwrap();
        let task_change = target.pending_task_changes(&legacy.id).unwrap().remove(0);
        let project_change = target
            .pending_project_changes(&legacy.project_id)
            .unwrap()
            .remove(0);
        for (owner, change, revision) in [
            (
                PlanningChanges::Task(&legacy.id),
                &task_change,
                row.snapshot.items[0].revision.as_deref(),
            ),
            (
                PlanningChanges::Project(&legacy.project_id),
                &project_change,
                row.snapshot.projects[0].revision.as_deref(),
            ),
        ] {
            assert!(target
                .attempt_planning_field(owner, change, revision)
                .unwrap());
            target
                .planning_field_error(owner, change, "lost local reply")
                .unwrap();
        }

        let incoming = export(&source, "/source");
        import(&target, "/target", "divergent", &incoming);
        assert!(target.task(&incoming_task).unwrap().is_none());
        assert!(target.project(&incoming_project).unwrap().is_none());
        // Common acquisition and saves stay available even while delivery waits.
        target.put_pm_snapshot(&row).unwrap();
        edit_title(&target, &legacy.id, "Later private title");
        target
            .edit_project(&legacy.project_id, None, Some("Later private summary"))
            .unwrap();
        let later_task = target
            .pending_task_changes(&legacy.id)
            .unwrap()
            .into_iter()
            .find(|c| c.id != task_change.id)
            .unwrap();
        let later_project = target
            .pending_project_changes(&legacy.project_id)
            .unwrap()
            .into_iter()
            .find(|c| c.id != project_change.id)
            .unwrap();
        for (owner, change, revision, rejected) in [
            (
                PlanningChanges::Task(&legacy.id),
                &later_task,
                row.snapshot.items[0].revision.as_deref(),
                incoming_task.as_str(),
            ),
            (
                PlanningChanges::Project(&legacy.project_id),
                &later_project,
                row.snapshot.projects[0].revision.as_deref(),
                incoming_project.as_str(),
            ),
        ] {
            let error = target
                .attempt_planning_field(owner, change, revision)
                .unwrap_err();
            assert!(error.to_string().contains(rejected), "{error}");
        }
        let receipts = || {
            let conn = target.conn.lock().unwrap();
            [(&task_change, "task_changes"), (&project_change, "project_changes"),
             (&later_task, "task_changes"), (&later_project, "project_changes")]
                .map(|(change, table)| conn.query_row(
                    &format!("SELECT attempted,acknowledged,error,conflict_json FROM {table} WHERE id=?1"),
                    [&change.id], |r| Ok((r.get::<_, bool>(0)?, r.get::<_, bool>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?)),
                ).unwrap())
        };
        let before = receipts();
        assert_eq!(
            before[0],
            (true, false, Some("lost local reply".into()), None)
        );
        assert_eq!(before[1], before[0]);
        assert_eq!(before[2], (false, false, None, None));
        assert_eq!(before[3], before[2]);
        let retained = export(&target, "/target");
        assert_eq!(retained, incoming);
        assert!(!retained
            .objects()
            .iter()
            .any(|o| o.id == legacy.id.as_str() || o.id == legacy.project_id.as_str()));
        let revisions = target.revisions().unwrap();
        import(&target, "/target", "divergent", &incoming);
        assert_eq!(target.revisions().unwrap(), revisions);
        assert_eq!(receipts(), before);
        assert_eq!(execution_rows(&target), execution);
        assert_eq!(
            target
                .task_by_issue(&legacy.plan.identifier)
                .unwrap()
                .unwrap()
                .id,
            legacy.id
        );
        assert_eq!(
            target
                .task_by_issue(legacy.id.as_str())
                .unwrap()
                .unwrap()
                .id,
            legacy.id
        );
        assert_eq!(export(&target, "/target"), retained);

        // A later null mapping cannot hide the disputed provider ownership or
        // turn the incoming IDs into new creation candidates. Losing mappings
        // and the attempted deletion still belong to the retained journal.
        {
            let conn = source.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET external_issue_id=NULL WHERE id=?1",
                [incoming_task.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE projects SET external_project_id=NULL WHERE id=?1",
                [incoming_project.as_str()],
            )
            .unwrap();
        }
        let masked = export(&source, "/source");
        import(&target, "/target", "masked-mapping", &masked);
        assert!(target.task(&incoming_task).unwrap().is_none());
        assert!(target.project(&incoming_project).unwrap().is_none());
        for (owner, change) in [
            (PlanningChanges::Task(&legacy.id), &later_task),
            (PlanningChanges::Project(&legacy.project_id), &later_project),
        ] {
            assert!(target
                .attempt_planning_field(owner, change, None)
                .unwrap_err()
                .to_string()
                .contains("retained peer projection conflict"));
        }
        assert_eq!(receipts(), before);
        assert_eq!(execution_rows(&target), execution);
        assert_eq!(export(&target, "/target"), masked);

        // Exact provider kind and identity, not Project membership or a matching
        // title, scope deferral. A Task whose provider ID equals the conflicted
        // Project's ID still has an independent effect.
        let independent = TaskId::new();
        target.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
             VALUES(?1,?2,?3,'OTHER-1','Private uncertain title','',1,1,'')",
            params![independent.as_str(), legacy.project_id.as_str(), row.snapshot.projects[0].id],
        ).unwrap();
        edit_title(&target, &independent, "Independent edit");
        let change = target.pending_task_changes(&independent).unwrap().remove(0);
        assert!(target
            .attempt_planning_field(PlanningChanges::Task(&independent), &change, None)
            .unwrap());
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
            .any(|c| c.object.id == task.as_str() && c.reason.contains("planning mapping")));
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

        // Retention is not recovery: clearing the legacy mapping would turn it
        // into a new provider-creation candidate while leaving its issue selector
        // and effects behind. No raw-SQL shortcut stands in for association.
        import(&right, "/target", "still-held", &Default::default());
        assert_eq!(
            right.peer_projection_conflicts("/target").unwrap(),
            conflicts
        );
        assert_eq!(
            right.task_by_issue("provider-1").unwrap().unwrap().id,
            legacy
        );
        assert!(right.task(&task).unwrap().is_none());
        assert_eq!(export(&right, "/target"), expected);
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
            super::capture_project_content(&conn, &moved_project).unwrap();
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
    #[test]
    fn peer_ordering_keeps_save_order_and_private_history() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let first = seed(&source);
        let project = source.task(&first).unwrap().unwrap().project_id;
        let second = TaskId::new();
        {
            let conn = source.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug,planning_rank)
                VALUES(?1,?2,'FIX-2','Second','',2,2,'',1)",params![second.as_str(),project.as_str()]).unwrap();
        }
        let reorder = |store: &SqliteStore, task: &TaskId| {
            let current = store.task(task).unwrap().unwrap();
            store
                .edit_task(
                    task,
                    current.plan.revision,
                    &crate::pm::PmItemUpdate {
                        rank: Some(0),
                        ..Default::default()
                    },
                )
                .unwrap();
        };
        reorder(&source, &second);
        let old = source
            .pending_project_changes(&project)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "task_order")
            .unwrap();
        import(&target, "/target", "one", &export(&source, "/source"));
        reorder(&target, &first);
        let latest = target
            .pending_project_changes(&project)
            .unwrap()
            .into_iter()
            .find(|c| c.field == "task_order")
            .unwrap();
        // A late diagnostic on the old move must not promote its intention.
        source
            .planning_field_error(
                super::PlanningChanges::Project(&project),
                &old,
                "late reply",
            )
            .unwrap();
        import(&source, "/source", "two", &export(&target, "/target"));
        import(&target, "/target", "three", &export(&source, "/source"));
        for store in [&source, &target] {
            assert_eq!(
                store
                    .pending_project_changes(&project)
                    .unwrap()
                    .into_iter()
                    .find(|c| c.field == "task_order")
                    .unwrap()
                    .id,
                latest.id
            );
            assert_eq!(
                store
                    .planning_task(&first)
                    .unwrap()
                    .record
                    .unwrap()
                    .item
                    .rank,
                0
            );
            assert_eq!(
                store
                    .planning_task(&second)
                    .unwrap()
                    .record
                    .unwrap()
                    .item
                    .rank,
                1
            );
        }
        let before = target.task(&first).unwrap().unwrap();
        let snapshot = export(&source, "/source");
        import(&target, "/target", "three", &snapshot);
        assert_eq!(target.task(&first).unwrap().unwrap(), before);
        assert_eq!(export(&target, "/target"), snapshot);

        // A previously selected Task moving into private ancestry also holds the
        // Project's historical order lists; order payloads cannot bypass selection.
        let private = Wave::new(WaveId::new(), "private".into(), "/source".into());
        source.create_wave(&private).unwrap();
        let private_project = ProjectId::new();
        {
            let conn = source.conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,created_at,project_name) VALUES(?1,?2,1,'Private')",
                params![private_project.as_str(),private.id()]).unwrap();
            super::capture_project_content(&conn, &private_project).unwrap();
            conn.execute(
                "UPDATE tasks SET project_id=?2 WHERE id=?1",
                params![second.as_str(), private_project.as_str()],
            )
            .unwrap();
        }
        let held = export(&source, "/source");
        assert!(!held
            .changes
            .values()
            .any(|change| change.object.id == project.as_str()));
        assert!(source
            .peer_projection_conflicts("/source")
            .unwrap()
            .iter()
            .any(|c| c.object.id == project.as_str()));
    }

    #[test]
    fn peer_ordering_rejects_malformed_effect_without_advancing_import() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let first = seed(&source);
        let project = source.task(&first).unwrap().unwrap().project_id;
        let conn = source.conn.lock().unwrap();
        super::PlanningChanges::Project(&project)
            .record_value(&conn, "task_order", "order-fixture", json!([first]), None)
            .unwrap();
        drop(conn);
        let mut snapshot = export(&source, "/source");
        let mutation = snapshot
            .changes
            .values_mut()
            .find(|c| c.order_receipt().is_some())
            .unwrap();
        mutation.value["effects"] = json!([{"before":[first],"after":[first],"issue":"FIX-1",
            "input":{"sortOrder":0,"prioritySortOrder":0,"process_lfid":"forbidden"},"settled":false}]);
        mutation.value["attempted"] = json!(true);
        assert!(target
            .import_peer_planning("/target", &destination(), "bad", &snapshot)
            .is_err());
        assert_eq!(import_revision(&target, "/target", &destination()), None);
        assert!(target.task(&first).unwrap().is_none());
    }
    #[test]
    fn peer_ordering_retains_competing_attempts_without_authorizing_another() {
        let (_source_home, source) = store();
        let (_target_home, target) = store();
        let task = seed(&source);
        let project = source.task(&task).unwrap().unwrap().project_id;
        {
            let conn = source.conn.lock().unwrap();
            super::PlanningChanges::Project(&project)
                .record_value(&conn, "task_order", "order-fixture", json!([task]), None)
                .unwrap();
        }
        let mut snapshot = export(&source, "/source");
        import(&target, "/target", "initial", &snapshot);
        let (parent, saved) = snapshot
            .changes
            .iter()
            .find(|(_, c)| c.order_receipt().is_some())
            .unwrap();
        let parent = parent.clone();
        let saved = saved.clone();
        for (name, sort) in [("attempt-a", 1), ("attempt-b", 2)] {
            let mut change = saved.clone();
            change.clock += sort;
            change.parents = [parent.clone()].into();
            change.value["attempted"] = json!(true);
            change.value["effects"] = json!([{"before":[task],"after":[task],"issue":"FIX-1",
                "input":{"sortOrder":sort,"prioritySortOrder":0},"settled":false}]);
            snapshot.changes.insert(name.into(), change);
        }
        import(&target, "/target", "competing", &snapshot);
        assert!(target
            .peer_projection_conflicts("/target")
            .unwrap()
            .iter()
            .any(|c| c.object.id == project.as_str()
                && c.reason.contains("competing planning ordering receipts")));
        assert_eq!(export(&target, "/target"), snapshot);
        let delivery = target.project_order_delivery(&project).unwrap().unwrap();
        assert!(delivery.effects.is_empty());
        let effect =
            serde_json::from_value(snapshot.changes["attempt-a"].value["effects"][0].clone())
                .unwrap();
        assert!(target
            .attempt_project_order(&project, &delivery.id, &effect)
            .unwrap_err()
            .to_string()
            .contains("deferred"));
        import(&target, "/target", "competing", &snapshot);
        assert_eq!(export(&target, "/target"), snapshot);
    }
}
