//! One close request on the Process that owns execution.
use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::store::{StoreError, StoreResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseRequest {
    Completed,
    Abandoned,
    Shell,
    RetiredShell,
}

impl CloseRequest {
    pub fn notice(self) -> &'static str {
        match self {
            Self::Completed => "Task completed",
            Self::Abandoned => "Task abandoned",
            Self::Shell | Self::RetiredShell => "Shell closed",
        }
    }

    pub fn is_shell(self) -> bool {
        matches!(self, Self::Shell | Self::RetiredShell)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
            Self::Shell => "shell",
            Self::RetiredShell => "retired_shell",
        }
    }
}

impl SqliteStore {
    pub(crate) fn owner_close_request(
        &self,
        owner: &LfProcessId,
    ) -> StoreResult<Option<CloseRequest>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let request: Option<String> = conn
            .query_row(
                "SELECT close_requested FROM processes WHERE id=?1",
                [owner],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(match request.as_deref() {
            None | Some("shell_pending") => None,
            Some("completed") => Some(CloseRequest::Completed),
            Some("abandoned") => Some(CloseRequest::Abandoned),
            Some("shell") => Some(CloseRequest::Shell),
            Some("retired_shell") => Some(CloseRequest::RetiredShell),
            Some(other) => {
                return Err(StoreError::InvalidData(format!(
                    "Unknown close request {other}"
                )))
            }
        })
    }

    /// Any intervening store write makes a cleanup decision retry next pass.
    pub(crate) fn close_revision(&self) -> StoreResult<i64> {
        Ok(self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT SUM(revision) FROM store_revisions",
            [],
            |r| r.get(0),
        )?)
    }

    pub(crate) fn request_owner_closes(
        &self,
        task: &TaskId,
        owners: &[LfProcessId],
        request: CloseRequest,
        revision: i64,
    ) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE processes SET close_requested=?3,close_task_id=?1
             WHERE id IN (SELECT value FROM json_each(?2)) AND completed_at IS NULL
               AND close_requested IS NULL
               AND (SELECT SUM(revision) FROM store_revisions)=?4",
            params![
                task.as_str(),
                serde_json::to_string(owners).expect("Process IDs serialize"),
                request.as_str(),
                revision
            ],
        )?;
        Ok(())
    }

    pub(crate) fn cancel_owner_close(
        &self,
        owner: &LfProcessId,
        request: CloseRequest,
    ) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE processes SET close_requested=NULL,close_task_id=NULL WHERE id=?1 AND close_requested=?2",
            params![owner,request.as_str()],
        )?;
        Ok(())
    }

    /// Completion and each newly launched owner receive the same viewing grace.
    pub(crate) fn task_close_candidates(
        &self,
        task: &TaskId,
        now: i64,
    ) -> StoreResult<Vec<(LfProcessId, bool)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT DISTINCT p.id,tw.abandoned_at IS NOT NULL FROM tasks tw JOIN agent_sessions s ON ({})
             JOIN processes p ON p.id=s.transport_lf_process_id
             WHERE tw.id=?1 AND s.interactive=1 AND s.completed_at IS NULL AND p.completed_at IS NULL
               AND NOT EXISTS(SELECT 1 FROM desktop_shells sh WHERE sh.owner=p.id)
               AND (tw.abandoned_at IS NOT NULL OR (tw.planning_completed=1
                 AND (CASE WHEN tw.planning_completed_at IS NULL THEN tw.updated_at
                     ELSE unixepoch(tw.planning_completed_at) END)+?3<=?2
                 AND COALESCE(p.viewer_hold_at,p.started_at)+?3<=?2
                 AND NOT EXISTS(SELECT 1 FROM task_graphs tg JOIN processes g ON g.id=tg.lf_process_id
                     WHERE tg.task_id=tw.id AND g.completed_at IS NULL)))",
            super::task_work::session_membership("s")))?;
        let owners = query
            .query_map(params![task.as_str(), now, super::WORKSPACE_GRACE], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(owners)
    }
}
