//! Schema migrations. See `MIGRATIONS.md` next to this file for the
//! convention; the one rule is that a merged migration is never edited.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

pub use super::migration_catalog::{Migration, MigrationId};
use super::migration_schema::{product_schema, ForeignKeyDefinition, ProductSchemaObject};
use crate::store::sqlite::SQLITE_WRITE_BUSY_TIMEOUT;
use crate::store::{StoreError, StoreResult};
use fs2::FileExt;
use rusqlite::OptionalExtension;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

// The catalog `build.rs` generates: released batches, then later migrations in
// the order origin/main added them, then this checkout's unmerged ones.
include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

/// The SQL of a migration named by its slug, or of the released batch whose
/// `-- draft: <name>` section carries it.
#[doc(hidden)]
pub fn migration_sql_for_test(name: &str) -> String {
    let marker = format!("-- draft: {name}");
    MIGRATIONS
        .iter()
        .find(|migration| {
            migration.version == name || migration.sql.lines().any(|line| line == marker)
        })
        .expect("migration is in the catalog")
        .sql
        .to_string()
}

/// The slug migrations this build carries that origin/main has not added.
pub fn unmerged_migrations() -> Vec<&'static str> {
    MIGRATIONS
        .iter()
        .filter(|migration| !migration.merged)
        .map(|migration| migration.version)
        .collect()
}

#[cfg(test)]
pub(crate) fn migration_is_applied_for_test(
    conn: &rusqlite::Connection,
    name: &str,
) -> StoreResult<bool> {
    let marker = format!("-- draft: {name}");
    let Some(migration) = MIGRATIONS.iter().find(|migration| {
        migration.version == name || migration.sql.lines().any(|line| line == marker)
    }) else {
        return Ok(false);
    };
    Ok(applied_versions(conn)?
        .iter()
        .any(|version| version == &migration.version()))
}

#[cfg(test)]
pub(crate) fn apply_released_planning_fixture(conn: &rusqlite::Connection) {
    let count = MIGRATIONS
        .iter()
        .take_while(|m| {
            MigrationId::parse_version(m.version)
                .is_some_and(|id| (id.major, id.minor, id.patch.unwrap_or(0)) <= (0, 12, 29))
        })
        .count();
    apply_set(conn, &MIGRATIONS[..count], &mut Vec::new()).unwrap();
}

#[cfg(test)]
pub(crate) use tests::{
    apply_before_migration, apply_pending_migrations, migration_section_sql, upgrade_from_migration,
};

/// The exact branch-local history that reached one production ledger before
/// main established `0.11.008_interactive_handoffs`. These ids were never
/// released. They remain here only long enough to recognize and converge that
/// known history without treating arbitrary unknown migrations as ours.
const DIVERGENT_MIGRATIONS: &[Migration] = &[
    Migration {
        version: "0.11.008_context_pressure",
        sql: include_str!("migrations/0.11.009_context_pressure.sql"),
        reader_floor: "0.11.008_context_pressure",
        merged: true,
    },
    Migration {
        version: "0.11.009_context_input_normalization",
        sql: include_str!("migrations/0.11.010_context_input_normalization.sql"),
        reader_floor: "0.11.009_context_input_normalization",
        merged: true,
    },
    Migration {
        version: "0.11.010_profiles",
        sql: include_str!("migrations/0.11.011_profiles.sql"),
        reader_floor: "0.11.010_profiles",
        merged: true,
    },
    Migration {
        version: "0.11.011_provider_account_lifecycle",
        sql: include_str!("migrations/0.11.012_provider_account_lifecycle.sql"),
        reader_floor: "0.11.011_provider_account_lifecycle",
        merged: true,
    },
];

const CONVERGED_VERSIONS: &[&str] = &[
    "0.11.008_interactive_handoffs",
    "0.11.009_context_pressure",
    "0.11.010_context_input_normalization",
    "0.11.011_profiles",
    "0.11.012_provider_account_lifecycle",
];

/// Databases written before release-scoped ids stamped the baseline under this
/// name. The file is byte-identical to `0.10.001_initial.sql`, so adoption is a
/// bookkeeping rename, not a schema change.
const LEGACY_BASELINE_VERSION: &str = "001_initial";

// -- Applying -----------------------------------------------------------------

pub fn apply_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    apply_sqlite_transaction(conn, |_| Ok(()))
}

/// Stage a fresh connection one migration behind the binary's known head. The
/// store-level shared-frontier regressions use it to build a database the running
/// binary could advance but an ordinary open must leave alone.
#[cfg(test)]
pub(crate) fn apply_all_but_head(conn: &rusqlite::Connection) -> StoreResult<()> {
    apply_set(conn, &MIGRATIONS[..MIGRATIONS.len() - 1], &mut Vec::new())
}

#[cfg(test)]
pub(crate) fn prior_known_version() -> String {
    MIGRATIONS[MIGRATIONS.len() - 2].version()
}

/// Stand in for a newer build: apply one more migration after this binary's
/// head, as the build that knows it would, and return its version. This binary
/// is then the older reader. An additive migration keeps the current floor.
#[cfg(test)]
pub(crate) fn apply_newer_migration(
    conn: &rusqlite::Connection,
    sql: &'static str,
    additive: bool,
) -> StoreResult<String> {
    let head = MIGRATIONS.last().expect("the catalog is not empty");
    let newer = Migration {
        version: "newer_migration",
        sql,
        reader_floor: if additive {
            head.reader_floor
        } else {
            "newer_migration"
        },
        merged: true,
    };
    let set: Vec<Migration> = MIGRATIONS.iter().copied().chain([newer]).collect();
    apply_set(conn, &set, &mut Vec::new())?;
    Ok(newer.version())
}

/// Whether the old reader — a binary whose head is the prior migration — still
/// recognizes the store as exactly its own frontier (nothing pending). It is
/// `false` if the store was advanced to the newer head, which is what makes the
/// shared-frontier regression sabotage-sensitive.
#[cfg(test)]
pub(crate) fn old_reader_recognizes(conn: &rusqlite::Connection) -> bool {
    let prior = &MIGRATIONS[..MIGRATIONS.len() - 1];
    applied_versions(conn)
        .and_then(|applied| pending_migrations(&applied, prior).map(<[_]>::is_empty))
        .unwrap_or(false)
}

/// Validate the schema this binary already understands without advancing it.
/// Every ordinary open of the Machine's shared store runs this: any build may
/// reuse a store it can read, but only the install switch advances it.
pub(crate) fn validate_sqlite_schema(conn: &rusqlite::Connection) -> StoreResult<()> {
    _read_snapshot(conn, |conn| {
        validate_set(MIGRATIONS).map_err(StoreError::InvalidData)?;
        if !has_ledger(conn)? {
            return Err(StoreError::InvalidData(
                "an ordinary lf cannot initialize the shared store; install lf with `lf self install`"
                    .to_string(),
            ));
        }
        let applied = applied_versions(conn)?;
        let newer = admitted_newer_floor(conn, &applied, MIGRATIONS)?;
        validate_applied_checksums(conn, MIGRATIONS)?;
        if newer.is_some() {
            validate_schema_within(conn, MIGRATIONS)
        } else {
            validate_schema(conn, &MIGRATIONS[..applied.len()])
        }
    })
}

/// Diagnose the release database in full: its schema and every stored foreign key.
pub(crate) fn validate_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    _read_snapshot(conn, |conn| {
        validate_sqlite_schema(conn)?;
        validate_foreign_keys(conn)
    })
}

fn apply_sqlite_transaction(
    conn: &rusqlite::Connection,
    before_migration: impl FnOnce(&rusqlite::Connection) -> StoreResult<()>,
) -> StoreResult<()> {
    _migration_transaction(conn, |conn, guards| {
        before_migration(conn)?;
        apply_set(conn, MIGRATIONS, guards)?;
        validate_foreign_keys(conn)?;
        validate_persisted_json(conn)
    })
}

fn _read_snapshot<T>(
    conn: &rusqlite::Connection,
    read: impl FnOnce(&rusqlite::Connection) -> StoreResult<T>,
) -> StoreResult<T> {
    if !conn.is_autocommit() {
        return read(conn);
    }
    let transaction =
        rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Deferred)?;
    let value = read(&transaction)?;
    transaction.commit()?;
    Ok(value)
}

fn _migration_transaction(
    conn: &rusqlite::Connection,
    migrate: impl FnOnce(&rusqlite::Connection, &mut Vec<File>) -> StoreResult<()>,
) -> StoreResult<()> {
    let foreign_keys_enabled: bool =
        conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let migration_result = match conn.execute_batch("BEGIN EXCLUSIVE") {
        Ok(()) => {
            // Even a preflight or validation error retains acquired OS fences
            // until SQLite has rolled back. Waiting dispatchers then re-read authority.
            let mut guards = Vec::new();
            let result = migrate(conn, &mut guards)
                .and_then(|()| conn.execute_batch("COMMIT").map_err(StoreError::from));
            if result.is_err() {
                let _ = conn.execute_batch("ROLLBACK");
            }
            result
        }
        Err(error) => Err(StoreError::from(error)),
    };
    let restore_result = if foreign_keys_enabled {
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(StoreError::from)
    } else {
        Ok(())
    };
    match migration_result {
        Err(error) => Err(error),
        Ok(()) => restore_result,
    }
}

pub(crate) fn validate_persisted_json(conn: &rusqlite::Connection) -> StoreResult<()> {
    // Dashboard intent is introduced by the current draft; older prefixes omit it.
    let has_dashboards: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='dashboards')",
        [],
        |row| row.get(0),
    )?;
    // Released stores still carry this field until delivery receipts take ownership.
    let mut failures = if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('tasks') WHERE name='pm_writeback_json')",
        [],
        |row| row.get::<_, bool>(0),
    )? {
        validate_json_column::<crate::work::task::LinearWritebackState>(
            conn,
            "tasks",
            "id",
            "pm_writeback_json",
        )?
    } else {
        Vec::new()
    };
    if has_dashboards {
        failures.extend(validate_json_column::<crate::work::selection::Selection>(
            conn,
            "dashboard_arrangements",
            "dashboard_id",
            "selection_json",
        )?);
        failures.extend(validate_json_column::<crate::work::selection::Arrangement>(
            conn,
            "dashboard_arrangements",
            "dashboard_id",
            "arrangement_json",
        )?);
        failures.extend(validate_json_column::<
            crate::work::selection::WorkPaneLayout,
        >(
            conn, "work_pane_layouts", "work_id", "layout_json"
        )?);
        failures.extend(validate_json_column::<crate::work::selection::SlotLayout>(
            conn,
            "dashboards",
            "id",
            "slots_json",
        )?);
        failures.extend(validate_json_column::<Vec<String>>(
            conn,
            "task_orders",
            "repository_id",
            "task_ids_json",
        )?);
    }
    if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('projects') WHERE name='krs')",
        [],
        |row| row.get::<_, bool>(0),
    )? {
        failures.extend(
            validate_json_column::<Vec<crate::work::project::ProjectKR>>(
                conn, "projects", "id", "krs",
            )?,
        );
        failures.extend(validate_json_column::<
            Vec<crate::work::wave::metrics::MetricTarget>,
        >(conn, "projects", "id", "metric_targets")?);
    }
    failures.extend(validate_json_column::<crate::work::task::CiObservation>(
        conn,
        "task_prs",
        "id",
        "ci_observation",
    )?);
    failures.extend(
        validate_json_column::<crate::work::task::GithubObservation>(
            conn,
            "task_prs",
            "id",
            "github_observation",
        )?,
    );
    failures.extend(validate_json_column::<crate::work::task::TaskEventKind>(
        conn,
        "task_events",
        "id",
        "kind_json",
    )?);
    failures.extend(validate_json_column::<
        crate::work::project::ProjectEventKind,
    >(conn, "project_events", "id", "kind_json")?);
    failures.extend(validate_json_column::<Vec<String>>(
        conn,
        "ci_incidents",
        "identity",
        "failure_set_json",
    )?);
    if failures.is_empty() {
        Ok(())
    } else {
        Err(StoreError::InvalidData(format!(
            "semantic migration check failed:\n  - {}",
            failures.join("\n  - ")
        )))
    }
}

fn validate_json_column<T: DeserializeOwned>(
    conn: &rusqlite::Connection,
    table: &str,
    key: &str,
    column: &str,
) -> StoreResult<Vec<String>> {
    let sql =
        format!("SELECT CAST({key} AS TEXT), {column} FROM {table} WHERE {column} IS NOT NULL");
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut failures = Vec::new();
    for row in rows {
        let (row_key, json) = row?;
        if let Err(error) = serde_json::from_str::<T>(&json) {
            failures.push(format!("{table}.{column} row {row_key}: {error}"));
        }
    }
    Ok(failures)
}

pub(super) fn migration_lock(path: &Path) -> StoreResult<std::fs::File> {
    let lock_path = path.with_file_name(format!(
        "{}.migration.lock",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("loopflow.db")
    ));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)
        .map_err(|error| StoreError::InvalidData(format!("open migration lock: {error}")))?;
    lock.lock_exclusive()
        .map_err(|error| StoreError::InvalidData(format!("acquire migration lock: {error}")))?;
    Ok(lock)
}

pub(crate) fn apply_sqlite_with_backup(
    conn: &rusqlite::Connection,
    path: &Path,
) -> StoreResult<()> {
    conn.busy_timeout(SQLITE_WRITE_BUSY_TIMEOUT)?;
    let lock = migration_lock(path)?;
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    let result = match requires_migration_sqlite(conn) {
        Ok(false) => Ok(()),
        Ok(true) => {
            let migrated = apply_sqlite_transaction(conn, |conn| {
                backup_before_migration(conn, path).map(|_| ())
            });
            if migrated.is_ok() {
                prune_migration_backups(path, 1);
            }
            migrated
        }
        Err(error) => Err(error),
    };
    let unlock = lock
        .unlock()
        .map_err(|error| StoreError::InvalidData(format!("release migration lock: {error}")));
    match result {
        Err(error) => Err(error),
        Ok(()) => unlock,
    }
}

fn backup_before_migration(
    conn: &rusqlite::Connection,
    path: &Path,
) -> StoreResult<Option<PathBuf>> {
    if !requires_migration_sqlite(conn)? {
        return Ok(None);
    }
    // Nothing applied: no previous generation to preserve, and the fingerprint
    // below reads `schema_migrations`, which does not exist yet.
    let Some(previous) = latest_applied_version_sqlite(conn)? else {
        return Ok(None);
    };
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("loopflow.db");
    let safe_version = previous
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let history = migration_history_fingerprint(conn)?;
    let backup_path = path.with_file_name(format!(
        "{file_name}.backup-{safe_version}-{}",
        &history[..16]
    ));

    let unique = format!(
        "{file_name}.backup-{safe_version}.tmp-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    );
    let temporary_path = path.with_file_name(unique);
    // An APFS clone shares blocks, so the backup costs nothing at any size.
    // Folding its WAL in keeps the published generation one file.
    let sealed = super::copy::snapshot(path, &temporary_path).and_then(|()| {
        let backup = rusqlite::Connection::open(&temporary_path)?;
        backup.query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))?;
        Ok(())
    });
    if let Err(error) = sealed {
        remove_with_sidecars(&temporary_path);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary_path, &backup_path) {
        return Err(StoreError::InvalidData(format!(
            "failed to atomically publish migration backup (existing backup and {} were preserved): {error}",
            temporary_path.display()
        )));
    }
    #[cfg(unix)]
    if let Some(parent) = backup_path.parent() {
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to sync migration backup directory: {error}"
                ))
            })?;
    }
    Ok(Some(backup_path))
}

/// Each release that migrates clones the store first. Once a migration commits
/// only the generation it made stays, and once its install settles none does
/// (`kept` 0). Names this writer did not produce are left alone, and a failed
/// removal is retried by the next migration or settlement.
pub(crate) fn prune_migration_backups(path: &Path, kept: usize) {
    let (Some(parent), Some(file_name)) = (
        path.parent(),
        path.file_name().and_then(|name| name.to_str()),
    ) else {
        return;
    };
    let prefix = format!("{file_name}.backup-");
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    let mut backups: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let Some(generation) = name.to_str().and_then(|name| name.strip_prefix(&prefix)) else {
                return false;
            };
            generation.rsplit_once('-').is_some_and(|(_, fingerprint)| {
                fingerprint.len() == 16
                    && fingerprint
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            })
        })
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .collect();
    backups.sort();
    let superseded = backups.len().saturating_sub(kept);
    for (_, backup) in backups.into_iter().take(superseded) {
        if let Err(error) = std::fs::remove_file(&backup) {
            tracing::warn!(%error, backup = %backup.display(), "superseded migration backup was not removed");
            continue;
        }
        remove_with_sidecars(&backup);
    }
}

fn remove_with_sidecars(path: &Path) {
    let _ = std::fs::remove_file(path);
    for sidecar in ["-shm", "-wal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(sidecar);
        let _ = std::fs::remove_file(name);
    }
}

fn migration_history_fingerprint(conn: &rusqlite::Connection) -> StoreResult<String> {
    let mut digest = Sha256::new();
    let versions = applied_versions(conn)?;
    digest.update((versions.len() as u64).to_be_bytes());
    for version in versions {
        hash_text(&mut digest, &version);
    }
    let schema = product_schema(conn)?;
    digest.update((schema.len() as u64).to_be_bytes());
    for object in schema.iter() {
        hash_text(&mut digest, &object.object_type);
        hash_text(&mut digest, &object.name);
        hash_text(&mut digest, &object.table_name);
        hash_text(&mut digest, &object.sql);
        digest.update((object.foreign_keys.len() as u64).to_be_bytes());
        for foreign_key in &object.foreign_keys {
            digest.update(foreign_key.id.to_be_bytes());
            digest.update(foreign_key.sequence.to_be_bytes());
            hash_text(&mut digest, &foreign_key.table);
            hash_text(&mut digest, &foreign_key.from);
            match &foreign_key.to {
                Some(to) => {
                    digest.update([1]);
                    hash_text(&mut digest, to);
                }
                None => digest.update([0]),
            }
            hash_text(&mut digest, &foreign_key.on_update);
            hash_text(&mut digest, &foreign_key.on_delete);
            hash_text(&mut digest, &foreign_key.match_clause);
        }
    }
    Ok(hex::encode(digest.finalize()))
}

fn hash_text(digest: &mut Sha256, value: &str) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

/// Scan every stored row for a dangling reference. This reads the whole
/// database, so opening a store never runs it: every connection enforces
/// foreign keys and only a migration can violate them. Migrations check before
/// they commit; `lf doctor` and installation preflight diagnose in full.
fn validate_foreign_keys(conn: &rusqlite::Connection) -> StoreResult<()> {
    let mut statement = conn.prepare("PRAGMA foreign_key_check")?;
    if statement.query([])?.next()?.is_some() {
        return Err(StoreError::InvalidData(
            "migration produced invalid foreign keys".to_string(),
        ));
    }
    Ok(())
}

/// Apply every migration in `set` the database has not seen, in id order, exactly
/// once. Taking the set as an argument is what lets the fixtures below drive
/// synthetic release histories without a test-only seam in the store.
fn apply_set(
    conn: &rusqlite::Connection,
    set: &[Migration],
    guards: &mut Vec<File>,
) -> StoreResult<()> {
    validate_set(set).map_err(StoreError::InvalidData)?;

    if !has_ledger(conn)? {
        // Product tables with no ledger were never written by us.
        if user_table_count(conn)? > 0 {
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
    adopt_divergent_history(conn, set)?;
    adopt_permuted_history(conn, set)?;

    let applied = applied_versions(conn)?;
    for (offset, migration) in pending_migrations(&applied, set)?.iter().enumerate() {
        let parent_history = migration_prefix_fingerprint(&set[..applied.len() + offset]);
        migration_preflight(conn, migration, guards)?;
        conn.execute_batch(migration.sql)?;
        convert_released_data(conn, migration.sql)?;
        backfill_known_checksums(conn, set)?;
        insert_applied_migration(conn, migration, &parent_history)?;
    }

    validate_applied_checksums(conn, set)?;
    validate_schema(conn, set)
}

/// Convert stored rows whose shape a migration's SQL changes. Released
/// migrations and drafts both run it after their SQL, keyed on that SQL.
pub(crate) fn convert_released_data(conn: &rusqlite::Connection, sql: &str) -> StoreResult<()> {
    if sql.contains("CREATE TABLE planning_peer_changes (") {
        convert_project_content(conn)?;
        let mut query = conn.prepare(
            "SELECT export_json FROM project_transitions WHERE export_json IS NOT NULL
            UNION ALL SELECT export_json FROM task_creation_intents WHERE export_json IS NOT NULL",
        )?;
        for row in query.query_map([], |row| row.get::<_, String>(0))? {
            let export: super::sqlite::planning_export::PlanningExport =
                serde_json::from_str(&row?)?;
            for value in export.baseline {
                value.validate()?;
            }
        }
        let mut query = conn.prepare("SELECT value_json,base_json,conflict_json FROM task_changes WHERE field!='deleted'
            UNION ALL SELECT value_json,base_json,conflict_json FROM project_changes WHERE field!='task_order'")?;
        for row in query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (value, base, conflict) = row?;
            serde_json::from_str::<crate::work::edit::WorkValue>(&value)?.validate()?;
            for retained in [base, conflict].into_iter().flatten() {
                let baseline: super::planning_sync::PlanningBaseline =
                    serde_json::from_str(&retained)?;
                if let Some(value) = baseline.value {
                    value.validate()?;
                }
            }
        }
    }
    if sql.contains("CREATE TABLE planning_peer_changes (")
        || sql.contains("CREATE TABLE delegated_placements (")
        || sql.contains("ALTER TABLE waves ADD COLUMN workflow TEXT;")
    {
        super::sqlite::planning_write::seed(conn)?;
    }
    Ok(())
}

fn convert_project_content(conn: &rusqlite::Connection) -> StoreResult<()> {
    let mut query =
        conn.prepare("SELECT id,description FROM projects WHERE description IS NOT NULL")?;
    let rows = query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, body) in rows {
        let document = crate::linear::project_document::parse_project_document(&body)
            .map_err(|error| StoreError::InvalidData(format!("Project {id}: {error}")))?;
        conn.execute(
            "UPDATE projects SET description=?2,krs=?3,metric_targets=?4 WHERE id=?1",
            rusqlite::params![
                id,
                document.description(),
                serde_json::to_string(&document.krs)?,
                serde_json::to_string(&document.metric_targets)?
            ],
        )?;
    }
    Ok(())
}

fn migration_preflight(
    conn: &rusqlite::Connection,
    migration: &Migration,
    guards: &mut Vec<File>,
) -> StoreResult<()> {
    if migration_contains(migration, "canonical_structured_ids") {
        super::canonical_ids::preflight(conn, guards)?;
    }
    if migration.sql.contains("CREATE TABLE tasks_migration (")
        && migration.sql.contains("repository_id TEXT NOT NULL")
    {
        let mut query = conn.prepare("SELECT t.id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
            LEFT JOIN waves w ON w.id=p.wave_id LEFT JOIN repository_plans r ON r.repo=w.repo AND r.selected=1
            WHERE r.id IS NULL UNION SELECT t.task_id FROM task_creation_intents t
            LEFT JOIN projects p ON p.id=t.project_id LEFT JOIN waves w ON w.id=p.wave_id
            LEFT JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE r.id IS NULL")?;
        let missing = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if !missing.is_empty() {
            return Err(StoreError::InvalidData(format!(
                "Tasks have no retained repository: {}",
                missing.join(", ")
            )));
        }
    }
    if !migration_contains(migration, "durable_input_spine") {
        return Ok(());
    }
    let mut active = Vec::new();
    for (kind, table) in [("Project", "project_sessions"), ("Task", "task_sessions")] {
        let mut statement = conn.prepare(&format!(
            "SELECT id FROM {table}
             WHERE process_lease_state IN ('reserved', 'active', 'revoked')
             ORDER BY id"
        ))?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        active.extend(ids.into_iter().map(|id| format!("{kind} {id}")));
    }
    if !active.is_empty() {
        return Err(StoreError::InvalidData(format!(
            "durable input migration requires every writer to be quiescent and reaped; active: {}",
            active.join(", ")
        )));
    }
    let ambiguous_project: Option<String> = conn
        .query_row(
            "SELECT project_id FROM project_sessions
             GROUP BY project_id HAVING COUNT(DISTINCT wave_id) > 1 LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(project) = ambiguous_project {
        return Err(StoreError::InvalidData(format!(
            "Project {project} appears under more than one Wave; repair parentage before durable input migration"
        )));
    }
    let ambiguous_task: Option<String> = conn
        .query_row(
            "SELECT issue_id FROM task_sessions
             GROUP BY issue_id HAVING COUNT(DISTINCT project_id) > 1 LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(task) = ambiguous_task {
        return Err(StoreError::InvalidData(format!(
            "Task {task} appears under more than one Project; repair parentage before durable input migration"
        )));
    }
    Ok(())
}

fn migration_contains(migration: &Migration, name: &str) -> bool {
    let marker = format!("-- draft: {name}");
    migration.version == name || migration.sql.lines().any(|line| line == marker)
}

fn migration_checksum(migration: &Migration) -> String {
    hex::encode(Sha256::digest(migration.sql.as_bytes()))
}

fn migration_prefix_fingerprint(prefix: &[Migration]) -> String {
    let mut digest = Sha256::new();
    digest.update((prefix.len() as u64).to_be_bytes());
    for migration in prefix {
        hash_text(&mut digest, &migration.version());
        hash_text(&mut digest, &migration_checksum(migration));
    }
    hex::encode(digest.finalize())
}

fn migration_ledger_has_provenance(conn: &rusqlite::Connection) -> StoreResult<bool> {
    let mut statement = conn.prepare("PRAGMA table_info(schema_migrations)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok([
        "checksum",
        "parent_history",
        "build_provenance",
        "source_identity",
        "source_revision",
        "package_version",
    ]
    .iter()
    .all(|required| columns.iter().any(|column| column == required)))
}

fn backfill_known_checksums(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    if !migration_ledger_has_provenance(conn)? {
        return Ok(());
    }
    let missing = conn
        .prepare("SELECT version FROM schema_migrations WHERE checksum IS NULL")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for version in missing {
        if let Some(migration) = set.iter().find(|migration| migration.version() == version) {
            conn.execute(
                "UPDATE schema_migrations SET checksum = ?1
                 WHERE version = ?2 AND checksum IS NULL",
                (migration_checksum(migration), version),
            )?;
        }
    }
    Ok(())
}

fn insert_applied_migration(
    conn: &rusqlite::Connection,
    migration: &Migration,
    parent_history: &str,
) -> StoreResult<()> {
    if migration_ledger_has_provenance(conn)? {
        conn.execute(
            "INSERT INTO schema_migrations (
                version, applied_at, checksum, parent_history, build_provenance,
                source_identity, source_revision, package_version
             ) VALUES (?1, unixepoch(), ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                migration.version(),
                migration_checksum(migration),
                parent_history,
                crate::build_info::provenance().as_str(),
                crate::build_info::source_identity(),
                crate::build_info::source_revision(),
                env!("CARGO_PKG_VERSION"),
            ),
        )?;
    } else {
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, unixepoch())",
            [migration.version()],
        )?;
    }
    if ledger_has_reader_floor(conn)? {
        conn.execute(
            "UPDATE schema_migrations SET reader_floor=?1 WHERE version=?2",
            (migration.reader_floor, migration.version()),
        )?;
    }
    Ok(())
}

fn validate_applied_checksums(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    if !migration_ledger_has_provenance(conn)? {
        return Ok(());
    }
    let mut statement = conn.prepare(
        "SELECT version, checksum FROM schema_migrations
         WHERE checksum IS NOT NULL ORDER BY applied_at, rowid",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (version, checksum) = row?;
        let Some(migration) = set.iter().find(|migration| migration.version() == version) else {
            // Callers check history first; only an admitted newer tail reaches here.
            continue;
        };
        if checksum != migration_checksum(migration) {
            let fix = if migration.merged {
                ""
            } else {
                "; it is unmerged schema work, so start a fresh LF_HOME"
            };
            return Err(StoreError::InvalidData(format!(
                "database migration {version} checksum does not match this lf build{fix}"
            )));
        }
    }
    Ok(())
}

/// Canonicalize a ledger whose migration names are exactly a known leading
/// prefix but whose branch-local ordinals were assigned in another order. The
/// product schema must already equal the canonical prefix before any ledger row
/// moves; migration SQL is never replayed during this repair.
fn adopt_permuted_history(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    let applied = applied_versions(conn)?;
    let Some(prefix_len) = permuted_history(&applied, set) else {
        return Ok(());
    };

    let expected = expected_schema(set[..prefix_len].iter().map(|migration| migration.sql))?;
    if *product_schema(conn)? != *expected {
        return Err(incompatible());
    }

    let mut applied_at = conn
        .prepare("SELECT applied_at FROM schema_migrations ORDER BY applied_at, rowid")?
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    applied_at.sort_unstable();

    conn.execute("DELETE FROM schema_migrations", [])?;
    for (migration, applied_at) in set[..prefix_len].iter().zip(applied_at) {
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            (migration.version(), applied_at),
        )?;
    }
    Ok(())
}

fn permuted_history(applied: &[String], set: &[Migration]) -> Option<usize> {
    if applied.len() > set.len() {
        return None;
    }
    let canonical = set[..applied.len()]
        .iter()
        .map(Migration::version)
        .collect::<Vec<_>>();
    if applied == canonical {
        return None;
    }

    let mut applied_names = applied
        .iter()
        .map(|version| {
            MigrationId::parse_version(version)?;
            version.split_once('_').map(|(_, name)| name)
        })
        .collect::<Option<Vec<_>>>()?;
    let mut canonical_names = set[..applied.len()]
        .iter()
        .map(|migration| {
            migration
                .version
                .split_once('_')
                .map_or(migration.version, |(_, name)| name)
        })
        .collect::<Vec<_>>();
    applied_names.sort_unstable();
    canonical_names.sort_unstable();
    (applied_names == canonical_names).then_some(applied.len())
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

/// Converge the one known branch-local history that reached production before
/// `0.11.008_interactive_handoffs` shipped. Product schema must exactly match
/// the claimed divergent prefix before any ledger identity is rewritten.
fn adopt_divergent_history(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    let applied = applied_versions(conn)?;
    let Some((canonical_start, divergent_len)) = divergent_history(&applied, set) else {
        return Ok(());
    };

    validate_divergent_schema(conn, set, canonical_start, divergent_len)?;

    let divergent_versions = DIVERGENT_MIGRATIONS[..divergent_len]
        .iter()
        .map(Migration::version)
        .collect::<Vec<_>>();
    let applied_at = divergent_versions
        .iter()
        .map(|version| {
            conn.query_row(
                "SELECT applied_at FROM schema_migrations WHERE version = ?1",
                [version],
                |row| row.get::<_, i64>(0),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    for version in &divergent_versions {
        conn.execute(
            "DELETE FROM schema_migrations WHERE version = ?1",
            [version],
        )?;
    }

    let interactive = &set[canonical_start];
    conn.execute_batch(interactive.sql)?;
    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        (interactive.version(), applied_at[0]),
    )?;

    for (offset, applied_at) in applied_at.into_iter().enumerate() {
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            (set[canonical_start + offset + 1].version(), applied_at),
        )?;
    }
    Ok(())
}

fn divergent_history(applied: &[String], set: &[Migration]) -> Option<(usize, usize)> {
    let canonical = set.iter().map(Migration::version).collect::<Vec<_>>();
    let canonical_start = canonical
        .iter()
        .position(|version| version == CONVERGED_VERSIONS[0])?;
    if canonical.len() < canonical_start + CONVERGED_VERSIONS.len()
        || !canonical[canonical_start..canonical_start + CONVERGED_VERSIONS.len()]
            .iter()
            .map(String::as_str)
            .eq(CONVERGED_VERSIONS.iter().copied())
        || applied.len() <= canonical_start
        || applied.len() > canonical_start + DIVERGENT_MIGRATIONS.len()
        || applied[..canonical_start] != canonical[..canonical_start]
    {
        return None;
    }

    let divergent_len = applied.len() - canonical_start;
    let divergent = DIVERGENT_MIGRATIONS[..divergent_len]
        .iter()
        .map(Migration::version)
        .collect::<Vec<_>>();
    (applied[canonical_start..] == divergent).then_some((canonical_start, divergent_len))
}

fn validate_divergent_schema(
    conn: &rusqlite::Connection,
    set: &[Migration],
    canonical_start: usize,
    divergent_len: usize,
) -> StoreResult<()> {
    let expected = expected_schema(
        set[..canonical_start]
            .iter()
            .chain(&DIVERGENT_MIGRATIONS[..divergent_len])
            .map(|migration| migration.sql),
    )?;
    if *product_schema(conn)? != *expected {
        return Err(incompatible());
    }
    Ok(())
}

/// The migrations still to run, given what the database has already applied.
///
/// The applied versions must be exactly the leading run of `set`: a database that
/// skipped a migration, or that carries one this binary does not know, cannot be
/// brought forward by running the tail.
///
/// An unknown released id or slug came from a newer lf. Anything else — the flat
/// ledger of the pre-loop store, abandoned when the runtime collapsed to waves,
/// projects, and tasks — is simply not our database.
fn pending_migrations<'a>(
    applied: &[String],
    set: &'a [Migration],
) -> StoreResult<&'a [Migration]> {
    let known: Vec<String> = set.iter().map(Migration::version).collect();
    for version in applied {
        if known.contains(version) {
            continue;
        }
        return if MigrationId::parse_version(version).is_some()
            || super::migration_catalog::is_slug(version)
        {
            Err(StoreError::InvalidData(format!(
                "database migration {version} is unknown to lf {} (latest known {}); a newer lf migrated this store; install it with `lf self install --main`",
                crate::build_info::BUILD_VERSION,
                set.last()
                    .map(Migration::version)
                    .unwrap_or_else(|| "none".to_string())
            )))
        } else {
            Err(incompatible())
        };
    }
    if applied.len() > known.len() || *applied != known[..applied.len()] {
        return Err(incompatible());
    }
    Ok(&set[applied.len()..])
}

// -- Reader floor -------------------------------------------------------------
//
// A migration's floor is the newest migration at or before it that older
// readers cannot read past. A binary opens a store holding migrations it does
// not know only when it knows every one of their floors.

fn ledger_has_reader_floor(conn: &rusqlite::Connection) -> StoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('schema_migrations') WHERE name='reader_floor')",
        [],
        |row| row.get(0),
    )?)
}

/// The floor of the newest migration the store applied that `set` does not
/// know, when `set` knows that floor; `None` when the store holds nothing
/// newer. The known part must be exactly `set`, as for every open.
fn admitted_newer_floor(
    conn: &rusqlite::Connection,
    applied: &[String],
    set: &[Migration],
) -> StoreResult<Option<String>> {
    let known: Vec<String> = set.iter().map(Migration::version).collect();
    if applied.len() <= set.len() || applied[..set.len()] != known[..] {
        pending_migrations(applied, set)?;
        return Ok(None);
    }
    if !ledger_has_reader_floor(conn)? {
        return Err(incompatible());
    }
    let mut floor = None;
    for version in &applied[set.len()..] {
        let recorded = applied_floor(conn, version)?;
        if !known.contains(&recorded) && !applied[set.len()..].contains(&recorded) {
            return Err(StoreError::InvalidData(format!(
                "database migration {version} records unreadable reader floor {recorded:?}"
            )));
        }
        floor = Some(recorded);
    }
    let floor = floor.expect("a newer tail has at least one row");
    if !known.contains(&floor) {
        return Err(StoreError::InvalidData(format!(
            "this store needs an lf that knows {floor} (it has {}; latest known {}); install it with `lf self install --main`",
            applied.last().expect("a newer tail is not empty"),
            known.last().map(String::as_str).unwrap_or("none"),
        )));
    }
    Ok(Some(floor))
}

/// The reader floor of a store a newer build migrated, when it still admits
/// this binary; `None` when the store is at or behind this binary's history.
pub(crate) fn newer_store_floor(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    admitted_newer_floor(conn, &applied_versions(conn)?, MIGRATIONS)
}

/// The migration a reader must know to open this store: the floor of its
/// newest applied migration. `None` for an empty store.
pub(crate) fn store_reader_floor(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    if !has_ledger(conn)? {
        return Ok(None);
    }
    let Some(newest) = applied_versions(conn)?.pop() else {
        return Ok(None);
    };
    if !ledger_has_reader_floor(conn)? {
        return Ok(Some(newest));
    }
    applied_floor(conn, &newest).map(Some)
}

/// An applied row's recorded floor; a row written before floors were recorded
/// names itself.
fn applied_floor(conn: &rusqlite::Connection, version: &str) -> StoreResult<String> {
    let floor: Option<String> = conn.query_row(
        "SELECT reader_floor FROM schema_migrations WHERE version=?1",
        [version],
        |row| row.get(0),
    )?;
    Ok(floor
        .filter(|floor| {
            MigrationId::parse_version(floor).is_some() || super::migration_catalog::is_slug(floor)
        })
        .unwrap_or_else(|| version.to_string()))
}

/// The newest pending migration that older builds cannot read past, or `None`
/// when every build that opens the store now still can after it advances.
pub(crate) fn raised_reader_floor(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    raised_floor_over(conn, MIGRATIONS)
}

fn raised_floor_over(
    conn: &rusqlite::Connection,
    set: &[Migration],
) -> StoreResult<Option<String>> {
    let applied = applied_versions(conn)?;
    Ok(pending_migrations(&applied, set)?
        .iter()
        .rev()
        .find(|migration| migration.breaks_readers())
        .map(Migration::version))
}

// -- Reader catalog -----------------------------------------------------------

/// The reader floors this build knows, in catalog order: every released batch
/// and every breaking migration. Packages ship it (`lf self install
/// reader-catalog`) so an updater can rule out a candidate before downloading it.
pub fn reader_catalog() -> Vec<&'static str> {
    MIGRATIONS
        .iter()
        .filter(|migration| migration.breaks_readers())
        .map(|migration| migration.version)
        .collect()
}

/// Whether a build whose reader catalog is `catalog` opens this store: it knows
/// the floor of the store's newest migration. It then applies what it knows
/// that the store lacks, or reads past additive migrations it does not know.
pub fn can_open(conn: &rusqlite::Connection, catalog: &[String]) -> StoreResult<bool> {
    _read_snapshot(conn, |conn| {
        Ok(store_reader_floor(conn)?.is_none_or(|floor| catalog.contains(&floor)))
    })
}

/// Pending migrations origin/main has not added. The shared store never
/// applies them; they belong in a private `LF_HOME`.
pub(crate) fn pending_unmerged(conn: &rusqlite::Connection) -> StoreResult<Vec<String>> {
    if !has_ledger(conn)? {
        return Ok(unmerged_migrations()
            .into_iter()
            .map(str::to_string)
            .collect());
    }
    let applied = applied_versions(conn)?;
    if admitted_newer_floor(conn, &applied, MIGRATIONS)?.is_some() {
        return Ok(Vec::new());
    }
    Ok(pending_migrations(&applied, MIGRATIONS)?
        .iter()
        .filter(|migration| !migration.merged)
        .map(Migration::version)
        .collect())
}

/// One row, however many tables: every open asks this, so it never lists them.
fn user_table_count(conn: &rusqlite::Connection) -> StoreResult<i64> {
    Ok(conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?)
}

fn has_table(conn: &rusqlite::Connection, name: &str) -> StoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [name],
        |row| row.get(0),
    )?)
}

fn has_ledger(conn: &rusqlite::Connection) -> StoreResult<bool> {
    has_table(conn, "schema_migrations")
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

/// Every version appears once. `build.rs` orders the catalog; a malformed set is
/// a programmer error caught by the tests, never a condition a user can create.
pub fn validate_set(set: &[Migration]) -> Result<(), String> {
    for (index, migration) in set.iter().enumerate() {
        if set[..index]
            .iter()
            .any(|earlier| earlier.version == migration.version)
        {
            return Err(format!("migration {} appears twice", migration.version));
        }
    }
    Ok(())
}

/// A database matches only if its complete product schema matches the schema the
/// migration chain builds. Names alone miss type, constraint, index, trigger,
/// and foreign-key drift.
fn validate_schema(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    let expected = expected_schema(set.iter().map(|migration| migration.sql))?;
    if *product_schema(conn)? != *expected {
        return Err(incompatible());
    }
    Ok(())
}

/// A store a newer release migrated additively still holds every object `set`
/// builds unchanged; its tables may have gained columns.
fn validate_schema_within(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    let expected = expected_schema(set.iter().map(|migration| migration.sql))?;
    let actual = product_schema(conn)?;
    if let Some(missing) = expected
        .iter()
        .find(|old| !actual.iter().any(|new| schema_object_within(old, new)))
    {
        return Err(StoreError::InvalidData(format!(
            "a newer lf changed {} {}, which lf {} reads; install the newer build with `lf self install --main`",
            missing.object_type,
            missing.name,
            crate::build_info::BUILD_VERSION
        )));
    }
    Ok(())
}

fn schema_object_within(old: &ProductSchemaObject, new: &ProductSchemaObject) -> bool {
    if old == new {
        return true;
    }
    old.object_type == "table"
        && new.object_type == "table"
        && old.name == new.name
        && old.foreign_keys.iter().all(|key| {
            new.foreign_keys.iter().any(|other| {
                ForeignKeyDefinition {
                    id: key.id,
                    ..other.clone()
                } == *key
            })
        })
        && super::migration_catalog::table_within(&old.sql, &new.sql)
}

/// Cache the immutable reference derived from this build's ordered SQL. Every
/// caller still compares it against the live schema, which `product_schema`
/// serves per file and schema cookie; no comparison result is cached.
fn expected_schema(
    sql: impl IntoIterator<Item = &'static str>,
) -> StoreResult<Arc<[ProductSchemaObject]>> {
    type References = HashMap<Vec<&'static str>, Arc<[ProductSchemaObject]>>;
    static REFERENCES: OnceLock<Mutex<References>> = OnceLock::new();
    let sql: Vec<_> = sql.into_iter().collect();
    let mut references = REFERENCES
        .get_or_init(|| {
            let canonical: Vec<ProductSchemaObject> = serde_json::from_str(include_str!(concat!(
                env!("OUT_DIR"),
                "/canonical_schema.json"
            )))
            .expect("build-generated canonical schema is valid");
            Mutex::new(HashMap::from([(
                MIGRATIONS.iter().map(|migration| migration.sql).collect(),
                canonical.into(),
            )]))
        })
        .lock()
        .expect("schema reference cache poisoned");
    if let Some(schema) = references.get(&sql) {
        return Ok(Arc::clone(schema));
    }
    let expected = rusqlite::Connection::open_in_memory()?;
    expected.execute_batch(
        "CREATE TABLE schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;
    for statement in &sql {
        expected.execute_batch(statement)?;
    }
    let schema = product_schema(&expected)?;
    references.insert(sql, Arc::clone(&schema));
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

pub fn latest_known_version() -> String {
    MIGRATIONS
        .last()
        .map(Migration::version)
        .unwrap_or_else(|| "none".to_string())
}

/// The next migration this binary knows that the store has not applied, or
/// `None` when the store is exactly at this binary's frontier.
///
/// Call only after [`validate_sqlite_schema`] has confirmed the applied history is a
/// clean recognized prefix; then `pending_migrations` cannot error and this is a
/// pure "is the store behind me?" question. An ordinary open of the shared store
/// refuses when this is `Some`: the running binary's code may query columns that
/// pending migration adds, so reusing the older schema would only fail later.
pub(crate) fn pending_shared_migration(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    let applied = applied_versions(conn)?;
    if admitted_newer_floor(conn, &applied, MIGRATIONS)?.is_some() {
        return Ok(None);
    }
    Ok(pending_migrations(&applied, MIGRATIONS)?
        .first()
        .map(Migration::version))
}

pub fn latest_applied_version_sqlite(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    if !has_ledger(conn)? {
        return Ok(None);
    }
    Ok(applied_versions(conn)?.last().cloned())
}

/// Whether this database still owes migration work.
///
/// An uninitialized database owes all of it — no user tables, or an empty
/// `schema_migrations` as its only table. Neither is "nothing to do"; only a
/// current schema is.
pub(crate) fn requires_migration_sqlite(conn: &rusqlite::Connection) -> StoreResult<bool> {
    _read_snapshot(conn, |conn| {
        if !has_ledger(conn)? {
            return if user_table_count(conn)? == 0 {
                Ok(true)
            } else {
                Err(incompatible())
            };
        }
        let applied = applied_versions(conn)?;
        if applied.is_empty() && user_table_count(conn)? == 1 {
            return Ok(true);
        }
        if applied.as_slice() == [LEGACY_BASELINE_VERSION] {
            return Ok(true);
        }
        if divergent_history(&applied, MIGRATIONS).is_some() {
            return Ok(true);
        }
        if permuted_history(&applied, MIGRATIONS).is_some() {
            return Ok(true);
        }
        Ok(!pending_migrations(&applied, MIGRATIONS)?.is_empty())
    })
}

/// A history or schema no build of this catalog produced. The data is still
/// someone's: name the builds that can open it and never suggest deletion.
fn incompatible() -> StoreError {
    StoreError::InvalidData(format!(
        "this Loopflow database's migration history or schema differs from lf {} (latest known {}); \
         keep loopflow.db and open it with the lf that last migrated it, or install the newest with `lf self install --main`",
        crate::build_info::BUILD_VERSION,
        latest_known_version(),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::sync_channel;
    use std::time::Duration;

    use super::has_table;
    use rusqlite::OptionalExtension;

    use super::apply_newer_migration;
    use super::{
        applied_versions, apply_set, apply_sqlite, apply_sqlite_transaction,
        apply_sqlite_with_backup, backup_before_migration, can_open, latest_applied_version_sqlite,
        latest_known_version, latest_version_sqlite, migration_checksum, pending_migrations,
        product_schema, prune_migration_backups, reader_catalog, validate_foreign_keys,
        validate_persisted_json, validate_schema, validate_set, validate_sqlite,
        validate_sqlite_schema, Migration, MigrationId, DIVERGENT_MIGRATIONS, MIGRATIONS,
    };

    const GATE_PROPOSAL_REPAIR_NAME: &str = "repair_legacy_task_gate_proposals";
    const LEGACY_TASK_FLOW_REPAIR_NAME: &str = "repair_legacy_task_flow";
    const TURN_USAGE_SAMPLES_NAME: &str = "add_turn_usage_samples";
    const REPOSITORY_OWNED_WAVES_NAME: &str = "repository_owned_waves";
    const REMOVE_TASK_LIFECYCLE_OUTCOME_NAME: &str = "remove_task_lifecycle_outcome";
    const RUN_IDENTITY_NAME: &str = "run_identity";

    /// Stand-ins for migrations that have not merged yet.
    const SECOND_IN_SAME_MINOR: Migration = Migration {
        version: "add_note",
        sql: "ALTER TABLE waves ADD COLUMN note TEXT;",
        reader_floor: "0.10.001_initial",
        merged: true,
    };
    const FIRST_IN_NEXT_MINOR: Migration = Migration {
        version: "add_colour",
        sql: "ALTER TABLE waves ADD COLUMN colour TEXT;",
        reader_floor: "0.10.001_initial",
        merged: true,
    };

    /// A migration's name: a slug, or a released batch's name after its id.
    fn name_of(migration: &Migration) -> &'static str {
        migration
            .version
            .split_once('_')
            .filter(|_| MigrationId::parse_version(migration.version).is_some())
            .map_or(migration.version, |(_, name)| name)
    }

    fn open() -> rusqlite::Connection {
        rusqlite::Connection::open_in_memory().unwrap()
    }

    #[test]
    fn a_candidate_opens_a_store_only_when_its_catalog_knows_the_store_floor() {
        let current: Vec<String> = reader_catalog().into_iter().map(str::to_string).collect();
        let older = current[..current.len() - 1].to_vec();
        let conn = open();
        assert!(
            can_open(&conn, &older).unwrap(),
            "an empty store opens anywhere"
        );

        apply_set(&conn, MIGRATIONS, &mut Vec::new()).unwrap();
        assert!(can_open(&conn, &current).unwrap());
        assert!(!can_open(&conn, &older).unwrap());

        for (additive, opens) in [(true, true), (false, false)] {
            let newer = open();
            apply_set(&newer, MIGRATIONS, &mut Vec::new()).unwrap();
            apply_newer_migration(&newer, "CREATE TABLE newer_table (id TEXT);", additive).unwrap();
            assert_eq!(
                can_open(&newer, &current).unwrap(),
                opens,
                "additive: {additive}"
            );
        }
    }

    #[test]
    fn a_newer_tail_must_leave_this_builds_schema_intact() {
        let conn = open();
        apply_set(&conn, MIGRATIONS, &mut Vec::new()).unwrap();
        apply_newer_migration(&conn, "DROP INDEX idx_tasks_project;", true).unwrap();

        let error = validate_sqlite_schema(&conn).unwrap_err().to_string();

        assert!(
            error.contains("a newer lf changed index idx_tasks_project"),
            "{error}"
        );
    }

    fn apply_project_content_cut(conn: &rusqlite::Connection) -> crate::store::StoreResult<()> {
        super::_migration_transaction(conn, |conn, guards| {
            apply_set(conn, MIGRATIONS, guards)?;
            super::validate_persisted_json(conn)
        })
    }

    #[test]
    fn structured_project_conversion_refuses_unreadable_criteria_without_losing_original() {
        let conn = open();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/project_content.sql"
        ))
        .unwrap();
        // Historical bytes are fixture data, not a second runtime content writer.
        let body = "Keep this evidence.\n## Metric targets\nnot valid JSON\n";
        conn.execute(
            "UPDATE projects SET project_prompt_context=?1 WHERE id='retained-project'",
            [body],
        )
        .unwrap();
        let error = apply_project_content_cut(&conn).unwrap_err().to_string();
        assert!(error.contains("retained-project"), "{error}");
        assert_eq!(
            conn.query_row(
                "SELECT project_prompt_context FROM projects WHERE id='retained-project'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            body
        );
        assert!(conn.prepare("SELECT krs FROM projects").is_err());
        assert_eq!(
            conn.query_row(
                "SELECT node FROM task_workflows WHERE task_id='retained-task'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "review"
        );
    }

    #[test]
    fn project_prose_receipt_migration_preserves_uncertain_effects() {
        let conn = open();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch("INSERT INTO waves(id,name,repo,created_at) VALUES('wave-proof','proof','/repo',1);
            INSERT INTO projects(id,wave_id,created_at) VALUES('project-proof','wave-proof',1);
            INSERT INTO project_changes(seq,id,project_id,field,value_json,base_json,conflict_json,attempted,error,acknowledged_revision,acknowledged,order_effects_json)
            VALUES(7,'receipt-proof','project-proof','summary','\"Saved\"','{\"revision\":\"r1\",\"value\":\"Before\"}',NULL,1,'Lost reply',NULL,0,'[\"effect-proof\"]');
            UPDATE sqlite_sequence SET seq=42 WHERE name='project_changes';").unwrap();
        let rows = |conn: &rusqlite::Connection| {
            conn.query_row("SELECT json_array(seq,id,project_id,field,value_json,base_json,conflict_json,attempted,error,acknowledged_revision,acknowledged,order_effects_json) FROM project_changes WHERE id='receipt-proof'", [], |r| r.get::<_, String>(0)).unwrap()
        };
        let mut expected: serde_json::Value = serde_json::from_str(&rows(&conn)).unwrap();
        expected[3] = serde_json::json!("project_summary");
        expected[4] = serde_json::json!(
            serde_json::json!({"field":"project_summary","value":"Saved"}).to_string()
        );
        expected[5]=serde_json::json!(serde_json::json!({"revision":"r1","value":{"field":"project_summary","value":"Before"}}).to_string());
        conn.execute_batch(&migration_section_sql("planning_peers"))
            .unwrap();
        let mut actual: serde_json::Value = serde_json::from_str(&rows(&conn)).unwrap();
        // Compare retained JSON values, independently of object-key ordering.
        for index in [4, 5] {
            actual[index] = serde_json::from_str(actual[index].as_str().unwrap()).unwrap();
            expected[index] = serde_json::from_str(expected[index].as_str().unwrap()).unwrap();
        }
        assert_eq!(actual, expected);
        conn.execute("INSERT INTO project_changes(id,project_id,value_json) VALUES('prose-proof','project-proof',?1)", [serde_json::to_string(&crate::work::edit::WorkValue::ProjectDescription(Some("Purpose".into()))).unwrap()]).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT seq FROM project_changes WHERE id='prose-proof'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            43
        );
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn provider_columns_preserve_released_observations_and_local_execution() {
        let conn = open();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/planning_relationships.sql"
        ))
        .unwrap();
        conn.execute_batch(r#"
            INSERT INTO pm_projects(repo,provider,id,observed_at,body,archived,membership_unresolved,task_order_json)
            VALUES('/repo','linear','foreign',17,'{"revision":"2026-10-10T12:00:00Z","description":"Foreign prose","initiative_ids":["other","disputed"],"team_ids":["foreign"]}',1,1,'["not-local"]'),
                  ('/repo','linear','partial',19,'{"initiative_ids":["initiative"]}',0,0,NULL);
            INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body,needs_refresh)
            VALUES('/repo','linear','unmapped','OTHER-1','foreign',23,'{"project_id":"contradictory","team_id":"foreign","completed_at":null,"due_date":"2026-10-17"}',1),
                  ('/repo','linear','partial','OTHER-2',NULL,11,'{"name":"Unmapped"}',0);
            INSERT INTO pm_issue_changes(issue_id,revision_ns,removed) VALUES('unmapped',123,1);
            INSERT INTO pm_wave_sync(wave_id,provider,initiative,synced_at) VALUES('wave-proof','linear','initiative',29);
            INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES('wave-proof','foreign',4);
            INSERT INTO processes(id,trace_id,command,cwd,started_at)
            VALUES('11111111-1111-4111-8111-111111111111','trace','lf run code','/retained/task',2);
            INSERT INTO task_workflows(task_id,graph,node,edge,lf_process_id,updated_at)
            VALUES('task-proof','{"name":"saved","nodes":[],"edges":[{"from":"review","to":"end"}]}','review',0,'11111111-1111-4111-8111-111111111111',7);
            UPDATE tasks SET worktree='/retained/task',external_issue_id='legacy-identity' WHERE id='task-proof';
            INSERT INTO task_state_deliveries(id,task_id,target,attempted,error)
            VALUES('uncertain','task-proof','completed',1,'Lost response');
        "#).unwrap();
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/planning_local_sessions.sql"
        ))
        .unwrap();
        let tables = [
            "tasks",
            "processes",
            "task_workflows",
            "agent_sessions",
            "session_events",
            "session_activity",
            "task_events",
            "task_comment_deliveries",
            "task_state_deliveries",
            "pm_issue_changes",
            "pm_wave_sync",
            "pm_wave_projects",
        ];
        let rows = |table: &str| {
            let mut query = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let columns = query.column_count();
            query
                .query_map([], |row| {
                    (0..columns)
                        .map(|i| row.get::<_, rusqlite::types::Value>(i))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let before: Vec<_> = tables.iter().map(|table| rows(table)).collect();
        let sql = migration_section_sql("planning_peers");
        super::_migration_transaction(&conn, |conn, _guards| {
            conn.execute_batch(&sql)?;
            super::convert_released_data(conn, &sql)
        })
        .unwrap();
        for (table, expected) in tables.iter().zip(before) {
            assert_eq!(rows(table), expected, "{table}");
        }
        let project: (String,String,i64,i64,i64,String) = conn.query_row(
            "SELECT description,description_revision,description_observed_at,observed_at,archived+membership_unresolved,initiative_ids FROM pm_projects WHERE id='foreign'", [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).unwrap();
        assert_eq!(
            project,
            (
                "Foreign prose".into(),
                "2026-10-10T12:00:00Z".into(),
                17,
                17,
                2,
                "[\"other\",\"disputed\"]".into()
            )
        );
        let issues: Vec<_> = conn.prepare(
            "SELECT id,project_id,claimed_project_id,completed_at_observed,due_date_observed,observed_at FROM pm_items ORDER BY id").unwrap()
            .query_map([], |row| Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,bool>(3)?,row.get::<_,bool>(4)?,row.get::<_,i64>(5)?))).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(
            issues,
            vec![
                ("partial".into(), None, None, false, false, 11),
                (
                    "unmapped".into(),
                    Some("foreign".into()),
                    Some("contradictory".into()),
                    true,
                    true,
                    23
                )
            ]
        );
        assert!(conn.prepare("SELECT body FROM pm_projects").is_err());
        assert!(conn.prepare("SELECT body FROM pm_items").is_err());
        assert_eq!(
            conn.query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn invalid_provider_conversion_rolls_back_schema_and_original_evidence() {
        for body in [
            "retained invalid input",
            r#"{"name":[]}"#,
            r#"{"name":12}"#,
            r#"{"rank":true}"#,
            r#"{"completed":1}"#,
        ] {
            let conn = open();
            apply_before_migration(&conn, "planning_peers");
            conn.execute("INSERT INTO pm_items(repo,provider,id,identifier,observed_at,body) VALUES('/repo','linear','unreadable','OTHER-1',17,?1)",[body]).unwrap();
            let error = super::_migration_transaction(&conn, |conn, _guards| {
                conn.execute_batch(&migration_section_sql("planning_peers"))?;
                Ok(())
            })
            .unwrap_err();
            assert!(
                error.to_string().contains("malformed JSON")
                    || error.to_string().contains("valid=1"),
                "{error}"
            );
            assert_eq!(
                conn.query_row("SELECT body,observed_at FROM pm_items", [], |r| Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?
                )))
                .unwrap(),
                (body.into(), 17)
            );
            assert!(conn.prepare("SELECT description FROM projects").is_err());
            assert!(conn.prepare("SELECT revision FROM pm_items").is_err());
        }
    }

    #[test]
    fn typed_journal_seeding_preserves_released_relationships_authors_and_receipts() {
        let conn = open();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/planning_relationships.sql"
        ))
        .unwrap();
        let receipt = |conn: &rusqlite::Connection| {
            conn.query_row(
                "SELECT json_array(comment_id,comment_json,acknowledged,error,conflicting_comment_json)
                 FROM task_comment_deliveries WHERE comment_id='comment-proof'",
                [], |row| row.get::<_, String>(0),
            ).unwrap()
        };
        let before = receipt(&conn);
        let sql = migration_section_sql("planning_peers");
        conn.execute_batch(&sql).unwrap();
        super::convert_released_data(&conn, &sql).unwrap();
        for (id, field, expected) in [
            (
                "project-proof",
                "project_teams",
                serde_json::json!(["team-a", "team-b"]),
            ),
            (
                "project-proof",
                "project_initiatives",
                serde_json::json!(["initiative-a"]),
            ),
            (
                "comment-proof",
                "comment_content",
                serde_json::json!({
                    "body":"Keep attribution", "author":{"kind":"person","name":"Maya"}, "created_at":null
                }),
            ),
        ] {
            let value: String = conn
                .query_row(
                    "SELECT mutation -> '$.edit.change.value' FROM planning_peer_changes WHERE object_id=?1 AND field=?2",
                    [id, field],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&value).unwrap(),
                expected
            );
        }
        assert_eq!(receipt(&conn), before);
        assert_eq!(
            conn.query_row(
                "SELECT provider_revision FROM task_comments WHERE id='comment-proof'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "revision-before"
        );
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn structured_project_content_preserves_released_work_and_pending_delivery() {
        let conn = open();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/project_content.sql"
        ))
        .unwrap();
        apply_project_content_cut(&conn).unwrap();
        let (description, krs, targets, revision): (String, String, String, i64) = conn.query_row(
            "SELECT description,krs,metric_targets,pm_snapshot_synced_at FROM projects WHERE id='retained-project'", [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
        assert_eq!(
            description,
            "Authored purpose.\n\n## Example\n````markdown\n```yaml\nworkflow: example\nflow: legacy-example\n```\n## KRs\n- [x] Example only\n````\nTarget rationale.\nTarget note.\nThis preamble is authored prose.\n## Notes\nKeep these bytes.\n"
        );
        assert_eq!(revision, 7);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&krs).unwrap(),
            serde_json::json!([
                {"text":"Retain this continued proof","holds":true}, {"text":"Check offline saves","holds":false}
            ])
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&targets).unwrap(),
            serde_json::json!([
                {"metric_id":"recovery","target":{"kind":"at_least","value":1.0}}
            ])
        );
        // `ship_it` drops Workflows short of `end`; the next run takes up a graph.
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM task_graphs WHERE task_id='retained-task'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT worktree FROM tasks WHERE id='retained-task'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "/retained/task"
        );
        assert_eq!(
            conn.query_row(
                "SELECT external_project_id FROM projects WHERE id='retained-project'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "linear-project"
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM task_state_deliveries WHERE task_id='retained-task'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT kind_json FROM project_events WHERE project_id='retained-project'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            r#"{"kind":"iteration_completed","iteration":3,"summary":"Keep judgment"}"#
        );
        assert!(conn
            .prepare("SELECT project_prompt_context FROM projects")
            .is_err());
        let captured: String = conn.query_row("SELECT mutation -> '$.edit.change.value' FROM planning_peer_changes WHERE object_id='retained-project' AND field='krs'", [], |r| r.get(0)).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&captured).unwrap(),
            serde_json::from_str::<serde_json::Value>(&krs).unwrap()
        );
    }

    #[test]
    fn task_checkout_machine_migration_preserves_locations_and_history() {
        let conn = open();
        apply_before_migration(&conn, "task_checkout_machine");
        conn.execute_batch(r#"
            INSERT INTO waves(id,name,repo,created_at) VALUES('w','proof','/repo',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('p','w','external-p',1);
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,worktree)
                VALUES('t','p','external-t','PROOF-1',1,'/repo/task'),
                      ('unknown','p','external-unknown','PROOF-2',1,'/repo/unknown'),
                      ('no-checkout','p','external-empty','PROOF-3',1,'');
            INSERT INTO machines(id,route,created_at,observed_at) VALUES('machine-b','worker',1,1);
            INSERT INTO work_placements(task_id,machine_id,placed_at)
                SELECT 't',id,1 FROM machines WHERE route='local';
            INSERT INTO work_placements(task_id,machine_id,placed_at)
                SELECT 'no-checkout',id,1 FROM machines WHERE route='local';
            UPDATE tasks SET branch='proof',base_commit='abc' WHERE id='t';
            INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
                VALUES('pr','t',1,'proof','proof','abc',1,1);
            INSERT INTO processes(id,trace_id,command,cwd,started_at)
                VALUES('process','trace','lf run code','/repo/task',1);
            INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,task_id,wave_id,input_published)
                VALUES('session','Keep draft','human',1,'/repo/task','t','w',0);
            INSERT INTO processes(id,kind,trace_id,parent_lf_process_id,agent_session_id,attached_lf_process_id,started_at)
                VALUES('agent','agent','trace','process','session','process',1);
            UPDATE agent_sessions SET agent_process_id='agent' WHERE id='session';
        "#).unwrap();
        let history = || -> Vec<Vec<Vec<rusqlite::types::Value>>> {
            ["task_prs", "processes", "agent_sessions"]
                .iter()
                .map(|table| {
                    let mut query = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
                    let count = query.column_count();
                    query
                        .query_map([], |row| (0..count).map(|i| row.get(i)).collect())
                        .unwrap()
                        .collect::<Result<_, _>>()
                        .unwrap()
                })
                .collect()
        };
        let before = history();
        let local: String = conn
            .query_row("SELECT id FROM machines WHERE route='local'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute_batch(&migration_section_sql("task_checkout_machine"))
            .unwrap();
        super::convert_released_data(&conn, &migration_section_sql("task_checkout_machine"))
            .unwrap();
        conn.execute(
            "UPDATE work_placements SET machine_id='machine-b' WHERE task_id='t'",
            [],
        )
        .unwrap();
        let locations: Vec<(String, String, Option<String>)> = conn
            .prepare("SELECT id,worktree,checkout_machine_id FROM tasks ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            locations,
            vec![
                ("no-checkout".into(), "".into(), None),
                ("t".into(), "/repo/task".into(), Some(local.clone())),
                ("unknown".into(), "/repo/unknown".into(), None),
            ]
        );
        let provenance: String = conn
            .query_row(
                "SELECT provenance FROM work_placements WHERE task_id='t'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provenance, "legacy");
        // An unconfigured destination can be retained without a Machine row;
        // the recorded execution location and local enablement stay unchanged.
        conn.execute(
            "UPDATE work_placements SET machine_id='unconfigured' WHERE task_id='t'",
            [],
        )
        .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM machines WHERE id='unconfigured'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        let repository: String = conn
            .query_row(
                "SELECT id FROM repository_plans WHERE repo='/repo'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        crate::durable::RepositoryId::parse(&repository).unwrap();
        assert_eq!(history(), before);
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn task_pr_continuation_preserves_released_delivery_and_one_current_pr() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "task_pr_continuation");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/task_pr_continuation.sql"
        ))
        .unwrap();
        conn.execute_batch(&migration_section_sql("task_pr_continuation"))
            .unwrap();
        conn.execute("UPDATE tasks SET base_commit='next-base' WHERE id='t'", [])
            .unwrap();
        let base: String = conn
            .query_row(
                "SELECT base_commit FROM task_prs WHERE id='first'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(base, "base");
        conn.execute("UPDATE task_prs SET historical=1 WHERE id='first'", [])
            .unwrap();
        conn.execute_batch("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,created_at,updated_at)
          VALUES('second','t',2,'work','work','next-base',2,2,2)").unwrap();
        assert!(conn
            .execute("UPDATE task_prs SET historical=1 WHERE id='second'", [])
            .is_err());
        assert!(conn
            .execute("UPDATE task_prs SET github_number=99 WHERE id='first'", [])
            .is_err());
        assert!(conn
            .execute("DELETE FROM task_prs WHERE id='first'", [])
            .is_err());
        assert!(conn.execute_batch("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
          VALUES('third','t',3,'work','work','next-base',3,3)").is_err());
        let current: String = conn
            .query_row(
                "SELECT id FROM task_prs WHERE task_id='t' AND historical=0",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(current, "second");
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn optional_task_pr_preserves_placement_and_freezes_prior_delivery() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "local_planning");
        conn.execute_batch(r#"
            INSERT INTO waves(id,name,repo,created_at) VALUES('w','product','/repo',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('p','w','linear-p',1);
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,workspace_slug,created_at,updated_at)
            VALUES('research','p','research-issue','R-1','/research','research',1,1),
                  ('single','p','single-issue','R-2','/single','single',1,1),
                  ('chain','p','chain-issue','R-3','/chain','chain',1,1),
                  ('continuing','p','continuing-issue','R-4','/continuing','continuing',1,1);
            INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
            VALUES('placeholder','research',1,'research','research','research-base',1,1);
            INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,
              github_number,github_url,merge_commit,created_at,updated_at)
            VALUES('single-pr','single',1,'single','single','single-base',1,1,'https://github.com/a/b/pull/1',NULL,1,1),
                  ('old-pr','chain',1,'old','old','old-base',1,2,'https://github.com/a/b/pull/2','merged',1,1),
                  ('current-pr','chain',2,'current','current','current-base',1,3,'https://github.com/a/b/pull/3',NULL,1,1);
            INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,
              github_number,github_url,merge_commit,after_merge,next_slug,merge_mode,merge_requested_at,merge_head_sha,github_head_sha,created_at,updated_at)
            VALUES('continued-pr','continuing',1,'prior','prior','prior-base',1,4,'https://github.com/a/b/pull/4','merged','continue_task','continuing','auto',1,'head','head',1,1);
            INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
            VALUES('successor-placeholder','continuing',2,'continuing','continuing','prior-base',2,2);
            INSERT INTO task_events(task_id,kind_json,created_at) VALUES('chain',
              '{"kind":"follow_up","remaining":{"outcome":"Check release","evidence":"Installed command works","check_at":42},"reason":"Accepted release check"}',1);
        "#).unwrap();
        conn.execute_batch(r#"INSERT INTO task_workflows(task_id,graph,node,updated_at)
            VALUES('research','{"name":"research","nodes":[],"edges":[]}','end',4);
            INSERT INTO task_events(task_id,kind_json,created_at) VALUES('research','{"kind":"completed","summary":"accepted findings"}',4);"#).unwrap();
        conn.execute_batch(r#"
            UPDATE tasks SET pm_writeback_json='{"state":"pending","operation":"complete_task","error":"offline"}' WHERE id='single';
            INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
                VALUES('conversation','Retained conversation','human',2,0,'/single','single','w');
            UPDATE task_prs SET parent_pr_id='old-pr' WHERE id='current-pr';
        "#).unwrap();
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/optional_task_pr_held_process.sql"
        ))
        .unwrap();
        apply_sqlite(&conn).unwrap();
        apply_pending_migrations(&conn);
        assert_eq!(
            conn.query_row(
                "SELECT planning_completed FROM tasks WHERE id='research'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        conn.execute(
            "UPDATE task_graphs SET step='start' WHERE task_id='research'",
            [],
        )
        .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT planning_completed FROM tasks WHERE id='research'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        let pending: (String, i64, i64) = conn
            .query_row(
                "SELECT target,attempted,settled FROM task_state_deliveries WHERE task_id='single'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(pending, ("completed".into(), 1, 0));
        // Released `ship_it` dropped positions short of `end`; their next run
        // takes up a Graph without retaining that earlier execution.
        let execution: i64 = conn
            .query_row(
                "SELECT count(*) FROM task_graphs WHERE task_id='single'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(execution, 0);
        assert_eq!(
            conn.query_row(
                "SELECT task_id FROM agent_sessions WHERE id='conversation'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "single"
        );
        assert_eq!(
            conn.query_row(
                "SELECT parent_pr_id FROM tasks WHERE id='chain'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "old-pr"
        );
        let placement: (String, String) = conn
            .query_row(
                "SELECT branch,base_commit FROM tasks WHERE id='research'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(placement, ("research".into(), "research-base".into()));
        let current: Vec<String> = conn
            .prepare("SELECT id FROM task_prs WHERE historical=0 ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(current, vec!["current-pr", "single-pr"]);
        assert!(conn
            .execute(
                "UPDATE task_prs SET base_commit='changed' WHERE id='old-pr'",
                []
            )
            .is_err());
        assert!(conn
            .execute("DELETE FROM task_prs WHERE id='old-pr'", [])
            .is_err());
        // The retained placeholder cannot block first publication on this branch.
        conn.execute_batch("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,created_at,updated_at)
          VALUES('research-pr','research',2,'research','research','research-base',2,2,2)").unwrap();
        assert!(conn.execute_batch("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,created_at,updated_at)
          VALUES('duplicate','research',3,'research','research','research-base',3,3,3)").is_err());
        let obligation: String = conn.query_row("SELECT kind_json FROM task_events WHERE task_id='chain' AND json_extract(kind_json,'$.kind')='follow_up'", [], |row| row.get(0)).unwrap();
        assert!(obligation.contains("Check release"));
        let conversion: String = conn.query_row("SELECT kind_json FROM task_events WHERE task_id='continuing' AND json_extract(kind_json,'$.kind')='follow_through_conversion'", [], |row| row.get(0)).unwrap();
        assert!(conversion.contains("continuing"));
        let current_continuation: i64 = conn
            .query_row(
                "SELECT count(*) FROM task_prs WHERE task_id='continuing' AND historical=0",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(current_continuation, 0);
        let violations: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
    }

    #[test]
    fn process_names_preserves_released_history_and_constraints() {
        let conn = open();
        apply_before_migration(&conn, "process_names");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/process_names_released.sql"
        ))
        .unwrap();
        let rows = |table: &str| -> Vec<Vec<rusqlite::types::Value>> {
            let mut query = conn.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let columns = query.column_count();
            query
                .query_map([], |row| (0..columns).map(|i| row.get(i)).collect())
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let tables = [
            ("execs", "processes"),
            ("flow_execs", "flow_processes"),
            ("flow_exec_steps", "flow_process_steps"),
            ("agent_sessions", "agent_sessions"),
            ("session_events", "session_events"),
            ("waves", "waves"),
            ("tasks", "tasks"),
            ("task_workflows", "task_workflows"),
            ("task_workflow_moves", "task_workflow_moves"),
        ];
        let before: Vec<_> = tables.iter().map(|(old, _)| rows(old)).collect();
        let revision: i64 = conn
            .query_row(
                "SELECT revision FROM store_revisions WHERE domain='execs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute_batch(&migration_section_sql("process_names"))
            .unwrap();
        for ((_, current), mut expected) in tables.iter().zip(before) {
            if *current == "processes" {
                for row in &mut expected {
                    row.push(rusqlite::types::Value::Null);
                }
            }
            assert_eq!(rows(current), expected, "{current}");
        }
        assert!(rows("pragma_foreign_key_check").is_empty());
        assert!(conn.prepare("SELECT * FROM execs").is_err());
        assert!(conn
            .execute("UPDATE flow_processes SET flow='changed'", [])
            .is_err());
        assert!(conn
            .execute("UPDATE flow_process_steps SET node=8", [])
            .is_err());
        assert!(conn
            .execute_batch(include_str!(
                "../../tests/fixtures/migrations/process_names_unstarted_step.sql"
            ))
            .is_err());
        conn.execute("UPDATE tasks SET started_at=started_at WHERE id='t'", [])
            .unwrap();
        assert!(conn
            .execute("UPDATE tasks SET started_at=started_at+1 WHERE id='t'", [])
            .is_err());
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/process_names_finish_parent.sql"
        ))
        .unwrap();
        let after: i64 = conn
            .query_row(
                "SELECT revision FROM store_revisions WHERE domain='processes'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, revision + 1);
    }

    #[test]
    fn store_revisions_upgrade_starts_counting_without_touching_released_rows() {
        let conn = open();
        apply_before_migration(&conn, "store_revisions");
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('wave-released','proof','/repo',1);",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("store_revisions"))
            .unwrap();
        let revision = |domain: &str| -> i64 {
            conn.query_row(
                "SELECT revision FROM store_revisions WHERE domain=?1",
                [domain],
                |row| row.get(0),
            )
            .unwrap()
        };
        // Rows written before the upgrade are not counted as changes.
        assert_eq!(revision("planning"), 0);
        conn.execute(
            "UPDATE waves SET name='renamed' WHERE id='wave-released'",
            [],
        )
        .unwrap();
        assert_eq!(revision("planning"), 1);
        assert_eq!(revision("sessions"), 0);
        let unfinished: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN
                 ('execs_unfinished','session_events_exec','flow_events_exec')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unfinished, 3);
    }

    #[test]
    fn remove_ask_preserves_conversations_and_history() {
        let conn = open();
        apply_before_migration(&conn, "remove_ask");
        conn.execute_batch("INSERT INTO waves(id,name,repo,created_at) VALUES('wave-preserved','proof','/repo',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('project-preserved','wave-preserved','external-project',1);
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES('task-preserved','project-preserved','external-issue','PROOF-1','/repo',1);").unwrap();
        for (id, completed, native, published) in [
            ("completed", Some(30), Some("native-completed"), true),
            ("open", None, Some("native-open"), true),
            ("reserved", None, None, false),
        ] {
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,ready_summary,completed_at,created_at,request,kind,interactive,cwd,skill,provider,provider_thread,input_published)
                VALUES(?1,?1,'human',?2,?3,10,'Retained request','ask',1,'/repo','unblock','codex',?4,?5)",
                rusqlite::params![id, completed.map(|_| "Retained answer"), completed, native, published]).unwrap();
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,'captured',?2,10,'{}')",
                rusqlite::params![id, format!("run_{id}")]).unwrap();
            conn.execute(
                "UPDATE agent_sessions SET current_capture=last_insert_rowid() WHERE id=?1",
                [id],
            )
            .unwrap();
        }
        conn.execute("UPDATE agent_sessions SET task_id='task-preserved',wave_id='wave-preserved',work_source='inherited'", []).unwrap();
        let before: Vec<(i64, String, String)> = conn
            .prepare("SELECT seq,session_id,payload FROM session_events ORDER BY seq")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        conn.execute_batch(&migration_section_sql("remove_ask"))
            .unwrap();
        for (id, completed, native, published) in [
            ("completed", Some(30), Some("native-completed"), true),
            ("open", None, Some("native-open"), true),
            ("reserved", None, None, false),
        ] {
            let row = conn.query_row("SELECT kind,skill,primary_scope,request,completed_at,provider_thread,input_published,ready_summary FROM agent_sessions WHERE id=?1", [id], |row| Ok((
                row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,Option<String>>(2)?,row.get::<_,String>(3)?,
                row.get::<_,Option<i64>>(4)?,row.get::<_,Option<String>>(5)?,row.get::<_,bool>(6)?,row.get::<_,Option<String>>(7)?
            ))).unwrap();
            assert_eq!(
                row,
                (
                    "conversation".into(),
                    None,
                    None,
                    "Retained request".into(),
                    completed,
                    native.map(str::to_string),
                    published,
                    completed.map(|_| "Retained answer".into())
                )
            );
        }
        let after: Vec<(i64, String, String)> = conn
            .prepare("SELECT seq,session_id,payload FROM session_events ORDER BY seq")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(after, before);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM agent_sessions WHERE task_id='task-preserved' AND wave_id='wave-preserved' AND work_source='inherited'", [], |row| row.get::<_, i64>(0)).unwrap(), 3);
    }

    #[test]
    fn expected_schema_reuse_keeps_prefixes_and_changed_sql_distinct() {
        let conn = open();
        let first = [baseline(), SECOND_IN_SAME_MINOR];
        apply_set(&conn, &first, &mut Vec::new()).unwrap();
        validate_schema(&conn, &first).unwrap();
        assert!(validate_schema(&conn, &first[..1]).is_err());

        let changed = [
            baseline(),
            Migration {
                sql: "ALTER TABLE waves ADD COLUMN different_note TEXT;",
                ..SECOND_IN_SAME_MINOR
            },
        ];
        assert!(validate_schema(&conn, &changed).is_err());
        let other = open();
        apply_set(&other, &changed, &mut Vec::new()).unwrap();
        validate_schema(&other, &changed).unwrap();
        assert!(validate_schema(&other, &first).is_err());
        validate_schema(&conn, &first).unwrap();
    }

    fn baseline() -> Migration {
        MIGRATIONS[0]
    }

    fn apply_divergent_history(conn: &rusqlite::Connection, count: usize) {
        let canonical_start = MIGRATIONS
            .iter()
            .position(|migration| name_of(migration) == "interactive_handoffs")
            .unwrap();
        apply_set(conn, &MIGRATIONS[..canonical_start], &mut Vec::new()).unwrap();
        for migration in &DIVERGENT_MIGRATIONS[..count] {
            conn.execute_batch(migration.sql).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, unixepoch())",
                [migration.version()],
            )
            .unwrap();
        }
    }

    fn apply_permuted_history(conn: &rusqlite::Connection) {
        let context_start = MIGRATIONS
            .iter()
            .position(|migration| name_of(migration) == "context_pressure")
            .unwrap();
        apply_set(conn, &MIGRATIONS[..context_start], &mut Vec::new()).unwrap();
        conn.execute("UPDATE schema_migrations SET applied_at = rowid", [])
            .unwrap();
        let applied_at = conn
            .query_row("SELECT MAX(applied_at) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let permutation = [
            ("0.11.009_profiles", "profiles"),
            (
                "0.11.010_provider_account_lifecycle",
                "provider_account_lifecycle",
            ),
            ("0.11.011_context_pressure", "context_pressure"),
            (
                "0.11.012_context_input_normalization",
                "context_input_normalization",
            ),
        ];
        for (offset, (version, name)) in permutation.into_iter().enumerate() {
            let migration = MIGRATIONS
                .iter()
                .find(|migration| name_of(migration) == name)
                .unwrap();
            conn.execute_batch(migration.sql).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                (version, applied_at + offset as i64 + 1),
            )
            .unwrap();
        }
    }

    fn columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        let quoted = format!("\"{}\"", table.replace('"', "\"\""));
        let mut statement = conn
            .prepare(&format!("PRAGMA table_info({quoted})"))
            .unwrap();
        statement
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    /// Insert one trace row carrying `capture_status`, reporting whether the
    /// table's CHECK constraint accepted it. Historical prefixes still use
    /// the pre-reduction table name.
    fn capture_status_accepts(conn: &rusqlite::Connection, capture_status: &str) -> bool {
        let id = format!("probe-{capture_status}");
        let table = if conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'agent_invocations'",
                [],
                |_| Ok(()),
            )
            .is_ok()
        {
            "agent_invocations"
        } else {
            "agent_launches"
        };
        let inserted = conn
            .execute(
                &format!(
                    "INSERT INTO {table} (
                     id, run_id, process_id, started_at, repo, worktree, provider,
                     surface, capture_status, outcome, artifact_dir,
                     conversation_path, conversation_event_count, conversation_bytes
                 ) VALUES (?1, 'run-probe', 'proc-probe', 100, '/repo', '/repo',
                     'codex', 'headless', ?2, 'completed', 'probe/dir',
                     'probe/conversation.jsonl', 1, 10)"
                ),
                rusqlite::params![id, capture_status],
            )
            .is_ok();
        if inserted {
            conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [&id])
                .unwrap();
        }
        inserted
    }

    fn find_backup(directory: &std::path::Path, prefix: &str) -> std::path::PathBuf {
        std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("no backup starts with {prefix:?}"))
    }

    /// Released batches stay in numeric id order and every later migration
    /// follows them.
    #[test]
    fn released_history_precedes_later_migrations_in_id_order() {
        let released: Vec<MigrationId> = MIGRATIONS
            .iter()
            .map_while(|migration| MigrationId::parse_version(migration.version))
            .collect();
        assert!(released.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(MIGRATIONS[released.len()..]
            .iter()
            .all(|migration| super::super::migration_catalog::is_slug(migration.version)));
        validate_set(MIGRATIONS).unwrap();
    }

    #[test]
    fn a_fresh_database_applies_the_whole_chain_once() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_sqlite(&conn).unwrap();

        assert!(conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, bool>(0))
            .unwrap());
        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            latest_known_version()
        );
        assert!(product_schema(&conn)
            .unwrap()
            .iter()
            .any(|object| object.object_type == "table" && object.name == "tasks"));
        let schema = product_schema(&conn).unwrap();
        for deleted in ["bus_messages", "bus_cursors"] {
            assert!(
                !schema
                    .iter()
                    .any(|object| object.object_type == "table" && object.name == deleted),
                "{deleted} must be absent from the current schema"
            );
        }
        assert!(!schema.iter().any(|object| {
            object.object_type == "table"
                && matches!(object.name.as_str(), "task_sessions" | "project_sessions")
        }));
        assert!(!columns(&conn, "tasks")
            .iter()
            .any(|column| { matches!(column.as_str(), "status" | "status_reason" | "status_at") }));
        assert!(!columns(&conn, "projects")
            .iter()
            .any(|column| { matches!(column.as_str(), "status_reason" | "status_at") }));
        for table in ["projects", "tasks"] {
            let names = columns(&conn, table);
            assert!(!names.iter().any(|name| name == "current_directive_version"));
            assert!(!names
                .iter()
                .any(|name| name == "incorporated_directive_version"));
        }
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type='table' AND name IN (
                    'child_directives', 'launches', 'turns',
                    'interaction_reviews', 'interactive_handoffs',
                    'bus_messages', 'bus_cursors'
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0,
            "the durable spine has no compatibility, Agent Bus, or shadow lifecycle tables"
        );
        apply_sqlite(&conn).unwrap();
        assert_eq!(
            applied_versions(&conn).unwrap(),
            MIGRATIONS
                .iter()
                .map(Migration::version)
                .collect::<Vec<_>>()
        );
    }

    /// Everything up to but excluding `name`. A test whose subject is one
    /// migration names it, so appending the next migration cannot silently
    /// re-point the test at different SQL.
    fn prefix_before(name: &str) -> &'static [Migration] {
        let index = MIGRATIONS
            .iter()
            .position(|migration| name_of(migration) == name)
            .expect("named migration is registered");
        &MIGRATIONS[..index]
    }

    fn apply_through(conn: &rusqlite::Connection, name: &str) {
        let index = MIGRATIONS
            .iter()
            .position(|migration| name_of(migration) == name)
            .expect("named migration is registered");
        apply_set(conn, &MIGRATIONS[..=index], &mut Vec::new()).unwrap();
    }

    /// A migration's section of the catalog: a slug migration's whole file, or
    /// the `-- draft: <name>` section of the released batch that carries it.
    struct Section {
        index: usize,
        sql: &'static str,
        start: usize,
        body: std::ops::Range<usize>,
    }

    fn section(name: &str) -> Section {
        let marker = format!("-- draft: {name}");
        MIGRATIONS
            .iter()
            .enumerate()
            .find_map(|(index, migration)| {
                if migration.version == name {
                    return Some(Section {
                        index,
                        sql: migration.sql,
                        start: 0,
                        body: 0..migration.sql.len(),
                    });
                }
                let start = migration.sql.find(&marker)?;
                let sql = migration.sql;
                let body_start = sql[start..]
                    .find('\n')
                    .map(|offset| start + offset + 1)
                    .unwrap_or(sql.len());
                let body_end = sql[body_start..]
                    .find("\n-- draft: ")
                    .map(|offset| body_start + offset)
                    .unwrap_or(sql.len());
                Some(Section {
                    index,
                    sql,
                    start,
                    body: body_start..body_end,
                })
            })
            .unwrap_or_else(|| panic!("migration {name} is in the catalog"))
    }

    fn apply_before_draft(conn: &rusqlite::Connection, name: &str) {
        let section = section(name);
        apply_set(conn, &MIGRATIONS[..section.index], &mut Vec::new()).unwrap();
        conn.execute_batch(&section.sql[..section.start]).unwrap();
    }

    fn apply_draft(conn: &rusqlite::Connection, name: &str) {
        let section = section(name);
        conn.execute_batch(&section.sql[section.body]).unwrap();
    }

    #[test]
    fn conversation_admission_preserves_started_history_and_starts_only_on_input() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "session_admission");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/conversation_admission_released.sql"
        ))
        .unwrap();
        let started = || {
            conn.query_row(
                "SELECT started_at FROM tasks WHERE id='started'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
        };
        let first = started();
        conn.execute_batch(&migration_section_sql("session_admission"))
            .unwrap();
        assert_eq!(started(), first);
        let retained: (String, String) = conn.query_row("SELECT t.primary_session_id,c.receipt_key FROM tasks t JOIN agent_sessions s ON s.id=t.primary_session_id JOIN session_events c ON c.seq=s.current_capture WHERE t.id='started'", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(
            retained,
            ("history".into(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into())
        );
        conn.execute("INSERT INTO agent_sessions(id,title,title_source,interactive,created_at,repo,task_id,wave_id,work_source,cwd,input_published) VALUES('admitted','Future conversation','human',1,3,'/repo',NULL,'wave','declared','/repo',0)", []).unwrap();
        let backlog = || {
            conn.query_row(
                "SELECT started_at FROM tasks WHERE id='backlog'",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .unwrap()
        };
        assert_eq!(backlog(), None);
        conn.execute("UPDATE agent_sessions SET task_id='backlog',work_source='bound',bound_at=6 WHERE id='admitted'", []).unwrap();
        assert_eq!(
            backlog(),
            None,
            "binding an uncaptured conversation is metadata admission"
        );
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('admitted','captured','bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',7,'{}')", []).unwrap();
        conn.execute(
            "UPDATE agent_sessions SET current_capture=?1 WHERE id='admitted'",
            [conn.last_insert_rowid()],
        )
        .unwrap();
        assert_eq!(backlog(), Some(7));
        assert_eq!(started(), first);
    }

    #[test]
    fn repository_relocation_upgrade_retains_released_work() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "repository_relocation");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/repository_relocation_released.sql"
        ))
        .unwrap();
        conn.execute_batch(&migration_section_sql("repository_relocation"))
            .unwrap();
        conn.execute(
            "INSERT INTO relocated_task_processes(task_id,process_id) VALUES('task','process')",
            [],
        )
        .unwrap();
        let addresses: (String, String) = conn.query_row(
            "SELECT t.worktree,p.cwd FROM relocated_task_processes r JOIN tasks t ON t.id=r.task_id JOIN processes p ON p.id=r.process_id", [],
            |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(addresses, ("/before/task".into(), "/before/task".into()));
    }

    #[test]
    fn designated_panes_resets_released_free_trees() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "designated_panes");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/pane_layouts_released.sql"
        ))
        .unwrap();
        let sql = migration_section_sql("designated_panes");
        conn.execute_batch(&sql).unwrap();
        super::convert_released_data(&conn, &sql).unwrap();
        super::validate_persisted_json(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM work_pane_layouts", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn session_preview_upgrades_released_activity_without_inventing_output() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "session_preview");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/session_preview_released.sql"
        ))
        .unwrap();
        conn.execute_batch(&migration_section_sql("session_preview"))
            .unwrap();
        let activity: (i64, i64, Option<String>) = conn.query_row(
            "SELECT observed_at,pending_input,preview FROM session_activity WHERE session_id='retained'", [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(activity, (100, 1, None));
    }

    #[test]
    fn session_facts_only_folds_stream_usage_into_native_turns_and_keeps_facts() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "session_facts_only");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/session_facts_only_released.sql"
        ))
        .unwrap();
        conn.execute_batch(&migration_section_sql("session_facts_only"))
            .unwrap();
        let usage = |thread: &str, turn: &str| -> (serde_json::Value, Option<i64>) {
            conn.query_row(
                "SELECT payload,captured_event FROM session_events
                 WHERE kind='usage' AND provider_thread=?1 AND provider_turn=?2",
                [thread, turn],
                |row| {
                    Ok((
                        serde_json::from_str(&row.get::<_, String>(0)?).unwrap(),
                        row.get(1)?,
                    ))
                },
            )
            .unwrap()
        };
        // A matched Claude receipt takes the stream's final usage, with cost.
        let (matched, _) = usage("claude-thread", "matched");
        assert_eq!(matched["final"], true);
        assert_eq!(matched["usage"]["cost_usd"], 0.25);
        assert_eq!(matched["usage"]["context_window_tokens"], 200000);
        // An unmatched one is converted from Claude's raw result usage.
        let (raw, _) = usage("claude-thread", "raw");
        assert_eq!(
            raw,
            serde_json::json!({"provider":"claude","final":true,"usage":{"input_tokens":3,
                "output_tokens":4,"total_input_tokens":10,"cache_read_tokens":5,"cache_write_tokens":2}})
        );
        // Stream-only turns get their native row, kept with their capture.
        let (stream_only, captured) = usage("claude-thread", "stream-only");
        assert_eq!(stream_only["usage"]["cost_usd"], 0.5);
        assert_eq!(captured, Some(1));
        let (codex, _) = usage("codex-thread", "codex-turn");
        assert_eq!(codex["provider"], "codex");
        assert_eq!(codex["usage"]["input_tokens"], 40);
        let (native_codex, _) = usage("codex-thread", "native-codex");
        assert_eq!(native_codex["total"]["inputTokens"], 30);
        let usage_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM session_events WHERE kind='usage'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(usage_rows, 5, "usage without a provider thread is dropped");

        let stream: Vec<i64> = conn
            .prepare(
                "SELECT seq FROM session_events WHERE kind='observed'
                 AND instr(receipt_key,':events.jsonl:')>0 ORDER BY seq",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(stream, [3, 8, 10, 31, 32]);
        let silences: Vec<(String, String)> = conn
            .prepare(
                "SELECT receipt_key,json_extract(payload,'$.evidence') FROM session_events
                 WHERE instr(receipt_key,':silence:')>0",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            silences,
            [(
                "run_00000000000000000000000000000001:silence:3".to_string(),
                r#"{"since":1020,"observed_at":1900}"#.to_string()
            )]
        );
        let exec: Option<String> = conn
            .query_row(
                "SELECT json_extract(payload,'$.evidence.exec') FROM session_events
                 WHERE receipt_key LIKE '%:manifest.json'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exec, None);

        let triggers: String = conn
            .query_row(
                "SELECT group_concat(sql,char(10)) FROM sqlite_master WHERE type='trigger'
                 AND (name LIKE 'store_revision_session_events_%' OR name='store_revision_usage_insert')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(triggers.matches("CREATE TRIGGER").count(), 4);
        for transcript in [
            "events.jsonl",
            "provider_output",
            "conversation",
            "evidence",
        ] {
            assert!(!triggers.contains(transcript), "{transcript} in {triggers}");
        }
        let revision = |domain: &str| -> i64 {
            conn.query_row(
                "SELECT revision FROM store_revisions WHERE domain=?1",
                [domain],
                |row| row.get(0),
            )
            .unwrap()
        };
        let (sessions, usage_revision) = (revision("sessions"), revision("usage"));
        conn.execute(
            "UPDATE session_events SET payload=payload WHERE provider_turn='raw'",
            [],
        )
        .unwrap();
        assert_eq!(
            revision("sessions"),
            sessions,
            "usage is not a Session change"
        );
        conn.execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
             VALUES('conversation','claude-thread','next','usage','',3000,'{}')",
            [],
        )
        .unwrap();
        assert_eq!(revision("usage"), usage_revision + 1);
    }

    /// Apply the catalog up to the named migration, as a fixture released
    /// before it would have it.
    pub(crate) fn apply_before_migration(conn: &rusqlite::Connection, name: &str) {
        apply_before_draft(conn, name);
    }

    /// Finish a historical fixture's partial migration before using current readers.
    pub(crate) fn upgrade_from_migration(conn: &rusqlite::Connection, name: &str) {
        super::_migration_transaction(conn, |conn, guards| {
            let section = section(name);
            conn.execute_batch(&section.sql[section.start..])?;
            super::convert_released_data(conn, section.sql)?;
            super::insert_applied_migration(
                conn,
                &MIGRATIONS[section.index],
                &super::migration_prefix_fingerprint(&MIGRATIONS[..section.index]),
            )?;
            apply_set(conn, MIGRATIONS, guards)?;
            super::validate_foreign_keys(conn)
        })
        .unwrap();
    }

    /// Bring a fixture seeded at a released frontier to this build's schema.
    pub(crate) fn apply_pending_migrations(conn: &rusqlite::Connection) {
        super::_migration_transaction(conn, |conn, guards| apply_set(conn, MIGRATIONS, guards))
            .unwrap();
    }

    /// One migration's own SQL: a slug file, or a released batch's section.
    pub(crate) fn migration_section_sql(name: &str) -> String {
        let section = section(name);
        section.sql[section.body].to_string()
    }

    #[test]
    fn ownership_log_records_released_evidence_without_inventing_owners() {
        let conn = open();
        apply_before_migration(&conn, "task_ownership");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/ownership_log_released.sql"
        ))
        .unwrap();
        super::_migration_transaction(&conn, |conn, _| {
            conn.execute_batch(&migration_section_sql("task_ownership"))?;
            validate_foreign_keys(conn)
        })
        .unwrap();
        type Entry = (String, String, Option<String>, Option<String>, i64);
        let entries: Vec<Entry> = conn
            .prepare(
                "SELECT task_id,kind,person,machine_id,at FROM task_ownership ORDER BY task_id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let recorded = |task: &str, machine: Option<&str>, at| {
            (
                task.to_string(),
                "recorded".to_string(),
                None,
                machine.map(str::to_string),
                at,
            )
        };
        assert_eq!(
            entries,
            vec![
                recorded("task_admitted", Some("home_admitted"), 12),
                recorded("task_checkout", Some("home_checkout"), 11),
                recorded("task_started", None, 20),
            ]
        );
        for table in ["task_admissions", "work_placements"] {
            assert!(conn.prepare(&format!("SELECT * FROM {table}")).is_err());
        }
        let retired: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('tasks') WHERE name IN ('started_at','admission_authority')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retired, 0);
        // The Machine registry is the person's: a label survives, a checkout does not.
        let label: String = conn
            .query_row(
                "SELECT label FROM machines WHERE id='home_labeled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(label, "mini");
        assert!(conn.prepare("SELECT repo FROM machines").is_err());
        let journal: Vec<String> = conn
            .prepare("SELECT id FROM planning_peer_changes ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(journal, vec!["kept-1".to_string()]);
        assert!(conn
            .execute("UPDATE task_ownership SET person='Jack'", [])
            .is_err());
        assert!(conn.execute("DELETE FROM task_ownership", []).is_err());
        conn.execute("UPDATE task_ownership SET ordered_at=ordered_at", [])
            .unwrap_err();
    }

    #[test]
    fn optional_task_ancestry_preserves_released_work_and_current_defaults() {
        let conn = open();
        apply_before_migration(&conn, "optional_task_ancestry");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/optional_task_ancestry.sql"
        ))
        .unwrap();
        super::_migration_transaction(&conn, |conn, _| {
            conn.execute_batch(&migration_section_sql("optional_task_ancestry"))?;
            validate_foreign_keys(conn)
        })
        .unwrap();
        let task: (String, String, String, String) = conn.query_row(
            "SELECT repository_id,project_id,worktree,branch FROM tasks WHERE id='task_retained'", [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
        assert_eq!(
            task,
            (
                "repo_retained".into(),
                "proj_current".into(),
                "/retained/task".into(),
                "branch".into()
            )
        );
        let defaults: Vec<(String, Option<String>)> = conn
            .prepare("SELECT id,workflow FROM waves ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            defaults,
            vec![
                ("wave-retained".into(), Some("code".into())),
                ("wave-unselected".into(), None)
            ]
        );
        let receipt: (String, bool, String) = conn
            .query_row(
                "SELECT repository_id,export_attempted,export_json FROM task_creation_intents",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            receipt,
            (
                "repo_retained".into(),
                true,
                r#"{"id":"reserved-issue"}"#.into()
            )
        );
        conn.execute(
            "UPDATE tasks SET project_id=NULL WHERE id='task_retained'",
            [],
        )
        .unwrap();
        let current: Option<String> = conn
            .query_row("SELECT wave_id FROM agent_sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(current, None);
        let capture: String = conn
            .query_row("SELECT wave_id FROM session_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(capture, "wave-retained");
        let position: String = conn
            .query_row("SELECT node FROM task_workflows", [], |row| row.get(0))
            .unwrap();
        assert_eq!(position, "review");
        assert!(!conn
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap());
    }

    #[test]
    fn paused_migration_keeps_existing_tasks_unpaused_and_retains_work() {
        let conn = open();
        apply_before_migration(&conn, "task_paused");
        conn.execute_batch("INSERT INTO waves(id,name,repo,created_at) VALUES('w','product','/repo',1);
            INSERT INTO projects(id,wave_id,created_at) VALUES('p','w',1);
            INSERT INTO tasks(id,project_id,issue_identifier,issue_title,worktree,created_at) VALUES('t','p','PROOF-1','Retained','/repo/task',1);").unwrap();
        conn.execute_batch(&migration_section_sql("task_paused"))
            .unwrap();
        let task: (bool, String, String) = conn
            .query_row(
                "SELECT paused,issue_title,worktree FROM tasks WHERE id='t'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(task, (false, "Retained".into(), "/repo/task".into()));
    }

    #[test]
    fn task_flow_observations_removes_scheduling_and_worker_control_and_preserves_history() {
        let conn = open();
        apply_before_migration(&conn, "task_flow_observations");
        conn.execute_batch(r#"
            INSERT INTO execs(id,trace_id,started_at,outcome) VALUES('old-exec','trace',1,'interrupted');
            INSERT INTO waves(id,name,repo,created_at) VALUES('w','product','/repo',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at,flow) VALUES('p','w','linear-p',1,'custom');
            INSERT INTO pm_projects(repo,provider,id,observed_at,body)
            VALUES('/repo','linear','linear-p',1,'{"id":"linear-p","flow":"custom"}');
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,worktree,
              automation_enabled,automation_exec_id,automation_retry_key,automation_retries,
              automation_checked_at,automation_detail)
            VALUES('t','p','linear-t','LOO-1',1,'/repo/task',0,'old-exec','old-position',3,20,'stopped');
            INSERT INTO task_events(task_id,kind_json,created_at) VALUES('t','{"kind":"started"}',17);
            INSERT INTO agent_sessions(id,title,title_source,created_at,ready_summary,completed_at,input_published,cwd,kind,task_id,wave_id)
            VALUES('review','Review','human',3,'Retained exact feedback',NULL,0,'/repo/task','flow_review','t','w'),
                  ('closed','Closed','human',4,'Historical result',7,0,'/repo/task','flow_review','t','w');
            INSERT INTO flow_sessions(id,task_id,wave_id,invocation_json,step_index,iteration,position_version,worker_generation,claim_json,updated_at,state,pending_session_id)
            VALUES('legacy-flow','t','w','{"id":"legacy-flow","flow":"feature"}',2,3,4,7,'{"generation":7}',5,'current','review');
            UPDATE tasks SET current_invocation_id='legacy-flow' WHERE id='t';
            UPDATE agent_sessions SET flow_session_id='legacy-flow' WHERE id='review';
            INSERT INTO flow_events(flow_id,version,node,iterations,kind,observed_at,payload)
            VALUES('legacy-flow',4,2,'[]','operation_started',5,'{"command":"pr publish"}');
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,worktree)
            VALUES('flow-only','p','linear-f','LOO-2',1,'/repo/flow-only');
            INSERT INTO flow_sessions(id,task_id,wave_id,invocation_json,step_index,iteration,position_version,worker_generation,updated_at,state)
            VALUES('only','flow-only','w','{"id":"only","flow":"code"}',0,0,1,0,9,'current');
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,updated_at,worktree,work_state,work_terminal_at)
            VALUES('finished','p','linear-d','LOO-3',1,30,'/repo/finished','done',25),
                  ('dropped','p','linear-a','LOO-4',1,30,'/repo/dropped','abandoned',26);
        "#).unwrap();
        conn.execute_batch(&migration_section_sql("task_flow_observations"))
            .unwrap();
        // A Task's state is now its Workflow position. A done Task stands at
        // the end of a workflow with nothing between, placed by a move no
        // process made; abandoned keeps its own mark; the rest have no
        // Workflow until they next run.
        let states: Vec<(String, String)> = conn
            .prepare(&format!(
                "SELECT t.id,{} FROM tasks t ORDER BY t.id",
                "CASE WHEN t.abandoned_at IS NOT NULL THEN 'abandoned' WHEN EXISTS(SELECT 1 FROM task_workflows w WHERE w.task_id=t.id AND w.node='end') THEN 'done' ELSE 'not_ready' END"
            ))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            states,
            [
                ("dropped", "abandoned"),
                ("finished", "done"),
                ("flow-only", "not_ready"),
                ("t", "not_ready")
            ]
            .map(|(id, state)| (id.to_string(), state.to_string()))
        );
        let placed: (String, i64, Option<String>, i64, i64) = conn
            .query_row(
                "SELECT w.graph,w.updated_at,m.exec_id,m.at,t.abandoned_at IS NULL
                 FROM task_workflows w JOIN task_workflow_moves m ON m.task_id=w.task_id
                 JOIN tasks t ON t.id=w.task_id WHERE w.task_id='finished'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        // That release's Workflow shape; `ship_it` later empties it to a graph.
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&placed.0).unwrap(),
            serde_json::json!({"name":"unplanned","nodes":[],"edges":[{"from":"start","to":"end","flow":null}]})
        );
        assert_eq!((placed.1, placed.2, placed.3, placed.4), (25, None, 25, 1));
        assert_eq!(
            conn.query_row(
                "SELECT abandoned_at FROM tasks WHERE id='dropped'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            26
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM pragma_table_info('tasks') WHERE name IN ('work_state','work_terminal_at')",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        let feedback: (String, Option<i64>) = conn
            .query_row(
                "SELECT json_extract(e.payload,'$.summary'),s.completed_at FROM agent_sessions s
             JOIN session_events e ON e.session_id=s.id AND e.receipt_key='legacy_review_feedback'
             WHERE s.id='review'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(feedback, ("Retained exact feedback".into(), None));
        let boundary: (String, i64, Option<i64>) = conn.query_row(
            "SELECT json_extract(e.payload,'$.flow_id'),json_extract(e.payload,'$.pending'),s.completed_at FROM agent_sessions s JOIN session_events e ON e.session_id=s.id AND e.receipt_key='legacy_flow_review' WHERE s.id='review'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(boundary, ("legacy-flow".into(), 1, None));
        // The saved Flow's name, state and last position stay on the
        // conversation it opened; nothing else of the Flow record remains.
        let history: (String, String, i64, i64) = conn.query_row(
            "SELECT json_extract(e.payload,'$.flow'),json_extract(e.payload,'$.state'),
                json_extract(e.payload,'$.step_index'),json_extract(e.payload,'$.iteration')
             FROM session_events e WHERE e.session_id='review' AND e.receipt_key='legacy_flow:legacy-flow'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap();
        assert_eq!(history, ("feature".into(), "current".into(), 2, 3));
        let control: i64 = conn.query_row(
            "SELECT (SELECT count(*) FROM pragma_table_info('tasks') WHERE name='current_invocation_id')
                + (SELECT count(*) FROM pragma_table_info('agent_sessions') WHERE name='flow_session_id')
                + (SELECT count(*) FROM sqlite_master WHERE name IN ('flow_sessions','flow_events')
                    OR sql LIKE '%flow_sessions%' OR sql LIKE '%flow_events%')",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(control, 0);
        // A Project's stored name for its Tasks' workflow keeps its value.
        let workflow: (String, String, Option<String>) = conn
            .query_row(
                "SELECT p.workflow,json_extract(m.body,'$.workflow'),json_extract(m.body,'$.flow')
                 FROM projects p JOIN pm_projects m ON m.id=p.external_project_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(workflow, ("custom".into(), "custom".into(), None));
        // No saved Flow becomes a Flow exec: the Flow process record starts empty.
        let flows: i64 = conn
            .query_row(
                "SELECT (SELECT count(*) FROM flow_execs)+(SELECT count(*) FROM flow_exec_steps)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(flows, 0);
        // A conversation still cannot leave its Task, and Started is still set once.
        assert!(conn
            .execute(
                "UPDATE agent_sessions SET task_id=NULL WHERE id='review'",
                []
            )
            .is_err());
        assert_eq!(
            conn.query_row(
                "SELECT started_at FROM tasks WHERE id='flow-only'",
                [],
                |row| row.get::<_, Option<i64>>(0)
            )
            .unwrap(),
            Some(9),
            "a Task whose only start evidence was its Flow row keeps it"
        );
        assert_eq!(
            conn.query_row(
                "SELECT completed_at FROM agent_sessions WHERE id='closed'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            7
        );
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM pragma_table_info('agent_sessions') WHERE name='ready_summary'", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        let retained: (String, bool, String) = conn.query_row(
            "SELECT worktree,automation_enabled,kind_json FROM tasks JOIN task_events ON tasks.id=task_events.task_id WHERE tasks.id='t'",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        assert_eq!(
            retained,
            ("/repo/task".into(), false, r#"{"kind":"started"}"#.into())
        );
        let retired: i64 = conn.query_row(
            "SELECT count(*) FROM pragma_table_info('tasks') WHERE name IN ('automation_exec_id','automation_retry_key','automation_retries','automation_checked_at','automation_detail')",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(
            conn.query_row("SELECT outcome FROM execs WHERE id='old-exec'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "interrupted"
        );
        assert_eq!(retired, 0);
    }

    #[test]
    fn task_agent_migration_preserves_existing_tasks() {
        let conn = open();
        apply_before_migration(&conn, "task_agent");
        conn.execute_batch("INSERT INTO waves (id, name, repo, created_at)
            VALUES ('wave_agent', 'agent', '/repo', 1);
            INSERT INTO projects (id, wave_id, external_project_id, created_at, updated_at)
            VALUES ('project_agent', 'wave_agent', 'linear-project', 1, 1);
            INSERT INTO tasks (id, project_id, external_issue_id, issue_identifier, worktree, workspace_slug, created_at, updated_at)
            VALUES ('task_agent', 'project_agent', 'linear-issue', 'TEST-1', '/repo.task', 'task-agent', 1, 1);").unwrap();
        conn.execute_batch(&migration_section_sql("task_agent"))
            .unwrap();
        let retained = conn
            .query_row(
                "SELECT issue_identifier, worktree, agent FROM tasks WHERE id='task_agent'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(retained, ("TEST-1".into(), "/repo.task".into(), None));
    }

    #[test]
    fn task_issue_identities_migration_preserves_confirmation_and_planning() {
        let conn = open();
        apply_before_migration(&conn, "task_issue_identities");
        conn.execute_batch(
            r#"INSERT INTO waves(id,name,repo,created_at) VALUES('wave_history','history','/repo',100);
             INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at)
               VALUES('wave_history','deleted','FIX-1',150);
             INSERT INTO pm_snapshots(wave_id,provider,initiative,synced_at,payload)
               VALUES('wave_history','linear','initiative',200,'{"items":[{"id":"observed"}]}');
             INSERT INTO wave_chapters(wave_id,chapter_id,project_id,current,receipt)
               VALUES('wave_history','old','linear-project',1,'{"tasks":[{"applied":true}]}');"#,
        ).unwrap();
        conn.execute_batch(&migration_section_sql("task_issue_identities"))
            .unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM task_issue_identities", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT issue_id,identifier,confirmed_at FROM task_deletions",
                [],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?
                ))
            )
            .unwrap(),
            ("deleted".into(), "FIX-1".into(), 150)
        );
        assert_eq!(
            conn.query_row("SELECT payload FROM pm_snapshots", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            r#"{"items":[{"id":"observed"}]}"#
        );
        assert_eq!(
            conn.query_row("SELECT receipt FROM wave_chapters", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            r#"{"tasks":[{"applied":true}]}"#
        );
    }

    #[test]
    fn task_deletions_migration_preserves_terminal_history_without_inventing_confirmation() {
        let conn = open();
        apply_before_migration(&conn, "task_deletions");
        conn.execute_batch(
            r#"INSERT INTO waves(id,name,repo,created_at) VALUES('wave_history','history','/repo',100);
             INSERT INTO projects(id,wave_id,external_project_id,created_at,updated_at)
               VALUES('project_history','wave_history','linear-project',100,200);
             INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,updated_at,work_state,work_terminal_at)
               VALUES('task_old','project_history','old-issue','FIX-1',100,200,'abandoned',150),
                     ('task_done','project_history','done-issue','FIX-2',100,200,'done',160);
             INSERT INTO pm_snapshots(wave_id,provider,initiative,synced_at,payload)
               VALUES('wave_history','linear','initiative',200,'{"items":[{"id":"old-issue"}]}');
             INSERT INTO wave_chapters(wave_id,chapter_id,project_id,current,receipt)
               VALUES('wave_history','old','linear-project',1,'{"tasks":[{"applied":true}]}');"#
        ).unwrap();
        conn.execute_batch(&migration_section_sql("task_deletions"))
            .unwrap();
        let rows = conn
            .prepare("SELECT id,work_state,work_terminal_at,updated_at FROM tasks ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            rows,
            vec![
                ("task_done".into(), "done".into(), 160, 200),
                ("task_old".into(), "abandoned".into(), 150, 200)
            ]
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM task_deletions", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row("SELECT payload FROM pm_snapshots", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            r#"{"items":[{"id":"old-issue"}]}"#
        );
        assert_eq!(
            conn.query_row("SELECT receipt FROM wave_chapters", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            r#"{"tasks":[{"applied":true}]}"#
        );
    }

    #[test]
    fn machine_connections_preserve_released_records() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "rename_home_to_machine");
        conn.execute_batch("INSERT INTO homes (id, route, created_at, observed_at)
            VALUES ('home_11111111111111111111111111111111', 'ssh://jack@mini', 9, 10);
            INSERT INTO waves (id, name, repo, created_at) VALUES ('machine-wave', 'machine', '/repo', 7);
            INSERT INTO work_placements (wave_id, home_id, placed_at)
            VALUES ('machine-wave', 'home_11111111111111111111111111111111', 8);").unwrap();
        conn.execute_batch(&migration_section_sql("rename_home_to_machine"))
            .unwrap();
        conn.execute_batch(&migration_section_sql("machine_connections"))
            .unwrap();
        let row: (String, i64, i64, Option<String>, Option<String>, i64) = conn.query_row(
            "SELECT m.route, m.created_at, m.observed_at, m.label, m.repo, p.placed_at
             FROM machines m JOIN work_placements p ON p.machine_id=m.id WHERE p.wave_id='machine-wave'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))).unwrap();
        assert_eq!(row, ("ssh://jack@mini".into(), 9, 10, None, None, 8));
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn machine_rename_preserves_released_identity_and_placement() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "rename_home_to_machine");
        let id: String = conn
            .query_row("SELECT id FROM homes WHERE route='local'", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute_batch(
            "INSERT INTO waves (id, name, repo, created_at)
            VALUES ('rename-wave', 'rename', '/repo', 7);
            INSERT INTO work_placements (wave_id, home_id, placed_at)
            SELECT 'rename-wave', id, 8 FROM homes WHERE route='local';
            INSERT INTO homes (id, route, created_at, observed_at)
            VALUES ('home_00000000000000000000000000000001', 'ssh://jack@mini', 9, 10);",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("rename_home_to_machine"))
            .unwrap();
        let retained: (String, i64) = conn
            .query_row(
                "SELECT machine_id, placed_at FROM work_placements WHERE wave_id='rename-wave'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(retained, (id.clone(), 8));
        assert!(crate::durable::MachineId::parse(&id).is_ok());
        let remote: (String, i64, i64) = conn.query_row(
            "SELECT route, created_at, observed_at FROM machines WHERE id='home_00000000000000000000000000000001'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
        assert_eq!(remote, ("ssh://jack@mini".into(), 9, 10));
        assert!(conn.prepare("SELECT * FROM homes").is_err());
        assert!(conn
            .execute("UPDATE work_placements SET machine_id='missing'", [])
            .is_err());
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn landing_timeout_removal_preserves_intent_and_recovers_synthetic_blocks() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "remove_landing_timeouts");
        conn.execute_batch(include_str!(
            "../../tests/fixtures/migrations/landing_timeouts.sql"
        ))
        .unwrap();
        conn.execute_batch(&migration_section_sql("remove_landing_timeouts"))
            .unwrap();
        let state: (String, String, String, Option<String>) = conn.query_row(
            "SELECT state,requested_head_sha,observed_head_sha,blocked_reason FROM pr_landings WHERE id='slow'",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
        ).unwrap();
        assert_eq!(
            state,
            (
                "watching".into(),
                "requested".into(),
                "observed".into(),
                None
            )
        );
        let blocker: String = conn
            .query_row(
                "SELECT blocked_reason FROM pr_landings WHERE id='blocked'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(blocker, "credential revoked");
        let columns: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('pr_landings')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!columns.iter().any(|name| name.starts_with("ci_")));
        validate_foreign_keys(&conn).unwrap();
    }

    #[tokio::test]
    async fn retired_home_landings_remain_readable_and_fence_old_supervisors() {
        use std::sync::Arc;

        use crate::ops::pr_landing::{
            reconcile_pr_landing, LandingIo, LandingObservation, ObservePr,
        };
        use crate::pr_landing::{LandingPlacement, LandingSupervisor, PrLandingId, PrLandingState};
        use crate::store::sqlite::SqliteStore;
        use time::OffsetDateTime;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("registry.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_before_migration(&conn, "retire_home_landing_supervisors");
        conn.execute_batch(
            "INSERT INTO homes (id, route, created_at, observed_at)
             VALUES ('retired_home', 'remote', 1, 2);
             INSERT INTO waves (id, name, repo, created_at)
             VALUES ('landing_wave', 'landing', '/repo', 1);
             INSERT INTO projects (id, wave_id, external_project_id, created_at, updated_at)
             VALUES ('landing_project', 'landing_wave', 'linear-project', 1, 1);
             INSERT INTO tasks (id, project_id, external_issue_id, issue_identifier,
                worktree, workspace_slug, created_at, updated_at)
             VALUES ('landing_task', 'landing_project', 'linear-task', 'INF-1',
                '/tmp/landing', 'landing', 1, 1);",
        )
        .unwrap();
        for (index, state) in ["watching", "repairing", "blocked", "merged", "closed"]
            .iter()
            .enumerate()
        {
            for placement in ["home", "local"] {
                conn.execute(
                    include_str!("../../tests/fixtures/migrations/retired_home_landing.sql"),
                    rusqlite::params![
                        format!("{placement}_{state}"),
                        index as i64 + if placement == "home" { 1 } else { 11 },
                        (*state == "merged").then_some("merge"),
                        state,
                        placement,
                        (placement == "home").then_some("retired_home"),
                        (*state == "blocked").then_some("credential revoked"),
                    ],
                )
                .unwrap();
            }
        }
        // A taskless retained delivery can settle through the real reconciler.
        assert!(std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(directory.path())
            .status()
            .unwrap()
            .success());
        conn.execute(
            "INSERT INTO pr_landings (id, repo, pr_number, worktree, branch,
                requested_head_sha, observed_head_sha, state, generation,
                supervisor_placement, supervisor_home_id, supervisor_process_id,
                supervisor_heartbeat_at, created_at, updated_at)
             VALUES ('home_taskless', 'taskless/repo', 42, ?1, 'repair', 'head', 'head',
                'watching', 7, 'home', 'retired_home', 123, 30, 10, 40)",
            [directory.path().to_str().unwrap()],
        )
        .unwrap();
        super::_migration_transaction(&conn, |conn, guards| {
            apply_set(conn, MIGRATIONS, guards)?;
            validate_foreign_keys(conn)
        })
        .unwrap();
        drop(conn);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(store.pending_pr_landings("owner/repo").unwrap().len(), 6);
        let now = OffsetDateTime::from_unix_timestamp(50).unwrap();
        for state in ["watching", "repairing", "blocked", "merged", "closed"] {
            for placement in ["home", "local"] {
                let id = PrLandingId::from_raw(format!("{placement}_{state}"));
                let landing = store.get_pr_landing(&id).unwrap().unwrap();
                assert_eq!(landing.state.as_str(), state);
                assert_eq!(landing.requested_head_sha, "requested");
                assert_eq!(landing.observed_head_sha, "observed");
                assert_eq!(landing.task_id.as_ref().unwrap().as_str(), "landing_task");
                let historical: (String, String) = rusqlite::Connection::open(&path)
                    .unwrap()
                    .query_row(
                        "SELECT after_merge,next_slug FROM pr_landings WHERE id=?1",
                        [id.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap();
                assert_eq!(historical, ("continue_task".into(), "follow-up".into()));
                assert_eq!(landing.updated_at.unix_timestamp(), 40);
                assert_eq!(
                    landing.blocked_reason.as_deref(),
                    (state == "blocked").then_some("credential revoked")
                );
                if placement == "local" {
                    assert_eq!(landing.generation, 7);
                    assert_eq!(landing.supervisor.unwrap().process_id, 123);
                    continue;
                }
                assert_eq!(landing.generation, 8);
                assert!(landing.supervisor.is_none());
                assert!(!store.heartbeat_pr_landing(&id, 7, now).unwrap());
                let mut stale = landing;
                stale.generation = 7;
                stale.state = PrLandingState::Closed;
                stale.blocked_reason = None;
                assert!(!store.update_pr_landing(&stale).unwrap());
                assert!(store
                    .claim_pr_landing(
                        &id,
                        7,
                        &LandingSupervisor {
                            placement: LandingPlacement::Local,
                            process_id: 456,
                            heartbeat_at: now,
                        },
                        now
                    )
                    .unwrap()
                    .is_none());
            }
        }
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(store.pending_pr_landings("owner/repo").unwrap().len(), 6);
        let conn = rusqlite::Connection::open(&path).unwrap();
        assert!(conn.execute(
            "UPDATE pr_landings SET supervisor_placement='home', supervisor_machine_id='retired_home',
                supervisor_process_id=456, supervisor_heartbeat_at=50 WHERE id='home_watching' AND generation=8", [],
        ).is_err());
        let history: String = conn
            .query_row(
                "SELECT retired_supervisor_json FROM pr_landings WHERE id='home_blocked'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&history).unwrap(),
            serde_json::json!({
                "placement": "home", "home_id": "retired_home", "process_id": 123,
                "heartbeat_at": 30, "generation": 7,
            })
        );
        validate_foreign_keys(&conn).unwrap();
        drop(conn);
        drop(store);

        // A merged delivery is observed without ever being offered a repair.
        let merged_observation: ObservePr = Arc::new(|_| {
            Ok(LandingObservation::Merged {
                head_sha: "head".into(),
                merge_commit: "merge".into(),
            })
        });
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(path))
                .await
                .unwrap(),
        );
        let id = PrLandingId::from_raw("home_taskless");
        let landing = store.get_pr_landing(&id).await.unwrap().unwrap();
        let merged = reconcile_pr_landing(
            store.clone(),
            landing,
            LandingIo::scripted(merged_observation.clone(), None),
        )
        .await
        .unwrap();
        assert_eq!(merged.state, PrLandingState::Merged);
        assert_eq!(merged.merge_commit.as_deref(), Some("merge"));
        let repeated = reconcile_pr_landing(
            store.clone(),
            merged.clone(),
            LandingIo::scripted(merged_observation, None),
        )
        .await
        .unwrap();
        assert_eq!(repeated, merged);
        assert_eq!(
            store.pending_pr_landings("owner/repo").await.unwrap().len(),
            6
        );
    }

    #[test]
    fn landing_repair_counter_removal_preserves_supervision() {
        let conn = open();
        let name = "remove_landing_repair_counter";
        apply_before_migration(&conn, name);
        conn.execute_batch(
            "INSERT INTO pr_landings (
                id, repo, pr_number, worktree, branch, requested_head_sha,
                observed_head_sha, state, generation, supervisor_placement,
                supervisor_process_id, supervisor_heartbeat_at, repair_count,
                blocked_reason, created_at, updated_at
             ) VALUES (
                'landing_history', 'owner/repo', 42, '/tmp/landing', 'repair',
                'failed', 'repaired', 'blocked', 7, 'local', 123, 30, 4,
                'credential revoked', 10, 40
             );",
        )
        .unwrap();
        let retained = columns(&conn, "pr_landings")
            .into_iter()
            .filter(|column| column != "repair_count")
            .collect::<Vec<_>>();
        let read_landing = || {
            conn.query_row(
                &format!("SELECT {} FROM pr_landings", retained.join(",")),
                [],
                |row| {
                    (0..retained.len())
                        .map(|index| row.get::<_, rusqlite::types::Value>(index))
                        .collect::<rusqlite::Result<Vec<_>>>()
                },
            )
            .unwrap()
        };
        let before = read_landing();

        conn.execute_batch(&migration_section_sql(name)).unwrap();

        assert_eq!(read_landing(), before);
        assert!(!columns(&conn, "pr_landings").contains(&"repair_count".to_string()));
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn stable_work_draft_keeps_definitions_and_clears_execution_state() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_before_migration(&conn, "stable_work_state");
        if !columns(&conn, "tasks").contains(&"work_state".to_string()) {
            conn.execute_batch(&migration_section_sql("opaque_steer_run_provenance"))
                .unwrap();
        }
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
             VALUES ('wave_keep', 'keep', '/repo', 1700000000, NULL)",
            [],
        )
        .unwrap();

        conn.execute_batch(&migration_section_sql("stable_work_state"))
            .unwrap();

        let state: (String, Option<i64>) = conn
            .query_row(
                "SELECT work_state, work_terminal_at FROM waves WHERE id='wave_keep'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, ("ready".to_string(), None));
        for table in [
            "waits",
            "work_truth",
            "epoch_revisions",
            "work_flow_positions",
        ] {
            let exists: bool = conn
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1
                     )",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!exists, "{table} must be deleted");
        }
        for table in ["steers", "tool_responses", "ask_exchanges"] {
            let rows: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(rows, 0, "{table} must start empty");
        }
    }

    #[test]
    fn obsolete_sql_lifecycle_draft_preserves_only_the_outer_exec_ledger() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
        apply_before_migration(&conn, "obsolete_sql_lifecycle");
        if !columns(&conn, "tasks").contains(&"work_state".to_string()) {
            for draft in ["opaque_steer_run_provenance", "stable_work_state"] {
                conn.execute_batch(&migration_section_sql(draft)).unwrap();
            }
        }
        conn.execute(
            "INSERT INTO run_events (run_id, process_id, seq, ts, node, event)
             VALUES ('trace-keep', 'exec-keep', 0, 1, 'run', 'started')",
            [],
        )
        .unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        conn.execute_batch(&migration_section_sql("obsolete_sql_lifecycle"))
            .unwrap();

        validate_foreign_keys(&conn).unwrap();
        for table in [
            "runs",
            "epochs",
            "run_liveness",
            "home_upgrade_work",
            "home_upgrades",
            "home_runtime_generations",
            "agent_invocations",
            "agent_turns",
            "turn_usage_samples",
            "context_assets",
            "context_decisions",
            "done_proposals",
            "sends",
            "performance_evidence_authority",
        ] {
            let exists = conn
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1
                     )",
                    [table],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap();
            assert!(!exists, "{table} must be deleted");
        }
        assert!(!columns(&conn, "project_events").contains(&"run_id".to_string()));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM run_events", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
    }

    #[test]
    fn task_lifecycle_outcome_is_removed_from_the_persisted_model() {
        let conn = open();
        apply_before_migration(&conn, REMOVE_TASK_LIFECYCLE_OUTCOME_NAME);
        assert!(columns(&conn, "tasks")
            .iter()
            .any(|column| column == "lifecycle_outcome"));

        conn.execute_batch(&migration_section_sql(REMOVE_TASK_LIFECYCLE_OUTCOME_NAME))
            .unwrap();

        assert!(!columns(&conn, "tasks")
            .iter()
            .any(|column| column == "lifecycle_outcome"));
        for column in ["pr_title", "pr_body", "pr_copy_head_sha"] {
            assert!(columns(&conn, "task_prs").iter().any(|name| name == column));
        }
    }

    #[test]
    fn run_identity_removes_secret_columns_and_preserves_run_invariants() {
        let conn = open();
        apply_before_migration(&conn, RUN_IDENTITY_NAME);
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&migration_section_sql(RUN_IDENTITY_NAME))
            .unwrap();
        validate_foreign_keys(&conn).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();

        let run_columns = columns(&conn, "runs");
        assert!(!run_columns.contains(&"lease_hash".to_string()));
        assert!(!run_columns.contains(&"lease_generation".to_string()));
        for object in [
            "idx_runs_one_active_epoch",
            "idx_runs_runtime_generation",
            "runs_execution_shape_insert",
            "runs_execution_shape_update",
            "runs_preserve_first_material",
        ] {
            assert!(conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1)",
                    [object],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap());
        }
        for removed in ["idx_runs_lease_hash", "idx_runs_source_generation"] {
            assert!(!conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=?1)",
                    [removed],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap());
        }
    }

    #[test]
    fn turn_usage_samples_move_provider_receipts_out_of_turn_lifecycle() {
        let conn = open();
        apply_before_migration(&conn, TURN_USAGE_SAMPLES_NAME);
        conn.execute_batch(
            "INSERT INTO agent_invocations (
                id, run_id, process_id, started_at, ended_at, repo, worktree,
                provider, model, surface, capture_status, outcome, artifact_dir,
                conversation_path, conversation_event_count, conversation_bytes
             ) VALUES (
                'invocation-measured', 'run-measured', 'process-measured', 90, 110,
                '/repo', '/repo', 'codex', 'gpt-5', 'headless', 'complete',
                'completed', 'artifact', 'conversation.jsonl', 2, 100
             );
             INSERT INTO agent_turns (
                id, invocation_id, ordinal, started_at, ended_at, status, input_op,
                context_coverage, tokenizer, task_prompt_path, system_tokens,
                task_tokens, supplied_context_tokens, provider_input_tokens,
                provider_total_input_tokens, peak_input_tokens, context_window_tokens,
                provider_output_tokens, reasoning_tokens, cache_read_tokens,
                cache_write_tokens, cost_usd, context_gather_ms, context_render_ms,
                context_persist_ms, root_output
             ) VALUES (
                'turn-measured', 'invocation-measured', 1, 100, 110, 'completed',
                'initial', 'assembled', 'provider', 'task.md', 11, 12, 13,
                101, 401, 390, 1000000, 202, 88, 303, 17, 1.25, 2, 3, 4,
                'done'
             );
             INSERT INTO agent_turns (
                id, invocation_id, ordinal, started_at, ended_at, status, input_op,
                context_coverage, tokenizer, task_prompt_path, system_tokens,
                task_tokens, supplied_context_tokens, context_gather_ms,
                context_render_ms, context_persist_ms
             ) VALUES (
                'turn-unmeasured', 'invocation-measured', 2, 111, 112, 'completed',
                'message', 'unknown', 'provider', 'task.md', 0, 0, 0, 0, 0, 0
             );
             INSERT INTO context_assets (
                turn_id, position, channel, kind, scope, label, source_path,
                included_by, content_sha256, byte_start, byte_end, bytes,
                isolated_tokens, attributed_tokens
             ) VALUES (
                'turn-measured', 0, 'task', 'repo_instructions', 'repo',
                'AGENTS.md', 'AGENTS.md', 'test', 'hash', 0, 12, 12, 3, 3
             );",
        )
        .unwrap();

        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&migration_section_sql(TURN_USAGE_SAMPLES_NAME))
            .unwrap();
        validate_foreign_keys(&conn).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();

        let turn_columns = columns(&conn, "agent_turns");
        for removed in [
            "provider_input_tokens",
            "provider_total_input_tokens",
            "peak_input_tokens",
            "context_window_tokens",
            "provider_output_tokens",
            "reasoning_tokens",
            "cache_read_tokens",
            "cache_write_tokens",
            "cost_usd",
        ] {
            assert!(!turn_columns.contains(&removed.to_string()));
        }
        assert_eq!(
            conn.query_row(
                "SELECT turn_id, observed_at, final_receipt, input_tokens,
                        total_input_tokens, peak_input_tokens, context_window_tokens,
                        output_tokens, reasoning_tokens, cache_read_tokens,
                        cache_write_tokens, model, cost_usd
                 FROM turn_usage_samples",
                [],
                |row| {
                    Ok((
                        (
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, bool>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, i64>(6)?,
                        ),
                        (
                            row.get::<_, i64>(7)?,
                            row.get::<_, i64>(8)?,
                            row.get::<_, i64>(9)?,
                            row.get::<_, i64>(10)?,
                            row.get::<_, String>(11)?,
                            row.get::<_, f64>(12)?,
                        ),
                    ))
                },
            )
            .unwrap(),
            (
                (
                    "turn-measured".to_string(),
                    110,
                    true,
                    101,
                    401,
                    390,
                    1_000_000,
                ),
                (202, 88, 303, 17, "gpt-5".to_string(), 1.25),
            )
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM agent_turns", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row(
                "SELECT root_output FROM agent_turns WHERE id='turn-measured'",
                [],
                |row| { row.get::<_, String>(0) }
            )
            .unwrap(),
            "done"
        );
        assert_eq!(
            conn.query_row(
                "SELECT turn_id, label, attributed_tokens FROM context_assets",
                [],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            )
            .unwrap(),
            ("turn-measured".to_string(), "AGENTS.md".to_string(), 3),
            "Turn-owned context evidence survives the table rebuild"
        );
    }

    #[test]
    fn repository_owned_waves_preserve_uuid_projection_and_allow_duplicate_slugs() {
        let conn = open();
        apply_before_migration(&conn, REPOSITORY_OWNED_WAVES_NAME);
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at) VALUES ('wave-a', 'infrastructure', '/repo/a', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pm_snapshots (repo, wave, provider, initiative, synced_at, payload)
             VALUES ('/repo/a', 'infrastructure', 'linear', 'initiative-a', 2, '{}')",
            [],
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO projects (id, wave_id, external_project_id, created_at)
             VALUES ('project-a', 'wave-a', 'linear-project-a', 3);
             INSERT INTO tasks (id, project_id, external_issue_id, issue_identifier, created_at)
             VALUES ('task-a', 'project-a', 'linear-task-a', 'LOO-127', 4);
             INSERT INTO epochs (
                 id, number, task_id, state, current_rev, created_at, terminal_at
             ) VALUES ('epoch-a', 1, 'task-a', 'done', 0, 5, 6);
             INSERT INTO runs (
                 id, epoch_id, home_id, state, trigger_json, source_kind,
                 source_id, created_at, ended_at
             ) VALUES (
                 'run-a', 'epoch-a', (SELECT id FROM homes LIMIT 1), 'ended',
                 '{\"kind\":\"migration\"}', 'task', 'task-a', 5, 6
             );
             INSERT INTO work_placements (task_id, home_id, placed_at)
             VALUES ('task-a', (SELECT id FROM homes LIMIT 1), 4);",
        )
        .unwrap();

        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&migration_section_sql(REPOSITORY_OWNED_WAVES_NAME))
            .unwrap();
        validate_foreign_keys(&conn).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at) VALUES ('wave-b', 'infrastructure', '/repo/b', 3)",
            [],
        )
        .unwrap();

        assert_eq!(
            conn.query_row(
                "SELECT wave_id || ':' || initiative FROM pm_snapshots",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "wave-a:initiative-a"
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM waves WHERE name = 'infrastructure'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row(
                "SELECT waves.id || ':' || projects.id || ':' || tasks.id || ':' || runs.id
                 FROM runs
                 JOIN epochs ON epochs.id = runs.epoch_id
                 JOIN tasks ON tasks.id = epochs.task_id
                 JOIN projects ON projects.id = tasks.project_id
                 JOIN waves ON waves.id = projects.wave_id
                 WHERE runs.id = 'run-a'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "wave-a:project-a:task-a:run-a"
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM work_placements WHERE task_id = 'task-a'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn repository_owned_waves_abort_on_an_unmatched_pm_projection() {
        let mut conn = open();
        apply_before_migration(&conn, REPOSITORY_OWNED_WAVES_NAME);
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at) VALUES ('wave-a', 'infrastructure', '/repo/a', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pm_snapshots (repo, wave, provider, initiative, synced_at, payload)
             VALUES ('/missing', 'infrastructure', 'linear', 'orphan', 2, '{}')",
            [],
        )
        .unwrap();

        let transaction = conn.transaction().unwrap();
        let error = transaction
            .execute_batch(&migration_section_sql(REPOSITORY_OWNED_WAVES_NAME))
            .unwrap_err();
        assert!(error.to_string().contains("NOT NULL"));
        transaction.rollback().unwrap();

        assert_eq!(columns(&conn, "pm_snapshots")[..2], ["repo", "wave"]);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM waves", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn validation_does_not_apply_a_pending_tail() {
        let conn = open();
        let published = prefix_before("durable_input_spine");
        apply_set(&conn, published, &mut Vec::new()).unwrap();

        validate_sqlite(&conn).unwrap();

        assert_eq!(
            latest_applied_version_sqlite(&conn).unwrap(),
            published.last().map(Migration::version),
            "validation advances nothing"
        );
        assert!(capture_status_accepts(&conn, "pruned"));
        // Bait for the withheld tail: the durable input migration creates the
        // stable Work tables. Their absence proves validation stayed read-only.
        assert!(
            conn.prepare("SELECT id FROM projects LIMIT 0").is_err(),
            "validation must not run the tail's schema change"
        );
    }

    /// The validation primitive underneath the shared-store gate recognizes a
    /// store one migration behind, names the exact pending head, and never
    /// advances the frontier the old reader still recognizes.
    #[test]
    fn validate_recognizes_a_shorter_frontier_and_names_the_pending_head() {
        let installed = &MIGRATIONS[..MIGRATIONS.len() - 1];
        let candidate = MIGRATIONS;

        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        // Bring the store to the frontier the installed binary shipped with.
        apply_set(&conn, installed, &mut Vec::new()).unwrap();
        let installed_frontier = latest_applied_version_sqlite(&conn).unwrap().unwrap();
        assert_eq!(
            installed_frontier,
            installed
                .last()
                .expect("installed set has a head")
                .version()
        );

        // The candidate is ahead by one migration. `validate_sqlite` runs the
        // full (candidate) set the ordinary runtime trusts and must leave the
        // frontier untouched — no unpublished migration becomes durable — while
        // reporting the exact pending head the store open refuses on.
        validate_sqlite(&conn).unwrap();
        assert_eq!(
            super::pending_shared_migration(&conn).unwrap().as_deref(),
            Some(latest_known_version().as_str()),
            "the pending head the ordinary shared open refuses on"
        );
        assert_eq!(
            latest_applied_version_sqlite(&conn).unwrap().unwrap(),
            installed_frontier,
            "validation must not advance the shared frontier"
        );

        // The installed binary — whose set ends at the store's frontier — still
        // opens the store: it is exactly the recognized prefix, nothing pending.
        assert!(
            pending_migrations(&applied_versions(&conn).unwrap(), installed)
                .unwrap()
                .is_empty(),
            "the installed binary must keep reading the store the candidate left alone"
        );

        // Only the promotion boundary advances the frontier to the head.
        apply_set(&conn, candidate, &mut Vec::new()).unwrap();
        assert_eq!(
            latest_applied_version_sqlite(&conn).unwrap().unwrap(),
            latest_known_version()
        );
    }

    /// Retire the pointer, keep the run: the rows are evidence of real work,
    /// only the parent id names nothing.
    #[test]
    fn the_lineage_boundary_migration_retires_ghost_parents_and_keeps_real_ones() {
        let conn = open();
        apply_set(&conn, prefix_before("lineage_boundary"), &mut Vec::new()).unwrap();
        conn.execute_batch(
            "INSERT INTO run_events (run_id, process_id, parent_process_id, seq, ts, node, event)
             VALUES ('trace_a', 'proc_root',   NULL,         0, 100, 'run', 'started'),
                    ('trace_a', 'proc_child',  'proc_root',  0, 101, 'run', 'started'),
                    ('trace_b', 'proc_orphan', 'proc_ghost', 0, 102, 'run', 'started')",
        )
        .unwrap();

        apply_through(&conn, "lineage_boundary");

        let parents = |process: &str| -> Option<Option<String>> {
            conn.query_row(
                "SELECT parent_process_id FROM run_events WHERE process_id = ?1",
                [process],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .unwrap()
        };
        assert_eq!(
            parents("proc_orphan"),
            Some(None),
            "a parent no row records is dropped, and the run itself stays"
        );
        assert_eq!(
            parents("proc_child"),
            Some(Some("proc_root".to_string())),
            "a parent the ledger holds is untouched"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM run_events", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            3,
            "the migration retires pointers, never rows"
        );
        apply_sqlite(&conn).unwrap();
        assert!(!conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='run_events')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
    }

    #[test]
    fn capture_pruned_migration_widens_the_enum_and_keeps_existing_launches() {
        // SQLite bakes CHECK into the table, so `pruned` arrives via a table
        // rebuild. The rebuild must carry every historical launch across —
        // those rows are the token and spend accounting we tombstone rather
        // than delete.
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(
            &conn,
            prefix_before("capture_pruned_state"),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(
            !capture_status_accepts(&conn, "pruned"),
            "pruned must not be a legal status before the migration"
        );
        conn.execute_batch(
            "INSERT INTO agent_launches (
                 id, run_id, process_id, started_at, ended_at, repo, worktree,
                 provider, surface, capture_status, outcome, artifact_dir,
                 conversation_path, conversation_event_count, conversation_bytes
             ) VALUES ('al_history', 'run_history', 'proc_history', 100, 200,
                 '/repo', '/repo', 'codex', 'headless', 'complete', 'completed',
                 'history/dir', 'history/conversation.jsonl', 7, 4096)",
        )
        .unwrap();

        apply_through(&conn, "capture_pruned_state");

        let (status, events, bytes) = conn
            .query_row(
                "SELECT capture_status, conversation_event_count, conversation_bytes
                 FROM agent_launches WHERE id = 'al_history'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(status, "complete");
        assert_eq!((events, bytes), (7, 4096));
        assert!(capture_status_accepts(&conn, "pruned"));
        assert!(
            !capture_status_accepts(&conn, "invented"),
            "the rebuild must keep the enum closed, not drop the CHECK"
        );
        assert_eq!(
            conn.query_row("PRAGMA foreign_key_check", [], |row| row
                .get::<_, String>(0))
                .optional()
                .unwrap(),
            None
        );
    }

    #[test]
    fn capture_terminal_states_preserve_history_and_keep_the_enum_closed() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(
            &conn,
            prefix_before("capture_terminal_states"),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(!capture_status_accepts(&conn, "interrupted"));
        assert!(!capture_status_accepts(&conn, "lost"));

        for (position, status) in ["capturing", "complete", "partial", "prompt_only", "pruned"]
            .into_iter()
            .enumerate()
        {
            conn.execute(
                "INSERT INTO agent_launches (
                     id, run_id, process_id, started_at, ended_at, repo, worktree,
                     provider, surface, capture_status, incomplete_reason, outcome,
                     artifact_dir, conversation_path, conversation_event_count,
                     conversation_bytes
                 ) VALUES (?1, ?2, ?3, 100, 200, '/repo', '/repo', 'codex',
                     'headless', ?4, 'historical reason', ?5, ?6, ?7, 7, 4096)",
                rusqlite::params![
                    format!("history-{status}"),
                    format!("run-{position}"),
                    format!("process-{position}"),
                    status,
                    if status == "capturing" {
                        "running"
                    } else {
                        "completed"
                    },
                    format!("history-{status}/dir"),
                    format!("history-{status}/conversation.jsonl"),
                ],
            )
            .unwrap();
        }

        apply_through(&conn, "capture_terminal_states");

        let mut statuses = conn
            .prepare(
                "SELECT capture_status FROM agent_launches
                 WHERE id LIKE 'history-%' ORDER BY capture_status",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        statuses.sort();
        assert_eq!(
            statuses,
            vec!["capturing", "complete", "partial", "prompt_only", "pruned"]
        );
        for status in [
            "capturing",
            "complete",
            "partial",
            "prompt_only",
            "pruned",
            "interrupted",
            "lost",
        ] {
            assert!(capture_status_accepts(&conn, status), "{status}");
        }
        assert!(!capture_status_accepts(&conn, "invented"));
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn capture_terminal_states_repairs_half_control_trace_receipts() {
        let conn = open();
        apply_set(
            &conn,
            prefix_before("run_launch_attention"),
            &mut Vec::new(),
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO agent_launches (
                 id, run_id, process_id, started_at, ended_at, repo, worktree,
                 provider, surface, capture_status, outcome, artifact_dir,
                 conversation_path, provider_session_id,
                 conversation_event_count, conversation_bytes
             ) VALUES ('legacy-trace', 'trace-run', 'trace-process', 100, 200,
                 '/repo', '/repo', 'codex', 'headless', 'complete', 'completed',
                 'trace/dir', 'trace/conversation.jsonl', 'provider-receipt', 7, 4096)",
        )
        .unwrap();

        apply_set(
            &conn,
            prefix_before("capture_terminal_states"),
            &mut Vec::new(),
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO agent_launches (
                 id, run_id, process_id, started_at, ended_at, repo, worktree,
                 provider, surface, capture_status, outcome, artifact_dir,
                 conversation_path, provider_session_id,
                 conversation_event_count, conversation_bytes, launch_state,
                 containment_kind, containment_id, resume_token
             ) VALUES ('control-shaped', 'control-run', 'control-process', 100, 200,
                 '/repo', '/repo', 'codex', 'headless', 'complete', 'completed',
                 'control/dir', 'control/conversation.jsonl', 'provider-receipt',
                 7, 4096, 'ended', 'process_group', 'control-process',
                 'explicit-resume')",
        )
        .unwrap();

        apply_through(&conn, "capture_terminal_states");

        let legacy_trace = conn
            .query_row(
                "SELECT product_run_id, home_id, containment_kind, containment_id,
                        resume_token
                 FROM agent_launches WHERE id = 'legacy-trace'",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(legacy_trace, (None, None, None, None, None));
        assert_eq!(
            conn.query_row(
                "SELECT resume_token FROM agent_launches WHERE id = 'control-shaped'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "explicit-resume"
        );
    }

    #[test]
    fn provenance_migration_records_checksums_and_the_applying_build() {
        let conn = open();
        apply_sqlite(&conn).unwrap();

        let old_checksum: Option<String> = conn
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = '0.10.001_initial'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let applied_by: (String, String, String, String) = conn
            .query_row(
                "SELECT build_provenance, source_identity, source_revision, package_version
                 FROM schema_migrations WHERE version = '0.11.017_migration_provenance'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert!(old_checksum.is_some());
        assert_eq!(applied_by.0, crate::build_info::provenance().as_str());
        assert_eq!(applied_by.1, crate::build_info::source_identity());
        assert_eq!(applied_by.2, crate::build_info::source_revision());
        assert_eq!(applied_by.3, env!("CARGO_PKG_VERSION"));
        // Receipts captured before prefix hashing stopped rereading the ledger.
        for (version, expected) in [
            (
                "0.11.017_migration_provenance",
                "bac372f031c01dc844a91f4327aa0a280c1194b2375b3fb89af0d2e20542736b",
            ),
            (
                "0.12.24.001_release",
                "20587abd87630443beb3874af02f01a8d4c37677859999ce542b7da301b1005e",
            ),
        ] {
            let parent: String = conn
                .query_row(
                    "SELECT parent_history FROM schema_migrations WHERE version = ?1",
                    [version],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(parent, expected, "{version}");
        }
    }

    #[test]
    fn current_database_does_not_backfill_missing_checksums_on_open() {
        let conn = open();
        apply_sqlite(&conn).unwrap();
        conn.execute(
            "UPDATE schema_migrations SET checksum = NULL WHERE version = '0.10.001_initial'",
            [],
        )
        .unwrap();
        let changes = conn.total_changes();

        apply_sqlite(&conn).unwrap();

        assert_eq!(conn.total_changes(), changes);
        let checksum: Option<String> = conn
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version = '0.10.001_initial'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(checksum, None);
    }

    #[test]
    fn failed_upgrade_preserves_missing_and_mismatched_checksums() {
        let conn = open();
        apply_through(&conn, "migration_provenance");
        let applied = applied_versions(&conn).unwrap();
        conn.execute_batch(
            "UPDATE schema_migrations SET checksum = NULL WHERE version = '0.10.001_initial';
             UPDATE schema_migrations SET checksum = 'wrong' WHERE version = '0.11.017_migration_provenance';"
        ).unwrap();

        let error = apply_sqlite(&conn).unwrap_err();

        assert!(error.to_string().contains("checksum does not match"));
        assert_eq!(applied_versions(&conn).unwrap(), applied);
        let checksums = conn.prepare(
            "SELECT checksum FROM schema_migrations WHERE version IN ('0.10.001_initial', '0.11.017_migration_provenance') ORDER BY version",
        ).unwrap().query_map([], |row| row.get::<_, Option<String>>(0)).unwrap()
            .collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(checksums, [None, Some("wrong".to_string())]);
    }

    #[test]
    fn validation_rejects_a_recorded_checksum_mismatch() {
        let conn = open();
        apply_sqlite(&conn).unwrap();
        conn.execute(
            "UPDATE schema_migrations SET checksum = 'wrong'
             WHERE version = '0.10.001_initial'",
            [],
        )
        .unwrap();

        let error = validate_sqlite(&conn).unwrap_err();

        assert!(error.to_string().contains("checksum does not match"));
    }

    #[test]
    fn released_0_12_4_frontier_is_reconstructible() {
        let migration = MIGRATIONS
            .iter()
            .find(|migration| migration.version() == "0.12.4.001_release")
            .expect("the production frontier remains in source history");

        assert_eq!(
            migration_checksum(migration),
            "6aa0076adfd1f115c8a473fd0403ede22d97d59d03a14bf234fad8170669a999"
        );
    }

    #[test]
    fn every_known_divergent_prefix_converges_without_losing_rows() {
        for count in 1..=DIVERGENT_MIGRATIONS.len() {
            let conn = open();
            conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
            apply_divergent_history(&conn, count);
            conn.execute(
                "INSERT INTO waves (id, name, repo, created_at) VALUES (?1, ?2, '/repo', 1)",
                (format!("wave-{count}"), format!("Wave {count}")),
            )
            .unwrap();

            apply_sqlite(&conn).unwrap();

            assert_eq!(
                latest_version_sqlite(&conn)
                    .unwrap_or_else(|error| panic!("divergent prefix {count}: {error}")),
                latest_known_version()
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM waves", [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                applied_versions(&conn).unwrap(),
                MIGRATIONS
                    .iter()
                    .map(Migration::version)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn live_permuted_history_converges_without_losing_rows() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_permuted_history(&conn);
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at) VALUES ('wave-live', 'Live', '/repo', 1)",
            [],
        )
        .unwrap();

        apply_sqlite_with_backup(&conn, &path).unwrap();

        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            latest_known_version()
        );
        assert_eq!(
            applied_versions(&conn).unwrap(),
            MIGRATIONS
                .iter()
                .map(Migration::version)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            conn.query_row("SELECT name FROM waves WHERE id = 'wave-live'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "Live"
        );
        let backup = rusqlite::Connection::open(find_backup(
            directory.path(),
            "loopflow.db.backup-0.11.012_context_input_normalization-",
        ))
        .unwrap();
        assert_eq!(
            applied_versions(&backup).unwrap()[10..],
            [
                "0.11.009_profiles",
                "0.11.010_provider_account_lifecycle",
                "0.11.011_context_pressure",
                "0.11.012_context_input_normalization",
            ]
        );
    }

    #[test]
    fn embedded_canonical_reference_matches_runtime_and_detects_drift() {
        let conn = open();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
        ).unwrap();
        for migration in MIGRATIONS {
            conn.execute_batch(migration.sql).unwrap();
        }
        validate_schema(&conn, MIGRATIONS).unwrap();

        let index: String = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute_batch(&format!("DROP INDEX \"{}\"", index.replace('"', "\"\"")))
            .unwrap();
        assert!(validate_schema(&conn, MIGRATIONS).is_err());
    }

    #[test]
    fn warmed_schema_reference_still_detects_live_index_drift() {
        let migration = Migration {
            sql: "CREATE TABLE example (id TEXT PRIMARY KEY, value TEXT NOT NULL);
                  CREATE INDEX idx_example_value ON example(value);",
            ..MIGRATIONS[0]
        };
        let conn = open();
        conn.execute_batch(migration.sql).unwrap();
        validate_schema(&conn, &[migration]).unwrap();

        conn.execute_batch("DROP INDEX idx_example_value;").unwrap();
        assert!(validate_schema(&conn, &[migration]).is_err());

        let intact = open();
        intact.execute_batch(migration.sql).unwrap();
        validate_schema(&intact, &[migration]).unwrap();
    }

    #[test]
    fn cached_schema_is_reread_after_ddl_from_another_connection() {
        let migration = Migration {
            sql: "CREATE TABLE example (id TEXT PRIMARY KEY, value TEXT NOT NULL);
                  CREATE INDEX idx_example_value ON example(value);",
            ..MIGRATIONS[0]
        };
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let first = rusqlite::Connection::open(&path).unwrap();
        first.execute_batch(migration.sql).unwrap();
        validate_schema(&first, &[migration]).unwrap();

        let second = rusqlite::Connection::open(&path).unwrap();
        second
            .execute_batch("DROP INDEX idx_example_value;")
            .unwrap();
        assert!(validate_schema(&first, &[migration]).is_err());

        let third = rusqlite::Connection::open(&path).unwrap();
        third
            .execute_batch("CREATE INDEX idx_example_value ON example(value);")
            .unwrap();
        validate_schema(&first, &[migration]).unwrap();
        third
            .execute_batch(
                "BEGIN IMMEDIATE;
                 CREATE INDEX idx_example_extra ON example(id, value);",
            )
            .unwrap();
        assert!(validate_schema(&third, &[migration]).is_err());
        third.execute_batch("ROLLBACK;").unwrap();
        validate_schema(&second, &[migration]).unwrap();
        validate_schema(&third, &[migration]).unwrap();
    }

    #[test]
    fn schema_reference_distinguishes_sql_under_the_same_migration_identity() {
        let required = Migration {
            sql: "CREATE TABLE example (id TEXT PRIMARY KEY, value TEXT NOT NULL);",
            ..MIGRATIONS[0]
        };
        let nullable = Migration {
            sql: "CREATE TABLE example (id TEXT PRIMARY KEY, value TEXT);",
            ..required
        };
        let conn = open();
        conn.execute_batch(required.sql).unwrap();
        validate_schema(&conn, &[required]).unwrap();
        assert!(validate_schema(&conn, &[nullable]).is_err());

        let other = open();
        other.execute_batch(nullable.sql).unwrap();
        validate_schema(&other, &[nullable]).unwrap();
        assert!(validate_schema(&other, &[required]).is_err());
    }

    #[test]
    fn product_schema_detects_constraint_and_index_drift() {
        let expected = open();
        expected
            .execute_batch(
                "CREATE TABLE example (id TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE INDEX idx_example_value ON example(value);",
            )
            .unwrap();
        let drifted = open();
        drifted
            .execute_batch("CREATE TABLE example (id TEXT PRIMARY KEY, value TEXT);")
            .unwrap();

        assert_ne!(
            product_schema(&expected).unwrap(),
            product_schema(&drifted).unwrap()
        );
    }

    #[test]
    fn provider_account_identity_preserves_existing_labels_and_health() {
        let conn = open();
        apply_before_migration(&conn, "provider_account_identity");
        conn.execute_batch("INSERT INTO provider_accounts (provider, account_id, home, login_email, credential_state, routing_state, plan, cooldown_until, created_at, updated_at)
            VALUES ('codex', 'engineering', '/fixture/account', 'eng@example.com', 'connected', 'automatic', 'pro', 123, 1, 2);").unwrap();
        conn.execute_batch(&migration_section_sql("provider_account_identity"))
            .unwrap();
        let row = conn.query_row("SELECT login_email, cooldown_until, observed_email, observed_subject, observed_plan FROM provider_accounts WHERE account_id = 'engineering'", [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, Option<String>>(4)?))).unwrap();
        assert_eq!(row, ("eng@example.com".into(), 123, None, None, None));
    }

    #[test]
    fn provider_account_credential_identity_retains_unbound_historical_observations() {
        let conn = open();
        apply_before_migration(&conn, "provider_account_credential_identity");
        conn.execute_batch("INSERT INTO provider_accounts (provider, account_id, home, login_email, credential_state, routing_state, observed_email, observed_subject, cooldown_until, created_at, updated_at)
            VALUES ('claude', 'engineering', '/fixture/account', 'eng@example.com', 'connected', 'explicit_only', 'eng@example.com', 'user-one', 123, 1, 2);").unwrap();
        conn.execute_batch(&migration_section_sql(
            "provider_account_credential_identity",
        ))
        .unwrap();
        let row = conn.query_row("SELECT observed_email, observed_subject, observed_credential_digest, cooldown_until, routing_state FROM provider_accounts WHERE account_id = 'engineering'", [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, i64>(3)?, row.get::<_, String>(4)?))).unwrap();
        assert_eq!(
            row,
            (
                "eng@example.com".into(),
                "user-one".into(),
                None,
                123,
                "explicit_only".into()
            )
        );
    }

    #[test]
    fn auth_browser_bindings_preserve_populated_profiles_accounts_and_routes() {
        let conn = open();
        apply_before_migration(&conn, "auth_browser_bindings");
        conn.execute_batch("INSERT INTO provider_accounts (provider, account_id, home, login_email, credential_state, routing_state, created_at, updated_at)
            VALUES ('codex', 'primary', '/account', 'person@example.com', 'connected', 'explicit_only', 1, 2);
            INSERT INTO access_profiles VALUES ('Work', 'Profile 3', 'person@example.com', 1, 2), ('Personal', 'Default', 'other@example.com', 3, 4);
            INSERT INTO account_access_profiles VALUES ('codex', 'primary', 0, 'Work'), ('codex', 'primary', 1, 'Personal');
            INSERT INTO provider_routes VALUES ('default', '', 'codex', 0, 'primary', 1, 2);").unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&migration_section_sql("auth_browser_bindings"))
            .unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        validate_foreign_keys(&conn).unwrap();
        assert_eq!(conn.query_row("SELECT group_concat(profile_id, ',') FROM (SELECT profile_id FROM auth_browser_bindings WHERE account_id = 'primary' ORDER BY position)", [], |row| row.get::<_, String>(0)).unwrap(), "Work,Personal");
        assert_eq!(
            conn.query_row(
                "SELECT expected_login FROM access_profiles WHERE profile_id = 'Work'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "person@example.com"
        );
        assert_eq!(
            conn.query_row("SELECT account_id FROM provider_routes", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
            "primary"
        );
        conn.execute_batch(
            "INSERT INTO access_profiles VALUES ('Unsigned', 'Profile 9', NULL, 5, 5);
            INSERT INTO auth_browser_bindings VALUES ('linear', NULL, 0, 'Work');",
        )
        .unwrap();
        assert!(conn
            .execute(
                "INSERT INTO auth_browser_bindings VALUES ('linear', NULL, 0, 'Personal')",
                []
            )
            .is_err());
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn accounts_first_migration_preserves_asymmetric_routes_venues_and_session_pins() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, prefix_before("accounts_first"), &mut Vec::new()).unwrap();
        conn.execute_batch(
            "INSERT INTO provider_accounts (
                provider, account_id, home, login_email, credential_state,
                routing_state, plan, paid_through, utilization_percent,
                cooldown_until, cooldown_reason, last_selected_at, created_at, updated_at
             ) VALUES
                ('claude', 'primary', '/accounts/claude/primary', 'jackstah@gmail.com',
                 'connected', 'automatic', 'max', NULL, 0, NULL, NULL, 5, 1, 5),
                ('claude', 'loopflow', '/accounts/claude/loopflow', 'jack@loopflow.studio',
                 'connected', 'automatic', 'max', NULL, 0, NULL, NULL, 6, 1, 6),
                ('codex', 'jackstah-1066', '/accounts/codex/jackstah-1066', 'jackstah@gmail.com',
                 'connected', 'automatic', 'plus', NULL, 0, NULL, NULL, 6, 1, 6),
                ('codex', 'engineering', '/accounts/codex/engineering', 'loopflow-eng@loopflow.studio',
                 'connected', 'automatic', 'team', NULL, 0, NULL, NULL, 5, 1, 5),
                ('codex', 'jack-42', '/accounts/codex/jack-42', 'jack@loopflow.studio',
                 'connected', 'automatic', 'plus', NULL, 0, NULL, NULL, 4, 1, 4),
                ('codex', 'manabot-eng', '/accounts/codex/manabot-eng', 'manabot-eng@loopflow.studio',
                 'connected', 'automatic', 'team', NULL, 80, NULL, NULL, NULL, 1, 1);

             INSERT INTO profiles (profile_id, created_at, updated_at) VALUES
                ('jackstah@gmail.com', 1, 1),
                ('loopflow-eng@loopflow.studio', 1, 2),
                ('jack@loopflow.studio', 1, 3),
                ('manabot-eng@loopflow.studio', 1, 4);
             INSERT INTO chrome_profile_bindings (
                profile_id, host_id, chrome_directory, created_at, updated_at
             ) VALUES
                ('jackstah@gmail.com', 'mini', 'Profile 3', 1, 1),
                ('loopflow-eng@loopflow.studio', 'mini', 'Profile 8', 1, 2),
                ('jack@loopflow.studio', 'mini', 'Default', 1, 3);
             INSERT INTO profile_provider_accounts (
                profile_id, provider, account_id, created_at, updated_at
             ) VALUES
                ('jackstah@gmail.com', 'claude', 'primary', 1, 1),
                ('loopflow-eng@loopflow.studio', 'claude', 'primary', 1, 2),
                ('jack@loopflow.studio', 'claude', 'loopflow', 1, 3),
                ('jackstah@gmail.com', 'codex', 'jackstah-1066', 1, 1),
                ('loopflow-eng@loopflow.studio', 'codex', 'engineering', 1, 2),
                ('jack@loopflow.studio', 'codex', 'jack-42', 1, 3),
                ('manabot-eng@loopflow.studio', 'codex', 'manabot-eng', 1, 4);
             INSERT INTO repo_profile_routes (
                repo_id, default_profile, created_at, updated_at
             ) VALUES ('loopflowstudio/loopflow', 'jackstah@gmail.com', 1, 4);
             INSERT INTO repo_backup_profiles (repo_id, position, profile_id) VALUES
                ('loopflowstudio/loopflow', 0, 'loopflow-eng@loopflow.studio'),
                ('loopflowstudio/loopflow', 1, 'jack@loopflow.studio');
             INSERT INTO provider_session_accounts (
                provider, provider_session_id, account_id, created_at, profile_id
             ) VALUES (
                'claude', 'session-1', 'primary', 7, 'loopflow-eng@loopflow.studio'
             );",
        )
        .unwrap();

        apply_through(&conn, "accounts_first");

        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM access_profiles", [], |row| row
                .get::<_, i64>(0),)
                .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row(
                "SELECT group_concat(profile_id, ',') FROM (
                    SELECT profile_id FROM account_access_profiles
                    WHERE provider = 'claude' AND account_id = 'primary'
                    ORDER BY position
                 )",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "jackstah@gmail.com,loopflow-eng@loopflow.studio"
        );
        assert_eq!(
            conn.query_row(
                "SELECT group_concat(account_id, ',') FROM (
                    SELECT account_id FROM provider_routes
                    WHERE scope = 'repo' AND scope_id = 'loopflowstudio/loopflow'
                      AND provider = 'claude'
                    ORDER BY position
                 )",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "primary,loopflow"
        );
        assert_eq!(
            conn.query_row(
                "SELECT group_concat(account_id, ',') FROM (
                    SELECT account_id FROM provider_routes
                    WHERE scope = 'repo' AND scope_id = 'loopflowstudio/loopflow'
                      AND provider = 'codex'
                    ORDER BY position
                 )",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "jackstah-1066,engineering,jack-42"
        );
        assert_eq!(
            conn.query_row(
                "SELECT group_concat(account_id, ',') FROM (
                    SELECT account_id FROM provider_routes
                    WHERE scope = 'default' AND provider = 'codex'
                    ORDER BY position
                 )",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "jackstah-1066,engineering,jack-42,manabot-eng"
        );
        assert_eq!(
            conn.query_row(
                "SELECT account_id FROM provider_session_accounts
                 WHERE provider = 'claude' AND provider_session_id = 'session-1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "primary"
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('provider_session_accounts')
                 WHERE name = 'profile_id'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'table' AND name IN (
                    'profiles', 'chrome_profile_bindings', 'profile_provider_accounts',
                    'repo_profile_routes', 'repo_backup_profiles'
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn project_successor_migration_allows_one_current_successor() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, &MIGRATIONS[..2], &mut Vec::new()).unwrap();
        conn.execute_batch(
            "INSERT INTO waves (id, name, repo, created_at)
                 VALUES ('w1', 'infrastructure', '/repo', 1);
             INSERT INTO project_sessions (
                 id, project_id, project_slug, project_name,
                 project_prompt_context, wave_id, pm_snapshot_synced_at,
                 status, status_reason, status_at, iteration,
                 observation_cursor, agent, provider, created_at, updated_at,
                 current_directive_version, incorporated_directive_version
             ) VALUES (
                 'ps_old', 'project-1', 'developer-efficiency',
                 'Developer Efficiency', 'Definition', 'w1', 1,
                 'abandoned', 'legacy Session ended', 2, 1, 0,
                 'codex', 'codex', 1, 2, 1, 1
             );",
        )
        .unwrap();

        apply_set(&conn, &MIGRATIONS[..4], &mut Vec::new()).unwrap();
        conn.execute_batch(
            "INSERT INTO project_sessions (
                 id, project_id, project_slug, project_name,
                 project_prompt_context, wave_id, pm_snapshot_synced_at,
                 status, status_reason, status_at, iteration,
                 observation_cursor, agent, provider, created_at, updated_at,
                 current_directive_version, incorporated_directive_version
             ) VALUES (
                 'ps_new', 'project-1', 'developer-efficiency',
                 'Developer Efficiency', 'Definition', 'w1', 3,
                 'created', 'successor', 3, 0, 0,
                 'codex', 'codex', 3, 3, 1, 1
             );",
        )
        .unwrap();
        assert!(conn
            .execute_batch(
                "INSERT INTO project_sessions (
                     id, project_id, project_slug, project_name,
                     project_prompt_context, wave_id, pm_snapshot_synced_at,
                     status, status_reason, status_at, iteration,
                     observation_cursor, agent, provider, created_at, updated_at,
                     current_directive_version, incorporated_directive_version
                 ) VALUES (
                     'ps_parallel', 'project-1', 'developer-efficiency',
                     'Developer Efficiency', 'Definition', 'w1', 3,
                     'created', 'parallel', 3, 0, 0,
                     'codex', 'codex', 3, 3, 1, 1
                 );"
            )
            .is_err());
        assert_eq!(
            conn.query_row("PRAGMA foreign_key_check", [], |row| row
                .get::<_, String>(0))
                .optional()
                .unwrap(),
            None
        );
    }

    /// The upgrade every shipped database takes: a pre-namespace `001_initial`
    /// stamp is adopted, and everything released after it runs on top.
    #[test]
    fn a_legacy_baseline_database_is_adopted_and_upgraded_without_data_loss() {
        let conn = open();
        apply_set(&conn, &[baseline()], &mut Vec::new()).unwrap();
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
            &mut Vec::new(),
        )
        .unwrap();

        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec!["0.10.001_initial", "add_note", "add_colour"]
        );
        let name: String = conn
            .query_row("SELECT name FROM waves WHERE id = 'w1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(name, "infra");
    }

    #[test]
    fn existing_task_rows_collapse_into_stable_work_and_prs() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, &MIGRATIONS[..2], &mut Vec::new()).unwrap();
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at)
             VALUES ('wave-1', 'runtime', '/repo', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_sessions (
                id, project_id, project_slug, project_name, project_prompt_context,
                wave_id, pm_snapshot_synced_at, status, status_reason, status_at,
                iteration, observation_cursor, agent, provider,
                process_generation, process_pid, process_tmux_name, process_started_at,
                created_at, updated_at,
                current_directive_version, incorporated_directive_version
             ) VALUES (
                'ps_legacy', 'project-1', 'runtime', 'Runtime', 'Definition',
                'wave-1', 9, 'running', 'active', 10, 1, 0, 'codex', 'codex',
                3, 33, 'project-legacy', 8, 10, 20, 1, 1
             )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_sessions (
                id, issue_id, issue_identifier, issue_title, issue_description,
                project_id, project_slug, project_name, project_prompt_context, wave_id,
                status, status_reason, status_at, worktree, branch, base_commit,
                agent, provider, process_generation, process_pid,
                process_tmux_name, process_started_at, pr_number, pr_url, created_at, updated_at,
                pm_snapshot_synced_at, pm_writeback_json, project_session_id,
                current_directive_version, incorporated_directive_version
             ) VALUES (
                'ts_legacy', 'issue-1', 'INF-123', 'Ship it', '',
                'project-1', 'runtime', 'Runtime', 'Definition', 'wave-1',
                'merged', 'pull request merged', 10, '/repo.inf-123',
                'jack/inf-123', 'base-sha', 'codex', 'codex', 7, 77,
                'task-legacy', 8, 101,
                'https://github.com/loopflowstudio/loopflow/pull/101', 10, 20,
                9, '{\"state\":\"current\"}', 'ps_legacy', 1, 1
             )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_events (session_id, kind_json, created_at)
             VALUES
                ('ts_legacy', '{\"kind\":\"started\"}', 11),
                ('ts_legacy', '{\"kind\":\"pull_request_opened\",\"number\":101,\"url\":\"https://github.com/loopflowstudio/loopflow/pull/101\"}', 12),
                ('ts_legacy', '{\"kind\":\"status_changed\",\"from\":\"submitted\",\"to\":\"merged\",\"reason\":\"merged\"}', 13)",
            [],
        )
        .unwrap();

        apply_sqlite(&conn).unwrap();

        apply_pending_migrations(&conn);

        let task: (String, String, String, String) = conn
            .query_row(
                "SELECT t.id, t.issue_identifier, t.workspace_slug, w.step
                 FROM tasks t JOIN task_graphs w ON w.task_id=t.id
                 WHERE t.external_issue_id='issue-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(task.1, "INF-123");
        assert_eq!(task.2, "inf-123");
        assert_eq!(task.3, "end");
        let pr: (
            i64,
            String,
            i64,
            Option<String>,
            i64,
            String,
            String,
            Option<i64>,
        ) = conn
            .query_row(
                "SELECT sequence, branch, publication_requested_at, after_merge,
                        github_number, github_url, merge_commit, abandoned_at
                 FROM task_prs WHERE task_id=?1",
                [&task.0],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            pr,
            (
                1,
                "jack/inf-123".to_string(),
                20,
                None,
                101,
                "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
                "legacy-unknown".to_string(),
                None,
            )
        );
        let events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id=?1",
                [&task.0],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 0);
    }

    #[test]
    fn planning_cutover_preserves_provider_fields_and_editable_project_content() {
        let conn = open();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        apply_before_migration(&conn, "local_planning");
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let project = &mut snapshot.projects[0];
        project.krs = vec![crate::work::project::ProjectKR {
            text: "Saved planning survives".into(),
            holds: true,
        }];
        let item = &mut snapshot.items[0];
        item.state = None;
        item.completed = true;
        item.completed_at = Some("2026-10-05T12:00:00Z".into());
        item.url = Some("https://linear.app/example".into());
        item.branch_name = Some("saved-branch".into());
        item.rank = 7;
        item.assignee = Some("person-id".into());
        conn.execute_batch("INSERT INTO waves(id,name,repo,created_at) VALUES('wave_fields','fields','/repo',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('project_fields','wave_fields','current',1);
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at)
                VALUES('task_fields','project_fields','LOO-318','LOO-318',1);").unwrap();
        let mut historical_project = serde_json::to_value(&project).unwrap();
        historical_project["workflow"] = serde_json::json!("");
        conn.execute("INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES('/repo','linear','current',17,?1)", [historical_project.to_string()]).unwrap();
        conn.execute("INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES('wave_fields','current',11)", []).unwrap();
        conn.execute("INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES('/repo','linear','LOO-318','LOO-318','current',17,?1)", [serde_json::to_string(item).unwrap()]).unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&migration_section_sql("local_planning"))
            .unwrap();
        validate_foreign_keys(&conn).unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM project_changes", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );

        assert_eq!(
            conn.query_row("SELECT count(*) FROM task_changes", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let order: String = conn
            .query_row(
                "SELECT task_order_json FROM pm_projects WHERE id='current'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&order).unwrap(),
            ["task_fields"]
        );
        let fields: serde_json::Value = conn.query_row("SELECT json_object('state',planning_state,'completed',planning_completed,
            'completed_at',planning_completed_at,'revision',planning_provider_revision,'url',planning_url,
            'branch_name',planning_branch_name,'team_id',planning_team_id,'assignee',planning_assignee,'rank',planning_rank)
            FROM tasks WHERE id='task_fields'", [], |row| row.get::<_, String>(0)).map(|json| serde_json::from_str(&json).unwrap()).unwrap();
        let expected = serde_json::to_value(item).unwrap();
        for field in [
            "state",
            "completed_at",
            "revision",
            "url",
            "branch_name",
            "team_id",
            "assignee",
            "rank",
        ] {
            assert_eq!(fields[field], expected[field], "{field}");
        }
        assert_eq!(fields["completed"], 1);
        assert_eq!(
            conn.query_row(
                "SELECT planning_rank FROM projects WHERE id='project_fields'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            11
        );
        let text: String = conn
            .query_row(
                "SELECT project_prompt_context FROM projects WHERE id='project_fields'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let content = crate::linear::project_document::parse_project_document(&text).unwrap();
        assert_eq!(content.krs, project.krs);
        assert_eq!(content.metric_targets, project.metric_targets);
        conn.execute("INSERT INTO task_changes(id,task_id,field,value_json) VALUES('membership','task_fields','project_id','\"project_fields\"')", []).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT task_id FROM task_changes WHERE id='membership'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "task_fields"
        );
    }

    #[test]
    fn local_planning_preserves_pending_decisions_without_workflow_history() {
        let conn = open();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        apply_before_migration(&conn, "local_planning");
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('wave_delivery','delivery','/repo',1);
             INSERT INTO projects(id,wave_id,external_project_id,created_at)
                 VALUES('project_delivery','wave_delivery','linear_project',1);
             INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,pm_writeback_json)
                 VALUES('task_complete','project_delivery','issue_complete','FIX-1',1,
                    '{\"state\":\"pending\",\"operation\":\"complete_task\",\"error\":\"lost completion reply\"}'),
                       ('task_reopen','project_delivery','issue_reopen','FIX-2',1,
                    '{\"state\":\"pending\",\"operation\":\"reopen_task\",\"error\":\"lost reopening reply\"}');",
        ).unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&migration_section_sql("local_planning"))
            .unwrap();
        validate_foreign_keys(&conn).unwrap();
        validate_persisted_json(&conn).unwrap();
        let mut query = conn.prepare(
            "SELECT task_id,target,attempted,settled,error,move_seq FROM task_state_deliveries ORDER BY task_id",
        ).unwrap();
        let receipts = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            receipts,
            vec![
                (
                    "task_complete".into(),
                    "completed".into(),
                    true,
                    false,
                    "lost completion reply".into(),
                    None
                ),
                (
                    "task_reopen".into(),
                    "unstarted".into(),
                    true,
                    false,
                    "lost reopening reply".into(),
                    None
                ),
            ]
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM task_workflow_moves", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM pragma_table_info('tasks') WHERE name='pm_writeback_json'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn legacy_persisted_json_upgrades_to_typed_stable_tasks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, &MIGRATIONS[..2], &mut Vec::new()).unwrap();
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at)
             VALUES ('00000000-0000-0000-0000-000000000001', 'runtime', '/repo', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO project_sessions (
                id, project_id, project_slug, project_name, project_prompt_context,
                wave_id, pm_snapshot_synced_at, status, status_reason, status_at,
                iteration, observation_cursor, agent, provider,
                created_at, updated_at,
                current_directive_version, incorporated_directive_version
             ) VALUES (
                'ps_reopen', 'project-reopen', 'runtime', 'Runtime', 'Definition',
                '00000000-0000-0000-0000-000000000001', 9, 'running', 'active', 10, 1, 0, 'codex', 'codex',
                10, 20, 1, 1
             )",
            [],
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO task_sessions (
                id, issue_id, issue_identifier, issue_title, issue_description,
                project_id, project_slug, project_name, project_prompt_context, wave_id,
                status, status_reason, status_at, worktree, branch, base_commit,
                agent, provider, created_at, updated_at,
                pm_snapshot_synced_at, pm_writeback_json, project_session_id,
                current_directive_version, incorporated_directive_version
             ) VALUES
             (
                'ts_completed', 'issue-completed', 'INF-DONE', 'Finish it', '',
                'project-reopen', 'runtime', 'Runtime', 'Definition',
                '00000000-0000-0000-0000-000000000001',
                'completed', 'done', 10, '/repo.inf-done',
                'jack/inf-done', 'base-sha', 'codex', 'codex', 10, 20,
                9, '{\"state\":\"current\"}', 'ps_reopen', 1, 1
             ),
             (
                'ts_reopen', 'issue-reopen', 'INF-REOPEN', 'Resume it', '',
                'project-reopen', 'runtime', 'Runtime', 'Definition',
                '00000000-0000-0000-0000-000000000001',
                'waiting', 'writeback failed', 10, '/repo.inf-reopen',
                'jack/inf-reopen', 'base-sha', 'codex', 'codex', 10, 20,
                9, '{\"state\":\"pending\",\"operation\":\"reopen_task\",\"error\":\"offline\"}',
                'ps_reopen', 1, 1
             ),
             (
                'ts_blocked', 'issue-blocked', 'INF-BLOCKED', 'Unblock it', '',
                'project-reopen', 'runtime', 'Runtime', 'Definition',
                '00000000-0000-0000-0000-000000000001',
                'blocked', 'dependency', 10, '/repo.inf-blocked',
                'jack/inf-blocked', 'base-sha', 'codex', 'codex', 10, 20,
                9, '{\"state\":\"current\"}', 'ps_reopen', 1, 1
             ),
             (
                'ts_failed', 'issue-failed', 'INF-FAILED', 'Recover it', '',
                'project-reopen', 'runtime', 'Runtime', 'Definition',
                '00000000-0000-0000-0000-000000000001',
                'failed', 'provider error', 10, '/repo.inf-failed',
                'jack/inf-failed', 'base-sha', 'codex', 'codex', 10, 20,
                9, '{\"state\":\"current\"}', 'ps_reopen', 1, 1
             ),
             (
                'ts_abandoned', 'issue-abandoned', 'INF-ABANDONED', 'Retire it', '',
                'project-reopen', 'runtime', 'Runtime', 'Definition',
                '00000000-0000-0000-0000-000000000001',
                'abandoned', 'superseded', 10, '/repo.inf-abandoned',
                'jack/inf-abandoned', 'base-sha', 'codex', 'codex', 10, 20,
                9, '{\"state\":\"current\"}', 'ps_reopen', 1, 1
             ),
             (
                'ts_current', 'issue-current', 'INF-CURRENT', 'Keep it', '',
                'project-reopen', 'runtime', 'Runtime', 'Definition',
                '00000000-0000-0000-0000-000000000001',
                'waiting', 'review', 10, '/repo.inf-current',
                'jack/inf-current', 'base-sha', 'codex', 'codex', 10, 20,
                9, '{\"state\":\"current\"}', 'ps_reopen', 1, 1
             );",
        )
        .unwrap();

        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        apply_set(&conn, prefix_before("interaction_reviews"), &mut Vec::new()).unwrap();
        conn.execute_batch(
            "UPDATE task_sessions
             SET gate_proposal_json = CASE id
                 WHEN 'ts_completed' THEN '{\"status\":\"completed\",\"reason\":\"all done\"}'
                 WHEN 'ts_reopen' THEN '{\"status\":\"waiting\",\"reason\":\"needs another pass\"}'
                 WHEN 'ts_blocked' THEN '{\"status\":\"blocked\",\"reason\":\"dependency\"}'
                 WHEN 'ts_failed' THEN '{\"status\":\"failed\",\"reason\":\"provider error\"}'
                 WHEN 'ts_abandoned' THEN '{\"status\":\"abandoned\",\"reason\":\"superseded\"}'
                 WHEN 'ts_current' THEN '{\"done\":false,\"reason\":\"current\",\"future\":\"preserved\"}'
             END
             WHERE id IN (
                 'ts_completed', 'ts_reopen', 'ts_blocked', 'ts_failed', 'ts_abandoned',
                 'ts_current'
             );",
        )
        .unwrap();

        // Prove the historical JSON repair before later releases retire its storage.
        let repair_index = section(GATE_PROPOSAL_REPAIR_NAME).index;
        apply_set(&conn, &MIGRATIONS[..=repair_index], &mut Vec::new()).unwrap();
        let current_gate: String = conn
            .query_row(
                "SELECT gate_proposal_json FROM tasks WHERE external_issue_id='issue-current'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            current_gate,
            "{\"done\":false,\"reason\":\"current\",\"future\":\"preserved\"}"
        );
        let stale_gate_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks
                 WHERE json_type(gate_proposal_json, '$.status') IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale_gate_count, 0);

        let error = super::_migration_transaction(&conn, |conn, _guards| {
            conn.execute(
                "UPDATE tasks
                 SET pm_writeback_json='{\"state\":\"pending\",\"operation\":\"invented\",\"error\":\"offline\"}'
                 WHERE external_issue_id='issue-reopen'",
                [],
            )?;
            validate_persisted_json(conn)
        })
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("tasks.pm_writeback_json"));
        assert_eq!(
            conn.query_row(
                "SELECT pm_writeback_json FROM tasks WHERE external_issue_id='issue-reopen'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "{\"state\":\"current\"}"
        );
        conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
        apply_set(&conn, MIGRATIONS, &mut Vec::new()).unwrap();
        conn.execute_batch("COMMIT").unwrap();
        validate_foreign_keys(&conn).unwrap();
        validate_persisted_json(&conn).unwrap();
        drop(conn);

        let store = crate::store::sqlite::SqliteStore::new(&path).unwrap();
        store.health_check().unwrap();
        let tasks = store.list_tasks(None).unwrap();
        assert_eq!(tasks.len(), 6);
        let reopen = tasks
            .iter()
            .find(|task| task.identifier == "INF-REOPEN")
            .unwrap();
        assert!(matches!(
            reopen.pm_writeback,
            crate::work::task::LinearWritebackState::Current
        ));
    }

    #[test]
    fn after_merge_review_rows_become_continue_task() {
        let conn = open();
        apply_before_draft(&conn, "after_merge_continue_task");
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute(
            "INSERT INTO task_prs (
                id, task_id, sequence, slug, branch, base_commit,
                publication_requested_at, after_merge, next_slug,
                github_number, github_url, merge_commit, abandoned_at,
                created_at, updated_at
             ) VALUES (
                'pr_legacy', 'task_legacy', 1, 'proof', 'jack/proof', 'base',
                10, 'review', 'follow-up', 17, 'https://example.test/pull/17',
                'merge', NULL, 1, 11
             )",
            [],
        )
        .unwrap();

        apply_draft(&conn, "after_merge_continue_task");

        let disposition: String = conn
            .query_row(
                "SELECT after_merge FROM task_prs WHERE id='pr_legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(disposition, "continue_task");
        assert!(conn
            .execute(
                "UPDATE task_prs SET after_merge='review' WHERE id='pr_legacy'",
                [],
            )
            .is_err());
    }

    #[test]
    fn historical_publications_gain_no_implicit_merge_request() {
        let conn = open();
        apply_before_draft(&conn, "explicit_pr_merge_requests");
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute(
            "INSERT INTO task_prs (
                id, task_id, sequence, slug, branch, base_commit,
                publication_requested_at, after_merge, next_slug,
                github_number, github_url, merge_commit, abandoned_at,
                created_at, updated_at, github_head_sha
             ) VALUES (
                'pr_published', 'task_legacy', 1, 'proof', 'jack/proof', 'base',
                10, 'continue_task', NULL, 17, 'https://example.test/pull/17',
                NULL, NULL, 1, 11, 'head-17'
             )",
            [],
        )
        .unwrap();

        apply_draft(&conn, "explicit_pr_merge_requests");

        let merge: (Option<String>, Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT merge_mode, merge_requested_at, merge_head_sha
                 FROM task_prs WHERE id='pr_published'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(merge, (None, None, None));
        let disposition: (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT after_merge, next_slug
                 FROM task_prs WHERE id='pr_published'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(disposition, (None, None));
        conn.execute(
            "UPDATE task_prs
             SET merge_mode='user', merge_requested_at=12, merge_head_sha='head-17',
                 after_merge='continue_task'
             WHERE id='pr_published'",
            [],
        )
        .unwrap();
        assert!(conn
            .execute(
                "UPDATE task_prs SET merge_head_sha='later-head' WHERE id='pr_published'",
                [],
            )
            .is_err());
    }

    #[test]
    fn several_migrations_in_one_minor_release_apply_in_order() {
        let conn = open();
        apply_set(&conn, &[baseline(), SECOND_IN_SAME_MINOR], &mut Vec::new()).unwrap();

        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec!["0.10.001_initial", "add_note"]
        );
        assert!(columns(&conn, "waves").contains(&"note".to_string()));
    }

    /// The release boundary: a database at the end of 0.10 takes only the 0.11
    /// tail, and takes it exactly once.
    #[test]
    fn a_database_from_the_previous_minor_applies_only_the_new_namespace() {
        let conn = open();
        apply_set(&conn, &[baseline(), SECOND_IN_SAME_MINOR], &mut Vec::new()).unwrap();

        let next = [baseline(), SECOND_IN_SAME_MINOR, FIRST_IN_NEXT_MINOR];
        apply_set(&conn, &next, &mut Vec::new()).unwrap();
        apply_set(&conn, &next, &mut Vec::new()).unwrap();

        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec!["0.10.001_initial", "add_note", "add_colour"]
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
    fn an_unknown_migration_reports_both_database_and_binary_evidence() {
        let conn = open();
        apply_set(&conn, &[baseline(), FIRST_IN_NEXT_MINOR], &mut Vec::new()).unwrap();

        let error = apply_set(&conn, &[baseline()], &mut Vec::new()).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("add_colour"), "{message}");
        assert!(
            message.contains("latest known 0.10.001_initial"),
            "{message}"
        );
        assert!(message.contains("lf self install --main"), "{message}");
    }

    fn assert_keeps_the_store(error: &crate::store::StoreError) {
        let message = error.to_string();
        assert!(!message.contains("delete"), "{message}");
        assert!(message.contains("keep loopflow.db"), "{message}");
        assert!(message.contains("lf self install --main"), "{message}");
    }

    /// The pre-loop store's flat ledger (`001_initial`, `002_...`, …) was abandoned
    /// when the runtime collapsed to waves, projects, and tasks. No build upgrades
    /// it, but its data is still the owner's: the build that wrote it opens it.
    #[test]
    fn a_database_from_the_abandoned_flat_ledger_keeps_the_store_and_names_the_newest_build() {
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
        assert_keeps_the_store(&error);
    }

    #[test]
    fn a_stale_edit_of_a_shipped_migration_keeps_the_store_and_names_the_newest_build() {
        let conn = open();
        apply_sqlite(&conn).unwrap();
        conn.execute_batch("DROP INDEX idx_tasks_project").unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert_keeps_the_store(&error);
    }

    #[test]
    fn a_skipped_migration_keeps_the_store_and_names_the_newest_build() {
        let conn = open();
        apply_set(&conn, &[baseline()], &mut Vec::new()).unwrap();
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES ('add_colour', 2)",
            [],
        )
        .unwrap();

        let error = apply_set(
            &conn,
            &[baseline(), SECOND_IN_SAME_MINOR, FIRST_IN_NEXT_MINOR],
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_keeps_the_store(&error);
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
            latest_known_version()
        );
    }

    /// A durable snapshot of a real store at the 0.10 release, frozen once and
    /// committed — deliberately *not* derived from the current MIGRATIONS
    /// registry. A prefix of MIGRATIONS regenerates itself from the same source
    /// it validates against, so a rewritten early migration would slip past it;
    /// this frozen fixture diverges and fails instead.
    const PREVIOUS_RELEASE_FIXTURE: &str = include_str!("tests/fixtures/store_0_10_release.sql");

    /// A real two-generation upgrade: a database frozen at the *previous release*
    /// (the committed fixture, independent of MIGRATIONS) takes the current
    /// canonical tail exactly once, reaches the latest known version, and carries
    /// its live rows and referential integrity across every rebuild in the tail.
    #[test]
    fn a_previous_release_database_upgrades_through_the_current_canonical_tail() {
        let conn = open();
        conn.execute_batch(PREVIOUS_RELEASE_FIXTURE).unwrap();

        // The fixture starts at the 0.10 generation with live, self-referential
        // data — a two-generation upgrade, not a from-scratch run.
        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![
                "0.10.001_initial".to_string(),
                "0.10.002_session_execution_context".to_string(),
            ]
        );
        assert!(MIGRATIONS.len() > 2, "need a tail beyond the fixture");

        // The generated tail advances it through the production transaction,
        // and applying it again is a no-op.
        apply_sqlite(&conn).unwrap();
        apply_sqlite(&conn).unwrap();

        assert_eq!(applied_versions(&conn).unwrap().len(), MIGRATIONS.len());
        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            latest_known_version()
        );
        validate_foreign_keys(&conn).unwrap();

        // Rows survive; directory parentage supersedes the old promotion link
        // once the subwave migration is part of the canonical release tail.
        let waves: i64 = conn
            .query_row("SELECT count(*) FROM waves", [], |row| row.get(0))
            .unwrap();
        assert_eq!(waves, 2, "seeded waves did not survive the upgrade");
        let child_parent: Option<String> = conn
            .query_row(
                "SELECT parent_wave_id FROM waves WHERE id = 'wave-child'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(child_parent, None);
        let tokens: i64 = conn
            .query_row("SELECT count(*) FROM provider_tokens", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            tokens, 1,
            "seeded provider token did not survive the upgrade"
        );
    }

    /// Fresh initialization over the full real registry — a brand-new database
    /// runs the entire canonical tail from empty to the latest known version.
    #[test]
    fn a_fresh_database_initializes_through_the_full_canonical_tail() {
        let conn = open();
        apply_set(&conn, MIGRATIONS, &mut Vec::new()).unwrap();

        assert_eq!(applied_versions(&conn).unwrap().len(), MIGRATIONS.len());
        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            latest_known_version()
        );
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn divergent_repair_publishes_the_unmodified_previous_generation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_divergent_history(&conn, DIVERGENT_MIGRATIONS.len());

        apply_sqlite_with_backup(&conn, &path).unwrap();

        let backup_path = find_backup(
            directory.path(),
            "loopflow.db.backup-0.11.011_provider_account_lifecycle-",
        );
        let backup = rusqlite::Connection::open(backup_path).unwrap();
        assert_eq!(
            latest_applied_version_sqlite(&backup).unwrap().as_deref(),
            Some("0.11.011_provider_account_lifecycle")
        );
        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            latest_known_version()
        );
    }

    #[test]
    fn release_migration_publishes_a_complete_previous_generation_backup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_set(&conn, &MIGRATIONS[..1], &mut Vec::new()).unwrap();

        apply_sqlite_with_backup(&conn, &path).unwrap();

        let backup_path = find_backup(directory.path(), "loopflow.db.backup-0.10.001_initial-");
        let backup = rusqlite::Connection::open(backup_path).unwrap();
        assert_eq!(
            latest_applied_version_sqlite(&backup).unwrap().as_deref(),
            Some("0.10.001_initial")
        );
        assert!(!columns(&backup, "task_sessions").contains(&"lf_bin".to_string()));
    }

    #[test]
    fn a_committed_migration_keeps_only_the_backup_it_made() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_set(&conn, &MIGRATIONS[..1], &mut Vec::new()).unwrap();
        let old = |name: &str, age_secs: u64| {
            let file = directory.path().join(name);
            std::fs::write(&file, b"generation").unwrap();
            let modified = std::time::SystemTime::now() - Duration::from_secs(age_secs);
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(modified)
                .unwrap();
            file
        };
        let oldest = old("loopflow.db.backup-0.9.001_a-0123456789abcdef", 300);
        let sidecar = old("loopflow.db.backup-0.9.001_a-0123456789abcdef-shm", 300);
        let previous = old("loopflow.db.backup-0.9.002_b-fedcba9876543210", 200);
        let by_hand = old("loopflow.db.backup-before-abandon-20260930", 400);
        let unfingerprinted = old("loopflow.db.backup-0.11.003_child_body_lease", 400);

        apply_sqlite_with_backup(&conn, &path).unwrap();

        find_backup(directory.path(), "loopflow.db.backup-0.10.001_initial-");
        assert!(!previous.exists());
        assert!(!oldest.exists());
        assert!(!sidecar.exists());
        assert!(by_hand.exists());
        assert!(unfingerprinted.exists());

        prune_migration_backups(&path, 0);
        let left: Vec<_> = std::fs::read_dir(directory.path())
            .unwrap()
            .filter_map(|entry| entry.unwrap().file_name().into_string().ok())
            .filter(|name| name.contains(".backup-0.10.001_initial-"))
            .collect();
        assert!(left.is_empty(), "settled backup remains: {left:?}");
        assert!(by_hand.exists());
    }

    #[test]
    fn backup_snapshot_and_migration_share_one_exclusive_transaction() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_set(&conn, &MIGRATIONS[..1], &mut Vec::new()).unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL").unwrap();
        let writer_path = path.clone();
        let (start_writer, writer_start) = sync_channel(0);
        let (first_attempt, observe_first_attempt) = sync_channel(1);
        let (retry_writer, writer_retry) = sync_channel(0);
        let (writer_done, observe_writer) = sync_channel(1);
        let writer = std::thread::spawn(move || {
            writer_start.recv().unwrap();
            let writer = rusqlite::Connection::open(writer_path).unwrap();
            writer.busy_timeout(Duration::ZERO).unwrap();
            let blocked = writer
                .execute(
                    "INSERT INTO blob_tokens (sha, lines, bytes, tokens)
                     VALUES ('writer-probe', 1, 2, 3)",
                    [],
                )
                .is_err_and(|error| {
                    matches!(
                        error.sqlite_error_code(),
                        Some(
                            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                        )
                    )
                });
            first_attempt.send(blocked).unwrap();
            writer_retry.recv().unwrap();
            writer.busy_timeout(Duration::from_secs(5)).unwrap();
            writer
                .execute(
                    "INSERT INTO blob_tokens (sha, lines, bytes, tokens)
                     VALUES ('writer-probe', 1, 2, 3)",
                    [],
                )
                .unwrap();
            writer_done
                .send(latest_version_sqlite(&writer).unwrap())
                .unwrap();
        });

        apply_sqlite_transaction(&conn, |conn| {
            backup_before_migration(conn, &path)?;
            start_writer.send(()).unwrap();
            assert!(
                observe_first_attempt
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap(),
                "competing writer committed between backup and migration"
            );
            Ok(())
        })
        .unwrap();

        retry_writer.send(()).unwrap();
        assert_eq!(observe_writer.recv().unwrap(), latest_known_version());
        writer.join().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT sha FROM blob_tokens WHERE sha = 'writer-probe'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "writer-probe"
        );
        let backup = rusqlite::Connection::open(find_backup(
            directory.path(),
            "loopflow.db.backup-0.10.001_initial-",
        ))
        .unwrap();
        assert_eq!(
            backup
                .query_row("SELECT count(*) FROM blob_tokens", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn current_schema_does_not_take_the_database_write_lock() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let current = rusqlite::Connection::open(&path).unwrap();
        apply_sqlite(&current).unwrap();
        current.execute_batch("PRAGMA journal_mode = WAL").unwrap();

        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        current.busy_timeout(Duration::ZERO).unwrap();

        apply_sqlite_with_backup(&current, &path).unwrap();

        writer.execute_batch("ROLLBACK").unwrap();
    }

    /// An interrupted first initialization can leave a header without a schema.
    #[test]
    fn an_existing_schema_less_database_still_gets_its_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        }
        assert!(
            std::fs::metadata(&path).unwrap().len() > 0,
            "this is not the case under test unless the file has bytes already"
        );

        let store = crate::store::sqlite::SqliteStore::new(&path).unwrap();

        // A bare `Ok` is not the proof: the regression opened fine and failed
        // on the first read of a table it never created.
        assert!(store.processes_since(0).unwrap().is_empty());
        let conn = rusqlite::Connection::open(&path).unwrap();
        validate_sqlite_schema(&conn).unwrap();
        assert_eq!(
            applied_versions(&conn).unwrap().last(),
            Some(&latest_known_version())
        );
    }

    #[test]
    fn unmarked_existing_schema_is_never_adopted() {
        let conn = open();
        conn.execute_batch("CREATE TABLE waves (id TEXT PRIMARY KEY)")
            .unwrap();

        let error = apply_sqlite(&conn).unwrap_err();
        assert_keeps_the_store(&error);
    }

    #[test]
    fn a_repeated_id_is_rejected_before_anything_runs() {
        let error = validate_set(&[baseline(), baseline()]).unwrap_err();
        assert!(error.contains("appears twice"));
    }

    #[test]
    fn legacy_and_release_scoped_versions_parse() {
        assert_eq!(
            MigrationId::parse_version("0.10.001_initial"),
            Some(MigrationId {
                major: 0,
                minor: 10,
                patch: None,
                ordinal: 1,
            })
        );
        assert_eq!(MigrationId::parse_version("001_initial"), None);
        assert_eq!(
            MigrationId::parse_version("0.12.2.001_release"),
            Some(MigrationId {
                major: 0,
                minor: 12,
                patch: Some(2),
                ordinal: 1,
            })
        );
        assert_eq!(MigrationId::parse_version("0.10.1.2.3_initial"), None);
        assert_eq!(MigrationId::parse_version("0.10.001"), None);
    }

    #[test]
    fn task_feedback_reviewers_rename_columns_and_map_authority() {
        let conn = open();
        apply_before_draft(&conn, "task_feedback_reviewers");
        conn.execute_batch(
            "INSERT INTO waves (id, name, repo, created_at)
                 VALUES ('wave_test', 'test', '/repo', 100);
             INSERT INTO projects (id, wave_id, external_project_id, created_at)
                 VALUES ('proj_test', 'wave_test', 'ext_proj', 100);
             INSERT INTO tasks (
                 id, project_id, external_issue_id, issue_identifier, created_at,
                 iterate_interaction_policy, kickoff_interaction_policy,
                 gate_interaction_policy
             ) VALUES
                 ('task_mixed', 'proj_test', 'ext_a', 'W2-1', 100,
                  'defer', 'require', 'require'),
                 ('task_parent', 'proj_test', 'ext_b', 'W2-2', 100,
                  'defer', 'defer', 'defer');",
        )
        .unwrap();

        apply_draft(&conn, "task_feedback_reviewers");

        let task_columns = columns(&conn, "tasks");
        assert!(task_columns.contains(&"iterate_reviewer".to_string()));
        assert!(task_columns.contains(&"kickoff_reviewer".to_string()));
        assert!(task_columns.contains(&"gate_reviewer".to_string()));
        assert!(!task_columns
            .iter()
            .any(|column| column.contains("interaction_policy")));

        let mixed: (String, String, String) = conn
            .query_row(
                "SELECT kickoff_reviewer, iterate_reviewer, gate_reviewer
                 FROM tasks WHERE id='task_mixed'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(mixed, ("user".into(), "parent".into(), "user".into()));
        let parent: (String, String, String) = conn
            .query_row(
                "SELECT kickoff_reviewer, iterate_reviewer, gate_reviewer
                 FROM tasks WHERE id='task_parent'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(parent, ("parent".into(), "parent".into(), "parent".into()));
    }

    #[test]
    fn ownership_cutover_keeps_current_task_review_without_importing_history() {
        let conn = open();
        apply_before_migration(&conn, "session_ownership");
        conn.execute_batch(r#"
            INSERT INTO waves(id,name,repo,created_at) VALUES('w','infra','/repo',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES('p','w','linear-p',1);
            INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,worktree)
              VALUES('t','p','linear-t','LOO-1',1,'/repo/task');
            INSERT INTO task_events(task_id,kind_json,created_at) VALUES('t','{"kind":"started"}',17);
            INSERT INTO task_flow_positions(task_id,invocation_json,flow,step,node_id,human,
              session_run_id,ready_summary,step_index,iteration,position_version,worker_generation,
              claim_json,updated_at,review_json)
            VALUES('t','{"id":"saved","flow":"custom","steps":[{"Skill":{"skill":{"name":"review","content":"Saved instructions"},"policy":{"id":"review","human":true},"flow_parents":["custom"]}}]}',
              'custom','review','review',1,'run_11111111111111111111111111111111','Keep this feedback',
              0,2,7,3,NULL,20,'{"index":0,"iteration":2}');
        "#).unwrap();
        conn.execute_batch(&migration_section_sql("session_ownership"))
            .unwrap();
        let (plan, feedback, started, worktree, selected): (String, String, i64, String, String) =
            conn.query_row(
                "SELECT f.invocation_json,a.ready_summary,t.started_at,t.worktree,c.receipt_key
             FROM tasks t JOIN flow_sessions f ON f.id=t.current_invocation_id
             JOIN agent_sessions a ON a.id=f.pending_session_id
             JOIN session_events c ON c.seq=a.current_capture",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        let plan: serde_json::Value = serde_json::from_str(&plan).unwrap();
        assert_eq!(plan["id"], "saved");
        let step = &plan["steps"][0]["Skill"];
        assert_eq!(step["human"], true);
        assert_eq!(step["skill"]["content"], "Saved instructions");
        assert_eq!(step["sources"], serde_json::json!(["custom"]));
        assert_eq!(feedback, "Keep this feedback");
        assert_eq!(started, 17);
        assert_eq!(worktree, "/repo/task");
        assert_eq!(selected, "run_11111111111111111111111111111111");
        assert_eq!(
            conn.query_row("SELECT count(*) FROM execs", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        for table in [
            "runs",
            "sessions",
            "flow_invocations",
            "import_evidence",
            "run_events",
            "task_flow_positions",
        ] {
            assert!(!has_table(&conn, table).unwrap(), "{table}");
        }
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn project_status_adoption_preserves_current_identity_and_custom_flow() {
        let conn = open();
        apply_before_migration(&conn, "project_status_chapters");
        conn.execute_batch(
            r#"INSERT INTO waves(id,name,repo,created_at) VALUES('w','infra','/repo',1);
            INSERT INTO projects(id,wave_id,external_project_id,created_at,updated_at,
                project_slug,project_name,project_prompt_context,pm_snapshot_synced_at,iteration)
            VALUES('retained','w','old',1,2,'old','Old','Retain context',2,7);
            INSERT INTO wave_chapters(wave_id,chapter_id,project_id,current,receipt) VALUES
                ('w','old','old',1,'{"phase":"complete","created_at":1}'),
                ('w','next','next',0,'{"phase":"preparing","created_at":2}');
            INSERT INTO pm_snapshots(wave_id,provider,initiative,synced_at,payload)
            VALUES('w','linear','initiative',3,'{"projects":[
                {"id":"old","slug":"old","name":"Old","summary":"","metric_targets":[],
                 "flows":{"recommended":"custom"},"krs":[],"initiative_ids":["initiative"],"team_ids":["team"]},
                {"id":"next","slug":"next","name":"Next","summary":"","metric_targets":[],
                 "flows":{"recommended":"future-custom"},"krs":[],"initiative_ids":["initiative"],"team_ids":["team"]}
            ],"items":[]}');"#,
        ).unwrap();
        conn.execute_batch(&migration_section_sql("project_status_chapters"))
            .unwrap();
        let preserved: (String, String, i64, i64, i64, String, i64) = conn.query_row(
            "SELECT id,project_prompt_context,created_at,updated_at,iteration,flow,legacy_current FROM projects WHERE external_project_id='old'",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
        ).unwrap();
        assert_eq!(
            preserved,
            (
                "retained".into(),
                "Retain context".into(),
                1,
                2,
                7,
                "custom".into(),
                1
            )
        );
        let future: (String, i64) = conn
            .query_row(
                "SELECT flow,legacy_current FROM projects WHERE external_project_id='next'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(future, ("future-custom".into(), 0));
        let payload: String = conn
            .query_row(
                "SELECT payload FROM pm_snapshots WHERE wave_id='w'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // This release's shape; the Project key is renamed by a later one.
        let snapshot: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(snapshot["projects"][0]["id"], "old");
        assert_eq!(snapshot["projects"][0]["flow"], "custom");
        assert_eq!(snapshot["projects"][0]["status"], "started");
        assert_eq!(snapshot["projects"][1]["status"], "planned");
        assert!(!has_table(&conn, "wave_chapters").unwrap());
        validate_foreign_keys(&conn).unwrap();
    }

    #[test]
    fn dropping_project_fingerprint_preserves_planning_and_progress() {
        let conn = open();
        apply_before_migration(&conn, "drop_project_fingerprint");
        conn.execute_batch(
            "INSERT INTO waves (id, name, repo, created_at)
             VALUES ('wave_project', 'project', '/repo', 100);
             INSERT INTO projects (
                 id, wave_id, external_project_id, created_at, updated_at,
                 project_prompt_context, iteration, last_state_fingerprint
             ) VALUES (
                 'proj_progress', 'wave_project', 'linear-project', 100, 200,
                 'Current definition and KRs', 7, 'retired-controller-evidence'
             );",
        )
        .unwrap();

        conn.execute_batch(&migration_section_sql("drop_project_fingerprint"))
            .unwrap();

        assert!(!columns(&conn, "projects").contains(&"last_state_fingerprint".to_string()));
        let retained: (String, String, i64, i64, i64) = conn
            .query_row(
                "SELECT wave_id, project_prompt_context, iteration, created_at, updated_at
                 FROM projects WHERE id='proj_progress' AND external_project_id='linear-project'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            retained,
            (
                "wave_project".into(),
                "Current definition and KRs".into(),
                7,
                100,
                200
            )
        );
    }

    #[test]
    fn task_loop_review_preserves_existing_human_and_claimed_positions() {
        let conn = open();
        apply_before_migration(&conn, "task_loop_review");
        conn.execute_batch(
            "INSERT INTO task_flow_positions (
                task_id, invocation_json, flow, step, node_id, human, session_run_id,
                ready_summary, step_index, iteration, position_version,
                worker_generation, claim_json, updated_at
            ) VALUES
                ('human', '{\"id\":\"pinned-human\"}', 'task-design', 'review-design', 'review', 1,
                 'run-human', 'approved scope', 1, 3, 7, 0, NULL, 100),
                ('worker', '{\"id\":\"pinned-worker\"}', 'slice', 'implement', NULL, 0,
                 NULL, NULL, 0, 2, 8, 4, '{\"generation\":4}', 101);",
        )
        .unwrap();
        let before: Vec<String> = conn
            .prepare(
                "SELECT json_array(task_id, invocation_json, session_run_id, ready_summary,
                step_index, iteration, position_version, worker_generation, claim_json)
             FROM task_flow_positions ORDER BY task_id",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        conn.execute_batch(&migration_section_sql("task_loop_review"))
            .unwrap();
        let after: Vec<String> = conn
            .prepare(
                "SELECT json_array(task_id, invocation_json, session_run_id, ready_summary,
                step_index, iteration, position_version, worker_generation, claim_json)
             FROM task_flow_positions ORDER BY task_id",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(before, after);
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM task_flow_positions WHERE review_json IS NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn durable_ask_invocations_migrate_pending_and_answered_history() {
        let conn = open();
        apply_before_draft(&conn, "durable_ask_invocations");
        conn.execute_batch(
            "PRAGMA foreign_keys=OFF;
             INSERT INTO waves (id, name, repo, created_at)
             VALUES ('wave_migration', 'migration', '/repo', 10);
             INSERT INTO epochs (
                 id, number, wave_id, project_id, task_id, state, current_rev,
                 created_at, terminal_at
             ) VALUES (
                 'epoch_00000000000000000000000000000001', 1, 'wave_migration',
                 NULL, NULL, 'open', 0, 10, NULL
             );
             INSERT INTO runs (
                 id, epoch_id, home_id, state, trigger_json, retry_of,
                 lease_hash, lease_generation, source_kind, source_id,
                 created_at, ended_at, stop_reason, containment_kind,
                 containment_id, cwd, started_at
             ) VALUES (
                 'run_00000000000000000000000000000001',
                 'epoch_00000000000000000000000000000001',
                 (SELECT id FROM homes LIMIT 1), 'active', '{\"kind\":\"user\"}',
                 NULL, 'lease', 1, 'wave', 'wave_migration', 11, NULL, NULL,
                 'tmux', 'migration', '/repo/worktree', 11
             );
             INSERT INTO agent_invocations (
                 id, run_id, process_id, started_at, ended_at, repo, worktree,
                 wave, flow, skill, provider, model, surface, capture_status,
                 incomplete_reason, outcome, artifact_dir, conversation_path,
                 provider_events_path, provider_session_id, provider_session_path,
                 conversation_event_count, conversation_bytes, project, task,
                 supervising_run_id, account_id, resume_token, handback_state,
                 answer_ask_id
             ) VALUES (
                 'invocation_00000000000000000000000000000001', 'trace', 'process',
                 12, NULL, '/repo', '/repo/worktree', 'migration', NULL, NULL,
                 'codex', NULL, 'headless', 'capturing', NULL, 'running',
                 '/tmp/artifacts', '/tmp/conversation', NULL, NULL, NULL,
                 0, 0, NULL, NULL, 'run_00000000000000000000000000000001',
                 NULL, NULL, NULL, 'ask_00000000000000000000000000000001'
             );
             INSERT INTO agent_turns (
                 id, invocation_id, ordinal, provider_turn_id, started_at,
                 ended_at, status, input_op, context_coverage, tokenizer,
                 system_prompt_path, task_prompt_path, system_tokens, task_tokens,
                 supplied_context_tokens, context_gather_ms, context_render_ms,
                 context_persist_ms, first_event_seq, last_event_seq, root_output,
                 epoch_id, basis_rev
             ) VALUES
                 ('turn_00000000000000000000000000000001',
                  'invocation_00000000000000000000000000000001', 1, NULL, 13,
                  NULL, 'running', 'initial', 'assembled', 'o200k_base', NULL,
                  '/tmp/task', 0, 0, 0, 0, 0, 0, NULL, NULL, NULL,
                  'epoch_00000000000000000000000000000001', 0),
                 ('turn_00000000000000000000000000000002',
                  'invocation_00000000000000000000000000000001', 2, NULL, 14,
                  NULL, 'running', 'message', 'assembled', 'o200k_base', NULL,
                  '/tmp/task', 0, 0, 0, 0, 0, 0, NULL, NULL, NULL,
                  'epoch_00000000000000000000000000000001', 0);
             INSERT INTO ask_exchanges (
                 id, turn_id, route_kind, route_work_kind, route_work_id,
                 question, asked_at, answer_author_kind, answer_author_id,
                 answer_text, answered_at
             ) VALUES
                 ('ask_00000000000000000000000000000001',
                  'turn_00000000000000000000000000000001', 'parent', 'project',
                  'proj_00000000000000000000000000000001', 'Pending?', 20,
                  NULL, NULL, NULL, NULL),
                 ('ask_00000000000000000000000000000002',
                 'turn_00000000000000000000000000000002', 'user', NULL, NULL,
                  'Answered?', 21, 'run',
                  'run_00000000000000000000000000000001', 'The answer', 22);
             INSERT INTO ask_linear_comment_outbox (
                 ask_id, transition, task_id, issue_id, body, created_at,
                 attempt_count, attempt_started_at, last_error,
                 linear_comment_id, delivered_at
             ) VALUES (
                 'ask_00000000000000000000000000000002', 'answer',
                 'task_00000000000000000000000000000001', 'ENG-1',
                 'historical task answer attribution', 22, 1, NULL, NULL,
                 'comment-1', 23
             );",
        )
        .unwrap();

        apply_draft(&conn, "durable_ask_invocations");

        let pending: (
            String,
            String,
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
        ) = conn
            .query_row(
                "SELECT state, target_kind, request_prompt, origin_cwd,
                        origin_turn_id, origin_invocation_id, asked_at, terminal_at
                 FROM ask_exchanges
                 WHERE id='ask_00000000000000000000000000000001'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            pending,
            (
                "queued".into(),
                "parent".into(),
                "Pending?".into(),
                "/repo/worktree".into(),
                "turn_00000000000000000000000000000001".into(),
                "invocation_00000000000000000000000000000001".into(),
                20,
                None,
            )
        );
        let answered: (
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
            i64,
            i64,
        ) = conn
            .query_row(
                "SELECT state, target_kind, result_kind, result_text,
                        terminal_author_kind, terminal_author_id,
                        origin_turn_id, origin_invocation_id, asked_at, terminal_at
                 FROM ask_exchanges
                 WHERE id='ask_00000000000000000000000000000002'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            answered,
            (
                "resolved".into(),
                "user".into(),
                "resolved".into(),
                "The answer".into(),
                "run".into(),
                Some("run_00000000000000000000000000000001".into()),
                "turn_00000000000000000000000000000002".into(),
                "invocation_00000000000000000000000000000001".into(),
                21,
                22,
            )
        );
        assert_eq!(
            conn.query_row(
                "SELECT issue_id || ':' || body FROM ask_linear_comment_outbox
                 WHERE ask_id='ask_00000000000000000000000000000002'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "ENG-1:historical task answer attribution"
        );
        assert_eq!(
            conn.query_row(
                "SELECT answer_ask_id FROM agent_invocations
                 WHERE id='invocation_00000000000000000000000000000001'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn legacy_task_flow_repair_restores_existing_tasks_without_moving_their_work() {
        let conn = open();
        apply_before_draft(&conn, LEGACY_TASK_FLOW_REPAIR_NAME);
        conn.execute_batch(
            "INSERT INTO waves (id, name, repo, created_at)
                 VALUES ('wave_restore', 'restore', '/repo', 100);
             INSERT INTO projects (id, wave_id, external_project_id, created_at)
                 VALUES ('proj_restore', 'wave_restore', 'linear_project', 100);
             INSERT INTO tasks (
                 id, project_id, external_issue_id, issue_identifier, created_at,
                 issue_title, issue_description, pm_snapshot_synced_at,
                 pm_writeback_json, worktree, workspace_slug, agent, provider,
                 iterate_flow, phase_cursor, phase_iteration, kickoff_flow,
                 gate_flow, lifecycle_phase, phase_epoch, gate_cycle, updated_at
             ) VALUES
                 ('task_legacy_a', 'proj_restore', 'issue_a', 'LOO-193', 101,
                  'Legacy A', '', 101, '{\"state\":\"current\"}', '/repo.a',
                  'legacy-a', 'codex', 'codex', 'task', 3, 4,
                  'task-design', 'ship', 'iterate', 7, 2, 111),
                 ('task_legacy_b', 'proj_restore', 'issue_b', 'LOO-195', 102,
                  'Legacy B', '', 102, '{\"state\":\"current\"}', '/repo.b',
                  'legacy-b', 'codex', 'codex', 'task', 5, 6,
                  'incident', 'ship', 'iterate', 8, 3, 112),
                 ('task_explicit', 'proj_restore', 'issue_c', 'LOO-206', 103,
                  'Explicit', '', 103, '{\"state\":\"current\"}', '/repo.c',
                  'explicit-c', 'codex', 'codex', 'ship-5whys', 1, 2,
                  'incident', 'ship', 'iterate', 9, 4, 113);
             INSERT INTO epochs (
                 id, number, task_id, state, current_rev, created_at
             ) VALUES
                 ('epoch_a', 1, 'task_legacy_a', 'open', 0, 101),
                 ('epoch_b', 1, 'task_legacy_b', 'open', 0, 102),
                 ('epoch_c', 1, 'task_explicit', 'open', 0, 103);
             INSERT INTO task_prs (
                 id, task_id, sequence, slug, branch, base_commit,
                 created_at, updated_at
             ) VALUES
                 ('pr_a', 'task_legacy_a', 1, 'legacy-a', 'jack/legacy-a', 'base-a', 101, 111),
                 ('pr_b', 'task_legacy_b', 1, 'legacy-b', 'jack/legacy-b', 'base-b', 102, 112),
                 ('pr_c', 'task_explicit', 1, 'explicit-c', 'jack/explicit-c', 'base-c', 103, 113);",
        )
        .unwrap();

        apply_draft(&conn, LEGACY_TASK_FLOW_REPAIR_NAME);

        let tasks = conn
            .prepare(
                "SELECT id, iterate_flow, kickoff_flow, gate_flow, lifecycle_phase,
                        phase_epoch, phase_cursor, phase_iteration, gate_cycle,
                        worktree, updated_at
                 FROM tasks ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            tasks,
            vec![
                (
                    "task_explicit".into(),
                    "ship-5whys".into(),
                    "incident".into(),
                    "ship".into(),
                    "iterate".into(),
                    9,
                    1,
                    2,
                    4,
                    "/repo.c".into(),
                    113,
                ),
                (
                    "task_legacy_a".into(),
                    "slice".into(),
                    "task-design".into(),
                    "ship".into(),
                    "iterate".into(),
                    7,
                    3,
                    4,
                    2,
                    "/repo.a".into(),
                    111,
                ),
                (
                    "task_legacy_b".into(),
                    "slice".into(),
                    "incident".into(),
                    "ship".into(),
                    "iterate".into(),
                    8,
                    5,
                    6,
                    3,
                    "/repo.b".into(),
                    112,
                ),
            ]
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM epochs", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM task_prs", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
    }

    #[test]
    fn repair_durable_input_timestamp_units_preserves_rows_and_seconds() {
        const LEGACY_NANOS: i64 = 1_784_521_517_123_456_789;
        const EXPECTED_SECONDS: i64 = 1_784_521_517;
        const EARLIEST_VALID_SECOND: i64 = -377_705_116_800;
        const LATEST_VALID_SECOND: i64 = 253_402_300_799;

        assert!(time::OffsetDateTime::from_unix_timestamp(EARLIEST_VALID_SECOND).is_ok());
        assert!(time::OffsetDateTime::from_unix_timestamp(EARLIEST_VALID_SECOND - 1).is_err());
        assert!(time::OffsetDateTime::from_unix_timestamp(LATEST_VALID_SECOND).is_ok());
        assert!(time::OffsetDateTime::from_unix_timestamp(LATEST_VALID_SECOND + 1).is_err());

        let conn = open();
        apply_before_draft(&conn, "repair_durable_input_timestamp_units");
        conn.execute_batch(&format!(
            "PRAGMA foreign_keys = OFF;
             INSERT INTO epoch_revisions (epoch_id, rev, kind, source_id, created_at)
             VALUES
                 ('epoch_legacy', 0, 'steer', 'steer_legacy', {LEGACY_NANOS}),
                 ('epoch_legacy', 1, 'tool_response', 'response_legacy', {LEGACY_NANOS}),
                 ('epoch_seconds', 0, 'steer', 'steer_seconds', {EXPECTED_SECONDS}),
                 ('epoch_seconds', 1, 'tool_response', 'response_seconds', {EXPECTED_SECONDS}),
                 ('epoch_seconds', 2, 'evidence', 'latest_valid', {LATEST_VALID_SECOND});
             INSERT INTO steers (
                 id, epoch_id, rev, author_kind, author_run_id, text, issued_at
             ) VALUES
                 ('steer_legacy', 'epoch_legacy', 0, 'user', NULL, 'legacy', {LEGACY_NANOS}),
                 ('steer_seconds', 'epoch_seconds', 0, 'user', NULL, 'seconds', {EXPECTED_SECONDS});
             INSERT INTO tool_responses (
                 id, epoch_id, rev, request_id, choice, responded_at
             ) VALUES
                 ('response_legacy', 'epoch_legacy', 1, 'request_legacy', 'legacy', {LEGACY_NANOS}),
                 ('response_seconds', 'epoch_seconds', 1, 'request_seconds', 'seconds', {EXPECTED_SECONDS});"
        ))
        .unwrap();

        apply_draft(&conn, "repair_durable_input_timestamp_units");

        let timestamp = |table: &str, column: &str, id_column: &str, id: &str| -> i64 {
            conn.query_row(
                &format!("SELECT {column} FROM {table} WHERE {id_column}=?1"),
                [id],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(
            timestamp("epoch_revisions", "created_at", "source_id", "steer_legacy"),
            EXPECTED_SECONDS
        );
        assert_eq!(
            timestamp("steers", "issued_at", "id", "steer_legacy"),
            EXPECTED_SECONDS
        );
        assert_eq!(
            timestamp("tool_responses", "responded_at", "id", "response_legacy"),
            EXPECTED_SECONDS
        );
        assert_eq!(
            timestamp("epoch_revisions", "created_at", "source_id", "latest_valid"),
            LATEST_VALID_SECOND,
            "an already valid second at the OffsetDateTime boundary is untouched"
        );
        assert_eq!(
            timestamp("steers", "issued_at", "id", "steer_seconds"),
            EXPECTED_SECONDS
        );
        assert_eq!(
            timestamp("tool_responses", "responded_at", "id", "response_seconds"),
            EXPECTED_SECONDS
        );
        for (table, expected) in [
            ("epoch_revisions", 5_i64),
            ("steers", 2_i64),
            ("tool_responses", 2_i64),
        ] {
            assert_eq!(
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
                expected,
                "the repair must preserve every {table} row"
            );
        }
    }

    #[test]
    fn wave_directory_migration_preserves_ids_and_derives_parent_addresses() {
        let conn = open();
        apply_before_migration(&conn, "wave_directory_parents");
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES
            ('parent','infrastructure','/repo',1),
            ('child','infrastructure/release','/repo',2),
            ('other','product/release','/repo',3);
            INSERT INTO projects(id,wave_id,external_project_id,created_at,updated_at)
            VALUES ('release-plan','child','linear-release',1,1);",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("wave_directory_parents"))
            .unwrap();
        let child: (String, String, String, i64) = conn
            .query_row(
                "SELECT name,parent_wave_id,slug,created_at FROM wave_addresses WHERE id='child'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            child,
            (
                "release".into(),
                "parent".into(),
                "infrastructure/release".into(),
                2
            )
        );
        assert_eq!(
            conn.query_row(
                "SELECT wave_id FROM projects WHERE id='release-plan'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "child"
        );
        conn.execute("UPDATE waves SET name='infra' WHERE id='parent'", [])
            .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT slug FROM wave_addresses WHERE id='child'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "infra/release"
        );
        assert_eq!(
            conn.query_row(
                "SELECT slug FROM wave_addresses WHERE id='other'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "product/release"
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
        let schema = product_schema(&conn).unwrap();
        conn.execute_batch("DROP VIEW wave_addresses").unwrap();
        assert_ne!(
            schema,
            product_schema(&conn).unwrap(),
            "schema verification must detect a missing address view"
        );
    }

    #[test]
    fn wave_directory_migration_preserves_paths_across_retired_ancestors() {
        let conn = open();
        apply_before_migration(&conn, "wave_directory_parents");
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at,retired_at,superseded_by_wave_id,retirement_reason) VALUES
            ('retired-parent','infra','/repo',1,2,'root','relocated'),
            ('child','infra/release','/repo',3,NULL,NULL,NULL),
            ('root','release','/repo',4,NULL,NULL,NULL),
            ('retired-child','archive/retired','/repo',1,2,'root','relocated');",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("wave_directory_parents"))
            .unwrap();
        let (slug, parent_retired): (String, Option<i64>) = conn
            .query_row(
                "SELECT child.slug,parent.retired_at FROM wave_addresses child
                 JOIN waves parent ON parent.id=child.parent_wave_id WHERE child.id='child'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(slug, "infra/release");
        assert_eq!(parent_retired, None);
        assert_eq!(
            conn.query_row(
                "SELECT slug FROM wave_addresses WHERE id='retired-child'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "archive/retired"
        );
        assert_eq!(
            conn.query_row(
                "SELECT retired_at FROM waves WHERE id='retired-parent'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM waves WHERE retired_at IS NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            3,
            "only the missing active ancestor is registered"
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            0
        );
    }

    #[test]
    fn wave_promotion_occurrence_does_not_backfill_existing_ancestry() {
        let conn = open();
        apply_before_draft(&conn, "wave_promotion_occurrence");
        conn.execute_batch(
            "INSERT INTO waves (id, name, repo, created_at)
                 VALUES ('wave_parent', 'parent', '/repo', 100);
             INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
                 VALUES ('wave_child', 'child', '/repo', 101, 'wave_parent');",
        )
        .unwrap();

        apply_draft(&conn, "wave_promotion_occurrence");

        assert!(columns(&conn, "waves").contains(&"promoted_at".to_string()));
        let promoted_at: Option<i64> = conn
            .query_row(
                "SELECT promoted_at FROM waves WHERE id='wave_child'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(promoted_at, None, "ancestry is not a promotion occurrence");
    }

    #[test]
    fn status_surfaces_migration_attributes_only_uniquely_contained_project_failures() {
        let conn = open();
        apply_before_migration(&conn, "status_truth");
        conn.execute_batch(
            "PRAGMA foreign_keys=OFF;
             INSERT INTO waves (id, name, repo, created_at)
             VALUES ('wave_status', 'status', '/repo', 1);
             INSERT INTO projects (id, wave_id, external_project_id, created_at)
             VALUES ('project_status', 'wave_status', 'linear-status', 2);
             INSERT INTO epochs (
                 id, number, wave_id, project_id, task_id, state, current_rev,
                 created_at, terminal_at
             ) VALUES
                 ('epoch_status', 1, NULL, 'project_status', NULL, 'open', 0, 100, NULL);
             INSERT INTO runs (
                 id, epoch_id, home_id, state, trigger_json, retry_of,
                 runtime_generation, source_kind, source_id,
                 created_at, ended_at, stop_reason,
                 containment_kind, containment_id, cwd, started_at
             ) VALUES
                 ('run_unique', 'epoch_status', (SELECT id FROM homes LIMIT 1),
                  'ended', '{\"kind\":\"user\"}', NULL, NULL,
                  'project', 'project_status', 100, 130, '{\"kind\":\"recovery\"}',
                  NULL, NULL, NULL, NULL),
                 ('run_overlap_a', 'epoch_status', (SELECT id FROM homes LIMIT 1),
                  'ended', '{\"kind\":\"user\"}', NULL, NULL,
                  'project', 'project_status', 200, 240, 'historical import',
                  NULL, NULL, NULL, NULL),
                 ('run_overlap_b', 'epoch_status', (SELECT id FROM homes LIMIT 1),
                  'ended', '{\"kind\":\"user\"}', NULL, NULL,
                  'project', 'project_status', 200, 240, NULL, NULL, NULL, NULL, NULL);
             INSERT INTO project_events (project_id, kind_json, created_at) VALUES
                 ('project_status',
                  '{\"kind\":\"failed\",\"error\":\"credential\",\"resumable\":true}',
                  120),
                 ('project_status',
                  '{\"kind\":\"failed\",\"error\":\"controller\",\"resumable\":true}',
                  90),
                 ('project_status',
                  '{\"kind\":\"failed\",\"error\":\"ambiguous\",\"resumable\":true}',
                  220);
             INSERT INTO agent_invocations (
                 id, run_id, process_id, started_at, ended_at, repo, worktree,
                 provider, surface, capture_status, outcome, artifact_dir,
                 conversation_path, conversation_event_count, conversation_bytes,
                 supervising_run_id
             ) VALUES (
                 'invocation_74115449', 'trace-status', 'process-status', 110, 125,
                 '/repo', '/repo', 'codex', 'headless', 'complete', 'completed',
                 '/tmp/artifact', '/tmp/conversation', 1, 1, 'run_unique'
             ), (
                 'invocation_11111111111111111111111111111111',
                 'trace-stale', 'process-stale', 115, NULL,
                 '/repo', '/repo', 'codex', 'headless', 'capturing', 'running',
                 '/tmp/stale-artifact', '/tmp/stale-conversation', 0, 0, 'run_unique'
             );
             INSERT INTO agent_turns (
                 id, invocation_id, ordinal, started_at, ended_at, status,
                 input_op, context_coverage, tokenizer, task_prompt_path,
                 system_tokens, task_tokens, supplied_context_tokens,
                 context_gather_ms, context_render_ms, context_persist_ms
             ) VALUES (
                 'turn_status', 'invocation_74115449', 0, 111, 124, 'completed',
                 'initial', 'assembled', 'none', '/tmp/prompt', 0, 0, 0, 0, 0, 0
             );
             INSERT INTO ask_exchanges (
                 id, epoch_id, origin_work_kind, origin_work_id, origin_run_id,
                 origin_turn_id, origin_invocation_id, origin_home_id, origin_cwd,
                 target_kind, request_kind, request_prompt, state,
                 active_invocation_id, asked_at
             ) VALUES (
                 'ask_status', 'epoch_status', 'project', 'project_status',
                 'run_unique', 'turn_status', 'invocation_74115449',
                 (SELECT id FROM homes LIMIT 1), '/repo', 'user', 'intervention',
                 'preserve invocation ownership', 'claimed',
                 'invocation_74115449', 112
             );",
        )
        .unwrap();

        conn.execute_batch(&migration_section_sql("status_truth"))
            .unwrap();

        let rows = conn
            .prepare("SELECT run_id FROM project_events ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get::<_, Option<String>>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows, vec![Some("run_unique".to_string()), None, None]);
        assert_eq!(
            conn.query_row(
                "SELECT stop_reason FROM runs WHERE id='run_overlap_a'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "historical import"
        );
        let canonical = "invocation_74115449000000000000000000000000";
        assert_eq!(
            conn.query_row(
                "SELECT invocation_id FROM agent_turns WHERE id='turn_status'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            canonical
        );
        assert_eq!(
            conn.query_row(
                "SELECT origin_invocation_id || ':' || active_invocation_id
                 FROM ask_exchanges WHERE id='ask_status'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            format!("{canonical}:{canonical}")
        );
        assert_eq!(
            conn.query_row(
                "SELECT ended_at || ':' || outcome || ':' || handback_state
                 FROM agent_invocations
                 WHERE id='invocation_11111111111111111111111111111111'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "130:unknown:unknown"
        );
    }
}

#[cfg(test)]
#[path = "canonical_ids_tests.rs"]
mod canonical_ids_tests;
