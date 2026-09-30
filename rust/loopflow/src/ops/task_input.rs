//! Task context and live input for the ordinary skill command.
use crate::child::ChildRef;
use crate::durable::{FlowSession, Steer, WorkRef};
use crate::harness::Harness;
use crate::planning::ProjectPlan;
use crate::store::SharedStore;
use crate::work::task::Task;
use anyhow::{anyhow, Result};
use std::io::BufRead;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Message and control cursors captured together before the skill starts.
pub(crate) struct TaskSeed {
    pub task: Task,
    pub message: String,
    pub steer: i64,
    pub interrupt: i64,
}

pub(crate) async fn prepare(
    store: &SharedStore,
    task: &Task,
    wave: &str,
    flow: &FlowSession,
) -> Result<TaskSeed> {
    let steers = store.task_steers(&task.id).await?;
    let interrupt = store
        .latest_interrupt_id(&WorkRef::Task(task.id.clone()))
        .await?;
    let pr = store
        .active_task_pr(&task.id)
        .await?
        .ok_or_else(|| anyhow!("Task {} has no active PR", task.id))?;
    let project = store
        .get_project(&task.project_id)
        .await?
        .ok_or_else(|| anyhow!("Task Project is missing"))?;
    let mut message = format!(
        "{}\n\n{}",
        task_seed(task, &project.plan, &pr, wave, &steers),
        crate::ops::task::task_workspace_context(task, &pr)?
    );
    let skill = crate::engine::current_skill(&flow.invocation.steps, &flow.cursor)
        .ok_or_else(|| anyhow!("Task boundary has no skill"))?;
    if let Some(repeat) = &skill.policy.repeat {
        let edge = skill
            .policy
            .id
            .as_deref()
            .expect("repeat occurrence has an id");
        let traversals = flow
            .cursor
            .leaf()
            .progress
            .repeats
            .get(edge)
            .copied()
            .unwrap_or(0);
        message.push_str(&format!("\n\nDecision occurrence {edge}: pass {}. The backward edge returns to {}. Compare the preceding pass's intended progress with its observed results; new evidence counts as progress. Missing prior evidence is an evidence gap, not proof of no progress.", u64::from(traversals) + 1, repeat.from));
    }
    Ok(TaskSeed {
        task: task.clone(),
        message,
        steer: steers.last().map_or(0, |steer| steer.id),
        interrupt,
    })
}

/// A command's live Task input survives its provider's transient retries.
#[derive(Clone)]
pub struct TaskInput(Arc<tokio::sync::Mutex<Controls>>);

impl std::fmt::Debug for TaskInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TaskInput")
    }
}

struct Controls {
    store: SharedStore,
    task: Task,
    steer: i64,
    interrupt: i64,
    receiver: tokio::sync::mpsc::UnboundedReceiver<String>,
    next_steer: Instant,
}

impl TaskInput {
    pub(crate) fn new(store: SharedStore, seed: TaskSeed) -> Self {
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
            seed.task.plan.identifier
        );
        Self(Arc::new(tokio::sync::Mutex::new(Controls {
            store,
            task: seed.task,
            steer: seed.steer,
            interrupt: seed.interrupt,
            receiver,
            next_steer: Instant::now(),
        })))
    }

    pub(crate) fn refresh(&self) -> CommentRefresh {
        let input = self.clone();
        CommentRefresh(tokio::spawn(async move {
            let (store, task) = {
                let controls = input.0.lock().await;
                (controls.store.clone(), controls.task.clone())
            };
            loop {
                tokio::time::sleep(Duration::from_secs(15)).await;
                if let Err(error) =
                    crate::ops::linear_observe::refresh_task_comments(&store, &task).await
                {
                    tracing::warn!(%error, "Linear comment refresh failed; retaining confirmed Task direction");
                }
            }
        }))
    }

    pub(crate) async fn poll(
        &self,
        harness: &mut dyn Harness,
        capture: Option<&crate::run_record::CaptureHandle>,
    ) -> Result<()> {
        let mut controls = self.0.lock().await;
        let Controls {
            store,
            task,
            steer,
            interrupt,
            receiver,
            next_steer,
        } = &mut *controls;
        while let Ok(line) = receiver.try_recv() {
            handle_attachment(store, task, harness, line).await?;
        }
        if Instant::now() >= *next_steer {
            for delivered in
                crate::ops::child::inject_live_steers(store, &task.id, harness, steer).await
            {
                if let Some(capture) = capture {
                    capture.record_input(
                        &format!("steer_transport_accepted:{}", delivered.id),
                        &delivered.text,
                    );
                }
            }
            crate::ops::child::observe_interrupt(
                store,
                &WorkRef::Task(task.id.clone()),
                harness,
                interrupt,
            )
            .await;
            *next_steer = Instant::now() + Duration::from_secs(5);
        }
        Ok(())
    }
}

pub(crate) struct CommentRefresh(tokio::task::JoinHandle<()>);
impl Drop for CommentRefresh {
    fn drop(&mut self) {
        self.0.abort();
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
        println!(
            "{}  {:?}",
            task.plan.identifier,
            store.work_status(&work).await?
        );
        return Ok(());
    }
    if line == "/detach" {
        let _ = std::process::Command::new("tmux")
            .args(["detach-client"])
            .status();
        return Ok(());
    }
    if line == "/interrupt" {
        harness.interrupt().await?;
        println!("interrupted active provider turn");
    } else {
        let comment_id = crate::ops::linear_observe::publish_task_steer(store, task, line).await?;
        println!("posted to Linear {comment_id}");
    }
    Ok(())
}

pub(crate) fn task_seed(
    task: &Task,
    project: &ProjectPlan,
    pr: &crate::work::task::TaskPr,
    wave_name: &str,
    steers: &[Steer],
) -> String {
    let context = crate::ops::render_task_context(task, project, pr, wave_name, steers);
    format!(
        "{context}\n\nYou are the current Task worker. Run the selected immutable Flow from its persisted boundary. This Flow does not choose what a later worker will run. Typed PR and Task operations own publication, landing, rotation, and completion. `lf pr abandon` discards only this PR. If this PR already merged out of band and follow-up work remains, `lf pr next [slug]` rotates to the next serial PR, carrying committed and uncommitted follow-up forward."
    )
}
