//! SQLite persistence for watched-landing CI recovery incidents.

use rusqlite::{params, types::Type};
use time::OffsetDateTime;

use crate::pr_landing::PrLandingId;
use crate::store::ci_incidents::CiIncidentReportRow;
use crate::store::{StoreError, StoreResult};
use crate::work::task::{CiIncident, TaskId, TaskPrId};

fn timestamp(value: OffsetDateTime) -> i64 {
    value.unix_timestamp_nanos() as i64
}

fn datetime(index: usize, value: i64) -> rusqlite::Result<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(value)).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Integer, Box::new(error))
    })
}

fn optional_datetime(index: usize, value: Option<i64>) -> rusqlite::Result<Option<OffsetDateTime>> {
    value.map(|value| datetime(index, value)).transpose()
}

fn map_incident_report_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CiIncidentReportRow> {
    let failure_set_json: String = row.get(7)?;
    let failure_set = serde_json::from_str(&failure_set_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, Type::Text, Box::new(error))
    })?;
    Ok(CiIncidentReportRow {
        incident: CiIncident {
            identity: row.get(0)?,
            landing_id: row.get::<_, Option<String>>(1)?.map(PrLandingId::from_raw),
            task_id: row.get::<_, Option<String>>(2)?.map(TaskId::from_raw),
            pr_id: row.get::<_, Option<String>>(3)?.map(TaskPrId::from_raw),
            repo: row.get(4)?,
            pr_number: row.get::<_, i64>(5)? as u32,
            failed_head_sha: row.get(6)?,
            failure_set,
            provider_completed_at: optional_datetime(8, row.get(8)?)?,
            poll_observed_at: optional_datetime(9, row.get(9)?)?,
            webhook_received_at: optional_datetime(10, row.get(10)?)?,
            claimed_landing_generation: row
                .get::<_, Option<i64>>(11)?
                .map(|generation| generation as u64),
            responded_at: optional_datetime(12, row.get(12)?)?,
            green_at: optional_datetime(13, row.get(13)?)?,
            merged_at: optional_datetime(14, row.get(14)?)?,
            blocked_at: optional_datetime(15, row.get(15)?)?,
            blocked_reason: row.get(16)?,
            created_at: datetime(17, row.get(17)?)?,
            updated_at: datetime(18, row.get(18)?)?,
            repaired_head_sha: row.get(19)?,
        },
        wave: row.get(20)?,
        task: row.get(21)?,
        task_status: row.get(22)?,
        task_started_at: optional_datetime(23, row.get(23)?)?,
        human_assisted: row.get::<_, i64>(24)? != 0,
    })
}

impl super::SqliteStore {
    pub fn observe_ci_incident(&self, incident: &CiIncident) -> StoreResult<()> {
        let failure_set = serde_json::to_string(&incident.failure_set)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO ci_incidents (
                identity, landing_id, task_id, pr_id, repo, pr_number,
                failed_head_sha, failure_set_json, provider_completed_at,
                poll_observed_at, webhook_received_at, claimed_landing_generation,
                responded_at, green_at, merged_at, blocked_at, blocked_reason,
                created_at, updated_at, repaired_head_sha
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20
             )
             ON CONFLICT(identity) DO UPDATE SET
                landing_id=COALESCE(ci_incidents.landing_id, excluded.landing_id),
                task_id=COALESCE(ci_incidents.task_id, excluded.task_id),
                pr_id=COALESCE(ci_incidents.pr_id, excluded.pr_id),
                provider_completed_at=CASE
                    WHEN ci_incidents.provider_completed_at IS NULL THEN excluded.provider_completed_at
                    WHEN excluded.provider_completed_at IS NULL THEN ci_incidents.provider_completed_at
                    ELSE MIN(ci_incidents.provider_completed_at, excluded.provider_completed_at)
                END,
                poll_observed_at=CASE
                    WHEN ci_incidents.poll_observed_at IS NULL THEN excluded.poll_observed_at
                    WHEN excluded.poll_observed_at IS NULL THEN ci_incidents.poll_observed_at
                    ELSE MIN(ci_incidents.poll_observed_at, excluded.poll_observed_at)
                END,
                webhook_received_at=CASE
                    WHEN ci_incidents.webhook_received_at IS NULL THEN excluded.webhook_received_at
                    WHEN excluded.webhook_received_at IS NULL THEN ci_incidents.webhook_received_at
                    ELSE MIN(ci_incidents.webhook_received_at, excluded.webhook_received_at)
                END,
                updated_at=MAX(ci_incidents.updated_at, excluded.updated_at)",
            params![
                incident.identity,
                incident.landing_id.as_ref().map(PrLandingId::as_str),
                incident.task_id.as_ref().map(TaskId::as_str),
                incident.pr_id.as_ref().map(TaskPrId::as_str),
                incident.repo,
                i64::from(incident.pr_number),
                incident.failed_head_sha,
                failure_set,
                incident.provider_completed_at.map(timestamp),
                incident.poll_observed_at.map(timestamp),
                incident.webhook_received_at.map(timestamp),
                incident.claimed_landing_generation.map(|value| value as i64),
                incident.responded_at.map(timestamp),
                incident.green_at.map(timestamp),
                incident.merged_at.map(timestamp),
                incident.blocked_at.map(timestamp),
                incident.blocked_reason,
                timestamp(incident.created_at),
                timestamp(incident.updated_at),
                incident.repaired_head_sha,
            ],
        )?;
        Ok(())
    }

    pub fn claim_ci_incident(
        &self,
        identity: &str,
        landing_id: &PrLandingId,
        generation: u64,
        responded_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let at = timestamp(responded_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET claimed_landing_generation=?3,
                 responded_at=COALESCE(responded_at, ?4),
                 updated_at=MAX(updated_at, ?4)
             WHERE identity=?1 AND landing_id=?2
               AND claimed_landing_generation IS NULL
               AND green_at IS NULL AND merged_at IS NULL AND blocked_at IS NULL
               AND EXISTS (
                    SELECT 1 FROM pr_landings landing
                    WHERE landing.id=?2 AND landing.generation=?3
                      AND landing.state IN ('watching', 'repairing')
               )",
            params![identity, landing_id.as_str(), generation as i64, at],
        )? > 0)
    }

    pub fn mark_ci_incident_repaired(
        &self,
        identity: &str,
        landing_id: &PrLandingId,
        generation: u64,
        repaired_head_sha: &str,
        updated_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let at = timestamp(updated_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET repaired_head_sha=COALESCE(repaired_head_sha, ?4),
                 updated_at=MAX(updated_at, ?5)
             WHERE identity=?1 AND landing_id=?2
               AND claimed_landing_generation=?3
               AND EXISTS (
                    SELECT 1 FROM pr_landings landing
                    WHERE landing.id=?2 AND landing.generation=?3
                      AND landing.state IN ('watching', 'repairing')
               )",
            params![
                identity,
                landing_id.as_str(),
                generation as i64,
                repaired_head_sha,
                at
            ],
        )? > 0)
    }

    pub fn mark_ci_incidents_green(
        &self,
        landing_id: &PrLandingId,
        generation: u64,
        green_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let at = timestamp(green_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET green_at=COALESCE(green_at, ?3), updated_at=MAX(updated_at, ?3)
             WHERE landing_id=?1 AND green_at IS NULL
               AND EXISTS (
                    SELECT 1 FROM pr_landings landing
                    WHERE landing.id=?1 AND landing.generation=?2
                      AND landing.state IN ('watching', 'repairing')
               )",
            params![landing_id.as_str(), generation as i64, at],
        )?)
    }

    pub fn mark_ci_incidents_merged(
        &self,
        landing_id: &PrLandingId,
        generation: u64,
        merged_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let at = timestamp(merged_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET merged_at=COALESCE(merged_at, ?3), updated_at=MAX(updated_at, ?3)
             WHERE landing_id=?1
               AND EXISTS (
                    SELECT 1 FROM pr_landings landing
                    WHERE landing.id=?1 AND landing.generation=?2
                      AND landing.state IN ('watching', 'repairing')
               )",
            params![landing_id.as_str(), generation as i64, at],
        )?)
    }

    pub fn mark_ci_incidents_blocked(
        &self,
        landing_id: &PrLandingId,
        generation: u64,
        blocked_at: OffsetDateTime,
        reason: &str,
    ) -> StoreResult<usize> {
        let at = timestamp(blocked_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET blocked_at=COALESCE(blocked_at, ?3), blocked_reason=?4,
                 updated_at=MAX(updated_at, ?3)
             WHERE landing_id=?1 AND green_at IS NULL AND merged_at IS NULL
               AND EXISTS (
                    SELECT 1 FROM pr_landings landing
                    WHERE landing.id=?1 AND landing.generation=?2
                      AND landing.state IN ('watching', 'repairing')
               )",
            params![landing_id.as_str(), generation as i64, at, reason],
        )?)
    }

    pub(crate) fn ci_incidents_since(
        &self,
        since: OffsetDateTime,
        wave: Option<&str>,
        repo: Option<&str>,
    ) -> StoreResult<Vec<CiIncidentReportRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT
                ci.identity, ci.landing_id, ci.task_id, ci.pr_id,
                ci.repo, ci.pr_number, ci.failed_head_sha, ci.failure_set_json,
                ci.provider_completed_at, ci.poll_observed_at, ci.webhook_received_at,
                ci.claimed_landing_generation, ci.responded_at, ci.green_at,
                ci.merged_at, ci.blocked_at, ci.blocked_reason, ci.created_at,
                ci.updated_at, ci.repaired_head_sha,
                w.name, ts.issue_identifier,
                ts.work_state,
                ts.created_at,
                EXISTS (
                    SELECT 1 FROM steers s
                    WHERE s.work_kind='task' AND s.work_id=ci.task_id
                      AND s.author_kind='user'
                      AND s.issued_at * 1000000000 >= CASE
                          WHEN ci.poll_observed_at IS NULL THEN COALESCE(ci.webhook_received_at, ci.created_at)
                          WHEN ci.webhook_received_at IS NULL THEN ci.poll_observed_at
                          ELSE MIN(ci.poll_observed_at, ci.webhook_received_at)
                      END
                      AND s.issued_at * 1000000000 <= COALESCE(ci.green_at, ci.merged_at, 9223372036854775807)
                )
             FROM ci_incidents ci
             LEFT JOIN tasks ts ON ts.id=ci.task_id
             LEFT JOIN projects p ON p.id=ts.project_id
             LEFT JOIN waves w ON w.id=p.wave_id
             WHERE COALESCE(ci.provider_completed_at, ci.poll_observed_at,
                            ci.webhook_received_at, ci.created_at) >= ?1
               AND (?2 IS NULL OR w.name=?2)
               AND (?3 IS NULL OR ci.repo=?3)
             ORDER BY COALESCE(ci.provider_completed_at, ci.poll_observed_at,
                               ci.webhook_received_at, ci.created_at) DESC",
        )?;
        let rows = statement.query_map(
            params![timestamp(since), wave, repo],
            map_incident_report_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }
}
