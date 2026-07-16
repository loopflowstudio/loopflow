//! On-demand subscription usage for managed provider accounts.
//!
//! Answers "how much of this account's plan is left" by asking the provider
//! directly: Claude through its OAuth usage endpoint (refreshing the stored
//! token when it has expired), Codex through a one-shot `codex app-server`
//! JSON-RPC exchange against the account's home. Results are persisted to
//! `provider_account_limits` so `lf usage` reads instantly and only polls
//! windows that have gone stale.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

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
/// or expired beyond refresh, and only `lf auth connect` fixes that.
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
}

/// Poll one managed account's provider for its live subscription state.
pub async fn poll_account(
    account: &ProviderAccount,
) -> Result<SubscriptionUsage, SubscriptionError> {
    let home = account.home.clone().ok_or_else(|| {
        SubscriptionError::Unavailable("account has no managed credential home".to_string())
    })?;
    match account.provider.as_str() {
        "claude" => poll_claude(&home).await,
        "codex" => poll_codex(&home).await,
        other => Err(SubscriptionError::Unavailable(format!(
            "no subscription poll for provider '{other}'"
        ))),
    }
}

// -- Claude ------------------------------------------------------------------

async fn poll_claude(home: &Path) -> Result<SubscriptionUsage, SubscriptionError> {
    let credentials_path = home.join(".credentials.json");
    let access_token = fresh_claude_token(&credentials_path).await?;
    let response = reqwest::Client::new()
        .get(CLAUDE_USAGE_URL)
        .bearer_auth(&access_token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| SubscriptionError::Unavailable(error.to_string()))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SubscriptionError::NeedsLogin(
            "usage endpoint rejected the refreshed token".to_string(),
        ));
    }
    if !response.status().is_success() {
        return Err(SubscriptionError::Unavailable(format!(
            "usage endpoint returned {}",
            response.status()
        )));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|error| SubscriptionError::Unavailable(error.to_string()))?;
    Ok(SubscriptionUsage {
        windows: claude_windows(&body),
    })
}

/// Parse the OAuth usage payload's `limits` array: one entry per rate-limit
/// window, each with a percent, a reset time, and an optional model scope.
fn claude_windows(body: &Value) -> Vec<AccountLimitWindow> {
    let Some(limits) = body.get("limits").and_then(Value::as_array) else {
        return Vec::new();
    };
    limits
        .iter()
        .filter_map(|limit| {
            let percent = limit.get("percent").and_then(Value::as_u64)?;
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
                used_percent: percent.min(100) as u8,
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

/// Return a live access token for the account home, refreshing through the
/// OAuth token endpoint when the stored one has expired. A successful refresh
/// rotates the refresh token, so the new pair is written back immediately —
/// dropping it would strand the account.
async fn fresh_claude_token(credentials_path: &Path) -> Result<String, SubscriptionError> {
    let raw = tokio::fs::read_to_string(credentials_path)
        .await
        .map_err(|_| SubscriptionError::NeedsLogin("no stored credentials".to_string()))?;
    let mut credentials: Value = serde_json::from_str(&raw)
        .map_err(|_| SubscriptionError::NeedsLogin("unreadable credentials".to_string()))?;
    let oauth = credentials
        .get("claudeAiOauth")
        .ok_or_else(|| SubscriptionError::NeedsLogin("no OAuth credentials".to_string()))?;
    let access_token = oauth
        .get("accessToken")
        .and_then(Value::as_str)
        .ok_or_else(|| SubscriptionError::NeedsLogin("no access token".to_string()))?;
    let expires_at_ms = oauth.get("expiresAt").and_then(Value::as_i64).unwrap_or(0);
    let now_ms = OffsetDateTime::now_utc().unix_timestamp() * 1000;
    if expires_at_ms > now_ms + 60_000 {
        return Ok(access_token.to_string());
    }

    let refresh_token = oauth
        .get("refreshToken")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SubscriptionError::NeedsLogin("token expired, no refresh token".to_string())
        })?
        .to_string();
    let response = reqwest::Client::new()
        .post(CLAUDE_TOKEN_URL)
        .json(&json!({
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
            "client_id": CLAUDE_OAUTH_CLIENT_ID,
        }))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| SubscriptionError::Unavailable(error.to_string()))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|error| SubscriptionError::Unavailable(error.to_string()))?;
    if status == reqwest::StatusCode::BAD_REQUEST || status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SubscriptionError::NeedsLogin(
            body.pointer("/error")
                .and_then(Value::as_str)
                .unwrap_or("refresh rejected")
                .to_string(),
        ));
    }
    if !status.is_success() {
        return Err(SubscriptionError::Unavailable(format!(
            "token refresh returned {status}"
        )));
    }
    let new_access = body
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| SubscriptionError::Unavailable("refresh returned no token".to_string()))?
        .to_string();
    let oauth = credentials.get_mut("claudeAiOauth").expect("checked above");
    oauth["accessToken"] = json!(new_access);
    if let Some(new_refresh) = body.get("refresh_token").and_then(Value::as_str) {
        oauth["refreshToken"] = json!(new_refresh);
    }
    if let Some(expires_in) = body.get("expires_in").and_then(Value::as_i64) {
        oauth["expiresAt"] = json!(now_ms + expires_in * 1000);
    }
    tokio::fs::write(
        credentials_path,
        serde_json::to_string(&credentials).expect("credentials serialize"),
    )
    .await
    .map_err(|error| {
        SubscriptionError::Unavailable(format!("failed to persist refreshed token: {error}"))
    })?;
    Ok(new_access)
}

// -- Codex -------------------------------------------------------------------

/// One-shot JSON-RPC exchange with `codex app-server`: initialize, then
/// `account/rateLimits/read`. The server refreshes its own token from the
/// account home, so a stale-but-valid credential still answers; a revoked one
/// fails with `token_invalidated`, which is a re-login, not an outage.
async fn poll_codex(home: &Path) -> Result<SubscriptionUsage, SubscriptionError> {
    let mut child = tokio::process::Command::new("codex")
        .arg("app-server")
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
        while let Some(line) = lines.next_line().await? {
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            match message.get("id").and_then(Value::as_i64) {
                Some(1) if !sent_read => {
                    sent_read = true;
                    stdin
                        .write_all(
                            format!(
                                "{}\n",
                                json!({"jsonrpc": "2.0", "id": 2, "method": "account/rateLimits/read", "params": {}})
                            )
                            .as_bytes(),
                        )
                        .await?;
                }
                Some(2) => return Ok(Some(message)),
                _ => {}
            }
        }
        Ok::<Option<Value>, std::io::Error>(None)
    };

    let response = tokio::time::timeout(CODEX_READ_TIMEOUT, exchange).await;
    let _ = child.kill().await;
    let response = response
        .map_err(|_| SubscriptionError::Unavailable("codex app-server timed out".to_string()))?
        .map_err(|error| SubscriptionError::Unavailable(error.to_string()))?
        .ok_or_else(|| {
            SubscriptionError::Unavailable("codex app-server closed without answering".to_string())
        })?;

    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("codex error");
        if message.contains("token_invalidated") || message.contains("401") {
            return Err(SubscriptionError::NeedsLogin(
                "credential revoked".to_string(),
            ));
        }
        return Err(SubscriptionError::Unavailable(message.to_string()));
    }
    Ok(SubscriptionUsage {
        windows: codex_windows(response.pointer("/result/rateLimits")),
    })
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
            let used_percent = value.get("usedPercent").and_then(Value::as_u64)?;
            Some(AccountLimitWindow {
                window: crate::harness::codex_window_name(
                    value.get("windowDurationMins").and_then(Value::as_u64),
                )
                .to_string(),
                used_percent: used_percent.min(100) as u8,
                resets_at: value.get("resetsAt").and_then(Value::as_i64),
                plan: plan.map(str::to_string),
            })
        })
        .collect()
}

/// Where each managed account's credential home lives, for `poll_account`
/// callers that only have the store row.
pub fn account_home(provider: &str, account_id: &str) -> PathBuf {
    crate::store::lf_home_dir()
        .join("accounts")
        .join(provider)
        .join(account_id)
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
