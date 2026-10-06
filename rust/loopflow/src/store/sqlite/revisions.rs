//! Change revisions kept by schema triggers. A domain's revision moves inside
//! the writer's own transaction whenever a displayed row changes, so a reader
//! learns what changed without scanning history. No call site bumps one.

use serde::{Deserialize, Serialize};

use crate::store::StoreResult;

use super::SqliteStore;

/// One counter per kind of displayed change. Equal revisions mean nothing a
/// workspace surface reads from that domain was committed in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreRevisions {
    pub planning: i64,
    pub sessions: i64,
    pub flows: i64,
    pub execs: i64,
    /// Token counts. Moves every few seconds while agents work.
    pub usage: i64,
}

impl SqliteStore {
    pub(crate) fn revisions(&self) -> StoreResult<StoreRevisions> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut revisions = StoreRevisions {
            planning: 0,
            sessions: 0,
            flows: 0,
            execs: 0,
            usage: 0,
        };
        let mut query = conn.prepare("SELECT domain,revision FROM store_revisions")?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let revision = row.get(1)?;
            match row.get::<_, String>(0)?.as_str() {
                "planning" => revisions.planning = revision,
                "sessions" => revisions.sessions = revision,
                "flows" => revisions.flows = revision,
                "execs" => revisions.execs = revision,
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
    use crate::id::{ExecId, TraceId, WaveId};

    /// Tables no workspace surface reads. A new table belongs here or in a
    /// domain; `session_events` is covered except for transcript and usage rows.
    const NEVER_DISPLAYED: &[&str] = &[
        "access_profiles",
        "auth_browser_bindings",
        "blob_tokens",
        "homes",
        "provider_account_limits",
        "provider_account_switches",
        "provider_accounts",
        "provider_routes",
        "provider_session_accounts",
        "provider_tokens",
        "schema_migrations",
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

    /// The tables the Task conversation work adds, each with the one domain
    /// its three triggers move.
    #[test]
    fn workflow_flow_exec_and_activity_tables_move_their_domains() {
        let (_dir, store) = store();
        let conn = store.conn.lock().unwrap();
        for (table, domain) in [
            ("flow_execs", "flows"),
            ("flow_exec_steps", "flows"),
            ("task_workflows", "planning"),
            ("task_workflow_moves", "planning"),
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
    fn a_session_reading_moves_sessions_only_when_waiting_could_change() {
        let (_dir, store) = store();
        session(&store, "conversation");
        let generation = store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT driver_generation FROM agent_sessions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let driver = crate::exec::SessionDriver {
            exec_id: None,
            generation,
            provider_generation: 0,
            provider_exec_id: ExecId::new(),
        };
        let record = |observed_at, open_tools, pending_input, yielded| {
            let before = store.revisions().unwrap();
            store
                .record_session_activity(
                    "conversation",
                    &driver,
                    &crate::session::SessionActivity {
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
        assert_eq!(store.next_quiet_waiting(1020).unwrap(), Some(1135));
        assert_eq!(record(1020, 0, 0, false), 0, "streaming text");
        assert_eq!(store.next_quiet_waiting(1025).unwrap(), Some(1140));
        assert_eq!(
            store.next_quiet_waiting(1140).unwrap(),
            None,
            "already Waiting"
        );
        assert_eq!(record(1140, 0, 0, false), 1, "activity after quiet");
        assert_eq!(record(1145, 0, 1, false), 1, "question asked");
        assert_eq!(store.next_quiet_waiting(1146).unwrap(), None);
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
                "INSERT INTO execs(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
                params![ExecId::new(), TraceId::new()],
            )
            .unwrap();
        }
        assert_eq!(
            store.revisions().unwrap(),
            StoreRevisions {
                planning: before.planning + 1,
                execs: before.execs + 1,
                ..before
            }
        );
    }

    #[test]
    fn transcript_lines_move_nothing_and_usage_moves_only_its_own_revision() {
        let (_dir, store) = store();
        session(&store, "conversation");
        let before = store.revisions().unwrap();
        // The types every summary reader skips.
        for (line, kind) in [
            "activity",
            "handoff",
            "user_input",
            "conversation",
            "text",
            "tool_use",
            "result",
            "provider_output",
        ]
        .into_iter()
        .enumerate()
        {
            let receipt = format!("input:events.jsonl:{line}");
            event(
                &store,
                "conversation",
                "observed",
                &receipt,
                &evidence(kind),
            );
        }
        assert_eq!(store.revisions().unwrap(), before);
        event(&store, "conversation", "usage", "turn", "{}");
        event(
            &store,
            "conversation",
            "observed",
            "input:events.jsonl:9",
            &evidence("usage"),
        );
        assert_eq!(
            store.revisions().unwrap(),
            StoreRevisions {
                usage: before.usage + 2,
                ..before
            }
        );
        // Lifecycle facts share the `observed` kind with transcript lines, and
        // provider attempts share their receipt key. Unreadable evidence counts.
        for (kind, receipt, payload) in [
            ("observed", "input:manifest.json", "{}".to_owned()),
            ("observed", "input:terminal.json", "{}".to_owned()),
            ("observed", "driver:0:exit", "{}".to_owned()),
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
            ("observed", "input:events.jsonl:23", "{}".to_owned()),
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
