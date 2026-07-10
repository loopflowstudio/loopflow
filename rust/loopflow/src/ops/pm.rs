//! `lf pm` — read and write a wave's PM tasks directly in a provider.
//!
//! The PM provider is the single source of truth for a wave's tasks. There is
//! no local mirror: every command here talks to the Linear project pinned by the
//! wave's `pm.*_project` frontmatter. Local measured bets live in
//! `wave/<wave>/projects/` and map to provider labels named `project:<slug>`.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::Path;

use crate::engine::config::load_config_or_default;
use crate::engine::wave_config::{read_wave_config, update_wave_goal_config, WavePmConfig};
use crate::lfd::pm::linear::LinearClient;
use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmLabel, PmProject, PmProviderKind, PmResult,
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
    pub project_id: String,
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
    pub project: String,
    pub local_project: Option<String>,
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
    pub project: String,
    pub project_name: Option<String>,
    pub open: usize,
    pub total: usize,
    pub unassigned: usize,
    pub open_by_project: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmStatusResult {
    pub waves: Vec<PmWaveStatus>,
    pub stranded_projects: Vec<PmProject>,
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
    pub project: String,
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

// ── Client + Linear project resolution ──────────────────────────────

/// A wave's PM client bound to its provider Linear project.
pub(crate) struct PmContext {
    pub client: PmClient,
    pub provider: PmProviderKind,
    pub project: String,
}

pub(crate) enum PmClient {
    Linear(LinearClient),
}

impl PmClient {
    async fn create_project(&self, name: &str, description: &str) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.create_project(name, description).await,
        }
    }

    async fn rename_project(&self, project_id: &str, name: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.rename_project(project_id, name).await,
        }
    }

    async fn list_projects(&self) -> PmResult<Vec<PmProject>> {
        match self {
            Self::Linear(client) => client.list_projects().await,
        }
    }

    async fn list_items(&self, project_id: &str) -> PmResult<Vec<PmItem>> {
        match self {
            Self::Linear(client) => client.list_items(project_id).await,
        }
    }

    async fn ensure_label(&self, name: &str) -> PmResult<String> {
        match self {
            Self::Linear(client) => client.ensure_label(name).await,
        }
    }

    async fn list_labels(&self) -> PmResult<Vec<PmLabel>> {
        match self {
            Self::Linear(client) => client.list_labels().await,
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

    async fn add_label_to_item(&self, item_id: &str, label_id: &str) -> PmResult<()> {
        match self {
            Self::Linear(client) => client.add_label_to_item(item_id, label_id).await,
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

fn read_project(repo: &Path, wave: &str, provider: PmProviderKind) -> Option<String> {
    let pm = read_wave_pm_config(repo, wave)?;
    let project = match provider {
        PmProviderKind::Linear => pm.linear_project,
    }?;
    Some(project).filter(|project| !project.trim().is_empty())
}

/// Whether a wave has a Linear project pinned for its resolved provider.
fn wave_has_pm_project(repo: &Path, wave: &str) -> bool {
    resolve_provider(repo, wave)
        .ok()
        .is_some_and(|provider| read_project(repo, wave, provider).is_some())
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
    let project = read_project(repo, wave, provider).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.{}`. \
             Run `lf pm init --wave {wave}` to connect its Linear project.",
            provider.project_key()
        ))
    })?;
    let client = build_client(repo, provider).await?;
    Ok(PmContext {
        client,
        provider,
        project,
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
    if let Some(existing) = read_project(repo, &wave, provider) {
        progress.status(&format!(
            "wave/{wave} already linked to {provider} Linear project {existing}"
        ));
        return Ok(PmInitResult {
            wave,
            project_id: existing,
            created: false,
        });
    }

    let client = build_client(repo, provider).await?;
    progress.status(&format!(
        "creating {provider} Linear project for wave/{wave}"
    ));
    let project_id = client
        .create_project(&title_case(&wave), "")
        .await
        .map_err(pm_to_ops)?;
    write_project_to_goal(repo, &wave, provider, &project_id)?;

    let _ = crate::ops::commit_workflow(
        repo,
        &crate::ops::CommitOptions {
            add: true,
            message: Some(format!("lf pm: connect {wave} to {provider}")),
            ..crate::ops::CommitOptions::for_task("pm")
        },
        progress,
    )?;

    Ok(PmInitResult {
        wave,
        project_id,
        created: true,
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
    let local_project = resolve_local_project(repo, &wave, options.project.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    fetch_items(&wave, local_project.as_deref(), &ctx, progress).await
}

async fn fetch_items(
    wave: &str,
    local_project: Option<&str>,
    ctx: &PmContext,
    progress: &impl Progress,
) -> OpsResult<PmShowResult> {
    progress.status(&format!(
        "fetching {} Linear project {} for wave/{wave}",
        ctx.provider, ctx.project
    ));
    let mut items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;
    if let Some(project) = local_project {
        items.retain(|item| item_matches_project(item, project));
    }
    Ok(PmShowResult {
        wave: wave.to_string(),
        provider: ctx.provider,
        project: ctx.project.clone(),
        local_project: local_project.map(str::to_string),
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
    let local_project = resolve_local_project(repo, &wave, options.project.as_deref())?;
    let ctx = resolve_context(repo, &wave).await?;
    apply_update(&wave, local_project.as_deref(), &ctx, options, progress).await
}

async fn apply_update(
    wave: &str,
    local_project: Option<&str>,
    ctx: &PmContext,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let mark_done = parse_done_status(options.status.as_deref())?;
    let label_id = match local_project {
        Some(project) => {
            let label = project_label(project);
            progress.status(&format!("ensuring {} label {label}", ctx.provider));
            Some(ctx.client.ensure_label(&label).await.map_err(pm_to_ops)?)
        }
        None => None,
    };

    let (id, created) = match options.id.as_ref() {
        Some(id) => {
            progress.status(&format!("updating {} task {id}", ctx.provider));
            ctx.client
                .update_item(
                    id,
                    &PmItemUpdate {
                        name: options.title.clone(),
                        description: options.notes.clone(),
                        rank: None,
                    },
                )
                .await
                .map_err(pm_to_ops)?;
            if let Some(label_id) = label_id.as_deref() {
                ctx.client
                    .add_label_to_item(id, label_id)
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
            progress.status(&format!(
                "creating {} task in Linear project {} for wave/{wave}",
                ctx.provider, ctx.project
            ));
            let id = ctx
                .client
                .create_item(
                    &ctx.project,
                    &PmItemCreate {
                        name: title.clone(),
                        description: options.notes.clone().unwrap_or_default(),
                        rank: 0,
                        label_ids: label_id.into_iter().collect(),
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
    let mut linked_project_ids = BTreeSet::new();
    for wave in &all_waves {
        let provider = resolve_provider(repo, wave)?;
        if let Some(project) = read_project(repo, wave, provider) {
            linked_project_ids.insert(project);
        }
    }

    let waves = if let Some(wave) = options.wave.as_deref() {
        vec![resolve_wave(repo, Some(wave))?]
    } else {
        list_pm_waves(repo)?
    };

    let mut results = Vec::new();
    let mut all_projects_by_provider: BTreeMap<String, Vec<PmProject>> = BTreeMap::new();
    for wave in waves {
        let provider = resolve_provider(repo, &wave)?;
        let Some(project) = read_project(repo, &wave, provider) else {
            continue;
        };
        let client = build_client(repo, provider).await?;
        progress.status(&format!("checking {provider} for wave/{wave}"));
        let projects = client.list_projects().await.map_err(pm_to_ops)?;
        all_projects_by_provider
            .entry(provider.as_str().to_string())
            .or_insert_with(|| projects.clone());
        let project_name = projects
            .into_iter()
            .find(|candidate| candidate.id == project)
            .map(|candidate| candidate.name);
        let items = client.list_items(&project).await.map_err(pm_to_ops)?;
        let total = items.len();
        let open = items.iter().filter(|item| !item.completed).count();
        let local_projects = list_local_projects(repo, &wave)?;
        let mut open_by_project = BTreeMap::new();
        for project in local_projects {
            open_by_project.insert(project, 0);
        }
        let mut unassigned = 0;
        for item in &items {
            if item.completed {
                continue;
            }
            let labels = item_project_labels(item);
            if labels.is_empty() {
                unassigned += 1;
            }
            for project in labels {
                *open_by_project.entry(project).or_default() += 1;
            }
        }
        results.push(PmWaveStatus {
            wave,
            provider,
            project,
            project_name,
            open,
            total,
            unassigned,
            open_by_project,
        });
    }

    let stranded_projects = all_projects_by_provider
        .into_values()
        .flatten()
        .filter(|project| !linked_project_ids.contains(&project.id))
        .collect();

    Ok(PmStatusResult {
        waves: results,
        stranded_projects,
    })
}

pub fn list_pm_waves(repo: &Path) -> OpsResult<Vec<String>> {
    Ok(list_local_waves(repo)?
        .into_iter()
        .filter(|wave| wave_has_pm_project(repo, wave))
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

    let mut linked_project_ids = BTreeSet::new();
    let mut provider_by_kind = BTreeMap::new();
    for wave in &waves {
        let provider = resolve_provider(repo, wave)?;
        provider_by_kind.insert(provider.as_str().to_string(), provider);
        if let Some(project) = read_project(repo, wave, provider) {
            linked_project_ids.insert(project);
        } else {
            diagnostics.push(format!("wave/{wave} has no Linear project"));
        }
    }

    let provider = provider_by_kind
        .values()
        .next()
        .copied()
        .unwrap_or(PmProviderKind::Linear);
    let client = build_client(repo, provider).await?;
    progress.status(&format!("checking {provider} Linear projects and labels"));
    let linear_projects = client.list_projects().await.map_err(pm_to_ops)?;
    let labels = client.list_labels().await.map_err(pm_to_ops)?;
    let label_names: BTreeSet<String> = labels.into_iter().map(|label| label.name).collect();

    let linear_projects_by_id: BTreeMap<String, String> = linear_projects
        .iter()
        .map(|project| (project.id.clone(), project.name.clone()))
        .collect();
    for project in &linear_projects {
        if !linked_project_ids.contains(&project.id) {
            diagnostics.push(format!(
                "Linear project `{}` ({}) is not linked by any local wave",
                project.name, project.id
            ));
        }
    }

    for wave in &waves {
        let provider = resolve_provider(repo, wave)?;
        let Some(linear_project_id) = read_project(repo, wave, provider) else {
            continue;
        };
        let expected_linear_project_name = title_case(wave);
        match linear_projects_by_id.get(&linear_project_id) {
            Some(actual) if actual != &expected_linear_project_name => {
                let message = format!(
                    "rename Linear project `{actual}` ({linear_project_id}) to `{expected_linear_project_name}` for wave/{wave}"
                );
                if !options.plan {
                    client
                        .rename_project(&linear_project_id, &expected_linear_project_name)
                        .await
                        .map_err(pm_to_ops)?;
                }
                actions.push(message);
            }
            None => diagnostics.push(format!(
                "wave/{wave} points at missing Linear project {linear_project_id}"
            )),
            _ => {}
        }

        let local_projects = list_local_projects(repo, wave)?;
        for project in &local_projects {
            let label = project_label(project);
            if !label_names.contains(&label) {
                if !options.plan {
                    client.ensure_label(&label).await.map_err(pm_to_ops)?;
                }
                actions.push(format!("create label `{label}` for wave/{wave}"));
            }
        }

        let local_project_set: BTreeSet<String> = local_projects.into_iter().collect();
        let items = client
            .list_items(&linear_project_id)
            .await
            .map_err(pm_to_ops)?;
        let mut open_by_project: BTreeMap<String, usize> = BTreeMap::new();
        for item in &items {
            let project_labels = item_project_labels(item);
            if !item.completed {
                for project in &project_labels {
                    *open_by_project.entry(project.clone()).or_default() += 1;
                }
            }
            if project_labels.is_empty() {
                diagnostics.push(format!(
                    "task `{}` ({}) in wave/{wave} has no project:<slug> label",
                    item.name, item.id
                ));
            }
            for project in project_labels {
                if !local_project_set.contains(&project) {
                    diagnostics.push(format!(
                        "task `{}` ({}) in wave/{wave} uses missing local project `{project}`",
                        item.name, item.id
                    ));
                }
            }
        }
        for project in &local_project_set {
            if open_by_project.get(project).copied().unwrap_or(0) == 0 {
                diagnostics.push(format!(
                    "wave/{wave}/projects/{project}.md has no open tasks"
                ));
            }
        }
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
        "renaming {} Linear project {} to {}",
        ctx.provider, ctx.project, options.title
    ));
    ctx.client
        .rename_project(&ctx.project, &options.title)
        .await
        .map_err(pm_to_ops)?;
    Ok(PmRenameResult {
        wave,
        project: ctx.project,
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
    resolve_local_project(repo, &wave, Some(&options.project))?;
    let ctx = resolve_context(repo, &wave).await?;
    let label = project_label(&options.project);
    let label_id = ctx.client.ensure_label(&label).await.map_err(pm_to_ops)?;
    progress.status(&format!(
        "moving {} task {} to wave/{wave} Linear project {}",
        ctx.provider, options.id, ctx.project
    ));
    ctx.client
        .move_item_to_project(&options.id, &ctx.project)
        .await
        .map_err(pm_to_ops)?;
    ctx.client
        .add_label_to_item(&options.id, &label_id)
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

fn resolve_local_project(
    repo: &Path,
    wave: &str,
    project: Option<&str>,
) -> OpsResult<Option<String>> {
    let Some(project) = project.map(str::trim).filter(|project| !project.is_empty()) else {
        return Ok(None);
    };
    let projects = list_local_projects(repo, wave)?;
    if projects.iter().any(|candidate| candidate == project) {
        return Ok(Some(project.to_string()));
    }
    Err(OpsError::Message(format!(
        "wave/{wave}/projects/{project}.md does not exist"
    )))
}

fn project_label(project: &str) -> String {
    format!("project:{project}")
}

pub fn item_project_labels(item: &PmItem) -> Vec<String> {
    item.labels
        .iter()
        .filter_map(|label| label.strip_prefix("project:").map(str::to_string))
        .collect()
}

fn item_matches_project(item: &PmItem, project: &str) -> bool {
    let label = project_label(project);
    item.labels.iter().any(|item_label| item_label == &label)
}

// ── helpers ─────────────────────────────────────────────────────────

fn write_project_to_goal(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
    project_id: &str,
) -> OpsResult<()> {
    update_wave_goal_config(repo, wave, |map| {
        let pm_key = serde_yaml_ng::Value::String("pm".to_string());
        let mut pm_map = map
            .get(&pm_key)
            .and_then(serde_yaml_ng::Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        pm_map.insert(
            serde_yaml_ng::Value::String(provider.project_key().to_string()),
            serde_yaml_ng::Value::String(project_id.to_string()),
        );
        map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));
        Ok(())
    })
    .map_err(OpsError::Message)
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
    use crate::lfd::pm::test_server::{self, json_response};
    use crate::ops::NullProgress;
    use axum::http::StatusCode;
    use serde_json::json;

    fn linear_test_ctx(base_url: String, project: &str) -> PmContext {
        PmContext {
            client: PmClient::Linear(crate::lfd::pm::linear::LinearClient::with_base_url(
                "linear-secret".to_string(),
                Some("team-9".to_string()),
                base_url,
            )),
            provider: PmProviderKind::Linear,
            project: project.to_string(),
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
    fn parse_done_status_maps_synonyms_and_rejects_others() {
        assert!(!parse_done_status(None).unwrap());
        assert!(parse_done_status(Some("done")).unwrap());
        assert!(parse_done_status(Some("Completed")).unwrap());
        assert!(parse_done_status(Some("blocked")).is_err());
    }

    #[tokio::test]
    async fn fetch_items_dispatches_to_linear_provider() {
        let (base_url, requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "project": { "issues": {
                "nodes": [
                    { "id": "issue-1", "title": "First", "description": "one",
                      "prioritySortOrder": 0.0, "sortOrder": 0.0,
                      "state": { "type": "unstarted" } }
                ],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } } }),
        )])
        .await;
        let ctx = linear_test_ctx(base_url, "project-123");

        let result = fetch_items("scan", None, &ctx, &NullProgress)
            .await
            .expect("fetch succeeds");
        assert_eq!(result.provider, PmProviderKind::Linear);
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "First");
        assert_eq!(
            requests.lock().await[0].authorization.as_deref(),
            Some("Bearer linear-secret")
        );
    }

    #[tokio::test]
    async fn fetch_items_filters_by_local_project_label() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": { "project": { "issues": {
                "nodes": [
                    { "id": "issue-1", "title": "Wave chat", "description": "",
                      "prioritySortOrder": 0.0, "sortOrder": 0.0,
                      "labels": { "nodes": [{ "name": "project:wave-chat" }] },
                      "state": { "type": "unstarted" } },
                    { "id": "issue-2", "title": "API", "description": "",
                      "prioritySortOrder": 1.0, "sortOrder": 1.0,
                      "labels": { "nodes": [{ "name": "project:loopflow-api" }] },
                      "state": { "type": "unstarted" } }
                ],
                "pageInfo": { "hasNextPage": false, "endCursor": null }
            } } } }),
        )])
        .await;
        let ctx = linear_test_ctx(base_url, "project-123");

        let result = fetch_items("product", Some("wave-chat"), &ctx, &NullProgress)
            .await
            .expect("fetch succeeds");

        assert_eq!(result.local_project.as_deref(), Some("wave-chat"));
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "Wave chat");
    }

    #[tokio::test]
    async fn apply_update_creates_when_no_id_is_given() {
        // With a team already resolved, create_item resolves the active (unstarted)
        // workflow state, then sends the issueCreate mutation.
        let (base_url, requests) = test_server::spawn(vec![
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
        let ctx = linear_test_ctx(base_url, "project-123");
        let options = PmUpdateOptions {
            wave: None,
            project: None,
            id: None,
            title: Some("New task".to_string()),
            notes: Some("details".to_string()),
            status: None,
            pr: None,
        };

        let result = apply_update("goals", None, &ctx, &options, &NullProgress)
            .await
            .expect("update succeeds");
        assert!(result.created);
        assert_eq!(result.id, "new-task");
        assert!(!result.completed);
        assert!(result.linked_pr.is_none());
        assert_eq!(requests.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn apply_update_creates_labeled_project_task() {
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueLabels": { "nodes": [] } } }),
            ),
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueLabelCreate": { "issueLabel": { "id": "label-1", "name": "project:wave-chat" } } } }),
            ),
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
        let ctx = linear_test_ctx(base_url, "project-123");
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
            serde_json::from_str(&requests[3].body).expect("create body is json");
        assert_eq!(create_body["variables"]["labelIds"], json!(["label-1"]));
    }

    #[tokio::test]
    async fn apply_update_completes_when_status_done() {
        let (base_url, _requests) = test_server::spawn(vec![
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
        let ctx = linear_test_ctx(base_url, "project-123");
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
        let ctx = linear_test_ctx(base_url, "project-123");
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
