//! Scratch reproduction for ENG-7. Not a permanent test yet — this exists to
//! name the cause before any fix is designed.

use loopflow::store::sqlite::SqliteStore;
use loopflow::store::RunEventRow;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn row(run: &str, seq: i64) -> RunEventRow {
    RunEventRow {
        run_id: run.to_string(),
        process_id: format!("p_{run}"),
        parent_process_id: None,
        seq,
        ts: 1_700_000_000,
        repo: Some("repo".into()),
        worktree: Some("/tmp/wt".into()),
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

/// Each writer opens its OWN connection, exactly as `journal::open_ledger()`
/// does per run event. Separate connections contend through the same file
/// locks whether or not they share a process, so this is a faithful stand-in
/// for the observed 51-process fleet.
#[test]
fn fanout_writers_lose_receipts_to_busy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    // Materialize schema once.
    drop(SqliteStore::new(&path).unwrap());

    const WRITERS: usize = 51;
    const EVENTS: usize = 100;

    let failures = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(WRITERS));
    let mut handles = Vec::new();
    for w in 0..WRITERS {
        let path: std::path::PathBuf = path.clone();
        let failures = failures.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            let store = SqliteStore::new(Path::new(&path)).unwrap();
            barrier.wait();
            for e in 0..EVENTS {
                if let Err(err) = store.insert_run_event(&row(&format!("run_{w}"), e as i64)) {
                    failures.fetch_add(1, Ordering::Relaxed);
                    eprintln!("writer {w} event {e}: {err}");
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let lost = failures.load(Ordering::Relaxed);
    eprintln!("lost {lost} of {} receipts", WRITERS * EVENTS);
    assert_eq!(lost, 0, "receipts lost to contention");
}

/// `journal::open_ledger()` opens a fresh connection per run event, so the
/// fleet's real pattern is an open-storm, not a write-storm.
#[test]
fn open_storm_at_fleet_fanout() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    drop(SqliteStore::new(&path).unwrap());

    const WRITERS: usize = 51;
    const EVENTS: usize = 20;

    let failures = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(WRITERS));
    let mut handles = Vec::new();
    for w in 0..WRITERS {
        let path: std::path::PathBuf = path.clone();
        let failures = failures.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            for e in 0..EVENTS {
                // Open per event, exactly as the journal does.
                match SqliteStore::new(Path::new(&path)) {
                    Ok(store) => {
                        if let Err(err) = store.insert_run_event(&row(&format!("run_{w}"), e as i64))
                        {
                            failures.fetch_add(1, Ordering::Relaxed);
                            eprintln!("writer {w} event {e} insert: {err}");
                        }
                    }
                    Err(err) => {
                        failures.fetch_add(1, Ordering::Relaxed);
                        eprintln!("writer {w} event {e} open: {err}");
                    }
                }
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    let lost = failures.load(Ordering::Relaxed);
    eprintln!("open-storm lost {lost} of {} receipts", WRITERS * EVENTS);
    assert_eq!(lost, 0, "receipts lost to contention");
}
