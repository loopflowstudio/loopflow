mod fork;
mod launch;
mod sidecar;
mod summary;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tracing::{debug, error, info, warn};

use time::OffsetDateTime;

use crate::engine::agent::build_agent_command;
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteItem, ConcreteStep, FlowAction, ForkSelect,
};
use crate::engine::worktree::remove_worktree;
use crate::lfd::config::{ExecutorConfig, ExecutorType};
use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::{ForkRunStatus, SharedStore};
use crate::lfd::types::{
    AgentStatus, Event, StimulusKind, Wave, WaveRun, WaveRunKind, WaveRunStatus, WaveStatus,
};

use super::docker::DockerExecutor;
use super::helpers::{
    advance_branch, auto_create_pr, build_agent_for_step, build_step_prompt,
    flow_parents_for_index, is_active_wave_run_status, is_ephemeral_worktree_path,
};
use super::local::LocalProcessExecutor;
use super::{AgentExecutor, EphemeralOwnerKind, EphemeralWorktree, JanitorReport, StartupRecovery};
use launch::AgentLaunchRequest;

#[derive(Clone)]
pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    output: OutputHub,
    runner: Arc<dyn AgentExecutor>,
    event_hub: EventHub,
    executor_type: ExecutorType,
}

impl std::fmt::Debug for WaveExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaveExecutor")
            .field("executor_type", &self.executor_type)
            .finish()
    }
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

    pub async fn run_worktree_janitor(&self, repo_roots: &[PathBuf]) -> Result<JanitorReport> {
        let active = self.collect_active_ephemeral_worktrees().await?;
        let active_paths: HashSet<String> =
            active.into_iter().map(|worktree| worktree.path).collect();

        let mut roots = HashSet::new();
        for repo_root in repo_roots {
            let canonical = crate::engine::worktrees::main_repo_root(repo_root)
                .unwrap_or_else(|_| repo_root.clone());
            roots.insert(canonical);
        }

        let mut report = JanitorReport {
            active: active_paths.len() as u32,
            ..Default::default()
        };

        for root in roots {
            let worktrees = match crate::engine::worktrees::list_worktrees(&root) {
                Ok(worktrees) => worktrees,
                Err(err) => {
                    warn!(repo = %root.display(), error = %err, "worktree janitor: failed to list worktrees");
                    report.errors += 1;
                    continue;
                }
            };

            for worktree in worktrees {
                let path = worktree.path;
                let path_string = path.to_string_lossy().to_string();
                if !is_ephemeral_worktree_path(&path_string) {
                    continue;
                }
                if active_paths.contains(&path_string) {
                    continue;
                }

                match remove_worktree(&path, true) {
                    Ok(()) => {
                        report.removed += 1;
                    }
                    Err(err) => {
                        warn!(worktree = %path.display(), error = %err, "worktree janitor: failed to remove stale worktree");
                        report.errors += 1;
                    }
                }
            }
        }

        Ok(report)
    }

    async fn collect_active_ephemeral_worktrees(&self) -> Result<Vec<EphemeralWorktree>> {
        let mut active = Vec::new();
        let runs = self.store.list_wave_runs(None, None).await?;

        for run in runs {
            if is_active_wave_run_status(run.status) && run.run_kind == WaveRunKind::Sidecar {
                active.push(EphemeralWorktree {
                    path: run.worktree.clone(),
                    owner_kind: EphemeralOwnerKind::Sidecar,
                    owner_id: run.id.to_string(),
                });
            }

            let forks = self.store.list_fork_runs(&run.id, run.step_index).await?;
            for fork in forks {
                if !matches!(fork.status, ForkRunStatus::Pending | ForkRunStatus::Running) {
                    continue;
                }
                active.push(EphemeralWorktree {
                    path: fork.worktree,
                    owner_kind: EphemeralOwnerKind::Fork,
                    owner_id: fork.id.to_string(),
                });
            }
        }

        Ok(active)
    }

    pub async fn execute(&self, run_id: &LfdId) -> Result<()> {
        let mut run = self
            .store
            .get_wave_run(run_id)
            .await?
            .ok_or_else(|| anyhow!("wave run not found"))?;
        if run.status == WaveRunStatus::Completed || run.status == WaveRunStatus::Failed {
            return Ok(());
        }

        let wave = self
            .store
            .get_wave(&run.wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave not found"))?;
        info!(run_id = %run.id, flow = %run.snapshot.flow, repo = %run.snapshot.repo, "loading flow");
        let flow = load_flow(&run.snapshot.flow, Path::new(&run.snapshot.repo))?;
        let plan = expand_flow(&flow, Path::new(&run.snapshot.repo))?;
        debug!(run_id = %run.id, plan_items = plan.len(), "flow expanded");

        loop {
            let current_flow_parents = flow_parents_for_index(&plan, run.step_index);
            if run.flow_parents != current_flow_parents {
                run.flow_parents = current_flow_parents;
                self.store.update_wave_run(&run).await?;
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
                        self.advance_run_step(&mut run, &plan, &wave.id).await?;
                    } else {
                        self.fail_run(&mut run, &wave, format!("step {} failed", step.step.name))
                            .await?;
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
                    self.store.start_agent(&agent).await?;
                    run.status = WaveRunStatus::Waiting;
                    run.flow_parents = step.flow_parents.clone();
                    self.store.update_wave_run(&run).await?;
                    self.set_wave_status(&wave.id, WaveStatus::Waiting).await;
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
                        .await
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

                    self.store.update_wave_run(&run).await?;
                    // Wave goes back to Idle after a run completes — the run
                    // is done, but the wave is ready for its next iteration.
                    self.set_wave_status(&wave.id, WaveStatus::Idle).await;
                    self.event_hub.send(Event::wave_updated(wave.id.clone()));
                    return Ok(());
                }
            }
        }
    }

    async fn set_wave_status(&self, wave_id: &LfdId, status: WaveStatus) {
        if let Ok(Some(mut wave)) = self.store.get_wave(wave_id).await {
            wave.status = status;
            if let Err(err) = self.store.update_wave(&wave).await {
                error!(wave_id = %wave_id, ?status, error = %err, "failed to update wave status");
            }
        }
    }

    async fn fail_run(&self, run: &mut WaveRun, wave: &Wave, error: String) -> Result<()> {
        run.status = WaveRunStatus::Failed;
        run.ended_at = Some(OffsetDateTime::now_utc());
        run.error = Some(error);
        self.store.update_wave_run(run).await?;
        self.set_wave_status(&wave.id, WaveStatus::Failed).await;
        self.event_hub.send(Event::wave_updated(wave.id.clone()));
        Ok(())
    }

    pub(super) async fn advance_run_step(
        &self,
        run: &mut WaveRun,
        plan: &[ConcreteItem],
        wave_id: &LfdId,
    ) -> Result<()> {
        run.step_index += 1;
        run.status = WaveRunStatus::Running;
        run.flow_parents = flow_parents_for_index(plan, run.step_index);
        self.store.update_wave_run(run).await?;
        self.event_hub.send(Event::wave_updated(wave_id.clone()));
        Ok(())
    }

    async fn run_step(&self, wave: &Wave, run: &mut WaveRun, step: &ConcreteStep) -> Result<i32> {
        let worktree = run.worktree.clone();
        debug!(run_id = %run.id, step = %step.step.name, worktree = %worktree, "building step prompt");
        let (prompt, model, launch) = build_step_prompt(
            &worktree,
            step,
            &run.snapshot.direction,
            Some(&wave.name),
            Some((&self.store, &wave.id)),
            None,
        )
        .await?;
        info!(run_id = %run.id, step = %step.step.name, model = %model, "launching agent");

        let outcome = self
            .launch_agent(AgentLaunchRequest {
                wave_id: run.wave_id.clone(),
                wave_run_id: run.id.clone(),
                repo: run.snapshot.repo.clone(),
                worktree,
                step: step.clone(),
                model: model.clone(),
                cmd: build_agent_command(&model, &prompt, &launch),
            })
            .await?;

        debug!(
            run_id = %run.id,
            step = %step.step.name,
            agent_id = %outcome.agent_id,
            exit_code = outcome.exit_code,
            "step agent finished"
        );

        Ok(outcome.exit_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::store::{open_store, StorageConfig};
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
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

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
            schema_ref: None,
            schema_name: None,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.create_wave(&wave).await.unwrap();

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
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave_id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store.create_wave_run(&run).await.unwrap();

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
    async fn execute_fails_fork_all_with_docker_executor() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path();

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
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db should open"),
        );

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
            schema_ref: None,
            schema_name: None,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .create_wave(&wave)
            .await
            .expect("wave should be created");

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
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave_id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store
            .create_wave_run(&run)
            .await
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
            .await
            .expect("run fetch should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, WaveRunStatus::Failed);
        assert_eq!(
            updated_run.error.as_deref(),
            Some("fork(select=all) is not supported by the docker executor yet")
        );
    }
}
