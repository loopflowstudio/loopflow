//! Field receipts share one protocol; each object's table retains its foreign key.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use super::SqliteStore;
use crate::durable::{ProjectId, TaskId};
use crate::planning::PlanningChange;
use crate::store::{StoreError, StoreResult};

impl SqliteStore {
    pub(crate) fn planning_field_owners(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<crate::durable::WorkRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT 'task',t.id FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
             WHERE w.repo=?1 AND t.external_issue_id IS NOT NULL AND EXISTS(
                 SELECT 1 FROM task_changes c WHERE c.task_id=t.id
                 AND ((t.planning_deleted_at IS NULL AND c.field!='deleted')
                     OR (t.planning_deleted_at IS NOT NULL AND c.field='deleted'))
                 AND c.acknowledged=0 AND c.conflict_json IS NULL
                 AND (c.field='deleted' OR c.attempted=1 OR c.seq=(SELECT max(seq) FROM task_changes WHERE task_id=t.id AND field=c.field)))
             UNION ALL
             SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id
             WHERE w.repo=?1 AND p.external_project_id IS NOT NULL AND EXISTS(
                 SELECT 1 FROM project_changes c WHERE c.project_id=p.id AND c.acknowledged=0 AND c.conflict_json IS NULL
                 AND (c.id IN (SELECT id FROM planning_order_deliveries) OR (c.field!='task_order' AND (c.attempted=1 OR c.seq=(SELECT max(seq) FROM project_changes WHERE project_id=p.id AND field=c.field)))))"
        )?;
        let rows = query.query_map([repo], |row| {
            let kind: String = row.get(0)?;
            let id: String = row.get(1)?;
            Ok(if kind == "task" {
                crate::durable::WorkRef::Task(TaskId::from_raw(id))
            } else {
                crate::durable::WorkRef::Project(ProjectId::from_raw(id))
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn attempt_planning_field(
        &self,
        owner: PlanningChanges<'_>,
        change: &PlanningChange,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        super::planning_peers::require_projected_effects(&tx, owner)?;
        let observation = owner.observation(&tx)?;
        if observation
            .as_ref()
            .and_then(|body| body["revision"].as_str())
            != revision
        {
            return Ok(false);
        }
        if let PlanningChanges::Task(id) = owner {
            let deleted: bool = tx.query_row(
                "SELECT planning_deleted_at IS NOT NULL FROM tasks WHERE id=?1",
                [id.as_str()],
                |row| row.get(0),
            )?;
            if deleted != (change.field == "deleted") {
                return Ok(false);
            }
        }
        let (owner, id) = owner.owner();
        let changed = tx.execute(
            &format!(
                "UPDATE {owner}_changes SET attempted=1,error=NULL WHERE id=?1 AND {owner}_id=?2 AND field=?3
             AND attempted=0 AND acknowledged=0 AND conflict_json IS NULL
             AND (field='deleted' OR seq=(SELECT max(seq) FROM {owner}_changes WHERE {owner}_id=?2 AND field=?3))
             AND NOT EXISTS(SELECT 1 FROM {owner}_changes WHERE {owner}_id=?2 AND field=?3
                 AND attempted=1 AND acknowledged=0 AND conflict_json IS NULL)"
            ),
            params![change.id, id, change.field],
        )? == 1;
        tx.commit()?;
        Ok(changed)
    }

    pub(crate) fn observe_task_deletion(&self, id: &TaskId, revision: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let owner = PlanningChanges::Task(id);
        let retained = owner.observation(&tx)?;
        if retained.as_ref().and_then(|body| body["revision"].as_str()) != Some(revision) {
            return Ok(());
        }
        for change in owner
            .pending(&tx)?
            .into_iter()
            .filter(|c| c.field == "deleted")
        {
            let baseline = change
                .base
                .as_ref()
                .and_then(|base| base["revision"].as_str());
            // Deletion competes with the whole issue. Only an explicitly active
            // issue at a newer revision can restore planning visibility.
            if baseline.is_some()
                && super::planning::revision_nanos(Some(revision))?
                    > super::planning::revision_nanos(baseline)?
            {
                tx.execute(
                    "UPDATE task_changes SET conflict_json=?2,error=NULL WHERE id=?1 AND conflict_json IS NULL",
                    params![change.id, serde_json::json!({"revision":revision,"value":false}).to_string()],
                )?;
            }
        }
        reconcile_deletion(&tx, id)?;
        tx.commit()?;
        Ok(())
    }

    /// Only a positive trash observation or the exact mutation acknowledgement settles removal.
    pub(crate) fn acknowledge_task_deletion(
        &self,
        id: &TaskId,
        change: &PlanningChange,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(revision) = revision {
            let observed = PlanningChanges::Task(id).observation(&tx)?;
            let retained = observed.as_ref().and_then(|body| body["revision"].as_str());
            if super::planning::revision_nanos(Some(revision))?
                < super::planning::revision_nanos(retained)?
            {
                return Ok(false);
            }
        }
        let changed = tx.execute(
            "UPDATE task_changes SET acknowledged=1,acknowledged_revision=?3,error=NULL
             WHERE id=?1 AND task_id=?2 AND field='deleted'
             AND acknowledged=0 AND conflict_json IS NULL",
            params![change.id, id.as_str(), revision],
        )? == 1;
        tx.commit()?;
        Ok(changed)
    }

    pub(crate) fn planning_field_error(
        &self,
        owner: PlanningChanges<'_>,
        change: &PlanningChange,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (owner, id) = owner.owner();
        conn.execute(
            &format!(
                "UPDATE {owner}_changes SET error=?3 WHERE id=?1 AND {owner}_id=?2
            AND acknowledged=0 AND conflict_json IS NULL AND error IS NOT ?3"
            ),
            params![change.id, id, error],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PlanningChanges<'a> {
    Task(&'a TaskId),
    Project(&'a ProjectId),
}

impl<'a> PlanningChanges<'a> {
    // A matching attempted write can arrive through either acquisition or readback.
    // Rebase later saves before conflict reconciliation; never acknowledge them.
    fn observe_attempts(self, conn: &Connection, observed: &Value) -> StoreResult<()> {
        let (owner, id) = self.owner();
        let mut query = conn.prepare(&format!(
            "SELECT id,field,value_json,base_json FROM {owner}_changes
             WHERE {owner}_id=?1 AND attempted=1 AND acknowledged=0 AND conflict_json IS NULL ORDER BY seq"
        ))?;
        let changes = query
            .query_and_then([id], read_change)?
            .collect::<StoreResult<Vec<_>>>()?;
        for PlanningChange {
            id: receipt,
            field,
            value,
            base,
        } in changes
        {
            let Some(remote) = observed.get(&field) else {
                continue;
            };
            let remote = self.normalize(conn, &field, remote.clone())?;
            if super::planning::revision_nanos(observed["revision"].as_str())?
                < super::planning::revision_nanos(
                    base.as_ref().and_then(|b| b["revision"].as_str()),
                )?
            {
                continue;
            }
            if remote == value {
                let baseline =
                    serde_json::json!({"revision": observed["revision"], "value": remote});
                conn.execute(
                    &format!(
                        "UPDATE {owner}_changes SET base_json=?2 WHERE {owner}_id=?3 AND field=?4
                     AND seq>(SELECT seq FROM {owner}_changes WHERE id=?1)
                     AND attempted=0 AND acknowledged=0 AND conflict_json IS NULL
                     AND (base_json IS ?5 OR (base_json IS NOT NULL AND ?5 IS NOT NULL
                         AND json_extract(base_json,'$.value') IS json_extract(?5,'$.value')))"
                    ),
                    params![
                        receipt,
                        baseline.to_string(),
                        id,
                        field,
                        base.as_ref().map(Value::to_string)
                    ],
                )?;
                conn.execute(
                    &format!("UPDATE {owner}_changes SET acknowledged=1,acknowledged_revision=?2,error=NULL WHERE id=?1"),
                    params![receipt, observed["revision"].as_str()],
                )?;
            } else if base.as_ref().is_none_or(|base| base["value"] != remote) {
                conn.execute(
                    &format!("UPDATE {owner}_changes SET conflict_json=?2,error=NULL WHERE id=?1"),
                    params![
                        receipt,
                        serde_json::json!({"revision":observed["revision"],"value":remote})
                            .to_string()
                    ],
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn owner(self) -> (&'static str, &'a str) {
        match self {
            Self::Task(id) => ("task", id.as_str()),
            Self::Project(id) => ("project", id.as_str()),
        }
    }

    fn observation(self, conn: &Connection) -> StoreResult<Option<Value>> {
        let (_, id) = self.owner();
        let observation = match self {
            Self::Task(_) => "SELECT i.body FROM tasks t JOIN projects p ON p.id=t.project_id
                JOIN waves w ON w.id=p.wave_id
                JOIN pm_items i ON i.id=t.external_issue_id AND i.repo=w.repo AND i.provider='linear'
                WHERE t.id=?1",
            Self::Project(_) => "SELECT o.body FROM projects p JOIN waves w ON w.id=p.wave_id
                JOIN pm_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.provider='linear'
                WHERE p.id=?1",
        };
        let body: Option<String> = conn
            .query_row(observation, [id], |row| row.get(0))
            .optional()?;
        let mut body: Option<Value> = body.map(|body| serde_json::from_str(&body)).transpose()?;
        if let (Self::Project(id), Some(body)) = (self, body.as_mut()) {
            body["task_order"] =
                serde_json::to_value(super::planning_order::observed_order(conn, id)?)?;
        }
        Ok(body)
    }

    pub(super) fn record(
        self,
        conn: &Connection,
        field: &str,
        previous: Value,
        value: Value,
    ) -> StoreResult<bool> {
        let previous = self.normalize(conn, field, previous)?;
        let value = self.normalize(conn, field, value)?;
        if previous == value {
            return Ok(false);
        }
        self.record_value(conn, field, &uuid::Uuid::new_v4().to_string(), value, None)?;
        Ok(true)
    }

    /// One receipt writer for ordinary edits and imported intentions. A caller
    /// with a retained mutation identity can repeat the save without a new effect.
    pub(super) fn record_value(
        self,
        conn: &Connection,
        field: &str,
        receipt: &str,
        value: Value,
        baseline: Option<Value>,
    ) -> StoreResult<()> {
        let value = self.normalize(conn, field, value)?;
        let (owner, id) = self.owner();
        // Preserve the causal predecessor's provider revision, not this
        // machine's potentially newer observation of a different value.
        let base = match baseline {
            Some(base) => Some((base["revision"].clone(), base["value"].clone())),
            None => self
                .observation(conn)?
                .map(|body| (body["revision"].clone(), body[field].clone())),
        }
        .map(|(revision, value)| -> StoreResult<Value> {
            Ok(serde_json::json!({
                "revision":revision, "value":self.normalize(conn, field, value)?
            }))
        })
        .transpose()?;
        let (capture_column, capture_value) = if matches!(self, Self::Task(_)) {
            (",deletion_saved_at", ",CASE WHEN ?3='deleted' THEN (SELECT planning_deleted_at FROM tasks WHERE id=?2) END")
        } else {
            ("", "")
        };
        conn.execute(
            &format!(
                "INSERT INTO {owner}_changes(id,{owner}_id,field,value_json,base_json{capture_column})
                VALUES(?1,?2,?3,?4,?5{capture_value}) ON CONFLICT(id) DO NOTHING"
            ),
            params![
                receipt,
                id,
                field,
                value.to_string(),
                base.map(|v| v.to_string())
            ],
        )?;
        Ok(())
    }

    /// Importing a Linear winner retires local intentions, not their history or
    /// attempted flags. It is not an acknowledgement of our own provider write.
    pub(super) fn adopt_peer_linear(
        self,
        conn: &Connection,
        field: &str,
        mutation: &str,
        value: Value,
        revision: Option<&str>,
    ) -> StoreResult<()> {
        let value = self.normalize(conn, field, value)?;
        let (owner, id) = self.owner();
        conn.execute(
            &format!(
                "UPDATE {owner}_changes SET conflict_json=?3
                WHERE {owner}_id=?1 AND field=?2 AND acknowledged=0 AND conflict_json IS NULL"
            ),
            params![
                id,
                field,
                serde_json::json!({"value":value,"peer_change":mutation,"revision":revision})
                    .to_string()
            ],
        )?;
        Ok(())
    }

    // Membership receipts retain durable identity even when a provider mapping arrives later.
    pub(super) fn normalize(
        self,
        conn: &Connection,
        field: &str,
        value: Value,
    ) -> StoreResult<Value> {
        if matches!(self, Self::Task(_)) && field == "project_id" {
            if let Some(selector) = value.as_str() {
                let id: Option<String> = conn
                    .query_row(
                        "SELECT id FROM projects WHERE id=?1 OR external_project_id=?1",
                        [selector],
                        |row| row.get(0),
                    )
                    .optional()?;
                if let Some(id) = id {
                    return Ok(Value::String(id));
                }
                return super::planning_peers::local_reference(
                    conn,
                    crate::engine::planning_exchange::PlanningKind::Project,
                    &value,
                );
            }
        }
        Ok(value)
    }

    pub(super) fn pending(self, conn: &Connection) -> StoreResult<Vec<PlanningChange>> {
        let (owner, id) = self.owner();
        let mut query = conn.prepare(&format!(
            "SELECT id,field,value_json,base_json FROM {owner}_changes c
             WHERE {owner}_id=?1 AND acknowledged=0 AND conflict_json IS NULL AND (field='deleted' OR (field='task_order' AND id IN (SELECT id FROM planning_order_current)) OR (field!='task_order' AND seq=(SELECT max(seq) FROM {owner}_changes
                 WHERE {owner}_id=c.{owner}_id AND field=c.field))) ORDER BY seq"
        ))?;
        let changes = query.query_and_then([id], read_change)?;
        changes.collect()
    }

    /// Unchanged baselines preserve saves; observed conflicts retire their delivery.
    /// Both values remain in the receipt. Only attempted writes can be acknowledged.
    pub(super) fn reconcile<T: Serialize + DeserializeOwned>(
        self,
        conn: &Connection,
        observed: &T,
    ) -> StoreResult<T> {
        let (owner, _) = self.owner();
        let mut saved = serde_json::to_value(observed)?;
        self.observe_attempts(conn, &saved)?;
        for change in self.pending(conn)? {
            // Ordinary inventory does not establish whether an issue is trashed.
            let Some(value) = saved.get(&change.field) else {
                continue;
            };
            let remote = self.normalize(conn, &change.field, value.clone())?;
            if remote != change.value
                && change
                    .base
                    .as_ref()
                    .is_none_or(|base| base["value"] != remote)
            {
                conn.execute(
                    &format!("UPDATE {owner}_changes SET conflict_json=?2 WHERE id=?1 AND conflict_json IS NULL"),
                    params![change.id, serde_json::json!({"revision": saved["revision"], "value": remote}).to_string()],
                )?;
            } else {
                saved[&change.field] = change.value;
            }
        }
        Ok(serde_json::from_value(saved)?)
    }
}

fn read_change(row: &rusqlite::Row<'_>) -> StoreResult<PlanningChange> {
    let value: String = row.get(2)?;
    let base: Option<String> = row.get(3)?;
    Ok(PlanningChange {
        id: row.get(0)?,
        field: row.get(1)?,
        value: serde_json::from_str(&value)?,
        base: base.map(|value| serde_json::from_str(&value)).transpose()?,
    })
}

/// Transport the common deletion receipt, never a synthetic provider observation.
/// The receipt's field key carries its identity; local sequence numbers stay local.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletionReceipt {
    deleted_at: Option<i64>,
    base: Option<DeletionObservation>,
    attempted: bool,
    acknowledged: bool,
    acknowledged_revision: Option<String>,
    conflict: Option<DeletionObservation>,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletionObservation {
    revision: Option<String>,
    value: Option<bool>,
}

pub(crate) fn validate_peer_deletion(value: &Value) -> StoreResult<()> {
    let receipt: DeletionReceipt = serde_json::from_value(value.clone())?;
    // Roundtrip also requires every optional member: omitted evidence is not null.
    if serde_json::to_value(&receipt)? != *value
        || receipt.deleted_at.is_some_and(|at| at < 0)
        || receipt
            .base
            .as_ref()
            .is_some_and(|base| base.value == Some(true))
        || receipt
            .conflict
            .as_ref()
            .is_some_and(|conflict| conflict.value != Some(false) || conflict.revision.is_none())
        || receipt.acknowledged_revision.is_some() && !receipt.acknowledged
        || receipt.acknowledged && receipt.conflict.is_some()
    {
        return Err(crate::store::StoreError::InvalidData(
            "invalid planning deletion receipt".into(),
        ));
    }
    for revision in [
        receipt
            .base
            .as_ref()
            .and_then(|base| base.revision.as_deref()),
        receipt.acknowledged_revision.as_deref(),
        receipt
            .conflict
            .as_ref()
            .and_then(|conflict| conflict.revision.as_deref()),
    ] {
        super::planning::revision_nanos(revision)?;
    }
    Ok(())
}

pub(super) fn import_peer_deletion(
    conn: &Connection,
    task: &TaskId,
    id: &str,
    history: &[&Value],
) -> StoreResult<()> {
    let mut receipts = history
        .iter()
        .map(|value| serde_json::from_value::<DeletionReceipt>((*value).clone()));
    let local: Option<(String, String, String)> = conn.query_row(
        "SELECT task_id,field,json_object('deleted_at',deletion_saved_at,'base',json(base_json),'attempted',json(CASE WHEN attempted THEN 'true' ELSE 'false' END),
        'acknowledged',json(CASE WHEN acknowledged THEN 'true' ELSE 'false' END),'acknowledged_revision',acknowledged_revision,
        'conflict',json(conflict_json),'error',error) FROM task_changes WHERE id=?1",
        [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional()?;
    let mut merged = if let Some((owner, field, body)) = local {
        if owner != task.as_str() || field != "deleted" {
            return Err(StoreError::PlanningReceiptConflict { effect: "deletion" });
        }
        serde_json::from_str::<DeletionReceipt>(&body)?
    } else {
        receipts
            .next_back()
            .expect("a deletion field has at least one mutation")?
    };
    for receipt in receipts {
        let receipt = receipt?;
        let baseline_conflict = match (&merged.base, &receipt.base) {
            (Some(left), Some(right)) => left != right,
            (None, Some(_)) => merged.attempted,
            (Some(_), None) => receipt.attempted,
            (None, None) => false,
        };
        if baseline_conflict
            || merged
                .deleted_at
                .zip(receipt.deleted_at)
                .is_some_and(|(left, right)| left != right)
            || merged.acknowledged && receipt.conflict.is_some()
            || receipt.acknowledged && merged.conflict.is_some()
        {
            // Unordered active/trash evidence stays a retained projection conflict;
            // a clock is not authority to discard either provider result.
            return Err(StoreError::PlanningReceiptConflict { effect: "deletion" });
        }
        // Creation readback may fill an unattempted, baseline-free deletion.
        // A captured attempt's baseline cannot subsequently change.
        if merged.base.is_none() {
            merged.base = receipt.base;
        }
        merged.deleted_at = merged.deleted_at.or(receipt.deleted_at);
        merged.attempted |= receipt.attempted;
        merged.acknowledged |= receipt.acknowledged;
        if super::planning::revision_nanos(receipt.acknowledged_revision.as_deref())?
            > super::planning::revision_nanos(merged.acknowledged_revision.as_deref())?
        {
            merged.acknowledged_revision = receipt.acknowledged_revision;
        }
        if super::planning::revision_nanos(
            receipt
                .conflict
                .as_ref()
                .and_then(|c| c.revision.as_deref()),
        )? > super::planning::revision_nanos(
            merged.conflict.as_ref().and_then(|c| c.revision.as_deref()),
        )? {
            merged.conflict = receipt.conflict;
        }
        // Diagnostics do not order effects. Keep a deterministic one here; all
        // original messages and losing values remain in the immutable journal.
        merged.error = merged.error.max(receipt.error);
    }
    if merged.acknowledged || merged.conflict.is_some() {
        merged.error = None;
    }
    // Import the captured receipt directly. The scalar save path would sample
    // this machine's provider baseline and visibility only to overwrite them.
    conn.execute(
        "INSERT INTO task_changes(id,task_id,field,value_json,base_json,attempted,
            acknowledged,acknowledged_revision,conflict_json,error,deletion_saved_at)
         VALUES(?1,?2,'deleted','true',?3,?4,?5,?6,?7,?8,?9)
         ON CONFLICT(id) DO UPDATE SET base_json=excluded.base_json,
            attempted=excluded.attempted,acknowledged=excluded.acknowledged,
            acknowledged_revision=excluded.acknowledged_revision,
            conflict_json=excluded.conflict_json,error=excluded.error,
            deletion_saved_at=excluded.deletion_saved_at
         WHERE base_json IS NOT excluded.base_json OR attempted IS NOT excluded.attempted
            OR acknowledged IS NOT excluded.acknowledged
            OR acknowledged_revision IS NOT excluded.acknowledged_revision
            OR conflict_json IS NOT excluded.conflict_json OR error IS NOT excluded.error
            OR deletion_saved_at IS NOT excluded.deletion_saved_at",
        params![
            id,
            task.as_str(),
            merged
                .base
                .map(|base| serde_json::to_string(&base))
                .transpose()?,
            merged.attempted,
            merged.acknowledged,
            merged.acknowledged_revision,
            merged
                .conflict
                .map(|conflict| serde_json::to_string(&conflict))
                .transpose()?,
            merged.error,
            merged.deleted_at
        ],
    )?;
    Ok(())
}

/// An explicitly active Linear response can defeat a concurrent tombstone.
/// Ordinary detail/list acquisition never creates this evidence. Keep unordered
/// acknowledgements conservative; neither absence nor a peer clock proves revival.
pub(super) fn reconcile_deletion(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    let mut query = conn.prepare("SELECT base_json,acknowledged,acknowledged_revision,conflict_json,deletion_saved_at FROM task_changes WHERE task_id=?1 AND field='deleted'")?;
    let rows = query.query_map([task.as_str()], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, bool>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<i64>>(4)?,
        ))
    })?;
    let mut active = None;
    let mut baseline = None;
    let mut unordered_ack = false;
    let mut deleted_at = None;
    for row in rows {
        let (base, acknowledged, revision, conflict, saved_at) = row?;
        if conflict.is_none() {
            deleted_at = deleted_at.max(saved_at);
        }
        let base: Option<DeletionObservation> =
            base.map(|body| serde_json::from_str(&body)).transpose()?;
        baseline = baseline.max(super::planning::revision_nanos(
            base.as_ref().and_then(|b| b.revision.as_deref()),
        )?);
        if acknowledged {
            let revision = super::planning::revision_nanos(revision.as_deref())?;
            unordered_ack |= revision.is_none();
            baseline = baseline.max(revision);
        }
        let conflict: Option<DeletionObservation> = conflict
            .map(|body| serde_json::from_str(&body))
            .transpose()?;
        if let Some(conflict) = conflict {
            let revision = super::planning::revision_nanos(conflict.revision.as_deref())?;
            if revision > active.as_ref().map(|(revision, _)| *revision) {
                active = revision.map(|revision| (revision, conflict));
            }
        }
    }
    if let Some((_, conflict)) =
        active.filter(|(revision, _)| !unordered_ack && Some(*revision) > baseline)
    {
        conn.execute("UPDATE task_changes SET conflict_json=?2,error=NULL WHERE task_id=?1 AND field='deleted' AND acknowledged=0 AND conflict_json IS NULL",
            params![task.as_str(),serde_json::to_string(&conflict)?])?;
        conn.execute("UPDATE tasks SET planning_deleted_at=NULL,planning_revision=planning_revision+1 WHERE id=?1 AND planning_deleted_at IS NOT NULL", [task.as_str()])?;
    } else if let Some(deleted_at) = deleted_at {
        // A later removal can share a millisecond with an earlier restoration.
        // Its own receipt, not the unrelated scalar's clock, supplies visibility.
        conn.execute("UPDATE tasks SET planning_deleted_at=?2,planning_revision=planning_revision+1 WHERE id=?1 AND planning_deleted_at IS NOT ?2", params![task.as_str(),deleted_at])?;
    }
    Ok(())
}
