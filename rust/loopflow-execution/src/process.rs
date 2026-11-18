//! Process records describe OS lifetimes; runtime resources hold exact attachment authority.

pub mod attachment;
pub mod launch_log;
pub mod os_process;
mod runtime;
pub use runtime::LfProcessRuntime;
pub use runtime::{
    finish_session_attachment, register_session_attachment_interrupt, resume_session_agent_process,
};

pub use loopflow_storage::process::*;
pub use os_process::{OsProcess, RecordedProcess};

pub const AGENT_CALLER_ENV: &str = "LF_AGENT_CALLER";

/// A command already rendered its diagnostic and selected its process status.
#[derive(Debug, thiserror::Error)]
#[error("command exited with status {0}")]
pub struct CommandExit(pub u8);

/// A Graph stopped where a person or a watcher takes over: blocked, or short of
/// its last step. Its process exits with [`GraphHeld::EXIT`], so a caller can
/// tell it from a Graph that failed.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct GraphHeld(pub String);

impl GraphHeld {
    pub const EXIT: u8 = 3;
}

pub mod diagnosis;
