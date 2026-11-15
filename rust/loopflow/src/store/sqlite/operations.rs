//! Receipts outlive their resources: deleting work never makes an identity reusable.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::SqliteStore;
use crate::store::planning_sync::{PlanningChangeKind, PlanningChangeRef};
use crate::store::{StoreError, StoreResult};

/// Captured with domain facts or a current status reading in one snapshot.
/// A status reading dates its evidence; only a domain writer establishes a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CommitFence {
    pub machine: String,
    pub epoch: String,
    pub schema: u32,
    pub revisions: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationCommit {
    pub resolution: OperationResolution,
    pub stage: String,
    pub resources: Vec<String>,
    pub fence: CommitFence,
    pub result: serde_json::Value,
    pub planning_effect: PlanningEffect,
    pub planning_changes: Vec<PlanningChangeRef>,
}

/// The domain writer declares whether this stage has an external planning effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanningEffect {
    None,
    Correlated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationPlanningReading {
    pub fence: CommitFence,
    pub changes: Vec<OperationPlanningChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationPlanningChange {
    pub change: PlanningChangeRef,
    pub state: PlanningDeliveryState,
    #[serde(deserialize_with = "Option::deserialize")]
    pub error: Option<String>,
}

/// Current evidence for the exact saved change. Unavailable never means delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanningDeliveryState {
    Pending,
    Uncertain,
    Delivered,
    Superseded,
    Conflict,
    Unavailable,
}

/// Resolution of this semantic stage, independent of delivery and other stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OperationResolution {
    Committed,
    DefinitelyRefused,
    Superseded,
}

/// Dispatch state is not a prediction about a provider, Graph or native surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationDelivery {
    Untracked,
    Queued,
    Dispatched,
    Finished,
    Uncertain,
}

/// A pending legacy candidate is reserved, but has no permission to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationValidation {
    Unbound,
    Pending,
    Candidate,
    Bound,
    Conflict,
}

#[derive(Debug)]
pub(crate) struct SubmittedCommand {
    pub command: serde_json::Value,
    pub generation: i64,
    pub validation: OperationValidation,
    pub delivery: OperationDelivery,
}

/// Creation recovery and general commands read the same row. Command output
/// stays separate from the domain results that committed with displayed facts.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationReceipt {
    pub request_id: String,
    pub kind: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub fingerprint: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub submitted_fingerprint: Option<String>,
    pub validation: OperationValidation,
    pub delivery: OperationDelivery,
    /// Orders receipt and correlated planning updates within one database epoch.
    pub revision: i64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub result: Option<serde_json::Value>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub error: Option<String>,
    pub commits: Vec<OperationCommit>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub planning: Option<OperationPlanningReading>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Record one semantic stage, before the caller commits. A savepoint is not
/// sufficient: allowing an autocommit caller would split facts from the fence.
pub(crate) fn commit_in(
    conn: &Connection,
    id: &str,
    stage: &str,
    resources: &[&str],
    domains: &[&str],
    result: &serde_json::Value,
) -> StoreResult<()> {
    let span = tracing::info_span!("operation_domain_commit", operation_id = id, stage);
    let _entered = span.enter();
    let commit = domain_commit_in(conn, stage, resources, domains, result)?;
    record_resolution_in(conn, id, &commit)
}

fn record_resolution_in(conn: &Connection, id: &str, commit: &OperationCommit) -> StoreResult<()> {
    if conn.execute(
        "UPDATE creation_receipts SET commits=json_insert(commits,'$[#]',json(?2)),updated_at=?3 WHERE request_id=?1 AND delivery_state<>'finished'",
        params![id,serde_json::to_string(commit)?,crate::store::rows::now_unix()],
    )? != 1 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

fn domain_commit_in(
    conn: &Connection,
    stage: &str,
    resources: &[&str],
    domains: &[&str],
    result: &serde_json::Value,
) -> StoreResult<OperationCommit> {
    if conn.is_autocommit() {
        return Err(StoreError::InvalidData(
            "operation commits require the domain transaction".into(),
        ));
    }
    Ok(OperationCommit {
        resolution: OperationResolution::Committed,
        stage: stage.into(),
        resources: resources.iter().map(|id| (*id).to_owned()).collect(),
        fence: fence_in(conn, domains)?,
        result: result.clone(),
        planning_effect: PlanningEffect::None,
        planning_changes: Vec::new(),
    })
}

/// The caller pins a snapshot; a reading's provenance is not an external acknowledgment.
fn fence_in(conn: &Connection, domains: &[&str]) -> StoreResult<CommitFence> {
    let (machine, epoch) = conn.query_row(
        "SELECT machines.id,client_store_epoch.epoch FROM machines CROSS JOIN client_store_epoch
         WHERE machines.is_local=1 AND singleton=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut revisions = BTreeMap::new();
    for domain in domains {
        if !matches!(
            *domain,
            "planning" | "sessions" | "graphs" | "processes" | "usage"
        ) {
            return Err(StoreError::InvalidData(format!(
                "unknown commit domain {domain}"
            )));
        }
        let revision = conn.query_row(
            "SELECT revision FROM store_revisions WHERE domain=?1",
            [domain],
            |row| row.get(0),
        )?;
        revisions.insert((*domain).to_owned(), revision);
    }
    Ok(CommitFence {
        machine,
        epoch,
        schema: crate::server::wire::REVISION,
        revisions,
    })
}

pub(super) fn commit_planning_edit_in(
    conn: &Connection,
    operation: &str,
    stage: &str,
    task: &str,
    changes: Vec<PlanningChangeRef>,
    result: &serde_json::Value,
) -> StoreResult<()> {
    let mut commit = domain_commit_in(conn, stage, &[task], &["planning"], result)?;
    commit.planning_effect = if changes.is_empty() {
        PlanningEffect::None
    } else {
        PlanningEffect::Correlated
    };
    commit.planning_changes = changes;
    record_resolution_in(conn, operation, &commit)
}

pub(super) fn commit_current_in(
    conn: &Connection,
    stage: &str,
    resources: &[&str],
    domains: &[&str],
    changes: Vec<PlanningChangeRef>,
    result: impl FnOnce() -> StoreResult<serde_json::Value>,
) -> StoreResult<()> {
    if let Some(id) = crate::journal::request::operation_id() {
        let mut commit = domain_commit_in(conn, stage, resources, domains, &result()?)?;
        commit.planning_effect = if changes.is_empty() {
            PlanningEffect::None
        } else {
            PlanningEffect::Correlated
        };
        commit.planning_changes = changes;
        record_resolution_in(conn, &id, &commit)?;
    }
    Ok(())
}

/// The caller has rejected this effect before changing any domain fact. Keep
/// the refusal in that transaction; a generic command error proves nothing.
pub(super) fn refuse_current_in(
    conn: &Connection,
    stage: &str,
    resources: &[&str],
    domains: &[&str],
    error: &StoreError,
) -> StoreResult<()> {
    if let Some(id) = crate::journal::request::operation_id() {
        let mut resolution = domain_commit_in(
            conn,
            stage,
            resources,
            domains,
            &serde_json::json!({"error": error.to_string()}),
        )?;
        resolution.resolution = OperationResolution::DefinitelyRefused;
        record_resolution_in(conn, &id, &resolution)?;
    }
    Ok(())
}

/// Settlement may run on a later periodic check with no request worker. Keep
/// its subscribers with the durable domain request, in the same receipt owner.
pub(super) fn completion_requested_in(
    conn: &Connection,
    task: &str,
    request: i64,
) -> StoreResult<()> {
    if let Some(id) = crate::journal::request::operation_id() {
        if conn.execute(
            "UPDATE creation_receipts SET task_id=?2,completion_request=?3
             WHERE request_id=?1 AND kind='task.complete' AND completion_request IS NULL",
            params![id, task, request],
        )? != 1
        {
            return Err(StoreError::Stale(
                "completion already has a domain request".into(),
            ));
        }
        commit_in(
            conn,
            &id,
            "task.completion_requested",
            &[task],
            &["planning"],
            &serde_json::json!({"request":request}),
        )?;
    }
    Ok(())
}

pub(super) fn completion_committed_in(
    conn: &Connection,
    task: &str,
    request: i64,
    changes: Vec<PlanningChangeRef>,
    result: &serde_json::Value,
) -> StoreResult<()> {
    completion_resolved_in(
        conn,
        task,
        request,
        OperationResolution::Committed,
        changes,
        result,
    )
}

/// Every writer withdrawing a completion records the replacement facts after
/// its changes, inside the same transaction. Historical absence proves nothing.
pub(crate) fn completion_superseded_in(
    conn: &Connection,
    task: &str,
    request: i64,
) -> StoreResult<()> {
    completion_resolved_in(
        conn,
        task,
        request,
        OperationResolution::Superseded,
        Vec::new(),
        &serde_json::json!({"request": request}),
    )
}

fn completion_resolved_in(
    conn: &Connection,
    task: &str,
    request: i64,
    resolution: OperationResolution,
    changes: Vec<PlanningChangeRef>,
    result: &serde_json::Value,
) -> StoreResult<()> {
    let waiting: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM creation_receipts
         WHERE kind='task.complete' AND task_id=?1 AND completion_request=?2
         AND NOT EXISTS(SELECT 1 FROM json_each(commits) WHERE json_extract(value,'$.stage')='task.completed'))",
        params![task,request], |row| row.get(0),
    )?;
    if !waiting {
        return Ok(());
    }
    let mut commit = domain_commit_in(conn, "task.completed", &[task], &["planning"], result)?;
    commit.resolution = resolution;
    commit.planning_effect = if changes.is_empty() {
        PlanningEffect::None
    } else {
        PlanningEffect::Correlated
    };
    commit.planning_changes = changes;
    // Delivery may already have finished while settlement was pending. It does
    // not discharge this domain obligation. The first resolution always wins.
    conn.execute(
        "UPDATE creation_receipts SET commits=json_insert(commits,'$[#]',json(?3)),updated_at=?4
         WHERE kind='task.complete' AND task_id=?1 AND completion_request=?2
         AND NOT EXISTS(SELECT 1 FROM json_each(commits) WHERE json_extract(value,'$.stage')='task.completed')",
        params![task,request,serde_json::to_string(&commit)?,crate::store::rows::now_unix()],
    )?;
    Ok(())
}

impl SqliteStore {
    /// Admission is durable before dispatch. A duplicate never resets delivery
    /// or replaces the immutable command, including after a server restart.
    pub(crate) fn admit_operation(
        &self,
        epoch: &str,
        id: &str,
        kind: &str,
        submitted_fingerprint: &str,
        command: &serde_json::Value,
    ) -> StoreResult<OperationReceipt> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let encoded = serde_json::to_string(&serde_json::json!({"version":1,"command":command}))?;
        let known = |conn: &Connection| -> StoreResult<Option<(String, Option<String>)>> {
            Ok(conn
                .query_row(
                    "SELECT kind,command FROM creation_receipts WHERE request_id=?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?)
        };
        let duplicate = |conn: &Connection| -> StoreResult<bool> {
            let Some((known_kind, known_command)) = known(conn)? else {
                return Ok(false);
            };
            if known_kind != kind
                || known_command
                    .as_ref()
                    .is_some_and(|value| value != &encoded)
            {
                return Err(StoreError::Stale(format!(
                    "operation {id} has different content"
                )));
            }
            Ok(known_command.is_some())
        };
        let check_epoch = |conn: &Connection| -> StoreResult<()> {
            let current: String = conn.query_row(
                "SELECT epoch FROM client_store_epoch WHERE singleton=1",
                [],
                |row| row.get(0),
            )?;
            if current != epoch {
                return Err(StoreError::Stale(
                    "the Machine database changed before operation admission".into(),
                ));
            }
            Ok(())
        };
        {
            let snapshot = conn.savepoint()?;
            check_epoch(&snapshot)?;
            if duplicate(&snapshot)? {
                let receipt = operation_in(&snapshot, id)?.ok_or(StoreError::NotFound)?;
                snapshot.commit()?;
                return Ok(receipt);
            }
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        check_epoch(&tx)?;
        if !duplicate(&tx)? {
            let now = crate::store::rows::now_unix();
            let validation = if matches!(kind, "task.create" | "session.new") {
                "pending"
            } else {
                "bound"
            };
            tx.execute("INSERT INTO creation_receipts(request_id,kind,run,milestone,created_at,updated_at,command,submitted_fingerprint,validation,candidate_generation,delivery_state)
                VALUES(?1,?2,0,'queued',?3,?3,?4,?5,?6,1,'queued')
                ON CONFLICT(request_id) DO UPDATE SET command=excluded.command,submitted_fingerprint=excluded.submitted_fingerprint,
                    validation=CASE WHEN excluded.validation='pending' THEN 'candidate' ELSE excluded.validation END,candidate_generation=creation_receipts.candidate_generation+1,command_error=NULL,updated_at=excluded.updated_at",
                params![id,kind,now,encoded,submitted_fingerprint,validation])?;
        }
        let receipt = operation_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(receipt)
    }

    pub(crate) fn submitted_command(&self, id: &str) -> StoreResult<Option<SubmittedCommand>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // The command, reservation and status are one SQL snapshot. A separate
        // status read could pair a replaced candidate with its successor's claim.
        let row: Option<(String, i64, String, String)> = conn.query_row(
            "SELECT command,candidate_generation,validation,delivery_state FROM creation_receipts WHERE request_id=?1 AND command IS NOT NULL AND validation<>'conflict'",
            [id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional()?;
        row.map(|(encoded, generation, validation, delivery)| {
            let envelope: serde_json::Value = serde_json::from_str(&encoded)?;
            if envelope["version"] != 1 {
                return Err(StoreError::InvalidData(
                    "unsupported submitted command version".into(),
                ));
            }
            Ok(SubmittedCommand {
                command: envelope
                    .get("command")
                    .ok_or_else(|| StoreError::InvalidData("submitted command is missing".into()))?
                    .clone(),
                generation,
                validation: serde_json::from_value(serde_json::Value::String(validation))?,
                delivery: serde_json::from_value(serde_json::Value::String(delivery))?,
            })
        })
        .transpose()
    }

    /// Resolve canonical identity without changing a saved domain outcome. The
    /// reservation generation prevents a held validator from binding a successor.
    pub(crate) fn validate_operation(
        &self,
        id: &str,
        generation: i64,
        identity: &CreationIdentity,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let row: Option<(String, Option<String>, String)> = tx.query_row(
            "SELECT kind,fingerprint,validation FROM creation_receipts WHERE request_id=?1 AND candidate_generation=?2",
            params![id,generation], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).optional()?;
        let Some((kind, canonical, validation)) = row else {
            return Ok(false);
        };
        if validation == "conflict" {
            return Ok(false);
        }
        if validation == "bound" {
            // The first verified envelope won. Another validator of that same
            // reservation cannot revoke it if repository discovery changed later.
            return Ok(true);
        }
        if kind != identity.kind
            || canonical
                .as_ref()
                .is_some_and(|value| value != &identity.fingerprint)
        {
            tx.execute("UPDATE creation_receipts SET command=CASE WHEN validation='candidate' THEN NULL ELSE command END,validation='conflict',command_error='submitted content conflicts with the saved creation identity' WHERE request_id=?1 AND candidate_generation=?2",
                params![id,generation])?;
            tx.commit()?;
            return Ok(false);
        }
        tx.execute("UPDATE creation_receipts SET fingerprint=?3,validation='bound',command_error=NULL WHERE request_id=?1 AND candidate_generation=?2",
            params![id,generation,identity.fingerprint])?;
        tx.commit()?;
        Ok(true)
    }

    pub(crate) fn record_validation_error(
        &self,
        id: &str,
        generation: i64,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("UPDATE creation_receipts SET command_error=?3 WHERE request_id=?1 AND candidate_generation=?2 AND validation IN ('pending','candidate') AND command_error IS NOT ?3",
            params![id,generation,error])?;
        Ok(())
    }

    /// Only a definitely unsent command can be claimed. Dispatched/uncertain
    /// commands require their domain recovery owner, never generic replay.
    pub(crate) fn dispatch_operation(&self, id: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let queued: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM creation_receipts WHERE request_id=?1 AND validation='bound' AND (delivery_state='queued' OR (delivery_state='untracked' AND kind IN ('task.create','session.new'))))",
            [id], |row|row.get(0),
        )?;
        if !queued {
            return Ok(false);
        }
        Ok(conn.execute(
            "UPDATE creation_receipts SET delivery_state='dispatched',updated_at=?2
            WHERE request_id=?1 AND validation='bound' AND (delivery_state='queued' OR (delivery_state='untracked' AND kind IN ('task.create','session.new')))",
            params![id, crate::store::rows::now_unix()],
        )? == 1)
    }

    pub(crate) fn finish_operation(
        &self,
        id: &str,
        result: Result<&serde_json::Value, &str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (state, output, error) = match result {
            Ok(value) => ("finished", Some(serde_json::to_string(value)?), None),
            Err(error) => ("uncertain", None, Some(error)),
        };
        if conn.execute("UPDATE creation_receipts SET delivery_state=?2,result=CASE WHEN ?2='finished' THEN ?3 ELSE result END,command_error=?4,updated_at=?5
            WHERE request_id=?1 AND delivery_state IN ('dispatched','uncertain')", params![id,state,output,error,crate::store::rows::now_unix()])? != 1 {
            let finished: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM creation_receipts WHERE request_id=?1 AND delivery_state='finished')", [id], |row| row.get(0))?;
            if !finished { return Err(StoreError::Stale(format!("operation {id} is not dispatched"))); }
        }
        Ok(())
    }

    /// Recover the original domain result after output was lost. Retain any
    /// later failure: a Task result does not certify cleanup or planning delivery.
    pub(crate) fn recover_operation_result(&self, id: &str, stage: &str) -> StoreResult<bool> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let Some(receipt) = operation_in(&conn, id)? else {
            return Ok(false);
        };
        let Some(commit) = receipt.commits.iter().find(|commit| {
            commit.stage == stage && commit.resolution == OperationResolution::Committed
        }) else {
            return Ok(false);
        };
        let changed = conn.execute(
            "UPDATE creation_receipts SET delivery_state='finished',result=?2,updated_at=?3
             WHERE request_id=?1 AND delivery_state IN ('dispatched','uncertain')",
            params![
                id,
                serde_json::to_string(&commit.result)?,
                crate::store::rows::now_unix()
            ],
        )? == 1;
        conn.commit()?;
        Ok(changed)
    }

    pub fn operation_receipt(&self, id: &str) -> StoreResult<Option<OperationReceipt>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let receipt = operation_in(&snapshot, id)?;
        snapshot.commit()?;
        Ok(receipt)
    }
}

fn operation_in(conn: &Connection, id: &str) -> StoreResult<Option<OperationReceipt>> {
    let row = conn.query_row("SELECT request_id,kind,fingerprint,delivery_state,CASE WHEN delivery_state='finished' THEN result END,command_error,commits,created_at,updated_at,receipt_revision,submitted_fingerprint,validation
        FROM creation_receipts WHERE request_id=?1", [id], |row| Ok((
            row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,String>(3)?,
            row.get::<_,Option<String>>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,String>(6)?,
            row.get::<_,i64>(7)?,row.get::<_,i64>(8)?,row.get::<_,i64>(9)?,row.get::<_,Option<String>>(10)?,row.get::<_,String>(11)?
        ))).optional()?;
    row.map(
        |(
            request_id,
            kind,
            fingerprint,
            delivery,
            result,
            error,
            commits,
            created_at,
            updated_at,
            revision,
            submitted_fingerprint,
            validation,
        )| {
            let commits: Vec<OperationCommit> = serde_json::from_str(&commits)?;
            let planning = planning_reading_in(conn, &commits)?;
            let revision = revision
                + planning
                    .as_ref()
                    .map_or(0, |reading| reading.fence.revisions["planning"]);
            Ok(OperationReceipt {
                request_id,
                kind,
                fingerprint,
                submitted_fingerprint,
                validation: serde_json::from_value(serde_json::Value::String(validation))?,
                revision,
                delivery: serde_json::from_value(serde_json::Value::String(delivery))?,
                result: result.map(|text| serde_json::from_str(&text)).transpose()?,
                error,
                commits,
                planning,
                created_at,
                updated_at,
            })
        },
    )
    .transpose()
}

fn planning_reading_in(
    conn: &Connection,
    commits: &[OperationCommit],
) -> StoreResult<Option<OperationPlanningReading>> {
    let references: Vec<_> = commits
        .iter()
        .flat_map(|commit| &commit.planning_changes)
        .collect();
    if references.is_empty() {
        return Ok(None);
    }
    let fence = fence_in(conn, &["planning"])?;
    let mut changes = Vec::new();
    for change in references {
        if change.kind == PlanningChangeKind::TaskCreation {
            let row = conn.query_row(
                "SELECT t.external_issue_id,json_extract(c.export_json,'$.id'),c.export_attempted,c.export_error
                 FROM task_creation_intents c JOIN tasks t ON t.id=c.task_id
                 WHERE c.task_id=?1 AND c.task_id=?2",
                params![change.id, change.owner],
                |row| Ok((row.get::<_,Option<String>>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,bool>(2)?,row.get::<_,Option<String>>(3)?)),
            ).optional()?;
            let (state, error) = match row {
                None => (PlanningDeliveryState::Unavailable, None),
                Some((Some(mapped), Some(expected), true, _)) if mapped == expected => {
                    (PlanningDeliveryState::Delivered, None)
                }
                Some((Some(_), _, _, error)) => (PlanningDeliveryState::Unavailable, error),
                Some((None, _, true, error)) => (PlanningDeliveryState::Uncertain, error),
                Some((None, _, false, error)) => (PlanningDeliveryState::Pending, error),
            };
            changes.push(OperationPlanningChange {
                change: change.clone(),
                state,
                error,
            });
            continue;
        }
        let (table, owner, settled, field) = match change.kind {
            PlanningChangeKind::Task => (
                "task_changes",
                "task_id",
                "acknowledged",
                " AND field=c.field",
            ),
            PlanningChangeKind::Project => (
                "project_changes",
                "project_id",
                "acknowledged",
                " AND field=c.field",
            ),
            PlanningChangeKind::TaskState => ("task_state_deliveries", "task_id", "settled", ""),
            PlanningChangeKind::TaskCreation => unreachable!("creation handled above"),
        };
        let row = conn
            .query_row(
                &format!(
                    "SELECT {settled},conflict_json IS NOT NULL,attempted,error,
             seq<(SELECT max(seq) FROM {table} WHERE {owner}=c.{owner}{field})
             FROM {table} c WHERE id=?1 AND {owner}=?2"
                ),
                params![change.id, change.owner],
                |row| {
                    Ok((
                        row.get::<_, bool>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, bool>(4)?,
                    ))
                },
            )
            .optional()?;
        let (state, error) = match row {
            None => (PlanningDeliveryState::Unavailable, None),
            Some((_, true, _, _, _)) => (PlanningDeliveryState::Conflict, None),
            Some((true, _, _, _, _)) => (PlanningDeliveryState::Delivered, None),
            Some((_, _, true, error, _)) => (PlanningDeliveryState::Uncertain, error),
            Some((_, _, _, _, true)) => (PlanningDeliveryState::Superseded, None),
            Some((_, _, _, error, _)) => (PlanningDeliveryState::Pending, error),
        };
        changes.push(OperationPlanningChange {
            change: change.clone(),
            state,
            error,
        });
    }
    Ok(Some(OperationPlanningReading { fence, changes }))
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreationReceipt {
    pub disposition: RecoveryDisposition,
    pub explanation: String,
    pub run: bool,
    pub request_id: String,
    pub kind: String,
    pub milestone: String,
    pub task_id: Option<String>,
    pub session_id: Option<String>,
    pub artifact_key: Option<String>,
    pub result: Option<serde_json::Value>,
    pub launch: Option<serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecoveryDisposition {
    Complete,
    Continue,
    Inspect,
}

#[derive(Debug, Clone)]
pub struct CreationIdentity {
    pub request_id: String,
    pub kind: &'static str,
    pub fingerprint: String,
    pub run: bool,
}

pub(crate) fn insert_in(
    conn: &Connection,
    identity: &CreationIdentity,
    task_id: Option<&str>,
    session_id: Option<&str>,
    artifact_key: Option<&str>,
    milestone: &str,
    result: Option<&serde_json::Value>,
) -> StoreResult<()> {
    let now = crate::store::rows::now_unix();
    let changed = conn.execute("INSERT INTO creation_receipts
        (request_id,kind,fingerprint,task_id,session_id,artifact_key,milestone,result,created_at,updated_at,run)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?10)
        ON CONFLICT(request_id) DO UPDATE SET task_id=excluded.task_id,session_id=excluded.session_id,
        artifact_key=excluded.artifact_key,milestone=excluded.milestone,result=excluded.result,run=excluded.run,updated_at=excluded.updated_at
        WHERE creation_receipts.kind=excluded.kind AND creation_receipts.fingerprint=excluded.fingerprint AND creation_receipts.milestone='queued'", params![identity.request_id, identity.kind,
        identity.fingerprint, task_id, session_id, artifact_key, milestone,
        result.map(serde_json::to_string).transpose()?, now, identity.run])?;
    if changed != 1 {
        return Err(StoreError::Stale(format!(
            "creation {} already has a domain result",
            identity.request_id
        )));
    }

    Ok(())
}

/// Filing and its external export correlation share the promotion transaction.
pub(super) fn commit_filing_in(
    conn: &Connection,
    id: &str,
    task: &str,
    session: Option<&str>,
    result: &serde_json::Value,
    changes: Vec<PlanningChangeRef>,
) -> StoreResult<()> {
    let mut commit = domain_commit_in(
        conn,
        "task.filed",
        &std::iter::once(task).chain(session).collect::<Vec<_>>(),
        &["planning", "sessions"],
        result,
    )?;
    commit.planning_effect = if changes.is_empty() {
        PlanningEffect::None
    } else {
        PlanningEffect::Correlated
    };
    commit.planning_changes = changes;
    record_resolution_in(conn, id, &commit)
}

impl SqliteStore {
    pub(crate) fn begin_thread_creation(
        &self,
        identity: &CreationIdentity,
        session: &str,
        artifact: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        insert_in(
            &conn,
            identity,
            None,
            Some(session),
            Some(artifact),
            "stopping",
            None,
        )
    }

    pub fn creation_receipt(&self, id: &str) -> StoreResult<Option<CreationReceipt>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let row = conn.query_row("SELECT request_id,kind,milestone,task_id,session_id,artifact_key,result,launch,created_at,updated_at,run
            FROM creation_receipts WHERE request_id=?1 AND milestone<>'queued'", [id], |row| {
            Ok((CreationReceipt {
                disposition: RecoveryDisposition::Inspect, explanation: String::new(), run: row.get(10)?,
                request_id: row.get(0)?, kind: row.get(1)?, milestone: row.get(2)?,
                task_id: row.get(3)?, session_id: row.get(4)?, artifact_key: row.get(5)?,
                result: None, launch: None, created_at: row.get(8)?, updated_at: row.get(9)?,
            }, row.get::<_,Option<String>>(6)?, row.get::<_,Option<String>>(7)?))
        }).optional()?;
        row.map(|(mut receipt, result, launch)| {
            receipt.result = result.map(|text| serde_json::from_str(&text)).transpose()?;
            receipt.launch = launch.map(|text| serde_json::from_str(&text)).transpose()?;
            let (disposition, explanation) = recovery_in(&conn, &receipt)?;
            receipt.disposition = disposition;
            receipt.explanation = explanation.into();
            Ok(receipt)
        })
        .transpose()
    }

    pub(crate) fn claim_creation(&self, identity: &CreationIdentity) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let existing: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT kind,fingerprint FROM creation_receipts WHERE request_id=?1",
                [&identity.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((kind, Some(fingerprint))) = existing {
            if kind == identity.kind && fingerprint == identity.fingerprint {
                return Ok(());
            }
            return Err(StoreError::Stale(format!(
                "creation request {} has different content",
                identity.request_id
            )));
        }
        let now = crate::store::rows::now_unix();
        if conn.execute("INSERT INTO creation_receipts(request_id,kind,fingerprint,run,milestone,created_at,updated_at)
            VALUES(?1,?2,?3,?4,'queued',?5,?5)
            ON CONFLICT(request_id) DO UPDATE SET fingerprint=excluded.fingerprint
            WHERE creation_receipts.kind=excluded.kind AND creation_receipts.fingerprint IS NULL",
            params![identity.request_id,identity.kind,identity.fingerprint,identity.run,now])? == 1 {
            return Ok(());
        }
        let matches: bool = conn.query_row(
            "SELECT kind=?2 AND fingerprint=?3 FROM creation_receipts WHERE request_id=?1",
            params![identity.request_id, identity.kind, identity.fingerprint],
            |row| row.get(0),
        )?;
        if !matches {
            return Err(StoreError::Stale(format!(
                "creation request {} has different content",
                identity.request_id
            )));
        }
        Ok(())
    }

    pub(crate) fn advance_creation(
        &self,
        id: &str,
        milestone: &str,
        result: Option<&serde_json::Value>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute("UPDATE creation_receipts SET milestone=?2,result=COALESCE(?3,result),updated_at=?4 WHERE request_id=?1",
            params![id,milestone,result.map(serde_json::to_string).transpose()?,crate::store::rows::now_unix()])? != 1 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub(crate) fn record_creation_launch(
        &self,
        id: &str,
        result: &serde_json::Value,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("UPDATE creation_receipts SET milestone='started',launch=?2,updated_at=?3 WHERE request_id=?1 AND milestone='run_requested'",
            params![id,serde_json::to_string(result)?,crate::store::rows::now_unix()])?;
        Ok(())
    }
}

// Read the receipt and the current resource in the same snapshot. The handler
// reads this projection again under its request lock before continuing.
fn recovery_in(
    conn: &Connection,
    receipt: &CreationReceipt,
) -> StoreResult<(RecoveryDisposition, &'static str)> {
    use RecoveryDisposition::{Complete, Continue, Inspect};
    match receipt.kind.as_str() {
        "task.create" => {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND planning_deleted_at IS NULL)",
                [&receipt.task_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Ok((Inspect, "The created Task was removed"));
            }
            Ok(match receipt.milestone.as_str() {
                "filed" if receipt.run => (Continue, "Task created; start pending"),
                "filed" => (Complete, "Task created"),
                "run_requested" => (Inspect, "Task created; start unconfirmed"),
                "started"
                    if receipt
                        .result
                        .as_ref()
                        .and_then(|value| value["milestone"].as_str())
                        != Some("started") =>
                {
                    (Continue, "Task started; conversation pending")
                }
                "started" | "admitted" => (Complete, "Task created"),
                _ => (Inspect, "Task creation needs inspection"),
            })
        }
        "session.new" => {
            let session = receipt
                .session_id
                .as_deref()
                .map(|id| super::sessions::session_in(conn, id))
                .transpose()?
                .flatten();
            let Some(session) = session else {
                return Ok((Inspect, "The conversation was removed"));
            };
            if receipt.milestone == "stopping" {
                return Ok((Inspect, "Conversation change unconfirmed"));
            }
            if receipt.milestone != "admitted" {
                return Ok((Inspect, "Conversation change needs inspection"));
            }
            if receipt.result.is_some() {
                return Ok((Complete, "Conversation changed"));
            }
            if session.artifact_key != receipt.artifact_key {
                return Ok((Inspect, "Newer conversation input replaced this change"));
            }
            Ok((Continue, "Conversation change awaiting publication"))
        }
        _ => Ok((Inspect, "Creation needs inspection")),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use serde_json::json;

    use super::{commit_in, OperationDelivery, OperationResolution, SqliteStore};

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        (dir, store)
    }

    #[test]
    fn admission_epoch_rejects_a_delayed_send_before_saving_or_dispatching() {
        let (_dir, store) = store();
        let epoch = store.reading_identity().unwrap().1;
        let command = json!({"kind":"task.complete","issue":"LOO-1"});
        store
            .admit_operation(&epoch, "known", "task.complete", "hash", &command)
            .unwrap();
        let before = store.operation_receipt("known").unwrap().unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE client_store_epoch SET epoch='replacement' WHERE singleton=1",
                [],
            )
            .unwrap();
        for id in ["known", "new"] {
            assert!(store
                .admit_operation(&epoch, id, "task.complete", "hash", &command)
                .is_err());
        }
        assert!(store.operation_receipt("new").unwrap().is_none());
        assert_eq!(
            serde_json::to_value(store.operation_receipt("known").unwrap().unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
    }

    #[test]
    fn receipt_fixture_keeps_nullable_fields_required() {
        let message: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/dto/server/operation_receipt.json"
        ))
        .unwrap();
        let wire = message["result"].clone();
        let receipt: super::OperationReceipt = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(receipt).unwrap(), wire);
        for resolution in ["committed", "definitely_refused", "superseded"] {
            let mut stage = wire["commits"][1].clone();
            stage["resolution"] = json!(resolution);
            let decoded: super::OperationCommit = serde_json::from_value(stage.clone()).unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), stage);
        }
        for field in ["resolution", "planning_effect"] {
            let mut missing = wire["commits"][1].clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<super::OperationCommit>(missing).is_err());
        }
        for field in [
            "request_id",
            "kind",
            "fingerprint",
            "submitted_fingerprint",
            "validation",
            "delivery",
            "revision",
            "result",
            "error",
            "commits",
            "created_at",
            "updated_at",
        ] {
            let mut missing = wire.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<super::OperationReceipt>(missing).is_err(),
                "{field}"
            );
        }
    }

    fn identity(id: &str, fingerprint: &str) -> super::CreationIdentity {
        super::CreationIdentity {
            request_id: id.into(),
            kind: "task.create",
            fingerprint: fingerprint.into(),
            run: true,
        }
    }

    #[test]
    fn filing_correlates_the_original_export_and_declares_local_only_planning() {
        let (dir, store) = store();
        let repository_id = store.ensure_repository("/filing").unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Filed once".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let epoch = store.reading_identity().unwrap().1;
        for (id, connected) in [("export", true), ("local", false)] {
            store
                .admit_operation(
                    &epoch,
                    id,
                    "task.create",
                    id,
                    &json!({"kind":"task.create"}),
                )
                .unwrap();
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            let changes = if connected {
                vec![crate::store::planning_sync::PlanningChangeRef {
                    kind: crate::store::planning_sync::PlanningChangeKind::TaskCreation,
                    owner: task.id.to_string(),
                    id: task.id.to_string(),
                }]
            } else {
                Vec::new()
            };
            super::commit_filing_in(
                &tx,
                id,
                task.id.as_str(),
                None,
                &json!({"original":id}),
                changes,
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let exported = store.operation_receipt("export").unwrap().unwrap();
        let stage = &exported.commits[0];
        assert_eq!(stage.planning_effect, super::PlanningEffect::Correlated);
        assert_eq!(stage.planning_changes[0].id, task.id.as_str());
        assert_eq!(
            exported.planning.unwrap().changes[0].state,
            super::PlanningDeliveryState::Pending
        );
        let local = store.operation_receipt("local").unwrap().unwrap();
        assert_eq!(
            local.commits[0].planning_effect,
            super::PlanningEffect::None
        );
        assert!(local.planning.is_none());
        store.conn.lock().unwrap().execute(
            "UPDATE task_creation_intents SET export_attempted=1,export_error='reply lost' WHERE task_id=?1", [task.id.as_str()]).unwrap();
        drop(store);
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let uncertain = store.operation_receipt("export").unwrap().unwrap();
        assert_eq!(uncertain.commits[0].result, json!({"original":"export"}));
        assert_eq!(
            uncertain.planning.unwrap().changes[0].state,
            super::PlanningDeliveryState::Uncertain
        );
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE task_creation_intents SET export_json=json_object('id','expected-export') WHERE task_id=?1", [task.id.as_str()]).unwrap();
            conn.execute(
                "UPDATE tasks SET external_issue_id='another-export' WHERE id=?1",
                [task.id.as_str()],
            )
            .unwrap();
        }
        assert_eq!(
            store
                .operation_receipt("export")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            super::PlanningDeliveryState::Unavailable
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET external_issue_id='expected-export' WHERE id=?1",
                [task.id.as_str()],
            )
            .unwrap();
        assert_eq!(
            store
                .operation_receipt("export")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            super::PlanningDeliveryState::Delivered
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM task_creation_intents WHERE task_id=?1",
                [task.id.as_str()],
            )
            .unwrap();
        assert_eq!(
            store
                .operation_receipt("export")
                .unwrap()
                .unwrap()
                .planning
                .unwrap()
                .changes[0]
                .state,
            super::PlanningDeliveryState::Unavailable
        );
    }

    #[test]
    fn canonical_claim_and_submitted_content_have_independent_immutable_identities() {
        let (_dir, store) = store();
        let command = json!({"kind":"task.create","directory":"/spelled/path"});
        let receipt = store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "task.create",
                "submitted",
                &command,
            )
            .unwrap();
        assert_eq!(receipt.fingerprint, None);
        assert_eq!(receipt.submitted_fingerprint.as_deref(), Some("submitted"));
        assert_eq!(receipt.validation, super::OperationValidation::Pending);
        assert!(!store.dispatch_operation("one").unwrap());
        let saved = store.submitted_command("one").unwrap().unwrap();
        // A direct CLI request claims the same canonical identity, atomically.
        store.claim_creation(&identity("one", "canonical")).unwrap();
        assert!(store.claim_creation(&identity("one", "other")).is_err());
        assert!(store
            .validate_operation("one", saved.generation, &identity("one", "canonical"))
            .unwrap());
        let validated = store.operation_receipt("one").unwrap().unwrap();
        assert_eq!(validated.fingerprint.as_deref(), Some("canonical"));
        assert_eq!(
            validated.submitted_fingerprint,
            receipt.submitted_fingerprint
        );
        assert!(store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "task.create",
                "another",
                &json!({"kind":"task.create","directory":"/equivalent"})
            )
            .is_err());
        assert!(store.dispatch_operation("one").unwrap());
        assert!(!store.dispatch_operation("one").unwrap());
    }

    #[test]
    fn wrong_legacy_candidate_releases_only_its_reservation_and_held_validation_cannot_bind_a_successor(
    ) {
        let (_dir, store) = store();
        store.conn.lock().unwrap().execute(r#"INSERT INTO creation_receipts
            (request_id,kind,fingerprint,run,task_id,milestone,result,launch,created_at,updated_at)
            VALUES ('old','task.create','canonical',1,'task','admitted','{"task":1}','{"graph":2}',1,2)"#,[]).unwrap();
        let before = store.creation_receipt("old").unwrap().unwrap();
        let wrong = store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "old",
                "task.create",
                "wrong",
                &json!({"directory":"wrong"}),
            )
            .unwrap();
        assert_eq!(wrong.validation, super::OperationValidation::Candidate);
        assert_eq!(wrong.delivery, OperationDelivery::Untracked);
        let generation = store.submitted_command("old").unwrap().unwrap().generation;
        assert!(!store.dispatch_operation("old").unwrap());
        assert!(store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "old",
                "task.create",
                "correct",
                &json!({"directory":"correct"})
            )
            .is_err());
        assert!(!store
            .validate_operation("old", generation, &identity("old", "wrong"))
            .unwrap());
        let conflict = store.operation_receipt("old").unwrap().unwrap();
        assert_eq!(conflict.validation, super::OperationValidation::Conflict);
        let next = store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "old",
                "task.create",
                "correct",
                &json!({"directory":"correct"}),
            )
            .unwrap();
        assert!(next.revision > conflict.revision);
        // A held success or failure for the first reservation cannot touch the next.
        assert!(!store
            .validate_operation("old", generation, &identity("old", "canonical"))
            .unwrap());
        store
            .record_validation_error("old", generation, "old failure")
            .unwrap();
        let generation = store.submitted_command("old").unwrap().unwrap().generation;
        assert!(store
            .validate_operation("old", generation, &identity("old", "canonical"))
            .unwrap());
        let bound_revision = store.operation_receipt("old").unwrap().unwrap().revision;
        assert!(store
            .validate_operation("old", generation, &identity("old", "late-wrong-discovery"))
            .unwrap());
        assert_eq!(
            store.operation_receipt("old").unwrap().unwrap().revision,
            bound_revision
        );
        let after = store.creation_receipt("old").unwrap().unwrap();
        assert_eq!(before.result, after.result);
        assert_eq!(before.launch, after.launch);
        assert_eq!(before.task_id, after.task_id);
        assert_eq!(before.milestone, after.milestone);
        assert_eq!(before.disposition, after.disposition);
        assert_eq!(before.created_at, after.created_at);
        let receipt = store.operation_receipt("old").unwrap().unwrap();
        assert_eq!(receipt.delivery, OperationDelivery::Untracked);
        assert_eq!(receipt.error, None);
        assert!(receipt.commits.is_empty());
    }

    #[test]
    fn legacy_adoption_keeps_complete_continue_and_inspect_dispositions_and_original_sessions() {
        let (_dir, store) = store();
        let repository_id = store.ensure_repository("/adoption").unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Original".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        for (id, milestone, run, disposition) in [
            (
                "complete",
                "filed",
                false,
                super::RecoveryDisposition::Complete,
            ),
            (
                "continue",
                "filed",
                true,
                super::RecoveryDisposition::Continue,
            ),
            (
                "inspect",
                "run_requested",
                true,
                super::RecoveryDisposition::Inspect,
            ),
        ] {
            let original = json!({"task":task.id,"session_id":"original-session"});
            store.conn.lock().unwrap().execute("INSERT INTO creation_receipts(request_id,kind,fingerprint,run,task_id,session_id,milestone,result,created_at,updated_at)
                VALUES(?1,'task.create','canonical',?2,?3,'original-session',?4,?5,1,1)",
                params![id,run,task.id.as_str(),milestone,original.to_string()]).unwrap();
            assert_eq!(
                store.creation_receipt(id).unwrap().unwrap().disposition,
                disposition
            );
            store
                .admit_operation(
                    &store.reading_identity().unwrap().1,
                    id,
                    "task.create",
                    "submitted",
                    &json!({"directory":"equivalent"}),
                )
                .unwrap();
            let generation = store.submitted_command(id).unwrap().unwrap().generation;
            assert!(store
                .validate_operation(id, generation, &identity(id, "canonical"))
                .unwrap());
            let adopted = store.creation_receipt(id).unwrap().unwrap();
            assert_eq!(adopted.disposition, disposition);
            assert_eq!(adopted.session_id.as_deref(), Some("original-session"));
            assert_eq!(adopted.result, Some(original));
            assert_eq!(adopted.milestone, milestone);
            assert!(store
                .operation_receipt(id)
                .unwrap()
                .unwrap()
                .commits
                .is_empty());
        }
    }

    #[test]
    fn unavailable_candidate_normalization_survives_restart_without_replacement_or_dispatch() {
        let (dir, store) = store();
        store.claim_creation(&identity("old", "canonical")).unwrap();
        let command = json!({"directory":"/unavailable"});
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "old",
                "task.create",
                "submitted",
                &command,
            )
            .unwrap();
        let generation = store.submitted_command("old").unwrap().unwrap().generation;
        store
            .record_validation_error("old", generation, "repository unavailable")
            .unwrap();
        drop(store);
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let duplicate = store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "old",
                "task.create",
                "submitted",
                &command,
            )
            .unwrap();
        assert_eq!(duplicate.validation, super::OperationValidation::Candidate);
        assert_eq!(duplicate.error.as_deref(), Some("repository unavailable"));
        assert!(!store.dispatch_operation("old").unwrap());
        let saved = store.submitted_command("old").unwrap().unwrap();
        assert_eq!(saved.command, command);
        assert_eq!(saved.generation, generation);
        assert!(store
            .validate_operation("old", generation, &identity("old", "canonical"))
            .unwrap());
        assert!(store.dispatch_operation("old").unwrap());
        drop(store);
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        assert!(!store.dispatch_operation("old").unwrap());
    }

    #[test]
    fn a_new_envelope_stays_immutable_when_a_different_cli_creation_wins_the_domain_claim() {
        let (_dir, store) = store();
        let original = json!({"directory":"original"});
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "task.create",
                "submitted",
                &original,
            )
            .unwrap();
        store
            .claim_creation(&identity("one", "cli-winner"))
            .unwrap();
        let generation = store.submitted_command("one").unwrap().unwrap().generation;
        assert!(!store
            .validate_operation("one", generation, &identity("one", "different"))
            .unwrap());
        assert!(store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "task.create",
                "replacement",
                &json!({"directory":"replacement"})
            )
            .is_err());
        assert_eq!(
            store
                .admit_operation(
                    &store.reading_identity().unwrap().1,
                    "one",
                    "task.create",
                    "submitted",
                    &original
                )
                .unwrap()
                .validation,
            super::OperationValidation::Conflict
        );
        assert!(!store.dispatch_operation("one").unwrap());
    }

    #[test]
    fn admission_and_dispatch_survive_restart_without_replaying_uncertain_work() {
        let (dir, store) = store();
        let command = json!({"kind":"task.complete","issue":"LOO-1"});
        let queued = store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "task.complete",
                "hash",
                &command,
            )
            .unwrap();
        assert_eq!(queued.delivery, OperationDelivery::Queued);
        assert!(store.dispatch_operation("one").unwrap());
        drop(store);
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let duplicate = store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "task.complete",
                "hash",
                &command,
            )
            .unwrap();
        assert_eq!(duplicate.delivery, OperationDelivery::Dispatched);
        assert!(duplicate.revision > queued.revision);
        assert!(!store.dispatch_operation("one").unwrap());
        store
            .finish_operation("one", Err("connection lost after a side effect"))
            .unwrap();
        assert!(!store.dispatch_operation("one").unwrap());
        let receipt = store.operation_receipt("one").unwrap().unwrap();
        assert_eq!(receipt.delivery, OperationDelivery::Uncertain);
        assert!(receipt.revision > duplicate.revision);
        assert_eq!(
            store
                .admit_operation(
                    &store.reading_identity().unwrap().1,
                    "one",
                    "task.complete",
                    "hash",
                    &command
                )
                .unwrap()
                .revision,
            receipt.revision
        );
        let saved: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT command FROM creation_receipts WHERE request_id='one'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&saved).unwrap(),
            json!({"version":1,"command":command})
        );
        assert!(store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "task.complete",
                "other",
                &json!({})
            )
            .is_err());
    }

    #[test]
    fn domain_facts_and_their_result_and_fence_commit_or_rollback_together() {
        let (_dir, store) = store();
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "one",
                "wave.rename",
                "hash",
                &json!({}),
            )
            .unwrap();
        let wave = crate::id::WaveId::new();
        let before = store.revisions().unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'Name','/repo',1)",
                [&wave],
            )
            .unwrap();
            commit_in(
                &tx,
                "one",
                "wave.renamed",
                &[wave.as_str()],
                &["planning"],
                &json!({"name":"Name"}),
            )
            .unwrap();
            tx.rollback().unwrap();
        }
        assert_eq!(store.revisions().unwrap(), before);
        assert!(store
            .operation_receipt("one")
            .unwrap()
            .unwrap()
            .commits
            .is_empty());
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'Name','/repo',1)",
                [&wave],
            )
            .unwrap();
            commit_in(
                &tx,
                "one",
                "wave.renamed",
                &[wave.as_str()],
                &["planning"],
                &json!({"name":"Name"}),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let fence_revision = store.revisions().unwrap().planning;
        // Another writer commits while the original command is still running.
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE waves SET name='Newer' WHERE id=?1", [&wave])
            .unwrap();
        store.dispatch_operation("one").unwrap();
        store
            .finish_operation("one", Ok(&json!({"returned":"later"})))
            .unwrap();
        let receipt = store.operation_receipt("one").unwrap().unwrap();
        assert_eq!(receipt.commits.len(), 1);
        assert_eq!(receipt.commits[0].fence.revisions.len(), 1);
        assert_eq!(
            receipt.commits[0].fence.revisions["planning"],
            fence_revision
        );
        assert!(fence_revision < store.revisions().unwrap().planning);
        assert_eq!(receipt.commits[0].result, json!({"name":"Name"}));
        assert_eq!(receipt.result, Some(json!({"returned":"later"})));
        let conn = store.conn.lock().unwrap();
        assert!(commit_in(&conn, "one", "too late", &[], &["planning"], &json!({})).is_err());
    }

    #[test]
    fn a_failed_fence_rolls_back_the_domain_change() {
        let (_dir, store) = store();
        let before = store.revisions().unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'Name','/repo',1)",
                [crate::id::WaveId::new()],
            )
            .unwrap();
            assert!(commit_in(
                &tx,
                "missing",
                "wave.created",
                &[],
                &["planning"],
                &json!({})
            )
            .is_err());
        }
        assert_eq!(store.revisions().unwrap(), before);
    }

    #[test]
    fn a_released_creation_keeps_its_identity_result_and_recovery_columns() {
        let (_dir, store) = store();
        store.conn.lock().unwrap().execute("INSERT INTO creation_receipts
            (request_id,kind,fingerprint,run,task_id,session_id,artifact_key,milestone,result,launch,created_at,updated_at)
            VALUES ('old','session.new','hash',0,NULL,'session','artifact','admitted',?1,?2,1,2)",
            params![json!({"session":"session"}).to_string(),json!({"started":true}).to_string()]).unwrap();
        let before = store.operation_revision().unwrap();
        let receipt = store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "old",
                "session.new",
                "hash",
                &json!({"kind":"session.new"}),
            )
            .unwrap();
        assert_eq!(receipt.request_id, "old");
        assert!(
            receipt.commits.is_empty(),
            "migration cannot invent historical fences"
        );
        let (milestone,result,launch,artifact): (String,String,String,String) = store.conn.lock().unwrap().query_row(
            "SELECT milestone,result,launch,artifact_key FROM creation_receipts WHERE request_id='old'",[],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
        assert_eq!(milestone, "admitted");
        assert_eq!(result, json!({"session":"session"}).to_string());
        assert_eq!(launch, json!({"started":true}).to_string());
        assert_eq!(artifact, "artifact");
        assert!(store.operation_revision().unwrap() > before);
    }
    #[test]
    fn abandonment_receipts_keep_the_same_canceled_delivery_on_repeat() {
        let (_dir, store) = store();
        let repo = loopflow_test_support::TestRepo::new();
        let repository_id = store
            .ensure_repository(repo.path().to_str().unwrap())
            .unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Abandon receipt".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let mut original = None;
        for id in ["abandon", "already-abandoned"] {
            store
                .admit_operation(
                    &store.reading_identity().unwrap().1,
                    id,
                    "task.abandon",
                    "hash",
                    &json!({}),
                )
                .unwrap();
            assert!(store.dispatch_operation(id).unwrap());
            crate::journal::request::with_operation(Some(id.into()), || {
                store
                    .abandon(&crate::durable::WorkRef::Task(task.id.clone()), "Canceled")
                    .unwrap();
            });
            let receipt = store.operation_receipt(id).unwrap().unwrap();
            assert_eq!(receipt.commits.len(), 1);
            let commit = &receipt.commits[0];
            assert_eq!(commit.stage, "task.abandoned");
            assert_eq!(
                commit.fence.revisions["planning"],
                store.revisions().unwrap().planning
            );
            assert_eq!(commit.planning_changes.len(), 1);
            if let Some(original) = &original {
                assert_eq!(&commit.planning_changes, original);
            }
            original = Some(commit.planning_changes.clone());
        }
    }

    #[test]
    fn periodic_settlement_confirms_every_subscriber_to_the_exact_completion_request() {
        let (dir, store) = store();
        let repo = loopflow_test_support::TestRepo::new();
        let repository_id = store
            .ensure_repository(repo.path().to_str().unwrap())
            .unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Completion subscribers".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let mut request = None;
        for id in ["first", "second"] {
            store
                .admit_operation(
                    &store.reading_identity().unwrap().1,
                    id,
                    "task.complete",
                    "hash",
                    &json!({}),
                )
                .unwrap();
            assert!(store.dispatch_operation(id).unwrap());
            let current = crate::journal::request::with_operation(Some(id.into()), || {
                store.request_task_completion(&task.id, None).unwrap()
            });
            if let Some(request) = request {
                assert_eq!(Some(request), current);
            }
            request = current;
        }
        drop(store);
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        // Periodic settlement has no request attribution, even after restart.
        assert!(store.complete_task(&task, request.unwrap()).unwrap());
        let first = store.operation_receipt("first").unwrap().unwrap();
        let second = store.operation_receipt("second").unwrap().unwrap();
        assert_eq!(first.commits, second.commits);
        assert_eq!(first.commits.len(), 2);
        assert_eq!(first.commits[1].stage, "task.completed");
        assert_eq!(
            first.commits[1].fence.revisions["planning"],
            store.revisions().unwrap().planning
        );
        store
            .finish_operation("first", Err("output lost after settlement"))
            .unwrap();
        store.reopen_task(&task.id, None).unwrap();
        assert!(store
            .recover_operation_result("first", "task.completed")
            .unwrap());
        let recovered = store.operation_receipt("first").unwrap().unwrap();
        assert_eq!(recovered.result, Some(first.commits[1].result.clone()));
        assert_eq!(
            recovered.error.as_deref(),
            Some("output lost after settlement")
        );
        assert_eq!(recovered.commits, first.commits);
        assert_ne!(
            store.task(&task.id).unwrap().unwrap().state.as_deref(),
            Some("completed")
        );

        // Withdraw an older request, then complete a newer one. A delayed old
        // settler must not certify that later completion for the withdrawn one.
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "withdrawn",
                "task.complete",
                "hash",
                &json!({}),
            )
            .unwrap();
        let withdrawn = crate::journal::request::with_operation(Some("withdrawn".into()), || {
            store
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap()
        });
        store.reopen_task(&task.id, None).unwrap();
        let newer = store
            .request_task_completion(&task.id, None)
            .unwrap()
            .unwrap();
        assert_ne!(withdrawn, newer);
        assert!(store.complete_task(&task, newer).unwrap());
        assert!(store.complete_task(&task, withdrawn).unwrap());
        let old = store.operation_receipt("withdrawn").unwrap().unwrap();
        assert_eq!(old.commits.len(), 2);
        assert_eq!(old.commits[0].stage, "task.completion_requested");
        assert_eq!(old.commits[1].resolution, OperationResolution::Superseded);
        assert!(!store
            .recover_operation_result("withdrawn", "task.completed")
            .unwrap());
    }

    #[test]
    fn withdrawn_completion_keeps_its_replacement_fence_across_restart_and_late_settlement() {
        let (dir, store) = store();
        let repo = loopflow_test_support::TestRepo::new();
        let repository_id = store
            .ensure_repository(repo.path().to_str().unwrap())
            .unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Withdrawn completion".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let mut request = None;
        for id in ["lost-worker", "returned-worker"] {
            store
                .admit_operation(
                    &store.reading_identity().unwrap().1,
                    id,
                    "task.complete",
                    "hash",
                    &json!({}),
                )
                .unwrap();
            assert!(store.dispatch_operation(id).unwrap());
            request = crate::journal::request::with_operation(Some(id.into()), || {
                store.request_task_completion(&task.id, None).unwrap()
            });
        }
        // Command output can return before settlement: delivery cannot hide a
        // later domain resolution from that subscriber.
        store
            .finish_operation("returned-worker", Ok(&json!({"pending":true})))
            .unwrap();
        let before = store.revisions().unwrap().planning;
        store.conn.lock().unwrap().execute_batch(
            "CREATE TEMP TRIGGER fail_resolution BEFORE UPDATE OF commits ON creation_receipts
             WHEN EXISTS(SELECT 1 FROM json_each(NEW.commits) WHERE json_extract(value,'$.stage')='task.completed')
             BEGIN SELECT RAISE(ABORT,'receipt write failed'); END;",
        ).unwrap();
        assert!(store.reopen_task(&task.id, None).is_err());
        assert_eq!(
            store
                .task_completion_pending(&task.id)
                .unwrap()
                .map(|(id, _)| id),
            request
        );
        assert_eq!(
            store
                .operation_receipt("lost-worker")
                .unwrap()
                .unwrap()
                .commits
                .len(),
            1
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_resolution")
            .unwrap();
        store.reopen_task(&task.id, None).unwrap();
        let receipt = store.operation_receipt("lost-worker").unwrap().unwrap();
        assert_eq!(
            receipt.commits,
            store
                .operation_receipt("returned-worker")
                .unwrap()
                .unwrap()
                .commits
        );
        let resolution = &receipt.commits[1];
        assert_eq!(resolution.stage, "task.completed");
        assert_eq!(resolution.resolution, OperationResolution::Superseded);
        assert_eq!(resolution.result, json!({"request":request.unwrap()}));
        assert!(resolution.fence.revisions["planning"] > before);
        assert_eq!(
            resolution.fence.revisions["planning"],
            store.revisions().unwrap().planning
        );
        assert!(!store.complete_task(&task, request.unwrap()).unwrap());
        assert!(store.task_completion_pending(&task.id).unwrap().is_none());
        drop(store);
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        assert_eq!(
            store
                .operation_receipt("lost-worker")
                .unwrap()
                .unwrap()
                .commits,
            receipt.commits
        );
        assert!(!store
            .recover_operation_result("lost-worker", "task.completed")
            .unwrap());
        store.reopen_task(&task.id, None).unwrap();
        let newer = store
            .request_task_completion(&task.id, None)
            .unwrap()
            .unwrap();
        assert!(store.complete_task(&task, newer).unwrap());
        assert!(store.complete_task(&task, request.unwrap()).unwrap());
        assert_eq!(
            store
                .operation_receipt("lost-worker")
                .unwrap()
                .unwrap()
                .commits,
            receipt.commits
        );
    }

    #[test]
    fn replacement_pr_resolves_its_withdrawn_completion_request() {
        let (_dir, store) = store();
        let repository_id = store.ensure_repository("/supersession").unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Replaced completion".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let mut pr = crate::work::task::TaskPr {
            id: crate::work::task::TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: "first".into(),
            branch: "first".into(),
            base_commit: "base".into(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: Some(task.created_at),
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: task.created_at,
            updated_at: task.updated_at,
        };
        store.conn.lock().unwrap().execute(
            "INSERT INTO task_checkouts(task_id,worktree,branch,base_commit,parent_pr_id) VALUES(?1,'/supersession/task',?2,?3,NULL)",
            params![task.id.as_str(), pr.branch, pr.base_commit],
        ).unwrap();
        store.insert_task_pr(&pr).unwrap();
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "pr",
                "task.complete",
                "hash",
                &json!({}),
            )
            .unwrap();
        let request = crate::journal::request::with_operation(Some("pr".into()), || {
            store
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap()
        });
        pr.id = crate::work::task::TaskPrId::new();
        pr.sequence = 2;
        pr.slug = "second".into();
        pr.abandoned_at = None;
        store.insert_task_pr(&pr).unwrap();
        assert!(store.task_completion_pending(&task.id).unwrap().is_none());
        let receipt = store.operation_receipt("pr").unwrap().unwrap();
        assert_eq!(
            receipt.commits[1].resolution,
            OperationResolution::Superseded
        );
        assert_eq!(receipt.commits[1].result, json!({"request":request}));
    }

    #[test]
    fn linear_status_resolves_its_withdrawn_completion_request() {
        let (_dir, store) = store();
        let repository_id = store.ensure_repository("/supersession").unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Replaced completion".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let inventory: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let mut item = inventory.items[0].clone();
        item.project_id = None;
        item.completed = false;
        item.state = Some("unstarted".into());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                params![task.id.as_str(), item.id],
            )
            .unwrap();
        crate::linear::acceptance::accept_linear_issue_in(
            &store,
            "/supersession",
            &item,
            None,
            1,
            None,
        )
        .unwrap();
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "linear",
                "task.complete",
                "hash",
                &json!({}),
            )
            .unwrap();
        let request = crate::journal::request::with_operation(Some("linear".into()), || {
            store
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap()
        });
        item.state = Some("started".into());
        crate::linear::acceptance::accept_linear_issue_in(
            &store,
            "/supersession",
            &item,
            None,
            2,
            None,
        )
        .unwrap();
        assert!(store.task_completion_pending(&task.id).unwrap().is_none());
        let receipt = store.operation_receipt("linear").unwrap().unwrap();
        assert_eq!(
            receipt.commits[1].resolution,
            OperationResolution::Superseded
        );
        assert_eq!(receipt.commits[1].result, json!({"request":request}));
        assert!(
            receipt.commits[1].fence.revisions["planning"] <= store.revisions().unwrap().planning
        );
    }

    #[test]
    fn completion_and_reopen_record_their_own_committed_task_results() {
        let (_dir, store) = store();
        let repo = loopflow_test_support::TestRepo::new();
        let repository_id = store
            .ensure_repository(repo.path().to_str().unwrap())
            .unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Receipt fixture".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "complete",
                "task.complete",
                "hash",
                &json!({}),
            )
            .unwrap();
        crate::journal::request::with_operation(Some("complete".into()), || {
            let request = store
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap();
            assert!(store.complete_task(&task, request).unwrap());
        });
        let completed = store.operation_receipt("complete").unwrap().unwrap();
        assert_eq!(
            completed
                .commits
                .iter()
                .map(|commit| commit.stage.as_str())
                .collect::<Vec<_>>(),
            ["task.completion_requested", "task.completed"]
        );
        let stage = completed.commits.last().unwrap();
        let result: crate::work::task::Task = serde_json::from_value(stage.result.clone()).unwrap();
        assert_eq!(result.state.as_deref(), Some("completed"));
        assert_eq!(
            stage.fence.revisions["planning"],
            store.revisions().unwrap().planning
        );
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "already",
                "task.complete",
                "hash",
                &json!({}),
            )
            .unwrap();
        crate::journal::request::with_operation(Some("already".into()), || {
            assert!(store
                .request_task_completion(&task.id, None)
                .unwrap()
                .is_none());
        });
        let already = store.operation_receipt("already").unwrap().unwrap();
        assert_eq!(already.commits[0].stage, "task.completed");
        assert_eq!(already.commits[0].result, stage.result);
        store
            .admit_operation(
                &store.reading_identity().unwrap().1,
                "reopen",
                "task.reopen",
                "hash",
                &json!({}),
            )
            .unwrap();
        let reopened = crate::journal::request::with_operation(Some("reopen".into()), || {
            store.reopen_task(&task.id, None)
        })
        .unwrap();
        assert_ne!(reopened.state.as_deref(), Some("completed"));
        let receipt = store.operation_receipt("reopen").unwrap().unwrap();
        assert_eq!(
            receipt.commits[0].result,
            serde_json::to_value(reopened).unwrap()
        );
        assert!(receipt.commits[0].fence.revisions["planning"] > stage.fence.revisions["planning"]);
        assert_eq!(
            store
                .operation_receipt("complete")
                .unwrap()
                .unwrap()
                .commits,
            completed.commits
        );
        assert!(crate::journal::request::operation_id().is_none());
    }

    #[test]
    fn finished_migration_preserves_released_creation_rows_and_revision_counters() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::store::migrations::apply_before_migration(&conn, "client_operations");
        conn.execute(r#"INSERT INTO creation_receipts(request_id,kind,fingerprint,run,task_id,session_id,artifact_key,milestone,result,launch,created_at,updated_at)
            VALUES('old','task.create','hash',1,'task','session','input','admitted','{"task":1}','{"graph":2}',10,20)"#,[]).unwrap();
        let prior: i64 = conn
            .query_row("SELECT sum(revision) FROM store_revisions", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute_batch(&crate::store::migrations::migration_section_sql(
            "client_operations",
        ))
        .unwrap();
        let (request,kind,fingerprint,run,task,session,artifact,milestone,result,launch,created,updated):
            (String,String,String,bool,String,String,String,String,String,String,i64,i64) = conn.query_row(
            "SELECT request_id,kind,fingerprint,run,task_id,session_id,artifact_key,milestone,result,launch,created_at,updated_at FROM creation_receipts",[],
            |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?))).unwrap();
        assert_eq!(
            (request, kind, fingerprint, run),
            ("old".into(), "task.create".into(), "hash".into(), true)
        );
        assert_eq!(
            (task, session, artifact, milestone),
            (
                "task".into(),
                "session".into(),
                "input".into(),
                "admitted".into()
            )
        );
        assert_eq!(
            (result, launch, created, updated),
            (
                json!({"task":1}).to_string(),
                json!({"graph":2}).to_string(),
                10,
                20
            )
        );
        assert_eq!(
            conn.query_row("SELECT sum(revision) FROM store_revisions", [], |row| row
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            prior
        );
        assert!(super::operation_in(&conn, "old")
            .unwrap()
            .unwrap()
            .commits
            .is_empty());
    }
    #[test]
    fn receipt_reads_and_duplicate_admission_use_point_queries_at_ten_thousand_tasks() {
        let (_dir, store) = store();
        let repository = store.ensure_repository("/receipt-benchmark").unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10000)
                INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,issue_description,workspace_slug,created_at,updated_at)
                SELECT 'task_receipt_'||i,?1,'RECEIPT-'||i,'Receipt Task','','',1,1 FROM n",[repository.as_str()]).unwrap();
            tx.execute("INSERT INTO creation_receipts(request_id,kind,fingerprint,run,task_id,milestone,created_at,updated_at)
                SELECT 'receipt_'||id,'task.complete','hash',0,id,'queued',1,1 FROM tasks",[]).unwrap();
            tx.execute("INSERT INTO task_changes(id,task_id,value_json)
                SELECT 'receipt_change_'||id,id,json_object('field','task_title','value',issue_title) FROM tasks", []).unwrap();
            tx.execute(
                "INSERT INTO task_creation_intents(task_id,title,description,repository_id)
                SELECT id,issue_title,issue_description,repository_id FROM tasks",
                [],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO task_state_deliveries(id,task_id,target)
                SELECT 'receipt_state_'||id,id,'completed' FROM tasks",
                [],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let command = json!({"kind":"task.complete"});
        let epoch = store.reading_identity().unwrap().1;
        store
            .admit_operation(&epoch, "selected", "task.complete", "hash", &command)
            .unwrap();
        store.dispatch_operation("selected").unwrap();
        let mut traced = crate::store::query_plans::assert_no_empty_writes(&store, || {
            crate::store::query_plans::statements(&store, || {
                assert!(store.operation_receipt("selected").unwrap().is_some());
                assert!(store.operation_revision().unwrap() > 0);
                store
                    .admit_operation(&epoch, "selected", "task.complete", "hash", &command)
                    .unwrap();
                assert!(!store.dispatch_operation("selected").unwrap());
            })
        });
        let mut samples = Vec::new();
        for _ in 0..20 {
            let began = std::time::Instant::now();
            store.operation_receipt("selected").unwrap().unwrap();
            store
                .admit_operation(&epoch, "selected", "task.complete", "hash", &command)
                .unwrap();
            assert!(!store.dispatch_operation("selected").unwrap());
            samples.push(began.elapsed().as_secs_f64() * 1_000.0);
        }
        samples.sort_by(f64::total_cmp);
        println!("10k Tasks/receipts; lookup + duplicate admission + no redispatch; n=20, median={:.3} ms, P95={:.3} ms (loaded-host diagnostic, not native latency)",
            (samples[9]+samples[10])/2.0,samples[18]);
        store.conn.lock().unwrap().execute(
            "UPDATE creation_receipts SET completion_request=1 WHERE request_id='receipt_task_receipt_1'", [],
        ).unwrap();
        let mut settlement_samples = Vec::new();
        for _ in 0..20 {
            let began = std::time::Instant::now();
            traced.extend(crate::store::query_plans::statements(&store, || {
                let mut conn = store.conn.lock().unwrap();
                let tx = conn.transaction().unwrap();
                super::completion_committed_in(
                    &tx,
                    "task_receipt_1",
                    1,
                    vec![crate::store::planning_sync::PlanningChangeRef {
                        kind: crate::store::planning_sync::PlanningChangeKind::TaskState,
                        owner: "task_receipt_1".into(),
                        id: "receipt_state_task_receipt_1".into(),
                    }],
                    &json!({}),
                )
                .unwrap();
                tx.commit().unwrap();
            }));
            settlement_samples.push(began.elapsed().as_secs_f64() * 1_000.0);
        }
        settlement_samples.sort_by(f64::total_cmp);
        traced.extend(crate::store::query_plans::statements(&store, || {
            let receipt = store
                .operation_receipt("receipt_task_receipt_1")
                .unwrap()
                .unwrap();
            assert_eq!(receipt.planning.unwrap().changes.len(), 1);
        }));
        println!("10k Tasks/receipts; exact-request settlement lookup; n=20, P95={:.3} ms (loaded-host diagnostic)", settlement_samples[18]);
        store.conn.lock().unwrap().execute(
            "UPDATE creation_receipts SET completion_request=2 WHERE request_id='receipt_task_receipt_2'", [],
        ).unwrap();
        traced.extend(crate::store::query_plans::statements(&store, || {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            super::completion_superseded_in(&tx, "task_receipt_2", 2).unwrap();
            tx.commit().unwrap();
        }));
        let inventory: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let mut issue = inventory.items[0].clone();
        issue.project_id = None;
        traced.extend(crate::store::query_plans::statements(&store, || {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            crate::linear::acceptance::put_item(&tx, "/receipt-benchmark", 1, &issue).unwrap();
            tx.commit().unwrap();
        }));
        let task = store
            .task(&crate::durable::TaskId::from_raw("task_receipt_3"))
            .unwrap()
            .unwrap();
        for (id, kind) in [
            ("edit-content", "task.update"),
            ("move-membership", "task.move"),
        ] {
            store
                .admit_operation(&epoch, id, kind, "hash", &json!({}))
                .unwrap();
            traced.extend(crate::store::query_plans::statements(&store, || {
                crate::journal::request::with_operation(Some(id.into()), || {
                    if kind == "task.update" {
                        store
                            .edit_task(
                                &task.id,
                                task.revision,
                                &crate::work::task::TaskUpdate {
                                    description: Some("Query trace".into()),
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                    } else {
                        store.move_task(&task.id, None, None).unwrap();
                    }
                });
            }));
        }
        traced.extend(crate::store::query_plans::statements(&store, || {
            let receipt = store.operation_receipt("edit-content").unwrap().unwrap();
            assert_eq!(receipt.planning.unwrap().changes.len(), 1);
        }));
        let filing = super::CreationIdentity {
            request_id: "filing-query".into(),
            kind: "task.create",
            fingerprint: "filing".into(),
            run: false,
        };
        traced.extend(crate::store::query_plans::statements(&store, || {
            store
                .promote_task(&task.id, None, None, None, None, Some(&filing))
                .unwrap();
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            super::commit_filing_in(
                &tx,
                &filing.request_id,
                task.id.as_str(),
                None,
                &json!({}),
                vec![crate::store::planning_sync::PlanningChangeRef {
                    kind: crate::store::planning_sync::PlanningChangeKind::TaskCreation,
                    owner: task.id.to_string(),
                    id: task.id.to_string(),
                }],
            )
            .unwrap();
            tx.commit().unwrap();
        }));
        traced.extend(crate::store::query_plans::statements(&store, || {
            assert_eq!(
                store
                    .operation_receipt(&filing.request_id)
                    .unwrap()
                    .unwrap()
                    .planning
                    .unwrap()
                    .changes
                    .len(),
                1
            );
        }));
        let conn = store.conn.lock().unwrap();
        for sql in traced {
            if !sql.starts_with("SELECT") && !sql.starts_with("UPDATE") {
                continue;
            }
            let mut query = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let unbound = vec![rusqlite::types::Null; query.parameter_count()];
            let details = query
                .query_map(rusqlite::params_from_iter(unbound), |row| {
                    row.get::<_, String>(3)
                })
                .unwrap();
            for detail in details {
                let detail = detail.unwrap();
                assert!(
                    !detail.starts_with("SCAN")
                        || detail == "SCAN CONSTANT ROW"
                        || detail.starts_with("SCAN json_each VIRTUAL TABLE")
                        // Planning writes inspect the fixed schema's columns,
                        // never Task rows, through SQLite's metadata table.
                        || detail.starts_with("SCAN pragma_table_info VIRTUAL TABLE"),
                    "{detail}: {sql}"
                );
            }
        }
    }
}
