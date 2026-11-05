//! A caller's create/start identity survives lost replies and process exit.
//! The command owns the per-request OS lock across effects; SQLite owns intent.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::durable::TaskId;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

#[derive(Debug)]
pub(crate) struct TaskCreateRequest {
    pub task_id: TaskId,
    pub start_attempted: bool,
    pub result: Option<String>,
}

impl SqliteStore {
    pub(crate) fn reserve_task_create(
        &self,
        id: &str,
        request: &str,
    ) -> StoreResult<TaskCreateRequest> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<(String, String, bool, Option<String>)> = tx
            .query_row(
                "SELECT request_json,task_id,start_attempted,result_json
                 FROM task_create_requests WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let reservation = if let Some((saved, task, start_attempted, result)) = previous {
            if saved != request {
                return Err(StoreError::InvalidData(
                    "creation request ID already belongs to a different request".into(),
                ));
            }
            TaskCreateRequest {
                task_id: TaskId::from_raw(task),
                start_attempted,
                result,
            }
        } else {
            let task_id = TaskId::new();
            tx.execute(
                "INSERT INTO task_create_requests(id,request_json,task_id) VALUES(?1,?2,?3)",
                params![id, request, task_id.as_str()],
            )?;
            TaskCreateRequest {
                task_id,
                start_attempted: false,
                result: None,
            }
        };
        tx.commit()?;
        Ok(reservation)
    }

    pub(crate) fn begin_task_create_start(&self, id: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE task_create_requests SET start_attempted=1
             WHERE id=?1 AND start_attempted=0 AND result_json IS NULL",
            [id],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidData(
                "Task start was already attempted".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn finish_task_create(&self, id: &str, result: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE task_create_requests SET result_json=?2
             WHERE id=?1 AND start_attempted=1 AND result_json IS NULL",
            params![id, result],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidData(
                "Task creation has no pending start".into(),
            ));
        }
        Ok(())
    }
}
