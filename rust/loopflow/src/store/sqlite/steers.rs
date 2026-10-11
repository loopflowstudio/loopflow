//! Context callbacks carry provider authority, independently of the attached client.
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::SqliteStore;
use crate::process::SessionAttachment;
use crate::steer::SteerAdapter;
use crate::store::{StoreError, StoreResult};

impl SqliteStore {
    pub(crate) fn reserve_steer_adapter(
        &self,
        session: &str,
        expected: &SessionAttachment,
    ) -> StoreResult<SteerAdapter> {
        self.with_session_attachment(session, expected, || {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "INSERT INTO steer_adapters(agent_process_id,capability,provider_thread,created_at)
                 SELECT ?2,?3,provider_thread,unixepoch() FROM agent_sessions WHERE id=?1
                 ON CONFLICT DO NOTHING",
                params![
                    session,
                    expected.agent_process_id,
                    uuid::Uuid::new_v4().to_string()
                ],
            )?;
            Ok(SteerAdapter {
                session: session.into(),
                agent_process_id: expected.agent_process_id.clone(),
                capability: conn.query_row(
                    "SELECT capability FROM steer_adapters WHERE agent_process_id=?1",
                    [&expected.agent_process_id],
                    |row| row.get(0),
                )?,
            })
        })
    }

    /// Root startup activates native identity once. Other boundaries must match
    /// that binding or identity already recorded by the owned native transport.
    pub(crate) fn with_provider_context<T>(
        &self,
        adapter: &SteerAdapter,
        thread: &str,
        startup: bool,
        write: impl FnOnce(&SessionAttachment) -> StoreResult<T>,
    ) -> StoreResult<T> {
        let _fence = self.lock_session_dispatch(&adapter.session)?;
        {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let binding: Option<(Option<String>, Option<String>)> = tx
                .query_row(
                    "SELECT a.provider_thread,s.provider_thread FROM steer_adapters a
                 JOIN processes p ON p.id=a.agent_process_id
                 JOIN agent_sessions s ON s.agent_process_id=p.id
                 WHERE s.id=?1 AND p.id=?2 AND a.capability=?3 AND p.completed_at IS NULL",
                    params![
                        adapter.session,
                        adapter.agent_process_id,
                        adapter.capability
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let (bound, observed) = binding.ok_or_else(|| {
                StoreError::InvalidAuthority("Context adapter no longer owns this provider".into())
            })?;
            if thread.is_empty()
                || bound.as_deref().is_some_and(|value| value != thread)
                || observed.as_deref().is_some_and(|value| value != thread)
                || (bound.is_none() && observed.is_none() && !startup)
            {
                return Err(StoreError::InvalidAuthority(
                    "Context callback does not belong to the selected native conversation".into(),
                ));
            }
            tx.execute(
                "UPDATE steer_adapters SET provider_thread=?2,activated_at=COALESCE(activated_at,unixepoch()) WHERE agent_process_id=?1",
                params![adapter.agent_process_id, thread],
            )?;
            tx.commit()?;
        }
        let claim = self
            .session_attachment(&adapter.session)?
            .ok_or(StoreError::NotFound)?;
        write(&claim)
    }
}
