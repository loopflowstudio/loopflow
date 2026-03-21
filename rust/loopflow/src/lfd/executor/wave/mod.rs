mod fork;
mod launch;
mod summary;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use time::OffsetDateTime;

use crate::engine::agent::{build_agent_command, build_agent_env, AgentConfig, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::fast_path::{try_fast_path, FailureContext, FastPathResult};
use crate::engine::flow::{
    build_xor_routing_suffix, expand_flow, load_flow, load_step, load_xor_path_items, next_action,
    read_xor_verdict, ConcreteItem, ConcreteStep, FlowAction, Step,
};
use crate::engine::launch::{prepare_launch_prompt, LaunchPromptInput};
use crate::engine::prompt::{write_prompt_log, Surface};
use crate::engine::structured_reply::ClientContext;
use crate::engine::worktree::remove_worktree;
use crate::lfd::attention::resolve_attention_item;
use crate::lfd::config::{ExecutorConfig, ExecutorType, GitHubConfig};
use crate::lfd::events::EventHub;
use crate::lfd::http::routes::infer_wave_git_state_for_worktree;
use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::{ExecutionStore, ForkRunStatus, SharedStore};
use crate::lfd::triggers::{
    dispatch_wave_if_ready, enqueue_pending_activation, spawn_immediate_activation,
    spawn_run_task_with_slot, ActivationEnvelope, EnqueueOutcome,
};
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, Event, LivePrState, LivePullRequestState,
    Signal, TerminalSession, TerminalSessionStatus, Wave, WaveMode, WaveRun, WaveRunSnapshot,
    WaveRunStatus, WaveStatus, CI_FIX_FLOW,
};

use super::docker::DockerExecutor;
use super::helpers::{
    advance_branch, auto_commit_if_dirty, auto_create_pr, build_agent_capabilities,
    build_step_prompt, cleanup_run_worktree, flow_parents_for_index, is_active_wave_run_status,
    is_ephemeral_worktree_path, post_step_sync, pre_step_sync,
};
use super::local::LocalProcessExecutor;
use super::{AgentExecutor, JanitorReport, StartupRecovery};
use launch::AgentLaunchRequest;

#[derive(Debug, Default)]
struct GitStatePoller {
    last_commit_shas: Option<Vec<String>>,
    last_diff_stat: Option<String>,
}

impl GitStatePoller {
    fn has_changed(&mut self, commit_shas: Vec<String>, diff_stat: Option<String>) -> bool {
        let changed = match &self.last_commit_shas {
            None => false,
            Some(previous) => previous != &commit_shas || self.last_diff_stat != diff_stat,
        };
        self.last_commit_shas = Some(commit_shas);
        self.last_diff_stat = diff_stat;
        changed
    }
}

#[derive(Debug)]
struct GitStatePollerTask {
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for GitStatePollerTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

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

    fn spawn_git_state_poller(
        &self,
        wave_id: LfdId,
        wave_name: String,
        worktree: String,
    ) -> GitStatePollerTask {
        let event_hub = self.event_hub.clone();
        let handle = tokio::spawn(async move {
            let mut poller = GitStatePoller::default();
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;

                let worktree_path = std::path::PathBuf::from(&worktree);
                let wave_name_for_lookup = wave_name.clone();
                let state = match tokio::task::spawn_blocking(move || {
                    infer_wave_git_state_for_worktree(&worktree_path, &wave_name_for_lookup)
                })
                .await
                {
                    Ok(state) => state,
                    Err(err) => {
                        warn!(
                            wave_id = %wave_id,
                            error = %err,
                            "git state poller task join failure"
                        );
                        continue;
                    }
                };

                let Some(state) = state else {
                    continue;
                };

                let commit_shas = state.commits.into_iter().map(|entry| entry.sha).collect();
                if poller.has_changed(commit_shas, state.diff_stat) {
                    event_hub.send(Event::wave_updated(wave_id.clone()));
                }
            }
        });
        GitStatePollerTask { handle }
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
        let active_paths = self.collect_active_ephemeral_worktrees().await?;

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

    async fn collect_active_ephemeral_worktrees(&self) -> Result<HashSet<String>> {
        let mut active = HashSet::new();
        let runs = self.store.list_wave_runs(None, None).await?;

        for run in runs {
            let run_is_active = is_active_wave_run_status(run.status);
            if !run_is_active {
                continue;
            }

            let forks = self.store.list_fork_runs(&run.id, run.step_index).await?;
            for fork in forks {
                if !matches!(fork.status, ForkRunStatus::Pending | ForkRunStatus::Running) {
                    continue;
                }
                active.insert(fork.worktree);
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
        let _git_state_poller = self.spawn_git_state_poller(
            wave.id().clone(),
            wave.name().clone(),
            run.worktree.clone(),
        );
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
                FlowAction::RunStep { mut step } => {
                    // Pre-step sync: pick up sibling pushes.
                    if let Err(err) = pre_step_sync(
                        Path::new(&run.snapshot.repo),
                        Path::new(&run.worktree),
                        &run.branch,
                    ) {
                        warn!(run_id = %run.id, error = %err, "pre-step sync failed, continuing");
                    }

                    // Try fast-path before spinning up an agent.
                    if let Some(ref cmd) = step.step.fast_path {
                        info!(run_id = %run.id, step = %step.step.name, cmd = cmd, "trying fast-path");
                        match try_fast_path(cmd, Path::new(&run.worktree)) {
                            Ok(FastPathResult::Success) => {
                                info!(run_id = %run.id, step = %step.step.name, "fast-path succeeded, skipping agent");
                                // Skip agent AND post_step_sync — the command handled everything.
                                // post_step_sync would fail here anyway (e.g. branch merged, worktree renamed).
                                self.advance_run_step(&mut run, &plan, wave.id()).await?;
                                continue;
                            }
                            Ok(FastPathResult::Failed {
                                exit_code,
                                stdout,
                                stderr,
                            }) => {
                                info!(run_id = %run.id, step = %step.step.name, exit_code, "fast-path failed, falling back to agent");
                                let ctx = FailureContext {
                                    cmd,
                                    exit_code,
                                    stdout: &stdout,
                                    stderr: &stderr,
                                };
                                let body = step.step.content.as_deref().unwrap_or("");
                                step.step.content = Some(format!("{ctx}{body}"));
                            }
                            Err(err) => {
                                warn!(run_id = %run.id, step = %step.step.name, error = %err, "fast-path execution error, falling back to agent");
                            }
                        }
                    }

                    // Ensure area summary is fresh before each step
                    if let Err(err) = self.ensure_summary_fresh(&wave, &run).await {
                        warn!(run_id = %run.id, error = %err, "summary refresh failed, continuing");
                    }
                    info!(run_id = %run.id, step = %step.step.name, step_index = run.step_index, "running step");
                    let exit_code = self.run_step(&wave, &mut run, &step).await?;
                    if exit_code == 0 {
                        // Post-step sync: commit and push changes.
                        let step_name = step.step.name.clone();
                        if let Err(err) =
                            post_step_sync(Path::new(&run.worktree), &run.branch, &step_name)
                        {
                            self.fail_run(
                                &mut run,
                                &wave,
                                format!("post-step sync failed for {step_name}: {err}"),
                            )
                            .await?;
                            return Ok(());
                        }
                        self.advance_run_step(&mut run, &plan, wave.id()).await?;
                    } else {
                        self.fail_run(&mut run, &wave, format!("step {} failed", step.step.name))
                            .await?;
                        return Ok(());
                    }
                }
                FlowAction::RunOps { ops } => {
                    if let Err(err) = pre_step_sync(
                        Path::new(&run.snapshot.repo),
                        Path::new(&run.worktree),
                        &run.branch,
                    ) {
                        warn!(run_id = %run.id, error = %err, "pre-ops sync failed, continuing");
                    }

                    let command = ops.item.display_name();
                    info!(
                        run_id = %run.id,
                        command = %command,
                        step_index = run.step_index,
                        "running ops item"
                    );

                    let worktree = run.worktree.clone();
                    let item = ops.item.clone();
                    let op_result = tokio::task::spawn_blocking(move || {
                        crate::ops::execute_flow_ops(
                            Path::new(&worktree),
                            &item,
                            &crate::ops::NullProgress,
                        )
                    })
                    .await;

                    match op_result {
                        Ok(Ok(())) => {
                            self.advance_run_step(&mut run, &plan, wave.id()).await?;
                        }
                        Ok(Err(err)) => {
                            self.fail_run(
                                &mut run,
                                &wave,
                                format!("ops item '{command}' failed: {err}"),
                            )
                            .await?;
                            return Ok(());
                        }
                        Err(err) => {
                            self.fail_run(
                                &mut run,
                                &wave,
                                format!("ops item '{command}' panicked: {err}"),
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                }
                FlowAction::WaitInteractive { step } => {
                    let terminal_session =
                        self.create_terminal_session(&wave, &run, &step)
                            .await
                            .map_err(|err| anyhow!("failed to create terminal session: {err}"))?;
                    let attention_item =
                        create_interactive_attention_item(&wave, &run, &step, &terminal_session);
                    self.store.upsert_attention_item(&attention_item).await?;
                    self.event_hub
                        .send(Event::attention_created(attention_item));
                    let terminal_session_id = terminal_session.id.clone();
                    self.spawn_terminal_session_watcher(
                        wave.id().clone(),
                        run.id.clone(),
                        terminal_session_id.clone(),
                    );

                    run.status = WaveRunStatus::Waiting;
                    run.flow_parents = step.flow_parents.clone();
                    self.store.update_wave_run(&run).await?;
                    self.set_wave_status(wave.id(), WaveStatus::Waiting).await;
                    self.event_hub.send(Event::wave_waiting(
                        wave.id().clone(),
                        run.id.clone(),
                        step.step.name.clone(),
                        None,
                        Some(terminal_session_id),
                        None,
                    ));
                    return Ok(());
                }
                FlowAction::And { fork } => {
                    // Pre-and sync: pick up sibling pushes.
                    if let Err(err) = pre_step_sync(
                        Path::new(&run.snapshot.repo),
                        Path::new(&run.worktree),
                        &run.branch,
                    ) {
                        warn!(run_id = %run.id, error = %err, "pre-and sync failed, continuing");
                    }
                    info!(
                        run_id = %run.id,
                        branches = fork.branches.len(),
                        step_index = run.step_index,
                        "running and (all branches)"
                    );
                    self.run_fork(&wave, &mut run, &plan, &fork).await?;
                    if run.status == WaveRunStatus::Failed {
                        return Ok(());
                    }
                    // Post-and sync: commit and push changes.
                    if let Err(err) = post_step_sync(Path::new(&run.worktree), &run.branch, "and") {
                        self.fail_run(&mut run, &wave, format!("post-and sync failed: {err}"))
                            .await?;
                        return Ok(());
                    }
                }
                FlowAction::Xor { branch } => {
                    // Pre-or sync: pick up sibling pushes.
                    if let Err(err) = pre_step_sync(
                        Path::new(&run.snapshot.repo),
                        Path::new(&run.worktree),
                        &run.branch,
                    ) {
                        warn!(run_id = %run.id, error = %err, "pre-or sync failed, continuing");
                    }

                    let router_name = branch.router.as_deref().unwrap_or("xor-route");

                    info!(
                        run_id = %run.id,
                        router = router_name,
                        paths = branch.paths.len(),
                        step_index = run.step_index,
                        "running or router"
                    );

                    // Build routing instructions to append to the router step.
                    let routing_suffix = build_xor_routing_suffix(&branch);

                    let routing_content = if let Some(ref router_name) = branch.router {
                        let router = load_step(router_name, Path::new(&run.snapshot.repo))?;
                        let base = router.content.as_deref().unwrap_or("");
                        format!("{base}\n\n{routing_suffix}")
                    } else {
                        format!(
                            "Previous steps have analyzed the current state and written their \
                             findings to scratch/.\nRead scratch/ to understand what's been \
                             decided, then choose the right path forward.\n\n{routing_suffix}"
                        )
                    };

                    let router = ConcreteStep {
                        step: Step {
                            name: router_name.to_string(),
                            agent: None,
                            default_agent: Some("claude:sonnet".to_string()),
                            directions: Vec::new(),
                            action_style: None,
                            interactive: None,
                            content: Some(routing_content),
                            fast_path: None,
                        },
                        flow_parents: branch.flow_parents.clone(),
                    };

                    let exit_code = self.run_step(&wave, &mut run, &router).await?;
                    if exit_code != 0 {
                        self.fail_run(&mut run, &wave, "xor router step failed".to_string())
                            .await?;
                        return Ok(());
                    }

                    // Post-routing sync: commit the verdict.
                    if let Err(err) =
                        post_step_sync(Path::new(&run.worktree), &run.branch, router_name)
                    {
                        self.fail_run(
                            &mut run,
                            &wave,
                            format!("post-xor-route sync failed: {err}"),
                        )
                        .await?;
                        return Ok(());
                    }

                    // Read the verdict.
                    let verdict_path = Path::new(&run.worktree).join("scratch/route-xor.md");
                    let selected_path = match read_xor_verdict(&verdict_path, &branch) {
                        Ok(path) => path,
                        Err(err) => {
                            self.fail_run(&mut run, &wave, err).await?;
                            return Ok(());
                        }
                    };

                    info!(
                        run_id = %run.id,
                        selected = %selected_path,
                        "xor routed"
                    );

                    // Load and execute the selected sub-flow inline.
                    let or_path = branch
                        .paths
                        .get(&selected_path)
                        .expect("selected path validated by read_or_verdict");

                    let sub_items = load_xor_path_items(or_path, Path::new(&run.snapshot.repo))?;

                    for sub_item in &sub_items {
                        let sub_step = match sub_item {
                            ConcreteItem::Step(step) => step,
                            other => {
                                warn!(
                                    run_id = %run.id,
                                    item = ?other,
                                    "xor sub-flow contains non-step item, skipping"
                                );
                                continue;
                            }
                        };

                        // Pre-step sync for each sub-step.
                        if let Err(err) = pre_step_sync(
                            Path::new(&run.snapshot.repo),
                            Path::new(&run.worktree),
                            &run.branch,
                        ) {
                            warn!(run_id = %run.id, error = %err, "pre-step sync failed in or sub-step, continuing");
                        }

                        if let Err(err) = self.ensure_summary_fresh(&wave, &run).await {
                            warn!(run_id = %run.id, error = %err, "summary refresh failed, continuing");
                        }

                        info!(
                            run_id = %run.id,
                            step = %sub_step.step.name,
                            "running or sub-step"
                        );

                        let exit_code = self.run_step(&wave, &mut run, sub_step).await?;
                        if exit_code != 0 {
                            self.fail_run(
                                &mut run,
                                &wave,
                                format!("xor sub-step {} failed", sub_step.step.name),
                            )
                            .await?;
                            return Ok(());
                        }

                        if let Err(err) = post_step_sync(
                            Path::new(&run.worktree),
                            &run.branch,
                            &sub_step.step.name,
                        ) {
                            self.fail_run(
                                &mut run,
                                &wave,
                                format!(
                                    "post-step sync failed for or sub-step {}: {err}",
                                    sub_step.step.name
                                ),
                            )
                            .await?;
                            return Ok(());
                        }
                    }

                    self.advance_run_step(&mut run, &plan, wave.id()).await?;
                }
                FlowAction::Or { .. } => {
                    self.fail_run(
                        &mut run,
                        &wave,
                        "or (multi-select) execution is not yet implemented".to_string(),
                    )
                    .await?;
                    return Ok(());
                }
                FlowAction::Loop { body } => loop {
                    info!(
                        run_id = %run.id,
                        step_index = run.step_index,
                        "running loop body"
                    );

                    self.run_inline_items(&wave, &mut run, &body.steps).await?;
                    if run.status == WaveRunStatus::Failed {
                        return Ok(());
                    }

                    let selected = self.run_inline_xor(&wave, &mut run, &body.exit).await?;
                    if run.status == WaveRunStatus::Failed {
                        return Ok(());
                    }

                    if selected == "done" {
                        self.advance_run_step(&mut run, &plan, wave.id()).await?;
                        break;
                    }
                },
                FlowAction::Complete => {
                    run.status = WaveRunStatus::Completed;
                    run.ended_at = Some(OffsetDateTime::now_utc());

                    let is_recurring = matches!(wave.mode(), WaveMode::Loop | WaveMode::Cron);

                    // Auto-create PR as draft; queue reconciliation promotes the queue head.
                    // Activations targeting "main" produce new branches and PRs.
                    // Activations targeting a specific branch (e.g. CI-fix on a PR branch)
                    // push directly to that branch — no new PR needed.
                    let should_manage_pr =
                        run.target_branch == "main" || run.target_branch.is_empty();
                    if should_manage_pr {
                        let worktree = run.worktree.clone();
                        let wave_name = wave.name().clone();
                        match tokio::task::spawn_blocking(move || {
                            auto_create_pr(Path::new(&worktree), Some(wave_name))
                        })
                        .await
                        {
                            Ok(Some(pr)) => {
                                info!(run_id = %run.id, url = %pr.url, "auto-created PR");
                                run.pr = Some(pr);
                            }
                            Ok(None) => {}
                            Err(err) => {
                                warn!(run_id = %run.id, error = %err, "failed to auto-create PR");
                            }
                        }
                    }

                    if let Some(pr) = run.pr.as_ref() {
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
                    if should_manage_pr && run.pr.is_some() && is_recurring {
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
                    self.output.close_writer(&run.id.to_string());
                    self.trigger_listeners_on_completion(wave.id(), &run.branch)
                        .await;
                    if should_manage_pr && run.pr.is_some() {
                        if let Err(err) = crate::lfd::queue::reconcile_wave_queue_with_events(
                            &self.store,
                            &self.github_config,
                            wave.id(),
                            crate::lfd::queue::QueueTrigger::RunCompleted,
                            Some(&self.event_hub),
                        )
                        .await
                        {
                            warn!(wave_id = %wave.id(), error = %err, "queue reconcile failed after run completion");
                        }
                    }
                    // Clean up run-scoped worktrees (parallel execution).
                    let wt = Path::new(&run.worktree);
                    if let Err(err) = cleanup_run_worktree(wt) {
                        warn!(run_id = %run.id, error = %err, "failed to clean up run worktree");
                    }

                    // Only go Idle if no other active runs remain for this wave.
                    let other_active = self
                        .store
                        .get_active_wave_run(wave.id())
                        .await
                        .ok()
                        .flatten()
                        .is_some();
                    if !other_active {
                        self.set_wave_status(wave.id(), WaveStatus::Idle).await;
                    }
                    self.event_hub.send(Event::wave_updated(wave.id().clone()));
                    return Ok(());
                }
            }
        }
    }

    async fn trigger_listeners_on_completion(&self, source_wave_id: &LfdId, source_branch: &str) {
        let triggers = match self
            .store
            .list_triggers_by_signal(Signal::Wave.as_i32())
            .await
        {
            Ok(triggers) => triggers,
            Err(err) => {
                warn!(
                    wave_id = %source_wave_id,
                    error = %err,
                    "failed to list wave triggers"
                );
                return;
            }
        };

        for mut trigger in triggers {
            if !trigger.enabled || trigger.source_wave_id.as_ref() != Some(source_wave_id) {
                continue;
            }

            let listener_wave = match self.store.get_wave(&trigger.wave_id).await {
                Ok(Some(wave)) => wave,
                Ok(None) => continue,
                Err(err) => {
                    warn!(trigger_id = %trigger.id, error = %err, "failed to load listening wave");
                    continue;
                }
            };

            if listener_wave.status() == WaveStatus::Paused {
                continue;
            }

            let reason = format!(
                "wave trigger {} triggered by source wave {}",
                trigger.id, source_wave_id
            );
            let envelope = ActivationEnvelope::new(
                listener_wave.id(),
                Some(&trigger.id),
                reason,
                "",
                "",
                source_branch,
            );
            let activated = if listener_wave.serialized {
                let enqueued = matches!(
                    enqueue_pending_activation(&self.store, &self.event_hub, envelope).await,
                    Some(EnqueueOutcome::Queued | EnqueueOutcome::Coalesced)
                );
                if enqueued {
                    let _ = dispatch_wave_if_ready(
                        &self.store,
                        self,
                        &self.scheduler,
                        &self.event_hub,
                        &listener_wave,
                    )
                    .await;
                }
                enqueued
            } else {
                spawn_immediate_activation(
                    &self.store,
                    self,
                    &self.scheduler,
                    &self.event_hub,
                    &listener_wave,
                    trigger.flow.clone(),
                    envelope,
                )
                .await
                .is_some()
            };
            if activated {
                trigger.last_triggered_at = Some(OffsetDateTime::now_utc().unix_timestamp());
                if let Err(err) = self.store.update_trigger(&trigger).await {
                    warn!(
                        trigger_id = %trigger.id,
                        error = %err,
                        "failed to update wave trigger last_triggered_at"
                    );
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
        run.error = Some(error.clone());
        self.store.update_wave_run(run).await?;

        // Repair dispatch and algedonic escalation are handled in
        // execute_run_inner (triggers/common.rs), which checks the full
        // repair chain depth and applies backoff.

        self.output.close_writer(&run.id.to_string());

        self.set_wave_status(wave.id(), WaveStatus::Failed).await;
        self.event_hub.send(Event::wave_updated(wave.id().clone()));
        Ok(())
    }

    async fn run_inline_items(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        items: &[ConcreteItem],
    ) -> Result<()> {
        for item in items {
            match item {
                ConcreteItem::Step(step) => {
                    if let Err(err) = pre_step_sync(
                        Path::new(&run.snapshot.repo),
                        Path::new(&run.worktree),
                        &run.branch,
                    ) {
                        warn!(run_id = %run.id, error = %err, "pre-step sync failed in inline execution, continuing");
                    }

                    if let Err(err) = self.ensure_summary_fresh(wave, run).await {
                        warn!(run_id = %run.id, error = %err, "summary refresh failed, continuing");
                    }

                    let exit_code = self.run_step(wave, run, step).await?;
                    if exit_code != 0 {
                        self.fail_run(run, wave, format!("inline step {} failed", step.step.name))
                            .await?;
                        return Ok(());
                    }

                    if let Err(err) =
                        post_step_sync(Path::new(&run.worktree), &run.branch, &step.step.name)
                    {
                        self.fail_run(
                            run,
                            wave,
                            format!(
                                "post-step sync failed for inline step {}: {err}",
                                step.step.name
                            ),
                        )
                        .await?;
                        return Ok(());
                    }
                }
                ConcreteItem::Op(ops) => {
                    if let Err(err) = pre_step_sync(
                        Path::new(&run.snapshot.repo),
                        Path::new(&run.worktree),
                        &run.branch,
                    ) {
                        warn!(run_id = %run.id, error = %err, "pre-op sync failed in inline execution, continuing");
                    }

                    let worktree = run.worktree.clone();
                    let item = ops.item.clone();
                    let command = item.display_name();
                    match tokio::task::spawn_blocking(move || {
                        crate::ops::execute_flow_ops(
                            Path::new(&worktree),
                            &item,
                            &crate::ops::NullProgress,
                        )
                    })
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => {
                            self.fail_run(
                                run,
                                wave,
                                format!("inline op '{command}' failed: {err}"),
                            )
                            .await?;
                            return Ok(());
                        }
                        Err(err) => {
                            self.fail_run(
                                run,
                                wave,
                                format!("inline op '{command}' panicked: {err}"),
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                }
                ConcreteItem::And(_) => {
                    self.fail_run(
                        run,
                        wave,
                        "inline and constructs are not supported inside loop bodies".to_string(),
                    )
                    .await?;
                    return Ok(());
                }
                ConcreteItem::Xor(branch) => {
                    let _ = self.run_inline_xor(wave, run, branch).await?;
                    if run.status == WaveRunStatus::Failed {
                        return Ok(());
                    }
                }
                ConcreteItem::Or(_) => {
                    self.fail_run(
                        run,
                        wave,
                        "or (multi-select) execution is not yet implemented".to_string(),
                    )
                    .await?;
                    return Ok(());
                }
                ConcreteItem::Loop(_) => {
                    self.fail_run(
                        run,
                        wave,
                        "nested loop constructs are not supported in inline execution".to_string(),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    async fn run_inline_xor(
        &self,
        wave: &Wave,
        run: &mut WaveRun,
        branch: &crate::engine::ConcreteXor,
    ) -> Result<String> {
        if let Err(err) = pre_step_sync(
            Path::new(&run.snapshot.repo),
            Path::new(&run.worktree),
            &run.branch,
        ) {
            warn!(run_id = %run.id, error = %err, "pre-or sync failed in inline execution, continuing");
        }

        let router_name = branch.router.as_deref().unwrap_or("xor-route");
        let routing_suffix = build_xor_routing_suffix(branch);
        let routing_content = if let Some(ref router_name) = branch.router {
            let router = load_step(router_name, Path::new(&run.snapshot.repo))?;
            let base = router.content.as_deref().unwrap_or("");
            format!("{base}\n\n{routing_suffix}")
        } else {
            format!(
                "Previous steps have analyzed the current state and written their findings to scratch/.\n\
                 Read scratch/ to understand what's been decided, then choose the right path forward.\n\n\
                 {routing_suffix}"
            )
        };

        let router = ConcreteStep {
            step: Step {
                name: router_name.to_string(),
                agent: None,
                default_agent: Some("claude:sonnet".to_string()),
                directions: Vec::new(),
                action_style: None,
                interactive: None,
                content: Some(routing_content),
                fast_path: None,
            },
            flow_parents: branch.flow_parents.clone(),
        };

        let exit_code = self.run_step(wave, run, &router).await?;
        if exit_code != 0 {
            self.fail_run(run, wave, "xor router step failed".to_string())
                .await?;
            return Ok("done".to_string());
        }

        if let Err(err) = post_step_sync(Path::new(&run.worktree), &run.branch, router_name) {
            self.fail_run(run, wave, format!("post-xor-route sync failed: {err}"))
                .await?;
            return Ok("done".to_string());
        }

        let verdict_path = Path::new(&run.worktree).join("scratch/route-xor.md");
        let selected = match read_xor_verdict(&verdict_path, branch) {
            Ok(path) => path,
            Err(err) => {
                self.fail_run(run, wave, err).await?;
                return Ok("done".to_string());
            }
        };

        let or_path = branch
            .paths
            .get(&selected)
            .expect("selected path validated by read_or_verdict");
        let sub_items = load_xor_path_items(or_path, Path::new(&run.snapshot.repo))?;
        for sub_item in &sub_items {
            match sub_item {
                ConcreteItem::Step(step) => {
                    if let Err(err) = pre_step_sync(
                        Path::new(&run.snapshot.repo),
                        Path::new(&run.worktree),
                        &run.branch,
                    ) {
                        warn!(run_id = %run.id, error = %err, "pre-step sync failed in inline or sub-step, continuing");
                    }

                    if let Err(err) = self.ensure_summary_fresh(wave, run).await {
                        warn!(run_id = %run.id, error = %err, "summary refresh failed, continuing");
                    }

                    let exit_code = self.run_step(wave, run, step).await?;
                    if exit_code != 0 {
                        self.fail_run(run, wave, format!("xor sub-step {} failed", step.step.name))
                            .await?;
                        return Ok(selected);
                    }

                    if let Err(err) =
                        post_step_sync(Path::new(&run.worktree), &run.branch, &step.step.name)
                    {
                        self.fail_run(
                            run,
                            wave,
                            format!(
                                "post-step sync failed for or sub-step {}: {err}",
                                step.step.name
                            ),
                        )
                        .await?;
                        return Ok(selected);
                    }
                }
                ConcreteItem::Op(ops) => {
                    let worktree = run.worktree.clone();
                    let item = ops.item.clone();
                    let command = item.display_name();
                    match tokio::task::spawn_blocking(move || {
                        crate::ops::execute_flow_ops(
                            Path::new(&worktree),
                            &item,
                            &crate::ops::NullProgress,
                        )
                    })
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => {
                            self.fail_run(
                                run,
                                wave,
                                format!("xor sub-op '{command}' failed: {err}"),
                            )
                            .await?;
                            return Ok(selected);
                        }
                        Err(err) => {
                            self.fail_run(
                                run,
                                wave,
                                format!("xor sub-op '{command}' panicked: {err}"),
                            )
                            .await?;
                            return Ok(selected);
                        }
                    }
                }
                ConcreteItem::And(_)
                | ConcreteItem::Xor(_)
                | ConcreteItem::Or(_)
                | ConcreteItem::Loop(_) => {
                    self.fail_run(
                        run,
                        wave,
                        "nested and/or/loop constructs are not supported inside inline or paths"
                            .to_string(),
                    )
                    .await?;
                    return Ok(selected);
                }
            }
        }

        Ok(selected)
    }

    /// Create a repair run in the same worktree/branch as the failed run.
    /// Returns the created run; the caller is responsible for executing it.
    pub(crate) async fn create_repair_run(
        &self,
        wave: &Wave,
        failed_run: &WaveRun,
        repair_flow: &str,
    ) -> Result<WaveRun> {
        let repair_run = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            snapshot: WaveRunSnapshot {
                repo: failed_run.snapshot.repo.clone(),
                flow: repair_flow.to_string(),
                direction: failed_run.snapshot.direction.clone(),
                area: failed_run.snapshot.area.clone(),
            },
            iteration: failed_run.iteration,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: failed_run.worktree.clone(),
            branch: failed_run.branch.clone(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            activation_log_id: None,
            parent_run_id: failed_run.parent_run_id.clone(),
            parent_pr_number: failed_run.parent_pr_number,
            stack_position: failed_run.stack_position,
            stack_group_id: failed_run.stack_group_id.clone(),
            stack_status: failed_run.stack_status,
            lineage_inferred: false,
            target_branch: failed_run.target_branch.clone(),
            repair_of: Some(failed_run.id.clone()),
            pr: failed_run.pr.clone(),
        };
        self.store.create_wave_run(&repair_run).await?;
        Ok(repair_run)
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

    pub async fn create_terminal_session_for_waiting_wave(
        &self,
        wave_id: &LfdId,
        wave_run_id: Option<&LfdId>,
    ) -> std::result::Result<TerminalSession, String> {
        let wave = self
            .store
            .get_wave(wave_id)
            .await
            .map_err(|err| format!("failed to load wave: {err}"))?
            .ok_or_else(|| "wave not found".to_string())?;
        let run = if let Some(wave_run_id) = wave_run_id {
            self.store
                .get_wave_run(wave_run_id)
                .await
                .map_err(|err| format!("failed to load wave run: {err}"))?
                .ok_or_else(|| "wave run not found".to_string())?
        } else {
            self.store
                .get_active_wave_run(wave_id)
                .await
                .map_err(|err| format!("failed to load active wave run: {err}"))?
                .ok_or_else(|| "no active wave run for wave".to_string())?
        };
        if run.status != WaveRunStatus::Waiting {
            return Err("wave run is not waiting for terminal input".to_string());
        }

        if let Some(session) = self
            .store
            .get_active_terminal_session_for_wave_run(&run.id)
            .await
            .map_err(|err| format!("failed to load terminal session: {err}"))?
        {
            return Ok(session);
        }

        let repo = Path::new(&run.snapshot.repo);
        let flow = load_flow(&run.snapshot.flow, repo).map_err(|err| err.to_string())?;
        let plan = expand_flow(&flow, repo).map_err(|err| err.to_string())?;
        let FlowAction::WaitInteractive { step } = next_action(&plan, run.step_index as usize)
        else {
            return Err("current wave step is not interactive".to_string());
        };
        self.create_terminal_session(&wave, &run, &step)
            .await
            .map_err(|err| err.to_string())
    }

    async fn create_terminal_session(
        &self,
        wave: &Wave,
        run: &WaveRun,
        step: &ConcreteStep,
    ) -> Result<TerminalSession> {
        let (launch, process, agent) = build_terminal_launch_config(wave, run, step).await?;
        let terminal_session = TerminalSession {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            wave_run_id: Some(run.id.clone()),
            step: step.step.name.clone(),
            agent,
            cwd: launch
                .cwd
                .clone()
                .unwrap_or_else(|| PathBuf::from(&run.worktree))
                .to_string_lossy()
                .to_string(),
            argv: build_agent_command(&launch, &process, &build_agent_capabilities(&run.worktree)),
            env: build_agent_env(&launch, &process),
            source: "wave_step".to_string(),
            status: TerminalSessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: Some(LfdId::new().to_string()),
        };
        self.store
            .create_terminal_session(&terminal_session)
            .await?;
        self.event_hub
            .send(Event::terminal_session_created(terminal_session.clone()));
        Ok(terminal_session)
    }

    fn spawn_terminal_session_watcher(&self, wave_id: LfdId, run_id: LfdId, session_id: LfdId) {
        let executor = self.clone();
        tokio::spawn(async move {
            if let Err(err) = executor
                .wait_for_terminal_session_and_resume(wave_id, run_id, session_id.clone())
                .await
            {
                warn!(session_id = %session_id, error = %err, "terminal session watcher failed");
            }
        });
    }

    async fn wait_for_terminal_session_and_resume(
        &self,
        wave_id: LfdId,
        run_id: LfdId,
        session_id: LfdId,
    ) -> Result<()> {
        let session_status = self.wait_for_terminal_session_status(&session_id).await?;
        info!(
            wave_run_id = %run_id,
            session_id = %session_id,
            status = %session_status.as_str(),
            "terminal session ended; resuming wave flow"
        );

        let Some(mut run) = self.store.get_wave_run(&run_id).await? else {
            return Ok(());
        };
        if run.status != WaveRunStatus::Waiting {
            return Ok(());
        }

        let Some(wave) = self.store.get_wave(&wave_id).await? else {
            return Ok(());
        };

        if let Some(item) = self
            .store
            .find_attention_item_for_run(&run.id, AttentionKind::Interactive)
            .await?
        {
            if let Some(item) = resolve_attention_item(&self.store, &item.id)
                .await
                .map_err(|err| anyhow!("failed to resolve interactive attention item: {err}"))?
            {
                self.event_hub.send(Event::attention_resolved(item));
            }
        }

        if matches!(
            session_status,
            TerminalSessionStatus::Failed | TerminalSessionStatus::Canceled
        ) {
            self.fail_run(
                &mut run,
                &wave,
                format!("terminal session {session_id} {}", session_status.as_str()),
            )
            .await?;
            return Ok(());
        }

        let repo = Path::new(&run.snapshot.repo);
        let flow = load_flow(&run.snapshot.flow, repo)?;
        let plan = expand_flow(&flow, repo)?;

        // Auto-commit any changes left by the interactive step.
        let step_name = match next_action(&plan, run.step_index as usize) {
            FlowAction::WaitInteractive { step } | FlowAction::RunStep { step } => step.step.name,
            FlowAction::RunOps { ops } => ops.item.to_string(),
            _ => format!("step-{}", run.step_index),
        };
        let worktree = run.worktree.clone();
        if let Err(err) = tokio::task::spawn_blocking(move || {
            auto_commit_if_dirty(Path::new(&worktree), &step_name)
        })
        .await
        .map_err(|err| anyhow!("terminal auto-commit task failed: {err}"))
        .and_then(|r| r.map_err(|err| anyhow!("terminal auto-commit failed: {err}")))
        {
            self.fail_run(&mut run, &wave, err.to_string()).await?;
            return Ok(());
        }

        self.advance_run_step(&mut run, &plan, wave.id()).await?;
        self.set_wave_status(wave.id(), WaveStatus::Running).await;
        self.resume_run_execution(run).await?;
        Ok(())
    }

    async fn wait_for_terminal_session_status(
        &self,
        session_id: &LfdId,
    ) -> Result<TerminalSessionStatus> {
        loop {
            let session = self
                .store
                .get_terminal_session(session_id)
                .await
                .map_err(|err| anyhow!("failed to load terminal session {session_id}: {err}"))?
                .ok_or_else(|| anyhow!("terminal session {session_id} not found"))?;
            if session.status.is_terminal() {
                return Ok(session.status);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    async fn resume_run_execution(&self, run: WaveRun) -> Result<()> {
        // Retry for up to 60s (120 × 500ms) waiting for a scheduler slot.
        for _ in 0..120 {
            if let Some(current) = self.store.get_wave_run(&run.id).await? {
                if current.status != WaveRunStatus::Running {
                    return Ok(());
                }
            } else {
                return Ok(());
            }

            if let Ok(slot_guard) = self.scheduler.acquire_guard(run.id.as_str()).await {
                spawn_run_task_with_slot(
                    self.store.clone(),
                    self.clone(),
                    self.event_hub.clone(),
                    run,
                    slot_guard,
                );
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(anyhow!(
            "unable to resume run {}: no scheduler slots available",
            run.id
        ))
    }

    async fn run_step(&self, wave: &Wave, run: &mut WaveRun, step: &ConcreteStep) -> Result<i32> {
        let worktree = run.worktree.clone();
        debug!(run_id = %run.id, step = %step.step.name, worktree = %worktree, "building step prompt");
        let agent_override =
            wave_agent_override(Path::new(wave.repo()), wave.name(), &step.step.name);
        let (launch, process) = build_step_prompt(
            &worktree,
            step,
            &run.snapshot.direction,
            Some(wave.name()),
            Some((&self.store, wave.id())),
            agent_override,
            None,
        )
        .await?;
        let capabilities = build_agent_capabilities(&worktree);
        let agent = launch.agent.clone().unwrap_or_else(|| "claude".to_string());

        info!(run_id = %run.id, step = %step.step.name, agent = %agent, "launching agent");

        let outcome = self
            .launch_agent(AgentLaunchRequest {
                wave_id: run.wave_id.clone(),
                wave_run_id: run.id.clone(),
                branch: Some(run.branch.clone()),
                repo: run.snapshot.repo.clone(),
                worktree,
                step: step.clone(),
                agent: agent.clone(),
                cmd: build_agent_command(&launch, &process, &capabilities),
                output_prefix: None,
            })
            .await;

        let outcome = outcome?;

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

fn wave_agent_override(repo: &Path, wave_name: &str, step_name: &str) -> Option<String> {
    let wave_config = read_wave_config(repo, wave_name)?;
    wave_config
        .step_agents
        .as_ref()
        .and_then(|step_agents| step_agents.get(step_name).cloned())
        .or(wave_config.agent)
}

async fn build_terminal_launch_config(
    wave: &Wave,
    run: &WaveRun,
    step: &ConcreteStep,
) -> Result<(AgentConfig, ProcessConfig, String)> {
    let repo_root = Path::new(&run.worktree);
    let repo_config = load_config_or_default(Some(repo_root));
    let summary = None;
    let agent = wave_agent_override(Path::new(wave.repo()), wave.name(), &step.step.name)
        .or_else(|| step.step.agent.clone())
        .or_else(|| repo_config.agent.clone())
        .or_else(|| step.step.default_agent.clone())
        .unwrap_or_else(|| "claude:opus".to_string());

    let prepared = prepare_launch_prompt(
        &repo_config,
        LaunchPromptInput {
            repo_root: repo_root.to_path_buf(),
            step: Some(step.step.name.clone()),
            resolved_step: None,
            surface: Surface::ConcertoMac,
            directions: run.snapshot.direction.clone(),
            area: None,
            wave: Some(wave.name().clone()),
            message: None,
            agent: Some(agent.clone()),
            cwd: None,
            max_turns: None,
            yolo_mode: false,
            include_config_directions: false,
            include_config_area: true,
            source_overrides: Default::default(),
            summary,
            client_context: ClientContext::default(),
            related_repos: Vec::new(),
        },
    )?;

    let _ = write_prompt_log(repo_root, &prepared.prompt, &step.step.name, None);
    let mut launch = prepared.config;
    let cwd = launch
        .cwd
        .clone()
        .unwrap_or_else(|| repo_root.to_path_buf());
    let context_file = write_prompt_log(
        &cwd,
        &launch.system_prompt,
        &format!("{}.context", step.step.name),
        None,
    )
    .ok();
    launch.cwd = Some(cwd);
    launch.skip_permissions = repo_config.yolo;

    Ok((
        launch,
        ProcessConfig {
            auto: false,
            stream: false,
            context_file,
            ..Default::default()
        },
        agent,
    ))
}

fn create_interactive_attention_item(
    wave: &Wave,
    run: &WaveRun,
    step: &ConcreteStep,
    terminal_session: &TerminalSession,
) -> AttentionItem {
    let details = interactive_attention_details(step, run);
    AttentionItem {
        id: LfdId::new(),
        wave_id: wave.id().clone(),
        run_id: Some(run.id.clone()),
        kind: AttentionKind::Interactive,
        status: AttentionStatus::Surfaced,
        title: interactive_title(step, wave),
        summary: interactive_summary(&details),
        context: build_interactive_context(step, terminal_session, &details),
        surfaced_at: OffsetDateTime::now_utc(),
        viewed_at: None,
        resolved_at: None,
    }
}

#[derive(Debug, Default)]
struct InteractiveAttentionDetails {
    design_path: Option<String>,
    mutation_summary: Option<String>,
    summary_source: Option<String>,
}

fn interactive_attention_details(
    step: &ConcreteStep,
    run: &WaveRun,
) -> InteractiveAttentionDetails {
    let worktree = Path::new(&run.worktree);
    match step.step.name.as_str() {
        "review-design" => {
            let design_path = find_review_design_path(worktree, &run.branch);
            let summary_source = design_path
                .as_ref()
                .and_then(|path| read_context_file(worktree, path));
            InteractiveAttentionDetails {
                design_path,
                mutation_summary: None,
                summary_source,
            }
        }
        "wave/review" => {
            let mutation_summary = read_context_file(worktree, "scratch/wave-mutate.md");
            InteractiveAttentionDetails {
                design_path: None,
                summary_source: mutation_summary.clone(),
                mutation_summary,
            }
        }
        _ => InteractiveAttentionDetails::default(),
    }
}

fn build_interactive_context(
    step: &ConcreteStep,
    terminal_session: &TerminalSession,
    details: &InteractiveAttentionDetails,
) -> Value {
    let mut context = json!({
        "step": step.step.name.clone(),
        "terminal_session_id": terminal_session.id.clone(),
    });

    if let Some(design_path) = &details.design_path {
        context["design_path"] = Value::String(design_path.clone());
    }
    if let Some(summary) = &details.mutation_summary {
        context["mutation_summary"] = Value::String(summary.clone());
    }

    context
}

fn interactive_title(step: &ConcreteStep, wave: &Wave) -> String {
    match step.step.name.as_str() {
        "review-design" => format!("Design review: {}", wave.name()),
        "wave/review" => format!("Wave review: {}", wave.name()),
        _ => format!("Interactive: {}", step.step.name),
    }
}

fn interactive_summary(details: &InteractiveAttentionDetails) -> String {
    details
        .summary_source
        .as_deref()
        .and_then(first_meaningful_line)
        .unwrap_or_default()
}

fn find_review_design_path(worktree: &Path, branch: &str) -> Option<String> {
    let branch_candidate = format!("scratch/{branch}.md");
    if worktree.join(&branch_candidate).is_file() {
        return Some(branch_candidate);
    }

    let scratch_dir = worktree.join("scratch");
    let mut candidates = std::fs::read_dir(scratch_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "md")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        !name.starts_with('.')
                            && name != "questions.md"
                            && name != "wave-mutate.md"
                            && name != "wave-review.md"
                            && !name.ends_with("-review.md")
                    })
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    candidates.reverse();

    candidates.into_iter().next().and_then(|path| {
        path.strip_prefix(worktree)
            .ok()
            .map(|relative| relative.to_string_lossy().to_string())
    })
}

fn read_context_file(worktree: &Path, relative_path: &str) -> Option<String> {
    let text = std::fs::read_to_string(worktree.join(relative_path)).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn first_meaningful_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty() && *line != "---" && !line.starts_with('#') && !line.starts_with("```")
        })
        .map(ToString::to_string)
}

// -----------------------------------------------------------------------------
// Or-routing helpers
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct OrphanedForkCleanup {
    cleaned_runs: u32,
    removed_worktrees: u32,
}

/// Determine which flow to use for a repair attempt based on the failed run.
///
/// CI-fix runs get `ci-fix`. Everything else gets `debug` — the universal
/// fallback that takes error context as input.
pub(crate) fn classify_repair_flow(failed_run: &WaveRun) -> String {
    // If the original run was a CI-fix that failed, don't loop ci-fix → ci-fix.
    // The debug step is the right fallback for a failed repair tool.
    if failed_run.snapshot.flow == CI_FIX_FLOW {
        return "debug".to_string();
    }
    // TODO: expand classification as we learn more error classes.
    // For now, `debug` handles everything — it reads error context from the
    // failed run and attempts a fix in the same worktree.
    "debug".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::worktree::create_worktree;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Signal, Trigger, WaveRunSnapshot};
    use async_trait::async_trait;
    use loopflow_test_support::TestRepo;
    use std::path::PathBuf;
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
        let log_dir = worktree.join(".lf/prompts");
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
            mode: WaveMode::Manual,
            primary_flow: flow_name.to_string(),
            cron: None,
            direction: vec![],
            area: vec![],
            status: WaveStatus::Running,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
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
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave_id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        };
        store
            .create_wave_run(&run)
            .await
            .expect("wave run should be created");
        (wave_id, run_id)
    }

    fn make_wave(name: &str, repo: &Path, flow: &str, status: WaveStatus) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: repo.to_string_lossy().to_string(),
            mode: WaveMode::Loop,
            primary_flow: flow.to_string(),
            cron: None,
            direction: vec![],
            area: vec![],
            status,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        }
    }

    async fn create_main_run(store: &SharedStore, wave: &Wave, status: WaveRunStatus) -> WaveRun {
        let run = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            snapshot: WaveRunSnapshot {
                repo: wave.repo().clone(),
                flow: wave.primary_flow().clone(),
                direction: wave.direction().clone(),
                area: wave.area().clone(),
            },
            iteration: 0,
            step_index: 0,
            status,
            worktree: wave.repo().clone(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id().to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        };
        store
            .create_wave_run(&run)
            .await
            .expect("wave run should be created");
        run
    }

    fn make_wave_trigger(listener_wave_id: &LfdId, source_wave_id: &LfdId) -> Trigger {
        Trigger {
            id: LfdId::new(),
            wave_id: listener_wave_id.clone(),
            source_wave_id: Some(source_wave_id.clone()),
            signal: Signal::Wave,
            flow: None,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
            max_iterations: None,
        }
    }

    #[test]
    fn git_state_poller_ignores_initial_snapshot() {
        let mut poller = GitStatePoller::default();
        assert!(!poller.has_changed(
            vec!["abc123".to_string()],
            Some("1 file changed".to_string())
        ));
    }

    #[test]
    fn git_state_poller_detects_commit_changes() {
        let mut poller = GitStatePoller::default();
        assert!(!poller.has_changed(
            vec!["abc123".to_string()],
            Some("1 file changed".to_string())
        ));
        assert!(!poller.has_changed(
            vec!["abc123".to_string()],
            Some("1 file changed".to_string())
        ));
        assert!(poller.has_changed(
            vec!["def456".to_string(), "abc123".to_string()],
            Some("2 files changed".to_string())
        ));
    }

    #[test]
    fn git_state_poller_detects_diff_stat_changes_without_new_commits() {
        let mut poller = GitStatePoller::default();
        assert!(!poller.has_changed(
            vec!["abc123".to_string()],
            Some("1 file changed".to_string())
        ));
        assert!(poller.has_changed(
            vec!["abc123".to_string()],
            Some("3 files changed".to_string())
        ));
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
    async fn execute_starts_listen_wave_on_completion() {
        let repo = TestRepo::new();
        repo.create_file(".lf/flows/test-flow.yaml", "- step-a\n");
        repo.create_file(".lf/steps/step-a.md", "do step a");
        repo.stage_all();
        repo.commit("add flow fixtures");

        let db_path = tempdir().expect("tempdir").path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let (source_wave_id, source_run_id) =
            create_wave_and_run(&store, repo.path(), "test-flow").await;
        let target_wave = Wave {
            id: LfdId::new(),
            name: "target-wave".to_string(),
            repo: repo.path().to_string_lossy().to_string(),
            mode: WaveMode::Loop,
            primary_flow: "test-flow".to_string(),
            cron: None,
            direction: vec![],
            area: vec![],
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: true,
        };
        store
            .create_wave(&target_wave)
            .await
            .expect("create target wave");
        let wave_trigger = make_wave_trigger(&target_wave.id, &source_wave_id);
        store
            .create_trigger(&wave_trigger)
            .await
            .expect("create wave trigger");

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

        executor
            .execute(&source_run_id)
            .await
            .expect("execute source run");

        let runs = store
            .list_wave_runs(Some(&target_wave.id), None)
            .await
            .expect("listener runs");
        assert!(
            runs.iter().any(|run| run.wave_id == target_wave.id),
            "listener should receive a run"
        );

        let pending = store
            .list_pending_activations(&target_wave.id)
            .await
            .expect("pending activations");
        assert!(
            pending.is_empty(),
            "listener should start immediately when runnable"
        );

        let updated_trigger = store
            .get_trigger(&wave_trigger.id)
            .await
            .expect("trigger lookup should succeed")
            .expect("trigger should exist");
        assert!(
            updated_trigger.last_triggered_at.is_some(),
            "wave trigger should record trigger time"
        );
    }

    #[tokio::test]
    async fn listen_trigger_queues_when_listener_running() {
        let repo = TestRepo::new();
        repo.create_file(".lf/flows/test-flow.yaml", "- step-a\n");
        repo.create_file(".lf/steps/step-a.md", "do step");
        repo.stage_all();
        repo.commit("add test flow");

        let db_path = tempdir().expect("tempdir").path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let source_wave = make_wave("source", repo.path(), "test-flow", WaveStatus::Running);
        let mut listener_wave =
            make_wave("listener", repo.path(), "test-flow", WaveStatus::Running);
        listener_wave.serialized = true;
        store
            .create_wave(&source_wave)
            .await
            .expect("create source wave");
        store
            .create_wave(&listener_wave)
            .await
            .expect("create listener wave");
        let source_run = create_main_run(&store, &source_wave, WaveRunStatus::Running).await;
        let _listener_active_run =
            create_main_run(&store, &listener_wave, WaveRunStatus::Running).await;

        let trigger = make_wave_trigger(listener_wave.id(), source_wave.id());
        store
            .create_trigger(&trigger)
            .await
            .expect("create wave trigger");

        let scheduler = Arc::new(Scheduler::new(1));
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

        executor
            .execute(&source_run.id)
            .await
            .expect("source run should complete");

        let pending = store
            .get_pending_for_trigger(listener_wave.id(), Some(&trigger.id))
            .await
            .expect("pending activation lookup should succeed");
        assert!(
            pending.is_some(),
            "wave activation should queue while listener is running"
        );

        let updated = store
            .get_trigger(&trigger.id)
            .await
            .expect("trigger lookup should succeed")
            .expect("trigger should exist");
        assert!(
            updated.last_triggered_at.is_some(),
            "queued wave activation should update last_triggered_at"
        );
    }

    #[tokio::test]
    async fn listen_trigger_queues_when_scheduler_full() {
        let repo = TestRepo::new();
        repo.create_file(".lf/flows/test-flow.yaml", "- step-a\n");
        repo.create_file(".lf/steps/step-a.md", "do step");
        repo.stage_all();
        repo.commit("add test flow");

        let db_path = tempdir().expect("tempdir").path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let source_wave = make_wave("source", repo.path(), "test-flow", WaveStatus::Running);
        let listener_wave = make_wave("listener", repo.path(), "test-flow", WaveStatus::Idle);
        store
            .create_wave(&source_wave)
            .await
            .expect("create source wave");
        store
            .create_wave(&listener_wave)
            .await
            .expect("create listener wave");
        let source_run = create_main_run(&store, &source_wave, WaveRunStatus::Running).await;

        let trigger = make_wave_trigger(listener_wave.id(), source_wave.id());
        store
            .create_trigger(&trigger)
            .await
            .expect("create wave trigger");

        let scheduler = Arc::new(Scheduler::new(0));
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

        executor
            .execute(&source_run.id)
            .await
            .expect("source run should complete");

        let pending = store
            .get_pending_for_trigger(listener_wave.id(), Some(&trigger.id))
            .await
            .expect("pending activation lookup should succeed");
        assert!(
            pending.is_some(),
            "wave activation should queue when scheduler is full"
        );

        let listener_runs = store
            .list_wave_runs(Some(listener_wave.id()), None)
            .await
            .expect("list listener runs");
        assert!(
            listener_runs.is_empty(),
            "listener should not start immediately when scheduler is full"
        );
    }

    #[tokio::test]
    async fn failed_terminal_session_marks_run_failed() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path();
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let (wave_id, run_id) = create_wave_and_run(&store, repo, "missing-flow").await;
        let mut run = store
            .get_wave_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        run.status = WaveRunStatus::Waiting;
        store
            .update_wave_run(&run)
            .await
            .expect("run update should succeed");

        let session_id = LfdId::new();
        let session = TerminalSession {
            id: session_id.clone(),
            wave_id: wave_id.clone(),
            wave_run_id: Some(run_id.clone()),
            step: "design".to_string(),
            agent: "claude".to_string(),
            cwd: repo.to_string_lossy().to_string(),
            argv: vec!["claude".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            status: TerminalSessionStatus::Failed,
            attached_at: None,
            started_at: None,
            completed_at: Some(OffsetDateTime::now_utc()),
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        };
        store
            .create_terminal_session(&session)
            .await
            .expect("terminal session should be created");

        let scheduler = Arc::new(Scheduler::new(1));
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

        executor
            .wait_for_terminal_session_and_resume(
                wave_id.clone(),
                run_id.clone(),
                session_id.clone(),
            )
            .await
            .expect("resume should succeed");

        let updated_run = store
            .get_wave_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, WaveRunStatus::Failed);
        assert!(updated_run
            .error
            .expect("failed run should include an error")
            .contains(&session_id.to_string()));

        let updated_wave = store
            .get_wave(&wave_id)
            .await
            .expect("wave lookup should succeed")
            .expect("wave should exist");
        assert_eq!(updated_wave.status, WaveStatus::Failed);
    }

    #[test]
    fn build_interactive_context_uses_design_and_mutation_artifacts() {
        let tmp = tempdir().expect("tempdir");
        let worktree = tmp.path();
        std::fs::create_dir_all(worktree.join("scratch")).expect("scratch dir");
        std::fs::write(
            worktree.join("scratch/feature-branch.md"),
            "# Design\n\nUse attention items for interactive steps.\n",
        )
        .expect("write design doc");
        std::fs::write(
            worktree.join("scratch/wave-mutate.md"),
            "# Mutation summary\n\n- Rebalance the PM wave.\n",
        )
        .expect("write mutate summary");

        let run = WaveRun {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            snapshot: WaveRunSnapshot {
                repo: worktree.to_string_lossy().to_string(),
                flow: "build".to_string(),
                direction: vec![],
                area: vec![],
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Waiting,
            worktree: worktree.to_string_lossy().to_string(),
            branch: "feature-branch".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "stack".to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        };
        let session = TerminalSession {
            id: LfdId::new(),
            wave_id: run.wave_id.clone(),
            wave_run_id: Some(run.id.clone()),
            step: "review-design".to_string(),
            agent: "claude".to_string(),
            cwd: worktree.to_string_lossy().to_string(),
            argv: vec!["claude".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            status: TerminalSessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        };

        let review_step = ConcreteStep {
            step: Step::named("review-design"),
            flow_parents: vec![],
        };
        let review_details = interactive_attention_details(&review_step, &run);
        let review_context = build_interactive_context(&review_step, &session, &review_details);
        assert_eq!(review_context["step"], "review-design");
        assert_eq!(review_context["design_path"], "scratch/feature-branch.md");
        assert_eq!(
            interactive_summary(&review_details),
            "Use attention items for interactive steps."
        );

        let wave_review_step = ConcreteStep {
            step: Step::named("wave/review"),
            flow_parents: vec![],
        };
        let wave_review_details = interactive_attention_details(&wave_review_step, &run);
        let wave_review_context =
            build_interactive_context(&wave_review_step, &session, &wave_review_details);
        assert_eq!(wave_review_context["step"], "wave/review");
        assert!(wave_review_context["mutation_summary"]
            .as_str()
            .expect("mutation summary")
            .contains("Rebalance the PM wave."));
        assert_eq!(
            interactive_summary(&wave_review_details),
            "- Rebalance the PM wave."
        );
    }

    #[tokio::test]
    async fn wait_interactive_creates_attention_item() {
        let repo = TestRepo::new();
        repo.create_file(".lf/flows/test-flow.yaml", "- review-design\n");
        repo.create_file(
            "scratch/main.md",
            "# Design review\n\nSurface interactive checkpoints in the queue.\n",
        );
        repo.stage_all();
        repo.commit("add interactive flow");

        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );
        let (wave_id, run_id) = create_wave_and_run(&store, repo.path(), "test-flow").await;

        let scheduler = Arc::new(Scheduler::new(1));
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

        executor
            .execute(&run_id)
            .await
            .expect("execution should pause");

        let run = store
            .get_wave_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        assert_eq!(run.status, WaveRunStatus::Waiting);

        let items = store
            .list_attention_items(None, Some(AttentionKind::Interactive))
            .await
            .expect("attention items should load");
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.wave_id, wave_id);
        assert_eq!(item.run_id.as_ref(), Some(&run_id));
        assert_eq!(item.title, "Design review: fork-wave");
        assert_eq!(
            item.summary,
            "Surface interactive checkpoints in the queue."
        );
        assert_eq!(item.context["step"], "review-design");
        assert_eq!(item.context["design_path"], "scratch/main.md");
        assert!(item.context["terminal_session_id"].is_string());
    }

    #[tokio::test]
    async fn completed_terminal_session_resolves_interactive_attention() {
        let repo = TestRepo::new();
        repo.create_file(".lf/flows/test-flow.yaml", "- review-design\n");
        repo.create_file(
            "scratch/main.md",
            "# Design review\n\nSurface interactive checkpoints in the queue.\n",
        );
        repo.stage_all();
        repo.commit("add interactive flow");

        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );
        let (wave_id, run_id) = create_wave_and_run(&store, repo.path(), "test-flow").await;

        let mut run = store
            .get_wave_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        run.status = WaveRunStatus::Waiting;
        store
            .update_wave_run(&run)
            .await
            .expect("run update should succeed");

        let mut wave = store
            .get_wave(&wave_id)
            .await
            .expect("wave lookup should succeed")
            .expect("wave should exist");
        wave.status = WaveStatus::Waiting;
        store
            .update_wave(&wave)
            .await
            .expect("wave update should succeed");

        let session_id = LfdId::new();
        let session = TerminalSession {
            id: session_id.clone(),
            wave_id: wave_id.clone(),
            wave_run_id: Some(run_id.clone()),
            step: "review-design".to_string(),
            agent: "claude".to_string(),
            cwd: repo.path().to_string_lossy().to_string(),
            argv: vec!["claude".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            status: TerminalSessionStatus::Succeeded,
            attached_at: None,
            started_at: None,
            completed_at: Some(OffsetDateTime::now_utc()),
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        };
        store
            .create_terminal_session(&session)
            .await
            .expect("terminal session should be created");

        let attention_item = AttentionItem {
            id: LfdId::new(),
            wave_id: wave_id.clone(),
            run_id: Some(run_id.clone()),
            kind: AttentionKind::Interactive,
            status: AttentionStatus::Surfaced,
            title: "Design review: fork-wave".to_string(),
            summary: "Surface interactive checkpoints in the queue.".to_string(),
            context: json!({
                "step": "review-design",
                "terminal_session_id": session_id,
                "design_path": "scratch/main.md",
            }),
            surfaced_at: OffsetDateTime::now_utc(),
            viewed_at: None,
            resolved_at: None,
        };
        store
            .upsert_attention_item(&attention_item)
            .await
            .expect("attention item should be created");

        let scheduler = Arc::new(Scheduler::new(1));
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

        executor
            .wait_for_terminal_session_and_resume(
                wave_id.clone(),
                run_id.clone(),
                session_id.clone(),
            )
            .await
            .expect("resume should succeed");

        let resolved = store
            .get_attention_item(&attention_item.id)
            .await
            .expect("attention lookup should succeed")
            .expect("attention item should exist");
        assert_eq!(resolved.status, AttentionStatus::Resolved);
        assert!(resolved.resolved_at.is_some());
    }

    #[tokio::test]
    async fn terminal_launch_config_uses_concerto_surface() {
        let tmp = tempdir().expect("tempdir");
        let repo = tmp.path();
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let (wave_id, run_id) = create_wave_and_run(&store, repo, "missing-flow").await;
        let wave = store
            .get_wave(&wave_id)
            .await
            .expect("wave lookup should succeed")
            .expect("wave should exist");
        let run = store
            .get_wave_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        let step = ConcreteStep {
            step: crate::engine::flow::Step::named("design"),
            flow_parents: Vec::new(),
        };

        let (launch, process, _) = build_terminal_launch_config(&wave, &run, &step)
            .await
            .expect("launch config should build");

        assert_eq!(launch.cwd.expect("cwd"), PathBuf::from(&run.worktree));
        assert!(!process.auto);
        assert!(!process.stream);
        assert!(process.context_file.is_some());
    }

    #[tokio::test]
    async fn execute_runs_fork_with_docker_executor() {
        let repo = TestRepo::new();
        let tmp = tempdir().expect("tempdir");
        create_fork_flow_repo(
            &repo,
            r#"
- and:
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
- and:
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
- and:
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
- and:
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
- and:
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

    #[test]
    fn classify_repair_flow_returns_debug_for_ci_fix() {
        let mut run = WaveRun::new(LfdId::new(), LfdId::new());
        run.snapshot.flow = CI_FIX_FLOW.to_string();
        assert_eq!(classify_repair_flow(&run), "debug");
    }

    #[test]
    fn classify_repair_flow_returns_debug_for_regular_flow() {
        let mut run = WaveRun::new(LfdId::new(), LfdId::new());
        run.snapshot.flow = "build".to_string();
        assert_eq!(classify_repair_flow(&run), "debug");
    }

    #[tokio::test]
    async fn create_repair_run_links_to_failed_run() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );
        let repo = TestRepo::new();
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

        let wave = make_wave("test-wave", repo.path(), "build", WaveStatus::Failed);
        store.create_wave(&wave).await.unwrap();
        let failed_run = create_main_run(&store, &wave, WaveRunStatus::Failed).await;

        let repair = executor
            .create_repair_run(&wave, &failed_run, "debug")
            .await
            .unwrap();

        assert_eq!(repair.repair_of.as_ref().unwrap(), &failed_run.id);
        assert_eq!(repair.snapshot.flow, "debug");
        assert_eq!(repair.worktree, failed_run.worktree);
        assert_eq!(repair.branch, failed_run.branch);
        assert_eq!(repair.status, WaveRunStatus::Running);

        // Verify persisted
        let loaded = store.get_wave_run(&repair.id).await.unwrap().unwrap();
        assert_eq!(loaded.repair_of.unwrap(), failed_run.id);
    }
}
