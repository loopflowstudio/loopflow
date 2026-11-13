//! Plan the statements a hot path runs the way a Machine's store runs them.
//! A fixture store is small, so a scan there is fast; on a Machine with
//! hundreds of thousands of Session events it is the slowest thing in the turn.
//! Tests trace what a path executes, then plan each statement twice:
//! unanalyzed, as a store runs before its server first measures it, and with
//! `sqlite_stat1` measured on a real store, since `ANALYZE` can steer the
//! planner off an index it would use.

use std::cell::RefCell;

use rusqlite::OptionalExtension;

use super::sqlite::SqliteStore;

/// Tables that grow with every Task, Session, turn, steer or Process.
const GROWING: [&str; 12] = [
    "agent_sessions",
    "planning_peer_changes",
    "planning_peer_routes",
    "processes",
    "session_events",
    "session_steers",
    "steer_batches",
    "steer_deliveries",
    "steers",
    "task_events",
    "task_retirements",
    "tasks",
];

/// `sqlite_stat1` as `ANALYZE` measured it on a Machine with 17,536
/// Sessions, 170,688 Processes, 283,304 Session events, 37,512 planning
/// changes and 3,362 steers (November 13). Indexes newer than that store's
/// schema were built and analyzed on a copy of it.
const MEASURED: &str = "DELETE FROM sqlite_stat1 WHERE tbl IN (
         'agent_sessions','processes','session_events','tasks','steer_batches',
         'steer_deliveries','session_steers','steer_adapters','projects','waves',
         'planning_peer_changes','planning_peer_routes','repository_plans','steers',
         'task_retirements');
     INSERT INTO sqlite_stat1(tbl,idx,stat) VALUES
         ('agent_sessions','agent_sessions_agent_process','17171 1'),
         ('agent_sessions','agent_sessions_held_terminal','85 2'),
         ('agent_sessions','agent_sessions_primary_repository','9 1'),
         ('agent_sessions','agent_sessions_primary_wave','15 1'),
         ('agent_sessions','agent_sessions_provider_thread','16361 1'),
         ('agent_sessions','agent_sessions_transport','11691 4'),
         ('agent_sessions','agent_sessions_wave','10438 498'),
         ('agent_sessions','conversation_task_inventory','17206 34 3 1'),
         ('agent_sessions','open_sessions','644 1 1'),
         ('agent_sessions','session_inventory','17206 401 3 2 1'),
         ('agent_sessions','session_repo_inventory','17206 101 58 1 1 1'),
         ('agent_sessions','sqlite_autoindex_agent_sessions_1','17206 1'),
         ('agent_sessions','sqlite_autoindex_agent_sessions_2','17206 1'),
         ('planning_peer_changes','planning_peer_changes_clock','37312 1'),
         ('planning_peer_changes','planning_peer_changes_object','37312 3 2'),
         ('planning_peer_changes','sqlite_autoindex_planning_peer_changes_1','37312 1'),
         ('planning_peer_routes','planning_peer_routes_mutation','4220 1'),
         ('planning_peer_routes','sqlite_autoindex_planning_peer_routes_1','4220 401 267 1'),
         ('processes','agent_process_attachment','17979 4'),
         ('processes','agent_process_unfinished','77 1'),
         ('processes','processes_agent_session','18327 1'),
         ('processes','processes_attachment_exit','17586 1'),
         ('processes','processes_cwd','169394 134'),
         ('processes','processes_held_terminal','135 3'),
         ('processes','processes_parent','169394 7 1 1'),
         ('processes','processes_receipt_locator','169394 1'),
         ('processes','processes_recent','169394 2 1'),
         ('processes','processes_trace','169394 81 2 1'),
         ('processes','processes_unfinished','768 2 1'),
         ('processes','processes_unrecorded_ancestry','0 0'),
         ('processes','sqlite_autoindex_processes_1','169394 1'),
         ('projects','idx_projects_wave','63 3 1'),
         ('projects','idx_projects_wave_updated','63 3 2'),
         ('projects','sqlite_autoindex_projects_1','63 1'),
         ('projects','sqlite_autoindex_projects_2','63 2'),
         ('repository_plans','repository_selected_plan','53 1'),
         ('repository_plans','sqlite_autoindex_repository_plans_1','53 1'),
         ('session_events','session_capture_caller','17360 15'),
         ('session_events','session_capture_key','17360 1'),
         ('session_events','session_event_input','20942 15 14'),
         ('session_events','session_event_receipt','278035 51 37 20 7 1'),
         ('session_events','session_events_captured_event','197461 8'),
         ('session_events','session_events_history','278035 51 1'),
         ('session_events','session_events_process','64427 4 4'),
         ('session_events','session_events_provider_session','20583 2'),
         ('session_events','session_events_task','159771 81'),
         ('session_events','session_events_turn','278035 51 37 20 1'),
         ('session_events','session_events_wave','112829 201'),
         ('session_events','session_input_membership','17340 2 1 1'),
         ('session_events','session_observation_receipt','176447 41 1'),
         ('session_events','session_process_membership','64427 10 3'),
         ('session_events','session_provider_transitions','3805 1 1'),
         ('session_events','session_turn_events','84228 45 45 3 1 1'),
         ('session_steers','session_steers_steer','43400 34'),
         ('session_steers','sqlite_autoindex_session_steers_1','43400 6 1'),
         ('steer_adapters','sqlite_autoindex_steer_adapters_1','13914 1'),
         ('steer_adapters','sqlite_autoindex_steer_adapters_2','13914 1'),
         ('steer_batches','sqlite_autoindex_steer_batches_1','11143 1'),
         ('steer_batches','steer_batches_agent_process','11163 2'),
         ('steer_batches','steer_batches_boundary','11078 2 1'),
         ('steer_batches','steer_batches_session','11143 5 5 1'),
         ('steer_deliveries','sqlite_autoindex_steer_deliveries_1','42654 6 1 1'),
         ('steer_deliveries','steer_deliveries_batch','42654 4'),
         ('steer_deliveries','steer_deliveries_steer','42654 34'),
         ('steers','sqlite_autoindex_steers_1','3349 1'),
         ('steers','sqlite_autoindex_steers_2','3349 1 1'),
         ('steers','steers_project','1 1'),
         ('steers','steers_repo','0 0'),
         ('steers','steers_session','435 3'),
         ('steers','steers_task','2913 5 1'),
         ('steers','steers_wave','0 0 0'),
         ('task_retirements','sqlite_autoindex_task_retirements_1','258 1'),
         ('task_retirements','task_retirements_unfinished','179 1'),
         ('tasks','idx_tasks_issue_identifier','1219 1'),
         ('tasks','idx_tasks_project','1219 20 1'),
         ('tasks','idx_tasks_repository','1219 134 22 2'),
         ('tasks','idx_tasks_updated','1219 2'),
         ('tasks','idx_tasks_worktree','1219 10'),
         ('tasks','sqlite_autoindex_tasks_1','1219 1'),
         ('tasks','sqlite_autoindex_tasks_2','1219 1'),
         ('tasks','tasks_abandoned_checkout','179 179'),
         ('tasks','tasks_branch','1219 10'),
         ('tasks','tasks_checkout_machine','1224 612'),
         ('tasks','tasks_checkout_root','858 401 1'),
         ('tasks','tasks_parent_pr','11 2'),
         ('tasks','tasks_primary_session','319 1'),
         ('tasks','tasks_ship_process','168 1'),
         ('waves','idx_waves_active_locator','61 2 2 1'),
         ('waves','idx_waves_parent','61 61'),
         ('waves','idx_waves_superseded_by','0 0'),
         ('waves','sqlite_autoindex_waves_1','63 1'),
         ('waves','waves_project_activation_process','15 1');
     ANALYZE sqlite_schema";

thread_local! {
    static TRACED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

fn trace(event: rusqlite::trace::TraceEvent<'_>) {
    if let rusqlite::trace::TraceEvent::Profile(statement, _) = event {
        TRACED.with(|traced| {
            if let Some(traced) = traced.borrow_mut().as_mut() {
                traced.extend(statement.expanded_sql());
            }
        });
    }
}

/// Every statement `run` executes on `store`'s connection, with its values bound.
pub(crate) fn statements(store: &SqliteStore, run: impl FnOnce()) -> Vec<String> {
    TRACED.with(|traced| *traced.borrow_mut() = Some(Vec::new()));
    store.conn.lock().unwrap().trace_v2(
        rusqlite::trace::TraceEventCodes::SQLITE_TRACE_PROFILE,
        Some(trace),
    );
    run();
    store
        .conn
        .lock()
        .unwrap()
        .trace_v2(rusqlite::trace::TraceEventCodes::empty(), None);
    TRACED.with(|traced| traced.borrow_mut().take().unwrap())
}

/// The call site of each write transaction `run` ends on `store`'s connection
/// that took the write lock and changed no row: an `IMMEDIATE` transaction that
/// only read, or an autocommit write that matched nothing. Each one made every
/// other writer on the Machine wait for nothing.
pub(crate) fn empty_writes(store: &SqliteStore, run: impl FnOnce()) -> Vec<String> {
    super::contention::empty_writes::record(&store.conn, run)
}

/// Fail naming each of `empty_writes`' call sites.
pub(crate) fn assert_no_empty_writes(found: &[String]) {
    assert!(
        found.is_empty(),
        "took the write lock and changed nothing:\n  {}",
        found.join("\n  ")
    );
}

/// A partial index over a live subset (open Sessions, unfinished Processes)
/// stays this small because concurrency bounds it; one over history does not.
const LIVE_SUBSET_ROWS: i64 = 1_000;

/// Each plan step of `statements` that scans a growing table, unanalyzed and
/// then measured. Walking a partial index counts as a scan once the index
/// holds `LIVE_SUBSET_ROWS` in the measured statistics. Leaves `store`
/// analyzed.
pub(crate) fn growing_scans(store: &SqliteStore, statements: &[String]) -> Vec<String> {
    let conn = store.conn.lock().unwrap();
    let plan = |stats: &'static str| {
        let mut steps = Vec::new();
        for sql in statements {
            let keyword = sql.split_whitespace().next().unwrap_or("");
            if !["SELECT", "WITH", "INSERT", "UPDATE", "DELETE", "REPLACE"]
                .contains(&keyword.to_ascii_uppercase().as_str())
            {
                continue;
            }
            // Plans name a table by its alias: `steers p` scans as `SCAN p`.
            let words: Vec<&str> = sql
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .filter(|word| !word.is_empty())
                .collect();
            let mut growing: Vec<&str> = GROWING.to_vec();
            for (at, word) in words.iter().enumerate() {
                if GROWING.contains(word) {
                    let alias = match words.get(at + 1) {
                        Some(&"AS") => words.get(at + 2),
                        alias => alias,
                    };
                    growing.extend(alias);
                }
            }
            let mut explain = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            // Lookups planned from the schema keep their `?` placeholders.
            let unbound = vec![rusqlite::types::Null; explain.parameter_count()];
            let details = explain
                .query_map(rusqlite::params_from_iter(unbound), |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            for detail in details {
                let table = detail
                    .strip_prefix("SCAN ")
                    .and_then(|rest| rest.split_whitespace().next());
                if table.is_some_and(|table| growing.contains(&table)) {
                    steps.push((stats, detail, sql));
                }
            }
        }
        steps
    };
    let mut steps = plan("unanalyzed");
    conn.execute_batch("ANALYZE").unwrap();
    conn.execute_batch(MEASURED).unwrap();
    steps.extend(plan("analyzed"));
    // An index's size is a property of the data, so both passes judge a walk
    // by the measured statistics.
    let partial_rows = |index: &str| -> Option<i64> {
        conn.query_row(
            "SELECT CAST(s.stat AS INTEGER) FROM sqlite_master m
             JOIN sqlite_stat1 s ON s.idx = m.name
             WHERE m.type = 'index' AND m.name = ?1 AND m.sql LIKE '% WHERE %'",
            [index],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
    };
    steps
        .into_iter()
        .filter_map(|(stats, detail, sql)| {
            let index = detail
                .split_whitespace()
                .skip_while(|word| *word != "INDEX")
                .nth(1);
            match index.and_then(partial_rows) {
                Some(rows) if rows < LIVE_SUBSET_ROWS => None,
                Some(rows) => Some(format!(
                    "{stats}: {detail} ({rows} rows measured)\n  in {sql}"
                )),
                None => Some(format!("{stats}: {detail}\n  in {sql}")),
            }
        })
        .collect()
}

/// The lookup SQLite runs for each foreign key whose child table grows when a
/// parent row is deleted or its key changes. `EXPLAIN QUERY PLAN` never shows
/// it, so the schema supplies it.
pub(crate) fn foreign_key_lookups(store: &SqliteStore) -> Vec<String> {
    let conn = store.conn.lock().unwrap();
    let mut keys = conn
        .prepare(
            "SELECT k.\"table\", group_concat(k.\"from\" || ' = ?', ' AND ')
             FROM pragma_foreign_key_list(?1) k GROUP BY k.id ORDER BY k.id",
        )
        .unwrap();
    GROWING
        .iter()
        .flat_map(|child| {
            keys.query_map([child], |row| {
                Ok(format!(
                    "SELECT 1 FROM {child} WHERE {} -- foreign key to {}",
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(0)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        assert_no_empty_writes, empty_writes, foreign_key_lookups, growing_scans, statements,
        GROWING, MEASURED,
    };
    use crate::durable::TaskId;
    use crate::store::sqlite::{SqliteStore, ViewerHold};
    use rusqlite::params;

    fn take_the_lock_for_nothing(store: &SqliteStore) {
        let mut conn = store.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        tx.query_row("SELECT count(*) FROM probe", [], |row| row.get::<_, i64>(0))
            .unwrap();
        drop(tx);
        conn.transaction().unwrap().commit().unwrap();
        conn.execute("UPDATE probe SET x=2 WHERE x=0", []).unwrap();
    }

    #[test]
    fn a_write_that_changes_nothing_is_reported_and_a_read_is_not() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("CREATE TABLE probe(x); INSERT INTO probe VALUES (1);")
            .unwrap();

        let found = empty_writes(&store, || take_the_lock_for_nothing(&store));
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(
            found
                .iter()
                .all(|site| site.ends_with("::take_the_lock_for_nothing")),
            "{found:?}"
        );

        let found = empty_writes(&store, || {
            let mut conn = store.conn.lock().unwrap();
            conn.execute_batch("BEGIN DEFERRED; SELECT count(*) FROM probe; ROLLBACK;")
                .unwrap();
            let savepoint = conn.savepoint().unwrap();
            savepoint
                .query_row("SELECT count(*) FROM probe", [], |row| row.get::<_, i64>(0))
                .unwrap();
            savepoint.commit().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute("UPDATE probe SET x=2 WHERE x=1", []).unwrap();
            tx.commit().unwrap();
        });
        assert_no_empty_writes(&found);
    }

    /// A Machine's retirement sweep and its viewers' polls.
    fn fixture() -> (
        tempfile::TempDir,
        SqliteStore,
        TaskId,
        crate::id::LfProcessId,
    ) {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = TaskId::new();
        let viewer = store.test_lf_process();
        store.test_session("viewed", &crate::session_record::new_artifact_key());
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,
                     checkout_machine_id,planning_completed,planning_completed_at)
                 VALUES(?1,?2,'PLAN-1',1,'/repo.task','task',(SELECT id FROM machines WHERE is_local=1),1,'done')",
                params![task.as_str(), repo.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET cwd='/repo.task/src',interactive=1,transport_lf_process_id=?1
                 WHERE id='viewed'",
                [&viewer],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO task_retirements(task_id,completion,checkout,branch,phase,due_at)
                 VALUES(?1,'done','/repo.task','task','waiting',100)",
                [task.as_str()],
            )
            .unwrap();
        }
        (home, store, task, viewer)
    }

    #[test]
    fn retirement_sweeps_and_viewer_polls_plan_by_index() {
        let (_home, store, task, viewer) = fixture();
        let mut traced = Vec::new();
        let empty = empty_writes(&store, || {
            traced = statements(&store, || {
                assert!(store.pending_retirements().unwrap().contains(&task));
                store
                    .record_owner_viewer_hold(&viewer, Some(ViewerHold::Viewed(200)))
                    .unwrap();
                store
                    .record_owner_viewer_hold(&viewer, Some(ViewerHold::Viewed(200)))
                    .unwrap();
                store
                    .record_owner_viewer_hold(&viewer, Some(ViewerHold::Absent(300)))
                    .unwrap();
            })
        });
        assert_no_empty_writes(&empty);
        let scans = growing_scans(&store, &traced);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }

    #[test]
    fn an_unchanged_viewer_poll_does_not_wait_for_another_write() {
        let (home, store, task, viewer) = fixture();
        store
            .record_owner_viewer_hold(&viewer, Some(ViewerHold::Viewed(200)))
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .busy_timeout(std::time::Duration::from_millis(50))
            .unwrap();
        let writer = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        store
            .record_owner_viewer_hold(&viewer, Some(ViewerHold::Viewed(200)))
            .unwrap();
        // A changed hold needs the write lock.
        assert!(store
            .record_owner_viewer_hold(&viewer, Some(ViewerHold::Absent(300)))
            .is_err());
        writer.execute_batch("ROLLBACK").unwrap();
        store
            .record_owner_viewer_hold(&viewer, Some(ViewerHold::Absent(300)))
            .unwrap();
        let due: Option<i64> = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT due_at FROM task_retirements WHERE task_id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            due, None,
            "viewing reset the deadline and absence keeps it reset"
        );
    }

    #[test]
    fn session_lookups_and_steer_reads_plan_by_index() {
        let (_home, store, _task, _viewer) = fixture();
        // A Machine's worth of steers for other Sessions.
        for other in 0..20 {
            store.test_session(
                &format!("other-{other}"),
                &crate::session_record::new_artifact_key(),
            );
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "INSERT INTO steers(id,session_id,source,revision,author,text,created_at)
                 WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<2999)
                 SELECT lower(hex(randomblob(16))),'other-'||(i%20),'fixture:'||i,'1',
                     '{\"kind\":\"named\",\"id\":\"Fixture author\"}','OTHER',unixepoch() FROM n",
            )
            .unwrap();
        let steer = store
            .save_session_steer("viewed", "person", "Look here")
            .unwrap();
        let mut traced = Vec::new();
        let empty = empty_writes(&store, || {
            traced = statements(&store, || {
                store
                    .sessions_for_agent_session(&crate::id::AgentSessionId::from("thread"))
                    .unwrap();
                assert!(store.resolve_history_input("abc").is_err());
                store.unrecorded_agent_ancestry().unwrap();
                // The first read files the steer's receipt; the second has nothing to file.
                store.session_steers("viewed", 0, 10).unwrap();
                store.session_steers("viewed", 0, 10).unwrap();
                store.session_steer_source("viewed", &steer.id).unwrap();
            })
        });
        assert_no_empty_writes(&empty);
        let scans = growing_scans(&store, &traced);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }

    #[test]
    fn an_unchanged_steer_read_does_not_wait_for_another_write() {
        let (home, store, _task, _viewer) = fixture();
        let steer = store
            .save_session_steer("viewed", "person", "Look here")
            .unwrap();
        // The first read files the steer's receipt.
        store.session_steers("viewed", 0, 10).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .busy_timeout(std::time::Duration::from_millis(50))
            .unwrap();
        let writer = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        let page = store.session_steers("viewed", 0, 10).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            store.session_steer_source("viewed", &steer.id).unwrap().id,
            steer.id
        );
        // A new steer needs a receipt, which needs the write lock.
        writer.execute_batch("ROLLBACK").unwrap();
        store
            .save_session_steer("viewed", "person", "And here")
            .unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(store.session_steers("viewed", 0, 10).is_err());
        writer.execute_batch("ROLLBACK").unwrap();
        assert_eq!(
            store.session_steers("viewed", 0, 10).unwrap().items.len(),
            2
        );
    }

    #[test]
    fn local_planning_writes_take_the_clock_by_index() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let repo = store.home_dir().unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let repository_id = store.ensure_repository(repo.to_str().unwrap()).unwrap();
        let traced = statements(&store, || {
            store
                .create_task(&crate::work::task::NewTask {
                    repository_id,
                    id: TaskId::from_raw("task_abcdef10000000000000000000000001"),
                    project_id: None,
                    title: "Plan by index".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
        });
        let scans: Vec<_> = growing_scans(&store, &traced)
            .into_iter()
            .filter(|scan| scan.contains("planning_peer_changes") || scan.contains("SCAN tasks"))
            .collect();
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }

    #[test]
    fn a_walk_of_a_large_partial_index_is_a_scan() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let history = "SELECT session_id FROM session_events WHERE kind='observed'
             AND json_extract(payload,'$.evidence.provider_session_id')='thread'";
        let scans = growing_scans(&store, &[history.to_owned()]);
        assert!(
            scans
                .iter()
                .any(|scan| scan.contains("session_observation_receipt (")
                    && scan.contains(" rows measured)")),
            "{}",
            scans.join("\n")
        );
        let live = "SELECT id FROM processes WHERE completed_at IS NULL";
        let scans = growing_scans(&store, &[live.to_owned()]);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }

    #[test]
    fn every_partial_index_on_a_growing_table_is_measured() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute_batch("ANALYZE; DELETE FROM sqlite_stat1;")
            .unwrap();
        conn.execute_batch(MEASURED).unwrap();
        let unmeasured: Vec<String> = conn
            .prepare(
                "SELECT m.tbl_name || '.' || m.name FROM sqlite_master m
                 WHERE m.type = 'index' AND m.sql LIKE '% WHERE %'
                     AND m.tbl_name IN (SELECT value FROM json_each(?1))
                     AND m.name NOT IN (SELECT idx FROM sqlite_stat1 WHERE idx IS NOT NULL)
                 ORDER BY 1",
            )
            .unwrap()
            .query_map([serde_json::to_string(&GROWING).unwrap()], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            unmeasured.is_empty(),
            "copy each index's sqlite_stat1 row from a Machine store into MEASURED: {}",
            unmeasured.join(", ")
        );
    }

    #[test]
    fn every_measured_row_names_an_index() {
        // SQLite reads a row for a missing index as the table's row count, so a
        // dropped index's row makes its table look tiny to every plan.
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute_batch("ANALYZE; DELETE FROM sqlite_stat1;")
            .unwrap();
        conn.execute_batch(MEASURED).unwrap();
        let stale: Vec<String> = conn
            .prepare(
                "SELECT s.tbl || '.' || s.idx FROM sqlite_stat1 s
                 WHERE s.idx IS NOT NULL AND s.idx != s.tbl
                     AND NOT EXISTS (SELECT 1 FROM sqlite_master m
                                     WHERE m.type = 'index' AND m.name = s.idx)
                 ORDER BY 1",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(
            stale.is_empty(),
            "MEASURED names indexes the schema no longer has: {}",
            stale.join(", ")
        );
    }

    #[test]
    fn foreign_key_children_on_growing_tables_are_indexed() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let lookups = foreign_key_lookups(&store);
        assert!(lookups
            .iter()
            .any(|lookup| lookup.contains("FROM processes WHERE agent_session_id = ?")));
        let scans = growing_scans(&store, &lookups);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }
}
