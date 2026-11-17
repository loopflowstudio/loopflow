//! Service authentication, separate from native agent authentication.
use super::{Error, ErrorCode, Result};
use crate::provider_auth::operations as handlers;
use crate::provider_auth::operations::account_status::AccountReport;
use crate::provider_auth::Provider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub provider: Provider,
}

pub fn list(target: Option<&str>) -> Result<AccountReport> {
    block_on(handlers::account_status::report(target)).map_err(unavailable)
}
pub fn disconnect(provider: &str) -> Result<Account> {
    let account = account(provider)?;
    block_on(handlers::disconnect_target(provider)).map_err(|error| Error::written(&error))?;
    Ok(account)
}
pub fn connect(provider: &str, chrome_profile: Option<&str>, api_key: bool) -> Result<Account> {
    let account = account(provider)?;
    block_on(handlers::connect_target(provider, chrome_profile, api_key))
        .map_err(|error| Error::written(&error))?;
    Ok(account)
}
fn account(provider: &str) -> Result<Account> {
    let provider =
        provider
            .parse()
            .map_err(|error: crate::provider_auth::ParseProviderError| {
                Error::new(ErrorCode::Invalid, error.to_string())
            })?;
    Ok(Account { provider })
}
fn unavailable(error: anyhow::Error) -> Error {
    Error::new(ErrorCode::Unavailable, error.to_string())
}
fn block_on<T>(future: impl std::future::Future<Output = anyhow::Result<T>>) -> anyhow::Result<T> {
    tokio::runtime::Runtime::new()?.block_on(future)
}
