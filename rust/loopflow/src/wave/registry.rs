//! The wave server's seat in the shared session registry — store-direct.
//!
//! `lf serve <name>` is its own process; no daemon launches or supervises it.
//! The shared local store (the same SQLite db lfd serves from — the db IS
//! the registry) carries two facts this module owns:
//!
//! - **Registration.** On boot the server ensures the wave itself has a row
//!   ([`ensure_wave_row`] — a reachable store with no row for the wave gets
//!   a minimal one, never an unregistered run), then writes itself a
//!   `WaveAgent` session row (source `wave_server`, endpoint + pid in `env`)
//!   so Loopflow's agent tree shows the loop and one-brain enforcement has a
//!   fact to key on. Before writing, [`register`] probes the wave's live
//!   WaveAgent rows: a `wave_server` row whose pid is dead is a crashed
//!   server — closed on the spot; a surviving live brain is a refusal naming
//!   it, unless `force` takes over (kill the recorded pid / tmux session,
//!   cancel the row). Graceful shutdown and Ctrl-C mark the row terminal;
//!   [`reconcile_wave_servers`] covers a crash at lfd boot, and so does the
//!   boot-time probe of the next `lf serve`.
//!
//! - **Observation.** [`StoreObserver`] drains typed Project and Task
//!   observations addressed to this wave. Child ledgers and the SQLite outbox
//!   are durable; delivery into the wave journal is idempotent.
//!
//! **The no-store story, honestly:** a machine without the registry db gets
//! no registration, no one-brain enforcement, and no child observations —
//! the wave server is fully functional anyway, and says so once.

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
    Session, SessionStatus, SessionUse, Wave, LIVE_SESSION_STATUSES, WAVE_SERVER_ENDPOINT_ENV,
    WAVE_SERVER_PID_ENV, WAVE_SERVER_SOURCE,
};
use crate::lfdb::{SharedStore, StoreResult};
use crate::task::TaskObservation;
use crate::wave::runtime::WaveRuntime;

/// How often the observer re-reads the store between turns. Modest by
/// design: the loop also refreshes right before every turn it takes.
pub const POLL_CADENCE: Duration = Duration::from_secs(10);

/// Everything registration needs: the opened store, the wave's row, and this
/// server's identity.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub store: SharedStore,
    pub wave: Wave,
    /// The worktree the server (and its loop) runs in.
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
/// minimal and refreshes authored launch configuration from GOAL.md
/// frontmatter ([`read_wave_config`]): goal and task capacity when present,
/// [`Wave::new`] defaults otherwise. This refresh makes a GOAL.md edit take
/// effect on the next `lf serve`, including for rows created by older builds.
///
/// # Errors
/// Store failures only; the caller treats them as soft (run unregistered).
pub async fn ensure_wave_row(
    store: &SharedStore,
    main_repo: &Path,
    name: &str,
) -> StoreResult<Wave> {
    let existing = store.get_wave_by_name(name).await?;
    let is_new = existing.is_none();
    let mut wave = existing.unwrap_or_else(|| {
        Wave::new(
            LfdId::new(),
            name.to_string(),
            main_repo.display().to_string(),
        )
    });
    if let Some(config) = read_wave_config(main_repo, name) {
        if let Some(goal) = config.goal.filter(|goal| !goal.trim().is_empty()) {
            wave.goal = goal;
        }
        if let Some(task_capacity) = config.task_capacity {
            wave.task_capacity = task_capacity;
        }
        wave.paused = config.paused.unwrap_or(false);
    }
    store.create_wave(&wave).await?;
    if is_new {
        tracing::info!(
            wave = name,
            wave_id = %wave.id,
            "wave was not in the session registry; created its row"
        );
    }
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
        skill: "loop".to_string(),
        agent: "lf".to_string(),
        cwd: config.cwd.clone(),
        argv: vec![
            "lf".to_string(),
            "serve".to_string(),
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
/// session supervisor's to observe and count as live.
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

/// Close every live `wave_server` row whose recorded process has died.
///
/// `lfd` calls this once at boot. It does not launch or supervise work; the
/// registry cleanup only keeps one-brain enforcement from keying on a crashed
/// server that never reached graceful deregistration.
///
/// # Errors
///
/// Returns a store error when sessions cannot be read or a reconciled row
/// cannot be persisted.
pub async fn reconcile_wave_servers(store: &SharedStore) -> StoreResult<u32> {
    let sessions = store
        .list_control_sessions(None, Some(LIVE_SESSION_STATUSES))
        .await?;
    let mut completed = 0;
    for mut session in sessions {
        if session.source != WAVE_SERVER_SOURCE {
            continue;
        }
        let alive = match wave_server_pid(&session) {
            Some(pid) => process_alive(pid).await,
            None => false,
        };
        if !alive && session.complete(1) {
            store.update_control_session(&session).await?;
            tracing::info!(session_id = %session.id, "closed crashed wave server session");
            completed += 1;
        }
    }
    Ok(completed)
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
pub async fn wave_server_endpoint(
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
        .output()
        .await
        .is_ok_and(|output| output.status.success())
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

/// Polls the durable child-observation outbox for this wave.
///
/// Project and Task lifecycle owners reconcile their own process liveness.
/// This observer has one job: carry their typed events into the Wave journal.
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

    /// Deliver every pending typed child observation. Store errors are logged
    /// and retried on the next poll.
    pub async fn poll_once(&self) {
        self.poll_child_observations().await;
    }

    async fn poll_child_observations(&self) {
        let supervisor = crate::child_session::SessionSupervisor::Wave {
            wave_id: self.wave_id.clone(),
        };
        let observations = match self.store.pending_observations(&supervisor).await {
            Ok(observations) => observations,
            Err(error) => {
                tracing::debug!(%error, "wave observer child outbox read failed");
                return;
            }
        };
        for observation in observations {
            let should_ack = match (observation.source, observation.payload) {
                (
                    crate::child_session::ChildRef::Task(session_id),
                    crate::project_session::ChildEventPayload::Task { event },
                ) => match self.store.get_task_session(&session_id).await {
                    Ok(Some(session)) => {
                        self.runtime.deliver_task_observation(TaskObservation {
                            session_id,
                            issue_identifier: session.issue.identifier,
                            event_id: observation.event_id,
                            event,
                        });
                        true
                    }
                    Ok(None) => false,
                    Err(error) => {
                        tracing::debug!(%error, %session_id, "wave observer Task read failed");
                        continue;
                    }
                },
                (
                    crate::child_session::ChildRef::Project(session_id),
                    crate::project_session::ChildEventPayload::Project { event },
                ) => match self.store.get_project_session(&session_id).await {
                    Ok(Some(session)) => {
                        self.runtime.deliver_project_observation(
                            crate::project_session::ProjectObservation {
                                session_id,
                                project: session.project.slug,
                                event_id: observation.event_id,
                                event,
                            },
                        );
                        true
                    }
                    Ok(None) => false,
                    Err(error) => {
                        tracing::debug!(%error, %session_id, "wave observer Project read failed");
                        continue;
                    }
                },
                _ => {
                    tracing::warn!(
                        outbox_id = observation.id,
                        "child observation shape mismatched its source"
                    );
                    false
                }
            };
            if should_ack {
                let _ = self.store.mark_observation_delivered(observation.id).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::lfd::types::WaveStatus;
    use crate::lfdb::{open_store, StorageConfig};

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
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: "/tmp/repo".to_string(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            task_capacity: 2,
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

    /// A wave_server WaveAgent row as a previous `lf serve` would have left it.
    fn server_session(wave: &Wave, pid: u32) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            skill: "loop".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.ship".to_string(),
            argv: vec!["lf".to_string(), "serve".to_string(), wave.name().clone()],
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
            "---\ngoal: keep shipping\ntask_capacity: 0\n---\nShip.\n",
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
        assert_eq!(stored.task_capacity, 0, "task capacity from GOAL.md");
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
        assert_eq!(
            again.task_capacity, 0,
            "authored capacity remains effective"
        );
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
    async fn lfd_boot_reconciles_only_crashed_wave_servers() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("seed wave");
        let crashed = server_session(&wave, 4_000_000);
        let live = server_session(&wave, std::process::id());
        store
            .register_session(&crashed)
            .await
            .expect("seed crashed server");
        store
            .register_session(&live)
            .await
            .expect("seed live server");

        assert_eq!(reconcile_wave_servers(&store).await.unwrap(), 1);
        assert_eq!(
            store
                .get_control_session(&crashed.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            SessionStatus::Failed
        );
        assert_eq!(
            store
                .get_control_session(&live.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            SessionStatus::Running
        );
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
}
