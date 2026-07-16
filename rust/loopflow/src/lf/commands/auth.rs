#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::engine::platform::open_url;
use crate::lf::AuthCommand;
use crate::profile::{EmailAddress, HostId, LocalChromeProfile, ProfileId, ProfileProviderAccount};
use crate::provider_account::{
    account_profile_path, ensure_account_profile, new_account, open_account_store,
    parse_account_id, remove_account_profile,
};
use crate::provider_auth::{
    capture_claude_profile_credentials, capture_codex_profile_credentials,
    disconnect_provider_account_auth, drive_claude_browser_authorization, no_event_sink,
    prepare_provider_account_access_token, provider_account_auth_status,
    start_provider_account_auth, AuthStatus, ClaudeKeychainGuard, Provider, ProviderAuthService,
    ProviderAuthSnapshot,
};
use crate::store::{
    open_store, CredentialState, CredentialType, ProviderAccount, ProviderAccountId, ProviderToken,
    RoutingState, SharedStore, StoreError,
};

const AUTH_STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(180);
const AUTH_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeAuthStatusOutput {
    logged_in: bool,
    email: Option<String>,
}

#[derive(Debug)]
struct AccountLifecycleUpdate<'a> {
    login_email: Option<&'a str>,
    routing: Option<&'a str>,
    plan: Option<&'a str>,
    clear_plan: bool,
    paid_through: Option<&'a str>,
    clear_paid_through: bool,
}

pub fn run(cmd: &AuthCommand) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    rt.block_on(run_async(cmd))
}

async fn run_async(cmd: &AuthCommand) -> Result<()> {
    match cmd {
        AuthCommand::Status { provider } => status(provider.as_deref()).await,
        AuthCommand::Disconnect { provider, account } => match account {
            Some(account) => disconnect_account(provider, account).await,
            None => disconnect(provider).await,
        },
        AuthCommand::Configure { provider } => configure(provider).await,
        AuthCommand::Connect { provider, profile } => match profile.as_deref() {
            Some(profile) => connect_profile_account(provider, profile).await,
            None => connect(provider).await,
        },
        AuthCommand::Import {
            provider,
            account,
            chrome_profile,
            profile,
        } => {
            import_account(
                provider,
                account,
                profile.as_deref(),
                chrome_profile.as_deref(),
            )
            .await
        }
        AuthCommand::Accounts { provider } => accounts(provider.as_deref()).await,
        AuthCommand::Set {
            provider,
            account,
            login_email,
            routing,
            plan,
            clear_plan,
            paid_through,
            clear_paid_through,
        } => {
            set_account_lifecycle(
                provider,
                account,
                AccountLifecycleUpdate {
                    login_email: login_email.as_deref(),
                    routing: routing.as_deref(),
                    plan: plan.as_deref(),
                    clear_plan: *clear_plan,
                    paid_through: paid_through.as_deref(),
                    clear_paid_through: *clear_paid_through,
                },
            )
            .await
        }
        AuthCommand::Reset { provider, account } => reset_account(provider, account).await,
        AuthCommand::External(args) => {
            let provider = args
                .first()
                .ok_or_else(|| anyhow!("usage: lf auth <provider>"))?;
            if args.len() > 1 {
                return Err(anyhow!(
                    "unexpected auth arguments: {}",
                    args[1..].join(" ")
                ));
            }
            connect(provider).await
        }
    }
}

async fn status(provider: Option<&str>) -> Result<()> {
    let service = local_auth_service().await?;
    match provider {
        Some(raw) => {
            let snapshot = service.status(parse_provider(raw)?).await?;
            println!("{}", format_snapshot(&snapshot));
        }
        None => {
            for snapshot in service.list_statuses().await? {
                println!("{}", format_snapshot(&snapshot));
            }
        }
    }
    Ok(())
}

async fn connect(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let service = local_auth_service().await?;
    let flow = service.start_auth(provider, no_event_sink()).await?;

    let verification_url = flow
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| flow.verification_uri.clone());
    println!(
        "Opening {} auth in your browser...",
        provider.display_name()
    );
    open_url(&verification_url);
    println!("Complete authorization in the browser.");

    wait_for_active_status(&service, provider, flow.expires_in).await
}

async fn connect_profile_account(raw_provider: &str, raw_profile: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let profile_id = ProfileId::parse(raw_profile).map_err(anyhow::Error::msg)?;
    let store = open_account_store().await?;
    if store.get_profile(&profile_id).await?.is_none() {
        return Err(anyhow!(
            "profile '{}' does not exist; run 'lf profile create {} --chrome-profile {}' first",
            profile_id,
            profile_id,
            profile_id
        ));
    }
    let account = account_for_profile(&store, provider, &profile_id).await?;
    let account_id = account
        .as_ref()
        .map(|account| account.account_id.clone())
        .unwrap_or_else(|| account_id_for_profile(&profile_id));
    let auth_profile_id = account
        .as_ref()
        .and_then(|account| account.login_email.as_ref())
        .map(|email| ProfileId::parse(email.as_str()))
        .transpose()
        .map_err(anyhow::Error::msg)?
        .unwrap_or_else(|| profile_id.clone());
    let chrome_profile = resolve_profile_chrome_profile(&store, &auth_profile_id).await?;
    connect_managed_account(&store, provider, account_id, profile_id, chrome_profile).await
}

async fn connect_managed_account(
    store: &SharedStore,
    provider: Provider,
    account_id: ProviderAccountId,
    profile_id: ProfileId,
    chrome_profile: LocalChromeProfile,
) -> Result<()> {
    let account_home = ensure_account_profile(provider, &account_id)?;
    let controller_profile = (provider == Provider::Claude
        && account_home.join(".credentials.json").is_file())
    .then(|| account_home.clone());
    let keychain_guard = if provider == Provider::Claude {
        Some(ClaudeKeychainGuard::preserve()?)
    } else {
        None
    };
    let handle = start_provider_account_auth(provider, account_home.clone()).await?;
    let flow = handle.response.clone();
    let verification_url = flow
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| flow.verification_uri.clone());
    println!(
        "Connecting {} for profile '{}'...",
        provider.display_name(),
        profile_id
    );
    if handle.requires_authorization_code() {
        open_chrome_profile(&chrome_profile, &verification_url)?;
        println!("Authorizing with Claude in Chrome...");
        if let Some(code) = drive_claude_browser_authorization(
            &verification_url,
            controller_profile.as_deref(),
            Some(chrome_profile.label.as_str()),
        )
        .await?
        {
            handle
                .submit_authorization_code(code.expose_secret())
                .await?;
        } else {
            println!("Chrome controller unavailable; complete authorization in the browser.");
            let code = SecretString::new(rpassword::prompt_password(
                "Paste the one-time code from the browser: ",
            )?);
            handle
                .submit_authorization_code(code.expose_secret())
                .await?;
        }
    } else {
        println!("Complete authorization in the browser.");
        open_chrome_profile(&chrome_profile, &verification_url)?;
    }
    tokio::time::timeout(AUTH_STATUS_POLL_TIMEOUT, handle.wait())
        .await
        .map_err(|_| {
            anyhow!(
                "timed out waiting for {} account '{}' browser confirmation",
                provider.display_name(),
                account_id
            )
        })??;

    let observed_claude_login = if provider == Provider::Claude {
        capture_claude_profile_credentials(&account_home)?;
        let login = match provider_account_auth_status(provider, account_home.clone()).await? {
            AuthStatus::Active { login } => login,
            _ => None,
        };
        require_managed_access_token(provider, &account_id, &account_home).await?;
        if let Some(guard) = keychain_guard {
            guard.restore()?;
        }
        login
    } else {
        None
    };

    let login = if provider == Provider::Claude {
        observed_claude_login
    } else {
        match provider_account_auth_status(provider, account_home.clone()).await? {
            AuthStatus::Active { login } => login,
            other => {
                return Err(anyhow!(
                    "{} account '{}' finished login with status {}",
                    provider.display_name(),
                    account_id,
                    other.as_str()
                ))
            }
        }
    };
    let login = resolve_profile_login(provider, Some(&chrome_profile), login)?;
    register_managed_account(
        store,
        provider,
        &account_id,
        account_home,
        login,
        Some(&profile_id),
    )
    .await?;
    println!(
        "Connected {} for profile '{}'",
        provider.display_name(),
        profile_id
    );
    Ok(())
}

async fn register_managed_account(
    store: &SharedStore,
    provider: Provider,
    account_id: &ProviderAccountId,
    account_home: PathBuf,
    login: Option<String>,
    bind_profile: Option<&ProfileId>,
) -> Result<()> {
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let existing = accounts
        .iter()
        .find(|account| account.account_id == *account_id);
    let login_email = login
        .map(|value| EmailAddress::parse(&value))
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let mut account = existing.cloned().unwrap_or_else(|| {
        new_account(
            provider,
            account_id.clone(),
            account_home.clone(),
            login_email.clone(),
        )
    });
    account.home = Some(account_home);
    if let Some(login_email) = login_email {
        account.login_email = Some(login_email);
    }
    account.credential_state = CredentialState::Connected;
    account.updated_at = OffsetDateTime::now_utc().unix_timestamp();
    store.upsert_provider_account(&account).await?;
    if let Some(profile_id) = bind_profile {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        store
            .set_profile_provider_account(&ProfileProviderAccount {
                profile_id: profile_id.clone(),
                provider,
                account_id: account_id.clone(),
                created_at: now,
                updated_at: now,
            })
            .await?;
    }
    Ok(())
}

async fn account_for_profile(
    store: &SharedStore,
    provider: Provider,
    profile_id: &ProfileId,
) -> Result<Option<ProviderAccount>> {
    if let Some(mapping) = store.profile_provider_account(profile_id, provider).await? {
        return store
            .get_provider_account(provider.as_str(), &mapping.account_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "profile '{}' references missing {} account '{}'",
                    profile_id,
                    provider,
                    mapping.account_id
                )
            })
            .map(Some);
    }

    let matches = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?
        .into_iter()
        .filter(|account| {
            account
                .login_email
                .as_ref()
                .is_some_and(|email| email.as_str().eq_ignore_ascii_case(profile_id.as_str()))
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [account] => Ok(Some(account.clone())),
        [_, ..] => Err(anyhow!(
            "{} login '{}' matches multiple managed accounts",
            provider,
            profile_id
        )),
    }
}

fn account_id_for_profile(profile_id: &ProfileId) -> ProviderAccountId {
    let mut prefix = String::new();
    let mut last_was_separator = false;
    for character in profile_id
        .as_str()
        .split_once('@')
        .expect("profile ids contain an at sign")
        .0
        .chars()
    {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            prefix.push(character);
            last_was_separator = false;
        } else if !last_was_separator && !prefix.is_empty() {
            prefix.push('-');
            last_was_separator = true;
        }
        if prefix.len() == 50 {
            break;
        }
    }
    while prefix.ends_with('-') {
        prefix.pop();
    }
    if prefix.is_empty() {
        prefix.push_str("account");
    }
    let digest = hex::encode(Sha256::digest(profile_id.as_str().as_bytes()));
    ProviderAccountId::parse(&format!("{prefix}-{}", &digest[..12]))
        .expect("derived account ids satisfy provider account constraints")
}

async fn import_account(
    raw_provider: &str,
    raw_account: &str,
    raw_profile: Option<&str>,
    raw_chrome_profile: Option<&str>,
) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let account_id = parse_account_id(raw_account)?;
    let provider_profile = ensure_account_profile(provider, &account_id)?;
    let chrome_profile = resolve_auth_chrome_profile(raw_profile, raw_chrome_profile).await?;
    let paired_login = chrome_profile
        .as_ref()
        .map(|profile| profile.label.as_str())
        .filter(|label| label.contains('@'))
        .map(String::from);

    let credential_exists = match provider {
        Provider::Claude => provider_profile.join(".credentials.json").is_file(),
        Provider::Codex => provider_profile.join("auth.json").is_file(),
        _ => false,
    };
    let login = if credential_exists {
        match provider_account_auth_status(provider, provider_profile.clone()).await? {
            AuthStatus::Active { login } => login,
            other => {
                return Err(anyhow!(
                    "{} account '{}' has status {}",
                    provider.display_name(),
                    account_id,
                    other.as_str()
                ))
            }
        }
    } else {
        match provider {
            Provider::Claude => {
                let ambient = read_ambient_claude_status()?;
                if !ambient.logged_in {
                    return Err(anyhow!("the ambient Claude CLI is not logged in"));
                }
                if let (Some(expected), Some(actual)) =
                    (paired_login.as_deref(), ambient.email.as_deref())
                {
                    if !expected.eq_ignore_ascii_case(actual) {
                        return Err(anyhow!(
                            "ambient Claude login '{}' does not match paired Chrome profile '{}'",
                            actual,
                            expected
                        ));
                    }
                }
                capture_claude_profile_credentials(&provider_profile)?;
                ambient.email.or(paired_login)
            }
            Provider::Codex => {
                capture_codex_profile_credentials(&provider_profile)?;
                match provider_account_auth_status(provider, provider_profile.clone()).await? {
                    AuthStatus::Active { login } => login,
                    other => {
                        return Err(anyhow!(
                            "ambient Codex credential produced status {}",
                            other.as_str()
                        ))
                    }
                }
            }
            _ => return Err(anyhow!("{} account import is unsupported", provider)),
        }
    };

    require_managed_access_token(provider, &account_id, &provider_profile).await?;
    let login = resolve_profile_login(provider, chrome_profile.as_ref(), login)?;
    let store = open_account_store().await?;
    register_managed_account(&store, provider, &account_id, provider_profile, login, None).await?;
    println!(
        "Imported {} account '{}'",
        provider.display_name(),
        account_id
    );
    Ok(())
}

async fn require_managed_access_token(
    provider: Provider,
    account_id: &ProviderAccountId,
    profile: &std::path::Path,
) -> Result<()> {
    if prepare_provider_account_access_token(provider, profile)
        .await?
        .is_some()
    {
        return Ok(());
    }
    Err(anyhow!(
        "{} account '{}' did not produce a usable access token",
        provider.display_name(),
        account_id
    ))
}

fn read_ambient_claude_status() -> Result<ClaudeAuthStatusOutput> {
    let output = Command::new("claude")
        .args(["auth", "status"])
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .context("read ambient Claude login")?;
    if !output.status.success() {
        return Err(anyhow!("the ambient Claude CLI is not logged in"));
    }
    serde_json::from_slice(&output.stdout).context("parse ambient Claude login status")
}

async fn resolve_auth_chrome_profile(
    raw_profile: Option<&str>,
    raw_chrome_profile: Option<&str>,
) -> Result<Option<LocalChromeProfile>> {
    if let Some(raw_profile) = raw_profile {
        let profile_id = ProfileId::parse(raw_profile).map_err(|error| anyhow!(error))?;
        let store = open_account_store().await?;
        return resolve_profile_chrome_profile(&store, &profile_id)
            .await
            .map(Some);
    }
    raw_chrome_profile
        .map(crate::profile::resolve_local_chrome_profile)
        .transpose()
        .map_err(|error| anyhow!(error))
}

async fn resolve_profile_chrome_profile(
    store: &SharedStore,
    profile_id: &ProfileId,
) -> Result<LocalChromeProfile> {
    let host_id = HostId::local().map_err(anyhow::Error::msg)?;
    let binding = store
        .chrome_profile_binding(profile_id, &host_id)
        .await?
        .ok_or_else(|| {
            anyhow!(
                "profile '{}' has no Chrome binding on {}; run 'lf profile create {} --chrome-profile {}'",
                profile_id,
                host_id,
                profile_id,
                profile_id
            )
        })?;
    Ok(LocalChromeProfile {
        directory: binding.chrome_directory,
        label: binding.profile_id.to_string(),
    })
}

fn resolve_profile_login(
    provider: Provider,
    chrome_profile: Option<&LocalChromeProfile>,
    provider_login: Option<String>,
) -> Result<Option<String>> {
    let expected_login = chrome_profile
        .map(|profile| profile.label.as_str())
        .filter(|label| label.contains('@'));
    let Some(expected) = expected_login else {
        return Ok(provider_login);
    };
    let Some(actual) = provider_login.as_deref() else {
        return match provider {
            Provider::Claude => Ok(Some(expected.to_string())),
            _ => Err(anyhow!(
                "provider did not report a login email for Chrome profile '{}'",
                expected
            )),
        };
    };
    if !expected.eq_ignore_ascii_case(actual) {
        return Err(anyhow!(
            "provider login '{}' does not match Chrome profile '{}'",
            actual,
            expected
        ));
    }
    Ok(provider_login)
}

#[cfg(target_os = "macos")]
fn open_chrome_profile(profile: &LocalChromeProfile, url: &str) -> Result<()> {
    let chrome = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    if !chrome.is_file() {
        return Err(anyhow!("Google Chrome is not installed in /Applications"));
    }
    let status = Command::new(chrome)
        .arg(format!("--profile-directory={}", profile.directory))
        .arg("--new-window")
        .arg(url)
        .status()
        .context("open matching Chrome profile")?;
    if !status.success() {
        return Err(anyhow!(
            "Google Chrome could not open profile '{}'",
            profile.label
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn open_chrome_profile(_profile: &LocalChromeProfile, _url: &str) -> Result<()> {
    Err(anyhow!(
        "opening a selected Chrome profile is currently supported on macOS only"
    ))
}

async fn disconnect(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let service = local_auth_service().await?;
    service.disconnect(provider, no_event_sink()).await?;
    let snapshot = service.status(provider).await?;
    if matches!(snapshot.status, AuthStatus::None) {
        println!("Disconnected {}", provider.display_name());
    } else {
        println!("Updated {}", format_snapshot(&snapshot));
    }
    Ok(())
}

async fn disconnect_account(raw_provider: &str, raw_account: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let account_id = parse_account_id(raw_account)?;
    let store = open_account_store().await?;
    let mut account = store
        .get_provider_account(provider.as_str(), &account_id)
        .await?
        .ok_or_else(|| anyhow!("unknown {} account '{}'", provider, account_id))?;
    if let Some(profile) = account.home.as_deref() {
        let expected_profile = account_profile_path(provider, &account_id)?;
        if profile != expected_profile {
            return Err(anyhow!(
                "refusing to remove unexpected {} account profile {}",
                provider.display_name(),
                profile.display()
            ));
        }
        disconnect_provider_account_auth(provider, expected_profile.clone()).await?;
        remove_account_profile(&expected_profile)?;
    }
    account.credential_state = CredentialState::Missing;
    account.utilization_percent = None;
    account.cooldown_until = None;
    account.cooldown_reason = None;
    account.updated_at = OffsetDateTime::now_utc().unix_timestamp();
    store.upsert_provider_account(&account).await?;
    println!(
        "Disconnected {} account '{}'",
        provider.display_name(),
        account_id
    );
    Ok(())
}

async fn accounts(raw_provider: Option<&str>) -> Result<()> {
    let provider = raw_provider.map(parse_managed_provider).transpose()?;
    let store = open_account_store().await?;
    let accounts = store
        .list_provider_accounts(provider.map(Provider::as_str))
        .await?;
    let accounts: Vec<_> = accounts
        .into_iter()
        .filter(|account| account.home.is_some())
        .collect();
    if accounts.is_empty() {
        println!("No managed OAuth accounts");
        return Ok(());
    }
    for account in accounts {
        println!("{}", format_account(&account));
    }
    Ok(())
}

async fn set_account_lifecycle(
    raw_provider: &str,
    raw_account: &str,
    update: AccountLifecycleUpdate<'_>,
) -> Result<()> {
    if update.login_email.is_none()
        && update.routing.is_none()
        && update.plan.is_none()
        && !update.clear_plan
        && update.paid_through.is_none()
        && !update.clear_paid_through
    {
        return Err(anyhow!(
            "lf auth set needs --login-email, --routing, --plan, or --paid-through"
        ));
    }
    let provider = parse_managed_provider(raw_provider)?;
    let account_id = parse_account_id(raw_account)?;
    let login_email = update
        .login_email
        .map(EmailAddress::parse)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let routing_state = update.routing.map(parse_routing_state).transpose()?;
    let plan = update.plan.map(parse_plan).transpose()?;
    let paid_through = update.paid_through.map(parse_paid_through).transpose()?;
    let store = open_account_store().await?;
    let mut account = store
        .get_provider_account(provider.as_str(), &account_id)
        .await?
        .ok_or_else(|| anyhow!("unknown {} account '{}'", provider, account_id))?;
    if let Some(login_email) = login_email {
        account.login_email = Some(login_email);
    }
    if let Some(routing_state) = routing_state {
        account.routing_state = routing_state;
    }
    if update.clear_plan {
        account.plan = None;
    } else if let Some(plan) = plan {
        account.plan = Some(plan);
    }
    if update.clear_paid_through {
        account.paid_through = None;
    } else if let Some(paid_through) = paid_through {
        account.paid_through = Some(paid_through);
    }
    account.updated_at = OffsetDateTime::now_utc().unix_timestamp();
    store
        .update_provider_account_lifecycle(&account)
        .await
        .map_err(|error| account_store_error(provider, &account_id.to_string(), error))?;
    println!("Updated {}", format_account(&account));
    Ok(())
}

fn parse_routing_state(value: &str) -> Result<RoutingState> {
    match value.trim().to_ascii_lowercase().as_str() {
        "automatic" => Ok(RoutingState::Automatic),
        "explicit-only" | "explicit_only" => Ok(RoutingState::ExplicitOnly),
        "disabled" => Ok(RoutingState::Disabled),
        _ => Err(anyhow!(
            "invalid routing state '{value}': expected automatic, explicit-only, or disabled"
        )),
    }
}

fn parse_plan(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 64 || value.chars().any(char::is_control) {
        return Err(anyhow!("plan must be 1-64 printable characters"));
    }
    Ok(value.to_string())
}

fn parse_paid_through(value: &str) -> Result<time::Date> {
    let format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")?;
    time::Date::parse(value.trim(), &format)
        .map_err(|_| anyhow!("invalid paid-through date '{value}': expected YYYY-MM-DD"))
}

async fn reset_account(raw_provider: &str, raw_account: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let account_id = parse_account_id(raw_account)?;
    let store = open_account_store().await?;
    store
        .reset_provider_account_health(provider.as_str(), &account_id)
        .await
        .map_err(|error| account_store_error(provider, &account_id.to_string(), error))?;
    println!(
        "Reset {} account '{}' usage state",
        provider.display_name(),
        account_id
    );
    Ok(())
}

fn account_store_error(provider: Provider, account: &str, error: StoreError) -> anyhow::Error {
    match error {
        StoreError::NotFound => anyhow!("unknown {} account '{}'", provider, account),
        other => anyhow!(other),
    }
}

fn parse_managed_provider(raw: &str) -> Result<Provider> {
    let provider = parse_provider(raw)?;
    if matches!(provider, Provider::Claude | Provider::Codex) {
        Ok(provider)
    } else {
        Err(anyhow!(
            "managed OAuth accounts support Claude and Codex only"
        ))
    }
}

fn format_account(account: &ProviderAccount) -> String {
    let mut details = Vec::new();
    let routing_state = account.effective_routing_state(OffsetDateTime::now_utc().date());
    if routing_state != RoutingState::Automatic {
        details.push(routing_state.as_str().replace('_', "-"));
    }
    if account.credential_state != CredentialState::Connected {
        details.push(account.credential_state.as_str().to_string());
    }
    if let Some(plan) = &account.plan {
        details.push(plan.clone());
    }
    if let Some(paid_through) = account.paid_through {
        details.push(format!("paid through {paid_through}"));
    }
    if let Some(utilization) = account.utilization_percent {
        details.push(format!("{utilization}% used"));
    }
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if let Some(cooldown_until) = account.cooldown_until.filter(|until| *until > now) {
        details.push(format!(
            "cooling for {}",
            format_relative_delta(cooldown_until - now)
        ));
    }
    if let Some(login) = &account.login_email {
        details.push(login.to_string());
    }
    if details.is_empty() {
        details.push("ready".to_string());
    }
    format!(
        "{:<12} {:<16} {}",
        account.provider,
        account.account_id,
        details.join(" · ")
    )
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
                "{env_name} is not set. Export it, then run `lf auth configure {}`.",
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

async fn local_store() -> Result<SharedStore> {
    let cfg = crate::store::storage_config_from_env()
        .context("failed to resolve local credential store")?;
    let store = open_store(&cfg)
        .await
        .map_err(|err| anyhow!("failed to open local credential store: {err}"))?;
    Ok(Arc::new(store))
}

async fn wait_for_active_status(
    service: &ProviderAuthService,
    provider: Provider,
    expires_in: Option<u64>,
) -> Result<()> {
    let timeout = expires_in
        .map(Duration::from_secs)
        .unwrap_or(AUTH_STATUS_POLL_TIMEOUT);
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let snapshot = service.status(provider).await?;
        match snapshot.status {
            AuthStatus::Active { login } => {
                if provider == Provider::GitHub {
                    if let Some(login) = login {
                        println!("Authenticated as @{login}");
                    } else {
                        println!("Authenticated");
                    }
                } else {
                    println!("Authenticated");
                }
                return Ok(());
            }
            AuthStatus::Expired => {
                println!(
                    "Authentication expired. Run `lf auth {}` again.",
                    provider.as_str()
                );
                return Ok(());
            }
            AuthStatus::Pending | AuthStatus::None => {
                if std::time::Instant::now() >= deadline {
                    println!(
                        "Authentication still pending. Finish the browser flow, then run `lf auth status {}`.",
                        provider.as_str()
                    );
                    return Ok(());
                }
                tokio::time::sleep(AUTH_STATUS_POLL_INTERVAL).await;
            }
        }
    }
}

fn parse_provider(raw: &str) -> Result<Provider> {
    raw.parse::<Provider>()
        .map_err(|_| anyhow!("unknown provider: {raw}"))
}

fn format_snapshot(snapshot: &ProviderAuthSnapshot) -> String {
    let provider = format!("{:<12}", snapshot.provider.display_name());
    let credential_type = snapshot.credential_type.unwrap_or(CredentialType::OAuth);

    match snapshot.status.clone() {
        AuthStatus::Active { login } => {
            let status = if credential_type == CredentialType::ApiKey {
                "apikey"
            } else {
                "oauth"
            };
            let mut details = Vec::new();
            if let Some(login) = login {
                if snapshot.provider == Provider::GitHub {
                    details.push(format!("@{login}"));
                } else {
                    details.push(login);
                }
            } else {
                details.push("authenticated".to_string());
            }

            let now = OffsetDateTime::now_utc().unix_timestamp();
            if let Some(expires_at) = snapshot.expires_at {
                let delta = expires_at - now;
                if delta <= 0 {
                    details.push("expired".to_string());
                } else {
                    details.push(format!("expires {}", format_relative_delta(delta)));
                }
            }
            if let Some(next_refresh_at) = snapshot.next_refresh_at {
                let delta = next_refresh_at - now;
                if delta <= 0 {
                    details.push("refreshing soon".to_string());
                } else {
                    details.push(format!("refresh in {}", format_relative_delta(delta)));
                }
            }
            if credential_type == CredentialType::ApiKey
                && snapshot.provider.api_key_bills_per_token()
            {
                details.push("pay-per-token".to_string());
            }

            format!("{provider} {status:<8} {}", details.join(" · "))
        }
        AuthStatus::Pending => {
            format!(
                "{provider} {:<8} waiting for browser confirmation",
                "pending"
            )
        }
        AuthStatus::Expired => format!("{provider} {:<8} expired", "expired"),
        AuthStatus::None => format!("{provider} {:<8} not connected", "none"),
    }
}

fn format_relative_delta(seconds: i64) -> String {
    let total_seconds = seconds.max(0);
    if total_seconds < 60 {
        return format!("{total_seconds}s");
    }
    if total_seconds < 3600 {
        return format!("{}m", total_seconds / 60);
    }
    if total_seconds < 86_400 {
        return format!("{}h", total_seconds / 3600);
    }
    format!("{}d", total_seconds / 86_400)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use time::OffsetDateTime;

    use crate::profile::{
        EmailAddress, LocalChromeProfile, Profile, ProfileId, ProfileProviderAccount,
    };
    use crate::provider_auth::{AuthStatus, Provider, ProviderAuthSnapshot};
    use crate::store::{
        open_store, CredentialState, CredentialType, ProviderAccount, ProviderAccountId,
        RoutingState, StorageConfig,
    };

    use super::{
        account_for_profile, account_id_for_profile, format_account, format_relative_delta,
        format_snapshot, parse_paid_through, parse_routing_state, register_managed_account,
        resolve_profile_login,
    };

    fn managed_account(provider: Provider, account_id: &str, login: &str) -> ProviderAccount {
        ProviderAccount {
            provider: provider.as_str().to_string(),
            account_id: ProviderAccountId::parse(account_id).unwrap(),
            home: Some(PathBuf::from(format!("/accounts/{account_id}"))),
            login_email: Some(EmailAddress::parse(login).unwrap()),
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
    fn format_account_shows_routing_state() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let rendered = format_account(&ProviderAccount {
            provider: "claude".to_string(),
            account_id: ProviderAccountId::parse("primary").unwrap(),
            home: None,
            login_email: Some(EmailAddress::parse("operator@example.com").unwrap()),
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: Some("max".to_string()),
            paid_through: None,
            utilization_percent: Some(72),
            cooldown_until: Some(now + 3_600),
            cooldown_reason: Some("five_hour".to_string()),
            last_selected_at: None,
            created_at: now,
            updated_at: now,
        });

        assert!(rendered.contains("claude"));
        assert!(rendered.contains("primary"));
        assert!(!rendered.contains("preferred"));
        assert!(rendered.contains("max"));
        assert!(rendered.contains("72% used"));
        assert!(rendered.contains("cooling for"));
        assert!(rendered.contains("operator@example.com"));
    }

    #[test]
    fn format_snapshot_shows_login_and_expiry() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let rendered = format_snapshot(&ProviderAuthSnapshot {
            provider: Provider::GitHub,
            status: AuthStatus::Active {
                login: Some("jackdanger".to_string()),
            },
            expires_at: Some(now + 7200),
            next_refresh_at: None,
            credential_type: Some(CredentialType::OAuth),
        });

        assert!(rendered.contains("GitHub"));
        assert!(rendered.contains("oauth"));
        assert!(rendered.contains("@jackdanger"));
        assert!(rendered.contains("expires 1h") || rendered.contains("expires 2h"));
    }

    #[test]
    fn lifecycle_values_are_strict_and_expired_plans_become_explicit_only() {
        assert_eq!(
            parse_routing_state("explicit-only").unwrap(),
            RoutingState::ExplicitOnly
        );
        assert!(parse_routing_state("fallback").is_err());
        assert!(parse_paid_through("08/14/2026").is_err());

        let today = OffsetDateTime::now_utc().date();
        let account = ProviderAccount {
            provider: "codex".to_string(),
            account_id: ProviderAccountId::parse("personal").unwrap(),
            home: None,
            login_email: Some(EmailAddress::parse("operator@example.com").unwrap()),
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: Some("plus".to_string()),
            paid_through: Some(today - time::Duration::days(1)),
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        };

        assert!(format_account(&account).contains("explicit-only"));
        assert!(!account.eligible_for_automatic_routing(today));
    }

    #[test]
    fn format_snapshot_shows_disconnected_providers() {
        let rendered = format_snapshot(&ProviderAuthSnapshot {
            provider: Provider::Linear,
            status: AuthStatus::None,
            expires_at: None,
            next_refresh_at: None,
            credential_type: None,
        });

        assert!(rendered.contains("Linear"));
        assert!(rendered.contains("not connected"));
    }

    #[test]
    fn format_relative_delta_uses_human_units() {
        assert_eq!(format_relative_delta(42), "42s");
        assert_eq!(format_relative_delta(180), "3m");
        assert_eq!(format_relative_delta(7_200), "2h");
        assert_eq!(format_relative_delta(172_800), "2d");
    }

    #[test]
    fn codex_profile_and_provider_login_must_name_the_same_account() {
        let chrome_profile = LocalChromeProfile {
            directory: "Profile 7".to_string(),
            label: "primary@example.com".to_string(),
        };

        assert_eq!(
            resolve_profile_login(
                Provider::Codex,
                Some(&chrome_profile),
                Some("PRIMARY@EXAMPLE.COM".to_string()),
            )
            .unwrap(),
            Some("PRIMARY@EXAMPLE.COM".to_string())
        );
        assert!(resolve_profile_login(
            Provider::Codex,
            Some(&chrome_profile),
            Some("personal@example.com".to_string()),
        )
        .is_err());
        assert!(resolve_profile_login(Provider::Codex, Some(&chrome_profile), None).is_err());
    }

    #[test]
    fn claude_uses_the_selected_profile_when_status_omits_email() {
        let chrome_profile = LocalChromeProfile {
            directory: "Profile 7".to_string(),
            label: "primary@example.com".to_string(),
        };

        assert_eq!(
            resolve_profile_login(Provider::Claude, Some(&chrome_profile), None).unwrap(),
            Some("primary@example.com".to_string())
        );
        assert!(resolve_profile_login(
            Provider::Claude,
            Some(&chrome_profile),
            Some("personal@example.com".to_string()),
        )
        .is_err());
    }

    #[test]
    fn new_profile_accounts_get_stable_internal_ids() {
        let profile = ProfileId::parse("Operator.Team@example.com").unwrap();

        assert_eq!(
            account_id_for_profile(&profile),
            account_id_for_profile(&profile)
        );
        assert!(account_id_for_profile(&profile)
            .as_str()
            .starts_with("operator-team-"));
    }

    #[tokio::test]
    async fn profile_connection_reuses_an_account_with_the_same_login() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let profile_id = ProfileId::parse("operator@example.com").unwrap();
        store
            .upsert_profile(&Profile {
                id: profile_id.clone(),
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();
        let account = managed_account(Provider::Codex, "existing", profile_id.as_str());
        store.upsert_provider_account(&account).await.unwrap();

        assert_eq!(
            account_for_profile(&store, Provider::Codex, &profile_id)
                .await
                .unwrap()
                .unwrap()
                .account_id,
            account.account_id
        );
    }

    #[tokio::test]
    async fn profile_connection_follows_an_explicit_shared_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let profile_id = ProfileId::parse("engineering@example.com").unwrap();
        store
            .upsert_profile(&Profile {
                id: profile_id.clone(),
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();
        let account = managed_account(Provider::Claude, "shared", "personal@example.com");
        store.upsert_provider_account(&account).await.unwrap();
        store
            .set_profile_provider_account(&ProfileProviderAccount {
                profile_id: profile_id.clone(),
                provider: Provider::Claude,
                account_id: account.account_id.clone(),
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();

        assert_eq!(
            account_for_profile(&store, Provider::Claude, &profile_id)
                .await
                .unwrap()
                .unwrap()
                .account_id,
            account.account_id
        );
    }

    #[tokio::test]
    async fn successful_profile_connection_registers_and_binds_the_account() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let profile_id = ProfileId::parse("operator@example.com").unwrap();
        store
            .upsert_profile(&Profile {
                id: profile_id.clone(),
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();
        let account_id = ProviderAccountId::parse("operator").unwrap();
        let account_home = dir.path().join("accounts/codex/operator");

        register_managed_account(
            &store,
            Provider::Codex,
            &account_id,
            account_home.clone(),
            Some(profile_id.to_string()),
            Some(&profile_id),
        )
        .await
        .unwrap();

        let account = store
            .get_provider_account(Provider::Codex.as_str(), &account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.home, Some(account_home));
        assert_eq!(account.login_email.unwrap().as_str(), profile_id.as_str());
        assert_eq!(
            store
                .profile_provider_account(&profile_id, Provider::Codex)
                .await
                .unwrap()
                .unwrap()
                .account_id,
            account_id
        );
    }
}
