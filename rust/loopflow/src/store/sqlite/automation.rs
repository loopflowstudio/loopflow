//! Task CI repair holds and incident-owned repair admission.

use rusqlite::params;

use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::ops::task_automation::TaskAutomation;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

#[derive(Debug)]
pub(crate) struct RepairReservation {
    pub process: Option<LfProcessId>,
    pub retries: u32,
    pub finished: Option<i64>,
    pub conclusion: Option<String>,
    pub error: Option<String>,
}

impl SqliteStore {
    pub(crate) fn ci_response_complete(&self, identity: &str) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT repair_conclusion IS NOT NULL OR (responded_at IS NOT NULL AND repair_lf_process_id IS NULL) FROM ci_incidents WHERE identity=?1", [identity], |row| row.get(0))?)
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
        Ok(conn.query_row("SELECT repair_lf_process_id,repair_retries,repair_finished_at,repair_error,repair_conclusion FROM ci_incidents WHERE identity=?1", [identity], |row| Ok(RepairReservation {
            process: row.get(0)?, retries: row.get(1)?, finished: row.get(2)?, error: row.get(3)?, conclusion: row.get(4)?,
        }))?)
    }

    /// Reserve an incident's repair for `process`; the run it launches takes
    /// the reservation over, and settlement records the Session it opened.
    pub(crate) fn reserve_repair(
        &self,
        identity: &str,
        generation: u64,
        process: &LfProcessId,
        retry: bool,
        task: Option<&TaskId>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(task) = task {
            if super::durable::task_state_in(&tx, task)? == crate::durable::TaskState::Paused {
                return Err(StoreError::InvalidAuthority(
                    "Task is paused; CI repair is held".into(),
                ));
            }
        }
        let changed = tx.execute("UPDATE ci_incidents SET repair_lf_process_id=?3,repair_session_id=NULL,repair_retries=repair_retries+?4,repair_finished_at=NULL,repair_error=NULL,claimed_landing_generation=?2
            WHERE identity=?1 AND EXISTS(SELECT 1 FROM pr_landings p WHERE p.id=ci_incidents.landing_id AND p.generation=?2 AND p.state IN ('watching','repairing','blocked'))",params![identity,generation as i64,process.as_str(),retry])? == 1;
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

    /// Settle an exited run: the Session and input it captured, if any, and
    /// its conclusion or the reason it reached none.
    pub(crate) fn finish_repair(
        &self,
        identity: &str,
        process: &LfProcessId,
        captured: Option<(&str, Option<i64>)>,
        error: Option<&str>,
        conclusion: Option<&str>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let (session, capture) = captured.unzip();
        let capture = capture.flatten();
        if tx.execute("UPDATE ci_incidents SET repair_finished_at=?3,repair_error=?4,repair_conclusion=?5,repair_session_id=?6 WHERE identity=?1 AND repair_lf_process_id=?2 AND repair_finished_at IS NULL",params![identity,process.as_str(),now,error,conclusion,session])? == 0 {
            // Another check already settled this run.
            return Ok(());
        }
        if conclusion.is_some() {
            tx.execute("UPDATE agent_sessions SET completed_at=?3 WHERE id=?1 AND current_capture=?2 AND completed_at IS NULL", params![session,capture,now])?;
        }
        tx.commit()?;
        Ok(())
    }
}
