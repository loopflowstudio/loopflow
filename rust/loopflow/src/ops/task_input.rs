//! Task context and live input for the ordinary skill command.
use crate::child::ChildRef;
use crate::durable::WorkRef;
use crate::harness::Harness;
use crate::store::SharedStore;
use crate::work::project::Project;
use crate::work::task::Task;
use anyhow::{anyhow, Result};
use std::io::BufRead;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Message and control cursors captured together before the skill starts.
pub(crate) struct TaskSeed {
    pub task: Task,
    pub message: String,
    pub interrupt: i64,
}

/// Read a Task snapshot without refreshing providers or advancing a control cursor.
pub(crate) async fn read_seed(store: &SharedStore, task: &Task, wave: &str) -> Result<TaskSeed> {
    let interrupt = store
        .latest_interrupt_id(&WorkRef::Task(task.id.clone()))
        .await?;
    let pr = store.active_task_pr(&task.id).await?;
    let project = store
        .get_project(&task.project_id)
        .await?
        .ok_or_else(|| anyhow!("Task Project is missing"))?;
    let message = format!(
        "{}\n\n{}",
        task_seed(task, &project, pr.as_ref(), wave),
        crate::ops::task::task_workspace_context(task)?
    );
    Ok(TaskSeed {
        task: task.clone(),
        message,
        interrupt,
    })
}

/// A command's live Task input survives its provider's transient retries.
#[derive(Clone)]
pub struct TaskInput {
    controls: Arc<tokio::sync::Mutex<Controls>>,
    store: SharedStore,
    task: Task,
}

impl std::fmt::Debug for TaskInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TaskInput")
    }
}

struct Controls {
    interrupt: i64,
    receiver: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
    next_interrupt: Instant,
}

impl TaskInput {
    pub(crate) fn new(store: SharedStore, seed: TaskSeed) -> Self {
        Self {
            store,
            task: seed.task,
            controls: Arc::new(tokio::sync::Mutex::new(Controls {
                interrupt: seed.interrupt,
                receiver: None,
                next_interrupt: Instant::now(),
            })),
        }
    }

    pub(crate) async fn poll(&self, harness: &mut dyn Harness) -> Result<()> {
        let mut controls = self.controls.lock().await;
        let store = &self.store;
        let task = &self.task;
        let Controls {
            interrupt,
            receiver,
            next_interrupt,
        } = &mut *controls;
        let receiver = receiver.get_or_insert_with(|| {
            let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
            std::thread::spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
            println!(
                "task {}> attached; /status, /interrupt, /detach, or type a message/instruction",
                task.identifier
            );
            receiver
        });
        while let Ok(line) = receiver.try_recv() {
            handle_attachment(store, task, harness, line).await?;
        }
        if Instant::now() >= *next_interrupt {
            crate::ops::child::observe_interrupt(
                store,
                &WorkRef::Task(task.id.clone()),
                harness,
                interrupt,
            )
            .await;
            *next_interrupt = Instant::now() + Duration::from_secs(5);
        }
        Ok(())
    }
}

async fn handle_attachment(
    store: &SharedStore,
    task: &Task,
    harness: &mut dyn Harness,
    line: String,
) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    if line == "/status" {
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await?;
        println!("{}  {:?}", task.identifier, store.work_status(&work).await?);
        return Ok(());
    }
    if line == "/detach" {
        crate::transport::detach_viewers()?;
        return Ok(());
    }
    if line == "/interrupt" {
        harness.interrupt().await?;
        println!("interrupted active provider turn");
    } else {
        let comment_id = crate::ops::task::append_task_comment(store, task, line, true)?;
        println!("saved comment {comment_id}");
    }
    Ok(())
}

pub(crate) fn task_seed(
    task: &Task,
    project: &Project,
    pr: Option<&crate::work::task::TaskPr>,
    wave_name: &str,
) -> String {
    let context = crate::ops::render_task_context(task, project, pr, wave_name);
    format!(
        "{context}\n\nA Task has zero or one pull request. Merge leaves the Task open until completion. Optional later work uses ordinary Task creation; no follow-up disposition is required. Dependent pull requests belong to separate stacked Tasks."
    )
}
