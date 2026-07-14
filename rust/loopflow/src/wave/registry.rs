//! Wave registry identity and child-observation delivery.
//!
//! `lf wave <name>` ensures the Wave has a durable row, then drains typed
//! Project and Task observations addressed to it. Listener presence and
//! one-brain enforcement live in the Wave's endpoint file; there is no global
//! process-session registry.

use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;

use crate::id::WaveId;
use crate::store::{SharedStore, StoreResult};
use crate::task::TaskObservation;
use crate::wave::runtime::WaveRuntime;
use crate::wave::Wave;

/// How often the observer re-reads the store between turns. Modest by
/// design: the loop also refreshes right before every turn it takes.
pub const POLL_CADENCE: Duration = Duration::from_secs(10);

/// Store state needed by a Wave listener.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub store: SharedStore,
    pub wave: Wave,
}

/// The wave's registry row, created if the store has never seen this wave.
///
/// The db IS the registry: a reachable store with no row for the wave must
/// not degrade to running unregistered (observed live — two brains on one
/// wave because boot skipped registration entirely). The created row is
/// minimal. Authored Wave policy remains in `GOAL.md`; the registry stores no
/// launch-policy cache.
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
    let wave = existing.unwrap_or_else(|| {
        Wave::new(
            WaveId::new(),
            name.to_string(),
            main_repo.display().to_string(),
        )
    });
    store.create_wave(&wave).await?;
    if is_new {
        tracing::info!(
            wave = name,
            wave_id = %wave.id,
            "wave was not in the registry; created its row"
        );
    }
    Ok(wave)
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

// -- Observation ---------------------------------------------------------

/// Polls the durable child-observation outbox for this wave.
///
/// Project and Task lifecycle owners reconcile their own process liveness.
/// This observer has one job: carry their typed events into the Wave journal.
pub struct StoreObserver {
    runtime: Arc<WaveRuntime>,
    store: SharedStore,
    wave_id: WaveId,
}

impl fmt::Debug for StoreObserver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoreObserver")
            .field("wave_id", &self.wave_id)
            .finish()
    }
}

impl StoreObserver {
    pub fn new(runtime: Arc<WaveRuntime>, store: SharedStore, wave_id: WaveId) -> Self {
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
                        let control_source = self.task_control_source(&session_id, &event).await;
                        self.runtime.deliver_task_observation(TaskObservation {
                            session_id,
                            issue_identifier: session.launch.issue.identifier,
                            event_id: observation.event_id,
                            control_source,
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
                        let control_source = self.project_control_source(&session_id, &event).await;
                        self.runtime.deliver_project_observation(
                            crate::project_session::ProjectObservation {
                                session_id,
                                project: session.launch.project.slug,
                                event_id: observation.event_id,
                                control_source,
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

    async fn task_control_source(
        &self,
        session_id: &crate::task::TaskSessionId,
        event: &crate::task::TaskEventKind,
    ) -> Option<crate::child_session::ChildCommandSource> {
        match event {
            crate::task::TaskEventKind::CommandChanged { command_id, .. } => self
                .store
                .get_child_command(command_id)
                .await
                .ok()
                .flatten()
                .map(|command| command.source),
            crate::task::TaskEventKind::DirectiveChanged { directive_id, .. }
            | crate::task::TaskEventKind::DirectiveIncorporated { directive_id, .. } => self
                .store
                .child_directives(&crate::child_session::ChildRef::Task(session_id.clone()))
                .await
                .ok()?
                .into_iter()
                .find(|directive| &directive.id == directive_id)
                .map(|directive| directive.source),
            _ => None,
        }
    }

    async fn project_control_source(
        &self,
        session_id: &crate::project_session::ProjectSessionId,
        event: &crate::project_session::ProjectEventKind,
    ) -> Option<crate::child_session::ChildCommandSource> {
        match event {
            crate::project_session::ProjectEventKind::CommandChanged { command_id, .. } => self
                .store
                .get_child_command(command_id)
                .await
                .ok()
                .flatten()
                .map(|command| command.source),
            crate::project_session::ProjectEventKind::DirectiveChanged { directive_id, .. }
            | crate::project_session::ProjectEventKind::DirectiveIncorporated {
                directive_id,
                ..
            } => self
                .store
                .child_directives(&crate::child_session::ChildRef::Project(session_id.clone()))
                .await
                .ok()?
                .into_iter()
                .find(|directive| &directive.id == directive_id)
                .map(|directive| directive.source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{open_store, StorageConfig};

    async fn temp_store(tmp: &std::path::Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(tmp.join("loopflow.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    #[tokio::test]
    async fn boot_with_no_wave_row_creates_one_durable_identity() {
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
        assert_eq!(stored.repo(), repo.display().to_string());

        let again = ensure_wave_row(&store, &repo, "ship")
            .await
            .expect("idempotent");
        assert_eq!(again.id, wave.id, "ensure reuses the existing row");
    }

    /// No GOAL.md at all: the registry still creates the identity row.
    #[tokio::test]
    async fn ensure_wave_row_without_goal_md_uses_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = ensure_wave_row(&store, tmp.path(), "ship")
            .await
            .expect("row created");
        assert_eq!(wave.name(), "ship");
    }
}
