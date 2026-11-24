//! Local-save delivery to the repository's tracker or plan carriers, and the planning
//! store and locks every planning command shares.

use crate::work::wave::Wave;
use std::fs::OpenOptions;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::store::{open_store, StorageConfig, Store};
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;
use loopflow_storage::planning_sync::{ActionOwnerKind, LostAnswer, TrackerActionStatus};
use loopflow_storage::sqlite::tracker_actions::{ActionOwner, ActionPhase};

use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// Saved Tasks need no network before a read or local save. On a cache miss,
/// read the hub and the remote once, without allocating execution.
pub(crate) async fn find_task(
    store: &Store,
    repo: &str,
    selector: &str,
) -> OpsResult<Option<loopflow_storage::work::task::Task>> {
    let read = || {
        crate::store::children::resolve_task_id(&store.sqlite, selector, Some(repo))?
            .map(|id| store.sqlite.task(&id))
            .transpose()
            .map(Option::flatten)
    };
    if let Some(task) = read().map_err(message)? {
        return Ok(Some(task));
    }
    if let Err(error) = super::plan_carriers::sync(store, repo).await {
        tracing::warn!(error = %format!("{error:#}"), "plan read pending; resolving retained Task");
    }
    read().map_err(message)
}

/// A short command saves first, then makes one exchange attempt with the owner
/// it saved ahead of older work. A served command answers first and the attempt
/// runs after its reply; queued actions are durable rows, so a server that
/// stops before it runs loses nothing. Failure leaves its successful local
/// mutation and durable pending state intact.
pub(crate) async fn sync_after_save(store: &Store, repo: &str, saved: &ActionOwner) {
    let (deferred, owner) = (repo.to_owned(), saved.clone());
    if crate::journal::request::after_reply(move || {
        after_reply(|store| async move { sync_saved(&store, &deferred, &owner).await })
    })
    .is_none()
    {
        return;
    }
    sync_saved(store, repo, saved).await
}

/// Run `sync` on the Machine's store once a served command has answered.
pub(crate) fn after_reply<F, Fut>(sync: F)
where
    F: FnOnce(crate::store::SharedStore) -> Fut,
    Fut: Future<Output = ()>,
{
    let finished = tokio::runtime::Runtime::new()
        .map_err(message)
        .and_then(|runtime| {
            runtime.block_on(async {
                let store = crate::store::open_existing_store()
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("no Loopflow store on this Machine"))?;
                sync(Arc::new(store)).await;
                Ok(())
            })
        });
    if let Err(error) = finished {
        tracing::warn!(%error, "planning synchronization after reply did not start");
    }
}

pub(crate) async fn sync_saved(store: &Store, repo: &str, saved: &ActionOwner) {
    let attempt = async {
        if !super::tracker::connected(repo) {
            return super::plan_carriers::send(store, repo)
                .await
                .map_err(|error| message(format!("{error:#}")));
        }
        super::tracker::drain::drain_repository(
            store,
            repo,
            super::tracker::drain::Scope::First(saved),
            super::tracker::drain::Bound::PASS,
        )
        .await?;
        Ok(())
    };
    if let Err(error) = attempt.await {
        tracing::debug!(%error, "planning synchronization pending after local save");
        crate::journal::request::diagnostic(format_args!(
            "Saved locally; planning synchronization pending. See `lf repo tracker`."
        ));
    }
}

/// `lf repo tracker answer <id>`: answer one lost line of `repo`, whoever owns
/// it, then make one exchange attempt for that owner. `id` is the line's
/// action id or the short prefix `lf repo tracker` prints.
pub fn answer_lost(repo: &Path, id: &str, answer: LostAnswer) -> OpsResult<TrackerActionStatus> {
    tokio::runtime::Runtime::new()
        .map_err(message)?
        .block_on(async {
            let store = planning_store().await?;
            let main = crate::git::worktrees::main_repo_root(repo).map_err(message)?;
            let repo = main.to_string_lossy();
            if super::tracker::queued_tracker(&main).is_none() {
                return Err(message(format!("{repo} has no Linear or Asana tracker")));
            }
            let lost: Vec<_> = store
                .sqlite
                .repository_tracker_actions(&repo)
                .map_err(message)?
                .into_iter()
                .filter(|line| line.phase == ActionPhase::Conflicted && line.id.starts_with(id))
                .collect();
            let line = match lost.as_slice() {
                [line] => line.clone(),
                [] => {
                    return Err(message(format!(
                        "{id} is not a lost line in {repo}; `lf repo tracker` lists them"
                    )))
                }
                _ => return Err(message(format!("{id} names more than one lost line"))),
            };
            store
                .sqlite
                .answer_lost(std::slice::from_ref(&line), answer)
                .map_err(message)?;
            let owner = match line.owner_kind {
                ActionOwnerKind::Task => {
                    Some(ActionOwner::Task(TaskId::from_raw(line.owner_id.clone())))
                }
                ActionOwnerKind::Project => Some(ActionOwner::Project(ProjectId::from_raw(
                    line.owner_id.clone(),
                ))),
                _ => WaveId::parse(&line.owner_id).ok().map(ActionOwner::Wave),
            };
            if let Some(owner) = owner {
                sync_after_save(&store, &repo, &owner).await;
            }
            Ok(line)
        })
}

/// Provider exit is also a useful boundary for Taskless conversations.
pub(crate) fn check_directory(directory: &Path) -> OpsResult<()> {
    sync_directory(directory, false)
}

/// `wave status --sync`'s exchange with Linear or Asana, before it refreshes
/// the Wave inventory.
pub fn refresh_directory(directory: &Path) -> OpsResult<()> {
    sync_directory(directory, true)
}

fn sync_directory(directory: &Path, refresh: bool) -> OpsResult<()> {
    let repo = crate::repository::discover(directory)
        .map_err(message)?
        .to_string();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let store = planning_store().await?;
            if !super::tracker::connected(&repo) {
                // Nobody asked for this read, so it fetches nothing.
                return super::plan_carriers::read(&store, &repo)
                    .await
                    .map_err(message);
            }
            if refresh {
                super::tracker_events::refresh_repository(&store, &repo).await;
            } else {
                super::tracker_events::sync_repository(&store, &repo).await;
            }
            Ok(())
        })
}

pub fn storage_config_from_env() -> OpsResult<crate::store::StorageConfig> {
    #[cfg(any(test, feature = "test-support"))]
    if let Ok(config) =
        super::linear::LINEAR_TEST_CONTEXT.try_with(|ctx| StorageConfig::sqlite(ctx.path.clone()))
    {
        return Ok(config);
    }
    crate::store::storage_config_from_env()
        .map_err(|err| OpsError::Message(format!("failed to resolve planning store: {err}")))
}

pub async fn planning_store() -> OpsResult<Store> {
    open_planning_store(&storage_config_from_env()?).await
}

pub(super) async fn open_planning_store(config: &StorageConfig) -> OpsResult<Store> {
    #[cfg(any(test, feature = "test-support"))]
    if let Ok(store) = super::linear::LINEAR_TEST_CONTEXT
        .try_with(|ctx| Store::from_sqlite_for_test(ctx.store.sqlite.clone()))
    {
        return Ok(store);
    }
    let config = config.clone();
    // Store opening uses synchronous SQLite I/O. Keep it off the deadline's executor.
    crate::journal::request::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(open_store(&config))
    })
    .await
    .map_err(|_| message("Could not open the planning store"))?
    .map_err(|err| OpsError::Message(format!("failed to open planning store: {err}")))
}

pub(crate) async fn lock_wave_planning(wave: &Wave) -> OpsResult<Arc<crate::store::PlanningLocks>> {
    // Keep the existing lock inode namespace shared with already-running callers.
    let path = crate::store::lf_home_dir().join("chapter-locks");
    #[cfg(any(test, feature = "test-support"))]
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
    // The OS releases the lock if the holder exits.
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
