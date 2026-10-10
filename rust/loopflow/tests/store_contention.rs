//! Concurrent local ledger writes stay deterministic at fleet fanout (ENG-7).
//!
//! The open is what contends, not the write, so the probe must open per event
//! as `journal::open_ledger()` does — writes over a live connection lost nothing
//! even at 5100 inserts, while open-per-event lost 426 of 1020 before #1030.
//!
//! Each writer opens its own connection. SQLite locks per connection through
//! file locks and the WAL's shared memory, not per process, so threads here
//! contend exactly as separate processes do.

use loopflow::process::LfProcess;
use loopflow::store::sqlite::SqliteStore;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

/// At least as large as the fleet whose contention killed the provider bodies.
const FLEET: usize = 51;
const EVENTS_PER_WRITER: usize = 20;

fn process() -> LfProcess {
    let ts = 1_700_000_000;
    LfProcess {
        kind: loopflow::process::ProcessKind::Lf,
        agent_session_id: None,
        os_started_at: None,
        lfid: loopflow::id::ProcessLfid::new(),
        pid: None,
        trace_id: loopflow::id::TraceId::new(),
        parent_process_lfid: None,
        via_agent: Some(false),
        caller_session_id: None,
        caller_agent_process_lfid: None,
        command: Some(r#"["lf","flow","telemetry-daily"]"#.into()),
        repo: Some("/src/loopflow".into()),
        cwd: None,
        started_at: ts,
        completed_at: Some(ts),
        outcome: Some("succeeded".into()),
        exit_code: Some(0),
        signal: None,
        error: None,
    }
}

fn count(path: &Path, sql: &str) -> i64 {
    rusqlite::Connection::open(path)
        .unwrap()
        .query_row(sql, [], |row| row.get(0))
        .expect("read back the ledger")
}

/// The row counts are what stop this passing for free: "no write errored" also
/// holds for a ledger that recorded nothing.
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
                let recorded = SqliteStore::new(Path::new(&path))
                    .and_then(|store| store.record_process(&process()));
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
        "writes failed under contention; each one is a lost execution receipt"
    );
    assert_eq!(
        count(&path, "SELECT COUNT(*) FROM processes"),
        expected,
        "the ledger must hold exactly the receipts the fleet requested"
    );
    assert_eq!(
        count(&path, "SELECT COUNT(DISTINCT lfid) FROM processes"),
        expected,
        "no receipt may be recorded twice"
    );
}
