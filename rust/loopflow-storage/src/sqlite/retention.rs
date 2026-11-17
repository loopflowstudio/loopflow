//! Retention: the store keeps facts, and streams stay in files.
//!
//! This prune closes Processes whose OS process is gone without recording its exit,
//! then forgets finished Processes nothing needs, as exit does. Each step is
//! one short transaction on its own connection, so live writers wait well
//! under their busy timeout.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::StoreResult;

const PROCESS_BATCH: i64 = 10_000;
/// Every live writer waits this long; each prune transaction stays well under.
const BUSY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionReport {
    pub closed_processes: u64,
    pub forgotten_processes: u64,
    pub vacuumed: bool,
}

/// Prune `database` once.
///
/// # Errors
/// When the store cannot be opened or a prune transaction fails.
pub fn prune(
    database: &Path,
    dead: &[(crate::process::ProcessEntry, Option<String>, i64)],
) -> StoreResult<RetentionReport> {
    let conn = Connection::open(database)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    // Nothing references the rows deleted here (see each predicate), and
    // per-row foreign-key probes would scan session_events for each delete.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    crate::history::attach(&conn, database);
    let closed_processes = close_dead_processes(&conn, dead)?;
    Ok(RetentionReport {
        closed_processes,
        forgotten_processes: forget_processes(&conn)?,
        vacuumed: reclaim(&conn)?,
    })
}

/// Saved identities and locators for observation before pruning.
pub fn unfinished_processes(
    database: &Path,
) -> StoreResult<Vec<(crate::process::ProcessEntry, Option<String>)>> {
    let conn = Connection::open(database)?;
    let unfinished = conn
        .prepare(&format!(
            "{} WHERE e.completed_at IS NULL",
            process_receipt_select()
        ))?
        .query_map([], process_receipt_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(unfinished)
}

fn process_receipt_select() -> String {
    super::processes::PROCESS_SELECT
        .replace(" FROM processes e", ",e.receipt_locator FROM processes e")
}

fn process_receipt_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(crate::process::ProcessEntry, Option<String>)> {
    Ok((
        super::processes::read_process(row)?,
        row.get("receipt_locator")?,
    ))
}

fn close_dead_processes(
    conn: &Connection,
    dead: &[(crate::process::ProcessEntry, Option<String>, i64)],
) -> StoreResult<u64> {
    let mut closed = 0;
    for (process, locator, at) in dead {
        let tx = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
        let current = tx
            .query_row(
                &format!("{} WHERE e.id=?1", process_receipt_select()),
                [process.id()],
                process_receipt_row,
            )
            .optional()?;
        if !current
            .as_ref()
            .is_some_and(|(saved, saved_locator)| saved == process && saved_locator == locator)
        {
            continue;
        }
        let closed_here = tx.execute(
            "UPDATE processes SET completed_at=?2,
                 spawn_state=CASE WHEN kind!='agent' THEN spawn_state
                     WHEN spawn_state='reserved' AND pid IS NULL THEN 'spawn_failed' ELSE 'exited' END,
                 endpoint=NULL
             WHERE id=?1 AND completed_at IS NULL AND pid IS ?3 AND os_started_at IS ?4",
            params![process.id(), at, process.pid(), process.os_started_at()],
        )?;
        if closed_here > 0 {
            super::processes::complete_dead_headless_run_in(&tx, process.id(), *at)?;
        }
        tx.commit()?;
        closed += closed_here as u64;
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

    #[test]
    fn prune_forgets_unneeded_processes() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        crate::test_fixture::open(&path).unwrap();

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

        assert_eq!(prune(&path, &[]).unwrap().forgotten_processes, 3);

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
}

#[cfg(test)]
mod observation_tests {
    use super::{prune, unfinished_processes};

    #[test]
    fn pruning_does_not_close_a_process_changed_after_observation() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let _store = crate::test_fixture::open(&path).unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        conn.execute(
            "INSERT INTO processes(id,trace_id,started_at,via_agent) VALUES(?1,?1,1,1)",
            [id],
        )
        .unwrap();
        let (saved, locator) = unfinished_processes(&path).unwrap().remove(0);
        conn.execute(
            "UPDATE processes SET pid=123,os_started_at=456 WHERE id=?1",
            [id],
        )
        .unwrap();
        assert_eq!(
            prune(&path, &[(saved, locator, 10)])
                .unwrap()
                .closed_processes,
            0
        );
        assert!(conn
            .query_row(
                "SELECT completed_at IS NULL FROM processes WHERE id=?1",
                [id],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
        let (current, locator) = unfinished_processes(&path).unwrap().remove(0);
        assert_eq!(
            prune(&path, &[(current, locator, 10)])
                .unwrap()
                .closed_processes,
            1
        );
    }
}
