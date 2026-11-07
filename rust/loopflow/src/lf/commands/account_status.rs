use anyhow::Result;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::format_relative_delta;
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
}

/// One invocation's projection; retained windows keep their original evidence.
#[derive(Debug, Serialize, Deserialize)]
struct AccountReport {
    accounts: Vec<AccountRow>,
    browser: Option<BrowserDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccountRow {
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
    /// When the credential stops working: an ambient token's expiry, or a
    /// rental's access token, which the laptop renews before then.
    expires_at: Option<i64>,
    windows: Vec<AccountLimitRow>,
    verified_windows: Vec<String>,
    reset_credits: Option<crate::subscription::RateLimitResetCredits>,
    /// Signed in to the provider's shared home.
    active: bool,
    /// Held there by `lf account use`.
    held: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct BrowserDetails {
    profiles: Vec<AccessProfile>,
    bindings: Vec<AuthBrowserBinding>,
    local_choices: Vec<LocalChromeProfile>,
    discovery_diagnostic: Option<String>,
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
const STALE_READING_SECS: i64 = 15 * 60;

impl Refresh {
    fn wants<'a>(self, windows: impl IntoIterator<Item = &'a AccountLimitRow>, now: i64) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Stale => newest_reading(windows).is_none_or(|at| now - at > STALE_READING_SECS),
        }
    }
}

fn newest_reading<'a>(windows: impl IntoIterator<Item = &'a AccountLimitRow>) -> Option<i64> {
    windows.into_iter().map(|window| window.observed_at).max()
}

/// `lf account` and `lf account <login|provider>`.
pub(super) async fn run(target: Option<super::Target>, cached: bool, json: bool) -> Result<()> {
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
            let account =
                super::super::profile::resolve_login(store, *provider, login, "lf account").await?;
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
    let report = AccountReport {
        accounts: rows,
        browser,
    };
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if detail {
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(80);
        print!("{}", render(&report, width, now));
    } else {
        let notes = match &store {
            Some(store) => route_notes(store).await?,
            None => vec![],
        };
        print!("{}", render_table(&report, &notes, now));
    }
    Ok(())
}

/// A route that leaves stored logins out: worth fixing when unintended.
#[derive(Debug)]
struct RouteNote {
    provider: Provider,
    scope: crate::profile::RouteScope,
    named: usize,
    rotating: usize,
}

async fn route_notes(store: &SharedStore) -> Result<Vec<RouteNote>> {
    let repo = crate::provider_account::current_repo_id()?;
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
                Some("current credential identity is unverified; run lf account claude".into());
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
        // Only an access token with nothing to refresh has an expiry worth
        // showing; a full login renews itself.
        let access_expires_at = match (provider, account.home.as_deref()) {
            (Provider::Claude, Some(home)) => crate::provider_auth::read_claude_login(home)
                .ok()
                .flatten()
                .and_then(|login| {
                    super::super::machine_credentials::access_only_expiry(login.expose_secret())
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
            cooldown_until: account.cooldown_until,
            expires_at: access_expires_at,
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

fn next_action(row: &AccountRow, now: i64) -> String {
    let provider = row.provider.as_str();

    if row.scope == Scope::Local {
        return if row.cached_credential_state == "active"
            && row.expires_at.is_none_or(|expires| expires > now)
        {
            "use the service; launch checks required access".into()
        } else {
            format!("lf account connect {provider} (ambient access is not checked here)")
        };
    }
    if row.routing.as_deref() == Some("disabled") {
        return "disabled for routing; lf account next explains the route".into();
    }
    if row.cooldown_until.is_some_and(|until| until > now) {
        return format!("wait for cooldown, then lf account {provider}");
    }
    if row.routing.as_deref() == Some("explicit_only") {
        return format!(
            "paused: name it with --account, or lf account unpause {provider}={}",
            row.login.as_deref().unwrap_or("<login>")
        );
    }
    format!("lf account {provider} (refresh credential and capacity evidence)")
}

fn render(report: &AccountReport, width: usize, now: i64) -> String {
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
                Verification::Accepted => "active (verified)".into(),
                Verification::Rejected => "needs login (identity or credential rejected)".into(),
                Verification::Unavailable => format!(
                    "cached {} · verification unavailable",
                    row.cached_credential_state
                ),
                Verification::NotChecked => {
                    format!("cached {} · not checked", row.cached_credential_state)
                }
            };
            lines.push(format!("  auth: {evidence}"));

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
            if let (Scope::Managed, Some(expires)) = (&row.scope, row.expires_at) {
                let state = if expires > now { "until" } else { "expired" };
                lines.push(format!(
                    "  access token {state} {}; the laptop's `lf machine connect` renews it",
                    date(expires)
                ));
            }
            if let Some(reason) = &row.diagnostic {
                lines.push(format!("  {reason}"));
            }
            if let Some(recovery) = &row.recovery {
                lines.push(format!("  recover: {recovery}"));
            } else {
                lines.push(format!("  next: {}", next_action(row, now)));
            }
            if row.windows.is_empty() {
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

const BAR_WIDTH: usize = 16;

/// One row per stored login: the weekly window as a bar and percent left,
/// when it resets, and what marks it; then a line per thing to fix, with the
/// command that fixes it.
fn render_table(report: &AccountReport, notes: &[RouteNote], now: i64) -> String {
    let rows: Vec<_> = report
        .accounts
        .iter()
        .filter(|row| row.scope == Scope::Managed)
        .collect();
    if rows.is_empty() {
        return "No stored logins. lf account connect claude=<email> adds one\n".into();
    }
    let names: Vec<String> = rows
        .iter()
        .map(|row| short_login(row, &report.accounts))
        .collect();
    let login_width = names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);
    let mut lines = Vec::new();
    let mut fixes = Vec::new();
    for (row, name) in rows.iter().zip(&names) {
        let weekly = crate::provider_account::tightest_weekly(&row.windows, now);
        let (bar, left) = match weekly {
            Some(window) => {
                let left = 100u8.saturating_sub(window.used_percent);
                let filled = usize::from(left) * BAR_WIDTH / 100;
                (
                    format!("{}{}", "━".repeat(filled), "─".repeat(BAR_WIDTH - filled)),
                    format!("{left:>3}% left"),
                )
            }
            None => ("╌".repeat(BAR_WIDTH), "   ? left".to_string()),
        };
        let reset = weekly
            .and_then(|window| window.resets_at)
            .map(local_time)
            .unwrap_or_default();
        let mut marks = Vec::new();
        if row.active {
            marks.push("● active".to_string());
        }
        if row.held {
            marks.push("held".to_string());
        }
        match row.routing.as_deref() {
            Some("explicit_only") => marks.push("paused".to_string()),
            Some("disabled") => marks.push("disabled".to_string()),
            _ => {}
        }
        if let Some(at) = newest_reading(&row.windows).filter(|at| now - at > STALE_READING_SECS) {
            marks.push(format!("{} ago", format_relative_delta(now - at)));
        }
        lines.push(
            format!(
                "  {:<6}  {name:<login_width$}  {bar}  {left}  {reset:<9}  {}",
                row.provider.as_str(),
                marks.join(" · ")
            )
            .trim_end()
            .to_string(),
        );
        let indent = " ".repeat(10 + login_width);
        if let Some(until) = row.cooldown_until.filter(|until| *until > now) {
            lines.push(format!("{indent}out until {}", local_time(until)));
        }
        for window in &row.windows {
            if let (Some(model), Some(until)) = (&window.model, window.limited_until) {
                if until > now {
                    lines.push(format!("{indent}{model} out until {}", local_time(until)));
                }
            }
        }
        if let Some(recovery) = &row.recovery {
            fixes.push((
                format!("{} {name} needs a login", row.provider.as_str()),
                recovery.clone(),
            ));
        }
    }
    for note in notes {
        let provider = note.provider.as_str();
        let (scope, command) = match &note.scope {
            crate::profile::RouteScope::Default => (
                "default order",
                format!("lf unset account.{provider} --global"),
            ),
            crate::profile::RouteScope::Repo(_) => {
                ("order here", format!("lf unset account.{provider}"))
            }
            crate::profile::RouteScope::Wave(wave) => (
                "Wave order",
                format!("lf --wave {wave} unset account.{provider}"),
            ),
        };
        fixes.push((
            format!(
                "{provider} {scope} uses {} of {}",
                note.named, note.rotating
            ),
            command,
        ));
    }
    if !fixes.is_empty() {
        lines.push(String::new());
        let width = fixes
            .iter()
            .map(|(what, _)| what.chars().count())
            .max()
            .unwrap_or(0);
        for (what, command) in fixes {
            lines.push(format!("  ! {what:<width$}   {command}"));
        }
    }
    lines.join("\n") + "\n"
}

/// `jack@` when no other login of the provider starts that way, else the
/// whole email: what `lf account use` accepts for it.
fn short_login(row: &AccountRow, rows: &[AccountRow]) -> String {
    let Some(login) = row.login.as_deref() else {
        return row
            .account_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
    };
    let Some((local, _)) = login.split_once('@') else {
        return login.to_string();
    };
    let prefix = format!("{local}@");
    let shared = rows.iter().any(|other| {
        other.provider == row.provider
            && other.scope == Scope::Managed
            && other.account_id != row.account_id
            && other.login.as_deref().is_some_and(|other| {
                other
                    .to_ascii_lowercase()
                    .starts_with(&prefix.to_ascii_lowercase())
            })
    });
    if shared {
        login.to_string()
    } else {
        prefix
    }
}

pub(crate) fn local_time(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .map(|at| {
            at.with_timezone(&chrono::Local)
                .format("%a %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

fn date(seconds: i64) -> String {
    OffsetDateTime::from_unix_timestamp(seconds)
        .map(|moment| moment.date().to_string())
        .unwrap_or_else(|_| "unknown".into())
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
    use super::{render, render_table, AccountReport, RouteNote};

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

    #[test]
    fn account_report_fixture_preserves_missingness_and_readable_widths() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_status.json");
        let report: AccountReport = serde_json::from_str(fixture).unwrap();
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
                "next: lf account claude",
                "wait for cooldown, then lf account claude",
                "needs login (identity or credential rejected)",
                "verification unavailable",
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
                "access token until 2027-05-11",
                "access token expired 2026-09-21",
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
        assert!(serde_json::from_value::<AccountReport>(incomplete).is_err());
    }

    #[test]
    fn the_overview_is_one_row_per_login_with_fixes_under_it() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_status.json");
        let mut report: AccountReport = serde_json::from_str(fixture).unwrap();
        let now = 1_800_000_000;
        report.accounts[0]
            .windows
            .push(crate::store::AccountLimitRow {
                provider: "claude".into(),
                account_id: crate::store::ProviderAccountId::parse("primary").unwrap(),
                window: "seven_day".into(),
                used_percent: 69,
                resets_at: Some(now + 86_400),
                plan: None,
                observed_at: now - 60,
                source: "poll".into(),
                model: None,
                limited_until: None,
            });
        let fable = crate::store::AccountLimitRow {
            window: "seven_day_fable".into(),
            used_percent: 100,
            model: Some("fable".into()),
            limited_until: Some(now + 86_400),
            ..report.accounts[0].windows[2].clone()
        };
        report.accounts[0].windows.push(fable);
        let notes = [RouteNote {
            provider: crate::provider_auth::Provider::Codex,
            scope: crate::profile::RouteScope::Default,
            named: 1,
            rotating: 4,
        }];
        let text = render_table(&report, &notes, now);
        println!("{text}");
        let rows: Vec<_> = text
            .lines()
            .filter(|line| line.starts_with("  claude") || line.starts_with("  codex"))
            .collect();
        assert_eq!(rows.len(), 5, "managed logins only: {text}");
        assert!(rows[0].contains("31% left") && rows[0].contains("● active · held"));
        assert!(
            rows[1].contains("? left"),
            "no reading is unknown, never 0%"
        );
        assert!(rows[4].contains("paused"));
        for expected in [
            "fable out until",
            "out until",
            "! claude missing@ needs a login",
            "lf account connect claude=missing@example.com",
            "! codex default order uses 1 of 4",
            "lf unset account.codex --global",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
        assert!(!text.contains(" 0% left"));
        assert!(!text.contains("local credentials"));
    }
}
