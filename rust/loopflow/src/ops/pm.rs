use std::collections::HashMap;
use std::path::Path;

use crate::engine::config::load_config_or_default;
use crate::engine::git::{get_default_branch, list_tree, show_file};
use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::lfd::pm::asana::AsanaClient;
use crate::lfd::pm::linear::LinearClient;
use crate::lfd::pm::{
    PmError, PmItem, PmItemCreate, PmItemUpdate, PmProvider, PmProviderKind, RoadmapItemDocument,
    RoadmapItemFrontmatter,
};
use crate::lfd::store::open_store;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::ingest::{list_numbered_items, WaveItem};
use crate::ops::progress::Progress;

// ── Options and results ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PmImportOptions {
    pub wave: String,
}

#[derive(Debug, Clone)]
pub struct PmImportResult {
    pub wave: String,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub deleted: Vec<String>,
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

// ── Provider construction ───────────────────────────────────────────

/// Resolved PM context for a wave: the provider client, provider kind, and project ID (if any).
pub(crate) struct PmContext {
    pub client: Box<dyn PmProvider>,
    pub provider: PmProviderKind,
    pub project: String,
}

pub(crate) async fn build_provider(repo: &Path, wave: &str) -> OpsResult<PmContext> {
    let config = load_config_or_default(Some(repo));
    let wave_config = read_wave_config(repo, wave);
    let wave_pm = wave_config.as_ref().and_then(|wc| wc.pm.as_ref());

    // Provider comes from repo-level config
    let provider = config.pm.as_ref().map(|pm| pm.provider).ok_or_else(|| {
        OpsError::Message(
            "No PM provider configured. Set `pm.provider` in config.yaml.".to_string(),
        )
    })?;

    // Project ID comes from wave config, keyed by provider
    let project = wave_pm
        .and_then(|pm| pm.project_for(provider))
        .unwrap_or("")
        .to_string();

    let client: Box<dyn PmProvider> = match provider {
        PmProviderKind::Asana => {
            let token = resolve_provider_token("asana", "ASANA_ACCESS_TOKEN").await?;
            Box::new(AsanaClient::new(token, config.asana.clone()))
        }
        PmProviderKind::Linear => {
            let token = resolve_provider_token("linear", "LINEAR_API_KEY").await?;
            let team_id = config.linear.team.clone();
            Box::new(LinearClient::new(token, team_id))
        }
    };

    Ok(PmContext {
        client,
        provider,
        project,
    })
}

async fn resolve_provider_token(provider: &str, env_name: &str) -> OpsResult<String> {
    if let Ok(token) = std::env::var(env_name) {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
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
                "No {provider} credential found. Run `lf ops auth configure {provider}`."
            ))
        })?;

    if token
        .expires_at
        .is_some_and(|expires_at| expires_at <= time::OffsetDateTime::now_utc().unix_timestamp())
    {
        return Err(OpsError::Message(format!(
            "Stored {provider} token has expired. Run `lf ops auth {provider}` again."
        )));
    }

    Ok(token.access_token)
}

fn storage_config_from_env() -> OpsResult<crate::lfd::store::StorageConfig> {
    crate::lfd::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve lfd credential store: {err}")))
}

// ── Import ──────────────────────────────────────────────────────────

pub fn pm_import(
    repo: &Path,
    options: &PmImportOptions,
    progress: &impl Progress,
) -> OpsResult<PmImportResult> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to create async runtime: {err}")))?;
    rt.block_on(pm_import_async(repo, options, progress))
}

async fn pm_import_async(
    repo: &Path,
    options: &PmImportOptions,
    progress: &impl Progress,
) -> OpsResult<PmImportResult> {
    let wave_dir = repo.join("wave").join(&options.wave);
    if !wave_dir.is_dir() {
        std::fs::create_dir_all(&wave_dir)?;
    }

    let ctx = build_provider(repo, &options.wave).await?;
    progress.status(&format!(
        "importing from {:?} project {}",
        ctx.provider, ctx.project
    ));

    let remote_items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;

    let provider_kind = ctx.provider;

    // Index existing local files by provider-specific ID
    let local_items = list_numbered_items(&wave_dir).unwrap_or_default();
    let mut local_by_pm_id: HashMap<String, (WaveItem, RoadmapItemDocument)> = HashMap::new();
    for item in &local_items {
        let path = wave_dir.join(&item.filename);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(doc) = RoadmapItemDocument::parse(&content) {
                if let Some(pm_id) = doc.frontmatter.id_for(provider_kind) {
                    local_by_pm_id.insert(pm_id.to_string(), (item.clone(), doc));
                }
            }
        }
    }

    let mut created = Vec::new();
    let mut updated = Vec::new();

    // Track which pm_ids we've seen so we can detect deletions
    let mut seen_pm_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for remote in &remote_items {
        seen_pm_ids.insert(remote.id.clone());

        let remote_doc = remote_item_to_document(remote, provider_kind);

        if let Some((existing_item, existing_doc)) = local_by_pm_id.get(&remote.id) {
            // Update existing file if content differs
            if existing_doc.body != remote_doc.body
                || extract_heading(&existing_doc.body) != extract_heading(&remote_doc.body)
            {
                let path = wave_dir.join(&existing_item.filename);
                let rendered = remote_doc.render().map_err(pm_to_ops)?;
                std::fs::write(&path, rendered)?;
                progress.status(&format!("  updated {}", existing_item.filename));
                updated.push(existing_item.filename.clone());
            }
        } else {
            // Create new file
            let filename = format!("{:02}-{}.md", remote.rank + 1, slugify(&remote.name));
            let path = wave_dir.join(&filename);
            let rendered = remote_doc.render().map_err(pm_to_ops)?;
            std::fs::write(&path, rendered)?;
            progress.status(&format!("  created {filename}"));
            created.push(filename);
        }
    }

    // Delete local files whose pm_id no longer exists remotely
    let mut deleted = Vec::new();
    for (pm_id, (item, _)) in &local_by_pm_id {
        if !seen_pm_ids.contains(pm_id) {
            let path = wave_dir.join(&item.filename);
            std::fs::remove_file(&path)?;
            progress.status(&format!("  deleted {}", item.filename));
            deleted.push(item.filename.clone());
        }
    }

    Ok(PmImportResult {
        wave: options.wave.clone(),
        created,
        updated,
        deleted,
    })
}

// ── Sync (three-way) ───────────────────────────────────────────────

pub fn pm_sync(
    repo: &Path,
    options: &PmSyncOptions,
    progress: &impl Progress,
) -> OpsResult<PmSyncResult> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to create async runtime: {err}")))?;
    rt.block_on(pm_sync_async(repo, options, progress))
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

    let ctx = build_provider(repo, &options.wave).await?;
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
                    remote_title != base_title || remote_description(remote_item) != base_doc.body;

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
                    let doc = remote_item_to_document(remote_item, provider_kind);
                    let path = wave_dir.join(local_file);
                    let rendered = doc.render().map_err(pm_to_ops)?;
                    std::fs::write(&path, rendered)?;
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
                let doc = remote_item_to_document(remote_item, provider_kind);
                let filename = format!(
                    "{:02}-{}.md",
                    remote_item.rank + 1,
                    slugify(&remote_item.name)
                );
                let rendered = doc.render().map_err(pm_to_ops)?;
                std::fs::write(wave_dir.join(&filename), rendered)?;
                progress.status(&format!("  pulled (new) {filename}"));
                pulled.push(filename);
            }

            // In base, deleted locally, unchanged remotely — archive remote
            (Some(base_doc), None, Some(remote_item)) => {
                let remote_changed = Some(remote_item.name.as_str())
                    != extract_heading(&base_doc.body)
                    || remote_description(remote_item) != base_doc.body;
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
    let items = list_numbered_items(wave_dir).unwrap_or_default();
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
    let heading = format!("# {}\n", item.name);
    let body = if item.description.is_empty() {
        heading
    } else {
        format!("{heading}\n{}\n", item.description.trim())
    };

    let mut frontmatter = RoadmapItemFrontmatter::default();
    frontmatter.set_id(provider, item.id.clone());

    RoadmapItemDocument { frontmatter, body }
}

fn remote_description(item: &PmItem) -> String {
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

fn pm_to_ops(err: PmError) -> OpsError {
    OpsError::Message(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_converts_name_to_filename_slug() {
        assert_eq!(slugify("Ship Linear Client"), "ship-linear-client");
        assert_eq!(slugify("01: Auth & Setup"), "01-auth-setup");
        assert_eq!(slugify("  spaces  "), "spaces");
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
}
