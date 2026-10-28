//! Change revisions kept by schema triggers. A domain's revision moves inside
//! the writer's own transaction whenever a displayed row changes, so a reader
//! learns what changed without scanning history. No call site bumps one.

use serde::{Deserialize, Serialize};

use crate::store::StoreResult;

use super::SqliteStore;

/// Pins the dedicated Work reader's connection while its joined projections run.
/// The connection must not be lent to a concurrent reader during this lifetime.
#[derive(Debug)]
pub(crate) struct ReadSnapshot<'a>(&'a SqliteStore);

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
    /// Token counts. Moves every few seconds while agents work.
    pub usage: i64,
}

impl SqliteStore {
    pub(crate) fn read_snapshot(&self) -> StoreResult<ReadSnapshot<'_>> {
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute_batch("BEGIN DEFERRED")?;
        Ok(ReadSnapshot(self))
    }

    pub(crate) fn revisions(&self) -> StoreResult<StoreRevisions> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut revisions = StoreRevisions {
            planning: 0,
            sessions: 0,
            graphs: 0,
            processes: 0,
            usage: 0,
        };
        let mut query = conn.prepare("SELECT domain,revision FROM store_revisions")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let revision = row.get(1)?;
            match row.get::<_, String>(0)?.as_str() {
                "planning" => revisions.planning = revision,
                "sessions" => revisions.sessions = revision,
                "graphs" => revisions.graphs = revision,
                "processes" => revisions.processes = revision,
                "usage" => revisions.usage = revision,
                _ => {}
            }
        }
        Ok(revisions)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::{SqliteStore, StoreRevisions};
    use crate::id::{LfProcessId, TraceId, WaveId};

    /// Tables no workspace surface reads. A new table belongs here or in a
    /// domain; `session_events` is covered except for transcript and usage rows.
    const NEVER_DISPLAYED: &[&str] = &[
        "access_profiles",
        "pm_project_name_cutover",
        "planning_user",
        "planning_peer_changes",
        "planning_peer_observed",
        "project_binding_imports",
        "auth_browser_bindings",
        "blob_tokens",
        // Desktop reads GitHub people once per repository through `lf ops people`.
        "github_people",
        // Acquisition attempts and watermarks do not change displayed Work.
        "linear_team_checks",
        "machines",
        // Copies from rented Machines are read by explicit CLI listings only.
        "machine_records",
        // Rental capacity is read by `lf pool`/`lf ondemand`, not workspace surfaces.
        "ondemands",
        "pools",
        "provider_account_limits",
        "provider_account_switches",
        "provider_accounts",
        "provider_session_accounts",
        "provider_tokens",
        "rentals",
        "schema_migrations",
        // Delivery diagnostics are explicit CLI reads, not workspace projections.
        "session_steers",
        "steer_adapters",
        "steer_batches",
        "steer_deliveries",
        "store_revisions",
        "task_linear_ingested_comments",
        "tool_responses",
    ];

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        (dir, store)
    }

    fn session(store: &SqliteStore, id: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd) VALUES(?1,?1,'human',1,0,'/repo')", [id]).unwrap();
    }

    fn event(store: &SqliteStore, session: &str, kind: &str, receipt: &str, payload: &str) {
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,?2,?3,1,?4)", params![session, kind, receipt, payload]).unwrap();
    }

    fn evidence(kind: &str) -> String {
        format!(r#"{{"evidence":{{"schema_version":1,"type":"{kind}"}}}}"#)
    }

    #[test]
    fn a_read_snapshot_keeps_joined_queries_and_revisions_at_one_commit() {
        let (dir, writer) = store();
        writer.test_session("first", &crate::session_record::new_artifact_key());
        writer.test_session("second", &crate::session_record::new_artifact_key());
        let reader = SqliteStore::open_read_only(&dir.path().join("db")).unwrap();
        let snapshot = reader.read_snapshot().unwrap();
        let before = reader.revisions().unwrap();
        assert_eq!(
            reader.session_summary("first", 1).unwrap().unwrap().title,
            "Retained"
        );
        writer
            .conn
            .lock()
            .unwrap()
            .execute_batch("BEGIN; UPDATE agent_sessions SET title='committed'; COMMIT;")
            .unwrap();
        // These readers have their own savepoints, nested inside the pinned read.
        assert_eq!(
            reader.session_summary("second", 1).unwrap().unwrap().title,
            "Retained"
        );
        assert_eq!(reader.revisions().unwrap(), before);
        drop(snapshot);
        assert_eq!(
            reader.session_summary("first", 1).unwrap().unwrap().title,
            "committed"
        );
        assert_eq!(
            reader.session_summary("second", 1).unwrap().unwrap().title,
            "committed"
        );
        assert!(reader.revisions().unwrap().sessions > before.sessions);
        // Failed projection exits release the transaction too.
        let failed = || -> crate::store::StoreResult<()> {
            let _snapshot = reader.read_snapshot()?;
            reader
                .conn
                .lock()
                .unwrap()
                .execute_batch("SELECT absent FROM no_table")?;
            Ok(())
        };
        assert!(failed().is_err());
        assert!(reader.read_snapshot().is_ok());
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
            .claim_session_attachment("conversation", None, &process, true)
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
        // Counting toward quiet from the newest reading, with no write when it arrives.
        assert_eq!(store.next_quiet_available(1020).unwrap(), Some(1135));
        assert_eq!(record(1020, 0, 0, false), 0, "streaming text");
        assert_eq!(store.next_quiet_available(1025).unwrap(), Some(1140));
        assert_eq!(
            store.next_quiet_available(1140).unwrap(),
            None,
            "already Available"
        );
        assert_eq!(record(1140, 0, 0, false), 1, "activity after quiet");
        assert_eq!(record(1145, 0, 1, false), 1, "question asked");
        assert_eq!(store.next_quiet_available(1146).unwrap(), None);
        assert_eq!(record(1150, 0, 0, false), 1, "question answered");
        assert_eq!(record(1155, 0, 0, true), 1, "turn handed back");
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
    fn usage_moves_only_its_own_revision_and_every_fact_moves_sessions() {
        let (_dir, store) = store();
        session(&store, "conversation");
        let before = store.revisions().unwrap();
        event(&store, "conversation", "usage", "turn", "{}");
        assert_eq!(
            store.revisions().unwrap(),
            StoreRevisions {
                usage: before.usage + 1,
                ..before
            }
        );
        // Facts share the `observed` kind and receipt keys; unreadable evidence counts.
        for (kind, receipt, payload) in [
            ("observed", "input:manifest.json", "{}".to_owned()),
            ("observed", "input:terminal.json", "{}".to_owned()),
            ("observed", "attachment:0:exit", "{}".to_owned()),
            ("started", "turn", "{}".to_owned()),
            ("completed", "turn", "{}".to_owned()),
            (
                "observed",
                "input:events.jsonl:20",
                evidence("provider_session_observed"),
            ),
            (
                "observed",
                "input:events.jsonl:21",
                evidence("provider_attempt_finished"),
            ),
            ("observed", "input:events.jsonl:22", "{".to_owned()),
            ("observed", "input:silence:23", "{}".to_owned()),
        ] {
            let sessions = store.revisions().unwrap().sessions;
            event(&store, "conversation", kind, receipt, &payload);
            assert_eq!(
                store.revisions().unwrap().sessions,
                sessions + 1,
                "{kind} {receipt}"
            );
        }
    }
}
