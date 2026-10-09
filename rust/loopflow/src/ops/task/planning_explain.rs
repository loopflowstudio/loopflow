//! Task mutation previews validate local input, without acquisition, saving or delivery.
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ops::context::{ContextExplanation, ContextFact};
use crate::ops::run::WorkSelection;
use crate::pm::PmItemUpdate;
use crate::store::SharedStore;

#[derive(Debug)]
pub enum TaskPlanningRequest<'a> {
    Create {
        title: Option<&'a str>,
        notes: Option<&'a str>,
    },
    Edit(PmItemUpdate),
    Refile {
        wave: &'a str,
    },
    Save {
        path: &'a str,
        revision: &'a str,
        content: &'a str,
    },
    Comment {
        message: Option<&'a str>,
        steer: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskPlanningAction {
    Create {
        title: String,
        description: String,
        project: Option<String>,
    },
    Edit {
        revision: u64,
        fields: Vec<String>,
    },
    Refile {
        wave: String,
        project: String,
        previous_project: String,
    },
    Save {
        path: String,
        revision: String,
        draft_bytes: usize,
    },
    Comment {
        message: Option<String>,
        steer: bool,
        refresh: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPlanningExplanation {
    pub resolution: ContextExplanation,
    pub action: Option<TaskPlanningAction>,
    pub effects: Vec<String>,
    pub impediments: Vec<String>,
    pub unavailable: Vec<String>,
}

impl TaskPlanningExplanation {
    pub fn unavailable(error: impl ToString) -> Self {
        Self {
            resolution: ContextExplanation::unavailable(error.to_string()),
            action: None,
            effects: Vec::new(),
            impediments: Vec::new(),
            unavailable: vec![error.to_string()],
        }
    }

    pub fn render(&self) -> String {
        let action = match &self.action {
            Some(TaskPlanningAction::Create { title, project, .. }) => format!(
                "create Task {title:?} in Project {}",
                project.as_deref().unwrap_or("unavailable")
            ),
            Some(TaskPlanningAction::Edit { revision, fields }) => {
                format!("edit {} at planning revision {revision}", fields.join(", "))
            }
            Some(TaskPlanningAction::Comment {
                message: Some(message),
                steer,
                ..
            }) => format!(
                "append {} {message:?}",
                if *steer { "direction" } else { "comment" }
            ),
            Some(TaskPlanningAction::Comment { message: None, .. }) => "read Task comments".into(),
            Some(TaskPlanningAction::Refile { wave, project, .. }) => {
                format!("refile Task to Wave {wave}, Project {project}")
            }
            Some(TaskPlanningAction::Save {
                path,
                revision,
                draft_bytes,
            }) => format!("save {path:?} at revision {revision} with {draft_bytes} draft bytes"),
            None => "unavailable".into(),
        };
        let mut lines = vec![
            self.resolution.render(),
            format!("Intended action: {action}"),
        ];
        for (label, values) in [
            ("Effect", &self.effects),
            ("Impediment", &self.impediments),
            ("Unavailable", &self.unavailable),
        ] {
            lines.extend(values.iter().map(|value| format!("{label}: {value}")));
        }
        lines.push("Observation only; execution re-reads planning. Nothing was saved, acquired or synchronized.".into());
        lines.join("\n")
    }
}

pub async fn explain_task_planning(
    store: &SharedStore,
    cwd: &Path,
    selection: WorkSelection<'_>,
    request: &TaskPlanningRequest<'_>,
) -> TaskPlanningExplanation {
    let resolution = crate::ops::context::explain_context(store, cwd, selection, None, None).await;
    let mut report = TaskPlanningExplanation {
        resolution,
        action: None,
        effects: Vec::new(),
        impediments: Vec::new(),
        unavailable: Vec::new(),
    };
    if let Err(error) = read_planning(store, cwd, selection, request, &mut report).await {
        report.unavailable.push(error.to_string());
    }
    report
}

async fn read_planning(
    store: &SharedStore,
    cwd: &Path,
    selection: WorkSelection<'_>,
    request: &TaskPlanningRequest<'_>,
    report: &mut TaskPlanningExplanation,
) -> anyhow::Result<()> {
    match request {
        TaskPlanningRequest::Create { title, notes } => {
            read_creation(store, cwd, selection.wave, *title, *notes, report).await?;
        }
        TaskPlanningRequest::Edit(update) => {
            if let Err(error) = super::validate_task_edit_input(update) {
                report.impediments.push(error.to_string());
                return Ok(());
            }
            let task = read_saved_task(store, cwd, selection.wave, &report.resolution).await?;
            if let Err(error) =
                store
                    .sqlite
                    .validate_task_edit(&task.id, task.plan.revision, update)
            {
                report.impediments.push(error.to_string());
                return Ok(());
            }
            let fields = [
                ("title", update.name.is_some()),
                ("description", update.description.is_some()),
                ("rank", update.rank.is_some()),
                ("assignee", update.assignee.is_some()),
            ]
            .into_iter()
            .filter(|(_, present)| *present)
            .map(|(field, _)| field.to_owned())
            .collect();
            report.action = Some(TaskPlanningAction::Edit {
                revision: task.plan.revision,
                fields,
            });
            report.effects.push("Save changed fields and pending planning mutations locally; attempt configured synchronization. Workflow, checkout and Processes stay unchanged".into());
        }
        TaskPlanningRequest::Refile { wave } => {
            let task = read_saved_task(store, cwd, selection.wave, &report.resolution).await?;
            let destination = crate::work::wave::context::resolve_managed_wave(
                Some(store),
                Some(cwd),
                Some(wave),
                None,
            )
            .await
            .map_err(|error| {
                anyhow::anyhow!("{error}; destination registration/acquisition was not performed")
            })?;
            let current = crate::ops::project::current_project(store, &destination)?;
            let project = store
                .get_project_by_project(&current.id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("destination Project is unavailable"))?;
            report.action = Some(TaskPlanningAction::Refile {
                wave: destination.id().to_string(),
                project: project.id.to_string(),
                previous_project: task.project_id.to_string(),
            });
            for wave in [super::owning_wave(store, &task).await?, destination] {
                if let Err(error) = crate::ops::pm::require_planning_home(store, &wave).await {
                    report.impediments.push(error.to_string());
                }
            }
            if let Err(error) =
                store
                    .sqlite
                    .validate_task_refile(&task.id, &task.project_id, &project.id)
            {
                report.impediments.push(error.to_string());
            }
            report.effects.push(if task.project_id == project.id {
                "Membership is unchanged; attempt configured planning synchronization".into()
            } else {
                "Save destination Project membership and its pending planning mutation locally; attempt configured synchronization. Recorded work prevents refiling; no execution is transferred".into()
            });
        }
        TaskPlanningRequest::Save {
            path,
            revision,
            content,
        } => {
            let task = read_saved_task(store, cwd, selection.wave, &report.resolution).await?;
            let validation =
                super::file_context(&store.sqlite, task.id.as_str()).and_then(|checkout| {
                    super::file_save::validate_save(
                        super::TaskWorkspace::from(&checkout),
                        path,
                        revision,
                        content,
                    )
                });
            let path = match validation {
                Ok(path) => path,
                Err(error) => {
                    report.impediments.push(error.to_string());
                    (*path).into()
                }
            };
            report.action = Some(TaskPlanningAction::Save {
                path,
                revision: (*revision).into(),
                draft_bytes: content.len(),
            });
            report.effects.push("Retain the submitted draft, receipt and displaced file under Git metadata, then atomically exchange the file. Planning and execution stay unchanged".into());
            report.unavailable.push("File permissions at publication and concurrent filesystem changes are not reserved; execution revalidates before exchange".into());
        }
        TaskPlanningRequest::Comment { message, steer } => {
            if let Some(message) = message {
                if let Err(error) = super::validate_task_comment_input(message) {
                    report.impediments.push(error.to_string());
                    return Ok(());
                }
            }
            let task = read_saved_task(store, cwd, selection.wave, &report.resolution).await?;
            if message.is_some() {
                if let Err(error) = store.sqlite.require_task_not_deleted(&task.id) {
                    report.impediments.push(error.to_string());
                    return Ok(());
                }
                report.effects.push("Append one local comment with stable identity and authored provenance; attempt configured synchronization. Execution stays unchanged".into());
                report.unavailable.push("Comment identity and caller/participant provenance are captured only when saving".into());
            } else if task.plan.linear_id.is_some() {
                report.effects.push("Refresh Linear comments with a bounded wait, then show the saved thread even if refresh fails".into());
            } else {
                report
                    .effects
                    .push("Read the saved local comment thread".into());
            }
            report.action = Some(TaskPlanningAction::Comment {
                message: message.map(|value| value.trim().to_owned()),
                steer: *steer,
                refresh: message.is_none() && task.plan.linear_id.is_some(),
            });
        }
    }
    report.unavailable.push("Provider freshness, synchronization delivery and concurrent saves are not observed or reserved".into());
    Ok(())
}

async fn read_saved_task(
    store: &SharedStore,
    cwd: &Path,
    wave: Option<&str>,
    resolution: &ContextExplanation,
) -> anyhow::Result<crate::work::task::Task> {
    let id = match &resolution.task {
        ContextFact::Bound { value, .. } => crate::durable::TaskId::parse(value)?,
        ContextFact::Unavailable { reason } => {
            anyhow::bail!("{reason}; no planning acquisition was performed")
        }
        ContextFact::Unbound => {
            anyhow::bail!("No saved Task selected; no planning acquisition was performed")
        }
    };
    let task = store
        .get_task(&id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Task {id} is missing"))?;
    crate::ops::pm::validate_saved_task_scope(store, cwd, wave, &task).await?;
    Ok(task)
}

async fn read_creation(
    store: &SharedStore,
    cwd: &Path,
    wave_selector: Option<&str>,
    title: Option<&str>,
    notes: Option<&str>,
    report: &mut TaskPlanningExplanation,
) -> anyhow::Result<()> {
    // A new Task has no execution location or planning observation yet.
    report.resolution.task = ContextFact::Unbound;
    report.resolution.wave = ContextFact::Unavailable {
        reason: "Creation Wave selection has not been validated".into(),
    };
    report.resolution.checkout = ContextFact::Unbound;
    report.resolution.execution_machine = ContextFact::Unbound;
    report.resolution.planning_observed_at = None;
    let input = match super::resolve_task_create_input(title, notes) {
        Ok(input) => input,
        Err(error) => {
            report.impediments.push(error.to_string());
            return Ok(());
        }
    };
    report.action = Some(TaskPlanningAction::Create {
        title: input.title,
        description: input.report,
        project: None,
    });
    report.effects.push("Save a new unstarted Task and pending planning mutations locally; attempt configured synchronization without allocating execution".into());
    let main = crate::engine::worktrees::main_repo_root(cwd)?;
    let selected = match super::select_task_creation_wave(store, &main, wave_selector).await {
        Ok(selected) => selected,
        Err(error) => {
            report.resolution.wave = ContextFact::Unavailable {
                reason: error.to_string(),
            };
            anyhow::bail!("{error}; Wave registration/acquisition was not performed");
        }
    };
    let (wave, source) = match selected {
        Some(wave) => (
            wave,
            if wave_selector.is_some() {
                "explicit"
            } else {
                "inherited_declaration"
            },
        ),
        None => {
            let locator = crate::work::wave::WaveLocator::discover(&main, "inbox")?;
            match store.get_wave_at(&locator).await? {
                Some(wave) => (wave, "default_inbox"),
                None => {
                    report.resolution.wave = ContextFact::Unbound;
                    report.effects.push(
                        "Ensure the repository's inbox Wave and Project before creating the Task"
                            .into(),
                    );
                    report.unavailable.push(
                        "Inbox Wave/Project identity requires initialization; it was not performed"
                            .into(),
                    );
                    return Ok(());
                }
            }
        }
    };
    report.resolution = crate::ops::context::explain_context(
        store,
        cwd,
        WorkSelection {
            task: None,
            wave: Some(wave.id().as_str()),
        },
        None,
        None,
    )
    .await;
    report.resolution.checkout = ContextFact::Unbound;
    report.resolution.execution_machine = ContextFact::Unbound;
    report.resolution.wave = ContextFact::Bound {
        value: wave.id().to_string(),
        source: source.into(),
    };
    let project = match crate::ops::project::current_project(store, &wave) {
        Ok(project) => project,
        Err(error) => {
            report.impediments.push(error.to_string());
            return Ok(());
        }
    };
    let project = store
        .get_project_by_project(&project.id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("selected Project is missing"))?;
    if let Some(TaskPlanningAction::Create {
        project: selected, ..
    }) = &mut report.action
    {
        *selected = Some(project.id.to_string());
    }
    Ok(())
}
