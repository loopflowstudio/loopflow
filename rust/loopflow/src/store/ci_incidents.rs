//! Durable measurements for the failed-CI recovery loop.

use time::OffsetDateTime;

use crate::pr_landing::PrLandingId;
use crate::work::task::CiIncident;

use super::{run_sqlite, Store, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CiIncidentReportRow {
    pub incident: CiIncident,
    pub wave: Option<String>,
    pub task: Option<String>,
    pub task_started_at: Option<OffsetDateTime>,
    pub task_status: Option<String>,
    pub human_assisted: bool,
}

impl Store {
    /// Preserve one failing poll observation without changing wake ownership.
    pub async fn observe_ci_incident(&self, incident: &CiIncident) -> StoreResult<()> {
        let incident = incident.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.observe_ci_incident(&incident)
        })
        .await
    }

    /// Claim one unsettled incident for the current landing generation.
    pub async fn claim_ci_incident(
        &self,
        identity: &str,
        landing_id: &PrLandingId,
        generation: u64,
        responded_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let identity = identity.to_string();
        let landing_id = landing_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_ci_incident(&identity, &landing_id, generation, responded_at)
        })
        .await
    }

    /// Record the head a ci-fix body shipped for this incident. First-write only,
    /// so the head that originally settled the incident survives a retry or a
    /// later push.
    pub async fn mark_ci_incident_repaired(
        &self,
        identity: &str,
        landing_id: &PrLandingId,
        generation: u64,
        repaired_head_sha: &str,
        updated_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let identity = identity.to_string();
        let landing_id = landing_id.clone();
        let repaired_head_sha = repaired_head_sha.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incident_repaired(
                &identity,
                &landing_id,
                generation,
                &repaired_head_sha,
                updated_at,
            )
        })
        .await
    }

    pub async fn mark_ci_incidents_green(
        &self,
        landing_id: &PrLandingId,
        generation: u64,
        green_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let landing_id = landing_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incidents_green(&landing_id, generation, green_at)
        })
        .await
    }

    pub async fn mark_ci_incidents_merged(
        &self,
        landing_id: &PrLandingId,
        generation: u64,
        merged_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let landing_id = landing_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incidents_merged(&landing_id, generation, merged_at)
        })
        .await
    }

    pub async fn mark_ci_incidents_blocked(
        &self,
        landing_id: &PrLandingId,
        generation: u64,
        blocked_at: OffsetDateTime,
        reason: &str,
    ) -> StoreResult<usize> {
        let landing_id = landing_id.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incidents_blocked(&landing_id, generation, blocked_at, &reason)
        })
        .await
    }

    pub(crate) async fn ci_incidents_since(
        &self,
        since: OffsetDateTime,
        wave: Option<&str>,
        repo: Option<&str>,
    ) -> StoreResult<Vec<CiIncidentReportRow>> {
        let wave = wave.map(str::to_string);
        let repo = repo.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.ci_incidents_since(since, wave.as_deref(), repo.as_deref())
        })
        .await
    }
}
