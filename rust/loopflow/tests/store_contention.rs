//! Concurrent local ledger writes stay deterministic at fleet fanout (ENG-7).
//!
//! On 2026-07-16/17 a fleet of ~51 concurrent Loopflow/provider processes killed
//! live Session bodies with `sqlite error: database is locked`, parking W2-284,
//! W2-285 and W2-287 on a manual resume.
//!
//! The cause was not write contention. Writes over a live connection never lost
//! a receipt at that fanout. It was that every `SqliteStore::new` took
//! `BEGIN EXCLUSIVE` and held it across a whole-database `PRAGMA
//! foreign_key_check` whether or not a migration was pending — while
//! `journal::open_ledger()` opens a connection per run event. The fleet
//! serialized behind that lock and writers starved past `busy_timeout`. At
//! fanout 51 that lost 426 of 1020 receipts.
//!
//! PR #1030 fixed it by consulting `requires_migration_sqlite` before opening
//! the migration transaction, and carries the timing-free proof that a current
//! schema takes no write lock (`current_schema_does_not_take_the_database_write_lock`).
//! This is the evidence that fix did not have: the fleet-scale measurement.
//!
//! Each writer opens its own connection. SQLite locks per connection through
//! file locks and the WAL's shared memory, not per process, so separate
//! connections in one process contend exactly as separate processes do — which
//! is what lets a fleet-scale proof run deterministically in one test binary.

use loopflow::store::sqlite::SqliteStore;
use loopflow::store::RunEventRow;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

/// At least as large as the fleet whose contention killed the Session bodies.
const FLEET: usize = 51;
const EVENTS_PER_WRITER: usize = 20;

fn run_event(run: &str, seq: i64) -> RunEventRow {
    RunEventRow {
        run_id: run.to_string(),
        process_id: format!("p_{run}"),
        parent_process_id: None,
        seq,
        ts: 1_700_000_000,
        repo: Some("/repo".into()),
        worktree: Some("/repo/wt".into()),
        wave: Some("infrastructure".into()),
        node: "flow".into(),
        event: "started".into(),
        command: None,
        flow: None,
        skill: None,
        step_index: None,
        error: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cost_usd: None,
        duration_secs: None,
        provider: None,
        model: None,
    }
}

/// Read back through a plain connection rather than a test-only accessor on the
/// store: the ledger's contents are the thing under test, and production code
/// owes tests no seam.
fn count(path: &Path, sql: &str) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row(sql, [], |row| row.get(0))
        .expect("read back the ledger")
}

/// The fleet-scale proof: every requested receipt is recorded exactly once, with
/// a fresh connection per event, at the fanout that produced the incident.
///
/// The row-count assertions are what stop this passing for free. "No write
/// returned an error" is also satisfied by a ledger that recorded nothing, so
/// the exact count and the distinct-key count are the ones that bite.
#[test]
fn every_receipt_at_fleet_fanout_is_recorded_exactly_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    drop(SqliteStore::new(&path).expect("materialize the schema"));

    let lost = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(FLEET));
    let mut writers = Vec::new();
    for writer in 0..FLEET {
        let path = path.clone();
        let lost = lost.clone();
        let barrier = barrier.clone();
        writers.push(std::thread::spawn(move || {
            barrier.wait();
            for seq in 0..EVENTS_PER_WRITER {
                // Open per event, exactly as `journal::open_ledger()` does.
                let recorded = SqliteStore::new(Path::new(&path)).and_then(|store| {
                    store.insert_run_event(&run_event(&format!("run_{writer}"), seq as i64))
                });
                if let Err(error) = recorded {
                    lost.fetch_add(1, Ordering::Relaxed);
                    eprintln!("writer {writer} seq {seq}: {error}");
                }
            }
        }));
    }
    for writer in writers {
        writer.join().unwrap();
    }

    let expected = (FLEET * EVENTS_PER_WRITER) as i64;
    assert_eq!(
        lost.load(Ordering::Relaxed),
        0,
        "writes failed under contention; each one is a lost execution receipt, \
         and enough of them killed live Session bodies"
    );
    assert_eq!(
        count(&path, "SELECT COUNT(*) FROM run_events"),
        expected,
        "the ledger must hold exactly the receipts the fleet requested"
    );
    assert_eq!(
        count(
            &path,
            "SELECT COUNT(*) FROM (SELECT DISTINCT run_id, seq FROM run_events)"
        ),
        expected,
        "no receipt may be recorded twice"
    );
}
