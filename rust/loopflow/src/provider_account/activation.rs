//! A provider's native home is signed in as one stored account at a time.
//!
//! The native credential is the only record of which account is active.
//! Activation saves it back to its stored profile before installing another,
//! so a refresh token the provider rotated while active is never lost.

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use secrecy::SecretString;
use sha2::{Digest, Sha256};

use super::identity::{claude_login, credential_digest, same_login, AccountIdentity};
use super::{account_login, ProviderAccountError};
use crate::provider_auth::{codex_identity_from_home, Provider};
use crate::store::{ProviderAccount, ProviderAccountId, RoutingState, SharedStore};

/// `isolated` or `shared`: the mode `--isolate` / `--shared` set for this
/// process and every launch beneath it. Absent, configuration decides.
pub const ACCOUNT_ISOLATION_ENV: &str = "LF_ACCOUNT_ISOLATION";

/// The variable and value that carry a mode to a child process.
pub fn isolation_env(isolate: bool) -> (&'static str, &'static str) {
    let mode = if isolate { "isolated" } else { "shared" };
    (ACCOUNT_ISOLATION_ENV, mode)
}

pub(crate) fn isolation_from_env() -> Option<bool> {
    match std::env::var(ACCOUNT_ISOLATION_ENV).as_deref() {
        Ok("isolated") => Some(true),
        Ok("shared") => Some(false),
        _ => None,
    }
}

/// Why the native home changed account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwitchCause {
    Person,
    Exhaustion,
}

impl SwitchCause {
    fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Exhaustion => "exhaustion",
        }
    }
}

pub(crate) fn home_env(provider: Provider) -> Option<&'static str> {
    match provider {
        Provider::Codex => Some("CODEX_HOME"),
        Provider::Claude => Some("CLAUDE_CONFIG_DIR"),
        _ => None,
    }
}

/// A stored profile directory is never the native home, even when a launch
/// inherited it from an isolated parent.
pub(crate) fn is_account_home(home: &Path) -> bool {
    home.starts_with(crate::store::lf_home_dir().join("accounts"))
}

/// The provider's ordinary home: the launch's explicit home, else the
/// ambient one, else the provider default.
pub(crate) fn native_home(provider: Provider, launch_home: Option<&OsStr>) -> PathBuf {
    let ambient = home_env(provider).and_then(std::env::var_os);
    launch_home
        .map(PathBuf::from)
        .or_else(|| ambient.map(PathBuf::from))
        .filter(|home| !home.as_os_str().is_empty() && !is_account_home(home))
        .unwrap_or_else(|| {
            let operator_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            match provider {
                Provider::Claude => operator_home.join(".claude"),
                _ => operator_home.join(".codex"),
            }
        })
}

/// Flag, then `isolate:` in config, then shared.
pub(crate) fn launch_isolated() -> bool {
    isolation_from_env().unwrap_or_else(|| {
        let repo = std::env::current_dir()
            .ok()
            .and_then(|cwd| crate::repo::discover_repo_root(&cwd).ok().flatten());
        crate::engine::config::load_config_or_default(repo.as_deref()).isolate
    })
}

/// Whether a running agent adopts a login installed after it started. Claude
/// re-reads its stored login; Codex keeps the one it started with until it
/// is resumed.
pub(crate) fn running_agents_follow_native_login(provider: Provider) -> bool {
    provider == Provider::Claude
}

/// The stored account the native credential identifies, from what is on
/// disk. A Claude login the provider has rotated since it was stored
/// matches nothing here; [`observe_active_account`] places it.
pub(crate) fn active_account<'a>(
    provider: Provider,
    native: &Path,
    accounts: &'a [ProviderAccount],
) -> Option<&'a ProviderAccount> {
    accounts
        .iter()
        .filter(|account| account.provider == provider.as_str())
        .find(|account| {
            account
                .home
                .as_deref()
                .is_some_and(|home| same_login(provider, native, home))
        })
}

/// The active account, asking the provider who a rotated Claude login belongs
/// to and bringing that account's stored profile back in step with it.
pub(crate) async fn observe_active_account(
    store: &SharedStore,
    provider: Provider,
    native: &Path,
) -> Result<Option<ProviderAccountId>, ProviderAccountError> {
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let active = |accounts| {
        active_account(provider, native, accounts).map(|account| account.account_id.clone())
    };
    if let Some(active) = active(&accounts) {
        return Ok(Some(active));
    }
    if provider != Provider::Claude || read_login(provider, native).is_none() {
        return Ok(None);
    }
    let _lock = lock_native_credential(provider)?;
    if let Some(active) = active(&accounts) {
        return Ok(Some(active));
    }
    Ok(match save_back(store, provider, native, &accounts).await? {
        Kept::Profile(account_id) => Some(account_id),
        Kept::Unplaced(_) | Kept::Nothing => None,
    })
}

/// The home holding an account's live credential: the native one while that
/// account is active there, since its stored profile is only current as of the
/// last switch and refreshing both copies could invalidate one of them.
pub(crate) fn credential_home(provider: Provider, profile: &Path) -> PathBuf {
    let native = native_home(provider, None);
    if same_login(provider, &native, profile) {
        native
    } else {
        profile.to_path_buf()
    }
}

/// Sign `native` in as `account_id`. Returns the provider credential lock
/// when the native credential changed, to be held until the launch that asked
/// for it has spawned; `None` when that account was already active.
pub(crate) async fn activate(
    store: &SharedStore,
    provider: Provider,
    account_id: &ProviderAccountId,
    native: &Path,
    cause: SwitchCause,
) -> Result<Option<fs::File>, ProviderAccountError> {
    if !matches!(provider, Provider::Codex | Provider::Claude) {
        return Err(ProviderAccountError::UnsupportedProvider);
    }
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let selected = accounts
        .iter()
        .find(|account| account.account_id == *account_id)
        .ok_or_else(|| ProviderAccountError::Runtime("selected account disappeared".into()))?;
    let profile = selected
        .home
        .as_deref()
        .filter(|home| has_login(provider, home));
    let Some(profile) = profile else {
        return Err(ProviderAccountError::NoAuthenticatedAccount {
            provider,
            accounts: format!(
                "lf account connect {provider} {} (its stored profile has no usable credential)",
                account_login(selected)
            ),
        });
    };
    let is_active = || same_login(provider, native, profile);
    if is_active() {
        return Ok(None);
    }
    let lock = lock_native_credential(provider)?;
    if is_active() {
        return Ok(None);
    }
    if provider == Provider::Codex {
        require_file_credential_store(native)?;
    }
    match save_back(store, provider, native, &accounts).await? {
        // The native login was this account's all along, rotated since it was stored.
        Kept::Profile(owner) if owner == *account_id => return Ok(None),
        Kept::Profile(_) | Kept::Nothing => {}
        Kept::Unplaced(credential) => keep_aside(provider, &credential)?,
    }
    let credential = read_login(provider, profile).ok_or_else(|| {
        ProviderAccountError::Filesystem(format!("read stored {provider} credential"))
    })?;
    write_login(provider, native, &credential)?;
    store
        .record_provider_account_switch(provider, account_id, "from_now_on", cause.as_str())
        .await?;
    Ok(Some(lock))
}

/// A home's login, as its provider stores it.
fn read_login(provider: Provider, home: &Path) -> Option<Vec<u8>> {
    match provider {
        Provider::Claude => claude_login(home).map(String::into_bytes),
        _ => fs::read(home.join("auth.json")).ok(),
    }
}

fn write_login(
    provider: Provider,
    home: &Path,
    credential: &[u8],
) -> Result<(), ProviderAccountError> {
    if provider != Provider::Claude {
        return write_private(&home.join("auth.json"), credential);
    }
    let credential = String::from_utf8(credential.to_vec())
        .map_err(|_| ProviderAccountError::Filesystem("Claude login is not UTF-8".into()))?;
    crate::provider_auth::write_claude_login(home, &SecretString::new(credential))
        .map_err(|error| ProviderAccountError::Filesystem(error.to_string()))
}

/// Whether a stored profile holds a login worth installing.
fn has_login(provider: Provider, home: &Path) -> bool {
    match provider {
        Provider::Claude => claude_login(home).is_some(),
        _ => codex_identity_from_home(home).is_some(),
    }
}

/// A launch waits here rather than failing: a switch in progress is short.
fn lock_native_credential(provider: Provider) -> Result<fs::File, ProviderAccountError> {
    let directory = crate::store::lf_home_dir()
        .join("accounts")
        .join(provider.as_str());
    let filesystem = |error: std::io::Error| {
        ProviderAccountError::Filesystem(format!("lock native {provider} credential: {error}"))
    };
    fs::create_dir_all(&directory).map_err(filesystem)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(directory.join(".native.lock"))
        .map_err(filesystem)?;
    fs2::FileExt::lock_exclusive(&lock).map_err(filesystem)?;
    Ok(lock)
}

/// Codex reads `auth.json` only when its config keeps the login in a file.
fn require_file_credential_store(native: &Path) -> Result<(), ProviderAccountError> {
    let Ok(config) = fs::read_to_string(native.join("config.toml")) else {
        return Ok(());
    };
    let store = config
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "cli_auth_credentials_store")
                .then(|| value.trim().trim_matches(['"', '\'']).to_string())
        });
    match store.as_deref() {
        None | Some("file") => Ok(()),
        Some(other) => Err(ProviderAccountError::Runtime(format!(
            "{} sets cli_auth_credentials_store = \"{other}\", so Codex would not read a switched login; set it to \"file\" or launch with --isolate",
            native.join("config.toml").display()
        ))),
    }
}

/// Where [`save_back`] put the native credential.
enum Kept {
    Nothing,
    Profile(ProviderAccountId),
    /// It names no account to file it under: an API key, or a login whose
    /// owner could not be learned.
    Unplaced(Vec<u8>),
}

/// Nothing is discarded: a login with no profile is kept beside them.
fn keep_aside(provider: Provider, credential: &[u8]) -> Result<(), ProviderAccountError> {
    let digest = hex::encode(Sha256::digest(credential));
    let file = match provider {
        Provider::Claude => "credentials.json",
        _ => "auth.json",
    };
    let kept = crate::store::lf_home_dir()
        .join("accounts")
        .join(provider.as_str())
        .join(format!("native-{}.{file}", &digest[..12]));
    tracing::warn!(path = %kept.display(), "kept an unidentified native {provider} login");
    write_private(&kept, credential)
}

/// Who the native login belongs to. Codex's names its person; Claude's is
/// opaque, so its provider is asked, which refreshes an expired login in
/// place as Claude itself would.
async fn native_identity(provider: Provider, native: &Path) -> Option<AccountIdentity> {
    match provider {
        Provider::Claude => crate::subscription::claude_identity(native).await.ok(),
        _ => codex_identity_from_home(native),
    }
}

/// File the native credential under its account: the stored profile with the
/// same identity, else a new profile.
async fn save_back(
    store: &SharedStore,
    provider: Provider,
    native: &Path,
    accounts: &[ProviderAccount],
) -> Result<Kept, ProviderAccountError> {
    if read_login(provider, native).is_none() {
        return Ok(Kept::Nothing);
    }
    let offline = active_account(provider, native, accounts);
    let identity = match offline {
        Some(_) if provider == Provider::Claude => None,
        _ => native_identity(provider, native).await,
    };
    // Read after identifying: a refresh there replaces the login.
    let Some(credential) = read_login(provider, native) else {
        return Ok(Kept::Nothing);
    };
    let owner = offline.or_else(|| {
        let identity = identity.as_ref()?;
        accounts.iter().find(|account| {
            account.observed_subject.as_deref() == Some(identity.subject.as_str())
                || account
                    .login_email
                    .as_ref()
                    .is_some_and(|email| email.as_str().eq_ignore_ascii_case(&identity.email))
        })
    });
    let observed = |account: &ProviderAccount| {
        let identity = identity.clone().or_else(|| {
            Some(AccountIdentity {
                email: account.observed_email.clone()?,
                subject: account.observed_subject.clone()?,
                credential_digest: None,
            })
        })?;
        // A Claude profile's identity is only trusted for the bytes it was observed on.
        let digest = (provider == Provider::Claude)
            .then(|| credential_digest(&String::from_utf8_lossy(&credential)));
        Some((identity, digest))
    };
    if let Some((owner, home)) = owner.and_then(|account| Some((account, account.home.as_deref()?)))
    {
        write_login(provider, home, &credential)?;
        if let Some((identity, Some(digest))) = observed(owner) {
            store
                .record_provider_account_identity(
                    provider.as_str(),
                    &owner.account_id,
                    &identity.email,
                    &identity.subject,
                    owner.observed_plan.as_deref(),
                    Some(&digest),
                )
                .await?;
        }
        return Ok(Kept::Profile(owner.account_id.clone()));
    }
    let login = identity
        .as_ref()
        .and_then(|identity| crate::profile::EmailAddress::parse(&identity.email).ok());
    let (Some(identity), Some(login)) = (identity, login) else {
        return Ok(Kept::Unplaced(credential));
    };
    let account_id = super::account_id_for_login(&login);
    let home = super::ensure_account_home(provider, &account_id)?;
    write_login(provider, &home, &credential)?;
    let mut account = super::new_account(provider, account_id.clone(), home, Some(login));
    if provider == Provider::Claude {
        account.observed_credential_digest =
            Some(credential_digest(&String::from_utf8_lossy(&credential)));
    }
    account.observed_email = Some(identity.email);
    account.observed_subject = Some(identity.subject);
    // A login nobody registered is kept, not volunteered for automatic work.
    account.routing_state = RoutingState::ExplicitOnly;
    store.upsert_provider_account(&account).await?;
    Ok(Kept::Profile(account_id))
}

/// Replace `path` by rename so a reader sees the old or the new credential.
fn write_private(path: &Path, contents: &[u8]) -> Result<(), ProviderAccountError> {
    let filesystem = |error: std::io::Error| {
        ProviderAccountError::Filesystem(format!("install {}: {error}", path.display()))
    };
    let directory = path.parent().expect("credential path has a parent");
    fs::create_dir_all(directory).map_err(filesystem)?;
    let mut staged = tempfile::NamedTempFile::new_in(directory).map_err(filesystem)?;
    staged.write_all(contents).map_err(filesystem)?;
    staged.as_file().sync_all().map_err(filesystem)?;
    staged
        .persist(path)
        .map_err(|error| filesystem(error.error))?;
    Ok(())
}
