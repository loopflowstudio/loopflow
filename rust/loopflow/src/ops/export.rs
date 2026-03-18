use std::collections::HashSet;
use std::path::Path;

use crate::lfd::pm::{PmError, PmItemCreate, PmItemUpdate, RoadmapItemDocument};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::ingest::{list_numbered_items, WaveItem};
use crate::ops::pm::{
    body_without_heading, build_export_contexts, extract_heading, title_case,
    write_pm_project_to_wave_yaml, PmContext,
};
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

    let contexts = build_export_contexts(repo, &options.wave).await?;
    let mut created = HashSet::new();
    let mut updated = HashSet::new();

    for ctx in contexts {
        let result = export_to_provider(repo, &wave_dir, options, ctx, progress).await?;
        created.extend(result.created);
        updated.extend(result.updated);
    }

    let mut created = created.into_iter().collect::<Vec<_>>();
    let mut updated = updated.into_iter().collect::<Vec<_>>();
    created.sort();
    updated.sort();

    Ok(ExportResult {
        wave: options.wave.clone(),
        created,
        updated,
    })
}

async fn export_to_provider(
    repo: &Path,
    wave_dir: &Path,
    options: &ExportOptions,
    ctx: PmContext,
    progress: &impl Progress,
) -> OpsResult<ExportResult> {
    let provider_kind = ctx.provider;
    let (project_id, reverse_create_order) = if !ctx.project.is_empty() {
        progress.status(&format!(
            "exporting to {provider_kind:?} project {}",
            ctx.project
        ));
        (ctx.project.clone(), false)
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

    let items = list_numbered_items(wave_dir)?;
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
    use crate::lfd::pm::PmProviderKind;

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
