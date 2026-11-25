//! Provider planning operations shared by Task, Wave, and repository commands.
//!
//! Work owns authored Project content and Tasks. Linear refresh accepts tracker
//! facts through the shared writer; ordinary readers consume saved Work.

use super::planning_sync::{lock_wave_planning, planning_store};
use std::collections::BTreeSet;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::linear::LinearClient;
use crate::linear::LinearError;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::provider_auth::{
    provider_token_refresh_due, refresh_stored_provider_token, Provider, TokenRefreshError,
};
use crate::repository::RepoId;
use crate::store::{
    open_existing_store, LinearIssueState, PlanningState, ProviderToken, ProviderTokenReplacement,
    StorageConfig, Store,
};
use crate::work::wave::Wave;
use loopflow_model::id::WaveId;
use loopflow_storage::planning::{ProjectInventory, TrackerProject};
use loopflow_storage::work::task::Task;
use loopflow_storage::work::wave::normalize_wave_name;
use serde::{Deserialize, Serialize};

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct LinearInitOptions {
    pub wave: Option<String>,
    /// The existing Linear Project to link: its URL, id or name. Without one,
    /// the Wave's Project is made in the repository's Team.
    pub project: Option<String>,
    /// Repository Team key (Task prefix, e.g. `LOO`). Defaults from the repository name.
    pub team_key: Option<String>,
    /// Repository Team display name. Defaults to the repository name.
    pub team_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinearInitResult {
    pub wave: String,
    /// The Linear Project the Wave's Project links; `None` while Loopflow is
    /// still making it.
    #[serde(deserialize_with = "Option::deserialize")]
    pub project_id: Option<String>,
    pub project_name: String,
    /// Whether the Wave's Project is made in Linear rather than linked to one
    /// that already existed.
    pub created: bool,
    /// Stable id of the repository Team (owns the Task prefix).
    pub team_id: String,
    /// The repository Team's current display key (Task prefix).
    pub team_key: String,
    /// Whether this run created the team (vs. adopted an existing one).
    pub team_created: bool,
    /// The binding's file (`.lf/config.yaml`) on the main checkout.
    pub files: super::wave_lifecycle::AuthoredFiles,
}

/// How planning reads reconcile the local snapshot with Linear before reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinearRefresh {
    /// Refresh only when the snapshot is stale (the default TTL policy).
    #[default]
    Auto,
    /// Always refresh before reading (`--sync`).
    Force,
    /// Never touch the network; serve the cache as-is (the default for status reads).
    Never,
}

#[derive(Debug, Clone, Default)]
pub struct LinearSyncOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearSyncResult {
    pub actions: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LinearReteamOptions {
    /// Execute the moves. Without it, `reteam` only prints the plan (dry run).
    pub apply: bool,
}

/// One issue that moves into the repository Team. `new_identifier` is filled only
/// after an applied move (Linear assigns the number then).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinearReteamMove {
    pub wave: String,
    pub project_id: String,
    pub id: String,
    pub old_identifier: String,
    pub title: String,
    pub new_identifier: Option<String>,
}

/// One Project narrowed onto the repository Team. Projects keep their id and slug on
/// a team move (Linear only renumbers issues), so there is no new identifier to
/// carry — `from_teams` records where it came from for the plan output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinearReteamProjectMove {
    pub wave: String,
    pub id: String,
    pub name: String,
    pub from_teams: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinearReteamResult {
    #[serde(rename = "repo")]
    pub repository: String,
    pub waves: Vec<String>,
    pub team_id: String,
    pub team_key: String,
    /// True when moves were executed; false for a dry run.
    pub applied: bool,
    /// Projects narrowed onto the repository Team (after their issues moved
    /// off the legacy team).
    pub project_moves: Vec<LinearReteamProjectMove>,
    pub moves: Vec<LinearReteamMove>,
    /// Issues already carrying the target Team id (skipped — idempotency).
    pub already: usize,
    /// Durable Tasks whose cached display identifier was reconciled.
    pub task_updates: usize,
}

// ── Client + Linear project resolution ──────────────────────────────

#[derive(Debug, Clone)]
pub struct LinearRepository {
    pub client: LinearClient,
    pub repo_id: RepoId,
    pub team_id: String,
}

pub(crate) fn resolve_wave(wave: Option<&str>) -> OpsResult<String> {
    wave.and_then(normalize_wave_name)
        .ok_or_else(|| OpsError::Message("cannot determine wave; pass --wave <name>".to_string()))
}

pub fn selected_linear_team(repo: &Path) -> OpsResult<Option<String>> {
    // The team is repository identity even when a personal preference selects
    // another synchronization transport. Connecting must retain that binding.
    let transport = crate::config::load_repo_config(repo)
        .map_err(|error| {
            OpsError::Message(format!("failed to read planning configuration: {error}"))
        })?
        .unwrap_or_default()
        .planning_transport();
    Ok(match transport {
        Some(crate::config::TrackerConfig::Linear { team }) => {
            team.filter(|team| !team.trim().is_empty())
        }
        _ => None,
    })
}

pub(crate) fn repository_team_id(repo: &Path) -> OpsResult<String> {
    selected_linear_team(repo)?.ok_or_else(|| {
        OpsError::Message(
            ".lf/config.yaml has no `repo.tracker.linear.team`. \
             Run `lf wave plan connect <wave> --team-key <KEY>` before creating or mutating work."
                .to_string(),
        )
    })
}

async fn build_client(team: Option<String>) -> OpsResult<LinearClient> {
    let token = resolve_planning_token(Provider::Linear).await?;
    #[cfg(any(test, feature = "test-support"))]
    if let Ok(url) = LINEAR_TEST_CONTEXT.try_with(|ctx| ctx.graphql_url.clone()) {
        return Ok(LinearClient::with_base_url(token, team, url));
    }
    Ok(LinearClient::new(token, team))
}

fn repository_id(repo: &Path) -> OpsResult<RepoId> {
    RepoId::discover(repo).map_err(|error| {
        OpsError::Message(format!(
            "cannot establish repository Linear identity from Git origin: {error}. \
             Configure an origin before running `lf wave plan connect`."
        ))
    })
}

/// Team claims this process has validated, by (Team, checkout). A claim changes
/// only through `lf reteam`, so one `ListTeams` per process suffices; a
/// long-lived process learns a rebinding on restart.
static VALIDATED_TEAM_CLAIMS: std::sync::Mutex<BTreeSet<(String, std::path::PathBuf)>> =
    std::sync::Mutex::new(BTreeSet::new());

async fn resolve_repository_context(repo: &Path) -> OpsResult<LinearRepository> {
    let team_id = repository_team_id(repo)?;
    let repo_id = repository_id(repo)?;
    let client = build_client(Some(team_id.clone()))
        .await?
        .for_repository(repo);
    let claim = (team_id.clone(), repo.to_path_buf());
    let validated = VALIDATED_TEAM_CLAIMS
        .lock()
        .expect("team claim cache poisoned")
        .contains(&claim);
    if !validated {
        client
            .validate_team_claim(&team_id, repo_id.as_str())
            .await
            .map_err(linear_error)?;
        VALIDATED_TEAM_CLAIMS
            .lock()
            .expect("team claim cache poisoned")
            .insert(claim);
    }
    Ok(LinearRepository {
        client,
        repo_id,
        team_id: team_id.clone(),
    })
}

/// A configured Linear client for repository-scoped webhook and exact-Issue operations.
pub async fn linear_client(repo: &Path) -> OpsResult<LinearClient> {
    Ok(resolve_repository_context(repo).await?.client)
}

/// A Task already names its linked issue; comment access needs no team discovery.
pub(crate) async fn issue_client() -> OpsResult<LinearClient> {
    build_client(None).await
}

/// The Linear Project `wave` links: its Project's tracker id, once it has one.
async fn linked_project(store: &Store, wave: &WaveId) -> OpsResult<Option<String>> {
    Ok(store
        .project_of_wave(wave)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .and_then(|project| project.tracker_id)
        .map(|id| id.as_str().to_owned()))
}

/// This repository's live Waves whose Project links a Linear Project.
async fn linked_waves(repo: &Path, store: &Store) -> OpsResult<Vec<(Wave, String)>> {
    let canonical =
        crate::repository::discover(repo).map_err(|error| OpsError::Message(error.to_string()))?;
    let mut linked = Vec::new();
    for wave in store
        .list_waves(Some(&canonical.to_string()))
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
    {
        if wave.is_retired() {
            continue;
        }
        if let Some(project) = linked_project(store, wave.id()).await? {
            linked.push((wave, project));
        }
    }
    Ok(linked)
}

pub(super) async fn resolve_planning_token(provider: Provider) -> OpsResult<String> {
    Ok(planning_token(provider, false).await?.access_token)
}

/// This laptop's credential just refreshed, so its access token lives as long as
/// the provider grants. The refresh grant stays here; a machine handed the access
/// token cannot rotate it away.
pub(crate) async fn renewed_planning_token(provider: Provider) -> OpsResult<ProviderToken> {
    planning_token(provider, true).await
}

async fn planning_token(provider: Provider, renew: bool) -> OpsResult<ProviderToken> {
    let deadline = Instant::now() + LINEAR_REFRESH_TIMEOUT;
    tokio::time::timeout(LINEAR_REFRESH_TIMEOUT, async {
        let config = super::planning_sync::storage_config_from_env()?;
        let store = super::planning_sync::open_planning_store(&config).await?;
        let StorageConfig::Sqlite { path } = config;
        resolve_planning_token_from_store(provider, &store, &path, deadline, renew).await
    })
    .await
    .map_err(|_| credential_deadline(provider))?
}

fn missing_credential(provider: Provider) -> OpsError {
    OpsError::Message(format!(
        "No {} credential found. Run `doppler run -- lf machine login {provider}`.",
        provider.display_name()
    ))
}

fn credential_retry(provider: Provider, reason: &str) -> OpsError {
    OpsError::Message(format!(
        "{reason}. Retry the {} operation; no credential was cleared.",
        provider.display_name()
    ))
}

fn credential_deadline(provider: Provider) -> OpsError {
    credential_retry(provider, &format!("{} credential resolution deadline elapsed; a pending write may still settle, so the next call must re-read the credential", provider.display_name()))
}

fn usable_token(provider: Provider, token: ProviderToken) -> OpsResult<ProviderToken> {
    if token.access_token.trim().is_empty()
        || token
            .expires_at
            .is_some_and(|expiry| expiry <= time::OffsetDateTime::now_utc().unix_timestamp())
    {
        return Err(credential_retry(
            provider,
            &format!(
                "Current {} credential is unusable or expired",
                provider.display_name()
            ),
        ));
    }
    Ok(token)
}

async fn planning_refresh_lock(
    provider: Provider,
    database: &Path,
    deadline: Instant,
) -> OpsResult<std::fs::File> {
    let database = std::fs::canonicalize(database)
        .map_err(|_| credential_retry(provider, "Could not resolve the credential store path"))?;
    let mut path = database.into_os_string();
    path.push(format!(".{provider}-refresh.lock"));
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| credential_retry(provider, "Could not open the refresh lock"))?;
    loop {
        if Instant::now() >= deadline {
            return Err(credential_deadline(provider));
        }
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(file),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(_) => {
                return Err(credential_retry(
                    provider,
                    "Could not acquire the refresh lock",
                ))
            }
        }
    }
}

async fn resolve_planning_token_from_store(
    provider: Provider,
    store: &Store,
    database: &Path,
    deadline: Instant,
    renew: bool,
) -> OpsResult<ProviderToken> {
    let due = |token: &ProviderToken| {
        provider_token_refresh_due(token, time::OffsetDateTime::now_utc().unix_timestamp())
            || (renew && token.refresh_token.is_some())
    };
    let read = || async {
        store
            .get_provider_token(provider.as_str())
            .await
            .map_err(|_| credential_retry(provider, "Could not read the credential"))
    };
    let initial = read().await?.ok_or_else(|| missing_credential(provider))?;
    if !due(&initial) {
        return usable_token(provider, initial);
    }
    let lock = planning_refresh_lock(provider, database, deadline).await?;
    let current = read().await?.ok_or_else(|| missing_credential(provider))?;
    if !due(&current) {
        return usable_token(provider, current);
    }
    for attempt in 0..2 {
        match refresh_stored_provider_token(provider, &current).await {
            Ok(refreshed) => {
                // Persistence owns the file descriptor even if this future is cancelled.
                let replacement = store
                    .replace_provider_token(&current, &refreshed, lock, deadline)
                    .await;
                return match replacement {
                    Ok(ProviderTokenReplacement::Replaced) => usable_token(provider, refreshed),
                    Ok(ProviderTokenReplacement::Changed(winner)) => usable_token(provider, winner),
                    Ok(ProviderTokenReplacement::Missing) => Err(missing_credential(provider)),
                    Err(_) => {
                        // A failed write (including an uncertain commit) cannot authorize
                        // an unpersisted response. Re-read before considering fallback.
                        if let Some(latest) = read().await? {
                            if let Ok(access) = usable_token(provider, latest) {
                                tracing::warn!(%provider, "refresh persistence failed; using the current stored token");
                                return Ok(access);
                            }
                        }
                        Err(credential_retry(provider, "Could not persist the refreshed credential; re-read it on the next call"))
                    }
                };
            }
            Err(error) => {
                let latest = read().await?;
                if latest.as_ref() != Some(&current) {
                    return usable_token(
                        provider,
                        latest.ok_or_else(|| missing_credential(provider))?,
                    );
                }
                let TokenRefreshError::OAuth { reason, .. } = error else {
                    return Err(credential_retry(provider, "Credential refresh failed"));
                };
                if attempt == 0 && reason.retryable_now() {
                    continue;
                }
                if let Ok(access) = usable_token(provider, current.clone()) {
                    tracing::warn!(error = %reason, %provider, "proactive planning refresh failed; using the still-valid token");
                    return Ok(access);
                }
                return Err(if reason.requires_reconnect() {
                    OpsError::Message(format!("{} refresh failed: {reason}. Run `doppler run -- lf machine login {provider}` to reconnect.", provider.display_name()))
                } else {
                    credential_retry(
                        provider,
                        &format!(
                            "{} refresh failed: {reason}; prior credential preserved",
                            provider.display_name()
                        ),
                    )
                });
            }
        }
    }
    unreachable!("both refresh attempts return or retry")
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone)]
pub struct LinearTestContext {
    pub path: std::path::PathBuf,
    pub store: std::sync::Arc<Store>,
    pub(crate) graphql_url: String,
}

#[cfg(any(test, feature = "test-support"))]
tokio::task_local! {
    pub(crate) static LINEAR_TEST_CONTEXT: LinearTestContext;
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_fixture;

#[cfg(test)]
mod asana_planning_tests;

#[cfg(test)]
pub(crate) mod task_planning_tests;

#[cfg(test)]
mod planning_lookup_tests;

#[cfg(test)]
mod oauth_tests;

#[cfg(test)]
mod task_comments_tests;

// ── snapshot freshness policy ────────────────────────────────────────

/// Past this age an Auto read opportunistically refreshes before serving.
pub(crate) const LINEAR_SOFT_STALE_SECS: i64 = 60 * 60; // 1 hour
/// Ceiling on an opportunistic refresh; exceeding it counts as a failure.
const LINEAR_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Refresh `wave` from the Linear Project it links, bounded by
/// `LINEAR_REFRESH_TIMEOUT`; `false` when it links none. A timeout, an auth
/// failure, or any network error surfaces as `Err`; saved Work remains intact.
pub async fn refresh_wave(repo: &Path, wave: &str) -> OpsResult<bool> {
    let work = async {
        let ctx = resolve_repository_context(repo).await?;
        refresh_inventory(repo, wave, &ctx).await
    };
    match tokio::time::timeout(LINEAR_REFRESH_TIMEOUT, work).await {
        Ok(result) => result,
        Err(_) => Err(OpsError::Message(format!(
            "Planning refresh exceeded its {}s deadline. Retry the Linear operation; a pending credential write may still settle and will be re-read",
            LINEAR_REFRESH_TIMEOUT.as_secs()
        ))),
    }
}

pub(crate) async fn refresh_inventory(
    repo: &Path,
    wave: &str,
    ctx: &LinearRepository,
) -> OpsResult<bool> {
    let store = planning_store().await?;
    let locator = crate::work::wave::discover(repo, wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let registered = store
        .get_wave_at(&locator)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| {
            OpsError::Message(format!(
                "no Wave {wave} here; run `lf wave add {wave}` first"
            ))
        })?;
    let locks = lock_wave_planning(registered.id()).await?;
    refresh_inventory_locked(&registered, ctx, &store, locks).await
}

pub(crate) async fn refresh_inventory_locked(
    wave: &Wave,
    ctx: &LinearRepository,
    store: &Store,
    locks: Arc<crate::store::PlanningLocks>,
) -> OpsResult<bool> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let Some(inventory) = read_wave_inventory(ctx, wave, store).await? else {
        return Ok(false);
    };
    crate::linear::events::accept_linear_inventory(
        store,
        wave.id().clone(),
        inventory,
        observed_at,
        Some(locks),
    )
    .await
    .map_err(|err| OpsError::Message(format!("failed to accept Linear inventory: {err}")))?;
    Ok(true)
}

// ── init ────────────────────────────────────────────────────────────

pub fn connect(
    repo: &Path,
    options: &LinearInitOptions,
    progress: &impl Progress,
) -> OpsResult<LinearInitResult> {
    block_on_linear(connect_async(repo, options, progress))
}

/// Link a Wave's Project to a Linear Project in the repository Team: the one
/// `options.project` names, else one Loopflow makes for it. Register a new
/// Wave locally so its Project can take the named link before any export.
async fn connect_async(
    repo: &Path,
    options: &LinearInitOptions,
    progress: &impl Progress,
) -> OpsResult<LinearInitResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let main = crate::repository::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .as_path()
        .to_path_buf();
    let repo = main.as_path();
    let store = planning_store().await?;
    let locator = crate::work::wave::discover(repo, &wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let repo_id = repository_id(repo)?;
    // Connecting selects a transport; it must first preserve the existing binding.
    let existing_team = selected_linear_team(repo)?;
    let client = build_client(existing_team.clone()).await?;
    let team_name = options
        .team_name
        .clone()
        .unwrap_or_else(|| title_case(repo_id.name()));
    let team_key = options
        .team_key
        .clone()
        .unwrap_or_else(|| default_team_key(repo_id.name()));
    let team = match existing_team.as_deref() {
        Some(team_id) => client
            .claim_configured_team(
                team_id,
                repo_id.as_str(),
                options.team_name.as_deref(),
                options.team_key.as_deref(),
            )
            .await
            .map_err(linear_error)?,
        None => client
            .ensure_team(&team_name, &team_key, repo_id.as_str())
            .await
            .map_err(linear_error)?,
    };
    let files = if write_repository_linear_config(repo, &team.id)? {
        super::wave_lifecycle::commit_authored(
            repo,
            &[".lf/config.yaml".to_string()],
            &format!("lf wave plan connect: {wave} to Linear"),
        )
    } else {
        super::wave_lifecycle::AuthoredFiles::Unchanged
    };

    let ctx = resolve_repository_context(repo).await?;
    // Find the named Project before adding a Wave: a Wave added for a connect
    // that then fails would be left holding a Project of its own.
    let named = match options.project.as_deref() {
        Some(key) => {
            progress.status(&format!("looking for Linear Project `{key}`"));
            let project = ctx
                .client
                .named_project(key)
                .await
                .map_err(linear_error)?
                .ok_or_else(|| OpsError::Message(format!("no Linear Project matches `{key}`")))?;
            validate_project_ownership(&project, &wave, &ctx.team_id)?;
            Some(project)
        }
        None => None,
    };
    // Hold the same lifecycle lock through the check, registration and link so
    // competing connects cannot both register a Wave for the same Project.
    let lifecycle = super::wave_lifecycle::recovery::lock_async(repo).await?;
    let registered = store
        .get_wave_at(&locator)
        .await
        .map_err(super::tracker::message)?;
    let wave_id = match registered {
        Some(registered) => registered.id().clone(),
        None => {
            if let Some(project) = &named {
                let id = project.id.clone();
                crate::store::run_sqlite(&store.sqlite, move |sqlite| {
                    loopflow_storage::sqlite::linear_events::check_project_link(&sqlite, &id)
                })
                .await
                .map_err(super::tracker::message)?;
            }
            progress.status(&format!("adding Wave {wave}"));
            super::wave_lifecycle::add_local(&store, repo, &wave, None)
                .await?
                .id
        }
    };
    let (own, linked) = match store
        .project_of_wave(&wave_id)
        .await
        .map_err(super::tracker::message)?
    {
        Some(project) => (project.id, project.tracker_id),
        None => (loopflow_model::durable::ProjectId::for_wave(&wave_id), None),
    };
    let create = named.is_none() && linked.is_none();
    if let Some(project) =
        named.filter(|project| linked.as_ref().map(|id| id.as_str()) != Some(project.id.as_str()))
    {
        progress.status(&format!(
            "linking wave/{wave} to Linear Project `{}`",
            project.name
        ));
        let locks = lock_wave_planning(&wave_id).await?;
        let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let linking = wave_id.clone();
        crate::store::run_planning_write(&store.sqlite, Some(locks), move |sqlite| {
            loopflow_storage::sqlite::linear_events::link_wave_project_in(
                &sqlite,
                &linking,
                &project,
                observed_at,
                loopflow_storage::planning_sync::TrackerKind::Linear,
            )
        })
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    }
    drop(lifecycle);
    if create {
        progress.status(&format!("making wave/{wave}'s Project in Linear"));
        super::tracker::create_wave_project(&store, repo, &wave_id).await?;
    }
    refresh_inventory(repo, &wave, &ctx).await?;
    let project = store
        .get_project(&own)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message(format!("wave/{wave} has no Project")))?;
    let project_id = project.tracker_id.map(|id| id.as_str().to_owned());
    let created = project_id.as_deref().is_some_and(|id| own.made_as(id));
    Ok(LinearInitResult {
        wave,
        project_id,
        project_name: project.name,
        created,
        team_id: team.id,
        team_key: team.key,
        team_created: team.created,
        files,
    })
}

// ── status ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) struct TaskPlanningInspection {
    pub issue_state: LinearIssueState,
    pub refresh_error: Option<String>,
}

pub(crate) async fn inspect_task_planning_async(
    repo: &Path,
    issue: &str,
    refresh: LinearRefresh,
) -> OpsResult<TaskPlanningInspection> {
    let scope = crate::repository::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .to_string();
    let store = planning_store().await?;
    let existing = store
        .linear_issue_state(&scope, issue)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let age = existing.observed_at.map(|at| now - at);
    if existing.state == PlanningState::Removed
        || refresh == LinearRefresh::Never
        || (existing.state == PlanningState::Available
            && refresh == LinearRefresh::Auto
            && (age.is_some_and(|age| age < LINEAR_SOFT_STALE_SECS)
                || super::tracker_events::team_current(&store, &scope).await))
    {
        return Ok(TaskPlanningInspection {
            issue_state: existing,
            refresh_error: None,
        });
    }
    let selector = existing.issue_id.as_deref().unwrap_or(issue);
    let fetch = async {
        let repository = resolve_repository_context(repo).await?;
        let mut observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut observation = repository
            .client
            .issue_ownership(selector)
            .await
            .map_err(linear_error)?;
        // An issue in a linked Project is planned under that Project's Wave.
        let discovered = observation
            .as_ref()
            .and_then(|(_, project)| project.as_ref().map(|project| project.id.clone()))
            .or_else(|| existing.project_id.clone());
        let mut locks = None;
        if let Some(discovered) = discovered {
            if let Some(local) = store
                .project_by_tracker_id(&discovered)
                .await
                .map_err(super::tracker::message)?
            {
                locks = Some(lock_wave_planning(&local.wave_id).await?);
                observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
                observation = repository
                    .client
                    .issue_ownership(selector)
                    .await
                    .map_err(linear_error)?;
                if let Some((_, Some(project))) = &observation {
                    if project.id != discovered {
                        return Err(OpsError::Message(format!(
                            "Task {selector} ownership changed during discovery; retry planning"
                        )));
                    }
                }
            }
        }
        let Some((item, project)) = observation else {
            if existing.issue_id.is_some() {
                store
                    .invalidate_linear_issue(&scope, existing.clone(), locks)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?;
            }
            return Ok(false);
        };
        if item.team_id.as_deref() != Some(repository.team_id.as_str()) {
            return Err(OpsError::Message(format!(
                "Linear task {} belongs to Team {}, expected repository Team {}",
                item.identifier,
                item.team_id.as_deref().unwrap_or("unmapped"),
                repository.team_id
            )));
        }
        crate::linear::events::accept_linear_issue(
            &store,
            &scope,
            item,
            project,
            observed_at,
            locks,
        )
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
        Ok(true)
    };
    let result = match tokio::time::timeout(LINEAR_REFRESH_TIMEOUT, fetch).await {
        Ok(result) => result,
        Err(_) => Err(OpsError::Message(format!(
            "Task planning refresh exceeded its {}s deadline",
            LINEAR_REFRESH_TIMEOUT.as_secs()
        ))),
    };
    // Read again: an event may have invalidated the record during the refresh.
    let mut issue_state = store
        .linear_issue_state(&scope, selector)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let refresh_error = match result {
        Ok(false) if matches!(issue_state.state, PlanningState::Removed | PlanningState::Available) => None,
        Ok(false) => {
            issue_state.state = PlanningState::Absent;
            Some(format!("task {issue:?} is absent from repository planning"))
        }
        Ok(true) if issue_state.state != PlanningState::Available => Some(format!(
            "Task {issue:?} refresh cannot repair stored invalidation, unresolved Project ownership or confirmed removal"
        )),
        Ok(true) => None,
        Err(error) => {
            if issue_state.state == PlanningState::Available {
                issue_state.state = PlanningState::Unavailable;
            }
            Some(format!("unable to resolve task {issue:?}: {error}"))
        }
    };
    Ok(TaskPlanningInspection {
        issue_state,
        refresh_error,
    })
}

pub(crate) async fn fetch_task(
    repo: &Path,
    issue: &str,
    refresh: LinearRefresh,
) -> OpsResult<Task> {
    let read = inspect_task_planning_async(repo, issue, refresh).await?;
    let issue_state = read.issue_state;
    if issue_state.state != PlanningState::Available {
        return Err(OpsError::Message(read.refresh_error.unwrap_or_else(|| {
            format!(
                "Task {issue:?} planning is {:?}; run `lf task sync {issue}`",
                issue_state.state
            )
        })));
    }
    let team = repository_team_id(repo)?;
    let store = planning_store().await?;
    if let Some(project) = &issue_state.project_id {
        if store
            .project_by_tracker_id(project)
            .await
            .map_err(super::tracker::message)?
            .is_none()
        {
            return Err(OpsError::Message(format!(
                "Linear Project {project} is linked by no Wave; run `lf wave plan connect <wave> {project}`"
            )));
        }
        if issue_state.project_team_ids.as_slice() != [team.as_str()] {
            return Err(OpsError::Message(format!(
                "Project {project} must belong to repository Team {team}"
            )));
        }
    }
    if issue_state.team_id.as_deref() != Some(team.as_str()) {
        return Err(OpsError::Message(format!(
            "Task {issue} must belong to repository Team {team}"
        )));
    }
    let task = store
        .get_task_by_issue(issue_state.issue_id.as_deref().unwrap_or(issue))
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| {
            OpsError::Message("accepted planning did not retain the Task identity".into())
        })?;
    let tracker_state = store
        .sqlite
        .task_tracker_state(&task.id)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if tracker_state.state != PlanningState::Available {
        return Err(OpsError::Message(format!(
            "Task {issue} has invalid retained ownership; run `lf task sync {issue}`"
        )));
    }
    Ok(task)
}

// ── reteam ──────────────────────────────────────────────────────────

struct LockedWave {
    wave: Wave,
    project: String,
    locks: Arc<crate::store::PlanningLocks>,
}

struct ResolvedReteamContext {
    repository: LinearRepository,
    team_key: String,
    store: Store,
}

fn reteam_comment_marker(old_identifier: &str, team_key: &str) -> String {
    format!("was {old_identifier}; moving onto team {team_key}")
}

fn reteam_comment_body(old_identifier: &str, team_key: &str) -> String {
    format!(
        "Reteamed by loopflow: {}. The issue id (UUID) is unchanged; Linear reassigns the number on the move.",
        reteam_comment_marker(old_identifier, team_key)
    )
}

async fn resolve_reteam_context(repo: &Path) -> OpsResult<ResolvedReteamContext> {
    let team_id = selected_linear_team(repo)?.ok_or_else(|| {
        OpsError::Message(
            ".lf/config.yaml has no `repo.tracker.linear.team`. \
             Run `lf wave plan connect <wave> --team-key <KEY>` to establish the migration target."
                .to_string(),
        )
    })?;
    let repo_id = repository_id(repo)?;
    let client = build_client(Some(team_id.clone()))
        .await?
        .for_repository(repo);
    let binding = client
        .validate_team_claim(&team_id, repo_id.as_str())
        .await
        .map_err(linear_error)?;
    let store = planning_store().await?;

    Ok(ResolvedReteamContext {
        repository: LinearRepository {
            client,
            repo_id,
            team_id,
        },
        team_key: binding.key,
        store,
    })
}

pub fn reteam(
    repo: &Path,
    options: &LinearReteamOptions,
    progress: &impl Progress,
) -> OpsResult<LinearReteamResult> {
    block_on_linear(reteam_async(repo, options, progress))
}

async fn reteam_async(
    repo: &Path,
    options: &LinearReteamOptions,
    progress: &impl Progress,
) -> OpsResult<LinearReteamResult> {
    let resolved = resolve_reteam_context(repo).await?;
    apply_or_plan_repository_reteam(&resolved, repo, options.apply, progress).await
}

async fn accept_reteam_project(
    resolved: &ResolvedReteamContext,
    linked: &LockedWave,
    project: &TrackerProject,
    expected_teams: &[String],
) -> OpsResult<()> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let confirmed = resolved
        .repository
        .client
        .project_ownership(&project.id)
        .await
        .map_err(linear_error)?;
    if confirmed.team_ids.len() != expected_teams.len()
        || !expected_teams
            .iter()
            .all(|team| confirmed.team_ids.contains(team))
    {
        return Err(OpsError::Message(format!(
            "Project {} ownership changed during reteam",
            project.id
        )));
    }
    crate::linear::events::reconcile_linear_project_teams(
        &resolved.store,
        linked.wave.id(),
        confirmed,
        observed_at,
        linked.locks.clone(),
    )
    .await
    .map_err(|error| OpsError::Message(error.to_string()))
}

async fn accept_reteam_task(
    resolved: &ResolvedReteamContext,
    linked: &LockedWave,
    issue: &str,
) -> OpsResult<(String, bool)> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let (item, project) = resolved
        .repository
        .client
        .issue_ownership(issue)
        .await
        .map_err(linear_error)?
        .ok_or_else(|| OpsError::Message(format!("Task {issue} disappeared during reteam")))?;
    let project = project
        .ok_or_else(|| OpsError::Message(format!("Task {issue} lost its Project during reteam")))?;
    if item.team_id.as_deref() != Some(resolved.repository.team_id.as_str()) {
        return Err(OpsError::Message(format!(
            "Task {issue} ownership changed during reteam"
        )));
    }
    let identifier = item.identifier.clone();
    let changed = resolved
        .store
        .task_issue_identifier(issue)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .is_some_and(|previous| previous != identifier);
    crate::linear::events::accept_linear_issue(
        &resolved.store,
        linked.wave.repo(),
        item,
        Some(project),
        observed_at,
        Some(linked.locks.clone()),
    )
    .await
    .map_err(|error| OpsError::Message(error.to_string()))?;
    Ok((identifier, changed))
}

async fn apply_or_plan_repository_reteam(
    resolved: &ResolvedReteamContext,
    repo: &Path,
    apply: bool,
    progress: &impl Progress,
) -> OpsResult<LinearReteamResult> {
    let team_id = &resolved.repository.team_id;
    let team_key = &resolved.team_key;
    let store = &resolved.store;
    if apply {
        // A clone's authored Waves join the store before the move; a removed one stays removed.
        crate::work::wave::bootstrap::register_authored_waves(
            &store.sqlite,
            &repo.to_string_lossy(),
        )
        .map_err(|error| OpsError::Message(error.to_string()))?;
    }
    let mut linked = linked_waves(repo, store).await?;
    if linked.is_empty() {
        return Err(OpsError::Message(
            "repository has no Waves linked to Linear Projects".to_string(),
        ));
    }
    // Acquire locks by stable ID, then present and process Waves by name.
    linked.sort_by(|(left, _), (right, _)| left.id().as_str().cmp(right.id().as_str()));
    let mut locked_waves = Vec::with_capacity(linked.len());
    for (wave, project) in linked {
        let locks = lock_wave_planning(wave.id()).await?;
        locked_waves.push(LockedWave {
            wave,
            project,
            locks,
        });
    }
    locked_waves.sort_by(|a, b| a.wave.slug().cmp(b.wave.slug()));
    let mut project_moves = Vec::new();
    let mut moves = Vec::new();
    let mut identifier_updates = Vec::new();
    let mut projects = Vec::new();
    let mut already = 0usize;
    let mut task_updates = 0usize;

    for entry in &locked_waves {
        let name = entry.wave.slug();
        progress.status(&format!(
            "preflighting wave/{name} Project {}",
            entry.project
        ));
        let project = resolved
            .repository
            .client
            .project_ownership(&entry.project)
            .await
            .map_err(linear_error)?;
        if project.team_ids.as_slice() != [team_id.as_str()] {
            project_moves.push(LinearReteamProjectMove {
                wave: name.to_owned(),
                id: project.id.clone(),
                name: project.name.clone(),
                from_teams: project.team_ids.clone(),
            });
        }
        let items = resolved
            .repository
            .client
            .list_items(&project.id)
            .await
            .map_err(linear_error)?;
        for item in items {
            if item.project_id.as_deref() != Some(project.id.as_str()) {
                return Err(OpsError::Message(format!(
                    "Linear task {} resolves to Project {:?}, expected {}",
                    item.identifier, item.project_id, project.id
                )));
            }
            if !project
                .team_ids
                .iter()
                .any(|team| Some(team) == item.team_id.as_ref())
            {
                return Err(OpsError::Message(format!(
                    "Linear task {} belongs to Team {}, but Project {} carries teams [{}]",
                    item.identifier,
                    item.team_id.as_deref().unwrap_or("unmapped"),
                    project.id,
                    project.team_ids.join(", ")
                )));
            }
            if item.team_id.as_deref() == Some(team_id.as_str()) {
                already += 1;
                let registered_identifier =
                    store
                        .task_issue_identifier(&item.id)
                        .await
                        .map_err(|error| {
                            OpsError::Message(format!("failed to read task registry: {error}"))
                        })?;
                if registered_identifier.is_some_and(|identifier| identifier != item.identifier) {
                    identifier_updates.push((entry, item.id));
                }
            } else {
                moves.push((
                    entry,
                    LinearReteamMove {
                        wave: name.to_owned(),
                        project_id: project.id.clone(),
                        id: item.id,
                        old_identifier: item.identifier,
                        title: item.name,
                        new_identifier: None,
                    },
                ));
            }
        }
        projects.push((entry, project));
    }

    if apply {
        // Linear requires the destination Team on a Project before its Issues
        // can move. Expand first; narrowing is the final tracker phase.
        for (entry, project) in &projects {
            let mut expected_teams = project.team_ids.clone();
            if !expected_teams.contains(team_id) {
                expected_teams.push(team_id.clone());
                progress.status(&format!(
                    "attaching team {team_key} to Project `{}`",
                    project.name
                ));
                resolved
                    .repository
                    .client
                    .set_project_teams(&project.id, &expected_teams)
                    .await
                    .map_err(linear_error)?;
            }
            accept_reteam_project(resolved, entry, project, &expected_teams).await?;
        }
        for (entry, issue) in identifier_updates {
            let (_, changed) = accept_reteam_task(resolved, entry, &issue).await?;
            task_updates += usize::from(changed);
        }

        for (entry, mv) in &mut moves {
            let marker = reteam_comment_marker(&mv.old_identifier, team_key);
            let issue = resolved
                .repository
                .client
                .fetch_issue(&mv.id)
                .await
                .map_err(linear_error)?;
            if !issue
                .comments
                .iter()
                .any(|comment| comment.body.contains(&marker))
            {
                resolved
                    .repository
                    .client
                    .comment(&mv.id, &reteam_comment_body(&mv.old_identifier, team_key))
                    .await
                    .map_err(linear_error)?;
            }
            progress.status(&format!(
                "moving {} into team {team_key}",
                mv.old_identifier
            ));
            let new_identifier = resolved
                .repository
                .client
                .move_item_to_team(&mv.id, team_id)
                .await
                .map_err(linear_error)?;
            let (confirmed_identifier, changed) =
                accept_reteam_task(resolved, entry, &mv.id).await?;
            if confirmed_identifier != new_identifier {
                return Err(OpsError::Message(format!(
                    "Task {} move is not confirmed",
                    mv.id
                )));
            }
            task_updates += usize::from(changed);
            mv.new_identifier = Some(new_identifier);
        }

        for (entry, project) in &projects {
            if project.team_ids.as_slice() != [team_id.as_str()] {
                progress.status(&format!(
                    "narrowing Project `{}` onto team {team_key}",
                    project.name
                ));
                resolved
                    .repository
                    .client
                    .set_project_teams(&project.id, std::slice::from_ref(team_id))
                    .await
                    .map_err(linear_error)?;
            }
            accept_reteam_project(resolved, entry, project, std::slice::from_ref(team_id)).await?;
        }

        // Confirm the complete inventory after moving issues and narrowing Teams.
        for entry in &locked_waves {
            refresh_inventory_locked(
                &entry.wave,
                &resolved.repository,
                store,
                entry.locks.clone(),
            )
            .await?;
        }
    }

    Ok(LinearReteamResult {
        repository: resolved.repository.repo_id.to_string(),
        waves: locked_waves
            .iter()
            .map(|entry| entry.wave.slug().to_owned())
            .collect(),
        team_id: team_id.to_string(),
        team_key: team_key.to_string(),
        applied: apply,
        project_moves,
        moves: moves.into_iter().map(|(_, mv)| mv).collect(),
        already,
        task_updates,
    })
}

// ── sync / doctor ──────────────────────────────────────────────────

pub fn refresh(
    repo: &Path,
    options: &LinearSyncOptions,
    progress: &impl Progress,
) -> OpsResult<LinearSyncResult> {
    block_on_linear(refresh_async(repo, options, progress))
}

async fn refresh_async(
    repo: &Path,
    options: &LinearSyncOptions,
    progress: &impl Progress,
) -> OpsResult<LinearSyncResult> {
    let waves = match options.wave.as_deref() {
        Some(wave) => vec![resolve_wave(Some(wave))?],
        None => list_local_waves(repo)?,
    };
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();
    let team_id = selected_linear_team(repo)?;

    if let Some(store) = open_existing_store()
        .await
        .map_err(|error| OpsError::Message(format!("failed to open Wave registry: {error}")))?
    {
        let origin = crate::work::wave::context::wave_origin(repo);
        for wave in store
            .list_waves(Some(&origin.display().to_string()))
            .await
            .map_err(|error| {
                OpsError::Message(format!("failed to inspect Wave registry: {error}"))
            })?
        {
            if wave.parent_wave_id().is_some()
                && wave.promoted_at().is_none()
                && !origin
                    .join(".lf/wave")
                    .join(wave.slug())
                    .join("GOAL.md")
                    .is_file()
            {
                diagnostics.push(format!(
                    "prepared child .lf/wave/{} has no GOAL.md; resume or abandon its promotion",
                    wave.slug()
                ));
            }
        }
    }

    if team_id.is_none() {
        let message = ".lf/config.yaml has no `repo.tracker.linear.team`; run `lf wave plan connect <wave> --team-key <KEY>`".to_string();
        diagnostics.push(message);
    }

    let client = build_client(team_id.clone()).await?;
    let repo_id = repository_id(repo)?;
    if let Some(team_id) = &team_id {
        client
            .validate_team_claim(team_id, repo_id.as_str())
            .await
            .map_err(linear_error)?;
    }
    progress.status("checking the Linear Projects and Tasks Waves link");
    let store = planning_store().await?;
    for wave in &waves {
        let locator = crate::work::wave::discover(repo, wave)
            .map_err(|error| OpsError::Message(error.to_string()))?;
        let Some(registered) = store
            .get_wave_at(&locator)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
        else {
            continue;
        };
        let Some(linked) = linked_project(&store, registered.id()).await? else {
            diagnostics.push(format!(
                "wave/{wave} links no Linear Project; run `{}`",
                super::tracker::make_project_command(wave)
            ));
            continue;
        };
        let project = client
            .project_ownership(&linked)
            .await
            .map_err(linear_error)?;
        let name = &project.name;
        if let Some(team_id) = &team_id {
            if project.team_ids.as_slice() != [team_id.as_str()] {
                diagnostics.push(format!(
                    "Linear Project `{name}` ({}) in wave/{wave} belongs to Teams [{}]; expected exactly repository Team {team_id}. Run `lf reteam`.",
                    project.id,
                    project.team_ids.join(", ")
                ));
            }
        }
        let items = client.list_items(&project.id).await.map_err(linear_error)?;
        if items.iter().all(|item| item.completed) {
            diagnostics.push(format!(
                "Linear Project `{name}` ({}) in wave/{wave} has no open tasks",
                project.id
            ));
        }
        for item in items {
            if item.project_id.as_deref() != Some(project.id.as_str()) {
                diagnostics.push(format!(
                    "Linear task {} resolves to Project {:?}, expected {}",
                    item.identifier, item.project_id, project.id
                ));
            }
            if let Some(team_id) = &team_id {
                if item.team_id.as_deref() != Some(team_id.as_str()) {
                    diagnostics.push(format!(
                        "Linear task {} belongs to Team {}, expected repository Team {team_id}. Run `lf reteam`.",
                        item.identifier,
                        item.team_id.as_deref().unwrap_or("unmapped")
                    ));
                }
            }
        }
        actions.push(format!(
            "refresh wave/{wave} Tasks from Linear Project `{name}` ({})",
            project.id
        ));
    }

    Ok(LinearSyncResult {
        actions,
        diagnostics,
    })
}

// ── explicit mutations ─────────────────────────────────────────────

pub fn list_local_waves(repo: &Path) -> OpsResult<Vec<String>> {
    let canonical =
        crate::repository::discover(repo).map_err(|error| OpsError::Message(error.to_string()))?;
    let crate::store::StorageConfig::Sqlite { path } =
        super::planning_sync::storage_config_from_env()?;
    if !path
        .try_exists()
        .map_err(|error| OpsError::Message(error.to_string()))?
    {
        return Ok(Vec::new());
    }
    let store = loopflow_storage::sqlite::SqliteStore::open_read_only(
        &path,
        crate::store::token_key::encryption_key,
    )
    .map_err(|error| OpsError::Message(error.to_string()))?;
    Ok(store
        .list_waves(Some(&canonical.to_string()))
        .map_err(|error| OpsError::Message(error.to_string()))?
        .into_iter()
        .filter(|wave| !wave.is_retired())
        .map(|wave| wave.slug().to_string())
        .collect())
}

// ── helpers ─────────────────────────────────────────────────────────

/// Declare the Linear tracker in the released layout, retaining unrelated settings and comments.
fn write_repository_linear_config(repo: &Path, team_id: &str) -> OpsResult<bool> {
    let path = repo.join(".lf/config.yaml");
    let planning = serde_yaml_ng::to_value(crate::config::TrackerConfig::Linear {
        team: Some(team_id.to_string()),
    })
    .map_err(|error| OpsError::Message(error.to_string()))?;
    let config_error =
        |error: anyhow::Error| OpsError::Message(format!("{}: {error:#}", path.display()));
    let original = crate::config::files::read_text(&path).map_err(config_error)?;
    let mut document: serde_yaml_ng::Value = if original.trim().is_empty() {
        serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new())
    } else {
        serde_yaml_ng::from_str(&original).map_err(|error| config_error(error.into()))?
    };
    if !document.is_mapping() {
        return Err(OpsError::Message(format!(
            "repository config {} must be a YAML mapping",
            path.display()
        )));
    }
    let previous = document.clone();
    let written = crate::config::keys::gather(
        &mut document,
        &["planning"],
        crate::config::keys::Root::General,
    );
    crate::config::settings::insert(
        &mut document,
        &written.iter().map(String::as_str).collect::<Vec<_>>(),
        planning,
    );
    if document == previous {
        return Ok(false);
    }
    std::fs::create_dir_all(path.parent().expect("config path has parent"))?;
    // The committed file takes no lock file beside it; comments and order stay.
    let text = crate::config::files::edited_text(&original, &document).map_err(config_error)?;
    crate::config::files::atomic_write(&path, &text).map_err(config_error)?;
    Ok(true)
}

/// A default team key (Task prefix) derived from the repository name: the first three
/// alphanumeric characters, uppercased. `--team-key` overrides it.
fn default_team_key(repository: &str) -> String {
    let key: String = repository
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(3)
        .collect::<String>()
        .to_ascii_uppercase();
    if key.len() >= 2 {
        key
    } else {
        "LF".to_string()
    }
}

/// The linked Project and its issues, when the Project belongs to this Team.
async fn read_wave_inventory(
    ctx: &LinearRepository,
    wave: &Wave,
    store: &Store,
) -> OpsResult<Option<ProjectInventory>> {
    let Some(linked) = linked_project(store, wave.id()).await? else {
        return Ok(None);
    };
    let project = ctx
        .client
        .project_ownership(&linked)
        .await
        .map_err(linear_error)?;
    if !project.team_ids.is_empty() && !project.team_ids.contains(&ctx.team_id) {
        return Ok(None);
    }
    validate_project_ownership(&project, wave.slug(), &ctx.team_id)?;
    let items = ctx
        .client
        .list_items(&project.id)
        .await
        .map_err(linear_error)?;
    Ok(Some(ProjectInventory { project, items }))
}

fn validate_project_ownership(
    project: &TrackerProject,
    wave: &str,
    team_id: &str,
) -> OpsResult<()> {
    let expected = match project.team_ids.as_slice() {
        [team] if team == team_id => return Ok(()),
        [team] => format!("Team {team}; expected repository Team {team_id}"),
        teams => format!(
            "{} Teams [{}]; expected exactly one repository Team",
            teams.len(),
            teams.join(", ")
        ),
    };
    Err(OpsError::Message(format!(
        "Linear Project `{}` ({}) in wave/{wave} belongs to {expected}. Repair the associations and run `lf wave status {wave} --sync`.",
        project.name, project.id
    )))
}

fn title_case(slug: &str) -> String {
    slug.split(['-', '_', '/'])
        .filter(|part| !part.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn block_on_linear<T>(future: impl Future<Output = OpsResult<T>>) -> OpsResult<T> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to create async runtime: {err}")))?;
    rt.block_on(future)
}

fn linear_error(err: LinearError) -> OpsError {
    match err {
        LinearError::RateLimited { .. } => OpsError::Tracker(err.into()),
        LinearError::Message(text) => OpsError::Message(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::test_server::{self, json_response, QueuedResponse};
    use crate::ops::NullProgress;
    use crate::test_linear_input::{InventoryFixture, SeedLinear};
    use axum::http::StatusCode;
    use serde_json::{json, Value};

    fn linear_test_ctx(base_url: String) -> LinearRepository {
        LinearRepository {
            client: crate::linear::LinearClient::with_base_url(
                "linear-secret".to_string(),
                Some("team-123".to_string()),
                base_url,
            ),
            repo_id: RepoId::parse("loopflowstudio/loopflow").unwrap(),
            team_id: "team-123".to_string(),
        }
    }

    async fn isolated_pm_store(repo: &Path) -> Store {
        crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            repo.join("registry.db"),
        ))
        .await
        .expect("open isolated planning store")
    }

    fn write_goal(repo: &Path, wave: &str, frontmatter: &str) {
        let dir = repo.join(".lf/wave").join(wave);
        std::fs::create_dir_all(&dir).expect("create wave dir");
        std::fs::write(
            dir.join("GOAL.md"),
            format!("---\n{frontmatter}---\nDrive the work.\n"),
        )
        .expect("write GOAL.md");
    }

    fn projects_response(projects: serde_json::Value) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "projects": {
                "nodes": projects,
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } }),
        )
    }

    /// Wave `scan`, its Project linked to Linear Project `project-123`.
    async fn scan_linked_to(store: &Store, repo: &Path) -> Wave {
        write_goal(repo, "scan", "agent: codex\n");
        let wave = crate::work::wave::ensure_wave_row(store, repo, "scan")
            .await
            .unwrap();
        link_wave(store, repo, "scan", &project_node("project-123", "Scan")).await;
        wave
    }

    /// Link `wave`'s Project to the Linear Project `node` describes.
    async fn link_wave(store: &Store, repo: &Path, wave: &str, node: &Value) {
        let locator = crate::work::wave::discover(repo, wave).unwrap();
        let wave = store.get_wave_at(&locator).await.unwrap().unwrap();
        let project: TrackerProject = serde_json::from_value(json!({
            "id": node["id"], "revision": null,
            "slug": crate::linear::project_slug(node["name"].as_str().unwrap()),
            "name": node["name"], "summary": "", "status": "started",
            "team_ids": node["teams"]["nodes"].as_array().unwrap().iter()
                .map(|team| team["id"].clone()).collect::<Vec<_>>(),
        }))
        .unwrap();
        loopflow_storage::sqlite::linear_events::link_wave_project_in(
            &store.sqlite,
            wave.id(),
            &project,
            1,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
        .unwrap();
    }

    fn project_node(id: &str, name: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "description": "",
            "content": "workflow: feature\n\n## Definition\n\nA measured bet.\n\n## KRs\n",
            "status": {"type":"started"},
            "teams": { "nodes": [{ "id": "team-123" }] }
        })
    }

    fn issues_response(items: serde_json::Value) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "project": { "issues": {
                "nodes": items,
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } } }),
        )
    }

    fn migration_project_node(id: &str, name: &str, teams: &[&str]) -> Value {
        json!({
            "id": id,
            "name": name,
            "description": "A measured bet.",
            "status": {"type":"started"},
            "content": "## Definition\n\nA measured bet.\n\n## Graphs\n\n- first: (none)\n- loop: (none)\n- finally: (none)\n\n## KRs\n\n- [ ] Ownership holds",
            "teams": { "nodes": teams.iter().map(|id| json!({ "id": id })).collect::<Vec<_>>() }
        })
    }

    fn migration_issue_node(
        id: &str,
        identifier: &str,
        project_id: &str,
        project_name: &str,
        team_id: &str,
        completed: bool,
    ) -> Value {
        json!({
            "id": id,
            "identifier": identifier,
            "url": null,
            "title": format!("Task {identifier}"),
            "description": "",
            "completedAt": null, "dueDate": null, "parent":null,"prioritySortOrder": 0.0,
            "sortOrder": 0.0, "updatedAt": time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
            "assignee": null,
            "state": { "type": if completed { "completed" } else { "unstarted" } },
            "project": { "id": project_id, "name": project_name },
            "team": { "id": team_id }
        })
    }

    fn issue_comments_response() -> QueuedResponse {
        issue_comments_response_with(None)
    }

    fn issue_comments_response_with(body: Option<&str>) -> QueuedResponse {
        let nodes = body
            .map(|body| vec![json!({ "id": "comment-reteam", "body": body, "user": null })])
            .unwrap_or_default();
        json_response(
            StatusCode::OK,
            json!({ "data": { "issue": {
                "updatedAt": "2026-07-20T00:00:00.000Z",
                "title": "Task", "description": "",
                "comments": {
                    "nodes": nodes,
                    "pageInfo": { "hasNextPage": false, "endCursor": null }
                }
            } } }),
        )
    }

    fn project_update_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "projectUpdate": { "project": { "id": id } } } }),
        )
    }

    fn with_pm_home(test: impl FnOnce(&tempfile::TempDir, &tokio::runtime::Runtime)) {
        let _lock = crate::journal::test_env_lock();
        let _restore = super::test_fixture::PlanningEnvironment::isolate();
        let repo = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", repo.path());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        test(&repo, &runtime);
    }

    fn write_repo_config(repo: &Path, content: &str) {
        std::fs::create_dir_all(repo.join(".lf")).unwrap();
        std::fs::write(repo.join(".lf/config.yaml"), content).unwrap();
    }

    #[test]
    fn connect_preserves_personal_transport_and_repository_binding() {
        with_pm_home(|home, runtime| {
            std::fs::write(
                home.path().join("config.yaml"),
                "planning:\n  provider: git\n  remote: personal\n  shared: private\n",
            )
            .unwrap();
            runtime.block_on(async {
                let fixture = test_fixture::Fixture::new().await;
                let (repo, wave) = fixture.planning_repo().await;
                fixture.seed(test_fixture::now() + 3600).await;
                let team = json!({"data": {"teams": {"nodes": [{
                    "id": "team-1", "name": "Retained work", "key": "OLD",
                    "description": "<!-- loopflow-repository: loopflowstudio/fixture -->"
                }, {
                    "id": "other-team", "name": "Fixture", "key": "FIX",
                    "description": "<!-- loopflow-repository: loopflowstudio/fixture -->"
                }]}}});
                let mut linked = project_node("project-1", "Billing");
                linked["teams"] = json!({ "nodes": [{ "id": "team-1" }] });
                let (url, _) = test_server::spawn(vec![
                    json_response(StatusCode::OK, team.clone()),
                    json_response(StatusCode::OK, team.clone()),
                    json_response(StatusCode::OK, team.clone()),
                    projects_response(json!([linked])),
                    project_readback(&linked),
                    issues_response(json!([])),
                    // This process already validated the claim; the second
                    // connect skips that ListTeams.
                    json_response(StatusCode::OK, team.clone()),
                    json_response(StatusCode::OK, team),
                    projects_response(json!([linked])),
                    project_readback(&linked),
                    issues_response(json!([])),
                ])
                .await;
                LINEAR_TEST_CONTEXT
                    .scope(fixture.context(&url), async {
                        let options = LinearInitOptions {
                            wave: Some("product".into()),
                            project: Some("Billing".into()),
                            ..Default::default()
                        };
                        // Connecting again to the Project it links changes nothing.
                        for _ in 0..2 {
                            let result =
                                connect_async(&repo, &options, &NullProgress).await.unwrap();
                            assert_eq!(result.team_id, "team-1");
                            assert_eq!(result.project_id.as_deref(), Some("project-1"));
                            assert_eq!(result.project_name, "Billing");
                            assert!(!result.created && !result.team_created);
                        }
                        let config = crate::config::load_config(Some(&repo)).unwrap().unwrap();
                        assert_eq!(config.planning_transport(), None);
                        assert_eq!(
                            selected_linear_team(&repo).unwrap().as_deref(),
                            Some("team-1")
                        );
                        assert_eq!(
                            fixture.store.list_waves(None).await.unwrap()[0].id(),
                            wave.id()
                        );
                        assert!(fixture
                            .store
                            .list_tasks(Some(wave.id()))
                            .await
                            .unwrap()
                            .is_empty());
                    })
                    .await;
            });
        });
    }

    /// The ops Wave's Project, once `connect` has run against `state`.
    fn ops_project(fixture: &test_fixture::Fixture) -> loopflow_storage::work::project::Project {
        let ops = fixture
            .store
            .sqlite
            .list_waves(None)
            .unwrap()
            .into_iter()
            .find(|wave| wave.slug() == "ops")
            .expect("ops is registered");
        fixture
            .store
            .sqlite
            .project_of_wave(ops.id())
            .unwrap()
            .unwrap()
    }

    #[test]
    fn connect_adds_an_unregistered_wave_on_main_and_makes_its_project() {
        with_pm_home(|home, runtime| {
            std::fs::write(
                home.path().join("config.yaml"),
                "planning:\n  provider: linear\n",
            )
            .unwrap();
            runtime.block_on(async {
                let fixture = test_fixture::Fixture::new().await;
                let (main, _) = fixture.planning_repo().await;
                fixture.seed(test_fixture::now() + 3600).await;
                let git = |args: &[&str], cwd: &Path| {
                    let output = std::process::Command::new("git")
                        .args(args)
                        .current_dir(cwd)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    String::from_utf8(output.stdout).unwrap()
                };
                test_fixture::commit_planning_repo(&main);
                let worktree = main.parent().unwrap().join("task");
                git(
                    &[
                        "worktree",
                        "add",
                        "-q",
                        worktree.to_str().unwrap(),
                        "-b",
                        "task",
                    ],
                    &main,
                );
                let state = Arc::new(tokio::sync::Mutex::new(
                    crate::test_support::planning_tasks::PlanningState {
                        export_mode: true,
                        ..Default::default()
                    },
                ));
                let (url, server) = crate::test_support::planning_tasks::serve(state.clone()).await;
                LINEAR_TEST_CONTEXT
                    .scope(fixture.context(&url), async {
                        let result = connect_async(
                            &worktree,
                            &LinearInitOptions {
                                wave: Some("ops".into()),
                                ..Default::default()
                            },
                            &NullProgress,
                        )
                        .await
                        .unwrap();
                        let project = ops_project(&fixture);
                        // Loopflow made it, under the Wave Project's own id.
                        assert!(result.created);
                        assert_eq!(
                            result.project_id.as_deref(),
                            project.tracker_id.as_ref().map(|id| id.as_str())
                        );
                        assert_eq!(state.lock().await.creation_writes, 1);
                        // The repository already names its Team; nothing to commit.
                        assert!(
                            matches!(
                                result.files,
                                super::super::wave_lifecycle::AuthoredFiles::Unchanged
                            ),
                            "{:?}",
                            result.files
                        );
                        // The Wave lives on the main checkout, committed, never on the Task branch.
                        assert!(!worktree.join(".lf/wave/ops").exists());
                        assert!(git(&["ls-files", ".lf/wave/ops"], &main).contains("GOAL.md"));
                        assert_eq!(git(&["status", "--porcelain"], &main), "");
                        let log = git(&["log", "--format=%s", "-3"], &main);
                        assert!(log.contains("lf wave add: ops"), "{log}");
                    })
                    .await;
                server.abort();
            });
        });
    }

    #[test]
    fn connect_links_an_existing_project_in_place_of_the_one_it_made() {
        with_pm_home(|home, runtime| {
            std::fs::write(
                home.path().join("config.yaml"),
                "planning:\n  provider: linear\n",
            )
            .unwrap();
            runtime.block_on(async {
                let fixture = test_fixture::Fixture::new().await;
                let (main, _) = fixture.planning_repo().await;
                fixture.seed(test_fixture::now() + 3600).await;
                let existing = json!({"id":"5d0c1b2a-3948-4e5f-9a6b-7c8d9e0f1a2b",
                    "name":"Refunds", "description":"", "content":"",
                    "status":{"type":"started"}, "updatedAt":"2026-09-30T12:00:00Z",
                    "archivedAt":null, "teams":{"nodes":[{"id":"team-1"}]}});
                let state = Arc::new(tokio::sync::Mutex::new(
                    crate::test_support::planning_tasks::PlanningState {
                        extra_projects: vec![existing.clone()],
                        export_mode: true,
                        ..Default::default()
                    },
                ));
                let (url, server) = crate::test_support::planning_tasks::serve(state.clone()).await;
                LINEAR_TEST_CONTEXT
                    .scope(fixture.context(&url), async {
                        let made = connect_async(
                            &main,
                            &LinearInitOptions {
                                wave: Some("ops".into()),
                                ..Default::default()
                            },
                            &NullProgress,
                        )
                        .await
                        .unwrap();
                        assert!(made.created);
                        let result = connect_async(
                            &main,
                            &LinearInitOptions {
                                wave: Some("ops".into()),
                                project: Some("refunds".into()),
                                ..Default::default()
                            },
                            &NullProgress,
                        )
                        .await
                        .unwrap();
                        assert!(!result.created);
                        assert_eq!(result.project_id.as_deref(), existing["id"].as_str());
                        assert_eq!(result.project_name, "Refunds");
                        let project = ops_project(&fixture);
                        assert_eq!(
                            project.tracker_id.as_ref().map(|id| id.as_str()),
                            existing["id"].as_str()
                        );
                        assert_eq!(project.name, "Refunds");
                        // A Task made in the Wave now goes to the linked Project.
                        let context = fixture.context(&url);
                        let task = tokio::task::block_in_place(|| {
                            LINEAR_TEST_CONTEXT.sync_scope(context, || {
                                crate::ops::task::task_create(
                                    &main,
                                    Some("ops"),
                                    Some("Refund the duplicate charge".into()),
                                    None,
                                    crate::ops::task::TaskAdoptionOptions::default(),
                                    None,
                                    None,
                                )
                            })
                        })
                        .unwrap();
                        let provider = state.lock().await;
                        let issue = provider
                            .issues
                            .iter()
                            .find(|issue| issue["title"] == "Refund the duplicate charge")
                            .expect("the Task reached Linear");
                        assert_eq!(issue["project"]["id"], existing["id"]);
                        assert!(task.tracker_id.is_some());
                    })
                    .await;
                server.abort();
            });
        });
    }

    #[test]
    fn connection_selects_linear_and_preserves_unrelated_authored_content() {
        with_pm_home(|_, _| {
            let repo = tempfile::tempdir().unwrap();
            write_repo_config(
                repo.path(),
                "agent: codex\nplanning:\n  provider: git\n  shared: colleagues\n",
            );
            write_goal(repo.path(), "product", "agent: claude\n");
            let goal = repo.path().join(".lf/wave/product/GOAL.md");

            write_repository_linear_config(repo.path(), "team-product").unwrap();
            let config = crate::config::load_repo_config(repo.path())
                .unwrap()
                .unwrap();
            assert_eq!(
                config.planning_transport(),
                Some(crate::config::TrackerConfig::Linear {
                    team: Some("team-product".into())
                })
            );
            assert_eq!(config.agent.as_deref(), Some("codex"));
            let updated = std::fs::read_to_string(goal).unwrap();
            assert!(updated.ends_with("---\nDrive the work.\n"));
            let wave = crate::config::load_wave_config(Some(repo.path()), Some("product"))
                .unwrap()
                .unwrap();
            assert_eq!(wave.agent.as_deref(), Some("claude"));
        });
    }

    #[test]
    fn repository_team_config_is_the_only_normal_authority() {
        with_pm_home(|repo, runtime| {
            write_repo_config(
                repo.path(),
                "planning:\n  provider: linear\n  team: team-loo\n",
            );
            write_goal(repo.path(), "product", "agent: codex\n");

            runtime.block_on(async {
                let store = crate::store::open_ephemeral_store(
                    &crate::store::StorageConfig::sqlite(repo.path().join("loopflow.db")),
                )
                .await
                .unwrap();
                crate::work::wave::ensure_wave_row(&store, repo.path(), "product")
                    .await
                    .unwrap();
            });
            assert_eq!(
                selected_linear_team(repo.path()).unwrap().as_deref(),
                Some("team-loo")
            );
        });
    }

    async fn run_reteam_fixture(
        resolved: &ResolvedReteamContext,
        repo: &Path,
        database: &Path,
    ) -> OpsResult<LinearReteamResult> {
        LINEAR_TEST_CONTEXT
            .scope(
                LinearTestContext {
                    path: database.to_path_buf(),
                    store: Arc::new(Store::from_sqlite_for_test(resolved.store.sqlite.clone())),
                    graphql_url: String::new(),
                },
                apply_or_plan_repository_reteam(resolved, repo, true, &NullProgress),
            )
            .await
    }

    fn project_readback(project: &Value) -> QueuedResponse {
        json_response(StatusCode::OK, json!({"data": {"project": project}}))
    }

    fn issue_readback(mut issue: Value, project: &Value) -> QueuedResponse {
        issue["project"] = project.clone();
        json_response(StatusCode::OK, json!({"data": {"issue": issue}}))
    }

    #[test]
    fn repository_team_reteam_moves_open_and_completed_issues() {
        with_pm_home(|_home, runtime| {
            let repo = loopflow_test_support::TestRepo::new();
            runtime.block_on(async {
                write_repo_config(
                    repo.path(),
                    "planning:\n  provider: linear\n  team: team-loo\n",
                );
                write_goal(
                    repo.path(),
                    "survival",
                    "agent: codex\n",
                );
                write_goal(
                    repo.path(),
                    "survival/infrastructure",
                    "agent: codex\n",
                );

                repo.stage_all();
                repo.commit("Bind fixture repository and Waves");
                let retained_head = repo.head_sha();
                repo.create_file("unrelated.txt", "Keep this local edit.\n");
                let database = repo.path().join("loopflow.db");
                let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                    database.clone(),
                ))
                .await
                .unwrap();
                crate::work::wave::ensure_wave_row(&store, repo.path(), "survival/infrastructure")
                    .await
                    .unwrap();
                // Queued responses expect survival first; discovery is newest-first.
                rusqlite::Connection::open(&database)
                    .unwrap()
                    .execute(
                        "UPDATE waves SET created_at = CASE name WHEN 'survival' THEN 2 ELSE 1 END",
                        [],
                    )
                    .unwrap();

                let old_survival = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    &["team-old"],);
                let old_infrastructure = migration_project_node(
                    "project-infrastructure",
                    "Infrastructure — Gmail",
                    &["team-old"],);
                let new_survival = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    &["team-loo"],);
                let new_infrastructure = migration_project_node(
                    "project-infrastructure",
                    "Infrastructure — Gmail",
                    &["team-loo"],);
                let mut expanded_survival = old_survival.clone();
                expanded_survival["teams"] = json!({"nodes": [{"id":"team-old"},{"id":"team-loo"}]});
                let mut expanded_infrastructure = old_infrastructure.clone();
                expanded_infrastructure["teams"] = expanded_survival["teams"].clone();
                let responses = vec![
                    project_readback(&old_survival),
                    issues_response(json!([migration_issue_node(
                        "issue-open",
                        "OLD-1",
                        "project-survival",
                        "Survival — A real task reaches done",
                        "team-old",
                        false,
                    )])),
                    project_readback(&old_infrastructure),
                    issues_response(json!([migration_issue_node(
                        "issue-done",
                        "OLD-2",
                        "project-infrastructure",
                        "Infrastructure — Gmail",
                        "team-old",
                        true,
                    )])),
                    project_update_response("project-survival"),
                    project_readback(&expanded_survival),
                    project_update_response("project-infrastructure"),
                    project_readback(&expanded_infrastructure),
                    issue_comments_response(),
                    json_response(
                        StatusCode::OK,
                        json!({ "data": { "commentCreate": { "comment": { "id": "comment-open" } } } }),
                    ),
                    json_response(
                        StatusCode::OK,
                        json!({ "data": { "issueUpdate": { "issue": { "id": "issue-open", "identifier": "LOO-1" } } } }),
                    ),
                    issue_readback(
                        migration_issue_node(
                            "issue-open",
                            "LOO-1",
                            "project-survival",
                            "Survival — A real task reaches done",
                            "team-loo",
                            false,
                        ),
                        &expanded_survival,
                    ),
                    issue_comments_response(),
                    json_response(
                        StatusCode::OK,
                        json!({ "data": { "commentCreate": { "comment": { "id": "comment-done" } } } }),
                    ),
                    json_response(
                        StatusCode::OK,
                        json!({ "data": { "issueUpdate": { "issue": { "id": "issue-done", "identifier": "LOO-2" } } } }),
                    ),
                    issue_readback(
                        migration_issue_node(
                            "issue-done",
                            "LOO-2",
                            "project-infrastructure",
                            "Infrastructure — Gmail",
                            "team-loo",
                            true,
                        ),
                        &expanded_infrastructure,
                    ),
                    project_update_response("project-survival"),
                    project_readback(&new_survival),
                    project_update_response("project-infrastructure"),
                    project_readback(&new_infrastructure),
                    project_readback(&new_survival),
                    issues_response(json!([migration_issue_node(
                        "issue-open",
                        "LOO-1",
                        "project-survival",
                        "Survival — A real task reaches done",
                        "team-loo",
                        false,
                    )])),
                    project_readback(&new_infrastructure),
                    issues_response(json!([migration_issue_node(
                        "issue-done",
                        "LOO-2",
                        "project-infrastructure",
                        "Infrastructure — Gmail",
                        "team-loo",
                        true,
                    )])),
                ];
                link_wave(&store, repo.path(), "survival", &old_survival).await;
                link_wave(&store, repo.path(), "survival/infrastructure", &old_infrastructure)
                    .await;
                let (base_url, requests) = test_server::spawn(responses).await;
                let client = crate::linear::LinearClient::with_base_url(
                    "linear-secret".to_string(),
                    Some("team-loo".to_string()),
                    base_url,
                );
                let resolved = ResolvedReteamContext {
                    repository: LinearRepository {
                        client,
                        repo_id: RepoId::parse("loopflowstudio/fixture").unwrap(),
                        team_id: "team-loo".to_string(),
                    },
                    team_key: "LOO".to_string(),
                    store,
                };

                let result = run_reteam_fixture(&resolved, repo.path(), &database)
                    .await
                    .unwrap();

                assert!(result.applied);
                assert_eq!(repo.head_sha(), retained_head);
                assert_eq!(std::fs::read_to_string(repo.path().join("unrelated.txt")).unwrap(), "Keep this local edit.\n");
                assert_eq!(result.moves.len(), 2);
                let identifiers = result
                    .moves
                    .iter()
                    .filter_map(|item| item.new_identifier.as_deref())
                    .collect::<BTreeSet<_>>();
                assert_eq!(identifiers, BTreeSet::from(["LOO-1", "LOO-2"]));

                assert_eq!(
                    selected_linear_team(repo.path()).unwrap().as_deref(),
                    Some("team-loo")
                );
                for wave in ["survival", "survival/infrastructure"] {
                    let locator = crate::work::wave::discover(repo.path(), wave).unwrap();
                    let registered = resolved.store.get_wave_at(&locator).await.unwrap().unwrap();
                    let project = resolved.store.project_of_wave(registered.id()).await.unwrap().unwrap();
                    let teams: String = rusqlite::Connection::open(&database).unwrap().query_row(
                        "SELECT team_ids FROM tracker_projects WHERE id=?1",
                        [project.tracker_id.as_ref().unwrap().as_str()], |r| r.get(0)
                    ).unwrap();
                    assert_eq!(serde_json::from_str::<Vec<String>>(&teams).unwrap(), ["team-loo"]);
                }

                let requests = requests.lock().await;
                let first_move = requests
                    .iter()
                    .position(|request| request.body.contains("MoveIssueToTeam"))
                    .unwrap();
                let attached_before_move = requests[..first_move]
                    .iter()
                    .filter(|request| request.body.contains("SetProjectTeams"))
                    .count();
                assert_eq!(attached_before_move, 2);
                assert_eq!(
                    result
                        .project_moves
                        .iter()
                        .map(|project| project.name.as_str())
                        .collect::<Vec<_>>(),
                    [
                        "Survival — A real task reaches done",
                        "Infrastructure — Gmail"
                    ]
                );
                assert!(!requests
                    .iter()
                    .any(|request| request.body.contains("UpdateProject(")));
            });
        });
    }

    #[test]
    fn repository_team_reteam_resumes_after_an_interrupted_issue_move() {
        interrupted_reteam(false, "move");
    }

    #[test]
    fn repository_team_reteam_resumes_with_cached_project_membership() {
        interrupted_reteam(true, "move");
    }

    #[test]
    fn repository_team_reteam_recovers_expansion_response_loss() {
        interrupted_reteam(true, "expansion");
    }

    #[test]
    fn repository_team_reteam_recovers_narrowing_response_loss() {
        interrupted_reteam(true, "narrowing");
    }

    fn interrupted_reteam(cache_planning: bool, interruption: &str) {
        with_pm_home(|repo, runtime| {
            runtime.block_on(async {
                write_repo_config(
                    repo.path(),
                    "planning:\n  provider: linear\n  team: team-loo\n",
                );
                write_goal(
                    repo.path(),
                    "survival",
                    "agent: codex\n",
                );

                let database = repo.path().join("loopflow.db");
                let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                    database.clone(),
                ))
                .await
                .unwrap();
                let wave = crate::work::wave::ensure_wave_row(&store, repo.path(), "survival")
                    .await.unwrap();

                let old_project = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    &["team-old"],);
                let expanded_project = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    &["team-old", "team-loo"],);
                let migrated_project = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    &["team-loo"],);
                let old_issue = migration_issue_node(
                    "issue-open",
                    "OLD-1",
                    "project-survival",
                    "Survival — A real task reaches done",
                    "team-old",
                    false,
                );
                let migrated_issue = migration_issue_node(
                    "issue-open",
                    "LOO-1",
                    "project-survival",
                    "Survival — A real task reaches done",
                    "team-loo",
                    false,
                );
                let marker = reteam_comment_body("OLD-1", "LOO");
                let mut responses = Vec::new();
                if cache_planning {
                    responses.push(project_readback(&old_project));
                    responses.push(issues_response(json!([old_issue.clone()])));
                }
                let failure = || {
                    json_response(
                        StatusCode::OK,
                        json!({"errors":[{"message":format!("{interruption} interrupted")}]}),
                    )
                };
                let move_success = || {
                    json_response(
                        StatusCode::OK,
                        json!({"data":{"issueUpdate":{"issue":{"id":"issue-open","identifier":"LOO-1"}}}}),
                    )
                };
                let comment_success = || {
                    json_response(
                        StatusCode::OK,
                        json!({"data":{"commentCreate":{"comment":{"id":"comment-reteam"}}}}),
                    )
                };
                responses.extend([
                    project_readback(&old_project),
                    issues_response(json!([old_issue.clone()])),
                ]);
                if interruption == "expansion" {
                    responses.extend([
                        failure(),
                        project_readback(&expanded_project),
                        issues_response(json!([old_issue.clone()])),
                        project_readback(&expanded_project),
                        issue_comments_response(),
                        comment_success(),
                    ]);
                } else {
                    responses.extend([
                        project_update_response("project-survival"),
                        project_readback(&expanded_project),
                        issue_comments_response(),
                        comment_success(),
                    ]);
                    if interruption == "move" {
                        responses.extend([
                            failure(),
                            project_readback(&expanded_project),
                            issues_response(json!([old_issue.clone()])),
                            project_readback(&expanded_project),
                            issue_comments_response_with(Some(&marker)),
                        ]);
                    }
                }
                responses.extend([
                    move_success(),
                    issue_readback(migrated_issue.clone(), &expanded_project),
                ]);
                if interruption == "narrowing" {
                    responses.extend([
                        failure(),
                        project_readback(&migrated_project),
                        issues_response(json!([migrated_issue.clone()])),
                        project_readback(&migrated_project),
                    ]);
                } else {
                    responses.push(project_update_response("project-survival"));
                }
                responses.extend([
                    project_readback(&migrated_project),
                    project_readback(&migrated_project),
                    issues_response(json!([migrated_issue.clone()])),
                ]);
                link_wave(&store, repo.path(), "survival", &old_project).await;
                let (base_url, requests) = test_server::spawn(responses).await;
                let resolved = ResolvedReteamContext {
                    repository: LinearRepository {
                        client: crate::linear::LinearClient::with_base_url(
                            "linear-secret".to_string(),
                            Some("team-loo".to_string()),
                            base_url,
                        ),
                        repo_id: RepoId::parse("loopflowstudio/fixture").unwrap(),
                        team_id: "team-loo".to_string(),
                    },
                    team_key: "LOO".to_string(),
                    store,
                };

                if cache_planning {
                    let project = resolved
                        .repository
                        .client
                        .project_ownership("project-survival")
                        .await
                        .unwrap();
                    let items = resolved
                        .repository
                        .client
                        .list_items("project-survival")
                        .await
                        .unwrap();
                    resolved
                        .store
                        .seed_inventory(
                            InventoryFixture {
                                wave_id: wave.id().clone(),
                                synced_at: 1,
                                snapshot: ProjectInventory { project, items },
                            },
                            None,
                        )
                        .await
                        .unwrap();
                }

                let first = run_reteam_fixture(&resolved, repo.path(), &database)
                    .await
                    .unwrap_err();
                assert!(
                    first
                        .to_string()
                        .contains(&format!("{interruption} interrupted")),
                    "{first}"
                );


                let resumed = run_reteam_fixture(&resolved, repo.path(), &database)
                    .await
                    .unwrap();
                if interruption == "narrowing" {
                    assert!(resumed.moves.is_empty());
                } else {
                    assert_eq!(resumed.moves[0].new_identifier.as_deref(), Some("LOO-1"));
                }
                let project = resolved.store.project_of_wave(wave.id()).await.unwrap().unwrap();
                let teams: String = rusqlite::Connection::open(&database).unwrap().query_row(
                        "SELECT team_ids FROM tracker_projects WHERE id=?1",
                        [project.tracker_id.as_ref().unwrap().as_str()], |r| r.get(0)
                    ).unwrap();
                    assert_eq!(serde_json::from_str::<Vec<String>>(&teams).unwrap(), ["team-loo"]);
                let tasks = resolved.store.list_tasks(Some(wave.id())).await.unwrap();
                assert_eq!(tasks[0].identifier, "LOO-1");

                let requests = requests.lock().await;
                assert_eq!(
                    requests
                        .iter()
                        .filter(|request| request.body.contains("commentCreate"))
                        .count(),
                    1,
                    "the resumed migration reuses its first traceability comment"
                );
            });
        });
    }

    #[tokio::test]
    async fn refresh_reads_the_linked_project_and_its_issues() {
        let (base_url, requests) = test_server::spawn(vec![
            project_readback(&project_node("project-123", "Scan")),
            issues_response(json!([
                { "id": "issue-1", "identifier": "LOO-1", "url": null,
                  "title": "First", "description": "one",
                  "completedAt": null, "dueDate": null, "parent":null,"prioritySortOrder": 0.0, "sortOrder": 0.0, "updatedAt":"2026-09-29T12:00:00.123Z",
                  "assignee": null, "state": { "type": "unstarted" },
                  "project": { "id": "project-123", "name": "Scan" },
                  "team": { "id": "team-123" } }
            ])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url);
        let repo = tempfile::tempdir().unwrap();
        let store = isolated_pm_store(repo.path()).await;
        let wave = scan_linked_to(&store, repo.path()).await;

        let ProjectInventory { project, items } = read_wave_inventory(&ctx, &wave, &store)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(project.id, "project-123");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "First");
        assert_eq!(
            requests.lock().await[1].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
    }

    #[tokio::test]
    async fn refresh_preserves_moved_and_detached_issue_ownership() {
        let (base_url, _) = test_server::spawn(vec![
            project_readback(&project_node("project-123", "Scan")),
            issues_response(json!([
                { "id": "moved", "identifier": "LOO-1", "url": null,
                  "title": "Moved", "description": "Preserve the observed destination",
                  "completedAt": null, "dueDate": null, "parent":null,"prioritySortOrder": 0.0, "sortOrder": 0.0,
                  "updatedAt": "2026-10-05T12:00:00Z",
                  "assignee": null, "state": { "type": "started" },
                  "project": { "id": "successor", "name": "Next work" },
                  "team": { "id": "team-123" } },
                { "id": "detached", "identifier": "LOO-2", "url": null,
                  "title": "Detached", "description": "Do not invent membership",
                  "completedAt": null, "dueDate": null, "parent":null,"prioritySortOrder": 1.0, "sortOrder": 1.0,
                  "updatedAt": "2026-10-05T12:00:01Z",
                  "assignee": null, "state": { "type": "started" },
                  "project": null, "team": { "id": "team-123" } }
            ])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url);
        let repo = tempfile::tempdir().unwrap();
        let store = isolated_pm_store(repo.path()).await;
        let wave = scan_linked_to(&store, repo.path()).await;

        let ProjectInventory { items, .. } = read_wave_inventory(&ctx, &wave, &store)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(items[0].project_id.as_deref(), Some("successor"));
        assert_eq!(items[0].revision.as_deref(), Some("2026-10-05T12:00:00Z"));
        assert_eq!(items[1].project_id, None);
        assert_eq!(items[1].revision.as_deref(), Some("2026-10-05T12:00:01Z"));
    }
}
