//! Application-supplied upgrade observations. Migration order and SQL live in the runner.

use std::fs::File;
use std::path::Path;

use super::{ProcessIdentityEvidence, StoreError, StoreResult};
pub use loopflow_storage::migration_runner::{
    apply_sqlite_with_backup, latest_applied_version_sqlite, latest_known_version,
    migration_backups, migration_sql_for_test, prune_migration_backups, unmerged_migrations,
    validate_set, validate_sqlite_schema, Migration, MigrationBuild, MigrationId,
    MigrationObservers, MigrationProcess,
};

pub fn observers() -> MigrationObservers<'static> {
    MigrationObservers {
        export_contract: &export_contract,
        process_evidence: &process_evidence,
    }
}

fn export_contract(conn: &rusqlite::Connection, guards: &mut Vec<File>) -> StoreResult<()> {
    crate::config::migrate::preflight(conn, guards)
        .map_err(|error| StoreError::InvalidData(error.to_string()))
}

pub(crate) fn process_evidence(
    database: Option<&Path>,
    process: &MigrationProcess,
) -> ProcessIdentityEvidence {
    if process.kind == "agent" {
        return crate::journal::agent_process_evidence(
            process.completed_at,
            process.pid,
            process.os_started_at,
            None,
            crate::journal::process_identity_evidence,
        );
    }
    let receipt = database.and_then(Path::parent).map(|home| {
        crate::journal::read_process_receipt(&crate::journal::process_receipt_path(
            home,
            &process.id,
        ))
    });
    match receipt {
        Some(Ok(Some(receipt))) if receipt.matches_process(&process.id, &process.trace) => {
            receipt.process_evidence()
        }
        Some(Ok(None))
            if process.completed_at.is_some()
                || crate::journal::began_before_boot(process.started_at) =>
        {
            ProcessIdentityEvidence::Dead
        }
        // A malformed or mismatched receipt is not absence, even for an old row.
        _ => ProcessIdentityEvidence::Unknown,
    }
}

pub fn apply_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    super::migration_runner::apply_sqlite(&build(), conn, &observers())
}

pub fn apply_sqlite_preview(conn: &rusqlite::Connection) -> StoreResult<()> {
    super::migration_runner::apply_sqlite_preview(&build(), conn, &observers())
}

pub fn build() -> MigrationBuild<'static> {
    MigrationBuild {
        provenance: crate::build_identity::provenance().as_str(),
        source_identity: crate::build_identity::source_identity(),
        source_revision: crate::build_identity::source_revision(),
        version: crate::build_identity::version(),
        package_version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(any(test, feature = "test-support"))]
pub fn apply_newer_migration(
    conn: &rusqlite::Connection,
    sql: &'static str,
) -> StoreResult<String> {
    super::migration_runner::apply_newer_migration(&observers(), &build(), conn, sql)
}

pub fn latest_version_sqlite(conn: &rusqlite::Connection) -> StoreResult<String> {
    super::migration_runner::latest_version_sqlite(&build(), conn)
}

pub fn pending_shared_migration(conn: &rusqlite::Connection) -> StoreResult<Option<String>> {
    super::migration_runner::pending_shared_migration(&build(), conn)
}

pub fn pending_unmerged(conn: &rusqlite::Connection) -> StoreResult<Vec<String>> {
    super::migration_runner::pending_unmerged(&build(), conn)
}

pub fn requires_migration_sqlite(conn: &rusqlite::Connection) -> StoreResult<bool> {
    super::migration_runner::requires_migration_sqlite(&build(), conn)
}

pub fn validate_sqlite(conn: &rusqlite::Connection) -> StoreResult<()> {
    super::migration_runner::validate_sqlite(&build(), conn)
}

#[cfg(test)]
#[path = "canonical_lifecycle_tests.rs"]
mod canonical_lifecycle_tests;

#[cfg(any(test, feature = "test-support"))]
pub fn apply_before_migration(conn: &rusqlite::Connection, name: &str) {
    super::migration_runner::fixtures::apply_before_migration(&observers(), &build(), conn, name);
}

#[cfg(any(test, feature = "test-support"))]
pub fn upgrade_from_migration(conn: &rusqlite::Connection, name: &str) {
    super::migration_runner::fixtures::upgrade_from_migration(&observers(), &build(), conn, name);
}

#[cfg(any(test, feature = "test-support"))]
pub fn apply_pending_migrations(conn: &rusqlite::Connection) {
    super::migration_runner::fixtures::apply_pending_migrations(&observers(), &build(), conn);
}

#[cfg(test)]
#[path = "migration_lifecycle_tests.rs"]
mod migration_lifecycle_tests;
