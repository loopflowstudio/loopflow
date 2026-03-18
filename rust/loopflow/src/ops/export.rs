use std::path::Path;

use crate::lfd::pm::{PmError, PmItemCreate, PmItemUpdate, PmProviderKind, RoadmapItemDocument};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::ingest::{list_numbered_items, WaveItem};
use crate::ops::pm::{body_without_heading, build_provider, extract_heading, title_case};
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

    let ctx = build_provider(repo, &options.wave).await?;
    let provider_kind = ctx.provider;

    // Resolve or create project
    let project_id = &ctx.project;
    let (project_id, reverse_create_order) = if !project_id.is_empty() {
        progress.status(&format!(
            "exporting to {provider_kind:?} project {project_id}"
        ));
        (project_id.to_string(), false)
    } else if options.dry_run {
        progress.status(&format!(
            "dry-run: would create {provider_kind:?} project \"{}\"",
            options.wave
        ));
        ("(dry-run)".to_string(), false)
    } else {
        progress.status(&format!(
            "creating {provider_kind:?} project \"{}\"",
            options.wave
        ));
        let project_name = title_case(&options.wave);
        let id = ctx
            .client
            .create_project(&project_name, "")
            .await
            .map_err(pm_to_ops)?;
        write_pm_project_to_wave_yaml(repo, &options.wave, provider_kind, &id)?;
        progress.status(&format!("created {provider_kind:?} project {id}"));
        (id, true)
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
        let description = body_without_heading(&doc.body);

        if let Some(pm_id) = doc.frontmatter.id_for(provider_kind) {
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
            ctx.client
                .update_item(
                    pm_id,
                    &PmItemUpdate {
                        name: Some(name.to_string()),
                        description: Some(description.to_string()),
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
            let pm_id = ctx
                .client
                .create_item(
                    &project_id,
                    &PmItemCreate {
                        name: name.to_string(),
                        description: description.to_string(),
                        rank: item.prefix,
                    },
                )
                .await
                .map_err(pm_to_ops)?;

            // Write pm_id back to frontmatter
            doc.frontmatter.set_id(provider_kind, pm_id);
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

/// Write a provider-specific project ID into the wave YAML's `pm:` block.
pub(crate) fn write_pm_project_to_wave_yaml(
    repo: &Path,
    wave: &str,
    provider: PmProviderKind,
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

    let project_key = match provider {
        PmProviderKind::Asana => "asana_project",
        PmProviderKind::Linear => "linear_project",
    };
    pm_map.insert(
        serde_yaml_ng::Value::String(project_key.to_string()),
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

        write_pm_project_to_wave_yaml(repo, "test", PmProviderKind::Asana, "proj-123").unwrap();

        let content = std::fs::read_to_string(wave_dir.join("test.yaml")).unwrap();
        let config: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).unwrap();
        let pm = config.get("pm").expect("pm block should exist");
        assert_eq!(
            pm.get("asana_project").unwrap().as_str().unwrap(),
            "proj-123"
        );
        // Existing keys preserved
        assert_eq!(config.get("flow").unwrap().as_str().unwrap(), "build");
    }

    #[test]
    fn write_pm_project_preserves_other_provider_ids() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();
        let wave_dir = repo.join("wave").join("test");
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(
            wave_dir.join("test.yaml"),
            "flow: build\npm:\n  linear_project: lin-old\n",
        )
        .unwrap();

        write_pm_project_to_wave_yaml(repo, "test", PmProviderKind::Asana, "proj-123").unwrap();

        let content = std::fs::read_to_string(wave_dir.join("test.yaml")).unwrap();
        let config: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).unwrap();
        let pm = config.get("pm").expect("pm block should exist");
        assert_eq!(
            pm.get("asana_project").unwrap().as_str().unwrap(),
            "proj-123"
        );
        assert_eq!(
            pm.get("linear_project").unwrap().as_str().unwrap(),
            "lin-old"
        );
    }
}
