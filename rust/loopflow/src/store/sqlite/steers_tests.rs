use rusqlite::params;

use super::{save_in, SqliteStore};
use crate::durable::Author;
use crate::id::LfProcessId;
use crate::steer::{NativeBoundary, SteerAdapter, SteerContext};
use crate::store::StoreError;

fn rendered(text: &str) -> SteerContext {
    SteerContext {
        text: text.into(),
        excerpts: Vec::new(),
    }
}

const ACTIVATE: NativeBoundary<'static> = NativeBoundary::Activate { source: "startup" };
const CLEAR: NativeBoundary<'static> = NativeBoundary::Select { source: "clear" };

fn fixture() -> (tempfile::TempDir, SqliteStore, SteerAdapter) {
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
    store.test_session("session", &crate::session_record::new_artifact_key());
    let process = LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [process.as_str()],
        )
        .unwrap();
    let claim = store
        .claim_session_attachment("session", None, &process, false)
        .unwrap();
    let adapter = store.reserve_steer_adapter("session", &claim).unwrap();
    store
        .with_provider_context(&adapter, "native", ACTIVATE, || Ok(()))
        .unwrap();
    (home, store, adapter)
}

#[test]
fn saving_a_steer_wakes_tool_hooks() {
    let (home, store, _) = fixture();
    let latest = crate::steer::latest_marker(home.path());
    assert!(!latest.exists());
    store
        .save_session_steer("session", "Named author", "Change direction")
        .unwrap();
    assert!(latest.exists(), "a saved steer must reach gated tool hooks");
}

#[test]
fn emitted_direction_is_not_replayed_and_retry_is_explicit() {
    let (_home, store, adapter) = fixture();
    let steer = store
        .save_session_steer("session", "Named author", "Change direction")
        .unwrap();
    store
        .emit_session_steers(
            &adapter,
            "native",
            "tool:one",
            |_, items, remaining| {
                assert_eq!(items, std::slice::from_ref(&steer));
                assert_eq!(remaining, 0);
                Ok(rendered("payload"))
            },
            |payload| {
                assert_eq!(payload, "payload");
                Ok(())
            },
        )
        .unwrap();
    let page = store.session_steers("session", 0, 32).unwrap();
    assert_eq!(page.items[0].evidence, "emitted");
    assert_eq!(page.pending, 0);
    assert!(page.items[0].payload_hash.is_some());
    store
        .emit_session_steers(
            &adapter,
            "native",
            "compact",
            |_, _, _| panic!("compaction replayed"),
            |_| panic!("compaction wrote"),
        )
        .unwrap();
    store.retry_session_steer("session", &steer.id).unwrap();
    assert_eq!(store.session_steers("session", 0, 32).unwrap().pending, 1);
    store
        .emit_session_steers(
            &adapter,
            "native",
            "tool:two",
            |_, _, _| Ok(rendered("retry")),
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        store.session_steers("session", 0, 32).unwrap().items[0].attempt,
        1
    );
}

#[test]
fn prewrite_failure_releases_but_write_failure_retains_uncertainty() {
    let (_home, store, adapter) = fixture();
    store
        .save_session_steer("session", "Author", "Direction")
        .unwrap();
    assert!(store
        .emit_session_steers(
            &adapter,
            "native",
            "first",
            |_, _, _| Err(StoreError::InvalidData("source disk full".into())),
            |_| panic!("render failure wrote")
        )
        .is_err());
    assert_eq!(store.session_steers("session", 0, 32).unwrap().pending, 1);
    assert!(store
        .emit_session_steers(
            &adapter,
            "native",
            "first",
            |_, _, _| Ok(rendered("payload")),
            |_| Err(StoreError::InvalidData("broken pipe".into()))
        )
        .is_err());
    assert_eq!(
        store.session_steers("session", 0, 32).unwrap().items[0].evidence,
        "uncertain"
    );
    store
        .emit_session_steers(
            &adapter,
            "native",
            "next",
            |_, _, _| panic!("uncertain replay"),
            |_| panic!("uncertain write"),
        )
        .unwrap();
}

#[test]
fn concurrent_boundaries_reserve_independent_bounded_pages() {
    let (_home, store, adapter) = fixture();
    for n in 0..20 {
        store
            .save_session_steer("session", "Author", &format!("Direction {n}"))
            .unwrap();
    }
    let other = store.clone();
    let another = adapter.clone();
    store
        .emit_session_steers(
            &adapter,
            "native",
            "one",
            |_, first, remaining| {
                assert_eq!(first.len(), 8);
                assert_eq!(remaining, 12);
                other
                    .emit_session_steers(
                        &another,
                        "native",
                        "two",
                        |_, second, pending| {
                            assert_eq!(second.len(), 8);
                            assert_eq!(pending, 4);
                            assert!(second
                                .iter()
                                .all(|row| !first.iter().any(|item| item.id == row.id)));
                            Ok(rendered("second"))
                        },
                        |_| Ok(()),
                    )
                    .unwrap();
                // A status read cannot cancel the first callback's live reservation.
                assert_eq!(other.session_steers("session", 0, 32).unwrap().pending, 4);
                Ok(rendered("first"))
            },
            |_| Ok(()),
        )
        .unwrap();
    store
        .emit_session_steers(
            &adapter,
            "native",
            "one",
            |_, _, _| panic!("duplicate boundary selected the pending page"),
            |_| panic!("duplicate write"),
        )
        .unwrap();
    let first = store.session_steers("session", 0, 3).unwrap();
    assert_eq!(first.items.len(), 3);
    let next = store
        .session_steers("session", first.next_after.unwrap(), 3)
        .unwrap();
    assert!(next.items[0].steer.sequence > first.items[2].steer.sequence);
}

#[test]
fn only_abandoned_prewrite_reservations_return_to_pending() {
    let (_home, store, adapter) = fixture();
    let direction = store
        .save_session_steer("session", "Author", "Direction")
        .unwrap();
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.emit_session_steers(
            &adapter,
            "native",
            "crashed",
            |_, _, _| panic!("simulated emitter exit before write"),
            |_| Ok(()),
        )
    }));
    assert!(crashed.is_err());
    let status = store.session_steers("session", 0, 32).unwrap();
    assert_eq!(status.pending, 1);
    assert_eq!(status.items[0].steer.id, direction.id);
    store
        .emit_session_steers(
            &adapter,
            "native",
            "crashed",
            |_, _, _| Ok(rendered("recovered")),
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        store.session_steers("session", 0, 32).unwrap().items[0].evidence,
        "emitted"
    );
}

#[test]
fn source_revisions_supersede_only_pending_and_keep_complete_sources() {
    let (_home, store, adapter) = fixture();
    let publish = |revision: &str, body: &str| {
        save_in(
            &store.conn.lock().unwrap(),
            ("session", "session"),
            "comment:immutable-id",
            revision,
            &Author::Named("Author".into()),
            body,
        )
        .unwrap()
    };
    let old = publish("local", "Original");
    assert_eq!(old, publish("local", "Original"));
    let edited = publish("edited", "Correction");
    let page = store.session_steers("session", 0, 32).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].steer, edited);
    store
        .emit_session_steers(
            &adapter,
            "native",
            "one",
            |_, _, _| Ok(rendered("payload")),
            |_| Ok(()),
        )
        .unwrap();
    let latest = publish("latest", &"界".repeat(20_000));
    let page = store.session_steers("session", 0, 32).unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].evidence, "emitted");
    assert!(store.retry_session_steer("session", &edited.id).is_err());
    assert_eq!(
        store
            .session_steer_source("session", &latest.id)
            .unwrap()
            .text,
        latest.text
    );
}

#[test]
fn scopes_graph_downward_and_idle_peers_keep_independent_pending_receipts() {
    let (_home, store, adapter) = fixture();
    for name in ["peer", "sibling", "parent", "taskless"] {
        store.test_session(name, &crate::session_record::new_artifact_key());
    }
    let root = crate::id::WaveId::new();
    let child = crate::id::WaveId::new();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'root','/repo',1)",
            [&root],
        )
        .unwrap();
        conn.execute("INSERT INTO waves(id,name,repo,parent_wave_id,created_at) VALUES(?1,'child','/repo',?2,1)", params![child,root]).unwrap();
        conn.execute(
            "INSERT INTO projects(id,wave_id,created_at) VALUES('project',?1,1)",
            [&child],
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000040008000000000000001','/repo',1);
            INSERT INTO tasks(repository_id,id,project_id,issue_identifier,worktree,created_at)
            VALUES('repo_00000000000040008000000000000001','task','project','T-1','/task',1),('repo_00000000000040008000000000000001','sibling-task','project','T-2','/sibling',1);
            UPDATE tasks SET checkout_machine_id=(SELECT id FROM machines WHERE route='local');
            UPDATE agent_sessions SET repo='/repo',cwd='/' ;
            UPDATE agent_sessions SET cwd='/task' WHERE id IN ('session','peer');
            UPDATE agent_sessions SET cwd='/sibling' WHERE id='sibling';",
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET wave_id=?1 WHERE id='parent'",
            [&root],
        )
        .unwrap();
        save_in(
            &conn,
            ("repository", "/repo"),
            "repo",
            "1",
            &Author::Named("Author".into()),
            "Repository direction",
        )
        .unwrap();
        save_in(
            &conn,
            ("wave", root.as_str()),
            "wave",
            "1",
            &Author::Named("Author".into()),
            "Parent direction",
        )
        .unwrap();
        save_in(
            &conn,
            ("task", "task"),
            "task",
            "1",
            &Author::Named("Author".into()),
            "Task direction",
        )
        .unwrap();
    }
    store
        .save_session_steer("session", "Author", "Direct direction")
        .unwrap();
    for (session, count) in [
        ("session", 4),
        ("peer", 3),
        ("sibling", 2),
        ("parent", 2),
        ("taskless", 1),
    ] {
        assert_eq!(store.session_steers(session, 0, 32).unwrap().pending, count);
    }
    store
        .emit_session_steers(
            &adapter,
            "native",
            "tool",
            |_, rows, _| {
                assert_eq!(rows.len(), 4);
                Ok(rendered("payload"))
            },
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(store.session_steers("peer", 0, 32).unwrap().pending, 3);
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch("UPDATE agent_sessions SET cwd='/' WHERE id='peer'")
        .unwrap();
    assert_eq!(store.session_steers("peer", 0, 32).unwrap().pending, 1);
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(
            "UPDATE agent_sessions SET cwd='/' WHERE id='session';
        UPDATE agent_sessions SET cwd='/task' WHERE id='session';",
        )
        .unwrap();
    assert_eq!(store.session_steers("session", 0, 32).unwrap().pending, 0);
}

#[test]
fn migration_preserves_released_event_copies_and_planning_receipts() {
    use crate::store::migrations::{apply_before_migration, migration_section_sql};
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(&conn, "session_steers");
    conn.execute_batch(include_str!(
        "../../../tests/fixtures/migrations/session_steers_released.sql"
    ))
    .unwrap();
    conn.execute_batch(&migration_section_sql("session_steers"))
        .unwrap();
    let body: String = conn
        .query_row(
            "SELECT p.text FROM task_events e JOIN steers p
        ON p.id=json_extract(e.kind_json,'$.steer_id') WHERE e.id=11",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(body, "Historical 界 direction");
    let copies = conn.prepare("SELECT e.id,p.text FROM project_events e JOIN steers p
        ON p.id=COALESCE(json_extract(e.kind_json,'$.event.steer_id'),json_extract(e.kind_json,'$.steer_id'))
        ORDER BY e.id").unwrap().query_map([], |row| Ok((row.get::<_,i64>(0)?,row.get::<_,String>(1)?)))
        .unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap();
    assert_eq!(
        copies,
        vec![
            (21, body),
            (22, "Project history".into()),
            (23, "Orphan retained source".into())
        ]
    );
    let receipt: String = conn
        .query_row(
            "SELECT comment_id FROM task_linear_ingested_comments",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(receipt, "comment@revision");
    let interruption: String = conn
        .query_row("SELECT kind_json FROM task_events WHERE id=12", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(interruption, r#"{"kind":"interrupt"}"#);
    let events = conn
        .prepare("SELECT kind_json FROM project_events")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    for event in events {
        serde_json::from_str::<crate::work::project::ProjectEventKind>(&event).unwrap();
    }
}

#[test]
fn correction_during_render_cancels_the_superseded_batch_before_write() {
    let (_home, store, adapter) = fixture();
    save_in(
        &store.conn.lock().unwrap(),
        ("session", "session"),
        "source",
        "1",
        &Author::Named("Author".into()),
        "Original",
    )
    .unwrap();
    let retained = store
        .save_session_steer("session", "Author", "Independent direction")
        .unwrap();
    store
        .emit_session_steers(
            &adapter,
            "native",
            "one",
            |_, _, _| {
                save_in(
                    &store.conn.lock().unwrap(),
                    ("session", "session"),
                    "source",
                    "2",
                    &Author::Named("Author".into()),
                    "Correction",
                )
                .unwrap();
                Ok(rendered("obsolete rendered text"))
            },
            |_| panic!("superseded direction escaped"),
        )
        .unwrap();
    let page = store.session_steers("session", 0, 32).unwrap();
    assert_eq!(page.pending, 2);
    assert_eq!(page.items[0].steer, retained);
    assert_eq!(page.items[1].steer.text, "Correction");
    store
        .emit_session_steers(
            &adapter,
            "native",
            "two",
            |_, rows, _| {
                assert_eq!(rows[0], retained);
                assert_eq!(rows[1].text, "Correction");
                Ok(rendered("corrected text"))
            },
            |_| Ok(()),
        )
        .unwrap();
}

fn selections(store: &SqliteStore) -> Vec<(Option<String>, String, String)> {
    store
        .conn
        .lock()
        .unwrap()
        .prepare(
            "SELECT json_extract(payload,'$.provider_thread'),json_extract(payload,'$.next_provider_thread'),
             json_extract(payload,'$.source') FROM session_events
             WHERE json_extract(payload,'$.type')='agent_session_selected' ORDER BY seq",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn a_cleared_conversation_becomes_the_selection_and_receives_steers() {
    let (_home, store, adapter) = fixture();
    store
        .save_session_steer("session", "Named author", "After clear")
        .unwrap();
    store
        .with_provider_context(&adapter, "cleared", CLEAR, || Ok(()))
        .unwrap();
    assert_eq!(
        store.session_thread("session").unwrap().unwrap().as_str(),
        "cleared"
    );
    assert_eq!(
        selections(&store).last().unwrap(),
        &(Some("native".into()), "cleared".into(), "clear".into())
    );
    let stale = store.with_provider_context(&adapter, "native", NativeBoundary::Turn, || Ok(()));
    assert!(
        matches!(stale, Err(StoreError::InvalidAuthority(message)) if message.contains("native") && message.contains("cleared"))
    );
    let emitted = store
        .with_provider_context(&adapter, "cleared", NativeBoundary::Turn, || {
            store.emit_session_steers(
                &adapter,
                "cleared",
                "UserPromptSubmit:one",
                |_, _, _| Ok(rendered("payload")),
                |_| Ok(()),
            )
        })
        .unwrap();
    assert!(emitted);
    assert_eq!(
        store.session_steers("session", 0, 32).unwrap().items[0].evidence,
        "emitted"
    );
}

#[test]
fn a_cleared_conversation_is_what_reconnect_resumes_on_the_same_account() {
    let (_home, store, adapter) = fixture();
    let session = store.session("session").unwrap().unwrap();
    let input = session.artifact_key.clone().unwrap();
    let account = crate::store::ProviderAccountId::parse("work").unwrap();
    store
        .retain_session_observation(
            &session,
            &crate::session_record::provider_session_observation(
                &session,
                &input,
                &"native".into(),
                Some(account.clone()),
            ),
        )
        .unwrap();
    store
        .with_provider_context(&adapter, "cleared", CLEAR, || Ok(()))
        .unwrap();
    let resumed = store.input_provider_session(&input).unwrap().unwrap();
    assert_eq!(resumed.agent_session.as_str(), "cleared");
    assert_eq!(resumed.account_id, Some(account));
}

#[test]
fn a_replaced_agent_process_cannot_select_any_conversation() {
    let (_home, store, stale) = fixture();
    let claim = store.session_attachment("session").unwrap().unwrap();
    let next = store
        .replace_session_agent_process("session", &claim, None, || Ok(true))
        .unwrap();
    for thread in ["native", "foreign"] {
        assert!(matches!(
            store.with_provider_context(&stale, thread, CLEAR, || Ok(())),
            Err(StoreError::InvalidAuthority(_))
        ));
    }
    let current = store.reserve_steer_adapter("session", &next).unwrap();
    store
        .with_provider_context(&current, "next", CLEAR, || Ok(()))
        .unwrap();
}

#[test]
fn activation_never_moves_an_existing_selection() {
    let (_home, store, adapter) = fixture();
    let subagent = store.with_provider_context(
        &adapter,
        "subagent",
        NativeBoundary::Activate {
            source: "OpenCodeMessagesTransform",
        },
        || Ok(()),
    );
    assert!(matches!(subagent, Err(StoreError::InvalidAuthority(_))));
    assert_eq!(
        store.session_thread("session").unwrap().unwrap().as_str(),
        "native"
    );
}

#[test]
fn dropping_the_adapter_thread_keeps_capabilities() {
    use crate::store::migrations::{apply_before_migration, migration_section_sql};
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(&conn, "drop_steer_adapter_thread");
    conn.execute_batch(
        "INSERT INTO processes(id,trace_id,started_at) VALUES('agent','fixture',1);
         INSERT INTO steer_adapters(agent_process_id,capability,provider_thread,created_at,activated_at)
         VALUES('agent','capability','native',1,2);",
    )
    .unwrap();
    conn.execute_batch(&migration_section_sql("drop_steer_adapter_thread"))
        .unwrap();
    let kept: (String, i64) = conn
        .query_row(
            "SELECT capability,activated_at FROM steer_adapters WHERE agent_process_id='agent'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(kept, ("capability".into(), 2));
}
