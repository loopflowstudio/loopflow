use std::path::Path;
use std::process::Command;

use hmac::{Hmac, Mac};
use reqwest::header::{ACCEPT, USER_AGENT};
use reqwest::Method;
use serde::Deserialize;
use sha2::Sha256;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::lfd::http_client::SafeHttpClient;
use crate::lfd::types::{LivePrState, LivePullRequestState};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubCheckRunEvent {
    pub action: String,
    pub check_run: CheckRun,
    pub repository: GitHubRepository,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubPullRequestEvent {
    pub action: String,
    pub pull_request: GitHubWebhookPullRequest,
    pub repository: GitHubRepository,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubWebhookPullRequest {
    pub number: u32,
    #[serde(default)]
    pub merged: bool,
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRun {
    pub id: u64,
    pub name: String,
    pub head_sha: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub pull_requests: Vec<CheckRunPR>,
    pub html_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRunPR {
    pub number: u32,
    pub head: CheckRunRef,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRunRef {
    #[serde(rename = "ref")]
    pub branch: String,
    pub sha: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRepository {
    pub full_name: String,
}

#[derive(Debug, Deserialize)]
struct CheckRunsResponse {
    check_runs: Vec<CheckRun>,
}

#[derive(Debug, Clone)]
pub struct GitHubPullRequestState {
    pub number: u32,
    pub state: LivePrState,
    pub is_draft: bool,
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    pub updated_at: OffsetDateTime,
    pub merged_at: Option<OffsetDateTime>,
}

pub fn into_live_pull_request_state(
    repo_id: String,
    pull_request: GitHubPullRequestState,
    synced_at: OffsetDateTime,
) -> LivePullRequestState {
    LivePullRequestState {
        repo_id,
        pr_number: pull_request.number,
        state: pull_request.state,
        is_draft: pull_request.is_draft,
        head_ref: pull_request.head_ref,
        head_sha: pull_request.head_sha,
        base_ref: pull_request.base_ref,
        updated_at: pull_request.updated_at,
        merged_at: pull_request.merged_at,
        synced_at,
    }
}

#[derive(Debug, Deserialize)]
struct PullRequestResponse {
    number: u32,
    state: String,
    #[serde(default)]
    draft: bool,
    head: PullRequestRef,
    base: PullRequestRef,
    updated_at: String,
    merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PullRequestRef {
    #[serde(rename = "ref")]
    branch: String,
    sha: String,
}

pub fn verify_webhook_signature(secret: &str, body: &[u8], signature_header: &str) -> bool {
    if secret.trim().is_empty() {
        return false;
    }

    let Some(signature) = signature_header.trim().strip_prefix("sha256=") else {
        return false;
    };
    let Ok(signature) = hex::decode(signature) else {
        return false;
    };

    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&signature).is_ok()
}

pub fn github_repo_from_local(repo_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let remote = String::from_utf8_lossy(&output.stdout).trim().to_string();
    github_repo_from_remote_url(&remote)
}

pub fn github_repo_from_remote_url(remote: &str) -> Option<String> {
    let mut value = remote.trim().trim_end_matches('/').to_string();
    if value.is_empty() {
        return None;
    }
    if let Some(stripped) = value.strip_suffix(".git") {
        value = stripped.to_string();
    }

    if let Some(rest) = value.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        if !host.eq_ignore_ascii_case("github.com") {
            return None;
        }
        return owner_repo_from_path(path);
    }

    if let Some((_scheme, rest)) = value.split_once("://") {
        let (host, path) = rest.split_once('/')?;
        let host = host.rsplit('@').next().unwrap_or(host);
        if !host.eq_ignore_ascii_case("github.com") {
            return None;
        }
        return owner_repo_from_path(path);
    }

    None
}

fn owner_repo_from_path(path: &str) -> Option<String> {
    let mut parts = path.split('/').filter(|part| !part.trim().is_empty());
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim().trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

pub async fn poll_check_runs(
    repo_full_name: &str,
    branch: &str,
    token: &str,
) -> Result<Vec<CheckRun>, String> {
    let branch = branch.replace('/', "%2F");
    let url = format!("https://api.github.com/repos/{repo_full_name}/commits/{branch}/check-runs");
    let client = SafeHttpClient::new().map_err(|err| err.to_string())?;
    let response = client
        .send(
            client
                .request(Method::GET, &url)
                .map_err(|err| err.to_string())?
                .header(ACCEPT, "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header(USER_AGENT, "loopflow-lfd")
                .bearer_auth(token),
        )
        .await
        .map_err(|err| err.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("github API error {status}: {body}"));
    }

    response
        .json::<CheckRunsResponse>()
        .await
        .map(|data| data.check_runs)
        .map_err(|err| err.to_string())
}

pub async fn fetch_pull_request(
    repo_full_name: &str,
    pr_number: u32,
    token: &str,
) -> Result<Option<GitHubPullRequestState>, String> {
    let url = format!("https://api.github.com/repos/{repo_full_name}/pulls/{pr_number}");
    let client = SafeHttpClient::new().map_err(|err| err.to_string())?;
    let response = client
        .send(
            client
                .request(Method::GET, &url)
                .map_err(|err| err.to_string())?
                .header(ACCEPT, "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header(USER_AGENT, "loopflow-lfd")
                .bearer_auth(token),
        )
        .await
        .map_err(|err| err.to_string())?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("github API error {status}: {body}"));
    }

    let payload = response
        .json::<PullRequestResponse>()
        .await
        .map_err(|err| err.to_string())?;

    let updated_at = OffsetDateTime::parse(&payload.updated_at, &Rfc3339)
        .map_err(|err| format!("invalid updated_at timestamp: {err}"))?;
    let merged_at = payload
        .merged_at
        .as_deref()
        .map(|value| OffsetDateTime::parse(value, &Rfc3339))
        .transpose()
        .map_err(|err| format!("invalid merged_at timestamp: {err}"))?;

    let state = if merged_at.is_some() {
        LivePrState::Merged
    } else if payload.state.eq_ignore_ascii_case("open") {
        LivePrState::Open
    } else if payload.state.eq_ignore_ascii_case("closed") {
        LivePrState::Closed
    } else {
        LivePrState::Unknown
    };

    Ok(Some(GitHubPullRequestState {
        number: payload.number,
        state,
        is_draft: payload.draft,
        head_ref: payload.head.branch,
        head_sha: payload.head.sha,
        base_ref: payload.base.branch,
        updated_at,
        merged_at,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_webhook_signature_accepts_valid_signature() {
        let secret = "topsecret";
        let body = br#"{"hello":"world"}"#;
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("mac");
        mac.update(body);
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        assert!(verify_webhook_signature(secret, body, &signature));
    }

    #[test]
    fn verify_webhook_signature_rejects_invalid_signature() {
        assert!(!verify_webhook_signature(
            "secret",
            b"{}",
            "sha256=deadbeef"
        ));
        assert!(!verify_webhook_signature("secret", b"{}", "bad-format"));
    }

    #[test]
    fn github_repo_from_remote_url_parses_common_formats() {
        assert_eq!(
            github_repo_from_remote_url("git@github.com:loopflowstudio/loopflow.git"),
            Some("loopflowstudio/loopflow".to_string())
        );
        assert_eq!(
            github_repo_from_remote_url("https://github.com/loopflowstudio/loopflow"),
            Some("loopflowstudio/loopflow".to_string())
        );
        assert_eq!(
            github_repo_from_remote_url("ssh://git@github.com/LoopflowStudio/Loopflow.git"),
            Some("LoopflowStudio/Loopflow".to_string())
        );
    }
}
