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
    if publish_repository(store, repo).await.is_err() {
        eprintln!("Saved locally; Git planning sync pending. See `lf planning status`.");
    }
}

async fn exchange_repository(store: &Store, repo: &str, publish: bool) -> OpsResult<()> {
    let destinations = store
        .peer_planning_destination_ids(repo)
        .await
        .map_err(message)?;
    let results = futures_util::future::join_all(destinations.into_iter().map(|id| {
        let store = store.sqlite.clone();
        let repo = repo.to_owned();
        tokio::task::spawn_blocking(move || exchange_destination(&store, &repo, &id, publish))
    }))
    .await;
    // Every destination gets an independent attempt, even if another failed.
    for result in results {
        result.map_err(message)??;
    }
    Ok(())
}

fn exchange_destination(store: &SqliteStore, repo: &str, id: &str, publish: bool) -> OpsResult<()> {
    let linear = super::linear_observe::connected(repo);
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
        // Peer provider frontiers/grouped receipts are not composed yet.
        // Do not activate the known unsafe mixed-provider path.
        if linear {
            return Err(message(
                "Git planning awaits Linear provenance reconciliation; local planning is retained",
            ));
        }
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
        if linear {
            "Git planning awaits Linear provenance reconciliation"
        } else if publish {
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

    use super::{acquire_destination, publish_destination, publish_repository};
    use crate::engine::planning_git::{PlanningDestination, PlanningGit};
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::store::{open_ephemeral_store, StorageConfig};
    use crate::work::wave::Wave;

    #[tokio::test]
    async fn damaged_journal_does_not_block_an_independent_destination() {
        let repo = TestRepo::new();
        repo.create_file(".lf/config.yaml", "pm: null\n");
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("loopflow.db");
        let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let root = repo.path().canonicalize().unwrap();
        let key = root.to_str().unwrap();
        let mut plans = Vec::new();
        for name in ["damaged", "independent"] {
            let binding = PlanningDestination::resolve(
                &root,
                "origin",
                &format!("refs/loopflow/planning/shared/{name}"),
            )
            .unwrap();
            let id = store.bind_peer_planning(key, &binding).await.unwrap();
            store.use_peer_planning(key, Some(&id)).await.unwrap();
            let wave = Wave::new(WaveId::new(), name.into(), key.into());
            store.create_wave(&wave).await.unwrap();
            plans.push((binding, wave));
        }
        let conn = rusqlite::Connection::open(database).unwrap();
        conn.execute(
            "DELETE FROM planning_peer_heads WHERE object_id=?1",
            [plans[0].1.id()],
        )
        .unwrap();
        assert!(store.peer_planning_status(key).await.is_err());

        // Dispatch cannot require rendering every plan's status first.
        assert!(publish_repository(&store, key).await.is_err());
        let damaged = PlanningGit::new(&root, &plans[0].0).unwrap();
        assert!(damaged.fetch().unwrap().is_none());
        let independent = PlanningGit::new(&root, &plans[1].0).unwrap();
        let published = independent.fetch().unwrap().unwrap();
        assert_eq!(
            crate::engine::planning_exchange::PlanningSnapshot::from_bytes(&published.bytes)
                .unwrap(),
            store
                .export_peer_planning(key, &plans[1].0.id())
                .await
                .unwrap()
        );
        let error: String = conn
            .query_row(
                "SELECT publication_error FROM planning_destinations WHERE repo=?1 AND id=?2",
                rusqlite::params![key, plans[0].0.id()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(error.contains("local planning retained"));
    }

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
