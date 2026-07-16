use std::process::{Command, Output};

use crate::child_session::ChildCommandSource;
use crate::engine::wave_context::{resolve_managed_wave_name, WaveResolveError, WAVE_ID_ENV};
use crate::id::WaveId;
use crate::ops::error::{OpsError, OpsResult};
use crate::store::SharedStore;

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

/// Normalize an explicitly selected Wave name for workflow operations.
pub fn resolve_wave_name(explicit: Option<&str>) -> Option<String> {
    explicit.and_then(normalize_wave_name)
}

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

/// Resolve the ambient Wave context into a [`ChildCommandSource`] for Task and
/// Project control commands. Both controls call this single helper so they
/// cannot drift on identity classification.
///
/// Classification:
/// - `Ok(name)` matching the owning wave → `Wave`
/// - `Ok(name)` matching a different registered wave → "cannot control" error
/// - `Ok(name)` not registered → "stale context" error
/// - `NoContext` → `Human`
/// - `StaleIdentity` / `Registry` / `UnknownExplicit` → loud error
///
/// `Human` is returned **only** on `NoContext`; every other outcome is a loud
/// error. This preserves the invariant that a foreign or stale wave must never
/// be reclassified as a human command.
pub(crate) async fn resolve_child_command_source(
    store: &SharedStore,
    owning_wave_id: &WaveId,
    subject: &str,
) -> OpsResult<ChildCommandSource> {
    let env_wave_id = std::env::var(WAVE_ID_ENV).ok();
    let resolved = resolve_managed_wave_name(Some(store), None, env_wave_id.as_deref()).await;

    match resolved {
        Ok(name) => match store.get_wave_by_name(&name).await {
            Ok(Some(row)) if row.id() == owning_wave_id => {
                Ok(ChildCommandSource::Wave(owning_wave_id.clone()))
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
        Err(WaveResolveError::NoContext) => Ok(ChildCommandSource::Human),
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

    /// The classification matrix: both Task and Project controls call
    /// `resolve_child_command_source`, so this single test proves identical
    /// classification across every ambient context. Real store rows, no network.
    #[tokio::test]
    async fn command_source_classifies_every_ambient_context() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var(WAVE_ID_ENV).ok();
        let restore = || match &previous {
            Some(value) => std::env::set_var(WAVE_ID_ENV, value),
            None => std::env::remove_var(WAVE_ID_ENV),
        };

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

        // 1. Registered name (== owning) → Wave
        std::env::set_var(WAVE_ID_ENV, "infrastructure");
        let result = resolve_child_command_source(&store, &owning_id, subject).await;
        assert_eq!(result.unwrap(), ChildCommandSource::Wave(owning_id.clone()));

        // 2. Registered UUID (== owning) → Wave
        std::env::set_var(WAVE_ID_ENV, owning_id.as_str());
        let result = resolve_child_command_source(&store, &owning_id, subject).await;
        assert_eq!(result.unwrap(), ChildCommandSource::Wave(owning_id.clone()));

        // 3. Stale UUID (valid UUID, not in registry) → loud error
        let stale_uuid = WaveId::new();
        std::env::set_var(WAVE_ID_ENV, stale_uuid.as_str());
        let result = resolve_child_command_source(&store, &owning_id, subject).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("stale"),
            "stale UUID error should mention stale: {msg}"
        );

        // 4. Stale name (not registered) → loud error, distinct from foreign
        std::env::set_var(WAVE_ID_ENV, "ghost-wave");
        let result = resolve_child_command_source(&store, &owning_id, subject).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not registered"),
            "stale name error should mention not registered: {msg}"
        );

        // 5. Absent context → Human
        std::env::remove_var(WAVE_ID_ENV);
        let result = resolve_child_command_source(&store, &owning_id, subject).await;
        assert_eq!(result.unwrap(), ChildCommandSource::Human);

        // 6. Foreign registered wave (by name) → "cannot control" error
        std::env::set_var(WAVE_ID_ENV, "product");
        let result = resolve_child_command_source(&store, &owning_id, subject).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("cannot control"),
            "foreign wave error should mention cannot control: {msg}"
        );

        // 7. Foreign registered wave (by UUID) → "cannot control" error
        std::env::set_var(WAVE_ID_ENV, foreign.id().as_str());
        let result = resolve_child_command_source(&store, &owning_id, subject).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("cannot control"),
            "foreign wave UUID error should mention cannot control: {msg}"
        );

        restore();
    }
}
