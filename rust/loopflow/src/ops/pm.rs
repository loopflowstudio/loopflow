use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;

use crate::engine::config::{load_config_or_default, Config};
use crate::engine::git::{get_default_branch, list_tree, show_file};
use crate::lfd::http::routes::wave_config::{read_wave_config, WavePmConfig};
use crate::lfd::pm::asana::AsanaClient;
use crate::lfd::pm::linear::LinearClient;
use crate::lfd::pm::notion::NotionClient;
use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProvider, PmProviderKind, RoadmapItemDocument,
    RoadmapItemFrontmatter,
};
use crate::lfd::store::open_store;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::ingest::{list_wave_items, WaveItem};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PmInitOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmInitResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub project_id: String,
    pub linked: usize,
    pub created: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PmImportOptions {
    pub team_id: String,
}

#[derive(Debug, Clone)]
pub struct PmImportResult {
    pub waves_created: Vec<String>,
    pub items_created: usize,
}

#[derive(Debug, Clone)]
pub struct PmSyncOptions {
    pub wave: String,
}

#[derive(Debug, Clone)]
pub struct PmSyncResult {
    pub wave: String,
    pub pushed: Vec<String>,
    pub pulled: Vec<String>,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PmPullOptions {
    pub wave: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmPullResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub project_id: String,
    pub local_removed: usize,
    pub local_written: usize,
}

#[derive(Debug, Clone)]
pub struct PmExportOptions {
    pub wave: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmExportResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub project_id: String,
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Default)]
pub struct PmStatusOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmProviderStatus {
    pub provider: PmProviderKind,
    pub project_id: String,
    pub local_total: usize,
    pub linked: usize,
    pub remote_total: usize,
    pub remote_only: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmWaveStatus {
    pub wave: String,
    pub status: PmProviderStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmStatusResult {
    pub waves: Vec<PmWaveStatus>,
}

// ── Provider construction ───────────────────────────────────────────

/// Resolved PM context for a wave: the provider client, provider kind, and project ID (if any).
pub(crate) struct PmContext {
    pub client: Box<dyn PmProvider>,
    pub provider: PmProviderKind,
    pub project: String,
}

fn read_wave_pm_config(repo: &Path, wave: &str) -> Option<WavePmConfig> {
    read_wave_config(repo, wave).and_then(|config| config.pm)
}

fn resolve_provider(repo: &Path, wave: &str) -> OpsResult<PmProviderKind> {
    let config = load_config_or_default(Some(repo));
    let wave_pm = read_wave_pm_config(repo, wave);
    wave_pm
        .as_ref()
        .and_then(|pm| pm.provider)
        .or_else(|| config.pm.as_ref().map(|pm| pm.provider))
        .ok_or_else(|| {
            OpsError::Message(
                "No PM provider configured. Set `pm.provider` in .lf/config.yaml or wave config."
                    .to_string(),
            )
        })
}

async fn build_client(repo: &Path, provider: PmProviderKind) -> OpsResult<Box<dyn PmProvider>> {
    let config = load_config_or_default(Some(repo));
    let client: Box<dyn PmProvider> = match provider {
        PmProviderKind::Asana => {
            let token = resolve_provider_token(
                "asana",
                Some("ASANA_ACCESS_TOKEN"),
                "lf ops auth configure asana",
            )
            .await?;
            Box::new(AsanaClient::new(token, config.asana.clone()))
        }
        PmProviderKind::Linear => {
            let token = resolve_provider_token(
                "linear",
                Some("LINEAR_API_KEY"),
                "lf ops auth configure linear",
            )
            .await?;
            let effective_team = team_id.or_else(|| config.linear.team.clone());
            Box::new(LinearClient::new(token, effective_team))
        }
        PmProviderKind::Notion => {
            let token = resolve_provider_token("notion", None, "lf ops auth notion").await?;
            Box::new(NotionClient::new(token, config.notion.clone()))
        }
    };
    Ok(client)
}

pub(crate) async fn build_provider(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
) -> OpsResult<PmContext> {
    let wave_pm = read_wave_pm_config(repo, wave);
    let project = wave_pm
        .as_ref()
        .and_then(|pm| pm.project_for(provider))
        .unwrap_or("")
        .to_string();
    let client = build_client(repo, provider).await?;
    Ok(PmContext {
        client,
        provider,
        project,
    })
}

pub(crate) async fn build_wave_provider(repo: &Path, wave: &str) -> OpsResult<PmContext> {
    let provider = resolve_provider(repo, wave)?;
    build_provider(repo, wave, provider).await
}

pub(crate) fn wave_pm_is_enabled(repo: &Path, wave: &str) -> bool {
    resolve_provider(repo, wave).ok().is_some_and(|provider| {
        read_wave_pm_config(repo, wave)
            .and_then(|pm| pm.project_for(provider).map(str::to_string))
            .is_some()
    })
}

fn require_project(ctx: &PmContext, wave: &str) -> OpsResult<()> {
    if ctx.project.trim().is_empty() {
        return Err(OpsError::Message(format!(
            "wave/{wave}/{wave}.yaml is missing a project id for {:?}",
            ctx.provider
        )));
    }
    Ok(())
}

async fn resolve_provider_token(
    provider: &str,
    env_name: Option<&str>,
    auth_hint: &str,
) -> OpsResult<String> {
    if let Some(env_name) = env_name {
        if let Ok(token) = std::env::var(env_name) {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open lfd credential store: {err}")))?;
    let token = store
        .get_provider_token(provider)
        .await
        .map_err(|err| OpsError::Message(format!("failed to load {provider} token: {err}")))?
        .ok_or_else(|| {
            OpsError::Message(format!(
                "No {provider} credential found. Run `{auth_hint}`."
            ))
        })?;

    if token
        .expires_at
        .is_some_and(|expires_at| expires_at <= time::OffsetDateTime::now_utc().unix_timestamp())
    {
        return Err(OpsError::Message(format!(
            "Stored {provider} token has expired. Run `lf op auth {provider}` again."
        )));
    }

    Ok(token.access_token)
}

fn storage_config_from_env() -> OpsResult<crate::lfd::store::StorageConfig> {
    crate::lfd::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve lfd credential store: {err}")))
}

fn normalize_title(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone)]
struct LocalRoadmapItem {
    item: WaveItem,
    doc: RoadmapItemDocument,
}

struct BootstrapArgs<'a> {
    repo: &'a Path,
    wave: &'a str,
    wave_dir: &'a Path,
    project_name: &'a str,
    description: &'a str,
    progress: &'a dyn Progress,
}

fn read_local_roadmap_items(wave_dir: &Path) -> OpsResult<Vec<LocalRoadmapItem>> {
    let mut items = Vec::new();
    for item in list_wave_items(wave_dir)? {
        let path = wave_dir.join(&item.filename);
        let content = std::fs::read_to_string(&path)?;
        items.push(LocalRoadmapItem {
            item,
            doc: RoadmapItemDocument::parse(&content).map_err(pm_to_ops)?,
        });
    }
    Ok(items)
}

fn local_item_title(item: &LocalRoadmapItem) -> String {
    extract_heading(&item.doc.body)
        .map(str::to_string)
        .unwrap_or_else(|| title_case(&item.item.slug))
}

fn project_key(provider: PmProviderKind) -> &'static str {
    match provider {
        PmProviderKind::Asana => "asana_project",
        PmProviderKind::Linear => "linear_project",
        PmProviderKind::Notion => "notion_project",
    }
}

fn yaml_string(value: &str) -> serde_yaml_ng::Value {
    serde_yaml_ng::Value::String(value.to_string())
}

fn update_wave_pm_yaml(
    repo: &Path,
    wave: &str,
    update: impl FnOnce(&mut serde_yaml_ng::Mapping) -> OpsResult<()>,
) -> OpsResult<()> {
    let path = repo.join("wave").join(wave).join(format!("{wave}.yaml"));
    let mut value = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&content)
            .map_err(|err| OpsError::Message(format!("invalid wave yaml: {err}")))?
    } else {
        serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new())
    };

    let map = value
        .as_mapping_mut()
        .ok_or_else(|| OpsError::Message("wave config must be a mapping".to_string()))?;
    let pm_key = yaml_string("pm");
    let mut pm_map = map
        .get(&pm_key)
        .and_then(serde_yaml_ng::Value::as_mapping)
        .cloned()
        .unwrap_or_default();

    update(&mut pm_map)?;
    map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));

    let output = serde_yaml_ng::to_string(&value)
        .map_err(|err| OpsError::Message(format!("failed to encode wave yaml: {err}")))?;
    std::fs::write(&path, output)?;
    Ok(())
}

pub(crate) fn write_pm_provider_to_wave_yaml(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
) -> OpsResult<()> {
    update_wave_pm_yaml(repo, wave, |pm_map| {
        pm_map.insert(
            yaml_string("provider"),
            serde_yaml_ng::to_value(provider)
                .map_err(|err| OpsError::Message(format!("failed to encode pm provider: {err}")))?,
        );
        Ok(())
    })
}

pub(crate) fn write_pm_project_to_wave_yaml(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
    project_id: &str,
) -> OpsResult<()> {
    update_wave_pm_yaml(repo, wave, |pm_map| {
        pm_map.insert(yaml_string(project_key(provider)), yaml_string(project_id));
        Ok(())
    })
}

// ── Bootstrap / status ─────────────────────────────────────────────

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
            "wave directory not found: {}",
            wave_dir.display()
        )));
    }

    let provider_kind = resolve_provider(repo, &wave)?;
    write_pm_provider_to_wave_yaml(repo, &wave, provider_kind)?;

    let (project_name, description) = read_wave_project_metadata(repo, &wave)?;
    let mut local_items = read_local_roadmap_items(&wave_dir)?;
    let bootstrap = BootstrapArgs {
        repo,
        wave: &wave,
        wave_dir: &wave_dir,
        project_name: &project_name,
        description: &description,
        progress,
    };

    let ctx = build_provider(repo, &wave, provider_kind).await?;
    let rw_result = bootstrap_read_write_provider(&bootstrap, &mut local_items, ctx).await?;

    let commit_message = format!("lf pm: bootstrap {wave}");
    let _ = crate::ops::commit_workflow(
        repo,
        &crate::ops::CommitOptions {
            add: true,
            message: Some(commit_message),
            ..crate::ops::CommitOptions::for_task("pm")
        },
        progress,
    )?;

    Ok(PmInitResult {
        wave,
        provider: rw_result.provider,
        project_id: rw_result.project_id,
        linked: rw_result.linked,
        created: rw_result.created_local,
    })
}

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
        let provider_kind = match resolve_provider(repo, &wave) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let wave_dir = repo.join("wave").join(&wave);
        let local_items = if wave_dir.is_dir() {
            read_local_roadmap_items(&wave_dir)?
        } else {
            Vec::new()
        };
        let local_titles = local_items
            .iter()
            .map(local_item_title)
            .map(|title| normalize_title(&title))
            .collect::<HashSet<_>>();

        let ctx = build_provider(repo, &wave, provider_kind).await?;
        if ctx.project.trim().is_empty() {
            continue;
        }
        progress.status(&format!("checking {:?} for wave/{wave}", provider_kind));
        let remote_items = ctx
            .client
            .list_items(&ctx.project)
            .await
            .map_err(pm_to_ops)?;
        let linked_ids = local_items
            .iter()
            .filter_map(|item| item.doc.frontmatter.id_for(provider_kind))
            .collect::<HashSet<_>>();
        let remote_only = remote_items
            .iter()
            .filter(|item| {
                !local_titles.contains(&normalize_title(&item.name))
                    && !linked_ids.contains(item.id.as_str())
            })
            .count();

        results.push(PmWaveStatus {
            wave,
            status: PmProviderStatus {
                provider: provider_kind,
                project_id: ctx.project,
                local_total: local_items.len(),
                linked: linked_ids.len(),
                remote_total: remote_items.len(),
                remote_only,
            },
        });
    }

    Ok(PmStatusResult { waves: results })
}

#[derive(Debug, Clone)]
struct BootstrapResult {
    provider: PmProviderKind,
    project_id: String,
    linked: usize,
    created_local: Vec<String>,
}

async fn bootstrap_read_write_provider(
    args: &BootstrapArgs<'_>,
    local_items: &mut [LocalRoadmapItem],
    ctx: PmContext,
) -> OpsResult<BootstrapResult> {
    let project_id = ensure_project(
        args.repo,
        args.wave,
        args.project_name,
        args.description,
        &ctx,
        args.progress,
    )
    .await?;
    let remote_items = ctx
        .client
        .list_items(&project_id)
        .await
        .map_err(pm_to_ops)?;
    let remote_by_id = remote_items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut remote_by_title = remote_items
        .iter()
        .map(|item| (normalize_title(&item.name), item))
        .collect::<HashMap<_, _>>();
    let mut matched_remote_ids = HashSet::new();
    let mut created_local = Vec::new();
    let mut pending_remote_creates = Vec::new();

    for (index, local_item) in local_items.iter_mut().enumerate() {
        let local_title = local_item_title(local_item);
        let local_key = normalize_title(&local_title);
        if let Some(pm_id) = local_item.doc.frontmatter.id_for(ctx.provider) {
            if let Some(remote_item) = remote_by_id.get(pm_id) {
                matched_remote_ids.insert(remote_item.id.clone());
                apply_remote_match(args.wave_dir, local_item, ctx.provider, remote_item, false)?;
                continue;
            }
        }

        if let Some(remote_item) = remote_by_title.remove(&local_key) {
            matched_remote_ids.insert(remote_item.id.clone());
            apply_remote_match(args.wave_dir, local_item, ctx.provider, remote_item, false)?;
            continue;
        }

        pending_remote_creates.push((index, local_title));
    }

    // Both supported PM providers append new items, so create from the bottom up
    // to preserve the local roadmap order.
    for (index, local_title) in pending_remote_creates.into_iter().rev() {
        create_remote_for_local_item(
            args.wave_dir,
            &mut local_items[index],
            &ctx,
            &project_id,
            &local_title,
        )
        .await?;
    }

    for remote_item in &remote_items {
        if matched_remote_ids.contains(&remote_item.id) {
            continue;
        }
        let filename = next_remote_filename(args.wave_dir, remote_item.rank, &remote_item.name);
        write_remote_item(&args.wave_dir.join(&filename), remote_item, ctx.provider)?;
        created_local.push(filename);
    }

    Ok(BootstrapResult {
        provider: ctx.provider,
        project_id,
        linked: count_linked_items(local_items, ctx.provider) + created_local.len(),
        created_local,
    })
}

async fn ensure_project(
    repo: &Path,
    wave: &str,
    project_name: &str,
    description: &str,
    ctx: &PmContext,
    progress: &(impl Progress + ?Sized),
) -> OpsResult<String> {
    if !ctx.project.trim().is_empty() {
        progress.status(&format!(
            "using existing {:?} project {}",
            ctx.provider, ctx.project
        ));
        return Ok(ctx.project.clone());
    }

    progress.status(&format!(
        "creating {:?} project for wave/{wave}",
        ctx.provider
    ));
    let project_id = ctx
        .client
        .create_project(project_name, description)
        .await
        .map_err(pm_to_ops)?;
    write_pm_project_to_wave_yaml(repo, wave, ctx.provider, &project_id)?;
    Ok(project_id)
}

fn read_wave_project_metadata(repo: &Path, wave: &str) -> OpsResult<(String, String)> {
    let readme_path = repo.join("wave").join(wave).join("README.md");
    let content = match std::fs::read_to_string(&readme_path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok((title_case(wave), String::new()));
        }
        Err(err) => return Err(err.into()),
    };
    Ok((
        extract_heading(&content)
            .map(str::to_string)
            .unwrap_or_else(|| title_case(wave)),
        body_without_heading(&content).trim().to_string(),
    ))
}

fn write_local_item(wave_dir: &Path, item: &LocalRoadmapItem) -> OpsResult<()> {
    let rendered = item.doc.render().map_err(pm_to_ops)?;
    std::fs::write(wave_dir.join(&item.item.filename), rendered)?;
    Ok(())
}

fn apply_remote_match(
    wave_dir: &Path,
    local_item: &mut LocalRoadmapItem,
    provider: PmProviderKind,
    remote_item: &PmItem,
    update_body: bool,
) -> OpsResult<()> {
    local_item
        .doc
        .frontmatter
        .set_id(provider, remote_item.id.clone());
    if update_body {
        local_item.doc.body = render_remote_body(remote_item);
    }
    write_local_item(wave_dir, local_item)
}

async fn create_remote_for_local_item(
    wave_dir: &Path,
    local_item: &mut LocalRoadmapItem,
    ctx: &PmContext,
    project_id: &str,
    title: &str,
) -> OpsResult<()> {
    let pm_id = ctx
        .client
        .create_item(
            project_id,
            &PmItemCreate {
                name: title.to_string(),
                description: body_without_heading(&local_item.doc.body)
                    .trim()
                    .to_string(),
                rank: local_item.item.rank(),
            },
        )
        .await
        .map_err(pm_to_ops)?;
    local_item.doc.frontmatter.set_id(ctx.provider, pm_id);
    write_local_item(wave_dir, local_item)
}

fn count_linked_items(items: &[LocalRoadmapItem], provider: PmProviderKind) -> usize {
    items
        .iter()
        .filter(|item| item.doc.frontmatter.id_for(provider).is_some())
        .count()
}

fn next_remote_filename(wave_dir: &Path, rank: u32, title: &str) -> String {
    next_item_filename(rank, title, |filename| !wave_dir.join(filename).exists())
}

fn next_item_filename(
    rank: u32,
    title: &str,
    mut is_available: impl FnMut(&str) -> bool,
) -> String {
    let slug = slugify(title);
    let mut prefix = rank;
    loop {
        let filename = format!("{:02}-{slug}.md", prefix + 1);
        if is_available(&filename) {
            return filename;
        }
        prefix += 1;
    }
}

fn overwrite_local_wave_from_remote(
    wave_dir: &Path,
    remote_items: &[PmItem],
    provider: PmProviderKind,
) -> OpsResult<usize> {
    let local_files = list_wave_items(wave_dir)?;
    let removed = local_files.len();
    for item in local_files {
        std::fs::remove_file(wave_dir.join(item.filename))?;
    }

    let mut used_filenames = HashSet::new();
    for (index, remote_item) in remote_items.iter().enumerate() {
        let filename = next_item_filename(index as u32, &remote_item.name, |filename| {
            !used_filenames.contains(filename)
        });
        used_filenames.insert(filename.clone());
        write_remote_item(&wave_dir.join(filename), remote_item, provider)?;
    }

    Ok(removed)
}

fn list_pm_waves(repo: &Path) -> OpsResult<Vec<String>> {
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
        if read_wave_pm_config(repo, &name).is_some() {
            waves.push(name);
        }
    }
    waves.sort();
    Ok(waves)
}

// ── Import ──────────────────────────────────────────────────────────

pub fn pm_import(
    repo: &Path,
    options: &PmImportOptions,
    progress: &impl Progress,
) -> OpsResult<PmImportResult> {
    block_on_pm(pm_import_async(repo, options, progress))
}

async fn pm_import_async(
    repo: &Path,
    options: &PmImportOptions,
    progress: &impl Progress,
) -> OpsResult<PmImportResult> {
    // Get the provider from global config (no wave context yet)
    let config = load_config_or_default(Some(repo));
    let provider_kind = config.pm.as_ref().map(|pm| pm.provider).ok_or_else(|| {
        OpsError::Message(
            "No PM provider configured. Set `pm.provider` in .lf/config.yaml.".to_string(),
        )
    })?;
    let client = build_client(repo, provider_kind).await?;

    progress.status(&format!(
        "listing projects in {:?} team {}",
        provider_kind, options.team_id
    ));
    let projects = client
        .list_projects(&options.team_id)
        .await
        .map_err(pm_to_ops)?;

    let mut waves_created = Vec::new();
    let mut items_created = 0usize;

    for project in &projects {
        let wave_name = slugify(&project.name);
        let wave_dir = repo.join("wave").join(&wave_name);
        if !wave_dir.is_dir() {
            std::fs::create_dir_all(&wave_dir)?;
        }

        // Write wave config
        let wave_yaml_path = wave_dir.join(format!("{wave_name}.yaml"));
        if !wave_yaml_path.exists() {
            std::fs::write(&wave_yaml_path, "flow: build\n")?;
        }
        write_pm_provider_to_wave_yaml(repo, &wave_name, provider_kind)?;
        write_pm_project_to_wave_yaml(repo, &wave_name, provider_kind, &project.id)?;

        // Pull items
        let remote_items = client.list_items(&project.id).await.map_err(pm_to_ops)?;
        let existing_items = read_local_roadmap_items(&wave_dir).unwrap_or_default();
        let existing_ids: std::collections::HashSet<String> = existing_items
            .iter()
            .filter_map(|item| {
                item.doc
                    .frontmatter
                    .id_for(provider_kind)
                    .map(str::to_string)
            })
            .collect();

        for remote_item in &remote_items {
            if existing_ids.contains(&remote_item.id) {
                continue; // additive only — skip existing
            }
            let filename = next_remote_filename(&wave_dir, remote_item.rank, &remote_item.name);
            write_remote_item(&wave_dir.join(&filename), remote_item, provider_kind)?;
            items_created += 1;
        }

        waves_created.push(wave_name);
    }

    Ok(PmImportResult {
        waves_created,
        items_created,
    })
}

// ── Sync (three-way) ───────────────────────────────────────────────

pub fn pm_sync(
    repo: &Path,
    options: &PmSyncOptions,
    progress: &impl Progress,
) -> OpsResult<PmSyncResult> {
    block_on_pm(pm_sync_async(repo, options, progress))
}

pub fn pm_pull(
    repo: &Path,
    options: &PmPullOptions,
    progress: &impl Progress,
) -> OpsResult<PmPullResult> {
    block_on_pm(pm_pull_async(repo, options, progress))
}

pub fn pm_export(
    repo: &Path,
    options: &PmExportOptions,
    progress: &impl Progress,
) -> OpsResult<PmExportResult> {
    block_on_pm(pm_export_async(repo, options, progress))
}

async fn pm_pull_async(
    repo: &Path,
    options: &PmPullOptions,
    progress: &impl Progress,
) -> OpsResult<PmPullResult> {
    let wave_dir = repo.join("wave").join(&options.wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{}/",
            options.wave
        )));
    }

    let ctx = build_wave_provider(repo, &options.wave).await?;
    require_project(&ctx, &options.wave)?;

    progress.status(&format!(
        "pulling {:?} project {} into wave/{}",
        ctx.provider, ctx.project, options.wave
    ));
    let remote_items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;
    let local_removed = overwrite_local_wave_from_remote(&wave_dir, &remote_items, ctx.provider)?;

    Ok(PmPullResult {
        wave: options.wave.clone(),
        provider: ctx.provider,
        project_id: ctx.project,
        local_removed,
        local_written: remote_items.len(),
    })
}

async fn pm_export_async(
    repo: &Path,
    options: &PmExportOptions,
    progress: &impl Progress,
) -> OpsResult<PmExportResult> {
    let wave_dir = repo.join("wave").join(&options.wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{}/",
            options.wave
        )));
    }

    let ctx = build_wave_provider(repo, &options.wave).await?;
    require_project(&ctx, &options.wave)?;

    progress.status(&format!(
        "exporting wave/{} to {:?} project {}",
        options.wave, ctx.provider, ctx.project
    ));
    let export = export_local_wave_to_remote(&wave_dir, &ctx, progress).await?;

    Ok(PmExportResult {
        wave: options.wave.clone(),
        provider: ctx.provider,
        project_id: ctx.project,
        created: export.created,
        updated: export.updated,
        skipped: export.skipped,
    })
}

async fn pm_sync_async(
    repo: &Path,
    options: &PmSyncOptions,
    progress: &impl Progress,
) -> OpsResult<PmSyncResult> {
    let wave_dir = repo.join("wave").join(&options.wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{}/",
            options.wave
        )));
    }

    let ctx = build_wave_provider(repo, &options.wave).await?;
    require_project(&ctx, &options.wave)?;
    let main_branch = get_default_branch(repo)
        .map_err(|err| OpsError::Message(format!("failed to determine main branch: {err}")))?;

    progress.status(&format!(
        "syncing wave/{} with {:?} (base: {main_branch})",
        options.wave, ctx.provider
    ));

    let provider_kind = ctx.provider;

    // 1. Read base state from main branch
    let base_items = read_base_items(repo, &main_branch, &options.wave, provider_kind)?;

    // 2. Read local state from disk
    let local_items = read_local_items(&wave_dir, provider_kind)?;

    // 3. Fetch remote state
    let remote_items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;
    let remote_by_id: HashMap<&str, &PmItem> = remote_items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();

    // 4. Three-way diff and apply
    let mut pushed = Vec::new();
    let mut pulled = Vec::new();
    let mut conflicts = Vec::new();

    // Collect all pm_ids across all three states
    let mut all_pm_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for pm_id in base_items.keys() {
        all_pm_ids.insert(pm_id.clone());
    }
    for pm_id in local_items.keys() {
        all_pm_ids.insert(pm_id.clone());
    }
    for item in &remote_items {
        all_pm_ids.insert(item.id.clone());
    }

    for pm_id in &all_pm_ids {
        let base = base_items.get(pm_id.as_str());
        let local = local_items.get(pm_id.as_str());
        let remote = remote_by_id.get(pm_id.as_str());

        match (base, local, remote) {
            // Exists in all three — check for changes
            (Some(base_doc), Some((local_file, local_doc)), Some(remote_item)) => {
                let base_title = extract_heading(&base_doc.body);
                let local_title = extract_heading(&local_doc.body);
                let remote_title = Some(remote_item.name.as_str());

                let local_changed = local_title != base_title || local_doc.body != base_doc.body;
                let remote_changed =
                    remote_title != base_title || render_remote_body(remote_item) != base_doc.body;

                if local_changed && remote_changed {
                    progress.status(&format!("  conflict: {}", local_file));
                    conflicts.push(local_file.clone());
                } else if local_changed {
                    // Push local → remote
                    let name = local_title.unwrap_or(&remote_item.name);
                    ctx.client
                        .update_item(
                            pm_id,
                            &PmItemUpdate {
                                name: Some(name.to_string()),
                                description: Some(local_doc.body.clone()),
                                rank: None,
                            },
                        )
                        .await
                        .map_err(pm_to_ops)?;
                    progress.status(&format!("  pushed {}", local_file));
                    pushed.push(local_file.clone());
                } else if remote_changed {
                    // Pull remote → local
                    let path = wave_dir.join(local_file);
                    write_remote_item(&path, remote_item, provider_kind)?;
                    progress.status(&format!("  pulled {local_file}"));
                    pulled.push(local_file.clone());
                }
                // else: no-op
            }

            // Not in base, exists locally — new local item, push
            (None, Some((local_file, local_doc)), None) => {
                let name = extract_heading(&local_doc.body).unwrap_or("untitled");
                let new_id = ctx
                    .client
                    .create_item(
                        &ctx.project,
                        &PmItemCreate {
                            name: name.to_string(),
                            description: local_doc.body.clone(),
                            rank: 0,
                        },
                    )
                    .await
                    .map_err(pm_to_ops)?;

                // Write pm_id back to file
                let mut updated_doc = local_doc.clone();
                updated_doc.frontmatter.set_id(provider_kind, new_id);
                let rendered = updated_doc.render().map_err(pm_to_ops)?;
                std::fs::write(wave_dir.join(local_file), rendered)?;
                progress.status(&format!("  pushed (new) {local_file}"));
                pushed.push(local_file.clone());
            }

            // Not in base, exists remotely — new remote item, pull
            (None, None, Some(remote_item)) => {
                let filename = format!(
                    "{:02}-{}.md",
                    remote_item.rank + 1,
                    slugify(&remote_item.name)
                );
                write_remote_item(&wave_dir.join(&filename), remote_item, provider_kind)?;
                progress.status(&format!("  pulled (new) {filename}"));
                pulled.push(filename);
            }

            // In base, deleted locally, unchanged remotely — archive remote
            (Some(base_doc), None, Some(remote_item)) => {
                let remote_changed = Some(remote_item.name.as_str())
                    != extract_heading(&base_doc.body)
                    || render_remote_body(remote_item) != base_doc.body;
                if remote_changed {
                    progress.status(&format!(
                        "  conflict (deleted locally, changed remotely): {pm_id}"
                    ));
                    conflicts.push(pm_id.clone());
                } else {
                    if !remote_item.completed {
                        ctx.client.complete_item(pm_id).await.map_err(pm_to_ops)?;
                    }
                    progress.status(&format!("  archived remote {pm_id}"));
                    pushed.push(format!("(archived) {pm_id}"));
                }
            }

            // In base, unchanged locally, deleted remotely — delete local
            (Some(base_doc), Some((local_file, local_doc)), None) => {
                let local_changed = local_doc.body != base_doc.body;
                if local_changed {
                    progress.status(&format!(
                        "  conflict (changed locally, deleted remotely): {local_file}"
                    ));
                    conflicts.push(local_file.clone());
                } else {
                    std::fs::remove_file(wave_dir.join(local_file))?;
                    progress.status(&format!("  deleted {local_file}"));
                    pulled.push(local_file.clone());
                }
            }

            // Already exists in both local and remote but wasn't in base — both new
            (None, Some((local_file, _)), Some(_)) => {
                progress.status(&format!("  conflict (new on both sides): {local_file}"));
                conflicts.push(local_file.clone());
            }

            // In base but gone from both — already reconciled
            (Some(_), None, None) => {}

            // Shouldn't happen (not in any state)
            (None, None, None) => {}
        }
    }

    Ok(PmSyncResult {
        wave: options.wave.clone(),
        pushed,
        pulled,
        conflicts,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────

fn read_base_items(
    repo: &Path,
    main_branch: &str,
    wave: &str,
    provider: PmProviderKind,
) -> OpsResult<HashMap<String, RoadmapItemDocument>> {
    let wave_path = format!("wave/{wave}");
    let files = list_tree(repo, main_branch, &wave_path)
        .map_err(|err| OpsError::Message(format!("failed to list base wave files: {err}")))?;

    let mut items = HashMap::new();
    for filename in files {
        if !filename.ends_with(".md") || filename.eq_ignore_ascii_case("README.md") {
            continue;
        }
        let file_path = format!("{wave_path}/{filename}");
        if let Some(content) = show_file(repo, main_branch, &file_path)
            .map_err(|err| OpsError::Message(format!("failed to read base file: {err}")))?
        {
            if let Ok(doc) = RoadmapItemDocument::parse(&content) {
                if let Some(pm_id) = doc.frontmatter.id_for(provider) {
                    items.insert(pm_id.to_string(), doc);
                }
            }
        }
    }
    Ok(items)
}

fn read_local_items(
    wave_dir: &Path,
    provider: PmProviderKind,
) -> OpsResult<HashMap<String, (String, RoadmapItemDocument)>> {
    let items = list_wave_items(wave_dir).unwrap_or_default();
    let mut result = HashMap::new();
    for item in items {
        let path = wave_dir.join(&item.filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(doc) = RoadmapItemDocument::parse(&content) {
                if let Some(pm_id) = doc.frontmatter.id_for(provider) {
                    result.insert(pm_id.to_string(), (item.filename.clone(), doc));
                }
            }
        }
    }
    Ok(result)
}

fn remote_item_to_document(item: &PmItem, provider: PmProviderKind) -> RoadmapItemDocument {
    let mut frontmatter = RoadmapItemFrontmatter::default();
    frontmatter.set_id(provider, item.id.clone());

    RoadmapItemDocument {
        frontmatter,
        body: render_remote_body(item),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PmExportCounts {
    created: usize,
    updated: usize,
    skipped: usize,
}

async fn export_local_wave_to_remote(
    wave_dir: &Path,
    ctx: &PmContext,
    progress: &impl Progress,
) -> OpsResult<PmExportCounts> {
    let remote_items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;
    let remote_by_id: HashMap<&str, &PmItem> = remote_items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect();
    let mut counts = PmExportCounts::default();
    let mut local_items = read_local_roadmap_items(wave_dir)?;

    for local_item in &mut local_items {
        let title = local_item_title(local_item);
        let description = body_without_heading(&local_item.doc.body)
            .trim()
            .to_string();

        match local_item.doc.frontmatter.id_for(ctx.provider) {
            Some(pm_id) => {
                let Some(remote_item) = remote_by_id.get(pm_id) else {
                    progress.status(&format!(
                        "  skipped {} (remote {} missing)",
                        local_item.item.filename, pm_id
                    ));
                    counts.skipped += 1;
                    continue;
                };

                let update = build_text_update(&title, &description, remote_item);
                if update.name.is_none() && update.description.is_none() {
                    counts.skipped += 1;
                    continue;
                }

                ctx.client
                    .update_item(pm_id, &update)
                    .await
                    .map_err(pm_to_ops)?;
                progress.status(&format!("  updated {}", local_item.item.filename));
                counts.updated += 1;
            }
            None => {
                let pm_id = ctx
                    .client
                    .create_item(
                        &ctx.project,
                        &PmItemCreate {
                            name: title,
                            description,
                            rank: local_item.item.rank(),
                        },
                    )
                    .await
                    .map_err(pm_to_ops)?;
                local_item.doc.frontmatter.set_id(ctx.provider, pm_id);
                write_local_item(wave_dir, local_item)?;
                progress.status(&format!("  created {}", local_item.item.filename));
                counts.created += 1;
            }
        }
    }

    Ok(counts)
}

fn build_text_update(title: &str, description: &str, remote_item: &PmItem) -> PmItemUpdate {
    PmItemUpdate {
        name: (title != remote_item.name).then(|| title.to_string()),
        description: (description != remote_item.description.trim())
            .then(|| description.to_string()),
        rank: None,
    }
}

fn write_remote_item(path: &Path, item: &PmItem, provider: PmProviderKind) -> OpsResult<()> {
    let rendered = remote_item_to_document(item, provider)
        .render()
        .map_err(pm_to_ops)?;
    std::fs::write(path, rendered)?;
    Ok(())
}

fn render_remote_body(item: &PmItem) -> String {
    let heading = format!("# {}\n", item.name);
    if item.description.is_empty() {
        heading
    } else {
        format!("{heading}\n{}\n", item.description.trim())
    }
}

/// Extract the first `# ` heading, stripping any `NN:` or `NN-` prefix.
pub(crate) fn extract_heading(body: &str) -> Option<&str> {
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                return Some(strip_number_prefix(heading));
            }
        }
    }
    None
}

/// Strip a leading `NN:` or `NN-` prefix from a heading.
fn strip_number_prefix(heading: &str) -> &str {
    let bytes = heading.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && (bytes[i] == b':' || bytes[i] == b'-') {
        heading[i + 1..].trim_start()
    } else {
        heading
    }
}

/// Return the body with the first `# ` heading line removed.
pub(crate) fn body_without_heading(body: &str) -> &str {
    for (i, line) in body.lines().enumerate() {
        if line.starts_with("# ") {
            // Skip heading line and any immediately following blank line
            let after_heading = &body[body.find(line).unwrap() + line.len()..];
            return after_heading.strip_prefix('\n').unwrap_or(after_heading);
        }
        // Only look at the first non-empty line
        if i > 0 && !line.trim().is_empty() {
            break;
        }
    }
    body
}

fn slugify(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Convert a slug like "agent-embedding" to "Agent Embedding".
pub(crate) fn title_case(slug: &str) -> String {
    slug.split('-')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
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
    use crate::lfd::pm::PmResult;
    use async_trait::async_trait;
    use tempfile::TempDir;

    #[derive(Debug)]
    struct StaticProvider {
        items: Vec<PmItem>,
    }

    #[derive(Debug)]
    struct RecordingProvider {
        created: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        next_id: std::sync::Arc<std::sync::Mutex<u32>>,
    }

    #[derive(Debug)]
    struct MutableProvider {
        items: std::sync::Arc<std::sync::Mutex<Vec<PmItem>>>,
        next_id: std::sync::Arc<std::sync::Mutex<u32>>,
    }

    #[async_trait]
    impl PmProvider for StaticProvider {
        async fn create_project(&self, _name: &str, _description: &str) -> PmResult<String> {
            panic!("create_project should not be called in this test");
        }

        async fn list_projects(&self, _team_id: &str) -> PmResult<Vec<crate::lfd::pm::PmProject>> {
            panic!("list_projects should not be called in this test");
        }

        async fn list_items(&self, _project_id: &str) -> PmResult<Vec<PmItem>> {
            Ok(self.items.clone())
        }

        async fn create_item(&self, _project_id: &str, _item: &PmItemCreate) -> PmResult<String> {
            panic!("create_item should not be called in this test");
        }

        async fn update_item(&self, _item_id: &str, _update: &PmItemUpdate) -> PmResult<()> {
            panic!("update_item should not be called in this test");
        }

        async fn complete_item(&self, _item_id: &str) -> PmResult<()> {
            panic!("complete_item should not be called in this test");
        }

        async fn comment(&self, _item_id: &str, _body: &str) -> PmResult<()> {
            panic!("comment should not be called in this test");
        }
    }

    #[async_trait]
    impl PmProvider for RecordingProvider {
        async fn create_project(&self, _name: &str, _description: &str) -> PmResult<String> {
            panic!("create_project should not be called in this test");
        }

        async fn list_projects(&self, _team_id: &str) -> PmResult<Vec<crate::lfd::pm::PmProject>> {
            panic!("list_projects should not be called in this test");
        }

        async fn list_items(&self, _project_id: &str) -> PmResult<Vec<PmItem>> {
            Ok(Vec::new())
        }

        async fn create_item(&self, _project_id: &str, item: &PmItemCreate) -> PmResult<String> {
            self.created
                .lock()
                .expect("created lock")
                .push(item.name.clone());
            let mut next_id = self.next_id.lock().expect("next_id lock");
            *next_id += 1;
            Ok(format!("lin-{}", *next_id))
        }

        async fn update_item(&self, _item_id: &str, _update: &PmItemUpdate) -> PmResult<()> {
            panic!("update_item should not be called in this test");
        }

        async fn complete_item(&self, _item_id: &str) -> PmResult<()> {
            panic!("complete_item should not be called in this test");
        }

        async fn comment(&self, _item_id: &str, _body: &str) -> PmResult<()> {
            panic!("comment should not be called in this test");
        }
    }

    #[async_trait]
    impl PmProvider for MutableProvider {
        async fn create_project(&self, _name: &str, _description: &str) -> PmResult<String> {
            panic!("create_project should not be called in this test");
        }

        async fn list_projects(&self, _team_id: &str) -> PmResult<Vec<crate::lfd::pm::PmProject>> {
            panic!("list_projects should not be called in this test");
        }

        async fn list_items(&self, _project_id: &str) -> PmResult<Vec<PmItem>> {
            Ok(self.items.lock().expect("items lock").clone())
        }

        async fn create_item(&self, _project_id: &str, item: &PmItemCreate) -> PmResult<String> {
            let mut next_id = self.next_id.lock().expect("next_id lock");
            *next_id += 1;
            let id = format!("lin-{}", *next_id);
            self.items.lock().expect("items lock").push(PmItem {
                id: id.clone(),
                name: item.name.clone(),
                description: item.description.clone(),
                rank: item.rank,
                completed: false,
            });
            Ok(id)
        }

        async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> PmResult<()> {
            let mut items = self.items.lock().expect("items lock");
            let item = items
                .iter_mut()
                .find(|item| item.id == item_id)
                .expect("item should exist");
            if let Some(name) = update.name.as_deref() {
                item.name = name.to_string();
            }
            if let Some(description) = update.description.as_deref() {
                item.description = description.to_string();
            }
            Ok(())
        }

        async fn complete_item(&self, _item_id: &str) -> PmResult<()> {
            panic!("complete_item should not be called in this test");
        }

        async fn comment(&self, _item_id: &str, _body: &str) -> PmResult<()> {
            panic!("comment should not be called in this test");
        }
    }

    #[test]
    fn slugify_converts_name_to_filename_slug() {
        assert_eq!(slugify("Ship Linear Client"), "ship-linear-client");
        assert_eq!(slugify("01: Auth & Setup"), "01-auth-setup");
        assert_eq!(slugify("  spaces  "), "spaces");
    }

    #[test]
    fn normalize_title_collapses_case_and_whitespace() {
        assert_eq!(
            normalize_title("  Ship   Linear   Client "),
            "ship linear client"
        );
    }

    #[test]
    fn remote_item_to_document_sets_provider_id_and_heading() {
        let item = PmItem {
            id: "item-42".to_string(),
            name: "Build the thing".to_string(),
            description: "Some details here.".to_string(),
            rank: 0,
            completed: false,
        };

        let doc = remote_item_to_document(&item, PmProviderKind::Linear);
        assert_eq!(doc.frontmatter.linear_id.as_deref(), Some("item-42"));
        assert_eq!(doc.frontmatter.asana_id, None);
        assert_eq!(doc.frontmatter.notion_id, None);
        assert!(doc.body.starts_with("# Build the thing\n"));
        assert!(doc.body.contains("Some details here."));
    }

    #[test]
    fn remote_item_to_document_handles_empty_description() {
        let item = PmItem {
            id: "item-1".to_string(),
            name: "Empty".to_string(),
            description: String::new(),
            rank: 0,
            completed: false,
        };

        let doc = remote_item_to_document(&item, PmProviderKind::Asana);
        assert_eq!(doc.body, "# Empty\n");
    }

    #[test]
    fn extract_heading_strips_number_prefix() {
        assert_eq!(
            extract_heading("# 03: Linear client\n\nSome description"),
            Some("Linear client")
        );
        assert_eq!(
            extract_heading("# No prefix here\n"),
            Some("No prefix here")
        );
    }

    #[test]
    fn extract_heading_returns_none_without_h1() {
        assert_eq!(extract_heading("no heading here\n"), None);
    }

    #[test]
    fn body_without_heading_strips_first_h1() {
        assert_eq!(
            body_without_heading("# Title\n\nBody text."),
            "\nBody text."
        );
        assert_eq!(
            body_without_heading("No heading\nJust text"),
            "No heading\nJust text"
        );
    }

    #[test]
    fn write_pm_provider_to_wave_yaml_sets_provider_field() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("pm.yaml"), "flow: build\n").expect("write wave config");

        write_pm_provider_to_wave_yaml(dir.path(), "pm", PmProviderKind::Linear)
            .expect("write pm provider");

        let content = std::fs::read_to_string(wave_dir.join("pm.yaml")).expect("read wave config");
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).expect("parse yaml");
        let pm = value.get("pm").expect("pm block");
        assert_eq!(
            pm.get("provider").and_then(|value| value.as_str()),
            Some("linear")
        );
    }

    #[test]
    fn wave_pm_is_enabled_requires_provider_project() {
        let dir = TempDir::new().expect("temp dir");
        let repo = dir.path();
        std::fs::create_dir_all(repo.join(".lf")).expect("create lf dir");
        std::fs::write(repo.join(".lf/config.yaml"), "pm:\n  provider: linear\n")
            .expect("write config");

        let wave_dir = repo.join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("pm.yaml"),
            "flow: build\npm:\n  asana_project: \"asa-1\"\n",
        )
        .expect("write wave config");
        assert!(!wave_pm_is_enabled(repo, "pm"));

        std::fs::write(
            wave_dir.join("pm.yaml"),
            "flow: build\npm:\n  linear_project: \"lin-1\"\n  asana_project: \"asa-1\"\n",
        )
        .expect("rewrite wave config");
        assert!(wave_pm_is_enabled(repo, "pm"));
    }

    #[tokio::test]
    async fn bootstrap_read_write_provider_preserves_local_body_and_uses_remote_rank() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("01-existing.md"),
            "# Existing task\n\nLocal body stays put.\n",
        )
        .expect("write local item");

        let mut local_items = read_local_roadmap_items(&wave_dir).expect("read local items");
        let progress = crate::ops::NullProgress;
        let args = BootstrapArgs {
            repo: dir.path(),
            wave: "pm",
            wave_dir: &wave_dir,
            project_name: "PM",
            description: "",
            progress: &progress,
        };
        let ctx = PmContext {
            client: Box::new(StaticProvider {
                items: vec![
                    PmItem {
                        id: "lin-1".to_string(),
                        name: "Existing task".to_string(),
                        description: "Remote body should not replace local.".to_string(),
                        rank: 0,
                        completed: false,
                    },
                    PmItem {
                        id: "lin-2".to_string(),
                        name: "Second task".to_string(),
                        description: "Imported from remote.".to_string(),
                        rank: 1,
                        completed: false,
                    },
                ],
            }),
            provider: PmProviderKind::Linear,
            project: "proj-1".to_string(),
        };

        let result = bootstrap_read_write_provider(&args, &mut local_items, ctx)
            .await
            .expect("bootstrap succeeds");

        assert_eq!(result.created_local, vec!["02-second-task.md"]);

        let existing = std::fs::read_to_string(wave_dir.join("01-existing.md")).expect("read");
        assert!(existing.contains("linear_id: lin-1"));
        assert!(existing.contains("Local body stays put."));
        assert!(!existing.contains("Remote body should not replace local."));

        let imported =
            std::fs::read_to_string(wave_dir.join("02-second-task.md")).expect("read imported");
        assert!(imported.contains("linear_id: lin-2"));
        assert!(imported.contains("# Second task"));
        assert!(imported.contains("Imported from remote."));
    }

    #[tokio::test]
    async fn bootstrap_read_write_provider_creates_linear_items_in_reverse_local_order() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("01-first.md"), "# First\n").expect("write first");
        std::fs::write(wave_dir.join("02-second.md"), "# Second\n").expect("write second");
        std::fs::write(wave_dir.join("03-third.md"), "# Third\n").expect("write third");

        let mut local_items = read_local_roadmap_items(&wave_dir).expect("read local items");
        let progress = crate::ops::NullProgress;
        let args = BootstrapArgs {
            repo: dir.path(),
            wave: "pm",
            wave_dir: &wave_dir,
            project_name: "PM",
            description: "",
            progress: &progress,
        };
        let created = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let provider = RecordingProvider {
            created: created.clone(),
            next_id: std::sync::Arc::new(std::sync::Mutex::new(0)),
        };
        let ctx = PmContext {
            client: Box::new(provider),
            provider: PmProviderKind::Linear,
            project: "proj-1".to_string(),
        };

        bootstrap_read_write_provider(&args, &mut local_items, ctx)
            .await
            .expect("bootstrap succeeds");

        let created = created.lock().expect("created lock").clone();
        assert_eq!(created, vec!["Third", "Second", "First"]);

        let created_order = std::fs::read_to_string(wave_dir.join("01-first.md")).expect("read");
        assert!(created_order.contains("linear_id: lin-3"));
        let created_order = std::fs::read_to_string(wave_dir.join("02-second.md")).expect("read");
        assert!(created_order.contains("linear_id: lin-2"));
        let created_order = std::fs::read_to_string(wave_dir.join("03-third.md")).expect("read");
        assert!(created_order.contains("linear_id: lin-1"));
    }

    #[tokio::test]
    async fn bootstrap_read_write_provider_creates_asana_items_in_reverse_local_order() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("01-first.md"), "# First\n").expect("write first");
        std::fs::write(wave_dir.join("02-second.md"), "# Second\n").expect("write second");
        std::fs::write(wave_dir.join("03-third.md"), "# Third\n").expect("write third");

        let mut local_items = read_local_roadmap_items(&wave_dir).expect("read local items");
        let progress = crate::ops::NullProgress;
        let args = BootstrapArgs {
            repo: dir.path(),
            wave: "pm",
            wave_dir: &wave_dir,
            project_name: "PM",
            description: "",
            progress: &progress,
        };
        let created = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let provider = RecordingProvider {
            created: created.clone(),
            next_id: std::sync::Arc::new(std::sync::Mutex::new(0)),
        };
        let ctx = PmContext {
            client: Box::new(provider),
            provider: PmProviderKind::Asana,
            project: "proj-1".to_string(),
        };

        bootstrap_read_write_provider(&args, &mut local_items, ctx)
            .await
            .expect("bootstrap succeeds");

        let created = created.lock().expect("created lock").clone();
        assert_eq!(created, vec!["Third", "Second", "First"]);

        let created_order = std::fs::read_to_string(wave_dir.join("01-first.md")).expect("read");
        assert!(created_order.contains("asana_id: lin-3"));
        let created_order = std::fs::read_to_string(wave_dir.join("02-second.md")).expect("read");
        assert!(created_order.contains("asana_id: lin-2"));
        let created_order = std::fs::read_to_string(wave_dir.join("03-third.md")).expect("read");
        assert!(created_order.contains("asana_id: lin-1"));
    }

    #[tokio::test]
    async fn pm_export_creates_updates_and_skips_without_recreating_missing_remote_items() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("01-new-item.md"),
            "# New item\n\nFresh local details.\n",
        )
        .expect("write new item");
        std::fs::write(
            wave_dir.join("02-unchanged.md"),
            "---\nlinear_id: lin-2\n---\n# Unchanged\n\nSame remote text.\n",
        )
        .expect("write unchanged item");
        std::fs::write(
            wave_dir.join("03-updated.md"),
            "---\nlinear_id: lin-3\n---\n# Updated title\n\nNew local text.\n",
        )
        .expect("write updated item");
        std::fs::write(
            wave_dir.join("04-missing.md"),
            "---\nlinear_id: lin-404\n---\n# Missing remote\n\nDo not recreate.\n",
        )
        .expect("write missing item");

        let items = std::sync::Arc::new(std::sync::Mutex::new(vec![
            PmItem {
                id: "lin-2".to_string(),
                name: "Unchanged".to_string(),
                description: "Same remote text.".to_string(),
                rank: 1,
                completed: false,
            },
            PmItem {
                id: "lin-3".to_string(),
                name: "Old title".to_string(),
                description: "Old remote text.".to_string(),
                rank: 2,
                completed: false,
            },
        ]));
        let ctx = PmContext {
            client: Box::new(MutableProvider {
                items: items.clone(),
                next_id: std::sync::Arc::new(std::sync::Mutex::new(99)),
            }),
            provider: PmProviderKind::Linear,
            project: "proj-1".to_string(),
        };

        let result = export_local_wave_to_remote(&wave_dir, &ctx, &crate::ops::NullProgress)
            .await
            .expect("export succeeds");

        assert_eq!(
            result,
            PmExportCounts {
                created: 1,
                updated: 1,
                skipped: 2,
            }
        );

        let new_file = std::fs::read_to_string(wave_dir.join("01-new-item.md")).expect("read");
        assert!(new_file.contains("linear_id: lin-100"));

        let items = items.lock().expect("items lock").clone();
        assert_eq!(items.len(), 3);
        assert!(items.iter().any(|item| {
            item.id == "lin-100"
                && item.name == "New item"
                && item.description == "Fresh local details."
                && item.rank == 0
        }));
        assert!(items.iter().any(|item| {
            item.id == "lin-3"
                && item.name == "Updated title"
                && item.description == "New local text."
        }));
        assert!(!items.iter().any(|item| item.id == "lin-404"));
    }

    #[test]
    fn build_text_update_only_includes_changed_text_fields() {
        let remote = PmItem {
            id: "lin-1".to_string(),
            name: "Existing".to_string(),
            description: "Remote body.".to_string(),
            rank: 0,
            completed: false,
        };

        let unchanged = build_text_update("Existing", "Remote body.", &remote);
        assert_eq!(unchanged.name, None);
        assert_eq!(unchanged.description, None);

        let changed = build_text_update("Renamed", "Local body.", &remote);
        assert_eq!(changed.name.as_deref(), Some("Renamed"));
        assert_eq!(changed.description.as_deref(), Some("Local body."));
        assert_eq!(changed.rank, None);
    }

    #[test]
    fn overwrite_local_wave_from_remote_rewrites_local_files_in_remote_order() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("01-local-first.md"), "# Local first\n").expect("write");
        std::fs::write(wave_dir.join("02-local-second.md"), "# Local second\n").expect("write");

        let remote_items = vec![
            PmItem {
                id: "lin-2".to_string(),
                name: "Remote second".to_string(),
                description: "Pulled from PM.".to_string(),
                rank: 1,
                completed: false,
            },
            PmItem {
                id: "lin-1".to_string(),
                name: "Remote first".to_string(),
                description: "Higher priority.".to_string(),
                rank: 0,
                completed: false,
            },
        ];

        let removed =
            overwrite_local_wave_from_remote(&wave_dir, &remote_items, PmProviderKind::Linear)
                .expect("pull should rewrite local files");

        assert_eq!(removed, 2);
        assert!(!wave_dir.join("01-local-first.md").exists());
        assert!(!wave_dir.join("02-local-second.md").exists());

        let first = std::fs::read_to_string(wave_dir.join("01-remote-second.md")).expect("read");
        assert!(first.contains("linear_id: lin-2"));
        assert!(first.contains("# Remote second"));

        let second = std::fs::read_to_string(wave_dir.join("02-remote-first.md")).expect("read");
        assert!(second.contains("linear_id: lin-1"));
        assert!(second.contains("# Remote first"));
    }
}
