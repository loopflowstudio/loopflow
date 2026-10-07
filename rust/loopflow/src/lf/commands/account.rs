#[path = "account_status.rs"]
mod account_status;
#[path = "auth_input.rs"]
mod auth_input;

use std::fs;
use std::future::Future;
use std::io::IsTerminal;
use std::path::Path;
use std::path::PathBuf;
#[cfg(all(target_os = "macos", not(test)))]
use std::process::Command;
#[cfg(all(target_os = "macos", not(test)))]
use std::process::Stdio;
use std::sync::Arc;
#[cfg(test)]
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use time::OffsetDateTime;

use crate::lf::AccountCommand;
use crate::profile::{AccessProfile, EmailAddress, LocalChromeProfile, ProfileId};
use crate::provider_account::{
    account_home_path, account_id_for_login, account_login, acquire_managed_login_lock,
    ensure_account_home, match_account, new_account, open_account_store, remove_account_home,
    AccountMatch,
};
use crate::provider_auth::{
    capture_claude_profile_credentials, disconnect_provider_account_auth,
    import_ambient_claude_profile_credentials, prepare_provider_account_access_token,
    start_provider_account_auth, AuthBroker, AuthCompletion, AuthError, AuthFlowResponse, Provider,
    ProviderAuthService,
};
use crate::store::{
    open_store, CredentialState, CredentialType, ProviderAccount, ProviderAccountId, ProviderToken,
    RoutingState, SharedStore, StoreError,
};
use crate::subscription::SubscriptionUsage;

const AUTH_BROWSER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
// Authorization-code flows wait on the browser login to finish; give
// them the ~10 minutes the OAuth authorization itself stays valid.
const AUTH_CODE_FLOW_TIMEOUT_SECS: u64 = 600;
#[cfg(test)]
static TEST_OPENED_CHROME_PROFILES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
#[cfg(test)]
static TEST_ACCESS_PROFILE_FAILURES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
#[derive(Debug)]
struct AccountLifecycleUpdate<'a> {
    login_email: Option<&'a str>,
    routing: Option<&'a str>,
    plan: Option<&'a str>,
    clear_plan: bool,
    paid_through: Option<&'a str>,
    clear_paid_through: bool,
}

pub fn run(
    cmd: Option<&AccountCommand>,
    provider: Option<Provider>,
    cached: bool,
    details: bool,
    json: bool,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    if let Some(misuse) = AccountCommand::misuse(cmd, provider, cached || details || json) {
        bail!(misuse);
    }
    match (cmd, provider) {
        (Some(AccountCommand::Use { email }), Some(provider)) => {
            rt.block_on(use_account(provider.as_str(), email))
        }
        (Some(cmd), _) => rt.block_on(run_async(cmd)),
        (None, provider) => rt.block_on(account_status::run(provider, !cached, details, json)),
    }
}

async fn run_async(cmd: &AccountCommand) -> Result<()> {
    match cmd {
        AccountCommand::Disconnect { provider, email } => match email {
            Some(email) => disconnect_account(provider, email).await,
            None => disconnect(provider).await,
        },
        AccountCommand::Connect {
            provider,
            email,
            chrome_profile,
            import,
            api_key,
        } => {
            if *api_key {
                return configure(provider).await;
            }
            if *import {
                return import_account(provider, email.as_deref().expect("import requires login"))
                    .await;
            }
            match email.as_deref() {
                Some(email) => connect_account(provider, email, chrome_profile.as_deref()).await,
                None => connect(provider, chrome_profile.as_deref()).await,
            }
        }
        AccountCommand::RedeemReset {
            provider,
            email,
            idempotency_key,
            credit_id,
            json,
        } => {
            redeem_reset(
                provider,
                email,
                idempotency_key.as_deref(),
                credit_id.as_deref(),
                *json,
            )
            .await
        }
        AccountCommand::Use { .. } => unreachable!("run handles use with its provider"),
        AccountCommand::Route {
            cmd,
            repo,
            default,
            json,
        } => super::profile::run_route_async(cmd.as_ref(), repo.as_deref(), *default, *json).await,
        AccountCommand::Set {
            provider,
            email,
            login_email,
            routing,
            plan,
            clear_plan,
            paid_through,
            clear_paid_through,
            clear_cooldown,
            chrome_profile,
            clear_chrome_profiles,
        } => {
            let lifecycle = login_email.is_some()
                || routing.is_some()
                || plan.is_some()
                || *clear_plan
                || paid_through.is_some()
                || *clear_paid_through;
            if !lifecycle && !clear_cooldown && chrome_profile.is_empty() && !clear_chrome_profiles
            {
                return Err(anyhow!("account set needs an account setting or --chrome-profile / --clear-chrome-profiles"));
            }
            if lifecycle {
                set_account_lifecycle(
                    provider,
                    email.as_deref().expect("account settings require login"),
                    AccountLifecycleUpdate {
                        login_email: login_email.as_deref(),
                        routing: routing.as_deref(),
                        plan: plan.as_deref(),
                        clear_plan: *clear_plan,
                        paid_through: paid_through.as_deref(),
                        clear_paid_through: *clear_paid_through,
                    },
                )
                .await?;
            }
            let updated_email = login_email.as_deref().or(email.as_deref());
            if *clear_cooldown {
                clear_account_cooldown(provider, updated_email.expect("cooldown requires login"))
                    .await?;
            }
            if !chrome_profile.is_empty() || *clear_chrome_profiles {
                set_browser_profiles(provider, updated_email, chrome_profile).await?;
            }
            Ok(())
        }
    }
}

/// The only command that changes which account a provider's shared home is
/// signed in as. Running Codex agents keep their login until they restart.
async fn use_account(raw_provider: &str, raw_email: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let store = open_account_store().await?;
    let account = super::profile::find_provider_account(&store, provider, raw_email).await?;
    let native = crate::provider_account::activation::native_home(provider, None);
    let switched = crate::provider_account::activation::activate(
        &store,
        provider,
        &account.account_id,
        &native,
        crate::provider_account::activation::SwitchCause::Person,
    )
    .await?;
    let login = crate::provider_account::account_login(&account);
    match switched {
        Some(_) => println!(
            "{} is now signed in as {login} in {}",
            provider.display_name(),
            native.display()
        ),
        None => println!(
            "{} is already signed in as {login}",
            provider.display_name()
        ),
    }
    Ok(())
}

async fn connect(raw_provider: &str, selection: Option<&str>) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let store = open_account_store().await?;
    let (profiles, remember) = browser_profiles(&store, provider, None, selection).await?;
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
            println!("Connecting local {} through profile '{}'...", provider.display_name(), profile.id);
            if matches!(flow.completion, AuthCompletion::Manual) { println!("Manual authorization is required."); }
            open_chrome_profile(&chrome, url).context("browser launch")?;
            let manual = async {
                if service.pending_supports_authorization_code(provider).await {
                    let code = manual_authorization(&flow, &chrome).await?;
                    service.complete_auth(provider, code.expose_secret()).await?;
                }
                std::future::pending::<Result<()>>().await
            };
            tokio::select! {
                biased;
                result = service.wait_for_auth(provider) => result.map_err(anyhow::Error::from),
                result = manual => result,
                _ = tokio::time::sleep(Duration::from_secs(flow.expires_in.unwrap_or(AUTH_CODE_FLOW_TIMEOUT_SECS))) => Err(anyhow!("authorization wait timed out")),
                _ = tokio::signal::ctrl_c() => Err(anyhow!("authorization cancelled")),
            }
        }.await;
        service.abort_pending(provider).await;
        result?;
        if remember {
            remember_browser_profile(&store, provider, None, &profile).await?;
        }
        println!("Connected local {}", provider.display_name());
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
    account: Option<&ProviderAccount>,
    selection: Option<&str>,
) -> Result<(Vec<AccessProfile>, bool)> {
    if let Some(selection) = selection {
        return Ok((
            vec![bootstrap_access_profile(store, selection).await?],
            true,
        ));
    }
    let account_id = account.map(|account| &account.account_id);
    let bindings = store
        .list_auth_browser_profiles(Some(provider), account_id)
        .await?;
    let mut profiles = Vec::new();
    for binding in bindings
        .into_iter()
        .filter(|binding| binding.account_id.as_ref() == account_id)
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
    let login = account
        .map(|a| format!(" {}", account_login(a)))
        .unwrap_or_default();
    if !std::io::stdin().is_terminal() {
        return Err(anyhow!("No saved Chrome profile. Run lf account connect {provider}{login} --chrome-profile <profile>"));
    }
    let choices = crate::profile::local_chrome_profiles().map_err(anyhow::Error::msg)?;
    for (index, choice) in choices.iter().enumerate() {
        println!("{}. {} ({})", index + 1, choice.name, choice.directory);
    }
    println!("Choose a Chrome profile number (remembered after successful login):");
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
    account_id: Option<&ProviderAccountId>,
    profile: &AccessProfile,
) -> Result<()> {
    if store.get_access_profile(&profile.id).await?.is_none() {
        store.upsert_access_profile(profile).await?;
    }
    store
        .set_auth_browser_profiles(provider, account_id, std::slice::from_ref(&profile.id))
        .await?;
    Ok(())
}

async fn redeem_reset(
    provider: &str,
    email: &str,
    key: Option<&str>,
    credit_id: Option<&str>,
    json: bool,
) -> Result<()> {
    anyhow::ensure!(
        parse_managed_provider(provider)? == Provider::Codex,
        "banked resets are available only for Codex"
    );
    anyhow::ensure!(
        key.is_none_or(|value| !value.trim().is_empty()),
        "idempotency key must not be empty"
    );
    anyhow::ensure!(
        credit_id.is_none_or(|value| !value.trim().is_empty()),
        "credit ID must not be empty"
    );
    let store = open_account_store().await?;
    let accounts = store.list_provider_accounts(Some("codex")).await?;
    let account = match match_account(&accounts.iter().collect::<Vec<_>>(), email) {
        AccountMatch::One(account) => account,
        AccountMatch::Ambiguous(_) => anyhow::bail!("ambiguous login prefix; use a full email"),
        AccountMatch::None => anyhow::bail!("no managed Codex login matches {email}"),
    };
    let home = account
        .home
        .as_deref()
        .context("account has no managed credential home")?;
    let _lock = acquire_managed_login_lock(home, Provider::Codex, &account.account_id)?;
    let key = key
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // Print before the request so a lost reply can be retried without another spend.
    eprintln!("Reset attempt {key}; retry this attempt with --idempotency-key {key}");
    let redemption = crate::subscription::redeem_codex_reset(account, &accounts, &key, credit_id)
        .await
        .with_context(|| {
            format!("reset outcome unconfirmed; retry only with --idempotency-key {key}")
        })?;
    if json {
        let snapshot = |usage: &SubscriptionUsage| {
            serde_json::json!({
                "windows": usage.windows, "reset_credits": usage.reset_credits,
            })
        };
        let report = serde_json::json!({
            "provider": "codex", "account_id": account.account_id, "login": account_login(account),
            "idempotency_key": key, "outcome": redemption.outcome,
            "before": snapshot(&redemption.before),
            "after": redemption.after.as_ref().ok().map(snapshot),
            "refresh_error": redemption.after.as_ref().err().map(ToString::to_string),
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "{}: {} (attempt {key})",
            account_login(account),
            redemption.outcome
        );
        println!("Before:");
        print_reset_usage(&redemption.before);
        println!("After:");
        match &redemption.after {
            Ok(usage) => print_reset_usage(usage),
            Err(error) => println!("  unknown; {error}"),
        }
    }
    for usage in std::iter::once(&redemption.before).chain(redemption.after.as_ref().ok()) {
        store
            .upsert_provider_account_limits("codex", &account.account_id, &usage.windows, "poll")
            .await
            .with_context(|| {
                format!(
                    "reset outcome {}; failed to save usage; attempt {key}",
                    redemption.outcome
                )
            })?;
    }
    Ok(())
}

fn print_reset_usage(usage: &SubscriptionUsage) {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    for window in &usage.windows {
        if window.resets_at.is_some_and(|reset| reset <= now) {
            println!("  {}: usage unknown; reset passed", window.window);
        } else {
            println!(
                "  {}: {}% used, {}% left",
                window.window,
                window.used_percent,
                100u8.saturating_sub(window.used_percent)
            );
        }
    }
    println!(
        "  banked resets: {}",
        usage
            .reset_credits
            .as_ref()
            .map(|credits| credits.available_count.to_string())
            .unwrap_or_else(|| "unknown".into())
    );
}

async fn connect_account(
    raw_provider: &str,
    raw_email: &str,
    raw_chrome_profile: Option<&str>,
) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let store = open_account_store().await?;
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let account = match match_account(&accounts.iter().collect::<Vec<_>>(), raw_email) {
        AccountMatch::One(account) => account.clone(),
        AccountMatch::Ambiguous(_) => {
            return Err(anyhow!("ambiguous login prefix; use a full email"))
        }
        AccountMatch::None => {
            let email = EmailAddress::parse(raw_email).map_err(|_| {
                anyhow!("new accounts require a full email; prefixes select existing accounts")
            })?;
            let id = account_id_for_login(&email);
            new_account(
                provider,
                id.clone(),
                account_home_path(provider, &id)?,
                Some(email),
            )
        }
    };
    if account.login_email.is_none() {
        return Err(anyhow!(
            "account '{}' needs an expected email first: lf account set {} {} --login-email <email>",
            account.account_id,
            provider,
            account.account_id
        ));
    }
    let (candidates, remember) =
        browser_profiles(&store, provider, Some(&account), raw_chrome_profile).await?;
    let mut failures = Vec::new();
    for profile in candidates {
        let attempt = match verified_chrome_profile(&profile) {
            Ok(chrome_profile) => {
                match connect_managed_account(&store, provider, &account, &profile, chrome_profile)
                    .await
                {
                    Err(error) if error.downcast_ref::<LoginMismatch>().is_none() => {
                        return Err(error)
                    }
                    result => result,
                }
            }
            Err(error) => Err(error),
        };
        match attempt {
            Ok(()) => {
                if remember {
                    remember_browser_profile(&store, provider, Some(&account.account_id), &profile)
                        .await?;
                }
                return Ok(());
            }
            Err(error) => {
                let failure = format!("{}: {error}", profile.id);
                #[cfg(test)]
                TEST_ACCESS_PROFILE_FAILURES
                    .lock()
                    .expect("test access profile failure log should not be poisoned")
                    .push(failure.clone());
                failures.push(failure);
            }
        }
    }
    Err(exhausted_access_profiles_error(
        provider, &account, &failures,
    ))
}

async fn connect_managed_account(
    store: &SharedStore,
    provider: Provider,
    account: &ProviderAccount,
    profile: &AccessProfile,
    chrome_profile: LocalChromeProfile,
) -> Result<()> {
    let account_id = &account.account_id;
    let account_home = account_home_path(provider, account_id)?;
    let parent = account_home
        .parent()
        .ok_or_else(|| anyhow!("account home has no parent directory"))?;
    fs::create_dir_all(parent).context("create provider accounts directory")?;
    let _login_lock = acquire_managed_login_lock(&account_home, provider, account_id)?;
    let login_home = tempfile::Builder::new()
        .prefix(".login-")
        .tempdir_in(parent)
        .context("create private provider login home")?;
    authorize_managed_login(provider, account, &login_home, profile, chrome_profile).await?;
    let _identity_lock =
        crate::provider_account::identity::acquire_identity_install_lock(&account_home)?;
    let (identity, plan) =
        verify_managed_identity(store, provider, account, login_home.path()).await?;
    let account_home = ensure_account_home(provider, account_id)?;
    match provider {
        Provider::Claude => install_claude_login(login_home.path(), &account_home)?,
        Provider::Codex => install_codex_login(login_home.path(), &account_home)?,
        _ => return Err(anyhow!("unsupported managed provider '{provider}'")),
    }
    register_managed_account(store, provider, account_id, account_home, identity, plan).await?;
    println!(
        "Connected {} login '{}' through profile '{}'",
        provider.display_name(),
        account_login(account),
        profile.id
    );
    Ok(())
}

async fn authorize_managed_login(
    provider: Provider,
    account: &ProviderAccount,
    login_home: &tempfile::TempDir,
    profile: &AccessProfile,
    chrome_profile: LocalChromeProfile,
) -> Result<()> {
    let account_id = &account.account_id;
    let auth_home = login_home.path().to_path_buf();
    let handle = start_provider_account_auth(
        provider,
        auth_home.clone(),
        account.login_email.as_ref().map(EmailAddress::as_str),
    )
    .await?;
    let flow = handle.response.clone();
    let verification_url = flow
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| flow.verification_uri.clone());
    println!(
        "Connecting {} login '{}' through profile '{}'...",
        provider.display_name(),
        account_login(account),
        profile.id
    );
    let input = handle.code_input();
    let manual = async {
        if let Some(input) = input {
            let code = manual_authorization(&flow, &chrome_profile).await?;
            input.submit(code.expose_secret()).await?;
        }
        std::future::pending::<Result<()>>().await
    };
    if matches!(flow.completion, AuthCompletion::Manual) {
        println!("Manual authorization is required; approve in Chrome, then enter the code here.");
    } else {
        println!("Approve authorization in Chrome; login completes automatically.");
    }
    open_chrome_profile(&chrome_profile, &verification_url).context("browser launch")?;
    let completion = wait_for_browser_confirmation(
        handle.wait(),
        flow.expires_in,
        provider,
        account_login(account),
    );
    tokio::pin!(completion);
    tokio::select! {
        biased;
        result = &mut completion => result.context("authorization wait")?,
        result = manual => result?,
        _ = tokio::signal::ctrl_c() => return Err(anyhow!("authorization cancelled")),
    }

    match provider {
        Provider::Claude => {
            capture_claude_profile_credentials(&auth_home)?;
            require_managed_access_token(provider, account_id, &auth_home).await?;
        }
        Provider::Codex => {}
        _ => return Err(anyhow!("unsupported managed provider '{provider}'")),
    }
    Ok(())
}

/// Mint a separate refresh chain without installing it in the origin account.
pub(super) async fn fresh_machine_login(
    store: &SharedStore,
    provider: Provider,
    account: &ProviderAccount,
    chrome_profile: Option<&str>,
) -> Result<tempfile::TempDir> {
    let (profiles, _) = browser_profiles(store, provider, Some(account), chrome_profile).await?;
    let profile = profiles
        .first()
        .ok_or_else(|| anyhow!("no browser profile available"))?;
    let chrome = verified_chrome_profile(profile)?;
    let login_home = tempfile::Builder::new()
        .prefix("lf-machine-login-")
        .tempdir()?;
    authorize_managed_login(provider, account, &login_home, profile, chrome).await?;
    let (identity, _) =
        verify_managed_identity(store, provider, account, login_home.path()).await?;
    if account
        .observed_subject
        .as_ref()
        .is_some_and(|subject| subject != &identity.subject)
    {
        bail!("fresh login belongs to a different provider identity; credential discarded");
    }
    Ok(login_home)
}

pub(super) async fn fresh_linear_login(
    store: &SharedStore,
    chrome_profile: Option<&str>,
) -> Result<ProviderToken> {
    let (profiles, _) = browser_profiles(store, Provider::Linear, None, chrome_profile).await?;
    let profile = profiles
        .first()
        .ok_or_else(|| anyhow!("no browser profile available"))?;
    let chrome = verified_chrome_profile(profile)?;
    let broker = crate::provider_auth::LinearOAuthBroker::new();
    let handle = broker.start_auth().await?;
    let flow = handle.response.clone();
    let url = flow
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&flow.verification_uri);
    open_chrome_profile(&chrome, url)?;
    wait_for_browser_confirmation(
        handle.wait(),
        flow.expires_in,
        Provider::Linear,
        "remote machine",
    )
    .await?;
    broker
        .extract_token()
        .await
        .ok_or_else(|| anyhow!("Linear login completed without a credential"))
}

async fn wait_for_browser_confirmation<F>(
    confirmation: F,
    expires_in: Option<u64>,
    provider: Provider,
    login: &str,
) -> Result<()>
where
    F: Future<Output = std::result::Result<(), AuthError>>,
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
            result = &mut confirmation => return result.map_err(anyhow::Error::from),
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

fn exhausted_access_profiles_error(
    provider: Provider,
    account: &ProviderAccount,
    failures: &[String],
) -> anyhow::Error {
    anyhow!(
        "Chrome profiles unavailable for {provider}/{}. {} Choose a profile: lf account connect {provider} {} --chrome-profile <profile>",
        account_login(account), failures.join("; "), account_login(account)
    )
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct LoginMismatch(String);

fn install_codex_login(login_home: &Path, account_home: &Path) -> Result<()> {
    let source = login_home.join("auth.json");
    if !source.is_file() {
        return Err(anyhow!("Codex login did not produce an OAuth credential"));
    }
    fs::rename(source, account_home.join("auth.json")).context("install verified Codex credential")
}

fn install_claude_login(login_home: &Path, account_home: &Path) -> Result<()> {
    let source = login_home.join(".credentials.json");
    if !source.is_file() {
        return Err(anyhow!("Claude login did not produce an OAuth credential"));
    }
    fs::rename(source, account_home.join(".credentials.json"))
        .context("install verified Claude credential")
}

pub(super) async fn verify_managed_identity(
    store: &SharedStore,
    provider: Provider,
    account: &ProviderAccount,
    home: &Path,
) -> Result<(
    crate::provider_account::identity::AccountIdentity,
    Option<String>,
)> {
    let (identity, plan) = match provider {
        Provider::Codex => crate::provider_auth::verify_codex_identity(home).await?,
        Provider::Claude => (crate::subscription::claude_identity(home).await?, None),
        _ => return Err(anyhow!("unsupported managed provider")),
    };
    // A different browser login can try the next saved profile; other failures stop connect.
    if let Some(expected) = &account.login_email {
        if !identity.email.eq_ignore_ascii_case(expected.as_str()) {
            return Err(LoginMismatch(format!(
                "{} reports {}; expected {}. Refused: the login was discarded and '{}' is unchanged.",
                provider.display_name(), identity.email, expected, expected
            )).into());
        }
    }
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let mut intended = account.clone();
    intended.observed_subject = None;
    crate::provider_account::identity::validate_observed_identity(&intended, &identity, &accounts)
        .await?;
    Ok((identity, plan))
}

pub(super) async fn register_managed_account(
    store: &SharedStore,
    provider: Provider,
    account_id: &ProviderAccountId,
    account_home: PathBuf,
    identity: crate::provider_account::identity::AccountIdentity,
    plan: Option<String>,
) -> Result<()> {
    let login_email = EmailAddress::parse(&identity.email).map_err(anyhow::Error::msg)?;
    let mut account = store
        .get_provider_account(provider.as_str(), account_id)
        .await?
        .unwrap_or_else(|| {
            new_account(
                provider,
                account_id.clone(),
                account_home.clone(),
                Some(login_email.clone()),
            )
        });
    account.observed_email = Some(identity.email);
    account.observed_subject = Some(identity.subject);
    account.observed_credential_digest = identity.credential_digest;
    account.observed_plan = plan;
    account.home = Some(account_home);
    account.login_email = Some(login_email);
    account.credential_state = CredentialState::Connected;
    account.updated_at = OffsetDateTime::now_utc().unix_timestamp();
    store.upsert_provider_account(&account).await?;
    Ok(())
}

async fn import_account(raw_provider: &str, raw_email: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let expected_login = EmailAddress::parse(raw_email).map_err(anyhow::Error::msg)?;
    let store = open_account_store().await?;
    let existing = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?
        .into_iter()
        .find(|account| {
            account
                .login_email
                .as_ref()
                .is_some_and(|login| login.as_str().eq_ignore_ascii_case(expected_login.as_str()))
        });
    let account_id = existing
        .as_ref()
        .map(|account| account.account_id.clone())
        .unwrap_or_else(|| account_id_for_login(&expected_login));
    let account_home = account_home_path(provider, &account_id)?;

    let parent = account_home
        .parent()
        .ok_or_else(|| anyhow!("account home has no parent directory"))?;
    fs::create_dir_all(parent).context("create provider accounts directory")?;
    let _login_lock = acquire_managed_login_lock(&account_home, provider, &account_id)?;

    let credentials_file = match provider {
        Provider::Claude => ".credentials.json",
        Provider::Codex => "auth.json",
        _ => unreachable!("parse_managed_provider admits Claude and Codex only"),
    };
    let staged_home = if account_home.join(credentials_file).is_file() {
        None
    } else {
        if provider != Provider::Claude {
            return Err(anyhow!("no stored {} login at {}; importing the ambient login is supported for Claude only", provider.display_name(), account_home.display()));
        }
        let staged_home = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(parent)
            .context("create private provider import home")?;
        import_ambient_claude_profile_credentials(staged_home.path())?;
        Some(staged_home)
    };

    let credential_home = staged_home
        .as_ref()
        .map(|home| home.path())
        .unwrap_or(account_home.as_path());
    require_managed_access_token(provider, &account_id, credential_home).await?;
    let _identity_lock =
        crate::provider_account::identity::acquire_identity_install_lock(&account_home)?;
    let intended = new_account(
        provider,
        account_id.clone(),
        account_home.clone(),
        Some(expected_login.clone()),
    );
    let (identity, plan) =
        verify_managed_identity(&store, provider, &intended, credential_home).await?;
    let account_home = ensure_account_home(provider, &account_id)?;
    if let Some(staged_home) = staged_home {
        install_claude_login(staged_home.path(), &account_home)?;
    }
    register_managed_account(&store, provider, &account_id, account_home, identity, plan).await?;
    println!(
        "Imported {} login '{}'",
        provider.display_name(),
        expected_login
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

async fn manual_authorization(
    flow: &AuthFlowResponse,
    profile: &LocalChromeProfile,
) -> Result<SecretString> {
    let Some(mut terminal) = auth_input::AuthInput::open()? else {
        return std::future::pending().await;
    };
    if let AuthCompletion::Browser { manual_uri } = &flow.completion {
        let Some(url) = manual_uri.as_deref() else {
            return std::future::pending().await;
        };
        println!(
            "If callback completion is unavailable, type m then Enter to open the manual route."
        );
        loop {
            if terminal.line().await?.expose_secret().trim() == "m" {
                open_chrome_profile(profile, url).context("manual browser launch")?;
                break;
            }
        }
    }
    println!("Paste the one-time authorization code, then Enter (input hidden).");
    loop {
        let code = terminal.line().await?;
        if !code.expose_secret().trim().is_empty() {
            return Ok(code);
        }
    }
}

#[cfg(all(not(target_os = "macos"), not(test)))]
fn open_chrome_profile(_profile: &LocalChromeProfile, _url: &str) -> Result<()> {
    Err(anyhow!(
        "opening a selected Chrome profile is currently supported on macOS only"
    ))
}

#[cfg(test)]
fn open_chrome_profile(profile: &LocalChromeProfile, _url: &str) -> Result<()> {
    TEST_OPENED_CHROME_PROFILES
        .lock()
        .expect("test Chrome profile log should not be poisoned")
        .push(profile.directory.clone());
    Ok(())
}

async fn disconnect(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let service = local_auth_service().await?;
    service.disconnect(provider).await?;
    println!("Disconnected local {}", provider.display_name());
    Ok(())
}

async fn disconnect_account(raw_provider: &str, raw_email: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let store = open_account_store().await?;
    let mut account = super::profile::find_provider_account(&store, provider, raw_email).await?;
    let account_id = account.account_id.clone();
    let _login_lock = account
        .home
        .as_deref()
        .filter(|home| home.parent().is_some_and(|parent| parent.exists()))
        .map(|home| acquire_managed_login_lock(home, provider, &account_id))
        .transpose()?;
    if let Some(home) = account.home.as_deref() {
        let expected_machine = account_home_path(provider, &account_id)?;
        if home != expected_machine {
            return Err(anyhow!(
                "refusing to remove unexpected {} account home {}",
                provider.display_name(),
                home.display()
            ));
        }
        disconnect_provider_account_auth(provider, expected_machine.clone()).await?;
        remove_account_home(&expected_machine)?;
    }
    account.credential_state = CredentialState::Missing;
    account.utilization_percent = None;
    account.cooldown_until = None;
    account.cooldown_reason = None;
    account.updated_at = OffsetDateTime::now_utc().unix_timestamp();
    store.upsert_provider_account(&account).await?;
    println!(
        "Disconnected {} login '{}'",
        provider.display_name(),
        account_login(&account)
    );
    Ok(())
}

async fn set_browser_profiles(
    provider: &str,
    email: Option<&str>,
    selections: &[String],
) -> Result<()> {
    let provider = parse_provider(provider)?;
    let store = open_account_store().await?;
    let account = match email {
        Some(email) => {
            parse_managed_provider(provider.as_str())?;
            Some(super::profile::find_provider_account(&store, provider, email).await?)
        }
        None => None,
    };
    let mut selected = Vec::new();
    for selection in selections {
        let profile = bootstrap_access_profile(&store, selection).await?;
        if !selected.iter().any(|p: &AccessProfile| p.id == profile.id) {
            selected.push(profile);
        }
    }
    for profile in &selected {
        store.upsert_access_profile(profile).await?;
    }
    let profiles = selected
        .into_iter()
        .map(|profile| profile.id)
        .collect::<Vec<_>>();
    store
        .set_auth_browser_profiles(provider, account.as_ref().map(|a| &a.account_id), &profiles)
        .await?;
    println!(
        "Updated {provider} browser choices for {}",
        account
            .as_ref()
            .map(account_login)
            .unwrap_or("local credentials")
    );
    Ok(())
}

async fn set_account_lifecycle(
    raw_provider: &str,
    raw_email: &str,
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
            "lf account set needs --login-email, --routing, --plan, or --paid-through"
        ));
    }
    let provider = parse_managed_provider(raw_provider)?;
    let login_email = update
        .login_email
        .map(EmailAddress::parse)
        .transpose()
        .map_err(anyhow::Error::msg)?;
    let routing_state = update.routing.map(parse_routing_state).transpose()?;
    let plan = update.plan.map(parse_plan).transpose()?;
    let paid_through = update.paid_through.map(parse_paid_through).transpose()?;
    let store = open_account_store().await?;
    let mut account = super::profile::find_provider_account(&store, provider, raw_email).await?;
    if let Some(login_email) = login_email {
        if let Some(home) = &account.home {
            let observed = match provider {
                Provider::Codex if home.join("auth.json").exists() => {
                    Some(crate::provider_auth::codex_identity_from_home(home)
                        .ok_or_else(|| anyhow!("cannot read credential identity; reconnect before changing the login email"))?.email)
                }
                Provider::Claude if home.join(".credentials.json").exists() => {
                    Some(crate::subscription::claude_identity(home).await?.email)
                }
                _ => None,
            };
            if let Some(observed) = observed {
                if !observed.eq_ignore_ascii_case(login_email.as_str()) {
                    return Err(anyhow!("credential reports {observed}; cannot relabel it as {login_email}. Reconnect with lf account connect {provider} {login_email}"));
                }
            }
        }
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
        .map_err(|error| account_store_error(provider, account_login(&account), error))?;
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

async fn clear_account_cooldown(raw_provider: &str, raw_email: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let store = open_account_store().await?;
    let account = super::profile::find_provider_account(&store, provider, raw_email).await?;
    store
        .clear_provider_account_cooldown(provider.as_str(), &account.account_id)
        .await
        .map_err(|error| account_store_error(provider, account_login(&account), error))?;
    println!(
        "Cleared {} login '{}' cooldown",
        provider.display_name(),
        account_login(&account)
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
    if details.is_empty() {
        details.push("automatic".to_string());
    }
    format!(
        "{:<12} {:<32} {}",
        account.provider,
        account_login(account),
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

fn parse_provider(raw: &str) -> Result<Provider> {
    raw.parse::<Provider>()
        .map_err(|_| anyhow!("unknown provider: {raw}"))
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
    use std::fs;
    use std::time::Duration;

    use time::OffsetDateTime;

    use crate::profile::EmailAddress;
    use crate::provider_auth::{AuthError, Provider};
    use crate::store::{CredentialState, ProviderAccount, ProviderAccountId, RoutingState};

    use super::{
        acquire_managed_login_lock, format_account, format_relative_delta, import_account,
        install_claude_login, install_codex_login, parse_paid_through, parse_routing_state,
        wait_for_browser_confirmation,
    };

    #[tokio::test(start_paused = true)]
    async fn browser_confirmation_survives_old_timeout_boundary() {
        let started_at = tokio::time::Instant::now();
        let confirmation = async {
            tokio::time::sleep(Duration::from_secs(181)).await;
            Ok::<(), AuthError>(())
        };

        wait_for_browser_confirmation(confirmation, None, Provider::Codex, "operator@example.com")
            .await
            .expect("browser confirmation should outlive the old timeout");

        assert_eq!(started_at.elapsed(), Duration::from_secs(181));
    }

    #[tokio::test(start_paused = true)]
    async fn browser_confirmation_uses_ten_minute_default() {
        let started_at = tokio::time::Instant::now();

        let error = wait_for_browser_confirmation(
            std::future::pending::<std::result::Result<(), AuthError>>(),
            None,
            Provider::Codex,
            "operator@example.com",
        )
        .await
        .expect_err("browser confirmation should remain bounded");

        assert_eq!(started_at.elapsed(), Duration::from_secs(600));
        assert!(error
            .to_string()
            .contains("timed out waiting for Codex account 'operator@example.com'"));
    }

    #[tokio::test(start_paused = true)]
    async fn browser_confirmation_honors_provider_expiry() {
        let started_at = tokio::time::Instant::now();

        let error = wait_for_browser_confirmation(
            std::future::pending::<std::result::Result<(), AuthError>>(),
            Some(75),
            Provider::Claude,
            "operator@example.com",
        )
        .await
        .expect_err("provider expiry should bound browser confirmation");

        assert_eq!(started_at.elapsed(), Duration::from_secs(75));
        assert!(error
            .to_string()
            .contains("timed out waiting for Claude account 'operator@example.com'"));
    }

    #[test]
    fn format_account_shows_routing_state() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let rendered = format_account(&ProviderAccount {
            provider: "claude".to_string(),
            account_id: ProviderAccountId::parse("primary").unwrap(),
            home: None,
            login_email: Some(EmailAddress::parse("operator@example.com").unwrap()),
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
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
        assert!(!rendered.contains("primary"));
        assert!(!rendered.contains("preferred"));
        assert!(rendered.contains("max"));
        assert!(rendered.contains("72% used"));
        assert!(rendered.contains("cooling for"));
        assert!(rendered.contains("operator@example.com"));
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
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
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
    fn format_relative_delta_uses_human_units() {
        assert_eq!(format_relative_delta(42), "42s");
        assert_eq!(format_relative_delta(180), "3m");
        assert_eq!(format_relative_delta(7_200), "2h");
        assert_eq!(format_relative_delta(172_800), "2d");
    }

    // The env lock must span the awaited import so no parallel test swaps
    // LF_HOME mid-flight; the single-threaded test runtime makes that safe.
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn codex_import_without_stored_credentials_names_the_ambient_limit() {
        // Codex import adopts an existing auth.json in the account home; with
        // none present there is no ambient fallback (that path is Claude's).
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("LF_HOME");
        std::env::set_var("LF_HOME", home.path());
        let result = import_account("codex", "engineering@example.com").await;
        match previous {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }

        let error = result.expect_err("Codex imports must require a stored login");
        assert!(error
            .to_string()
            .contains("importing the ambient login is supported for Claude only"));
    }

    #[test]
    fn verified_codex_login_replaces_the_previous_credential() {
        let parent = tempfile::tempdir().unwrap();
        let login_home = parent.path().join("login");
        let account_home = parent.path().join("account");
        fs::create_dir_all(&login_home).unwrap();
        fs::create_dir_all(&account_home).unwrap();
        fs::write(login_home.join("auth.json"), "verified").unwrap();
        fs::write(account_home.join("auth.json"), "previous").unwrap();

        install_codex_login(&login_home, &account_home).unwrap();

        assert_eq!(
            fs::read_to_string(account_home.join("auth.json")).unwrap(),
            "verified"
        );
        assert!(!login_home.join("auth.json").exists());
    }

    #[test]
    fn verified_claude_login_replaces_the_previous_credential() {
        let parent = tempfile::tempdir().unwrap();
        let login_home = parent.path().join("login");
        let account_home = parent.path().join("account");
        fs::create_dir_all(&login_home).unwrap();
        fs::create_dir_all(&account_home).unwrap();
        fs::write(login_home.join(".credentials.json"), "verified").unwrap();
        fs::write(account_home.join(".credentials.json"), "previous").unwrap();

        install_claude_login(&login_home, &account_home).unwrap();

        assert_eq!(
            fs::read_to_string(account_home.join(".credentials.json")).unwrap(),
            "verified"
        );
        assert!(!login_home.join(".credentials.json").exists());
    }

    #[test]
    fn concurrent_login_for_the_same_account_is_rejected() {
        let account_home = tempfile::tempdir().unwrap();
        let account_id = ProviderAccountId::parse("engineering").unwrap();
        let _first =
            acquire_managed_login_lock(account_home.path(), Provider::Codex, &account_id).unwrap();

        let error = acquire_managed_login_lock(account_home.path(), Provider::Codex, &account_id)
            .expect_err("second login must not open another browser flow");

        assert!(error.to_string().contains("already in progress"));
    }
}

#[cfg(test)]
mod account_first_tests {
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use serde_json::json;

    use super::{
        connect_account, exhausted_access_profiles_error, TEST_ACCESS_PROFILE_FAILURES,
        TEST_OPENED_CHROME_PROFILES,
    };
    use crate::profile::{AccessProfile, EmailAddress, ProfileId};
    const ACCOUNT_LEASE_ENV: &str = "LF_ACCOUNT_LEASE";
    use crate::provider_account::{account_home_path, parse_account_id};
    use crate::provider_auth::Provider;
    use crate::store::{CredentialState, ProviderAccount, RoutingState, StorageConfig};
    use tempfile::tempdir;

    const CONNECT_ENV: &[&str] = &[
        "HOME",
        "LF_HOME",
        "PATH",
        ACCOUNT_LEASE_ENV,
        "LF_TEST_CODEX_AUTH_JSON",
        "LF_TEST_CODEX_EMAIL",
        "LF_TEST_CODEX_RELEASE",
        "LF_TEST_CODEX_COUNT",
        "LF_TEST_CODEX_HOMES",
        "LF_TEST_CODEX_FAIL_FIRST",
    ];

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    fn configure_connect_test(temp: &Path, reported_login: &str, fail_first: bool) {
        let bin = temp.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let codex = bin.join("codex");
        fs::write(
            &codex,
            r#"#!/bin/sh
case "$*" in
  *app-server*) ;;
  *) exit 90;; # Native login would open a second browser.
esac
read -r initialize
echo '{"id":1,"result":{}}'
read -r initialized
read -r request
case "$request" in
  *account/read*)
    printf '{"id":2,"result":{"account":{"email":"%s"}}}\n' "$LF_TEST_CODEX_EMAIL"
    exit 0;;
  *account/login/start*) ;;
  *) exit 91;;
esac
count=0
if [ -f "$LF_TEST_CODEX_COUNT" ]; then count=$(cat "$LF_TEST_CODEX_COUNT"); fi
count=$((count + 1))
printf '%s' "$count" > "$LF_TEST_CODEX_COUNT"
printf '%s\n' "$CODEX_HOME" >> "$LF_TEST_CODEX_HOMES"
if [ "$LF_TEST_CODEX_FAIL_FIRST" = "1" ] && [ "$count" = "1" ]; then exit 1; fi
echo '{"id":2,"result":{"type":"chatgpt","loginId":"fixture-login","authUrl":"https://auth.openai.com/oauth/authorize?client_id=test"}}'
if [ -n "$LF_TEST_CODEX_RELEASE" ]; then
  while [ ! -f "$LF_TEST_CODEX_RELEASE" ]; do sleep 0.05; done
fi
mkdir -p "$CODEX_HOME"
cp "$LF_TEST_CODEX_AUTH_JSON" "$CODEX_HOME/auth.json"
echo '{"method":"account/login/completed","params":{"loginId":"fixture-login","success":true}}'
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&codex).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&codex, permissions).unwrap();
        }
        let claims = URL_SAFE_NO_PAD.encode(format!(
            r#"{{"email":"{reported_login}","sub":"user-{reported_login}"}}"#
        ));
        let auth_json = temp.join("codex-auth.json");
        fs::write(
            &auth_json,
            serde_json::json!({
                "tokens": {
                    "access_token": "test-access-token",
                    "id_token": format!("header.{claims}.signature"),
                }
            })
            .to_string(),
        )
        .unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        std::env::set_var("HOME", temp);
        std::env::set_var("LF_HOME", temp);
        std::env::remove_var(ACCOUNT_LEASE_ENV);
        std::env::set_var("LF_TEST_CODEX_AUTH_JSON", auth_json);
        std::env::set_var("LF_TEST_CODEX_EMAIL", reported_login);
        std::env::set_var("LF_TEST_CODEX_COUNT", temp.join("codex-count"));
        std::env::set_var("LF_TEST_CODEX_HOMES", temp.join("codex-homes"));
        std::env::set_var(
            "LF_TEST_CODEX_FAIL_FIRST",
            if fail_first { "1" } else { "0" },
        );
        TEST_ACCESS_PROFILE_FAILURES.lock().unwrap().clear();
        TEST_OPENED_CHROME_PROFILES.lock().unwrap().clear();
    }

    fn write_chrome_profiles(temp: &Path, profiles: &[(&str, &str, &str)]) {
        let info_cache = profiles
            .iter()
            .map(|(directory, name, login)| {
                (
                    (*directory).to_string(),
                    serde_json::json!({"name": name, "user_name": login}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let local_state = temp.join("Library/Application Support/Google/Chrome/Local State");
        fs::create_dir_all(local_state.parent().unwrap()).unwrap();
        fs::write(
            local_state,
            serde_json::json!({"profile": {"info_cache": info_cache}}).to_string(),
        )
        .unwrap();
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn stale_refresh_rejection_preserves_replacement_account_evidence() {
        let _lock = crate::journal::test_env_lock();
        verify_replaced_claude_credential(false).await;
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn stale_usage_acceptance_preserves_replacement_account_evidence() {
        let _lock = crate::journal::test_env_lock();
        verify_replaced_claude_credential(true).await;
    }

    async fn verify_replaced_claude_credential(accepted: bool) {
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&["LF_HOME", ACCOUNT_LEASE_ENV]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var(ACCOUNT_LEASE_ENV);
        let home = temp.path().join("claude");
        fs::create_dir(&home).unwrap();
        let path = home.join(".credentials.json");
        fs::write(
            &path,
            json!({"claudeAiOauth": {
                "accessToken": "original-access", "refreshToken": "original-refresh",
                "expiresAt": if accepted { 4102444800000i64 } else { 1 },
                "subscriptionType": "max"
            }})
            .to_string(),
        )
        .unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            temp.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        let mut account = crate::provider_account::new_account(
            Provider::Claude,
            parse_account_id("personal").unwrap(),
            home,
            None,
        );
        account.credential_state = if accepted {
            CredentialState::Missing
        } else {
            CredentialState::Connected
        };
        account.cooldown_until = Some(4102444800);
        account.cooldown_reason = Some("retained cooldown".into());
        store.upsert_provider_account(&account).await.unwrap();
        store
            .upsert_provider_account_limits(
                "claude",
                &account.account_id,
                &[crate::store::AccountLimitWindow {
                    window: "session".into(),
                    used_percent: 17,
                    resets_at: Some(4102444800),
                    plan: None,
                }],
                "stream",
            )
            .await
            .unwrap();
        let before_account = store
            .get_provider_account("claude", &account.account_id)
            .await
            .unwrap();
        let before_windows = store.provider_account_limits(Some("claude")).await.unwrap();
        let replacement =
            json!({"claudeAiOauth": {"accessToken": "native-replacement"}}).to_string();
        let replace_path = path.clone();
        let replace_bytes = replacement.clone();
        let response = if accepted {
            (
                200,
                json!({"limits": [{"group": "session", "percent": 96}]}).to_string(),
            )
        } else {
            (400, json!({"error": "invalid_grant"}).to_string())
        };
        let (_endpoints, server) =
            crate::subscription::observation_tests::serve(vec![response], move |_| {
                // The native writer deliberately does not take Loopflow's login lock.
                fs::write(&replace_path, &replace_bytes).unwrap();
            })
            .await;
        super::account_status::run(Some(Provider::Claude), true, false, false)
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), replacement);
        assert_eq!(
            store
                .get_provider_account("claude", &account.account_id)
                .await
                .unwrap(),
            before_account
        );
        assert_eq!(
            store.provider_account_limits(Some("claude")).await.unwrap(),
            before_windows
        );
    }

    fn account(account_home: Option<&Path>, login: &str) -> ProviderAccount {
        ProviderAccount {
            provider: "codex".to_string(),
            account_id: parse_account_id("primary").unwrap(),
            home: account_home.map(Path::to_path_buf),
            login_email: Some(EmailAddress::parse(login).unwrap()),
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: if account_home.is_some() {
                CredentialState::Connected
            } else {
                CredentialState::Missing
            },
            routing_state: RoutingState::Automatic,
            plan: Some("plus".to_string()),
            paid_through: None,
            utilization_percent: Some(12),
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: Some(7),
            created_at: 1,
            updated_at: 2,
        }
    }

    fn access_profile(directory: &str, login: &str, position: i64) -> AccessProfile {
        AccessProfile {
            id: ProfileId::parse(directory).unwrap(),
            chrome_directory: directory.to_string(),
            expected_login: Some(EmailAddress::parse(login).unwrap()),
            created_at: position,
            updated_at: position,
        }
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn fresh_machine_login_does_not_replace_the_laptop_login() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "operator@example.com", false);
        write_chrome_profiles(
            temp.path(),
            &[("Profile 3", "Work", "operator@example.com")],
        );
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let home = temp.path().join("laptop-login");
        fs::create_dir(&home).unwrap();
        fs::write(home.join("auth.json"), "unchanged-laptop-refresh-chain").unwrap();
        let original = account(Some(&home), "operator@example.com");
        store.upsert_provider_account(&original).await.unwrap();
        let profile = access_profile("Profile 3", "operator@example.com", 1);
        store.upsert_access_profile(&profile).await.unwrap();
        store
            .set_auth_browser_profiles(
                Provider::Codex,
                Some(&original.account_id),
                std::slice::from_ref(&profile.id),
            )
            .await
            .unwrap();
        let staging = super::fresh_machine_login(&store, Provider::Codex, &original, None)
            .await
            .unwrap();
        assert_ne!(staging.path(), home);
        assert!(staging.path().join("auth.json").is_file());
        assert_eq!(
            fs::read_to_string(home.join("auth.json")).unwrap(),
            "unchanged-laptop-refresh-chain"
        );
        assert_eq!(
            store
                .get_provider_account("codex", &original.account_id)
                .await
                .unwrap(),
            Some(original)
        );
        let path = staging.path().to_path_buf();
        drop(staging);
        assert!(!path.exists());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn saved_browser_directory_never_resolves_as_another_profiles_name() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "operator@example.com", false);
        let mut saved = access_profile("Profile 3", "operator@example.com", 1);
        saved.expected_login = None;
        write_chrome_profiles(
            temp.path(),
            &[("Profile 3", "Work", ""), ("Profile 8", "Profile 3", "")],
        );
        assert_eq!(
            super::verified_chrome_profile(&saved).unwrap().directory,
            "Profile 3"
        );

        write_chrome_profiles(temp.path(), &[("Profile 8", "Profile 3", "")]);
        let error = super::verified_chrome_profile(&saved).unwrap_err();
        assert!(error.to_string().contains("Profile 3"));
        assert!(error.to_string().contains("missing"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn fresh_login_remembers_unsigned_chrome_profile_without_seeded_account() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "fresh@example.com", false);
        write_chrome_profiles(temp.path(), &[("Profile 3", "Personal", "")]);
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        connect_account("codex", "fresh@example.com", Some("Personal"))
            .await
            .unwrap();
        let accounts = store.list_provider_accounts(Some("codex")).await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(
            accounts[0].login_email.as_ref().unwrap().as_str(),
            "fresh@example.com"
        );
        assert_eq!(accounts[0].credential_state, CredentialState::Connected);
        let profiles = store.list_access_profiles().await.unwrap();
        assert_eq!(profiles[0].id.as_str(), "Personal");
        assert_eq!(profiles[0].expected_login, None);
        connect_account("codex", "fresh@", None).await.unwrap();
        assert_eq!(
            store.list_provider_accounts(Some("codex")).await.unwrap()[0].account_id,
            accounts[0].account_id
        );
        assert_eq!(
            *TEST_OPENED_CHROME_PROFILES.lock().unwrap(),
            ["Profile 3", "Profile 3"]
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn failed_fresh_login_registers_neither_account_nor_profile() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "other@example.com", false);
        write_chrome_profiles(temp.path(), &[("Profile 3", "Personal", "")]);
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let error = connect_account("codex", "fresh@example.com", Some("Personal"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("expected fresh@example.com"));
        assert!(store.list_provider_accounts(None).await.unwrap().is_empty());
        assert!(store.list_access_profiles().await.unwrap().is_empty());
        assert!(store
            .list_auth_browser_profiles(None, None)
            .await
            .unwrap()
            .is_empty());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn linear_and_account_browser_bindings_are_independent_and_reusable() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "operator@example.com", false);
        write_chrome_profiles(
            temp.path(),
            &[("Profile 3", "Work", ""), ("Profile 8", "Personal", "")],
        );
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account = account(None, "operator@example.com");
        store.upsert_provider_account(&account).await.unwrap();
        let work = super::bootstrap_access_profile(&store, "Work")
            .await
            .unwrap();
        super::remember_browser_profile(&store, Provider::Linear, None, &work)
            .await
            .unwrap();
        super::remember_browser_profile(&store, Provider::Codex, Some(&account.account_id), &work)
            .await
            .unwrap();
        let personal = super::bootstrap_access_profile(&store, "Personal")
            .await
            .unwrap();
        super::remember_browser_profile(
            &store,
            Provider::Codex,
            Some(&account.account_id),
            &personal,
        )
        .await
        .unwrap();
        let (profiles, remember) = super::browser_profiles(&store, Provider::Linear, None, None)
            .await
            .unwrap();
        assert!(!remember);
        assert_eq!(profiles, vec![work]);
        let chrome = super::verified_chrome_profile(&profiles[0]).unwrap();
        super::open_chrome_profile(&chrome, "https://example.com/synthetic").unwrap();
        assert_eq!(*TEST_OPENED_CHROME_PROFILES.lock().unwrap(), ["Profile 3"]);
        assert_eq!(store.list_access_profiles().await.unwrap().len(), 2);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn inline_browser_edits_preserve_shared_profiles_and_other_targets() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "operator@example.com", false);
        write_chrome_profiles(
            temp.path(),
            &[("Profile 3", "Work", ""), ("Profile 8", "Personal", "")],
        );
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account = account(None, "operator@example.com");
        store.upsert_provider_account(&account).await.unwrap();
        super::set_browser_profiles("linear", None, &["Work".into()])
            .await
            .unwrap();
        let original = store.list_access_profiles().await.unwrap();
        super::set_browser_profiles(
            "codex",
            Some("operator@"),
            &["Work".into(), "Personal".into()],
        )
        .await
        .unwrap();
        assert_eq!(
            store
                .get_access_profile(&original[0].id)
                .await
                .unwrap()
                .unwrap(),
            original[0]
        );
        let managed = store
            .list_auth_browser_profiles(Some(Provider::Codex), Some(&account.account_id))
            .await
            .unwrap();
        assert_eq!(managed.len(), 2);
        super::set_browser_profiles("linear", None, &[])
            .await
            .unwrap();
        assert!(store
            .list_auth_browser_profiles(Some(Provider::Linear), None)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .list_auth_browser_profiles(Some(Provider::Codex), Some(&account.account_id))
                .await
                .unwrap(),
            managed
        );
        assert_eq!(store.list_access_profiles().await.unwrap().len(), 2);
        assert!(
            super::set_browser_profiles("linear", None, &["nonexistent".into()])
                .await
                .is_err()
        );
        assert_eq!(
            store
                .list_auth_browser_profiles(Some(Provider::Codex), Some(&account.account_id))
                .await
                .unwrap(),
            managed
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn provider_launch_failure_does_not_retry_other_profiles() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "operator@example.com", true);
        write_chrome_profiles(
            temp.path(),
            &[
                ("Profile 3", "First", "operator@example.com"),
                ("Profile 8", "Second", "operator@example.com"),
            ],
        );
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account = account(None, "operator@example.com");
        store.upsert_provider_account(&account).await.unwrap();
        let first = access_profile("Profile 3", "operator@example.com", 1);
        let second = access_profile("Profile 8", "operator@example.com", 2);
        store.upsert_access_profile(&first).await.unwrap();
        store.upsert_access_profile(&second).await.unwrap();
        store
            .set_auth_browser_profiles(
                Provider::Codex,
                Some(&account.account_id),
                &[first.id.clone(), second.id.clone()],
            )
            .await
            .unwrap();

        let error = connect_account("codex", "operator@", None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("disconnected"));
        assert!(TEST_OPENED_CHROME_PROFILES.lock().unwrap().is_empty());
        assert_eq!(
            fs::read_to_string(temp.path().join("codex-count")).unwrap(),
            "1"
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn connect_skips_drifted_venue_and_names_both_logins() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "operator@example.com", false);
        write_chrome_profiles(
            temp.path(),
            &[
                ("Profile 3", "Drifted", "someone.else@example.com"),
                ("Profile 8", "Operator", "operator@example.com"),
            ],
        );
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account = account(None, "operator@example.com");
        store.upsert_provider_account(&account).await.unwrap();
        let first = access_profile("Profile 3", "operator@example.com", 1);
        let second = access_profile("Profile 8", "operator@example.com", 2);
        store.upsert_access_profile(&first).await.unwrap();
        store.upsert_access_profile(&second).await.unwrap();
        store
            .set_auth_browser_profiles(
                Provider::Codex,
                Some(&account.account_id),
                &[first.id.clone(), second.id.clone()],
            )
            .await
            .unwrap();

        connect_account("codex", "operator@", None).await.unwrap();

        assert_eq!(
            *TEST_OPENED_CHROME_PROFILES.lock().unwrap(),
            ["Profile 8".to_string()]
        );
        assert_eq!(
            *TEST_ACCESS_PROFILE_FAILURES.lock().unwrap(),
            ["Profile 3: signed in as 'someone.else@example.com', expected 'operator@example.com'"
                .to_string()]
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("codex-count")).unwrap(),
            "1"
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn connect_identity_mismatch_preserves_account_and_removes_staged_home() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "other@example.com", false);
        write_chrome_profiles(
            temp.path(),
            &[("Profile 3", "Primary", "operator@example.com")],
        );
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account_id = parse_account_id("primary").unwrap();
        let account_home = account_home_path(Provider::Codex, &account_id).unwrap();
        fs::create_dir_all(&account_home).unwrap();
        fs::write(account_home.join("auth.json"), b"durable-credential").unwrap();
        let account = account(Some(&account_home), "operator@example.com");
        store.upsert_provider_account(&account).await.unwrap();
        let profile = access_profile("Profile 3", "operator@example.com", 1);
        store.upsert_access_profile(&profile).await.unwrap();
        store
            .set_auth_browser_profiles(Provider::Codex, Some(&account.account_id), &[profile.id])
            .await
            .unwrap();

        let error = connect_account("codex", "operator@", None)
            .await
            .unwrap_err();

        assert!(error.to_string().contains(
            "Profile 3: Codex reports other@example.com; expected operator@example.com. Refused: the login was discarded and 'operator@example.com' is unchanged."
        ));
        assert_eq!(
            store
                .get_provider_account("codex", &account.account_id)
                .await
                .unwrap(),
            Some(account)
        );
        assert_eq!(
            fs::read(account_home.join("auth.json")).unwrap(),
            b"durable-credential"
        );
        let staged_homes = fs::read_to_string(temp.path().join("codex-homes")).unwrap();
        let staged_homes = staged_homes.lines().collect::<Vec<_>>();
        assert_eq!(staged_homes.len(), 1);
        assert!(!Path::new(staged_homes[0]).exists());
        assert_eq!(
            *TEST_OPENED_CHROME_PROFILES.lock().unwrap(),
            ["Profile 3".to_string()]
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn reconnect_preserves_live_credential_while_browser_login_is_pending() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(CONNECT_ENV);
        configure_connect_test(temp.path(), "operator@example.com", false);
        write_chrome_profiles(
            temp.path(),
            &[("Profile 3", "Primary", "operator@example.com")],
        );
        let home =
            account_home_path(Provider::Codex, &parse_account_id("primary").unwrap()).unwrap();
        fs::create_dir_all(&home).unwrap();
        let original = fs::read(temp.path().join("codex-auth.json")).unwrap();
        fs::write(home.join("auth.json"), &original).unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            temp.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        let account = account(Some(&home), "operator@example.com");
        store.upsert_provider_account(&account).await.unwrap();
        let release = temp.path().join("release-login");
        std::env::set_var("LF_TEST_CODEX_RELEASE", &release);
        let connect = connect_account("codex", "operator@", Some("Primary"));
        tokio::pin!(connect);
        // Keep driving the real async login while checking the live home from outside it.
        tokio::select! {
            result = &mut connect => panic!("login completed before browser approval: {result:?}"),
            _ = async {
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    while !temp.path().join("codex-homes").exists() { tokio::time::sleep(std::time::Duration::from_millis(10)).await; }
                }).await.unwrap();
            } => {}
        }
        assert_eq!(fs::read(home.join("auth.json")).unwrap(), original);
        assert_eq!(
            store
                .get_provider_account("codex", &account.account_id)
                .await
                .unwrap(),
            Some(account.clone())
        );
        fs::write(&release, "approved").unwrap();
        connect.await.unwrap();
        let installed = store
            .get_provider_account("codex", &account.account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            installed.observed_email.as_deref(),
            Some("operator@example.com")
        );
        assert_eq!(
            installed.observed_subject.as_deref(),
            Some("user-operator@example.com")
        );
        assert_eq!(fs::read(home.join("auth.json")).unwrap(), original);

        // Even staged credential bytes cannot substitute for this attempt's success.
        let provider = temp.path().join("bin/codex");
        let script = fs::read_to_string(&provider).unwrap();
        fs::write(
            &provider,
            script.replace("\"success\":true", "\"success\":false"),
        )
        .unwrap();
        let error = connect_account("codex", "operator@", Some("Primary"))
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("Codex login failed"));
        assert_eq!(fs::read(home.join("auth.json")).unwrap(), original);
        assert_eq!(
            store
                .get_provider_account("codex", &account.account_id)
                .await
                .unwrap(),
            Some(installed)
        );
    }

    #[test]
    fn exhausted_venues_name_every_attempt_and_both_repairs() {
        let account = ProviderAccount {
            provider: "claude".to_string(),
            account_id: parse_account_id("primary").unwrap(),
            home: None,
            login_email: Some(EmailAddress::parse("jackstah@gmail.com").unwrap()),
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: CredentialState::Missing,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        };
        let error = exhausted_access_profiles_error(
            Provider::Claude,
            &account,
            &[
                "Profile 3: signed in as someone else".to_string(),
                "Profile 8: no signed-in account".to_string(),
            ],
        );

        assert_eq!(
            error.to_string(),
            "Chrome profiles unavailable for claude/jackstah@gmail.com. Profile 3: signed in as someone else; Profile 8: no signed-in account Choose a profile: lf account connect claude jackstah@gmail.com --chrome-profile <profile>"
        );
    }
}
