//! Native domain types for lfd.
//!
//! These replace the proto-generated types, giving us clean Rust types
//! without gRPC/proto dependencies.

mod attention;
mod chat_memory;
mod chat_message;
mod repo;
mod session;
mod summary;
mod wave;

pub use attention::{AttentionItem, AttentionKind, AttentionStatus};
pub use chat_memory::ChatMemoryBlock;
pub use chat_message::ChatMessage;
pub use repo::{Repo, RepoEdge, RepoId};
pub use session::{
    Session, SessionStatus, SessionUse, LF_CLI_SOURCE, LIVE_SESSION_STATUSES,
    PALETTE_TERMINAL_SOURCE, TMUX_TERMINAL_SOURCE, WAVE_SERVER_ENDPOINT_ENV, WAVE_SERVER_PID_ENV,
    WAVE_SERVER_SOURCE,
};
pub use summary::Summary;
pub use wave::{Wave, WaveStatus, DEFAULT_WAVE_FLOW};
