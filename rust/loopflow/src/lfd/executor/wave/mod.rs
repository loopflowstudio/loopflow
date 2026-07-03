mod launch;
mod summary;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
#[cfg(test)]
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::Duration;
use tracing::{error, info, warn};

use time::OffsetDateTime;

#[cfg(test)]
use crate::engine::flow::ConcreteStep;
use crate::engine::flow::InFlightDispatch;
use crate::engine::worktree::remove_worktree;
use crate::lfd::config::{ExecutorConfig, ExecutorType, GitHubConfig};
use crate::lfd::events::EventHub;
use crate::lfd::http::routes::{infer_wave_git_state_for_worktree, is_open_pr_state};
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputHub;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
#[cfg(test)]
use crate::lfd::triggers::spawn_run_task_with_slot;
use crate::lfd::triggers::{
    dispatch_wave_if_ready, enqueue_pending_activation, spawn_immediate_activation,
    ActivationEnvelope, EnqueueOutcome, ImmediateActivation,
};
use crate::lfd::types::{
    tmux_session_name, Event, LivePrState, LivePullRequestState, Run, RunStatus, Session,
    SessionStatus, SessionUse, Signal, Wave, WaveMode, WaveStatus, CI_FIX_FLOW,
    LIVE_SESSION_STATUSES, PALETTE_TERMINAL_SOURCE, TMUX_TERMINAL_SOURCE,
};
#[cfg(test)]
use crate::lfd::types::{AttentionItem, AttentionKind, AttentionStatus};

use super::docker::DockerExecutor;
use super::helpers::{
    advance_branch, auto_create_pr, build_lf_inline_command, build_lf_step_command,
    cleanup_run_worktree, is_active_run_status, is_ephemeral_worktree_path,
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
    cwd.join(".lf/tmp/sessions")
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

async fn tmux_session_exists(session_name: &str) -> Result<bool> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .await
        .map_err(|err| anyhow!("tmux session probe failed: {err}"))?;
    Ok(status.success())
}

async fn read_tmux_exit_code(exit_file: PathBuf) -> Result<Option<i32>> {
    tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(&exit_file)
            .ok()
            .and_then(|text| text.trim().parse::<i32>().ok())
    })
    .await
    .map_err(|err| anyhow!("terminal exit file task failed: {err}"))
}

fn infer_branch_name(worktree: &str) -> Option<String> {
    std::process::Command::new("git")
        .args(["-C", worktree, "branch", "--show-current"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|branch| branch.trim().to_string())
        .filter(|branch| !branch.is_empty())
}

fn build_run_command(
    wave: &Wave,
    run: &Run,
    in_flight: Vec<InFlightDispatch>,
) -> Result<(Vec<String>, String)> {
    if let Some(task) = run.task.as_ref() {
        let mut cmd =
            build_lf_step_command(&run.flow, true, &run.direction, &run.area, wave.name());
        let flow_arg = cmd
            .get_mut(1)
            .ok_or_else(|| anyhow!("lf dispatch command missing flow argument"))?;
        flow_arg.push(':');
        cmd.push(task.clone());
        return Ok((cmd, format!("dispatch:{}", run.flow)));
    }

    build_wave_agent_command(wave, &run.worktree, &run.direction, &run.area, in_flight)
}

fn build_wave_agent_command(
    wave: &Wave,
    worktree: &str,
    direction: &[String],
    area: &[String],
    in_flight: Vec<InFlightDispatch>,
) -> Result<(Vec<String>, String)> {
    let repo = Path::new(worktree);
    let goal = crate::engine::load_goal(wave.goal(), repo)?;
    let memory = read_wave_memory(repo, wave.name())?;
    let prompt = crate::engine::render_goal(
        &goal,
        &crate::engine::GoalRenderContext {
            flows: crate::engine::available_flow_names(repo),
            roadmap: format!("wave/{}", wave.name()),
            memory,
            metrics: wave.metrics().clone(),
            in_flight,
        },
    );
    let cmd = build_lf_inline_command(&prompt, true, direction, area, wave.name());
    Ok((cmd, format!("goal:{}", wave.goal())))
}

/// Runs that are still dispatched — active status or an open PR — excluding
/// `exclude_run_id` (the run about to launch, which isn't in flight yet).
async fn list_in_flight_dispatches(
    store: &SharedStore,
    wave_id: &LfdId,
    exclude_run_id: Option<&LfdId>,
) -> Result<Vec<InFlightDispatch>> {
    let runs = store.list_runs(Some(wave_id), None).await?;
    let in_flight = runs
        .into_iter()
        .filter(|run| exclude_run_id != Some(&run.id))
        .filter(|run| {
            is_active_run_status(run.status)
                || is_open_pr_state(run.pr.as_ref().and_then(|pr| pr.state.as_deref()))
        })
        .map(|run| InFlightDispatch {
            task: run.task.clone(),
            flow: run.flow.clone(),
            status: format!("{:?}", run.status).to_lowercase(),
            pr_url: run.pr.as_ref().map(|pr| pr.url.clone()),
            pr_state: run.pr.as_ref().and_then(|pr| pr.state.clone()),
        })
        .collect();
    Ok(in_flight)
}

fn read_wave_memory(repo: &Path, wave_name: &str) -> Result<String> {
    let path = repo.join("wave").join(wave_name).join("MEMORY.md");
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(anyhow!("failed to read wave memory: {err}")),
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

    pub async fn launch_palette_session(
        &self,
        wave_id: &LfdId,
        flow: &str,
        worktree: &str,
        agent: &str,
    ) -> Result<Session> {
        if self.disable_tmux || !tmux_available() {
            return Err(anyhow!("tmux is required for palette terminal sessions"));
        }
        let wave = self
            .store
            .get_wave(wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave not found"))?;
        let flow = flow.trim();
        if flow.is_empty() {
            return Err(anyhow!("flow is required"));
        }
        let agent = agent.trim();
        if agent.is_empty() {
            return Err(anyhow!("agent is required"));
        }

        let session_id = LfdId::new();
        let mut cmd = build_lf_step_command(flow, false, &wave.direction, &wave.area, wave.name());
        cmd.push("-m".to_string());
        cmd.push(agent.to_string());

        let branch = infer_branch_name(worktree)
            .unwrap_or_else(|| format!("{}-{}", wave.name(), session_id));
        let session = Session {
            id: session_id.clone(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::Palette,
            step: flow.to_string(),
            agent: agent.to_string(),
            cwd: worktree.to_string(),
            argv: cmd,
            env: BTreeMap::from([
                ("LFD_WAVE_ID".to_string(), wave.id().to_string()),
                ("LFD_SESSION_ID".to_string(), session_id.to_string()),
                ("LFD_AGENT_ROLE".to_string(), "palette".to_string()),
            ]),
            source: PALETTE_TERMINAL_SOURCE.to_string(),
            tmux_name: tmux_session_name(&format!("{branch}-{}", session_id.as_str())),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        };
        self.store.create_control_session(&session).await?;
        self.event_hub.send(Event::session_created(session.clone()));

        let running = self.launch_tmux_session(session).await?;
        self.spawn_palette_completion_watcher(running.clone());
        Ok(running)
    }

    pub async fn launch_wave_agent_session(&self, wave_id: &LfdId) -> Result<Session> {
        let wave = self
            .store
            .get_wave(wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave not found"))?;
        self.ensure_wave_workspace(&wave).await?;

        let worktree =
            crate::engine::worktrees::worktree_path(Path::new(wave.primary_repo()), wave.name());
        let worktree = worktree.display().to_string();
        let session_id = LfdId::new();
        let in_flight = list_in_flight_dispatches(&self.store, wave.id(), None).await?;
        let (cmd, terminal_step) =
            build_wave_agent_command(&wave, &worktree, wave.direction(), wave.area(), in_flight)?;
        let branch = infer_branch_name(&worktree)
            .unwrap_or_else(|| format!("{}-{}", wave.name(), session_id));
        let tmux_managed = !self.disable_tmux && tmux_available();
        let session = Session {
            id: session_id.clone(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            step: terminal_step,
            agent: "lf".to_string(),
            cwd: worktree,
            argv: cmd,
            env: BTreeMap::from([
                ("LFD_WAVE_ID".to_string(), wave.id().to_string()),
                ("LFD_SESSION_ID".to_string(), session_id.to_string()),
                (
                    "LFD_AGENT_ROLE".to_string(),
                    SessionUse::WaveAgent.as_str().to_string(),
                ),
            ]),
            source: if tmux_managed {
                TMUX_TERMINAL_SOURCE.to_string()
            } else {
                "wave_agent".to_string()
            },
            tmux_name: tmux_session_name(&format!("{branch}-{}", session_id.as_str())),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: (!tmux_managed).then(|| session_id.to_string()),
        };
        self.store.create_control_session(&session).await?;
        self.event_hub.send(Event::session_created(session.clone()));

        if tmux_managed {
            let running = self.launch_tmux_session(session).await?;
            self.spawn_session_completion_watcher(running.clone());
            Ok(running)
        } else {
            self.spawn_process_session(session.clone());
            Ok(session)
        }
    }

    pub async fn reconcile_sessions(&self) -> Result<u32> {
        let active = self
            .store
            .list_control_sessions(None, Some(LIVE_SESSION_STATUSES))
            .await?;
        let mut completed = 0;
        for session in active {
            if !session.is_tmux_backed() {
                continue;
            }
            if tmux_session_exists(&session.tmux_name).await? {
                if session.source == PALETTE_TERMINAL_SOURCE {
                    self.spawn_palette_completion_watcher(session);
                }
                continue;
            }
            let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);
            let exit_code = read_tmux_exit_code(exit_file.clone()).await?.unwrap_or(1);
            if self.complete_session(&session.id, exit_code).await? {
                completed += 1;
            }
            let _ = tokio::task::spawn_blocking(move || std::fs::remove_file(&exit_file)).await;
        }
        Ok(completed)
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
        let runs = self.store.list_runs(None, None).await?;
        for run in runs {
            if is_active_run_status(run.status) {
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

    pub async fn prepare_session_for_run(&self, run_id: &LfdId) -> Result<Session> {
        self.prepare_session_for_run_with_parent(run_id, None).await
    }

    pub async fn prepare_session_for_run_with_parent(
        &self,
        run_id: &LfdId,
        parent_session_id: Option<LfdId>,
    ) -> Result<Session> {
        let run = self
            .store
            .get_run(run_id)
            .await?
            .ok_or_else(|| anyhow!("wave run not found"))?;
        let wave = self
            .store
            .get_wave(&run.wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave not found"))?;
        let session_id = LfdId::new();
        let mut env = flow_step_env(wave.id(), &run.id, Some(&session_id));
        let in_flight = list_in_flight_dispatches(&self.store, &run.wave_id, Some(&run.id)).await?;
        let (cmd, terminal_step) = build_run_command(&wave, &run, in_flight)?;
        let session_use = if run.task.is_some() {
            SessionUse::Worker
        } else {
            SessionUse::WaveAgent
        };
        env.push((
            "LFD_AGENT_ROLE".to_string(),
            session_use.as_str().to_string(),
        ));
        let tmux_managed = !self.disable_tmux && tmux_available();
        let session = Session {
            id: session_id.clone(),
            wave_id: wave.id().clone(),
            run_id: Some(run.id.clone()),
            parent_session_id,
            session_use,
            step: terminal_step,
            agent: "lf".to_string(),
            cwd: run.worktree.clone(),
            argv: cmd,
            env: env.into_iter().collect::<BTreeMap<_, _>>(),
            source: if tmux_managed {
                TMUX_TERMINAL_SOURCE.to_string()
            } else {
                "run".to_string()
            },
            tmux_name: tmux_session_name(&run.branch),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: (!tmux_managed).then(|| session_id.to_string()),
        };
        self.store.create_control_session(&session).await?;
        self.event_hub.send(Event::session_created(session.clone()));
        Ok(session)
    }

    pub async fn execute(&self, run_id: &LfdId) -> Result<()> {
        if let Some(run) = self.store.get_run(run_id).await? {
            if run.status == RunStatus::Completed || run.status == RunStatus::Failed {
                return Ok(());
            }
        }
        let session = self.prepare_session_for_run(run_id).await?;
        self.execute_with_prepared_session(run_id, session).await
    }

    pub async fn execute_with_session(&self, run_id: &LfdId, session_id: &LfdId) -> Result<()> {
        let session = self
            .store
            .get_control_session(session_id)
            .await?
            .ok_or_else(|| anyhow!("terminal session {session_id} not found"))?;
        if session.run_id.as_ref() != Some(run_id) {
            return Err(anyhow!(
                "terminal session {session_id} does not belong to wave run {run_id}"
            ));
        }
        self.execute_with_prepared_session(run_id, session).await
    }

    async fn execute_with_prepared_session(&self, run_id: &LfdId, session: Session) -> Result<()> {
        let mut run = self
            .store
            .get_run(run_id)
            .await?
            .ok_or_else(|| anyhow!("wave run not found"))?;
        if run.status == RunStatus::Completed || run.status == RunStatus::Failed {
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

        info!(run_id = %run.id, flow = %run.flow, repo = %run.repo, "launching flow in tmux session");

        let exit_code = if session.is_tmux_backed() {
            let session = self.launch_tmux_session(session).await?;
            self.wait_for_tmux_session_exit(&session).await?
        } else {
            let mut running = session.clone();
            let _ = running.start();
            self.store.update_control_session(&running).await?;
            self.event_hub.send(Event::session_updated(running.clone()));
            // Fallback: run directly via the agent launcher.
            let outcome = self
                .launch_agent(launch::ExecutionProcessRequest {
                    wave_id: wave.id().clone(),
                    run_id: run.id.clone(),
                    branch: Some(run.branch.clone()),
                    repo: run.repo.clone(),
                    worktree: run.worktree.clone(),
                    step: crate::engine::flow::ConcreteStep {
                        step: crate::engine::flow::Step::named(&running.step),
                        flow_parents: Vec::new(),
                    },
                    agent: running.agent.clone(),
                    cmd: running.argv.clone(),
                    output_prefix: None,
                    extra_env: running.env.clone().into_iter().collect(),
                })
                .await?;
            self.complete_session(&running.id, outcome.exit_code)
                .await?;
            outcome.exit_code
        };

        if exit_code == 0 {
            self.finish_completed_run(&wave, &mut run).await
        } else {
            let flow_name = run.flow.clone();
            self.fail_run(
                &mut run,
                &wave,
                format!("flow {flow_name} exited with code {exit_code}"),
            )
            .await?;
            Ok(())
        }
    }

    async fn finish_completed_run(&self, wave: &Wave, run: &mut Run) -> Result<()> {
        run.status = RunStatus::Completed;
        run.ended_at = Some(OffsetDateTime::now_utc());

        let is_recurring = matches!(wave.mode(), WaveMode::Loop);
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
                    repo_id: run.repo.clone(),
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

        self.store.update_run(run).await?;
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
            .get_active_run(wave.id())
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
            let activated = if listener_wave.workers() == 1 {
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
                match spawn_immediate_activation(
                    &self.store,
                    self,
                    &self.scheduler,
                    &self.event_hub,
                    ImmediateActivation {
                        wave: &listener_wave,
                        flow_override: trigger.flow.clone(),
                        roadmap_item: None,
                        force_parallel: false,
                        envelope,
                    },
                )
                .await
                {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(err) => {
                        warn!(
                            wave_id = %listener_wave.id(),
                            trigger_id = %trigger.id,
                            error = %err,
                            "failed to activate listener wave immediately"
                        );
                        false
                    }
                }
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

    async fn fail_run(&self, run: &mut Run, wave: &Wave, error: String) -> Result<()> {
        run.status = RunStatus::Failed;
        run.ended_at = Some(OffsetDateTime::now_utc());
        run.error = Some(error.clone());
        self.store.update_run(run).await?;

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
        failed_run: &Run,
        repair_flow: &str,
    ) -> Result<Run> {
        let repair_run = Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: failed_run.repo.clone(),
            flow: repair_flow.to_string(),
            task: None,
            direction: failed_run.direction.clone(),
            area: failed_run.area.clone(),
            iteration: failed_run.iteration,
            step_index: 0,
            status: RunStatus::Running,
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
        self.store.create_run(&repair_run).await?;
        Ok(repair_run)
    }

    async fn launch_tmux_session(&self, session: Session) -> Result<Session> {
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
        let tail = if session.source == PALETTE_TERMINAL_SOURCE {
            r#"exec "${SHELL:-/bin/zsh}""#
        } else {
            r#"exit "$EXIT_CODE""#
        };
        let shell_command = format!(
            "mkdir -p {exit_dir}; rm -f {exit_file}; {env_prefix}{command}; EXIT_CODE=$?; printf '%s' \"$EXIT_CODE\" > {exit_file}; {tail}",
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
        self.store.update_control_session(&running).await?;
        self.event_hub.send(Event::session_updated(running.clone()));
        Ok(running)
    }

    /// Block until the tmux session exits and return the exit code.
    async fn wait_for_tmux_session_exit(&self, session: &Session) -> Result<i32> {
        let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);

        while tmux_session_exists(&session.tmux_name).await? {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let exit_code = read_tmux_exit_code(exit_file.clone()).await?.unwrap_or(1);

        self.complete_session(&session.id, exit_code).await?;

        let _ = tokio::task::spawn_blocking(move || std::fs::remove_file(&exit_file)).await;
        Ok(exit_code)
    }

    fn spawn_palette_completion_watcher(&self, session: Session) {
        let executor = self.clone();
        tokio::spawn(async move {
            if let Err(err) = executor.wait_for_palette_session_completion(&session).await {
                warn!(session_id = %session.id, error = %err, "palette terminal completion watcher failed");
            }
        });
    }

    fn spawn_session_completion_watcher(&self, session: Session) {
        let executor = self.clone();
        tokio::spawn(async move {
            if let Err(err) = executor.wait_for_tmux_session_exit(&session).await {
                warn!(session_id = %session.id, error = %err, "terminal completion watcher failed");
            }
        });
    }

    fn spawn_process_session(&self, session: Session) {
        let executor = self.clone();
        tokio::spawn(async move {
            if let Err(err) = executor.run_process_session(session.clone()).await {
                warn!(session_id = %session.id, error = %err, "process terminal session failed");
                let _ = executor.complete_session(&session.id, 1).await;
            }
        });
    }

    async fn run_process_session(&self, session: Session) -> Result<()> {
        let mut running = session.clone();
        let _ = running.start();
        self.store.update_control_session(&running).await?;
        self.event_hub.send(Event::session_updated(running.clone()));

        let mut argv = running.argv.iter();
        let program = argv
            .next()
            .ok_or_else(|| anyhow!("terminal session command is empty"))?;
        let status = Command::new(program)
            .args(argv)
            .current_dir(&running.cwd)
            .envs(&running.env)
            .status()
            .await?;
        self.complete_session(&running.id, status.code().unwrap_or(1))
            .await?;
        Ok(())
    }

    async fn wait_for_palette_session_completion(&self, session: &Session) -> Result<i32> {
        let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);
        loop {
            if std::fs::metadata(&exit_file).is_ok() {
                break;
            }
            if !tmux_session_exists(&session.tmux_name).await? {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let exit_code = read_tmux_exit_code(exit_file.clone()).await?.unwrap_or(1);
        self.complete_session(&session.id, exit_code).await?;
        let _ = tokio::task::spawn_blocking(move || std::fs::remove_file(&exit_file)).await;
        Ok(exit_code)
    }

    async fn complete_session(&self, session_id: &LfdId, exit_code: i32) -> Result<bool> {
        if let Some(mut stored) = self.store.get_control_session(session_id).await? {
            if stored.complete(exit_code) {
                self.store.update_control_session(&stored).await?;
                self.event_hub.send(Event::session_updated(stored.clone()));
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(test)]
    pub(crate) async fn wait_for_session_and_resume(
        &self,
        wave_id: LfdId,
        run_id: LfdId,
        session_id: LfdId,
    ) -> Result<()> {
        let status = self.wait_for_session_status(&session_id).await?;
        let mut run = self
            .store
            .get_run(&run_id)
            .await?
            .ok_or_else(|| anyhow!("wave run {run_id} not found"))?;
        let wave = self
            .store
            .get_wave(&wave_id)
            .await?
            .ok_or_else(|| anyhow!("wave {wave_id} not found"))?;

        // Resolve the attention item for this session.
        if let Ok(items) = self
            .store
            .list_attention_items(None, Some(AttentionKind::Interactive))
            .await
        {
            for item in items {
                if item.context.get("session_id").and_then(|v| v.as_str())
                    == Some(session_id.as_str())
                {
                    let mut resolved = item.clone();
                    resolved.status = AttentionStatus::Resolved;
                    resolved.resolved_at = Some(OffsetDateTime::now_utc());
                    let _ = self.store.upsert_attention_item(&resolved).await;
                }
            }
        }

        if status != SessionStatus::Succeeded {
            let error_msg = format!("terminal session {session_id} failed");
            self.fail_run(&mut run, &wave, error_msg).await?;
            return Ok(());
        }

        self.set_wave_status(wave.id(), WaveStatus::Running).await;
        self.resume_run_execution(run).await?;
        Ok(())
    }

    #[cfg(test)]
    async fn wait_for_session_status(&self, session_id: &LfdId) -> Result<SessionStatus> {
        loop {
            let session = self
                .store
                .get_control_session(session_id)
                .await
                .map_err(|err| anyhow!("failed to load terminal session {session_id}: {err}"))?
                .ok_or_else(|| anyhow!("terminal session {session_id} not found"))?;
            if session.status.is_terminal() {
                return Ok(session.status);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    #[cfg(test)]
    async fn resume_run_execution(&self, run: Run) -> Result<()> {
        // Retry for up to 60s (120 × 500ms) waiting for a scheduler slot.
        for _ in 0..120 {
            if let Some(current) = self.store.get_run(&run.id).await? {
                if current.status != RunStatus::Running {
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
}

#[cfg(test)]
#[derive(Debug, Default)]
struct InteractiveAttentionDetails {
    design_path: Option<String>,
    mutation_summary: Option<String>,
    summary_source: Option<String>,
}

#[cfg(test)]
fn interactive_attention_details(step: &ConcreteStep, run: &Run) -> InteractiveAttentionDetails {
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
        "review" => {
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

#[cfg(test)]
fn build_interactive_context(
    step: &ConcreteStep,
    session: &Session,
    details: &InteractiveAttentionDetails,
) -> Value {
    let mut context = json!({
        "step": step.step.name.clone(),
        "session_id": session.id.clone(),
    });

    if let Some(design_path) = &details.design_path {
        context["design_path"] = Value::String(design_path.clone());
    }
    if let Some(summary) = &details.mutation_summary {
        context["mutation_summary"] = Value::String(summary.clone());
    }

    context
}

#[cfg(test)]
fn interactive_summary(details: &InteractiveAttentionDetails) -> String {
    details
        .summary_source
        .as_deref()
        .and_then(first_meaningful_line)
        .unwrap_or_default()
}

#[cfg(test)]
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

#[cfg(test)]
fn read_context_file(worktree: &Path, relative_path: &str) -> Option<String> {
    let text = std::fs::read_to_string(worktree.join(relative_path)).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
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

fn flow_step_env(
    wave_id: &LfdId,
    run_id: &LfdId,
    session_id: Option<&LfdId>,
) -> Vec<(String, String)> {
    let mut env = vec![
        ("LFD_WAVE_ID".to_string(), wave_id.to_string()),
        ("LFD_RUN_ID".to_string(), run_id.to_string()),
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
pub(crate) fn classify_repair_flow(failed_run: &Run) -> String {
    // If the original run was a CI-fix that failed, don't loop ci-fix → ci-fix.
    // The debug step is the right fallback for a failed repair tool.
    if failed_run.flow == CI_FIX_FLOW {
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
    use crate::engine::flow::Step;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{PullRequest, RepoWork, Signal, Trigger};
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
            _context: super::super::ExecutionContext,
        ) -> Result<i32> {
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct CapturingRunner {
        cmd: Mutex<Option<Vec<String>>>,
    }

    #[async_trait]
    impl AgentExecutor for CapturingRunner {
        async fn run(
            &self,
            cmd: Vec<String>,
            _cwd: &Path,
            _context: super::super::ExecutionContext,
        ) -> Result<i32> {
            *self.cmd.lock().expect("capture mutex poisoned") = Some(cmd);
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
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.to_string_lossy().to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Running,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
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

        let run = Run {
            id: run_id.clone(),
            wave_id: wave_id.clone(),
            repo: repo.to_string_lossy().to_string(),
            flow: flow_name.to_string(),
            task: None,
            direction: vec![],
            area: vec![],
            iteration: 0,
            step_index: 0,
            status: RunStatus::Running,
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
            stack_status: crate::lfd::types::RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        };
        store
            .create_run(&run)
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
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.to_string_lossy().to_string(),
                worktree: String::new(),
                branch: String::new(),
                status,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: vec![],
            area: vec![],
            status,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
        }
    }

    async fn create_main_run(store: &SharedStore, wave: &Wave, status: RunStatus) -> Run {
        let repo_work = wave
            .repos
            .first()
            .expect("wave always has at least one RepoWork");
        let run = Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: repo_work.repo.clone(),
            flow: wave.primary_flow().clone(),
            task: None,
            direction: wave.direction().clone(),
            area: wave.area().clone(),
            iteration: 0,
            step_index: 0,
            status,
            worktree: repo_work.repo.clone(),
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
            stack_status: crate::lfd::types::RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        };
        store
            .create_run(&run)
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
    async fn execute_wave_run_with_goal_runs_rendered_goal_prompt() {
        let repo = TestRepo::new();
        let goals_dir = repo.path().join(".lf/goals");
        std::fs::create_dir_all(&goals_dir).expect("create goal dir");
        std::fs::write(goals_dir.join("drive.md"), "Execute the custom goal body.")
            .expect("write goal");
        std::fs::create_dir_all(repo.path().join(".lf/flows")).expect("create flows dir");
        std::fs::write(repo.path().join(".lf/flows/custom.yaml"), "- implement\n")
            .expect("write flow");

        let db_path = repo.path().join("lfd.db");
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store"),
        );
        let scheduler = Arc::new(crate::lfd::scheduler::Scheduler::new(1));
        let event_hub = EventHub::new(16);
        let runner = Arc::new(CapturingRunner {
            cmd: Mutex::new(None),
        });
        let executor = WaveExecutor::with_runner(
            store.clone(),
            scheduler,
            OutputHub::new(1024, repo.path().join("output")),
            event_hub,
            runner.clone(),
        );

        let mut wave = make_wave(
            "goal-wave",
            repo.path(),
            "ship-roadmap",
            WaveStatus::Running,
        );
        wave.mode = WaveMode::Manual;
        wave.goal = "drive".to_string();
        store.create_wave(&wave).await.expect("create wave");

        let mut run = create_main_run(&store, &wave, RunStatus::Running).await;
        run.target_branch = "goal-branch".to_string();
        run.flow = "qa".to_string();
        store.update_run(&run).await.expect("update run");

        executor.execute(&run.id).await.expect("execute run");

        let cmd = runner
            .cmd
            .lock()
            .expect("capture mutex poisoned")
            .clone()
            .expect("runner command");
        assert!(cmd.iter().any(|arg| arg == ":"));
        let prompt = cmd.last().expect("inline prompt should be last arg");
        assert!(prompt.contains("Execute the custom goal body."));
        assert!(prompt.contains("- custom"));
        assert!(prompt.contains("wave/goal-wave"));
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
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.path().to_string_lossy().to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
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
            .list_runs(Some(&target_wave.id), None)
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
        let source_run = create_main_run(&store, &source_wave, RunStatus::Running).await;
        let _listener_active_run =
            create_main_run(&store, &listener_wave, RunStatus::Running).await;

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
        let source_run = create_main_run(&store, &source_wave, RunStatus::Running).await;

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
            .list_runs(Some(listener_wave.id()), None)
            .await
            .expect("list listener runs");
        assert!(
            listener_runs.is_empty(),
            "listener should not start immediately when scheduler is full"
        );
    }

    #[tokio::test]
    async fn failed_session_marks_run_failed() {
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
            .get_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        run.status = RunStatus::Waiting;
        store
            .update_run(&run)
            .await
            .expect("run update should succeed");

        let session_id = LfdId::new();
        let session = Session {
            id: session_id.clone(),
            wave_id: wave_id.clone(),
            run_id: Some(run_id.clone()),
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "design".to_string(),
            agent: "claude".to_string(),
            cwd: repo.to_string_lossy().to_string(),
            argv: vec!["claude".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            status: SessionStatus::Failed,
            attached_at: None,
            started_at: None,
            completed_at: Some(OffsetDateTime::now_utc()),
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
            tmux_name: "lf-test".to_string(),
        };
        store
            .create_control_session(&session)
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
            .wait_for_session_and_resume(wave_id.clone(), run_id.clone(), session_id.clone())
            .await
            .expect("resume should succeed");

        let updated_run = store
            .get_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, RunStatus::Failed);
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

        let run = Run {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            repo: worktree.to_string_lossy().to_string(),
            flow: "build".to_string(),
            task: None,
            direction: vec![],
            area: vec![],
            iteration: 0,
            step_index: 0,
            status: RunStatus::Waiting,
            worktree: worktree.to_string_lossy().to_string(),
            branch: "feature-branch".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
            execution_cursor: None,
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "stack".to_string(),
            stack_status: crate::lfd::types::RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        };
        let session = Session {
            id: LfdId::new(),
            wave_id: run.wave_id.clone(),
            run_id: Some(run.id.clone()),
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "review-design".to_string(),
            agent: "claude".to_string(),
            cwd: worktree.to_string_lossy().to_string(),
            argv: vec!["claude".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
            tmux_name: "lf-test-review".to_string(),
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
            step: Step::named("review"),
            flow_parents: vec![],
        };
        let wave_review_details = interactive_attention_details(&wave_review_step, &run);
        let wave_review_context =
            build_interactive_context(&wave_review_step, &session, &wave_review_details);
        assert_eq!(wave_review_context["step"], "review");
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
    async fn completed_session_resolves_interactive_attention() {
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
            .get_run(&run_id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        run.status = RunStatus::Waiting;
        store
            .update_run(&run)
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
        let session = Session {
            id: session_id.clone(),
            wave_id: wave_id.clone(),
            run_id: Some(run_id.clone()),
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "review-design".to_string(),
            agent: "claude".to_string(),
            cwd: repo.path().to_string_lossy().to_string(),
            argv: vec!["claude".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            status: SessionStatus::Succeeded,
            attached_at: None,
            started_at: None,
            completed_at: Some(OffsetDateTime::now_utc()),
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
            tmux_name: "lf-test-resolve".to_string(),
        };
        store
            .create_control_session(&session)
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
                "session_id": session_id,
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
            .wait_for_session_and_resume(wave_id.clone(), run_id.clone(), session_id.clone())
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

    #[test]
    fn classify_repair_flow_returns_debug_for_ci_fix() {
        let mut run = Run::new(LfdId::new(), LfdId::new());
        run.flow = CI_FIX_FLOW.to_string();
        assert_eq!(classify_repair_flow(&run), "debug");
    }

    #[test]
    fn classify_repair_flow_returns_debug_for_regular_flow() {
        let mut run = Run::new(LfdId::new(), LfdId::new());
        run.flow = "build".to_string();
        assert_eq!(classify_repair_flow(&run), "debug");
    }

    #[test]
    fn build_run_command_includes_wave_memory_in_prompt() {
        let repo = tempdir().expect("tempdir");
        let wave_dir = repo.path().join("wave").join("memory-wave");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("GOAL.md"), "Drive the memory wave.\n").expect("write goal");
        std::fs::write(
            wave_dir.join("MEMORY.md"),
            "Last loop found the roadmap gap.\n",
        )
        .expect("write memory");

        let mut wave = make_wave("memory-wave", repo.path(), "build", WaveStatus::Running);
        wave.goal = "memory-wave".to_string();
        let mut run = Run::new(LfdId::new(), wave.id().clone());
        run.worktree = repo.path().to_string_lossy().to_string();

        let (cmd, terminal_step) =
            build_run_command(&wave, &run, Vec::new()).expect("build wave command");
        let rendered = cmd.join("\n");

        assert_eq!(terminal_step, "goal:memory-wave");
        assert!(rendered.contains("Drive the memory wave."));
        assert!(rendered.contains("<lf:wave-memory>"));
        assert!(rendered.contains("Last loop found the roadmap gap."));
        assert!(rendered.contains("<lf:in-flight>\nNo work is in flight."));
    }

    #[test]
    fn build_run_command_includes_in_flight_dispatches() {
        let repo = tempdir().expect("tempdir");
        let wave_dir = repo.path().join("wave").join("memory-wave");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("GOAL.md"), "Drive the memory wave.\n").expect("write goal");

        let mut wave = make_wave("memory-wave", repo.path(), "build", WaveStatus::Running);
        wave.goal = "memory-wave".to_string();
        let mut run = Run::new(LfdId::new(), wave.id().clone());
        run.worktree = repo.path().to_string_lossy().to_string();

        let in_flight = vec![InFlightDispatch {
            task: Some("Add the dispatch endpoint.".to_string()),
            flow: "implement".to_string(),
            status: "running".to_string(),
            pr_url: Some("https://github.com/example/repo/pull/7".to_string()),
            pr_state: Some("open".to_string()),
        }];

        let (cmd, _terminal_step) =
            build_run_command(&wave, &run, in_flight).expect("build wave command");
        let rendered = cmd.join("\n");

        assert!(rendered.contains("<lf:in-flight>"));
        assert!(rendered.contains(
            "- [running] implement: Add the dispatch endpoint. (open https://github.com/example/repo/pull/7)"
        ));
    }

    #[test]
    fn build_run_command_dispatches_task() {
        let repo = tempdir().expect("tempdir");
        let wave = make_wave("dispatch-wave", repo.path(), "build", WaveStatus::Running);
        let mut run = Run::new(LfdId::new(), wave.id().clone());
        run.worktree = repo.path().to_string_lossy().to_string();
        run.flow = "implement".to_string();
        run.task = Some("Add the dispatch endpoint.".to_string());

        let (cmd, terminal_step) =
            build_run_command(&wave, &run, Vec::new()).expect("build dispatch command");

        assert_eq!(terminal_step, "dispatch:implement");
        assert!(cmd.contains(&"implement:".to_string()));
        assert!(cmd.contains(&"-b".to_string()));
        assert!(cmd.contains(&"Add the dispatch endpoint.".to_string()));
    }

    #[test]
    fn build_run_command_errors_when_wave_memory_is_unreadable() {
        let repo = tempdir().expect("tempdir");
        let wave_dir = repo.path().join("wave").join("memory-wave");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(wave_dir.join("GOAL.md"), "Drive the memory wave.\n").expect("write goal");
        std::fs::create_dir(wave_dir.join("MEMORY.md")).expect("create unreadable memory path");

        let mut wave = make_wave("memory-wave", repo.path(), "build", WaveStatus::Running);
        wave.goal = "memory-wave".to_string();
        let mut run = Run::new(LfdId::new(), wave.id().clone());
        run.worktree = repo.path().to_string_lossy().to_string();

        let err = build_run_command(&wave, &run, Vec::new()).expect_err("memory read should fail");
        assert!(err.to_string().contains("failed to read wave memory"));
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
        let failed_run = create_main_run(&store, &wave, RunStatus::Failed).await;

        let repair = executor
            .create_repair_run(&wave, &failed_run, "debug")
            .await
            .unwrap();

        assert_eq!(repair.repair_of.as_ref().unwrap(), &failed_run.id);
        assert_eq!(repair.flow, "debug");
        assert_eq!(repair.worktree, failed_run.worktree);
        assert_eq!(repair.branch, failed_run.branch);
        assert_eq!(repair.status, RunStatus::Running);

        // Verify persisted
        let loaded = store.get_run(&repair.id).await.unwrap().unwrap();
        assert_eq!(loaded.repair_of.unwrap(), failed_run.id);
    }

    #[tokio::test]
    async fn list_in_flight_dispatches_includes_active_and_open_pr_runs_only() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );
        let repo = TestRepo::new();
        let wave = make_wave("in-flight-wave", repo.path(), "build", WaveStatus::Running);
        store.create_wave(&wave).await.unwrap();

        // The run about to launch — excluded from its own in-flight list.
        let launching_run = create_main_run(&store, &wave, RunStatus::Pending).await;

        // Still running — included regardless of PR state.
        let mut running_run = create_main_run(&store, &wave, RunStatus::Running).await;
        running_run.task = Some("Fix the flaky test.".to_string());
        store.update_run(&running_run).await.unwrap();

        // Completed, but the PR is still open — included.
        let mut completed_open_pr = create_main_run(&store, &wave, RunStatus::Completed).await;
        completed_open_pr.pr = Some(PullRequest {
            url: "https://github.com/example/repo/pull/1".to_string(),
            number: Some(1),
            state: Some("open".to_string()),
            title: None,
            branch: Some("feature".to_string()),
        });
        store.update_run(&completed_open_pr).await.unwrap();

        // Completed with a merged PR — excluded.
        let mut completed_merged_pr = create_main_run(&store, &wave, RunStatus::Completed).await;
        completed_merged_pr.pr = Some(PullRequest {
            url: "https://github.com/example/repo/pull/2".to_string(),
            number: Some(2),
            state: Some("merged".to_string()),
            title: None,
            branch: Some("feature-2".to_string()),
        });
        store.update_run(&completed_merged_pr).await.unwrap();

        // Completed with no PR at all — excluded.
        let _completed_no_pr = create_main_run(&store, &wave, RunStatus::Completed).await;

        let in_flight = list_in_flight_dispatches(&store, wave.id(), Some(&launching_run.id))
            .await
            .expect("list in-flight dispatches");

        assert_eq!(in_flight.len(), 2);
        assert!(in_flight
            .iter()
            .any(|dispatch| dispatch.task.as_deref() == Some("Fix the flaky test.")));
        assert!(in_flight
            .iter()
            .any(|dispatch| dispatch.pr_url.as_deref()
                == Some("https://github.com/example/repo/pull/1")));
    }
}
