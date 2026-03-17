use crate::lfd::store::StoreResult;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: &'static str,
    pub sql: &'static str,
}

/// All migrations in order. Add new migrations here.
///
/// Each migration should work for both sqlite and postgres using portable SQL
/// (TEXT/INTEGER/BOOLEAN — no backend-specific syntax).
///
/// If a migration genuinely requires different SQL per backend, add separate
/// entries and filter them in `migrations()`.
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
        version: "010_sessions",
        sql: include_str!("migrations/010_sessions.sql"),
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
        version: "014_rename_provider_to_harness",
        sql: include_str!("migrations/014_rename_provider_to_harness.sql"),
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
];

/// Migrations applicable to a backend. Currently returns all migrations
/// since everything is portable SQL. Override resolution would go here
/// if backend-specific migrations are ever needed.
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
            conn.execute_batch(migration.sql)?;
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

// -- Postgres ----------------------------------------------------------------

pub async fn apply_postgres(client: &mut tokio_postgres::Client) -> StoreResult<()> {
    client
        .batch_execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at BIGINT NOT NULL
            )",
        )
        .await?;

    let applied = applied_versions_postgres(client).await?;

    for migration in migrations() {
        if applied.contains(migration.version) {
            continue;
        }
        let transaction = client.transaction().await?;
        transaction.batch_execute(migration.sql).await?;
        transaction
            .execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES ($1, $2)",
                &[&migration.version, &now_unix()],
            )
            .await?;
        transaction.commit().await?;
    }

    Ok(())
}

async fn applied_versions_postgres(
    client: &tokio_postgres::Client,
) -> StoreResult<HashSet<String>> {
    // Check if table exists
    let exists: bool = client
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'schema_migrations')",
            &[],
        )
        .await?
        .get(0);

    if !exists {
        return Ok(HashSet::new());
    }

    let rows = client
        .query("SELECT version FROM schema_migrations", &[])
        .await?;
    let versions = rows.into_iter().map(|row| row.get(0)).collect();
    Ok(versions)
}

/// Latest applied migration version via a raw client, or empty string if none applied.
pub async fn latest_version_postgres_client(
    client: &tokio_postgres::Client,
) -> StoreResult<String> {
    latest_version_postgres_query(client).await
}

/// Latest applied migration version via a connection pool, or empty string if none applied.
pub async fn latest_version_postgres_pool(pool: &deadpool_postgres::Pool) -> StoreResult<String> {
    let client = pool
        .get()
        .await
        .map_err(|e| crate::lfd::store::StoreError::InvalidData(format!("pool error: {e}")))?;
    latest_version_postgres_query(&**client).await
}

async fn latest_version_postgres_query(
    client: &(impl tokio_postgres::GenericClient + Sync),
) -> StoreResult<String> {
    // Check if table exists
    let exists: bool = client
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = 'schema_migrations')",
            &[],
        )
        .await?
        .get(0);

    if !exists {
        return Ok(String::new());
    }

    let rows = client
        .query("SELECT version FROM schema_migrations", &[])
        .await?;
    let applied: HashSet<String> = rows.into_iter().map(|row| row.get(0)).collect();
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
        for expected in ["waves", "sessions", "repos", "triggers", "wave_runs"] {
            assert!(
                tables.iter().any(|t| t == expected),
                "expected table {expected} not found; tables: {tables:?}"
            );
        }

        for unexpected in ["chords", "chord_members"] {
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
}
