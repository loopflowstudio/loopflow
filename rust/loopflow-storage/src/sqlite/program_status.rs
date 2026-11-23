//! Passive terminal observation shares session_activity; no attachment claim.
//! A conversation Loopflow never attached to has no AgentProcess; its absence
//! is the witness, and a later launch replaces it like any other provider.

use rusqlite::{params, OptionalExtension};

use crate::{StoreError, StoreResult};
use loopflow_model::program_status::Records;

use super::SqliteStore;

impl SqliteStore {
    pub fn begin_program_status(
        &self,
        session: &str,
        agent_process: Option<&loopflow_model::id::AgentProcessId>,
        stream: &str,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "INSERT INTO session_activity(session_id,attachment_token,observed_at,open_tools,pending_input,yielded,agent_process_id,status_stream,status_sequence)
             SELECT s.id,NULL,0,0,0,0,?2,?3,0 FROM agent_sessions s
             WHERE s.id=?1 AND s.agent_process_id IS ?2 AND s.completed_at IS NULL
             ON CONFLICT(session_id) DO UPDATE SET
                attachment_token=CASE WHEN session_activity.agent_process_id IS ?2 THEN session_activity.attachment_token ELSE NULL END,
                program_status=CASE WHEN session_activity.agent_process_id IS ?2 THEN session_activity.program_status END,
                preview=CASE WHEN session_activity.agent_process_id IS ?2 THEN session_activity.preview END,
                observed_at=CASE WHEN session_activity.agent_process_id IS ?2 THEN session_activity.observed_at ELSE 0 END,
                agent_process_id=?2,status_stream=?3,status_sequence=0",
            params![session, agent_process, stream],
        )? == 1)
    }

    pub fn record_program_status(
        &self,
        session: &str,
        agent_process: Option<&loopflow_model::id::AgentProcessId>,
        stream: &str,
        sequence: i64,
        records: &Records,
    ) -> StoreResult<bool> {
        if sequence <= 0 || !records.seen || !records.validate() {
            return Err(StoreError::InvalidData(
                "invalid Program Status snapshot".into(),
            ));
        }
        let json = serde_json::to_string(records)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE session_activity SET program_status=?5,status_sequence=?4
             WHERE session_id=?1 AND agent_process_id IS ?2 AND status_stream=?3 AND status_sequence<?4
             AND EXISTS(SELECT 1 FROM agent_sessions s WHERE s.id=?1 AND s.agent_process_id IS ?2 AND s.completed_at IS NULL)",
            params![session, agent_process, stream, sequence, json],
        )? == 1)
    }

    /// Record a held terminal's Program Status as the status of the Session
    /// it hosts, under that Session's current AgentProcess. The server is its
    /// only writer; a new provider or server starts a new stream. False when
    /// the terminal hosts no open Session.
    pub fn record_held_program_status(
        &self,
        terminal: &loopflow_model::id::TerminalId,
        sequence: i64,
        records: &Records,
    ) -> StoreResult<bool> {
        let hosted = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.query_row(
                "SELECT id,agent_process_id FROM agent_sessions
                 WHERE held_terminal_id=?1 AND completed_at IS NULL
                 ORDER BY created_at DESC LIMIT 1",
                [terminal.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<loopflow_model::id::AgentProcessId>>(1)?,
                    ))
                },
            )
            .optional()?
        };
        let Some((session, agent)) = hosted else {
            return Ok(false);
        };
        let stream = format!("held:{terminal}");
        if self.record_program_status(&session, agent.as_ref(), &stream, sequence, records)? {
            return Ok(true);
        }
        Ok(
            self.begin_program_status(&session, agent.as_ref(), &stream)?
                && self.record_program_status(
                    &session,
                    agent.as_ref(),
                    &stream,
                    sequence,
                    records,
                )?,
        )
    }

    /// Whether a held terminal hosts `session`: its server records the
    /// Session's Program Status, and no view's reading replaces it.
    pub fn session_is_held(&self, session: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT held_terminal_id IS NOT NULL FROM agent_sessions WHERE id=?1",
                [session],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false))
    }
}
