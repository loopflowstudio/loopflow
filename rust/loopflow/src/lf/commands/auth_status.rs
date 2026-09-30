use anyhow::Result;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{format_relative_delta, parse_provider};
use crate::profile::{AccessProfile, AuthBrowserBinding, LocalChromeProfile};
use crate::provider_account::{account_login, acquire_managed_login_lock, open_account_store};
use crate::provider_auth::{Provider, ProviderAuthService};
use crate::store::{AccountLimitRow, CredentialState, ProviderAccountId, SharedStore};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Verification {
    Accepted,
    Rejected,
    Unavailable,
    NotChecked,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scope {
    Managed,
    Local,
    Forwarded,
}

/// One invocation's projection; retained windows keep their original evidence.
#[derive(Debug, Serialize, Deserialize)]
struct AuthReport {
    accounts: Vec<AuthRow>,
    forwarded_accounts_diagnostic: Option<String>,
    browser: Option<BrowserDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthRow {
    provider: Provider,
    account_id: Option<ProviderAccountId>,
    login: Option<String>,
    scope: Scope,
    source: String,
    cached_credential_state: String,
    verification: Verification,
    diagnostic: Option<String>,
    recovery: Option<String>,
    observed_plan: Option<String>,
    configured_plan: Option<String>,
    routing: Option<String>,
    cooldown_until: Option<i64>,
    expires_at: Option<i64>,
    windows: Vec<AccountLimitRow>,
    verified_windows: Vec<String>,
    reset_credits: Option<crate::subscription::RateLimitResetCredits>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BrowserDetails {
    profiles: Vec<AccessProfile>,
    bindings: Vec<AuthBrowserBinding>,
    local_choices: Vec<LocalChromeProfile>,
    discovery_diagnostic: Option<String>,
}

pub(super) async fn run(
    provider: Option<&str>,
    verify: bool,
    details: bool,
    json: bool,
) -> Result<()> {
    let provider = provider.map(parse_provider).transpose()?;
    let store = if verify {
        Some(open_account_store().await?)
    } else {
        crate::provider_account::read_account_store()?
    };
    let service = store
        .as_ref()
        .map(|store| ProviderAuthService::new(store.clone()));
    let mut rows = match &store {
        Some(store) => managed_rows(store, provider, verify).await?,
        None => vec![],
    };
    let forwarded_accounts_diagnostic = if crate::provider_account::lease::account_lease_active() {
        if verify {
            match forwarded_rows(provider) {
                Ok(forwarded) => {
                    rows.extend(forwarded);
                    None
                }
                Err(_) => Some("forwarded account identities unavailable; origin broker could not be inspected".into()),
            }
        } else {
            Some("forwarded account identities uninspected; cached status does not contact the origin broker".into())
        }
    } else {
        None
    };
    rows.extend(local_rows(service.as_ref(), provider, verify).await?);
    let browser = if details {
        let (local_choices, discovery_diagnostic) = match crate::profile::local_chrome_profiles() {
            Ok(choices) => (choices, None),
            Err(_) => (vec![], Some("local Chrome profiles unavailable".into())),
        };
        Some(BrowserDetails {
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
        })
    } else {
        None
    };
    let report = AuthReport {
        accounts: rows,
        forwarded_accounts_diagnostic,
        browser,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(80);
        print!(
            "{}",
            render(&report, width, OffsetDateTime::now_utc().unix_timestamp())
        );
    }
    Ok(())
}

fn forwarded_rows(provider: Option<Provider>) -> Result<Vec<AuthRow>> {
    let mut rows = Vec::new();
    if let Some(client) = crate::provider_account::lease::AccountLeaseClient::from_env()? {
        let lease = client.describe()?;
        for grant in lease
            .grants
            .iter()
            .filter(|g| provider.is_none_or(|p| p == g.provider))
        {
            for account_id in &grant.accounts {
                rows.push(AuthRow {
                    provider: grant.provider,
                    account_id: Some(account_id.clone()),
                    login: Some(client.login_email(grant.provider, account_id)?),
                    scope: Scope::Forwarded,
                    source: "forwarded_origin".into(),
                    cached_credential_state: "uninspected".into(),
                    verification: Verification::Unavailable,
                    diagnostic: Some("remote verification is unavailable".into()),
                    recovery: None,
                    observed_plan: None,
                    configured_plan: None,
                    routing: None,
                    cooldown_until: None,
                    expires_at: None,
                    windows: vec![],
                    verified_windows: vec![],
                    reset_credits: None,
                });
            }
        }
    }
    Ok(rows)
}

async fn local_rows(
    service: Option<&ProviderAuthService>,
    filter: Option<Provider>,
    verify: bool,
) -> Result<Vec<AuthRow>> {
    let mut rows = Vec::new();
    for provider in Provider::all()
        .into_iter()
        .filter(|p| filter.is_none_or(|f| f == *p))
    {
        let snapshot = match service {
            Some(service) => service.cached_status(provider).await?,
            None => None,
        };
        rows.push(AuthRow {
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
            windows: vec![],
            verified_windows: vec![],
            reset_credits: None,
        });
    }
    Ok(rows)
}

async fn managed_rows(
    store: &SharedStore,
    provider: Option<Provider>,
    verify: bool,
) -> Result<Vec<AuthRow>> {
    let accounts = store
        .list_provider_accounts(provider.map(Provider::as_str))
        .await?;
    let mut rows = Vec::new();
    for account in &accounts {
        let provider = account.provider.parse::<Provider>()?;
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
                    store
                        .upsert_provider_account_limits(
                            &account.provider,
                            &account.account_id,
                            &usage.windows,
                            "poll",
                        )
                        .await?;
                    store
                        .update_provider_account_credential_state(
                            &account.provider,
                            &account.account_id,
                            CredentialState::Connected,
                        )
                        .await?;
                    live_usage = Some(usage);
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
                Some("current credential identity is unverified; run lf auth status claude".into());
            Verification::Unavailable
        } else if verify {
            diagnostic = Some("no readable managed credential".into());
            Verification::Unavailable
        } else {
            Verification::NotChecked
        };
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
        rows.push(AuthRow {
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
                    "lf auth connect {} {}",
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
            cooldown_until: account.cooldown_until,
            expires_at: None,
            windows,
            reset_credits: live_usage
                .as_ref()
                .and_then(|usage| usage.reset_credits.clone()),
            verified_windows: live_usage
                .map(|u| u.windows.into_iter().map(|w| w.window).collect())
                .unwrap_or_default(),
        });
    }
    Ok(rows)
}

fn render(report: &AuthReport, width: usize, now: i64) -> String {
    let mut lines = Vec::new();
    for (local, title) in [(false, "Managed accounts"), (true, "Local services")] {
        lines.push(title.into());
        let mut count = 0;
        for row in report
            .accounts
            .iter()
            .filter(|row| (row.scope == Scope::Local) == local)
        {
            count += 1;
            let identity = match (&row.account_id, &row.login) {
                (Some(id), Some(login)) => format!("{id} · {login}"),
                (Some(id), None) => id.to_string(),
                (None, Some(login)) => login.clone(),
                (None, None) => "local credentials".into(),
            };
            lines.push(format!("{:<12} {identity}", row.provider.as_str()));
            let evidence = match row.verification {
                Verification::Accepted => "active (verified)",
                Verification::Rejected => "needs login (identity or credential rejected)",
                Verification::Unavailable => "verification unavailable",
                Verification::NotChecked => "not checked",
            };
            match row.verification {
                Verification::Accepted | Verification::Rejected => {
                    lines.push(format!("  auth: {evidence}"))
                }
                _ => lines.push(format!(
                    "  auth: cached {} · {evidence}",
                    row.cached_credential_state
                )),
            }
            if row.scope == Scope::Forwarded {
                lines.push("  forwarded from origin · remote usage unknown".into());
            }
            if let Some(plan) = &row.observed_plan {
                lines.push(format!("  observed plan: {plan}"));
            }
            let mut policy = Vec::new();
            if let Some(routing) = row.routing.as_ref().filter(|r| r.as_str() != "automatic") {
                policy.push(routing.replace('_', "-"));
            }
            if let Some(until) = row.cooldown_until.filter(|until| *until > now) {
                policy.push(format!(
                    "cooling for {}",
                    format_relative_delta(until - now)
                ));
            }
            if !policy.is_empty() {
                lines.push(format!("  routing: {}", policy.join(" · ")));
            }
            if let Some(reason) = &row.diagnostic {
                lines.push(format!("  {reason}"));
            }
            if let Some(recovery) = &row.recovery {
                lines.push(format!("  recover: {recovery}"));
            }
            if !local && row.windows.is_empty() {
                lines.push("  usage: unknown".into());
            }
            for window in &row.windows {
                let freshness = if row.verified_windows.contains(&window.window) {
                    "live".to_string()
                } else {
                    format!(
                        "cached {} ago",
                        format_relative_delta(now - window.observed_at)
                    )
                };
                let reset = match window.resets_at {
                    Some(reset) if reset <= now => "reset passed; refresh needed".into(),
                    Some(reset) => format!("resets {}", timestamp(reset)),
                    None => "reset unknown".into(),
                };
                let usage = if window.resets_at.is_some_and(|reset| reset <= now) {
                    "usage unknown".to_string()
                } else {
                    format!(
                        "{}% used, {}% left",
                        window.used_percent,
                        100u8.saturating_sub(window.used_percent)
                    )
                };
                lines.push(format!(
                    "  {}: {usage} · {freshness} · {reset}",
                    window.window
                ));
                if report.browser.is_some() {
                    lines.push(format!(
                        "    source={} observed={} plan={}",
                        window.source,
                        timestamp(window.observed_at),
                        window.plan.as_deref().unwrap_or("unknown")
                    ));
                }
            }
            if row.provider == Provider::Codex && row.scope == Scope::Managed {
                match &row.reset_credits {
                    Some(credits) => {
                        lines.push(format!(
                            "  banked resets: {} available (live)",
                            credits.available_count
                        ));
                        for credit in credits.credits.iter().flatten() {
                            let expiry = credit
                                .expires_at
                                .map(timestamp)
                                .unwrap_or_else(|| "unknown".into());
                            lines.push(format!(
                                "    {}: {} · expires {expiry}",
                                credit.id, credit.status
                            ));
                        }
                    }
                    None => lines.push("  banked resets: unknown".into()),
                }
            }
            if report.browser.is_some() {
                lines.push(format!(
                    "  source={} configured plan={} expires={}",
                    row.source,
                    row.configured_plan.as_deref().unwrap_or("unknown"),
                    row.expires_at
                        .map(timestamp)
                        .unwrap_or_else(|| "unknown".into())
                ));
            }
        }
        if count == 0 {
            lines.push("  none".into());
        }
        if !local {
            if let Some(diagnostic) = &report.forwarded_accounts_diagnostic {
                lines.push(String::new());
                lines.push("Forwarded accounts".into());
                lines.push(format!("  {diagnostic}"));
            }
        }
        lines.push(String::new());
    }
    if let Some(browser) = &report.browser {
        lines.push("Browser profiles".into());
        for profile in &browser.profiles {
            lines.push(format!(
                "{} · {} · expects {}",
                profile.id,
                profile.chrome_directory,
                profile
                    .expected_login
                    .as_ref()
                    .map(|e| e.as_str())
                    .unwrap_or("unconstrained")
            ));
            for binding in browser
                .bindings
                .iter()
                .filter(|b| b.profile_id == profile.id)
            {
                lines.push(format!(
                    "  {} / {} · choice {}",
                    binding.provider,
                    binding
                        .account_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "local credentials".into()),
                    binding.position + 1
                ));
            }
        }
        lines.push("Local Chrome choices".into());
        for choice in &browser.local_choices {
            lines.push(format!(
                "  {} · {} · {}",
                choice.name,
                choice.directory,
                choice.login.as_deref().unwrap_or("not signed in")
            ));
        }
        if let Some(reason) = &browser.discovery_diagnostic {
            lines.push(format!("  {reason}"));
        }
    }
    lines
        .into_iter()
        .map(|line| wrap_line(&line, width.max(40)))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn timestamp(seconds: i64) -> String {
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|date| {
            date.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| "unknown".into())
}

// Preserve every character of selectors; continuation lines never collide with columns.
fn wrap_line(line: &str, width: usize) -> String {
    let mut remaining: String = line.chars().filter(|c| !c.is_control()).collect();
    let mut lines = Vec::new();
    while remaining.chars().count() > width {
        let boundary = remaining
            .char_indices()
            .nth(width)
            .expect("line exceeds width")
            .0;
        let split = remaining[..boundary]
            .rfind(char::is_whitespace)
            .filter(|split| *split > 4)
            .unwrap_or(boundary);
        lines.push(remaining[..split].to_string());
        remaining = format!("    {}", remaining[split..].trim_start());
    }
    lines.push(remaining);
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{render, AuthReport};

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn claude_cached_identity_requires_current_verified_credentials() {
        let _env = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                root.path().join("store.db"),
            ))
            .await
            .unwrap(),
        );
        let mut account = crate::provider_account::new_account(
            crate::provider_auth::Provider::Claude,
            crate::store::ProviderAccountId::parse("primary").unwrap(),
            root.path().to_path_buf(),
            Some(crate::profile::EmailAddress::parse("first@example.com").unwrap()),
        );
        // Legacy observations have no binding to credential bytes.
        account.observed_email = Some("first@example.com".into());
        account.observed_subject = Some("first-user".into());
        store.upsert_provider_account(&account).await.unwrap();
        let credential =
            r#"{"claudeAiOauth":{"accessToken":"fixture-first","expiresAt":4102444800000}}"#;
        std::fs::write(root.path().join(".credentials.json"), credential).unwrap();
        std::fs::write(
            root.path().join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"first@example.com","accountUuid":"first-user"}}"#,
        )
        .unwrap();
        let rows = super::managed_rows(&store, None, false).await.unwrap();
        assert_eq!(rows[0].verification, super::Verification::Unavailable);
        assert!(rows[0]
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("identity is unverified"));

        let (_endpoints, server) = crate::subscription::observation_tests::serve(
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
        let rows = super::managed_rows(&store, None, true).await.unwrap();
        server.await.unwrap();
        assert_eq!(rows[0].verification, super::Verification::Accepted);
        let verified = store
            .get_provider_account("claude", &account.account_id)
            .await
            .unwrap()
            .unwrap();
        assert!(verified.observed_credential_digest.is_some());
        // The fixture server has exited. Cached inspection needs neither it nor
        // a native provider process and writes no new evidence.
        let rows = super::managed_rows(&store, None, false).await.unwrap();
        assert_eq!(rows[0].verification, super::Verification::NotChecked);
        assert_eq!(rows[0].login.as_deref(), Some("first@example.com"));
        assert_eq!(
            store
                .get_provider_account("claude", &account.account_id)
                .await
                .unwrap(),
            Some(verified.clone())
        );

        std::fs::write(
            root.path().join(".credentials.json"),
            credential.replace("fixture-first", "fixture-second"),
        )
        .unwrap();
        let rows = super::managed_rows(&store, None, false).await.unwrap();
        assert_eq!(rows[0].verification, super::Verification::Unavailable);
        assert_eq!(
            store
                .get_provider_account("claude", &account.account_id)
                .await
                .unwrap(),
            Some(verified.clone())
        );
        let (_endpoints, server) = crate::subscription::observation_tests::serve(
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
        let rows = super::managed_rows(&store, None, true).await.unwrap();
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

        std::fs::write(root.path().join(".credentials.json"), credential).unwrap();
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
        let rows = super::managed_rows(&store, None, false).await.unwrap();
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

    #[test]
    fn auth_report_fixture_preserves_missingness_and_readable_widths() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_status.json");
        let report: AuthReport = serde_json::from_str(fixture).unwrap();
        let rejected = report
            .accounts
            .iter()
            .find(|row| row.verification == super::Verification::Rejected)
            .unwrap();
        assert_eq!(rejected.cached_credential_state, "missing");
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        for width in [80, 120] {
            let text = render(&report, width, 1_800_000_000);
            assert!(text.lines().all(|line| line.chars().count() <= width));
            for evidence in [
                "someone-with-a-very-long-but-valid-username@engineering.department.example.com",
                "session: 22% used, 78% left · live",
                "weekly:opus: usage unknown · cached",
                "reset passed; refresh needed",
                "reset unknown",
                "usage: unknown",
                "needs login (identity or credential rejected)",
                "verification unavailable",
                "forwarded from origin",
                "Forwarded accounts",
                "forwarded account identities uninspected",
                "cached expired",
                "cached uninspected",
                "Local services",
                "source=poll observed=2027-01-15T08:00:00Z plan=unknown",
                "Browser profiles",
                "personal · Profile 3 · expects personal@example.com",
                "work · Profile 7 · expects unconstrained",
                "claude / personal · choice 2",
                "linear / local credentials · choice 1",
                "Spare · Profile 9 · not signed in",
            ] {
                assert!(
                    text.replace("\n    ", "").contains(evidence),
                    "missing {evidence}: {text}"
                );
            }
            assert!(!text.contains(": 0% used"));
            assert!(!text.contains("96%"));
            assert!(!text.contains("unlimited"));
            // A decisive rejection renders its verdict, never the pre-verification cache.
            assert!(!text.contains("cached connected · needs login"));
            assert!(!text.contains("cached missing · needs login"));
            println!("--- {width} columns ---\n{text}");
        }
        let mut incomplete = serde_json::to_value(&report).unwrap();
        incomplete["accounts"][0]
            .as_object_mut()
            .unwrap()
            .remove("windows");
        assert!(serde_json::from_value::<AuthReport>(incomplete).is_err());
    }
}
