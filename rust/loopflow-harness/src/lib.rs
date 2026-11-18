//! What each native client (Claude Code, Codex, OpenCode, Pi) writes.
//!
//! Nothing here opens Loopflow's store, starts a process or reads the process
//! table; `loopflow-execution` does that with what these modules decode.

pub mod claude;
pub mod claude_decode;
pub mod codex_decode;
pub mod fact;
mod lf_tag;
pub mod opencode_decode;
pub mod pi_decode;
