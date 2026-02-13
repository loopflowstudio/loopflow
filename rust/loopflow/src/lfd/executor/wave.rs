use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use time::OffsetDateTime;

use crate::engine::agent::build_agent_command;
use crate::engine::agent::LaunchConfig;
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::config::load_config_or_default;
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteFork, ConcreteItem, ConcreteStep, FlowAction,
    ForkSelect, Step,
};
use crate::engine::git::hash_areas;
use crate::engine::worktree::{create_worktree, remove_worktree};

use crate::lfd::config::{ExecutorConfig, ExecutorType};
use crate::lfd::events::EventHub;
use crate::lfd::executor::docker::DockerExecutor;
use crate::lfd::executor::helpers::{
    advance_branch, auto_create_pr, build_agent_for_step, build_step_prompt,
    flow_parents_for_index, fork_worktree_path,
};
use crate::lfd::executor::local::LocalProcessExecutor;
use crate::lfd::executor::{AgentExecutor, StartupRecovery};
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::{ForkRun, ForkRunStatus, SharedStore};
use crate::lfd::types::{
    AgentStatus, Event, StimulusKind, Summary, Wave, WaveRun, WaveRunStatus, WaveStatus,
};

use std::path::PathBuf;

#[derive(Clone)]
pub struct WaveExecutor {
    pub(crate) store: SharedStore,
    pub(crate) scheduler: Arc<Scheduler>,
    pub(crate) output: OutputHub,
    pub(crate) runner: Arc<dyn AgentExecutor>,
    pub(crate) event_hub: EventHub,
    pub(crate) executor_type: ExecutorType,
}

impl WaveExecutor {
    pub fn new(
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        output: OutputHub,
        event_hub: EventHub,
        config: ExecutorConfig,
    ) -> Result<Self> {
        let executor_type = config.r#type;
        let runner: Arc<dyn AgentExecutor> = match executor_type {
            ExecutorType::Docker => Arc::new(DockerExecutor::new(store.clone(), &config)?),
            ExecutorType::Local => Arc::new(LocalProcessExecutor::new(store.clone())),
        };
        Ok(Self {
            store,
            scheduler,
            output,
            runner,
            event_hub,
            executor_type,
        })
    }

    #[cfg(test)]
    pub fn with_runner(
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        output: OutputHub,
        event_hub: EventHub,
        runner: Arc<dyn AgentExecutor>,
    ) -> Self {
        Self {
            store,
            scheduler,
            output,
            runner,
            event_hub,
            executor_type: ExecutorType::Local,
        }
    }

    pub fn executor_type(&self) -> ExecutorType {
        self.executor_type
    }

    pub async fn recover_startup(&self) -> Result<StartupRecovery> {
        self.runner.recover_startup(&self.output).await
    }

    pub async fn cleanup_wave_workspace(&self, wave: &Wave) -> Result<()> {
        self.runner.cleanup_wave(wave).await
    }

    pub async fn terminate_agent(&self, agent_id: &LfdId) -> Result<()> {
        self.runner.terminate(agent_id.as_str()).await
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
                    // Ensure area summary is fresh before each step
                    if let Err(err) = self.ensure_summary_fresh(&wave, &run).await {
                        warn!(run_id = %run.id, error = %err, "summary refresh failed, continuing");
                    }
                    info!(run_id = %run.id, step = %step.step.name, step_index = run.step_index, "running step");
                    let exit_code = self.run_step(&wave, &mut run, &step).await?;
                    if exit_code == 0 {
                        run.step_index += 1;
                        run.status = WaveRunStatus::Running;
                        run.flow_parents = flow_parents_for_index(&plan, run.step_index);
                        self.store.update_wave_run(&run)?;
                        self.event_hub.send(Event::wave_updated(wave.id.clone()));
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
                            "running fork (all branches)"
                        );
                        self.run_fork(&wave, &mut run, &plan, &fork).await?;
                        if run.status == WaveRunStatus::Failed {
                            return Ok(());
                        }
                    }
                    ForkSelect::One | ForkSelect::Prompt { .. } => {
                        info!(run_id = %run.id, step_index = run.step_index, "running fork (choose)");
                        self.run_choose(&wave, &mut run, &plan, &fork).await?;
                        if run.status == WaveRunStatus::Failed {
                            return Ok(());
                        }
                    }
                },
                FlowAction::Complete => {
                    run.status = WaveRunStatus::Completed;
                    run.ended_at = Some(OffsetDateTime::now_utc());

                    let is_recurring = self
                        .store
                        .list_stimuli(Some(&wave.id))
                        .map(|stimuli| {
                            stimuli.iter().any(|s| {
                                matches!(
                                    s.kind,
                                    StimulusKind::Loop | StimulusKind::Watch | StimulusKind::Cron
                                )
                            })
                        })
                        .unwrap_or(false);

                    // Auto-create PR (draft for manual, ready for auto stimulus).
                    let worktree = run.worktree.clone();
                    match tokio::task::spawn_blocking(move || {
                        auto_create_pr(Path::new(&worktree), is_recurring)
                    })
                    .await
                    {
                        Ok(Some(pr)) => {
                            info!(run_id = %run.id, url = %pr.url, "auto-created PR");
                            run.snapshot.pr = Some(pr);
                        }
                        Ok(None) => {}
                        Err(err) => {
                            warn!(run_id = %run.id, error = %err, "failed to auto-create PR");
                        }
                    }

                    // For recurring waves, advance to a new branch so the
                    // next iteration gets its own PR.
                    if run.snapshot.pr.is_some() && is_recurring {
                        let wt = run.worktree.clone();
                        let name = wave.name.clone();
                        match tokio::task::spawn_blocking(move || {
                            advance_branch(Path::new(&wt), &name)
                        })
                        .await
                        {
                            Ok(Ok(new_branch)) => {
                                info!(
                                    run_id = %run.id,
                                    new_branch = %new_branch,
                                    "advanced to new branch for next iteration"
                                );
                            }
                            Ok(Err(err)) => {
                                warn!(
                                    run_id = %run.id,
                                    error = %err,
                                    "failed to advance branch"
                                );
                            }
                            Err(err) => {
                                warn!(
                                    run_id = %run.id,
                                    error = %err,
                                    "advance_branch task panicked"
                                );
                            }
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
        let (prompt, model, launch) = {
            let worktree = worktree.clone();
            let step_clone = step.clone();
            let directions = run.snapshot.direction.clone();
            let wave_name = wave.name.clone();
            let store = self.store.clone();
            let wave_id = wave.id.clone();
            tokio::task::spawn_blocking(move || {
                build_step_prompt(
                    &worktree,
                    &step_clone,
                    &directions,
                    Some(&wave_name),
                    Some((&store, &wave_id)),
                )
            })
            .await
            .expect("build_step_prompt task panicked")?
        };
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

    // Summary management

    /// Check if the wave's area summary is fresh; regenerate if stale or missing.
    pub(crate) async fn ensure_summary_fresh(&self, wave: &Wave, run: &WaveRun) -> Result<()> {
        if wave.area.is_empty() {
            return Ok(());
        }

        let worktree_path = Path::new(&run.worktree);
        let current_hash = match hash_areas(worktree_path, &wave.area) {
            Ok(h) => h,
            Err(err) => {
                warn!(wave = %wave.name, error = %err, "failed to hash areas, skipping summary");
                return Ok(());
            }
        };

        if let Ok(Some(existing)) = self.store.get_summary(&wave.id) {
            if existing.source_hash == current_hash {
                debug!(wave = %wave.name, "summary is fresh");
                return Ok(());
            }
            info!(wave = %wave.name, "summary is stale, regenerating");
        } else {
            info!(wave = %wave.name, "no summary found, generating");
        }

        self.run_internal_summarize(wave, run, &current_hash).await
    }

    /// Run the builtin summarize step as an internal agent and store the result.
    async fn run_internal_summarize(
        &self,
        wave: &Wave,
        run: &WaveRun,
        source_hash: &str,
    ) -> Result<()> {
        let template = get_builtin_ops_prompt("summarize")
            .ok_or_else(|| anyhow!("builtin summarize prompt not found"))?;

        let config = load_config_or_default(Some(Path::new(&run.worktree)));
        let token_budget = config.summary_tokens;

        // Build the prompt with area paths as content guidance
        let area_list = wave.area.join(", ");
        let prompt = template
            .replace("{token_budget}", &token_budget.to_string())
            .replace(
                "{content}",
                &format!("Read and summarize these paths: {area_list}"),
            );

        let model = config.agent_model.clone();
        let launch = LaunchConfig {
            auto: true,
            stream: true,
            skip_permissions: config.yolo,
            model_variant: None,
            chrome: false,
            cwd: Some(PathBuf::from(&run.worktree)),
            context_file: None,
            ..Default::default()
        };

        let cmd = build_agent_command(&model, &prompt, &launch);
        info!(wave = %wave.name, model = %model, "running internal summarize step");

        let step = ConcreteStep {
            step: Step {
                name: "_summarize".to_string(),
                model: Some(model.clone()),
                directions: Vec::new(),
                interactive: Some(false),
                content: None,
            },
            flow_parents: Vec::new(),
        };

        let agent = build_agent_for_step(
            &run.id,
            &run.snapshot.repo,
            &run.worktree,
            &step,
            AgentStatus::Running,
            &config.agent_model,
        );
        let agent_id = agent.id.clone();
        self.store.start_agent(&agent)?;

        let exit_code = self
            .runner
            .run(
                cmd,
                Path::new(&run.worktree),
                wave.id.as_str(),
                agent_id.as_str(),
                run.id.as_str(),
                &self.output,
            )
            .await?;

        let ended_at = OffsetDateTime::now_utc().unix_timestamp();
        let status = if exit_code == 0 {
            AgentStatus::Completed
        } else {
            AgentStatus::Failed
        };
        self.store.end_agent(&agent_id, status.as_i32(), ended_at)?;

        if exit_code != 0 {
            warn!(wave = %wave.name, exit_code, "summarize step failed, continuing without summary");
            return Ok(());
        }

        // Read the summary file the agent wrote
        let summary_path = Path::new(&run.worktree).join(".lf/summary.md");
        match std::fs::read_to_string(&summary_path) {
            Ok(content) if !content.trim().is_empty() => {
                let summary = Summary {
                    id: LfdId::new(),
                    wave_id: wave.id.clone(),
                    content,
                    source_hash: source_hash.to_string(),
                    token_budget: token_budget as u32,
                    model: config.agent_model,
                    created_at: Some(OffsetDateTime::now_utc()),
                };
                self.store.upsert_summary(&summary)?;
                info!(wave = %wave.name, "summary stored");
            }
            Ok(_) => {
                warn!(wave = %wave.name, "summarize step produced empty output");
            }
            Err(err) => {
                warn!(wave = %wave.name, error = %err, "failed to read summary file");
            }
        }

        Ok(())
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
        self.event_hub.send(Event::wave_updated(wave.id.clone()));
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

            let fork_wt = fork_worktree_path(run, index as u32);
            if !Path::new(&fork_wt).exists() {
                debug!(
                    run_id = %run.id,
                    branch_index = index,
                    step = %branch.step.name,
                    worktree = %fork_wt,
                    "creating fork worktree"
                );
                create_worktree(
                    Path::new(&run.snapshot.repo),
                    Path::new(&fork_wt),
                    &format!("{}-fork-{}", run.id, index),
                )?;
            }

            let fork_run = ForkRun {
                id: LfdId::new(),
                wave_run_id: run.id.clone(),
                step_index: run.step_index,
                branch_index: index as u32,
                status: ForkRunStatus::Pending,
                worktree: fork_wt,
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
                let prompt = {
                    let worktree = worktree.clone();
                    let step = step.clone();
                    let wave_directions = wave_directions.clone();
                    let store = store.clone();
                    let fork_wave_id = fork_wave_id.clone();
                    tokio::task::spawn_blocking(move || {
                        build_step_prompt(
                            &worktree,
                            &step,
                            &wave_directions,
                            None,
                            Some((&store, &fork_wave_id)),
                        )
                    })
                    .await
                    .expect("build_step_prompt task panicked")
                };
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

        let synth_step = ConcreteStep {
            step: Step::named("synthesize"),
            flow_parents: fork.flow_parents.clone(),
        };
        let exit_code = self.run_step(wave, run, &synth_step).await?;
        if exit_code != 0 {
            self.cleanup_fork(run, &fork_runs).await;
            self.fail_run(run, wave, "synthesize failed".to_string())?;
            return Ok(());
        }

        self.cleanup_fork(run, &fork_runs).await;
        run.step_index += 1;
        run.status = WaveRunStatus::Running;
        run.flow_parents = flow_parents_for_index(plan, run.step_index);
        self.store.update_wave_run(run)?;
        self.event_hub.send(Event::wave_updated(wave.id.clone()));
        Ok(())
    }

    async fn cleanup_fork(&self, run: &WaveRun, fork_runs: &[(ForkRun, ConcreteStep)]) {
        let mut tasks = tokio::task::JoinSet::new();
        for (fork_run, _) in fork_runs {
            let worktree = fork_run.worktree.clone();
            tasks.spawn_blocking(move || {
                let worktree_path = Path::new(&worktree);
                if worktree_path.join(".git").exists() {
                    let _ = remove_worktree(worktree_path, true);
                }
            });
        }
        while tasks.join_next().await.is_some() {}
        for (fork_run, _) in fork_runs {
            self.scheduler.release(fork_run.id.as_str());
        }
        let _ = self.store.delete_fork_runs(&run.id, run.step_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::store::sqlite::SqliteStore;
    use crate::lfd::types::WaveRunSnapshot;
    use async_trait::async_trait;
    use tempfile::tempdir;

    struct MockRunner;

    #[async_trait]
    impl AgentExecutor for MockRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            _cwd: &Path,
            _wave_id: &str,
            _agent_id: &str,
            _wave_run_id: &str,
            _output: &OutputHub,
        ) -> Result<i32> {
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn execute_emits_wave_updated_on_step_advance() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path();

        // Create a two-step flow
        let flow_dir = repo.join(".lf/flows");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::write(flow_dir.join("test-flow.yaml"), "- step-a\n- step-b\n").unwrap();

        // Create step files so load_step resolves
        let step_dir = repo.join(".lf/steps");
        std::fs::create_dir_all(&step_dir).unwrap();
        std::fs::write(step_dir.join("step-a.md"), "do step a").unwrap();
        std::fs::write(step_dir.join("step-b.md"), "do step b").unwrap();

        // Set up store
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(SqliteStore::new(&db_path).unwrap());

        let wave_id = LfdId::new();
        let run_id = LfdId::new();

        let wave = Wave {
            id: wave_id.clone(),
            name: "test-wave".to_string(),
            repo: repo.to_string_lossy().to_string(),
            flow: "test-flow".to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Running,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.create_wave(&wave).unwrap();

        let run = WaveRun {
            id: run_id.clone(),
            wave_id: wave_id.clone(),
            snapshot: WaveRunSnapshot {
                repo: repo.to_string_lossy().to_string(),
                flow: "test-flow".to_string(),
                direction: vec![],
                area: vec![],
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: repo.to_string_lossy().to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
        };
        store.create_wave_run(&run).unwrap();

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let mut rx = event_hub.subscribe();

        let executor = WaveExecutor::with_runner(
            store.clone(),
            scheduler,
            output,
            event_hub,
            Arc::new(MockRunner),
        );

        executor.execute(&run_id).await.unwrap();

        // Collect all wave_updated events
        let mut wave_updated_count = 0;
        while let Ok(event) = rx.try_recv() {
            let json = serde_json::to_value(&event).unwrap();
            if json["type"] == "wave_updated" {
                wave_updated_count += 1;
            }
        }

        // Two steps means two step advances (step-a -> step-b, step-b -> complete),
        // plus one final wave_updated when the run completes.
        // After step-a: step_index 0->1, emit wave_updated
        // After step-b: step_index 1->2, emit wave_updated
        // Run completes: emit wave_updated
        assert_eq!(
            wave_updated_count, 3,
            "expected wave_updated after each step advance and on completion"
        );
    }

    #[tokio::test]
    async fn execute_runs_fork_all_with_docker_executor() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path().canonicalize().expect("canonical repo path");

        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("init")
            .status()
            .expect("git init should run");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .expect("git config email should run");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["config", "user.name", "Test User"])
            .status()
            .expect("git config name should run");
        std::fs::write(repo.join("README.md"), "# test\n").expect("seed file should be written");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "."])
            .status()
            .expect("git add should run");
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["commit", "-m", "initial"])
            .status()
            .expect("git commit should run");

        let flow_dir = repo.join(".lf/flows");
        std::fs::create_dir_all(&flow_dir).expect("flow dir should exist");
        std::fs::write(
            flow_dir.join("fork-flow.yaml"),
            r#"
- fork:
    branches:
      - step: { name: step-a }
      - step: { name: step-b }
    select: all
"#,
        )
        .expect("flow file should be written");

        let step_dir = repo.join(".lf/steps");
        std::fs::create_dir_all(&step_dir).expect("step dir should exist");
        std::fs::write(step_dir.join("step-a.md"), "do step a").expect("step file should write");
        std::fs::write(step_dir.join("step-b.md"), "do step b").expect("step file should write");

        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(SqliteStore::new(&db_path).expect("db should open"));

        let wave_id = LfdId::new();
        let run_id = LfdId::new();
        let wave = Wave {
            id: wave_id.clone(),
            name: "fork-wave".to_string(),
            repo: repo.to_string_lossy().to_string(),
            flow: "fork-flow".to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Running,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.create_wave(&wave).expect("wave should be created");

        let run = WaveRun {
            id: run_id.clone(),
            wave_id: wave_id.clone(),
            snapshot: WaveRunSnapshot {
                repo: repo.to_string_lossy().to_string(),
                flow: "fork-flow".to_string(),
                direction: vec![],
                area: vec![],
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: repo.to_string_lossy().to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
        };
        store
            .create_wave_run(&run)
            .expect("wave run should be created");

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let executor = WaveExecutor {
            store: store.clone(),
            scheduler,
            output,
            runner: Arc::new(MockRunner),
            event_hub,
            executor_type: ExecutorType::Docker,
        };

        executor
            .execute(&run_id)
            .await
            .expect("execution should finish");

        let updated_run = store
            .get_wave_run(&run_id)
            .expect("run fetch should succeed")
            .expect("run should exist");
        assert_ne!(
            updated_run.error.as_deref(),
            Some("fork(select=all) is not supported by the docker executor yet")
        );
    }
}
