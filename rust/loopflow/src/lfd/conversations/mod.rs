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
//! - [`turns`]: folds a live [`crate::engine::stream::StreamEvent`] sequence into
//!   [`turns::ChatTurn`]s — the wire type the wave server streams to Concerto.
//!
//! The reactive server that hosts these lives in [`crate::lfd::wave`]. (The old
//! file-based `server.rs` — `MAILBOX.md` + NDJSON sink — was rejected and is not
//! carried over.)

pub mod harness;
pub mod opencode_runtime;
pub mod turns;
pub mod types;
