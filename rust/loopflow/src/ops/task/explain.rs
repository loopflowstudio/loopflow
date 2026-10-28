//! Invocation readings share the Task run's choice and launch admission, never preparation.
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ops::context::{ContextExplanation, ContextFact};
use crate::ops::run::WorkSelection;
use crate::store::SharedStore;

use super::{select_task_run, TaskProcessOptions, TaskRunSelection};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskRunAction {
    /// Run `graph` from the step `from` until the interactive step
    /// `stops_at`, or to the end when `None`.
    Run {
        graph: String,
        take_up: bool,
        from: String,
        stops_at: Option<String>,
    },
    /// Put the Task at the interactive step `step`, running nothing.
    Wait {
        graph: String,
        take_up: bool,
        step: String,
    },
    /// Nothing remains: the Task reaches `end`, requesting completion.
    End { graph: String, take_up: bool },
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
        let take = |graph: &str, take_up: bool| {
            format!("{}graph {graph}", if take_up { "take up " } else { "" })
        };
        let action = match &self.action {
            Some(TaskRunAction::Run {
                graph,
                take_up,
                from,
                stops_at,
            }) => format!(
                "{}: run from {from} {}",
                take(graph, *take_up),
                match stops_at {
                    Some(step) => format!("until {step}"),
                    None => "to the end".into(),
                }
            ),
            Some(TaskRunAction::Wait {
                graph,
                take_up,
                step,
            }) => format!("{}: wait at {step}; run nothing", take(graph, *take_up)),
            Some(TaskRunAction::End { graph, take_up }) => {
                format!("{}: nothing remains; reach end", take(graph, *take_up))
            }
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
    through: bool,
) -> TaskRunExplanation {
    let resolution = crate::ops::context::explain_context(store, cwd, selection, None, None).await;
    let mut report = TaskRunExplanation {
        resolution,
        action: None,
        impediments: Vec::new(),
        unavailable: Vec::new(),
    };
    if let Err(error) = read_task_run(store, options, through, &mut report).await {
        report.unavailable.push(error.to_string());
    }
    report
}

async fn read_task_run(
    store: &SharedStore,
    options: &TaskProcessOptions,
    through: bool,
    report: &mut TaskRunExplanation,
) -> anyhow::Result<()> {
    let task = read_local_task(store, &mut report.resolution).await?;
    if task.worktree.is_none() {
        report.unavailable.push("Future checkout is not observed; definitions are read from the local repository before preparation".into());
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
    let repo = super::repository_for_task(store, &task)?;
    let checkout = task.worktree.as_deref().unwrap_or(&repo);
    if task.worktree.is_some() && !checkout.exists() {
        anyhow::bail!(
            "Recorded checkout definitions are unavailable at {}",
            checkout.display()
        );
    }
    let choice = match select_task_run(
        store,
        &task,
        checkout,
        options.graph.as_deref(),
        options.from.as_deref(),
    ) {
        Ok(choice) => choice,
        Err(error) => {
            report.impediments.push(error.to_string());
            return Ok(());
        }
    };
    let TaskRunSelection {
        graph,
        take_up,
        start,
        ..
    } = choice;
    let name = graph.name.clone();
    report.action = Some(match start {
        None => {
            report.unavailable.push("Arrival requests planning completion; PR gates and eligible checkout cleanup are evaluated during settlement, not by this preview".into());
            TaskRunAction::End {
                graph: name,
                take_up,
            }
        }
        Some(place) => match graph.interactive_at(&place).filter(|_| !through) {
            Some(step) => TaskRunAction::Wait {
                graph: name,
                take_up,
                step: step.into(),
            },
            None => TaskRunAction::Run {
                from: graph.step_name(&place).into(),
                stops_at: (!through)
                    .then(|| graph.next_interactive(&place))
                    .flatten()
                    .map(str::to_owned),
                graph: name,
                take_up,
            },
        },
    });
    report
        .unavailable
        .push("Provider/account checks and post-preparation admission have not run".into());
    Ok(())
}

// A Task's Graph position stays on the execution Machine, never in portable planning.
pub(super) async fn read_local_task(
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
    let route = crate::ops::task_location::explain(&store.sqlite, &id, resolution)?;
    let local_machine = store.local_machine().await?.id;
    if route.machine_id != local_machine {
        anyhow::bail!(
            "Execution state belongs to Machine {}; no remote preparation was performed",
            route.machine_id
        );
    }
    Ok(task)
}
