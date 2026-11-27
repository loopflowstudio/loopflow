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
use loopflow_model::durable::Machine;
use loopflow_storage::rental::{GithubRentalGrant, Rental, RentalId};
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
    pub(crate) static TEST_GITHUB_URL: String;
}

/// GitHub no longer takes the token it was asked with.
#[derive(Debug, thiserror::Error)]
#[error("GitHub rejected the credential")]
pub(crate) struct Rejected;

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
        reqwest::StatusCode::UNAUTHORIZED => Err(Rejected.into()),
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
pub(crate) async fn origin_repository(repo: &Path, token: &str) -> Result<Option<Repository>> {
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

/// Asks GitHub whether `token` is what a rental of `repository` may hold: it
/// acts as `login` and reaches that repository and no other. The inner error
/// is GitHub's answer that it is not; the outer one is no answer, or
/// [`Rejected`] when GitHub no longer takes the token.
async fn reach(
    token: &str,
    repository: &Repository,
    login: &str,
) -> Result<std::result::Result<(), String>> {
    let approved = login_of(token).await?;
    if approved != login {
        return Ok(Err(format!(
            "GitHub was approved as {approved}, but `gh` here is signed in as {login}; \
             approve as {login}"
        )));
    }
    let (mut reaches, mut others) = (false, 0_u64);
    let installations = read(token, "/user/installations?per_page=100")
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
            token,
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
        return Ok(Err(format!(
            "the Loopflow Studio app is not installed on {name}: install it at {APP_URL}, \
             then run this again for a new code",
            name = repository.name
        )));
    }
    if others > 0 {
        return Ok(Err(format!(
            "GitHub issued a token that reaches {others} more than {}; discarded",
            repository.name
        )));
    }
    Ok(Ok(()))
}

/// What GitHub issued as `repository`'s grant: an expiring, renewable token
/// for `login`. What it reaches is still to be asked ([`reach`]).
fn grant_of(issued: Issued, repository: &Repository, login: &str) -> Result<GithubRentalGrant> {
    let (Some(refresh_token), Some(expires_in)) = (issued.refresh_token, issued.expires_in) else {
        bail!("GitHub issued a token that cannot be renewed; discarded");
    };
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

/// A command approver needs no terminal; a browser needs a person at one.
fn can_ask(approver: Option<&Approver>, batch: bool) -> bool {
    matches!(approver, Some(Approver::Command { .. }))
        || crate::machine::credentials::foreground(batch)
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

/// One device approval for `repository`, stored as its grant. An approval by
/// another account than `asker`'s is discarded. The caller holds the
/// repository's lock.
async fn approve_repository(
    sqlite: &SqliteStore,
    repository: &Repository,
    asker: &Asker<'_>,
) -> Result<GithubRentalGrant> {
    let Asker {
        login,
        approver,
        batch,
    } = *asker;
    let name = &repository.name;
    if !can_ask(approver, batch) {
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
    // Nothing but a token from this app can ask whether it is installed, so
    // the link comes before the code an approval without it would spend.
    eprintln!("First install the app on {name}: {APP_URL}");
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
    // Checked before it is kept: nothing renews an approval that reaches the
    // wrong thing into one that does not.
    let grant = grant_of(issued, repository, login)?;
    reach(&grant.access_token, repository, login)
        .await?
        .map_err(|refusal| anyhow!(refusal))?;
    sqlite.put_github_rental_grant(&grant)?;
    Ok(grant)
}

/// Renew `grant`, which ends the token it held: every rental holding that
/// token is behind until it is sent the new one. The new tokens are stored as
/// soon as GitHub issues them, since the old ones are spent by then; what the
/// new token reaches is asked afterwards, by [`usable`]. The inner error is
/// GitHub's refusal, which keeps the stored grant until an approval replaces
/// it.
async fn renew(
    sqlite: &SqliteStore,
    grant: &GithubRentalGrant,
) -> Result<std::result::Result<GithubRentalGrant, String>> {
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
    let issued = match reply {
        Ok(issued) => issued,
        Err(refusal) => {
            return Ok(Err(format!(
                "GitHub no longer renews rentals' access to {} ({refusal}); \
                 run `lf ondemand login github` in its checkout to approve again",
                repository.name
            )))
        }
    };
    let renewed = match grant_of(issued, &repository, &grant.login) {
        Ok(renewed) => renewed,
        Err(unrenewable) => return Ok(Err(unrenewable.to_string())),
    };
    sqlite.put_github_rental_grant(&renewed)?;
    Ok(Ok(renewed))
}

/// The token `repository`'s rentals can be sent until `need`: `stored` when
/// it lasts and GitHub still takes it, otherwise a renewed one. Either way
/// GitHub is asked what the token reaches before anything relies on it, so a
/// revoked approval, an uninstalled app or a renewal that lost its
/// restriction shows here, as the inner error. With no answer the stored
/// grant stays as it is and the next settle asks again.
async fn usable(
    sqlite: &SqliteStore,
    repository: &Repository,
    stored: GithubRentalGrant,
    need: OffsetDateTime,
) -> Result<std::result::Result<GithubRentalGrant, String>> {
    if covers(&stored, need) {
        match reach(&stored.access_token, repository, &stored.login).await {
            Ok(answer) => return Ok(answer.map(|()| stored)),
            // GitHub no longer takes it; the grant may still renew.
            Err(error) if error.is::<Rejected>() => {}
            Err(error) => return Err(error),
        }
    }
    let renewed = match renew(sqlite, &stored).await? {
        Ok(renewed) => renewed,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let answer = reach(&renewed.access_token, repository, &renewed.login).await?;
    Ok(answer.map(|()| renewed))
}

/// A token with this long left is as good as a new one. GitHub issues none
/// lasting past 8 hours, so renewing it would only end what rentals hold.
const AS_GOOD_AS_NEW: time::Duration = time::Duration::hours(7);
/// A rental's deadline is set when it is admitted, moments after placement
/// asked for a token that lasts until it.
const ADMISSION_MARGIN: time::Duration = time::Duration::minutes(10);

fn covers(grant: &GithubRentalGrant, until: OffsetDateTime) -> bool {
    grant.expires_at >= until.min(OffsetDateTime::now_utc() + AS_GOOD_AS_NEW)
}

/// One settle of a repository at a time: its grant has a single refresh
/// token, and what each rental holds is read and written around it. A second
/// `lf` with someone watching (`wait`) says it is waiting and waits here, also
/// while a person types the first one's code, then finds what the first one
/// left. An ending gets `None` instead: it runs inside this Machine's one
/// recovery pass, which must not stand behind an untyped code.
async fn repository_lock(repository: &Repository, wait: bool) -> Result<Option<std::fs::File>> {
    let directory = crate::store::lf_home_dir().join("accounts");
    std::fs::create_dir_all(&directory)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(format!(".github-rentals-{}.lock", repository.id)))?;
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => return Ok(Some(file)),
        Err(held) if held.kind() != fs2::lock_contended_error().kind() => return Err(held.into()),
        Err(_) if !wait => return Ok(None),
        Err(_) => eprintln!(
            "Waiting for another lf to finish with GitHub for {}; if it shows a code, approve \
             it there",
            repository.name
        ),
    }
    tokio::task::spawn_blocking(move || {
        fs2::FileExt::lock_exclusive(&file)?;
        Ok(Some(file))
    })
    .await?
}

/// Who a settle can ask for an approval: the account this Machine's `gh` is
/// signed in to, and where its approval page goes.
#[derive(Clone, Copy)]
pub(crate) struct Asker<'a> {
    pub login: &'a str,
    pub approver: Option<&'a Approver>,
    pub batch: bool,
}

/// A live rental the repository's current token did not reach. It stays
/// recorded as behind, so the next settle of the repository sends again.
pub(crate) struct Unsent {
    pub rental: RentalId,
    pub machine: String,
    pub error: anyhow::Error,
}

pub(crate) struct Settled {
    /// When the repository's token stops, while anything needs it.
    pub expires_at: Option<OffsetDateTime>,
    /// A person approved during this settle.
    pub approved: bool,
    pub unsent: Vec<Unsent>,
}

/// Bring a repository's rentals and its token into agreement. The token must
/// last until `until` and the deadline of every live rental signed in for the
/// repository: the stored one when it does, otherwise a renewed one, or a new
/// approval when GitHub no longer renews and `asker` can be asked. GitHub is
/// asked what the token reaches before any rental is sent it ([`usable`]).
/// Every live rental not recorded as holding the current token is sent it,
/// and each send that lands is recorded. With nothing needing a token, the
/// grant is renewed once so the token the last rental held stops working.
///
/// Placement, an approval ahead of time, a sign-in by hand and an ending all
/// come here; a rental one of them could not reach is sent by the next. An
/// ending (no `asker`) that finds another `lf` settling the repository does
/// nothing.
pub(crate) async fn settle(
    sqlite: &SqliteStore,
    repository: &Repository,
    until: Option<OffsetDateTime>,
    asker: Option<&Asker<'_>>,
) -> Result<Settled> {
    let nothing = Settled {
        expires_at: None,
        approved: false,
        unsent: Vec::new(),
    };
    let Some(_lock) = repository_lock(repository, asker.is_some()).await? else {
        // Whoever holds it is settling this repository now. A token this
        // ending would have cut stops at its own expiry.
        tracing::info!(
            repository = %repository.name,
            "another lf is settling GitHub; this ending leaves it to that one"
        );
        return Ok(nothing);
    };
    let need = sqlite
        .rentals_holding_github(repository.id)?
        .iter()
        .map(|holder| holder.rental.deadline)
        .chain(until)
        .max();
    let stored = sqlite
        .github_rental_grant(repository.id)?
        .filter(|stored| asker.is_none_or(|asker| stored.login == asker.login));
    let asking = asker.filter(|asker| can_ask(asker.approver, asker.batch));
    let (grant, approved) = match (stored, need) {
        (Some(stored), Some(need)) => {
            match (usable(sqlite, repository, stored, need).await?, asking) {
                (Ok(grant), _) => (grant, false),
                // GitHub's refusal: only a new approval answers it, and the
                // person asked for one is told why.
                (Err(refusal), Some(asker)) => {
                    eprintln!("{refusal}");
                    (approve_repository(sqlite, repository, asker).await?, true)
                }
                (Err(refusal), None) => bail!(refusal),
            }
        }
        (None, Some(_)) => {
            let Some(asker) = asker else {
                bail!(
                    "rentals have no GitHub approval for {}; run `lf ondemand login github` \
                     in its checkout",
                    repository.name
                );
            };
            (approve_repository(sqlite, repository, asker).await?, true)
        }
        (Some(stored), None) if stored.expires_at > OffsetDateTime::now_utc() => {
            // Nothing is sent this token, so nothing asks what it reaches.
            renew(sqlite, &stored)
                .await?
                .map_err(|refusal| anyhow!(refusal))?;
            return Ok(nothing);
        }
        (_, None) => return Ok(nothing),
    };
    Ok(Settled {
        expires_at: Some(grant.expires_at),
        approved,
        unsent: send_behind(sqlite, &grant)?,
    })
}

/// Send `grant`'s token to every live rental of its repository not recorded
/// as holding it. The caller holds the repository's lock.
fn send_behind(sqlite: &SqliteStore, grant: &GithubRentalGrant) -> Result<Vec<Unsent>> {
    let mut unsent = Vec::new();
    for holder in sqlite.rentals_holding_github(grant.repository_id)? {
        if holder.sent_expires_at == Some(grant.expires_at) {
            continue;
        }
        // Signed in only once it is reachable as a Machine.
        let Some(machine) = holder
            .rental
            .machine_id
            .as_ref()
            .map(|id| crate::store::machines::machine_by_id(sqlite, id))
            .transpose()?
            .flatten()
        else {
            continue;
        };
        let sent =
            crate::machine::credentials::send_github(&machine, &grant.login, &grant.access_token);
        match sent {
            Ok(()) => sqlite.set_rental_github_token_sent(&holder.rental.id, grant.expires_at)?,
            Err(error) => unsent.push(Unsent {
                rental: holder.rental.id,
                machine: login::name(&machine),
                error,
            }),
        }
    }
    Ok(unsent)
}

/// Say which rentals of `repository` hold a token that stopped working.
fn report(unsent: &[Unsent], repository: &str) {
    for Unsent { machine, error, .. } in unsent {
        eprintln!(
            "{machine} holds a GitHub token that stopped working ({error:#}); the next rental \
             command for {repository} sends the new one, as does \
             `lf machine login github {machine}`"
        );
    }
}

/// The repository whose grant is stored under `repository_id`.
fn approved_repository(sqlite: &SqliteStore, repository_id: i64) -> Result<Option<Repository>> {
    Ok(sqlite
        .github_rental_grant(repository_id)?
        .map(|grant| Repository {
            id: grant.repository_id,
            name: grant.repository,
        }))
}

/// Settle a repository nobody is here to approve for: a rental signed in for
/// it ended.
pub(crate) async fn settle_unattended(sqlite: &SqliteStore, repository_id: i64) -> Result<()> {
    let Some(repository) = approved_repository(sqlite, repository_id)? else {
        return Ok(());
    };
    let settled = settle(sqlite, &repository, None, None).await?;
    report(&settled.unsent, &repository.name);
    Ok(())
}

/// Settle the repository `repo` is a checkout of for the person at this
/// Machine: its rentals hold a GitHub token that lasts until `until`. `None`
/// when the origin is not on GitHub.
async fn settle_checkout(
    sqlite: &SqliteStore,
    repo: &Path,
    until: OffsetDateTime,
    batch: bool,
) -> Result<Option<Approval>> {
    if crate::git::worktrees::github_repo_nwo(repo).is_none() {
        return Ok(None);
    }
    let laptop = crate::machine::credentials::gh_token()?;
    let token = secrecy::ExposeSecret::expose_secret(&laptop);
    let Some(repository) = origin_repository(repo, token).await? else {
        return Ok(None);
    };
    let login = login_of(token).await?;
    let approver = login::approver(Provider::GitHub, None)?;
    let asker = Asker {
        login: &login,
        approver: approver.as_ref(),
        batch,
    };
    let settled = settle(sqlite, &repository, Some(until), Some(&asker)).await?;
    report(&settled.unsent, &repository.name);
    Ok(Some(Approval {
        repository: repository.name,
        login,
        already: !settled.approved,
    }))
}

/// Before anything is rented: `repo`'s rentals hold a GitHub token that lasts
/// until `until`, the deadline the rental would get, and GitHub still lets it
/// reach the repository. A missing approval is asked for here; one that
/// cannot be asked for, a refused renewal, another `gh` account or a missing
/// install stops the placement with nothing billed. An origin outside GitHub
/// needs none.
pub async fn settle_before_renting(
    sqlite: &SqliteStore,
    repo: &Path,
    until: OffsetDateTime,
    batch: bool,
) -> Result<()> {
    settle_checkout(sqlite, repo, until + ADMISSION_MARGIN, batch).await?;
    Ok(())
}

/// The standing approval for a repository's rentals.
#[derive(Debug)]
pub struct Approval {
    /// `owner/name`.
    pub repository: String,
    pub login: String,
    /// It stood before this command; nothing was typed.
    pub already: bool,
}

/// `lf ondemand login github`: the rentals of the repository `repo` is a
/// checkout of have a working GitHub approval. One that GitHub still honors
/// is kept and asks for nothing; otherwise it is renewed, or approved by a
/// new code, and rentals already running are sent the new token.
pub async fn approve_ahead(sqlite: &SqliteStore, repo: &Path, batch: bool) -> Result<Approval> {
    settle_checkout(sqlite, repo, OffsetDateTime::now_utc(), batch)
        .await?
        .with_context(|| format!("{} has no GitHub origin", repo.display()))
}

/// Sign a rental in to GitHub for the repository it is recorded for, from any
/// directory. A rental's repository is set once: `repo`, a checkout, names it
/// only for a rental that has held no token yet (one from `lf ondemand rent`).
/// A rental never receives this Machine's own `gh` token, and is sent its
/// repository's token whatever it was sent before: asking again says the one
/// it holds may be gone.
///
/// # Errors
/// When GitHub has no approval and none can be asked for, or the rental
/// cannot be reached. Another rental the token did not reach is reported, not
/// returned.
pub(crate) async fn sign_in(
    sqlite: &SqliteStore,
    machine: &Machine,
    rental: &Rental,
    repo: &Path,
    approver: Option<&Approver>,
    batch: bool,
) -> Result<()> {
    let laptop = crate::machine::credentials::gh_token()?;
    let laptop = secrecy::ExposeSecret::expose_secret(&laptop);
    sign_in_as(sqlite, machine, rental, repo, laptop, approver, batch).await
}

/// `laptop` is this Machine's `gh` token: it names the account and reads the
/// origin, and goes nowhere else.
async fn sign_in_as(
    sqlite: &SqliteStore,
    machine: &Machine,
    rental: &Rental,
    repo: &Path,
    laptop: &str,
    approver: Option<&Approver>,
    batch: bool,
) -> Result<()> {
    let login = login_of(laptop).await?;
    // A recorded repository with no grant never had a token to send.
    let recorded = sqlite
        .rental_github_repository(&rental.id)?
        .map(|id| approved_repository(sqlite, id))
        .transpose()?
        .flatten();
    let repository = match recorded {
        Some(repository) => repository,
        None => origin_repository(repo, laptop).await?.ok_or_else(|| {
            anyhow!(
                "{} has no GitHub origin; run this from a checkout of the repository \
                 the rental works in",
                repo.display()
            )
        })?,
    };
    // Recorded before the settle, so no ending retires the token on the way.
    sqlite.set_rental_github_repository(&rental.id, repository.id)?;
    let asker = Asker {
        login: &login,
        approver,
        batch,
    };
    let settled = settle(sqlite, &repository, None, Some(&asker)).await?;
    let (mine, others): (Vec<_>, Vec<_>) = settled
        .unsent
        .into_iter()
        .partition(|unsent| unsent.rental == rental.id);
    report(&others, &repository.name);
    let name = login::name(machine);
    if let Some(unsent) = mine.into_iter().next() {
        return Err(unsent.error);
    }
    if let Some(expires_at) = settled.expires_at {
        println!("{}", access_line(&name, expires_at, rental.deadline));
    }
    Ok(())
}

/// When GitHub stops on a rental, and what to do when that is before the
/// rental itself ends.
fn access_line(name: &str, expires_at: OffsetDateTime, deadline: OffsetDateTime) -> String {
    let until = crate::rental::clock(expires_at);
    if expires_at < deadline {
        format!(
            "GitHub on {name} until {until}, before the rental ends; \
             `lf machine login github {name}` renews"
        )
    } else {
        format!("GitHub on {name} until {until}")
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use std::sync::{Arc, Mutex};

    use super::{
        access_line, approve_repository, origin_repository, renew, settle, sign_in_as, Asker,
        Repository, APP_CLIENT_ID, APP_URL, TEST_GITHUB_URL,
    };
    use crate::config::environment::EnvVarGuard;
    use crate::config::placement::Approver;
    use crate::linear::test_server::{json_response, spawn, QueuedResponse};
    use crate::machine::credentials::{TestSends, TEST_SENDS};
    use loopflow_model::durable::{Machine, MachineId};
    use loopflow_storage::rental::{
        GithubDelivery, GithubRentalGrant, Hardware, OnDemand, Provider, Rental, Usd,
    };
    use loopflow_storage::sqlite::SqliteStore;
    use time::{Duration, OffsetDateTime};

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

    /// The account `gh` is signed in to here, with no terminal to ask in.
    fn asker(approver: Option<&Approver>) -> Asker<'_> {
        Asker {
            login: "jack",
            approver,
            batch: true,
        }
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
                approve_repository(&sqlite, &repository(), &asker(approver().as_ref())),
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
                approve_repository(&sqlite, &repository(), &asker(approver().as_ref())),
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
                approve_repository(&sqlite, &repository(), &asker(None)),
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
        let (url, requests) = spawn(vec![issued("ghu_NEW", "ghr_NEW")]).await;
        let renewed = TEST_GITHUB_URL
            .scope(url, renew(&sqlite, &stored("ghu_OLD", "ghr_OLD")))
            .await;
        let Ok(Ok(renewed)) = renewed else {
            panic!("GitHub renewed the grant")
        };
        assert!(renewed.access_token == "ghu_NEW");
        assert!(sqlite.github_rental_grant(42).unwrap() == Some(renewed));
        let requests = requests.lock().await;
        assert!(requests[0].body.contains("grant_type=refresh_token"));
        assert!(requests[0].body.contains("refresh_token=ghr_OLD"));
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
        let Ok(Err(refusal)) = result else {
            panic!("GitHub refused the renewal")
        };
        assert!(refusal.contains("lf ondemand login github"));
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

    // -- Rentals ------------------------------------------------------------

    /// A checkout whose origin is `owner/name` on GitHub.
    fn checkout(dir: &std::path::Path, nwo: &str) -> std::path::PathBuf {
        let repo = dir.join(nwo.replace('/', "-"));
        std::fs::create_dir(&repo).unwrap();
        let origin = format!("git@github.com:{nwo}.git");
        for args in [&["init", "-q"][..], &["remote", "add", "origin", &origin]] {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success());
        }
        repo
    }

    /// A store whose on-demand `code` rents for four hours.
    fn renting(dir: &std::path::Path, logins: Vec<crate::provider_auth::Provider>) -> SqliteStore {
        let store = crate::store::opening::open_ephemeral(&dir.join("db")).unwrap();
        crate::store::rentals::add_ondemand(
            &store,
            &OnDemand {
                name: "code".into(),
                provider: Provider::Daytona,
                hardware: Hardware {
                    vcpus: 4,
                    memory_gb: 8,
                    disk_gb: 10,
                    gpu: None,
                    spot: false,
                    modal_sizing: None,
                },
                max_machines: 3,
                max_usd_per_hour: Usd::parse("0.40").unwrap(),
                hours: 4.0,
                logins,
            },
        )
        .unwrap();
        store
    }

    /// A running rental of `code` whose Machine is `label`.
    fn rental(store: &SqliteStore, label: &str) -> (Rental, Machine) {
        let rental = crate::store::rentals::admit_rental(store, "code", None, None, false).unwrap();
        let sandbox = format!("sb-{label}");
        let rental = store
            .confirm_rental(&rental.id, &sandbox, rental.usd_per_hour)
            .unwrap();
        let route = format!("ssh://lf@{label}.example:22");
        let machine =
            crate::store::machines::add_machine(store, &MachineId::new(), &route, label).unwrap();
        (
            store.attach_rental(&rental.id, &machine.id).unwrap(),
            machine,
        )
    }

    /// A rental signed in for `repository`, holding the token of `held`.
    fn holding(store: &SqliteStore, label: &str, held: &GithubRentalGrant) -> (Rental, Machine) {
        let (rental, machine) = rental(store, label);
        store
            .set_rental_github_repository(&rental.id, held.repository_id)
            .unwrap();
        store
            .set_rental_github_token_sent(&rental.id, held.expires_at)
            .unwrap();
        (rental, machine)
    }

    /// Repository `id`'s grant, its token stopping `hours` from now.
    fn grant(id: i64, access: &str, hours: i64) -> GithubRentalGrant {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        GithubRentalGrant {
            repository_id: id,
            repository: format!("loopflowstudio/repository-{id}"),
            login: "jack".into(),
            access_token: access.into(),
            refresh_token: "ghr_OLD".into(),
            expires_at: OffsetDateTime::from_unix_timestamp(now).unwrap() + Duration::hours(hours),
        }
    }

    /// The account this Machine's `gh` token names.
    fn laptop_login() -> QueuedResponse {
        ok(json!({ "login": "jack" }))
    }

    /// What this Machine's `gh` token is asked for at a rental's first
    /// sign-in: its account, then the origin.
    fn laptop_reads(id: i64) -> Vec<QueuedResponse> {
        vec![
            laptop_login(),
            ok(json!({ "id": id, "full_name": format!("loopflowstudio/repository-{id}") })),
        ]
    }

    /// GitHub renewing repository `id`'s grant into `access`, lasting `hours`.
    fn renewal(id: i64, access: &str, hours: i64) -> Vec<QueuedResponse> {
        let mut responses = vec![ok(json!({
            "access_token": access, "refresh_token": "ghr_NEW", "expires_in": hours * 3600
        }))];
        responses.extend(reach("jack", &[id]));
        responses
    }

    fn sends() -> Arc<Mutex<TestSends>> {
        Arc::default()
    }

    /// The tokens each Machine was sent, in order.
    fn sent(sends: &Arc<Mutex<TestSends>>) -> Vec<(MachineId, String)> {
        sends.lock().unwrap().sent.clone()
    }

    fn renewals(requests: &[crate::linear::test_server::CapturedRequest]) -> usize {
        requests
            .iter()
            .filter(|request| request.body.contains("grant_type=refresh_token"))
            .count()
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_rental_is_sent_its_repositorys_token_and_never_this_machines() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let repo = checkout(dir.path(), "loopflowstudio/repository-42");
        let (rental, machine) = rental(&store, "code-1");
        let held = grant(42, "ghu_RENTAL", 6);
        store.put_github_rental_grant(&held).unwrap();
        let mut responses = laptop_reads(42);
        responses.extend(reach("jack", &[42]));
        let (url, requests) = spawn(responses).await;
        let sends = sends();
        let sign_in = sign_in_as(&store, &machine, &rental, &repo, LAPTOP, None, true);
        TEST_GITHUB_URL
            .scope(url, TEST_SENDS.scope(sends.clone(), sign_in))
            .await
            .unwrap();
        let sent = sent(&sends);
        assert_eq!(sent.len(), 1);
        assert!(!sent[0].1.contains(LAPTOP), "this Machine's token left it");
        assert!(sent[0].1.contains(r#""token":"ghu_RENTAL""#));
        let holders = store.rentals_holding_github(42).unwrap();
        assert_eq!(holders[0].sent_expires_at, Some(held.expires_at));
        // Nothing was renewed: the stored token outlives the rental, and
        // GitHub said it still reaches the repository.
        let requests = requests.lock().await;
        assert_eq!((requests.len(), renewals(&requests)), (5, 0));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn signing_in_again_sends_the_token_the_rental_is_recorded_as_holding() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let repo = checkout(dir.path(), "loopflowstudio/repository-42");
        let held = grant(42, "ghu_RENTAL", 6);
        store.put_github_rental_grant(&held).unwrap();
        let (rental, machine) = holding(&store, "code-1", &held);
        let mut responses = vec![laptop_login()];
        responses.extend(reach("jack", &[42]));
        let (url, _) = spawn(responses).await;
        let sends = sends();
        let sign_in = sign_in_as(&store, &machine, &rental, &repo, LAPTOP, None, true);
        TEST_GITHUB_URL
            .scope(url, TEST_SENDS.scope(sends.clone(), sign_in))
            .await
            .unwrap();
        assert_eq!(sent(&sends).len(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_rental_the_new_token_did_not_reach_is_sent_it_by_the_next_settle() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let repo = checkout(dir.path(), "loopflowstudio/repository-42");
        // One rental holds a token that stops before a new rental's deadline.
        let old = grant(42, "ghu_OLD", 1);
        store.put_github_rental_grant(&old).unwrap();
        let (first_rental, first) = holding(&store, "code-1", &old);
        let (second, second_machine) = rental(&store, "code-2");
        let mut responses = laptop_reads(42);
        responses.extend(renewal(42, "ghu_NEW", 8));
        responses.extend(reach("jack", &[42]));
        let (url, requests) = spawn(responses).await;
        let sends = sends();
        sends
            .lock()
            .unwrap()
            .unreachable
            .push(second_machine.id.clone());

        let sign_in = sign_in_as(&store, &second_machine, &second, &repo, LAPTOP, None, true);
        let result = TEST_GITHUB_URL
            .scope(url.clone(), TEST_SENDS.scope(sends.clone(), sign_in))
            .await;
        assert!(result.is_err(), "the new rental was not reached");
        // The renewal ended the first rental's token; it has the new one.
        let reached = sent(&sends);
        assert_eq!(reached.len(), 1);
        assert_eq!(reached[0].0, first.id);
        assert!(reached[0].1.contains(r#""token":"ghu_NEW""#));
        // What `lf ondemand list` reads: who has the new token and who missed it.
        let new = store.github_rental_grant(42).unwrap().unwrap().expires_at;
        let delivery = |rental: &Rental| store.rental_github_delivery(&rental.id).unwrap();
        assert_eq!(delivery(&first_rental), Some(GithubDelivery::Until(new)));
        assert_eq!(delivery(&second), Some(GithubDelivery::NotDelivered));

        // Any later settle of the repository sends the one left behind, once
        // GitHub says what the stored token reaches; nothing is renewed.
        sends.lock().unwrap().unreachable.clear();
        let settled = TEST_GITHUB_URL
            .scope(
                url,
                TEST_SENDS.scope(sends.clone(), settle(&store, &repository_42(), None, None)),
            )
            .await
            .unwrap();
        assert!(settled.unsent.is_empty());
        let reached = sent(&sends);
        assert_eq!(reached.len(), 2);
        assert_eq!(reached[1].0, second_machine.id);
        assert!(reached[1].1.contains(r#""token":"ghu_NEW""#));
        assert_eq!(delivery(&second), Some(GithubDelivery::Until(new)));
        let requests = requests.lock().await;
        assert_eq!(renewals(&requests), 1);
        assert_eq!(requests.len(), 9);
    }

    fn repository_42() -> Repository {
        Repository {
            id: 42,
            name: "loopflowstudio/repository-42".into(),
        }
    }

    fn codes(requests: &[crate::linear::test_server::CapturedRequest]) -> usize {
        requests
            .iter()
            .filter(|request| request.path == "/login/device/code")
            .count()
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_sign_in_from_another_checkout_renews_the_rentals_own_repository() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let elsewhere = checkout(dir.path(), "loopflowstudio/repository-43");
        let held = grant(42, "ghu_FIRST", 6);
        store.put_github_rental_grant(&held).unwrap();
        store
            .put_github_rental_grant(&grant(43, "ghu_SECOND", 6))
            .unwrap();
        let (rental, machine) = holding(&store, "code-1", &held);
        // The origin of the directory is never read.
        let mut responses = vec![laptop_login()];
        responses.extend(reach("jack", &[42]));
        let (url, requests) = spawn(responses).await;
        let sends = sends();
        let approver = approver();
        let sign_in = sign_in_as(
            &store,
            &machine,
            &rental,
            &elsewhere,
            LAPTOP,
            approver.as_ref(),
            true,
        );
        TEST_GITHUB_URL
            .scope(url, TEST_SENDS.scope(sends.clone(), sign_in))
            .await
            .unwrap();
        let reached = sent(&sends);
        assert_eq!(reached.len(), 1);
        assert!(reached[0].1.contains(r#""token":"ghu_FIRST""#));
        assert_eq!(
            store.rental_github_repository(&rental.id).unwrap(),
            Some(42)
        );
        assert!(store.github_rental_grant(42).unwrap() == Some(held));
        let requests = requests.lock().await;
        assert_eq!(
            (requests.len(), renewals(&requests), codes(&requests)),
            (4, 0, 0)
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_renewal_github_could_not_be_asked_about_keeps_the_approval() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let old = grant(42, "ghu_OLD", 1);
        store.put_github_rental_grant(&old).unwrap();
        let (_, machine) = holding(&store, "code-1", &old);
        // GitHub issues the new tokens, then stops answering.
        let mut responses = vec![
            ok(json!({
                "access_token": "ghu_NEW", "refresh_token": "ghr_NEW", "expires_in": 28800
            })),
            json_response(StatusCode::BAD_GATEWAY, json!({})),
        ];
        responses.extend(reach("jack", &[42]));
        let (url, requests) = spawn(responses).await;
        let sends = sends();
        let approver = approver();
        let asker = asker(approver.as_ref());
        let repository = repository_42();
        let dropped = TEST_GITHUB_URL
            .scope(
                url.clone(),
                TEST_SENDS.scope(
                    sends.clone(),
                    settle(&store, &repository, None, Some(&asker)),
                ),
            )
            .await;
        assert!(dropped.is_err());
        // The old tokens are spent; the new ones are what the store holds,
        // and no rental was sent a token nobody asked GitHub about.
        let kept = store.github_rental_grant(42).unwrap().unwrap();
        assert!(kept.access_token == "ghu_NEW" && kept.refresh_token == "ghr_NEW");
        assert!(sent(&sends).is_empty());

        // The next settle, with nobody here, asks and then sends it.
        let settled = TEST_GITHUB_URL
            .scope(
                url,
                TEST_SENDS.scope(sends.clone(), settle(&store, &repository, None, None)),
            )
            .await
            .unwrap();
        assert!(settled.unsent.is_empty());
        let reached = sent(&sends);
        assert_eq!(reached.len(), 1);
        assert_eq!(reached[0].0, machine.id);
        assert!(reached[0].1.contains(r#""token":"ghu_NEW""#));
        let requests = requests.lock().await;
        assert_eq!((renewals(&requests), codes(&requests)), (1, 0));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_renewed_token_that_lost_its_restriction_reaches_no_rental() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let old = grant(42, "ghu_OLD", 1);
        store.put_github_rental_grant(&old).unwrap();
        holding(&store, "code-1", &old);
        let mut responses = vec![ok(json!({
            "access_token": "ghu_WIDE", "refresh_token": "ghr_NEW", "expires_in": 28800
        }))];
        responses.extend(reach("jack", &[42, 9]));
        responses.extend(reach("jack", &[42, 9]));
        let (url, requests) = spawn(responses).await;
        let sends = sends();
        // Asked again, GitHub says the same, and nothing is renewed twice.
        for _ in 0..2 {
            let result = TEST_GITHUB_URL
                .scope(
                    url.clone(),
                    TEST_SENDS.scope(sends.clone(), settle(&store, &repository_42(), None, None)),
                )
                .await;
            let Err(error) = result else {
                panic!("the token reaches a second repository")
            };
            assert!(error.to_string().contains("reaches 1 more"), "{error}");
        }
        assert!(sent(&sends).is_empty());
        assert_eq!(renewals(&requests.lock().await), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_refused_renewal_with_nobody_to_ask_stops_before_anything_is_rented() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        store
            .put_github_rental_grant(&grant(42, "ghu_OLD", 1))
            .unwrap();
        let (url, requests) = spawn(vec![ok(json!({ "error": "bad_refresh_token" }))]).await;
        let until = OffsetDateTime::now_utc() + Duration::hours(4);
        let result = TEST_GITHUB_URL
            .scope(
                url,
                settle(&store, &repository_42(), Some(until), Some(&asker(None))),
            )
            .await;
        let Err(error) = result else {
            panic!("GitHub refused the renewal")
        };
        assert!(error.to_string().contains("lf ondemand login github"));
        assert!(store.rentals(None).unwrap().is_empty());
        assert_eq!(requests.lock().await.len(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn two_placements_of_one_repository_share_one_approval() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let mut responses = vec![device_code(), issued("ghu_RENTAL", "ghr_GRANT")];
        responses.extend(reach("jack", &[42]));
        // The second placement asks what the first one's token reaches.
        responses.extend(reach("jack", &[42]));
        let (url, requests) = spawn(responses).await;
        let until = OffsetDateTime::now_utc() + Duration::hours(4);
        let approver = approver();
        let asker = asker(approver.as_ref());
        let repository = repository_42();
        let place = || settle(&store, &repository, Some(until), Some(&asker));
        let (first, second) = TEST_GITHUB_URL
            .scope(url, async { tokio::join!(place(), place()) })
            .await;
        assert!(first.is_ok() && second.is_ok());
        assert_eq!(codes(&requests.lock().await), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn an_ending_does_not_wait_behind_another_lf_settling_its_repository() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        store
            .put_github_rental_grant(&grant(42, "ghu_HELD", 6))
            .unwrap();
        let (url, requests) = spawn(renewal(42, "ghu_CUT", 8)).await;
        let repository = repository_42();
        // A placement showing a code holds the lock for as long as the code lives.
        let placement = super::repository_lock(&repository, true).await.unwrap();

        TEST_GITHUB_URL
            .scope(url.clone(), settle(&store, &repository, None, None))
            .await
            .unwrap();
        assert_eq!(renewals(&requests.lock().await), 0);

        drop(placement);
        TEST_GITHUB_URL
            .scope(url, settle(&store, &repository, None, None))
            .await
            .unwrap();
        assert_eq!(renewals(&requests.lock().await), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn an_approval_github_still_honors_asks_for_no_code() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let held = grant(42, "ghu_RENTAL", 6);
        store.put_github_rental_grant(&held).unwrap();
        let (url, requests) = spawn(reach("jack", &[42])).await;
        let approver = approver();
        // `lf ondemand login github`: a token is needed now, by no rental yet.
        let now = OffsetDateTime::now_utc();
        let settled = TEST_GITHUB_URL
            .scope(
                url,
                settle(
                    &store,
                    &repository_42(),
                    Some(now),
                    Some(&asker(approver.as_ref())),
                ),
            )
            .await
            .unwrap();
        assert!(!settled.approved);
        assert!(store.github_rental_grant(42).unwrap() == Some(held));
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 3);
        assert!(requests
            .iter()
            .all(|request| request.authorization.as_deref() == Some("Bearer ghu_RENTAL")));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_stored_token_that_lost_the_repository_stops_before_anything_is_rented() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        let held = grant(42, "ghu_RENTAL", 6);
        store.put_github_rental_grant(&held).unwrap();
        // The app was taken off the repository after the approval.
        let (url, requests) = spawn(reach("jack", &[9])).await;
        let until = OffsetDateTime::now_utc() + Duration::hours(4);
        let result = TEST_GITHUB_URL
            .scope(
                url,
                settle(&store, &repository_42(), Some(until), Some(&asker(None))),
            )
            .await;
        let Err(error) = result else {
            panic!("the token no longer reaches the repository")
        };
        assert!(error.to_string().contains(APP_URL), "{error}");
        assert!(store.rentals(None).unwrap().is_empty());
        // Renewing would spend the grant on a token that reaches the same.
        assert!(store.github_rental_grant(42).unwrap() == Some(held));
        assert_eq!(requests.lock().await.len(), 3);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_stored_token_github_rejects_is_renewed_before_anything_is_rented() {
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), Vec::new());
        store
            .put_github_rental_grant(&grant(42, "ghu_REVOKED", 6))
            .unwrap();
        let mut responses = vec![json_response(StatusCode::UNAUTHORIZED, json!({}))];
        responses.extend(renewal(42, "ghu_NEW", 8));
        let (url, requests) = spawn(responses).await;
        let until = OffsetDateTime::now_utc() + Duration::hours(4);
        let settled = TEST_GITHUB_URL
            .scope(
                url,
                settle(&store, &repository_42(), Some(until), Some(&asker(None))),
            )
            .await
            .unwrap();
        assert!(!settled.approved);
        let renewed = store.github_rental_grant(42).unwrap().unwrap();
        assert!(renewed.access_token == "ghu_NEW");
        assert_eq!(renewals(&requests.lock().await), 1);
    }

    #[test]
    fn the_renewal_is_named_only_when_github_stops_before_the_rental() {
        let stops = OffsetDateTime::now_utc();
        let outlasted = access_line("code-1", stops, stops - Duration::hours(1));
        assert!(!outlasted.contains("renews"), "{outlasted}");
        let short = access_line("code-1", stops, stops + Duration::hours(1));
        assert!(
            short.ends_with("before the rental ends; `lf machine login github code-1` renews"),
            "{short}"
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn a_saved_template_listing_github_loads_without_it() {
        use crate::provider_auth::Provider as Login;
        let _env = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
        let store = renting(dir.path(), vec![Login::Codex, Login::GitHub, Login::Linear]);
        let read = crate::store::rentals::ondemand(&store, "code")
            .unwrap()
            .unwrap();
        assert_eq!(read.logins, [Login::Codex]);
    }
}
