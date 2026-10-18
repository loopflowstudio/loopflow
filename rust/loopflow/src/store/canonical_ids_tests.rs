use std::fs;
use std::sync::{Arc, Mutex};

use fs2::FileExt;
use rusqlite::{params, Connection};

use super::{_migration_transaction, apply_sqlite, tests::current_draft_sql};
use crate::id::LfProcessId;
use crate::journal::{ProcessIdentityEvidence, ProcessReceipt, PROCESS_RECEIPT_ROOT};
use crate::store::canonical_ids::{preflight, session_fence_file};
use crate::store::sqlite::SqliteStore;
use crate::work::wave::metrics::MetricObservation;

const SESSION: &str = "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const PROCESS: &str = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB";
const TRACE: &str = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC";

fn released() -> (tempfile::TempDir, Connection) {
    let home = tempfile::tempdir().unwrap();
    let conn = Connection::open(home.path().join("loopflow.db")).unwrap();
    let frontier = super::MIGRATIONS
        .iter()
        .position(|migration| super::migration_contains(migration, "canonical_structured_ids"))
        .expect("focused tests run against the materialized release");
    super::apply_set(&conn, &super::MIGRATIONS[..frontier], &mut Vec::new()).unwrap();
    conn.execute_batch(include_str!(
        "../../tests/fixtures/migrations/canonical_ids_authority.sql"
    ))
    .unwrap();
    (home, conn)
}

fn receipt(
    home: &std::path::Path,
    process: &str,
    trace: &str,
    pid: u32,
    started_at: i64,
) -> std::path::PathBuf {
    let root = home.join(PROCESS_RECEIPT_ROOT);
    fs::create_dir_all(&root).unwrap();
    let path = root.join(format!("{process}.json"));
    fs::write(
        &path,
        serde_json::to_vec(&ProcessReceipt {
            schema_version: 1,
            lf_process_id: process.into(),
            trace_id: trace.into(),
            pid,
            started_at,
        })
        .unwrap(),
    )
    .unwrap();
    path
}

#[test]
fn canonical_ids_busy_pre_admission_fence_defers_materialized_release() {
    let (home, conn) = released();
    let fence = session_fence_file(&home.path().join("loopflow.db"), SESSION).unwrap();
    fence.lock_exclusive().unwrap();
    let error = apply_sqlite(&conn).unwrap_err();
    assert!(error.to_string().contains("fence is busy"), "{error}");
    assert!(conn
        .prepare("SELECT fence_locator FROM agent_sessions")
        .is_err());
    assert_eq!(
        conn.query_row("SELECT id FROM agent_sessions", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        SESSION
    );
    drop(fence);
    apply_sqlite(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT fence_locator FROM agent_sessions", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        SESSION
    );
}

#[test]
fn canonical_ids_retained_fence_serializes_stale_waiter_after_identity_change() {
    let (home, conn) = released();
    let waiter = session_fence_file(&home.path().join("loopflow.db"), SESSION).unwrap();
    let (started, waiting) = std::sync::mpsc::sync_channel(0);
    let (acquired, locked) = std::sync::mpsc::sync_channel(1);
    let mut caller = None;
    _migration_transaction(&conn, |conn, guards| {
        preflight(conn, guards)?;
        caller = Some(std::thread::spawn(move || {
            started.send(()).unwrap();
            waiter.lock_exclusive().unwrap();
            acquired.send(()).unwrap();
            waiter
        }));
        waiting.recv().unwrap();
        conn.execute_batch(&current_draft_sql("canonical_structured_ids"))?;
        conn.execute("UPDATE agent_sessions SET id=lower(id)", [])?;
        assert!(locked.try_recv().is_err());
        Ok(())
    })
    .unwrap();
    locked
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let waiter = caller.unwrap().join().unwrap();
    let store = SqliteStore::open_processes_read_only(&home.path().join("loopflow.db")).unwrap();
    assert!(store.session_attachment(SESSION).is_err());
    let stale_insert = conn.execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES(?1,'stale','human',1,0,'/repo')", [SESSION]
    ).unwrap_err();
    assert!(stale_insert
        .to_string()
        .contains("Session identity must be canonical"));
    assert!(store
        .try_lock_session_attachment(&SESSION.to_ascii_lowercase())
        .unwrap()
        .is_none());
    drop(waiter);
    assert!(store
        .try_lock_session_attachment(&SESSION.to_ascii_lowercase())
        .unwrap()
        .is_some());
}

#[test]
fn canonical_ids_receipt_failure_stays_unknown_until_repaired() {
    let (home, conn) = released();
    let path = receipt(home.path(), PROCESS, TRACE, std::process::id(), 1);
    for bytes in [
        b"{broken".as_slice(),
        br#"{"schema_version":2,"exec_id":"x","trace_id":"y","pid":1,"started_at":1}"#,
        br#"{"schema_version":1,"exec_id":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","trace_id":"CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC","pid":1,"started_at":1}"#,
        br#"{"schema_version":1,"exec_id":"BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB","trace_id":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","pid":1,"started_at":1}"#,
    ] {
        fs::write(&path, bytes).unwrap();
        let error = apply_sqlite(&conn).unwrap_err();
        assert!(
            error.to_string().contains("Unknown ownership evidence"),
            "{error}"
        );
        assert!(conn
            .prepare("SELECT receipt_locator FROM processes")
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(apply_sqlite(&conn)
        .unwrap_err()
        .to_string()
        .contains("Unknown ownership evidence"));
    fs::remove_dir(&path).unwrap();
    receipt(home.path(), PROCESS, TRACE, std::process::id(), 1);
    apply_sqlite(&conn).unwrap();
}

#[test]
fn canonical_ids_live_receipt_defers_then_normal_settlement_allows_upgrade() {
    let (home, conn) = released();
    let pid = std::process::id();
    let started_at = crate::journal::process_started_at(pid).unwrap().unwrap();
    let path = receipt(home.path(), PROCESS, TRACE, pid, started_at);
    conn.execute(
        "UPDATE processes SET completed_at=NULL,started_at=?1",
        [started_at],
    )
    .unwrap();
    let error = apply_sqlite(&conn).unwrap_err();
    assert!(
        error.to_string().contains("Live ownership evidence"),
        "{error}"
    );
    conn.execute("UPDATE processes SET completed_at=?1", [started_at + 1])
        .unwrap();
    // Normal process shutdown removes its receipt after recording completion.
    fs::remove_file(path).unwrap();
    apply_sqlite(&conn).unwrap();
}

#[test]
fn canonical_ids_retained_receipt_and_trace_locator_survive_identity_change() {
    let (home, conn) = released();
    let path = receipt(home.path(), PROCESS, TRACE, std::process::id(), 1);
    let original = fs::read(&path).unwrap();
    _migration_transaction(&conn, |conn, guards| {
        preflight(conn, guards)?;
        conn.execute_batch(&current_draft_sql("canonical_structured_ids"))?;
        conn.execute(
            "UPDATE processes SET id=lower(id),trace_id=lower(trace_id)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let store = SqliteStore::open_processes_read_only(&home.path().join("loopflow.db")).unwrap();
    let id = LfProcessId::parse(&PROCESS.to_ascii_lowercase()).unwrap();
    let record = store.process(&id).unwrap().unwrap();
    let retained_path = store.process_receipt_path(&id).unwrap().unwrap();
    assert_eq!(retained_path.file_name(), path.file_name());
    assert_eq!(
        retained_path.canonicalize().unwrap(),
        path.canonicalize().unwrap()
    );
    let retained = crate::journal::read_process_receipt(&retained_path)
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::journal::recorded_process_evidence(&record, Some(&[retained]), |pid, start| {
            assert_eq!((pid, start), (std::process::id(), 1));
            ProcessIdentityEvidence::Dead
        }),
        ProcessIdentityEvidence::Dead
    );
    assert_eq!(
        store
            .journal_trace_locator(&record.trace_id)
            .unwrap()
            .unwrap(),
        TRACE
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    conn.execute(
        "UPDATE processes SET started_at=?1,completed_at=NULL",
        [time::OffsetDateTime::now_utc().unix_timestamp()],
    )
    .unwrap();
    assert_eq!(
        crate::journal::process_evidence(&store, &id),
        ProcessIdentityEvidence::Dead
    );
    assert!(conn
        .execute("UPDATE processes SET receipt_locator='other'", [])
        .is_err());
    let child = LfProcessId::new();
    conn.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?2,3)",
        params![child, record.trace_id],
    )
    .unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT journal_trace_locator FROM processes WHERE id=?1",
            [&child],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        TRACE
    );
}

#[test]
fn canonical_ids_pruning_uses_retained_receipt_owner_and_requires_completion() {
    let (home, conn) = released();
    let path = receipt(home.path(), PROCESS, TRACE, std::process::id(), 1);
    _migration_transaction(&conn, |conn, guards| {
        preflight(conn, guards)?;
        conn.execute_batch(&current_draft_sql("canonical_structured_ids"))?;
        conn.execute(
            "UPDATE processes SET id=lower(id),trace_id=lower(trace_id),completed_at=NULL",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        crate::journal::prune_process_receipts_at(home.path(), &[std::process::id()]).unwrap(),
        0
    );
    assert!(path.exists());
    conn.execute("UPDATE processes SET completed_at=2", [])
        .unwrap();
    assert_eq!(
        crate::journal::prune_process_receipts_at(home.path(), &[std::process::id()]).unwrap(),
        1
    );
    assert!(!path.exists());
}

#[test]
fn canonical_ids_changed_claim_waits_for_canonical_live_caller() {
    let (home, conn) = released();
    let pid = std::process::id();
    let started_at = crate::journal::process_started_at(pid).unwrap().unwrap();
    let process = PROCESS.to_ascii_lowercase();
    let trace = TRACE.to_ascii_lowercase();
    let session = SESSION.to_ascii_lowercase();
    let agent = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    conn.execute("UPDATE agent_sessions SET id=?1", [&session])
        .unwrap();
    conn.execute(
        "UPDATE processes SET id=?1,trace_id=?2,started_at=?3,completed_at=NULL",
        params![process, trace, started_at],
    )
    .unwrap();
    conn.execute("INSERT INTO processes(id,trace_id,kind,parent_lf_process_id,attached_lf_process_id,attachment_token,agent_session_id,started_at,completed_at)
        VALUES(?1,?2,'agent',?3,?3,'EEEEEEEE-EEEE-4EEE-8EEE-EEEEEEEEEEEE',?4,1,2)", params![agent,trace,process,session]).unwrap();
    conn.execute("UPDATE agent_sessions SET agent_process_id=?1", [agent])
        .unwrap();
    let path = receipt(home.path(), &process, &trace, pid, started_at);
    let error = apply_sqlite(&conn).unwrap_err();
    assert!(
        error
            .to_string()
            .contains(&format!("Process {process} has Live")),
        "{error}"
    );
    conn.execute(
        "UPDATE processes SET completed_at=?1 WHERE id=?2",
        params![started_at + 1, process],
    )
    .unwrap();
    fs::remove_file(path).unwrap();
    apply_sqlite(&conn).unwrap();
}

#[test]
fn canonical_ids_canonical_live_claim_does_not_defer_upgrade() {
    let (home, conn) = released();
    let pid = std::process::id();
    let started_at = crate::journal::process_started_at(pid).unwrap().unwrap();
    conn.execute("UPDATE agent_sessions SET id=lower(id)", [])
        .unwrap();
    conn.execute("UPDATE processes SET id=lower(id),trace_id=lower(trace_id),pid=?1,started_at=?2,completed_at=NULL", params![pid,started_at]).unwrap();
    let agent = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    let token = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    let session = SESSION.to_ascii_lowercase();
    conn.execute("INSERT INTO processes(id,trace_id,kind,parent_lf_process_id,attached_lf_process_id,attachment_token,agent_session_id,pid,os_started_at,started_at)
        VALUES(?1,?2,'agent',?3,?3,?4,?5,?6,?7,?7)",
        params![agent,TRACE.to_ascii_lowercase(),PROCESS.to_ascii_lowercase(),token,session,pid,started_at]).unwrap();
    conn.execute(
        "UPDATE agent_sessions SET agent_process_id=?1,provider_thread='Native:UPPER'",
        [agent],
    )
    .unwrap();
    let store = home.path().join("loopflow.db");
    let reader = SqliteStore::open_processes_read_only(&store).unwrap();
    let original_claim = reader.session_attachment(&session).unwrap().unwrap();
    let fence = session_fence_file(&store, &SESSION.to_ascii_lowercase()).unwrap();
    FileExt::lock_shared(&fence).unwrap();
    apply_sqlite(&conn).unwrap();
    assert_eq!(
        conn.query_row("SELECT pid FROM processes WHERE kind='agent'", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        pid
    );
    assert!(session_fence_file(&store, &SESSION.to_ascii_lowercase())
        .unwrap()
        .try_lock_exclusive()
        .is_err());
    let store = SqliteStore::open_processes_read_only(&store).unwrap();
    let claim = store.session_attachment(&session).unwrap().unwrap();
    assert_eq!(claim, original_claim);
    assert_eq!(claim.token.as_str(), token);
    assert_eq!(claim.agent_process_id.as_str(), agent);
    assert_eq!(
        store.session_thread(&session).unwrap().unwrap().as_str(),
        "Native:UPPER"
    );
    // Exercise the production dispatch boundary with the original live claim.
    assert_eq!(
        store
            .with_session_dispatch(&session, &claim, || Ok("sent"))
            .unwrap(),
        "sent"
    );
}

#[test]
fn canonical_ids_conflicting_trace_directories_roll_back_the_release() {
    let (home, conn) = released();
    let other = LfProcessId::new();
    let lower = TRACE.to_ascii_lowercase();
    conn.execute(
        "INSERT INTO processes(id,trace_id,started_at,completed_at) VALUES(?1,?2,1,2)",
        params![other, lower],
    )
    .unwrap();
    let before: i64 = conn
        .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    let error = apply_sqlite(&conn).unwrap_err();
    let error = error.to_string();
    assert!(error.contains("Trace journal locator"), "{error}");
    assert!(error.contains(TRACE) && error.contains(&lower), "{error}");
    assert_eq!(
        conn.query_row("SELECT count(*) FROM schema_migrations", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        before
    );
    assert!(conn
        .prepare("SELECT receipt_locator FROM processes")
        .is_err());
    assert_eq!(
        conn.query_row(
            "SELECT trace_id FROM processes WHERE id=?1",
            [PROCESS],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        TRACE
    );
    assert_eq!(
        conn.query_row(
            "SELECT trace_id FROM processes WHERE id=?1",
            [&other],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        lower
    );
    // Rollback releases every acquired Session fence, including the first one.
    session_fence_file(&home.path().join("loopflow.db"), SESSION)
        .unwrap()
        .try_lock_exclusive()
        .unwrap();
}

#[test]
fn canonical_ids_partial_preflight_retains_guards_until_rollback() {
    let (home, conn) = released();
    let path = receipt(home.path(), PROCESS, TRACE, std::process::id(), 1);
    fs::write(path, "{broken").unwrap();
    let fence = session_fence_file(&home.path().join("loopflow.db"), SESSION).unwrap();
    let rollback_fence = fence.try_clone().unwrap();
    let rollback_seen = Arc::new(Mutex::new(false));
    let seen = rollback_seen.clone();
    conn.rollback_hook(Some(move || {
        *seen.lock().unwrap() = rollback_fence
            .try_lock_exclusive()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::WouldBlock);
    }))
    .unwrap();
    let error = _migration_transaction(&conn, |conn, guards| {
        conn.execute("UPDATE agent_sessions SET title='rolled back'", [])?;
        preflight(conn, guards)
    })
    .unwrap_err();
    assert!(error.to_string().contains("Unknown ownership evidence"));
    assert!(*rollback_seen.lock().unwrap());
    assert_eq!(
        conn.query_row("SELECT title FROM agent_sessions", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "Retained Session"
    );
    fence.try_lock_exclusive().unwrap();
}

#[test]
fn canonical_ids_failed_commit_retains_guards_through_rollback() {
    let (home, conn) = released();
    let fence = session_fence_file(&home.path().join("loopflow.db"), SESSION).unwrap();
    let commit_fence = fence.try_clone().unwrap();
    let rollback_fence = fence.try_clone().unwrap();
    let rollback_seen = Arc::new(Mutex::new(false));
    let seen = rollback_seen.clone();
    let commit_seen = Arc::new(Mutex::new(false));
    let committing = commit_seen.clone();
    conn.commit_hook(Some(move || {
        *committing.lock().unwrap() = commit_fence
            .try_lock_exclusive()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::WouldBlock);
        true // SQLite rejects this commit and rolls back while guards remain held.
    }))
    .unwrap();
    conn.rollback_hook(Some(move || {
        *seen.lock().unwrap() = rollback_fence
            .try_lock_exclusive()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::WouldBlock);
    }))
    .unwrap();
    assert!(_migration_transaction(&conn, |conn, guards| {
        preflight(conn, guards)?;
        conn.execute_batch(&current_draft_sql("canonical_structured_ids"))?;
        Ok(())
    })
    .is_err());
    assert!(*commit_seen.lock().unwrap());
    assert!(*rollback_seen.lock().unwrap());
    assert!(conn
        .prepare("SELECT fence_locator FROM agent_sessions")
        .is_err());
    fence.try_lock_exclusive().unwrap();
}

#[test]
fn canonical_ids_metric_payload_case_changes_its_content_identity() {
    let mut payload = serde_json::json!({
        "kind": "unavailable",
        "identity": {"wave_id": TRACE, "metric_id": "Case-Sensitive-Metric"},
        "contract_revision": "retained-revision",
        "instrument": "retained-producer",
        "observation_id": "",
        "source_as_of": "2026-10-17T00:00:00Z",
        "reason": "retained evidence"
    });
    let original: MetricObservation = serde_json::from_value(payload.clone()).unwrap();
    let original_id = original.expected_observation_id().unwrap();
    payload["observation_id"] = original_id.clone().into();
    let original_bytes = serde_json::to_vec(&payload).unwrap();
    payload["identity"]["wave_id"] = TRACE.to_ascii_lowercase().into();
    let canonical: MetricObservation = serde_json::from_value(payload).unwrap();
    assert_ne!(canonical.expected_observation_id().unwrap(), original_id);
    let retained: MetricObservation = serde_json::from_slice(&original_bytes).unwrap();
    assert_eq!(retained.expected_observation_id().unwrap(), original_id);
}
