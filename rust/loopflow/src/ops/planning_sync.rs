//! Foreground Git exchange and local-save delivery for the selected transport.
//! Acquisition and delivery run independently; the common writer alone changes Work.
//! Blocking workers retain their locks through cancellation, commit and readback.

use crate::work::wave::Wave;
use std::fs::OpenOptions;
use std::future::Future;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::planning_exchange::PlanningSnapshot;
use crate::planning_git::{PlanningGit, PlanningPublication, PlanningRevision};
use crate::store::{open_store, sqlite::SqliteStore, StorageConfig, Store};

use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// Saved Tasks need no network before a read or local save. On a cache miss,
/// acquire the selected planning destinations once, without allocating execution.
pub(crate) async fn find_task(
    store: &Store,
    repo: &str,
    selector: &str,
) -> OpsResult<Option<crate::work::task::Task>> {
    let read = || {
        store
            .sqlite
            .resolve_task_id(selector, Some(repo))?
            .map(|id| store.sqlite.task(&id))
            .transpose()
            .map(Option::flatten)
    };
    if let Some(task) = read().map_err(message)? {
        return Ok(Some(task));
    }
    if let Err(error) = acquire_repository(store, repo).await {
        tracing::warn!(%error, "planning acquisition pending; resolving retained Task");
    }
    read().map_err(message)
}

/// One acquisition per destination, also used before resolving a cold Task.
/// An unavailable remote never prevents a saved local plan from being used.
pub(crate) async fn acquire_repository(store: &Store, repo: &str) -> OpsResult<()> {
    exchange_repository(store, repo, false).await
}

pub(crate) async fn publish_repository(store: &Store, repo: &str) -> OpsResult<()> {
    exchange_repository(store, repo, true).await
}

/// A short command saves first, then makes one foreground exchange attempt.
/// Failure leaves its successful local mutation and durable pending state intact.
pub(crate) async fn sync_after_save(store: &Store, repo: &str) {
    let attempt = async {
        let config = crate::config::load_config(Some(Path::new(repo)))
            .map_err(message)?
            .unwrap_or_default();
        match config.planning_transport() {
            crate::config::PlanningConfig::Git { .. } => publish_repository(store, repo).await,
            crate::config::PlanningConfig::Linear { .. }
            | crate::config::PlanningConfig::Asana { .. }
            | crate::config::PlanningConfig::Notion { .. } => {
                // Export attempts have individual deadlines. A shared deadline
                // lets older pending work starve the Task that was just saved.
                super::planning_export::sync_repository_exports(store, repo).await?;
                let (fields, deliveries) = tokio::join!(
                    super::planning_delivery::sync_repository_fields(store, repo),
                    super::linear_observe::sync_repository_deliveries(store, repo)
                );
                fields?;
                deliveries?;
                Ok(())
            }
        }
    };
    if let Err(error) = attempt.await {
        tracing::debug!(%error, "planning synchronization pending after local save");
        eprintln!("Saved locally; planning synchronization pending. See `lf planning status`.");
    }
}

/// Resolve the repository configuration before touching a planning remote.
/// A Linear outage cannot select Git: routing is configuration, not availability.
pub(crate) async fn configured_destination(store: &Store, repo: &str) -> OpsResult<Option<String>> {
    use crate::config::{load_config, GitPlan, PlanningConfig};
    let config = load_config(Some(Path::new(repo)))
        .map_err(message)?
        .unwrap_or_default();
    let PlanningConfig::Git { remote, plan } = config.planning_transport() else {
        return Ok(None);
    };
    let key = match plan {
        GitPlan::Shared(_) => None,
        GitPlan::Personal => Some(provisioned_user_key(store).await?),
    };
    let reference = plan
        .reference(key.as_deref())
        .expect("a personal plan has a provisioned user key");
    let destination =
        crate::planning_git::PlanningDestination::resolve(Path::new(repo), &remote, &reference)
            .map_err(message)?;
    let id = store
        .bind_peer_planning(repo, &destination)
        .await
        .map_err(message)?;
    Ok(Some(id))
}

/// The first personal connection creates the key; a concurrent creator's key wins.
async fn provisioned_user_key(store: &Store) -> OpsResult<String> {
    if let Some(key) = store.planning_user_key().await.map_err(message)? {
        return Ok(key);
    }
    let key = uuid::Uuid::new_v4().to_string();
    let result = store.provision_planning_user_key(&key).await;
    match store.planning_user_key().await.map_err(message)? {
        Some(saved) => Ok(saved),
        None => result.map(|()| key).map_err(message),
    }
}

/// Passive status uses configuration, never a persisted copy of its selection.
/// Retained routes remain readable in a Work folder without a Git checkout.
pub(crate) async fn peer_status(
    store: &Store,
    repo: &str,
) -> OpsResult<Vec<crate::store::PeerPlanningStatus>> {
    let Some(active) = active_destination(store, repo).await? else {
        return Ok(Vec::new());
    };
    store
        .peer_planning_status(repo, active.as_deref())
        .await
        .map_err(message)
}

/// `lf planning journal`: each destination's retained changes.
pub(crate) async fn peer_journal(
    store: &Store,
    repo: &str,
) -> OpsResult<Vec<crate::store::PlanningJournal>> {
    let Some(active) = active_destination(store, repo).await? else {
        return Ok(Vec::new());
    };
    store
        .peer_planning_journal(repo, active.as_deref())
        .await
        .map_err(message)
}

/// The Git destination this repository plans through: `None` when it does not
/// plan through Git, `Some(None)` outside a checkout.
async fn active_destination(store: &Store, repo: &str) -> OpsResult<Option<Option<String>>> {
    use crate::config::{load_config, GitPlan, PlanningConfig};
    let config = load_config(Some(Path::new(repo)))
        .map_err(message)?
        .unwrap_or_default();
    let PlanningConfig::Git { remote, plan } = config.planning_transport() else {
        return Ok(None);
    };
    let key = match plan {
        GitPlan::Shared(_) => None,
        GitPlan::Personal => store.planning_user_key().await.map_err(message)?,
    };
    let reference = plan.reference(key.as_deref());
    let active = if crate::repo::discover_repo_root(Path::new(repo))
        .map_err(message)?
        .is_some()
    {
        reference
            .map(|reference| {
                crate::planning_git::PlanningDestination::resolve(
                    Path::new(repo),
                    &remote,
                    &reference,
                )
                .map(|destination| destination.id())
                .map_err(message)
            })
            .transpose()?
    } else {
        None
    };
    Ok(Some(active))
}

/// A World build is this process's only writer to a remote nobody else reads,
/// so its saves stay local and [`crate::world::World::finish`] publishes once.
/// A Git exchange per save made a 150-Task world cost minutes.
static EXCHANGE_DEFERRED: AtomicBool = AtomicBool::new(false);

pub(crate) fn defer_exchange(deferred: bool) {
    EXCHANGE_DEFERRED.store(deferred, Ordering::SeqCst);
}

async fn exchange_repository(store: &Store, repo: &str, publish: bool) -> OpsResult<()> {
    if EXCHANGE_DEFERRED.load(Ordering::SeqCst) {
        return Ok(());
    }
    let Some(destination) = configured_destination(store, repo).await? else {
        return Ok(());
    };
    let sqlite = store.sqlite.clone();
    let repo = repo.to_owned();
    tokio::task::spawn_blocking(move || exchange_destination(&sqlite, &repo, &destination, publish))
        .await
        .map_err(message)?
}

fn exchange_destination(store: &SqliteStore, repo: &str, id: &str, publish: bool) -> OpsResult<()> {
    // The blocking worker retains effect ownership through status writes, even
    // if its async waiter is canceled. Acquisition never takes this lock.
    let _lock = if publish {
        let path = store
            .home_dir()
            .map_err(message)?
            .join("locks/planning-peers")
            .join(format!("{id}.lock"));
        let Some(lock) = super::planning_delivery::lock_delivery(&path)? else {
            return Ok(());
        };
        Some(lock)
    } else {
        None
    };
    let result = (|| {
        let binding = store
            .peer_planning_destination(repo, id)
            .map_err(message)?
            .ok_or_else(|| message("planning destination is missing"))?;
        let git = PlanningGit::new(Path::new(repo), &binding).map_err(message)?;
        if publish {
            store.recover_task_filings(repo).map_err(message)?;
            publish_destination(store, repo, id, &git)
        } else {
            acquire_destination(store, repo, id, &git).map(|_| ())
        }
    })();
    // Do not persist provider data or credential-bearing destinations in errors.
    let error = result.as_ref().err().map(|_| {
        if publish {
            "Git publication pending; local planning retained"
        } else {
            "Git acquisition failed; retained import unchanged"
        }
    });
    store
        .record_peer_error(repo, id, !publish, error)
        .map_err(message)?;
    result
}

fn acquire_destination(
    store: &SqliteStore,
    repo: &str,
    destination: &str,
    git: &PlanningGit,
) -> OpsResult<Option<(PlanningRevision, PlanningSnapshot)>> {
    let remote = git.fetch().map_err(message)?;
    store
        .record_peer_fetch(
            repo,
            destination,
            remote.as_ref().map(|document| document.revision.as_str()),
        )
        .map_err(message)?;
    let imported = if let Some(document) = remote {
        let snapshot = PlanningSnapshot::from_bytes(&document.bytes).map_err(message)?;
        store
            .import_peer_planning(repo, destination, document.revision.as_str(), &snapshot)
            .map_err(message)?;
        Some((document.revision, snapshot))
    } else {
        None
    };
    store
        .record_peer_error(repo, destination, true, None)
        .map_err(message)?;
    Ok(imported)
}

fn publish_destination(
    store: &SqliteStore,
    repo: &str,
    destination: &str,
    git: &PlanningGit,
) -> OpsResult<()> {
    // Always acquire before deciding what to publish, including recovery from a
    // lost push response. Never publish from a stale local Git document alone.
    let (remote, remote_snapshot) = match acquire_destination(store, repo, destination, git)? {
        Some((revision, snapshot)) => (Some(revision), snapshot),
        None => (None, PlanningSnapshot::default()),
    };
    let exported = store
        .export_peer_planning(repo, destination)
        .map_err(message)?;
    // Omitted/held objects retain their last shared history, not private local
    // moves. Only today's selected export may add local mutations to the remote.
    let merged = remote_snapshot.merge(&exported).map_err(message)?;
    if let Some(remote) = &remote {
        if merged == remote_snapshot {
            return store
                .record_peer_publication(repo, destination, remote.as_str(), "confirmed", &exported)
                .map_err(message);
        }
    } else if merged.changes.is_empty() {
        return Ok(());
    }
    let bytes = merged.to_bytes().map_err(message)?;
    let local = git.local().map_err(message)?;
    let reusable = match (&local, &remote) {
        (Some(local), Some(remote)) => {
            local.bytes == bytes && git.is_ancestor(remote, &local.revision).map_err(message)?
        }
        (Some(local), None) => local.bytes == bytes,
        _ => false,
    };
    let saved = if reusable {
        local.expect("reusable revision is present")
    } else {
        git.save(&bytes, local.as_ref().map(|d| &d.revision), remote.as_ref())
            .map_err(message)?
    };
    // Persist uncertainty before the effect. A crash at any later point recovers
    // by fetching first, not by assuming that a failed response means no push.
    store
        .record_peer_publication(
            repo,
            destination,
            saved.revision.as_str(),
            "unconfirmed",
            &exported,
        )
        .map_err(message)?;
    let state = match git.publish(&saved.revision).map_err(message)? {
        PlanningPublication::Confirmed => "confirmed",
        PlanningPublication::Pending => "pending",
        PlanningPublication::Unconfirmed => "unconfirmed",
    };
    store
        .record_peer_publication(repo, destination, saved.revision.as_str(), state, &exported)
        .map_err(message)
}

/// Provider exit is also a useful boundary for Taskless conversations.
pub(crate) fn check_directory(directory: &Path) -> OpsResult<()> {
    sync_directory(directory, false)
}

/// `wave status --sync`'s exchange, before it refreshes the Wave inventory.
pub(crate) fn refresh_directory(directory: &Path) -> OpsResult<()> {
    sync_directory(directory, true)
}

fn sync_directory(directory: &Path, refresh: bool) -> OpsResult<()> {
    let repo = crate::repository::CanonicalRepo::discover(directory)
        .map_err(message)?
        .to_string();
    if !super::planning_client::connected(&repo) {
        return Ok(());
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let store = planning_store().await?;
            if refresh {
                super::linear_observe::refresh_repository(&store, &repo).await;
            } else {
                super::linear_observe::sync_repository(&store, &repo).await;
            }
            Ok(())
        })
}

/// Foreground Git exchange while a provider runs.
/// Dropping the connection cancels requests; durable identities survive cancellation.
#[derive(Debug)]
pub(crate) struct PlanningSync {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PlanningSync {
    /// Repository planning follows every foreground provider, not Task attribution.
    /// One thread owns initialization and exchange. Callers may already be inside
    /// an async runtime; startup errors still return before the provider launches.
    pub(crate) fn start_for_directory(directory: &Path) -> OpsResult<Self> {
        let repo = crate::repository::CanonicalRepo::discover(directory)
            .map_err(message)?
            .to_string();
        if super::planning_client::connected(&repo) {
            return Ok(Self {
                stop: None,
                thread: None,
            });
        }
        let config = storage_config_from_env()?;
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let (stop, stopped) = tokio::sync::oneshot::channel();
        #[cfg(test)]
        let context = super::linear::LINEAR_TEST_CONTEXT
            .try_with(Clone::clone)
            .ok();
        let thread = std::thread::Builder::new()
            .name("planning-sync".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready.send(Err(message(error)));
                        return;
                    }
                };
                let drive = async {
                    let store = match open_planning_store(&config).await.map(Arc::new) {
                        Ok(store) => store,
                        Err(error) => {
                            let _ = ready.send(Err(error));
                            return;
                        }
                    };
                    if ready.send(Ok(())).is_err() {
                        return;
                    }
                    tokio::select! {
                        _ = stopped => {},
                        _ = repeat_sync("Git planning acquisition", Duration::from_secs(5), || {
                            acquire_repository(&store, &repo)
                        }) => {},
                        _ = repeat_sync("Git planning publication", Duration::from_secs(1), || {
                            publish_repository(&store, &repo)
                        }) => {},
                    }
                };
                #[cfg(test)]
                if let Some(context) = context {
                    runtime.block_on(super::linear::LINEAR_TEST_CONTEXT.scope(context, drive));
                    return;
                }
                runtime.block_on(drive);
            })?;
        let connection = Self {
            stop: Some(stop),
            thread: Some(thread),
        };
        started.recv().map_err(message)??;
        Ok(connection)
    }
}

// Each branch has its own delay after an attempt. A slow delivery cannot
// postpone acquisition or cause missed ticks to burst when the request returns.
async fn repeat_sync<F: Future<Output = OpsResult<()>>>(
    operation: &str,
    delay: Duration,
    mut attempt: impl FnMut() -> F,
) {
    loop {
        if let Err(error) = attempt().await {
            tracing::debug!(%error, operation, "planning synchronization pending");
        }
        tokio::time::sleep(delay).await;
    }
}

impl Drop for PlanningSync {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn storage_config_from_env() -> OpsResult<crate::store::StorageConfig> {
    #[cfg(test)]
    if let Ok(config) =
        super::linear::LINEAR_TEST_CONTEXT.try_with(|ctx| StorageConfig::sqlite(ctx.path.clone()))
    {
        return Ok(config);
    }
    crate::store::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve planning store: {err}")))
}

pub(crate) async fn planning_store() -> OpsResult<Store> {
    open_planning_store(&storage_config_from_env()?).await
}

pub(super) async fn open_planning_store(config: &StorageConfig) -> OpsResult<Store> {
    #[cfg(test)]
    if let Ok(store) = super::linear::LINEAR_TEST_CONTEXT
        .try_with(|ctx| Store::from_sqlite_for_test(ctx.store.sqlite.clone()))
    {
        return Ok(store);
    }
    let config = config.clone();
    // Store opening uses synchronous SQLite I/O. Keep it off the deadline's executor.
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(open_store(&config))
    })
    .await
    .map_err(|_| message("Could not open the planning store"))?
    .map_err(|err| OpsError::Message(format!("failed to open planning store: {err}")))
}

pub(crate) async fn lock_wave_planning(wave: &Wave) -> OpsResult<Arc<crate::store::PlanningLocks>> {
    // Keep the existing lock inode namespace shared with already-running callers.
    let path = crate::store::lf_home_dir().join("chapter-locks");
    #[cfg(test)]
    let path = super::linear::LINEAR_TEST_CONTEXT
        .try_with(|context| context.path.with_extension("chapter-locks"))
        .unwrap_or(path);
    std::fs::create_dir_all(&path).map_err(|error| OpsError::Message(error.to_string()))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.join(format!("{}.lock", wave.id())))
        .map_err(|error| OpsError::Message(error.to_string()))?;
    // OS ownership releases on crash; provider state makes the next holder a resumer.
    for _ in 0..300 {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(Arc::new(crate::store::PlanningLocks::new(file))),
            Err(cause) if cause.kind() == std::io::ErrorKind::WouldBlock => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(cause) => return Err(OpsError::Message(cause.to_string())),
        }
    }
    Err(OpsError::Message(
        "another planning operation is active; retry after it finishes".into(),
    ))
}
