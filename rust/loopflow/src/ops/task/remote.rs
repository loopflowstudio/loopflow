//! Portable code requirements for one SSH invocation; no execution state travels.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::git::{is_ancestor, is_clean, ref_exists, rev_parse};
use crate::ops::{OpsError, OpsResult};
use crate::store::{PlanningState, PmTaskRecord, SharedStore};
use crate::work::task::Task;

use super::{fetch_task_refs, find_task, task_error};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TaskSource {
    pub branch: String,
    pub commit: String,
    pub planning: PmTaskRecord,
}

impl TaskSource {
    pub async fn resolve(store: &SharedStore, selector: &str) -> OpsResult<Option<Self>> {
        let Some(task) = find_task(store, selector).await? else {
            return Ok(None);
        };
        let pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
        if !is_clean(&task.worktree)? {
            return Err(task_error(format!("Task {} has uncommitted work on branch {}; commit and push it before running it on another machine", task.plan.identifier, pr.branch)));
        }
        let commit = rev_parse(&task.worktree, "HEAD")?;
        require_pushed_code(&task.worktree, &task.plan.identifier, &pr.branch, &commit)?;
        Ok(Some(Self {
            branch: pr.branch,
            commit,
            planning: crate::ops::pm::read_task_planning_async(
                &task.worktree,
                &task.plan.identifier,
                crate::ops::pm::PmRefresh::Auto,
            )
            .await?,
        }))
    }

    pub fn issue(&self) -> &str {
        &self.planning.item.identifier
    }

    pub async fn accept_planning(&self, repo: &Path, store: &SharedStore) -> OpsResult<()> {
        let scope = crate::repository::CanonicalRepo::discover(repo)
            .map_err(task_error)?
            .to_string();
        // Existing accepted facts and invalidations remain authoritative on this machine.
        if store
            .pm_task_observation(&scope, "linear", &self.planning.item.id)
            .await
            .map_err(task_error)?
            .state
            != PlanningState::Unavailable
        {
            return Ok(());
        }
        let project = self
            .planning
            .project
            .as_ref()
            .ok_or_else(|| task_error("SSH Task has no Project"))?;
        let initiative = crate::ops::pm::singular_project_initiative(project)?;
        let name = crate::ops::pm::wave_for_initiative(repo, &initiative)?;
        let team = crate::ops::pm::repository_team_id(repo)?;
        crate::pm::validate_project_ownership(&name, &initiative, Some(&team), project)
            .map_err(task_error)?;
        if self.planning.item.team_id != team {
            return Err(task_error("SSH Task belongs to another repository Team"));
        }
        let wave = crate::work::wave::ensure_wave_row(store, repo, &name)
            .await
            .map_err(task_error)?;
        let acquisition = crate::ops::pm::lock_wave_planning(&wave).await?;
        // Recheck under the planning lock. A local refresh may have won the race.
        if store
            .pm_task_observation(&scope, "linear", &self.planning.item.id)
            .await
            .map_err(task_error)?
            .state
            == PlanningState::Unavailable
        {
            store
                .put_pm_task(
                    &scope,
                    "linear",
                    self.planning.clone(),
                    Some((wave.id().clone(), initiative)),
                    Some(acquisition),
                )
                .await
                .map_err(task_error)?;
        }
        Ok(())
    }

    pub fn require_pushed(&self, repo: &Path) -> OpsResult<()> {
        require_pushed_code(repo, self.issue(), &self.branch, &self.commit)
    }

    pub fn require_checkout(&self, task: &Task) -> OpsResult<()> {
        let branch = crate::engine::git::current_branch(&task.worktree)?;
        if branch.as_deref() != Some(&self.branch)
            || !is_ancestor(&task.worktree, &self.commit, "HEAD")?
        {
            return Err(task_error(format!("Task {} needs branch {} at commit {}; run `lf sync` in {} before continuing; existing work is preserved", self.issue(), self.branch, self.commit, task.worktree.display())));
        }
        Ok(())
    }
}

pub(crate) fn source_for_issue(issue: &str) -> OpsResult<Option<TaskSource>> {
    let Some(value) = std::env::var_os(crate::lf::TASK_SOURCE_ENV) else {
        return Ok(None);
    };
    let source: TaskSource = serde_json::from_str(&value.to_string_lossy())
        .map_err(|error| OpsError::Message(format!("invalid SSH Task source: {error}")))?;
    Ok((source.issue() == issue).then_some(source))
}

fn require_pushed_code(repo: &Path, issue: &str, branch: &str, commit: &str) -> OpsResult<()> {
    fetch_task_refs(repo)?;
    let remote = format!("refs/remotes/origin/{}", branch);
    if !ref_exists(repo, &remote)? {
        return Err(task_error(format!(
            "Task {} branch {} is missing on origin; push it from the source machine first",
            issue, branch
        )));
    }
    if !crate::engine::git::commit_exists(repo, commit)? || !is_ancestor(repo, commit, &remote)? {
        return Err(task_error(format!(
            "Task {} commit {} is missing from origin/{}; push it from the source machine first",
            issue, commit, branch
        )));
    }
    Ok(())
}
