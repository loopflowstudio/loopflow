//! The `account` namespace: the provider logins this Machine holds and how a
//! repository routes to them. `account.list` and `account.next` read;
//! `account.connect`, `.disconnect`, `.set`, `.use`, `.pause`, `.unpause` and
//! `.redeem_reset` change one provider's logins. None of them prompts: a
//! connect with no remembered browser profile refuses and names the flag.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Error, ErrorCode, Result};
use crate::lf::commands::account as handlers;
use crate::lf::commands::account::account_status::AccountReport;
use crate::lf::commands::profile::NextLaunch;
use crate::provider_auth::Provider;
use crate::store::{AccountLimitWindow, ProviderAccountId};
use crate::subscription::RateLimitResetCredits;

/// The login a write selected, as the caller named it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
    pub plan: Option<String>,
    pub clear_plan: bool,
    pub paid_through: Option<String>,
    pub clear_paid_through: bool,
    pub clear_cooldown: bool,
    /// Replace the ordered browser choices; empty keeps them.
    pub chrome_profiles: Vec<String>,
    pub clear_chrome_profiles: bool,
}

/// An account's windows and banked resets at one instant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UsageSnapshot {
    pub windows: Vec<AccountLimitWindow>,
    pub reset_credits: Option<RateLimitResetCredits>,
}

/// What `account.redeem_reset` spent, and the usage before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
/// Naming a target includes its details; cached reads never contact providers.
pub fn list(target: Option<&str>, cached: bool) -> Result<AccountReport> {
    let target = target
        .map(handlers::Target::parse)
        .transpose()
        .map_err(|error| Error::new(ErrorCode::Invalid, error.to_string()))?;
    block_on(handlers::account_status::report(target, cached)).map_err(unavailable)
}

/// Which login the next launch takes for the caller's repository.
pub fn next(directory: &Path, repo: Option<&str>, default: bool) -> Result<Vec<NextLaunch>> {
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
    block_on(crate::lf::commands::profile::next(repo_id.as_ref())).map_err(unavailable)
}

pub fn disconnect(provider: &str, email: Option<&str>) -> Result<Account> {
    change(
        provider,
        email,
        crate::lf::AccountCommand::Disconnect {
            target: target(provider, email),
        },
    )
}

pub fn connect(
    provider: &str,
    email: Option<&str>,
    chrome_profile: Option<&str>,
    import: bool,
    api_key: bool,
) -> Result<Account> {
    change(
        provider,
        email,
        crate::lf::AccountCommand::Connect {
            target: target(provider, email),
            chrome_profile: chrome_profile.map(str::to_owned),
            import,
            api_key,
        },
    )
}

pub fn set(provider: &str, email: Option<&str>, settings: &Settings) -> Result<Account> {
    change(
        provider,
        email,
        crate::lf::AccountCommand::Set {
            target: target(provider, email),
            login_email: settings.login_email.clone(),
            plan: settings.plan.clone(),
            clear_plan: settings.clear_plan,
            paid_through: settings.paid_through.clone(),
            clear_paid_through: settings.clear_paid_through,
            clear_cooldown: settings.clear_cooldown,
            chrome_profile: settings.chrome_profiles.clone(),
            clear_chrome_profiles: settings.clear_chrome_profiles,
        },
    )
}

fn target(provider: &str, email: Option<&str>) -> String {
    email
        .map(|email| format!("{provider}={email}"))
        .unwrap_or_else(|| provider.to_owned())
}

fn change(
    provider: &str,
    email: Option<&str>,
    command: crate::lf::AccountCommand,
) -> Result<Account> {
    let account = account(provider, email)?;
    block_on(handlers::run_async(&command)).map_err(|error| match command {
        crate::lf::AccountCommand::Connect { .. } => Error::outcome_unknown(&error),
        _ => Error::written(&error),
    })?;
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

/// Hold a login on this Machine, or release holds with `auto`.
pub fn activate(login: &str) -> Result<()> {
    block_on(handlers::run_async(&crate::lf::AccountCommand::Use {
        login: login.to_owned(),
    }))
    .map_err(|error| Error::written(&error))
}

pub fn pause(login: &str, paused: bool) -> Result<()> {
    let command = if paused {
        crate::lf::AccountCommand::Pause {
            login: login.to_owned(),
        }
    } else {
        crate::lf::AccountCommand::Unpause {
            login: login.to_owned(),
        }
    };
    block_on(handlers::run_async(&command)).map_err(|error| Error::written(&error))
}

fn account(provider: &str, email: Option<&str>) -> Result<Account> {
    Ok(Account {
        provider: provider
            .parse::<Provider>()
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
    use super::{Account, AccountReport, NextLaunch, Redemption};

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
        let routes: Vec<NextLaunch> = result(include_str!(
            "../../../../tests/fixtures/dto/server/account_next.json"
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
