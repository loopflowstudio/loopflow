//! `lf account use <login>` holds a provider's launches on that login until it
//! fills up or `lf account use auto` hands them back to routing. The hold is
//! this Machine's, like its login, so it lives beside the stored logins.

use std::fs;
use std::path::PathBuf;

use super::{now_unix, ProviderAccountError};
use crate::provider_auth::Provider;
use crate::store::{ProviderAccountId, SharedStore};

fn path(provider: Provider) -> PathBuf {
    crate::store::lf_home_dir()
        .join("accounts")
        .join(provider.as_str())
        .join("held")
}

/// The login launches stay on, if one is held.
pub(crate) fn held(provider: Provider) -> Option<ProviderAccountId> {
    let raw = fs::read_to_string(path(provider)).ok()?;
    ProviderAccountId::parse(raw.trim()).ok()
}

pub(crate) fn hold(
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<(), ProviderAccountError> {
    let path = path(provider);
    let directory = path.parent().expect("hold path has a parent");
    fs::create_dir_all(directory)
        .and_then(|()| fs::write(&path, account_id.as_str()))
        .map_err(|error| ProviderAccountError::Filesystem(format!("hold {provider}: {error}")))
}

/// Hand `provider`'s launches back to routing. Whether a hold existed.
pub(crate) fn release(provider: Provider) -> Result<bool, ProviderAccountError> {
    match fs::remove_file(path(provider)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ProviderAccountError::Filesystem(format!(
            "release {provider} hold: {error}"
        ))),
    }
}

/// A held login that cannot take this launch keeps its hold while only a
/// model-scoped window holds it back; once the whole login is out (a cooldown
/// or an account-wide window), it is full and the hold ends.
pub(crate) async fn end_when_full(
    store: &SharedStore,
    provider: Provider,
    held: &ProviderAccountId,
) -> Result<(), ProviderAccountError> {
    let Some(account) = store.get_provider_account(provider.as_str(), held).await? else {
        release(provider)?;
        return Ok(());
    };
    let now = now_unix();
    let limits = store
        .provider_account_limits(Some(provider.as_str()))
        .await?;
    let account_wide = account.cooldown_until.is_some_and(|until| until > now)
        || limits.iter().any(|limit| {
            limit.account_id == *held
                && limit.model.is_none()
                && limit.limited_until.is_some_and(|until| until > now)
        });
    if account_wide {
        tracing::info!(%held, "{provider} hold ends: the held login is full");
        release(provider)?;
    }
    Ok(())
}
