//! Provider planning operations shared by Task, Wave, and repository commands.
//!
//! Linear owns authored Project content and Tasks. `lf repo refresh` accepts
//! provider facts and projects them into SQLite atomically. Reads serve that
//! snapshot and only reach Linear through a bounded staleness policy
//! (see `load_show_snapshot`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::future::try_join_all;

use crate::durable::WorkRef;
use crate::engine::config::load_repo_config;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::task_pm::ResolvedTask;
use crate::ops::util::normalize_wave_name;
use crate::pm::linear::LinearClient;
use crate::pm::{PmError, PmItem, PmProject, PmSnapshot, PmWave};
use crate::provider_auth::{
    provider_token_refresh_due, refresh_stored_provider_token, Provider, TokenRefreshError,
};
use crate::repository::RepoId;
use crate::store::{
    open_existing_store, open_store, PlanningState, PmSnapshotRow, PmTaskObservation, PmTaskRecord,
    ProviderToken, ProviderTokenReplacement, StorageConfig, Store,
};
use crate::work::wave::config::{read_wave_config, update_wave_goal_config, WavePmConfig};
use crate::work::wave::Wave;

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PmInitOptions {
    pub wave: Option<String>,
    /// Repository Team key (Task prefix, e.g. `LOO`). Defaults from the repository name.
    pub team_key: Option<String>,
    /// Repository Team display name. Defaults to the repository name.
    pub team_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmInitResult {
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

#[derive(Debug, Clone, Default)]
pub struct PmShowOptions {
    pub wave: Option<String>,
    pub refresh: PmRefresh,
}

/// How planning reads reconcile the local snapshot with Linear before reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PmRefresh {
    /// Refresh only when the snapshot is stale (the default TTL policy).
    #[default]
    Auto,
    /// Always refresh before reading (`--sync`).
    Force,
    /// Never touch the network; serve the cache as-is (the default for status reads).
    Never,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PmShowResult {
    pub wave: String,
    pub provider: String,
    pub initiative: String,
    pub synced_at: i64,
    pub projects: Vec<PmProject>,
    pub items: Vec<PmItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmUpdateResult {
    pub wave: String,
    pub id: String,
}

/// The saved Task thread, its pending deliveries and any failed refresh.
/// Partial provider reads never replace the retained thread.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskComments {
    pub identifier: String,
    pub comments: Vec<TaskComment>,
    pub pending_sync: Vec<String>,
    /// Losing local comment bodies; the thread contains the adopted Linear values.
    pub conflicts: std::collections::BTreeMap<String, String>,
    pub refresh_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskComment {
    pub id: String,
    pub body: String,
    pub author: TaskCommentAuthor,
    pub created_at: Option<String>,
}

impl From<&crate::pm::IssueComment> for TaskComment {
    fn from(comment: &crate::pm::IssueComment) -> Self {
        let author =
            if comment.author_id.is_some() || crate::ops::linear_observe::is_steer(&comment.body) {
                TaskCommentAuthor::Person {
                    name: crate::ops::linear_observe::comment_requester(
                        &comment.body,
                        comment.author_name.as_deref(),
                    ),
                }
            } else {
                TaskCommentAuthor::Integration
            };
        Self {
            id: comment.id.clone(),
            body: comment.body.clone(),
            author,
            created_at: comment.created_at.clone(),
        }
    }
}

/// Who wrote a comment. A person has a provider user; an integration has none
/// and is never participant direction, whatever the display says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskCommentAuthor {
    Person { name: Option<String> },
    Integration,
}

#[derive(Debug, Clone, Default)]
pub struct PmSyncOptions {
    pub wave: Option<String>,
    pub plan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmSyncResult {
    pub actions: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PmReteamOptions {
    /// Execute the moves. Without it, `reteam` only prints the plan (dry run).
    pub apply: bool,
}

/// One issue that moves into the repository Team. `new_identifier` is filled only
/// after an applied move (Linear assigns the number then).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmReteamMove {
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
pub struct PmReteamProjectMove {
    pub wave: String,
    pub id: String,
    pub name: String,
    pub from_teams: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmReteamResult {
    pub repository: String,
    pub waves: Vec<String>,
    pub team_id: String,
    pub team_key: String,
    /// True when moves were executed; false for a dry run.
    pub applied: bool,
    /// Projects narrowed onto the repository Team (after their issues moved
    /// off the legacy team).
    pub project_moves: Vec<PmReteamProjectMove>,
    pub moves: Vec<PmReteamMove>,
    /// Issues already carrying the target Team id (skipped — idempotency).
    pub already: usize,
    /// Durable Tasks whose cached display identifier was reconciled.
    pub task_updates: usize,
}

#[derive(Debug, Clone)]
pub struct PmRenameOptions {
    pub wave: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmRenameResult {
    pub wave: String,
    pub initiative: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PmResolvedTask {
    pub wave: String,
    pub initiative_id: String,
    pub project: PmProject,
    pub item: PmItem,
}

// ── Client + Linear project resolution ──────────────────────────────

#[derive(Clone)]
pub(crate) struct RepositoryPmContext {
    pub client: LinearClient,
    pub repo_id: RepoId,
    pub team_id: String,
}

/// A Wave's Initiative inside its repository-owned PM authority.
pub(crate) struct PmContext {
    pub repository: RepositoryPmContext,
    pub initiative: String,
}

impl std::ops::Deref for PmContext {
    type Target = RepositoryPmContext;

    fn deref(&self) -> &Self::Target {
        &self.repository
    }
}

fn read_wave_pm_config(repo: &Path, wave: &str) -> Option<WavePmConfig> {
    // The PM fixture's saved Wave definitions share its snapshot/token store.
    #[cfg(test)]
    if let Ok(config) = PM_TEST_CONTEXT.try_with(|ctx| {
        let locator = crate::work::wave::WaveLocator::discover(repo, wave).unwrap();
        let wave = ctx.store.sqlite.get_wave_at(&locator).unwrap()?;
        let goal = ctx
            .store
            .sqlite
            .wave_document(wave.id(), "GOAL.md")
            .unwrap()?;
        crate::work::wave::config::parse_wave_config(&goal)
            .unwrap()
            .pm
    }) {
        return config;
    }
    read_wave_config(repo, wave).and_then(|config| config.pm)
}

pub(crate) fn resolve_wave(wave: Option<&str>) -> OpsResult<String> {
    wave.and_then(normalize_wave_name)
        .ok_or_else(|| OpsError::Message("cannot determine wave; pass --wave <name>".to_string()))
}

fn require_linear_provider(value: &str) -> OpsResult<()> {
    if value.trim().eq_ignore_ascii_case("linear") {
        return Ok(());
    }
    Err(OpsError::Message(format!(
        "unsupported PM provider {:?}; expected \"linear\"",
        value.trim().to_ascii_lowercase()
    )))
}

fn require_linear_config(repo: &Path) -> OpsResult<()> {
    let config = load_repo_config(repo)
        .map_err(|error| OpsError::Message(format!("failed to read .lf/config.yaml: {error}")))?
        .unwrap_or_default();
    if let Some(provider) = config
        .pm
        .as_ref()
        .and_then(|pm| pm.provider.as_deref())
        .filter(|provider| !provider.trim().is_empty())
    {
        require_linear_provider(provider)?;
    }
    Ok(())
}

pub(super) fn read_initiative(repo: &Path, wave: &str) -> Option<String> {
    read_wave_pm_config(repo, wave)?
        .linear_initiative
        .filter(|initiative| !initiative.trim().is_empty())
}

fn read_repository_team(repo: &Path) -> OpsResult<Option<String>> {
    let config = load_repo_config(repo)
        .map_err(|error| OpsError::Message(format!("failed to read .lf/config.yaml: {error}")))?
        .unwrap_or_default();
    Ok(config
        .pm
        .and_then(|pm| pm.linear_team)
        .filter(|team| !team.trim().is_empty()))
}

fn require_repository_team(repo: &Path) -> OpsResult<String> {
    read_repository_team(repo)?.ok_or_else(|| {
        OpsError::Message(
            ".lf/config.yaml has no repository `pm.linear_team`. \
             Run `lf repo connect <wave> --team-key <KEY>` before creating or mutating work."
                .to_string(),
        )
    })
}

fn wave_has_pm_initiative(repo: &Path, wave: &str) -> bool {
    require_linear_config(repo).is_ok() && read_initiative(repo, wave).is_some()
}

fn legacy_pm_sentinels(repo: &Path) -> OpsResult<Vec<String>> {
    let config = load_repo_config(repo)
        .map_err(|error| OpsError::Message(format!("failed to read .lf/config.yaml: {error}")))?
        .unwrap_or_default();
    let mut sentinels = Vec::new();
    if config.linear.team.is_some() {
        sentinels.push(".lf/config.yaml `linear.team`".to_string());
    }
    for wave in list_local_waves(repo)? {
        let Some(pm) = read_wave_pm_config(repo, &wave) else {
            continue;
        };
        if pm.provider.is_some() {
            sentinels.push(format!("wave/{wave}/GOAL.md `pm.provider`"));
        }
        if pm.linear_team.is_some() {
            sentinels.push(format!("wave/{wave}/GOAL.md `pm.linear_team`"));
        }
    }
    Ok(sentinels)
}

pub(crate) fn require_repository_pm_ready(repo: &Path) -> OpsResult<()> {
    let sentinels = legacy_pm_sentinels(repo)?;
    if sentinels.is_empty() {
        return Ok(());
    }
    Err(OpsError::Message(format!(
        "repository PM migration is required before this mutation; legacy authority remains at {}. \
         Run `lf repo reteam` to inspect the repository-wide plan, then `lf repo reteam --apply`. \
         Loopflow's live migration is owned by PRD-44.",
        sentinels.join(", ")
    )))
}

pub(crate) fn repository_team_id(repo: &Path) -> OpsResult<String> {
    require_repository_pm_ready(repo)?;
    require_linear_config(repo)?;
    require_repository_team(repo)
}

/// Expected Team for strict cached reads after migration. During the deliberate
/// PRD-43/PRD-44 mixed state, legacy snapshots remain inspectable and validate
/// their own singular Team rather than pretending they already carry the new one.
pub(crate) fn repository_team_for_snapshot_validation(
    repo: &Path,
    store: &Store,
) -> OpsResult<Option<String>> {
    let config = load_repo_config(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .unwrap_or_default();
    if config.linear.team.is_some() {
        return Ok(None);
    }
    for wave in store
        .sqlite
        .list_waves(Some(&repo.to_string_lossy()))
        .map_err(|error| OpsError::Message(error.to_string()))?
    {
        let Some(goal) = store
            .sqlite
            .wave_document(wave.id(), "GOAL.md")
            .map_err(|error| OpsError::Message(error.to_string()))?
        else {
            continue;
        };
        let definition = crate::work::wave::config::parse_wave_config(&goal)
            .map_err(|error| OpsError::Message(error.to_string()))?;
        if definition
            .pm
            .is_some_and(|pm| pm.provider.is_some() || pm.linear_team.is_some())
        {
            return Ok(None);
        }
    }
    require_linear_config(repo)?;
    read_repository_team(repo)
}

async fn build_client(team: Option<String>) -> OpsResult<LinearClient> {
    let token = resolve_pm_token().await?;
    #[cfg(test)]
    if let Ok(url) = PM_TEST_CONTEXT.try_with(|ctx| ctx.graphql_url.clone()) {
        return Ok(LinearClient::with_base_url(token, team, url));
    }
    Ok(LinearClient::new(token, team))
}

fn repository_id(repo: &Path) -> OpsResult<RepoId> {
    RepoId::discover(repo).map_err(|error| {
        OpsError::Message(format!(
            "cannot establish repository PM identity from Git origin: {error}. \
             Configure an origin before running `lf repo connect`."
        ))
    })
}

async fn resolve_repository_context(repo: &Path) -> OpsResult<RepositoryPmContext> {
    require_repository_pm_ready(repo)?;
    require_linear_config(repo)?;
    let team_id = require_repository_team(repo)?;
    let repo_id = repository_id(repo)?;
    let client = build_client(Some(team_id.clone())).await?;
    client
        .validate_team_claim(&team_id, repo_id.as_str())
        .await
        .map_err(pm_to_ops)?;
    Ok(RepositoryPmContext {
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
pub(crate) async fn issue_client(repo: &Path) -> OpsResult<LinearClient> {
    require_linear_config(repo)?;
    build_client(None).await
}

pub(crate) async fn resolve_context(repo: &Path, wave: &str) -> OpsResult<PmContext> {
    let repository = resolve_repository_context(repo).await?;
    let initiative = read_initiative(repo, wave).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.linear_initiative`. \
             Run `lf repo connect {wave}` to connect its Linear Initiative."
        ))
    })?;
    Ok(PmContext {
        repository,
        initiative,
    })
}

/// Linear authenticates via OAuth: the access token and refresh grant live in
/// store, and PM access refreshes the grant before the access token expires.
async fn resolve_pm_token() -> OpsResult<String> {
    let deadline = Instant::now() + PM_REFRESH_TIMEOUT;
    tokio::time::timeout(PM_REFRESH_TIMEOUT, async {
        let config = storage_config_from_env()?;
        let store = open_pm_store(&config).await?;
        let StorageConfig::Sqlite { path } = config;
        resolve_pm_token_from_store(&store, &path, deadline).await
    })
    .await
    .map_err(|_| credential_deadline())?
}

fn missing_linear_credential() -> OpsError {
    OpsError::Message(
        "No Linear credential found. Run `doppler run -- lf account connect linear`.".into(),
    )
}

fn credential_retry(reason: &str) -> OpsError {
    OpsError::Message(format!(
        "{reason}. Retry the PM operation; no credential was cleared."
    ))
}

fn credential_deadline() -> OpsError {
    credential_retry("Linear credential resolution deadline elapsed; a pending write may still settle, so the next call must re-read the credential")
}

fn usable_token(token: ProviderToken) -> OpsResult<String> {
    if token.access_token.trim().is_empty()
        || token
            .expires_at
            .is_some_and(|expiry| expiry <= time::OffsetDateTime::now_utc().unix_timestamp())
    {
        return Err(credential_retry(
            "Current Linear credential is unusable or expired",
        ));
    }
    Ok(token.access_token)
}

async fn linear_refresh_lock(database: &Path, deadline: Instant) -> OpsResult<std::fs::File> {
    let database = std::fs::canonicalize(database)
        .map_err(|_| credential_retry("Could not resolve the Linear credential store path"))?;
    let mut path = database.into_os_string();
    path.push(".linear-refresh.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| credential_retry("Could not open the Linear refresh lock"))?;
    loop {
        if Instant::now() >= deadline {
            return Err(credential_deadline());
        }
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(file),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err(_) => {
                return Err(credential_retry(
                    "Could not acquire the Linear refresh lock",
                ))
            }
        }
    }
}

async fn resolve_pm_token_from_store(
    store: &Store,
    database: &Path,
    deadline: Instant,
) -> OpsResult<String> {
    let read = || async {
        store
            .get_provider_token("linear")
            .await
            .map_err(|_| credential_retry("Could not read the Linear credential"))
    };
    let initial = read().await?.ok_or_else(missing_linear_credential)?;
    if !provider_token_refresh_due(&initial, time::OffsetDateTime::now_utc().unix_timestamp()) {
        return usable_token(initial);
    }
    let lock = linear_refresh_lock(database, deadline).await?;
    let current = read().await?.ok_or_else(missing_linear_credential)?;
    if !provider_token_refresh_due(&current, time::OffsetDateTime::now_utc().unix_timestamp()) {
        return usable_token(current);
    }
    for attempt in 0..2 {
        match refresh_stored_provider_token(Provider::Linear, &current).await {
            Ok(refreshed) => {
                // Persistence owns the file descriptor even if this future is cancelled.
                let replacement = store
                    .replace_provider_token(&current, &refreshed, lock, deadline)
                    .await;
                return match replacement {
                    Ok(ProviderTokenReplacement::Replaced) => usable_token(refreshed),
                    Ok(ProviderTokenReplacement::Changed(winner)) => usable_token(winner),
                    Ok(ProviderTokenReplacement::Missing) => Err(missing_linear_credential()),
                    Err(_) => {
                        // A failed write (including an uncertain commit) cannot authorize
                        // an unpersisted response. Re-read before considering fallback.
                        if let Some(latest) = read().await? {
                            if let Ok(access) = usable_token(latest) {
                                tracing::warn!("Linear refresh persistence failed; using the current stored token");
                                return Ok(access);
                            }
                        }
                        Err(credential_retry("Could not persist the refreshed Linear credential; re-read it on the next call"))
                    }
                };
            }
            Err(error) => {
                let latest = read().await?;
                if latest.as_ref() != Some(&current) {
                    return usable_token(latest.ok_or_else(missing_linear_credential)?);
                }
                let TokenRefreshError::OAuth { reason, .. } = error else {
                    return Err(credential_retry("Linear credential refresh failed"));
                };
                if attempt == 0 && reason.retryable_now() {
                    continue;
                }
                if let Ok(access) = usable_token(current.clone()) {
                    tracing::warn!(error = %reason, "proactive Linear refresh failed; using the still-valid token");
                    return Ok(access);
                }
                return Err(if reason.requires_reconnect() {
                    OpsError::Message(format!("Linear refresh failed: {reason}. Run `doppler run -- lf account connect linear` to reconnect."))
                } else {
                    credential_retry(&format!(
                        "Linear refresh failed: {reason}; prior credential preserved"
                    ))
                });
            }
        }
    }
    unreachable!("both refresh attempts return or retry")
}

fn storage_config_from_env() -> OpsResult<crate::store::StorageConfig> {
    #[cfg(test)]
    if let Ok(config) = PM_TEST_CONTEXT.try_with(|ctx| StorageConfig::sqlite(ctx.path.clone())) {
        return Ok(config);
    }
    crate::store::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve credential store: {err}")))
}

pub(crate) async fn pm_store() -> OpsResult<Store> {
    open_pm_store(&storage_config_from_env()?).await
}

async fn open_pm_store(config: &StorageConfig) -> OpsResult<Store> {
    #[cfg(test)]
    if let Ok(store) =
        PM_TEST_CONTEXT.try_with(|ctx| Store::from_sqlite_for_test(ctx.store.sqlite.clone()))
    {
        return Ok(store);
    }
    let config = config.clone();
    // Store opening uses synchronous SQLite I/O. Keep it off the deadline's executor.
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(open_store(&config))
    })
    .await
    .map_err(|_| credential_retry("Could not open the PM store"))?
    .map_err(|err| OpsError::Message(format!("failed to open PM snapshot store: {err}")))
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct PmTestContext {
    pub(crate) path: std::path::PathBuf,
    pub(crate) store: std::sync::Arc<Store>,
    pub(crate) graphql_url: String,
}

#[cfg(test)]
tokio::task_local! {
    pub(crate) static PM_TEST_CONTEXT: PmTestContext;
}

#[cfg(test)]
pub(super) mod test_fixture;

#[cfg(test)]
mod task_planning_tests;

#[cfg(test)]
mod planning_lookup_tests;

#[cfg(test)]
mod oauth_tests;

#[cfg(test)]
mod task_comments_tests;

// ── snapshot freshness policy ────────────────────────────────────────

/// Past this age an Auto read opportunistically refreshes before serving.
const PM_SOFT_STALE_SECS: i64 = 60 * 60; // 1 hour
/// Past this age a failed refresh is an error, not a silent cache fallback.
const PM_HARD_STALE_SECS: i64 = 7 * 24 * 60 * 60; // 1 week
/// Ceiling on an opportunistic refresh; exceeding it counts as a failure.
const PM_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

fn missing_snapshot_error(wave: &str) -> OpsError {
    OpsError::Message(format!(
        "wave/{wave} has no local PM snapshot. Run `lf repo refresh {wave}`."
    ))
}

pub(crate) fn format_age(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        "just now".to_string()
    } else if secs < 60 * 60 {
        format!("{}m", secs / 60)
    } else if secs < 24 * 60 * 60 {
        format!("{}h", secs / (60 * 60))
    } else {
        format!("{}d", secs / (24 * 60 * 60))
    }
}

async fn snapshot_row(repo: &Path, wave: &str) -> OpsResult<Option<PmSnapshotRow>> {
    let store = pm_store().await?;
    let locator = crate::work::wave::WaveLocator::discover(repo, wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let Some(wave) = store
        .get_wave_at(&locator)
        .await
        .map_err(|err| OpsError::Message(format!("failed to read Wave registry: {err}")))?
    else {
        return Ok(None);
    };
    store
        .pm_snapshot(wave.id())
        .await
        .map_err(|err| OpsError::Message(format!("failed to read PM snapshot: {err}")))
}

async fn read_pm_snapshot(repo: &Path, wave: &str) -> OpsResult<PmSnapshotRow> {
    snapshot_row(repo, wave)
        .await?
        .ok_or_else(|| missing_snapshot_error(wave))
}

/// Refresh from Linear, bounded by `PM_REFRESH_TIMEOUT`. A timeout, an auth
/// failure, or any network error surfaces as `Err` so callers can fall back to
/// the cache.
async fn try_timed_refresh(repo: &Path, wave: &str) -> OpsResult<PmSnapshotRow> {
    let work = async {
        let ctx = resolve_context(repo, wave).await?;
        refresh_pm_snapshot(repo, wave, &ctx).await?;
        read_pm_snapshot(repo, wave).await
    };
    match tokio::time::timeout(PM_REFRESH_TIMEOUT, work).await {
        Ok(result) => result,
        Err(_) => Err(OpsError::Message(format!(
            "Planning refresh exceeded its {}s deadline. Retry the PM operation; a pending credential write may still settle and will be re-read",
            PM_REFRESH_TIMEOUT.as_secs()
        ))),
    }
}

/// How a read reconciles a cached snapshot of a given age, independent of I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotPlan {
    /// Serve the cache without touching the network.
    ServeCache,
    /// Refresh first. `hard` means a failed refresh is an error, not a fallback.
    Refresh { hard: bool },
}

/// The freshness decision. `age` is `None` when no snapshot exists yet.
fn plan_snapshot_read(mode: PmRefresh, age: Option<i64>) -> SnapshotPlan {
    match mode {
        PmRefresh::Never => SnapshotPlan::ServeCache,
        PmRefresh::Force => SnapshotPlan::Refresh { hard: true },
        PmRefresh::Auto => match age {
            Some(age) if age < PM_SOFT_STALE_SECS => SnapshotPlan::ServeCache,
            Some(age) => SnapshotPlan::Refresh {
                hard: age >= PM_HARD_STALE_SECS,
            },
            None => SnapshotPlan::Refresh { hard: true },
        },
    }
}

/// Resolve the snapshot to serve, applying the requested refresh mode.
///
/// `Never` serves the cache untouched. `Force` always refreshes and errors if it
/// cannot. `Auto` serves a fresh (<1h) cache without touching the network,
/// refreshes past that, and on failure falls back to the cache — except a
/// hard-stale (>1w) snapshot that cannot refresh is an error, since serving a
/// week-old snapshot silently would mislead.
async fn load_show_snapshot(
    repo: &Path,
    wave: &str,
    mode: PmRefresh,
    progress: &impl Progress,
) -> OpsResult<PmSnapshotRow> {
    let existing = snapshot_row(repo, wave).await?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let age = existing.as_ref().map(|row| now - row.synced_at);

    let hard = match plan_snapshot_read(mode, age) {
        SnapshotPlan::ServeCache => return existing.ok_or_else(|| missing_snapshot_error(wave)),
        SnapshotPlan::Refresh { hard } => hard,
    };

    match age {
        Some(age) => progress.status(&format!(
            "wave/{wave} PM snapshot is {} stale; refreshing from Linear",
            format_age(age)
        )),
        None => progress.status(&format!(
            "wave/{wave} has no local PM snapshot; fetching from Linear"
        )),
    }

    match try_timed_refresh(repo, wave).await {
        Ok(row) => Ok(row),
        Err(err) => match existing {
            Some(row) if !hard => {
                progress.status(&format!(
                    "PM refresh failed ({err}); showing cached snapshot from {} ago",
                    format_age(now - row.synced_at)
                ));
                Ok(row)
            }
            Some(_) => {
                let reason = if mode == PmRefresh::Force {
                    format!("could not refresh wave/{wave} from Linear: {err}")
                } else {
                    format!(
                        "wave/{wave} PM snapshot is over a week stale and refresh failed: {err}"
                    )
                };
                Err(OpsError::Message(format!(
                    "{reason}. Retry with `lf repo refresh {wave}` after addressing the reported cause."
                )))
            }
            None => Err(OpsError::Message(format!(
                "{}; refresh failed: {err}",
                missing_snapshot_error(wave)
            ))),
        },
    }
}

async fn fetch_pm_snapshot_with_store(
    repo: &Path,
    wave: &str,
    ctx: &PmContext,
    store: &Store,
) -> OpsResult<PmSnapshot> {
    let projects = checked_projects_with_store(repo, ctx, wave, store).await?;
    let mut snapshot = fetch_pm_snapshot_for_projects(ctx, projects).await?;
    let locator = crate::work::wave::WaveLocator::discover(repo, wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if let Some(registered) = store
        .get_wave_at(&locator)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
    {
        let removed = store
            .deleted_task_issues(registered.id())
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?;
        snapshot.items.retain(|item| !removed.contains(&item.id));
    }
    Ok(snapshot)
}

async fn fetch_pm_snapshot_for_projects(
    ctx: &PmContext,
    projects: Vec<PmProject>,
) -> OpsResult<PmSnapshot> {
    let project_items = try_join_all(
        projects
            .iter()
            .map(|project| ctx.client.list_items(&project.id)),
    )
    .await
    .map_err(pm_to_ops)?;
    Ok(PmSnapshot {
        projects,
        items: project_items.into_iter().flatten().collect(),
    })
}

pub(crate) async fn require_planning_home(store: &Store, wave: &Wave) -> OpsResult<()> {
    let placement = store
        .placement(&WorkRef::Wave(wave.id().clone()))
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let local = store
        .local_machine()
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if placement.machine_id != local.id {
        return Err(OpsError::Message(format!(
            "Wave {} is placed on {}; run this command with `lf --machine {}`",
            wave.slug(),
            placement.machine_id,
            placement.machine_id
        )));
    }
    Ok(())
}

pub(crate) async fn lock_wave_planning(wave: &Wave) -> OpsResult<Arc<crate::store::PlanningLocks>> {
    // Keep the existing lock inode namespace shared with already-running callers.
    let path = crate::store::lf_home_dir().join("chapter-locks");
    #[cfg(test)]
    let path = PM_TEST_CONTEXT
        .try_with(|context| context.path.with_extension("chapter-locks"))
        .unwrap_or(path);
    std::fs::create_dir_all(&path).map_err(|error| OpsError::Message(error.to_string()))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.join(format!("{}.lock", wave.id())))
        .map_err(|error| OpsError::Message(error.to_string()))?;
    // OS ownership releases on crash; provider state makes the next holder a resumer.
    for _ in 0..300 {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(Arc::new(crate::store::PlanningLocks::new(file))),
            Err(cause) if cause.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(cause) => return Err(OpsError::Message(cause.to_string())),
        }
    }
    Err(OpsError::Message(
        "another planning operation is active; retry after it finishes".into(),
    ))
}

pub(crate) async fn refresh_pm_snapshot(
    repo: &Path,
    wave: &str,
    ctx: &PmContext,
) -> OpsResult<PmSnapshot> {
    let store = pm_store().await?;
    let registered = crate::work::wave::ensure_wave_row(&store, repo, wave)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let acquisition = lock_wave_planning(&registered).await?;
    refresh_pm_snapshot_locked(repo, &registered, ctx, &store, acquisition).await
}

pub(crate) async fn refresh_pm_snapshot_locked(
    repo: &Path,
    wave: &Wave,
    ctx: &PmContext,
    store: &Store,
    acquisition: Arc<crate::store::PlanningLocks>,
) -> OpsResult<PmSnapshot> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let snapshot = fetch_pm_snapshot_with_store(repo, wave.slug(), ctx, store).await?;
    store
        .put_pm_snapshot(
            PmSnapshotRow {
                wave_id: wave.id().clone(),
                provider: "linear".to_string(),
                initiative: ctx.initiative.clone(),
                synced_at: observed_at,
                snapshot: snapshot.clone(),
            },
            Some(acquisition),
        )
        .await
        .map_err(|err| OpsError::Message(format!("failed to store PM snapshot: {err}")))?;
    Ok(snapshot)
}

// ── init ────────────────────────────────────────────────────────────

pub fn pm_init(
    repo: &Path,
    options: &PmInitOptions,
    progress: &impl Progress,
) -> OpsResult<PmInitResult> {
    block_on_pm(pm_init_async(repo, options, progress))
}

async fn pm_init_async(
    repo: &Path,
    options: &PmInitOptions,
    progress: &impl Progress,
) -> OpsResult<PmInitResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let store = pm_store().await?;
    crate::work::wave::ensure_wave_row(&store, repo, &wave)
        .await
        .map_err(|cause| OpsError::Message(cause.to_string()))?;

    require_linear_config(repo)?;
    let repo_id = repository_id(repo)?;
    let existing_initiative = read_initiative(repo, &wave);
    let existing_team = read_repository_team(repo)?;

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
            .map_err(pm_to_ops)?,
        None => client
            .ensure_team(&team_name, &team_key, repo_id.as_str())
            .await
            .map_err(pm_to_ops)?,
    };
    let team_changed = existing_team.as_deref() != Some(team.id.as_str());

    // Initiative: keep an existing binding, else find or create it.
    let initiative_missing = existing_initiative.is_none();
    let (initiative_id, created) = match existing_initiative {
        Some(id) => (id, false),
        None => {
            progress.status(&format!("looking for Linear Initiative `{title}`"));
            match matching_wave_id(&client.list_waves().await.map_err(pm_to_ops)?, &title)? {
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
                            .map_err(pm_to_ops)?,
                        true,
                    )
                }
            }
        }
    };
    if initiative_missing {
        write_initiative_to_goal(repo, &wave, &initiative_id)?;
    }
    if team_changed {
        write_repository_pm_config(repo, &team.id)?;
    }

    if initiative_missing || team_changed {
        let _ = crate::ops::commit_workflow(
            repo,
            &crate::ops::CommitOptions {
                add: true,
                message: Some(format!("lf repo connect: {wave} to Linear")),
                ..crate::ops::CommitOptions::for_task("pm")
            },
            progress,
            &|_| {},
        )?;
    }

    let ctx = resolve_context(repo, &wave).await?;
    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(PmInitResult {
        wave,
        initiative_id,
        created,
        team_id: team.id,
        team_key: team.key,
        team_created: team.created,
    })
}

// ── show ────────────────────────────────────────────────────────────

pub fn pm_show(
    repo: &Path,
    options: &PmShowOptions,
    progress: &impl Progress,
) -> OpsResult<PmShowResult> {
    block_on_pm(pm_show_async(repo, options, progress))
}

pub(crate) async fn pm_show_async(
    repo: &Path,
    options: &PmShowOptions,
    progress: &impl Progress,
) -> OpsResult<PmShowResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let row = load_show_snapshot(repo, &wave, options.refresh, progress).await?;
    require_linear_provider(&row.provider)?;
    Ok(PmShowResult {
        wave,
        provider: "linear".into(),
        initiative: row.initiative,
        synced_at: row.synced_at,
        projects: row.snapshot.projects,
        items: row.snapshot.items,
    })
}

// ── update ──────────────────────────────────────────────────────────

/// Read the saved Task thread or save a comment without allocating execution state.
pub(crate) async fn task_comment_async(
    repo: &Path,
    wave: Option<&str>,
    issue: &str,
    message: Option<&str>,
    steer: bool,
) -> OpsResult<TaskComments> {
    let (store, task) = resolve_saved_task(repo, wave, issue).await?;
    if let Some(message) = message {
        super::task::append_task_comment(&store, &task, message, steer)?;
        let owner = store
            .get_wave(&task.wave_id)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
            .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
        super::planning_peer::sync_after_save(&store, owner.repo()).await;
    }
    let refresh_error = if message.is_none() && task.plan.linear_id.is_some() {
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            super::linear_observe::refresh_task_comments(&store, &task),
        )
        .await
        {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some("Comment refresh timed out; showing saved comments".into()),
        }
    } else {
        None
    };
    let mut thread = store
        .sqlite
        .task_comments(&task.id)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    thread.refresh_error = refresh_error;
    Ok(thread)
}

pub(crate) async fn resolve_saved_task(
    repo: &Path,
    wave: Option<&str>,
    issue: &str,
) -> OpsResult<(Arc<Store>, crate::work::task::Task)> {
    let store = Arc::new(pm_store().await?);
    let canonical = crate::repository::CanonicalRepo::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let task = super::planning_peer::find_task(&store, &canonical.to_string(), issue).await?;
    let task = match task {
        Some(task) => task,
        None => {
            // Cold acquisition imports the remote record once. The stored Task owns
            // every subsequent read and write, including while Linear is unavailable.
            let resolved = resolve_owned_issue(repo, issue).await?;
            store
                .get_task_by_issue(&resolved.item.id)
                .await
                .map_err(|error| OpsError::Message(error.to_string()))?
                .ok_or_else(|| OpsError::Message(format!("Task {issue} is not stored")))?
        }
    };
    let owner = store
        .get_wave(&task.wave_id)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message("Task Wave is missing".into()))?;
    let expected_wave = match wave {
        Some(selector) => Some(
            crate::work::wave::context::resolve_managed_wave(
                Some(&store),
                Some(repo),
                Some(selector),
                None,
            )
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?,
        ),
        None => None,
    };
    if owner.repo() != canonical.to_string()
        || expected_wave
            .as_ref()
            .is_some_and(|expected| expected.id() != owner.id())
    {
        return Err(OpsError::Message(format!(
            "Task {issue} belongs to {} in {}",
            owner.slug(),
            owner.repo()
        )));
    }
    Ok((store, task))
}

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
pub(crate) async fn pm_link_pr_async(
    repo: &Path,
    wave: &str,
    request: &PrLinkRequest,
    prior: &PrLinkageIds,
) -> PrLinkageOutcome {
    let ctx = match resolve_context(repo, wave).await {
        Ok(ctx) => ctx,
        Err(error) => {
            return PrLinkageOutcome {
                ids: prior.clone(),
                error: Some(error.to_string()),
            }
        }
    };
    match resolve_owned_issue(repo, &request.issue_id).await {
        Ok(ResolvedTask {
            wave: owning_wave, ..
        }) if owning_wave == wave => {}
        Ok(ResolvedTask {
            wave: owning_wave, ..
        }) => {
            return PrLinkageOutcome {
                ids: prior.clone(),
                error: Some(format!(
                    "Linear issue {} belongs to wave/{owning_wave}, not wave/{wave}",
                    request.issue_id
                )),
            }
        }
        Err(error) => {
            return PrLinkageOutcome {
                ids: prior.clone(),
                error: Some(error.to_string()),
            }
        }
    }
    link_pr_with_client(&ctx.client, request, prior).await
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

pub fn list_pm_waves(repo: &Path) -> OpsResult<Vec<String>> {
    Ok(list_local_waves(repo)?
        .into_iter()
        .filter(|wave| wave_has_pm_initiative(repo, wave))
        .collect())
}

/// Read planning under the existing managed freshness policy.
pub(crate) async fn read_task_planning_async(
    repo: &Path,
    issue: &str,
    refresh: PmRefresh,
) -> OpsResult<PmTaskRecord> {
    let read = inspect_task_planning_async(repo, issue, refresh).await?;
    if read.observation.state == PlanningState::Available {
        if let Some(record) = read.observation.record {
            return Ok(record);
        }
    }
    Err(OpsError::Message(read.refresh_error.unwrap_or_else(|| {
        format!(
            "Task {issue:?} planning is {:?}; refresh planning",
            read.observation.state
        )
    })))
}

#[derive(Debug)]
pub(crate) struct TaskPlanningInspection {
    pub observation: PmTaskObservation,
    pub refresh_error: Option<String>,
}

impl TaskPlanningInspection {
    fn age(&self) -> Option<i64> {
        self.observation
            .record
            .as_ref()
            .map(|record| time::OffsetDateTime::now_utc().unix_timestamp() - record.observed_at)
    }

    pub(crate) fn is_stale(&self) -> bool {
        self.observation.state != PlanningState::Available
            || self.age().is_some_and(|age| age >= PM_SOFT_STALE_SECS)
    }
}

pub(crate) fn inspect_task_planning(
    repo: &Path,
    issue: &str,
    refresh: PmRefresh,
) -> OpsResult<TaskPlanningInspection> {
    block_on_pm(inspect_task_planning_async(repo, issue, refresh))
}

async fn inspect_task_planning_async(
    repo: &Path,
    issue: &str,
    refresh: PmRefresh,
) -> OpsResult<TaskPlanningInspection> {
    let scope = crate::repository::CanonicalRepo::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?
        .to_string();
    require_linear_config(repo)?;
    let store = pm_store().await?;
    let existing = store
        .pm_task_observation(&scope, "linear", issue)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let age = existing
        .record
        .as_ref()
        .map(|record| now - record.observed_at);
    if existing.state == PlanningState::Removed
        || refresh == PmRefresh::Never
        || (existing.state == PlanningState::Available
            && matches!(plan_snapshot_read(refresh, age), SnapshotPlan::ServeCache))
    {
        return Ok(TaskPlanningInspection {
            observation: existing,
            refresh_error: None,
        });
    }
    let selector = existing
        .record
        .as_ref()
        .map_or(issue, |record| record.item.id.as_str());
    let fetch = async {
        let repository = resolve_repository_context(repo).await?;
        let mut observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut observation = repository
            .client
            .issue_ownership(selector)
            .await
            .map_err(pm_to_ops)?;
        let discovered_project = observation
            .as_ref()
            .and_then(|(_, project)| project.as_ref())
            .or_else(|| {
                existing
                    .record
                    .as_ref()
                    .and_then(|record| record.project.as_ref())
            });
        let mut acquisition = None;
        let mut confirmed_wave = None;
        if let Some(project) = discovered_project {
            let initiative = singular_project_initiative(project)?;
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
                .map_err(pm_to_ops)?;
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
            if let Some(expected) = &existing.record {
                store
                    .invalidate_pm_task(&scope, "linear", expected.clone(), acquisition)
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
        store
            .put_pm_task(
                &scope,
                "linear",
                PmTaskRecord {
                    item,
                    project,
                    observed_at,
                },
                confirmed_wave,
                acquisition,
            )
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?;
        Ok(true)
    };
    let result = match tokio::time::timeout(PM_REFRESH_TIMEOUT, fetch).await {
        Ok(result) => result,
        Err(_) => Err(OpsError::Message(format!(
            "Task planning refresh exceeded its {}s deadline",
            PM_REFRESH_TIMEOUT.as_secs()
        ))),
    };
    // Read again: an event may have invalidated the record while acquisition ran.
    let mut observation = store
        .pm_task_observation(&scope, "linear", selector)
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

pub fn pm_resolve_task(repo: &Path, issue: &str) -> OpsResult<PmResolvedTask> {
    block_on_pm(pm_resolve_task_async(repo, issue))
}

pub(crate) async fn pm_resolve_task_async(repo: &Path, issue: &str) -> OpsResult<PmResolvedTask> {
    let ResolvedTask {
        wave,
        item,
        project,
        ..
    } = resolve_owned_issue(repo, issue).await?;
    let initiative_id = singular_project_initiative(&project)?;
    Ok(PmResolvedTask {
        wave,
        initiative_id,
        project,
        item,
    })
}

async fn resolve_owned_issue(repo: &Path, issue: &str) -> OpsResult<ResolvedTask> {
    let task = pm_store()
        .await?
        .get_task_by_issue(issue)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let issue = task
        .as_ref()
        .map(|task| task.plan.linear_id())
        .transpose()?
        .map_or(issue, |id| id.as_str());
    crate::ops::task_pm::resolve_task_async(repo, issue, PmRefresh::Force).await
}

pub(crate) fn singular_project_initiative(project: &PmProject) -> OpsResult<String> {
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
    require_linear_config(repo)?;
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
    repository: RepositoryPmContext,
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
    require_linear_config(repo)?;
    let team_id = read_repository_team(repo)?.ok_or_else(|| {
        OpsError::Message(
            ".lf/config.yaml has no repository `pm.linear_team`. \
             Run `lf repo connect <wave> --team-key <KEY>` to establish the migration target."
                .to_string(),
        )
    })?;
    let repo_id = repository_id(repo)?;
    let client = build_client(Some(team_id.clone())).await?;
    let binding = client
        .validate_team_claim(&team_id, repo_id.as_str())
        .await
        .map_err(pm_to_ops)?;
    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open task registry: {err}")))?;

    Ok(ResolvedReteamContext {
        repository: RepositoryPmContext {
            client,
            repo_id,
            team_id,
        },
        team_key: binding.key,
        store,
    })
}

pub fn pm_reteam(
    repo: &Path,
    options: &PmReteamOptions,
    progress: &impl Progress,
) -> OpsResult<PmReteamResult> {
    block_on_pm(pm_reteam_async(repo, options, progress))
}

async fn pm_reteam_async(
    repo: &Path,
    options: &PmReteamOptions,
    progress: &impl Progress,
) -> OpsResult<PmReteamResult> {
    let resolved = resolve_reteam_context(repo).await?;
    apply_or_plan_repository_reteam(&resolved, repo, options.apply, progress).await
}

async fn accept_reteam_project(
    resolved: &ResolvedReteamContext,
    wave: &crate::work::wave::Wave,
    project: &PmProject,
    acquisition: Arc<crate::store::PlanningLocks>,
    expected_teams: &[String],
) -> OpsResult<()> {
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let confirmed = resolved
        .repository
        .client
        .project_ownership(&project.id)
        .await
        .map_err(pm_to_ops)?;
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
    resolved
        .store
        .reconcile_pm_project_teams(
            wave.id(),
            "linear",
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
        .map_err(pm_to_ops)?
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
    resolved
        .store
        .put_pm_task(
            wave.repo(),
            "linear",
            PmTaskRecord {
                item,
                project: Some(project),
                observed_at,
            },
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
) -> OpsResult<PmReteamResult> {
    let team_id = &resolved.repository.team_id;
    let team_key = &resolved.team_key;
    let store = &resolved.store;
    let waves = list_pm_waves(repo)?;
    if waves.is_empty() {
        return Err(OpsError::Message(
            "repository has no Waves linked to Linear Initiatives".to_string(),
        ));
    }
    if apply && repo.join(".git").exists() && !crate::engine::git::is_clean(repo)? {
        return Err(OpsError::Message(
            "`lf repo reteam --apply` requires a clean Git checkout so its repository PM config and Wave bindings can commit atomically; commit or stash existing changes, then rerun the dry-run"
                .to_string(),
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
                "Linear Initiative {initiative} is bound by both wave/{owner} and wave/{wave}; repair GOAL.md ownership before reteam"
            )));
        }
        progress.status(&format!("preflighting wave/{wave} Initiative {initiative}"));
        let wave_projects = resolved
            .repository
            .client
            .list_projects(&initiative)
            .await
            .map_err(pm_to_ops)?;
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
                project_moves.push(PmReteamProjectMove {
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
                .map_err(pm_to_ops)?;
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
                    moves.push(PmReteamMove {
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
                    .map_err(pm_to_ops)?;
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
                .map_err(pm_to_ops)?
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
                    .map_err(pm_to_ops)?;
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
                .map_err(pm_to_ops)?;
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
                    .map_err(pm_to_ops)?;
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

        // Re-fetch and validate the complete repository before deleting any
        // migration sentinel. A crash before cleanup remains loudly resumable.
        for wave in &waves {
            let initiative =
                read_initiative(repo, wave).expect("preflight required every Initiative");
            let ctx = PmContext {
                repository: resolved.repository.clone(),
                initiative,
            };
            let (registered, acquisition) = &locked_waves[wave];
            refresh_pm_snapshot_locked(repo, registered, &ctx, store, acquisition.clone()).await?;
        }
        remove_legacy_pm_sentinels(repo, &waves)?;
        if repo.join(".git").exists() {
            let _ = crate::ops::commit_workflow(
                repo,
                &crate::ops::CommitOptions {
                    add: true,
                    message: Some(
                        "lf repo reteam: migrate repository to one Linear Team".to_string(),
                    ),
                    ..crate::ops::CommitOptions::for_task("pm")
                },
                progress,
                &|_| {},
            )?;
        }
    }

    Ok(PmReteamResult {
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

pub fn pm_sync(
    repo: &Path,
    options: &PmSyncOptions,
    progress: &impl Progress,
) -> OpsResult<PmSyncResult> {
    block_on_pm(pm_sync_async(repo, options, progress))
}

async fn pm_sync_async(
    repo: &Path,
    options: &PmSyncOptions,
    progress: &impl Progress,
) -> OpsResult<PmSyncResult> {
    let all_waves = list_local_waves(repo)?;
    let waves = match options.wave.as_deref() {
        Some(wave) => vec![resolve_wave(Some(wave))?],
        None => all_waves.clone(),
    };
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut blocking = Vec::new();
    require_linear_config(repo)?;
    let team_id = read_repository_team(repo)?;

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
                    .join("wave")
                    .join(wave.slug())
                    .join("GOAL.md")
                    .is_file()
            {
                diagnostics.push(format!(
                    "prepared child wave/{} has no GOAL.md; resume or abandon its promotion",
                    wave.slug()
                ));
            }
        }
    }

    for sentinel in legacy_pm_sentinels(repo)? {
        diagnostics.push(format!(
            "legacy PM authority remains at {sentinel}; run repository-wide `lf repo reteam`"
        ));
    }
    if !options.plan {
        require_repository_pm_ready(repo)?;
    }
    if team_id.is_none() {
        let message = ".lf/config.yaml has no repository `pm.linear_team`; run `lf repo connect <wave> --team-key <KEY>`".to_string();
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
            .map_err(pm_to_ops)?;
    }
    progress.status("checking Linear repository Initiatives, Projects, and Tasks");
    let linear_waves = client.list_waves().await.map_err(pm_to_ops)?;
    let linear_waves_by_id: BTreeMap<String, String> = linear_waves
        .iter()
        .map(|wave| (wave.id.clone(), wave.name.clone()))
        .collect();
    for linear_wave in &linear_waves {
        if !initiative_waves.contains_key(&linear_wave.id) {
            diagnostics.push(format!(
                "Linear Initiative `{}` ({}) is not linked by any local wave",
                linear_wave.name, linear_wave.id
            ));
        }
    }

    let mut seen_projects: BTreeMap<String, String> = BTreeMap::new();
    for wave in &waves {
        let Some(initiative_id) = read_initiative(repo, wave) else {
            blocking.push(format!("wave/{wave} has no Linear Initiative"));
            continue;
        };
        let expected_initiative_name = title_case(wave);
        match linear_waves_by_id.get(&initiative_id) {
            Some(actual) if actual != &expected_initiative_name => {
                let message = format!(
                    "rename Linear Initiative `{actual}` ({initiative_id}) to `{expected_initiative_name}` for wave/{wave}"
                );
                actions.push(message);
            }
            None => {
                let message =
                    format!("wave/{wave} points at missing Linear Initiative {initiative_id}");
                diagnostics.push(message.clone());
                blocking.push(message);
                continue;
            }
            _ => {}
        }

        let projects = client
            .list_projects(&initiative_id)
            .await
            .map_err(pm_to_ops)?;
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
                        "Linear Project `{}` ({}) in wave/{wave} belongs to Teams [{}]; expected exactly repository Team {team_id}. Run `lf repo reteam`.",
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

            let items = client.list_items(&project.id).await.map_err(pm_to_ops)?;
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
                            "Linear task {} belongs to Team {}, expected repository Team {team_id}. Run `lf repo reteam`.",
                            item.identifier, item.team_id.as_deref().unwrap_or("unmapped")
                        );
                        diagnostics.push(message.clone());
                        blocking.push(message);
                    }
                }
            }
        }
        actions.push(format!(
            "refresh wave/{wave} PM snapshot from Linear Initiative {initiative_id}"
        ));
    }

    if !options.plan && !blocking.is_empty() {
        return Err(OpsError::Message(format!(
            "PM ownership validation failed before mutation: {}",
            blocking.join("; ")
        )));
    }

    if !options.plan {
        let team_id = team_id.expect("non-plan sync requires repository Team");
        for wave in &waves {
            let initiative =
                read_initiative(repo, wave).expect("preflight required every selected Initiative");
            let expected_initiative_name = title_case(wave);
            if linear_waves_by_id.get(&initiative) != Some(&expected_initiative_name) {
                client
                    .rename_wave(&initiative, &expected_initiative_name)
                    .await
                    .map_err(pm_to_ops)?;
            }
            let ctx = PmContext {
                repository: RepositoryPmContext {
                    client: client.clone(),
                    repo_id: repo_id.clone(),
                    team_id: team_id.clone(),
                },
                initiative: initiative.clone(),
            };
            let store = pm_store().await?;
            super::chapter::adopt_legacy_projects(repo, &store, wave, &ctx, true).await?;
            refresh_pm_snapshot(repo, wave, &ctx).await?;
        }
    }

    Ok(PmSyncResult {
        actions,
        diagnostics,
    })
}

// ── explicit mutations ─────────────────────────────────────────────

pub(crate) async fn pm_rename(
    repo: &Path,
    options: &PmRenameOptions,
    progress: &impl Progress,
) -> OpsResult<PmRenameResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    progress.status(&format!(
        "renaming Linear Initiative {} to {}",
        ctx.initiative, options.title
    ));
    ctx.client
        .rename_wave(&ctx.initiative, &options.title)
        .await
        .map_err(pm_to_ops)?;
    progress.status(&format!("refreshing local PM snapshot for wave/{wave}"));
    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(PmRenameResult {
        wave,
        initiative: ctx.initiative,
        title: options.title.clone(),
    })
}

pub fn list_local_waves(repo: &Path) -> OpsResult<Vec<String>> {
    let canonical = crate::repository::CanonicalRepo::discover(repo)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let StorageConfig::Sqlite { path } = storage_config_from_env()?;
    match std::fs::metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(OpsError::Message(error.to_string())),
        Ok(_) => {}
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

fn write_initiative_to_goal(repo: &Path, wave: &str, initiative_id: &str) -> OpsResult<()> {
    update_wave_goal_config(repo, wave, |map| {
        let pm_key = serde_yaml_ng::Value::String("pm".to_string());
        let mut pm_map = map
            .get(&pm_key)
            .and_then(serde_yaml_ng::Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        pm_map.insert(
            serde_yaml_ng::Value::String("linear_initiative".to_string()),
            serde_yaml_ng::Value::String(initiative_id.to_string()),
        );
        map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));
        Ok(())
    })
    .map_err(OpsError::Message)
}

fn write_repository_pm_config(repo: &Path, team_id: &str) -> OpsResult<()> {
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
    let root_map = root.as_mapping_mut().ok_or_else(|| {
        OpsError::Message(format!(
            "repository config {} must be a YAML mapping",
            path.display()
        ))
    })?;
    let pm_key = serde_yaml_ng::Value::String("pm".to_string());
    let mut pm = root_map
        .get(&pm_key)
        .and_then(serde_yaml_ng::Value::as_mapping)
        .cloned()
        .unwrap_or_default();
    pm.insert(
        serde_yaml_ng::Value::String("provider".to_string()),
        serde_yaml_ng::Value::String("linear".to_string()),
    );
    pm.insert(
        serde_yaml_ng::Value::String("linear_team".to_string()),
        serde_yaml_ng::Value::String(team_id.to_string()),
    );
    root_map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm));
    std::fs::create_dir_all(path.parent().expect("config path has parent"))?;
    std::fs::write(
        &path,
        serde_yaml_ng::to_string(&root).map_err(|error| {
            OpsError::Message(format!("failed to encode {}: {error}", path.display()))
        })?,
    )?;
    Ok(())
}

fn remove_legacy_pm_sentinels(repo: &Path, waves: &[String]) -> OpsResult<()> {
    for wave in waves {
        update_wave_goal_config(repo, wave, |root| {
            let pm_key = serde_yaml_ng::Value::String("pm".to_string());
            let Some(mut pm) = root
                .get(&pm_key)
                .and_then(serde_yaml_ng::Value::as_mapping)
                .cloned()
            else {
                return Ok(());
            };
            pm.remove(serde_yaml_ng::Value::String("provider".to_string()));
            pm.remove(serde_yaml_ng::Value::String("linear_team".to_string()));
            if pm.is_empty() {
                root.remove(&pm_key);
            } else {
                root.insert(pm_key, serde_yaml_ng::Value::Mapping(pm));
            }
            Ok(())
        })
        .map_err(OpsError::Message)?;
    }

    let path = repo.join(".lf/config.yaml");
    let content = std::fs::read_to_string(&path)?;
    let mut root: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).map_err(|error| {
        OpsError::Message(format!(
            "invalid repository config {}: {error}",
            path.display()
        ))
    })?;
    let root_map = root.as_mapping_mut().ok_or_else(|| {
        OpsError::Message(format!(
            "repository config {} must be a YAML mapping",
            path.display()
        ))
    })?;
    let linear_key = serde_yaml_ng::Value::String("linear".to_string());
    if let Some(mut linear) = root_map
        .get(&linear_key)
        .and_then(serde_yaml_ng::Value::as_mapping)
        .cloned()
    {
        linear.remove(serde_yaml_ng::Value::String("team".to_string()));
        if linear.is_empty() {
            root_map.remove(&linear_key);
        } else {
            root_map.insert(linear_key, serde_yaml_ng::Value::Mapping(linear));
        }
    }
    std::fs::write(
        &path,
        serde_yaml_ng::to_string(&root).map_err(|error| {
            OpsError::Message(format!("failed to encode {}: {error}", path.display()))
        })?,
    )?;
    Ok(())
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

fn matching_wave_id(waves: &[PmWave], title: &str) -> OpsResult<Option<String>> {
    let matches: Vec<_> = waves.iter().filter(|wave| wave.name == title).collect();
    match matches.as_slice() {
        [] => Ok(None),
        [wave] => Ok(Some(wave.id.clone())),
        many => Err(OpsError::Message(format!(
            "multiple Linear Initiatives are named `{title}`: {}. Rename duplicates before running `lf repo connect`",
            many.iter()
                .map(|wave| wave.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn ensure_unique_project_slugs(projects: &[PmProject], wave: &str) -> OpsResult<()> {
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
    ctx: &PmContext,
    wave: &str,
) -> OpsResult<Vec<PmProject>> {
    let store = pm_store().await?;
    checked_projects_with_store(repo, ctx, wave, &store).await
}

pub(super) async fn checked_projects_with_store(
    repo: &Path,
    ctx: &PmContext,
    wave: &str,
    store: &Store,
) -> OpsResult<Vec<PmProject>> {
    let mut projects = ctx
        .client
        .list_projects(&ctx.initiative)
        .await
        .map_err(pm_to_ops)?;
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
            if !projects.iter().any(|project| {
                known
                    .plan
                    .linear_id
                    .as_ref()
                    .is_some_and(|id| project.id == id.as_str())
            }) {
                // An omitted membership cannot erase retained Project/Task history.
                projects.push(
                    ctx.client
                        .project_ownership(known.plan.linear_id()?.as_str())
                        .await
                        .map_err(pm_to_ops)?,
                );
            }
        }
    }

    for adopted in super::chapter::adopt_legacy_projects(repo, store, wave, ctx, false).await? {
        if let Some(project) = projects.iter_mut().find(|project| project.id == adopted.id) {
            *project = adopted;
        } else {
            projects.push(adopted);
        }
    }
    projects.retain(|project| !project_is_foreign(project, &ctx.team_id));
    for project in &projects {
        validate_project_ownership(project, wave, &ctx.initiative, &ctx.team_id)?;
    }
    ensure_unique_project_slugs(&projects, wave)?;
    Ok(projects)
}

pub(super) fn project_is_foreign(project: &PmProject, team_id: &str) -> bool {
    !project.team_ids.is_empty() && !project.team_ids.iter().any(|id| id == team_id)
}

fn validate_project_ownership(
    project: &PmProject,
    wave: &str,
    initiative_id: &str,
    team_id: &str,
) -> OpsResult<()> {
    crate::pm::validate_project_ownership(wave, initiative_id, Some(team_id), project).map_err(
        |error| {
            OpsError::Message(format!(
                "{error}. Repair the associations and run `lf repo refresh {wave}`."
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
    block_on_pm(canonical_wave_title_path_async(repo, wave))
}

async fn canonical_wave_title_path_async(repo: &Path, wave: &str) -> OpsResult<String> {
    let store = pm_store().await?;
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

    let main =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
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

fn block_on_pm<T>(future: impl Future<Output = OpsResult<T>>) -> OpsResult<T> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to create async runtime: {err}")))?;
    rt.block_on(future)
}

fn pm_to_ops(err: PmError) -> OpsError {
    OpsError::Message(err.to_string())
}

#[derive(Debug)]
pub(crate) struct ChapterSweep {
    pub candidates: Vec<(String, String, PmItem)>,
    pub skipped_projects: Vec<(String, PmProject)>,
}

/// Read every linked Initiative, including archived predecessor Projects.
pub(crate) async fn chapter_sweep_candidates(repo: &Path) -> OpsResult<ChapterSweep> {
    let store = pm_store().await?;
    let mut candidates = Vec::new();
    let mut skipped_projects = Vec::new();
    let mut skipped_ids = BTreeSet::new();
    for name in list_pm_waves(repo)? {
        let ctx = resolve_context(repo, &name).await?;
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
            .list_projects_including_archived(&ctx.initiative, true)
            .await
            .map_err(|error| OpsError::Message(error.to_string()))?
        {
            if project_is_foreign(&project, &ctx.team_id) {
                if skipped_ids.insert(project.id.clone()) {
                    skipped_projects.push((name.clone(), project));
                }
                continue;
            }
            validate_project_ownership(&project, &name, &ctx.initiative, &ctx.team_id)?;
            if project.id == chapter.id {
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

pub(crate) async fn require_outside_current_chapter(
    repo: &Path,
    issue: &str,
) -> OpsResult<PmResolvedTask> {
    let resolved = pm_resolve_task_async(repo, issue).await?;
    let store = pm_store().await?;
    let locator = crate::work::wave::WaveLocator::discover(repo, &resolved.wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let wave = store
        .get_wave_at(&locator)
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?
        .ok_or_else(|| OpsError::Message("current Wave is unavailable".into()))?;
    let chapter = super::project::current_project(&store, &wave)?;
    if resolved.item.project_id.as_deref() == Some(chapter.id.as_str())
        || resolved.item.completed
        || matches!(
            resolved.item.state.as_deref(),
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
    use crate::ops::NullProgress;
    use crate::pm::test_server::{self, json_response, QueuedResponse};
    use axum::http::StatusCode;
    use serde_json::{json, Value};

    fn linear_test_ctx(base_url: String, initiative: &str) -> PmContext {
        PmContext {
            repository: RepositoryPmContext {
                client: crate::pm::linear::LinearClient::with_base_url(
                    "linear-secret".to_string(),
                    Some("team-123".to_string()),
                    base_url,
                ),
                repo_id: RepoId::parse("loopflowstudio/loopflow").unwrap(),
                team_id: "team-123".to_string(),
            },
            initiative: initiative.to_string(),
        }
    }

    async fn isolated_pm_store(repo: &Path) -> Store {
        crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            repo.join("registry.db"),
        ))
        .await
        .expect("open isolated PM store")
    }

    fn write_goal(repo: &Path, wave: &str, frontmatter: &str) {
        let dir = repo.join("wave").join(wave);
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
            "content": "## Definition\n\nA measured bet.\n\n## Flows\n\n- first: (none)\n- loop: (none)\n- finally: (none)\n\n## KRs\n\n- [ ] Ownership holds",
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
    fn repository_team_config_is_the_only_normal_authority() {
        with_pm_home(|repo, runtime| {
            write_repo_config(
                repo.path(),
                "pm:\n  provider: linear\n  linear_team: team-loo\n",
            );
            write_goal(
                repo.path(),
                "product",
                "pm:\n  linear_initiative: initiative-product\n",
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
                read_repository_team(repo.path()).unwrap().as_deref(),
                Some("team-loo")
            );
            assert_eq!(
                read_initiative(repo.path(), "product").as_deref(),
                Some("initiative-product")
            );
            assert!(legacy_pm_sentinels(repo.path()).unwrap().is_empty());
        });
    }

    #[test]
    fn legacy_wave_team_authority_blocks_mutations_with_prd_44_recovery() {
        with_pm_home(|repo, runtime| {
            write_repo_config(
                repo.path(),
                "pm:\n  provider: linear\n  linear_team: team-loo\nlinear:\n  team: team-old\n",
            );
            write_goal(
                repo.path(),
                "product",
                "pm:\n  provider: linear\n  linear_initiative: initiative-product\n  linear_team: team-old\n",
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
            let error = require_repository_pm_ready(repo.path()).unwrap_err();
            assert!(error.to_string().contains("lf repo reteam --apply"));
            assert!(error.to_string().contains("PRD-44"));
        });
    }

    async fn run_reteam_fixture(
        resolved: &ResolvedReteamContext,
        repo: &Path,
        database: &Path,
    ) -> OpsResult<PmReteamResult> {
        PM_TEST_CONTEXT
            .scope(
                PmTestContext {
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
    fn repository_team_reteam_migrates_open_and_completed_issues_before_cleanup() {
        with_pm_home(|repo, runtime| {
            runtime.block_on(async {
                write_repo_config(
                    repo.path(),
                    "pm:\n  provider: linear\n  linear_team: team-loo\nlinear:\n  team: team-old\n",
                );
                write_goal(
                    repo.path(),
                    "survival",
                    "pm:\n  provider: linear\n  linear_initiative: initiative-survival\n  linear_team: team-old\n",
                );
                write_goal(
                    repo.path(),
                    "survival/infrastructure",
                    "pm:\n  provider: linear\n  linear_initiative: initiative-infrastructure\n  linear_team: team-old\n",
                );

                let database = repo.path().join("loopflow.db");
                let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                    database.clone(),
                ))
                .await
                .unwrap();
                crate::work::wave::ensure_wave_row(&store, repo.path(), "survival/infrastructure")
                    .await
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
                let client = crate::pm::linear::LinearClient::with_base_url(
                    "linear-secret".to_string(),
                    Some("team-loo".to_string()),
                    base_url,
                );
                let resolved = ResolvedReteamContext {
                    repository: RepositoryPmContext {
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
                assert_eq!(result.moves.len(), 2);
                let identifiers = result
                    .moves
                    .iter()
                    .filter_map(|item| item.new_identifier.as_deref())
                    .collect::<BTreeSet<_>>();
                assert_eq!(identifiers, BTreeSet::from(["LOO-1", "LOO-2"]));
                assert!(legacy_pm_sentinels(repo.path()).unwrap().is_empty());
                assert_eq!(
                    read_repository_team(repo.path()).unwrap().as_deref(),
                    Some("team-loo")
                );
                for wave in ["survival", "survival/infrastructure"] {
                    let pm = read_wave_pm_config(repo.path(), wave).unwrap();
                    assert!(pm.provider.is_none());
                    assert!(pm.linear_team.is_none());
                    assert!(pm.linear_initiative.is_some());
                    let locator = crate::work::wave::WaveLocator::discover(repo.path(), wave).unwrap();
                    let registered = resolved.store.get_wave_at(&locator).await.unwrap().unwrap();
                    let planning = resolved
                        .store
                        .pm_snapshot(registered.id())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(planning.snapshot.projects.len(), 1);
                    assert_eq!(planning.snapshot.projects[0].team_ids, ["team-loo"]);
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
                    "pm:\n  provider: linear\n  linear_team: team-loo\nlinear:\n  team: team-old\n",
                );
                write_goal(
                    repo.path(),
                    "survival",
                    "pm:\n  provider: linear\n  linear_initiative: initiative-survival\n  linear_team: team-old\n",
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
                    repository: RepositoryPmContext {
                        client: crate::pm::linear::LinearClient::with_base_url(
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
                        .put_pm_snapshot(
                            PmSnapshotRow {
                                wave_id: wave.id().clone(),
                                provider: "linear".into(),
                                initiative: "initiative-survival".into(),
                                synced_at: 1,
                                snapshot: PmSnapshot { projects, items },
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
                assert!(!legacy_pm_sentinels(repo.path()).unwrap().is_empty());

                let resumed = run_reteam_fixture(&resolved, repo.path(), &database)
                    .await
                    .unwrap();
                if interruption == "narrowing" {
                    assert!(resumed.moves.is_empty());
                } else {
                    assert_eq!(resumed.moves[0].new_identifier.as_deref(), Some("LOO-1"));
                }
                let planning = resolved
                    .store
                    .pm_snapshot(wave.id())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(planning.snapshot.projects[0].team_ids, ["team-loo"]);
                assert_eq!(planning.snapshot.items[0].identifier, "LOO-1");
                assert!(legacy_pm_sentinels(repo.path()).unwrap().is_empty());
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
        let project = |id: &str, name: &str| PmProject {
            revision: None,
            id: id.to_string(),
            slug: crate::pm::project_slug(name),
            name: name.to_string(),
            summary: String::new(),

            metric_targets: Vec::new(),
            workflow: "feature".into(),
            status: crate::pm::ProjectStatus::Started,
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
    async fn fetch_pm_snapshot_reads_projects_and_their_items() {
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
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let repo = tempfile::tempdir().unwrap();
        let store = isolated_pm_store(repo.path()).await;

        let result = fetch_pm_snapshot_with_store(repo.path(), "scan", &ctx, &store)
            .await
            .expect("fetch succeeds");
        assert_eq!(result.projects.len(), 1);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "First");
        assert_eq!(result.items[0].project.as_deref(), Some("scan"));
        assert_eq!(
            requests.lock().await[1].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
    }

    #[tokio::test]
    async fn fetch_pm_snapshot_preserves_moved_and_detached_issue_ownership() {
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
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let repo = tempfile::tempdir().unwrap();
        let store = isolated_pm_store(repo.path()).await;

        let snapshot = fetch_pm_snapshot_with_store(repo.path(), "scan", &ctx, &store)
            .await
            .unwrap();
        assert_eq!(snapshot.items[0].project_id.as_deref(), Some("successor"));
        assert_eq!(snapshot.items[0].project.as_deref(), Some("next-work"));
        assert_eq!(
            snapshot.items[0].revision.as_deref(),
            Some("2026-10-05T12:00:00Z")
        );
        assert_eq!(snapshot.items[1].project_id, None);
        assert_eq!(snapshot.items[1].project, None);
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
        let client = linear_test_ctx(base_url, "initiative-1").client.clone();

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
        let client = linear_test_ctx(base_url, "initiative-1").client.clone();
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
        let client = linear_test_ctx(base_url, "initiative-1").client.clone();

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
        let client = linear_test_ctx(base_url, "initiative-1").client.clone();

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

    #[test]
    fn plan_snapshot_read_covers_every_band() {
        use PmRefresh::{Auto, Force, Never};
        use SnapshotPlan::{Refresh, ServeCache};

        // Never never touches the network, at any age or with no snapshot.
        assert_eq!(plan_snapshot_read(Never, None), ServeCache);
        assert_eq!(
            plan_snapshot_read(Never, Some(10 * PM_HARD_STALE_SECS)),
            ServeCache
        );
        // Force always refreshes, and a failure is hard.
        assert_eq!(plan_snapshot_read(Force, Some(0)), Refresh { hard: true });
        assert_eq!(plan_snapshot_read(Force, None), Refresh { hard: true });
        // Auto: fresh serves cache; soft-stale refreshes with fallback; hard-stale
        // and a missing snapshot refresh hard.
        assert_eq!(plan_snapshot_read(Auto, Some(0)), ServeCache);
        assert_eq!(
            plan_snapshot_read(Auto, Some(PM_SOFT_STALE_SECS - 1)),
            ServeCache
        );
        assert_eq!(
            plan_snapshot_read(Auto, Some(PM_SOFT_STALE_SECS)),
            Refresh { hard: false }
        );
        assert_eq!(
            plan_snapshot_read(Auto, Some(PM_HARD_STALE_SECS - 1)),
            Refresh { hard: false }
        );
        assert_eq!(
            plan_snapshot_read(Auto, Some(PM_HARD_STALE_SECS)),
            Refresh { hard: true }
        );
        assert_eq!(plan_snapshot_read(Auto, None), Refresh { hard: true });
    }
}
