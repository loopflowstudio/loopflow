//! What hosts a Session's execution. An owned execution is its owner's
//! LfProcess; a held terminal is a row of its own, because the holder that
//! keeps it is one process however many terminals it holds.
use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::id::{LfProcessId, TerminalId};
use crate::store::StoreResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport<'a> {
    Owner(&'a LfProcessId),
    Terminal(&'a TerminalId),
}

impl<'a> From<&'a LfProcessId> for Transport<'a> {
    fn from(owner: &'a LfProcessId) -> Self {
        Self::Owner(owner)
    }
}

impl<'a> From<&'a TerminalId> for Transport<'a> {
    fn from(terminal: &'a TerminalId) -> Self {
        Self::Terminal(terminal)
    }
}

impl std::fmt::Display for Transport<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Transport<'_> {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Owner(owner) => owner.as_str(),
            Self::Terminal(terminal) => terminal.as_str(),
        }
    }

    /// The `agent_sessions` column naming this kind of host.
    pub(super) fn session_column(&self) -> &'static str {
        match self {
            Self::Owner(_) => "transport_lf_process_id",
            Self::Terminal(_) => "held_terminal_id",
        }
    }

    /// The table whose row carries this host's viewer hold and native input.
    pub(super) fn table(&self) -> &'static str {
        match self {
            Self::Owner(_) => "processes",
            Self::Terminal(_) => "held_terminals",
        }
    }
}

impl SqliteStore {
    /// The repository a host's work belongs to, whose settings bound its idle time.
    pub(crate) fn host_repository(&self, host: Transport<'_>) -> StoreResult<Option<String>> {
        match host {
            Transport::Owner(owner) => Ok(self.lf_process(owner)?.and_then(|process| process.repo)),
            Transport::Terminal(terminal) => {
                let conn = self.conn.lock().expect("store mutex poisoned");
                Ok(conn
                    .query_row(
                        "SELECT repo FROM agent_sessions WHERE held_terminal_id=?1 AND repo IS NOT NULL LIMIT 1",
                        [terminal],
                        |row| row.get(0),
                    )
                    .optional()?)
            }
        }
    }

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

    pub(crate) fn end_terminal(&self, terminal: &TerminalId, code: Option<u8>) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE held_terminals SET ended_at=unixepoch(),exit_code=?2,native_input=NULL
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
