//! Account observations combined with authored preferences and routes.
use crate::profile::{AuthBrowserBinding, ProfileId, ProviderRoute, RouteScope};
use crate::provider_auth::Provider;
use crate::store::{ProviderAccount, ProviderAccountId, StoreError, StoreResult};
use loopflow_storage::sqlite::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub fn upsert_provider_account(store: &SqliteStore, account: &ProviderAccount) -> StoreResult<()> {
    crate::config::placement::write_account_preferences(&store.home_dir()?, account)
        .map_err(invalid)?;
    store.upsert_provider_account_row(account)
}

pub fn get_provider_account(
    store: &SqliteStore,
    provider: &str,
    id: &ProviderAccountId,
) -> StoreResult<Option<ProviderAccount>> {
    let account = store.provider_account_row(provider, id)?;
    let Some(account) = account else {
        return Ok(None);
    };
    let config =
        crate::config::placement::global_settings_at(&store.home_dir()?).map_err(invalid)?;
    crate::config::placement::account_preferences(&config, account)
        .map(Some)
        .map_err(invalid)
}

pub fn list_provider_accounts(
    store: &SqliteStore,
    provider: Option<&str>,
) -> StoreResult<Vec<ProviderAccount>> {
    let accounts = store.provider_account_rows(provider)?;
    let config =
        crate::config::placement::global_settings_at(&store.home_dir()?).map_err(invalid)?;
    accounts
        .into_iter()
        .map(|account| {
            crate::config::placement::account_preferences(&config, account).map_err(invalid)
        })
        .collect()
}

pub(crate) fn update_provider_account_lifecycle(
    store: &SqliteStore,
    account: &ProviderAccount,
) -> StoreResult<()> {
    if store
        .provider_account_row(&account.provider, &account.account_id)?
        .is_none()
    {
        return Err(StoreError::NotFound);
    }
    crate::config::placement::write_account_preferences(&store.home_dir()?, account)
        .map_err(invalid)
}

pub(crate) fn set_auth_browser_profiles(
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
        Some(
            serde_yaml_ng::to_value(profile_ids)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        ),
    )
    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    Ok(())
}

pub fn list_auth_browser_profiles(
    store: &SqliteStore,
    provider: Option<Provider>,
    account_id: Option<&ProviderAccountId>,
) -> StoreResult<Vec<AuthBrowserBinding>> {
    let declarations = crate::config::placement::global_settings_at(&store.home_dir()?)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        .auth_browsers;
    let mut out = Vec::new();
    for (name, declaration) in declarations {
        let selected = name
            .parse::<Provider>()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
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

pub fn set_provider_route(store: &SqliteStore, route: &ProviderRoute) -> StoreResult<()> {
    let (path, prefix) = crate::config::placement::route_location(store, &route.scope)
        .map_err(|e| StoreError::InvalidData(e.to_string()))?;
    let parts: Vec<_> = prefix
        .iter()
        .map(String::as_str)
        .chain(["account", route.provider.as_str()])
        .collect();
    crate::config::files::update(&path, |document| {
        crate::config::settings::insert(
            document,
            &parts,
            serde_yaml_ng::to_value(&route.accounts)?,
        );
        Ok(())
    })
    .map_err(|e| StoreError::InvalidData(e.to_string()))
}

pub(crate) fn account_declarations(
    store: &SqliteStore,
    scopes: &[RouteScope],
) -> StoreResult<Vec<crate::config::ConfigDeclarations>> {
    scopes
        .iter()
        .map(|scope| {
            let (path, prefix) = crate::config::placement::route_location(store, scope)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            let mut document = crate::config::files::read(&path)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            crate::config::remote::apply_document(&path, &mut document)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            let accounts = prefix
                .iter()
                .try_fold(&document, |value, key| value.get(key))
                .and_then(|value| value.get("account"));
            let mut value = serde_yaml_ng::Value::Mapping(Default::default());
            if let Some(accounts) = accounts {
                value["account"] = accounts.clone();
            }
            Ok(crate::config::ConfigDeclarations {
                source: crate::config::ConfigSource::Personal,
                scope: match scope {
                    RouteScope::Wave(_) => crate::config::settings::SettingScope::Wave,
                    RouteScope::Repo(_) => crate::config::settings::SettingScope::Repository,
                    RouteScope::Default => crate::config::settings::SettingScope::Global,
                },
                path,
                value,
            })
        })
        .collect()
}
