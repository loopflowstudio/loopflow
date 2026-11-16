use anyhow::Result;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::{AccessProfile, AuthBrowserBinding, LocalChromeProfile};
use crate::provider_account::{account_login, acquire_managed_login_lock, open_account_store};
use crate::provider_auth::{Provider, ProviderAuthService};
use crate::store::{AccountLimitRow, CredentialState, ProviderAccountId, SharedStore};

#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    Accepted,
    Rejected,
    Unavailable,
    NotChecked,
}

#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Managed,
    Local,
}

/// One invocation's projection; retained windows keep their original evidence.
#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountReport {
    pub accounts: Vec<AccountRow>,
    pub browser: Option<BrowserDetails>,
}

#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRow {
    pub provider: Provider,
    pub account_id: Option<ProviderAccountId>,
    pub login: Option<String>,
    pub scope: Scope,
    pub source: String,
    pub cached_credential_state: String,
    pub verification: Verification,
    pub diagnostic: Option<String>,
    pub recovery: Option<String>,
    pub observed_plan: Option<String>,
    pub configured_plan: Option<String>,
    pub routing: Option<String>,
    pub cooldown_until: Option<i64>,
    /// When the credential stops working: an ambient token's expiry, or a
    /// rental's access token, which the laptop renews before then.
    pub expires_at: Option<i64>,
    pub windows: Vec<AccountLimitRow>,
    pub launches: Vec<AccountLaunch>,
    pub verified_windows: Vec<String>,
    pub reset_credits: Option<crate::subscription::RateLimitResetCredits>,
    /// Signed in to the provider's shared home.
    pub active: bool,
    /// Held there by `lf account use`.
    pub held: bool,
}

#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountLaunch {
    #[serde(deserialize_with = "Option::deserialize")]
    pub model: Option<String>,
    pub eligible: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub out_until: Option<i64>,
    pub blockers: Vec<String>,
}

fn account_launches(
    account: &crate::store::ProviderAccount,
    limits: &[AccountLimitRow],
    accounts: &[crate::store::ProviderAccount],
    now: i64,
) -> Vec<AccountLaunch> {
    let models = (account.provider == "claude")
        .then_some(crate::config::CLAUDE_MODEL_FAMILIES)
        .into_iter()
        .flatten()
        .map(Some)
        .chain(std::iter::once(None));
    let identity_blocker =
        crate::provider_account::identity::check_account_identity(account, accounts).err();
    models
        .map(|model| {
            let mut blockers =
                crate::provider_account::account_launch_blockers(account, limits, model, now);
            if let Some(reason) = &identity_blocker {
                blockers.push(reason.clone());
            }
            AccountLaunch {
                model: model.map(str::to_owned),
                eligible: blockers.is_empty(),
                out_until: crate::provider_account::account_held_until(account, limits, model, now),
                blockers,
            }
        })
        .collect()
}

#[derive(Debug, Clone, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserDetails {
    pub profiles: Vec<AccessProfile>,
    pub bindings: Vec<AuthBrowserBinding>,
    pub local_choices: Vec<LocalChromeProfile>,
    pub discovery_diagnostic: Option<String>,
}

/// Which accounts a read asks the provider about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refresh {
    None,
    All,
    /// Only those whose newest reading is older than [`STALE_READING_SECS`].
    Stale,
}

/// A reading older than this is refreshed by `lf account` and shows its age.
pub const STALE_READING_SECS: i64 = 15 * 60;

impl Refresh {
    fn wants<'a>(self, windows: impl IntoIterator<Item = &'a AccountLimitRow>, now: i64) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Stale => newest_reading(windows).is_none_or(|at| now - at > STALE_READING_SECS),
        }
    }
}

pub fn newest_reading<'a>(windows: impl IntoIterator<Item = &'a AccountLimitRow>) -> Option<i64> {
    windows.into_iter().map(|window| window.observed_at).max()
}

pub async fn report(target: Option<super::Target>, cached: bool) -> Result<AccountReport> {
    Ok(project(target, cached).await?.0)
}

pub async fn project(
    target: Option<super::Target>,
    cached: bool,
) -> Result<(
    AccountReport,
    Option<SharedStore>,
    Option<ProviderAccountId>,
)> {
    let detail = target.is_some();
    let refresh = match (cached, detail) {
        (true, _) => Refresh::None,
        (false, true) => Refresh::All,
        (false, false) => Refresh::Stale,
    };
    let store = if refresh == Refresh::None {
        crate::provider_account::read_account_store()?
    } else {
        Some(open_account_store().await?)
    };
    let (provider, only) = match &target {
        None => (None, None),
        Some(super::Target::Provider(provider)) => (Some(*provider), None),
        Some(super::Target::Login { provider, login }) => {
            let store = store
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("no stored logins on this Machine"))?;
            let account = crate::provider_account::routing::resolve_login(
                store,
                *provider,
                login,
                "lf account",
            )
            .await?;
            (Some(account.provider.parse()?), Some(account.account_id))
        }
    };
    let service = store
        .as_ref()
        .map(|store| ProviderAuthService::new(store.clone()));
    let mut rows = match &store {
        Some(store) => managed_rows(store, provider, only.as_ref(), refresh).await?,
        None => vec![],
    };
    if only.is_none() {
        rows.extend(local_rows(service.as_ref(), provider, refresh != Refresh::None).await?);
    }
    let browser = if detail {
        let (local_choices, discovery_diagnostic) = match crate::profile::local_chrome_profiles() {
            Ok(choices) => (choices, None),
            Err(_) => (vec![], Some("local Chrome profiles unavailable".into())),
        };
        let browser = BrowserDetails {
            profiles: match &store {
                Some(store) => store.list_access_profiles().await?,
                None => vec![],
            },
            bindings: match &store {
                Some(store) => store.list_auth_browser_profiles(provider, None).await?,
                None => vec![],
            },
            local_choices,
            discovery_diagnostic,
        };
        // One login shows only the browsers bound to it.
        Some(match (&only, rows.first()) {
            (Some(_), Some(row)) => login_browser(browser, row),
            _ => browser,
        })
    } else {
        None
    };
    let report = AccountReport {
        accounts: rows,
        browser,
    };
    Ok((report, store, only))
}

/// A route that leaves stored logins out: worth fixing when unintended.
#[derive(Debug)]
pub struct RouteNote {
    pub provider: Provider,
    pub scope: crate::profile::RouteScope,
    pub named: usize,
    pub rotating: usize,
}

pub async fn route_notes(store: &SharedStore) -> Result<Vec<RouteNote>> {
    let repo = crate::provider_account::launch_repo_id(None)?;
    let today = OffsetDateTime::now_utc().date();
    let mut notes = Vec::new();
    for provider in [Provider::Claude, Provider::Codex] {
        let Some(route) =
            crate::provider_account::nearest_provider_route(store, repo.as_ref(), None, provider)
                .await?
        else {
            continue;
        };
        let rotating = store
            .list_provider_accounts(Some(provider.as_str()))
            .await?
            .iter()
            .filter(|account| account.eligible_for_automatic_routing(today))
            .count();
        if route.accounts.len() < rotating {
            notes.push(RouteNote {
                provider,
                scope: route.scope,
                named: route.accounts.len(),
                rotating,
            });
        }
    }
    Ok(notes)
}

async fn local_rows(
    service: Option<&ProviderAuthService>,
    filter: Option<Provider>,
    verify: bool,
) -> Result<Vec<AccountRow>> {
    let mut rows = Vec::new();
    for provider in Provider::all()
        .into_iter()
        .filter(|p| filter.is_none_or(|f| f == *p))
    {
        let snapshot = match service {
            Some(service) => service.cached_status(provider).await?,
            None => None,
        };
        rows.push(AccountRow {
            provider,
            account_id: None,
            login: snapshot.as_ref().and_then(|s| s.status.login()),
            scope: Scope::Local,
            source: if snapshot.is_some() {
                "stored_token"
            } else {
                "ambient_uninspected"
            }
            .into(),
            cached_credential_state: snapshot
                .as_ref()
                .map(|s| s.status.as_str())
                .unwrap_or("uninspected")
                .into(),
            verification: if verify {
                Verification::Unavailable
            } else {
                Verification::NotChecked
            },
            diagnostic: verify.then(|| {
                "server verification is unavailable for local credentials; cached evidence only"
                    .into()
            }),
            recovery: None,
            observed_plan: None,
            configured_plan: None,
            routing: None,
            cooldown_until: None,
            expires_at: snapshot.and_then(|s| s.expires_at),
            launches: vec![],
            windows: vec![],
            verified_windows: vec![],
            reset_credits: None,
            active: false,
            held: false,
        });
    }
    Ok(rows)
}

async fn managed_rows(
    store: &SharedStore,
    provider: Option<Provider>,
    only: Option<&ProviderAccountId>,
    refresh: Refresh,
) -> Result<Vec<AccountRow>> {
    if refresh != Refresh::None && provider.is_none_or(|provider| provider == Provider::Claude) {
        // A login Claude refreshed in its own home is placed first, so the
        // active account is polled there and not on its stale stored copy.
        let native = crate::provider_account::activation::native_home(Provider::Claude, None);
        let placed = crate::provider_account::activation::observe_active_account(
            store,
            Provider::Claude,
            &native,
        )
        .await;
        if let Err(error) = placed {
            tracing::warn!("could not place the native Claude login: {error}");
        }
    }
    let accounts = store
        .list_provider_accounts(provider.map(Provider::as_str))
        .await?;
    let limits = store
        .provider_account_limits(provider.map(Provider::as_str))
        .await?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let active: Vec<_> = [Provider::Claude, Provider::Codex]
        .into_iter()
        .filter_map(|provider| {
            crate::provider_account::activation::active_account(
                provider,
                &crate::provider_account::activation::native_home(provider, None),
                &accounts,
            )
            .map(|account| account.account_id.clone())
        })
        .collect();
    let mut rows = Vec::new();
    for account in accounts
        .iter()
        .filter(|account| only.is_none_or(|only| account.account_id == *only))
    {
        let provider = account.provider.parse::<Provider>()?;
        let verify = refresh.wants(
            limits.iter().filter(|limit| {
                limit.provider == account.provider && limit.account_id == account.account_id
            }),
            now,
        );
        let presence = account
            .home
            .as_deref()
            .map(|home| crate::provider_auth::provider_account_credential_presence(provider, home))
            .unwrap_or(crate::provider_auth::CredentialPresence::Missing);
        let mut cached = match presence {
            crate::provider_auth::CredentialPresence::Present => account.credential_state.as_str(),
            crate::provider_auth::CredentialPresence::Missing => "missing",
            crate::provider_auth::CredentialPresence::Unreadable => "unreadable",
            crate::provider_auth::CredentialPresence::Uninspected => "uninspected",
        };
        // Retain the login lock through evidence writes: a concurrent connect must
        // not be invalidated by an observation of its predecessor credential.
        let _login_lock = if verify && presence == crate::provider_auth::CredentialPresence::Present
        {
            Some(acquire_managed_login_lock(
                account
                    .home
                    .as_deref()
                    .expect("present credential has a home"),
                provider,
                &account.account_id,
            ))
        } else {
            None
        };
        let cached_identity = crate::provider_account::identity::cached_identity(account);
        let mut observed_login = cached_identity
            .as_ref()
            .map(|identity| identity.email.clone());
        let mut live_usage = None;
        let identity_error = (presence == crate::provider_auth::CredentialPresence::Present
            && (provider == Provider::Codex || (!verify && cached_identity.is_some())))
        .then(|| {
            crate::provider_account::identity::check_account_identity(account, &accounts).err()
        })
        .flatten();
        let mut diagnostic = None;
        let mut recover = presence == crate::provider_auth::CredentialPresence::Missing
            || account.credential_state == CredentialState::Missing;
        let auth = if let Some(reason) = identity_error {
            diagnostic = Some(reason);
            recover = true;
            Verification::Rejected
        } else if verify && presence == crate::provider_auth::CredentialPresence::Present {
            let revision = store
                .provider_account_capacity_revision(&account.provider, &account.account_id)
                .await?;
            let observation = if _login_lock.as_ref().is_some_and(|lock| lock.is_err()) {
                Err(crate::subscription::SubscriptionError::Unavailable(
                    "managed credential cannot be locked; another credential operation may be in progress; retry verification".into(),
                ))
            } else {
                match crate::subscription::poll_account(account).await {
                    Ok(usage) => {
                        observed_login = Some(usage.identity.email.clone());
                        crate::provider_account::identity::validate_observed_identity(
                            account,
                            &usage.identity,
                            &accounts,
                        )
                        .await
                        .map(|()| usage)
                    }
                    Err(error) => Err(error),
                }
            };
            match observation {
                Ok(usage) => {
                    store
                        .record_provider_account_identity(
                            &account.provider,
                            &account.account_id,
                            &usage.identity.email,
                            &usage.identity.subject,
                            usage.plan.as_deref(),
                            usage.identity.credential_digest.as_deref(),
                        )
                        .await?;
                    let committed = store
                        .reconcile_provider_account_capacity(account, revision, &usage.windows)
                        .await?;
                    if !committed {
                        diagnostic =
                            Some("usage refresh superseded by a newer capacity observation".into());
                    }
                    store
                        .update_provider_account_credential_state(
                            &account.provider,
                            &account.account_id,
                            CredentialState::Connected,
                        )
                        .await?;
                    live_usage = committed.then_some(usage);
                    recover = false;
                    cached = CredentialState::Connected.as_str();
                    Verification::Accepted
                }
                Err(crate::subscription::SubscriptionError::NeedsLogin(reason)) => {
                    store
                        .update_provider_account_credential_state(
                            &account.provider,
                            &account.account_id,
                            CredentialState::Missing,
                        )
                        .await?;
                    recover = true;
                    cached = CredentialState::Missing.as_str();
                    diagnostic = Some(reason);
                    Verification::Rejected
                }
                Err(crate::subscription::SubscriptionError::Unavailable(reason)) => {
                    diagnostic = Some(reason);
                    Verification::Unavailable
                }
            }
        } else if !verify
            && provider == Provider::Claude
            && cached_identity.is_none()
            && presence == crate::provider_auth::CredentialPresence::Present
        {
            diagnostic =
                Some("current credential identity is unverified; run lf account claude".into());
            Verification::Unavailable
        } else if verify {
            diagnostic = Some("no readable managed credential".into());
            Verification::Unavailable
        } else {
            Verification::NotChecked
        };
        let current = store
            .get_provider_account(&account.provider, &account.account_id)
            .await?
            .ok_or(crate::store::StoreError::NotFound)?;
        let cooldown_until = current.cooldown_until;
        let windows: Vec<_> = store
            .provider_account_limits(Some(&account.provider))
            .await?
            .into_iter()
            .filter(|w| w.account_id == account.account_id)
            .collect();
        if live_usage.as_ref().is_some_and(|u| u.windows.is_empty()) {
            diagnostic = Some("usage response has no recognized percentage windows".into());
        }
        let observed_plan = live_usage
            .as_ref()
            .and_then(|u| u.plan.clone())
            .or_else(|| account.observed_plan.clone())
            .or_else(|| {
                windows
                    .iter()
                    .filter(|w| w.plan.is_some())
                    .max_by_key(|w| w.observed_at)
                    .and_then(|w| w.plan.clone())
            });
        // Only an access token with nothing to refresh has an expiry worth
        // showing; a full login renews itself.
        let access_expires_at = match (provider, account.home.as_deref()) {
            (Provider::Claude, Some(home)) => crate::provider_auth::read_claude_login(home)
                .ok()
                .flatten()
                .and_then(|login| {
                    crate::provider_auth::access_only_expiry(login.expose_secret())
                        .ok()
                        .flatten()
                }),
            _ => None,
        };
        rows.push(AccountRow {
            provider,
            account_id: Some(account.account_id.clone()),
            login: observed_login.or_else(|| account.login_email.as_ref().map(ToString::to_string)),
            scope: Scope::Managed,
            source: "managed_home".into(),
            cached_credential_state: cached.into(),
            verification: auth,
            diagnostic,
            recovery: recover.then(|| {
                format!(
                    "lf account connect {}={}",
                    account.provider,
                    account_login(account)
                )
            }),
            observed_plan,
            configured_plan: account.plan.clone(),
            routing: Some(
                account
                    .effective_routing_state(OffsetDateTime::now_utc().date())
                    .as_str()
                    .into(),
            ),
            cooldown_until,
            expires_at: access_expires_at,
            launches: account_launches(&current, &windows, &accounts, now),
            windows,
            reset_credits: live_usage
                .as_ref()
                .and_then(|usage| usage.reset_credits.clone()),
            verified_windows: live_usage
                .map(|u| u.windows.into_iter().map(|w| w.window).collect())
                .unwrap_or_default(),
            active: active.contains(&account.account_id),
            held: crate::provider_account::hold::held(provider).as_ref()
                == Some(&account.account_id),
        });
    }
    Ok(rows)
}

pub fn login_browser(browser: BrowserDetails, row: &AccountRow) -> BrowserDetails {
    let mut bindings: Vec<_> = browser
        .bindings
        .into_iter()
        .filter(|binding| binding.provider == row.provider && binding.account_id == row.account_id)
        .collect();
    bindings.sort_by_key(|binding| binding.position);
    let profiles: Vec<_> = browser
        .profiles
        .into_iter()
        .filter(|profile| {
            bindings
                .iter()
                .any(|binding| binding.profile_id == profile.id)
        })
        .collect();
    let local_choices = browser
        .local_choices
        .into_iter()
        .filter(|choice| {
            profiles
                .iter()
                .any(|profile| profile.chrome_directory == choice.directory)
        })
        .collect();
    BrowserDetails {
        profiles,
        bindings,
        local_choices,
        discovery_diagnostic: browser.discovery_diagnostic,
    }
}

/// `lf account <login>`: the one login, then the browsers it connects through.
pub fn local_time(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|at| {
            at.with_timezone(&chrono::Local)
                .format("%a %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn claude_cached_identity_requires_current_verified_credentials() {
        let _env = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _homes = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "CLAUDE_CONFIG_DIR"]);
        std::env::set_var("LF_HOME", root.path().join("machine"));
        // Status also inspects the native login before polling stored accounts.
        // Keep that read independent of the developer's provider credentials.
        std::env::set_var("CLAUDE_CONFIG_DIR", root.path().join("native-claude"));
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                root.path().join("store.db"),
            ))
            .await
            .unwrap(),
        );
        let account_home = root.path().join("primary");
        std::fs::create_dir(&account_home).unwrap();
        let mut account = crate::provider_account::new_account(
            crate::provider_auth::Provider::Claude,
            crate::store::ProviderAccountId::parse("primary").unwrap(),
            account_home.clone(),
            Some(crate::profile::EmailAddress::parse("first@example.com").unwrap()),
        );
        // Legacy observations have no binding to credential bytes.
        account.observed_email = Some("first@example.com".into());
        account.observed_subject = Some("first-user".into());
        store.upsert_provider_account(&account).await.unwrap();
        let credential =
            r#"{"claudeAiOauth":{"accessToken":"fixture-first","expiresAt":4102444800000}}"#;
        std::fs::write(account_home.join(".credentials.json"), credential).unwrap();
        std::fs::write(
            account_home.join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"first@example.com","accountUuid":"first-user"}}"#,
        )
        .unwrap();
        let rows = super::managed_rows(&store, None, None, super::Refresh::None)
            .await
            .unwrap();
        assert_eq!(rows[0].verification, super::Verification::Unavailable);
        assert!(rows[0]
            .launches
            .iter()
            .all(|launch| !launch.eligible && !launch.blockers.is_empty()));
        assert!(rows[0]
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("identity is unverified"));

        let (_endpoints, server) = crate::test_support::subscription::serve(
            vec![
                (200, r#"{"limits":[]}"#.into()),
                (
                    200,
                    r#"{"account":{"email":"first@example.com","uuid":"first-user"}}"#.into(),
                ),
            ],
            |_| {},
        )
        .await;
        let rows = super::managed_rows(&store, None, None, super::Refresh::All)
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(
            rows[0].verification,
            super::Verification::Accepted,
            "{:?}",
            rows[0].diagnostic
        );
        let verified = store
            .get_provider_account("claude", &account.account_id)
            .await
            .unwrap()
            .unwrap();
        assert!(verified.observed_credential_digest.is_some());
        // The fixture server has exited. Cached inspection needs neither it nor
        // a native provider process and writes no new evidence.
        let rows = super::managed_rows(&store, None, None, super::Refresh::None)
            .await
            .unwrap();
        assert_eq!(rows[0].verification, super::Verification::NotChecked);
        assert!(rows[0]
            .launches
            .iter()
            .all(|launch| launch.eligible && launch.blockers.is_empty()));
        assert_eq!(rows[0].login.as_deref(), Some("first@example.com"));
        assert_eq!(
            store
                .get_provider_account("claude", &account.account_id)
                .await
                .unwrap(),
            Some(verified.clone())
        );

        std::fs::write(
            account_home.join(".credentials.json"),
            credential.replace("fixture-first", "fixture-second"),
        )
        .unwrap();
        let rows = super::managed_rows(&store, None, None, super::Refresh::None)
            .await
            .unwrap();
        assert_eq!(rows[0].verification, super::Verification::Unavailable);
        assert!(rows[0]
            .launches
            .iter()
            .all(|launch| !launch.eligible && !launch.blockers.is_empty()));
        assert_eq!(
            store
                .get_provider_account("claude", &account.account_id)
                .await
                .unwrap(),
            Some(verified.clone())
        );
        let (_endpoints, server) = crate::test_support::subscription::serve(
            vec![
                (200, r#"{"limits":[]}"#.into()),
                (
                    200,
                    r#"{"account":{"email":"wrong@example.com","uuid":"wrong-user"}}"#.into(),
                ),
            ],
            |_| {},
        )
        .await;
        let rows = super::managed_rows(&store, None, None, super::Refresh::All)
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(rows[0].verification, super::Verification::Rejected);
        assert!(rows[0]
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("wrong@example.com"));
        let rejected = store
            .get_provider_account("claude", &account.account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            rejected.observed_credential_digest,
            verified.observed_credential_digest
        );
        assert_eq!(rejected.observed_subject, verified.observed_subject);

        std::fs::write(account_home.join(".credentials.json"), credential).unwrap();
        store.upsert_provider_account(&verified).await.unwrap();
        let duplicate_home = root.path().join("duplicate");
        std::fs::create_dir(&duplicate_home).unwrap();
        std::fs::write(duplicate_home.join(".credentials.json"), credential).unwrap();
        let duplicate = crate::provider_account::new_account(
            crate::provider_auth::Provider::Claude,
            crate::store::ProviderAccountId::parse("duplicate").unwrap(),
            duplicate_home,
            Some(crate::profile::EmailAddress::parse("other@example.com").unwrap()),
        );
        store.upsert_provider_account(&duplicate).await.unwrap();
        let rows = super::managed_rows(&store, None, None, super::Refresh::None)
            .await
            .unwrap();
        let primary = rows
            .iter()
            .find(|row| row.account_id.as_ref() == Some(&account.account_id))
            .unwrap();
        assert_eq!(primary.verification, super::Verification::Rejected);
        assert!(primary
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("share login"));
    }
}
