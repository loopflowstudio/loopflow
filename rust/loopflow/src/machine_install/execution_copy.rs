//! A promotion backup preserves bytes, not ownership of ongoing Task execution.
//! Compare the copy with its recorded baseline before routing to its source.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use sha2::{Digest, Sha256};

const FLOWS: &str = "SELECT id FROM flow_sessions WHERE task_id=?1";
const SESSIONS: &str = "SELECT id FROM agent_sessions WHERE task_id=?1 OR flow_session_id IN (SELECT id FROM flow_sessions WHERE task_id=?1)";

pub(crate) fn task_fingerprints(database: &Path) -> Result<BTreeMap<String, String>> {
    let conn = open(database)?;
    let tasks = conn
        .prepare("SELECT id FROM tasks")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tasks
        .into_iter()
        .map(|task| {
            let hash = fingerprint(&conn, &task)?;
            Ok((task, hash))
        })
        .collect()
}

pub(crate) fn unchanged(database: &Path, task: &str, expected: &str) -> Result<bool> {
    Ok(fingerprint(&open(database)?, task)? == expected)
}

fn open(database: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // One consistent snapshot for all owners and their subordinate history.
    conn.execute_batch("PRAGMA query_only=ON; PRAGMA busy_timeout=5000; BEGIN")?;
    Ok(conn)
}

fn fingerprint(conn: &Connection, task: &str) -> Result<String> {
    let mut digest = Sha256::new();
    for (table, predicate) in [
        ("tasks", "id=?1".to_owned()),
        ("task_events", "task_id=?1".into()),
        ("task_prs", "task_id=?1".into()),
        ("pr_landings", "task_id=?1".into()),
        ("ci_incidents", "task_id=?1".into()),
        ("flow_sessions", "task_id=?1".into()),
        ("flow_events", format!("flow_id IN ({FLOWS})")),
        ("agent_sessions", format!("id IN ({SESSIONS})")),
        ("session_events", format!("task_id=?1 OR session_id IN ({SESSIONS})")),
        ("import_evidence", format!("historical_task_id=?1 OR historical_flow_id IN ({FLOWS}) OR historical_session_id IN ({SESSIONS})")),
        ("execs", format!("id IN (SELECT exec_id FROM session_events WHERE task_id=?1 OR session_id IN ({SESSIONS})) OR id IN (SELECT exec_id FROM flow_events WHERE flow_id IN ({FLOWS})) OR id IN (SELECT driver_exec_id FROM agent_sessions WHERE id IN ({SESSIONS})) OR id IN (SELECT provider_exec_id FROM agent_sessions WHERE id IN ({SESSIONS}))")),
    ] {
        let mut statement = conn.prepare(&format!("SELECT * FROM {table} WHERE {predicate}"))
            .with_context(|| format!("inspect copied Task {task} in {table}"))?;
        feed(&mut digest, table.as_bytes());
        for name in statement.column_names() {
            feed(&mut digest, name.as_bytes());
        }
        let columns = statement.column_count();
        let mut rows = statement.query([task])?;
        let mut hashes = Vec::new();
        while let Some(row) = rows.next()? {
            let mut hash = Sha256::new();
            for column in 0..columns {
                match row.get_ref(column)? {
                    ValueRef::Null => hash.update([0]),
                    ValueRef::Integer(value) => { hash.update([1]); hash.update(value.to_be_bytes()); }
                    ValueRef::Real(value) => { hash.update([2]); hash.update(value.to_bits().to_be_bytes()); }
                    ValueRef::Text(value) => { hash.update([3]); feed(&mut hash, value); }
                    ValueRef::Blob(value) => { hash.update([4]); feed(&mut hash, value); }
                }
            }
            hashes.push(hash.finalize());
        }
        hashes.sort();
        digest.update((hashes.len() as u64).to_be_bytes());
        for hash in hashes { digest.update(hash); }
    }
    Ok(hex::encode(digest.finalize()))
}

fn feed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}
