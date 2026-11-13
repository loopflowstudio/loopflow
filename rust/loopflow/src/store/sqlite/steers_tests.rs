use rusqlite::{params, OptionalExtension};

use super::{save_in, SqliteStore};
use crate::durable::Author;
use crate::id::LfProcessId;
use crate::steer::{NativeBoundary, Steer, SteerAdapter};
use crate::store::sqlite::sessions::OwnerRetention::LiveTurn;
use crate::store::StoreError;

/// Claim the conversation's next unread steers and stamp them as printed.
fn read(store: &SqliteStore, adapter: &SteerAdapter, thread: &str) -> Vec<Steer> {
    let (steers, _) = store.claim_session_steers(adapter, thread).unwrap();
    let ids: Vec<_> = steers.iter().map(|steer| steer.id.clone()).collect();
    store
        .deliver_session_steers(&adapter.session, &ids)
        .unwrap();
    steers
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
    store
        .record_agent_process_identity("session", &claim, std::process::id(), 1)
        .unwrap();
    let adapter = store.reserve_steer_adapter("session", &claim).unwrap();
    store
        .with_provider_context(&adapter, "native", ACTIVATE, || Ok(()))
        .unwrap();
    (home, store, adapter)
}

fn native_fixture() -> (tempfile::TempDir, SqliteStore, SteerAdapter) {
    let (home, store, adapter) = fixture();
    let attachment = store.session_attachment("session").unwrap().unwrap();
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE agent_sessions SET interactive=1,transport_lf_process_id=?1 WHERE id='session'",
            [attachment.lf_process_id.as_ref().unwrap()],
        )
        .unwrap();
        conn.execute(
            "UPDATE processes SET agent_interactive=1 WHERE id=?1",
            [&adapter.agent_process_id],
        )
        .unwrap();
    }
    (home, store, adapter)
}

#[test]
fn pending_requests_hold_the_owner_until_the_current_attachment_settles_them() {
    let (_home, store, _adapter) = fixture();
    let first = store.session_attachment("session").unwrap().unwrap();
    let owner = first.lf_process_id.as_ref().unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET interactive=1,transport_lf_process_id=?1 WHERE id='session'",
            [owner],
        )
        .unwrap();
    let mut activity = crate::session::SessionActivity {
        preview: None,
        observed_at: 1,
        open_tools: 0,
        pending_input: 1,
        yielded: true,
    };
    store
        .record_session_activity("session", &first, &activity)
        .unwrap();
    for viewed in [false, true] {
        store
            .record_owner_viewer_hold(
                owner,
                viewed.then_some(crate::store::sqlite::ViewerHold::Viewed(1)),
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                r#"UPDATE session_activity SET program_status='{"records":[{"state":"idle"}]}' WHERE session_id='session'"#,
                [],
            )
            .unwrap();
        assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    }
    let next = store
        .claim_session_attachment("session", Some(&first), owner, false)
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    activity.pending_input = 0;
    store
        .record_session_activity("session", &first, &activity)
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    store
        .record_session_activity("session", &next, &activity)
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), None);
    activity.pending_input = 1;
    store
        .record_session_activity("session", &next, &activity)
        .unwrap();
    store
        .replace_session_agent_process("session", &next, None, || Ok(true))
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), None);
}

#[test]
fn silent_native_turns_follow_the_provider_through_attachment_handoff() {
    let (_home, store, adapter) = native_fixture();
    let first = store.session_attachment("session").unwrap().unwrap();
    let owner = first.lf_process_id.as_ref().unwrap();
    store
        .record_native_turn(&adapter, "native", false, None)
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    let next = store
        .claim_session_attachment("session", Some(&first), owner, false)
        .unwrap();
    assert_ne!(first.token, next.token);
    // No editor input or open tool is needed to keep an unyielded turn.
    {
        let conn = store.conn.lock().unwrap();
        conn.execute("UPDATE processes SET viewed=1 WHERE id=?1", [owner])
            .unwrap();
        conn.execute(
            r#"UPDATE session_activity SET program_status='{"records":[{"state":"idle"}]}' WHERE session_id='session'"#,
            [],
        ).unwrap();
    }
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    // The provider itself survived; its stop belongs to its current attachment.
    store
        .record_native_turn(&adapter, "native", true, Some(1))
        .unwrap();
    // A viewed prompt also cannot settle its provider-owned watch.
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    store
        .record_native_turn(&adapter, "native", true, Some(0))
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), None);
    store
        .conn
        .lock()
        .unwrap()
        .execute("UPDATE processes SET viewed=0 WHERE id=?1", [owner])
        .unwrap();
    store
        .record_native_turn(&adapter, "native", false, None)
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    assert!(store
        .record_native_turn(&adapter, "child", true, Some(0))
        .is_err());
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    let replacement = store
        .replace_session_agent_process("session", &next, None, || Ok(true))
        .unwrap();
    // Activity from the old provider cannot keep its replacement alive.
    assert_eq!(store.owner_retention(owner).unwrap(), None);
    assert!(store
        .record_native_turn(&adapter, "native", true, Some(1))
        .is_err());
    let current = store
        .reserve_steer_adapter("session", &replacement)
        .unwrap();
    store
        .with_provider_context(&current, "native", ACTIVATE, || Ok(()))
        .unwrap();
    store
        .record_native_turn(&current, "native", true, Some(1))
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    assert!(store
        .record_native_turn(&adapter, "native", true, Some(0))
        .is_err());
    assert_eq!(store.owner_retention(owner).unwrap(), Some(LiveTurn));
    store
        .record_native_turn(&current, "native", true, Some(0))
        .unwrap();
    assert_eq!(store.owner_retention(owner).unwrap(), None);
}

/// Interactive Codex: lf spawned its app-server, so the AgentProcess is not a
/// native terminal, but a person's client takes the turns and only its hooks
/// see them end. A headless run reports through lf's own stream instead.
#[test]
fn a_conversation_on_an_app_server_records_its_turns_from_hooks() {
    let (_home, store, adapter) = fixture();
    let interactive = |value: bool| {
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET interactive=?1 WHERE id='session'",
                [value],
            )
            .unwrap();
    };
    interactive(false);
    let yielded = |store: &SqliteStore| -> Option<bool> {
        store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT yielded FROM session_activity WHERE session_id='session'",
                [],
                |row| row.get(0),
            )
            .optional()
            .unwrap()
    };
    store
        .record_native_turn(&adapter, "native", true, Some(0))
        .unwrap();
    assert_eq!(yielded(&store), None, "headless: lf's stream reports");
    interactive(true);
    store
        .record_native_turn(&adapter, "native", false, None)
        .unwrap();
    assert_eq!(yielded(&store), Some(false));
    store
        .record_native_turn(&adapter, "native", true, Some(0))
        .unwrap();
    assert_eq!(
        yielded(&store),
        Some(true),
        "idle, so the server can wake it"
    );
}

#[test]
fn saving_a_steer_wakes_tool_hooks() {
    let (home, store, _) = fixture();
    let marker = crate::steer::session_marker(home.path(), "session");
    assert!(!marker.exists());
    store
        .save_session_steer("session", "Named author", "Change direction")
        .unwrap();
    assert!(marker.exists(), "a saved steer must reach gated tool hooks");
    assert!(
        !crate::steer::latest_marker(home.path()).exists(),
        "one Session's steer starts lf in no other conversation"
    );
}

#[test]
fn a_read_steer_is_not_replayed() {
    let (_home, store, adapter) = fixture();
    let steer = store
        .save_session_steer("session", "Named author", "Change direction")
        .unwrap();
    assert_eq!(
        read(&store, &adapter, "native"),
        std::slice::from_ref(&steer)
    );
    let page = store.session_steers("session", 0, 32).unwrap();
    assert!(page.items[0].delivered_at.is_some());
    assert_eq!(page.pending, 0);
    assert!(read(&store, &adapter, "native").is_empty());
}

/// A hook that claims and never prints: the claim hides the steer from a hook
/// beside it, then lapses, and the steer is claimed again.
#[test]
fn an_unprinted_claim_hides_the_steer_then_lapses() {
    let (_home, store, adapter) = fixture();
    let steer = store
        .save_session_steer("session", "Author", "Direction")
        .unwrap();
    let (claimed, remaining) = store.claim_session_steers(&adapter, "native").unwrap();
    assert_eq!(
        (claimed.as_slice(), remaining),
        ([steer.clone()].as_slice(), 0)
    );
    assert!(store
        .claim_session_steers(&adapter, "native")
        .unwrap()
        .0
        .is_empty());
    let page = store.session_steers("session", 0, 32).unwrap();
    assert_eq!((page.pending, page.items[0].delivered_at), (0, None));
    store
        .conn
        .lock()
        .unwrap()
        .execute("UPDATE steer_delivered SET claimed_at=claimed_at-60", [])
        .unwrap();
    assert_eq!(store.session_steers("session", 0, 32).unwrap().pending, 1);
    assert_eq!(
        read(&store, &adapter, "native"),
        std::slice::from_ref(&steer)
    );
    // A claim handed back is unread at once.
    store
        .save_session_steer("session", "Named author", "And another")
        .unwrap();
    let (claimed, _) = store.claim_session_steers(&adapter, "native").unwrap();
    store
        .release_session_steers("session", &[claimed[0].id.clone()])
        .unwrap();
    assert_eq!(store.session_steers("session", 0, 32).unwrap().pending, 1);
}

#[test]
fn claims_take_bounded_pages_oldest_first() {
    let (_home, store, adapter) = fixture();
    for n in 0..20 {
        store
            .save_session_steer("session", "Author", &format!("Direction {n}"))
            .unwrap();
    }
    let (first, remaining) = store.claim_session_steers(&adapter, "native").unwrap();
    assert_eq!((first.len(), remaining), (8, 12));
    let (second, remaining) = store.claim_session_steers(&adapter, "native").unwrap();
    assert_eq!((second.len(), remaining), (8, 4));
    assert!(second[0].sequence > first[7].sequence);
    let page = store.session_steers("session", 0, 3).unwrap();
    assert_eq!(page.items.len(), 3);
    let next = store
        .session_steers("session", page.next_after.unwrap(), 3)
        .unwrap();
    assert!(next.items[0].steer.sequence > page.items[2].steer.sequence);
}

#[test]
fn source_revisions_supersede_unread_ones_and_keep_complete_sources() {
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
    assert_eq!(
        read(&store, &adapter, "native"),
        std::slice::from_ref(&edited)
    );
    // A correction to a steer already read is a new steer to read.
    let latest = publish("latest", &"界".repeat(20_000));
    let page = store.session_steers("session", 0, 32).unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page.items[0].delivered_at.is_some());
    assert_eq!(
        (page.items[1].steer.id.clone(), page.pending),
        (latest.id.clone(), 1)
    );
    assert_eq!(
        store
            .session_steer_source("session", &latest.id)
            .unwrap()
            .text,
        latest.text
    );
}

#[test]
fn scopes_graph_downward_and_each_session_reads_for_itself() {
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
            UPDATE tasks SET checkout_machine_id=(SELECT id FROM machines WHERE is_local=1);
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
    assert_eq!(read(&store, &adapter, "native").len(), 4);
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
fn a_session_anywhere_inside_a_task_checkout_gets_its_steers() {
    let (_home, store, _adapter) = fixture();
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000040008000000000000001','/repo',1);
            INSERT INTO tasks(repository_id,id,issue_identifier,worktree,created_at)
            VALUES('repo_00000000000040008000000000000001','task','T-1','/work/task',1);
            UPDATE tasks SET checkout_machine_id=(SELECT id FROM machines WHERE is_local=1);",
        )
        .unwrap();
    save_in(
        &store.conn.lock().unwrap(),
        ("task", "task"),
        "task",
        "1",
        &Author::Named("Author".into()),
        "Task direction",
    )
    .unwrap();
    let pending = |cwd: &str| {
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE agent_sessions SET cwd=?1 WHERE id='session'", [cwd])
            .unwrap();
        store.session_steers("session", 0, 32).unwrap().pending
    };
    assert_eq!(pending("/work/task/rust/src/"), 1);
    assert_eq!(pending("/work/task"), 1);
    assert_eq!(pending("/work/task-other"), 0);
    assert_eq!(pending("/work"), 0);
}

#[test]
fn a_new_task_session_skips_steers_an_earlier_session_read() {
    let (_home, store, adapter) = fixture();
    let save = |source: &str, text: &str| {
        let conn = store.conn.lock().unwrap();
        save_in(
            &conn,
            ("task", "task"),
            source,
            "1",
            &Author::Named("Author".into()),
            text,
        )
        .unwrap();
    };
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000040008000000000000001','/repo',1);
            INSERT INTO tasks(repository_id,id,issue_identifier,worktree,created_at)
            VALUES('repo_00000000000040008000000000000001','task','T-1','/task',1);
            UPDATE tasks SET checkout_machine_id=(SELECT id FROM machines WHERE is_local=1);
            UPDATE agent_sessions SET cwd='/task';",
        )
        .unwrap();
    save("read", "Already read");
    assert_eq!(read(&store, &adapter, "native").len(), 1);
    // Pending for no Session when the next one starts.
    save("unread", "Never read");
    store.test_session("next", &crate::session_record::new_artifact_key());
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(
            "UPDATE steers SET created_at=unixepoch()-10;
            UPDATE agent_sessions SET cwd='/task',created_at=unixepoch()-5 WHERE id='next';",
        )
        .unwrap();
    let texts = |page: crate::steer::SessionSteerPage| {
        page.items
            .into_iter()
            .map(|item| item.steer.text)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        texts(store.session_steers("next", 0, 32).unwrap()),
        ["Never read"]
    );
    save("later", "Saved after it started");
    let page = store.session_steers("next", 0, 32).unwrap();
    assert_eq!(page.pending, 2);
    assert_eq!(texts(page), ["Never read", "Saved after it started"]);
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

fn selections(store: &SqliteStore) -> Vec<(Option<String>, String, String)> {
    store
        .conn
        .lock()
        .unwrap()
        .prepare(
            "SELECT json_extract(payload,'$.transition.from_provider_session'),json_extract(payload,'$.transition.to_provider_session'),
             json_extract(payload,'$.source') FROM session_events
             WHERE json_extract(payload,'$.type')='provider_transition' ORDER BY seq",
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
    assert_eq!(read(&store, &adapter, "cleared").len(), 1);
    assert!(store.session_steers("session", 0, 32).unwrap().items[0]
        .delivered_at
        .is_some());
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
