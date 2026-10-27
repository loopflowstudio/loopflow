//! Retention: the store keeps facts, and streams stay in files.
//!
//! Captures write only Session facts, and native usage keeps one row per
//! turn; this prune merges native usage an earlier lf recorded per receipt.
//! It closes Processes whose OS process is gone without recording its exit,
//! then expires inspection Processes nothing references, and those of commands
//! lf no longer records. Each step is one short transaction on its own
//! connection, so live writers wait well under their busy timeout.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use clap::{CommandFactory, FromArgMatches};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::journal::{OsProcess, ProcessIdentityEvidence};
use crate::session_record::{late_usage_key, merge_native_usage, native_usage_key};
use crate::store::{StoreError, StoreResult};

/// Finished inspection Processes stay listed this long.
pub(crate) const PROCESS_RETENTION_SECONDS: i64 = 7 * 86_400;
const PROCESS_BATCH: i64 = 10_000;
/// Every live writer waits this long; each prune transaction stays well under.
const BUSY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct RetentionReport {
    pub usage_rows: u64,
    pub closed_processes: u64,
    pub processes: u64,
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
    let mut report = RetentionReport::default();
    // The capture pass this cursor followed ended when migration converted
    // every capture to facts.
    let _ = std::fs::remove_file(Cursor::beside(database, "retention").0);
    report.usage_rows = compact_usage(&conn, &Cursor::beside(database, "usage"))?;
    let home = database.parent().unwrap_or(Path::new("."));
    report.closed_processes = close_dead_processes(&conn, home, now)?;
    report.processes = expire_processes(
        &conn,
        now - PROCESS_RETENTION_SECONDS,
        &Cursor::beside(database, "processes"),
    )?;
    report.vacuumed = reclaim(&conn)?;
    Ok(report)
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
fn compact_native_turn(
    conn: &Connection,
    session: &str,
    thread: &str,
    turn: &str,
) -> StoreResult<u64> {
    let tx = conn.unchecked_transaction()?;
    let rows = tx
        .prepare(
            "SELECT seq,receipt_key,payload FROM session_events
             WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3 AND kind='usage' ORDER BY seq",
        )?
        .query_map([session, thread, turn], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let starts = tx
        .prepare(
            "SELECT seq FROM session_events WHERE session_id=?1 AND provider_thread=?2
             AND provider_turn!=?3 AND kind='started' ORDER BY seq",
        )?
        .query_map([session, thread, turn], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    // As the recorder keys them: a receipt that arrives after the thread's
    // next turn began keeps a row of its own.
    let mut turn_rows = BTreeMap::<String, i64>::new();
    let mut keys = BTreeMap::<String, Vec<(i64, String, serde_json::Value)>>::new();
    for (seq, receipt, payload) in rows {
        // An unreadable receipt stays as it was recorded.
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
    let mut removed = 0;
    for (key, receipts) in keys {
        if let [(_, receipt, _)] = receipts.as_slice() {
            if *receipt == key {
                continue;
            }
        }
        let kept = receipts.last().expect("a key has a receipt").0;
        let merged = receipts
            .iter()
            .map(|(_, _, payload)| payload.clone())
            .reduce(|saved, later| merge_native_usage(&saved, &later))
            .expect("a key has a receipt");
        for (seq, ..) in &receipts[..receipts.len() - 1] {
            removed += tx.execute("DELETE FROM session_events WHERE seq=?1", [seq])? as u64;
        }
        tx.execute(
            "UPDATE session_events SET receipt_key=?2,payload=?3 WHERE seq=?1",
            params![kept, key, serde_json::to_string(&merged)?],
        )?;
    }
    tx.commit()?;
    Ok(removed)
}

/// Close unfinished Processes whose OS process is gone: a restart, SIGKILL, a
/// crash or a closed terminal ends lf without its exit path. As when a dead
/// agent is settled, `completed_at` records when it was found dead and the
/// outcome stays unknown. A machine restart bounds the exit by the boot time.
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

/// Finished `lf` Processes older than `before` that nothing references: no
/// child, attachment, Session event, Graph or step, Workflow or move, Wave
/// activation, CI repair, landing supervisor, Session agent or transport,
/// relocation or steer.
fn expire_processes(conn: &Connection, before: i64, cursor: &Cursor) -> StoreResult<u64> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.referenced_processes;
         CREATE TEMP TABLE referenced_processes(id TEXT PRIMARY KEY) WITHOUT ROWID;
         INSERT OR IGNORE INTO temp.referenced_processes
             SELECT lf_process_id FROM session_events WHERE lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM graph_processes WHERE lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM graph_process_steps WHERE lf_process_id IS NOT NULL
             UNION SELECT graph_lf_process_id FROM graph_process_steps WHERE graph_lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM task_workflows WHERE lf_process_id IS NOT NULL
             UNION SELECT lf_process_id FROM task_workflow_moves WHERE lf_process_id IS NOT NULL
             UNION SELECT project_activation_lf_process_id FROM waves WHERE project_activation_lf_process_id IS NOT NULL
             UNION SELECT repair_lf_process_id FROM ci_incidents WHERE repair_lf_process_id IS NOT NULL
             UNION SELECT supervisor_process_id FROM pr_landings WHERE supervisor_process_id IS NOT NULL
             UNION SELECT agent_process_id FROM agent_sessions WHERE agent_process_id IS NOT NULL
             UNION SELECT transport_lf_process_id FROM agent_sessions WHERE transport_lf_process_id IS NOT NULL
             UNION SELECT process_id FROM relocated_task_processes WHERE process_id IS NOT NULL
             UNION SELECT agent_process_id FROM steer_adapters WHERE agent_process_id IS NOT NULL
             UNION SELECT agent_process_id FROM steer_batches WHERE agent_process_id IS NOT NULL;",
    )?;
    let mut expired = 0;
    loop {
        let deleted = conn.execute(
            "DELETE FROM processes WHERE id IN (
                 SELECT p.id FROM processes p
                 WHERE p.started_at<?1 AND p.kind='lf' AND p.completed_at IS NOT NULL AND p.completed_at<?1
                   AND p.id NOT IN (SELECT id FROM temp.referenced_processes)
                   AND NOT EXISTS(SELECT 1 FROM processes c WHERE c.parent_lf_process_id=p.id)
                   AND NOT EXISTS(SELECT 1 FROM processes a WHERE a.kind='agent' AND a.attached_lf_process_id=p.id)
                 LIMIT ?2)",
            params![before, PROCESS_BATCH],
        )?;
        expired += deleted as u64;
        if (deleted as i64) < PROCESS_BATCH {
            break;
        }
    }
    expired += forget_unrecorded(conn, cursor)?;
    conn.execute_batch("DROP TABLE temp.referenced_processes;")?;
    Ok(expired)
}

/// Finished, unreferenced Processes of commands lf no longer records
/// ([`crate::lf::Cli::records_process`]), at any age. Each row is read once;
/// one still running when its pass came expires with the rest.
fn forget_unrecorded(conn: &Connection, cursor: &Cursor) -> StoreResult<u64> {
    let after = cursor.read();
    let last: Option<i64> =
        conn.query_row("SELECT max(rowid) FROM processes", [], |row| row.get(0))?;
    let Some(last) = last.filter(|last| *last > after) else {
        return Ok(0);
    };
    let candidates = conn
        .prepare(
            "SELECT p.id,p.command FROM processes p
             WHERE p.rowid>?1 AND p.rowid<=?2
               AND p.kind='lf' AND p.completed_at IS NOT NULL AND p.command IS NOT NULL
               AND p.id NOT IN (SELECT id FROM temp.referenced_processes)
               AND NOT EXISTS(SELECT 1 FROM processes c WHERE c.parent_lf_process_id=p.id)
               AND NOT EXISTS(SELECT 1 FROM processes a WHERE a.kind='agent' AND a.attached_lf_process_id=p.id)",
        )?
        .query_map([after, last], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut cli = crate::lf::Cli::command();
    let unrecorded: Vec<String> = candidates
        .into_iter()
        .filter(|(_, command)| {
            serde_json::from_str::<Vec<String>>(command).is_ok_and(|argv| !records(&mut cli, argv))
        })
        .map(|(id, _)| id)
        .collect();
    let mut forgotten = 0;
    for batch in unrecorded.chunks(PROCESS_BATCH as usize) {
        let tx = conn.unchecked_transaction()?;
        let mut delete = tx.prepare("DELETE FROM processes WHERE id=?1")?;
        for id in batch {
            forgotten += delete.execute([id])? as u64;
        }
        drop(delete);
        tx.commit()?;
    }
    cursor.write(last)?;
    Ok(forgotten)
}

/// Whether lf records `argv` today; an argv it no longer parses stays recorded.
fn records(cli: &mut clap::Command, argv: Vec<String>) -> bool {
    let Ok(argv) = crate::lf::navigation::normalize_args(argv) else {
        return true;
    };
    cli.try_get_matches_from_mut(argv)
        .ok()
        .and_then(|matches| crate::lf::Cli::from_arg_matches(&matches).ok())
        .is_none_or(|parsed| parsed.records_process())
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

    use super::{prune, PROCESS_RETENTION_SECONDS};
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn prune_expires_unreferenced_processes_and_retires_the_capture_cursor() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        SqliteStore::open_ephemeral(&path).unwrap();
        let cursor = home.path().join("store.db.retention-cursor");
        std::fs::write(&cursor, "7").unwrap();

        let now = time::OffsetDateTime::now_utc().unix_timestamp() + 2 * 86_400;
        let old = now - PROCESS_RETENTION_SECONDS - 60;
        let conn = Connection::open(&path).unwrap();
        let process = |id: &str, parent: Option<&str>, at: i64| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,parent_lf_process_id,started_at,completed_at,outcome)
                 VALUES(?1,?1,?2,?3,?3,'succeeded')",
                rusqlite::params![id, parent, at],
            )
            .unwrap();
        };
        let id = |n: u8| format!("00000000-0000-4000-8000-0000000000{n:02}");
        process(&id(1), None, old);
        process(&id(2), None, old);
        process(&id(3), Some(&id(2)), now);
        process(&id(4), None, now - 60);

        assert_eq!(prune(&path, now).unwrap().processes, 1);
        assert!(!cursor.exists());

        let left: Vec<String> = conn
            .prepare("SELECT id FROM processes ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(left, [id(2), id(3), id(4)]);
        let violations: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
    }

    #[test]
    fn prune_closes_dead_processes_and_forgets_unrecorded_commands() {
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
        let process = |n: u8, pid: Option<u32>, os: Option<i64>, at: i64, command: &str| {
            conn.execute(
                "INSERT INTO processes(id,trace_id,pid,os_started_at,started_at,command) VALUES(?1,?1,?2,?3,?4,?5)",
                rusqlite::params![id(n), pid, os, at, command],
            )
            .unwrap();
        };
        process(1, None, None, now - 7_200, r#"["lf","task","run"]"#);
        process(2, Some(exited), None, now, r#"["lf","task","run"]"#);
        process(3, Some(own), own_start, now, r#"["lf","task","run"]"#);
        let finished = |n: u8, command: &str| {
            process(n, None, None, now, command);
            conn.execute(
                "UPDATE processes SET completed_at=?2,outcome='succeeded' WHERE id=?1",
                rusqlite::params![id(n), now],
            )
            .unwrap();
        };
        finished(4, r#"["lf","__context-block","--moment","start"]"#);
        finished(5, r#"["lf","task","file","LOO-1","src/lib.rs","--json"]"#);
        finished(6, r#"["lf","task","status","LOO-1"]"#);

        let report = prune(&path, now).unwrap();
        assert_eq!((report.closed_processes, report.processes), (2, 2));
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
