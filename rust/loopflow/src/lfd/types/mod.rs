//! Native domain types for lfd.
//!
//! These replace the proto-generated types, giving us clean Rust types
//! without gRPC/proto dependencies.

mod attention;
mod chat_memory;
mod chat_message;
mod event;
mod execution;
mod repo;
mod session;
mod summary;
mod trigger;
mod wave;

pub use attention::{AttentionItem, AttentionKind, AttentionStatus};
pub use chat_memory::ChatMemoryBlock;
pub use chat_message::ChatMessage;
pub use event::Event;
pub use execution::{ExecutionProcess, ExecutionProcessStatus};
pub use repo::{Repo, RepoEdge, RepoId};
pub use session::{
    tmux_session_name, Session, SessionStatus, SessionUse, LIVE_SESSION_STATUSES,
    PALETTE_TERMINAL_SOURCE, TMUX_TERMINAL_SOURCE,
};
pub use summary::Summary;
pub use trigger::{
    ActivationLog, ActivationOutcome, PendingActivation, Signal, Trigger, CI_FIX_FLOW,
};
pub use wave::{
    LivePrState, LivePullRequestState, PullRequest, QueueBlock, QueueBlockReason, QueueMergeEvent,
    Run, RunStackStatus, RunStatus, Wave, WaveCron, WaveMode, WaveStatus,
};
