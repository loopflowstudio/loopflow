//! Saved service authentication. Native agent logins remain with their clients.
use crate::provider_auth::{Provider, ProviderAuthService};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountReport {
    pub accounts: Vec<AccountRow>,
}

#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRow {
    pub provider: Provider,
    #[serde(deserialize_with = "Option::deserialize")]
    pub login: Option<String>,
    pub state: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub expires_at: Option<i64>,
}

/// What this Machine last saved for each service; no provider is contacted.
pub async fn report(target: Option<&str>) -> Result<AccountReport> {
    let filter = target.map(super::parse_provider).transpose()?;
    let service = ProviderAuthService::new(super::local_store().await?);
    let mut accounts = Vec::new();
    for provider in Provider::all()
        .into_iter()
        .filter(|p| !matches!(p, Provider::Claude | Provider::Codex))
    {
        if filter.is_some_and(|filter| filter != provider) {
            continue;
        }
        let snapshot = service.cached_status(provider).await?;
        accounts.push(AccountRow {
            provider,
            login: snapshot.as_ref().and_then(|s| s.status.login()),
            state: snapshot
                .as_ref()
                .map_or("none", |s| s.status.as_str())
                .to_owned(),
            expires_at: snapshot.and_then(|s| s.expires_at),
        });
    }
    Ok(AccountReport { accounts })
}

pub async fn run(target: Option<&str>, json: bool) -> Result<()> {
    let report = report(target).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for account in report.accounts {
            println!(
                "{}: {}{}",
                account.provider,
                account.state,
                account
                    .login
                    .map(|login| format!(" ({login})"))
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}
