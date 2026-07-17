//! Host-local provider accounts, selection, and process-lifetime SSH
//! credential leases for Claude and Codex.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::profile::{ProviderRoute, RouteScope};
use crate::provider_auth::{provider_account_auth_status, AuthStatus, Provider};
use crate::repository::RepoId;
use crate::store::{open_store, ProviderAccount, ProviderAccountId, SharedStore, StoreError};
use crate::store::{CredentialState, RoutingState};

pub const FORWARDED_ACCOUNT_BUNDLE_ENV: &str = "LF_FORWARDED_ACCOUNT_BUNDLE";
pub const FORWARDED_ACCOUNT_STORE_ENV: &str = "LF_FORWARDED_ACCOUNT_STORE";
pub const ACCOUNT_REPO_ID_ENV: &str = "LF_ACCOUNT_REPO_ID";
/// Explicit account selection (`lf --account <email|id>`, or exported as
/// `LF_ACCOUNT=<email|id>`): every provider resolution in this process and
/// its children uses the named account, bypassing the repo route and health
/// gating — an explicit human choice.
pub const PROVIDER_ACCOUNT_ENV: &str = "LF_ACCOUNT";
const DEFAULT_COOLDOWN_SECS: i64 = 15 * 60;
const RESET_GRACE_SECS: i64 = 5;

#[derive(Debug, Error)]
pub enum ProviderAccountError {
    #[error("{0}")]
    InvalidAccountId(String),
    #[error("provider account routing supports Claude and Codex OAuth only")]
    UnsupportedProvider,
    #[error("provider account store failed: {0}")]
    Store(#[from] StoreError),
    #[error("provider account filesystem failed: {0}")]
    Filesystem(String),
    #[error("invalid forwarded provider credential bundle: {0}")]
    ForwardedBundle(String),
    #[error("cannot forward {provider} account '{account_id}': {reason}")]
    ForwardingCredential {
        provider: Provider,
        account_id: ProviderAccountId,
        reason: String,
    },
    #[error("provider account runtime failed: {0}")]
    Runtime(String),
    #[error("no authenticated {provider} account remains; reconnect {accounts}")]
    NoAuthenticatedAccount {
        provider: Provider,
        accounts: String,
    },
    #[error("configured {provider} route has no eligible account: {accounts}")]
    NoEligibleAccount {
        provider: Provider,
        accounts: String,
    },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardedProviderCredential {
    pub provider: Provider,
    pub account_id: ProviderAccountId,
    access_token: String,
}

impl std::fmt::Debug for ForwardedProviderCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ForwardedProviderCredential")
            .field("provider", &self.provider)
            .field("account_id", &self.account_id)
            .field("access_token", &"[REDACTED]")
            .finish()
    }
}

impl ForwardedProviderCredential {
    pub fn new(provider: Provider, account_id: ProviderAccountId, access_token: String) -> Self {
        Self {
            provider,
            account_id,
            access_token,
        }
    }

    fn access_token(&self) -> &str {
        &self.access_token
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardedProviderAccount {
    pub provider: Provider,
    pub account_id: ProviderAccountId,
    pub login_email: Option<crate::profile::EmailAddress>,
    pub credential_state: CredentialState,
    pub routing_state: RoutingState,
    pub plan: Option<String>,
    pub paid_through: Option<time::Date>,
    pub utilization_percent: Option<u8>,
    pub cooldown_until: Option<i64>,
    pub cooldown_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardedProviderRoute {
    pub provider: Provider,
    pub accounts: Vec<ProviderAccountId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardedAccountBundle {
    pub repo_id: RepoId,
    pub routes: Vec<ForwardedProviderRoute>,
    pub accounts: Vec<ForwardedProviderAccount>,
    credentials: Vec<ForwardedProviderCredential>,
}

impl ForwardedAccountBundle {
    pub fn new(
        repo_id: RepoId,
        routes: Vec<ForwardedProviderRoute>,
        accounts: Vec<ForwardedProviderAccount>,
        credentials: Vec<ForwardedProviderCredential>,
    ) -> Self {
        Self {
            repo_id,
            routes,
            accounts,
            credentials,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitSignal {
    pub utilization_percent: Option<u8>,
    pub resets_at: Option<i64>,
    pub limited: bool,
    pub reason: String,
    /// Per-window subscription state carried by the provider event, persisted
    /// so `lf usage` can answer "how much is left" without a fresh poll.
    pub windows: Vec<crate::store::AccountLimitWindow>,
}

#[derive(Clone)]
enum AccountCredential {
    NativeHome(PathBuf),
    AccessToken(String),
}

#[derive(Clone)]
pub struct ProviderAccountRoute {
    provider: Provider,
    account_id: ProviderAccountId,
    credential: AccountCredential,
    resume_requested_session: bool,
    store: SharedStore,
}

impl std::fmt::Debug for ProviderAccountRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let credential = match self.credential {
            AccountCredential::NativeHome(_) => "native_home",
            AccountCredential::AccessToken(_) => "access_token",
        };
        f.debug_struct("ProviderAccountRoute")
            .field("provider", &self.provider)
            .field("account_id", &self.account_id)
            .field("credential", &credential)
            .field("resume_requested_session", &self.resume_requested_session)
            .finish()
    }
}

impl ProviderAccountRoute {
    pub fn account_id(&self) -> &ProviderAccountId {
        &self.account_id
    }

    pub fn resume_requested_session(&self) -> bool {
        self.resume_requested_session
    }

    pub fn uses_native_home(&self) -> bool {
        matches!(self.credential, AccountCredential::NativeHome(_))
    }

    pub fn apply(&self, command: &mut Command) {
        command.env_remove(FORWARDED_ACCOUNT_BUNDLE_ENV);
        match (&self.provider, &self.credential) {
            (Provider::Claude, AccountCredential::NativeHome(home)) => {
                command.env("CLAUDE_CONFIG_DIR", home);
                command.env_remove("CLAUDE_CODE_OAUTH_TOKEN");
                command.env_remove("ANTHROPIC_API_KEY");
            }
            (Provider::Claude, AccountCredential::AccessToken(token)) => {
                command.env("CLAUDE_CODE_OAUTH_TOKEN", token);
                command.env_remove("CLAUDE_CONFIG_DIR");
                command.env_remove("ANTHROPIC_API_KEY");
            }
            (Provider::Codex, AccountCredential::NativeHome(home)) => {
                command.env("CODEX_HOME", home);
                command.env_remove("CODEX_ACCESS_TOKEN");
                command.env_remove("OPENAI_API_KEY");
            }
            (Provider::Codex, AccountCredential::AccessToken(token)) => {
                command.env("CODEX_ACCESS_TOKEN", token);
                command.env_remove("CODEX_HOME");
                command.env_remove("OPENAI_API_KEY");
            }
            _ => {}
        }
    }

    pub fn apply_tokio(&self, command: &mut tokio::process::Command) {
        command.env_remove(FORWARDED_ACCOUNT_BUNDLE_ENV);
        match (&self.provider, &self.credential) {
            (Provider::Claude, AccountCredential::NativeHome(home)) => {
                command.env("CLAUDE_CONFIG_DIR", home);
                command.env_remove("CLAUDE_CODE_OAUTH_TOKEN");
                command.env_remove("ANTHROPIC_API_KEY");
            }
            (Provider::Claude, AccountCredential::AccessToken(token)) => {
                command.env("CLAUDE_CODE_OAUTH_TOKEN", token);
                command.env_remove("CLAUDE_CONFIG_DIR");
                command.env_remove("ANTHROPIC_API_KEY");
            }
            (Provider::Codex, AccountCredential::NativeHome(home)) => {
                command.env("CODEX_HOME", home);
                command.env_remove("CODEX_ACCESS_TOKEN");
                command.env_remove("OPENAI_API_KEY");
            }
            (Provider::Codex, AccountCredential::AccessToken(token)) => {
                command.env("CODEX_ACCESS_TOKEN", token);
                command.env_remove("CODEX_HOME");
                command.env_remove("OPENAI_API_KEY");
            }
            _ => {}
        }
    }

    pub async fn pin_session(&self, provider_session_id: &str) -> Result<(), ProviderAccountError> {
        self.store
            .pin_provider_session_route(self.provider, provider_session_id, &self.account_id)
            .await?;
        Ok(())
    }

    pub async fn record_rate_limit(
        &self,
        signal: &RateLimitSignal,
    ) -> Result<(), ProviderAccountError> {
        let cooldown_until = signal.limited.then(|| {
            let now = now_unix();
            signal
                .resets_at
                .unwrap_or(now + DEFAULT_COOLDOWN_SECS)
                .max(now)
                + RESET_GRACE_SECS
        });
        self.store
            .record_provider_account_health(
                self.provider.as_str(),
                &self.account_id,
                signal.utilization_percent,
                cooldown_until,
                signal.limited.then_some(signal.reason.as_str()),
            )
            .await?;
        if !signal.windows.is_empty() {
            self.store
                .upsert_provider_account_limits(
                    self.provider.as_str(),
                    &self.account_id,
                    &signal.windows,
                    "stream",
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbientReason {
    StoreUnavailable,
    NoRoutes { repo_id: Option<RepoId> },
}

#[derive(Debug, Clone)]
pub enum ProviderAccountResolution {
    Managed(ProviderAccountRoute),
    Ambient(AmbientReason),
}

impl ProviderAccountResolution {
    pub fn into_route(self) -> Option<ProviderAccountRoute> {
        match self {
            Self::Managed(route) => Some(route),
            Self::Ambient(_) => None,
        }
    }
}

pub fn parse_account_id(value: &str) -> Result<ProviderAccountId, ProviderAccountError> {
    ProviderAccountId::parse(value).map_err(ProviderAccountError::InvalidAccountId)
}

pub fn account_home_path(
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<PathBuf, ProviderAccountError> {
    ensure_supported(provider)?;
    Ok(crate::store::lf_home_dir()
        .join("accounts")
        .join(provider.as_str())
        .join(account_id.as_str()))
}

pub fn ensure_account_home(
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<PathBuf, ProviderAccountError> {
    let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let home = account_home_path(provider, account_id)?;
    ensure_account_home_at(&operator_home, &home, provider)?;
    Ok(home)
}

pub fn remove_account_home(home: &Path) -> Result<(), ProviderAccountError> {
    if home.exists() {
        fs::remove_dir_all(home).map_err(|error| {
            ProviderAccountError::Filesystem(format!("remove {}: {error}", home.display()))
        })?;
    }
    Ok(())
}

fn ensure_account_home_at(
    operator_home: &Path,
    home: &Path,
    provider: Provider,
) -> Result<(), ProviderAccountError> {
    ensure_supported(provider)?;
    if let Ok(metadata) = fs::symlink_metadata(home) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ProviderAccountError::Filesystem(format!(
                "provider account home {} is not a real directory",
                home.display()
            )));
        }
    }
    fs::create_dir_all(home).map_err(|error| {
        ProviderAccountError::Filesystem(format!("create {}: {error}", home.display()))
    })?;
    set_private_directory(home)?;

    let (canonical, shared_names): (PathBuf, &[&str]) = match provider {
        Provider::Claude => (
            operator_home.join(".claude"),
            &["skills", "commands", "plugins", "settings.json"],
        ),
        Provider::Codex => (
            operator_home.join(".codex"),
            &["config.toml", "rules", "skills"],
        ),
        _ => return Err(ProviderAccountError::UnsupportedProvider),
    };
    for name in shared_names {
        let source = canonical.join(name);
        let target = home.join(name);
        link_shared_path(&source, &target)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), ProviderAccountError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| {
        ProviderAccountError::Filesystem(format!("chmod {}: {error}", path.display()))
    })
}

#[cfg(not(unix))]
fn set_private_directory(_path: &Path) -> Result<(), ProviderAccountError> {
    Ok(())
}

#[cfg(unix)]
fn link_shared_path(source: &Path, target: &Path) -> Result<(), ProviderAccountError> {
    if !source.exists() || target.exists() || target.is_symlink() {
        return Ok(());
    }
    std::os::unix::fs::symlink(source, target).map_err(|error| {
        ProviderAccountError::Filesystem(format!(
            "link {} -> {}: {error}",
            target.display(),
            source.display()
        ))
    })
}

#[cfg(not(unix))]
fn link_shared_path(_source: &Path, _target: &Path) -> Result<(), ProviderAccountError> {
    Ok(())
}

pub fn encode_forwarded_account_bundle(
    bundle: &ForwardedAccountBundle,
) -> Result<String, ProviderAccountError> {
    let json = serde_json::to_vec(bundle)
        .map_err(|error| ProviderAccountError::ForwardedBundle(error.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

pub fn decode_forwarded_account_bundle(
    encoded: &str,
) -> Result<ForwardedAccountBundle, ProviderAccountError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.trim())
        .map_err(|error| ProviderAccountError::ForwardedBundle(error.to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| ProviderAccountError::ForwardedBundle(error.to_string()))
}

pub async fn local_forwarded_account_bundle(
    store: &SharedStore,
) -> Result<Option<ForwardedAccountBundle>, ProviderAccountError> {
    let Some(repo_id) = current_repo_id()? else {
        return Ok(None);
    };
    let mut routes = Vec::new();
    for provider in [Provider::Claude, Provider::Codex] {
        let route = match store
            .provider_route(&RouteScope::Repo(repo_id.clone()), provider)
            .await?
        {
            Some(route) => Some(route),
            None => store.provider_route(&RouteScope::Default, provider).await?,
        };
        if let Some(route) = route {
            routes.push(ForwardedProviderRoute {
                provider,
                accounts: route.accounts,
            });
        }
    }
    if routes.is_empty() {
        return Ok(None);
    }
    let routed_accounts = referenced_provider_accounts(store, &routes).await?;
    let mut accounts = Vec::new();
    let mut credentials = Vec::new();
    for (provider, account) in routed_accounts {
        accounts.push(ForwardedProviderAccount {
            provider,
            account_id: account.account_id.clone(),
            login_email: account.login_email.clone(),
            credential_state: account.credential_state,
            routing_state: account.routing_state,
            plan: account.plan.clone(),
            paid_through: account.paid_through,
            utilization_percent: account.utilization_percent,
            cooldown_until: account.cooldown_until,
            cooldown_reason: account.cooldown_reason.clone(),
        });
        if !account.eligible_for_automatic_routing(time::OffsetDateTime::now_utc().date()) {
            continue;
        }
        let home =
            account
                .home
                .as_deref()
                .ok_or_else(|| ProviderAccountError::ForwardingCredential {
                    provider,
                    account_id: account.account_id.clone(),
                    reason: "managed account has no native credential home".to_string(),
                })?;
        let access_token =
            crate::provider_auth::prepare_provider_account_access_token(provider, home)
                .await
                .map_err(|error| ProviderAccountError::ForwardingCredential {
                    provider,
                    account_id: account.account_id.clone(),
                    reason: error.to_string(),
                })?
                .ok_or_else(|| ProviderAccountError::ForwardingCredential {
                    provider,
                    account_id: account.account_id.clone(),
                    reason: "provider CLI reports no active OAuth login".to_string(),
                })?;
        credentials.push(ForwardedProviderCredential::new(
            provider,
            account.account_id,
            access_token,
        ));
    }
    Ok(Some(ForwardedAccountBundle::new(
        repo_id,
        routes,
        accounts,
        credentials,
    )))
}

async fn referenced_provider_accounts(
    store: &SharedStore,
    routes: &[ForwardedProviderRoute],
) -> Result<Vec<(Provider, ProviderAccount)>, ProviderAccountError> {
    let mut accounts = Vec::new();
    let mut seen_accounts = HashSet::new();
    for route in routes {
        for account_id in &route.accounts {
            if !seen_accounts.insert((route.provider, account_id.clone())) {
                continue;
            }
            let account = store
                .get_provider_account(route.provider.as_str(), account_id)
                .await?
                .ok_or_else(|| {
                    ProviderAccountError::ForwardedBundle(format!(
                        "route references missing {}/{}",
                        route.provider, account_id
                    ))
                })?;
            accounts.push((route.provider, account));
        }
    }
    Ok(accounts)
}

pub async fn resolve_provider_account(
    provider: Provider,
    provider_session_id: Option<&str>,
) -> Result<ProviderAccountResolution, ProviderAccountError> {
    ensure_supported(provider)?;
    if let Ok(selector) = std::env::var(PROVIDER_ACCOUNT_ENV) {
        if !selector.trim().is_empty() {
            return resolve_explicit_account(provider, selector.trim())
                .await
                .map(ProviderAccountResolution::Managed);
        }
    }
    let forwarded_bundle = match std::env::var(FORWARDED_ACCOUNT_BUNDLE_ENV) {
        Ok(value) if !value.trim().is_empty() => Some(decode_forwarded_account_bundle(&value)?),
        _ => None,
    };
    let store = route_store(forwarded_bundle.is_some()).await?;
    let Some(store) = store else {
        return Ok(ProviderAccountResolution::Ambient(
            AmbientReason::StoreUnavailable,
        ));
    };

    let mut access_tokens = HashMap::new();
    let (candidates, routed_repo_id) = if let Some(bundle) = &forwarded_bundle {
        let candidates = hydrate_forwarded_account_bundle(&store, bundle, provider).await?;
        for credential in bundle
            .credentials
            .iter()
            .filter(|credential| credential.provider == provider)
        {
            access_tokens.insert(
                credential.account_id.clone(),
                credential.access_token().to_string(),
            );
        }
        (candidates, Some(bundle.repo_id.clone()))
    } else {
        let repo_id = current_repo_id()?;
        let route = match &repo_id {
            Some(repo_id) => {
                store
                    .provider_route(&RouteScope::Repo(repo_id.clone()), provider)
                    .await?
            }
            None => None,
        };
        let route = match route {
            Some(route) => Some(route),
            None => store.provider_route(&RouteScope::Default, provider).await?,
        };
        let Some(route) = route else {
            return Ok(ProviderAccountResolution::Ambient(
                AmbientReason::NoRoutes { repo_id },
            ));
        };
        (route.accounts, repo_id)
    };

    if candidates.is_empty() {
        return Ok(ProviderAccountResolution::Ambient(
            AmbientReason::NoRoutes {
                repo_id: routed_repo_id,
            },
        ));
    }
    let selection = store
        .select_provider_account(provider, &candidates, provider_session_id)
        .await?;
    let Some(selection) = selection else {
        let accounts = store
            .list_provider_accounts(Some(provider.as_str()))
            .await?;
        let reasons = candidates
            .iter()
            .map(|account_id| {
                accounts
                    .iter()
                    .find(|account| account.account_id == *account_id)
                    .map(account_unavailable_reason)
                    .unwrap_or_else(|| format!("'{account_id}' missing"))
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ProviderAccountError::NoEligibleAccount {
            provider,
            accounts: reasons,
        });
    };
    let account_id = selection.account.account_id.clone();
    let credential = match access_tokens.remove(&account_id) {
        Some(access_token) => AccountCredential::AccessToken(access_token),
        None => match selection.account.home.as_deref() {
            Some(home) => {
                let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                ensure_account_home_at(&operator_home, home, provider)?;
                AccountCredential::NativeHome(home.to_path_buf())
            }
            None => {
                return Err(ProviderAccountError::ForwardedBundle(format!(
                    "missing access token for {provider}/{account_id}"
                )))
            }
        },
    };
    Ok(ProviderAccountResolution::Managed(ProviderAccountRoute {
        provider,
        account_id,
        credential,
        resume_requested_session: selection.resume_requested_session,
        store,
    }))
}

fn account_unavailable_reason(account: &ProviderAccount) -> String {
    let now = now_unix();
    if account.credential_state != CredentialState::Connected {
        return format!("'{}' credential is missing", account.account_id);
    }
    let routing = account.effective_routing_state(time::OffsetDateTime::now_utc().date());
    if routing != RoutingState::Automatic {
        return format!("'{}' routing is {}", account.account_id, routing.as_str());
    }
    if let Some(until) = account.cooldown_until.filter(|until| *until > now) {
        return format!(
            "'{}' cooling{}",
            account.account_id,
            format_reset_time(until)
        );
    }
    format!("'{}' is unavailable", account.account_id)
}

#[derive(Debug)]
enum AccountMatch<'a> {
    One(&'a ProviderAccount),
    Ambiguous(Vec<&'a ProviderAccount>),
    None,
}

/// An explicit selector names an account by login email or account id —
/// exactly, or by any prefix that matches exactly one known account
/// (`manabot` reaches `manabot-eng`). Matching is case-insensitive.
fn match_account<'a>(accounts: &'a [ProviderAccount], selector: &str) -> AccountMatch<'a> {
    let selector_lower = selector.to_ascii_lowercase();
    let exact = accounts.iter().find(|account| {
        account
            .login_email
            .as_ref()
            .is_some_and(|email| email.as_str().eq_ignore_ascii_case(selector))
            || account.account_id.as_str().eq_ignore_ascii_case(selector)
    });
    if let Some(account) = exact {
        return AccountMatch::One(account);
    }
    let prefixed: Vec<&ProviderAccount> = accounts
        .iter()
        .filter(|account| {
            account.login_email.as_ref().is_some_and(|email| {
                email
                    .as_str()
                    .to_ascii_lowercase()
                    .starts_with(&selector_lower)
            }) || account
                .account_id
                .as_str()
                .to_ascii_lowercase()
                .starts_with(&selector_lower)
        })
        .collect();
    match prefixed.as_slice() {
        [] => AccountMatch::None,
        [account] => AccountMatch::One(account),
        _ => AccountMatch::Ambiguous(prefixed),
    }
}

/// Resolve `lf --account <email|id>`: the named account, verified live, with
/// no route lookup and no cooldown gating — refusing an explicit human choice
/// helps nobody; a dead credential still errors with the re-login fix.
async fn resolve_explicit_account(
    provider: Provider,
    selector: &str,
) -> Result<ProviderAccountRoute, ProviderAccountError> {
    let store = route_store(true).await?.ok_or_else(|| {
        ProviderAccountError::Runtime("account store unavailable for --account".to_string())
    })?;
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let account = match match_account(&accounts, selector) {
        AccountMatch::One(account) => account,
        AccountMatch::Ambiguous(candidates) => {
            return Err(ProviderAccountError::Runtime(format!(
                "'{selector}' matches several {provider} accounts: {}",
                candidates
                    .iter()
                    .map(|account| account.account_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        AccountMatch::None => {
            return Err(ProviderAccountError::Runtime(format!(
                "no managed {provider} account matches '{selector}'; see `lf auth accounts`"
            )));
        }
    };
    let home = account.home.clone().ok_or_else(|| {
        ProviderAccountError::Runtime(format!(
            "{provider} account '{}' has no managed credential home",
            account.account_id
        ))
    })?;
    let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    ensure_account_home_at(&operator_home, &home, provider)?;
    let status = provider_account_auth_status(provider, home.clone())
        .await
        .map_err(|error| {
            ProviderAccountError::Runtime(format!(
                "check {provider} account '{}': {error}",
                account.account_id
            ))
        })?;
    if !retain_authenticated_account(&store, account, &status).await? {
        return Err(ProviderAccountError::NoAuthenticatedAccount {
            provider,
            accounts: format!(
                "'{}' with `lf auth connect {provider} {}`",
                account.account_id, account.account_id
            ),
        });
    }
    Ok(ProviderAccountRoute {
        provider,
        account_id: account.account_id.clone(),
        credential: AccountCredential::NativeHome(home),
        resume_requested_session: false,
        store,
    })
}

async fn retain_authenticated_account(
    store: &SharedStore,
    account: &ProviderAccount,
    status: &AuthStatus,
) -> Result<bool, ProviderAccountError> {
    if matches!(status, AuthStatus::Active { .. }) {
        return Ok(true);
    }
    let mut account = account.clone();
    account.credential_state = CredentialState::Missing;
    account.updated_at = now_unix();
    store.upsert_provider_account(&account).await?;
    Ok(false)
}

async fn hydrate_forwarded_account_bundle(
    store: &SharedStore,
    bundle: &ForwardedAccountBundle,
    provider: Provider,
) -> Result<Vec<ProviderAccountId>, ProviderAccountError> {
    let Some(route) = bundle
        .routes
        .iter()
        .find(|route| route.provider == provider)
    else {
        return Ok(Vec::new());
    };

    // The route is written last and serves as the initialization marker. A
    // restarted provider process reuses health accrued in this SSH lease
    // instead of resetting it from the original local snapshot.
    let scope = RouteScope::Repo(bundle.repo_id.clone());
    if store.provider_route(&scope, provider).await?.is_none() {
        let now = now_unix();
        for account in &bundle.accounts {
            store
                .upsert_provider_account(&ProviderAccount {
                    provider: account.provider.as_str().to_string(),
                    account_id: account.account_id.clone(),
                    home: None,
                    login_email: account.login_email.clone(),
                    credential_state: account.credential_state,
                    routing_state: account.routing_state,
                    plan: account.plan.clone(),
                    paid_through: account.paid_through,
                    utilization_percent: account.utilization_percent,
                    cooldown_until: account.cooldown_until,
                    cooldown_reason: account.cooldown_reason.clone(),
                    last_selected_at: None,
                    created_at: now,
                    updated_at: now,
                })
                .await?;
        }
        store
            .set_provider_route(&ProviderRoute {
                scope,
                provider,
                accounts: route.accounts.clone(),
                created_at: now,
                updated_at: now,
            })
            .await?;
    }

    Ok(route.accounts.clone())
}

fn current_repo_id() -> Result<Option<RepoId>, ProviderAccountError> {
    if let Ok(value) = std::env::var(ACCOUNT_REPO_ID_ENV) {
        return RepoId::parse(&value)
            .map(Some)
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()));
    }
    let current = std::env::current_dir()
        .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
    Ok(RepoId::discover(&current).ok())
}

pub fn resolve_provider_account_blocking(
    provider: Provider,
    provider_session_id: Option<String>,
) -> Result<ProviderAccountResolution, ProviderAccountError> {
    std::thread::Builder::new()
        .name(format!("lf-{}-account-route", provider.as_str()))
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
            runtime.block_on(resolve_provider_account(
                provider,
                provider_session_id.as_deref(),
            ))
        })
        .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?
        .join()
        .map_err(|_| ProviderAccountError::Runtime("account resolver panicked".to_string()))?
}

async fn route_store(create: bool) -> Result<Option<SharedStore>, ProviderAccountError> {
    if !create {
        return match crate::store::open_registry_for_authority().await {
            Ok(store) => Ok(Some(Arc::new(store))),
            Err(crate::store::RegistryUnavailable::MissingFile { .. }) => Ok(None),
            Err(error) => Err(ProviderAccountError::Runtime(format!(
                "open provider account store: {error:?}"
            ))),
        };
    }
    let config = match std::env::var_os(FORWARDED_ACCOUNT_STORE_ENV) {
        Some(path) => crate::store::StorageConfig::sqlite(PathBuf::from(path)),
        None => crate::store::storage_config_from_env().map_err(|error| {
            ProviderAccountError::Filesystem(format!("resolve provider account store: {error}"))
        })?,
    };
    Ok(Some(Arc::new(open_store(&config).await?)))
}

pub async fn open_account_store() -> Result<SharedStore, ProviderAccountError> {
    let config = crate::store::storage_config_from_env().map_err(|error| {
        ProviderAccountError::Filesystem(format!("resolve provider account store: {error}"))
    })?;
    let store = Arc::new(open_store(&config).await?);
    Ok(store)
}

pub fn new_account(
    provider: Provider,
    account_id: ProviderAccountId,
    home: PathBuf,
    login_email: Option<crate::profile::EmailAddress>,
) -> ProviderAccount {
    let now = now_unix();
    ProviderAccount {
        provider: provider.as_str().to_string(),
        account_id,
        home: Some(home),
        login_email,
        credential_state: CredentialState::Connected,
        routing_state: RoutingState::Automatic,
        plan: None,
        paid_through: None,
        utilization_percent: None,
        cooldown_until: None,
        cooldown_reason: None,
        last_selected_at: None,
        created_at: now,
        updated_at: now,
    }
}

fn ensure_supported(provider: Provider) -> Result<(), ProviderAccountError> {
    if matches!(provider, Provider::Claude | Provider::Codex) {
        Ok(())
    } else {
        Err(ProviderAccountError::UnsupportedProvider)
    }
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn format_reset_time(timestamp: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(timestamp)
        .ok()
        .and_then(|value| {
            value
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .map(|value| format!(" until {value}"))
        .unwrap_or_else(|| format!(" until unix {timestamp}"))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn explicit_account_selector_matches_exactly_or_by_unique_prefix() {
        let temp = tempdir().unwrap();
        let accounts = vec![
            new_account(
                Provider::Codex,
                parse_account_id("engineering").unwrap(),
                temp.path().join("engineering"),
                Some(crate::profile::EmailAddress::parse("loopflow-eng@loopflow.studio").unwrap()),
            ),
            new_account(
                Provider::Codex,
                parse_account_id("manabot-eng").unwrap(),
                temp.path().join("manabot-eng"),
                None,
            ),
            new_account(
                Provider::Codex,
                parse_account_id("manabot-ops").unwrap(),
                temp.path().join("manabot-ops"),
                None,
            ),
        ];
        let one = |selector: &str| match match_account(&accounts, selector) {
            AccountMatch::One(account) => account.account_id.as_str().to_string(),
            other => panic!("expected one match for '{selector}', got {other:?}"),
        };

        assert_eq!(one("LoopFlow-Eng@loopflow.studio"), "engineering");
        assert_eq!(one("manabot-eng"), "manabot-eng");
        // A unique prefix reaches its account; email prefixes count too.
        assert_eq!(one("manabot-e"), "manabot-eng");
        assert_eq!(one("loopflow-eng@"), "engineering");
        // An exact id wins even when it prefixes nothing else and another
        // account's id shares its prefix.
        assert!(matches!(
            match_account(&accounts, "manabot"),
            AccountMatch::Ambiguous(candidates) if candidates.len() == 2
        ));
        assert!(matches!(
            match_account(&accounts, "nobody@example.com"),
            AccountMatch::None
        ));
    }

    #[test]
    fn account_ids_are_shell_and_path_safe() {
        assert_eq!(parse_account_id("primary-2").unwrap().as_str(), "primary-2");
        assert!(parse_account_id("Primary").is_err());
        assert!(parse_account_id("../primary").is_err());
        assert!(parse_account_id("primary account").is_err());
    }

    #[test]
    fn account_homes_are_private_and_link_shared_configuration() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let claude_home = operator_home.join(".claude");
        fs::create_dir_all(claude_home.join("skills")).unwrap();
        fs::write(claude_home.join("settings.json"), "{}").unwrap();
        let account_home = temp.path().join("accounts/claude/primary");

        ensure_account_home_at(&operator_home, &account_home, Provider::Claude).unwrap();

        assert_eq!(
            fs::metadata(&account_home).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::read_link(account_home.join("skills")).unwrap(),
            claude_home.join("skills")
        );
        assert_eq!(
            fs::read_link(account_home.join("settings.json")).unwrap(),
            claude_home.join("settings.json")
        );
        assert!(!account_home.join(".credentials.json").exists());

        remove_account_home(&account_home).unwrap();
        assert!(!account_home.exists());
        assert!(claude_home.join("skills").exists());
        assert!(claude_home.join("settings.json").exists());
    }

    #[test]
    fn codex_account_home_links_config_but_not_auth() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let codex_home = operator_home.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "model = \"gpt-5\"").unwrap();
        let account_home = temp.path().join("accounts/codex/reserve");

        ensure_account_home_at(&operator_home, &account_home, Provider::Codex).unwrap();

        assert_eq!(
            fs::read_link(account_home.join("config.toml")).unwrap(),
            codex_home.join("config.toml")
        );
        assert!(!account_home.join("auth.json").exists());
    }

    #[test]
    fn account_home_relinks_shared_configuration_created_after_login() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let claude_home = operator_home.join(".claude");
        fs::create_dir_all(&claude_home).unwrap();
        let account_home = temp.path().join("accounts/claude/primary");

        ensure_account_home_at(&operator_home, &account_home, Provider::Claude).unwrap();
        assert!(!account_home.join("skills").exists());

        fs::create_dir_all(claude_home.join("skills")).unwrap();
        ensure_account_home_at(&operator_home, &account_home, Provider::Claude).unwrap();

        assert_eq!(
            fs::read_link(account_home.join("skills")).unwrap(),
            claude_home.join("skills")
        );
    }

    #[test]
    fn account_home_creation_rejects_a_symlink() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let account_home = temp.path().join("accounts/claude/primary");
        fs::create_dir_all(account_home.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&outside, &account_home).unwrap();

        let error = ensure_account_home_at(&operator_home, &account_home, Provider::Claude)
            .expect_err("symlinked account home must be rejected");

        assert!(error.to_string().contains("not a real directory"));
    }

    #[tokio::test]
    async fn routes_apply_only_the_selected_provider_credential() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let route = ProviderAccountRoute {
            provider: Provider::Codex,
            account_id: parse_account_id("reserve").unwrap(),
            credential: AccountCredential::AccessToken("codex-secret".to_string()),
            resume_requested_session: false,
            store,
        };
        let mut command = Command::new("codex");
        route.apply(&mut command);
        let environment = command
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().to_string(),
                    value.map(|value| value.to_string_lossy().to_string()),
                )
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(
            environment
                .get("CODEX_ACCESS_TOKEN")
                .and_then(|value| value.as_deref()),
            Some("codex-secret")
        );
        assert_eq!(environment.get("OPENAI_API_KEY"), Some(&None));
        assert_eq!(environment.get("CODEX_HOME"), Some(&None));
        assert_eq!(environment.get(FORWARDED_ACCOUNT_BUNDLE_ENV), Some(&None));
        assert!(!environment.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));

        let mut async_command = tokio::process::Command::new("codex");
        route.apply_tokio(&mut async_command);
        assert!(async_command
            .as_std()
            .get_envs()
            .any(|(name, value)| name == FORWARDED_ACCOUNT_BUNDLE_ENV && value.is_none()));
    }

    #[tokio::test]
    async fn native_routes_select_independent_provider_homes() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let claude_home = temp.path().join("claude-primary");
        let codex_home = temp.path().join("codex-reserve");
        let routes = [
            (
                ProviderAccountRoute {
                    provider: Provider::Claude,
                    account_id: parse_account_id("primary").unwrap(),
                    credential: AccountCredential::NativeHome(claude_home.clone()),
                    resume_requested_session: false,
                    store: store.clone(),
                },
                "CLAUDE_CONFIG_DIR",
                claude_home,
            ),
            (
                ProviderAccountRoute {
                    provider: Provider::Codex,
                    account_id: parse_account_id("reserve").unwrap(),
                    credential: AccountCredential::NativeHome(codex_home.clone()),
                    resume_requested_session: false,
                    store,
                },
                "CODEX_HOME",
                codex_home,
            ),
        ];

        for (route, env_name, expected_home) in routes {
            let mut command = Command::new(route.provider.as_str());
            route.apply(&mut command);
            let selected_home = command
                .get_envs()
                .find(|(name, _)| *name == std::ffi::OsStr::new(env_name))
                .and_then(|(_, value)| value)
                .map(PathBuf::from);
            assert_eq!(selected_home.as_deref(), Some(expected_home.as_path()));
        }
    }

    #[tokio::test]
    async fn expired_live_check_marks_the_credential_missing() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account_id = parse_account_id("primary").unwrap();
        let account = new_account(
            Provider::Claude,
            account_id.clone(),
            temp.path().join("primary"),
            None,
        );
        store.upsert_provider_account(&account).await.unwrap();

        let retained = retain_authenticated_account(&store, &account, &AuthStatus::Expired)
            .await
            .unwrap();

        assert!(!retained);
        let account = store
            .get_provider_account(Provider::Claude.as_str(), &account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.credential_state, CredentialState::Missing);
    }

    #[tokio::test]
    async fn hard_rate_limit_cools_the_active_account() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account_id = parse_account_id("primary").unwrap();
        let account = new_account(
            Provider::Claude,
            account_id.clone(),
            temp.path().join("primary"),
            None,
        );
        store.upsert_provider_account(&account).await.unwrap();
        let route = ProviderAccountRoute {
            provider: Provider::Claude,
            account_id: account_id.clone(),
            credential: AccountCredential::AccessToken("claude-secret".to_string()),
            resume_requested_session: false,
            store: store.clone(),
        };

        route
            .record_rate_limit(&RateLimitSignal {
                utilization_percent: Some(100),
                resets_at: Some(now_unix() + 300),
                limited: true,
                reason: "five_hour".to_string(),
                windows: vec![crate::store::AccountLimitWindow {
                    window: "session".to_string(),
                    used_percent: 100,
                    resets_at: Some(now_unix() + 300),
                    plan: None,
                }],
            })
            .await
            .unwrap();

        let account = store
            .get_provider_account("claude", &account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.utilization_percent, Some(100));
        assert!(account
            .cooldown_until
            .is_some_and(|until| until > now_unix()));
        assert_eq!(account.cooldown_reason.as_deref(), Some("five_hour"));
    }
}

#[cfg(test)]
mod account_first_tests {
    use std::ffi::OsString;
    use std::sync::Arc;

    use tempfile::tempdir;

    use super::*;
    use crate::profile::{ProviderRoute, RouteScope};
    use crate::store::{open_store, StorageConfig};

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

    fn account(provider: Provider, account_id: &str, home: &Path) -> ProviderAccount {
        new_account(
            provider,
            parse_account_id(account_id).unwrap(),
            home.join(account_id),
            Some(
                crate::profile::EmailAddress::parse(&format!("{account_id}@example.com")).unwrap(),
            ),
        )
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn default_route_selection_does_not_invoke_the_provider_cli() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let claude = bin.join("claude");
        fs::write(&claude, "#!/bin/sh\nexit 99\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&claude).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&claude, permissions).unwrap();
        }
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "PATH",
            PROVIDER_ACCOUNT_ENV,
            FORWARDED_ACCOUNT_BUNDLE_ENV,
            FORWARDED_ACCOUNT_STORE_ENV,
        ]);
        std::env::set_var("LF_HOME", temp.path());
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        std::env::remove_var(PROVIDER_ACCOUNT_ENV);
        std::env::remove_var(FORWARDED_ACCOUNT_BUNDLE_ENV);
        std::env::remove_var(FORWARDED_ACCOUNT_STORE_ENV);
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let selected = account(Provider::Claude, "selected", temp.path());
        store.upsert_provider_account(&selected).await.unwrap();
        store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider: Provider::Claude,
                accounts: vec![selected.account_id.clone()],
                created_at: now_unix(),
                updated_at: now_unix(),
            })
            .await
            .unwrap();

        let selection = resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .into_route()
            .expect("default route should select a managed account");

        assert_eq!(selection.account_id(), &selected.account_id);
        assert!(store
            .get_provider_account(Provider::Claude.as_str(), &selected.account_id)
            .await
            .unwrap()
            .unwrap()
            .last_selected_at
            .is_some());
    }

    #[tokio::test]
    async fn session_resume_is_pinned_by_account_only() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("store.db")))
                .await
                .unwrap(),
        );
        let first = account(Provider::Codex, "first", temp.path());
        let second = account(Provider::Codex, "second", temp.path());
        store.upsert_provider_account(&first).await.unwrap();
        store.upsert_provider_account(&second).await.unwrap();
        store
            .pin_provider_session_route(Provider::Codex, "session", &second.account_id)
            .await
            .unwrap();

        let selection = store
            .select_provider_account(
                Provider::Codex,
                &[first.account_id, second.account_id.clone()],
                Some("session"),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(selection.account.account_id, second.account_id);
        assert!(selection.resume_requested_session);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn ambient_states_and_an_unusable_configured_route_are_distinct() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            PROVIDER_ACCOUNT_ENV,
            FORWARDED_ACCOUNT_BUNDLE_ENV,
            FORWARDED_ACCOUNT_STORE_ENV,
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var(PROVIDER_ACCOUNT_ENV);
        std::env::remove_var(FORWARDED_ACCOUNT_BUNDLE_ENV);
        std::env::remove_var(FORWARDED_ACCOUNT_STORE_ENV);

        assert!(matches!(
            resolve_provider_account(Provider::Claude, None)
                .await
                .unwrap(),
            ProviderAccountResolution::Ambient(AmbientReason::StoreUnavailable)
        ));

        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        assert!(matches!(
            resolve_provider_account(Provider::Claude, None)
                .await
                .unwrap(),
            ProviderAccountResolution::Ambient(AmbientReason::NoRoutes { .. })
        ));

        let mut unavailable = account(Provider::Claude, "missing", temp.path());
        unavailable.credential_state = CredentialState::Missing;
        store.upsert_provider_account(&unavailable).await.unwrap();
        store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider: Provider::Claude,
                accounts: vec![unavailable.account_id],
                created_at: now_unix(),
                updated_at: now_unix(),
            })
            .await
            .unwrap();

        assert_eq!(
            resolve_provider_account(Provider::Claude, None)
                .await
                .unwrap_err()
                .to_string(),
            "configured claude route has no eligible account: 'missing' credential is missing"
        );
    }

    #[test]
    fn forwarded_account_bundle_round_trips_without_exposing_tokens() {
        let account_id = parse_account_id("engineering").unwrap();
        let bundle = ForwardedAccountBundle::new(
            RepoId::parse("loopflowstudio/loopflow").unwrap(),
            vec![ForwardedProviderRoute {
                provider: Provider::Codex,
                accounts: vec![account_id.clone()],
            }],
            vec![],
            vec![ForwardedProviderCredential::new(
                Provider::Codex,
                account_id,
                "secret-token".to_string(),
            )],
        );

        let encoded = encode_forwarded_account_bundle(&bundle).unwrap();
        assert_eq!(decode_forwarded_account_bundle(&encoded).unwrap(), bundle);
        assert!(!format!("{bundle:?}").contains("secret-token"));
    }
}
