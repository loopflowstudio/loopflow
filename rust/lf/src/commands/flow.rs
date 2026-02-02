use crate::commands::util::find_repo_root;
use crate::Cli;
use anyhow::{anyhow, Result};
use loopflow_engine::config::load_config_or_default;
use loopflow_engine::runtime::{
    tick_flow_with_runner, Agent, ForkRun, RunId, StepResult, StepRunner, TickResult, WaveRun,
    WaveRunStatus,
};
use loopflow_engine::{
    format_prompt, gather_context, launch_agent, parse_model, GatherContextOpts, LaunchConfig,
};
use loopflow_engine::{CoreError, StoreError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub fn run(
    name: &str,
    area: &[PathBuf],
    model_override: Option<&str>,
    pr: bool,
    cli: &Cli,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let config = load_config_or_default(Some(&repo_root));

    if pr {
        eprintln!("Warning: --pr is not implemented in Rust lf yet.");
    }

    let mut directions = config.direction.clone().unwrap_or_default();
    directions.extend(cli.direction.clone());

    let area_list = if !area.is_empty() {
        area.iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    } else if !cli.area.is_empty() {
        cli.area
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    } else if let Some(default_area) = config.area.clone() {
        vec![default_area]
    } else {
        Vec::new()
    };

    let model = model_override
        .or(cli.model.as_deref())
        .unwrap_or(&config.agent_model)
        .to_string();

    let runner = FlowStepRunner {
        model,
        chrome: cli.chrome.unwrap_or(config.chrome),
        yolo: cli.yolo || config.yolo,
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: config.paste,
        area: area_list.first().cloned(),
    };

    let run_id = RunId::new(uuid::Uuid::new_v4().to_string());
    let run = WaveRun {
        id: run_id.clone(),
        flow: name.to_string(),
        direction: directions,
        area: area_list,
        repo: repo_root,
        status: WaveRunStatus::Running,
        step_index: 0,
        worktree: None,
        current_step: None,
        error: None,
    };

    let store = InMemoryStore::new(run);

    loop {
        match tick_flow_with_runner(&run_id, &store, &runner)? {
            TickResult::StepComplete => continue,
            TickResult::FlowComplete => break,
            TickResult::WaitingInteractive => {
                println!("Flow paused for interactive step.");
                break;
            }
            TickResult::StepFailed => {
                if let Some(err) = store.get_error() {
                    return Err(anyhow!("step failed: {}", err));
                }
                return Err(anyhow!("step failed"));
            }
        }
    }

    Ok(())
}

struct FlowStepRunner {
    model: String,
    chrome: bool,
    yolo: bool,
    lfdocs: bool,
    diff_files: bool,
    diff: bool,
    clipboard: bool,
    area: Option<String>,
}

impl StepRunner for FlowStepRunner {
    fn run(
        &self,
        step: &loopflow_engine::Step,
        worktree: &Path,
        directions: &[String],
    ) -> Result<StepResult, CoreError> {
        let opts = GatherContextOpts {
            repo_root: worktree.to_path_buf(),
            step: Some(step.name.clone()),
            inline: None,
            step_args: Vec::new(),
            run_mode: Some("auto".to_string()),
            directions: directions.to_vec(),
            lfdocs: self.lfdocs,
            diff_files: self.diff_files,
            diff: self.diff,
            clipboard: self.clipboard,
            area: self.area.clone(),
            wave: None,
        };

        let components = gather_context(&opts)?;
        let prompt = format_prompt(&components);

        let (backend, variant) = parse_model(&self.model);
        let launch = LaunchConfig {
            auto: true,
            stream: false,
            skip_permissions: self.yolo,
            model_variant: variant,
            chrome: self.chrome,
            cwd: Some(worktree.to_path_buf()),
        };

        let result = launch_agent(&backend, &prompt, &launch)?;
        Ok(StepResult {
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
        })
    }
}

struct InMemoryStore {
    run: Mutex<WaveRun>,
    agents: Mutex<Vec<Agent>>,
    fork_runs: Mutex<HashMap<(String, usize), Vec<ForkRun>>>,
}

impl InMemoryStore {
    fn new(run: WaveRun) -> Self {
        Self {
            run: Mutex::new(run),
            agents: Mutex::new(Vec::new()),
            fork_runs: Mutex::new(HashMap::new()),
        }
    }

    fn get_error(&self) -> Option<String> {
        self.run.lock().ok().and_then(|run| run.error.clone())
    }
}

impl loopflow_engine::RunStore for InMemoryStore {
    fn get_run(&self, id: &RunId) -> Result<WaveRun, StoreError> {
        let run = self
            .run
            .lock()
            .map_err(|_| StoreError::Other("store poisoned".to_string()))?;
        if &run.id == id {
            Ok(run.clone())
        } else {
            Err(StoreError::RunNotFound(id.to_string()))
        }
    }

    fn update_run(&self, run: &WaveRun) -> Result<(), StoreError> {
        let mut stored = self
            .run
            .lock()
            .map_err(|_| StoreError::Other("store poisoned".to_string()))?;
        *stored = run.clone();
        Ok(())
    }

    fn create_agent(&self, agent: &Agent) -> Result<(), StoreError> {
        let mut agents = self
            .agents
            .lock()
            .map_err(|_| StoreError::Other("store poisoned".to_string()))?;
        agents.push(agent.clone());
        Ok(())
    }

    fn list_fork_runs(
        &self,
        run_id: &RunId,
        step_index: usize,
    ) -> Result<Vec<ForkRun>, StoreError> {
        let fork_runs = self
            .fork_runs
            .lock()
            .map_err(|_| StoreError::Other("store poisoned".to_string()))?;
        Ok(fork_runs
            .get(&(run_id.to_string(), step_index))
            .cloned()
            .unwrap_or_default())
    }

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> Result<(), StoreError> {
        let mut fork_runs = self
            .fork_runs
            .lock()
            .map_err(|_| StoreError::Other("store poisoned".to_string()))?;
        let key = (fork_run.run_id.to_string(), fork_run.step_index);
        let entry = fork_runs.entry(key).or_insert_with(Vec::new);
        if let Some(existing) = entry.iter_mut().find(|r| r.id == fork_run.id) {
            *existing = fork_run.clone();
        } else {
            entry.push(fork_run.clone());
        }
        Ok(())
    }

    fn delete_fork_runs(&self, run_id: &RunId, step_index: usize) -> Result<(), StoreError> {
        let mut fork_runs = self
            .fork_runs
            .lock()
            .map_err(|_| StoreError::Other("store poisoned".to_string()))?;
        fork_runs.remove(&(run_id.to_string(), step_index));
        Ok(())
    }
}
