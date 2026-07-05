//! Shared chat and streamed turn vocabulary.
//!
//! Harness drivers produce [`types::ConversationEvent`] values; the wave
//! listener folds them into [`turns::ChatTurn`] wire frames.

pub mod turns;
pub mod types;
