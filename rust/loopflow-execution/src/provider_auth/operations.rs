//! Service sign-in on this Machine for `lf machine login`. Claude and Codex
//! logins belong to their native clients.
use crate::config::placement::Approver;
use crate::machine::login::{send, Page, Sent};
use crate::profile::LocalChromeProfile;
use crate::provider_auth::{AuthCompletion, Provider, ProviderAuthService};
use crate::store::{open_store, CredentialType, ProviderToken, SharedStore};
use anyhow::{anyhow, bail, Context, Result};
use std::future::Future;
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

/// Connect a service: through its browser login, or from its API key variable.
pub async fn connect(
    provider: Provider,
    chrome_profile: Option<&str>,
    api_key: bool,
) -> Result<()> {
    if api_key {
        configure(provider).await
    } else {
        sign_in(provider, chrome_profile).await
    }
}

async fn sign_in(provider: Provider, chrome_profile: Option<&str>) -> Result<()> {
    let approver = crate::machine::login::approver(provider, chrome_profile)?;
    let service = local_auth_service().await?;
    let flow = service
        .start_auth(provider)
        .await
        .context("provider launch / URL discovery")?;
    let result = async {
        let url = flow
            .verification_uri_complete
            .clone()
            .unwrap_or_else(|| flow.verification_uri.clone());
        println!("Connecting local {}...", provider.display_name());
        if matches!(flow.completion, AuthCompletion::Manual) {
            println!("Manual authorization is required.");
        }
        // An already signed-in Doppler returns a finished flow with no page.
        if !url.is_empty() {
            approve(provider, approver, url, flow.user_code.clone()).await?;
        }
        tokio::select! {
            biased;
            result = service.wait_for_auth(provider) => result.map_err(anyhow::Error::from),
            _ = tokio::time::sleep(Duration::from_secs(flow.expires_in.unwrap_or(AUTH_CODE_FLOW_TIMEOUT_SECS))) => Err(anyhow!("authorization wait timed out")),
            _ = tokio::signal::ctrl_c() => Err(anyhow!("authorization cancelled")),
        }
    }
    .await;
    service.abort_pending(provider).await;
    result?;
    println!(
        "{}: {} signed in",
        crate::store::ownership::local_label(),
        provider.display_name()
    );
    Ok(())
}

/// Sends a service's approval page to its approver off the runtime, since an
/// approver command may take as long as the approval.
async fn approve(
    provider: Provider,
    approver: Option<Approver>,
    url: String,
    code: Option<String>,
) -> Result<()> {
    let sent = crate::journal::request::spawn_blocking(move || {
        let page = Page {
            provider,
            machine: &crate::store::ownership::local_label(),
            url: &url,
            code: code.as_deref(),
        };
        send(approver.as_ref(), &page)
    })
    .await??;
    if let Sent::Opened(place) = sent {
        println!("Approve {} {place}.", provider.display_name());
    }
    Ok(())
}

pub async fn disconnect(provider: Provider) -> Result<()> {
    let service = local_auth_service().await?;
    service.disconnect(provider).await?;
    Ok(())
}

async fn configure(provider: Provider) -> Result<()> {
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
                "{env_name} is not set. Export it, then run `lf machine login {} --api-key`.",
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
        println!("API key auth bills per token. OAuth uses your existing subscription.");
    }
    println!("Stored {} API key", provider.display_name());
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
pub(crate) fn open_chrome_profile(profile: &LocalChromeProfile, url: &str) -> Result<()> {
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
pub(crate) fn open_chrome_profile(_profile: &LocalChromeProfile, _url: &str) -> Result<()> {
    Err(anyhow!(
        "opening a selected Chrome profile is currently supported on macOS only"
    ))
}

#[cfg(test)]
pub(crate) fn open_chrome_profile(_profile: &LocalChromeProfile, _url: &str) -> Result<()> {
    Ok(())
}

pub async fn fresh_opencode_login(approver: Option<&Approver>, home: &Path) -> Result<()> {
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
                approve(
                    Provider::OpenCodeZen,
                    approver.cloned(),
                    url.to_string(),
                    None,
                )
                .await?;
                println!("OpenCode's login completes once it is approved.");
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
                println!(
                    "Still waiting for browser approval ({}s elapsed); the login listener is still running. Press Ctrl-C to cancel.",
                    started_at.elapsed().as_secs()
                );
            }
        }
    }
}
