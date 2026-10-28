use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::{ProviderRoute, RouteScope};
use crate::provider_account::{account_login, match_account, nearest_provider_route, AccountMatch};
use crate::provider_auth::Provider;
use crate::repository::RepoId;
use crate::store::{ProviderAccount, ProviderAccountId, SharedStore};

pub(super) async fn show_route(repo: Option<&str>, default: bool, json: bool) -> Result<()> {
    let repo_id = if default {
        None
    } else {
        resolve_repo_id(repo)?
    };
    show_routes(
        crate::provider_account::read_account_store()?.as_ref(),
        repo_id.as_ref(),
        json,
    )
    .await
}

#[derive(Debug, Serialize, Deserialize)]
struct RouteReport {
    provider: Provider,
    scope: String,
    ambient: bool,
    /// Each conversation runs in its account's own home rather than the
    /// provider's native one.
    isolated: bool,
    /// The stored account the native home is signed in as; absent when
    /// isolated or when its login belongs to no stored account.
    active_account: Option<ProviderAccountId>,
    candidates: Vec<RouteAccount>,
    diagnostic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RouteAccount {
    account_id: ProviderAccountId,
    login: Option<String>,
    credential_state: String,
    routing: String,
    cooldown_until: Option<i64>,
    /// The account's most-used window that has not reset.
    tightest: Option<String>,
}

async fn show_routes(
    store: Option<&SharedStore>,
    repo_id: Option<&RepoId>,
    json: bool,
) -> Result<()> {
    let mut reports = Vec::new();
    for provider in [Provider::Claude, Provider::Codex] {
        let route = match store {
            Some(store) => nearest_provider_route(store, repo_id, None, provider).await?,
            None => None,
        };
        let scope = match route {
            Some(route) if route.scope == RouteScope::Default => "default".into(),
            Some(route) => route.scope.id().to_owned(),
            None => "automatic".into(),
        };
        let (accounts, diagnostic) =
            match crate::provider_account::inspect_provider_route(store, repo_id, provider).await {
                Ok(accounts) => (accounts, None),
                Err(crate::provider_account::ProviderAccountError::NoEligibleAccount {
                    ..
                }) => (
                    Some(vec![]),
                    Some("no eligible account in the selected route".into()),
                ),
                Err(error) => return Err(error.into()),
            };
        let ambient = accounts.is_none();
        let limits = match store {
            Some(store) => {
                store
                    .provider_account_limits(Some(provider.as_str()))
                    .await?
            }
            None => vec![],
        };
        let mut candidates = Vec::new();
        for account in accounts.unwrap_or_default() {
            let tightest = crate::provider_account::tightest_window(
                provider.as_str(),
                &account.account_id,
                &limits,
                now_unix(),
            )
            .map(|limit| format!("{} {}% used", limit.window, limit.used_percent));
            candidates.push(RouteAccount {
                account_id: account.account_id,
                login: account.login_email.map(|email| email.to_string()),
                credential_state: account.credential_state.as_str().into(),
                routing: account.routing_state.as_str().into(),
                cooldown_until: account.cooldown_until,
                tightest,
            });
        }
        let isolated = crate::provider_account::activation::launch_isolated();
        let active_account = match store {
            Some(store) if !isolated => {
                let stored = store
                    .list_provider_accounts(Some(provider.as_str()))
                    .await?;
                crate::provider_account::activation::active_account(
                    provider,
                    &crate::provider_account::activation::native_home(provider, None),
                    &stored,
                )
                .map(|account| account.account_id.clone())
            }
            _ => None,
        };
        reports.push(RouteReport {
            provider,
            scope,
            ambient,
            isolated,
            active_account,
            candidates,
            diagnostic,
        });
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        for report in reports {
            println!("{} ({})", report.provider, report.scope);
            if report.isolated {
                println!("  isolated: each conversation runs in its account's own home");
            } else {
                let active = report.active_account.as_ref().map(|active| {
                    report
                        .candidates
                        .iter()
                        .find(|account| account.account_id == *active)
                        .and_then(|account| account.login.clone())
                        .unwrap_or_else(|| active.to_string())
                });
                println!(
                    "  shared: {} is signed in as {}",
                    crate::provider_account::activation::native_home(report.provider, None)
                        .display(),
                    active.as_deref().unwrap_or("no stored account")
                );
            }
            if report.ambient {
                println!("  ambient");
            } else if report.candidates.is_empty() {
                println!("  no eligible managed account");
            }
            for (position, account) in report.candidates.iter().enumerate() {
                println!(
                    "  {}. {} · {} · {}",
                    position + 1,
                    account.account_id,
                    account.login.as_deref().unwrap_or("login unknown"),
                    account.credential_state
                );
                if let Some(tightest) = &account.tightest {
                    println!("     {tightest}");
                }
            }
        }
    }
    Ok(())
}

/// Replace `provider`'s route at `scope` with the logins in order.
pub(crate) async fn set_route(
    store: &SharedStore,
    scope: RouteScope,
    label: &str,
    provider: Provider,
    raw_accounts: &[&str],
) -> Result<()> {
    let mut accounts = Vec::new();
    for raw_account in raw_accounts {
        accounts.push(find_provider_account(store, provider, raw_account).await?);
    }
    let account_ids = accounts
        .iter()
        .map(|account| account.account_id.clone())
        .collect();
    let now = now_unix();
    store
        .set_provider_route(&ProviderRoute {
            scope,
            provider,
            accounts: account_ids,
            created_at: now,
            updated_at: now,
        })
        .await?;
    println!(
        "{label} {provider}: {}",
        accounts
            .iter()
            .map(account_login)
            .collect::<Vec<_>>()
            .join(" -> ")
    );
    Ok(())
}

pub(crate) async fn find_provider_account(
    store: &SharedStore,
    provider: Provider,
    raw_email: &str,
) -> Result<ProviderAccount> {
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let candidates = accounts.iter().collect::<Vec<_>>();
    match match_account(&candidates, raw_email.trim()) {
        AccountMatch::One(account) => Ok(account.clone()),
        AccountMatch::None => Err(anyhow!(
            "managed {} login '{}' does not exist",
            provider,
            raw_email.trim()
        )),
        AccountMatch::Ambiguous(accounts) => Err(anyhow!(
            "{} login prefix '{}' is ambiguous: {}",
            provider,
            raw_email.trim(),
            accounts
                .iter()
                .map(|account| account_login(account))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

pub(crate) fn parse_managed_provider(raw: &str) -> Result<Provider> {
    let provider = raw.parse::<Provider>()?;
    if matches!(provider, Provider::Claude | Provider::Codex) {
        Ok(provider)
    } else {
        Err(anyhow!(
            "managed OAuth accounts support Claude and Codex only"
        ))
    }
}

fn resolve_repo_id(raw_repo: Option<&str>) -> Result<Option<RepoId>> {
    match raw_repo {
        Some(repo) => Ok(Some(RepoId::parse(repo).map_err(|error| anyhow!(error))?)),
        None => {
            let Some(root) = crate::repo::discover_repo_root(&std::env::current_dir()?)? else {
                return Ok(None);
            };
            // A local-only repository has no provider route identity yet.
            let origin = std::process::Command::new("git")
                .args(["config", "--get", "remote.origin.url"])
                .current_dir(&root)
                .output()?;
            if origin.status.code() == Some(1) {
                return Ok(None);
            }
            if !origin.status.success() {
                return Err(anyhow!(
                    "cannot read repository origin: {}",
                    String::from_utf8_lossy(&origin.stderr).trim()
                ));
            }
            Ok(Some(RepoId::discover(&root)?))
        }
    }
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use super::RouteReport;

    #[test]
    fn route_json_preserves_account_identity_and_unknowns() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_routes.json");
        let reports: Vec<RouteReport> = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            serde_json::to_value(&reports).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        assert!(reports[0].isolated && reports[0].active_account.is_none());
        assert_eq!(
            reports[1].active_account.as_ref().map(|id| id.as_str()),
            Some("personal")
        );
        let mut missing: serde_json::Value = serde_json::from_str(fixture).unwrap();
        missing[0].as_object_mut().unwrap().remove("isolated");
        assert!(serde_json::from_value::<Vec<RouteReport>>(missing).is_err());
        assert_eq!(reports[0].candidates[0].account_id.as_str(), "engineering");
        assert_eq!(
            reports[0].candidates[0].login.as_deref(),
            Some("engineering@example.com")
        );
        assert!(reports[1].ambient);
        assert!(reports[1].candidates.is_empty());
    }
}
