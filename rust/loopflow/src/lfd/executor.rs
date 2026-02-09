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
use crate::engine::config::{load_config, load_config_or_default};
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteFork, ConcreteItem, ConcreteStep, FlowAction,
    ForkSelect, Step,
};
use crate::engine::git::current_branch;
use crate::engine::prompt::{
    drop_native_instruction_docs, format_context_prompt, format_prompt, format_task_prompt,
    gather_context, trim_context_with_breakdown, write_prompt_log, GatherContextOpts,
    DEFAULT_CONTEXT_BUDGET,
};
use crate::engine::stream::{render_event, ParseResult, StreamParser};
use crate::engine::worktree::{create_worktree, remove_worktree};
use crate::engine::worktrees::{
    create_with_schema, schedule_upstream_sync, worktree_path as wave_worktree_path,
};

use time::OffsetDateTime;

use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
use crate::lfd::output::{OutputEvent, OutputHub};
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::{ForkRun, ForkRunStatus, SharedStore};
use crate::lfd::types::{
    Agent, AgentStatus, Event, Wave, WaveRun, WaveRunSnapshot, WaveRunStatus, WaveStatus,
};

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

pub struct AgentRunner {
    store: SharedStore,
}

impl std::fmt::Debug for AgentRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRunner").finish()
    }
}

impl AgentRunner {
    pub fn new(store: SharedStore) -> Self {
        Self { store }
    }
}

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

        // Record the PID so the process can be killed on stop.
        if let Some(pid) = child.id() {
            let agent_lfd_id = LfdId::from_raw(agent_id);
            let _ = self.store.update_agent_status(
                &agent_lfd_id,
                AgentStatus::Running.as_i32(),
                Some(pid),
            );
        }

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
    let mut parser = StreamParser::new();
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        match parser.feed_line(&line) {
            ParseResult::Events(events) => {
                for event in &events {
                    let (stdout, stderr) = render_event(event, false);
                    let text = if !stdout.is_empty() { stdout } else { stderr };
                    if !text.is_empty() {
                        output.send(OutputEvent {
                            wave_id: wave_id.clone(),
                            wave_run_id: wave_run_id.clone(),
                            agent_id: agent_id.clone(),
                            text,
                        });
                    }
                }
            }
            ParseResult::Skipped => {}
            ParseResult::Passthrough => {
                output.send(OutputEvent {
                    wave_id: wave_id.clone(),
                    wave_run_id: wave_run_id.clone(),
                    agent_id: agent_id.clone(),
                    text: line,
                });
            }
        }
    }
}

#[derive(Clone)]
pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    output: OutputHub,
    runner: Arc<dyn StepRunner>,
    event_hub: EventHub,
}

impl WaveExecutor {
    pub fn new(
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        output: OutputHub,
        event_hub: EventHub,
    ) -> Self {
        let runner = Arc::new(AgentRunner::new(store.clone()));
        Self {
            store,
            scheduler,
            output,
            runner,
            event_hub,
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn with_runner(
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        output: OutputHub,
        event_hub: EventHub,
        runner: Arc<dyn StepRunner>,
    ) -> Self {
        Self {
            store,
            scheduler,
            output,
            runner,
            event_hub,
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
                        self.fail_run(&mut run, &wave, format!("step {} failed", step.step.name))?;
                        return Ok(());
                    }
                }
                FlowAction::WaitInteractive { step } => {
                    let model = step
                        .step
                        .model
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let worktree = run.worktree.clone();
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
                    self.event_hub.send(Event::wave_waiting(
                        wave.id.clone(),
                        run.id.clone(),
                        step.step.name.clone(),
                    ));
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

                    // Auto-create draft PR (best-effort).
                    let worktree = run.worktree.clone();
                    match tokio::task::spawn_blocking(move || auto_create_pr(Path::new(&worktree)))
                        .await
                    {
                        Ok(Some(pr)) => {
                            info!(run_id = %run.id, url = %pr.url, "auto-created draft PR");
                            run.snapshot.pr = Some(pr);
                        }
                        Ok(None) => {}
                        Err(err) => {
                            warn!(run_id = %run.id, error = %err, "failed to auto-create PR");
                        }
                    }

                    self.store.update_wave_run(&run)?;
                    // Wave goes back to Idle after a run completes — the run
                    // is done, but the wave is ready for its next iteration.
                    self.set_wave_status(&wave.id, WaveStatus::Idle);
                    self.event_hub.send(Event::wave_updated(wave.id.clone()));
                    return Ok(());
                }
            }
        }
    }

    fn set_wave_status(&self, wave_id: &LfdId, status: WaveStatus) {
        if let Ok(Some(mut wave)) = self.store.get_wave(wave_id) {
            wave.status = status;
            if let Err(err) = self.store.update_wave(&wave) {
                error!(wave_id = %wave_id, ?status, error = %err, "failed to update wave status");
            }
        }
    }

    fn fail_run(&self, run: &mut WaveRun, wave: &Wave, error: String) -> Result<()> {
        run.status = WaveRunStatus::Failed;
        run.ended_at = Some(OffsetDateTime::now_utc());
        run.error = Some(error);
        self.store.update_wave_run(run)?;
        self.set_wave_status(&wave.id, WaveStatus::Failed);
        self.event_hub.send(Event::wave_updated(wave.id.clone()));
        Ok(())
    }

    async fn run_step(&self, wave: &Wave, run: &mut WaveRun, step: &ConcreteStep) -> Result<i32> {
        let worktree = run.worktree.clone();
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
        self.event_hub.send(Event::agent_started(
            agent_id.clone(),
            step.step.name.clone(),
            worktree.clone(),
        ));

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
        self.event_hub.send(Event::agent_ended(agent_id, status));

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
            self.fail_run(run, wave, "fork has no branches".to_string())?;
            return Ok(());
        }

        let selected = fork
            .branches
            .first()
            .ok_or_else(|| anyhow!("fork has no branches"))?
            .clone();

        if selected.step.interactive.unwrap_or(false) {
            self.fail_run(
                run,
                wave,
                "interactive fork branches are not supported".to_string(),
            )?;
            return Ok(());
        }

        let exit_code = self.run_step(wave, run, &selected).await?;
        if exit_code != 0 {
            self.fail_run(
                run,
                wave,
                format!("fork step {} failed", selected.step.name),
            )?;
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
                self.fail_run(
                    run,
                    wave,
                    "interactive fork branches are not supported".to_string(),
                )?;
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
            let event_hub = self.event_hub.clone();
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
                event_hub.send(Event::agent_started(
                    agent.id.clone(),
                    step.step.name.clone(),
                    worktree.clone(),
                ));

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
                event_hub.send(Event::agent_ended(agent.id.clone(), agent_status));

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
            self.fail_run(run, wave, error)?;
            return Ok(());
        }

        if let Some(step_name) = fork.synthesize.as_deref() {
            let synth_step = ConcreteStep {
                step: Step::named(step_name),
                flow_parents: fork.flow_parents.clone(),
            };
            let exit_code = self.run_step(wave, run, &synth_step).await?;
            if exit_code != 0 {
                self.cleanup_fork(run, &fork_runs).await;
                self.fail_run(run, wave, format!("synthesize {} failed", step_name))?;
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

/// Create a wave run with a worktree and branch for the wave.
pub fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
) -> anyhow::Result<WaveRun> {
    let last_run = store
        .list_wave_runs(Some(&wave.id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let main_repo = Path::new(&wave.repo);
    let (wt_path, branch) = ensure_wave_worktree(main_repo, &wave.name)?;

    let run = WaveRun {
        id: run_id.clone(),
        wave_id: wave.id.clone(),
        snapshot: WaveRunSnapshot {
            repo: wave.repo.clone(),
            flow: wave.flow.clone(),
            direction: wave.direction.clone(),
            area: wave.area.clone(),
            pr: None,
        },
        iteration,
        step_index: 0,
        status: WaveRunStatus::Running,
        worktree: wt_path,
        branch,
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
    };
    store.create_wave_run(&run)?;
    if let Ok(Some(mut wave)) = store.get_wave(&wave.id) {
        wave.status = WaveStatus::Running;
        wave.iteration = iteration;
        if let Err(err) = store.update_wave(&wave) {
            warn!(wave_id = %wave.id, error = %err, "failed to set wave status to running");
        }
    }
    Ok(run)
}

/// Create a worktree for this wave, or reuse the existing one.
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<(String, String)> {
    let wt = wave_worktree_path(main_repo, wave_name);
    if wt.exists() {
        let branch = current_branch(&wt)?.unwrap_or_default();
        if !branch.is_empty() {
            schedule_upstream_sync(wt.clone(), branch.clone());
        }
        return Ok((wt.to_string_lossy().to_string(), branch));
    }

    let config = load_config(Some(main_repo)).ok().flatten();
    let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
    let result = create_with_schema(main_repo, wave_name, None, branch_config)?;
    Ok((result.path.to_string_lossy().to_string(), result.branch))
}

/// Kill a process by PID using SIGTERM.
pub fn kill_process(pid: u32) {
    let _ = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
}

fn fork_worktree_path(run: &WaveRun, branch_index: u32) -> String {
    format!("{}-fork-{branch_index}", run.worktree)
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

    let mut components = gather_context(&opts)?;
    let repo_root = PathBuf::from(worktree);
    drop_native_instruction_docs(&mut components, &repo_root);
    let (components, _breakdown) = trim_context_with_breakdown(components, DEFAULT_CONTEXT_BUDGET);

    // Log full prompt, then write context/task split for --append-system-prompt-file
    let _ = write_prompt_log(
        &repo_root,
        &format_prompt(&components),
        &step.step.name,
        None,
    );
    let task_prompt = format_task_prompt(&components);
    let context_file = write_prompt_log(
        &repo_root,
        &format_context_prompt(&components),
        &format!("{}.context", step.step.name),
        None,
    )
    .ok();

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
        cwd: Some(repo_root),
        context_file,
        ..Default::default()
    };

    Ok((task_prompt, model, launch))
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

/// Commit any remaining changes, push, and create a draft PR.
/// Returns the PR info if successful, None if skipped or failed.
fn auto_create_pr(worktree: &Path) -> Option<crate::lfd::types::PullRequest> {
    use crate::ops::{
        commit_workflow, current_pr, generate_pr_message, update_pr, CommitOptions, NullProgress,
    };

    let commit_options = CommitOptions {
        add: true,
        lint: false,
        push: true,
        create_draft_pr: true,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: None,
    };
    if let Err(err) = commit_workflow(worktree, &commit_options, &NullProgress) {
        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: commit/push failed");
        return None;
    }

    match current_pr(worktree) {
        Ok(Some(pr)) => {
            // Update the draft PR with an LLM-generated title and description,
            // matching what `lf ops pr` produces.
            match generate_pr_message(worktree) {
                Ok(message) => {
                    if let Err(err) = update_pr(worktree, pr.number, &message.title, &message.body)
                    {
                        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to update title/body");
                    }
                }
                Err(err) => {
                    warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to generate PR message");
                }
            }

            Some(crate::lfd::types::PullRequest {
                url: pr.url,
                number: Some(pr.number as u32),
                state: Some(pr.state),
                branch: Some(pr.branch),
                title: None,
            })
        }
        Ok(None) => {
            debug!(worktree = %worktree.display(), "auto-create PR: no PR found after push");
            None
        }
        Err(err) => {
            warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to fetch PR info");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::{AsyncWriteExt, DuplexStream};

    async fn write_lines(mut writer: DuplexStream, lines: &[&str]) {
        for line in lines {
            writer
                .write_all(line.as_bytes())
                .await
                .expect("writer should accept line");
            writer
                .write_all(b"\n")
                .await
                .expect("writer should accept newline");
        }
        writer.shutdown().await.expect("writer should shut down");
    }

    #[tokio::test]
    async fn read_stream_renders_stream_json_events() {
        let output_dir = tempdir().expect("tempdir should be created");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let (writer, reader) = tokio::io::duplex(4096);

        let write_task = tokio::spawn(write_lines(
            writer,
            &[
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/lib.rs"}}]}}"#,
                r#"{"type":"result","subtype":"success"}"#,
            ],
        ));

        read_stream(
            reader,
            output.clone(),
            "wave-1".to_string(),
            "run-1".to_string(),
            "agent-1".to_string(),
        )
        .await;

        write_task.await.expect("writer task should complete");

        let lines = output.read_log("run-1").expect("output log should exist").0;

        assert_eq!(lines, vec!["hello", "-> Read  src/lib.rs", "ok"]);
    }

    #[tokio::test]
    async fn read_stream_skips_known_events_and_passes_through_unknown_lines() {
        let output_dir = tempdir().expect("tempdir should be created");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let (writer, reader) = tokio::io::duplex(4096);

        let write_task = tokio::spawn(write_lines(
            writer,
            &[
                r#"{"type":"system","message":"skip me"}"#,
                r#"{"type":"mystery","payload":42}"#,
                "plain text line",
            ],
        ));

        read_stream(
            reader,
            output.clone(),
            "wave-1".to_string(),
            "run-2".to_string(),
            "agent-1".to_string(),
        )
        .await;

        write_task.await.expect("writer task should complete");

        let lines = output.read_log("run-2").expect("output log should exist").0;

        assert_eq!(
            lines,
            vec![r#"{"type":"mystery","payload":42}"#, "plain text line"]
        );
    }
}
