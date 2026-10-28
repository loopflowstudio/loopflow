//! The one Task row Task reads print: `lf task list`, `lf task status` and the
//! Task rows of `lf wave show` and `lf wave status`. Each fact appears once.
//! Desktop's presentation of the same Tasks (sections, conditions, controls)
//! is `lf desktop`'s, from `RoadmapTask`.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::session::SessionFacts;
use super::waves::RoadmapTask;
use crate::durable::{GraphProcessDetail, TaskState};
use crate::session::GraphProcessSummaryState;
use crate::store::rows::unix_to_datetime;
use crate::store::SharedStore;
use crate::work::task::comment::{TaskComment, TaskComments};
use crate::work::task::PrPhase;

/// One Task. Wire type; no defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRow {
    pub id: String,
    pub identifier: String,
    pub title: String,
    /// Its Wave's name; `null` outside any Wave.
    pub wave: Option<String>,
    /// Its Project's name; `null` outside any Project.
    pub project: Option<String>,
    pub state: TaskState,
    /// Why it is in that state, or what it waits on.
    pub reason: String,
    pub graph: Option<TaskRowGraph>,
    /// Its published pull request.
    pub pr: Option<TaskRowPr>,
    /// The command that moves it on; `null` while it moves by itself or is closed.
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRowGraph {
    pub name: String,
    /// The step it stands at (`start`, `end` or an interactive step); `null` while a run carries it.
    pub step: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRowPr {
    pub number: u32,
    pub url: String,
    pub phase: PrPhase,
}

/// `lf task status`: one Task with its Sessions, Graph processes and comments.
/// The enclosing Task names each one's Task and Wave. Wire type; no defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetail {
    pub task: TaskRow,
    /// As `lf session list` lists them, joined by `id`.
    pub sessions: Vec<SessionFacts>,
    pub graph_processes: Vec<TaskGraphRow>,
    pub comments: Option<TaskThread>,
}

/// One of the Task's Graph processes; `lf graph show <id> --processes` draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraphRow {
    /// The Graph process.
    pub id: String,
    pub name: String,
    pub state: GraphProcessSummaryState,
    /// The current or last step's position, "implement · pass 2"; `null` before a first step.
    pub step: Option<String>,
    /// When its latest step started, or its Graph process exited.
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl TaskGraphRow {
    pub(crate) fn of(detail: GraphProcessDetail) -> Self {
        let summary = detail.entry.summary;
        Self {
            step: detail.steps.last().map(|step| step.position()),
            updated_at: unix_to_datetime(summary.updated_at),
            id: summary.id,
            name: summary.name,
            state: summary.state,
        }
    }
}

/// The Task's comment thread, as `lf task comment` keeps it, without restating
/// the Task's identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskThread {
    pub comments: Vec<TaskComment>,
    pub pending_sync: Vec<String>,
    /// Losing local comment bodies; the thread contains the adopted Linear values.
    pub conflicts: std::collections::BTreeMap<String, String>,
    pub refresh_error: Option<String>,
}

impl From<TaskComments> for TaskThread {
    fn from(thread: TaskComments) -> Self {
        Self {
            comments: thread.comments,
            pending_sync: thread.pending_sync,
            conflicts: thread.conflicts,
            refresh_error: thread.refresh_error,
        }
    }
}

/// Wave and Project names, read once per reading.
#[derive(Debug, Default)]
pub(crate) struct TaskNames {
    waves: HashMap<String, String>,
    projects: HashMap<String, String>,
}

impl TaskNames {
    /// Without a store there are no Tasks to name.
    pub(crate) async fn read(store: Option<&SharedStore>) -> Result<Self> {
        let Some(store) = store else {
            return Ok(Self::default());
        };
        let waves = store
            .list_waves(None)
            .await
            .map_err(|err| anyhow!("failed to read wave registry: {err}"))?;
        let projects = store.list_projects(None).await?;
        Ok(Self {
            waves: waves
                .iter()
                .map(|wave| (wave.id().to_string(), wave.name().to_owned()))
                .collect(),
            projects: projects
                .into_iter()
                .map(|project| (project.id.to_string(), project.name))
                .collect(),
        })
    }

    pub(crate) fn row(&self, task: RoadmapTask) -> TaskRow {
        let state = super::waves::task_state(&task);
        let next = next_command(&task, state);
        let summary = task.task;
        TaskRow {
            wave: summary
                .wave_id
                .as_ref()
                .and_then(|id| self.waves.get(id).cloned()),
            project: summary
                .project_id
                .as_ref()
                .and_then(|id| self.projects.get(id).cloned()),
            id: summary.id,
            identifier: summary.identifier,
            title: summary.name,
            state,
            // An approved Task waits on its ship, whatever its next move says.
            reason: task
                .shipping
                .map_or(task.next_move.reason, |shipping| shipping.reason),
            graph: task.graph_name.map(|name| TaskRowGraph {
                name,
                step: task.step,
            }),
            pr: task.pr.and_then(|pr| {
                let github = pr.publication?.github?;
                Some(TaskRowPr {
                    number: github.number,
                    url: github.url,
                    phase: pr.phase,
                })
            }),
            next,
        }
    }
}

fn next_command(task: &RoadmapTask, state: TaskState) -> Option<String> {
    let id = &task.task.identifier;
    match state {
        TaskState::Deferred => Some(format!("lf task resume {id}")),
        TaskState::Done | TaskState::Abandoned | TaskState::Moving | TaskState::Shipping => None,
        _ if task
            .pr
            .as_ref()
            .is_some_and(|pr| pr.phase == PrPhase::Merged) =>
        {
            Some(format!("lf task complete {id}"))
        }
        _ if task.run_control.unavailable.is_none() => Some(format!("lf task run {id}")),
        _ => None,
    }
}
