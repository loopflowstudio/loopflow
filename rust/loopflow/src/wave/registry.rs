//! The wave server's seat in the shared session registry — store-direct.
//!
//! `lf wave <name>` is its own process; no daemon launches or supervises it.
//! The shared local store (the same SQLite db lfd serves from — the db IS
//! the registry) carries two facts this module owns:
//!
//! - **Registration.** On boot the server ensures the wave itself has a row
//!   ([`ensure_wave_row`] — a reachable store with no row for the wave gets
//!   a minimal one, never an unregistered run), then writes itself a
//!   `WaveAgent` session row (source `wave_server`, endpoint + pid in `env`)
//!   so Concerto's agent tree shows the mind and one-brain enforcement has a
//!   fact to key on. Before writing, [`register`] probes the wave's live
//!   WaveAgent rows: a `wave_server` row whose pid is dead is a crashed
//!   server — closed on the spot; a surviving live brain is a refusal naming
//!   it, unless `force` takes over (kill the recorded pid / tmux session,
//!   cancel the row). Graceful shutdown and Ctrl-C mark the row terminal;
//!   lfd's session reconciliation (pid probe on `wave_server` rows) covers a
//!   crash, and so does the boot-time probe of the next `lf wave`.
//!
//! - **Observation.** [`StoreObserver`] polls the store — this wave's worker
//!   sessions and runs — and journals confirmed worker facts:
//!   `RunObserved` when a worker session+run appears, `RunCompleted`
//!   when the session goes terminal (summary = what the registry knows:
//!   session exit status, run error, PR url). These are OBSERVATIONS — the
//!   server is never in the dispatch path. The runtime's run-id guard keeps
//!   every fact journaled exactly once, however polls and restarts overlap.
//!
//! **The no-store story, honestly:** a machine without the registry db gets
//! no registration, no one-brain enforcement, and no worker observations —
//! the wave server is fully functional anyway, and says so once.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::process::Command;

use crate::engine::wave_config::read_wave_config;
use crate::lfd::id::LfdId;
use crate::lfd::types::{
    Run, RunStatus, Session, SessionStatus, SessionUse, Wave, WaveStatus, LIVE_SESSION_STATUSES,
    WAVE_SERVER_ENDPOINT_ENV, WAVE_SERVER_PID_ENV, WAVE_SERVER_SOURCE,
};
use crate::lfdb::{SharedStore, StoreResult};
use crate::wave::journal::WorkerOutcome;
use crate::wave::runtime::WaveRuntime;

fn is_active_run_status(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Pending | RunStatus::Running | RunStatus::Waiting
    )
}

async fn tmux_session_exists(session_name: &str) -> anyhow::Result<bool> {
    let status = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .await
        .map_err(|err| anyhow::anyhow!("tmux session probe failed: {err}"))?;
    Ok(status.success())
}

/// How often the observer re-reads the store between turns. Modest by
/// design: the mind also refreshes right before every turn it takes.
pub const POLL_CADENCE: Duration = Duration::from_secs(10);

/// Everything registration needs: the opened store, the wave's row, and this
/// server's identity.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub store: SharedStore,
    pub wave: Wave,
    /// The worktree the server (and its mind) runs in.
    pub cwd: String,
    pub pid: u32,
    /// Take over from an existing live wave-agent session.
    pub force: bool,
}

/// One registration attempt's outcome. Store errors surface as `Err` — the
/// caller treats them as soft (run unregistered), like a missing registry.
/// The registration is boxed only for enum-size hygiene (it carries the row).
#[derive(Debug)]
pub enum RegisterOutcome {
    Registered(Box<Registration>),
    /// Another live wave-agent session exists. The message names it.
    Refused {
        message: String,
    },
}

/// The server's live session row. Clone freely; whichever holder deregisters
/// first wins (the rest are no-ops).
#[derive(Debug, Clone)]
pub struct Registration {
    store: SharedStore,
    session: Session,
    done: Arc<AtomicBool>,
}

impl Registration {
    pub fn session_id(&self) -> &LfdId {
        &self.session.id
    }

    /// Mark the row terminal (exit 0), exactly once, best-effort.
    pub async fn deregister(&self) {
        if self.done.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut session = self.session.clone();
        if !session.complete(0) {
            return;
        }
        if let Err(err) = self.store.update_control_session(&session).await {
            tracing::warn!(error = %err, "wave server deregistration failed; the next boot's pid probe will close the row");
        }
    }

    /// [`Registration::deregister`] for synchronous contexts — the Ctrl-C
    /// interrupt hook runs on a plain thread.
    pub fn deregister_blocking(&self) {
        let registration = self.clone();
        block_on(async move { registration.deregister().await });
    }
}

/// The wave's registry row, created if the store has never seen this wave.
///
/// The db IS the registry: a reachable store with no row for the wave must
/// not degrade to running unregistered (observed live — two brains on one
/// wave because boot skipped registration entirely). The created row is
/// minimal and mirrors wave creation from GOAL.md frontmatter
/// ([`read_wave_config`]): goal/primary-flow from the frontmatter when
/// present, [`Wave::new`] defaults otherwise.
///
/// # Errors
/// Store failures only; the caller treats them as soft (run unregistered).
pub async fn ensure_wave_row(
    store: &SharedStore,
    main_repo: &Path,
    name: &str,
) -> StoreResult<Wave> {
    if let Some(wave) = store.get_wave_by_name(name).await? {
        return Ok(wave);
    }
    let mut wave = Wave::new(
        LfdId::new(),
        name.to_string(),
        main_repo.display().to_string(),
    );
    if let Some(config) = read_wave_config(main_repo, name) {
        if let Some(flow) = config.primary_flow {
            wave.primary_flow = flow;
        }
        if let Some(goal) = config.goal.filter(|goal| !goal.trim().is_empty()) {
            wave.goal = goal;
        }
    }
    // Not born paused: the row exists so the SERVER can register as the
    // wave's brain, not to silence a daemon (the ticker that read this column
    // died in the collapse). The safety valve now lives in GOAL.md
    // frontmatter, read file-first by the listener — a registry row born
    // `paused: true` was a lie that only confused Concerto.
    wave.paused = read_wave_config(main_repo, name)
        .and_then(|config| config.paused)
        .unwrap_or(false);
    store.create_wave(&wave).await?;
    tracing::info!(
        wave = name,
        wave_id = %wave.id,
        "wave was not in the session registry; created its row"
    );
    Ok(wave)
}

/// Register this server as the wave's one brain.
///
/// Probes the wave's live WaveAgent sessions first: crashed `wave_server`
/// rows (dead pid) are closed; a surviving live brain refuses the start
/// unless `force` takes over — kill by the recorded pid when it's alive (or
/// the tmux session for an lfd-launched brain), cancel the row, proceed.
///
/// # Errors
/// Store failures only; a refusal is a value, not an error.
pub async fn register(config: &RegistryConfig, endpoint: &str) -> StoreResult<RegisterOutcome> {
    let wave_id = config.wave.id();
    if let Some(mut existing) = live_brain_after_probe(&config.store, wave_id).await? {
        if !config.force {
            return Ok(RegisterOutcome::Refused {
                message: format!(
                    "wave '{}' already has a live wave-agent session {} (source '{}')",
                    config.wave.name(),
                    existing.id,
                    existing.source
                ),
            });
        }
        // Force takeover: this server is the brain now.
        if let Some(pid) = wave_server_pid(&existing) {
            if process_alive(pid).await {
                let _ = Command::new("kill").arg(pid.to_string()).status().await;
            }
        } else if existing.is_tmux_backed() && !existing.tmux_name.is_empty() {
            let _ = Command::new("tmux")
                .args(["kill-session", "-t", &existing.tmux_name])
                .status()
                .await;
        }
        if existing.cancel() {
            config.store.update_control_session(&existing).await?;
        }
    }

    let session_id = LfdId::new();
    let now = OffsetDateTime::now_utc();
    let session = Session {
        id: session_id,
        wave_id: wave_id.clone(),
        run_id: None,
        parent_session_id: None,
        session_use: SessionUse::WaveAgent,
        step: "mind".to_string(),
        agent: "lf".to_string(),
        cwd: config.cwd.clone(),
        argv: vec![
            "lf".to_string(),
            "wave".to_string(),
            config.wave.name().clone(),
        ],
        env: std::collections::BTreeMap::from([
            (WAVE_SERVER_ENDPOINT_ENV.to_string(), endpoint.to_string()),
            (WAVE_SERVER_PID_ENV.to_string(), config.pid.to_string()),
        ]),
        source: WAVE_SERVER_SOURCE.to_string(),
        tmux_name: String::new(),
        status: SessionStatus::Running,
        attached_at: None,
        started_at: Some(now),
        completed_at: None,
        created_at: now,
        completion_token: None,
    };
    config.store.register_session(&session).await?;
    Ok(RegisterOutcome::Registered(Box::new(Registration {
        store: config.store.clone(),
        session,
        done: Arc::new(AtomicBool::new(false)),
    })))
}

/// The wave's live brain after pid-probing self-registered servers: a
/// `wave_server` row whose recorded pid is dead is a server that crashed
/// without deregistering — closed here so one-brain enforcement never keys
/// on a ghost. lfd-launched WaveAgent sessions (tmux-backed) are the
/// executor's to supervise and count as live.
pub async fn live_brain_after_probe(
    store: &SharedStore,
    wave_id: &LfdId,
) -> StoreResult<Option<Session>> {
    let sessions = store
        .list_control_sessions(Some(wave_id), Some(LIVE_SESSION_STATUSES))
        .await?;
    let mut live = None;
    for mut session in sessions {
        if session.session_use != SessionUse::WaveAgent {
            continue;
        }
        if session.source == WAVE_SERVER_SOURCE {
            let alive = match wave_server_pid(&session) {
                Some(pid) => process_alive(pid).await,
                None => false,
            };
            if !alive {
                if session.complete(1) {
                    store.update_control_session(&session).await?;
                    tracing::info!(session_id = %session.id, "closed crashed wave server session");
                }
                continue;
            }
        }
        if live.is_none() {
            live = Some(session);
        }
    }
    Ok(live)
}

fn wave_server_pid(session: &Session) -> Option<u32> {
    if session.source != WAVE_SERVER_SOURCE {
        return None;
    }
    session
        .env
        .get(WAVE_SERVER_PID_ENV)
        .and_then(|pid| pid.parse().ok())
}

/// The endpoint a wave's live server listens on, off the live WaveAgent
/// session row's env (trimmed, empty dropped). Shared by `lf chat`'s target
/// resolution and the work-line channel knock; callers fall back to the
/// `wave/<name>/.wave-endpoint` discovery file when the store has no live row.
pub(crate) async fn wave_server_endpoint(
    store: &SharedStore,
    wave_id: &LfdId,
) -> anyhow::Result<Option<String>> {
    let Some(session) = store.live_wave_agent_session(wave_id).await? else {
        return Ok(None);
    };
    Ok(session
        .env
        .get(WAVE_SERVER_ENDPOINT_ENV)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

/// Whether a process with `pid` is running on this host (`kill -0` probe).
/// Shared with the supervisor's attached-resident probe.
pub(crate) async fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .await
        .is_ok_and(|status| status.success())
}

/// Run a registry future from a synchronous context (the Ctrl-C hook's
/// plain thread); hops to a fresh thread if a runtime is already current.
fn block_on(future: impl Future<Output = ()> + Send + 'static) {
    if tokio::runtime::Handle::try_current().is_ok() {
        let _ = std::thread::spawn(move || block_on_new_runtime(future)).join();
        return;
    }
    block_on_new_runtime(future);
}

fn block_on_new_runtime(future: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime always builds")
        .block_on(future);
}

// -- Observation ---------------------------------------------------------

/// Polls the store for this wave's worker facts and journals them.
/// Idempotent end to end: the runtime's run-id guard makes overlapping
/// polls, restarts, and the pre-turn refresh journal each fact exactly once.
///
/// The observer is also the daemonless liveness authority for workers: a
/// SIGKILL'd worker never closes its own Running row and the exit-file
/// janitor only runs at lfd boot, so the poll probes live tmux-backed
/// worker rows (the same way brains are pid-probed) and closes dead ones —
/// otherwise `in_flight` and wave capacity stay poisoned forever.
pub struct StoreObserver {
    runtime: Arc<WaveRuntime>,
    store: SharedStore,
    wave_id: LfdId,
    /// Unix-seconds cutoff for the recently-terminal session query; `None`
    /// until the first (full-history, catch-up) poll succeeds.
    terminal_cutoff: std::sync::Mutex<Option<i64>>,
    /// Run ids already journaled as dispatched — a local snapshot so
    /// steady-state polls don't take the runtime lock per session.
    seen: std::sync::Mutex<HashSet<String>>,
    #[cfg(test)]
    liveness_override: std::sync::Mutex<Option<LivenessProbe>>,
}

/// Test seam: fabricated worker rows have no real tmux session to probe.
#[cfg(test)]
type LivenessProbe = Box<dyn Fn(&Session) -> bool + Send + Sync>;

impl fmt::Debug for StoreObserver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoreObserver")
            .field("wave_id", &self.wave_id)
            .finish()
    }
}

/// Margin subtracted from each poll's start when it becomes the next poll's
/// terminal cutoff: clock jitter between writers can never hide a finish,
/// and re-reading a row twice is free (journaling is idempotent).
const TERMINAL_CUTOFF_MARGIN: time::Duration = time::Duration::seconds(60);

impl StoreObserver {
    pub fn new(runtime: Arc<WaveRuntime>, store: SharedStore, wave_id: LfdId) -> Self {
        Self {
            runtime,
            store,
            wave_id,
            terminal_cutoff: std::sync::Mutex::new(None),
            seen: std::sync::Mutex::new(HashSet::new()),
            #[cfg(test)]
            liveness_override: std::sync::Mutex::new(None),
        }
    }

    /// Poll forever on `cadence`. Runs until aborted at server shutdown.
    pub async fn run(self: Arc<Self>, cadence: Duration) {
        loop {
            self.poll_once().await;
            tokio::time::sleep(cadence).await;
        }
    }

    /// Whether a `RunObserved` is already journaled for `run_id`,
    /// via the local seen-cache first (the runtime lock is taken at most
    /// once per run id over the observer's lifetime).
    fn known(&self, run_id: &str) -> bool {
        if self
            .seen
            .lock()
            .expect("observer seen cache poisoned")
            .contains(run_id)
        {
            return true;
        }
        if self.runtime.worker_known(run_id) {
            self.mark_seen(run_id);
            return true;
        }
        false
    }

    fn mark_seen(&self, run_id: &str) {
        self.seen
            .lock()
            .expect("observer seen cache poisoned")
            .insert(run_id.to_string());
    }

    async fn worker_alive(&self, session: &Session) -> bool {
        #[cfg(test)]
        if let Some(probe) = self
            .liveness_override
            .lock()
            .expect("observer probe override poisoned")
            .as_ref()
        {
            return probe(session);
        }
        tmux_session_exists(&session.tmux_name)
            .await
            .unwrap_or(false)
    }

    #[cfg(test)]
    fn set_liveness_probe(&self, probe: impl Fn(&Session) -> bool + Send + Sync + 'static) {
        *self
            .liveness_override
            .lock()
            .expect("observer probe override poisoned") = Some(Box::new(probe));
    }

    /// One reconciliation pass: read this wave's worker sessions (live plus
    /// recently-terminal — the first poll scans full history to catch up on
    /// anything that happened with no server watching), close dead workers,
    /// journal new dispatches and fresh finishes. Store errors are logged
    /// and skipped — the next poll retries.
    pub async fn poll_once(&self) {
        // Sweep dead child channels on the poll cadence: a landed/deleted
        // work line's channel pins its journal file handle until dropped.
        self.runtime.sweep_dead_children();
        let poll_started = OffsetDateTime::now_utc();
        let cutoff = *self
            .terminal_cutoff
            .lock()
            .expect("observer cutoff poisoned");
        let sessions = match cutoff {
            None => {
                self.store
                    .list_control_sessions(Some(&self.wave_id), None)
                    .await
            }
            Some(since) => {
                self.store
                    .list_recent_control_sessions(&self.wave_id, since)
                    .await
            }
        };
        let sessions = match sessions {
            Ok(sessions) => sessions,
            Err(err) => {
                tracing::debug!(error = %err, "wave observer session read failed");
                return;
            }
        };
        // Advance the cutoff only after a successful read.
        *self
            .terminal_cutoff
            .lock()
            .expect("observer cutoff poisoned") =
            Some((poll_started - TERMINAL_CUTOFF_MARGIN).unix_timestamp());

        let mut workers: Vec<(Session, String)> = sessions
            .into_iter()
            .filter(|session| session.session_use == SessionUse::Worker)
            .filter_map(|session| {
                let run_id = session.run_id.as_ref().map(ToString::to_string)?;
                Some((session, run_id))
            })
            .collect();

        // Liveness: a Running tmux-backed worker whose tmux session is gone
        // died without closing its row (SIGKILL, crash). Close it here —
        // at most one probe per session per poll — so capacity frees up and
        // the finish is journaled below as `process gone`.
        let mut gone: HashSet<String> = HashSet::new();
        for (session, run_id) in workers.iter_mut() {
            if session.status != SessionStatus::Running
                || !session.is_tmux_backed()
                || session.tmux_name.is_empty()
                || self.worker_alive(session).await
            {
                continue;
            }
            if session.complete(1) {
                if let Err(err) = self.store.update_control_session(session).await {
                    tracing::debug!(
                        error = %err,
                        session_id = %session.id,
                        "failed to close dead worker session; will retry next poll"
                    );
                    continue;
                }
                tracing::info!(
                    session_id = %session.id,
                    run_id,
                    "closed dead worker session (process gone)"
                );
            }
            gone.insert(run_id.clone());
        }

        // Runs carry the flow/task for a dispatch and the PR/error for a
        // finish summary; fetch them only when this pass will journal.
        let in_flight: HashSet<String> = self
            .runtime
            .in_flight_workers()
            .into_iter()
            .map(|worker| worker.run_id)
            .collect();
        let needs_runs = workers.iter().any(|(session, run_id)| {
            !self.known(run_id) || (session.status.is_terminal() && in_flight.contains(run_id))
        });
        if !needs_runs {
            return;
        }
        let runs: HashMap<String, Run> = match self.store.list_runs(Some(&self.wave_id), None).await
        {
            Ok(runs) => runs
                .into_iter()
                .map(|run| (run.id.to_string(), run))
                .collect(),
            Err(err) => {
                tracing::debug!(error = %err, "wave observer run read failed");
                return;
            }
        };

        for (session, run_id) in workers {
            if !self.known(&run_id) {
                let Some(run) = runs.get(&run_id) else {
                    tracing::warn!(run_id, "observed worker session with no run row; skipped");
                    continue;
                };
                if self.runtime.journal_run_observed(
                    &run_id,
                    session.id.as_str(),
                    &run.flow,
                    run.task.as_deref().unwrap_or(""),
                ) {
                    tracing::info!(
                        run_id,
                        session_id = %session.id,
                        flow = run.flow,
                        "worker dispatched"
                    );
                }
                self.mark_seen(&run_id);
            }
            if !session.status.is_terminal() {
                continue;
            }
            let outcome = if session.status == SessionStatus::Succeeded {
                WorkerOutcome::Completed
            } else {
                WorkerOutcome::Failed
            };
            let summary = if gone.contains(&run_id) {
                "process gone".to_string()
            } else {
                let mut summary = format!("session {}", session.status.as_str());
                if let Some(run) = runs.get(&run_id) {
                    if let Some(pr) = &run.pr {
                        summary.push_str(&format!("; pr {}", pr.url));
                    }
                    if let Some(error) = &run.error {
                        summary.push_str(&format!("; error: {error}"));
                    }
                }
                summary
            };
            if self
                .runtime
                .journal_run_completed(&run_id, outcome, &summary)
            {
                tracing::info!(run_id, outcome = outcome.name(), summary, "worker finished");
            }
            // Close the loop the trinity leaves open: a terminal worker
            // SESSION whose Run row is still active keeps the run — and the
            // wave — counted Running forever, starving capacity. Mark the run
            // row terminal here (the observer is the daemonless authority),
            // then reset the wave's repo status if no active runs remain.
            self.close_run_and_maybe_idle_wave(&runs, &run_id, outcome)
                .await;
        }
    }

    /// Mark a finished worker's Run row terminal and, when the wave has no
    /// active runs left, reset its repo status `Running → Idle`. Idempotent:
    /// an already-terminal run row is skipped, and the wave reset is a no-op
    /// once the status is settled. Store errors are logged and retried next
    /// poll — never fatal.
    async fn close_run_and_maybe_idle_wave(
        &self,
        runs: &HashMap<String, Run>,
        run_id: &str,
        outcome: WorkerOutcome,
    ) {
        let Some(run) = runs.get(run_id) else {
            return;
        };
        if is_active_run_status(run.status) {
            let mut run = run.clone();
            run.status = match outcome {
                WorkerOutcome::Completed => RunStatus::Completed,
                WorkerOutcome::Failed => RunStatus::Failed,
            };
            run.ended_at = Some(OffsetDateTime::now_utc());
            if let Err(err) = self.store.update_run(&run).await {
                tracing::debug!(run_id, error = %err, "failed to close run row; retry next poll");
                return;
            }
        }
        // No active runs → the wave is idle again. Reset it if still stamped
        // Running (the status create_run_for_placement set on dispatch); leave
        // Paused and already-Idle waves alone.
        match self.store.count_active_runs(&self.wave_id).await {
            Ok(0) => self.reset_wave_to_idle().await,
            Ok(_) => {}
            Err(err) => {
                tracing::debug!(error = %err, "active-run count failed; wave status unchanged");
            }
        }
    }

    async fn reset_wave_to_idle(&self) {
        let Ok(Some(mut wave)) = self.store.get_wave(&self.wave_id).await else {
            return;
        };
        if wave.status != WaveStatus::Running {
            return;
        }
        wave.status = WaveStatus::Idle;
        if let Err(err) = self.store.update_wave(&wave).await {
            tracing::debug!(wave_id = %self.wave_id, error = %err, "failed to reset wave status to idle");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::lfd::types::{
        PullRequest, RunStackStatus, RunStatus, WaveStatus, TMUX_TERMINAL_SOURCE,
    };
    use crate::lfdb::{open_store, StorageConfig};
    use crate::wave::journal::{journal_path, EventKind, Journal};

    async fn temp_store(tmp: &std::path::Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(tmp.join("lfd.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    fn make_wave(name: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: "/tmp/repo".to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 2,
            parent_wave_id: None,
        }
    }

    fn registry_config(store: SharedStore, wave: Wave, force: bool) -> RegistryConfig {
        RegistryConfig {
            store,
            wave,
            cwd: "/tmp/repo.ship".to_string(),
            pid: std::process::id(),
            force,
        }
    }

    /// A wave_server WaveAgent row as a previous `lf wave` would have left it.
    fn server_session(wave: &Wave, pid: u32) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            step: "mind".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.ship".to_string(),
            argv: vec!["lf".to_string(), "wave".to_string(), wave.name().clone()],
            env: BTreeMap::from([
                (
                    WAVE_SERVER_ENDPOINT_ENV.to_string(),
                    "127.0.0.1:9".to_string(),
                ),
                (WAVE_SERVER_PID_ENV.to_string(), pid.to_string()),
            ]),
            source: WAVE_SERVER_SOURCE.to_string(),
            tmux_name: String::new(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    fn worker_session(wave: &Wave, run_id: &LfdId) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: Some(run_id.clone()),
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "dispatch:implement".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.ship".to_string(),
            argv: Vec::new(),
            env: BTreeMap::new(),
            source: TMUX_TERMINAL_SOURCE.to_string(),
            tmux_name: "lf-x".to_string(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    fn make_run(wave: &Wave, flow: &str, task: &str) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: "/tmp/repo".to_string(),
            flow: flow.to_string(),
            task: Some(task.to_string()),
            direction: Vec::new(),
            area: Vec::new(),
            iteration: 0,
            step_index: 0,
            status: RunStatus::Running,
            worktree: "/tmp/repo.ship".to_string(),
            branch: "ship-branch".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id().to_string(),
            stack_status: RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        }
    }

    /// Boot on a wave the store has never seen: the row is created (the db
    /// IS the registry — no warn-and-run-unregistered), the server registers,
    /// and a second boot is refused off the same row.
    #[tokio::test]
    async fn boot_with_no_wave_row_creates_it_and_registers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = tmp.path().join("repo");
        let goal_dir = repo.join("wave/ship");
        std::fs::create_dir_all(&goal_dir).expect("wave dir");
        std::fs::write(
            goal_dir.join("GOAL.md"),
            "---\ngoal: keep shipping\n---\nShip.\n",
        )
        .expect("GOAL.md");

        let wave = ensure_wave_row(&store, &repo, "ship")
            .await
            .expect("row created");
        let stored = store
            .get_wave_by_name("ship")
            .await
            .expect("lookup")
            .expect("row exists");
        assert_eq!(stored.id, wave.id);
        assert_eq!(stored.goal, "keep shipping", "goal from GOAL.md");
        assert_eq!(stored.repo(), repo.display().to_string());

        // Registered against the created row; one-brain now has its fact.
        let config = registry_config(store.clone(), wave.clone(), false);
        let RegisterOutcome::Registered(_reg) =
            register(&config, "127.0.0.1:4").await.expect("register")
        else {
            panic!("first boot registers");
        };

        // Second boot: same row (no duplicate), and the live brain refuses it.
        let again = ensure_wave_row(&store, &repo, "ship")
            .await
            .expect("idempotent");
        assert_eq!(again.id, wave.id, "ensure reuses the existing row");
        let outcome = register(&registry_config(store, again, false), "127.0.0.1:5")
            .await
            .expect("attempt");
        assert!(
            matches!(outcome, RegisterOutcome::Refused { .. }),
            "second boot refused"
        );
    }

    /// No GOAL.md at all: the created row falls back to `Wave::new` defaults.
    #[tokio::test]
    async fn ensure_wave_row_without_goal_md_uses_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = ensure_wave_row(&store, tmp.path(), "ship")
            .await
            .expect("row created");
        assert_eq!(wave.goal, "ship-roadmap");
        assert_eq!(wave.primary_flow, "ship-roadmap");
    }

    #[tokio::test]
    async fn register_writes_wave_server_row_and_deregisters_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");

        let config = registry_config(store.clone(), wave.clone(), false);
        let RegisterOutcome::Registered(registration) =
            register(&config, "127.0.0.1:4242").await.expect("register")
        else {
            panic!("fresh wave must register");
        };

        let stored = store
            .get_control_session(registration.session_id())
            .await
            .expect("lookup")
            .expect("row stored");
        assert_eq!(stored.session_use, SessionUse::WaveAgent);
        assert_eq!(stored.source, WAVE_SERVER_SOURCE);
        assert_eq!(stored.status, SessionStatus::Running);
        assert_eq!(
            stored.env.get(WAVE_SERVER_ENDPOINT_ENV).map(String::as_str),
            Some("127.0.0.1:4242")
        );
        assert_eq!(
            stored.env.get(WAVE_SERVER_PID_ENV).map(String::as_str),
            Some(config.pid.to_string().as_str())
        );
        assert_eq!(stored.cwd, "/tmp/repo.ship");

        // One fact drives one-brain everywhere.
        let live = store
            .live_wave_agent_session(wave.id())
            .await
            .expect("live lookup")
            .expect("brain live");
        assert_eq!(live.id, *registration.session_id());

        registration.deregister().await;
        registration.deregister().await; // second is a no-op
        let stored = store
            .get_control_session(registration.session_id())
            .await
            .expect("lookup")
            .expect("row stored");
        assert_eq!(stored.status, SessionStatus::Succeeded);
    }

    #[tokio::test]
    async fn second_brain_is_refused_naming_the_live_session() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");
        // A live server: its recorded pid is this test process — alive.
        let live = server_session(&wave, std::process::id());
        store
            .register_session(&live)
            .await
            .expect("seed live brain");

        let config = registry_config(store.clone(), wave, false);
        let outcome = register(&config, "127.0.0.1:1").await.expect("attempt");
        let RegisterOutcome::Refused { message } = outcome else {
            panic!("second brain must be refused");
        };
        assert!(
            message.contains(live.id.as_str()),
            "refusal names the live session: {message}"
        );
    }

    #[tokio::test]
    async fn dead_pid_row_is_reconciled_and_takeover_needs_no_force() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");
        // A crashed server: the recorded pid is long dead.
        let crashed = server_session(&wave, 4_000_000);
        store
            .register_session(&crashed)
            .await
            .expect("seed crashed brain");

        let config = registry_config(store.clone(), wave.clone(), false);
        let RegisterOutcome::Registered(registration) =
            register(&config, "127.0.0.1:2").await.expect("register")
        else {
            panic!("dead brain must not block a new server");
        };

        // The ghost row was closed, the new row is the one live brain.
        let old = store
            .get_control_session(&crashed.id)
            .await
            .expect("lookup")
            .expect("row kept");
        assert_eq!(old.status, SessionStatus::Failed);
        let live = store
            .live_wave_agent_session(wave.id())
            .await
            .expect("live lookup")
            .expect("new brain live");
        assert_eq!(live.id, *registration.session_id());
    }

    #[tokio::test]
    async fn force_takes_over_a_live_brain_by_killing_its_pid() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");
        // A genuinely live "server": a sleep child this test owns.
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleep");
        let live = server_session(&wave, child.id());
        store
            .register_session(&live)
            .await
            .expect("seed live brain");

        let config = registry_config(store.clone(), wave.clone(), true);
        let RegisterOutcome::Registered(registration) =
            register(&config, "127.0.0.1:3").await.expect("register")
        else {
            panic!("--force must take over");
        };

        let old = store
            .get_control_session(&live.id)
            .await
            .expect("lookup")
            .expect("row kept");
        assert_eq!(old.status, SessionStatus::Canceled);
        let now_live = store
            .live_wave_agent_session(wave.id())
            .await
            .expect("live lookup")
            .expect("new brain live");
        assert_eq!(now_live.id, *registration.session_id());
        // The old process got the SIGTERM.
        let status = child.wait().expect("child reaped");
        assert!(!status.success(), "sleep was killed, not completed");
    }

    /// The observation fold, across poll gaps: rows created between polls
    /// are picked up; nothing journals twice however often polls repeat.
    #[tokio::test]
    async fn observer_journals_worker_facts_exactly_once_across_polls() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");
        let runtime =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = StoreObserver::new(runtime.clone(), store.clone(), wave.id().clone());
        // This test's workers are fabricated rows, not real tmux sessions;
        // pin them alive so the liveness probe stays out of the way.
        observer.set_liveness_probe(|_| true);

        // Poll 1: one running worker → one dispatch.
        let run_1 = make_run(&wave, "implement", "wire the tail");
        store.create_run(&run_1).await.expect("run-1");
        let mut sess_1 = worker_session(&wave, &run_1.id);
        store.register_session(&sess_1).await.expect("sess-1");
        observer.poll_once().await;
        observer.poll_once().await; // repeat poll: guard holds
        assert!(runtime.worker_known(run_1.id.as_str()));
        assert_eq!(runtime.in_flight_workers().len(), 1);

        // Between polls: a second dispatch appears, and run-1 finishes with
        // a PR — the terminal fact and the late-run fact land in one gap.
        let run_2 = make_run(&wave, "design", "sketch the next item");
        store.create_run(&run_2).await.expect("run-2");
        store
            .register_session(&worker_session(&wave, &run_2.id))
            .await
            .expect("sess-2");
        let mut finished_run = run_1.clone();
        finished_run.pr = Some(PullRequest {
            url: "https://github.com/x/y/pull/7".to_string(),
            number: Some(7),
            state: Some("open".to_string()),
            title: None,
            branch: None,
        });
        store.update_run(&finished_run).await.expect("run-1 pr");
        assert!(sess_1.complete(0));
        store
            .update_control_session(&sess_1)
            .await
            .expect("sess-1 terminal");

        observer.poll_once().await;
        observer.poll_once().await; // and once more

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let dispatched: Vec<String> = events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::RunObserved {
                    run_id, flow, task, ..
                } => Some(format!("{run_id}/{flow}/{task}")),
                _ => None,
            })
            .collect();
        let mut sorted = dispatched.clone();
        sorted.sort();
        let mut expected = vec![
            format!("{}/implement/wire the tail", run_1.id),
            format!("{}/design/sketch the next item", run_2.id),
        ];
        expected.sort();
        assert_eq!(sorted, expected, "each dispatch journaled exactly once");

        let finished: Vec<(String, String)> = events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::RunCompleted {
                    run_id, summary, ..
                } => Some((run_id.clone(), summary.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1, "one finish, once");
        assert_eq!(finished[0].0, run_1.id.to_string());
        assert!(
            finished[0].1.contains("succeeded")
                && finished[0].1.contains("https://github.com/x/y/pull/7"),
            "summary carries what the registry knows: {}",
            finished[0].1
        );

        // A restarted server (fresh runtime over the same journal) keeps the
        // guard: nothing re-journals.
        let runtime_2 =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("reopen runtime");
        let observer_2 = StoreObserver::new(runtime_2, store, wave.id().clone());
        observer_2.set_liveness_probe(|_| true);
        observer_2.poll_once().await;
        let (_, events_after) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        assert_eq!(
            events.len(),
            events_after.len(),
            "restart journals nothing new"
        );
    }

    /// Daemonless liveness: a SIGKILL'd worker leaves a Running row and no
    /// process. The observer's probe must close the row (Failed) and journal
    /// the finish as `process gone`, freeing in_flight and wave capacity —
    /// and stay idempotent across repeated polls.
    #[tokio::test]
    async fn observer_closes_dead_worker_rows_and_journals_process_gone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");
        let runtime =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = StoreObserver::new(runtime.clone(), store.clone(), wave.id().clone());
        observer.set_liveness_probe(|_| false);

        let run = make_run(&wave, "implement", "wire the tail");
        store.create_run(&run).await.expect("run");
        let session = worker_session(&wave, &run.id);
        store.register_session(&session).await.expect("session");

        observer.poll_once().await;
        observer.poll_once().await; // repeat poll: guard holds

        let closed = store
            .get_control_session(&session.id)
            .await
            .expect("lookup")
            .expect("row kept");
        assert_eq!(
            closed.status,
            SessionStatus::Failed,
            "a dead worker's Running row is closed by the probe"
        );
        assert!(
            runtime.in_flight_workers().is_empty(),
            "in_flight frees up when the dead worker is closed"
        );

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let finished: Vec<(String, WorkerOutcome, String)> = events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::RunCompleted {
                    run_id,
                    outcome,
                    summary,
                } => Some((run_id.clone(), *outcome, summary.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1, "one finish, once");
        assert_eq!(finished[0].0, run.id.to_string());
        assert_eq!(finished[0].1, WorkerOutcome::Failed);
        assert_eq!(finished[0].2, "process gone");
    }

    /// Pending rows (registered but not yet launched) and non-tmux rows are
    /// never probed — only a Running tmux-backed worker can be "gone".
    #[tokio::test]
    async fn observer_probe_leaves_pending_workers_alone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");
        let runtime =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = StoreObserver::new(runtime, store.clone(), wave.id().clone());
        observer.set_liveness_probe(|_| false);

        let run = make_run(&wave, "implement", "not yet launched");
        store.create_run(&run).await.expect("run");
        let mut session = worker_session(&wave, &run.id);
        session.status = SessionStatus::Pending;
        store.register_session(&session).await.expect("session");

        observer.poll_once().await;

        let stored = store
            .get_control_session(&session.id)
            .await
            .expect("lookup")
            .expect("row kept");
        assert_eq!(
            stored.status,
            SessionStatus::Pending,
            "a Pending row is the dispatcher's to launch, not the probe's to close"
        );
    }
}
