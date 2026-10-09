//! Machine-local provider accounts, selection, and launch homes for Claude and Codex.

pub mod activation;
pub(crate) mod identity;
pub mod selection;

use crate::id::AgentSessionId;
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

pub(crate) fn account_route_eligible(account: &ProviderAccount, now: i64) -> bool {
    let today = time::OffsetDateTime::from_unix_timestamp(now)
        .expect("current timestamp is valid")
        .date();
    account.eligible_for_automatic_routing(today)
        && account.cooldown_until.is_none_or(|until| until <= now)
}

/// Stable sort: strained accounts fall behind unstrained ones, and declared
/// route order decides everything else.
pub(crate) fn order_accounts_by_strain(
    accounts: &mut [ProviderAccount],
    limits: &[AccountLimitRow],
    now: i64,
) {
    accounts.sort_by_key(|account| {
        (
            is_strained(&account.provider, &account.account_id, limits, now),
            plan_preference(account),
        )
    });
}

fn plan_preference(account: &ProviderAccount) -> u8 {
    match (account.provider.as_str(), account.observed_plan.as_deref()) {
        ("codex", Some("pro")) => 0,
        ("codex", Some("plus")) => 1,
        _ => 2,
    }
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
    #[error("invalid account selection: {0}")]
    AccountSelection(String),
    #[error("cannot authenticate {provider} account '{account_id}': {reason}")]
    CredentialUnavailable {
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
    /// so cached auth inspection can show capacity without a fresh poll.
    pub windows: Vec<crate::store::AccountLimitWindow>,
}

/// Where a conversation runs.
#[derive(Clone)]
enum RouteHome {
    /// In a home of the account's own.
    Isolated,
    /// In the provider's native home.
    Shared {
        cause: activation::SwitchCause,
        launched_at: i64,
    },
}

/// The catalog holding an account's login, independent of its launch home.
#[derive(Clone)]
enum AccountLogin {
    /// A profile in this Machine's catalog. A shared launch signs the native
    /// home in from it.
    Stored {
        store: SharedStore,
        profile: PathBuf,
    },
    /// A profile in a recorded capture's catalog, read to check identity. Replay
    /// names the account it requires and records nothing.
    Replayed {
        catalog: SharedStore,
        profile: PathBuf,
    },
}

#[derive(Clone)]
struct AccountCandidate {
    account: ProviderAccount,
    credential_available: bool,
    strained: bool,
    home: PathBuf,
}

#[derive(Clone)]
pub(crate) struct ProviderAccountRoute {
    provider: Provider,
    account_id: ProviderAccountId,
    resume_requested_session: bool,
    home: RouteHome,
    login: AccountLogin,
}

impl std::fmt::Debug for ProviderAccountRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let home = match self.home {
            RouteHome::Isolated => "account_home",
            RouteHome::Shared { .. } => "native_home",
        };
        f.debug_struct("ProviderAccountRoute")
            .field("provider", &self.provider)
            .field("account_id", &self.account_id)
            .field("home", &home)
            .field("resume_requested_session", &self.resume_requested_session)
            .finish()
    }
}

impl ProviderAccountRoute {
    fn shared_home(named: bool) -> RouteHome {
        RouteHome::Shared {
            // A launch that named its account switches on a person's behalf;
            // one that was routed switches because the active account ran out.
            cause: if named {
                activation::SwitchCause::Person
            } else {
                activation::SwitchCause::Exhaustion
            },
            launched_at: now_unix(),
        }
    }

    fn shared(
        provider: Provider,
        account_id: ProviderAccountId,
        store: SharedStore,
        profile: PathBuf,
        named: bool,
    ) -> Self {
        Self {
            provider,
            account_id,
            // A shared conversation is in the native home whichever account runs it.
            resume_requested_session: true,
            home: Self::shared_home(named),
            login: AccountLogin::Stored { store, profile },
        }
    }

    /// The home an isolated conversation on this account runs in.
    fn account_home(&self) -> &Path {
        match &self.login {
            AccountLogin::Stored { profile, .. } | AccountLogin::Replayed { profile, .. } => {
                profile
            }
        }
    }

    pub(crate) fn account_id(&self) -> &ProviderAccountId {
        &self.account_id
    }

    /// The account this route's usage belongs to now. A launch's account is
    /// where a shared agent began; the switch log says where a provider that
    /// follows its native login has moved it since.
    async fn used_account(&self) -> Result<ProviderAccountId, ProviderAccountError> {
        if let (RouteHome::Shared { launched_at, .. }, AccountLogin::Stored { store, .. }) =
            (&self.home, &self.login)
        {
            if activation::running_agents_follow_native_login(self.provider) {
                let moved = store
                    .provider_account_switched_since(self.provider, *launched_at)
                    .await?;
                if let Some(account_id) = moved {
                    return Ok(account_id);
                }
            }
        }
        Ok(self.account_id.clone())
    }

    pub(crate) fn resume_requested_session(&self) -> bool {
        self.resume_requested_session
    }

    #[cfg(test)]
    pub(crate) fn is_shared(&self) -> bool {
        matches!(self.home, RouteHome::Shared { .. })
    }

    /// The home holding this route's live credential. A shared route's is the
    /// native home while its account is active there; an isolated conversation
    /// runs on its profile's own copy.
    fn credential_home(&self, profile: &Path) -> PathBuf {
        match self.home {
            RouteHome::Shared { .. } => activation::credential_home(self.provider, profile),
            RouteHome::Isolated => profile.to_path_buf(),
        }
    }

    /// Provider arguments that must precede the subcommand. An isolated Codex
    /// home keeps its login in a file whatever the mirrored config selects.
    pub(crate) fn provider_args(&self) -> &'static [&'static str] {
        match (self.provider, &self.home) {
            (Provider::Codex, RouteHome::Isolated) => {
                &["-c", "cli_auth_credentials_store=\"file\""]
            }
            _ => &[],
        }
    }

    /// Prove that this route can authenticate before durable Work is reserved.
    ///
    /// The token is deliberately consumed here and never returned: Task launch
    /// needs evidence that the configured authority is usable, not another
    /// secret-bearing representation to persist or log.
    pub(crate) async fn verify_ready(&self) -> Result<(), ProviderAccountError> {
        self.check_identity().await?;
        let home = self.credential_home(self.account_home());
        crate::provider_auth::prepare_provider_account_access_token(self.provider, &home)
            .await
            .map_err(|error| ProviderAccountError::CredentialUnavailable {
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
        self.check_identity().await?;
        Ok(())
    }

    async fn check_identity(&self) -> Result<(), ProviderAccountError> {
        let (store, profile) = match &self.login {
            AccountLogin::Stored { store, profile }
            | AccountLogin::Replayed {
                catalog: store,
                profile,
            } => (store, profile),
        };
        let accounts = store
            .list_provider_accounts(Some(self.provider.as_str()))
            .await?;
        let account = accounts
            .iter()
            .find(|account| account.account_id == self.account_id)
            .ok_or_else(|| ProviderAccountError::Runtime("selected account disappeared".into()))?;
        let mut selected = account.clone();
        selected.home = Some(self.credential_home(profile));
        identity::check_current_identity(&selected, &accounts)
            .await
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()))
    }

    fn apply(&self, command: &mut Command) {
        for name in PROVIDER_CREDENTIAL_ENV_VARS {
            // A shared launch keeps the home its caller chose. A home inherited
            // from an isolated parent is cleared, so the provider falls back to
            // its native one.
            let kept_home = matches!(self.home, RouteHome::Shared { .. })
                && matches!(name, "CODEX_HOME" | "CLAUDE_CONFIG_DIR")
                && !launch_env(command, name)
                    .is_some_and(|home| activation::is_account_home(Path::new(&home)));
            if !kept_home {
                command.env_remove(name);
            }
        }
        if let (RouteHome::Isolated, Some(name)) = (&self.home, activation::home_env(self.provider))
        {
            command.env(name, self.account_home());
        }
    }

    /// The one place a launch takes on an account. Every route configures
    /// `command`; a shared route on a stored login first signs the native home
    /// in as its account, doing nothing when that account is already active.
    /// Hold the returned lock until the child has spawned.
    pub(crate) async fn launch_as(
        &self,
        command: &mut Command,
    ) -> Result<Option<fs::File>, ProviderAccountError> {
        self.apply(command);
        let (RouteHome::Shared { cause, .. }, AccountLogin::Stored { store, .. }) =
            (&self.home, &self.login)
        else {
            return Ok(None);
        };
        let launch_home =
            activation::home_env(self.provider).and_then(|name| launch_env(command, name));
        let native = activation::native_home(self.provider, launch_home.as_deref());
        activation::activate(store, self.provider, &self.account_id, &native, *cause).await
    }

    pub(crate) fn launch_as_blocking(
        &self,
        command: &mut Command,
    ) -> Result<Option<fs::File>, ProviderAccountError> {
        _run_blocking_account(self.provider, "activate", |runtime| {
            runtime.block_on(self.launch_as(command))
        })
    }

    pub(crate) async fn pin_session(
        &self,
        agent_session: &AgentSessionId,
    ) -> Result<(), ProviderAccountError> {
        match &self.login {
            AccountLogin::Stored { store, .. } => {
                store
                    .pin_provider_session_route(
                        self.provider,
                        agent_session,
                        &self.account_id,
                        matches!(self.home, RouteHome::Isolated),
                    )
                    .await?;
            }
            AccountLogin::Replayed { .. } => {}
        }
        Ok(())
    }

    pub(crate) async fn record_rate_limit(
        &self,
        signal: &RateLimitSignal,
    ) -> Result<(), ProviderAccountError> {
        match &self.login {
            AccountLogin::Stored { store, .. } => {
                let account_id = self.used_account().await?;
                record_rate_limit_signal(store, self.provider, &account_id, signal).await?;
            }
            AccountLogin::Replayed { .. } => {}
        }
        Ok(())
    }

    pub(crate) fn record_process_blocking(
        &self,
        agent_session: Option<AgentSessionId>,
        signal: Option<RateLimitSignal>,
    ) -> Result<(), ProviderAccountError> {
        if agent_session.is_none() && signal.is_none() {
            return Ok(());
        }
        _run_blocking_account(self.provider, "record", move |runtime| {
            runtime.block_on(async {
                if let Some(signal) = signal {
                    self.record_rate_limit(&signal).await?;
                }
                if let Some(agent_session) = agent_session {
                    if let Err(error) = self.pin_session(&agent_session).await {
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
        _run_blocking_account(self.provider, "invalidate", move |runtime| {
            runtime.block_on(async {
                match &self.login {
                    AccountLogin::Stored { store, .. } => {
                        store
                            .record_provider_account_credential_invalidated(
                                self.provider.as_str(),
                                &self.used_account().await?,
                                reason,
                            )
                            .await?
                    }
                    AccountLogin::Replayed { .. } => {}
                }
                Ok(())
            })
        })
    }
}

/// The value a launch sets for `name`, else what it inherits.
fn launch_env(command: &Command, name: &str) -> Option<std::ffi::OsString> {
    match command
        .get_envs()
        .find(|(key, _)| *key == std::ffi::OsStr::new(name))
    {
        Some((_, value)) => value.map(std::ffi::OsString::from),
        None => std::env::var_os(name),
    }
}

/// Record a provider rate-limit signal against a store: health row plus any
/// observed limit windows.
async fn record_rate_limit_signal(
    store: &SharedStore,
    provider: Provider,
    account_id: &ProviderAccountId,
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
            .upsert_provider_account_limits(
                provider.as_str(),
                account_id,
                &signal.windows,
                "stream",
            )
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

pub(crate) fn acquire_managed_login_lock(
    account_home: &Path,
    provider: Provider,
    account_id: &ProviderAccountId,
) -> anyhow::Result<fs::File> {
    let parent = account_home
        .parent()
        .ok_or_else(|| anyhow::anyhow!("account home has no parent directory"))?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(parent.join(format!(".{}.login.lock", account_id.as_str())))
        .map_err(|_| anyhow::anyhow!("open managed login lock failed"))?;
    fs2::FileExt::try_lock_exclusive(&lock).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            anyhow::anyhow!(
                "another {} credential operation is already in progress for account '{}'",
                provider.display_name(),
                account_id
            )
        } else {
            anyhow::anyhow!(
                "could not lock {} account '{}' for credential update: {error}",
                provider.display_name(),
                account_id
            )
        }
    })?;
    Ok(lock)
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

#[cfg(test)]
pub(crate) async fn resolve_provider_account(
    provider: Provider,
    agent_session: Option<&AgentSessionId>,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    resolve_provider_account_exact(provider, agent_session, None).await
}

pub(crate) async fn resolve_provider_account_exact(
    provider: Provider,
    agent_session: Option<&AgentSessionId>,
    exact_account_id: Option<&ProviderAccountId>,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    ensure_supported(provider)?;
    let store = route_store().await?;
    // A conversation resumes in the home it started in; a new one follows the
    // launch's mode.
    let recorded = match (&store, agent_session) {
        (Some(store), Some(session_id)) => {
            store
                .provider_session_isolated(provider, session_id)
                .await?
        }
        _ => None,
    };
    let isolated = recorded.unwrap_or_else(activation::launch_isolated);
    // A shared conversation resumes under whichever account is active; the
    // account it began under is history, not a pin.
    let exact_account_id = exact_account_id.filter(|_| recorded != Some(false));
    if !selection::AccountSelection::from_env()?.is_default() {
        return resolve_selected_provider_account(
            provider,
            agent_session,
            exact_account_id,
            store,
            isolated,
        )
        .await;
    }
    let Some(store) = store else {
        if let Some(account_id) = exact_account_id {
            return Err(ProviderAccountError::NoEligibleAccount {
                provider,
                accounts: format!("'{account_id}' is not available on this Machine"),
            });
        }
        return Ok(None);
    };

    let routed_repo_id = current_repo_id()?;
    let Some(candidates) =
        provider_route_account_ids(&store, routed_repo_id.as_ref(), provider).await?
    else {
        if let Some(account_id) = exact_account_id {
            return Err(ProviderAccountError::NoEligibleAccount {
                provider,
                accounts: format!("'{account_id}' is outside the configured account route"),
            });
        }
        return Ok(None);
    };

    if candidates.is_empty() {
        if let Some(account_id) = exact_account_id {
            return Err(ProviderAccountError::NoEligibleAccount {
                provider,
                accounts: format!("'{account_id}' is outside the configured account route"),
            });
        }
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
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let mut rejected = Vec::new();
    let mut eligible = Vec::new();
    for id in &candidates {
        let Some(account) = accounts.iter().find(|account| account.account_id == *id) else {
            continue;
        };
        match identity::check_current_identity(account, &accounts).await {
            Ok(()) => eligible.push(id.clone()),
            Err(error) => {
                let reason = error.to_string();
                tracing::warn!("skipping managed account: {reason}");
                rejected.push(reason);
            }
        }
    }
    if eligible.is_empty() && !rejected.is_empty() {
        return Err(ProviderAccountError::NoEligibleAccount {
            provider,
            accounts: rejected.join("; "),
        });
    }
    let selection = if isolated {
        store
            .select_provider_account(provider, &eligible, agent_session)
            .await?
    } else {
        select_shared_account(&store, provider, &eligible).await?
    };
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
    if !isolated {
        return Ok(Some(ProviderAccountRoute::shared(
            provider,
            account_id,
            store,
            home,
            exact_account_id.is_some(),
        )));
    }
    Ok(Some(ProviderAccountRoute {
        provider,
        account_id,
        resume_requested_session: selection.resume_requested_session,
        home: RouteHome::Isolated,
        login: AccountLogin::Stored {
            store,
            profile: home,
        },
    }))
}

/// Shared routing stays on the active account while it is eligible and
/// unstrained, and otherwise moves to the next eligible account.
async fn select_shared_account(
    store: &SharedStore,
    provider: Provider,
    eligible: &[ProviderAccountId],
) -> Result<Option<crate::store::ProviderAccountSelection>, ProviderAccountError> {
    let native = activation::native_home(provider, None);
    let active = activation::observe_active_account(store, provider, &native)
        .await?
        .filter(|account_id| eligible.contains(account_id));
    if let Some(active) = &active {
        let limits = store
            .provider_account_limits(Some(provider.as_str()))
            .await?;
        if !is_strained(provider.as_str(), active, &limits, now_unix()) {
            let kept = store
                .select_provider_account(provider, std::slice::from_ref(active), None)
                .await?;
            if kept.is_some() {
                return Ok(kept);
            }
        }
    }
    Ok(store
        .select_provider_account(provider, eligible, None)
        .await?)
}

async fn resolve_selected_provider_account(
    provider: Provider,
    agent_session: Option<&AgentSessionId>,
    exact_account_id: Option<&ProviderAccountId>,
    local_store: Option<SharedStore>,
    isolated: bool,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    let repo_id = current_repo_id()?;
    let Some(mut candidates) = ordered_selected_candidates(
        provider,
        agent_session,
        exact_account_id,
        repo_id.as_ref(),
        local_store.as_ref(),
        true,
    )
    .await?
    else {
        return Ok(None);
    };
    let store = local_store
        .as_ref()
        .expect("candidates belong to the machine store");
    if !isolated {
        let native = activation::native_home(provider, None);
        let active = activation::observe_active_account(store, provider, &native).await?;
        if let Some(index) = active.and_then(|active| active_candidate(&active, &candidates)) {
            let active = candidates.remove(index);
            candidates.insert(0, active);
        }
    }
    for (candidate, explicit) in candidates {
        let home = &candidate.home;
        if !explicit
            && store
                .select_provider_account(
                    provider,
                    std::slice::from_ref(&candidate.account.account_id),
                    agent_session,
                )
                .await?
                .is_none()
        {
            continue;
        }
        let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        ensure_account_home_at(&operator_home, home, provider)?;
        let resumed = match agent_session {
            Some(session_id) => store
                .provider_session_account(provider, session_id)
                .await?
                .is_some_and(|account_id| account_id == candidate.account.account_id),
            None => false,
        };
        if !isolated {
            return Ok(Some(ProviderAccountRoute::shared(
                provider,
                candidate.account.account_id.clone(),
                Arc::clone(store),
                home.clone(),
                explicit,
            )));
        }
        return Ok(Some(ProviderAccountRoute {
            provider,
            account_id: candidate.account.account_id.clone(),
            resume_requested_session: resumed,
            home: RouteHome::Isolated,
            login: AccountLogin::Stored {
                store: Arc::clone(store),
                profile: home.clone(),
            },
        }));
    }

    Err(ProviderAccountError::NoEligibleAccount {
        provider,
        accounts: "no healthy selected account remains".to_string(),
    })
}

/// A shared launch stays on the active account where its own choice allows:
/// among the accounts it named, or among all of them when it named none.
fn active_candidate(
    active: &ProviderAccountId,
    candidates: &[(AccountCandidate, bool)],
) -> Option<usize> {
    let named = candidates.iter().any(|(_, explicit)| *explicit);
    candidates.iter().position(|(candidate, explicit)| {
        *explicit == named
            && candidate.credential_available
            && !candidate.strained
            && candidate.account.account_id == *active
    })
}

async fn ordered_selected_candidates(
    provider: Provider,
    agent_session: Option<&AgentSessionId>,
    exact_account_id: Option<&ProviderAccountId>,
    repo_id: Option<&RepoId>,
    local_store: Option<&SharedStore>,
    verify_identity: bool,
) -> Result<Option<Vec<(AccountCandidate, bool)>>, ProviderAccountError> {
    let mut catalog = match &local_store {
        Some(store) => store.list_provider_accounts(None).await?,
        None => Vec::new(),
    };
    let mut candidates = Vec::new();
    let mut local_route = Vec::new();
    if let Some(store) = &local_store {
        local_route = provider_route_account_ids(store, repo_id, provider)
            .await?
            .unwrap_or_default();
        let limits = store
            .provider_account_limits(Some(provider.as_str()))
            .await?;
        let now = now_unix();
        for account in catalog
            .iter()
            .filter(|account| account.provider == provider.as_str())
        {
            let identity = if verify_identity {
                identity::check_current_identity(account, &catalog)
                    .await
                    .map_err(|error| error.to_string())
            } else {
                identity::check_account_identity(account, &catalog)
            };
            let identity_matches = match identity {
                Ok(()) => true,
                Err(reason) => {
                    tracing::warn!("skipping managed account: {reason}");
                    false
                }
            };
            let Some(home) = account.home.clone() else {
                continue;
            };
            candidates.push(AccountCandidate {
                credential_available: account.credential_state == CredentialState::Connected
                    && identity_matches,
                account: account.clone(),
                strained: active_account_strain(
                    provider.as_str(),
                    &account.account_id,
                    &limits,
                    now,
                )
                .is_some(),
                home,
            });
        }
    }
    catalog.retain(|account| account.home.is_some());
    let selection = selection::AccountSelection::from_env()?;
    let selected = selection.resolved_accounts(&catalog)?;
    if candidates.is_empty() {
        if selection.is_restricted() || exact_account_id.is_some() {
            return Err(ProviderAccountError::NoEligibleAccount {
                provider,
                accounts: "the restricted account selection excludes this provider".to_string(),
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
        for account_id in &local_route {
            if let Some(index) = candidates
                .iter()
                .position(|candidate| candidate.account.account_id == *account_id)
            {
                push_candidate(&mut order, index);
            }
        }

        for index in 0..candidates.len() {
            push_candidate(&mut order, index);
        }
    }

    if let Some(account_id) = exact_account_id {
        order.retain(|index| candidates[*index].account.account_id == *account_id);
    }
    if order.is_empty() {
        return Err(ProviderAccountError::NoEligibleAccount {
            provider,
            accounts: "the account selection is empty".to_string(),
        });
    }

    if let Some(session_id) = agent_session {
        let local_pin = match &local_store {
            Some(store) => store.provider_session_account(provider, session_id).await?,
            None => None,
        };
        if let Some(index) = order.iter().position(|index| {
            let candidate = &candidates[*index];
            local_pin
                .as_ref()
                .is_some_and(|account_id| *account_id == candidate.account.account_id)
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
    order[preferred_count..].sort_by_key(|index| {
        (
            candidates[*index].strained,
            plan_preference(&candidates[*index].account),
        )
    });
    Ok(Some(
        order
            .into_iter()
            .enumerate()
            .filter_map(|(position, index)| {
                let candidate = &candidates[index];
                let explicit = position < preferred_count;
                (candidate.credential_available
                    && (explicit || account_route_eligible(&candidate.account, now)))
                .then(|| (candidate.clone(), explicit))
            })
            .collect(),
    ))
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
        (account.login_email.is_none() && account.account_id.as_str() == selector)
            || account
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

/// Read the launch candidate order without selecting an account or acquiring credentials.
pub(crate) async fn inspect_provider_route(
    store: Option<&SharedStore>,
    repo_id: Option<&RepoId>,
    provider: Provider,
) -> Result<Option<Vec<ProviderAccount>>, ProviderAccountError> {
    if !selection::AccountSelection::from_env()?.is_default() {
        return Ok(
            ordered_selected_candidates(provider, None, None, repo_id, store, false)
                .await?
                .map(|candidates| {
                    candidates
                        .into_iter()
                        .map(|(candidate, _)| candidate.account)
                        .collect()
                }),
        );
    }
    let Some(store) = store else {
        return Ok(None);
    };
    let Some(ids) = provider_route_account_ids(store, repo_id, provider).await? else {
        return Ok(None);
    };
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let now = now_unix();
    let mut available = ids
        .iter()
        .filter_map(|id| accounts.iter().find(|a| a.account_id == *id))
        .filter(|account| account_route_eligible(account, now))
        .filter(
            |account| match identity::check_account_identity(account, &accounts) {
                Ok(()) => true,
                Err(reason) => {
                    tracing::warn!("skipping managed account: {reason}");
                    false
                }
            },
        )
        .cloned()
        .collect::<Vec<_>>();
    order_accounts_by_strain(
        &mut available,
        &store
            .provider_account_limits(Some(provider.as_str()))
            .await?,
        now,
    );
    Ok(Some(available))
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

pub(crate) fn resolve_provider_account_exact_blocking(
    provider: Provider,
    agent_session: Option<AgentSessionId>,
    exact_account_id: Option<ProviderAccountId>,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    _run_blocking_account(provider, "route", move |runtime| {
        runtime.block_on(resolve_provider_account_exact(
            provider,
            agent_session.as_ref(),
            exact_account_id.as_ref(),
        ))
    })
}

/// Resolve one recorded account without consulting current planning routes.
///
/// The account catalog on the recorded Machine is the authority. This path deliberately does not apply current repository
/// routing or account-health policy: replay names the account it requires.
/// Both providers read the owning account catalog to check credential identity.
pub(crate) fn resolve_recorded_provider_account_blocking(
    provider: Provider,
    account_id: ProviderAccountId,
    lf_home: PathBuf,
) -> Result<Option<ProviderAccountRoute>, ProviderAccountError> {
    _run_blocking_account(provider, "recorded-route", move |runtime| {
        runtime.block_on(async move {
            ensure_supported(provider)?;

            let home = lf_home
                .join("accounts")
                .join(provider.as_str())
                .join(account_id.as_str());
            if !home.is_dir() {
                return Err(ProviderAccountError::NoEligibleAccount {
                    provider,
                    accounts: format!(
                        "'{account_id}' has no credential Machine at {}",
                        home.display()
                    ),
                });
            }
            let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            ensure_account_home_at(&operator_home, &home, provider)?;
            let catalog = if lf_home == crate::store::lf_home_dir() {
                crate::store::database_path_from_env()
                    .map_err(|error| ProviderAccountError::Filesystem(error.to_string()))?
            } else {
                lf_home.join("loopflow.db")
            };
            let catalog = Arc::new(crate::store::Store {
                sqlite: crate::store::sqlite::SqliteStore::open_read_only(&catalog)?,
            });
            let route = ProviderAccountRoute {
                provider,
                account_id,
                resume_requested_session: false,
                home: RouteHome::Isolated,
                login: AccountLogin::Replayed {
                    catalog,
                    profile: home,
                },
            };
            route.verify_ready().await?;
            Ok(Some(route))
        })
    })
}

fn _run_blocking_account<T: Send>(
    provider: Provider,
    action: &'static str,
    operation: impl FnOnce(&tokio::runtime::Runtime) -> Result<T, ProviderAccountError> + Send,
) -> Result<T, ProviderAccountError> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name(format!("lf-{}-account-{action}", provider.as_str()))
            .spawn_scoped(scope, move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
                operation(&runtime)
            })
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?
            .join()
            .map_err(|_| {
                ProviderAccountError::Runtime(format!("account {action} worker panicked"))
            })?
    })
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

pub(crate) fn read_account_store() -> Result<Option<SharedStore>, ProviderAccountError> {
    let crate::store::StorageConfig::Sqlite { path } = crate::store::storage_config_from_env()
        .map_err(|error| ProviderAccountError::Filesystem(error.to_string()))?;
    match path.try_exists() {
        Ok(false) => Ok(None),
        Ok(true) => Ok(Some(Arc::new(crate::store::Store {
            sqlite: crate::store::sqlite::SqliteStore::open_read_only(&path)?,
        }))),
        Err(error) => Err(ProviderAccountError::Filesystem(error.to_string())),
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
        observed_email: None,
        observed_subject: None,
        observed_credential_digest: None,
        observed_plan: None,
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
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let route = ProviderAccountRoute {
            provider: Provider::Codex,
            account_id: parse_account_id("reserve").unwrap(),
            resume_requested_session: false,
            home: RouteHome::Isolated,
            login: AccountLogin::Stored {
                store,
                profile: temp.path().join("codex-reserve"),
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
        assert_eq!(environment.get("CLAUDE_CODE_OAUTH_TOKEN"), Some(&None));

        let mut async_command = tokio::process::Command::new("codex");
        async_command.env("CLAUDE_CODE_OAUTH_TOKEN", "ancestor-secret");
        route.apply(async_command.as_std_mut());
        assert!(async_command
            .as_std()
            .get_envs()
            .any(|(name, value)| { name == "CLAUDE_CODE_OAUTH_TOKEN" && value.is_none() }));
    }

    #[tokio::test]
    async fn native_routes_select_independent_provider_homes() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
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
                    home: RouteHome::Isolated,
                    login: AccountLogin::Stored {
                        store: store.clone(),
                        profile: claude_home.clone(),
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
                    home: RouteHome::Isolated,
                    login: AccountLogin::Stored {
                        store,
                        profile: codex_home.clone(),
                    },
                },
                "CODEX_HOME",
                codex_home,
            ),
        ];

        for (route, env_name, expected_machine) in routes {
            let mut command = Command::new(route.provider.as_str());
            route.apply(&mut command);
            let selected_home = command
                .get_envs()
                .find(|(name, _)| *name == std::ffi::OsStr::new(env_name))
                .and_then(|(_, value)| value)
                .map(PathBuf::from);
            assert_eq!(selected_home.as_deref(), Some(expected_machine.as_path()));
        }
    }

    #[tokio::test]
    async fn hard_rate_limit_cools_the_active_account() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
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
            home: RouteHome::Isolated,
            login: AccountLogin::Stored {
                store: store.clone(),
                profile: temp.path().join("primary"),
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

    use base64::Engine;
    use tempfile::tempdir;

    use super::*;
    use crate::profile::{ProviderRoute, RouteScope};
    use crate::store::StorageConfig;

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
        if provider == Provider::Codex {
            let account_home = home.join(account_id);
            fs::create_dir_all(&account_home).unwrap();
            let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
                serde_json::json!({"email":format!("{account_id}@example.com"), "sub":account_id})
                    .to_string(),
            );
            fs::write(account_home.join("auth.json"), serde_json::json!({"tokens":{"access_token":"fixture", "id_token":format!("h.{claims}.s")}}).to_string()).unwrap();
        }
        let mut account = new_account(
            provider,
            parse_account_id(account_id).unwrap(),
            home.join(account_id),
            Some(
                crate::profile::EmailAddress::parse(&format!("{account_id}@example.com")).unwrap(),
            ),
        );
        if provider == Provider::Claude {
            identity::tests::write_claude_identity(&mut account);
        }
        account
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn codex_routing_skips_mismatched_identity_and_readiness_rechecks_it() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            temp.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        let first = account(
            Provider::Codex,
            "first",
            &temp.path().join("accounts/codex"),
        );
        let second = account(
            Provider::Codex,
            "second",
            &temp.path().join("accounts/codex"),
        );
        let write = |account: &ProviderAccount, email: &str| {
            let home = account.home.as_ref().unwrap();
            fs::create_dir_all(home).unwrap();
            let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::json!({"email":email, "sub":email}).to_string());
            fs::write(home.join("auth.json"), serde_json::json!({"tokens":{"access_token":"fixture", "id_token":format!("h.{claims}.s")}}).to_string()).unwrap();
        };
        write(&first, "wrong@example.com");
        write(&second, "second@example.com");
        for account in [&first, &second] {
            store.upsert_provider_account(account).await.unwrap();
        }
        store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider: Provider::Codex,
                accounts: vec![first.account_id.clone(), second.account_id.clone()],
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();
        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(route.account_id(), &second.account_id);
        assert_eq!(
            store
                .get_provider_account("codex", &first.account_id)
                .await
                .unwrap()
                .unwrap()
                .last_selected_at,
            None
        );
        write(&second, "replacement@example.com");
        let error = route.verify_ready().await.unwrap_err().to_string();
        assert!(error.contains("replacement@example.com") && error.contains("second@example.com"));
        let error = resolve_recorded_provider_account_blocking(
            Provider::Codex,
            second.account_id.clone(),
            temp.path().to_path_buf(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("replacement@example.com") && error.contains("second@example.com"));
        let error = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("first")
                && error.contains("second")
                && error.contains("lf account connect")
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn claude_routing_refreshes_changed_identity_and_rechecks_readiness() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            temp.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        let first = account(
            Provider::Claude,
            "first",
            &temp.path().join("accounts/claude"),
        );
        let second = account(
            Provider::Claude,
            "second",
            &temp.path().join("accounts/claude"),
        );
        for account in [&first, &second] {
            store.upsert_provider_account(account).await.unwrap();
        }
        let path = first.home.as_ref().unwrap().join(".credentials.json");
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, original.replace("fixture-first", "replacement")).unwrap();
        let profile = |email: &str, subject: &str| {
            serde_json::json!({"account":{"email":email,"uuid":subject}}).to_string()
        };
        // Both candidate checks inspect the changed account: it is mismatched,
        // but distinct from the healthy second login.
        let (_endpoints, server) = crate::subscription::observation_tests::serve(
            vec![
                (200, profile("wrong@example.com", "wrong")),
                (200, profile("wrong@example.com", "wrong")),
            ],
            |_| {},
        )
        .await;
        let route = resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .unwrap();
        server.await.unwrap();
        assert_eq!(route.account_id(), &second.account_id);
        assert_eq!(
            store
                .get_provider_account("claude", &first.account_id)
                .await
                .unwrap()
                .unwrap()
                .last_selected_at,
            None
        );

        // The selected account is replaced before spawn. Readiness must not
        // accept its earlier observation, even with the expected email.
        fs::write(&path, &original).unwrap();
        let second_path = second.home.as_ref().unwrap().join(".credentials.json");
        let second_original = fs::read_to_string(&second_path).unwrap();
        fs::write(
            &second_path,
            second_original.replace("fixture-second", "changed"),
        )
        .unwrap();
        let (_endpoints, server) = crate::subscription::observation_tests::serve(
            vec![(200, profile("second@example.com", "different-user"))],
            |_| {},
        )
        .await;
        let error = route.verify_ready().await.unwrap_err().to_string();
        server.await.unwrap();
        assert!(error.contains("different credential user"), "{error}");

        // A normal native refresh of the same user remains launchable.
        let (_endpoints, server) = crate::subscription::observation_tests::serve(
            vec![
                (200, profile("second@example.com", "second")),
                (200, profile("second@example.com", "second")),
            ],
            |_| {},
        )
        .await;
        route.verify_ready().await.unwrap();
        server.await.unwrap();
        fs::write(&second_path, &second_original).unwrap();

        // Distinct tokens reporting one user are duplicates; copied tokens also
        // fail the offline check without requiring a saved destination binding.
        fs::write(&path, original.replace("fixture-first", "duplicate")).unwrap();
        let (_endpoints, server) = crate::subscription::observation_tests::serve(
            vec![(200, profile("second@example.com", "second"))],
            |_| {},
        )
        .await;
        let error = identity::check_current_identity(&second, &[first.clone(), second.clone()])
            .await
            .unwrap_err()
            .to_string();
        server.await.unwrap();
        assert!(error.contains("share login"), "{error}");
        fs::write(&path, &second_original).unwrap();
        assert!(
            identity::check_account_identity(&second, &[first, second.clone()])
                .unwrap_err()
                .contains("share login")
        );
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
        let _restore = EnvRestore::capture(&["LF_HOME", "PATH"]);
        std::env::set_var("LF_HOME", temp.path());
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
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
    async fn blocking_pin_honors_exact_account_without_changing_dynamic_selection() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let first = account(Provider::Codex, "first", temp.path());
        let requested = account(Provider::Codex, "requested", temp.path());
        store.upsert_provider_account(&first).await.unwrap();
        store.upsert_provider_account(&requested).await.unwrap();
        store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider: Provider::Codex,
                accounts: vec![first.account_id.clone(), requested.account_id.clone()],
                created_at: now_unix(),
                updated_at: now_unix(),
            })
            .await
            .unwrap();

        let mut dynamic = crate::engine::AgentConfig {
            agent: Some("codex".to_string()),
            ..crate::engine::AgentConfig::default()
        };
        crate::engine::agent::pin_provider_account_id_blocking(&mut dynamic).unwrap();
        assert_eq!(dynamic.provider_account_id, Some(first.account_id));

        let mut exact = crate::engine::AgentConfig {
            agent: Some("codex".to_string()),
            provider_account_id: Some(requested.account_id.clone()),
            ..crate::engine::AgentConfig::default()
        };
        crate::engine::agent::pin_provider_account_id_blocking(&mut exact).unwrap();
        assert_eq!(exact.provider_account_id, Some(requested.account_id));
    }

    #[test]
    fn recorded_account_requires_its_owning_identity_catalog() {
        let _lock = crate::journal::test_env_lock();
        let home = tempdir().unwrap();
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", home.path());
        let registry = home.path().join("loopflow.db");
        fs::create_dir(&registry).unwrap();

        let account_id = parse_account_id("recorded").unwrap();
        let account_home = home.path().join("accounts/claude/recorded");
        fs::create_dir_all(&account_home).unwrap();
        fs::write(
            account_home.join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"test-token","expiresAt":4102444800000}}"#,
        )
        .unwrap();

        let route = resolve_recorded_provider_account_blocking(
            Provider::Claude,
            account_id.clone(),
            home.path().to_path_buf(),
        )
        .unwrap_err();

        assert!(matches!(route, ProviderAccountError::Store(_)), "{route}");
        assert!(registry.is_dir());
        fs::remove_dir(&registry).unwrap();

        let owning_database = registry;
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(owning_database))
                .await
                .unwrap();
            let verified = account(
                Provider::Claude,
                "recorded",
                &home.path().join("accounts/claude"),
            );
            store.upsert_provider_account(&verified).await.unwrap();
        });
        let route = resolve_recorded_provider_account_blocking(
            Provider::Claude,
            account_id.clone(),
            home.path().to_path_buf(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(route.account_id(), &account_id);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn hard_limit_moves_the_route_to_the_next_account() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
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
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("store.db"),
            ))
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
            .pin_provider_session_route(
                Provider::Codex,
                &"session".into(),
                &second.account_id,
                true,
            )
            .await
            .unwrap();

        let selection = store
            .select_provider_account(
                Provider::Codex,
                &[first.account_id, second.account_id.clone()],
                Some(&"session".into()),
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
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("store.db"),
            ))
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
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());

        assert!(resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .is_none());

        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
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
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                temp.path().join("loopflow.db"),
            ))
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

    const SHARED_ENV: [&str; 5] = [
        "LF_HOME",
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
        "LF_TEST_CLAUDE_PROFILE_URL",
        activation::ACCOUNT_ISOLATION_ENV,
    ];

    fn set_isolation(isolate: bool) {
        let (name, mode) = activation::isolation_env(isolate);
        std::env::set_var(name, mode);
    }

    /// A Machine with `provider` accounts `first` and `second` on the default
    /// route and a native home signed in as `active`; `native` is a login no
    /// stored account holds.
    async fn shared_home(provider: Provider, temp: &Path, active: &str) -> (SharedStore, PathBuf) {
        std::env::set_var("LF_HOME", temp);
        std::env::remove_var(activation::ACCOUNT_ISOLATION_ENV);
        let native = temp.join("native");
        fs::create_dir_all(&native).unwrap();
        std::env::set_var(activation::home_env(provider).unwrap(), &native);
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(temp.join("loopflow.db")))
                .await
                .unwrap(),
        );
        let mut route = Vec::new();
        for account_id in ["first", "second"] {
            let stored = account(provider, account_id, temp);
            store.upsert_provider_account(&stored).await.unwrap();
            route.push(stored.account_id);
        }
        store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider,
                accounts: route,
                created_at: now_unix(),
                updated_at: now_unix(),
            })
            .await
            .unwrap();
        if active == "native" {
            account(provider, "native", temp);
        } else {
            let login = match provider {
                Provider::Claude => ".credentials.json",
                _ => "auth.json",
            };
            fs::copy(temp.join(active).join(login), native.join(login)).unwrap();
        }
        (store, native)
    }

    fn codex_login(home: &Path) -> String {
        crate::provider_auth::codex_identity_from_home(home)
            .unwrap()
            .email
    }

    async fn activate(
        store: &SharedStore,
        provider: Provider,
        account_id: &str,
        native: &Path,
    ) -> bool {
        activation::activate(
            store,
            provider,
            &parse_account_id(account_id).unwrap(),
            native,
            activation::SwitchCause::Person,
        )
        .await
        .unwrap()
        .is_some()
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn switching_away_and_back_keeps_a_login_the_provider_rotated() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Codex, temp.path(), "first").await;
        // The provider refreshed the active login in place.
        let rotated = fs::read_to_string(native.join("auth.json"))
            .unwrap()
            .replace("fixture", "rotated");
        fs::write(native.join("auth.json"), &rotated).unwrap();

        assert!(!activate(&store, Provider::Codex, "first", &native).await);
        assert!(activate(&store, Provider::Codex, "second", &native).await);
        assert_eq!(codex_login(&native), "second@example.com");
        assert_eq!(
            fs::read_to_string(temp.path().join("first/auth.json")).unwrap(),
            rotated
        );

        assert!(activate(&store, Provider::Codex, "first", &native).await);
        assert_eq!(
            fs::read_to_string(native.join("auth.json")).unwrap(),
            rotated
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn the_active_account_is_probed_in_the_native_home() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (_store, native) = shared_home(Provider::Codex, temp.path(), "first").await;

        let probed = |account: &str| {
            activation::credential_home(Provider::Codex, &temp.path().join(account))
        };
        assert_eq!(probed("first"), native);
        assert_eq!(probed("second"), temp.path().join("second"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn an_unknown_native_login_is_kept_as_a_new_profile() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Codex, temp.path(), "native").await;
        let stranger = fs::read_to_string(native.join("auth.json")).unwrap();

        assert!(activate(&store, Provider::Codex, "first", &native).await);

        let kept = store
            .list_provider_accounts(Some("codex"))
            .await
            .unwrap()
            .into_iter()
            .find(|account| account_login(account) == "native@example.com")
            .expect("the unknown login became a stored profile");
        assert_eq!(kept.routing_state, RoutingState::ExplicitOnly);
        assert_eq!(
            fs::read_to_string(kept.home.unwrap().join("auth.json")).unwrap(),
            stranger
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn a_keyring_native_login_is_refused_untouched() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Codex, temp.path(), "first").await;
        fs::write(
            native.join("config.toml"),
            "cli_auth_credentials_store = \"keyring\"\n",
        )
        .unwrap();

        let error = activation::activate(
            &store,
            Provider::Codex,
            &parse_account_id("second").unwrap(),
            &native,
            activation::SwitchCause::Person,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("cli_auth_credentials_store"));
        assert_eq!(codex_login(&native), "first@example.com");
    }

    /// Claude's profile endpoint, reporting `login` to the one request it serves.
    fn claude_reports(login: &str) -> std::thread::JoinHandle<()> {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std::env::set_var(
            "LF_TEST_CLAUDE_PROFILE_URL",
            format!("http://{}/profile", listener.local_addr().unwrap()),
        );
        let body =
            serde_json::json!({"account":{"email":format!("{login}@example.com"),"uuid":login}})
                .to_string();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let _ = stream.read(&mut request).unwrap();
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            stream.write_all(response.as_bytes()).unwrap();
        })
    }

    fn claude_credential(home: &Path) -> String {
        fs::read_to_string(home.join(".credentials.json")).unwrap()
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn claude_switching_away_and_back_keeps_a_login_the_provider_rotated() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Claude, temp.path(), "first").await;
        // Claude refreshed the active login in place; neither token survives.
        let rotated = claude_credential(&native).replace("fixture-first", "rotated-first");
        fs::write(native.join(".credentials.json"), &rotated).unwrap();
        let asked = claude_reports("first");

        // Only the provider can say whose login this is now.
        assert!(!activate(&store, Provider::Claude, "first", &native).await);
        asked.join().unwrap();
        assert_eq!(claude_credential(&temp.path().join("first")), rotated);

        assert!(activate(&store, Provider::Claude, "second", &native).await);
        assert_eq!(
            claude_credential(&native),
            claude_credential(&temp.path().join("second"))
        );
        assert!(activate(&store, Provider::Claude, "first", &native).await);
        assert_eq!(claude_credential(&native), rotated);
        // The rotated profile still proves its identity without the provider.
        let accounts = store.list_provider_accounts(Some("claude")).await.unwrap();
        assert!(identity::check_account_identity(&accounts[0], &accounts).is_ok());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn an_unknown_native_claude_login_is_kept_as_a_new_profile() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Claude, temp.path(), "first").await;
        let stranger = claude_credential(&native).replace("fixture-first", "fixture-stranger");
        fs::write(native.join(".credentials.json"), &stranger).unwrap();
        let asked = claude_reports("stranger");

        assert!(activate(&store, Provider::Claude, "second", &native).await);
        asked.join().unwrap();

        let kept = store
            .list_provider_accounts(Some("claude"))
            .await
            .unwrap()
            .into_iter()
            .find(|account| account_login(account) == "stranger@example.com")
            .expect("the unknown login became a stored profile");
        assert_eq!(kept.routing_state, RoutingState::ExplicitOnly);
        assert_eq!(claude_credential(&kept.home.unwrap()), stranger);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn shared_claude_launch_names_no_home_or_credential() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Claude, temp.path(), "first").await;
        assert!(activate(&store, Provider::Claude, "second", &native).await);

        // `second` is active though the route lists `first` ahead of it.
        let route = resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .unwrap();
        let mut command = Command::new("claude");
        command.env("CLAUDE_CODE_OAUTH_TOKEN", "inherited");
        let switched = route.launch_as(&mut command).await.unwrap();

        assert!(route.is_shared());
        assert_eq!(route.account_id().as_str(), "second");
        assert!(switched.is_none());
        let set = |name: &str| {
            command
                .get_envs()
                .any(|(key, value)| key == std::ffi::OsStr::new(name) && value.is_some())
        };
        assert!(!set("CLAUDE_CONFIG_DIR") && !set("CLAUDE_CODE_OAUTH_TOKEN"));
        assert_eq!(
            activation::credential_home(Provider::Claude, &temp.path().join("second")),
            native
        );
    }

    fn launch_home(command: &Command) -> Option<Option<PathBuf>> {
        command
            .get_envs()
            .find(|(name, _)| *name == std::ffi::OsStr::new("CODEX_HOME"))
            .map(|(_, value)| value.map(PathBuf::from))
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn shared_launch_follows_the_active_account_in_the_native_home() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        // `second` is active though the route lists `first` ahead of it.
        let (_store, native) = shared_home(Provider::Codex, temp.path(), "second").await;

        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        let mut command = Command::new("codex");
        let switched = route.launch_as(&mut command).await.unwrap();

        assert!(route.is_shared());
        assert_eq!(route.account_id().as_str(), "second");
        assert!(switched.is_none());
        assert_eq!(launch_home(&command), None);
        assert!(route.provider_args().is_empty());
        assert_eq!(codex_login(&native), "second@example.com");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn shared_launch_moves_off_a_strained_active_account() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Codex, temp.path(), "first").await;
        store
            .upsert_provider_account_limits(
                "codex",
                &parse_account_id("first").unwrap(),
                &[crate::store::AccountLimitWindow {
                    window: "weekly".to_string(),
                    used_percent: 99,
                    resets_at: Some(now_unix() + 3600),
                    plan: None,
                }],
                "poll",
            )
            .await
            .unwrap();

        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        let switched = route.launch_as(&mut Command::new("codex")).await.unwrap();

        assert!(switched.is_some());
        assert_eq!(codex_login(&native), "second@example.com");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn isolated_launch_and_its_conversation_stay_in_the_account_home() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (_store, native) = shared_home(Provider::Codex, temp.path(), "second").await;
        set_isolation(true);

        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        let mut command = Command::new("codex");
        let switched = route.launch_as(&mut command).await.unwrap();
        route.pin_session(&"conversation".into()).await.unwrap();

        assert!(switched.is_none());
        assert_eq!(launch_home(&command), Some(Some(temp.path().join("first"))));
        assert_eq!(codex_login(&native), "second@example.com");

        // Resumed from a shared launch, it returns to the home it started in.
        set_isolation(false);
        let resumed = resolve_provider_account(Provider::Codex, Some(&"conversation".into()))
            .await
            .unwrap()
            .unwrap();
        assert!(!resumed.is_shared());
        assert_eq!(resumed.account_id().as_str(), "first");
        assert!(resumed.resume_requested_session());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn shared_conversation_resumes_in_the_native_home_after_a_switch() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, native) = shared_home(Provider::Codex, temp.path(), "first").await;
        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        route.pin_session(&"conversation".into()).await.unwrap();
        assert!(activate(&store, Provider::Codex, "second", &native).await);

        set_isolation(true);
        let resumed = resolve_provider_account(Provider::Codex, Some(&"conversation".into()))
            .await
            .unwrap()
            .unwrap();

        assert!(resumed.is_shared());
        assert_eq!(resumed.account_id().as_str(), "second");
        assert!(resumed.resume_requested_session());

        // The account it began under is history: resuming does not switch back.
        let began = parse_account_id("first").unwrap();
        let resumed = resolve_provider_account_exact(
            Provider::Codex,
            Some(&"conversation".into()),
            Some(&began),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(resumed.account_id().as_str(), "second");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn a_named_account_list_keeps_the_active_account_it_includes() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let _selection = EnvRestore::capture(&[selection::ACCOUNT_SELECTION_ENV]);
        let (store, native) = shared_home(Provider::Codex, temp.path(), "second").await;
        let listed = |accounts: &[&str]| {
            let accounts: Vec<String> = accounts.iter().map(|id| format!("codex={id}")).collect();
            let selection = selection::AccountSelection::from_flags(&accounts, &[]).unwrap();
            std::env::set_var(
                selection::ACCOUNT_SELECTION_ENV,
                selection.env_value().unwrap(),
            );
        };

        listed(&["first", "second"]);
        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        assert!(route.is_shared());
        assert_eq!(route.account_id().as_str(), "second");

        // A strained active account does not displace a healthy named choice.
        store
            .upsert_provider_account_limits(
                "codex",
                &parse_account_id("second").unwrap(),
                &[crate::store::AccountLimitWindow {
                    window: "weekly".into(),
                    used_percent: 95,
                    resets_at: Some(now_unix() + 3600),
                    plan: None,
                }],
                "stream",
            )
            .await
            .unwrap();
        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(route.account_id().as_str(), "first");

        // Naming only another account is a request to move to it.
        listed(&["first"]);
        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(route.account_id().as_str(), "first");
        assert!(route
            .launch_as(&mut Command::new("codex"))
            .await
            .unwrap()
            .is_some());
        assert_eq!(codex_login(&native), "first@example.com");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn usage_follows_the_switch_log_only_where_a_running_agent_does() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (store, _native) = shared_home(Provider::Codex, temp.path(), "first").await;
        let first = parse_account_id("first").unwrap();
        let second = parse_account_id("second").unwrap();
        let launched = |provider| {
            ProviderAccountRoute::shared(
                provider,
                first.clone(),
                Arc::clone(&store),
                temp.path().join("first"),
                false,
            )
        };
        let (codex, claude) = (launched(Provider::Codex), launched(Provider::Claude));
        assert_eq!(claude.used_account().await.unwrap(), first);

        for provider in [Provider::Codex, Provider::Claude] {
            store
                .record_provider_account_switch(provider, &second, "person")
                .await
                .unwrap();
        }

        // Claude adopts the new login while running; Codex keeps its own.
        assert_eq!(claude.used_account().await.unwrap(), second);
        assert_eq!(codex.used_account().await.unwrap(), first);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn shared_launch_under_an_isolated_parent_drops_the_inherited_home() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _restore = EnvRestore::capture(&SHARED_ENV);
        let (_store, _native) = shared_home(Provider::Codex, temp.path(), "first").await;
        let inherited = temp.path().join("accounts/codex/second");
        std::env::set_var("CODEX_HOME", &inherited);
        // The provider default is this test's HOME; route without touching it.
        let route = resolve_provider_account(Provider::Codex, None)
            .await
            .unwrap()
            .unwrap();
        let mut command = Command::new("codex");
        route.apply(&mut command);

        assert_eq!(launch_home(&command), Some(None));
    }
}

#[cfg(test)]
mod inspection_tests {
    use std::fs;
    use std::sync::Arc;

    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    use crate::profile::{EmailAddress, ProviderRoute, RouteScope};
    use crate::provider_account::{inspect_provider_route, new_account};
    use crate::provider_auth::Provider;
    use crate::repository::RepoId;
    use crate::store::{
        open_ephemeral_store, AccountLimitWindow, ProviderAccountId, StorageConfig,
    };

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn route_inspection_matches_selection_without_writes() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(temp.path().join("db")))
                .await
                .unwrap(),
        );
        let repo = RepoId::parse("example/project").unwrap();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        assert!(
            inspect_provider_route(Some(&store), Some(&repo), Provider::Codex)
                .await
                .unwrap()
                .is_none()
        );
        let mut accounts = Vec::new();
        for id in ["strained", "healthy", "cooling"] {
            let home = temp.path().join(id);
            fs::create_dir_all(&home).unwrap();
            let claims = URL_SAFE_NO_PAD.encode(
                serde_json::json!({"email": format!("{id}@example.com"), "sub": id}).to_string(),
            );
            fs::write(
                home.join("auth.json"),
                serde_json::json!({"tokens": {"access_token": "fixture", "id_token": format!("h.{claims}.s")}}).to_string(),
            ).unwrap();
            let mut account = new_account(
                Provider::Codex,
                ProviderAccountId::parse(id).unwrap(),
                home,
                Some(EmailAddress::parse(&format!("{id}@example.com")).unwrap()),
            );
            if id == "cooling" {
                account.cooldown_until = Some(now + 1000);
            }
            store.upsert_provider_account(&account).await.unwrap();
            accounts.push(account);
        }
        store
            .upsert_provider_account_limits(
                "codex",
                &accounts[0].account_id,
                &[AccountLimitWindow {
                    window: "weekly".into(),
                    used_percent: 95,
                    resets_at: Some(now + 1000),
                    plan: None,
                }],
                "stream",
            )
            .await
            .unwrap();
        for scope in [
            None,
            Some(RouteScope::Default),
            Some(RouteScope::Repo(repo.clone())),
        ] {
            if let Some(scope) = scope {
                store
                    .set_provider_route(&ProviderRoute {
                        scope,
                        provider: Provider::Codex,
                        accounts: accounts.iter().map(|a| a.account_id.clone()).collect(),
                        created_at: now,
                        updated_at: now,
                    })
                    .await
                    .unwrap();
            }
            let before = store.list_provider_accounts(None).await.unwrap();
            let inspected = inspect_provider_route(Some(&store), Some(&repo), Provider::Codex)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                inspected
                    .iter()
                    .map(|a| a.account_id.as_str())
                    .collect::<Vec<_>>(),
                ["healthy", "strained"]
            );
            assert_eq!(store.list_provider_accounts(None).await.unwrap(), before);
            let selected = store
                .select_provider_account(
                    Provider::Codex,
                    &accounts
                        .iter()
                        .map(|a| a.account_id.clone())
                        .collect::<Vec<_>>(),
                    None,
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(selected.account.account_id, inspected[0].account_id);
        }
    }
}
