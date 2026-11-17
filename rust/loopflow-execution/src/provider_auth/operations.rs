//! Service authentication through `lf account`. Claude and Codex logins belong
//! to their native clients.
pub mod account_status;

use crate::profile::{AccessProfile, LocalChromeProfile, ProfileId};
use crate::provider_auth::{AuthCompletion, Provider, ProviderAuthService};
use crate::store::{open_store, CredentialType, ProviderToken, SharedStore};
use anyhow::{anyhow, bail, Context, Result};
use std::future::Future;
use std::io::IsTerminal;
use std::path::Path;
#[cfg(all(target_os = "macos", not(test)))]
use std::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use tokio::io::AsyncBufReadExt;

const AUTH_BROWSER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const AUTH_CODE_FLOW_TIMEOUT_SECS: u64 = 600;

tokio::task_local! { static SERVED: (); }
pub fn served<T>(operation: impl FnOnce() -> T) -> T {
    SERVED.sync_scope((), operation)
}
fn is_served() -> bool {
    SERVED.try_with(|()| ()).is_ok()
}
macro_rules! narrate {
    ($($line:tt)*) => {
        if is_served() { tracing::info!($($line)*); } else { println!($($line)*); }
    };
}

/// Connect a service: through its browser login, or from its API key variable.
pub async fn connect_target(
    target: &str,
    chrome_profile: Option<&str>,
    api_key: bool,
) -> Result<()> {
    let provider = parse_provider(target)?;
    if api_key {
        configure(provider.as_str()).await
    } else {
        connect(provider.as_str(), chrome_profile).await
    }
}

pub async fn disconnect_target(target: &str) -> Result<()> {
    disconnect(target).await
}

fn parse_provider(raw: &str) -> Result<Provider> {
    let provider: Provider = raw.parse()?;
    anyhow::ensure!(
        !matches!(provider, Provider::Claude | Provider::Codex),
        "{provider} authentication belongs to the native client"
    );
    Ok(provider)
}

async fn connect(raw_provider: &str, selection: Option<&str>) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let store = local_store().await?;
    let (profiles, remember) = browser_profiles(&store, provider, selection).await?;
    let mut failures = Vec::new();
    for profile in profiles {
        let chrome = match verified_chrome_profile(&profile) {
            Ok(chrome) => chrome,
            Err(error) => {
                failures.push(format!("{}: {error}", profile.id));
                continue;
            }
        };
        let service = local_auth_service().await?;
        let flow = service
            .start_auth(provider)
            .await
            .context("provider launch / URL discovery")?;
        let result = async {
            let url = flow.verification_uri_complete.as_deref().unwrap_or(&flow.verification_uri);
            narrate!("Connecting local {} through profile '{}'...", provider.display_name(), profile.id);
            if matches!(flow.completion, AuthCompletion::Manual) { narrate!("Manual authorization is required."); }
            open_chrome_profile(&chrome, url).context("browser launch")?;
            tokio::select! {
                biased;
                result = service.wait_for_auth(provider) => result.map_err(anyhow::Error::from),
                _ = tokio::time::sleep(Duration::from_secs(flow.expires_in.unwrap_or(AUTH_CODE_FLOW_TIMEOUT_SECS))) => Err(anyhow!("authorization wait timed out")),
                _ = tokio::signal::ctrl_c() => Err(anyhow!("authorization cancelled")),
            }
        }.await;
        service.abort_pending(provider).await;
        result?;
        if remember {
            remember_browser_profile(&store, provider, &profile).await?;
        }
        narrate!("Connected local {}", provider.display_name());
        return Ok(());
    }
    Err(anyhow!(
        "Chrome profile discovery: {}. Run lf account connect {provider} --chrome-profile <profile>",
        failures.join("; ")
    ))
}

async fn browser_profiles(
    store: &SharedStore,
    provider: Provider,
    selection: Option<&str>,
) -> Result<(Vec<AccessProfile>, bool)> {
    if let Some(selection) = selection {
        return Ok((
            vec![bootstrap_access_profile(store, selection).await?],
            true,
        ));
    }
    let bindings = store
        .list_auth_browser_profiles(Some(provider), None)
        .await?;
    let mut profiles = Vec::new();
    for binding in bindings
        .into_iter()
        .filter(|binding| binding.account_id.is_none())
    {
        profiles.push(
            store
                .get_access_profile(&binding.profile_id)
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "saved Chrome profile '{}' is missing; use --chrome-profile <profile>",
                        binding.profile_id
                    )
                })?,
        );
    }
    if !profiles.is_empty() {
        return Ok((profiles, false));
    }
    if is_served() || !std::io::stdin().is_terminal() {
        return Err(anyhow!(
            "No saved Chrome profile. Run lf account connect {provider} --chrome-profile <profile>"
        ));
    }
    let choices = crate::profile::local_chrome_profiles().map_err(anyhow::Error::msg)?;
    for (index, choice) in choices.iter().enumerate() {
        narrate!("{}. {} ({})", index + 1, choice.name, choice.directory);
    }
    narrate!("Choose a Chrome profile number (remembered after successful login):");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let choice = line
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .and_then(|i| choices.get(i))
        .ok_or_else(|| anyhow!("choose a listed Chrome profile with --chrome-profile <profile>"))?;
    Ok((
        vec![bootstrap_access_profile(store, &choice.directory).await?],
        true,
    ))
}

async fn remember_browser_profile(
    store: &SharedStore,
    provider: Provider,
    profile: &AccessProfile,
) -> Result<()> {
    if store.get_access_profile(&profile.id).await?.is_none() {
        store.upsert_access_profile(profile).await?;
    }
    store
        .set_auth_browser_profiles(provider, None, std::slice::from_ref(&profile.id))
        .await?;
    Ok(())
}

async fn bootstrap_access_profile(
    store: &SharedStore,
    raw_chrome_profile: &str,
) -> Result<AccessProfile> {
    if let Ok(id) = ProfileId::parse(raw_chrome_profile) {
        if let Some(profile) = store.get_access_profile(&id).await? {
            return Ok(profile);
        }
    }
    let chrome_profile = crate::profile::resolve_local_chrome_profile(raw_chrome_profile)
        .map_err(anyhow::Error::msg)?;
    let saved = store.list_access_profiles().await?;
    if let Some(profile) = saved
        .iter()
        .find(|p| p.chrome_directory == chrome_profile.directory)
    {
        return Ok(profile.clone());
    }
    let mut id = ProfileId::parse(&chrome_profile.name)
        .or_else(|_| ProfileId::parse(&chrome_profile.directory))
        .map_err(anyhow::Error::msg)?;
    if saved.iter().any(|p| p.id == id) {
        id = ProfileId::parse(&chrome_profile.directory).map_err(anyhow::Error::msg)?;
    }
    if saved.iter().any(|p| p.id == id) {
        return Err(anyhow!(
            "Chrome profile name and directory conflict with saved profiles"
        ));
    }
    Ok(AccessProfile {
        id,
        chrome_directory: chrome_profile.directory,
        expected_login: None,
        created_at: OffsetDateTime::now_utc().unix_timestamp(),
        updated_at: OffsetDateTime::now_utc().unix_timestamp(),
    })
}

fn verified_chrome_profile(profile: &AccessProfile) -> Result<LocalChromeProfile> {
    let chrome_profile = crate::profile::local_chrome_profiles()
        .map_err(anyhow::Error::msg)?
        .into_iter()
        .find(|chrome| chrome.directory == profile.chrome_directory)
        .ok_or_else(|| {
            anyhow!(
                "saved Chrome profile '{}' directory '{}' is missing; use --chrome-profile <profile>",
                profile.id,
                profile.chrome_directory
            )
        })?;
    verify_chrome_profile_login(profile, chrome_profile)
}

fn verify_chrome_profile_login(
    profile: &AccessProfile,
    chrome_profile: LocalChromeProfile,
) -> Result<LocalChromeProfile> {
    let Some(expected) = &profile.expected_login else {
        return Ok(chrome_profile);
    };
    let actual = chrome_profile.login.as_deref().ok_or_else(|| {
        anyhow!(
            "Chrome profile '{}' has no signed-in account",
            chrome_profile.name
        )
    })?;
    if !actual.eq_ignore_ascii_case(expected.as_str()) {
        return Err(anyhow!(
            "signed in as '{}', expected '{}'",
            actual,
            expected
        ));
    }
    Ok(chrome_profile)
}

async fn disconnect(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let service = local_auth_service().await?;
    service.disconnect(provider).await?;
    narrate!("Disconnected local {}", provider.display_name());
    Ok(())
}

async fn configure(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    if let Some(message) = provider.api_key_configure_error() {
        return Err(anyhow!(message));
    }
    let env_name = provider
        .api_key_env_name()
        .ok_or_else(|| anyhow!("{} does not support API key auth", provider.display_name()))?;
    let api_key = std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "{env_name} is not set. Export it, then run `lf account connect {} --api-key`.",
                provider.as_str()
            )
        })?;

    let store = local_store().await?;
    store
        .upsert_provider_token(&ProviderToken {
            provider: provider.as_str().to_string(),
            access_token: api_key,
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: OffsetDateTime::now_utc().unix_timestamp(),
            credential_type: CredentialType::ApiKey,
        })
        .await?;

    if provider.api_key_bills_per_token() {
        narrate!("API key auth bills per token. OAuth uses your existing subscription.");
    }
    narrate!("Stored {} API key", provider.display_name());
    Ok(())
}

async fn local_auth_service() -> Result<ProviderAuthService> {
    Ok(ProviderAuthService::new(local_store().await?))
}

pub async fn local_store() -> Result<SharedStore> {
    let cfg = crate::store::storage_config_from_env()
        .context("failed to resolve local credential store")?;
    let store = open_store(&cfg)
        .await
        .map_err(|err| anyhow!("failed to open local credential store: {err}"))?;
    Ok(Arc::new(store))
}

#[cfg(all(target_os = "macos", not(test)))]
fn open_chrome_profile(profile: &LocalChromeProfile, url: &str) -> Result<()> {
    let chrome = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    if !chrome.is_file() {
        return Err(anyhow!("Google Chrome is not installed in /Applications"));
    }
    Command::new(chrome)
        .arg(format!("--profile-directory={}", profile.directory))
        .arg("--new-window")
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("open matching Chrome profile")?;
    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(test)))]
fn open_chrome_profile(_profile: &LocalChromeProfile, _url: &str) -> Result<()> {
    Err(anyhow!(
        "opening a selected Chrome profile is currently supported on macOS only"
    ))
}

#[cfg(test)]
fn open_chrome_profile(_profile: &LocalChromeProfile, _url: &str) -> Result<()> {
    Ok(())
}

pub async fn fresh_opencode_login(
    store: &SharedStore,
    chrome_profile: Option<&str>,
    home: &Path,
) -> Result<()> {
    let (profiles, _) = browser_profiles(store, Provider::OpenCodeZen, chrome_profile).await?;
    let profile = profiles
        .first()
        .ok_or_else(|| anyhow!("no browser profile available"))?;
    let chrome = verified_chrome_profile(profile)?;
    let mut child = tokio::process::Command::from(crate::provider_auth::opencode_command(
        Some(home),
        &["auth", "login", "opencode", "--method", "device"],
    ))
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .kill_on_drop(true)
    .spawn()
    .context("start OpenCode device login")?;
    let mut lines =
        tokio::io::BufReader::new(child.stdout.take().expect("piped OpenCode stdout")).lines();
    let login = async {
        while let Some(line) = lines.next_line().await? {
            if let Some(url) = line
                .split(|c: char| c.is_whitespace() || c.is_control())
                .find(|word| word.starts_with("https://") && word.contains("/device?"))
            {
                open_chrome_profile(&chrome, url)?;
                narrate!("Approve the OpenCode login in Chrome; it completes automatically.");
            }
        }
        let status = child.wait().await?;
        if !status.success() {
            bail!("OpenCode device login failed (exit {:?})", status.code());
        }
        Ok(())
    };
    let completion =
        wait_for_browser_confirmation(login, None, Provider::OpenCodeZen, "remote machine");
    tokio::pin!(completion);
    tokio::select! {
        biased;
        result = &mut completion => result,
        _ = tokio::signal::ctrl_c() => Err(anyhow!("authorization cancelled")),
    }
}

async fn wait_for_browser_confirmation<F, E>(
    confirmation: F,
    expires_in: Option<u64>,
    provider: Provider,
    login: &str,
) -> Result<()>
where
    F: Future<Output = std::result::Result<(), E>>,
    E: Into<anyhow::Error>,
{
    let timeout = Duration::from_secs(expires_in.unwrap_or(AUTH_CODE_FLOW_TIMEOUT_SECS));
    let started_at = tokio::time::Instant::now();
    let deadline = tokio::time::sleep_until(started_at + timeout);
    let mut heartbeat = tokio::time::interval_at(
        started_at + AUTH_BROWSER_HEARTBEAT_INTERVAL,
        AUTH_BROWSER_HEARTBEAT_INTERVAL,
    );
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tokio::pin!(confirmation);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            biased;
            result = &mut confirmation => return result.map_err(Into::into),
            _ = &mut deadline => {
                return Err(anyhow!(
                    "timed out waiting for {} account '{}' browser confirmation",
                    provider.display_name(),
                    login
                ));
            }
            _ = heartbeat.tick() => {
                narrate!(
                    "Still waiting for browser approval ({}s elapsed); the login listener is still running. Press Ctrl-C to cancel.",
                    started_at.elapsed().as_secs()
                );
            }
        }
    }
}
