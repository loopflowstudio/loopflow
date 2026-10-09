//! Invocation readings share Workflow choice and launch admission, never preparation.
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::durable::TaskExecutionSource;
use crate::ops::context::{ContextExplanation, ContextFact};
use crate::ops::run::WorkSelection;
use crate::store::SharedStore;

use super::{select_task_run, TaskProcessOptions, TaskRunSelection};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskRunAction {
    Edge {
        workflow: String,
        take_up: bool,
        from: String,
        to: String,
        flow: Option<String>,
    },
    Flow {
        flow: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunExplanation {
    pub resolution: ContextExplanation,
    pub action: Option<TaskRunAction>,
    pub impediments: Vec<String>,
    /// Evidence not read or not available; an empty impediment list is not admission.
    pub unavailable: Vec<String>,
}

impl TaskRunExplanation {
    pub fn render(&self) -> String {
        let action = match &self.action {
            Some(TaskRunAction::Edge {
                workflow,
                take_up,
                from,
                to,
                flow,
            }) => format!(
                "{}Workflow {workflow}: {from} → {to}; {}",
                if *take_up { "take up " } else { "" },
                flow.as_ref()
                    .map(|flow| format!("run {flow}"))
                    .unwrap_or_else(|| "run no Flow".into()),
            ),
            Some(TaskRunAction::Flow { flow }) => format!("run {flow} without moving the Workflow"),
            None => "unavailable".into(),
        };
        let mut lines = vec![
            self.resolution.render(),
            format!("Intended action: {action}"),
        ];
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
        lines.push(
            "Observation only; launch re-reads admission and prepares Work. Nothing was executed."
                .into(),
        );
        lines.join("\n")
    }
}

pub async fn explain_task_run(
    store: &SharedStore,
    cwd: &Path,
    selection: WorkSelection<'_>,
    options: &TaskProcessOptions,
) -> TaskRunExplanation {
    let resolution = crate::ops::context::explain_context(store, cwd, selection, None, None).await;
    let mut report = TaskRunExplanation {
        resolution,
        action: None,
        impediments: Vec::new(),
        unavailable: Vec::new(),
    };
    if let Err(error) = read_task_run(store, options, &mut report).await {
        report.unavailable.push(error.to_string());
    }
    report
}

async fn read_task_run(
    store: &SharedStore,
    options: &TaskProcessOptions,
    report: &mut TaskRunExplanation,
) -> anyhow::Result<()> {
    let task = read_local_task(store, &mut report.resolution).await?;
    if task.worktree.is_none() {
        report.unavailable.push("Future checkout and peer-exclusive first-start admission are not observed; definitions are read from the local repository before preparation".into());
    }
    // The launch owner's read-only check includes planning validity, current
    // chapter, completion and cancellation, rather than a status approximation.
    if let Err(error) = store.sqlite.require_task_launch(&task.id) {
        match error {
            crate::store::StoreError::InvalidAuthority(_) => {
                report.impediments.push(error.to_string())
            }
            _ => report.unavailable.push(error.to_string()),
        }
    }
    if let Some(blocker) = super::task_worktree_blocker(store, &task).await? {
        report.impediments.push(blocker.reason);
    }
    for result in [
        options.validate_directive(Some(&task)),
        options.validate_workspace_name(&task),
    ] {
        if let Err(error) = result {
            report.impediments.push(error.to_string());
        }
    }
    if options.stack_on.is_some() {
        report
            .unavailable
            .push("Stack placement requires preparation; it has not been performed".into());
    }
    let wave = super::owning_wave(store, &task).await?;
    let checkout = task
        .worktree
        .as_deref()
        .unwrap_or_else(|| Path::new(wave.repo()));
    if task.worktree.is_some() && !checkout.exists() {
        anyhow::bail!(
            "Recorded checkout definitions are unavailable at {}",
            checkout.display()
        );
    }
    let choice = match select_task_run(store, &task, checkout, options.flow.as_deref()) {
        Ok(choice) => choice,
        Err(error) => {
            report.impediments.push(error.to_string());
            return Ok(());
        }
    };
    report.action = Some(match choice {
        TaskRunSelection::Flow(flow) => TaskRunAction::Flow { flow },
        TaskRunSelection::Edge {
            workflow,
            index,
            take_up,
        } => {
            let edge = &workflow.definition.edges[index as usize];
            if edge.to == crate::engine::workflow::END && !options.end.force {
                if let Some(conflict) = super::planning_conflict(store, &task).await? {
                    report.impediments.push(conflict);
                }
            }
            if let Some(flow) = &edge.flow {
                if let Err(error) = super::load_task_flow(checkout, flow) {
                    report.impediments.push(error.to_string());
                }
            } else {
                report.unavailable.push("Completion will reconcile PR evidence before moving to end; no reconciliation was performed".into());
            }
            TaskRunAction::Edge {
                workflow: workflow.definition.name,
                take_up,
                from: edge.from.clone(),
                to: edge.to.clone(),
                flow: edge.flow.clone(),
            }
        }
    });
    report
        .unavailable
        .push("Provider/account checks and post-preparation admission have not run".into());
    Ok(())
}

/// An explicit position change, not an edge execution or review approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMoveAction {
    pub workflow: Option<String>,
    pub from: Option<crate::ops::workflow::WorkflowPosition>,
    pub to: String,
    pub reason: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMoveExplanation {
    pub resolution: ContextExplanation,
    pub action: Option<TaskMoveAction>,
    pub impediments: Vec<String>,
    pub unavailable: Vec<String>,
}

impl TaskMoveExplanation {
    pub fn render(&self) -> String {
        let mut lines = vec![self.resolution.render()];
        if let Some(action) = &self.action {
            lines.push(format!(
                "Intended action: put Task at {}{}; {}",
                action.to,
                action
                    .workflow
                    .as_ref()
                    .map(|name| format!(" of Workflow {name}"))
                    .unwrap_or_default(),
                if action.to == crate::engine::workflow::END {
                    "complete and attempt checkout cleanup after reconciliation"
                } else {
                    "run nothing; leave existing Processes alone"
                },
            ));
            if let Some(reason) = &action.reason {
                lines.push(format!("Reason: {reason}"));
            }
            if action.force {
                lines.push(
                    "Force: bypass only the planning-completion conflict, not completion gates"
                        .into(),
                );
            }
        } else {
            lines.push("Intended action: unavailable".into());
        }
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
        lines.push("Observation only; execution re-reads the Workflow and completion evidence. Nothing was executed.".into());
        lines.join("\n")
    }
}

pub async fn explain_task_move(
    store: &SharedStore,
    cwd: &Path,
    selection: WorkSelection<'_>,
    node: &str,
    reason: Option<&str>,
    end: &super::EndOptions,
) -> TaskMoveExplanation {
    let resolution = crate::ops::context::explain_context(store, cwd, selection, None, None).await;
    let mut report = TaskMoveExplanation {
        resolution,
        action: None,
        impediments: Vec::new(),
        unavailable: Vec::new(),
    };
    if let Err(error) = read_task_move(store, node, reason, end, &mut report).await {
        report.unavailable.push(error.to_string());
    }
    report
}

async fn read_task_move(
    store: &SharedStore,
    node: &str,
    reason: Option<&str>,
    end: &super::EndOptions,
    report: &mut TaskMoveExplanation,
) -> anyhow::Result<()> {
    let task = read_local_task(store, &mut report.resolution).await?;
    let workflow = store.sqlite.workflow(&task.id)?;
    if let Err(error) =
        super::validate_workflow_move(workflow.as_ref(), &task.plan.identifier, node, end)
    {
        report.impediments.push(error.to_string());
        return Ok(());
    }
    report.action = Some(TaskMoveAction {
        workflow: workflow
            .as_ref()
            .map(|workflow| workflow.definition.name.clone()),
        from: workflow.map(|workflow| workflow.position),
        to: node.into(),
        reason: reason
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
            .map(str::to_owned),
        force: end.force,
    });
    if node == crate::engine::workflow::END {
        report.unavailable.push("Completion requires fresh reconciliation, PR gates and checkout cleanup checks; none were performed. This is not completion permission".into());
    }
    Ok(())
}

// Workflow position stays on the execution Machine, never in portable planning.
async fn read_local_task(
    store: &SharedStore,
    resolution: &mut ContextExplanation,
) -> anyhow::Result<crate::work::task::Task> {
    let id = match &resolution.task {
        ContextFact::Bound { value, .. } => crate::durable::TaskId::parse(value)?,
        ContextFact::Unbound => {
            anyhow::bail!("No Task selected; name a Task or run from its checkout")
        }
        ContextFact::Unavailable { reason } => anyhow::bail!("{reason}"),
    };
    let task = store
        .get_task(&id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Task {id} is missing"))?;
    let route = match store.task_execution_route(&id).await {
        Ok(route) => route,
        Err(error) => {
            resolution.execution_machine = ContextFact::Unavailable {
                reason: error.to_string(),
            };
            return Err(error.into());
        }
    };
    resolution.execution_machine = ContextFact::Bound {
        value: route.machine_id.to_string(),
        source: match route.source {
            TaskExecutionSource::RecordedCheckout => "recorded_checkout",
            TaskExecutionSource::EffectiveDelegation => "effective_delegation",
        }
        .into(),
    };
    if route.machine_id != store.local_machine().await?.id {
        anyhow::bail!("Execution state belongs to Machine {}; no peer read or remote preparation was performed", route.machine_id);
    }
    Ok(task)
}
