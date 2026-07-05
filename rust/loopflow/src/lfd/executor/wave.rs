//! The surviving executor surface: palette terminal sessions, boot session
//! reconciliation, and the worktree janitor. Run dispatch left this process
//! — placed `lf` runs launch workers and every flow run registers
//! its own session row; this module only observes and tidies.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use tokio::time::Duration;
use tracing::{info, warn};

use time::OffsetDateTime;

use crate::engine::worktree::remove_worktree;
use crate::lfd::events::EventHub;
use crate::lfd::id::LfdId;
use crate::lfd::types::{
    tmux_session_name, Event, Session, SessionStatus, SessionUse, LIVE_SESSION_STATUSES,
    PALETTE_TERMINAL_SOURCE, WAVE_SERVER_PID_ENV, WAVE_SERVER_SOURCE,
};
use crate::lfdb::SharedStore;

use super::helpers::{
    build_lf_step_command, is_active_run_status, is_ephemeral_worktree_path,
    launch_session_in_tmux, tmux_exit_file, tmux_session_exists, TMUX_EXIT_TAIL,
};
use super::JanitorReport;
use crate::wave::registry::process_alive;

fn tmux_available() -> bool {
    std::process::Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|output| output.status.success())
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

/// Wrapper tail per session kind: palette panes stay open as a shell after
/// the command; everything else propagates the exit code and ends.
fn session_tail(session: &Session) -> &'static str {
    if session.source == PALETTE_TERMINAL_SOURCE {
        r#"exec "${SHELL:-/bin/zsh}""#
    } else {
        TMUX_EXIT_TAIL
    }
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

#[derive(Clone)]
pub struct WaveExecutor {
    store: SharedStore,
    event_hub: EventHub,
}

impl std::fmt::Debug for WaveExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaveExecutor").finish()
    }
}

impl WaveExecutor {
    pub fn new(store: SharedStore, event_hub: EventHub) -> Self {
        Self { store, event_hub }
    }

    pub async fn launch_palette_session(
        &self,
        wave_id: &LfdId,
        flow: &str,
        worktree: &str,
        agent: &str,
    ) -> Result<Session> {
        if !tmux_available() {
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

    pub async fn reconcile_sessions(&self) -> Result<u32> {
        let active = self
            .store
            .list_control_sessions(None, Some(LIVE_SESSION_STATUSES))
            .await?;
        let mut completed = 0;
        for session in active {
            if session.source == WAVE_SERVER_SOURCE {
                // A registered `lf wave` server. lfd never launched it, so
                // liveness is the recorded pid (same host — the endpoint is
                // loopback). A dead pid is a server that crashed without
                // deregistering: close the row so one-brain enforcement
                // doesn't key on a ghost.
                let alive = match session
                    .env
                    .get(WAVE_SERVER_PID_ENV)
                    .and_then(|pid| pid.parse::<u32>().ok())
                {
                    Some(pid) => process_alive(pid).await,
                    None => false,
                };
                if !alive && self.complete_session(&session.id, 1).await? {
                    info!(session_id = %session.id, "reconciled crashed wave server session");
                    completed += 1;
                }
                continue;
            }
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

    async fn launch_tmux_session(&self, session: Session) -> Result<Session> {
        // The wrapper (exit-file contract, inherited-marker unset) has one
        // authoring site: helpers::tmux_shell_command. Only the tail is the
        // executor's choice.
        launch_session_in_tmux(&session, session_tail(&session)).await?;

        let mut running = session;
        let _ = running.start();
        self.store.update_control_session(&running).await?;
        self.event_hub.send(Event::session_updated(running.clone()));
        Ok(running)
    }

    fn spawn_palette_completion_watcher(&self, session: Session) {
        let executor = self.clone();
        tokio::spawn(async move {
            if let Err(err) = executor.wait_for_palette_session_completion(&session).await {
                warn!(session_id = %session.id, error = %err, "palette terminal completion watcher failed");
            }
        });
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::types::TMUX_TERMINAL_SOURCE;

    /// The executor's tmux launch rides the shared wrapper: exit-code tail
    /// for run sessions, shell tail for palette panes, and the inherited
    /// marker cleared in both — a fresh tmux server inherits the
    /// dispatcher's env, and a leaked marker makes workers double-register.
    #[test]
    fn executor_tmux_wrapper_unsets_inherited_marker_for_both_tails() {
        let mut session = Session {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "dispatch:implement".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.wave".to_string(),
            argv: vec!["lf".to_string(), "implement".to_string()],
            env: Default::default(),
            source: TMUX_TERMINAL_SOURCE.to_string(),
            tmux_name: "lf-test".to_string(),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        };

        let wrapper = super::super::helpers::tmux_shell_command(&session, session_tail(&session));
        assert!(wrapper.starts_with("unset LFD_SESSION_INHERITED; "));
        assert!(wrapper.ends_with(TMUX_EXIT_TAIL));

        session.source = PALETTE_TERMINAL_SOURCE.to_string();
        let wrapper = super::super::helpers::tmux_shell_command(&session, session_tail(&session));
        assert!(wrapper.starts_with("unset LFD_SESSION_INHERITED; "));
        assert!(wrapper.ends_with(r#"exec "${SHELL:-/bin/zsh}""#));
    }
}
