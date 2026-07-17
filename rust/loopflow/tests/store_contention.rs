//! Concurrent local ledger writes must be deterministic (ENG-7).
//!
//! On 2026-07-16/17 a fleet of ~51 concurrent Loopflow/provider processes
//! killed live Session bodies with `sqlite error: database is locked`. The
//! cause was not write contention: it was that every `SqliteStore::new` took
//! `BEGIN EXCLUSIVE` and held it across a whole-database `PRAGMA
//! foreign_key_check` whether or not a migration was pending, while
//! `journal::open_ledger()` opens a connection per run event. The fleet
//! serialized behind that lock and writers starved past `busy_timeout`.
//!
//! Each writer below opens its own connection. SQLite locks per connection
//! through file locks and the WAL's shared memory, not per process, so separate
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
/// store: the ledger's shape is the thing under test, and production code owes
/// tests no seam.
fn count(path: &Path, sql: &str) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row(sql, [], |row| row.get(0))
        .expect("read back the ledger")
}

/// The fleet-scale proof: every requested receipt is recorded exactly once,
/// with a fresh connection per event, at the fanout that produced the incident.
///
/// Guard against passing for free: this fails loudly if the store never
/// materialized, because a ledger that recorded nothing would otherwise satisfy
/// "lost no receipts" trivially.
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
        "writes failed under contention; every one of these is a lost execution receipt"
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
        "a retried write must not record its receipt twice"
    );
}

/// The precise, timing-free regression guard for the named cause.
///
/// A store open against a database with nothing pending must take no exclusive
/// lock. Proven by holding the database's write lock and opening anyway: with
/// the migration transaction gated on a pending-migration read this returns at
/// once, and without the gate the open blocks on `BEGIN EXCLUSIVE` until
/// `busy_timeout` expires and then fails — which is precisely how the fleet
/// starved itself.
///
/// This asserts on the open's *outcome*, not on elapsed time, so it cannot
/// flake under parallel test load.
#[test]
fn opening_a_current_database_takes_no_exclusive_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    drop(SqliteStore::new(&path).expect("materialize the schema"));

    // Hold the write lock for the whole open, as a busy peer in the fleet does.
    let holder = rusqlite::Connection::open(&path).unwrap();
    holder.busy_timeout(std::time::Duration::from_millis(0)).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();

    let opened = SqliteStore::new(&path);

    holder.execute_batch("ROLLBACK").unwrap();
    opened.expect("opening a current database must not wait on the write lock");
}

/// A writer must still record its receipt while a peer holds the write lock,
/// rather than dying of contention alone. The holder releases from another
/// thread, so the write can only land by riding the retry ladder.
#[test]
fn a_write_survives_a_peer_holding_the_write_lock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    let store = SqliteStore::new(&path).expect("materialize the schema");

    let holder = rusqlite::Connection::open(&path).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    holder
        .execute("INSERT INTO run_events (run_id, process_id, seq, ts, node, event) VALUES ('held', 'p', 0, 1, 'flow', 'started')", [])
        .unwrap();

    let released = Arc::new(Barrier::new(2));
    let releaser = released.clone();
    let unlock = std::thread::spawn(move || {
        releaser.wait();
        std::thread::sleep(std::time::Duration::from_millis(80));
        holder.execute_batch("COMMIT").unwrap();
    });

    released.wait();
    store
        .insert_run_event(&run_event("contended", 0))
        .expect("a contended write must ride the retry ladder, not fail the caller");
    unlock.join().unwrap();

    assert_eq!(
        count(
            &path,
            "SELECT COUNT(*) FROM run_events WHERE run_id = 'contended'"
        ),
        1,
        "the contended receipt must be recorded exactly once"
    );
}
