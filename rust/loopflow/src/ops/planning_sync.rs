//! Foreground custom-ref exchange. Git observes/publishes documents; the common
//! writer alone imports planning. Fetch, import and publication retain separate
//! evidence. Blocking workers own effect locks through cancellation and readback.

use std::path::Path;

use crate::engine::planning_exchange::PlanningSnapshot;
use crate::engine::planning_git::{PlanningGit, PlanningPublication, PlanningRevision};
use crate::store::{sqlite::SqliteStore, Store};

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
    if let Some(task) = store.get_task_by_issue(selector).await.map_err(message)? {
        return Ok(Some(task));
    }
    if let Err(error) = acquire_repository(store, repo).await {
        tracing::warn!(%error, "planning acquisition pending; resolving retained Task");
    }
    store.get_task_by_issue(selector).await.map_err(message)
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
        let config = crate::engine::config::load_config(Some(Path::new(repo)))
            .map_err(message)?
            .unwrap_or_default();
        match config.planning_transport() {
            crate::engine::config::PlanningConfig::Git { .. } => {
                publish_repository(store, repo).await
            }
            crate::engine::config::PlanningConfig::Linear {} => {
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
    use crate::engine::config::{load_config, PlanningConfig};
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
    let destination = crate::engine::planning_git::PlanningDestination::resolve(
        Path::new(repo),
        &remote,
        &reference,
    )
    .map_err(message)?;
    let id = store
        .bind_peer_planning(repo, &destination)
        .await
        .map_err(message)?;
    store
        .use_peer_planning(repo, Some(&id))
        .await
        .map_err(message)?;
    Ok(Some(id))
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
