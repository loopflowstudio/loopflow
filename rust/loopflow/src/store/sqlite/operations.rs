//! Receipts outlive their resources: deleting work never makes an identity reusable.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::SqliteStore;
use crate::store::{StoreError, StoreResult};

/// A domain writer captures this inside the transaction that changes its facts.
/// Only named domains can acknowledge the commit; unrelated counters do not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CommitFence {
    pub machine: String,
    pub epoch: String,
    pub schema: u32,
    pub revisions: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationCommit {
    pub stage: String,
    pub resources: Vec<String>,
    pub fence: CommitFence,
    pub result: serde_json::Value,
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

/// Creation recovery and general commands read the same row. Command output
/// stays separate from the domain results that committed with displayed facts.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationReceipt {
    pub request_id: String,
    pub kind: String,
    pub fingerprint: String,
    pub delivery: OperationDelivery,
    /// Orders receipt updates within one database epoch, including same-second stages.
    pub revision: i64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub result: Option<serde_json::Value>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub error: Option<String>,
    pub commits: Vec<OperationCommit>,
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
    if conn.is_autocommit() {
        return Err(StoreError::InvalidData(
            "operation commits require the domain transaction".into(),
        ));
    }
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
    let commit = OperationCommit {
        stage: stage.into(),
        resources: resources.iter().map(|id| (*id).to_owned()).collect(),
        fence: CommitFence {
            machine,
            epoch,
            schema: crate::server::wire::REVISION,
            revisions,
        },
        result: result.clone(),
    };
    if conn.execute(
        "UPDATE creation_receipts SET commits=json_insert(commits,'$[#]',json(?2)),updated_at=?3 WHERE request_id=?1",
        params![id,serde_json::to_string(&commit)?,crate::store::rows::now_unix()],
    )? != 1 {
        return Err(StoreError::NotFound);
    }
    Ok(())
}

pub(super) fn commit_current_in(
    conn: &Connection,
    stage: &str,
    resources: &[&str],
    domains: &[&str],
    result: impl FnOnce() -> StoreResult<serde_json::Value>,
) -> StoreResult<()> {
    if let Some(id) = crate::journal::request::operation_id() {
        commit_in(conn, &id, stage, resources, domains, &result()?)?;
    }
    Ok(())
}

impl SqliteStore {
    /// Admission is durable before dispatch. A duplicate never resets delivery
    /// or replaces the immutable command, including after a server restart.
    pub(crate) fn admit_operation(
        &self,
        id: &str,
        kind: &str,
        fingerprint: &str,
        command: &serde_json::Value,
    ) -> StoreResult<OperationReceipt> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let encoded = serde_json::to_string(command)?;
        let known = |conn: &Connection| -> StoreResult<Option<(String, String, Option<String>)>> {
            Ok(conn
                .query_row(
                    "SELECT kind,fingerprint,command FROM creation_receipts WHERE request_id=?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?)
        };
        if let Some((known_kind, known_fingerprint, known_command)) = known(&conn)? {
            if known_kind != kind
                || known_fingerprint != fingerprint
                || known_command
                    .as_ref()
                    .is_some_and(|value| value != &encoded)
            {
                return Err(StoreError::Stale(format!(
                    "operation {id} has different content"
                )));
            }
            if known_command.is_some() {
                return operation_in(&conn, id)?.ok_or(StoreError::NotFound);
            }
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing = known(&tx)?;
        if let Some((known_kind, known_fingerprint, known_command)) = existing {
            if known_kind != kind
                || known_fingerprint != fingerprint
                || known_command
                    .as_ref()
                    .is_some_and(|known| known != &encoded)
            {
                return Err(StoreError::Stale(format!(
                    "operation {id} has different content"
                )));
            }
            // Released creation rows have no command envelope. Their domain
            // recovery owner decides whether the exact creation may continue.
            tx.execute("UPDATE creation_receipts SET command=?2,delivery_state='queued' WHERE request_id=?1 AND command IS NULL",
                params![id,encoded])?;
        } else {
            let now = crate::store::rows::now_unix();
            tx.execute("INSERT INTO creation_receipts(request_id,kind,fingerprint,run,milestone,created_at,updated_at,command,delivery_state)
                VALUES(?1,?2,?3,0,'queued',?4,?4,?5,'queued')", params![id,kind,fingerprint,now,encoded])?;
        }
        let receipt = operation_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(receipt)
    }

    /// Only a definitely unsent command can be claimed. Dispatched/uncertain
    /// commands require their domain recovery owner, never generic replay.
    pub(crate) fn dispatch_operation(&self, id: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let queued: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM creation_receipts WHERE request_id=?1 AND delivery_state='queued')",
            [id], |row|row.get(0),
        )?;
        if !queued {
            return Ok(false);
        }
        Ok(conn.execute(
            "UPDATE creation_receipts SET delivery_state='dispatched',updated_at=?2
            WHERE request_id=?1 AND delivery_state='queued'",
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
            WHERE request_id=?1 AND delivery_state='dispatched'", params![id,state,output,error,crate::store::rows::now_unix()])? != 1 {
            return Err(StoreError::Stale(format!("operation {id} is not dispatched")));
        }
        Ok(())
    }

    pub fn operation_receipt(&self, id: &str) -> StoreResult<Option<OperationReceipt>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        operation_in(&conn, id)
    }
}

fn operation_in(conn: &Connection, id: &str) -> StoreResult<Option<OperationReceipt>> {
    let row = conn.query_row("SELECT request_id,kind,fingerprint,delivery_state,CASE WHEN delivery_state='finished' THEN result END,command_error,commits,created_at,updated_at,receipt_revision
        FROM creation_receipts WHERE request_id=?1", [id], |row| Ok((
            row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,
            row.get::<_,Option<String>>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,String>(6)?,
            row.get::<_,i64>(7)?,row.get::<_,i64>(8)?,row.get::<_,i64>(9)?
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
        )| {
            Ok(OperationReceipt {
                request_id,
                kind,
                fingerprint,
                revision,
                delivery: serde_json::from_value(serde_json::Value::String(delivery))?,
                result: result.map(|text| serde_json::from_str(&text)).transpose()?,
                error,
                commits: serde_json::from_str(&commits)?,
                created_at,
                updated_at,
            })
        },
    )
    .transpose()
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
    if milestone == "filed" {
        commit_in(
            conn,
            &identity.request_id,
            "task.filed",
            &task_id.into_iter().chain(session_id).collect::<Vec<_>>(),
            &["planning", "sessions"],
            result.unwrap_or(&serde_json::Value::Null),
        )?;
    }
    Ok(())
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

    pub(crate) fn check_creation(&self, identity: &CreationIdentity) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let fingerprint: Option<(String, String)> = conn
            .query_row(
                "SELECT kind,fingerprint FROM creation_receipts WHERE request_id=?1",
                [&identity.request_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if fingerprint.is_some_and(|(kind, fingerprint)| {
            kind != identity.kind || fingerprint != identity.fingerprint
        }) {
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

    use super::{commit_in, OperationDelivery, SqliteStore};

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        (dir, store)
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
        for field in [
            "request_id",
            "kind",
            "fingerprint",
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

    #[test]
    fn admission_and_dispatch_survive_restart_without_replaying_uncertain_work() {
        let (dir, store) = store();
        let command = json!({"kind":"task.complete","issue":"LOO-1"});
        let queued = store
            .admit_operation("one", "task.complete", "hash", &command)
            .unwrap();
        assert_eq!(queued.delivery, OperationDelivery::Queued);
        assert!(store.dispatch_operation("one").unwrap());
        drop(store);
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let duplicate = store
            .admit_operation("one", "task.complete", "hash", &command)
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
                .admit_operation("one", "task.complete", "hash", &command)
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
            command
        );
        assert!(store
            .admit_operation("one", "task.complete", "other", &json!({}))
            .is_err());
    }

    #[test]
    fn domain_facts_and_their_result_and_fence_commit_or_rollback_together() {
        let (_dir, store) = store();
        store
            .admit_operation("one", "wave.rename", "hash", &json!({}))
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
            .admit_operation("old", "session.new", "hash", &json!({"kind":"session.new"}))
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
            .admit_operation("complete", "task.complete", "hash", &json!({}))
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
            .admit_operation("already", "task.complete", "hash", &json!({}))
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
            .admit_operation("reopen", "task.reopen", "hash", &json!({}))
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
                INSERT INTO tasks(id,repository_id,created_at) SELECT 'task_receipt_'||i,?1,1 FROM n",[repository.as_str()]).unwrap();
            tx.execute("INSERT INTO creation_receipts(request_id,kind,fingerprint,run,task_id,milestone,created_at,updated_at)
                SELECT 'receipt_'||id,'task.complete','hash',0,id,'queued',1,1 FROM tasks",[]).unwrap();
            tx.commit().unwrap();
        }
        let command = json!({"kind":"task.complete"});
        store
            .admit_operation("selected", "task.complete", "hash", &command)
            .unwrap();
        store.dispatch_operation("selected").unwrap();
        let traced = crate::store::query_plans::assert_no_empty_writes(&store, || {
            crate::store::query_plans::statements(&store, || {
                assert!(store.operation_receipt("selected").unwrap().is_some());
                assert!(store.operation_revision().unwrap() > 0);
                store
                    .admit_operation("selected", "task.complete", "hash", &command)
                    .unwrap();
                assert!(!store.dispatch_operation("selected").unwrap());
            })
        });
        let conn = store.conn.lock().unwrap();
        for sql in traced {
            if !sql.starts_with("SELECT") {
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
                assert!(!detail.unwrap().starts_with("SCAN"), "{sql}");
            }
        }
    }
}
