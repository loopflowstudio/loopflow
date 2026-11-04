//! Input stays queued until retention commits, then advances only by bytes written.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crate::id::LfProcessId;

#[derive(Debug, Default)]
pub(super) struct PendingInput {
    bytes: VecDeque<u8>,
    recorded: usize,
    retry_at: Option<Instant>,
    failure: Option<String>,
}

impl PendingInput {
    /// False leaves the queue unchanged when the owner cannot accept more input.
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        if self.bytes.len() + bytes.len() > super::LIMIT {
            return false;
        }
        self.bytes.extend(bytes);
        true
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn retry_at(&self) -> Option<Instant> {
        self.retry_at
    }

    pub fn needs_write(&self) -> bool {
        !self.is_empty() && self.retry_at.is_none()
    }

    pub fn forward(&mut self, input: &mut impl Write, owner: &LfProcessId, terminal: bool) {
        if self.is_empty() || self.retry_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        if terminal && self.recorded < self.bytes.len() && !self.retain(owner) {
            self.retry_at = Some(Instant::now() + Duration::from_millis(100));
            return;
        }
        self.retry_at = None;
        if self.failure.take().is_some() {
            tracing::info!(%owner, "terminal input store recovered; delivering queued input");
        }
        self.recorded = self.bytes.len();
        match input.write(self.bytes.as_slices().0) {
            Ok(n) => {
                self.bytes.drain(..n);
                self.recorded -= n;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => {
                self.bytes.clear();
                self.recorded = 0;
            }
        }
    }

    fn retain(&mut self, owner: &LfProcessId) -> bool {
        let bytes = &self.bytes.make_contiguous()[self.recorded..];
        match crate::journal::process_ledger()
            .and_then(|store| store.try_record_owner_input(owner, bytes))
        {
            Ok(recorded) => recorded,
            Err(error) => {
                let message = format!("cannot retain terminal input; retrying: {error}");
                super::lifetime::warn_changed(&mut self.failure, owner, message);
                false
            }
        }
    }

    pub fn native_ended(&mut self, owner: &LfProcessId) {
        if let Err(error) =
            crate::journal::process_ledger().and_then(|store| store.clear_owner_input(owner))
        {
            tracing::warn!(%error, "cannot clear ended native input; retaining its owner");
        }
        self.recorded = 0;
    }
}
