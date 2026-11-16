//! Service authentication, separate from native agent authentication.
use super::{Error, ErrorCode, Result};
use crate::lf::commands::account as handlers;
use crate::lf::commands::account::account_status::AccountReport;
use crate::provider_auth::Provider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub provider: Provider,
}

pub fn list(target: Option<&str>, cached: bool) -> Result<AccountReport> {
    block_on(handlers::account_status::report(target, cached)).map_err(unavailable)
}
pub fn disconnect(provider: &str) -> Result<Account> {
    change(
        provider,
        crate::lf::AccountCommand::Disconnect {
            target: provider.to_owned(),
        },
    )
}
pub fn connect(provider: &str, chrome_profile: Option<&str>, api_key: bool) -> Result<Account> {
    change(
        provider,
        crate::lf::AccountCommand::Connect {
            target: provider.to_owned(),
            chrome_profile: chrome_profile.map(str::to_owned),
            api_key,
        },
    )
}
fn change(provider: &str, command: crate::lf::AccountCommand) -> Result<Account> {
    let provider =
        provider
            .parse()
            .map_err(|error: crate::provider_auth::ParseProviderError| {
                Error::new(ErrorCode::Invalid, error.to_string())
            })?;
    block_on(handlers::run_async(&command)).map_err(|error| Error::written(&error))?;
    Ok(Account { provider })
}
fn unavailable(error: anyhow::Error) -> Error {
    Error::new(ErrorCode::Unavailable, error.to_string())
}
fn block_on<T>(future: impl std::future::Future<Output = anyhow::Result<T>>) -> anyhow::Result<T> {
    tokio::runtime::Runtime::new()?.block_on(future)
}
