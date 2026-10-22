//! Task CI repair holds and incident-owned repair admission.

use rusqlite::{params, OptionalExtension};

use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::ops::task_automation::TaskAutomation;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

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

impl SqliteStore {
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

    pub(crate) fn task_automation(&self, task: &TaskId) -> StoreResult<TaskAutomation> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.unchecked_transaction()?;
        let issue = super::children::short_task_id(&conn, task)?.ok_or(StoreError::NotFound)?;
        Ok(conn.query_row(
            "SELECT automation_enabled AND paused=0 FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| {
                Ok(TaskAutomation {
                    task_id: task.to_string(),
                    issue,
                    enabled: row.get(0)?,
                })
            },
        )?)
    }

    pub(crate) fn set_task_automation(&self, task: &TaskId, enabled: bool) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE tasks SET automation_enabled=?2 WHERE id=?1",
            params![task.as_str(), enabled],
        )?;
        Ok(())
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
