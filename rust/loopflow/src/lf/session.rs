//! Self-registration of bare `lf` runs in the shared session registry.
//!
//! Managed sessions get `LFD_WAVE_ID`/`LFD_SESSION_ID` from the lfd executor,
//! and dispatches chain wave/parent attribution off that env. A bare
//! `lf design` typed inside such a session used to be invisible — no Session
//! row, no wave attribution. This module closes the gap: when an
//! agent-launching `lf` command starts inside a wave context — the shared
//! ambient rule ([`crate::engine::wave_context::resolve_ambient_wave`]): env
//! first, else the wave worktree the run executes in — it writes its own
//! Session row to the shared local store (the db IS the registry — no daemon
//! in the path) and marks itself terminal when the run ends. A run whose row
//! placement already created
//! registers nothing but still completes that existing row at run end —
//! daemonless dispatches have no other process to close them. No wave
//! context or no store on this machine → exactly the old behavior, silently.
//!
//! # Env contract
//!
//! - `LFD_WAVE_ID` — wave attribution. Set by the lfd executor, inherited down
//!   the process tree. Absent → fall back to the run's wave worktree; outside
//!   any wave context, never register.
//! - `LFD_SESSION_ID` — the nearest enclosing session. The executor sets it on
//!   the process it launches, pointing at that process's *own* session row. A
//!   self-registered `lf` overwrites it for its descendants with the id of the
//!   session it just registered, so grandchildren chain parentage correctly.
//! - `LFD_SESSION_INHERITED` — whose session `LFD_SESSION_ID` is. The executor
//!   never sets it, so `LFD_SESSION_ID` without the marker means "this very
//!   process already has a session row" (executor-launched) — registering
//!   again would double-count the run. Every `lf` exports
//!   `LFD_SESSION_INHERITED=1` before spawning anything, flipping the meaning
//!   for descendants to "an ancestor's session — register your own row with it
//!   as parent".

use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use time::OffsetDateTime;

use crate::engine::wave_context::{resolve_ambient_wave, AmbientWaveRef};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Session, SessionStatus, SessionUse, LF_CLI_SOURCE};
use crate::lfdb::{open_existing_store, SharedStore};

pub const WAVE_ID_ENV: &str = "LFD_WAVE_ID";
/// The channel a dispatched process speaks on by default — the work line's
/// ownership name (`goals.148e0e02`), set by placed `lf` runs. A bare run
/// without it falls back to the worktree name, which is the same channel
/// name by construction (see `engine::wave_context::resolve_ambient_channel`).
pub const CHANNEL_ENV: &str = "LFD_CHANNEL";
pub const SESSION_ID_ENV: &str = "LFD_SESSION_ID";
pub const SESSION_INHERITED_ENV: &str = "LFD_SESSION_INHERITED";

/// Where this `lf` invocation stands relative to the session registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunContext {
    /// No ambient wave (no env, not a wave worktree): leave everything
    /// untouched.
    Outside,
    /// Dispatcher-launched: placement already created this process's session row and set
    /// `LFD_SESSION_ID` to it. Registering again would double-count, but the
    /// run still completes its own existing row at exit — with no daemon
    /// guaranteed to be running, nothing else may ever mark it terminal.
    OwnSession,
    /// Inside a wave context without an own session row: register one.
    NeedsRegistration {
        wave: AmbientWaveRef,
        parent_session_id: Option<String>,
    },
}

/// The double-registration rule, in one place: `LFD_SESSION_ID` without
/// `LFD_SESSION_INHERITED` is *this process's* session (the dispatcher made
/// the row); with the marker it is an ancestor's session and this run
/// registers its own row underneath it. The wave itself comes from the
/// shared ambient rule ([`resolve_ambient_wave`]) — a bare run inside a wave
/// worktree registers just like an env-attributed one.
pub fn classify_run_context(
    ambient: Option<AmbientWaveRef>,
    session_id: Option<&str>,
    session_inherited: bool,
) -> RunContext {
    let Some(wave) = ambient else {
        return RunContext::Outside;
    };
    let session_id = session_id.filter(|value| !value.is_empty());
    if session_id.is_some() && !session_inherited {
        return RunContext::OwnSession;
    }
    RunContext::NeedsRegistration {
        wave,
        parent_session_id: session_id.map(str::to_string),
    }
}

/// Register this `lf` invocation as a session if it runs inside a wave
/// context, or adopt the row a dispatcher already created for it. Returns a
/// guard to complete with the run's exit code; dropping it unfinished (panic,
/// early error) records a failure, and Ctrl+C reports via the interrupt-hook
/// machinery. Returns `None` — with zero noise — outside wave contexts and
/// when the machine has no registry store (or the write fails).
pub fn register_run(skill: &str, agent: &str, argv: &[String]) -> Option<RunSession> {
    let repo_root = crate::lf::commands::util::find_repo_root().ok();
    register_run_in(repo_root.as_deref(), skill, agent, argv)
}

/// The body of [`register_run`], a function of the repo the run executes in
/// (the binary hands in the discovered root; tests hand in fixture paths so
/// they never touch the developer's real registry).
fn register_run_in(
    repo_root: Option<&Path>,
    skill: &str,
    agent: &str,
    argv: &[String],
) -> Option<RunSession> {
    let context = classify_run_context(
        resolve_ambient_wave(env_var(WAVE_ID_ENV).as_deref(), repo_root),
        env_var(SESSION_ID_ENV).as_deref(),
        env_var(SESSION_INHERITED_ENV).is_some(),
    );
    let (wave, parent_session_id) = match context {
        RunContext::Outside => return None,
        RunContext::OwnSession => {
            mark_child_sessions_inherited();
            return adopt_own_session();
        }
        RunContext::NeedsRegistration {
            wave,
            parent_session_id,
        } => (wave, parent_session_id),
    };
    let parent_session_id: Option<LfdId> = match parent_session_id {
        Some(value) => Some(value.parse().ok()?),
        None => None,
    };
    let session = block_on(register_session(
        wave,
        parent_session_id,
        skill.to_string(),
        agent.to_string(),
        argv.to_vec(),
    ))??;

    // Descendants chain off the new session; the marker keeps reading as
    // "an ancestor's session" for them.
    std::env::set_var(SESSION_ID_ENV, session.session_id());
    mark_child_sessions_inherited();

    let interrupted = Arc::clone(&session.inner);
    crate::engine::agent::register_interrupt_cleanup(move || interrupted.complete(130));
    Some(session)
}

/// Adopt the session row a dispatcher created for this process — placement
/// points `LFD_SESSION_ID` at a row the
/// child owns. If no daemon ever runs, nothing else marks the row terminal,
/// so the run completes it at exit through the same guard machinery as a
/// self-registered row. Never creates a row: a missing row or store is a
/// silent `None`, and an already-terminal row (canceled, or closed by a
/// faster reconciler) stays untouched.
fn adopt_own_session() -> Option<RunSession> {
    let session_id: LfdId = env_var(SESSION_ID_ENV)?.parse().ok()?;
    let (store, session) = block_on(async move {
        let store: SharedStore = Arc::new(open_existing_store().await?);
        let session = store.get_control_session(&session_id).await.ok()??;
        Some((store, session))
    })??;
    if session.status.is_terminal() {
        return None;
    }
    let session = RunSession {
        inner: Arc::new(SessionHandle {
            store,
            session,
            completed: AtomicBool::new(false),
        }),
    };
    let interrupted = Arc::clone(&session.inner);
    crate::engine::agent::register_interrupt_cleanup(move || interrupted.complete(130));
    Some(session)
}

/// Flip `LFD_SESSION_ID`'s meaning for everything this process spawns: the
/// row belongs to an ancestor, not to the spawned process. Called by every
/// `lf` command — including non-registering ones like `lf pr`/`lf wave`,
/// which may themselves be the executor-launched session owner.
pub fn mark_child_sessions_inherited() {
    if env_var(SESSION_ID_ENV).is_some() {
        std::env::set_var(SESSION_INHERITED_ENV, "1");
    }
}

/// A session registered for this process. Complete it with the run's exit
/// code; dropping it unfinished records a failure.
#[derive(Debug)]
pub struct RunSession {
    inner: Arc<SessionHandle>,
}

impl RunSession {
    pub fn complete(&self, exit_code: i32) {
        self.inner.complete(exit_code);
    }

    pub fn session_id(&self) -> &str {
        self.inner.session.id.as_str()
    }
}

impl Drop for RunSession {
    fn drop(&mut self) {
        // Panic / early-error safety net: a run that never reported failed.
        self.inner.complete(1);
    }
}

#[derive(Debug)]
struct SessionHandle {
    store: SharedStore,
    session: Session,
    completed: AtomicBool,
}

impl SessionHandle {
    /// Mark the row terminal, exactly once, best-effort and silent — a run
    /// must never fail because its registry write did. The row is re-read
    /// before the write, adopted and self-registered alike: a running lfd's
    /// reconciler (or a cancel) may have closed it already, and
    /// `update_control_session` overwrites unconditionally — a terminal row
    /// must never be flipped.
    fn complete(&self, exit_code: i32) {
        if self.completed.swap(true, Ordering::SeqCst) {
            return;
        }
        let store = self.store.clone();
        let id = self.session.id.clone();
        let _ = block_on(async move {
            let Some(mut row) = store.get_control_session(&id).await.ok().flatten() else {
                return;
            };
            if !row.complete(exit_code) {
                return;
            }
            let _ = store.update_control_session(&row).await;
        });
    }
}

/// Open the registry (if this machine has one), resolve the ambient wave to
/// its row, and write the session row. Any failure — no store, wave row
/// missing, insert error — is a silent `None`: instrumentation never gets in
/// the run's way.
async fn register_session(
    wave: AmbientWaveRef,
    mut parent_session_id: Option<LfdId>,
    skill: String,
    agent: String,
    argv: Vec<String>,
) -> Option<RunSession> {
    let store: SharedStore = Arc::new(open_existing_store().await?);
    // The wave row anchors the registration; an env id this store never
    // heard of is stale env, and a worktree name with no wave row is just a
    // branch — both silent no-ops, not errors.
    let wave_id = match wave {
        AmbientWaveRef::Id(id) => {
            let id: LfdId = id.parse().ok()?;
            store.get_wave(&id).await.ok()??;
            id
        }
        AmbientWaveRef::Name(name) => store.get_wave_by_name(&name).await.ok()??.id().clone(),
    };
    // A parent the store never heard of: keep the registration, drop the
    // attribution (the row's FK would reject it otherwise).
    if let Some(parent) = &parent_session_id {
        if store.get_control_session(parent).await.ok()?.is_none() {
            parent_session_id = None;
        }
    }
    let now = OffsetDateTime::now_utc();
    let session = Session {
        id: LfdId::new(),
        wave_id,
        run_id: None,
        parent_session_id,
        session_use: SessionUse::Worker,
        skill,
        agent,
        cwd: std::env::current_dir()
            .map(|cwd| cwd.display().to_string())
            .unwrap_or_default(),
        argv,
        env: BTreeMap::new(),
        source: LF_CLI_SOURCE.to_string(),
        tmux_name: current_tmux_session_name().unwrap_or_default(),
        // The registering process is already running when it writes the row.
        status: SessionStatus::Running,
        attached_at: Some(now),
        started_at: Some(now),
        completed_at: None,
        created_at: now,
        completion_token: None,
    };
    store.register_session(&session).await.ok()?;
    Some(RunSession {
        inner: Arc::new(SessionHandle {
            store,
            session,
            completed: AtomicBool::new(false),
        }),
    })
}

/// Run a store future to completion from lf's synchronous paths (main,
/// interrupt hooks, Drop). If a tokio runtime is already running on this
/// thread, hop to a fresh thread rather than panicking.
fn block_on<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> Option<T> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return std::thread::spawn(move || block_on_new_runtime(future))
            .join()
            .ok();
    }
    Some(block_on_new_runtime(future))
}

fn block_on_new_runtime<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("current-thread runtime always builds")
        .block_on(future)
}

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

/// Serializes tests across the crate that mutate process env (the session
/// vars, `LFD_DB_PATH`). Poison-tolerant so one panicked test doesn't wedge
/// the rest.
#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn current_tmux_session_name() -> Option<String> {
    std::env::var_os("TMUX")?;
    let output = std::process::Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use time::OffsetDateTime;

    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Session, SessionStatus, Wave, WaveStatus, TMUX_TERMINAL_SOURCE};
    use crate::lfdb::{open_store, StorageConfig};

    use crate::engine::wave_context::AmbientWaveRef;

    use super::{block_on_new_runtime, classify_run_context, register_run_in, RunContext};

    fn env_wave(id: &str) -> Option<AmbientWaveRef> {
        Some(AmbientWaveRef::Id(id.to_string()))
    }

    // ── classify_run_context: the disambiguation rule ───────────────────

    #[test]
    fn no_ambient_wave_means_no_registration() {
        assert_eq!(
            classify_run_context(None, Some("sess-1"), true),
            RunContext::Outside
        );
        assert_eq!(classify_run_context(None, None, false), RunContext::Outside);
    }

    #[test]
    fn executor_launched_runs_do_not_register_again() {
        // LFD_SESSION_ID without LFD_SESSION_INHERITED is this very process's
        // session row: the executor created it. Registering would double-count.
        assert_eq!(
            classify_run_context(env_wave("wave-1"), Some("sess-1"), false),
            RunContext::OwnSession
        );
    }

    #[test]
    fn inherited_session_id_becomes_the_parent() {
        assert_eq!(
            classify_run_context(env_wave("wave-1"), Some("sess-1"), true),
            RunContext::NeedsRegistration {
                wave: AmbientWaveRef::Id("wave-1".to_string()),
                parent_session_id: Some("sess-1".to_string()),
            }
        );
    }

    #[test]
    fn wave_context_without_session_registers_a_root_child() {
        // The ambient rule hands worktree-resolved runs in by name; they
        // classify exactly like env-attributed ones.
        assert_eq!(
            classify_run_context(Some(AmbientWaveRef::Name("goals".to_string())), None, false),
            RunContext::NeedsRegistration {
                wave: AmbientWaveRef::Name("goals".to_string()),
                parent_session_id: None,
            }
        );
    }

    // ── register_run against a temp registry store ──────────────────────

    fn clear_session_env() {
        for key in [
            super::WAVE_ID_ENV,
            super::SESSION_ID_ENV,
            super::SESSION_INHERITED_ENV,
            "LFD_DB_PATH",
        ] {
            std::env::remove_var(key);
        }
    }

    fn make_wave(repo: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "registry-wave".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    /// Create a registry db at `path` with one wave; point `LFD_DB_PATH` at it.
    fn seed_registry(path: &Path) -> Wave {
        let wave = make_wave("/tmp/repo");
        let seeded = wave.clone();
        let path_buf = path.to_path_buf();
        block_on_new_runtime(async move {
            let store = open_store(&StorageConfig::sqlite(path_buf))
                .await
                .expect("open registry store");
            store.create_wave(&seeded).await.expect("seed wave");
        });
        std::env::set_var("LFD_DB_PATH", path);
        wave
    }

    /// Seed a live session row (a dispatcher the run would chain off).
    fn seed_parent_session(path: &Path, wave: &Wave) -> LfdId {
        let now = OffsetDateTime::now_utc();
        let parent = Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: crate::lfd::types::SessionUse::WaveAgent,
            skill: "flowloop".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: Vec::new(),
            env: Default::default(),
            source: "wave_server".to_string(),
            tmux_name: String::new(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(now),
            completed_at: None,
            created_at: now,
            completion_token: None,
        };
        let id = parent.id.clone();
        let path = path.to_path_buf();
        block_on_new_runtime(async move {
            let store = open_store(&StorageConfig::sqlite(path))
                .await
                .expect("open registry store");
            store
                .register_session(&parent)
                .await
                .expect("seed parent session");
        });
        id
    }

    /// Seed the row a dispatcher creates for this very process — the
    /// Placed run shape: worker-owned row already running.
    fn seed_own_session(path: &Path, wave: &Wave) -> Session {
        let now = OffsetDateTime::now_utc();
        let session = Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: crate::lfd::types::SessionUse::Worker,
            skill: "dispatch:implement".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: Vec::new(),
            env: Default::default(),
            source: TMUX_TERMINAL_SOURCE.to_string(),
            tmux_name: "lf-worker".to_string(),
            status: SessionStatus::Running,
            attached_at: Some(now),
            started_at: Some(now),
            completed_at: None,
            created_at: now,
            completion_token: None,
        };
        let stored = session.clone();
        let path = path.to_path_buf();
        block_on_new_runtime(async move {
            let store = open_store(&StorageConfig::sqlite(path))
                .await
                .expect("open registry store");
            store
                .register_session(&stored)
                .await
                .expect("seed own session");
        });
        session
    }

    fn store_session(path: &Path, session: &Session) {
        let session = session.clone();
        let path = path.to_path_buf();
        block_on_new_runtime(async move {
            let store = open_store(&StorageConfig::sqlite(path))
                .await
                .expect("open registry store");
            store
                .update_control_session(&session)
                .await
                .expect("update session");
        });
    }

    fn count_sessions(path: &Path, wave: &Wave) -> usize {
        let wave_id = wave.id().clone();
        let path = path.to_path_buf();
        block_on_new_runtime(async move {
            let store = open_store(&StorageConfig::sqlite(path))
                .await
                .expect("open registry store");
            store
                .list_control_sessions(Some(&wave_id), None)
                .await
                .expect("session list")
                .len()
        })
    }

    fn load_session(path: &Path, id: &str) -> Session {
        let id: LfdId = id.parse().expect("session id");
        let path = path.to_path_buf();
        block_on_new_runtime(async move {
            let store = open_store(&StorageConfig::sqlite(path))
                .await
                .expect("open registry store");
            store
                .get_control_session(&id)
                .await
                .expect("session lookup")
                .expect("session stored")
        })
    }

    #[test]
    fn register_run_without_env_is_a_no_op() {
        let _guard = super::test_env_lock();
        clear_session_env();

        // No env and no wave-shaped repo: classification short-circuits
        // before any store is touched, so this holds even with a registry on
        // the machine.
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(register_run_in(Some(tmp.path()), "design", "lf", &["lf".to_string()]).is_none());
        assert!(register_run_in(None, "design", "lf", &["lf".to_string()]).is_none());
        assert!(std::env::var(super::SESSION_INHERITED_ENV).is_err());
    }

    /// Decision 2b — no invisible runs: a bare `lf` typed inside a wave
    /// worktree (no env at all) registers against the wave row its worktree
    /// name resolves to, root-parented.
    #[test]
    fn bare_run_in_wave_worktree_registers_by_worktree_name() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_wave_worktree("registry-wave");
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let wave = seed_registry(&db);

        let argv = vec!["lf".to_string(), "design".to_string()];
        let registration =
            register_run_in(Some(&worktree), "design", "lf", &argv).expect("registered");

        let stored = load_session(&db, registration.session_id());
        assert_eq!(stored.status, SessionStatus::Running);
        assert_eq!(stored.wave_id, *wave.id());
        assert_eq!(stored.parent_session_id, None, "root-parented");
        assert_eq!(stored.skill, "design");
        // Descendants chain off the new session.
        assert_eq!(
            std::env::var(super::SESSION_ID_ENV).as_deref(),
            Ok(registration.session_id())
        );

        registration.complete(0);
        assert_eq!(
            load_session(&db, registration.session_id()).status,
            SessionStatus::Succeeded
        );
        clear_session_env();
    }

    #[test]
    fn executor_launched_run_marks_children_without_registering() {
        let _guard = super::test_env_lock();
        clear_session_env();
        std::env::set_var(super::WAVE_ID_ENV, "wave-1");
        std::env::set_var(super::SESSION_ID_ENV, "sess-1");

        let registration = register_run_in(None, "design", "lf", &["lf".to_string()]);

        assert!(registration.is_none());
        // Children now read LFD_SESSION_ID as their parent's session.
        assert_eq!(
            std::env::var(super::SESSION_INHERITED_ENV).as_deref(),
            Ok("1")
        );
        // The executor's own value is untouched.
        assert_eq!(
            std::env::var(super::SESSION_ID_ENV).as_deref(),
            Ok("sess-1")
        );
        clear_session_env();
    }

    #[test]
    fn register_run_writes_running_row_and_completes_by_exit_code() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let wave = seed_registry(&db);
        let parent = seed_parent_session(&db, &wave);
        std::env::set_var(super::WAVE_ID_ENV, wave.id().to_string());
        std::env::set_var(super::SESSION_ID_ENV, parent.to_string());
        std::env::set_var(super::SESSION_INHERITED_ENV, "1");

        let argv = vec!["lf".to_string(), "design".to_string()];
        let registration = register_run_in(None, "design", "lf", &argv).expect("registered");

        let stored = load_session(&db, registration.session_id());
        assert_eq!(stored.status, SessionStatus::Running);
        assert_eq!(stored.wave_id, *wave.id());
        assert_eq!(stored.parent_session_id, Some(parent));
        assert_eq!(stored.argv, argv);
        assert_eq!(stored.skill, "design");
        assert_eq!(stored.source, "lf_cli");
        // Descendants chain off the new session.
        assert_eq!(
            std::env::var(super::SESSION_ID_ENV).as_deref(),
            Ok(registration.session_id())
        );
        assert_eq!(
            std::env::var(super::SESSION_INHERITED_ENV).as_deref(),
            Ok("1")
        );

        registration.complete(0);
        let stored = load_session(&db, registration.session_id());
        assert_eq!(stored.status, SessionStatus::Succeeded);

        // Drop after complete must not overwrite the terminal status.
        let id = registration.session_id().to_string();
        drop(registration);
        assert_eq!(load_session(&db, &id).status, SessionStatus::Succeeded);
        clear_session_env();
    }

    #[test]
    fn nonzero_exit_and_drop_record_failure() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let wave = seed_registry(&db);
        std::env::set_var(super::WAVE_ID_ENV, wave.id().to_string());

        let failed = register_run_in(None, "debug", "lf", &["lf".to_string()]).expect("registered");
        failed.complete(3);
        assert_eq!(
            load_session(&db, failed.session_id()).status,
            SessionStatus::Failed
        );

        // A registration dropped unfinished records a failure too (the
        // panic / early-error safety net).
        std::env::remove_var(super::SESSION_ID_ENV);
        std::env::remove_var(super::SESSION_INHERITED_ENV);
        let dropped =
            register_run_in(None, "debug", "lf", &["lf".to_string()]).expect("registered");
        let id = dropped.session_id().to_string();
        drop(dropped);
        assert_eq!(load_session(&db, &id).status, SessionStatus::Failed);
        clear_session_env();
    }

    #[test]
    fn register_run_is_silent_when_no_registry_exists() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("never-created.db");
        std::env::set_var("LFD_DB_PATH", &db);
        std::env::set_var(super::WAVE_ID_ENV, LfdId::new().to_string());

        assert!(register_run_in(None, "design", "lf", &["lf".to_string()]).is_none());
        assert!(!db.exists(), "a missing registry must not be conjured");
        clear_session_env();
    }

    // ── OwnSession: complete the dispatcher-created row, never make one ──

    #[test]
    fn own_session_run_completes_its_existing_row_by_exit_code() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let wave = seed_registry(&db);
        let own = seed_own_session(&db, &wave);
        std::env::set_var(super::WAVE_ID_ENV, wave.id().to_string());
        std::env::set_var(super::SESSION_ID_ENV, own.id.to_string());
        // LFD_SESSION_INHERITED absent: this process owns the row.

        let adoption =
            register_run_in(None, "implement", "lf", &["lf".to_string()]).expect("adopted");
        assert_eq!(adoption.session_id(), own.id.as_str());
        // No second row: adoption never registers.
        assert_eq!(count_sessions(&db, &wave), 1);
        // Children chain under the adopted session.
        assert_eq!(
            std::env::var(super::SESSION_ID_ENV).as_deref(),
            Ok(own.id.as_str())
        );
        assert_eq!(
            std::env::var(super::SESSION_INHERITED_ENV).as_deref(),
            Ok("1")
        );

        adoption.complete(0);
        assert_eq!(
            load_session(&db, own.id.as_str()).status,
            SessionStatus::Succeeded
        );

        // Drop after complete must not overwrite the terminal status.
        drop(adoption);
        assert_eq!(
            load_session(&db, own.id.as_str()).status,
            SessionStatus::Succeeded
        );
        clear_session_env();
    }

    #[test]
    fn own_session_nonzero_exit_and_drop_record_failure() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let wave = seed_registry(&db);
        std::env::set_var(super::WAVE_ID_ENV, wave.id().to_string());

        let failed_row = seed_own_session(&db, &wave);
        std::env::set_var(super::SESSION_ID_ENV, failed_row.id.to_string());
        std::env::remove_var(super::SESSION_INHERITED_ENV);
        let failed =
            register_run_in(None, "implement", "lf", &["lf".to_string()]).expect("adopted");
        failed.complete(3);
        assert_eq!(
            load_session(&db, failed_row.id.as_str()).status,
            SessionStatus::Failed
        );

        // An adoption dropped unfinished records a failure too (the
        // panic / early-error safety net).
        let dropped_row = seed_own_session(&db, &wave);
        std::env::set_var(super::SESSION_ID_ENV, dropped_row.id.to_string());
        std::env::remove_var(super::SESSION_INHERITED_ENV);
        let dropped =
            register_run_in(None, "implement", "lf", &["lf".to_string()]).expect("adopted");
        drop(dropped);
        assert_eq!(
            load_session(&db, dropped_row.id.as_str()).status,
            SessionStatus::Failed
        );
        clear_session_env();
    }

    #[test]
    fn own_session_with_missing_row_or_store_is_silent() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let wave = seed_registry(&db);
        std::env::set_var(super::WAVE_ID_ENV, wave.id().to_string());

        // Registry exists but the row does not: silent no-op, no row conjured.
        std::env::set_var(super::SESSION_ID_ENV, LfdId::new().to_string());
        assert!(register_run_in(None, "implement", "lf", &["lf".to_string()]).is_none());
        assert_eq!(count_sessions(&db, &wave), 0);

        // No registry at all: silent no-op, db not conjured.
        let missing = tmp.path().join("never-created.db");
        std::env::set_var("LFD_DB_PATH", &missing);
        std::env::remove_var(super::SESSION_INHERITED_ENV);
        assert!(register_run_in(None, "implement", "lf", &["lf".to_string()]).is_none());
        assert!(!missing.exists(), "a missing registry must not be conjured");
        clear_session_env();
    }

    #[test]
    fn own_session_never_overwrites_a_terminal_row() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let wave = seed_registry(&db);
        std::env::set_var(super::WAVE_ID_ENV, wave.id().to_string());

        // Already terminal at adopt time (canceled, or a running lfd
        // reconciled first): nothing to guard, nothing gets written.
        let mut canceled = seed_own_session(&db, &wave);
        assert!(canceled.cancel());
        store_session(&db, &canceled);
        std::env::set_var(super::SESSION_ID_ENV, canceled.id.to_string());
        assert!(register_run_in(None, "implement", "lf", &["lf".to_string()]).is_none());
        assert_eq!(
            load_session(&db, canceled.id.as_str()).status,
            SessionStatus::Canceled
        );

        // Goes terminal between adopt and complete (the reconciler race):
        // the completion write re-reads and skips.
        let raced = seed_own_session(&db, &wave);
        std::env::set_var(super::SESSION_ID_ENV, raced.id.to_string());
        std::env::remove_var(super::SESSION_INHERITED_ENV);
        let adoption =
            register_run_in(None, "implement", "lf", &["lf".to_string()]).expect("adopted");
        let mut closed = raced.clone();
        assert!(closed.cancel());
        store_session(&db, &closed);
        adoption.complete(0);
        assert_eq!(
            load_session(&db, raced.id.as_str()).status,
            SessionStatus::Canceled
        );
        clear_session_env();
    }

    #[test]
    fn register_run_is_silent_when_wave_is_unknown() {
        let _guard = super::test_env_lock();
        clear_session_env();
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        seed_registry(&db);
        // A wave id the store never heard of: stale env, silent no-op.
        std::env::set_var(super::WAVE_ID_ENV, LfdId::new().to_string());

        assert!(register_run_in(None, "design", "lf", &["lf".to_string()]).is_none());
        clear_session_env();
    }
}
