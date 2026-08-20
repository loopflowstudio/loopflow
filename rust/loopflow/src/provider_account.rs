//! Host-local provider accounts, selection, and process-lifetime credential
//! leases for Claude and Codex.

pub mod lease;
pub mod recovery;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::profile::RouteScope;
use crate::provider_auth::Provider;
use crate::repository::RepoId;
use crate::store::{
    open_store, AccountLimitRow, ProviderAccount, ProviderAccountId, SharedStore, StoreError,
};
use crate::store::{CredentialState, RoutingState};

const DEFAULT_COOLDOWN_SECS: i64 = 15 * 60;
const RESET_GRACE_SECS: i64 = 5;
const STRAINED_UTILIZATION_PERCENT: u8 = 95;
const PROVIDER_CREDENTIAL_ENV_VARS: [&str; 6] = [
    "CLAUDE_CODE_OAUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "CLAUDE_CONFIG_DIR",
    "CODEX_ACCESS_TOKEN",
    "OPENAI_API_KEY",
    "CODEX_HOME",
];

/// The provider-observed limit window that most justifies demoting an account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountStrain {
    pub window: String,
    pub used_percent: u8,
}

/// An account is strained while a provider-observed window sits at or above
/// [`STRAINED_UTILIZATION_PERCENT`] and has not yet reset. Ties break on window
/// name so the reported evidence is stable however the rows arrive.
pub(crate) fn active_account_strain(
    provider: &str,
    account_id: &ProviderAccountId,
    limits: &[AccountLimitRow],
    now: i64,
) -> Option<AccountStrain> {
    limits
        .iter()
        .filter(|limit| limit.provider == provider && limit.account_id == *account_id)
        .filter(|limit| limit.used_percent >= STRAINED_UTILIZATION_PERCENT)
        .filter(|limit| limit.resets_at.is_some_and(|reset| reset > now))
        .max_by(|left, right| {
            left.used_percent
                .cmp(&right.used_percent)
                .then_with(|| right.window.cmp(&left.window))
        })
        .map(|limit| AccountStrain {
            window: limit.window.clone(),
            used_percent: limit.used_percent,
        })
}

fn is_strained(
    provider: &str,
    account_id: &ProviderAccountId,
    limits: &[AccountLimitRow],
    now: i64,
) -> bool {
    active_account_strain(provider, account_id, limits, now).is_some()
}

/// Stable sort: strained accounts fall behind unstrained ones, and declared
/// route order decides everything else.
pub(crate) fn order_accounts_by_strain(
    accounts: &mut [ProviderAccount],
    limits: &[AccountLimitRow],
    now: i64,
) {
    accounts
        .sort_by_key(|account| is_strained(&account.provider, &account.account_id, limits, now));
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
    #[error("invalid forwarded account lease: {0}")]
    AccountLease(String),
    #[error("cannot forward {provider} account '{account_id}': {reason}")]
    ForwardingCredential {
        provider: Provider,
        account_id: ProviderAccountId,
        reason: String,
    },
    #[error("{0}")]
    Runtime(String),
    #[error("no authenticated {provider} account remains; reconnect {accounts}")]
    NoAuthenticatedAccount {
        provider: Provider,
        accounts: String,
    },
    #[error("no eligible managed {provider} account: {accounts}")]
    NoEligibleAccount {
        provider: Provider,
        accounts: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RateLimitSignal {
    pub utilization_percent: Option<u8>,
    pub resets_at: Option<i64>,
    pub limited: bool,
    pub reason: String,
    /// Per-window subscription state carried by the provider event, persisted
    /// so `lf usage` can answer "how much is left" without a fresh poll.
    pub windows: Vec<crate::store::AccountLimitWindow>,
}

#[derive(Clone)]
enum AccountRouteAuthority {
    Local {
        store: SharedStore,
        home: PathBuf,
    },
    Lease {
        client: lease::AccountLeaseClient,
        access_token: String,
    },
}

#[derive(Clone)]
enum AccountCandidateAuthority {
    Local { store: SharedStore, home: PathBuf },
    Forwarded { client: lease::AccountLeaseClient },
}

#[derive(Clone)]
struct AccountCandidate {
    account: ProviderAccount,
    limits: Vec<AccountLimitRow>,
    credential_available: bool,
    authority: AccountCandidateAuthority,
}

impl AccountCandidate {
    fn is_forwarded(&self) -> bool {
        matches!(self.authority, AccountCandidateAuthority::Forwarded { .. })
    }

    fn eligible_for_automatic_routing(&self, now: i64) -> bool {
        self.credential_available
            && self
                .account
                .eligible_for_automatic_routing(time::OffsetDateTime::now_utc().date())
            && self.account.cooldown_until.is_none_or(|until| until <= now)
    }

    fn is_strained(&self, now: i64) -> bool {
        active_account_strain(
            &self.account.provider,
            &self.account.account_id,
            &self.limits,
            now,
        )
        .is_some()
    }
}

#[derive(Clone)]
pub(crate) struct ProviderAccountRoute {
    provider: Provider,
    account_id: ProviderAccountId,
    resume_requested_session: bool,
    authority: AccountRouteAuthority,
}

impl std::fmt::Debug for ProviderAccountRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let credential = match self.authority {
            AccountRouteAuthority::Local { .. } => "native_home",
            AccountRouteAuthority::Lease { .. } => "access_token",
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
    pub(crate) fn account_id(&self) -> &ProviderAccountId {
        &self.account_id
    }

    pub(crate) fn resume_requested_session(&self) -> bool {
        self.resume_requested_session
    }

    pub(crate) fn uses_native_home(&self) -> bool {
        matches!(self.authority, AccountRouteAuthority::Local { .. })
    }

    /// Prove that this route can authenticate before durable Work is reserved.
    ///
    /// The token is deliberately consumed here and never returned: Task launch
    /// needs evidence that the configured authority is usable, not another
    /// secret-bearing representation to persist or log.
    pub(crate) async fn verify_ready(&self) -> Result<(), ProviderAccountError> {
        match &self.authority {
            AccountRouteAuthority::Local { home, .. } => {
                crate::provider_auth::prepare_provider_account_access_token(self.provider, home)
                    .await
                    .map_err(|error| ProviderAccountError::ForwardingCredential {
                        provider: self.provider,
                        account_id: self.account_id.clone(),
                        reason: error.to_string(),
                    })?
                    .ok_or_else(|| ProviderAccountError::NoAuthenticatedAccount {
                        provider: self.provider,
                        accounts: format!(
                            "{} (provider CLI reports no active OAuth login)",
                            self.account_id
                        ),
                    })?;
            }
            AccountRouteAuthority::Lease { access_token, .. } if access_token.trim().is_empty() => {
                return Err(ProviderAccountError::NoAuthenticatedAccount {
                    provider: self.provider,
                    accounts: format!("{} (forwarded credential is empty)", self.account_id),
                });
            }
            AccountRouteAuthority::Lease { .. } => {}
        }
        Ok(())
    }

    pub(crate) fn apply(&self, command: &mut Command) {
        for name in PROVIDER_CREDENTIAL_ENV_VARS {
            command.env_remove(name);
        }
        match (self.provider, &self.authority) {
            (Provider::Claude, AccountRouteAuthority::Local { home, .. }) => {
                command.env("CLAUDE_CONFIG_DIR", home);
            }
            (Provider::Claude, AccountRouteAuthority::Lease { access_token, .. }) => {
                command.env("CLAUDE_CODE_OAUTH_TOKEN", access_token);
            }
            (Provider::Codex, AccountRouteAuthority::Local { home, .. }) => {
                command.env("CODEX_HOME", home);
            }
            (Provider::Codex, AccountRouteAuthority::Lease { access_token, .. }) => {
                command.env("CODEX_ACCESS_TOKEN", access_token);
            }
            _ => {}
        }
    }

    pub(crate) fn apply_tokio(&self, command: &mut tokio::process::Command) {
        self.apply(command.as_std_mut());
    }

    pub(crate) async fn pin_session(
        &self,
        provider_session_id: &str,
    ) -> Result<(), ProviderAccountError> {
        match &self.authority {
            AccountRouteAuthority::Local { store, .. } => {
                store
                    .pin_provider_session_route(
                        self.provider,
                        provider_session_id,
                        &self.account_id,
                    )
                    .await?;
            }
            AccountRouteAuthority::Lease { client, .. } => {
                client.pin_session(self.provider, provider_session_id, &self.account_id)?;
            }
        }
        Ok(())
    }

    pub(crate) async fn record_rate_limit(
        &self,
        signal: &RateLimitSignal,
    ) -> Result<(), ProviderAccountError> {
        match &self.authority {
            AccountRouteAuthority::Local { store, .. } => {
                record_rate_limit_signal(store, self.provider, &self.account_id, signal, "stream")
                    .await?;
            }
            AccountRouteAuthority::Lease { client, .. } => {
                client.record_health(self.provider, &self.account_id, signal)?;
            }
        }
        Ok(())
    }

    pub(crate) fn record_launch_blocking(
        &self,
        provider_session_id: Option<String>,
        signal: Option<RateLimitSignal>,
    ) -> Result<(), ProviderAccountError> {
        if provider_session_id.is_none() && signal.is_none() {
            return Ok(());
        }
        let route = self.clone();
        _run_blocking_account(self.provider, "record", move |runtime| {
            runtime.block_on(async {
                if let Some(signal) = signal {
                    route.record_rate_limit(&signal).await?;
                }
                if let Some(provider_session_id) = provider_session_id {
                    if let Err(error) = route.pin_session(&provider_session_id).await {
                        tracing::warn!(%error, "failed to pin provider session account");
                    }
                }
                Ok(())
            })
        })
    }

    pub(crate) fn record_credential_invalidated_blocking(
        &self,
        reason: &str,
    ) -> Result<(), ProviderAccountError> {
        let route = self.clone();
        let reason = reason.to_string();
        _run_blocking_account(self.provider, "invalidate", move |runtime| {
            runtime.block_on(async {
                match &route.authority {
                    AccountRouteAuthority::Local { store, .. } => {
                        store
                            .record_provider_account_credential_invalidated(
                                route.provider.as_str(),
                                &route.account_id,
                                &reason,
                            )
                            .await?
                    }
                    AccountRouteAuthority::Lease { client, .. } => client
                        .record_credential_invalidated(
                            route.provider,
                            &route.account_id,
                            &reason,
                        )?,
                }
                Ok(())
            })
        })
    }
}

/// Record a provider rate-limit signal against a store: health row plus any
/// observed limit windows. Shared by the local route and the lease broker so
/// cooldown policy stays in one place.
pub(crate) async fn record_rate_limit_signal(
    store: &SharedStore,
    provider: Provider,
    account_id: &ProviderAccountId,
    signal: &RateLimitSignal,
    source: &str,
) -> Result<(), ProviderAccountError> {
    let cooldown_until = signal.limited.then(|| {
        let now = now_unix();
        signal
            .resets_at
            .unwrap_or(now + DEFAULT_COOLDOWN_SECS)
            .max(now)
            + RESET_GRACE_SECS
    });
    store
        .record_provider_account_health(
            provider.as_str(),
            account_id,
            signal.utilization_percent,
            cooldown_until,
            signal.limited.then_some(signal.reason.as_str()),
        )
        .await?;
    if !signal.windows.is_empty() {
        store
            .upsert_provider_account_limits(provider.as_str(), account_id, &signal.windows, source)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn parse_account_id(value: &str) -> Result<ProviderAccountId, ProviderAccountError> {
    ProviderAccountId::parse(value).map_err(ProviderAccountError::InvalidAccountId)
}

pub(crate) fn account_id_for_login(login: &crate::profile::EmailAddress) -> ProviderAccountId {
    let normalized = login.as_str().to_ascii_lowercase();
    let local = normalized
        .split_once('@')
        .map(|(local, _)| local)
        .unwrap_or("account");
    let mut stem = String::new();
    for character in local.chars() {
        let character = if character.is_ascii_lowercase() || character.is_ascii_digit() {
            character
        } else {
            '-'
        };
        if character != '-' || !stem.ends_with('-') {
            stem.push(character);
        }
    }
    let stem = stem.trim_matches('-');
    let stem = if stem.is_empty() { "account" } else { stem };
    let digest = hex::encode(Sha256::digest(normalized.as_bytes()));
    let maximum_stem_len = 63 - 1 - 12;
    let stem = &stem[..stem.len().min(maximum_stem_len)];
    ProviderAccountId::parse(&format!("{stem}-{}", &digest[..12]))
        .expect("generated account id is path safe and within 63 characters")
}

pub(crate) fn account_login(account: &ProviderAccount) -> &str {
    account
        .login_email
        .as_ref()
        .map(crate::profile::EmailAddress::as_str)
        .unwrap_or_else(|| account.account_id.as_str())
}

pub(crate) fn account_home_path(
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<PathBuf, ProviderAccountError> {
    ensure_supported(provider)?;
    Ok(crate::store::lf_home_dir()
        .join("accounts")
        .join(provider.as_str())
        .join(account_id.as_str()))
}

pub(crate) fn ensure_account_home(
    provider: Provider,
    account_id: &ProviderAccountId,
) -> Result<PathBuf, ProviderAccountError> {
    let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let home = account_home_path(provider, account_id)?;
    ensure_account_home_at(&operator_home, &home, provider)?;
    Ok(home)
}

pub(crate) fn remove_account_home(home: &Path) -> Result<(), ProviderAccountError> {
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

pub(crate) async fn prepare_account_access_token(
    provider: Provider,
    account: &ProviderAccount,
) -> Result<String, ProviderAccountError> {
    let home =
        account
            .home
            .as_deref()
            .ok_or_else(|| ProviderAccountError::ForwardingCredential {
                provider,
                account_id: account.account_id.clone(),
                reason: "managed account has no native credential home".to_string(),
            })?;
    let access_token = crate::provider_auth::prepare_provider_account_access_token(provider, home)
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
    Ok(access_token)
}

pub(crate) async fn resolve_provider_account(
    provider: Provider,
    provider_session_id: Option<&str>,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    resolve_provider_account_exact(provider, provider_session_id, None).await
}

pub(crate) async fn resolve_provider_account_exact(
    provider: Provider,
    provider_session_id: Option<&str>,
    exact_account_id: Option<&ProviderAccountId>,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    ensure_supported(provider)?;
    if let Some(client) = lease::AccountLeaseClient::from_env()? {
        return resolve_merged_provider_account(
            provider,
            provider_session_id,
            exact_account_id,
            client,
        )
        .await;
    }
    let store = route_store().await?;
    let Some(store) = store else {
        return Ok(None);
    };

    let routed_repo_id = current_repo_id()?;
    let Some(candidates) =
        provider_route_account_ids(&store, routed_repo_id.as_ref(), provider).await?
    else {
        return Ok(None);
    };

    if candidates.is_empty() {
        return Ok(None);
    }
    let candidates = match exact_account_id {
        Some(account_id) if candidates.contains(account_id) => vec![account_id.clone()],
        Some(account_id) => {
            return Err(ProviderAccountError::Runtime(format!(
                "{provider}/{account_id} is outside the configured account route"
            )))
        }
        None => candidates,
    };
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
    let home = match selection.account.home.as_deref() {
        Some(home) => {
            let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            ensure_account_home_at(&operator_home, home, provider)?;
            home.to_path_buf()
        }
        None => {
            return Err(ProviderAccountError::Runtime(format!(
                "managed {provider}/{account_id} has no credential home"
            )))
        }
    };
    Ok(Some(ProviderAccountRoute {
        provider,
        account_id,
        resume_requested_session: selection.resume_requested_session,
        authority: AccountRouteAuthority::Local { store, home },
    }))
}

async fn resolve_merged_provider_account(
    provider: Provider,
    provider_session_id: Option<&str>,
    exact_account_id: Option<&ProviderAccountId>,
    client: lease::AccountLeaseClient,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    let forwarded = client.describe()?;
    let grant = forwarded.grant(provider).cloned();
    let local_store = route_store().await?;
    let mut candidates = Vec::new();
    let mut local_route = Vec::new();
    if !forwarded.restricted {
        if let Some(store) = &local_store {
            let repo_id = current_repo_id()?;
            local_route = provider_route_account_ids(store, repo_id.as_ref(), provider)
                .await?
                .unwrap_or_default();
            let limits = store
                .provider_account_limits(Some(provider.as_str()))
                .await?;
            for account in store
                .list_provider_accounts(Some(provider.as_str()))
                .await?
            {
                let Some(home) = account.home.clone() else {
                    continue;
                };
                candidates.push(AccountCandidate {
                    credential_available: account.credential_state == CredentialState::Connected,
                    account,
                    limits: limits.clone(),
                    authority: AccountCandidateAuthority::Local {
                        store: Arc::clone(store),
                        home,
                    },
                });
            }
        }
    }
    let local_count = candidates.len();
    if let Some(grant) = &grant {
        for account_id in &grant.accounts {
            let facts = client.account_facts(provider, account_id)?;
            let Some(account) = facts.account else {
                continue;
            };
            candidates.push(AccountCandidate {
                account,
                limits: facts.limits,
                credential_available: facts.credential_available,
                authority: AccountCandidateAuthority::Forwarded {
                    client: client.clone(),
                },
            });
        }
    }
    let selection = lease::AccountSelection::from_env()?;
    // Resolve target-side selectors across both providers. A selector qualified
    // for Codex must not fail a Claude launch, and vice versa. Equivalent
    // identities are one selection entry, with the target's local copy first;
    // origin preferences retain forwarded provenance through `grant.preferred`.
    let mut catalog = Vec::new();
    let mut seen_accounts = HashSet::new();
    if !forwarded.restricted {
        if let Some(store) = &local_store {
            for account in store.list_provider_accounts(None).await? {
                let key = (account.provider.clone(), account.account_id.clone());
                if account.home.is_some() && seen_accounts.insert(key) {
                    catalog.push(account);
                }
            }
        }
    }
    for forwarded_grant in &forwarded.grants {
        for account_id in &forwarded_grant.accounts {
            let facts = client.account_facts(forwarded_grant.provider, account_id)?;
            if let Some(account) = facts.account {
                let key = (account.provider.clone(), account.account_id.clone());
                if seen_accounts.insert(key) {
                    catalog.push(account);
                }
            }
        }
    }
    let selected = selection.resolved_accounts(&catalog)?;
    if candidates.is_empty() {
        if forwarded.restricted || selection.is_restricted() || exact_account_id.is_some() {
            return Err(ProviderAccountError::NoEligibleAccount {
                provider,
                accounts: "the restricted merged account selection excludes this provider"
                    .to_string(),
            });
        }
        return Ok(None);
    }
    let mut explicitly_preferred = Vec::new();
    for (selected_provider, account_id) in selected {
        if selected_provider != provider {
            continue;
        }
        if let Some(index) = candidates
            .iter()
            .position(|candidate| candidate.account.account_id == account_id)
        {
            explicitly_preferred.push(index);
        }
    }

    let mut order = explicitly_preferred.clone();
    if !selection.is_restricted() {
        if let Some(grant) = &grant {
            for account_id in grant.accounts.iter().take(grant.preferred) {
                if let Some(index) = candidates.iter().position(|candidate| {
                    candidate.is_forwarded() && candidate.account.account_id == *account_id
                }) {
                    push_candidate(&mut order, index);
                    push_candidate(&mut explicitly_preferred, index);
                }
            }
        }
        for account_id in &local_route {
            if let Some(index) = candidates[..local_count]
                .iter()
                .position(|candidate| candidate.account.account_id == *account_id)
            {
                push_candidate(&mut order, index);
            }
        }
        if let Some(grant) = &grant {
            for account_id in &grant.accounts {
                if let Some(index) = candidates[..local_count]
                    .iter()
                    .position(|candidate| candidate.account.account_id == *account_id)
                {
                    push_candidate(&mut order, index);
                }
                if let Some(index) = candidates.iter().position(|candidate| {
                    candidate.is_forwarded() && candidate.account.account_id == *account_id
                }) {
                    push_candidate(&mut order, index);
                }
            }
        }
        for index in 0..local_count {
            push_candidate(&mut order, index);
        }
    }

    if let Some(account_id) = exact_account_id {
        order.retain(|index| candidates[*index].account.account_id == *account_id);
    }
    if order.is_empty() {
        return Err(ProviderAccountError::NoEligibleAccount {
            provider,
            accounts: "the merged local and forwarded account selection is empty".to_string(),
        });
    }

    if let Some(session_id) = provider_session_id {
        let local_pin = match &local_store {
            Some(store) => store.provider_session_account(provider, session_id).await?,
            None => None,
        };
        let forwarded_pin = client.pinned_account(provider, session_id)?;
        if let Some(index) = order.iter().position(|index| {
            let candidate = &candidates[*index];
            match &candidate.authority {
                AccountCandidateAuthority::Local { .. } => local_pin
                    .as_ref()
                    .is_some_and(|account_id| *account_id == candidate.account.account_id),
                AccountCandidateAuthority::Forwarded { .. } => forwarded_pin
                    .as_ref()
                    .is_some_and(|account_id| *account_id == candidate.account.account_id),
            }
        }) {
            let pinned = order.remove(index);
            push_candidate(&mut explicitly_preferred, pinned);
            order.insert(0, pinned);
        }
    }

    let now = now_unix();
    let preferred_count = order
        .iter()
        .take_while(|index| explicitly_preferred.contains(index))
        .count();
    order[preferred_count..].sort_by_key(|index| candidates[*index].is_strained(now));
    let mut last_forwarded_error = None;
    for (position, index) in order.into_iter().enumerate() {
        let candidate = &candidates[index];
        let explicit = position < preferred_count;
        if !candidate.credential_available {
            continue;
        }
        if !explicit && !candidate.eligible_for_automatic_routing(now) {
            continue;
        }
        match &candidate.authority {
            AccountCandidateAuthority::Local { store, home } => {
                if !explicit
                    && store
                        .select_provider_account(
                            provider,
                            std::slice::from_ref(&candidate.account.account_id),
                            provider_session_id,
                        )
                        .await?
                        .is_none()
                {
                    continue;
                }
                let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                ensure_account_home_at(&operator_home, home, provider)?;
                let resumed = match provider_session_id {
                    Some(session_id) => store
                        .provider_session_account(provider, session_id)
                        .await?
                        .is_some_and(|account_id| account_id == candidate.account.account_id),
                    None => false,
                };
                return Ok(Some(ProviderAccountRoute {
                    provider,
                    account_id: candidate.account.account_id.clone(),
                    resume_requested_session: resumed,
                    authority: AccountRouteAuthority::Local {
                        store: Arc::clone(store),
                        home: home.clone(),
                    },
                }));
            }
            AccountCandidateAuthority::Forwarded { client } => {
                match client.resolve_exact(
                    provider,
                    &candidate.account.account_id,
                    provider_session_id.map(str::to_string),
                ) {
                    Ok(resolution) => {
                        return Ok(Some(ProviderAccountRoute {
                            provider,
                            account_id: resolution.account_id.clone(),
                            resume_requested_session: resolution.resume_requested_session,
                            authority: AccountRouteAuthority::Lease {
                                client: client.clone(),
                                access_token: resolution.access_token().to_string(),
                            },
                        }));
                    }
                    Err(error) => last_forwarded_error = Some(error),
                }
            }
        }
    }
    if let Some(error) = last_forwarded_error {
        return Err(error);
    }
    Err(ProviderAccountError::NoEligibleAccount {
        provider,
        accounts: "no healthy local or forwarded account remains".to_string(),
    })
}

fn push_candidate(order: &mut Vec<usize>, index: usize) {
    if !order.contains(&index) {
        order.push(index);
    }
}

fn account_unavailable_reason(account: &ProviderAccount) -> String {
    let now = now_unix();
    if account.credential_state != CredentialState::Connected {
        return format!("'{}' credential is missing", account_login(account));
    }
    let routing = account.effective_routing_state(time::OffsetDateTime::now_utc().date());
    if routing != RoutingState::Automatic {
        return format!(
            "'{}' routing is {}",
            account_login(account),
            routing.as_str()
        );
    }
    if let Some(until) = account.cooldown_until.filter(|until| *until > now) {
        return format!(
            "'{}' cooling{}",
            account_login(account),
            format_reset_time(until)
        );
    }
    format!("'{}' is unavailable", account_login(account))
}

#[derive(Debug)]
pub(crate) enum AccountMatch<'a> {
    One(&'a ProviderAccount),
    Ambiguous(Vec<&'a ProviderAccount>),
    None,
}

/// An explicit selector names an account by login email, exactly or by an
/// unambiguous prefix. Matching is case-insensitive; internal account ids are
/// stable storage keys, not a second user-facing identity.
pub(crate) fn match_account<'a>(
    accounts: &[&'a ProviderAccount],
    selector: &str,
) -> AccountMatch<'a> {
    let selector_lower = selector.to_ascii_lowercase();
    let exact = accounts.iter().copied().find(|account| {
        account
            .login_email
            .as_ref()
            .is_some_and(|email| email.as_str().eq_ignore_ascii_case(selector))
    });
    if let Some(account) = exact {
        return AccountMatch::One(account);
    }
    let prefixed: Vec<&ProviderAccount> = accounts
        .iter()
        .copied()
        .filter(|account| {
            account.login_email.as_ref().is_some_and(|email| {
                email
                    .as_str()
                    .to_ascii_lowercase()
                    .starts_with(&selector_lower)
            })
        })
        .collect();
    match prefixed.as_slice() {
        [] => AccountMatch::None,
        [account] => AccountMatch::One(account),
        _ => AccountMatch::Ambiguous(prefixed),
    }
}

pub(crate) fn current_repo_id() -> Result<Option<RepoId>, ProviderAccountError> {
    let current = std::env::current_dir()
        .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
    Ok(RepoId::discover(&current).ok())
}

pub(crate) async fn provider_route_account_ids(
    store: &SharedStore,
    repo_id: Option<&RepoId>,
    provider: Provider,
) -> Result<Option<Vec<ProviderAccountId>>, ProviderAccountError> {
    let route = match repo_id {
        Some(repo_id) => {
            store
                .provider_route(&RouteScope::Repo(repo_id.clone()), provider)
                .await?
        }
        None => None,
    };
    if let Some(route) = route {
        return Ok(Some(route.accounts));
    }
    if let Some(route) = store.provider_route(&RouteScope::Default, provider).await? {
        return Ok(Some(route.accounts));
    }

    let today = time::OffsetDateTime::now_utc().date();
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?
        .into_iter()
        .filter(|account| account.eligible_for_automatic_routing(today))
        .map(|account| account.account_id)
        .collect::<Vec<_>>();
    Ok((!accounts.is_empty()).then_some(accounts))
}

pub(crate) fn resolve_provider_account_blocking(
    provider: Provider,
    provider_session_id: Option<String>,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    _run_blocking_account(provider, "route", move |runtime| {
        runtime.block_on(resolve_provider_account(
            provider,
            provider_session_id.as_deref(),
        ))
    })
}

fn _run_blocking_account<T: Send + 'static>(
    provider: Provider,
    action: &'static str,
    operation: impl FnOnce(&tokio::runtime::Runtime) -> Result<T, ProviderAccountError> + Send + 'static,
) -> Result<T, ProviderAccountError> {
    std::thread::Builder::new()
        .name(format!("lf-{}-account-{action}", provider.as_str()))
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
            operation(&runtime)
        })
        .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?
        .join()
        .map_err(|_| ProviderAccountError::Runtime(format!("account {action} worker panicked")))?
}

async fn route_store() -> Result<Option<SharedStore>, ProviderAccountError> {
    match crate::store::open_registry_for_authority().await {
        Ok(store) => Ok(Some(Arc::new(store))),
        Err(crate::store::RegistryUnavailable::MissingFile { .. }) => Ok(None),
        Err(error) => Err(ProviderAccountError::Runtime(format!(
            "open provider account store: {error:?}"
        ))),
    }
}

pub(crate) async fn open_account_store() -> Result<SharedStore, ProviderAccountError> {
    let config = crate::store::storage_config_from_env().map_err(|error| {
        ProviderAccountError::Filesystem(format!("resolve provider account store: {error}"))
    })?;
    let store = Arc::new(open_store(&config).await?);
    Ok(store)
}

pub(crate) fn new_account(
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
    use std::collections::HashMap;
    use std::os::unix::fs::PermissionsExt;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn explicit_account_selector_matches_email_exactly_or_by_unique_prefix() {
        let temp = tempdir().unwrap();
        let accounts = [
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
                Some(crate::profile::EmailAddress::parse("manabot-eng@loopflow.studio").unwrap()),
            ),
            new_account(
                Provider::Codex,
                parse_account_id("manabot-ops").unwrap(),
                temp.path().join("manabot-ops"),
                Some(crate::profile::EmailAddress::parse("manabot-ops@loopflow.studio").unwrap()),
            ),
        ];
        let accounts = accounts.iter().collect::<Vec<_>>();
        let one = |selector: &str| match match_account(&accounts, selector) {
            AccountMatch::One(account) => account.account_id.as_str().to_string(),
            other => panic!("expected one match for '{selector}', got {other:?}"),
        };

        assert_eq!(one("LoopFlow-Eng@loopflow.studio"), "engineering");
        // A unique email prefix reaches its account.
        assert_eq!(one("manabot-e"), "manabot-eng");
        assert_eq!(one("loopflow-eng@"), "engineering");
        assert!(matches!(
            match_account(&accounts, "manabot"),
            AccountMatch::Ambiguous(candidates) if candidates.len() == 2
        ));
        assert!(matches!(
            match_account(&accounts, "engineering"),
            AccountMatch::None
        ));
        assert!(matches!(
            match_account(&accounts, "nobody@example.com"),
            AccountMatch::None
        ));
    }

    #[test]
    fn account_ids_are_derived_from_normalized_login_email() {
        let jack = crate::profile::EmailAddress::parse("Jack@Loopflow.Studio").unwrap();
        let jackstah = crate::profile::EmailAddress::parse("jackstah@gmail.com").unwrap();

        assert_eq!(account_id_for_login(&jack).as_str(), "jack-42d1021d3f2d");
        assert_eq!(
            account_id_for_login(&jackstah).as_str(),
            "jackstah-1066ea9c99d1"
        );
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
    async fn routes_clear_ambient_provider_credentials() {
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
            resume_requested_session: false,
            authority: AccountRouteAuthority::Local {
                store,
                home: temp.path().join("codex-reserve"),
            },
        };
        let mut command = Command::new("codex");
        command.env("CLAUDE_CODE_OAUTH_TOKEN", "ancestor-secret");
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

        assert_eq!(environment.get("CODEX_ACCESS_TOKEN"), Some(&None));
        assert_eq!(environment.get("OPENAI_API_KEY"), Some(&None));
        assert_eq!(
            environment
                .get("CODEX_HOME")
                .and_then(|value| value.as_deref()),
            Some(temp.path().join("codex-reserve").to_str().unwrap())
        );
        // Provider launch carries no account-selection env; descendants inherit
        // the lease handle, not a re-derivable selection.
        assert_eq!(
            environment
                .get(lease::ACCOUNT_LEASE_ENV)
                .map(Option::is_some),
            None
        );
        assert_eq!(environment.get("CLAUDE_CODE_OAUTH_TOKEN"), Some(&None));

        let mut async_command = tokio::process::Command::new("codex");
        async_command.env("CLAUDE_CODE_OAUTH_TOKEN", "ancestor-secret");
        route.apply_tokio(&mut async_command);
        assert!(async_command
            .as_std()
            .get_envs()
            .any(|(name, value)| { name == "CLAUDE_CODE_OAUTH_TOKEN" && value.is_none() }));
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
                    resume_requested_session: false,
                    authority: AccountRouteAuthority::Local {
                        store: store.clone(),
                        home: claude_home.clone(),
                    },
                },
                "CLAUDE_CONFIG_DIR",
                claude_home,
            ),
            (
                ProviderAccountRoute {
                    provider: Provider::Codex,
                    account_id: parse_account_id("reserve").unwrap(),
                    resume_requested_session: false,
                    authority: AccountRouteAuthority::Local {
                        store,
                        home: codex_home.clone(),
                    },
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
            resume_requested_session: false,
            authority: AccountRouteAuthority::Local {
                store: store.clone(),
                home: temp.path().join("primary"),
            },
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
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
            "PATH",
            lease::ACCOUNT_LEASE_ENV,
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_CONTROL_HOME", temp.path());
        std::env::remove_var("LF_CONTROL_DB_PATH");
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        std::env::remove_var(lease::ACCOUNT_LEASE_ENV);
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
            .expect("default route should select a managed account");

        assert_eq!(selection.account_id, selected.account_id);
        assert!(store
            .get_provider_account(Provider::Claude.as_str(), &selected.account_id)
            .await
            .unwrap()
            .unwrap()
            .last_selected_at
            .is_some());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn hard_limit_moves_the_route_to_the_next_account() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
            lease::ACCOUNT_LEASE_ENV,
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_CONTROL_HOME", temp.path());
        std::env::remove_var("LF_CONTROL_DB_PATH");
        std::env::remove_var(lease::ACCOUNT_LEASE_ENV);
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let first = account(Provider::Codex, "first", temp.path());
        let second = account(Provider::Codex, "second", temp.path());
        store.upsert_provider_account(&first).await.unwrap();
        store.upsert_provider_account(&second).await.unwrap();
        store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider: Provider::Codex,
                accounts: vec![first.account_id.clone(), second.account_id.clone()],
                created_at: now_unix(),
                updated_at: now_unix(),
            })
            .await
            .unwrap();

        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .expect("first routed account");
        assert_eq!(route.account_id, first.account_id);
        route
            .record_rate_limit(&RateLimitSignal {
                utilization_percent: Some(100),
                resets_at: Some(now_unix() + 300),
                limited: true,
                reason: "subscription usage limit".to_string(),
                windows: Vec::new(),
            })
            .await
            .unwrap();

        let failover = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .expect("second routed account");
        assert_eq!(failover.account_id, second.account_id);
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
            .upsert_provider_account_limits(
                Provider::Codex.as_str(),
                &second.account_id,
                &[crate::store::AccountLimitWindow {
                    window: "weekly".to_string(),
                    used_percent: 95,
                    resets_at: Some(now_unix() + 3600),
                    plan: None,
                }],
                "poll",
            )
            .await
            .unwrap();
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

    #[tokio::test]
    async fn automatic_selection_demotes_only_active_strain() {
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
        let window = |used_percent, resets_at| crate::store::AccountLimitWindow {
            window: "weekly".to_string(),
            used_percent,
            resets_at,
            plan: None,
        };
        store
            .upsert_provider_account_limits(
                Provider::Codex.as_str(),
                &first.account_id,
                &[window(95, Some(now_unix() + 3600))],
                "poll",
            )
            .await
            .unwrap();

        let selection = store
            .select_provider_account(
                Provider::Codex,
                &[first.account_id.clone(), second.account_id.clone()],
                None,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(selection.account.account_id, second.account_id);

        let only = store
            .select_provider_account(
                Provider::Codex,
                std::slice::from_ref(&first.account_id),
                None,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(only.account.account_id, first.account_id);

        // Under the bar, already reset, or never resetting: declared order stands.
        for window in [
            window(94, Some(now_unix() + 3600)),
            window(95, Some(now_unix() - 1)),
            window(95, None),
        ] {
            store
                .upsert_provider_account_limits(
                    Provider::Codex.as_str(),
                    &first.account_id,
                    &[window],
                    "poll",
                )
                .await
                .unwrap();
            let selection = store
                .select_provider_account(
                    Provider::Codex,
                    &[first.account_id.clone(), second.account_id.clone()],
                    None,
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(selection.account.account_id, first.account_id);
        }
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn missing_routes_are_unmanaged_and_unusable_routes_fail() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
            lease::ACCOUNT_LEASE_ENV,
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_CONTROL_HOME", temp.path());
        std::env::remove_var("LF_CONTROL_DB_PATH");
        std::env::remove_var(lease::ACCOUNT_LEASE_ENV);

        assert!(resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .is_none());

        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        assert!(resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .is_none());

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
            "no eligible managed claude account: 'missing@example.com' credential is missing"
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn accounts_form_an_implicit_route_when_no_route_is_configured() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
            lease::ACCOUNT_LEASE_ENV,
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_CONTROL_HOME", temp.path());
        std::env::remove_var("LF_CONTROL_DB_PATH");
        std::env::remove_var(lease::ACCOUNT_LEASE_ENV);
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let limited = account(Provider::Claude, "a-limited", temp.path());
        let healthy = account(Provider::Claude, "z-healthy", temp.path());
        store.upsert_provider_account(&limited).await.unwrap();
        store.upsert_provider_account(&healthy).await.unwrap();
        store
            .record_provider_account_health(
                Provider::Claude.as_str(),
                &limited.account_id,
                Some(100),
                Some(now_unix() + 300),
                Some("subscription usage limit"),
            )
            .await
            .unwrap();

        let route = resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .expect("a healthy managed account should be selected");

        assert_eq!(route.account_id(), &healthy.account_id);
    }
}
