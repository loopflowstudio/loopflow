//! The one Task row Task reads print: `lf task list`, `lf task status` and the
//! Task rows of `lf wave show` and `lf wave status`. Each fact appears once.
//! Desktop's presentation of the same Tasks (sections, conditions, controls)
//! is `lf desktop`'s, from `RoadmapTask`.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::waves::RoadmapTask;
use crate::durable::{GraphProcessDetail, TaskState};
use crate::store::SharedStore;
use crate::task_work::TaskSession;
use crate::work::task::comment::TaskComments;
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
    pub workflow: Option<TaskRowWorkflow>,
    /// Its published pull request.
    pub pr: Option<TaskRowPr>,
    /// The command that moves it on; `null` while it moves by itself or is closed.
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRowWorkflow {
    pub name: String,
    /// The stage it stands at; `null` while a route runs.
    pub stage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRowPr {
    pub number: u32,
    pub url: String,
    pub phase: PrPhase,
}

/// `lf task status`: one Task with its Sessions, Graph processes and comments.
/// Wire type; no defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    pub task: TaskRow,
    pub sessions: Vec<TaskSession>,
    pub graph_processes: Vec<GraphProcessDetail>,
    pub comments: Option<TaskComments>,
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
            reason: task.next_move.reason,
            workflow: task.workflow_name.map(|name| TaskRowWorkflow {
                name,
                stage: task.stage,
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
