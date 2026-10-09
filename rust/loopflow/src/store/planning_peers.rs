use crate::engine::planning_exchange::PlanningObject;

use crate::engine::planning_git::PlanningDestination;
use crate::id::WaveId;

use super::{run_sqlite, Store, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PeerProjectionConflict {
    pub object: PlanningObject,
    pub reason: String,
}

/// Local routing and import evidence. Endpoints are deliberately not displayed:
/// Git URLs can contain credentials. A checkpoint is not publication evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PeerPlanningStatus {
    pub id: String,
    pub reference: String,
    pub active: bool,
    pub selected_records: u64,
    pub imported_revision: Option<String>,
    pub fetched_revision: Option<String>,
    pub acquisition_error: Option<String>,
    pub publication_revision: Option<String>,
    pub publication_state: Option<String>,
    pub publication_error: Option<String>,
    pub pending_local: bool,
}

impl Store {
    pub async fn use_peer_planning(
        &self,
        repo: &str,
        destination: Option<&str>,
    ) -> StoreResult<()> {
        let repo = repo.to_string();
        let destination = destination.map(str::to_string);
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.use_peer_planning(&repo, destination.as_deref())
        })
        .await
    }

    pub async fn peer_planning_status(&self, repo: &str) -> StoreResult<Vec<PeerPlanningStatus>> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.peer_planning_status(&repo)
        })
        .await
    }

    pub async fn provision_planning_user_key(&self, key: &str) -> StoreResult<()> {
        let key = key.to_string();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.provision_planning_user_key(&key)
        })
        .await
    }

    pub async fn planning_user_key(&self) -> StoreResult<Option<String>> {
        run_sqlite(&self.sqlite, move |sqlite| sqlite.planning_user_key()).await
    }

    pub async fn bind_peer_planning(
        &self,
        repo: &str,
        destination: &PlanningDestination,
    ) -> StoreResult<String> {
        let repo = repo.to_string();
        let destination = destination.clone();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.bind_peer_planning(&repo, &destination)
        })
        .await
    }

    pub(crate) async fn peer_planning_destination_ids(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<String>> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.peer_planning_destination_ids(&repo)
        })
        .await
    }

    pub async fn select_peer_waves(
        &self,
        repo: &str,
        destination: &str,
        waves: &[WaveId],
    ) -> StoreResult<()> {
        let repo = repo.to_string();
        let destination = destination.to_string();
        let waves = waves.to_vec();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.select_peer_waves(&repo, &destination, &waves)
        })
        .await
    }

    pub async fn peer_projection_conflicts(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<PeerProjectionConflict>> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.peer_projection_conflicts(&repo)
        })
        .await
    }
}
