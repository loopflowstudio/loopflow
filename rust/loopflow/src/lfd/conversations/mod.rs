//! Per-wave conversation subsystem.
//!
//! Restored from the removed central "conversations" subsystem, re-homed for the
//! per-wave `lf wave` runtime. There is no central conversation daemon anymore —
//! these modules are consumed in-process by the `lf wave` chat server.
//!
//! Two layers:
//!
//! - [`harness`] + [`types`]: the reusable vendor stream → conversation-turn
//!   engine (codex/claude/opencode). It maps raw agent output into
//!   [`types::ConversationItem`]s and [`types::ConversationEvent`]s. The
//!   conformance tests under `harness/` pin this mapping against captured traces.
//! - [`turns`] + [`server`]: the live per-wave chat surface. [`turns`] assembles
//!   the agent's streamed events into [`turns::ChatTurn`]s; [`server`] hosts them
//!   over an in-process HTTP API that Concerto observes.

pub mod harness;
pub mod opencode_runtime;
pub mod server;
pub mod turns;
pub mod types;
