//! The wave server's seat in the shared session registry — store-direct.
//!
//! `lf wave <name>` is its own process; no daemon launches or supervises it.
//! The shared local store (the same SQLite db lfd serves from — the db IS
//! the registry) carries two facts this module owns:
//!
//! - **Registration.** On boot the server writes itself a `WaveAgent`
//!   session row (source `wave_server`, endpoint + pid in `env`) so
//!   Concerto's agent tree shows the mind and one-brain enforcement has a
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
//!   `WorkerDispatched` when a worker session+run appears, `WorkerFinished`
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::process::Command;

use crate::lfd::id::LfdId;
use crate::lfd::store::{SharedStore, StoreResult};
use crate::lfd::types::{
    Run, Session, SessionStatus, SessionUse, Wave, LIVE_SESSION_STATUSES, WAVE_SERVER_ENDPOINT_ENV,
    WAVE_SERVER_PID_ENV, WAVE_SERVER_SOURCE,
};
use crate::lfd::wave::journal::WorkerOutcome;
use crate::lfd::wave::runtime::WaveRuntime;

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

/// Whether a process with `pid` is running on this host (`kill -0` probe).
async fn process_alive(pid: u32) -> bool {
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
pub struct StoreObserver {
    runtime: Arc<WaveRuntime>,
    store: SharedStore,
    wave_id: LfdId,
}

impl fmt::Debug for StoreObserver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoreObserver")
            .field("wave_id", &self.wave_id)
            .finish()
    }
}

impl StoreObserver {
    pub fn new(runtime: Arc<WaveRuntime>, store: SharedStore, wave_id: LfdId) -> Self {
        Self {
            runtime,
            store,
            wave_id,
        }
    }

    /// Poll forever on `cadence`. Runs until aborted at server shutdown.
    pub async fn run(self: Arc<Self>, cadence: Duration) {
        loop {
            self.poll_once().await;
            tokio::time::sleep(cadence).await;
        }
    }

    /// One reconciliation pass: read this wave's worker sessions, journal
    /// new dispatches and fresh finishes. Store errors are logged and
    /// skipped — the next poll retries.
    pub async fn poll_once(&self) {
        let sessions = match self
            .store
            .list_control_sessions(Some(&self.wave_id), None)
            .await
        {
            Ok(sessions) => sessions,
            Err(err) => {
                tracing::debug!(error = %err, "wave observer session read failed");
                return;
            }
        };
        let workers: Vec<(&Session, String)> = sessions
            .iter()
            .filter(|session| session.session_use == SessionUse::Worker)
            .filter_map(|session| {
                session
                    .run_id
                    .as_ref()
                    .map(|run_id| (session, run_id.to_string()))
            })
            .collect();

        // Runs carry the flow/task for a dispatch and the PR/error for a
        // finish summary; fetch them only when this pass will journal.
        let in_flight: HashSet<String> = self
            .runtime
            .in_flight_workers()
            .into_iter()
            .map(|worker| worker.run_id)
            .collect();
        let needs_runs = workers.iter().any(|(session, run_id)| {
            !self.runtime.worker_known(run_id)
                || (session.status.is_terminal() && in_flight.contains(run_id))
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
            if !self.runtime.worker_known(&run_id) {
                let Some(run) = runs.get(&run_id) else {
                    tracing::warn!(run_id, "observed worker session with no run row; skipped");
                    continue;
                };
                if self.runtime.journal_worker_dispatched(
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
            }
            if !session.status.is_terminal() {
                continue;
            }
            let outcome = if session.status == SessionStatus::Succeeded {
                WorkerOutcome::Completed
            } else {
                WorkerOutcome::Failed
            };
            let mut summary = format!("session {}", session.status.as_str());
            if let Some(run) = runs.get(&run_id) {
                if let Some(pr) = &run.pr {
                    summary.push_str(&format!("; pr {}", pr.url));
                }
                if let Some(error) = &run.error {
                    summary.push_str(&format!("; error: {error}"));
                }
            }
            if self
                .runtime
                .journal_worker_finished(&run_id, outcome, &summary)
            {
                tracing::info!(run_id, outcome = outcome.name(), summary, "worker finished");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{
        PullRequest, RepoWork, RunStackStatus, RunStatus, WaveMode, WaveStatus,
        TMUX_TERMINAL_SOURCE,
    };
    use crate::lfd::wave::journal::{journal_path, EventKind, Journal};

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
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo: "/tmp/repo".to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
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
            activation_log_id: None,
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
        let (runtime, _inbox) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = StoreObserver::new(runtime.clone(), store.clone(), wave.id().clone());

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
                EventKind::WorkerDispatched {
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
                EventKind::WorkerFinished {
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
        let (runtime_2, _inbox) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("reopen runtime");
        let observer_2 = StoreObserver::new(runtime_2, store, wave.id().clone());
        observer_2.poll_once().await;
        let (_, events_after) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        assert_eq!(
            events.len(),
            events_after.len(),
            "restart journals nothing new"
        );
    }
}
