use std::process::{Command, Output};

use crate::child_session::{CallerAuthority, ChildRef};
use crate::engine::wave_context::{resolve_managed_wave_name, WaveResolveError, WAVE_ID_ENV};
use crate::id::WaveId;
use crate::ops::error::{OpsError, OpsResult};
use crate::project_session::ProjectSessionId;
use crate::store::SharedStore;

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

/// Resolve the ambient context into a typed [`CallerAuthority`] for a Task or
/// Project control command. One funnel, shared by the Task and Project control
/// paths so they cannot drift, evaluated top-down:
///
/// 1. **Project caller** (Task target only): `LF_PROJECT_SESSION_ID` present →
///    `Project(id)` iff it equals `project_route_current` — the *live* routing
///    target ([`resolve_task_project_route`](crate::ops::project::resolve_task_project_route)`.current`),
///    which follows a terminal historical Project Session to its live successor
///    (W2-243). A mismatch (or a Project caller against a target with no route)
///    is a loud "cannot control". A Project target skips this arm — a Project is
///    not controlled through the project-session marker.
/// 2. **Wave caller**: `LF_WAVE_ID` resolves to the owning wave → `Wave`; a
///    different registered wave → "cannot control"; unregistered → stale error.
/// 3. **Inconsistent managed session**: no wave/project resolved but a managed
///    marker is still present (e.g. a body that stripped `LF_WAVE_ID` but keeps
///    `LF_CHANNEL`) → **refuse**, never downgrade to `Operator`.
/// 4. **Operator**: no managed marker at all → `Operator`.
///
/// This is the fail-closed inversion of the old `NoContext → Human` rule:
/// removing one variable from a managed body can no longer mint operator
/// authority.
pub(crate) async fn resolve_caller_authority(
    store: &SharedStore,
    owning_wave_id: &WaveId,
    target: &ChildRef,
    project_route_current: Option<&ProjectSessionId>,
    subject: &str,
) -> OpsResult<CallerAuthority> {
    // Arm 1: a Project caller, and only for a Task target.
    if matches!(target, ChildRef::Task(_)) {
        if let Some(raw) = std::env::var_os("LF_PROJECT_SESSION_ID") {
            let raw = raw.into_string().map_err(|_| {
                OpsError::Message("ambient Project Session id is not valid UTF-8".into())
            })?;
            let project_id = ProjectSessionId::parse(&raw).map_err(|error| {
                OpsError::Message(format!("invalid ambient Project Session id: {error}"))
            })?;
            return match project_route_current {
                Some(current) if *current == project_id => Ok(CallerAuthority::Project(project_id)),
                Some(current) => Err(OpsError::Message(format!(
                    "Project Session {project_id} cannot control {subject}; \
                     its live routing target is Project Session {current}"
                ))),
                None => Err(OpsError::Message(format!(
                    "Project Session {project_id} cannot control {subject}"
                ))),
            };
        }
    }

    // Arms 2–4: wave, inconsistent managed session, or operator.
    let env_wave_id = std::env::var(WAVE_ID_ENV).ok();
    let resolved = resolve_managed_wave_name(Some(store), None, env_wave_id.as_deref()).await;

    match resolved {
        Ok(name) => match store.get_wave_by_name(&name).await {
            Ok(Some(row)) if row.id() == owning_wave_id => {
                Ok(CallerAuthority::Wave(owning_wave_id.clone()))
            }
            Ok(Some(_)) => {
                let owning_name = owning_wave_name(store, owning_wave_id).await;
                Err(OpsError::Message(format!(
                    "Wave {name} cannot control {subject} owned by Wave {owning_name}"
                )))
            }
            Ok(None) => Err(OpsError::Message(format!(
                "ambient wave '{name}' is not registered on this machine; \
                 the context is stale — re-register the wave or fix {WAVE_ID_ENV}"
            ))),
            Err(error) => Err(OpsError::Message(format!(
                "failed to read wave registry: {error}"
            ))),
        },
        Err(WaveResolveError::NoContext) => match stray_managed_marker() {
            Some(marker) => Err(OpsError::Message(format!(
                "{subject}: this command runs inside a managed session ({marker} is set) \
                 but carries no resolvable Wave or Project authority. Re-run with the \
                 session identity intact, or from a clean operator shell."
            ))),
            None => Ok(CallerAuthority::Operator),
        },
        Err(error) => Err(OpsError::Message(error.to_string())),
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
            Self {
                _lock: lock,
                previous,
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
        }
    }

    /// The classification matrix: Task and Project controls share
    /// `resolve_caller_authority`, so this single test proves identical
    /// classification across every ambient context. Real store rows, no network.
    #[tokio::test]
    async fn caller_authority_classifies_every_ambient_context() {
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
        let owning_id = owning.id().clone();
        let subject = "Task INF-123";
        // A Task target with no parent Project route supplied: these cases
        // exercise the wave/operator/fail-closed arms, not the Project arm.
        let target = ChildRef::Task(crate::task::TaskSessionId::new());
        let resolve = |store: &SharedStore, owning: &WaveId| {
            let store = store.clone();
            let owning = owning.clone();
            let target = target.clone();
            async move { resolve_caller_authority(&store, &owning, &target, None, subject).await }
        };

        // 1. Registered name (== owning) → Wave
        std::env::set_var(WAVE_ID_ENV, "infrastructure");
        assert_eq!(
            resolve(&store, &owning_id).await.unwrap(),
            CallerAuthority::Wave(owning_id.clone())
        );

        // 2. Registered UUID (== owning) → Wave
        std::env::set_var(WAVE_ID_ENV, owning_id.as_str());
        assert_eq!(
            resolve(&store, &owning_id).await.unwrap(),
            CallerAuthority::Wave(owning_id.clone())
        );

        // 3. Stale UUID (valid UUID, not in registry) → loud error
        std::env::set_var(WAVE_ID_ENV, WaveId::new().as_str());
        let msg = resolve(&store, &owning_id).await.unwrap_err().to_string();
        assert!(
            msg.contains("stale"),
            "stale UUID should mention stale: {msg}"
        );

        // 4. Stale name (not registered) → loud error, distinct from foreign
        std::env::set_var(WAVE_ID_ENV, "ghost-wave");
        let msg = resolve(&store, &owning_id).await.unwrap_err().to_string();
        assert!(
            msg.contains("not registered"),
            "stale name should mention not registered: {msg}"
        );

        // 5. No managed marker at all → Operator (was Human under the old rule).
        std::env::remove_var(WAVE_ID_ENV);
        assert_eq!(
            resolve(&store, &owning_id).await.unwrap(),
            CallerAuthority::Operator
        );

        // 6. A stray managed marker with no resolvable wave → fail closed. This
        //    is the ENG-19 escape (`env -u LF_WAVE_ID` from a wave body leaves
        //    LF_CHANNEL): it must refuse, never downgrade to Operator.
        std::env::set_var("LF_CHANNEL", "infrastructure");
        let msg = resolve(&store, &owning_id).await.unwrap_err().to_string();
        assert!(
            msg.contains("managed session") && msg.contains("LF_CHANNEL"),
            "stray marker should fail closed naming the marker: {msg}"
        );
        std::env::remove_var("LF_CHANNEL");

        // 7. Foreign registered wave (by name) → "cannot control" error
        std::env::set_var(WAVE_ID_ENV, "product");
        let msg = resolve(&store, &owning_id).await.unwrap_err().to_string();
        assert!(
            msg.contains("cannot control"),
            "foreign wave should mention cannot control: {msg}"
        );

        // 8. Foreign registered wave (by UUID) → "cannot control" error
        std::env::set_var(WAVE_ID_ENV, foreign.id().as_str());
        let msg = resolve(&store, &owning_id).await.unwrap_err().to_string();
        assert!(
            msg.contains("cannot control"),
            "foreign wave UUID should mention cannot control: {msg}"
        );
    }
}
