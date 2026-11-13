//! Plan the statements a hot path runs the way a Machine's store runs them.
//! A fixture store is small, so a scan there is fast; on a Machine with
//! hundreds of thousands of Session events it is the slowest thing in the turn.
//! Tests trace what a path executes, then plan each statement twice:
//! unanalyzed, as a store runs before its server first measures it, and with
//! `sqlite_stat1` measured on a real store, since `ANALYZE` can steer the
//! planner off an index it would use.

use std::cell::RefCell;

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

/// Partial indexes whose rows are current work, not history: walking one
/// reads what is live now. Any other partial index (captured inputs, turn
/// events) holds all history of its kind and counts as a scan of its table.
const LIVE_SUBSETS: [&str; 10] = [
    "agent_process_unfinished",
    "agent_sessions_held_terminal",
    "agent_sessions_primary_repository",
    "agent_sessions_primary_wave",
    "open_sessions",
    "processes_held_terminal",
    "processes_unfinished",
    "processes_unrecorded_ancestry",
    "task_retirements_unfinished",
    "tasks_abandoned_checkout",
];

/// `sqlite_stat1` as `ANALYZE` measured it on a Machine with 16,489
/// Sessions, 166,864 Processes, 266,075 Session events and 36,055 planning
/// changes (November 12); the history window indexes as measured on that
/// Machine on November 13. `steers` and `task_retirements` are left to the
/// fixture, which analyzes them as built so partial indexes are measured.
const MEASURED: &str = "DELETE FROM sqlite_stat1 WHERE tbl IN (
         'agent_sessions','processes','session_events','tasks','steer_batches',
         'steer_deliveries','session_steers','steer_adapters','projects','waves',
         'planning_peer_changes','planning_peer_routes','repository_plans');
     INSERT INTO sqlite_stat1(tbl,idx,stat) VALUES
         ('agent_sessions','agent_sessions_agent_process','16454 1'),
         ('agent_sessions','agent_sessions_held_terminal','66 2'),
         ('agent_sessions','agent_sessions_primary_repository','9 1'),
         ('agent_sessions','agent_sessions_provider_thread','15649 1'),
         ('agent_sessions','agent_sessions_primary_wave','15 1'),
         ('agent_sessions','agent_sessions_transport','11054 4'),
         ('agent_sessions','conversation_task_inventory','16489 32 3 1'),
         ('agent_sessions','open_sessions','620 1 1'),
         ('agent_sessions','session_inventory','16489 8245 2 2 1'),
         ('agent_sessions','session_repo_inventory','16489 750 532 2 2 1'),
         ('agent_sessions','sqlite_autoindex_agent_sessions_1','16489 1'),
         ('agent_sessions','sqlite_autoindex_agent_sessions_2','16489 1'),
         ('planning_peer_changes','planning_peer_changes_clock','36055 1'),
         ('planning_peer_changes','planning_peer_changes_object','36055 7 2'),
         ('planning_peer_changes','sqlite_autoindex_planning_peer_changes_1','36055 1'),
         ('planning_peer_routes','planning_peer_routes_mutation','4220 1'),
         ('planning_peer_routes','sqlite_autoindex_planning_peer_routes_1','4220 844 704 1'),
         ('processes','agent_process_attachment','17262 113'),
         ('processes','agent_process_session','17262 1'),
         ('processes','agent_process_unfinished','68 1'),
         ('processes','processes_completed','170063 2 1'),
         ('processes','processes_cwd','166864 159'),
         ('processes','processes_held_terminal','97 3'),
         ('processes','processes_parent','166864 7 2 1'),
         ('processes','processes_receipt_locator','166864 1'),
         ('processes','processes_recent','166864 2 1'),
         ('processes','processes_trace','166864 35 2 1'),
         ('processes','processes_unrecorded_ancestry','0 0'),
         ('processes','processes_unfinished','741 2 1'),
         ('processes','sqlite_autoindex_processes_1','166864 1'),
         ('projects','idx_projects_wave','63 3 1'),
         ('projects','idx_projects_wave_updated','63 3 2'),
         ('projects','sqlite_autoindex_projects_1','63 1'),
         ('projects','sqlite_autoindex_projects_2','63 2'),
         ('repository_plans','repository_selected_plan','53 1'),
         ('repository_plans','sqlite_autoindex_repository_plans_1','53 1'),
         ('session_events','session_capture_caller','16643 15'),
         ('session_events','session_capture_key','16643 1'),
         ('session_events','session_capture_window','17723 1'),
         ('session_events','session_event_input','20248 2 2'),
         ('session_events','session_event_receipt','266075 17 9 8 3 1'),
         ('session_events','session_events_captured_event','188608 12'),
         ('session_events','session_events_history','266075 17 1'),
         ('session_events','session_events_process','61597 4 4'),
         ('session_events','session_events_provider_session','19846 2'),
         ('session_events','session_events_task','152437 289'),
         ('session_events','session_events_turn','266075 17 9 8 1'),
         ('session_events','session_input_membership','16623 1 1 1'),
         ('session_events','session_input_outcome_window','34882 1'),
         ('session_events','session_observation_receipt','168288 11 1'),
         ('session_events','session_process_membership','61597 4 4'),
         ('session_events','session_turn_events','81144 6 6 4 1 1'),
         ('session_events','session_turn_window','85815 2 1 1 1 1'),
         ('session_events','session_events_wave','112013 5092'),
         ('session_steers','session_steers_steer','43213 24'),
         ('session_steers','sqlite_autoindex_session_steers_1','43213 6 1'),
         ('steer_adapters','sqlite_autoindex_steer_adapters_1','13328 1'),
         ('steer_adapters','sqlite_autoindex_steer_adapters_2','13328 1'),
         ('steer_batches','sqlite_autoindex_steer_batches_1','11004 1'),
         ('steer_batches','steer_batches_boundary','10976 2 1'),
         ('steer_batches','steer_batches_session','11004 2 2 1'),
         ('steer_deliveries','sqlite_autoindex_steer_deliveries_1','42524 6 1 1'),
         ('steer_deliveries','steer_deliveries_batch','42524 4'),
         ('steer_deliveries','steer_deliveries_steer','42524 25'),
         ('tasks','idx_tasks_issue_identifier','1185 1'),
         ('tasks','idx_tasks_project','1185 22 1'),
         ('tasks','idx_tasks_repository','1185 149 21 2'),
         ('tasks','idx_tasks_updated','1185 2'),
         ('tasks','idx_tasks_worktree','1185 2'),
         ('tasks','sqlite_autoindex_tasks_1','1185 1'),
         ('tasks','sqlite_autoindex_tasks_2','1185 1'),
         ('tasks','tasks_branch','1185 2'),
         ('tasks','tasks_checkout_root','838 838 1'),
         ('tasks','tasks_primary_session','299 1'),
         ('tasks','tasks_ship_process','156 1'),
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
                traced.push(statement.sql().into_owned());
            }
        });
    }
}

/// Every statement `run` executes on `store`'s connection, as prepared, with
/// its parameters unbound. SQLite never folds a parameter into the plan, so
/// `?6 AND …` keeps its index where the literal `0 AND …` would plan as a scan
/// that never runs.
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

/// Each plan step of `statements` that scans a growing table, unanalyzed and
/// then measured. Leaves `store` analyzed.
pub(crate) fn growing_scans(store: &SqliteStore, statements: &[String]) -> Vec<String> {
    let conn = store.conn.lock().unwrap();
    let plan = |stats: &str| {
        let mut scans = Vec::new();
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
            let details = explain
                .raw_query()
                .mapped(|row| row.get::<_, String>(3))
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            for detail in details {
                let table = detail
                    .strip_prefix("SCAN ")
                    .and_then(|rest| rest.split_whitespace().next());
                let through_live = detail
                    .split_whitespace()
                    .skip_while(|word| *word != "INDEX")
                    .nth(1)
                    .is_some_and(|index| LIVE_SUBSETS.contains(&index));
                if table.is_some_and(|table| growing.contains(&table)) && !through_live {
                    scans.push(format!("{stats}: {detail}\n  in {sql}"));
                }
            }
        }
        scans
    };
    let mut found = plan("unanalyzed");
    conn.execute_batch("ANALYZE").unwrap();
    conn.execute_batch(MEASURED).unwrap();
    found.extend(plan("analyzed"));
    found
}

#[cfg(test)]
mod tests {
    use super::{growing_scans, statements};
    use crate::durable::TaskId;
    use crate::store::sqlite::{SqliteStore, ViewerHold};
    use rusqlite::params;

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
        let traced = statements(&store, || {
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
        });
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
        let traced = statements(&store, || {
            store
                .sessions_for_agent_session(&crate::id::AgentSessionId::from("thread"))
                .unwrap();
            assert!(store.resolve_history_input("abc").is_err());
            store.unrecorded_agent_ancestry().unwrap();
            store.session_steers("viewed", 0, 10).unwrap();
            store.session_steer_source("viewed", &steer.id).unwrap();
        });
        let scans = growing_scans(&store, &traced);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }

    #[test]
    fn session_history_reads_plan_by_window() {
        let (_home, store, _task, _viewer) = fixture();
        let input = crate::session_record::new_artifact_key();
        store.test_session("history", &input);
        store
            .record_session_event(
                "history",
                &"thread".into(),
                "turn",
                crate::session::SessionEventKind::Completed,
                &serde_json::json!({"status":"completed"}),
            )
            .unwrap();
        let traced = statements(&store, || {
            store
                .recent_conversation_history(None, None, None, 100, 50)
                .unwrap();
            store
                .pending_conversation_history(None, None, None, 100, true)
                .unwrap();
            store.input_history(&input).unwrap();
        });
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
}
