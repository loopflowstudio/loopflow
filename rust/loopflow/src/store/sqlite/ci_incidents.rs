//! SQLite persistence for historical CI recovery incidents.

use std::str::FromStr;

use rusqlite::{params, types::Type};
use time::OffsetDateTime;

use crate::child_session::ChildCommandId;
use crate::store::ci_incidents::CiIncidentReportRow;
use crate::store::{StoreError, StoreResult};
use crate::task::{CiIncident, TaskPrId, TaskSessionId, TaskSessionStatus};

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

pub(super) fn mark_ci_incident_responded_on(
    conn: &rusqlite::Connection,
    pr_id: &TaskPrId,
    failed_head_sha: &str,
    failure_set: &[String],
    responded_at: OffsetDateTime,
) -> StoreResult<bool> {
    let failure_set = serde_json::to_string(failure_set)?;
    let at = timestamp(responded_at);
    Ok(conn.execute(
        "UPDATE ci_incidents
         SET responded_at=COALESCE(responded_at, ?4), updated_at=MAX(updated_at, ?4)
         WHERE pr_id=?1 AND failed_head_sha=?2 AND failure_set_json=?3",
        params![pr_id.as_str(), failed_head_sha, failure_set, at],
    )? > 0)
}

fn map_incident_report_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CiIncidentReportRow> {
    let failure_set_json: String = row.get(6)?;
    let failure_set = serde_json::from_str(&failure_set_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, Type::Text, Box::new(error))
    })?;
    let task_status_value: String = row.get(20)?;
    let task_status = TaskSessionStatus::from_str(&task_status_value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(20, Type::Text, Box::new(error))
    })?;
    Ok(CiIncidentReportRow {
        incident: CiIncident {
            identity: row.get(0)?,
            task_session_id: TaskSessionId::from_raw(row.get::<_, String>(1)?),
            pr_id: TaskPrId::from_raw(row.get::<_, String>(2)?),
            repo: row.get(3)?,
            pr_number: row.get::<_, i64>(4)? as u32,
            failed_head_sha: row.get(5)?,
            failure_set,
            provider_completed_at: optional_datetime(7, row.get(7)?)?,
            poll_observed_at: optional_datetime(8, row.get(8)?)?,
            webhook_received_at: optional_datetime(9, row.get(9)?)?,
            trigger_command_id: row
                .get::<_, Option<String>>(10)?
                .map(ChildCommandId::from_raw),
            responded_at: optional_datetime(11, row.get(11)?)?,
            green_at: optional_datetime(12, row.get(12)?)?,
            merged_at: optional_datetime(13, row.get(13)?)?,
            blocked_at: optional_datetime(14, row.get(14)?)?,
            blocked_reason: row.get(15)?,
            created_at: datetime(16, row.get(16)?)?,
            updated_at: datetime(17, row.get(17)?)?,
        },
        wave: row.get(18)?,
        task: row.get(19)?,
        task_status,
        task_status_reason: row.get(21)?,
        task_started_at: datetime(22, row.get(22)?)?,
        human_assisted: row.get::<_, i64>(23)? != 0,
    })
}

impl super::SqliteStore {
    pub fn observe_ci_incident(&self, incident: &CiIncident) -> StoreResult<()> {
        let failure_set = serde_json::to_string(&incident.failure_set)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO ci_incidents (
                identity, task_session_id, pr_id, repo, pr_number,
                failed_head_sha, failure_set_json, provider_completed_at,
                poll_observed_at, webhook_received_at, trigger_command_id,
                responded_at, green_at, merged_at, blocked_at, blocked_reason,
                created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18
             )
             ON CONFLICT(identity) DO UPDATE SET
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
                trigger_command_id=COALESCE(ci_incidents.trigger_command_id, excluded.trigger_command_id),
                updated_at=MAX(ci_incidents.updated_at, excluded.updated_at)",
            params![
                incident.identity,
                incident.task_session_id.as_str(),
                incident.pr_id.as_str(),
                incident.repo,
                i64::from(incident.pr_number),
                incident.failed_head_sha,
                failure_set,
                incident.provider_completed_at.map(timestamp),
                incident.poll_observed_at.map(timestamp),
                incident.webhook_received_at.map(timestamp),
                incident.trigger_command_id.as_ref().map(ChildCommandId::as_str),
                incident.responded_at.map(timestamp),
                incident.green_at.map(timestamp),
                incident.merged_at.map(timestamp),
                incident.blocked_at.map(timestamp),
                incident.blocked_reason,
                timestamp(incident.created_at),
                timestamp(incident.updated_at),
            ],
        )?;
        Ok(())
    }

    pub fn mark_ci_incident_responded(
        &self,
        pr_id: &TaskPrId,
        failed_head_sha: &str,
        failure_set: &[String],
        responded_at: OffsetDateTime,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        mark_ci_incident_responded_on(&conn, pr_id, failed_head_sha, failure_set, responded_at)
    }

    pub fn mark_ci_incidents_green(
        &self,
        pr_id: &TaskPrId,
        green_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let at = timestamp(green_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET green_at=?2, updated_at=MAX(updated_at, ?2)
             WHERE pr_id=?1 AND green_at IS NULL",
            params![pr_id.as_str(), at],
        )?)
    }

    pub fn mark_ci_incidents_merged(
        &self,
        pr_id: &TaskPrId,
        merged_at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let at = timestamp(merged_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET merged_at=COALESCE(merged_at, ?2), updated_at=MAX(updated_at, ?2)
             WHERE pr_id=?1",
            params![pr_id.as_str(), at],
        )?)
    }

    pub fn mark_ci_incidents_blocked(
        &self,
        pr_id: &TaskPrId,
        blocked_at: OffsetDateTime,
        reason: &str,
    ) -> StoreResult<usize> {
        let at = timestamp(blocked_at);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE ci_incidents
             SET blocked_at=COALESCE(blocked_at, ?2), blocked_reason=?3,
                 updated_at=MAX(updated_at, ?2)
             WHERE pr_id=?1 AND green_at IS NULL AND merged_at IS NULL",
            params![pr_id.as_str(), at, reason],
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
                ci.identity, ci.task_session_id, ci.pr_id, ci.repo, ci.pr_number,
                ci.failed_head_sha, ci.failure_set_json, ci.provider_completed_at,
                ci.poll_observed_at, ci.webhook_received_at, ci.trigger_command_id,
                ci.responded_at, ci.green_at, ci.merged_at, ci.blocked_at,
                ci.blocked_reason, ci.created_at, ci.updated_at,
                w.name, ts.issue_identifier, ts.status, ts.status_reason,
                ts.created_at,
                EXISTS (
                    SELECT 1 FROM child_commands cc
                    WHERE cc.target_kind='task'
                      AND cc.session_id=ci.task_session_id
                      AND json_extract(cc.source_json, '$.kind') IN ('human', 'linear')
                      AND cc.created_at >= CASE
                          WHEN ci.poll_observed_at IS NULL THEN ci.webhook_received_at
                          WHEN ci.webhook_received_at IS NULL THEN ci.poll_observed_at
                          ELSE MIN(ci.poll_observed_at, ci.webhook_received_at)
                      END
                      AND cc.created_at <= COALESCE(ci.green_at, ci.merged_at, 9223372036854775807)
                )
             FROM ci_incidents ci
             JOIN task_sessions ts ON ts.id=ci.task_session_id
             JOIN waves w ON w.id=ts.wave_id
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
