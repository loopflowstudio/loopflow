use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lf::RouteCommand;
use crate::profile::{ProviderRoute, RouteScope};
use crate::provider_account::{account_login, match_account, open_account_store, AccountMatch};
use crate::provider_auth::Provider;
use crate::repository::RepoId;
use crate::store::{ProviderAccount, ProviderAccountId, SharedStore};

pub(super) async fn run_route_async(
    cmd: Option<&RouteCommand>,
    repo: Option<&str>,
    default: bool,
    json: bool,
) -> Result<()> {
    match cmd {
        Some(RouteCommand::Set {
            provider,
            accounts,
            repo,
            default,
        }) => {
            let scope = if *default {
                RouteScope::Default
            } else {
                RouteScope::Repo(resolve_repo_id(repo.as_deref())?.ok_or_else(|| anyhow!(
                    "Run lf account route set in a repository with an origin, or pass --repo owner/name or --default."
                ))?)
            };
            let store = open_account_store().await?;
            set_route(&store, scope, provider, accounts).await
        }
        None => {
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
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RouteReport {
    provider: Provider,
    scope: String,
    ambient: bool,
    candidates: Vec<RouteAccount>,
    diagnostic: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RouteAccount {
    account_id: ProviderAccountId,
    login: Option<String>,
    source: String,
    credential_state: String,
    routing: String,
    cooldown_until: Option<i64>,
    demotion: Option<String>,
}

async fn show_routes(
    store: Option<&SharedStore>,
    repo_id: Option<&RepoId>,
    json: bool,
) -> Result<()> {
    let mut reports = Vec::new();
    for provider in [Provider::Claude, Provider::Codex] {
        let scope = match store {
            Some(store) => {
                let route = match repo_id {
                    Some(repo) => {
                        store
                            .provider_route(&RouteScope::Repo(repo.clone()), provider)
                            .await?
                    }
                    None => None,
                };
                if let Some(route) = route {
                    route.scope.id().to_string()
                } else if store
                    .provider_route(&RouteScope::Default, provider)
                    .await?
                    .is_some()
                {
                    "default".into()
                } else {
                    "automatic".into()
                }
            }
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
        let local_limits = match store {
            Some(store) => {
                store
                    .provider_account_limits(Some(provider.as_str()))
                    .await?
            }
            None => vec![],
        };
        let forwarded_client = crate::provider_account::lease::AccountLeaseClient::from_env()?;
        let mut candidates = Vec::new();
        for (account, forwarded) in accounts.unwrap_or_default() {
            let remote_limits = if forwarded {
                Some(
                    forwarded_client
                        .as_ref()
                        .expect("forwarded candidate has a lease")
                        .account_facts(provider, &account.account_id)?
                        .limits,
                )
            } else {
                None
            };
            let limits = remote_limits.as_ref().unwrap_or(&local_limits);
            let demotion = crate::provider_account::active_account_strain(
                provider.as_str(),
                &account.account_id,
                limits,
                now_unix(),
            )
            .map(|strain| format!("{} {}% used", strain.window, strain.used_percent));
            candidates.push(RouteAccount {
                account_id: account.account_id,
                login: account.login_email.map(|email| email.to_string()),
                source: if forwarded {
                    "forwarded_origin"
                } else {
                    "local"
                }
                .into(),
                credential_state: account.credential_state.as_str().into(),
                routing: account.routing_state.as_str().into(),
                cooldown_until: account.cooldown_until,
                demotion,
            });
        }
        reports.push(RouteReport {
            provider,
            scope,
            ambient,
            candidates,
            diagnostic,
        });
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        for report in reports {
            println!("{} ({})", report.provider, report.scope);
            if crate::provider_account::activation::launch_isolated(report.provider) {
                println!("  isolated: each conversation runs in its account's own home");
            } else {
                let native =
                    crate::provider_account::activation::native_home(report.provider, None);
                let accounts = match store {
                    Some(store) => {
                        store
                            .list_provider_accounts(Some(report.provider.as_str()))
                            .await?
                    }
                    None => vec![],
                };
                let active =
                    crate::provider_account::activation::active_account(&native, &accounts)
                        .map(crate::provider_account::account_login)
                        .unwrap_or("no stored account");
                println!("  shared: {} is signed in as {active}", native.display());
            }
            if report.ambient {
                println!("  ambient");
            } else if report.candidates.is_empty() {
                println!("  no eligible managed account");
            }
            for (position, account) in report.candidates.iter().enumerate() {
                println!(
                    "  {}. {} · {} · {} · {}",
                    position + 1,
                    account.account_id,
                    account.login.as_deref().unwrap_or("login unknown"),
                    account.credential_state,
                    account.source
                );
                if let Some(demotion) = &account.demotion {
                    println!("     demoted: {demotion}");
                }
            }
        }
    }
    Ok(())
}

async fn set_route(
    store: &SharedStore,
    scope: RouteScope,
    raw_provider: &str,
    raw_accounts: &[String],
) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
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
            scope: scope.clone(),
            provider,
            accounts: account_ids,
            created_at: now,
            updated_at: now,
        })
        .await?;
    let label = match scope {
        RouteScope::Repo(repo_id) => repo_id.to_string(),
        RouteScope::Default => "default".to_string(),
    };
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
    fn route_json_preserves_account_identity_origin_and_unknowns() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_routes.json");
        let reports: Vec<RouteReport> = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            serde_json::to_value(&reports).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        assert_eq!(reports[0].candidates[0].account_id.as_str(), "engineering");
        assert_eq!(
            reports[0].candidates[0].login.as_deref(),
            Some("engineering@example.com")
        );
        assert!(reports[1].ambient);
        assert!(reports[1].candidates.is_empty());
    }
}
