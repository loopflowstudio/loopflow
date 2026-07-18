//! Durable measurements for the failed-CI recovery loop.

use time::OffsetDateTime;

use crate::durable::RunId;
use crate::task::{CiIncident, TaskPrId, TaskSessionStatus};

use super::{run_sqlite, Store, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CiIncidentReportRow {
    pub incident: CiIncident,
    pub wave: String,
    pub task: String,
    pub task_started_at: OffsetDateTime,
    pub task_status: TaskSessionStatus,
    pub task_status_reason: String,
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

    /// Claim one unsettled incident for an exact active Run and stamp response.
    pub async fn claim_ci_incident(
        &self,
        identity: &str,
        run_id: &RunId,
        responded_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let identity = identity.to_string();
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_ci_incident(&identity, &run_id, responded_at)
        })
        .await
    }

    /// Record the head a ci-fix body shipped for this incident. First-write only,
    /// so the head that originally settled the incident survives a retry or a
    /// later push.
    pub async fn mark_ci_incident_repaired(
        &self,
        identity: &str,
        repaired_head_sha: &str,
        updated_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let identity = identity.to_string();
        let repaired_head_sha = repaired_head_sha.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incident_repaired(&identity, &repaired_head_sha, updated_at)
        })
        .await
    }

    pub async fn mark_ci_incidents_green(
        &self,
        pr_id: &TaskPrId,
        green_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let pr_id = pr_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incidents_green(&pr_id, green_at)
        })
        .await
    }

    pub async fn mark_ci_incidents_merged(
        &self,
        pr_id: &TaskPrId,
        merged_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let pr_id = pr_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incidents_merged(&pr_id, merged_at)
        })
        .await
    }

    pub async fn mark_ci_incidents_blocked(
        &self,
        pr_id: &TaskPrId,
        blocked_at: OffsetDateTime,
        reason: &str,
    ) -> StoreResult<usize> {
        let pr_id = pr_id.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incidents_blocked(&pr_id, blocked_at, &reason)
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
