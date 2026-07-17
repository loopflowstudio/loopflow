use std::process::{Command, Output};

use crate::child_session::{CallerAuthority, ChildCommandSource, ChildRef};
use crate::engine::wave_context::{WaveResolveError, WAVE_ID_ENV};
use crate::id::WaveId;
use crate::ops::error::{OpsError, OpsResult};
use crate::project_session::ProjectSessionId;
use crate::store::SharedStore;
use crate::wave::Wave;

/// Env vars a managed body carries: their presence proves the process is inside
/// a Wave/Project/Task Session, whatever else was stripped. Each body type
/// carries at least two, so removing one still leaves a marker and the resolver
/// fails closed rather than downgrading to [`CallerAuthority::Operator`].
/// `LF_RUN_ID`/`LF_PROCESS_ID` are excluded — the journal sets them on every
/// `lf` process, including the human CLI, so they cannot distinguish a body from
/// a shell.
const MANAGED_SESSION_MARKERS: [&str; 4] = [
    "LF_WAVE_ID",
    "LF_PROJECT_SESSION_ID",
    "LF_TASK_SESSION_ID",
    "LF_CHANNEL",
];

/// The first managed-session marker present in this process's environment, if
/// any. Used only on the no-wave/no-project path to tell an operator shell (no
/// markers → `Operator`) from a managed body whose identity env is inconsistent
/// (a marker present but no resolvable authority → refuse).
fn stray_managed_marker() -> Option<&'static str> {
    MANAGED_SESSION_MARKERS
        .into_iter()
        .find(|var| std::env::var_os(var).is_some())
}

pub fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn stderr_from_output(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}

/// Normalize a Wave name: trim, strip a leading `wave/`, and reject empty.
pub fn normalize_wave_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed
        .strip_prefix("wave/")
        .unwrap_or(trimmed)
        .trim_matches('/');
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

/// Resolve caller identity once at the CLI invocation boundary.
///
/// An explicit, already-resolved `--wave` wins over inherited context and is
/// converted directly to [`CallerAuthority::Wave`]. Otherwise the inherited
/// Project/Wave identity is transport into this function and is parsed into the
/// typed value. No ops function reads it again.
///
/// 1. **Explicit Wave**: the resolved `--wave` row → `Wave(id)`.
/// 2. **Inherited Project**: `LF_PROJECT_SESSION_ID` → `Project(id)`.
/// 3. **Inherited Wave**: `LF_WAVE_ID` resolves through the registry →
///    `Wave(id)`; stale identity is a loud error.
/// 4. **Inconsistent managed session**: no wave/project resolved but a managed
///    marker is still present (e.g. a body that stripped `LF_WAVE_ID` but keeps
///    `LF_CHANNEL`) → **refuse**, never downgrade to `Operator`.
/// 5. **Operator**: no managed marker at all → `Operator`.
///
/// This is the fail-closed inversion of the old `NoContext → Human` rule:
/// removing one variable from a managed body can no longer mint operator
/// authority.
pub fn resolve_caller_authority(explicit_wave: Option<&Wave>) -> OpsResult<CallerAuthority> {
    if let Some(wave) = explicit_wave {
        return Ok(CallerAuthority::Wave(wave.id().clone()));
    }

    if let Some(raw) = std::env::var_os("LF_PROJECT_SESSION_ID") {
        let raw = raw.into_string().map_err(|_| {
            OpsError::Message("ambient Project Session id is not valid UTF-8".into())
        })?;
        let project_id = ProjectSessionId::parse(&raw).map_err(|error| {
            OpsError::Message(format!("invalid ambient Project Session id: {error}"))
        })?;
        return Ok(CallerAuthority::Project(project_id));
    }

    if let Some(raw) = std::env::var_os(WAVE_ID_ENV) {
        let raw = raw
            .into_string()
            .map_err(|_| OpsError::Message("ambient Wave identity is not valid UTF-8".into()))?;
        let identity = normalize_wave_name(&raw)
            .ok_or_else(|| OpsError::Message(WaveResolveError::NoContext.to_string()))?;
        let lookup = identity.clone();
        let wave = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("current-thread runtime always builds");
            runtime.block_on(async move {
                let store = crate::store::open_existing_store().await.ok_or_else(|| {
                    WaveResolveError::Registry("no wave registry on this machine".to_string())
                })?;
                let row = if let Ok(id) = lookup.parse::<WaveId>() {
                    store.get_wave(&id).await
                } else {
                    store.get_wave_by_name(&lookup).await
                }
                .map_err(|error| WaveResolveError::Registry(error.to_string()))?;
                row.ok_or_else(|| WaveResolveError::StaleIdentity(lookup))
            })
        })
        .join()
        .map_err(|_| OpsError::Message("caller authority resolver thread panicked".into()))?
        .map_err(|error| OpsError::Message(error.to_string()))?;
        return Ok(CallerAuthority::Wave(wave.id().clone()));
    }

    match stray_managed_marker() {
        Some(marker) => Err(OpsError::Message(format!(
            "this command runs inside a managed session ({marker} is set) but carries no \
             resolvable Wave or Project authority. Re-run with the session identity intact, \
             or from a clean operator shell."
        ))),
        None => Ok(CallerAuthority::Operator),
    }
}

/// Validate one surface-resolved authority against the command target and
/// convert it to stored command provenance. This function uses only the typed
/// value; environment is no longer an authority input below CLI dispatch.
pub(crate) async fn validate_caller_authority(
    store: &SharedStore,
    owning_wave_id: &WaveId,
    target: &ChildRef,
    project_route_current: Option<&ProjectSessionId>,
    subject: &str,
    authority: &CallerAuthority,
) -> OpsResult<ChildCommandSource> {
    match authority {
        CallerAuthority::Operator => Ok(ChildCommandSource::Human),
        CallerAuthority::Wave(wave_id) if wave_id == owning_wave_id => {
            Ok(ChildCommandSource::Wave(wave_id.clone()))
        }
        CallerAuthority::Wave(wave_id) => {
            let caller_name = owning_wave_name(store, wave_id).await;
            let owning_name = owning_wave_name(store, owning_wave_id).await;
            Err(OpsError::Message(format!(
                "Wave {caller_name} cannot control {subject} owned by Wave {owning_name}"
            )))
        }
        CallerAuthority::Project(project_id) if matches!(target, ChildRef::Task(_)) => {
            match project_route_current {
                Some(current) if current == project_id => {
                    Ok(ChildCommandSource::Project(project_id.clone()))
                }
                Some(current) => Err(OpsError::Message(format!(
                    "Project Session {project_id} cannot control {subject}; \
                     its live routing target is Project Session {current}"
                ))),
                None => Err(OpsError::Message(format!(
                    "Project Session {project_id} cannot control {subject}"
                ))),
            }
        }
        CallerAuthority::Project(project_id) => Err(OpsError::Message(format!(
            "Project Session {project_id} cannot control {subject}"
        ))),
    }
}

/// Best-effort owning wave name for error messages. Falls back to the UUID
/// display if the owning wave is not registered (very unusual).
async fn owning_wave_name(store: &SharedStore, wave_id: &WaveId) -> String {
    store
        .get_wave(wave_id)
        .await
        .ok()
        .flatten()
        .map(|wave| wave.name().to_string())
        .unwrap_or_else(|| wave_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{open_store, StorageConfig};
    use crate::wave::Wave;

    /// Holds the test-env serialization lock and snapshots **every** managed
    /// marker, clearing them so each case controls the full set — the Session
    /// this test runs inside leaks its own `LF_TASK_SESSION_ID`/`LF_WAVE_ID`
    /// otherwise, which would turn the "no markers → Operator" case into a
    /// fail-closed refusal. The `MutexGuard` lives in a field so clippy's
    /// `await_holding_lock` stays quiet — the lock must span the async body.
    struct ManagedEnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
        previous_db_path: Option<std::ffi::OsString>,
    }

    impl ManagedEnvGuard {
        fn new() -> Self {
            let lock = crate::journal::test_env_lock();
            let previous = MANAGED_SESSION_MARKERS
                .into_iter()
                .map(|var| {
                    let prior = std::env::var_os(var);
                    std::env::remove_var(var);
                    (var, prior)
                })
                .collect();
            let previous_db_path = std::env::var_os("LF_DB_PATH");
            std::env::remove_var("LF_DB_PATH");
            Self {
                _lock: lock,
                previous,
                previous_db_path,
            }
        }
    }

    impl Drop for ManagedEnvGuard {
        fn drop(&mut self) {
            for (var, prior) in &self.previous {
                match prior {
                    Some(value) => std::env::set_var(var, value),
                    None => std::env::remove_var(var),
                }
            }
            match &self.previous_db_path {
                Some(value) => std::env::set_var("LF_DB_PATH", value),
                None => std::env::remove_var("LF_DB_PATH"),
            }
        }
    }

    #[tokio::test]
    async fn invocation_authority_distinguishes_explicit_and_inherited_context() {
        let _guard = ManagedEnvGuard::new();
        let tmp = tempfile::tempdir().unwrap();
        let database = tmp.path().join("loopflow.db");
        std::env::set_var("LF_DB_PATH", &database);
        let store = open_store(&StorageConfig::sqlite(database)).await.unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "infrastructure".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();

        let project = ProjectSessionId::new();
        std::env::set_var("LF_PROJECT_SESSION_ID", project.as_str());
        std::env::set_var(WAVE_ID_ENV, wave.id().as_str());
        assert_eq!(
            resolve_caller_authority(Some(&wave)).unwrap(),
            CallerAuthority::Wave(wave.id().clone()),
            "explicit --wave must win without round-tripping through inherited Project context"
        );
        assert_eq!(
            resolve_caller_authority(None).unwrap(),
            CallerAuthority::Project(project.clone())
        );

        std::env::remove_var("LF_PROJECT_SESSION_ID");
        assert_eq!(
            resolve_caller_authority(None).unwrap(),
            CallerAuthority::Wave(wave.id().clone())
        );

        std::env::remove_var(WAVE_ID_ENV);
        assert_eq!(
            resolve_caller_authority(None).unwrap(),
            CallerAuthority::Operator
        );

        std::env::set_var("LF_CHANNEL", "infrastructure");
        let message = resolve_caller_authority(None).unwrap_err().to_string();
        assert!(
            message.contains("managed session") && message.contains("LF_CHANNEL"),
            "stray managed evidence must fail closed: {message}"
        );
    }

    #[tokio::test]
    async fn wave_authority_is_validated_against_the_target() {
        let _guard = ManagedEnvGuard::new();
        let tmp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(tmp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let owning = Wave::new(
            WaveId::new(),
            "infrastructure".into(),
            tmp.path().display().to_string(),
        );
        let foreign = Wave::new(
            WaveId::new(),
            "product".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&owning).await.unwrap();
        store.create_wave(&foreign).await.unwrap();
        let target = ChildRef::Task(crate::task::TaskSessionId::new());

        let source = validate_caller_authority(
            &store,
            owning.id(),
            &target,
            None,
            "Task INF-123",
            &CallerAuthority::Wave(owning.id().clone()),
        )
        .await
        .unwrap();
        assert_eq!(source, ChildCommandSource::Wave(owning.id().clone()));

        let message = validate_caller_authority(
            &store,
            owning.id(),
            &target,
            None,
            "Task INF-123",
            &CallerAuthority::Wave(foreign.id().clone()),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(message.contains("cannot control"), "{message}");
    }

    /// A Project caller is validated against the *live* routing target, not the
    /// historical `project_session_id`. W2-243 routes supervision to a live
    /// successor when the launcher Project Session is terminal, so the successor
    /// must be able to control the Task and the terminal predecessor must not.
    /// This drives the funnel's arm 1 comparison directly by supplying
    /// `route.current`; sabotage that compares against the historical id instead
    /// would reject the successor here.
    #[tokio::test]
    async fn project_caller_is_validated_against_the_live_route() {
        let tmp = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(tmp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let owning = Wave::new(
            WaveId::new(),
            "infrastructure".into(),
            tmp.path().display().to_string(),
        );
        store.create_wave(&owning).await.unwrap();
        let owning_id = owning.id().clone();
        let subject = "Task INF-123";

        let historical = ProjectSessionId::new(); // the terminal launcher
        let successor = ProjectSessionId::new(); // the live routing target
        let target = ChildRef::Task(crate::task::TaskSessionId::new());

        let source = validate_caller_authority(
            &store,
            &owning_id,
            &target,
            Some(&successor),
            subject,
            &CallerAuthority::Project(successor.clone()),
        )
        .await
        .expect("the live routing target controls the Task");
        assert_eq!(source, ChildCommandSource::Project(successor.clone()));

        // The terminal historical predecessor's command is refused, naming the
        // live target — a comparison against the historical id would accept it.
        let msg = validate_caller_authority(
            &store,
            &owning_id,
            &target,
            Some(&successor),
            subject,
            &CallerAuthority::Project(historical),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(
            msg.contains("cannot control") && msg.contains(successor.as_str()),
            "a stale predecessor is refused, naming the live target: {msg}"
        );
    }
}
