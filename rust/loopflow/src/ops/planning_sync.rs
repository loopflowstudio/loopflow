//! Foreground planning exchange for the repository's selected transport.
//! Acquisition and delivery run independently; the common writer alone changes Work.
//! Blocking workers retain their locks through cancellation, commit and readback.

use crate::work::wave::Wave;
use std::fs::OpenOptions;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::planning_exchange::PlanningSnapshot;
use crate::planning_git::{PlanningGit, PlanningPublication, PlanningRevision};
use crate::store::{open_store, sqlite::SqliteStore, SharedStore, StorageConfig, Store};

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
    let scope = crate::repository::CanonicalRepo::discover(Path::new(repo)).map_err(message)?;
    let read = || {
        store
            .sqlite
            .resolve_task_id(selector, Some(&scope.to_string()))?
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
            crate::config::PlanningConfig::Linear {} => {
                tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    super::planning_export::sync_repository_exports(store, repo).await?;
                    let (fields, comments, state) = tokio::join!(
                        super::planning_delivery::sync_repository_fields(store, repo),
                        super::linear_observe::sync_repository_deliveries(store, repo, false),
                        super::linear_observe::sync_repository_deliveries(store, repo, true)
                    );
                    fields?;
                    comments?;
                    state?;
                    Ok(())
                })
                .await
                .map_err(message)?
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
    use crate::config::{load_config, PlanningConfig};
    let config = load_config(Some(Path::new(repo)))
        .map_err(message)?
        .unwrap_or_default();
    let PlanningConfig::Git { remote, shared } = config.planning_transport() else {
        return Ok(None);
    };
    let reference = match shared {
        Some(name) => format!("refs/loopflow/planning/shared/{name}"),
        None => {
            let key = match store.planning_user_key().await.map_err(message)? {
                Some(key) => key,
                None => {
                    let key = uuid::Uuid::new_v4().to_string();
                    let result = store.provision_planning_user_key(&key).await;
                    match store.planning_user_key().await.map_err(message)? {
                        Some(saved) => saved,
                        None => {
                            result.map_err(message)?;
                            key
                        }
                    }
                }
            };
            format!("refs/loopflow/planning/users/{key}")
        }
    };
    let destination =
        crate::planning_git::PlanningDestination::resolve(Path::new(repo), &remote, &reference)
            .map_err(message)?;
    let id = store
        .bind_peer_planning(repo, &destination)
        .await
        .map_err(message)?;
    Ok(Some(id))
}

/// Passive status uses configuration, never a persisted copy of its selection.
/// Retained routes remain readable in a Work folder without a Git checkout.
pub(crate) async fn peer_status(
    store: &Store,
    repo: &str,
) -> OpsResult<Vec<crate::store::PeerPlanningStatus>> {
    let config = crate::config::load_config(Some(Path::new(repo)))
        .map_err(message)?
        .unwrap_or_default();
    let crate::config::PlanningConfig::Git { remote, shared } = config.planning_transport() else {
        return Ok(Vec::new());
    };
    let reference = match shared {
        Some(name) => Some(format!("refs/loopflow/planning/shared/{name}")),
        None => store
            .planning_user_key()
            .await
            .map_err(message)?
            .map(|key| format!("refs/loopflow/planning/users/{key}")),
    };
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
    store
        .peer_planning_status(repo, active.as_deref())
        .await
        .map_err(message)
}

async fn exchange_repository(store: &Store, repo: &str, publish: bool) -> OpsResult<()> {
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

/// A foreground connection's independent planning acquisition and delivery.
/// Dropping the connection cancels requests; durable identities survive cancellation.
#[derive(Debug)]
pub(crate) struct PlanningSync {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl PlanningSync {
    /// Repository planning follows every foreground provider, not Task attribution.
    pub(crate) fn start_for_directory(directory: &std::path::Path) -> OpsResult<Self> {
        super::task::block_on_task(async {
            let store = super::planning_sync::planning_store().await?;
            let repo = crate::repository::CanonicalRepo::discover(directory)
                .map_err(|error| OpsError::Message(error.to_string()))?
                .to_string();
            Self::start(std::sync::Arc::new(store), repo)
                .map_err(|error| OpsError::Message(error.to_string()))
        })
    }

    pub(crate) fn start(store: SharedStore, repo: String) -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        #[cfg(test)]
        let context = super::linear::LINEAR_TEST_CONTEXT
            .try_with(Clone::clone)
            .ok();
        let thread = std::thread::Builder::new()
            .name("planning-sync".into())
            .spawn(move || {
                let drive = async {
                    tokio::select! {
                        _ = stopped => {},
                        _ = repeat_sync("Git planning acquisition", Duration::from_secs(5), || {
                            acquire_repository(&store, &repo)
                        }) => {},
                        _ = repeat_sync("Git planning publication", Duration::from_secs(1), || {
                            publish_repository(&store, &repo)
                        }) => {},
                        _ = repeat_sync("comment acquisition", Duration::from_secs(15), || {
                            super::linear_observe::refresh_repository_comments(&store, &repo)
                        }) => {},
                        _ = repeat_sync("planning acquisition", Duration::from_secs(15), || async {
                            tokio::time::timeout(Duration::from_secs(5), super::linear_observe::refresh_repository_planning(&store, &repo))
                                .await.map_err(|error| OpsError::Message(error.to_string()))?
                        }) => {},
                        _ = repeat_sync("comment delivery", Duration::from_secs(1), || {
                            super::linear_observe::sync_repository_deliveries(&store, &repo, false)
                        }) => {},
                        _ = repeat_sync("planning export", Duration::from_secs(1), || {
                            super::planning_export::sync_repository_exports(&store, &repo)
                        }) => {},
                        _ = repeat_sync("field delivery", Duration::from_secs(1), || {
                            super::planning_delivery::sync_repository_fields(&store, &repo)
                        }) => {},
                        _ = repeat_sync("Task state delivery", Duration::from_secs(1), || {
                            super::linear_observe::sync_repository_deliveries(&store, &repo, true)
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
        Ok(Self {
            stop: Some(stop),
            thread: Some(thread),
        })
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
