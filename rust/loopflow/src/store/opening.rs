//! Application selection of the Machine home and migration frontier.

use super::sqlite::{SqliteStore, SQLITE_WRITE_BUSY_TIMEOUT};
use super::{FrontierAdvance, StoreError, StoreResult};
use std::path::Path;
use std::time::Duration;

/// Open for ordinary use, validating the shared frontier without advancing it.
pub fn open_sqlite(path: &Path) -> StoreResult<SqliteStore> {
    open_with(
        path,
        &super::machine_home_dir(),
        FrontierAdvance::Forbidden,
        SQLITE_WRITE_BUSY_TIMEOUT,
    )
}

/// Apply pending migrations while the install caller holds the promotion lock.
pub(crate) fn open_for_promotion(path: &Path) -> StoreResult<SqliteStore> {
    open_with(
        path,
        &super::machine_home_dir(),
        FrontierAdvance::Authorized,
        SQLITE_WRITE_BUSY_TIMEOUT,
    )
}

/// Open a private store without acquiring the Machine's frontier authority.
pub(crate) fn open_ephemeral(path: &Path) -> StoreResult<SqliteStore> {
    SqliteStore::open_ephemeral_prepared(
        path,
        &super::migrations::build(),
        &super::migrations::observers(),
    )
}

pub(super) fn open_with(
    path: &Path,
    home: &Path,
    advance: FrontierAdvance,
    busy: Duration,
) -> StoreResult<SqliteStore> {
    // Decide before creating a directory or an empty shared database.
    let may_apply = super::may_apply_migrations(path, home, advance).map_err(|error| {
        StoreError::InvalidData(format!("resolve migration authority: {error}"))
    })?;
    let shared =
        super::same_database_file(path, &home.join(".lf/loopflow.db")).map_err(|error| {
            StoreError::InvalidData(format!("resolve shared store identity: {error}"))
        })?;
    SqliteStore::open_prepared(
        path,
        shared,
        may_apply,
        busy,
        &super::migrations::build(),
        &super::migrations::observers(),
    )
}
