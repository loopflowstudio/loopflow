//! A served command's first answer under its request ID. A client that got no
//! reply sends the command again with the same ID and receives this answer;
//! the command does not run twice. Answers are kept for a day.
use rusqlite::{params, OptionalExtension};

use super::revisions::Revisions;
use super::SqliteStore;
use crate::StoreResult;

/// How long a request ID is remembered.
pub const ANSWER_LIFETIME_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandAnswer {
    /// Names the command, so a different command under the same ID is refused.
    pub fingerprint: String,
    /// The command's result, or the error it was refused or failed with.
    pub outcome: Result<serde_json::Value, serde_json::Value>,
    /// The domains the command advanced; none when nothing moved.
    pub revisions: Option<Revisions>,
}

impl SqliteStore {
    pub fn command_answer(&self, request_id: &str) -> StoreResult<Option<CommandAnswer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row = conn
            .query_row(
                "SELECT fingerprint,result,error,revisions FROM command_answers
                 WHERE request_id=?1 AND answered_at>?2",
                params![
                    request_id,
                    crate::rows::now_unix() - ANSWER_LIFETIME_SECONDS
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((fingerprint, result, error, revisions)) = row else {
            return Ok(None);
        };
        let outcome = match (result, error) {
            (Some(result), None) => Ok(serde_json::from_str(&result)?),
            (None, Some(error)) => Err(serde_json::from_str(&error)?),
            _ => unreachable!("command_answers holds exactly one of result and error"),
        };
        Ok(Some(CommandAnswer {
            fingerprint,
            outcome,
            revisions: revisions.as_deref().map(serde_json::from_str).transpose()?,
        }))
    }

    /// Remember `answer` under `request_id`, forgetting answers older than a day.
    pub fn record_command_answer(
        &self,
        request_id: &str,
        answer: &CommandAnswer,
    ) -> StoreResult<()> {
        let now = crate::rows::now_unix();
        let (result, error) = match &answer.outcome {
            Ok(result) => (Some(serde_json::to_string(result)?), None),
            Err(error) => (None, Some(serde_json::to_string(error)?)),
        };
        let revisions = answer
            .revisions
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        transaction.execute(
            "DELETE FROM command_answers WHERE answered_at<=?1",
            [now - ANSWER_LIFETIME_SECONDS],
        )?;
        transaction.execute(
            "INSERT OR REPLACE INTO command_answers(request_id,fingerprint,result,error,revisions,answered_at)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![request_id, answer.fingerprint, result, error, revisions, now],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::CommandAnswer;
    use crate::sqlite::Revisions;

    #[test]
    fn an_answer_is_returned_until_a_day_passes() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        assert_eq!(store.command_answer("a").unwrap(), None);
        let answer = CommandAnswer {
            fingerprint: "f".into(),
            outcome: Err(serde_json::json!({"code": "conflict"})),
            revisions: Some(Revisions([("planning".to_string(), 4)].into())),
        };
        store.record_command_answer("a", &answer).unwrap();
        assert_eq!(store.command_answer("a").unwrap(), Some(answer.clone()));
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE command_answers SET answered_at=answered_at-86400",
                [],
            )
            .unwrap();
        assert_eq!(store.command_answer("a").unwrap(), None);
        // The next answer sweeps the expired one.
        store.record_command_answer("b", &answer).unwrap();
        let rows: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM command_answers", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }
}
