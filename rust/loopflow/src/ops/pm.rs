use std::future::Future;
use std::path::Path;

use crate::engine::config::load_config_or_default;
use crate::lfd::http::routes::wave_config::{
    read_wave_config, update_wave_goal_config, WavePmConfig,
};
use crate::lfd::pm::asana::AsanaClient;
use crate::lfd::pm::linear::LinearClient;
use crate::lfd::pm::notion::NotionClient;
use crate::lfd::pm::{PmError, PmProvider, PmProviderKind};
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
    pub provider: PmProviderKind,
    pub project_id: String,
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

pub(crate) fn resolve_provider(repo: &Path, wave: &str) -> OpsResult<PmProviderKind> {
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

pub(crate) async fn build_client(
    repo: &Path,
    provider: PmProviderKind,
) -> OpsResult<Box<dyn PmProvider>> {
    let config = load_config_or_default(Some(repo));
    let client: Box<dyn PmProvider> = match provider {
        PmProviderKind::Asana => {
            let token = resolve_provider_token("asana").await?;
            Box::new(AsanaClient::new(token, config.asana.clone()))
        }
        PmProviderKind::Linear => {
            let token = resolve_provider_token("linear").await?;
            Box::new(LinearClient::new(token, config.linear.team.clone()))
        }
        PmProviderKind::Notion => {
            let token = resolve_provider_token("notion").await?;
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

async fn resolve_provider_token(provider: &str) -> OpsResult<String> {
    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open lfd credential store: {err}")))?;
    let token = store
        .get_provider_token(provider)
        .await
        .map_err(|err| OpsError::Message(format!("failed to load {provider} token: {err}")))?
        .ok_or_else(|| {
            OpsError::Message(format!(
                "No {provider} credential found. Run `lf op auth {provider}`."
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

fn update_wave_pm_goal(
    repo: &Path,
    wave: &str,
    update: impl FnOnce(&mut serde_yaml_ng::Mapping) -> OpsResult<()>,
) -> OpsResult<()> {
    update_wave_goal_config(repo, wave, |map| {
        let pm_key = yaml_string("pm");
        let mut pm_map = map
            .get(&pm_key)
            .and_then(serde_yaml_ng::Value::as_mapping)
            .cloned()
            .unwrap_or_default();

        update(&mut pm_map).map_err(|err| err.to_string())?;
        map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));
        Ok(())
    })
    .map_err(OpsError::Message)
}

pub(crate) fn write_pm_provider_to_wave_goal(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
) -> OpsResult<()> {
    update_wave_pm_goal(repo, wave, |pm_map| {
        pm_map.insert(
            yaml_string("provider"),
            serde_yaml_ng::to_value(provider)
                .map_err(|err| OpsError::Message(format!("failed to encode pm provider: {err}")))?,
        );
        Ok(())
    })
}

pub(crate) fn write_pm_project_to_wave_goal(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
    project_id: &str,
) -> OpsResult<()> {
    update_wave_pm_goal(repo, wave, |pm_map| {
        pm_map.insert(yaml_string(project_key(provider)), yaml_string(project_id));
        Ok(())
    })
}

// ── Bootstrap ───────────────────────────────────────────────────────

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
    write_pm_project_to_wave_goal(repo, wave, ctx.provider, &project_id)?;
    Ok(project_id)
}

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
    write_pm_provider_to_wave_goal(repo, &wave, provider_kind)?;

    let (project_name, description) = read_wave_project_metadata(repo, &wave)?;
    let ctx = build_provider(repo, &wave, provider_kind).await?;
    let project_id =
        ensure_project(repo, &wave, &project_name, &description, &ctx, progress).await?;
    ctx.client
        .init_project(&project_id)
        .await
        .map_err(pm_to_ops)?;

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
        provider: provider_kind,
        project_id,
    })
}

pub(crate) fn block_on_pm<T>(future: impl Future<Output = OpsResult<T>>) -> OpsResult<T> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to create async runtime: {err}")))?;
    rt.block_on(future)
}

pub(crate) fn pm_to_ops(err: PmError) -> OpsError {
    OpsError::Message(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
    fn write_pm_provider_to_wave_goal_sets_provider_field() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("pm");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("goal.md"),
            "---\nprimary_flow: build\n---\nDrive the work.\n",
        )
        .expect("write wave goal");

        write_pm_provider_to_wave_goal(dir.path(), "pm", PmProviderKind::Linear)
            .expect("write pm provider");

        let config = read_wave_config(dir.path(), "pm").expect("read wave config");
        assert_eq!(
            config.pm.and_then(|pm| pm.provider),
            Some(PmProviderKind::Linear)
        );
        let content = std::fs::read_to_string(wave_dir.join("goal.md")).expect("read wave goal");
        assert!(content.ends_with("Drive the work.\n"));
    }
}
