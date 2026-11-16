//! The `account` namespace: the provider logins this Machine holds and how a
//! repository routes to them. `account.list` and `account.next` read;
//! `account.connect`, `.disconnect`, `.set`, `.use`, `.pause`, `.unpause` and
//! `.redeem_reset` change one provider's logins. None of them prompts: a
//! connect with no remembered browser profile refuses and names the flag.

use std::path::Path;

use super::{Error, ErrorCode, Result};
use crate::provider_account::operations as handlers;
use crate::provider_account::operations::account_status::AccountReport;
use crate::provider_account::routing::NextLaunch;
use crate::provider_auth::Provider;

pub use crate::provider_account::operations::{Account, Redemption, Settings, UsageSnapshot};

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
pub fn next(
    directory: &Path,
    repo: Option<&str>,
    default: bool,
    agent: Option<&str>,
) -> Result<Vec<NextLaunch>> {
    let repo_id = if default {
        None
    } else {
        match repo {
            Some(repo) => Some(
                crate::repository::discover(Path::new(repo))
                    .map_err(|error| Error::new(ErrorCode::Invalid, error.to_string()))?,
            ),
            None => crate::repository::containing(directory).map_err(unavailable)?,
        }
    };
    block_on(crate::provider_account::routing::next(
        repo_id.as_ref(),
        agent,
    ))
    .map_err(unavailable)
}

pub fn disconnect(provider: &str, email: Option<&str>) -> Result<Account> {
    let account = account(provider, email)?;
    block_on(handlers::disconnect_target(&target(provider, email)))
        .map_err(|error| Error::written(&error))?;
    Ok(account)
}
pub fn connect(
    provider: &str,
    email: Option<&str>,
    chrome_profile: Option<&str>,
    import: bool,
    api_key: bool,
) -> Result<Account> {
    let account = account(provider, email)?;
    block_on(handlers::connect_target(
        &target(provider, email),
        chrome_profile,
        import,
        api_key,
    ))
    .map_err(|error| Error::outcome_unknown(&error))?;
    Ok(account)
}
pub fn set(provider: &str, email: Option<&str>, settings: &Settings) -> Result<Account> {
    let account = account(provider, email)?;
    block_on(handlers::set(&target(provider, email), settings))
        .map_err(|error| Error::written(&error))?;
    Ok(account)
}
fn target(provider: &str, email: Option<&str>) -> String {
    email
        .map(|email| format!("{provider}={email}"))
        .unwrap_or_else(|| provider.to_owned())
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
    block_on(handlers::use_login(login)).map_err(|error| Error::written(&error))
}
pub fn pause(login: &str, paused: bool) -> Result<()> {
    let routing = if paused {
        crate::store::RoutingState::ExplicitOnly
    } else {
        crate::store::RoutingState::Automatic
    };
    block_on(handlers::set_rotation(login, routing)).map_err(|error| Error::written(&error))
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
            "../../tests/fixtures/dto/server/account_report.json"
        ));
        assert_eq!(report.accounts.len(), 1);
        let routes: Vec<NextLaunch> = result(include_str!(
            "../../tests/fixtures/dto/server/account_next.json"
        ));
        assert_eq!(routes.len(), 2);
        let account: Account = result(include_str!("../../tests/fixtures/dto/server/account.json"));
        assert_eq!(account.login.as_deref(), Some("maya@example.com"));
        let redeemed: Redemption = result(include_str!(
            "../../tests/fixtures/dto/server/account_redeemed.json"
        ));
        assert!(redeemed.after.is_none());
        assert!(serde_json::from_str::<Account>(r#"{"login":"maya@example.com"}"#).is_err());
    }
}
