use std::path::Path;

use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::lfd::pm::{PmItem, PmItemCreate, PmItemUpdate, PmProviderKind};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pm::{block_on_pm, build_client, pm_to_ops, resolve_provider, PmContext};
use crate::ops::progress::Progress;
use crate::ops::util::resolve_wave_name;

#[derive(Debug, Clone, Default)]
pub struct RoadmapFetchOptions {
    pub wave: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapFetchResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub project: String,
    pub items: Vec<PmItem>,
}

#[derive(Debug, Clone)]
pub struct RoadmapUpdateOptions {
    pub wave: Option<String>,
    pub id: Option<String>,
    pub title: String,
    pub notes: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadmapUpdateResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub id: String,
    pub created: bool,
    pub completed: bool,
}

/// Resolve the wave's live roadmap handle (`roadmap:` in GOAL.md frontmatter) into a
/// PM provider client + project id. No local mirror — this always talks to the
/// provider directly.
async fn resolve_roadmap_context(repo: &Path, wave: &str) -> OpsResult<PmContext> {
    let handle = read_wave_config(repo, wave)
        .and_then(|config| config.roadmap)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            OpsError::Message(format!(
                "wave/{wave}/GOAL.md has no `roadmap:` handle. Set `roadmap: asana://<project_id>` \
                 (or a bare project id alongside a `pm:` provider) to enable `lf op roadmap`."
            ))
        })?;

    let (provider, project) = match handle.split_once("://") {
        Some((scheme, rest)) => (parse_provider_scheme(scheme)?, rest.to_string()),
        None => (resolve_provider(repo, wave)?, handle),
    };

    let client = build_client(repo, provider).await?;
    Ok(PmContext {
        client,
        provider,
        project,
    })
}

fn parse_provider_scheme(scheme: &str) -> OpsResult<PmProviderKind> {
    match scheme.to_ascii_lowercase().as_str() {
        "asana" => Ok(PmProviderKind::Asana),
        "linear" => Ok(PmProviderKind::Linear),
        "notion" => Ok(PmProviderKind::Notion),
        other => Err(OpsError::Message(format!(
            "unknown roadmap provider scheme: {other}://"
        ))),
    }
}

pub fn roadmap_fetch(
    repo: &Path,
    options: &RoadmapFetchOptions,
    progress: &impl Progress,
) -> OpsResult<RoadmapFetchResult> {
    block_on_pm(roadmap_fetch_async(repo, options, progress))
}

async fn roadmap_fetch_async(
    repo: &Path,
    options: &RoadmapFetchOptions,
    progress: &impl Progress,
) -> OpsResult<RoadmapFetchResult> {
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let ctx = resolve_roadmap_context(repo, &wave).await?;
    fetch_items(&wave, &ctx, progress).await
}

async fn fetch_items(
    wave: &str,
    ctx: &PmContext,
    progress: &impl Progress,
) -> OpsResult<RoadmapFetchResult> {
    progress.status(&format!(
        "fetching {:?} project {} for wave/{wave}",
        ctx.provider, ctx.project
    ));
    let items = ctx
        .client
        .list_items(&ctx.project)
        .await
        .map_err(pm_to_ops)?;
    Ok(RoadmapFetchResult {
        wave: wave.to_string(),
        provider: ctx.provider,
        project: ctx.project.clone(),
        items,
    })
}

/// One scannable line per task: `id`, status, assignee, name.
pub fn format_roadmap_item(item: &PmItem) -> String {
    let status = if item.completed { "done" } else { "open" };
    let assignee = item.assignee.as_deref().unwrap_or("-");
    format!(
        "{:<8} {:<40} assignee:{assignee:<20} id:{}",
        status, item.name, item.id
    )
}

pub fn roadmap_update(
    repo: &Path,
    options: &RoadmapUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<RoadmapUpdateResult> {
    block_on_pm(roadmap_update_async(repo, options, progress))
}

async fn roadmap_update_async(
    repo: &Path,
    options: &RoadmapUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<RoadmapUpdateResult> {
    let wave = resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let ctx = resolve_roadmap_context(repo, &wave).await?;
    apply_update(&wave, &ctx, options, progress).await
}

async fn apply_update(
    wave: &str,
    ctx: &PmContext,
    options: &RoadmapUpdateOptions,
    progress: &impl Progress,
) -> OpsResult<RoadmapUpdateResult> {
    let mark_done = match options.status.as_deref() {
        None => false,
        Some(status)
            if status.eq_ignore_ascii_case("done")
                || status.eq_ignore_ascii_case("complete")
                || status.eq_ignore_ascii_case("completed") =>
        {
            true
        }
        Some(other) => {
            return Err(OpsError::Message(format!(
                "unsupported roadmap status {other:?}; only \"done\" is supported"
            )));
        }
    };

    let (id, created) = match options.id.as_ref() {
        Some(id) => {
            progress.status(&format!("updating {:?} task {id}", ctx.provider));
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
                "creating {:?} task on project {} for wave/{wave}",
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

    if mark_done {
        ctx.client.complete_item(&id).await.map_err(pm_to_ops)?;
    }

    Ok(RoadmapUpdateResult {
        wave: wave.to_string(),
        provider: ctx.provider,
        id,
        created,
        completed: mark_done,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::pm::{PmProject, PmResult};
    use crate::ops::NullProgress;
    use async_trait::async_trait;
    use tempfile::TempDir;

    #[derive(Debug)]
    struct FakeProvider {
        items: Vec<PmItem>,
    }

    #[async_trait]
    impl crate::lfd::pm::PmProvider for FakeProvider {
        async fn create_project(&self, _name: &str, _description: &str) -> PmResult<String> {
            panic!("create_project should not be called in this test");
        }

        async fn list_projects(&self, _team_id: &str) -> PmResult<Vec<PmProject>> {
            panic!("list_projects should not be called in this test");
        }

        async fn list_items(&self, _project_id: &str) -> PmResult<Vec<PmItem>> {
            Ok(self.items.clone())
        }

        async fn create_item(&self, _project_id: &str, item: &PmItemCreate) -> PmResult<String> {
            Ok(format!("new-{}", item.name))
        }

        async fn update_item(&self, _item_id: &str, _update: &PmItemUpdate) -> PmResult<()> {
            Ok(())
        }

        async fn complete_item(&self, _item_id: &str) -> PmResult<()> {
            Ok(())
        }

        async fn comment(&self, _item_id: &str, _body: &str) -> PmResult<()> {
            panic!("comment should not be called in this test");
        }

        async fn claim_item(&self, _item_id: &str, _branch: &str) -> PmResult<()> {
            panic!("claim_item should not be called in this test");
        }
    }

    fn fake_ctx(items: Vec<PmItem>) -> PmContext {
        PmContext {
            client: Box::new(FakeProvider { items }),
            provider: PmProviderKind::Asana,
            project: "proj-1".to_string(),
        }
    }

    #[tokio::test]
    async fn fetch_items_renders_tasks_from_the_project() {
        let ctx = fake_ctx(vec![PmItem {
            id: "123".to_string(),
            name: "Ship it".to_string(),
            description: "".to_string(),
            rank: 0,
            completed: false,
            assignee: Some("me".to_string()),
        }]);

        let result = fetch_items("goals", &ctx, &NullProgress)
            .await
            .expect("fetch succeeds");

        assert_eq!(result.wave, "goals");
        assert_eq!(result.items.len(), 1);
        let line = format_roadmap_item(&result.items[0]);
        assert!(line.contains("Ship it"));
        assert!(line.contains("id:123"));
        assert!(line.contains("assignee:me"));
        assert!(line.starts_with("open"));
    }

    #[tokio::test]
    async fn resolve_roadmap_context_errors_without_a_roadmap_handle() {
        let dir = TempDir::new().expect("temp dir");
        let wave_dir = dir.path().join("wave").join("goals");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("GOAL.md"),
            "---\nprimary_flow: build\n---\nDrive the work.\n",
        )
        .expect("write goal");

        let result = resolve_roadmap_context(dir.path(), "goals").await;
        let Err(err) = result else {
            panic!("missing roadmap handle should error");
        };
        assert!(err.to_string().contains("no `roadmap:` handle"));
    }

    #[tokio::test]
    async fn apply_update_creates_when_no_id_given() {
        let ctx = fake_ctx(Vec::new());
        let options = RoadmapUpdateOptions {
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
        assert_eq!(result.id, "new-New task");
        assert!(!result.completed);
    }

    #[tokio::test]
    async fn apply_update_completes_when_status_is_done() {
        let ctx = fake_ctx(Vec::new());
        let options = RoadmapUpdateOptions {
            wave: None,
            id: Some("123".to_string()),
            title: "Existing".to_string(),
            notes: None,
            status: Some("done".to_string()),
        };

        let result = apply_update("goals", &ctx, &options, &NullProgress)
            .await
            .expect("update succeeds");

        assert!(!result.created);
        assert_eq!(result.id, "123");
        assert!(result.completed);
    }

    #[tokio::test]
    async fn apply_update_rejects_unsupported_status() {
        let ctx = fake_ctx(Vec::new());
        let options = RoadmapUpdateOptions {
            wave: None,
            id: Some("123".to_string()),
            title: "Existing".to_string(),
            notes: None,
            status: Some("blocked".to_string()),
        };

        let err = apply_update("goals", &ctx, &options, &NullProgress)
            .await
            .expect_err("unsupported status should error");
        assert!(err.to_string().contains("unsupported roadmap status"));
    }
}
