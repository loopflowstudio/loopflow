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

use crate::engine::config::load_config_or_default;
use crate::engine::wave_config::{read_wave_config, update_wave_goal_config, WavePmConfig};
use crate::lfd::pm::linear::LinearClient;
use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmKr, PmProject, PmProviderKind, PmResult, PmWave,
};
use crate::lfdb::{open_store, PmSnapshotRow, ProviderToken, Store};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;
use crate::provider_auth::{
    provider_token_refresh_due, refresh_stored_provider_token, Provider, TokenRefreshError,
};

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PmInitOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmInitResult {
    pub wave: String,
    pub initiative_id: String,
    pub created: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
    pub provider: PmProviderKind,
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
    pub definition: String,
    pub krs: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PmSnapshot {
    projects: Vec<PmProject>,
    items: Vec<PmItem>,
}

// ── Client + Linear project resolution ──────────────────────────────

/// A wave's PM client bound to its provider Linear Initiative.
pub(crate) struct PmContext {
    pub client: PmClient,
    pub provider: PmProviderKind,
    pub initiative: String,
}

#[derive(Clone)]
pub(crate) enum PmClient {
    Linear(LinearClient),
}

impl PmClient {
    async fn create_wave(&self, name: &str, summary: &str) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.create_wave(name, summary).await,
        }
    }

    async fn rename_wave(&self, initiative_id: &str, name: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.rename_wave(initiative_id, name).await,
        }
    }

    async fn list_waves(&self) -> PmResult<Vec<PmWave>> {
        match self {
            Self::Linear(client) => client.list_waves().await,
        }
    }

    async fn list_projects(&self, initiative_id: &str) -> PmResult<Vec<PmProject>> {
        match self {
            Self::Linear(client) => client.list_projects(initiative_id).await,
        }
    }

    async fn create_project(
        &self,
        initiative_id: &str,
        name: &str,
        definition: &str,
        krs: &[PmKr],
    ) -> PmResult<String> {
        match self {
            Self::Linear(client) => {
                client
                    .create_project(
                        initiative_id,
                        name,
                        &first_paragraph(definition),
                        definition,
                        krs,
                    )
                    .await
            }
        }
    }

    async fn update_project(
        &self,
        project_id: &str,
        name: &str,
        definition: &str,
        krs: &[PmKr],
    ) -> PmResult<()> {
        match self {
            Self::Linear(client) => {
                client
                    .update_project(
                        project_id,
                        name,
                        &first_paragraph(definition),
                        definition,
                        krs,
                    )
                    .await
            }
        }
    }

    async fn archive_project(&self, project_id: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.archive_project(project_id).await,
        }
    }

    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        match self {
            Self::Linear(client) => client.list_items(project_id).await,
        }
    }

    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.create_item(project_id, item).await,
        }
    }

    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.update_item(item_id, update).await,
        }
    }

    async fn move_item_to_project(&self, item_id: &str, project_id: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.move_item_to_project(item_id, project_id).await,
        }
    }

    async fn complete_item(&self, item_id: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.complete_item(item_id).await,
        }
    }

    async fn comment(&self, item_id: &str, body: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.comment(item_id, body).await,
        }
    }
}

fn read_wave_pm_config(repo: &Path, wave: &str) -> Option<WavePmConfig> {
    read_wave_config(repo, wave).and_then(|config| config.pm)
}

fn resolve_wave(repo: &Path, wave: Option<&str>) -> OpsResult<String> {
    resolve_wave_name(repo, wave)
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))
}

fn parse_provider(value: &str) -> OpsResult<PmProviderKind> {
    value.parse::<PmProviderKind>().map_err(pm_to_ops)
}

fn resolve_provider(repo: &Path, wave: &str) -> OpsResult<PmProviderKind> {
    let config = load_config_or_default(Some(repo));
    let wave_pm = read_wave_pm_config(repo, wave);
    if let Some(provider) = wave_pm
        .as_ref()
        .and_then(|pm| pm.provider.as_deref())
        .filter(|provider| !provider.trim().is_empty())
    {
        return parse_provider(provider);
    }
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

/// Whether a wave has a Linear Initiative pinned for its resolved provider.
fn wave_has_pm_initiative(repo: &Path, wave: &str) -> bool {
    resolve_provider(repo, wave)
        .ok()
        .is_some_and(|provider| read_initiative(repo, wave, provider).is_some())
}

async fn build_client(repo: &Path, provider: PmProviderKind) -> OpsResult<PmClient> {
    let config = load_config_or_default(Some(repo));
    let token = resolve_pm_token(provider).await?;
    match provider {
        PmProviderKind::Linear => Ok(PmClient::Linear(LinearClient::new(
            token,
            config.linear.team.clone(),
        ))),
    }
}

async fn resolve_context(repo: &Path, wave: &str) -> OpsResult<PmContext> {
    let provider = resolve_provider(repo, wave)?;
    let initiative = read_initiative(repo, wave, provider).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.{}`. \
             Run `lf pm init --wave {wave}` to connect its Linear Initiative.",
            provider.initiative_key()
        ))
    })?;
    let client = build_client(repo, provider).await?;
    Ok(PmContext {
        client,
        provider,
        initiative,
    })
}

/// Linear authenticates via OAuth: the access token and refresh grant live in
/// lfdb, and PM access refreshes the grant before the access token expires.
async fn resolve_pm_token(provider: PmProviderKind) -> OpsResult<String> {
    // A forwarded token wins over the local store: `lf ssh` resolves the PM
    // credential on the caller's machine (where lfdb lives) and hands it to the
    // remote through the environment. The remote lfdb holds no PM credential, so
    // without this hook remote `lf pm` could never authenticate.
    if let Some(token) = forwarded_pm_token(provider) {
        return Ok(token);
    }

    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open lfd credential store: {err}")))?;
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

fn storage_config_from_env() -> OpsResult<crate::lfdb::StorageConfig> {
    crate::lfd::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve lfd credential store: {err}")))
}

fn pm_repo_key(repo: &Path) -> String {
    let root =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    std::fs::canonicalize(&root)
        .unwrap_or(root)
        .to_string_lossy()
        .into_owned()
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
    pm_store()
        .await?
        .pm_snapshot(pm_repo_key(repo), wave.to_string())
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

async fn fetch_pm_snapshot(wave: &str, ctx: &PmContext) -> OpsResult<PmSnapshot> {
    let projects = checked_projects(&ctx.client, &ctx.initiative, wave).await?;
    let project_items = try_join_all(projects.iter().cloned().map(|project| async move {
        let mut items = ctx
            .client
            .list_items(&project.id)
            .await
            .map_err(pm_to_ops)?;
        for item in &mut items {
            item.project = Some(project.slug.clone());
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
    let payload = serde_json::to_string(snapshot).map_err(|err| {
        OpsError::Message(format!(
            "failed to serialize PM snapshot for wave/{wave}: {err}"
        ))
    })?;
    pm_store()
        .await?
        .put_pm_snapshot(PmSnapshotRow {
            repo: pm_repo_key(repo),
            wave: wave.to_string(),
            provider: ctx.provider.as_str().to_string(),
            initiative: ctx.initiative.clone(),
            synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            payload,
        })
        .await
        .map_err(|err| OpsError::Message(format!("failed to store PM snapshot: {err}")))
}

async fn refresh_pm_snapshot(repo: &Path, wave: &str, ctx: &PmContext) -> OpsResult<PmSnapshot> {
    let snapshot = fetch_pm_snapshot(wave, ctx).await?;
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
    let wave = resolve_wave(repo, options.wave.as_deref())?;
    let wave_dir = repo.join("wave").join(&wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{wave}/"
        )));
    }

    let provider = resolve_provider(repo, &wave)?;
    let existing_initiative = read_initiative(repo, &wave, provider);
    let binding_missing = existing_initiative.is_none();
    if let Some(existing) = existing_initiative.as_ref() {
        progress.status(&format!(
            "wave/{wave} already linked to {provider} Linear Initiative {existing}"
        ));
        return Ok(PmInitResult {
            wave,
            initiative_id: existing.clone(),
            created: false,
        });
    }

    let summary = wave_summary(repo, &wave)?;
    let client = build_client(repo, provider).await?;
    let title = title_case(&wave);
    progress.status(&format!(
        "looking for {provider} Linear Initiative `{title}`"
    ));
    let existing = matching_wave_id(&client.list_waves().await.map_err(pm_to_ops)?, &title)?;
    let (initiative_id, created) = match existing {
        Some(id) => {
            progress.status(&format!(
                "linking wave/{wave} to existing {provider} Linear Initiative {id}"
            ));
            (id, false)
        }
        None => {
            progress.status(&format!(
                "creating {provider} Linear Initiative for wave/{wave}"
            ));
            (
                client
                    .create_wave(&title, &summary)
                    .await
                    .map_err(pm_to_ops)?,
                true,
            )
        }
    };
    write_initiative_to_goal(repo, &wave, provider, &initiative_id)?;

    if binding_missing {
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

async fn pm_show_async(
    repo: &Path,
    options: &PmShowOptions,
    progress: &impl Progress,
) -> OpsResult<PmShowResult> {
    let wave = resolve_wave(repo, options.wave.as_deref())?;
    let row = load_show_snapshot(repo, &wave, options.refresh, progress).await?;
    let snapshot: PmSnapshot = serde_json::from_str(&row.payload).map_err(|err| {
        OpsError::Message(format!(
            "invalid PM snapshot for wave/{wave}; run `lf pm sync`: {err}"
        ))
    })?;
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
        .filter(|item| {
            item.project
                .as_deref()
                .is_some_and(|project| slugs.contains(project))
        })
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

async fn pm_update_async(
    repo: &Path,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let wave = resolve_wave(repo, options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    let result = apply_update(&wave, options.project.as_deref(), &ctx, options, progress).await?;
    progress.status(&format!("refreshing local PM snapshot for wave/{wave}"));
    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(result)
}

async fn apply_update(
    wave: &str,
    project_slug: Option<&str>,
    ctx: &PmContext,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let mark_done = parse_done_status(options.status.as_deref())?;
    let projects = checked_projects(&ctx.client, &ctx.initiative, wave).await?;
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

    // Attach the PR link as a comment before closing so the task carries a
    // durable pointer to the work without clobbering its description.
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

    if mark_done {
        ctx.client.complete_item(&id).await.map_err(pm_to_ops)?;
    }

    Ok(PmUpdateResult {
        wave: wave.to_string(),
        id,
        created,
        completed: mark_done,
        linked_pr,
    })
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
        vec![resolve_wave(repo, Some(wave))?]
    } else {
        list_pm_waves(repo)?
    };

    let mut results = Vec::new();
    for wave in waves {
        let row = read_pm_snapshot(repo, &wave).await?;
        let snapshot: PmSnapshot = serde_json::from_str(&row.payload).map_err(|err| {
            OpsError::Message(format!("invalid PM snapshot for wave/{wave}: {err}"))
        })?;
        let total = snapshot.items.len();
        let open = snapshot.items.iter().filter(|item| !item.completed).count();
        let mut open_by_project = BTreeMap::new();
        for project in snapshot.projects {
            let project_open = snapshot
                .items
                .iter()
                .filter(|item| {
                    !item.completed && item.project.as_deref() == Some(project.slug.as_str())
                })
                .count();
            open_by_project.insert(project.slug, project_open);
        }
        results.push(PmWaveStatus {
            initiative_name: title_case(&wave),
            wave,
            provider: row.provider.parse().map_err(pm_to_ops)?,
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
        Some(wave) => vec![resolve_wave(repo, Some(wave))?],
        None => all_waves.clone(),
    };
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();

    let mut linked_initiative_ids = BTreeSet::new();
    let mut provider_by_kind = BTreeMap::new();
    for wave in &all_waves {
        let provider = resolve_provider(repo, wave)?;
        provider_by_kind.insert(provider.as_str().to_string(), provider);
        if let Some(initiative) = read_initiative(repo, wave, provider) {
            linked_initiative_ids.insert(initiative);
        } else {
            diagnostics.push(format!("wave/{wave} has no Linear Initiative"));
        }
    }

    let provider = provider_by_kind
        .values()
        .next()
        .copied()
        .unwrap_or(PmProviderKind::Linear);
    let client = build_client(repo, provider).await?;
    progress.status(&format!(
        "checking {provider} Linear Initiatives and Projects"
    ));
    let linear_waves = client.list_waves().await.map_err(pm_to_ops)?;
    let linear_waves_by_id: BTreeMap<String, String> = linear_waves
        .iter()
        .map(|wave| (wave.id.clone(), wave.name.clone()))
        .collect();
    for linear_wave in &linear_waves {
        if !linked_initiative_ids.contains(&linear_wave.id) {
            diagnostics.push(format!(
                "Linear Initiative `{}` ({}) is not linked by any local wave",
                linear_wave.name, linear_wave.id
            ));
        }
    }

    for wave in &waves {
        let provider = resolve_provider(repo, wave)?;
        let Some(initiative_id) = read_initiative(repo, wave, provider) else {
            continue;
        };
        let expected_initiative_name = title_case(wave);
        match linear_waves_by_id.get(&initiative_id) {
            Some(actual) if actual != &expected_initiative_name => {
                let message = format!(
                    "rename Linear Initiative `{actual}` ({initiative_id}) to `{expected_initiative_name}` for wave/{wave}"
                );
                if !options.plan {
                    client
                        .rename_wave(&initiative_id, &expected_initiative_name)
                        .await
                        .map_err(pm_to_ops)?;
                }
                actions.push(message);
            }
            None => diagnostics.push(format!(
                "wave/{wave} points at missing Linear Initiative {initiative_id}"
            )),
            _ => {}
        }

        let ctx = PmContext {
            client: client.clone(),
            provider,
            initiative: initiative_id.clone(),
        };
        let snapshot = fetch_pm_snapshot(wave, &ctx).await?;
        for project in &snapshot.projects {
            let managed_initiatives = project
                .initiative_ids
                .iter()
                .filter(|id| linked_initiative_ids.contains(*id))
                .count();
            if managed_initiatives != 1 {
                diagnostics.push(format!(
                    "Linear Project `{}` ({}) belongs to {managed_initiatives} Loopflow-managed Initiatives; expected exactly one",
                    project.name, project.id
                ));
            }
            let items: Vec<_> = snapshot
                .items
                .iter()
                .filter(|item| item.project.as_deref() == Some(project.slug.as_str()))
                .collect();
            if items.iter().all(|item| item.completed) {
                diagnostics.push(format!(
                    "Linear Project `{}` ({}) in wave/{wave} has no open tasks",
                    project.name, project.id
                ));
            }
        }
        actions.push(format!(
            "refresh wave/{wave} PM snapshot from Linear Initiative {initiative_id}"
        ));
        if !options.plan {
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
    let wave = resolve_wave(repo, options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    let projects = checked_projects(&ctx.client, &ctx.initiative, &wave).await?;
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
    let wave = resolve_wave(repo, options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    let krs = options
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
    if krs.is_empty() {
        return Err(OpsError::Message(
            "at least one `--kr` is required".to_string(),
        ));
    }

    let projects = checked_projects(&ctx.client, &ctx.initiative, &wave).await?;
    let (id, slug, created) = if let Some(slug) = options.project.as_deref() {
        let project = find_project(&projects, &wave, slug)?;
        let name = options
            .title
            .clone()
            .unwrap_or_else(|| project.name.clone());
        let new_slug = crate::lfd::pm::project_slug(&name);
        if projects
            .iter()
            .any(|candidate| candidate.id != project.id && candidate.slug == new_slug)
        {
            return Err(OpsError::Message(format!(
                "wave/{wave} already has a Linear Project with slug `{new_slug}`"
            )));
        }
        progress.status(&format!("updating Linear Project `{}`", project.name));
        ctx.client
            .update_project(&project.id, &name, &options.definition, &krs)
            .await
            .map_err(pm_to_ops)?;
        (project.id.clone(), new_slug, false)
    } else {
        let name = options.title.clone().ok_or_else(|| {
            OpsError::Message("`lf pm project create --title` is required".to_string())
        })?;
        let slug = crate::lfd::pm::project_slug(&name);
        if projects.iter().any(|project| project.slug == slug) {
            return Err(OpsError::Message(format!(
                "wave/{wave} already has a Linear Project with slug `{slug}`; use `lf pm project update`"
            )));
        }
        progress.status(&format!("creating Linear Project `{name}`"));
        let id = ctx
            .client
            .create_project(&ctx.initiative, &name, &options.definition, &krs)
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
    let wave = resolve_wave(repo, options.wave.as_deref())?;
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
    let wave = resolve_wave(repo, options.wave.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    let projects = checked_projects(&ctx.client, &ctx.initiative, &wave).await?;
    let project = find_project(&projects, &wave, &options.project)?;
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
    for entry in std::fs::read_dir(&wave_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            waves.push(name.to_string());
        }
    }
    waves.sort();
    Ok(waves)
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
            serde_yaml_ng::Value::String("provider".to_string()),
            serde_yaml_ng::Value::String(provider.as_str().to_string()),
        );
        pm_map.insert(
            serde_yaml_ng::Value::String(provider.initiative_key().to_string()),
            serde_yaml_ng::Value::String(initiative_id.to_string()),
        );
        map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));
        Ok(())
    })
    .map_err(OpsError::Message)
}

fn wave_summary(repo: &Path, wave: &str) -> OpsResult<String> {
    let path = repo.join("wave").join(wave).join("GOAL.md");
    let content = std::fs::read_to_string(&path)?;
    let objective = markdown_section(&content, "Objective");
    Ok(first_paragraph(&objective))
}

fn markdown_section(content: &str, heading: &str) -> String {
    let marker = format!("## {heading}");
    let mut in_section = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        if line.trim() == marker {
            in_section = true;
            continue;
        }
        if in_section && line.trim_start().starts_with("## ") {
            break;
        }
        if in_section {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_string()
}

fn first_paragraph(content: &str) -> String {
    content
        .split("\n\n")
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|paragraph| !paragraph.is_empty())
        .unwrap_or_default()
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

/// List an initiative's Linear Projects and validate their slugs are unique.
async fn checked_projects(
    client: &PmClient,
    initiative: &str,
    wave: &str,
) -> OpsResult<Vec<PmProject>> {
    let projects = client.list_projects(initiative).await.map_err(pm_to_ops)?;
    ensure_unique_project_slugs(&projects, wave)?;
    Ok(projects)
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
    use crate::lfd::pm::test_server::{self, json_response, QueuedResponse};
    use crate::ops::NullProgress;
    use axum::http::StatusCode;
    use serde_json::json;

    fn linear_test_ctx(base_url: String, initiative: &str) -> PmContext {
        PmContext {
            client: PmClient::Linear(crate::lfd::pm::linear::LinearClient::with_base_url(
                "linear-secret".to_string(),
                Some("team-9".to_string()),
                base_url,
            )),
            provider: PmProviderKind::Linear,
            initiative: initiative.to_string(),
        }
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
            "initiatives": { "nodes": [{ "id": "initiative-123" }] }
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

    #[test]
    fn resolve_provider_defaults_to_linear() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(repo.path(), "goals", "pm:\n  provider: \"\"\n");
        assert_eq!(
            resolve_provider(repo.path(), "goals").unwrap(),
            PmProviderKind::Linear
        );
    }

    #[test]
    fn resolve_provider_selects_linear_from_frontmatter() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "scan",
            "pm:\n  provider: linear\n  linear_initiative: \"lin-1\"\n",
        );
        assert_eq!(
            resolve_provider(repo.path(), "scan").unwrap(),
            PmProviderKind::Linear
        );
    }

    #[test]
    fn title_case_humanizes_wave_slug() {
        assert_eq!(title_case("wave-repo-split"), "Wave Repo Split");
        assert_eq!(title_case("concerto"), "Concerto");
    }

    #[test]
    fn initiative_title_discovery_requires_an_exact_unique_match() {
        let waves = vec![
            PmWave {
                id: "product-1".to_string(),
                name: "Product".to_string(),
                summary: String::new(),
            },
            PmWave {
                id: "lowercase".to_string(),
                name: "product".to_string(),
                summary: String::new(),
            },
        ];

        assert_eq!(
            matching_wave_id(&waves, "Product").expect("unique match"),
            Some("product-1".to_string())
        );
        assert_eq!(matching_wave_id(&waves, "Missing").expect("no match"), None);

        let duplicates = vec![waves[0].clone(), waves[0].clone()];
        let error = matching_wave_id(&duplicates, "Product").expect_err("duplicates must fail");
        assert!(error.to_string().contains("product-1, product-1"));
    }

    #[test]
    fn initiative_write_persists_only_the_stable_binding() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(repo.path(), "product", "pm:\n  provider: linear\n");

        write_initiative_to_goal(
            repo.path(),
            "product",
            PmProviderKind::Linear,
            "initiative-1",
        )
        .expect("write initiative");

        let pm = read_wave_pm_config(repo.path(), "product").expect("pm config");
        assert_eq!(pm.provider.as_deref(), Some("linear"));
        assert_eq!(pm.linear_initiative.as_deref(), Some("initiative-1"));
    }

    #[test]
    fn duplicate_linear_project_slugs_are_drift() {
        let project = |id: &str, name: &str| PmProject {
            id: id.to_string(),
            slug: crate::lfd::pm::project_slug(name),
            name: name.to_string(),
            summary: String::new(),
            definition: String::new(),
            krs: Vec::new(),
            initiative_ids: vec!["initiative-1".to_string()],
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
                krs: vec![PmKr {
                    text: "Replies survive restarts.".to_string(),
                    holds: true,
                }],
                initiative_ids: vec!["initiative-1".to_string()],
            }],
            items: Vec::new(),
        };

        let value = serde_json::to_value(result).expect("serialize PM show result");
        assert_eq!(value["synced_at"], 42);
        assert_eq!(
            value["projects"][0]["definition"],
            "Conversation stays in flow."
        );
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
                { "id": "issue-1", "title": "First", "description": "one",
                  "prioritySortOrder": 0.0, "sortOrder": 0.0,
                  "state": { "type": "unstarted" } }
            ])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");

        let result = fetch_pm_snapshot("scan", &ctx)
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
    async fn apply_update_requires_a_project_when_creating() {
        let (base_url, _requests) = test_server::spawn(vec![projects_response(json!([]))]).await;
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let options = PmUpdateOptions {
            wave: None,
            project: None,
            id: None,
            title: Some("New task".to_string()),
            notes: Some("details".to_string()),
            status: None,
            pr: None,
        };

        let error = apply_update("goals", None, &ctx, &options, &NullProgress)
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
        let options = PmUpdateOptions {
            wave: None,
            project: Some("wave-chat".to_string()),
            id: None,
            title: Some("New task".to_string()),
            notes: None,
            status: None,
            pr: None,
        };

        let result = apply_update("product", Some("wave-chat"), &ctx, &options, &NullProgress)
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
            // complete_item resolves the completed workflow state, then transitions
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
        let options = PmUpdateOptions {
            wave: None,
            project: None,
            id: Some("task-9".to_string()),
            title: None,
            notes: None,
            status: Some("done".to_string()),
            pr: None,
        };

        let result = apply_update("goals", None, &ctx, &options, &NullProgress)
            .await
            .expect("update succeeds");
        assert!(!result.created);
        assert_eq!(result.id, "task-9");
        assert!(result.completed);
    }

    #[tokio::test]
    async fn apply_update_comments_pr_link_then_closes() {
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([])),
            // update_item (issueUpdate)
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "task-9" } } } }),
            ),
            // comment (commentCreate) carrying the PR link
            json_response(
                StatusCode::OK,
                json!({ "data": { "commentCreate": { "comment": { "id": "comment-1" } } } }),
            ),
            // complete_item: resolve the completed state, then transition
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
        let options = PmUpdateOptions {
            wave: None,
            project: None,
            id: Some("task-9".to_string()),
            title: Some("Existing".to_string()),
            notes: None,
            status: Some("done".to_string()),
            pr: Some("https://github.com/acme/repo/pull/42".to_string()),
        };

        let result = apply_update("goals", None, &ctx, &options, &NullProgress)
            .await
            .expect("update succeeds");
        assert!(result.completed);
        assert_eq!(
            result.linked_pr.as_deref(),
            Some("https://github.com/acme/repo/pull/42")
        );

        let requests = requests.lock().await;
        let comment = requests
            .iter()
            .find(|req| req.body.contains("commentCreate"))
            .expect("PR link is posted as a comment");
        assert!(comment.body.contains("Shipped:"));
        assert!(comment.body.contains("pull/42"));
    }

    // Env vars are process-global; serialize the forwarded-token tests so a
    // concurrent test never observes a half-set environment.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn pm_refreshes_due_linear_token_before_using_it() {
        let db_path =
            std::env::temp_dir().join(format!("lf-pm-refresh-{}.db", crate::lfd::id::LfdId::new()));
        let store = std::sync::Arc::new(
            open_store(&crate::lfdb::StorageConfig::sqlite(db_path))
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
                credential_type: crate::lfdb::CredentialType::OAuth,
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
                    credential_type: crate::lfdb::CredentialType::OAuth,
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
            std::env::temp_dir().join(format!("lf-pm-refresh-{}.db", crate::lfd::id::LfdId::new()));
        let store = std::sync::Arc::new(
            open_store(&crate::lfdb::StorageConfig::sqlite(db_path))
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
                credential_type: crate::lfdb::CredentialType::OAuth,
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

        // Returns the forwarded token without ever opening the lfdb store.
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

    #[tokio::test]
    async fn local_mutations_keep_no_sync_read_current() {
        // Every PM mutation refreshes the acting machine's snapshot, so a
        // `--no-sync` read there reflects all local changes without ever calling
        // Linear. This exercises the storage layer each mutation's refresh writes
        // through and the exact (de)serialization `pm show` reads back.
        let db_path =
            std::env::temp_dir().join(format!("lf-pm-cache-{}.db", crate::lfd::id::LfdId::new()));
        let store = open_store(&crate::lfdb::StorageConfig::sqlite(db_path))
            .await
            .expect("open store");

        let project = PmProject {
            id: "proj-core".to_string(),
            slug: "core".to_string(),
            name: "Core".to_string(),
            summary: String::new(),
            definition: String::new(),
            krs: vec![],
            initiative_ids: vec!["init-1".to_string()],
        };

        // Three successive local mutations; each mutation's refresh stores a
        // fuller snapshot (one more task folded in), with no explicit sync.
        let mut items = Vec::new();
        for n in 1..=3 {
            items.push(PmItem {
                id: format!("task-{n}"),
                name: format!("Task {n}"),
                description: String::new(),
                rank: n as u32,
                completed: false,
                project: Some("core".to_string()),
                assignee: None,
            });
            let payload = serde_json::to_string(&PmSnapshot {
                projects: vec![project.clone()],
                items: items.clone(),
            })
            .expect("serialize snapshot");
            store
                .put_pm_snapshot(PmSnapshotRow {
                    repo: "/repo".to_string(),
                    wave: "product".to_string(),
                    provider: "linear".to_string(),
                    initiative: "init-1".to_string(),
                    synced_at: 1000 * n as i64,
                    payload,
                })
                .await
                .expect("store snapshot");
        }

        // A cache-only read returns the latest snapshot: all three local changes.
        let row = store
            .pm_snapshot("/repo".to_string(), "product".to_string())
            .await
            .expect("read snapshot")
            .expect("snapshot present");
        let snapshot: PmSnapshot =
            serde_json::from_str(&row.payload).expect("deserialize snapshot");
        let ids: Vec<_> = snapshot.items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["task-1", "task-2", "task-3"]);
        assert_eq!(row.synced_at, 3000);
    }
}
