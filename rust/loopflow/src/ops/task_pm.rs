use std::path::Path;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pm::{PmRefresh, PmShowOptions, PmShowResult, PmTaskUpdate, PmUpdateOptions};
use crate::pm::{PmItem, PmPortfolioValidator, PmProject};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTask {
    pub snapshot: PmShowResult,
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

async fn load_wave_async(repo: &Path, wave: &str, refresh: PmRefresh) -> OpsResult<PmShowResult> {
    crate::ops::pm::pm_show_async(
        repo,
        &PmShowOptions {
            wave: Some(wave.to_string()),
            refresh,
        },
        &crate::ops::NullProgress,
    )
    .await
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
    let team_id = crate::ops::pm::repository_team_id(repo)?;
    let mut matches = Vec::new();
    for snapshot in repository_snapshots_async(repo, &team_id).await? {
        if let Some(item) = snapshot
            .items
            .iter()
            .find(|item| item.id == issue || item.identifier.eq_ignore_ascii_case(issue))
        {
            matches.push((snapshot.wave.clone(), item.id.clone()));
        }
    }
    let (wave, item_id) = match matches.len() {
        0 => {
            return Err(OpsError::Message(format!(
                "task {issue:?} is absent from local PM snapshots. Run `lf wave sync --wave <wave>`."
            )))
        }
        1 => matches.pop().expect("one task match"),
        count => {
            return Err(OpsError::Message(format!(
                "task {issue:?} belongs to {count} PM snapshots; repair Wave ownership before running it"
            )))
        }
    };
    let snapshot = load_wave_async(repo, &wave, refresh).await?;
    let item = snapshot
        .items
        .iter()
        .find(|item| item.id == item_id)
        .cloned()
        .ok_or_else(|| {
            OpsError::Message(format!(
                "task {issue:?} disappeared from wave/{wave}; run `lf wave sync --wave {wave}`"
            ))
        })?;
    let project = project_for_item(&snapshot, &item, &team_id)?;
    Ok(ResolvedTask {
        snapshot,
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
    let snapshot = load_wave(repo, wave.name(), refresh)?;
    let project = tokio::runtime::Runtime::new()
        .map_err(|error| OpsError::Message(error.to_string()))?
        .block_on(async {
            let store = crate::ops::pm::pm_store().await?;
            crate::ops::chapter::current_project(&store, &wave).await
        })?;
    Ok(ResolvedProject { snapshot, project })
}

async fn repository_snapshots_async(repo: &Path, team_id: &str) -> OpsResult<Vec<PmShowResult>> {
    let mut snapshots = Vec::new();
    let mut ownership = PmPortfolioValidator::default();
    for wave in crate::ops::pm::list_pm_waves(repo)? {
        let snapshot = match load_wave_async(repo, &wave, PmRefresh::Never).await {
            Ok(snapshot) => snapshot,
            Err(error) if error.to_string().contains("has no local PM snapshot") => continue,
            Err(error) => return Err(error),
        };
        ownership
            .validate(
                &snapshot.wave,
                &snapshot.initiative,
                Some(team_id),
                &snapshot.projects,
                &snapshot.items,
            )
            .map_err(|error| OpsError::Message(error.to_string()))?;
        snapshots.push(snapshot);
    }
    Ok(snapshots)
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
    if let Err(error) = load_wave_async(repo, wave, PmRefresh::Force).await {
        return Err(OpsError::Message(format!(
            "Linear task {issue} is committed, but the local wave/{wave} snapshot could not refresh: {error}. No new Task or worktree was created. Retry the same `lf task create` command, retaining its original options, to refresh the snapshot and reuse the issue's creation marker."
        )));
    }
    Ok((
        resolve_task_async(repo, &issue, PmRefresh::Never).await?,
        prepared,
    ))
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

fn project_for_item(snapshot: &PmShowResult, item: &PmItem, team_id: &str) -> OpsResult<PmProject> {
    if item.team_id != team_id {
        return Err(OpsError::Message(format!(
            "task {} belongs to Linear Team {}, expected repository Team {}; \
             run `lf doctor --planning` and repair repository ownership",
            item.identifier, item.team_id, team_id
        )));
    }
    let project = snapshot
        .projects
        .iter()
        .find(|project| project.id == item.project_id)
        .cloned()
        .ok_or_else(|| {
            OpsError::Message(format!(
                "task {} names unknown Project {} in wave/{}",
                item.identifier, item.project_id, snapshot.wave
            ))
        })?;
    validate_project_ownership(snapshot, &project, team_id)?;
    if item.project != project.slug {
        return Err(OpsError::Message(format!(
            "task {} carries stale Project slug {:?}, expected {:?}; run `lf wave sync --wave {}`",
            item.identifier, item.project, project.slug, snapshot.wave
        )));
    }
    Ok(project)
}

fn validate_project_ownership(
    snapshot: &PmShowResult,
    project: &PmProject,
    team_id: &str,
) -> OpsResult<()> {
    crate::pm::validate_project_ownership(
        &snapshot.wave,
        &snapshot.initiative,
        Some(team_id),
        project,
    )
    .map_err(|error| OpsError::Message(error.to_string()))
}
