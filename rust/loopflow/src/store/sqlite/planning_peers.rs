//! Peer receipts and projection commit together on the existing planning tables.
//! Export reads retained mutation identities; it never creates a new edit.

mod recovery;

use std::collections::{btree_map::Entry, BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::durable::{ProjectId, TaskId};
use crate::engine::planning_exchange::{
    winning_heads, winning_heads_by, LinearObservation, PlanningKind, PlanningMutation,
    PlanningObject, PlanningSnapshot,
};
use crate::engine::planning_git::PlanningDestination;
use crate::id::WaveId;
use crate::store::{PeerPlanningStatus, PeerProjectionConflict, StoreError, StoreResult};

use super::planning::ProviderEvidence;
use super::planning_changes::{DeletionReceipt, PlanningChanges};
use super::planning_export::CreationReceipt;
use super::planning_order::OrderReceipt;
use super::SqliteStore;

const SHARING_PROJECTION_PENDING: &str = "peer projection skipped by sharing hold; import required";

// Projection and delivery share the same field-keyed winners, including the
// mutation identity and provenance. Retries never rebuild a value-only index.
type WinningFields<'a> = BTreeMap<&'a str, (&'a str, &'a PlanningMutation)>;

// Prepare scalar winners and decode validated receipt/evidence histories before
// projection retries. The snapshot retains every mutation, loser and causal link.
#[derive(Default)]
struct ObjectChanges<'a> {
    winners: WinningFields<'a>,
    observations: Vec<&'a LinearObservation>,
    creation: BTreeMap<&'a PlanningObject, (CreationReceipt, Vec<CreationReceipt>)>,
    observed: Vec<(&'a str, &'a PlanningMutation)>,
    deletions: BTreeMap<&'a str, Vec<DeletionReceipt>>,
    orders: BTreeMap<&'a str, FieldHistory<OrderReceipt>>,
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

fn changes_by_object<'a>(
    snapshot: &'a PlanningSnapshot,
    owners: &'a BTreeMap<PlanningObject, PlanningObject>,
) -> StoreResult<BTreeMap<PlanningObject, ObjectChanges<'a>>> {
    let heads: BTreeMap<_, _> = snapshot.heads().collect();
    let mut fields = BTreeMap::<_, BTreeSet<_>>::new();
    // Every retained field has a head. Check each origin's completeness before
    // grouping aliases, without revisiting all of its superseded mutations.
    for change in heads.values() {
        fields
            .entry(&change.object)
            .or_default()
            .insert(change.field.as_str());
    }
    for (origin, fields) in fields {
        if !origin
            .kind
            .fields()
            .iter()
            .all(|field| fields.contains(field))
        {
            return Err(invalid(format!("incomplete planning record {}", origin.id)));
        }
    }
    let mut objects: BTreeMap<_, ObjectChanges<'_>> = BTreeMap::new();
    // Receipts and independent provider evidence keep their own merge protocols;
    // only scalar planning fields participate in the joint projection frontier.
    for (id, change) in snapshot.frontier_by(|object| &owners[object]) {
        if change.object.kind.fields().contains(&change.field.as_str()) {
            objects
                .entry(owners[&change.object].clone())
                .or_default()
                .observed
                .push((id, change));
        }
    }
    // Group by resolved owner once. The same accepted frontier supplies both
    // field winners and the sources retained for the next local save.
    for changes in objects.values_mut() {
        changes.winners = winning_heads_by(changes.observed.iter().copied(), |_| ())
            .map(|(id, change)| (change.field.as_str(), (id, change)))
            .collect();
    }
    // Creation keeps per-origin winners, not the joint scalar owner. Reuse the
    // same head index used below for receipt histories.
    for (_, change) in winning_heads(
        heads
            .iter()
            .filter(|(_, change)| change.field == "creation")
            .map(|(id, change)| (*id, *change)),
    ) {
        objects
            .get_mut(&owners[&change.object])
            .expect("creation has an owner")
            .creation
            .insert(
                &change.object,
                (serde_json::from_value(change.value.clone())?, Vec::new()),
            );
    }
    for (id, change) in &snapshot.changes {
        let is_head = heads.contains_key(id.as_str());
        let object = objects
            .get_mut(&owners[&change.object])
            .expect("every retained object has a winning field");
        if change.field == "creation" {
            object
                .creation
                .get_mut(&change.object)
                .expect("creation has a frontier")
                .1
                .push(serde_json::from_value(change.value.clone())?);
        }
        if let Some(receipt) = change.deletion_receipt() {
            object
                .deletions
                .entry(receipt)
                .or_default()
                .push(serde_json::from_value(change.value.clone())?);
        }
        if let Some(receipt) = change.order_receipt() {
            object
                .orders
                .entry(receipt)
                .or_default()
                .push(serde_json::from_value(change.value.clone())?, is_head);
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
    // Removal ages select the first known timestamp, never random mutation-ID order.
    for changes in objects.values_mut() {
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
    let mut heads = BTreeMap::<String, PlanningMutation>::new();
    let mut rows = query.query([project.as_str()])?;
    while let Some(row) = rows.next()? {
        heads.insert(row.get(7)?, read_mutation(row)?);
    }
    let winners: BTreeMap<_, _> = winning_heads_by(
        heads.iter().map(|(id, change)| (id.as_str(), change)),
        |_| (),
    )
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
            heads.values().any(|head| {
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
                "SELECT EXISTS(SELECT 1 FROM planning_peer_capture_heads
             WHERE kind='project' AND object_id=?1 AND field=?2 AND missing_parent)",
                params![project.as_str(), field],
                |row| row.get(0),
            )?;
        if unchanged && frontier_retained && !missing_origin {
            continue;
        }
        conn.execute("INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
            SELECT lower(hex(randomblob(16))),'project',?1,?2,?3,
                max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),?4,
                (SELECT json_group_array(h.id) FROM planning_peer_capture_heads h
                    WHERE h.kind='project' AND h.object_id=?1 AND h.field=?2
                        AND (h.accepted OR ?4 IS NOT NULL))",
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
        // Lookup alone never settles retained effects. Import must project the
        // joint frontier before it can clear this conflict.
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
            // Both views start at the same validated journal. Recovery may add
            // private dependencies; only the selected copy can describe export.
            let retained = match export_in(&tx, repo, &id) {
                Ok(snapshot) => Some(snapshot),
                Err(StoreError::InvalidData(_) | StoreError::Serde(_)) => None,
                Err(error) => return Err(error),
            };
            let selected = retained
                .as_ref()
                .map(|snapshot| export_selected(&tx, repo, &id, snapshot.clone()))
                .transpose();
            let (pending_local, local_error) = match selected {
                Ok(Some((snapshot, held))) => {
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
                Ok(None) | Err(StoreError::InvalidData(_) | StoreError::Serde(_)) => (
                    None,
                    Some("Local planning journal is invalid; pending changes and sharing holds are unknown. Retained plans and sync receipts are unchanged.".into()),
                ),
                Err(error) => return Err(error),
            };
            let (records, recovery_error) = match retained
                .map(|snapshot| recovery::records(&tx, repo, snapshot))
                .transpose()
            {
                Ok(Some(records)) => (records, None),
                Ok(None) | Err(StoreError::InvalidData(_) | StoreError::Serde(_)) => (
                    Vec::new(),
                    Some("Recovery values unavailable: retained planning is invalid; nothing changed.".into()),
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
                records,
                recovery_error,
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
        let retained = export_in(&tx, repo, destination)?;
        let (snapshot, _) = export_selected(&tx, repo, destination, retained)?;
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
        tx.execute("UPDATE planning_peer_context SET importing=1", [])?;
        retain_mutations(&tx, &saved, incoming)?;
        let owners = resolved_owners(&tx, repo, &merged)?;
        let mut objects = changes_by_object(&merged, &owners)?;
        let mut held = selection_conflicts(&tx, repo, destination, &merged)?;
        // One rejected or private origin holds the entire projection, not only
        // that origin's winner. Association never enrolls its local owner.
        for (origin, local) in &owners {
            if let Some(reason) = held.get(origin).cloned() {
                held.insert(local.clone(), reason);
            }
        }
        for object in objects.keys() {
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
        // Correspondence retains creation origins even when private selection
        // holds scalar projection. This releases no effect: the retained
        // sharing/projection conflict still fences every new attempt.
        // Import suppression keeps private local edits out of the peer journal.
        for (object, changes) in &objects {
            if !held.contains_key(object) || changes.creation.is_empty() {
                continue;
            }
            if !exists(&tx, object)? {
                continue;
            }
            let receipt = tx.savepoint()?;
            match project_creation(&receipt, object, changes) {
                Ok(()) => receipt.commit()?,
                // Contradictory captured operations remain in the journal and
                // behind the existing hold; never replace the retained receipt.
                Err(error) if projection_conflict(&error) => receipt.finish()?,
                Err(error) => return Err(error),
            }
        }
        let mut pending = objects
            .iter_mut()
            .filter(|(object, _)| !held.contains_key(*object))
            .map(|(object, changes)| {
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
        for (object, changes) in &objects {
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
                Ok(()) => observe_projected_fields(&tx, object, changes, &["current_project_id"])?,
                Err(error) if projection_conflict(&error) => {
                    conflicts.insert((object.clone(), error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        // Project lists can refer to Tasks first created by this import. Apply
        // only after their projection; no receipt is settled by this pass.
        for object in objects.keys().filter(|o| o.kind == PlanningKind::Project) {
            if !held.contains_key(object)
                && !conflicts.iter().any(|(failed, _)| failed == object)
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
            selection_conflicts(&tx, repo, destination, &merged)?
                .into_keys()
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

// Import and recovery rank the same explicitly associated owners. This map
// resolves projection identity only; it never changes an origin's membership.
fn resolved_owners(
    conn: &Connection,
    repo: &str,
    snapshot: &PlanningSnapshot,
) -> StoreResult<BTreeMap<PlanningObject, PlanningObject>> {
    snapshot
        .objects()
        .into_iter()
        .map(|origin| {
            let mut owner = origin.clone();
            if let Some(id) = associated_local_id(conn, origin.kind, &origin.id, Some(repo))? {
                owner.id = id;
            }
            Ok((origin.clone(), owner))
        })
        .collect()
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
    let objects = snapshot.objects();
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
    for &object in &objects {
        if member_destination(conn, repo, object)?.as_deref() != Some(destination) {
            held.insert(object.clone(), "planning exchange held: causal predecessor is outside this selection; private history is retained".into());
            pending.insert(object.clone());
        }
        if belongs_elsewhere(conn, object, repo)? {
            held.insert(object.clone(), "planning exchange held: record moved outside this repository; local work and peer history are retained".into());
            pending.insert(object.clone());
        }
        if let Some(local) = associated_local_id(conn, object.kind, &object.id, Some(repo))? {
            let local = PlanningObject {
                kind: object.kind,
                id: local,
            };
            dependents.entry(local.clone()).or_default().insert(object);
            if let Some(&local) = objects.get(&local) {
                dependents.entry(object.clone()).or_default().insert(local);
            }
            if member_destination(conn, repo, &local)?.as_deref() != Some(destination) {
                held.insert(object.clone(), "planning exchange held: associated local Work is outside this selection; private history is retained".into());
                pending.insert(object.clone());
            }
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
    mut snapshot: PlanningSnapshot,
) -> StoreResult<(PlanningSnapshot, BTreeMap<PlanningObject, String>)> {
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
        return observe_projected_fields(conn, object, changes, object.kind.fields());
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
                    params![object.id,projected_value(conn, "project_id", &winners["project_id"].1.value)?.as_str(),required("issue_identifier")?])?;
            }
            PlanningKind::Comment => unreachable!("comments use the common thread writer"),
        }
    }
    if object.kind == PlanningKind::Wave {
        validate_wave(conn, object, winners, repo)?;
    }
    project_creation(conn, object, changes)?;
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
                history.values(),
                history.heads(),
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
        let project = projected_value(conn, "project_id", &winners["project_id"].1.value)?;
        let project = project.as_str();
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
    // Projection may add receipts captured on a peer. Apply readback to those
    // newly inserted rows too; neither pass acknowledges a later local save.
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
    observe_projected_fields(conn, object, changes, &projected)
}

/// Retain the evaluated frontier without synthesizing a local edit. Keeping all
/// accepted heads (including losers) lets the next save follow that decision.
/// Rejected and held objects never enter here; their prior observation survives.
fn observe_projected_fields(
    conn: &Connection,
    object: &PlanningObject,
    changes: &ObjectChanges<'_>,
    fields: &[&str],
) -> StoreResult<()> {
    let projected = fields;
    let fields = serde_json::to_string(fields)?;
    conn.execute(
        "DELETE FROM planning_peer_observed WHERE object_id=?1 AND EXISTS (
            SELECT 1 FROM planning_peer_changes c WHERE c.id=planning_peer_observed.id
                AND c.kind=?2 AND c.field IN (SELECT value FROM json_each(?3)))",
        params![object.id, object.kind.as_str(), fields],
    )?;
    for (id, change) in &changes.observed {
        if projected.contains(&change.field.as_str()) {
            conn.execute(
                "INSERT INTO planning_peer_observed(object_id,id) VALUES(?1,?2)",
                params![object.id, id],
            )?;
        }
    }
    Ok(())
}

fn project_creation(
    conn: &Connection,
    local: &PlanningObject,
    changes: &ObjectChanges<'_>,
) -> StoreResult<()> {
    let task = TaskId::from_raw(&local.id);
    let project = ProjectId::from_raw(&local.id);
    for (origin, (winner, history)) in &changes.creation {
        super::planning_export::import_peer_receipts(
            conn,
            origin,
            match local.kind {
                PlanningKind::Task => PlanningChanges::Task(&task),
                PlanningKind::Project => PlanningChanges::Project(&project),
                _ => return Err(invalid("creation requires a Task or Project")),
            },
            winner,
            history,
        )?;
    }
    Ok(())
}

fn project_comment(
    conn: &Connection,
    object: &PlanningObject,
    changes: &ObjectChanges<'_>,
) -> StoreResult<()> {
    let winners = &changes.winners;
    let task = TaskId::from_raw(
        projected_value(conn, "task_id", &winners["task_id"].1.value)?
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
        } else if previous.get(&change.field)
            != Some(&projected_value(conn, &change.field, &change.value)?)
        {
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
    let (_, prior) = snapshot
        .ancestors(change.parents.iter().map(String::as_str))
        .filter(|(_, prior)| prior.linear.is_some())
        .max_by_key(|(id, prior)| {
            (
                prior
                    .linear
                    .as_ref()
                    .and_then(LinearObservation::revision_time),
                prior.clock,
                *id,
            )
        })?;
    Some(
        serde_json::json!({"value":prior.value,"revision":prior.linear.as_ref().and_then(LinearObservation::revision)}),
    )
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

/// Resolve a planning reference only at the local projection boundary. Journal
/// values and captured delivery inputs keep their original identities.
pub(super) fn projected_value(conn: &Connection, field: &str, value: &Value) -> StoreResult<Value> {
    let kind = match field {
        "project_id" | "current_project_id" => PlanningKind::Project,
        "task_id" => PlanningKind::Task,
        _ => return Ok(value.clone()),
    };
    let Some(origin) = value.as_str() else {
        return Ok(value.clone());
    };
    Ok(associated_local_id(conn, kind, origin, None)?
        .map(Value::String)
        .unwrap_or_else(|| value.clone()))
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
                    .map(|(key, value)| (key.as_str(), value.clone())),
            );
        } else {
            columns.insert(field, projected_value(conn, field, value)?);
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
mod tests;
