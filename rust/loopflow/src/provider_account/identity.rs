//! Native credential identity is evidence; configured email remains intent.

use std::path::Path;

use crate::provider_auth::{codex_identity_from_home, Provider};
use crate::store::ProviderAccount;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountIdentity {
    pub email: String,
    pub subject: String,
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
        "lf auth connect {} {}",
        account.provider,
        super::account_login(account)
    );
    let expected = account.login_email.as_ref().ok_or_else(|| {
        format!(
            "account '{}' needs an expected email first: lf auth set {} {} --login-email <email>",
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
        if let Some(other_identity) = if account.provider == "codex" {
            other.home.as_deref().and_then(codex_identity_from_home)
        } else {
            other
                .observed_email
                .as_ref()
                .zip(other.observed_subject.as_ref())
                .map(|(email, subject)| AccountIdentity {
                    email: email.clone(),
                    subject: subject.clone(),
                })
        } {
            if identity.same_login(&other_identity) {
                return Err(format!("account '{}' and account '{}' share login {}; disconnect the duplicate with lf auth disconnect {} {}, then {recovery}",
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
) -> Result<(), crate::subscription::SubscriptionError> {
    if account.provider != "claude" {
        return validate_identity(account, identity, accounts)
            .map_err(crate::subscription::SubscriptionError::NeedsLogin);
    }
    let mut catalog = accounts.to_vec();
    for other in catalog
        .iter_mut()
        .filter(|other| other.provider == "claude" && other.account_id != account.account_id)
    {
        other.observed_email = None;
        other.observed_subject = None;
        if let Some(home) = other
            .home
            .as_deref()
            .filter(|home| home.join(".credentials.json").is_file())
        {
            let observed = crate::subscription::claude_identity(home).await?;
            other.observed_email = Some(observed.email);
            other.observed_subject = Some(observed.subject);
        }
    }
    validate_identity(account, identity, &catalog)
        .map_err(crate::subscription::SubscriptionError::NeedsLogin)
}

pub(crate) fn check_account_identity(
    account: &ProviderAccount,
    accounts: &[ProviderAccount],
) -> Result<(), String> {
    if account.provider != Provider::Codex.as_str() {
        return Ok(());
    }
    let identity = account.home.as_deref().and_then(codex_identity_from_home).ok_or_else(|| {
        format!("account '{}' has no readable Codex email and user identity; lf auth connect codex {}",
            account.account_id, super::account_login(account))
    })?;
    validate_identity(account, &identity, accounts)
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
mod tests {
    use super::{check_account_identity, validate_identity};
    use crate::profile::EmailAddress;
    use crate::provider_account::{new_account, order_accounts_by_strain};
    use crate::provider_auth::{codex_identity_from_home, Provider};
    use crate::store::ProviderAccountId;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use std::fs;
    use std::path::Path;

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
