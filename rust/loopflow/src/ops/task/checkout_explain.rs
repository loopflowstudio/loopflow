//! Checkout intent reads preparation's validators, never its locks or effects.
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ops::context::ContextExplanation;
use crate::ops::run::WorkSelection;
use crate::store::SharedStore;

use super::TaskProcessOptions;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskCheckoutBehavior {
    Prepare,
    Reuse,
    Restore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCheckoutAction {
    pub behavior: TaskCheckoutBehavior,
    /// A local proposal for new Work, not an allocated or reserved path.
    pub path: String,
    pub branch: String,
    pub stack_on: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCheckoutExplanation {
    pub resolution: ContextExplanation,
    pub action: Option<TaskCheckoutAction>,
    pub impediments: Vec<String>,
    pub unavailable: Vec<String>,
}

impl TaskCheckoutExplanation {
    pub fn unavailable(error: impl std::fmt::Display) -> Self {
        Self {
            resolution: ContextExplanation::unavailable(&error),
            action: None,
            impediments: Vec::new(),
            unavailable: vec![error.to_string()],
        }
    }

    pub fn render(&self) -> String {
        let mut lines = vec![self.resolution.render()];
        lines.push(match &self.action {
            Some(action) => format!(
                "Intended action: {} checkout {} on branch {}{}",
                match action.behavior {
                    TaskCheckoutBehavior::Prepare => "prepare proposed",
                    TaskCheckoutBehavior::Reuse => "reuse recorded",
                    TaskCheckoutBehavior::Restore => "restore recorded",
                },
                action.path,
                action.branch,
                action
                    .stack_on
                    .as_ref()
                    .map(|parent| format!("; stack on {parent}"))
                    .unwrap_or_default(),
            ),
            None => "Intended action: unavailable".into(),
        });
        lines.extend(
            self.impediments
                .iter()
                .map(|reason| format!("Impediment: {reason}")),
        );
        lines.extend(
            self.unavailable
                .iter()
                .map(|reason| format!("Unavailable: {reason}")),
        );
        lines.push("Observation only; no Git preparation, placement, claim or provider contact. Execution revalidates before effects.".into());
        lines.join("\n")
    }
}

pub async fn explain_task_checkout(
    store: &SharedStore,
    cwd: &Path,
    selection: WorkSelection<'_>,
    options: &TaskProcessOptions,
) -> TaskCheckoutExplanation {
    let mut report = TaskCheckoutExplanation {
        resolution: crate::ops::context::explain_context(store, cwd, selection, None, None).await,
        action: None,
        impediments: Vec::new(),
        unavailable: Vec::new(),
    };
    if let Err(error) = read_checkout(store, options, &mut report).await {
        report.unavailable.push(error.to_string());
    }
    report
}

async fn read_checkout(
    store: &SharedStore,
    options: &TaskProcessOptions,
    report: &mut TaskCheckoutExplanation,
) -> anyhow::Result<()> {
    let task = super::explain::read_local_task(store, &mut report.resolution).await?;
    // Preparation has different rules from Flow launch: a missing checkout is
    // precisely what checkout restores, not a reason to refuse this command.
    let status = super::task_work_status(store, &task).await?;
    if let Err(error) = super::validate_preparation_status(&task, status) {
        report.impediments.push(error.to_string());
        return Ok(());
    }
    if let Err(error) = store.sqlite.validate_task_planning(&task) {
        match error {
            crate::store::StoreError::InvalidAuthority(_) => {
                report.impediments.push(error.to_string())
            }
            _ => return Err(error.into()),
        }
        return Ok(());
    }
    for result in [
        options.validate_directive(Some(&task)),
        options.validate_workspace_name(&task),
    ] {
        if let Err(error) = result {
            report.impediments.push(error.to_string());
        }
    }
    if !report.impediments.is_empty() {
        return Ok(());
    }
    let wave = super::owning_wave(store, &task).await?;
    let repo = crate::engine::worktrees::main_repo_root(Path::new(wave.repo()))?;
    report.unavailable.push("Checkout leases, placement admission and concurrent changes are rechecked only during execution; this reading reserves nothing".into());
    if let Some(worktree) = &task.worktree {
        let Some(pr) = store.active_task_pr(&task.id).await? else {
            report
                .impediments
                .push("Task has no active PR from which to restore its checkout".into());
            return Ok(());
        };
        let present = worktree.join(".git").try_exists()?;
        report.action = Some(TaskCheckoutAction {
            behavior: if present {
                TaskCheckoutBehavior::Reuse
            } else {
                TaskCheckoutBehavior::Restore
            },
            path: worktree.display().to_string(),
            branch: pr.branch.clone(),
            stack_on: options.stack_on.clone(),
        });
        if !present {
            match super::validate_checkout_restoration(store, &task, &pr, &repo) {
                Ok(()) => {}
                Err(super::OpsError::Message(reason)) => report.impediments.push(reason),
                Err(error) => return Err(error.into()),
            }
            if !crate::engine::worktrees::branch_exists(&repo, &pr.branch)? {
                report.unavailable.push("Recorded branch is absent locally; restoration must fetch before deciding whether its history can be recovered. No fetch was performed".into());
            }
        }
        report.unavailable.push("Checkout finalization may record PrStarted and clear inherited scratch for an untouched stacked PR; it has not run".into());
        if let Some(parent) = options.stack_on.as_deref() {
            match super::select_existing_stack_parent(store, &task, &pr, parent).await {
                Ok(_) => report.unavailable.push("Stack mutation validation runs under the execution lock; no dependency was changed".into()),
                Err(error) => report.impediments.push(error.to_string()),
            }
        }
    } else {
        let branch = super::task_planning_item(store, &task)?.branch_name;
        let (plan, _) = match super::plan_task_placement(&repo, &task, branch.as_deref(), options) {
            Ok(plan) => plan,
            Err(super::OpsError::Message(reason)) => {
                report.impediments.push(reason);
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        report.action = Some(TaskCheckoutAction {
            behavior: TaskCheckoutBehavior::Prepare,
            path: plan.worktree_path.display().to_string(),
            branch: plan.branch,
            stack_on: options.stack_on.clone(),
        });
        if let Err(error) =
            super::select_new_stack_parent(store, &task, options.stack_on.as_deref()).await
        {
            report.impediments.push(error.to_string());
        }
        report.unavailable.push("Proposed path and branch use local Git facts only; fetched refs, base commit and provider PR evidence are unavailable. No checkout or PR was allocated".into());
    }
    Ok(())
}
