//! `lf op pm` — read and write a wave's roadmap directly in Asana.
//!
//! Asana is the single source of truth for a wave's roadmap. There is no local
//! mirror: every command here talks to the Asana project pinned by
//! `pm.asana_project` in `wave/<name>/GOAL.md`.

use std::future::Future;
use std::path::Path;

use crate::engine::config::load_config_or_default;
use crate::lfd::http::routes::wave_config::{read_wave_config, update_wave_goal_config};
use crate::lfd::pm::asana::AsanaClient;
use crate::lfd::pm::{PmError, PmItem, PmItemCreate, PmItemUpdate};
use crate::lfd::provider_auth::{refresh_pm_oauth_token, Provider};
use crate::lfd::store::open_store;
use crate::ops::error::{OpsError, OpsResult};
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmUpdateResult {
    pub wave: String,
    pub id: String,
    pub created: bool,
    pub completed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PmStatusOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmWaveStatus {
    pub wave: String,
    pub project: String,
    pub open: usize,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmStatusResult {
    pub waves: Vec<PmWaveStatus>,
}

// ── Client + project resolution ─────────────────────────────────────

/// A wave's Asana client bound to its roadmap project.
pub(crate) struct PmContext {
    pub client: AsanaClient,
    pub project: String,
}

/// The `pm.asana_project` handle from `wave/<name>/GOAL.md`, if set.
fn read_asana_project(repo: &Path, wave: &str) -> Option<String> {
    read_wave_config(repo, wave)
        .and_then(|config| config.pm)
        .and_then(|pm| pm.asana_project)
        .filter(|project| !project.trim().is_empty())
}

async fn build_asana_client(repo: &Path) -> OpsResult<AsanaClient> {
    let config = load_config_or_default(Some(repo));
    let token = resolve_asana_token().await?;
    Ok(AsanaClient::new(token, config.asana.clone()))
}

async fn resolve_context(repo: &Path, wave: &str) -> OpsResult<PmContext> {
    let project = read_asana_project(repo, wave).ok_or_else(|| {
        OpsError::Message(format!(
            "wave/{wave}/GOAL.md has no `pm.asana_project`. \
             Run `lf op pm init --wave {wave}` to connect its roadmap."
        ))
    })?;
    let client = build_asana_client(repo).await?;
    Ok(PmContext { client, project })
}

/// Resolve the stored Asana OAuth access token for embedding in cloud scaffolds
/// (`lf op cloud`), refreshing it in place if it has expired.
pub(crate) fn asana_access_token() -> OpsResult<String> {
    block_on_pm(resolve_asana_token())
}

async fn resolve_asana_token() -> OpsResult<String> {
    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open lfd credential store: {err}")))?;
    let token = store
        .get_provider_token("asana")
        .await
        .map_err(|err| OpsError::Message(format!("failed to load asana token: {err}")))?
        .ok_or_else(|| {
            OpsError::Message("No asana credential found. Run `lf op auth asana`.".to_string())
        })?;

    let expired = token
        .expires_at
        .is_some_and(|expires_at| expires_at <= time::OffsetDateTime::now_utc().unix_timestamp());
    if !expired {
        return Ok(token.access_token);
    }

    // The stored token is expired but may be refreshable: if it carries a refresh
    // token and the OAuth client creds are in the env, refresh in place rather than
    // forcing the user to re-authenticate.
    let refresh_token = token
        .refresh_token
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    if let Some(refresh_token) = refresh_token {
        if let Ok(mut refreshed) = refresh_pm_oauth_token(Provider::Asana, refresh_token).await {
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
                    OpsError::Message(format!("failed to persist refreshed asana token: {err}"))
                })?;
            return Ok(access_token);
        }
    }

    Err(OpsError::Message(
        "Stored asana token has expired. Run `lf op auth asana` again.".to_string(),
    ))
}

fn storage_config_from_env() -> OpsResult<crate::lfd::store::StorageConfig> {
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

    if let Some(existing) = read_asana_project(repo, &wave) {
        progress.status(&format!(
            "wave/{wave} already linked to asana project {existing}"
        ));
        return Ok(PmInitResult {
            wave,
            project_id: existing,
            created: false,
        });
    }

    let client = build_asana_client(repo).await?;
    progress.status(&format!("creating asana project for wave/{wave}"));
    let project_id = client
        .create_project(&title_case(&wave), "")
        .await
        .map_err(pm_to_ops)?;
    write_asana_project_to_goal(repo, &wave, &project_id)?;

    let _ = crate::ops::commit_workflow(
        repo,
        &crate::ops::CommitOptions {
            add: true,
            message: Some(format!("lf pm: connect {wave} to asana")),
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
        "fetching asana project {} for wave/{wave}",
        ctx.project
    ));
    let items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;
    Ok(PmShowResult {
        wave: wave.to_string(),
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

async fn apply_update(
    wave: &str,
    ctx: &PmContext,
    options: &PmUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<PmUpdateResult> {
    let mark_done = parse_done_status(options.status.as_deref())?;

    let (id, created) = match options.id.as_ref() {
        Some(id) => {
            progress.status(&format!("updating asana task {id}"));
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
                "creating asana task on project {} for wave/{wave}",
                ctx.project
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

    if mark_done {
        ctx.client.complete_item(&id).await.map_err(pm_to_ops)?;
    }

    Ok(PmUpdateResult {
        wave: wave.to_string(),
        id,
        created,
        completed: mark_done,
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
        let Some(project) = read_asana_project(repo, &wave) else {
            continue;
        };
        let client = build_asana_client(repo).await?;
        progress.status(&format!("checking asana for wave/{wave}"));
        let items = client.list_items(&project).await.map_err(pm_to_ops)?;
        let total = items.len();
        let open = items.iter().filter(|item| !item.completed).count();
        results.push(PmWaveStatus {
            wave,
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
        if read_asana_project(repo, &name).is_some() {
            waves.push(name);
        }
    }
    waves.sort();
    Ok(waves)
}

// ── helpers ─────────────────────────────────────────────────────────

fn write_asana_project_to_goal(repo: &Path, wave: &str, project_id: &str) -> OpsResult<()> {
    update_wave_goal_config(repo, wave, |map| {
        let pm_key = serde_yaml_ng::Value::String("pm".to_string());
        let mut pm_map = map
            .get(&pm_key)
            .and_then(serde_yaml_ng::Value::as_mapping)
            .cloned()
            .unwrap_or_default();
        pm_map.insert(
            serde_yaml_ng::Value::String("asana_project".to_string()),
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
    use crate::engine::config::AsanaConfig;
    use crate::lfd::pm::test_server::{self, json_response};
    use crate::ops::NullProgress;
    use axum::http::StatusCode;
    use serde_json::json;

    fn test_ctx(base_url: String, project: &str) -> PmContext {
        PmContext {
            client: AsanaClient::with_base_url(
                "test-token".to_string(),
                AsanaConfig::default(),
                base_url,
            ),
            project: project.to_string(),
        }
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
            assignee: Some("me".to_string()),
        });
        assert!(line.starts_with("open"));
        assert!(line.contains("Ship it"));
        assert!(line.contains("assignee:me"));
        assert!(line.contains("id:123"));
    }

    #[tokio::test]
    async fn fetch_items_reads_the_project_roadmap() {
        let (base_url, _requests) = test_server::spawn(vec![json_response(
            StatusCode::OK,
            json!({ "data": [
                { "gid": "1", "name": "First", "completed": false },
                { "gid": "2", "name": "Second", "completed": true }
            ] }),
        )])
        .await;
        let ctx = test_ctx(base_url, "proj-1");

        let result = fetch_items("goals", &ctx, &NullProgress)
            .await
            .expect("fetch succeeds");
        assert_eq!(result.project, "proj-1");
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].name, "First");
    }

    #[tokio::test]
    async fn apply_update_creates_when_no_id_is_given() {
        // create_item first resolves the project priority field, then POSTs the task.
        let (base_url, requests) = test_server::spawn(vec![
            json_response(
                StatusCode::OK,
                json!({ "data": [{
                    "custom_field": {
                        "gid": "field-1",
                        "name": "Priority",
                        "resource_subtype": "enum",
                        "enum_options": [
                            { "gid": "opt-urgent", "name": "Urgent" },
                            { "gid": "opt-high", "name": "High" },
                            { "gid": "opt-medium", "name": "Medium" },
                            { "gid": "opt-low", "name": "Low" }
                        ]
                    }
                }] }),
            ),
            json_response(
                StatusCode::CREATED,
                json!({ "data": { "gid": "new-task" } }),
            ),
        ])
        .await;
        let ctx = test_ctx(base_url, "proj-1");
        let options = PmUpdateOptions {
            wave: None,
            id: None,
            title: "New task".to_string(),
            notes: Some("details".to_string()),
            status: None,
        };

        let result = apply_update("goals", &ctx, &options, &NullProgress)
            .await
            .expect("update succeeds");
        assert!(result.created);
        assert_eq!(result.id, "new-task");
        assert!(!result.completed);
        assert_eq!(requests.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn apply_update_completes_when_status_done() {
        let (base_url, _requests) = test_server::spawn(vec![
            // update_item PUT
            json_response(StatusCode::OK, json!({ "data": {} })),
            // complete_item PUT
            json_response(StatusCode::OK, json!({ "data": {} })),
        ])
        .await;
        let ctx = test_ctx(base_url, "proj-1");
        let options = PmUpdateOptions {
            wave: None,
            id: Some("task-9".to_string()),
            title: "Existing".to_string(),
            notes: None,
            status: Some("done".to_string()),
        };

        let result = apply_update("goals", &ctx, &options, &NullProgress)
            .await
            .expect("update succeeds");
        assert!(!result.created);
        assert_eq!(result.id, "task-9");
        assert!(result.completed);
    }
}
