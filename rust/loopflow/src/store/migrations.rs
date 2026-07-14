//! Release-scoped schema migrations. See `MIGRATIONS.md` next to this file for
//! the convention; the one rule is that a shipped migration is never edited.

use std::fmt;

use crate::store::{StoreError, StoreResult};

// -- Identity -----------------------------------------------------------------

/// A migration's release-scoped identity: `{major}.{minor}.{ordinal:03}`.
///
/// The namespace is the package major.minor when the migration is authored;
/// patch releases append into the same namespace. Ordering is the numeric tuple,
/// never a string sort — `0.9.001` precedes `0.10.001`, which lexical order
/// would invert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MigrationId {
    pub major: u32,
    pub minor: u32,
    pub ordinal: u32,
}

impl MigrationId {
    /// The id leading a canonical version string (`0.10.001_initial`), or `None`
    /// if the string does not carry a release-scoped id at all — which is how a
    /// ledger row from the pre-namespace era is told apart from a future release.
    fn parse_version(version: &str) -> Option<Self> {
        let (id, _name) = version.split_once('_')?;
        let mut parts = id.split('.');
        let mut number = || parts.next()?.parse().ok();
        let (major, minor, ordinal) = (number()?, number()?, number()?);
        if parts.next().is_some() {
            return None;
        }
        Some(MigrationId {
            major,
            minor,
            ordinal,
        })
    }
}

impl fmt::Display for MigrationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{:03}", self.major, self.minor, self.ordinal)
    }
}

/// One migration file. `version()` is the canonical string recorded in
/// `schema_migrations` and is exactly the file stem, so renaming a shipped file
/// is a schema break rather than a cosmetic edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub id: MigrationId,
    pub name: &'static str,
    pub sql: &'static str,
}

impl Migration {
    pub fn version(&self) -> String {
        format!("{}_{}", self.id, self.name)
    }
}

/// Every migration, in id order. A new one is appended here and to the
/// directory; `scripts/new_migration.py` picks the next free ordinal in the
/// active namespace.
const MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            ordinal: 1,
        },
        name: "initial",
        sql: include_str!("migrations/0.10.001_initial.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            ordinal: 2,
        },
        name: "session_execution_context",
        sql: include_str!("migrations/0.10.002_session_execution_context.sql"),
    },
];

/// Databases written before release-scoped ids stamped the baseline under this
/// name. The file is byte-identical to `0.10.001_initial.sql`, so adoption is a
/// bookkeeping rename, not a schema change.
const LEGACY_BASELINE_VERSION: &str = "001_initial";

const RECREATE_MESSAGE: &str =
    "incompatible Loopflow database; delete loopflow.db and rerun the command";
const NEWER_MESSAGE: &str =
    "loopflow.db was written by a newer Loopflow; upgrade lf to open this database";

/// The major.minor a migration authored today belongs to, from the single version
/// source of truth (the workspace `Cargo.toml`, via Cargo).
///
/// # Panics
///
/// Panics if the package version is not `major.minor.patch`, which Cargo rejects
/// long before this runs.
pub fn active_namespace() -> (u32, u32) {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    let major = parts.next().and_then(|part| part.parse().ok());
    let minor = parts.next().and_then(|part| part.parse().ok());
    match (major, minor) {
        (Some(major), Some(minor)) => (major, minor),
        _ => panic!("package version {version} is not major.minor.patch"),
    }
}

// -- Applying -----------------------------------------------------------------

pub fn apply_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    conn.execute_batch("BEGIN EXCLUSIVE")?;
    let result = apply_set(conn, MIGRATIONS);
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

/// Apply every migration in `set` the database has not seen, in id order, exactly
/// once. Taking the set as an argument is what lets the fixtures below drive
/// synthetic release histories without a test-only seam in the store.
fn apply_set(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    validate_set(set).map_err(StoreError::InvalidData)?;

    let tables = user_tables(conn)?;
    if !tables.iter().any(|table| table == "schema_migrations") {
        // Product tables with no ledger were never written by us.
        if !tables.is_empty() {
            return Err(incompatible());
        }
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );",
        )?;
    }

    adopt_legacy_baseline(conn, set)?;

    let applied = applied_versions(conn)?;
    for migration in pending_migrations(&applied, set)? {
        conn.execute_batch(migration.sql)?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, unixepoch())",
            [migration.version()],
        )?;
    }

    validate_schema(conn, set)
}

/// Rewrite a pre-namespace baseline stamp to its release-scoped id. The bytes on
/// disk are the same file, so no product data moves; only the ledger row changes.
fn adopt_legacy_baseline(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    if applied_versions(conn)?.as_slice() != [LEGACY_BASELINE_VERSION] {
        return Ok(());
    }
    let Some(baseline) = set.first() else {
        return Err(incompatible());
    };
    conn.execute(
        "UPDATE schema_migrations SET version = ?1 WHERE version = ?2",
        [baseline.version(), LEGACY_BASELINE_VERSION.to_string()],
    )?;
    Ok(())
}

/// The migrations still to run, given what the database has already applied.
///
/// The applied versions must be exactly the leading run of `set`: a database that
/// skipped a migration, or that carries one this binary does not know, cannot be
/// brought forward by running the tail.
///
/// An unknown id that *is* release-scoped came from a newer Loopflow. An unknown
/// id that is not — the flat ledger of the pre-loop store, abandoned when the
/// runtime collapsed to waves, projects, and tasks — is simply not our database.
fn pending_migrations<'a>(
    applied: &[String],
    set: &'a [Migration],
) -> StoreResult<&'a [Migration]> {
    let known: Vec<String> = set.iter().map(Migration::version).collect();
    for version in applied {
        if known.contains(version) {
            continue;
        }
        return match MigrationId::parse_version(version) {
            Some(_) => Err(StoreError::InvalidData(NEWER_MESSAGE.to_string())),
            None => Err(incompatible()),
        };
    }
    if applied.len() > known.len() || *applied != known[..applied.len()] {
        return Err(incompatible());
    }
    Ok(&set[applied.len()..])
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

/// Applied versions in the order they were applied. Sorting by the version string
/// would be wrong — `0.10.001` sorts before `0.9.001` lexically — and the ledger
/// records application order, which is the order that matters.
fn applied_versions(conn: &rusqlite::Connection) -> StoreResult<Vec<String>> {
    let mut statement =
        conn.prepare("SELECT version FROM schema_migrations ORDER BY applied_at, rowid")?;
    let rows = statement.query_map([], |row| row.get(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// -- Validation ---------------------------------------------------------------

/// Ids strictly increase, so a duplicate and an out-of-order id are the same
/// error. A malformed set is a programmer error, caught by the tests and by
/// `scripts/check_migrations.py` — never a condition a user can create.
pub fn validate_set(set: &[Migration]) -> Result<(), String> {
    let mut previous: Option<&Migration> = None;
    for migration in set {
        if let Some(previous) = previous {
            if migration.id <= previous.id {
                return Err(format!(
                    "migration {} does not come after {}",
                    migration.version(),
                    previous.version()
                ));
            }
        }
        previous = Some(migration);
    }
    Ok(())
}

/// A database matches only if every product table and every column matches the
/// schema the migration chain builds. Comparing table names alone lets a database
/// built from an older edit of a migration open cleanly and then fail each query
/// with a raw `no such column` error instead of the recreate message.
fn validate_schema(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    let expected = rusqlite::Connection::open_in_memory()?;
    for migration in set {
        expected.execute_batch(migration.sql)?;
    }

    if product_schema(conn)? != product_schema(&expected)? {
        return Err(incompatible());
    }
    Ok(())
}

/// Every product table with its columns, in declaration order. `schema_migrations`
/// is bookkeeping rather than product schema, so no migration declares it.
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

/// The highest migration the database has applied — a release-scoped version
/// string such as `0.10.001_initial`.
pub fn latest_version_sqlite(conn: &rusqlite::Connection) -> StoreResult<String> {
    let applied = applied_versions(conn)?;
    if !pending_migrations(&applied, MIGRATIONS)?.is_empty() {
        return Err(incompatible());
    }
    validate_schema(conn, MIGRATIONS)?;
    applied.last().cloned().ok_or_else(incompatible)
}

fn incompatible() -> StoreError {
    StoreError::InvalidData(RECREATE_MESSAGE.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        active_namespace, applied_versions, apply_set, apply_sqlite, latest_version_sqlite,
        product_schema, validate_set, Migration, MigrationId, MIGRATIONS,
    };

    /// Stand-ins for the releases that have not happened yet: one more migration
    /// in the baseline's minor, and the first of the next minor.
    const SECOND_IN_SAME_MINOR: Migration = Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            ordinal: 2,
        },
        name: "add_note",
        sql: "ALTER TABLE waves ADD COLUMN note TEXT;",
    };
    const FIRST_IN_NEXT_MINOR: Migration = Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 1,
        },
        name: "add_colour",
        sql: "ALTER TABLE waves ADD COLUMN colour TEXT;",
    };

    fn open() -> rusqlite::Connection {
        rusqlite::Connection::open_in_memory().unwrap()
    }

    fn baseline() -> Migration {
        MIGRATIONS[0]
    }

    fn columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        product_schema(conn)
            .unwrap()
            .into_iter()
            .find(|(name, _)| name == table)
            .map(|(_, columns)| columns)
            .unwrap()
    }

    #[test]
    fn the_shipped_set_is_ordered_and_within_the_active_namespace() {
        validate_set(MIGRATIONS).unwrap();

        let active = active_namespace();
        for migration in MIGRATIONS {
            assert!(
                (migration.id.major, migration.id.minor) <= active,
                "{} is namespaced ahead of the package version",
                migration.version()
            );
        }
    }

    /// The directory and the registered set are two hands that must stay in sync:
    /// a file added without registering it would never run.
    #[test]
    fn every_migration_file_is_registered_under_its_own_name() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/store/migrations");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        on_disk.sort();

        let mut registered: Vec<String> = MIGRATIONS.iter().map(Migration::version).collect();
        registered.sort();

        assert_eq!(on_disk, registered);
    }

    #[test]
    fn a_fresh_database_applies_the_whole_chain_once() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_sqlite(&conn).unwrap();

        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            "0.10.002_session_execution_context"
        );
        assert!(product_schema(&conn)
            .unwrap()
            .iter()
            .any(|(table, _)| table == "task_sessions"));

        apply_sqlite(&conn).unwrap();
        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![
                "0.10.001_initial".to_string(),
                "0.10.002_session_execution_context".to_string()
            ]
        );
    }

    /// The upgrade every shipped database takes: a pre-namespace `001_initial`
    /// stamp is adopted, and everything released after it runs on top.
    #[test]
    fn a_legacy_baseline_database_is_adopted_and_upgraded_without_data_loss() {
        let conn = open();
        apply_set(&conn, &[baseline()]).unwrap();
        conn.execute(
            "UPDATE schema_migrations SET version = '001_initial' WHERE version = ?1",
            [baseline().version()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at) VALUES ('w1', 'infra', '/repo', 1)",
            [],
        )
        .unwrap();

        apply_set(
            &conn,
            &[baseline(), SECOND_IN_SAME_MINOR, FIRST_IN_NEXT_MINOR],
        )
        .unwrap();

        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![
                "0.10.001_initial",
                "0.10.002_add_note",
                "0.11.001_add_colour"
            ]
        );
        let name: String = conn
            .query_row("SELECT name FROM waves WHERE id = 'w1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(name, "infra");
    }

    #[test]
    fn several_migrations_in_one_minor_release_apply_in_order() {
        let conn = open();
        apply_set(&conn, &[baseline(), SECOND_IN_SAME_MINOR]).unwrap();

        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec!["0.10.001_initial", "0.10.002_add_note"]
        );
        assert!(columns(&conn, "waves").contains(&"note".to_string()));
    }

    /// The release boundary: a database at the end of 0.10 takes only the 0.11
    /// tail, and takes it exactly once.
    #[test]
    fn a_database_from_the_previous_minor_applies_only_the_new_namespace() {
        let conn = open();
        apply_set(&conn, &[baseline(), SECOND_IN_SAME_MINOR]).unwrap();

        let next = [baseline(), SECOND_IN_SAME_MINOR, FIRST_IN_NEXT_MINOR];
        apply_set(&conn, &next).unwrap();
        apply_set(&conn, &next).unwrap();

        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![
                "0.10.001_initial",
                "0.10.002_add_note",
                "0.11.001_add_colour"
            ]
        );
        assert_eq!(
            columns(&conn, "waves")
                .iter()
                .filter(|column| *column == "colour")
                .count(),
            1
        );
    }

    /// Downgrade: the database ran a migration this binary has never heard of.
    #[test]
    fn a_database_from_a_newer_loopflow_asks_for_an_upgrade() {
        let conn = open();
        apply_set(&conn, &[baseline(), FIRST_IN_NEXT_MINOR]).unwrap();

        let error = apply_set(&conn, &[baseline()]).unwrap_err();
        assert!(error.to_string().contains("upgrade lf"));
    }

    /// The pre-loop store's flat ledger (`001_initial`, `002_...`, …) was abandoned
    /// when the runtime collapsed to waves, projects, and tasks. Its databases are
    /// not ours to upgrade, and telling their owners to upgrade `lf` would send
    /// them in a circle.
    #[test]
    fn a_database_from_the_abandoned_flat_ledger_tells_the_user_to_recreate() {
        let conn = open();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
             );
             INSERT INTO schema_migrations VALUES ('001_initial', 1);
             INSERT INTO schema_migrations VALUES ('002_stimulus_enabled', 2);
             CREATE TABLE waves (id TEXT PRIMARY KEY, workers INTEGER NOT NULL);",
        )
        .unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert!(error.to_string().contains("delete loopflow.db"));
    }

    #[test]
    fn a_stale_edit_of_a_shipped_migration_tells_the_user_to_recreate() {
        let conn = open();
        apply_sqlite(&conn).unwrap();
        conn.execute_batch("ALTER TABLE task_sessions DROP COLUMN project_prompt_context")
            .unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert!(error.to_string().contains("delete loopflow.db"));
    }

    #[test]
    fn a_skipped_migration_tells_the_user_to_recreate() {
        let conn = open();
        apply_set(&conn, &[baseline()]).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES ('0.11.001_add_colour', 2)",
            [],
        )
        .unwrap();

        let error = apply_set(
            &conn,
            &[baseline(), SECOND_IN_SAME_MINOR, FIRST_IN_NEXT_MINOR],
        )
        .unwrap_err();
        assert!(error.to_string().contains("delete loopflow.db"));
    }

    #[test]
    fn fresh_on_disk_database_reopens_at_the_latest_version() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");

        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            apply_sqlite(&conn).unwrap();
        }

        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_sqlite(&conn).unwrap();
        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            "0.10.002_session_execution_context"
        );
    }

    #[test]
    fn unmarked_existing_schema_is_never_adopted() {
        let conn = open();
        conn.execute_batch("CREATE TABLE waves (id TEXT PRIMARY KEY)")
            .unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert!(error.to_string().contains("delete loopflow.db"));
    }

    #[test]
    fn a_repeated_id_is_rejected_before_anything_runs() {
        let error = validate_set(&[baseline(), baseline()]).unwrap_err();
        assert!(error.contains("does not come after"));
    }

    #[test]
    fn ids_order_numerically_rather_than_lexically() {
        let id = |major, minor, ordinal| MigrationId {
            major,
            minor,
            ordinal,
        };
        assert!(id(0, 9, 1) < id(0, 10, 1));
        assert!(id(0, 10, 2) < id(0, 11, 1));
        assert_eq!(id(0, 10, 1).to_string(), "0.10.001");
    }

    #[test]
    fn only_release_scoped_versions_parse() {
        assert_eq!(
            MigrationId::parse_version("0.10.001_initial"),
            Some(MigrationId {
                major: 0,
                minor: 10,
                ordinal: 1,
            })
        );
        assert_eq!(MigrationId::parse_version("001_initial"), None);
        assert_eq!(MigrationId::parse_version("0.10.1.2_initial"), None);
        assert_eq!(MigrationId::parse_version("0.10.001"), None);
    }
}
