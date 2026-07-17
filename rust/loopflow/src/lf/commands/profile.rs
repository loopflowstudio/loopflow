use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use time::OffsetDateTime;

use crate::lf::{DefaultRouteCommand, ProfileCommand, RouteCommand};
use crate::profile::{
    resolve_local_chrome_profile, AccessProfile, EmailAddress, ProfileId, ProviderRoute, RouteScope,
};
use crate::provider_account::{
    account_is_automatic_candidate, account_unavailable_reason, active_account_strain,
    open_account_store, order_accounts_by_strain, STRAINED_UTILIZATION_PERCENT,
};
use crate::provider_auth::Provider;
use crate::repository::RepoId;
use crate::store::{ProviderAccount, ProviderAccountId, SharedStore};

pub fn run(cmd: &ProfileCommand, _repo_root: &Path) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    runtime.block_on(run_async(cmd))
}

pub fn run_route(cmd: &RouteCommand, repo_root: &Path) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    runtime.block_on(run_route_async(cmd, repo_root))
}

async fn run_async(cmd: &ProfileCommand) -> Result<()> {
    let store = open_account_store().await?;
    match cmd {
        ProfileCommand::Create {
            chrome_profile,
            name,
            expects,
        } => create_profile(&store, chrome_profile, name.as_deref(), expects.as_deref()).await,
        ProfileCommand::List => list_profiles(&store).await,
    }
}

async fn run_route_async(cmd: &RouteCommand, repo_root: &Path) -> Result<()> {
    let store = open_account_store().await?;
    match cmd {
        RouteCommand::Set {
            provider,
            accounts,
            repo,
        } => {
            let repo_id = resolve_repo_id(repo_root, repo.as_deref())?;
            set_route(&store, RouteScope::Repo(repo_id), provider, accounts).await
        }
        RouteCommand::Default { cmd } => match cmd {
            DefaultRouteCommand::Set { provider, accounts } => {
                set_route(&store, RouteScope::Default, provider, accounts).await
            }
        },
        RouteCommand::Show { repo, explain } => {
            show_routes(&store, repo_root, repo.as_deref(), *explain).await
        }
    }
}

async fn create_profile(
    store: &SharedStore,
    requested_chrome_profile: &str,
    raw_name: Option<&str>,
    raw_expected: Option<&str>,
) -> Result<()> {
    let chrome_profile =
        resolve_local_chrome_profile(requested_chrome_profile).map_err(anyhow::Error::msg)?;
    let live_login = chrome_profile.login.as_deref().ok_or_else(|| {
        anyhow!(
            "Chrome profile '{}' has no signed-in account",
            requested_chrome_profile
        )
    })?;
    let expected_login =
        EmailAddress::parse(raw_expected.unwrap_or(live_login)).map_err(anyhow::Error::msg)?;
    if !live_login.eq_ignore_ascii_case(expected_login.as_str()) {
        return Err(anyhow!(
            "Chrome profile '{}' is signed in as '{}', not '{}'",
            chrome_profile.name,
            live_login,
            expected_login
        ));
    }
    let profile_id = ProfileId::parse(raw_name.unwrap_or(&chrome_profile.directory))
        .map_err(anyhow::Error::msg)?;
    let now = now_unix();
    let existed = store.get_access_profile(&profile_id).await?.is_some();
    store
        .upsert_access_profile(&AccessProfile {
            id: profile_id.clone(),
            chrome_directory: chrome_profile.directory,
            expected_login,
            created_at: now,
            updated_at: now,
        })
        .await?;
    println!(
        "{} access profile '{}'",
        if existed { "Updated" } else { "Created" },
        profile_id
    );
    Ok(())
}

async fn list_profiles(store: &SharedStore) -> Result<()> {
    let profiles = store.list_access_profiles().await?;
    let mappings = store.list_account_access_profiles(None, None).await?;
    for profile in profiles {
        let actual = resolve_local_chrome_profile(&profile.chrome_directory)
            .ok()
            .and_then(|profile| profile.login)
            .unwrap_or_else(|| "not signed in".to_string());
        println!(
            "{}  chrome={}  expects={}  signed-in={}",
            profile.id, profile.chrome_directory, profile.expected_login, actual
        );
        for mapping in mappings
            .iter()
            .filter(|mapping| mapping.profile_id == profile.id)
        {
            println!(
                "  {}/{} (position {})",
                mapping.provider,
                mapping.account_id,
                mapping.position + 1
            );
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
        accounts.push(
            find_provider_account(store, provider, raw_account)
                .await?
                .account_id,
        );
    }
    let now = now_unix();
    store
        .set_provider_route(&ProviderRoute {
            scope: scope.clone(),
            provider,
            accounts: accounts.clone(),
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
            .map(ProviderAccountId::as_str)
            .collect::<Vec<_>>()
            .join(" -> ")
    );
    Ok(())
}

async fn show_routes(
    store: &SharedStore,
    repo_root: &Path,
    raw_repo: Option<&str>,
    explain: bool,
) -> Result<()> {
    let repo_id = resolve_repo_id(repo_root, raw_repo)?;
    let accounts = store
        .list_provider_accounts(None)
        .await?
        .into_iter()
        .map(|account| {
            (
                (account.provider.clone(), account.account_id.clone()),
                account,
            )
        })
        .collect::<HashMap<_, _>>();
    let limits = if explain {
        store.provider_account_limits(None).await?
    } else {
        Vec::new()
    };
    let now = now_unix();
    for provider in [Provider::Claude, Provider::Codex] {
        let repo_scope = RouteScope::Repo(repo_id.clone());
        let (route, fallback) = match store.provider_route(&repo_scope, provider).await? {
            Some(route) => (Some(route), false),
            None => (
                store.provider_route(&RouteScope::Default, provider).await?,
                true,
            ),
        };
        let Some(route) = route else {
            println!("{provider}  (ambient; no repo or default route)");
            continue;
        };
        if fallback {
            println!("{provider}  (default route — this repo has no route)");
        } else {
            println!("{provider}  ({repo_id})");
        }
        if explain {
            for line in route_explanation(provider, &route, &accounts, &limits, now) {
                println!("{line}");
            }
            continue;
        }
        for (position, account_id) in route.accounts.iter().enumerate() {
            let account = accounts.get(&(provider.as_str().to_string(), account_id.clone()));
            let login = account
                .and_then(|account| account.login_email.as_ref())
                .map(EmailAddress::as_str)
                .unwrap_or("—");
            let state = account
                .map(|account| account.credential_state.as_str())
                .unwrap_or("missing row");
            println!(
                "  {}. {:<20} {:<32} {}",
                position + 1,
                account_id,
                login,
                state
            );
        }
    }
    Ok(())
}

fn route_explanation(
    provider: Provider,
    route: &ProviderRoute,
    accounts: &HashMap<(String, ProviderAccountId), ProviderAccount>,
    limits: &[crate::store::AccountLimitRow],
    now: i64,
) -> Vec<String> {
    let today = OffsetDateTime::from_unix_timestamp(now)
        .expect("route preview timestamp is valid")
        .date();
    let mut lines = Vec::new();
    let mut available = Vec::new();
    for (position, account_id) in route.accounts.iter().enumerate() {
        let Some(account) = accounts.get(&(provider.as_str().to_string(), account_id.clone()))
        else {
            lines.push(format!("  {}. {account_id:<20} missing row", position + 1));
            continue;
        };
        if !account_is_automatic_candidate(account, now, today) {
            lines.push(format!(
                "  {}. {account_id:<20} excluded: {}",
                position + 1,
                account_unavailable_reason(account, now, today)
            ));
            continue;
        }
        available.push(account.clone());
        match active_account_strain(provider.as_str(), account_id, limits, now) {
            Some(strain) => lines.push(format!(
                "  {}. {account_id:<20} {} {}%  demoted: strained (>= {}%, resets {})",
                position + 1,
                strain.window,
                strain.used_percent,
                STRAINED_UTILIZATION_PERCENT,
                format_reset_in(strain.resets_at, now)
            )),
            None => lines.push(format!("  {}. {account_id:<20} ready", position + 1)),
        }
    }
    order_accounts_by_strain(&mut available, limits, now);
    lines.push(format!(
        "  effective order: {}",
        available
            .iter()
            .map(|account| account.account_id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    lines.push(match available.first() {
        Some(account) => format!(
            "  next unpinned selection: {provider}/{}",
            account.account_id
        ),
        None => "  next unpinned selection: none".to_string(),
    });
    lines
}

fn format_reset_in(resets_at: i64, now: i64) -> String {
    let seconds = (resets_at - now).max(0);
    if seconds >= 86_400 {
        format!("in {}d", seconds / 86_400)
    } else if seconds >= 3_600 {
        format!("in {}h", seconds / 3_600)
    } else {
        format!("in {}m", seconds / 60)
    }
}

pub(crate) async fn find_provider_account(
    store: &SharedStore,
    provider: Provider,
    raw_account: &str,
) -> Result<ProviderAccount> {
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let matches = accounts
        .iter()
        .filter(|account| {
            account
                .account_id
                .as_str()
                .eq_ignore_ascii_case(raw_account.trim())
                || account
                    .login_email
                    .as_ref()
                    .is_some_and(|email| email.as_str().eq_ignore_ascii_case(raw_account.trim()))
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [account] => Ok(account.clone()),
        [] => Err(anyhow!(
            "managed {} account '{}' does not exist",
            provider,
            raw_account.trim()
        )),
        [_, ..] => Err(anyhow!(
            "{} account '{}' is ambiguous; use an account id",
            provider,
            raw_account.trim()
        )),
    }
}

pub(crate) fn parse_managed_provider(raw: &str) -> Result<Provider> {
    let provider = raw
        .parse::<Provider>()
        .map_err(|error| anyhow!(error.to_string()))?;
    if matches!(provider, Provider::Claude | Provider::Codex) {
        Ok(provider)
    } else {
        Err(anyhow!(
            "managed OAuth accounts support Claude and Codex only"
        ))
    }
}

fn resolve_repo_id(repo_root: &Path, raw_repo: Option<&str>) -> Result<RepoId> {
    match raw_repo {
        Some(repo) => RepoId::parse(repo).map_err(|error| anyhow!(error)),
        None => RepoId::discover(repo_root).map_err(|error| anyhow!(error)),
    }
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{AccountLimitRow, CredentialState, RoutingState};

    fn account(account_id: &str) -> ProviderAccount {
        ProviderAccount {
            provider: Provider::Codex.as_str().to_string(),
            account_id: ProviderAccountId::parse(account_id).unwrap(),
            home: None,
            login_email: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn explanation_previews_demotion_without_selecting() {
        let now = 1_000_000;
        let first = account("strained");
        let second = account("healthy");
        let route = ProviderRoute {
            scope: RouteScope::Default,
            provider: Provider::Codex,
            accounts: vec![first.account_id.clone(), second.account_id.clone()],
            created_at: 1,
            updated_at: 1,
        };
        let accounts = [first.clone(), second.clone()]
            .into_iter()
            .map(|account| {
                (
                    (account.provider.clone(), account.account_id.clone()),
                    account,
                )
            })
            .collect();
        let limits = vec![AccountLimitRow {
            provider: Provider::Codex.as_str().to_string(),
            account_id: first.account_id.clone(),
            window: "weekly".to_string(),
            used_percent: 80,
            resets_at: Some(now + 86_400),
            plan: None,
            observed_at: now - 10,
            source: "poll".to_string(),
        }];

        let rendered =
            route_explanation(Provider::Codex, &route, &accounts, &limits, now).join("\n");

        assert!(rendered.contains("weekly 80%  demoted: strained"));
        assert!(rendered.contains("effective order: healthy, strained"));
        assert!(rendered.contains("next unpinned selection: codex/healthy"));
        assert_eq!(
            accounts[&("codex".to_string(), first.account_id)].last_selected_at,
            None
        );
        assert_eq!(
            accounts[&("codex".to_string(), second.account_id)].last_selected_at,
            None
        );
    }
}
