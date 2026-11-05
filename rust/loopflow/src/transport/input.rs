//! Input stays queued while retention is busy, then advances only by bytes written.
//! A failed store never holds a person's input; the owner holds it instead.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crate::id::LfProcessId;
use crate::store::RecordingGap;

#[derive(Debug, Default)]
pub(super) struct PendingInput {
    bytes: VecDeque<u8>,
    recorded: usize,
    retry_at: Option<Instant>,
    failure: RecordingGap,
    /// Bytes reached the provider without a retention record. The owner holds
    /// them in memory until the native client ends, since the store cannot.
    unrecorded: bool,
}

/// What retaining queued bytes in the store came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Retained {
    Recorded,
    /// A busy store: wait and try the same bytes again.
    Busy,
    /// A failed store: deliver the bytes; the person's turn is not lost for a record.
    Failed,
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

    /// Input that keeps this owner alive: queued bytes, or delivered bytes the
    /// store never recorded.
    pub fn holds_owner(&self) -> bool {
        !self.is_empty() || self.unrecorded
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
        if terminal && self.recorded < self.bytes.len() {
            match self.retain(owner) {
                Retained::Recorded => self.failure.recovered(),
                Retained::Busy => {
                    self.retry_at = Some(Instant::now() + Duration::from_millis(100));
                    return;
                }
                Retained::Failed => self.unrecorded = true,
            }
        }
        self.retry_at = None;
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

    fn retain(&mut self, owner: &LfProcessId) -> Retained {
        let bytes = &self.bytes.make_contiguous()[self.recorded..];
        match crate::journal::process_ledger()
            .and_then(|store| store.try_record_owner_input(owner, bytes))
        {
            Ok(true) => Retained::Recorded,
            Ok(false) => Retained::Busy,
            Err(error) => {
                self.failure
                    .note("terminal input, delivered unrecorded", &error);
                Retained::Failed
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
        self.unrecorded = false;
    }
}
