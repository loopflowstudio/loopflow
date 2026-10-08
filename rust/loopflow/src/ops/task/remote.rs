//! Portable code requirements for one SSH invocation; no execution state travels.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::git::{is_ancestor, is_clean, ref_exists, rev_parse};
use crate::ops::{OpsError, OpsResult};
use crate::store::{PmTaskRecord, SharedStore};
use crate::work::task::Task;

use super::{fetch_task_refs, task_error};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TaskSource {
    pub branch: String,
    pub commit: String,
    pub planning: PmTaskRecord,
}

impl TaskSource {
    pub async fn resolve(store: &SharedStore, selector: &str) -> OpsResult<Option<Self>> {
        let Some(task) = store
            .get_task_by_issue(selector)
            .await
            .map_err(task_error)?
        else {
            return Ok(None);
        };
        let pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("Task {} has no active PR", task.plan.identifier)))?;
        if !is_clean(task.worktree()?)? {
            return Err(task_error(format!("Task {} has uncommitted work on branch {}; commit and push it before running it on another machine", task.plan.identifier, pr.branch)));
        }
        let commit = rev_parse(task.worktree()?, "HEAD")?;
        require_pushed_code(task.worktree()?, &task.plan.identifier, &pr.branch, &commit)?;
        Ok(Some(Self {
            branch: pr.branch,
            commit,
            planning: crate::ops::pm::read_task_planning_async(
                task.worktree()?,
                &task.plan.identifier,
                crate::ops::pm::PmRefresh::Auto,
            )
            .await?,
        }))
    }

    pub fn issue(&self) -> &str {
        &self.planning.item.identifier
    }

    pub fn require_pushed(&self, repo: &Path) -> OpsResult<()> {
        require_pushed_code(repo, self.issue(), &self.branch, &self.commit)
    }

    pub fn require_checkout(&self, task: &Task) -> OpsResult<()> {
        let branch = crate::engine::git::current_branch(task.worktree()?)?;
        if branch.as_deref() != Some(&self.branch)
            || !is_ancestor(task.worktree()?, &self.commit, "HEAD")?
        {
            return Err(task_error(format!("Task {} needs branch {} at commit {}; run `lf sync` in {} before continuing; existing work is preserved", self.issue(), self.branch, self.commit, task.worktree()?.display())));
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
