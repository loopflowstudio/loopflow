use crate::store::{StoreError, StoreResult};

const BASELINE_VERSION: &str = "001_initial";
const BASELINE_SQL: &str = include_str!("migrations/001_initial.sql");
const RECREATE_MESSAGE: &str =
    "incompatible Loopflow database; delete loopflow.db and rerun the command";

const REQUIRED_TABLES: &[&str] = &[
    "agent_launches",
    "agent_turns",
    "blob_tokens",
    "bus_cursors",
    "bus_messages",
    "child_commands",
    "child_directives",
    "context_assets",
    "context_decisions",
    "observation_outbox",
    "pm_snapshots",
    "project_events",
    "project_sessions",
    "provider_tokens",
    "run_events",
    "task_events",
    "task_sessions",
    "trace_capture_meta",
    "waves",
];

pub fn apply_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    conn.execute_batch("BEGIN EXCLUSIVE")?;
    let result = apply_locked(conn);
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn apply_locked(conn: &rusqlite::Connection) -> StoreResult<()> {
    let tables = user_tables(conn)?;
    if tables.is_empty() {
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );",
        )?;
        conn.execute_batch(BASELINE_SQL)?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, unixepoch())",
            [BASELINE_VERSION],
        )?;
        return validate_baseline(conn);
    }

    if !tables.iter().any(|table| table == "schema_migrations") {
        return Err(incompatible());
    }
    let versions = applied_versions(conn)?;
    if versions.as_slice() != [BASELINE_VERSION] {
        return Err(incompatible());
    }
    validate_baseline(conn)
}

fn user_tables(conn: &rusqlite::Connection) -> StoreResult<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn applied_versions(conn: &rusqlite::Connection) -> StoreResult<Vec<String>> {
    let mut statement = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn validate_baseline(conn: &rusqlite::Connection) -> StoreResult<()> {
    let tables = user_tables(conn)?;
    if tables.len() != REQUIRED_TABLES.len() + 1
        || REQUIRED_TABLES
            .iter()
            .any(|required| !tables.iter().any(|table| table == required))
    {
        return Err(incompatible());
    }

    let wave_columns = table_columns(conn, "waves")?;
    if wave_columns != ["id", "name", "repo", "created_at", "parent_wave_id"] {
        return Err(incompatible());
    }
    Ok(())
}

fn table_columns(conn: &rusqlite::Connection, table: &str) -> StoreResult<Vec<String>> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get(1))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn latest_version_sqlite(conn: &rusqlite::Connection) -> StoreResult<String> {
    let versions = applied_versions(conn)?;
    if versions.as_slice() != [BASELINE_VERSION] {
        return Err(incompatible());
    }
    validate_baseline(conn)?;
    Ok(BASELINE_VERSION.to_string())
}

fn incompatible() -> StoreError {
    StoreError::InvalidData(RECREATE_MESSAGE.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_database_has_only_the_live_product_schema() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_sqlite(&conn).unwrap();

        assert_eq!(latest_version_sqlite(&conn).unwrap(), BASELINE_VERSION);
        let tables = user_tables(&conn).unwrap();
        for table in REQUIRED_TABLES {
            assert!(tables.iter().any(|candidate| candidate == table), "{table}");
        }
        assert_eq!(tables.len(), REQUIRED_TABLES.len() + 1);

        apply_sqlite(&conn).unwrap();
    }

    #[test]
    fn fresh_on_disk_database_reopens_at_the_baseline() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");

        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            apply_sqlite(&conn).unwrap();
        }

        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_sqlite(&conn).unwrap();
        assert_eq!(latest_version_sqlite(&conn).unwrap(), BASELINE_VERSION);
    }

    #[test]
    fn incompatible_history_tells_the_user_to_recreate() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
             );
             INSERT INTO schema_migrations VALUES ('001_initial', 0);
             CREATE TABLE waves (id TEXT PRIMARY KEY, workers INTEGER NOT NULL);",
        )
        .unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert!(error.to_string().contains("delete loopflow.db"));
    }

    #[test]
    fn unmarked_existing_schema_is_never_adopted() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE waves (id TEXT PRIMARY KEY)")
            .unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert!(error.to_string().contains("delete loopflow.db"));
    }
}
