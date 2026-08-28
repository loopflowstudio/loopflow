//! Wave registry identity and child-observation delivery.
//!
//! `lf wave <name>` ensures the Wave has a durable row, then drains its durable
//! promotion occurrence and typed Project/Task observations. Listener presence
//! and one-brain enforcement live in the Wave's endpoint file; there is no
//! global process registry.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::RwLock;

use crate::controller::wave::runtime::WaveRuntime;
use crate::id::WaveId;
use crate::store::{open_existing_store, SharedStore, Store, StoreResult};
use crate::work::task::TaskObservation;
use crate::work::wave::{Wave, WaveLocator};

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
pub async fn ensure_wave_row(store: &Store, main_repo: &Path, name: &str) -> StoreResult<Wave> {
    let locator = WaveLocator::discover(main_repo, name)
        .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
    let existing = store.get_wave_at(&locator).await?;
    let is_new = existing.is_none();
    let wave = existing.unwrap_or_else(|| {
        Wave::new(
            WaveId::new(),
            locator.slug().to_string(),
            locator.repo().to_string(),
        )
    });
    store.create_wave(&wave).await?;
    if is_new {
        tracing::info!(
            wave = name,
            wave_id = %wave.id(),
            "wave was not in the registry; created its row"
        );
    }
    Ok(wave)
}

/// Ensure a Wave row with the identity chosen by its authoritative origin.
/// A remote Home may observe a different repo path, but never mint a second id
/// for the same named Work.
pub async fn ensure_wave_row_with_id(
    store: &Store,
    main_repo: &Path,
    name: &str,
    wave_id: &WaveId,
) -> StoreResult<Wave> {
    let locator = WaveLocator::discover(main_repo, name)
        .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
    if let Some(existing) = store.get_wave_at(&locator).await? {
        if existing.id() != wave_id {
            return Err(crate::store::StoreError::InvalidData(format!(
                "Wave '{name}' is {} on this Home, not authoritative id {wave_id}",
                existing.id()
            )));
        }
        return Ok(existing);
    }
    if let Some(existing) = store.get_wave(wave_id).await? {
        return Err(crate::store::StoreError::InvalidData(format!(
            "Wave id {wave_id} belongs to '{}', not '{name}'",
            existing.name()
        )));
    }
    let wave = Wave::new(
        wave_id.clone(),
        locator.slug().to_string(),
        locator.repo().to_string(),
    );
    store.create_wave(&wave).await?;
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

/// The listener's single late-installable observer reference.
///
/// A Wave listener may outlive the absence of the machine registry. Both its
/// heartbeat and request-time freshness checks use this slot, so the first
/// successful registry open installs exactly one observer without restarting
/// the listener.
pub(crate) struct ObserverSlot {
    runtime: Arc<WaveRuntime>,
    main_repo: PathBuf,
    wave: String,
    observer: RwLock<Option<Arc<StoreObserver>>>,
}

impl fmt::Debug for ObserverSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObserverSlot")
            .field("main_repo", &self.main_repo)
            .field("wave", &self.wave)
            .finish_non_exhaustive()
    }
}

impl ObserverSlot {
    pub(crate) fn new(runtime: Arc<WaveRuntime>, observer: Option<Arc<StoreObserver>>) -> Self {
        Self {
            main_repo: runtime.repo_root().to_path_buf(),
            wave: runtime.name().to_string(),
            runtime,
            observer: RwLock::new(observer),
        }
    }

    /// Return the installed observer, acquiring the registry if it appeared
    /// after listener boot.
    pub(crate) async fn acquire(&self) -> Option<Arc<StoreObserver>> {
        if let Some(observer) = self.observer.read().await.as_ref() {
            return Some(Arc::clone(observer));
        }

        let store: SharedStore = Arc::new(open_existing_store().await?);
        let wave = ensure_wave_row(&store, &self.main_repo, &self.wave)
            .await
            .map_err(|error| {
                tracing::debug!(wave = self.wave, %error, "late registry acquisition failed")
            })
            .ok()?;
        let candidate = Arc::new(StoreObserver::new(
            Arc::clone(&self.runtime),
            store,
            wave.id().clone(),
        ));
        let mut installed = self.observer.write().await;
        if let Some(observer) = installed.as_ref() {
            return Some(Arc::clone(observer));
        }
        tracing::info!(
            wave = self.wave,
            "Wave listener acquired the local registry"
        );
        *installed = Some(Arc::clone(&candidate));
        Some(candidate)
    }

    pub(crate) async fn poll_once(&self) {
        if let Some(observer) = self.acquire().await {
            observer.poll_once().await;
        }
    }

    /// Poll forever on `cadence`. Runs until aborted at server shutdown.
    pub(crate) async fn run(self: Arc<Self>, cadence: Duration) {
        loop {
            self.poll_once().await;
            tokio::time::sleep(cadence).await;
        }
    }
}

/// Polls durable typed input for this Wave.
///
/// Project and Task lifecycle owners reconcile their own process liveness.
/// This observer carries their typed events and the Wave row's one-time
/// promotion occurrence into the Wave journal.
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

    /// Deliver the durable promotion occurrence and every pending typed child
    /// observation. Store errors are logged and retried on the next poll.
    pub async fn poll_once(&self) {
        self.poll_promotion().await;
        self.poll_child_observations().await;
    }

    /// Verify an HTTP latency nudge against the durable promotion and registry
    /// parentage, then deliver its typed wake idempotently. The request string
    /// identifies no occurrence and grants no authority.
    pub async fn deliver_promotion(&self, expected_parent: &str) -> StoreResult<bool> {
        let wake = self.durable_promotion().await?.ok_or_else(|| {
            crate::store::StoreError::InvalidData(format!(
                "Wave {} has no durable promotion occurrence",
                self.wave_id
            ))
        })?;
        if wake.parent != expected_parent {
            return Err(crate::store::StoreError::InvalidData(format!(
                "Wave {} belongs to '{}', not expected parent '{expected_parent}'",
                self.wave_id, wake.parent
            )));
        }
        Ok(self.runtime.deliver_promotion_wake(wake))
    }

    async fn durable_promotion(&self) -> StoreResult<Option<crate::work::wave::PromotionWake>> {
        let wave = self.store.get_wave(&self.wave_id).await?.ok_or_else(|| {
            crate::store::StoreError::InvalidData(format!(
                "Wave {} disappeared from the registry",
                self.wave_id
            ))
        })?;
        if wave.promoted_at().is_none() {
            return Ok(None);
        }
        let parent_wave_id = wave.parent_wave_id().cloned().ok_or_else(|| {
            crate::store::StoreError::InvalidData(format!(
                "Wave '{}' records promotion without a parent",
                wave.name()
            ))
        })?;
        let parent = self.store.get_wave(&parent_wave_id).await?.ok_or_else(|| {
            crate::store::StoreError::InvalidData(format!(
                "promotion parent {parent_wave_id} is absent"
            ))
        })?;
        Ok(Some(crate::work::wave::PromotionWake {
            parent_wave_id,
            parent: parent.name().to_string(),
        }))
    }

    async fn poll_promotion(&self) {
        match self.durable_promotion().await {
            Ok(Some(wake)) => {
                self.runtime.deliver_promotion_wake(wake);
            }
            Ok(None) => {}
            Err(error) => {
                tracing::debug!(%error, "wave observer promotion read failed");
            }
        }
    }

    async fn poll_child_observations(&self) {
        let recipient = crate::child::ObservationRecipient::Wave {
            wave_id: self.wave_id.clone(),
        };
        let observations = match self.store.pending_observations(&recipient).await {
            Ok(observations) => observations,
            Err(error) => {
                tracing::debug!(%error, "wave observer child outbox read failed");
                return;
            }
        };
        for observation in observations {
            let should_ack = match (observation.source, observation.payload) {
                (
                    crate::child::ChildRef::Task(task_id),
                    crate::work::project::ChildEventPayload::Task { event },
                ) => match self.store.get_task(&task_id).await {
                    Ok(Some(task)) => {
                        self.runtime.deliver_task_observation(TaskObservation {
                            task_id,
                            issue_identifier: task.plan.identifier,
                            event_id: observation.event_id,
                            event,
                        });
                        true
                    }
                    Ok(None) => false,
                    Err(error) => {
                        tracing::debug!(%error, %task_id, "wave observer Task read failed");
                        continue;
                    }
                },
                (
                    crate::child::ChildRef::Project(project_id),
                    crate::work::project::ChildEventPayload::Project { event },
                ) => match self.store.get_project(&project_id).await {
                    Ok(Some(project)) => {
                        self.runtime.deliver_project_observation(
                            crate::work::project::ProjectObservation {
                                project_id,
                                project: project.plan.slug,
                                event_id: observation.event_id,
                                event,
                            },
                        );
                        true
                    }
                    Ok(None) => false,
                    Err(error) => {
                        tracing::debug!(%error, %project_id, "wave observer Project read failed");
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
    use crate::chat::types::Lifecycle;
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
            .get_wave_at(&WaveLocator::discover(&repo, "ship").unwrap())
            .await
            .expect("lookup")
            .expect("row exists");
        assert_eq!(stored.id(), wave.id());
        assert_eq!(
            stored.repo(),
            WaveLocator::discover(&repo, "ship")
                .unwrap()
                .repo()
                .to_string()
        );

        let again = ensure_wave_row(&store, &repo, "ship")
            .await
            .expect("idempotent");
        assert_eq!(again.id(), wave.id(), "ensure reuses the existing row");
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

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env guard serializes the shared registry path
    async fn observerless_listener_acquires_late_registry_and_delivers_once() {
        let _env = crate::journal::TestLedgerGuard::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime =
            WaveRuntime::open("ship".to_string(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = ObserverSlot::new(runtime.clone(), None);

        observer.poll_once().await;
        assert_eq!(promotion_event_count(tmp.path()), 0);

        let store: SharedStore = Arc::new(
            open_store(&crate::store::storage_config_from_env().expect("store config"))
                .await
                .expect("create registry"),
        );
        let parent = Wave::new(
            WaveId::new(),
            "platform".to_string(),
            tmp.path().display().to_string(),
        );
        let mut child = Wave::new(
            WaveId::new(),
            "ship".to_string(),
            tmp.path().display().to_string(),
        );
        child
            .record_promotion(parent.id(), time::OffsetDateTime::now_utc())
            .expect("record promotion");
        store.create_wave(&parent).await.expect("store parent");
        store.create_wave(&child).await.expect("store child");

        observer.poll_once().await;
        observer.poll_once().await;

        assert_eq!(promotion_event_count(tmp.path()), 1);
        assert_eq!(runtime.pending_messages().len(), 1);
    }

    fn promotion_event_count(repo: &std::path::Path) -> usize {
        let (_, events) = crate::controller::wave::journal::Journal::open(
            &crate::controller::wave::journal::journal_path(repo, "ship"),
        )
        .expect("read Wave journal");
        events
            .iter()
            .filter(|event| {
                matches!(
                    &event.kind,
                    crate::controller::wave::journal::EventKind::PromotionObserved { .. }
                )
            })
            .count()
    }

    #[tokio::test]
    async fn parent_link_without_occurrence_never_promotes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let parent = Wave::new(
            WaveId::new(),
            "platform".to_string(),
            tmp.path().display().to_string(),
        );
        let child = Wave::new(
            WaveId::new(),
            "ship".to_string(),
            tmp.path().display().to_string(),
        )
        .with_parent(parent.id().clone());
        store.create_wave(&parent).await.expect("store parent");
        store.create_wave(&child).await.expect("store child");
        let runtime =
            WaveRuntime::open("ship".to_string(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = StoreObserver::new(runtime, store, child.id().clone());

        observer.poll_once().await;
        assert!(
            observer.deliver_promotion("platform").await.is_err(),
            "an HTTP request cannot turn ancestry into a promotion occurrence"
        );
        assert_eq!(promotion_event_count(tmp.path()), 0);
    }

    #[tokio::test]
    async fn polling_recovers_recorded_promotion_once_across_reopen() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let parent = Wave::new(
            WaveId::new(),
            "platform".to_string(),
            tmp.path().display().to_string(),
        );
        let mut child = Wave::new(
            WaveId::new(),
            "ship".to_string(),
            tmp.path().display().to_string(),
        )
        .with_parent(parent.id().clone());
        child
            .record_promotion(parent.id(), time::OffsetDateTime::now_utc())
            .expect("record promotion");
        store.create_wave(&parent).await.expect("store parent");
        store.create_wave(&child).await.expect("store child");

        let runtime =
            WaveRuntime::open("ship".to_string(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = StoreObserver::new(runtime.clone(), store.clone(), child.id().clone());

        // No HTTP request: the heartbeat reconstructs the wake from registry truth.
        observer.poll_once().await;
        observer.poll_once().await;
        assert_eq!(promotion_event_count(tmp.path()), 1);
        assert_eq!(runtime.pending_messages().len(), 1);

        let id = format!("promotion:{}", parent.id());
        runtime.apply_resident_delta(crate::controller::wave::wire::ResidentDelta::TurnOpened {
            answers: vec![id],
        });
        runtime.apply_resident_delta(crate::controller::wave::wire::ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            reason: None,
        });
        assert!(runtime.pending_messages().is_empty());
        drop(observer);
        drop(runtime);

        let reopened = WaveRuntime::open("ship".to_string(), tmp.path().to_path_buf())
            .expect("reopen runtime");
        let observer = StoreObserver::new(reopened.clone(), store, child.id().clone());
        observer.poll_once().await;

        assert!(reopened.pending_messages().is_empty());
        assert_eq!(promotion_event_count(tmp.path()), 1);
    }

    #[tokio::test]
    async fn explicit_promotion_nudge_verifies_registry_parent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let parent = Wave::new(
            WaveId::new(),
            "platform".to_string(),
            tmp.path().display().to_string(),
        );
        let mut child = Wave::new(
            WaveId::new(),
            "ship".to_string(),
            tmp.path().display().to_string(),
        )
        .with_parent(parent.id().clone());
        child
            .record_promotion(parent.id(), time::OffsetDateTime::now_utc())
            .expect("record promotion");
        store.create_wave(&parent).await.expect("store parent");
        store.create_wave(&child).await.expect("store child");
        let runtime =
            WaveRuntime::open("ship".to_string(), tmp.path().to_path_buf()).expect("open runtime");
        let observer = StoreObserver::new(runtime, store, child.id().clone());

        let error = observer
            .deliver_promotion("wrong-parent")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not expected parent"));
        assert_eq!(promotion_event_count(tmp.path()), 0);
        assert!(observer.deliver_promotion("platform").await.unwrap());
        assert!(!observer.deliver_promotion("platform").await.unwrap());

        let (_, events) = crate::controller::wave::journal::Journal::open(
            &crate::controller::wave::journal::journal_path(tmp.path(), "ship"),
        )
        .expect("read journal");
        assert_eq!(promotion_event_count(tmp.path()), 1);
        assert!(!events.iter().any(|event| matches!(
            &event.kind,
            crate::controller::wave::journal::EventKind::UserMessage { .. }
        )));
    }
}
