//! Native credential identity is evidence; configured email remains intent.

use std::path::Path;

use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};

use crate::provider_auth::{codex_identity_from_home, Provider};
use crate::store::ProviderAccount;
use crate::subscription::SubscriptionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountIdentity {
    pub email: String,
    pub subject: String,
    pub credential_digest: Option<String>,
}

impl AccountIdentity {
    pub(crate) fn same_login(&self, other: &Self) -> bool {
        self.subject == other.subject || self.email.eq_ignore_ascii_case(&other.email)
    }
}

pub(crate) fn validate_identity(
    account: &ProviderAccount,
    identity: &AccountIdentity,
    accounts: &[ProviderAccount],
) -> Result<(), String> {
    let recovery = format!(
        "lf account connect {} {}",
        account.provider,
        super::account_login(account)
    );
    let expected = account.login_email.as_ref().ok_or_else(|| {
        format!(
            "account '{}' needs an expected email first: lf account set {} {} --login-email <email>",
            account.account_id, account.provider, account.account_id
        )
    })?;
    if !identity.email.eq_ignore_ascii_case(expected.as_str()) {
        return Err(format!(
            "account '{}' expects {}; credential reports {}; reconnect with {recovery}",
            account.account_id, expected, identity.email
        ));
    }
    if account
        .observed_subject
        .as_ref()
        .is_some_and(|subject| subject != &identity.subject)
    {
        return Err(format!(
            "account '{}' has a different credential user; reconnect with {recovery}",
            account.account_id
        ));
    }
    for other in accounts.iter().filter(|other| {
        other.provider == account.provider && other.account_id != account.account_id
    }) {
        // Inspect current bytes: historical observations do not prove a login is still held.
        let other_identity = cached_identity(other).or_else(|| {
            // A copied credential is the same login even before its destination
            // has a saved profile observation.
            identity.credential_digest.as_ref().and_then(|digest| {
                (current_credential_digest(other).as_ref() == Some(digest))
                    .then(|| identity.clone())
            })
        });
        if let Some(other_identity) = other_identity {
            if identity.same_login(&other_identity) {
                return Err(format!("account '{}' and account '{}' share login {}; disconnect the duplicate with lf account disconnect {} {}, then {recovery}",
                    account.account_id, other.account_id, identity.email, other.provider, super::account_login(other)));
            }
        }
    }
    Ok(())
}

pub(crate) async fn validate_observed_identity(
    account: &ProviderAccount,
    identity: &AccountIdentity,
    accounts: &[ProviderAccount],
) -> Result<(), SubscriptionError> {
    if account.provider != "claude" {
        return validate_identity(account, identity, accounts)
            .map_err(SubscriptionError::NeedsLogin);
    }
    let mut catalog = accounts.to_vec();
    for other in catalog.iter_mut().filter(|other| {
        other.provider == "claude"
            && other.account_id != account.account_id
            && cached_identity(other).is_none()
    }) {
        if let Some(home) = other
            .home
            .as_deref()
            .filter(|home| claude_login(home).is_some())
        {
            let observed = crate::subscription::claude_identity(home).await?;
            other.observed_email = Some(observed.email);
            other.observed_subject = Some(observed.subject);
            other.observed_credential_digest = observed.credential_digest;
        }
    }
    validate_identity(account, identity, &catalog).map_err(SubscriptionError::NeedsLogin)
}

pub(crate) fn check_account_identity(
    account: &ProviderAccount,
    accounts: &[ProviderAccount],
) -> Result<(), String> {
    let identity = cached_identity(account).ok_or_else(|| {
        format!("account '{}' has no identity verified against its current {} credential; run lf account {}",
            account.account_id, account.provider, account.provider)
    })?;
    validate_identity(account, &identity, accounts)
}

/// Claude tokens are opaque. Only a profile observation bound to these bytes
/// can supply offline identity; native metadata in `.claude.json` cannot.
pub(crate) fn cached_identity(account: &ProviderAccount) -> Option<AccountIdentity> {
    let home = account.home.as_deref()?;
    match account.provider.as_str() {
        "codex" => codex_identity_from_home(home),
        "claude" => {
            let digest = current_credential_digest(account)?;
            if account.observed_credential_digest.as_ref() != Some(&digest) {
                return None;
            }
            Some(AccountIdentity {
                email: account.observed_email.clone()?,
                subject: account.observed_subject.clone()?,
                credential_digest: Some(digest),
            })
        }
        _ => None,
    }
}

fn current_credential_digest(account: &ProviderAccount) -> Option<String> {
    Some(credential_digest(&claude_login(account.home.as_ref()?)?))
}

pub(crate) fn claude_login(home: &Path) -> Option<String> {
    let login = crate::provider_auth::read_claude_login(home).ok()??;
    Some(login.expose_secret().clone())
}

/// Whether two homes hold a credential for the same person. A Codex login
/// names its person. A Claude login is opaque, so only a shared token proves
/// it: once the provider rotates both, the answer needs
/// `activation::active_account`.
pub(crate) fn same_login(provider: Provider, left: &Path, right: &Path) -> bool {
    if provider == Provider::Claude {
        return match (claude_login(left), claude_login(right)) {
            (Some(left), Some(right)) => same_claude_grant(&left, &right),
            _ => false,
        };
    }
    match (
        codex_identity_from_home(left),
        codex_identity_from_home(right),
    ) {
        (Some(left), Some(right)) => left.same_login(&right),
        _ => false,
    }
}

fn same_claude_grant(left: &str, right: &str) -> bool {
    let tokens = |raw: &str| {
        let json: serde_json::Value = serde_json::from_str(raw).ok()?;
        let oauth = json.get("claudeAiOauth").unwrap_or(&json);
        let token = |name| {
            oauth
                .get(name)
                .and_then(serde_json::Value::as_str)
                .filter(|token| !token.is_empty())
                .map(str::to_string)
        };
        Some((token("accessToken"), token("refreshToken")))
    };
    match (tokens(left), tokens(right)) {
        (Some((left_access, left_refresh)), Some((right_access, right_refresh))) => {
            (left_access.is_some() && left_access == right_access)
                || (left_refresh.is_some() && left_refresh == right_refresh)
        }
        _ => false,
    }
}

pub(crate) fn credential_digest(credential: &str) -> String {
    format!("{:x}", Sha256::digest(credential.as_bytes()))
}

/// Routing may refresh missing evidence; cached inspection never calls this.
pub(crate) async fn check_current_identity(
    account: &ProviderAccount,
    accounts: &[ProviderAccount],
) -> Result<(), SubscriptionError> {
    if account.provider != Provider::Claude.as_str() {
        return check_account_identity(account, accounts).map_err(SubscriptionError::NeedsLogin);
    }
    let identity = match cached_identity(account) {
        Some(identity) => identity,
        None => {
            let home = account.home.as_deref().ok_or_else(|| {
                SubscriptionError::NeedsLogin("account has no credential home".into())
            })?;
            crate::subscription::claude_identity(home).await?
        }
    };
    validate_observed_identity(account, &identity, accounts).await?;
    if current_credential_digest(account) != identity.credential_digest {
        return Err(SubscriptionError::Unavailable(
            "credential changed during identity verification; retry".into(),
        ));
    }
    Ok(())
}

/// Serialize duplicate detection and credential installation across managed accounts.
pub(crate) fn acquire_identity_install_lock(account_home: &Path) -> anyhow::Result<std::fs::File> {
    let parent = account_home
        .parent()
        .ok_or_else(|| anyhow::anyhow!("account home has no parent"))?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(parent.join(".identity.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)
        .map_err(|_| anyhow::anyhow!("another account login is being installed; retry connect"))?;
    Ok(lock)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{check_account_identity, validate_identity};
    use crate::profile::EmailAddress;
    use crate::provider_account::{new_account, order_accounts_by_strain};
    use crate::provider_auth::{codex_identity_from_home, Provider};
    use crate::store::ProviderAccountId;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use std::fs;
    use std::path::Path;

    pub(crate) fn write_claude_identity(account: &mut crate::store::ProviderAccount) {
        let home = account.home.as_ref().unwrap();
        fs::create_dir_all(home).unwrap();
        let raw = serde_json::json!({"claudeAiOauth":{"accessToken":format!("fixture-{}", account.account_id),"expiresAt":4102444800000i64}}).to_string();
        fs::write(home.join(".credentials.json"), &raw).unwrap();
        account.observed_email = Some(account.login_email.as_ref().unwrap().to_string());
        account.observed_subject = Some(account.account_id.to_string());
        account.observed_credential_digest = Some(super::credential_digest(&raw));
    }

    fn account(root: &Path, id: &str, email: &str, subject: &str) -> crate::store::ProviderAccount {
        let home = root.join(id);
        fs::create_dir_all(&home).unwrap();
        let claims = URL_SAFE_NO_PAD.encode(serde_json::json!({"email":email,"sub":subject,"https://api.openai.com/auth":{"chatgpt_account_id":"shared-team"}}).to_string());
        fs::write(home.join("auth.json"), serde_json::json!({"tokens":{"access_token":"fixture", "id_token":format!("h.{claims}.s")}}).to_string()).unwrap();
        new_account(
            Provider::Codex,
            ProviderAccountId::parse(id).unwrap(),
            home,
            Some(EmailAddress::parse(email).unwrap()),
        )
    }

    #[test]
    fn identity_rejects_wrong_labels_missing_email_and_duplicate_users() {
        let root = tempfile::tempdir().unwrap();
        let first = account(root.path(), "engineering", "engineering@example.com", "one");
        let second = account(root.path(), "personal", "personal@example.com", "two");
        let accounts = vec![first.clone(), second.clone()];
        // Workspace ids may match; they identify no individual user.
        assert!(check_account_identity(&first, &accounts).is_ok());
        assert!(check_account_identity(&second, &accounts).is_ok());
        let identity = codex_identity_from_home(first.home.as_ref().unwrap()).unwrap();
        let mut unlabeled = first.clone();
        unlabeled.login_email = None;
        assert!(validate_identity(&unlabeled, &identity, &[])
            .unwrap_err()
            .contains("--login-email"));
        let error = validate_identity(&second, &identity, &[]).unwrap_err();
        assert!(
            error.contains("engineering@example.com") && error.contains("personal@example.com")
        );
        let duplicate = account(root.path(), "duplicate", "renamed@example.com", "one");
        let error = check_account_identity(&first, &[first.clone(), duplicate]).unwrap_err();
        assert!(
            error.contains("engineering")
                && error.contains("duplicate")
                && error.contains("share login")
        );
    }

    #[test]
    fn identity_requires_user_subject_and_accepts_chatgpt_user_id() {
        let root = tempfile::tempdir().unwrap();
        for (auth, expected) in [
            (serde_json::json!({"chatgpt_account_id":"workspace"}), None),
            (
                serde_json::json!({"chatgpt_account_id":"workspace","chatgpt_user_id":"user"}),
                Some("user"),
            ),
        ] {
            let claims = URL_SAFE_NO_PAD.encode(
                serde_json::json!({"email":"a@example.com","https://api.openai.com/auth":auth})
                    .to_string(),
            );
            fs::write(
                root.path().join("auth.json"),
                serde_json::json!({"tokens":{"id_token":format!("h.{claims}.s")}}).to_string(),
            )
            .unwrap();
            assert_eq!(
                codex_identity_from_home(root.path()).map(|i| i.subject),
                expected.map(str::to_string)
            );
        }
    }

    #[test]
    fn routing_uses_observed_plan_after_health_and_before_declared_order() {
        let root = tempfile::tempdir().unwrap();
        let mut plus = account(root.path(), "plus", "plus@example.com", "plus");
        let mut pro = account(root.path(), "pro", "pro@example.com", "pro");
        plus.observed_plan = Some("plus".into());
        pro.observed_plan = Some("pro".into());
        let mut accounts = vec![plus.clone(), pro.clone()];
        order_accounts_by_strain(&mut accounts, &[], 100);
        assert_eq!(accounts, vec![pro.clone(), plus.clone()]);
        let limit = crate::store::AccountLimitRow {
            provider: "codex".into(),
            account_id: pro.account_id.clone(),
            window: "weekly".into(),
            used_percent: 99,
            resets_at: Some(200),
            plan: Some("pro".into()),
            observed_at: 99,
            source: "poll".into(),
        };
        order_accounts_by_strain(&mut accounts, &[limit], 100);
        assert_eq!(accounts, vec![plus, pro]);
    }
}
