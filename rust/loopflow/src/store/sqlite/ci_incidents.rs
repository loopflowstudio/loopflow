//! SQLite persistence for watched-landing CI recovery incidents and the
//! admission of the repairs they own.

use rusqlite::{params, types::Type, OptionalExtension};
use time::OffsetDateTime;

use crate::id::LfProcessId;
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
        task: None,
        task_status: row.get(21)?,
        task_started_at: optional_datetime(22, row.get(22)?)?,
        human_assisted: row.get::<_, i64>(23)? != 0,
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

    pub fn record_ci_response(
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
                 responded_at=?4,
                 updated_at=MAX(updated_at, ?4)
             WHERE identity=?1 AND landing_id=?2 AND responded_at IS NULL
               AND EXISTS (
                    SELECT 1 FROM pr_landings landing
                    WHERE landing.id=?2 AND landing.generation=?3
                      AND landing.state IN ('watching', 'repairing', 'blocked')
               )",
            params![identity, landing_id.as_str(), generation as i64, at],
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

    /// Processes holding an unfinished repair for this landing, newest first.
    pub(crate) fn landing_repair_processes(
        &self,
        landing_id: &PrLandingId,
    ) -> StoreResult<Vec<crate::id::LfProcessId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT repair_lf_process_id FROM ci_incidents
             WHERE landing_id=?1 AND repair_lf_process_id IS NOT NULL AND repair_finished_at IS NULL
             ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([landing_id.as_str()], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub(crate) fn ci_incidents_since(
        &self,
        since: OffsetDateTime,
        wave: Option<&str>,
        repo: Option<&str>,
    ) -> StoreResult<Vec<CiIncidentReportRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.unchecked_transaction()?;
        let mut statement = conn.prepare(&format!(
            "SELECT
                ci.identity, ci.landing_id, ci.task_id, ci.pr_id,
                ci.repo, ci.pr_number, ci.failed_head_sha, ci.failure_set_json,
                ci.provider_completed_at, ci.poll_observed_at, ci.webhook_received_at,
                ci.claimed_landing_generation, ci.responded_at, ci.green_at,
                ci.merged_at, ci.blocked_at, ci.blocked_reason, ci.created_at,
                ci.updated_at, ci.repaired_head_sha,
                w.slug,
                CASE WHEN ts.id IS NOT NULL THEN {task_state} END,
                (SELECT min(o.at) * 1000000000 FROM task_ownership o
                 WHERE o.task_id=ci.task_id AND o.kind IN ('claim','recorded')),
                EXISTS (
                    SELECT 1 FROM task_events event
                    JOIN steers direction ON direction.id=json_extract(event.kind_json, '$.steer_id')
                    WHERE event.task_id=ci.task_id
                      AND json_extract(event.kind_json, '$.kind')='steer'
                      AND json_extract(direction.author, '$.kind') IN ('user','named')
                      AND event.created_at * 1000000000 >= CASE
                          WHEN ci.poll_observed_at IS NULL THEN COALESCE(ci.webhook_received_at, ci.created_at)
                          WHEN ci.webhook_received_at IS NULL THEN ci.poll_observed_at
                          ELSE MIN(ci.poll_observed_at, ci.webhook_received_at)
                      END
                      AND event.created_at * 1000000000 <= COALESCE(ci.green_at, ci.merged_at, 9223372036854775807)
                )
             FROM ci_incidents ci
             LEFT JOIN tasks ts ON ts.id=ci.task_id
             LEFT JOIN projects p ON p.id=ts.project_id
             LEFT JOIN wave_addresses w ON w.id=p.wave_id
             WHERE COALESCE(ci.provider_completed_at, ci.poll_observed_at,
                            ci.webhook_received_at, ci.created_at) >= ?1
               AND (?2 IS NULL OR w.slug=?2)
               AND (?3 IS NULL OR ci.repo=?3)
             ORDER BY COALESCE(ci.provider_completed_at, ci.poll_observed_at,
                               ci.webhook_received_at, ci.created_at) DESC",
            task_state = super::durable::task_state_sql("ts")
        ))?;
        let rows = statement.query_map(
            params![timestamp(since), wave, repo],
            map_incident_report_row,
        )?;
        let mut incidents = rows.collect::<Result<Vec<_>, _>>()?;
        let mut labels = std::collections::HashMap::new();
        for row in &mut incidents {
            row.task = row
                .incident
                .task_id
                .as_ref()
                .map(|id| super::children::cached_short_task_id(&conn, &mut labels, id))
                .transpose()?
                .flatten();
        }
        Ok(incidents)
    }
}

// ---- Repair admission

#[derive(Debug)]
pub(crate) struct RepairReservation {
    pub process: Option<LfProcessId>,
    pub session: Option<String>,
    pub retries: u32,
    pub finished: Option<i64>,
    pub conclusion: Option<String>,
    pub error: Option<String>,
}

/// One repair attempt for a CI incident, as its row records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RepairAttempt {
    /// No repair responded yet, or one is still starting or running.
    Open,
    /// The attempt concluded, or its startup exhausted its retries.
    Finished {
        /// Unix seconds.
        ended_at: i64,
    },
}

impl super::SqliteStore {
    /// `None` when no incident row has this identity yet.
    pub(crate) fn repair_attempt(
        &self,
        identity: &str,
        startup_retries: u32,
    ) -> StoreResult<Option<RepairAttempt>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT repair_conclusion IS NOT NULL
                     OR (responded_at IS NOT NULL AND repair_lf_process_id IS NULL)
                     OR (repair_finished_at IS NOT NULL AND repair_retries >= ?2),
                     COALESCE(repair_finished_at, responded_at / 1000000000, updated_at / 1000000000)
                 FROM ci_incidents WHERE identity=?1",
                params![identity, startup_retries],
                |row| {
                    Ok(if row.get::<_, bool>(0)? {
                        RepairAttempt::Finished {
                            ended_at: row.get(1)?,
                        }
                    } else {
                        RepairAttempt::Open
                    })
                },
            )
            .optional()?)
    }

    pub(crate) fn repair_reservation(&self, identity: &str) -> StoreResult<RepairReservation> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("SELECT repair_lf_process_id,repair_session_id,repair_retries,repair_finished_at,repair_error,repair_conclusion FROM ci_incidents WHERE identity=?1", [identity], |row| Ok(RepairReservation {
            process: row.get(0)?, session: row.get(1)?, retries: row.get(2)?, finished: row.get(3)?, error: row.get(4)?, conclusion: row.get(5)?,
        }))?)
    }

    pub(crate) fn reserve_repair(
        &self,
        identity: &str,
        generation: u64,
        process: &LfProcessId,
        retry: bool,
        session: crate::session::LfSession,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(task) = &session.task_id {
            if super::durable::task_state_in(&tx, task)? == crate::durable::TaskState::Paused {
                return Err(StoreError::InvalidAuthority(
                    "Task is paused; CI repair is held".into(),
                ));
            }
        }
        let session = match super::sessions::session_in(&tx, &session.id)? {
            Some(mut existing) if retry => {
                existing.artifact_key = Some(crate::session_record::new_artifact_key());
                existing.input_published = false;
                super::sessions::replace_input_in(&tx, &mut existing, Some(process))?;
                existing
            }
            Some(existing) => existing,
            None => super::sessions::reserve_session_in(&tx, session, Some(process))?,
        };
        let changed = tx.execute("UPDATE ci_incidents SET repair_lf_process_id=?3,repair_session_id=?5,repair_retries=repair_retries+?4,repair_finished_at=NULL,repair_error=NULL,claimed_landing_generation=?2
            WHERE identity=?1 AND EXISTS(SELECT 1 FROM pr_landings p WHERE p.id=ci_incidents.landing_id AND p.generation=?2 AND p.state IN ('watching','repairing','blocked'))",params![identity,generation as i64,process.as_str(),retry,session.id])? == 1;
        if changed {
            tx.commit()?;
        }
        Ok(changed)
    }

    pub(crate) fn handoff_repair(
        &self,
        identity: &str,
        launcher: &LfProcessId,
        worker: &LfProcessId,
    ) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").execute("UPDATE ci_incidents SET repair_lf_process_id=?3,responded_at=COALESCE(responded_at,?4) WHERE identity=?1 AND repair_lf_process_id=?2 AND repair_finished_at IS NULL", params![identity,launcher.as_str(),worker.as_str(),time::OffsetDateTime::now_utc().unix_timestamp_nanos() as i64])? == 1)
    }

    pub(crate) fn finish_repair(
        &self,
        identity: &str,
        process: &LfProcessId,
        error: Option<&str>,
        conclusion: Option<&str>,
        captured: Option<i64>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        if tx.execute("UPDATE ci_incidents SET repair_finished_at=?3,repair_error=?4,repair_conclusion=?5 WHERE identity=?1 AND repair_lf_process_id=?2",params![identity,process.as_str(),now,error,conclusion])? != 1 {
            return Err(StoreError::InvalidAuthority("repair reservation changed before completion".into()));
        }
        if conclusion.is_some() {
            tx.execute("UPDATE agent_sessions SET completed_at=?3 WHERE id=(SELECT repair_session_id FROM ci_incidents WHERE identity=?1) AND current_capture=?2 AND completed_at IS NULL", params![identity,captured,now])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn incident_landing(&self, identity: &str) -> StoreResult<Option<String>> {
        Ok(self
            .conn
            .lock()
            .expect("store mutex poisoned")
            .query_row(
                "SELECT landing_id FROM ci_incidents WHERE identity=?1",
                [identity],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }
}
