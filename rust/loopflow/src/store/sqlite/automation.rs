//! Automation selection belongs to Task; repair admission belongs to its CI incident.
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::durable::TaskId;
use crate::id::ExecId;
use crate::ops::task_automation::TaskAutomation;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

#[derive(Debug)]
pub(crate) struct RepairReservation {
    pub exec: Option<ExecId>,
    pub session: Option<String>,
    pub retries: u32,
    pub finished: Option<i64>,
    pub conclusion: Option<String>,
    pub error: Option<String>,
}

impl SqliteStore {
    pub(crate) fn ci_response_complete(&self, identity: &str) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT repair_conclusion IS NOT NULL OR (responded_at IS NOT NULL AND repair_exec_id IS NULL) FROM ci_incidents WHERE identity=?1", [identity], |row| row.get(0))?)
    }

    pub(crate) fn lock_checkout(&self, cwd: &Path) -> StoreResult<Vec<File>> {
        self.lock_task_checkouts(&[cwd], None)
    }

    pub(crate) fn lock_task_checkouts(
        &self,
        workspaces: &[&Path],
        task: Option<&TaskId>,
    ) -> StoreResult<Vec<File>> {
        let canonical = |path: &Path| {
            crate::store::canonicalize_with_missing_tail(path)
                .map_err(|error| StoreError::InvalidData(error.to_string()))
        };
        let workspaces = workspaces
            .iter()
            .map(|path| canonical(path))
            .collect::<StoreResult<Vec<_>>>()?;
        let mut paths = Vec::<PathBuf>::new();
        for checkout in self.task_checkouts()? {
            if checkout.worktree.as_os_str().is_empty() {
                continue;
            }
            let path = canonical(&checkout.worktree)?;
            if workspaces.iter().any(|cwd| cwd.starts_with(&path))
                || task == Some(&checkout.task_id)
            {
                paths.push(path);
            }
        }
        for cwd in workspaces {
            paths.push(canonical(
                &crate::engine::git::worktree_root(&cwd).unwrap_or(cwd),
            )?);
        }
        paths.sort();
        paths.dedup();
        paths
            .iter()
            .map(|path| self.lock_checkout_path(path))
            .collect()
    }

    fn lock_checkout_path(&self, cwd: &Path) -> StoreResult<File> {
        let database = self
            .conn
            .lock()
            .expect("store mutex poisoned")
            .path()
            .map(str::to_string);
        let root = Path::new(database.as_deref().unwrap_or("/tmp/loopflow.db"))
            .with_extension("admission");
        std::fs::create_dir_all(&root)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let name = hex::encode(Sha256::digest(cwd.as_os_str().as_encoded_bytes()));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(name))
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => break,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(StoreError::InvalidData(format!(
                        "checkout admission unavailable: {error}"
                    )))
                }
            }
        }
        Ok(file)
    }

    pub(crate) fn task_automation(&self, task: &TaskId) -> StoreResult<TaskAutomation> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("SELECT automation_enabled,automation_exec_id,automation_retry_key,automation_retries,automation_checked_at,automation_detail,issue_identifier FROM tasks WHERE id=?1", [task.as_str()], |row| Ok(TaskAutomation {
            task_id: task.to_string(), issue: row.get(6)?, enabled: row.get(0)?, exec_id: row.get(1)?, retry_key: row.get(2)?, retries: row.get(3)?, checked_at: row.get(4)?, detail: row.get(5)?,
        }))?)
    }

    pub(crate) fn set_task_automation(
        &self,
        task: &TaskId,
        enabled: bool,
        only_unset: bool,
    ) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE tasks SET automation_enabled=?2, automation_retries=CASE WHEN ?3=0 AND ?2=1 THEN 0 ELSE automation_retries END WHERE id=?1 AND (?3=0 OR automation_enabled IS NULL)", params![task.as_str(), enabled, only_unset])?;
        Ok(())
    }

    pub(crate) fn record_automation(&self, state: &TaskAutomation) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE tasks SET automation_exec_id=?2,automation_retry_key=?3,automation_retries=?4,automation_checked_at=?5,automation_detail=?6 WHERE id=?1",
            params![state.task_id,state.exec_id,state.retry_key,state.retries,state.checked_at,state.detail])?;
        Ok(())
    }

    pub(crate) fn repair_reservation(&self, identity: &str) -> StoreResult<RepairReservation> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("SELECT repair_exec_id,repair_session_id,repair_retries,repair_finished_at,repair_error,repair_conclusion FROM ci_incidents WHERE identity=?1", [identity], |row| Ok(RepairReservation {
            exec: row.get(0)?, session: row.get(1)?, retries: row.get(2)?, finished: row.get(3)?, error: row.get(4)?, conclusion: row.get(5)?,
        }))?)
    }

    pub(crate) fn reserve_repair(
        &self,
        identity: &str,
        generation: u64,
        exec: &ExecId,
        retry: bool,
        session: crate::session::AgentSession,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let session = match super::sessions::session_in(&tx, &session.id)? {
            Some(mut existing) if retry => {
                existing.artifact_key = crate::session_record::new_artifact_key();
                existing.input_published = false;
                super::sessions::replace_input_in(&tx, &mut existing, Some(exec))?;
                existing
            }
            Some(existing) => existing,
            None => super::sessions::reserve_session_in(&tx, session, Some(exec))?,
        };
        let changed = tx.execute("UPDATE ci_incidents SET repair_exec_id=?3,repair_session_id=?5,repair_retries=repair_retries+?4,repair_finished_at=NULL,repair_error=NULL,claimed_landing_generation=?2
            WHERE identity=?1 AND EXISTS(SELECT 1 FROM pr_landings p WHERE p.id=ci_incidents.landing_id AND p.generation=?2 AND p.state IN ('watching','repairing','blocked'))",params![identity,generation as i64,exec.as_str(),retry,session.id])? == 1;
        if changed {
            tx.commit()?;
        }
        Ok(changed)
    }

    pub(crate) fn handoff_repair(
        &self,
        identity: &str,
        launcher: &ExecId,
        worker: &ExecId,
    ) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").execute("UPDATE ci_incidents SET repair_exec_id=?3,responded_at=COALESCE(responded_at,?4) WHERE identity=?1 AND repair_exec_id=?2 AND repair_finished_at IS NULL", params![identity,launcher.as_str(),worker.as_str(),time::OffsetDateTime::now_utc().unix_timestamp_nanos() as i64])? == 1)
    }

    pub(crate) fn finish_repair(
        &self,
        identity: &str,
        exec: &ExecId,
        error: Option<&str>,
        conclusion: Option<&str>,
        captured: Option<i64>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        if tx.execute("UPDATE ci_incidents SET repair_finished_at=?3,repair_error=?4,repair_conclusion=?5 WHERE identity=?1 AND repair_exec_id=?2",params![identity,exec.as_str(),now,error,conclusion])? != 1 {
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

    pub(crate) fn ci_timeout(
        &self,
        landing: &str,
        head: &str,
        attempt: Option<&str>,
        now: i64,
        allowance: u32,
    ) -> StoreResult<Option<bool>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("UPDATE pr_landings SET ci_pending_head=?2,ci_pending_attempt=?4,ci_pending_since=?3 WHERE id=?1 AND (ci_pending_head IS NULL OR ci_pending_head!=?2 OR ci_pending_attempt IS NOT ?4)",params![landing,head,now,attempt])?;
        Ok(conn.query_row("SELECT CASE WHEN ci_pending_since<=?2-1800 THEN ci_timeout_reruns<?3 ELSE NULL END FROM pr_landings WHERE id=?1",params![landing,now,allowance],|row| row.get(0))?)
    }

    pub(crate) fn consume_timeout_rerun(&self, landing: &str, now: i64) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute("UPDATE pr_landings SET ci_timeout_reruns=ci_timeout_reruns+1,ci_pending_since=?2 WHERE id=?1",params![landing,now])?;
        Ok(())
    }
}
