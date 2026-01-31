use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::agent::{launch_agent, LaunchConfig};
use crate::config::load_config_or_default;
use crate::error::CoreError;
use crate::flow::{load_flow, FlowItem, Step};
use crate::prompt::{format_prompt, gather_context, GatherContextOpts};
use crate::store::RunStore;
use crate::worktree::{create_worktree, remove_worktree};

// -----------------------------------------------------------------------------
// Newtypes
// -----------------------------------------------------------------------------

/// Unique identifier for a flow run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RunId(pub String);

impl RunId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// -----------------------------------------------------------------------------
// Status enums
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum FlowRunStatus {
    #[default]
    Running,
    Waiting,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum StepRunStatus {
    #[default]
    Running,
    Waiting,
    Completed,
    Failed,
}

// -----------------------------------------------------------------------------
// Core structs
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlowRun {
    pub id: RunId,
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
    pub flow_run_id: Option<RunId>,
    pub status: StepRunStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ForkRunStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForkRun {
    pub id: String,
    pub run_id: RunId,
    pub step_index: usize,
    pub branch_index: usize,
    pub status: ForkRunStatus,
    pub worktree: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TickResult {
    StepComplete,
    FlowComplete,
    WaitingInteractive,
    StepFailed,
}

#[derive(Debug, Clone, Default)]
pub struct StepResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

// -----------------------------------------------------------------------------
// Step runner trait and implementation
// -----------------------------------------------------------------------------

pub trait StepRunner {
    fn run(
        &self,
        step: &Step,
        worktree: &Path,
        directions: &[String],
    ) -> Result<StepResult, CoreError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandStepRunner;

impl StepRunner for CommandStepRunner {
    fn run(
        &self,
        step: &Step,
        worktree: &Path,
        directions: &[String],
    ) -> Result<StepResult, CoreError> {
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

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentStepRunner;

impl StepRunner for AgentStepRunner {
    fn run(
        &self,
        step: &Step,
        worktree: &Path,
        directions: &[String],
    ) -> Result<StepResult, CoreError> {
        let config = load_config_or_default(Some(worktree));
        let model = step
            .model
            .clone()
            .unwrap_or_else(|| config.agent_model.clone());

        let opts = GatherContextOpts {
            repo_root: worktree.to_path_buf(),
            step: Some(step.name.clone()),
            inline: None,
            step_args: Vec::new(),
            run_mode: Some("auto".to_string()),
            directions: directions.to_vec(),
            lfdocs: config.lfdocs,
            diff_files: config.diff_files,
            diff: config.diff,
            clipboard: config.paste,
            area: config.area.clone(),
            wave: None,
        };

        let components = gather_context(&opts)?;
        let prompt = format_prompt(&components);

        let launch = LaunchConfig {
            auto: true,
            stream: false,
            skip_permissions: config.yolo,
            model_variant: None,
            chrome: config.chrome,
            cwd: Some(worktree.to_path_buf()),
        };

        let result = launch_agent(&model, &prompt, &launch)?;
        Ok(StepResult {
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
        })
    }
}

pub fn tick_flow(run_id: &RunId, store: &dyn RunStore) -> Result<TickResult, CoreError> {
    tick_flow_with_runner(run_id, store, &AgentStepRunner)
}

pub fn tick_flow_with_runner(
    run_id: &RunId,
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
        FlowItem::Fork {
            branches,
            synthesize,
        } => {
            return run_fork_item(&mut run, branches, synthesize.as_deref(), store, runner);
        }
        FlowItem::Choose { prompt, options } => {
            return run_choose_item(&mut run, &prompt, options, store, runner);
        }
        FlowItem::LoopUntilEmpty {
            steps,
            wave,
            max_iterations,
        } => {
            return run_loop_until_empty(
                &mut run,
                &steps,
                wave.as_deref(),
                max_iterations,
                store,
                runner,
            );
        }
    };

    let worktree = run
        .worktree
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| run.repo.clone());

    if step.interactive.unwrap_or(false) {
        let step_run = StepRun {
            id: format!("{}:{}", run.id.as_str(), run.step_index),
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
        id: format!("{}:{}", run.id.as_str(), run.step_index),
        step: step.name.clone(),
        repo: run.repo.to_string_lossy().to_string(),
        worktree: worktree.to_string_lossy().to_string(),
        flow_run_id: Some(run.id.clone()),
        status: StepRunStatus::Running,
    };
    store.create_step_run(&step_run)?;

    let step_directions = merge_directions(&run.direction, &step.directions);
    let result = runner.run(&step, &worktree, &step_directions)?;
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

pub fn run_step(
    step: &Step,
    worktree: &Path,
    direction: &[String],
) -> Result<StepResult, CoreError> {
    AgentStepRunner.run(step, worktree, direction)
}

fn run_fork_item(
    run: &mut FlowRun,
    branches: Vec<FlowItem>,
    synthesize: Option<&str>,
    store: &dyn RunStore,
    runner: &dyn StepRunner,
) -> Result<TickResult, CoreError> {
    let mut fork_runs = store.list_fork_runs(&run.id, run.step_index)?;
    if fork_runs.is_empty() {
        let step_run = StepRun {
            id: format!("{}:{}", run.id.as_str(), run.step_index),
            step: "fork".to_string(),
            repo: run.repo.to_string_lossy().to_string(),
            worktree: run
                .worktree
                .clone()
                .unwrap_or_else(|| run.repo.to_string_lossy().to_string()),
            flow_run_id: Some(run.id.clone()),
            status: StepRunStatus::Running,
        };
        store.create_step_run(&step_run)?;

        for (index, branch) in branches.iter().enumerate() {
            let step = match branch {
                FlowItem::Step(step) => step,
                _ => {
                    run.status = FlowRunStatus::Failed;
                    run.error = Some("non-step fork branches are not supported".to_string());
                    store.update_run(run)?;
                    return Ok(TickResult::StepFailed);
                }
            };

            let fork_worktree = fork_worktree_path(run, index);
            let fork_branch = format!("{}-fork-{}", run.id.as_str(), index);
            if !Path::new(&fork_worktree).exists() {
                if let Err(err) =
                    create_worktree(&run.repo, Path::new(&fork_worktree), &fork_branch)
                {
                    run.status = FlowRunStatus::Failed;
                    run.error = Some(err.to_string());
                    store.update_run(run)?;
                    return Ok(TickResult::StepFailed);
                }
            }

            let fork_run = ForkRun {
                id: fork_run_id(run, index),
                run_id: run.id.clone(),
                step_index: run.step_index,
                branch_index: index,
                status: if step.interactive.unwrap_or(false) {
                    ForkRunStatus::Failed
                } else {
                    ForkRunStatus::Pending
                },
                worktree: fork_worktree,
            };
            store.upsert_fork_run(&fork_run)?;
        }
        fork_runs = store.list_fork_runs(&run.id, run.step_index)?;
    }

    for (index, branch) in branches.iter().enumerate() {
        let step = match branch {
            FlowItem::Step(step) => step,
            _ => {
                run.status = FlowRunStatus::Failed;
                run.error = Some("non-step fork branches are not supported".to_string());
                store.update_run(run)?;
                return Ok(TickResult::StepFailed);
            }
        };

        if step.interactive.unwrap_or(false) {
            run.status = FlowRunStatus::Failed;
            run.error = Some("interactive fork branches are not supported".to_string());
            store.update_run(run)?;
            return Ok(TickResult::StepFailed);
        }

        let mut fork_run = match fork_runs.iter().find(|fork| fork.branch_index == index) {
            Some(fork_run) => fork_run.clone(),
            None => {
                run.status = FlowRunStatus::Failed;
                run.error = Some("fork run missing branch metadata".to_string());
                store.update_run(run)?;
                return Ok(TickResult::StepFailed);
            }
        };

        if matches!(fork_run.status, ForkRunStatus::Completed) {
            continue;
        }

        fork_run.status = ForkRunStatus::Running;
        store.upsert_fork_run(&fork_run)?;

        let branch_directions = merge_directions(&run.direction, &step.directions);
        let result = runner.run(step, Path::new(&fork_run.worktree), &branch_directions)?;
        if result.exit_code == 0 {
            fork_run.status = ForkRunStatus::Completed;
            store.upsert_fork_run(&fork_run)?;
            continue;
        }

        fork_run.status = ForkRunStatus::Failed;
        store.upsert_fork_run(&fork_run)?;
        run.status = FlowRunStatus::Failed;
        run.error = Some(result.stderr);
        store.update_run(run)?;
        return Ok(TickResult::StepFailed);
    }

    let all_done = store
        .list_fork_runs(&run.id, run.step_index)?
        .iter()
        .all(|fork| matches!(fork.status, ForkRunStatus::Completed));
    if !all_done {
        run.status = FlowRunStatus::Failed;
        run.error = Some("fork branches did not complete".to_string());
        store.update_run(run)?;
        return Ok(TickResult::StepFailed);
    }

    if let Some(step_name) = synthesize {
        let synth_step = Step {
            name: step_name.to_string(),
            model: None,
            directions: Vec::new(),
            interactive: None,
            content: None,
        };
        let base_worktree = run
            .worktree
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| run.repo.clone());

        let result = runner.run(&synth_step, &base_worktree, &run.direction)?;
        if result.exit_code != 0 {
            run.status = FlowRunStatus::Failed;
            run.error = Some(result.stderr);
            store.update_run(run)?;
            return Ok(TickResult::StepFailed);
        }
    }

    for fork_run in store.list_fork_runs(&run.id, run.step_index)? {
        let worktree_path = Path::new(&fork_run.worktree);
        if worktree_path.join(".git").exists() {
            // Force-delete branch since we created these fork worktrees
            let _ = remove_worktree(worktree_path, true);
        }
    }
    store.delete_fork_runs(&run.id, run.step_index)?;

    run.step_index += 1;
    run.status = FlowRunStatus::Running;
    run.current_step = None;
    store.update_run(run)?;
    Ok(TickResult::StepComplete)
}

fn fork_run_id(run: &FlowRun, branch_index: usize) -> String {
    format!("{}:{}:{}", run.id.as_str(), run.step_index, branch_index)
}

fn fork_worktree_path(run: &FlowRun, branch_index: usize) -> String {
    let base = run
        .worktree
        .clone()
        .unwrap_or_else(|| run.repo.to_string_lossy().to_string());
    format!("{base}-fork-{branch_index}")
}

fn merge_directions(base: &[String], extra: &[String]) -> Vec<String> {
    if extra.is_empty() {
        return base.to_vec();
    }
    let mut combined = base.to_vec();
    for item in extra {
        if !combined.contains(item) {
            combined.push(item.clone());
        }
    }
    combined
}

// -----------------------------------------------------------------------------
// Choose execution
// -----------------------------------------------------------------------------

/// Execute a choose flow item.
///
/// The choose construct requires selecting a branch based on the prompt.
/// For now, we select the first option (deterministic for testing).
/// In production, this would invoke an LLM to evaluate the prompt.
fn run_choose_item(
    run: &mut FlowRun,
    _prompt: &str,
    options: HashMap<String, Vec<FlowItem>>,
    store: &dyn RunStore,
    runner: &dyn StepRunner,
) -> Result<TickResult, CoreError> {
    // For now, select the first option alphabetically (deterministic)
    // In production, would invoke LLM with the prompt to select
    let mut option_keys: Vec<&String> = options.keys().collect();
    option_keys.sort();

    let selected_key = match option_keys.first() {
        Some(key) => (*key).clone(),
        None => {
            run.status = FlowRunStatus::Failed;
            run.error = Some("choose has no options".to_string());
            store.update_run(run)?;
            return Ok(TickResult::StepFailed);
        }
    };

    let selected_items = match options.get(&selected_key) {
        Some(items) => items.clone(),
        None => {
            run.status = FlowRunStatus::Failed;
            run.error = Some(format!("choose option '{}' not found", selected_key));
            store.update_run(run)?;
            return Ok(TickResult::StepFailed);
        }
    };

    // Execute each item in the selected branch
    let worktree = run
        .worktree
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| run.repo.clone());

    for item in selected_items {
        match item {
            FlowItem::Step(step) => {
                if step.interactive.unwrap_or(false) {
                    run.status = FlowRunStatus::Failed;
                    run.error =
                        Some("interactive steps in choose branches not supported".to_string());
                    store.update_run(run)?;
                    return Ok(TickResult::StepFailed);
                }

                let step_run = StepRun {
                    id: format!(
                        "{}:{}:choose:{}",
                        run.id.as_str(),
                        run.step_index,
                        step.name
                    ),
                    step: step.name.clone(),
                    repo: run.repo.to_string_lossy().to_string(),
                    worktree: worktree.to_string_lossy().to_string(),
                    flow_run_id: Some(run.id.clone()),
                    status: StepRunStatus::Running,
                };
                store.create_step_run(&step_run)?;

                let directions = merge_directions(&run.direction, &step.directions);
                let result = runner.run(&step, &worktree, &directions)?;
                if result.exit_code != 0 {
                    run.status = FlowRunStatus::Failed;
                    run.error = Some(result.stderr);
                    store.update_run(run)?;
                    return Ok(TickResult::StepFailed);
                }
            }
            _ => {
                // Nested choose/fork/loop not supported yet
                run.status = FlowRunStatus::Failed;
                run.error = Some("nested flow items in choose not supported".to_string());
                store.update_run(run)?;
                return Ok(TickResult::StepFailed);
            }
        }
    }

    run.step_index += 1;
    run.status = FlowRunStatus::Running;
    run.current_step = None;
    store.update_run(run)?;
    Ok(TickResult::StepComplete)
}

// -----------------------------------------------------------------------------
// LoopUntilEmpty execution
// -----------------------------------------------------------------------------

/// Execute a loop_until_empty flow item.
///
/// The loop runs until a termination condition is met. For now, we run
/// each step once (single iteration). In production, the loop would
/// continue until the steps indicate completion (e.g., no more items to process).
fn run_loop_until_empty(
    run: &mut FlowRun,
    steps: &[FlowItem],
    wave: Option<&str>,
    max_iterations: usize,
    store: &dyn RunStore,
    runner: &dyn StepRunner,
) -> Result<TickResult, CoreError> {
    let worktree = run
        .worktree
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| run.repo.clone());

    let wave_name = resolve_wave_name(&run.repo, wave);
    if steps.is_empty() && !is_wave_empty(&run.repo, &wave_name) {
        run.status = FlowRunStatus::Failed;
        run.error = Some("loop_until_empty has no steps".to_string());
        store.update_run(run)?;
        return Ok(TickResult::StepFailed);
    }

    let mut iteration = 0usize;
    while !is_wave_empty(&run.repo, &wave_name) {
        iteration += 1;
        if iteration > max_iterations {
            run.status = FlowRunStatus::Failed;
            run.error = Some(format!(
                "loop_until_empty max iterations ({}) reached",
                max_iterations
            ));
            store.update_run(run)?;
            return Ok(TickResult::StepFailed);
        }

        for item in steps {
            match item {
                FlowItem::Step(step) => {
                    if step.interactive.unwrap_or(false) {
                        run.status = FlowRunStatus::Failed;
                        run.error = Some("interactive steps in loop not supported".to_string());
                        store.update_run(run)?;
                        return Ok(TickResult::StepFailed);
                    }

                    let step_run = StepRun {
                        id: format!(
                            "{}:{}:loop:{}:{}",
                            run.id.as_str(),
                            run.step_index,
                            iteration,
                            step.name
                        ),
                        step: step.name.clone(),
                        repo: run.repo.to_string_lossy().to_string(),
                        worktree: worktree.to_string_lossy().to_string(),
                        flow_run_id: Some(run.id.clone()),
                        status: StepRunStatus::Running,
                    };
                    store.create_step_run(&step_run)?;

                    let directions = merge_directions(&run.direction, &step.directions);
                    let result = runner.run(&step, &worktree, &directions)?;
                    if result.exit_code != 0 {
                        run.status = FlowRunStatus::Failed;
                        run.error = Some(result.stderr);
                        store.update_run(run)?;
                        return Ok(TickResult::StepFailed);
                    }
                }
                _ => {
                    run.status = FlowRunStatus::Failed;
                    run.error = Some("nested flow items in loop not supported".to_string());
                    store.update_run(run)?;
                    return Ok(TickResult::StepFailed);
                }
            }
        }
    }

    run.step_index += 1;
    run.status = FlowRunStatus::Running;
    run.current_step = None;
    store.update_run(run)?;
    Ok(TickResult::StepComplete)
}

fn resolve_wave_name(repo_root: &Path, explicit: Option<&str>) -> String {
    if let Some(name) = explicit {
        return name.to_string();
    }
    if let Some(dir_name) = repo_root.file_name().and_then(|n| n.to_str()) {
        if let Some((_prefix, wave)) = dir_name.rsplit_once('.') {
            return wave.to_string();
        }
    }
    if let Some(branch) = current_branch(repo_root) {
        return branch;
    }
    "default".to_string()
}

fn current_branch(repo_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

fn is_wave_empty(repo_root: &Path, wave: &str) -> bool {
    let roadmap_dir = repo_root.join("roadmap").join(wave);
    if !roadmap_dir.exists() {
        return true;
    }
    let entries = match roadmap_dir.read_dir() {
        Ok(entries) => entries,
        Err(_) => return true,
    };
    for entry in entries.flatten() {
        if entry
            .path()
            .extension()
            .map(|ext| ext == "md")
            .unwrap_or(false)
        {
            return false;
        }
    }
    true
}
