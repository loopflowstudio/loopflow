//! Host-local provider profiles, account selection, and process-lifetime SSH
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

use crate::profile::{
    ChromeProfileBinding, HostId, Profile, ProfileId, ProfileProviderAccount,
    ProviderProfileCandidate, RepoProfileRoute,
};
use crate::provider_auth::{provider_account_auth_status, AuthStatus, Provider};
use crate::repository::RepoId;
use crate::store::{open_store, ProviderAccount, ProviderAccountId, SharedStore, StoreError};
use crate::store::{CredentialState, RoutingState};

pub const FORWARDED_PROFILE_BUNDLE_ENV: &str = "LF_FORWARDED_PROFILE_BUNDLE";
pub const FORWARDED_PROFILE_STORE_ENV: &str = "LF_FORWARDED_PROFILE_STORE";
pub const PROFILE_REPO_ID_ENV: &str = "LF_PROFILE_REPO_ID";
const DEFAULT_COOLDOWN_SECS: i64 = 15 * 60;
const RESET_GRACE_SECS: i64 = 5;
const LEGACY_CHROME_PROFILE_FILE: &str = ".loopflow-chrome-profile.json";

#[derive(Debug, Deserialize)]
struct LegacyChromeProfileBinding {
    directory: String,
    label: String,
}

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
    #[error("all {provider} accounts are unavailable{reset}")]
    NoHealthyAccount { provider: Provider, reset: String },
    #[error("no authenticated {provider} account remains; reconnect {accounts}")]
    NoAuthenticatedAccount {
        provider: Provider,
        accounts: String,
    },
    #[error("repository {repo_id} has no {provider} account in its profile route")]
    NoMappedAccount { provider: Provider, repo_id: RepoId },
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
pub struct ForwardedProfileProviderAccount {
    pub profile_id: ProfileId,
    pub provider: Provider,
    pub account_id: ProviderAccountId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardedProfileBundle {
    pub repo_id: RepoId,
    pub default_profile: ProfileId,
    pub backup_profiles: Vec<ProfileId>,
    pub mappings: Vec<ForwardedProfileProviderAccount>,
    pub accounts: Vec<ForwardedProviderAccount>,
    credentials: Vec<ForwardedProviderCredential>,
}

impl ForwardedProfileBundle {
    pub fn new(
        repo_id: RepoId,
        default_profile: ProfileId,
        backup_profiles: Vec<ProfileId>,
        mappings: Vec<ForwardedProfileProviderAccount>,
        accounts: Vec<ForwardedProviderAccount>,
        credentials: Vec<ForwardedProviderCredential>,
    ) -> Self {
        Self {
            repo_id,
            default_profile,
            backup_profiles,
            mappings,
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
    profile_id: ProfileId,
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
            .field("profile_id", &self.profile_id)
            .field("account_id", &self.account_id)
            .field("credential", &credential)
            .field("resume_requested_session", &self.resume_requested_session)
            .finish()
    }
}

impl ProviderAccountRoute {
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

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
        command.env_remove(FORWARDED_PROFILE_BUNDLE_ENV);
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
        command.env_remove(FORWARDED_PROFILE_BUNDLE_ENV);
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
            .pin_provider_session_route(
                self.provider,
                provider_session_id,
                &self.profile_id,
                &self.account_id,
            )
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

pub fn parse_account_id(value: &str) -> Result<ProviderAccountId, ProviderAccountError> {
    ProviderAccountId::parse(value).map_err(ProviderAccountError::InvalidAccountId)
}

pub fn account_profile_path(
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<PathBuf, ProviderAccountError> {
    ensure_supported(provider)?;
    Ok(crate::store::lf_home_dir()
        .join("accounts")
        .join(provider.as_str())
        .join(account_id.as_str()))
}

pub fn ensure_account_profile(
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<PathBuf, ProviderAccountError> {
    let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let profile = account_profile_path(provider, account_id)?;
    ensure_account_profile_at(&operator_home, &profile, provider)?;
    Ok(profile)
}

pub fn remove_account_profile(profile: &Path) -> Result<(), ProviderAccountError> {
    if profile.exists() {
        fs::remove_dir_all(profile).map_err(|error| {
            ProviderAccountError::Filesystem(format!("remove {}: {error}", profile.display()))
        })?;
    }
    Ok(())
}

fn ensure_account_profile_at(
    operator_home: &Path,
    profile: &Path,
    provider: Provider,
) -> Result<(), ProviderAccountError> {
    ensure_supported(provider)?;
    if let Ok(metadata) = fs::symlink_metadata(profile) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ProviderAccountError::Filesystem(format!(
                "provider profile {} is not a real directory",
                profile.display()
            )));
        }
    }
    fs::create_dir_all(profile).map_err(|error| {
        ProviderAccountError::Filesystem(format!("create {}: {error}", profile.display()))
    })?;
    set_private_directory(profile)?;

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
        let target = profile.join(name);
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

pub fn encode_forwarded_profile_bundle(
    bundle: &ForwardedProfileBundle,
) -> Result<String, ProviderAccountError> {
    let json = serde_json::to_vec(bundle)
        .map_err(|error| ProviderAccountError::ForwardedBundle(error.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

pub fn decode_forwarded_profile_bundle(
    encoded: &str,
) -> Result<ForwardedProfileBundle, ProviderAccountError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.trim())
        .map_err(|error| ProviderAccountError::ForwardedBundle(error.to_string()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| ProviderAccountError::ForwardedBundle(error.to_string()))
}

pub async fn local_forwarded_profile_bundle(
    store: &SharedStore,
) -> Result<Option<ForwardedProfileBundle>, ProviderAccountError> {
    let Some(repo_id) = current_repo_id()? else {
        return Ok(None);
    };
    let Some(route) = store.repo_profile_route(&repo_id).await? else {
        return Ok(None);
    };
    let (mappings, routed_accounts) = referenced_provider_accounts(store, &route).await?;
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
    Ok(Some(ForwardedProfileBundle::new(
        repo_id,
        route.default_profile,
        route.backup_profiles,
        mappings,
        accounts,
        credentials,
    )))
}

async fn referenced_provider_accounts(
    store: &SharedStore,
    route: &RepoProfileRoute,
) -> Result<
    (
        Vec<ForwardedProfileProviderAccount>,
        Vec<(Provider, ProviderAccount)>,
    ),
    ProviderAccountError,
> {
    let mut profile_ids = Vec::with_capacity(route.backup_profiles.len() + 1);
    profile_ids.push(route.default_profile.clone());
    profile_ids.extend(route.backup_profiles.iter().cloned());
    let mut mappings = Vec::new();
    let mut accounts = Vec::new();
    let mut seen_accounts = HashSet::new();
    for profile_id in profile_ids {
        for provider in [Provider::Claude, Provider::Codex] {
            let Some(mapping) = store
                .profile_provider_account(&profile_id, provider)
                .await?
            else {
                continue;
            };
            mappings.push(ForwardedProfileProviderAccount {
                profile_id: profile_id.clone(),
                provider,
                account_id: mapping.account_id.clone(),
            });
            if !seen_accounts.insert((provider, mapping.account_id.clone())) {
                continue;
            }
            let account = store
                .get_provider_account(provider.as_str(), &mapping.account_id)
                .await?
                .ok_or_else(|| {
                    ProviderAccountError::ForwardedBundle(format!(
                        "profile {profile_id} references missing {provider}/{}",
                        mapping.account_id
                    ))
                })?;
            accounts.push((provider, account));
        }
    }
    Ok((mappings, accounts))
}

pub async fn resolve_provider_account(
    provider: Provider,
    provider_session_id: Option<&str>,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    ensure_supported(provider)?;
    let forwarded_bundle = match std::env::var(FORWARDED_PROFILE_BUNDLE_ENV) {
        Ok(value) if !value.trim().is_empty() => Some(decode_forwarded_profile_bundle(&value)?),
        _ => None,
    };
    let store = route_store(forwarded_bundle.is_some()).await?;
    let Some(store) = store else {
        return Ok(None);
    };

    let mut access_tokens = HashMap::new();
    let (candidates, routed_repo_id) = if let Some(bundle) = &forwarded_bundle {
        let candidates = hydrate_forwarded_profile_bundle(&store, bundle, provider).await?;
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
        (candidates, bundle.repo_id.clone())
    } else {
        let Some(repo_id) = current_repo_id()? else {
            return Ok(None);
        };
        let Some(route) = store.repo_profile_route(&repo_id).await? else {
            return Ok(None);
        };
        let mut profile_ids = Vec::with_capacity(route.backup_profiles.len() + 1);
        profile_ids.push(route.default_profile);
        profile_ids.extend(route.backup_profiles);
        let mut candidates = Vec::new();
        for profile_id in profile_ids {
            if let Some(mapping) = store
                .profile_provider_account(&profile_id, provider)
                .await?
            {
                candidates.push(ProviderProfileCandidate {
                    profile_id,
                    account_id: mapping.account_id,
                });
            }
        }
        (candidates, repo_id)
    };

    if candidates.is_empty() {
        return Err(ProviderAccountError::NoMappedAccount {
            provider,
            repo_id: routed_repo_id,
        });
    }
    let mut unauthenticated = Vec::new();
    loop {
        let selection = store
            .select_provider_profile(provider, &candidates, provider_session_id)
            .await?;
        let Some(selection) = selection else {
            if !unauthenticated.is_empty() {
                return Err(ProviderAccountError::NoAuthenticatedAccount {
                    provider,
                    accounts: unauthenticated.join(", "),
                });
            }
            let accounts = store
                .list_provider_accounts(Some(provider.as_str()))
                .await?;
            let now = now_unix();
            let reset = accounts
                .iter()
                .filter(|account| {
                    account.eligible_for_automatic_routing(time::OffsetDateTime::now_utc().date())
                        && candidates
                            .iter()
                            .any(|candidate| candidate.account_id == account.account_id)
                })
                .filter_map(|account| account.cooldown_until.filter(|until| *until > now))
                .min()
                .map(format_reset_time)
                .unwrap_or_default();
            return Err(ProviderAccountError::NoHealthyAccount { provider, reset });
        };
        let profile_id = selection.profile_id;
        let account_id = selection.account.account_id.clone();
        let credential = match access_tokens.remove(&account_id) {
            Some(access_token) => AccountCredential::AccessToken(access_token),
            None => match selection.account.home.as_deref() {
                Some(home) => {
                    let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                    ensure_account_profile_at(&operator_home, home, provider)?;
                    let status = provider_account_auth_status(provider, home.to_path_buf())
                        .await
                        .map_err(|error| {
                            ProviderAccountError::Runtime(format!(
                                "check {provider} account '{account_id}': {error}"
                            ))
                        })?;
                    if !retain_authenticated_account(&store, &selection.account, &status).await? {
                        unauthenticated.push(format!(
                            "'{account_id}' with `lf auth connect {provider} --profile {profile_id}`"
                        ));
                        continue;
                    }
                    AccountCredential::NativeHome(home.to_path_buf())
                }
                None => {
                    return Err(ProviderAccountError::ForwardedBundle(format!(
                        "missing access token for {provider}/{account_id}"
                    )))
                }
            },
        };
        return Ok(Some(ProviderAccountRoute {
            provider,
            profile_id,
            account_id,
            credential,
            resume_requested_session: selection.resume_requested_session,
            store,
        }));
    }
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

async fn hydrate_forwarded_profile_bundle(
    store: &SharedStore,
    bundle: &ForwardedProfileBundle,
    provider: Provider,
) -> Result<Vec<ProviderProfileCandidate>, ProviderAccountError> {
    let mut profile_ids = Vec::with_capacity(bundle.backup_profiles.len() + 1);
    profile_ids.push(bundle.default_profile.clone());
    profile_ids.extend(bundle.backup_profiles.iter().cloned());

    // The route is written last and serves as the initialization marker. A
    // restarted provider process reuses health accrued in this SSH lease
    // instead of resetting it from the original local snapshot.
    if store.repo_profile_route(&bundle.repo_id).await?.is_none() {
        let now = now_unix();
        for profile_id in &profile_ids {
            store
                .upsert_profile(&Profile {
                    id: profile_id.clone(),
                    created_at: now,
                    updated_at: now,
                })
                .await?;
        }
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
        for mapping in &bundle.mappings {
            store
                .set_profile_provider_account(&ProfileProviderAccount {
                    profile_id: mapping.profile_id.clone(),
                    provider: mapping.provider,
                    account_id: mapping.account_id.clone(),
                    created_at: now,
                    updated_at: now,
                })
                .await?;
        }
        store
            .set_repo_profile_route(&crate::profile::RepoProfileRoute {
                repo_id: bundle.repo_id.clone(),
                default_profile: bundle.default_profile.clone(),
                backup_profiles: bundle.backup_profiles.clone(),
                created_at: now,
                updated_at: now,
            })
            .await?;
    }

    Ok(profile_ids
        .into_iter()
        .filter_map(|profile_id| {
            bundle
                .mappings
                .iter()
                .find(|mapping| mapping.profile_id == profile_id && mapping.provider == provider)
                .map(|mapping| ProviderProfileCandidate {
                    profile_id,
                    account_id: mapping.account_id.clone(),
                })
        })
        .collect())
}

fn current_repo_id() -> Result<Option<RepoId>, ProviderAccountError> {
    if let Ok(value) = std::env::var(PROFILE_REPO_ID_ENV) {
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
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
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
        return Ok(crate::store::open_existing_store().await.map(Arc::new));
    }
    let config = match std::env::var_os(FORWARDED_PROFILE_STORE_ENV) {
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
    migrate_legacy_chrome_bindings(&store).await?;
    Ok(store)
}

async fn migrate_legacy_chrome_bindings(store: &SharedStore) -> Result<(), ProviderAccountError> {
    let host_id = HostId::local().map_err(ProviderAccountError::Filesystem)?;
    let mappings = store.list_profile_provider_accounts(None).await?;
    for mapping in mappings
        .into_iter()
        .filter(|mapping| mapping.provider == Provider::Claude)
    {
        if store
            .chrome_profile_binding(&mapping.profile_id, &host_id)
            .await?
            .is_some()
        {
            continue;
        }
        let Some(account) = store
            .get_provider_account(Provider::Claude.as_str(), &mapping.account_id)
            .await?
        else {
            continue;
        };
        let Some(home) = account.home else {
            continue;
        };
        let legacy_path = home.join(LEGACY_CHROME_PROFILE_FILE);
        if !legacy_path.is_file() {
            continue;
        }
        let bytes = fs::read(&legacy_path).map_err(|error| {
            ProviderAccountError::Filesystem(format!(
                "read legacy Chrome binding {}: {error}",
                legacy_path.display()
            ))
        })?;
        let legacy =
            serde_json::from_slice::<LegacyChromeProfileBinding>(&bytes).map_err(|error| {
                ProviderAccountError::Filesystem(format!(
                    "parse legacy Chrome binding {}: {error}",
                    legacy_path.display()
                ))
            })?;
        if !legacy
            .label
            .eq_ignore_ascii_case(mapping.profile_id.as_str())
        {
            continue;
        }
        let now = now_unix();
        store
            .upsert_chrome_profile_binding(&ChromeProfileBinding {
                profile_id: mapping.profile_id,
                host_id: host_id.clone(),
                chrome_directory: legacy.directory,
                created_at: now,
                updated_at: now,
            })
            .await?;
        fs::remove_file(&legacy_path).map_err(|error| {
            ProviderAccountError::Filesystem(format!(
                "remove migrated Chrome binding {}: {error}",
                legacy_path.display()
            ))
        })?;
    }
    Ok(())
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
    fn account_ids_are_shell_and_path_safe() {
        assert_eq!(parse_account_id("primary-2").unwrap().as_str(), "primary-2");
        assert!(parse_account_id("Primary").is_err());
        assert!(parse_account_id("../primary").is_err());
        assert!(parse_account_id("primary account").is_err());
    }

    #[test]
    fn profiles_are_private_and_link_shared_configuration() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let claude_home = operator_home.join(".claude");
        fs::create_dir_all(claude_home.join("skills")).unwrap();
        fs::write(claude_home.join("settings.json"), "{}").unwrap();
        let profile = temp.path().join("accounts/claude/primary");

        ensure_account_profile_at(&operator_home, &profile, Provider::Claude).unwrap();

        assert_eq!(
            fs::metadata(&profile).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::read_link(profile.join("skills")).unwrap(),
            claude_home.join("skills")
        );
        assert_eq!(
            fs::read_link(profile.join("settings.json")).unwrap(),
            claude_home.join("settings.json")
        );
        assert!(!profile.join(".credentials.json").exists());

        remove_account_profile(&profile).unwrap();
        assert!(!profile.exists());
        assert!(claude_home.join("skills").exists());
        assert!(claude_home.join("settings.json").exists());
    }

    #[test]
    fn codex_profile_links_config_but_not_auth() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let codex_home = operator_home.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::write(codex_home.join("config.toml"), "model = \"gpt-5\"").unwrap();
        let profile = temp.path().join("accounts/codex/reserve");

        ensure_account_profile_at(&operator_home, &profile, Provider::Codex).unwrap();

        assert_eq!(
            fs::read_link(profile.join("config.toml")).unwrap(),
            codex_home.join("config.toml")
        );
        assert!(!profile.join("auth.json").exists());
    }

    #[test]
    fn profile_relinks_shared_configuration_created_after_login() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let claude_home = operator_home.join(".claude");
        fs::create_dir_all(&claude_home).unwrap();
        let profile = temp.path().join("accounts/claude/primary");

        ensure_account_profile_at(&operator_home, &profile, Provider::Claude).unwrap();
        assert!(!profile.join("skills").exists());

        fs::create_dir_all(claude_home.join("skills")).unwrap();
        ensure_account_profile_at(&operator_home, &profile, Provider::Claude).unwrap();

        assert_eq!(
            fs::read_link(profile.join("skills")).unwrap(),
            claude_home.join("skills")
        );
    }

    #[test]
    fn profile_creation_rejects_a_symlinked_account_home() {
        let temp = tempdir().unwrap();
        let operator_home = temp.path().join("operator");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let profile = temp.path().join("accounts/claude/primary");
        fs::create_dir_all(profile.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&outside, &profile).unwrap();

        let error = ensure_account_profile_at(&operator_home, &profile, Provider::Claude)
            .expect_err("symlinked profile must be rejected");

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
            profile_id: ProfileId::parse("reserve@example.com").unwrap(),
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
        assert_eq!(environment.get(FORWARDED_PROFILE_BUNDLE_ENV), Some(&None));
        assert!(!environment.contains_key("CLAUDE_CODE_OAUTH_TOKEN"));

        let mut async_command = tokio::process::Command::new("codex");
        route.apply_tokio(&mut async_command);
        assert!(async_command
            .as_std()
            .get_envs()
            .any(|(name, value)| name == FORWARDED_PROFILE_BUNDLE_ENV && value.is_none()));
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
                    profile_id: ProfileId::parse("primary@example.com").unwrap(),
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
                    profile_id: ProfileId::parse("reserve@example.com").unwrap(),
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
    async fn expired_native_account_leaves_automatic_rotation() {
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
            profile_id: ProfileId::parse("primary@example.com").unwrap(),
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

    #[tokio::test]
    async fn legacy_account_chrome_pairing_moves_to_the_profile_binding() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let account_id = parse_account_id("primary").unwrap();
        let account_home = temp.path().join("accounts/claude/primary");
        fs::create_dir_all(&account_home).unwrap();
        let legacy_path = account_home.join(LEGACY_CHROME_PROFILE_FILE);
        fs::write(
            &legacy_path,
            r#"{"directory":"Profile 3","label":"jack@example.com"}"#,
        )
        .unwrap();
        store
            .upsert_provider_account(&new_account(
                Provider::Claude,
                account_id.clone(),
                account_home,
                Some(crate::profile::EmailAddress::parse("jack@example.com").unwrap()),
            ))
            .await
            .unwrap();
        let profile_id = ProfileId::parse("jack@example.com").unwrap();
        store
            .upsert_profile(&Profile {
                id: profile_id.clone(),
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();
        store
            .set_profile_provider_account(&ProfileProviderAccount {
                profile_id: profile_id.clone(),
                provider: Provider::Claude,
                account_id,
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();

        migrate_legacy_chrome_bindings(&store).await.unwrap();

        let binding = store
            .chrome_profile_binding(&profile_id, &HostId::local().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(binding.chrome_directory, "Profile 3");
        assert!(!legacy_path.exists());
    }

    #[tokio::test]
    async fn forwarded_route_keeps_profile_mappings_but_deduplicates_accounts() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let primary = ProfileId::parse("primary@example.com").unwrap();
        let engineering = ProfileId::parse("engineering@example.com").unwrap();
        let personal = ProfileId::parse("personal@example.com").unwrap();
        for profile_id in [&primary, &engineering, &personal] {
            store
                .upsert_profile(&Profile {
                    id: profile_id.clone(),
                    created_at: 1,
                    updated_at: 1,
                })
                .await
                .unwrap();
        }
        let claude_account = new_account(
            Provider::Claude,
            parse_account_id("personal").unwrap(),
            temp.path().join("claude-personal"),
            Some(crate::profile::EmailAddress::parse("personal@example.com").unwrap()),
        );
        let codex_account = new_account(
            Provider::Codex,
            parse_account_id("engineering").unwrap(),
            temp.path().join("codex-engineering"),
            Some(crate::profile::EmailAddress::parse("engineering@example.com").unwrap()),
        );
        store
            .upsert_provider_account(&claude_account)
            .await
            .unwrap();
        store.upsert_provider_account(&codex_account).await.unwrap();
        for profile_id in [&primary, &engineering, &personal] {
            store
                .set_profile_provider_account(&ProfileProviderAccount {
                    profile_id: profile_id.clone(),
                    provider: Provider::Claude,
                    account_id: claude_account.account_id.clone(),
                    created_at: 1,
                    updated_at: 1,
                })
                .await
                .unwrap();
        }
        store
            .set_profile_provider_account(&ProfileProviderAccount {
                profile_id: engineering.clone(),
                provider: Provider::Codex,
                account_id: codex_account.account_id.clone(),
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();
        let route = RepoProfileRoute {
            repo_id: RepoId::parse("loopflowstudio/loopflow").unwrap(),
            default_profile: primary,
            backup_profiles: vec![engineering, personal],
            created_at: 1,
            updated_at: 1,
        };

        let (mappings, accounts) = referenced_provider_accounts(&store, &route).await.unwrap();

        assert_eq!(mappings.len(), 4);
        assert_eq!(accounts.len(), 2);
        assert_eq!(
            accounts
                .iter()
                .filter(|(provider, _)| *provider == Provider::Claude)
                .count(),
            1
        );
    }

    #[test]
    fn forwarded_bundle_round_trips_without_debugging_tokens() {
        let primary = ProfileId::parse("primary@example.com").unwrap();
        let engineering = ProfileId::parse("engineering@example.com").unwrap();
        let claude = parse_account_id("primary").unwrap();
        let codex = parse_account_id("reserve").unwrap();
        let bundle = ForwardedProfileBundle::new(
            RepoId::parse("loopflowstudio/loopflow").unwrap(),
            primary.clone(),
            vec![engineering.clone()],
            vec![
                ForwardedProfileProviderAccount {
                    profile_id: primary,
                    provider: Provider::Claude,
                    account_id: claude.clone(),
                },
                ForwardedProfileProviderAccount {
                    profile_id: engineering,
                    provider: Provider::Claude,
                    account_id: claude.clone(),
                },
            ],
            vec![ForwardedProviderAccount {
                provider: Provider::Claude,
                account_id: claude.clone(),
                login_email: Some(
                    crate::profile::EmailAddress::parse("personal@example.com").unwrap(),
                ),
                credential_state: CredentialState::Connected,
                routing_state: RoutingState::Automatic,
                plan: Some("max".to_string()),
                paid_through: None,
                utilization_percent: None,
                cooldown_until: None,
                cooldown_reason: None,
            }],
            vec![
                ForwardedProviderCredential::new(
                    Provider::Claude,
                    claude,
                    "secret-claude-access-token".to_string(),
                ),
                ForwardedProviderCredential::new(
                    Provider::Codex,
                    codex,
                    "secret-codex-access-token".to_string(),
                ),
            ],
        );
        let encoded = encode_forwarded_profile_bundle(&bundle).unwrap();
        assert!(!encoded.contains("secret-claude-access-token"));
        assert!(!encoded.contains("secret-codex-access-token"));
        let decoded = decode_forwarded_profile_bundle(&encoded).unwrap();
        assert_eq!(decoded, bundle);
        assert_eq!(decoded.credentials.len(), 2);
        assert!(!format!("{decoded:?}").contains("secret-claude-access-token"));
        assert!(!format!("{decoded:?}").contains("secret-codex-access-token"));
    }

    #[tokio::test]
    async fn restarted_provider_reuses_health_from_the_process_lease() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("lease.db"),
            ))
            .await
            .unwrap(),
        );
        let profile_id = ProfileId::parse("engineering@example.com").unwrap();
        let account_id = parse_account_id("engineering").unwrap();
        let bundle = ForwardedProfileBundle::new(
            RepoId::parse("loopflowstudio/loopflow").unwrap(),
            profile_id.clone(),
            Vec::new(),
            vec![ForwardedProfileProviderAccount {
                profile_id,
                provider: Provider::Codex,
                account_id: account_id.clone(),
            }],
            vec![ForwardedProviderAccount {
                provider: Provider::Codex,
                account_id: account_id.clone(),
                login_email: Some(
                    crate::profile::EmailAddress::parse("engineering@example.com").unwrap(),
                ),
                credential_state: CredentialState::Connected,
                routing_state: RoutingState::Automatic,
                plan: Some("max".to_string()),
                paid_through: None,
                utilization_percent: None,
                cooldown_until: None,
                cooldown_reason: None,
            }],
            vec![ForwardedProviderCredential::new(
                Provider::Codex,
                account_id.clone(),
                "codex-access-token".to_string(),
            )],
        );

        let candidates = hydrate_forwarded_profile_bundle(&store, &bundle, Provider::Codex)
            .await
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let cooldown_until = now_unix() + 300;
        store
            .record_provider_account_health(
                Provider::Codex.as_str(),
                &account_id,
                Some(100),
                Some(cooldown_until),
                Some("remote-limit"),
            )
            .await
            .unwrap();

        hydrate_forwarded_profile_bundle(&store, &bundle, Provider::Codex)
            .await
            .unwrap();

        let account = store
            .get_provider_account(Provider::Codex.as_str(), &account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(account.utilization_percent, Some(100));
        assert_eq!(account.cooldown_until, Some(cooldown_until));
        assert_eq!(account.cooldown_reason.as_deref(), Some("remote-limit"));
    }
}
