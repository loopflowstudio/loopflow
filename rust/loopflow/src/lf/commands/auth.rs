use std::fs;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
#[cfg(all(target_os = "macos", not(test)))]
use std::process::Stdio;
use std::sync::Arc;
#[cfg(test)]
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::engine::platform::open_url;
use crate::lf::{AuthAccessCommand, AuthCommand};
use crate::profile::{AccessProfile, EmailAddress, LocalChromeProfile, ProfileId};
use crate::provider_account::{
    account_home_path, account_id_for_login, account_login, ensure_account_home, new_account,
    open_account_store, remove_account_home,
};
use crate::provider_auth::{
    capture_claude_authorization_code_from_chrome, capture_claude_profile_credentials,
    disconnect_provider_account_auth, import_ambient_claude_profile_credentials, no_event_sink,
    prepare_provider_account_access_token, provider_account_auth_status,
    start_provider_account_auth, AuthStatus, Provider, ProviderAuthService, ProviderAuthSnapshot,
};
use crate::store::{
    open_store, CredentialState, CredentialType, ProviderAccount, ProviderAccountId, ProviderToken,
    RoutingState, SharedStore, StoreError,
};

const AUTH_STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(180);
const AUTH_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
// Authorization-code flows wait on a human finishing a browser login; give
// them the ~10 minutes the OAuth authorization itself stays valid.
const AUTH_CODE_FLOW_TIMEOUT_SECS: u64 = 600;
#[cfg(test)]
static TEST_OPENED_CHROME_PROFILES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
#[cfg(test)]
static TEST_ACCESS_PROFILE_FAILURES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
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
    let result = rt.block_on(run_async(cmd));
    // Don't wait for lingering blocking tasks: when the browser handoff
    // completes a login, the backup stdin prompt is still mid-read and would
    // otherwise hold the process open until the user presses Enter.
    rt.shutdown_background();
    result
}

async fn run_async(cmd: &AuthCommand) -> Result<()> {
    if crate::provider_account::lease::account_lease_active()
        && matches!(cmd, AuthCommand::Accounts { verify: false, .. })
    {
        let AuthCommand::Accounts { provider, .. } = cmd else {
            unreachable!("the guarded command is accounts")
        };
        accounts(provider.as_deref(), false).await?;
        return forwarded_accounts(provider.as_deref());
    }
    match cmd {
        AuthCommand::Status { provider } => status(provider.as_deref()).await,
        AuthCommand::Disconnect { provider, email } => match email {
            Some(email) => disconnect_account(provider, email).await,
            None => disconnect(provider).await,
        },
        AuthCommand::Configure { provider } => configure(provider).await,
        AuthCommand::Connect {
            provider,
            email,
            chrome_profile,
        } => match email.as_deref() {
            Some(email) => connect_account(provider, email, chrome_profile.as_deref()).await,
            None => connect(provider).await,
        },
        AuthCommand::Import {
            provider,
            email,
            chrome_profile,
        } => import_account(provider, email, chrome_profile.as_deref()).await,
        AuthCommand::Access { cmd } => access(cmd).await,
        AuthCommand::Accounts { provider, verify } => accounts(provider.as_deref(), *verify).await,
        AuthCommand::Set {
            provider,
            email,
            login_email,
            routing,
            plan,
            clear_plan,
            paid_through,
            clear_paid_through,
        } => {
            set_account_lifecycle(
                provider,
                email,
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
        AuthCommand::Reset { provider, email } => reset_account(provider, email).await,
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

fn forwarded_accounts(raw_provider: Option<&str>) -> Result<()> {
    let provider = raw_provider.map(parse_managed_provider).transpose()?;
    let client = crate::provider_account::lease::AccountLeaseClient::from_env()?
        .ok_or_else(|| anyhow!("forwarded account lease is unavailable"))?;
    let lease = client.describe()?;
    for grant in lease
        .grants
        .iter()
        .filter(|grant| provider.is_none_or(|provider| provider == grant.provider))
    {
        for (position, account_id) in grant.accounts.iter().enumerate() {
            let mut labels = vec!["forwarded"];
            if position < grant.preferred {
                labels.push("preferred");
            }
            let account = client.login_email(grant.provider, account_id)?;
            println!("{:<12} {} {}", grant.provider, account, labels.join(" · "));
        }
    }
    Ok(())
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

    if service.pending_requires_authorization_code(provider).await {
        // The browser normally hands the code back to the provider CLI's
        // localhost listener on approval; the pasted code is the backup for
        // when that handoff can't reach this machine.
        println!("Approve authorization in the browser; login completes automatically.");
        println!("If the browser shows a one-time code instead, paste it here.");
        let timeout = flow.expires_in.or(Some(AUTH_CODE_FLOW_TIMEOUT_SECS));
        tokio::select! {
            result = wait_for_active_status(&service, provider, timeout) => result,
            code = read_authorization_code_line() => {
                if let Some(code) = code? {
                    service.complete_auth(provider, &code).await?;
                }
                wait_for_active_status(&service, provider, timeout).await
            }
        }
    } else {
        println!("Complete authorization in the browser.");
        wait_for_active_status(&service, provider, flow.expires_in).await
    }
}

/// Read a pasted authorization code from stdin; None when stdin closes
/// without one (headless runs fall back to the browser handoff alone).
async fn read_authorization_code_line() -> Result<Option<String>> {
    tokio::task::spawn_blocking(|| {
        let stdin = std::io::stdin();
        loop {
            let mut line = String::new();
            let read = stdin
                .read_line(&mut line)
                .context("read authorization code")?;
            if read == 0 {
                return Ok(None);
            }
            let code = line.trim();
            if !code.is_empty() {
                return Ok(Some(code.to_string()));
            }
        }
    })
    .await
    .context("authorization code prompt task failed")?
}

async fn connect_account(
    raw_provider: &str,
    raw_email: &str,
    raw_chrome_profile: Option<&str>,
) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let store = open_account_store().await?;
    let account = super::profile::find_provider_account(&store, provider, raw_email).await?;
    let candidates = if let Some(raw_chrome_profile) = raw_chrome_profile {
        vec![bootstrap_access_profile(&store, raw_chrome_profile).await?]
    } else {
        let mappings = store
            .list_account_access_profiles(Some(provider), Some(&account.account_id))
            .await?;
        let mut profiles = Vec::new();
        for mapping in mappings {
            let profile = store
                .get_access_profile(&mapping.profile_id)
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "{provider}/{} references missing access profile '{}'",
                        account.account_id,
                        mapping.profile_id
                    )
                })?;
            profiles.push(profile);
        }
        profiles
    };
    if candidates.is_empty() {
        return Err(anyhow!(
            "No access profile can log in {provider}/{}. Add a venue: lf auth access add {provider} {} --profile <profile>",
            account_login(&account),
            account_login(&account)
        ));
    }

    let mut failures = Vec::new();
    let mut selected = None;
    for profile in candidates {
        let attempt = match verified_chrome_profile(&profile) {
            Ok(chrome_profile) => {
                connect_managed_account(&store, provider, &account, &profile, chrome_profile).await
            }
            Err(error) => Err(error),
        };
        match attempt {
            Ok(()) => {
                selected = Some(profile);
                break;
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
    let selected =
        selected.ok_or_else(|| exhausted_access_profiles_error(provider, &account, &failures))?;
    if raw_chrome_profile.is_some() {
        store.upsert_access_profile(&selected).await?;
        let mut profile_ids = store
            .list_account_access_profiles(Some(provider), Some(&account.account_id))
            .await?
            .into_iter()
            .map(|mapping| mapping.profile_id)
            .collect::<Vec<_>>();
        if !profile_ids.contains(&selected.id) {
            profile_ids.push(selected.id);
        }
        store
            .set_account_access_profiles(provider, &account.account_id, &profile_ids)
            .await?;
    }
    Ok(())
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
    let auth_home = login_home.path().to_path_buf();
    let handle =
        start_provider_account_auth(provider, auth_home.clone(), chrome_profile.login.as_deref())
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
    if handle.requires_authorization_code() {
        open_chrome_profile(&chrome_profile, &verification_url)?;
        println!("Approve authorization in the browser.");
        let code = match capture_claude_authorization_code_from_chrome(
            &verification_url,
            chrome_profile.name.as_str(),
        )
        .await?
        {
            Some(code) => code,
            None => capture_claude_authorization_code_visibly().await?,
        };
        handle
            .submit_authorization_code(code.expose_secret())
            .await?;
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
                account_login(account)
            )
        })??;

    match provider {
        Provider::Claude => {
            capture_claude_profile_credentials(&auth_home)?;
            require_managed_access_token(provider, account_id, &auth_home).await?;
        }
        Provider::Codex => {}
        _ => return Err(anyhow!("unsupported managed provider '{provider}'")),
    }
    let login = match provider_account_auth_status(provider, auth_home.clone()).await? {
        AuthStatus::Active { login } => login,
        other => {
            return Err(anyhow!(
                "{} account '{}' finished login with status {}",
                provider.display_name(),
                account_id,
                other.as_str()
            ))
        }
    };
    verify_provider_login(
        provider,
        account_id,
        account.login_email.as_ref(),
        login.as_deref(),
    )?;
    let login = login.expect("provider login verification requires an email");
    let account_home = ensure_account_home(provider, account_id)?;
    match provider {
        Provider::Claude => install_claude_login(login_home.path(), &account_home)?,
        Provider::Codex => install_codex_login(login_home.path(), &account_home)?,
        _ => return Err(anyhow!("unsupported managed provider '{provider}'")),
    }
    register_managed_account(store, provider, account_id, account_home, Some(login)).await?;
    println!(
        "Connected {} login '{}' through profile '{}'",
        provider.display_name(),
        account_login(account),
        profile.id
    );
    Ok(())
}

async fn bootstrap_access_profile(
    store: &SharedStore,
    raw_chrome_profile: &str,
) -> Result<AccessProfile> {
    let chrome_profile = crate::profile::resolve_local_chrome_profile(raw_chrome_profile)
        .map_err(anyhow::Error::msg)?;
    if let Some(profile) = store
        .list_access_profiles()
        .await?
        .into_iter()
        .find(|profile| profile.chrome_directory == chrome_profile.directory)
    {
        return Ok(profile);
    }
    let login = chrome_profile.login.as_deref().ok_or_else(|| {
        anyhow!(
            "Chrome profile '{}' has no signed-in account",
            chrome_profile.name
        )
    })?;
    Ok(AccessProfile {
        id: ProfileId::parse(&chrome_profile.directory).map_err(anyhow::Error::msg)?,
        chrome_directory: chrome_profile.directory,
        expected_login: EmailAddress::parse(login).map_err(anyhow::Error::msg)?,
        created_at: OffsetDateTime::now_utc().unix_timestamp(),
        updated_at: OffsetDateTime::now_utc().unix_timestamp(),
    })
}

fn verified_chrome_profile(profile: &AccessProfile) -> Result<LocalChromeProfile> {
    let chrome_profile = crate::profile::resolve_local_chrome_profile(&profile.chrome_directory)
        .map_err(anyhow::Error::msg)?;
    verify_chrome_profile_login(profile, chrome_profile)
}

fn verify_chrome_profile_login(
    profile: &AccessProfile,
    chrome_profile: LocalChromeProfile,
) -> Result<LocalChromeProfile> {
    let actual = chrome_profile.login.as_deref().ok_or_else(|| {
        anyhow!(
            "Chrome profile '{}' has no signed-in account",
            chrome_profile.name
        )
    })?;
    if !actual.eq_ignore_ascii_case(profile.expected_login.as_str()) {
        return Err(anyhow!(
            "signed in as '{}', expected '{}'",
            actual,
            profile.expected_login
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
        "No access profile could log in {provider}/{}. {} Sign a venue in as {}, or add a venue: lf auth access add {provider} {} --profile <profile>",
        account_login(account),
        failures.join("; "),
        account
            .login_email
            .as_ref()
            .map(EmailAddress::as_str)
            .unwrap_or("the account login"),
        account_login(account)
    )
}

fn verify_provider_login(
    provider: Provider,
    account_id: &ProviderAccountId,
    expected_login: Option<&EmailAddress>,
    reported_login: Option<&str>,
) -> Result<()> {
    let account = expected_login
        .map(EmailAddress::as_str)
        .unwrap_or_else(|| account_id.as_str());
    let reported_login = reported_login.ok_or_else(|| {
        anyhow!(
            "{} did not report a login email; account '{}' is unchanged.",
            provider.display_name(),
            account
        )
    })?;
    let Some(expected_login) = expected_login else {
        return Ok(());
    };
    if reported_login.eq_ignore_ascii_case(expected_login.as_str()) {
        return Ok(());
    }
    Err(anyhow!(
        "{} reports {}; expected {}. Refused: the login was discarded and '{}' is unchanged.",
        provider.display_name(),
        reported_login,
        expected_login,
        account
    ))
}

fn acquire_managed_login_lock(
    account_home: &Path,
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<fs::File> {
    let parent = account_home
        .parent()
        .ok_or_else(|| anyhow!("account home has no parent directory"))?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(parent.join(format!(".{}.login.lock", account_id.as_str())))
        .context("open managed login lock")?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            anyhow!(
                "another {} login is already in progress for account '{}'",
                provider.display_name(),
                account_id
            )
        } else {
            anyhow!(
                "could not lock {} account '{}' for login: {error}",
                provider.display_name(),
                account_id
            )
        }
    })?;
    Ok(lock)
}

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

async fn register_managed_account(
    store: &SharedStore,
    provider: Provider,
    account_id: &ProviderAccountId,
    account_home: PathBuf,
    login: Option<String>,
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
    account.cooldown_until = None;
    account.cooldown_reason = None;
    account.updated_at = OffsetDateTime::now_utc().unix_timestamp();
    store.upsert_provider_account(&account).await?;
    Ok(())
}

async fn import_account(
    raw_provider: &str,
    raw_email: &str,
    raw_chrome_profile: Option<&str>,
) -> Result<()> {
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
    let access_profile = match raw_chrome_profile {
        Some(profile) => Some(bootstrap_access_profile(&store, profile).await?),
        None => None,
    };
    if let Some(profile) = &access_profile {
        verified_chrome_profile(profile)?;
    }
    let account_home = account_home_path(provider, &account_id)?;

    let credentials_file = match provider {
        Provider::Claude => ".credentials.json",
        Provider::Codex => "auth.json",
        _ => unreachable!("parse_managed_provider admits Claude and Codex only"),
    };
    let (login, staged_home) = if account_home.join(credentials_file).is_file() {
        let login = match provider_account_auth_status(provider, account_home.clone()).await? {
            AuthStatus::Active { login: Some(login) } => login,
            AuthStatus::Active { login: None } => {
                return Err(anyhow!(
                    "{} did not report a login email; '{}' is unchanged",
                    provider.display_name(),
                    account_id
                ))
            }
            other => {
                return Err(anyhow!(
                    "{} account '{}' has status {}",
                    provider.display_name(),
                    account_id,
                    other.as_str()
                ))
            }
        };
        (login, None)
    } else {
        if provider != Provider::Claude {
            return Err(anyhow!(
                "no stored {} login at {}; importing the ambient login is supported for Claude only",
                provider.display_name(),
                account_home.display()
            ));
        }
        let ambient = read_ambient_claude_status()?;
        if !ambient.logged_in {
            return Err(anyhow!("the ambient Claude CLI is not logged in"));
        }
        let login = ambient.email.ok_or_else(|| {
            anyhow!(
                "Claude did not report a login email; '{}' is unchanged",
                account_id
            )
        })?;
        let parent = account_home
            .parent()
            .ok_or_else(|| anyhow!("account home has no parent directory"))?;
        fs::create_dir_all(parent).context("create provider accounts directory")?;
        let staged_home = tempfile::Builder::new()
            .prefix(".import-")
            .tempdir_in(parent)
            .context("create private provider import home")?;
        import_ambient_claude_profile_credentials(staged_home.path())?;
        require_managed_access_token(provider, &account_id, staged_home.path()).await?;
        (login, Some(staged_home))
    };

    let credential_home = staged_home
        .as_ref()
        .map(|home| home.path())
        .unwrap_or(account_home.as_path());
    require_managed_access_token(provider, &account_id, credential_home).await?;
    verify_provider_login(provider, &account_id, Some(&expected_login), Some(&login))?;
    let account_home = ensure_account_home(provider, &account_id)?;
    if let Some(staged_home) = staged_home {
        install_claude_login(staged_home.path(), &account_home)?;
    }
    register_managed_account(&store, provider, &account_id, account_home, Some(login)).await?;
    if let Some(profile) = access_profile {
        store.upsert_access_profile(&profile).await?;
        let mut profile_ids = store
            .list_account_access_profiles(Some(provider), Some(&account_id))
            .await?
            .into_iter()
            .map(|mapping| mapping.profile_id)
            .collect::<Vec<_>>();
        if !profile_ids.contains(&profile.id) {
            profile_ids.push(profile.id);
        }
        store
            .set_account_access_profiles(provider, &account_id, &profile_ids)
            .await?;
    }
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

#[cfg(target_os = "macos")]
fn visible_claude_prompt_script(socket: &Path) -> String {
    let socket = socket.to_string_lossy().replace('\'', "'\\''");
    format!(
        "#!/bin/zsh\n\
         echo 'Claude authorization needs a visible handoff.'\n\
         echo 'Approve the Chrome page, then paste its one-time code here.'\n\
         read -r -s 'code?One-time code: '\n\
         print\n\
         if [[ -z \"$code\" ]]; then\n\
           echo 'No code entered; close this window and rerun lf auth connect.'\n\
           exit 1\n\
         fi\n\
         printf '%s' \"$code\" | /usr/bin/nc -U '{socket}'\n\
         status=$?\n\
         unset code\n\
         if [[ $status -eq 0 ]]; then\n\
           echo 'Authorization returned to Loopflow. This window can close.'\n\
         else\n\
           echo 'Loopflow stopped waiting; rerun lf auth connect.'\n\
         fi\n\
         exit $status\n"
    )
}

#[cfg(all(target_os = "macos", not(test)))]
async fn capture_claude_authorization_code_visibly() -> Result<SecretString> {
    use std::io::{ErrorKind, Read};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    let handoff = tempfile::tempdir().context("create private Claude handoff")?;
    let socket = handoff.path().join("authorization.sock");
    let listener = UnixListener::bind(&socket).context("open private Claude handoff")?;
    listener
        .set_nonblocking(true)
        .context("make Claude handoff nonblocking")?;
    let script = handoff.path().join("Claude Authorization.command");
    fs::write(&script, visible_claude_prompt_script(&socket))
        .context("write visible Claude handoff")?;
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700))
        .context("protect visible Claude handoff")?;

    let status = Command::new("open")
        .args(["-a", "Terminal"])
        .arg(&script)
        .status()
        .context("open visible Claude authorization handoff")?;
    if !status.success() {
        return Err(anyhow!(
            "could not open the visible Claude authorization handoff; rerun `lf auth connect claude <account>` in Terminal"
        ));
    }

    let deadline = std::time::Instant::now() + AUTH_STATUS_POLL_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .context("bound Claude handoff read")?;
                let mut code = String::new();
                stream
                    .take(4097)
                    .read_to_string(&mut code)
                    .context("read visible Claude handoff")?;
                let code = code.trim();
                if code.is_empty() || code.len() > 4096 {
                    return Err(anyhow!("visible Claude handoff returned an invalid code"));
                }
                return Ok(SecretString::new(code.to_string()));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Err(anyhow!(
                        "timed out waiting for the visible Claude authorization handoff"
                    ));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(error) => return Err(error).context("accept visible Claude handoff"),
        }
    }
}

#[cfg(all(not(target_os = "macos"), not(test)))]
async fn capture_claude_authorization_code_visibly() -> Result<SecretString> {
    Ok(SecretString::new(rpassword::prompt_password(
        "Browser handoff unavailable; paste the one-time code: ",
    )?))
}

#[cfg(test)]
async fn capture_claude_authorization_code_visibly() -> Result<SecretString> {
    Ok(SecretString::new("test-code".to_string()))
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
    service.disconnect(provider, no_event_sink()).await?;
    let snapshot = service.status(provider).await?;
    if matches!(snapshot.status, AuthStatus::None) {
        println!("Disconnected {}", provider.display_name());
    } else {
        println!("Updated {}", format_snapshot(&snapshot));
    }
    Ok(())
}

async fn disconnect_account(raw_provider: &str, raw_email: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let store = open_account_store().await?;
    let mut account = super::profile::find_provider_account(&store, provider, raw_email).await?;
    let account_id = account.account_id.clone();
    if let Some(home) = account.home.as_deref() {
        let expected_home = account_home_path(provider, &account_id)?;
        if home != expected_home {
            return Err(anyhow!(
                "refusing to remove unexpected {} account home {}",
                provider.display_name(),
                home.display()
            ));
        }
        disconnect_provider_account_auth(provider, expected_home.clone()).await?;
        remove_account_home(&expected_home)?;
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

async fn access(cmd: &AuthAccessCommand) -> Result<()> {
    let store = open_account_store().await?;
    match cmd {
        AuthAccessCommand::Set {
            provider,
            email,
            profiles,
        } => {
            let provider = parse_managed_provider(provider)?;
            let account = super::profile::find_provider_account(&store, provider, email).await?;
            let profile_ids = parse_existing_profiles(&store, profiles).await?;
            store
                .set_account_access_profiles(provider, &account.account_id, &profile_ids)
                .await?;
            println!(
                "{} access profiles: {}",
                account_login(&account),
                profile_ids
                    .iter()
                    .map(ProfileId::as_str)
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
            Ok(())
        }
        AuthAccessCommand::Add {
            provider,
            email,
            profile,
        } => {
            let provider = parse_managed_provider(provider)?;
            let account = super::profile::find_provider_account(&store, provider, email).await?;
            let profile_id = parse_existing_profile(&store, profile).await?;
            let mut profile_ids = store
                .list_account_access_profiles(Some(provider), Some(&account.account_id))
                .await?
                .into_iter()
                .map(|mapping| mapping.profile_id)
                .collect::<Vec<_>>();
            if !profile_ids.contains(&profile_id) {
                profile_ids.push(profile_id);
            }
            store
                .set_account_access_profiles(provider, &account.account_id, &profile_ids)
                .await?;
            println!(
                "Updated {provider}/{} access profiles",
                account_login(&account)
            );
            Ok(())
        }
        AuthAccessCommand::Rm {
            provider,
            email,
            profile,
        } => {
            let provider = parse_managed_provider(provider)?;
            let account = super::profile::find_provider_account(&store, provider, email).await?;
            let profile_id = ProfileId::parse(profile).map_err(anyhow::Error::msg)?;
            let profile_ids = store
                .list_account_access_profiles(Some(provider), Some(&account.account_id))
                .await?
                .into_iter()
                .map(|mapping| mapping.profile_id)
                .filter(|candidate| candidate != &profile_id)
                .collect::<Vec<_>>();
            store
                .set_account_access_profiles(provider, &account.account_id, &profile_ids)
                .await?;
            println!(
                "Updated {provider}/{} access profiles",
                account_login(&account)
            );
            Ok(())
        }
    }
}

async fn parse_existing_profiles(
    store: &SharedStore,
    raw_profiles: &[String],
) -> Result<Vec<ProfileId>> {
    let mut profiles = Vec::new();
    for profile in raw_profiles {
        profiles.push(parse_existing_profile(store, profile).await?);
    }
    Ok(profiles)
}

async fn parse_existing_profile(store: &SharedStore, raw_profile: &str) -> Result<ProfileId> {
    let profile_id = ProfileId::parse(raw_profile).map_err(anyhow::Error::msg)?;
    if store.get_access_profile(&profile_id).await?.is_none() {
        return Err(anyhow!(
            "access profile '{}' does not exist; run `lf profile create --chrome-profile <profile> --as {}` first",
            profile_id,
            profile_id
        ));
    }
    Ok(profile_id)
}

async fn accounts(raw_provider: Option<&str>, verify: bool) -> Result<()> {
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
        if !crate::provider_account::lease::account_lease_active() {
            println!("No managed OAuth accounts");
        }
        return Ok(());
    }
    for mut account in accounts {
        let provenance = if crate::provider_account::lease::account_lease_active() {
            "  local"
        } else {
            ""
        };
        println!("{}{provenance}", format_account(&account));
        let cached = account.credential_state.as_str();
        if verify {
            match crate::subscription::poll_account(&account).await {
                Ok(_) => {
                    println!("  auth: cached {cached} · live active");
                    if account.credential_state != CredentialState::Connected {
                        account.credential_state = CredentialState::Connected;
                        account.cooldown_until = None;
                        account.cooldown_reason = None;
                        store.upsert_provider_account(&account).await?;
                    }
                }
                Err(crate::subscription::SubscriptionError::NeedsLogin(reason)) => {
                    println!("  auth: cached {cached} · live needs re-login ({reason})");
                    store
                        .record_provider_account_credential_invalidated(
                            &account.provider,
                            &account.account_id,
                            "token_invalidated",
                        )
                        .await?;
                    println!(
                        "  recover: lf auth connect {} {}",
                        account.provider,
                        account_login(&account)
                    );
                }
                Err(crate::subscription::SubscriptionError::Unavailable(reason)) => {
                    println!("  auth: cached {cached} · live unavailable ({reason})");
                }
            }
        } else {
            println!("  auth: cached {cached} · live not checked (use --verify)");
        }
        let provider = account
            .provider
            .parse::<Provider>()
            .map_err(|error| anyhow!(error.to_string()))?;
        let profiles = store
            .list_account_access_profiles(Some(provider), Some(&account.account_id))
            .await?;
        if profiles.is_empty() {
            println!("  access: none");
        } else {
            println!(
                "  access: {}",
                profiles
                    .iter()
                    .map(|profile| profile.profile_id.as_str())
                    .collect::<Vec<_>>()
                    .join(" -> ")
            );
        }
    }
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
            "lf auth set needs --login-email, --routing, --plan, or --paid-through"
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

async fn reset_account(raw_provider: &str, raw_email: &str) -> Result<()> {
    let provider = parse_managed_provider(raw_provider)?;
    let store = open_account_store().await?;
    let account = super::profile::find_provider_account(&store, provider, raw_email).await?;
    store
        .reset_provider_account_health(provider.as_str(), &account.account_id)
        .await
        .map_err(|error| account_store_error(provider, account_login(&account), error))?;
    println!(
        "Reset {} login '{}' usage state",
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
        details.push("ready".to_string());
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
    use std::fs;

    use time::OffsetDateTime;

    use crate::profile::EmailAddress;
    use crate::provider_auth::{AuthStatus, Provider, ProviderAuthSnapshot};
    use crate::store::{
        CredentialState, CredentialType, ProviderAccount, ProviderAccountId, RoutingState,
    };

    use super::{
        acquire_managed_login_lock, format_account, format_relative_delta, format_snapshot,
        import_account, install_claude_login, install_codex_login, parse_paid_through,
        parse_routing_state,
    };

    #[cfg(target_os = "macos")]
    #[test]
    fn visible_claude_handoff_neither_echoes_nor_embeds_the_code() {
        let script = super::visible_claude_prompt_script(std::path::Path::new("/tmp/auth.sock"));
        assert!(script.contains("read -r -s"));
        assert!(script.contains("/usr/bin/nc -U '/tmp/auth.sock'"));
        assert!(!script.contains("verification_uri"));
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
        assert!(!rendered.contains("primary"));
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
        let previous_db_path = std::env::var_os("LF_DB_PATH");
        let previous_control_home = std::env::var_os(crate::store::CONTROL_HOME_ENV);
        let previous_control_db_path = std::env::var_os(crate::store::CONTROL_DB_PATH_ENV);
        std::env::set_var("LF_HOME", home.path());
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(crate::store::CONTROL_HOME_ENV);
        std::env::remove_var(crate::store::CONTROL_DB_PATH_ENV);
        let result = import_account("codex", "engineering@example.com", None).await;
        match previous {
            Some(value) => std::env::set_var("LF_HOME", value),
            None => std::env::remove_var("LF_HOME"),
        }
        match previous_db_path {
            Some(value) => std::env::set_var("LF_DB_PATH", value),
            None => std::env::remove_var("LF_DB_PATH"),
        }
        match previous_control_home {
            Some(value) => std::env::set_var(crate::store::CONTROL_HOME_ENV, value),
            None => std::env::remove_var(crate::store::CONTROL_HOME_ENV),
        }
        match previous_control_db_path {
            Some(value) => std::env::set_var(crate::store::CONTROL_DB_PATH_ENV, value),
            None => std::env::remove_var(crate::store::CONTROL_DB_PATH_ENV),
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

    use super::{
        connect_account, exhausted_access_profiles_error, verify_provider_login,
        TEST_ACCESS_PROFILE_FAILURES, TEST_OPENED_CHROME_PROFILES,
    };
    use crate::profile::{AccessProfile, EmailAddress, ProfileId};
    use crate::provider_account::lease::ACCOUNT_LEASE_ENV;
    use crate::provider_account::{account_home_path, parse_account_id};
    use crate::provider_auth::Provider;
    use crate::store::{
        open_store, CredentialState, ProviderAccount, RoutingState, StorageConfig,
        CONTROL_DB_PATH_ENV, CONTROL_HOME_ENV,
    };
    use tempfile::tempdir;

    const CONNECT_ENV: &[&str] = &[
        "HOME",
        "LF_HOME",
        "LF_DB_PATH",
        CONTROL_HOME_ENV,
        CONTROL_DB_PATH_ENV,
        "PATH",
        ACCOUNT_LEASE_ENV,
        "LF_TEST_CODEX_AUTH_JSON",
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
count=0
if [ -f "$LF_TEST_CODEX_COUNT" ]; then count=$(cat "$LF_TEST_CODEX_COUNT"); fi
count=$((count + 1))
printf '%s' "$count" > "$LF_TEST_CODEX_COUNT"
printf '%s\n' "$CODEX_HOME" >> "$LF_TEST_CODEX_HOMES"
if [ "$LF_TEST_CODEX_FAIL_FIRST" = "1" ] && [ "$count" = "1" ]; then exit 1; fi
printf '%s\n' 'https://auth.openai.com/oauth/authorize?client_id=test'
mkdir -p "$CODEX_HOME"
cp "$LF_TEST_CODEX_AUTH_JSON" "$CODEX_HOME/auth.json"
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
        let claims = URL_SAFE_NO_PAD.encode(format!(r#"{{"email":"{reported_login}"}}"#));
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
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(CONTROL_HOME_ENV);
        std::env::remove_var(CONTROL_DB_PATH_ENV);
        std::env::remove_var(ACCOUNT_LEASE_ENV);
        std::env::set_var("LF_TEST_CODEX_AUTH_JSON", auth_json);
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

    fn account(account_home: Option<&Path>, login: &str) -> ProviderAccount {
        ProviderAccount {
            provider: "codex".to_string(),
            account_id: parse_account_id("primary").unwrap(),
            home: account_home.map(Path::to_path_buf),
            login_email: Some(EmailAddress::parse(login).unwrap()),
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
            expected_login: EmailAddress::parse(login).unwrap(),
            created_at: position,
            updated_at: position,
        }
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn connect_tries_access_profiles_in_configured_order() {
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
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
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
            .set_account_access_profiles(
                Provider::Codex,
                &account.account_id,
                &[first.id.clone(), second.id.clone()],
            )
            .await
            .unwrap();

        connect_account("codex", "operator@", None).await.unwrap();

        assert_eq!(
            *TEST_OPENED_CHROME_PROFILES.lock().unwrap(),
            ["Profile 8".to_string()]
        );
        let failures = TEST_ACCESS_PROFILE_FAILURES.lock().unwrap();
        assert_eq!(failures.len(), 1);
        assert!(failures[0].starts_with("Profile 3:"));
        assert_eq!(
            fs::read_to_string(temp.path().join("codex-count")).unwrap(),
            "2"
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
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
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
            .set_account_access_profiles(
                Provider::Codex,
                &account.account_id,
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
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
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
            .set_account_access_profiles(Provider::Codex, &account.account_id, &[profile.id])
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

    #[test]
    fn provider_identity_is_required() {
        let account_id = parse_account_id("primary").unwrap();

        let error = verify_provider_login(Provider::Claude, &account_id, None, None).unwrap_err();

        assert_eq!(
            error.to_string(),
            "Claude did not report a login email; account 'primary' is unchanged."
        );
    }

    #[test]
    fn exhausted_venues_name_every_attempt_and_both_repairs() {
        let account = ProviderAccount {
            provider: "claude".to_string(),
            account_id: parse_account_id("primary").unwrap(),
            home: None,
            login_email: Some(EmailAddress::parse("jackstah@gmail.com").unwrap()),
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
            "No access profile could log in claude/jackstah@gmail.com. Profile 3: signed in as someone else; Profile 8: no signed-in account Sign a venue in as jackstah@gmail.com, or add a venue: lf auth access add claude jackstah@gmail.com --profile <profile>"
        );
    }
}
