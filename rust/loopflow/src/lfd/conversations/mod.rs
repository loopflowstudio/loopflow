//! Vendor stream → conversation-turn engine.
//!
//! Restored from the removed central "conversations" subsystem and re-homed as
//! the reusable engine the `lf wave` reactive server builds on. There is no
//! central conversation daemon — these modules are consumed in-process.
//!
//! Two layers:
//!
//! - [`harness`] + [`types`]: the vendor stream → conversation engine
//!   (codex/claude/opencode). It maps raw agent output into
//!   [`types::ConversationItem`]s and [`types::ConversationEvent`]s. The
//!   conformance tests under `harness/` pin this mapping against captured traces.
//! - [`turns`]: the turn vocabulary — [`turns::ChatTurn`] (the wire type the
//!   wave server streams to Concerto) and [`turns::TurnDelta`]. The mind's
//!   [`types::ConversationEvent`] stream is adapted into `TurnDelta`s (see
//!   [`crate::wave::mind::EventAdapter`]) and folded by the wave
//!   runtime's `TurnSink` into journaled, broadcast turns.
//!
//! The reactive server that hosts these lives in [`crate::wave`]. (The old
//! file-based `server.rs` — `MAILBOX.md` + NDJSON sink — was rejected and is not
//! carried over.)

pub mod harness;
pub mod opencode_runtime;
pub mod turns;
pub mod types;
