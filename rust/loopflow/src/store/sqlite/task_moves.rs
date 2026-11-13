//! Machine-local moves of a Task: `leaving` from the claim naming its target,
//! then `sent` when this Machine sent it, until it arrives. A leaving move
//! keeps its claim and the Graph it stopped, so running the same move again
//! carries on leaving; a sent move keeps what it sent, so running it again
//! delivers it without stopping, committing or pushing again. The target
//! records a move it `checked` it can take, naming the Machine it comes from.
use rusqlite::{params, OptionalExtension};

use crate::durable::{MachineId, OwnershipEntryId, TaskId};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

/// A move of the Task this Machine began that has not arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveInFlight {
    pub to_machine_id: MachineId,
    /// When the move began; sending it keeps this time.
    pub at: i64,
    pub stage: MoveStage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveStage {
    /// Its claim naming the target is written; stopping, committing, pushing
    /// or taking its conversation did not finish.
    Leaving {
        claim_id: OwnershipEntryId,
        /// The Graph the move stopped, as JSON; absent until one stopped.
        stopped: Option<String>,
    },
    /// Sent, with the move as sent.
    Sent { payload: String },
}

impl SqliteStore {
    /// The move stopped the Task's Graph: keep it, so carrying on leaving
    /// continues the step it stopped.
    pub fn record_move_stopped(&self, task: &TaskId, stopped: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE task_moves SET stopped=?2 WHERE task_id=?1 AND state='leaving'",
            params![task.as_str(), stopped],
        )?;
        Ok(())
    }

    /// The leaving move was sent as `payload`. It stays one move, as old as
    /// when it began; a repeat of a sent move returns its payload instead.
    pub(crate) fn record_move_sent(&self, task: &TaskId, payload: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE task_moves SET state='sent',claim_id=NULL,stopped=NULL,payload=?2
             WHERE task_id=?1 AND state='leaving'",
            params![task.as_str(), payload],
        )?;
        if updated == 0 {
            return Err(StoreError::InvalidData(format!(
                "Task {task} has no move leaving"
            )));
        }
        Ok(())
    }

    /// A move of the Task from `from` checked that this Machine can take it.
    /// Until it arrives, running the move here asks `from` for it.
    pub(crate) fn record_move_checked(&self, task: &TaskId, from: &MachineId) -> StoreResult<()> {
        let here = self.local_machine()?.id;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM task_moves WHERE task_id=?1 AND state='checked'",
            [task.as_str()],
        )?;
        tx.execute(
            "INSERT INTO task_moves(task_id,from_machine_id,to_machine_id,state,at)
             VALUES(?1,?2,?3,'checked',?4)",
            params![task.as_str(), from.as_str(), here.as_str(), now_unix()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// The Machine a move of the Task checked here comes from, when that is
    /// this Machine's latest record of it.
    pub fn task_move_checked_from(&self, task: &TaskId) -> StoreResult<Option<MachineId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT from_machine_id FROM task_moves WHERE task_id=?1
                 AND id=(SELECT max(id) FROM task_moves WHERE task_id=?1) AND state='checked'",
                [task.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(MachineId::from_raw))
    }

    /// The Task arrived on `to`: the sender's record keeps only that, and the
    /// receiver records it too. A Task arriving here ends any move this
    /// Machine sent of it earlier, even one whose arrival it never heard of.
    pub(crate) fn record_move_arrived(
        &self,
        task: &TaskId,
        from: &MachineId,
        to: &MachineId,
    ) -> StoreResult<()> {
        let here = &self.local_machine()?.id == to;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let updated = tx.execute(
            "UPDATE task_moves SET state='arrived',at=?3,payload=NULL,from_machine_id=?4
             WHERE task_id=?1 AND to_machine_id=?2 AND state IN ('checked','sent')",
            params![task.as_str(), to.as_str(), now_unix(), from.as_str()],
        )?;
        if updated == 0 {
            tx.execute(
                "INSERT INTO task_moves(task_id,from_machine_id,to_machine_id,state,at)
                 VALUES(?1,?2,?3,'arrived',?4)",
                params![task.as_str(), from.as_str(), to.as_str(), now_unix()],
            )?;
        }
        if here {
            tx.execute(
                "UPDATE task_moves SET state='arrived',claim_id=NULL,stopped=NULL,payload=NULL
                 WHERE task_id=?1 AND state IN ('leaving','sent')",
                [task.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Whether the last move of the Task this Machine knows of arrived on `to`.
    pub fn task_move_arrived_on(&self, task: &TaskId, to: &MachineId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT state='arrived' AND to_machine_id=?2 FROM task_moves
                 WHERE task_id=?1 ORDER BY id DESC LIMIT 1",
                [task.as_str(), to.as_str()],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false))
    }

    /// The Task's move this Machine began that has not arrived. At most one
    /// is in flight: beginning or sending a move replaces the last.
    pub fn task_move_in_flight(&self, task: &TaskId) -> StoreResult<Option<MoveInFlight>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT to_machine_id,at,claim_id,stopped,payload FROM task_moves
                 WHERE task_id=?1 AND state IN ('leaving','sent') ORDER BY id DESC LIMIT 1",
                [task.as_str()],
                |row| {
                    let stage = match row.get::<_, Option<String>>(2)? {
                        Some(claim_id) => MoveStage::Leaving {
                            claim_id: OwnershipEntryId::from_raw(claim_id),
                            stopped: row.get(3)?,
                        },
                        None => MoveStage::Sent {
                            payload: row.get(4)?,
                        },
                    };
                    Ok(MoveInFlight {
                        to_machine_id: MachineId::from_raw(row.get::<_, String>(0)?),
                        at: row.get(1)?,
                        stage,
                    })
                },
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::MoveStage;
    use crate::durable::{MachineId, TaskId};
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::NewTask;

    #[test]
    fn a_sent_move_stays_until_it_arrives_and_then_keeps_no_payload() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let repo = dir.path().canonicalize().unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id: store.ensure_repository(repo.to_str().unwrap()).unwrap(),
                project_id: None,
                title: "Moving".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap()
            .id;
        let (laptop, mini) = (store.local_machine().unwrap().id, MachineId::new());
        assert_eq!(store.task_move_in_flight(&task).unwrap(), None);

        // Leaving, then sent: still one move, as old as when it began.
        store.begin_task_move(&task, &mini).unwrap();
        let began = 1_000;
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE task_moves SET at=?1", [began])
            .unwrap();
        store.record_move_sent(&task, "{\"first\":1}").unwrap();
        let sent = store.task_move_in_flight(&task).unwrap().unwrap();
        assert_eq!(sent.to_machine_id, mini);
        assert_eq!(sent.at, began);
        assert_eq!(
            sent.stage,
            MoveStage::Sent {
                payload: "{\"first\":1}".into()
            }
        );

        store.record_move_arrived(&task, &laptop, &mini).unwrap();
        assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
        let conn = store.conn.lock().unwrap();
        let rows: Vec<(String, Option<String>)> = conn
            .prepare("SELECT state,payload FROM task_moves WHERE task_id=?1")
            .unwrap()
            .query_map([task.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, [("arrived".to_string(), None)]);
        drop(conn);
        assert!(store.task_move_arrived_on(&task, &mini).unwrap());

        // Sent on to a third Machine, whose arrival this one never hears of,
        // then back here: nothing is in flight any more.
        let third = MachineId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO task_moves(task_id,from_machine_id,to_machine_id,state,at,payload)
                 VALUES(?1,?2,?3,'sent',1,'{}')",
                [task.as_str(), laptop.as_str(), third.as_str()],
            )
            .unwrap();
        assert!(!store.task_move_arrived_on(&task, &third).unwrap());
        store.record_move_arrived(&task, &third, &laptop).unwrap();
        assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
        assert!(store.task_move_arrived_on(&task, &laptop).unwrap());
    }
}
