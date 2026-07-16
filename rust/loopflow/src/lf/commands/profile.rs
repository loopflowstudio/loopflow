use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use time::OffsetDateTime;

use crate::lf::{ProfileAccountCommand, ProfileCommand, ProfileRouteCommand};
use crate::profile::{
    resolve_local_chrome_profile, ChromeProfileBinding, HostId, Profile, ProfileId,
    ProfileProviderAccount, RepoProfileRoute,
};
use crate::provider_account::open_account_store;
use crate::provider_auth::Provider;
use crate::repository::RepoId;
use crate::store::{ProviderAccount, ProviderAccountId, SharedStore};

pub fn run(cmd: &ProfileCommand, repo_root: &Path) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    runtime.block_on(run_async(cmd, repo_root))
}

async fn run_async(cmd: &ProfileCommand, repo_root: &Path) -> Result<()> {
    let store = open_account_store().await?;
    match cmd {
        ProfileCommand::Create {
            profile,
            chrome_profile,
        } => create_profile(&store, profile, chrome_profile.as_deref()).await,
        ProfileCommand::List => list_profiles(&store).await,
        ProfileCommand::Account { cmd } => match cmd {
            ProfileAccountCommand::Set {
                profile,
                provider,
                account,
            } => set_profile_account(&store, profile, provider, account).await,
        },
        ProfileCommand::Route { cmd } => match cmd {
            ProfileRouteCommand::Set {
                default,
                backups,
                repo,
            } => set_repo_route(&store, repo_root, repo.as_deref(), default, backups).await,
            ProfileRouteCommand::Show { repo } => {
                show_repo_route(&store, repo_root, repo.as_deref()).await
            }
        },
    }
}

async fn create_profile(
    store: &SharedStore,
    raw_profile: &str,
    raw_chrome_profile: Option<&str>,
) -> Result<()> {
    let profile_id = parse_profile_id(raw_profile)?;
    let chrome_profile = raw_chrome_profile
        .map(resolve_local_chrome_profile)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    if let Some(chrome_profile) = &chrome_profile {
        let login = chrome_profile.login.as_deref().ok_or_else(|| {
            anyhow!(
                "Chrome profile '{}' has no signed-in account",
                raw_chrome_profile.expect("resolved Chrome profile has an input")
            )
        })?;
        if !login.eq_ignore_ascii_case(profile_id.as_str()) {
            return Err(anyhow!(
                "Chrome profile '{}' is signed in as '{}', not '{}'",
                raw_chrome_profile.expect("resolved Chrome profile has an input"),
                login,
                profile_id
            ));
        }
    }
    let now = now_unix();
    let existed = store.get_profile(&profile_id).await?.is_some();
    store
        .upsert_profile(&Profile {
            id: profile_id.clone(),
            created_at: now,
            updated_at: now,
        })
        .await?;
    if let Some(chrome_profile) = chrome_profile {
        store
            .upsert_chrome_profile_binding(&ChromeProfileBinding {
                profile_id: profile_id.clone(),
                host_id: HostId::local().map_err(|error| anyhow!(error))?,
                chrome_directory: chrome_profile.directory,
                created_at: now,
                updated_at: now,
            })
            .await?;
    }
    println!(
        "{} profile '{}'",
        if existed { "Updated" } else { "Created" },
        profile_id
    );
    Ok(())
}

async fn list_profiles(store: &SharedStore) -> Result<()> {
    let profiles = store.list_profiles().await?;
    let host_id = HostId::local().map_err(|error| anyhow!(error))?;
    let mappings = store.list_profile_provider_accounts(None).await?;
    let accounts = store.list_provider_accounts(None).await?;
    let accounts = accounts
        .into_iter()
        .map(|account| {
            (
                (account.provider.clone(), account.account_id.clone()),
                account,
            )
        })
        .collect::<HashMap<_, _>>();
    for profile in profiles {
        println!("{}", profile.id);
        if let Some(binding) = store.chrome_profile_binding(&profile.id, &host_id).await? {
            println!("  chrome  {}", binding.chrome_directory);
        }
        for mapping in mappings
            .iter()
            .filter(|mapping| mapping.profile_id == profile.id)
        {
            let label = accounts
                .get(&(
                    mapping.provider.as_str().to_string(),
                    mapping.account_id.clone(),
                ))
                .and_then(|account| account.login_email.as_ref().map(|email| email.as_str()))
                .unwrap_or(mapping.account_id.as_str());
            println!("  {:<7} {}", mapping.provider.as_str(), label);
        }
    }
    Ok(())
}

async fn set_profile_account(
    store: &SharedStore,
    raw_profile: &str,
    raw_provider: &str,
    raw_account: &str,
) -> Result<()> {
    let profile_id = parse_profile_id(raw_profile)?;
    if store.get_profile(&profile_id).await?.is_none() {
        return Err(anyhow!(
            "profile '{}' does not exist; run 'lf profile create {}' first",
            profile_id,
            profile_id
        ));
    }
    let provider = parse_managed_provider(raw_provider)?;
    let account = find_provider_account(store, provider, raw_account).await?;
    let now = now_unix();
    store
        .set_profile_provider_account(&ProfileProviderAccount {
            profile_id: profile_id.clone(),
            provider,
            account_id: account.account_id.clone(),
            created_at: now,
            updated_at: now,
        })
        .await?;
    println!(
        "Mapped profile '{}' {} to {}",
        profile_id,
        provider,
        account
            .login_email
            .as_ref()
            .map(|email| email.as_str())
            .unwrap_or(account.account_id.as_str())
    );
    Ok(())
}

async fn set_repo_route(
    store: &SharedStore,
    repo_root: &Path,
    raw_repo: Option<&str>,
    raw_default: &str,
    raw_backups: &[String],
) -> Result<()> {
    let repo_id = resolve_repo_id(repo_root, raw_repo)?;
    let default_profile = parse_profile_id(raw_default)?;
    let backup_profiles = raw_backups
        .iter()
        .map(|profile| parse_profile_id(profile))
        .collect::<Result<Vec<_>>>()?;
    for profile_id in std::iter::once(&default_profile).chain(backup_profiles.iter()) {
        if store.get_profile(profile_id).await?.is_none() {
            return Err(anyhow!("profile '{}' does not exist", profile_id));
        }
    }
    let now = now_unix();
    store
        .set_repo_profile_route(&RepoProfileRoute {
            repo_id: repo_id.clone(),
            default_profile: default_profile.clone(),
            backup_profiles: backup_profiles.clone(),
            created_at: now,
            updated_at: now,
        })
        .await?;
    println!(
        "{}: {}",
        repo_id,
        format_route(&default_profile, &backup_profiles)
    );
    Ok(())
}

async fn show_repo_route(
    store: &SharedStore,
    repo_root: &Path,
    raw_repo: Option<&str>,
) -> Result<()> {
    let repo_id = resolve_repo_id(repo_root, raw_repo)?;
    let route = store
        .repo_profile_route(&repo_id)
        .await?
        .ok_or_else(|| anyhow!("no profile route configured for {repo_id}"))?;
    println!(
        "{}: {}",
        repo_id,
        format_route(&route.default_profile, &route.backup_profiles)
    );

    let accounts = store.list_provider_accounts(None).await?;
    let mut seen: HashMap<(Provider, ProviderAccountId), ProfileId> = HashMap::new();
    let profiles = std::iter::once(&route.default_profile).chain(route.backup_profiles.iter());
    for profile_id in profiles {
        println!("  {profile_id}");
        for provider in [Provider::Claude, Provider::Codex] {
            let Some(mapping) = store.profile_provider_account(profile_id, provider).await? else {
                println!("    {:<7} —", provider.as_str());
                continue;
            };
            let label = account_label(&accounts, provider, &mapping.account_id);
            let key = (provider, mapping.account_id.clone());
            match seen.get(&key) {
                Some(shared_with) => println!(
                    "    {:<7} {} (shared with {})",
                    provider.as_str(),
                    label,
                    shared_with
                ),
                None => {
                    println!("    {:<7} {}", provider.as_str(), label);
                    seen.insert(key, profile_id.clone());
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn find_provider_account(
    store: &SharedStore,
    provider: Provider,
    raw_account: &str,
) -> Result<ProviderAccount> {
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let login_matches = accounts
        .iter()
        .filter(|account| {
            account
                .login_email
                .as_ref()
                .map(|email| email.as_str())
                .is_some_and(|login| login.eq_ignore_ascii_case(raw_account.trim()))
        })
        .cloned()
        .collect::<Vec<_>>();
    match login_matches.as_slice() {
        [account] => return Ok(account.clone()),
        [_, ..] => {
            return Err(anyhow!(
                "{} login '{}' matches multiple managed accounts; use an account id",
                provider,
                raw_account
            ));
        }
        [] => {}
    }
    let account_id = ProviderAccountId::parse(raw_account).map_err(|_| {
        anyhow!(
            "no managed {} account has login '{}'",
            provider,
            raw_account.trim()
        )
    })?;
    accounts
        .into_iter()
        .find(|account| account.account_id == account_id)
        .ok_or_else(|| {
            anyhow!(
                "managed {} account '{}' does not exist",
                provider,
                account_id
            )
        })
}

fn resolve_repo_id(repo_root: &Path, raw_repo: Option<&str>) -> Result<RepoId> {
    match raw_repo {
        Some(repo) => RepoId::parse(repo).map_err(|error| anyhow!(error)),
        None => RepoId::discover(repo_root).map_err(|error| anyhow!(error)),
    }
}

fn account_label<'a>(
    accounts: &'a [ProviderAccount],
    provider: Provider,
    account_id: &'a ProviderAccountId,
) -> &'a str {
    accounts
        .iter()
        .find(|account| account.provider == provider.as_str() && account.account_id == *account_id)
        .and_then(|account| account.login_email.as_ref().map(|email| email.as_str()))
        .unwrap_or(account_id.as_str())
}

fn format_route(default_profile: &ProfileId, backups: &[ProfileId]) -> String {
    let mut route = vec![default_profile.as_str()];
    route.extend(backups.iter().map(ProfileId::as_str));
    route.join(" -> ")
}

fn parse_profile_id(value: &str) -> Result<ProfileId> {
    ProfileId::parse(value).map_err(|error| anyhow!(error))
}

fn parse_managed_provider(value: &str) -> Result<Provider> {
    let provider = value.parse::<Provider>()?;
    if matches!(provider, Provider::Claude | Provider::Codex) {
        Ok(provider)
    } else {
        Err(anyhow!("profiles support Claude and Codex OAuth only"))
    }
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use super::format_route;
    use crate::profile::ProfileId;

    #[test]
    fn route_format_preserves_declared_order() {
        assert_eq!(
            format_route(
                &ProfileId::parse("primary@example.com").unwrap(),
                &[
                    ProfileId::parse("engineering@example.com").unwrap(),
                    ProfileId::parse("personal@example.com").unwrap(),
                ],
            ),
            "primary@example.com -> engineering@example.com -> personal@example.com"
        );
    }
}
