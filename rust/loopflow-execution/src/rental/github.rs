//! The GitHub token a repository's rentals hold: a GitHub App user token that
//! acts as the person in that one repository and expires 8 hours after issue.
//! It is approved once per repository by device code and renewed here; the
//! grant that renews it never leaves this Machine.
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use time::OffsetDateTime;

use crate::config::placement::Approver;
use crate::machine::login::{self, Page, Sent};
use crate::provider_auth::Provider;
use loopflow_storage::rental::GithubRentalGrant;
use loopflow_storage::sqlite::SqliteStore;

/// The public "Loopflow Studio" GitHub App. Device flow needs no secret, so
/// this id is all `lf` carries of it.
const APP_CLIENT_ID: &str = "Iv23likXtOBTpX1WyV4f";
/// Where a person installs the app on a repository.
pub const APP_URL: &str = "https://github.com/apps/loopflow-studio";

const REQUEST_DEADLINE: Duration = Duration::from_secs(15);

#[cfg(test)]
tokio::task_local! {
    /// The stand-in serving both github.com and api.github.com paths.
    static TEST_GITHUB_URL: String;
}

fn web(path: &str) -> String {
    #[cfg(test)]
    let base = TEST_GITHUB_URL
        .try_with(Clone::clone)
        .unwrap_or_else(|_| "http://127.0.0.1:1".into());
    #[cfg(not(test))]
    let base = "https://github.com";
    format!("{base}{path}")
}

fn api(path: &str) -> String {
    #[cfg(test)]
    let base = TEST_GITHUB_URL
        .try_with(Clone::clone)
        .unwrap_or_else(|_| "http://127.0.0.1:1".into());
    #[cfg(not(test))]
    let base = "https://api.github.com";
    format!("{base}{path}")
}

/// GitHub's answer to an API read with `token`; `None` when it has no such
/// resource for this token.
async fn read(token: &str, path: &str) -> Result<Option<serde_json::Value>> {
    let response = reqwest::Client::new()
        .get(api(path))
        .header("User-Agent", "loopflow")
        .header("Accept", "application/vnd.github+json")
        .bearer_auth(token)
        .timeout(REQUEST_DEADLINE)
        .send()
        .await
        .map_err(|_| anyhow!("GitHub request failed"))?;
    match response.status() {
        status if status.is_success() => response
            .json()
            .await
            .map(Some)
            .map_err(|_| anyhow!("invalid GitHub response")),
        reqwest::StatusCode::NOT_FOUND => Ok(None),
        reqwest::StatusCode::UNAUTHORIZED => bail!("GitHub rejected the credential"),
        status => bail!("GitHub answered {status}"),
    }
}

/// The account `token` acts as.
pub(crate) async fn login_of(token: &str) -> Result<String> {
    read(token, "/user")
        .await?
        .and_then(|user| user["login"].as_str().map(str::to_string))
        .ok_or_else(|| anyhow!("GitHub identity missing login"))
}

/// A GitHub repository as GitHub names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub id: i64,
    /// `owner/name`.
    pub name: String,
}

/// The GitHub repository `repo`'s origin names, read with this Machine's own
/// `gh` token; `None` when the origin is not on GitHub.
async fn origin_repository(repo: &Path, token: &str) -> Result<Option<Repository>> {
    let Some((owner, name)) = crate::git::worktrees::github_repo_nwo(repo) else {
        return Ok(None);
    };
    let found = read(token, &format!("/repos/{owner}/{name}"))
        .await?
        .ok_or_else(|| {
            anyhow!("GitHub has no {owner}/{name} for the account `gh` is signed in to")
        })?;
    Ok(Some(Repository {
        id: found["id"]
            .as_i64()
            .ok_or_else(|| anyhow!("GitHub named no id for {owner}/{name}"))?,
        name: found["full_name"]
            .as_str()
            .map_or_else(|| format!("{owner}/{name}"), str::to_string),
    }))
}

// Deliberately no Debug: both tokens are credentials.
#[derive(Deserialize)]
struct Issued {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

/// A form post to github.com, answered as JSON.
fn post(path: &str, form: &[(&str, &str)]) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(web(path))
        .header("User-Agent", "loopflow")
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(serde_urlencoded::to_string(form).expect("form fields are strings"))
        .timeout(REQUEST_DEADLINE)
}

/// GitHub's reply to a token request: tokens, or the reason it gave none.
async fn token_request(form: &[(&str, &str)]) -> Result<std::result::Result<Issued, String>> {
    let reply: serde_json::Value = post("/login/oauth/access_token", form)
        .send()
        .await
        .map_err(|_| anyhow!("GitHub token request failed"))?
        .json()
        .await
        .map_err(|_| anyhow!("invalid GitHub token response"))?;
    if let Some(refusal) = reply["error"].as_str() {
        return Ok(Err(refusal.to_string()));
    }
    serde_json::from_value(reply)
        .map(Ok)
        .map_err(|_| anyhow!("invalid GitHub token response"))
}

/// Checks what GitHub issued before anything keeps it: an expiring, renewable
/// token for `login` that reaches `repository` and no other.
async fn grant_from(
    issued: Issued,
    repository: &Repository,
    login: &str,
) -> Result<GithubRentalGrant> {
    let (Some(refresh_token), Some(expires_in)) = (issued.refresh_token, issued.expires_in) else {
        bail!("GitHub issued a token that cannot be renewed; discarded");
    };
    let approved = login_of(&issued.access_token).await?;
    if approved != login {
        bail!(
            "GitHub was approved as {approved}, but `gh` here is signed in as {login}; \
             approve as {login}"
        );
    }
    let (mut reaches, mut others) = (false, 0_u64);
    let installations = read(&issued.access_token, "/user/installations?per_page=100")
        .await?
        .unwrap_or_default();
    for installation in installations["installations"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let id = installation["id"]
            .as_i64()
            .ok_or_else(|| anyhow!("invalid GitHub installation"))?;
        let listed = read(
            &issued.access_token,
            &format!("/user/installations/{id}/repositories?per_page=100"),
        )
        .await?
        .unwrap_or_default();
        let here = listed["repositories"]
            .as_array()
            .is_some_and(|all| all.iter().any(|found| found["id"] == repository.id));
        reaches |= here;
        others += listed["total_count"]
            .as_u64()
            .unwrap_or_default()
            .saturating_sub(u64::from(here));
    }
    if !reaches {
        bail!(
            "the Loopflow Studio app is not installed on {name}: install it at {APP_URL}, \
             then run this again for a new code",
            name = repository.name
        );
    }
    if others > 0 {
        bail!(
            "GitHub issued a token that reaches {others} more than {}; discarded",
            repository.name
        );
    }
    Ok(GithubRentalGrant {
        repository_id: repository.id,
        repository: repository.name.clone(),
        login: login.to_string(),
        access_token: issued.access_token,
        refresh_token,
        // Whole seconds, as the store keeps it.
        expires_at: OffsetDateTime::from_unix_timestamp(
            OffsetDateTime::now_utc().unix_timestamp() + expires_in,
        )?,
    })
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

/// One device approval for `repository`, stored as its grant. `login` is the
/// account this Machine's `gh` is signed in to; another account's approval is
/// discarded.
async fn approve_repository(
    sqlite: &SqliteStore,
    repository: &Repository,
    login: &str,
    approver: Option<&Approver>,
    batch: bool,
) -> Result<GithubRentalGrant> {
    let name = &repository.name;
    // A command approver needs no terminal; a browser needs a person at one.
    if !matches!(approver, Some(Approver::Command { .. }))
        && !crate::machine::credentials::foreground(batch)
    {
        bail!(
            "rentals have no GitHub approval for {name} and this run has no terminal to ask in; \
             run `lf ondemand login github` from a terminal in its checkout, or set a command \
             approver (`approvers.github`)"
        );
    }
    let device: DeviceCode = post(
        "/login/device/code",
        &[("client_id", APP_CLIENT_ID), ("scope", "offline_access")],
    )
    .send()
    .await
    .map_err(|_| anyhow!("GitHub device code request failed"))?
    .json()
    .await
    .map_err(|_| anyhow!("GitHub offered no device code"))?;
    eprintln!("Rentals need GitHub for {name}.");
    let page = Page {
        provider: Provider::GitHub,
        machine: "rentals",
        url: &device.verification_uri,
        code: Some(&device.user_code),
    };
    if let Sent::Opened(_) = login::send(approver, &page)? {
        eprintln!(
            "Approve at {} with code {}",
            device.verification_uri, device.user_code
        );
    }
    let repository_id = repository.id.to_string();
    let deadline = SystemTime::now() + Duration::from_secs(device.expires_in);
    let mut interval = Duration::from_secs(device.interval);
    let issued = loop {
        tokio::time::sleep(interval).await;
        let reply = token_request(&[
            ("client_id", APP_CLIENT_ID),
            ("device_code", &device.device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("repository_id", &repository_id),
        ])
        .await?;
        match reply {
            Ok(issued) => break issued,
            Err(wait) if wait == "authorization_pending" && SystemTime::now() < deadline => {}
            Err(wait) if wait == "slow_down" && SystemTime::now() < deadline => {
                interval += Duration::from_secs(5);
            }
            Err(refusal) if refusal == "access_denied" => {
                bail!("the GitHub approval for {name} was declined")
            }
            Err(_) => bail!("GitHub was not approved for {name} before its code expired"),
        }
    };
    let grant = grant_from(issued, repository, login).await?;
    sqlite.put_github_rental_grant(&grant)?;
    Ok(grant)
}

/// Renew `grant`, which ends the token it held: every rental holding that
/// token needs the new one. A refused renewal keeps the stored grant until an
/// approval replaces it.
pub async fn renew(sqlite: &SqliteStore, grant: &GithubRentalGrant) -> Result<GithubRentalGrant> {
    let repository = Repository {
        id: grant.repository_id,
        name: grant.repository.clone(),
    };
    let reply = token_request(&[
        ("client_id", APP_CLIENT_ID),
        ("grant_type", "refresh_token"),
        ("refresh_token", &grant.refresh_token),
    ])
    .await?;
    let issued = reply.map_err(|refusal| {
        anyhow!(
            "GitHub no longer renews rentals' access to {} ({refusal}); \
             run `lf ondemand login github` in its checkout to approve again",
            repository.name
        )
    })?;
    let renewed = grant_from(issued, &repository, &grant.login).await?;
    sqlite.put_github_rental_grant(&renewed)?;
    Ok(renewed)
}

/// `lf ondemand login github`: approve GitHub for the rentals of the
/// repository `repo` is a checkout of. Returns the repository and the account.
pub async fn approve(sqlite: &SqliteStore, repo: &Path, batch: bool) -> Result<(String, String)> {
    let laptop = crate::machine::credentials::gh_token()?;
    let token = secrecy::ExposeSecret::expose_secret(&laptop);
    let repository = origin_repository(repo, token)
        .await?
        .with_context(|| format!("{} has no GitHub origin", repo.display()))?;
    let login = login_of(token).await?;
    let approver = login::approver(Provider::GitHub, None)?;
    let grant = approve_repository(sqlite, &repository, &login, approver.as_ref(), batch).await?;
    Ok((grant.repository, grant.login))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use super::{
        approve_repository, origin_repository, renew, Repository, APP_CLIENT_ID, APP_URL,
        TEST_GITHUB_URL,
    };
    use crate::config::placement::Approver;
    use crate::linear::test_server::{json_response, spawn, QueuedResponse};
    use loopflow_storage::rental::GithubRentalGrant;
    use loopflow_storage::sqlite::SqliteStore;

    const LAPTOP: &str = "gho_LAPTOP_SENTINEL";

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        (dir, store)
    }

    fn repository() -> Repository {
        Repository {
            id: 42,
            name: "loopflowstudio/loopflow".into(),
        }
    }

    fn ok(body: serde_json::Value) -> QueuedResponse {
        json_response(StatusCode::OK, body)
    }

    fn device_code() -> QueuedResponse {
        ok(json!({
            "device_code": "DEVICE", "user_code": "ABCD-1234",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900, "interval": 0
        }))
    }

    fn issued(access: &str, refresh: &str) -> QueuedResponse {
        ok(json!({
            "access_token": access, "refresh_token": refresh, "expires_in": 28800,
            "refresh_token_expires_in": 15897600, "token_type": "bearer", "scope": ""
        }))
    }

    /// What GitHub says a token is and reaches: its account, then one
    /// installation listing `repositories`.
    fn reach(login: &str, repositories: &[i64]) -> Vec<QueuedResponse> {
        let listed: Vec<_> = repositories.iter().map(|id| json!({ "id": id })).collect();
        vec![
            ok(json!({ "login": login })),
            ok(json!({ "total_count": 1, "installations": [{ "id": 7 }] })),
            ok(json!({ "total_count": listed.len(), "repositories": listed })),
        ]
    }

    fn approver() -> Option<Approver> {
        Some(Approver::Command {
            command: "true".into(),
        })
    }

    fn stored(access: &str, refresh: &str) -> GithubRentalGrant {
        GithubRentalGrant {
            repository_id: 42,
            repository: "loopflowstudio/loopflow".into(),
            login: "jack".into(),
            access_token: access.into(),
            refresh_token: refresh.into(),
            expires_at: time::OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap(),
        }
    }

    #[tokio::test]
    async fn an_approval_stores_a_renewable_token_for_the_one_repository() {
        let (dir, sqlite) = store();
        let mut responses = vec![
            device_code(),
            ok(json!({ "error": "authorization_pending" })),
            issued("ghu_RENTAL", "ghr_GRANT"),
        ];
        responses.extend(reach("jack", &[42]));
        let (url, requests) = spawn(responses).await;
        let grant = TEST_GITHUB_URL
            .scope(
                url,
                approve_repository(&sqlite, &repository(), "jack", approver().as_ref(), true),
            )
            .await
            .unwrap();
        assert!(grant.access_token == "ghu_RENTAL" && grant.refresh_token == "ghr_GRANT");
        assert!(sqlite.github_rental_grant(42).unwrap() == Some(grant));

        let requests = requests.lock().await;
        assert_eq!(requests[0].path, "/login/device/code");
        assert!(requests[0].body.contains(APP_CLIENT_ID));
        assert!(requests[0].body.contains("scope=offline_access"));
        assert_eq!(requests[2].path, "/login/oauth/access_token");
        assert!(requests[2].body.contains("repository_id=42"));
        assert!(requests[5]
            .path
            .starts_with("/user/installations/7/repositories"));
        // Everything GitHub is asked after the code uses the token it issued.
        for request in &requests[3..] {
            assert_eq!(request.authorization.as_deref(), Some("Bearer ghu_RENTAL"));
        }

        let raw: (String, String) = rusqlite::Connection::open(dir.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT access_token,refresh_token FROM github_rental_grants",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(!raw.0.contains("ghu_RENTAL") && !raw.1.contains("ghr_GRANT"));
    }

    async fn refused_approval(reach: Vec<QueuedResponse>) -> String {
        let (_dir, sqlite) = store();
        let mut responses = vec![device_code(), issued("ghu_RENTAL", "ghr_GRANT")];
        responses.extend(reach);
        let (url, _) = spawn(responses).await;
        let result = TEST_GITHUB_URL
            .scope(
                url,
                approve_repository(&sqlite, &repository(), "jack", approver().as_ref(), true),
            )
            .await;
        assert!(sqlite.github_rental_grant(42).unwrap().is_none());
        // Unlike expect_err, this cannot print a token on an unexpected success.
        match result {
            Err(error) => error.to_string(),
            Ok(_) => panic!("the approval should be refused"),
        }
    }

    #[tokio::test]
    async fn an_app_not_installed_on_the_repository_names_where_to_install_it() {
        // GitHub ignores `repository_id` there, so the token reaches the
        // repositories the app is installed on instead.
        let error = refused_approval(reach("jack", &[9])).await;
        assert!(error.contains(APP_URL), "{error}");
    }

    #[tokio::test]
    async fn a_token_reaching_a_second_repository_is_discarded() {
        let error = refused_approval(reach("jack", &[42, 9])).await;
        assert!(error.contains("reaches 1 more"), "{error}");
    }

    #[tokio::test]
    async fn an_approval_by_another_account_is_discarded() {
        let error = refused_approval(reach("someone-else", &[42])).await;
        assert!(error.contains("approve as jack"), "{error}");
    }

    #[tokio::test]
    async fn no_terminal_and_no_command_approver_asks_github_for_nothing() {
        let (_dir, sqlite) = store();
        let (url, requests) = spawn(vec![device_code()]).await;
        let result = TEST_GITHUB_URL
            .scope(
                url,
                approve_repository(&sqlite, &repository(), "jack", None, true),
            )
            .await;
        let Err(error) = result else {
            panic!("a run with no terminal cannot approve")
        };
        assert!(error.to_string().contains("lf ondemand login github"));
        assert!(requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn a_renewal_replaces_the_stored_tokens() {
        let (_dir, sqlite) = store();
        sqlite
            .put_github_rental_grant(&stored("ghu_OLD", "ghr_OLD"))
            .unwrap();
        let mut responses = vec![issued("ghu_NEW", "ghr_NEW")];
        responses.extend(reach("jack", &[42]));
        let (url, requests) = spawn(responses).await;
        let renewed = TEST_GITHUB_URL
            .scope(url, renew(&sqlite, &stored("ghu_OLD", "ghr_OLD")))
            .await
            .unwrap();
        assert!(renewed.access_token == "ghu_NEW");
        assert!(sqlite.github_rental_grant(42).unwrap() == Some(renewed));
        let requests = requests.lock().await;
        assert!(requests[0].body.contains("grant_type=refresh_token"));
        assert!(requests[0].body.contains("refresh_token=ghr_OLD"));
    }

    #[tokio::test]
    async fn a_renewed_token_that_lost_its_restriction_is_discarded() {
        let (_dir, sqlite) = store();
        sqlite
            .put_github_rental_grant(&stored("ghu_OLD", "ghr_OLD"))
            .unwrap();
        let mut responses = vec![issued("ghu_NEW", "ghr_NEW")];
        responses.extend(reach("jack", &[42, 9]));
        let (url, _) = spawn(responses).await;
        let result = TEST_GITHUB_URL
            .scope(url, renew(&sqlite, &stored("ghu_OLD", "ghr_OLD")))
            .await;
        assert!(result.is_err());
        assert!(sqlite.github_rental_grant(42).unwrap() == Some(stored("ghu_OLD", "ghr_OLD")));
    }

    #[tokio::test]
    async fn a_refused_renewal_keeps_the_grant_and_names_the_approval() {
        let (_dir, sqlite) = store();
        sqlite
            .put_github_rental_grant(&stored("ghu_OLD", "ghr_OLD"))
            .unwrap();
        let (url, _) = spawn(vec![ok(json!({ "error": "bad_refresh_token" }))]).await;
        let result = TEST_GITHUB_URL
            .scope(url, renew(&sqlite, &stored("ghu_OLD", "ghr_OLD")))
            .await;
        let Err(error) = result else {
            panic!("GitHub refused the renewal")
        };
        assert!(error.to_string().contains("lf ondemand login github"));
        assert!(sqlite.github_rental_grant(42).unwrap() == Some(stored("ghu_OLD", "ghr_OLD")));
    }

    #[tokio::test]
    async fn the_repository_is_the_origins_as_this_machines_gh_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        for args in [
            &["init", "-q"][..],
            &[
                "remote",
                "add",
                "origin",
                "git@github.com:loopflowstudio/Loopflow.git",
            ],
        ] {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success());
        }
        let (url, requests) = spawn(vec![ok(
            json!({ "id": 42, "full_name": "loopflowstudio/loopflow" }),
        )])
        .await;
        let found = TEST_GITHUB_URL
            .scope(url, origin_repository(dir.path(), LAPTOP))
            .await
            .unwrap();
        assert_eq!(found, Some(repository()));
        let requests = requests.lock().await;
        assert_eq!(requests[0].path, "/repos/loopflowstudio/Loopflow");
    }
}
