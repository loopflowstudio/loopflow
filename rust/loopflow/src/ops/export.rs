use std::path::Path;

use crate::engine::config::load_config_or_default;
use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::lfd::pm::asana::AsanaClient;
use crate::lfd::pm::{
    PmError, PmItemCreate, PmItemUpdate, PmProvider, PmProviderKind, RoadmapItemDocument,
};
use crate::lfd::store::open_store;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::ingest::{list_numbered_items, WaveItem};
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub wave: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct ExportResult {
    pub wave: String,
    pub created: Vec<String>,
    pub updated: Vec<String>,
}

pub fn export(
    repo: &Path,
    options: &ExportOptions,
    progress: &impl Progress,
) -> OpsResult<ExportResult> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to create async runtime: {err}")))?;
    rt.block_on(export_async(repo, options, progress))
}

async fn export_async(
    repo: &Path,
    options: &ExportOptions,
    progress: &impl Progress,
) -> OpsResult<ExportResult> {
    let wave_dir = repo.join("wave").join(&options.wave);
    if !wave_dir.is_dir() {
        return Err(OpsError::Message(format!(
            "wave directory not found: wave/{}/",
            options.wave
        )));
    }

    let config = load_config_or_default(Some(repo));

    // Read wave config to get PM project ID
    let wave_config = read_wave_config(repo, &options.wave);
    let pm_config = wave_config.as_ref().and_then(|wc| wc.pm.as_ref());

    let provider_kind = pm_config
        .map(|pm| pm.provider)
        .unwrap_or(PmProviderKind::Asana);

    if provider_kind != PmProviderKind::Asana {
        return Err(OpsError::Message(format!(
            "export currently supports asana only, got: {:?}",
            provider_kind
        )));
    }

    let token = resolve_asana_token().await?;
    let mut asana_config = config.asana.clone();
    if let Some(team) = pm_config
        .and_then(|pm| pm.team.as_ref())
        .filter(|team| !team.trim().is_empty())
    {
        asana_config.default_team = Some(team.clone());
    }
    let client = AsanaClient::new(token, asana_config);

    // Resolve or create project
    let (project_id, reverse_create_order) = match pm_config.map(|pm| pm.project.as_str()) {
        Some(id) if !id.is_empty() => {
            progress.status(&format!("exporting to asana project {id}"));
            (id.to_string(), false)
        }
        _ => {
            if options.dry_run {
                progress.status(&format!(
                    "dry-run: would create asana project \"{}\"",
                    options.wave
                ));
                ("(dry-run)".to_string(), false)
            } else {
                progress.status(&format!("creating asana project \"{}\"", options.wave));
                let id = client
                    .create_project(&options.wave, "")
                    .await
                    .map_err(pm_to_ops)?;
                write_pm_config_to_wave_yaml(repo, &options.wave, "asana", &id)?;
                progress.status(&format!("created asana project {id}"));
                (id, true)
            }
        }
    };

    // List roadmap items
    let items = list_numbered_items(&wave_dir)?;
    if items.is_empty() {
        progress.status(&format!("no numbered items in wave/{}/", options.wave));
        return Ok(ExportResult {
            wave: options.wave.clone(),
            created: vec![],
            updated: vec![],
        });
    }

    let mut created = Vec::new();
    let mut updated = Vec::new();

    for item in export_order(&items, reverse_create_order) {
        let path = wave_dir.join(&item.filename);
        let content = std::fs::read_to_string(&path)?;
        let mut doc = RoadmapItemDocument::parse(&content).map_err(pm_to_ops)?;

        let name = extract_heading(&doc.body).unwrap_or(&item.slug);

        if let Some(pm_id) = &doc.frontmatter.pm_id {
            // Update existing task
            if options.dry_run {
                progress.status(&format!(
                    "  dry-run: would update {} → {pm_id}",
                    item.filename
                ));
                updated.push(item.filename.clone());
                continue;
            }
            progress.status(&format!("  update {} → {pm_id}", item.filename));
            client
                .update_item(
                    pm_id,
                    &PmItemUpdate {
                        name: Some(name.to_string()),
                        description: Some(doc.body.clone()),
                        rank: None,
                    },
                )
                .await
                .map_err(pm_to_ops)?;
            updated.push(item.filename.clone());
        } else {
            // Create new task
            if options.dry_run {
                progress.status(&format!("  dry-run: would create {}", item.filename));
                created.push(item.filename.clone());
                continue;
            }
            progress.status(&format!("  create {}", item.filename));
            let pm_id = client
                .create_item(
                    &project_id,
                    &PmItemCreate {
                        name: name.to_string(),
                        description: doc.body.clone(),
                        rank: item.prefix,
                    },
                )
                .await
                .map_err(pm_to_ops)?;

            // Write pm_id back to frontmatter
            doc.frontmatter.pm_id = Some(pm_id);
            let rendered = doc.render().map_err(pm_to_ops)?;
            std::fs::write(&path, rendered)?;
            created.push(item.filename.clone());
        }
    }

    Ok(ExportResult {
        wave: options.wave.clone(),
        created,
        updated,
    })
}

async fn resolve_asana_token() -> OpsResult<String> {
    if let Ok(token) = std::env::var("ASANA_ACCESS_TOKEN") {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    resolve_asana_token_from_store().await
}

async fn resolve_asana_token_from_store() -> OpsResult<String> {
    let store = open_store(&storage_config_from_env()?)
        .await
        .map_err(|err| OpsError::Message(format!("failed to open lfd credential store: {err}")))?;
    let token = store
        .get_provider_token("asana")
        .await
        .map_err(|err| OpsError::Message(format!("failed to load Asana token: {err}")))?
        .ok_or_else(|| {
            OpsError::Message(
                "No Asana credential found. Run `lf ops auth asana` or `lfq auth asana`."
                    .to_string(),
            )
        })?;

    if token
        .expires_at
        .is_some_and(|expires_at| expires_at <= time::OffsetDateTime::now_utc().unix_timestamp())
    {
        return Err(OpsError::Message(
            "Stored Asana OAuth token has expired. Run `lf ops auth asana` again.".to_string(),
        ));
    }

    Ok(token.access_token)
}

fn storage_config_from_env() -> OpsResult<crate::lfd::store::StorageConfig> {
    crate::lfd::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve lfd credential store: {err}")))
}

/// Extract the first `# ` heading from markdown body.
fn extract_heading(body: &str) -> Option<&str> {
    for line in body.lines() {
        if let Some(heading) = line.strip_prefix("# ") {
            let heading = heading.trim();
            if !heading.is_empty() {
                return Some(heading);
            }
        }
    }
    None
}

/// Write `pm:` block to wave YAML, preserving existing keys.
fn write_pm_config_to_wave_yaml(
    repo: &Path,
    wave: &str,
    provider: &str,
    project_id: &str,
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

    let pm_key = serde_yaml_ng::Value::String("pm".to_string());
    let mut pm_map = map
        .get(&pm_key)
        .and_then(serde_yaml_ng::Value::as_mapping)
        .cloned()
        .unwrap_or_default();
    pm_map.insert(
        serde_yaml_ng::Value::String("provider".to_string()),
        serde_yaml_ng::Value::String(provider.to_string()),
    );
    pm_map.insert(
        serde_yaml_ng::Value::String("project".to_string()),
        serde_yaml_ng::Value::String(project_id.to_string()),
    );
    map.insert(pm_key, serde_yaml_ng::Value::Mapping(pm_map));

    let output = serde_yaml_ng::to_string(&value)
        .map_err(|err| OpsError::Message(format!("failed to encode wave yaml: {err}")))?;
    std::fs::write(&path, output)?;

    Ok(())
}

fn pm_to_ops(err: PmError) -> OpsError {
    OpsError::Message(err.to_string())
}

fn export_order(items: &[WaveItem], reverse: bool) -> Vec<&WaveItem> {
    let mut ordered = items.iter().collect::<Vec<_>>();
    if reverse {
        ordered.reverse();
    }
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_heading_from_body() {
        assert_eq!(
            extract_heading("# 03: Linear client\n\nSome description"),
            Some("03: Linear client")
        );
    }

    #[test]
    fn extract_heading_skips_empty() {
        assert_eq!(extract_heading("no heading here\n"), None);
    }

    #[test]
    fn extract_heading_skips_h2() {
        assert_eq!(
            extract_heading("## Not this\n# This one\n"),
            Some("This one")
        );
    }

    #[test]
    fn export_order_reverses_bootstrap_creates() {
        let items = vec![
            WaveItem {
                filename: "01-one.md".to_string(),
                prefix: 1,
                slug: "one".to_string(),
            },
            WaveItem {
                filename: "02-two.md".to_string(),
                prefix: 2,
                slug: "two".to_string(),
            },
            WaveItem {
                filename: "03-three.md".to_string(),
                prefix: 3,
                slug: "three".to_string(),
            },
        ];

        let ordered = export_order(&items, true)
            .into_iter()
            .map(|item| item.filename.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ordered, vec!["03-three.md", "02-two.md", "01-one.md"]);
    }

    #[test]
    fn write_pm_config_creates_block() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();
        let wave_dir = repo.join("wave").join("test");
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(
            wave_dir.join("test.yaml"),
            "flow: build\ndirection:\n  - clarity\n",
        )
        .unwrap();

        write_pm_config_to_wave_yaml(repo, "test", "asana", "proj-123").unwrap();

        let content = std::fs::read_to_string(wave_dir.join("test.yaml")).unwrap();
        let config: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).unwrap();
        let pm = config.get("pm").expect("pm block should exist");
        assert_eq!(pm.get("provider").unwrap().as_str().unwrap(), "asana");
        assert_eq!(pm.get("project").unwrap().as_str().unwrap(), "proj-123");
        // Existing keys preserved
        assert_eq!(config.get("flow").unwrap().as_str().unwrap(), "build");
    }

    #[test]
    fn write_pm_config_preserves_existing_team_override() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();
        let wave_dir = repo.join("wave").join("test");
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(
            wave_dir.join("test.yaml"),
            "flow: build\npm:\n  provider: asana\n  project: old-proj\n  team: eng-platform\n",
        )
        .unwrap();

        write_pm_config_to_wave_yaml(repo, "test", "asana", "proj-123").unwrap();

        let content = std::fs::read_to_string(wave_dir.join("test.yaml")).unwrap();
        let config: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).unwrap();
        let pm = config.get("pm").expect("pm block should exist");
        assert_eq!(pm.get("provider").unwrap().as_str().unwrap(), "asana");
        assert_eq!(pm.get("project").unwrap().as_str().unwrap(), "proj-123");
        assert_eq!(pm.get("team").unwrap().as_str().unwrap(), "eng-platform");
    }

    #[test]
    fn dry_run_does_not_require_token() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();

        let wave_dir = repo.join("wave").join("test");
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(
            wave_dir.join("test.yaml"),
            "flow: build\npm:\n  provider: asana\n  project: proj-123\n",
        )
        .unwrap();
        std::fs::write(
            wave_dir.join("01-first.md"),
            "# First item\n\nDescription.\n",
        )
        .unwrap();

        // Set a fake token so we get past validation
        std::env::set_var("ASANA_ACCESS_TOKEN", "fake-token-for-test");

        let result = export(
            repo,
            &ExportOptions {
                wave: "test".to_string(),
                dry_run: true,
            },
            &crate::ops::NullProgress,
        )
        .unwrap();

        assert_eq!(result.wave, "test");
        assert_eq!(result.created, vec!["01-first.md"]);
        assert!(result.updated.is_empty());
    }
}
