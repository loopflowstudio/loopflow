use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use tracing::{debug, error, info, warn};

use crate::engine::agent::{build_agent_command, LaunchConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteFork, ConcreteItem, ConcreteStep, FlowAction,
    ForkSelect, Step,
};
use crate::engine::prompt::{format_prompt, gather_context, GatherContextOpts};
use crate::engine::worktree::{create_worktree, remove_worktree};

use time::OffsetDateTime;

use crate::lfd::id::LfdId;
use crate::lfd::output::{OutputEvent, OutputHub};
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::{ForkRun, ForkRunStatus, SharedStore};
use crate::lfd::types::{Agent, AgentStatus, Wave, WaveRun, WaveRunStatus, WaveStatus};

#[async_trait]
pub trait StepRunner: Send + Sync {
    async fn run(
        &self,
        cmd: Vec<String>,
        cwd: &Path,
        wave_id: &str,
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
        wave_id: &str,
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
            wave_id.to_string(),
            wave_run_id.to_string(),
            agent_id.to_string(),
        ));
        let stderr_task = tokio::spawn(read_stream(
            stderr,
            output.clone(),
            wave_id.to_string(),
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
    wave_id: String,
    wave_run_id: String,
    agent_id: String,
) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        output.send(OutputEvent {
            wave_id: wave_id.clone(),
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
        if run.status == WaveRunStatus::Completed || run.status == WaveRunStatus::Failed {
            return Ok(());
        }

        let wave = self
            .store
            .get_wave(&run.wave_id)?
            .ok_or_else(|| anyhow!("wave not found"))?;
        info!(run_id = %run.id, flow = %run.snapshot.flow, repo = %run.snapshot.repo, "loading flow");
        let flow = load_flow(&run.snapshot.flow, Path::new(&run.snapshot.repo))?;
        let plan = expand_flow(&flow, Path::new(&run.snapshot.repo))?;
        debug!(run_id = %run.id, plan_items = plan.len(), "flow expanded");

        loop {
            let current_flow_parents = flow_parents_for_index(&plan, run.step_index);
            if run.flow_parents != current_flow_parents {
                run.flow_parents = current_flow_parents;
                self.store.update_wave_run(&run)?;
            }

            match next_action(&plan, run.step_index as usize) {
                FlowAction::RunStep { step } => {
                    info!(run_id = %run.id, step = %step.step.name, step_index = run.step_index, "running step");
                    let exit_code = self.run_step(&wave, &mut run, &step).await?;
                    if exit_code == 0 {
                        run.step_index += 1;
                        run.status = WaveRunStatus::Running;
                        run.flow_parents = flow_parents_for_index(&plan, run.step_index);
                        self.store.update_wave_run(&run)?;
                    } else {
                        run.status = WaveRunStatus::Failed;
                        run.ended_at = Some(OffsetDateTime::now_utc());
                        run.error = Some(format!("step {} failed", step.step.name));
                        self.store.update_wave_run(&run)?;
                        self.set_wave_status(&wave.id, WaveStatus::Failed);
                        return Ok(());
                    }
                }
                FlowAction::WaitInteractive { step } => {
                    let model = step
                        .step
                        .model
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let worktree = worktree_path(&run);
                    let agent = build_agent_for_step(
                        &run.id,
                        &run.snapshot.repo,
                        &worktree,
                        &step,
                        AgentStatus::Waiting,
                        &model,
                    );
                    self.store.start_agent(&agent)?;
                    run.status = WaveRunStatus::Waiting;
                    run.flow_parents = step.flow_parents.clone();
                    self.store.update_wave_run(&run)?;
                    self.set_wave_status(&wave.id, WaveStatus::Waiting);
                    return Ok(());
                }
                FlowAction::Fork { fork } => match &fork.select {
                    ForkSelect::All => {
                        info!(
                            run_id = %run.id,
                            branches = fork.branches.len(),
                            step_index = run.step_index,
                            synthesize = ?fork.synthesize,
                            "running fork (all branches)"
                        );
                        self.run_fork(&wave, &mut run, &plan, &fork).await?;
                    }
                    ForkSelect::One | ForkSelect::Prompt { .. } => {
                        info!(run_id = %run.id, step_index = run.step_index, "running fork (choose)");
                        self.run_choose(&wave, &mut run, &plan, &fork).await?;
                    }
                },
                FlowAction::Complete => {
                    run.status = WaveRunStatus::Completed;
                    run.ended_at = Some(OffsetDateTime::now_utc());
                    self.store.update_wave_run(&run)?;
                    self.set_wave_status(&wave.id, WaveStatus::Completed);
                    return Ok(());
                }
            }
        }
    }

    fn set_wave_status(&self, wave_id: &LfdId, status: WaveStatus) {
        if let Ok(Some(mut wave)) = self.store.get_wave(wave_id) {
            wave.status = status;
            let _ = self.store.update_wave(&wave);
        }
    }

    async fn run_step(&self, wave: &Wave, run: &mut WaveRun, step: &ConcreteStep) -> Result<i32> {
        let worktree = worktree_path(run);
        debug!(run_id = %run.id, step = %step.step.name, worktree = %worktree, "building step prompt");
        let (prompt, model, launch) =
            build_step_prompt(&worktree, step, &run.snapshot.direction, Some(&wave.name))?;
        let cmd = build_agent_command(&model, &prompt, &launch);
        info!(run_id = %run.id, step = %step.step.name, model = %model, "launching agent");

        let agent = build_agent_for_step(
            &run.id,
            &run.snapshot.repo,
            &worktree,
            step,
            AgentStatus::Running,
            &model,
        );
        let agent_id = agent.id.clone();
        self.store.start_agent(&agent)?;

        let exit_code = self
            .runner
            .run(
                cmd,
                Path::new(&worktree),
                run.wave_id.as_str(),
                agent_id.as_str(),
                run.id.as_str(),
                &self.output,
            )
            .await?;

        let ended_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let status = if exit_code == 0 {
            AgentStatus::Completed
        } else {
            AgentStatus::Failed
        };
        self.store.end_agent(&agent_id, status.as_i32(), ended_at)?;

        Ok(exit_code)
    }

    async fn run_choose(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        if fork.branches.is_empty() {
            run.status = WaveRunStatus::Failed;
            run.error = Some("fork has no branches".to_string());
            self.store.update_wave_run(run)?;
            self.set_wave_status(&wave.id, WaveStatus::Failed);
            return Ok(());
        }

        let selected = fork
            .branches
            .first()
            .ok_or_else(|| anyhow!("fork has no branches"))?
            .clone();

        if selected.step.interactive.unwrap_or(false) {
            run.status = WaveRunStatus::Failed;
            run.error = Some("interactive fork branches are not supported".to_string());
            self.store.update_wave_run(run)?;
            self.set_wave_status(&wave.id, WaveStatus::Failed);
            return Ok(());
        }

        let exit_code = self.run_step(wave, run, &selected).await?;
        if exit_code != 0 {
            run.status = WaveRunStatus::Failed;
            run.error = Some(format!("fork step {} failed", selected.step.name));
            self.store.update_wave_run(run)?;
            self.set_wave_status(&wave.id, WaveStatus::Failed);
            return Ok(());
        }

        run.step_index += 1;
        run.status = WaveRunStatus::Running;
        run.flow_parents = flow_parents_for_index(plan, run.step_index);
        self.store.update_wave_run(run)?;
        Ok(())
    }

    async fn run_fork(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        fork: &ConcreteFork,
    ) -> Result<()> {
        let mut fork_runs = Vec::new();
        for (index, branch) in fork.branches.iter().enumerate() {
            if branch.step.interactive.unwrap_or(false) {
                run.status = WaveRunStatus::Failed;
                run.error = Some("interactive fork branches are not supported".to_string());
                self.store.update_wave_run(run)?;
                self.set_wave_status(&wave.id, WaveStatus::Failed);
                return Ok(());
            }

            let fork_worktree = fork_worktree_path(run, index as u32);
            if !Path::new(&fork_worktree).exists() {
                debug!(
                    run_id = %run.id,
                    branch_index = index,
                    step = %branch.step.name,
                    worktree = %fork_worktree,
                    "creating fork worktree"
                );
                create_worktree(
                    Path::new(&run.snapshot.repo),
                    Path::new(&fork_worktree),
                    &format!("{}-fork-{}", run.id, index),
                )?;
            }

            let fork_run = ForkRun {
                id: LfdId::new(),
                wave_run_id: run.id.clone(),
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

        let wave_directions = run.snapshot.direction.clone();
        for (fork_run, step) in fork_runs.iter() {
            let store = self.store.clone();
            let runner = self.runner.clone();
            let output = self.output.clone();
            let scheduler = self.scheduler.clone();
            let cancel = cancel.clone();
            let tx = tx.clone();
            let fork_wave_id = wave.id.clone();
            let wave_run_id = run.id.clone();
            let wave_repo = run.snapshot.repo.clone();
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

                debug!(
                    fork_run_id = %fork_run_id,
                    step = %step.step.name,
                    worktree = %worktree,
                    directions = ?wave_directions,
                    "building fork branch prompt"
                );
                let prompt = build_step_prompt(&worktree, &step, &wave_directions, None);
                let (prompt, model, launch) = match prompt {
                    Ok(result) => result,
                    Err(err) => {
                        error!(
                            fork_run_id = %fork_run_id,
                            step = %step.step.name,
                            error = %err,
                            "fork branch prompt build failed"
                        );
                        let _ = tx.send((fork_run_id.to_string(), Err(err))).await;
                        scheduler.release(fork_run_id.as_str());
                        return;
                    }
                };
                let cmd = build_agent_command(&model, &prompt, &launch);
                info!(
                    fork_run_id = %fork_run_id,
                    step = %step.step.name,
                    model = %model,
                    cmd_len = cmd.len(),
                    "launching fork branch agent"
                );
                let agent = build_agent_for_step(
                    &wave_run_id,
                    &wave_repo,
                    &worktree,
                    &step,
                    AgentStatus::Running,
                    &model,
                );
                let _ = store.start_agent(&agent);

                let result = runner
                    .run(
                        cmd,
                        Path::new(&worktree),
                        fork_wave_id.as_str(),
                        agent.id.as_str(),
                        wave_run_id.as_str(),
                        &output,
                    )
                    .await;

                // End the agent in the store (mirrors run_step behavior).
                let ended_at = time::OffsetDateTime::now_utc().unix_timestamp();
                let agent_status = match &result {
                    Ok(0) => AgentStatus::Completed,
                    _ => AgentStatus::Failed,
                };
                let _ = store.end_agent(&agent.id, agent_status.as_i32(), ended_at);

                let status = match &result {
                    Ok(0) => {
                        info!(fork_run_id = %fork_run_id, step = %step.step.name, "fork branch completed");
                        ForkRunStatus::Completed
                    }
                    Ok(code) => {
                        warn!(fork_run_id = %fork_run_id, step = %step.step.name, exit_code = code, "fork branch failed");
                        ForkRunStatus::Failed
                    }
                    Err(err) => {
                        error!(fork_run_id = %fork_run_id, step = %step.step.name, error = %err, "fork branch error");
                        ForkRunStatus::Failed
                    }
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
        let total = fork_runs.len();
        debug!(run_id = %run.id, total_branches = total, "waiting for fork results");
        while let Some((fork_id, result)) = rx.recv().await {
            match result {
                Ok(0) => {
                    completed += 1;
                    debug!(run_id = %run.id, completed, total, "fork branch done");
                    if completed == total {
                        break;
                    }
                }
                Ok(code) => {
                    failures = Some(format!("fork branch {} exited with code {}", fork_id, code));
                    break;
                }
                Err(err) => {
                    failures = Some(format!("fork branch {} error: {}", fork_id, err));
                    break;
                }
            }
        }

        if let Some(error) = failures {
            error!(run_id = %run.id, error = %error, "fork failed");
            cancel.cancel();
            for handle in handles {
                handle.abort();
            }
            self.cleanup_fork(run, &fork_runs).await;
            run.status = WaveRunStatus::Failed;
            run.error = Some(error);
            self.store.update_wave_run(run)?;
            self.set_wave_status(&wave.id, WaveStatus::Failed);
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
                run.status = WaveRunStatus::Failed;
                run.error = Some(format!("synthesize {} failed", step_name));
                self.store.update_wave_run(run)?;
                self.set_wave_status(&wave.id, WaveStatus::Failed);
                return Ok(());
            }
        }

        self.cleanup_fork(run, &fork_runs).await;
        run.step_index += 1;
        run.status = WaveRunStatus::Running;
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
        let _ = self.store.delete_fork_runs(&run.id, run.step_index);
    }
}

fn worktree_path(run: &WaveRun) -> String {
    if run.worktree.is_empty() {
        run.snapshot.repo.clone()
    } else {
        run.worktree.clone()
    }
}

fn fork_worktree_path(run: &WaveRun, branch_index: u32) -> String {
    let base = worktree_path(run);
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
        message: None,
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
        context_file: None,
        ..Default::default()
    };

    Ok((prompt, model, launch))
}

fn build_agent_for_step(
    wave_run_id: &LfdId,
    repo: &str,
    worktree: &str,
    step: &ConcreteStep,
    status: AgentStatus,
    model: &str,
) -> Agent {
    Agent {
        id: LfdId::new(),
        step: step.step.name.clone(),
        repo: repo.to_string(),
        worktree: worktree.to_string(),
        wave_run_id: Some(wave_run_id.clone()),
        status,
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        pid: None,
        model: model.to_string(),
        run_mode: "auto".to_string(),
    }
}
