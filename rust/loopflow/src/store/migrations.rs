use crate::store::{StoreError, StoreResult};

const BASELINE_VERSION: &str = "001_initial";
const BASELINE_SQL: &str = include_str!("migrations/001_initial.sql");
const RECREATE_MESSAGE: &str =
    "incompatible Loopflow database; delete loopflow.db and rerun the command";

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

/// A database matches the baseline only if every product table and every column
/// matches `001_initial.sql`. Comparing table names alone lets a database built
/// from an older edit of the baseline open cleanly and then fail each query with
/// a raw `no such column` error instead of the recreate message.
fn validate_baseline(conn: &rusqlite::Connection) -> StoreResult<()> {
    let baseline = rusqlite::Connection::open_in_memory()?;
    baseline.execute_batch(BASELINE_SQL)?;

    if product_schema(conn)? != product_schema(&baseline)? {
        return Err(incompatible());
    }
    Ok(())
}

/// Every product table with its columns, in declaration order. `schema_migrations`
/// is bookkeeping rather than product schema, so the baseline SQL never declares it.
fn product_schema(conn: &rusqlite::Connection) -> StoreResult<Vec<(String, Vec<String>)>> {
    let mut schema = Vec::new();
    for table in user_tables(conn)? {
        if table == "schema_migrations" {
            continue;
        }
        let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = statement.query_map([], |row| row.get(1))?;
        schema.push((table, rows.collect::<Result<Vec<_>, _>>()?));
    }
    Ok(schema)
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
        assert!(product_schema(&conn)
            .unwrap()
            .iter()
            .any(|(table, _)| table == "task_sessions"));

        apply_sqlite(&conn).unwrap();
    }

    /// A database written by an earlier edit of `001_initial` carries the right
    /// table names and version row, so only column comparison catches it.
    #[test]
    fn a_stale_edit_of_the_baseline_tells_the_user_to_recreate() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_sqlite(&conn).unwrap();
        conn.execute_batch("ALTER TABLE task_sessions DROP COLUMN project_prompt_context")
            .unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert!(error.to_string().contains("delete loopflow.db"));
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
