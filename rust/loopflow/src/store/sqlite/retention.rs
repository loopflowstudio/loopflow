//! Retention: the store keeps facts, and streams stay in files.
//!
//! Captures write only Session facts, and native usage keeps one row per
//! turn; this prune merges native usage an earlier lf recorded per receipt.
//! It closes Processes whose OS process is gone without recording its exit,
//! then forgets finished Processes nothing needs, as exit does. Each step is
//! one short transaction on its own connection, so live writers wait well
//! under their busy timeout.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::journal::{OsProcess, ProcessIdentityEvidence};
use crate::session_record::{late_usage_key, merge_native_usage, native_usage_key};
use crate::store::{StoreError, StoreResult};

const PROCESS_BATCH: i64 = 10_000;
/// Every live writer waits this long; each prune transaction stays well under.
const BUSY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionReport {
    pub usage_rows: u64,
    pub closed_processes: u64,
    pub forgotten_processes: u64,
    pub vacuumed: bool,
}

/// Prune `database` once. Cursor files beside it hold the last row each pass
/// visited, so a pass after the first reads only new rows and a draft-build
/// copy keeps its own.
///
/// # Errors
/// When the store cannot be opened or a prune transaction fails.
pub(crate) fn prune(database: &Path, now: i64) -> StoreResult<RetentionReport> {
    let conn = Connection::open(database)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // Nothing references the rows deleted here (see each predicate), and
    // per-row foreign-key probes would scan session_events for each delete.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    // The capture pass this cursor followed ended when migration converted
    // every capture to facts.
    let _ = std::fs::remove_file(Cursor::beside(database, "retention").0);
    let usage_rows = compact_usage(&conn, &Cursor::beside(database, "usage"))?;
    let home = database.parent().unwrap_or(Path::new("."));
    let closed_processes = close_dead_processes(&conn, home, now)?;
    Ok(RetentionReport {
        usage_rows,
        closed_processes,
        forgotten_processes: forget_processes(&conn)?,
        vacuumed: reclaim(&conn)?,
    })
}

/// The last row a pass visited, in a file beside the database, so a later pass
/// reads only newer rows and a draft-build copy keeps its own.
struct Cursor(std::path::PathBuf);

impl Cursor {
    fn beside(database: &Path, name: &str) -> Self {
        let mut path = database.as_os_str().to_owned();
        path.push(format!(".{name}-cursor"));
        Self(path.into())
    }

    fn read(&self) -> i64 {
        std::fs::read_to_string(&self.0)
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0)
    }

    fn write(&self, seq: i64) -> StoreResult<()> {
        std::fs::write(&self.0, seq.to_string())
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }
}

/// Merge native usage recorded as one row per receipt into one row per turn,
/// as the store now writes it. Returns the rows removed.
fn compact_usage(conn: &Connection, cursor: &Cursor) -> StoreResult<u64> {
    let after = cursor.read();
    let last: Option<i64> =
        conn.query_row("SELECT max(seq) FROM session_events", [], |row| row.get(0))?;
    let Some(last) = last.filter(|last| *last > after) else {
        return Ok(0);
    };
    let turns = conn
        .prepare(
            "SELECT DISTINCT session_id,provider_thread,provider_turn FROM session_events
             WHERE seq>?1 AND seq<=?2 AND kind='usage'",
        )?
        .query_map([after, last], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut removed = 0;
    for (session, thread, turn) in turns {
        removed += compact_native_turn(conn, &session, &thread, &turn)?;
    }
    cursor.write(last)?;
    Ok(removed)
}

/// One row per native usage key in the turn, at its latest receipt's position.
/// One usage key's receipts merged into its last row; the earlier rows go.
struct UsageMerge {
    removed: Vec<i64>,
    kept: i64,
    key: String,
    payload: String,
}

fn compact_native_turn(
    conn: &Connection,
    session: &str,
    thread: &str,
    turn: &str,
) -> StoreResult<u64> {
    if native_turn_merges(conn, session, thread, turn)?.is_empty() {
        return Ok(0);
    }
    // A deferred transaction that has read cannot wait for the write lock:
    // SQLite returns busy at once if any live writer holds it. Take the lock
    // first, then plan again under it.
    let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let mut removed = 0;
    for merge in native_turn_merges(&tx, session, thread, turn)? {
        for seq in &merge.removed {
            removed += tx.execute("DELETE FROM session_events WHERE seq=?1", [seq])? as u64;
        }
        tx.execute(
            "UPDATE session_events SET receipt_key=?2,payload=?3 WHERE seq=?1",
            params![merge.kept, merge.key, merge.payload],
        )?;
    }
    tx.commit()?;
    Ok(removed)
}

fn native_turn_merges(
    conn: &Connection,
    session: &str,
    thread: &str,
    turn: &str,
) -> StoreResult<Vec<UsageMerge>> {
    let rows = conn
        .prepare(
            "SELECT seq,receipt_key,payload FROM session_events
             WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3 AND kind='usage' ORDER BY seq",
        )?
        .query_map([session, thread, turn], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let starts = conn
        .prepare(
            "SELECT seq FROM session_events WHERE session_id=?1 AND provider_thread=?2
             AND provider_turn!=?3 AND kind='started' ORDER BY seq",
        )?
        .query_map([session, thread, turn], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut turn_rows = BTreeMap::<String, i64>::new();
    let mut keys = BTreeMap::<String, Vec<(i64, String, serde_json::Value)>>::new();
    for (seq, receipt, payload) in rows {
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&payload) else {
            continue;
        };
        let base = native_usage_key(&payload);
        let started = starts.iter().rev().find(|start| **start < seq).copied();
        let key = match (turn_rows.get(&base), started) {
            (Some(last), Some(started)) if started > *last => late_usage_key(&base, started),
            _ => {
                turn_rows.insert(base.clone(), seq);
                base
            }
        };
        keys.entry(key).or_default().push((seq, receipt, payload));
    }
    let mut merges = Vec::new();
    for (key, receipts) in keys {
        if let [(_, receipt, _)] = receipts.as_slice() {
            if *receipt == key {
                continue;
            }
        }
        let merged = receipts
            .iter()
            .map(|(_, _, payload)| payload.clone())
            .reduce(|saved, later| merge_native_usage(&saved, &later))
            .expect("a key has a receipt");
        let (last, earlier) = receipts.split_last().expect("a key has a receipt");
        merges.push(UsageMerge {
            removed: earlier.iter().map(|(seq, ..)| *seq).collect(),
            kept: last.0,
            key,
            payload: serde_json::to_string(&merged)?,
        });
    }
    Ok(merges)
}

fn close_dead_processes(conn: &Connection, home: &Path, now: i64) -> StoreResult<u64> {
    let columns = super::processes::PROCESS_SELECT
        .strip_suffix(" FROM processes e")
        .expect("Process select ends with its table");
    let unfinished = conn
        .prepare(&format!(
            "{columns},e.receipt_locator FROM processes e WHERE e.completed_at IS NULL"
        ))?
        .query_map([], |row| {
            Ok((
                super::processes::read_process(row)?,
                row.get::<_, Option<String>>("receipt_locator")?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let booted_at = crate::journal::machine_booted_at();
    let mut closed = 0;
    for (process, locator) in unfinished {
        let receipt = match locator {
            Some(locator) => crate::journal::read_process_receipt(
                &crate::journal::process_receipt_path(home, &locator),
            ),
            None => Ok(None),
        };
        let evidence = crate::journal::recorded_process_evidence(
            &process,
            &receipt,
            crate::journal::process_identity_evidence,
        );
        // No process holds the PID at all, so the recorded one has exited.
        let gone = evidence == ProcessIdentityEvidence::Dead
            || (evidence == ProcessIdentityEvidence::Unknown
                && process
                    .pid
                    .is_some_and(|pid| matches!(OsProcess::read(pid), Ok(None))));
        if !gone {
            continue;
        }
        let at = booted_at
            .filter(|booted_at| process.started_at < *booted_at)
            .unwrap_or(now);
        closed += conn.execute(
            "UPDATE processes SET completed_at=?2,
                 spawn_state=CASE WHEN kind!='agent' THEN spawn_state
                     WHEN spawn_state='reserved' AND pid IS NULL THEN 'spawn_failed' ELSE 'exited' END,
                 endpoint=NULL
             WHERE id=?1 AND completed_at IS NULL AND pid IS ?3 AND os_started_at IS ?4",
            params![process.id, at, process.pid, process.os_started_at],
        )? as u64;
    }
    Ok(closed)
}

/// Finished Processes nothing needs ([`super::processes::FORGETTABLE`]): those
/// crash-closed above, and those a finished parent left behind when it exited
/// before them. Exit forgets the rest as it happens.
fn forget_processes(conn: &Connection) -> StoreResult<u64> {
    let forgettable = super::processes::FORGETTABLE;
    let mut forgotten = 0;
    loop {
        let deleted = conn.execute(
            &format!(
                "DELETE FROM processes WHERE id IN (
                     SELECT p.id FROM processes p WHERE {forgettable} LIMIT ?1)"
            ),
            [PROCESS_BATCH],
        )?;
        forgotten += deleted as u64;
        // A parent becomes forgettable once its last child goes.
        if deleted == 0 {
            return Ok(forgotten);
        }
    }
}

/// Return freed pages to the filesystem. The first reclaim converts the store
/// to incremental auto-vacuum with one VACUUM; later passes free pages in place.
/// A busy store is left for the next pass.
fn reclaim(conn: &Connection) -> StoreResult<bool> {
    let pages: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let free: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
    let mode: i64 = conn.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
    if mode == 2 {
        conn.execute_batch("PRAGMA incremental_vacuum;")?;
        return Ok(false);
    }
    if free * 2 <= pages {
        return Ok(false);
    }
    match conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;") {
        Ok(()) => Ok(true),
        Err(error) if is_busy(&error) => {
            tracing::info!(%error, "store reclaim deferred: the store is busy");
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::prune;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn prune_merges_per_receipt_usage_while_a_live_writer_holds_the_lock() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let conn = Connection::open(&path).unwrap();
        for (receipt, total) in [("r1", 10), ("r2", 20)] {
            let counts = serde_json::json!({"inputTokens":total,"outputTokens":0});
            conn.execute(
                "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
                 VALUES('conversation','thread','a','usage',?1,0,?2)",
                rusqlite::params![receipt, serde_json::json!({"total":counts,"last":counts}).to_string()],
            )
            .unwrap();
        }

        let (locked, held) = std::sync::mpsc::channel();
        let writer = {
            let path = path.clone();
            std::thread::spawn(move || {
                let conn = Connection::open(path).unwrap();
                conn.execute_batch("BEGIN IMMEDIATE").unwrap();
                locked.send(()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(300));
                conn.execute_batch("COMMIT").unwrap();
            })
        };
        held.recv().unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        assert_eq!(prune(&path, now).unwrap().usage_rows, 1);
        writer.join().unwrap();

        let rows: Vec<(String, String)> = conn
            .prepare("SELECT receipt_key,payload FROM session_events WHERE kind='usage'")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "");
        assert!(rows[0].1.contains("20"));
        assert!(home.path().join("store.db.usage-cursor").exists());
    }

    #[test]
    fn prune_forgets_unneeded_processes_and_retires_the_capture_cursor() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        SqliteStore::open_ephemeral(&path).unwrap();
        let cursor = home.path().join("store.db.retention-cursor");
        std::fs::write(&cursor, "7").unwrap();

        let now = time::OffsetDateTime::now_utc().unix_timestamp() + 2 * 86_400;
        let conn = Connection::open(&path).unwrap();
        let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
        let process = |n: u8, parent: Option<u8>, finished: bool, via_agent: bool| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,started_at,completed_at,outcome,via_agent)
                 VALUES(?1,?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    id(n),
                    parent.map(id),
                    now,
                    finished.then_some(now),
                    finished.then_some("succeeded"),
                    via_agent
                ],
            )
            .unwrap();
        };
        // Finished long ago or just now, nothing needs these three.
        process(1, None, true, false);
        process(7, None, true, false);
        process(8, Some(7), true, false);
        // A running child holds its parent; a running parent holds its child.
        process(2, None, true, false);
        process(3, Some(2), false, false);
        process(4, Some(3), true, false);
        process(5, None, true, true);

        assert_eq!(prune(&path, now).unwrap().forgotten_processes, 3);
        assert!(!cursor.exists());

        let left: Vec<String> = conn
            .prepare("SELECT id FROM processes ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(left, [id(2), id(3), id(4), id(5)]);
        let violations: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
    }

    #[test]
    fn prune_closes_dead_processes_and_forgets_those_nothing_needs() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        SqliteStore::open_ephemeral(&path).unwrap();
        let conn = Connection::open(&path).unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        crate::journal::set_test_machine_booted_at(Some(now - 3_600));
        let exited = std::process::Command::new("true")
            .spawn()
            .and_then(|mut child| child.wait().map(|_| child.id()))
            .unwrap();
        let own = std::process::id();
        let own_start = crate::journal::process_started_at(own).unwrap();
        let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
        let process = |n: u8, pid: Option<u32>, os: Option<i64>, at: i64, parent: Option<u8>| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,pid,os_started_at,started_at,parent_lf_process_id)
                 VALUES(?1,?1,?2,?3,?4,?5)",
                rusqlite::params![id(n), pid, os, at, parent.map(id)],
            )
            .unwrap();
        };
        // Crash leftovers: once closed, only the agent-issued one stays.
        process(1, None, None, now - 7_200, None);
        process(2, Some(exited), None, now, None);
        process(3, Some(own), own_start, now, None);
        process(9, Some(exited), None, now, None);
        conn.execute(
            "UPDATE processes SET via_agent=1 WHERE id IN (?1,?2)",
            [id(1), id(2)],
        )
        .unwrap();
        let finished = |n: u8, parent: Option<u8>| {
            process(n, None, None, now, parent);
            conn.execute(
                "UPDATE processes SET completed_at=?2,outcome='succeeded' WHERE id=?1",
                rusqlite::params![id(n), now],
            )
            .unwrap();
        };
        finished(4, None);
        finished(5, None);
        finished(6, Some(3));

        let report = prune(&path, now).unwrap();
        assert_eq!(
            (report.closed_processes, report.forgotten_processes),
            (3, 3)
        );
        let rows: Vec<(String, Option<i64>, Option<String>)> = conn
            .prepare("SELECT id,completed_at,outcome FROM processes ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            rows,
            [
                // A restart bounds the exit; neither exit was observed.
                (id(1), Some(now - 3_600), None),
                (id(2), Some(now), None),
                (id(3), None, None),
                (id(6), Some(now), Some("succeeded".into())),
            ]
        );
    }
}
