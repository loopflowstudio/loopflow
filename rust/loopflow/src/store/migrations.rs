//! Release-scoped schema migrations. See `MIGRATIONS.md` next to this file for
//! the convention; the one rule is that a shipped migration is never edited.

use std::collections::HashSet;
use std::fmt;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::store::sqlite::SQLITE_WRITE_BUSY_TIMEOUT;
use crate::store::{StoreError, StoreResult};
use fs2::FileExt;
use rusqlite::OptionalExtension;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

// -- Identity -----------------------------------------------------------------

/// A migration's identity: legacy `{major}.{minor}.{ordinal:03}` or release-scoped
/// `{major}.{minor}.{patch}.{ordinal:03}`.
///
/// New migrations carry the full package version of their release cut. Historical
/// three-part ids remain immutable and sort before release-scoped ids in the same
/// major/minor line. Ordering is numeric, never a string sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MigrationId {
    pub major: u32,
    pub minor: u32,
    pub patch: Option<u32>,
    pub ordinal: u32,
}

impl MigrationId {
    /// The id leading a canonical version string (`0.10.001_initial`), or `None`
    /// if the string does not carry a release-scoped id at all — which is how a
    /// ledger row from the pre-namespace era is told apart from a future release.
    fn parse_version(version: &str) -> Option<Self> {
        let (id, _name) = version.split_once('_')?;
        let numbers = id
            .split('.')
            .map(str::parse)
            .collect::<Result<Vec<u32>, _>>()
            .ok()?;
        match numbers.as_slice() {
            [major, minor, ordinal] => Some(MigrationId {
                major: *major,
                minor: *minor,
                patch: None,
                ordinal: *ordinal,
            }),
            [major, minor, patch, ordinal] => Some(MigrationId {
                major: *major,
                minor: *minor,
                patch: Some(*patch),
                ordinal: *ordinal,
            }),
            _ => None,
        }
    }
}

impl fmt::Display for MigrationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.patch {
            Some(patch) => write!(
                f,
                "{}.{}.{}.{:03}",
                self.major, self.minor, patch, self.ordinal
            ),
            None => write!(f, "{}.{}.{:03}", self.major, self.minor, self.ordinal),
        }
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

/// Every canonical migration, in id order. Release cuts append one generated
/// batch after topologically ordering the ordinal-free drafts.
const MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            patch: None,
            ordinal: 1,
        },
        name: "initial",
        sql: include_str!("migrations/0.10.001_initial.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            patch: None,
            ordinal: 2,
        },
        name: "session_execution_context",
        sql: include_str!("migrations/0.10.002_session_execution_context.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 1,
        },
        name: "task_prs",
        sql: include_str!("migrations/0.11.001_task_prs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 2,
        },
        name: "project_session_successors",
        sql: include_str!("migrations/0.11.002_project_session_successors.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 3,
        },
        name: "child_body_lease",
        sql: include_str!("migrations/0.11.003_child_body_lease.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 4,
        },
        name: "task_pr_ci_state",
        sql: include_str!("migrations/0.11.004_task_pr_ci_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 5,
        },
        name: "provider_accounts",
        sql: include_str!("migrations/0.11.005_provider_accounts.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 6,
        },
        name: "context_launch_work",
        sql: include_str!("migrations/0.11.006_context_launch_work.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 7,
        },
        name: "task_pr_parent",
        sql: include_str!("migrations/0.11.007_task_pr_parent.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 8,
        },
        name: "interactive_handoffs",
        sql: include_str!("migrations/0.11.008_interactive_handoffs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 9,
        },
        name: "context_pressure",
        sql: include_str!("migrations/0.11.009_context_pressure.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 10,
        },
        name: "context_input_normalization",
        sql: include_str!("migrations/0.11.010_context_input_normalization.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 11,
        },
        name: "profiles",
        sql: include_str!("migrations/0.11.011_profiles.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 12,
        },
        name: "provider_account_lifecycle",
        sql: include_str!("migrations/0.11.012_provider_account_lifecycle.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 13,
        },
        name: "task_review_state",
        sql: include_str!("migrations/0.11.013_task_review_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 14,
        },
        name: "task_lifecycle",
        sql: include_str!("migrations/0.11.014_task_lifecycle.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 15,
        },
        name: "interaction_reviews",
        sql: include_str!("migrations/0.11.015_interaction_reviews.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 16,
        },
        name: "task_linear_observations",
        sql: include_str!("migrations/0.11.016_task_linear_observations.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 17,
        },
        name: "migration_provenance",
        sql: include_str!("migrations/0.11.017_migration_provenance.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 18,
        },
        name: "session_body_provenance",
        sql: include_str!("migrations/0.11.018_session_body_provenance.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 19,
        },
        name: "task_pr_github_observation",
        sql: include_str!("migrations/0.11.019_task_pr_github_observation.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 20,
        },
        name: "task_pr_linear_linkage",
        sql: include_str!("migrations/0.11.020_task_pr_linear_linkage.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 21,
        },
        name: "provider_deliveries",
        sql: include_str!("migrations/0.11.021_provider_deliveries.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 22,
        },
        name: "task_session_successors",
        sql: include_str!("migrations/0.11.022_task_session_successors.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 23,
        },
        name: "capture_pruned_state",
        sql: include_str!("migrations/0.11.023_capture_pruned_state.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 24,
        },
        name: "ci_incidents",
        sql: include_str!("migrations/0.11.024_ci_incidents.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 25,
        },
        name: "usage_deltas",
        sql: include_str!("migrations/0.11.025_usage_deltas.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 26,
        },
        name: "lineage_boundary",
        sql: include_str!("migrations/0.11.026_lineage_boundary.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 27,
        },
        name: "accounts_first",
        sql: include_str!("migrations/0.11.027_accounts_first.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 29,
        },
        name: "ci_incident_repaired_head",
        sql: include_str!("migrations/0.11.029_ci_incident_repaired_head.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 30,
        },
        name: "one_spend_grain",
        sql: include_str!("migrations/0.11.030_one_spend_grain.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 31,
        },
        name: "durable_input_spine",
        sql: include_str!("migrations/0.11.031_durable_input_spine.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 32,
        },
        name: "run_launch_attention",
        sql: include_str!("migrations/0.11.032_run_launch_attention.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 33,
        },
        name: "launch_attention_only",
        sql: include_str!("migrations/0.11.033_launch_attention_only.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 34,
        },
        name: "typed_ci_runs",
        sql: include_str!("migrations/0.11.034_typed_ci_runs.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 35,
        },
        name: "drop_child_commands",
        sql: include_str!("migrations/0.11.035_drop_child_commands.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 36,
        },
        name: "delete_sessions",
        sql: include_str!("migrations/0.11.036_delete_sessions.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 37,
        },
        name: "capture_terminal_states",
        sql: include_str!("migrations/0.11.037_capture_terminal_states.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(2),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.2.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(3),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.3.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(4),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.4.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(5),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.5.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(7),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.7.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(8),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.8.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(10),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.10.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(12),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.12.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(13),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.13.001_release.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 12,
            patch: Some(14),
            ordinal: 1,
        },
        name: "release",
        sql: include_str!("migrations/0.12.14.001_release.sql"),
    },
];

#[doc(hidden)]
pub fn migration_sql_for_test(name: &str) -> String {
    let marker = format!("-- draft: {name}");
    if let Some(migration) = MIGRATIONS
        .iter()
        .find(|migration| migration.sql.lines().any(|line| line == marker))
    {
        return migration.sql.to_string();
    }

    let drafts =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/store/migrations/drafts");
    let prefix = format!("{name}__");
    let path = std::fs::read_dir(drafts)
        .expect("migration draft directory")
        .map(|entry| entry.expect("migration draft entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".sql"))
        })
        .expect("migration is canonical or present as an ordinal-free draft");
    std::fs::read_to_string(path).expect("migration SQL")
}

#[cfg(test)]
pub(crate) fn migration_is_applied_for_test(
    conn: &rusqlite::Connection,
    name: &str,
) -> StoreResult<bool> {
    let marker = format!("-- draft: {name}");
    let Some(migration) = MIGRATIONS
        .iter()
        .find(|migration| migration.sql.lines().any(|line| line == marker))
    else {
        return Ok(false);
    };
    Ok(applied_versions(conn)?
        .iter()
        .any(|version| version == &migration.version()))
}

/// The exact branch-local history that reached one production ledger before
/// main established `0.11.008_interactive_handoffs`. These ids were never
/// released. They remain here only long enough to recognize and converge that
/// known history without treating arbitrary unknown migrations as ours.
const DIVERGENT_MIGRATIONS: &[Migration] = &[
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 8,
        },
        name: "context_pressure",
        sql: include_str!("migrations/0.11.009_context_pressure.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 9,
        },
        name: "context_input_normalization",
        sql: include_str!("migrations/0.11.010_context_input_normalization.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 10,
        },
        name: "profiles",
        sql: include_str!("migrations/0.11.011_profiles.sql"),
    },
    Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
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
const DEVELOPMENT_MIGRATIONS_TABLE: &str = "development_migrations";

/// The package version a migration release cut belongs to, from the single
/// source of truth (the workspace `Cargo.toml`, via Cargo).
///
/// # Panics
///
/// Panics if the package version is not `major.minor.patch`, which Cargo rejects
/// long before this runs.
pub fn active_namespace() -> (u32, u32, u32) {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    let major = parts.next().and_then(|part| part.parse().ok());
    let minor = parts.next().and_then(|part| part.parse().ok());
    let patch = parts.next().and_then(|part| part.parse().ok());
    match (major, minor, patch, parts.next()) {
        (Some(major), Some(minor), Some(patch), None) => (major, minor, patch),
        _ => panic!("package version {version} is not major.minor.patch"),
    }
}

// -- Applying -----------------------------------------------------------------

pub fn apply_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    apply_sqlite_transaction(conn, |_| Ok(()))
}

/// Apply the exact draft manifest embedded in an installed development build.
///
/// Drafts are durable only in the disposable installed-development store. The
/// release ledger remains untouched, and reuse accepts only an exact applied
/// prefix so edited, removed, or reordered SQL requires an explicit fresh fork.
pub(crate) fn apply_installed_development_sqlite(
    conn: &rusqlite::Connection,
    drafts: &[crate::build_info::MigrationDraft],
) -> StoreResult<()> {
    _validate_draft_manifest(drafts)?;
    let has_draft_ledger = user_tables(conn)?
        .iter()
        .any(|table| table == DEVELOPMENT_MIGRATIONS_TABLE);
    if has_draft_ledger {
        _validate_canonical_history_for_development(conn)?;
    } else {
        apply_sqlite(conn)?;
    }

    let applied = _applied_development_migrations(conn)?;
    _validate_applied_draft_prefix(&applied, drafts)?;
    _validate_development_schema(conn, &drafts[..applied.len()])?;
    if applied.len() == drafts.len() {
        return Ok(());
    }

    let foreign_keys_enabled: bool =
        conn.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let result = match conn.execute_batch("BEGIN EXCLUSIVE") {
        Ok(()) => {
            let result = (|| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS development_migrations (
                         position INTEGER NOT NULL UNIQUE,
                         id TEXT PRIMARY KEY,
                         name TEXT NOT NULL UNIQUE,
                         checksum TEXT NOT NULL,
                         applied_at INTEGER NOT NULL
                     );",
                )?;
                for (position, draft) in drafts.iter().enumerate().skip(applied.len()) {
                    conn.execute_batch(draft.sql)?;
                    conn.execute(
                        "INSERT INTO development_migrations (
                             position, id, name, checksum, applied_at
                         ) VALUES (?1, ?2, ?3, ?4, unixepoch())",
                        (position as i64, draft.id, draft.name, draft.checksum),
                    )?;
                }
                validate_foreign_keys(conn)?;
                _validate_development_schema(conn, drafts)
            })();
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
    let restore = if foreign_keys_enabled {
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(StoreError::from)
    } else {
        Ok(())
    };
    match result {
        Err(error) => Err(error),
        Ok(()) => restore,
    }
}

pub(crate) fn validate_installed_development_sqlite(
    conn: &rusqlite::Connection,
    drafts: &[crate::build_info::MigrationDraft],
) -> StoreResult<()> {
    _validate_draft_manifest(drafts)?;
    _validate_canonical_history_for_development(conn)?;
    let applied = _applied_development_migrations(conn)?;
    _validate_applied_draft_prefix(&applied, drafts)?;
    if applied.len() != drafts.len() {
        return Err(_incompatible_development_store(format!(
            "store has {} applied draft(s), candidate requires {}",
            applied.len(),
            drafts.len()
        )));
    }
    _validate_development_schema(conn, drafts)?;
    validate_foreign_keys(conn)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedDevelopmentMigration {
    id: String,
    name: String,
    checksum: String,
}

fn _validate_draft_manifest(drafts: &[crate::build_info::MigrationDraft]) -> StoreResult<()> {
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let draft_names = drafts
        .iter()
        .map(|draft| draft.name)
        .collect::<HashSet<_>>();
    for draft in drafts {
        if !ids.insert(draft.id) || !names.insert(draft.name) {
            return Err(StoreError::InvalidData(format!(
                "installed development draft manifest repeats {}",
                draft.name
            )));
        }
        for dependency in draft.dependencies {
            if draft_names.contains(dependency) && !names.contains(dependency) {
                return Err(StoreError::InvalidData(format!(
                    "installed development draft {} precedes dependency {}",
                    draft.name, dependency
                )));
            }
        }
        let checksum = hex::encode(Sha256::digest(draft.sql.as_bytes()));
        if checksum != draft.checksum {
            return Err(StoreError::InvalidData(format!(
                "installed development draft {} checksum does not match its SQL",
                draft.name
            )));
        }
    }
    Ok(())
}

fn _validate_canonical_history_for_development(conn: &rusqlite::Connection) -> StoreResult<()> {
    validate_set(MIGRATIONS).map_err(StoreError::InvalidData)?;
    if !user_tables(conn)?
        .iter()
        .any(|table| table == "schema_migrations")
    {
        return Err(_incompatible_development_store(
            "canonical migration ledger is missing".to_string(),
        ));
    }
    let applied = applied_versions(conn)?;
    let pending = pending_migrations(&applied, MIGRATIONS)?;
    if let Some(next) = pending.first() {
        return Err(_incompatible_development_store(format!(
            "canonical frontier changed before {}",
            next.version()
        )));
    }
    validate_applied_checksums(conn, MIGRATIONS)
}

fn _applied_development_migrations(
    conn: &rusqlite::Connection,
) -> StoreResult<Vec<AppliedDevelopmentMigration>> {
    if !user_tables(conn)?
        .iter()
        .any(|table| table == DEVELOPMENT_MIGRATIONS_TABLE)
    {
        return Ok(Vec::new());
    }
    let mut statement = conn.prepare(
        "SELECT id, name, checksum
         FROM development_migrations
         ORDER BY position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(AppliedDevelopmentMigration {
            id: row.get(0)?,
            name: row.get(1)?,
            checksum: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn _validate_applied_draft_prefix(
    applied: &[AppliedDevelopmentMigration],
    drafts: &[crate::build_info::MigrationDraft],
) -> StoreResult<()> {
    if applied.len() > drafts.len() {
        return Err(_incompatible_development_store(
            "candidate removed or canonicalized an applied draft".to_string(),
        ));
    }
    for (position, (applied, draft)) in applied.iter().zip(drafts).enumerate() {
        if applied.id != draft.id
            || applied.name != draft.name
            || applied.checksum != draft.checksum
        {
            return Err(_incompatible_development_store(format!(
                "applied draft at position {position} no longer matches {}",
                draft.name
            )));
        }
    }
    Ok(())
}

fn _validate_development_schema(
    conn: &rusqlite::Connection,
    drafts: &[crate::build_info::MigrationDraft],
) -> StoreResult<()> {
    let expected = rusqlite::Connection::open_in_memory()?;
    expected.execute_batch(
        "CREATE TABLE schema_migrations (
             version TEXT PRIMARY KEY,
             applied_at INTEGER NOT NULL
         );",
    )?;
    for migration in MIGRATIONS {
        expected.execute_batch(migration.sql)?;
    }
    for draft in drafts {
        expected.execute_batch(draft.sql)?;
    }
    if product_schema(conn)? != product_schema(&expected)? {
        return Err(_incompatible_development_store(
            "schema does not match the applied draft prefix".to_string(),
        ));
    }
    Ok(())
}

fn _incompatible_development_store(reason: String) -> StoreError {
    StoreError::InvalidData(format!(
        "installed development store is incompatible ({reason}); rerun local promotion with --fresh"
    ))
}

/// Stage a fresh connection one migration behind the binary's known head. The
/// store-level shared-frontier regressions use it to build a database the running
/// binary could advance but an ordinary open must leave alone.
#[cfg(test)]
pub(crate) fn apply_all_but_head(conn: &rusqlite::Connection) -> StoreResult<()> {
    apply_set(conn, &MIGRATIONS[..MIGRATIONS.len() - 1])
}

#[cfg(test)]
pub(crate) fn prior_known_version() -> String {
    MIGRATIONS[MIGRATIONS.len() - 2].version()
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
                .and_then(|()| validate_foreign_keys(conn))
                .and_then(|()| validate_persisted_json(conn));
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

pub(crate) fn validate_persisted_json(conn: &rusqlite::Connection) -> StoreResult<()> {
    let mut failures = validate_json_column::<crate::task::PmWritebackState>(
        conn,
        "tasks",
        "id",
        "pm_writeback_json",
    )?;
    failures.extend(validate_json_column::<crate::task::TaskGateProposal>(
        conn,
        "tasks",
        "id",
        "gate_proposal_json",
    )?);
    failures.extend(validate_json_column::<crate::task::CiObservation>(
        conn,
        "task_prs",
        "id",
        "ci_observation",
    )?);
    failures.extend(validate_json_column::<crate::task::GithubObservation>(
        conn,
        "task_prs",
        "id",
        "github_observation",
    )?);
    failures.extend(validate_json_column::<crate::task::TaskEventKind>(
        conn,
        "task_events",
        "id",
        "kind_json",
    )?);
    failures.extend(validate_json_column::<crate::project::ProjectEventKind>(
        conn,
        "project_events",
        "id",
        "kind_json",
    )?);
    failures.extend(validate_json_column::<crate::project::ChildEventPayload>(
        conn,
        "observation_outbox",
        "id",
        "payload_json",
    )?);
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

pub(crate) fn apply_sqlite_with_backup(
    conn: &rusqlite::Connection,
    path: &Path,
) -> StoreResult<()> {
    conn.busy_timeout(SQLITE_WRITE_BUSY_TIMEOUT)?;
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
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
           AND name NOT IN ('schema_migrations', 'development_migrations')
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
        active_namespace, applied_versions, apply_installed_development_sqlite, apply_set,
        apply_sqlite, apply_sqlite_transaction, apply_sqlite_with_backup, backup_before_migration,
        latest_applied_version_sqlite, latest_known_version, latest_version_sqlite,
        migration_checksum, migration_sql_for_test, pending_migrations, product_schema,
        validate_foreign_keys, validate_installed_development_sqlite, validate_persisted_json,
        validate_set, validate_sqlite, Migration, MigrationId, DIVERGENT_MIGRATIONS, MIGRATIONS,
    };

    const REOPEN_REPAIR_NAME: &str = "retire_obsolete_pm_reopen_writebacks";
    const GATE_PROPOSAL_REPAIR_NAME: &str = "repair_legacy_task_gate_proposals";
    const LEGACY_TASK_FLOW_REPAIR_NAME: &str = "repair_legacy_task_flow";
    const TURN_USAGE_SAMPLES_NAME: &str = "add_turn_usage_samples";
    const REPOSITORY_OWNED_WAVES_NAME: &str = "repository_owned_waves";
    const REMOVE_TASK_LIFECYCLE_OUTCOME_NAME: &str = "remove_task_lifecycle_outcome";
    const RUN_IDENTITY_NAME: &str = "run_identity";

    fn _draft_is_canonical(name: &str) -> bool {
        let marker = format!("-- draft: {name}");
        MIGRATIONS
            .iter()
            .any(|migration| migration.sql.lines().any(|line| line == marker.as_str()))
    }

    /// Stand-ins for the releases that have not happened yet: one more migration
    /// in the baseline's minor, and the first of the next minor.
    const SECOND_IN_SAME_MINOR: Migration = Migration {
        id: MigrationId {
            major: 0,
            minor: 10,
            patch: None,
            ordinal: 2,
        },
        name: "add_note",
        sql: "ALTER TABLE waves ADD COLUMN note TEXT;",
    };
    const FIRST_IN_NEXT_MINOR: Migration = Migration {
        id: MigrationId {
            major: 0,
            minor: 11,
            patch: None,
            ordinal: 1,
        },
        name: "add_colour",
        sql: "ALTER TABLE waves ADD COLUMN colour TEXT;",
    };

    fn open() -> rusqlite::Connection {
        rusqlite::Connection::open_in_memory().unwrap()
    }

    fn development_draft(
        id: &'static str,
        name: &'static str,
        dependencies: &'static [&'static str],
        sql: &'static str,
    ) -> crate::build_info::MigrationDraft {
        use sha2::{Digest, Sha256};

        crate::build_info::MigrationDraft {
            id,
            name,
            dependencies,
            sql,
            checksum: Box::leak(hex::encode(Sha256::digest(sql.as_bytes())).into_boxed_str()),
        }
    }

    #[test]
    fn installed_development_store_appends_only_an_exact_draft_prefix() {
        let conn = open();
        let first = development_draft(
            "11111111111111111111111111111111",
            "local_feature",
            &[],
            "CREATE TABLE local_feature (id TEXT PRIMARY KEY);",
        );
        apply_installed_development_sqlite(&conn, std::slice::from_ref(&first)).unwrap();
        validate_installed_development_sqlite(&conn, std::slice::from_ref(&first)).unwrap();

        let second = development_draft(
            "22222222222222222222222222222222",
            "extend_local_feature",
            &["local_feature"],
            "ALTER TABLE local_feature ADD COLUMN note TEXT;",
        );
        apply_installed_development_sqlite(&conn, &[first.clone(), second.clone()]).unwrap();
        validate_installed_development_sqlite(&conn, &[first.clone(), second]).unwrap();
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM development_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(applied, 2);

        let changed = development_draft(
            first.id,
            first.name,
            &[],
            "CREATE TABLE local_feature (id TEXT PRIMARY KEY, changed TEXT);",
        );
        let error = apply_installed_development_sqlite(&conn, &[changed]).unwrap_err();
        assert!(error.to_string().contains("--fresh"));
    }

    #[test]
    fn installed_development_draft_accepts_a_released_dependency() {
        let conn = open();
        let draft = development_draft(
            "33333333333333333333333333333333",
            "local_after_released",
            &[REOPEN_REPAIR_NAME],
            "CREATE TABLE local_after_released (id TEXT PRIMARY KEY);",
        );

        apply_installed_development_sqlite(&conn, &[draft]).unwrap();
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

    #[test]
    fn the_shipped_set_is_ordered_and_within_the_active_namespace() {
        validate_set(MIGRATIONS).unwrap();

        let active = active_namespace();
        for migration in MIGRATIONS {
            let namespace = (
                migration.id.major,
                migration.id.minor,
                migration.id.patch.unwrap_or(0),
            );
            assert!(
                namespace <= active,
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
            .any(|column| { matches!(column.as_str(), "status" | "status_reason" | "status_at") }));
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
            .position(|migration| migration.name == name)
            .expect("named migration is registered");
        &MIGRATIONS[..index]
    }

    fn apply_through(conn: &rusqlite::Connection, name: &str) {
        let index = MIGRATIONS
            .iter()
            .position(|migration| migration.name == name)
            .expect("named migration is registered");
        apply_set(conn, &MIGRATIONS[..=index]).unwrap();
    }

    fn draft_location(name: &str) -> (usize, &'static str, usize) {
        let marker = format!("-- draft: {name}");
        MIGRATIONS
            .iter()
            .enumerate()
            .find_map(|(index, migration)| {
                migration
                    .sql
                    .find(&marker)
                    .map(|offset| (index, migration.sql, offset))
            })
            .unwrap_or_else(|| panic!("draft {name} is materialized in a release migration"))
    }

    fn apply_before_draft(conn: &rusqlite::Connection, name: &str) {
        let (migration_index, sql, draft_offset) = draft_location(name);
        apply_set(conn, &MIGRATIONS[..migration_index]).unwrap();
        conn.execute_batch(&sql[..draft_offset]).unwrap();
    }

    fn apply_draft(conn: &rusqlite::Connection, name: &str) {
        let (_, sql, draft_offset) = draft_location(name);
        let body_start = sql[draft_offset..]
            .find('\n')
            .map(|offset| draft_offset + offset + 1)
            .unwrap_or(sql.len());
        let body_end = sql[body_start..]
            .find("\n-- draft: ")
            .map(|offset| body_start + offset)
            .unwrap_or(sql.len());
        conn.execute_batch(&sql[body_start..body_end]).unwrap();
    }

    fn apply_before_current_draft(conn: &rusqlite::Connection, name: &str) {
        if _draft_is_canonical(name) {
            apply_before_draft(conn, name);
        } else {
            apply_set(conn, MIGRATIONS).unwrap();
        }
    }

    fn current_draft_sql(name: &str) -> String {
        if !_draft_is_canonical(name) {
            return migration_sql_for_test(name);
        }
        let (_, sql, draft_offset) = draft_location(name);
        let body_start = sql[draft_offset..]
            .find('\n')
            .map(|offset| draft_offset + offset + 1)
            .unwrap_or(sql.len());
        let body_end = sql[body_start..]
            .find("\n-- draft: ")
            .map(|offset| body_start + offset)
            .unwrap_or(sql.len());
        sql[body_start..body_end].to_string()
    }

    fn apply_current_work_schema(conn: &rusqlite::Connection) {
        if columns(conn, "tasks").contains(&"work_state".to_string()) {
            return;
        }
        let foreign_keys: bool = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        for draft in [
            "generic_ask_run_claim",
            "opaque_steer_run_provenance",
            "stable_work_state",
            "obsolete_sql_lifecycle",
        ] {
            conn.execute_batch(&current_draft_sql(draft)).unwrap();
        }
        if foreign_keys {
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        }
    }

    #[test]
    fn stable_work_draft_keeps_definitions_and_clears_execution_state() {
        let conn = open();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_before_current_draft(&conn, "stable_work_state");
        if !columns(&conn, "ask_exchanges").contains(&"source_run_id".to_string()) {
            for draft in ["generic_ask_run_claim", "opaque_steer_run_provenance"] {
                conn.execute_batch(&current_draft_sql(draft)).unwrap();
            }
        }
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
             VALUES ('wave_keep', 'keep', '/repo', 1700000000, NULL)",
            [],
        )
        .unwrap();

        conn.execute_batch(&current_draft_sql("stable_work_state"))
            .unwrap();

        let state: (String, Option<i64>) = conn
            .query_row(
                "SELECT work_state, work_terminal_at FROM waves WHERE id='wave_keep'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, ("ready".to_string(), None));
        for table in ["waits", "work_truth", "epoch_revisions"] {
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
        for table in [
            "steers",
            "tool_responses",
            "work_flow_positions",
            "ask_exchanges",
        ] {
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
        apply_before_current_draft(&conn, "obsolete_sql_lifecycle");
        if !columns(&conn, "tasks").contains(&"work_state".to_string()) {
            for draft in [
                "generic_ask_run_claim",
                "opaque_steer_run_provenance",
                "stable_work_state",
            ] {
                conn.execute_batch(&current_draft_sql(draft)).unwrap();
            }
        }
        conn.execute(
            "INSERT INTO run_events (run_id, process_id, seq, ts, node, event)
             VALUES ('trace-keep', 'exec-keep', 0, 1, 'run', 'started')",
            [],
        )
        .unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        conn.execute_batch(&current_draft_sql("obsolete_sql_lifecycle"))
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
        apply_before_current_draft(&conn, REMOVE_TASK_LIFECYCLE_OUTCOME_NAME);
        assert!(columns(&conn, "tasks")
            .iter()
            .any(|column| column == "lifecycle_outcome"));

        conn.execute_batch(&current_draft_sql(REMOVE_TASK_LIFECYCLE_OUTCOME_NAME))
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
        apply_before_current_draft(&conn, RUN_IDENTITY_NAME);
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&current_draft_sql(RUN_IDENTITY_NAME))
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
        apply_before_current_draft(&conn, TURN_USAGE_SAMPLES_NAME);
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
        conn.execute_batch(&current_draft_sql(TURN_USAGE_SAMPLES_NAME))
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
        apply_before_current_draft(&conn, REPOSITORY_OWNED_WAVES_NAME);
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
        conn.execute_batch(&current_draft_sql(REPOSITORY_OWNED_WAVES_NAME))
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
        apply_before_current_draft(&conn, REPOSITORY_OWNED_WAVES_NAME);
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
            .execute_batch(&current_draft_sql(REPOSITORY_OWNED_WAVES_NAME))
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
    fn validation_only_open_does_not_apply_an_unpublished_tail() {
        let conn = open();
        let published = prefix_before("durable_input_spine");
        apply_set(&conn, published).unwrap();

        validate_sqlite(&conn).unwrap();

        assert_eq!(
            latest_applied_version_sqlite(&conn).unwrap(),
            published.last().map(Migration::version),
            "a validation-only open advances nothing"
        );
        assert!(capture_status_accepts(&conn, "pruned"));
        // Bait for the withheld tail: the durable input migration creates the
        // stable Work tables. Their absence proves validation stayed read-only.
        assert!(
            conn.prepare("SELECT id FROM projects LIMIT 0").is_err(),
            "a validation-only open must not run the tail's schema change"
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
        apply_set(&conn, installed).unwrap();
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
        apply_set(&conn, prefix_before("capture_terminal_states")).unwrap();
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
        apply_set(&conn, prefix_before("run_launch_attention")).unwrap();
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

        apply_set(&conn, prefix_before("capture_terminal_states")).unwrap();
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
    fn project_successor_migration_allows_one_current_successor() {
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
             );",
        )
        .unwrap();

        apply_set(&conn, &MIGRATIONS[..4]).unwrap();
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
    fn existing_task_rows_collapse_into_stable_work_and_prs() {
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
        apply_current_work_schema(&conn);

        let task: (String, String, String, String) = conn
            .query_row(
                "SELECT id, issue_identifier, workspace_slug, work_state
                 FROM tasks WHERE external_issue_id='issue-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(task.1, "INF-123");
        assert_eq!(task.2, "inf-123");
        assert_eq!(task.3, "ready");
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
    fn legacy_persisted_json_upgrades_to_typed_stable_tasks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_set(&conn, &MIGRATIONS[..2]).unwrap();
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
        apply_set(&conn, prefix_before("interaction_reviews")).unwrap();
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

        conn.execute_batch("BEGIN EXCLUSIVE").unwrap();
        apply_set(&conn, MIGRATIONS).unwrap();
        conn.execute_batch("COMMIT").unwrap();
        let stale: String = conn
            .query_row(
                "SELECT pm_writeback_json FROM tasks WHERE external_issue_id='issue-reopen'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        if !_draft_is_canonical(REOPEN_REPAIR_NAME) {
            assert!(stale.contains("reopen_task"));
            conn.execute_batch(&migration_sql_for_test(REOPEN_REPAIR_NAME))
                .unwrap();
        }
        if !_draft_is_canonical(GATE_PROPOSAL_REPAIR_NAME) {
            conn.execute_batch(&migration_sql_for_test(GATE_PROPOSAL_REPAIR_NAME))
                .unwrap();
        }
        assert_eq!(
            conn.query_row(
                "SELECT gate_proposal_json FROM tasks WHERE external_issue_id='issue-current'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "{\"done\":false,\"reason\":\"current\",\"future\":\"preserved\"}"
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM tasks
                 WHERE json_type(gate_proposal_json, '$.status') IS NOT NULL",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
        validate_foreign_keys(&conn).unwrap();
        validate_persisted_json(&conn).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();

        let error = apply_sqlite_transaction(&conn, |conn| {
            conn.execute(
                "UPDATE tasks
                 SET pm_writeback_json='{\"state\":\"pending\",\"operation\":\"invented\",\"error\":\"offline\"}'
                 WHERE external_issue_id='issue-reopen'",
                [],
            )?;
            conn.execute(
                "UPDATE tasks
                 SET gate_proposal_json='{\"reason\":\"missing done\"}'
                 WHERE external_issue_id='issue-completed'",
                [],
            )?;
            Ok(())
        })
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("tasks.pm_writeback_json"));
        assert!(message.contains("tasks.gate_proposal_json"));
        assert_eq!(
            conn.query_row(
                "SELECT pm_writeback_json FROM tasks WHERE external_issue_id='issue-reopen'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "{\"state\":\"current\"}"
        );
        drop(conn);

        let store = crate::store::sqlite::SqliteStore::new(&path).unwrap();
        store.health_check().unwrap();
        let tasks = store.list_tasks(None).unwrap();
        assert_eq!(tasks.len(), 6);
        let reopen = tasks
            .iter()
            .find(|task| task.plan.identifier == "INF-REOPEN")
            .unwrap();
        assert!(matches!(
            reopen.pm_writeback,
            crate::task::PmWritebackState::Current
        ));
        let mut decisions = tasks
            .iter()
            .map(|task| task.gate_proposal.as_ref().unwrap().done)
            .collect::<Vec<_>>();
        decisions.sort_unstable();
        assert_eq!(decisions, vec![false, false, false, false, false, true]);
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
        conn.execute_batch("ALTER TABLE tasks DROP COLUMN issue_description")
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
        validate_installed_development_sqlite(&conn, crate::build_info::migration_draft_manifest())
            .unwrap();
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
            patch: None,
            ordinal,
        };
        assert!(id(0, 9, 1) < id(0, 10, 1));
        assert!(id(0, 10, 2) < id(0, 11, 1));
        assert_eq!(id(0, 10, 1).to_string(), "0.10.001");

        let release = MigrationId {
            major: 0,
            minor: 12,
            patch: Some(2),
            ordinal: 1,
        };
        assert!(id(0, 12, 37) < release);
        assert_eq!(release.to_string(), "0.12.2.001");
    }

    #[test]
    fn a_skipped_patch_is_part_of_the_pending_release_suffix() {
        let releases = [
            Migration {
                id: MigrationId {
                    major: 1,
                    minor: 1,
                    patch: Some(0),
                    ordinal: 1,
                },
                name: "release",
                sql: "SELECT 1;",
            },
            Migration {
                id: MigrationId {
                    major: 1,
                    minor: 1,
                    patch: Some(1),
                    ordinal: 1,
                },
                name: "release",
                sql: "SELECT 2;",
            },
            Migration {
                id: MigrationId {
                    major: 1,
                    minor: 2,
                    patch: Some(0),
                    ordinal: 1,
                },
                name: "release",
                sql: "SELECT 3;",
            },
        ];
        let applied = [releases[0].version()];

        let pending = pending_migrations(&applied, &releases).unwrap();

        assert_eq!(
            pending.iter().map(Migration::version).collect::<Vec<_>>(),
            ["1.1.1.001_release", "1.2.0.001_release"]
        );
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
    fn stable_work_schema_keeps_plural_asks_without_execution_authority() {
        let conn = open();
        apply_sqlite(&conn).unwrap();
        apply_current_work_schema(&conn);

        assert!(!conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type='table' AND name='agent_invocations'
                )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
        let task_columns = columns(&conn, "tasks");
        for deleted in ["kickoff_reviewer", "iterate_reviewer", "gate_reviewer"] {
            assert!(!task_columns.contains(&deleted.to_string()));
        }
        let position_columns = columns(&conn, "work_flow_positions");
        for present in ["work_kind", "work_id", "node_id", "human"] {
            assert!(position_columns.contains(&present.to_string()));
        }
        for deleted in ["epoch_id", "interactive"] {
            assert!(!position_columns.contains(&deleted.to_string()));
        }

        let ask_columns = columns(&conn, "ask_exchanges");
        assert!(ask_columns.contains(&"source_run_id".to_string()));
        for deleted in [
            "epoch_id",
            "origin_run_id",
            "origin_turn_id",
            "origin_invocation_id",
        ] {
            assert!(!ask_columns.contains(&deleted.to_string()));
        }

        conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        conn.execute_batch(
            "INSERT INTO ask_exchanges (
                id, origin_work_kind, origin_work_id, source_run_id,
                origin_home_id, origin_cwd,
                target_kind, target_work_kind, target_work_id,
                request_kind, request_prompt, state, asked_at
             ) VALUES
             (
                'ask_one', 'task', 'task_one', 'run_one', 'home_one', '/repo',
                'parent', 'project', 'proj_parent',
                'intervention', 'Which proof?', 'queued', 1
             ),
             (
                'ask_two', 'task', 'task_one', 'run_one', 'home_one', '/repo',
                'user', NULL, NULL,
                'intervention', 'Another proof?', 'queued', 2
             )",
        )
        .unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM ask_exchanges
                 WHERE state='queued' AND source_run_id='run_one'",
                [],
                |row| row.get::<_, i64>(0),
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
    fn generic_ask_run_claim_keeps_history_and_removes_invocation_authority() {
        let conn = open();
        apply_before_current_draft(&conn, "generic_ask_run_claim");
        conn.execute_batch(
            "INSERT INTO waves (id, name, repo, created_at)
                 VALUES ('wave_ask', 'ask', '/repo', 1);
             INSERT INTO projects (id, wave_id, external_project_id, created_at)
                 VALUES ('project_ask', 'wave_ask', 'linear-ask', 1);
             INSERT INTO epochs (
                 id, number, wave_id, project_id, task_id, state, current_rev,
                 created_at, terminal_at
             ) VALUES ('epoch_ask', 1, NULL, 'project_ask', NULL, 'open', 0, 1, NULL);
             INSERT INTO runs (
                 id, epoch_id, home_id, state, trigger_json, retry_of,
                 runtime_generation, source_kind, source_id, created_at,
                 ended_at, stop_reason, containment_kind, containment_id, cwd,
                 started_at
             ) VALUES (
                 'run_source_bytes', 'epoch_ask', (SELECT id FROM homes LIMIT 1),
                 'ended', '{\"kind\":\"user\"}', NULL, NULL, 'project',
                 'project_ask', 1, 2, NULL, NULL, NULL, NULL, NULL
             );
             INSERT INTO ask_exchanges (
                 id, epoch_id, origin_work_kind, origin_work_id, origin_run_id,
                 origin_turn_id, origin_invocation_id, origin_home_id, origin_cwd,
                 target_kind, request_kind, request_prompt, state, asked_at
             ) VALUES (
                 'ask_generic', 'epoch_ask', 'project', 'project_ask',
                 'run_source_bytes', NULL, NULL, (SELECT id FROM homes LIMIT 1),
                 '/repo', 'user', 'intervention', 'Recover safely', 'queued', 2
             );
             INSERT INTO agent_invocations (
                 id, run_id, process_id, started_at, repo, worktree, provider,
                 surface, capture_status, outcome, artifact_dir,
                 conversation_path, conversation_event_count, conversation_bytes,
                 supervising_run_id, answer_ask_id, ask_ready_at, ask_presented_at
             ) VALUES (
                 'invocation_ask', 'trace_ask', 'process_ask', 3, '/repo', '/repo',
                 'codex', 'ask_tui', 'prompt_only', 'running', '', '', 0, 0,
                 'run_source_bytes', 'ask_generic', 4, 5
             );
             UPDATE ask_exchanges
                SET state='claimed', active_invocation_id='invocation_ask'
              WHERE id='ask_generic';",
        )
        .unwrap();

        conn.execute_batch(&current_draft_sql("generic_ask_run_claim"))
            .unwrap();

        let ask_columns = columns(&conn, "ask_exchanges");
        for removed in [
            "origin_run_id",
            "origin_turn_id",
            "origin_invocation_id",
            "active_invocation_id",
        ] {
            assert!(!ask_columns.contains(&removed.to_string()));
        }
        for added in ["source_run_id", "active_run_id", "ready_at", "presented_at"] {
            assert!(ask_columns.contains(&added.to_string()));
        }
        assert_eq!(
            conn.query_row(
                "SELECT state || ':' || source_run_id || ':' ||
                        COALESCE(active_run_id, 'none')
                 FROM ask_exchanges WHERE id='ask_generic'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "queued:run_source_bytes:none"
        );
        assert!(conn
            .query_row(
                "SELECT answer_ask_id IS NULL AND ask_ready_at IS NULL
                        AND ask_presented_at IS NULL
                 FROM agent_invocations WHERE id='invocation_ask'",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
    }

    #[test]
    fn legacy_task_flow_repair_restores_existing_tasks_without_moving_their_work() {
        let conn = open();
        if _draft_is_canonical(LEGACY_TASK_FLOW_REPAIR_NAME) {
            apply_before_draft(&conn, LEGACY_TASK_FLOW_REPAIR_NAME);
        } else {
            apply_set(&conn, MIGRATIONS).unwrap();
        }
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

        if _draft_is_canonical(LEGACY_TASK_FLOW_REPAIR_NAME) {
            apply_draft(&conn, LEGACY_TASK_FLOW_REPAIR_NAME);
        } else {
            conn.execute_batch(&migration_sql_for_test(LEGACY_TASK_FLOW_REPAIR_NAME))
                .unwrap();
        }

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

        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for flow in ["slice", "slice", "ship-5whys"] {
            let invocation = crate::wave::playhead::QueuedInvocation::load(&repo, flow)
                .expect("every repaired persisted loop flow resolves");
            assert_eq!(invocation.flow, flow);
        }
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
        apply_before_current_draft(&conn, "status_truth");
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

        conn.execute_batch(&current_draft_sql("status_truth"))
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
