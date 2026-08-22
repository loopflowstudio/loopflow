//! Atomic persistence for watched pull-request landings.

use time::OffsetDateTime;

use crate::pr_landing::{LandingClaim, PrLanding, PrLandingId};

use super::{run_sqlite, Store, StoreResult};

impl Store {
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

    pub async fn recoverable_pr_landings(
        &self,
        stale_before: OffsetDateTime,
    ) -> StoreResult<Vec<PrLanding>> {
        run_sqlite(&self.sqlite, move |store| {
            store.recoverable_pr_landings(stale_before)
        })
        .await
    }

    pub async fn claim_pr_landing(
        &self,
        landing_id: &PrLandingId,
        expected_generation: u64,
        claim: &LandingClaim,
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
