//! `lf task status`'s Task row: each fact once, with the Task's Sessions, Graph
//! processes and comments. Every list read (`lf task list`, `lf wave show`,
//! `lf wave status`) prints `RoadmapTask` records, the same record the server
//! sends.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::session::SessionFacts;
use super::waves::RoadmapTask;
use loopflow_execution::durable::{GraphProcessDetail, TaskState};
use loopflow_execution::session::GraphProcessSummaryState;
use loopflow_execution::store::SharedStore;
use loopflow_model::comment::{TaskComment, TaskComments};
use loopflow_storage::rows::unix_to_datetime;
use loopflow_storage::work::task::PrPhase;

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
    /// Which of `sessions` is the Task's conversation, the one the server
    /// starts again when it dies; `null` before the Task has one.
    #[serde(deserialize_with = "Option::deserialize")]
    pub conversation: Option<loopflow_model::id::LfSessionId>,
    /// The Graph the Task took up and where it stands on it, as the workflow
    /// line reads it; `null` before it takes one up.
    #[serde(deserialize_with = "Option::deserialize")]
    pub graph: Option<loopflow_execution::task_graph::TaskGraph>,
    pub graph_processes: Vec<TaskGraphRow>,
    pub comments: Option<TaskThread>,
    /// What the Task's run came to, written when it completed or was
    /// abandoned; `null` while it never has.
    #[serde(deserialize_with = "Option::deserialize")]
    pub summary: Option<loopflow_storage::sqlite::task_summaries::TaskSummary>,
    /// The Task's auto setting: on or off, and who set it when; `null`
    /// when nobody has.
    #[serde(deserialize_with = "Option::deserialize")]
    pub auto: Option<loopflow_storage::sqlite::Auto>,
    /// The Task this one is stacked on; `null` when it builds on the default branch.
    #[serde(deserialize_with = "Option::deserialize")]
    pub stack_parent: Option<loopflow_execution::ops::task::TaskStackParent>,
}

/// What `lf task status` adds to a Task's state line: " · auto: on by Maya, Nov 18 09:04".
pub fn auto_phrase(auto: &loopflow_storage::sqlite::Auto) -> String {
    use loopflow_storage::process::Starter;
    // A mark an earlier lf wrote may name nobody.
    let by = match &auto.by {
        Starter::Person { name: None } => String::new(),
        by => format!(" by {by}"),
    };
    let state = auto.state.as_str();
    let at = loopflow_execution::output::format_time(auto.at);
    format!(" · auto: {state}{by}, {at}")
}

/// One of the Task's Graph runs; `lf graph show <id>` draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraphRow {
    /// The Graph run.
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

/// The Task's comment thread, as `lf steer` and `lf task report` keep it, without restating
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
        let state = task.lifecycle;
        let next = next_command(&task);
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

fn next_command(task: &RoadmapTask) -> Option<String> {
    let id = &task.task.identifier;
    match task.lifecycle {
        TaskState::Done | TaskState::Abandoned | TaskState::Moving => None,
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
