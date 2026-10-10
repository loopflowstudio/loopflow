//! Portable code requirements for one SSH invocation; no execution state travels.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::engine::git::{is_ancestor, is_clean, ref_exists, rev_parse};
use crate::ops::{OpsError, OpsResult};
use crate::store::Store;
use crate::work::task::Task;

use super::{fetch_task_refs, task_error};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TaskSource {
    pub branch: String,
    pub commit: String,
    pub task_id: crate::durable::TaskId,
    pub identifier: String,
    pub issue_id: Option<String>,
}

impl TaskSource {
    pub async fn resolve(store: &Store, selector: &str) -> OpsResult<Option<Self>> {
        let Some(task) = store
            .get_task_by_issue(selector)
            .await
            .map_err(task_error)?
        else {
            return Ok(None);
        };
        let Some(worktree) = task.worktree.as_deref() else {
            return Ok(None);
        };
        if !is_clean(worktree)? {
            return Err(task_error(format!("Task {} has uncommitted work on branch {}; commit and push it before running it on another machine", task.plan.identifier, task.branch)));
        }
        let commit = rev_parse(worktree, "HEAD")?;
        require_pushed_code(worktree, &task.plan.identifier, &task.branch, &commit)?;
        Ok(Some(Self {
            branch: task.branch,
            commit,
            task_id: task.id,
            identifier: task.plan.identifier,
            issue_id: task
                .plan
                .linear_id
                .as_ref()
                .map(|id| id.as_str().to_owned()),
        }))
    }

    pub fn require_pushed(&self, repo: &Path) -> OpsResult<()> {
        require_pushed_code(repo, &self.identifier, &self.branch, &self.commit)
    }

    pub fn require_checkout(&self, task: &Task) -> OpsResult<()> {
        let worktree = task.worktree()?;
        let branch = crate::engine::git::current_branch(worktree)?;
        if branch.as_deref() != Some(&self.branch) || !is_ancestor(worktree, &self.commit, "HEAD")?
        {
            return Err(task_error(format!("Task {} needs branch {} at commit {}; run `lf sync` in {} before continuing; existing work is preserved", self.identifier, self.branch, self.commit, worktree.display())));
        }
        Ok(())
    }
}

pub(crate) fn source_for_task(issue: &str) -> OpsResult<Option<TaskSource>> {
    let Some(value) = std::env::var_os(crate::lf::TASK_SOURCE_ENV) else {
        return Ok(None);
    };
    let source: TaskSource = serde_json::from_str(&value.to_string_lossy())
        .map_err(|error| OpsError::Message(format!("invalid SSH Task source: {error}")))?;
    Ok((source.identifier == issue
        || source.task_id.as_str() == issue
        || source.issue_id.as_deref() == Some(issue))
    .then_some(source))
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
