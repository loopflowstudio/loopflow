//! Change revisions kept by schema triggers. A domain's revision moves inside
//! the writer's own transaction whenever a displayed row changes, so a reader
//! learns what changed without scanning history. No call site bumps one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::StoreResult;

use super::SqliteStore;

/// Pins the dedicated Work reader's connection while its joined projections run.
/// The connection must not be lent to a concurrent reader during this lifetime.
#[derive(Debug)]
pub struct ReadSnapshot<'a>(&'a SqliteStore);

impl Drop for ReadSnapshot<'_> {
    fn drop(&mut self) {
        if let Err(error) = self
            .0
            .conn
            .lock()
            .expect("store mutex poisoned")
            .execute_batch("ROLLBACK")
        {
            tracing::error!(%error, "cannot release Work read snapshot");
        }
    }
}

/// One counter per kind of displayed change. Equal revisions mean nothing a
/// workspace surface reads from that domain was committed in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreRevisions {
    pub planning: i64,
    pub sessions: i64,
    pub graphs: i64,
    pub processes: i64,
    /// Session history and turns in `history.db`. Moves at every turn's
    /// start and end; only a read that names `history.*` waits on it.
    pub history: i64,
}

/// Every domain's revision, by name, as clients see them: a write's answer
/// carries the domains it advanced, a reading the domains it was read at. A
/// domain added to `store_revisions` appears here without a type change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct Revisions(pub BTreeMap<String, i64>);

impl Revisions {
    /// The domains `self` advanced past `before`, at their value in `self`.
    pub fn advanced_since(&self, before: &Self) -> Self {
        Self(
            self.0
                .iter()
                .filter(|(domain, revision)| before.0.get(*domain) < Some(revision))
                .map(|(domain, revision)| (domain.clone(), *revision))
                .collect(),
        )
    }

    /// The wake test's fixed domains; a domain not yet created reads 0.
    pub fn to_store_revisions(&self) -> StoreRevisions {
        let domain = |name: &str| self.0.get(name).copied().unwrap_or(0);
        StoreRevisions {
            planning: domain("planning"),
            sessions: domain("sessions"),
            graphs: domain("graphs"),
            processes: domain("processes"),
            history: domain("history"),
        }
    }
}

impl SqliteStore {
    /// Workflow observation excludes transcript, usage and ordinary tool traffic.
    pub fn task_update_revision(&self) -> StoreResult<i64> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT revision FROM store_revisions WHERE domain='task_updates'",
            [],
            |row| row.get(0),
        )?)
    }

    /// Moves whenever a steer is saved, in any scope.
    pub fn steer_revision(&self) -> StoreResult<i64> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT COALESCE(MAX(seq),0) FROM steers",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn read_snapshot(&self) -> StoreResult<ReadSnapshot<'_>> {
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute_batch("BEGIN DEFERRED")?;
        Ok(ReadSnapshot(self))
    }

    pub fn revisions(&self) -> StoreResult<StoreRevisions> {
        Ok(self.revision_map()?.to_store_revisions())
    }

    pub fn revision_map(&self) -> StoreResult<Revisions> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT domain,revision FROM store_revisions")?;
        let mut revisions: BTreeMap<String, i64> = query
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let history =
            conn.query_row("SELECT revision FROM history.history_revision", [], |row| {
                row.get(0)
            })?;
        revisions.insert("history".to_owned(), history);
        Ok(Revisions(revisions))
    }
}

#[cfg(test)]
mod tests {

    use rusqlite::{params, OptionalExtension};

    use super::{SqliteStore, StoreRevisions};
    use loopflow_model::durable::TaskId;
    use loopflow_model::id::{LfProcessId, TraceId, WaveId};

    /// Tables no workspace surface reads. A new table belongs here or in a
    /// domain; `session_events` is covered except for transcript and usage rows.
    const NEVER_DISPLAYED: &[&str] = &[
        // Request-ID memory for served commands; the commands' own rows move domains.
        "command_answers",
        "creation_receipts",
        "access_profiles",
        "planning_user",
        "git_tracker_changes",
        "git_tracker_seen",
        // Released lf still writes the old tracker tables; mirror triggers copy
        // each write into its tracker table, which counts it.
        "pm_issue_changes",
        "pm_items",
        "pm_projects",
        "pm_wave_projects",
        "pm_wave_sync",
        "planning_destinations",
        "planning_peer_changes",
        "planning_peer_conflicts",
        "planning_peer_imports",
        "planning_peer_observed",
        "planning_peer_routes",
        // A field's stamp decides which value is kept; views read the value.
        "plan_stamps",
        // Which commit of another Machine's plan ref was read; views read the plan.
        "plan_refs",
        "blob_tokens",
        // A lookup cache `lf repo people` fills; no Work projection reads it.
        "github_people",
        // Refresh attempts and watermarks do not change displayed Work.
        "linear_team_checks",
        "machines",
        "session_export_changes",
        // Owned Machines' pools are read by `lf machine pool`, not workspace surfaces.
        "pool_machines",
        "provider_account_limits",
        "provider_account_switches",
        "provider_accounts",
        "provider_session_accounts",
        "provider_tokens",
        "rentals",
        "schema_migrations",
        // Delivery diagnostics are explicit CLI reads, not workspace projections.
        // A busy Session's pending move; its Stop hook reads it, no view does.
        "session_moves",
        "steer_adapters",
        "steer_delivered",
        // Only the Machine server reads this cursor to find Sessions to wake.
        "steer_wake_reading",
        "store_revisions",
        "task_linear_ingested_comments",
        // A Task's last reading is compared by the next reader; its news is the steers.
        "task_update_readings",
        // Wave retry receipts are read only by explicit lifecycle operations.
        "wave_lifecycle_operations",
        // Leaving (LOO-954): nothing reads or writes these.
        "ondemands",
        "pm_project_name_cutover",
        "pools",
        "project_binding_imports",
        "session_steers",
        "steer_batches",
        "steer_deliveries",
        "tool_responses",
    ];

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        (dir, store)
    }

    fn session(store: &SqliteStore, id: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES(?1,?1,'human',1,0,'/repo')", [id]).unwrap();
    }

    #[test]
    fn store_revisions_cover_every_table() {
        let (_dir, store) = store();
        let conn = store.conn.lock().unwrap();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let mut uncovered = Vec::new();
        for table in &tables {
            if NEVER_DISPLAYED.contains(&table.as_str()) {
                continue;
            }
            for operation in ["insert", "update", "delete"] {
                // Ownership entries reject deletion; it can never commit.
                if table == "task_ownership" && operation == "delete" {
                    continue;
                }
                let name = format!("store_revision_{table}_{operation}");
                let exists: bool = conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='trigger' AND name=?1 AND tbl_name=?2)",
                        params![name, table],
                        |row| row.get(0),
                    )
                    .unwrap();
                if !exists {
                    uncovered.push(name);
                }
            }
        }
        assert!(
            uncovered.is_empty(),
            "assign each table a store_revisions domain or list it as never displayed: {uncovered:?}"
        );
        for table in NEVER_DISPLAYED {
            assert!(tables.iter().any(|name| name == table), "{table} is gone");
        }
    }

    #[test]
    fn saving_a_steer_invalidates_planning_without_changing_session_state() {
        let (_dir, store) = store();
        session(&store, "conversation");
        let before = store.revisions().unwrap();
        store
            .save_session_steer("conversation", "Fixture author", "Keep the public name")
            .unwrap();
        let after = store.revisions().unwrap();
        assert_eq!(after.planning, before.planning + 1);
        assert_eq!(after.sessions, before.sessions);
    }

    /// The tables the Task conversation work adds, each with the one domain
    /// its three triggers move.
    #[test]
    fn task_graph_graph_process_and_activity_tables_move_their_domains() {
        let (_dir, store) = store();
        let conn = store.conn.lock().unwrap();
        for (table, domain) in [
            ("graph_processes", "graphs"),
            ("graph_process_steps", "graphs"),
            ("task_graphs", "planning"),
            ("session_activity", "sessions"),
        ] {
            let moved: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND tbl_name=?1
                     AND name LIKE 'store_revision_%' AND sql LIKE ?2",
                    params![table, format!("%domain = '{domain}'%")],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(moved, 3, "{table} moves {domain}");
        }
    }

    #[test]
    fn a_session_reading_moves_sessions_only_when_availability_could_change() {
        let (_dir, store) = store();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&process],
            )
            .unwrap();
        let attachment = store
            .attach_session(
                "conversation",
                None,
                &process,
                None,
                &crate::sqlite::SessionCaller {
                    agent: None,
                    capture_key: None,
                },
            )
            .unwrap();
        let record = |observed_at, open_tools, pending_input, yielded| {
            let before = store.revisions().unwrap();
            store
                .record_session_activity(
                    "conversation",
                    &attachment,
                    &crate::session::SessionActivity {
                        preview: None,
                        observed_at,
                        open_tools,
                        pending_input,
                        yielded,
                    },
                )
                .unwrap();
            let after = store.revisions().unwrap();
            assert_eq!(
                StoreRevisions {
                    sessions: before.sessions,
                    ..after
                },
                before
            );
            after.sessions - before.sessions
        };
        assert_eq!(record(1000, 1, 0, false), 1, "first reading");
        assert_eq!(record(1005, 1, 0, false), 0, "still streaming");
        assert_eq!(record(1010, 2, 0, false), 0, "another open tool");
        assert_eq!(record(1015, 0, 0, false), 1, "last tool returned");
        // Quiet arrives with no write; a reader's clock observes it.
        assert_eq!(record(1020, 0, 0, false), 0, "streaming text");
        assert_eq!(record(1140, 0, 0, false), 1, "activity after quiet");
        assert_eq!(record(1145, 0, 1, false), 1, "question asked");
        assert_eq!(record(1150, 0, 0, false), 1, "question answered");
        assert_eq!(record(1155, 0, 0, true), 1, "turn handed back");
    }

    #[test]
    fn planning_moves_for_changes_not_reading_times() {
        let (_dir, store) = store();
        let repo = crate::test_fixture::repository(&store, "/repo");
        let task = TaskId::new();
        let issue = uuid::Uuid::new_v4().to_string();
        let delivery = uuid::Uuid::new_v4().to_string();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES(?1,?2,'LOO-1',1)",
            params![task.as_str(), repo.as_str()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracker_items(repo,tracker,id,identifier,observed_at) VALUES('/repo','linear',?1,'LOO-1',1)",
            [&issue],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracker_actions(id,task_id,kind,value_json,phase,tracker,error)
             VALUES(?1,?2,'state','{}','sent','linear','rate limited')",
            params![delivery, task.as_str()],
        )
        .unwrap();
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tasks SET pm_snapshot_synced_at=10 WHERE id=?1",
                task.as_str()
            ),
            1,
            "first observation"
        );
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tasks SET pm_snapshot_synced_at=20 WHERE id=?1",
                task.as_str()
            ),
            0,
            "Task restamp"
        );
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tracker_items SET observed_at=30 WHERE id=?1",
                &issue
            ),
            0,
            "issue restamp"
        );
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tracker_items SET observed_at=40,name='Renamed' WHERE id=?1",
                &issue
            ),
            1,
            "issue change read with a new stamp"
        );
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tracker_actions SET error='rate limited' WHERE id=?1",
                &delivery
            ),
            0,
            "same error"
        );
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tracker_actions SET error='denied' WHERE id=?1",
                &delivery
            ),
            1,
            "new error"
        );
    }

    /// A released lf writes the old table names while this one writes the
    /// tracker names; each sees the other's writes, counted once.
    #[test]
    fn tracker_table_mirror_keeps_released_writers_and_readers_in_step() {
        let (dir, _store) = store();
        // Released writers still use the retiring Git journal tables.
        let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
        let issue = uuid::Uuid::new_v4().to_string();
        let read = |sql: &str| -> Option<String> {
            conn.query_row(sql, [&issue], |row| row.get(0))
                .optional()
                .unwrap()
        };

        // Released writers: plain insert, upsert, replace.
        let moved = planning_moved(
            &conn,
            "INSERT INTO pm_items(repo,provider,id,identifier,observed_at) VALUES('/repo','linear',?1,'LOO-1',1)",
            &issue,
        );
        assert_eq!(moved, 1, "one change, one revision");
        assert_eq!(
            read("SELECT tracker||' '||identifier FROM tracker_items WHERE id=?1").as_deref(),
            Some("linear LOO-1")
        );
        conn.execute(
            "INSERT INTO pm_items(repo,provider,id,identifier,observed_at) VALUES('/repo','linear',?1,'LOO-2',2)
             ON CONFLICT(repo,provider,id) DO UPDATE SET identifier=excluded.identifier",
            [&issue],
        )
        .unwrap();
        assert_eq!(
            read("SELECT identifier FROM tracker_items WHERE id=?1").as_deref(),
            Some("LOO-2")
        );
        conn.execute(
            "INSERT OR REPLACE INTO pm_items(repo,provider,id,identifier,observed_at) VALUES('/repo','linear',?1,'LOO-3',3)",
            [&issue],
        )
        .unwrap();
        assert_eq!(
            read("SELECT identifier FROM tracker_items WHERE id=?1").as_deref(),
            Some("LOO-3")
        );

        conn.execute(
            "UPDATE pm_items SET identifier='Mixed' WHERE id=?1",
            [&issue],
        )
        .unwrap();
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tracker_items SET identifier='MIXED' WHERE id=?1",
                &issue
            ),
            1
        );
        assert_eq!(
            read("SELECT identifier FROM pm_items WHERE id=?1").as_deref(),
            Some("MIXED")
        );

        // Current writers reach released readers, reading times included.
        let moved = planning_moved(
            &conn,
            "UPDATE tracker_items SET name='Renamed' WHERE id=?1",
            &issue,
        );
        assert_eq!(moved, 1);
        assert_eq!(
            planning_moved(
                &conn,
                "UPDATE tracker_items SET observed_at=9 WHERE id=?1",
                &issue
            ),
            0,
            "a restamp is not a change"
        );
        assert_eq!(
            read("SELECT name||' '||observed_at FROM pm_items WHERE id=?1").as_deref(),
            Some("Renamed 9")
        );
        conn.execute("DELETE FROM tracker_items WHERE id=?1", [&issue])
            .unwrap();
        assert_eq!(read("SELECT id FROM pm_items WHERE id=?1"), None);

        // Git's journal keeps its references across both names.
        conn.execute_batch(&format!(
            "INSERT INTO planning_destinations(repo,id,endpoint,reference) VALUES('/repo','origin','/remote','refs/lf/plan');
             INSERT INTO planning_peer_changes(id,record_table,mutation)
             VALUES('{issue}','tasks','{{\"edit\":{{\"target\":\"t\",\"change\":{{\"field\":\"name\"}}}},\"clock\":1,\"parents\":[]}}');
             INSERT INTO planning_peer_observed(id) VALUES('{issue}');
             INSERT INTO planning_peer_routes(mutation_id,repo,destination) VALUES('{issue}','/repo','origin');
             UPDATE git_tracker_destinations SET fetch_error='offline';"
        ))
        .unwrap();
        assert_eq!(
            read("SELECT field FROM git_tracker_changes WHERE id IN (SELECT id FROM git_tracker_seen) AND id IN (SELECT mutation_id FROM git_tracker_routes) AND id=?1").as_deref(),
            Some("name")
        );
        let error: String = conn
            .query_row(
                "SELECT acquisition_error FROM planning_destinations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(error, "offline");
        conn.execute_batch(
            "DELETE FROM git_tracker_routes; DELETE FROM git_tracker_seen; DELETE FROM git_tracker_changes;",
        )
        .unwrap();
        assert_eq!(
            read("SELECT id FROM planning_peer_changes WHERE id=?1"),
            None
        );
    }

    #[test]
    fn tracker_table_mirror_carries_project_membership_events_and_git_conflicts() {
        let (dir, _store) = store();
        // Released writers still use the retiring Git journal tables.
        let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('wave','work','/repo',1);
             INSERT INTO pm_projects(repo,provider,id,observed_at) VALUES('/repo','linear','p',1);
             INSERT INTO pm_wave_projects VALUES('wave','p',1);
             INSERT INTO pm_wave_sync VALUES('wave','linear','initiative',1);
             INSERT INTO pm_issue_changes VALUES('issue',1,0);
             INSERT INTO planning_peer_imports VALUES('/repo','origin','a');
             INSERT INTO planning_peer_conflicts VALUES('/repo','tasks','task','reason',1);",
        )
        .unwrap();
        for (current, released, field, before, after) in [
            ("tracker_projects", "pm_projects", "observed_at", "1", "2"),
            (
                "tracker_wave_projects",
                "pm_wave_projects",
                "position",
                "1",
                "2",
            ),
            ("tracker_wave_sync", "pm_wave_sync", "synced_at", "1", "2"),
            ("tracker_events", "pm_issue_changes", "removed", "0", "1"),
            (
                "git_tracker_imports",
                "planning_peer_imports",
                "revision",
                "a",
                "b",
            ),
            (
                "git_tracker_conflicts",
                "planning_peer_conflicts",
                "active",
                "1",
                "0",
            ),
        ] {
            let read = |table| {
                conn.query_row(
                    &format!("SELECT CAST({field} AS TEXT) FROM {table}"),
                    [],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .unwrap()
            };
            assert_eq!(read(current).as_deref(), Some(before), "{current}");
            conn.execute(&format!("UPDATE {current} SET {field}=?1"), [after])
                .unwrap();
            assert_eq!(read(released).as_deref(), Some(after), "{released}");
            conn.execute(&format!("DELETE FROM {released}"), [])
                .unwrap();
            assert_eq!(read(current), None, "{current}");
        }
    }

    fn planning_moved(conn: &rusqlite::Connection, sql: &str, id: &str) -> i64 {
        let planning = || -> i64 {
            conn.query_row(
                "SELECT revision FROM store_revisions WHERE domain='planning'",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        let before = planning();
        conn.execute(sql, [id]).unwrap();
        planning() - before
    }

    /// Planning update triggers compare every column, so a column added
    /// later must be named there or its changes stay invisible to readers.
    #[test]
    fn planning_update_triggers_compare_every_column() {
        let (_dir, store) = store();
        let conn = store.conn.lock().unwrap();
        for table in [
            "tracker_items",
            "tracker_projects",
            "projects",
            "tasks",
            "task_changes",
            "task_state_deliveries",
            "tracker_actions",
            "plan_carriers",
            "plan_changes",
        ] {
            let trigger: String = conn
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?1",
                    [format!("store_revision_{table}_update")],
                    |row| row.get(0),
                )
                .unwrap();
            let columns: Vec<String> = conn
                .prepare(&format!(
                    "SELECT name FROM pragma_table_xinfo('{table}') WHERE hidden=0"
                ))
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            for column in columns {
                assert!(
                    trigger.contains(&format!("NEW.{column} IS")),
                    "{table}.{column} missing from its planning trigger"
                );
            }
        }
    }

    #[test]
    fn store_revisions_follow_committed_writes_only() {
        let (_dir, store) = store();
        let before = store.revisions().unwrap();
        let wave = WaveId::new();
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            tx.rollback().unwrap();
        }
        assert_eq!(store.revisions().unwrap(), before);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
                params![LfProcessId::new(), TraceId::new()],
            )
            .unwrap();
        }
        assert_eq!(
            store.revisions().unwrap(),
            StoreRevisions {
                planning: before.planning + 1,
                processes: before.processes + 1,
                ..before
            }
        );
    }

    #[test]
    fn history_and_turns_move_only_the_history_revision() {
        let (_dir, store) = store();
        session(&store, "conversation");
        let before = store.revisions().unwrap();
        store.record_turn_start("conversation", &"thread".into(), "turn", 1);
        store.record_turn_completion(
            "conversation",
            &"thread".into(),
            "turn",
            2,
            &serde_json::json!({"status": "completed"}),
        );
        store.append_history(
            "observation",
            &[crate::history::Row::observed(
                "conversation",
                "store-gap:1:2".into(),
                None,
                1,
                "{}".into(),
            )],
        );
        let after = store.revisions().unwrap();
        assert_eq!(
            after,
            StoreRevisions {
                history: before.history + 3,
                ..before
            }
        );
    }
}
