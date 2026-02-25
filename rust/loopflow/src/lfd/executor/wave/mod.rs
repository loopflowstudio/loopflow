mod fork;
mod launch;
mod sidecar;
mod summary;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tracing::{debug, error, info, warn};

use time::OffsetDateTime;

use crate::engine::agent::build_agent_command;
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteItem, ConcreteStep, FlowAction,
};
use crate::engine::worktree::remove_worktree;
use crate::lfd::config::{ExecutorConfig, ExecutorType, GitHubConfig};
use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::{ExecutionStore, ForkRunStatus, SharedStore};
use crate::lfd::types::{
    AgentStatus, Event, LivePrState, LivePullRequestState, StimulusKind, Wave, WaveRun,
    WaveRunKind, WaveRunStatus, WaveStatus,
};

use super::docker::DockerExecutor;
use super::helpers::{
    advance_branch, auto_create_pr, build_agent_capabilities, build_agent_for_step,
    build_step_prompt, flow_parents_for_index, is_active_wave_run_status,
    is_ephemeral_worktree_path,
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
    github_config: GitHubConfig,
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
        github_config: GitHubConfig,
    ) -> Result<Self> {
        let executor_type = config.r#type;
        let runner: Arc<dyn AgentExecutor> = match executor_type {
            ExecutorType::Docker => Arc::new(DockerExecutor::new(store.clone(), &config)?),
            ExecutorType::Local => Arc::new(LocalProcessExecutor::new(
                store.clone(),
                config.agent_timeout,
            )),
        };
        Ok(Self {
            store,
            scheduler,
            output,
            runner,
            event_hub,
            executor_type,
            github_config,
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
            github_config: GitHubConfig::default(),
        }
    }

    pub fn executor_type(&self) -> ExecutorType {
        self.executor_type
    }

    pub async fn recover_startup(&self) -> Result<StartupRecovery> {
        let orphaned = self.cleanup_orphaned_fork_runs().await?;
        let mut recovery = self.runner.recover_startup(&self.output).await?;
        recovery.orphaned_fork_runs_cleaned = orphaned.cleaned_runs;
        recovery.orphaned_fork_worktrees_removed = orphaned.removed_worktrees;
        Ok(recovery)
    }

    pub async fn ensure_wave_workspace(&self, wave: &Wave) -> Result<()> {
        self.runner.ensure_wave_workspace(wave).await
    }

    pub async fn cleanup_wave_workspace(&self, wave: &Wave) -> Result<()> {
        self.runner.cleanup_wave_workspace(wave).await
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
            let run_is_active = is_active_wave_run_status(run.status);
            if run_is_active && run.run_kind == WaveRunKind::Sidecar {
                active.push(EphemeralWorktree {
                    path: run.worktree.clone(),
                    owner_kind: EphemeralOwnerKind::Sidecar,
                    owner_id: run.id.to_string(),
                });
            }

            if !run_is_active {
                continue;
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

    async fn cleanup_orphaned_fork_runs(&self) -> Result<OrphanedForkCleanup> {
        let orphaned_runs = self.store.list_orphaned_fork_runs().await?;
        if orphaned_runs.is_empty() {
            return Ok(OrphanedForkCleanup::default());
        }

        let mut removed_worktrees = 0u32;
        let mut stale_groups = HashSet::new();
        let mut repo_cache: HashMap<LfdId, Option<String>> = HashMap::new();
        for fork_run in &orphaned_runs {
            stale_groups.insert((fork_run.wave_run_id.clone(), fork_run.step_index));

            let worktree_path = Path::new(&fork_run.worktree);
            let repo = if let Some(repo) = repo_cache.get(&fork_run.wave_run_id) {
                repo.clone()
            } else {
                let run_repo = self
                    .store
                    .get_wave_run(&fork_run.wave_run_id)
                    .await?
                    .map(|run| run.snapshot.repo);
                repo_cache.insert(fork_run.wave_run_id.clone(), run_repo.clone());
                run_repo
            };
            let Some(repo) = repo else {
                warn!(
                    wave_run_id = %fork_run.wave_run_id,
                    worktree = %worktree_path.display(),
                    "unable to resolve repo for orphaned fork worktree cleanup"
                );
                continue;
            };

            match self
                .runner
                .cleanup_ephemeral_worktree(Path::new(&repo), worktree_path)
                .await
            {
                Ok(()) => removed_worktrees += 1,
                Err(err) => warn!(
                    worktree = %worktree_path.display(),
                    error = %err,
                    "failed removing orphaned fork worktree"
                ),
            }
        }

        let mut cleaned_runs = 0u32;
        for (wave_run_id, step_index) in stale_groups {
            match self.store.delete_fork_runs(&wave_run_id, step_index).await {
                Ok(deleted) => cleaned_runs += deleted,
                Err(err) => warn!(
                    wave_run_id = %wave_run_id,
                    step_index,
                    error = %err,
                    "failed deleting orphaned fork run records"
                ),
            }
        }

        Ok(OrphanedForkCleanup {
            cleaned_runs,
            removed_worktrees,
        })
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
                        self.advance_run_step(&mut run, &plan, wave.id()).await?;
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
                    self.set_wave_status(wave.id(), WaveStatus::Waiting).await;
                    self.event_hub.send(Event::wave_waiting(
                        wave.id().clone(),
                        run.id.clone(),
                        step.step.name.clone(),
                    ));
                    return Ok(());
                }
                FlowAction::Fork { fork } => {
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
                FlowAction::Complete => {
                    run.status = WaveRunStatus::Completed;
                    run.ended_at = Some(OffsetDateTime::now_utc());

                    let is_recurring = self
                        .store
                        .list_stimuli(Some(wave.id()))
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

                    // Auto-create PR as draft; queue reconciliation promotes the queue head.
                    let worktree = run.worktree.clone();
                    match tokio::task::spawn_blocking(move || auto_create_pr(Path::new(&worktree)))
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

                    if let Some(pr) = run.snapshot.pr.as_ref() {
                        if let Some(pr_number) = pr.number {
                            let live_state = LivePullRequestState {
                                repo_id: run.snapshot.repo.clone(),
                                pr_number,
                                state: LivePrState::Open,
                                is_draft: pr
                                    .state
                                    .as_deref()
                                    .is_some_and(|value| value.eq_ignore_ascii_case("draft")),
                                head_ref: run.branch.clone(),
                                head_sha: String::new(),
                                base_ref: "main".to_string(),
                                updated_at: OffsetDateTime::now_utc(),
                                merged_at: None,
                                synced_at: OffsetDateTime::now_utc(),
                            };
                            if let Err(err) = self.store.upsert_live_pr_state(&live_state).await {
                                warn!(
                                    run_id = %run.id,
                                    error = %err,
                                    "failed to upsert live PR state after PR creation"
                                );
                            }
                        }
                    }

                    // For recurring waves, advance to a new branch so the
                    // next iteration gets its own PR.
                    if run.snapshot.pr.is_some() && is_recurring {
                        let wt = run.worktree.clone();
                        let name = wave.name().clone();
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
                    if run.snapshot.pr.is_some() {
                        if let Err(err) = crate::lfd::queue::reconcile_wave_queue(
                            &self.store,
                            &self.github_config,
                            wave.id(),
                            crate::lfd::queue::QueueTrigger::RunCompleted,
                        )
                        .await
                        {
                            warn!(wave_id = %wave.id(), error = %err, "queue reconcile failed after run completion");
                        }
                    }
                    // Wave goes back to Idle after a run completes — the run
                    // is done, but the wave is ready for its next iteration.
                    self.set_wave_status(wave.id(), WaveStatus::Idle).await;
                    self.event_hub.send(Event::wave_updated(wave.id().clone()));
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
        self.set_wave_status(wave.id(), WaveStatus::Failed).await;
        self.event_hub.send(Event::wave_updated(wave.id().clone()));
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
        let (launch, process) = build_step_prompt(
            &worktree,
            step,
            &run.snapshot.direction,
            Some(wave.name()),
            Some((&self.store, wave.id())),
            None,
        )
        .await?;
        let capabilities = build_agent_capabilities(&worktree);
        let model = launch.model.clone().unwrap_or_else(|| "claude".to_string());
        info!(run_id = %run.id, step = %step.step.name, model = %model, "launching agent");

        let outcome = self
            .launch_agent(AgentLaunchRequest {
                wave_id: run.wave_id.clone(),
                wave_run_id: run.id.clone(),
                branch: Some(run.branch.clone()),
                repo: run.snapshot.repo.clone(),
                worktree,
                step: step.clone(),
                model: model.clone(),
                cmd: build_agent_command(&launch, &process, &capabilities),
                output_prefix: None,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct OrphanedForkCleanup {
    cleaned_runs: u32,
    removed_worktrees: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::worktree::create_worktree;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::WaveRunSnapshot;
    use async_trait::async_trait;
    use loopflow_test_support::TestRepo;
    use std::sync::Mutex;
    use tempfile::tempdir;

    struct MockRunner;

    #[async_trait]
    impl AgentExecutor for MockRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            _cwd: &Path,
            _context: super::super::AgentRunContext<'_>,
        ) -> Result<i32> {
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Debug, Clone, Default)]
    struct ForkRunnerCall {
        cwd: String,
        branch: Option<String>,
        output_prefix: Option<String>,
        prompt_logs: String,
    }

    #[derive(Debug, Default)]
    struct ForkTestRunner {
        fail_suffix: Option<String>,
        fail_code: i32,
        calls: Mutex<Vec<ForkRunnerCall>>,
    }

    #[async_trait]
    impl AgentExecutor for ForkTestRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            cwd: &Path,
            context: super::super::AgentRunContext<'_>,
        ) -> Result<i32> {
            let call = ForkRunnerCall {
                cwd: cwd.to_string_lossy().to_string(),
                branch: context.branch.map(str::to_string),
                output_prefix: context.output_prefix.map(str::to_string),
                prompt_logs: read_prompt_logs(cwd),
            };
            self.calls.lock().expect("runner mutex").push(call);

            if let Some(suffix) = &self.fail_suffix {
                if cwd.to_string_lossy().ends_with(suffix) {
                    return Ok(self.fail_code);
                }
            }
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
    }

    fn read_prompt_logs(worktree: &Path) -> String {
        let log_dir = worktree.join(".lf/log");
        if !log_dir.exists() {
            return String::new();
        }
        let mut files = std::fs::read_dir(log_dir)
            .expect("read log dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        files.sort();

        let mut content = String::new();
        for file in files {
            if let Ok(text) = std::fs::read_to_string(&file) {
                content.push_str(&text);
            }
        }
        content
    }

    fn create_fork_flow_repo(repo: &TestRepo, flow: &str) {
        repo.create_file(".lf/flows/fork-flow.yaml", flow);
        repo.create_file(".lf/steps/step-a.md", "do step a");
        repo.create_file(".lf/steps/step-b.md", "do step b");
        repo.stage_all();
        repo.commit("add fork fixtures");
    }

    async fn create_wave_and_run(
        store: &SharedStore,
        repo: &Path,
        flow_name: &str,
    ) -> (LfdId, LfdId) {
        let wave_id = LfdId::new();
        let run_id = LfdId::new();

        let wave = Wave {
            id: wave_id.clone(),
            name: "fork-wave".to_string(),
            repo: repo.to_string_lossy().to_string(),
            flow: flow_name.to_string(),
            direction: vec![],
            area: vec![],
            status: WaveStatus::Running,
            iteration: 0,
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
                flow: flow_name.to_string(),
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
        (wave_id, run_id)
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

        let (_wave_id, run_id) = create_wave_and_run(&store, repo, "test-flow").await;

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
    async fn execute_runs_fork_with_docker_executor() {
        let repo = TestRepo::new();
        let tmp = tempdir().expect("tempdir");
        create_fork_flow_repo(
            &repo,
            r#"
- fork:
    branches:
      - step: { name: step-a }
      - step: { name: step-b }
"#,
        );

        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db should open"),
        );

        let (_wave_id, run_id) = create_wave_and_run(&store, repo.path(), "fork-flow").await;

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
            github_config: GitHubConfig::default(),
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
        assert_eq!(updated_run.status, WaveRunStatus::Completed);
        assert!(updated_run.error.is_none());
    }

    #[tokio::test]
    async fn execute_fork_with_no_branches_fails_cleanly() {
        let repo = TestRepo::new();
        let tmp = tempdir().expect("tempdir");
        create_fork_flow_repo(
            &repo,
            r#"
- fork:
    branches: []
"#,
        );

        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db should open"),
        );
        let (_wave_id, run_id) = create_wave_and_run(&store, repo.path(), "fork-flow").await;

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let runner = Arc::new(ForkTestRunner::default());
        let executor =
            WaveExecutor::with_runner(store.clone(), scheduler, output, event_hub, runner);

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
        assert_eq!(updated_run.error.as_deref(), Some("fork has no branches"));
    }

    #[tokio::test]
    async fn execute_fork_success_cleans_worktrees_and_records() {
        let repo = TestRepo::new();
        let tmp = tempdir().expect("tempdir");
        create_fork_flow_repo(
            &repo,
            r#"
- fork:
    branches:
      - step: { name: step-a }
      - step: { name: step-b }
"#,
        );

        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db should open"),
        );
        let (_wave_id, run_id) = create_wave_and_run(&store, repo.path(), "fork-flow").await;

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let runner = Arc::new(ForkTestRunner::default());
        let executor =
            WaveExecutor::with_runner(store.clone(), scheduler.clone(), output, event_hub, runner);

        executor
            .execute(&run_id)
            .await
            .expect("execution should finish");

        let updated_run = store
            .get_wave_run(&run_id)
            .await
            .expect("run fetch should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, WaveRunStatus::Completed);
        assert!(!Path::new(&(updated_run.worktree.clone() + "-fork-0")).exists());
        assert!(!Path::new(&(updated_run.worktree.clone() + "-fork-1")).exists());
        assert_eq!(
            store
                .list_fork_runs(&run_id, 0)
                .await
                .expect("fork runs should load")
                .len(),
            0
        );
        assert_eq!(scheduler.slots_used(), 0);
    }

    #[tokio::test]
    async fn execute_fork_failure_cleans_worktrees_and_releases_slots() {
        let repo = TestRepo::new();
        let tmp = tempdir().expect("tempdir");
        create_fork_flow_repo(
            &repo,
            r#"
- fork:
    branches:
      - step: { name: step-a }
      - step: { name: step-b }
"#,
        );

        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db should open"),
        );
        let (_wave_id, run_id) = create_wave_and_run(&store, repo.path(), "fork-flow").await;

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let runner = Arc::new(ForkTestRunner {
            fail_suffix: Some("-fork-1".to_string()),
            fail_code: 42,
            ..Default::default()
        });
        let executor =
            WaveExecutor::with_runner(store.clone(), scheduler.clone(), output, event_hub, runner);

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
            Some("1 fork branch(es) failed")
        );
        assert!(!Path::new(&(updated_run.worktree.clone() + "-fork-0")).exists());
        assert!(!Path::new(&(updated_run.worktree.clone() + "-fork-1")).exists());
        assert_eq!(
            store
                .list_fork_runs(&run_id, 0)
                .await
                .expect("fork runs should load")
                .len(),
            0
        );
        assert_eq!(scheduler.slots_used(), 0);
    }

    #[tokio::test]
    async fn execute_fork_merges_directions_and_prefixes_branch_logs() {
        let repo = TestRepo::new();
        let tmp = tempdir().expect("tempdir");
        create_fork_flow_repo(
            &repo,
            r#"
- fork:
    branches:
      - step:
          name: step-a
          directions: [branch]
"#,
        );
        repo.create_file(".lf/directions/base.md", "BASE_DIRECTION_MARKER");
        repo.create_file(".lf/directions/branch.md", "BRANCH_DIRECTION_MARKER");
        repo.stage_all();
        repo.commit("add fork direction fixtures");

        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db should open"),
        );
        let (wave_id, run_id) = create_wave_and_run(&store, repo.path(), "fork-flow").await;

        let mut run = store
            .get_wave_run(&run_id)
            .await
            .expect("run should load")
            .expect("run should exist");
        run.snapshot.direction = vec!["base".to_string()];
        store
            .update_wave_run(&run)
            .await
            .expect("run should update");
        let mut wave = store
            .get_wave(&wave_id)
            .await
            .expect("wave should load")
            .expect("wave should exist");
        wave.direction = vec!["base".to_string()];
        store.update_wave(&wave).await.expect("wave should update");

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let runner = Arc::new(ForkTestRunner::default());
        let runner_ref = runner.clone();
        let executor =
            WaveExecutor::with_runner(store.clone(), scheduler, output, event_hub, runner);

        executor
            .execute(&run_id)
            .await
            .expect("execution should finish");

        let calls = runner_ref.calls.lock().expect("runner mutex");
        assert_eq!(calls.len(), 2);

        let fork_call = calls
            .iter()
            .find(|call| call.output_prefix.as_deref() == Some("[fork-0] "))
            .expect("fork call should be recorded");
        assert!(fork_call.cwd.ends_with("-fork-0"));
        assert_eq!(fork_call.branch, Some(format!("{run_id}-fork-0")));
        assert!(fork_call.prompt_logs.contains("BASE_DIRECTION_MARKER"));
        assert!(fork_call.prompt_logs.contains("BRANCH_DIRECTION_MARKER"));

        let synth_call = calls
            .iter()
            .find(|call| call.output_prefix.is_none() && !call.cwd.ends_with("-fork-0"))
            .expect("synthesize call should be recorded");
        assert_eq!(synth_call.cwd, repo.path().to_string_lossy());
        assert_eq!(synth_call.branch.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn recover_startup_cleans_orphaned_fork_worktree_records() {
        let repo = TestRepo::new();
        let db_dir = tempdir().expect("tempdir");
        let db_path = db_dir.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("db should open"),
        );
        let (_wave_id, run_id) = create_wave_and_run(&store, repo.path(), "fork-flow").await;

        let mut run = store
            .get_wave_run(&run_id)
            .await
            .expect("run should load")
            .expect("run should exist");
        run.status = WaveRunStatus::Failed;
        store
            .update_wave_run(&run)
            .await
            .expect("run should update");

        let fork_worktree = format!("{}-fork-0", repo.path().to_string_lossy());
        create_worktree(
            repo.path(),
            Path::new(&fork_worktree),
            "orphan-fork-recovery-test",
        )
        .expect("fork worktree should be created");

        store
            .upsert_fork_run(&crate::lfd::store::ForkRun {
                id: LfdId::new(),
                wave_run_id: run_id.clone(),
                step_index: 0,
                branch_index: 0,
                status: ForkRunStatus::Running,
                worktree: fork_worktree.clone(),
            })
            .await
            .expect("fork run should be stored");

        let scheduler = Arc::new(Scheduler::new(4));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let executor = WaveExecutor::with_runner(
            store.clone(),
            scheduler,
            output,
            event_hub,
            Arc::new(MockRunner),
        );

        let recovery = executor
            .recover_startup()
            .await
            .expect("startup recovery should succeed");
        assert_eq!(recovery.orphaned_fork_runs_cleaned, 1);
        assert_eq!(recovery.orphaned_fork_worktrees_removed, 1);
        assert!(!Path::new(&fork_worktree).exists());
        assert_eq!(
            store
                .list_fork_runs(&run_id, 0)
                .await
                .expect("fork runs should load")
                .len(),
            0
        );
    }
}
