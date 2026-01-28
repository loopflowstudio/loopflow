use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::flow::{load_flow, FlowItem, Step};
use crate::store::RunStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FlowRunStatus {
    Running,
    Waiting,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepRunStatus {
    Running,
    Waiting,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlowRun {
    pub id: String,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub repo: PathBuf,
    pub status: FlowRunStatus,
    pub step_index: usize,
    pub worktree: Option<String>,
    pub current_step: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepRun {
    pub id: String,
    pub step: String,
    pub repo: String,
    pub worktree: String,
    pub flow_run_id: Option<String>,
    pub status: StepRunStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TickResult {
    StepComplete,
    FlowComplete,
    WaitingInteractive,
    StepFailed,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait StepRunner {
    fn run(&self, step: &Step, worktree: &Path, directions: &[String]) -> Result<StepResult, CoreError>;
}

pub struct CommandStepRunner;

impl StepRunner for CommandStepRunner {
    fn run(&self, step: &Step, worktree: &Path, directions: &[String]) -> Result<StepResult, CoreError> {
        let mut cmd = Command::new("lf");
        cmd.arg("--step").arg(&step.name);
        cmd.arg("--worktree").arg(worktree);
        cmd.arg("--auto");
        if !directions.is_empty() {
            cmd.arg("--direction").arg(directions.join(","));
        }
        let output = cmd.output()?;
        Ok(StepResult {
            exit_code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn tick_flow(run_id: &str, store: &dyn RunStore) -> Result<TickResult, CoreError> {
    tick_flow_with_runner(run_id, store, &CommandStepRunner)
}

pub fn tick_flow_with_runner(
    run_id: &str,
    store: &dyn RunStore,
    runner: &dyn StepRunner,
) -> Result<TickResult, CoreError> {
    let mut run = store.get_run(run_id)?;
    if matches!(run.status, FlowRunStatus::Completed) {
        return Ok(TickResult::FlowComplete);
    }
    if matches!(run.status, FlowRunStatus::Failed) {
        return Ok(TickResult::StepFailed);
    }

    let flow = load_flow(&run.flow, &run.repo)?;
    let next_item = match flow.items.get(run.step_index) {
        Some(item) => item.clone(),
        None => {
            run.status = FlowRunStatus::Completed;
            store.update_run(&run)?;
            return Ok(TickResult::FlowComplete);
        }
    };

    let step = match next_item {
        FlowItem::Step(step) => step,
        _ => {
            run.status = FlowRunStatus::Failed;
            run.error = Some("non-step flow item not supported in tick".to_string());
            store.update_run(&run)?;
            return Ok(TickResult::StepFailed);
        }
    };

    let worktree = run
        .worktree
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| run.repo.clone());

    if step.interactive.unwrap_or(false) {
        let step_run = StepRun {
            id: format!("{}:{}", run.id, run.step_index),
            step: step.name.clone(),
            repo: run.repo.to_string_lossy().to_string(),
            worktree: worktree.to_string_lossy().to_string(),
            flow_run_id: Some(run.id.clone()),
            status: StepRunStatus::Waiting,
        };
        store.create_step_run(&step_run)?;
        run.status = FlowRunStatus::Waiting;
        run.current_step = Some(step.name);
        store.update_run(&run)?;
        return Ok(TickResult::WaitingInteractive);
    }

    let step_run = StepRun {
        id: format!("{}:{}", run.id, run.step_index),
        step: step.name.clone(),
        repo: run.repo.to_string_lossy().to_string(),
        worktree: worktree.to_string_lossy().to_string(),
        flow_run_id: Some(run.id.clone()),
        status: StepRunStatus::Running,
    };
    store.create_step_run(&step_run)?;

    let result = runner.run(&step, &worktree, &run.direction)?;
    if result.exit_code == 0 {
        run.step_index += 1;
        run.status = FlowRunStatus::Running;
        run.current_step = None;
        store.update_run(&run)?;
        return Ok(TickResult::StepComplete);
    }

    run.status = FlowRunStatus::Failed;
    run.error = Some(result.stderr);
    store.update_run(&run)?;
    Ok(TickResult::StepFailed)
}

pub fn run_step(step: &Step, worktree: &Path, direction: &[String]) -> Result<StepResult, CoreError> {
    CommandStepRunner.run(step, worktree, direction)
}
