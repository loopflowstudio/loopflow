use std::path::Path;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pm::{PmRefresh, PmShowOptions, PmShowResult, PmTaskUpdate, PmUpdateOptions};
use crate::pm::{PmItem, PmProject};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTask {
    pub wave: String,
    pub observed_at: i64,
    pub project: PmProject,
    pub item: PmItem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedProject {
    pub snapshot: PmShowResult,
    pub project: PmProject,
}

pub fn load_wave(repo: &Path, wave: &str, refresh: PmRefresh) -> OpsResult<PmShowResult> {
    crate::ops::pm::pm_show(
        repo,
        &PmShowOptions {
            wave: Some(wave.to_string()),
            refresh,
        },
        &crate::ops::NullProgress,
    )
}

pub fn resolve_task(repo: &Path, issue: &str, refresh: PmRefresh) -> OpsResult<ResolvedTask> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| OpsError::Message(format!("failed to create async runtime: {error}")))?;
    runtime.block_on(resolve_task_async(repo, issue, refresh))
}

pub(crate) async fn resolve_task_async(
    repo: &Path,
    issue: &str,
    refresh: PmRefresh,
) -> OpsResult<ResolvedTask> {
    let record = crate::ops::pm::read_task_planning_async(repo, issue, refresh).await?;
    let item = record.item;
    let project = record.project.ok_or_else(|| {
        OpsError::Message(format!(
            "task {} has no Project; planning can be inspected but managed work requires ownership",
            item.identifier
        ))
    })?;
    let initiative = crate::ops::pm::singular_project_initiative(&project)?;
    let wave = crate::ops::pm::wave_for_initiative(repo, &initiative)?;
    let team_id = crate::ops::pm::repository_team_id(repo)?;
    crate::pm::validate_project_ownership(&wave, &initiative, Some(&team_id), &project)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    if item.team_id != team_id {
        return Err(OpsError::Message(format!(
            "task {} belongs to Team {}, expected repository Team {}",
            item.identifier, item.team_id, team_id
        )));
    }
    Ok(ResolvedTask {
        wave,
        observed_at: record.observed_at,
        project,
        item,
    })
}

pub fn resolve_current_project(
    repo: &Path,
    wave: Option<&str>,
    refresh: PmRefresh,
) -> OpsResult<ResolvedProject> {
    let wave = crate::work::wave::context::resolve_managed_wave_sync(Some(repo), wave)
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let snapshot = load_wave(repo, wave.slug(), refresh)?;
    let project = tokio::runtime::Runtime::new()
        .map_err(|error| OpsError::Message(error.to_string()))?
        .block_on(async {
            let store = crate::ops::pm::pm_store().await?;
            crate::ops::chapter::current_project(&store, &wave).await
        })?;
    Ok(ResolvedProject { snapshot, project })
}

pub(crate) async fn create_and_load_task<T, F, Fut>(
    repo: &Path,
    wave: &str,
    title: &str,
    report: &str,
    marker: &str,
    prepare: F,
) -> OpsResult<(ResolvedTask, T)>
where
    F: FnOnce(Option<PmItem>, PmProject) -> Fut,
    Fut: std::future::Future<Output = OpsResult<T>>,
{
    let (issue, prepared) =
        crate::ops::pm::pm_create_task_idempotent(repo, wave, title, report, marker, prepare)
            .await?;
    let resolved = resolve_task_async(repo, &issue, PmRefresh::Force)
        .await
        .map_err(|error| OpsError::Message(format!(
            "Linear task {issue} is committed, but its planning could not be confirmed: {error}. Retry the same `lf task create` command, retaining its original options, to reuse the issue's creation marker."
        )))?;
    if resolved.wave != wave {
        return Err(OpsError::Message(format!(
            "Linear task {issue} belongs to wave/{}, expected wave/{wave}",
            resolved.wave
        )));
    }
    Ok((resolved, prepared))
}

pub async fn complete_task(
    repo: &Path,
    wave: &str,
    item_id: &str,
    pr: Option<&str>,
) -> OpsResult<()> {
    crate::ops::pm::pm_update_async(
        repo,
        &PmUpdateOptions {
            wave: Some(wave.to_string()),
            id: item_id.to_string(),
            update: PmTaskUpdate::Complete {
                pr: pr.map(str::to_string),
            },
        },
        &crate::ops::NullProgress,
    )
    .await?;
    Ok(())
}
