//! On-demand subscription usage for managed provider accounts.
//!
//! Answers "how much of this account's plan is left" by asking the provider
//! directly: Claude through its OAuth usage endpoint (refreshing the stored
//! token when it has expired), Codex through a one-shot `codex app-server`
//! JSON-RPC exchange against the account's home. Results are persisted to
//! `provider_account_limits` by live auth status. Cached auth reads
//! never poll; `lf history usage` separately reports Session token/cost evidence.

use std::path::Path;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::provider_account::activation::{claude_credential_home, ClaudeRefreshLock};
use crate::provider_account::identity::{credential_digest, AccountIdentity};
use crate::store::{AccountLimitWindow, ProviderAccount};

#[cfg(not(test))]
const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
#[cfg(not(test))]
const CLAUDE_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
/// Claude Code's public OAuth client id — the tokens in an imported account
/// home were minted for it, so refreshes must present the same client.
const CLAUDE_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// Why an account's subscription state could not be read. `NeedsLogin` is an
/// answer, not a failure: the account exists but its credential was revoked
/// or expired beyond refresh, and only `lf account connect` fixes that.
#[derive(Debug, thiserror::Error)]
pub enum SubscriptionError {
    #[error("needs re-login: {0}")]
    NeedsLogin(String),
    #[error("{0}")]
    Unavailable(String),
}

/// The freshly observed subscription state of one account.
#[derive(Debug)]
pub struct SubscriptionUsage {
    pub windows: Vec<AccountLimitWindow>,
    pub plan: Option<String>,
    pub reset_credits: Option<RateLimitResetCredits>,
    pub(crate) identity: AccountIdentity,
}

/// Poll one managed account's provider for its live subscription state.
pub async fn poll_account(
    account: &ProviderAccount,
) -> Result<SubscriptionUsage, SubscriptionError> {
    let home = account.home.as_deref().ok_or_else(|| {
        SubscriptionError::Unavailable("account has no managed credential home".to_string())
    })?;
    let mut usage = match account.provider.as_str() {
        "claude" => poll_claude(&claude_credential_home(home)).await?,
        "codex" => {
            crate::provider_account::codex_access::resolve(account, None)
                .await?
                .usage
        }
        other => {
            return Err(SubscriptionError::Unavailable(format!(
                "no subscription poll for provider '{other}'"
            )))
        }
    };
    // A spent window holds its account back until it resets, whichever read
    // saw it; a hold the stream set survives a later poll.
    for window in &mut usage.windows {
        if window.used_percent >= 100 {
            window.limited_until = window
                .resets_at
                .map(|reset| reset + crate::provider_account::RESET_GRACE_SECS);
        }
    }
    Ok(usage)
}

// -- Claude ------------------------------------------------------------------

async fn poll_claude(home: &Path) -> Result<SubscriptionUsage, SubscriptionError> {
    let client = reqwest::Client::new();
    let (mut token, mut credential, refreshed) =
        fresh_claude_token(&client, home, false, None).await?;
    let mut response = request_claude_usage(&client, token.expose_secret()).await?;
    ensure_claude_credential_unchanged(home, credential.expose_secret())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED && !refreshed {
        (token, credential, _) = fresh_claude_token(&client, home, true, None).await?;
        response = request_claude_usage(&client, token.expose_secret()).await?;
    }
    let observation = async {
        classify_http_status(response.status(), "usage")?;
        let body: Value = response.json().await.map_err(|_| {
            SubscriptionError::Unavailable("usage response is not valid JSON".into())
        })?;
        // subscriptionType in the credential is cached login metadata, not a
        // plan observed by this request. No live Claude plan schema is confirmed.
        Ok(SubscriptionUsage {
            windows: claude_windows(&body),
            plan: None,
            reset_credits: None,
            identity: request_claude_identity(
                &client,
                token.expose_secret(),
                credential.expose_secret(),
            )
            .await?,
        })
    }
    .await;
    // A result describes the credential used for this request, not a replacement
    // installed by a native provider while the request was in flight.
    ensure_claude_credential_unchanged(home, credential.expose_secret())?;
    observation
}

/// `held` is Claude's refresh lock on `home` when the caller already holds it.
pub(crate) async fn claude_identity(
    home: &Path,
    held: Option<&ClaudeRefreshLock>,
) -> Result<AccountIdentity, SubscriptionError> {
    let client = reqwest::Client::new();
    let (token, credential, _) = fresh_claude_token(&client, home, false, held).await?;
    let identity =
        request_claude_identity(&client, token.expose_secret(), credential.expose_secret()).await?;
    ensure_claude_credential_unchanged(home, credential.expose_secret())?;
    Ok(identity)
}

async fn request_claude_identity(
    client: &reqwest::Client,
    token: &str,
    credential: &str,
) -> Result<AccountIdentity, SubscriptionError> {
    #[cfg(test)]
    let url = std::env::var("LF_TEST_CLAUDE_PROFILE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:1/profile".into());
    #[cfg(not(test))]
    let url = "https://api.anthropic.com/api/oauth/profile";
    let response = client
        .get(url)
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| {
            SubscriptionError::Unavailable("Claude profile request failed or timed out".into())
        })?;
    classify_http_status(response.status(), "profile")?;
    let profile: Value = response
        .json()
        .await
        .map_err(|_| SubscriptionError::Unavailable("invalid Claude profile response".into()))?;
    let email = profile
        .pointer("/account/email")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let subject = profile
        .pointer("/account/uuid")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    match (email, subject) {
        (Some(email), Some(subject)) => Ok(AccountIdentity {
            email: email.into(),
            subject: subject.into(),
            credential_digest: Some(credential_digest(credential)),
        }),
        _ => Err(SubscriptionError::Unavailable(
            "Claude profile did not report an account email and user UUID".into(),
        )),
    }
}

async fn request_claude_usage(
    client: &reqwest::Client,
    token: &str,
) -> Result<reqwest::Response, SubscriptionError> {
    client
        .get({
            #[cfg(test)]
            {
                std::env::var("LF_TEST_CLAUDE_USAGE_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:1/usage".into())
            }
            #[cfg(not(test))]
            {
                CLAUDE_USAGE_URL
            }
        })
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| SubscriptionError::Unavailable("usage request failed or timed out".into()))
}

fn classify_http_status(status: reqwest::StatusCode, stage: &str) -> Result<(), SubscriptionError> {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SubscriptionError::NeedsLogin(format!(
            "{stage} rejected credentials (HTTP 401)"
        )));
    }
    if !status.is_success() {
        return Err(SubscriptionError::Unavailable(format!(
            "{stage} returned HTTP {}",
            status.as_u16()
        )));
    }
    Ok(())
}

// Retains the existing limits-array decoder. Its fixture is synthetic; the
// configured 2026-09-27 refresh was rejected before any usage payload arrived.
// Do not treat that fixture as evidence for the current Claude endpoint schema.
fn claude_windows(body: &Value) -> Vec<AccountLimitWindow> {
    let Some(limits) = body.get("limits").and_then(Value::as_array) else {
        return Vec::new();
    };
    limits
        .iter()
        .filter_map(|limit| {
            let percent = percentage(limit.get("percent")?)?;
            let group = limit.get("group").and_then(Value::as_str)?;
            let scope = limit
                .pointer("/scope/model/display_name")
                .and_then(Value::as_str);
            // Named as Claude's stream names its `rate_limit_type`, so a poll
            // and a running Claude update the same window.
            let model = scope.map(str::to_lowercase);
            let window = match (group, &model) {
                ("session", _) => "five_hour".to_string(),
                ("weekly", None) => "seven_day".to_string(),
                ("weekly", Some(model)) => format!("seven_day_{model}"),
                (other, _) => other.to_string(),
            };
            Some(AccountLimitWindow {
                window,
                used_percent: percent,
                resets_at: limit
                    .get("resets_at")
                    .and_then(Value::as_str)
                    .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
                    .map(|value| value.unix_timestamp()),
                plan: None,
                model,
                limited_until: None,
            })
        })
        .collect()
}

/// A Claude access token, its expiry (unix seconds) and whether it was just
/// refreshed.
pub(crate) struct ClaudeAccess {
    pub token: SecretString,
    pub expires_at: Option<i64>,
    pub refreshed: bool,
}

/// The access token of the Claude login in `home`, refreshed first when it
/// expires within `margin` seconds. The refreshed login is written back to
/// `home`; the caller holds its lock.
pub(crate) async fn claude_access_token(
    home: &Path,
    margin: i64,
) -> Result<ClaudeAccess, SubscriptionError> {
    let expiry = |credential: &str| claude_access(credential).ok()?.1.map(|ms| ms / 1000);
    let stored = crate::provider_auth::read_claude_login(home)
        .map_err(|_| SubscriptionError::Unavailable("cannot read stored credentials".into()))?
        .ok_or_else(|| SubscriptionError::NeedsLogin("no stored credentials".into()))?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let due = expiry(stored.expose_secret()).is_none_or(|expires| expires <= now + margin);
    let client = reqwest::Client::new();
    let (token, credential, refreshed) = fresh_claude_token(&client, home, due, None).await?;
    Ok(ClaudeAccess {
        token,
        expires_at: expiry(credential.expose_secret()),
        refreshed,
    })
}

/// Whether the Claude login in `home` has an access token past its expiry.
/// With no recorded expiry the server judges it, so nothing is due.
pub(crate) fn claude_token_due(home: &Path) -> bool {
    let expiry = crate::provider_auth::read_claude_login(home)
        .ok()
        .flatten()
        .and_then(|login| claude_access(login.expose_secret()).ok())
        .and_then(|(_, expiry)| expiry);
    expiry.is_some_and(claude_token_expired)
}

fn claude_token_expired(expires_at_ms: i64) -> bool {
    expires_at_ms <= now_ms() + 60_000
}

fn now_ms() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp() * 1000
}

pub(crate) async fn acquire_claude_refresh_lock(
    home: &Path,
) -> Result<ClaudeRefreshLock, SubscriptionError> {
    let home = home.to_path_buf();
    tokio::task::spawn_blocking(move || ClaudeRefreshLock::acquire(&home))
        .await
        .expect("refresh lock wait does not panic")
        .map_err(|error| SubscriptionError::Unavailable(error.to_string()))
}

/// A refresh and its write-back hold Claude's own refresh lock on `home`
/// (`held` when the caller already does), so neither lf nor a running Claude
/// rotates the refresh token while the other is mid-refresh. Comparisons
/// detect completed replacements by writers that skip the lock.
/// Returns the access token, its serialized credential, and whether refreshed.
async fn fresh_claude_token(
    client: &reqwest::Client,
    home: &Path,
    force_refresh: bool,
    held: Option<&ClaudeRefreshLock>,
) -> Result<(SecretString, SecretString, bool), SubscriptionError> {
    let read = || -> Result<String, SubscriptionError> {
        Ok(crate::provider_auth::read_claude_login(home)
            .map_err(|_| SubscriptionError::Unavailable("cannot read stored credentials".into()))?
            .ok_or_else(|| SubscriptionError::NeedsLogin("no stored credentials".into()))?
            .expose_secret()
            .to_string())
    };
    let raw = read()?;
    let (access_token, expires_at_ms) = claude_access(&raw)?;
    // Missing expiry is unknown; let the server judge the token.
    if !force_refresh && !expires_at_ms.is_some_and(claude_token_expired) {
        return Ok((access_token, SecretString::new(raw), false));
    }
    let _lock = match held {
        Some(_) => None,
        None => Some(acquire_claude_refresh_lock(home).await?),
    };
    // Another holder may have refreshed while this one waited.
    let current = read()?;
    if current != raw {
        let (access_token, _) = claude_access(&current)?;
        return Ok((access_token, SecretString::new(current), true));
    }
    let mut credentials: Value = serde_json::from_str(&raw)
        .map_err(|_| SubscriptionError::Unavailable("unreadable credential format".into()))?;
    let oauth = credentials.get("claudeAiOauth").unwrap_or(&credentials);
    let refresh_token = oauth
        .get("refreshToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            SubscriptionError::NeedsLogin("expired or rejected token has no refresh token".into())
        })?;
    let response = client.post({
            #[cfg(test)]
            // A test never refreshes a real login it happened to find.
            { std::env::var("LF_TEST_CLAUDE_TOKEN_URL").unwrap_or_else(|_| "http://127.0.0.1:1/token".into()) }
            #[cfg(not(test))]
            { CLAUDE_TOKEN_URL }
        })
        .json(&json!({"grant_type": "refresh_token", "refresh_token": refresh_token, "client_id": CLAUDE_OAUTH_CLIENT_ID}))
        .timeout(Duration::from_secs(15)).send().await
        .map_err(|_| SubscriptionError::Unavailable("token refresh failed or timed out".into()))?;
    let status = response.status();
    let body: Value = response.json().await.map_err(|_| {
        SubscriptionError::Unavailable("token refresh returned invalid JSON".into())
    })?;
    ensure_claude_credential_unchanged(home, &raw)?;
    if status == reqwest::StatusCode::BAD_REQUEST
        && body.get("error").and_then(Value::as_str) == Some("invalid_grant")
    {
        return Err(SubscriptionError::NeedsLogin(
            "refresh grant rejected".into(),
        ));
    }
    classify_http_status(status, "token refresh")?;
    let new_access = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| SubscriptionError::Unavailable("refresh returned no token".into()))?
        .to_string();
    let oauth = if credentials.get("claudeAiOauth").is_some() {
        credentials
            .get_mut("claudeAiOauth")
            .expect("checked OAuth object")
    } else {
        &mut credentials
    };
    oauth["accessToken"] = json!(new_access);
    if let Some(refresh) = body
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
    {
        oauth["refreshToken"] = json!(refresh);
    }
    // An omitted expiry must not leave the new token apparently expired.
    oauth
        .as_object_mut()
        .expect("OAuth object with accessToken")
        .remove("expiresAt");
    if let Some(expiry) = body
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|seconds| *seconds > 0)
        .and_then(|seconds| seconds.checked_mul(1000))
        .and_then(|duration| now_ms().checked_add(duration))
    {
        oauth["expiresAt"] = json!(expiry);
    }
    persist_claude_refresh(home, &raw, &credentials)?;
    Ok((
        SecretString::new(new_access),
        SecretString::new(credentials.to_string()),
        true,
    ))
}

/// The access token and expiry (ms) in a serialized Claude login.
fn claude_access(raw: &str) -> Result<(SecretString, Option<i64>), SubscriptionError> {
    let credentials: Value = serde_json::from_str(raw)
        .map_err(|_| SubscriptionError::Unavailable("unreadable credential format".into()))?;
    let oauth = credentials.get("claudeAiOauth").unwrap_or(&credentials);
    let access_token = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| SubscriptionError::NeedsLogin("no access token".into()))?;
    Ok((
        SecretString::new(access_token.to_string()),
        oauth.get("expiresAt").and_then(Value::as_i64),
    ))
}

fn persist_claude_refresh(
    home: &Path,
    original: &str,
    credentials: &Value,
) -> Result<(), SubscriptionError> {
    ensure_claude_credential_unchanged(home, original)?;
    crate::provider_auth::write_claude_login(home, &SecretString::new(credentials.to_string()))
        .map_err(|_| {
            SubscriptionError::Unavailable("failed to persist refreshed credential".into())
        })
}

fn ensure_claude_credential_unchanged(
    home: &Path,
    original: &str,
) -> Result<(), SubscriptionError> {
    let current = crate::provider_auth::read_claude_login(home).ok().flatten();
    if current
        .as_ref()
        .map(|current| current.expose_secret().as_str())
        != Some(original)
    {
        return Err(SubscriptionError::Unavailable(
            "credential changed during verification; retry verification".into(),
        ));
    }
    Ok(())
}

// -- Codex -------------------------------------------------------------------

/// Wire fields from `account/rateLimits/read`. Missing details are not an empty list.
#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitResetCredits {
    pub available_count: u64,
    pub credits: Option<Vec<RateLimitResetCredit>>,
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitResetCredit {
    pub id: String,
    pub reset_type: String,
    pub status: String,
    pub granted_at: i64,
    pub expires_at: Option<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
}

pub(crate) async fn codex_connection(
    home: &Path,
) -> Result<crate::provider_auth::codex::Connection, SubscriptionError> {
    crate::provider_auth::codex::Connection::start(
        tokio::process::Command::new("codex")
            .args(["-c", "cli_auth_credentials_store=\"file\"", "app-server"])
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("OPENAI_API_KEY")
            .env("CODEX_HOME", home),
    )
    .await
    .map_err(|_| SubscriptionError::Unavailable("codex app-server unavailable".into()))
}

async fn codex_request(
    connection: &mut crate::provider_auth::codex::Connection,
    method: &str,
    params: Value,
) -> Result<Value, SubscriptionError> {
    let response = connection.request_raw(method, params).await.map_err(|_| {
        SubscriptionError::Unavailable(format!("codex {method} request failed or timed out"))
    })?;
    if let Some(error) = response.get("error") {
        let message = error.get("message").and_then(Value::as_str).unwrap_or("");
        if message.contains("token_invalidated")
            || message.contains("401 Unauthorized")
            || error.get("code").and_then(Value::as_i64) == Some(401)
        {
            return Err(SubscriptionError::NeedsLogin("credential revoked".into()));
        }
        return Err(SubscriptionError::Unavailable(format!(
            "codex {method} request failed"
        )));
    }
    response
        .get("result")
        .filter(|result| result.is_object())
        .cloned()
        .ok_or_else(|| {
            SubscriptionError::Unavailable(format!("codex returned an invalid {method} response"))
        })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CodexUsageError {
    #[error(transparent)]
    Access(#[from] SubscriptionError),
    #[error("{0}")]
    Identity(String),
}

impl From<CodexUsageError> for SubscriptionError {
    fn from(error: CodexUsageError) -> Self {
        match error {
            CodexUsageError::Access(error) => error,
            CodexUsageError::Identity(reason) => Self::NeedsLogin(reason),
        }
    }
}

pub(crate) async fn codex_usage(
    connection: &mut crate::provider_auth::codex::Connection,
    home: &Path,
    refresh: bool,
) -> Result<SubscriptionUsage, CodexUsageError> {
    let account_response =
        codex_request(connection, "account/read", json!({"refreshToken": refresh})).await?;
    let identity = crate::provider_auth::codex_identity_from_account(home, &account_response)
        .map_err(CodexUsageError::Identity)?;
    let response = codex_request(connection, "account/rateLimits/read", json!({})).await?;
    let snapshot = response.get("rateLimits");
    let windows = codex_windows(snapshot)?;
    let plan = account_response
        .pointer("/account/planType")
        .or_else(|| snapshot.and_then(|value| value.get("planType")))
        .and_then(Value::as_str)
        .map(str::to_string);
    let reset_credits = response
        .get("rateLimitResetCredits")
        .filter(|value| !value.is_null())
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|_| {
            SubscriptionError::Unavailable("codex returned invalid reset credits".into())
        })?;
    Ok(SubscriptionUsage {
        windows,
        plan,
        reset_credits,
        identity,
    })
}

#[derive(Debug)]
pub(crate) struct ResetRedemption {
    pub before: SubscriptionUsage,
    pub outcome: String,
    pub after: Result<SubscriptionUsage, SubscriptionError>,
}

/// Only the explicit named-account command calls this. Never retry a spend implicitly.
pub(crate) async fn redeem_codex_reset(
    account: &ProviderAccount,
    accounts: &[ProviderAccount],
    key: &str,
    credit_id: Option<&str>,
) -> Result<ResetRedemption, SubscriptionError> {
    let access = crate::provider_account::codex_access::resolve(account, None).await?;
    crate::provider_account::identity::validate_identity(account, &access.usage.identity, accounts)
        .map_err(SubscriptionError::NeedsLogin)?;
    access.ensure_unchanged()?;
    let home = access.home().to_path_buf();
    let mut connection = access.connection;
    let before = access.usage;
    let result = codex_request(
        &mut connection,
        "account/rateLimitResetCredit/consume",
        json!({"idempotencyKey": key, "creditId": credit_id}),
    )
    .await?;
    let outcome = result["outcome"]
        .as_str()
        .filter(|outcome| {
            matches!(
                *outcome,
                "reset" | "alreadyRedeemed" | "nothingToReset" | "noCredit"
            )
        })
        .ok_or_else(|| {
            SubscriptionError::Unavailable(
                "codex returned an unknown reset outcome; retry only with the same idempotency key"
                    .into(),
            )
        })?
        .to_string();
    let after = async {
        let usage = codex_usage(&mut connection, &home, false).await?;
        crate::provider_account::identity::validate_identity(account, &usage.identity, accounts)
            .map_err(SubscriptionError::NeedsLogin)?;
        Ok(usage)
    }
    .await;
    Ok(ResetRedemption {
        before,
        outcome,
        after,
    })
}

fn percentage(value: &Value) -> Option<u8> {
    value
        .as_f64()
        .filter(|value| value.is_finite())
        .map(|value| value.round().clamp(0.0, 100.0) as u8)
}

fn codex_windows(
    rate_limits: Option<&Value>,
) -> Result<Vec<AccountLimitWindow>, SubscriptionError> {
    let Some(snapshot) = rate_limits.filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let invalid = || SubscriptionError::Unavailable("codex returned invalid rate limits".into());
    let snapshot = snapshot.as_object().ok_or_else(invalid)?;
    let plan = snapshot.get("planType").and_then(Value::as_str);
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| snapshot.get(*key))
        .filter(|value| !value.is_null())
        .map(|value| {
            let used_percent = value
                .get("usedPercent")
                .and_then(percentage)
                .ok_or_else(invalid)?;
            Ok(AccountLimitWindow {
                window: crate::harness::codex_window_name(
                    value.get("windowDurationMins").and_then(Value::as_u64),
                )
                .to_string(),
                used_percent,
                resets_at: value.get("resetsAt").and_then(Value::as_i64),
                plan: plan.map(str::to_string),
                model: None,
                limited_until: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{claude_windows, codex_windows, SubscriptionError};
    use serde_json::json;

    #[test]
    fn claude_limits_map_to_named_windows() {
        let body = json!({
            "limits": [
                {"kind": "session", "group": "session", "percent": 22,
                 "resets_at": "2026-07-16T22:09:59+00:00", "scope": null},
                {"kind": "weekly_all", "group": "weekly", "percent": 12,
                 "resets_at": "2026-07-22T11:59:59+00:00", "scope": null},
                {"kind": "weekly_scoped", "group": "weekly", "percent": 11,
                 "resets_at": "2026-07-22T11:59:59+00:00",
                 "scope": {"model": {"id": null, "display_name": "Fable"}}}
            ]
        });
        let windows = claude_windows(&body);
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].window, "five_hour");
        assert_eq!(windows[0].used_percent, 22);
        assert!(windows[0].resets_at.is_some());
        assert_eq!(windows[0].plan, None);
        assert_eq!(windows[0].model, None);
        assert_eq!(windows[1].window, "seven_day");
        assert_eq!(windows[2].window, "seven_day_fable");
        assert_eq!(windows[2].model.as_deref(), Some("fable"));
        assert_eq!(windows[2].used_percent, 11);
    }

    #[test]
    fn codex_rate_limits_map_plan_and_weekly_window() {
        let windows = codex_windows(Some(&json!({
            "limitId": "codex",
            "primary": {"usedPercent": 78, "windowDurationMins": 10080, "resetsAt": 1784780166},
            "secondary": null,
            "planType": "pro"
        })))
        .unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].window, "weekly");
        assert_eq!(windows[0].used_percent, 78);
        assert_eq!(windows[0].plan.as_deref(), Some("pro"));
        assert_eq!(windows[0].resets_at, Some(1784780166));
    }

    #[test]
    fn a_codex_session_window_is_named_by_duration() {
        let windows = codex_windows(Some(&json!({
            "primary": {"usedPercent": 40, "windowDurationMins": 300},
            "planType": "pro"
        })))
        .unwrap();
        assert_eq!(windows[0].window, "session");
    }

    #[test]
    fn codex_missing_limits_are_unknown_but_malformed_limits_are_unavailable() {
        assert!(codex_windows(None).unwrap().is_empty());
        for snapshot in [json!(null), json!({}), json!({"primary": null})] {
            assert!(codex_windows(Some(&snapshot)).unwrap().is_empty());
        }
        for snapshot in [
            json!([]),
            json!({"primary": {}}),
            json!({"primary": {"usedPercent": "12"}}),
            json!({"primary": {"usedPercent": 12}, "secondary": false}),
        ] {
            assert!(matches!(
                codex_windows(Some(&snapshot)),
                Err(SubscriptionError::Unavailable(_))
            ));
        }
    }
}

#[cfg(test)]
pub(crate) mod observation_tests {
    use super::{claude_windows, percentage, poll_claude, SubscriptionError};
    use secrecy::ExposeSecret;
    use serde_json::{json, Value};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub(crate) struct Endpoints;
    impl Drop for Endpoints {
        fn drop(&mut self) {
            std::env::remove_var("LF_TEST_CLAUDE_USAGE_URL");
            std::env::remove_var("LF_TEST_CLAUDE_TOKEN_URL");
            std::env::remove_var("LF_TEST_CLAUDE_PROFILE_URL");
        }
    }

    pub(crate) async fn serve(
        responses: Vec<(u16, String)>,
        before_response: impl Fn(usize) + Send + 'static,
    ) -> (Endpoints, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        std::env::set_var("LF_TEST_CLAUDE_USAGE_URL", &endpoint);
        std::env::set_var("LF_TEST_CLAUDE_TOKEN_URL", &endpoint);
        std::env::set_var("LF_TEST_CLAUDE_PROFILE_URL", &endpoint);
        let task = tokio::spawn(async move {
            for (index, (status, body)) in responses.into_iter().enumerate() {
                let (mut stream, _) =
                    tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let count = stream.read(&mut buffer).await.unwrap();
                    assert_ne!(count, 0);
                    request.extend_from_slice(&buffer[..count]);
                }
                before_response(index);
                let response = format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (Endpoints, task)
    }

    fn credentials(home: &std::path::Path) -> String {
        let raw = json!({"claudeAiOauth": {
            "accessToken": "fixture-access", "refreshToken": "fixture-refresh",
            "expiresAt": 4102444800000i64, "subscriptionType": "max"
        }})
        .to_string();
        std::fs::write(home.join(".credentials.json"), &raw).unwrap();
        raw
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn claude_identity_comes_from_profile_not_cached_oauth_account() {
        let _env = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        credentials(home.path());
        std::fs::write(home.path().join(".claude.json"), json!({"oauthAccount":{"emailAddress":"stale@example.com", "accountUuid":"stale-user"}}).to_string()).unwrap();
        let (_endpoints, server) = serve(vec![(200, json!({"account":{"email":"actual@example.com", "uuid":"actual-user"}, "organization":{"uuid":"shared-team"}}).to_string())], |_| {}).await;
        let identity = super::claude_identity(home.path(), None).await.unwrap();
        server.await.unwrap();
        assert_eq!(identity.email, "actual@example.com");
        assert_eq!(identity.subject, "actual-user");
    }

    fn expired_credentials(home: &std::path::Path) {
        let raw = json!({"claudeAiOauth": {
            "accessToken": "expired-access", "refreshToken": "fixture-refresh", "expiresAt": 1000
        }});
        std::fs::write(home.join(".credentials.json"), raw.to_string()).unwrap();
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn a_refresh_waits_for_claudes_lock_and_takes_the_login_claude_refreshed() {
        let _env = crate::journal::test_env_lock();
        std::env::remove_var("LF_TEST_CLAUDE_TOKEN_URL");
        let home = tempfile::tempdir().unwrap();
        expired_credentials(home.path());
        let lock = home.path().join(".oauth_refresh.lock");
        std::fs::create_dir(&lock).unwrap();
        let claude = {
            let home = home.path().to_path_buf();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                let refreshed = json!({"claudeAiOauth": {
                    "accessToken": "claude-access", "refreshToken": "claude-refresh",
                    "expiresAt": 4102444800000i64
                }});
                std::fs::write(home.join(".credentials.json"), refreshed.to_string()).unwrap();
                std::fs::remove_dir(home.join(".oauth_refresh.lock")).unwrap();
            })
        };
        // Refreshing without the lock would reach the unroutable token URL.
        let access = super::claude_access_token(home.path(), 60).await.unwrap();
        claude.join().unwrap();
        assert_eq!(access.token.expose_secret(), "claude-access");
        assert!(!lock.exists());
        let stored = std::fs::read_to_string(home.path().join(".credentials.json")).unwrap();
        assert!(stored.contains("claude-refresh"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn a_caller_holding_the_refresh_lock_refreshes_without_waiting_on_itself() {
        let _env = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        expired_credentials(home.path());
        let held =
            crate::provider_account::activation::ClaudeRefreshLock::acquire(home.path()).unwrap();
        let (_endpoints, server) = serve(
            vec![
                (
                    200,
                    json!({"access_token":"replacement", "expires_in":3600}).to_string(),
                ),
                (
                    200,
                    json!({"account":{"email":"actual@example.com", "uuid":"actual-user"}})
                        .to_string(),
                ),
            ],
            |_| {},
        )
        .await;
        let identity = super::claude_identity(home.path(), Some(&held))
            .await
            .unwrap();
        server.await.unwrap();
        assert_eq!(identity.email, "actual@example.com");
        let stored = std::fs::read_to_string(home.path().join(".credentials.json")).unwrap();
        assert!(stored.contains("replacement"));
    }

    #[test]
    fn unknown_percentages_stay_unknown_and_numbers_round_once() {
        for value in [Value::Null, json!("22"), json!({}), json!(true)] {
            assert_eq!(percentage(&value), None);
        }
        for (value, expected) in [(22.49, 22), (22.5, 23), (-1.0, 0), (101.0, 100)] {
            assert_eq!(percentage(&json!(value)), Some(expected));
        }
        let windows = claude_windows(&json!({"limits": [
            {"group":"session", "percent": 22.5},
            {"group":"weekly", "percent": null},
            {"group":"weekly", "percent": "unknown"}
        ]}));
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].used_percent, 23);
        assert_eq!(windows[0].resets_at, None);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn unexpired_rejection_refreshes_once_and_retains_omitted_refresh_token() {
        let _env = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        credentials(home.path());
        let (_endpoints, server) = serve(
            vec![
                (401, "{}".into()),
                (
                    200,
                    json!({"access_token":"replacement", "expires_in":3600}).to_string(),
                ),
                (
                    200,
                    json!({"limits":[{"group":"session", "percent":22.5}]}).to_string(),
                ),
                (
                    200,
                    json!({"account":{"email":"actual@example.com", "uuid":"actual-user"}})
                        .to_string(),
                ),
            ],
            |_| {},
        )
        .await;
        let usage = poll_claude(home.path()).await.unwrap();
        server.await.unwrap();
        assert_eq!(usage.windows[0].used_percent, 23);
        assert_eq!(usage.plan, None);
        assert_eq!(usage.windows[0].plan, None);
        let persisted: Value =
            serde_json::from_slice(&std::fs::read(home.path().join(".credentials.json")).unwrap())
                .unwrap();
        assert_eq!(persisted["claudeAiOauth"]["accessToken"], "replacement");
        assert_eq!(
            persisted["claudeAiOauth"]["refreshToken"],
            "fixture-refresh"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(home.path().join(".credentials.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn refresh_does_not_overwrite_a_concurrent_credential_replacement() {
        let _env = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        credentials(home.path());
        let path = home.path().join(".credentials.json");
        let replace_path = path.clone();
        let (_endpoints, server) = serve(
            vec![
                (401, "{}".into()),
                (200, json!({"access_token":"stale-refresh"}).to_string()),
            ],
            move |index| {
                if index == 1 {
                    std::fs::write(&replace_path, r#"{"accessToken":"concurrent-login"}"#).unwrap();
                }
            },
        )
        .await;
        let result = poll_claude(home.path()).await.unwrap_err();
        server.await.unwrap();
        assert!(matches!(result, SubscriptionError::Unavailable(_)));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            r#"{"accessToken":"concurrent-login"}"#
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn usage_outages_and_unknown_payloads_do_not_reject_credentials() {
        let _env = crate::journal::test_env_lock();
        for (status, body) in [
            (403, "secret-provider-body"),
            (429, "secret-provider-body"),
            (500, "secret-provider-body"),
            (200, "{}"),
            (200, "null"),
            (200, "not-json"),
        ] {
            let home = tempfile::tempdir().unwrap();
            let original = credentials(home.path());
            let (_endpoints, server) = serve(vec![(status, body.into())], |_| {}).await;
            match poll_claude(home.path()).await {
                Ok(usage) => {
                    assert_eq!(status, 200);
                    assert!(usage.windows.is_empty());
                    assert_eq!(usage.plan, None);
                }
                Err(error) => {
                    assert!(matches!(error, SubscriptionError::Unavailable(_)));
                    assert!(!error.to_string().contains("secret-provider-body"));
                }
            }
            server.await.unwrap();
            assert_eq!(
                std::fs::read_to_string(home.path().join(".credentials.json")).unwrap(),
                original
            );
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn refresh_rejection_is_decisive_only_for_invalid_grant_or_unauthorized() {
        let _env = crate::journal::test_env_lock();
        for (status, error_code, rejected) in [
            (400, "invalid_grant", true),
            (401, "private-provider-message", true),
            (400, "invalid_request", false),
            (403, "invalid_grant", false),
            (503, "invalid_grant", false),
        ] {
            let home = tempfile::tempdir().unwrap();
            let original = credentials(home.path());
            let (_endpoints, server) = serve(
                vec![
                    (401, "{}".into()),
                    (
                        status,
                        json!({"error":error_code, "message":"never print this"}).to_string(),
                    ),
                ],
                |_| {},
            )
            .await;
            let error = poll_claude(home.path()).await.unwrap_err();
            server.await.unwrap();
            assert_eq!(matches!(error, SubscriptionError::NeedsLogin(_)), rejected);
            assert!(!error.to_string().contains("private-provider-message"));
            assert!(!error.to_string().contains("never print this"));
            assert_eq!(
                std::fs::read_to_string(home.path().join(".credentials.json")).unwrap(),
                original
            );
        }
    }
}
