//! Forward queued input directly to the provider, advancing only by bytes written.

use std::collections::VecDeque;
use std::io::{self, Write};

#[derive(Debug, Default)]
pub(super) struct PendingInput {
    bytes: VecDeque<u8>,
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

    pub fn forward(&mut self, input: &mut impl Write) {
        if self.is_empty() {
            return;
        }
        match input.write(self.bytes.as_slices().0) {
            Ok(n) => {
                self.bytes.drain(..n);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => self.bytes.clear(),
        }
    }
}
