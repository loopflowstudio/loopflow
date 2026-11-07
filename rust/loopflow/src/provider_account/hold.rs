//! `lf account use <login>` holds a provider's launches on that login until it
//! fills up or `lf account use auto` hands them back to routing. The hold is
//! this Machine's, like its login, so it lives beside the stored logins.

use std::fs;
use std::path::PathBuf;

use super::{account_held_until, ProviderAccountError};
use crate::provider_auth::Provider;
use crate::store::{AccountLimitRow, CredentialState, ProviderAccount, ProviderAccountId};

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

/// The held login, when it can take a launch of `model`. Unlike routing, this
/// asks nothing about an order or a pause.
pub(crate) fn ready<'a>(
    provider: Provider,
    accounts: &'a [ProviderAccount],
    limits: &[AccountLimitRow],
    model: Option<&str>,
    now: i64,
) -> Option<&'a ProviderAccount> {
    let held = held(provider)?;
    accounts
        .iter()
        .find(|account| account.account_id == held)
        .filter(|account| {
            account.credential_state == CredentialState::Connected
                && account_held_until(account, limits, model, now).is_none()
        })
}

/// A held login that cannot take this launch keeps its hold while only a
/// model-scoped window holds it back; once the whole login is out (a cooldown
/// or an account-wide window) or gone, the hold ends.
pub(crate) fn end_when_full(
    provider: Provider,
    accounts: &[ProviderAccount],
    limits: &[AccountLimitRow],
    now: i64,
) -> Result<(), ProviderAccountError> {
    let Some(held) = held(provider) else {
        return Ok(());
    };
    let full = accounts
        .iter()
        .find(|account| account.account_id == held)
        .is_none_or(|account| {
            account.cooldown_until.is_some_and(|until| until > now)
                || limits.iter().any(|limit| {
                    limit.account_id == held
                        && limit.model.is_none()
                        && limit.limited_until.is_some_and(|until| until > now)
                })
        });
    if full {
        tracing::info!(%held, "{provider} hold ends: the held login is full");
        release(provider)?;
    }
    Ok(())
}
