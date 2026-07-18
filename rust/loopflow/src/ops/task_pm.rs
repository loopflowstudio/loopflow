use std::path::Path;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pm::{PmRefresh, PmShowOptions, PmShowResult, PmUpdateOptions};
use crate::pm::{PmItem, PmProject};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTask {
    pub snapshot: PmShowResult,
    pub project: PmProject,
    pub item: PmItem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProject {
    pub snapshot: PmShowResult,
    pub project: PmProject,
}

pub fn load_wave(repo: &Path, wave: &str, refresh: PmRefresh) -> OpsResult<PmShowResult> {
    crate::ops::pm::pm_show(
        repo,
        &PmShowOptions {
            wave: Some(wave.to_string()),
            project: None,
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
            project: None,
            refresh,
        },
        &crate::ops::NullProgress,
    )
    .await
}

pub fn resolve_task(repo: &Path, issue: &str, refresh: PmRefresh) -> OpsResult<ResolvedTask> {
    let mut matches = Vec::new();
    for wave in crate::ops::pm::list_pm_waves(repo)? {
        let Ok(snapshot) = load_wave(repo, &wave, PmRefresh::Never) else {
            continue;
        };
        if let Some(item) = snapshot
            .items
            .iter()
            .find(|item| item.id == issue || item.identifier.eq_ignore_ascii_case(issue))
        {
            project_for_item(&snapshot, item)?;
            matches.push((snapshot.wave.clone(), item.id.clone()));
        }
    }
    let (wave, item_id) = match matches.len() {
        0 => {
            return Err(OpsError::Message(format!(
                "task {issue:?} is absent from local PM snapshots. Run `lf pm sync --wave <wave>`."
            )))
        }
        1 => matches.pop().expect("one task match"),
        count => {
            return Err(OpsError::Message(format!(
                "task {issue:?} belongs to {count} PM snapshots; repair Wave ownership before running it"
            )))
        }
    };
    let snapshot = load_wave(repo, &wave, refresh)?;
    let item = snapshot
        .items
        .iter()
        .find(|item| item.id == item_id)
        .cloned()
        .ok_or_else(|| {
            OpsError::Message(format!(
                "task {issue:?} disappeared from wave/{wave}; run `lf pm sync --wave {wave}`"
            ))
        })?;
    if item.completed {
        return Err(OpsError::Message(format!(
            "task {} is already complete and cannot start a Task",
            item.identifier
        )));
    }
    let project = project_for_item(&snapshot, &item)?;
    Ok(ResolvedTask {
        snapshot,
        project,
        item,
    })
}

pub fn resolve_project(
    repo: &Path,
    project_id: &str,
    refresh: PmRefresh,
) -> OpsResult<ResolvedProject> {
    let mut matches = Vec::new();
    for wave in crate::ops::pm::list_pm_waves(repo)? {
        let Ok(snapshot) = load_wave(repo, &wave, PmRefresh::Never) else {
            continue;
        };
        if snapshot
            .projects
            .iter()
            .any(|project| project.id == project_id || project.slug == project_id)
        {
            matches.push(snapshot.wave);
        }
    }
    let wave = match matches.len() {
        0 => {
            return Err(OpsError::Message(format!(
                "Linear Project {project_id:?} is absent from local PM snapshots"
            )))
        }
        1 => matches.pop().expect("one project match"),
        count => {
            return Err(OpsError::Message(format!(
                "Linear Project {project_id:?} belongs to {count} Wave snapshots"
            )))
        }
    };
    let snapshot = load_wave(repo, &wave, refresh)?;
    let project = snapshot
        .projects
        .iter()
        .find(|project| project.id == project_id || project.slug == project_id)
        .cloned()
        .ok_or_else(|| OpsError::Message(format!("Linear Project {project_id:?} disappeared")))?;
    Ok(ResolvedProject { snapshot, project })
}

pub fn create_and_load_task(
    repo: &Path,
    wave: &str,
    project: &str,
    title: &str,
    marker: &str,
) -> OpsResult<ResolvedTask> {
    let result = crate::ops::pm::pm_create_task_idempotent(
        repo,
        wave,
        project,
        title,
        marker,
        &crate::ops::NullProgress,
    )?;
    if let Err(error) = load_wave(repo, wave, PmRefresh::Force) {
        return Err(OpsError::Message(format!(
            "Linear task {} is committed, but the local wave/{wave} snapshot could not refresh: {error}. No new Task or worktree was created. Run `lf pm sync --wave {wave}`, then `lf task run {}`. Retrying `lf task start` is also safe because the Linear task carries an idempotency marker.",
            result.id, result.id
        )));
    }
    resolve_task(repo, &result.id, PmRefresh::Never)
}

pub async fn complete_task(
    repo: &Path,
    wave: &str,
    item_id: &str,
    pr: Option<&str>,
) -> OpsResult<()> {
    // main's `pm_update_async` completes the Linear mutation and refreshes the
    // snapshot in one call, returning `Err` if either step fails; a successful
    // return means the write-back is reconciled.
    crate::ops::pm::pm_update_async(
        repo,
        &PmUpdateOptions {
            wave: Some(wave.to_string()),
            project: None,
            id: Some(item_id.to_string()),
            title: None,
            notes: None,
            status: Some("done".to_string()),
            pr: pr.map(str::to_string),
        },
        &crate::ops::NullProgress,
    )
    .await?;
    Ok(())
}

pub async fn retry_complete_task(
    repo: &Path,
    wave: &str,
    item_id: &str,
    pr: Option<&str>,
) -> OpsResult<()> {
    let snapshot = load_wave_async(repo, wave, PmRefresh::Force).await?;
    let item = snapshot
        .items
        .iter()
        .find(|item| item.id == item_id)
        .ok_or_else(|| {
            OpsError::Message(format!(
                "completed task {item_id} is absent from refreshed wave/{wave} snapshot"
            ))
        })?;
    if item.completed {
        return Ok(());
    }
    complete_task(repo, wave, item_id, pr).await
}

fn project_for_item(snapshot: &PmShowResult, item: &PmItem) -> OpsResult<PmProject> {
    let slug = item.project.as_deref().ok_or_else(|| {
        OpsError::Message(format!(
            "task {} has no Project in wave/{}",
            item.identifier, snapshot.wave
        ))
    })?;
    snapshot
        .projects
        .iter()
        .find(|project| project.slug == slug)
        .cloned()
        .ok_or_else(|| {
            OpsError::Message(format!(
                "task {} names unknown Project {slug:?} in wave/{}",
                item.identifier, snapshot.wave
            ))
        })
}
