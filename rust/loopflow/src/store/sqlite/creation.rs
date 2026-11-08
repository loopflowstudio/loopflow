//! Receipts outlive their resources: deleting work never makes an identity reusable.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::SqliteStore;
use crate::store::{StoreError, StoreResult};

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
    conn.execute("INSERT INTO creation_receipts
        (request_id,kind,fingerprint,task_id,session_id,artifact_key,milestone,result,created_at,updated_at,run)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?10)", params![identity.request_id, identity.kind,
        identity.fingerprint, task_id, session_id, artifact_key, milestone,
        result.map(serde_json::to_string).transpose()?, now, identity.run])?;
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
            FROM creation_receipts WHERE request_id=?1", [id], |row| {
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
