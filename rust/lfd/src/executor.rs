use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use loopflow_engine::agent::{build_agent_command, LaunchConfig};
use loopflow_engine::config::load_config_or_default;
use loopflow_engine::flow::{
    expand_flow, load_flow, next_action, ConcreteFork, ConcreteItem, ConcreteStep, FlowAction,
    ForkSelect, Step,
};
use loopflow_engine::prompt::{format_prompt, gather_context, GatherContextOpts};
use loopflow_engine::worktree::{create_worktree, remove_worktree};

use crate::id::LfdId;
use crate::loops::common::now_timestamp;
use crate::output::{OutputEvent, OutputHub};
use crate::proto::control::{Agent, AgentStatus, WaveRun, WaveRunStatus};
use crate::scheduler::Scheduler;
use crate::store::{ForkRun, ForkRunStatus, SharedStore};

#[async_trait]
pub trait StepRunner: Send + Sync {
    async fn run(
        &self,
        cmd: Vec<String>,
        cwd: &Path,
        agent_id: &str,
        wave_run_id: &str,
        output: &OutputHub,
    ) -> Result<i32>;
}

#[derive(Debug, Default)]
pub struct AgentRunner;

#[async_trait]
impl StepRunner for AgentRunner {
    async fn run(
        &self,
        cmd: Vec<String>,
        cwd: &Path,
        agent_id: &str,
        wave_run_id: &str,
        output: &OutputHub,
    ) -> Result<i32> {
        if cmd.is_empty() {
            return Err(anyhow!("empty agent command"));
        }

        let mut command = Command::new(&cmd[0]);
        command.args(&cmd[1..]);
        command.current_dir(cwd);
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("missing stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("missing stderr"))?;

        let stdout_task = tokio::spawn(read_stream(
            stdout,
            output.clone(),
            wave_run_id.to_string(),
            agent_id.to_string(),
        ));
        let stderr_task = tokio::spawn(read_stream(
            stderr,
            output.clone(),
            wave_run_id.to_string(),
            agent_id.to_string(),
        ));

        let status = child.wait().await?;
        let _ = stdout_task.await;
        let _ = stderr_task.await;

        let exit_code = status.code().unwrap_or(1);
        Ok(exit_code)
    }
}

async fn read_stream<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    output: OutputHub,
    wave_run_id: String,
    agent_id: String,
) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        output.send(OutputEvent {
            wave_run_id: wave_run_id.clone(),
            agent_id: agent_id.clone(),
            text: line,
        });
    }
}

#[derive(Clone)]
pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    output: OutputHub,
    runner: Arc<dyn StepRunner>,
}

impl WaveExecutor {
    pub fn new(store: SharedStore, scheduler: Arc<Scheduler>, output: OutputHub) -> Self {
        Self {
            store,
            scheduler,
            output,
            runner: Arc::new(AgentRunner),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn with_runner(
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        output: OutputHub,
        runner: Arc<dyn StepRunner>,
    ) -> Self {
        Self {
            store,
            scheduler,
            output,
            runner,
        }
    }

    pub async fn execute(&self, run_id: &LfdId) -> Result<()> {
        let mut run = self
            .store
            .get_wave_run(run_id)?
            .ok_or_else(|| anyhow!("wave run not found"))?;
        if run.status == WaveRunStatus::WaveRunCompleted as i32
            || run.status == WaveRunStatus::WaveRunFailed as i32
        {
            return Ok(());
        }

        let wave_id = LfdId::parse(&run.wave_id)?;
        let wave = self
            .store
            .get_wave(&wave_id)?
            .ok_or_else(|| anyhow!("wave not found"))?;
        let flow = load_flow(&wave.flow, Path::new(&wave.repo))?;
        let plan = expand_flow(&flow, Path::new(&wave.repo))?;

        loop {
            let current_flow_parents = flow_parents_for_index(&plan, run.step_index);
            if run.flow_parents != current_flow_parents {
                run.flow_parents = current_flow_parents;
                self.store.update_wave_run(&run)?;
            }

            match next_action(&plan, run.step_index as usize) {
                FlowAction::RunStep { step } => {
                    let exit_code = self.run_step(&wave, &mut run, &step).await?;
                    if exit_code == 0 {
                        run.step_index += 1;
                        run.status = WaveRunStatus::WaveRunRunning as i32;
                        run.flow_parents = flow_parents_for_index(&plan, run.step_index);
                        self.store.update_wave_run(&run)?;
                    } else {
                        run.status = WaveRunStatus::WaveRunFailed as i32;
                        run.ended_at = Some(now_timestamp());
                        run.error = Some(format!("step {} failed", step.step.name));
                        self.store.update_wave_run(&run)?;
                        return Ok(());
                    }
                }
                FlowAction::WaitInteractive { step } => {
                    let model = step
                        .step
                        .model
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let agent =
                        self.new_agent(&run, &wave.repo, &step, AgentStatus::AgentWaiting, &model);
                    self.store.start_agent(&agent)?;
                    run.status = WaveRunStatus::WaveRunWaiting as i32;
                    run.flow_parents = step.flow_parents.clone();
                    self.store.update_wave_run(&run)?;
                    return Ok(());
                }
                FlowAction::Fork { fork } => match &fork.select {
                    ForkSelect::All => {
                        self.run_fork(&wave, &mut run, &plan, &fork).await?;
                    }
                    ForkSelect::One | ForkSelect::Prompt { .. } => {
                        self.run_choose(&wave, &mut run, &plan, &fork).await?;
                    }
                },
                FlowAction::Complete => {
                    run.status = WaveRunStatus::WaveRunCompleted as i32;
                    run.ended_at = Some(now_timestamp());
                    self.store.update_wave_run(&run)?;
                    return Ok(());
                }
            }
        }
    }

    async fn run_step(
        &self,
        wave: &crate::proto::control::Wave,
        run: &mut WaveRun,
        step: &ConcreteStep,
    ) -> Result<i32> {
        let worktree = worktree_path(run, wave);
        let (prompt, model, launch) =
            build_step_prompt(&worktree, step, &wave.direction, Some(&wave.name))?;
        let cmd = build_agent_command(&model, &prompt, &launch);

        let agent = build_agent_for_step(
            &run.id,
            &wave.repo,
            &worktree,
            step,
            AgentStatus::AgentRunning,
            &model,
        );
        let agent_id = agent.id.clone();
        self.store.start_agent(&agent)?;

        let exit_code = self
            .runner
            .run(cmd, Path::new(&worktree), &agent_id, &run.id, &self.output)
            .await?;

        let ended_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let status = if exit_code == 0 {
            AgentStatus::AgentCompleted
        } else {
            AgentStatus::AgentFailed
        };
        self.store
            .end_agent(&LfdId::parse(&agent_id)?, status as i32, ended_at)?;

        Ok(exit_code)
    }

    async fn run_choose(
        &self,
        wave: &crate::proto::control::Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        if fork.branches.is_empty() {
            run.status = WaveRunStatus::WaveRunFailed as i32;
            run.error = Some("fork has no branches".to_string());
            self.store.update_wave_run(run)?;
            return Ok(());
        }

        let selected = fork
            .branches
            .first()
            .ok_or_else(|| anyhow!("fork has no branches"))?
            .clone();

        if selected.step.interactive.unwrap_or(false) {
            run.status = WaveRunStatus::WaveRunFailed as i32;
            run.error = Some("interactive fork branches are not supported".to_string());
            self.store.update_wave_run(run)?;
            return Ok(());
        }

        let exit_code = self.run_step(wave, run, &selected).await?;
        if exit_code != 0 {
            run.status = WaveRunStatus::WaveRunFailed as i32;
            run.error = Some(format!("fork step {} failed", selected.step.name));
            self.store.update_wave_run(run)?;
            return Ok(());
        }

        run.step_index += 1;
        run.status = WaveRunStatus::WaveRunRunning as i32;
        run.flow_parents = flow_parents_for_index(plan, run.step_index);
        self.store.update_wave_run(run)?;
        Ok(())
    }

    async fn run_fork(
        &self,
        wave: &crate::proto::control::Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        let mut fork_runs = Vec::new();
        for (index, branch) in fork.branches.iter().enumerate() {
            if branch.step.interactive.unwrap_or(false) {
                run.status = WaveRunStatus::WaveRunFailed as i32;
                run.error = Some("interactive fork branches are not supported".to_string());
                self.store.update_wave_run(run)?;
                return Ok(());
            }

            let fork_worktree = fork_worktree_path(run, wave, index as u32);
            if !Path::new(&fork_worktree).exists() {
                create_worktree(
                    Path::new(&wave.repo),
                    Path::new(&fork_worktree),
                    &format!("{}-fork-{}", run.id, index),
                )?;
            }

            let fork_run = ForkRun {
                id: LfdId::new(),
                wave_run_id: LfdId::parse(&run.id)?,
                step_index: run.step_index,
                branch_index: index as u32,
                status: ForkRunStatus::Pending,
                worktree: fork_worktree,
            };
            self.store.upsert_fork_run(&fork_run)?;
            fork_runs.push((fork_run, branch.clone()));
        }

        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(fork_runs.len());
        let mut handles = Vec::new();

        let wave_directions = wave.direction.clone();
        for (fork_run, step) in fork_runs.iter() {
            let store = self.store.clone();
            let runner = self.runner.clone();
            let output = self.output.clone();
            let scheduler = self.scheduler.clone();
            let cancel = cancel.clone();
            let tx = tx.clone();
            let wave_run_id = run.id.clone();
            let wave_repo = wave.repo.clone();
            let worktree = fork_run.worktree.clone();
            let fork_run_id = fork_run.id.clone();
            let fork_run = fork_run.clone();
            let step = step.clone();
            let wave_directions = wave_directions.clone();

            let handle = tokio::spawn(async move {
                if cancel.is_cancelled() {
                    return;
                }

                loop {
                    let (acquired, _) = scheduler.acquire(fork_run_id.as_str()).await;
                    if acquired {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }

                let _ = store.upsert_fork_run(&ForkRun {
                    status: ForkRunStatus::Running,
                    ..fork_run.clone()
                });

                let prompt = build_step_prompt(&worktree, &step, &wave_directions, None);
                let (prompt, model, launch) = match prompt {
                    Ok(result) => result,
                    Err(err) => {
                        let _ = tx.send((fork_run_id.to_string(), Err(err))).await;
                        scheduler.release(fork_run_id.as_str());
                        return;
                    }
                };
                let cmd = build_agent_command(&model, &prompt, &launch);
                let agent = build_agent_for_step(
                    &wave_run_id,
                    &wave_repo,
                    &worktree,
                    &step,
                    AgentStatus::AgentRunning,
                    &model,
                );
                let _ = store.start_agent(&agent);

                let result = runner
                    .run(cmd, Path::new(&worktree), &agent.id, &wave_run_id, &output)
                    .await;

                let status = match result {
                    Ok(0) => ForkRunStatus::Completed,
                    _ => ForkRunStatus::Failed,
                };
                let _ = store.upsert_fork_run(&ForkRun {
                    status,
                    ..fork_run.clone()
                });
                let _ = tx.send((fork_run_id.to_string(), result)).await;
                scheduler.release(fork_run_id.as_str());
            });

            handles.push(handle);
        }

        let mut failures = None;
        let mut completed = 0usize;
        while let Some((fork_id, result)) = rx.recv().await {
            match result {
                Ok(0) => {
                    completed += 1;
                    if completed == fork_runs.len() {
                        break;
                    }
                }
                Ok(code) => {
                    failures = Some(format!("fork branch {} failed ({})", fork_id, code));
                    break;
                }
                Err(err) => {
                    failures = Some(format!("fork branch {} failed: {}", fork_id, err));
                    break;
                }
            }
        }

        if let Some(error) = failures {
            cancel.cancel();
            for handle in handles {
                handle.abort();
            }
            self.cleanup_fork(run, &fork_runs).await;
            run.status = WaveRunStatus::WaveRunFailed as i32;
            run.error = Some(error);
            self.store.update_wave_run(run)?;
            return Ok(());
        }

        if let Some(step_name) = fork.synthesize.as_deref() {
            let synth_step = ConcreteStep {
                step: Step {
                    name: step_name.to_string(),
                    model: None,
                    directions: Vec::new(),
                    interactive: None,
                    content: None,
                },
                flow_parents: fork.flow_parents.clone(),
            };
            let exit_code = self.run_step(wave, run, &synth_step).await?;
            if exit_code != 0 {
                self.cleanup_fork(run, &fork_runs).await;
                run.status = WaveRunStatus::WaveRunFailed as i32;
                run.error = Some(format!("synthesize {} failed", step_name));
                self.store.update_wave_run(run)?;
                return Ok(());
            }
        }

        self.cleanup_fork(run, &fork_runs).await;
        run.step_index += 1;
        run.status = WaveRunStatus::WaveRunRunning as i32;
        run.flow_parents = flow_parents_for_index(plan, run.step_index);
        self.store.update_wave_run(run)?;
        Ok(())
    }

    async fn cleanup_fork(&self, run: &WaveRun, fork_runs: &[(ForkRun, ConcreteStep)]) {
        for (fork_run, _) in fork_runs {
            let worktree_path = Path::new(&fork_run.worktree);
            if worktree_path.join(".git").exists() {
                let _ = remove_worktree(worktree_path, true);
            }
            self.scheduler.release(fork_run.id.as_str());
        }
        let _ = self.store.delete_fork_runs(
            &LfdId::parse(&run.id).unwrap_or_else(|_| LfdId::new()),
            run.step_index,
        );
    }
}

fn worktree_path(run: &WaveRun, wave: &crate::proto::control::Wave) -> String {
    if run.worktree.is_empty() {
        wave.repo.clone()
    } else {
        run.worktree.clone()
    }
}

fn fork_worktree_path(
    run: &WaveRun,
    wave: &crate::proto::control::Wave,
    branch_index: u32,
) -> String {
    let base = worktree_path(run, wave);
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

fn flow_parents_for_index(items: &[ConcreteItem], step_index: u32) -> Vec<String> {
    match items.get(step_index as usize) {
        Some(ConcreteItem::Step(step)) => step.flow_parents.clone(),
        Some(ConcreteItem::Fork(fork)) => fork.flow_parents.clone(),
        None => Vec::new(),
    }
}

fn build_step_prompt(
    worktree: &str,
    step: &ConcreteStep,
    directions: &[String],
    wave: Option<&str>,
) -> Result<(String, String, LaunchConfig)> {
    let config = load_config_or_default(Some(Path::new(worktree)));
    let directions = merge_directions(directions, &step.step.directions);
    let opts = GatherContextOpts {
        repo_root: PathBuf::from(worktree),
        step: Some(step.step.name.clone()),
        inline: None,
        step_args: Vec::new(),
        run_mode: Some("auto".to_string()),
        directions,
        files: Vec::new(),
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: config.paste,
        area: config.area.clone(),
        wave: wave.map(str::to_string),
    };

    let components = gather_context(&opts)?;
    let prompt = format_prompt(&components);
    let model = step
        .step
        .model
        .clone()
        .unwrap_or_else(|| config.agent_model.clone());
    let launch = LaunchConfig {
        auto: true,
        stream: true,
        skip_permissions: config.yolo,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(PathBuf::from(worktree)),
    };

    Ok((prompt, model, launch))
}

fn build_agent_for_step(
    wave_run_id: &str,
    repo: &str,
    worktree: &str,
    step: &ConcreteStep,
    status: AgentStatus,
    model: &str,
) -> Agent {
    Agent {
        id: Uuid::new_v4().to_string(),
        step: step.step.name.clone(),
        repo: repo.to_string(),
        worktree: worktree.to_string(),
        wave_run_id: Some(wave_run_id.to_string()),
        status: status as i32,
        started_at: Some(now_timestamp()),
        ended_at: None,
        pid: None,
        model: model.to_string(),
        run_mode: "auto".to_string(),
    }
}
