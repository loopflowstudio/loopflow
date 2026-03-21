mod launch;
mod summary;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::process::Command;
use tokio::time::Duration;
use tracing::{error, info, warn};

use time::OffsetDateTime;

use crate::engine::worktree::remove_worktree;
use crate::lfd::config::{ExecutorConfig, ExecutorType, GitHubConfig};
use crate::lfd::events::EventHub;
use crate::lfd::http::routes::infer_wave_git_state_for_worktree;
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::triggers::{dispatch_or_enqueue_activation, ActivationEnvelope};
use crate::lfd::types::{
    tmux_session_name, Event, LivePrState, LivePullRequestState, Signal, TerminalSession,
    TerminalSessionStatus, Wave, WaveMode, WaveRun, WaveRunSnapshot, WaveRunStatus, WaveStatus,
    CI_FIX_FLOW, TMUX_TERMINAL_SOURCE,
};

use super::docker::DockerExecutor;
use super::helpers::{
    advance_branch, auto_create_pr, build_lf_step_command, cleanup_run_worktree,
    is_active_wave_run_status, is_ephemeral_worktree_path,
};
use super::local::LocalProcessExecutor;
use super::{AgentExecutor, JanitorReport, StartupRecovery};

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

fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn tmux_exit_file(cwd: &Path, session_id: &LfdId) -> PathBuf {
    cwd.join(".lf/tmp/terminal-sessions")
        .join(format!("{session_id}.exit"))
}

fn tmux_available() -> bool {
    if std::env::var_os("LFD_DISABLE_TMUX").is_some() {
        return false;
    }
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|output| output.status.success())
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
    disable_tmux: bool,
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
            disable_tmux: false,
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
            disable_tmux: true,
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
        self.runner.recover_startup(&self.output).await
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
        // Collect worktrees belonging to active runs so we don't remove them.
        let mut active_paths = HashSet::new();
        let runs = self.store.list_wave_runs(None, None).await?;
        for run in runs {
            if is_active_wave_run_status(run.status) {
                active_paths.insert(run.worktree);
            }
        }

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

        info!(run_id = %run.id, flow = %run.snapshot.flow, repo = %run.snapshot.repo, "launching flow in tmux session");

        let session_id = LfdId::new();
        let env = flow_step_env(wave.id(), &run.id, Some(&session_id));
        let cmd = build_lf_step_command(
            &run.snapshot.flow,
            true,
            &run.snapshot.direction,
            &run.snapshot.area,
            wave.name(),
        );
        let tmux_managed = !self.disable_tmux && tmux_available();
        let terminal_session = TerminalSession {
            id: session_id.clone(),
            wave_id: wave.id().clone(),
            wave_run_id: Some(run.id.clone()),
            step: run.snapshot.flow.clone(),
            agent: "lf".to_string(),
            cwd: run.worktree.clone(),
            argv: cmd.clone(),
            env: env.into_iter().collect::<BTreeMap<_, _>>(),
            source: if tmux_managed {
                TMUX_TERMINAL_SOURCE.to_string()
            } else {
                "wave_run".to_string()
            },
            tmux_name: tmux_session_name(&run.branch),
            status: TerminalSessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: (!tmux_managed).then(|| session_id.to_string()),
        };
        self.store
            .create_terminal_session(&terminal_session)
            .await?;
        self.event_hub
            .send(Event::terminal_session_created(terminal_session.clone()));

        let exit_code = if tmux_managed {
            let session = self.launch_tmux_terminal_session(terminal_session).await?;
            self.wait_for_tmux_session_exit(&session).await?
        } else {
            // Fallback: run directly via the agent launcher.
            let outcome = self
                .launch_agent(launch::AgentLaunchRequest {
                    wave_id: wave.id().clone(),
                    wave_run_id: run.id.clone(),
                    branch: Some(run.branch.clone()),
                    repo: run.snapshot.repo.clone(),
                    worktree: run.worktree.clone(),
                    step: crate::engine::flow::ConcreteStep {
                        step: crate::engine::flow::Step::named(&run.snapshot.flow),
                        flow_parents: Vec::new(),
                    },
                    agent: "lf".to_string(),
                    cmd,
                    output_prefix: None,
                    extra_env: flow_step_env(wave.id(), &run.id, Some(&session_id)),
                })
                .await?;
            outcome.exit_code
        };

        if exit_code == 0 {
            self.finish_completed_run(&wave, &mut run).await
        } else {
            let flow_name = run.snapshot.flow.clone();
            self.fail_run(
                &mut run,
                &wave,
                format!("flow {flow_name} exited with code {exit_code}"),
            )
            .await?;
            Ok(())
        }
    }

    async fn finish_completed_run(&self, wave: &Wave, run: &mut WaveRun) -> Result<()> {
        run.status = WaveRunStatus::Completed;
        run.ended_at = Some(OffsetDateTime::now_utc());

        let is_recurring = matches!(wave.mode(), WaveMode::Loop | WaveMode::Cron);
        let should_manage_pr = run.target_branch == "main" || run.target_branch.is_empty();
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

        if should_manage_pr && run.pr.is_some() && is_recurring {
            let wt = run.worktree.clone();
            let name = wave.name().clone();
            match tokio::task::spawn_blocking(move || advance_branch(Path::new(&wt), &name)).await {
                Ok(Ok(new_branch)) => {
                    info!(
                        run_id = %run.id,
                        new_branch = %new_branch,
                        "advanced to new branch for next iteration"
                    );
                }
                Ok(Err(err)) => {
                    warn!(run_id = %run.id, error = %err, "failed to advance branch");
                }
                Err(err) => {
                    warn!(run_id = %run.id, error = %err, "advance_branch task panicked");
                }
            }
        }

        self.store.update_wave_run(run).await?;
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

        let wt = Path::new(&run.worktree);
        if let Err(err) = cleanup_run_worktree(wt) {
            warn!(run_id = %run.id, error = %err, "failed to clean up run worktree");
        }

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
        Ok(())
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
            let activated = dispatch_or_enqueue_activation(
                &self.store,
                self,
                &self.scheduler,
                &self.event_hub,
                &listener_wave,
                trigger.flow.clone(),
                envelope,
            )
            .await;
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
            execution_cursor: None,
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

    async fn launch_tmux_terminal_session(
        &self,
        session: TerminalSession,
    ) -> Result<TerminalSession> {
        let session_name = &session.tmux_name;
        let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);
        let exit_dir = exit_file
            .parent()
            .expect("tmux exit file should always have a parent");
        let env_prefix = session
            .env
            .iter()
            .map(|(key, value)| format!("{key}={} ", shell_escape(value)))
            .collect::<String>();
        let command = session
            .argv
            .iter()
            .map(|arg| shell_escape(arg))
            .collect::<Vec<_>>()
            .join(" ");
        let shell_command = format!(
            "mkdir -p {exit_dir}; rm -f {exit_file}; {env_prefix}{command}; EXIT_CODE=$?; printf '%s' \"$EXIT_CODE\" > {exit_file}; exit \"$EXIT_CODE\"",
            exit_dir = shell_escape(&exit_dir.display().to_string()),
            exit_file = shell_escape(&exit_file.display().to_string()),
        );

        let status = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                session_name,
                "-c",
                &session.cwd,
                "/bin/zsh",
                "-lc",
                &shell_command,
            ])
            .status()
            .await?;
        if !status.success() {
            return Err(anyhow!("tmux failed to launch terminal session"));
        }

        // Enable mouse mode so scroll events reach tmux rather than the inner shell.
        let _ = Command::new("tmux")
            .args(["set-option", "-t", session_name, "mouse", "on"])
            .status()
            .await;

        let mut running = session.clone();
        let _ = running.start();
        self.store.update_terminal_session(&running).await?;
        self.event_hub
            .send(Event::terminal_session_updated(running.clone()));
        Ok(running)
    }

    /// Block until the tmux session exits and return the exit code.
    async fn wait_for_tmux_session_exit(&self, session: &TerminalSession) -> Result<i32> {
        let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);

        loop {
            let status = Command::new("tmux")
                .args(["has-session", "-t", &session.tmux_name])
                .status()
                .await;
            match status {
                Ok(status) if status.success() => {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Ok(_) => break,
                Err(err) => return Err(anyhow!("tmux session probe failed: {err}")),
            }
        }

        let exit_code = tokio::task::spawn_blocking({
            let exit_file = exit_file.clone();
            move || std::fs::read_to_string(&exit_file).ok()
        })
        .await
        .map_err(|err| anyhow!("terminal exit file task failed: {err}"))?
        .and_then(|text| text.trim().parse::<i32>().ok())
        .unwrap_or(1);

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
    AttentionItem {
        id: LfdId::new(),
        wave_id: wave.id().clone(),
        run_id: Some(run.id.clone()),
        kind: AttentionKind::Interactive,
        status: AttentionStatus::Surfaced,
        title: interactive_title(step, wave),
        summary: interactive_summary(step, run),
        context: build_interactive_context(step, terminal_session, run),
        surfaced_at: OffsetDateTime::now_utc(),
        viewed_at: None,
        resolved_at: None,
    }
}

fn build_interactive_context(
    step: &ConcreteStep,
    terminal_session: &TerminalSession,
    run: &WaveRun,
) -> Value {
    let mut context = json!({
        "step": step.step.name.clone(),
        "terminal_session_id": terminal_session.id.clone(),
    });

    match step.step.name.as_str() {
        "review-design" => {
            if let Some(design_path) =
                find_review_design_path(Path::new(&run.worktree), &run.branch)
            {
                context["design_path"] = Value::String(design_path);
            }
        }
        "wave/review" => {
            if let Some(summary) =
                read_context_file(Path::new(&run.worktree), "scratch/wave-mutate.md")
            {
                context["mutation_summary"] = Value::String(summary);
            }
        }
        _ => {}
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

fn interactive_summary(step: &ConcreteStep, run: &WaveRun) -> String {
    let worktree = Path::new(&run.worktree);
    let source = match step.step.name.as_str() {
        "review-design" => find_review_design_path(worktree, &run.branch)
            .and_then(|path| std::fs::read_to_string(worktree.join(path)).ok()),
        "wave/review" => read_context_file(worktree, "scratch/wave-mutate.md"),
        _ => None,
    };

    source
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

fn flow_step_env(
    wave_id: &LfdId,
    run_id: &LfdId,
    session_id: Option<&LfdId>,
) -> Vec<(String, String)> {
    let mut env = vec![
        ("LFD_WAVE_ID".to_string(), wave_id.to_string()),
        ("LF_RUN_ID".to_string(), run_id.to_string()),
    ];
    if let Some(session_id) = session_id {
        env.push(("LFD_SESSION_ID".to_string(), session_id.to_string()));
    }
    env
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
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Signal, Trigger, WaveRunSnapshot};
    use async_trait::async_trait;
    use loopflow_test_support::TestRepo;
    use tempfile::tempdir;

    struct MockRunner;

    #[async_trait]
    impl AgentExecutor for MockRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            _cwd: &Path,
            _context: super::super::AgentRunContext,
        ) -> Result<i32> {
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
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
            workers: 1,
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
            execution_cursor: None,
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
            workers: 1,
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
            execution_cursor: None,
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
            workers: 1,
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
        listener_wave.workers = 1;
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
        let review_context = build_interactive_context(&review_step, &session, &run);
        assert_eq!(review_context["step"], "review-design");
        assert_eq!(review_context["design_path"], "scratch/feature-branch.md");
        assert_eq!(
            interactive_summary(&review_step, &run),
            "Use attention items for interactive steps."
        );

        let wave_review_step = ConcreteStep {
            step: Step::named("wave/review"),
            flow_parents: vec![],
        };
        let wave_review_context = build_interactive_context(&wave_review_step, &session, &run);
        assert_eq!(wave_review_context["step"], "wave/review");
        assert!(wave_review_context["mutation_summary"]
            .as_str()
            .expect("mutation summary")
            .contains("Rebalance the PM wave."));
        assert_eq!(
            interactive_summary(&wave_review_step, &run),
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
