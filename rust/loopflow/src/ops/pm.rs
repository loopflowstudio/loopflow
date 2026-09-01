//! `lf pm` — read and write a wave's PM tasks directly in a provider.
//!
//! Linear is authoritative for the wave's project inventory, project specs, and
//! tasks. `lf pm sync` projects that state into SQLite; reads serve that
//! snapshot and only reach Linear through a bounded staleness policy (see
//! `load_show_snapshot`).

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::Path;

use futures_util::future::try_join_all;

use crate::engine::config::load_repo_config;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::normalize_wave_name;
use crate::pm::linear::LinearClient;
use crate::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmKr, PmPortfolioValidator, PmProject,
    PmProviderKind, PmSnapshot, PmWave, ProjectContent, ProjectFlowPlan,
};
use crate::provider_auth::{
    provider_token_refresh_due, refresh_stored_provider_token, Provider, TokenRefreshError,
};
use crate::repository::RepoId;
use crate::store::{open_existing_store, open_store, PmSnapshotRow, ProviderToken, Store};
use crate::work::wave::config::{read_wave_config, update_wave_goal_config, WavePmConfig};

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
    pub project: Option<String>,
    pub refresh: PmRefresh,
}

/// How `pm show` reconciles the local snapshot with Linear before reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PmRefresh {
    /// Refresh only when the snapshot is stale (the default TTL policy).
    #[default]
    Auto,
    /// Always refresh before reading (`--sync`).
    Force,
    /// Never touch the network; serve the cache as-is (`--no-sync`).
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PmShowResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub initiative: String,
    pub project: Option<String>,
    pub synced_at: i64,
    pub projects: Vec<PmProject>,
    pub items: Vec<PmItem>,
}

#[derive(Debug, Clone)]
pub struct PmUpdateOptions {
    pub wave: Option<String>,
    pub project: Option<String>,
    pub id: Option<String>,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
    /// PR URL to attach as a comment on the task (the loop's write-back link).
    pub pr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmUpdateResult {
    pub wave: String,
    pub id: String,
    pub created: bool,
    pub completed: bool,
    pub linked_pr: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PmStatusOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmWaveStatus {
    pub wave: String,
    pub initiative: String,
    pub initiative_name: String,
    pub open: usize,
    pub total: usize,
    pub open_by_project: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmStatusResult {
    pub waves: Vec<PmWaveStatus>,
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
    pub target_name: String,
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

#[derive(Debug, Clone)]
pub struct PmTaskMoveOptions {
    pub id: String,
    pub wave: Option<String>,
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmTaskMoveResult {
    pub wave: String,
    pub id: String,
    pub project: String,
}

#[derive(Debug, Clone)]
pub struct PmProjectWriteOptions {
    pub wave: Option<String>,
    pub project: Option<String>,
    pub title: Option<String>,
    pub definition: Option<String>,
    pub krs: Vec<String>,
    pub first: Option<String>,
    pub loop_: Option<String>,
    pub finally: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmProjectWriteResult {
    pub wave: String,
    pub id: String,
    pub slug: String,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub struct PmProjectArchiveOptions {
    pub wave: Option<String>,
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmProjectArchiveResult {
    pub wave: String,
    pub id: String,
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmResolvedTask {
    pub wave: String,
    pub initiative_id: String,
    pub project: PmProject,
    pub item: PmItem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmResolvedProject {
    pub wave: String,
    pub initiative_id: String,
    pub project: PmProject,
}

pub fn pm_create_project(
    repo: &Path,
    wave: Option<&str>,
    title: &str,
) -> OpsResult<PmResolvedProject> {
    block_on_pm(pm_create_project_async(repo, wave, title))
}

async fn pm_create_project_async(
    repo: &Path,
    wave: Option<&str>,
    title: &str,
) -> OpsResult<PmResolvedProject> {
    let wave = resolve_wave(wave)?;
    let ctx = resolve_context(repo, &wave).await?;
    let projects = checked_projects(repo, &ctx, &wave).await?;
    if let Some(project) = projects
        .into_iter()
        .find(|project| project.name.eq_ignore_ascii_case(title))
    {
        return Ok(PmResolvedProject {
            wave,
            initiative_id: ctx.initiative,
            project,
        });
    }
    let seed = LocalProject {
        slug: crate::pm::project_slug(title),
        name: title.to_string(),
        summary: title.to_string(),
        definition: title.to_string(),
        flows: ProjectFlowPlan::empty(),
        krs: Vec::new(),
    };
    let linear_name = linear_project_name(repo, &wave, &seed.name).await?;
    let id = match ctx
        .client
        .create_project(
            &ctx.initiative,
            &linear_name,
            &ProjectContent {
                definition: seed.definition.clone(),
                flows: seed.flows.clone(),
                krs: seed.krs.clone(),
            },
        )
        .await
    {
        Ok(id) => id,
        Err(create_error) => checked_projects(repo, &ctx, &wave)
            .await?
            .into_iter()
            .find(|project| project.name.eq_ignore_ascii_case(title))
            .map(|project| project.id)
            .ok_or_else(|| pm_to_ops(create_error))?,
    };
    Ok(PmResolvedProject {
        wave,
        initiative_id: ctx.initiative.clone(),
        project: PmProject {
            id,
            slug: seed.slug,
            name: seed.name,
            summary: seed.summary,
            definition: seed.definition,
            flows: Some(seed.flows),
            krs: seed.krs,
            initiative_ids: vec![ctx.initiative.clone()],
            // The create result is transient — the next sync resolves the
            // authoritative teams from Linear.
            team_ids: vec![ctx.team_id.clone()],
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalProject {
    slug: String,
    name: String,
    summary: String,
    definition: String,
    flows: ProjectFlowPlan,
    krs: Vec<PmKr>,
}

// ── Client + Linear project resolution ──────────────────────────────

#[derive(Clone)]
pub(crate) struct RepositoryPmContext {
    pub client: LinearClient,
    pub provider: PmProviderKind,
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
    read_wave_config(repo, wave).and_then(|config| config.pm)
}

fn resolve_wave(wave: Option<&str>) -> OpsResult<String> {
    wave.and_then(normalize_wave_name)
        .ok_or_else(|| OpsError::Message("cannot determine wave; pass --wave <name>".to_string()))
}

fn parse_provider(value: &str) -> OpsResult<PmProviderKind> {
    value.parse::<PmProviderKind>().map_err(pm_to_ops)
}

fn resolve_provider(repo: &Path) -> OpsResult<PmProviderKind> {
    let config = load_repo_config(repo)
        .map_err(|error| OpsError::Message(format!("failed to read .lf/config.yaml: {error}")))?
        .unwrap_or_default();
    if let Some(provider) = config
        .pm
        .as_ref()
        .and_then(|pm| pm.provider.as_deref())
        .filter(|provider| !provider.trim().is_empty())
    {
        return parse_provider(provider);
    }
    Ok(PmProviderKind::Linear)
}

fn read_initiative(repo: &Path, wave: &str, provider: PmProviderKind) -> Option<String> {
    let pm = read_wave_pm_config(repo, wave)?;
    let initiative = match provider {
        PmProviderKind::Linear => pm.linear_initiative,
    }?;
    Some(initiative).filter(|initiative| !initiative.trim().is_empty())
}

fn read_repository_team(repo: &Path, provider: PmProviderKind) -> OpsResult<Option<String>> {
    let config = load_repo_config(repo)
        .map_err(|error| OpsError::Message(format!("failed to read .lf/config.yaml: {error}")))?
        .unwrap_or_default();
    let team = match provider {
        PmProviderKind::Linear => config.pm.and_then(|pm| pm.linear_team),
    };
    Ok(team.filter(|team| !team.trim().is_empty()))
}

fn require_repository_team(repo: &Path, provider: PmProviderKind) -> OpsResult<String> {
    read_repository_team(repo, provider)?.ok_or_else(|| {
        OpsError::Message(
            ".lf/config.yaml has no repository `pm.linear_team`. \
             Run `lf pm init --wave <wave> --team-key <KEY>` before creating or mutating work."
                .to_string(),
        )
    })
}

/// Whether a wave has a Linear Initiative pinned for its resolved provider.
fn wave_has_pm_initiative(repo: &Path, wave: &str) -> bool {
    resolve_provider(repo)
        .ok()
        .is_some_and(|provider| read_initiative(repo, wave, provider).is_some())
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
         Run `lf pm reteam` to inspect the repository-wide plan, then `lf pm reteam --apply`. \
         Loopflow's live migration is owned by PRD-44.",
        sentinels.join(", ")
    )))
}

pub(crate) fn repository_team_id(repo: &Path) -> OpsResult<String> {
    require_repository_pm_ready(repo)?;
    let provider = resolve_provider(repo)?;
    require_repository_team(repo, provider)
}

/// Expected Team for strict cached reads after migration. During the deliberate
/// PRD-43/PRD-44 mixed state, legacy snapshots remain inspectable and validate
/// their own singular Team rather than pretending they already carry the new one.
pub(crate) fn repository_team_for_snapshot_validation(repo: &Path) -> OpsResult<Option<String>> {
    if !legacy_pm_sentinels(repo)?.is_empty() {
        return Ok(None);
    }
    let provider = resolve_provider(repo)?;
    read_repository_team(repo, provider)
}

async fn build_client(
    _repo: &Path,
    provider: PmProviderKind,
    team: Option<String>,
) -> OpsResult<LinearClient> {
    let token = resolve_pm_token(provider).await?;
    match provider {
        PmProviderKind::Linear => Ok(LinearClient::new(token, team)),
    }
}

fn repository_id(repo: &Path) -> OpsResult<RepoId> {
    RepoId::discover(repo).map_err(|error| {
        OpsError::Message(format!(
            "cannot establish repository PM identity from Git origin: {error}. \
             Configure an origin before running `lf pm init`."
        ))
    })
}

async fn resolve_repository_context(repo: &Path) -> OpsResult<RepositoryPmContext> {
    require_repository_pm_ready(repo)?;
    let provider = resolve_provider(repo)?;
    let team_id = require_repository_team(repo, provider)?;
    let repo_id = repository_id(repo)?;
    let client = build_client(repo, provider, Some(team_id.clone())).await?;
    client
        .validate_team_claim(&team_id, repo_id.as_str())
        .await
        .map_err(pm_to_ops)?;
    Ok(RepositoryPmContext {
        client,
        provider,
        repo_id,
        team_id: team_id.clone(),
    })
}

/// A configured Linear client for repository-scoped webhook and exact-Issue operations.
pub async fn linear_client(repo: &Path) -> OpsResult<LinearClient> {
    Ok(resolve_repository_context(repo).await?.client)
}

async fn resolve_context(repo: &Path, wave: &str) -> OpsResult<PmContext> {
    let repository = resolve_repository_context(repo).await?;
    let provider = repository.provider;
    let initiative = read_initiative(repo, wave, provider).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.{}`. \
             Run `lf pm init --wave {wave}` to connect its Linear Initiative.",
            provider.initiative_key()
        ))
    })?;
    Ok(PmContext {
        repository,
        initiative,
    })
}

/// Linear authenticates via OAuth: the access token and refresh grant live in
/// store, and PM access refreshes the grant before the access token expires.
async fn resolve_pm_token(provider: PmProviderKind) -> OpsResult<String> {
    // A forwarded token wins over the local store: `lf ssh` resolves the PM
    // credential on the caller's machine (where store lives) and hands it to the
    // remote through the environment. The remote store holds no PM credential, so
    // without this hook remote `lf pm` could never authenticate.
    if let Some(token) = forwarded_pm_token(provider) {
        return Ok(token);
    }

    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open credential store: {err}")))?;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    resolve_pm_token_from_store(provider, &store, now, |provider, token| async move {
        refresh_stored_provider_token(provider, &token).await
    })
    .await
}

async fn resolve_pm_token_from_store<F, Fut>(
    provider: PmProviderKind,
    store: &Store,
    now: i64,
    refresh: F,
) -> OpsResult<String>
where
    F: FnOnce(Provider, ProviderToken) -> Fut,
    Fut: Future<Output = Result<ProviderToken, TokenRefreshError>>,
{
    let auth_provider = match provider {
        PmProviderKind::Linear => Provider::Linear,
    };
    let token = store
        .get_provider_token(provider.as_str())
        .await
        .map_err(|err| OpsError::Message(format!("failed to load {provider} token: {err}")))?
        .ok_or_else(|| {
            OpsError::Message(format!(
                "No {provider} credential found. Run `lf auth {provider}`."
            ))
        })?;

    let expired = token.expires_at.is_some_and(|expires_at| expires_at <= now);
    if !provider_token_refresh_due(&token, now) {
        return Ok(token.access_token);
    }

    match refresh(auth_provider, token.clone()).await {
        Ok(refreshed) => {
            let access_token = refreshed.access_token.clone();
            store
                .upsert_provider_token(&refreshed)
                .await
                .map_err(|err| {
                    OpsError::Message(format!(
                        "failed to persist refreshed {provider} token: {err}"
                    ))
                })?;
            Ok(access_token)
        }
        Err(error) if !expired => {
            tracing::warn!(
                provider = %provider,
                error = %error,
                "proactive PM token refresh failed; using the current token"
            );
            Ok(token.access_token)
        }
        Err(error) => Err(OpsError::Message(format!(
            "Stored {provider} token expired and automatic refresh failed: {error}. \
             Run `lf auth {provider}` once to reconnect."
        ))),
    }
}

/// Env var carrying a PM access token forwarded by `lf ssh`.
pub(crate) const FORWARDED_PM_TOKEN_ENV: &str = "LF_FORWARDED_PM_TOKEN";
/// Env var naming the provider the forwarded token belongs to (e.g. `linear`).
pub(crate) const FORWARDED_PM_PROVIDER_ENV: &str = "LF_FORWARDED_PM_PROVIDER";

/// A forwarded PM token from the environment, if present and matching `provider`.
/// When `LF_FORWARDED_PM_PROVIDER` is set it must name `provider`; when it is
/// absent the token is accepted for whatever provider the wave resolves to.
fn forwarded_pm_token(provider: PmProviderKind) -> Option<String> {
    let token = std::env::var(FORWARDED_PM_TOKEN_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    match std::env::var(FORWARDED_PM_PROVIDER_ENV) {
        Ok(name) if !name.trim().is_empty() => {
            (name.trim().eq_ignore_ascii_case(provider.as_str())).then_some(token)
        }
        _ => Some(token),
    }
}

fn storage_config_from_env() -> OpsResult<crate::store::StorageConfig> {
    crate::store::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve credential store: {err}")))
}

async fn pm_store() -> OpsResult<Store> {
    open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open PM snapshot store: {err}")))
}

// ── snapshot freshness policy ────────────────────────────────────────

/// Past this age an Auto read opportunistically refreshes before serving.
const PM_SOFT_STALE_SECS: i64 = 60 * 60; // 1 hour
/// Past this age a failed refresh is an error, not a silent cache fallback.
const PM_HARD_STALE_SECS: i64 = 7 * 24 * 60 * 60; // 1 week
/// Ceiling on an opportunistic refresh; exceeding it counts as a failure.
const PM_REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

fn missing_snapshot_error(wave: &str) -> OpsError {
    OpsError::Message(format!(
        "wave/{wave} has no local PM snapshot. Run `lf pm sync --wave {wave}`."
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

fn decode_snapshot(wave: &str, payload: &str) -> OpsResult<PmSnapshot> {
    serde_json::from_str(payload).map_err(|err| {
        OpsError::Message(format!(
            "PM snapshot schema changed for wave/{wave}; run `lf pm sync`: {err}"
        ))
    })
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
            "Linear did not respond within {}s",
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
                    "Linear unreachable ({err}); showing cached snapshot from {} ago",
                    format_age(now - row.synced_at)
                ));
                Ok(row)
            }
            Some(_) => {
                let reason = if mode == PmRefresh::Force {
                    format!("could not refresh wave/{wave} from Linear: {err}")
                } else {
                    format!(
                        "wave/{wave} PM snapshot is over a week stale and Linear is unreachable: {err}"
                    )
                };
                Err(OpsError::Message(format!(
                    "{reason}. Reconnect or run `lf pm sync --wave {wave}`."
                )))
            }
            None => Err(missing_snapshot_error(wave)),
        },
    }
}

async fn fetch_pm_snapshot(repo: &Path, wave: &str, ctx: &PmContext) -> OpsResult<PmSnapshot> {
    let store = pm_store().await?;
    fetch_pm_snapshot_with_store(repo, wave, ctx, &store).await
}

async fn fetch_pm_snapshot_with_store(
    repo: &Path,
    wave: &str,
    ctx: &PmContext,
    store: &Store,
) -> OpsResult<PmSnapshot> {
    let projects = checked_projects_with_store(repo, ctx, wave, store).await?;
    fetch_pm_snapshot_for_projects(ctx, projects).await
}

async fn fetch_pm_snapshot_for_projects(
    ctx: &PmContext,
    projects: Vec<PmProject>,
) -> OpsResult<PmSnapshot> {
    let project_items = try_join_all(projects.iter().cloned().map(|project| async move {
        let mut items = ctx
            .client
            .list_items(&project.id)
            .await
            .map_err(pm_to_ops)?;
        for item in &mut items {
            item.project_id = project.id.clone();
            item.project = project.slug.clone();
        }
        Ok::<_, OpsError>(items)
    }))
    .await?;
    Ok(PmSnapshot {
        projects,
        items: project_items.into_iter().flatten().collect(),
    })
}

async fn store_pm_snapshot(
    repo: &Path,
    wave: &str,
    ctx: &PmContext,
    snapshot: &PmSnapshot,
) -> OpsResult<()> {
    let store = pm_store().await?;
    store_pm_snapshot_with_store(repo, wave, ctx, snapshot, &store).await
}

async fn store_pm_snapshot_with_store(
    repo: &Path,
    wave: &str,
    ctx: &PmContext,
    snapshot: &PmSnapshot,
    store: &Store,
) -> OpsResult<()> {
    let payload = serde_json::to_string(snapshot).map_err(|err| {
        OpsError::Message(format!(
            "failed to serialize PM snapshot for wave/{wave}: {err}"
        ))
    })?;
    let registered = crate::controller::wave::registry::ensure_wave_row(store, repo, wave)
        .await
        .map_err(|err| OpsError::Message(format!("failed to register PM Wave: {err}")))?;
    store
        .put_pm_snapshot(PmSnapshotRow {
            wave_id: registered.id().clone(),
            provider: ctx.provider.as_str().to_string(),
            initiative: ctx.initiative.clone(),
            synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            payload,
        })
        .await
        .map_err(|err| OpsError::Message(format!("failed to store PM snapshot: {err}")))
}

async fn refresh_pm_snapshot(repo: &Path, wave: &str, ctx: &PmContext) -> OpsResult<PmSnapshot> {
    let snapshot = fetch_pm_snapshot(repo, wave, ctx).await?;
    store_pm_snapshot(repo, wave, ctx, &snapshot).await?;
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
    let wave_dir = repo.join("wave").join(&wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{wave}/"
        )));
    }

    let provider = resolve_provider(repo)?;
    let repo_id = repository_id(repo)?;
    let existing_initiative = read_initiative(repo, &wave, provider);
    let existing_team = read_repository_team(repo, provider)?;

    let summary = wave_summary(repo, &wave)?;
    let title = title_case(&wave);
    let client = build_client(repo, provider, existing_team.clone()).await?;

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
            progress.status(&format!(
                "looking for {provider} Linear Initiative `{title}`"
            ));
            match matching_wave_id(&client.list_waves().await.map_err(pm_to_ops)?, &title)? {
                Some(id) => {
                    progress.status(&format!(
                        "linking wave/{wave} to existing {provider} Initiative {id}"
                    ));
                    (id, false)
                }
                None => {
                    progress.status(&format!("creating {provider} Initiative for wave/{wave}"));
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
        write_initiative_to_goal(repo, &wave, provider, &initiative_id)?;
    }
    if team_changed {
        write_repository_pm_config(repo, provider, &team.id)?;
    }

    if initiative_missing || team_changed {
        let _ = crate::ops::commit_workflow(
            repo,
            &crate::ops::CommitOptions {
                add: true,
                message: Some(format!("lf pm: connect {wave} to {provider}")),
                ..crate::ops::CommitOptions::for_task("pm")
            },
            progress,
        )?;
    }

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
    let snapshot = decode_snapshot(&wave, &row.payload)?;
    let projects = match options.project.as_deref() {
        Some(slug) => vec![find_project(&snapshot.projects, &wave, slug)?.clone()],
        None => snapshot.projects,
    };
    let slugs: BTreeSet<_> = projects
        .iter()
        .map(|project| project.slug.as_str())
        .collect();
    let items = snapshot
        .items
        .into_iter()
        .filter(|item| slugs.contains(item.project.as_str()))
        .collect();
    Ok(PmShowResult {
        wave,
        provider: row.provider.parse().map_err(pm_to_ops)?,
        initiative: row.initiative,
        project: options.project.clone(),
        synced_at: row.synced_at,
        projects,
        items,
    })
}

// ── update ──────────────────────────────────────────────────────────

pub fn pm_update(
    repo: &Path,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    block_on_pm(pm_update_async(repo, options, progress))
}

pub fn pm_create_task_idempotent(
    repo: &Path,
    wave: &str,
    project_slug: &str,
    title: &str,
    description: &str,
    marker: &str,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    block_on_pm(pm_create_task_idempotent_async(
        repo,
        wave,
        project_slug,
        title,
        description,
        marker,
        progress,
    ))
}

async fn pm_create_task_idempotent_async(
    repo: &Path,
    wave: &str,
    project_slug: &str,
    title: &str,
    description: &str,
    marker: &str,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let ctx = resolve_context(repo, wave).await?;
    let projects = checked_projects(repo, &ctx, wave).await?;
    let project = find_project(&projects, wave, project_slug)?;
    let find_existing = |items: Vec<PmItem>| {
        items
            .into_iter()
            .find(|item| item.description.contains(marker))
    };
    if let Some(existing) = find_existing(
        ctx.client
            .list_items(&project.id)
            .await
            .map_err(pm_to_ops)?,
    ) {
        return Ok(PmUpdateResult {
            wave: wave.to_string(),
            id: existing.id,
            created: false,
            completed: existing.completed,
            linked_pr: None,
        });
    }

    progress.status(&format!(
        "creating idempotent {} task in Linear Project {} for wave/{wave}",
        ctx.provider, project.id
    ));
    let item = PmItemCreate {
        name: title.to_string(),
        description: task_description_with_marker(description, marker),
    };
    match ctx.client.create_item(&project.id, &item).await {
        Ok(id) => Ok(PmUpdateResult {
            wave: wave.to_string(),
            id,
            created: true,
            completed: false,
            linked_pr: None,
        }),
        Err(create_error) => {
            // The request may have reached Linear before the transport failed.
            // Resolve the durable marker before reporting failure so a retry
            // cannot create a second issue.
            let items = ctx
                .client
                .list_items(&project.id)
                .await
                .map_err(pm_to_ops)?;
            if let Some(existing) = find_existing(items) {
                return Ok(PmUpdateResult {
                    wave: wave.to_string(),
                    id: existing.id,
                    created: false,
                    completed: existing.completed,
                    linked_pr: None,
                });
            }
            Err(pm_to_ops(create_error))
        }
    }
}

fn task_description_with_marker(description: &str, marker: &str) -> String {
    format!("{}\n\n{}", description.trim(), marker)
}

pub(crate) async fn pm_update_async(
    repo: &Path,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    if let Some(issue_id) = options.id.as_deref() {
        let (owning_wave, _, _, _) = resolve_owned_issue(repo, &ctx.repository, issue_id).await?;
        if owning_wave != wave {
            return Err(OpsError::Message(format!(
                "Linear task {issue_id} belongs to wave/{owning_wave}, not wave/{wave}"
            )));
        }
    }
    let store = pm_store().await?;
    let result = apply_update(
        repo,
        &wave,
        options.project.as_deref(),
        &ctx,
        options,
        progress,
        &store,
    )
    .await?;
    progress.status(&format!("refreshing local PM snapshot for wave/{wave}"));
    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(result)
}

async fn apply_update(
    repo: &Path,
    wave: &str,
    project_slug: Option<&str>,
    ctx: &PmContext,
    options: &PmUpdateOptions,
    progress: &impl Progress,
    store: &Store,
) -> OpsResult<PmUpdateResult> {
    let mark_done = parse_done_status(options.status.as_deref())?;
    let projects = checked_projects_with_store(repo, ctx, wave, store).await?;
    let project = project_slug
        .map(|slug| find_project(&projects, wave, slug))
        .transpose()?;

    let (id, created) = match options.id.as_ref() {
        Some(id) => {
            progress.status(&format!("updating {} task {id}", ctx.provider));
            ctx.client
                .update_item(
                    id,
                    &PmItemUpdate {
                        name: options.title.clone(),
                        description: options.notes.clone(),
                    },
                )
                .await
                .map_err(pm_to_ops)?;
            if let Some(project) = project {
                ctx.client
                    .move_item_to_project(id, &project.id)
                    .await
                    .map_err(pm_to_ops)?;
            }
            (id.clone(), false)
        }
        None => {
            let Some(title) = options.title.as_ref() else {
                return Err(OpsError::Message(
                    "`lf pm task create --title` is required".to_string(),
                ));
            };
            let Some(project) = project else {
                return Err(OpsError::Message(
                    "`lf pm task create --project <slug>` is required".to_string(),
                ));
            };
            progress.status(&format!(
                "creating {} task in Linear Project {} for wave/{wave}",
                ctx.provider, project.id
            ));
            let id = ctx
                .client
                .create_item(
                    &project.id,
                    &PmItemCreate {
                        name: title.clone(),
                        description: options.notes.clone().unwrap_or_default(),
                    },
                )
                .await
                .map_err(pm_to_ops)?;
            (id, true)
        }
    };

    // Transition the state before commenting so a rejected close never leaves a
    // "Shipped" comment on a still-open issue — the comment follows the state.
    if mark_done {
        ctx.client.complete_item(&id).await.map_err(pm_to_ops)?;
    }

    // Attach the PR link as a comment so the task carries a durable pointer to
    // the work without clobbering its description.
    let linked_pr = match options
        .pr
        .as_deref()
        .map(str::trim)
        .filter(|pr| !pr.is_empty())
    {
        Some(pr) => {
            progress.status(&format!("commenting PR link on {} task {id}", ctx.provider));
            let body = if mark_done {
                format!("Shipped: {pr}")
            } else {
                format!("PR: {pr}")
            };
            ctx.client.comment(&id, &body).await.map_err(pm_to_ops)?;
            Some(pr.to_string())
        }
        None => None,
    };

    Ok(PmUpdateResult {
        wave: wave.to_string(),
        id,
        created,
        completed: mark_done,
        linked_pr,
    })
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
    match resolve_owned_issue(repo, &ctx.repository, &request.issue_id).await {
        Ok((owning_wave, _, _, _)) if owning_wave == wave => {}
        Ok((owning_wave, _, _, _)) => {
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

/// `--status done` closes the task; absence leaves it open; anything else errors.
fn parse_done_status(status: Option<&str>) -> OpsResult<bool> {
    match status {
        None => Ok(false),
        Some(value)
            if value.eq_ignore_ascii_case("done")
                || value.eq_ignore_ascii_case("complete")
                || value.eq_ignore_ascii_case("completed") =>
        {
            Ok(true)
        }
        Some(other) => Err(OpsError::Message(format!(
            "unsupported task status {other:?}; only \"done\" is supported"
        ))),
    }
}

// ── status ──────────────────────────────────────────────────────────

pub fn pm_status(
    repo: &Path,
    options: &PmStatusOptions,
    progress: &impl Progress,
) -> OpsResult<PmStatusResult> {
    block_on_pm(pm_status_async(repo, options, progress))
}

async fn pm_status_async(
    repo: &Path,
    options: &PmStatusOptions,
    _progress: &impl Progress,
) -> OpsResult<PmStatusResult> {
    let waves = if let Some(wave) = options.wave.as_deref() {
        vec![resolve_wave(Some(wave))?]
    } else {
        list_pm_waves(repo)?
    };

    let expected_team = repository_team_for_snapshot_validation(repo)?;
    let mut ownership = PmPortfolioValidator::default();
    let mut results = Vec::new();
    for wave in waves {
        let row = read_pm_snapshot(repo, &wave).await?;
        let snapshot = decode_snapshot(&wave, &row.payload)?;
        ownership
            .validate(
                &wave,
                &row.initiative,
                expected_team.as_deref(),
                &snapshot.projects,
                &snapshot.items,
            )
            .map_err(pm_to_ops)?;
        let total = snapshot.items.len();
        let open = snapshot.items.iter().filter(|item| !item.completed).count();
        let mut open_by_project = BTreeMap::new();
        for project in snapshot.projects {
            let project_open = snapshot
                .items
                .iter()
                .filter(|item| !item.completed && item.project == project.slug)
                .count();
            open_by_project.insert(project.slug, project_open);
        }
        results.push(PmWaveStatus {
            initiative_name: title_case(&wave),
            wave,
            initiative: row.initiative,
            open,
            total,
            open_by_project,
        });
    }

    Ok(PmStatusResult { waves: results })
}

pub fn list_pm_waves(repo: &Path) -> OpsResult<Vec<String>> {
    Ok(list_local_waves(repo)?
        .into_iter()
        .filter(|wave| wave_has_pm_initiative(repo, wave))
        .collect())
}

pub fn pm_resolve_task(repo: &Path, issue: &str) -> OpsResult<PmResolvedTask> {
    block_on_pm(pm_resolve_task_async(repo, issue))
}

async fn pm_resolve_task_async(repo: &Path, issue: &str) -> OpsResult<PmResolvedTask> {
    let repository = resolve_repository_context(repo).await?;
    let (wave, initiative_id, mut item, mut project) =
        resolve_owned_issue(repo, &repository, issue).await?;
    let title_path = canonical_wave_title_path_async(repo, &wave).await?;
    project.name = canonical_project_name(&title_path, &wave, &project.name)?;
    project.slug = crate::pm::project_slug(&project.name);
    item.project_id = project.id.clone();
    item.project = project.slug.clone();
    Ok(PmResolvedTask {
        wave,
        initiative_id,
        project,
        item,
    })
}

async fn resolve_owned_issue(
    repo: &Path,
    repository: &RepositoryPmContext,
    issue: &str,
) -> OpsResult<(String, String, PmItem, PmProject)> {
    let (item, project) = repository
        .client
        .issue_ownership(issue)
        .await
        .map_err(pm_to_ops)?;
    if item.team_id != repository.team_id {
        return Err(OpsError::Message(format!(
            "Linear task {} belongs to Team {}, not repository {} Team {}",
            item.identifier, item.team_id, repository.repo_id, repository.team_id
        )));
    }
    let initiative_id = singular_project_initiative(&project)?;
    let wave = wave_for_initiative(repo, &initiative_id)?;
    validate_project_ownership(&project, &wave, &initiative_id, &repository.team_id)?;
    Ok((wave, initiative_id, item, project))
}

pub fn pm_resolve_project(repo: &Path, project_id: &str) -> OpsResult<PmResolvedProject> {
    block_on_pm(pm_resolve_project_async(repo, project_id))
}

async fn pm_resolve_project_async(repo: &Path, project_id: &str) -> OpsResult<PmResolvedProject> {
    let repository = resolve_repository_context(repo).await?;
    let mut project = repository
        .client
        .project_ownership(project_id)
        .await
        .map_err(pm_to_ops)?;
    let initiative_id = singular_project_initiative(&project)?;
    let wave = wave_for_initiative(repo, &initiative_id)?;
    validate_project_ownership(&project, &wave, &initiative_id, &repository.team_id)?;
    let title_path = canonical_wave_title_path_async(repo, &wave).await?;
    project.name = canonical_project_name(&title_path, &wave, &project.name)?;
    project.slug = crate::pm::project_slug(&project.name);
    Ok(PmResolvedProject {
        wave,
        initiative_id,
        project,
    })
}

fn singular_project_initiative(project: &PmProject) -> OpsResult<String> {
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

fn wave_for_initiative(repo: &Path, initiative_id: &str) -> OpsResult<String> {
    let provider = resolve_provider(repo)?;
    let matches = list_local_waves(repo)?
        .into_iter()
        .filter(|wave| read_initiative(repo, wave, provider).as_deref() == Some(initiative_id))
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

/// How repository-wide `reteam` treats one issue.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReteamClass {
    Already,
    Move,
}

fn classify_reteam_item(item: &PmItem, team_id: &str) -> ReteamClass {
    if item.team_id == team_id {
        return ReteamClass::Already;
    }
    ReteamClass::Move
}

fn project_needs_reteam(bound_team: &str, project_team_ids: &[String]) -> bool {
    project_team_ids.len() != 1 || project_team_ids[0] != bound_team
}

#[derive(Debug)]
struct ReteamIdentifierUpdate {
    issue_id: String,
    old_identifier: String,
    new_identifier: String,
}

struct ResolvedReteamContext {
    repository: RepositoryPmContext,
    team_key: String,
    store: Store,
}

#[derive(Debug, Clone)]
struct ReteamProjectState {
    project: PmProject,
    target_name: String,
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
    let provider = resolve_provider(repo)?;
    let team_id = read_repository_team(repo, provider)?.ok_or_else(|| {
        OpsError::Message(
            ".lf/config.yaml has no repository `pm.linear_team`. \
             Run `lf pm init --wave <wave> --team-key <KEY>` to establish the migration target."
                .to_string(),
        )
    })?;
    let repo_id = repository_id(repo)?;
    let client = build_client(repo, provider, Some(team_id.clone())).await?;
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
            provider,
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
    let mut project_moves = Vec::new();
    let mut moves = Vec::new();
    let mut identifier_updates = Vec::new();
    let mut states = Vec::new();
    let mut seen_initiatives = BTreeMap::new();
    let mut seen_projects = BTreeSet::new();
    let mut already = 0usize;
    let mut task_updates = 0usize;

    if apply && repo.join(".git").exists() && !crate::engine::git::is_clean(repo)? {
        return Err(OpsError::Message(
            "`lf pm reteam --apply` requires a clean Git checkout so its repository PM config and Wave bindings can commit atomically; commit or stash existing changes, then rerun the dry-run"
                .to_string(),
        ));
    }

    for wave in &waves {
        let initiative =
            read_initiative(repo, wave, resolved.repository.provider).ok_or_else(|| {
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
        let projects = resolved
            .repository
            .client
            .list_projects(&initiative)
            .await
            .map_err(pm_to_ops)?;
        let title_path = canonical_wave_title_path_with_store(repo, wave, store).await?;
        for project in projects {
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
            let canonical_name = canonical_project_name(&title_path, wave, &project.name)?;
            let target_name = format!("{title_path} — {canonical_name}");
            if project_needs_reteam(team_id, &project.team_ids) || project.name != target_name {
                project_moves.push(PmReteamProjectMove {
                    wave: wave.clone(),
                    id: project.id.clone(),
                    name: project.name.clone(),
                    target_name: target_name.clone(),
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
                if item.project_id != project.id {
                    return Err(OpsError::Message(format!(
                        "Linear task {} resolves to Project {}, expected {}",
                        item.identifier, item.project_id, project.id
                    )));
                }
                if !project.team_ids.iter().any(|team| team == &item.team_id) {
                    return Err(OpsError::Message(format!(
                        "Linear task {} belongs to Team {}, but Project {} carries teams [{}]",
                        item.identifier,
                        item.team_id,
                        project.id,
                        project.team_ids.join(", ")
                    )));
                }
                let registered_identifier =
                    store
                        .task_issue_identifier(&item.id)
                        .await
                        .map_err(|error| {
                            OpsError::Message(format!("failed to read task registry: {error}"))
                        })?;
                match classify_reteam_item(&item, team_id) {
                    ReteamClass::Already => {
                        already += 1;
                        if let Some(old_identifier) = registered_identifier
                            .filter(|identifier| identifier != &item.identifier)
                        {
                            identifier_updates.push(ReteamIdentifierUpdate {
                                issue_id: item.id,
                                old_identifier,
                                new_identifier: item.identifier,
                            });
                        }
                    }
                    ReteamClass::Move => moves.push(PmReteamMove {
                        wave: wave.clone(),
                        project_id: project.id.clone(),
                        id: item.id,
                        old_identifier: item.identifier,
                        title: item.name,
                        new_identifier: None,
                    }),
                }
            }
            states.push(ReteamProjectState {
                project,
                target_name,
            });
        }
    }

    if apply {
        // Linear requires the destination Team on a Project before its Issues
        // can move. Expand first; narrowing is the final provider phase.
        for state in &states {
            if !state.project.team_ids.iter().any(|team| team == team_id) {
                let mut teams = state.project.team_ids.clone();
                teams.push(team_id.clone());
                progress.status(&format!(
                    "attaching team {team_key} to Project `{}`",
                    state.project.name
                ));
                resolved
                    .repository
                    .client
                    .set_project_teams(&state.project.id, &teams)
                    .await
                    .map_err(pm_to_ops)?;
            }
        }

        for update in identifier_updates {
            task_updates += usize::from(
                store
                    .rebind_task_issue_identifier(
                        &update.issue_id,
                        &update.old_identifier,
                        &update.new_identifier,
                    )
                    .await
                    .map_err(|err| {
                        OpsError::Message(format!(
                            "failed to reconcile Task {}: {err}",
                            update.new_identifier
                        ))
                    })?,
            );
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
            task_updates += usize::from(
                store
                    .rebind_task_issue_identifier(&mv.id, &mv.old_identifier, &new_identifier)
                    .await
                    .map_err(|err| {
                        OpsError::Message(format!(
                            "moved {} to {new_identifier}, but failed to reconcile its Task: {err}",
                            mv.old_identifier
                        ))
                    })?,
            );
            mv.new_identifier = Some(new_identifier);
        }

        for state in &states {
            if project_needs_reteam(team_id, &state.project.team_ids) {
                progress.status(&format!(
                    "narrowing Project `{}` onto team {team_key}",
                    state.project.name
                ));
                resolved
                    .repository
                    .client
                    .move_project_to_team(&state.project.id, team_id)
                    .await
                    .map_err(pm_to_ops)?;
            }
            if state.project.name != state.target_name {
                let flows = state.project.flows.clone().ok_or_else(|| {
                    OpsError::Message(format!(
                        "Project {} has no flow payload; refresh before reteam",
                        state.project.id
                    ))
                })?;
                resolved
                    .repository
                    .client
                    .update_project(
                        &state.project.id,
                        &state.target_name,
                        &ProjectContent {
                            definition: state.project.definition.clone(),
                            flows,
                            krs: state.project.krs.clone(),
                        },
                    )
                    .await
                    .map_err(pm_to_ops)?;
            }
        }

        // Re-fetch and validate the complete repository before deleting any
        // migration sentinel. A crash before cleanup remains loudly resumable.
        for wave in &waves {
            let initiative = read_initiative(repo, wave, resolved.repository.provider)
                .expect("preflight required every Initiative");
            let ctx = PmContext {
                repository: resolved.repository.clone(),
                initiative,
            };
            let projects = checked_projects_with_store(repo, &ctx, wave, store).await?;
            let snapshot = fetch_pm_snapshot_for_projects(&ctx, projects).await?;
            store_pm_snapshot_with_store(repo, wave, &ctx, &snapshot, store).await?;
        }
        remove_legacy_pm_sentinels(repo, &waves)?;
        if repo.join(".git").exists() {
            let _ = crate::ops::commit_workflow(
                repo,
                &crate::ops::CommitOptions {
                    add: true,
                    message: Some("lf pm: migrate repository to one Linear Team".to_string()),
                    ..crate::ops::CommitOptions::for_task("pm")
                },
                progress,
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
    let provider = resolve_provider(repo)?;
    let team_id = read_repository_team(repo, provider)?;

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
                    .join(wave.name())
                    .join("GOAL.md")
                    .is_file()
            {
                diagnostics.push(format!(
                    "prepared child wave/{} has no GOAL.md; resume or abandon its promotion",
                    wave.name()
                ));
            }
        }
    }

    for sentinel in legacy_pm_sentinels(repo)? {
        diagnostics.push(format!(
            "legacy PM authority remains at {sentinel}; run repository-wide `lf pm reteam`"
        ));
    }
    if !options.plan {
        require_repository_pm_ready(repo)?;
    }
    if team_id.is_none() {
        let message = ".lf/config.yaml has no repository `pm.linear_team`; run `lf pm init --wave <wave> --team-key <KEY>`".to_string();
        diagnostics.push(message.clone());
        blocking.push(message);
    }

    let mut initiative_waves: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for wave in &all_waves {
        if let Some(initiative) = read_initiative(repo, wave, provider) {
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

    let client = build_client(repo, provider, team_id.clone()).await?;
    let repo_id = repository_id(repo)?;
    if let Some(team_id) = &team_id {
        client
            .validate_team_claim(team_id, repo_id.as_str())
            .await
            .map_err(pm_to_ops)?;
    }
    progress.status(&format!(
        "checking {provider} repository Initiatives, Projects, and Tasks"
    ));
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
        let Some(initiative_id) = read_initiative(repo, wave, provider) else {
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

        let title_path = canonical_wave_title_path_async(repo, wave).await?;
        let projects = client
            .list_projects(&initiative_id)
            .await
            .map_err(pm_to_ops)?;
        let mut slugs = BTreeMap::new();
        for project in projects {
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
                        "Linear Project `{}` ({}) in wave/{wave} belongs to Teams [{}]; expected exactly repository Team {team_id}. Run `lf pm reteam`.",
                        project.name,
                        project.id,
                        project.team_ids.join(", ")
                    );
                    diagnostics.push(message.clone());
                    blocking.push(message);
                }
            }
            let canonical_name = match canonical_project_name(&title_path, wave, &project.name) {
                Ok(name) => name,
                Err(error) => {
                    let message = error.to_string();
                    diagnostics.push(message.clone());
                    blocking.push(message);
                    continue;
                }
            };
            let slug = crate::pm::project_slug(&canonical_name);
            if let Some(existing) = slugs.insert(slug.clone(), canonical_name.clone()) {
                let message = format!(
                    "Linear Projects `{existing}` and `{canonical_name}` in wave/{wave} both derive slug `{slug}`"
                );
                diagnostics.push(message.clone());
                blocking.push(message);
            }
            let expected_project_name = format!("{title_path} — {canonical_name}");
            if project.name != expected_project_name {
                if project.flows.is_none() {
                    let message = format!(
                        "Linear Project `{}` ({}) has no flow payload, so its title cannot be repaired safely",
                        project.name, project.id
                    );
                    diagnostics.push(message.clone());
                    blocking.push(message);
                } else {
                    actions.push(format!(
                        "rename Linear Project `{}` ({}) to `{expected_project_name}`",
                        project.name, project.id
                    ));
                }
            }

            let items = client.list_items(&project.id).await.map_err(pm_to_ops)?;
            if items.iter().all(|item| item.completed) {
                diagnostics.push(format!(
                    "Linear Project `{canonical_name}` ({}) in wave/{wave} has no open tasks",
                    project.id
                ));
            }
            for item in items {
                if item.project_id != project.id {
                    let message = format!(
                        "Linear task {} resolves to Project {}, expected {}",
                        item.identifier, item.project_id, project.id
                    );
                    diagnostics.push(message.clone());
                    blocking.push(message);
                }
                if let Some(team_id) = &team_id {
                    if item.team_id != *team_id {
                        let message = format!(
                            "Linear task {} belongs to Team {}, expected repository Team {team_id}. Run `lf pm reteam`.",
                            item.identifier, item.team_id
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
            let initiative = read_initiative(repo, wave, provider)
                .expect("preflight required every selected Initiative");
            let expected_initiative_name = title_case(wave);
            if linear_waves_by_id.get(&initiative) != Some(&expected_initiative_name) {
                client
                    .rename_wave(&initiative, &expected_initiative_name)
                    .await
                    .map_err(pm_to_ops)?;
            }
            let title_path = canonical_wave_title_path_async(repo, wave).await?;
            for project in client.list_projects(&initiative).await.map_err(pm_to_ops)? {
                let canonical_name = canonical_project_name(&title_path, wave, &project.name)?;
                let expected_name = format!("{title_path} — {canonical_name}");
                if project.name != expected_name {
                    client
                        .update_project(
                            &project.id,
                            &expected_name,
                            &ProjectContent {
                                definition: project.definition.clone(),
                                flows: project.flows.clone().expect("preflight required flows"),
                                krs: project.krs.clone(),
                            },
                        )
                        .await
                        .map_err(pm_to_ops)?;
                }
            }
            let ctx = PmContext {
                repository: RepositoryPmContext {
                    client: client.clone(),
                    provider,
                    repo_id: repo_id.clone(),
                    team_id: team_id.clone(),
                },
                initiative,
            };
            let snapshot = fetch_pm_snapshot(repo, wave, &ctx).await?;
            store_pm_snapshot(repo, wave, &ctx, &snapshot).await?;
        }
    }

    Ok(PmSyncResult {
        actions,
        diagnostics,
    })
}

// ── explicit mutations ─────────────────────────────────────────────

pub fn pm_project_write(
    repo: &Path,
    options: &PmProjectWriteOptions,
    progress: &impl Progress,
) -> OpsResult<PmProjectWriteResult> {
    block_on_pm(pm_project_write_async(repo, options, progress))
}

pub fn pm_project_archive(
    repo: &Path,
    options: &PmProjectArchiveOptions,
    progress: &impl Progress,
) -> OpsResult<PmProjectArchiveResult> {
    block_on_pm(pm_project_archive_async(repo, options, progress))
}

async fn pm_project_archive_async(
    repo: &Path,
    options: &PmProjectArchiveOptions,
    progress: &impl Progress,
) -> OpsResult<PmProjectArchiveResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    let projects = checked_projects(repo, &ctx, &wave).await?;
    let project = find_project(&projects, &wave, &options.project)?;
    progress.status(&format!("archiving Linear Project `{}`", project.name));
    ctx.client
        .archive_project(&project.id)
        .await
        .map_err(pm_to_ops)?;
    let result = PmProjectArchiveResult {
        wave: wave.clone(),
        id: project.id.clone(),
        slug: project.slug.clone(),
    };
    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(result)
}

async fn pm_project_write_async(
    repo: &Path,
    options: &PmProjectWriteOptions,
    progress: &impl Progress,
) -> OpsResult<PmProjectWriteResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    // Creating a Project (no `--project` slug to update) must bind an explicit
    // team; updating an existing Project does not move it between teams.
    let ctx = resolve_context(repo, &wave).await?;
    let requested_krs = options
        .krs
        .iter()
        .map(|value| {
            let value = value.trim();
            let (holds, text) = value
                .strip_prefix("[x] ")
                .or_else(|| value.strip_prefix("[X] "))
                .map(|text| (true, text))
                .or_else(|| value.strip_prefix("[ ] ").map(|text| (false, text)))
                .unwrap_or((false, value));
            PmKr {
                text: text.trim().to_string(),
                holds,
            }
        })
        .filter(|kr| !kr.text.is_empty())
        .collect::<Vec<_>>();
    if options.project.is_none() && requested_krs.is_empty() {
        return Err(OpsError::Message(
            "at least one `--kr` is required".to_string(),
        ));
    }

    let projects = checked_projects(repo, &ctx, &wave).await?;
    let (id, slug, created) = if let Some(slug) = options.project.as_deref() {
        let project = find_project(&projects, &wave, slug)?;
        let name = options
            .title
            .clone()
            .unwrap_or_else(|| project.name.clone());
        let new_slug = crate::pm::project_slug(&name);
        if projects
            .iter()
            .any(|candidate| candidate.id != project.id && candidate.slug == new_slug)
        {
            return Err(OpsError::Message(format!(
                "wave/{wave} already has a Linear Project with slug `{new_slug}`"
            )));
        }
        progress.status(&format!("updating Linear Project `{}`", project.name));
        let linear_name = linear_project_name(repo, &wave, &name).await?;
        let content = ProjectContent {
            definition: options
                .definition
                .clone()
                .unwrap_or_else(|| project.definition.clone()),
            flows: ProjectFlowPlan {
                first: options
                    .first
                    .clone()
                    .or_else(|| project.flows.as_ref().and_then(|flows| flows.first.clone())),
                loop_: options
                    .loop_
                    .clone()
                    .or_else(|| project.flows.as_ref().and_then(|flows| flows.loop_.clone())),
                finally: options.finally.clone().or_else(|| {
                    project
                        .flows
                        .as_ref()
                        .and_then(|flows| flows.finally.clone())
                }),
            },
            krs: if options.krs.is_empty() {
                project.krs.clone()
            } else {
                requested_krs.clone()
            },
        };
        ctx.client
            .update_project(&project.id, &linear_name, &content)
            .await
            .map_err(pm_to_ops)?;
        (project.id.clone(), new_slug, false)
    } else {
        let name = options.title.clone().ok_or_else(|| {
            OpsError::Message("`lf pm project create --title` is required".to_string())
        })?;
        let slug = crate::pm::project_slug(&name);
        if projects.iter().any(|project| project.slug == slug) {
            return Err(OpsError::Message(format!(
                "wave/{wave} already has a Linear Project with slug `{slug}`; use `lf pm project update`"
            )));
        }
        progress.status(&format!("creating Linear Project `{name}`"));
        let linear_name = linear_project_name(repo, &wave, &name).await?;
        let content = ProjectContent {
            definition: options.definition.clone().ok_or_else(|| {
                OpsError::Message("`lf pm project create --definition` is required".to_string())
            })?,
            flows: ProjectFlowPlan {
                first: options.first.clone(),
                loop_: options.loop_.clone(),
                finally: options.finally.clone(),
            },
            krs: requested_krs.clone(),
        };
        let id = ctx
            .client
            .create_project(&ctx.initiative, &linear_name, &content)
            .await
            .map_err(pm_to_ops)?;
        (id, slug, true)
    };

    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(PmProjectWriteResult {
        wave,
        id,
        slug,
        created,
    })
}

pub fn pm_rename(
    repo: &Path,
    options: &PmRenameOptions,
    progress: &impl Progress,
) -> OpsResult<PmRenameResult> {
    block_on_pm(pm_rename_async(repo, options, progress))
}

async fn pm_rename_async(
    repo: &Path,
    options: &PmRenameOptions,
    progress: &impl Progress,
) -> OpsResult<PmRenameResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    progress.status(&format!(
        "renaming {} Linear Initiative {} to {}",
        ctx.provider, ctx.initiative, options.title
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

pub fn pm_task_move(
    repo: &Path,
    options: &PmTaskMoveOptions,
    progress: &impl Progress,
) -> OpsResult<PmTaskMoveResult> {
    block_on_pm(pm_task_move_async(repo, options, progress))
}

async fn pm_task_move_async(
    repo: &Path,
    options: &PmTaskMoveOptions,
    progress: &impl Progress,
) -> OpsResult<PmTaskMoveResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    let projects = checked_projects(repo, &ctx, &wave).await?;
    let project = find_project(&projects, &wave, &options.project)?;
    resolve_owned_issue(repo, &ctx.repository, &options.id).await?;
    progress.status(&format!(
        "moving {} task {} to wave/{wave} Linear Project {}",
        ctx.provider, options.id, project.id
    ));
    ctx.client
        .move_item_to_project(&options.id, &project.id)
        .await
        .map_err(pm_to_ops)?;
    progress.status(&format!("refreshing local PM snapshot for wave/{wave}"));
    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(PmTaskMoveResult {
        wave,
        id: options.id.clone(),
        project: options.project.clone(),
    })
}

pub fn list_local_waves(repo: &Path) -> OpsResult<Vec<String>> {
    let wave_dir = repo.join("wave");
    if !wave_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut waves = Vec::new();
    collect_local_waves(&wave_dir, &wave_dir, &mut waves)?;
    waves.sort();
    Ok(waves)
}

fn collect_local_waves(root: &Path, directory: &Path, waves: &mut Vec<String>) -> OpsResult<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if path.join("GOAL.md").is_file() {
            let relative = path.strip_prefix(root).expect("walk remains below wave/");
            let name = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            waves.push(name);
        }
        collect_local_waves(root, &path, waves)?;
    }
    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────

fn write_initiative_to_goal(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
    initiative_id: &str,
) -> OpsResult<()> {
    update_wave_goal_config(repo, wave, |map| {
        let pm_key = serde_yaml_ng::Value::String("pm".to_string());
        let mut pm_map = map
            .get(&pm_key)
            .and_then(serde_yaml_ng::Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        pm_map.insert(
            serde_yaml_ng::Value::String(provider.initiative_key().to_string()),
            serde_yaml_ng::Value::String(initiative_id.to_string()),
        );
        map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));
        Ok(())
    })
    .map_err(OpsError::Message)
}

fn write_repository_pm_config(
    repo: &Path,
    provider: PmProviderKind,
    team_id: &str,
) -> OpsResult<()> {
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
        serde_yaml_ng::Value::String(provider.as_str().to_string()),
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

fn wave_summary(repo: &Path, wave: &str) -> OpsResult<String> {
    Ok(crate::work::wave::config::read_wave_summary(repo, wave)?)
}

fn matching_wave_id(waves: &[PmWave], title: &str) -> OpsResult<Option<String>> {
    let matches: Vec<_> = waves.iter().filter(|wave| wave.name == title).collect();
    match matches.as_slice() {
        [] => Ok(None),
        [wave] => Ok(Some(wave.id.clone())),
        many => Err(OpsError::Message(format!(
            "multiple Linear Initiatives are named `{title}`: {}. Rename duplicates before running `lf pm init`",
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

/// List a Wave's Projects and enforce repository Team + singular Initiative ownership.
async fn checked_projects(repo: &Path, ctx: &PmContext, wave: &str) -> OpsResult<Vec<PmProject>> {
    let store = pm_store().await?;
    checked_projects_with_store(repo, ctx, wave, &store).await
}

async fn checked_projects_with_store(
    repo: &Path,
    ctx: &PmContext,
    wave: &str,
    store: &Store,
) -> OpsResult<Vec<PmProject>> {
    let title_path = canonical_wave_title_path_with_store(repo, wave, store).await?;
    let mut projects = ctx
        .client
        .list_projects(&ctx.initiative)
        .await
        .map_err(pm_to_ops)?;
    for project in &mut projects {
        validate_project_ownership(project, wave, &ctx.initiative, &ctx.team_id)?;
        project.name = canonical_project_name(&title_path, wave, &project.name)?;
        project.slug = crate::pm::project_slug(&project.name);
    }
    ensure_unique_project_slugs(&projects, wave)?;
    Ok(projects)
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
                "{error}. Repair the associations and run `lf pm sync --wave {wave}`."
            ))
        },
    )
}

fn find_project<'a>(projects: &'a [PmProject], wave: &str, slug: &str) -> OpsResult<&'a PmProject> {
    projects
        .iter()
        .find(|project| project.slug == slug)
        .ok_or_else(|| {
            OpsError::Message(format!(
                "wave/{wave} has no Linear Project with slug `{slug}`"
            ))
        })
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

async fn linear_project_name(repo: &Path, wave: &str, canonical_name: &str) -> OpsResult<String> {
    Ok(format!(
        "{} — {}",
        canonical_wave_title_path_async(repo, wave).await?,
        canonical_name.trim()
    ))
}

fn canonical_project_name(title_path: &str, wave: &str, linear_name: &str) -> OpsResult<String> {
    let expected = format!("{title_path} — ");
    if let Some(name) = linear_name.strip_prefix(&expected) {
        return Ok(name.trim().to_string());
    }
    let leaf = wave.rsplit('/').next().unwrap_or(wave);
    for legacy in [title_case(wave), title_case(leaf)] {
        let prefix = format!("{legacy} — ");
        if let Some(name) = linear_name.strip_prefix(&prefix) {
            return Ok(name.trim().to_string());
        }
    }
    if linear_name.contains(" — ") {
        return Err(OpsError::Message(format!(
            "Linear Project title {linear_name:?} has an unrecognized Wave prefix; \
             run `lf pm project update --wave {wave} --project <slug> --title <name>` explicitly"
        )));
    }
    Ok(linear_name.trim().to_string())
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
                current.name(),
                current.repo()
            )));
        }
        let leaf = current.name().rsplit('/').next().unwrap_or(current.name());
        segments.push(title_case(leaf));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::WaveId;
    use crate::ops::NullProgress;
    use crate::pm::test_server::{self, json_response, QueuedResponse};
    use crate::work::wave::Wave;
    use axum::http::StatusCode;
    use serde_json::{json, Value};

    #[test]
    fn idempotent_task_description_preserves_the_report() {
        let description = task_description_with_marker(
            "failure heading\n\nfull stack trace",
            "<!-- loopflow-task-start:abc -->",
        );

        assert_eq!(
            description,
            "failure heading\n\nfull stack trace\n\n<!-- loopflow-task-start:abc -->"
        );
    }

    fn linear_test_ctx(base_url: String, initiative: &str) -> PmContext {
        PmContext {
            repository: RepositoryPmContext {
                client: crate::pm::linear::LinearClient::with_base_url(
                    "linear-secret".to_string(),
                    Some("team-123".to_string()),
                    base_url,
                ),
                provider: PmProviderKind::Linear,
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

    async fn isolated_apply_update(
        repo: &Path,
        wave: &str,
        project: Option<&str>,
        ctx: &PmContext,
        options: &PmUpdateOptions,
    ) -> OpsResult<PmUpdateResult> {
        let store = isolated_pm_store(repo).await;
        apply_update(repo, wave, project, ctx, options, &NullProgress, &store).await
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
            "content": "## Definition\n\nA measured bet.\n\n## KRs\n",
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
            "prioritySortOrder": 0.0,
            "sortOrder": 0.0,
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
                "comments": { "nodes": nodes }
            } } }),
        )
    }

    fn project_update_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "projectUpdate": { "project": { "id": id } } } }),
        )
    }

    fn write_repo_config(repo: &Path, content: &str) {
        std::fs::create_dir_all(repo.join(".lf")).unwrap();
        std::fs::write(repo.join(".lf/config.yaml"), content).unwrap();
    }

    #[test]
    fn repository_team_config_is_the_only_normal_authority() {
        let repo = tempfile::tempdir().unwrap();
        write_repo_config(
            repo.path(),
            "pm:\n  provider: linear\n  linear_team: team-loo\n",
        );
        write_goal(
            repo.path(),
            "product",
            "pm:\n  linear_initiative: initiative-product\n",
        );

        assert_eq!(
            read_repository_team(repo.path(), PmProviderKind::Linear)
                .unwrap()
                .as_deref(),
            Some("team-loo")
        );
        assert_eq!(
            read_initiative(repo.path(), "product", PmProviderKind::Linear).as_deref(),
            Some("initiative-product")
        );
        assert!(legacy_pm_sentinels(repo.path()).unwrap().is_empty());
    }

    #[test]
    fn legacy_wave_team_authority_blocks_mutations_with_prd_44_recovery() {
        let repo = tempfile::tempdir().unwrap();
        write_repo_config(
            repo.path(),
            "pm:\n  provider: linear\n  linear_team: team-loo\nlinear:\n  team: team-old\n",
        );
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_initiative: initiative-product\n  linear_team: team-old\n",
        );

        let error = require_repository_pm_ready(repo.path()).unwrap_err();
        assert!(error.to_string().contains("lf pm reteam --apply"));
        assert!(error.to_string().contains("PRD-44"));
    }

    #[test]
    fn project_titles_strip_only_recognized_wave_paths() {
        assert_eq!(
            canonical_project_name("Survival", "survival", "Survival — A real task").unwrap(),
            "A real task"
        );
        assert_eq!(
            canonical_project_name(
                "Survival / Infrastructure",
                "infrastructure",
                "Infrastructure — Gmail",
            )
            .unwrap(),
            "Gmail"
        );
        assert!(canonical_project_name(
            "Survival / Infrastructure",
            "infrastructure",
            "Another Wave — Gmail",
        )
        .is_err());
        assert_eq!(default_team_key("loopflow"), "LOO");
    }

    #[tokio::test]
    async fn repository_team_reteam_migrates_open_and_completed_issues_before_cleanup() {
        let repo = tempfile::tempdir().unwrap();
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
            "infrastructure",
            "pm:\n  provider: linear\n  linear_initiative: initiative-infrastructure\n  linear_team: team-old\n",
        );

        let database = repo.path().join("registry.db");
        let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            database.clone(),
        ))
        .await
        .unwrap();
        let survival = Wave::new(
            WaveId::new(),
            "survival".to_string(),
            repo.path().display().to_string(),
        );
        let infrastructure = Wave::new(
            WaveId::new(),
            "infrastructure".to_string(),
            repo.path().display().to_string(),
        )
        .with_parent(survival.id().clone());
        store.create_wave(&survival).await.unwrap();
        store.create_wave(&infrastructure).await.unwrap();

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
            "Survival / Infrastructure — Gmail",
            "initiative-infrastructure",
            &["team-loo"],
        );
        let responses = vec![
            projects_response(json!([old_infrastructure])),
            issues_response(json!([migration_issue_node(
                "issue-done",
                "OLD-2",
                "project-infrastructure",
                "Infrastructure — Gmail",
                "team-old",
                true,
            )])),
            projects_response(json!([old_survival])),
            issues_response(json!([migration_issue_node(
                "issue-open",
                "OLD-1",
                "project-survival",
                "Survival — A real task reaches done",
                "team-old",
                false,
            )])),
            project_update_response("project-survival"),
            project_update_response("project-infrastructure"),
            issue_comments_response(),
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-done" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "issue-done", "identifier": "LOO-2" } } } }),
            ),
            issue_comments_response(),
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-open" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "issue-open", "identifier": "LOO-1" } } } }),
            ),
            project_update_response("project-survival"),
            project_update_response("project-infrastructure"),
            project_update_response("project-infrastructure"),
            projects_response(json!([new_infrastructure])),
            issues_response(json!([migration_issue_node(
                "issue-done",
                "LOO-2",
                "project-infrastructure",
                "Survival / Infrastructure — Gmail",
                "team-loo",
                true,
            )])),
            projects_response(json!([new_survival])),
            issues_response(json!([migration_issue_node(
                "issue-open",
                "LOO-1",
                "project-survival",
                "Survival — A real task reaches done",
                "team-loo",
                false,
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
                provider: PmProviderKind::Linear,
                repo_id: RepoId::parse("loopflowstudio/fixture").unwrap(),
                team_id: "team-loo".to_string(),
            },
            team_key: "LOO".to_string(),
            store,
        };

        let result = apply_or_plan_repository_reteam(&resolved, repo.path(), true, &NullProgress)
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
            read_repository_team(repo.path(), PmProviderKind::Linear)
                .unwrap()
                .as_deref(),
            Some("team-loo")
        );
        for wave in ["survival", "infrastructure"] {
            let pm = read_wave_pm_config(repo.path(), wave).unwrap();
            assert!(pm.provider.is_none());
            assert!(pm.linear_team.is_none());
            assert!(pm.linear_initiative.is_some());
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
        assert!(requests
            .iter()
            .any(|request| { request.body.contains("Survival / Infrastructure — Gmail") }));
    }

    #[tokio::test]
    async fn repository_team_reteam_resumes_after_an_interrupted_issue_move() {
        let repo = tempfile::tempdir().unwrap();
        write_repo_config(
            repo.path(),
            "pm:\n  provider: linear\n  linear_team: team-loo\nlinear:\n  team: team-old\n",
        );
        write_goal(
            repo.path(),
            "survival",
            "pm:\n  provider: linear\n  linear_initiative: initiative-survival\n  linear_team: team-old\n",
        );

        let database = repo.path().join("registry.db");
        let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
            database.clone(),
        ))
        .await
        .unwrap();
        store
            .create_wave(&Wave::new(
                WaveId::new(),
                "survival".to_string(),
                repo.path().display().to_string(),
            ))
            .await
            .unwrap();

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
        let responses = vec![
            projects_response(json!([old_project])),
            issues_response(json!([old_issue.clone()])),
            project_update_response("project-survival"),
            issue_comments_response(),
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-reteam" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "errors": [{ "message": "move interrupted" }] }),
            ),
            projects_response(json!([expanded_project])),
            issues_response(json!([old_issue])),
            issue_comments_response_with(Some(&marker)),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "issue-open", "identifier": "LOO-1" } } } }),
            ),
            project_update_response("project-survival"),
            projects_response(json!([migrated_project])),
            issues_response(json!([migrated_issue])),
        ];
        let (base_url, requests) = test_server::spawn(responses).await;
        let resolved = ResolvedReteamContext {
            repository: RepositoryPmContext {
                client: crate::pm::linear::LinearClient::with_base_url(
                    "linear-secret".to_string(),
                    Some("team-loo".to_string()),
                    base_url,
                ),
                provider: PmProviderKind::Linear,
                repo_id: RepoId::parse("loopflowstudio/fixture").unwrap(),
                team_id: "team-loo".to_string(),
            },
            team_key: "LOO".to_string(),
            store,
        };

        let first = apply_or_plan_repository_reteam(&resolved, repo.path(), true, &NullProgress)
            .await
            .unwrap_err();
        assert!(first.to_string().contains("move interrupted"));
        assert!(!legacy_pm_sentinels(repo.path()).unwrap().is_empty());

        let resumed = apply_or_plan_repository_reteam(&resolved, repo.path(), true, &NullProgress)
            .await
            .unwrap();
        assert_eq!(resumed.moves[0].new_identifier.as_deref(), Some("LOO-1"));
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
    }

    #[test]
    fn duplicate_linear_project_slugs_are_drift() {
        let project = |id: &str, name: &str| PmProject {
            id: id.to_string(),
            slug: crate::pm::project_slug(name),
            name: name.to_string(),
            summary: String::new(),
            definition: String::new(),
            flows: Some(ProjectFlowPlan::empty()),
            krs: Vec::new(),
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: vec!["team-loo".to_string()],
        };
        let projects = vec![project("one", "Wave Chat"), project("two", "Wave-Chat")];

        let error = ensure_unique_project_slugs(&projects, "product")
            .expect_err("duplicate slug must fail");
        assert!(error.to_string().contains("both derive slug `wave-chat`"));
    }

    #[test]
    fn show_result_serializes_the_complete_local_snapshot() {
        let result = PmShowResult {
            wave: "product".to_string(),
            provider: PmProviderKind::Linear,
            initiative: "initiative-1".to_string(),
            project: None,
            synced_at: 42,
            projects: vec![PmProject {
                id: "project-1".to_string(),
                slug: "wave-chat".to_string(),
                name: "Wave Chat".to_string(),
                summary: "Stay in flow.".to_string(),
                definition: "Conversation stays in flow.".to_string(),
                flows: Some(ProjectFlowPlan {
                    first: Some("task-design".to_string()),
                    loop_: Some("slice".to_string()),
                    finally: Some("ship".to_string()),
                }),
                krs: vec![PmKr {
                    text: "Replies survive restarts.".to_string(),
                    holds: true,
                }],
                initiative_ids: vec!["initiative-1".to_string()],
                team_ids: vec!["team-prd".to_string()],
            }],
            items: Vec::new(),
        };

        let value = serde_json::to_value(result).expect("serialize PM show result");
        assert_eq!(value["synced_at"], 42);
        assert_eq!(value["projects"][0]["team_ids"][0], "team-prd");
        assert_eq!(
            value["projects"][0]["definition"],
            "Conversation stays in flow."
        );
        assert_eq!(value["projects"][0]["flows"]["loop"], "slice");
        assert_eq!(value["projects"][0]["krs"][0]["holds"], true);
        assert_eq!(value["items"], serde_json::json!([]));
    }

    #[test]
    fn wave_summary_reads_objective_first_paragraph() {
        let repo = tempfile::tempdir().expect("temp dir");
        let dir = repo.path().join("wave/product");
        std::fs::create_dir_all(&dir).expect("create wave");
        std::fs::write(
            dir.join("GOAL.md"),
            "---\ncrons: []\n---\n\n## Objective\n\nProduct work stays coherent\nacross surfaces.\n\nSecond paragraph.\n\n## Bounds\n\nNo drift.\n",
        )
        .expect("write goal");

        assert_eq!(
            wave_summary(repo.path(), "product").expect("read summary"),
            "Product work stays coherent across surfaces."
        );
    }

    #[test]
    fn parse_done_status_maps_synonyms_and_rejects_others() {
        assert!(!parse_done_status(None).unwrap());
        assert!(parse_done_status(Some("done")).unwrap());
        assert!(parse_done_status(Some("Completed")).unwrap());
        assert!(parse_done_status(Some("blocked")).is_err());
    }

    #[tokio::test]
    async fn fetch_pm_snapshot_reads_projects_and_tags_their_items() {
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([project_node("project-123", "Scan")])),
            issues_response(json!([
                { "id": "issue-1", "identifier": "LOO-1", "url": null,
                  "title": "First", "description": "one",
                  "prioritySortOrder": 0.0, "sortOrder": 0.0,
                  "state": { "type": "unstarted" },
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
        assert_eq!(result.items[0].project, "scan");
        assert_eq!(
            requests.lock().await[1].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
    }

    #[tokio::test]
    async fn apply_update_requires_a_project_when_creating() {
        let (base_url, _requests) = test_server::spawn(vec![projects_response(json!([]))]).await;
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let repo = tempfile::tempdir().unwrap();
        let options = PmUpdateOptions {
            wave: None,
            project: None,
            id: None,
            title: Some("New task".to_string()),
            notes: Some("details".to_string()),
            status: None,
            pr: None,
        };

        let error = isolated_apply_update(repo.path(), "goals", None, &ctx, &options)
            .await
            .expect_err("project is required");
        assert!(error.to_string().contains("--project <slug>"));
    }

    #[tokio::test]
    async fn apply_update_creates_task_in_native_project() {
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([project_node("project-chat", "Wave Chat")])),
            json_response(
                StatusCode::OK,
                json!({ "data": { "workflowStates": { "nodes": [{ "id": "state-todo", "position": 1.0 }] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueCreate": { "issue": { "id": "new-task" } } } }),
            ),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let repo = tempfile::tempdir().unwrap();
        let options = PmUpdateOptions {
            wave: None,
            project: Some("wave-chat".to_string()),
            id: None,
            title: Some("New task".to_string()),
            notes: None,
            status: None,
            pr: None,
        };

        let result =
            isolated_apply_update(repo.path(), "product", Some("wave-chat"), &ctx, &options)
                .await
                .expect("update succeeds");

        assert!(result.created);
        let requests = requests.lock().await;
        let create_body: serde_json::Value =
            serde_json::from_str(&requests[2].body).expect("create body is json");
        assert_eq!(create_body["variables"]["projectId"], "project-chat");
        assert!(create_body["variables"].get("labelIds").is_none());
    }

    #[tokio::test]
    async fn apply_update_completes_when_status_done() {
        let (base_url, _requests) = test_server::spawn(vec![
            projects_response(json!([])),
            // complete_item reads the issue's owning team, resolves that team's
            // completed workflow state, then transitions.
            json_response(
                StatusCode::OK,
                json!({ "data": { "issue": { "team": { "id": "team-9" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "workflowStates": { "nodes": [{ "id": "state-done" }] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "task-9" } } } }),
            ),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let repo = tempfile::tempdir().unwrap();
        let options = PmUpdateOptions {
            wave: None,
            project: None,
            id: Some("task-9".to_string()),
            title: None,
            notes: None,
            status: Some("done".to_string()),
            pr: None,
        };

        let result = isolated_apply_update(repo.path(), "goals", None, &ctx, &options)
            .await
            .expect("update succeeds");
        assert!(!result.created);
        assert_eq!(result.id, "task-9");
        assert!(result.completed);
    }

    #[tokio::test]
    async fn apply_update_closes_then_comments_pr_link() {
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([])),
            // update_item (issueUpdate)
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "task-9" } } } }),
            ),
            // complete_item: read the issue team, resolve its completed state,
            // then transition — before any comment, so a rejected close never
            // leaves a "Shipped" comment behind.
            json_response(
                StatusCode::OK,
                json!({ "data": { "issue": { "team": { "id": "team-9" } } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "workflowStates": { "nodes": [{ "id": "state-done" }] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "task-9" } } } }),
            ),
            // comment (commentCreate) carrying the PR link, posted last
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-1" } } } }),
            ),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let repo = tempfile::tempdir().unwrap();
        let options = PmUpdateOptions {
            wave: None,
            project: None,
            id: Some("task-9".to_string()),
            title: Some("Existing".to_string()),
            notes: None,
            status: Some("done".to_string()),
            pr: Some("https://github.com/acme/repo/pull/42".to_string()),
        };

        let result = isolated_apply_update(repo.path(), "goals", None, &ctx, &options)
            .await
            .expect("update succeeds");
        assert!(result.completed);
        assert_eq!(
            result.linked_pr.as_deref(),
            Some("https://github.com/acme/repo/pull/42")
        );

        let requests = requests.lock().await;
        let comment_at = requests
            .iter()
            .position(|req| req.body.contains("commentCreate"))
            .expect("PR link is posted as a comment");
        let state_at = requests
            .iter()
            .position(|req| req.body.contains("SetIssueState"))
            .expect("issue state is transitioned to done");
        // The comment must follow the state transition: a rejected close never
        // leaves a "Shipped" comment on a still-open issue.
        assert!(state_at < comment_at);
        assert!(requests[comment_at].body.contains("Shipped:"));
        assert!(requests[comment_at].body.contains("pull/42"));
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

    // Env vars are process-global; serialize the forwarded-token tests so a
    // concurrent test never observes a half-set environment.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn pm_refreshes_due_linear_token_before_using_it() {
        let db_path =
            std::env::temp_dir().join(format!("lf-pm-refresh-{}.db", crate::id::WaveId::new()));
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(db_path))
                .await
                .expect("open token store"),
        );
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        store
            .upsert_provider_token(&ProviderToken {
                provider: "linear".to_string(),
                access_token: "old-access".to_string(),
                refresh_token: Some("old-refresh".to_string()),
                oauth_client_id: Some("linear-client".to_string()),
                expires_at: Some(now + 60),
                login: None,
                updated_at: now,
                credential_type: crate::store::CredentialType::OAuth,
            })
            .await
            .expect("store current token");

        let access_token = resolve_pm_token_from_store(
            PmProviderKind::Linear,
            &store,
            now,
            |provider, current| async move {
                assert_eq!(provider, Provider::Linear);
                assert_eq!(current.refresh_token.as_deref(), Some("old-refresh"));
                Ok(ProviderToken {
                    provider: "linear".to_string(),
                    access_token: "new-access".to_string(),
                    refresh_token: Some("new-refresh".to_string()),
                    oauth_client_id: Some("linear-client".to_string()),
                    expires_at: Some(now + 24 * 60 * 60),
                    login: None,
                    updated_at: now,
                    credential_type: crate::store::CredentialType::OAuth,
                })
            },
        )
        .await
        .expect("resolve refreshed token");

        assert_eq!(access_token, "new-access");
        let stored = store
            .get_provider_token("linear")
            .await
            .expect("load refreshed token")
            .expect("refreshed token row");
        assert_eq!(stored.refresh_token.as_deref(), Some("new-refresh"));
        assert_eq!(stored.oauth_client_id.as_deref(), Some("linear-client"));
    }

    #[tokio::test]
    async fn proactive_refresh_failure_uses_still_valid_token() {
        let db_path =
            std::env::temp_dir().join(format!("lf-pm-refresh-{}.db", crate::id::WaveId::new()));
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(db_path))
                .await
                .expect("open token store"),
        );
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        store
            .upsert_provider_token(&ProviderToken {
                provider: "linear".to_string(),
                access_token: "still-valid".to_string(),
                refresh_token: Some("refresh-token".to_string()),
                oauth_client_id: Some("linear-client".to_string()),
                expires_at: Some(now + 60),
                login: None,
                updated_at: now,
                credential_type: crate::store::CredentialType::OAuth,
            })
            .await
            .expect("store current token");

        let access_token = resolve_pm_token_from_store(
            PmProviderKind::Linear,
            &store,
            now,
            |provider, _| async move {
                Err(TokenRefreshError::OAuth {
                    provider,
                    reason: "the token endpoint rejected or could not complete the request",
                })
            },
        )
        .await
        .expect("valid token remains usable");

        assert_eq!(access_token, "still-valid");
    }

    #[test]
    fn resolve_pm_token_prefers_forwarded_env() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::set_var(FORWARDED_PM_TOKEN_ENV, "forwarded-secret");
        std::env::remove_var(FORWARDED_PM_PROVIDER_ENV);

        // Returns the forwarded token without ever opening the store store.
        let token = block_on_pm(resolve_pm_token(PmProviderKind::Linear)).expect("token");
        assert_eq!(token, "forwarded-secret");

        std::env::remove_var(FORWARDED_PM_TOKEN_ENV);
    }

    #[test]
    fn forwarded_pm_token_matches_provider_and_skips_blank() {
        let _guard = ENV_LOCK.lock().expect("env lock");

        std::env::set_var(FORWARDED_PM_TOKEN_ENV, "tok");
        std::env::set_var(FORWARDED_PM_PROVIDER_ENV, "linear");
        assert_eq!(
            forwarded_pm_token(PmProviderKind::Linear).as_deref(),
            Some("tok")
        );

        // A provider that doesn't match falls through to the store.
        std::env::set_var(FORWARDED_PM_PROVIDER_ENV, "github");
        assert_eq!(forwarded_pm_token(PmProviderKind::Linear), None);

        // A blank token is treated as absent.
        std::env::set_var(FORWARDED_PM_TOKEN_ENV, "   ");
        std::env::remove_var(FORWARDED_PM_PROVIDER_ENV);
        assert_eq!(forwarded_pm_token(PmProviderKind::Linear), None);

        std::env::remove_var(FORWARDED_PM_TOKEN_ENV);
        std::env::remove_var(FORWARDED_PM_PROVIDER_ENV);
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
