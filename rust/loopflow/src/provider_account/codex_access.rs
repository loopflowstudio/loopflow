//! Resolve access between the two existing copies of a managed Codex login.
//! Provider responses prove access; matching identity alone never does.

use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use secrecy::{ExposeSecret, SecretVec};

use super::activation::native_home;
use super::identity::validate_identity;
use crate::provider_auth::{
    codex::Connection, codex_identity_from_credential, codex_identity_from_home, Provider,
};
use crate::store::ProviderAccount;
use crate::subscription::{
    codex_connection, codex_usage, CodexUsageError, SubscriptionError, SubscriptionUsage,
};

struct Credential {
    home: PathBuf,
    bytes: SecretVec<u8>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credential")
            .field("home", &self.home)
            .finish_non_exhaustive()
    }
}

impl Credential {
    fn read(account: &ProviderAccount, home: &Path) -> Result<Self, SubscriptionError> {
        let bytes = fs::read(home.join("auth.json"))
            .map_err(|_| unavailable("could not read Codex credential"))?;
        let identity = codex_identity_from_credential(&bytes)
            .ok_or_else(|| SubscriptionError::NeedsLogin("Codex login is missing".into()))?;
        validate_identity(account, &identity, &[]).map_err(SubscriptionError::NeedsLogin)?;
        Ok(Self {
            home: home.into(),
            bytes: SecretVec::new(bytes),
        })
    }

    fn unchanged(&self) -> bool {
        fs::read(self.home.join("auth.json")).ok().as_ref() == Some(self.bytes.expose_secret())
    }
}

#[derive(Debug)]
pub(crate) struct CodexAccess {
    pub(crate) usage: SubscriptionUsage,
    pub(crate) connection: Connection,
    accepted: Credential,
    rejected: Option<Credential>,
}

impl CodexAccess {
    pub(crate) fn home(&self) -> &Path {
        &self.accepted.home
    }

    pub(crate) fn ensure_unchanged(&self) -> Result<(), SubscriptionError> {
        if !self.accepted.unchanged() || self.rejected.as_ref().is_some_and(|c| !c.unchanged()) {
            return Err(unavailable(
                "Codex credential changed during verification; retry",
            ));
        }
        Ok(())
    }

    /// Repair only a home this resolution rejected. Keep the actual displaced
    /// inode, since another Codex process may still have it open for refresh.
    pub(crate) fn repair(&self) -> Result<(), SubscriptionError> {
        self.ensure_unchanged()?;
        let Some(rejected) = &self.rejected else {
            return Ok(());
        };
        install(&self.accepted, rejected)
    }

    /// Save a proven native refresh into its profile without losing either copy.
    pub(crate) fn save_to(
        &self,
        account: &ProviderAccount,
        home: &Path,
    ) -> Result<(), SubscriptionError> {
        self.ensure_unchanged()?;
        let mut expected = account.clone();
        expected.observed_subject = Some(self.usage.identity.subject.clone());
        let target = Credential::read(&expected, home)?;
        if target.bytes.expose_secret() == self.accepted.bytes.expose_secret() {
            return Ok(());
        }
        install(&self.accepted, &target)
    }
}

fn unavailable(message: &str) -> SubscriptionError {
    SubscriptionError::Unavailable(message.into())
}

/// `preferred` is the actual conversation home for launch, or the matching
/// native home for status. Never change conversation placement to recover access.
pub(crate) async fn resolve(
    account: &ProviderAccount,
    preferred: Option<&Path>,
) -> Result<CodexAccess, SubscriptionError> {
    tokio::time::timeout(Duration::from_secs(60), resolve_bounded(account, preferred))
        .await
        .map_err(|_| unavailable("Codex access verification timed out; retry"))?
}

async fn resolve_bounded(
    account: &ProviderAccount,
    preferred: Option<&Path>,
) -> Result<CodexAccess, SubscriptionError> {
    let profile = account
        .home
        .as_deref()
        .ok_or_else(|| unavailable("account has no credential home"))?;
    let native = native_home(Provider::Codex, None);
    let profile_identity = codex_identity_from_home(profile);
    let mut expected = account.clone();
    if expected.observed_subject.is_none() {
        expected.observed_subject = profile_identity
            .as_ref()
            .map(|identity| identity.subject.clone());
    }
    let account = &expected;
    let matching_native = profile_identity
        .as_ref()
        .zip(codex_identity_from_home(&native).as_ref())
        .is_some_and(|(a, b)| a.same_login(b));
    let preferred = preferred.unwrap_or(if matching_native { &native } else { profile });
    let alternate = if preferred == profile {
        native.as_path()
    } else {
        profile
    };
    let mut refreshed = [false; 2];
    // A provider refresh or concurrent replacement invalidates old evidence.
    // One retry shares the original deadline and refresh budget.
    let result = match resolve_once(account, preferred, alternate, &mut refreshed).await {
        Err(ObserveError::Changed) => {
            resolve_once(account, preferred, alternate, &mut refreshed).await
        }
        result => result,
    };
    result.map_err(|error| match error {
        ObserveError::Changed => unavailable("Codex credential changed during verification; retry"),
        ObserveError::Provider(error) => error,
    })
}

enum ObserveError {
    Changed,
    Provider(SubscriptionError),
}

impl From<SubscriptionError> for ObserveError {
    fn from(error: SubscriptionError) -> Self {
        Self::Provider(error)
    }
}

async fn observe(
    account: &ProviderAccount,
    home: &Path,
    refreshed: &mut bool,
) -> Result<
    (
        Credential,
        Connection,
        Result<SubscriptionUsage, SubscriptionError>,
    ),
    ObserveError,
> {
    let credential = Credential::read(account, home)?;
    let mut connection = codex_connection(home).await?;
    let mut usage = codex_usage(&mut connection, home, false).await;
    if !credential.unchanged() {
        return Err(ObserveError::Changed);
    }
    if matches!(
        usage,
        Err(CodexUsageError::Access(SubscriptionError::NeedsLogin(_)))
    ) && !*refreshed
    {
        *refreshed = true;
        usage = codex_usage(&mut connection, home, true).await;
        // A refresh may have changed the bytes. Re-observe those bytes on a
        // fresh connection rather than attributing an old response to them.
        if !credential.unchanged() {
            return Err(ObserveError::Changed);
        }
    }
    let usage = match usage {
        // A different identity is not a rejected token: neither refresh nor
        // another credential copy can repair the account the server reported.
        Err(CodexUsageError::Identity(reason)) => {
            return Err(SubscriptionError::NeedsLogin(reason).into());
        }
        result => result.map_err(SubscriptionError::from),
    };
    if let Ok(usage) = &usage {
        validate_identity(account, &usage.identity, &[]).map_err(SubscriptionError::NeedsLogin)?;
    }
    Ok((credential, connection, usage))
}

async fn resolve_once(
    account: &ProviderAccount,
    preferred: &Path,
    alternate: &Path,
    refreshed: &mut [bool; 2],
) -> Result<CodexAccess, ObserveError> {
    let (first, connection, usage) = observe(account, preferred, &mut refreshed[0]).await?;
    let rejection = match usage {
        Ok(usage) => {
            return Ok(CodexAccess {
                usage,
                connection,
                accepted: first,
                rejected: None,
            })
        }
        Err(SubscriptionError::NeedsLogin(reason)) => reason,
        Err(error) => return Err(error.into()),
    };
    drop(connection);
    let identities = codex_identity_from_credential(first.bytes.expose_secret())
        .zip(codex_identity_from_home(alternate));
    let Some((left, right)) =
        identities.filter(|(left, right)| preferred != alternate && left.same_login(right))
    else {
        return Err(SubscriptionError::NeedsLogin(rejection).into());
    };
    // Email is selection intent; a repair source must also have the same user subject.
    if left.subject != right.subject {
        return Err(unavailable(
            "Codex copies identify different users; reconnect the selected account",
        )
        .into());
    }
    let (accepted, connection, usage) = observe(account, alternate, &mut refreshed[1]).await?;
    if !first.unchanged() {
        return Err(ObserveError::Changed);
    }
    Ok(CodexAccess {
        usage: usage?,
        connection,
        accepted,
        rejected: Some(first),
    })
}

/// A different-account switch also retains the actual outgoing inode. Saving a
/// verified native refresh to its profile must not unlink later native writes.
pub(crate) fn switch(account: &ProviderAccount, native: &Path) -> Result<(), SubscriptionError> {
    let profile = account
        .home
        .as_deref()
        .ok_or_else(|| unavailable("account has no credential home"))?;
    let source = Credential::read(account, profile)?;
    let bytes = fs::read(native.join("auth.json"))
        .map_err(|_| unavailable("native Codex credential changed before switching; retry"))?;
    let target = Credential {
        home: native.into(),
        bytes: SecretVec::new(bytes),
    };
    install(&source, &target)
}

fn install(source: &Credential, target: &Credential) -> Result<(), SubscriptionError> {
    let io_error = |_| unavailable("could not preserve and install Codex credential; retry");
    // Private directory on the target filesystem; it is durable before exchange.
    let recovery = tempfile::Builder::new()
        .prefix(".lf-auth-recovery-")
        .tempdir_in(&target.home)
        .map_err(io_error)?
        .keep();
    let parent = File::open(&target.home).map_err(io_error)?;
    let directory = File::open(&recovery).map_err(io_error)?;
    let mut staged = File::options()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(recovery.join("displaced"))
        .map_err(io_error)?;
    staged
        .write_all(source.bytes.expose_secret())
        .map_err(io_error)?;
    staged.sync_all().map_err(io_error)?;
    directory.sync_all().map_err(io_error)?;
    parent.sync_all().map_err(io_error)?;
    if !source.unchanged() || !target.unchanged() {
        return Err(unavailable(
            "Codex credential changed before installation; retry",
        ));
    }
    #[cfg(test)]
    BEFORE_EXCHANGE.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
    crate::platform::file_exchange::exchange(&parent, c"auth.json", &directory, c"displaced")
        .map_err(io_error)?;
    directory.sync_all().map_err(io_error)?;
    parent.sync_all().map_err(io_error)?;
    if fs::read(recovery.join("displaced")).map_err(io_error)? != *target.bytes.expose_secret()
        || fs::read(target.home.join("auth.json")).map_err(io_error)?
            != *source.bytes.expose_secret()
        || !source.unchanged()
    {
        return Err(unavailable(
            "Codex credential changed during installation; both versions retained; retry",
        ));
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static BEFORE_EXCHANGE: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
mod tests {
    use super::{install, Credential, BEFORE_EXCHANGE};
    use secrecy::SecretVec;
    use std::fs;
    use std::io::{Seek, SeekFrom, Write};

    #[test]
    fn codex_same_login_recovery_retains_racing_path_and_descriptor_writes() {
        let temp = tempfile::tempdir().unwrap();
        let source_home = temp.path().join("source");
        let target_home = temp.path().join("target");
        for home in [&source_home, &target_home] {
            fs::create_dir(home).unwrap();
        }
        fs::write(source_home.join("auth.json"), b"accepted").unwrap();
        fs::write(target_home.join("auth.json"), b"rejected").unwrap();
        let source = Credential {
            home: source_home,
            bytes: SecretVec::new(b"accepted".to_vec()),
        };
        let target = Credential {
            home: target_home.clone(),
            bytes: SecretVec::new(b"rejected".to_vec()),
        };
        let replaced = target_home.join("auth.json");
        let newer = target_home.join("refresh");
        fs::write(&newer, b"newer native update").unwrap();
        let mut writer = fs::OpenOptions::new().write(true).open(&newer).unwrap();
        BEFORE_EXCHANGE.with(|hook| {
            *hook.borrow_mut() = Some(Box::new(move || {
                fs::rename(newer, replaced).unwrap();
            }))
        });
        assert!(install(&source, &target).is_err());
        let recovery = fs::read_dir(&target_home)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|path| path.is_dir())
            .unwrap();
        assert_eq!(
            fs::read(recovery.join("displaced")).unwrap(),
            b"newer native update"
        );
        writer.seek(SeekFrom::End(0)).unwrap();
        writer.write_all(b" later").unwrap();
        assert_eq!(
            fs::read(recovery.join("displaced")).unwrap(),
            b"newer native update later"
        );
        assert_eq!(
            fs::read(target_home.join("auth.json")).unwrap(),
            b"accepted"
        );
    }

    #[test]
    fn codex_same_login_recovery_preservation_failure_leaves_target() {
        let temp = tempfile::tempdir().unwrap();
        let source_home = temp.path().join("source");
        fs::create_dir(&source_home).unwrap();
        fs::write(source_home.join("auth.json"), b"accepted").unwrap();
        let target_home = temp.path().join("not-a-directory");
        fs::write(&target_home, b"unchanged").unwrap();
        let source = Credential {
            home: source_home,
            bytes: SecretVec::new(b"accepted".to_vec()),
        };
        let target = Credential {
            home: target_home.clone(),
            bytes: SecretVec::new(b"rejected".to_vec()),
        };
        assert!(install(&source, &target).is_err());
        assert_eq!(fs::read(target_home).unwrap(), b"unchanged");
    }
}
