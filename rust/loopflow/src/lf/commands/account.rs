use crate::lf::AccountCommand;
use anyhow::{Context, Result};
use loopflow_execution::provider_auth::operations;

pub fn run(cmd: Option<&AccountCommand>, target: Option<&str>, json: bool) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    match cmd {
        Some(cmd) => rt.block_on(run_async(cmd)),
        None => rt.block_on(operations::account_status::run(target, json)),
    }
}

pub(crate) async fn run_async(cmd: &AccountCommand) -> Result<()> {
    match cmd {
        AccountCommand::Connect {
            target,
            chrome_profile,
            api_key,
        } => operations::connect_target(target, chrome_profile.as_deref(), *api_key).await,
        AccountCommand::Disconnect { target } => operations::disconnect_target(target).await,
    }
}
