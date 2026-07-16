//! Durable measurements for the failed-CI recovery loop.

use time::OffsetDateTime;

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

    pub async fn mark_ci_incident_responded(
        &self,
        pr_id: &TaskPrId,
        failed_head_sha: &str,
        failure_set: &[String],
        responded_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let pr_id = pr_id.clone();
        let failed_head_sha = failed_head_sha.to_string();
        let failure_set = failure_set.to_vec();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_ci_incident_responded(&pr_id, &failed_head_sha, &failure_set, responded_at)
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
