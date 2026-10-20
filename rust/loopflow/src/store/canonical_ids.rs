//! Filesystem authority retained while structured identities change spelling.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::path::Path;

use fs2::FileExt;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::journal::{process_identity_evidence, ProcessIdentityEvidence};
use crate::store::{StoreError, StoreResult};

pub(crate) fn session_fence_file(database: &Path, locator: &str) -> StoreResult<File> {
    let root = database
        .canonicalize()
        .map_err(io_error)?
        .with_extension("session-locks");
    std::fs::create_dir_all(&root).map_err(io_error)?;
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(hex::encode(Sha256::digest(locator.as_bytes()))))
        .map_err(io_error)
}

fn io_error(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn changes_uuid(value: &str) -> bool {
    value != crate::id::canonical_uuid(value)
}

fn changes_session(value: &str) -> bool {
    value != crate::id::canonical_session(value)
}

/// The caller owns the guards until SQLite commits or rolls back. Never wait for
/// a fence while holding SQLite: its holder may be waiting to finish admission.
pub(super) fn preflight(conn: &Connection, guards: &mut Vec<File>) -> StoreResult<()> {
    let database = conn.path().filter(|path| !path.is_empty()).map(Path::new);
    let mut affected_owners = HashSet::new();
    let mut sessions = conn.prepare(
        "SELECT s.id,s.agent_process_id,p.trace_id,p.parent_lf_process_id,
         p.attached_lf_process_id,p.attachment_token FROM agent_sessions s
         LEFT JOIN processes p ON p.id=s.agent_process_id",
    )?;
    let mut sessions = sessions.query([])?;
    while let Some(row) = sessions.next()? {
        let session: String = row.get(0)?;
        let agent: Option<String> = row.get(1)?;
        let trace: Option<String> = row.get(2)?;
        let parent: Option<String> = row.get(3)?;
        let attached: Option<String> = row.get(4)?;
        let token: Option<String> = row.get(5)?;
        if !changes_session(&session)
            && ![&agent, &trace, &parent, &attached, &token]
                .into_iter()
                .flatten()
                .any(|value| changes_uuid(value))
        {
            continue;
        }
        affected_owners.extend([agent, parent, attached].into_iter().flatten());
        if let Some(database) = database {
            let file = session_fence_file(database, &session)?;
            match file.try_lock_exclusive() {
                Ok(()) => guards.push(file),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(StoreError::InvalidData(format!(
                        "canonical identity migration deferred: Session {session} fence is busy; retry after dispatch settles"
                    )));
                }
                Err(error) => return Err(io_error(error)),
            }
        }
    }
    let mut query = conn.prepare(
        "SELECT id,trace_id,kind,pid,os_started_at,started_at,completed_at,
         agent_session_id,caller_session_id,parent_lf_process_id,
         attached_lf_process_id,attachment_token,caller_agent_process_id FROM processes",
    )?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let trace: String = row.get(1)?;
        let session: Option<String> = row.get(7)?;
        let caller_session: Option<String> = row.get(8)?;
        let mut affected = affected_owners.contains(&id)
            || changes_uuid(&id)
            || changes_uuid(&trace)
            || session.as_deref().is_some_and(changes_session)
            || caller_session.as_deref().is_some_and(changes_session);
        for column in 9..=12 {
            affected |= row
                .get::<_, Option<String>>(column)?
                .as_deref()
                .is_some_and(changes_uuid);
        }
        if !affected {
            continue;
        }
        let kind: String = row.get(2)?;
        let completed: Option<i64> = row.get(6)?;
        let evidence = if kind == "agent" {
            crate::journal::agent_process_evidence(
                completed,
                row.get(3)?,
                row.get(4)?,
                process_identity_evidence,
            )
        } else {
            let receipt = database.and_then(Path::parent).map(|home| {
                crate::journal::read_process_receipt(&crate::journal::process_receipt_path(
                    home, &id,
                ))
            });
            match receipt {
                Some(Ok(Some(receipt))) if receipt.matches_process(&id, &trace) => {
                    receipt.process_evidence()
                }
                Some(Ok(None))
                    if completed.is_some() || crate::journal::began_before_boot(row.get(5)?) =>
                {
                    ProcessIdentityEvidence::Dead
                }
                // A malformed or mismatched receipt is not absence, even for an old row.
                _ => ProcessIdentityEvidence::Unknown,
            }
        };
        if evidence != ProcessIdentityEvidence::Dead {
            return Err(StoreError::InvalidData(format!(
                "canonical identity migration deferred: Process {id} has {evidence:?} ownership evidence; retry after ordinary settlement or repair the receipt"
            )));
        }
    }
    Ok(())
}
