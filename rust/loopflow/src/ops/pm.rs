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
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProvider, PmProviderKind, PriorityBucket,
    RoadmapItemDocument, RoadmapItemFrontmatter,
};
use crate::lfd::store::open_store;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::ingest::{list_wave_items, parse_wave_item_filename, WaveItem};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmInitResult {
    pub waves: Vec<PmInitWaveResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmInitWaveResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub project_id: String,
    pub items: usize,
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
    build_client_with_team(repo, provider, None).await
}

async fn build_client_with_team(
    repo: &Path,
    provider: PmProviderKind,
    team_id: Option<String>,
) -> OpsResult<Box<dyn PmProvider>> {
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

#[allow(dead_code)] // Used by upcoming PM CLI commands
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

fn local_item_priority(item: &LocalRoadmapItem) -> PriorityBucket {
    item.item.priority_bucket().unwrap_or(PriorityBucket::High)
}

fn local_item_description(item: &LocalRoadmapItem) -> String {
    body_without_heading(&item.doc.body).trim().to_string()
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

pub fn pm_init(repo: &Path, progress: &impl Progress) -> OpsResult<PmInitResult> {
    block_on_pm(pm_init_async(repo, progress))
}

async fn pm_init_async(repo: &Path, progress: &impl Progress) -> OpsResult<PmInitResult> {
    let wave_root = repo.join("wave");
    if !wave_root.is_dir() {
        return Err(OpsError::Message("no wave/ directory found".to_string()));
    }

    let mut waves: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&wave_root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                waves.push(name.to_string());
            }
        }
    }
    waves.sort();

    if waves.is_empty() {
        return Err(OpsError::Message("no waves found in wave/".to_string()));
    }

    // Resolve provider from first wave (all waves use the same provider for init).
    let provider_kind = resolve_provider(repo, &waves[0])?;
    let config = load_config_or_default(Some(repo));
    let client = build_client(repo, provider_kind).await?;

    let team_id = resolve_project_parent_id(&*client, provider_kind, &config, progress)
        .await
        .map_err(pm_to_ops)?;

    // Clear stale provider IDs from wave item frontmatter before creating fresh projects.
    for wave in &waves {
        let wave_dir = wave_root.join(wave);
        clear_provider_ids(&wave_dir, provider_kind)?;
    }

    let mut results = Vec::new();
    for wave in &waves {
        let wave_dir = wave_root.join(wave);
        write_pm_provider_to_wave_yaml(repo, wave, provider_kind)?;

        let (project_name, description) = read_wave_project_metadata(repo, wave)?;
        let mut local_items = read_local_roadmap_items(&wave_dir)?;

        progress.status(&format!(
            "creating {:?} project for wave/{wave}",
            provider_kind
        ));
        let project_id = client
            .create_project_in_team(&team_id, &project_name, &description)
            .await
            .map_err(pm_to_ops)?;
        write_pm_project_to_wave_yaml(repo, wave, provider_kind, &project_id)?;

        let ctx = PmContext {
            client: build_client_with_team(repo, provider_kind, Some(team_id.clone())).await?,
            provider: provider_kind,
            project: project_id.clone(),
        };

        // Create all local items on the remote (reverse order to preserve sort).
        let item_count = local_items.len();
        for local_item in local_items.iter_mut().rev() {
            let title = local_item_title(local_item);
            create_remote_for_local_item(&wave_dir, local_item, &ctx, &project_id, &title).await?;
        }

        results.push(PmInitWaveResult {
            wave: wave.clone(),
            provider: provider_kind,
            project_id,
            items: item_count,
        });
    }

    let _ = crate::ops::commit_workflow(
        repo,
        &crate::ops::CommitOptions {
            add: true,
            message: Some("lf pm: init".to_string()),
            ..crate::ops::CommitOptions::for_task("pm")
        },
        progress,
    )?;

    Ok(PmInitResult { waves: results })
}

const DEFAULT_TEAM_NAME: &str = "Waves";

async fn resolve_project_parent_id(
    client: &dyn PmProvider,
    provider: PmProviderKind,
    config: &Config,
    progress: &impl Progress,
) -> crate::lfd::pm::PmResult<String> {
    if provider == PmProviderKind::Notion {
        if let Some(parent_page) = config.notion.parent_page.as_deref() {
            return Ok(parent_page.to_string());
        }
    }

    progress.status("creating PM team");
    create_fresh_team(client).await
}

async fn create_fresh_team(client: &dyn PmProvider) -> crate::lfd::pm::PmResult<String> {
    let existing = client.find_team(DEFAULT_TEAM_NAME).await?;
    if existing.is_some() {
        let now = time::OffsetDateTime::now_utc();
        let timestamp = format!(
            "{:04}-{:02}-{:02}",
            now.year(),
            now.month() as u8,
            now.day()
        );
        let name = format!("{DEFAULT_TEAM_NAME} {timestamp}");
        client.create_team(&name).await
    } else {
        client.create_team(DEFAULT_TEAM_NAME).await
    }
}

fn clear_provider_ids(wave_dir: &Path, provider: PmProviderKind) -> OpsResult<()> {
    for item in list_wave_items(wave_dir)? {
        let path = wave_dir.join(&item.filename);
        let content = std::fs::read_to_string(&path)?;
        let mut doc = RoadmapItemDocument::parse(&content).map_err(pm_to_ops)?;
        let had_id = doc.frontmatter.id_for(provider).is_some();
        if had_id {
            doc.frontmatter.clear_id(provider);
            let rendered = doc.render().map_err(pm_to_ops)?;
            std::fs::write(&path, rendered)?;
        }
    }
    Ok(())
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

    let filename = next_remote_filename(
        wave_dir,
        remote_item.priority,
        &remote_item.name,
        Some(local_item.item.filename.as_str()),
    );
    if filename != local_item.item.filename {
        let old_path = wave_dir.join(&local_item.item.filename);
        std::fs::remove_file(&old_path)?;
        local_item.item =
            parse_wave_item_filename(&filename).expect("generated roadmap filename should parse");
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
                description: local_item_description(local_item),
                priority: local_item_priority(local_item),
            },
        )
        .await
        .map_err(pm_to_ops)?;
    local_item.doc.frontmatter.set_id(ctx.provider, pm_id);
    write_local_item(wave_dir, local_item)
}

fn next_remote_filename(
    wave_dir: &Path,
    priority: PriorityBucket,
    title: &str,
    current_filename: Option<&str>,
) -> String {
    next_item_filename(priority, title, |filename| {
        current_filename.is_some_and(|current| current == filename)
            || !wave_dir.join(filename).exists()
    })
}

fn next_item_filename(
    priority: PriorityBucket,
    title: &str,
    mut is_available: impl FnMut(&str) -> bool,
) -> String {
    let slug = slugify(title);
    let prefix = priority.filename_prefix();
    let filename = format!("{prefix}-{slug}.md");
    if is_available(&filename) {
        return filename;
    }

    let mut suffix = 2;
    loop {
        let filename = format!("{prefix}-{slug}-{suffix}.md");
        if is_available(&filename) {
            return filename;
        }
        suffix += 1;
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
    for remote_item in remote_items {
        let filename = next_item_filename(remote_item.priority, &remote_item.name, |filename| {
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
            let filename =
                next_remote_filename(&wave_dir, remote_item.priority, &remote_item.name, None);
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
            (Some(base_item), Some(local_item), Some(remote_item)) => {
                let base_title = extract_heading(&base_item.doc.body);
                let local_title = extract_heading(&local_item.doc.body);
                let remote_title = Some(remote_item.name.as_str());

                let local_changed = local_title != base_title
                    || local_item.doc.body != base_item.doc.body
                    || !base_and_local_priorities_match(base_item, local_item);
                let remote_changed = remote_title != base_title
                    || render_remote_body(remote_item) != base_item.doc.body
                    || !base_and_remote_priorities_match(base_item, remote_item);

                if local_changed && remote_changed {
                    progress.status(&format!("  conflict: {}", local_item.item.filename));
                    conflicts.push(local_item.item.filename.clone());
                } else if local_changed {
                    // Push local → remote
                    let name = local_title.unwrap_or(&remote_item.name);
                    ctx.client
                        .update_item(
                            pm_id,
                            &PmItemUpdate {
                                name: Some(name.to_string()),
                                description: Some(local_item_description(local_item)),
                                priority: Some(local_item_priority(local_item)),
                            },
                        )
                        .await
                        .map_err(pm_to_ops)?;
                    progress.status(&format!("  pushed {}", local_item.item.filename));
                    pushed.push(local_item.item.filename.clone());
                } else if remote_changed {
                    // Pull remote → local
                    let mut updated_local = local_item.clone();
                    apply_remote_match(
                        &wave_dir,
                        &mut updated_local,
                        provider_kind,
                        remote_item,
                        true,
                    )?;
                    progress.status(&format!("  pulled {}", updated_local.item.filename));
                    pulled.push(updated_local.item.filename.clone());
                }
                // else: no-op
            }

            // Not in base, exists locally — new local item, push
            (None, Some(local_item), None) => {
                let name = extract_heading(&local_item.doc.body).unwrap_or("untitled");
                let new_id = ctx
                    .client
                    .create_item(
                        &ctx.project,
                        &PmItemCreate {
                            name: name.to_string(),
                            description: local_item_description(local_item),
                            priority: local_item_priority(local_item),
                        },
                    )
                    .await
                    .map_err(pm_to_ops)?;

                // Write pm_id back to file
                let mut updated_doc = local_item.doc.clone();
                updated_doc.frontmatter.set_id(provider_kind, new_id);
                let rendered = updated_doc.render().map_err(pm_to_ops)?;
                std::fs::write(wave_dir.join(&local_item.item.filename), rendered)?;
                progress.status(&format!("  pushed (new) {}", local_item.item.filename));
                pushed.push(local_item.item.filename.clone());
            }

            // Not in base, exists remotely — new remote item, pull
            (None, None, Some(remote_item)) => {
                let filename =
                    next_remote_filename(&wave_dir, remote_item.priority, &remote_item.name, None);
                write_remote_item(&wave_dir.join(&filename), remote_item, provider_kind)?;
                progress.status(&format!("  pulled (new) {filename}"));
                pulled.push(filename);
            }

            // In base, deleted locally, unchanged remotely — archive remote
            (Some(base_item), None, Some(remote_item)) => {
                let remote_changed = Some(remote_item.name.as_str())
                    != extract_heading(&base_item.doc.body)
                    || render_remote_body(remote_item) != base_item.doc.body
                    || !base_and_remote_priorities_match(base_item, remote_item);
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
            (Some(base_item), Some(local_item), None) => {
                let local_changed = local_item.doc.body != base_item.doc.body
                    || !base_and_local_priorities_match(base_item, local_item);
                if local_changed {
                    progress.status(&format!(
                        "  conflict (changed locally, deleted remotely): {}",
                        local_item.item.filename
                    ));
                    conflicts.push(local_item.item.filename.clone());
                } else {
                    std::fs::remove_file(wave_dir.join(&local_item.item.filename))?;
                    progress.status(&format!("  deleted {}", local_item.item.filename));
                    pulled.push(local_item.item.filename.clone());
                }
            }

            // Already exists in both local and remote but wasn't in base — both new
            (None, Some(local_item), Some(_)) => {
                progress.status(&format!(
                    "  conflict (new on both sides): {}",
                    local_item.item.filename
                ));
                conflicts.push(local_item.item.filename.clone());
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
) -> OpsResult<HashMap<String, LocalRoadmapItem>> {
    let wave_path = format!("wave/{wave}");
    let files = list_tree(repo, main_branch, &wave_path)
        .map_err(|err| OpsError::Message(format!("failed to list base wave files: {err}")))?;

    let mut items = HashMap::new();
    for filename in files {
        if !filename.ends_with(".md") || filename.eq_ignore_ascii_case("README.md") {
            continue;
        }
        let Some(item) = parse_wave_item_filename(&filename) else {
            continue;
        };
        let file_path = format!("{wave_path}/{filename}");
        if let Some(content) = show_file(repo, main_branch, &file_path)
            .map_err(|err| OpsError::Message(format!("failed to read base file: {err}")))?
        {
            if let Some((pm_id, local_item)) = linked_local_item(item, &content, provider) {
                items.insert(pm_id, local_item);
            }
        }
    }
    Ok(items)
}

fn read_local_items(
    wave_dir: &Path,
    provider: PmProviderKind,
) -> OpsResult<HashMap<String, LocalRoadmapItem>> {
    let items = list_wave_items(wave_dir).unwrap_or_default();
    let mut result = HashMap::new();
    for item in items {
        let path = wave_dir.join(&item.filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some((pm_id, local_item)) = linked_local_item(item, &content, provider) {
                result.insert(pm_id, local_item);
            }
        }
    }
    Ok(result)
}

fn linked_local_item(
    item: WaveItem,
    content: &str,
    provider: PmProviderKind,
) -> Option<(String, LocalRoadmapItem)> {
    let doc = RoadmapItemDocument::parse(content).ok()?;
    let pm_id = doc.frontmatter.id_for(provider)?.to_string();
    Some((pm_id, LocalRoadmapItem { item, doc }))
}

fn remote_item_to_document(item: &PmItem, provider: PmProviderKind) -> RoadmapItemDocument {
    let mut frontmatter = RoadmapItemFrontmatter::default();
    frontmatter.set_id(provider, item.id.clone());

    RoadmapItemDocument {
        frontmatter,
        body: render_remote_body(item),
    }
}

fn write_remote_item(path: &Path, item: &PmItem, provider: PmProviderKind) -> OpsResult<()> {
    let rendered = remote_item_to_document(item, provider)
        .render()
        .map_err(pm_to_ops)?;
    std::fs::write(path, rendered)?;
    Ok(())
}

fn base_and_local_priorities_match(base: &LocalRoadmapItem, local: &LocalRoadmapItem) -> bool {
    local_item_priority(base) == local_item_priority(local)
}

fn base_and_remote_priorities_match(base: &LocalRoadmapItem, remote: &PmItem) -> bool {
    local_item_priority(base) == remote.priority
}

fn render_remote_body(item: &PmItem) -> String {
    let heading = format!("# {}\n", item.name);
    if item.description.is_empty() {
        heading
    } else {
        format!("{heading}\n{}\n", item.description.trim())
    }
}

/// Extract the first `# ` heading, stripping any roadmap priority or legacy prefix.
pub(crate) fn extract_heading(body: &str) -> Option<&str> {
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                return Some(strip_roadmap_prefix(heading));
            }
        }
    }
    None
}

/// Strip a leading `NN:`/`NN-` numeric prefix from a heading.
fn strip_roadmap_prefix(heading: &str) -> &str {
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
    let mut offset = 0;
    for line in body.lines() {
        if line.starts_with("# ") {
            let after_heading = &body[offset + line.len()..];
            return after_heading.strip_prefix('\n').unwrap_or(after_heading);
        }
        if !line.trim().is_empty() {
            break;
        }
        offset += line.len() + 1;
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use tempfile::TempDir;

    use crate::engine::config::NotionConfig;
    use crate::lfd::pm::PmResult;
    use crate::ops::progress::NullProgress;

    #[derive(Debug)]
    struct FakePmProvider {
        create_team_calls: AtomicUsize,
        existing_team_id: Option<String>,
        created_team_id: String,
    }

    impl FakePmProvider {
        fn new(existing_team_id: Option<&str>, created_team_id: &str) -> Self {
            Self {
                create_team_calls: AtomicUsize::new(0),
                existing_team_id: existing_team_id.map(str::to_string),
                created_team_id: created_team_id.to_string(),
            }
        }
    }

    #[async_trait]
    impl PmProvider for FakePmProvider {
        async fn create_team(&self, _name: &str) -> PmResult<String> {
            self.create_team_calls.fetch_add(1, Ordering::Relaxed);
            Ok(self.created_team_id.clone())
        }

        async fn find_team(&self, _name: &str) -> PmResult<Option<String>> {
            Ok(self.existing_team_id.clone())
        }

        async fn create_project(&self, _name: &str, _description: &str) -> PmResult<String> {
            panic!("unused in test")
        }

        async fn create_project_in_team(
            &self,
            _team_id: &str,
            _name: &str,
            _description: &str,
        ) -> PmResult<String> {
            panic!("unused in test")
        }

        async fn list_projects(&self, _team_id: &str) -> PmResult<Vec<crate::lfd::pm::PmProject>> {
            panic!("unused in test")
        }

        async fn list_items(&self, _project_id: &str) -> PmResult<Vec<PmItem>> {
            panic!("unused in test")
        }

        async fn create_item(&self, _project_id: &str, _item: &PmItemCreate) -> PmResult<String> {
            panic!("unused in test")
        }

        async fn update_item(&self, _item_id: &str, _update: &PmItemUpdate) -> PmResult<()> {
            panic!("unused in test")
        }

        async fn complete_item(&self, _item_id: &str) -> PmResult<()> {
            panic!("unused in test")
        }

        async fn comment(&self, _item_id: &str, _body: &str) -> PmResult<()> {
            panic!("unused in test")
        }
    }

    #[tokio::test]
    async fn resolve_project_parent_id_uses_configured_notion_parent_page() {
        let progress = NullProgress;
        let client = FakePmProvider::new(None, "created-team");
        let config = Config {
            notion: NotionConfig {
                parent_page: Some("page-123".to_string()),
                ..NotionConfig::default()
            },
            ..Config::default()
        };

        let team_id =
            resolve_project_parent_id(&client, PmProviderKind::Notion, &config, &progress)
                .await
                .expect("resolve notion parent");

        assert_eq!(team_id, "page-123");
        assert_eq!(client.create_team_calls.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn resolve_project_parent_id_creates_fresh_team_for_non_notion_providers() {
        let progress = NullProgress;
        let client = FakePmProvider::new(None, "created-team");
        let config = Config {
            notion: NotionConfig {
                parent_page: Some("page-123".to_string()),
                ..NotionConfig::default()
            },
            ..Config::default()
        };

        let team_id =
            resolve_project_parent_id(&client, PmProviderKind::Linear, &config, &progress)
                .await
                .expect("create fresh team");

        assert_eq!(team_id, "created-team");
        assert_eq!(client.create_team_calls.load(Ordering::Relaxed), 1);
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
            priority: PriorityBucket::High,
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
            priority: PriorityBucket::Low,
            completed: false,
        };

        let doc = remote_item_to_document(&item, PmProviderKind::Asana);
        assert_eq!(doc.body, "# Empty\n");
    }

    #[test]
    fn extract_heading_strips_roadmap_prefixes() {
        assert_eq!(
            extract_heading("# 03: Linear client\n\nSome description"),
            Some("Linear client")
        );
        assert_eq!(
            extract_heading("# 2: Priority client\n\nSome description"),
            Some("Priority client")
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
    fn body_without_heading_handles_leading_blank_lines() {
        assert_eq!(
            body_without_heading("\n# Title\n\nBody text."),
            "\nBody text."
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

    #[test]
    fn overwrite_local_wave_from_remote_rewrites_local_files_by_priority_bucket() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("2-local-first.md"), "# Local first\n").expect("write");
        std::fs::write(wave_dir.join("3-local-second.md"), "# Local second\n").expect("write");

        let remote_items = vec![
            PmItem {
                id: "lin-2".to_string(),
                name: "Remote second".to_string(),
                description: "Pulled from PM.".to_string(),
                priority: PriorityBucket::Medium,
                completed: false,
            },
            PmItem {
                id: "lin-1".to_string(),
                name: "Remote first".to_string(),
                description: "Higher priority.".to_string(),
                priority: PriorityBucket::Urgent,
                completed: false,
            },
        ];

        let removed =
            overwrite_local_wave_from_remote(&wave_dir, &remote_items, PmProviderKind::Linear)
                .expect("pull should rewrite local files");

        assert_eq!(removed, 2);
        assert!(!wave_dir.join("2-local-first.md").exists());
        assert!(!wave_dir.join("3-local-second.md").exists());

        let second = std::fs::read_to_string(wave_dir.join("1-remote-first.md")).expect("read");
        assert!(second.contains("linear_id: lin-1"));
        assert!(second.contains("# Remote first"));

        let first = std::fs::read_to_string(wave_dir.join("3-remote-second.md")).expect("read");
        assert!(first.contains("linear_id: lin-2"));
        assert!(first.contains("# Remote second"));
    }
}
