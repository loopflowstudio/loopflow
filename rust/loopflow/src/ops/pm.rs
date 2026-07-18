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
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::normalize_wave_name;
use crate::pm::linear::LinearClient;
use crate::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmKr, PmProject, PmProviderKind, PmResult, PmWave,
    TeamBinding,
};
use crate::provider_auth::{
    provider_token_refresh_due, refresh_stored_provider_token, Provider, TokenRefreshError,
};
use crate::store::{open_store, PmSnapshotRow, ProviderToken, Store, TaskWriterState};
#[cfg(test)]
use crate::task::{TaskSession, TaskSessionStatus};

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PmInitOptions {
    pub wave: Option<String>,
    /// Team key (Task prefix, e.g. `PRD`). Defaults to one derived from the wave.
    pub team_key: Option<String>,
    /// Team display name. Defaults to the title-cased wave name.
    pub team_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmInitResult {
    pub wave: String,
    pub initiative_id: String,
    pub created: bool,
    /// Stable id of the wave's team (owns the Task prefix).
    pub team_id: String,
    /// The team's key, when this run resolved it (`None` on a full no-op re-init).
    pub team_key: Option<String>,
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
    pub wave: Option<String>,
    /// Execute the moves. Without it, `reteam` only prints the plan (dry run).
    pub apply: bool,
}

/// One issue that moves into the wave's team. `new_identifier` is filled only
/// after an applied move (Linear assigns the number then).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmReteamMove {
    pub id: String,
    pub old_identifier: String,
    pub title: String,
    pub new_identifier: Option<String>,
}

/// One issue left in place while a Task Run can still write its old id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmReteamDeferral {
    pub identifier: String,
    pub title: String,
    pub reason: String,
}

/// One Project narrowed onto the wave's team. Projects keep their id and slug on
/// a team move (Linear only renumbers issues), so there is no new identifier to
/// carry — `from_teams` records where it came from for the plan output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmReteamProjectMove {
    pub id: String,
    pub name: String,
    pub from_teams: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmReteamResult {
    pub wave: String,
    pub team_id: String,
    pub team_key: String,
    /// True when moves were executed; false for a dry run.
    pub applied: bool,
    /// Projects narrowed onto the wave's team (narrowed after their issues moved
    /// off the legacy team).
    pub project_moves: Vec<PmReteamProjectMove>,
    pub moves: Vec<PmReteamMove>,
    pub deferrals: Vec<PmReteamDeferral>,
    /// Issues already carrying the target team key (skipped — idempotency).
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
    require_creation_team(repo, &wave, resolve_provider(repo, &wave)?)?;
    let ctx = resolve_context(repo, &wave).await?;
    let projects = checked_projects(&ctx.client, &ctx.initiative, &wave).await?;
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
        krs: Vec::new(),
    };
    let linear_name = linear_project_name(&wave, &seed.name);
    let id = match ctx
        .client
        .create_project(&ctx.initiative, &linear_name, &seed.definition, &seed.krs)
        .await
    {
        Ok(id) => id,
        Err(create_error) => checked_projects(&ctx.client, &ctx.initiative, &wave)
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
            krs: seed.krs,
            initiative_ids: vec![ctx.initiative],
            // The create result is transient — the next sync resolves the
            // authoritative teams from Linear.
            team_ids: None,
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PmSnapshot {
    projects: Vec<PmProject>,
    items: Vec<PmItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalProject {
    slug: String,
    name: String,
    summary: String,
    definition: String,
    krs: Vec<PmKr>,
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

    async fn ensure_team(&self, name: &str, key: &str) -> PmResult<TeamBinding> {
        match self {
            Self::Linear(client) => client.ensure_team(name, key).await,
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

    async fn move_item_to_team(&self, item_id: &str, team_id: &str) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.move_item_to_team(item_id, team_id).await,
        }
    }

    async fn move_project_to_team(&self, project_id: &str, team_id: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.move_project_to_team(project_id, team_id).await,
        }
    }

    async fn team_key(&self, team_id: &str) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.team_key(team_id).await,
        }
    }

    async fn complete_item(&self, item_id: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.complete_item(item_id).await,
        }
    }

    async fn reopen_item(&self, item_id: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.reopen_item(item_id).await,
        }
    }

    async fn comment(&self, item_id: &str, body: &str) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.comment(item_id, body).await,
        }
    }

    /// The bodies of an issue's existing comments, newest page first. `reteam`
    /// reads these to make its traceability comment idempotent: it re-posts only
    /// when this migration's marker is absent.
    async fn issue_comment_bodies(&self, item_id: &str) -> PmResult<Vec<String>> {
        match self {
            Self::Linear(client) => Ok(client
                .observe_issue(item_id)
                .await?
                .comments
                .into_iter()
                .map(|comment| comment.body)
                .collect()),
        }
    }

    async fn update_comment(&self, comment_id: &str, body: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.update_comment(comment_id, body).await,
        }
    }

    async fn link_attachment(&self, issue_id: &str, url: &str, title: &str) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.link_attachment(issue_id, url, title).await,
        }
    }

    async fn update_attachment(
        &self,
        attachment_id: &str,
        title: &str,
        subtitle: &str,
    ) -> PmResult<()> {
        match self {
            Self::Linear(client) => {
                client
                    .update_attachment(attachment_id, title, subtitle)
                    .await
            }
        }
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

/// The team id bound to a wave in GOAL.md, if any (`pm.linear_team`).
fn read_team(repo: &Path, wave: &str, provider: PmProviderKind) -> Option<String> {
    let pm = read_wave_pm_config(repo, wave)?;
    let team = match provider {
        PmProviderKind::Linear => pm.linear_team,
    }?;
    Some(team).filter(|team| !team.trim().is_empty())
}

/// The team a wave's PM client should act in: its own `pm.linear_team` binding,
/// falling back to the machine-global `config.linear.team` for waves not yet
/// bound (keeps unmigrated waves working on the shared team). Reads follow
/// Initiative → Project → Issue and ignore the team, so an unbound wave still
/// syncs its existing issues; the team only steers *creation*.
fn resolve_team(repo: &Path, wave: &str, provider: PmProviderKind) -> Option<String> {
    read_team(repo, wave, provider)
        .or_else(|| load_config_or_default(Some(repo)).linear.team.clone())
}

/// The team *creation* must act in. Fail closed: creation resolves a wave's
/// explicit `pm.linear_team` binding and never borrows `config.linear.team` or
/// Linear's auto-created default team, so a Project/Task cannot silently land
/// on a foreign team. An unbound wave errors with the exact recovery and no
/// Linear side effect; reads keep [`resolve_team`]'s fallback so an unbound
/// wave still syncs its existing issues.
fn require_creation_team(repo: &Path, wave: &str, provider: PmProviderKind) -> OpsResult<String> {
    read_team(repo, wave, provider).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.{key}`, so creating work would fall \
             back to the shared team. Bind the wave's team first: \
             `lf pm init --wave {wave} --team-key <KEY>`.",
            key = provider.team_key()
        ))
    })
}

/// Whether a wave has a Linear Initiative pinned for its resolved provider.
fn wave_has_pm_initiative(repo: &Path, wave: &str) -> bool {
    resolve_provider(repo, wave)
        .ok()
        .is_some_and(|provider| read_initiative(repo, wave, provider).is_some())
}

async fn build_client(
    _repo: &Path,
    provider: PmProviderKind,
    team: Option<String>,
) -> OpsResult<PmClient> {
    let token = resolve_pm_token(provider).await?;
    match provider {
        PmProviderKind::Linear => Ok(PmClient::Linear(LinearClient::new(token, team))),
    }
}

/// A configured Linear client for a wave, for webhook serve/register. Resolves
/// and refreshes the wave's OAuth token exactly like every other `lf pm` read.
pub async fn linear_client(repo: &Path, wave: &str) -> OpsResult<LinearClient> {
    let provider = resolve_provider(repo, wave)?;
    let PmClient::Linear(client) =
        build_client(repo, provider, resolve_team(repo, wave, provider)).await?;
    Ok(client)
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
    let client = build_client(repo, provider, resolve_team(repo, wave, provider)).await?;
    Ok(PmContext {
        client,
        provider,
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

fn decode_snapshot(wave: &str, payload: &str) -> OpsResult<PmSnapshot> {
    serde_json::from_str(payload).map_err(|err| {
        OpsError::Message(format!(
            "invalid PM snapshot for wave/{wave}; run `lf pm sync`: {err}"
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
    store
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
    let wave = resolve_wave(options.wave.as_deref())?;
    let wave_dir = repo.join("wave").join(&wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{wave}/"
        )));
    }

    let provider = resolve_provider(repo, &wave)?;
    let existing_initiative = read_initiative(repo, &wave, provider);
    let existing_team = read_team(repo, &wave, provider);

    let explicit_team = options.team_key.is_some() || options.team_name.is_some();

    // Full no-op fast path: both bindings present and the caller did not ask
    // to adopt a different team. An explicit team selection is a rebind.
    if let (Some(initiative_id), Some(team_id)) =
        (existing_initiative.as_ref(), existing_team.as_ref())
    {
        if !explicit_team {
            progress.status(&format!(
                "wave/{wave} already linked to {provider} Initiative {initiative_id} and team {team_id}"
            ));
            return Ok(PmInitResult {
                wave,
                initiative_id: initiative_id.clone(),
                created: false,
                team_id: team_id.clone(),
                team_key: None,
                team_created: false,
            });
        }
    }

    let summary = wave_summary(repo, &wave)?;
    let title = title_case(&wave);
    let client = build_client(repo, provider, resolve_team(repo, &wave, provider)).await?;

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

    // Team: an explicit key/name rebinds an existing Wave; otherwise keep its
    // binding or create the default one when missing.
    let resolve_requested_team = explicit_team || existing_team.is_none();
    let (team_id, team_key, team_created) = if resolve_requested_team {
        let name = options.team_name.clone().unwrap_or_else(|| title.clone());
        let key = options
            .team_key
            .clone()
            .unwrap_or_else(|| default_team_key(&wave));
        progress.status(&format!(
            "resolving {provider} team `{name}` (key {key}) for wave/{wave}"
        ));
        let team = client.ensure_team(&name, &key).await.map_err(pm_to_ops)?;
        progress.status(&format!(
            "{} {provider} team {} (key {})",
            if team.created { "created" } else { "adopted" },
            team.id,
            team.key
        ));
        (team.id, Some(team.key), team.created)
    } else {
        (
            existing_team
                .clone()
                .expect("an unrequested existing team was checked above"),
            None,
            false,
        )
    };
    let team_changed = existing_team.as_deref() != Some(team_id.as_str());
    if team_changed {
        write_team_to_goal(repo, &wave, provider, &team_id)?;
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
        team_id,
        team_key,
        team_created,
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

pub fn pm_create_task_idempotent(
    repo: &Path,
    wave: &str,
    project_slug: &str,
    title: &str,
    marker: &str,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    block_on_pm(pm_create_task_idempotent_async(
        repo,
        wave,
        project_slug,
        title,
        marker,
        progress,
    ))
}

async fn pm_create_task_idempotent_async(
    repo: &Path,
    wave: &str,
    project_slug: &str,
    title: &str,
    marker: &str,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    require_creation_team(repo, wave, resolve_provider(repo, wave)?)?;
    let ctx = resolve_context(repo, wave).await?;
    let projects = checked_projects(&ctx.client, &ctx.initiative, wave).await?;
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
        description: marker.to_string(),
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

pub(crate) async fn pm_update_async(
    repo: &Path,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let wave = resolve_wave(options.wave.as_deref())?;
    // A create (no `--id`) must bind an explicit team; an update on an existing
    // issue does not attach to a team, so it stays team-agnostic.
    if options.id.is_none() {
        require_creation_team(repo, &wave, resolve_provider(repo, &wave)?)?;
    }
    let ctx = resolve_context(repo, &wave).await?;
    let result = apply_update(&wave, options.project.as_deref(), &ctx, options, progress).await?;
    progress.status(&format!("refreshing local PM snapshot for wave/{wave}"));
    refresh_pm_snapshot(repo, &wave, &ctx).await?;
    Ok(result)
}

/// Reopen one PM issue: move it from its completed workflow state back to the
/// team's default active state, then refresh the local snapshot. The Task
/// repair path calls this when a Task was prematurely completed while its PR or
/// required review gates were still open. Idempotent at the Linear layer: a
/// second reopen of an already-open issue is a no-op state transition.
pub(crate) async fn pm_reopen_task_async(
    repo: &Path,
    wave: &str,
    item_id: &str,
    progress: &impl Progress,
) -> OpsResult<()> {
    let ctx = resolve_context(repo, wave).await?;
    progress.status(&format!("reopening {} task {item_id}", ctx.provider));
    ctx.client.reopen_item(item_id).await.map_err(pm_to_ops)?;
    refresh_pm_snapshot(repo, wave, &ctx).await?;
    Ok(())
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
    link_pr_with_client(&ctx.client, request, prior).await
}

async fn link_pr_with_client(
    client: &PmClient,
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

    let mut results = Vec::new();
    for wave in waves {
        let row = read_pm_snapshot(repo, &wave).await?;
        let snapshot = decode_snapshot(&wave, &row.payload)?;
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
    let mut matches = Vec::new();
    for wave in list_pm_waves(repo)? {
        let ctx = resolve_context(repo, &wave).await?;
        for project in checked_projects(&ctx.client, &ctx.initiative, &wave).await? {
            let items = ctx
                .client
                .list_items(&project.id)
                .await
                .map_err(pm_to_ops)?;
            for item in items {
                if item.id == issue || item.identifier.eq_ignore_ascii_case(issue) {
                    matches.push(PmResolvedTask {
                        wave: wave.clone(),
                        initiative_id: ctx.initiative.clone(),
                        project: project.clone(),
                        item,
                    });
                }
            }
        }
    }
    match matches.len() {
        0 => Err(OpsError::Message(format!(
            "Linear task {issue:?} does not belong to a known Loopflow Project and Wave"
        ))),
        1 => Ok(matches.pop().expect("one task match")),
        count => Err(OpsError::Message(format!(
            "Linear task {issue:?} belongs to {count} known Loopflow Waves; repair PM ownership before running it"
        ))),
    }
}

pub fn pm_resolve_project(repo: &Path, project_id: &str) -> OpsResult<PmResolvedProject> {
    block_on_pm(pm_resolve_project_async(repo, project_id))
}

async fn pm_resolve_project_async(repo: &Path, project_id: &str) -> OpsResult<PmResolvedProject> {
    let mut matches = Vec::new();
    for wave in list_pm_waves(repo)? {
        let ctx = resolve_context(repo, &wave).await?;
        for project in checked_projects(&ctx.client, &ctx.initiative, &wave).await? {
            if project.id == project_id {
                matches.push(PmResolvedProject {
                    wave: wave.clone(),
                    initiative_id: ctx.initiative.clone(),
                    project,
                });
            }
        }
    }
    match matches.len() {
        0 => Err(OpsError::Message(format!(
            "Linear Project {project_id:?} does not belong to a known Loopflow Wave"
        ))),
        1 => Ok(matches.pop().expect("one project match")),
        count => Err(OpsError::Message(format!(
            "Linear Project {project_id:?} belongs to {count} known Loopflow Waves; each Project must belong to exactly one Wave"
        ))),
    }
}

// ── reteam ──────────────────────────────────────────────────────────

/// How `reteam` treats one issue. Pure classification, so it is unit-tested
/// without a live Linear.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReteamClass {
    /// Already carries the target team key → skip the move; only reconcile a
    /// stale cached Task identifier.
    Already,
    /// An active Task Run owns it → defer until the Run stops.
    Defer(String),
    /// On the legacy team and not being written → move onto the wave's team.
    Move,
}

#[derive(Debug, Clone, Copy)]
struct ReteamWriterState<'a> {
    identifier: &'a str,
    run_id: Option<&'a str>,
}

impl<'a> From<&'a TaskWriterState> for ReteamWriterState<'a> {
    fn from(state: &'a TaskWriterState) -> Self {
        Self {
            identifier: &state.identifier,
            run_id: state.run.as_ref().map(|run| run.id.as_str()),
        }
    }
}

impl ReteamWriterState<'_> {
    fn protection_reason(self) -> Option<String> {
        self.run_id.map(|run_id| format!("active Run {run_id}"))
    }
}

/// Every foreign-team issue moves — completed included — unless a Task Run can
/// still write the old identifier back. Completion is not a shield: a Project
/// cannot narrow to one team while any of its issues (completed or not) stay on
/// the legacy team, so completed issues migrate like the rest. Protection keys on
/// an active Run, not on Linear completion. Open Work without a Run is safe to
/// renumber once its cached identifier is reconciled.
fn classify_reteam_item(
    item: &PmItem,
    team_key: &str,
    writer: Option<ReteamWriterState<'_>>,
) -> ReteamClass {
    let already = identifier_has_team_prefix(&item.identifier, team_key);
    let identifier_needs_update = writer.is_some_and(|writer| writer.identifier != item.identifier);
    if let Some(reason) = writer
        .filter(|_| !already || identifier_needs_update)
        .and_then(ReteamWriterState::protection_reason)
    {
        return ReteamClass::Defer(reason);
    }
    if already {
        return ReteamClass::Already;
    }
    ReteamClass::Move
}

/// Whether an identifier already belongs to the team keyed by `team_key`. The
/// trailing `-` guards against a prefix collision (`PRD-` must not match a
/// `PRODUCT-1` identifier).
fn identifier_has_team_prefix(identifier: &str, team_key: &str) -> bool {
    let prefix = format!("{}-", team_key.trim().to_ascii_uppercase());
    identifier.trim().to_ascii_uppercase().starts_with(&prefix)
}

/// Whether a Project's resolved ownership differs from exactly the wave's team.
/// `None` means the read did not resolve teams (an older snapshot) — unknown,
/// not a mismatch, so no false positive. Empty, foreign, and multi-team sets all
/// need repair because one Project belongs to one Wave-owned team.
fn project_needs_reteam(bound_team: &str, project_team_ids: Option<&[String]>) -> bool {
    match project_team_ids {
        None => false,
        Some(team_ids) => team_ids.len() != 1 || team_ids[0] != bound_team,
    }
}

/// Refuse the whole apply when any Task Run can still write the old identifier.
/// The plan is read-only up to this point, so this preserves the hierarchy rather
/// than moving its Project and idle siblings around the protected Task.
fn ensure_reteam_apply_safe(deferrals: &[PmReteamDeferral]) -> OpsResult<()> {
    if deferrals.is_empty() {
        return Ok(());
    }

    let protected = deferrals
        .iter()
        .map(|deferral| {
            format!(
                "{} `{}` ({})",
                deferral.identifier, deferral.title, deferral.reason
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(OpsError::Message(format!(
        "cannot apply reteam while {} Task Run(s) can still write the old identifier: {protected}. No Projects or Tasks were moved; stop those Runs and rerun the dry-run.",
        deferrals.len()
    )))
}

/// Moving an issue to `target_team` keeps it in its Project only when that
/// Project already carries the target team. Refuse unresolved and missing-team
/// Projects before the first store or provider mutation.
fn ensure_move_projects_carry_target_team(
    projects: &[PmProject],
    moving_project_ids: &BTreeSet<String>,
    target_team: &str,
) -> OpsResult<()> {
    let unsafe_projects = projects
        .iter()
        .filter(|project| moving_project_ids.contains(&project.id))
        .filter_map(|project| match project.team_ids.as_deref() {
            Some(team_ids) if team_ids.iter().any(|team| team == target_team) => None,
            Some(team_ids) => Some(format!(
                "`{}` (teams [{}])",
                project.name,
                team_ids.join(", ")
            )),
            None => Some(format!("`{}` (teams unresolved)", project.name)),
        })
        .collect::<Vec<_>>();
    if unsafe_projects.is_empty() {
        return Ok(());
    }

    Err(OpsError::Message(format!(
        "cannot apply reteam because Project(s) containing issues to move do not already carry target team {target_team}: {}. No Projects or Tasks were moved; attach the target team and rerun the dry-run.",
        unsafe_projects.join("; ")
    )))
}

#[derive(Debug)]
struct ReteamIdentifierUpdate {
    issue_id: String,
    old_identifier: String,
    new_identifier: String,
}

struct ResolvedReteamContext {
    wave: String,
    team_id: String,
    team_key: String,
    pm: PmContext,
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

async fn resolve_reteam_context(
    repo: &Path,
    wave: Option<&str>,
) -> OpsResult<ResolvedReteamContext> {
    let wave = resolve_wave(wave)?;
    let provider = resolve_provider(repo, &wave)?;
    let team_id = read_team(repo, &wave, provider).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.{}`. \
             Run `lf pm init --wave {wave} --team-key <KEY>` to bind its team first.",
            provider.team_key()
        ))
    })?;
    let initiative = read_initiative(repo, &wave, provider).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.{}`. \
             Run `lf pm init --wave {wave}` to connect its Linear Initiative.",
            provider.initiative_key()
        ))
    })?;
    let client = build_client(repo, provider, Some(team_id.clone())).await?;
    let team_key = client.team_key(&team_id).await.map_err(pm_to_ops)?;
    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open task registry: {err}")))?;

    Ok(ResolvedReteamContext {
        wave,
        team_id,
        team_key,
        pm: PmContext {
            client,
            provider,
            initiative,
        },
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
    let resolved = resolve_reteam_context(repo, options.wave.as_deref()).await?;
    apply_or_plan_reteam(
        &resolved.pm,
        &resolved.store,
        repo,
        &resolved.wave,
        &resolved.team_id,
        &resolved.team_key,
        options.apply,
        progress,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn apply_or_plan_reteam(
    ctx: &PmContext,
    store: &Store,
    repo: &Path,
    wave: &str,
    team_id: &str,
    team_key: &str,
    apply: bool,
    progress: &impl Progress,
) -> OpsResult<PmReteamResult> {
    progress.status(&format!(
        "listing wave/{wave} issues under {} Initiative {}",
        ctx.provider, ctx.initiative
    ));
    let projects = ctx
        .client
        .list_projects(&ctx.initiative)
        .await
        .map_err(pm_to_ops)?;

    let mut project_moves = Vec::new();
    let mut moves = Vec::new();
    let mut deferrals = Vec::new();
    let mut moving_project_ids = BTreeSet::new();
    let mut identifier_updates = Vec::new();
    let mut already = 0usize;
    let mut task_updates = 0usize;

    for project in &projects {
        // A Project without exactly the wave's team is moved onto it. A
        // multi-team Project is repaired too: ownership is singular, not merely
        // membership. An unresolved team set (`None`, older snapshot) is skipped,
        // never guessed.
        if project_needs_reteam(team_id, project.team_ids.as_deref()) {
            project_moves.push(PmReteamProjectMove {
                id: project.id.clone(),
                name: project.name.clone(),
                from_teams: project.team_ids.clone().unwrap_or_default(),
            });
        }
        let items = ctx
            .client
            .list_items(&project.id)
            .await
            .map_err(pm_to_ops)?;
        for item in items {
            // Resolve protection from stable Task Work and its active Run.
            let writer = store
                .task_writer_state(&item.id)
                .await
                .map_err(|err| OpsError::Message(format!("failed to read task registry: {err}")))?;
            match classify_reteam_item(
                &item,
                team_key,
                writer.as_ref().map(ReteamWriterState::from),
            ) {
                ReteamClass::Already => {
                    already += 1;
                    if let Some(writer) =
                        writer.filter(|writer| writer.identifier != item.identifier)
                    {
                        identifier_updates.push(ReteamIdentifierUpdate {
                            issue_id: item.id,
                            old_identifier: writer.identifier,
                            new_identifier: item.identifier,
                        });
                    }
                }
                ReteamClass::Defer(reason) => deferrals.push(PmReteamDeferral {
                    identifier: item.identifier,
                    title: item.name,
                    reason,
                }),
                ReteamClass::Move => {
                    moving_project_ids.insert(project.id.clone());
                    moves.push(PmReteamMove {
                        id: item.id,
                        old_identifier: item.identifier,
                        title: item.name,
                        new_identifier: None,
                    });
                }
            }
        }
    }

    if apply {
        ensure_reteam_apply_safe(&deferrals)?;
        ensure_move_projects_carry_target_team(&projects, &moving_project_ids, team_id)?;

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
            let comment_bodies = ctx
                .client
                .issue_comment_bodies(&mv.id)
                .await
                .map_err(pm_to_ops)?;
            if !comment_bodies.iter().any(|body| body.contains(&marker)) {
                ctx.client
                    .comment(&mv.id, &reteam_comment_body(&mv.old_identifier, team_key))
                    .await
                    .map_err(pm_to_ops)?;
            }
            progress.status(&format!(
                "moving {} into team {team_key}",
                mv.old_identifier
            ));
            let new_identifier = ctx
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

        // Linear refuses to remove a team from a Project while that team's
        // issues remain in it, so narrowing is necessarily the final provider
        // phase. `teamIds` is a set replacement: exactly the wave's team.
        for pm in &project_moves {
            progress.status(&format!(
                "narrowing Project `{}` onto team {team_key}",
                pm.name
            ));
            ctx.client
                .move_project_to_team(&pm.id, team_id)
                .await
                .map_err(pm_to_ops)?;
        }

        // Always refresh after a successful explicit apply. A prior run may have
        // completed every provider mutation and then failed while refreshing;
        // its retry classifies everything as Already and must still repair the
        // local snapshot.
        let snapshot = fetch_pm_snapshot(wave, ctx).await?;
        store_pm_snapshot_with_store(repo, wave, ctx, &snapshot, store).await?;
    }

    Ok(PmReteamResult {
        wave: wave.to_string(),
        team_id: team_id.to_string(),
        team_key: team_key.to_string(),
        applied: apply,
        project_moves,
        moves,
        deferrals,
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
        // Every wave must bind a team explicitly so creation never falls back to
        // the shared team. Sharing one team across a product's waves (each with
        // its own Initiative, as Cadenza does) is allowed — the Task prefix is
        // per team, not per wave — so no "waves share a team" diagnostic here.
        if read_team(repo, wave, provider).is_none() {
            diagnostics.push(format!(
                "wave/{wave} has no `pm.{}`; run `lf pm init --wave {wave} --team-key <KEY>` \
                 so its work lands on an explicit team",
                provider.team_key()
            ));
        }
    }

    let provider = provider_by_kind
        .values()
        .next()
        .copied()
        .unwrap_or(PmProviderKind::Linear);
    // Machine-wide diff over many waves; reads follow Initiative → Project →
    // Issue and are team-agnostic, so no per-wave team is resolved here.
    let client = build_client(repo, provider, None).await?;
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
        let bound_team = read_team(repo, wave, provider);
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
            // A Project stranded on a foreign team (or simultaneously attached
            // to the bound and a legacy team) violates singular Wave ownership.
            if let Some(team_id) = &bound_team {
                if project_needs_reteam(team_id, project.team_ids.as_deref()) {
                    let teams = project.team_ids.as_deref().unwrap_or_default().join(", ");
                    diagnostics.push(format!(
                        "Linear Project `{}` ({}) in wave/{wave} belongs to team(s) [{teams}], \
                         not the wave's team {team_id}; run `lf pm reteam --wave {wave}` to move it",
                        project.name, project.id
                    ));
                }
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
        // Stranded issues: a team-bound wave whose open issues still carry a
        // foreign prefix are `reteam` candidates not yet moved.
        if let Some(team_id) = &bound_team {
            let team_key = client.team_key(team_id).await.map_err(pm_to_ops)?;
            let stranded = snapshot
                .items
                .iter()
                .filter(|item| {
                    !item.completed && !identifier_has_team_prefix(&item.identifier, &team_key)
                })
                .count();
            if stranded > 0 {
                diagnostics.push(format!(
                    "wave/{wave} has {stranded} open issue(s) not in team {team_key}; \
                     run `lf pm reteam --wave {wave}` to plan their migration"
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
    let wave = resolve_wave(options.wave.as_deref())?;
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
    let wave = resolve_wave(options.wave.as_deref())?;
    // Creating a Project (no `--project` slug to update) must bind an explicit
    // team; updating an existing Project does not move it between teams.
    if options.project.is_none() {
        require_creation_team(repo, &wave, resolve_provider(repo, &wave)?)?;
    }
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
        let linear_name = linear_project_name(&wave, &name);
        ctx.client
            .update_project(&project.id, &linear_name, &options.definition, &krs)
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
        let linear_name = linear_project_name(&wave, &name);
        let id = ctx
            .client
            .create_project(&ctx.initiative, &linear_name, &options.definition, &krs)
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

fn write_team_to_goal(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
    team_id: &str,
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
            serde_yaml_ng::Value::String(provider.team_key().to_string()),
            serde_yaml_ng::Value::String(team_id.to_string()),
        );
        map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));
        Ok(())
    })
    .map_err(OpsError::Message)
}

/// A default team key (Task prefix) derived from the wave name: the first three
/// alphanumeric characters, uppercased. `--team-key` overrides it.
fn default_team_key(wave: &str) -> String {
    let key: String = wave
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
    Ok(crate::engine::wave_config::read_wave_summary(repo, wave)?)
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
    let mut projects = client.list_projects(initiative).await.map_err(pm_to_ops)?;
    for project in &mut projects {
        project.name = canonical_project_name(wave, &project.name).to_string();
        project.slug = crate::pm::project_slug(&project.name);
    }
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

fn linear_project_name(wave: &str, canonical_name: &str) -> String {
    format!("{} — {}", title_case(wave), canonical_name.trim())
}

fn canonical_project_name<'a>(wave: &str, linear_name: &'a str) -> &'a str {
    let prefix = format!("{} — ", title_case(wave));
    linear_name
        .strip_prefix(&prefix)
        .unwrap_or(linear_name)
        .trim()
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
    use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::task::{
        Observation, PmWritebackState, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr, TaskPrId,
        TaskSessionId,
    };
    use crate::wave::Wave;
    use axum::http::StatusCode;
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use time::OffsetDateTime;

    fn linear_test_ctx(base_url: String, initiative: &str) -> PmContext {
        PmContext {
            client: PmClient::Linear(crate::pm::linear::LinearClient::with_base_url(
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

    fn reteam_project_node(id: &str, name: &str, team_ids: &[&str]) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "description": "",
            "content": "## Definition\n\nA measured bet.\n\n## KRs\n",
            "initiatives": { "nodes": [{ "id": "initiative-123" }] },
            "teams": {
                "nodes": team_ids
                    .iter()
                    .map(|id| json!({ "id": id }))
                    .collect::<Vec<_>>()
            }
        })
    }

    fn reteam_issue_node(id: &str, identifier: &str, completed: bool) -> serde_json::Value {
        json!({
            "id": id,
            "identifier": identifier,
            "url": null,
            "title": format!("Task {identifier}"),
            "description": "",
            "prioritySortOrder": 0.0,
            "sortOrder": 0.0,
            "state": { "type": if completed { "completed" } else { "unstarted" } }
        })
    }

    fn issue_observation_response(comments: &[&str]) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "issue": {
                "updatedAt": "2026-07-17T00:00:00.000Z",
                "title": "Legacy task",
                "description": "",
                "comments": {
                    "nodes": comments
                        .iter()
                        .enumerate()
                        .map(|(index, body)| json!({
                            "id": format!("comment-{index}"),
                            "body": body,
                            "user": null
                        }))
                        .collect::<Vec<_>>()
                }
            } } }),
        )
    }

    fn reteam_comment_create_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "commentCreate": { "comment": { "id": id } } } }),
        )
    }

    fn issue_team_move_response(id: &str, identifier: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "issueUpdate": { "issue": {
                "id": id,
                "identifier": identifier
            } } } }),
        )
    }

    fn project_team_move_response(id: &str) -> QueuedResponse {
        json_response(
            StatusCode::OK,
            json!({ "data": { "projectUpdate": { "project": { "id": id } } } }),
        )
    }

    async fn reteam_test_store() -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("temp store directory");
        let store = open_store(&crate::store::StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .expect("open reteam store");
        (directory, store)
    }

    async fn seed_reteam_task_session(
        store: &Store,
        issue_id: &str,
        identifier: &str,
    ) -> TaskSessionId {
        let now = OffsetDateTime::now_utc();
        let wave = Wave::new(
            WaveId::new(),
            "infrastructure".to_string(),
            "/repo".to_string(),
        );
        store.create_wave(&wave).await.expect("create wave");
        let project_snapshot = LinearProjectSnapshot {
            id: LinearProjectId::new("project-uuid").expect("project id"),
            slug: "developer-efficiency".to_string(),
            name: "Developer Efficiency".to_string(),
            prompt_context: "Keep development fast.".to_string(),
        };
        let project = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: project_snapshot.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            status: ProjectSessionStatus::Waiting,
            status_reason: "waiting".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store
            .create_project_session(&project)
            .await
            .expect("create project session");

        let session_id = TaskSessionId::new();
        let session = TaskSession {
            id: session_id.clone(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(issue_id).expect("issue id"),
                    identifier: identifier.to_string(),
                    title: format!("Task {identifier}"),
                    description: String::new(),
                },
                project: project_snapshot,
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id,
            status: TaskSessionStatus::Waiting,
            status_reason: "awaiting review".to_string(),
            status_at: now,
            worktree: PathBuf::from(format!("/repo.{identifier}")),
            workspace_slug: identifier.to_ascii_lowercase(),
            lifecycle: TaskLifecyclePlan::standard("task"),
            lifecycle_phase: TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: session.workspace_slug.clone(),
            branch: format!("jack/{}", session.workspace_slug),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        store
            .create_task_session(&session, &pr)
            .await
            .expect("create task session");
        session_id
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
    fn team_write_persists_alongside_the_initiative_binding() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_initiative: \"initiative-1\"\n",
        );

        write_team_to_goal(repo.path(), "product", PmProviderKind::Linear, "team-prd")
            .expect("write team");

        let pm = read_wave_pm_config(repo.path(), "product").expect("pm config");
        // Both bindings coexist; the team write never disturbs the Initiative.
        assert_eq!(pm.linear_initiative.as_deref(), Some("initiative-1"));
        assert_eq!(pm.linear_team.as_deref(), Some("team-prd"));
        assert_eq!(
            read_team(repo.path(), "product", PmProviderKind::Linear).as_deref(),
            Some("team-prd")
        );
    }

    #[test]
    fn team_write_rebinds_without_disturbing_the_initiative() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_initiative: \"initiative-1\"\n  linear_team: \"team-shared\"\n",
        );

        write_team_to_goal(repo.path(), "product", PmProviderKind::Linear, "team-prd")
            .expect("replace team");

        let pm = read_wave_pm_config(repo.path(), "product").expect("pm config");
        assert_eq!(pm.linear_initiative.as_deref(), Some("initiative-1"));
        assert_eq!(pm.linear_team.as_deref(), Some("team-prd"));
    }

    #[test]
    fn resolve_team_prefers_wave_binding_over_config() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_team: \"team-prd\"\n",
        );

        // A wave with its own binding ignores any global fallback.
        assert_eq!(
            resolve_team(repo.path(), "product", PmProviderKind::Linear).as_deref(),
            Some("team-prd")
        );
        // An unbound wave has no team here (no global config in the temp repo),
        // which is safe: reads are team-agnostic and only creation needs one.
        assert_eq!(
            resolve_team(repo.path(), "unbound", PmProviderKind::Linear),
            None
        );
    }

    #[test]
    fn require_creation_team_fails_closed_on_an_unbound_wave() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_team: \"team-prd\"\n",
        );

        // A bound wave yields its explicit team — never a fallback.
        assert_eq!(
            require_creation_team(repo.path(), "product", PmProviderKind::Linear)
                .expect("bound wave resolves a team"),
            "team-prd"
        );

        // An unbound wave errors with the `lf pm init` recovery instead of
        // silently borrowing the shared team.
        let err = require_creation_team(repo.path(), "unbound", PmProviderKind::Linear)
            .expect_err("unbound wave fails closed");
        let message = err.to_string();
        assert!(message.contains("lf pm init --wave unbound"));
        assert!(message.contains("linear_team"));
    }

    #[test]
    fn default_team_key_derives_from_wave_name() {
        assert_eq!(default_team_key("product"), "PRO");
        assert_eq!(default_team_key("infrastructure"), "INF");
        assert_eq!(default_team_key("intelligence"), "INT");
        assert_eq!(default_team_key("x"), "LF");
    }

    #[test]
    fn linear_project_prefix_is_presentation_only() {
        assert_eq!(
            linear_project_name("product", "Loopflow API"),
            "Product — Loopflow API"
        );
        assert_eq!(
            canonical_project_name("product", "Product — Loopflow API"),
            "Loopflow API"
        );
        assert_eq!(
            crate::pm::project_slug(canonical_project_name("product", "Product — Loopflow API")),
            "loopflow-api"
        );
        assert_eq!(
            canonical_project_name("product", "Unprefixed migration input"),
            "Unprefixed migration input"
        );
    }

    fn reteam_item(identifier: &str, completed: bool) -> PmItem {
        PmItem {
            id: format!("uuid-of-{identifier}"),
            identifier: identifier.to_string(),
            url: None,
            name: format!("Task {identifier}"),
            description: String::new(),
            rank: 0,
            completed,
            project: None,
            assignee: None,
        }
    }

    fn reteam_writer<'a>(identifier: &'a str, run_id: Option<&'a str>) -> ReteamWriterState<'a> {
        ReteamWriterState { identifier, run_id }
    }

    #[test]
    fn identifier_prefix_needs_the_trailing_dash() {
        assert!(identifier_has_team_prefix("PRD-4", "PRD"));
        assert!(identifier_has_team_prefix("prd-4", "prd"));
        // A shared leading substring must not false-match.
        assert!(!identifier_has_team_prefix("PRODUCT-1", "PRD"));
        assert!(!identifier_has_team_prefix("W2-155", "PRD"));
    }

    #[test]
    fn project_needs_reteam_requires_exactly_one_bound_team() {
        // Unknown teams (older snapshot) → never a mismatch.
        assert!(!project_needs_reteam("team-prd", None));
        // Exactly the bound team → owned.
        assert!(!project_needs_reteam(
            "team-prd",
            Some(&["team-prd".to_string()])
        ));
        // The bound team plus a legacy team still violates one-team ownership.
        assert!(project_needs_reteam(
            "team-prd",
            Some(&["team-prd".to_string(), "team-shared".to_string()])
        ));
        // Bound team absent → stranded.
        assert!(project_needs_reteam(
            "team-prd",
            Some(&["team-shared".to_string()])
        ));
        // Belongs to no team at all → stranded.
        assert!(project_needs_reteam("team-prd", Some(&[])));
    }

    #[test]
    fn reteam_apply_stops_before_mutation_when_any_task_run_is_active() {
        let deferrals = vec![
            PmReteamDeferral {
                identifier: "W2-157".to_string(),
                title: "Unify practice targets".to_string(),
                reason: "active Run run-one".to_string(),
            },
            PmReteamDeferral {
                identifier: "W2-166".to_string(),
                title: "Resolve team ownership".to_string(),
                reason: "active Run run-two".to_string(),
            },
        ];

        let error = ensure_reteam_apply_safe(&deferrals).expect_err("apply must stop");
        let message = error.to_string();
        assert!(message.contains("W2-157 `Unify practice targets` (active Run run-one)"));
        assert!(message.contains("W2-166 `Resolve team ownership` (active Run run-two)"));
        assert!(message.contains("No Projects or Tasks were moved"));
    }

    #[test]
    fn reteam_classifies_completed_move_defer_and_skip() {
        // Completion does not exempt a foreign-team issue from migration.
        assert_eq!(
            classify_reteam_item(&reteam_item("W2-1", true), "PRD", None),
            ReteamClass::Move
        );
        // A completed issue with an active Run remains protected.
        assert!(matches!(
            classify_reteam_item(
                &reteam_item("W2-5", true),
                "PRD",
                Some(reteam_writer("W2-5", Some("run-one")))
            ),
            ReteamClass::Defer(_)
        ));
        // Already in the target team → skip (idempotency).
        assert_eq!(
            classify_reteam_item(&reteam_item("PRD-7", false), "PRD", None),
            ReteamClass::Already
        );
        // Open with an active Run → defer with the Run as reason.
        assert_eq!(
            classify_reteam_item(
                &reteam_item("W2-2", false),
                "PRD",
                Some(reteam_writer("W2-2", Some("run-two")))
            ),
            ReteamClass::Defer("active Run run-two".to_string())
        );
        // Open Work without a Run → move and reconcile its display id.
        assert_eq!(
            classify_reteam_item(
                &reteam_item("W2-3", false),
                "PRD",
                Some(reteam_writer("W2-3", None))
            ),
            ReteamClass::Move
        );
        // Open with no Session → move.
        assert_eq!(
            classify_reteam_item(&reteam_item("W2-4", false), "PRD", None),
            ReteamClass::Move
        );
    }

    #[test]
    fn reteam_moving_issues_requires_the_target_team_on_every_project() {
        let project = |id: &str, name: &str, team_ids: Option<Vec<&str>>| PmProject {
            id: id.to_string(),
            slug: crate::pm::project_slug(name),
            name: name.to_string(),
            summary: String::new(),
            definition: String::new(),
            krs: Vec::new(),
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: team_ids.map(|ids| ids.into_iter().map(str::to_string).collect()),
        };
        let projects = vec![
            project("safe", "Safe", Some(vec!["team-prd", "team-shared"])),
            project("missing", "Missing target", Some(vec!["team-shared"])),
            project("unknown", "Unknown teams", None),
        ];

        ensure_move_projects_carry_target_team(
            &projects,
            &BTreeSet::from(["safe".to_string()]),
            "team-prd",
        )
        .expect("safe Project carries target team");

        let error = ensure_move_projects_carry_target_team(
            &projects,
            &BTreeSet::from(["missing".to_string(), "unknown".to_string()]),
            "team-prd",
        )
        .expect_err("missing and unresolved target teams fail closed");
        let message = error.to_string();
        assert!(message.contains("`Missing target` (teams [team-shared])"));
        assert!(message.contains("`Unknown teams` (teams unresolved)"));
        assert!(message.contains("No Projects or Tasks were moved"));
    }

    #[tokio::test]
    async fn reteam_apply_moves_completed_issues_before_narrowing_projects() {
        let initial_project = reteam_project_node(
            "project-1",
            "Developer Efficiency",
            &["team-prd", "team-shared"],
        );
        let completed_issue = reteam_issue_node("issue-1", "W2-41", true);
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([initial_project])),
            issues_response(json!([completed_issue])),
            issue_observation_response(&[]),
            reteam_comment_create_response("comment-1"),
            issue_team_move_response("issue-1", "PRD-7"),
            project_team_move_response("project-1"),
            projects_response(json!([reteam_project_node(
                "project-1",
                "Developer Efficiency",
                &["team-prd"],
            )])),
            issues_response(json!([reteam_issue_node("issue-1", "PRD-7", true)])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let (_directory, store) = reteam_test_store().await;

        let result = apply_or_plan_reteam(
            &ctx,
            &store,
            Path::new("/repo"),
            "product",
            "team-prd",
            "PRD",
            true,
            &NullProgress,
        )
        .await
        .expect("reteam apply succeeds");

        assert_eq!(result.moves.len(), 1);
        assert_eq!(result.moves[0].old_identifier, "W2-41");
        assert_eq!(result.moves[0].new_identifier.as_deref(), Some("PRD-7"));

        let requests = requests.lock().await;
        let bodies = requests
            .iter()
            .map(|request| {
                serde_json::from_str::<serde_json::Value>(&request.body)
                    .expect("GraphQL request body")
            })
            .collect::<Vec<_>>();
        let comment = bodies
            .iter()
            .position(|body| {
                body["query"]
                    .as_str()
                    .is_some_and(|query| query.contains("mutation CreateComment"))
            })
            .expect("traceability comment");
        let issue_move = bodies
            .iter()
            .position(|body| {
                body["query"]
                    .as_str()
                    .is_some_and(|query| query.contains("mutation MoveIssueToTeam"))
            })
            .expect("completed issue move");
        let project_move = bodies
            .iter()
            .position(|body| {
                body["query"]
                    .as_str()
                    .is_some_and(|query| query.contains("mutation MoveProjectToTeam"))
            })
            .expect("Project narrow");
        assert!(
            comment < issue_move,
            "old identifier is recorded before move"
        );
        assert!(
            issue_move < project_move,
            "every issue moves before Project narrowing"
        );
        assert_eq!(bodies[issue_move]["variables"]["id"], "issue-1");
        assert!(bodies[comment]["variables"]["body"]
            .as_str()
            .expect("comment body")
            .contains("was W2-41; moving onto team PRD"));
    }

    #[tokio::test]
    async fn reteam_apply_refuses_unsafe_project_before_any_mutation() {
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([reteam_project_node(
                "project-1",
                "Developer Efficiency",
                &["team-shared"],
            )])),
            issues_response(json!([reteam_issue_node("issue-1", "W2-41", false)])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");
        let (_directory, store) = reteam_test_store().await;

        let error = apply_or_plan_reteam(
            &ctx,
            &store,
            Path::new("/repo"),
            "product",
            "team-prd",
            "PRD",
            true,
            &NullProgress,
        )
        .await
        .expect_err("missing target team refuses apply");

        assert!(error.to_string().contains("Developer Efficiency"));
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .all(|request| !request.body.contains("mutation")));
    }

    #[tokio::test]
    async fn reteam_apply_refuses_completed_issue_with_an_active_run() {
        let (_directory, store) = reteam_test_store().await;
        let session_id = seed_reteam_task_session(&store, "issue-live", "W2-42").await;
        let session = store
            .get_task_session(&session_id)
            .await
            .expect("read session")
            .expect("session exists");
        let work = store
            .work_for_child(&crate::child_session::ChildRef::Task(session.id.clone()))
            .await
            .expect("resolve Task Work");
        store
            .reserve_run(&work, crate::durable::RunTrigger::User)
            .await
            .expect("reserve active Run");
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([reteam_project_node(
                "project-1",
                "Developer Efficiency",
                &["team-prd", "team-shared"],
            )])),
            issues_response(json!([reteam_issue_node("issue-live", "W2-42", true)])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");

        let error = apply_or_plan_reteam(
            &ctx,
            &store,
            Path::new("/repo"),
            "product",
            "team-prd",
            "PRD",
            true,
            &NullProgress,
        )
        .await
        .expect_err("active Run refuses the whole apply");

        assert!(error.to_string().contains("W2-42"));
        assert!(error
            .to_string()
            .contains("No Projects or Tasks were moved"));
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 2);
        assert!(requests
            .iter()
            .all(|request| !request.body.contains("mutation")));
    }

    #[tokio::test]
    async fn reteam_retry_reuses_pre_move_comment_then_moves_and_rebinds() {
        let (_directory, store) = reteam_test_store().await;
        let session_id = seed_reteam_task_session(&store, "issue-legacy", "W2-9").await;
        let initial_project = || {
            reteam_project_node(
                "project-1",
                "Developer Efficiency",
                &["team-prd", "team-shared"],
            )
        };
        let legacy_issue = || reteam_issue_node("issue-legacy", "W2-9", false);

        let (base_url, first_requests) = test_server::spawn(vec![
            projects_response(json!([initial_project()])),
            issues_response(json!([legacy_issue()])),
            issue_observation_response(&[]),
            reteam_comment_create_response("comment-1"),
            json_response(
                StatusCode::OK,
                json!({ "errors": [{ "message": "issue move failed" }] }),
            ),
        ])
        .await;
        let first_ctx = linear_test_ctx(base_url, "initiative-123");
        let error = apply_or_plan_reteam(
            &first_ctx,
            &store,
            Path::new("/repo"),
            "product",
            "team-prd",
            "PRD",
            true,
            &NullProgress,
        )
        .await
        .expect_err("first move fails after comment");
        assert!(error.to_string().contains("issue move failed"));
        assert_eq!(
            store
                .get_task_session(&session_id)
                .await
                .expect("read session")
                .expect("session exists")
                .launch
                .issue
                .identifier,
            "W2-9"
        );

        let existing_comment = reteam_comment_body("W2-9", "PRD");
        let (base_url, second_requests) = test_server::spawn(vec![
            projects_response(json!([initial_project()])),
            issues_response(json!([legacy_issue()])),
            issue_observation_response(&[&existing_comment]),
            issue_team_move_response("issue-legacy", "PRD-9"),
            project_team_move_response("project-1"),
            projects_response(json!([reteam_project_node(
                "project-1",
                "Developer Efficiency",
                &["team-prd"],
            )])),
            issues_response(json!([reteam_issue_node("issue-legacy", "PRD-9", false,)])),
        ])
        .await;
        let second_ctx = linear_test_ctx(base_url, "initiative-123");
        let result = apply_or_plan_reteam(
            &second_ctx,
            &store,
            Path::new("/repo"),
            "product",
            "team-prd",
            "PRD",
            true,
            &NullProgress,
        )
        .await
        .expect("retry succeeds");

        assert_eq!(result.task_updates, 1);
        assert_eq!(
            store
                .get_task_session(&session_id)
                .await
                .expect("read session")
                .expect("session exists")
                .launch
                .issue
                .identifier,
            "PRD-9"
        );
        let first_requests = first_requests.lock().await;
        let second_requests = second_requests.lock().await;
        assert_eq!(
            first_requests
                .iter()
                .filter(|request| request.body.contains("mutation CreateComment"))
                .count(),
            1
        );
        assert_eq!(
            second_requests
                .iter()
                .filter(|request| request.body.contains("mutation CreateComment"))
                .count(),
            0,
            "retry must reuse this migration's existing comment"
        );
    }

    #[tokio::test]
    async fn reteam_already_moved_issue_only_rebinds_a_stale_session() {
        let (_directory, store) = reteam_test_store().await;
        let session_id = seed_reteam_task_session(&store, "issue-moved", "W2-10").await;
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([reteam_project_node(
                "project-1",
                "Developer Efficiency",
                &["team-prd"],
            )])),
            issues_response(json!([
                reteam_issue_node("issue-moved", "PRD-10", false),
                reteam_issue_node("issue-direct", "PRD-11", false)
            ])),
            projects_response(json!([reteam_project_node(
                "project-1",
                "Developer Efficiency",
                &["team-prd"],
            )])),
            issues_response(json!([
                reteam_issue_node("issue-moved", "PRD-10", false),
                reteam_issue_node("issue-direct", "PRD-11", false)
            ])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");

        let result = apply_or_plan_reteam(
            &ctx,
            &store,
            Path::new("/repo"),
            "product",
            "team-prd",
            "PRD",
            true,
            &NullProgress,
        )
        .await
        .expect("already-moved reconciliation succeeds");

        assert_eq!(result.already, 2);
        assert_eq!(result.task_updates, 1);
        assert!(result.moves.is_empty());
        assert_eq!(
            store
                .get_task_session(&session_id)
                .await
                .expect("read session")
                .expect("session exists")
                .launch
                .issue
                .identifier,
            "PRD-10"
        );
        let requests = requests.lock().await;
        assert_eq!(requests.len(), 4);
        assert!(requests
            .iter()
            .all(|request| !request.body.contains("mutation")));
    }

    #[test]
    fn reteam_protects_only_tasks_with_an_active_run() {
        assert_eq!(
            classify_reteam_item(
                &reteam_item("W2-9", false),
                "PRD",
                Some(reteam_writer("W2-9", None))
            ),
            ReteamClass::Move
        );
        assert_eq!(
            classify_reteam_item(
                &reteam_item("W2-9", false),
                "PRD",
                Some(reteam_writer("W2-9", Some("run-active")))
            ),
            ReteamClass::Defer("active Run run-active".to_string())
        );
    }

    #[test]
    fn reteam_reconciles_only_when_already_moved_work_has_no_run() {
        assert_eq!(
            classify_reteam_item(
                &reteam_item("PRD-8", false),
                "PRD",
                Some(reteam_writer("W2-9", None))
            ),
            ReteamClass::Already
        );
        assert!(matches!(
            classify_reteam_item(
                &reteam_item("PRD-8", false),
                "PRD",
                Some(reteam_writer("W2-9", Some("run-active")))
            ),
            ReteamClass::Defer(_)
        ));
    }

    #[test]
    fn duplicate_linear_project_slugs_are_drift() {
        let project = |id: &str, name: &str| PmProject {
            id: id.to_string(),
            slug: crate::pm::project_slug(name),
            name: name.to_string(),
            summary: String::new(),
            definition: String::new(),
            krs: Vec::new(),
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: None,
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
                team_ids: Some(vec!["team-prd".to_string()]),
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
        let client = linear_test_ctx(base_url, "initiative-1").client;

        let outcome = link_pr_with_client(
            &client,
            &link_request("Open · in review"),
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
        let client = linear_test_ctx(base_url, "initiative-1").client;
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
        let client = linear_test_ctx(base_url, "initiative-1").client;

        let degraded = link_pr_with_client(
            &client,
            &link_request("Open · in review"),
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
        let client = linear_test_ctx(base_url, "initiative-1").client;

        let healed =
            link_pr_with_client(&client, &link_request("Open · in review"), &degraded.ids).await;

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
            open_store(&crate::store::StorageConfig::sqlite(db_path))
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
            open_store(&crate::store::StorageConfig::sqlite(db_path))
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
