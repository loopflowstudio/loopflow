use crate::engine::planning_exchange::PlanningSnapshot;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn export_peer_planning(&self, repo: &str) -> StoreResult<PlanningSnapshot> {
        let repo = repo.to_string();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.export_peer_planning(&repo)
        })
        .await
    }

    pub async fn import_peer_planning(
        &self,
        repo: &str,
        destination: &str,
        revision: &str,
        incoming: &PlanningSnapshot,
    ) -> StoreResult<PlanningSnapshot> {
        let repo = repo.to_string();
        let destination = destination.to_string();
        let revision = revision.to_string();
        let incoming = incoming.clone();
        run_sqlite(&self.sqlite, move |sqlite| {
            sqlite.import_peer_planning(&repo, &destination, &revision, &incoming)
        })
        .await
    }
}
