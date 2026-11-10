use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::id::SteerId;
use crate::steer::Steer;
use crate::store::{StoreError, StoreResult};
use crate::work::task::comment::TaskComment;
use crate::work::task::{TaskEvent, TaskId};

impl SqliteStore {
    pub(crate) fn subscription_task_events(
        &self,
        task: &TaskId,
        after: i64,
        limit: usize,
    ) -> StoreResult<Vec<TaskEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT id,task_id,kind_json,created_at FROM task_events WHERE task_id=?1 AND id>?2 ORDER BY id LIMIT ?3")?;
        let events = query
            .query_map(
                params![task.as_str(), after, limit as i64 + 1],
                super::children::map_task_event_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if events.len() > limit {
            return Err(StoreError::InvalidData(
                "subscription fell behind its events; subscribe again for current state".into(),
            ));
        }
        Ok(events)
    }

    pub(crate) fn subscription_steer(
        &self,
        task: &TaskId,
        id: &SteerId,
    ) -> StoreResult<(Steer, Option<TaskComment>)> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let conn = connection.savepoint()?;
        let steer = conn.query_row(
            &format!(
                "SELECT {} FROM steers p WHERE p.id=?1",
                super::steers::COLUMNS
            ),
            [id],
            super::steers::read_steer,
        )?;
        let comment = match steer.source.strip_prefix("comment:") {
            Some(id) => conn.query_row(
                "SELECT id,body,author,created_at FROM task_comments WHERE id=?1 AND task_id=?2",
                params![id, task.as_str()], super::task_comments::read_comment,
            ).optional()?,
            None => None,
        };
        Ok((steer, comment))
    }
}
