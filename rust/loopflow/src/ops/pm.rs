//! `lf pm` — read and write a wave's PM tasks directly in a provider.
//!
//! Linear is authoritative for the wave's project inventory, project specs, and
//! tasks. A wave maps to a Linear Initiative, each project maps to a Linear
//! Project, and each task maps to an Issue. Local project Markdown is an offline
//! cache and migration seed, never a second editable source of truth.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::Path;

use futures_util::future::try_join_all;

use crate::engine::config::load_config_or_default;
use crate::engine::wave_config::{read_wave_config, update_wave_goal_config, WavePmConfig};
use crate::lfd::pm::linear::LinearClient;
use crate::lfd::pm::{
    parse_project_content, project_slug, PmError, PmItem, PmItemCreate, PmItemUpdate, PmKr,
    PmLegacyItem, PmProject, PmProviderKind, PmResult, PmWave,
};
use crate::lfdb::{open_store, ProviderToken, Store};
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmShowResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub initiative: String,
    pub project: Option<String>,
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
    pub initiative_name: Option<String>,
    pub open: usize,
    pub total: usize,
    pub open_by_project: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmStatusResult {
    pub waves: Vec<PmWaveStatus>,
    pub stranded_waves: Vec<PmWave>,
}

#[derive(Debug, Clone, Default)]
pub struct PmSyncOptions {
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
        project: &LocalProject,
    ) -> PmResult<String> {
        match self {
            Self::Linear(client) => {
                client
                    .create_project(
                        initiative_id,
                        &project.name,
                        &project.summary,
                        &project.definition,
                        &project.krs,
                    )
                    .await
            }
        }
    }

    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        match self {
            Self::Linear(client) => client.list_items(project_id).await,
        }
    }

    async fn list_legacy_items(&self, project_id: &str) -> PmResult<Vec<PmLegacyItem>> {
        match self {
            Self::Linear(client) => client.list_legacy_items(project_id).await,
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

fn read_legacy_project(repo: &Path, wave: &str) -> Option<String> {
    read_wave_pm_config(repo, wave)?
        .linear_project
        .filter(|project| !project.trim().is_empty())
}

fn project_seed_pending(repo: &Path, wave: &str) -> bool {
    read_wave_pm_config(repo, wave).is_some_and(|pm| pm.linear_seed_pending)
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
    if project_seed_pending(repo, wave) {
        return Err(OpsError::Message(format!(
            "wave/{wave} has an incomplete Linear Project seed. Run `lf pm init --wave {wave}` to resume it."
        )));
    }
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
    let legacy_project = read_legacy_project(repo, &wave);
    let seed_pending = project_seed_pending(repo, &wave);
    if legacy_project.is_none() && !seed_pending {
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
    }

    let should_seed_projects = existing_initiative.is_none() || seed_pending;
    let summary = if existing_initiative.is_none() {
        wave_summary(repo, &wave)?
    } else {
        String::new()
    };
    let local_projects = if should_seed_projects {
        load_local_projects(repo, &wave)?
    } else {
        Vec::new()
    };
    let client = build_client(repo, provider).await?;
    let (initiative_id, created) = if let Some(existing) = existing_initiative {
        progress.status(&format!(
            "wave/{wave} already linked to {provider} Linear Initiative {existing}"
        ));
        (existing, false)
    } else {
        progress.status(&format!(
            "creating {provider} Linear Initiative for wave/{wave}"
        ));
        let initiative_id = client
            .create_wave(&title_case(&wave), &summary)
            .await
            .map_err(pm_to_ops)?;
        // Persist the remote handle before creating child projects. If a later
        // request fails, rerunning init resumes the seed instead of creating a
        // duplicate Initiative.
        write_initiative_to_goal(repo, &wave, provider, &initiative_id, false, true)?;
        (initiative_id, true)
    };

    let existing_projects = if created {
        Vec::new()
    } else {
        checked_projects(&client, &initiative_id, &wave).await?
    };
    let mut project_ids: BTreeMap<String, String> = existing_projects
        .into_iter()
        .map(|project| (project.slug, project.id))
        .collect();
    if should_seed_projects {
        for project in local_projects {
            if project_ids.contains_key(&project.slug) {
                continue;
            }
            progress.status(&format!(
                "creating {provider} Linear Project {} for wave/{wave}",
                project.name
            ));
            let project_id = client
                .create_project(&initiative_id, &project)
                .await
                .map_err(pm_to_ops)?;
            project_ids.insert(project.slug, project_id);
        }
        write_initiative_to_goal(repo, &wave, provider, &initiative_id, false, false)?;
    }

    let mut unmigrated = 0;
    if let Some(legacy_project_id) = legacy_project.as_deref() {
        progress.status(&format!(
            "migrating tasks from legacy Linear Project {legacy_project_id}"
        ));
        for legacy in client
            .list_legacy_items(legacy_project_id)
            .await
            .map_err(pm_to_ops)?
        {
            let Some((slug, project_id)) =
                unique_legacy_project_destination(&legacy.project_slugs, &project_ids)
            else {
                progress.status(&format!(
                    "leaving legacy task {} without exactly one recognized project label in {legacy_project_id}",
                    legacy.item.id,
                ));
                unmigrated += 1;
                continue;
            };
            progress.status(&format!(
                "moving legacy task {} to project:{slug}",
                legacy.item.id
            ));
            client
                .move_item_to_project(&legacy.item.id, &project_id)
                .await
                .map_err(pm_to_ops)?;
        }
    }
    if unmigrated > 0 {
        progress.status(&format!(
            "kept pm.linear_project because {unmigrated} legacy task(s) still need exactly one recognized project:<slug> label"
        ));
    }
    let config_changed = created || legacy_project.is_some() || seed_pending;
    if legacy_project.is_some() {
        write_initiative_to_goal(
            repo,
            &wave,
            provider,
            &initiative_id,
            unmigrated == 0,
            false,
        )?;
    }

    if config_changed {
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
    let ctx = resolve_context(repo, &wave).await?;
    fetch_items(&wave, options.project.as_deref(), &ctx, progress).await
}

async fn fetch_items(
    wave: &str,
    project_slug: Option<&str>,
    ctx: &PmContext,
    progress: &impl Progress,
) -> OpsResult<PmShowResult> {
    progress.status(&format!(
        "fetching {} Linear Initiative {} for wave/{wave}",
        ctx.provider, ctx.initiative
    ));
    let projects = checked_projects(&ctx.client, &ctx.initiative, wave).await?;
    let projects = match project_slug {
        Some(slug) => vec![find_project(&projects, wave, slug)?.clone()],
        None => projects,
    };
    let project_items = try_join_all(projects.into_iter().map(|project| async move {
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
    Ok(PmShowResult {
        wave: wave.to_string(),
        provider: ctx.provider,
        initiative: ctx.initiative.clone(),
        project: project_slug.map(str::to_string),
        items: project_items.into_iter().flatten().collect(),
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
    apply_update(&wave, options.project.as_deref(), &ctx, options, progress).await
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
                    "`lf pm update --title` is required when creating a task".to_string(),
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
    progress: &impl Progress,
) -> OpsResult<PmStatusResult> {
    let all_waves = list_local_waves(repo)?;
    let mut linked_initiative_ids = BTreeSet::new();
    for wave in &all_waves {
        let provider = resolve_provider(repo, wave)?;
        if let Some(initiative) = read_initiative(repo, wave, provider) {
            linked_initiative_ids.insert(initiative);
        }
    }

    let waves = if let Some(wave) = options.wave.as_deref() {
        vec![resolve_wave(repo, Some(wave))?]
    } else {
        list_pm_waves(repo)?
    };

    let mut results = Vec::new();
    let mut all_waves_by_provider: BTreeMap<String, Vec<PmWave>> = BTreeMap::new();
    for wave in waves {
        let provider = resolve_provider(repo, &wave)?;
        let Some(initiative) = read_initiative(repo, &wave, provider) else {
            continue;
        };
        let client = build_client(repo, provider).await?;
        progress.status(&format!("checking {provider} for wave/{wave}"));
        let provider_waves = client.list_waves().await.map_err(pm_to_ops)?;
        all_waves_by_provider
            .entry(provider.as_str().to_string())
            .or_insert_with(|| provider_waves.clone());
        let initiative_name = provider_waves
            .into_iter()
            .find(|candidate| candidate.id == initiative)
            .map(|candidate| candidate.name);
        let projects = checked_projects(&client, &initiative, &wave).await?;
        let mut total = 0;
        let mut open = 0;
        let mut open_by_project = BTreeMap::new();
        for project in projects {
            let items = client.list_items(&project.id).await.map_err(pm_to_ops)?;
            total += items.len();
            let project_open = items.iter().filter(|item| !item.completed).count();
            open += project_open;
            open_by_project.insert(project.slug, project_open);
        }
        results.push(PmWaveStatus {
            wave,
            provider,
            initiative,
            initiative_name,
            open,
            total,
            open_by_project,
        });
    }

    let stranded_waves = all_waves_by_provider
        .into_values()
        .flatten()
        .filter(|wave| !linked_initiative_ids.contains(&wave.id))
        .collect();

    Ok(PmStatusResult {
        waves: results,
        stranded_waves,
    })
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
    let waves = list_local_waves(repo)?;
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();

    let mut linked_initiative_ids = BTreeSet::new();
    let mut provider_by_kind = BTreeMap::new();
    for wave in &waves {
        let provider = resolve_provider(repo, wave)?;
        provider_by_kind.insert(provider.as_str().to_string(), provider);
        let has_legacy_project = read_legacy_project(repo, wave).is_some();
        if let Some(initiative) = read_initiative(repo, wave, provider) {
            linked_initiative_ids.insert(initiative);
            if project_seed_pending(repo, wave) {
                diagnostics.push(format!(
                    "wave/{wave} has an incomplete Linear Project seed; run `lf pm init --wave {wave}` to resume"
                ));
            } else if has_legacy_project {
                diagnostics.push(format!(
                    "wave/{wave} retains pm.linear_project because legacy task migration is incomplete; run `lf pm init --wave {wave}` after assigning project labels"
                ));
            }
        } else if has_legacy_project {
            diagnostics.push(format!(
                "wave/{wave} still uses pm.linear_project; run `lf pm init --wave {wave}` to migrate"
            ));
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
        if project_seed_pending(repo, wave) {
            continue;
        }
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

        let projects = checked_projects(&client, &initiative_id, wave).await?;
        let project_slugs: BTreeSet<_> = projects
            .iter()
            .map(|project| project.slug.clone())
            .collect();
        for project in projects {
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
            let cache_action = sync_project_cache(repo, wave, &project, options.plan)?;
            if let Some(action) = cache_action {
                actions.push(action);
            }
            let items = client.list_items(&project.id).await.map_err(pm_to_ops)?;
            if items.iter().all(|item| item.completed) {
                diagnostics.push(format!(
                    "Linear Project `{}` ({}) in wave/{wave} has no open tasks",
                    project.name, project.id
                ));
            }
        }
        actions.extend(remove_stale_project_caches(
            repo,
            wave,
            &project_slugs,
            options.plan,
        )?);
    }

    Ok(PmSyncResult {
        actions,
        diagnostics,
    })
}

// ── explicit mutations ─────────────────────────────────────────────

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

pub fn list_local_projects(repo: &Path, wave: &str) -> OpsResult<Vec<String>> {
    let projects_dir = repo.join("wave").join(wave).join("projects");
    if !projects_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();
    for entry in std::fs::read_dir(projects_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            projects.push(stem.to_string());
        }
    }
    projects.sort();
    Ok(projects)
}

// ── helpers ─────────────────────────────────────────────────────────

fn write_initiative_to_goal(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
    initiative_id: &str,
    clear_legacy_project: bool,
    seed_pending: bool,
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
        if clear_legacy_project {
            pm_map.remove(serde_yaml_ng::Value::String("linear_project".to_string()));
        }
        let seed_key = serde_yaml_ng::Value::String("linear_seed_pending".to_string());
        if seed_pending {
            pm_map.insert(seed_key, serde_yaml_ng::Value::Bool(true));
        } else {
            pm_map.remove(&seed_key);
        }
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

fn load_local_projects(repo: &Path, wave: &str) -> OpsResult<Vec<LocalProject>> {
    let mut projects = Vec::new();
    for slug in list_local_projects(repo, wave)? {
        let path = repo
            .join("wave")
            .join(wave)
            .join("projects")
            .join(format!("{slug}.md"));
        let content = std::fs::read_to_string(&path)?;
        let name = content
            .lines()
            .find_map(|line| line.trim().strip_prefix("# "))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| OpsError::Message(format!("{} has no project title", path.display())))?
            .to_string();
        let derived_slug = project_slug(&name);
        if derived_slug != slug {
            return Err(OpsError::Message(format!(
                "{} is named `{name}`, which derives slug `{derived_slug}` instead of `{slug}`",
                path.display()
            )));
        }
        let (definition, krs) = parse_project_content(&content);
        projects.push(LocalProject {
            slug,
            name,
            summary: first_paragraph(&definition),
            definition,
            krs,
        });
    }
    Ok(projects)
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

fn unique_legacy_project_destination(
    project_slugs: &[String],
    project_ids: &BTreeMap<String, String>,
) -> Option<(String, String)> {
    let mut destinations = project_slugs.iter().filter_map(|slug| {
        project_ids
            .get(slug)
            .map(|project_id| (slug.clone(), project_id.clone()))
    });
    let destination = destinations.next()?;
    destinations.next().is_none().then_some(destination)
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

fn sync_project_cache(
    repo: &Path,
    wave: &str,
    project: &PmProject,
    plan: bool,
) -> OpsResult<Option<String>> {
    let path = repo
        .join("wave")
        .join(wave)
        .join("projects")
        .join(format!("{}.md", project.slug));
    let expected = render_project_cache(project);
    if std::fs::read_to_string(&path).ok().as_deref() == Some(expected.as_str()) {
        return Ok(None);
    }
    let action = format!(
        "refresh wave/{wave}/projects/{}.md from Linear Project `{}`",
        project.slug, project.name
    );
    if !plan {
        let parent = path
            .parent()
            .ok_or_else(|| OpsError::Message(format!("invalid cache path: {}", path.display())))?;
        std::fs::create_dir_all(parent)?;
        std::fs::write(path, expected)?;
    }
    Ok(Some(action))
}

fn render_project_cache(project: &PmProject) -> String {
    let mut content = format!(
        "# {}\n\n{}\n\n## KRs",
        project.name,
        project.definition.trim()
    );
    for kr in &project.krs {
        let marker = if kr.holds { "x" } else { " " };
        content.push_str(&format!("\n\n- [{marker}] {}", kr.text.trim()));
    }
    content.push('\n');
    content
}

fn remove_stale_project_caches(
    repo: &Path,
    wave: &str,
    project_slugs: &BTreeSet<String>,
    plan: bool,
) -> OpsResult<Vec<String>> {
    let mut actions = Vec::new();
    for local_slug in list_local_projects(repo, wave)? {
        if project_slugs.contains(&local_slug) {
            continue;
        }
        let path = repo
            .join("wave")
            .join(wave)
            .join("projects")
            .join(format!("{local_slug}.md"));
        actions.push(format!(
            "remove stale wave/{wave}/projects/{local_slug}.md cache; no Linear Project has that slug"
        ));
        if !plan {
            std::fs::remove_file(path)?;
        }
    }
    Ok(actions)
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
            "pm:\n  provider: linear\n  linear_project: \"lin-1\"\n",
        );
        assert_eq!(
            resolve_provider(repo.path(), "scan").unwrap(),
            PmProviderKind::Linear
        );
    }

    #[test]
    fn resolve_provider_infers_linear_from_project_key() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(repo.path(), "scan", "pm:\n  linear_project: \"lin-1\"\n");
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
    fn local_project_seed_parses_definition_and_krs() {
        let repo = tempfile::tempdir().expect("temp dir");
        let dir = repo.path().join("wave/product/projects");
        std::fs::create_dir_all(&dir).expect("create projects");
        std::fs::write(
            dir.join("wave-chat.md"),
            "# Wave Chat\n\nConversation stays in flow.\n\n## KRs\n\n- [x] Replies stream\n- Threads survive\n",
        )
        .expect("write project");

        let projects = load_local_projects(repo.path(), "product").expect("load projects");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].slug, "wave-chat");
        assert_eq!(projects[0].summary, "Conversation stays in flow.");
        assert!(projects[0].krs[0].holds);
        assert!(!projects[0].krs[1].holds);
    }

    #[test]
    fn local_project_seed_rejects_name_slug_drift() {
        let repo = tempfile::tempdir().expect("temp dir");
        let dir = repo.path().join("wave/product/projects");
        std::fs::create_dir_all(&dir).expect("create projects");
        std::fs::write(dir.join("chat.md"), "# Wave Chat\n\nDefinition\n").expect("write project");

        let error = load_local_projects(repo.path(), "product").expect_err("slug must match");
        assert!(error.to_string().contains("derives slug `wave-chat`"));
    }

    #[test]
    fn initiative_write_replaces_legacy_project_handle() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_project: legacy-1\n",
        );

        write_initiative_to_goal(
            repo.path(),
            "product",
            PmProviderKind::Linear,
            "initiative-1",
            true,
            false,
        )
        .expect("write initiative");

        let pm = read_wave_pm_config(repo.path(), "product").expect("pm config");
        assert_eq!(pm.provider.as_deref(), Some("linear"));
        assert_eq!(pm.linear_initiative.as_deref(), Some("initiative-1"));
        assert_eq!(pm.linear_project, None);
    }

    #[test]
    fn initiative_write_retains_legacy_handle_during_partial_migration() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_project: legacy-1\n",
        );

        write_initiative_to_goal(
            repo.path(),
            "product",
            PmProviderKind::Linear,
            "initiative-1",
            false,
            true,
        )
        .expect("write initiative");

        let pm = read_wave_pm_config(repo.path(), "product").expect("pm config");
        assert_eq!(pm.linear_initiative.as_deref(), Some("initiative-1"));
        assert_eq!(pm.linear_project.as_deref(), Some("legacy-1"));
        assert!(pm.linear_seed_pending);

        write_initiative_to_goal(
            repo.path(),
            "product",
            PmProviderKind::Linear,
            "initiative-1",
            false,
            false,
        )
        .expect("finish project seed");
        assert!(
            !read_wave_pm_config(repo.path(), "product")
                .expect("pm config")
                .linear_seed_pending
        );
    }

    #[tokio::test]
    async fn incomplete_project_seed_blocks_task_operations() {
        let repo = tempfile::tempdir().expect("temp dir");
        write_goal(
            repo.path(),
            "product",
            "pm:\n  provider: linear\n  linear_initiative: initiative-1\n  linear_seed_pending: true\n",
        );

        let error = match resolve_context(repo.path(), "product").await {
            Err(error) => error,
            Ok(_) => panic!("incomplete seed must block task operations"),
        };
        assert!(error.to_string().contains("lf pm init --wave product"));
    }

    #[test]
    fn sync_project_cache_plans_then_writes_linear_state() {
        let repo = tempfile::tempdir().expect("temp dir");
        let project = PmProject {
            id: "project-1".to_string(),
            slug: "wave-chat".to_string(),
            name: "Wave Chat".to_string(),
            summary: "Conversation stays in flow.".to_string(),
            definition: "Conversation stays in flow.".to_string(),
            krs: vec![PmKr {
                text: "Replies stream".to_string(),
                holds: true,
            }],
            initiative_ids: vec!["initiative-1".to_string()],
        };
        let path = repo.path().join("wave/product/projects/wave-chat.md");

        assert!(sync_project_cache(repo.path(), "product", &project, true)
            .expect("plan cache")
            .is_some());
        assert!(!path.exists());
        assert!(sync_project_cache(repo.path(), "product", &project, false)
            .expect("write cache")
            .is_some());
        assert_eq!(
            std::fs::read_to_string(path).expect("read cache"),
            "# Wave Chat\n\nConversation stays in flow.\n\n## KRs\n\n- [x] Replies stream\n"
        );
        assert_eq!(
            sync_project_cache(repo.path(), "product", &project, false).expect("cache unchanged"),
            None
        );
    }

    #[test]
    fn stale_project_cache_removal_is_plannable_and_scoped() {
        let repo = tempfile::tempdir().expect("temp dir");
        let dir = repo.path().join("wave/product/projects");
        std::fs::create_dir_all(&dir).expect("create projects");
        std::fs::write(dir.join("current.md"), "current").expect("write current");
        std::fs::write(dir.join("stale.md"), "stale").expect("write stale");
        let current = BTreeSet::from(["current".to_string()]);

        let actions = remove_stale_project_caches(repo.path(), "product", &current, true)
            .expect("plan stale cache removal");
        assert_eq!(actions.len(), 1);
        assert!(actions[0].contains("stale.md"));
        assert!(dir.join("stale.md").exists());

        remove_stale_project_caches(repo.path(), "product", &current, false)
            .expect("remove stale cache");
        assert!(dir.join("current.md").exists());
        assert!(!dir.join("stale.md").exists());
    }

    #[test]
    fn duplicate_linear_project_slugs_are_drift() {
        let project = |id: &str, name: &str| PmProject {
            id: id.to_string(),
            slug: project_slug(name),
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
    fn legacy_task_migration_requires_one_recognized_project() {
        let projects = BTreeMap::from([
            ("context".to_string(), "project-1".to_string()),
            ("evals".to_string(), "project-2".to_string()),
        ]);

        assert_eq!(
            unique_legacy_project_destination(&["context".to_string()], &projects),
            Some(("context".to_string(), "project-1".to_string()))
        );
        assert_eq!(
            unique_legacy_project_destination(&["unknown".to_string()], &projects),
            None
        );
        assert_eq!(
            unique_legacy_project_destination(
                &["context".to_string(), "evals".to_string()],
                &projects,
            ),
            None
        );
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
    async fn fetch_items_dispatches_to_linear_provider() {
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

        let result = fetch_items("scan", None, &ctx, &NullProgress)
            .await
            .expect("fetch succeeds");
        assert_eq!(result.provider, PmProviderKind::Linear);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "First");
        assert_eq!(
            requests.lock().await[1].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
    }

    #[tokio::test]
    async fn fetch_items_filters_by_native_project() {
        let (base_url, requests) = test_server::spawn(vec![
            projects_response(json!([
                project_node("project-chat", "Wave Chat"),
                project_node("project-api", "Loopflow API")
            ])),
            issues_response(json!([
                { "id": "issue-1", "title": "Wave chat", "description": "",
                  "prioritySortOrder": 0.0, "sortOrder": 0.0,
                  "state": { "type": "unstarted" } }
            ])),
        ])
        .await;
        let ctx = linear_test_ctx(base_url, "initiative-123");

        let result = fetch_items("product", Some("wave-chat"), &ctx, &NullProgress)
            .await
            .expect("fetch succeeds");

        assert_eq!(result.project.as_deref(), Some("wave-chat"));
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "Wave chat");
        assert_eq!(result.items[0].project.as_deref(), Some("wave-chat"));
        let requests = requests.lock().await;
        let issue_request: serde_json::Value =
            serde_json::from_str(&requests[1].body).expect("issue request is json");
        assert_eq!(issue_request["variables"]["projectId"], "project-chat");
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
}
