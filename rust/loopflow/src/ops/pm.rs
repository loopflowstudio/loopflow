//! `lf op pm` — read and write a wave's roadmap directly in a PM provider.
//!
//! The PM provider is the single source of truth for a wave's roadmap. There is
//! no local mirror: every command here talks to the project pinned by the wave's
//! `pm.*_project` frontmatter.

use std::future::Future;
use std::path::Path;

use crate::engine::config::load_config_or_default;
use crate::engine::wave_config::{read_wave_config, update_wave_goal_config, WavePmConfig};
use crate::lfd::pm::linear::LinearClient;
use crate::lfd::pm::{PmError, PmItem, PmItemCreate, PmItemUpdate, PmProviderKind, PmResult};
use crate::lfdb::open_store;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;
use crate::provider_auth::{refresh_pm_oauth_token, Provider};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmShowResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub project: String,
    pub items: Vec<PmItem>,
}

#[derive(Debug, Clone)]
pub struct PmUpdateOptions {
    pub wave: Option<String>,
    pub id: Option<String>,
    pub title: String,
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

#[derive(Debug, Clone)]
pub(crate) struct PmCompleteOptions {
    pub wave: Option<String>,
    pub id: String,
    /// PR URL to attach as a comment on the task before closing it.
    pub pr: Option<String>,
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
    pub open: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmStatusResult {
    pub waves: Vec<PmWaveStatus>,
}

// ── Client + project resolution ─────────────────────────────────────

/// A wave's PM client bound to its roadmap project.
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

/// Whether a wave has a roadmap project pinned for its resolved provider.
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
             Run `lf op pm init --wave {wave}` to connect its roadmap.",
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

/// Linear authenticates via OAuth: the access token lives in the lfdb credential
/// store (keyed by provider), and an expired token is refreshed in place when it
/// carries a refresh token and the OAuth client creds resolve.
async fn resolve_pm_token(provider: PmProviderKind) -> OpsResult<String> {
    // A forwarded token wins over the local store: `lf ssh` resolves the PM
    // credential on the caller's machine (where lfdb lives) and hands it to the
    // remote through the environment. The remote lfdb holds no PM credential, so
    // without this hook remote `lf op pm` could never authenticate.
    if let Some(token) = forwarded_pm_token(provider) {
        return Ok(token);
    }

    let auth_provider = match provider {
        PmProviderKind::Linear => Provider::Linear,
    };

    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open lfd credential store: {err}")))?;
    let token = store
        .get_provider_token(provider.as_str())
        .await
        .map_err(|err| OpsError::Message(format!("failed to load {provider} token: {err}")))?
        .ok_or_else(|| {
            OpsError::Message(format!(
                "No {provider} credential found. Run `lf op auth {provider}`."
            ))
        })?;

    let expired = token
        .expires_at
        .is_some_and(|expires_at| expires_at <= time::OffsetDateTime::now_utc().unix_timestamp());
    if !expired {
        return Ok(token.access_token);
    }

    // The stored token is expired but may be refreshable: if it carries a refresh
    // token and the OAuth client creds resolve, refresh in place rather than
    // forcing the user to re-authenticate.
    let refresh_token = token
        .refresh_token
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    if let Some(refresh_token) = refresh_token {
        if let Ok(mut refreshed) = refresh_pm_oauth_token(auth_provider, refresh_token).await {
            if refreshed.refresh_token.is_none() {
                refreshed.refresh_token = token.refresh_token.clone();
            }
            if refreshed.login.is_none() {
                refreshed.login = token.login.clone();
            }
            let access_token = refreshed.access_token.clone();
            store
                .upsert_provider_token(&refreshed)
                .await
                .map_err(|err| {
                    OpsError::Message(format!(
                        "failed to persist refreshed {provider} token: {err}"
                    ))
                })?;
            return Ok(access_token);
        }
    }

    Err(OpsError::Message(format!(
        "Stored {provider} token has expired. Run `lf op auth {provider}` again."
    )))
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
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let wave_dir = repo.join("wave").join(&wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{wave}/"
        )));
    }

    let provider = resolve_provider(repo, &wave)?;
    if let Some(existing) = read_project(repo, &wave, provider) {
        progress.status(&format!(
            "wave/{wave} already linked to {provider} project {existing}"
        ));
        return Ok(PmInitResult {
            wave,
            project_id: existing,
            created: false,
        });
    }

    let client = build_client(repo, provider).await?;
    progress.status(&format!("creating {provider} project for wave/{wave}"));
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
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let ctx = resolve_context(repo, &wave).await?;
    fetch_items(&wave, &ctx, progress).await
}

async fn fetch_items(
    wave: &str,
    ctx: &PmContext,
    progress: &impl Progress,
) -> OpsResult<PmShowResult> {
    progress.status(&format!(
        "fetching {} project {} for wave/{wave}",
        ctx.provider, ctx.project
    ));
    let items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;
    Ok(PmShowResult {
        wave: wave.to_string(),
        provider: ctx.provider,
        project: ctx.project.clone(),
        items,
    })
}

/// One scannable line per task: status, name, assignee, id.
pub fn format_roadmap_item(item: &PmItem) -> String {
    let status = if item.completed { "done" } else { "open" };
    let assignee = item.assignee.as_deref().unwrap_or("-");
    format!(
        "{status:<8} {:<40} assignee:{assignee:<20} id:{}",
        item.name, item.id
    )
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
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let ctx = resolve_context(repo, &wave).await?;
    apply_update(&wave, &ctx, options, progress).await
}

pub(crate) fn pm_complete(
    repo: &Path,
    options: &PmCompleteOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    block_on_pm(pm_complete_async(repo, options, progress))
}

async fn pm_complete_async(
    repo: &Path,
    options: &PmCompleteOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let ctx = resolve_context(repo, &wave).await?;
    complete_existing_item(&wave, &ctx, &options.id, options.pr.as_deref(), progress).await
}

async fn apply_update(
    wave: &str,
    ctx: &PmContext,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let mark_done = parse_done_status(options.status.as_deref())?;

    let (id, created) = match options.id.as_ref() {
        Some(id) => {
            progress.status(&format!("updating {} task {id}", ctx.provider));
            ctx.client
                .update_item(
                    id,
                    &PmItemUpdate {
                        name: Some(options.title.clone()),
                        description: options.notes.clone(),
                        rank: None,
                    },
                )
                .await
                .map_err(pm_to_ops)?;
            (id.clone(), false)
        }
        None => {
            progress.status(&format!(
                "creating {} task on project {} for wave/{wave}",
                ctx.provider, ctx.project
            ));
            let id = ctx
                .client
                .create_item(
                    &ctx.project,
                    &PmItemCreate {
                        name: options.title.clone(),
                        description: options.notes.clone().unwrap_or_default(),
                        rank: 0,
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

async fn complete_existing_item(
    wave: &str,
    ctx: &PmContext,
    id: &str,
    pr: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let linked_pr = match pr.map(str::trim).filter(|pr| !pr.is_empty()) {
        Some(pr) => {
            progress.status(&format!("commenting PR link on {} task {id}", ctx.provider));
            ctx.client
                .comment(id, &format!("Shipped: {pr}"))
                .await
                .map_err(pm_to_ops)?;
            Some(pr.to_string())
        }
        None => None,
    };

    progress.status(&format!("closing {} task {id}", ctx.provider));
    ctx.client.complete_item(id).await.map_err(pm_to_ops)?;

    Ok(PmUpdateResult {
        wave: wave.to_string(),
        id: id.to_string(),
        created: false,
        completed: true,
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
            "unsupported roadmap status {other:?}; only \"done\" is supported"
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
    let waves = if let Some(wave) = options.wave.as_deref() {
        vec![resolve_wave_name(repo, Some(wave))
            .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?]
    } else {
        list_pm_waves(repo)?
    };

    let mut results = Vec::new();
    for wave in waves {
        let provider = resolve_provider(repo, &wave)?;
        let Some(project) = read_project(repo, &wave, provider) else {
            continue;
        };
        let client = build_client(repo, provider).await?;
        progress.status(&format!("checking {provider} for wave/{wave}"));
        let items = client.list_items(&project).await.map_err(pm_to_ops)?;
        let total = items.len();
        let open = items.iter().filter(|item| !item.completed).count();
        results.push(PmWaveStatus {
            wave,
            provider,
            project,
            open,
            total,
        });
    }

    Ok(PmStatusResult { waves: results })
}

pub fn list_pm_waves(repo: &Path) -> OpsResult<Vec<String>> {
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
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if wave_has_pm_project(repo, &name) {
            waves.push(name);
        }
    }
    waves.sort();
    Ok(waves)
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

    #[test]
    fn format_roadmap_item_is_one_scannable_line() {
        let line = format_roadmap_item(&PmItem {
            id: "123".to_string(),
            name: "Ship it".to_string(),
            description: String::new(),
            rank: 0,
            completed: false,
            labels: Vec::new(),
            assignee: Some("me".to_string()),
        });
        assert!(line.starts_with("open"));
        assert!(line.contains("Ship it"));
        assert!(line.contains("assignee:me"));
        assert!(line.contains("id:123"));
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

        let result = fetch_items("scan", &ctx, &NullProgress)
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
            id: None,
            title: "New task".to_string(),
            notes: Some("details".to_string()),
            status: None,
            pr: None,
        };

        let result = apply_update("goals", &ctx, &options, &NullProgress)
            .await
            .expect("update succeeds");
        assert!(result.created);
        assert_eq!(result.id, "new-task");
        assert!(!result.completed);
        assert!(result.linked_pr.is_none());
        assert_eq!(requests.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn apply_update_completes_when_status_done() {
        let (base_url, _requests) = test_server::spawn(vec![
            // update_item (issueUpdate)
            json_response(
                StatusCode::OK,
                json!({ "data": { "issueUpdate": { "issue": { "id": "task-9" } } } }),
            ),
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
            id: Some("task-9".to_string()),
            title: "Existing".to_string(),
            notes: None,
            status: Some("done".to_string()),
            pr: None,
        };

        let result = apply_update("goals", &ctx, &options, &NullProgress)
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
            id: Some("task-9".to_string()),
            title: "Existing".to_string(),
            notes: None,
            status: Some("done".to_string()),
            pr: Some("https://github.com/acme/repo/pull/42".to_string()),
        };

        let result = apply_update("goals", &ctx, &options, &NullProgress)
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
