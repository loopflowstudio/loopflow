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

/// Open for reading only with the same frontier checks as [`open_sqlite`].
pub fn open_read_only_validated(path: &Path) -> StoreResult<SqliteStore> {
    let home = super::machine_home_dir();
    let may_apply = super::may_apply_migrations(path, &home, FrontierAdvance::Forbidden)
        .map_err(|error| StoreError::InvalidData(format!("resolve migration authority: {error}")))?;
    let shared = super::same_database_file(path, &home.join(".lf/loopflow.db")).map_err(|error| {
        StoreError::InvalidData(format!("resolve shared store identity: {error}"))
    })?;
    SqliteStore::open_read_only_validated(
        path,
        shared,
        may_apply,
        &super::migrations::build(),
        &super::migrations::observers(),
        crate::store::token_key::encryption_key,
    )
}

/// Apply pending migrations while the install caller holds the promotion lock.
pub fn open_for_promotion(path: &Path) -> StoreResult<SqliteStore> {
    open_with(
        path,
        &super::machine_home_dir(),
        FrontierAdvance::Authorized,
        SQLITE_WRITE_BUSY_TIMEOUT,
    )
}

/// Open a private store without acquiring the Machine's frontier authority.
pub fn open_ephemeral(path: &Path) -> StoreResult<SqliteStore> {
    SqliteStore::open_ephemeral_prepared(
        path,
        &super::migrations::build(),
        &super::migrations::observers(),
        crate::store::token_key::encryption_key,
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
        crate::store::token_key::encryption_key,
    )
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use crate::store::{CredentialType, ProviderToken};
    use loopflow_storage::token_crypto::TokenCryptoError;

    #[test]
    fn credential_keys_are_lazy_and_the_supplied_source_survives_reopen() {
        fn unavailable() -> Result<[u8; 32], TokenCryptoError> {
            Err(TokenCryptoError::KeyRetrieval("key unavailable".into()))
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("db");
        let store = SqliteStore::open_ephemeral_prepared(
            &path,
            &crate::store::migrations::build(),
            &crate::store::migrations::observers(),
            unavailable,
        )
        .unwrap();
        assert!(store.get_provider_token("linear").unwrap().is_none());
        let token = ProviderToken {
            provider: "linear".into(),
            access_token: "access".into(),
            refresh_token: Some("refresh".into()),
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1,
            credential_type: CredentialType::OAuth,
        };
        assert!(store.upsert_provider_token(&token).is_err());
        assert!(store.get_provider_token("linear").unwrap().is_none());
        drop(store);
        let store = SqliteStore::open_prepared(
            &path,
            false,
            true,
            super::SQLITE_WRITE_BUSY_TIMEOUT,
            &crate::store::migrations::build(),
            &crate::store::migrations::observers(),
            || Ok([7; 32]),
        )
        .unwrap();
        store.upsert_provider_token(&token).unwrap();
        assert_eq!(
            store.clone().get_provider_token("linear").unwrap(),
            Some(token.clone())
        );
        drop(store);
        let reader = SqliteStore::open_read_only(&path, unavailable).unwrap();
        assert!(reader
            .provider_auth_snapshot(crate::provider_auth::Provider::Linear)
            .unwrap()
            .is_some());
        assert!(reader.get_provider_token("linear").is_err());
        let reader = SqliteStore::open_read_only(&path, || Ok([7; 32])).unwrap();
        assert_eq!(reader.get_provider_token("linear").unwrap(), Some(token));
    }
}

#[cfg(test)]
#[path = "opening_frontier_tests.rs"]
mod frontier_tests;
