use crate::test_fixture::build;
use std::sync::{Arc, Mutex};

use fs2::FileExt;
use loopflow_model::id::LfProcessId;
use rusqlite::{params, Connection};

use super::{_migration_transaction, apply_pending_migrations};
use crate::canonical_ids::{preflight, session_fence_file};
use crate::metric::MetricObservation;
use crate::sqlite::SqliteStore;
use crate::ProcessIdentityEvidence;

fn observers() -> super::MigrationObservers<'static> {
    super::MigrationObservers {
        export_contract: &|_, _| Ok(()),
        process_evidence: &|_, _| ProcessIdentityEvidence::Dead,
    }
}

fn apply_sqlite(conn: &Connection) -> crate::StoreResult<()> {
    super::apply_sqlite(&build(), conn, &observers())
}

const SESSION: &str = "SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const PROCESS: &str = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB";
const TRACE: &str = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC";

fn released() -> (tempfile::TempDir, Connection) {
    super::canonical_fixture(&observers(), &build())
}

/// Released stores open as experiments only after this build's drafts apply.
fn conversion_sql() -> &'static str {
    super::MIGRATIONS
        .iter()
        .find(|migration| super::migration_contains(migration, "canonical_structured_ids"))
        .expect("materialized conversion release")
        .sql
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
        preflight(conn, guards, &|_, _| ProcessIdentityEvidence::Dead)?;
        caller = Some(std::thread::spawn(move || {
            started.send(()).unwrap();
            waiter.lock_exclusive().unwrap();
            acquired.send(()).unwrap();
            waiter
        }));
        waiting.recv().unwrap();
        conn.execute_batch(conversion_sql())?;
        assert!(locked.try_recv().is_err());
        Ok(())
    })
    .unwrap();
    locked
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let waiter = caller.unwrap().join().unwrap();
    let store =
        SqliteStore::open_read_only(&home.path().join("loopflow.db"), crate::test_fixture::key)
            .unwrap();
    // The store is still at the conversion release's schema, which kept the
    // attachment on the agent's Process row.
    let token: Option<String> = conn
        .query_row(
            "SELECT p.attachment_token FROM agent_sessions s
             LEFT JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1",
            [SESSION.to_ascii_lowercase()],
            |row| row.get(0),
        )
        .unwrap();
    assert!(token.is_none());
    let stale_insert = conn.execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES(?1,'stale','human',1,0,'/repo')", [SESSION]
    ).unwrap_err();
    assert!(stale_insert.to_string().contains("canonical"));
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
        preflight(conn, guards, &|_, _| ProcessIdentityEvidence::Unknown)
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
        preflight(conn, guards, &|_, _| ProcessIdentityEvidence::Dead)?;
        conn.execute_batch(conversion_sql())?;
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

#[test]
fn canonical_ids_wave_collision_preserves_original_metric_evidence() {
    let (_home, conn) = released();
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/migrations/canonical_ids_metrics.sql"
    )))
    .unwrap();
    conn.execute(
        "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'Other Wave','/repo',1)",
        [TRACE.to_ascii_lowercase()],
    )
    .unwrap();
    let before: String = conn.query_row("SELECT json_group_array(json_array(observation_id,wave_id,payload)) FROM metric_observations", [], |r|r.get(0)).unwrap();
    let ledger: i64 = conn
        .query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    let error = apply_sqlite(&conn).unwrap_err().to_string();
    assert!(
        error.contains("canonical identity collision: waves"),
        "{error}"
    );
    assert!(error.contains(TRACE), "{error}");
    assert_eq!(conn.query_row("SELECT json_group_array(json_array(observation_id,wave_id,payload)) FROM metric_observations", [], |r|r.get::<_,String>(0)).unwrap(), before);
    assert_eq!(
        conn.query_row("SELECT count(*) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        ledger
    );
    assert!(conn
        .prepare("SELECT fence_locator FROM agent_sessions")
        .is_err());
}

#[test]
fn canonical_ids_wave_associations_keep_uuid_punctuation() {
    let (_home, conn) = released();
    let spellings = [
        TRACE.to_string(),
        TRACE.replace('-', ""),
        format!("{{{TRACE}}}"),
        format!("URN:UUID:{TRACE}"),
        "Historical-Wave".to_string(),
    ];
    for (index, spelling) in spellings.iter().enumerate() {
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,?2,'/repo',1)",
            params![spelling, format!("Wave {index}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metric_instruments VALUES(?1,'metric','producer',1)",
            [spelling],
        )
        .unwrap();
    }
    super::validate_foreign_keys(&conn).unwrap();
    apply_sqlite(&conn).unwrap();
    for (index, spelling) in spellings.iter().enumerate() {
        let expected = loopflow_model::id::canonical_uuid(spelling);
        let association: String = conn.query_row("SELECT m.wave_id FROM metric_instruments m JOIN waves w ON m.wave_id=w.id WHERE w.name=?1", [format!("Wave {index}")], |r|r.get(0)).unwrap();
        assert_eq!(association, expected);
    }
    assert_eq!(
        conn.query_row("SELECT count(*) FROM waves", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        spellings.len() as i64
    );
}

#[test]
fn canonical_ids_released_work_upgrades_without_replacing_history() {
    let (home, conn) = released();
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/migrations/canonical_ids_work.sql"
    )))
    .unwrap();
    // Corrupt retained evidence is opaque; conversion and later writes never parse it.
    conn.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
         VALUES(?1,'observed','Case-Receipt',2,'not JSON')",
        [SESSION],
    )
    .unwrap();
    apply_sqlite(&conn).unwrap();
    apply_pending_migrations(&observers(), &build(), &conn);
    assert_eq!(
        conn.query_row(
            "SELECT session_id,payload FROM session_events WHERE receipt_key='Case-Receipt'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        (SESSION.to_ascii_lowercase(), "not JSON".to_string())
    );
    assert_eq!(
        conn.query_row("SELECT provider_thread FROM agent_sessions", [], |r| r
            .get::<_, String>(
            0
        ))
        .unwrap(),
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"
    );
    // `ship_it` drops Workflows short of `end`; the next run takes up a graph.
    assert_eq!(
        conn.query_row("SELECT count(*) FROM task_graphs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    // The admitting Machine survives as the Task's recorded ownership entry.
    assert_eq!(
        conn.query_row(
            "SELECT kind,machine_id FROM task_ownership WHERE task_id='task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        (
            "recorded".to_string(),
            "home_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()
        )
    );
    let event: String = conn
        .query_row("SELECT kind_json FROM task_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&event).unwrap()["steer_id"],
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    );
    let store =
        SqliteStore::open_read_only(&home.path().join("loopflow.db"), crate::test_fixture::key)
            .unwrap();
    for selector in [
        "TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "lOo-506",
        "FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF",
        "LF-AAAA",
    ] {
        assert_eq!(
            store
                .resolve_task_id(selector, None)
                .unwrap()
                .unwrap()
                .as_str(),
            "task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
    }
    assert_eq!(
        store.session(SESSION).unwrap().unwrap().id.as_str(),
        "session_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        conn.query_row(
            "SELECT id FROM processes WHERE id=?1",
            [PROCESS.to_ascii_lowercase()],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        PROCESS.to_ascii_lowercase()
    );
    assert!(conn
        .execute("UPDATE task_ownership SET machine_id='other'", [])
        .unwrap_err()
        .to_string()
        .contains("append-only"));
    assert!(conn
        .execute(
            "UPDATE tasks SET external_issue_id=upper(external_issue_id)",
            []
        )
        .unwrap_err()
        .to_string()
        .contains("canonical identity"));
    // References need no trigger: the store's foreign keys match canonical owners exactly.
    conn.execute_batch(
        "PRAGMA foreign_keys=ON;
         INSERT INTO task_graphs(task_id,graph,step,updated_at)
         SELECT id,'{\"name\":\"code\",\"steps\":[]}','end',1 FROM tasks LIMIT 1",
    )
    .unwrap();
    assert!(conn
        .execute("UPDATE task_graphs SET task_id=upper(task_id)", [])
        .unwrap_err()
        .to_string()
        .contains("FOREIGN KEY"));
}

#[test]
fn canonical_ids_planning_key_collision_rolls_back_release() {
    let (_home, conn) = released();
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/migrations/canonical_ids_work.sql"
    )))
    .unwrap();
    conn.execute("INSERT INTO planning_peer_changes(id,record_table,mutation) SELECT lower(id),record_table,mutation FROM planning_peer_changes ORDER BY id LIMIT 1",[]).unwrap();
    let error = apply_sqlite(&conn).unwrap_err().to_string();
    assert!(
        error.contains("canonical identity collision: planning_peer_changes"),
        "{error}"
    );
    assert!(conn
        .prepare("SELECT fence_locator FROM agent_sessions")
        .is_err());
    assert_eq!(
        conn.query_row("SELECT count(*) FROM planning_peer_changes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        3
    );
}

#[test]
fn canonical_ids_unique_linear_claim_and_json_membership_collisions_roll_back() {
    for setup in [
        "INSERT INTO projects(id,wave_id,external_project_id,created_at) SELECT 'proj_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',wave_id,lower(external_project_id),1 FROM projects",
        "UPDATE projects SET planning_teams='[\"AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA\",\"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\"]'",
        "INSERT INTO planning_peer_changes(id,record_table,mutation) SELECT 'CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC',record_table,json_set(mutation,'$.clock',2,'$.parents',json('[]')) FROM planning_peer_changes ORDER BY id LIMIT 1; UPDATE planning_peer_changes SET id=lower(id) WHERE id LIKE 'BBBB%'",
    ] {
        let (_home,conn)=released();
        conn.execute_batch(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/migrations/canonical_ids_work.sql"))).unwrap();
        conn.execute_batch(setup).unwrap();
        let error=apply_sqlite(&conn).unwrap_err().to_string();
        assert!(error.contains("canonical identity collision"),"{error}");
        assert!(conn.prepare("SELECT receipt_locator FROM processes").is_err());
        assert_eq!(conn.query_row("SELECT id FROM tasks",[],|r|r.get::<_,String>(0)).unwrap(),"TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }
}

#[test]
fn canonical_ids_supplied_evidence_keeps_live_and_unknown_ownership() {
    let (_home, conn) = released();
    for evidence in [
        ProcessIdentityEvidence::Live,
        ProcessIdentityEvidence::Unknown,
        ProcessIdentityEvidence::Dead,
    ] {
        let mut guards = Vec::new();
        let result = preflight(&conn, &mut guards, &|_, process| {
            assert!(!process.id.is_empty());
            assert!(!process.trace.is_empty());
            evidence
        });
        if evidence == ProcessIdentityEvidence::Dead {
            result.unwrap();
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains(&format!("{evidence:?} ownership evidence")));
        }
    }
}
