use std::path::Path;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pm::PmRefresh;
use crate::pm::{PmItem, PmProject};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTask {
    pub wave: String,
    pub observed_at: i64,
    pub project: PmProject,
    pub item: PmItem,
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
    if item.team_id.as_deref() != Some(team_id.as_str()) {
        return Err(OpsError::Message(format!(
            "task {} belongs to Team {}, expected repository Team {}",
            item.identifier,
            item.team_id.as_deref().unwrap_or("unmapped"),
            team_id
        )));
    }
    Ok(ResolvedTask {
        wave,
        observed_at: record.observed_at,
        project,
        item,
    })
}
