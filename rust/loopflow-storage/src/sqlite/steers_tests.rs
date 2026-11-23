use crate::test_fixture::{native_steer as native_fixture, steer as fixture};
use rusqlite::{params, OptionalExtension};

use super::{save_in, SqliteStore};
use crate::steer::{NativeBoundary, Steer, SteerAdapter};
use crate::StoreError;
use loopflow_model::durable::Author;
use loopflow_model::id::LfProcessId;

/// Claim the conversation's next unread steers and stamp them as printed.
fn read(store: &SqliteStore, adapter: &SteerAdapter, thread: &str) -> Vec<Steer> {
    let (steers, _) = store.claim_session_steers(adapter, thread).unwrap();
    let ids: Vec<_> = steers.iter().map(|steer| steer.id.clone()).collect();
    store
        .deliver_session_steers(&adapter.session, &ids)
        .unwrap();
    steers
}

const CLEAR: NativeBoundary<'static> = NativeBoundary::Select { source: "clear" };

/// The retention of a held terminal after `setup` changes its store.
fn retention_after(
    setup: impl FnOnce(&rusqlite::Connection, &loopflow_model::id::TerminalId),
) -> Option<crate::sqlite::sessions::TerminalRetention> {
    let (_home, store, _adapter, terminal) = native_fixture();
    {
        let conn = store.conn.lock().unwrap();
        setup(&conn, &terminal);
    }
    store.terminal_retention(&terminal).unwrap()
}

/// A process under `parent`; a Graph when `graph` names its Graph.
fn insert_process(
    conn: &rusqlite::Connection,
    parent: Option<&LfProcessId>,
    graph: bool,
) -> LfProcessId {
    let id = LfProcessId::new();
    conn.execute(
        "INSERT INTO processes(id,trace_id,started_at,parent_lf_process_id) VALUES(?1,'fixture',1,?2)",
        params![id, parent],
    )
    .unwrap();
    if graph {
        conn.execute(
            "INSERT INTO graph_processes(lf_process_id,graph,compiled) VALUES(?1,'fixture','{}')",
            [&id],
        )
        .unwrap();
    }
    id
}

/// An idle terminal holds nothing; a Graph in it, a captured Graph step,
/// a headless run or a finished Session each name what holds it.
#[test]
fn terminal_retention_names_the_graph_run_or_absence_that_holds_an_idle_terminal() {
    use crate::sqlite::sessions::TerminalRetention::{HeadlessRun, NoOpenSession, RunningGraph};
    assert_eq!(retention_after(|_, _| {}), None);
    assert_eq!(
        retention_after(|conn, terminal| {
            let graph = insert_process(conn, None, true);
            conn.execute(
                "UPDATE graph_processes SET held_terminal_id=?1 WHERE lf_process_id=?2",
                params![terminal, graph],
            )
            .unwrap();
        }),
        Some(RunningGraph)
    );
    // The Session's current capture comes from a step of a Graph it doesn't own.
    let step = retention_after(|conn, _| {
        let graph = insert_process(conn, None, true);
        let step = insert_process(conn, Some(&graph), false);
        conn.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,lf_process_id)
             SELECT session_id,kind,receipt_key||':step',observed_at,payload,?1 FROM session_events
             WHERE seq=(SELECT current_capture FROM agent_sessions WHERE id='session')",
            [&step],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET current_capture=last_insert_rowid()",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations)
             VALUES(?1,?2,0,'[]')",
            params![graph, step],
        )
        .unwrap();
    });
    assert_eq!(step, Some(RunningGraph));
    let run = retention_after(|conn, _| {
        conn.execute("UPDATE agent_sessions SET interactive=0", [])
            .unwrap();
    });
    assert_eq!(run, Some(HeadlessRun));
    let completed = retention_after(|conn, _| {
        conn.execute("UPDATE agent_sessions SET completed_at=1", [])
            .unwrap();
    });
    assert_eq!(completed, Some(NoOpenSession));
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
    let attempts = |store: &SqliteStore| -> (u32, Option<i64>) {
        store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT starts_since_turn,last_started_at FROM agent_sessions WHERE id='session'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap()
    };
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET starts_since_turn=3,last_started_at=7 WHERE id='session'",
            [],
        )
        .unwrap();
    store
        .record_native_turn(&adapter, "native", false, None)
        .unwrap();
    assert_eq!(yielded(&store), Some(false));
    assert_eq!(
        attempts(&store),
        (3, Some(7)),
        "a started turn has not finished"
    );
    store
        .record_native_turn(&adapter, "native", true, Some(0))
        .unwrap();
    assert_eq!(
        yielded(&store),
        Some(true),
        "idle, so the server can wake it"
    );
    assert_eq!(
        attempts(&store),
        (0, None),
        "a finished turn resets recovery; its next death starts at once"
    );
}

#[test]
fn saving_a_steer_wakes_tool_hooks() {
    let (home, store, _) = fixture();
    let marker = crate::steer_markers::session_marker(home.path(), "session");
    assert!(!marker.exists());
    store
        .save_session_steer("session", "Named author", "Change direction")
        .unwrap();
    assert!(marker.exists(), "a saved steer must reach gated tool hooks");
    assert!(
        !crate::steer_markers::latest_marker(home.path()).exists(),
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

/// A foreground turn takes its own steer with every earlier unread one, and
/// its hooks find nothing left to print.
#[test]
fn a_turn_takes_its_steer_with_the_unread_ones_before_it() {
    let (_home, store, adapter) = fixture();
    let earlier = store
        .save_session_steer("session", "Author", "First")
        .unwrap();
    let taken = store
        .take_session_steers("session", "Graph", "Second")
        .unwrap();
    assert_eq!(taken[0], earlier);
    assert_eq!(
        taken
            .iter()
            .map(|steer| steer.text.as_str())
            .collect::<Vec<_>>(),
        ["First", "Second"]
    );
    let page = store.session_steers("session", 0, 32).unwrap();
    assert_eq!(page.pending, 0);
    assert!(page.items.iter().all(|row| row.delivered_at.is_some()));
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
    let root = loopflow_model::id::WaveId::new();
    let child = loopflow_model::id::WaveId::new();
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
            INSERT INTO tasks(repository_id,id,project_id,issue_identifier,created_at)
            VALUES('repo_00000000000040008000000000000001','task','project','T-1',1),('repo_00000000000040008000000000000001','sibling-task','project','T-2',1);
            INSERT INTO task_checkouts(task_id,worktree) VALUES('task','/task'),('sibling-task','/sibling');
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
            INSERT INTO tasks(repository_id,id,issue_identifier,created_at)
            VALUES('repo_00000000000040008000000000000001','task','T-1',1);
            INSERT INTO task_checkouts(task_id,worktree) VALUES('task','/work/task');",
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
    for source in ["declared", "checkout", "inherited", "bound"] {
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET work_source=?1 WHERE id='session'",
                [source],
            )
            .unwrap();
        assert_eq!(pending("/work/task/rust/src/"), 1, "{source}");
        assert_eq!(pending("/work/task"), 1, "{source}");
        assert_eq!(pending("/work/task-other"), 0, "{source}");
        assert_eq!(pending("/work"), 0, "{source}");
    }
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
            INSERT INTO tasks(repository_id,id,issue_identifier,created_at)
            VALUES('repo_00000000000040008000000000000001','task','T-1',1);
            INSERT INTO task_checkouts(task_id,worktree) VALUES('task','/task');
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
    use crate::migration_runner::{apply_before_migration, migration_section_sql};
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(
        &crate::test_fixture::observers(),
        &crate::test_fixture::build(),
        &conn,
        "session_steers",
    );
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/migrations/session_steers_released.sql"
    )))
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
             json_extract(payload,'$.source') FROM history.session_history
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
    let account = crate::ProviderAccountId::parse("work").unwrap();
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

/// Antigravity names its conversation at its first start, and a held terminal
/// leaves no launcher to publish it afterwards.
#[test]
fn a_start_names_what_reconnect_resumes_when_the_launch_named_nothing() {
    let (_home, store, adapter) = fixture();
    let session = store.session("session").unwrap().unwrap();
    let input = session.artifact_key.unwrap();
    assert!(store.input_provider_session(&input).unwrap().is_none());
    store
        .with_provider_context(
            &adapter,
            "reported",
            NativeBoundary::Select { source: "startup" },
            || Ok(()),
        )
        .unwrap();
    let resumed = store.input_provider_session(&input).unwrap().unwrap();
    assert_eq!(resumed.agent_session.as_str(), "reported");
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
    use crate::migration_runner::{apply_before_migration, migration_section_sql};
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    apply_before_migration(
        &crate::test_fixture::observers(),
        &crate::test_fixture::build(),
        &conn,
        "drop_steer_adapter_thread",
    );
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

#[test]
fn a_hook_check_after_the_marker_waits_for_the_steer_commit() {
    let (home, store, adapter) = fixture();
    let mut writer = rusqlite::Connection::open(home.path().join("store.db")).unwrap();
    let tx = writer
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    save_in(
        &tx,
        ("session", "session"),
        "marker-order",
        "1",
        &Author::Named("writer".into()),
        "keep this",
    )
    .unwrap();
    assert!(crate::steer_markers::session_marker(home.path(), "session").exists());
    store
        .conn
        .lock()
        .unwrap()
        .busy_timeout(std::time::Duration::ZERO)
        .unwrap();
    assert!(
        matches!(store.claim_session_steers(&adapter, "native"), Err(StoreError::Sqlite(error))
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy))
    );
    tx.commit().unwrap();
    assert_eq!(read(&store, &adapter, "native")[0].text, "keep this");
}

/// A Task with `primary` as its conversation and `worker` as another of its
/// interactive Sessions, both in its checkout.
fn task_with_primary(store: &SqliteStore, primary: &str, worker: &str) {
    for session in [primary, worker] {
        store.test_session(session, &crate::session_record::new_artifact_key());
    }
    store
        .conn
        .lock()
        .unwrap()
        .execute_batch(&format!(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_00000000000040008000000000000001','/repo',1);
            INSERT INTO tasks(repository_id,id,issue_identifier,created_at)
            VALUES('repo_00000000000040008000000000000001','task','T-1',1);
            INSERT INTO task_checkouts(task_id,worktree) VALUES('task','/task');
            INSERT INTO task_primary_sessions(task_id,session_id) VALUES('task','{primary}');
            UPDATE agent_sessions SET cwd='/task',interactive=1 WHERE id IN ('{primary}','{worker}');"
        ))
        .unwrap();
}

fn woken(store: &SqliteStore) -> Vec<(String, bool)> {
    store
        .read_steer_wakes()
        .unwrap()
        .into_iter()
        .map(|wake| (wake.session, wake.identity.is_some()))
        .collect()
}

/// The server reads each steer once: one naming a Session wakes it, a Task's
/// wakes only its primary conversation, and a Task update, a Wave's or a
/// read one wakes no one.
#[test]
fn each_steer_wakes_the_session_it_names_once() {
    let (_home, store, _adapter) = fixture();
    task_with_primary(&store, "primary", "worker");
    assert!(woken(&store).is_empty(), "nothing saved yet");
    store
        .save_session_steer("worker", "Author", "Direct")
        .unwrap();
    store
        .save_session_steer("worker", "Author", "Again")
        .unwrap();
    {
        let conn = store.conn.lock().unwrap();
        save_in(
            &conn,
            ("task", "task"),
            "task",
            "1",
            &Author::Named("Author".into()),
            "Task direction",
        )
        .unwrap();
        save_in(
            &conn,
            ("task", "task"),
            "update:-:pr",
            "1",
            &Author::Named("Loopflow".into()),
            "Task update",
        )
        .unwrap();
    }
    let taken = store
        .take_session_steers("primary", "Author", "Read in its own turn")
        .unwrap();
    assert_eq!(taken.last().unwrap().text, "Read in its own turn");
    // `primary` read the Task's steer in that turn: nothing wakes it.
    assert_eq!(
        woken(&store),
        [("worker".to_string(), false), ("worker".to_string(), false)]
    );
    assert!(woken(&store).is_empty(), "each steer is read once");
    // A running Session is named with its AgentProcess, for the caller to see.
    store
        .save_session_steer("session", "Author", "Running")
        .unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id='session'",
            [],
        )
        .unwrap();
    assert_eq!(woken(&store), [("session".to_string(), true)]);
    {
        let conn = store.conn.lock().unwrap();
        save_in(
            &conn,
            ("task", "task"),
            "task-2",
            "1",
            &Author::Named("Author".into()),
            "Task direction",
        )
        .unwrap();
    }
    assert_eq!(woken(&store), [("primary".to_string(), false)]);
}

/// The server starts a Session only for a steer its start would take: not a
/// Task steer a later revision replaced, nor one another Session read before
/// the primary conversation began.
#[test]
fn a_steer_its_start_would_not_take_starts_nothing() {
    let (_home, store, _adapter) = fixture();
    task_with_primary(&store, "primary", "worker");
    {
        let conn = store.conn.lock().unwrap();
        for (source, revision, text) in [
            ("task", "1", "First"),
            ("task", "2", "Replaced"),
            ("shared", "1", "Read"),
        ] {
            save_in(
                &conn,
                ("task", "task"),
                source,
                revision,
                &Author::Named("Author".into()),
                text,
            )
            .unwrap();
        }
        conn.execute_batch(
            "INSERT INTO steer_delivered(session_id,steer_id,claimed_at,delivered_at)
               SELECT 'worker',id,1,1 FROM steers WHERE source='shared';
             INSERT INTO steer_delivered(session_id,steer_id,claimed_at,delivered_at)
               SELECT 'primary',id,1,1 FROM steers WHERE source='task' AND revision='2';
             UPDATE agent_sessions SET created_at=(SELECT MAX(created_at)+1 FROM steers)
               WHERE id='primary';",
        )
        .unwrap();
    }
    assert!(store.take_steers_to_start("primary").unwrap().is_empty());
    assert!(woken(&store).is_empty());
}

/// A conversation starting again takes its unread steers as its first input
/// only when one names it.
#[test]
fn a_starting_conversation_takes_its_steers_when_one_names_it() {
    let (_home, store, _adapter) = fixture();
    task_with_primary(&store, "primary", "worker");
    {
        let conn = store.conn.lock().unwrap();
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
    assert!(
        store.take_steers_to_start("worker").unwrap().is_empty(),
        "a Task steer names only the primary conversation"
    );
    assert_eq!(store.session_steers("worker", 0, 32).unwrap().pending, 1);
    store
        .save_session_steer("worker", "Author", "Direct")
        .unwrap();
    let texts = |steers: Vec<Steer>| {
        steers
            .into_iter()
            .map(|steer| steer.text)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        texts(store.take_steers_to_start("worker").unwrap()),
        ["Task direction", "Direct"]
    );
    assert_eq!(store.session_steers("worker", 0, 32).unwrap().pending, 0);
    assert_eq!(
        texts(store.take_steers_to_start("primary").unwrap()),
        ["Task direction"]
    );
    assert!(store.take_steers_to_start("primary").unwrap().is_empty());
}

/// A Task steer the Task's conversation read when it started is still the
/// next worker's to read, and the reverse: each kind reads its own copy.
/// A later conversation starts past what a conversation already read.
#[test]
fn a_task_steer_the_conversation_read_still_reaches_the_next_worker() {
    let (_home, store, _adapter) = fixture();
    task_with_primary(&store, "primary", "worker");
    {
        let conn = store.conn.lock().unwrap();
        save_in(
            &conn,
            ("task", "task"),
            "task",
            "1",
            &Author::Named("Author".into()),
            "USE_THE_FIXTURE",
        )
        .unwrap();
    }
    assert_eq!(store.take_steers_to_start("primary").unwrap().len(), 1);
    for (session, interactive) in [("next-worker", 0), ("next-conversation", 1)] {
        store.test_session(session, &crate::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET cwd='/task',interactive=?2,
                 created_at=(SELECT MAX(created_at)+1 FROM steers) WHERE id=?1",
                params![session, interactive],
            )
            .unwrap();
    }
    assert_eq!(
        store.session_steers("next-worker", 0, 32).unwrap().pending,
        1
    );
    assert_eq!(
        store
            .session_steers("next-conversation", 0, 32)
            .unwrap()
            .pending,
        0
    );
}

/// The server's reading and a starting conversation's check scan no growing
/// table, and a start that nothing names takes no write lock.
#[test]
fn reading_steers_for_starts_scans_nothing_growing() {
    let (_home, store, _adapter) = fixture();
    task_with_primary(&store, "primary", "worker");
    store
        .save_session_steer("worker", "Author", "Direct")
        .unwrap();
    crate::query_plans::assert_no_empty_writes(&store, || {
        store.take_steers_to_start("primary").unwrap()
    });
    let traced = crate::query_plans::statements(&store, || {
        store.read_steer_wakes().unwrap();
        store.take_steers_to_start("primary").unwrap();
        store.take_steers_to_start("worker").unwrap();
    });
    assert_eq!(
        crate::query_plans::growing_scans(&store, &traced),
        Vec::<String>::new()
    );
}
