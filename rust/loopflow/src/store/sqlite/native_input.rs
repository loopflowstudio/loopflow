//! Native editor input is held until its provider acknowledges one exact prompt.
//! This is not an editor emulator: edits, queued input and unknown controls retain
//! the native process. Neither a Return key nor a Stop alone settles a draft.

use std::sync::TryLockError;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use super::SqliteStore;
use crate::id::LfProcessId;
use crate::steer::{NativeBoundary, SteerAdapter};
use crate::store::{StoreError, StoreResult};

#[derive(Debug, Default, Serialize, Deserialize)]
struct NativeInput {
    state: State,
    escape: Vec<u8>,
    working: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
enum State {
    #[default]
    Empty,
    Text(Vec<u8>),
    Submitted(Vec<u8>),
    Acknowledged(String),
    Uncertain,
}

impl NativeInput {
    fn pending(&self) -> bool {
        !matches!(self.state, State::Empty) || !self.escape.is_empty()
    }

    fn accept(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if byte == 0x1b || !self.escape.is_empty() {
                self.escape.push(byte);
                if self.escape.len() == 1 {
                    continue;
                }
                let complete = match self.escape[1] {
                    b'[' => self.escape.len() > 2 && (0x40..=0x7e).contains(&byte),
                    b']' | b'P' => byte == 7 || self.escape.ends_with(b"\x1b\\"),
                    _ => true,
                };
                if complete || self.escape.len() > 1024 {
                    if !terminal_reply(&self.escape) {
                        self.state = State::Uncertain;
                    }
                    self.escape.clear();
                }
                continue;
            }
            if self.working {
                self.state = State::Uncertain;
                continue;
            }
            self.state = match std::mem::take(&mut self.state) {
                State::Empty if byte >= 32 && byte != 127 => State::Text(vec![byte]),
                State::Text(mut text) if byte >= 32 && byte != 127 && text.len() < 8192 => {
                    text.push(byte);
                    State::Text(text)
                }
                State::Text(text) if byte == b'\r' || byte == b'\n' => State::Submitted(text),
                _ => State::Uncertain,
            };
        }
    }

    fn start(&mut self, id: &str, text: &str) {
        match &self.state {
            State::Submitted(typed) if !self.working && typed == text.as_bytes() => {
                // Once matched, only the prompt ID is needed for settlement.
                self.state = State::Acknowledged(id.into());
            }
            State::Empty => {}
            _ => self.state = State::Uncertain,
        }
        self.working = true;
    }

    fn stop(&mut self, id: &str) {
        if matches!(&self.state, State::Acknowledged(prompt) if prompt == id) {
            self.state = State::Empty;
        }
        self.working = false;
    }
}

// Terminal replies are not editor input. Recognize only their complete wire
// forms; bracketed paste, editing keys and incomplete sequences stay protected.
fn terminal_reply(bytes: &[u8]) -> bool {
    if let Some(csi) = bytes.strip_prefix(b"\x1b[") {
        if matches!(csi, b"I" | b"O") {
            return true;
        }
        let Some((&last, body)) = csi.split_last() else {
            return false;
        };
        let numbers =
            |s: &[u8]| !s.is_empty() && s.iter().all(|b| b.is_ascii_digit() || *b == b';');
        return match last {
            b'R' => numbers(body) && body.contains(&b';'),
            b'c' => body
                .strip_prefix(b"?")
                .or_else(|| body.strip_prefix(b">"))
                .is_some_and(numbers),
            b'u' => body.strip_prefix(b"?").is_some_and(numbers),
            _ => false,
        };
    }
    let Some(osc) = bytes.strip_prefix(b"\x1b]") else {
        return false;
    };
    let Some(body) = osc
        .strip_suffix(b"\x1b\\")
        .or_else(|| osc.strip_suffix(b"\x07"))
    else {
        return false;
    };
    [b"10;rgb:".as_slice(), b"11;rgb:", b"12;rgb:"]
        .iter()
        .any(|prefix| {
            body.strip_prefix(*prefix).is_some_and(|color| {
                !color.is_empty() && color.iter().all(|b| b.is_ascii_hexdigit() || *b == b'/')
            })
        })
}

pub(super) fn pending(conn: &Connection, owner: &LfProcessId) -> StoreResult<bool> {
    Ok(read(conn, owner)?.pending())
}

fn read(conn: &Connection, owner: &LfProcessId) -> StoreResult<NativeInput> {
    let saved: Option<String> = conn
        .query_row(
            "SELECT native_input FROM processes WHERE id=?1",
            [owner],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    Ok(saved
        .map(|value| serde_json::from_str(&value))
        .transpose()?
        .unwrap_or_default())
}

fn update(
    conn: &mut Connection,
    owner: &LfProcessId,
    change: impl FnOnce(&mut NativeInput),
) -> StoreResult<()> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut input = read(&tx, owner)?;
    change(&mut input);
    let saved = serde_json::to_string(&input)?;
    tx.execute(
        "UPDATE processes SET native_input=?2 WHERE id=?1 AND native_input IS NOT ?2",
        params![owner, saved],
    )?;
    tx.commit()?;
    Ok(())
}

impl SqliteStore {
    /// Record before forwarding, without blocking the terminal's descriptor loop.
    /// False leaves the bytes with the owner to retry after store contention.
    pub(crate) fn try_record_owner_input(
        &self,
        owner: &LfProcessId,
        bytes: &[u8],
    ) -> StoreResult<bool> {
        let mut conn = match self.conn.try_lock() {
            Ok(conn) => conn,
            Err(TryLockError::WouldBlock) => return Ok(false),
            Err(TryLockError::Poisoned(_)) => panic!("store mutex poisoned"),
        };
        let previous: u32 = conn.pragma_query_value(None, "busy_timeout", |row| row.get(0))?;
        conn.busy_timeout(Duration::ZERO)?;
        let result = update(&mut conn, owner, |input| input.accept(bytes));
        conn.busy_timeout(Duration::from_millis(u64::from(previous)))?;
        match result {
            Ok(()) => Ok(true),
            Err(StoreError::Sqlite(error))
                if matches!(
                    error.sqlite_error_code(),
                    Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
                ) =>
            {
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn clear_owner_input(&self, owner: &LfProcessId) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE processes SET native_input=NULL WHERE id=?1 AND native_input IS NOT NULL",
            [owner],
        )?;
        Ok(())
    }

    pub(crate) fn acknowledge_native_input(
        &self,
        adapter: &SteerAdapter,
        thread: &str,
        prompt_id: &str,
        prompt: Option<&str>,
    ) -> StoreResult<()> {
        self.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let owner: Option<LfProcessId> = conn
                .query_row(
                    "SELECT s.transport_lf_process_id FROM agent_sessions s
                 JOIN processes p ON p.id=s.agent_process_id
                 WHERE s.id=?1 AND p.agent_interactive=1",
                    [&adapter.session],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            if let Some(owner) = owner {
                update(&mut conn, &owner, |input| match prompt {
                    Some(text) => input.start(prompt_id, text),
                    None => input.stop(prompt_id),
                })?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::NativeInput;

    #[test]
    fn only_one_exact_acknowledged_submission_can_settle_input() {
        let mut input = NativeInput::default();
        input.accept(b"draft");
        input.stop("unrelated");
        assert!(input.pending());
        input.accept(b"\r");
        assert!(input.pending());
        input.start("prompt", "draft");
        assert!(input.pending());
        input.stop("prompt");
        assert!(!input.pending());
    }

    #[test]
    fn ambiguous_or_later_input_keeps_the_native_editor() {
        for bytes in [
            b"draft\x7f\r".as_slice(),
            b"draft\rnext",
            b"draft\r\r",
            b"\x1b[200~draft\x1b[201~\r",
        ] {
            let mut input = NativeInput::default();
            input.accept(bytes);
            input.start("prompt", "draft");
            input.stop("prompt");
            assert!(input.pending(), "{bytes:?}");
        }
        let mut input = NativeInput::default();
        input.accept(b"draft\r");
        input.start("prompt", "draft");
        input.accept(b"next");
        input.stop("prompt");
        assert!(input.pending());
    }

    #[test]
    fn split_terminal_replies_do_not_create_a_draft() {
        let mut input = NativeInput::default();
        for byte in b"\x1b[1;1R\x1b[?1;2c\x1b[?7u\x1b]11;rgb:ffff/ffff/ffff\x1b\\" {
            input.accept(&[*byte]);
        }
        assert!(!input.pending());
    }

    #[test]
    fn a_split_terminal_reply_can_finish_after_prompt_settlement() {
        let mut input = NativeInput::default();
        input.accept(b"draft\r\x1b[");
        input.start("prompt", "draft");
        input.accept(b"1;");
        input.stop("prompt");
        assert!(input.pending());
        input.accept(b"1R");
        assert!(!input.pending());
    }

    #[test]
    fn missing_mismatched_and_multiple_acknowledgements_do_not_settle_a_draft() {
        let mut missing = NativeInput::default();
        missing.accept(b"draft\r");
        missing.stop("prompt");
        assert!(missing.pending());
        let mut mismatch = NativeInput::default();
        mismatch.accept(b"draft\r");
        mismatch.start("prompt", "different");
        mismatch.stop("prompt");
        assert!(mismatch.pending());
        let mut multiple = NativeInput::default();
        multiple.accept(b"draft\r");
        multiple.start("one", "draft");
        multiple.start("two", "draft");
        multiple.stop("two");
        assert!(multiple.pending());
    }
}
