//! The `account` namespace: the provider logins this Machine holds and how a
//! repository routes to them. `account.list` and `account.route` read;
//! `account.connect`, `.disconnect`, `.set`, `.token`, `.use` and
//! `.redeem_reset` change one provider's logins. None of them prompts: a
//! connect with no remembered browser profile refuses and names the flag.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Error, ErrorCode, Result};
use crate::lf::commands::account as handlers;
use crate::lf::commands::account::account_status::AccountReport;
use crate::lf::commands::profile::RouteReport;
use crate::provider_auth::Provider;
use crate::store::{AccountLimitWindow, ProviderAccountId};
use crate::subscription::RateLimitResetCredits;

/// The login a write selected, as the caller named it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub provider: Provider,
    /// The managed login; null for the provider's local credentials.
    pub login: Option<String>,
}

/// `account.set`'s changes; every absent field leaves its setting alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    pub login_email: Option<String>,
    pub routing: Option<String>,
    pub plan: Option<String>,
    pub clear_plan: bool,
    pub paid_through: Option<String>,
    pub clear_paid_through: bool,
    pub clear_cooldown: bool,
    /// Replace the ordered browser choices; empty keeps them.
    pub chrome_profiles: Vec<String>,
    pub clear_chrome_profiles: bool,
}

impl Settings {
    fn lifecycle(&self) -> bool {
        self.login_email.is_some()
            || self.routing.is_some()
            || self.plan.is_some()
            || self.clear_plan
            || self.paid_through.is_some()
            || self.clear_paid_through
    }
}

/// An account's windows and banked resets at one instant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageSnapshot {
    pub windows: Vec<AccountLimitWindow>,
    pub reset_credits: Option<RateLimitResetCredits>,
}

/// What `account.redeem_reset` spent, and the usage before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Redemption {
    pub provider: Provider,
    pub account_id: ProviderAccountId,
    pub login: String,
    /// Retry this attempt with the same key; a new key is a new spend.
    pub idempotency_key: String,
    pub outcome: String,
    pub before: UsageSnapshot,
    /// Null when the refresh after the spend failed; `refresh_error` says why.
    pub after: Option<UsageSnapshot>,
    pub refresh_error: Option<String>,
}

/// `account.list`: every login's state, as `lf account [provider]` shows it.
/// `verify` contacts providers; `details` adds credential sources and
/// browser choices.
pub fn list(provider: Option<Provider>, verify: bool, details: bool) -> Result<AccountReport> {
    block_on(handlers::account_status::report(provider, verify, details)).map_err(unavailable)
}

/// `account.route`: how `directory`'s repository (or `repo`, or the default
/// route) selects an account for each provider.
pub fn route(directory: &Path, repo: Option<&str>, default: bool) -> Result<Vec<RouteReport>> {
    let repo_id = if default {
        None
    } else {
        match repo {
            Some(repo) => Some(
                crate::repository::CanonicalRepo::discover(Path::new(repo))
                    .map_err(|error| Error::new(ErrorCode::Invalid, error.to_string()))?,
            ),
            None => crate::repository::CanonicalRepo::containing(directory).map_err(unavailable)?,
        }
    };
    block_on(async {
        let store = crate::provider_account::read_account_store()?;
        crate::lf::commands::profile::routes(store.as_ref(), repo_id.as_ref()).await
    })
    .map_err(unavailable)
}

/// `account.disconnect`: forget the provider's local credentials, or one
/// managed login's.
pub fn disconnect(provider: &str, email: Option<&str>) -> Result<Account> {
    let account = account(provider, email)?;
    block_on(async {
        match email {
            Some(email) => handlers::disconnect_account(provider, email).await,
            None => handlers::disconnect(provider).await,
        }
    })
    .map_err(|error| Error::written(&error))?;
    Ok(account)
}

/// `account.connect`: sign the provider's local credentials or one managed
/// login in through a remembered browser profile, adopt an existing login
/// (`import`), or read the provider's API key variable (`api_key`).
pub fn connect(
    provider: &str,
    email: Option<&str>,
    chrome_profile: Option<&str>,
    import: bool,
    api_key: bool,
) -> Result<Account> {
    let account = account(provider, email)?;
    block_on(async {
        if api_key {
            return handlers::configure(provider).await;
        }
        if import {
            let email = email.ok_or_else(|| anyhow::anyhow!("import names the login to adopt"))?;
            return handlers::import_account(provider, email).await;
        }
        match email {
            Some(email) => handlers::connect_account(provider, email, chrome_profile).await,
            None => handlers::connect(provider, chrome_profile).await,
        }
    })
    .map_err(|error| Error::outcome_unknown(&error))?;
    Ok(account)
}

/// `account.set`: edit a managed login's lifecycle, clear its cooldown, or
/// replace the browser choices of the login or the local credentials.
pub fn set(provider: &str, email: Option<&str>, settings: &Settings) -> Result<Account> {
    let account = account(provider, email)?;
    if !settings.lifecycle()
        && !settings.clear_cooldown
        && settings.chrome_profiles.is_empty()
        && !settings.clear_chrome_profiles
    {
        return Err(Error::new(
            ErrorCode::Invalid,
            "account set needs an account setting or --chrome-profile / --clear-chrome-profiles",
        ));
    }
    block_on(async {
        if settings.lifecycle() {
            let email = email.ok_or_else(|| anyhow::anyhow!("account settings name a login"))?;
            handlers::set_account_lifecycle(
                provider,
                email,
                handlers::AccountLifecycleUpdate {
                    login_email: settings.login_email.as_deref(),
                    routing: settings.routing.as_deref(),
                    plan: settings.plan.as_deref(),
                    clear_plan: settings.clear_plan,
                    paid_through: settings.paid_through.as_deref(),
                    clear_paid_through: settings.clear_paid_through,
                },
            )
            .await?;
        }
        let updated = settings.login_email.as_deref().or(email);
        if settings.clear_cooldown {
            let email = updated.ok_or_else(|| anyhow::anyhow!("a cooldown belongs to a login"))?;
            handlers::clear_account_cooldown(provider, email).await?;
        }
        if !settings.chrome_profiles.is_empty() || settings.clear_chrome_profiles {
            handlers::set_browser_profiles(provider, updated, &settings.chrome_profiles).await?;
        }
        Ok(())
    })
    .map_err(|error| Error::written(&error))?;
    Ok(account)
}

/// `account.token`: store a long-lived Claude token for a managed login,
/// minted in its browser profile or read from `file`.
pub fn token(
    provider: &str,
    email: &str,
    file: Option<&Path>,
    chrome_profile: Option<&str>,
) -> Result<Account> {
    let account = account(provider, Some(email))?;
    block_on(handlers::token_account(
        provider,
        email,
        file,
        chrome_profile,
    ))
    .map_err(|error| Error::outcome_unknown(&error))?;
    Ok(account)
}

/// `account.redeem_reset`: spend one banked Codex reset for a managed login.
/// The attempt's key is in the result; a retry passes it back.
pub fn redeem_reset(
    provider: &str,
    email: &str,
    idempotency_key: Option<&str>,
    credit_id: Option<&str>,
) -> Result<Redemption> {
    block_on(handlers::redeem_reset(
        provider,
        email,
        idempotency_key,
        credit_id,
    ))
    .map_err(|error| Error::outcome_unknown(&error))
}

/// `account.use`: sign the provider's ordinary home in as a stored login, as
/// `lf account <provider> use <email>` does.
pub fn activate(provider: &str, email: &str) -> Result<Account> {
    let account = account(provider, Some(email))?;
    block_on(handlers::use_account(provider, email)).map_err(|error| Error::written(&error))?;
    Ok(account)
}

fn account(provider: &str, email: Option<&str>) -> Result<Account> {
    Ok(Account {
        provider: handlers::parse_provider(provider)
            .map_err(|error| Error::new(ErrorCode::Invalid, error.to_string()))?,
        login: email.map(str::to_owned),
    })
}

fn block_on<T>(future: impl std::future::Future<Output = anyhow::Result<T>>) -> anyhow::Result<T> {
    tokio::runtime::Runtime::new()?.block_on(future)
}

fn unavailable(error: anyhow::Error) -> Error {
    Error::refused(&error, ErrorCode::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::{Account, AccountReport, Redemption, RouteReport};

    fn result<T: serde::de::DeserializeOwned + serde::Serialize>(raw: &str) -> T {
        let message: serde_json::Value = serde_json::from_str(raw).unwrap();
        let typed: T = serde_json::from_value(message["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), message["result"]);
        typed
    }

    #[test]
    fn reports_match_the_shared_fixtures_without_defaults() {
        let report: AccountReport = result(include_str!(
            "../../../../tests/fixtures/dto/server/account_report.json"
        ));
        assert_eq!(report.accounts.len(), 1);
        let routes: Vec<RouteReport> = result(include_str!(
            "../../../../tests/fixtures/dto/server/account_routes.json"
        ));
        assert_eq!(routes.len(), 2);
        let account: Account = result(include_str!(
            "../../../../tests/fixtures/dto/server/account.json"
        ));
        assert_eq!(account.login.as_deref(), Some("maya@example.com"));
        let redeemed: Redemption = result(include_str!(
            "../../../../tests/fixtures/dto/server/account_redeemed.json"
        ));
        assert!(redeemed.after.is_none());
        assert!(serde_json::from_str::<Account>(r#"{"login":"maya@example.com"}"#).is_err());
    }
}
