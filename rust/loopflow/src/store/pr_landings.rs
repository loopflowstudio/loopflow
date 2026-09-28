//! Atomic persistence for finite pull-request delivery checks.

use time::OffsetDateTime;

use crate::pr_landing::{LandingSupervisor, PrLanding, PrLandingId};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn pending_pr_landings(&self, repo: &str) -> StoreResult<Vec<PrLanding>> {
        let repo = repo.to_owned();
        run_sqlite(&self.sqlite, move |store| store.pending_pr_landings(&repo)).await
    }

    pub async fn release_pr_landing(&self, landing: &PrLanding) -> StoreResult<()> {
        let landing = landing.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.release_pr_landing(&landing)
        })
        .await
    }

    pub async fn operation_landing(
        &self,
        run_id: &crate::durable::RunId,
    ) -> StoreResult<Option<PrLanding>> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| store.operation_landing(&run_id)).await
    }

    /// Create the active landing for a PR or join the existing one.
    pub async fn start_or_join_pr_landing(&self, landing: &PrLanding) -> StoreResult<PrLanding> {
        let landing = landing.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.start_or_join_pr_landing(&landing)
        })
        .await
    }

    pub async fn get_pr_landing(&self, landing_id: &PrLandingId) -> StoreResult<Option<PrLanding>> {
        let landing_id = landing_id.clone();
        run_sqlite(&self.sqlite, move |store| store.get_pr_landing(&landing_id)).await
    }

    pub async fn claim_pr_landing(
        &self,
        landing_id: &PrLandingId,
        expected_generation: u64,
        claim: &LandingSupervisor,
        stale_before: OffsetDateTime,
    ) -> StoreResult<Option<PrLanding>> {
        let landing_id = landing_id.clone();
        let claim = claim.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_pr_landing(&landing_id, expected_generation, &claim, stale_before)
        })
        .await
    }

    pub async fn heartbeat_pr_landing(
        &self,
        landing_id: &PrLandingId,
        expected_generation: u64,
        now: OffsetDateTime,
    ) -> StoreResult<bool> {
        let landing_id = landing_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.heartbeat_pr_landing(&landing_id, expected_generation, now)
        })
        .await
    }

    pub async fn update_pr_landing(&self, landing: &PrLanding) -> StoreResult<bool> {
        let landing = landing.clone();
        run_sqlite(&self.sqlite, move |store| store.update_pr_landing(&landing)).await
    }
}
