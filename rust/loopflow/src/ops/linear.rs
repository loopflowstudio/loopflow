//! Provider planning operations shared by Task, Wave, and repository commands.
//!
//! Work owns authored Project content and Tasks. Linear refresh accepts provider
//! facts through the shared writer; ordinary readers consume saved Work.

use super::planning_sync::{lock_wave_planning, planning_store};
use crate::linear::LinearIssue;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::linear::LinearClient;
use crate::linear::LinearError;
use crate::linear::LinearProject;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::normalize_wave_name;
use crate::provider_auth::{
    provider_token_refresh_due, refresh_stored_provider_token, Provider, TokenRefreshError,
};
use crate::repository::RepoId;
use crate::store::{
    open_existing_store, LinearIssueEvidence, PlanningState, ProviderToken,
    ProviderTokenReplacement, StorageConfig, Store,
};
use crate::work::task::Task;
use crate::work::wave::config::{read_wave_config, update_wave_config};
use crate::work::wave::Wave;

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct LinearInitOptions {
    pub wave: Option<String>,
    /// Repository Team key (Task prefix, e.g. `LOO`). Defaults from the repository name.
    pub team_key: Option<String>,
    /// Repository Team display name. Defaults to the repository name.
    pub team_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearInitResult {
    pub wave: String,
    pub initiative_id: String,
    pub created: bool,
    /// Stable id of the repository Team (owns the Task prefix).
    pub team_id: String,
    /// The repository Team's current display key (Task prefix).
    pub team_key: String,
    /// Whether this run created the team (vs. adopted an existing one).
    pub team_created: bool,
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
    pub plan: bool,
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearReteamProjectMove {
    pub wave: String,
    pub id: String,
    pub name: String,
    pub from_teams: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearReteamResult {
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

#[derive(Debug, Clone)]
pub struct LinearRenameOptions {
    pub wave: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinearRenameResult {
    pub wave: String,
    pub initiative: String,
    pub title: String,
}

// ── Client + Linear project resolution ──────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct LinearRepository {
    pub client: LinearClient,
    pub repo_id: RepoId,
    pub team_id: String,
}

pub(crate) fn resolve_wave(wave: Option<&str>) -> OpsResult<String> {
    wave.and_then(normalize_wave_name)
        .ok_or_else(|| OpsError::Message("cannot determine wave; pass --wave <name>".to_string()))
}

pub(super) fn read_initiative(repo: &Path, wave: &str) -> Option<String> {
    read_wave_config(repo, wave)?
        .linear?
        .initiative
        .filter(|initiative| !initiative.trim().is_empty())
}

pub(crate) fn selected_linear_team(repo: &Path) -> OpsResult<Option<String>> {
    let transport = crate::config::load_planning_transport(repo).map_err(|error| {
        OpsError::Message(format!("failed to read planning configuration: {error}"))
    })?;
    Ok(match transport {
        crate::config::PlanningConfig::Linear { team } => {
            team.filter(|team| !team.trim().is_empty())
        }
        _ => None,
    })
}

pub(crate) fn repository_team_id(repo: &Path) -> OpsResult<String> {
    selected_linear_team(repo)?.ok_or_else(|| {
        OpsError::Message(
            ".lf/config.yaml has no repository `planning.team`. \
             Run `lf wave connect <wave> --team-key <KEY>` before creating or mutating work."
                .to_string(),
        )
    })
}

fn wave_has_linear_initiative(repo: &Path, wave: &str) -> bool {
    read_initiative(repo, wave).is_some()
}

async fn build_client(team: Option<String>) -> OpsResult<LinearClient> {
    let token = resolve_planning_token(Provider::Linear).await?;
    #[cfg(test)]
    if let Ok(url) = LINEAR_TEST_CONTEXT.try_with(|ctx| ctx.graphql_url.clone()) {
        return Ok(LinearClient::with_base_url(token, team, url));
    }
    Ok(LinearClient::new(token, team))
}

fn repository_id(repo: &Path) -> OpsResult<RepoId> {
    RepoId::discover(repo).map_err(|error| {
        OpsError::Message(format!(
            "cannot establish repository Linear identity from Git origin: {error}. \
             Configure an origin before running `lf wave connect`."
        ))
    })
}

async fn resolve_repository_context(repo: &Path) -> OpsResult<LinearRepository> {
    let team_id = repository_team_id(repo)?;
    let repo_id = repository_id(repo)?;
    let client = build_client(Some(team_id.clone()))
        .await?
        .for_repository(repo);
    client
        .validate_team_claim(&team_id, repo_id.as_str())
        .await
        .map_err(linear_error)?;
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

pub(crate) async fn resolve_context(
    repo: &Path,
    wave: &str,
) -> OpsResult<(LinearRepository, String)> {
    let repository = resolve_repository_context(repo).await?;
    let initiative = read_initiative(repo, wave).ok_or_else(|| {
        OpsError::Message(format!(
            ".lf/wave/{wave}/GOAL.md has no `linear.initiative`. \
             Run `lf wave connect {wave}` to connect its Linear Initiative."
        ))
    })?;
    Ok((repository, initiative))
}

/// Planning providers authenticate via OAuth: the access token and refresh grant
/// live in store, and access refreshes the grant before the access token expires.
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
        "No {} credential found. Run `doppler run -- lf account connect {provider}`.",
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
                    OpsError::Message(format!("{} refresh failed: {reason}. Run `doppler run -- lf account connect {provider}` to reconnect.", provider.display_name()))
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

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct LinearTestContext {
    pub(crate) path: std::path::PathBuf,
    pub(crate) store: std::sync::Arc<Store>,
    pub(crate) graphql_url: String,
}

#[cfg(test)]
tokio::task_local! {
    pub(crate) static LINEAR_TEST_CONTEXT: LinearTestContext;
}

#[cfg(test)]
pub(super) mod test_fixture;

#[cfg(test)]
mod asana_planning_tests;

#[cfg(test)]
mod notion_planning_tests;

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

/// Refresh from Linear, bounded by `LINEAR_REFRESH_TIMEOUT`. A timeout, an auth
/// failure, or any network error surfaces as `Err`; saved Work remains intact.
pub(crate) async fn refresh_wave(repo: &Path, wave: &str) -> OpsResult<()> {
    let work = async {
        let (ctx, initiative) = resolve_context(repo, wave).await?;
        refresh_inventory(repo, wave, &ctx, &initiative).await?;
        Ok(())
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
    initiative: &str,
) -> OpsResult<()> {
    let store = planning_store().await?;
    let registered = crate::work::wave::ensure_wave_row(&store, repo, wave)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let acquisition = lock_wave_planning(&registered).await?;
    refresh_inventory_locked(repo, &registered, ctx, initiative, &store, acquisition).await
}

pub(crate) async fn refresh_inventory_locked(
    repo: &Path,
    wave: &Wave,
    ctx: &LinearRepository,
    initiative: &str,
    store: &Store,
    acquisition: Arc<crate::store::PlanningLocks>,
) -> OpsResult<()> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let projects = checked_projects_with_store(repo, ctx, initiative, wave.slug(), store).await?;
    let mut items = ctx
        .client
        .list_project_issues(&projects)
        .await
        .map_err(linear_error)?;
    let removed = store
        .deleted_task_issues(wave.id())
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    items.retain(|item| !removed.contains(&item.id));
    crate::linear::acceptance::accept_linear_inventory(
        store,
        wave.id().clone(),
        initiative.to_owned(),
        projects,
        items,
        observed_at,
        Some(acquisition),
    )
    .await
    .map_err(|err| OpsError::Message(format!("failed to accept Linear inventory: {err}")))?;
    Ok(())
}

// ── init ────────────────────────────────────────────────────────────

pub fn connect(
    repo: &Path,
    options: &LinearInitOptions,
    progress: &impl Progress,
) -> OpsResult<LinearInitResult> {
    block_on_linear(connect_async(repo, options, progress))
}

async fn connect_async(
    repo: &Path,
    options: &LinearInitOptions,
    progress: &impl Progress,
) -> OpsResult<LinearInitResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let store = planning_store().await?;
    crate::work::wave::ensure_wave_row(&store, repo, &wave)
        .await
        .map_err(|cause| OpsError::Message(cause.to_string()))?;

    let repo_id = repository_id(repo)?;
    let existing_initiative = read_initiative(repo, &wave);
    // Connecting selects a transport; it must first preserve the existing binding.
    let existing_team = selected_linear_team(repo)?;

    let summary = crate::work::wave::config::read_wave_summary(repo, &wave)?;
    let title = title_case(&wave);
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

    // Initiative: keep an existing binding, else find or create it.
    let initiative_missing = existing_initiative.is_none();
    let (initiative_id, created) = match existing_initiative {
        Some(id) => (id, false),
        None => {
            progress.status(&format!("looking for Linear Initiative `{title}`"));
            match matching_initiative_id(
                &client.initiatives().await.map_err(linear_error)?,
                &title,
            )? {
                Some(id) => {
                    progress.status(&format!(
                        "linking wave/{wave} to existing Linear Initiative {id}"
                    ));
                    (id, false)
                }
                None => {
                    progress.status(&format!("creating Linear Initiative for wave/{wave}"));
                    (
                        client
                            .create_wave(&title, &summary)
                            .await
                            .map_err(linear_error)?,
                        true,
                    )
                }
            }
        }
    };
    if initiative_missing {
        write_initiative_to_wave(repo, &wave, &initiative_id)?;
    }
    let connection_changed = write_repository_linear_config(repo, &team.id)?;

    if initiative_missing || connection_changed {
        let _ = crate::ops::commit_workflow(
            repo,
            &crate::ops::CommitOptions {
                add: true,
                message: Some(format!("lf wave connect: {wave} to Linear")),
                ..crate::ops::CommitOptions::for_task("linear")
            },
            progress,
            &|_| {},
        )?;
    }

    let (ctx, initiative) = resolve_context(repo, &wave).await?;
    refresh_inventory(repo, &wave, &ctx, &initiative).await?;
    Ok(LinearInitResult {
        wave,
        initiative_id,
        created,
        team_id: team.id,
        team_key: team.key,
        team_created: team.created,
    })
}

// ── Pull request linkage ────────────────────────────────────────────

/// The Linear linkage a published PR carries on its owning issue: a first-class
/// attachment and a loopflow-managed comment. Ids are `None` until each is created;
/// their presence switches the writeback from create to idempotent update.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PrLinkageIds {
    pub attachment_id: Option<String>,
    pub comment_id: Option<String>,
}

/// The content a PR linkage writes onto its Linear issue: the issue to link, the
/// PR URL, the attachment title/subtitle, and the comment body. Assembled by the
/// caller from the PR model so the writeback layer stays free of PR-domain shape.
#[derive(Debug, Clone)]
pub(crate) struct PrLinkRequest {
    pub issue_id: String,
    pub url: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
}

/// A best-effort linkage attempt: the ids obtained so far (carrying prior ids
/// forward), plus the failure message when Linear writeback degraded. `error` is
/// `None` only on full success. This never fails the caller — the GitHub
/// publication has already happened.
#[derive(Debug, Clone)]
pub(crate) struct PrLinkageOutcome {
    pub ids: PrLinkageIds,
    pub error: Option<String>,
}

/// Idempotently link a published PR to its owning Linear issue. Upserts a
/// first-class attachment (create via `attachmentLinkURL`, later `attachmentUpdate`)
/// and a managed comment (create via `commentCreate`, later `commentUpdate`), so
/// repeated `pr open/submit/land` refresh the same linkage instead of duplicating.
/// Partial progress is preserved: whatever id was obtained rides back in the
/// outcome even when a later step fails.
pub(crate) async fn link_pr_async(
    repo: &Path,
    request: &PrLinkRequest,
    prior: &PrLinkageIds,
) -> PrLinkageOutcome {
    // PR links spend the reserved Linear budget so they never fail behind sync.
    crate::linear::urgent(link_pr(repo, request, prior)).await
}

async fn link_pr(repo: &Path, request: &PrLinkRequest, prior: &PrLinkageIds) -> PrLinkageOutcome {
    let client = match linear_client(repo).await {
        Ok(client) => client,
        Err(error) => {
            return PrLinkageOutcome {
                ids: prior.clone(),
                error: Some(error.to_string()),
            }
        }
    };
    if let Err(error) = acquire_task(repo, &request.issue_id, LinearRefresh::Auto).await {
        return PrLinkageOutcome {
            ids: prior.clone(),
            error: Some(error.to_string()),
        };
    }
    link_pr_with_client(&client, request, prior).await
}

async fn link_pr_with_client(
    client: &LinearClient,
    request: &PrLinkRequest,
    prior: &PrLinkageIds,
) -> PrLinkageOutcome {
    let mut ids = prior.clone();

    match &ids.attachment_id {
        Some(id) => {
            if let Err(error) = client
                .update_attachment(id, &request.title, &request.subtitle)
                .await
            {
                return PrLinkageOutcome {
                    ids,
                    error: Some(error.to_string()),
                };
            }
        }
        None => match client
            .link_attachment(&request.issue_id, &request.url, &request.title)
            .await
        {
            Ok(id) => ids.attachment_id = Some(id),
            Err(error) => {
                return PrLinkageOutcome {
                    ids,
                    error: Some(error.to_string()),
                }
            }
        },
    }

    match &ids.comment_id {
        Some(id) => {
            if let Err(error) = client.update_comment(id, &request.body).await {
                return PrLinkageOutcome {
                    ids,
                    error: Some(error.to_string()),
                };
            }
        }
        None => match client.comment(&request.issue_id, &request.body).await {
            Ok(id) => ids.comment_id = Some(id),
            Err(error) => {
                return PrLinkageOutcome {
                    ids,
                    error: Some(error.to_string()),
                }
            }
        },
    }

    PrLinkageOutcome { ids, error: None }
}

// ── status ──────────────────────────────────────────────────────────

pub fn list_linear_waves(repo: &Path) -> OpsResult<Vec<String>> {
    Ok(list_local_waves(repo)?
        .into_iter()
        .filter(|wave| wave_has_linear_initiative(repo, wave))
        .collect())
}

#[derive(Debug)]
pub(crate) struct TaskPlanningInspection {
    pub observation: LinearIssueEvidence,
    pub refresh_error: Option<String>,
}

pub(crate) async fn inspect_task_planning_async(
    repo: &Path,
    issue: &str,
    refresh: LinearRefresh,
) -> OpsResult<TaskPlanningInspection> {
    let scope = crate::repository::CanonicalRepo::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .to_string();
    let store = planning_store().await?;
    let existing = store
        .linear_issue_evidence(&scope, issue)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let age = existing.observed_at.map(|at| now - at);
    if existing.state == PlanningState::Removed
        || refresh == LinearRefresh::Never
        || (existing.state == PlanningState::Available
            && refresh == LinearRefresh::Auto
            && (age.is_some_and(|age| age < LINEAR_SOFT_STALE_SECS)
                || super::linear_observe::team_current(&store, &scope).await))
    {
        return Ok(TaskPlanningInspection {
            observation: existing,
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
        let discovered_initiative = if let Some(project) = observation
            .as_ref()
            .and_then(|(_, project)| project.as_ref())
        {
            Some(singular_project_initiative(project)?)
        } else if let Some(project) = &existing.project_id {
            let [initiative] = existing.initiative_ids.as_slice() else {
                return Err(OpsError::Message(format!(
                    "Linear Project {project} belongs to {} Initiatives; expected exactly one",
                    existing.initiative_ids.len()
                )));
            };
            Some(initiative.clone())
        } else {
            None
        };
        let mut acquisition = None;
        let mut confirmed_wave = None;
        if let Some(initiative) = discovered_initiative {
            let wave = wave_for_initiative(repo, &initiative)?;
            let locator = crate::work::wave::WaveLocator::discover(repo, &wave)
                .map_err(|error| OpsError::Message(error.to_string()))?;
            let registered = store
                .get_wave_at(&locator)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?
                .ok_or_else(|| OpsError::Message(format!("Wave {wave} is not initialized")))?;
            acquisition = Some(lock_wave_planning(&registered).await?);
            observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
            observation = repository
                .client
                .issue_ownership(selector)
                .await
                .map_err(linear_error)?;
            if let Some((_, Some(project))) = &observation {
                if project.initiative_ids.as_slice() != [initiative.as_str()] {
                    return Err(OpsError::Message(format!(
                        "Task {selector} ownership changed during discovery; retry planning"
                    )));
                }
                confirmed_wave = Some((registered.id().clone(), initiative));
            }
        }
        let Some((item, project)) = observation else {
            if existing.issue_id.is_some() {
                store
                    .invalidate_linear_issue(&scope, existing.clone(), acquisition)
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
        crate::linear::acceptance::accept_linear_issue(
            &store,
            &scope,
            item,
            project,
            observed_at,
            confirmed_wave,
            acquisition,
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
    // Read again: an event may have invalidated the record while acquisition ran.
    let mut observation = store
        .linear_issue_evidence(&scope, selector)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let refresh_error = match result {
        Ok(false) if matches!(observation.state, PlanningState::Removed | PlanningState::Available) => None,
        Ok(false) => {
            observation.state = PlanningState::Absent;
            Some(format!("task {issue:?} is absent from repository planning"))
        }
        Ok(true) if observation.state != PlanningState::Available => Some(format!(
            "Task {issue:?} observation cannot repair stored invalidation, unresolved Project ownership or confirmed removal"
        )),
        Ok(true) => None,
        Err(error) => {
            if observation.state == PlanningState::Available {
                observation.state = PlanningState::Unavailable;
            }
            Some(format!("unable to resolve task {issue:?}: {error}"))
        }
    };
    Ok(TaskPlanningInspection {
        observation,
        refresh_error,
    })
}

pub(crate) async fn acquire_task(
    repo: &Path,
    issue: &str,
    refresh: LinearRefresh,
) -> OpsResult<Task> {
    let read = inspect_task_planning_async(repo, issue, refresh).await?;
    let evidence = read.observation;
    if evidence.state != PlanningState::Available {
        return Err(OpsError::Message(read.refresh_error.unwrap_or_else(|| {
            format!(
                "Task {issue:?} planning is {:?}; refresh planning",
                evidence.state
            )
        })));
    }
    let team = repository_team_id(repo)?;
    if let Some(project) = &evidence.project_id {
        let [initiative] = evidence.initiative_ids.as_slice() else {
            return Err(OpsError::Message(format!(
                "Project {project} has no unambiguous Initiative"
            )));
        };
        wave_for_initiative(repo, initiative)?;
        if evidence.project_team_ids.as_slice() != [team.as_str()] {
            return Err(OpsError::Message(format!(
                "Project {project} must belong to repository Team {team}"
            )));
        }
    }
    if evidence.team_id.as_deref() != Some(team.as_str()) {
        return Err(OpsError::Message(format!(
            "Task {issue} must belong to repository Team {team}"
        )));
    }
    let store = planning_store().await?;
    let task = store
        .get_task_by_issue(evidence.issue_id.as_deref().unwrap_or(issue))
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| {
            OpsError::Message("accepted planning did not retain the Task identity".into())
        })?;
    let acquisition = store
        .sqlite
        .task_acquisition(&task.id)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if acquisition.state != PlanningState::Available {
        return Err(OpsError::Message(format!(
            "Task {issue} has invalid retained ownership; refresh planning"
        )));
    }
    Ok(task)
}

pub(crate) fn singular_project_initiative(project: &LinearProject) -> OpsResult<String> {
    match project.initiative_ids.as_slice() {
        [initiative] => Ok(initiative.clone()),
        initiatives => Err(OpsError::Message(format!(
            "Linear Project `{}` ({}) belongs to {} Initiatives [{}]; expected exactly one",
            project.name,
            project.id,
            initiatives.len(),
            project.initiative_ids.join(", ")
        ))),
    }
}

pub(crate) fn wave_for_initiative(repo: &Path, initiative_id: &str) -> OpsResult<String> {
    let matches = list_local_waves(repo)?
        .into_iter()
        .filter(|wave| read_initiative(repo, wave).as_deref() == Some(initiative_id))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [wave] => Ok(wave.clone()),
        [] => Err(OpsError::Message(format!(
            "Linear Initiative {initiative_id} is not bound by any local Wave"
        ))),
        waves => Err(OpsError::Message(format!(
            "Linear Initiative {initiative_id} is bound by multiple local Waves: {}; repair GOAL.md ownership",
            waves.join(", ")
        ))),
    }
}

// ── reteam ──────────────────────────────────────────────────────────

fn project_needs_reteam(bound_team: &str, project_team_ids: &[String]) -> bool {
    project_team_ids.len() != 1 || project_team_ids[0] != bound_team
}

#[derive(Debug)]
struct ReteamIdentifierUpdate {
    wave: String,
    issue_id: String,
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
            ".lf/config.yaml has no repository `planning.team`. \
             Run `lf wave connect <wave> --team-key <KEY>` to establish the migration target."
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
    wave: &crate::work::wave::Wave,
    project: &LinearProject,
    acquisition: Arc<crate::store::PlanningLocks>,
    expected_teams: &[String],
) -> OpsResult<()> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let confirmed = resolved
        .repository
        .client
        .project_ownership(&project.id)
        .await
        .map_err(linear_error)?;
    if confirmed.initiative_ids != project.initiative_ids
        || confirmed.team_ids.len() != expected_teams.len()
        || !expected_teams
            .iter()
            .all(|team| confirmed.team_ids.contains(team))
    {
        return Err(OpsError::Message(format!(
            "Project {} ownership changed during reteam",
            project.id
        )));
    }
    crate::linear::acceptance::reconcile_linear_project_teams(
        &resolved.store,
        wave.id(),
        &project.initiative_ids[0],
        confirmed,
        observed_at,
        acquisition,
    )
    .await
    .map_err(|error| OpsError::Message(error.to_string()))
}

async fn accept_reteam_task(
    repo: &Path,
    resolved: &ResolvedReteamContext,
    wave: &crate::work::wave::Wave,
    issue: &str,
    acquisition: Arc<crate::store::PlanningLocks>,
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
    let initiative = read_initiative(repo, wave.slug())
        .ok_or_else(|| OpsError::Message(format!("Wave {} lost its Initiative", wave.slug())))?;
    if item.team_id.as_deref() != Some(resolved.repository.team_id.as_str())
        || project.initiative_ids.as_slice() != [initiative.as_str()]
    {
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
    crate::linear::acceptance::accept_linear_issue(
        &resolved.store,
        wave.repo(),
        item,
        Some(project),
        observed_at,
        Some((wave.id().clone(), initiative)),
        Some(acquisition),
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
    let waves = list_linear_waves(repo)?;
    if waves.is_empty() {
        return Err(OpsError::Message(
            "repository has no Waves linked to Linear Initiatives".to_string(),
        ));
    }

    let mut registered = Vec::new();
    for wave in &waves {
        if apply {
            registered.push(
                crate::work::wave::ensure_wave_row(store, repo, wave)
                    .await
                    .map_err(|error| OpsError::Message(error.to_string()))?,
            );
        } else {
            let locator = crate::work::wave::WaveLocator::discover(repo, wave)
                .map_err(|error| OpsError::Message(error.to_string()))?;
            if let Some(wave) = store
                .get_wave_at(&locator)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?
            {
                registered.push(wave);
            }
        }
    }
    registered.sort_by(|left, right| left.id().as_str().cmp(right.id().as_str()));
    let mut locked_waves = BTreeMap::new();
    for wave in registered {
        let acquisition = lock_wave_planning(&wave).await?;
        locked_waves.insert(wave.slug().to_string(), (wave, acquisition));
    }
    let mut project_moves = Vec::new();
    let mut moves = Vec::new();
    let mut identifier_updates = Vec::new();
    let mut projects = Vec::new();
    let mut seen_initiatives = BTreeMap::new();
    let mut seen_projects = BTreeSet::new();
    let mut already = 0usize;
    let mut task_updates = 0usize;

    for wave in &waves {
        let initiative = read_initiative(repo, wave).ok_or_else(|| {
            OpsError::Message(format!(
                "wave/{wave} has no Linear Initiative; initialize every Wave before reteam"
            ))
        })?;
        if let Some(owner) = seen_initiatives.insert(initiative.clone(), wave.clone()) {
            return Err(OpsError::Message(format!(
                "Linear Initiative {initiative} is bound by both .lf/wave/{owner} and .lf/wave/{wave}; repair GOAL.md ownership before reteam"
            )));
        }
        progress.status(&format!("preflighting wave/{wave} Initiative {initiative}"));
        let wave_projects = resolved
            .repository
            .client
            .list_projects(&initiative)
            .await
            .map_err(linear_error)?;
        for project in wave_projects {
            if !seen_projects.insert(project.id.clone()) {
                return Err(OpsError::Message(format!(
                    "Linear Project `{}` ({}) appears under multiple Wave Initiatives",
                    project.name, project.id
                )));
            }
            if project.initiative_ids.as_slice() != [initiative.as_str()] {
                return Err(OpsError::Message(format!(
                    "Linear Project `{}` ({}) in wave/{wave} belongs to Initiatives [{}]; \
                     reteam requires exactly {initiative} before any provider mutation",
                    project.name,
                    project.id,
                    project.initiative_ids.join(", ")
                )));
            }
            if project_needs_reteam(team_id, &project.team_ids) {
                project_moves.push(LinearReteamProjectMove {
                    wave: wave.clone(),
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
                    let registered_identifier = store
                        .task_issue_identifier(&item.id)
                        .await
                        .map_err(|error| {
                            OpsError::Message(format!("failed to read task registry: {error}"))
                        })?;
                    if registered_identifier.is_some_and(|identifier| identifier != item.identifier)
                    {
                        identifier_updates.push(ReteamIdentifierUpdate {
                            wave: wave.clone(),
                            issue_id: item.id,
                        });
                    }
                } else {
                    moves.push(LinearReteamMove {
                        wave: wave.clone(),
                        project_id: project.id.clone(),
                        id: item.id,
                        old_identifier: item.identifier,
                        title: item.name,
                        new_identifier: None,
                    });
                }
            }
            projects.push((wave, project));
        }
    }

    if apply {
        // Linear requires the destination Team on a Project before its Issues
        // can move. Expand first; narrowing is the final provider phase.
        for (wave, project) in &projects {
            let (wave, acquisition) = &locked_waves[*wave];
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
            accept_reteam_project(
                resolved,
                wave,
                project,
                acquisition.clone(),
                &expected_teams,
            )
            .await?;
        }
        for update in identifier_updates {
            let (wave, acquisition) = &locked_waves[&update.wave];
            let (_, changed) =
                accept_reteam_task(repo, resolved, wave, &update.issue_id, acquisition.clone())
                    .await?;
            task_updates += usize::from(changed);
        }

        for mv in &mut moves {
            let marker = reteam_comment_marker(&mv.old_identifier, team_key);
            let comment_bodies = resolved
                .repository
                .client
                .observe_issue(&mv.id)
                .await
                .map_err(linear_error)?
                .comments
                .into_iter()
                .map(|comment| comment.body)
                .collect::<Vec<_>>();
            if !comment_bodies.iter().any(|body| body.contains(&marker)) {
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
            let (wave, acquisition) = &locked_waves[&mv.wave];
            let (confirmed_identifier, changed) =
                accept_reteam_task(repo, resolved, wave, &mv.id, acquisition.clone()).await?;
            if confirmed_identifier != new_identifier {
                return Err(OpsError::Message(format!(
                    "Task {} move is not confirmed",
                    mv.id
                )));
            }
            task_updates += usize::from(changed);
            mv.new_identifier = Some(new_identifier);
        }

        for (wave, project) in &projects {
            let (wave, acquisition) = &locked_waves[*wave];
            if project_needs_reteam(team_id, &project.team_ids) {
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
            accept_reteam_project(
                resolved,
                wave,
                project,
                acquisition.clone(),
                std::slice::from_ref(team_id),
            )
            .await?;
        }

        // Confirm the complete inventory after moving issues and narrowing Teams.
        for wave in &waves {
            let initiative =
                read_initiative(repo, wave).expect("preflight required every Initiative");
            let ctx = &resolved.repository;
            let (registered, acquisition) = &locked_waves[wave];
            refresh_inventory_locked(
                repo,
                registered,
                ctx,
                &initiative,
                store,
                acquisition.clone(),
            )
            .await?;
        }
    }

    Ok(LinearReteamResult {
        repository: resolved.repository.repo_id.to_string(),
        waves,
        team_id: team_id.to_string(),
        team_key: team_key.to_string(),
        applied: apply,
        project_moves,
        moves,
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
    let all_waves = list_local_waves(repo)?;
    let waves = match options.wave.as_deref() {
        Some(wave) => vec![resolve_wave(Some(wave))?],
        None => all_waves.clone(),
    };
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut blocking = Vec::new();
    let team_id = selected_linear_team(repo)?;

    if let Some(store) = open_existing_store().await {
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
        let message = ".lf/config.yaml has no repository `planning.team`; run `lf wave connect <wave> --team-key <KEY>`".to_string();
        diagnostics.push(message.clone());
        blocking.push(message);
    }

    let mut initiative_waves: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for wave in &all_waves {
        if let Some(initiative) = read_initiative(repo, wave) {
            initiative_waves
                .entry(initiative)
                .or_default()
                .push(wave.clone());
        } else {
            diagnostics.push(format!("wave/{wave} has no Linear Initiative"));
        }
    }
    for (initiative, owners) in &initiative_waves {
        if owners.len() > 1 {
            let message = format!(
                "Linear Initiative {initiative} is bound by multiple local Waves: {}",
                owners.join(", ")
            );
            diagnostics.push(message.clone());
            blocking.push(message);
        }
    }

    let client = build_client(team_id.clone()).await?;
    let repo_id = repository_id(repo)?;
    if let Some(team_id) = &team_id {
        client
            .validate_team_claim(team_id, repo_id.as_str())
            .await
            .map_err(linear_error)?;
    }
    progress.status("checking Linear repository Initiatives, Projects, and Tasks");
    let initiatives = client.initiatives().await.map_err(linear_error)?;
    let initiatives_by_id: BTreeMap<String, &crate::linear::InitiativeNode> = initiatives
        .iter()
        .map(|initiative| (initiative.id.clone(), initiative))
        .collect();
    for initiative in &initiatives {
        if !initiative_waves.contains_key(&initiative.id) {
            diagnostics.push(format!(
                "Linear Initiative `{}` ({}) is not linked by any local wave",
                initiative.name, initiative.id
            ));
        }
    }

    let default_branch = crate::git::get_default_branch(repo)?;
    let summary_revision =
        crate::git::rev_parse(repo, &format!("refs/heads/{default_branch}^{{commit}}"))?;
    let mut wave_updates = Vec::new();
    let mut seen_projects: BTreeMap<String, String> = BTreeMap::new();
    for wave in &waves {
        let goal = format!(".lf/wave/{wave}/GOAL.md");
        let Some(content) = crate::git::show_file(repo, &summary_revision, &goal)? else {
            let message = format!(
                "{goal} is missing on default branch {default_branch}; cannot sync its summary"
            );
            diagnostics.push(message.clone());
            blocking.push(message);
            continue;
        };
        let summary = crate::work::wave::config::wave_summary(&content);
        let summary = crate::linear::linear_description(&summary);
        let Some(initiative_id) = read_initiative(repo, wave) else {
            blocking.push(format!("wave/{wave} has no Linear Initiative"));
            continue;
        };
        let expected_initiative_name = title_case(wave);
        let Some(actual) = initiatives_by_id.get(&initiative_id) else {
            let message =
                format!("wave/{wave} points at missing Linear Initiative {initiative_id}");
            diagnostics.push(message.clone());
            blocking.push(message);
            continue;
        };
        let name = (actual.name != expected_initiative_name).then_some(expected_initiative_name);
        let summary =
            (actual.description.as_deref().unwrap_or_default() != summary).then_some(summary);
        if let Some(name) = &name {
            actions.push(format!(
                "rename Linear Initiative `{}` ({initiative_id}) to `{name}` for wave/{wave}",
                actual.name
            ));
        }
        if summary.is_some() {
            actions.push(format!(
                "update Linear Initiative {initiative_id} summary from {default_branch}:.lf/wave/{wave}/GOAL.md"
            ));
        }

        let projects = client
            .list_projects(&initiative_id)
            .await
            .map_err(linear_error)?;
        let mut slugs = BTreeMap::new();
        for project in projects {
            if team_id
                .as_deref()
                .is_some_and(|team| project_is_foreign(&project, team))
            {
                diagnostics.push(format!(
                    "skipped foreign-Team Project `{}` ({}) in wave/{wave}: Teams [{}]",
                    project.name,
                    project.id,
                    project.team_ids.join(", ")
                ));
                continue;
            }
            if let Some(existing_wave) = seen_projects.insert(project.id.clone(), wave.clone()) {
                let message = format!(
                    "Linear Project `{}` ({}) appears under both wave/{existing_wave} and wave/{wave}",
                    project.name, project.id
                );
                diagnostics.push(message.clone());
                blocking.push(message);
            }
            if project.initiative_ids.as_slice() != [initiative_id.as_str()] {
                let message = format!(
                    "Linear Project `{}` ({}) in wave/{wave} belongs to Initiatives [{}]; expected exactly {initiative_id}",
                    project.name,
                    project.id,
                    project.initiative_ids.join(", ")
                );
                diagnostics.push(message.clone());
                blocking.push(message);
            }
            if let Some(team_id) = &team_id {
                if project.team_ids.as_slice() != [team_id.as_str()] {
                    let message = format!(
                        "Linear Project `{}` ({}) in wave/{wave} belongs to Teams [{}]; expected exactly repository Team {team_id}. Run `lf reteam`.",
                        project.name,
                        project.id,
                        project.team_ids.join(", ")
                    );
                    diagnostics.push(message.clone());
                    blocking.push(message);
                }
            }
            let name = &project.name;
            let slug = &project.slug;
            if let Some(existing) = slugs.insert(slug.clone(), name.clone()) {
                let message = format!(
                    "Linear Projects `{existing}` and `{name}` in wave/{wave} both derive slug `{slug}`"
                );
                diagnostics.push(message.clone());
                blocking.push(message);
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
                    let message = format!(
                        "Linear task {} resolves to Project {:?}, expected {}",
                        item.identifier, item.project_id, project.id
                    );
                    diagnostics.push(message.clone());
                    blocking.push(message);
                }
                if let Some(team_id) = &team_id {
                    if item.team_id.as_deref() != Some(team_id.as_str()) {
                        let message = format!(
                            "Linear task {} belongs to Team {}, expected repository Team {team_id}. Run `lf reteam`.",
                            item.identifier, item.team_id.as_deref().unwrap_or("unmapped")
                        );
                        diagnostics.push(message.clone());
                        blocking.push(message);
                    }
                }
            }
        }
        actions.push(format!(
            "refresh wave/{wave} Linear observations from Linear Initiative {initiative_id}"
        ));
        // Apply the checked binding and fields, not a later reread of the goal.
        wave_updates.push((wave, initiative_id, name, summary));
    }

    if !options.plan && !blocking.is_empty() {
        return Err(OpsError::Message(format!(
            "Linear ownership validation failed before mutation: {}",
            blocking.join("; ")
        )));
    }

    if !options.plan {
        let team_id = team_id.expect("non-plan sync requires repository Team");
        for (wave, initiative, name, summary) in wave_updates {
            if name.is_some() || summary.is_some() {
                client
                    .update_wave(&initiative, name.as_deref(), summary.as_deref())
                    .await
                    .map_err(linear_error)?;
            }
            let ctx = LinearRepository {
                client: client.clone(),
                repo_id: repo_id.clone(),
                team_id: team_id.clone(),
            };
            let store = planning_store().await?;
            super::chapter::adopt_legacy_projects(repo, &store, wave, &ctx, &initiative, true)
                .await?;
            refresh_inventory(repo, wave, &ctx, &initiative).await?;
        }
    }

    Ok(LinearSyncResult {
        actions,
        diagnostics,
    })
}

// ── explicit mutations ─────────────────────────────────────────────

pub(crate) async fn rename(
    repo: &Path,
    options: &LinearRenameOptions,
    progress: &impl Progress,
) -> OpsResult<LinearRenameResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let (ctx, initiative) = resolve_context(repo, &wave).await?;
    progress.status(&format!(
        "renaming Linear Initiative {} to {}",
        initiative, options.title
    ));
    ctx.client
        .rename_wave(&initiative, &options.title)
        .await
        .map_err(linear_error)?;
    progress.status(&format!(
        "refreshing local Linear observations for wave/{wave}"
    ));
    refresh_inventory(repo, &wave, &ctx, &initiative).await?;
    Ok(LinearRenameResult {
        wave,
        initiative,
        title: options.title.clone(),
    })
}

pub fn list_local_waves(repo: &Path) -> OpsResult<Vec<String>> {
    let canonical = crate::repository::CanonicalRepo::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let crate::store::StorageConfig::Sqlite { path } =
        super::planning_sync::storage_config_from_env()?;
    if !path
        .try_exists()
        .map_err(|error| OpsError::Message(error.to_string()))?
    {
        return Ok(Vec::new());
    }
    let store = crate::store::sqlite::SqliteStore::open_read_only(&path)
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

fn write_initiative_to_wave(repo: &Path, wave: &str, initiative_id: &str) -> OpsResult<()> {
    update_wave_config(repo, wave, |map| {
        let linear_key = serde_yaml_ng::Value::String("linear".to_string());
        let mut linear_map = map
            .get(&linear_key)
            .and_then(serde_yaml_ng::Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        linear_map.insert(
            serde_yaml_ng::Value::String("initiative".to_string()),
            serde_yaml_ng::Value::String(initiative_id.to_string()),
        );
        map.insert(linear_key, serde_yaml_ng::Value::Mapping(linear_map));
        Ok(())
    })
    .map_err(OpsError::Message)
}

fn write_repository_linear_config(repo: &Path, team_id: &str) -> OpsResult<bool> {
    let path = repo.join(".lf/config.yaml");
    let mut root = match std::fs::read_to_string(&path) {
        Ok(content) if !content.trim().is_empty() => {
            serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&content).map_err(|error| {
                OpsError::Message(format!(
                    "invalid repository config {}: {error}",
                    path.display()
                ))
            })?
        }
        Ok(_) => serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new())
        }
        Err(error) => return Err(error.into()),
    };
    let previous = root.clone();
    let root_map = root.as_mapping_mut().ok_or_else(|| {
        OpsError::Message(format!(
            "repository config {} must be a YAML mapping",
            path.display()
        ))
    })?;
    root_map.insert(
        "planning".into(),
        serde_yaml_ng::to_value(crate::config::PlanningConfig::Linear {
            team: Some(team_id.to_string()),
        })
        .map_err(|error| OpsError::Message(error.to_string()))?,
    );
    if root == previous {
        return Ok(false);
    }
    std::fs::create_dir_all(path.parent().expect("config path has parent"))?;
    std::fs::write(
        &path,
        serde_yaml_ng::to_string(&root).map_err(|error| {
            OpsError::Message(format!("failed to encode {}: {error}", path.display()))
        })?,
    )?;
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

fn matching_initiative_id(
    initiatives: &[crate::linear::InitiativeNode],
    title: &str,
) -> OpsResult<Option<String>> {
    let matches: Vec<_> = initiatives
        .iter()
        .filter(|initiative| initiative.name == title)
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [initiative] => Ok(Some(initiative.id.clone())),
        many => Err(OpsError::Message(format!(
            "multiple Linear Initiatives are named `{title}`: {}. Rename duplicates before running `lf wave connect`",
            many.iter()
                .map(|initiative| initiative.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn ensure_unique_project_slugs(projects: &[LinearProject], wave: &str) -> OpsResult<()> {
    let mut names_by_slug = BTreeMap::new();
    for project in projects {
        if project.slug.is_empty() {
            return Err(OpsError::Message(format!(
                "Linear Project `{}` ({}) in wave/{wave} has no usable slug",
                project.name, project.id
            )));
        }
        if let Some(existing) = names_by_slug.insert(project.slug.clone(), project.name.clone()) {
            return Err(OpsError::Message(format!(
                "Linear Projects `{existing}` and `{}` in wave/{wave} both derive slug `{}`",
                project.name, project.slug
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) async fn checked_projects(
    repo: &Path,
    ctx: &LinearRepository,
    initiative: &str,
    wave: &str,
) -> OpsResult<Vec<LinearProject>> {
    let store = planning_store().await?;
    checked_projects_with_store(repo, ctx, initiative, wave, &store).await
}

pub(super) async fn checked_projects_with_store(
    repo: &Path,
    ctx: &LinearRepository,
    initiative: &str,
    wave: &str,
    store: &Store,
) -> OpsResult<Vec<LinearProject>> {
    let mut projects = ctx
        .client
        .list_projects(initiative)
        .await
        .map_err(linear_error)?;
    let locator = crate::work::wave::WaveLocator::discover(repo, wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if let Some(registered) = store
        .get_wave_at(&locator)
        .await
        .map_err(|error| OpsError::Message(format!("failed to read Wave registry: {error}")))?
    {
        for known in store
            .list_projects(Some(registered.id()))
            .await
            .map_err(|error| {
                OpsError::Message(format!("failed to read retained Projects: {error}"))
            })?
        {
            let Some(id) = &known.linear_id else {
                // Local Work awaits its own export; it supplies no Linear lookup identity.
                continue;
            };
            if !projects.iter().any(|project| project.id == id.as_str()) {
                // An omitted membership cannot erase retained Project/Task history.
                projects.push(
                    ctx.client
                        .project_ownership(id.as_str())
                        .await
                        .map_err(linear_error)?,
                );
            }
        }
    }

    for adopted in
        super::chapter::adopt_legacy_projects(repo, store, wave, ctx, initiative, false).await?
    {
        if let Some(project) = projects.iter_mut().find(|project| project.id == adopted.id) {
            *project = adopted;
        } else {
            projects.push(adopted);
        }
    }
    projects.retain(|project| !project_is_foreign(project, &ctx.team_id));
    for project in &projects {
        validate_project_ownership(project, wave, initiative, &ctx.team_id)?;
    }
    ensure_unique_project_slugs(&projects, wave)?;
    Ok(projects)
}

pub(super) fn project_is_foreign(project: &LinearProject, team_id: &str) -> bool {
    !project.team_ids.is_empty() && !project.team_ids.iter().any(|id| id == team_id)
}

fn validate_project_ownership(
    project: &LinearProject,
    wave: &str,
    initiative_id: &str,
    team_id: &str,
) -> OpsResult<()> {
    crate::linear::validate_project_ownership(wave, initiative_id, Some(team_id), project).map_err(
        |error| {
            OpsError::Message(format!(
                "{error}. Repair the associations and run `lf wave status {wave} --sync`."
            ))
        },
    )
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

pub fn canonical_wave_title_path(repo: &Path, wave: &str) -> OpsResult<String> {
    block_on_linear(canonical_wave_title_path_async(repo, wave))
}

async fn canonical_wave_title_path_async(repo: &Path, wave: &str) -> OpsResult<String> {
    let store = planning_store().await?;
    canonical_wave_title_path_with_store(repo, wave, &store).await
}

async fn canonical_wave_title_path_with_store(
    repo: &Path,
    wave: &str,
    store: &Store,
) -> OpsResult<String> {
    let locator = crate::work::wave::WaveLocator::discover(repo, wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let Some(mut current) = store
        .get_wave_at(&locator)
        .await
        .map_err(|error| OpsError::Message(format!("failed to read Wave ancestry: {error}")))?
    else {
        if wave.contains('/') {
            return Err(OpsError::Message(format!(
                "nested wave/{wave} has no durable registry ancestry; start or prepare its promotion first"
            )));
        }
        return Ok(title_case(wave));
    };

    let main = crate::git::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let main = std::fs::canonicalize(&main).unwrap_or(main);
    let mut seen = BTreeSet::new();
    let mut segments = Vec::new();
    loop {
        if !seen.insert(current.id().as_str().to_string()) {
            return Err(OpsError::Message(format!(
                "Wave ancestry for wave/{wave} contains a cycle at {}",
                current.id()
            )));
        }
        let current_repo = std::fs::canonicalize(current.repo())
            .unwrap_or_else(|_| Path::new(current.repo()).to_path_buf());
        if current_repo != main {
            return Err(OpsError::Message(format!(
                "Wave ancestry for wave/{wave} crosses repositories at {} ({})",
                current.slug(),
                current.repo()
            )));
        }
        segments.push(title_case(current.name()));
        let Some(parent_id) = current.parent_wave_id().cloned() else {
            break;
        };
        current = store
            .get_wave(&parent_id)
            .await
            .map_err(|error| OpsError::Message(format!("failed to read Wave ancestry: {error}")))?
            .ok_or_else(|| {
                OpsError::Message(format!(
                    "Wave ancestry for wave/{wave} is incomplete: parent {parent_id} is missing"
                ))
            })?;
    }
    segments.reverse();
    Ok(segments.join(" / "))
}

fn block_on_linear<T>(future: impl Future<Output = OpsResult<T>>) -> OpsResult<T> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to create async runtime: {err}")))?;
    rt.block_on(future)
}

fn linear_error(err: LinearError) -> OpsError {
    OpsError::Message(err.to_string())
}

#[derive(Debug)]
pub(crate) struct ChapterSweep {
    pub candidates: Vec<(String, String, LinearIssue)>,
    pub skipped_projects: Vec<(String, LinearProject)>,
}

/// Read every linked Initiative, including archived predecessor Projects.
pub(crate) async fn chapter_sweep_candidates(repo: &Path) -> OpsResult<ChapterSweep> {
    let store = planning_store().await?;
    let mut candidates = Vec::new();
    let mut skipped_projects = Vec::new();
    let mut skipped_ids = BTreeSet::new();
    for name in list_linear_waves(repo)? {
        let (ctx, initiative) = resolve_context(repo, &name).await?;
        let locator = crate::work::wave::WaveLocator::discover(repo, &name)
            .map_err(|error| OpsError::Message(error.to_string()))?;
        let wave = store
            .get_wave_at(&locator)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
            .ok_or_else(|| OpsError::Message(format!("{name}: current chapter is unavailable")))?;
        let chapter = super::project::current_project(&store, &wave)?;
        for project in ctx
            .client
            .list_projects_including_archived(&initiative, true)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
        {
            if project_is_foreign(&project, &ctx.team_id) {
                if skipped_ids.insert(project.id.clone()) {
                    skipped_projects.push((name.clone(), project));
                }
                continue;
            }
            validate_project_ownership(&project, &name, &initiative, &ctx.team_id)?;
            if chapter
                .linear_id
                .as_ref()
                .is_some_and(|id| id.as_str() == project.id)
            {
                continue;
            }
            for item in ctx
                .client
                .list_items_including_archived(&project.id, true)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?
            {
                if item.completed
                    || matches!(
                        item.state.as_deref(),
                        Some("completed" | "canceled" | "duplicate")
                    )
                {
                    continue;
                }
                if item.state.is_none() || item.team_id.as_deref() != Some(ctx.team_id.as_str()) {
                    return Err(OpsError::Message(format!(
                        "{} has unresolved state or ownership; sweep was not applied",
                        item.identifier
                    )));
                }
                candidates.push((name.clone(), project.name.clone(), item));
            }
        }
    }
    Ok(ChapterSweep {
        candidates,
        skipped_projects,
    })
}

pub(crate) async fn require_outside_current_chapter(repo: &Path, issue: &str) -> OpsResult<Task> {
    let resolved = acquire_task(repo, issue, LinearRefresh::Force).await?;
    let store = planning_store().await?;
    let Some(wave_id) = &resolved.wave_id else {
        return Ok(resolved);
    };
    let wave = store
        .get_wave(wave_id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message("current Wave is unavailable".into()))?;
    let chapter = super::project::current_project(&store, &wave)?;
    if Some(&chapter.id) == resolved.project_id.as_ref()
        || matches!(
            resolved.state.as_deref(),
            Some("completed" | "canceled" | "duplicate")
        )
    {
        return Err(OpsError::Message(
            "issue moved to the current chapter or is already terminal".into(),
        ));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::test_server::{self, json_response, QueuedResponse};
    use crate::ops::NullProgress;
    use crate::test_linear_input::{Inventory, InventoryFixture, SeedLinear};
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
            json!({ "data": { "initiative": { "projects": {
                "nodes": projects,
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } } }),
        )
    }

    fn project_node(id: &str, name: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "description": "",
            "content": "workflow: feature\n\n## Definition\n\nA measured bet.\n\n## KRs\n",
            "status": {"type":"started"},
            "initiatives": { "nodes": [{ "id": "initiative-123" }] },
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

    fn migration_project_node(id: &str, name: &str, initiative: &str, teams: &[&str]) -> Value {
        json!({
            "id": id,
            "name": name,
            "description": "A measured bet.",
            "status": {"type":"started"},
            "content": "## Definition\n\nA measured bet.\n\n## Graphs\n\n- first: (none)\n- loop: (none)\n- finally: (none)\n\n## KRs\n\n- [ ] Ownership holds",
            "initiatives": { "nodes": [{ "id": initiative }] },
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
            "completedAt": null, "dueDate": null, "prioritySortOrder": 0.0,
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
    fn connect_overrides_personal_git_without_rebinding_the_repository() {
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
                let (url, _) = test_server::spawn(vec![
                    json_response(StatusCode::OK, team.clone()),
                    json_response(StatusCode::OK, team.clone()),
                    json_response(StatusCode::OK, team.clone()),
                    projects_response(json!([])),
                    json_response(StatusCode::OK, team.clone()),
                    json_response(StatusCode::OK, team.clone()),
                    json_response(StatusCode::OK, team),
                    projects_response(json!([])),
                ])
                .await;
                LINEAR_TEST_CONTEXT
                    .scope(fixture.context(&url), async {
                        let options = LinearInitOptions {
                            wave: Some("product".into()),
                            ..Default::default()
                        };
                        for _ in 0..2 {
                            let result =
                                connect_async(&repo, &options, &NullProgress).await.unwrap();
                            assert_eq!(result.team_id, "team-1");
                            assert_eq!(result.initiative_id, "initiative-1");
                            assert!(!result.created && !result.team_created);
                        }
                        let config = crate::config::load_config(Some(&repo)).unwrap().unwrap();
                        assert_eq!(
                            config.planning_transport(),
                            crate::config::PlanningConfig::Linear {
                                team: Some("team-1".into())
                            }
                        );
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

    #[test]
    fn connection_selects_linear_and_preserves_unrelated_authored_content() {
        let repo = tempfile::tempdir().unwrap();
        write_repo_config(
            repo.path(),
            "agent: codex\nplanning:\n  provider: git\n  shared: colleagues\n",
        );
        write_goal(repo.path(), "product", "agent: claude\n");
        let goal = repo.path().join(".lf/wave/product/GOAL.md");

        write_repository_linear_config(repo.path(), "team-product").unwrap();
        write_initiative_to_wave(repo.path(), "product", "initiative-product").unwrap();
        let config = crate::config::load_repo_config(repo.path())
            .unwrap()
            .unwrap();
        assert_eq!(
            config.planning_transport(),
            crate::config::PlanningConfig::Linear {
                team: Some("team-product".into())
            }
        );
        assert_eq!(config.agent.as_deref(), Some("codex"));
        let updated = std::fs::read_to_string(goal).unwrap();
        assert!(updated.ends_with("---\nDrive the work.\n"));
        let wave = crate::config::load_wave_config(Some(repo.path()), Some("product"))
            .unwrap()
            .unwrap();
        assert_eq!(wave.agent.as_deref(), Some("claude"));
        let wave = read_wave_config(repo.path(), "product").unwrap();
        assert_eq!(
            wave.linear.unwrap().initiative.as_deref(),
            Some("initiative-product")
        );
    }

    #[test]
    fn repository_team_config_is_the_only_normal_authority() {
        with_pm_home(|repo, runtime| {
            write_repo_config(
                repo.path(),
                "planning:\n  provider: linear\n  team: team-loo\n",
            );
            write_goal(
                repo.path(),
                "product",
                "linear:\n  initiative: initiative-product\n",
            );

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
            assert_eq!(
                read_initiative(repo.path(), "product").as_deref(),
                Some("initiative-product")
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
                    "linear:\n  initiative: initiative-survival\n",
                );
                write_goal(
                    repo.path(),
                    "survival/infrastructure",
                    "linear:\n  initiative: initiative-infrastructure\n",
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
                    "initiative-survival",
                    &["team-old"],
                );
                let old_infrastructure = migration_project_node(
                    "project-infrastructure",
                    "Infrastructure — Gmail",
                    "initiative-infrastructure",
                    &["team-old"],
                );
                let new_survival = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    "initiative-survival",
                    &["team-loo"],
                );
                let new_infrastructure = migration_project_node(
                    "project-infrastructure",
                    "Infrastructure — Gmail",
                    "initiative-infrastructure",
                    &["team-loo"],
                );
                let mut expanded_survival = old_survival.clone();
                expanded_survival["teams"] = json!({"nodes": [{"id":"team-old"},{"id":"team-loo"}]});
                let mut expanded_infrastructure = old_infrastructure.clone();
                expanded_infrastructure["teams"] = expanded_survival["teams"].clone();
                let responses = vec![
                    projects_response(json!([old_survival])),
                    issues_response(json!([migration_issue_node(
                        "issue-open",
                        "OLD-1",
                        "project-survival",
                        "Survival — A real task reaches done",
                        "team-old",
                        false,
                    )])),
                    projects_response(json!([old_infrastructure])),
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
                    projects_response(json!([new_survival])),
                    issues_response(json!([migration_issue_node(
                        "issue-open",
                        "LOO-1",
                        "project-survival",
                        "Survival — A real task reaches done",
                        "team-loo",
                        false,
                    )])),
                    projects_response(json!([new_infrastructure])),
                    issues_response(json!([migration_issue_node(
                        "issue-done",
                        "LOO-2",
                        "project-infrastructure",
                        "Infrastructure — Gmail",
                        "team-loo",
                        true,
                    )])),
                ];
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
                    assert!(read_initiative(repo.path(), wave).is_some());
                    let locator = crate::work::wave::WaveLocator::discover(repo.path(), wave).unwrap();
                    let registered = resolved.store.get_wave_at(&locator).await.unwrap().unwrap();
                    let projects = resolved.store.list_projects(Some(registered.id())).await.unwrap();
                    assert_eq!(projects.len(), 1);
                    let teams: String = rusqlite::Connection::open(&database).unwrap().query_row(
                        "SELECT team_ids FROM pm_projects WHERE id=?1",
                        [projects[0].linear_id.as_ref().unwrap().as_str()], |r| r.get(0)
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
                    "linear:\n  initiative: initiative-survival\n",
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
                    "initiative-survival",
                    &["team-old"],
                );
                let expanded_project = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    "initiative-survival",
                    &["team-old", "team-loo"],
                );
                let migrated_project = migration_project_node(
                    "project-survival",
                    "Survival — A real task reaches done",
                    "initiative-survival",
                    &["team-loo"],
                );
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
                    responses.push(projects_response(json!([old_project.clone()])));
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
                    projects_response(json!([old_project.clone()])),
                    issues_response(json!([old_issue.clone()])),
                ]);
                if interruption == "expansion" {
                    responses.extend([
                        failure(),
                        projects_response(json!([expanded_project.clone()])),
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
                            projects_response(json!([expanded_project.clone()])),
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
                        projects_response(json!([migrated_project.clone()])),
                        issues_response(json!([migrated_issue.clone()])),
                        project_readback(&migrated_project),
                    ]);
                } else {
                    responses.push(project_update_response("project-survival"));
                }
                responses.extend([
                    project_readback(&migrated_project),
                    projects_response(json!([migrated_project.clone()])),
                    issues_response(json!([migrated_issue.clone()])),
                ]);
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
                    let projects = resolved
                        .repository
                        .client
                        .list_projects("initiative-survival")
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
                                provider: "linear".into(),
                                initiative: "initiative-survival".into(),
                                synced_at: 1,
                                snapshot: Inventory { projects, items },
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
                let projects = resolved.store.list_projects(Some(wave.id())).await.unwrap();
                let teams: String = rusqlite::Connection::open(&database).unwrap().query_row(
                        "SELECT team_ids FROM pm_projects WHERE id=?1",
                        [projects[0].linear_id.as_ref().unwrap().as_str()], |r| r.get(0)
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

    #[test]
    fn duplicate_linear_project_slugs_are_drift() {
        let project = |id: &str, name: &str| LinearProject {
            description: None,
            revision: None,
            id: id.to_string(),
            slug: crate::linear::project_slug(name),
            name: name.to_string(),
            summary: String::new(),

            metric_targets: Vec::new(),
            status: crate::work::project::ProjectStatus::Started,
            krs: Vec::new(),
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: vec!["team-loo".to_string()],
        };
        let projects = vec![project("one", "Wave Chat"), project("two", "Wave-Chat")];

        let error = ensure_unique_project_slugs(&projects, "product")
            .expect_err("duplicate slug must fail");
        assert!(error.to_string().contains("both derive slug `wave-chat`"));
    }

    #[tokio::test]
    async fn acquisition_reads_projects_and_their_issues() {
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([project_node("project-123", "Scan")])),
            issues_response(json!([
                { "id": "issue-1", "identifier": "LOO-1", "url": null,
                  "title": "First", "description": "one",
                  "completedAt": null, "dueDate": null, "prioritySortOrder": 0.0, "sortOrder": 0.0, "updatedAt":"2026-09-29T12:00:00.123Z",
                  "assignee": null, "state": { "type": "unstarted" },
                  "project": { "id": "project-123", "name": "Scan" },
                  "team": { "id": "team-123" } }
            ])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url);
        let repo = tempfile::tempdir().unwrap();
        let store = isolated_pm_store(repo.path()).await;

        let result =
            test_fixture::capture_inventory(repo.path(), "scan", &ctx, "initiative-123", &store)
                .await
                .expect("fetch succeeds");
        assert_eq!(result.projects.len(), 1);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "First");
        assert_eq!(
            requests.lock().await[1].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
    }

    #[tokio::test]
    async fn acquisition_preserves_moved_and_detached_issue_ownership() {
        let (base_url, _) = test_server::spawn(vec![
            projects_response(json!([project_node("project-123", "Scan")])),
            issues_response(json!([
                { "id": "moved", "identifier": "LOO-1", "url": null,
                  "title": "Moved", "description": "Preserve the observed destination",
                  "completedAt": null, "dueDate": null, "prioritySortOrder": 0.0, "sortOrder": 0.0,
                  "updatedAt": "2026-10-05T12:00:00Z",
                  "assignee": null, "state": { "type": "started" },
                  "project": { "id": "successor", "name": "Next work" },
                  "team": { "id": "team-123" } },
                { "id": "detached", "identifier": "LOO-2", "url": null,
                  "title": "Detached", "description": "Do not invent membership",
                  "completedAt": null, "dueDate": null, "prioritySortOrder": 1.0, "sortOrder": 1.0,
                  "updatedAt": "2026-10-05T12:00:01Z",
                  "assignee": null, "state": { "type": "started" },
                  "project": null, "team": { "id": "team-123" } }
            ])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url);
        let repo = tempfile::tempdir().unwrap();
        let store = isolated_pm_store(repo.path()).await;

        let snapshot =
            test_fixture::capture_inventory(repo.path(), "scan", &ctx, "initiative-123", &store)
                .await
                .unwrap();
        assert_eq!(snapshot.items[0].project_id.as_deref(), Some("successor"));
        assert_eq!(
            snapshot.items[0].revision.as_deref(),
            Some("2026-10-05T12:00:00Z")
        );
        assert_eq!(snapshot.items[1].project_id, None);
        assert_eq!(
            snapshot.items[1].revision.as_deref(),
            Some("2026-10-05T12:00:01Z")
        );
    }

    fn attachment_link_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "attachmentLinkURL": { "attachment": { "id": id } } } }),
        )
    }

    fn attachment_update_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "attachmentUpdate": { "attachment": { "id": id } } } }),
        )
    }

    fn comment_create_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "commentCreate": { "comment": { "id": id } } } }),
        )
    }

    fn comment_update_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "commentUpdate": { "comment": { "id": id } } } }),
        )
    }

    fn link_request(subtitle: &str) -> PrLinkRequest {
        PrLinkRequest {
            issue_id: "issue-uuid".to_string(),
            url: "https://github.com/acme/repo/pull/7".to_string(),
            title: "GitHub PR #7".to_string(),
            subtitle: subtitle.to_string(),
            body: format!("[GitHub PR #7](https://github.com/acme/repo/pull/7) — {subtitle}"),
        }
    }

    #[tokio::test]
    async fn link_pr_creates_attachment_and_comment_on_first_publish() {
        let (base_url, requests) = test_server::spawn(vec![
            attachment_link_response("att-1"),
            comment_create_response("comment-1"),
        ])
        .await;
        let client = linear_test_ctx(base_url).client.clone();

        let outcome = link_pr_with_client(
            &client,
            &link_request("Open · published"),
            &PrLinkageIds::default(),
        )
        .await;

        assert_eq!(outcome.ids.attachment_id.as_deref(), Some("att-1"));
        assert_eq!(outcome.ids.comment_id.as_deref(), Some("comment-1"));
        assert!(outcome.error.is_none());

        let requests = requests.lock().await;
        let link = requests
            .iter()
            .find(|req| req.body.contains("attachmentLinkURL"))
            .expect("create sends attachmentLinkURL");
        // The create path must never send an argument Linear rejects: the
        // `subtitle` on attachmentLinkURL is the 400 that shipped in #1010.
        let link_body: Value = serde_json::from_str(&link.body).expect("link body is json");
        assert!(
            link_body["variables"].get("subtitle").is_none(),
            "attachmentLinkURL must not send a subtitle variable"
        );
        assert!(requests
            .iter()
            .any(|req| req.body.contains("commentCreate")));
    }

    #[tokio::test]
    async fn link_pr_updates_existing_linkage_without_duplicating() {
        let (base_url, requests) = test_server::spawn(vec![
            attachment_update_response("att-1"),
            comment_update_response("comment-1"),
        ])
        .await;
        let client = linear_test_ctx(base_url).client.clone();
        let prior = PrLinkageIds {
            attachment_id: Some("att-1".to_string()),
            comment_id: Some("comment-1".to_string()),
        };

        let outcome = link_pr_with_client(
            &client,
            &link_request("Open · completes task on merge"),
            &prior,
        )
        .await;

        assert!(outcome.error.is_none());
        assert_eq!(outcome.ids, prior);

        let requests = requests.lock().await;
        // Existing ids drive in-place updates, never a second create.
        assert!(requests
            .iter()
            .any(|req| req.body.contains("attachmentUpdate")));
        assert!(requests
            .iter()
            .any(|req| req.body.contains("commentUpdate")));
        assert!(!requests
            .iter()
            .any(|req| req.body.contains("commentCreate")));
        assert!(!requests
            .iter()
            .any(|req| req.body.contains("attachmentLinkURL")));
        // The refreshed state rides the update body.
        assert!(requests
            .iter()
            .any(|req| req.body.contains("completes task on merge")));
    }

    #[tokio::test]
    async fn link_pr_records_error_then_completes_on_retry() {
        // First publish: attachment links, but the comment write fails.
        let (base_url, _requests) = test_server::spawn(vec![
            attachment_link_response("att-1"),
            json_response(
                StatusCode::OK,
                json!({ "errors": [{ "message": "linear is down" }] }),
            ),
        ])
        .await;
        let client = linear_test_ctx(base_url).client.clone();

        let degraded = link_pr_with_client(
            &client,
            &link_request("Open · published"),
            &PrLinkageIds::default(),
        )
        .await;

        // Partial progress is preserved: the attachment id survives for the retry.
        assert_eq!(degraded.ids.attachment_id.as_deref(), Some("att-1"));
        assert!(degraded.ids.comment_id.is_none());
        assert!(degraded.error.is_some());

        // Retry with the surviving ids: the attachment updates in place and the
        // missing comment is created, clearing the error.
        let (base_url, requests) = test_server::spawn(vec![
            attachment_update_response("att-1"),
            comment_create_response("comment-1"),
        ])
        .await;
        let client = linear_test_ctx(base_url).client.clone();

        let healed =
            link_pr_with_client(&client, &link_request("Open · published"), &degraded.ids).await;

        assert!(healed.error.is_none());
        assert_eq!(healed.ids.attachment_id.as_deref(), Some("att-1"));
        assert_eq!(healed.ids.comment_id.as_deref(), Some("comment-1"));

        let requests = requests.lock().await;
        assert!(requests
            .iter()
            .any(|req| req.body.contains("attachmentUpdate")));
        assert!(requests
            .iter()
            .any(|req| req.body.contains("commentCreate")));
    }
}
