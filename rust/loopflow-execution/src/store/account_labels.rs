//! Saved account rows read for history, with their old configuration labels,
//! and the Chrome profiles service logins open in.
use crate::profile::{AuthBrowserBinding, ProfileId};
use crate::provider_auth::Provider;
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

pub fn set_auth_browser_profiles(
    store: &SqliteStore,
    provider: Provider,
    account_id: Option<&ProviderAccountId>,
    profile_ids: &[ProfileId],
) -> StoreResult<()> {
    let mut path = vec!["auth_browsers", provider.as_str()];
    if let Some(account) = account_id {
        path.extend(["accounts", account.as_str()]);
    } else {
        path.push("default");
    }
    crate::config::placement::write_global(
        &store.home_dir()?,
        &path,
        Some(serde_yaml_ng::to_value(profile_ids).map_err(invalid)?),
    )
    .map_err(invalid)?;
    Ok(())
}

pub fn list_auth_browser_profiles(
    store: &SqliteStore,
    provider: Option<Provider>,
    account_id: Option<&ProviderAccountId>,
) -> StoreResult<Vec<AuthBrowserBinding>> {
    let declarations = crate::config::placement::global_settings_at(&store.home_dir()?)
        .map_err(invalid)?
        .auth_browsers;
    let mut out = Vec::new();
    for (name, declaration) in declarations {
        let selected = name.parse::<Provider>().map_err(invalid)?;
        if provider.is_some_and(|provider| provider != selected) {
            continue;
        }
        for (account, profiles) in std::iter::once((None, declaration.default)).chain(
            declaration
                .accounts
                .into_iter()
                .map(|(account, profiles)| (Some(account), profiles)),
        ) {
            if account_id.is_some_and(|id| account.as_deref() != Some(id.as_str())) {
                continue;
            }
            for (position, profile) in profiles.into_iter().enumerate() {
                out.push(AuthBrowserBinding {
                    provider: selected,
                    account_id: account
                        .as_deref()
                        .map(ProviderAccountId::parse)
                        .transpose()
                        .map_err(StoreError::InvalidData)?,
                    position,
                    profile_id: ProfileId::parse(&profile).map_err(StoreError::InvalidData)?,
                });
            }
        }
    }
    Ok(out)
}
