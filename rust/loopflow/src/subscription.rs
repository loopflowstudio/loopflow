//! On-demand subscription usage for managed provider accounts.
//!
//! Answers "how much of this account's plan is left" by asking the provider
//! directly: Claude through its OAuth usage endpoint (refreshing the stored
//! token when it has expired), Codex through a one-shot `codex app-server`
//! JSON-RPC exchange against the account's home. Results are persisted to
//! `provider_account_limits` by live auth status. Cached auth reads
//! never poll; `lf usage` separately reports Run token/cost evidence.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::store::{AccountLimitWindow, ProviderAccount};

const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const CLAUDE_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
/// Claude Code's public OAuth client id — the tokens in an imported account
/// home were minted for it, so refreshes must present the same client.
const CLAUDE_OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const CODEX_READ_TIMEOUT: Duration = Duration::from_secs(20);

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
    pub(crate) identity: crate::provider_account::identity::AccountIdentity,
}

/// Poll one managed account's provider for its live subscription state.
pub async fn poll_account(
    account: &ProviderAccount,
) -> Result<SubscriptionUsage, SubscriptionError> {
    let home = account.home.as_deref().ok_or_else(|| {
        SubscriptionError::Unavailable("account has no managed credential home".to_string())
    })?;
    match account.provider.as_str() {
        "claude" => poll_claude(home).await,
        "codex" => {
            crate::provider_account::identity::check_account_identity(account, &[])
                .map_err(SubscriptionError::NeedsLogin)?;
            let usage = poll_codex(home).await?;
            crate::provider_account::identity::check_account_identity(account, &[])
                .map_err(SubscriptionError::NeedsLogin)?;
            Ok(usage)
        }
        other => Err(SubscriptionError::Unavailable(format!(
            "no subscription poll for provider '{other}'"
        ))),
    }
}

// -- Claude ------------------------------------------------------------------

async fn poll_claude(home: &Path) -> Result<SubscriptionUsage, SubscriptionError> {
    let credentials_path = home.join(".credentials.json");
    let client = reqwest::Client::new();
    let (mut token, mut credential, refreshed) =
        fresh_claude_token(&client, &credentials_path, false).await?;
    let mut response = request_claude_usage(&client, token.expose_secret()).await?;
    ensure_claude_credential_unchanged(&credentials_path, credential.expose_secret())?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED && !refreshed {
        (token, credential, _) = fresh_claude_token(&client, &credentials_path, true).await?;
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
            identity: request_claude_identity(&client, token.expose_secret()).await?,
        })
    }
    .await;
    // A result describes the credential used for this request, not a replacement
    // installed by a native provider while the request was in flight.
    ensure_claude_credential_unchanged(&credentials_path, credential.expose_secret())?;
    observation
}

pub(crate) async fn claude_identity(
    home: &Path,
) -> Result<crate::provider_account::identity::AccountIdentity, SubscriptionError> {
    let path = home.join(".credentials.json");
    let client = reqwest::Client::new();
    let (token, credential, _) = fresh_claude_token(&client, &path, false).await?;
    let identity = request_claude_identity(&client, token.expose_secret()).await?;
    ensure_claude_credential_unchanged(&path, credential.expose_secret())?;
    Ok(identity)
}

async fn request_claude_identity(
    client: &reqwest::Client,
    token: &str,
) -> Result<crate::provider_account::identity::AccountIdentity, SubscriptionError> {
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
        (Some(email), Some(subject)) => Ok(crate::provider_account::identity::AccountIdentity {
            email: email.into(),
            subject: subject.into(),
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
                    .unwrap_or_else(|_| CLAUDE_USAGE_URL.to_string())
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
            let window = match (group, scope) {
                ("session", _) => "session".to_string(),
                ("weekly", None) => "weekly".to_string(),
                ("weekly", Some(model)) => format!("weekly:{}", model.to_lowercase()),
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
            })
        })
        .collect()
}

/// The caller holds the managed login lock through polling and persistence.
/// Comparisons detect completed replacements; they do not serialize native
/// provider writes or competing server-side refreshes.
/// Returns the access token, its serialized credential, and whether refreshed.
async fn fresh_claude_token(
    client: &reqwest::Client,
    credentials_path: &Path,
    force_refresh: bool,
) -> Result<(SecretString, SecretString, bool), SubscriptionError> {
    let raw = std::fs::read_to_string(credentials_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SubscriptionError::NeedsLogin("no stored credentials".into())
        } else {
            SubscriptionError::Unavailable("cannot read stored credentials".into())
        }
    })?;
    let mut credentials: Value = serde_json::from_str(&raw)
        .map_err(|_| SubscriptionError::Unavailable("unreadable credential format".into()))?;
    let oauth = credentials.get("claudeAiOauth").unwrap_or(&credentials);
    let access_token = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| SubscriptionError::NeedsLogin("no access token".into()))?;
    let expires_at_ms = oauth.get("expiresAt").and_then(Value::as_i64);
    let now_ms = OffsetDateTime::now_utc().unix_timestamp() * 1000;
    // Missing expiry is unknown; let the server judge the token.
    if !force_refresh && expires_at_ms.is_none_or(|expiry| expiry > now_ms + 60_000) {
        return Ok((
            SecretString::new(access_token.to_string()),
            SecretString::new(raw),
            false,
        ));
    }
    let refresh_token = oauth
        .get("refreshToken")
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            SubscriptionError::NeedsLogin("expired or rejected token has no refresh token".into())
        })?;
    let response = client.post({
            #[cfg(test)]
            { std::env::var("LF_TEST_CLAUDE_TOKEN_URL").unwrap_or_else(|_| CLAUDE_TOKEN_URL.to_string()) }
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
    ensure_claude_credential_unchanged(credentials_path, &raw)?;
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
        .and_then(|duration| now_ms.checked_add(duration))
    {
        oauth["expiresAt"] = json!(expiry);
    }
    persist_claude_refresh(credentials_path, &raw, &credentials)?;
    Ok((
        SecretString::new(new_access),
        SecretString::new(credentials.to_string()),
        true,
    ))
}

fn persist_claude_refresh(
    path: &Path,
    original: &str,
    credentials: &Value,
) -> Result<(), SubscriptionError> {
    ensure_claude_credential_unchanged(path, original)?;
    let home = path.parent().expect("credential file has a parent");
    crate::provider_auth::write_claude_profile_credentials(
        home,
        &SecretString::new(credentials.to_string()),
    )
    .map_err(|_| SubscriptionError::Unavailable("failed to persist refreshed credential".into()))
}

fn ensure_claude_credential_unchanged(
    path: &Path,
    original: &str,
) -> Result<(), SubscriptionError> {
    let current = std::fs::read_to_string(path).map_err(|_| {
        SubscriptionError::Unavailable(
            "credential changed during verification; retry verification".into(),
        )
    })?;
    if current != original {
        return Err(SubscriptionError::Unavailable(
            "credential changed during verification; retry verification".into(),
        ));
    }
    Ok(())
}

// -- Codex -------------------------------------------------------------------

/// One-shot JSON-RPC exchange with `codex app-server`: initialize, then
/// `account/rateLimits/read`. The server refreshes its own token from the
/// account home, so a stale-but-valid credential still answers; a revoked one
/// fails with `token_invalidated`, which is a re-login, not an outage.
async fn poll_codex(home: &Path) -> Result<SubscriptionUsage, SubscriptionError> {
    let mut child = tokio::process::Command::new("codex")
        .args(["-c", "cli_auth_credentials_store=\"file\"", "app-server"])
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env("CODEX_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| SubscriptionError::Unavailable(format!("codex unavailable: {error}")))?;

    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut lines = BufReader::new(stdout).lines();

    let exchange = async {
        stdin
            .write_all(
                format!(
                    "{}\n",
                    json!({
                        "jsonrpc": "2.0", "id": 1, "method": "initialize",
                        "params": {"clientInfo": {"name": "lf", "title": "loopflow", "version": env!("CARGO_PKG_VERSION")}}
                    })
                )
                .as_bytes(),
            )
            .await?;
        let mut sent_read = false;
        let mut account_response = None;
        while let Some(line) = lines.next_line().await? {
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match message.get("id").and_then(Value::as_i64) {
                Some(1) if !sent_read => {
                    sent_read = true;
                    stdin.write_all(b"{\"method\":\"initialized\"}\n").await?;
                    stdin
                        .write_all(
                            format!(
                                "{}\n",
                                json!({"jsonrpc": "2.0", "id": 2, "method": "account/read", "params": {"refreshToken": true}})
                            )
                            .as_bytes(),
                        )
                        .await?;
                }
                Some(2) => {
                    account_response = Some(message);
                    stdin.write_all(format!("{}\n", json!({"jsonrpc":"2.0", "id":3, "method":"account/rateLimits/read", "params":{}})).as_bytes()).await?;
                }
                Some(3) => return Ok(Some((account_response, message))),
                _ => {}
            }
        }
        Ok::<Option<(Option<Value>, Value)>, std::io::Error>(None)
    };

    let response = tokio::time::timeout(CODEX_READ_TIMEOUT, exchange).await;
    let _ = child.kill().await;
    let response = response
        .map_err(|_| SubscriptionError::Unavailable("codex app-server timed out".to_string()))?
        .map_err(|error| SubscriptionError::Unavailable(error.to_string()))?
        .ok_or_else(|| {
            SubscriptionError::Unavailable("codex app-server closed without answering".to_string())
        })?;

    let (account_response, response) = response;
    for (response, request) in [
        (account_response.as_ref().unwrap_or(&Value::Null), "account"),
        (&response, "rate-limit"),
    ] {
        if let Some(error) = response.get("error") {
            let message = error.get("message").and_then(Value::as_str).unwrap_or("");
            if message.contains("token_invalidated")
                || error.get("code").and_then(Value::as_i64) == Some(401)
            {
                return Err(SubscriptionError::NeedsLogin("credential revoked".into()));
            }
            return Err(SubscriptionError::Unavailable(format!(
                "codex {request} request failed"
            )));
        }
        if !response.get("result").is_some_and(Value::is_object) {
            return Err(SubscriptionError::Unavailable(format!(
                "codex returned an invalid {request} response"
            )));
        }
    }
    let account_response = account_response
        .as_ref()
        .and_then(|response| response.get("result"))
        .unwrap_or(&Value::Null);
    let identity = crate::provider_auth::codex_identity_from_account(home, account_response)
        .map_err(SubscriptionError::NeedsLogin)?;
    let snapshot = response.pointer("/result/rateLimits");
    let windows = codex_windows(snapshot);
    let plan = account_response
        .pointer("/account/planType")
        .or_else(|| snapshot.and_then(|value| value.get("planType")))
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(SubscriptionUsage {
        windows,
        plan,
        identity,
    })
}

fn percentage(value: &Value) -> Option<u8> {
    value
        .as_f64()
        .filter(|value| value.is_finite())
        .map(|value| value.round().clamp(0.0, 100.0) as u8)
}

fn codex_windows(rate_limits: Option<&Value>) -> Vec<AccountLimitWindow> {
    let Some(snapshot) = rate_limits else {
        return Vec::new();
    };
    let plan = snapshot.get("planType").and_then(Value::as_str);
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| snapshot.get(*key))
        .filter(|value| !value.is_null())
        .filter_map(|value| {
            let used_percent = percentage(value.get("usedPercent")?)?;
            Some(AccountLimitWindow {
                window: crate::harness::codex_window_name(
                    value.get("windowDurationMins").and_then(Value::as_u64),
                )
                .to_string(),
                used_percent,
                resets_at: value.get("resetsAt").and_then(Value::as_i64),
                plan: plan.map(str::to_string),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{claude_windows, codex_windows};
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
        assert_eq!(windows[0].window, "session");
        assert_eq!(windows[0].used_percent, 22);
        assert!(windows[0].resets_at.is_some());
        assert_eq!(windows[0].plan, None);
        assert_eq!(windows[1].window, "weekly");
        assert_eq!(windows[2].window, "weekly:fable");
        assert_eq!(windows[2].used_percent, 11);
    }

    #[test]
    fn codex_rate_limits_map_plan_and_weekly_window() {
        let windows = codex_windows(Some(&json!({
            "limitId": "codex",
            "primary": {"usedPercent": 78, "windowDurationMins": 10080, "resetsAt": 1784780166},
            "secondary": null,
            "planType": "pro"
        })));
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
        })));
        assert_eq!(windows[0].window, "session");
    }
}

#[cfg(test)]
pub(crate) mod observation_tests {
    use super::{claude_windows, percentage, poll_claude, SubscriptionError};
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
        let identity = super::claude_identity(home.path()).await.unwrap();
        server.await.unwrap();
        assert_eq!(identity.email, "actual@example.com");
        assert_eq!(identity.subject, "actual-user");
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
