mod launch;
mod summary;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::process::Command;
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

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

        debug!(
            source_wave_id = %source_wave_id,
            trigger_count = triggers.len(),
            "trigger_listeners_on_completion: found triggers"
        );

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
            debug!(
                listener_wave_id = %listener_wave.id(),
                listener_workers = listener_wave.workers(),
                "trigger_listeners_on_completion: dispatching activation"
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
            debug!(
                listener_wave_id = %listener_wave.id(),
                activated,
                "trigger_listeners_on_completion: activation result"
            );
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
        let session_name = tmux_session_name(&session.id);
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
                &session_name,
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

        let mut running = session.clone();
        let _ = running.start();
        self.store.update_terminal_session(&running).await?;
        self.event_hub
            .send(Event::terminal_session_updated(running.clone()));
        Ok(running)
    }

    /// Block until the tmux session exits and return the exit code.
    async fn wait_for_tmux_session_exit(&self, session: &TerminalSession) -> Result<i32> {
        let session_name = tmux_session_name(&session.id);
        let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);

        loop {
            let status = Command::new("tmux")
                .args(["has-session", "-t", &session_name])
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

        if let Some(mut stored) = self.store.get_terminal_session(&session.id).await? {
            if stored.complete(exit_code) {
                self.store.update_terminal_session(&stored).await?;
                self.event_hub
                    .send(Event::terminal_session_updated(stored.clone()));
            }
        }

        let _ = tokio::task::spawn_blocking(move || std::fs::remove_file(&exit_file)).await;
        Ok(exit_code)
    }
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
        let _ = tracing_subscriber::fmt::try_init();
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
        let _ = tracing_subscriber::fmt::try_init();
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
        let _ = tracing_subscriber::fmt::try_init();
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
