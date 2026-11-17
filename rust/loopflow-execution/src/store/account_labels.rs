//! Saved account rows read for history, with their old configuration labels.
use crate::store::{ProviderAccount, ProviderAccountId, StoreError, StoreResult};
use loopflow_storage::sqlite::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub fn get_provider_account(
    store: &SqliteStore,
    provider: &str,
    id: &ProviderAccountId,
) -> StoreResult<Option<ProviderAccount>> {
    let Some(account) = store.provider_account_row(provider, id)? else {
        return Ok(None);
    };
    let labels =
        crate::config::placement::historical_account_labels(&store.home_dir()?).map_err(invalid)?;
    crate::config::placement::historical_account(&labels, account)
        .map(Some)
        .map_err(invalid)
}

pub fn list_provider_accounts(
    store: &SqliteStore,
    provider: Option<&str>,
) -> StoreResult<Vec<ProviderAccount>> {
    let accounts = store.provider_account_rows(provider)?;
    let labels =
        crate::config::placement::historical_account_labels(&store.home_dir()?).map_err(invalid)?;
    accounts
        .into_iter()
        .map(|account| crate::config::placement::historical_account(&labels, account))
        .collect::<anyhow::Result<_>>()
        .map_err(invalid)
}
