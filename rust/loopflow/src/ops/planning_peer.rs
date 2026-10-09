//! Foreground custom-ref exchange. Git observes/publishes documents; the common
//! writer alone imports planning. Fetch, import and publication retain separate
//! evidence. Blocking workers own effect locks through cancellation and readback.

use std::path::Path;

use crate::engine::planning_exchange::PlanningSnapshot;
use crate::engine::planning_git::{PlanningDocument, PlanningGit, PlanningPublication};
use crate::store::{sqlite::SqliteStore, Store};

use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

/// One acquisition per destination, also used before resolving a cold Task.
/// An unavailable remote never prevents a saved local plan from being used.
pub(crate) async fn acquire_repository(store: &Store, repo: &str) -> OpsResult<()> {
    exchange_repository(store, repo, false).await
}

pub(crate) async fn publish_repository(store: &Store, repo: &str) -> OpsResult<()> {
    exchange_repository(store, repo, true).await
}

async fn exchange_repository(store: &Store, repo: &str, publish: bool) -> OpsResult<()> {
    let destinations = store.peer_planning_status(repo).await.map_err(message)?;
    let results = futures_util::future::join_all(destinations.into_iter().map(|destination| {
        let store = store.sqlite.clone();
        let repo = repo.to_owned();
        tokio::task::spawn_blocking(move || {
            let id = &destination.id;
            // The blocking worker retains effect ownership through status writes,
            // even if its async waiter is canceled. Acquisition never takes it.
            let _lock = if publish {
                let path = store.home_dir().map_err(message)?
                    .join("locks/planning-peers").join(format!("{id}.lock"));
                let Some(lock) = super::planning_delivery::lock_delivery(&path)? else {
                    return Ok(());
                };
                Some(lock)
            } else {
                None
            };
            let result = (|| {
                // Peer provider frontiers/grouped receipts are not composed yet.
                // Do not activate the known unsafe mixed-provider path.
                if super::linear_observe::connected(&repo) {
                    return Err(message("Git planning awaits Linear provenance reconciliation; local planning is retained"));
                }
                let binding = store.peer_planning_destination(&repo, id).map_err(message)?
                    .ok_or_else(|| message("planning destination is missing"))?;
                let git = PlanningGit::new(Path::new(&repo), &binding).map_err(message)?;
                if publish {
                    publish_destination(&store, &repo, id, &git)
                } else {
                    acquire_destination(&store, &repo, id, &git).map(|_| ())
                }
            })();
            // Do not persist provider data or credential-bearing destinations in errors.
            let error = result.as_ref().err().map(|_| {
                if super::linear_observe::connected(&repo) {
                    "Git planning awaits Linear provenance reconciliation"
                } else if publish {
                    "Git publication pending; local planning retained"
                } else {
                    "Git acquisition failed; retained import unchanged"
                }
            });
            store.record_peer_error(&repo, id, !publish, error).map_err(message)?;
            result
        })
    })).await;
    // Every destination gets an independent attempt, even if another failed.
    for result in results {
        result.map_err(message)??;
    }
    Ok(())
}

fn acquire_destination(
    store: &SqliteStore,
    repo: &str,
    destination: &str,
    git: &PlanningGit,
) -> OpsResult<Option<PlanningDocument>> {
    let remote = git.fetch().map_err(message)?;
    store
        .record_peer_fetch(
            repo,
            destination,
            remote.as_ref().map(|document| document.revision.as_str()),
        )
        .map_err(message)?;
    if let Some(document) = &remote {
        let snapshot = PlanningSnapshot::from_bytes(&document.bytes).map_err(message)?;
        store
            .import_peer_planning(repo, destination, document.revision.as_str(), &snapshot)
            .map_err(message)?;
    }
    store
        .record_peer_error(repo, destination, true, None)
        .map_err(message)?;
    Ok(remote)
}

fn publish_destination(
    store: &SqliteStore,
    repo: &str,
    destination: &str,
    git: &PlanningGit,
) -> OpsResult<()> {
    // Always acquire before deciding what to publish, including recovery from a
    // lost push response. Never publish from a stale local Git document alone.
    let remote = acquire_destination(store, repo, destination, git)?;
    let exported = store
        .export_peer_planning(repo, destination)
        .map_err(message)?;
    let remote_snapshot = remote
        .as_ref()
        .map(|document| PlanningSnapshot::from_bytes(&document.bytes))
        .transpose()
        .map_err(message)?
        .unwrap_or_default();
    // Omitted/held objects retain their last shared history, not private local
    // moves. Only today's selected export may add local mutations to the remote.
    let merged = remote_snapshot.merge(&exported).map_err(message)?;
    if let Some(remote) = &remote {
        if merged == remote_snapshot {
            return store
                .record_peer_publication(
                    repo,
                    destination,
                    remote.revision.as_str(),
                    "confirmed",
                    &exported,
                )
                .map_err(message);
        }
    } else if merged.changes.is_empty() {
        return Ok(());
    }
    let bytes = merged.to_bytes().map_err(message)?;
    let local = git.local().map_err(message)?;
    let reusable = match (&local, &remote) {
        (Some(local), Some(remote)) => {
            local.bytes == bytes
                && git
                    .is_ancestor(&remote.revision, &local.revision)
                    .map_err(message)?
        }
        (Some(local), None) => local.bytes == bytes,
        _ => false,
    };
    let saved = if reusable {
        local.expect("reusable revision is present")
    } else {
        git.save(
            &bytes,
            local.as_ref().map(|d| &d.revision),
            remote.as_ref().map(|d| &d.revision),
        )
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
        PlanningPublication::Pending { .. } => "pending",
        PlanningPublication::Unconfirmed => "unconfirmed",
    };
    store
        .record_peer_publication(repo, destination, saved.revision.as_str(), state, &exported)
        .map_err(message)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use loopflow_test_support::TestRepo;

    use super::{acquire_destination, publish_destination};
    use crate::engine::planning_git::{PlanningDestination, PlanningGit};
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::wave::Wave;

    #[test]
    fn lost_publication_receipt_recovers_without_another_commit() {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let root = repo.path().canonicalize().unwrap();
        let key = root.to_str().unwrap();
        let destination =
            PlanningDestination::resolve(&root, "origin", "refs/loopflow/planning/shared/recovery")
                .unwrap();
        let id = store.bind_peer_planning(key, &destination).unwrap();
        store.use_peer_planning(key, Some(&id)).unwrap();
        store
            .create_wave(&Wave::new(WaveId::new(), "shared".into(), key.into()))
            .unwrap();
        let git = PlanningGit::new(&root, &destination).unwrap();
        publish_destination(&store, key, &id, &git).unwrap();
        let published = git.fetch().unwrap().unwrap();
        let snapshot = store.export_peer_planning(key, &id).unwrap();
        // The effect succeeded but its durable readback did not finish.
        store
            .record_peer_publication(
                key,
                &id,
                published.revision.as_str(),
                "unconfirmed",
                &snapshot,
            )
            .unwrap();
        publish_destination(&store, key, &id, &git).unwrap();
        assert_eq!(git.fetch().unwrap().unwrap(), published);
        let status = store.peer_planning_status(key).unwrap().remove(0);
        assert_eq!(status.publication_state.as_deref(), Some("confirmed"));
        assert!(!status.pending_local);
        let revisions = store.revisions().unwrap();
        publish_destination(&store, key, &id, &git).unwrap();
        assert_eq!(store.revisions().unwrap(), revisions);

        // A separate store must commit its import even when Git was already fetched.
        let other = tempfile::tempdir().unwrap();
        let receiver = SqliteStore::open_ephemeral(&other.path().join("loopflow.db")).unwrap();
        receiver.bind_peer_planning(key, &destination).unwrap();
        let connection = PlanningGit::new(Path::new(key), &destination).unwrap();
        connection.fetch().unwrap();
        assert!(receiver.peer_planning_status(key).unwrap()[0]
            .imported_revision
            .is_none());
        acquire_destination(&receiver, key, &id, &connection).unwrap();
        assert_eq!(receiver.export_peer_planning(key, &id).unwrap(), snapshot);
        assert_eq!(
            receiver.peer_planning_status(key).unwrap()[0]
                .imported_revision
                .as_deref(),
            Some(published.revision.as_str())
        );
    }
}
