//! Release-scoped schema migrations. See `MIGRATIONS.md` next to this file for
//! the convention; the one rule is that a shipped migration is never edited.

use std::fmt;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::store::{StoreError, StoreResult};
use fs2::FileExt;
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};

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
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 1,
        },
        name: "task_prs",
        sql: include_str!("migrations/0.11.001_task_prs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 2,
        },
        name: "project_session_successors",
        sql: include_str!("migrations/0.11.002_project_session_successors.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 3,
        },
        name: "child_body_lease",
        sql: include_str!("migrations/0.11.003_child_body_lease.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 4,
        },
        name: "task_pr_ci_state",
        sql: include_str!("migrations/0.11.004_task_pr_ci_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 5,
        },
        name: "provider_accounts",
        sql: include_str!("migrations/0.11.005_provider_accounts.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 6,
        },
        name: "context_launch_work",
        sql: include_str!("migrations/0.11.006_context_launch_work.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 7,
        },
        name: "task_pr_parent",
        sql: include_str!("migrations/0.11.007_task_pr_parent.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 8,
        },
        name: "interactive_handoffs",
        sql: include_str!("migrations/0.11.008_interactive_handoffs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 9,
        },
        name: "context_pressure",
        sql: include_str!("migrations/0.11.009_context_pressure.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 10,
        },
        name: "context_input_normalization",
        sql: include_str!("migrations/0.11.010_context_input_normalization.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 11,
        },
        name: "profiles",
        sql: include_str!("migrations/0.11.011_profiles.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 12,
        },
        name: "provider_account_lifecycle",
        sql: include_str!("migrations/0.11.012_provider_account_lifecycle.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 13,
        },
        name: "task_review_state",
        sql: include_str!("migrations/0.11.013_task_review_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 14,
        },
        name: "task_lifecycle",
        sql: include_str!("migrations/0.11.014_task_lifecycle.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 15,
        },
        name: "interaction_reviews",
        sql: include_str!("migrations/0.11.015_interaction_reviews.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 16,
        },
        name: "task_linear_observations",
        sql: include_str!("migrations/0.11.016_task_linear_observations.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 17,
        },
        name: "migration_provenance",
        sql: include_str!("migrations/0.11.017_migration_provenance.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 18,
        },
        name: "session_body_provenance",
        sql: include_str!("migrations/0.11.018_session_body_provenance.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 19,
        },
        name: "task_pr_github_observation",
        sql: include_str!("migrations/0.11.019_task_pr_github_observation.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 20,
        },
        name: "task_pr_linear_linkage",
        sql: include_str!("migrations/0.11.020_task_pr_linear_linkage.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 21,
        },
        name: "provider_deliveries",
        sql: include_str!("migrations/0.11.021_provider_deliveries.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 22,
        },
        name: "task_session_successors",
        sql: include_str!("migrations/0.11.022_task_session_successors.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 23,
        },
        name: "capture_pruned_state",
        sql: include_str!("migrations/0.11.023_capture_pruned_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 24,
        },
        name: "ci_incidents",
        sql: include_str!("migrations/0.11.024_ci_incidents.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 25,
        },
        name: "usage_deltas",
        sql: include_str!("migrations/0.11.025_usage_deltas.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 26,
        },
        name: "lineage_boundary",
        sql: include_str!("migrations/0.11.026_lineage_boundary.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 27,
        },
        name: "accounts_first",
        sql: include_str!("migrations/0.11.027_accounts_first.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 29,
        },
        name: "ci_incident_repaired_head",
        sql: include_str!("migrations/0.11.029_ci_incident_repaired_head.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 30,
        },
        name: "one_spend_grain",
        sql: include_str!("migrations/0.11.030_one_spend_grain.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 31,
        },
        name: "durable_input_spine",
        sql: include_str!("migrations/0.11.031_durable_input_spine.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 32,
        },
        name: "run_launch_attention",
        sql: include_str!("migrations/0.11.032_run_launch_attention.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 33,
        },
        name: "launch_attention_only",
        sql: include_str!("migrations/0.11.033_launch_attention_only.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 34,
        },
        name: "typed_ci_runs",
        sql: include_str!("migrations/0.11.034_typed_ci_runs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 35,
        },
        name: "drop_child_commands",
        sql: include_str!("migrations/0.11.035_drop_child_commands.sql"),
    },
];

/// The exact branch-local history that reached one production ledger before
/// main established `0.11.008_interactive_handoffs`. These ids were never
/// released. They remain here only long enough to recognize and converge that
/// known history without treating arbitrary unknown migrations as ours.
const DIVERGENT_MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 8,
        },
        name: "context_pressure",
        sql: include_str!("migrations/0.11.009_context_pressure.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 9,
        },
        name: "context_input_normalization",
        sql: include_str!("migrations/0.11.010_context_input_normalization.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 10,
        },
        name: "profiles",
        sql: include_str!("migrations/0.11.011_profiles.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            ordinal: 11,
        },
        name: "provider_account_lifecycle",
        sql: include_str!("migrations/0.11.012_provider_account_lifecycle.sql"),
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

const RECREATE_MESSAGE: &str =
    "incompatible Loopflow database; delete loopflow.db and rerun the command";

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
    apply_sqlite_transaction(conn, |_| Ok(()))
}

/// Stage a fresh connection one migration behind the binary's known head. The
/// store-level shared-frontier regressions use it to build a database the running
/// binary could advance but an ordinary open must leave alone; the resulting
/// frontier is `MIGRATIONS[len - 2]` (today `0.11.027_accounts_first`).
#[cfg(test)]
pub(crate) fn apply_all_but_head(conn: &rusqlite::Connection) -> StoreResult<()> {
    apply_set(conn, &MIGRATIONS[..MIGRATIONS.len() - 1])
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
/// Branch builds use this against the release-owned database: they can reuse
/// compatible state, but an unpublished migration never becomes durable there.
pub(crate) fn validate_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    validate_set(MIGRATIONS).map_err(StoreError::InvalidData)?;
    if !user_tables(conn)?
        .iter()
        .any(|table| table == "schema_migrations")
    {
        return Err(StoreError::InvalidData(
            "a validation-only lf cannot initialize the release database; install a published lf"
                .to_string(),
        ));
    }
    let applied = applied_versions(conn)?;
    pending_migrations(&applied, MIGRATIONS)?;
    validate_applied_checksums(conn, MIGRATIONS)?;
    validate_schema(conn, &MIGRATIONS[..applied.len()])?;
    validate_foreign_keys(conn)
}

fn apply_sqlite_transaction(
    conn: &rusqlite::Connection,
    before_migration: impl FnOnce(&rusqlite::Connection) -> StoreResult<()>,
) -> StoreResult<()> {
    let foreign_keys_enabled: bool =
        conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let migration_result = match conn.execute_batch("BEGIN EXCLUSIVE") {
        Ok(()) => {
            let result = before_migration(conn)
                .and_then(|()| apply_set(conn, MIGRATIONS))
                .and_then(|()| validate_foreign_keys(conn));
            match result {
                Ok(()) => conn.execute_batch("COMMIT").map_err(StoreError::from),
                Err(error) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    Err(error)
                }
            }
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

pub(crate) fn apply_sqlite_with_backup(
    conn: &rusqlite::Connection,
    path: &Path,
) -> StoreResult<()> {
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")?;
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
    let result = match requires_migration_sqlite(conn) {
        Ok(false) => Ok(()),
        Ok(true) => {
            apply_sqlite_transaction(conn, |conn| backup_before_migration(conn, path).map(|_| ()))
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
    if valid_backup(&backup_path, &history) {
        return Ok(Some(backup_path));
    }

    let unique = format!(
        "{file_name}.backup-{safe_version}.tmp-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    );
    let temporary_path = path.with_file_name(unique);
    let source = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut destination = rusqlite::Connection::open(&temporary_path)?;
    let backup = rusqlite::backup::Backup::new(&source, &mut destination)?;
    if let Err(error) = backup.run_to_completion(64, Duration::from_millis(10), None) {
        drop(backup);
        drop(destination);
        drop(source);
        let _ = std::fs::remove_file(&temporary_path);
        return Err(error.into());
    }
    drop(backup);
    drop(destination);
    drop(source);
    if let Err(error) = std::fs::File::open(&temporary_path).and_then(|file| file.sync_all()) {
        let _ = std::fs::remove_file(&temporary_path);
        return Err(StoreError::InvalidData(format!(
            "failed to sync migration backup: {error}"
        )));
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

fn valid_backup(path: &Path, expected_history: &str) -> bool {
    let Ok(connection) = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return false;
    };
    let integrity: rusqlite::Result<String> =
        connection.pragma_query_value(None, "integrity_check", |row| row.get(0));
    integrity.as_deref() == Ok("ok")
        && migration_history_fingerprint(&connection)
            .is_ok_and(|history| history == expected_history)
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
    for object in schema {
        hash_text(&mut digest, &object.object_type);
        hash_text(&mut digest, &object.name);
        hash_text(&mut digest, &object.table_name);
        hash_text(&mut digest, &object.sql);
        digest.update((object.foreign_keys.len() as u64).to_be_bytes());
        for foreign_key in object.foreign_keys {
            digest.update(foreign_key.id.to_be_bytes());
            digest.update(foreign_key.sequence.to_be_bytes());
            hash_text(&mut digest, &foreign_key.table);
            hash_text(&mut digest, &foreign_key.from);
            match foreign_key.to {
                Some(to) => {
                    digest.update([1]);
                    hash_text(&mut digest, &to);
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
    adopt_divergent_history(conn, set)?;
    adopt_permuted_history(conn, set)?;

    let applied = applied_versions(conn)?;
    for migration in pending_migrations(&applied, set)? {
        let parent_history = migration_prefix_fingerprint(&applied_versions(conn)?, set)?;
        migration_preflight(conn, migration)?;
        conn.execute_batch(migration.sql)?;
        backfill_known_checksums(conn, set)?;
        insert_applied_migration(conn, migration, &parent_history)?;
    }

    validate_applied_checksums(conn, set)?;
    validate_schema(conn, set)
}

fn migration_preflight(conn: &rusqlite::Connection, migration: &Migration) -> StoreResult<()> {
    if migration.name != "durable_input_spine" {
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

fn migration_checksum(migration: &Migration) -> String {
    hex::encode(Sha256::digest(migration.sql.as_bytes()))
}

fn migration_prefix_fingerprint(applied: &[String], set: &[Migration]) -> StoreResult<String> {
    let mut digest = Sha256::new();
    digest.update((applied.len() as u64).to_be_bytes());
    for version in applied {
        let migration = set
            .iter()
            .find(|migration| migration.version() == *version)
            .ok_or_else(incompatible)?;
        hash_text(&mut digest, version);
        hash_text(&mut digest, &migration_checksum(migration));
    }
    Ok(hex::encode(digest.finalize()))
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
    for migration in set {
        conn.execute(
            "UPDATE schema_migrations SET checksum = ?1
             WHERE version = ?2 AND checksum IS NULL",
            (migration_checksum(migration), migration.version()),
        )?;
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
        let migration = set
            .iter()
            .find(|migration| migration.version() == version)
            .ok_or_else(incompatible)?;
        if checksum != migration_checksum(migration) {
            return Err(StoreError::InvalidData(format!(
                "database migration {version} checksum does not match this lf build"
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

    let expected = rusqlite::Connection::open_in_memory()?;
    for migration in &set[..prefix_len] {
        expected.execute_batch(migration.sql)?;
    }
    if product_schema(conn)? != product_schema(&expected)? {
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
        .map(|migration| migration.name)
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
    let expected = rusqlite::Connection::open_in_memory()?;
    for migration in &set[..canonical_start] {
        expected.execute_batch(migration.sql)?;
    }
    for migration in &DIVERGENT_MIGRATIONS[..divergent_len] {
        expected.execute_batch(migration.sql)?;
    }
    if product_schema(conn)? != product_schema(&expected)? {
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
            Some(_) => Err(StoreError::InvalidData(format!(
                "database migration {version} is unknown to lf {} (latest known {}); this database needs a newer release or the matching divergent local build; run lf doctor with that binary",
                env!("CARGO_PKG_VERSION"),
                set.last()
                    .map(Migration::version)
                    .unwrap_or_else(|| "none".to_string())
            ))),
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

/// A database matches only if its complete product schema matches the schema the
/// migration chain builds. Names alone miss type, constraint, index, trigger,
/// and foreign-key drift.
fn validate_schema(conn: &rusqlite::Connection, set: &[Migration]) -> StoreResult<()> {
    let expected = rusqlite::Connection::open_in_memory()?;
    expected.execute_batch(
        "CREATE TABLE schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;
    for migration in set {
        expected.execute_batch(migration.sql)?;
    }

    if product_schema(conn)? != product_schema(&expected)? {
        return Err(incompatible());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductSchemaObject {
    object_type: String,
    name: String,
    table_name: String,
    sql: String,
    foreign_keys: Vec<ForeignKeyDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForeignKeyDefinition {
    id: i64,
    sequence: i64,
    table: String,
    from: String,
    to: Option<String>,
    on_update: String,
    on_delete: String,
    match_clause: String,
}

/// Every product table, index, and trigger with defining SQL and
/// explicit foreign-key metadata. `schema_migrations` is bookkeeping rather
/// than product schema, so no migration declares it.
fn product_schema(conn: &rusqlite::Connection) -> StoreResult<Vec<ProductSchemaObject>> {
    let mut statement = conn.prepare(
        "SELECT type, name, tbl_name, COALESCE(sql, '')
         FROM sqlite_master
         WHERE type IN ('table', 'index', 'trigger')
           AND name NOT LIKE 'sqlite_%'
           AND name != 'schema_migrations'
         ORDER BY type, name",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut schema = Vec::new();
    for row in rows {
        let (object_type, name, table_name, sql) = row?;
        let foreign_keys = if object_type == "table" {
            let quoted = format!("\"{}\"", name.replace('"', "\"\""));
            let mut foreign_key_statement =
                conn.prepare(&format!("PRAGMA foreign_key_list({quoted})"))?;
            let foreign_keys = foreign_key_statement
                .query_map([], |row| {
                    Ok(ForeignKeyDefinition {
                        id: row.get(0)?,
                        sequence: row.get(1)?,
                        table: row.get(2)?,
                        from: row.get(3)?,
                        to: row.get(4)?,
                        on_update: row.get(5)?,
                        on_delete: row.get(6)?,
                        match_clause: row.get(7)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            foreign_keys
        } else {
            Vec::new()
        };
        schema.push(ProductSchemaObject {
            object_type,
            name,
            table_name,
            sql: sql.trim().to_string(),
            foreign_keys,
        });
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

pub fn latest_known_version() -> String {
    MIGRATIONS
        .last()
        .map(Migration::version)
        .unwrap_or_else(|| "none".to_string())
}

/// The next migration this binary knows that the store has not applied, or
/// `None` when the store is exactly at this binary's frontier.
///
/// Call only after [`validate_sqlite`] has confirmed the applied history is a
/// clean recognized prefix; then `pending_migrations` cannot error and this is a
/// pure "is the store behind me?" question. An ordinary open of the shared store
/// refuses when this is `Some`: the running binary's code may query columns that
/// pending migration adds, so reusing the older schema would only fail later.
pub(crate) fn pending_shared_migration(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    let applied = applied_versions(conn)?;
    Ok(pending_migrations(&applied, MIGRATIONS)?
        .first()
        .map(Migration::version))
}

pub fn latest_applied_version_sqlite(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    if !user_tables(conn)?
        .iter()
        .any(|table| table == "schema_migrations")
    {
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
    let tables = user_tables(conn)?;
    if !tables.iter().any(|table| table == "schema_migrations") {
        return if tables.is_empty() {
            Ok(true)
        } else {
            Err(incompatible())
        };
    }
    let applied = applied_versions(conn)?;
    if applied.is_empty() && tables.len() == 1 {
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
}

fn incompatible() -> StoreError {
    StoreError::InvalidData(RECREATE_MESSAGE.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::sync_channel;
    use std::time::Duration;

    use rusqlite::OptionalExtension;

    use super::{
        active_namespace, applied_versions, apply_set, apply_sqlite, apply_sqlite_transaction,
        apply_sqlite_with_backup, backup_before_migration, latest_applied_version_sqlite,
        latest_known_version, latest_version_sqlite, pending_migrations, product_schema,
        validate_foreign_keys, validate_set, validate_sqlite, Migration, MigrationId,
        DIVERGENT_MIGRATIONS, MIGRATIONS,
    };
    use crate::task::TaskEventKind;

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

    fn apply_divergent_history(conn: &rusqlite::Connection, count: usize) {
        let canonical_start = MIGRATIONS
            .iter()
            .position(|migration| migration.name == "interactive_handoffs")
            .unwrap();
        apply_set(conn, &MIGRATIONS[..canonical_start]).unwrap();
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
            .position(|migration| migration.name == "context_pressure")
            .unwrap();
        apply_set(conn, &MIGRATIONS[..context_start]).unwrap();
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
                .find(|migration| migration.name == name)
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

    /// Insert one `agent_launches` row carrying `capture_status`, reporting
    /// whether the table's CHECK constraint accepted it. Rolls the probe row
    /// back so callers can reuse the connection.
    fn capture_status_accepts(conn: &rusqlite::Connection, capture_status: &str) -> bool {
        let id = format!("probe-{capture_status}");
        let inserted = conn
            .execute(
                "INSERT INTO agent_launches (
                     id, run_id, process_id, started_at, repo, worktree, provider,
                     surface, capture_status, outcome, artifact_dir,
                     conversation_path, conversation_event_count, conversation_bytes
                 ) VALUES (?1, 'run-probe', 'proc-probe', 100, '/repo', '/repo',
                     'codex', 'headless', ?2, 'completed', 'probe/dir',
                     'probe/conversation.jsonl', 1, 10)",
                rusqlite::params![id, capture_status],
            )
            .is_ok();
        if inserted {
            conn.execute("DELETE FROM agent_launches WHERE id = ?1", [&id])
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
            .filter_map(|entry| {
                let entry = entry.unwrap();
                entry.file_type().unwrap().is_file().then(|| {
                    entry
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned()
                })
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
            .any(|object| object.object_type == "table" && object.name == "task_sessions"));
        for table in ["project_sessions", "task_sessions"] {
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
                    'interaction_reviews', 'interactive_handoffs'
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0,
            "the durable spine has no compatibility or shadow lifecycle tables"
        );
        let turn_columns = columns(&conn, "agent_turns");
        assert!(turn_columns.iter().any(|name| name == "epoch_id"));
        assert!(turn_columns.iter().any(|name| name == "basis_rev"));
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM pragma_foreign_key_list('sends')
                 WHERE \"table\"='agent_turns' AND \"from\"='turn_id' AND \"to\"='id'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1,
            "Send evidence belongs to the sole durable Turn row"
        );

        apply_sqlite(&conn).unwrap();
        assert_eq!(
            applied_versions(&conn).unwrap(),
            vec![
                "0.10.001_initial".to_string(),
                "0.10.002_session_execution_context".to_string(),
                "0.11.001_task_prs".to_string(),
                "0.11.002_project_session_successors".to_string(),
                "0.11.003_child_body_lease".to_string(),
                "0.11.004_task_pr_ci_state".to_string(),
                "0.11.005_provider_accounts".to_string(),
                "0.11.006_context_launch_work".to_string(),
                "0.11.007_task_pr_parent".to_string(),
                "0.11.008_interactive_handoffs".to_string(),
                "0.11.009_context_pressure".to_string(),
                "0.11.010_context_input_normalization".to_string(),
                "0.11.011_profiles".to_string(),
                "0.11.012_provider_account_lifecycle".to_string(),
                "0.11.013_task_review_state".to_string(),
                "0.11.014_task_lifecycle".to_string(),
                "0.11.015_interaction_reviews".to_string(),
                "0.11.016_task_linear_observations".to_string(),
                "0.11.017_migration_provenance".to_string(),
                "0.11.018_session_body_provenance".to_string(),
                "0.11.019_task_pr_github_observation".to_string(),
                "0.11.020_task_pr_linear_linkage".to_string(),
                "0.11.021_provider_deliveries".to_string(),
                "0.11.022_task_session_successors".to_string(),
                "0.11.023_capture_pruned_state".to_string(),
                "0.11.024_ci_incidents".to_string(),
                "0.11.025_usage_deltas".to_string(),
                "0.11.026_lineage_boundary".to_string(),
                "0.11.027_accounts_first".to_string(),
                "0.11.029_ci_incident_repaired_head".to_string(),
                "0.11.030_one_spend_grain".to_string(),
                "0.11.031_durable_input_spine".to_string(),
                "0.11.032_run_launch_attention".to_string(),
                "0.11.033_launch_attention_only".to_string(),
                "0.11.034_typed_ci_runs".to_string(),
                "0.11.035_drop_child_commands".to_string()
            ]
        );
    }

    /// Everything up to but excluding `name`. A test whose subject is one
    /// migration names it, so appending the next migration cannot silently
    /// re-point the test at different SQL.
    fn prefix_before(name: &str) -> &'static [Migration] {
        let index = MIGRATIONS
            .iter()
            .position(|migration| migration.name == name)
            .expect("named migration is registered");
        &MIGRATIONS[..index]
    }

    #[test]
    fn validation_only_open_does_not_apply_an_unpublished_tail() {
        let conn = open();
        let published = &MIGRATIONS[..MIGRATIONS.len() - 1];
        apply_set(&conn, published).unwrap();
        let tail = MIGRATIONS.last().expect("a tail to withhold");

        validate_sqlite(&conn).unwrap();

        assert_eq!(
            latest_applied_version_sqlite(&conn).unwrap(),
            published.last().map(Migration::version),
            "a validation-only open advances nothing"
        );
        assert!(capture_status_accepts(&conn, "pruned"));
        // Bait for the withheld tail: the durable input migration creates the
        // stable Work tables. Their absence proves validation stayed read-only.
        assert_eq!(
            tail.name, "durable_input_spine",
            "bait tracks the current tail"
        );
        assert!(
            conn.prepare("SELECT id FROM projects LIMIT 0").is_err(),
            "a validation-only open must not run the tail's schema change"
        );
    }

    /// The validation primitive underneath the shared-store gate: with the store
    /// at the frontier the installed `lf` knows (`0.11.027`) and the candidate one
    /// migration ahead (`0.11.029`), `validate_sqlite` recognizes the applied
    /// prefix and pins `pending_shared_migration` to the exact head the ordinary
    /// store open then refuses on — the frontier never advances, and the old
    /// reader still recognizes the store.
    #[test]
    fn validate_recognizes_a_shorter_frontier_and_names_the_pending_head() {
        let installed = &MIGRATIONS[..MIGRATIONS.len() - 1];
        let candidate = MIGRATIONS;

        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        // Bring the store to the frontier the installed binary shipped with.
        apply_set(&conn, installed).unwrap();
        let installed_frontier = latest_applied_version_sqlite(&conn).unwrap().unwrap();
        assert_eq!(installed_frontier, "0.11.027_accounts_first");

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
        apply_set(&conn, candidate).unwrap();
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
        apply_set(&conn, prefix_before("lineage_boundary")).unwrap();
        conn.execute_batch(
            "INSERT INTO run_events (run_id, process_id, parent_process_id, seq, ts, node, event)
             VALUES ('trace_a', 'proc_root',   NULL,         0, 100, 'run', 'started'),
                    ('trace_a', 'proc_child',  'proc_root',  0, 101, 'run', 'started'),
                    ('trace_b', 'proc_orphan', 'proc_ghost', 0, 102, 'run', 'started')",
        )
        .unwrap();

        apply_sqlite(&conn).unwrap();

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
    }

    #[test]
    fn capture_pruned_migration_widens_the_enum_and_keeps_existing_launches() {
        // SQLite bakes CHECK into the table, so `pruned` arrives via a table
        // rebuild. The rebuild must carry every historical launch across —
        // those rows are the token and spend accounting we tombstone rather
        // than delete.
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, prefix_before("capture_pruned_state")).unwrap();
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

        apply_sqlite(&conn).unwrap();

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
    fn the_capture_pruned_rebuild_restores_every_launch_index() {
        // A rebuild drops the table, and with it its indexes. Losing one would
        // silently degrade every `lf runs`/`lf trace` lookup.
        let conn = open();
        apply_sqlite(&conn).unwrap();

        let mut indexes = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'agent_launches'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        indexes.sort();
        indexes.retain(|name| !name.starts_with("sqlite_autoindex"));

        assert_eq!(
            indexes,
            vec![
                "idx_agent_launches_process",
                "idx_agent_launches_project",
                "idx_agent_launches_run",
                "idx_agent_launches_task",
                "idx_agent_launches_wave",
            ]
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
    fn accounts_first_migration_preserves_asymmetric_routes_venues_and_session_pins() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, prefix_before("accounts_first")).unwrap();
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

        apply_sqlite(&conn).unwrap();

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
    fn project_successor_migration_preserves_history_and_child_references() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, &MIGRATIONS[..2]).unwrap();
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
             );
             INSERT INTO project_events (session_id, kind_json, created_at)
                 VALUES ('ps_old', '{\"kind\":\"completed\",\"summary\":\"history\"}', 2);",
        )
        .unwrap();

        apply_sqlite(&conn).unwrap();

        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM project_events WHERE session_id = 'ps_old'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        conn.execute_batch(
            "INSERT INTO project_sessions (
                 id, project_id, project_slug, project_name,
                 project_prompt_context, wave_id, pm_snapshot_synced_at,
                 status, status_reason, status_at, iteration,
                 observation_cursor, agent, provider, created_at, updated_at
             ) VALUES (
                 'ps_new', 'project-1', 'developer-efficiency',
                 'Developer Efficiency', 'Definition', 'w1', 3,
                 'created', 'successor', 3, 0, 0,
                 'codex', 'codex', 3, 3
             );",
        )
        .unwrap();
        assert!(conn
            .execute_batch(
                "INSERT INTO project_sessions (
                     id, project_id, project_slug, project_name,
                     project_prompt_context, wave_id, pm_snapshot_synced_at,
                     status, status_reason, status_at, iteration,
                     observation_cursor, agent, provider, created_at, updated_at
                 ) VALUES (
                     'ps_parallel', 'project-1', 'developer-efficiency',
                     'Developer Efficiency', 'Definition', 'w1', 3,
                     'created', 'parallel', 3, 0, 0,
                     'codex', 'codex', 3, 3
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
    fn existing_task_rows_become_sequence_one_prs_without_losing_events() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, &MIGRATIONS[..2]).unwrap();
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

        let session: (String, String) = conn
            .query_row(
                "SELECT status, workspace_slug FROM task_sessions WHERE id='ts_legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(session, ("completed".to_string(), "inf-123".to_string()));
        let project_lease: (String, String, Option<String>) = conn
            .query_row(
                "SELECT process_lease_token, process_lease_state, process_outcome_json
                 FROM project_sessions WHERE id='ps_legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert!(project_lease.0.starts_with("cl_"));
        assert_eq!(project_lease.1, "legacy");
        assert_eq!(project_lease.2, None);
        let task_lease: (String, String, String) = conn
            .query_row(
                "SELECT process_lease_token, process_lease_state, process_outcome_json
                 FROM task_sessions WHERE id='ts_legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert!(task_lease.0.starts_with("cl_"));
        assert_eq!(task_lease.1, "finished");
        assert_eq!(task_lease.2, "{\"kind\":\"completed\"}");
        let pr: (i64, String, i64, String, i64, String, String, Option<i64>) = conn
            .query_row(
                "SELECT sequence, branch, publication_requested_at, after_merge,
                        github_number, github_url, merge_commit, abandoned_at
                 FROM task_prs WHERE task_session_id='ts_legacy'",
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
            pr,
            (
                1,
                "jack/inf-123".to_string(),
                20,
                "complete_task".to_string(),
                101,
                "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
                "legacy-unknown".to_string(),
                None,
            )
        );
        let events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE session_id='ts_legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(events, 3);
        let event_shape: (String, String, String, String) = conn
            .query_row(
                "SELECT
                    kind_json,
                    json_extract(kind_json, '$.pr_id'),
                    (SELECT json_extract(kind_json, '$.from') FROM task_events
                     WHERE json_extract(kind_json, '$.kind')='status_changed'),
                    (SELECT json_extract(kind_json, '$.to') FROM task_events
                     WHERE json_extract(kind_json, '$.kind')='status_changed')
                 FROM task_events
                 WHERE json_extract(kind_json, '$.kind')='pr_opened'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        let event: TaskEventKind = serde_json::from_str(&event_shape.0).unwrap();
        assert!(matches!(event, TaskEventKind::PrOpened { sequence: 1, .. }));
        assert!(event_shape.1.starts_with("pr_"));
        assert_eq!(event_shape.2, "waiting");
        assert_eq!(event_shape.3, "completed");
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
    fn an_unknown_migration_reports_both_database_and_binary_evidence() {
        let conn = open();
        apply_set(&conn, &[baseline(), FIRST_IN_NEXT_MINOR]).unwrap();

        let error = apply_set(&conn, &[baseline()]).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("0.11.001_add_colour"), "{message}");
        assert!(
            message.contains("latest known 0.10.001_initial"),
            "{message}"
        );
        assert!(message.contains("run lf doctor"), "{message}");
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

        // The generated tail advances it, and applying it again is a no-op.
        apply_set(&conn, MIGRATIONS).unwrap();
        apply_set(&conn, MIGRATIONS).unwrap();

        assert_eq!(applied_versions(&conn).unwrap().len(), MIGRATIONS.len());
        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            latest_known_version()
        );
        validate_foreign_keys(&conn).unwrap();

        // The previous release's rows — including the parent/child foreign key —
        // survive the whole tail.
        let waves: i64 = conn
            .query_row("SELECT count(*) FROM waves", [], |row| row.get(0))
            .unwrap();
        assert_eq!(waves, 2, "seeded waves did not survive the upgrade");
        let child_parent: String = conn
            .query_row(
                "SELECT parent_wave_id FROM waves WHERE id = 'wave-child'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(child_parent, "wave-root", "foreign key relationship lost");
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
        apply_set(&conn, MIGRATIONS).unwrap();

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
        apply_set(&conn, &MIGRATIONS[..1]).unwrap();

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
    fn backup_snapshot_and_migration_share_one_exclusive_transaction() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_set(&conn, &MIGRATIONS[..1]).unwrap();
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
                    "INSERT INTO bus_messages (channel, byline, text, at)
                     VALUES ('test', 'writer', 'after migration', 1)",
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
                    "INSERT INTO bus_messages (channel, byline, text, at)
                     VALUES ('test', 'writer', 'after migration', 1)",
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
                "SELECT text FROM bus_messages WHERE byline = 'writer'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "after migration"
        );
        let backup = rusqlite::Connection::open(find_backup(
            directory.path(),
            "loopflow.db.backup-0.10.001_initial-",
        ))
        .unwrap();
        assert_eq!(
            backup
                .query_row("SELECT count(*) FROM bus_messages", [], |row| row
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

    /// What a process killed mid-migration leaves: the file keeps its header,
    /// the tables are gone. `SqliteStore::new` still routes it to the migrate
    /// path, because `existing_database` asks whether the file has bytes, not
    /// whether it holds a schema.
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
        assert!(store.list_run_events_since(0).unwrap().is_empty());
        let conn = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            latest_version_sqlite(&conn).unwrap(),
            latest_known_version()
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
