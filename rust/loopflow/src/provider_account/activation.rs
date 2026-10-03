//! A provider's native home is signed in as one stored account at a time.
//!
//! The native credential is the only record of which account is active.
//! Activation saves it back to its stored profile before installing another,
//! so a refresh token the provider rotated while active is never lost.

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::identity::same_codex_login;
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

/// Flag, then `isolate:` in config, then shared. Claude launches stay
/// isolated until its native credential has an activation path.
pub(crate) fn launch_isolated(provider: Provider) -> bool {
    if provider != Provider::Codex {
        return true;
    }
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

/// The stored account the native credential identifies.
pub(crate) fn active_account<'a>(
    native: &Path,
    accounts: &'a [ProviderAccount],
) -> Option<&'a ProviderAccount> {
    let identity = codex_identity_from_home(native)?;
    accounts
        .iter()
        .filter(|account| account.provider == Provider::Codex.as_str())
        .find(|account| {
            account
                .home
                .as_deref()
                .and_then(codex_identity_from_home)
                .is_some_and(|stored| stored.same_login(&identity))
        })
}

/// The home holding an account's live credential: the native one while that
/// account is active there, since its stored profile is only current as of the
/// last switch and refreshing both copies could invalidate one of them.
pub(crate) fn credential_home(provider: Provider, profile: &Path) -> PathBuf {
    let native = native_home(provider, None);
    if provider == Provider::Codex && same_codex_login(&native, profile) {
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
    if provider != Provider::Codex {
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
        .filter(|home| codex_identity_from_home(home).is_some());
    let Some(profile) = profile else {
        return Err(ProviderAccountError::NoAuthenticatedAccount {
            provider,
            accounts: format!(
                "lf account connect {provider} {} (its stored profile has no usable credential)",
                account_login(selected)
            ),
        });
    };
    let is_active = || same_codex_login(native, profile);
    if is_active() {
        return Ok(None);
    }
    let lock = lock_native_credential(provider)?;
    if is_active() {
        return Ok(None);
    }
    require_file_credential_store(native)?;
    save_back(store, provider, native, &accounts).await?;
    let credential = fs::read(profile.join("auth.json")).map_err(|error| {
        ProviderAccountError::Filesystem(format!("read stored {provider} credential: {error}"))
    })?;
    write_private(&native.join("auth.json"), &credential)?;
    store
        .record_provider_account_switch(provider, account_id, "from_now_on", cause.as_str())
        .await?;
    Ok(Some(lock))
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

/// Keep the native credential before it is replaced: in the stored profile
/// with the same identity, else as a new profile. Nothing is discarded.
async fn save_back(
    store: &SharedStore,
    provider: Provider,
    native: &Path,
    accounts: &[ProviderAccount],
) -> Result<(), ProviderAccountError> {
    let Ok(credential) = fs::read(native.join("auth.json")) else {
        return Ok(());
    };
    let identity = codex_identity_from_home(native);
    let login = identity
        .as_ref()
        .and_then(|identity| crate::profile::EmailAddress::parse(&identity.email).ok());
    let (Some(identity), Some(login)) = (identity, login) else {
        // An API key or unreadable login names no account to file it under.
        let digest = hex::encode(Sha256::digest(&credential));
        let kept = crate::store::lf_home_dir()
            .join("accounts")
            .join(provider.as_str())
            .join(format!("native-{}.auth.json", &digest[..12]));
        tracing::warn!(path = %kept.display(), "kept an unidentified native {provider} login");
        return write_private(&kept, &credential);
    };
    let owner = active_account(native, accounts).or_else(|| {
        accounts.iter().find(|account| {
            account
                .login_email
                .as_ref()
                .is_some_and(|email| email.as_str().eq_ignore_ascii_case(login.as_str()))
        })
    });
    if let Some(home) = owner.and_then(|account| account.home.as_deref()) {
        return write_private(&home.join("auth.json"), &credential);
    }
    let account_id = super::account_id_for_login(&login);
    let home = super::ensure_account_home(provider, &account_id)?;
    write_private(&home.join("auth.json"), &credential)?;
    let mut account = super::new_account(provider, account_id, home, Some(login));
    account.observed_email = Some(identity.email);
    account.observed_subject = Some(identity.subject);
    // A login nobody registered is kept, not volunteered for automatic work.
    account.routing_state = RoutingState::ExplicitOnly;
    store.upsert_provider_account(&account).await?;
    Ok(())
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
