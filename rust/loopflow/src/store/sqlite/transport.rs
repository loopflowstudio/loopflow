//! Held terminals: a row of their own, because the holder that keeps them is
//! one process however many terminals it holds.
use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::id::TerminalId;
use crate::store::StoreResult;

impl SqliteStore {
    /// Record a terminal before its holder is asked to start it.
    pub(crate) fn create_terminal(
        &self,
        terminal: &TerminalId,
        holder_build: &str,
    ) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "INSERT OR IGNORE INTO held_terminals(id,holder_build,created_at) VALUES(?1,?2,unixepoch())",
            params![terminal, holder_build],
        )?;
        Ok(())
    }

    /// Record that a terminal's child ended; `code` is absent when the holder
    /// died before reporting one. The first ending stands.
    /// Whether a holder still runs `terminal`'s child, as last recorded.
    pub(crate) fn terminal_running(&self, terminal: &TerminalId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM held_terminals WHERE id=?1 AND ended_at IS NULL)",
            [terminal.as_str()],
            |row| row.get(0),
        )?)
    }

    /// What an ended terminal's child exited with, once recorded with a
    /// status, and the provider its Session launched, when it has one.
    pub(crate) fn terminal_exit(
        &self,
        terminal: &TerminalId,
    ) -> StoreResult<Option<(u8, Option<String>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT t.exit_code,
                        (SELECT s.provider FROM agent_sessions s WHERE s.held_terminal_id=t.id LIMIT 1)
                 FROM held_terminals t WHERE t.id=?1 AND t.ended_at IS NOT NULL AND t.exit_code IS NOT NULL",
                [terminal.as_str()],
                |row| Ok((row.get::<_, u8>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?)
    }

    pub(crate) fn end_terminal(&self, terminal: &TerminalId, code: Option<u8>) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE held_terminals SET ended_at=unixepoch(),exit_code=?2
             WHERE id=?1 AND ended_at IS NULL",
            params![terminal, code],
        )?;
        Ok(())
    }

    /// Terminals whose child has not been recorded as ended, with their holder's build.
    pub(crate) fn running_terminals(&self) -> StoreResult<Vec<(TerminalId, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT id,holder_build FROM held_terminals WHERE ended_at IS NULL ORDER BY id",
        )?;
        let rows = query
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
}
