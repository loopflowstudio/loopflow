use crate::lfdb::StoreResult;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
}

/// All migrations in order. Add new migrations here.
///
/// Each migration should work for sqlite.
const ALL_MIGRATIONS: &[Migration] = &[
    Migration {
        version: "001_initial",
        sql: include_str!("migrations/001_initial.sql"),
    },
    Migration {
        version: "002_stimulus_enabled",
        sql: include_str!("migrations/002_stimulus_enabled.sql"),
    },
    Migration {
        version: "003_agent_container_id",
        sql: include_str!("migrations/003_agent_container_id.sql"),
    },
    Migration {
        version: "004_wave_run_kind",
        sql: include_str!("migrations/004_wave_run_kind.sql"),
    },
    Migration {
        version: "005_wave_run_lineage_live_pr_state",
        sql: include_str!("migrations/005_wave_run_lineage_live_pr_state.sql"),
    },
    Migration {
        version: "006_wave_schema_provenance",
        sql: include_str!("migrations/006_wave_schema_provenance.sql"),
    },
    Migration {
        version: "007_chat_memory_blocks",
        sql: include_str!("migrations/007_chat_memory_blocks.sql"),
    },
    Migration {
        version: "008_chat_messages",
        sql: include_str!("migrations/008_chat_messages.sql"),
    },
    Migration {
        version: "009_wave_queue_state",
        sql: include_str!("migrations/009_wave_queue_state.sql"),
    },
    Migration {
        version: "011_chords_data_model",
        sql: include_str!("migrations/011_chords_data_model.sql"),
    },
    Migration {
        version: "012_drop_wave_schema_columns",
        sql: include_str!("migrations/012_drop_wave_schema_columns.sql"),
    },
    Migration {
        version: "013_remove_chord_tree",
        sql: include_str!("migrations/013_remove_chord_tree.sql"),
    },
    Migration {
        version: "015_activation_orchestration",
        sql: include_str!("migrations/015_activation_orchestration.sql"),
    },
    Migration {
        version: "016_provider_tokens",
        sql: include_str!("migrations/016_provider_tokens.sql"),
    },
    // Two migrations share the 016_ numeric prefix. The version strings stored
    // in schema_migrations are the full names, so they're distinct rows. No
    // renumbering — that would require a corrective migration.
    Migration {
        version: "016_rename_sidecar_kind_to_ci_fix_kind",
        sql: include_str!("migrations/016_rename_sidecar_kind_to_ci_fix_kind.sql"),
    },
    Migration {
        version: "017_signal_simplification",
        sql: include_str!("migrations/017_signal_simplification.sql"),
    },
    Migration {
        version: "018_wave_serialized",
        sql: include_str!("migrations/018_wave_serialized.sql"),
    },
    Migration {
        version: "019_activation_target_branch",
        sql: include_str!("migrations/019_activation_target_branch.sql"),
    },
    Migration {
        version: "020_repos",
        sql: include_str!("migrations/020_repos.sql"),
    },
    Migration {
        version: "021_repo_edges",
        sql: include_str!("migrations/021_repo_edges.sql"),
    },
    Migration {
        version: "022_stimulus_max_iterations",
        sql: include_str!("migrations/022_stimulus_max_iterations.sql"),
    },
    Migration {
        version: "023_wave_cycle_start_iteration",
        sql: include_str!("migrations/023_wave_cycle_start_iteration.sql"),
    },
    Migration {
        version: "024_signal_cleanup",
        sql: include_str!("migrations/024_signal_cleanup.sql"),
    },
    Migration {
        version: "025_credential_type",
        sql: include_str!("migrations/025_credential_type.sql"),
    },
    Migration {
        version: "026_rename_stimuli_to_triggers",
        sql: include_str!("migrations/026_rename_stimuli_to_triggers.sql"),
    },
    Migration {
        version: "027_provider_tokens_encrypted",
        sql: include_str!("migrations/027_provider_tokens_encrypted.sql"),
    },
    Migration {
        version: "028_drop_chords_tables",
        sql: include_str!("migrations/028_drop_chords_tables.sql"),
    },
    Migration {
        version: "029_attention_items",
        sql: include_str!("migrations/029_attention_items.sql"),
    },
    Migration {
        version: "030_wave_run_repair_of",
        sql: include_str!("migrations/030_wave_run_repair_of.sql"),
    },
    Migration {
        version: "031_secrets_provider",
        sql: include_str!("migrations/031_secrets_provider.sql"),
    },
    Migration {
        version: "032_sessions",
        sql: include_str!("migrations/032_terminal_sessions.sql"),
    },
    Migration {
        version: "033_wave_workers",
        sql: include_str!("migrations/033_wave_workers.sql"),
    },
    Migration {
        version: "034_wave_run_execution_cursor",
        sql: include_str!("migrations/034_wave_run_execution_cursor.sql"),
    },
    Migration {
        version: "035_session_tmux_name",
        sql: include_str!("migrations/035_terminal_session_tmux_name.sql"),
    },
    Migration {
        version: "036_wave_crons",
        sql: include_str!("migrations/036_wave_crons.sql"),
    },
    Migration {
        version: "037_wave_goal",
        sql: include_str!("migrations/037_wave_goal.sql"),
    },
    Migration {
        version: "038_wave_metrics",
        sql: include_str!("migrations/038_wave_metrics.sql"),
    },
    Migration {
        version: "039_wave_run_snapshot_task",
        sql: include_str!("migrations/039_wave_run_snapshot_task.sql"),
    },
    Migration {
        version: "040_session_use",
        sql: include_str!("migrations/040_terminal_session_use.sql"),
    },
    Migration {
        version: "041_session_parent",
        sql: include_str!("migrations/041_terminal_session_parent.sql"),
    },
    Migration {
        version: "042_wave_repos",
        sql: include_str!("migrations/042_wave_repos.sql"),
    },
    Migration {
        version: "043_drop_legacy_wave_columns",
        sql: include_str!("migrations/043_drop_legacy_wave_columns.sql"),
    },
    Migration {
        version: "044_wave_parent",
        sql: include_str!("migrations/044_wave_parent.sql"),
    },
    Migration {
        version: "045_run_token_usage",
        sql: include_str!("migrations/045_run_token_usage.sql"),
    },
    Migration {
        version: "046_run_token_usage_repo",
        sql: include_str!("migrations/046_run_token_usage_repo.sql"),
    },
    Migration {
        version: "047_run_events",
        sql: include_str!("migrations/047_run_events.sql"),
    },
    Migration {
        version: "048_terminal_sessions_run_id",
        sql: include_str!("migrations/048_terminal_sessions_run_id.sql"),
    },
    Migration {
        version: "049_runs_rename",
        sql: include_str!("migrations/049_runs_rename.sql"),
    },
    Migration {
        version: "050_drop_trigger_organs",
        sql: include_str!("migrations/050_drop_trigger_organs.sql"),
    },
    Migration {
        version: "051_drop_dead_tables",
        sql: include_str!("migrations/051_drop_dead_tables.sql"),
    },
    Migration {
        version: "052_wave_single_repo",
        sql: include_str!("migrations/052_wave_single_repo.sql"),
    },
    Migration {
        version: "053_drop_wave_primary_flow",
        sql: include_str!("migrations/053_drop_wave_primary_flow.sql"),
    },
    Migration {
        version: "054_step_to_skill",
        sql: include_str!("migrations/054_step_to_skill.sql"),
    },
    Migration {
        version: "055_run_events_step_index_repair",
        sql: include_str!("migrations/055_run_events_step_index_repair.sql"),
    },
    Migration {
        version: "056_run_events_provider",
        sql: include_str!("migrations/056_run_events_provider.sql"),
    },
    Migration {
        version: "057_run_events_identity",
        sql: include_str!("migrations/057_run_events_identity.sql"),
    },
    Migration {
        version: "058_blob_tokens",
        sql: include_str!("migrations/058_blob_tokens.sql"),
    },
    Migration {
        version: "059_bus",
        sql: include_str!("migrations/059_bus.sql"),
    },
    Migration {
        version: "060_provider_token_oauth_client_id",
        sql: include_str!("migrations/060_provider_token_oauth_client_id.sql"),
    },
    Migration {
        version: "061_pm_snapshots",
        sql: include_str!("migrations/061_pm_snapshots.sql"),
    },
    // 061_pm_snapshots and 061_trace_capture share the 061_ numeric prefix but
    // stored as distinct full version strings, like the 016_ pair above. Both
    // are already applied to the long-lived ledger; renumbering either would
    // re-run its CREATEs and strand a phantom schema_migrations row.
    Migration {
        version: "061_trace_capture",
        sql: include_str!("migrations/061_trace_capture.sql"),
    },
    Migration {
        version: "062_trace_capture_contract",
        sql: include_str!("migrations/062_trace_capture_contract.sql"),
    },
    Migration {
        version: "063_trace_capture_repair",
        sql: include_str!("migrations/063_trace_capture_repair.sql"),
    },
    Migration {
        version: "064_trace_capture_epoch",
        sql: include_str!("migrations/064_trace_capture_epoch.sql"),
    },
    Migration {
        version: "065_trace_capture_activation",
        sql: include_str!("migrations/065_trace_capture_activation.sql"),
    },
    Migration {
        version: "066_trace_capture_ship_epoch",
        sql: include_str!("migrations/066_trace_capture_ship_epoch.sql"),
    },
    Migration {
        version: "067_trace_capture_audit_epoch",
        sql: include_str!("migrations/067_trace_capture_audit_epoch.sql"),
    },
    Migration {
        version: "062_task_sessions",
        sql: include_str!("migrations/062_task_sessions.sql"),
    },
    Migration {
        // Like the two 016 migrations, the full version string is the key.
        // Keep this name stable because task-session dogfood databases may
        // already have it recorded.
        version: "062_task_project_context",
        sql: include_str!("migrations/062_task_project_context.sql"),
    },
    Migration {
        version: "064_task_pm_receipt",
        sql: include_str!("migrations/064_task_pm_receipt.sql"),
    },
    Migration {
        version: "065_task_agent",
        sql: include_str!("migrations/065_task_agent.sql"),
    },
];

/// Migrations that rename or drop schema objects some dbs never had (the
/// collapse edited historical CREATEs in place; drops may re-run against an
/// already-converged db): their "no such table/column" failure means
/// "already in the target state".
const RENAME_CONVERGENCE_MIGRATIONS: &[&str] = &[
    "048_terminal_sessions_run_id",
    "049_runs_rename",
    "050_drop_trigger_organs",
    "053_drop_wave_primary_flow",
    "055_run_events_step_index_repair",
];

/// Per-migration failures that mean "the db is already in the target state":
/// record the migration as applied and converge instead of crashing.
fn is_tolerated_migration_error(version: &str, message: &str) -> bool {
    // Additive ADD COLUMN re-runs after a version-id rename.
    if message.contains("duplicate column name") {
        return true;
    }
    if version == "062_trace_capture_contract" && message.contains("already exists") {
        return true;
    }
    RENAME_CONVERGENCE_MIGRATIONS.contains(&version)
        && (message.contains("no such column")
            || message.contains("no such table")
            || message.contains("does not exist"))
}

pub fn migrations() -> &'static [Migration] {
    ALL_MIGRATIONS
}

// -- SQLite ------------------------------------------------------------------

pub fn apply_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
    )?;

    let applied = applied_versions_sqlite(conn)?;

    for migration in migrations() {
        if applied.contains(migration.version) {
            continue;
        }
        conn.execute_batch("BEGIN EXCLUSIVE")?;
        let result = (|| -> StoreResult<()> {
            // Another connection may have applied this migration between the
            // `applied` read above and taking the exclusive lock (lf and lfd
            // can open a fresh store concurrently) — re-check inside it.
            let already: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                rusqlite::params![migration.version],
                |row| row.get(0),
            )?;
            if already {
                conn.execute_batch("COMMIT")?;
                return Ok(());
            }
            // A migration that fails only because the db is already in its
            // target state (column already added, column already renamed) is
            // effectively applied — record it and converge rather than
            // crashing every store whose history diverged.
            match conn.execute_batch(migration.sql) {
                Ok(()) => {}
                Err(e) if is_tolerated_migration_error(migration.version, &e.to_string()) => {}
                Err(e) => return Err(e.into()),
            }
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![migration.version, now_unix()],
            )?;
            conn.execute_batch("COMMIT")?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }

    Ok(())
}

fn applied_versions_sqlite(conn: &rusqlite::Connection) -> StoreResult<HashSet<String>> {
    // Table might not exist yet on first run
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
        [],
        |row| row.get(0),
    )?;

    if !exists {
        return Ok(HashSet::new());
    }

    let mut stmt = conn.prepare("SELECT version FROM schema_migrations")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<HashSet<_>, _>>()?)
}

/// Latest applied migration version, or empty string if none applied.
pub fn latest_version_sqlite(conn: &rusqlite::Connection) -> StoreResult<String> {
    let applied = applied_versions_sqlite(conn)?;
    Ok(latest_applied_version(&applied))
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn latest_applied_version(applied: &HashSet<String>) -> String {
    migrations()
        .iter()
        .rev()
        .find(|m| applied.contains(m.version))
        .map(|m| m.version.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_apply_to_fresh_sqlite() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_sqlite(&conn).unwrap();

        // All migration versions recorded
        let applied = applied_versions_sqlite(&conn).unwrap();
        let expected_count = migrations().len();
        assert_eq!(
            applied.len(),
            expected_count,
            "expected {expected_count} migrations, got {}",
            applied.len()
        );
        for migration in migrations() {
            assert!(
                applied.contains(migration.version),
                "migration {} not found in schema_migrations",
                migration.version
            );
        }

        // Key tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for expected in ["waves", "repos", "runs", "terminal_sessions"] {
            assert!(
                tables.iter().any(|t| t == expected),
                "expected table {expected} not found; tables: {tables:?}"
            );
        }

        // Chords died long ago; the trigger organs died in migration 050.
        for unexpected in [
            "chords",
            "chord_members",
            "triggers",
            "stimuli",
            "pending_activations",
            "activation_log",
            "agents",
            "wave_crons",
            // Collapsed back onto `waves` in migration 052 (wave = 1 repo).
            "wave_repos",
        ] {
            assert!(
                tables.iter().all(|t| t != unexpected),
                "unexpected table {unexpected} found; tables: {tables:?}"
            );
        }

        // Re-application is idempotent
        apply_sqlite(&conn).unwrap();
        let applied_again = applied_versions_sqlite(&conn).unwrap();
        assert_eq!(applied, applied_again);
    }

    #[test]
    fn renamed_migration_id_tolerates_existing_column() {
        // Reproduces the `035` rename bug: a db that already has an additive
        // column but recorded the migration under a since-renamed id would
        // re-run its `ADD COLUMN` and crash with "duplicate column name". The
        // runner must treat that as already-applied and converge.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_sqlite(&conn).unwrap();

        // Simulate the pre-rename state: the column exists (from the first
        // apply) but the current version id is no longer recorded.
        conn.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            rusqlite::params!["035_session_tmux_name"],
        )
        .unwrap();

        // Previously this re-ran `ALTER TABLE terminal_sessions ADD COLUMN
        // tmux_name` against a column that already exists and errored.
        apply_sqlite(&conn).expect("re-apply must tolerate the existing column");

        let applied = applied_versions_sqlite(&conn).unwrap();
        assert!(
            applied.contains("035_session_tmux_name"),
            "migration should be recorded again after convergence"
        );
    }

    #[test]
    fn task_agent_migration_repairs_dogfood_schema() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_sqlite(&conn).unwrap();

        conn.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            rusqlite::params!["065_task_agent"],
        )
        .unwrap();
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        conn.execute_batch(
            "ALTER TABLE task_sessions DROP COLUMN agent;
             INSERT INTO task_sessions (
                 id, issue_id, issue_identifier, issue_title, issue_description,
                 project_id, project_slug, project_name, project_context,
                 wave_id, wave_name, status, status_reason, status_at,
                 worktree, branch, base_commit, provider, created_at, updated_at,
                 pm_snapshot_synced_at, pm_writeback_json
             ) VALUES (
                 'ts_demo', 'issue-id', 'INF-123', 'Demo task', '',
                 'project-id', 'work-isolation', 'Work Isolation', '',
                 'wave-id', 'infrastructure', 'waiting', 'ready', 1,
                 '/tmp/loopflow.inf-123', 'jack/inf-123', 'abc123', 'codex', 1, 1,
                 1, '{\"state\":\"current\"}'
             );",
        )
        .unwrap();

        apply_sqlite(&conn).expect("old task-session schemas should migrate");

        let agent: String = conn
            .query_row(
                "SELECT agent FROM task_sessions WHERE id = 'ts_demo'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(agent, "codex");
    }
}

#[cfg(test)]
mod drift_tests {
    use super::*;

    #[test]
    fn trace_contract_migration_preserves_capture_and_run_evidence() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE run_events (
                run_id TEXT NOT NULL, process_id TEXT NOT NULL, seq BIGINT NOT NULL,
                ts BIGINT NOT NULL, node TEXT NOT NULL, event TEXT NOT NULL,
                input_tokens BIGINT, output_tokens BIGINT, cache_read_tokens BIGINT,
                context TEXT
             );
             CREATE INDEX idx_run_events_run ON run_events(run_id, ts);
             CREATE INDEX idx_run_events_process ON run_events(process_id, seq);
             CREATE INDEX idx_run_events_time ON run_events(ts);
             INSERT INTO run_events VALUES
                ('run', 'process', 1, 1, 'run', 'completed', 10, 4, 3, NULL);",
        )
        .unwrap();
        conn.execute_batch(include_str!("migrations/061_trace_capture.sql"))
            .unwrap();
        conn.execute_batch(
            "INSERT INTO agent_launches VALUES (
                'launch', 'run', 'process', 1, 2, '/repo', '/worktree',
                'intelligence', 'code', 'implement', 'codex', 'gpt-5',
                'headless', 'complete', NULL, 'completed',
                '/home/me/.lf/traces/run/process/launch',
                '/home/me/.lf/traces/run/process/launch/conversation.jsonl',
                NULL, 'vendor', NULL, 11, 12, 13, 1, 120
             );
             INSERT INTO agent_turns VALUES (
                'turn', 'launch', 1, 'vendor-turn', 1, 2, 'completed',
                'initial', 'assembled', 'cl100k_base', NULL,
                '/home/me/.lf/traces/run/process/launch/turns/0001-task.md',
                0, 8, 8, 10, 4, NULL, 3, NULL, 0.1, 0, 1
             );
             INSERT INTO context_assets VALUES (
                'turn', 0, 'task', 'loopflow', 'system', 'guide', NULL,
                'operate', 'abc', 0, 5, 5, 2, 2
             );
             INSERT INTO context_decisions VALUES (
                'turn', 0, 'loopflow', 'guide', NULL, 'included',
                'included by operate', 5, 2, 0
             );",
        )
        .unwrap();

        let error = conn
            .execute_batch(include_str!("migrations/062_trace_capture_contract.sql"))
            .expect_err("062 stops after its applied index-name collision");
        assert!(error.to_string().contains("already exists"));
        conn.execute_batch(include_str!("migrations/063_trace_capture_repair.sql"))
            .unwrap();

        let run_totals: (i64, i64) = conn
            .query_row(
                "SELECT COUNT(*), SUM(input_tokens + output_tokens + cache_read_tokens)
                 FROM run_events",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(run_totals, (1, 17));
        let indexes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='index' AND name IN (
                    'idx_run_events_run', 'idx_run_events_process', 'idx_run_events_time'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexes, 3);

        let launch_path: String = conn
            .query_row("SELECT artifact_dir FROM agent_launches", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(launch_path, "run/process/launch");
        let turn: (i64, i64, i64) = conn
            .query_row(
                "SELECT context_gather_ms, context_render_ms, context_persist_ms
                 FROM agent_turns",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(turn, (11, 12, 13));
        let asset: (String, String) = conn
            .query_row("SELECT kind, scope FROM context_assets", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(asset, ("operating_instructions".into(), "global".into()));
        let decision: (String, String) = conn
            .query_row("SELECT kind, scope FROM context_decisions", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(decision, ("operating_instructions".into(), "global".into()));
    }

    #[test]
    fn rename_migration_converges_an_old_schema_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Old-world db: pre-collapse column name, all prior migrations recorded.
        conn.execute_batch(
            "CREATE TABLE terminal_sessions (id TEXT PRIMARY KEY, wave_id TEXT NOT NULL, wave_run_id TEXT);
             CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
        )
        .unwrap();
        for m in migrations() {
            if m.version != "048_terminal_sessions_run_id" {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, 0)",
                    rusqlite::params![m.version],
                )
                .unwrap();
            }
        }
        apply_sqlite(&conn).unwrap();
        let has_run_id: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('terminal_sessions') WHERE name='run_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(has_run_id, "wave_run_id should be renamed to run_id");
    }

    #[test]
    fn runs_rename_converges_an_old_schema_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Old-world shape: pre-rename table names, and runs still carrying
        // the activation_log_id column migration 050 removes.
        conn.execute_batch(
            "CREATE TABLE wave_runs (id TEXT PRIMARY KEY, wave_id TEXT NOT NULL, activation_log_id TEXT);
             CREATE TABLE agents (id TEXT PRIMARY KEY, wave_run_id TEXT);
             CREATE TABLE fork_runs (id TEXT PRIMARY KEY, wave_run_id TEXT);
             CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
        )
        .unwrap();
        for m in migrations() {
            if m.version != "049_runs_rename" && m.version != "050_drop_trigger_organs" {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, 0)",
                    rusqlite::params![m.version],
                )
                .unwrap();
            }
        }
        apply_sqlite(&conn).unwrap();
        let runs_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='runs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(runs_exists, "wave_runs should be renamed to runs");
        let has_run_id: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('fork_runs') WHERE name='run_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            has_run_id,
            "fork_runs.wave_run_id should be renamed to run_id"
        );
        // Migration 050 drops the agents table after 049 renamed its column.
        let agents_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='agents'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!agents_exists, "agents should be dropped by migration 050");
    }

    #[test]
    fn step_index_repair_converges_a_prerelease_054_ledger() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // A pre-release 054 renamed step_index alongside step -> skill, so this
        // ledger carries skill_index and every run_events reader fails on it.
        conn.execute_batch(
            "CREATE TABLE run_events (run_id TEXT NOT NULL, seq BIGINT NOT NULL, skill TEXT, skill_index BIGINT);
             INSERT INTO run_events (run_id, seq, skill, skill_index) VALUES ('r1', 1, 'ci-fix', 7);
             CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
        )
        .unwrap();
        for m in migrations() {
            if m.version != "055_run_events_step_index_repair" {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, 0)",
                    rusqlite::params![m.version],
                )
                .unwrap();
            }
        }
        apply_sqlite(&conn).unwrap();

        // The rename preserves the recorded run history — a repair that drops
        // the ledger is worse than the breakage it fixes.
        let (index, skill): (i64, String) = conn
            .query_row(
                "SELECT step_index, skill FROM run_events WHERE run_id='r1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(index, 7);
        assert_eq!(skill, "ci-fix");
    }

    #[test]
    fn the_migration_starts_the_ledger_empty() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE run_events (
                 run_id TEXT NOT NULL,
                 seq BIGINT NOT NULL,
                 ts BIGINT NOT NULL,
                 node TEXT NOT NULL,
                 event TEXT NOT NULL,
                 provider TEXT
             );
             INSERT INTO run_events (run_id, seq, ts, node, event)
             VALUES ('legacy', 0, 1, 'step', 'started');
             CREATE TABLE schema_migrations (
                 version TEXT PRIMARY KEY,
                 applied_at INTEGER NOT NULL
             );",
        )
        .unwrap();
        for migration in migrations() {
            if migration.version != "057_run_events_identity" {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, 0)",
                    rusqlite::params![migration.version],
                )
                .unwrap();
            }
        }

        apply_sqlite(&conn).unwrap();

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM run_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
        let process_not_null: i64 = conn
            .query_row(
                "SELECT \"notnull\" FROM pragma_table_info('run_events') WHERE name='process_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(process_not_null, 1);
    }

    #[test]
    fn rename_migration_is_tolerated_on_a_fresh_db() {
        // Fresh db: full migration chain creates run_id directly; 048's rename
        // fails benignly and must still be recorded as applied.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_sqlite(&conn).unwrap();
        let recorded: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM schema_migrations WHERE version='048_terminal_sessions_run_id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(recorded);
        // Idempotent on re-run.
        apply_sqlite(&conn).unwrap();
    }
}
